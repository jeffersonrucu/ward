//! Master password read from the terminal (Phase 25, the unlock subcommand of the CLI;
//! scope: unlock "through the CLI in the user's terminal"; S5, S8).
//!
//! - Reads from `/dev/tty`, never from stdin nor argv (S3): a pipe or a file do not
//!   work, and the password does not go through the command line.
//! - The terminal stays without echo, without canonical mode and without signals
//!   (`ISIG`) during the read, and goes back to the previous state on the way out, also
//!   on error. Ctrl+C arrives as a byte and cancels; that way the process does not die
//!   with the echo turned off.
//! - Pending input is discarded before the prompt (`TCSAFLUSH`): what was typed before
//!   the question showed up does not become the password.
//! - The buffer is a `Zeroizing<Vec<u8>>` with the final capacity reserved before the
//!   first byte: no reallocation leaves a copy of the password behind. The erased byte
//!   (Backspace, Ctrl+U) is zeroed before leaving the buffer.

use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsFd;
use std::os::unix::fs::OpenOptionsExt;

use zeroize::{Zeroize, Zeroizing};

use crate::sys;

/// Why the password was not read. No content: it can go to the message to the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalError {
    /// No terminal (`/dev/tty` does not open or is not a terminal).
    NoTerminal,
    /// The user canceled (Ctrl+C, Ctrl+D with an empty line, end of input).
    Canceled,
    /// More bytes than the limit.
    TooLong,
    /// The echo did not turn off, or another I/O error of the terminal.
    Io(io::ErrorKind),
}

impl fmt::Display for TerminalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TerminalError::NoTerminal => f.write_str("sem terminal para ler a senha"),
            TerminalError::Canceled => f.write_str("cancelado"),
            TerminalError::TooLong => f.write_str("senha longa demais"),
            TerminalError::Io(k) => write!(f, "erro no terminal ({k})"),
        }
    }
}

impl std::error::Error for TerminalError {}

/// Opens `/dev/tty` and reads a password of up to `max` bytes, without echo, after
/// writing `prompt` on the terminal itself.
pub fn read_password(prompt: &str, max: usize) -> Result<Zeroizing<Vec<u8>>, TerminalError> {
    let tty = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOCTTY | libc::O_CLOEXEC)
        .open("/dev/tty")
        .map_err(|_| TerminalError::NoTerminal)?;
    read_password_on(&tty, prompt, max)
}

/// Puts the terminal back to the previous state on `Drop`.
struct Restore<'a> {
    tty: &'a File,
    original: libc::termios,
}

impl Drop for Restore<'_> {
    fn drop(&mut self) {
        let _ = sys::tcsetattr(self.tty.as_fd(), libc::TCSANOW, &self.original);
    }
}

/// [`read_password`] on an already open terminal.
pub(crate) fn read_password_on(
    tty: &File,
    prompt: &str,
    max: usize,
) -> Result<Zeroizing<Vec<u8>>, TerminalError> {
    let original = sys::tcgetattr(tty.as_fd()).map_err(|_| TerminalError::NoTerminal)?;
    let mut raw = original;
    raw.c_lflag &= !(libc::ECHO | libc::ECHONL | libc::ICANON | libc::ISIG | libc::IEXTEN);
    raw.c_cc[libc::VMIN] = 1;
    raw.c_cc[libc::VTIME] = 0;
    let restore = Restore { tty, original };
    sys::tcsetattr(tty.as_fd(), libc::TCSAFLUSH, &raw).map_err(|e| TerminalError::Io(e.kind()))?;
    // Checks back: a terminal that accepted the request without turning the echo off
    // would show the password on the screen.
    let now = sys::tcgetattr(tty.as_fd()).map_err(|e| TerminalError::Io(e.kind()))?;
    if now.c_lflag & (libc::ECHO | libc::ICANON) != 0 {
        return Err(TerminalError::Io(io::ErrorKind::Unsupported));
    }
    let mut writer = tty;
    writer
        .write_all(prompt.as_bytes())
        .and_then(|()| writer.flush())
        .map_err(|e| TerminalError::Io(e.kind()))?;
    let mut reader = tty;
    let r = read_line(&mut reader, max);
    let _ = writer.write_all(b"\n");
    drop(restore);
    r
}

