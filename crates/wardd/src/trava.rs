//! Trava automática (Fase 24, R23, D30): sem uso pelo tempo da configuração do cofre
//! (padrão 240 min), bloqueio de tela e suspensão.
//!
//! - **Inatividade.** Cada uso zera a contagem ([`conta_como_uso`]): entrega de
//!   credencial (agente e extensão) e ação do app que deu certo, menos `estado`,
//!   `travar` e `destravar` (o que abre marca o início; com o cofre aberto ele responde
//!   `ok` sem olhar a senha) e, desde a Fase 55, o `detalhe` e o `totp` que a janela de
//!   aprovação e o formulário pedem sozinhos e a resposta da janela. Consulta do agente (`lista`) e da extensão (`sugerir`) não
//!   conta (Fase 54). Erro não conta: um agente que manda pedidos inválidos ou PIN errado
//!   não segura o cofre aberto. O relógio é o [`Relogio::agora`](crate::nucleo::Relogio) do
//!   daemon (`CLOCK_BOOTTIME`, injetado nos testes), que anda durante a suspensão. Um
//!   relógio que não se lê ou que volta trava (falha fechada, [`venceu`]).
//! - **logind**, no barramento do **sistema** (o `wardd` é serviço do sistema, D46, e
//!   não está no barramento da sessão):
//!   - `Lock` de uma sessão (`loginctl lock-session`) e `LockedHint` = verdadeiro (ou
//!     invalidado) no `PropertiesChanged` de uma sessão travam;
//!   - `PrepareForSleep(true)` trava. O daemon segura um inibidor `delay` de `sleep` e só
//!     o solta **depois** de travar, para a suspensão não chegar antes da trava; ao
//!     acordar (`PrepareForSleep(false)`) pega outro.
//!   - Qualquer sessão trava, não só as do uid do daemon: travar demais é seguro, e
//!     conferir o dono de cada sessão seria mais um pedido que pode falhar.
//!   - Nada que chega pelo barramento destrava ou adia a trava: um sinal forjado (de
//!     quem não é o logind) só consegue travar, o que o agente já pode pedir (`travar`).
//!   - Conexão perdida depois de aberta trava (não há mais como ver o bloqueio de tela)
//!     e tenta de novo com espera crescente até [`ESPERA_MAXIMA_BARRAMENTO`]. Sem
//!     barramento desde a partida, só avisa: a inatividade continua valendo.
//! - O endereço do barramento é fixo ([`BARRAMENTO_SISTEMA`]); nunca sai do ambiente.
//!   Só o daemon de teste aceita `WARD_DEV_DBUS_SISTEMA` (caminho de um socket, para o
//!   `dbus-daemon` falso dos testes); sem ela, o daemon de teste não liga o logind.
//!
//! Nada daqui toca em segredo; o log só tem textos fixos.

use std::collections::HashMap;
use std::io;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use std::future::poll_fn;
use std::num::NonZeroUsize;
use std::pin::Pin;
use ward_core::protocol::{Operation, Resposta, Role};

use futures_core::Stream;
use zbus::connection::Builder;
use zbus::message::Type as TipoMensagem;
use zbus::{Connection, MatchRule, Message, MessageStream, OwnedVariant, Variant};

use crate::aviso;
use crate::nucleo::Daemon;

/// Socket do barramento do sistema.
pub const BARRAMENTO_SISTEMA: &str = "/run/dbus/system_bus_socket";
/// De quanto em quanto tempo a inatividade é conferida.
pub const INTERVALO_INATIVIDADE: Duration = Duration::from_secs(10);
/// Maior espera entre tentativas de conectar ao barramento.
pub const ESPERA_MAXIMA_BARRAMENTO: Duration = Duration::from_secs(60);
/// Prazo das chamadas ao logind (o inibidor).
pub const PRAZO_CHAMADA: Duration = Duration::from_secs(5);
/// Mensagens na fila antes de o leitor esperar.
const FILA: NonZeroUsize = NonZeroUsize::new(256).unwrap();

