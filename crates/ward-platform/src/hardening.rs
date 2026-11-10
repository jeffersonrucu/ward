//! Hardening of the process itself (S8, V3.6, V3.7, V3.25).
//!
//! `wardd` calls [`start_hardened`] as the first thing in `main`, before opening the
//! socket or the vault. The order matters: first `PR_SET_DUMPABLE=0` (from then on no
//! other process of the user reads `/proc/<pid>/mem`/`environ` nor does `ptrace`), then
//! the check for a debugger (one already attached before still shows in `TracerPid`) and
//! of the environment.
//!
//! Environment (D48): the daemon refuses any `LD_*` and glibc's insecure list
//! (`UNSECURE_ENVVARS` of `unsecvars.h`, the one `ld.so` deletes in secure mode), plus
//! the `MALLOC_*` aliases of the tunables. The environment of the installed daemon comes
//! from the unit, so the strict rule costs nothing.
//!
//! Memory (D14 revised by D62): last, `mlockall(MCL_CURRENT | MCL_FUTURE |
//! MCL_ONFAULT)` takes the whole process out of swap (data key, secrets and the Argon2
//! memory). Without an infinite `RLIMIT_MEMLOCK`, `MCL_FUTURE` would make the Argon2
//! allocation fail; so, without the infinite limit or with an `mlockall` error, the
//! release build fails closed (exits ≠ 0 with no socket) and `test-mode` only returns
//! the warning ([`Memory::NotLocked`]), without calling `mlockall`, so `make dev-daemon`
//! starts with the user's regular limit. `fork` and `execve` do not inherit the lock.

use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::os::unix::ffi::OsStrExt;

use crate::process::read_tracer_pid;
use crate::sys;

/// Variables that glibc 2.39 (Ubuntu 24.04) ignores in setuid/setgid programs
/// (`UNSECURE_ENVVARS`), besides the `LD_*` ones, which are already refused by the
/// prefix.
const INSECURE_VARIABLES: &[&str] = &[
    "GCONV_PATH",
    "GETCONF_DIR",
    "GLIBC_TUNABLES",
    "HOSTALIASES",
    "LOCALDOMAIN",
    "LOCPATH",
    "MALLOC_TRACE",
    "NIS_PATH",
    "NLSPATH",
    "RESOLV_HOST_CONF",
    "RES_OPTIONS",
    "TMPDIR",
    "TZDIR",
];

/// Refused prefixes: the loader (`LD_`) and the aliases of the `malloc` tunables
/// (`MALLOC_ARENA_MAX`, `MALLOC_CHECK_`, `MALLOC_PERTURB_`…).
const INSECURE_PREFIXES: &[&str] = &["LD_", "MALLOC_"];

/// Longest variable name shown in the message.
const MAX_VARIABLE_NAME: usize = 64;

/// Failure hardening the process. No variable value nor memory content.
#[derive(Debug)]
pub enum StartError {
    /// `PR_SET_DUMPABLE` or `RLIMIT_CORE` did not end up at 0.
    Harden(io::Error),
    /// Insecure variable in the environment (D48). Only the name, sanitized (letters,
    /// digits and `_`, up to 64), never the value.
    InsecureVariable(String),
    /// There is a debugger (`TracerPid` ≠ 0), for instance `strace` or `gdb`.
    Debugger,
    /// Could not read `/proc/self/status`.
    ReadStatus(io::Error),
    /// `RLIMIT_MEMLOCK` is not infinite (D62): with a limit, `mlockall` with
    /// `MCL_FUTURE` would make the Argon2 allocation fail. Carries the current limit in
    /// bytes.
    MemlockLimit(u64),
    /// `mlockall` (or the read of `RLIMIT_MEMLOCK`) failed (D62).
    LockMemory(io::Error),
}

/// How the memory of the process ended up after [`start_hardened`] (D62).
#[derive(Debug)]
pub enum Memory {
    /// `mlockall(MCL_CURRENT | MCL_FUTURE | MCL_ONFAULT)` applied.
    Locked,
    /// Only in `test-mode`: the memory was not locked; the error says why, for the
    /// warning in the log. In release, the same reason is an error.
    NotLocked(StartError),
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StartError::Harden(e) => write!(f, "não deu para endurecer o processo: {e}"),
            StartError::InsecureVariable(name) => write!(
                f,
                "variável {name} definida no ambiente; o processo não sobe com LD_*, \
                 GLIBC_TUNABLES, GCONV_PATH e as outras da lista insegura da glibc"
            ),
            StartError::Debugger => {
                write!(f, "processo sob depurador (TracerPid ≠ 0); não sobe")
            }
            StartError::ReadStatus(e) => write!(f, "não deu para ler /proc/self/status: {e}"),
            StartError::MemlockLimit(current) => write!(
                f,
                "RLIMIT_MEMLOCK de {current} bytes, e não infinito; o processo trava toda a \
                 memória (mlockall) e sobe pela unit com LimitMEMLOCK=infinity"
            ),
            StartError::LockMemory(e) => {
                write!(
                    f,
                    "não deu para travar a memória do processo (mlockall): {e}"
                )
            }
        }
    }
}

