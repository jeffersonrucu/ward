//! Local socket of the daemon (S5, V3.1, V3.2).
//!
//! - [`prepare_dir`]: the socket directory is 0700 and the user's. If it already exists
//!   with another mode, another owner or as a symbolic link, it refuses (the daemon does
//!   not start); it never "fixes" what it found. Each directory above belongs to root or
//!   to the user, with no write for group and others, with no symbolic link.
//! - [`SocketDir::listen`]: creates the 0600 socket inside it. A stale socket (nobody
//!   listening) is replaced; with another daemon listening, it refuses.
//! - [`listen_from_listen_fds`]: the installed daemon does not create the socket. The
//!   system systemd (`wardd@<uid>.socket`) creates `/run/ward/<uid>/wardd.sock` in a
//!   root directory and hands the fd over by `LISTEN_FDS` (D46); the daemon verifies the
//!   fd is a listening Unix socket at the expected path, in a root-only directory, with
//!   its socket 0600.
//! - [`Listener::accept`] / [`verify_connection`]: closes, without answering, the
//!   connection of a uid other than the owner, and returns the reason for the log. The
//!   pid and pidfd of the peer go along in the [`Connection`] for the identification of
//!   D5 ([`crate::process`]).

use std::fmt;
use std::fs::File;
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use crate::process::{PeerProcess, simple_absolute_path};
use crate::sys;

/// Ward's directory inside `/run/user/<uid>/`, with the socket and temporary files (S2).
pub const DIR_NAME: &str = "ward";
/// Directory of the test daemon (D7); never the same as the real daemon's.
pub const TEST_DIR_NAME: &str = "ward-teste";
/// Name of the socket inside the directory.
pub const SOCKET_NAME: &str = "wardd.sock";
/// Root directory where the system systemd creates each user's directory (D46).
pub const SYSTEM_DIR: &str = "/run/ward";
/// Longest socket name accepted.
const MAX_SOCKET_NAME: usize = 64;

/// `/run/user/<euid>`, by the effective uid and not by `XDG_RUNTIME_DIR` (which comes
/// from the environment).
pub fn runtime_dir() -> PathBuf {
    PathBuf::from(format!("/run/user/{}", sys::geteuid()))
}

/// `/run/ward/<euid>/wardd.sock`: the socket of the installed daemon (D46), by the
/// effective uid.
pub fn system_socket_path() -> PathBuf {
    PathBuf::from(format!("{SYSTEM_DIR}/{}/{SOCKET_NAME}", sys::geteuid()))
}

/// Problem with the socket directory. No secret data; it can go to the log.
#[derive(Debug)]
pub enum DirError {
    /// Path not absolute, with `.`/`..` or with no directory above.
    InvalidPath,
    /// A directory above belongs neither to root nor to the user, accepts group/other
    /// writes or is a symbolic link.
    UntrustedParent(PathBuf),
    /// Exists and is not a directory (includes a symbolic link).
    NotDirectory,
    /// Exists and belongs to another owner.
    WrongOwner {
        uid: u32,
    },
    /// Exists with a mode other than 0700.
    WrongMode {
        mode: u32,
    },
    /// The directory changed between the check and the use.
    Swapped,
    Io(io::Error),
}

impl fmt::Display for DirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DirError::InvalidPath => f.write_str("caminho do diretório do socket inválido"),
            DirError::UntrustedParent(p) => write!(
                f,
                "pasta acima do socket não confiável (dono ou permissão): {}",
                p.display()
            ),
            DirError::NotDirectory => f.write_str("o caminho do socket existe e não é diretório"),
            DirError::WrongOwner { uid } => {
                write!(f, "o diretório do socket é do uid {uid}, não deste usuário")
            }
            DirError::WrongMode { mode } => {
                write!(f, "o diretório do socket tem permissão {mode:o}, não 700")
            }
            DirError::Swapped => f.write_str("o diretório do socket mudou durante o uso"),
            DirError::Io(e) => write!(f, "erro no diretório do socket: {e}"),
        }
    }
}

impl std::error::Error for DirError {}

impl From<io::Error> for DirError {
    fn from(e: io::Error) -> Self {
        DirError::Io(e)
    }
}

/// Verified socket directory (0700, the user's), kept open.
#[derive(Debug)]
pub struct SocketDir {
    path: PathBuf,
    opened: File,
}

