//! Automatic lock (Phase 24, V3.19): inactivity with an injected clock, read from the
//! lock timeout in the vault settings, and logind (`Lock`, `LockedHint`,
//! `PrepareForSleep`) on a **fake bus**: a private `dbus-daemon`, for this test only, in a
//! 0700 directory of `$XDG_RUNTIME_DIR`, with a fake logind built with `zbus` itself.
//! Never the system bus nor the real logind.
//!
//! Attacks (all fail on the attacker's side):
//! - an agent that queries the status or sends failing requests does not keep the vault
//!   open;
//! - a clock that goes back or cannot be read locks;
//! - a bus that drops with the vault open locks;
//! - a signal forged by someone who is not logind neither unlocks nor undoes the lock;
//! - suspend does not get ahead of the lock (the inhibitor is only released after).

mod common;

use std::collections::HashMap;
use std::fs;
use std::io::{PipeReader, Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::*;
use ward_core::protocol::{self, ResponseError, Resposta, Role};
use wardd::core::{Clock, Daemon};
use zbus::Connection;
use zbus::Variant as Value;
use zbus::connection::Builder;

/// zbus `emit_signal`, awaited right here (the test has no runtime of its own).
macro_rules! emit {
    ($c:expr, $($a:expr),+ $(,)?) => {
        zbus::block_on($c.emit_signal(None::<&str>, $($a),+)).unwrap()
    };
}

const H: u64 = 3600;
const LOGIND: &str = "org.freedesktop.login1";
const SESSION: &str = "/org/freedesktop/login1/session/_32";
const SESSION_IFACE: &str = "org.freedesktop.login1.Session";
const WARDD: &str = env!("CARGO_BIN_EXE_wardd");

// ---------------------------------------------------------------- injected clock

/// Manual clock: the reading returns what the test set (`None` = unreadable); the sleep
/// only records the D20 wait.
#[derive(Clone)]
struct ManualClock(Arc<Mutex<Option<Duration>>>, Arc<Mutex<Vec<Duration>>>);

impl ManualClock {
    fn new() -> Self {
        ManualClock(Arc::new(Mutex::new(Some(Duration::ZERO))), Arc::default())
    }
    fn waits(&self) -> Vec<Duration> {
        std::mem::take(&mut *self.1.lock().unwrap())
    }
    fn set(&self, secs: u64) {
        *self.0.lock().unwrap() = Some(Duration::from_secs(secs));
    }
    fn unreadable(&self) {
        *self.0.lock().unwrap() = None;
    }
}

impl Clock for ManualClock {
    fn sleep(&self, d: Duration) {
        self.1.lock().unwrap().push(d);
    }
    fn now(&self) -> Option<Duration> {
        *self.0.lock().unwrap()
    }
}

/// New test vault (unlocked at t = 0) in `dir`, with the manual clock.
fn vault_with_clock(dir: &Dir) -> (Daemon, ManualClock) {
    let r = ManualClock::new();
    let d = Daemon::new(dir.paths(), Box::new(r.clone()));
    let create = request(
        &d,
        Role::App,
        "criar_cofre",
        &format!(
            r#"{{"senha":"{}","pin":"{}"}}"#,
            b64(PASSWORD.as_bytes()),
            b64(PIN.as_bytes())
        ),
    );
    assert!(ok(&create), "{create:?}");
    let s = request(
        &d,
        Role::App,
        "salvar_entrada",
        &login("t-aprov", SECRETS[0]),
    );
    assert!(ok(&s), "{s:?}");
    (d, r)
}

/// An app action (the list of the Credentials screen): counts as use (Phase 54).
fn list_ok(d: &Daemon) {
    let r = request(d, Role::App, "lista", "{}");
    assert!(matches!(r, Resposta::Lista(_)), "{r:?}");
}

#[test]
fn v3_19_inactivity_with_injected_clock() {
    let dir = Dir::new("inat");
    let (d, r) = vault_with_clock(&dir);
    // Settings default (D30): 240 min.
    r.set(4 * H - 60);
    assert!(!d.lock_if_idle(), "3h59 sem uso não trava");
    assert!(!d.is_locked());
    r.set(4 * H);
    assert!(d.lock_if_idle(), "4h00 sem uso trava");
    assert!(d.is_locked());
    assert_eq!(
        error(&request(&d, Role::Agent, "lista", "{}")),
        Some(ResponseError::Locked)
    );
    // Locked, nothing more to do.
    assert!(!d.lock_if_idle());

    // A use at 3h resets the count.
    let t0 = 10 * H;
    r.set(t0);
    assert!(ok(&unlock(&d, PASSWORD)));
    r.set(t0 + 3 * H);
    list_ok(&d);
    r.set(t0 + 4 * H);
    assert!(!d.lock_if_idle(), "o uso às 3h zerou");
    r.set(t0 + 3 * H + 4 * H - 1);
    assert!(!d.lock_if_idle());
    r.set(t0 + 7 * H);
    assert!(d.lock_if_idle());
}

#[test]
fn v3_19_timeout_read_from_the_vault_settings() {
    let dir = Dir::new("cfg");
    let (d, r) = vault_with_clock(&dir);
    let t = request(&d, Role::App, "tempo_trava", r#"{"minutos":30}"#);
    assert!(ok(&t), "{t:?}");
    r.set(30 * 60 - 1);
    assert!(!d.lock_if_idle());
    r.set(30 * 60);
    assert!(d.lock_if_idle(), "30 min da configuração");
    // Holds after reopening: the timeout is in the vault, not in memory.
    let d2 = Daemon::new(dir.paths(), Box::new(r.clone()));
    r.set(100 * H);
    assert!(ok(&unlock(&d2, PASSWORD)));
    r.set(100 * H + 29 * 60);
    assert!(!d2.lock_if_idle());
    r.set(100 * H + 30 * 60);
    assert!(d2.lock_if_idle());
}

#[test]
fn attack_agent_does_not_keep_the_vault_open_with_status_or_errors() {
    let dir = Dir::new("segura");
    let (d, r) = vault_with_clock(&dir);
    // For 4 h, every 10 min: status, a missing entry, a repeated unlock, an app request
    // by the agent and, four times, a wrong PIN (the fifth would lock by S9).
    let mut t = 0;
    while t < 4 * H {
        r.set(t);
        assert!(matches!(
            request(&d, Role::Agent, "estado", "{}"),
            Resposta::Estado { travado: false, .. }
        ));
        assert_eq!(
            error(&request(
                &d,
                Role::Agent,
                "executar",
                &run_request("nao-existe")
            )),
            Some(ResponseError::EntryNotFound)
        );
        // Unlocking with the vault already open (any password) is not use.
        assert!(ok(&unlock(&d, "qualquer-coisa")));
        assert!(
            error(&request(
                &d,
                Role::Agent,
                "detalhe",
                r#"{"entrada":"t-aprov"}"#
            ))
            .is_some()
        );
        if t % 2400 == 0 && t / 2400 < 4 {
            let m = set_level(&d, Role::App, "t-aprov", "livre", WRONG_PIN);
            assert_eq!(error(&m), Some(ResponseError::WrongPin));
        }
        assert!(!d.lock_if_idle(), "t = {t}");
        t += 600;
    }
    r.set(4 * H);
    assert!(d.lock_if_idle(), "nenhum desses pedidos conta como uso");
}

/// Phase 54 (final audit): the agent that lists the entries every 10 min (and the
/// extension that suggests on every page) does not delay the lock; at 4h00 the vault
/// locks. An app action does reset the count.
#[test]
fn attack_periodic_agent_list_does_not_delay_the_lock() {
    let dir = Dir::new("lista54");
    let (d, r) = vault_with_clock(&dir);
    let mut t = 0;
    while t < 4 * H {
        r.set(t);
        let l = request(&d, Role::Agent, "lista", "{}");
        assert!(matches!(l, Resposta::Lista(_)), "{l:?}");
        let s = request(
            &d,
            Role::Extension,
            "sugerir",
            r#"{"origem":"https://wp.acme.test"}"#,
        );
        assert!(!matches!(s, Resposta::Erro(ResponseError::Locked)), "{s:?}");
        assert!(!d.lock_if_idle(), "t = {t}");
        t += 600;
    }
    r.set(4 * H);
    assert!(
        d.lock_if_idle(),
        "a lista do agente e o sugerir seguraram o cofre aberto"
    );

    // Control: with an app action at 3h, it does not lock at 4h00.
    let t0 = 10 * H;
    r.set(t0);
    assert!(ok(&unlock(&d, PASSWORD)));
    r.set(t0 + 3 * H);
    list_ok(&d);
    r.set(t0 + 4 * H);
    assert!(!d.lock_if_idle(), "a ação do app zerou");
}

/// Phase 55 (final re-audit): an agent `ward run` repeated every 10 min on an entry that
/// asks for approval, denied in the window, with the entry detail the window asks for
/// and the TOTP code the form renews, does not delay the lock: at 4h00 the vault locks.
#[test]
fn attack_f55_denied_ward_run_and_window_do_not_delay_the_lock() {
    let dir = Dir::new("negado55");
    let (d, r) = vault_with_clock(&dir);
    let totp = format!(
        r#"{{"entrada":{{"nome":"t-totp","grupo":"clientex","ambiente":"dev","tipo":"login","campos":{{"url":"https://clientex.invalid/login","usuario":"admin","senha":"{}","totp_semente":"{}"}}}}}}"#,
        b64(SECRETS[1].as_bytes()),
        b64(b"JBSWY3DPEHPK3PXP")
    );
    assert!(ok(&request(&d, Role::App, "salvar_entrada", &totp)));
    let mut t = 0;
    while t < 4 * H {
        r.set(t);
        let (denied, seen) = with_window(&d, "negar", None, || {
            let p = req(Role::Agent, "executar", &run_request("t-aprov")).unwrap();
            d.authorize_use(&wardd::approval::NoClient, &p)
        });
        assert_eq!(denied.err(), Some(ResponseError::Denied));
        assert_eq!(seen.len(), 1);
        // What the window asks for to show the request, and the open form.
        let det = request(&d, Role::App, "detalhe", r#"{"entrada":"t-aprov"}"#);
        assert!(matches!(det, Resposta::Detalhe(_)), "{det:?}");
        let c = request(&d, Role::App, "totp", r#"{"entrada":"t-totp"}"#);
        assert!(matches!(c, Resposta::CodigoTotp { .. }), "{c:?}");
        assert!(!d.lock_if_idle(), "t = {t}");
        t += 600;
    }
    r.set(4 * H);
    assert!(
        d.lock_if_idle(),
        "o ward run negado, o detalhe ou o totp seguraram o cofre aberto"
    );
}

fn run_request(entry: &str) -> String {
    format!(
        r#"{{"entrada":"{entry}","argv":["{}"],"cwd":"{}","tipo_entrada":"pipe","ambiente":[]}}"#,
        b64(b"true"),
        b64(b"/")
    )
}

/// D62: the automatic lock does not touch the D20 wait counter (it neither resets the
/// wait of whoever got the password wrong nor creates a wait for the user).
#[test]
fn d62_auto_lock_does_not_touch_the_d20_wait() {
    let dir = Dir::new("d20");
    let (d, r) = vault_with_clock(&dir);
    r.set(4 * H);
    assert!(d.lock_if_idle());
    r.waits();
    // After the automatic lock, the first attempt does not wait.
    assert_eq!(
        error(&unlock(&d, "senha-errada")),
        Some(ResponseError::WrongPassword)
    );
    assert_eq!(
        error(&unlock(&d, "senha-errada")),
        Some(ResponseError::WrongPassword)
    );
    assert_eq!(r.waits(), [Duration::ZERO, Duration::from_secs(1)]);
    // Locked, the inactivity check does not reset the two failures: the next waits 2 s.
    r.set(100 * H);
    assert!(!d.lock_if_idle());
    assert!(ok(&unlock(&d, PASSWORD)));
    assert_eq!(r.waits(), [Duration::from_secs(2)]);
}

#[test]
fn attack_clock_that_goes_back_or_is_unreadable_locks() {
    let dir = Dir::new("relogio");
    let (d, r) = vault_with_clock(&dir);
    r.set(2 * H);
    list_ok(&d);
    r.set(H);
    assert!(d.lock_if_idle(), "relógio voltou: trava");

    r.set(5 * H);
    assert!(ok(&unlock(&d, PASSWORD)));
    r.unreadable();
    assert!(d.lock_if_idle(), "relógio ilegível: trava");

    // A use recorded without a clock also locks on the next check.
    r.set(6 * H);
    assert!(ok(&unlock(&d, PASSWORD)));
    r.unreadable();
    let _ = request(&d, Role::App, "lista", "{}");
    r.set(6 * H + 1);
    assert!(d.lock_if_idle());
}

// ---------------------------------------------------------------- fake bus

static N: AtomicU32 = AtomicU32::new(0);

/// Private `dbus-daemon`, for this test only (killed on `Drop`).
struct FakeBus {
    dir: PathBuf,
    socket: PathBuf,
    child: Option<Child>,
}

impl FakeBus {
    fn start() -> FakeBus {
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| {
                PathBuf::from(format!("/run/user/{}", ward_platform::effective_uid()))
            });
        let dir = base.join(format!(
            "wardd-bus-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        let socket = dir.join("bus");
        let config = dir.join("bus.conf");
        fs::write(
            &config,
            format!(
                r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <listen>unix:path={}</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#,
                socket.display()
            ),
        )
        .unwrap();
        let child = Command::new("/usr/bin/dbus-daemon")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--nopidfile", "--nosyslog"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("dbus-daemon");
        let deadline = Instant::now() + Duration::from_secs(10);
        while UnixStream::connect(&socket).is_err() {
            assert!(Instant::now() < deadline, "o dbus-daemon falso não subiu");
            std::thread::sleep(Duration::from_millis(20));
        }
        FakeBus {
            dir,
            socket,
            child: Some(child),
        }
    }

    fn address(&self) -> String {
        format!("unix:path={}", self.socket.display())
    }

    fn client(&self) -> Connection {
        zbus::block_on(Builder::address(self.address().as_str()).build()).unwrap()
    }

    fn kill(&mut self) {
        if let Some(mut f) = self.child.take() {
            let _ = f.kill();
            let _ = f.wait();
        }
    }
}

impl Drop for FakeBus {
    fn drop(&mut self) {
        self.kill();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Inhibitors handed out by the fake logind: (what, mode, read end of the pipe).
type Inhibitors = Arc<Mutex<Vec<(String, String, PipeReader)>>>;

struct Manager {
    inhibitors: Inhibitors,
}

#[zbus::interface(name = "org.freedesktop.login1.Manager")]
impl Manager {
    fn inhibit(
        &self,
        what: &str,
        _who: &str,
        _why: &str,
        mode: &str,
    ) -> zbus::fdo::Result<zbus::OwnedFd> {
        let (r, w) = std::io::pipe().map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        self.inhibitors
            .lock()
            .unwrap()
            .push((what.to_owned(), mode.to_owned(), r));
        Ok(OwnedFd::from(w).into())
    }
}

/// The fake logind: owner of `org.freedesktop.login1` on the fake bus.
struct FakeLogind {
    conn: Connection,
    inhibitors: Inhibitors,
}

impl FakeLogind {
    fn new(b: &FakeBus) -> FakeLogind {
        let inhibitors = Inhibitors::default();
        let conn = zbus::block_on(
            Builder::address(b.address().as_str())
                .name(LOGIND)
                .serve_at(
                    "/org/freedesktop/login1",
                    Manager {
                        inhibitors: inhibitors.clone(),
                    },
                )
                .build(),
        )
        .unwrap();
        FakeLogind { conn, inhibitors }
    }

    fn lock(&self) {
        emit!(self.conn, SESSION, SESSION_IFACE, "Lock", &());
    }

    fn locked_hint(&self, value: bool) {
        let mut changed: HashMap<&str, Value<'_>> = HashMap::new();
        changed.insert("LockedHint", Value::Bool(value));
        emit!(
            self.conn,
            SESSION,
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
            &(SESSION_IFACE, changed, Vec::<&str>::new()),
        );
    }

    fn prepare_for_sleep(&self, value: bool) {
        emit!(
            self.conn,
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
            "PrepareForSleep",
            &value,
        );
    }

    /// Waits for the `n`-th inhibitor and returns a channel that receives when it closes.
    fn inhibitor(&self, n: usize) -> mpsc::Receiver<usize> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            {
                let mut v = self.inhibitors.lock().unwrap();
                if v.len() > n {
                    let (what, mode, r) = &mut v[n];
                    assert_eq!((what.as_str(), mode.as_str()), ("sleep", "delay"));
                    let mut r = r.try_clone().unwrap();
                    let (tx, rx) = mpsc::channel();
                    std::thread::spawn(move || {
                        let mut b = [0u8; 1];
                        let _ = tx.send(r.read(&mut b).unwrap_or(0));
                    });
                    return rx;
                }
            }
            assert!(
                Instant::now() < deadline,
                "o daemon não pediu o inibidor {n}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// Test daemon with the logind watcher connected to the fake bus.
fn watched_daemon(dir: &Dir, b: &FakeBus) -> (Arc<Daemon>, mpsc::Receiver<()>) {
    let (d, _r) = test_vault(dir);
    let d = Arc::new(d);
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    wardd::lock::watch_logind(
        d.clone(),
        b.socket.clone(),
        Box::new(move || {
            let _ = tx.lock().unwrap().send(());
        }),
    )
    .unwrap();
    rx.recv_timeout(Duration::from_secs(10))
        .expect("o vigia não conectou ao barramento falso");
    (d, rx)
}

fn wait_locked(d: &Daemon, limit: Duration) -> Duration {
    let start = Instant::now();
    while !d.is_locked() {
        assert!(
            start.elapsed() < limit,
            "não travou em {limit:?} (V3.19: até 2 s)"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    start.elapsed()
}

#[test]
fn v3_19_lock_and_locked_hint_lock_within_2s() {
    let b = FakeBus::start();
    let logind = FakeLogind::new(&b);
    let dir = Dir::new("lock");
    let (d, _ready) = watched_daemon(&dir, &b);
    assert!(!d.is_locked());

    // `loginctl lock-session` → `Lock` on the session.
    logind.lock();
    wait_locked(&d, Duration::from_secs(2));

    // `LockedHint` false does not lock; true locks.
    assert!(ok(&unlock(&d, PASSWORD)));
    logind.locked_hint(false);
    std::thread::sleep(Duration::from_millis(300));
    assert!(!d.is_locked(), "LockedHint falso não trava");
    logind.locked_hint(true);
    wait_locked(&d, Duration::from_secs(2));
}

#[test]
fn v3_19_prepare_for_sleep_locks_before_releasing_the_inhibitor() {
    let b = FakeBus::start();
    let logind = FakeLogind::new(&b);
    let dir = Dir::new("sleep");
    let (d, _ready) = watched_daemon(&dir, &b);
    let closed = logind.inhibitor(0);
    assert!(
        closed.recv_timeout(Duration::from_millis(300)).is_err(),
        "o inibidor fica aberto enquanto não há suspensão"
    );
    assert!(!d.is_locked());
    logind.prepare_for_sleep(true);
    closed
        .recv_timeout(Duration::from_secs(2))
        .expect("o inibidor não foi solto");
    // When logind sees the fd close, the vault is already locked.
    assert!(d.is_locked(), "a suspensão passou à frente da trava");

    // On wake-up, the daemon takes another inhibitor; the next suspend locks again.
    logind.prepare_for_sleep(false);
    let closed2 = logind.inhibitor(1);
    assert!(ok(&unlock(&d, PASSWORD)));
    logind.prepare_for_sleep(true);
    closed2.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(d.is_locked());
}

#[test]
fn attack_bus_drops_with_the_vault_open_locks() {
    let mut b = FakeBus::start();
    let _logind = FakeLogind::new(&b);
    let dir = Dir::new("cai");
    let (d, _ready) = watched_daemon(&dir, &b);
    assert!(!d.is_locked());
    b.kill();
    wait_locked(&d, Duration::from_secs(2));
}

#[test]
fn attack_forged_signal_neither_unlocks_nor_holds() {
    let b = FakeBus::start();
    let logind = FakeLogind::new(&b);
    let dir = Dir::new("forja");
    let (d, _ready) = watched_daemon(&dir, &b);
    // Any process on the bus (not the owner of org.freedesktop.login1).
    let forger = b.client();
    logind.lock();
    wait_locked(&d, Duration::from_secs(2));
    for (path, iface, member) in [
        (SESSION, SESSION_IFACE, "Unlock"),
        (
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
            "PrepareForSleep",
        ),
    ] {
        if member == "PrepareForSleep" {
            emit!(forger, path, iface, member, &false);
        } else {
            emit!(forger, path, iface, member, &());
        }
    }
    let mut changed: HashMap<&str, Value<'_>> = HashMap::new();
    changed.insert("LockedHint", Value::Bool(false));
    emit!(
        forger,
        SESSION,
        "org.freedesktop.DBus.Properties",
        "PropertiesChanged",
        &(SESSION_IFACE, changed, Vec::<&str>::new()),
    );
    std::thread::sleep(Duration::from_millis(300));
    assert!(d.is_locked(), "nada do barramento destrava");
    assert_eq!(
        error(&request(&d, Role::Agent, "lista", "{}")),
        Some(ResponseError::Locked)
    );
}

// ---------------------------------------------------------------- binary

fn send(s: &mut UnixStream, json: &str) {
    s.write_all(&(json.len() as u32).to_be_bytes()).unwrap();
    s.write_all(json.as_bytes()).unwrap();
}

fn request_over(s: &mut UnixStream, op: &str, data: &str) -> Resposta {
    send(s, &format!(r#"{{"v":1,"op":"{op}","dados":{data}}}"#));
    let mut n = [0u8; 4];
    s.read_exact(&mut n).unwrap();
    let mut q = vec![0u8; u32::from_be_bytes(n) as usize];
    s.read_exact(&mut q).unwrap();
    protocol::decode_response(Role::Agent, &q).unwrap()
}

struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_in_log(log: &Path, text: &str) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if fs::read_to_string(log).is_ok_and(|t| t.contains(text)) {
            return;
        }
        assert!(Instant::now() < deadline, "o log não mostrou {text:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn v3_19_binary_locks_on_lock_session_of_the_fake_logind() {
    let b = FakeBus::start();
    let logind = FakeLogind::new(&b);
    let dev = Dir::new("bin");
    // The test vault is created by the core and then served by the binary.
    drop(test_vault(&dev));
    let sock = b.dir.join("sock");
    let log = dev.0.join("wardd.log");
    let _w = KillOnDrop(
        Command::new(WARDD)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("WARD_DEV_DIR", &dev.0)
            .env("WARD_DEV_SOCKET_DIR", &sock)
            .env("WARD_DEV_DBUS_SISTEMA", &b.socket)
            .env("RUST_LOG", "trace")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(fs::File::create(&log).unwrap())
            .spawn()
            .unwrap(),
    );
    wait_in_log(&log, "logind: vigiando");
    // The zbus runtime does not take the daemon's SIGCHLD (`ward run` and the app
    // launcher wait for their own children).
    let status = fs::read_to_string(format!("/proc/{}/status", _w.0.id())).unwrap();
    let cgt = status
        .lines()
        .find_map(|l| l.strip_prefix("SigCgt:"))
        .map(|v| u64::from_str_radix(v.trim(), 16).unwrap())
        .unwrap();
    assert_eq!(cgt & (1 << (17 - 1)), 0, "SIGCHLD capturado: {cgt:x}");
    let deadline = Instant::now() + Duration::from_secs(20);
    let path = sock.join(ward_platform::socket::SOCKET_NAME);
    while !path.exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut s = UnixStream::connect(&path).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(60))).unwrap();
    let r = request_over(
        &mut s,
        "destravar",
        &format!(r#"{{"senha":"{}"}}"#, b64(PASSWORD.as_bytes())),
    );
    assert!(ok(&r), "{r:?}");
    logind.lock();
    let start = Instant::now();
    loop {
        match request_over(&mut s, "estado", "{}") {
            Resposta::Estado { travado: true, .. } => break,
            Resposta::Estado { .. } => {}
            other => panic!("{other:?}"),
        }
        assert!(start.elapsed() < Duration::from_secs(2), "V3.19: até 2 s");
        std::thread::sleep(Duration::from_millis(20));
    }
    wait_in_log(&log, "tela bloqueada: cofre travado");
    // The log has no password and no secret (S13).
    let text = fs::read(&log).unwrap();
    for secret in [PASSWORD, PIN].iter().chain(SECRETS.iter()) {
        assert!(!contains(&text, secret.as_bytes()), "segredo no log");
    }
}

#[test]
fn test_daemon_without_fake_bus_does_not_enable_logind() {
    let dev = Dir::new("semdbus");
    let b = FakeBus::start(); // only to have a short socket directory
    let sock = b.dir.join("sock2");
    let log = dev.0.join("wardd.log");
    let _w = KillOnDrop(
        Command::new(WARDD)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("WARD_DEV_DIR", &dev.0)
            .env("WARD_DEV_SOCKET_DIR", &sock)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(fs::File::create(&log).unwrap())
            .spawn()
            .unwrap(),
    );
    wait_in_log(&log, "logind desligado");
    // Odd path in the variable: refuses to start.
    let log2 = dev.0.join("wardd2.log");
    let st = Command::new(WARDD)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("WARD_DEV_DIR", &dev.0)
        .env("WARD_DEV_SOCKET_DIR", b.dir.join("sock3"))
        .env("WARD_DEV_DBUS_SISTEMA", "/run/dbus/../x;unix:path=/tmp/x")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(fs::File::create(&log2).unwrap())
        .status()
        .unwrap();
    assert!(!st.success());
    assert!(
        fs::read_to_string(&log2)
            .unwrap()
            .contains("WARD_DEV_DBUS_SISTEMA recusado")
    );
}
