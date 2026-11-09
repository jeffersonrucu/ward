//! Estado do daemon e regra de negócio de cada operação (Fase 22).
//!
//! - **Travado / destravado.** Destravado, o daemon guarda um único [`CofreAberto`]
//!   (dono único, sem cópia) e as entradas. Travar solta os dois: a chave de dados, o
//!   hash do PIN e os segredos zeram ao sair (S8, V3.8).
//! - **Destravar** (D20, revista pela D62): a tentativa espera 0 s, e cada senha errada
//!   seguida dobra a espera da próxima a partir de 1 s, até 30 s. São dois contadores,
//!   cada um para todas as conexões do papel: o do agente (inclui o `ward destravar`) e
//!   o do app. Erro do agente não atrasa o app. O do agente vai para o arquivo de
//!   estado, com fsync antes da resposta de erro, e volta ao subir: matar o `wardd` não
//!   zera a espera (a primeira tentativa depois da partida espera o que faltar, contado
//!   da partida). O do app fica em memória. O destravar certo de qualquer papel zera os
//!   dois. A resposta não diz quantas tentativas houve.
//! - **PIN** (S9): confere pelo hash da configuração do cofre (`HashPin`, em tempo
//!   constante, D30); 5 erros seguidos travam o cofre, mesmo reconectando.
//! - **Gerações** (D39, D53): o arquivo de estado guarda `vista` e `queimada`. Abre com
//!   (`vista`, `queimada`); antes de cada gravação grava `queimada` = geração + 1 e só
//!   então grava o cofre; depois da gravação que deu certo, `vista` = geração. Sem
//!   estado com o cofre existente, não destrava.
//! - **Identidade** (D39, D62): `dev`/inode/`mtime`/tamanho do cofre por `fstat` do fd
//!   lido e, depois de gravar, do fd do temporário (devolvida pelo `ward_core`), nunca
//!   por um `stat` do caminho depois do rename; antes de gravar (e ao reabrir) confere.
//!   Mudou: não grava, trava e responde [`ErroResposta::CofreMudou`].
//! - **Criar com estado velho** (D62): sem cofre e com um estado de `queimada` q, o
//!   cofre novo nasce na geração q+1, e uma cópia de um cofre anterior fica abaixo da
//!   `vista`.
//! - Nenhuma resposta ao agente tem nível (D50), e uma entrada bloqueada responde igual
//!   a uma inexistente (V3.17).
//! - Uso do agente e aprovação (Fase 23): em [`crate::uso`], com o motor de
//!   [`crate::aprovacao`].

use std::collections::HashMap;
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

use crate::aprovacao::{Cliente, Motor, SemCliente};
use crate::arquivos::{self, ArquivoEstado, Geracoes, Identidade};
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

    /// Tempo monotônico que conta a suspensão (`CLOCK_BOOTTIME`), para a trava por
    /// inatividade (Fase 24). `None` (não deu para ler) trava: falha fechada.
    fn agora(&self) -> Option<Duration> {
        ward_plataforma::tempo_desde_boot().ok()
    }
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
pub(crate) struct Aberto {
    cofre: CofreAberto,
    /// Em ordem de nome.
    pub(crate) entradas: Vec<Entrada>,
    /// "Permitir por 1 hora" (D2): entrada → até quando, no relógio do motor. Some ao
    /// travar (vai junto com o `Aberto`), ao mudar o nível e ao remover ou renomear.
    pub(crate) liberacoes: HashMap<String, Duration>,
    /// Identidade do arquivo depois da última leitura ou gravação. `None`: não deu para
    /// ler depois de gravar, e a próxima gravação recusa como "mudou".
    identidade: Option<Identidade>,
    geracoes: Geracoes,
}

pub(crate) struct Estado {
    pub(crate) aberto: Option<Aberto>,
    /// Identidade do cofre ao travar, para conferir ao reabrir (D39).
    identidade_conhecida: Option<Identidade>,
    erros_pin: u32,
    /// Último uso com o cofre destravado, no relógio do [`Relogio::agora`] (Fase 24).
    ultimo_uso: Option<Duration>,
    /// Senhas erradas seguidas do papel agente (D20, D62); espelho do arquivo de
    /// estado. Travar não mexe nelas.
    falhas_agente: u32,
    /// Senhas erradas seguidas do papel app (D62), só em memória.
    falhas_app: u32,
    /// A próxima tentativa do agente é a primeira desde a partida: a espera conta da
    /// partida (D62).
    primeira_do_agente: bool,
}