/// Prepares the socket directory: creates it with 0700 if it does not exist; if it
/// exists, only accepts a directory of the user with exactly 0700.
pub fn prepare_dir(path: &Path) -> Result<SocketDir, DirError> {
    if !simple_absolute_path(path) {
        return Err(DirError::InvalidPath);
    }
    let euid = sys::geteuid();
    let parent = path.parent().ok_or(DirError::InvalidPath)?;
    verify_dirs_above(parent, euid)?;

    match std::fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {
            // The umask only removes bits; at most 0700 is left. Set it to exactly 0700
            // on a directory we have just created (and is ours).
            let meta = std::fs::symlink_metadata(path)?;
            if meta.file_type().is_dir() && meta.uid() == euid && meta.mode() & 0o7777 != 0o700 {
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
            }
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.into()),
    }

    let opened = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| match e.raw_os_error() {
            Some(libc::ELOOP) | Some(libc::ENOTDIR) => DirError::NotDirectory,
            _ => DirError::Io(e),
        })?;
    let dir = SocketDir {
        path: path.to_path_buf(),
        opened,
    };
    dir.verify()?;
    Ok(dir)
}

/// Each directory from `dir` up to `/`: a directory (not a link), owned by root or by
/// `euid`, with no write for group and others.
fn verify_dirs_above(dir: &Path, euid: u32) -> Result<(), DirError> {
    let mut current = Some(dir);
    while let Some(p) = current {
        let m =
            std::fs::symlink_metadata(p).map_err(|_| DirError::UntrustedParent(p.to_path_buf()))?;
        let owner_ok = m.uid() == 0 || m.uid() == euid;
        if !m.file_type().is_dir() || !owner_ok || m.mode() & 0o022 != 0 {
            return Err(DirError::UntrustedParent(p.to_path_buf()));
        }
        current = p.parent();
    }
    Ok(())
}

impl SocketDir {
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Verifies the open directory and that the path still leads to it.
    fn verify(&self) -> Result<(), DirError> {
        let opened = self.opened.metadata()?;
        if !opened.file_type().is_dir() {
            return Err(DirError::NotDirectory);
        }
        if opened.uid() != sys::geteuid() {
            return Err(DirError::WrongOwner { uid: opened.uid() });
        }
        let mode = opened.mode() & 0o7777;
        if mode != 0o700 {
            return Err(DirError::WrongMode { mode });
        }
        let by_name = std::fs::symlink_metadata(&self.path)?;
        if !by_name.file_type().is_dir()
            || (by_name.dev(), by_name.ino()) != (opened.dev(), opened.ino())
        {
            return Err(DirError::Swapped);
        }
        Ok(())
    }

    /// Creates the socket `name` (0600) and listens on it.
    pub fn listen(&self, name: &str) -> Result<Listener, ListenError> {
        if name.is_empty()
            || name.len() > MAX_SOCKET_NAME
            || name == "."
            || name == ".."
            || name.contains(['/', '\0'])
        {
            return Err(ListenError::InvalidName);
        }
        self.verify()?;
        let path = self.path.join(name);
        let euid = sys::geteuid();

        match std::fs::symlink_metadata(&path) {
            Ok(m) => {
                if !m.file_type().is_socket() || m.uid() != euid {
                    return Err(ListenError::PathTaken);
                }
                match UnixStream::connect(&path) {
                    Ok(_) => return Err(ListenError::AlreadyInUse),
                    Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => {
                        std::fs::remove_file(&path)?;
                    }
                    Err(e) => return Err(e.into()),
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }

        let listener = UnixListener::bind(&path)?;
        // The 0700 directory already keeps the others out; the 0600 is the second layer
        // (V3.1).
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        let m = std::fs::symlink_metadata(&path)?;
        if !m.file_type().is_socket() || m.uid() != euid || m.mode() & 0o7777 != 0o600 {
            let _ = std::fs::remove_file(&path);
            return Err(ListenError::PathTaken);
        }
        self.verify()?;
        Ok(Listener {
            listener,
            path,
            id: Some((m.dev(), m.ino())),
        })
    }
}

/// Problem creating the socket.
#[derive(Debug)]
pub enum ListenError {
    InvalidName,
    /// Another daemon already listens on the socket.
    AlreadyInUse,
    /// The path exists and is not a socket of this user.
    PathTaken,
    Dir(DirError),
    Io(io::Error),
}

impl fmt::Display for ListenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ListenError::InvalidName => f.write_str("nome de socket inválido"),
            ListenError::AlreadyInUse => f.write_str("já há um daemon escutando no socket"),
            ListenError::PathTaken => {
                f.write_str("o caminho do socket existe e não é um socket deste usuário")
            }
            ListenError::Dir(e) => e.fmt(f),
            ListenError::Io(e) => write!(f, "erro ao criar o socket: {e}"),
        }
    }
}

