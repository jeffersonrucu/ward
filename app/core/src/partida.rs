//! Partida do app setgid (D47, D57; S1, S8). O `main` do Tauri chama [`iniciar`] antes
//! de tudo, antes do GTK. A ordem:
//! 1. `PR_SET_DUMPABLE=0` e core 0 (o `exec` setgid já deixou o processo não-dumpable
//!    para o usuário; com `fs.suid_dumpable=2` o valor é 2, e aqui vira 0);
//! 2. sem depurador (`TracerPid` 0);
//! 3. o ambiente recebido é exatamente o de D57 (o que o lançador do daemon monta),
//!    senão o app sai sem conectar ([`ward_platform::verify_app_environment`]);
//! 4. guarda o pai (pidfd), para saber depois se ele continua sendo o pai;
//! 5. conecta ao socket, com a conferência do servidor (D46), ainda com o egid do grupo
//!    `ward-app`: o daemon dá o papel por ele; o socket tem `CLOEXEC`;
//! 6. larga o grupo (`setresgid` para o gid real; o GTK 3 se recusa a iniciar com
//!    setgid) e chama `PR_SET_DUMPABLE=0` de novo (trocar de gid volta o dumpable ao
//!    `fs.suid_dumpable`);
//! 7. confere de novo gids, dumpable e depurador, e que o pai parece o daemon (`wardd`
//!    do mesmo uid, não-dumpable);
//! 8. pede `estado` e confere o papel na primeira resposta (D63): sem `app`, não usa a
//!    conexão e sai com mensagem, antes de qualquer tela. Com outro pai (o app do menu,
//!    do autostart), pede antes ao daemon a instância verdadeira (`abrir_app`, D57):
//!    quem decide o papel é o daemon, e com outro pai ele já deu o papel agente.
//!
//! Antes do passo 3: aberto pelo menu ou pelo autostart (o pai não é o daemon), o
//! ambiente nunca é o de D57. O app então larga o grupo `ward-app` antes de conectar,
//! pede `abrir_app` como um cliente comum (o daemon aceita de qualquer papel) e sai: a
//! instância verdadeira é a que o daemon abre, com o ambiente que ele monta.
//!
//! Depois, [`Partida::vigiar`] espera a conexão cair (sem ler nada). Se ela cair e o
//! pai ainda for o mesmo daemon, o app se executa de novo (`/proc/self/exe`, para o
//! kernel aplicar o setgid outra vez e o daemon dar o papel de novo); com outro pai,
//! sai com mensagem.

use std::ffi::OsString;
use std::fmt;
use std::os::unix::net::UnixStream;

use ward_platform::app::{self, Parent, ParentMismatch};
use ward_platform::{AppEnvironment, SessionError, harden_process, verify_app_environment};

use ward_core::protocol::Role;

use crate::AppError;
use crate::client;
use crate::daemon::DaemonSocket;

/// Por que o app não abriu. Sem valor de variável nem caminho.
#[derive(Debug)]
pub enum ErroPartida {
    /// `PR_SET_DUMPABLE`/`RLIMIT_CORE` não ficaram em 0.
    Endurecer,
    /// Sob depurador.
    Depurador,
    /// Ambiente fora de D57.
    Ambiente(SessionError),
    /// O pai não é (ou não parece) o daemon.
    Pai(ParentMismatch),
    /// O daemon não aceitou a conexão ou não é confiável (D46).
    Daemon(AppError),
    /// O socket não ficou com `CLOEXEC`.
    SemCloexec,
    /// `setresgid` não deixou os três gids no real.
    Grupo,
    /// O daemon não deu o papel app a esta conexão (D63).
    Papel(Role),
    /// Aberto fora do daemon: pediu a instância verdadeira (`abrir_app`) e sai.
    Delegado,
}