/// Reads bytes up to Enter, one at a time, with the minimal editing of a terminal
/// without canonical mode: Backspace/DEL erases the last byte (zeroed), Ctrl+U erases
/// the line, Ctrl+C cancels, Ctrl+D cancels with an empty line. Other control bytes are
/// ignored.
pub(crate) fn read_line(
    r: &mut impl Read,
    max: usize,
) -> Result<Zeroizing<Vec<u8>>, TerminalError> {
    let mut password = Zeroizing::new(Vec::with_capacity(max));
    let mut b = Zeroizing::new([0u8; 1]);
    loop {
        match r.read(&mut b[..]) {
            Ok(0) => return Err(TerminalError::Canceled),
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(TerminalError::Io(e.kind())),
        }
        match b[0] {
            b'\r' | b'\n' => break,
            0x03 => return Err(TerminalError::Canceled),
            0x04 if password.is_empty() => return Err(TerminalError::Canceled),
            0x7f | 0x08 => {
                if let Some(last) = password.last_mut() {
                    *last = 0;
                    let n = password.len() - 1;
                    password.truncate(n);
                }
            }
            0x15 => {
                password.zeroize();
                password.clear();
            }
            c if c < 0x20 => {}
            c => {
                if password.len() >= max {
                    return Err(TerminalError::TooLong);
                }
                password.push(c);
            }
        }
    }
    b.zeroize();
    Ok(password)
}

// --- interactive `ward ssh` (Phase 27) -------------------------------------------------

/// Is `fd` a terminal?
pub fn is_terminal(fd: impl AsFd) -> bool {
    sys::tcgetattr(fd.as_fd()).is_ok()
}

/// (rows, columns) of the terminal on `fd`; `None` if it is not a terminal or does not
/// say.
pub fn size(fd: impl AsFd) -> Option<(u16, u16)> {
    sys::read_window_size(fd.as_fd())
        .ok()
        .filter(|&(l, c)| l > 0 && c > 0)
}

/// The CLI's terminal in raw mode while it lasts (`cfmakeraw`: no echo, no canonical
/// mode, no signals; Ctrl+C goes as a byte to the remote `ssh`, as in `ssh`). Goes back
/// to the previous state on `Drop`, also on error or panic.
pub struct RawMode {
    fd: std::os::fd::OwnedFd,
    original: libc::termios,
}

impl fmt::Debug for RawMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RawMode")
    }
}

impl RawMode {
    /// Turns raw mode on for the terminal `fd` (a copy of the fd is kept to go back).
    pub fn enable(fd: impl AsFd) -> io::Result<RawMode> {
        let fd = fd.as_fd().try_clone_to_owned()?;
        let original = sys::tcgetattr(fd.as_fd())?;
        let mut raw = original;
        sys::make_raw(&mut raw);
        sys::tcsetattr(fd.as_fd(), libc::TCSANOW, &raw)?;
        Ok(RawMode { fd, original })
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = sys::tcsetattr(self.fd.as_fd(), libc::TCSANOW, &self.original);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_mode_turns_on_and_back_and_pty_size() {
        let (master, slave) = sys::open_pty().unwrap();
        sys::set_window_size(master.as_fd(), 33, 101).unwrap();
        assert!(is_terminal(&slave));
        assert!(!is_terminal(File::open("/dev/null").unwrap()));
        assert_eq!(size(&slave), Some((33, 101)));
        assert_eq!(size(File::open("/dev/null").unwrap()), None);
        let before = sys::tcgetattr(slave.as_fd()).unwrap();
        assert_ne!(before.c_lflag & libc::ECHO, 0);
        {
            let _raw = RawMode::enable(&slave).unwrap();
            let now = sys::tcgetattr(slave.as_fd()).unwrap();
            assert_eq!(now.c_lflag & (libc::ECHO | libc::ICANON | libc::ISIG), 0);
        }
        let after = sys::tcgetattr(slave.as_fd()).unwrap();
        assert_eq!(after.c_lflag, before.c_lflag);
        assert_eq!(after.c_iflag, before.c_iflag);
    }

    fn line(bytes: &[u8], max: usize) -> Result<Vec<u8>, TerminalError> {
        read_line(&mut &bytes[..], max).map(|s| s.to_vec())
    }

    #[test]
    fn line_editing() {
        assert_eq!(line(b"abc\n", 64).unwrap(), b"abc");
        assert_eq!(line(b"abc\r", 64).unwrap(), b"abc");
        assert_eq!(line(b"abx\x7fc\n", 64).unwrap(), b"abc");
        assert_eq!(line(b"\x7f\x08ab\x08c\n", 64).unwrap(), b"ac");
        assert_eq!(line(b"errada\x15certa\n", 64).unwrap(), b"certa");
        assert_eq!(line(b"a\x1bb\x01c\n", 64).unwrap(), b"abc");
        assert_eq!(line("ção\n".as_bytes(), 64).unwrap(), "ção".as_bytes());
        assert_eq!(line(b"\n", 64).unwrap(), b"");
        assert_eq!(line(b"ab\x04c\n", 64).unwrap(), b"abc");
    }

    #[test]
    fn cancel_and_limit() {
        assert_eq!(line(b"abc\x03def\n", 64), Err(TerminalError::Canceled));
        assert_eq!(line(b"\x04", 64), Err(TerminalError::Canceled));
        assert_eq!(line(b"abc", 64), Err(TerminalError::Canceled));
        assert_eq!(line(b"abcd\n", 4).unwrap(), b"abcd");
        assert_eq!(line(b"abcde\n", 4), Err(TerminalError::TooLong));
    }

    #[test]
    fn capacity_reserved_without_reallocating() {
        let s = read_line(&mut &b"0123456789\n"[..], 16).unwrap();
        assert_eq!(s.capacity(), 16);
    }

    #[test]
    fn regular_file_is_not_a_terminal() {
        let f = File::open("/dev/null").unwrap();
        assert_eq!(
            read_password_on(&f, "senha: ", 64).unwrap_err(),
            TerminalError::NoTerminal
        );
    }

    /// Attack: does someone looking at the screen (or at what the terminal sends back)
    /// see the password? On a real pty, the master only gets the prompt and the line
    /// break; the typed password does not echo, the input pending before the prompt is
    /// discarded and the terminal comes back with the echo on.
    #[test]
    fn attack_password_does_not_echo_on_the_pty() {
        let (master, slave) = sys::open_pty().unwrap();
        let master = File::from(master);
        let slave = File::from(slave);
        let before = sys::tcgetattr(slave.as_fd()).unwrap();
        assert_ne!(before.c_lflag & libc::ECHO, 0, "o pty novo começa com eco");

        // Typed before the prompt: discarded by TCSAFLUSH and does not become the
        // password.
        (&master).write_all(b"antes\n").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));

