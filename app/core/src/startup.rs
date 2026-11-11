//! Startup of the setgid app (D47, D57; S1, S8). The Tauri `main` calls [`start`]
//! before anything else, before GTK. The order:
//! 1. `PR_SET_DUMPABLE=0` and core 0 (the setgid `exec` already left the process
//!    non-dumpable for the user; with `fs.suid_dumpable=2` the value is 2, and here it
//!    becomes 0);
//! 2. no debugger (`TracerPid` 0);
//! 3. the environment received is exactly the D57 one (what the daemon launcher builds),
//!    otherwise the app exits without connecting
//!    ([`ward_platform::verify_app_environment`]);
//! 4. keeps the parent (pidfd), to know later whether it is still the parent;
//! 5. connects to the socket, with the server check (D46), still with the egid of the
//!    `ward-app` group: the daemon gives the role by it; the socket has `CLOEXEC`;
//! 6. drops the group (`setresgid` to the real gid; GTK 3 refuses to start with setgid)
//!    and calls `PR_SET_DUMPABLE=0` again (changing gid brings dumpable back to
//!    `fs.suid_dumpable`);
//! 7. checks gids, dumpable and debugger again, and that the parent looks like the daemon
//!    (`wardd` of the same uid, non-dumpable);
//! 8. asks the status and checks the role in the first response (D63): without `app`, it
//!    does not use the connection and exits with a message, before any screen. With
//!    another parent (the app from the menu, from autostart), it first asks the daemon
//!    for the true instance (open app request, D57): the daemon decides the role, and with
//!    another parent it has already given the agent role.
//!
//! Before step 3: opened from the menu or from autostart (the parent is not the daemon),
//! the environment is never the D57 one. The app then drops the `ward-app` group before
//! connecting, sends the open app request as an ordinary client (the daemon accepts it from any
//! role) and exits: the true instance is the one the daemon opens, with the environment
//! it builds.
//!
//! Afterwards, [`Startup::watch`] waits for the connection to drop (without reading
//! anything). If it drops and the parent is still the same daemon, the app executes
//! itself again (`/proc/self/exe`, so the kernel applies the setgid again and the daemon
//! gives the role again); with another parent, it exits with a message.

use std::ffi::OsString;
use std::fmt;
use std::os::unix::net::UnixStream;

use ward_platform::app::{self, Parent, ParentMismatch};
use ward_platform::{AppEnvironment, SessionError, harden_process, verify_app_environment};

use ward_core::protocol::Role;

use crate::AppError;
use crate::client;
use crate::daemon::DaemonSocket;

/// Why the app did not open. No variable value nor path.
#[derive(Debug)]
pub enum StartupError {
    /// `PR_SET_DUMPABLE`/`RLIMIT_CORE` did not end up at 0.
    Harden,
    /// Under a debugger.
    Debugger,
    /// Environment outside D57.
    Environment(SessionError),
    /// The parent is not (or does not look like) the daemon.
    Parent(ParentMismatch),
    /// The daemon did not accept the connection or is not trusted (D46).
    Daemon(AppError),
    /// The socket did not end up with `CLOEXEC`.
    NoCloexec,
    /// `setresgid` did not leave the three gids at the real one.
    Group,
    /// The daemon did not give the app role to this connection (D63).
    Role(Role),
    /// Opened outside the daemon: asked for the true instance (open app) and exits.
    Delegated,
}

impl fmt::Display for StartupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("o app do Ward não abriu: ")?;
        match self {
            StartupError::Harden => f.write_str("não deu para endurecer o processo"),
            StartupError::Debugger => f.write_str("processo sob depurador"),
            StartupError::Environment(e) => write!(
                f,
                "{e}; abra o Ward pelo daemon (o ambiente do app é montado por ele)"
            ),
            StartupError::Parent(e) => write!(f, "{e}; abra o Ward pelo menu ou pela bandeja"),
            StartupError::Daemon(e) => write!(f, "{e}"),
            StartupError::NoCloexec => f.write_str("o socket do daemon ficou herdável"),
            StartupError::Group => f.write_str("não deu para largar o grupo do app"),
            StartupError::Role(p) => write!(
                f,
                "o daemon deu o papel {} a este processo, não o de app; abra o Ward pelo menu ou pela bandeja",
                p.name()
            ),
            StartupError::Delegated => f.write_str("pedido ao daemon; ele abre o app"),
        }
    }
}

impl std::error::Error for StartupError {}

/// The app after the startup: the connection (inside the [`DaemonSocket`]), the checked
/// environment and the parent from the start.
pub struct Startup {
    environment: AppEnvironment,
    parent: Parent,
    daemon: Option<DaemonSocket>,
    watcher: UnixStream,
    args: Vec<OsString>,
}

