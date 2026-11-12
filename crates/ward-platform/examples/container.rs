//! Test binary of the container of Phase 21b (`make test-docker-platform`; D46, D47;
//! V3.5, V3.23, V3.24). Only with `test-mode`; it is never installed outside the
//! container.
//!
//! A single binary, with subcommands, installed under different names:
//! - `activate <socket> <uid> -- <program…>`: the role of the system systemd (runs as
//!   root): creates the socket, hands it to the uid, 0600, and executes the program as
//!   that uid (`setpriv`) with the socket on fd 3 and `LISTEN_FDS=1`;
//! - `server --chrome <exe> [--launch <program…>]`: the test "wardd": hardens itself,
//!   receives the socket through `LISTEN_FDS`, reads the groups from `/etc/group` and
//!   answers each connection with one line with the role; with `--launch`, it opens a
//!   child (the app opened by the daemon). If the client sends a frame right away (the
//!   test app of 23a asks for the vault status at startup, D63), the reply is the status
//!   frame with the role, and the connection closes;
//! - `client [--system-at <socket>] [--pause]`: verifies the server (D46), connects and
//!   prints the role; installed as `ward-app` and `ward-nm` (setgid);
//! - `attach <program…>`: launches the program (a `client --pause`) and tries
//!   `PTRACE_ATTACH`, `PTRACE_SEIZE` and reading its `/proc/<pid>/mem`;
//! - `traceme <program…>`: launches the program already under `ptrace`
//!   (`PTRACE_TRACEME`) and shows the egid after the `exec`: under a debugger the kernel
//!   does not apply the setgid;
//! - `bind <socket>`: tries to create a socket (the attacker in the root's folder).
//!
//! Phase 29a (D57; V5.31), with the test `ward-app` of `app/core`
//! (`startup-container`):
//! - `server … --open-app <bin>`: the daemon opens the app through the launcher of D57
//!   (and tries to open it again right after: one instance at a time);
//! - `server … --open-app-with <bin> <VAR=value>`: the daemon opens the app with the
//!   environment of D57 and one variable forced in (test of the app's checker);
//! - `launch-app <bin>`: only the launcher, for the wrong session cases (two
//!   `wayland-<n>`, link);
//! - `listen <socket>`: creates a socket and keeps listening (fake Wayland and bus);
//! - `environment`: prints the environment of D57 (`VAR=value` per line);
//! - `--user` (called as `/usr/lib/systemd/systemd --user`): imitates the user's manager
//!   and opens the program named by the manager-program variable with the environment of
//!   D57.
//!
//! The subcommands and flags are the interface with `dev/container-platform/inside.sh`
//! and `dev/container-install/inside.sh`; the printed markers are what those scripts look
//! for and stay as they are.
//!
//! `unsafe` only for the `ptrace`/`waitpid`/`prctl` of the attacks, with `// SAFETY:`.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ward_platform::{
    AllowedBinary, ExpectedServer, Install, KernelPolicy, Launcher, PeerClass, app_environment,
    connect_server, listen_from_listen_fds, start_hardened, system_socket_path, verify_connection,
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cmd, rest) = args
        .split_first()
        .map(|(c, r)| (c.as_str(), r))
        .unwrap_or(("", &[]));
    match cmd {
        "activate" => activate(rest),
        "server" => server(rest),
        "client" => client(rest),
        "attach" => attach(rest),
        "traceme" => traceme(rest),
        "bind" => bind(rest),
        "launch-app" => launch_app(rest),
        "listen" => listen(rest),
        "environment" => environment(),
        "--user" => manager(),
        _ => {
            eprintln!(
                "uso: activate|server|client|attach|traceme|bind|launch-app|listen|environment"
            );
            std::process::exit(2);
        }
    }
}

fn fail(msg: impl std::fmt::Display) -> ! {
    eprintln!("erro: {msg}");
    std::process::exit(1)
}