impl Estado {
    pub(crate) fn travar(&mut self) {
        if let Some(a) = self.aberto.take() {
            self.identidade_conhecida = a.identidade;
            // `CofreAberto` e `Entrada` zeram ao sair (chave de dados, hash, segredos).
            drop(a);
        }
        self.erros_pin = 0;
        self.ultimo_uso = None;
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
    /// Uma tentativa de destravar por vez em cada papel (D20, D62), de qualquer
    /// conexão, segurado durante a espera e a tentativa. Um mutex por papel: a espera
    /// do agente não segura o app. Os contadores ficam no [`Estado`].
    vez_agente: Mutex<()>,
    vez_app: Mutex<()>,
    /// Hora da partida no [`Relogio::agora`] (D62).
    partida: Option<Duration>,
    relogio: Box<dyn Relogio>,
    /// Gancho chamado logo depois de cada gravação do cofre, antes de o daemon olhar o
    /// resultado. Só os testes ligam (V3.35: troca do arquivo logo depois do rename); o
    /// binário nunca.
    gancho_gravado: Option<Gancho>,
    /// Motor de aprovação (Fase 23).
    pub(crate) aprovacao: Motor,
    /// `<rt>` (Fase 26): os anexos do `ward run` ficam em `<rt>tmp/` (S2). `None`: o
    /// `ward run` de entrada com arquivo responde `entrega_falhou`.
    temporarios: Option<PathBuf>,
}

/// Gancho de teste de [`Daemon::com_gancho_depois_de_gravar`].
pub type Gancho = Box<dyn Fn() + Send + Sync>;

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
fn gravar(caminhos: &Caminhos, a: &mut Aberto, gancho: Option<&Gancho>) -> Result<(), FalhaGravar> {
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
    let r = gravacao::gravar_entradas(&caminhos.cofre, &mut a.cofre, &a.entradas);
    if let Some(g) = gancho {
        g();
    }
    match r {
        Ok(id) => {
            confirmar(caminhos, a, id);
            Ok(())
        }
        Err(e) => {
            if let ErroGravacao::Io {
                etapa: Etapa::FsyncDiretorio,
                identidade,
                ..
            } = e
            {
                // O rename aconteceu: o arquivo no lugar é o que o Ward acabou de gravar,
                // com a identidade lida do fd do temporário (D62), sem `stat` do caminho.
                a.identidade = identidade;
            }
            aviso!("falha ao gravar o cofre: {}", codigo_gravacao(&e));
            Err(FalhaGravar::Resposta(ErroResposta::from(&e)))
        }
    }
}

/// Depois de uma gravação que deu certo: `vista` = geração gravada e a identidade que a
/// gravação leu do fd do temporário (D62). Nenhum `stat` do caminho depois do rename:
/// um arquivo trocado nesse meio-tempo fica com outra identidade, e a próxima gravação
/// recusa como "mudou".
fn confirmar(caminhos: &Caminhos, a: &mut Aberto, id: Identidade) {
    let g = a.cofre.geracao();
    a.geracoes = Geracoes {
        vista: g,
        queimada: a.geracoes.queimada.max(g),
    };
    if arquivos::gravar_estado(&caminhos.estado, a.geracoes).is_err() {
        // O cofre está no disco; a próxima gravação regrava o estado antes.
        aviso!("não deu para atualizar o arquivo de estado depois de gravar");
    }
    a.identidade = Some(id);
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
    /// Daemon travado. Lê do arquivo de estado as senhas erradas do agente (D62): a
    /// espera de antes do reinício continua valendo.
    pub fn novo(caminhos: Caminhos, relogio: Box<dyn Relogio>) -> Self {
        let falhas_agente = arquivos::ler_estado_completo(&caminhos.estado)
            .map(|e| e.falhas_agente)
            .unwrap_or(0);
        if falhas_agente > 0 {
            aviso!("{falhas_agente} senha(s) errada(s) do agente antes da partida (D20)");
        }
        let partida = relogio.agora();
        Self {
            caminhos,
            estado: Mutex::new(Estado {
                aberto: None,
                identidade_conhecida: None,
                erros_pin: 0,
                ultimo_uso: None,
                falhas_agente,
                falhas_app: 0,
                primeira_do_agente: true,
            }),
            vez_agente: Mutex::new(()),
            vez_app: Mutex::new(()),
            partida,
            relogio,
            aprovacao: Motor::sem_app(),
            gancho_gravado: None,
            temporarios: None,
        }
    }

    /// Liga o `<rt>` dos temporários do `ward run` (Fase 26, S2): `<rt>tmp/`.
    pub fn com_temporarios(mut self, rt: PathBuf) -> Self {
        self.temporarios = Some(rt);
        self
    }

    /// O `<rt>` dos temporários, se houver.
    pub(crate) fn temporarios(&self) -> Option<&std::path::Path> {
        self.temporarios.as_deref()
    }

    /// Liga o gancho de teste chamado logo depois de cada gravação do cofre (V3.35). O
    /// binário não chama, e nenhum pedido do socket chega aqui.
    #[doc(hidden)]
    pub fn com_gancho_depois_de_gravar(mut self, gancho: Gancho) -> Self {
        self.gancho_gravado = Some(gancho);
        self
    }

    /// Troca o motor de aprovação (o do daemon abre o app pelo lançador; os testes
    /// usam um relógio e um app falsos).
    pub fn com_motor(mut self, motor: Motor) -> Self {
        self.aprovacao = motor;
        self
    }

    /// O motor de aprovação (o servidor assina nele a conexão do app).
    pub fn motor(&self) -> &Motor {
        &self.aprovacao
    }

    /// O estado; se uma thread caiu no meio de uma operação, trava (falha fechada).
    pub(crate) fn estado(&self) -> MutexGuard<'_, Estado> {
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

    /// Trava se o cofre ficou sem uso pelo tempo da configuração (Fase 24, D30). Devolve
    /// `true` se travou agora. Relógio ilegível ou que voltou também trava.
    pub fn travar_se_inativo(&self) -> bool {
        let mut est = self.estado();
        let Some(a) = est.aberto.as_ref() else {
            return false;
        };
        // Lido com o estado travado: um uso de outra thread não fica "no futuro".
        let agora = self.relogio.agora();
        let minutos = a.cofre.configuracao().minutos_trava();
        if !crate::trava::venceu(est.ultimo_uso, agora, minutos) {
            return false;
        }
        est.travar();
        aviso!("{minutos} min sem uso: cofre travado");
        true
    }

    /// Anota um uso (Fase 24): só com o cofre destravado.
    pub(crate) fn registrar_uso(&self) {
        let mut est = self.estado();
        if est.aberto.is_some() {
            est.ultimo_uso = self.relogio.agora();
        }
    }

    /// Atende um pedido já decodificado para o papel da conexão.
    pub fn atender(&self, papel: Papel, pedido: Pedido) -> Resposta {
        self.atender_de(&SemCliente, papel, pedido)
    }

    /// Como [`Daemon::atender`], com o cliente da conexão (cadeia de processos para a
    /// janela de aprovação e se ele ainda está lá).
    pub fn atender_de(&self, cliente: &dyn Cliente, papel: Papel, pedido: Pedido) -> Resposta {
        let op = pedido.operacao();
        // O protocolo já recusou; aqui é a segunda camada.
        if !op.permitida(papel) {
            return Resposta::Erro(ErroResposta::NaoAutorizado);
        }
        let r = self.atender_permitido(cliente, papel, pedido);
        if crate::trava::conta_como_uso(op, &r) {
            self.registrar_uso();
        }
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

    fn atender_permitido(&self, cliente: &dyn Cliente, papel: Papel, pedido: Pedido) -> Resposta {
        let r = match pedido {
            Pedido::Estado => {
                let travado = self.travado();
                Ok(Resposta::Estado {
                    travado,
                    cofre_existe: !travado || arquivos::cofre_existe(&self.caminhos.cofre),
                    // D63: o papel desta conexão, sem nenhum outro dado.
                    papel,
                })
            }
            Pedido::Destravar { senha } => self.destravar(papel, &senha),
            Pedido::Travar => {
                self.travar();
                Ok(Resposta::Ok)
            }
            Pedido::Lista => self.lista(papel),
            // Fase 23: nível, destrave e aprovação. A execução chega nas Fases 26/27;
            // até lá, o uso autorizado responde erro interno.
            Pedido::Executar(_)
            | Pedido::Ssh { .. }
            | Pedido::Login { .. }
            | Pedido::Playwright { .. } => self
                .autorizar_uso(cliente, &pedido)
                .and(Err(ErroResposta::Interno)),
            Pedido::ResponderAprovacao {
                pedido,
                decisao,
                pin,
            } => self.responder_aprovacao(pedido, decisao, pin.as_ref()),
            Pedido::AbrirApp => self.aprovacao.abrir_app().map(|()| Resposta::Ok),
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
            // Das fases seguintes (revelar/copiar/TOTP: 30+; token: 27/34;
            // extensão: 40+). Nada é feito e nada sai.
            Pedido::Revelar { .. }
            | Pedido::Copiar { .. }
            | Pedido::Totp { .. }
            | Pedido::EmitirToken { .. }
            | Pedido::Sugerir { .. }
            | Pedido::Preencher { .. }
            | Pedido::SalvarLogin { .. } => Err(ErroResposta::Interno),
        };
        r.unwrap_or_else(Resposta::Erro)
    }

    /// Espera da próxima tentativa do papel (D20, D62). A primeira do agente depois da
    /// partida espera o que faltar da espera das falhas lidas do estado, contado da
    /// partida (relógio ilegível: a espera inteira).
    fn espera_de(&self, est: &mut Estado, agente: bool) -> Duration {
        if !agente {
            return espera_destrave(est.falhas_app);
        }
        let espera = espera_destrave(est.falhas_agente);
        if !std::mem::take(&mut est.primeira_do_agente) {
            return espera;
        }
        match (self.partida, self.relogio.agora()) {
            (Some(p), Some(agora)) if agora >= p => espera.saturating_sub(agora - p),
            _ => espera,
        }
    }

    fn destravar(&self, papel: Papel, senha: &Segredo) -> Result<Resposta, ErroResposta> {
        if !self.travado() {
            return Ok(Resposta::Ok);
        }
        // D62: o papel extensão não destrava (o protocolo recusa); qualquer outro que
        // não seja o app conta como agente.
        let agente = papel != Papel::App;
        let vez = if agente {
            &self.vez_agente
        } else {
            &self.vez_app
        };
        let _vez = vez.lock().unwrap_or_else(|e| e.into_inner());
        // D20: a espera vem antes de olhar a senha, certa ou errada.
        let espera = {
            let mut est = self.estado();
            self.espera_de(&mut est, agente)
        };
        self.relogio.dormir(espera);
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
        let (g, falhas_no_disco) = match arquivos::ler_estado_completo(&self.caminhos.estado) {
            Ok(e) => (e.geracoes, e.falhas_agente),
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
                        ErroAbrir::Formato(_) => self.contar_falha(&mut est, agente, g),
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
        // D62: o destravar certo zera as falhas do agente também no disco.
        if (novo != g || est.falhas_agente != 0 || falhas_no_disco != 0)
            && arquivos::gravar_estado(&self.caminhos.estado, novo).is_err()
        {
            aviso!("destravar recusado: não deu para atualizar o arquivo de estado");
            return Err(ErroResposta::FalhaAoGravar);
        }
        match gravacao::limpar_temporarios(&self.caminhos.cofre) {
            Ok(0) => {}
            Ok(n) => aviso!("{n} temporário(s) de gravação interrompida apagado(s)"),
            Err(_) => aviso!("não deu para limpar os temporários do cofre"),
        }
        est.falhas_agente = 0;
        est.falhas_app = 0;
        est.erros_pin = 0;
        est.ultimo_uso = self.relogio.agora();
        est.aberto = Some(Aberto {
            cofre,
            entradas,
            liberacoes: HashMap::new(),
            identidade: Some(id),
            geracoes: novo,
        });
        Ok(Resposta::Ok)
    }

    /// Uma senha errada (D20, D62). A do agente vai para o arquivo de estado, com fsync,
    /// antes de a resposta de erro sair; se não der para gravar, fica em memória e vai
    /// ao log.
    fn contar_falha(&self, est: &mut Estado, agente: bool, g: Geracoes) {
        if !agente {
            est.falhas_app = est.falhas_app.saturating_add(1);
            return;
        }
        est.falhas_agente = est.falhas_agente.saturating_add(1);
        let e = ArquivoEstado {
            geracoes: g,
            falhas_agente: est.falhas_agente,
        };
        if arquivos::gravar_estado_completo(&self.caminhos.estado, e).is_err() {
            aviso!("não deu para gravar a senha errada do agente no arquivo de estado (D62)");
        }
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

    fn criar_cofre(&self, senha: &Segredo, pin: &Segredo) -> Result<Resposta, ErroResposta> {
        let mut est = self.estado();
        if est.aberto.is_some() || arquivos::cofre_existe(&self.caminhos.cofre) {
            return Err(ErroResposta::CofreJaExiste);
        }
        let hash = HashPin::criar(pin.expor()).map_err(|_| ErroResposta::PedidoInvalido)?;
        // D62: um estado velho (cofre apagado) não desce. O cofre novo nasce na geração
        // `queimada` + 1, e uma cópia de um cofre anterior fica abaixo da `vista`.
        let velho = match arquivos::ler_estado(&self.caminhos.estado) {
            Ok(g) => Some(g),
            Err(arquivos::ErroEstado::Ausente) => None,
            Err(arquivos::ErroEstado::Invalido) => {
                aviso!("criar cofre recusado: arquivo de estado inválido (D62)");
                return Err(ErroResposta::FalhaAoGravar);
            }
        };
        let q = velho.map_or(0, |g| g.queimada);
        let primeira = q.checked_add(1).ok_or(ErroResposta::GeracaoEsgotada)?;
        if velho.is_some() {
            aviso!("criar cofre: estado anterior na geração {q}; o cofre novo nasce na {primeira}");
        }
        let mut cofre = CofreAberto::novo(&senha_de(senha), Configuracao::nova(hash))
            .map_err(|_| ErroResposta::Interno)?
            .depois_da_geracao(q);
        if let Some(dir) = self.caminhos.cofre.parent() {
            use std::os::unix::fs::DirBuilderExt;
            let _ = std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir);
        }
        // Primeiro uso: o estado nasce (ou continua) aqui (D39), com a primeira geração
        // queimada antes.
        let antes = Geracoes {
            vista: velho.map_or(0, |g| g.vista),
            queimada: primeira,
        };
        if arquivos::gravar_estado(&self.caminhos.estado, antes).is_err() {
            aviso!("criar cofre: não deu para gravar o arquivo de estado");
            return Err(ErroResposta::FalhaAoGravar);
        }
        let r = gravacao::gravar_entradas(&self.caminhos.cofre, &mut cofre, &[]);
        if let Some(g) = &self.gancho_gravado {
            g();
        }
        let id = match r {
            Ok(id) => id,
            Err(e) => {
                aviso!("criar cofre: falha ao gravar ({})", codigo_gravacao(&e));
                return Err(ErroResposta::FalhaAoGravar);
            }
        };
        let mut a = Aberto {
            cofre,
            entradas: Vec::new(),
            liberacoes: HashMap::new(),
            identidade: None,
            geracoes: antes,
        };
        confirmar(&self.caminhos, &mut a, id);
        est.identidade_conhecida = None;
        est.falhas_agente = 0;
        est.falhas_app = 0;
        est.erros_pin = 0;
        est.ultimo_uso = self.relogio.agora();
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
    pub(crate) fn gravar_ou_desfazer(
        &self,
        est: &mut Estado,
        desfazer: impl FnOnce(&mut Aberto),
    ) -> Result<Resposta, ErroResposta> {
        let Some(a) = est.aberto.as_mut() else {
            return Err(ErroResposta::Travado);
        };
        match gravar(&self.caminhos, a, self.gancho_gravado.as_ref()) {
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
                // Editar não muda o nível. Renomear tira a liberação de 1 hora (D2).
                nova.definir_nivel(a.entradas[i].nivel());
                if nova.nome() != nome {
                    a.liberacoes.remove(nome);
                }
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
        a.liberacoes.remove(nome);
        self.gravar_ou_desfazer(&mut est, move |a| a.entradas.insert(i, velha))
    }

    /// Confere o PIN (S9): acerto zera a contagem; o 5.º erro seguido trava.
    pub(crate) fn conferir_pin(est: &mut Estado, pin: &Segredo) -> Result<(), ErroResposta> {
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
        // Voltar atrás é mudar o nível: a liberação de 1 hora acaba (D2).
        a.liberacoes.remove(nome);
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
        if let Some(g) = &self.gancho_gravado {
            g();
        }
        match r {
            Ok((novo, id)) => {
                // O cofre antigo (chave antiga) zera aqui.
                a.cofre = novo;
                confirmar(&self.caminhos, a, id);
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