/// The whole startup with this process's environment and default daemon.
pub fn start() -> Result<Startup, StartupError> {
    let parent_is_daemon = Parent::current()
        .and_then(|p| p.verify_looks_like_daemon())
        .is_ok();
    start_with(
        std::env::vars_os(),
        DaemonSocket::standard,
        parent_is_daemon,
    )
}

fn tracer_zero() -> bool {
    std::fs::read("/proc/self/status").ok().and_then(|s| {
        s.split(|&b| b == b'\n')
            .find_map(|l| l.strip_prefix(b"TracerPid:"))
            .and_then(|v| std::str::from_utf8(v).ok())
            .and_then(|v| v.trim().parse::<u64>().ok())
    }) == Some(0)
}

fn start_with<I, F>(
    environment: I,
    daemon: F,
    parent_is_daemon: bool,
) -> Result<Startup, StartupError>
where
    I: IntoIterator<Item = (OsString, OsString)>,
    F: FnOnce() -> Result<DaemonSocket, AppError>,
{
    harden_process().map_err(|_| StartupError::Harden)?;
    if !tracer_zero() {
        return Err(StartupError::Debugger);
    }
    if !parent_is_daemon {
        return Err(delegate(daemon));
    }
    let environment = verify_app_environment(environment).map_err(StartupError::Environment)?;
    let parent = Parent::current().map_err(StartupError::Parent)?;

    let mut daemon = daemon().map_err(StartupError::Daemon)?;
    let connection = daemon.connect_now().map_err(StartupError::Daemon)?;
    if !app::has_cloexec(connection).unwrap_or(false) {
        return Err(StartupError::NoCloexec);
    }
    let watcher = connection
        .try_clone()
        .map_err(|_| StartupError::Daemon(AppError::DaemonUnavailable))?;

    app::release_group().map_err(|_| StartupError::Group)?;
    harden_process().map_err(|_| StartupError::Harden)?;
    match app::gids() {
        Ok((r, e, s)) if r == e && e == s => {}
        _ => return Err(StartupError::Group),
    }
    if app::dumpable().unwrap_or(true) {
        return Err(StartupError::Harden);
    }
    if !tracer_zero() {
        return Err(StartupError::Debugger);
    }
    if !app::has_cloexec(&watcher).unwrap_or(false) {
        return Err(StartupError::NoCloexec);
    }
    check_role(&mut daemon, parent.verify_looks_like_daemon())?;
    Ok(Startup {
        environment,
        parent,
        daemon: Some(daemon),
        watcher,
        args: std::env::args_os().skip(1).collect(),
    })
}

/// Opened outside the daemon (menu, autostart, terminal): drops the `ward-app` group
/// before connecting, so the connection does not carry the app egid, and asks for the
/// true instance.
fn delegate<F>(daemon: F) -> StartupError
where
    F: FnOnce() -> Result<DaemonSocket, AppError>,
{
    if app::release_group().is_err() {
        return StartupError::Group;
    }
    if harden_process().is_err() {
        return StartupError::Harden;
    }
    match app::gids() {
        Ok((r, e, s)) if r == e && e == s => {}
        _ => return StartupError::Group,
    }
    let mut daemon = match daemon() {
        Ok(d) => d,
        Err(e) => return StartupError::Daemon(e),
    };
    if let Err(e) = daemon.connect_now() {
        return StartupError::Daemon(e);
    }
    request_instance(&mut daemon)
}

/// Open app and nothing else: no other request on this connection.
fn request_instance(daemon: &mut DaemonSocket) -> StartupError {
    match client::open_app_as_agent(daemon) {
        Ok(()) => StartupError::Delegated,
        Err(e) => StartupError::Daemon(e),
    }
}

/// Step 8: the role in the first response (D63). With another parent, asks open app
/// before exiting (D57). No screen has been shown up to here.
fn check_role(
    daemon: &mut DaemonSocket,
    parent: Result<(), ParentMismatch>,
) -> Result<(), StartupError> {
    let role = client::role(daemon);
    if let Err(e) = parent {
        let _ = client::open_app(daemon);
        return Err(StartupError::Parent(e));
    }
    match role {
        Ok(Role::App) => Ok(()),
        Ok(p) => Err(StartupError::Role(p)),
        Err(e) => Err(StartupError::Daemon(e)),
    }
}

impl Startup {
    /// The daemon client with the startup connection. Only once.
    pub fn take_daemon(&mut self) -> Option<DaemonSocket> {
        self.daemon.take()
    }

    /// pid of the parent (the daemon).
    pub fn parent_pid(&self) -> u32 {
        self.parent.pid()
    }

