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
//! - Nenhuma resposta ao agente tem nível nem resumo (D50, D65 c), e uma entrada
//!   bloqueada responde igual a uma inexistente (V3.17).
//! - **Revelar, copiar e TOTP do app** (D4, D65 b): só o papel app, com o cofre
//!   destravado, sem PIN, sem janela de aprovação e sem olhar o nível (a entrada
//!   bloqueada também). O log registra `revelar <entrada> <campo>` (ou `copiar`,
//!   `totp`) sem valor (S13); o valor vai só na resposta, num `Segredo` que zera ao
//!   sair.
//! - Uso do agente e aprovação (Fase 23): em [`crate::uso`], com o motor de
//!   [`crate::aprovacao`].
//! - **Backup local** (Fase 45, D17): cada gravação do cofre que deu certo grava a mesma
//!   versão selada em `<pasta do cofre>/backups/`, aberta pelo fd com `O_NOFOLLOW`
//!   ([`arquivos::abrir_pasta_backup`]); 30 versões. Se a pasta não abre ou não é
//!   segura, o cofre é gravado assim mesmo e o log avisa. **Restaurar** é só do app,
//!   com o cofre destravado, o PIN atual (S9) e a senha mestra da versão: o cofre atual
//!   vira backup antes, e a versão é selada de novo com geração = maior queimada + 1
//!   (D39), com o estado atualizado antes e depois, como numa gravação comum.
//! - **Backup externo** (Fase 46, D17, D30): ligado/desligado e o UUID ficam na
//!   configuração do cofre (`backup_externo`/`definir_backup_externo`, só app; desligado
//!   por padrão). Com o cofre destravado e o externo ligado,
//!   [`Daemon::sincronizar_externo`] copia as versões do backup local para o pendrive
//!   do UUID ([`crate::externo`]); a thread do [`crate::udisks`] chama a cada pendrive
//!   conectado, e o daemon a cutuca depois de destravar e de gravar.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, SystemTime};

use zeroize::Zeroizing;

use ward_core::backup::{self, ErroBackup, Versao};
use ward_core::configuracao::{BackupExterno, Configuracao, HashPin};
use ward_core::corpo::{self, ErroAbrir, ErroCorpo};
use ward_core::entrada::{Dados, Datas, Entrada, NivelAcesso, Rascunho, RascunhoDados, Segredo};
use ward_core::formato::{CofreAberto, SenhaMestra};
use ward_core::gravacao::{self, ErroGravacao, Etapa};
use ward_core::origem::Origem;
use ward_core::protocolo::{
    Alvo, DetalheEntrada, ErroResposta, ItemLista, MAX_BACKUPS, MAX_PENDRIVES, Operacao, Papel,
    Pedido, PedidoDeAprovacao, Pendrive, Problema, Resposta, Sugestao, VersaoBackup,
};
use ward_core::totp::Totp;

use crate::aprovacao::{Cliente, Motor, SemCliente};
use crate::arquivos::{self, ArquivoEstado, Geracoes, Identidade};
use crate::externo::{self, Copia, ErroExterno, FontePendrives};
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

    /// Segundos desde 1970 pelo relógio de parede, para o código TOTP (D65 b). `None`
    /// (relógio antes de 1970): o `totp` responde erro, nunca um código.
    fn segundos_unix(&self) -> Option<u64> {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs())
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
    /// Tokens do "Conectar" ainda não usados (D3, Fase 27): somem ao travar, junto com
    /// o `Aberto`.
    pub(crate) tokens: Vec<crate::ssh::TokenConectar>,
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
    /// Quantas vezes o cofre travou (Fase 27): o agente SSH de uma sessão some quando
    /// o número muda (D12).
    travas: Arc<AtomicU64>,
    /// `<rt>` (Fase 44, D6): ao travar, os storage states do `ward playwright` somem.
    rt_playwright: Option<PathBuf>,
}

