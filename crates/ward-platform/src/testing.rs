//! Only in `test-mode`: support for the D46 tests on the host and in the container.

use std::os::fd::AsFd;
use std::os::unix::net::UnixListener;
use std::os::unix::process::CommandExt;
use std::process::Command;

use crate::sys;

/// Plays the role of systemd: puts `listener` on fd 3 without `O_CLOEXEC` and executes
/// `cmd` in the same pid, with `LISTEN_PID` = this pid and `LISTEN_FDS=1`. Only returns
/// if the `exec` fails, and then the process exits with 127 without using fd 3 again.
pub fn run_with_socket(listener: &UnixListener, mut cmd: Command) -> ! {
    cmd.env("LISTEN_PID", std::process::id().to_string())
        .env("LISTEN_FDS", "1")
        .env_remove("LISTEN_FDNAMES");
    // SAFETY: the contract of `put_on_activation_fd` requires `exec` or exit right after
    // swapping fd 3: the `exec` comes on the next line and, if it fails, the process
    // exits without using fd 3.
    #[allow(unsafe_code)]
    let r = unsafe { sys::put_on_activation_fd(listener.as_fd()) };
    let error = match r {
        Ok(()) => cmd.exec(),
        Err(e) => e,
    };
    eprintln!("run_with_socket: {error}");
    std::process::exit(127)
}
