//! Support for the `wardd` tests: test directories, JSON requests, a fake clock and
//! a test vault of their own (fake values only; never the real vault nor the one in
//! `.dev/`).

#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ward_core::protocol::{self, Request, Response, ResponseError, Role};
use wardd::nucleo::{Caminhos, Daemon, Relogio};

pub const PASSWORD: &str = "falso-mestra-ward-01";
pub const PIN: &str = "135790";
pub const WRONG_PIN: &str = "246801";
/// Secrets of the test entries (fake).
pub const SECRETS: [&str; 4] = [
    "falso-clientex-01",
    "falso-servidor-ward-07",
    "falso-banco-ward-08",
    "falso-livre-ward-09",
];

/// Standard base64 (only to build the test frames).
pub fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                s.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

static N: AtomicU32 = AtomicU32::new(0);

/// 0700 directory of this test only, in `target/tmp` (deleted on `Drop`).
pub struct Dir(pub PathBuf);

impl Dir {
    pub fn new(label: &str) -> Dir {
        let p = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "wardd-{}-{label}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        Dir(p)
    }

    pub fn paths(&self) -> Caminhos {
        wardd::config::caminhos_de_teste(&self.0)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700));
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Clock that only records the waits.
#[derive(Clone, Default)]
pub struct FakeClock(pub Arc<Mutex<Vec<Duration>>>);

impl Relogio for FakeClock {
    fn dormir(&self, d: Duration) {
        self.0.lock().unwrap().push(d);
    }
}

pub fn daemon_at(c: Caminhos) -> (Daemon, FakeClock) {
    let r = FakeClock::default();
    (Daemon::novo(c, Box::new(r.clone())), r)
}

