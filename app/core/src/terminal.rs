//! "Connect" of a server entry (D3, V5.24, Phase 34).
//!
//! The app asks the daemon for a single-use token (issue token operation: 30 s, one connection,
//! bound to the entry) and opens the default terminal running
//! `ward ssh --token-fd <n> <entry>`.
//!
//! The token skips the level and the approval, so no other process of the same uid may
//! see it (Phase 27a; high finding of the M3 audit). It **never** goes in argv nor in an
//! environment variable (`/proc/<pid>/cmdline` and `/proc/<pid>/environ` are readable by
//! the same uid), nor to the interface, nor to a log. It goes through a Unix socket
//! ([`ward_platform::connect::ConnectChannel`]): the app writes the token and closes its
//! end; the other end is the only fd ≥ 3 the terminal inherits, and the terminal passes
//! it on to `ward ssh`, which reads the token and closes the fd before connecting. The fd
//! number goes in argv (it is not a secret). A socket does not reopen through
//! `/proc/<pid>/fd/<n>`, unlike a pipe or a file.
//!
//! Checked terminals (the terminal must pass the inherited fd on to the `-e` command):
//! `cosmic-term` (the COSMIC `x-terminal-emulator`) and `xterm`. A terminal that hands the
//! command to an already open server (e.g. `gnome-terminal`) does not pass the fd on:
//! `ward ssh` exits with the error that the Connect token channel is not open and does
//! not connect (fails closed).
//!
//! The real [`Terminal`] ([`SystemTerminal`]) calls `/usr/bin/x-terminal-emulator -e`
//! (the Debian/Ubuntu default terminal; the Debian policy requires the `-e <command>
//! <arguments>`) with the `ward` installed in `/usr/local/bin` and an environment built
//! from scratch with only what the terminal needs to open in the session. The app has
//! already dropped the `ward-app` group before GTK (D47), so the terminal does not
//! inherit the app role.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ward_platform::connect::ConnectChannel;
use zeroize::Zeroizing;

use crate::AppError;

/// "Connect" token, already checked ([`Token::from_response`]).
pub struct Token(Zeroizing<String>);

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(••••)")
    }
}

/// Smallest and largest token accepted from the daemon response.
const MIN_TOKEN: usize = 16;
const MAX_TOKEN: usize = 64;

impl Token {
    /// The daemon response is not trusted (S1): only `[A-Za-z0-9_-]`, from 16 to 64
    /// bytes, not starting with `-` (the format `ward ssh` accepts from the channel).
    pub fn from_response(bytes: &[u8]) -> Option<Token> {
        let ok = (MIN_TOKEN..=MAX_TOKEN).contains(&bytes.len())
            && bytes[0] != b'-'
            && bytes
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'));
        ok.then(|| Token(Zeroizing::new(String::from_utf8_lossy(bytes).into_owned())))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn expose(&self) -> &str {
        &self.0
    }
}

/// Entry name that may go to argv: the `ward-core` format (starts with a lowercase letter
/// or a digit; then `[a-z0-9._-]`), up to `MAX_NAME`.
pub fn entry_for_argv(name: &str) -> bool {
    let mut b = name.bytes();
    name.len() <= ward_core::limits::MAX_NAME
        && b.next()
            .is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit())
        && b.all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
        })
}

/// Who opens the terminal.
pub trait Terminal: Send + Sync {
    fn open(&self, entry: &str, token: &Token) -> Result<(), AppError>;
}

/// A terminal chosen at run time (the test bridge, Phase 39c).
impl<T: Terminal + ?Sized> Terminal for Box<T> {
    fn open(&self, entry: &str, token: &Token) -> Result<(), AppError> {
        (**self).open(entry, token)
    }
}

/// No terminal (the service default until someone sets one up).
pub struct NoTerminal;