const LOGIND: &str = "org.freedesktop.login1";
const CAMINHO_LOGIND: &str = "/org/freedesktop/login1";
const CAMINHO_SESSOES: &str = "/org/freedesktop/login1/session";
const IFACE_MANAGER: &str = "org.freedesktop.login1.Manager";
const IFACE_SESSAO: &str = "org.freedesktop.login1.Session";
const IFACE_PROPRIEDADES: &str = "org.freedesktop.DBus.Properties";
/// Maior caminho aceito para o socket do barramento (o `sun_path` tem 108 bytes).
const MAX_CAMINHO: usize = 107;

/// Esta resposta a esta operação, deste papel, conta como uso do cofre?
///
/// Fase 54 (auditoria final): só zera a contagem o uso que entrega credencial (o
/// `ward run`, `ssh`, `login` e `playwright` do agente, que anotam o uso eles mesmos
/// depois da autorização; o preencher da extensão) ou uma ação do app. Consulta do
/// agente (`lista` periódica, `abrir_app`) e da extensão (`sugerir`, a cada página;
/// `aguardar_login`, a cada 25 s) não adia a trava: um agente que só lista não segura
/// o cofre aberto.
///
/// `destravar` não entra: o destravar que abre o cofre já marca o início da contagem, e
/// o que chega com o cofre aberto responde `ok` sem olhar a senha. Erro nunca conta.
pub fn conta_como_uso(papel: Role, op: Operation, r: &Resposta) -> bool {
    if matches!(r, Resposta::Erro(_)) {
        return false;
    }
    match papel {
        // Fase 55 (reauditoria final): o `detalhe` que a janela de aprovação pede para
        // mostrar o pedido, o `totp` que o formulário renova sozinho e a resposta da
        // janela (um "Negar" a um `ward run` repetido do agente) também não: o pedido
        // permitido anota o uso ele mesmo, depois da autorização.
        Role::App => !matches!(
            op,
            Operation::Status
                | Operation::Lock
                | Operation::Unlock
                | Operation::OpenApp
                | Operation::Detail
                | Operation::Totp
                | Operation::AnswerApproval
        ),
        Role::Agent => matches!(
            op,
            Operation::Run | Operation::Ssh | Operation::Login | Operation::Playwright
        ),
        Role::Extension => matches!(
            op,
            Operation::Fill | Operation::FillLogin | Operation::SaveLogin
        ),
    }
}

/// O tempo sem uso venceu? `ultimo`/`agora` são leituras do mesmo relógio monotônico.
/// Sem leitura, relógio que voltou ou conta que estoura: venceu (falha fechada).
pub fn venceu(ultimo: Option<Duration>, agora: Option<Duration>, minutos: u32) -> bool {
    let (Some(ultimo), Some(agora)) = (ultimo, agora) else {
        return true;
    };
    let Some(passado) = agora.checked_sub(ultimo) else {
        return true;
    };
    passado >= Duration::from_secs(u64::from(minutos) * 60)
}

/// Por que o logind pediu a trava (só para o log).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evento {
    /// `Lock` de uma sessão.
    TelaBloqueada,
    /// `LockedHint` verdadeiro ou invalidado.
    LockedHint,
    /// `PrepareForSleep(true)`.
    Suspensao,
    /// `PrepareForSleep(false)`: pegar o inibidor de novo.
    Acordou,
}

impl Evento {
    fn texto(self) -> &'static str {
        match self {
            Evento::TelaBloqueada => "tela bloqueada",
            Evento::LockedHint => "sessão travada (LockedHint)",
            Evento::Suspensao => "suspensão",
            Evento::Acordou => "acordou",
        }
    }
}

