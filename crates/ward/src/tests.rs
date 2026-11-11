//! CLI tests against fake servers in the test process itself: an honest "daemon" that
//! speaks the protocol and malicious servers that record everything they receive. Only
//! fake values; the socket lives in a 0700 folder of the test alone in
//! `/run/user/<uid>/` (the path does not fit in `target/tmp`), removed at the end. Never
//! the real vault nor the test daemon.

// m2:Phase 54a: without `test-mode` the binaries this test runs are the release ones,
// which only know the system socket (`/run/ward/<uid>/wardd.sock`, the installed
// daemon). The build fails, loudly, instead of a test reaching it.
#[cfg(not(feature = "test-mode"))]
compile_error!(
    "este teste exige a feature test-mode (cargo test --all-features ou --features test-mode): sem ela os binários do teste são os de release, que só conhecem o socket do sistema"
);

use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use ward_core::limits::MAX_COMMON_FRAME;
use ward_core::protocol::frame::{self, RESPONSE_LIMIT};
use ward_core::protocol::{self, Request, Role};
use ward_platform::{ExpectedServer, TerminalError};
use zeroize::Zeroizing;

use crate::client::{Connection, ConnectionError};
use crate::commands::{EXIT_ERROR, EXIT_OK, EXIT_USAGE, Env};

const PASSWORD: &str = "falso-mestra-ward-01";

static N: AtomicU32 = AtomicU32::new(0);

/// m2:Phase 54a: the release rules (root's folder, peer uid 0) on a path that never
/// exists (`/run` is root's, the folder below it is not there), in place of
/// `ExpectedServer::system()`: a test that connects by mistake gets "daemon not
/// running", never the installed daemon.
fn unreachable_server() -> ExpectedServer {
    ExpectedServer::system_at(PathBuf::from("/run/ward-nao-existe-falso/1/wardd.sock"))
}

/// 0700 folder of this test alone, removed on `Drop`.
struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let p = ward_platform::socket::runtime_dir().join(format!(
            "ward-cli-t-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir(&p).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        TestDir(p)
    }
    fn socket(&self) -> PathBuf {
        self.0.join("wardd.sock")
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// What the fake server saw.
#[derive(Default)]
struct Seen {
    connections: u32,
    bytes: Vec<u8>,
    operations: Vec<&'static str>,
    passwords: Vec<Vec<u8>>,
}

type ReplyFn = dyn Fn(&Request) -> Vec<u8> + Send + Sync;

/// Fake server on the folder's socket. `reply = None`: only records the bytes (a
/// malicious one waiting for the password). With `reply`: reads frames like the daemon
/// and answers.
fn fake_server(dir: &TestDir, reply: Option<Arc<ReplyFn>>) -> Arc<Mutex<Seen>> {
    let l = UnixListener::bind(dir.socket()).unwrap();
    fs::set_permissions(dir.socket(), fs::Permissions::from_mode(0o600)).unwrap();
    let seen = Arc::new(Mutex::new(Seen::default()));
    let v = Arc::clone(&seen);
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { return };
            v.lock().unwrap().connections += 1;
            match &reply {
                None => {
                    let mut buf = Vec::new();
                    let _ = s.read_to_end(&mut buf);
                    v.lock().unwrap().bytes.extend_from_slice(&buf);
                }
                Some(r) => loop {
                    let Ok(body) = frame::read_frame(&mut s, MAX_COMMON_FRAME) else {
                        break;
                    };
                    let p = protocol::decode_request(Role::Agent, &body).unwrap();
                    {
                        let mut v = v.lock().unwrap();
                        v.bytes.extend_from_slice(&body);
                        v.operations.push(p.operation().name());
                        if let Request::Unlock { password } = &p {
                            v.passwords.push(password.expose().to_vec());
                        }
                    }
                    let response = r(&p);
                    if frame::write_frame(&mut s, &response, RESPONSE_LIMIT).is_err() {
                        break;
                    }
                },
            }
        }
    });
    seen
}

/// Test environment: records the order of the steps and the output.
struct TestEnv {
    expected: ExpectedServer,
    /// Full check (`Connection::connect`) or without the extra one of `test-mode`.
    full: bool,
    events: Vec<&'static str>,
    before_password: Option<Box<dyn FnMut()>>,
    password: Result<&'static str, TerminalError>,
    out: Vec<u8>,
    err: Vec<u8>,
    /// Environment of `ward run` (instead of the test process's).
    env: Vec<(std::ffi::OsString, std::ffi::OsString)>,
    /// Phase 54b: the kind of standard input the environment reports (the test
    /// process's does not count).
    kind: ward_core::protocol::StdinKind,
    /// How many times the standard input was asked for.
    stdin_reads: u32,
}

