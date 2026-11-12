//! Child of `ward run` (Phase 26; D13, D60; S2, S3, S8).
//!
//! The daemon creates the process with the argv, the directory and the environment it
//! builds, and only those:
//! - **exact environment**: nothing from the daemon's environment reaches the child
//!   (`LISTEN_*`, D60); the `envp` is only what [`Command::variable`] received;
//! - **no loose copy of a secret** (S8): argv, environment and directory stay in
//!   `Zeroizing`, allocated once at the exact size, and the `fork` + `execve` is done here
//!   (`sys::fork_exec`), without `std::process::Command`, which copies the environment
//!   into buffers that are not zeroed;
//! - **no shell** (S3): the argv goes straight to `execve`; the `PATH` search (the `PATH`
//!   of the child's environment, like `execvpe`) is done before, with no recourse to
//!   `/bin/sh`;
//! - **own session and group** (`setsid`): the child is the leader of a session and of a
//!   process group; no outside process joins the group (`setpgid` across sessions is
//!   refused by the kernel; M4 audit), and [`Group::kill`] kills the whole group
//!   (grandchildren included) when the CLI drops or the command ends;
//! - **dies together**: `PR_SET_PDEATHSIG` with `SIGKILL`, bound to the daemon thread
//!   that created it (the connection's, which lives until the end of the command);
//! - **no fd of the daemon**: every fd ≥ 3 closes on `exec`; the child gets only the
//!   pipes on 0, 1 and 2;
//! - signals at default and empty mask (the Rust runtime ignores `SIGPIPE`).
//!
//! The kernel leaves the child's `/proc/<pid>/environ` readable by another process of
//! the same uid while it runs (RA1, recorded in the Phase 26 handoff): `execve` sets
//! `dumpable` back to 1 for any ordinary program, and the daemon's `PR_SET_DUMPABLE`
//! does not survive the `exec`.

use std::ffi::CStr;
use std::fs::File;
use std::io;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use zeroize::Zeroizing;

use crate::{descendants, sys};

pub use crate::sys::OUTPUT_PIPE_CAPACITY;

/// `PATH` of glibc's `execvpe` when the environment has none.
const DEFAULT_PATH: &[u8] = b"/bin:/usr/bin";

/// Why the command was not built. Only fixed texts (never the value).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandError {
    /// Empty program or directory.
    Empty,
    /// Nul byte in an argument, name, value or directory.
    NulByte,
    /// Variable name empty or with `=`.
    InvalidName,
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CommandError::Empty => "programa ou diretório vazio",
            CommandError::NulByte => "byte nulo no comando ou no ambiente",
            CommandError::InvalidName => "nome de variável inválido",
        })
    }
}

impl std::error::Error for CommandError {}

/// Why the child was not created. No value of the command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchError {
    /// Failure of the daemon (pipe, `fork`).
    System(io::ErrorKind),
    /// Preparation of the child (`prctl`, `dup2`, `close_range`), with the `errno`.
    Preparation(i32),
    /// The `chdir` into the CLI's directory failed.
    Directory(i32),
    /// The `execve` failed on every path.
    Program(i32),
}

impl LaunchError {
    /// Exit code the CLI returns, like the shell: 127 for "not found", 126 for "found,
    /// but not executable".
    pub fn exit_code(self) -> u8 {
        match self {
            LaunchError::Program(e) if e == libc::ENOENT || e == libc::ENOTDIR => 127,
            LaunchError::Program(_) => 126,
            _ => 127,
        }
    }

    /// Fixed message in Portuguese.
    pub fn message(self) -> &'static str {
        match self {
            LaunchError::Program(e) if e == libc::ENOENT || e == libc::ENOTDIR => {
                "comando não encontrado"
            }
            LaunchError::Program(e) if e == libc::EACCES => "sem permissão para executar o comando",
            LaunchError::Program(e) if e == libc::ENOEXEC => {
                "o arquivo não é um executável (sem `#!`?)"
            }
            LaunchError::Program(_) => "o comando não pôde ser executado",
            LaunchError::Directory(_) => "o diretório de trabalho não existe ou não é acessível",
            LaunchError::Preparation(_) | LaunchError::System(_) => "não deu para criar o processo",
        }
    }
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for LaunchError {}

/// Bytes with the trailing nul, allocated once at the exact size.
fn c_bytes(parts: &[&[u8]]) -> Result<Zeroizing<Vec<u8>>, CommandError> {
    let n: usize = parts.iter().map(|p| p.len()).sum();
    let mut v = Zeroizing::new(Vec::with_capacity(n + 1));
    for p in parts {
        if p.contains(&0) {
            return Err(CommandError::NulByte);
        }
        v.extend_from_slice(p);
    }
    v.push(0);
    Ok(v)
}

/// The command to launch: argv, directory and the whole environment (nothing comes from
/// the daemon).
pub struct Command {
    argv: Vec<Zeroizing<Vec<u8>>>,
    env: Vec<Zeroizing<Vec<u8>>>,
    cwd: Zeroizing<Vec<u8>>,
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Comando({} argumentos, {} variáveis, ••••)",
            self.argv.len(),
            self.env.len()
        )
    }
}

