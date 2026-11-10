//! Channel of the "Connect" button token (Phase 27a; D3; high finding of the M3 audit).
//!
//! The single-use token the app gets from the daemon skips the level and the approval,
//! so no other process of the same uid may read it. It **never** goes in argv nor in an
//! environment variable: both stay in `/proc/<pid>/cmdline` and `/proc/<pid>/environ`,
//! readable by the same uid (and the `execve` puts the `dumpable` of the terminal and of
//! `ward` back to 1).
//!
//! The path is a pair of Unix sockets (`socketpair`):
//! - the app ([`ConnectChannel::new`]) writes the token on its end and closes it; the
//!   bytes stay in the read queue of the other end;
//! - only the other end goes through the `exec` of the terminal
//!   ([`ConnectChannel::prepare`]: every other fd ≥ 3 of the app closes on `exec`), and
//!   the terminal passes it on to `ward ssh`, which gets the number in
//!   `--token-fd <n>`;
//! - `ward ssh` ([`read_inherited_token`]) reads the token, closes the fd and only then
//!   connects.
//!
//! Why a socket and not a pipe, memfd or file: the same uid opens again a pipe, a memfd
//! or a file of another process through `/proc/<pid>/fd/<n>` (`PTRACE_MODE_READ` is
//! enough, and Yama does not restrict it). A socket does not reopen that way (`ENXIO`).
//! Taking the fd out of another process needs `pidfd_getfd`, which asks for
//! `PTRACE_MODE_ATTACH`: with `ptrace_scope` ≥ 1, which Ward requires (D47), only an
//! ancestor of the terminal could. That is why the read refuses an fd that is not a
//! Unix stream socket: a pipe passed in its place would be readable by others.

use std::fmt;
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, AsRawFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::process::Command;
use std::time::Duration;

use zeroize::Zeroizing;

use crate::sys;

/// Largest token accepted on the channel (the daemon's format: 16 to 64 bytes).
pub const MAX_TOKEN: usize = 64;

/// How long `ward ssh` waits for the token on the channel. The app writes and closes
/// its end before opening the terminal, so the token is already there; the deadline
/// only avoids hanging on a socket that never closes.
pub const READ_DEADLINE: Duration = Duration::from_secs(2);

/// Largest fd number accepted in `--token-fd`.
pub const MAX_FD_NUMBER: u32 = 1 << 20;

/// The end of the channel that goes to the terminal. The app keeps it alive until the
/// `spawn` returns and then lets it go (`drop`): the terminal keeps the only copy.
pub struct ConnectChannel {
    child: OwnedFd,
}

impl fmt::Debug for ConnectChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ConnectChannel(fd {})", self.child.as_raw_fd())
    }
}

impl ConnectChannel {
    /// Creates the pair, writes `token` on the app's end and closes it. Both ends are
    /// born with `FD_CLOEXEC`: no other process the app creates at the same time
    /// inherits them.
    pub fn new(token: &[u8]) -> io::Result<ConnectChannel> {
        if token.is_empty() || token.len() > MAX_TOKEN {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let (mut app, child) = UnixStream::pair()?;
        app.write_all(token)?;
        drop(app);
        Ok(ConnectChannel {
            child: child.into(),
        })
    }

    /// The number that goes in `--token-fd`.
    pub fn number(&self) -> RawFd {
        self.child.as_raw_fd()
    }

    /// Makes the `exec` of `cmd` carry only this fd besides 0–2 (every other fd ≥ 3
    /// closes). The channel has to stay alive until the `spawn` of `cmd` returns.
    pub fn prepare(&self, cmd: &mut Command) {
        sys::inherit_only_on_exec(cmd, self.child.as_raw_fd());
    }
}

/// Why the token was not read. Fixed texts only: never the content of the channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelError {
    /// Number outside 3..=[`MAX_FD_NUMBER`], or a closed fd.
    Number,
    /// The fd is not a Unix stream socket (pipe, file, memfd, listening socket…).
    NotSocket,
    /// The other side of the socket belongs to another user.
    OtherUser,
    /// The channel closed without any byte.
    Empty,
    /// More than [`MAX_TOKEN`] bytes.
    TooLarge,
    /// Nothing arrived within [`READ_DEADLINE`].
    Deadline,
    /// Another I/O error.
    Io(io::ErrorKind),
}

impl ChannelError {
    pub fn message(self) -> &'static str {
        match self {
            ChannelError::Number => "o canal do token do Conectar não está aberto",
            ChannelError::NotSocket => "o canal do token do Conectar não é o do app",
            ChannelError::OtherUser => "o canal do token do Conectar é de outro usuário",
            ChannelError::Empty => "o canal do token do Conectar veio vazio",
            ChannelError::TooLarge => "o canal do token do Conectar passa do tamanho do token",
            ChannelError::Deadline => "o token do Conectar não chegou a tempo",
            ChannelError::Io(_) => "erro ao ler o canal do token do Conectar",
        }
    }
}

impl fmt::Display for ChannelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for ChannelError {}

