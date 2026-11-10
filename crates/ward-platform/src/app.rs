//! Calls on the side of the setgid app (D47, D57; Phase 29a). The startup order lives in
//! `app/core` (its startup module); only the system operations stay here, safe to call:
//! - [`release_group`]: `setresgid` to the real gid, verified (GTK 3 refuses to start
//!   with setgid, and the group is only good for the `connect`);
//! - [`Parent`]: the parent at the start, with the pidfd open, to know later whether it
//!   is still the parent (the re-exec of D47 only holds with `PPid` equal to the daemon);
//! - [`has_cloexec`] and [`wait_for_close`] of the socket;
//! - [`reexec`]: `execve` of `/proc/self/exe` with the verified environment and every
//!   fd ≥ 3 closed, for the kernel to apply the setgid again.

use std::ffi::OsString;
use std::fmt;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::Command;

use crate::launcher::AppEnvironment;
use crate::sys;

/// Real, effective and saved gid.
pub fn gids() -> io::Result<(u32, u32, u32)> {
    sys::getresgid()
}

/// Releases the group of the setgid binary: the three gids to the real one. Without
/// setgid, changes nothing.
pub fn release_group() -> io::Result<()> {
    sys::release_effective_group()
}

/// `true` when the process is dumpable (`PR_GET_DUMPABLE` ≠ 0).
pub fn dumpable() -> io::Result<bool> {
    sys::dumpable()
}

/// The value of `PR_GET_DUMPABLE`: 0, 1 or 2 (2 after the setgid `exec` with
/// `fs.suid_dumpable=2`).
pub fn dumpable_value() -> io::Result<i32> {
    sys::dumpable_value()
}

/// The fd has `FD_CLOEXEC` (the app's socket cannot go to the WebKitGTK processes).
pub fn has_cloexec(fd: &impl AsFd) -> io::Result<bool> {
    sys::has_cloexec(fd.as_fd())
}

/// Blocks until the other side of the socket closes, without reading anything.
pub fn wait_for_close(fd: &impl AsFd) -> io::Result<()> {
    sys::wait_for_close(fd.as_fd())
}

/// Why the parent does not look like the daemon. No content from `/proc`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParentMismatch {
    /// `PPid` 0 or 1 (orphan, or the parent exited).
    Orphan,
    /// The parent process is not a `wardd` of the same uid.
    NotDaemon,
    /// The parent is dumpable: `wardd` never is (S8).
    DumpableParent,
    /// The parent exited, or the pid already belongs to another.
    Exited,
    /// Could not read the parent's `/proc`.
    Read,
}

impl fmt::Display for ParentMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ParentMismatch::Orphan => "o app não tem pai (o daemon saiu)",
            ParentMismatch::NotDaemon => "o app não foi aberto pelo daemon do Ward",
            ParentMismatch::DumpableParent => "o processo pai não é o daemon endurecido do Ward",
            ParentMismatch::Exited => "o processo pai saiu",
            ParentMismatch::Read => "não deu para conferir o processo pai",
        })
    }
}

impl std::error::Error for ParentMismatch {}

/// Name of the daemon in `status` (`comm`).
const DAEMON_NAME: &[u8] = b"Name:\twardd";

/// The parent of this process at the time of [`Parent::current`], with its pidfd.
#[derive(Debug)]
pub struct Parent {
    pid: u32,
    pidfd: OwnedFd,
}

impl Parent {
    /// The parent now (`getppid`), with the pidfd open and `PPid` read again afterwards.
    pub fn current() -> Result<Parent, ParentMismatch> {
        let pid = sys::getppid();
        if pid <= 1 {
            return Err(ParentMismatch::Orphan);
        }
        let p = libc::pid_t::try_from(pid).map_err(|_| ParentMismatch::Read)?;
        let pidfd = sys::pidfd_open(p).map_err(|_| ParentMismatch::Exited)?;
        if sys::getppid() != pid {
            return Err(ParentMismatch::Exited);
        }
        Ok(Parent { pid, pidfd })
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// The same process is still alive and is still the parent of this one.
    pub fn still_parent(&self) -> bool {
        matches!(sys::pidfd_alive(self.pidfd.as_fd()), Ok(true)) && sys::getppid() == self.pid
    }

    /// On the app's side, only so as not to go on when the parent is clearly not the
    /// daemon: the parent has `Name: wardd`, the four uids equal to this process's and is
    /// non-dumpable (its `environ` gives `EACCES`; call it after [`release_group`],
    /// otherwise the different group would already give `EACCES`). It is not proof: the
    /// daemon decides the role, by the pidfd of `SO_PEERPIDFD` (D47, D57).
    pub fn verify_looks_like_daemon(&self) -> Result<(), ParentMismatch> {
        let status = std::fs::read(format!("/proc/{}/status", self.pid))
            .map_err(|_| ParentMismatch::Exited)?;
        let uid = sys::geteuid();
        let name_ok = status.split(|&b| b == b'\n').any(|l| l == DAEMON_NAME);
        let uids_ok = status
            .split(|&b| b == b'\n')
            .find_map(|l| l.strip_prefix(b"Uid:"))
            .and_then(|l| std::str::from_utf8(l).ok())
            .is_some_and(|l| {
                let v: Vec<&str> = l.split_whitespace().collect();
                v.len() == 4 && v.iter().all(|u| u.parse::<u32>().ok() == Some(uid))
            });
        if !name_ok || !uids_ok {
            return Err(ParentMismatch::NotDaemon);
        }
        match std::fs::File::open(format!("/proc/{}/environ", self.pid)) {
            Err(e) if e.raw_os_error() == Some(libc::EACCES) => {}
            Ok(_) => return Err(ParentMismatch::DumpableParent),
            Err(_) => return Err(ParentMismatch::Read),
        }
        if !self.still_parent() {
            return Err(ParentMismatch::Exited);
        }
        Ok(())
    }
}

/// `execve("/proc/self/exe")` with `arguments` (without `argv[0]`), only `environment`
/// and every fd ≥ 3 closed. The kernel applies the setgid of the file again. Only
/// returns on error.
pub fn reexec(arguments: &[OsString], environment: &AppEnvironment) -> io::Error {
    let mut cmd = Command::new("/proc/self/exe");
    cmd.args(arguments)
        .env_clear()
        .envs(environment.variables())
        .current_dir("/");
    sys::close_fds_on_exec(&mut cmd);
    cmd.exec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream;

    #[test]
    fn without_setgid_release_group_changes_nothing() {
        let before = gids().unwrap();
        release_group().unwrap();
        let (r, e, s) = gids().unwrap();
        assert_eq!((r, e, s), (before.0, before.0, before.0));
    }

    #[test]
    fn std_socket_has_cloexec_and_the_close_is_seen() {
        let (a, b) = UnixStream::pair().unwrap();
        assert!(has_cloexec(&a).unwrap());
        let t = std::thread::spawn(move || wait_for_close(&a));
        std::thread::sleep(std::time::Duration::from_millis(50));
        drop(b);
        assert!(t.join().unwrap().is_ok());
    }

    #[test]
    fn attack_parent_that_is_not_the_daemon_is_refused() {
        // The parent of the test is cargo (or the shell): neither `wardd` nor hardened.
        let parent = Parent::current().unwrap();
        assert!(parent.still_parent());
        assert_eq!(
            parent.verify_looks_like_daemon(),
            Err(ParentMismatch::NotDaemon)
        );
        assert!(!ParentMismatch::NotDaemon.to_string().contains('/'));
    }
}