impl Estado {
    pub(crate) fn travar(&mut self) {
        if let Some(a) = self.aberto.take() {
            self.travas.fetch_add(1, Ordering::AcqRel);
            self.identidade_conhecida = a.identidade;
            // `CofreAberto` e `Entrada` zeram ao sair (chave de dados, hash, segredos).
            drop(a);
            // D6: as sessões do `ward playwright` não sobrevivem à trava.
            if let Some(rt) = &self.rt_playwright {
                let n = crate::playwright::apagar_sessoes(rt);
                if n > 0 {
                    aviso!("{n} sessão(ões) do ward playwright apagada(s) ao travar (D6)");
                }
            }
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
    /// Os pendrives (udisks2) para a operação `pendrives` (Fase 46). `None`: sem
    /// barramento, responde `pendrives_indisponiveis`.
    pendrives: Option<Arc<dyn FontePendrives>>,
    /// Cutuca a thread da cópia externa (Fase 46), se ligada.
    cutucao: OnceLock<SyncSender<()>>,
    /// Uma cópia externa por vez.
    vez_externo: Mutex<()>,
    /// `ssh`, `sshpass` e `sftp` do `ward ssh` (Fase 27): os do sistema, por caminho
    /// absoluto (sem busca no `PATH`).
    programas_ssh: crate::ssh::ProgramasSsh,
    /// O Chrome do `ward playwright` (Fase 44): o instalado, por caminho absoluto.
    chrome: PathBuf,
    /// Pedidos do `ward login` à extensão (Fase 43).
    logins: crate::login::Logins,
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

/// Senha mestra do pedido, em NFC (Fase 39c): destravar, criar, trocar e restaurar
/// passam todos por aqui, então o app e a CLI abrem o mesmo cofre com o mesmo texto
/// composto ou decomposto, sem depender do cliente.
fn senha_de(s: &Segredo) -> SenhaMestra {
    SenhaMestra::digitada(Zeroizing::new(s.expor().to_vec()))
}

/// Texto fixo (sem valor) de senha mestra nova abaixo do mínimo, igual ao do app.
const SENHA_CURTA: &str = "a senha mestra precisa de 12 caracteres ou mais";

/// Senha mestra nova (criar, trocar): o mínimo vale também aqui, não só no app
/// (Fase 39c; um cliente do papel app que pule a tela não cria cofre com senha curta).
fn senha_nova(s: &Segredo, campo: &str) -> Result<SenhaMestra, ErroResposta> {
    let senha = senha_de(s);
    if senha.serve_de_nova() {
        Ok(senha)
    } else {
        aviso!("senha mestra nova recusada: abaixo do mínimo de caracteres");
        Err(problema(campo, SENHA_CURTA))
    }
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
    let r = gravar_com_backup(caminhos, &mut a.cofre, &a.entradas);
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

/// Abre a pasta de backups ao lado do cofre (Fase 45). Falha vai ao log, sem caminho.
fn abrir_pasta(caminhos: &Caminhos) -> Option<arquivos::PastaAberta> {
    let dir = arquivos::dir_backups(&caminhos.cofre)?;
    match arquivos::abrir_pasta_backup(&dir) {
        Ok(p) => Some(p),
        Err(_) => {
            aviso!(
                "backup local: a pasta de backups não abre ou não é um diretório 0700 do usuário"
            );
            None
        }
    }
}

/// Nome fixo do erro do backup para o log (sem caminho nem valor).
fn codigo_backup(e: &ErroBackup) -> &'static str {
    match e {
        ErroBackup::PastaInsegura => "pasta insegura",
        ErroBackup::NaoAutentico => "cofre atual não autêntico",
        ErroBackup::Relogio => "relógio fora do intervalo",
        ErroBackup::NomesEsgotados => "nomes do segundo esgotados",
        ErroBackup::Gravacao(g) => codigo_gravacao(g),
        ErroBackup::Io(_) => "e/s na pasta",
    }
}

fn registrar_backup(r: &Result<Versao, ErroBackup>) {
    match r {
        Ok(_) => depurar!("backup local gravado"),
        Err(e) => aviso!(
            "backup local não gravado ({}); o cofre foi gravado",
            codigo_backup(e)
        ),
    }
}

/// Grava as entradas e, se a pasta de backups abre, o backup da mesma versão (Fase 45).
/// Sem pasta, grava só o cofre: o backup nunca impede a gravação.
fn gravar_com_backup(
    caminhos: &Caminhos,
    cofre: &mut CofreAberto,
    entradas: &[Entrada],
) -> Result<Identidade, ErroGravacao> {
    match abrir_pasta(caminhos) {
        Some(p) => {
            let g = gravacao::gravar_entradas_com_backup(
                &caminhos.cofre,
                cofre,
                entradas,
                &p.pasta(),
                SystemTime::now(),
            )?;
            registrar_backup(&g.backup);
            Ok(g.identidade)
        }
        None => gravacao::gravar_entradas(&caminhos.cofre, cofre, entradas),
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

/// Operações que, dando certo, podem ter algo novo para o pendrive (Fase 46): o cofre
/// destravou, foi gravado ou o externo foi ligado.
fn muda_o_externo(op: Operacao) -> bool {
    matches!(
        op,
        Operacao::Destravar
            | Operacao::CriarCofre
            | Operacao::SalvarEntrada
            | Operacao::RemoverEntrada
            | Operacao::MudarNivel
            | Operacao::ResponderAprovacao
            | Operacao::TrocarPin
            | Operacao::TrocarSenha
            | Operacao::TempoTrava
            | Operacao::RestaurarBackup
            | Operacao::DefinirBackupExterno
            | Operacao::SalvarLogin
    )
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
        // Fase 49a: temporários do estado de uma gravação interrompida (outro processo).
        match arquivos::limpar_temporarios_estado(&caminhos.estado) {
            Ok(0) => {}
            Ok(n) => aviso!("{n} temporário(s) do arquivo de estado apagado(s) na partida"),
            Err(_) => aviso!("não deu para limpar os temporários do arquivo de estado"),
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
                travas: Arc::new(AtomicU64::new(0)),
                rt_playwright: None,
            }),
            vez_agente: Mutex::new(()),
            vez_app: Mutex::new(()),
            partida,
            relogio,
            aprovacao: Motor::sem_app(),
            gancho_gravado: None,
            temporarios: None,
            pendrives: None,
            cutucao: OnceLock::new(),
            vez_externo: Mutex::new(()),
            programas_ssh: crate::ssh::ProgramasSsh::do_sistema(),
            chrome: PathBuf::from(crate::playwright::CHROME),
            logins: crate::login::Logins::default(),
        }
    }

    /// Liga a fonte dos pendrives (o udisks2; uma lista nos testes) da operação
    /// `pendrives` (Fase 46).
    pub fn com_pendrives(mut self, fonte: Arc<dyn FontePendrives>) -> Self {
        self.pendrives = Some(fonte);
        self
    }

    /// Cria o canal dos cutucões da cópia externa e devolve a ponta de leitura (para a
    /// thread de [`crate::udisks::vigiar_copia`]). Só a primeira chamada liga.
    pub fn ligar_copia_externa(&self) -> Option<Receiver<()>> {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        self.cutucao.set(tx).ok().map(|()| rx)
    }

    /// Pede uma cópia externa à thread dela (sem esperar; cutucões juntam).
    pub fn cutucar_externo(&self) {
        if let Some(tx) = self.cutucao.get() {
            match tx.try_send(()) {
                Ok(()) | Err(TrySendError::Full(())) => {}
                Err(TrySendError::Disconnected(())) => {
                    aviso!("backup externo: a thread da cópia parou");
                }
            }
        }
    }

    /// UUID do pendrive se o cofre está destravado e o externo ligado.
    fn uuid_externo(&self) -> Option<String> {
        let est = self.estado();
        let b = est.aberto.as_ref()?.cofre.configuracao().backup();
        if b.ligado() {
            b.uuid().map(str::to_owned)
        } else {
            None
        }
    }

    /// Copia o backup local para o pendrive escolhido (Fase 46, D17). `Ok(None)`: cofre
    /// travado ou externo desligado (nada é feito, nem a consulta ao udisks2). O estado
    /// não fica preso durante a cópia: só o UUID é lido dele.
    pub fn sincronizar_externo(
        &self,
        fonte: &dyn FontePendrives,
    ) -> Result<Option<Copia>, ErroExterno> {
        let _vez = self.vez_externo.lock().unwrap_or_else(|e| e.into_inner());
        let Some(uuid) = self.uuid_externo() else {
            return Ok(None);
        };
        let vistos = fonte.pendrives().ok_or(ErroExterno::SemUdisks)?;
        let visto = externo::escolher(&vistos, &uuid)?.clone();
        let local = abrir_pasta(&self.caminhos).ok_or(ErroExterno::SemBackupLocal)?;
        externo::copiar(&local, &visto, &uuid, fonte).map(Some)
    }

    /// Troca os programas do `ward ssh` (Fase 27). Só os testes do crate chamam (com
    /// programas falsos que registram o argv); o binário usa sempre os do sistema, e
    /// nenhum pedido do socket chega aqui. Fora do release (scrutiny do M3): só existe
    /// com a feature `ganchos-teste`, que liga o `modo-teste` (V3.15, V9.2).
    #[cfg(feature = "ganchos-teste")]
    #[doc(hidden)]
    pub fn com_programas_ssh(mut self, p: crate::ssh::ProgramasSsh) -> Self {
        self.programas_ssh = p;
        self
    }

    /// Troca o Chrome do `ward playwright` (Fase 44). Só os testes do crate chamam (com
    /// um navegador falso que fala o protocolo pelo pipe); o binário usa sempre o
    /// instalado, e nenhum pedido do socket chega aqui. Só com a feature `ganchos-teste`
    /// (V3.15, V9.2).
    #[cfg(feature = "ganchos-teste")]
    #[doc(hidden)]
    pub fn com_chrome(mut self, chrome: PathBuf) -> Self {
        self.chrome = chrome;
        self
    }

    /// Troca os prazos do `ward login` (Fase 43). Só os testes do crate chamam (prazos
    /// curtos); o binário usa sempre os padrões, e nenhum pedido do socket chega aqui.
    /// Só com a feature `ganchos-teste` (V3.15, V9.2).
    #[cfg(feature = "ganchos-teste")]
    #[doc(hidden)]
    pub fn com_prazos_login(mut self, prazos: crate::login::PrazosLogin) -> Self {
        self.logins = crate::login::Logins::novo(prazos);
        self
    }

    pub(crate) fn logins(&self) -> &crate::login::Logins {
        &self.logins
    }

    pub(crate) fn chrome(&self) -> &std::path::Path {
        &self.chrome
    }

    pub(crate) fn programas_ssh(&self) -> &crate::ssh::ProgramasSsh {
        &self.programas_ssh
    }

    /// `known_hosts` próprio do Ward (D13), ao lado do cofre: no release,
    /// `~/.local/share/ward/known_hosts`; no daemon de teste, dentro de `WARD_DEV_DIR`.
    /// Conferido antes de cada uso (auditoria do M4): a pasta é do daemon e 0700, o
    /// arquivo é do daemon, regular (sem link) e sem nenhuma permissão de grupo/outros;
    /// se não existe, nasce vazio e 0600. Frouxo: recusa (o `ssh` não roda).
    pub(crate) fn known_hosts(&self) -> Result<PathBuf, &'static str> {
        let caminho = self
            .caminhos
            .cofre
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.join("known_hosts"))
            .ok_or("known_hosts do Ward sem pasta")?;
        arquivos::preparar_known_hosts(&caminho).map_err(|_| {
            "known_hosts do Ward recusado: a pasta tem de ser 0700 e o arquivo 0600, do usuário, sem link"
        })?;
        Ok(caminho)
    }