impl Command {
    /// `program` is `argv[0]`: with `/`, it is the path (relative to `cwd`); without `/`,
    /// it is searched in the `PATH` of the child's environment.
    pub fn new(program: &[u8], cwd: &[u8]) -> Result<Command, CommandError> {
        if program.is_empty() || cwd.is_empty() {
            return Err(CommandError::Empty);
        }
        Ok(Command {
            argv: vec![c_bytes(&[program])?],
            env: Vec::new(),
            cwd: c_bytes(&[cwd])?,
        })
    }

    /// One more argument.
    pub fn argument(&mut self, a: &[u8]) -> Result<(), CommandError> {
        self.argv.push(c_bytes(&[a])?);
        Ok(())
    }

    /// One more variable of the child's environment (`name=value`).
    pub fn variable(&mut self, name: &[u8], value: &[u8]) -> Result<(), CommandError> {
        if name.is_empty() || name.contains(&b'=') {
            return Err(CommandError::InvalidName);
        }
        self.env.push(c_bytes(&[name, b"=", value])?);
        Ok(())
    }

    /// The paths to try, like `execvpe`.
    fn paths(&self) -> Vec<Zeroizing<Vec<u8>>> {
        let prog = &self.argv[0][..self.argv[0].len() - 1];
        if prog.contains(&b'/') {
            return vec![self.argv[0].clone()];
        }
        let path = self
            .env
            .iter()
            .find_map(|v| v.strip_prefix(b"PATH="))
            .map(|v| &v[..v.len() - 1])
            .unwrap_or(DEFAULT_PATH);
        path.split(|b| *b == b':')
            .filter_map(|dir| {
                let dir: &[u8] = if dir.is_empty() { b"." } else { dir };
                c_bytes(&[dir, b"/", prog]).ok()
            })
            .collect()
    }

    /// Creates the child with three pipes. Returns after the `execve` (or its failure,
    /// with the reason).
    pub fn launch(&self) -> Result<(Group, Pipes), LaunchError> {
        let (group, ends) = self.launch_with(sys::Stdio::Pipes)?;
        match ends {
            sys::ParentEnds::Pipes {
                stdin,
                stdout,
                stderr,
            } => Ok((
                group,
                Pipes {
                    stdin: Some(File::from(stdin)),
                    stdout: Some(File::from(stdout)),
                    stderr: Some(File::from(stderr)),
                },
            )),
            sys::ParentEnds::Pty | sys::ParentEnds::DebugPipe { .. } => {
                Err(LaunchError::System(io::ErrorKind::InvalidData))
            }
        }
    }

    /// Creates the child with Chrome's debugging pipe (`ward playwright`, Phase 44, D6):
    /// `/dev/null` on 0, 1 and 2, the commands on fd 3 and the responses on fd 4 of the
    /// child (`--remote-debugging-pipe`). No TCP port; the daemon's ends are `CLOEXEC`,
    /// so no other child inherits them.
    pub fn launch_with_debugging(&self) -> Result<(Group, DebugPipe), LaunchError> {
        let (group, ends) = self.launch_with(sys::Stdio::DebugPipe)?;
        match ends {
            sys::ParentEnds::DebugPipe {
                commands,
                responses,
            } => Ok((
                group,
                DebugPipe {
                    commands: File::from(commands),
                    responses: File::from(responses),
                },
            )),
            _ => Err(LaunchError::System(io::ErrorKind::InvalidData)),
        }
    }

    /// Creates the child in a new pty (interactive `ward ssh`, Phase 27; D13): the child
    /// is the leader of a new session with the slave as controlling terminal on 0, 1 and
    /// 2, and the window size already holds at `exec`. The daemon keeps only the master.
    pub fn launch_in_pty(&self, rows: u16, cols: u16) -> Result<(Group, Pty), LaunchError> {
        let (master, slave) = sys::open_pty().map_err(|e| LaunchError::System(e.kind()))?;
        sys::set_window_size(std::os::fd::AsFd::as_fd(&master), rows, cols)
            .map_err(|e| LaunchError::System(e.kind()))?;
        let (group, _) = self.launch_with(sys::Stdio::Pty(slave))?;
        Ok((
            group,
            Pty {
                master: File::from(master),
            },
        ))
    }

    fn launch_with(&self, stdio: sys::Stdio) -> Result<(Group, sys::ParentEnds), LaunchError> {
        let paths = self.paths();
        let ptr = |v: &Zeroizing<Vec<u8>>| v.as_ptr().cast::<libc::c_char>();
        let c_list: Vec<*const libc::c_char> = paths.iter().map(ptr).collect();
        let mut argv: Vec<*const libc::c_char> = self.argv.iter().map(ptr).collect();
        argv.push(std::ptr::null());
        let mut envp: Vec<*const libc::c_char> = self.env.iter().map(ptr).collect();
        envp.push(std::ptr::null());
        let cwd = CStr::from_bytes_with_nul(&self.cwd)
            .map_err(|_| LaunchError::System(io::ErrorKind::InvalidInput))?;
        let preparation = sys::ExecPreparation {
            paths: &c_list,
            argv: &argv,
            envp: &envp,
            cwd,
        };
        // Phase 55: the registry stays locked from the `fork` to the registration, so the
        // sweep at the end of a session never sees the new leader as an ownerless orphan.
        let mut leaders = descendants::record();
        match sys::fork_exec(&preparation, stdio) {
            Ok(f) => {
                leaders.push(f.pid);
                drop(leaders);
                Ok((
                    Group {
                        pid: f.pid,
                        collected: Mutex::new(false),
                        leftovers: AtomicUsize::new(0),
                    },
                    f.ends,
                ))
            }
            Err(sys::ExecError::System(e)) => Err(LaunchError::System(e.kind())),
            Err(sys::ExecError::Child { step, errno }) => Err(match step {
                sys::STEP_DIRECTORY => LaunchError::Directory(errno),
                sys::STEP_EXEC => LaunchError::Program(errno),
                _ => LaunchError::Preparation(errno),
            }),
        }
    }
}

