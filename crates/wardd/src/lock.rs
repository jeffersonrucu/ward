//! Automatic lock (Phase 24, R23, D30): no use for the time in the vault settings
//! (default 240 min), screen lock and suspend.
//!
//! - **Inactivity.** Each use resets the count ([`counts_as_use`]): credential delivery
//!   (agent and extension) and an app action that succeeded, except status, lock and
//!   unlock (the one that opens marks the start; with the vault open it answers `ok`
//!   without looking at the password) and, since Phase 55, the detail and the TOTP that
//!   the approval window and the form ask for on their own and the window's answer. A
//!   query from the agent (the list) and from the extension (the suggestions) does not
//!   count (Phase 54). An error does not count: an agent that sends invalid requests or
//!   a wrong PIN does not keep the vault open. The clock is the daemon's clock
//!   (`CLOCK_BOOTTIME`, injected in the tests), which runs during suspend. A clock that cannot be read or that goes back
//!   locks (fails closed, [`expired`]).
//! - **logind**, on the **system** bus (`wardd` is a system service, D46, and is not on
//!   the session bus):
//!   - `Lock` of a session (`loginctl lock-session`) and `LockedHint` = true (or
//!     invalidated) in the `PropertiesChanged` of a session both lock;
//!   - `PrepareForSleep(true)` locks. The daemon holds a `delay` inhibitor of `sleep`
//!     and only releases it **after** locking, so that the suspend does not arrive before
//!     the lock; on waking up (`PrepareForSleep(false)`) it takes another.
//!   - Any session locks, not only those of the daemon's uid: locking too much is safe,
//!     and checking the owner of each session would be one more request that can fail.
//!   - Nothing that arrives through the bus unlocks or delays the lock: a forged signal
//!     (from someone who is not logind) can only lock, which the agent can already ask
//!     for.
//!   - A connection lost after being open locks (there is no longer a way to see the
//!     screen lock) and tries again with a growing wait up to [`MAX_BUS_WAIT`]. Without
//!     a bus since startup, it only warns: the inactivity still holds.
//! - The bus address is fixed ([`SYSTEM_BUS`]); it never comes from the environment.
//!   Only the test daemon accepts `WARD_DEV_SYSTEM_DBUS` (path of a socket, for the
//!   tests' fake `dbus-daemon`); without it, the test daemon does not turn logind on.
//!
//! Nothing here touches a secret; the log only has fixed texts.

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
use ward_core::protocol::{Operation, Response, Role};

use futures_core::Stream;
use zbus::connection::Builder;
use zbus::message::Type as MessageType;
use zbus::{Connection, MatchRule, Message, MessageStream, OwnedVariant, Variant};

use crate::core::Daemon;
use crate::warning;

/// Socket of the system bus.
pub const SYSTEM_BUS: &str = "/run/dbus/system_bus_socket";
/// How often the inactivity is checked.
pub const INACTIVITY_INTERVAL: Duration = Duration::from_secs(10);
/// Largest wait between attempts to connect to the bus.
pub const MAX_BUS_WAIT: Duration = Duration::from_secs(60);
/// Deadline of the calls to logind (the inhibitor).
pub const CALL_DEADLINE: Duration = Duration::from_secs(5);
/// Messages in the queue before the reader waits.
const QUEUE: NonZeroUsize = NonZeroUsize::new(256).unwrap();

const LOGIND: &str = "org.freedesktop.login1";
const LOGIND_PATH: &str = "/org/freedesktop/login1";
const SESSIONS_PATH: &str = "/org/freedesktop/login1/session";
const IFACE_MANAGER: &str = "org.freedesktop.login1.Manager";
const SESSION_IFACE: &str = "org.freedesktop.login1.Session";
const PROPERTIES_IFACE: &str = "org.freedesktop.DBus.Properties";
/// Longest path accepted for the bus socket (`sun_path` has 108 bytes).
const MAX_PATH: usize = 107;