    /// A contagem de travas e o valor dela agora, lido com o estado travado (Fase 27,
    /// D12): o agente SSH de uma sessão some quando o número muda.
    pub(crate) fn travas_agora(&self, est: &Estado) -> (Arc<AtomicU64>, u64) {
        (est.travas.clone(), est.travas.load(Ordering::Acquire))
    }

    /// Liga o `<rt>` dos temporários do `ward run` (Fase 26, S2): `<rt>tmp/`.
    pub fn com_temporarios(mut self, rt: PathBuf) -> Self {
        // D6 (Fase 44): o mesmo `<rt>` guarda os storage states, apagados ao travar.
        self.estado
            .get_mut()
            .unwrap_or_else(|e| e.into_inner())
            .rt_playwright = Some(rt.clone());
        self.temporarios = Some(rt);
        self
    }

    /// O `<rt>` dos temporários, se houver.
    pub(crate) fn temporarios(&self) -> Option<&std::path::Path> {
        self.temporarios.as_deref()
    }

    /// Liga o gancho de teste chamado logo depois de cada gravação do cofre (V3.35). O
    /// binário não chama, e nenhum pedido do socket chega aqui. Fora do release
    /// (scrutiny do M3): só existe com a feature `ganchos-teste` (V3.15, V9.2).
    #[cfg(feature = "ganchos-teste")]
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
        if muda_o_externo(op) && !matches!(r, Resposta::Erro(_)) {
            self.cutucar_externo();
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
            // D6 (Fase 44): só o caminho do storage state volta.
            Pedido::Playwright { .. } => self.playwright(cliente, &pedido),
            // Fase 43: o daemon leva o pedido à extensão; só `ok` ou erro voltam.
            Pedido::Login { .. } => self.login(cliente, &pedido),
            Pedido::AguardarLogin => self.aguardar_login(papel),
            Pedido::PreencherLogin { id, origem } => self.preencher_login(papel, &id, &origem),
            Pedido::ResultadoLogin { id, resultado } => self.resultado_login(papel, &id, resultado),
            // Fase 23: nível, destrave e aprovação. O `executar` e o `ssh` são atendidos
            // pela sessão do servidor (Fases 26/27); aqui o uso autorizado responde erro
            // interno.
            Pedido::Executar(_) | Pedido::Ssh { .. } => self
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
            Pedido::SalvarEntrada {
                anterior,
                rascunho,
                manter_anexos,
            } => self.salvar_entrada(anterior.as_deref(), rascunho, &manter_anexos),
            Pedido::RemoverEntrada { entrada } => self.remover_entrada(&entrada),
            Pedido::MudarNivel {
                entrada,
                nivel,
                pin,
            } => self.mudar_nivel(&entrada, nivel, &pin),
            Pedido::TrocarPin { atual, novo } => self.trocar_pin(&atual, &novo),
            Pedido::TrocarSenha { atual, nova } => self.trocar_senha(&atual, &nova),
            Pedido::TempoTrava { minutos } => self.tempo_trava(minutos),
            // D4, D65 b: só o app, sem PIN, aprovação nem nível.
            Pedido::Revelar { entrada, alvo } => {
                self.valor_do_app(papel, Operacao::Revelar, &entrada, &alvo)
            }
            Pedido::Copiar { entrada, alvo } => {
                self.valor_do_app(papel, Operacao::Copiar, &entrada, &alvo)
            }
            Pedido::Totp { entrada } => self.totp_do_app(papel, &entrada),
            Pedido::Backups => self.backups(),
            Pedido::Configuracao => self.configuracao(),
            Pedido::BackupExterno => self.backup_externo(),
            Pedido::DefinirBackupExterno { ligado, uuid } => {
                self.definir_backup_externo(ligado, uuid.as_deref())
            }
            Pedido::Pendrives => self.pendrives(),
            Pedido::RestaurarBackup { versao, senha, pin } => {
                self.restaurar_backup(&versao, &senha, &pin)
            }
            // D3 (Fase 27): token de uso único do "Conectar", só para o app.
            Pedido::EmitirToken { entrada } => self.emitir_token(papel, &entrada),
            // Fase 40 (D9): sugerir e preencher pela origem da aba, só para a extensão.
            Pedido::Sugerir { origem } => self.sugerir(papel, &origem),
            // Fase 44c: numa origem `http`, só depois da confirmação na janela.
            Pedido::Preencher { entrada, origem } => {
                self.preencher_do_popup(cliente, papel, &entrada, &origem)
            }
            // Fase 42: login novo visto pela extensão, sempre "com aprovação" (D50, S9).
            Pedido::SalvarLogin { rascunho } => self.salvar_login(papel, rascunho),
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
            tokens: Vec::new(),
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

    /// `revelar` e `copiar` (D4, D65 b): o valor de um segredo da entrada para o app.
    /// Só o papel app (a tabela do protocolo e o `atender_de` já conferiram; aqui é a
    /// terceira camada), com o cofre destravado, sem PIN, sem aprovação e sem olhar o
    /// nível: a entrada bloqueada também é revelável no app. O log leva a operação, o
    /// nome da entrada (do cofre, já validado) e o campo, nunca o valor (S13).
    fn valor_do_app(
        &self,
        papel: Papel,
        op: Operacao,
        nome: &str,
        alvo: &Alvo,
    ) -> Result<Resposta, ErroResposta> {
        if papel != Papel::App {
            return Err(ErroResposta::NaoAutorizado);
        }
        let est = self.estado();
        let a = est.aberto.as_ref().ok_or(ErroResposta::Travado)?;
        let e = a
            .entradas
            .iter()
            .find(|e| e.nome() == nome)
            .ok_or(ErroResposta::EntradaInexistente)?;
        let valor = alvo.segredo_em(e).ok_or(ErroResposta::CampoInexistente)?;
        // O campo do tipo já passou pela lista fixa do protocolo; o nome do extra vem
        // da entrada e sai com escape (sem quebra de linha nem controle no log).
        match alvo {
            Alvo::Campo(c) => aviso!("{} {} {c}", op.nome(), e.nome()),
            Alvo::Extra(x) => aviso!("{} {} extra {x:?}", op.nome(), e.nome()),
        }
        Ok(Resposta::Segredo(Segredo::de_bytes(valor.expor().to_vec())))
    }

    /// `totp` do app (D65 b): o código atual da semente da entrada `login`, pelo
    /// relógio do daemon, com as mesmas regras do `revelar`. Sem semente: "campo
    /// inexistente". O código não é segredo do cofre (R8), mas vai num `Zeroizing`.
    fn totp_do_app(&self, papel: Papel, nome: &str) -> Result<Resposta, ErroResposta> {
        if papel != Papel::App {
            return Err(ErroResposta::NaoAutorizado);
        }
        let est = self.estado();
        let a = est.aberto.as_ref().ok_or(ErroResposta::Travado)?;
        let e = a
            .entradas
            .iter()
            .find(|e| e.nome() == nome)
            .ok_or(ErroResposta::EntradaInexistente)?;
        let semente = Alvo::Campo("totp_semente".to_owned())
            .segredo_em(e)
            .ok_or(ErroResposta::CampoInexistente)?;
        let agora = self.relogio.segundos_unix().ok_or(ErroResposta::Interno)?;
        let c = Totp::de_semente(semente)
            .map_err(|_| ErroResposta::Interno)?
            .codigo_em(agora);
        aviso!("totp {}", e.nome());
        Ok(Resposta::CodigoTotp {
            codigo: Zeroizing::new(c.codigo().to_owned()),
            restante: c.restante(),
        })
    }

    /// `sugerir` (Fase 40, D9): as entradas `login` cuja URL tem a mesma origem da aba
    /// (esquema, host e porta, [`Origem`]). Só o papel extensão (terceira camada, como
    /// em [`Daemon::valor_do_app`]). Sem segredo: nome e usuário. O nível não conta
    /// (ele regula o agente; quem usa a extensão é o usuário), então a bloqueada também
    /// aparece. Origem que o parser estrito recusa não casa com nada: lista vazia. A
    /// origem vem da extensão (não confiável) e nunca vai para o log.
    fn sugerir(&self, papel: Papel, origem: &str) -> Result<Resposta, ErroResposta> {
        if papel != Papel::Extensao {
            return Err(ErroResposta::NaoAutorizado);
        }
        let est = self.estado();
        let a = est.aberto.as_ref().ok_or(ErroResposta::Travado)?;
        let Some(aba) = Origem::da_url(origem) else {
            return Ok(Resposta::Sugestoes(Vec::new()));
        };
        let mut itens: Vec<Sugestao> = a
            .entradas
            .iter()
            .filter_map(|e| match e.dados() {
                Dados::Login(l) if Origem::da_url(&l.url).as_ref() == Some(&aba) => {
                    Some(Sugestao {
                        nome: e.nome().to_owned(),
                        usuario: l.usuario.clone(),
                        ambiente: e.ambiente(),
                        totp: l.totp_semente.is_some(),
                    })
                }
                _ => None,
            })
            .collect();
        itens.sort_by(|x, y| x.nome.cmp(&y.nome));
        depurar!("sugerir: {} entrada(s)", itens.len());
        Ok(Resposta::Sugestoes(itens))
    }

    /// `preencher` do popup da extensão (Fase 44c, D9): numa entrada cuja URL é `http`
    /// (`localhost`/`127.0.0.1`), os valores só saem depois de o usuário confirmar a
    /// origem na janela do app (sem PIN: quem clicou no popup é o usuário, e o nível não
    /// conta para a extensão). Sem app, "aprovação indisponível". Origem diferente
    /// responde antes de perguntar nada. Em `https`, o [`Daemon::preencher`] direto.
    fn preencher_do_popup(
        &self,
        cliente: &dyn Cliente,
        papel: Papel,
        nome: &str,
        origem: &str,
    ) -> Result<Resposta, ErroResposta> {
        if papel != Papel::Extensao {
            return Err(ErroResposta::NaoAutorizado);
        }
        let pedido = {
            let est = self.estado();
            let a = est.aberto.as_ref().ok_or(ErroResposta::Travado)?;
            let e = a
                .entradas
                .iter()
                .find(|e| e.nome() == nome)
                .ok_or(ErroResposta::EntradaInexistente)?;
            let Dados::Login(l) = e.dados() else {
                return Err(ErroResposta::EntradaInexistente);
            };
            let casa = match (Origem::da_url(&l.url), Origem::da_url(origem)) {
                (Some(x), Some(y)) => x == y,
                _ => false,
            };
            if !casa {
                aviso!("preencher {} recusado: origem diferente (D9)", e.nome());
                return Err(ErroResposta::OrigemDiferente);
            }
            crate::uso::origem_http(e, Operacao::Preencher).map(|o| PedidoDeAprovacao {
                id: 0,
                entradas: vec![e.nome().to_owned()],
                ambiente: e.ambiente(),
                operacao: Operacao::Preencher,
                comando: String::new(),
                diretorio: String::new(),
                cadeia: crate::uso::cadeia_para_exibir(cliente),
                origem: o,
                confirmacao: true,
            })
        };
        if let Some(p) = pedido {
            aviso!("preencher {nome}: origem http, confirmação na janela (D9)");
            self.aprovacao.pedir_aprovacao(p, cliente)?;
        }
        // A entrada pode ter mudado durante a espera: o `preencher` confere tudo de novo.
        self.preencher(papel, nome, origem)
    }

    /// `preencher` (Fase 40, D9): usuário, senha e o código TOTP atual da entrada
    /// `login`, só se a origem da aba for a da URL da entrada. Só o papel extensão. A
    /// entrada que não existe e a que não é `login` respondem igual; a origem diferente
    /// responde `origem_diferente` sem nenhum valor. O log leva o nome da entrada (do
    /// cofre), nunca a origem nem os valores (S13).
    pub(crate) fn preencher(
        &self,
        papel: Papel,
        nome: &str,
        origem: &str,
    ) -> Result<Resposta, ErroResposta> {
        if papel != Papel::Extensao {
            return Err(ErroResposta::NaoAutorizado);
        }
        let est = self.estado();
        let a = est.aberto.as_ref().ok_or(ErroResposta::Travado)?;
        let e = a
            .entradas
            .iter()
            .find(|e| e.nome() == nome)
            .ok_or(ErroResposta::EntradaInexistente)?;
        let Dados::Login(l) = e.dados() else {
            return Err(ErroResposta::EntradaInexistente);
        };
        let casa = match (Origem::da_url(&l.url), Origem::da_url(origem)) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        };
        if !casa {
            aviso!("preencher {} recusado: origem diferente (D9)", e.nome());
            return Err(ErroResposta::OrigemDiferente);
        }
        let totp = match &l.totp_semente {
            Some(semente) => {
                let agora = self.relogio.segundos_unix().ok_or(ErroResposta::Interno)?;
                let c = Totp::de_semente(semente)
                    .map_err(|_| ErroResposta::Interno)?
                    .codigo_em(agora);
                Some(Zeroizing::new(c.codigo().to_owned()))
            }
            None => None,
        };
        aviso!("preencher {}", e.nome());
        Ok(Resposta::Preenchimento {
            usuario: l.usuario.clone(),
            senha: Segredo::de_bytes(l.senha.expor().to_vec()),
            totp,
        })
    }