/// O que um sinal do logind pede. Corpo que não se lê num sinal que pode travar trava.
pub fn evento_do_sinal(msg: &Message) -> Option<Evento> {
    if msg.message_type() != TipoMensagem::Signal {
        return None;
    }
    let h = msg.header();
    let iface = h.interface()?.as_str();
    let membro = h.member()?.as_str();
    let caminho = h.path()?.as_str();
    let de_sessao = caminho
        .strip_prefix(CAMINHO_SESSOES)
        .is_some_and(|resto| resto.starts_with('/') && resto.len() > 1);
    match (iface, membro) {
        (IFACE_MANAGER, "PrepareForSleep") if caminho == CAMINHO_LOGIND => {
            match msg.body().deserialize::<bool>() {
                Ok(false) => Some(Evento::Acordou),
                _ => Some(Evento::Suspensao),
            }
        }
        (IFACE_SESSAO, "Lock") if de_sessao => Some(Evento::TelaBloqueada),
        (IFACE_PROPRIEDADES, "PropertiesChanged") if de_sessao => {
            let corpo = msg
                .body()
                .deserialize::<(String, HashMap<String, OwnedVariant>, Vec<String>)>();
            let Ok((interface, mudou, invalidou)) = corpo else {
                return Some(Evento::LockedHint);
            };
            if interface != IFACE_SESSAO {
                return None;
            }
            let travou = match mudou.get("LockedHint").map(|v| &**v) {
                Some(Variant::Bool(b)) => *b,
                Some(_) => true,
                None => false,
            };
            (travou || invalidou.iter().any(|p| p == "LockedHint")).then_some(Evento::LockedHint)
        }
        _ => None,
    }
}

/// Socket do barramento que o daemon vigia, ou `None` (sem logind).
///
/// Release: sempre [`BARRAMENTO_SISTEMA`]. Daemon de teste: só `WARD_DEV_DBUS_SISTEMA`,
/// conferido por [`caminho_do_barramento`]; sem ela, `None`. O nome da variável só
/// existe no binário com `test-mode` (Fase 51a: o `install.sh` recusa qualquer
/// `WARD_DEV_*` no artefato, sem exceção).
pub fn barramento_do_daemon() -> Result<Option<PathBuf>, String> {
    #[cfg(not(feature = "test-mode"))]
    {
        Ok(Some(PathBuf::from(BARRAMENTO_SISTEMA)))
    }
    #[cfg(feature = "test-mode")]
    {
        match std::env::var_os("WARD_DEV_DBUS_SISTEMA") {
            None => Ok(None),
            Some(v) => caminho_do_barramento(Path::new(&v)).map(Some).map_err(|_| {
                "WARD_DEV_DBUS_SISTEMA recusado: caminho absoluto simples de um socket".into()
            }),
        }
    }
}

/// Caminho de socket aceito para o barramento: absoluto, sem `.`/`..`/`//`, até 107
/// bytes e só `[A-Za-z0-9/._-]` (vai para o endereço D-Bus sem escape).
pub fn caminho_do_barramento(p: &Path) -> Result<PathBuf, String> {
    use std::os::unix::ffi::OsStrExt;
    let b = p.as_os_str().as_bytes();
    let ok = b.first() == Some(&b'/')
        && b.len() <= MAX_CAMINHO
        && b.iter()
            .all(|c| c.is_ascii_alphanumeric() || b"/._-".contains(c))
        && !b
            .split(|c| *c == b'/')
            .skip(1)
            .any(|parte| parte.is_empty() || parte == b"." || parte == b"..");
    if ok {
        Ok(p.to_path_buf())
    } else {
        Err("caminho do barramento recusado: caminho absoluto simples de um socket".into())
    }
}

/// Conexão ao barramento em `socket` (caminho conferido por [`caminho_do_barramento`]), com o
/// prazo [`PRAZO_CHAMADA`]. Também usada pelo udisks2 (Fase 46).
pub(crate) async fn conectar(socket: &Path) -> Option<Connection> {
    let caminho = caminho_do_barramento(socket).ok()?;
    let endereco = format!("unix:path={}", caminho.display());
    Builder::address(endereco.as_str())
        .method_timeout(PRAZO_CHAMADA)
        .build()
        .await
        .ok()
}

/// Inibidor `delay` de `sleep` (o logind espera o fd fechar antes de suspender).
async fn inibir(conn: &Connection) -> Option<OwnedFd> {
    let r = conn
        .call_method(
            Some(LOGIND),
            CAMINHO_LOGIND,
            Some(IFACE_MANAGER),
            "Inhibit",
            &(
                "sleep",
                "Ward",
                "Travar o cofre antes de suspender",
                "delay",
            ),
        )
        .await;
    match r.and_then(|m| m.body().deserialize::<zbus::OwnedFd>()) {
        Ok(fd) => Some(fd.into()),
        Err(_) => {
            aviso!("logind: sem inibidor de suspensão (a trava na suspensão pode chegar tarde)");
            None
        }
    }
}