impl std::error::Error for ListenError {}

impl From<io::Error> for ListenError {
    fn from(e: io::Error) -> Self {
        ListenError::Io(e)
    }
}

impl From<DirError> for ListenError {
    fn from(e: DirError) -> Self {
        ListenError::Dir(e)
    }
}

/// Listening socket. On drop, deletes the socket file if it is still the same.
#[derive(Debug)]
pub struct Listener {
    listener: UnixListener,
    path: PathBuf,
    /// dev/inode of the socket this process created and deletes on drop. `None` for the
    /// systemd socket, which is not ours to delete.
    id: Option<(u64, u64)>,
}

impl Listener {
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The `UnixListener` (for instance, for an async loop). Each connection it accepts
    /// goes through [`verify_connection`] before any read.
    pub fn listener(&self) -> &UnixListener {
        &self.listener
    }

    /// Accepts a connection and verifies the uid of the peer. Different uid: the
    /// connection has already been closed without an answer when the error comes back.
    pub fn accept(&self) -> Result<Connection, AcceptError> {
        let (stream, _) = self.listener.accept().map_err(AcceptError::Io)?;
        let peer = verify_connection(&stream)?;
        Ok(Connection { stream, peer })
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        if let Some(id) = self.id
            && let Ok(m) = std::fs::symlink_metadata(&self.path)
            && m.file_type().is_socket()
            && (m.dev(), m.ino()) == id
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Problem with the socket received from systemd (D46). No environment value: it can go
/// to the log.
#[derive(Debug)]
pub enum ActivationError {
    /// No `LISTEN_PID`/`LISTEN_FDS` (the daemon was not activated by systemd).
    NotActivated,
    /// `LISTEN_PID` is not this process, or not a number.
    PidMismatch,
    /// `LISTEN_FDS` other than 1.
    FdCount,
    /// fd 3 was already used in this process, or is not open.
    FdUnavailable(io::Error),
    /// The fd is not a listening Unix stream socket.
    NotUnixListener,
    /// The socket listens on another path (or on an abstract name).
    PathMismatch,
    /// A directory of the path is not root's, accepts group/other writes or is a link.
    UntrustedDir(PathBuf),
    /// The socket file is not a socket, not this user's or not 0600.
    UntrustedSocket,
    Io(io::Error),
}

impl fmt::Display for ActivationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActivationError::NotActivated => {
                f.write_str("sem socket do systemd (LISTEN_PID/LISTEN_FDS ausentes)")
            }
            ActivationError::PidMismatch => f.write_str("LISTEN_PID não é este processo"),
            ActivationError::FdCount => f.write_str("LISTEN_FDS tem de ser 1"),
            ActivationError::FdUnavailable(e) => {
                write!(f, "fd 3 do systemd indisponível: {e}")
            }
            ActivationError::NotUnixListener => {
                f.write_str("o fd do systemd não é um socket Unix em escuta")
            }
            ActivationError::PathMismatch => {
                f.write_str("o socket do systemd não está no caminho esperado")
            }
            ActivationError::UntrustedDir(p) => write!(
                f,
                "pasta do socket não é só do root (dono, permissão ou link): {}",
                p.display()
            ),
            ActivationError::UntrustedSocket => {
                f.write_str("o arquivo do socket não é um socket 0600 deste usuário")
            }
            ActivationError::Io(e) => write!(f, "erro no socket do systemd: {e}"),
        }
    }
}

impl std::error::Error for ActivationError {}

/// Plain decimal number (digits only, no sign nor space).
fn decimal(value: &std::ffi::OsStr) -> Option<u32> {
    let b = value.as_encoded_bytes();
    if b.is_empty() || b.len() > 10 || !b.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(b).ok()?.parse().ok()
}