/// Reads the token from the fd `number` inherited from the terminal and closes it
/// (always, also on error). Verifies before reading: Unix stream socket, not listening,
/// with the other side of the same uid. Reads up to the end of the channel (the app
/// closes its end), at most [`MAX_TOKEN`] bytes, into a buffer that zeroes.
///
/// Called once, at the start of the process (`ward ssh`), with the number from argv.
pub fn read_inherited_token(number: u32) -> Result<Zeroizing<Vec<u8>>, ChannelError> {
    if !(3..=MAX_FD_NUMBER).contains(&number) {
        return Err(ChannelError::Number);
    }
    let n = RawFd::try_from(number).map_err(|_| ChannelError::Number)?;
    let fd = sys::take_inherited_socket(n).map_err(|e| match e.raw_os_error() {
        Some(libc::ENOTSOCK) => ChannelError::NotSocket,
        _ => ChannelError::Number,
    })?;
    verify_socket(&fd)?;
    let mut s = UnixStream::from(fd);
    s.set_read_timeout(Some(READ_DEADLINE))
        .map_err(|e| ChannelError::Io(e.kind()))?;
    let r = read_until_end(&mut s);
    drop(s);
    r
}

fn verify_socket(fd: &OwnedFd) -> Result<(), ChannelError> {
    let b = fd.as_fd();
    let domain = sys::sockopt_int(b, libc::SO_DOMAIN).map_err(|_| ChannelError::NotSocket)?;
    let kind = sys::sockopt_int(b, libc::SO_TYPE).map_err(|_| ChannelError::NotSocket)?;
    let listening =
        sys::sockopt_int(b, libc::SO_ACCEPTCONN).map_err(|_| ChannelError::NotSocket)?;
    if domain != libc::AF_UNIX || kind != libc::SOCK_STREAM || listening != 0 {
        return Err(ChannelError::NotSocket);
    }
    let cred = sys::peercred(b).map_err(|_| ChannelError::NotSocket)?;
    if cred.uid != sys::geteuid() {
        return Err(ChannelError::OtherUser);
    }
    Ok(())
}

