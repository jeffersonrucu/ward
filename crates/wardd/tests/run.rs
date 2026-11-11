//! `ward run` through the `wardd` binary (test mode), end to end over the socket (Phase
//! 26): D10 delivery, mask (S4), exit code, environment only from the CLI without `LD_*`
//! (D13, D60), attachments 0600 in `<rt>tmp/` deleted on every exit and at startup (S2,
//! V4.15), a CLI that drops kills the child's group (D13) and the RA1 record.
//!
//! The vault is a test one, created in this directory by the core itself, only with
//! fake values; the socket (and the `<rt>`) lives in a directory of this test only in
//! `/run/user/<uid>/`.

mod common;

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::*;
use ward_core::protocolo::{self, ErroResposta, Fluxo, Papel, Resposta};

const WARDD: &str = env!("CARGO_BIN_EXE_wardd");
/// Password of the free test entry (fake).
const FREE_PASSWORD: &str = SECRETS[3];
const LICENSE: &str = "falso-licenca-run-26";
const ATTACHMENT: &str = "FALSO_ENV=falso-anexo-run-26";

struct SocketDir(PathBuf);

impl SocketDir {
    fn new(label: &str) -> SocketDir {
        let p = ward_platform::socket::runtime_dir()
            .join(format!("wardd-r-{}-{label}", std::process::id()));
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

fn start(dev: &Dir, sock: &SocketDir) -> Wardd {
    start_with(dev, sock, &[])
}

/// The test `wardd` with extra variables (Phase 54b: `WARD_DEV_APP`).
fn start_with(dev: &Dir, sock: &SocketDir, extra: &[(&str, &std::ffi::OsStr)]) -> Wardd {
    let log = dev.0.join("wardd.log");
    let mut cmd = Command::new(WARDD);
    cmd.env_clear();
    for (k, v) in extra {
        cmd.env(k, v);
    }
    let child = cmd
        .env("PATH", "/usr/bin:/bin")
        .env("WARD_DEV_DIR", &dev.0)
        .env("WARD_DEV_SOCKET_DIR", &sock.0)
        .env("RUST_LOG", "trace")
        .env("VARIAVEL_DO_DAEMON", "nao-pode-vazar")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log)
                .unwrap(),
        )
        .spawn()
        .unwrap();
    let socket = sock.0.join(ward_platform::socket::SOCKET_NAME);
    let end = Instant::now() + Duration::from_secs(20);
    while UnixStream::connect(&socket).is_err() {
        assert!(Instant::now() < end, "o wardd não criou o socket");
        std::thread::sleep(Duration::from_millis(20));
    }
    Wardd { child, log, socket }
}

fn send(s: &mut UnixStream, json: &str) {
    s.write_all(&(json.len() as u32).to_be_bytes()).unwrap();
    s.write_all(json.as_bytes()).unwrap();
}

fn receive(s: &mut UnixStream) -> Option<Resposta> {
    let mut n = [0u8; 4];
    s.read_exact(&mut n).ok()?;
    let mut b = vec![0u8; u32::from_be_bytes(n) as usize];
    s.read_exact(&mut b).ok()?;
    Some(protocolo::decodificar_resposta(Papel::Agente, &b).unwrap())
}

fn connect(w: &Wardd) -> UnixStream {
    let s = UnixStream::connect(&w.socket).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(60))).unwrap();
    s
}

fn unlock_through(w: &Wardd) {
    let mut s = connect(w);
    send(
        &mut s,
        &format!(
            r#"{{"v":1,"op":"destravar","dados":{{"senha":"{}"}}}}"#,
            b64(PASSWORD.as_bytes())
        ),
    );
    assert!(matches!(receive(&mut s), Some(Resposta::Ok)));
}

fn run_request(entry: &str, argv: &[&str], env_vars: &[(&str, &str)]) -> String {
    run_request_with(entry, argv, env_vars, "pipe")
}

/// The run request with the stdin kind (Phase 54b).
fn run_request_with(entry: &str, argv: &[&str], env_vars: &[(&str, &str)], kind: &str) -> String {
    let argv: Vec<String> = argv
        .iter()
        .map(|a| format!("\"{}\"", b64(a.as_bytes())))
        .collect();
    let env_vars: Vec<String> = env_vars
        .iter()
        .map(|(k, v)| {
            format!(
                r#"{{"nome":"{}","valor":"{}"}}"#,
                b64(k.as_bytes()),
                b64(v.as_bytes())
            )
        })
        .collect();
    format!(
        r#"{{"v":1,"op":"executar","dados":{{"entrada":"{entry}","argv":[{}],"cwd":"{}","tipo_entrada":"{kind}","ambiente":[{}]}}}}"#,
        argv.join(","),
        b64(b"/"),
        env_vars.join(",")
    )
}

fn send_stdin(s: &mut UnixStream, data: &[u8]) {
    send(
        s,
        &format!(
            r#"{{"v":1,"op":"entrada_padrao","dados":{{"dados":"{}"}}}}"#,
            b64(data)
        ),
    );
}

/// What the session returned: stdout, stderr and the code (or the error).
#[derive(Debug, Default)]
struct Session {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    end: Option<u8>,
    failure: Option<ErroResposta>,
}

fn read_session(s: &mut UnixStream) -> Session {
    let mut r = Session::default();
    loop {
        match receive(s) {
            Some(Resposta::Saida { fluxo, dados }) => match fluxo {
                Fluxo::Saida => r.stdout.extend_from_slice(&dados),
                Fluxo::Erro => r.stderr.extend_from_slice(&dados),
            },
            Some(Resposta::Fim { codigo }) => {
                r.end = Some(codigo);
                break;
            }
            Some(Resposta::Erro(e)) => {
                r.failure = Some(e);
                break;
            }
            Some(other) => panic!("resposta inesperada: {other:?}"),
            None => break,
        }
    }
    // After the end, the daemon closes the connection; refused before launching, it
    // goes on.
    if r.end.is_some() {
        assert!(receive(s).is_none());
    }
    r
}

