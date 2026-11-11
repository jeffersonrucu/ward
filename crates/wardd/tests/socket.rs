//! The `wardd` binary (test mode) end to end through the socket: starts with a clean
//! environment, refuses `TMPDIR`/`LD_*` (V3.30, V3.25), serves the agent, withstands
//! stalled connections and a frame that is too big (V3.4), and the log with
//! `RUST_LOG=trace` has no password, PIN nor secret (V3.20, S8).
//!
//! The vault is a test one, created in this directory by the core itself; the socket
//! lives in a directory of this test only in `$XDG_RUNTIME_DIR` (the socket path does
//! not fit in `target/tmp`), deleted at the end.

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
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::*;
use ward_core::protocol::{self, Response, ResponseError, Role};

const WARDD: &str = env!("CARGO_BIN_EXE_wardd");

struct SocketDir(PathBuf);

impl SocketDir {
    fn new(label: &str) -> SocketDir {
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| {
                PathBuf::from(format!("/run/user/{}", ward_platform::effective_uid()))
            });
        let p = base.join(format!("wardd-t-{}-{label}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        SocketDir(p)
    }
}

impl Drop for SocketDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Wardd {
    child: Child,
    log: PathBuf,
    socket: PathBuf,
}

impl Drop for Wardd {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn command(dev: &Path, sock: &Path, log: &Path) -> Command {
    let mut c = Command::new(WARDD);
    c.env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("WARD_DEV_DIR", dev)
        .env("WARD_DEV_SOCKET_DIR", sock)
        .env("RUST_LOG", "trace")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(fs::File::create(log).unwrap());
    c
}

fn start(dev: &Dir, sock: &SocketDir) -> Wardd {
    let log = dev.0.join("wardd.log");
    let child = command(&dev.0, &sock.0, &log).spawn().unwrap();
    let socket = sock.0.join(ward_platform::socket::SOCKET_NAME);
    let deadline = Instant::now() + Duration::from_secs(20);
    while !socket.exists() {
        assert!(Instant::now() < deadline, "o wardd não criou o socket");
        std::thread::sleep(Duration::from_millis(20));
    }
    Wardd { child, log, socket }
}

fn connect(w: &Wardd) -> UnixStream {
    let s = UnixStream::connect(&w.socket).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
    s
}

fn send(s: &mut UnixStream, json: &str) {
    if let Err(e) = s
        .write_all(&(json.len() as u32).to_be_bytes())
        .and_then(|()| s.write_all(json.as_bytes()))
    {
        panic!("escrita no socket: {e}");
    }
}

fn receive(s: &mut UnixStream) -> Option<Vec<u8>> {
    let mut n = [0u8; 4];
    s.read_exact(&mut n).ok()?;
    let mut b = vec![0u8; u32::from_be_bytes(n) as usize];
    s.read_exact(&mut b).ok()?;
    Some(b)
}

fn request_on(s: &mut UnixStream, op: &str, data: &str) -> (Vec<u8>, Response) {
    send(s, &format!(r#"{{"v":2,"op":"{op}","data":{data}}}"#));
    let b = receive(s).expect("sem resposta");
    let r = protocol::decode_response(Role::Agent, &b).unwrap();
    (b, r)
}

fn password_json(s: &str) -> String {
    format!(r#"{{"password":"{}"}}"#, b64(s.as_bytes()))
}

#[test]
fn v3_30_env_with_tmpdir_or_ld_refuses_to_start() {
    let dev = Dir::new("env");
    for (k, v) in [
        ("TMPDIR", "/tmp"),
        ("LD_LIBRARY_PATH", "/nao-existe"),
        ("LD_PRELOAD", "/nao-existe.so"),
    ] {
        let sock = SocketDir::new("env");
        let log = dev.0.join("env.log");
        let st = command(&dev.0, &sock.0, &log).env(k, v).status().unwrap();
        assert!(!st.success(), "{k}");
        assert!(
            !sock.0.join(ward_platform::socket::SOCKET_NAME).exists(),
            "{k}"
        );
        let text = fs::read_to_string(&log).unwrap();
        assert!(text.contains(k), "{k}: {text}");
    }
    // Without WARD_DEV_DIR the test daemon does not start (it never falls back to the
    // real vault).
    let sock = SocketDir::new("env2");
    let log = dev.0.join("env2.log");
    let st = command(&dev.0, &sock.0, &log)
        .env_remove("WARD_DEV_DIR")
        .status()
        .unwrap();
    assert!(!st.success());
}

/// V3.20, V3.17, V3.26, V3.9, V3.4: a whole cycle through the socket, with the log at
/// `trace`.
#[test]
fn v3_20_cycle_through_the_socket_without_secret_in_the_log() {
    let dev = Dir::new("sock");
    {
        let (d, _) = test_vault(&dev);
        d.lock();
    }
    let sock = SocketDir::new("cycle");
    let w = start(&dev, &sock);
    let mut s = connect(&w);

    let (_, r) = request_on(&mut s, "status", "{}");
    assert!(
        matches!(
            r,
            Response::Status {
                locked: true,
                vault_exists: true,
                role: Role::Agent,
            }
        ),
        "{r:?}"
    );
    // Wrong password: same response, no count; the right one (which waits 1 s) unlocks.
    let (q1, r) = request_on(&mut s, "unlock", &password_json("falso-errada-01"));
    assert_eq!(protocol_error(&r), Some(ResponseError::WrongPassword));
    let (_, r) = request_on(&mut s, "unlock", &password_json(PASSWORD));
    assert!(ok(&r), "{r:?}");

    // The agent's list: without the blocked one and without level.
    let (q, r) = request_on(&mut s, "list", "{}");
    let Response::List(l) = r else {
        panic!("{r:?}")
    };
    assert_eq!(l.len(), 4);
    assert!(!contains(&q, b"t-bloq") && !contains(&q, b"\"level\""));
    // Blocked = nonexistent, byte for byte.
    let exec = |name: &str| {
        format!(
            r#"{{"entry":"{name}","argv":["{}"],"cwd":"{}","stdin_kind":"null","environment":[]}}"#,
            b64(b"env"),
            b64(b"/")
        )
    };
    let (a, _) = request_on(&mut s, "run", &exec("t-bloq"));
    let (b, _) = request_on(&mut s, "run", &exec("nao-existe"));
    assert_eq!(a, b);
    // The agent does not change the level even with the right PIN.
    let (_, r) = request_on(
        &mut s,
        "set_level",
        &format!(
            r#"{{"entry":"t-bloq","level":"free","pin":"{}"}}"#,
            b64(PIN.as_bytes())
        ),
    );
    assert_eq!(protocol_error(&r), Some(ResponseError::Unauthorized));
    // Phase 23: the agent does not answer approvals (and there is no operation to
    // receive the notices: only the app role's connection receives them, D65); the
    // request to open the app without an install (test daemon, D63 without 23a) is "approval
    // unavailable", and so is the use with approval (D21).
    let (_, r) = request_on(&mut s, "receber_pedidos", "{}");
    assert_eq!(protocol_error(&r), Some(ResponseError::UnknownOperation));
    let (_, r) = request_on(
        &mut s,
        "answer_approval",
        &format!(
            r#"{{"request":1,"decision":"once","pin":"{}"}}"#,
            b64(PIN.as_bytes())
        ),
    );
    assert_eq!(protocol_error(&r), Some(ResponseError::Unauthorized));
    let (_, r) = request_on(&mut s, "open_app", "{}");
    // Without `WARD_DEV_APP`, the test daemon uses the launcher of the installed
    // `ward-app`: on a machine without Ward it is "approval unavailable"; with it
    // installed, it opens (Ok).
    let installed = std::path::Path::new("/usr/local/bin/ward-app").exists();
    let e = protocol_error(&r);
    assert!(
        e == Some(ResponseError::ApprovalUnavailable) || (installed && e.is_none()),
        "abrir_app: {e:?}"
    );
    let (_, r) = request_on(&mut s, "run", &exec("t-aprov"));
    assert_eq!(protocol_error(&r), Some(ResponseError::ApprovalUnavailable));

    // V3.4: 70 stalled connections do not take the service away (the oldest stalled
    // one leaves).
    let stalled: Vec<UnixStream> = (0..70).map(|_| connect(&w)).collect();
    std::thread::sleep(Duration::from_millis(200));
    let mut fresh = connect(&w);
    let (_, r) = request_on(&mut fresh, "status", "{}");
    assert!(matches!(r, Response::Status { locked: false, .. }), "{r:?}");
    drop(stalled);

    // Frame larger than the limit: refused, connection closed, the daemon goes on.
    let mut big = connect(&w);
    big.write_all(&u32::MAX.to_be_bytes()).unwrap();
    let _ = receive(&mut big);
    let mut b = [0u8; 1];
    assert_eq!(big.read(&mut b).unwrap_or(0), 0);
    // Invalid JSON, without closing the daemon.
    let mut junk = connect(&w);
    send(&mut junk, "{nao e json");
    let _ = receive(&mut junk);

    // The first connection was stalled during the flood: it was the one that left.
    let mut b = [0u8; 1];
    assert_eq!(
        s.read(&mut b).unwrap_or(0),
        0,
        "a conexão parada mais antiga saiu"
    );
    let mut s = connect(&w);
    let (_, r) = request_on(&mut s, "lock", "{}");
    assert!(ok(&r));
    let (_, r) = request_on(&mut s, "list", "{}");
    assert_eq!(protocol_error(&r), Some(ResponseError::Locked));
    drop(s);
    drop(fresh);

    // V3.20: the log at `trace` has no password, PIN nor secret (not even in base64).
    let log = w.log.clone();
    drop(w);
    let text = fs::read(&log).unwrap();
    assert!(
        contains(&text, b"pronto"),
        "{}",
        String::from_utf8_lossy(&text)
    );
    for s in SECRETS.iter().chain([&PASSWORD, &PIN, &"falso-errada-01"]) {
        assert!(!contains(&text, s.as_bytes()), "{s} no log");
        assert!(
            !contains(&text, b64(s.as_bytes()).as_bytes()),
            "{s} (b64) no log"
        );
    }
    assert!(!contains(&q1, b"falso"));
}

fn protocol_error(r: &Response) -> Option<ResponseError> {
    error(r)
}

/// V3.33 (D62, S9) through the binary: `kill -9` between the agent's wrong passwords
/// does not reset the wait. The failure is already in the state file when the response
/// arrives, and the daemon that starts again waits by the counter on disk (counted from
/// the start).
#[test]
fn v3_33_kill_9_between_the_wrong_passwords() {
    let dev = Dir::new("kill9");
    {
        let (d, _) = test_vault(&dev);
        d.lock();
    }
    let state = dev.paths().state;
    let failures = || {
        wardd::files::read_full_state(&state)
            .unwrap()
            .agent_failures
    };
    let sock = SocketDir::new("kill9");
    let mut waits = Vec::new();
    for i in 0..3u32 {
        let mut w = start(&dev, &sock);
        let mut s = connect(&w);
        let t = Instant::now();
        let (_, r) = request_on(&mut s, "unlock", &password_json("falso-errada-k9"));
        waits.push(t.elapsed());
        assert_eq!(protocol_error(&r), Some(ResponseError::WrongPassword));
        // Written (with fsync) before the response: the `kill -9` right after loses
        // nothing.
        w.child.kill().unwrap();
        let _ = w.child.wait();
        assert_eq!(failures(), i + 1);
        let _ = fs::remove_dir_all(&sock.0);
    }
    // 0 s, 1 s and 2 s, minus the start-up time (counted from the start). The first one
    // has only Argon2 (with a margin for a loaded machine); that it did not wait 1 s
    // shows in the distance to the third, which waits 2 s minus the start-up.
    assert!(waits[0] < Duration::from_millis(1500), "{waits:?}");
    assert!(
        waits[2] >= waits[0] + Duration::from_millis(1200),
        "{waits:?}"
    );
    assert!(waits[1] >= Duration::from_millis(500), "{waits:?}");
    assert!(waits[2] >= Duration::from_millis(1500), "{waits:?}");
    // The log does not have the password.
    let log = fs::read(dev.0.join("wardd.log")).unwrap();
    // V3.34 (D62): with the user's usual limit, the test daemon starts with a warning.
    let unlimited = fs::read_to_string("/proc/self/limits")
        .unwrap()
        .lines()
        .any(|l| l.starts_with("Max locked memory") && l.contains("unlimited"));
    if !unlimited {
        assert!(
            contains(&log, "memória sem travar".as_bytes()),
            "{}",
            String::from_utf8_lossy(&log)
        );
    }
    for s in [PASSWORD, "falso-errada-k9"] {
        assert!(!contains(&log, s.as_bytes()));
        assert!(!contains(&log, b64(s.as_bytes()).as_bytes()));
    }
}

/// Reads `/proc/<pid>/<file>` through another process of the same uid (`/usr/bin/cat`):
/// only tells whether it managed to read some byte, never the content.
fn other_process_reads(pid: u32, file: &str) -> bool {
    let o = Command::new("/usr/bin/cat")
        .arg(format!("/proc/{pid}/{file}"))
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .unwrap();
    o.status.success() || !o.stdout.is_empty()
}

/// V3.6 (S8) on the binary (M3 scrutiny): the test `wardd` running and unlocked (with
/// secrets in memory). Another process of the same uid does not read `/proc/<pid>/mem`,
/// `environ` nor `maps`, and neither does this test (the parent, which Yama would
/// allow); `strace -p` and `gdb -p` (if present) do not attach; `/proc/<pid>/limits`
/// shows core 0 (current and maximum); and `kill -SEGV` produces no core (the kernel
/// does not mark a core in the status and no file is left). Control: the same `cat`
/// reads the `environ` of a regular process.
#[test]
fn v3_6_running_wardd_denies_proc_ptrace_and_core() {
    use std::os::unix::process::ExitStatusExt;

    let mut control = Command::new("/usr/bin/sleep")
        .arg("30")
        .env_clear()
        .env("WARD_CONTROLE", "1")
        .spawn()
        .unwrap();
    // The `environ` is only right after the exec: try for a moment.
    let deadline = Instant::now() + Duration::from_secs(5);
    while !other_process_reads(control.id(), "environ") {
        assert!(
            Instant::now() < deadline,
            "controle: cat não leu o environ de um processo comum"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = control.kill();
    let _ = control.wait();

    let dev = Dir::new("v36");
    {
        let (d, _) = test_vault(&dev);
        d.lock();
    }
    let sock = SocketDir::new("v36");
    let log = dev.0.join("wardd.log");
    let mut c = command(&dev.0, &sock.0, &log);
    // The daemon's working directory: where a core file would land.
    c.current_dir(&dev.0);
    let child = c.spawn().unwrap();
    let socket = sock.0.join(ward_platform::socket::SOCKET_NAME);
    let deadline = Instant::now() + Duration::from_secs(20);
    while !socket.exists() {
        assert!(Instant::now() < deadline, "o wardd não criou o socket");
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut w = Wardd { child, log, socket };
    let mut s = connect(&w);
    let (_, r) = request_on(&mut s, "unlock", &password_json(PASSWORD));
    assert!(ok(&r), "{r:?}");
    let pid = w.child.id();

    for file in ["mem", "environ", "maps"] {
        assert!(
            !other_process_reads(pid, file),
            "/proc/<pid>/{file} legível por outro processo"
        );
        let r = fs::read(format!("/proc/{pid}/{file}"));
        assert_eq!(
            r.as_ref().map_err(|e| e.kind()).err(),
            Some(std::io::ErrorKind::PermissionDenied),
            "/proc/<pid>/{file} legível pelo pai"
        );
    }
    // `strace -p`: does not attach (exits ≠ 0 with EPERM).
    let st = Command::new("/usr/bin/strace")
        .args(["-o", "/dev/null", "-p", &pid.to_string()])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(!st.success(), "strace -p conseguiu");
    // `gdb -p`, if there is gdb: the attach fails for lack of permission.
    if Path::new("/usr/bin/gdb").exists() {
        let o = Command::new("/usr/bin/gdb")
            .args([
                "-nx",
                "-batch",
                "-p",
                &pid.to_string(),
                "-ex",
                "info registers",
            ])
            .env_clear()
            .env("HOME", &dev.0)
            .env("LANG", "C")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let text =
            String::from_utf8_lossy(&o.stderr).into_owned() + &String::from_utf8_lossy(&o.stdout);
        assert!(
            text.contains("Operation not permitted"),
            "gdb -p não recusou: {text}"
        );
    } else {
        eprintln!("v3_6: sem /usr/bin/gdb; o strace -p cobre o mesmo ptrace");
    }
    // Core turned off: `Max core file size 0 0`.
    let limits = fs::read_to_string(format!("/proc/{pid}/limits")).unwrap();
    let core = limits
        .lines()
        .find(|l| l.starts_with("Max core file size"))
        .unwrap();
    let fields: Vec<&str> = core.split_whitespace().collect();
    assert_eq!(&fields[4..6], &["0", "0"], "{core}");
    // The daemon keeps serving after the attempts.
    let (_, r) = request_on(&mut s, "status", "{}");
    assert!(matches!(r, Response::Status { locked: false, .. }), "{r:?}");

    // `kill -SEGV` with the vault unlocked: dies by the signal, without a core. The first
    // SIGSEGV sent by `kill` lands in Rust's std handler (the stack overflow one), which
    // resets the handler to the default and returns; the next one kills. Send it again
    // every 200 ms until it dies.
    let deadline = Instant::now() + Duration::from_secs(10);
    let exit_status = loop {
        if let Some(st) = w.child.try_wait().unwrap() {
            break st;
        }
        assert!(Instant::now() < deadline, "o wardd não morreu com SIGSEGV");
        let st = Command::new("/usr/bin/kill")
            .args(["-SEGV", &pid.to_string()])
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(st.success() || w.child.try_wait().unwrap().is_some());
        std::thread::sleep(Duration::from_millis(200));
    };
    assert_eq!(exit_status.signal(), Some(11), "{exit_status:?}");
    assert!(!exit_status.core_dumped(), "o kernel gerou core do wardd");
    for e in fs::read_dir(&dev.0).unwrap() {
        let name = e.unwrap().file_name().to_string_lossy().into_owned();
        assert!(!name.starts_with("core"), "core no disco: {name}");
    }
    drop(s);
}

/// The request frames of the 0.1.4 protocol (`dev/fixtures/legacy/protocol-v1.jsonl`).
fn v1_requests() -> Vec<String> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dev/fixtures/legacy/protocol-v1.jsonl");
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| l.starts_with(r#"{"kind":"request","#))
        .map(|l| {
            let (_, frame) = l.split_once(r#","frame":"#).expect("linha sem frame");
            frame.strip_suffix('}').expect("linha sem fecho").to_owned()
        })
        .collect()
}

/// The only answer a v1 frame gets (m2:DT8): code and fixed message, no data.
fn unknown_version_frame() -> Vec<u8> {
    protocol::encode_response(Role::Agent, &Response::Error(ResponseError::UnknownVersion))
        .unwrap()
        .to_vec()
}

/// After the error, the connection is closed: nothing more comes, and a write either
/// fails or is never answered.
fn assert_closed(s: &mut UnixStream, what: &str) {
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut b = [0u8; 1];
    assert_eq!(
        s.read(&mut b).unwrap_or(0),
        0,
        "{what}: a conexão seguiu aberta"
    );
}

/// Attack m2:V4.13 (S1, RM3): every request line of the 0.1.4 protocol, against the
/// 0.2.0 daemon with the vault **unlocked**, gets exactly the `unknown_version` error
/// (no data field) and the connection closes; a second v1 frame on the same connection
/// (the v1 unlock with the right password) gets no answer. No response carries a
/// secret of the vault. Then, with the vault locked, the v1 unlock with the right
/// password does not unlock it (m2:V4.17).
#[test]
fn attack_m2_v4_13_v1_frames_get_unknown_version_and_the_connection_closes() {
    let dev = Dir::new("v1");
    {
        let (d, _) = test_vault(&dev);
        d.lock();
    }
    let sock = SocketDir::new("v1");
    let w = start(&dev, &sock);
    let mut s = connect(&w);
    let (_, r) = request_on(&mut s, "unlock", &password_json(PASSWORD));
    assert!(ok(&r), "{r:?}");

    let expected = unknown_version_frame();
    assert_eq!(
        String::from_utf8_lossy(&expected),
        r#"{"v":2,"type":"error","data":{"code":"unknown_version","message":"versão do protocolo incompatível: atualize o Ward"}}"#
    );
    let requests = v1_requests();
    assert!(requests.len() >= 30, "{} linhas", requests.len());
    let v1_unlock = format!(
        r#"{{"v":1,"op":"destravar","dados":{{"senha":"{}"}}}}"#,
        b64(PASSWORD.as_bytes())
    );
    for frame in &requests {
        let mut c = connect(&w);
        send(&mut c, frame);
        let answer = receive(&mut c).expect("sem resposta ao quadro v1");
        assert_eq!(answer, expected, "{frame}");
        for secret in SECRETS {
            assert!(!contains(&answer, secret.as_bytes()));
            assert!(!contains(&answer, b64(secret.as_bytes()).as_bytes()));
        }
        // The same connection: the next frame is never read.
        let _ = c
            .write_all(&(v1_unlock.len() as u32).to_be_bytes())
            .and_then(|()| c.write_all(v1_unlock.as_bytes()));
        assert_closed(&mut c, frame);
    }
    // The daemon goes on serving v2, still unlocked.
    let (_, r) = request_on(&mut s, "status", "{}");
    assert!(matches!(r, Response::Status { locked: false, .. }), "{r:?}");

    // m2:V4.17: locked, the old CLI's unlock with the right password does not unlock.
    let (_, r) = request_on(&mut s, "lock", "{}");
    assert!(ok(&r), "{r:?}");
    let mut c = connect(&w);
    send(&mut c, &v1_unlock);
    assert_eq!(receive(&mut c).as_deref(), Some(expected.as_slice()));
    assert_closed(&mut c, "destravar v1");
    let (_, r) = request_on(&mut s, "status", "{}");
    assert!(matches!(r, Response::Status { locked: true, .. }), "{r:?}");

    // The log has the refusal code and nothing of the password.
    drop(w);
    let log = fs::read(dev.0.join("wardd.log")).unwrap();
    assert!(contains(&log, b"unknown_version"));
    assert!(!contains(&log, PASSWORD.as_bytes()));
    assert!(!contains(&log, b64(PASSWORD.as_bytes()).as_bytes()));
}