impl TestEnv {
    fn new(expected: ExpectedServer, full: bool) -> TestEnv {
        TestEnv {
            expected,
            full,
            events: Vec::new(),
            before_password: None,
            password: Ok(PASSWORD),
            out: Vec::new(),
            err: Vec::new(),
            env: vec![("PATH".into(), "/usr/bin:/bin".into())],
            kind: ward_core::protocol::StdinKind::Null,
            stdin_reads: 0,
        }
    }
    fn run(&mut self, args: &[&str]) -> u8 {
        crate::main(args.iter().map(std::ffi::OsString::from), self)
    }
    fn out(&self) -> String {
        String::from_utf8(self.out.clone()).unwrap()
    }
    fn err(&self) -> String {
        String::from_utf8(self.err.clone()).unwrap()
    }
}

impl Env for TestEnv {
    fn connect(&mut self) -> Result<Connection, ConnectionError> {
        self.events.push("conectar");
        if self.full {
            Connection::connect(&self.expected)
        } else {
            #[cfg(feature = "test-mode")]
            {
                Connection::connect_without_extra(&self.expected)
            }
            #[cfg(not(feature = "test-mode"))]
            {
                Connection::connect(&self.expected)
            }
        }
    }
    fn harden(&mut self) -> io::Result<()> {
        // The test process stays dumpable: it only records the step.
        self.events.push("endurecer");
        Ok(())
    }
    fn read_password(&mut self) -> Result<Zeroizing<Vec<u8>>, TerminalError> {
        self.events.push("ler_senha");
        if let Some(f) = self.before_password.as_mut() {
            f();
        }
        self.password.map(|s| Zeroizing::new(s.as_bytes().to_vec()))
    }
    fn stdout(&mut self) -> &mut dyn Write {
        &mut self.out
    }
    fn stderr(&mut self) -> &mut dyn Write {
        &mut self.err
    }
    fn environment(&self) -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
        self.env.clone()
    }
    fn stdin(&mut self) -> Box<dyn Read + Send> {
        self.stdin_reads += 1;
        Box::new(io::empty())
    }
    fn stdin_kind(&self) -> ward_core::protocol::StdinKind {
        self.kind
    }
    fn terminal(&self) -> Option<ward_core::protocol::WindowSize> {
        // Never the terminal of whoever runs the tests.
        None
    }
    fn raw_mode(&mut self) -> Option<ward_platform::RawMode> {
        None
    }
}

/// D52: argv + environment above 750 KiB exit ≠ 0 with the fixed message, without
/// quoting a value and without even connecting; a field above 128 KiB too. Wrong usage
/// exits 2.
#[test]
fn d52_run_too_large_refused_before_connecting() {
    use crate::commands::{EXIT_RUN_ERROR, MSG_RUN_FIELD, MSG_RUN_TOO_LARGE};
    let value = "falso-grande-".repeat(70 * 1024 / 13);
    let mut t = TestEnv::new(unreachable_server(), true);
    t.env = (0..12)
        .map(|i| (format!("V{i}").into(), value.clone().into()))
        .collect();
    assert_eq!(t.run(&["run", "acme-pg-dev", "--", "psql"]), EXIT_RUN_ERROR);
    assert_eq!(t.err(), format!("ward: {MSG_RUN_TOO_LARGE}\n"));
    assert!(t.events.is_empty(), "{:?}", t.events);
    assert!(!t.err().contains("falso-grande") && t.out.is_empty());

    let mut t = TestEnv::new(unreachable_server(), true);
    t.env = vec![("X".into(), "falso-campo-".repeat(12 * 1024).into())];
    assert_eq!(t.run(&["run", "e", "--", "true"]), EXIT_RUN_ERROR);
    assert_eq!(t.err(), format!("ward: {MSG_RUN_FIELD}\n"));
    assert!(t.events.is_empty());

    // Below the limit: passes the measure (here, it stops at the missing daemon).
    let mut t = TestEnv::new(unreachable_server(), true);
    t.env = vec![("X".into(), "x".repeat(600 * 1024 / 8).into())];
    assert_eq!(t.run(&["run", "e", "--", "true"]), EXIT_RUN_ERROR);
    assert_eq!(t.events, ["conectar"]);
    let mut t = TestEnv::new(unreachable_server(), true);
    assert_eq!(t.run(&["run", "e", "true"]), EXIT_USAGE);
    assert!(t.events.is_empty());
}

