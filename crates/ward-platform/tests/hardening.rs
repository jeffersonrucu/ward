//! Hardening of the process (V3.6, V3.7; S8).
//!
//! The hardened process is this same test binary, called again with
//! `--exact helper_child` and the [`MODE`] variable saying what to do.
//! Attacks: reading `/proc/<pid>/mem` and `environ`, `ptrace` (also by the parent, which
//! Yama would allow), `kill -SEGV` looking for a core, starting with `LD_*` or under
//! `strace`.

mod common;

use std::io::{BufRead, BufReader, Read};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const MODE: &str = "WARD_PLATAFORMA_FILHO";
const MARK: &str = "RESULTADO:";

/// Not a real test: it is the body of the child process. Without the variable, does
/// nothing.
#[test]
fn helper_child() {
    let Ok(mode) = std::env::var(MODE) else {
        return;
    };
    match mode.as_str() {
        "endurecer" | "endurecer-segv" | "sem-endurecer" => {
            if mode != "sem-endurecer" {
                ward_platform::harden_process().unwrap();
            }
            if mode == "endurecer-segv" {
                default_sigsegv();
            }
            println!("{MARK}pronto");
            // Waits for stdin to close.
            let _ = std::io::stdin().read_to_end(&mut Vec::new());
        }
        "iniciar" => match ward_platform::start_hardened() {
            Ok(ward_platform::Memory::Locked) => println!("{MARK}ok"),
            Ok(ward_platform::Memory::NotLocked(e)) => println!("{MARK}ok-sem-travar:{e}"),
            Err(e) => {
                println!("{MARK}erro:{e}");
                std::process::exit(3);
            }
        },
        "mlockall-e-filho" => {
            if mlockall_like_the_daemon() != 0 {
                println!("{MARK}sem-mlockall");
                return;
            }
            let mine = vmlck(&std::fs::read_to_string("/proc/self/status").unwrap());
            let child = Command::new("/bin/cat")
                .arg("/proc/self/status")
                .env_clear()
                .output()
                .unwrap();
            let theirs = vmlck(&String::from_utf8_lossy(&child.stdout));
            println!("{MARK}{mine} {theirs}");
        }
        _ => panic!("modo desconhecido"),
    }
}

/// The same flags as `start_hardened` (D62), here without the limit rule, to see what
/// the child inherits.
#[allow(unsafe_code)]
fn mlockall_like_the_daemon() -> i32 {
    // SAFETY: `mlockall` only receives the flags; it neither reads nor writes memory of
    // the process.
    unsafe { libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE | libc::MCL_ONFAULT) }
}

/// Rust's `std` installs a SIGSEGV handler (for stack overflow) that, on a SIGSEGV sent
/// by `kill`, just returns and leaves the process alive. For the core test, the child
/// goes back to the default action (die and, if allowed, generate a core).
#[allow(unsafe_code)]
fn default_sigsegv() {
    // SAFETY: swaps the action of a signal for the default one; no function pointer of
    // ours.
    let r = unsafe { libc::signal(libc::SIGSEGV, libc::SIG_DFL) };
    assert_ne!(r, libc::SIG_ERR);
}

/// `VmLck` of `/proc/<pid>/status`, in kB.
fn vmlck(status: &str) -> u64 {
    status
        .lines()
        .find_map(|l| l.strip_prefix("VmLck:"))
        .and_then(|v| v.trim().strip_suffix("kB"))
        .and_then(|v| v.trim().parse().ok())
        .expect("VmLck no status")
}