/// Reads the request as the socket would (the role is checked first).
pub fn req(role: Role, op: &str, data: &str) -> Result<Request, ResponseError> {
    protocol::decode_request(
        role,
        format!(r#"{{"v":1,"op":"{op}","dados":{data}}}"#).as_bytes(),
    )
}

/// Requests and returns the response (the request has to go through the reading).
pub fn request(d: &Daemon, role: Role, op: &str, data: &str) -> Response {
    match req(role, op, data) {
        Ok(p) => d.atender(role, p),
        Err(e) => Response::Error(e),
    }
}

pub fn error(r: &Response) -> Option<ResponseError> {
    match r {
        Response::Error(e) => Some(e.clone()),
        _ => None,
    }
}

pub fn ok(r: &Response) -> bool {
    matches!(r, Response::Ok)
}

pub fn unlock(d: &Daemon, password: &str) -> Response {
    request(
        d,
        Role::Agent,
        "destravar",
        &format!(r#"{{"senha":"{}"}}"#, b64(password.as_bytes())),
    )
}

pub fn login(name: &str, password: &str) -> String {
    format!(
        r#"{{"entrada":{{"nome":"{name}","grupo":"clientex","ambiente":"dev","tipo":"login","campos":{{"url":"https://clientex.invalid/login","usuario":"admin","senha":"{}"}}}}}}"#,
        b64(password.as_bytes())
    )
}

pub fn server(name: &str) -> String {
    format!(
        r#"{{"entrada":{{"nome":"{name}","grupo":"clientex","ambiente":"stage","tipo":"servidor","campos":{{"host":"ssh.clientex.invalid","porta":"22","usuario":"deploy","senha":"{}"}}}}}}"#,
        b64(SECRETS[1].as_bytes())
    )
}

pub fn database(name: &str, tunnel: &str) -> String {
    format!(
        r#"{{"entrada":{{"nome":"{name}","grupo":"clientex","ambiente":"prod","tipo":"banco","campos":{{"motor":"postgres","host":"127.0.0.1","porta":"5432","base":"app","usuario":"app","senha":"{}","tunel":"{tunnel}"}}}}}}"#,
        b64(SECRETS[2].as_bytes())
    )
}

pub fn set_level(d: &Daemon, role: Role, name: &str, level: &str, pin: &str) -> Response {
    request(
        d,
        role,
        "mudar_nivel",
        &format!(
            r#"{{"entrada":"{name}","nivel":"{level}","pin":"{}"}}"#,
            b64(pin.as_bytes())
        ),
    )
}

/// New test vault, unlocked, with: `t-aprov` (with approval), `t-livre` (free),
/// `t-bloq` (blocked), the server `s1` and the database `b1` with a tunnel through `s1`.
pub fn test_vault(dir: &Dir) -> (Daemon, FakeClock) {
    let (d, r) = daemon_at(dir.paths());
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
    for q in [
        login("t-aprov", SECRETS[0]),
        login("t-livre", SECRETS[3]),
        login("t-bloq", SECRETS[0]),
        server("s1"),
        database("b1", "s1"),
    ] {
        let r = request(&d, Role::App, "salvar_entrada", &q);
        assert!(ok(&r), "{r:?}");
    }
    assert!(ok(&set_level(&d, Role::App, "t-livre", "livre", PIN)));
    assert!(ok(&set_level(&d, Role::App, "t-bloq", "bloqueado", PIN)));
    (d, r)
}

pub fn encoded(role: Role, r: &Response) -> Vec<u8> {
    protocol::encode_response(role, r).unwrap().to_vec()
}

pub fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && needle.len() <= haystack.len()
        && haystack.windows(needle.len()).any(|w| w == needle)
}

pub fn generations(dir: &Dir) -> wardd::arquivos::Geracoes {
    wardd::arquivos::ler_estado(&dir.paths().estado).unwrap()
}

/// Generation read from disk (opens with the test password, no floor).
pub fn generation_on_disk(path: &Path, password: &str) -> u64 {
    let bytes = fs::read(path).unwrap();
    let s = ward_core::format::MasterPassword::new(password.to_owned());
    ward_core::body::open_entries_with_generation(&bytes, &s, 0, 0)
        .unwrap()
        .2
}

/// Phase 44c (D9): fake approval window, in process (D8: the same path as the
/// socket, the approval answer with the app role). During `f`, each
/// approval request that the engine delivers is kept and answered with `decision`
/// (confirm, deny…; with `pin` when the decision asks for it). Returns the result
/// of `f` and the requests seen. The daemon's engine opens no app: with the window
/// subscribed, it delivers directly.
pub fn with_window<R>(
    d: &Daemon,
    decision: &str,
    pin: Option<&str>,
    f: impl FnOnce() -> R,
) -> (R, Vec<protocol::ApprovalRequest>) {
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;
    use wardd::aprovacao::Assinante;

    struct FakeWindow(mpsc::Sender<protocol::ApprovalRequest>);
    impl Assinante for FakeWindow {
        fn entregar(&mut self, r: &Response) -> bool {
            if let Response::ApprovalRequest(p) = r {
                return self.0.send(p.clone()).is_ok();
            }
            true
        }
    }

    const WINDOW_ID: u64 = 9_999;
    let (tx, rx) = mpsc::channel();
    d.motor().assinar(WINDOW_ID, Box::new(FakeWindow(tx)));
    let stop = AtomicBool::new(false);
    let seen = Mutex::new(Vec::new());
    let r = std::thread::scope(|s| {
        let (stop, seen) = (&stop, &seen);
        s.spawn(move || {
            while !stop.load(Ordering::Acquire) {
                let Ok(p) = rx.recv_timeout(Duration::from_millis(20)) else {
                    continue;
                };
                let pin = pin
                    .map(|p| format!(r#","pin":"{}""#, b64(p.as_bytes())))
                    .unwrap_or_default();
                let _ = request(
                    d,
                    Role::App,
                    "responder_aprovacao",
                    &format!(r#"{{"pedido":{},"decisao":"{decision}"{pin}}}"#, p.id),
                );
                seen.lock().unwrap().push(p);
            }
        });
        let r = f();
        stop.store(true, Ordering::Release);
        r
    });
    d.motor().desassinar(WINDOW_ID);
    (r, seen.into_inner().unwrap())
}