impl std::error::Error for StartError {}

/// `PR_SET_DUMPABLE=0` and `RLIMIT_CORE` 0 (current and maximum), verified after
/// applying. With the process non-dumpable the kernel does not generate a core either,
/// even with `core_pattern` pointing at a program (apport, systemd-coredump).
pub fn harden_process() -> io::Result<()> {
    sys::disable_dumpable()?;
    sys::zero_rlimit_core()?;
    if sys::rlimit_core()? != (0, 0) {
        return Err(io::Error::other("RLIMIT_CORE não ficou em 0"));
    }
    Ok(())
}

/// Verifies its own environment: no insecure variable (D48) and no debugger.
pub fn verify_own_environment() -> Result<(), StartError> {
    if let Some(name) = first_insecure_variable(std::env::vars_os().map(|(k, _)| k)) {
        return Err(StartError::InsecureVariable(name));
    }
    let status = std::fs::read("/proc/self/status").map_err(StartError::ReadStatus)?;
    match read_tracer_pid(&status) {
        Some(0) => Ok(()),
        Some(_) => Err(StartError::Debugger),
        None => Err(StartError::ReadStatus(io::ErrorKind::InvalidData.into())),
    }
}

/// [`harden_process`], [`verify_own_environment`] and the memory lock (D62). Error: the
/// process must exit ≠ 0 with the message, without creating a socket (V3.7, V3.25,
/// V3.34). [`Memory::NotLocked`] only in `test-mode`: the caller warns in the log.
pub fn start_hardened() -> Result<Memory, StartError> {
    harden_process().map_err(StartError::Harden)?;
    verify_own_environment()?;
    let limit = sys::rlimit_memlock().map(|(current, _)| current);
    decide_memory(limit, cfg!(feature = "test-mode"), sys::lock_all_memory)
}

/// The rule of D62, apart from the system calls so the test covers both builds:
/// infinite limit → `mlockall`; finite limit, error reading it or in `mlockall` → error
/// in release and [`Memory::NotLocked`] in `test-mode`. With a finite limit, `mlockall`
/// is never called (`MCL_FUTURE` would break Argon2).
fn decide_memory(
    limit: io::Result<u64>,
    test_mode: bool,
    lock: impl FnOnce() -> io::Result<()>,
) -> Result<Memory, StartError> {
    let failure = match limit {
        Ok(libc::RLIM_INFINITY) => match lock() {
            Ok(()) => return Ok(Memory::Locked),
            Err(e) => StartError::LockMemory(e),
        },
        Ok(current) => StartError::MemlockLimit(current),
        Err(e) => StartError::LockMemory(e),
    };
    if test_mode {
        Ok(Memory::NotLocked(failure))
    } else {
        Err(failure)
    }
}

/// The name is insecure by the list or by the prefix. Compares exact bytes
/// (`ld_preload` is not the loader's).
fn is_insecure(name: &[u8]) -> bool {
    INSECURE_VARIABLES.iter().any(|v| v.as_bytes() == name)
        || INSECURE_PREFIXES
            .iter()
            .any(|p| name.starts_with(p.as_bytes()))
}

/// Name for the message (V3.25, D48): the prefix of the name up to the first byte
/// outside `[A-Za-z0-9_]` (exclusive), with at most [`MAX_VARIABLE_NAME`] characters.
/// Nothing is replaced nor added (no `?`, no `…`): the message only carries bytes that
/// came from the name and belong to that set, so a `\n`, an `\x1b` or a long name do
/// not forge a line nor a terminal sequence in the log.
pub(crate) fn name_for_message(name: &[u8]) -> String {
    name.iter()
        .take(MAX_VARIABLE_NAME)
        .take_while(|&&b| b.is_ascii_alphanumeric() || b == b'_')
        .map(|&b| char::from(b))
        .collect()
}