fn child_command(program: Option<&Path>, mode: &str) -> Command {
    let me = std::env::current_exe().unwrap();
    let mut c = match program {
        Some(p) => {
            let mut c = Command::new(p);
            c.args(["-f", "-o", "/dev/null"]).arg(&me);
            c
        }
        None => Command::new(&me),
    };
    c.args(["--exact", "helper_child", "--nocapture", "--test-threads=1"])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env(MODE, mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    c
}

/// Reads the child's stdout up to the line with the mark and returns the rest of it.
fn result(output: &mut impl BufRead) -> String {
    let mut line = String::new();
    let mut read = String::new();
    loop {
        line.clear();
        assert!(
            output.read_line(&mut line).unwrap() > 0,
            "filho saiu sem resultado; saída: {read:?}"
        );
        read.push_str(&line);
        // libtest writes "test helper_child ... " before, on the same line.
        if let Some((_, r)) = line.trim().split_once(MARK) {
            return r.to_owned();
        }
    }
}

struct Hardened {
    child: std::process::Child,
    _output: BufReader<std::process::ChildStdout>,
}

fn spawn(mode: &str, cwd: Option<&Path>) -> Hardened {
    let mut c = child_command(None, mode);
    if let Some(d) = cwd {
        c.current_dir(d);
    }
    let mut child = c.spawn().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    assert_eq!(result(&mut output), "pronto");
    Hardened {
        child,
        _output: output,
    }
}

fn read_proc(pid: u32, file: &str) -> std::io::Result<Vec<u8>> {
    let mut buf = vec![0u8; 64];
    let mut f = std::fs::File::open(format!("/proc/{pid}/{file}"))?;
    let n = f.read(&mut buf)?;
    buf.truncate(n);
    Ok(buf)
}

#[allow(unsafe_code)]
fn ptrace_seize(pid: u32) -> std::io::Result<()> {
    let pid = libc::pid_t::try_from(pid).unwrap();
    // SAFETY: `PTRACE_SEIZE` only receives integers (pid, address 0, options 0); it does
    // not touch memory of this process.
    let r = unsafe {
        libc::ptrace(
            libc::PTRACE_SEIZE,
            pid,
            std::ptr::null_mut::<libc::c_void>(),
            std::ptr::null_mut::<libc::c_void>(),
        )
    };
    if r != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // If it somehow succeeded, detach before going on.
    // SAFETY: as above; `PTRACE_DETACH` with signal 0.
    unsafe {
        libc::ptrace(
            libc::PTRACE_DETACH,
            pid,
            std::ptr::null_mut::<libc::c_void>(),
            std::ptr::null_mut::<libc::c_void>(),
        );
    }
    Ok(())
}

#[test]
fn attack_read_memory_environment_and_ptrace_of_the_hardened_process() {
    // Control: without hardening, the same uid reads the environment (the attack would
    // work).
    let mut control = spawn("sem-endurecer", None);
    assert!(read_proc(control.child.id(), "environ").is_ok());
    drop(control.child.stdin.take());
    control.child.wait().unwrap();

    let mut e = spawn("endurecer", None);
    let pid = e.child.id();
    for file in ["mem", "environ", "maps"] {
        let r = read_proc(pid, file);
        assert_eq!(
            r.as_ref().map_err(|e| e.kind()).err(),
            Some(std::io::ErrorKind::PermissionDenied),
            "/proc/<pid>/{file} legível"
        );
    }
    // The test is the child's parent: Yama (ptrace_scope 1) would allow it; dumpable=0
    // blocks it.
    let r = ptrace_seize(pid);
    assert_eq!(r.map_err(|e| e.raw_os_error()), Err(Some(libc::EPERM)));
    // `strace -p` and `gdb -p` depend on the same ptrace.
    let st = Command::new("/usr/bin/strace")
        .args(["-o", "/dev/null", "-p", &pid.to_string()])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(!st.success(), "strace -p conseguiu");

    // Core off: `Max core file size 0 0`.
    let limits = std::fs::read_to_string(format!("/proc/{pid}/limits")).unwrap();
    let core = limits
        .lines()
        .find(|l| l.starts_with("Max core file size"))
        .unwrap();
    let fields: Vec<&str> = core.split_whitespace().collect();
    assert_eq!(&fields[4..6], &["0", "0"], "{core}");

    drop(e.child.stdin.take());
    e.child.wait().unwrap();
}

#[test]
fn attack_sigsegv_leaves_no_core() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("ward-platform-core-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut e = spawn("endurecer-segv", Some(&dir));
    let pid = libc::pid_t::try_from(e.child.id()).unwrap();
    // `kill` through /usr/bin/kill, no unsafe here.
    let st = Command::new("/usr/bin/kill")
        .args(["-SEGV", &pid.to_string()])
        .status()
        .unwrap();
    assert!(st.success());
    let end = e.child.wait().unwrap();
    assert_eq!(end.signal(), Some(libc::SIGSEGV));
    assert!(!end.core_dumped(), "o kernel gerou core");
    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        0,
        "arquivo de core"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn start_with(var: Option<(&str, &str)>, under_strace: bool) -> (String, Option<i32>) {
    let strace = Path::new("/usr/bin/strace");
    let mut c = child_command(under_strace.then_some(strace), "iniciar");
    if let Some((k, v)) = var {
        c.env(k, v);
    }
    let mut child = c.spawn().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let r = result(&mut output);
    drop(child.stdin.take());
    let st = child.wait().unwrap();
    (r, st.code())
}

#[test]
fn starts_clean_without_ld_and_without_debugger() {
    let (r, code) = start_with(None, false);
    let infinite = memlock_limit_infinite();
    if infinite {
        assert_eq!(r, "ok");
        assert_eq!(code, Some(0));
    } else if cfg!(feature = "test-mode") {
        // D62: with the user's regular limit, test mode only warns.
        assert!(r.starts_with("ok-sem-travar:"), "{r}");
        assert_eq!(code, Some(0));
    } else {
        assert!(r.starts_with("erro:") && r.contains("LimitMEMLOCK"), "{r}");
        assert_ne!(code, Some(0));
    }
}

fn memlock_limit_infinite() -> bool {
    std::fs::read_to_string("/proc/self/limits")
        .unwrap()
        .lines()
        .any(|l| l.starts_with("Max locked memory") && l.contains("unlimited"))
}

/// V3.34 (D62, S8): with an 8 MiB `RLIMIT_MEMLOCK`, the release `start_hardened`
/// refuses to start (the `test-mode` one, which is this one, only warns and locks
/// nothing); the message says the limit and the unit directive.
#[test]
fn v3_34_memlock_of_8_mib() {
    let me = std::env::current_exe().unwrap();
    let mut c = Command::new("/usr/bin/prlimit");
    c.arg("--memlock=8388608:8388608").arg("--").arg(&me);
    c.args(["--exact", "helper_child", "--nocapture", "--test-threads=1"])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env(MODE, "iniciar")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = c.spawn().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let r = result(&mut output);
    drop(child.stdin.take());
    let st = child.wait().unwrap();
    assert!(
        r.contains("8388608") && r.contains("LimitMEMLOCK=infinity"),
        "{r}"
    );
    if cfg!(feature = "test-mode") {
        assert!(r.starts_with("ok-sem-travar:"), "{r}");
        assert_eq!(st.code(), Some(0));
    } else {
        assert!(r.starts_with("erro:"), "{r}");
        assert_ne!(st.code(), Some(0));
    }
}

/// V3.34 (D62): the daemon's `mlockall` does not pass to the child (`fork` + `execve`):
/// the locked process has `VmLck` > 0, and the program it runs, `VmLck: 0 kB` (the case
/// of the launched app and of the children of `ward run`).
#[test]
fn v3_34_child_does_not_inherit_the_mlockall() {
    let mut child = child_command(None, "mlockall-e-filho").spawn().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let r = result(&mut output);
    drop(child.stdin.take());
    let _ = child.wait();
    if r == "sem-mlockall" {
        eprintln!("v3_34: o limite do usuário não deixa o mlockall; caso coberto no contêiner");
        return;
    }
    let (mine, theirs) = r.split_once(' ').unwrap();
    let (mine, theirs): (u64, u64) = (mine.parse().unwrap(), theirs.parse().unwrap());
    assert!(mine > 0, "o processo travado tem VmLck 0");
    assert_eq!(theirs, 0, "o filho herdou o travamento");
}

#[test]
fn attack_starting_with_ld_refuses() {
    for (name, value) in [
        ("LD_PRELOAD", ""),
        ("LD_AUDIT", ""),
        ("LD_LIBRARY_PATH", "/nao-existe-ward"),
    ] {
        let (r, code) = start_with(Some((name, value)), false);
        assert!(r.starts_with("erro:"), "{name}: {r}");
        assert!(r.contains(name), "{r}");
        assert!(!r.contains("/nao-existe-ward"), "valor na mensagem: {r}");
        assert_ne!(code, Some(0));
    }
}

#[test]
fn attack_starting_with_the_glibc_insecure_list_refuses() {
    // V3.25 (D48): the name goes in the message, the value does not.
    for (name, value) in [
        ("GLIBC_TUNABLES", "glibc.malloc.check=3:valor-falso-ward"),
        ("GCONV_PATH", "/tmp/valor-falso-ward"),
        ("LD_nome_inventado_ward", "valor-falso-ward"),
        ("MALLOC_ARENA_MAX", "valor-falso-ward"),
        ("LOCPATH", "/tmp/valor-falso-ward"),
    ] {
        let (r, code) = start_with(Some((name, value)), false);
        assert!(r.starts_with("erro:"), "{name}: {r}");
        assert!(r.contains(name), "sem o nome: {r}");
        assert!(!r.contains("valor-falso-ward"), "valor na mensagem: {r}");
        assert_ne!(code, Some(0), "{name}");
    }
}

/// V3.25 (M3 scrutiny): a name with `\n`, `\x1b` or 200 characters comes out cut at the
/// first byte outside `[A-Za-z0-9_]` and at the 64th, with no `?` nor `…`, and without
/// the value.
#[test]
fn attack_v3_25_forged_name_comes_out_cut() {
    let long = format!("LD_{}", "Z".repeat(197));
    for (name, expected) in [
        ("LD_FORJA\nwardd: destravado", "LD_FORJA"),
        ("LD_COR\x1b[31mvermelho", "LD_COR"),
        ("GCONV_PATH", "GCONV_PATH"),
        (long.as_str(), &long[..64]),
    ] {
        let (r, code) = start_with(Some((name, "valor-falso-ward")), false);
        assert_ne!(code, Some(0), "{name:?}");
        let m = r
            .strip_prefix("erro:")
            .unwrap_or_else(|| panic!("{name:?}: {r}"));
        assert!(!m.contains("valor-falso-ward"), "valor na mensagem: {r}");
        assert!(!m.contains('?') && !m.contains('…'), "marca de corte: {r}");
        assert!(!m.contains('\x1b') && !m.contains('\n'), "controle: {r:?}");
        let after = m
            .strip_prefix(&format!("variável {expected} "))
            .unwrap_or_else(|| panic!("{name:?}: nome cortado errado: {r}"));
        assert!(after.starts_with("definida"), "{r}");
    }
}

#[test]
fn attack_starting_under_strace_refuses() {
    let (r, code) = start_with(None, true);
    assert!(r.starts_with("erro:") && r.contains("depurador"), "{r}");
    assert_ne!(code, Some(0));
}