        let reader = std::thread::spawn(move || {
            let r = read_password_on(&slave, "senha mestra: ", 64).map(|s| s.to_vec());
            let after = sys::tcgetattr(slave.as_fd()).unwrap();
            (r, after.c_lflag)
        });
        // Waits for the prompt before typing.
        let mut seen = Vec::new();
        let mut buf = [0u8; 256];
        while !seen.ends_with(b"senha mestra: ") {
            let n = (&master).read(&mut buf).unwrap();
            assert!(n > 0);
            seen.extend_from_slice(&buf[..n]);
        }
        (&master).write_all(b"falso-eco-01\r").unwrap();
        let (r, lflag) = reader.join().unwrap();
        assert_eq!(r.unwrap(), b"falso-eco-01");
        assert_ne!(lflag & libc::ECHO, 0, "o eco volta ao sair");
        assert_ne!(lflag & libc::ICANON, 0);
        assert_ne!(lflag & libc::ISIG, 0);

        // What showed up on the "screen": only the prompt and the line break.
        let mut rest = Vec::new();
        let _ = sys::tcsetattr(master.as_fd(), libc::TCSANOW, &{
            let mut t = sys::tcgetattr(master.as_fd()).unwrap();
            t.c_cc[libc::VMIN] = 0;
            t.c_cc[libc::VTIME] = 2;
            t
        });
        loop {
            match (&master).read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => rest.extend_from_slice(&buf[..n]),
            }
        }
        seen.extend_from_slice(&rest);
        let screen = String::from_utf8_lossy(&seen);
        assert!(
            !screen.contains("falso-eco-01"),
            "a senha ecoou: {screen:?}"
        );
        // "antes" echoed when it was typed, with the echo still on; what matters is that
        // it did not become the password (checked above).
        assert!(screen.contains("senha mestra: "));
    }

    #[test]
    fn ctrl_c_cancels_and_restores() {
        let (master, slave) = sys::open_pty().unwrap();
        let master = File::from(master);
        let slave = File::from(slave);
        let reader = std::thread::spawn(move || {
            let r = read_password_on(&slave, "senha: ", 64).map(|s| s.to_vec());
            (r, sys::tcgetattr(slave.as_fd()).unwrap().c_lflag)
        });
        let mut buf = [0u8; 64];
        let mut seen = Vec::new();
        while !seen.ends_with(b"senha: ") {
            let n = (&master).read(&mut buf).unwrap();
            seen.extend_from_slice(&buf[..n]);
        }
        (&master).write_all(b"meia\x03").unwrap();
        let (r, lflag) = reader.join().unwrap();
        assert_eq!(r, Err(TerminalError::Canceled));
        assert_ne!(lflag & libc::ECHO, 0);
    }
}