/// The daemon's ends of the child's pipes: write to stdin, read from stdout and stderr.
#[derive(Debug)]
pub struct Pipes {
    pub stdin: Option<File>,
    pub stdout: Option<File>,
    pub stderr: Option<File>,
}

/// The daemon's ends of the debugging pipe: writes the commands (fd 3 of the child) and
/// reads the responses (fd 4 of the child).
#[derive(Debug)]
pub struct DebugPipe {
    pub commands: File,
    pub responses: File,
}

/// The master of the child's pty: the daemon reads the output and writes the input to it.
#[derive(Debug)]
pub struct Pty {
    pub master: File,
}

impl Pty {
    /// New window size (the kernel notifies the foreground group with `SIGWINCH`).
    pub fn set_window_size(&self, rows: u16, cols: u16) -> io::Result<()> {
        use std::os::fd::AsFd;
        sys::set_window_size(self.master.as_fd(), rows, cols)
    }
}

/// How the child ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    Code(u8),
    Signal(i32),
}

impl End {
    /// Exit code like the shell: its own, or 128 + the signal.
    pub fn exit_code(self) -> u8 {
        match self {
            End::Code(c) => c,
            End::Signal(s) => u8::try_from(128 + s.clamp(0, 127)).unwrap_or(255),
        }
    }
}

/// Is there something to read in the pipe (data or end) within `wait_ms`? Without
/// reading: the thread that reads the child's output checks from time to time whether it
/// should stop (a grandchild outside the group can hold the pipe open forever).
pub fn ready_to_read(f: &File, wait_ms: i32) -> io::Result<bool> {
    use std::os::fd::AsFd;
    sys::wait_readable(f.as_fd(), wait_ms)
}

/// Like [`ready_to_read`], for any fd (the SSH agent socket, Phase 27).
pub fn ready_to_read_fd(fd: std::os::fd::BorrowedFd<'_>, wait_ms: i32) -> io::Result<bool> {
    sys::wait_readable(fd, wait_ms)
}

/// The process group of the child (the child is the leader). Can be used from several
/// threads: one waits for the end, another kills the group when the CLI drops. When
/// dropped without having collected, kills the group (and the direct children with their
/// own session, Phase 54l) and collects (no process with a secret in the environment is
/// left behind).
#[derive(Debug)]
pub struct Group {
    pid: libc::pid_t,
    /// Already collected: the pid (and the group) may belong to another process; no
    /// `kill`.
    collected: Mutex<bool>,
    /// Alive after the last end of session (Phase 55).
    leftovers: AtomicUsize,
}

impl Group {
    /// Pid of the child (= id of the group).
    pub fn pid(&self) -> u32 {
        self.pid.unsigned_abs()
    }

    /// Waits for the child to end, without collecting: until collected, the group keeps
    /// this id.
    pub fn wait_for_end(&self) -> io::Result<()> {
        sys::wait_without_collecting(self.pid)
    }

    /// `SIGKILL` to the whole group (the child and what it created in the same group).
    /// After collected, does nothing.
    pub fn kill(&self) {
        let collected = self.collected.lock().unwrap_or_else(|e| e.into_inner());
        if !*collected {
            let _ = sys::kill_group(self.pid);
        }
    }

    /// Like [`Group::kill`], for the whole session (Phase 55, S8; before, Phase 54c):
    /// every descendant of the leader, including the one that left the group (`setpgid`)
    /// or the session (`setsid`, like the `ssh` that `sshpass` creates), and, in `wardd`
    /// (subreaper), the orphans of the session and the ownerless ones
    /// ([`crate::descendants`]). Everything gets `SIGSTOP` before `SIGKILL`; an outside
    /// process (another uid, another session that did not come out of a daemon session)
    /// gets nothing. After collected, does nothing. What was left alive goes to
    /// [`Group::remaining`].
    pub fn kill_with_child_sessions(&self) -> descendants::Report {
        let collected = self.collected.lock().unwrap_or_else(|e| e.into_inner());
        if *collected {
            return descendants::Report::default();
        }
        let r = descendants::kill_session(Some(self.pid));
        self.leftovers.store(r.remaining, Ordering::SeqCst);
        r
    }

    /// How many processes of the session were still alive after the last
    /// [`Group::kill_with_child_sessions`]. The CLI only hears the "ended" notice with 0.
    pub fn remaining(&self) -> usize {
        self.leftovers.load(Ordering::SeqCst)
    }