/// Does this answer to this operation, from this role, count as a use of the vault?
///
/// Phase 54 (final audit): only the use that delivers a credential (the agent's
/// `ward run`, `ssh`, `login` and `playwright`, which record the use themselves after
/// the authorization; the extension's fill) or an app action resets the count. A query
/// from the agent (the periodic list, the open-app operation) and from the extension
/// (the suggestions, on every page; the login wait, every 25 s) does not delay the lock:
/// an agent that only lists does not keep the vault open.
///
/// The unlock does not count: the unlock that opens the vault already marks the start of
/// the count, and the one that arrives with the vault open answers `ok` without looking
/// at the password. An error never counts.
pub fn counts_as_use(role: Role, op: Operation, r: &Response) -> bool {
    if matches!(r, Response::Error(_)) {
        return false;
    }
    match role {
        // Phase 55 (final re-audit): the detail that the approval window asks for to show
        // the request, the TOTP that the form renews on its own and the window's answer
        // (a "Deny" to a repeated `ward run` of the agent) do not either: the allowed
        // request records the use itself, after the authorization.
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

/// Has the time without use expired? `last`/`now` are readings of the same monotonic
/// clock. Without a reading, a clock that went back or an overflowing count: expired
/// (fails closed).
pub fn expired(last: Option<Duration>, now: Option<Duration>, minutes: u32) -> bool {
    let (Some(last), Some(now)) = (last, now) else {
        return true;
    };
    let Some(elapsed) = now.checked_sub(last) else {
        return true;
    };
    elapsed >= Duration::from_secs(u64::from(minutes) * 60)
}

/// Why logind asked for the lock (only for the log).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// `Lock` of a session.
    ScreenLocked,
    /// `LockedHint` true or invalidated.
    LockedHint,
    /// `PrepareForSleep(true)`.
    Suspend,
    /// `PrepareForSleep(false)`: take the inhibitor again.
    WokeUp,
}

impl Event {
    fn text(self) -> &'static str {
        match self {
            Event::ScreenLocked => "tela bloqueada",
            Event::LockedHint => "sessão travada (LockedHint)",
            Event::Suspend => "suspensão",
            Event::WokeUp => "acordou",
        }
    }
}

/// What a logind signal asks for. A body that cannot be read in a signal that can lock
/// locks.
pub fn event_from_signal(msg: &Message) -> Option<Event> {
    if msg.message_type() != MessageType::Signal {
        return None;
    }
    let h = msg.header();
    let iface = h.interface()?.as_str();
    let member = h.member()?.as_str();
    let path = h.path()?.as_str();
    let from_session = path
        .strip_prefix(SESSIONS_PATH)
        .is_some_and(|rest| rest.starts_with('/') && rest.len() > 1);
    match (iface, member) {
        (IFACE_MANAGER, "PrepareForSleep") if path == LOGIND_PATH => {
            match msg.body().deserialize::<bool>() {
                Ok(false) => Some(Event::WokeUp),
                _ => Some(Event::Suspend),
            }
        }
        (SESSION_IFACE, "Lock") if from_session => Some(Event::ScreenLocked),
        (PROPERTIES_IFACE, "PropertiesChanged") if from_session => {
            let body = msg
                .body()
                .deserialize::<(String, HashMap<String, OwnedVariant>, Vec<String>)>();
            let Ok((interface, changed, invalidated)) = body else {
                return Some(Event::LockedHint);
            };
            if interface != SESSION_IFACE {
                return None;
            }
            let locked = match changed.get("LockedHint").map(|v| &**v) {
                Some(Variant::Bool(b)) => *b,
                Some(_) => true,
                None => false,
            };
            (locked || invalidated.iter().any(|p| p == "LockedHint")).then_some(Event::LockedHint)
        }
        _ => None,
    }
}

/// Socket of the bus that the daemon watches, or `None` (without logind).
///
/// Release: always [`SYSTEM_BUS`]. Test daemon: only `WARD_DEV_SYSTEM_DBUS`, checked by
/// [`bus_path`]; without it, `None`. The variable's name only exists in the binary with
/// `test-mode` (Phase 51a: `install.sh` refuses any `WARD_DEV_*` in the artifact,
/// without exception).
pub fn daemon_bus() -> Result<Option<PathBuf>, String> {
    #[cfg(not(feature = "test-mode"))]
    {
        Ok(Some(PathBuf::from(SYSTEM_BUS)))
    }
    #[cfg(feature = "test-mode")]
    {
        match std::env::var_os("WARD_DEV_SYSTEM_DBUS") {
            None => Ok(None),
            Some(v) => bus_path(Path::new(&v)).map(Some).map_err(|_| {
                "WARD_DEV_SYSTEM_DBUS recusado: caminho absoluto simples de um socket".into()
            }),
        }
    }
}