fn activate(args: &[String]) {
    let [socket, uid, separator, program @ ..] = args else {
        fail("activate <socket> <uid> -- <programa…>");
    };
    if separator != "--" || program.is_empty() {
        fail("falta -- <programa…>");
    }
    let uid: u32 = uid.parse().unwrap_or_else(|_| fail("uid"));
    let _ = std::fs::remove_file(socket);
    let listener = UnixListener::bind(socket).unwrap_or_else(|e| fail(e));
    std::os::unix::fs::chown(socket, Some(uid), None).unwrap_or_else(|e| fail(e));
    std::fs::set_permissions(socket, std::os::unix::fs::PermissionsExt::from_mode(0o600))
        .unwrap_or_else(|e| fail(e));
    let mut cmd = Command::new("/usr/bin/setpriv");
    cmd.arg(format!("--reuid={uid}"))
        .arg(format!("--regid={uid}"))
        .arg("--clear-groups")
        .arg("--")
        .args(program)
        .env_clear()
        .env("PATH", "/usr/bin:/bin");
    ward_platform::testing::run_with_socket(&listener, cmd);
}

/// fd of `listener` in `/proc/self/fd`, fd 3 still open and the `O_CLOEXEC` of the fd.
fn fds_state(listener: &UnixListener) -> String {
    use std::os::fd::AsRawFd;
    let fd = listener.as_raw_fd();
    let info = std::fs::read_to_string(format!("/proc/self/fdinfo/{fd}")).unwrap_or_default();
    let flags = info
        .lines()
        .find_map(|l| l.strip_prefix("flags:"))
        .and_then(|f| u32::from_str_radix(f.trim(), 8).ok())
        .unwrap_or(0);
    let cloexec = flags & 0o2_000_000 != 0;
    let fd3 = Path::new("/proc/self/fd/3").exists();
    format!("fd3_aberto={fd3} cloexec={cloexec}")
}

/// How the app will be opened by the server (Phase 29a).
enum Opening {
    /// Through the launcher of D57.
    Launcher(String),
    /// Environment of D57 with `VAR=value` forced in.
    With(String, String),
}

fn server(args: &[String]) {
    let mut chrome = None;
    let mut launch: Vec<String> = Vec::new();
    let mut openings: Vec<Opening> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--chrome" => {
                chrome = args.get(i + 1).cloned();
                i += 2;
            }
            "--open-app" => {
                let bin = args
                    .get(i + 1)
                    .cloned()
                    .unwrap_or_else(|| fail("--open-app"));
                openings.push(Opening::Launcher(bin));
                i += 2;
            }
            "--open-app-with" => {
                let (Some(bin), Some(var)) = (args.get(i + 1), args.get(i + 2)) else {
                    fail("--open-app-with <bin> <VAR=valor>");
                };
                openings.push(Opening::With(bin.clone(), var.clone()));
                i += 3;
            }
            "--launch" => {
                launch = args[i + 1..].to_vec();
                break;
            }
            _ => fail("argumento"),
        }
    }
    if let Err(e) = start_hardened() {
        fail(e);
    }
    let listening = listen_from_listen_fds(&system_socket_path()).unwrap_or_else(|e| fail(e));
    let chrome =
        AllowedBinary::new(chrome.unwrap_or_else(|| fail("--chrome"))).unwrap_or_else(|e| fail(e));
    let inst = Install::from_system(chrome).unwrap_or_else(|e| fail(e));
    let policy = KernelPolicy::read();
    println!(
        "servidor: pronto {} gid_app={} gid_nm={} politica={:?} vmlck={}",
        fds_state(listening.listener()),
        inst.app_gid(),
        inst.extension_gid(),
        policy.verify(),
        status("VmLck:")
    );
    let _ = std::io::stdout().flush();
    if let Some((program, rest)) = launch.split_first() {
        let output = Command::new(program)
            .args(rest)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn();
        match output {
            Ok(child) => {
                std::thread::spawn(move || {
                    let output = child
                        .wait_with_output()
                        .map(|o| o.stdout)
                        .unwrap_or_default();
                    println!("app-filho: {}", String::from_utf8_lossy(&output).trim());
                    let _ = std::io::stdout().flush();
                });
            }
            Err(e) => println!("app-filho: erro {e}"),
        }
    }
    let _launchers = open_apps(&openings, &inst);
    for connection in listening.listener().incoming() {
        let Ok(mut stream) = connection else { continue };
        let peer = match verify_connection(&stream) {
            Ok(p) => p,
            Err(e) => {
                println!("servidor: recusado {e:?}");
                let _ = std::io::stdout().flush();
                continue;
            }
        };
        let class = peer.classify(&inst, &policy);
        let line = match class {
            PeerClass::App => "papel=app".to_owned(),
            PeerClass::ExtensionHost => "papel=extensao".to_owned(),
            PeerClass::Common { reason } => format!("papel=agente motivo={reason:?}"),
        };
        println!(
            "servidor: pid={} gid={} {line}",
            peer.credentials().pid,
            peer.credentials().gid
        );
        let _ = std::io::stdout().flush();
        let role = match class {
            PeerClass::App => "app",
            PeerClass::ExtensionHost => "extension",
            PeerClass::Common { .. } => "agent",
        };
        if !answer_status(&mut stream, role) {
            let _ = writeln!(stream, "{line}");
        }
    }
}