    /// Waits for the end and collects. Only once.
    pub fn collect(&self) -> io::Result<End> {
        self.wait_for_end()?;
        let mut collected = self.collected.lock().unwrap_or_else(|e| e.into_inner());
        if *collected {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        // Phase 55: leaves the leaders registry together with the collection (the pid may
        // go to another process right after).
        let mut leaders = descendants::record();
        let status = sys::collect(self.pid)?;
        leaders.retain(|p| *p != self.pid);
        drop(leaders);
        *collected = true;
        if libc::WIFSIGNALED(status) {
            Ok(End::Signal(libc::WTERMSIG(status)))
        } else {
            Ok(End::Code(
                u8::try_from(libc::WEXITSTATUS(status) & 0xff).unwrap_or(255),
            ))
        }
    }
}

impl Drop for Group {
    fn drop(&mut self) {
        let collected = *self.collected.lock().unwrap_or_else(|e| e.into_inner());
        if !collected {
            // Phase 54l/55: the whole session, not only the group.
            self.kill_with_child_sessions();
            let _ = self.collect();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::os::fd::AsFd;
    use std::time::{Duration, Instant};

    fn sh(script: &str) -> Command {
        let mut c = Command::new(b"/bin/sh", b"/").unwrap();
        c.argument(b"-c").unwrap();
        c.argument(script.as_bytes()).unwrap();
        c
    }

    fn run(c: &Command) -> (End, Vec<u8>, Vec<u8>) {
        let (g, mut p) = c.launch().unwrap();
        drop(p.stdin.take());
        let mut out = Vec::new();
        let mut err = Vec::new();
        p.stdout.take().unwrap().read_to_end(&mut out).unwrap();
        p.stderr.take().unwrap().read_to_end(&mut err).unwrap();
        (g.collect().unwrap(), out, err)
    }

    /// Reads the pty master until the `EIO` of the end of the session (or the deadline).
    fn read_pty(p: &mut Pty, deadline: Duration) -> Vec<u8> {
        let end = Instant::now() + deadline;
        let mut out = Vec::new();
        let mut buf = [0u8; 4096];
        while Instant::now() < end {
            if !ready_to_read(&p.master, 100).unwrap() {
                continue;
            }
            match p.master.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => out.extend_from_slice(&buf[..n]),
            }
        }
        out
    }

    #[test]
    fn pty_controlling_terminal_window_and_input() {
        // The child is a session leader, has the slave as controlling terminal on 0–2,
        // the requested window holds from the `exec` and changes later (with `SIGWINCH`).
        let c = sh(
            "tty >/dev/null && echo T; [ \"$(ps -o sid= -p $$ | tr -d ' ')\" = $$ ] && echo S; \
             stty size; trap 'stty size' WINCH; echo pronto; read x || read x; echo \"lido:$x\"",
        );
        let (g, mut p) = c.launch_in_pty(24, 80).unwrap();
        let mut seen = Vec::new();
        let end = Instant::now() + Duration::from_secs(10);
        let mut buf = [0u8; 4096];
        while !String::from_utf8_lossy(&seen).contains("pronto") {
            assert!(Instant::now() < end, "{}", String::from_utf8_lossy(&seen));
            if ready_to_read(&p.master, 100).unwrap() {
                let n = p.master.read(&mut buf).unwrap();
                seen.extend_from_slice(&buf[..n]);
            }
        }
        p.set_window_size(40, 132).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        p.master.write_all(b"oi\n").unwrap();
        seen.extend(read_pty(&mut p, Duration::from_secs(10)));
        let s = String::from_utf8_lossy(&seen);
        assert!(s.contains("T\r\nS\r\n24 80"), "{s}");
        assert!(s.contains("40 132"), "{s}");
        assert!(s.contains("lido:oi"), "{s}");
        assert_eq!(g.collect().unwrap(), End::Code(0));
    }

    #[test]
    fn pty_killing_the_group_takes_the_session_and_the_environment_is_exact() {
        let mut c = sh("env; sleep 30 & sleep 30");
        c.variable(b"SO", b"isto").unwrap();
        let (g, mut p) = c.launch_in_pty(24, 80).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        g.kill();
        let out = read_pty(&mut p, Duration::from_secs(5));
        assert_eq!(g.collect().unwrap(), End::Signal(libc::SIGKILL));
        let s = String::from_utf8_lossy(&out);
        assert!(s.contains("SO=isto"), "{s}");
        // D60: nothing from this process's environment (`sh` adds PWD, SHLVL, `_`).
        for line in s.lines() {
            let name = line.split('=').next().unwrap();
            assert!(
                ["SO", "PWD", "SHLVL", "_", ""].contains(&name.trim()),
                "{s}"
            );
        }
    }

    #[test]
    fn exact_argv_cwd_and_environment() {
        let mut c = Command::new(b"/bin/sh", b"/tmp").unwrap();
        for a in [
            &b"-c"[..],
            b"printf '%s|' \"$0\" \"$1\" \"$X\"; pwd; env | sort",
            b"a b",
            b"$(nada)",
        ] {
            c.argument(a).unwrap();
        }
        c.variable(b"X", b"valor com espa\xc3\xa7o=e igual")
            .unwrap();
        let (end, out, _) = run(&c);
        assert_eq!(end, End::Code(0));
        let out = String::from_utf8(out).unwrap();
        assert!(
            out.starts_with("a b|$(nada)|valor com espaço=e igual|/tmp\n"),
            "{out}"
        );
        // D60: only what was passed (`sh` adds PWD, SHLVL and `_`).
        let names: Vec<&str> = out
            .lines()
            .skip(1)
            .map(|l| l.split('=').next().unwrap())
            .collect();
        for n in &names {
            assert!(
                ["X", "PWD", "SHLVL", "_", "OLDPWD"].contains(n),
                "variável herdada: {n}"
            );
        }
    }

    /// Phase 44 (D6, V6.14): the debugging pipe reaches only the child, on fds 3 and 4,
    /// with `/dev/null` on 0–2 and no other fd; the daemon's ends do not go to a second
    /// child launched while the first one lives (attack: another process inheriting the
    /// pipe and sending commands to Chrome).
    #[test]
    fn debugging_pipe_only_in_the_child_on_3_and_4() {
        let c = sh("read -r linha <&3; \
             printf '%s|%s|%s|' \"$linha\" \"$(readlink /proc/$$/fd/1)\" \"$(readlink /proc/$$/fd/0)\" >&4; \
             for f in /proc/$$/fd/*; do printf '%s ' \"${f##*/}\" >&4; done; printf '\\n' >&4; \
             read -r fim <&3");
        let (g, mut p) = c.launch_with_debugging().unwrap();
        // A second, ordinary child while the first one lives: none of its fds is the pipe.
        use std::os::unix::fs::MetadataExt;
        let inos = [
            p.commands.metadata().unwrap().ino(),
            p.responses.metadata().unwrap().ino(),
        ];
        let (end2, out2, _) = run(&sh(
            "for f in /proc/$$/fd/*; do printf '%s=%s ' \"${f##*/}\" \"$(readlink $f)\"; done",
        ));
        assert_eq!(end2, End::Code(0));
        let fds2 = String::from_utf8(out2).unwrap();
        for ino in inos {
            assert!(!fds2.contains(&format!("pipe:[{ino}]")), "{fds2}");
        }
        p.commands.write_all(b"ola\n").unwrap();
        let mut line = Vec::new();
        let mut b = [0u8; 1];
        while p.responses.read(&mut b).unwrap() == 1 && b[0] != b'\n' {
            line.push(b[0]);
        }
        let s = String::from_utf8(line).unwrap();
        let parts: Vec<&str> = s.split('|').collect();
        assert_eq!(parts[0], "ola");
        assert_eq!(parts[1], "/dev/null");
        assert_eq!(parts[2], "/dev/null");
        // The `for` glob opens the directory on the lowest free fd (5).
        let fds: Vec<u32> = parts[3]
            .split_whitespace()
            .map(|f| f.parse().unwrap())
            .collect();
        for f in &fds {
            assert!(*f <= 5, "fd {f} a mais no filho: {s}");
        }
        for f in 0..=4 {
            assert!(fds.contains(&f), "falta o fd {f}: {s}");
        }
        p.commands.write_all(b"fim\n").unwrap();
        drop(p);
        assert_eq!(g.collect().unwrap(), End::Code(0));
    }

    /// D60: nothing from this process's environment (HOME, PATH, CARGO_*; in the daemon,
    /// `LISTEN_*`) reaches the child.
    #[test]
    fn attack_daemon_environment_does_not_reach_the_child() {
        let c = sh("env");
        let (_, out, _) = run(&c);
        let out = String::from_utf8(out).unwrap();
        for forbidden in ["LISTEN_", "HOME=", "PATH=/", "CARGO", "RUST"] {
            assert!(
                !out.contains(forbidden),
                "{forbidden} no ambiente do filho: {out}"
            );
        }
    }

    #[test]
    fn search_in_the_child_path_without_shell() {
        let mut c = Command::new(b"printf", b"/").unwrap();
        c.argument(b"%s").unwrap();
        c.argument(b"ok;echo injetado").unwrap();
        c.variable(b"PATH", b"/nao-existe:/usr/bin:/bin").unwrap();
        let (end, out, _) = run(&c);
        assert_eq!(end, End::Code(0));
        assert_eq!(out, b"ok;echo injetado");
        // Without PATH: the `execvpe` default (`/bin:/usr/bin`).
        let mut c = Command::new(b"true", b"/").unwrap();
        assert_eq!(run(&c).0, End::Code(0));
        c.variable(b"PATH", b"/nao-existe").unwrap();
        assert_eq!(c.launch().unwrap_err(), LaunchError::Program(libc::ENOENT));
    }

    #[test]
    fn failures_before_exec() {
        let c = Command::new(b"/nao/existe", b"/").unwrap();
        let e = c.launch().unwrap_err();
        assert_eq!(e, LaunchError::Program(libc::ENOENT));
        assert_eq!(e.exit_code(), 127);
        let c = Command::new(b"/bin/true", b"/nao/existe").unwrap();
        assert!(matches!(c.launch().unwrap_err(), LaunchError::Directory(_)));
        let c = Command::new(b"/etc/hostname", b"/").unwrap();
        let e = c.launch().unwrap_err();
        assert_eq!(e.exit_code(), 126, "{e:?}");
        assert_eq!(Command::new(b"", b"/").unwrap_err(), CommandError::Empty);
        assert_eq!(
            Command::new(b"a\0b", b"/").unwrap_err(),
            CommandError::NulByte
        );
        let mut c = Command::new(b"/bin/true", b"/").unwrap();
        assert_eq!(
            c.variable(b"A=B", b"x").unwrap_err(),
            CommandError::InvalidName
        );
        assert_eq!(
            c.variable(b"", b"x").unwrap_err(),
            CommandError::InvalidName
        );
        assert_eq!(
            c.variable(b"A", b"x\0y").unwrap_err(),
            CommandError::NulByte
        );
        assert_eq!(c.argument(b"\0").unwrap_err(), CommandError::NulByte);
    }

    #[test]
    fn code_and_signal() {
        assert_eq!(run(&sh("exit 7")).0, End::Code(7));
        let end = run(&sh("kill -TERM $$")).0;
        assert_eq!(end, End::Signal(libc::SIGTERM));
        assert_eq!(end.exit_code(), 143);
    }

    /// No fd of the daemon reaches the child, not even one without `CLOEXEC`.
    #[test]
    fn attack_daemon_fd_does_not_reach_the_child() {
        let f = File::open("/etc/hostname").unwrap();
        sys::clear_cloexec(f.as_fd()).unwrap();
        let (_, out, _) = run(&sh("ls /proc/self/fd"));
        let fds: Vec<String> = String::from_utf8(out)
            .unwrap()
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        // 0–2 and the fd `ls` opens to read the directory.
        assert!(fds.len() <= 4, "{fds:?}");
        for fd in ["0", "1", "2"] {
            assert!(fds.iter().any(|x| x == fd), "{fds:?}");
        }
        drop(f);
    }

    /// The Rust runtime ignores `SIGPIPE`; the child does not inherit that.
    #[test]
    fn signals_at_default() {
        let (_, out, _) = run(&sh("grep -E '^Sig(Ign|Blk):' /proc/self/status"));
        let out = String::from_utf8(out).unwrap();
        assert!(out.contains("SigBlk:\t0000000000000000"), "{out}");
        assert!(out.contains("SigIgn:\t0000000000000000"), "{out}");
    }

    /// M4 audit (the group-join script): the child without a pty is the leader of its own
    /// session; another process of the same user does not join its group by `setpgid`
    /// (the kernel refuses across sessions), so it does not pass the group check of the
    /// tunnel/jump bridge.
    #[test]
    fn attack_another_process_does_not_join_the_child_group() {
        let (g, mut p) = sh("echo pronto; sleep 30").launch().unwrap();
        let mut line = [0u8; 16];
        let _ = p.stdout.as_mut().unwrap().read(&mut line).unwrap();
        let pid = g.pid();
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
        let fields: Vec<&str> = stat[stat.rfind(')').unwrap() + 1..]
            .split_whitespace()
            .collect();
        // pgrp and session (4th and 5th fields after the name) are the child's pid.
        assert_eq!(fields[2], pid.to_string(), "{stat}");
        assert_eq!(fields[3], pid.to_string(), "{stat}");
        let attacker = std::process::Command::new("/usr/bin/python3")
            .args([
                "-I",
                "-c",
                "import os,sys\ntry:\n os.setpgid(0, int(sys.argv[1])); print('entrou')\nexcept OSError as e: print('recusado', e.errno)",
            ])
            .arg(pid.to_string())
            .output()
            .unwrap();
        let s = String::from_utf8_lossy(&attacker.stdout);
        assert_eq!(s.trim(), format!("recusado {}", libc::EPERM), "{s}");
        g.kill();
        assert_eq!(g.collect().unwrap(), End::Signal(libc::SIGKILL));
    }

    /// Killing the group also kills the grandchild the child left running.
    #[test]
    fn killing_the_group_takes_the_grandchildren() {
        let (g, mut p) = sh("sleep 60 & echo $!; wait").launch().unwrap();
        let mut line = [0u8; 32];
        let n = p.stdout.as_mut().unwrap().read(&mut line).unwrap();
        let grandchild: u32 = std::str::from_utf8(&line[..n])
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(std::path::Path::new(&format!("/proc/{grandchild}")).exists());
        g.kill();
        assert_eq!(g.collect().unwrap(), End::Signal(libc::SIGKILL));
        let end = Instant::now() + Duration::from_secs(5);
        // The grandchild becomes a zombie of init and goes away.
        while std::fs::read_to_string(format!("/proc/{grandchild}/stat"))
            .is_ok_and(|s| !s.contains(") Z "))
        {
            assert!(Instant::now() < end, "o neto continua vivo");
            std::thread::sleep(Duration::from_millis(20));
        }
        // After collected, kill does nothing (the pid may belong to another).
        g.kill();
    }

    /// Phase 54c (S8), attack: the child creates a process with `setsid()` (as `sshpass`
    /// does with `ssh`), outside the group. `kill` does not reach it (control);
    /// `kill_with_child_sessions` kills both. An outside process with the same uid, in
    /// another session, gets nothing.
    #[test]
    fn attack_child_with_own_session_dies_with_kill_with_child_sessions() {
        const CHILD: &str = "import os,sys,time\npid=os.fork()\nif pid==0:\n os.setsid()\n time.sleep(60)\n os._exit(0)\nprint(pid,flush=True)\nos.waitpid(pid,0)\n";
        let launch = || {
            let mut c = Command::new(b"/usr/bin/python3", b"/").unwrap();
            for a in [&b"-I"[..], b"-c", CHILD.as_bytes()] {
                c.argument(a).unwrap();
            }
            let (g, mut p) = c.launch().unwrap();
            let mut line = [0u8; 32];
            let n = p.stdout.as_mut().unwrap().read(&mut line).unwrap();
            let child: libc::pid_t = std::str::from_utf8(&line[..n])
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            let leader = libc::pid_t::try_from(g.pid()).unwrap();
            // Waits for the child's `setsid()`.
            let end = Instant::now() + Duration::from_secs(5);
            while sid_of(child) != Some(child) || ppid_of(child) != Some(leader) {
                assert!(Instant::now() < end, "o filho não abriu sessão própria");
                std::thread::sleep(Duration::from_millis(20));
            }
            (g, p, child)
        };
        let alive = |pid: libc::pid_t| {
            std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|s| !s.contains(") Z "))
        };
        let dies = |pid: libc::pid_t| {
            let end = Instant::now() + Duration::from_secs(5);
            while alive(pid) {
                if Instant::now() > end {
                    return false;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            true
        };
        // A stranger of the same uid, in another session, child of this test.
        let mut stranger = std::process::Command::new("/usr/bin/python3")
            .args(["-I", "-c", "import os,time\nos.setsid()\ntime.sleep(60)"])
            .spawn()
            .unwrap();
        let stranger_pid = libc::pid_t::try_from(stranger.id()).unwrap();

        // Control: `kill` leaves the child with its own session alive.
        let (g, _p, child) = launch();
        g.kill();
        assert_eq!(g.collect().unwrap(), End::Signal(libc::SIGKILL));
        std::thread::sleep(Duration::from_millis(300));
        assert!(alive(child), "matar alcançou o filho fora do grupo");
        let _ = sys::kill_group(child);
        assert!(dies(child));

        // With the fix: both die; the stranger does not.
        let (g, _p, child) = launch();
        let r = g.kill_with_child_sessions();
        assert_eq!((r.killed, r.remaining), (2, 0), "{r:?}");
        assert_eq!(g.remaining(), 0);
        assert_eq!(g.collect().unwrap(), End::Signal(libc::SIGKILL));
        assert!(dies(child), "o filho com sessão própria sobreviveu");
        assert!(alive(stranger_pid), "um processo de fora levou o sinal");
        // Collected: does nothing.
        assert_eq!(g.kill_with_child_sessions(), descendants::Report::default());
        let _ = stranger.kill();
        let _ = stranger.wait();
    }

    fn stat_field(pid: libc::pid_t, i: usize) -> Option<libc::pid_t> {
        let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        s[s.rfind(')')? + 1..]
            .split_whitespace()
            .nth(i)?
            .parse()
            .ok()
    }

    fn ppid_of(pid: libc::pid_t) -> Option<libc::pid_t> {
        stat_field(pid, 1)
    }

    fn sid_of(pid: libc::pid_t) -> Option<libc::pid_t> {
        stat_field(pid, 3)
    }

    /// Phase 55 (S8), 2nd step of the final re-audit: the grandchild leaves the group
    /// with `setpgid(0, 0)` (stays in the session) and also a great-grandchild with
    /// `setsid` whose parent is still alive. With the leader alive (the lock), `kill`
    /// does not reach them (control); `kill_with_child_sessions` takes the whole tree.
    #[test]
    fn attack_f55_grandchild_outside_the_group_dies_on_lock() {
        const CHILD: &str = "import os,time\nr,w=os.pipe()\npid=os.fork()\nif pid==0:\n os.setpgid(0,0)\n q=os.fork()\n if q==0:\n  os.setsid(); time.sleep(60); os._exit(0)\n os.write(w,(str(os.getpid())+' '+str(q)+'\\n').encode()); time.sleep(60); os._exit(0)\nprint(os.read(r,64).decode().strip(),flush=True)\ntime.sleep(60)\n";
        let launch = || {
            let mut c = Command::new(b"/usr/bin/python3", b"/").unwrap();
            for a in [&b"-I"[..], b"-c", CHILD.as_bytes()] {
                c.argument(a).unwrap();
            }
            let (g, mut p) = c.launch().unwrap();
            let mut line = [0u8; 64];
            let n = p.stdout.as_mut().unwrap().read(&mut line).unwrap();
            let v: Vec<libc::pid_t> = std::str::from_utf8(&line[..n])
                .unwrap()
                .split_whitespace()
                .map(|x| x.parse().unwrap())
                .collect();
            let (grandchild, great_grandchild) = (v[0], v[1]);
            let end = Instant::now() + Duration::from_secs(5);
            while sid_of(great_grandchild) != Some(great_grandchild) {
                assert!(Instant::now() < end, "o bisneto não abriu sessão");
                std::thread::sleep(Duration::from_millis(20));
            }
            (g, p, grandchild, great_grandchild)
        };
        let alive = |pid: libc::pid_t| {
            std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|s| !s.contains(") Z "))
        };
        let dies = |pid: libc::pid_t| {
            let end = Instant::now() + Duration::from_secs(5);
            while alive(pid) {
                if Instant::now() > end {
                    return false;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            true
        };
        // Control: the group dies, the grandchild (another group) and the great-grandchild
        // (another session) do not.
        let (g, _p, grandchild, great_grandchild) = launch();
        g.kill();
        assert_eq!(g.collect().unwrap(), End::Signal(libc::SIGKILL));
        std::thread::sleep(Duration::from_millis(300));
        assert!(alive(grandchild), "matar alcançou o neto fora do grupo");
        assert!(alive(great_grandchild));
        let _ = sys::kill_group(grandchild);
        let _ = sys::kill_group(great_grandchild);
        assert!(dies(grandchild) && dies(great_grandchild));

        // With the fix: all three die, nothing is left.
        let (g, _p, grandchild, great_grandchild) = launch();
        let r = g.kill_with_child_sessions();
        assert_eq!((r.killed, r.remaining), (3, 0), "{r:?}");
        assert_eq!(g.collect().unwrap(), End::Signal(libc::SIGKILL));
        assert!(dies(grandchild), "o neto fora do grupo sobreviveu");
        assert!(
            dies(great_grandchild),
            "o bisneto com sessão própria sobreviveu"
        );
    }

    /// `PR_SET_PDEATHSIG`: the thread that created the child dies, the child gets
    /// `SIGKILL`.
    #[test]
    fn child_dies_with_the_thread_that_created_it() {
        let pid = std::thread::spawn(|| {
            let (g, p) = sh("sleep 60").launch().unwrap();
            let pid = g.pid;
            drop(p);
            std::mem::forget(g);
            pid
        })
        .join()
        .unwrap();
        let status = sys::collect(pid).unwrap();
        assert!(libc::WIFSIGNALED(status));
        assert_eq!(libc::WTERMSIG(status), libc::SIGKILL);
    }

    #[test]
    fn input_reaches_the_child() {
        let (g, mut p) = sh("cat").launch().unwrap();
        let mut e = p.stdin.take().unwrap();
        e.write_all(b"linha\n").unwrap();
        drop(e);
        let mut out = Vec::new();
        p.stdout.take().unwrap().read_to_end(&mut out).unwrap();
        assert_eq!(out, b"linha\n");
        assert_eq!(g.collect().unwrap(), End::Code(0));
    }

    /// Phase 28g: the output and error pipes have an enlarged capacity (a single `write`
    /// up to that size goes in whole before the daemon reads), unless the kernel refuses
    /// (the user's page limit, which stays at the 64 KiB default).
    #[test]
    fn output_pipes_with_enlarged_capacity() {
        let (g, mut p) = sh("cat >/dev/null").launch().unwrap();
        for f in [p.stdout.as_ref().unwrap(), p.stderr.as_ref().unwrap()] {
            let c = sys::pipe_capacity(f.as_fd()).unwrap();
            assert!(
                c >= OUTPUT_PIPE_CAPACITY || c == 64 * 1024,
                "capacidade inesperada: {c}"
            );
        }
        // The input is not enlarged (only the output is masked).
        let e = sys::pipe_capacity(p.stdin.as_ref().unwrap().as_fd()).unwrap();
        assert_eq!(e, 64 * 1024);
        drop(p.stdin.take());
        assert_eq!(g.collect().unwrap(), End::Code(0));
    }

    /// Does the kernel accept enlarging a new pipe now?
    fn kernel_enlarges_pipe() -> bool {
        let (r, _w) = std::io::pipe().unwrap();
        sys::widen_pipe(r.as_fd(), OUTPUT_PIPE_CAPACITY).is_some_and(|c| c >= OUTPUT_PIPE_CAPACITY)
    }

    /// Phase 28g, attack of the 2nd M4 audit: a single `write` of `n` filler bytes + the
    /// password, with `n` around the 16 KiB and 64 KiB boundaries, reaches the reader
    /// whole in a single read (with the buffer the size of the capacity).
    #[test]
    fn attack_single_write_above_64_kib_arrives_whole() {
        let mut cases: Vec<usize> = (16_370..16_390).collect();
        cases.extend(65_520..65_540);
        cases.extend([131_072 - 5, 200_000, OUTPUT_PIPE_CAPACITY - 40]);
        for n in cases {
            let script =
                format!("import os\nos.write(1, b'b' * {n} + b'falso-28g-senha-longa\\n')\n");
            let mut c = Command::new(b"/usr/bin/python3", b"/").unwrap();
            c.argument(b"-c").unwrap();
            c.argument(script.as_bytes()).unwrap();
            let (g, mut p) = c.launch().unwrap();
            drop(p.stdin.take());
            let stdout = p.stdout.take().unwrap();
            if sys::pipe_capacity(stdout.as_fd()).unwrap() < OUTPUT_PIPE_CAPACITY {
                // Only acceptable if the kernel refuses the enlargement now (the user's
                // page limit); if it accepts on a new pipe, the launch did not enlarge.
                let _ = g.collect();
                assert!(
                    !kernel_enlarges_pipe(),
                    "o pipe de saída do filho não foi ampliado"
                );
                return;
            }
            assert!(ready_to_read(&stdout, 10_000).unwrap());
            let mut buf = vec![0u8; OUTPUT_PIPE_CAPACITY];
            let k = (&stdout).read(&mut buf).unwrap();
            assert_eq!(k, n + 22, "n={n}: o write chegou cortado");
            assert_eq!(g.collect().unwrap(), End::Code(0));
        }
    }

    /// `Debug` shows neither argv nor environment.
    #[test]
    fn debug_without_value() {
        let mut c = Command::new(b"/bin/true", b"/").unwrap();
        c.variable(b"SENHA", b"falso-PG:s3nha/01@#").unwrap();
        let d = format!("{c:?}");
        assert!(!d.contains("falso") && !d.contains("SENHA"), "{d}");
    }
}
