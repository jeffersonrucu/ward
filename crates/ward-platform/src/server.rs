//! Verification of the server by the client (S5, D46): the CLI and the app only send any
//! byte, and in particular the master password, after verifying that the other side of
//! the socket is the socket of the system systemd:
//! - every directory of the path (`/run/ward/<uid>/` and above) belongs to root, with
//!   no write for group and others, with no symbolic link;
//! - the socket belongs to the uid itself and is 0600;
//! - the `SO_PEERCRED` of the server has uid 0. Seen by the client, it is the credential
//!   of whoever did the `listen`, which was the system systemd; no process of the user
//!   creates a listening socket with uid 0;
//! - the address of the peer is the expected path.
//!
//! The test daemon (D7) listens in a 0700 directory of the user, with the user's own
//! uid. That form ([`ExpectedServer::test`]) only exists with the `test-mode` feature:
//! without it, the binary has no path at all that accepts a server that is not root's.

use std::fmt;
use std::io;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use crate::socket::{PathRefusal, system_socket_path, verify_system_path};
use crate::sys;

/// Which server the client accepts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpectedServer {
    path: PathBuf,
    kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// Socket of the system systemd: root directory and peer with uid 0.
    System,
    /// Test daemon: 0700 directory of the user and peer of the same uid.
    #[cfg(feature = "test-mode")]
    Test,
}

impl ExpectedServer {
    /// The installed `wardd`: `/run/ward/<euid>/wardd.sock`.
    pub fn system() -> ExpectedServer {
        ExpectedServer::system_at(system_socket_path())
    }

    /// Same rules as [`system`](Self::system), at another path (tests, container). The
    /// checks do not change: root directory and peer with uid 0.
    pub fn system_at(path: PathBuf) -> ExpectedServer {
        ExpectedServer {
            path,
            kind: Kind::System,
        }
    }

    /// Test daemon (D7): [`crate::socket::TEST_DIR_NAME`] under the runtime directory.
    #[cfg(feature = "test-mode")]
    pub fn test() -> ExpectedServer {
        ExpectedServer::test_at(
            crate::socket::runtime_dir()
                .join(crate::socket::TEST_DIR_NAME)
                .join(crate::socket::SOCKET_NAME),
        )
    }

    /// Test daemon at another path.
    #[cfg(feature = "test-mode")]
    pub fn test_at(path: PathBuf) -> ExpectedServer {
        ExpectedServer {
            path,
            kind: Kind::Test,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn verify_path(&self) -> Result<(), ServerRefusal> {
        if !crate::process::simple_absolute_path(&self.path) {
            return Err(ServerRefusal::InvalidPath);
        }
        let r = match self.kind {
            Kind::System => verify_system_path(&self.path),
            #[cfg(feature = "test-mode")]
            Kind::Test => crate::socket::verify_test_path(&self.path),
        };
        r.map_err(|e| match e {
            PathRefusal::Missing => ServerRefusal::Missing,
            PathRefusal::Dir(_) => ServerRefusal::UntrustedDir,
            PathRefusal::Socket => ServerRefusal::UntrustedSocket,
        })
    }

    fn peer_uid_accepted(&self, uid: u32) -> bool {
        match self.kind {
            Kind::System => uid == 0,
            #[cfg(feature = "test-mode")]
            Kind::Test => uid == sys::geteuid(),
        }
    }
}

/// Why the client does not talk to the server. No path nor content: it can go to the
/// log and to the message to the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerRefusal {
    /// There is no socket (or nobody listening): daemon unavailable.
    Missing,
    /// Expected path not absolute or with `.`/`..`.
    InvalidPath,
    /// A directory of the path is not root's (or is not the test directory), accepts
    /// group/other writes or is a link.
    UntrustedDir,
    /// The socket file is not a socket, not of the uid itself or not 0600.
    UntrustedSocket,
    /// The peer of the socket does not have the expected uid (0 for the system).
    UntrustedPeer { uid: u32 },
    /// The peer is not listening on the expected path.
    AddressMismatch,
    /// Another error connecting or reading the credentials.
    Io(io::ErrorKind),
}

impl fmt::Display for ServerRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerRefusal::Missing => f.write_str("o daemon não está rodando"),
            ServerRefusal::InvalidPath => f.write_str("caminho do socket inválido"),
            ServerRefusal::UntrustedDir => {
                f.write_str("a pasta do socket não é do sistema; o servidor não é confiável")
            }
            ServerRefusal::UntrustedSocket => {
                f.write_str("o socket não é 0600 deste usuário; o servidor não é confiável")
            }
            ServerRefusal::UntrustedPeer { uid } => write!(
                f,
                "o socket foi criado pelo uid {uid}, não pelo sistema; o servidor não é confiável"
            ),
            ServerRefusal::AddressMismatch => {
                f.write_str("o servidor não escuta no caminho esperado")
            }
            ServerRefusal::Io(k) => write!(f, "erro ao falar com o daemon: {k}"),
        }
    }
}

