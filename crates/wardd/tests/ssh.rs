//! `ward ssh` in the core (Phase 27), with fake `ssh`, `sshpass` and `sftp`
//! (`tests/fake_ssh.py`) that record the argv and never connect: D3 ("Connect"
//! token, V5.24, V4.32), D12 (agent only for the child, V4.24), D13 (`-F /dev/null`,
//! own `known_hosts`, environment from scratch), D34 revised (no prefix oracle), D43
//! (user with `@`, V4.35) and D56 (`sftp` with `User=`, V4.37).
//!
//! The session runs as in the server (`wardd::ssh::session` on one end of a socket
//! pair); the other end plays the CLI. The vault is a test one, created by the core
//! with fake values; the SSH key is generated here, with a random passphrase.
//!
//! The fake programs come in through the daemon builder for the SSH programs, which
//! only exists with the `test-hooks` feature (`make test` turns them all on).

#![cfg(feature = "test-hooks")]

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::*;
use ward_core::entry::Secret;
use ward_core::limits::MAX_COMMON_FRAME;
use ward_core::protocol::frame::{self, RESPONSE_LIMIT};
use ward_core::protocol::{self, Request, Response, ResponseError, Role, Stream, WindowSize};
use wardd::aprovacao::{Motor, RelogioMotor, SemApp, SemCliente};
use wardd::nucleo::Daemon;
use wardd::ssh::SshPrograms;
use zeroize::Zeroizing;

/// Password of the test servers (the one of the shared server fixture, fake).
const SSH_PASSWORD: &str = SECRETS[1];
/// Password of the tunnel database (fake).
const DB_PASSWORD: &str = SECRETS[2];

#[derive(Clone, Default)]
struct ManualClock(Arc<Mutex<Duration>>);

impl ManualClock {
    fn advance(&self, d: Duration) {
        *self.0.lock().unwrap() += d;
    }
}

impl RelogioMotor for ManualClock {
    fn agora(&self) -> Duration {
        *self.0.lock().unwrap()
    }
}

/// `<rt>` of this test only, in `/run/user/<uid>/` (short path: the agent socket fits
/// in `sun_path`).
struct Rt(PathBuf);