impl fmt::Display for ErroPartida {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("o app do Ward não abriu: ")?;
        match self {
            ErroPartida::Endurecer => f.write_str("não deu para endurecer o processo"),
            ErroPartida::Depurador => f.write_str("processo sob depurador"),
            ErroPartida::Ambiente(e) => write!(
                f,
                "{e}; abra o Ward pelo daemon (o ambiente do app é montado por ele)"
            ),
            ErroPartida::Pai(e) => write!(f, "{e}; abra o Ward pelo menu ou pela bandeja"),
            ErroPartida::Daemon(e) => write!(f, "{e}"),
            ErroPartida::SemCloexec => f.write_str("o socket do daemon ficou herdável"),
            ErroPartida::Grupo => f.write_str("não deu para largar o grupo do app"),
            ErroPartida::Papel(p) => write!(
                f,
                "o daemon deu o papel {} a este processo, não o de app; abra o Ward pelo menu ou pela bandeja",
                p.name()
            ),
            ErroPartida::Delegado => f.write_str("pedido ao daemon; ele abre o app"),
        }
    }
}

impl std::error::Error for ErroPartida {}

/// O app depois da partida: a conexão (dentro do [`DaemonSocket`]), o ambiente
/// conferido e o pai do começo.
pub struct Partida {
    ambiente: AppEnvironment,
    pai: Parent,
    daemon: Option<DaemonSocket>,
    vigia: UnixStream,
    argumentos: Vec<OsString>,
}

/// A partida inteira com o ambiente e o daemon padrão deste processo.
pub fn iniciar() -> Result<Partida, ErroPartida> {
    let pai_e_o_daemon = Parent::current()
        .and_then(|p| p.verify_looks_like_daemon())
        .is_ok();
    iniciar_com(std::env::vars_os(), DaemonSocket::standard, pai_e_o_daemon)
}

fn tracer_zero() -> bool {
    std::fs::read("/proc/self/status").ok().and_then(|s| {
        s.split(|&b| b == b'\n')
            .find_map(|l| l.strip_prefix(b"TracerPid:"))
            .and_then(|v| std::str::from_utf8(v).ok())
            .and_then(|v| v.trim().parse::<u64>().ok())
    }) == Some(0)
}

fn iniciar_com<I, F>(ambiente: I, daemon: F, pai_e_o_daemon: bool) -> Result<Partida, ErroPartida>
where
    I: IntoIterator<Item = (OsString, OsString)>,
    F: FnOnce() -> Result<DaemonSocket, AppError>,
{
    harden_process().map_err(|_| ErroPartida::Endurecer)?;
    if !tracer_zero() {
        return Err(ErroPartida::Depurador);
    }
    if !pai_e_o_daemon {
        return Err(delegar(daemon));
    }
    let ambiente = verify_app_environment(ambiente).map_err(ErroPartida::Ambiente)?;
    let pai = Parent::current().map_err(ErroPartida::Pai)?;

    let mut daemon = daemon().map_err(ErroPartida::Daemon)?;
    let conexao = daemon.connect_now().map_err(ErroPartida::Daemon)?;
    if !app::has_cloexec(conexao).unwrap_or(false) {
        return Err(ErroPartida::SemCloexec);
    }
    let vigia = conexao
        .try_clone()
        .map_err(|_| ErroPartida::Daemon(AppError::DaemonUnavailable))?;

    app::release_group().map_err(|_| ErroPartida::Grupo)?;
    harden_process().map_err(|_| ErroPartida::Endurecer)?;
    match app::gids() {
        Ok((r, e, s)) if r == e && e == s => {}
        _ => return Err(ErroPartida::Grupo),
    }
    if app::dumpable().unwrap_or(true) {
        return Err(ErroPartida::Endurecer);
    }
    if !tracer_zero() {
        return Err(ErroPartida::Depurador);
    }
    if !app::has_cloexec(&vigia).unwrap_or(false) {
        return Err(ErroPartida::SemCloexec);
    }
    conferir_papel(&mut daemon, pai.verify_looks_like_daemon())?;
    Ok(Partida {
        ambiente,
        pai,
        daemon: Some(daemon),
        vigia,
        argumentos: std::env::args_os().skip(1).collect(),
    })
}