fn run(w: &Wardd, entry: &str, argv: &[&str], env_vars: &[(&str, &str)]) -> Session {
    let mut s = connect(w);
    send(&mut s, &run_request(entry, argv, env_vars));
    send_stdin(&mut s, b"");
    read_session(&mut s)
}

fn sh(script: &str) -> Vec<&str> {
    vec!["/bin/sh", "-c", script]
}

fn sha256(data: &[u8]) -> String {
    let mut c = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    c.stdin.take().unwrap().write_all(data).unwrap();
    let out = c.wait_with_output().unwrap();
    String::from_utf8(out.stdout).unwrap()
}

/// Test vault with, besides the shared test entries, a secure note and a generic
/// entry (with a secret extra and an attachment), both free. Locked at the end (the
/// binary opens it).
fn vault(dev: &Dir) {
    let (d, _) = test_vault(dev);
    for q in [
        format!(
            r#"{{"entrada":{{"nome":"t-nota","grupo":"clientex","ambiente":"dev","tipo":"nota-segura","campos":{{"texto":"{}"}}}}}}"#,
            b64(b"falso-nota-run-26")
        ),
        format!(
            r#"{{"entrada":{{"nome":"t-gen","grupo":"clientex","ambiente":"stage","tipo":"generico","extras":[{{"nome":"REGIAO","valor":"{}","segredo":false}},{{"nome":"CHAVE_LICENCA","valor":"{}","segredo":true}}],"anexos":[{{"nome":"app.env","conteudo":"{}"}}]}}}}"#,
            b64(b"r-01"),
            b64(LICENSE.as_bytes()),
            b64(ATTACHMENT.as_bytes())
        ),
    ] {
        let r = request(&d, Papel::App, "salvar_entrada", &q);
        assert!(ok(&r), "{r:?}");
    }
    for n in ["t-nota", "t-gen"] {
        assert!(ok(&set_level(&d, Papel::App, n, "livre", PIN)));
    }
    d.travar();
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn tmp_empty(sock: &SocketDir) -> bool {
    fs::read_dir(sock.0.join("tmp")).map_or(true, |mut d| d.next().is_none())
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

fn alive(pid: u32) -> bool {
    fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|s| !s.contains(") Z "))
}

/// V4.3–V4.11, V4.17, V4.19, V4.20 with the entries of this test, plus D13, D60 and the
/// locale (handoff 22a).
#[test]
fn v4_delivery_mask_code_and_refusals() {
    let dev = Dir::new("run");
    vault(&dev);
    let sock = SocketDir::new("run");
    let w = start(&dev, &sock);
    unlock_through(&w);

    // Delivery: the hash of the password matches (V4.3); the printed password comes out
    // `••••` (S4).
    let r = run(
        &w,
        "t-livre",
        &sh("printf %s \"$WARD_PASS\" | sha256sum"),
        &[],
    );
    assert_eq!(r.end, Some(0), "{r:?}");
    assert_eq!(text(&r.stdout), sha256(FREE_PASSWORD.as_bytes()));
    let r = run(
        &w,
        "t-livre",
        &sh(
            "printf '%s|%s|%s|%s' \"$WARD_ENTRADA\" \"$WARD_GRUPO\" \"$WARD_AMBIENTE\" \"$WARD_USER\"",
        ),
        &[],
    );
    assert_eq!(text(&r.stdout), "t-livre|clientex|dev|admin");

    // V4.17: plain, base64, percent, in two chunks with a pause and on stderr. In two
    // chunks (D34 revised), the start comes out as it came out and the chunk that
    // completes the password becomes a mark: the whole password never comes out.
    for script in [
        "printf %s \"$WARD_PASS\"",
        "printf %s \"$WARD_PASS\" | base64",
        "printf 'falso-li'; sleep 0.2; printf 'vre-ward-09'",
        "printf %s \"$WARD_PASS\" >&2",
        "printf 'falso-li' >&2; sleep 0.2; printf 'vre-ward-09' >&2",
    ] {
        let r = run(&w, "t-livre", &sh(script), &[]);
        assert_eq!(r.end, Some(0), "{script}: {r:?}");
        let all = [r.stdout.as_slice(), r.stderr.as_slice()].concat();
        assert!(!contains(&all, b"falso-livre"), "{script}: {}", text(&all));
        assert!(!contains(&all, b"ZmFsc28tbGl2cmU"), "{script}");
        assert!(
            contains(&all, "••••".as_bytes()),
            "{script}: {}",
            text(&all)
        );
    }

    // V4.19: the command code; killed by SIGTERM, 143.
    assert_eq!(run(&w, "t-livre", &sh("exit 7"), &[]).end, Some(7));
    assert_eq!(run(&w, "t-livre", &sh("kill -TERM $$"), &[]).end, Some(143));
    // A command that does not exist: like the shell (127), with a fixed message.
    let r = run(&w, "t-livre", &["comando-que-nao-existe-26"], &[]);
    assert_eq!(r.end, Some(127));
    assert!(text(&r.stderr).contains("comando não encontrado"), "{r:?}");

    // D13, D60 and Phase 26a: the environment is built from scratch (only the CLI list,
    // without `LD_*`, and nothing from the daemon); locale C becomes LC_CTYPE=C.UTF-8;
    // the delivery goes over a same name that came from the CLI.
    let r = run(
        &w,
        "t-livre",
        &["/usr/bin/env"],
        &[
            ("LD_PRELOAD", "/nao/existe.so"),
            ("LD_LIBRARY_PATH", "/tmp"),
            ("LANG", "C"),
            ("DA_CLI", "nao-pode-passar"),
            ("WARD_PASS", "valor-da-cli"),
        ],
    );
    let env = text(&r.stdout);
    assert_eq!(r.end, Some(0), "{r:?}");
    for forbidden in [
        "LD_",
        "VARIAVEL_DO_DAEMON",
        "WARD_DEV_",
        "RUST_LOG",
        "LISTEN_",
        "valor-da-cli",
        "DA_CLI",
    ] {
        assert!(
            !env.contains(forbidden),
            "{forbidden} no ambiente do filho:\n{env}"
        );
    }
    for expected in [
        "LANG=C\n",
        "LC_CTYPE=C.UTF-8",
        "WARD_PASS=••••",
        "WARD_ENTRADA=t-livre",
        "PATH=/usr/local/bin:/usr/bin:/bin\n",
    ] {
        assert!(env.contains(expected), "{expected} faltando:\n{env}");
    }
    // UTF-8 locale: bash's `${X@Q}` with an accent uses quotes (the mask covers it); it
    // stays `$'…'` (RA3) only in locale C.
    let r = run(
        &w,
        "t-livre",
        &[
            "/bin/bash",
            "-c",
            "X=\"$WARD_PASS-ç\"; echo \"${X@Q}\"; locale charmap",
        ],
        &[("LANG", "C")],
    );
    assert!(text(&r.stdout).contains("UTF-8"), "{r:?}");
    assert!(!contains(&r.stdout, b"falso-livre"), "{r:?}");

    // Stdin (D13).
    let mut s = connect(&w);
    send(&mut s, &run_request("t-livre", &["/bin/cat"], &[]));
    send_stdin(&mut s, b"linha 1\n");
    send_stdin(&mut s, b"linha 2\n");
    send_stdin(&mut s, b"");
    let r = read_session(&mut s);
    assert_eq!(
        (r.end, text(&r.stdout).as_str()),
        (Some(0), "linha 1\nlinha 2\n")
    );

    // Attachment and secret extra (V4.10, V4.5): a 0600 file in <rt>tmp/, the content
    // and the license masked, and the file goes away at the end (V4.15).
    let r = run(
        &w,
        "t-gen",
        &sh(
            "f=\"$WARD_ARQUIVO_APP_ENV\"; stat -c '%a %U' \"$f\"; stat -c '%a' \"$(dirname \"$f\")\"; \
             printf '%s\\n' \"$f\"; cat \"$f\"; echo; printf '%s|%s\\n' \"$WARD_REGIAO\" \"$WARD_CHAVE_LICENCA\"; \
             sha256sum < \"$f\"",
        ),
        &[],
    );
    assert_eq!(r.end, Some(0), "{r:?}");
    let out = text(&r.stdout);
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines[0].starts_with("600 "), "{out}");
    assert_eq!(lines[1], "700", "{out}");
    let path = PathBuf::from(lines[2]);
    assert!(path.starts_with(sock.0.join("tmp")), "{out}");
    assert!(path.ends_with("app.env"), "{out}");
    assert!(
        !out.contains("falso-anexo") && !out.contains(LICENSE),
        "{out}"
    );
    assert!(out.contains("r-01|••••"), "{out}");
    assert!(
        out.contains(sha256(ATTACHMENT.as_bytes()).split(' ').next().unwrap()),
        "{out}"
    );
    assert!(!path.exists(), "o anexo ficou depois do fim");
    assert!(tmp_empty(&sock));

    // V4.11: a secure note runs nothing.
    let mark = dev.0.join("marca-nota");
    let r = run(
        &w,
        "t-nota",
        &["/usr/bin/touch", mark.to_str().unwrap()],
        &[],
    );
    assert_eq!(r.failure, Some(ErroResposta::TipoSemEntrega), "{r:?}");
    assert_eq!(ErroResposta::TipoSemEntrega.mensagem(), "tipo sem entrega");
    assert!(!mark.exists());
    // V4.20: needs approval and no app → approval unavailable, nothing run.
    let r = run(
        &w,
        "t-aprov",
        &["/usr/bin/touch", mark.to_str().unwrap()],
        &[],
    );
    assert_eq!(
        r.failure,
        Some(ErroResposta::AprovacaoIndisponivel),
        "{r:?}"
    );
    assert!(!mark.exists());
    // Blocked = nonexistent (V3.17); the database tunnel is Phase 28 (nothing run).
    let r = run(
        &w,
        "t-bloq",
        &["/usr/bin/touch", mark.to_str().unwrap()],
        &[],
    );
    assert_eq!(r.failure, Some(ErroResposta::EntradaInexistente), "{r:?}");
    assert!(!mark.exists());
    // Refused before launching, the connection goes on to the next request.
    let mut s = connect(&w);
    send(&mut s, &run_request("t-nota", &["/bin/true"], &[]));
    assert!(matches!(
        receive(&mut s),
        Some(Resposta::Erro(ErroResposta::TipoSemEntrega))
    ));
    send(&mut s, &run_request("t-livre", &sh("exit 4"), &[]));
    send_stdin(&mut s, b"");
    assert_eq!(read_session(&mut s).end, Some(4));

    // A frame outside the session (another operation in the middle) ends the command.
    let mut s = connect(&w);
    send(&mut s, &run_request("t-livre", &sh("sleep 30"), &[]));
    std::thread::sleep(Duration::from_millis(300));
    send(&mut s, r#"{"v":1,"op":"lista","dados":{}}"#);
    let t = Instant::now();
    let r = read_session(&mut s);
    assert_eq!(r.end, Some(137), "{r:?}");
    assert!(t.elapsed() < Duration::from_secs(5));

    // S13: no secret in the log (`RUST_LOG=trace`).
    let log = fs::read(&w.log).unwrap();
    for secret in [
        PASSWORD,
        FREE_PASSWORD,
        LICENSE,
        ATTACHMENT,
        "falso-nota-run-26",
    ] {
        assert!(!contains(&log, secret.as_bytes()), "{secret} no log");
        assert!(
            !contains(&log, b64(secret.as_bytes()).as_bytes()),
            "{secret} (b64) no log"
        );
    }
}

/// Phase 26a (medium finding of the M3 audit): the CLI environment does not change the
/// approved command. `PATH` with the attacker's folder in front, `BASH_ENV`/`ENV`,
/// `NODE_OPTIONS=--require`, `GIT_SSH_COMMAND`, `PYTHONSTARTUP`, `PGSERVICEFILE`,
/// `HOME` and locale/timezone/terminal pointing to an attacker's file: none of that
/// reaches the child, and a command without `/` is found only through the fixed `PATH`.
/// The working directory is still the CLI's.
#[test]
fn f26a_attack_cli_environment_does_not_change_the_command() {
    let dev = Dir::new("f26a");
    vault(&dev);
    let sock = SocketDir::new("f26a");
    let w = start(&dev, &sock);
    unlock_through(&w);

    // The attacker's folder: fake `true`, `sh`, `env` and `wardfalso-26a` that leave a
    // mark; an rc for `BASH_ENV`/`ENV`; a module for `--require`.
    let attacker = dev.0.join("atacante");
    fs::create_dir_all(&attacker).unwrap();
    let mark = dev.0.join("marca-26a");
    let fake = format!("#!/bin/sh\necho pego >> '{}'\n", mark.display());
    for name in ["true", "sh", "env", "id", "wardfalso-26a"] {
        let p = attacker.join(name);
        fs::write(&p, &fake).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let rc = attacker.join("rc.sh");
    fs::write(&rc, format!("echo pego >> '{}'\n", mark.display())).unwrap();
    let a = |n: &str| attacker.join(n).display().to_string();
    let attacker_path = format!("{}:/usr/bin:/bin", attacker.display());
    let rc_s = rc.display().to_string();
    let require = format!("--require {}", a("mod.js"));
    let ssh_cmd = format!("{} -x", a("true"));
    let home = attacker.display().to_string();
    let env_vars: Vec<(&str, &str)> = vec![
        ("PATH", attacker_path.as_str()),
        ("BASH_ENV", rc_s.as_str()),
        ("ENV", rc_s.as_str()),
        ("NODE_OPTIONS", require.as_str()),
        ("GIT_SSH_COMMAND", ssh_cmd.as_str()),
        ("PYTHONSTARTUP", rc_s.as_str()),
        ("PYTHONPATH", home.as_str()),
        ("PERL5LIB", home.as_str()),
        ("PGSERVICEFILE", rc_s.as_str()),
        ("SSH_AUTH_SOCK", rc_s.as_str()),
        ("HOME", home.as_str()),
        ("USER", "atacante"),
        ("LOCPATH", home.as_str()),
        ("LANG", "/tmp/atacante-locale"),
        ("TZ", "../../../tmp/atacante-fuso"),
        ("TERM", "../atacante"),
        ("TERMINFO", home.as_str()),
        ("COLORTERM", "truecolor"),
        ("NO_COLOR", "1"),
    ];

    // A command without `/` comes from the fixed `PATH`: the system `true`, `sh` and
    // `env`; what only exists in the attacker's folder is not found (127).
    let r = run(&w, "t-livre", &["true"], &env_vars);
    assert_eq!(r.end, Some(0), "{r:?}");
    let r = run(&w, "t-livre", &["wardfalso-26a"], &env_vars);
    assert_eq!(r.end, Some(127), "{r:?}");
    // `bash -c` (not interactive) would read `BASH_ENV`; `sh -c` with `ENV`.
    let r = run(&w, "t-livre", &["bash", "-c", "exit 0"], &env_vars);
    assert_eq!(r.end, Some(0), "{r:?}");
    let r = run(&w, "t-livre", &["sh", "-c", "exit 0"], &env_vars);
    assert_eq!(r.end, Some(0), "{r:?}");
    let r = run(&w, "t-livre", &["sh", "-i", "-c", "exit 0"], &env_vars);
    assert_eq!(r.end, Some(0), "{r:?}");
    assert!(!mark.exists(), "o comando do atacante rodou");

    // The child's environment: only the fixed part, the list and the delivery.
    let r = run(&w, "t-livre", &["env"], &env_vars);
    assert_eq!(r.end, Some(0), "{r:?}");
    let env = text(&r.stdout);
    let (user, home_dir) = ward_platform::user_account().unwrap();
    for expected in [
        "PATH=/usr/local/bin:/usr/bin:/bin\n".to_owned(),
        format!("HOME={}\n", home_dir.to_string_lossy()),
        format!("USER={}\n", user.to_string_lossy()),
        format!("LOGNAME={}\n", user.to_string_lossy()),
        "COLORTERM=truecolor\n".to_owned(),
        "NO_COLOR=1\n".to_owned(),
        "LC_CTYPE=C.UTF-8\n".to_owned(),
        "WARD_ENTRADA=t-livre\n".to_owned(),
    ] {
        assert!(env.contains(&expected), "{expected:?} faltando:\n{env}");
    }
    for forbidden in [
        "atacante",
        "BASH_ENV",
        "ENV=",
        "NODE_OPTIONS",
        "GIT_",
        "PYTHON",
        "PERL5LIB",
        "PGSERVICEFILE",
        "SSH_",
        "LOCPATH",
        "LANG=",
        "TZ=",
        "TERM=",
        "TERMINFO",
    ] {
        assert!(
            !env.lines()
                .any(|l| l.starts_with(forbidden) || l.contains("atacante")),
            "{forbidden} no ambiente do filho:\n{env}"
        );
    }
    let lines: Vec<&str> = env.lines().collect();
    let names: Vec<&str> = lines
        .iter()
        .map(|l| l.split_once('=').map_or(*l, |(n, _)| n))
        .collect();
    for n in &names {
        assert!(
            [
                "PATH",
                "HOME",
                "USER",
                "LOGNAME",
                "COLORTERM",
                "NO_COLOR",
                "LC_CTYPE"
            ]
            .contains(n)
                || n.starts_with("WARD_"),
            "{n} não devia estar no ambiente:\n{env}"
        );
    }

    // Accepted values pass as they came; the working directory is the CLI's.
    let r = run(
        &w,
        "t-livre",
        &[
            "sh",
            "-c",
            "printf '%s|%s|%s|' \"$TZ\" \"$TERM\" \"$LANG\"; pwd",
        ],
        &[
            ("TZ", "America/Sao_Paulo"),
            ("TERM", "xterm-256color"),
            ("LANG", "pt_BR.UTF-8"),
        ],
    );
    assert_eq!(
        text(&r.stdout),
        "America/Sao_Paulo|xterm-256color|pt_BR.UTF-8|/\n",
        "{r:?}"
    );
    assert!(!mark.exists(), "o comando do atacante rodou");
    // The daemon log counts the removed variables, without name or value.
    let log = fs::read_to_string(&w.log).unwrap();
    assert!(!log.contains("atacante"), "{log}");
}

/// D13 and V4.15: a CLI that drops kills the child's whole group (and the
/// grandchildren) and the attachment goes away within 1 s; a child killed by SIGKILL
/// also deletes the attachment; a `wardd` killed by SIGKILL takes the child along
/// (`PR_SET_PDEATHSIG`) and, when it starts again, empties `<rt>tmp/`.
#[test]
fn v4_15_cli_that_drops_and_killed_daemon() {
    let dev = Dir::new("cai");
    vault(&dev);
    let sock = SocketDir::new("cai");
    let mut w = start(&dev, &sock);
    unlock_through(&w);
    let script = "echo \"$$ $WARD_ARQUIVO_APP_ENV\"; sleep 60 & echo $!; wait";

    // 1. The CLI drops (closes the connection) with the command and a grandchild
    //    running.
    let mut s = connect(&w);
    send(&mut s, &run_request("t-gen", &sh(script), &[]));
    let mut read = Vec::new();
    while read.iter().filter(|b| **b == b'\n').count() < 2 {
        match receive(&mut s) {
            Some(Resposta::Saida { dados, .. }) => read.extend_from_slice(&dados),
            r => panic!("{r:?}"),
        }
    }
    let t = text(&read);
    let mut it = t.split_whitespace();
    let child: u32 = it.next().unwrap().parse().unwrap();
    let file = PathBuf::from(it.next().unwrap());
    let grandchild: u32 = it.next().unwrap().parse().unwrap();
    assert!(alive(child) && alive(grandchild) && file.exists());
    drop(s);
    assert!(
        wait_for(Duration::from_secs(1), || !alive(child)
            && !alive(grandchild)),
        "o grupo segue vivo"
    );
    assert!(
        wait_for(Duration::from_secs(1), || !file.exists()),
        "o anexo ficou"
    );

    // 2. The child gets SIGKILL: 137, and the attachment goes away.
    let r = run(
        &w,
        "t-gen",
        &sh("test -f \"$WARD_ARQUIVO_APP_ENV\" && kill -9 $$"),
        &[],
    );
    assert_eq!(r.end, Some(137), "{r:?}");
    assert!(tmp_empty(&sock));

    // 3. The `wardd` gets SIGKILL in the middle: the child dies along; the attachment
    //    stays until the next startup, which deletes it.
    let mut s = connect(&w);
    send(&mut s, &run_request("t-gen", &sh(script), &[]));
    let mut read = Vec::new();
    while !read.contains(&b'\n') {
        match receive(&mut s) {
            Some(Resposta::Saida { dados, .. }) => read.extend_from_slice(&dados),
            r => panic!("{r:?}"),
        }
    }
    let child: u32 = text(&read)
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    w.child.kill().unwrap();
    let _ = w.child.wait();
    assert!(
        wait_for(Duration::from_secs(2), || !alive(child)),
        "o filho sobreviveu ao daemon"
    );
    assert!(
        !tmp_empty(&sock),
        "sem o daemon, o temporário ainda está lá"
    );
    drop(s);
    drop(w);
    let w = start(&dev, &sock);
    assert!(tmp_empty(&sock), "a partida não limpou <rt>tmp/");
    assert_eq!(
        fs::metadata(sock.0.join("tmp"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    drop(w);
}

/// RA1 (record, no generic mitigation): while the child runs, another process of the
/// same uid reads its `/proc/<pid>/environ` and the 0600 attachment. `execve` sets
/// `dumpable` back to 1 for any ordinary program; neither a user namespace nor Yama
/// (only the `ATTACH` mode) change the reading of `environ` (`PTRACE_MODE_READ`). What
/// holds: the `wardd` (not dumpable) is not readable, and the window is only the
/// command's. This test pins the state recorded in the handoff (V4.13 fails and goes to
/// the user): if one day the child's `environ` stops being readable, it fails and the
/// record changes.
#[test]
fn ra1_record_child_environ_readable_by_same_uid() {
    let dev = Dir::new("ra1");
    vault(&dev);
    let sock = SocketDir::new("ra1");
    let w = start(&dev, &sock);
    unlock_through(&w);
    let mut s = connect(&w);
    send(&mut s, &run_request("t-gen", &sh("echo $$; sleep 30"), &[]));
    let pid: u32 = match receive(&mut s) {
        Some(Resposta::Saida { dados, .. }) => text(&dados).trim().parse().unwrap(),
        r => panic!("{r:?}"),
    };
    let environ = fs::read(format!("/proc/{pid}/environ")).unwrap_or_default();
    assert!(
        contains(&environ, LICENSE.as_bytes()),
        "RA1 mudou: o environ do filho deixou de ser legível pelo mesmo uid"
    );
    // The daemon, not dumpable (S8), is not readable.
    let daemon = w.child.id();
    assert!(fs::read(format!("/proc/{daemon}/environ")).is_err());
    assert!(fs::File::open(format!("/proc/{daemon}/mem")).is_err());
    drop(s);
    assert!(wait_for(Duration::from_secs(1), || !alive(pid)));
    drop(w);
}

/// The request size (D52): above the frame, the daemon refuses with the invalid frame
/// error and runs nothing (the CLI measures first; this is the second layer).
#[test]
fn d52_oversized_request_refused_by_daemon() {
    let dev = Dir::new("d52");
    vault(&dev);
    let sock = SocketDir::new("d52");
    let w = start(&dev, &sock);
    unlock_through(&w);
    let mark = dev.0.join("marca-d52");
    let large = "x".repeat(100 * 1024);
    let env_vars: Vec<(String, String)> =
        (0..9).map(|i| (format!("V{i}"), large.clone())).collect();
    let env_vars: Vec<(&str, &str)> = env_vars
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let mut s = connect(&w);
    let json = run_request(
        "t-livre",
        &["/usr/bin/touch", mark.to_str().unwrap()],
        &env_vars,
    );
    // The daemon may close before reading the whole body (broken pipe here).
    let _ = s
        .write_all(&(json.len() as u32).to_be_bytes())
        .and_then(|()| s.write_all(json.as_bytes()));
    let r = read_session(&mut s);
    assert!(r.end.is_none(), "{r:?}");
    assert!(
        matches!(r.failure, None | Some(ErroResposta::QuadroInvalido)),
        "{r:?}"
    );
    std::thread::sleep(Duration::from_millis(200));
    assert!(!mark.exists());
    assert!(tmp_empty(&sock));
}

/// V4.21 (D16): with the vault locked and no app, `ward run` waits; an unlock through
/// another connection (the CLI unlock) releases the request, which runs with the
/// delivery.
#[test]
fn v4_21_locked_waits_for_cli_unlock() {
    let dev = Dir::new("v421");
    vault(&dev);
    let sock = SocketDir::new("v421");
    let w = start(&dev, &sock);
    let mut s = connect(&w);
    send(
        &mut s,
        &run_request("t-livre", &sh("printf %s \"$WARD_PASS\" | wc -c"), &[]),
    );
    send_stdin(&mut s, b"");
    std::thread::sleep(Duration::from_millis(1500));
    unlock_through(&w);
    let r = read_session(&mut s);
    assert_eq!(r.end, Some(0), "{r:?}");
    assert_eq!(text(&r.stdout).trim(), FREE_PASSWORD.len().to_string());
}

/// Phase 28g, attack of the 2nd M4 audit: a single `write` of `n` bytes + the password,
/// with `n` around the 16 KiB (old buffer) and 64 KiB (default pipe) boundaries, came
/// out with the start of the password in clear (the reader cut at the buffer size). Now
/// the pipe has 256 KiB and the daemon reads the whole batch before masking: the
/// password comes out only `••••`, on stdout and on stderr.
#[test]
fn f28g_attack_single_large_write_does_not_show_password_start() {
    let dev = Dir::new("run-28g");
    vault(&dev);
    let sock = SocketDir::new("28g");
    let w = start(&dev, &sock);
    unlock_through(&w);
    let mut cases: Vec<usize> = (16_370..16_390).collect();
    cases.extend(65_520..65_540);
    cases.extend([131_067, 200_000]);
    for fd in [1, 2] {
        for &n in &cases {
            let script = format!(
                "import os\nos.write({fd}, b'b' * {n} + os.environ['WARD_PASS'].encode() + b'\\n')\n"
            );
            let r = run(&w, "t-livre", &["/usr/bin/python3", "-c", &script], &[]);
            assert_eq!(r.end, Some(0), "fd={fd} n={n}: {:?}", r.failure);
            let all = if fd == 1 { &r.stdout } else { &r.stderr };
            assert!(
                !contains(all, b"falso-"),
                "fd={fd} n={n}: o começo da senha saiu em claro"
            );
            let mut expected = vec![b'b'; n];
            expected.extend_from_slice("••••\n".as_bytes());
            assert!(*all == expected, "fd={fd} n={n}: saída inesperada");
        }
    }
    let log = fs::read(&w.log).unwrap();
    assert!(!contains(&log, FREE_PASSWORD.as_bytes()));
}

/// Phase 54 (S8), attack of the final audit: the `ward run` command got the password in
/// the environment and waits for the lock to use it (`sleep; printenv … > file`, outside
/// the mask). Locking in the middle kills the whole group (the `sh` and the
/// grandchild): the file is never born, the session ends with 137 and the notice, the
/// attachment goes away. A `ward run` after the lock waits for the unlock, like V4.21.
#[test]
fn f54_attack_ward_run_dies_on_lock() {
    let dev = Dir::new("run-54");
    vault(&dev);
    let sock = SocketDir::new("54");
    let w = start(&dev, &sock);
    unlock_through(&w);
    let mark = dev.0.join("marca-f54");
    let grandchild = dev.0.join("marca-f54-neto");
    let script = format!(
        "echo $$; (sleep 4; printenv WARD_PASS > '{n}') & sleep 4; printenv WARD_PASS > '{m}'; wait",
        n = grandchild.display(),
        m = mark.display()
    );
    let mut s = connect(&w);
    send(&mut s, &run_request("t-livre", &sh(&script), &[]));
    send_stdin(&mut s, b"");
    let pid: u32 = match receive(&mut s) {
        Some(Resposta::Saida { dados, .. }) => text(&dados).trim().parse().unwrap(),
        r => panic!("{r:?}"),
    };
    assert!(alive(pid));
    let before = Instant::now();
    let mut t = connect(&w);
    send(&mut t, r#"{"v":1,"op":"travar","dados":{}}"#);
    assert!(matches!(receive(&mut t), Some(Resposta::Ok)));
    let r = read_session(&mut s);
    assert!(
        before.elapsed() < Duration::from_secs(3),
        "a sessão só acabou com o fim do comando"
    );
    assert_eq!(r.end, Some(137), "{r:?}");
    assert!(
        text(&r.stderr).contains("o cofre travou"),
        "sem o aviso: {r:?}"
    );
    assert!(!alive(pid));
    // After the `sleep` deadline, nothing wrote the password.
    std::thread::sleep(Duration::from_secs(5));
    assert!(!mark.exists(), "o comando usou a senha depois da trava");
    assert!(!grandchild.exists(), "o neto usou a senha depois da trava");
    assert!(tmp_empty(&sock));

    // Locked, a new `ward run` does not run until the unlock (V4.21): released by the
    // unlock, and a lock before the launch catches it too (the registration checks the
    // count).
    let mut s = connect(&w);
    send(
        &mut s,
        &run_request("t-livre", &sh("printf %s \"$WARD_PASS\" | wc -c"), &[]),
    );
    send_stdin(&mut s, b"");
    std::thread::sleep(Duration::from_millis(500));
    unlock_through(&w);
    let r = read_session(&mut s);
    assert_eq!(r.end, Some(0), "{r:?}");
    assert_eq!(text(&r.stdout).trim(), FREE_PASSWORD.len().to_string());

    // A `ward run` that ends before the lock gets no notice and keeps its code.
    let r = run(&w, "t-livre", &sh("exit 3"), &[]);
    assert_eq!(r.end, Some(3), "{r:?}");
    assert!(r.stderr.is_empty(), "{r:?}");
    let log = fs::read(&w.log).unwrap();
    assert!(!contains(&log, FREE_PASSWORD.as_bytes()));
    assert!(contains(&log, "encerrado(s) ao travar".as_bytes()));
}

/// Phase 55 (S8), the two steps of the final re-audit against the real `wardd`:
/// 1. `setsid -f`: the command leaves a grandchild with its own session and ends; the
///    grandchild would write the password 3 s later;
/// 2. `setpgid(0, 0)`: the child leaves the group, the command sleeps and the vault
///    locks; the child would write the password 3 s later.
///
/// Before the fix, both wrote (the group died, they did not). Now: nothing writes, no
/// process is left, the CLI hears the ended notice on the lock. The control without the fix
/// (the runaway survives) is in `ward-platform/tests/subreaper.rs`.
#[test]
fn f55_attack_descendant_outside_group_dies_at_end_and_on_lock() {
    let dev = Dir::new("run-55");
    vault(&dev);
    let sock = SocketDir::new("55");
    let w = start(&dev, &sock);
    unlock_through(&w);

    // 1. `setsid -f` and the command ends.
    let mark1 = dev.0.join("marca-f55-setsid");
    let script = format!(
        "setsid -f /bin/sh -c 'echo $$ > \"{p}\"; sleep 3; printenv WARD_PASS > \"{m}\"'; \
         i=0; while [ ! -s \"{p}\" ] && [ $i -lt 100 ]; do sleep 0.05; i=$((i+1)); done; cat \"{p}\"",
        p = dev.0.join("pid-f55-setsid").display(),
        m = mark1.display()
    );
    let r = run(&w, "t-livre", &sh(&script), &[]);
    assert_eq!(r.end, Some(0), "{r:?}");
    let runaway: u32 = text(&r.stdout).trim().parse().unwrap();
    assert!(
        wait_for(Duration::from_secs(3), || !alive(runaway)),
        "o neto com setsid sobreviveu ao fim do comando"
    );
    assert!(r.stderr.is_empty(), "{r:?}");

    // 2. `setpgid(0, 0)` and the lock with the command alive.
    let mark2 = dev.0.join("marca-f55-setpgid");
    let py = format!(
        "import os,time\npid=os.fork()\nif pid==0:\n os.setpgid(0,0)\n time.sleep(3)\n open({m:?},'w').write(os.environ.get('WARD_PASS',''))\n os._exit(0)\nprint(pid,flush=True)\ntime.sleep(30)\n",
        m = mark2.display().to_string()
    );
    let mut s = connect(&w);
    send(
        &mut s,
        &run_request("t-livre", &["/usr/bin/python3", "-I", "-c", &py], &[]),
    );
    send_stdin(&mut s, b"");
    let child: u32 = match receive(&mut s) {
        Some(Resposta::Saida { dados, .. }) => text(&dados).trim().parse().unwrap(),
        r => panic!("{r:?}"),
    };
    assert!(alive(child));
    let mut t = connect(&w);
    send(&mut t, r#"{"v":1,"op":"travar","dados":{}}"#);
    assert!(matches!(receive(&mut t), Some(Resposta::Ok)));
    let r = read_session(&mut s);
    assert_eq!(r.end, Some(137), "{r:?}");
    assert!(
        text(&r.stderr).contains("o comando foi encerrado"),
        "sem o aviso de encerrado: {r:?}"
    );
    assert!(!alive(child), "o filho fora do grupo sobreviveu à trava");

    // After the deadline of both, nothing wrote the password.
    std::thread::sleep(Duration::from_secs(4));
    assert!(
        !mark1.exists(),
        "o neto com setsid usou a senha depois do fim"
    );
    assert!(
        !mark2.exists(),
        "o filho com setpgid usou a senha depois da trava"
    );
    let log = fs::read(&w.log).unwrap();
    assert!(!contains(&log, FREE_PASSWORD.as_bytes()));
}

/// Phase 54b (D68): whoever declares a null stdin sends nothing afterwards. The child's
/// input closes at launch (`cat` ends at once, without waiting for the end of the CLI)
/// and a stdin frame with data kills the group, without the data reaching the
/// command.
#[test]
fn f54b_attack_null_stdin_with_data_kills_the_command() {
    let dev = Dir::new("run-54b-nula");
    vault(&dev);
    let sock = SocketDir::new("54b-nula");
    let w = start(&dev, &sock);
    unlock_through(&w);
    // No data: `cat` sees the end already, before the empty frame from the CLI.
    let mut s = connect(&w);
    send(
        &mut s,
        &run_request_with("t-livre", &sh("cat; echo fim"), &[], "nula"),
    );
    let r = read_session(&mut s);
    assert_eq!(r.end, Some(0), "{r:?}");
    assert_eq!(text(&r.stdout), "fim\n");
    // With data: the command dies and the data does not arrive.
    let mut s = connect(&w);
    send(
        &mut s,
        &run_request_with("t-livre", &sh("sleep 3; cat; echo fim"), &[], "nula"),
    );
    let before = Instant::now();
    send_stdin(&mut s, b"falso-script-do-agente-54b\n");
    let r = read_session(&mut s);
    assert!(
        before.elapsed() < Duration::from_secs(3),
        "o comando seguiu"
    );
    assert_eq!(r.end, Some(137), "{r:?}");
    let all = [r.stdout.as_slice(), r.stderr.as_slice()].concat();
    assert!(!contains(&all, b"falso-script-do-agente"), "{r:?}");
    assert!(!contains(&all, b"fim"), "{r:?}");
    let log = fs::read(&w.log).unwrap();
    assert!(contains(&log, "entrada padrão declarada nula".as_bytes()));
    assert!(!contains(&log, b"falso-script-do-agente"));
}

const APPROVER: &str = env!("CARGO_BIN_EXE_wardd-aprovador-teste");

/// Attack of the final audit, end to end through the binary (D68): `printf <script> |
/// ward run <entry that needs approval> -- python3`, with the test approval window
/// (opened by the daemon through `WARD_DEV_APP`) answering "allow once" with the right
/// PIN. Without the stdin box, the daemon refuses the answer, the `python3` never runs
/// and nothing comes out; with the box, it runs. With a terminal, "allow once" passes
/// as before.
#[test]
fn f54b_attack_piped_script_on_approval() {
    let dev = Dir::new("run-54b-aprov");
    vault(&dev);
    let app = dev.0.join("wardd-aprovador-teste");
    fs::hard_link(APPROVER, &app).unwrap();
    let mode_file = dev.0.join("aprovador.modo");
    let app_log = dev.0.join("logs/wardd-aprovador-teste.log");
    let sock = SocketDir::new("54b-aprov");
    let w = start_with(&dev, &sock, &[("WARD_DEV_APP", app.as_os_str())]);
    unlock_through(&w);
    // The script measures the password (nothing of it comes out, not even masked).
    let script =
        b"import os, sys\nsys.stdout.write('ENTREGUE:%d' % len(os.environ['WARD_PASS']))\n";
    let answers = |m: &str| {
        fs::read_to_string(&app_log)
            .unwrap_or_default()
            .matches(m)
            .count()
    };

    // 1. Without the box: refused; the request stays pending and nothing runs. The
    // audit's `python3` and a reader of commands from stdin (`sh`, in place of `psql`,
    // which reads SQL the same way; the decision does not look at the argv), by pipe
    // and by file.
    let cases: [(&[&str], &[u8], &str); 3] = [
        (&["/usr/bin/python3", "-I"], script, "pipe"),
        (
            &["/bin/sh"],
            b"printf 'ENTREGUE:%s' \"${#WARD_PASS}\"\n",
            "pipe",
        ),
        (
            &["/bin/sh"],
            b"printf 'ENTREGUE:%s' \"${#WARD_PASS}\"\n",
            "arquivo",
        ),
    ];
    for (n, (argv, data, kind)) in cases.iter().enumerate() {
        fs::write(&mode_file, format!("uma_vez {PIN}\n")).unwrap();
        let mut s = connect(&w);
        send(&mut s, &run_request_with("t-aprov", argv, &[], kind));
        send_stdin(&mut s, data);
        send_stdin(&mut s, b"");
        assert!(
            wait_for(Duration::from_secs(20), || answers(
                "resposta=pedido_invalido"
            ) > n),
            "a janela de teste não respondeu: {}",
            fs::read_to_string(&app_log).unwrap_or_default()
        );
        let lines = fs::read_to_string(&app_log).unwrap();
        assert!(
        lines.contains("pedido operacao=executar entradas=t-aprov origem=- confirmacao=0 entrada_oculta=1 decisao=uma_vez"),
        "{lines}"
    );
        s.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        assert!(
            receive(&mut s).is_none(),
            "o comando rodou sem a confirmação"
        );
        // The CLI gives up: the request is cancelled, nothing ran.
        drop(s);
        assert!(wait_for(Duration::from_secs(5), || {
            String::from_utf8_lossy(&fs::read(&w.log).unwrap())
                .matches("o cliente saiu; cancelada")
                .count()
                > n
        }));
    }

    // 2. With the box: it runs.
    fs::write(&mode_file, format!("uma_vez_ciente {PIN}\n")).unwrap();
    let mut s = connect(&w);
    send(
        &mut s,
        &run_request_with("t-aprov", &["/usr/bin/python3", "-I"], &[], "pipe"),
    );
    send_stdin(&mut s, script);
    send_stdin(&mut s, b"");
    let r = read_session(&mut s);
    assert_eq!(r.end, Some(0), "{r:?}");
    assert_eq!(text(&r.stdout), format!("ENTREGUE:{}", SECRETS[0].len()));

    // 3. Terminal: "allow once" without the box, as before.
    fs::write(&mode_file, format!("uma_vez {PIN}\n")).unwrap();
    let mut s = connect(&w);
    send(
        &mut s,
        &run_request_with(
            "t-aprov",
            &sh("printf %s \"$WARD_PASS\" | wc -c"),
            &[],
            "terminal",
        ),
    );
    send_stdin(&mut s, b"");
    let r = read_session(&mut s);
    assert_eq!(r.end, Some(0), "{r:?}");
    assert_eq!(text(&r.stdout).trim(), SECRETS[0].len().to_string());

    let log = fs::read(&w.log).unwrap();
    for secret in SECRETS {
        assert!(!contains(&log, secret.as_bytes()));
        assert!(!contains(&fs::read(&app_log).unwrap(), secret.as_bytes()));
    }
    assert!(contains(
        &log,
        b"confirma\xc3\xa7\xc3\xa3o da entrada padr\xc3\xa3o"
    ));
}