fn first_insecure_variable<I, S>(names: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    names
        .into_iter()
        .find(|n| is_insecure(n.as_ref().as_bytes()))
        .map(|n| name_for_message(n.as_ref().as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_any_ld_variable() {
        assert_eq!(first_insecure_variable(["PATH", "HOME"]), None);
        for name in ["LD_PRELOAD", "LD_AUDIT", "LD_LIBRARY_PATH", "LD_BIND_NOW"] {
            assert_eq!(
                first_insecure_variable(["PATH", name]).as_deref(),
                Some(name)
            );
        }
        // A made-up name also refuses, and the name goes to the message (V3.25).
        assert_eq!(
            first_insecure_variable(["LD_nome_inventado"]).as_deref(),
            Some("LD_nome_inventado")
        );
        // Only the exact prefix: `OLD_PWD` and `ld_preload` are not the loader's.
        assert_eq!(first_insecure_variable(["OLD_PWD", "ld_preload"]), None);
    }

    #[test]
    fn attack_glibc_insecure_list() {
        for name in INSECURE_VARIABLES {
            assert_eq!(
                first_insecure_variable([*name]).as_deref(),
                Some(*name),
                "{name}"
            );
        }
        for name in ["MALLOC_ARENA_MAX", "MALLOC_CHECK_", "MALLOC_PERTURB_"] {
            assert_eq!(first_insecure_variable([name]).as_deref(), Some(name));
        }
        // Similar, but outside the list.
        assert_eq!(
            first_insecure_variable(["GLIBC_TUNABLES_X", "XTMPDIR", "LANG", "TZ"]),
            None
        );
    }

    #[test]
    fn attack_name_with_control_does_not_go_raw_to_the_message() {
        // V3.25: cuts at the first byte outside `[A-Za-z0-9_]`, without replacing by `?`.
        let n = first_insecure_variable([OsStr::from_bytes(b"LD_\x1b[31m\nx\xff")]).unwrap();
        assert_eq!(n, "LD_");
        let n = first_insecure_variable([OsStr::from_bytes(b"LD_AB\nCD")]).unwrap();
        assert_eq!(n, "LD_AB");
        let n = first_insecure_variable([OsStr::from_bytes(b"LD_X\xffY")]).unwrap();
        assert_eq!(n, "LD_X");
        let n = first_insecure_variable([OsStr::from_bytes(b"LD_\xe2\x80\xa6")]).unwrap();
        assert_eq!(n, "LD_");
        // 200 characters: the first 64, with no `…`.
        let long = format!("LD_{}", "A".repeat(200));
        let n = first_insecure_variable([long.as_str()]).unwrap();
        assert_eq!(n, long[..MAX_VARIABLE_NAME]);
        // Exactly 64 passes whole; the cut by a control byte before the 64th applies.
        let exact = format!("LD_{}", "B".repeat(MAX_VARIABLE_NAME - 3));
        assert_eq!(name_for_message(exact.as_bytes()), exact);
        for name in [
            &b"LD_\x1b]0;titulo\x07"[..],
            b"GLIBC_TUNABLES",
            b"LD_a-b",
            b"LD_\r\nwardd: forjado",
        ] {
            let m = name_for_message(name);
            assert!(m.len() <= MAX_VARIABLE_NAME);
            assert!(
                m.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
                "{m:?}"
            );
            assert!(name.starts_with(m.as_bytes()), "{m:?} não é prefixo");
        }
    }

    /// D62 (V3.34): release fails closed without an infinite limit or with an
    /// `mlockall` error; `test-mode` only warns; with a finite limit `mlockall` is never
    /// called.
    #[test]
    fn d62_memory_release_fails_closed_and_test_mode_warns() {
        let eight_mib = 8 * 1024 * 1024;
        for test_mode in [false, true] {
            let mut called = false;
            let r = decide_memory(Ok(eight_mib), test_mode, || {
                called = true;
                Ok(())
            });
            assert!(!called, "mlockall com limite finito");
            match (test_mode, r) {
                (false, Err(StartError::MemlockLimit(n))) => assert_eq!(n, eight_mib),
                (true, Ok(Memory::NotLocked(StartError::MemlockLimit(n)))) => {
                    assert_eq!(n, eight_mib)
                }
                (m, r) => panic!("modo_teste={m}: {r:?}"),
            }
            let r = decide_memory(Ok(libc::RLIM_INFINITY), test_mode, || {
                Err(io::Error::from_raw_os_error(libc::ENOMEM))
            });
            match (test_mode, r) {
                (false, Err(StartError::LockMemory(_))) => {}
                (true, Ok(Memory::NotLocked(StartError::LockMemory(_)))) => {}
                (m, r) => panic!("modo_teste={m}: {r:?}"),
            }
            let r = decide_memory(Err(io::ErrorKind::Other.into()), test_mode, || Ok(()));
            assert_eq!(r.is_err(), !test_mode);
            let r = decide_memory(Ok(libc::RLIM_INFINITY), test_mode, || Ok(()));
            assert!(matches!(r, Ok(Memory::Locked)));
        }
        let m = StartError::MemlockLimit(eight_mib).to_string();
        assert!(
            m.contains("LimitMEMLOCK=infinity") && m.contains("8388608"),
            "{m}"
        );
    }

    #[test]
    fn message_has_no_value() {
        let m = StartError::InsecureVariable("LD_PRELOAD".to_owned()).to_string();
        assert!(m.contains("LD_PRELOAD"));
        assert!(!m.contains('/'));
    }
}