fn travar(daemon: &Daemon, motivo: &str) {
    if !daemon.travado() {
        daemon.travar();
        aviso!("{motivo}: cofre travado");
    }
}

/// Próxima mensagem do fluxo; `None` quando a conexão fecha.
async fn proxima(sinais: &mut MessageStream) -> Option<zbus::Result<Message>> {
    poll_fn(|cx| Pin::new(&mut *sinais).poll_next(cx)).await
}

/// Como uma sessão no barramento acabou.
enum Fim {
    /// Não conectou (ou não assinou os sinais).
    NaoConectou,
    /// Estava aberta e caiu.
    Caiu,
}

/// Uma sessão no barramento: assina os sinais, segura o inibidor e trava a cada evento.
async fn sessao(daemon: &Daemon, socket: &Path, pronto: &dyn Fn()) -> Fim {
    let Some(conn) = conectar(socket).await else {
        return Fim::NaoConectou;
    };
    let Ok(regra) = MatchRule::builder()
        .msg_type(TipoMensagem::Signal)
        .sender(LOGIND)
        .path_namespace(CAMINHO_LOGIND)
        .build()
    else {
        return Fim::NaoConectou;
    };
    // A assinatura vem antes do inibidor: nenhum sinal entre os dois se perde.
    let Ok(mut sinais) = MessageStream::for_match_rule(regra, &conn, Some(FILA)).await else {
        return Fim::NaoConectou;
    };
    let mut inibidor = inibir(&conn).await;
    aviso!("logind: vigiando bloqueio de tela e suspensão");
    pronto();
    while let Some(Ok(msg)) = proxima(&mut sinais).await {
        match evento_do_sinal(&msg) {
            None => {}
            Some(Evento::Acordou) => {
                if inibidor.is_none() {
                    inibidor = inibir(&conn).await;
                }
            }
            Some(e) => {
                travar(daemon, e.texto());
                if e == Evento::Suspensao {
                    // Só depois de travar: a suspensão espera este fd fechar.
                    drop(inibidor.take());
                }
            }
        }
    }
    drop(inibidor);
    Fim::Caiu
}

/// Thread que vigia o logind em `socket` para sempre. `pronto` é chamado a cada conexão
/// aberta (os testes esperam por ele).
pub fn vigiar_logind(
    daemon: Arc<Daemon>,
    socket: PathBuf,
    pronto: Box<dyn Fn() + Send>,
) -> io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("wardd-logind".into())
        .spawn(move || {
            let mut espera = Duration::from_secs(1);
            let mut avisou = false;
            loop {
                match zbus::block_on(sessao(&daemon, &socket, &*pronto)) {
                    Fim::Caiu => {
                        travar(&daemon, "conexão com o logind perdida");
                        espera = Duration::from_secs(1);
                        avisou = false;
                    }
                    Fim::NaoConectou if !avisou => {
                        aviso!(
                            "logind: barramento do sistema indisponível; só a trava por \
                             inatividade vale até conectar"
                        );
                        avisou = true;
                    }
                    Fim::NaoConectou => {}
                }
                std::thread::sleep(espera);
                espera = (espera * 2).min(ESPERA_MAXIMA_BARRAMENTO);
            }
        })
}

/// Thread que confere a inatividade a cada `intervalo`, para sempre.
pub fn vigiar_inatividade(daemon: Arc<Daemon>, intervalo: Duration) -> io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("wardd-inatividade".into())
        .spawn(move || {
            loop {
                std::thread::sleep(intervalo);
                daemon.travar_se_inativo();
            }
        })
}

#[cfg(test)]
mod testes {
    use super::*;
    use ward_core::protocol::ResponseError;

    const H: u64 = 3600;

