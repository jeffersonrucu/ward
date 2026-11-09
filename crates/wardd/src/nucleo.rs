//! Estado do daemon e regra de negócio de cada operação (Fase 22).
//!
//! - **Travado / destravado.** Destravado, o daemon guarda um único [`CofreAberto`]
//!   (dono único, sem cópia) e as entradas. Travar solta os dois: a chave de dados, o
//!   hash do PIN e os segredos zeram ao sair (S8, V3.8).
//! - **Destravar** (D20): a tentativa espera 0 s, e cada senha errada seguida dobra a
//!   espera da próxima a partir de 1 s, até 30 s. A contagem é global (todas as
//!   conexões) e a resposta não diz quantas houve.
//! - **PIN** (S9): confere pelo hash da configuração do cofre (`HashPin`, em tempo
//!   constante, D30); 5 erros seguidos travam o cofre, mesmo reconectando.
//! - **Gerações** (D39, D53): o arquivo de estado guarda `vista` e `queimada`. Abre com
//!   (`vista`, `queimada`); antes de cada gravação grava `queimada` = geração + 1 e só
//!   então grava o cofre; depois da gravação que deu certo, `vista` = geração. Sem
//!   estado com o cofre existente, não destrava.
//! - **Identidade** (D39): `dev`/inode/`mtime`/tamanho do cofre por `fstat` depois de
//!   cada leitura e gravação; antes de gravar (e ao reabrir) confere. Mudou: não grava,
//!   trava e responde [`ErroResposta::CofreMudou`].
//! - Nenhuma resposta ao agente tem nível (D50), e uma entrada bloqueada responde igual
//!   a uma inexistente (V3.17).

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use zeroize::Zeroizing;

use ward_core::configuracao::{Configuracao, HashPin};
use ward_core::corpo::{self, ErroAbrir, ErroCorpo};
use ward_core::entrada::{Entrada, NivelAcesso, Rascunho, Segredo};
use ward_core::formato::{CofreAberto, SenhaMestra};
use ward_core::gravacao::{self, ErroGravacao, Etapa};
use ward_core::protocolo::{
    DetalheEntrada, ErroResposta, ItemLista, Papel, Pedido, Problema, Resposta,
};

use crate::arquivos::{self, Geracoes, Identidade};
use crate::{aviso, depurar};

/// Erros de PIN seguidos que travam o cofre (S9).
pub const MAX_ERROS_PIN: u32 = 5;
/// Maior espera do destravar (D20).
pub const ESPERA_MAXIMA: Duration = Duration::from_secs(30);

/// Espera antes da próxima tentativa de destravar, depois de `falhas` senhas erradas
/// seguidas (D20): 0, 1, 2, 4, 8, 16, 30, 30… segundos.
pub fn espera_destrave(falhas: u32) -> Duration {
    if falhas == 0 {
        return Duration::ZERO;
    }
    let segundos = 1u64.checked_shl(falhas - 1).unwrap_or(u64::MAX);
    Duration::from_secs(segundos).min(ESPERA_MAXIMA)
}

/// Relógio do daemon (injetável nos testes).
pub trait Relogio: Send + Sync {
    fn dormir(&self, d: Duration);
}

/// O relógio do sistema.
pub struct RelogioReal;

impl Relogio for RelogioReal {
    fn dormir(&self, d: Duration) {
        if !d.is_zero() {
            std::thread::sleep(d);
        }
    }
}

/// Onde ficam o cofre e o arquivo de estado.
#[derive(Debug, Clone)]
pub struct Caminhos {
    pub cofre: PathBuf,
    pub estado: PathBuf,
}

/// Cofre destravado.
struct Aberto {
    cofre: CofreAberto,
    /// Em ordem de nome.
    entradas: Vec<Entrada>,
    /// Identidade do arquivo depois da última leitura ou gravação. `None`: não deu para
    /// ler depois de gravar, e a próxima gravação recusa como "mudou".
    identidade: Option<Identidade>,
    geracoes: Geracoes,
}

struct Estado {
    aberto: Option<Aberto>,
    /// Identidade do cofre ao travar, para conferir ao reabrir (D39).
    identidade_conhecida: Option<Identidade>,
    erros_pin: u32,
}