/// Phase 23a (D63): the test app asks for the vault status right after connecting and
/// checks the role in the reply. If a frame arrives within 1 s, it answers with the status
/// (no secret, only the role) and returns `true`; otherwise (the `client` subcommand,
/// which only reads), `false`. The frame is protocol v2 (m2:DT8): the app reads it
/// strictly and stops on another version.
fn answer_status(stream: &mut UnixStream, role: &str) -> bool {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(1)));
    let mut n = [0u8; 4];
    if stream.read_exact(&mut n).is_err() {
        return false;
    }
    let size = u32::from_be_bytes(n) as usize;
    if size == 0 || size > 64 * 1024 {
        return true;
    }
    let mut body = vec![0u8; size];
    if stream.read_exact(&mut body).is_err() {
        return true;
    }
    // The app opened outside the daemon (the one from the menu) only asks the daemon to
    // open the app: "ok", like the daemon.
    let open_app = body.windows(8).any(|j| j == b"open_app");
    let r = if open_app {
        r#"{"v":2,"type":"ok","data":{}}"#.to_owned()
    } else {
        format!(
            r#"{{"v":2,"type":"status","data":{{"locked":true,"vault_exists":false,"role":"{role}"}}}}"#
        )
    };
    let _ = stream
        .write_all(&(r.len() as u32).to_be_bytes())
        .and_then(|()| stream.write_all(r.as_bytes()));
    true
}

fn status(field: &str) -> String {
    std::fs::read_to_string("/proc/self/status")
        .unwrap_or_default()
        .lines()
        .find_map(|l| {
            l.strip_prefix(field)
                .map(|v| v.split_whitespace().collect::<Vec<_>>().join(","))
        })
        .unwrap_or_default()
}

fn client(args: &[String]) {
    let mut socket: Option<PathBuf> = None;
    let mut pause = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--system-at" => {
                socket = args.get(i + 1).map(PathBuf::from);
                i += 2;
            }
            "--pause" => {
                pause = true;
                i += 1;
            }
            _ => fail("argumento"),
        }
    }
    let expected = match socket {
        Some(s) => ExpectedServer::system_at(s),
        None => ExpectedServer::system(),
    };
    let mark = std::env::var_os("LD_PRELOAD").is_some();
    if pause {
        println!("pronto gid={}", status("Gid:"));
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
    }
    let stream: UnixStream = match connect_server(&expected) {
        Ok(s) => s,
        Err(e) => {
            println!("servidor={e:?}");
            return;
        }
    };
    let mut reply = String::new();
    let _ = BufReader::new(stream).read_line(&mut reply);
    println!(
        "servidor=ok {} gid={} dumpable={} ld_preload_no_ambiente={mark}",
        reply.trim(),
        status("Gid:"),
        dumpable()
    );
}

#[allow(unsafe_code)]
fn dumpable() -> i32 {
    // SAFETY: `PR_GET_DUMPABLE` takes no pointer; it returns the value in the return.
    unsafe { libc::prctl(libc::PR_GET_DUMPABLE, 0, 0, 0, 0) }
}

fn error_of(r: libc::c_long) -> String {
    if r == 0 {
        "ok".to_owned()
    } else {
        let e = std::io::Error::last_os_error();
        match e.raw_os_error() {
            Some(libc::EPERM) => "EPERM".to_owned(),
            Some(libc::EACCES) => "EACCES".to_owned(),
            _ => format!("{e}"),
        }
    }
}