/// Socket path accepted for the bus: absolute, without `.`/`..`/`//`, up to 107 bytes
/// and only `[A-Za-z0-9/._-]` (it goes into the D-Bus address without escape).
pub fn bus_path(p: &Path) -> Result<PathBuf, String> {
    use std::os::unix::ffi::OsStrExt;
    let b = p.as_os_str().as_bytes();
    let ok = b.first() == Some(&b'/')
        && b.len() <= MAX_PATH
        && b.iter()
            .all(|c| c.is_ascii_alphanumeric() || b"/._-".contains(c))
        && !b
            .split(|c| *c == b'/')
            .skip(1)
            .any(|part| part.is_empty() || part == b"." || part == b"..");
    if ok {
        Ok(p.to_path_buf())
    } else {
        Err("caminho do barramento recusado: caminho absoluto simples de um socket".into())
    }
}

/// Connection to the bus at `socket` (path checked by [`bus_path`]), with the deadline
/// [`CALL_DEADLINE`]. Also used by udisks2 (Phase 46).
pub(crate) async fn connect(socket: &Path) -> Option<Connection> {
    let path = bus_path(socket).ok()?;
    let address = format!("unix:path={}", path.display());
    Builder::address(address.as_str())
        .method_timeout(CALL_DEADLINE)
        .build()
        .await
        .ok()
}

/// `delay` inhibitor of `sleep` (logind waits for the fd to close before suspending).
async fn inhibit(conn: &Connection) -> Option<OwnedFd> {
    let r = conn
        .call_method(
            Some(LOGIND),
            LOGIND_PATH,
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
            warning!("logind: sem inibidor de suspensão (a trava na suspensão pode chegar tarde)");
            None
        }
    }
}

fn lock_vault(daemon: &Daemon, reason: &str) {
    if !daemon.is_locked() {
        daemon.lock();
        warning!("{reason}: cofre travado");
    }
}

/// Next message of the stream; `None` when the connection closes.
async fn next_message(signals: &mut MessageStream) -> Option<zbus::Result<Message>> {
    poll_fn(|cx| Pin::new(&mut *signals).poll_next(cx)).await
}

/// How a session on the bus ended.
enum End {
    /// Did not connect (or did not subscribe to the signals).
    NotConnected,
    /// Was open and dropped.
    Dropped,
}

/// A session on the bus: subscribes to the signals, holds the inhibitor and locks on
/// each event.
async fn session(daemon: &Daemon, socket: &Path, ready: &dyn Fn()) -> End {
    let Some(conn) = connect(socket).await else {
        return End::NotConnected;
    };
    let Ok(rule) = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .sender(LOGIND)
        .path_namespace(LOGIND_PATH)
        .build()
    else {
        return End::NotConnected;
    };
    // The subscription comes before the inhibitor: no signal between the two is lost.
    let Ok(mut signals) = MessageStream::for_match_rule(rule, &conn, Some(QUEUE)).await else {
        return End::NotConnected;
    };
    let mut inhibitor = inhibit(&conn).await;
    warning!("logind: vigiando bloqueio de tela e suspensão");
    ready();
    while let Some(Ok(msg)) = next_message(&mut signals).await {
        match event_from_signal(&msg) {
            None => {}
            Some(Event::WokeUp) => {
                if inhibitor.is_none() {
                    inhibitor = inhibit(&conn).await;
                }
            }
            Some(e) => {
                lock_vault(daemon, e.text());
                if e == Event::Suspend {
                    // Only after locking: the suspend waits for this fd to close.
                    drop(inhibitor.take());
                }
            }
        }
    }
    drop(inhibitor);
    End::Dropped
}

/// Thread that watches logind at `socket` forever. `ready` is called on each connection
/// opened (the tests wait for it).
pub fn watch_logind(
    daemon: Arc<Daemon>,
    socket: PathBuf,
    ready: Box<dyn Fn() + Send>,
) -> io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("wardd-logind".into())
        .spawn(move || {
            let mut wait = Duration::from_secs(1);
            let mut warned = false;
            loop {
                match zbus::block_on(session(&daemon, &socket, &*ready)) {
                    End::Dropped => {
                        lock_vault(&daemon, "conexão com o logind perdida");
                        wait = Duration::from_secs(1);
                        warned = false;
                    }
                    End::NotConnected if !warned => {
                        warning!(
                            "logind: barramento do sistema indisponível; só a trava por \
                             inatividade vale até conectar"
                        );
                        warned = true;
                    }
                    End::NotConnected => {}
                }
                std::thread::sleep(wait);
                wait = (wait * 2).min(MAX_BUS_WAIT);
            }
        })
}