    #[test]
    fn venceu_no_limite_e_falha_fechada() {
        let s = Duration::from_secs;
        assert!(!venceu(Some(s(0)), Some(s(4 * H - 60)), 240));
        assert!(venceu(Some(s(0)), Some(s(4 * H)), 240));
        assert!(!venceu(Some(s(3 * H)), Some(s(7 * H - 1)), 240));
        // Relógio ilegível ou que voltou: trava.
        assert!(venceu(None, Some(s(1)), 240));
        assert!(venceu(Some(s(1)), None, 240));
        assert!(venceu(Some(s(10)), Some(s(9)), 240));
        // Tempo da configuração.
        assert!(venceu(Some(s(0)), Some(s(60)), 1));
        assert!(!venceu(Some(s(0)), Some(s(59)), 1));
    }

    #[test]
    fn estado_travar_e_erro_nao_contam_como_uso() {
        let lista = Resposta::Lista(vec![]);
        for p in [Role::Agent, Role::App, Role::Extension] {
            assert!(!conta_como_uso(p, Operation::Status, &Resposta::Ok));
            assert!(!conta_como_uso(p, Operation::Lock, &Resposta::Ok));
            assert!(!conta_como_uso(p, Operation::Unlock, &Resposta::Ok));
            assert!(!conta_como_uso(p, Operation::OpenApp, &Resposta::Ok));
        }
        assert!(!conta_como_uso(
            Role::App,
            Operation::List,
            &Resposta::Erro(ResponseError::Locked)
        ));
        assert!(!conta_como_uso(
            Role::App,
            Operation::SetLevel,
            &Resposta::Erro(ResponseError::WrongPin)
        ));
        assert!(conta_como_uso(Role::App, Operation::List, &lista));
        assert!(conta_como_uso(Role::App, Operation::Reveal, &Resposta::Ok));
    }

    /// Fase 54: consulta do agente e da extensão não adia a trava; entrega, sim.
    #[test]
    fn ataque_lista_e_sugerir_nao_contam_como_uso() {
        let lista = Resposta::Lista(vec![]);
        assert!(!conta_como_uso(Role::Agent, Operation::List, &lista));
        assert!(!conta_como_uso(
            Role::Agent,
            Operation::Stdin,
            &Resposta::Ok
        ));
        assert!(!conta_como_uso(
            Role::Agent,
            Operation::Resize,
            &Resposta::Ok
        ));
        assert!(!conta_como_uso(
            Role::Extension,
            Operation::Suggest,
            &Resposta::Ok
        ));
        assert!(!conta_como_uso(
            Role::Extension,
            Operation::AwaitLogin,
            &Resposta::Ok
        ));
        assert!(!conta_como_uso(
            Role::Extension,
            Operation::LoginResult,
            &Resposta::Ok
        ));
        assert!(conta_como_uso(Role::Agent, Operation::Run, &Resposta::Ok));
        assert!(conta_como_uso(
            Role::Extension,
            Operation::Fill,
            &Resposta::Ok
        ));
        assert!(conta_como_uso(
            Role::Extension,
            Operation::FillLogin,
            &Resposta::Ok
        ));
    }

    /// Fase 55: o que a janela de aprovação e o formulário pedem sozinhos não conta.
    #[test]
    fn ataque_f55_detalhe_totp_e_resposta_da_janela_nao_contam() {
        for op in [
            Operation::Detail,
            Operation::Totp,
            Operation::AnswerApproval,
        ] {
            assert!(!conta_como_uso(Role::App, op, &Resposta::Ok), "{op:?}");
        }
        assert!(conta_como_uso(Role::App, Operation::Copy, &Resposta::Ok));
        assert!(conta_como_uso(
            Role::App,
            Operation::SaveEntry,
            &Resposta::Ok
        ));
    }

    #[test]
    fn ataque_caminho_do_barramento_estranho() {
        for ruim in [
            "relativo/bus",
            "/a/../bus",
            "/a/./bus",
            "/a//bus",
            "/a/bus,guid=1",
            "/a/bus;unix:path=/b",
            "/a/b us",
            "/a/bus%2c",
            "/",
        ] {
            assert!(caminho_do_barramento(Path::new(ruim)).is_err(), "{ruim}");
        }
        let longo = format!("/{}", "a".repeat(MAX_CAMINHO));
        assert!(caminho_do_barramento(Path::new(&longo)).is_err());
        assert!(caminho_do_barramento(Path::new("/run/user/1000/ward-teste/bus-1")).is_ok());
    }
}