#[allow(unsafe_code)]
fn attach(args: &[String]) {
    let Some((program, rest)) = args.split_first() else {
        fail("attach <programa…>");
    };
    let mut child = Command::new(program)
        .args(rest)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| fail(e));
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut ready = String::new();
    output.read_line(&mut ready).unwrap_or_else(|e| fail(e));
    let pid = libc::pid_t::try_from(child.id()).unwrap_or_else(|_| fail("pid"));
    // SAFETY: `PTRACE_ATTACH` on a pid of ours, without pointers (null address and data).
    let attach = unsafe {
        libc::ptrace(
            libc::PTRACE_ATTACH,
            pid,
            std::ptr::null_mut::<libc::c_void>(),
            std::ptr::null_mut::<libc::c_void>(),
        )
    };
    let attach = error_of(attach);
    if attach == "ok" {
        // Control (binary without setgid): waits for the attach stop and releases the
        // child.
        let mut wstatus: libc::c_int = 0;
        // SAFETY: `waitpid` of a child of ours, with a local `wstatus`.
        unsafe { libc::waitpid(pid, &raw mut wstatus, 0) };
        // SAFETY: releases the stopped child, without a signal (null data).
        unsafe {
            libc::ptrace(
                libc::PTRACE_DETACH,
                pid,
                std::ptr::null_mut::<libc::c_void>(),
                std::ptr::null_mut::<libc::c_void>(),
            )
        };
    }
    // SAFETY: as above, with `PTRACE_SEIZE` and options 0.
    let seize = unsafe {
        libc::ptrace(
            libc::PTRACE_SEIZE,
            pid,
            std::ptr::null_mut::<libc::c_void>(),
            std::ptr::null_mut::<libc::c_void>(),
        )
    };
    let seize = error_of(seize);
    let mem = match std::fs::File::open(format!("/proc/{pid}/mem")) {
        Ok(_) => "ok".to_owned(),
        Err(e) if e.raw_os_error() == Some(libc::EACCES) => "EACCES".to_owned(),
        Err(e) if e.raw_os_error() == Some(libc::EPERM) => "EPERM".to_owned(),
        Err(e) => format!("{e}"),
    };
    let tracer = std::fs::read_to_string(format!("/proc/{pid}/status"))
        .unwrap_or_default()
        .lines()
        .find_map(|l| l.strip_prefix("TracerPid:").map(|v| v.trim().to_owned()))
        .unwrap_or_default();
    let _ = child.stdin.take().unwrap().write_all(b"\n");
    let mut rest_output = String::new();
    let _ = output.read_to_string(&mut rest_output);
    let _ = child.wait();
    println!(
        "prender: {} attach={attach} seize={seize} mem={mem} tracer={tracer} | {}",
        ready.trim(),
        rest_output.trim()
    );
}