/// Aberto fora do daemon (menu, autostart, terminal): larga o grupo `ward-app` antes de
/// conectar, para a conexão não levar o egid do app, e pede a instância verdadeira.
fn delegar<F>(daemon: F) -> ErroPartida
where
    F: FnOnce() -> Result<DaemonSocket, AppError>,
{
    if app::release_group().is_err() {
        return ErroPartida::Grupo;
    }
    if harden_process().is_err() {
        return ErroPartida::Endurecer;
    }
    match app::gids() {
        Ok((r, e, s)) if r == e && e == s => {}
        _ => return ErroPartida::Grupo,
    }
    let mut daemon = match daemon() {
        Ok(d) => d,
        Err(e) => return ErroPartida::Daemon(e),
    };
    if let Err(e) = daemon.connect_now() {
        return ErroPartida::Daemon(e);
    }
    pedir_instancia(&mut daemon)
}

/// `abrir_app` e nada mais: nenhum outro pedido nesta conexão.
fn pedir_instancia(daemon: &mut DaemonSocket) -> ErroPartida {
    match client::open_app_as_agent(daemon) {
        Ok(()) => ErroPartida::Delegado,
        Err(e) => ErroPartida::Daemon(e),
    }
}

/// Passo 8: o papel na primeira resposta (D63). Com outro pai, pede `abrir_app` antes
/// de sair (D57). Nenhuma tela foi mostrada até aqui.
fn conferir_papel(
    daemon: &mut DaemonSocket,
    pai: Result<(), ParentMismatch>,
) -> Result<(), ErroPartida> {
    let papel = client::role(daemon);
    if let Err(e) = pai {
        let _ = client::open_app(daemon);
        return Err(ErroPartida::Pai(e));
    }
    match papel {
        Ok(Role::App) => Ok(()),
        Ok(p) => Err(ErroPartida::Papel(p)),
        Err(e) => Err(ErroPartida::Daemon(e)),
    }
}

impl Partida {
    /// O cliente do daemon com a conexão da partida. Só uma vez.
    pub fn tomar_daemon(&mut self) -> Option<DaemonSocket> {
        self.daemon.take()
    }

    /// pid do pai (o daemon).
    pub fn parent_pid(&self) -> u32 {
        self.pai.pid()
    }