impl Estado {
    fn travar(&mut self) {
        if let Some(a) = self.aberto.take() {
            self.identidade_conhecida = a.identidade;
            // `CofreAberto` e `Entrada` zeram ao sair (chave de dados, hash, segredos).
            drop(a);
        }
        self.erros_pin = 0;
    }

    /// Trava por causa de uma troca do arquivo: esquece a identidade, para o destravar
    /// seguinte decidir só pela geração.
    fn travar_mudou(&mut self) {
        self.travar();
        self.identidade_conhecida = None;
    }
}

/// O daemon: estado travado/destravado e as operações.
pub struct Daemon {
    caminhos: Caminhos,
    estado: Mutex<Estado>,
    /// Senhas erradas seguidas no destravar (D20). Travado durante a tentativa inteira:
    /// as tentativas são uma de cada vez, de qualquer conexão.
    falhas_senha: Mutex<u32>,
    relogio: Box<dyn Relogio>,
}

enum FalhaGravar {
    Resposta(ErroResposta),
    /// O arquivo mudou fora do Ward: travar.
    Mudou,
}

fn problema(campo: &str, motivo: &str) -> ErroResposta {
    ErroResposta::Validacao(vec![Problema {
        campo: campo.to_owned(),
        posicao: None,
        motivo: motivo.to_owned(),
    }])
}

fn senha_de(s: &Segredo) -> SenhaMestra {
    SenhaMestra::de_bytes(Zeroizing::new(s.expor().to_vec()))
}

/// Confere o cofre e grava as entradas e a configuração do `a` (D39, D53).
fn gravar(caminhos: &Caminhos, a: &mut Aberto) -> Result<(), FalhaGravar> {
    match arquivos::identidade(&caminhos.cofre) {
        Ok(Some(id)) if Some(id) == a.identidade => {}
        _ => return Err(FalhaGravar::Mudou),
    }
    // O conjunto reabriria? (nome repetido, túnel/salto quebrado por renomear ou
    // remover um servidor usado). Sem tocar no disco nem queimar geração.
    if let Err(e) = corpo::serializar(a.cofre.configuracao(), 1, &a.entradas) {
        return Err(FalhaGravar::Resposta(match e {
            ErroCorpo::GrandeDemais => problema("cofre", "o cofre passaria do limite de tamanho"),
            _ => problema(
                "nome",
                "outra entrada usa esta como túnel SSH ou servidor de salto",
            ),
        }));
    }
    let proxima = a
        .cofre
        .geracao()
        .checked_add(1)
        .ok_or(FalhaGravar::Resposta(ErroResposta::GeracaoEsgotada))?;
    let antes = Geracoes {
        vista: a.geracoes.vista,
        queimada: a.geracoes.queimada.max(proxima),
    };
    // D53: `queimada` vai para o disco antes do cofre; sem isso, o cofre não é gravado.
    if arquivos::gravar_estado(&caminhos.estado, antes).is_err() {
        aviso!("gravação recusada: não deu para gravar o arquivo de estado");
        return Err(FalhaGravar::Resposta(ErroResposta::FalhaAoGravar));
    }
    a.geracoes = antes;
    match gravacao::gravar_entradas(&caminhos.cofre, &mut a.cofre, &a.entradas) {
        Ok(()) => {
            confirmar(caminhos, a);
            Ok(())
        }
        Err(e) => {
            if matches!(
                e,
                ErroGravacao::Io {
                    etapa: Etapa::FsyncDiretorio,
                    ..
                }
            ) {
                // O rename aconteceu: o arquivo no lugar é o que o Ward acabou de gravar.
                a.identidade = arquivos::identidade(&caminhos.cofre).ok().flatten();
            }
            aviso!("falha ao gravar o cofre: {}", codigo_gravacao(&e));
            Err(FalhaGravar::Resposta(ErroResposta::from(&e)))
        }
    }
}