fn read_until_end(s: &mut impl Read) -> Result<Zeroizing<Vec<u8>>, ChannelError> {
    // One byte more than the maximum: if it arrives, the channel went over the size.
    let mut buf = Zeroizing::new([0u8; MAX_TOKEN + 1]);
    let mut read = 0;
    while read < buf.len() {
        match s.read(&mut buf[read..]) {
            Ok(0) => break,
            Ok(n) => read += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                return Err(ChannelError::Deadline);
            }
            Err(e) => return Err(ChannelError::Io(e.kind())),
        }
    }
    if read == 0 {
        return Err(ChannelError::Empty);
    }
    if read > MAX_TOKEN {
        return Err(ChannelError::TooLarge);
    }
    let mut v = Zeroizing::new(Vec::with_capacity(read));
    v.extend_from_slice(&buf[..read]);
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::IntoRawFd;

    const TOKEN: &[u8] = b"AbCdEfGhIjKlMnOpQrStUvWxYz012345";

    /// The number of an fd whose ownership leaves this test (it goes to
    /// `read_inherited_token`, as in `ward ssh`, which gets the fd from the parent).
    fn hand_over(fd: impl Into<OwnedFd>) -> u32 {
        u32::try_from(fd.into().into_raw_fd()).unwrap()
    }

    /// What `/proc/self/fd/<n>` points to now (`socket:[inode]`, …). The tests run in
    /// parallel and the number may be reused, but a closed socket never comes back with
    /// the same inode.
    fn target(n: u32) -> Option<std::path::PathBuf> {
        std::fs::read_link(format!("/proc/self/fd/{n}")).ok()
    }

    #[test]
    fn token_goes_through_the_channel_and_the_fd_closes() {
        let c = ConnectChannel::new(TOKEN).unwrap();
        let n = hand_over(c.child);
        let before = target(n);
        assert!(before.is_some());
        let t = read_inherited_token(n).unwrap();
        assert_eq!(t.as_slice(), TOKEN);
        assert_ne!(target(n), before, "o fd do canal continua aberto");
    }

    #[test]
    fn the_ends_are_born_with_cloexec() {
        let c = ConnectChannel::new(TOKEN).unwrap();
        assert!(sys::has_cloexec(c.child.as_fd()).unwrap());
        assert!(!format!("{c:?}").contains("AbCd"));
    }

    /// Attack: channel with big garbage, empty, that never closes, or that is not a
    /// socket (pipe and file, which the same uid reopens through `/proc/<pid>/fd`);
    /// number 0–2, out of range and closed. All refused, and the message never carries
    /// the content.
    #[test]
    fn attack_forged_channel_is_refused_without_echo() {
        let garbage = b"falso-lixo-do-canal-".repeat(40);
        let (mut a, b) = UnixStream::pair().unwrap();
        a.write_all(&garbage).unwrap();
        drop(a);
        let e = read_inherited_token(hand_over(b)).unwrap_err();
        assert_eq!(e, ChannelError::TooLarge);
        assert!(!e.message().contains("falso"));

        // 65 bytes: one more than the maximum.
        let (mut a, b) = UnixStream::pair().unwrap();
        a.write_all(&[b'a'; MAX_TOKEN + 1]).unwrap();
        drop(a);
        assert_eq!(
            read_inherited_token(hand_over(b)),
            Err(ChannelError::TooLarge)
        );

        let (a, b) = UnixStream::pair().unwrap();
        drop(a);
        assert_eq!(read_inherited_token(hand_over(b)), Err(ChannelError::Empty));

        // The other side never closes nor writes: the deadline runs out and the fd
        // closes.
        let (a, b) = UnixStream::pair().unwrap();
        let n = hand_over(b);
        let before = target(n);
        assert_eq!(read_inherited_token(n), Err(ChannelError::Deadline));
        assert_ne!(target(n), before, "o fd do canal continua aberto");
        drop(a);

        let (r, mut w) = std::io::pipe().unwrap();
        w.write_all(TOKEN).unwrap();
        drop(w);
        assert_eq!(
            read_inherited_token(hand_over(r)),
            Err(ChannelError::NotSocket)
        );

        let f = std::fs::File::open("/proc/self/status").unwrap();
        assert_eq!(
            read_inherited_token(hand_over(f)),
            Err(ChannelError::NotSocket)
        );

        // Listening socket and datagram socket.
        let dir = crate::socket::runtime_dir().join(format!("ward-canal-t-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let l = std::os::unix::net::UnixListener::bind(dir.join("s")).unwrap();
        assert_eq!(
            read_inherited_token(hand_over(l)),
            Err(ChannelError::NotSocket)
        );
        let _ = std::fs::remove_dir_all(&dir);
        let (d, _d2) = std::os::unix::net::UnixDatagram::pair().unwrap();
        assert_eq!(
            read_inherited_token(hand_over(d)),
            Err(ChannelError::NotSocket)
        );

        for n in [0, 1, 2, MAX_FD_NUMBER + 1, u32::MAX] {
            assert_eq!(read_inherited_token(n), Err(ChannelError::Number), "{n}");
        }
        // Closed (a high number this process does not use).
        assert_eq!(
            read_inherited_token(MAX_FD_NUMBER - 7),
            Err(ChannelError::Number)
        );
    }

    /// Attack (the reason for the socket): another process of the same uid tries to
    /// reopen the channel through `/proc/<pid>/fd/<n>` of whoever holds it. With a pipe
    /// that would work; with the socket, the `open` fails (`ENXIO`) and the token stays
    /// only in the channel's queue.
    #[test]
    fn attack_reopening_the_channel_through_proc_fails() {
        let c = ConnectChannel::new(TOKEN).unwrap();
        let path = format!("/proc/self/fd/{}", c.number());
        let e = std::fs::File::open(&path).unwrap_err();
        assert_eq!(e.raw_os_error(), Some(libc::ENXIO), "{e}");
        assert!(std::fs::OpenOptions::new().write(true).open(&path).is_err());
        // The token is still whole for the owner of the fd.
        let t = read_inherited_token(hand_over(c.child)).unwrap();
        assert_eq!(t.as_slice(), TOKEN);
        // Counterproof: a pipe in the same place would be reopened and read.
        let (r, mut w) = std::io::pipe().unwrap();
        w.write_all(b"falso-pelo-pipe").unwrap();
        let mut other = std::fs::File::open(format!("/proc/self/fd/{}", r.as_raw_fd())).unwrap();
        drop(w);
        let mut read = Vec::new();
        other.read_to_end(&mut read).unwrap();
        assert_eq!(read, b"falso-pelo-pipe");
    }

    /// The `exec` carries only the channel (besides 0–2): an fd of the process without
    /// `FD_CLOEXEC` does not pass, and the channel passes with the same number. Checks
    /// by the link target (inode), not only by the number, which `ls` may reuse.
    #[test]
    fn exec_carries_only_the_channel() {
        let c = ConnectChannel::new(TOKEN).unwrap();
        let (loose, _w) = std::io::pipe().unwrap();
        sys::clear_cloexec(loose.as_fd()).unwrap();
        let loose_target = target(u32::try_from(loose.as_raw_fd()).unwrap()).unwrap();
        let channel_target = target(u32::try_from(c.number()).unwrap()).unwrap();
        let mut cmd = Command::new("/bin/ls");
        cmd.args(["-l", "/proc/self/fd"])
            .env_clear()
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        c.prepare(&mut cmd);
        let output = String::from_utf8(cmd.output().unwrap().stdout).unwrap();
        let channel = format!("{} -> {}", c.number(), channel_target.display());
        assert!(output.contains(&channel), "{output}");
        assert!(
            !output.contains(&*loose_target.to_string_lossy()),
            "{output}"
        );
        // 0–2, the channel and the fd `ls` itself opens to read the directory.
        assert!(
            output.lines().filter(|l| l.contains(" -> ")).count() <= 5,
            "{output}"
        );
        assert!(sys::has_cloexec(c.child.as_fd()).unwrap());
    }

    #[test]
    fn empty_or_large_token_does_not_even_become_a_channel() {
        assert!(ConnectChannel::new(b"").is_err());
        assert!(ConnectChannel::new(&[b'a'; MAX_TOKEN + 1]).is_err());
    }
}
