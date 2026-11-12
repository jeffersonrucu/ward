//! `ward-nm`: native messaging host of the extension (Phase 40). See the crate
//! documentation (`lib.rs`).
//!
//! Usage (by Chrome): `ward-nm chrome-extension://<id>/`. Exits 0 when Chrome closes
//! stdin; ≠ 0 with an argument out of form, a bad message or a failure on stdout.

#![forbid(unsafe_code)]

use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::UnixStream;
use std::process::ExitCode;
use std::time::Duration;

use zeroize::Zeroizing;

use ward_core::limits::MAX_COMMON_FRAME;
use ward_core::protocol::frame::{self, RESPONSE_LIMIT};
use ward_nm::{Daemon, DaemonFailure};
use ward_platform::{ExpectedServer, connect_server};

/// Time limit for the daemon to answer (the same as the CLI's).
const READ_TIMEOUT: Duration = Duration::from_secs(130);
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// The server this host accepts. Release: only the installed `wardd` (D46), with no
/// variable nor argument that changes it. `test-mode`: the test daemon.
fn expected_server() -> ExpectedServer {
    #[cfg(feature = "test-mode")]
    {
        if let Some(d) = std::env::var_os("WARD_DEV_SOCKET_DIR") {
            let d = std::path::PathBuf::from(d);
            return ExpectedServer::test_at(d.join(ward_platform::socket::SOCKET_NAME));
        }
        ExpectedServer::test()
    }
    #[cfg(not(feature = "test-mode"))]
    {
        ExpectedServer::system()
    }
}

/// A new connection per message, checked before the first byte (D46). The host does not
/// keep an idle connection on the daemon.
struct DaemonSocket {
    expected: ExpectedServer,
}

impl DaemonSocket {
    fn connect(&self) -> Result<UnixStream, DaemonFailure> {
        let s = connect_server(&self.expected).map_err(|_| DaemonFailure::Unavailable)?;
        s.set_read_timeout(Some(READ_TIMEOUT))
            .and_then(|()| s.set_write_timeout(Some(WRITE_TIMEOUT)))
            .map_err(|_| DaemonFailure::Conversation)?;
        verify_extra(&s)?;
        Ok(s)
    }
}

/// `test-mode`: without uid 0 (the test daemon belongs to the user), the socket's
/// process must be non-dumpable, like `wardd` (the same rule as the test CLI).
#[cfg(feature = "test-mode")]
fn verify_extra(s: &UnixStream) -> Result<(), DaemonFailure> {
    let peer =
        ward_platform::PeerProcess::from_socket(s).map_err(|_| DaemonFailure::Unavailable)?;
    match std::fs::read_link(format!("/proc/{}/exe", peer.credentials().pid)) {
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {}
        _ => return Err(DaemonFailure::Unavailable),
    }
    peer.reverify_no_debugger()
        .map_err(|_| DaemonFailure::Unavailable)
}

#[cfg(not(feature = "test-mode"))]
fn verify_extra(_: &UnixStream) -> Result<(), DaemonFailure> {
    Ok(())
}

impl Daemon for DaemonSocket {
    fn exchange(&mut self, request: &[u8]) -> Result<Zeroizing<Vec<u8>>, DaemonFailure> {
        let mut s = self.connect()?;
        frame::write_frame(&mut s, request, MAX_COMMON_FRAME)
            .map_err(|_| DaemonFailure::Conversation)?;
        let r =
            frame::read_frame(&mut s, RESPONSE_LIMIT).map_err(|_| DaemonFailure::Conversation)?;
        let _ = s.shutdown(std::net::Shutdown::Both);
        Ok(r)
    }
}

fn main() -> ExitCode {
    // S8: no core and no ptrace by the same uid (the setgid exec already left the
    // process non-dumpable; here it becomes 0 in the build without setgid too).
    if ward_platform::harden_process().is_err() {
        eprintln!("ward-nm: não deu para endurecer o processo");
        return ExitCode::FAILURE;
    }
    // Chrome passes only the extension's origin.
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match args.as_slice() {
        [origin] if ward_nm::valid_extension_origin(origin.as_bytes()) => {}
        _ => {
            eprintln!("ward-nm: uso: ward-nm chrome-extension://<id>/ (aberto pelo Chrome)");
            return ExitCode::from(2);
        }
    }
    let mut daemon = DaemonSocket {
        expected: expected_server(),
    };
    let mut log = |m: &str| eprintln!("ward-nm: {m}");
    let r = ward_nm::serve(
        &mut io::stdin().lock(),
        &mut io::stdout().lock(),
        &mut daemon,
        &mut log,
    );
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