/// Depois de uma gravação que deu certo: `vista` = geração gravada e identidade nova.
fn confirmar(caminhos: &Caminhos, a: &mut Aberto) {
    let g = a.cofre.geracao();
    a.geracoes = Geracoes {
        vista: g,
        queimada: a.geracoes.queimada.max(g),
    };
    if arquivos::gravar_estado(&caminhos.estado, a.geracoes).is_err() {
        // O cofre está no disco; a próxima gravação regrava o estado antes.
        aviso!("não deu para atualizar o arquivo de estado depois de gravar");
    }
    a.identidade = arquivos::identidade(&caminhos.cofre).ok().flatten();
}

/// Nome fixo do erro de gravação para o log (sem caminho nem valor).
fn codigo_gravacao(e: &ErroGravacao) -> &'static str {
    match e {
        ErroGravacao::GeracaoEsgotada => "geração esgotada",
        ErroGravacao::Retrocesso => "retrocesso",
        ErroGravacao::NaoECofre => "não é cofre",
        ErroGravacao::Formato(_) => "formato",
        ErroGravacao::Corpo(_) => "corpo",
        ErroGravacao::DestinoInvalido => "destino inválido",
        ErroGravacao::Io { etapa, .. } => match etapa {
            Etapa::Ler => "e/s ao ler",
            Etapa::CriarTemporario => "e/s ao criar o temporário",
            Etapa::Escrever => "e/s ao escrever",
            Etapa::FsyncTemporario => "e/s no fsync do temporário",
            Etapa::Renomear => "e/s no rename",
            Etapa::FsyncDiretorio => "e/s no fsync do diretório",
        },
    }
}

impl Daemon {
    pub fn novo(caminhos: Caminhos, relogio: Box<dyn Relogio>) -> Self {
        Self {
            caminhos,
            estado: Mutex::new(Estado {
                aberto: None,
                identidade_conhecida: None,
                erros_pin: 0,
            }),
            falhas_senha: Mutex::new(0),
            relogio,
        }
    }

