//! D63 (Phase 23a), through the binary: the test daemon with `WARD_DEV_APP` opens the
//! test app through the open-app operation and only that process, its child, gets the app role
//! (without the group's egid); the same binary run by hand, a copy of it and any client
//! stay agent; the open-app operation with a field in the request opens nothing; the app's
//! environment is the minimal fixed one; the output goes to `.dev/logs/<binary>.log`
//! (0600) and the daemon log records the exit code, only the number (V3.36, V3.37,
//! V3.38).
//!
//! The "repository" is a folder of this test only in `target/tmp`, with `.dev/` inside
//! and the test app linked by a hard link (the same file, with no write fd). No vault:
//! the requests do not need one.

// m2:Phase 54a: without `test-mode` the binaries this test runs are the release ones,
// which only know the system socket (`/run/ward/<uid>/wardd.sock`, the installed
// daemon). The build fails, loudly, instead of a test reaching it.
#[cfg(not(feature = "test-mode"))]
compile_error!(
    "este teste exige a feature test-mode (cargo test --all-features ou --features test-mode): sem ela os binários do teste são os de release, que só conhecem o socket do sistema"
);

mod common;

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::Dir;
use ward_core::protocol::{self, Response, ResponseError, Role};

const WARDD: &str = env!("CARGO_BIN_EXE_wardd");
const TEST_APP: &str = env!("CARGO_BIN_EXE_wardd-app-teste");

struct SocketDir(PathBuf);