    /// In a thread: waits for the connection to drop, calls `on_drop` (clear the
    /// clipboard) and then [`reexec_or_exit`](Self::reexec_or_exit).
    pub fn watch(self, on_drop: impl FnOnce() + Send + 'static) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            let _ = app::wait_for_close(&self.watcher);
            on_drop();
            self.reexec_or_exit()
        })
    }

    /// With the parent still equal to the daemon from the start, executes the app again
    /// with the same checked environment; otherwise, exits with a message. Does not return.
    pub fn reexec_or_exit(&self) -> ! {
        if self.parent.still_parent() {
            let e = app::reexec(&self.args, &self.environment);
            eprintln!("ward-app: a conexão com o daemon caiu e o app não se reabriu ({e})");
        } else {
            eprintln!("ward-app: a conexão com o daemon caiu e o pai não é mais o daemon");
        }
        std::process::exit(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;

    #[test]
    fn attack_outside_environment_does_not_get_to_connect() {
        // Any process opens the app with its own environment (or with a proxy in
        // `WAYLAND_DISPLAY`): the app exits before connecting, without using the role.
        let uid = ward_platform::effective_uid();
        let dir = PathBuf::from(format!(
            "/run/user/{uid}/ward-partida-teste-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("wardd.sock");
        let l = UnixListener::bind(&path).unwrap();
        l.set_nonblocking(true).unwrap();
        for environment in [
            vec![("WAYLAND_DISPLAY", "/tmp/proxy.sock")],
            vec![("GTK_MODULES", "/tmp/x.so"), ("PATH", "/usr/bin")],
            vec![],
        ] {
            let c = path.clone();
            let r = start_with(
                environment
                    .into_iter()
                    .map(|(k, v)| (OsString::from(k), OsString::from(v))),
                move || Ok(DaemonSocket::system_at(c)),
                true,
            );
            assert!(
                matches!(r, Err(StartupError::Environment(_))),
                "{:?}",
                r.err()
            );
        }
        assert!(l.accept().is_err(), "o app conectou com o ambiente errado");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// V3.38 (D63): the daemon on the other side of the pair answers the status with
    /// `role`; what it received comes back to the test.
    fn daemon_giving(role: Role) -> (DaemonSocket, std::thread::JoinHandle<Vec<String>>) {
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
                        papel: role,
                    },
                    _ => Resposta::Ok,
                };
                // The daemon writes the status in the role it gave (D63).
                let q = protocol::encode_response(role, &r).unwrap();
                frame::write_frame(&mut b, &q, frame::RESPONSE_LIMIT).unwrap();
            }
            ops
        });
        (d, h)
    }

    #[test]
    fn attack_without_app_role_in_the_first_response_exits_without_using_the_connection() {
        for role in [Role::Agent, Role::Extension] {
            let (mut d, h) = daemon_giving(role);
            let r = check_role(&mut d, Ok(()));
            assert!(
                matches!(r, Err(StartupError::Role(p)) if p == role),
                "{r:?}"
            );
            drop(d);
            // Only the status: no screen request afterwards.
            assert_eq!(h.join().unwrap(), ["estado"]);
        }
        let (mut d, h) = daemon_giving(Role::App);
        assert!(check_role(&mut d, Ok(())).is_ok());
        drop(d);
        assert_eq!(h.join().unwrap(), ["estado"]);
    }

    #[test]
    fn with_another_parent_asks_for_the_true_instance_and_exits() {
        // The app from the menu (any parent) received the agent role: asks open app and
        // exits (D57).
        let (mut d, h) = daemon_giving(Role::Agent);
        let r = check_role(&mut d, Err(ParentMismatch::NotDaemon));
        assert!(matches!(
            r,
            Err(StartupError::Parent(ParentMismatch::NotDaemon))
        ));
        drop(d);
        assert_eq!(h.join().unwrap(), ["estado", "abrir_app"]);
    }

    #[test]
    fn opened_from_the_menu_only_asks_open_app() {
        // The COSMIC launcher passes the whole session environment: the app does not
        // check the environment nor ask the status; only open app, and exits with no
        // screen.
        let (mut d, h) = daemon_giving(Role::Agent);
        assert!(matches!(request_instance(&mut d), StartupError::Delegated));
        drop(d);
        assert_eq!(h.join().unwrap(), ["abrir_app"]);
    }

    #[test]
    fn opened_from_the_menu_with_any_environment_does_not_check_the_environment() {
        // A parent that is not the daemon and an outside environment: goes to the request
        // to the daemon, which here fails the server check (D46), and not to the
        // environment error.
        let r = start_with(
            vec![(OsString::from("ALACRITTY_WINDOW_ID"), OsString::from("1"))],
            || Err(AppError::DaemonUnavailable),
            false,
        );
        assert!(matches!(r, Err(StartupError::Daemon(_))), "{:?}", r.err());
    }

    #[test]
    fn message_without_value() {
        let m = StartupError::Environment(SessionError::Different("WAYLAND_DISPLAY")).to_string();
        assert!(m.contains("WAYLAND_DISPLAY") && !m.contains("/tmp"));
        let m = StartupError::Parent(ParentMismatch::NotDaemon).to_string();
        assert!(m.starts_with("o app do Ward não abriu"));
    }
}