impl Terminal for NoTerminal {
    fn open(&self, _entry: &str, _token: &Token) -> Result<(), AppError> {
        Err(AppError::TerminalUnavailable)
    }
}

/// Variables the terminal receives from the app environment (which is already the D57
/// one).
const TERMINAL_ENV: &[&str] = &[
    "HOME",
    "LANG",
    "XDG_RUNTIME_DIR",
    "WAYLAND_DISPLAY",
    "DBUS_SESSION_BUS_ADDRESS",
    "XDG_CURRENT_DESKTOP",
    "XDG_SESSION_TYPE",
];

/// The system default terminal.
pub struct SystemTerminal {
    emulator: PathBuf,
    ward: PathBuf,
}

impl Default for SystemTerminal {
    fn default() -> Self {
        SystemTerminal {
            emulator: PathBuf::from("/usr/bin/x-terminal-emulator"),
            ward: PathBuf::from("/usr/local/bin/ward"),
        }
    }
}

/// The terminal command and the token channel. The channel must live until the `spawn`
/// returns ([`Opening::open`] takes care of that).
pub struct Opening {
    pub command: Command,
    channel: ConnectChannel,
}

impl Opening {
    /// Runs the terminal and releases the app end of the channel: the terminal keeps the
    /// only copy, and the token, only in the socket queue.
    pub fn open(mut self) -> std::io::Result<std::process::Child> {
        let child = self.command.spawn();
        drop(self.channel);
        child
    }
}

impl SystemTerminal {
    /// For the tests: another emulator and another `ward`.
    pub fn with_paths(emulator: PathBuf, ward: PathBuf) -> SystemTerminal {
        SystemTerminal { emulator, ward }
    }