    /// O estado; se uma thread caiu no meio de uma operação, trava (falha fechada).
    fn estado(&self) -> MutexGuard<'_, Estado> {
        match self.estado.lock() {
            Ok(g) => g,
            Err(envenenado) => {
                let mut g = envenenado.into_inner();
                g.travar();
                self.estado.clear_poison();
                g
            }
        }
    }

    /// O cofre está travado?
    pub fn travado(&self) -> bool {
        self.estado().aberto.is_none()
    }

    /// Trava agora (S8: a chave e as entradas zeram).
    pub fn travar(&self) {
        self.estado().travar();
    }

    /// Atende um pedido já decodificado para o papel da conexão.
    pub fn atender(&self, papel: Papel, pedido: Pedido) -> Resposta {
        let op = pedido.operacao();
        // O protocolo já recusou; aqui é a segunda camada.
        if !op.permitida(papel) {
            return Resposta::Erro(ErroResposta::NaoAutorizado);
        }
        let r = self.atender_permitido(papel, pedido);
        depurar!(
            "{} ({}): {}",
            op.nome(),
            papel.nome(),
            match &r {
                Resposta::Erro(e) => e.codigo(),
                outra => outra.tipo().nome(),
            }
        );
        r
    }

    fn atender_permitido(&self, papel: Papel, pedido: Pedido) -> Resposta {
        let r = match pedido {
            Pedido::Estado => {
                let travado = self.travado();
                Ok(Resposta::Estado {
                    travado,
                    cofre_existe: !travado || arquivos::cofre_existe(&self.caminhos.cofre),
                })
            }
            Pedido::Destravar { senha } => self.destravar(&senha),
            Pedido::Travar => {
                self.travar();
                Ok(Resposta::Ok)
            }
            Pedido::Lista => self.lista(papel),
            Pedido::Executar(x) => self.uso_do_agente(&x.entrada),
            Pedido::Ssh { entrada, .. }
            | Pedido::Login { entrada }
            | Pedido::Playwright { entrada } => self.uso_do_agente(&entrada),
            // Não há sessão aberta nesta fase (Fases 26/27).
            Pedido::EntradaPadrao { .. } | Pedido::Janela(_) => Err(ErroResposta::PedidoInvalido),
            Pedido::CriarCofre { senha, pin } => self.criar_cofre(&senha, &pin),
            Pedido::Detalhe { entrada } => self.detalhe(&entrada),
            Pedido::SalvarEntrada { anterior, rascunho } => {
                self.salvar_entrada(anterior.as_deref(), rascunho)
            }
            Pedido::RemoverEntrada { entrada } => self.remover_entrada(&entrada),
            Pedido::MudarNivel {
                entrada,
                nivel,
                pin,
            } => self.mudar_nivel(&entrada, nivel, &pin),
            Pedido::TrocarPin { atual, novo } => self.trocar_pin(&atual, &novo),
            Pedido::TrocarSenha { atual, nova } => self.trocar_senha(&atual, &nova),
            Pedido::TempoTrava { minutos } => self.tempo_trava(minutos),
            // Das fases seguintes (revelar/copiar/TOTP: 30+; aprovação: 23; token: 27/34;
            // extensão: 40+). Nada é feito e nada sai.
            Pedido::Revelar { .. }
            | Pedido::Copiar { .. }
            | Pedido::Totp { .. }
            | Pedido::ResponderAprovacao { .. }
            | Pedido::EmitirToken { .. }
            | Pedido::Sugerir { .. }
            | Pedido::Preencher { .. }
            | Pedido::SalvarLogin { .. } => Err(ErroResposta::Interno),
        };
        r.unwrap_or_else(Resposta::Erro)
    }

    fn destravar(&self, senha: &Segredo) -> Result<Resposta, ErroResposta> {
        if !self.travado() {
            return Ok(Resposta::Ok);
        }
        let mut falhas = self.falhas_senha.lock().unwrap_or_else(|e| e.into_inner());
        // D20: a espera vem antes de olhar a senha, certa ou errada.
        self.relogio.dormir(espera_destrave(*falhas));
        let mut est = self.estado();
        if est.aberto.is_some() {
            return Ok(Resposta::Ok);
        }
        let (bytes, id) = match arquivos::ler_cofre(&self.caminhos.cofre) {
            Ok(Some(x)) => x,
            Ok(None) => return Err(ErroResposta::CofreInexistente),
            Err(_) => {
                aviso!("destravar recusado: o arquivo do cofre não pôde ser lido");
                return Err(ErroResposta::CofreInvalido);
            }
        };
        let g = match arquivos::ler_estado(&self.caminhos.estado) {
            Ok(g) => g,
            Err(_) => {
                aviso!("destravar recusado: arquivo de estado ausente ou inválido (D39)");
                return Err(ErroResposta::Retrocesso);
            }
        };
        let senha = senha_de(senha);
        let (cofre, entradas, lida) =
            match corpo::abrir_entradas_lida(&bytes, &senha, g.vista, g.queimada) {
                Ok(x) => x,
                Err(e) => {
                    match e {
                        ErroAbrir::Formato(_) => *falhas = falhas.saturating_add(1),
                        ErroAbrir::Retrocesso => {
                            aviso!("destravar recusado: o cofre no disco é mais antigo (D39)")
                        }
                        ErroAbrir::Corpo(_) => aviso!("destravar recusado: corpo inválido"),
                    }
                    return Err(ErroResposta::from(&e));
                }
            };
        drop(senha);
        if let Some(conhecida) = est.identidade_conhecida.take()
            && conhecida != id
        {
            aviso!("destravar recusado: o arquivo do cofre mudou fora do Ward (D39)");
            return Err(ErroResposta::CofreMudou);
        }
        let novo = Geracoes {
            vista: g.vista.max(lida),
            queimada: g.queimada.max(lida),
        };
        if novo != g && arquivos::gravar_estado(&self.caminhos.estado, novo).is_err() {
            aviso!("destravar recusado: não deu para atualizar o arquivo de estado");
            return Err(ErroResposta::FalhaAoGravar);
        }
        match gravacao::limpar_temporarios(&self.caminhos.cofre) {
            Ok(0) => {}
            Ok(n) => aviso!("{n} temporário(s) de gravação interrompida apagado(s)"),
            Err(_) => aviso!("não deu para limpar os temporários do cofre"),
        }
        *falhas = 0;
        est.erros_pin = 0;
        est.aberto = Some(Aberto {
            cofre,
            entradas,
            identidade: Some(id),
            geracoes: novo,
        });
        Ok(Resposta::Ok)
    }

    fn lista(&self, papel: Papel) -> Result<Resposta, ErroResposta> {
        let est = self.estado();
        let a = est.aberto.as_ref().ok_or(ErroResposta::Travado)?;
        let itens = a
            .entradas
            .iter()
            .filter(|e| papel == Papel::App || e.nivel() != NivelAcesso::Bloqueado)
            .map(|e| ItemLista::de_entrada(e, papel))
            .collect();
        Ok(Resposta::Lista(itens))
    }

    /// `run`/`ssh`/`login`/`playwright` do agente. A bloqueada responde igual à que não
    /// existe (V3.17). A execução chega nas Fases 23 e 26: até lá, "com aprovação" é
    /// negado como aprovação indisponível (D21) e "livre" responde erro interno.
    fn uso_do_agente(&self, nome: &str) -> Result<Resposta, ErroResposta> {
        let est = self.estado();
        let a = est.aberto.as_ref().ok_or(ErroResposta::Travado)?;
        let nivel = a
            .entradas
            .iter()
            .find(|e| e.nome() == nome)
            .map(Entrada::nivel)
            .filter(|n| *n != NivelAcesso::Bloqueado)
            .ok_or(ErroResposta::EntradaInexistente)?;
        Err(match nivel {
            NivelAcesso::ComAprovacao => ErroResposta::AprovacaoIndisponivel,
            _ => ErroResposta::Interno,
        })
    }

    fn criar_cofre(&self, senha: &Segredo, pin: &Segredo) -> Result<Resposta, ErroResposta> {
        let mut est = self.estado();
        if est.aberto.is_some() || arquivos::cofre_existe(&self.caminhos.cofre) {
            return Err(ErroResposta::CofreJaExiste);
        }
        let hash = HashPin::criar(pin.expor()).map_err(|_| ErroResposta::PedidoInvalido)?;
        let mut cofre = CofreAberto::novo(&senha_de(senha), Configuracao::nova(hash))
            .map_err(|_| ErroResposta::Interno)?;
        if let Some(dir) = self.caminhos.cofre.parent() {
            use std::os::unix::fs::DirBuilderExt;
            let _ = std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir);
        }
        // Primeiro uso: o estado nasce aqui (D39), com a geração 1 queimada antes.
        let antes = Geracoes {
            vista: 0,
            queimada: 1,
        };
        if arquivos::gravar_estado(&self.caminhos.estado, antes).is_err() {
            aviso!("criar cofre: não deu para gravar o arquivo de estado");
            return Err(ErroResposta::FalhaAoGravar);
        }
        if let Err(e) = gravacao::gravar_entradas(&self.caminhos.cofre, &mut cofre, &[]) {
            aviso!("criar cofre: falha ao gravar ({})", codigo_gravacao(&e));
            return Err(ErroResposta::FalhaAoGravar);
        }
        let mut a = Aberto {
            cofre,
            entradas: Vec::new(),
            identidade: None,
            geracoes: antes,
        };
        confirmar(&self.caminhos, &mut a);
        est.identidade_conhecida = None;
        est.erros_pin = 0;
        est.aberto = Some(a);
        Ok(Resposta::Ok)
    }

    fn detalhe(&self, nome: &str) -> Result<Resposta, ErroResposta> {
        let est = self.estado();
        let a = est.aberto.as_ref().ok_or(ErroResposta::Travado)?;
        let e = a
            .entradas
            .iter()
            .find(|e| e.nome() == nome)
            .ok_or(ErroResposta::EntradaInexistente)?;
        Ok(Resposta::Detalhe(DetalheEntrada::de_entrada(e)))
    }

    /// Grava; em erro desfaz a mudança em memória com `desfazer`. Arquivo trocado: trava.
    fn gravar_ou_desfazer(
        &self,
        est: &mut Estado,
        desfazer: impl FnOnce(&mut Aberto),
    ) -> Result<Resposta, ErroResposta> {
        let Some(a) = est.aberto.as_mut() else {
            return Err(ErroResposta::Travado);
        };
        match gravar(&self.caminhos, a) {
            Ok(()) => Ok(Resposta::Ok),
            Err(FalhaGravar::Resposta(r)) => {
                desfazer(a);
                Err(r)
            }
            Err(FalhaGravar::Mudou) => {
                aviso!("gravação recusada: o arquivo do cofre mudou fora do Ward; travado (D39)");
                est.travar_mudou();
                Err(ErroResposta::CofreMudou)
            }
        }
    }

    fn salvar_entrada(
        &self,
        anterior: Option<&str>,
        rascunho: Rascunho,
    ) -> Result<Resposta, ErroResposta> {
        let mut est = self.estado();
        let a = est.aberto.as_mut().ok_or(ErroResposta::Travado)?;
        let mut nova = rascunho
            .validar()
            .map_err(|e| ErroResposta::de_validacao(&e))?;
        let antiga = match anterior {
            None => {
                // D50: toda entrada nova nasce "com aprovação"; outro nível só pelo
                // `mudar_nivel`, com PIN (S9).
                nova.definir_nivel(NivelAcesso::ComAprovacao);
                nova.conferir_no_cofre(a.entradas.iter())
                    .map_err(|e| ErroResposta::de_validacao(&e))?;
                None
            }
            Some(nome) => {
                let i = a
                    .entradas
                    .iter()
                    .position(|e| e.nome() == nome)
                    .ok_or(ErroResposta::EntradaInexistente)?;
                // Editar não muda o nível.
                nova.definir_nivel(a.entradas[i].nivel());
                nova.conferir_no_cofre(a.entradas.iter().filter(|e| e.nome() != nome))
                    .map_err(|e| ErroResposta::de_validacao(&e))?;
                Some(a.entradas.remove(i))
            }
        };
        let nome_novo = nova.nome().to_owned();
        let pos = a
            .entradas
            .binary_search_by(|e| e.nome().cmp(&nome_novo))
            .unwrap_or_else(|p| p);
        a.entradas.insert(pos, nova);
        self.gravar_ou_desfazer(&mut est, move |a| {
            if let Some(i) = a.entradas.iter().position(|e| e.nome() == nome_novo) {
                drop(a.entradas.remove(i));
            }
            if let Some(velha) = antiga {
                let p = a
                    .entradas
                    .binary_search_by(|e| e.nome().cmp(velha.nome()))
                    .unwrap_or_else(|p| p);
                a.entradas.insert(p, velha);
            }
        })
    }

    fn remover_entrada(&self, nome: &str) -> Result<Resposta, ErroResposta> {
        let mut est = self.estado();
        let a = est.aberto.as_mut().ok_or(ErroResposta::Travado)?;
        let i = a
            .entradas
            .iter()
            .position(|e| e.nome() == nome)
            .ok_or(ErroResposta::EntradaInexistente)?;
        let velha = a.entradas.remove(i);
        self.gravar_ou_desfazer(&mut est, move |a| a.entradas.insert(i, velha))
    }

    /// Confere o PIN (S9): acerto zera a contagem; o 5.º erro seguido trava.
    fn conferir_pin(est: &mut Estado, pin: &Segredo) -> Result<(), ErroResposta> {
        let a = est.aberto.as_ref().ok_or(ErroResposta::Travado)?;
        if a.cofre.configuracao().conferir_pin(pin.expor()) {
            est.erros_pin = 0;
            return Ok(());
        }
        est.erros_pin = est.erros_pin.saturating_add(1);
        if est.erros_pin >= MAX_ERROS_PIN {
            aviso!("{MAX_ERROS_PIN} erros de PIN seguidos: cofre travado (S9)");
            est.travar();
        }
        Err(ErroResposta::PinIncorreto)
    }

    fn mudar_nivel(
        &self,
        nome: &str,
        nivel: NivelAcesso,
        pin: &Segredo,
    ) -> Result<Resposta, ErroResposta> {
        let mut est = self.estado();
        Self::conferir_pin(&mut est, pin)?;
        let a = est.aberto.as_mut().ok_or(ErroResposta::Travado)?;
        let e = a
            .entradas
            .iter_mut()
            .find(|e| e.nome() == nome)
            .ok_or(ErroResposta::EntradaInexistente)?;
        let antes = e.nivel();
        e.definir_nivel(nivel);
        let nome = nome.to_owned();
        self.gravar_ou_desfazer(&mut est, move |a| {
            if let Some(e) = a.entradas.iter_mut().find(|e| e.nome() == nome) {
                e.definir_nivel(antes);
            }
        })
    }

    fn trocar_pin(&self, atual: &Segredo, novo: &Segredo) -> Result<Resposta, ErroResposta> {
        let mut est = self.estado();
        Self::conferir_pin(&mut est, atual)?;
        let hash = HashPin::criar(novo.expor()).map_err(|_| ErroResposta::PedidoInvalido)?;
        let a = est.aberto.as_mut().ok_or(ErroResposta::Travado)?;
        a.cofre.configuracao_mut().trocar_hash_pin(hash);
        match self.gravar_ou_desfazer(&mut est, |_| {}) {
            Ok(r) => Ok(r),
            Err(e) => {
                // O hash antigo já zerou: trava, e o destravar relê o PIN do disco.
                aviso!("troca de PIN não gravada: cofre travado");
                est.travar();
                Err(e)
            }
        }
    }

    fn tempo_trava(&self, minutos: u32) -> Result<Resposta, ErroResposta> {
        let mut est = self.estado();
        let a = est.aberto.as_mut().ok_or(ErroResposta::Travado)?;
        let antes = a.cofre.configuracao().minutos_trava();
        a.cofre
            .configuracao_mut()
            .definir_minutos_trava(minutos)
            .map_err(|_| ErroResposta::PedidoInvalido)?;
        self.gravar_ou_desfazer(&mut est, move |a| {
            let _ = a.cofre.configuracao_mut().definir_minutos_trava(antes);
        })
    }

    /// Troca a senha mestra (D53): `queimada` antes, `trocar_senha` do núcleo com
    /// (`vista`, geração atual), e um erro depois da senha conferida trava o cofre.
    fn trocar_senha(&self, atual: &Segredo, nova: &Segredo) -> Result<Resposta, ErroResposta> {
        let mut est = self.estado();
        let a = est.aberto.as_mut().ok_or(ErroResposta::Travado)?;
        match arquivos::identidade(&self.caminhos.cofre) {
            Ok(Some(id)) if Some(id) == a.identidade => {}
            _ => {
                aviso!("troca de senha recusada: o arquivo do cofre mudou fora do Ward; travado");
                est.travar_mudou();
                return Err(ErroResposta::CofreMudou);
            }
        }
        let geracao = a.cofre.geracao();
        let proxima = geracao
            .checked_add(1)
            .ok_or(ErroResposta::GeracaoEsgotada)?;
        let antes = Geracoes {
            vista: a.geracoes.vista,
            queimada: a.geracoes.queimada.max(proxima),
        };
        if arquivos::gravar_estado(&self.caminhos.estado, antes).is_err() {
            aviso!("troca de senha recusada: não deu para gravar o arquivo de estado");
            return Err(ErroResposta::FalhaAoGravar);
        }
        a.geracoes = antes;
        let r = gravacao::trocar_senha(
            &self.caminhos.cofre,
            &senha_de(atual),
            &senha_de(nova),
            a.geracoes.vista,
            geracao,
        );
        match r {
            Ok(novo) => {
                // O cofre antigo (chave antiga) zera aqui.
                a.cofre = novo;
                confirmar(&self.caminhos, a);
                Ok(Resposta::Ok)
            }
            // D52: na troca de senha, `Formato` é a senha atual errada; nada foi selado.
            Err(ErroGravacao::Formato(_)) => Err(ErroResposta::SenhaIncorreta),
            Err(e) => {
                // D53: o cofre guardado não passou pela queima da troca; trava, e o
                // destravar reabre com a `queimada` do estado.
                aviso!(
                    "troca de senha falhou ({}): cofre travado",
                    codigo_gravacao(&e)
                );
                est.travar_mudou();
                Err(ErroResposta::from(&e))
            }
        }
    }
}