/// Thread that checks the inactivity every `interval`, forever.
pub fn watch_inactivity(daemon: Arc<Daemon>, interval: Duration) -> io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("wardd-inatividade".into())
        .spawn(move || {
            loop {
                std::thread::sleep(interval);
                daemon.lock_if_idle();
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ward_core::protocol::ResponseError;

    const H: u64 = 3600;

    #[test]
    fn expired_at_the_limit_and_fails_closed() {
        let s = Duration::from_secs;
        assert!(!expired(Some(s(0)), Some(s(4 * H - 60)), 240));
        assert!(expired(Some(s(0)), Some(s(4 * H)), 240));
        assert!(!expired(Some(s(3 * H)), Some(s(7 * H - 1)), 240));
        // Unreadable clock or one that went back: locks.
        assert!(expired(None, Some(s(1)), 240));
        assert!(expired(Some(s(1)), None, 240));
        assert!(expired(Some(s(10)), Some(s(9)), 240));
        // Time from the settings.
        assert!(expired(Some(s(0)), Some(s(60)), 1));
        assert!(!expired(Some(s(0)), Some(s(59)), 1));
    }

    #[test]
    fn status_lock_and_error_do_not_count_as_use() {
        let list = Response::List(vec![]);
        for p in [Role::Agent, Role::App, Role::Extension] {
            assert!(!counts_as_use(p, Operation::Status, &Response::Ok));
            assert!(!counts_as_use(p, Operation::Lock, &Response::Ok));
            assert!(!counts_as_use(p, Operation::Unlock, &Response::Ok));
            assert!(!counts_as_use(p, Operation::OpenApp, &Response::Ok));
        }
        assert!(!counts_as_use(
            Role::App,
            Operation::List,
            &Response::Error(ResponseError::Locked)
        ));
        assert!(!counts_as_use(
            Role::App,
            Operation::SetLevel,
            &Response::Error(ResponseError::WrongPin)
        ));
        assert!(counts_as_use(Role::App, Operation::List, &list));
        assert!(counts_as_use(Role::App, Operation::Reveal, &Response::Ok));
    }

    /// Phase 54: a query from the agent and from the extension does not delay the lock;
    /// a delivery does.
    #[test]
    fn attack_list_and_suggest_do_not_count_as_use() {
        let list = Response::List(vec![]);
        assert!(!counts_as_use(Role::Agent, Operation::List, &list));
        assert!(!counts_as_use(Role::Agent, Operation::Stdin, &Response::Ok));
        assert!(!counts_as_use(
            Role::Agent,
            Operation::Resize,
            &Response::Ok
        ));
        assert!(!counts_as_use(
            Role::Extension,
            Operation::Suggest,
            &Response::Ok
        ));
        assert!(!counts_as_use(
            Role::Extension,
            Operation::AwaitLogin,
            &Response::Ok
        ));
        assert!(!counts_as_use(
            Role::Extension,
            Operation::LoginResult,
            &Response::Ok
        ));
        assert!(counts_as_use(Role::Agent, Operation::Run, &Response::Ok));
        assert!(counts_as_use(
            Role::Extension,
            Operation::Fill,
            &Response::Ok
        ));
        assert!(counts_as_use(
            Role::Extension,
            Operation::FillLogin,
            &Response::Ok
        ));
    }

    /// Phase 55: what the approval window and the form ask for on their own does not
    /// count.
    #[test]
    fn attack_f55_detail_totp_and_window_answer_do_not_count() {
        for op in [
            Operation::Detail,
            Operation::Totp,
            Operation::AnswerApproval,
        ] {
            assert!(!counts_as_use(Role::App, op, &Response::Ok), "{op:?}");
        }
        assert!(counts_as_use(Role::App, Operation::Copy, &Response::Ok));
        assert!(counts_as_use(
            Role::App,
            Operation::SaveEntry,
            &Response::Ok
        ));
    }

    #[test]
    fn attack_strange_bus_path() {
        for bad in [
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
            assert!(bus_path(Path::new(bad)).is_err(), "{bad}");
        }
        let long = format!("/{}", "a".repeat(MAX_PATH));
        assert!(bus_path(Path::new(&long)).is_err());
        assert!(bus_path(Path::new("/run/user/1000/ward-test/bus-1")).is_ok());
    }
}