/// Receives the socket the system systemd created and passed on fd 3 (D46) and
/// verifies:
/// - `LISTEN_PID` is this process and `LISTEN_FDS` is 1;
/// - the fd is a Unix stream socket, listening (`SO_ACCEPTCONN`), with exactly the name
///   `expected_path` (`/run/ward/<uid>/wardd.sock`, see [`system_socket_path`]);
/// - every directory of the path belongs to root, with no write for group and others,
///   with no link;
/// - the socket file is a socket, of the effective uid and 0600.
///
/// The fd gets `O_CLOEXEC` (no child of `ward run` inherits the listening socket) and
/// the original fd 3 is closed. Call it at the start of `main`, once. The returned
/// [`Listener`] does not delete the socket on drop (it is systemd's). The `LISTEN_*`
/// variables stay in the environment: the daemon builds the environment of each child
/// without them.
pub fn listen_from_listen_fds(expected_path: &Path) -> Result<Listener, ActivationError> {
    if !simple_absolute_path(expected_path) {
        return Err(ActivationError::PathMismatch);
    }
    let pid = std::env::var_os("LISTEN_PID").ok_or(ActivationError::NotActivated)?;
    let fds = std::env::var_os("LISTEN_FDS").ok_or(ActivationError::NotActivated)?;
    if decimal(&pid) != Some(std::process::id()) {
        return Err(ActivationError::PidMismatch);
    }
    if decimal(&fds) != Some(1) {
        return Err(ActivationError::FdCount);
    }
    let copy = sys::dup_activation_fd().map_err(ActivationError::FdUnavailable)?;
    verify_listening_fd(copy.as_fd())?;
    let listener = UnixListener::from(copy);
    let name = listener.local_addr().map_err(ActivationError::Io)?;
    if name.as_pathname() != Some(expected_path) {
        return Err(ActivationError::PathMismatch);
    }
    verify_system_path(expected_path).map_err(|e| match e {
        PathRefusal::Dir(p) => ActivationError::UntrustedDir(p),
        PathRefusal::Socket | PathRefusal::Missing => ActivationError::UntrustedSocket,
    })?;
    // The copy is the same socket as fd 3, so the original can close.
    sys::close_activation_fd();
    Ok(Listener {
        listener,
        path: expected_path.to_path_buf(),
        id: None,
    })
}

/// Listening Unix stream socket.
fn verify_listening_fd(fd: BorrowedFd<'_>) -> Result<(), ActivationError> {
    let is_socket = fd
        .try_clone_to_owned()
        .map(File::from)
        .and_then(|f| f.metadata())
        .map(|m| m.file_type().is_socket())
        .map_err(ActivationError::Io)?;
    let domain =
        sys::sockopt_int(fd, libc::SO_DOMAIN).map_err(|_| ActivationError::NotUnixListener)?;
    let kind = sys::sockopt_int(fd, libc::SO_TYPE).map_err(|_| ActivationError::NotUnixListener)?;
    let listening =
        sys::sockopt_int(fd, libc::SO_ACCEPTCONN).map_err(|_| ActivationError::NotUnixListener)?;
    if !is_socket || domain != libc::AF_UNIX || kind != libc::SOCK_STREAM || listening != 1 {
        return Err(ActivationError::NotUnixListener);
    }
    Ok(())
}

/// Why the socket path is not trusted.
#[derive(Debug)]
pub(crate) enum PathRefusal {
    /// The socket (or a directory) does not exist.
    Missing,
    /// A directory that is not root's (or the user's, in test mode), with write for
    /// group and others, or a link.
    Dir(PathBuf),
    /// The file is not a socket, not of the effective uid or not 0600.
    Socket,
}

/// D46: every directory above the socket belongs to root, with no write for group and
/// others, with no link; the socket belongs to the effective uid and is 0600.
pub(crate) fn verify_system_path(socket: &Path) -> Result<(), PathRefusal> {
    let dir = socket.parent().ok_or(PathRefusal::Socket)?;
    verify_dirs(dir, |m| m.uid() == 0)?;
    verify_socket_file(socket)
}

/// Test daemon (D7): the socket directory belongs to the user and is 0700; the ones
/// above, to root or to the user, with no write for group and others; the socket
/// belongs to the user and is 0600.
#[cfg(feature = "test-mode")]
pub(crate) fn verify_test_path(socket: &Path) -> Result<(), PathRefusal> {
    let euid = sys::geteuid();
    let dir = socket.parent().ok_or(PathRefusal::Socket)?;
    let m = lstat(dir)?;
    if !m.file_type().is_dir() || m.uid() != euid || m.mode() & 0o7777 != 0o700 {
        return Err(PathRefusal::Dir(dir.to_path_buf()));
    }
    if let Some(above) = dir.parent() {
        verify_dirs(above, |m| m.uid() == 0 || m.uid() == euid)?;
    }
    verify_socket_file(socket)
}