#[allow(unsafe_code)]
fn traceme(args: &[String]) {
    use std::os::unix::process::CommandExt;
    let Some((program, rest)) = args.split_first() else {
        fail("traceme <programa…>");
    };
    let mut cmd = Command::new(program);
    cmd.args(rest)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .stdout(Stdio::piped());
    // SAFETY: the closure runs in the child between the fork and the exec and only calls
    // `ptrace` (async-signal-safe), without allocating or touching a lock.
    unsafe {
        cmd.pre_exec(|| {
            let r = libc::ptrace(
                libc::PTRACE_TRACEME,
                0,
                std::ptr::null_mut::<libc::c_void>(),
                std::ptr::null_mut::<libc::c_void>(),
            );
            if r != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = cmd.spawn().unwrap_or_else(|e| fail(e));
    let pid = libc::pid_t::try_from(child.id()).unwrap_or_else(|_| fail("pid"));
    let mut wstatus: libc::c_int = 0;
    // SAFETY: `waitpid` of a child of ours, with a local `wstatus`.
    let r = unsafe { libc::waitpid(pid, &raw mut wstatus, 0) };
    if r != pid || !libc::WIFSTOPPED(wstatus) {
        fail("o filho não parou no exec");
    }
    let gid = std::fs::read_to_string(format!("/proc/{pid}/status"))
        .unwrap_or_default()
        .lines()
        .find_map(|l| {
            l.strip_prefix("Gid:")
                .map(|v| v.split_whitespace().collect::<Vec<_>>().join(","))
        })
        .unwrap_or_default();
    // SAFETY: releases the stopped child, without a signal (data 0).
    let r = unsafe {
        libc::ptrace(
            libc::PTRACE_DETACH,
            pid,
            std::ptr::null_mut::<libc::c_void>(),
            std::ptr::null_mut::<libc::c_void>(),
        )
    };
    if r != 0 {
        fail("PTRACE_DETACH");
    }
    let mut output = String::new();
    let _ = child.stdout.take().unwrap().read_to_string(&mut output);
    let _ = child.wait();
    println!("traceme: gid_no_exec={gid} | {}", output.trim());
}

fn bind(args: &[String]) {
    let [socket] = args else {
        fail("bind <socket>")
    };
    match UnixListener::bind(socket) {
        Ok(_) => {
            let _ = std::fs::remove_file(socket);
            println!("bind=ok");
        }
        Err(e) if e.raw_os_error() == Some(libc::EACCES) => println!("bind=EACCES"),
        Err(e) => println!("bind={e}"),
    }
}

/// Opens the requested apps (Phase 29a). Returns the launchers, so the children are not
/// reaped too early.
fn open_apps(openings: &[Opening], inst: &Install) -> Vec<Launcher> {
    let mut launchers = Vec::new();
    for a in openings {
        match a {
            Opening::Launcher(bin) => {
                let b = AllowedBinary::new(bin).unwrap_or_else(|e| fail(e));
                let mut l = Launcher::new(b, inst);
                match l.open() {
                    Ok(pid) => println!("lancador: aberto pid={pid}"),
                    Err(e) => println!("lancador: erro {e:?}"),
                }
                match l.open() {
                    Ok(pid) => println!("lancador: segundo aberto pid={pid}"),
                    Err(e) => println!("lancador: segundo {e:?}"),
                }
                launchers.push(l);
            }
            Opening::With(bin, var) => {
                let Some((name, value)) = var.split_once('=') else {
                    fail("VAR=valor");
                };
                let app_env = app_environment().unwrap_or_else(|e| fail(e));
                let child = Command::new(bin)
                    .env_clear()
                    .envs(app_env.variables().filter(|(k, _)| *k != name))
                    .env(name, value)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn();
                match child {
                    Ok(f) => println!("forcado: aberto pid={} {name}", f.id()),
                    Err(e) => println!("forcado: erro {e}"),
                }
            }
        }
        let _ = std::io::stdout().flush();
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    launchers
}

fn launch_app(args: &[String]) {
    let [bin] = args else {
        fail("launch-app <bin>")
    };
    let chrome = AllowedBinary::new("/usr/bin/bash").unwrap_or_else(|e| fail(e));
    let inst = Install::from_system(chrome).unwrap_or_else(|e| fail(e));
    let b = AllowedBinary::new(bin).unwrap_or_else(|e| fail(e));
    match Launcher::new(b, &inst).open() {
        Ok(pid) => println!("lancar-app: aberto pid={pid}"),
        Err(e) => println!(
            "lancar-app: {e:?} indisponivel={}",
            matches!(&e, ward_platform::LauncherError::Session(s) if s.session_unavailable())
        ),
    }
}

fn listen(args: &[String]) {
    let [socket] = args else {
        fail("listen <socket>")
    };
    let _ = std::fs::remove_file(socket);
    let _l = UnixListener::bind(socket).unwrap_or_else(|e| fail(e));
    println!("escutando");
    let _ = std::io::stdout().flush();
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}

fn environment() {
    let app_env = app_environment().unwrap_or_else(|e| fail(e));
    for (k, v) in app_env.variables() {
        println!("{k}={v}");
    }
}

/// Imitates `systemd --user` (Phase 29a, V5.31): `Name: systemd`, `cmdline`
/// `/usr/lib/systemd/systemd --user`, orphan (`PPid: 1`). Opens the program with the
/// environment of D57, as a unit of `~/.config/systemd/user/` would.
fn manager() {
    let program =
        std::env::var("WARD_MANAGER_PROGRAM").unwrap_or_else(|_| fail("WARD_MANAGER_PROGRAM"));
    // The subshell that left this process orphaned exits right away; wait for `PPid` to
    // become 1.
    std::thread::sleep(std::time::Duration::from_millis(300));
    let cmdline = std::fs::read("/proc/self/cmdline").unwrap_or_default();
    println!(
        "gerente: nome={} ppid={} cmdline={}",
        status("Name:"),
        status("PPid:"),
        String::from_utf8_lossy(&cmdline).replace('\0', " ").trim()
    );
    let app_env = app_environment().unwrap_or_else(|e| fail(e));
    let status = Command::new(program)
        .env_clear()
        .envs(app_env.variables())
        .stdin(Stdio::null())
        .status();
    println!("gerente: {status:?}");
}