    /// The command, without running it (the tests check argv and environment), with the
    /// token channel already written. The token goes neither in argv nor in the
    /// environment: only the fd number.
    pub fn command(&self, entry: &str, token: &Token) -> Result<Opening, AppError> {
        if !entry_for_argv(entry) {
            return Err(AppError::InvalidRequest);
        }
        let channel = ConnectChannel::new(token.expose().as_bytes())
            .map_err(|_| AppError::TerminalUnavailable)?;
        let mut c = Command::new(&self.emulator);
        c.env_clear()
            .env("PATH", "/usr/local/bin:/usr/bin:/bin")
            .current_dir(Path::new("/"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .arg("-e")
            .arg(&self.ward)
            .args(["ssh", "--token-fd", &channel.number().to_string(), entry]);
        for k in TERMINAL_ENV {
            if let Some(v) = std::env::var_os(k) {
                c.env(k, v);
            }
        }
        channel.prepare(&mut c);
        Ok(Opening {
            command: c,
            channel,
        })
    }
}

impl Terminal for SystemTerminal {
    fn open(&self, entry: &str, token: &Token) -> Result<(), AppError> {
        let a = self.command(entry, token)?;
        let mut child = a.open().map_err(|_| AppError::TerminalUnavailable)?;
        // Waits for the terminal in a thread so no zombie process is left.
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attack_token_forged_by_the_daemon_is_refused() {
        for t in [
            &b""[..],
            b"curto",
            b"--help-aaaaaaaaaaaaaaaa",
            b"-aaaaaaaaaaaaaaaaaaaaaaaa",
            b"aaaaaaaaaaaaaaaa bbbbbbbb",
            b"aaaaaaaaaaaaaaaa;rm -rf /",
            b"aaaaaaaaaaaaaaaa\nbbbbbbbb",
            b"aaaaaaaaaaaaaaaa\xff",
            &[b'a'; 65],
        ] {
            assert!(Token::from_response(t).is_none(), "{t:?}");
        }
        let ok = Token::from_response(b"Ab3_-xyzXYZ01234").unwrap();
        assert_eq!(ok.len(), 16);
        assert!(!format!("{ok:?}").contains("Ab3"));
    }

    #[test]
    fn attack_entry_that_would_become_an_option_does_not_go_to_argv() {
        for e in ["", "-o", "--token", "Acme", "a b", "a;b", "a\nb", "á"] {
            assert!(!entry_for_argv(e), "{e:?}");
        }
        assert!(entry_for_argv("acme-ssh-salto"));
        let t = Token::from_response(b"aaaaaaaaaaaaaaaaaaaa").unwrap();
        assert_eq!(
            SystemTerminal::default()
                .command("-oProxyCommand=x", &t)
                .err(),
            Some(AppError::InvalidRequest)
        );
    }

    #[test]
    fn terminal_command_with_environment_from_scratch() {
        let t = Token::from_response(b"tokendeteste0123456789").unwrap();
        let a = SystemTerminal::default()
            .command("acme-ssh-salto", &t)
            .unwrap();
        let c = &a.command;
        assert_eq!(c.get_program(), "/usr/bin/x-terminal-emulator");
        let args: Vec<_> = c.get_args().map(|a| a.to_str().unwrap()).collect();
        let fd = a.channel.number().to_string();
        assert_eq!(
            args,
            [
                "-e",
                "/usr/local/bin/ward",
                "ssh",
                "--token-fd",
                fd.as_str(),
                "acme-ssh-salto"
            ]
        );
        let envs: Vec<_> = c
            .get_envs()
            .filter_map(|(k, v)| v.map(|_| k.to_str().unwrap().to_owned()))
            .collect();
        for k in &envs {
            assert!(k == "PATH" || TERMINAL_ENV.contains(&k.as_str()), "{k}");
        }
        // Phase 27a: the token is in no argument nor environment value.
        for v in c
            .get_args()
            .chain(c.get_envs().filter_map(|(_, v)| v))
            .chain([c.get_program()])
        {
            assert!(!v.to_string_lossy().contains("tokendeteste"), "{v:?}");
        }
        assert_eq!(c.get_current_dir(), Some(Path::new("/")));
    }

    /// Phase 27a (high finding of the M3 audit): a real "Connect", with a fake terminal
    /// (drops the `-e` and runs the rest) and a fake `ward` (reads the `--token-fd` fd,
    /// writes what it read and the fd list, and waits). While both run, a reader in a
    /// loop sweeps `/proc/*/cmdline` and `/proc/*/environ` of every process and tries to
    /// reopen `/proc/<pid>/fd/*` of the terminal and of the `ward`: the token shows up
    /// nowhere, and the fake `ward` receives it whole through the channel.
    #[test]
    fn attack_connect_token_does_not_show_up_in_proc() {
        use std::os::unix::fs::PermissionsExt;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

        let base = std::env::temp_dir().join(format!("ward-terminal-27a-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir(&base).unwrap();
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700)).unwrap();
        let emulator = base.join("terminal-falso");
        let ward = base.join("ward-falso");
        let output = base.join("saida");
        let ready = base.join("pronto");
        // The scripts are written to a `.tmp` and `/bin/cp` creates the file that runs:
        // no fd of this (multithreaded) process ever writes the inode that is executed,
        // so a child forked at the same time by another test cannot hold it open for
        // writing and make the `exec` fail with ETXTBSY (dash then exits 2).
        let install = |path: &Path, content: &str| {
            let tmp = path.with_extension("tmp");
            std::fs::write(&tmp, content).unwrap();
            let st = Command::new("/bin/cp")
                .arg("--")
                .arg(&tmp)
                .arg(path)
                .status()
                .unwrap();
            assert!(st.success(), "{st:?}");
            std::fs::remove_file(&tmp).unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
        };
        install(&emulator, "#!/bin/sh\nshift\nexec \"$@\"\n");
        // $1=ssh $2=--token-fd $3=<n> $4=<entry>. Reads the channel, lists the fds and
        // stays alive a little so the reader sweeps the process with the fd still open.
        install(
            &ward,
            &format!(
                "#!/bin/sh\nls -l /proc/$$/fd > '{d}/fds' 2>&1\nsleep 0.4\n\
                 eval \"cat <&$3\" > '{s}'\n: > '{p}'\nsleep 0.4\n",
                d = base.display(),
                s = output.display(),
                p = ready.display()
            ),
        );

        // A new token on each run, generated here (never written in the repository).
        let generated = ward_core::generator::connect_token().unwrap();
        let token = Token::from_response(generated.expose()).unwrap();
        let needle = generated.expose().to_vec();

        let stop = Arc::new(AtomicBool::new(false));
        let found = Arc::new(AtomicUsize::new(0));
        let scanned = Arc::new(AtomicUsize::new(0));
        let fds_tried = Arc::new(AtomicUsize::new(0));
        let reader = {
            let (stop, found, scanned, fds_tried) = (
                stop.clone(),
                found.clone(),
                scanned.clone(),
                fds_tried.clone(),
            );
            let needle = needle.clone();
            let ward = ward.clone();
            std::thread::spawn(move || {
                let contains = |b: &[u8]| b.windows(needle.len()).any(|w| w == needle);
                while !stop.load(Ordering::Relaxed) {
                    let Ok(dir) = std::fs::read_dir("/proc") else {
                        continue;
                    };
                    for p in dir.flatten() {
                        let name = p.file_name();
                        if !name.to_string_lossy().bytes().all(|b| b.is_ascii_digit()) {
                            continue;
                        }
                        for file in ["cmdline", "environ"] {
                            if let Ok(b) = std::fs::read(p.path().join(file)) {
                                scanned.fetch_add(1, Ordering::Relaxed);
                                if contains(&b) {
                                    found.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        }
                        // The fake terminal and `ward` processes: reopen each fd.
                        let cmd = std::fs::read(p.path().join("cmdline")).unwrap_or_default();
                        if contains_path(&cmd, &ward)
                            && let Ok(fds) = std::fs::read_dir(p.path().join("fd"))
                        {
                            for f in fds.flatten() {
                                // 0–2 are the test's own output files (the fake `ward`
                                // writes what it read to them); the channel is ≥ 3.
                                if matches!(f.file_name().to_str(), Some("0" | "1" | "2")) {
                                    continue;
                                }
                                fds_tried.fetch_add(1, Ordering::Relaxed);
                                if let Ok(mut a) = std::fs::File::open(f.path()) {
                                    let mut buf = vec![0u8; 256];
                                    if let Ok(n) = std::io::Read::read(&mut a, &mut buf)
                                        && contains(&buf[..n])
                                    {
                                        found.fetch_add(1, Ordering::Relaxed);
                                    }
                                }
                            }
                        }
                    }
                }
            })
        };

        let t = SystemTerminal::with_paths(emulator.clone(), ward.clone());
        let a = t.command("acme-ssh-salto", &token).unwrap();
        let mut child = a.open().unwrap();
        let st = child.wait().unwrap();
        stop.store(true, Ordering::Relaxed);
        reader.join().unwrap();

        assert!(st.success(), "{st:?}");
        assert!(ready.exists());
        assert_eq!(
            std::fs::read(&output).unwrap(),
            needle,
            "o ward não recebeu o token"
        );
        let fds = std::fs::read_to_string(base.join("fds")).unwrap();
        assert!(fds.contains("socket:["), "{fds}");
        assert!(scanned.load(Ordering::Relaxed) > 0);
        assert!(
            fds_tried.load(Ordering::Relaxed) > 0,
            "o leitor não viu o ward"
        );
        assert_eq!(
            found.load(Ordering::Relaxed),
            0,
            "token visto por outro processo"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    fn contains_path(cmdline: &[u8], path: &Path) -> bool {
        let c = path.as_os_str().as_encoded_bytes();
        cmdline.split(|b| *b == 0).any(|a| a == c)
    }
}