fn lstat(p: &Path) -> Result<std::fs::Metadata, PathRefusal> {
    std::fs::symlink_metadata(p).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            PathRefusal::Missing
        } else {
            PathRefusal::Dir(p.to_path_buf())
        }
    })
}

/// Each directory from `dir` up to `/`: a directory (not a link), with an owner accepted
/// by `owner_ok`, with no write for group and others.
fn verify_dirs(
    dir: &Path,
    owner_ok: impl Fn(&std::fs::Metadata) -> bool,
) -> Result<(), PathRefusal> {
    if !simple_absolute_path(dir) && dir != Path::new("/") {
        return Err(PathRefusal::Dir(dir.to_path_buf()));
    }
    let mut current = Some(dir);
    while let Some(p) = current {
        let m = lstat(p)?;
        if !m.file_type().is_dir() || !owner_ok(&m) || m.mode() & 0o022 != 0 {
            return Err(PathRefusal::Dir(p.to_path_buf()));
        }
        current = p.parent();
    }
    Ok(())
}

fn verify_socket_file(socket: &Path) -> Result<(), PathRefusal> {
    let m = match std::fs::symlink_metadata(socket) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(PathRefusal::Missing),
        Err(_) => return Err(PathRefusal::Socket),
    };
    if !m.file_type().is_socket() || m.uid() != sys::geteuid() || m.mode() & 0o7777 != 0o600 {
        return Err(PathRefusal::Socket);
    }
    Ok(())
}

/// Accepted connection, with the process on the other side.
#[derive(Debug)]
pub struct Connection {
    pub stream: UnixStream,
    pub peer: PeerProcess,
}

/// Connection refused at `accept`. Only uid and pid (not secret) for the log.
#[derive(Debug)]
pub enum AcceptError {
    /// uid of the peer different from the owner of the daemon (V3.2).
    UidRefused {
        uid: u32,
        pid: i32,
    },
    /// No trusted `SO_PEERCRED`/`SO_PEERPIDFD` (pid in another namespace, old kernel).
    NoIdentity(io::Error),
    Io(io::Error),
}

impl fmt::Display for AcceptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AcceptError::UidRefused { uid, pid } => write!(
                f,
                "conexão recusada: uid {uid} (pid {pid}) diferente do dono do daemon"
            ),
            AcceptError::NoIdentity(e) => {
                write!(f, "conexão recusada: sem identidade do processo ({e})")
            }
            AcceptError::Io(e) => write!(f, "erro ao aceitar conexão: {e}"),
        }
    }
}

impl std::error::Error for AcceptError {}

/// Decides by the uid: only the owner of the daemon passes.
fn uid_accepted(peer_uid: u32, euid: u32) -> bool {
    peer_uid == euid
}

/// Verifies a freshly accepted connection (from any loop, synchronous or not): uid of
/// the peer equal to the daemon's and identity of the process by pidfd. On error, the
/// caller closes the connection without answering (the `stream` is dropped). The role
/// (D5) comes from [`PeerProcess::classify`].
pub fn verify_connection(stream: &impl AsFd) -> Result<PeerProcess, AcceptError> {
    let cred = sys::peercred(stream.as_fd()).map_err(AcceptError::NoIdentity)?;
    if !uid_accepted(cred.uid, sys::geteuid()) {
        return Err(AcceptError::UidRefused {
            uid: cred.uid,
            pid: cred.pid,
        });
    }
    let peer = PeerProcess::from_socket(stream).map_err(AcceptError::NoIdentity)?;
    // `SO_PEERCRED` is fixed at `connect`; the two reads have to match.
    if peer.credentials().uid != cred.uid {
        return Err(AcceptError::UidRefused {
            uid: peer.credentials().uid,
            pid: cred.pid,
        });
    }
    Ok(peer)
}