impl SocketDir {
    fn new(label: &str) -> SocketDir {
        let p = PathBuf::from(format!(
            "/run/user/{}/wardd-app-{}-{label}",
            ward_platform::effective_uid(),
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        SocketDir(p)
    }
}

impl Drop for SocketDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Wardd(Child);

impl Drop for Wardd {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn send(s: &mut UnixStream, json: &str) {
    s.write_all(&(json.len() as u32).to_be_bytes())
        .and_then(|()| s.write_all(json.as_bytes()))
        .unwrap();
}

fn request(s: &mut UnixStream, op: &str, data: &str) -> Response {
    send(s, &format!(r#"{{"v":2,"op":"{op}","data":{data}}}"#));
    let mut n = [0u8; 4];
    s.read_exact(&mut n).unwrap();
    let mut b = vec![0u8; u32::from_be_bytes(n) as usize];
    s.read_exact(&mut b).unwrap();
    protocol::decode_response(Role::Agent, &b).unwrap()
}

fn wait_for(mut f: impl FnMut() -> bool, what: &str) {
    let end = Instant::now() + Duration::from_secs(20);
    while !f() {
        assert!(Instant::now() < end, "esperando {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn read(p: &Path) -> String {
    fs::read_to_string(p).unwrap_or_default()
}

/// The test app run by hand (child of this test, not of the daemon). A freshly written
/// copy can give ETXTBSY if another test forks at that instant: it tries again.
fn by_hand(binary: &Path, sock: &Path) -> String {
    for _ in 0..50 {
        match Command::new(binary)
            .env_clear()
            .env("WARD_DEV_SOCKET_DIR", sock)
            .stdin(Stdio::null())
            .output()
        {
            Ok(o) => return String::from_utf8(o.stdout).unwrap(),
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => panic!("{e}"),
        }
    }
    panic!("ETXTBSY o tempo todo");
}

#[test]
fn v3_37_only_the_app_opened_by_the_test_daemon_is_app() {
    let repo = Dir::new("repo-app");
    let dev = repo.0.join(".dev");
    fs::DirBuilder::new().mode(0o700).create(&dev).unwrap();
    fs::DirBuilder::new()
        .mode(0o700)
        .create(repo.0.join("bin"))
        .unwrap();
    let app = repo.0.join("bin/wardd-app-teste");
    fs::hard_link(TEST_APP, &app).unwrap();
    let sock = SocketDir::new("v337");
    let log = dev.join("wardd.log");

    // Outside the repository, the daemon does not even start.
    let st = Command::new(WARDD)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("WARD_DEV_DIR", &dev)
        .env("WARD_DEV_SOCKET_DIR", &sock.0)
        .env("WARD_DEV_APP", "/usr/bin/true")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(!st.success(), "WARD_DEV_APP fora do repositório aceito");

    let _w = Wardd(
        Command::new(WARDD)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C.UTF-8")
            .env("WARD_DEV_DIR", &dev)
            .env("WARD_DEV_SOCKET_DIR", &sock.0)
            .env("WARD_DEV_APP", &app)
            .env("RUST_LOG", "trace")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(fs::File::create(&log).unwrap())
            .spawn()
            .unwrap(),
    );
    let socket = sock.0.join("wardd.sock");
    wait_for(|| socket.exists(), "o socket do daemon");
    let mut c = UnixStream::connect(&socket).unwrap();
    c.set_read_timeout(Some(Duration::from_secs(30))).unwrap();

    // Any client (this test): agent.
    assert!(matches!(
        request(&mut c, "status", "{}"),
        Response::Status {
            role: Role::Agent,
            ..
        }
    ));
    // Open-app with a path, arguments or environment in the request: refused,
    // nothing opens.
    let app_output = dev.join("logs/wardd-app-teste.log");
    for data in [
        r#"{"path":"/usr/bin/sh"}"#,
        r#"{"argumentos":["-c","id"]}"#,
        r#"{"environment":{"LD_PRELOAD":"/tmp/x.so"}}"#,
    ] {
        let r = request(&mut c, "open_app", data);
        assert!(matches!(r, Response::Error(_)), "{data}: {r:?}");
    }
    std::thread::sleep(Duration::from_millis(300));
    assert!(!app_output.exists(), "abrir_app com campo abriu o app");

    // Without a field: opens `WARD_DEV_APP`, which gets the app role.
    assert!(matches!(request(&mut c, "open_app", "{}"), Response::Ok));
    wait_for(|| read(&app_output).contains("revelar="), "a saída do app");
    let text = read(&app_output);
    assert!(text.contains("papel=app\n"), "{text}");
    assert!(text.contains("argumentos=0\n"), "{text}");
    assert!(
        text.contains("ambiente=HOME,LANG,PATH,WARD_DEV_DIR,WARD_DEV_SOCKET_DIR,XDG_RUNTIME_DIR\n"),
        "{text}"
    );
    assert_eq!(
        fs::symlink_metadata(&app_output).unwrap().mode() & 0o777,
        0o600
    );
    // V3.38: the daemon log has the exit code, only the number.
    wait_for(
        || read(&log).contains(") saiu com código 0"),
        "o código de saída no log",
    );

    // By hand (the same file) and a copy: agent, and reveal not authorized.
    let not_authorized = ResponseError::Unauthorized.code();
    let copy = repo.0.join("bin/copia-do-app");
    fs::copy(&app, &copy).unwrap();
    for b in [&app, &copy] {
        let output = by_hand(b, &sock.0);
        assert!(
            output.contains("papel=agent\n"),
            "{}: {output}",
            b.display()
        );
        assert!(
            output.contains(&format!("revelar={not_authorized}\n")),
            "{}: {output}",
            b.display()
        );
    }
    let r = request(
        &mut c,
        "reveal",
        r#"{"entry":"acme-pg-dev","alvo":{"field":"password"}}"#,
    );
    assert!(
        matches!(r, Response::Error(ResponseError::Unauthorized)),
        "{r:?}"
    );

    // The app exited: opened again, it is app again (the new pid is the launcher's).
    assert!(matches!(request(&mut c, "open_app", "{}"), Response::Ok));
    wait_for(
        || read(&app_output).matches("papel=app\n").count() == 2,
        "a segunda abertura",
    );
    let daemon = read(&log);
    assert!(!daemon.contains("falso-"), "valor no log do daemon");
}