#[cfg(feature = "test-mode")]
fn wait_for_connections(seen: &Arc<Mutex<Seen>>, n: u32) {
    for _ in 0..200 {
        if seen.lock().unwrap().connections >= n {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// Attack (V4.2, S5, D46): a fake server of the same uid in a folder of the user. With
/// the release rule (root's folder, peer uid 0), the CLI refuses before connecting: it
/// does not read the password, hardens nothing and the fake gets not even a
/// connection.
#[test]
fn v4_2_attack_fake_server_refused_before_reading_the_password() {
    let dir = TestDir::new();
    let seen = fake_server(&dir, None);
    for cmd in [&["unlock"][..], &["list"], &["list", "--json"], &["lock"]] {
        let mut t = TestEnv::new(ExpectedServer::system_at(dir.socket()), true);
        assert_eq!(t.run(cmd), EXIT_ERROR, "{cmd:?}");
        assert!(
            t.err().contains("servidor não é o wardd"),
            "{cmd:?}: {}",
            t.err()
        );
        assert!(t.err().contains("nada foi enviado"));
        assert_eq!(t.events, ["conectar"], "{cmd:?}");
        assert!(t.out.is_empty());
    }
    std::thread::sleep(std::time::Duration::from_millis(50));
    let v = seen.lock().unwrap();
    assert_eq!(v.connections, 0);
    assert!(v.bytes.is_empty());
}

/// Attack (V4.2 in `test-mode`): the fake sits on the path of the test socket, which
/// passes the test rule (the user's 0700 folder, peer of the same uid). The extra check
/// refuses because the fake's process is dumpable; the fake sees the connection and
/// zero bytes, and the password is not read.
#[cfg(feature = "test-mode")]
#[test]
fn v4_2_attack_fake_server_on_the_test_socket_refused() {
    let dir = TestDir::new();
    let seen = fake_server(&dir, None);
    for cmd in [&["unlock"][..], &["list", "--json"], &["lock"]] {
        let mut t = TestEnv::new(ExpectedServer::test_at(dir.socket()), true);
        assert_eq!(t.run(cmd), EXIT_ERROR);
        assert!(t.err().contains("servidor não é o wardd"), "{}", t.err());
        assert!(t.err().contains("pode ser depurado"), "{}", t.err());
        assert_eq!(t.events, ["conectar"]);
    }
    wait_for_connections(&seen, 3);
    std::thread::sleep(std::time::Duration::from_millis(50));
    let v = seen.lock().unwrap();
    assert_eq!(v.connections, 3);
    assert!(
        v.bytes.is_empty(),
        "o falso recebeu {} bytes",
        v.bytes.len()
    );
}

#[test]
fn daemon_missing() {
    let dir = TestDir::new();
    let mut t = TestEnv::new(ExpectedServer::system_at(dir.socket()), true);
    // The folder belongs to the user: refuses even before seeing there is no socket.
    assert_eq!(t.run(&["list"]), EXIT_ERROR);
    let mut t = TestEnv::new(unreachable_server(), true);
    assert_eq!(t.run(&["unlock"]), EXIT_ERROR);
    assert_eq!(t.err(), "ward: o daemon não está rodando\n");
    assert_eq!(t.events, ["conectar"]);
}

#[test]
fn usage_and_help() {
    let mut t = TestEnv::new(unreachable_server(), true);
    assert_eq!(t.run(&["--help"]), EXIT_OK);
    assert!(t.out().contains("ward list [--json]"));
    assert!(t.out().contains("ward unlock"));
    assert!(t.out().contains("ward lock"));
    let mut t = TestEnv::new(unreachable_server(), true);
    assert_eq!(t.run(&[]), EXIT_USAGE);
    assert!(t.err().contains("Uso:"));
    let mut t = TestEnv::new(unreachable_server(), true);
    assert_eq!(t.run(&["unlock", PASSWORD]), EXIT_USAGE);
    assert!(!t.err().contains(PASSWORD));
    assert!(t.events.is_empty(), "uso errado não conecta");
    let mut t = TestEnv::new(unreachable_server(), true);
    assert_eq!(t.run(&["--version"]), EXIT_OK);
    assert!(t.out().starts_with("ward "));
}

/// Attack (m2:DT10, m2:V5.1): the 0.1.x subcommands have no alias. Each one exits 2 with
/// the pt-BR "unknown command" message before connecting: a server listening on the test socket sees
/// no connection, no password is read and nothing is hardened. A password typed after
/// the old unlock name does not come back in the message.
#[test]
fn attack_old_subcommands_unknown_without_talking_to_the_daemon() {
    let dir = TestDir::new();
    let seen = fake_server(&dir, None);
    for cmd in [
        &["lista"][..],
        &["lista", "--json"],
        &["travar"],
        &["destravar"],
        &["destravar", PASSWORD],
        &["ajuda"],
    ] {
        let mut t = TestEnv::new(ExpectedServer::test_at(dir.socket()), true);
        assert_eq!(t.run(cmd), EXIT_USAGE, "{cmd:?}");
        assert!(
            t.err().contains("comando desconhecido"),
            "{cmd:?}: {}",
            t.err()
        );
        assert!(!t.err().contains(PASSWORD), "{cmd:?}");
        assert!(t.events.is_empty(), "{cmd:?}: {:?}", t.events);
        assert!(t.out.is_empty(), "{cmd:?}");
    }
    std::thread::sleep(std::time::Duration::from_millis(50));
    let v = seen.lock().unwrap();
    assert_eq!(v.connections, 0);
    assert!(v.bytes.is_empty());
}

#[cfg(feature = "test-mode")]
mod with_fake_daemon {
    use super::*;
    use ward_core::entry::{EntryType, Environment};
    use ward_core::protocol::{ListItem, Response, ResponseError};

    fn encode(r: &Response) -> Vec<u8> {
        protocol::encode_response(Role::Agent, r).unwrap().to_vec()
    }

    /// Response to the state request. The only place that builds the state response:
    /// Phase 23 adds the role (D63) and only this spot changes.
    fn state_response(locked: bool) -> Response {
        Response::Status {
            locked,
            vault_exists: true,
            role: Role::Agent,
        }
    }

    fn item(name: &str, kind: EntryType, address: Option<&str>) -> ListItem {
        ListItem {
            name: name.to_owned(),
            kind,
            group: "acme".to_owned(),
            environment: Environment::Dev,
            project: Some("site".to_owned()),
            tags: vec![],
            level: None,
            address: address.map(str::to_owned),
            summary: None,
        }
    }

    /// Honest fake daemon: `locked` changes with unlock/lock.
    fn daemon(dir: &TestDir, locked: bool) -> Arc<Mutex<Seen>> {
        let state = Arc::new(Mutex::new(locked));
        fake_server(
            dir,
            Some(Arc::new(move |p: &Request| {
                let mut locked = state.lock().unwrap();
                encode(&match p {
                    Request::Status => state_response(*locked),
                    Request::Lock => {
                        *locked = true;
                        Response::Ok
                    }
                    Request::Unlock { password } if password.expose() == PASSWORD.as_bytes() => {
                        *locked = false;
                        Response::Ok
                    }
                    Request::Unlock { .. } => Response::Error(ResponseError::WrongPassword),
                    Request::List if *locked => Response::Error(ResponseError::Locked),
                    Request::List => Response::List(vec![
                        item(
                            "acme-wp-dev",
                            EntryType::Login,
                            Some("http://127.0.0.1:8081"),
                        ),
                        item("gama-nota", EntryType::SecureNote, None),
                    ]),
                    _ => Response::Error(ResponseError::Unauthorized),
                })
            })),
        )
    }

    fn test_env(dir: &TestDir) -> TestEnv {
        TestEnv::new(ExpectedServer::test_at(dir.socket()), false)
    }

    /// `ward run` against a fake daemon: the request carries argv, cwd and the
    /// environment without `LD_*` (D13); the output and the code come from the daemon; a
    /// daemon error exits 125. The locked vault warns that it waits for the unlock
    /// (V4.21).
    #[test]
    fn run_request_output_and_code() {
        use crate::commands::{EXIT_RUN_ERROR, MSG_RUN_LOCKED};
        use ward_core::protocol::Stream;
        for (entry, locked) in [("t-ok", false), ("t-nota", false), ("t-ok", true)] {
            let dir = TestDir::new();
            let seen_env: Arc<Mutex<Vec<Vec<u8>>>> = Arc::default();
            let va = Arc::clone(&seen_env);
            let _v = fake_server(
                &dir,
                Some(Arc::new(move |p: &Request| match p {
                    Request::Status => encode(&state_response(locked)),
                    Request::Run(x) if x.entry == "t-nota" => {
                        encode(&Response::Error(ResponseError::TypeWithoutDelivery))
                    }
                    Request::Run(x) => {
                        assert_eq!(x.argv.len(), 2);
                        assert_eq!(x.argv[0].as_slice(), b"sh");
                        assert!(!x.cwd.is_empty());
                        va.lock()
                            .unwrap()
                            .extend(x.env.iter().map(|v| v.name.to_vec()));
                        // The fake answers one frame per request: the output here and
                        // the end in the response to the end of the standard input.
                        encode(&Response::Output {
                            stream: Stream::Stdout,
                            data: Zeroizing::new("ok ••••\n".as_bytes().to_vec()),
                        })
                    }
                    Request::Stdin { .. } => encode(&Response::Exit { code: 3 }),
                    _ => encode(&Response::Error(ResponseError::Unauthorized)),
                })),
            );
            let mut t = test_env(&dir);
            t.env = vec![
                ("PATH".into(), "/usr/bin:/bin".into()),
                ("LD_PRELOAD".into(), "/x.so".into()),
                ("LD_AUDIT".into(), "/y.so".into()),
            ];
            let code = t.run(&["run", entry, "--", "sh", "-c"]);
            if entry == "t-nota" {
                assert_eq!(code, EXIT_RUN_ERROR);
                assert_eq!(t.err(), "ward: tipo sem entrega\n");
                continue;
            }
            assert_eq!(code, 3, "{}", t.err());
            assert_eq!(t.out(), "ok ••••\n");
            assert_eq!(*seen_env.lock().unwrap(), [b"PATH".to_vec()]);
            assert_eq!(t.err().contains(MSG_RUN_LOCKED), locked, "{}", t.err());
        }
    }

    /// Phase 54b (D68): `ward run` and `ward ssh` send the kind of standard input the
    /// environment reports; with a null input, the CLI does not even ask for the input
    /// (only the empty end frame goes).
    #[test]
    fn f54b_run_and_ssh_send_the_kind_and_null_does_not_read() {
        use ward_core::protocol::StdinKind as T;
        for (sub, kind) in T::ALL.into_iter().flat_map(|t| [("run", t), ("ssh", t)]) {
            let dir = TestDir::new();
            let seen: Arc<Mutex<Vec<(T, usize)>>> = Arc::default();
            let v = Arc::clone(&seen);
            let output = Response::Output {
                stream: ward_core::protocol::Stream::Stdout,
                data: Zeroizing::new(b"ok\n".to_vec()),
            };
            let _v = fake_server(
                &dir,
                Some(Arc::new(move |p: &Request| match p {
                    Request::Status => encode(&state_response(false)),
                    Request::Run(x) => {
                        v.lock().unwrap().push((x.stdin_kind, 0));
                        encode(&output)
                    }
                    Request::Ssh { stdin_kind, .. } => {
                        v.lock().unwrap().push((*stdin_kind, 0));
                        encode(&output)
                    }
                    Request::Stdin { data } => {
                        v.lock().unwrap().push((T::Null, data.len() + 1));
                        encode(&Response::Exit { code: 0 })
                    }
                    _ => encode(&Response::Error(ResponseError::Unauthorized)),
                })),
            );
            let mut t = test_env(&dir);
            t.kind = kind;
            let code = if sub == "run" {
                t.run(&["run", "e", "--", "true"])
            } else {
                t.run(&["ssh", "e", "true"])
            };
            assert_eq!(code, 0, "{sub} {kind:?}: {}", t.err());
            let seen = seen.lock().unwrap().clone();
            assert_eq!(seen.first().map(|x| x.0), Some(kind), "{sub}");
            // Only the empty end frame (size 0 + 1).
            assert_eq!(seen.get(1).map(|x| x.1), Some(1), "{sub} {kind:?}");
            assert_eq!(t.stdin_reads, u32::from(kind != T::Null), "{sub} {kind:?}");
        }
    }

    /// A channel like the app's (Phase 27a): `content` queued on a Unix socket whose
    /// other end has already closed. Returns the fd number; ownership passes to the CLI,
    /// which closes it.
    fn channel(content: &[u8]) -> u32 {
        use std::os::fd::IntoRawFd;
        let (mut a, b) = std::os::unix::net::UnixStream::pair().unwrap();
        a.write_all(content).unwrap();
        drop(a);
        u32::try_from(b.into_raw_fd()).unwrap()
    }

    /// Attack (Phase 27a): token channel with garbage, empty, large, of another kind (a
    /// pipe, which the same uid would reopen through `/proc`) or closed. The CLI exits
    /// 125 with a fixed message, without the content, and **without connecting** (nothing
    /// goes to the daemon); `--token` in argv exits 2 without connecting.
    #[test]
    fn attack_forged_token_channel_does_not_connect() {
        use crate::commands::{EXIT_RUN_ERROR, MSG_TOKEN_FORMAT};
        use std::os::fd::IntoRawFd;
        let dir = TestDir::new();
        let seen = fake_server(&dir, None);
        let garbage = b"falso-lixo;rm -rf /\n\xff".repeat(2);
        let (r, mut w) = std::io::pipe().unwrap();
        w.write_all(b"AbCdEfGhIjKlMnOpQrStUvWxYz012345").unwrap();
        drop(w);
        let pipe = u32::try_from(r.into_raw_fd()).unwrap();
        for (fd, msg) in [
            (channel(&garbage[..30]), MSG_TOKEN_FORMAT),
            (channel(b"-AbCdEfGhIjKlMnOpQr"), MSG_TOKEN_FORMAT),
            (channel(b""), "o canal do token do Conectar veio vazio"),
            (
                channel(&b"falso-grande-".repeat(10)),
                "o canal do token do Conectar passa do tamanho do token",
            ),
            (pipe, "o canal do token do Conectar não é o do app"),
            (1_000_000, "o canal do token do Conectar não está aberto"),
        ] {
            let mut t = test_env(&dir);
            let n = fd.to_string();
            assert_eq!(
                t.run(&["ssh", "--token-fd", &n, "acme-ssh", "id"]),
                EXIT_RUN_ERROR
            );
            assert_eq!(t.err(), format!("ward: {msg}\n"));
            assert!(!t.err().contains("falso") && !t.err().contains("AbCd"));
            assert_eq!(t.events, ["endurecer"], "{msg}");
        }
        let mut t = test_env(&dir);
        let code = t.run(&["ssh", "--token", "AbCdEfGhIjKlMnOpQrStUvWxYz012345", "e"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(!t.err().contains("AbCd"), "{}", t.err());
        assert!(t.events.is_empty());
        assert_eq!(seen.lock().unwrap().connections, 0);
    }

    /// `ward ssh` (Phases 27 and 27a) against a fake daemon: the request carries the
    /// entry, the command and the token that arrived through the fd (no window outside a
    /// terminal); the output and the code come from the daemon; a daemon error exits 125
    /// with the fixed message.
    #[test]
    fn ssh_request_token_output_and_code() {
        use crate::commands::EXIT_RUN_ERROR;
        use ward_core::protocol::Stream;
        let token = "AbCdEfGhIjKlMnOpQrStUvWxYz012345";
        let dir = TestDir::new();
        let _v = fake_server(
            &dir,
            Some(Arc::new(move |p: &Request| match p {
                Request::Status => encode(&state_response(false)),
                Request::Ssh { entry, .. } if entry == "t-nota" => {
                    encode(&Response::Error(ResponseError::TypeWithoutDelivery))
                }
                Request::Ssh {
                    entry,
                    command,
                    token: t,
                    window_size,
                    stdin_kind,
                } => {
                    assert_eq!(entry, "acme-ssh");
                    assert_eq!(*stdin_kind, ward_core::protocol::StdinKind::Null);
                    let c: Vec<&[u8]> = command.iter().map(|x| x.as_slice()).collect();
                    assert_eq!(c, [b"ls".as_slice(), b"-la"]);
                    assert_eq!(t.as_ref().map(|t| t.expose().to_vec()), Some(token.into()));
                    assert!(window_size.is_none());
                    encode(&Response::Output {
                        stream: Stream::Stdout,
                        data: Zeroizing::new("senha=••••\n".as_bytes().to_vec()),
                    })
                }
                Request::Stdin { .. } => encode(&Response::Exit { code: 4 }),
                _ => encode(&Response::Error(ResponseError::Unauthorized)),
            })),
        );
        let mut t = test_env(&dir);
        let fd = channel(token.as_bytes()).to_string();
        let code = t.run(&["ssh", "--token-fd", &fd, "acme-ssh", "--", "ls", "-la"]);
        assert_eq!(code, 4, "{}", t.err());
        assert_eq!(t.out(), "senha=••••\n");
        // The process hardens before the token exists in memory and reads before
        // connecting.
        assert_eq!(t.events, ["endurecer", "conectar"]);
        let mut t = test_env(&dir);
        assert_eq!(t.run(&["ssh", "t-nota", "id"]), EXIT_RUN_ERROR);
        assert_eq!(t.err(), "ward: tipo sem entrega\n");
    }

    #[test]
    fn v4_1_list_as_text_and_json() {
        let dir = TestDir::new();
        let _v = daemon(&dir, false);
        let mut t = test_env(&dir);
        assert_eq!(t.run(&["list", "--json"]), EXIT_OK, "{}", t.err());
        assert_eq!(
            t.out(),
            "[{\"name\":\"acme-wp-dev\",\"type\":\"login\",\"group\":\"acme\",\"environment\":\"dev\",\
             \"project\":\"site\",\"address\":\"http://127.0.0.1:8081\"},\
             {\"name\":\"gama-nota\",\"type\":\"secure-note\",\"group\":\"acme\",\"environment\":\"dev\",\
             \"project\":\"site\",\"address\":null}]\n"
        );
        let mut t = test_env(&dir);
        assert_eq!(t.run(&["list"]), EXIT_OK);
        assert!(t.out().starts_with("NOME "));
        assert!(t.out().contains("acme-wp-dev"));
        // m2:V4.18: the table shows the pt-BR label, not the stored name.
        assert!(t.out().contains("nota segura"), "{}", t.out());
        assert!(!t.out().contains("secure-note"), "{}", t.out());
        assert_eq!(t.events, ["conectar"]);
    }

    /// V4.22: after a lock, the list answers that the vault is locked.
    #[test]
    fn v4_22_lock_then_list_answers_locked() {
        let dir = TestDir::new();
        let v = daemon(&dir, false);
        let mut t = test_env(&dir);
        assert_eq!(t.run(&["lock"]), EXIT_OK);
        assert_eq!(t.out(), "cofre travado\n");
        let mut t = test_env(&dir);
        assert_eq!(t.run(&["list"]), EXIT_ERROR);
        assert!(t.err().starts_with("ward: travado"), "{}", t.err());
        assert!(t.out.is_empty());
        assert_eq!(v.lock().unwrap().operations, ["lock", "list"]);
    }

    /// Order of the unlock: server checked, state, harden, password, reverify, unlock.
    /// The password goes only once, in the unlock request.
    #[test]
    fn unlock_in_order() {
        let dir = TestDir::new();
        let v = daemon(&dir, true);
        let mut t = test_env(&dir);
        assert_eq!(t.run(&["unlock"]), EXIT_OK, "{}", t.err());
        assert_eq!(t.out(), "cofre destravado\n");
        assert_eq!(t.events, ["conectar", "endurecer", "ler_senha"]);
        let v = v.lock().unwrap();
        assert_eq!(v.operations, ["status", "unlock"]);
        assert_eq!(v.passwords, [PASSWORD.as_bytes()]);
        drop(v);

        // Already unlocked: does not read the password.
        let mut t = test_env(&dir);
        assert_eq!(t.run(&["unlock"]), EXIT_OK);
        assert_eq!(t.out(), "o cofre já está destravado\n");
        assert_eq!(t.events, ["conectar"]);
    }

    #[test]
    fn unlock_wrong_empty_or_canceled_password() {
        let dir = TestDir::new();
        let v = daemon(&dir, true);
        let mut t = test_env(&dir);
        t.password = Ok("falso-errada-01");
        assert_eq!(t.run(&["unlock"]), EXIT_ERROR);
        assert!(t.err().contains("senha mestra incorreta"), "{}", t.err());
        assert!(!t.err().contains("falso-errada-01") && !t.out().contains("falso-errada-01"));

        for (password, msg) in [
            (Ok(""), "senha vazia"),
            (Err(TerminalError::Canceled), "cancelado"),
            (Err(TerminalError::NoTerminal), "precisa de um terminal"),
            (Err(TerminalError::TooLong), "longa demais"),
        ] {
            let mut t = test_env(&dir);
            t.password = password;
            assert_eq!(t.run(&["unlock"]), EXIT_ERROR);
            assert!(t.err().contains(msg), "{}", t.err());
        }
        // Only the wrong password reached the daemon.
        let v = v.lock().unwrap();
        assert_eq!(v.passwords, [b"falso-errada-01".to_vec()]);
    }

    /// Attack: the socket is swapped (becomes 0666) while the user types. The check
    /// before sending refuses and the password does not go out.
    #[test]
    fn attack_socket_swapped_while_typing() {
        let dir = TestDir::new();
        let v = daemon(&dir, true);
        let mut t = test_env(&dir);
        let sock = dir.socket();
        t.before_password = Some(Box::new(move || {
            fs::set_permissions(&sock, fs::Permissions::from_mode(0o666)).unwrap();
        }));
        assert_eq!(t.run(&["unlock"]), EXIT_ERROR);
        assert!(t.err().contains("servidor não é o wardd"), "{}", t.err());
        let v = v.lock().unwrap();
        assert_eq!(v.operations, ["status"]);
        assert!(v.passwords.is_empty());
        assert!(!String::from_utf8_lossy(&v.bytes).contains("ZmFsc28tbWVzdHJh"));
    }

    /// Attack (S1): a server that sends a response with a credential (a secret) or a list
    /// with a level (D50) is refused on read; none of it reaches the output.
    #[test]
    fn attack_response_with_credential_or_level_refused() {
        for body in [
            r#"{"v":2,"type":"secret","data":{"value":"ZmFsc28tUEc6czNuaGEvMDFAIw=="}}"#,
            r#"{"v":2,"type":"list","data":{"items":[{"name":"a","type":"login","group":"g","environment":"dev","tags":[],"level":"free","address":"http://x"}]}}"#,
            r#"{"v":2,"type":"fill","data":{"username":"u","password":"ZmFsc28="}}"#,
            r#"{"v":2,"tipo":"ok"}"#,
        ] {
            let dir = TestDir::new();
            let body = body.as_bytes().to_vec();
            let _v = fake_server(&dir, Some(Arc::new(move |_: &Request| body.clone())));
            let mut t = test_env(&dir);
            assert_eq!(t.run(&["list", "--json"]), EXIT_ERROR);
            assert_eq!(t.err(), "ward: resposta inválida do daemon\n");
            assert!(t.out.is_empty());
        }
    }

    /// Attack m2:V4.16 (S5, m2:DT8): a server that answers in another protocol version
    /// (the 0.1.4 daemon, v1; a future one, v3). The CLI stops at the state response with
    /// the version message: it neither hardens nor reads the password, and the server
    /// only ever sees the state request (no byte of the password).
    #[test]
    fn attack_m2_v4_16_server_of_another_version_stops_before_the_password() {
        use crate::client::MSG_INCOMPATIBLE_VERSION;
        use crate::commands::EXIT_RUN_ERROR;
        for status in [
            r#"{"v":1,"tipo":"estado","dados":{"travado":true,"cofre_existe":true,"papel":"agente"}}"#,
            r#"{"v":1,"tipo":"erro","dados":{"codigo":"versao_desconhecida","mensagem":"versão desconhecida"}}"#,
            r#"{"v":3,"type":"status","data":{"locked":true,"vault_exists":true,"role":"agent"}}"#,
        ] {
            let dir = TestDir::new();
            let body = status.as_bytes().to_vec();
            let seen = fake_server(&dir, Some(Arc::new(move |_: &Request| body.clone())));
            for (cmd, code) in [
                (&["unlock"][..], EXIT_ERROR),
                (&["list", "--json"], EXIT_ERROR),
                (&["run", "acme-pg-dev", "--", "true"], EXIT_RUN_ERROR),
            ] {
                let mut t = test_env(&dir);
                assert_eq!(t.run(cmd), code, "{cmd:?} {status}");
                assert_eq!(
                    t.err(),
                    format!("ward: {MSG_INCOMPATIBLE_VERSION}\n"),
                    "{cmd:?} {status}"
                );
                assert!(t.out.is_empty());
                assert!(
                    !t.events.contains(&"endurecer") && !t.events.contains(&"ler_senha"),
                    "{cmd:?}: {:?}",
                    t.events
                );
            }
            let v = seen.lock().unwrap();
            // The unlock and run commands ask for the status; the list asks for the list.
            assert_eq!(v.operations, ["status", "list", "status"], "{status}");
            assert!(v.passwords.is_empty());
            assert!(!String::from_utf8_lossy(&v.bytes).contains("ZmFsc28tbWVzdHJh"));
            assert!(!contains_bytes(&v.bytes, PASSWORD.as_bytes()));
        }
    }

    /// m2:V4.16: the 0.2.0 daemon's own `unknown_version` error (a v2 frame) also stops
    /// the unlock before the password, with the protocol's fixed message.
    #[test]
    fn m2_v4_16_unknown_version_error_stops_before_the_password() {
        let dir = TestDir::new();
        let seen = fake_server(
            &dir,
            Some(Arc::new(|_: &Request| {
                encode(&Response::Error(ResponseError::UnknownVersion))
            })),
        );
        let mut t = test_env(&dir);
        assert_eq!(t.run(&["unlock"]), EXIT_ERROR);
        assert_eq!(
            t.err(),
            "ward: versão do protocolo incompatível: atualize o Ward\n"
        );
        assert_eq!(t.events, ["conectar"]);
        let v = seen.lock().unwrap();
        assert_eq!(v.operations, ["status"]);
        assert!(v.passwords.is_empty());
    }

    fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }
}

/// Phase 54b (D68): the kind of standard input by `fstat`: `/dev/null` is null, pipe and
/// socket are pipe, a regular file is file, `/dev/zero` is other; terminal only with
/// `isatty` on a character device (a file that claims to be a terminal does not
/// become one).
#[test]
fn f54b_stdin_kind_by_fstat() {
    use crate::commands::classify_stdin;
    use ward_core::protocol::StdinKind as T;
    let meta = |p: &str| std::fs::metadata(p).unwrap();
    assert_eq!(classify_stdin(Some(&meta("/dev/null")), false), T::Null);
    // `/dev/null` is never a terminal, even if `isatty` lied.
    assert_eq!(classify_stdin(Some(&meta("/dev/null")), true), T::Null);
    assert_eq!(classify_stdin(Some(&meta("/dev/zero")), false), T::Other);
    assert_eq!(classify_stdin(None, false), T::Null);
    let (reader, _writer) = std::io::pipe().unwrap();
    let f = std::fs::File::from(std::os::fd::OwnedFd::from(reader));
    assert_eq!(classify_stdin(Some(&f.metadata().unwrap()), false), T::Pipe);
    let (a, _b) = std::os::unix::net::UnixStream::pair().unwrap();
    let f = std::fs::File::from(std::os::fd::OwnedFd::from(a));
    assert_eq!(classify_stdin(Some(&f.metadata().unwrap()), false), T::Pipe);
    let file = std::env::current_exe().unwrap();
    let m = std::fs::metadata(&file).unwrap();
    assert_eq!(classify_stdin(Some(&m), false), T::File);
    assert_eq!(classify_stdin(Some(&m), true), T::File);
    if let Ok(m) = std::fs::metadata("/dev/ptmx") {
        assert_eq!(classify_stdin(Some(&m), true), T::Terminal);
    }
}