/// Did the client on the other side close (or fail)? Neither reads nor waits: the bytes
/// of a following request stay in the socket. For the daemon to cancel a long wait
/// (approval, unlock) when the CLI goes away (Phase 23).
pub fn peer_closed(stream: &impl AsFd) -> bool {
    sys::is_closed(stream.as_fd(), 0).unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_closed_without_consuming_bytes() {
        use std::io::{Read, Write};
        let (a, mut b) = UnixStream::pair().unwrap();
        assert!(!peer_closed(&a));
        b.write_all(b"x").unwrap();
        assert!(!peer_closed(&a));
        drop(b);
        assert!(peer_closed(&a));
        // The byte was not consumed.
        let mut buf = [0u8; 2];
        assert_eq!((&a).read(&mut buf).unwrap(), 1);
    }

    #[test]
    fn different_uid_refuses() {
        assert!(uid_accepted(1000, 1000));
        assert!(!uid_accepted(4242, 1000));
        assert!(!uid_accepted(0, 1000));
        assert!(!uid_accepted(u32::MAX, 1000));
    }

    #[test]
    fn invalid_dir_path_refuses() {
        for bad in ["", "relativo/ward", "/", "/run/user/../ward", "/run//ward"] {
            assert!(matches!(
                prepare_dir(Path::new(bad)),
                Err(DirError::InvalidPath)
            ));
        }
    }

    #[test]
    fn parent_writable_by_others_refuses() {
        // /tmp is 1777: anyone renames what is inside.
        let r = prepare_dir(Path::new("/tmp/ward-platform-falso-nao-criar"));
        assert!(matches!(r, Err(DirError::UntrustedParent(_))), "{r:?}");
        assert!(!Path::new("/tmp/ward-platform-falso-nao-criar").exists());
    }

    #[test]
    fn listen_pid_and_fds_plain_decimal_only() {
        use std::ffi::OsStr;
        assert_eq!(decimal(OsStr::new("1")), Some(1));
        assert_eq!(decimal(OsStr::new("4242")), Some(4242));
        for bad in ["", "+1", " 1", "1 ", "-1", "0x1", "1a", "99999999999"] {
            assert_eq!(decimal(OsStr::new(bad)), None, "{bad:?}");
        }
    }

    #[test]
    fn system_path_by_the_euid() {
        assert_eq!(
            system_socket_path(),
            PathBuf::from(format!("/run/ward/{}/wardd.sock", sys::geteuid()))
        );
    }

    #[test]
    fn attack_user_dir_does_not_pass_as_system_dir() {
        // The runtime directory is the user's: a socket there never passes as systemd's
        // (D46).
        let dir_path = runtime_dir().join(format!("ward-platform-u-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir_path);
        let dir = prepare_dir(&dir_path).unwrap();
        let listener = dir.listen(SOCKET_NAME).unwrap();
        assert!(matches!(
            verify_system_path(listener.path()),
            Err(PathRefusal::Dir(_))
        ));
        assert!(verify_listening_fd(listener.listener().as_fd()).is_ok());
        // A connected socket (not listening) and a regular file do not pass.
        let client = UnixStream::connect(listener.path()).unwrap();
        assert!(matches!(
            verify_listening_fd(client.as_fd()),
            Err(ActivationError::NotUnixListener)
        ));
        let file = File::open("/proc/self/status").unwrap();
        assert!(verify_listening_fd(file.as_fd()).is_err());
        drop(listener);
        let _ = std::fs::remove_dir_all(&dir_path);
    }

    #[test]
    fn root_dirs_are_accepted_and_a_link_is_not() {
        // `/usr/bin` is root's and 0755; `/bin` is a link to `usr/bin` on Ubuntu 24.04.
        assert!(verify_dirs(Path::new("/usr/bin"), |m| m.uid() == 0).is_ok());
        assert!(matches!(
            verify_dirs(Path::new("/bin"), |m| m.uid() == 0),
            Err(PathRefusal::Dir(_))
        ));
        assert!(matches!(
            verify_dirs(Path::new("/tmp"), |m| m.uid() == 0),
            Err(PathRefusal::Dir(_))
        ));
        assert!(matches!(
            verify_system_path(Path::new("/run/ward-nao-existe-falso/1/wardd.sock")),
            Err(PathRefusal::Missing)
        ));
    }

    #[test]
    fn runtime_by_the_euid() {
        assert_eq!(
            runtime_dir(),
            PathBuf::from(format!("/run/user/{}", sys::geteuid()))
        );
    }
}