impl std::error::Error for ServerRefusal {}

/// Verifies the path, connects and verifies the peer; only after that does the socket go
/// back to the caller. No byte is written here.
pub fn connect_server(expected: &ExpectedServer) -> Result<UnixStream, ServerRefusal> {
    expected.verify_path()?;
    let stream = UnixStream::connect(&expected.path).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused => ServerRefusal::Missing,
        k => ServerRefusal::Io(k),
    })?;
    verify_server(&stream, expected)?;
    Ok(stream)
}

/// Verifies an already connected socket: path (directories and socket), address of the
/// peer and uid of the peer (`SO_PEERCRED`). Error: the caller closes the socket without
/// writing anything.
pub fn verify_server(stream: &UnixStream, expected: &ExpectedServer) -> Result<(), ServerRefusal> {
    expected.verify_path()?;
    let address = stream
        .peer_addr()
        .map_err(|e| ServerRefusal::Io(e.kind()))?;
    if address.as_pathname() != Some(expected.path.as_path()) {
        return Err(ServerRefusal::AddressMismatch);
    }
    let cred = sys::peercred(stream.as_fd()).map_err(|e| ServerRefusal::Io(e.kind()))?;
    if !expected.peer_uid_accepted(cred.uid) {
        return Err(ServerRefusal::UntrustedPeer { uid: cred.uid });
    }
    // The path again: the whole verification holds for the same state of the disk.
    expected.verify_path()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_only_accepts_uid_0() {
        let s = ExpectedServer::system();
        assert!(s.peer_uid_accepted(0));
        assert!(!s.peer_uid_accepted(sys::geteuid().max(1)));
        assert!(!s.peer_uid_accepted(1000));
        assert!(!s.peer_uid_accepted(u32::MAX));
        assert_eq!(
            s.path(),
            Path::new(&format!("/run/ward/{}/wardd.sock", sys::geteuid()))
        );
    }

    #[cfg(feature = "test-mode")]
    #[test]
    fn test_accepts_its_own_uid_and_not_root() {
        let t = ExpectedServer::test();
        assert!(t.peer_uid_accepted(sys::geteuid()));
        if sys::geteuid() != 0 {
            assert!(!t.peer_uid_accepted(0));
        }
        assert_eq!(
            t.path(),
            Path::new(&format!(
                "/run/user/{}/ward-test/wardd.sock",
                sys::geteuid()
            ))
        );
    }

    #[test]
    fn invalid_path_refuses() {
        for bad in ["", "relativo", "/run/ward/../x/wardd.sock", "/run//ward"] {
            assert!(
                matches!(
                    connect_server(&ExpectedServer::system_at(PathBuf::from(bad))),
                    Err(ServerRefusal::InvalidPath)
                ),
                "{bad}"
            );
        }
    }

    #[test]
    fn messages_without_path() {
        for r in [
            ServerRefusal::Missing,
            ServerRefusal::UntrustedDir,
            ServerRefusal::UntrustedSocket,
            ServerRefusal::UntrustedPeer { uid: 1000 },
            ServerRefusal::AddressMismatch,
        ] {
            assert!(!r.to_string().contains('/'), "{r}");
        }
    }
}