    fn criar_cofre(&self, senha: &Segredo, pin: &Segredo) -> Result<Resposta, ErroResposta> {
        let senha = senha_nova(senha, "senha")?;
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
        let mut cofre = CofreAberto::novo(&senha, Configuracao::nova(hash))
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
        let r = gravar_com_backup(&self.caminhos, &mut cofre, &[]);
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
            tokens: Vec::new(),
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

    /// Segundos UTC do relógio do daemon para carimbar uma entrada (D65 d). Relógio
    /// ilegível ou fora da faixa de [`Datas`]: `None`, e a gravação segue sem carimbo
    /// novo (as datas são só informação; nenhuma decisão de segurança usa).
    fn agora_para_datas(&self) -> Option<u64> {
        let agora = self.relogio.segundos_unix().and_then(Datas::instante);
        if agora.is_none() {
            aviso!("relógio de parede ilegível: a entrada é gravada sem data nova (D65 d)");
        }
        agora
    }

    /// As datas da entrada depois de uma alteração em `agora` (as mesmas sem relógio).
    fn alterado(datas: Datas, agora: Option<u64>) -> Datas {
        agora.and_then(|a| datas.alterado(a)).unwrap_or(datas)
    }

    fn salvar_entrada(
        &self,
        anterior: Option<&str>,
        mut rascunho: Rascunho,
        manter_anexos: &[String],
    ) -> Result<Resposta, ErroResposta> {
        let agora = self.agora_para_datas();
        let mut est = self.estado();
        let a = est.aberto.as_mut().ok_or(ErroResposta::Travado)?;
        // Fase 39b (V5.12): os anexos mantidos são copiados da entrada editada aqui
        // dentro, antes da validação inteira (soma de D29, nomes, orçamento da máscara).
        if !manter_anexos.is_empty() {
            let nome = anterior.ok_or(ErroResposta::PedidoInvalido)?;
            let antiga = a
                .entradas
                .iter()
                .find(|e| e.nome() == nome)
                .ok_or(ErroResposta::EntradaInexistente)?;
            ward_core::protocolo::juntar_anexos_mantidos(&mut rascunho, antiga, manter_anexos)?;
        }
        let mut nova = rascunho
            .validar()
            .map_err(|e| ErroResposta::de_validacao(&e))?;
        let antiga = match anterior {
            None => {
                // D50: toda entrada nova nasce "com aprovação"; outro nível só pelo
                // `mudar_nivel`, com PIN (S9).
                nova.definir_nivel(NivelAcesso::ComAprovacao);
                // D65 d: a entrada nova ganha as duas datas, do relógio do daemon.
                nova.definir_datas(
                    agora
                        .and_then(Datas::carimbo_novo)
                        .unwrap_or(Datas::NENHUMA),
                );
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
                // D65 d: a criação fica a da entrada editada; só a alteração muda.
                nova.definir_datas(Self::alterado(a.entradas[i].datas(), agora));
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

    /// `salvar_login` (Fase 42, V6.8): o login novo que a extensão viu ser enviado,
    /// com nome, grupo e ambiente do popup. Só o papel extensão; só entrada nova (nunca
    /// substitui uma existente: o nome repetido é erro de validação) e só os campos do
    /// formulário do popup — URL, usuário e senha. TOTP, extras, anexos, projeto e tags
    /// são recusados sem gravar nada (o app é o lugar de completar a entrada). O nível é
    /// sempre "com aprovação" (D50, S9), como no `salvar_entrada` do app. O log não leva
    /// o nome nem a URL (vieram da página).
    fn salvar_login(&self, papel: Papel, rascunho: Rascunho) -> Result<Resposta, ErroResposta> {
        if papel != Papel::Extensao {
            return Err(ErroResposta::NaoAutorizado);
        }
        let RascunhoDados::Login { totp_semente, .. } = &rascunho.dados else {
            return Err(ErroResposta::PedidoInvalido);
        };
        if totp_semente.is_some()
            || !rascunho.extras.is_empty()
            || !rascunho.anexos.is_empty()
            || !rascunho.projeto.is_empty()
            || !rascunho.tags.is_empty()
        {
            return Err(ErroResposta::PedidoInvalido);
        }
        let r = self.salvar_entrada(None, rascunho, &[]);
        if r.is_ok() {
            aviso!("salvar_login: login novo gravado pela extensão (com aprovação)");
        }
        r
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
        let agora = self.agora_para_datas();
        let mut est = self.estado();
        Self::conferir_pin(&mut est, pin)?;
        let a = est.aberto.as_mut().ok_or(ErroResposta::Travado)?;
        let e = a
            .entradas
            .iter_mut()
            .find(|e| e.nome() == nome)
            .ok_or(ErroResposta::EntradaInexistente)?;
        let antes = e.nivel();
        let datas_antes = e.datas();
        // D65 d: mudar o nível é alterar a entrada.
        let datas = Self::alterado(datas_antes, agora);
        e.definir_nivel(nivel);
        e.definir_datas(datas);
        // Voltar atrás é mudar o nível: a liberação de 1 hora acaba (D2).
        a.liberacoes.remove(nome);
        let nome = nome.to_owned();
        self.gravar_ou_desfazer(&mut est, move |a| {
            if let Some(e) = a.entradas.iter_mut().find(|e| e.nome() == nome) {
                e.definir_nivel(antes);
                e.definir_datas(datas_antes);
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
        // Antes de tudo: senha nova curta não queima geração nem toca no disco.
        let nova = senha_nova(nova, "nova")?;
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
        let atual = senha_de(atual);
        let r = match abrir_pasta(&self.caminhos) {
            Some(p) => gravacao::trocar_senha_com_backup(
                &self.caminhos.cofre,
                &atual,
                &nova,
                a.geracoes.vista,
                geracao,
                &p.pasta(),
                SystemTime::now(),
            )
            .map(|(novo, g)| {
                registrar_backup(&g.backup);
                (novo, g.identidade)
            }),
            None => gravacao::trocar_senha(
                &self.caminhos.cofre,
                &atual,
                &nova,
                a.geracoes.vista,
                geracao,
            ),
        };
        drop((atual, nova));
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

    /// Tempo de trava e backup externo ligado ou não (Fase 38), para a tela
    /// Configurações. Só com o cofre destravado e só pelo app; nunca o hash do PIN nem o
    /// UUID do pendrive.
    fn configuracao(&self) -> Result<Resposta, ErroResposta> {
        let est = self.estado();
        let a = est.aberto.as_ref().ok_or(ErroResposta::Travado)?;
        let c = a.cofre.configuracao();
        // Fase 49a: a tela avisa quando a pasta do backup local não abre ou não é segura
        // (o cofre é gravado sem backup). Só um sim/não: nenhum caminho.
        let backup_local = abrir_pasta(&self.caminhos).is_some_and(|p| p.pasta().listar().is_ok());
        Ok(Resposta::Configuracao {
            minutos_trava: c.minutos_trava(),
            backup_externo: c.backup().ligado(),
            backup_local,
        })
    }

    /// Versões do backup local (Fase 45), da mais nova para a mais antiga. Só com o
    /// cofre destravado e só pelo app (a tabela e o [`Daemon::atender_de`] conferem).
    ///
    /// Fase 49a: sem as versões com data depois de agora + margem (a data do nome é de
    /// quem criou o arquivo), e com a geração selada de cada versão da chave do cofre
    /// aberto ([`backup::geracao_selada`], autenticada), que vira as marcas `conferida`
    /// e `fora_de_ordem` ([`backup::fora_de_ordem`]): uma cópia antiga com nome novo sai
    /// marcada. A geração não vai na resposta (D39).
    fn backups(&self) -> Result<Resposta, ErroResposta> {
        let est = self.estado();
        let a = est.aberto.as_ref().ok_or(ErroResposta::Travado)?;
        let pasta = abrir_pasta(&self.caminhos).ok_or(ErroResposta::BackupIndisponivel)?;
        let versoes = pasta.pasta().listar_ate(SystemTime::now()).map_err(|e| {
            aviso!("backup local: não deu para listar ({})", codigo_backup(&e));
            ErroResposta::BackupIndisponivel
        })?;
        let com_geracao: Vec<(Versao, Option<u64>)> = versoes
            .into_iter()
            .take(MAX_BACKUPS)
            .map(|v| {
                let geracao = match arquivos::ler_cofre(&pasta.pasta().caminho(v)) {
                    Ok(Some((b, _))) => backup::geracao_selada(&a.cofre, &b),
                    _ => None,
                };
                (v, geracao)
            })
            .collect();
        let marcas = backup::fora_de_ordem(&com_geracao);
        let lista = com_geracao
            .iter()
            .zip(marcas)
            .map(|((v, g), fora)| VersaoBackup {
                nome: v.nome(),
                conferida: g.is_some(),
                fora_de_ordem: fora,
            })
            .collect();
        Ok(Resposta::Backups(lista))
    }

    /// Configuração do backup externo (Fase 46): só com o cofre destravado.
    fn backup_externo(&self) -> Result<Resposta, ErroResposta> {
        let est = self.estado();
        let a = est.aberto.as_ref().ok_or(ErroResposta::Travado)?;
        let b = a.cofre.configuracao().backup();
        Ok(Resposta::BackupExterno {
            ligado: b.ligado(),
            uuid: b.uuid().map(str::to_owned),
        })
    }

    /// Liga ou desliga o backup externo (Fase 46, D30): grava a configuração no cofre
    /// pela gravação comum; erro desfaz. Desligar não apaga nada do pendrive.
    fn definir_backup_externo(
        &self,
        ligado: bool,
        uuid: Option<&str>,
    ) -> Result<Resposta, ErroResposta> {
        let novo = BackupExterno::novo(ligado, uuid).map_err(|_| ErroResposta::PedidoInvalido)?;
        let mut est = self.estado();
        let a = est.aberto.as_mut().ok_or(ErroResposta::Travado)?;
        let antes = a.cofre.configuracao().backup().clone();
        a.cofre.configuracao_mut().definir_backup(novo);
        let r = self.gravar_ou_desfazer(&mut est, move |a| {
            a.cofre.configuracao_mut().definir_backup(antes);
        });
        if r.is_ok() {
            aviso!(
                "backup externo {}",
                if ligado { "ligado" } else { "desligado" }
            );
        }
        r
    }

    /// Pendrives que o udisks2 vê (Fase 46): só com o cofre destravado. O estado não
    /// fica preso durante a consulta.
    fn pendrives(&self) -> Result<Resposta, ErroResposta> {
        if self.travado() {
            return Err(ErroResposta::Travado);
        }
        let fonte = self
            .pendrives
            .as_ref()
            .ok_or(ErroResposta::PendrivesIndisponiveis)?;
        let vistos = fonte
            .pendrives()
            .ok_or(ErroResposta::PendrivesIndisponiveis)?;
        Ok(Resposta::Pendrives(
            vistos
                .into_iter()
                .take(MAX_PENDRIVES)
                .map(|v| Pendrive {
                    montado: v.montagem.is_some(),
                    uuid: v.uuid,
                    rotulo: v.rotulo,
                })
                .collect(),
        ))
    }

    /// Restaura uma versão do backup local (D17, D39; Fase 45). Exige o cofre
    /// destravado e o PIN atual (S9: a versão traz outros níveis e outro PIN; o erro
    /// conta para os 5 que travam) e a senha mestra da versão. Ordem:
    ///
    /// 1. abre a versão pelo fd da pasta, sem seguir link, com a senha dela e sem
    ///    geração mínima (todo backup é mais antigo);
    /// 2. confere que o arquivo do cofre é o que o Ward gravou por último (identidade) e
    ///    o guarda como backup, autenticado pela chave do cofre aberto; sem isso, não
    ///    troca;
    /// 3. queima no estado a geração nova = maior queimada + 1 e sela a versão com ela
    ///    pela gravação comum (com o backup da versão nova); depois, `vista` = ela.
    ///
    /// Restaurar nunca baixa a geração: a cópia à mão do mesmo backup segue em
    /// `retrocesso` (V7.11). Erro depois de selar trava o cofre, como na troca de senha.
    fn restaurar_backup(
        &self,
        versao: &str,
        senha: &Segredo,
        pin: &Segredo,
    ) -> Result<Resposta, ErroResposta> {
        let versao = Versao::de_nome(versao).ok_or(ErroResposta::PedidoInvalido)?;
        let mut est = self.estado();
        Self::conferir_pin(&mut est, pin)?;
        let pasta = abrir_pasta(&self.caminhos).ok_or(ErroResposta::BackupIndisponivel)?;
        // Fase 49a: a versão tem de estar na lista que o app vê (sem data futura).
        let na_lista = pasta
            .pasta()
            .listar_ate(SystemTime::now())
            .map_err(|_| ErroResposta::BackupIndisponivel)?
            .contains(&versao);
        if !na_lista {
            aviso!(
                "restaurar recusado: a versão não está na lista (inexistente ou com data futura)"
            );
            return Err(ErroResposta::BackupInexistente);
        }
        let bytes = match arquivos::ler_cofre(&pasta.pasta().caminho(versao)) {
            Ok(Some((b, _))) => b,
            Ok(None) => return Err(ErroResposta::BackupInexistente),
            Err(_) => {
                aviso!("restaurar recusado: a versão não é um arquivo comum do usuário");
                return Err(ErroResposta::BackupIndisponivel);
            }
        };
        let (mut restaurado, entradas, _) =
            corpo::abrir_entradas_lida(&bytes, &senha_de(senha), 0, 0).map_err(|e| {
                aviso!("restaurar recusado: a versão não abriu");
                ErroResposta::from(&e)
            })?;
        drop(bytes);
        let Some(a) = est.aberto.as_mut() else {
            return Err(ErroResposta::Travado);
        };
        // O cofre atual tem de ser o que o Ward gravou por último (D39, D62).
        let atual = match arquivos::ler_cofre(&self.caminhos.cofre) {
            Ok(Some((b, id))) if Some(id) == a.identidade => b,
            _ => {
                aviso!("restaurar recusado: o arquivo do cofre mudou fora do Ward; travado (D39)");
                est.travar_mudou();
                return Err(ErroResposta::CofreMudou);
            }
        };
        let agora = SystemTime::now();
        // D17: o atual vira backup antes de trocar; sem isso, não troca.
        if let Err(e) = backup::guardar_atual(&pasta.pasta(), &a.cofre, &atual, agora) {
            aviso!(
                "restaurar recusado: o cofre atual não foi guardado ({})",
                codigo_backup(&e)
            );
            return Err(ErroResposta::BackupIndisponivel);
        }
        drop(atual);
        // Fase 49a: o tempo de trava e o backup externo continuam os de agora (a versão
        // traz os dela; restaurar uma antiga não desliga a cópia para o pendrive sem
        // aviso). PIN, senha e entradas são os da versão (D17).
        {
            let atual = a.cofre.configuracao();
            let minutos = atual.minutos_trava();
            let externo = atual.backup().clone();
            let c = restaurado.configuracao_mut();
            if c.definir_minutos_trava(minutos).is_err() {
                aviso!("restaurar: tempo de trava atual fora da faixa; ficou o da versão");
            }
            c.definir_backup(externo);
        }
        // D39: geração = maior queimada + 1, queimada no estado antes de selar (D53).
        let q = a.geracoes.queimada.max(a.cofre.geracao());
        let proxima = q.checked_add(1).ok_or(ErroResposta::GeracaoEsgotada)?;
        restaurado = restaurado.depois_da_geracao(q);
        let antes = Geracoes {
            vista: a.geracoes.vista,
            queimada: proxima,
        };
        if arquivos::gravar_estado(&self.caminhos.estado, antes).is_err() {
            aviso!("restaurar recusado: não deu para gravar o arquivo de estado");
            return Err(ErroResposta::FalhaAoGravar);
        }
        a.geracoes = antes;
        let r = gravacao::gravar_entradas_com_backup(
            &self.caminhos.cofre,
            &mut restaurado,
            &entradas,
            &pasta.pasta(),
            agora,
        );
        if let Some(g) = &self.gancho_gravado {
            g();
        }
        match r {
            Ok(g) => {
                registrar_backup(&g.backup);
                // O cofre de antes (chave, PIN, segredos) zera aqui.
                a.cofre = restaurado;
                a.entradas = entradas;
                a.liberacoes.clear();
                confirmar(&self.caminhos, a, g.identidade);
                est.erros_pin = 0;
                // Fase 49a: um pedido de aprovação aberto era sobre o cofre de antes;
                // uma resposta que chegasse agora mudaria a entrada de mesmo nome da
                // versão restaurada. Todos são negados, com o estado ainda preso.
                let n = self.aprovacao.cancelar_pendentes();
                if n > 0 {
                    aviso!("{n} pedido(s) de aprovação cancelado(s) pelo restaurar");
                }
                aviso!("backup restaurado; o cofre anterior ficou na pasta de backups");
                Ok(Resposta::Ok)
            }
            Err(e) => {
                if let ErroGravacao::Io {
                    etapa: Etapa::FsyncDiretorio,
                    identidade,
                    ..
                } = e
                {
                    // A versão restaurada está no lugar (o rename aconteceu).
                    a.identidade = identidade;
                }
                aviso!("restaurar falhou ({}): cofre travado", codigo_gravacao(&e));
                // A geração queimada está no estado; o destravar reabre com ela (D53).
                est.travar();
                Err(ErroResposta::from(&e))
            }
        }
    }
}