impl Rt {
    fn new(label: &str) -> Rt {
        let p = ward_platform::socket::runtime_dir()
            .join(format!("wsh-{}-{label}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        Rt(p)
    }
}

impl Drop for Rt {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Scene {
    d: Arc<Daemon>,
    dir: Dir,
    rt: Rt,
    /// Where the fake programs, the mode file and the records live.
    base: PathBuf,
    clock: ManualClock,
}

fn save(d: &Daemon, json: &str) {
    let r = request(d, Role::App, "salvar_entrada", json);
    assert!(ok(&r), "{r:?}");
}

fn server_json(name: &str, user: &str, auth_fields: &str, extra: &str) -> String {
    format!(
        r#"{{"entrada":{{"nome":"{name}","grupo":"clientex","ambiente":"stage","tipo":"servidor","campos":{{"host":"127.0.0.1","porta":"2222","usuario":"{user}",{auth_fields}{extra}}}}}}}"#
    )
}

fn sftp_json(name: &str, protocol: &str, auth_fields: &str) -> String {
    format!(
        r#"{{"entrada":{{"nome":"{name}","grupo":"clientex","ambiente":"dev","tipo":"ftp-sftp","campos":{{"protocolo":"{protocol}","host":"127.0.0.1","porta":"2222","usuario":"sftpuser",{auth_fields}}}}}}}"#
    )
}

fn password_auth() -> String {
    format!(r#""senha":"{}""#, b64(SSH_PASSWORD.as_bytes()))
}

/// New Ed25519 key with a random passphrase (nothing fixed in the repository).
fn generate_key(dir: &Path) -> (Vec<u8>, String) {
    let pp = format!(
        "pp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let file = dir.join("chave_teste");
    let st = Command::new("/usr/bin/ssh-keygen")
        .env_clear()
        .env("HOME", dir)
        .env("PATH", "/usr/bin:/bin")
        .args(["-q", "-t", "ed25519", "-C", "teste-fase27", "-N"])
        .arg(&pp)
        .arg("-f")
        .arg(&file)
        .status()
        .unwrap();
    assert!(st.success());
    let key = fs::read(&file).unwrap();
    fs::remove_file(&file).unwrap();
    let _ = fs::remove_file(file.with_extension("pub"));
    (key, pp)
}

fn scene(label: &str) -> Scene {
    let dir = Dir::new(label);
    let (d, _) = test_vault(&dir);
    let base = dir.0.join("falsos");
    fs::create_dir(&base).unwrap();
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fake_ssh.py");
    for name in ["ssh", "sshpass", "sftp"] {
        let p = base.join(name);
        fs::write(
            &p,
            format!(
                "#!/bin/sh\nexec /usr/bin/python3 -I '{}' {name} '{}' \"$@\"\n",
                script.display(),
                base.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let rt = Rt::new(label);
    let clock = ManualClock::default();
    let d = d
        .com_motor(Motor::novo(Arc::new(clock.clone()), Box::new(SemApp)))
        .com_programas_ssh(SshPrograms {
            ssh: base.join("ssh"),
            sshpass: base.join("sshpass"),
            sftp: base.join("sftp"),
        })
        .com_temporarios(rt.0.clone());
    let (key, pp) = generate_key(&dir.0);
    let key_auth = format!(
        r#""chave_privada":"{}","passphrase":"{}""#,
        b64(&key),
        b64(pp.as_bytes())
    );
    save(
        &d,
        &server_json("s-livre", "deploy@cliente.com.br", &password_auth(), ""),
    );
    save(&d, &server_json("s-outro", "deploy", &password_auth(), ""));
    save(&d, &server_json("s-bloq", "deploy", &password_auth(), ""));
    save(&d, &server_json("s-chave", "deploy", &key_auth, ""));
    save(
        &d,
        &server_json(
            "s-salto",
            "deploy",
            &password_auth(),
            r#","salto":"s-livre""#,
        ),
    );
    // Phase 28: jump with a key on the main host, jump with a key on the intermediate,
    // jump that needs approval or is blocked (D19) and the database tunnel.
    save(
        &d,
        &server_json(
            "s-salto-chave",
            "deploy",
            &key_auth,
            r#","salto":"s-livre""#,
        ),
    );
    save(
        &d,
        &server_json(
            "s-via-chave",
            "deploy",
            &password_auth(),
            r#","salto":"s-chave""#,
        ),
    );
    save(
        &d,
        &server_json(
            "s-salto-aprov",
            "deploy",
            &password_auth(),
            r#","salto":"s-outro""#,
        ),
    );
    save(
        &d,
        &server_json(
            "s-salto-bloq",
            "deploy",
            &password_auth(),
            r#","salto":"s-bloq""#,
        ),
    );
    save(
        &d,
        &format!(
            r#"{{"entrada":{{"nome":"b-tunel","grupo":"clientex","ambiente":"stage","tipo":"banco","campos":{{"motor":"postgres","host":"db.interno","porta":"5432","base":"app","usuario":"app","senha":"{}","tunel":"s-livre"}}}}}}"#,
            b64(DB_PASSWORD.as_bytes())
        ),
    );
    save(&d, &sftp_json("f-sftp", "sftp", &password_auth()));
    save(&d, &sftp_json("f-sftp-chave", "sftp", &key_auth));
    save(&d, &sftp_json("f-ftp", "ftp", &password_auth()));
    for n in [
        "s-livre",
        "s-chave",
        "s-salto",
        "s-salto-chave",
        "s-via-chave",
        "s-salto-aprov",
        "s-salto-bloq",
        "b-tunel",
        "f-sftp",
        "f-sftp-chave",
        "f-ftp",
    ] {
        assert!(ok(&set_level(&d, Role::App, n, "livre", PIN)));
    }
    assert!(ok(&set_level(&d, Role::App, "s-bloq", "bloqueado", PIN)));
    Scene {
        d: Arc::new(d),
        dir,
        rt,
        base,
        clock,
    }
}

impl Scene {
    fn set_mode(&self, m: &str) {
        fs::write(self.base.join("modo"), m).unwrap();
    }

    /// The records of the fake program `name`.
    fn records(&self, name: &str) -> Vec<Rec> {
        let mut v = Vec::new();
        for e in fs::read_dir(&self.base).unwrap() {
            let n = e.unwrap().file_name().into_string().unwrap();
            if n.starts_with(&format!("registro-{name}-")) && n.ends_with(".txt") {
                let t = fs::read_to_string(self.base.join(&n)).unwrap();
                v.push(Rec(t
                    .lines()
                    .map(|l| {
                        let (k, v) = l.split_once('\t').unwrap();
                        (k.to_owned(), v.to_owned())
                    })
                    .collect()));
            }
        }
        v
    }

    fn record(&self, name: &str) -> Rec {
        let r = self.records(name);
        assert_eq!(r.len(), 1, "registros de {name}: {r:?}");
        r.into_iter().next().unwrap()
    }

    fn nothing_ran(&self) -> bool {
        ["ssh", "sshpass", "sftp"]
            .iter()
            .all(|n| self.records(n).is_empty())
    }

    fn token(&self, entry: &str) -> Secret {
        match self.d.atender(
            Role::App,
            Request::IssueToken {
                entry: entry.into(),
            },
        ) {
            Response::Token(t) => t,
            other => panic!("emitir_token: {other:?}"),
        }
    }
}

fn ssh_request(
    entry: &str,
    command: &[&str],
    token: Option<&Secret>,
    window: Option<WindowSize>,
) -> Request {
    Request::Ssh {
        entry: entry.into(),
        command: command
            .iter()
            .map(|c| Zeroizing::new(c.as_bytes().to_vec()))
            .collect(),
        token: token.map(|t| Secret::from_bytes(t.expose().to_vec())),
        window_size: window,
        // Phase 54b: the tests send the input over the socket (like a pipe), or type
        // it in the pty.
        stdin_kind: if window.is_some() {
            ward_core::protocol::StdinKind::Terminal
        } else {
            ward_core::protocol::StdinKind::Pipe
        },
    }
}

/// What the session returned, with the time of each output chunk.
#[derive(Debug, Default)]
struct Session {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    chunks: Vec<(Duration, Vec<u8>)>,
    end: Option<u8>,
    failure: Option<ResponseError>,
}

impl Session {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
}

fn send(s: &mut UnixStream, p: &Request) {
    let body = protocol::encode_request(Role::Agent, p).unwrap();
    frame::write_frame(s, &body, MAX_COMMON_FRAME).unwrap();
}

/// Runs the session as the server would. `cli` gets the CLI end right after the
/// request (sends input, window, touches the vault) and the session is read to the end.
fn session(c: &Scene, request: Request, cli: impl FnOnce(&mut UnixStream)) -> Session {
    session_of(c, request, cli, false)
}

/// Like [`session`]; `run`: the `ward run` session (`wardd::execution::session`).
fn session_of(
    c: &Scene,
    request: Request,
    cli: impl FnOnce(&mut UnixStream),
    run: bool,
) -> Session {
    let (mut cli_end, daemon_end) = UnixStream::pair().unwrap();
    cli_end
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    let d = c.d.clone();
    let t = std::thread::spawn(move || {
        if run {
            wardd::execution::session(&d, &SemCliente, &daemon_end, request)
        } else {
            wardd::ssh::session(&d, &SemCliente, &daemon_end, request)
        }
    });
    let start = Instant::now();
    cli(&mut cli_end);
    let mut r = Session::default();
    loop {
        let Ok(b) = frame::read_frame(&mut cli_end, RESPONSE_LIMIT) else {
            break;
        };
        match protocol::decode_response(Role::Agent, &b).unwrap() {
            Response::Output {
                stream: fluxo,
                data: dados,
            } => {
                r.chunks.push((start.elapsed(), dados.to_vec()));
                match fluxo {
                    Stream::Stdout => r.stdout.extend_from_slice(&dados),
                    Stream::Stderr => r.stderr.extend_from_slice(&dados),
                }
            }
            Response::Exit { code: codigo } => {
                r.end = Some(codigo);
                break;
            }
            Response::Error(e) => {
                r.failure = Some(e);
                break;
            }
            other => panic!("resposta inesperada: {other:?}"),
        }
    }
    let continued = t.join().unwrap();
    // Refused before launching: the connection goes on; after the session, it ends.
    assert_eq!(continued, r.failure.is_some(), "{r:?}");
    r
}

/// Closes the command input. When the request is refused before launching (invalid
/// token, level), the daemon may have answered and let go of its end already: a write
/// that fails there is not a test error (the answer is checked afterwards).
fn close_stdin(s: &mut UnixStream) {
    let body = protocol::encode_request(
        Role::Agent,
        &Request::Stdin {
            data: Secret::from_bytes(Vec::new()),
        },
    )
    .unwrap();
    let _ = frame::write_frame(s, &body, MAX_COMMON_FRAME);
}

/// The record of a fake program: `field → values`, in order.
#[derive(Debug)]
struct Rec(Vec<(String, String)>);

impl Rec {
    fn all(&self, field: &str) -> Vec<String> {
        self.0
            .iter()
            .filter(|(k, _)| k == field)
            .map(|(_, v)| v.clone())
            .collect()
    }

    /// The last value of the field (the agent record is written again with `depois`).
    fn one(&self, field: &str) -> String {
        self.0
            .iter()
            .rev()
            .find(|(k, _)| k == field)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| panic!("sem {field}: {self:?}"))
    }
}

fn argv(rec: &Rec) -> Vec<String> {
    rec.all("argv")
}

/// The names of the environment, minus the `PWD` that the `/bin/sh` of the fake program
/// sets.
fn env(rec: &Rec) -> BTreeSet<String> {
    rec.all("env").into_iter().filter(|n| n != "PWD").collect()
}

fn options(a: &[String]) -> Vec<String> {
    a.windows(2)
        .filter(|w| w[0] == "-o")
        .map(|w| w[1].clone())
        .collect()
}

fn sha256(data: &[u8]) -> String {
    let out = Command::new("/usr/bin/sha256sum")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut c| {
            use std::io::Write;
            c.stdin.take().unwrap().write_all(data)?;
            c.wait_with_output()
        })
        .unwrap();
    String::from_utf8(out.stdout).unwrap()[..64].to_owned()
}

fn wait_for(deadline: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + deadline;
    while Instant::now() < end {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    cond()
}

fn rt_without_agent(c: &Scene) -> bool {
    fs::read_dir(&c.rt.0).map_or(true, |d| {
        d.filter_map(Result::ok)
            .all(|e| !e.file_name().to_string_lossy().starts_with("ssh-"))
    })
}

/// D43 + D13 (V4.35, V4.23): the user with `@` whole after `-l`, the host after `--`,
/// never `user@host` nor `-o User=`; the password only in `SSHPASS` (never in argv);
/// `-F /dev/null`, the Ward `known_hosts` and an environment from scratch; output that
/// repeats the password comes back masked; the code of the remote command passes.
#[test]
fn v4_35_d43_user_with_at_sign_and_password_only_in_env() {
    let c = scene("d43");
    c.set_mode("normal");
    let r = session(
        &c,
        ssh_request("s-livre", &["whoami", "--", "-l", "x@y"], None, None),
        close_stdin,
    );
    assert_eq!(r.end, Some(7), "{r:?}");
    let sp = c.record("sshpass");
    let ssh = c.record("ssh");
    let a = argv(&sp);
    assert_eq!(a[0], "-e");
    assert_eq!(a[1], c.base.join("ssh").to_str().unwrap());
    assert_eq!(&argv(&ssh)[..], &a[2..]);
    let a = &a[2..];
    assert_eq!(&a[..2], ["-F", "/dev/null"]);
    // The whole user, as its own argument of `-l`, before the `--`.
    let end_of_options = a.iter().position(|x| x == "--").unwrap();
    assert_eq!(
        &a[end_of_options - 4..],
        [
            "-p",
            "2222",
            "-l",
            "deploy@cliente.com.br",
            "--",
            "127.0.0.1",
            "whoami",
            "--",
            "-l",
            "x@y"
        ]
    );
    for x in a {
        assert!(
            !x.contains("@127.0.0.1") && !x.contains("@cliente.com.br@"),
            "{x}"
        );
        assert!(!x.starts_with("User=") && !x.contains(SSH_PASSWORD), "{x}");
    }
    let o = options(a);
    let kh = c.dir.0.join("known_hosts");
    for expected in [
        "IdentitiesOnly=yes".to_owned(),
        "CertificateFile=/dev/null".to_owned(),
        format!("UserKnownHostsFile={}", kh.display()),
        "GlobalKnownHostsFile=/dev/null".to_owned(),
        "StrictHostKeyChecking=accept-new".to_owned(),
        "PubkeyAuthentication=no".to_owned(),
        "IdentityAgent=none".to_owned(),
        "IdentityFile=none".to_owned(),
        "ForwardAgent=no".to_owned(),
        "ControlMaster=no".to_owned(),
        "ProxyCommand=none".to_owned(),
    ] {
        assert!(o.contains(&expected), "falta {expected}: {o:?}");
    }
    // Environment from scratch: nothing from the test nor from the daemon (no
    // SSH_AUTH_SOCK, no HOME).
    let expected: BTreeSet<String> = ["LANG", "PATH", "SSHPASS"].map(String::from).into();
    assert_eq!(env(&sp), expected);
    assert_eq!(sp.one("sshpass_sha256"), sha256(SSH_PASSWORD.as_bytes()));
    assert_eq!(sp.one("tty"), "0");
    // S4: the child printed the password; the CLI only sees the mark.
    let t = r.text();
    assert!(t.contains("SSHPASS=••••"), "{t}");
    assert!(
        !contains(&r.stdout, SSH_PASSWORD.as_bytes())
            && !contains(&r.stderr, SSH_PASSWORD.as_bytes())
    );
    assert!(t.contains("remoto=whoami -- -l x@y"), "{t}");
}

/// D56 (V4.37): in `sftp`, `-o User=<user>`, `-P`, `-F /dev/null` and the host after
/// `--`; never `-l` (bandwidth limit in `sftp`). The commands come from stdin; `sftp`
/// with a command is refused without running anything.
#[test]
fn v4_37_d56_sftp_with_user_and_host_after_double_dash() {
    let c = scene("d56");
    let r = session(&c, ssh_request("f-sftp", &[], None, None), |s| {
        send(
            s,
            &Request::Stdin {
                data: Secret::from_bytes(b"pwd\nls\n".to_vec()),
            },
        );
        close_stdin(s);
    });
    assert_eq!(r.end, Some(0), "{r:?}");
    assert_eq!(r.text(), "sftp> pwd\nsftp> ls\n");
    let a = argv(&c.record("sshpass"));
    assert_eq!(&a[..2], ["-e", c.base.join("sftp").to_str().unwrap()]);
    let a = &a[2..];
    assert_eq!(&a[..2], ["-F", "/dev/null"]);
    assert_eq!(
        &a[a.len() - 6..],
        ["-o", "User=sftpuser", "-P", "2222", "--", "127.0.0.1"]
    );
    assert!(!a.iter().any(|x| x == "-l" || x.contains('@')), "{a:?}");

    let c = scene("d56b");
    let r = session(&c, ssh_request("f-sftp", &["ls"], None, None), |_| {});
    assert_eq!(r.failure, Some(ResponseError::InvalidRequest));
    assert!(c.nothing_ran());
}

/// Refusals before running: a kind without `ward ssh`, `ftp` (not `sftp`), blocked
/// (nonexistent), needs approval without the app (the level holds) and the combined
/// level of D19 (V4.30): free with a jump that "needs approval" without the app gives
/// approval unavailable, and free with a blocked jump is denied without asking.
#[test]
fn refusals_without_running_anything() {
    let c = scene("recusas");
    for (entry, error) in [
        ("t-livre", ResponseError::TypeWithoutDelivery),
        ("f-ftp", ResponseError::TypeWithoutDelivery),
        ("s-bloq", ResponseError::EntryNotFound),
        ("nao-existe", ResponseError::EntryNotFound),
        ("s-outro", ResponseError::ApprovalUnavailable),
        ("s-salto-aprov", ResponseError::ApprovalUnavailable),
        ("s-salto-bloq", ResponseError::Denied),
    ] {
        let r = session(&c, ssh_request(entry, &["id"], None, None), |_| {});
        assert_eq!(r.failure, Some(error), "{entry}: {r:?}");
    }
    assert!(c.nothing_ran());
    assert!(rt_without_agent(&c));
}

/// D12 (V4.24, attack): the agent lists the key and signs only for the child `ssh`;
/// it refuses to add, remove and lock; a grandchild of `ssh` and the test itself (same
/// uid, another pid) are closed without a single byte. Folder 0700, socket 0600, no
/// `SSH_AUTH_SOCK` in the environment; everything goes away at the end of the session.
#[test]
fn v4_24_d12_agent_only_for_the_child() {
    let c = scene("d12");
    c.set_mode("normal");
    fs::write(c.base.join("esperar"), "").unwrap();
    let base = c.base.clone();
    let rt = c.rt.0.clone();
    let r = session(
        &c,
        ssh_request("s-chave", &["true"], None, None),
        move |s| {
            close_stdin(s);
            // Waits for the fake `ssh` to record the argv and attacks the socket from here.
            assert!(wait_for(Duration::from_secs(30), || {
                fs::read_dir(&base).unwrap().any(|e| {
                    e.unwrap()
                        .file_name()
                        .to_string_lossy()
                        .starts_with("registro-ssh-")
                })
            }));
            let dir = fs::read_dir(&rt)
                .unwrap()
                .filter_map(Result::ok)
                .find(|e| e.file_name().to_string_lossy().starts_with("ssh-"))
                .unwrap()
                .path();
            let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&dir), 0o700);
            assert_eq!(mode(&dir.join("agent.sock")), 0o600);
            assert_eq!(mode(&dir.join("id.pub")), 0o600);
            let public_key = fs::read_to_string(dir.join("id.pub")).unwrap();
            assert!(public_key.starts_with("ssh-ed25519 "), "{public_key}");
            // Attack: another process of the same user.
            let mut a = UnixStream::connect(dir.join("agent.sock")).unwrap();
            a.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            use std::io::{Read, Write};
            let _ = a.write_all(&[0, 0, 0, 1, 11]);
            let mut buf = [0u8; 16];
            assert_eq!(
                a.read(&mut buf).unwrap_or(0),
                0,
                "o agente respondeu a um estranho"
            );
            fs::write(base.join("continuar"), "").unwrap();
        },
    );
    assert_eq!(r.end, Some(7), "{r:?}");
    let ssh = c.record("ssh");
    assert_eq!(ssh.one("agente.lista"), "12", "{ssh:?}");
    assert_eq!(ssh.one("agente.quantas"), "1");
    assert_eq!(ssh.one("agente.assina"), "14", "{ssh:?}");
    assert!(ssh.one("agente.assinatura").len() > 100);
    for op in ["adiciona", "remove", "trava"] {
        assert_eq!(ssh.one(&format!("agente.{op}")), "5", "{op}: {ssh:?}");
    }
    assert_eq!(ssh.one("agente.neto"), "recusada", "{ssh:?}");
    assert_eq!(ssh.one("depois"), "12");
    let a = argv(&ssh);
    let o = options(&a);
    assert!(o.contains(&"PubkeyAuthentication=yes".to_owned()));
    assert!(o.contains(&"IdentitiesOnly=yes".to_owned()));
    assert!(o.contains(&"PasswordAuthentication=no".to_owned()));
    assert!(
        o.iter()
            .any(|x| x.starts_with("IdentityAgent=") && x.ends_with("/agent.sock"))
    );
    assert!(
        o.iter()
            .any(|x| x.starts_with("IdentityFile=") && x.ends_with("/id.pub"))
    );
    assert!(c.records("sshpass").is_empty());
    let expected: BTreeSet<String> = ["LANG", "PATH"].map(String::from).into();
    assert_eq!(env(&ssh), expected);
    // End of the session: the agent and the folder go away.
    assert!(rt_without_agent(&c));
    let sock = o
        .iter()
        .find_map(|x| x.strip_prefix("IdentityAgent="))
        .unwrap();
    assert!(!Path::new(sock).exists());
}

/// D12: locking the vault brings down the agent in the middle of the session; since
/// Phase 54c (S8), the whole session (the `ssh` dies before asking the agent again, and
/// the CLI gets 137).
#[test]
fn d12_lock_brings_down_the_agent() {
    let c = scene("d12trava");
    fs::write(c.base.join("esperar"), "").unwrap();
    let (base, d, rt) = (c.base.clone(), c.d.clone(), c.rt.0.clone());
    let r = session(
        &c,
        ssh_request("s-chave", &["true"], None, None),
        move |s| {
            close_stdin(s);
            assert!(wait_for(Duration::from_secs(30), || {
                fs::read_dir(&base).unwrap().any(|e| {
                    e.unwrap()
                        .file_name()
                        .to_string_lossy()
                        .starts_with("registro-ssh-")
                })
            }));
            d.travar();
            assert!(wait_for(Duration::from_secs(5), || {
                fs::read_dir(&rt).map_or(true, |d| {
                    d.filter_map(Result::ok)
                        .all(|e| !e.file_name().to_string_lossy().starts_with("ssh-"))
                })
            }));
            fs::write(base.join("continuar"), "").unwrap();
        },
    );
    assert_eq!(r.end, Some(137), "{r:?}");
    let ssh = c.record("ssh");
    assert_eq!(ssh.one("agente.lista"), "12");
    assert!(
        ssh.0.iter().all(|(k, v)| k != "depois" || v != "12"),
        "{ssh:?}"
    );
}

/// Phase 54c (S8; user decision, 2026-11-10), attack: the agent opens `ward ssh` and the
/// session stays up (the fake `ssh` sleeps 30 s, connected). The user locks the vault:
/// the session dies at once, by password (the `ssh` that `sshpass` creates with
/// `setsid()`, outside the group, too) and by key; the CLI gets the notice and 137, and
/// nothing the `ssh` would do afterwards arrives. Before the fix, the session went on to
/// the end (code 7, 30 s).
#[test]
fn f54c_attack_ward_ssh_dies_on_lock() {
    for entry in ["s-livre", "s-chave"] {
        let c = scene(&format!("f54c-{entry}"));
        c.set_mode("parado");
        let locked = std::cell::Cell::new(None);
        let r = session(&c, ssh_request(entry, &["sleep", "20"], None, None), |s| {
            close_stdin(s);
            assert!(wait_for(Duration::from_secs(30), || {
                !c.records("ssh").is_empty()
            }));
            std::thread::sleep(Duration::from_millis(300));
            c.d.travar();
            locked.set(Some(Instant::now()));
        });
        let after = locked.get().unwrap().elapsed();
        assert_eq!(r.end, Some(137), "{entry}: {r:?}");
        assert!(after < Duration::from_secs(5), "{entry}: {after:?}");
        let t = format!("{}{}", r.text(), String::from_utf8_lossy(&r.stderr));
        assert!(t.contains("conectado"), "{entry}: {t}");
        assert!(
            t.contains("ward: o cofre travou; o comando foi encerrado"),
            "{entry}: {t}"
        );
        assert!(!t.contains("depois-da-trava"), "{entry}: {t}");
        // No process of the session is left: neither the `ssh` (child of `sshpass` with
        // its own session, for a password) nor the `sshpass`.
        for name in ["ssh", "sshpass"] {
            for rec in c.records(name) {
                let pid = rec.one("pid");
                assert!(
                    wait_for(Duration::from_secs(5), || {
                        fs::read_to_string(format!("/proc/{pid}/stat"))
                            .map_or(true, |s| s.contains(") Z "))
                    }),
                    "{entry}: {name} {pid} sobreviveu à trava"
                );
            }
        }
        if entry == "s-livre" {
            assert_eq!(c.records("sshpass").len(), 1);
        }
        assert!(rt_without_agent(&c));
    }
}

/// Phase 54l (finding of 54c; S8, D13), attack: the `ward ssh` CLI drops in the middle
/// of the open session (closes the socket, like a SIGKILL on it). For a password,
/// `sshpass` creates the `ssh` with `setsid()`, outside the group; the fake one does not
/// die on `SIGHUP`. Before, the end of the session killed only the `sshpass` group and
/// that `ssh` stayed alive, with the authenticated session, until the end of the
/// command. Now no process of the session is left, with pipes and with a pty, by
/// password and by key, and the daemon lets go of the session soon.
#[test]
fn f54l_attack_cli_drops_in_ward_ssh_no_ssh_left() {
    let window = WindowSize {
        rows: 30,
        cols: 100,
    };
    for (entry, pty) in [("s-livre", false), ("s-livre", true), ("s-chave", false)] {
        let label = format!("{entry}{}", if pty { "-pty" } else { "" });
        let c = scene(&format!("f54l-{label}"));
        c.set_mode("parado");
        let request = if pty {
            ssh_request(entry, &[], None, Some(window))
        } else {
            ssh_request(entry, &["sleep", "20"], None, None)
        };
        let (mut cli_end, daemon_end) = UnixStream::pair().unwrap();
        let d = c.d.clone();
        let t = std::thread::spawn(move || {
            let _ = wardd::ssh::session(&d, &SemCliente, &daemon_end, request);
        });
        close_stdin(&mut cli_end);
        assert!(
            wait_for(Duration::from_secs(30), || !c.records("ssh").is_empty()),
            "{label}: o ssh falso não subiu"
        );
        std::thread::sleep(Duration::from_millis(300));
        let dropped = Instant::now();
        drop(cli_end);
        // No process of the session is left: neither the `ssh` (with its own session,
        // for a password) nor the `sshpass`.
        for name in ["ssh", "sshpass"] {
            for rec in c.records(name) {
                let pid = rec.one("pid");
                assert!(
                    wait_for(Duration::from_secs(5), || {
                        fs::read_to_string(format!("/proc/{pid}/stat"))
                            .map_or(true, |s| s.contains(") Z "))
                    }),
                    "{label}: {name} {pid} sobreviveu à CLI que caiu"
                );
            }
        }
        t.join().unwrap();
        let dt = dropped.elapsed();
        assert!(dt < Duration::from_secs(10), "{label}: {dt:?}");
        if entry == "s-livre" {
            assert_eq!(c.records("sshpass").len(), 1, "{label}");
        }
        assert!(rt_without_agent(&c), "{label}");
    }
}

/// D12 in `sftp` with a key: the `ssh` that `sftp` calls (child of the child) is
/// served; its grandchild is not.
#[test]
fn d12_sftp_with_key_serves_the_ssh_of_sftp() {
    let c = scene("d12sftp");
    let r = session(
        &c,
        ssh_request("f-sftp-chave", &[], None, None),
        close_stdin,
    );
    assert_eq!(r.end, Some(0), "{r:?}");
    let ssh = c.record("ssh");
    assert_eq!(ssh.one("agente.lista"), "12", "{ssh:?}");
    assert_eq!(ssh.one("agente.assina"), "14");
    assert_eq!(ssh.one("agente.neto"), "recusada");
    assert_eq!(ssh.one("ppid"), c.record("sftp").one("pid"));
    assert!(rt_without_agent(&c));
}

/// D34 revised (M4 audit, prefix oracle in the pty): the child prints a wrong prefix of
/// 10 characters and stops 1 s, then the right prefix of the password and stops 1 s,
/// then the rest. Both prefixes reach the CLI the same way: in clear and at once (no
/// `••••` in place of the right one, nothing held until the pause or until the rest).
/// The chunk that completes the password comes out marked. The child has a terminal of
/// the requested size, `TERM`, and follows the new window.
#[test]
fn d34_attack_pty_right_and_wrong_prefix_come_out_alike_and_at_once() {
    let c = scene("d34");
    c.set_mode("pty");
    let window = WindowSize {
        rows: 30,
        cols: 100,
    };
    let ready = c.base.join("pty-pronto");
    let r = session(
        &c,
        ssh_request("s-livre", &[], None, Some(window)),
        move |s| {
            assert!(wait_for(Duration::from_secs(60), || ready.exists()));
            send(
                s,
                &Request::Resize(WindowSize {
                    rows: 40,
                    cols: 120,
                }),
            );
        },
    );
    assert_eq!(r.end, Some(0), "{r:?}");
    let t = r.text();
    let timeline = || {
        r.chunks
            .iter()
            .map(|(t, p)| (t, String::from_utf8_lossy(p).into_owned()))
            .collect::<Vec<_>>()
    };
    assert!(t.contains("tty=True/True 30x100"), "{t}");
    assert!(t.contains("fim 40x120"), "{t}");
    let right = format!("[{}", &SSH_PASSWORD[..10]);
    let wrong: String = SSH_PASSWORD[..10]
        .chars()
        .map(|a| if a == 'X' { 'Y' } else { 'X' })
        .collect();
    let wrong = format!("<{wrong}");
    // Both prefixes come out in clear, as the child wrote them.
    assert!(t.contains(&wrong), "{t}");
    assert!(t.contains(&right), "o prefixo certo foi trocado: {t}");
    // The whole password never comes out; the chunk that completes it becomes a mark.
    assert!(!contains(&r.stdout, SSH_PASSWORD.as_bytes()), "{t}");
    assert!(t.contains(&format!("{right}••••]")), "{t}");
    // Timing: each prefix reaches the CLI before the 1 s pause ends (nothing held until
    // the next output); the arrival does not depend on it being the right prefix.
    let when = |mark: &str| {
        r.chunks
            .iter()
            .find(|(_, p)| contains(p, mark.as_bytes()))
            .map(|(t, _)| *t)
            .unwrap_or_else(|| panic!("sem {mark}: {:?}", timeline()))
    };
    let (t_wrong, t_close) = (when(&wrong), when(">"));
    let (t_right, t_rest) = (when(&right), when("••••"));
    for (name, t0, t1) in [("errado", t_wrong, t_close), ("certo", t_right, t_rest)] {
        assert!(
            t1.saturating_sub(t0) >= Duration::from_millis(600),
            "prefixo {name} segurado até a saída seguinte: {:?}",
            timeline()
        );
    }
    let sp = c.record("sshpass");
    assert!(env(&sp).contains("TERM"));
    // Interactive: no remote command (nothing after the host).
    let a = argv(&sp);
    assert_eq!(a.last().unwrap(), "127.0.0.1");
}

/// D34 revised (M4 audit, timing script): without a pty (`ward ssh` with a command), the
/// child writes `A`, stops, a prefix candidate, stops, and `Z`. The candidate reaches
/// the CLI before the `Z`, whether or not it is the start of the password (before, the
/// start of the password was held until the `Z`: a timing oracle).
#[test]
fn d34_attack_timing_without_pty_prefix_not_delayed() {
    let c = scene("d34tempo");
    c.set_mode("tempo");
    let r = session(&c, ssh_request("s-livre", &["x"], None, None), close_stdin);
    assert_eq!(r.end, Some(0), "{r:?}");
    let timeline: Vec<_> = r
        .chunks
        .iter()
        .map(|(t, p)| (*t, String::from_utf8_lossy(p).into_owned()))
        .collect();
    let when = |mark: &str| {
        timeline
            .iter()
            .find(|(_, p)| p.contains(mark))
            .map(|(t, _)| *t)
            .unwrap_or_else(|| panic!("sem {mark}: {timeline:?}"))
    };
    for i in 0..4 {
        let (t_candidate, t_z) = (when(&format!("C{i}=")), when(&format!("Z{i}")));
        assert!(
            t_z.saturating_sub(t_candidate) >= Duration::from_millis(400),
            "candidato {i} segurado até o Z: {timeline:?}"
        );
    }
    assert!(!contains(&r.stdout, SSH_PASSWORD.as_bytes()));
}

/// M4 audit (low): a loose Ward `known_hosts` (another user could read the hosts or,
/// with write access, plant a host key) refuses before running anything; if it does
/// not exist, it is born 0600.
#[test]
fn attack_loose_known_hosts_refused_without_running() {
    let c = scene("khfrouxo");
    c.set_mode("normal");
    let kh = c.dir.0.join("known_hosts");
    let r = session(
        &c,
        ssh_request("s-livre", &["true"], None, None),
        close_stdin,
    );
    assert_eq!(r.end, Some(7), "{r:?}");
    assert_eq!(
        fs::metadata(&kh).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let before = c.records("ssh").len();
    for mode in [0o644, 0o660, 0o606] {
        fs::set_permissions(&kh, fs::Permissions::from_mode(mode)).unwrap();
        let r = session(
            &c,
            ssh_request("s-livre", &["true"], None, None),
            close_stdin,
        );
        assert_eq!(
            r.failure,
            Some(ResponseError::DeliveryFailed),
            "{mode:o}: {r:?}"
        );
    }
    fs::set_permissions(&kh, fs::Permissions::from_mode(0o600)).unwrap();
    fs::set_permissions(&c.dir.0, fs::Permissions::from_mode(0o755)).unwrap();
    let r = session(
        &c,
        ssh_request("s-livre", &["true"], None, None),
        close_stdin,
    );
    fs::set_permissions(&c.dir.0, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(r.failure, Some(ResponseError::DeliveryFailed), "{r:?}");
    assert_eq!(
        c.records("ssh").len(),
        before,
        "rodou com known_hosts frouxo"
    );
}

/// M4 audit (high): in `sftp` with a key, the `!cmd` of `sftp` is another child of it
/// and used the agent (`ssh-add -L`, login with the key on another host). The agent is
/// held to the `ssh` that `sftp` created: the `!cmd` is refused with the `ssh` alive and
/// after it has exited; the `ssh` is still served.
#[test]
fn attack_sftp_bang_does_not_use_the_agent() {
    let c = scene("sftpbang");
    c.set_mode("sftp-bang");
    fs::write(c.base.join("esperar"), "").unwrap();
    let r = session(
        &c,
        ssh_request("f-sftp-chave", &[], None, None),
        close_stdin,
    );
    assert_eq!(r.end, Some(0), "{r:?}");
    let ssh = c.record("ssh");
    assert_eq!(ssh.one("agente.lista"), "12", "{ssh:?}");
    assert_eq!(
        ssh.one("depois"),
        "12",
        "o ssh do sftp perdeu o agente: {ssh:?}"
    );
    let sftp = c.record("sftp");
    assert_eq!(sftp.one("bang_durante"), "recusada", "{sftp:?}");
    assert_eq!(sftp.one("bang_depois"), "recusada", "{sftp:?}");
    assert!(rt_without_agent(&c));
}

/// D3 / V5.24 (attacks): the "Connect" token is single use, lasts 30 s, only for its
/// entry, and only the app issues it. Invalid, expired, repeated or of another entry:
/// the level holds (here, needs approval without the app: approval unavailable),
/// nothing runs.
#[test]
fn v5_24_connect_token() {
    let c = scene("token");
    c.set_mode("normal");
    let run = |token: Option<&Secret>, entry: &str| {
        session(&c, ssh_request(entry, &["true"], token, None), close_stdin)
    };
    let unavailable = Some(ResponseError::ApprovalUnavailable);

    // Only the app issues; no token for a kind without `ward ssh`.
    for role in [Role::Agent, Role::Extension] {
        let r = c.d.atender(
            role,
            Request::IssueToken {
                entry: "s-outro".into(),
            },
        );
        assert_eq!(error(&r), Some(ResponseError::Unauthorized), "{role:?}");
    }
    let r = c.d.atender(
        Role::App,
        Request::IssueToken {
            entry: "t-livre".into(),
        },
    );
    assert_eq!(error(&r), Some(ResponseError::InvalidRequest));

    // Valid: skips the approval once.
    let t = c.token("s-outro");
    assert_eq!(t.expose().len(), 32);
    assert!(t.expose().iter().all(u8::is_ascii_alphanumeric));
    assert_eq!(run(Some(&t), "s-outro").end, Some(7));
    // Repeated: the level holds.
    assert_eq!(run(Some(&t), "s-outro").failure, unavailable);
    assert_eq!(c.records("ssh").len(), 1);

    // Made up (in the right format).
    let fake = Secret::from_text("AbCdEfGhIjKlMnOpQrStUvWxYz012345".into());
    assert_eq!(run(Some(&fake), "s-outro").failure, unavailable);

    // Of another entry: not valid and dies there.
    let t = c.token("s-livre");
    assert_eq!(run(Some(&t), "s-outro").failure, unavailable);
    let before = c.records("ssh").len();
    let r = run(Some(&t), "s-livre");
    assert_eq!(r.end, Some(7), "s-livre é livre: roda pelo nível");
    assert_eq!(c.records("ssh").len(), before + 1);

    // Expired: 30 s on the engine clock.
    let t = c.token("s-outro");
    c.clock.advance(Duration::from_secs(31));
    assert_eq!(run(Some(&t), "s-outro").failure, unavailable);
    let t = c.token("s-outro");
    c.clock.advance(Duration::from_secs(29));
    assert_eq!(run(Some(&t), "s-outro").end, Some(7));

    // Locking erases the tokens.
    let t = c.token("s-outro");
    c.d.travar();
    assert!(ok(&unlock(&c.d, PASSWORD)));
    assert_eq!(run(Some(&t), "s-outro").failure, unavailable);

    // Blocked is still nonexistent, even with a valid token.
    let t = c.token("s-bloq");
    assert_eq!(
        run(Some(&t), "s-bloq").failure,
        Some(ResponseError::EntryNotFound)
    );

    // Too many tokens: the oldest goes.
    let first = c.token("s-outro");
    for _ in 0..16 {
        let _ = c.token("s-outro");
    }
    assert_eq!(run(Some(&first), "s-outro").failure, unavailable);
}

// --- Phase 28: jump and tunnel through the bridge (no `ProxyJump`, no shell) -----------

/// Waits for the record of the fake program `name` whose argv has `mark` (whole or as
/// the start of an argument).
fn wait_record(c: &Scene, name: &str, mark: &str) -> Rec {
    let mut found = None;
    assert!(
        wait_for(Duration::from_secs(30), || {
            found = c
                .records(name)
                .into_iter()
                .find(|r| argv(r).iter().any(|a| a.starts_with(mark)));
            found.is_some()
        }),
        "sem registro de {name} com {mark}"
    );
    found.unwrap()
}

/// The value after `op` in the argv.
fn after(a: &[String], op: &str) -> String {
    let i = a.iter().position(|x| x == op).unwrap();
    a[i + 1].clone()
}

/// Attack: a process outside the session (this test: same uid, another process group)
/// connects to the bridge. It is closed without a single byte and nothing is launched.
fn stranger_is_refused(port: u16) {
    use std::io::{Read, Write};
    let mut t = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    t.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let _ = t.write_all(b"SSH-2.0-estranho\r\n");
    let mut buf = [0u8; 64];
    assert_eq!(
        t.read(&mut buf).unwrap_or(0),
        0,
        "a ponte respondeu a um estranho"
    );
}

/// The inode of the socket that listens on `127.0.0.1:<port>` (`/proc/net/tcp`, state
/// `0A`). The final check uses the inode, not the port: with the tests in parallel,
/// another bridge may get the same port number from the kernel right afterwards.
fn listener_inode(port: u16) -> u64 {
    let t = fs::read_to_string("/proc/net/tcp").unwrap();
    let local = format!("0100007F:{port:04X}");
    t.lines()
        .skip(1)
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .find(|c| c.len() > 9 && c[1] == local && c[3] == "0A")
        .and_then(|c| c[9].parse().ok())
        .unwrap_or_else(|| panic!("ninguém escuta em 127.0.0.1:{port}"))
}

/// Was the bridge listener (by inode) closed? (The daemon runs in this process.)
fn listener_closed(inode: u64) -> bool {
    let target = format!("socket:[{inode}]");
    wait_for(Duration::from_secs(5), || {
        !fs::read_dir("/proc/self/fd").unwrap().any(|e| {
            fs::read_link(e.unwrap().path()).is_ok_and(|l| l.as_os_str() == target.as_str())
        })
    })
}

fn nobody_with_w(c: &Scene) -> bool {
    !c.records("ssh")
        .iter()
        .any(|r| argv(r).contains(&"-W".to_owned()))
}

/// Checks the bridge `ssh -W` (D13, D43): `-F /dev/null`, the Ward `known_hosts`, the
/// user only after `-l`, the host after `--`, the target in `-W`, no `ProxyJump`,
/// `User=` nor `user@host`.
fn verify_bridge_argv(c: &Scene, a: &[String], target: &str, user: &str) {
    // The record has the argv without the program (the fake `ssh` of `c.base`).
    let _ = c;
    assert_eq!(&a[0..2], ["-F", "/dev/null"], "{a:?}");
    let o = options(a);
    for x in [
        "IdentitiesOnly=yes",
        "CertificateFile=/dev/null",
        "GlobalKnownHostsFile=/dev/null",
        "StrictHostKeyChecking=accept-new",
        "ProxyCommand=none",
        "ControlMaster=no",
        "ForwardAgent=no",
        "PermitLocalCommand=no",
        "ExitOnForwardFailure=yes",
    ] {
        assert!(o.contains(&x.to_owned()), "{x}: {a:?}");
    }
    assert!(o.iter().any(|x| x.starts_with("UserKnownHostsFile=")
        && x.ends_with("/known_hosts")
        && !x.contains("/.ssh/")));
    let end: Vec<String> = ["-W", target, "-p", "2222", "-l", user, "--", "127.0.0.1"]
        .map(String::from)
        .into();
    assert_eq!(&a[a.len() - 8..], end.as_slice(), "{a:?}");
    assert!(
        !a.iter().any(|x| x.contains("ProxyJump")
            || x.starts_with("User=")
            || x.ends_with("@127.0.0.1"))
    );
}

/// Phase 28 / V4.25 / V4.35 (D13, D19, D43; S1, S3, S4): `ward ssh` of a server with a
/// jump, password on both. The bridge only accepts the main `ssh` (the test, from
/// outside, is refused without a byte and without launching anything; the second
/// connection too); the jump `ssh` gets the jump password only in `SSHPASS`; the main
/// one connects to `127.0.0.1:<port>` with the `HostKeyAlias` of the destination; the
/// jump `stderr` arrives masked; at the end the bridge closes.
#[test]
fn phase28_jump_with_password_through_the_bridge() {
    let c = scene("salto");
    c.set_mode("normal");
    fs::write(c.base.join("esperar-ponte"), "").unwrap();
    let port = std::cell::Cell::new(0u16);
    let inode = std::cell::Cell::new(0u64);
    let r = session(&c, ssh_request("s-salto", &["hostname"], None, None), |s| {
        close_stdin(s);
        let main = wait_record(&c, "ssh", "HostKeyAlias=");
        let pt: u16 = after(&argv(&main), "-p").parse().unwrap();
        port.set(pt);
        inode.set(listener_inode(pt));
        stranger_is_refused(pt);
        assert!(
            nobody_with_w(&c),
            "a conexão do estranho lançou o ssh do salto"
        );
        fs::write(c.base.join("continuar-ponte"), "").unwrap();
    });
    assert_eq!(r.end, Some(0), "{r:?}");
    let t = r.text();
    assert!(t.contains("ponte=eco:127.0.0.1:2222:oi"), "{t}");
    assert!(
        t.contains("segunda=fechada"),
        "a ponte do salto aceitou duas: {t}"
    );
    let stderr = String::from_utf8_lossy(&r.stderr).into_owned();
    assert!(stderr.contains("erro-salto ••••"), "{stderr}");
    assert!(!contains(&r.stdout, SSH_PASSWORD.as_bytes()));
    assert!(!contains(&r.stderr, SSH_PASSWORD.as_bytes()));
    let pt = port.get();
    assert!(
        listener_closed(inode.get()),
        "a ponte ficou aberta depois da sessão"
    );

    // The main one: `sshpass -e ssh … -p <bridge> -l deploy -- 127.0.0.1 hostname`.
    let main = wait_record(&c, "ssh", "HostKeyAlias=");
    let a = argv(&main);
    let o = options(&a);
    assert!(
        o.contains(&"HostKeyAlias=[127.0.0.1]:2222".to_owned()),
        "{a:?}"
    );
    assert!(o.contains(&"CheckHostIP=no".to_owned()));
    assert!(o.contains(&"ProxyCommand=none".to_owned()));
    let end: Vec<String> = [
        "-p",
        &pt.to_string(),
        "-l",
        "deploy",
        "--",
        "127.0.0.1",
        "hostname",
    ]
    .map(String::from)
    .into();
    assert_eq!(&a[a.len() - 7..], end.as_slice());
    assert!(
        !a.iter()
            .any(|x| x.contains("ProxyJump") || x.starts_with("User="))
    );

    // The jump one: the user with `@` whole after `-l` (D43), the password only in the
    // environment.
    let jump = wait_record(&c, "ssh", "-W");
    let a = argv(&jump);
    verify_bridge_argv(&c, &a, "127.0.0.1:2222", "deploy@cliente.com.br");
    let o = options(&a);
    assert!(o.contains(&"PubkeyAuthentication=no".to_owned()));
    assert!(o.contains(&"IdentityAgent=none".to_owned()));
    assert!(o.contains(&"NumberOfPasswordPrompts=1".to_owned()));
    let expected: BTreeSet<String> = ["LANG", "PATH", "SSHPASS"].map(String::from).into();
    assert_eq!(env(&jump), expected);
    assert_eq!(jump.one("sshpass_sha256"), sha256(SSH_PASSWORD.as_bytes()));
    // Two `sshpass -e` (the main one and the jump one), none with the password in argv.
    let sp = c.records("sshpass");
    assert_eq!(sp.len(), 2, "{sp:?}");
    for r in &sp {
        assert_eq!(argv(r)[0], "-e");
        assert!(!argv(r).iter().any(|x| x.contains(SSH_PASSWORD)));
    }
    assert!(rt_without_agent(&c));
}

/// Phase 28 / D12: a key on the main host (like the internal SSH host of the test
/// vault) and a key on the jump. Each `ssh` has its own agent, which only serves it (the grandchild is
/// refused); everything goes away at the end.
#[test]
fn phase28_jump_with_key_agent_per_ssh() {
    // Main host with a key, jump with a password.
    let c = scene("saltochave");
    let r = session(
        &c,
        ssh_request("s-salto-chave", &["hostname"], None, None),
        close_stdin,
    );
    assert_eq!(r.end, Some(0), "{r:?}");
    assert!(r.text().contains("ponte=eco:127.0.0.1:2222:oi"), "{r:?}");
    let main = wait_record(&c, "ssh", "HostKeyAlias=");
    assert_eq!(main.one("agente.lista"), "12", "{main:?}");
    assert_eq!(main.one("agente.assina"), "14");
    assert_eq!(main.one("agente.neto"), "recusada");
    let o = options(&argv(&main));
    assert!(o.contains(&"PubkeyAuthentication=yes".to_owned()));
    assert_eq!(c.records("sshpass").len(), 1, "só o salto usa sshpass");
    assert!(rt_without_agent(&c));

    // Main host with a password, jump with a key.
    let c = scene("viachave");
    let r = session(
        &c,
        ssh_request("s-via-chave", &["hostname"], None, None),
        close_stdin,
    );
    assert_eq!(r.end, Some(0), "{r:?}");
    assert!(r.text().contains("ponte=eco:127.0.0.1:2222:oi"), "{r:?}");
    let jump = wait_record(&c, "ssh", "-W");
    assert_eq!(jump.one("agente.lista"), "12", "{jump:?}");
    assert_eq!(jump.one("agente.assina"), "14");
    for op in ["adiciona", "remove", "trava"] {
        assert_eq!(jump.one(&format!("agente.{op}")), "5", "{op}");
    }
    assert_eq!(jump.one("agente.neto"), "recusada");
    verify_bridge_argv(&c, &argv(&jump), "127.0.0.1:2222", "deploy");
    let expected: BTreeSet<String> = ["LANG", "PATH"].map(String::from).into();
    assert_eq!(env(&jump), expected, "sem SSHPASS nem SSH_AUTH_SOCK");
    assert!(rt_without_agent(&c));
}

/// Phase 28 / V4.29 (D10, D19): `ward run` of a database with a tunnel. The command gets
/// `PGHOST=127.0.0.1` and the bridge port and talks to the target through the `ssh -W`
/// of the tunnel server (two connections); a stranger is refused without launching
/// anything; the database password and the tunnel server one come out masked (D19); the
/// bridge closes at the end.
#[test]
fn phase28_database_tunnel_in_run() {
    let c = scene("tunel");
    let mark = c.base.join("continuar-tunel");
    let port_file = c.base.join("porta-tunel");
    let script = format!(
        r#"
import os, socket, time
porta = int(os.environ["PGPORT"])
open({file:?}, "w").write(str(porta))
print("PGHOST=" + os.environ["PGHOST"], flush=True)
print("PGPORT=" + os.environ["PGPORT"], flush=True)
print("url=" + os.environ["DATABASE_URL"], flush=True)
print("senha=" + os.environ["PGPASSWORD"], flush=True)
print("servidor={server}", flush=True)
fim = time.time() + 30
while time.time() < fim and not os.path.exists({mark:?}):
    time.sleep(0.02)
for i in range(2):
    s = socket.create_connection(("127.0.0.1", porta), timeout=60)
    s.sendall(b"select %d\n" % i)
    d = b""
    while not d.endswith(b"\n"):
        x = s.recv(4096)
        if not x:
            break
        d += x
    print("resposta=" + d.decode().strip(), flush=True)
    s.close()
"#,
        file = port_file.to_str().unwrap(),
        mark = mark.to_str().unwrap(),
        server = SSH_PASSWORD,
    );
    let request = Request::Run(ward_core::protocol::RunRequest {
        entry: "b-tunel".into(),
        argv: ["/usr/bin/python3", "-I", "-c", script.as_str()]
            .iter()
            .map(|x| Zeroizing::new(x.as_bytes().to_vec()))
            .collect(),
        cwd: Zeroizing::new(b"/".to_vec()),
        stdin_kind: ward_core::protocol::StdinKind::Null,
        env: vec![ward_core::protocol::Variable {
            name: Zeroizing::new(b"PATH".to_vec()),
            value: Secret::from_text("/usr/bin:/bin".into()),
        }],
    });
    let port = std::cell::Cell::new(0u16);
    let inode = std::cell::Cell::new(0u64);
    let r = session_of(
        &c,
        request,
        |s| {
            close_stdin(s);
            assert!(wait_for(Duration::from_secs(30), || {
                fs::read_to_string(&port_file).is_ok_and(|t| t.parse::<u16>().is_ok())
            }));
            let pt: u16 = fs::read_to_string(&port_file).unwrap().parse().unwrap();
            port.set(pt);
            inode.set(listener_inode(pt));
            stranger_is_refused(pt);
            assert!(
                nobody_with_w(&c),
                "a conexão do estranho lançou o ssh do túnel"
            );
            fs::write(&mark, "").unwrap();
        },
        true,
    );
    assert_eq!(r.end, Some(0), "{r:?}");
    let t = r.text();
    let pt = port.get();
    assert!(t.contains("PGHOST=127.0.0.1"), "{t}");
    assert!(t.contains(&format!("PGPORT={pt}\n")), "{t}");
    // The whole `DATABASE_URL` is a secret (D11): it comes out masked.
    assert!(t.contains("url=••••"), "{t}");
    assert!(t.contains("resposta=eco:db.interno:5432:select 0"), "{t}");
    assert!(t.contains("resposta=eco:db.interno:5432:select 1"), "{t}");
    assert!(t.contains("senha=••••"), "{t}");
    assert!(
        t.contains("servidor=••••"),
        "D19: a senha do túnel na máscara: {t}"
    );
    for secret in [SSH_PASSWORD, DB_PASSWORD] {
        assert!(!contains(&r.stdout, secret.as_bytes()));
        assert!(!contains(&r.stderr, secret.as_bytes()));
    }
    let stderr = String::from_utf8_lossy(&r.stderr).into_owned();
    assert!(stderr.contains("erro-salto ••••"), "{stderr}");
    assert!(
        listener_closed(inode.get()),
        "o túnel ficou aberto depois do comando"
    );
    let tunnels: Vec<Rec> = c
        .records("ssh")
        .into_iter()
        .filter(|r| argv(r).contains(&"-W".to_owned()))
        .collect();
    assert_eq!(tunnels.len(), 2, "um ssh -W por conexão");
    for r in &tunnels {
        verify_bridge_argv(&c, &argv(r), "db.interno:5432", "deploy@cliente.com.br");
        assert_eq!(r.one("sshpass_sha256"), sha256(SSH_PASSWORD.as_bytes()));
    }
    assert!(rt_without_agent(&c));
}

/// Phase 28 (D12 + D19): locking the vault during a `ward run` with a tunnel makes the
/// bridge stop accepting new connections; since Phase 54 (S8), the command itself dies
/// (137).
#[test]
fn phase28_lock_closes_the_tunnel_to_new_connections() {
    let c = scene("tuneltrava");
    let mark = c.base.join("continuar-tunel");
    let port_file = c.base.join("porta-tunel");
    let script = format!(
        r#"
import os, socket, time
porta = int(os.environ["PGPORT"])
open({file:?}, "w").write(str(porta))
fim = time.time() + 30
while time.time() < fim and not os.path.exists({mark:?}):
    time.sleep(0.02)
try:
    s = socket.create_connection(("127.0.0.1", porta), timeout=5)
    s.sendall(b"x\n")
    print("depois=" + (s.recv(64).decode() or "fechada"), flush=True)
except OSError as e:
    print("depois=" + type(e).__name__, flush=True)
"#,
        file = port_file.to_str().unwrap(),
        mark = mark.to_str().unwrap(),
    );
    let request = Request::Run(ward_core::protocol::RunRequest {
        entry: "b-tunel".into(),
        argv: ["/usr/bin/python3", "-I", "-c", script.as_str()]
            .iter()
            .map(|x| Zeroizing::new(x.as_bytes().to_vec()))
            .collect(),
        cwd: Zeroizing::new(b"/".to_vec()),
        env: Vec::new(),
        stdin_kind: ward_core::protocol::StdinKind::Null,
    });
    let r = session_of(
        &c,
        request,
        |s| {
            close_stdin(s);
            assert!(wait_for(Duration::from_secs(30), || {
                fs::read_to_string(&port_file).is_ok_and(|t| t.parse::<u16>().is_ok())
            }));
            c.d.travar();
            std::thread::sleep(Duration::from_millis(400));
            fs::write(&mark, "").unwrap();
        },
        true,
    );
    // Phase 54 (S8): the lock kills the whole `ward run` (137), besides closing the
    // bridge.
    assert_eq!(r.end, Some(137), "{r:?}");
    let t = r.text();
    assert!(
        !t.contains("depois=eco"),
        "a ponte aceitou depois de travar: {t}"
    );
    assert!(nobody_with_w(&c));
}