    /// Numa thread: espera a conexão cair, chama `ao_cair` (limpar a área de
    /// transferência) e depois [`reexecutar_ou_sair`](Self::reexecutar_ou_sair).
    pub fn vigiar(self, ao_cair: impl FnOnce() + Send + 'static) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            let _ = app::wait_for_close(&self.vigia);
            ao_cair();
            self.reexecutar_ou_sair()
        })
    }

    /// Com o pai ainda igual ao daemon do começo, executa o app de novo com o mesmo
    /// ambiente conferido; senão, sai com mensagem. Não volta.
    pub fn reexecutar_ou_sair(&self) -> ! {
        if self.pai.still_parent() {
            let e = app::reexec(&self.argumentos, &self.ambiente);
            eprintln!("ward-app: a conexão com o daemon caiu e o app não se reabriu ({e})");
        } else {
            eprintln!("ward-app: a conexão com o daemon caiu e o pai não é mais o daemon");
        }
        std::process::exit(1)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;

    #[test]
    fn ataque_ambiente_de_fora_nao_chega_a_conectar() {
        // Um processo qualquer abre o app com o próprio ambiente (ou com um proxy em
        // `WAYLAND_DISPLAY`): o app sai antes de conectar, sem usar o papel.
        let uid = ward_platform::effective_uid();
        let dir = PathBuf::from(format!(
            "/run/user/{uid}/ward-partida-teste-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let caminho = dir.join("wardd.sock");
        let l = UnixListener::bind(&caminho).unwrap();
        l.set_nonblocking(true).unwrap();
        for ambiente in [
            vec![("WAYLAND_DISPLAY", "/tmp/proxy.sock")],
            vec![("GTK_MODULES", "/tmp/x.so"), ("PATH", "/usr/bin")],
            vec![],
        ] {
            let c = caminho.clone();
            let r = iniciar_com(
                ambiente
                    .into_iter()
                    .map(|(k, v)| (OsString::from(k), OsString::from(v))),
                move || Ok(DaemonSocket::system_at(c)),
                true,
            );
            assert!(matches!(r, Err(ErroPartida::Ambiente(_))), "{:?}", r.err());
        }
        assert!(l.accept().is_err(), "o app conectou com o ambiente errado");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// V3.38 (D63): o daemon do outro lado do par responde `estado` com `papel`; o que
    /// ele recebeu volta para o teste.
    fn daemon_que_da(papel: Role) -> (DaemonSocket, std::thread::JoinHandle<Vec<String>>) {
        use ward_core::protocol::{self, Resposta, frame};
        let (a, mut b) = std::os::unix::net::UnixStream::pair().unwrap();
        let d = DaemonSocket::from_pair(a, std::time::Duration::from_secs(10));
        let h = std::thread::spawn(move || {
            let mut ops = Vec::new();
            while let Ok(q) = frame::read_frame(&mut b, frame::frame_limit(Role::App)) {
                let p = protocol::decode_request(Role::App, &q).unwrap();
                ops.push(p.operation().name().to_owned());
                let r = match p {
                    protocol::Request::Status => Resposta::Estado {
                        travado: true,
                        cofre_existe: true,
                        papel,
                    },
                    _ => Resposta::Ok,
                };
                // O daemon escreve o `estado` no papel que deu (D63).
                let q = protocol::encode_response(papel, &r).unwrap();
                frame::write_frame(&mut b, &q, frame::RESPONSE_LIMIT).unwrap();
            }
            ops
        });
        (d, h)
    }

    #[test]
    fn ataque_sem_papel_app_na_primeira_resposta_sai_sem_usar_a_conexao() {
        for papel in [Role::Agent, Role::Extension] {
            let (mut d, h) = daemon_que_da(papel);
            let r = conferir_papel(&mut d, Ok(()));
            assert!(
                matches!(r, Err(ErroPartida::Papel(p)) if p == papel),
                "{r:?}"
            );
            drop(d);
            // Só o `estado`: nenhum pedido de tela depois.
            assert_eq!(h.join().unwrap(), ["estado"]);
        }
        let (mut d, h) = daemon_que_da(Role::App);
        assert!(conferir_papel(&mut d, Ok(())).is_ok());
        drop(d);
        assert_eq!(h.join().unwrap(), ["estado"]);
    }

    #[test]
    fn com_outro_pai_pede_a_instancia_verdadeira_e_sai() {
        // O app do menu (pai qualquer) recebeu `agente`: pede `abrir_app` e sai (D57).
        let (mut d, h) = daemon_que_da(Role::Agent);
        let r = conferir_papel(&mut d, Err(ParentMismatch::NotDaemon));
        assert!(matches!(
            r,
            Err(ErroPartida::Pai(ParentMismatch::NotDaemon))
        ));
        drop(d);
        assert_eq!(h.join().unwrap(), ["estado", "abrir_app"]);
    }

    #[test]
    fn aberto_pelo_menu_so_pede_abrir_app() {
        // O launcher do COSMIC passa o ambiente da sessão inteira: o app não confere o
        // ambiente nem pede `estado`; só `abrir_app`, e sai sem tela.
        let (mut d, h) = daemon_que_da(Role::Agent);
        assert!(matches!(pedir_instancia(&mut d), ErroPartida::Delegado));
        drop(d);
        assert_eq!(h.join().unwrap(), ["abrir_app"]);
    }

    #[test]
    fn aberto_pelo_menu_com_ambiente_qualquer_nao_confere_o_ambiente() {
        // Pai que não é o daemon e ambiente de fora: vai para o pedido ao daemon, que
        // aqui falha na conferência do servidor (D46), e não para o erro de ambiente.
        let r = iniciar_com(
            vec![(OsString::from("ALACRITTY_WINDOW_ID"), OsString::from("1"))],
            || Err(AppError::DaemonUnavailable),
            false,
        );
        assert!(matches!(r, Err(ErroPartida::Daemon(_))), "{:?}", r.err());
    }

    #[test]
    fn mensagem_sem_valor() {
        let m = ErroPartida::Ambiente(SessionError::Different("WAYLAND_DISPLAY")).to_string();
        assert!(m.contains("WAYLAND_DISPLAY") && !m.contains("/tmp"));
        let m = ErroPartida::Pai(ParentMismatch::NotDaemon).to_string();
        assert!(m.starts_with("o app do Ward não abriu"));
    }
}
