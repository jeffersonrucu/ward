//! Support for the integration tests of `ward-platform`. Nothing here touches the real
//! vault: the test directories stay under `/run/user/<uid>/` (tmpfs, the same place as
//! the real socket, so the same directory rules apply; see [`TestDir`]) and are deleted
//! at the end; binary copies stay in `CARGO_TARGET_TMPDIR`.

#![allow(dead_code)]

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use ward_platform::AllowedBinary;

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Directory of this test only inside `/run/user/<uid>/`, deleted on drop.
pub struct TestDir {
    pub path: PathBuf,
}

impl TestDir {
    /// Path not created yet.
    pub fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = ward_platform::socket::runtime_dir()
            .join(format!("ward-platform-teste-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        TestDir { path }
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// The system python, by the real file (`/usr/bin/python3` is a link).
pub fn real_python() -> PathBuf {
    std::fs::canonicalize("/usr/bin/python3").expect("python3 do sistema")
}

pub fn allowed(path: &Path) -> AllowedBinary {
    AllowedBinary::new(path.to_path_buf()).unwrap()
}

/// Python client that connects to `socket`, writes a ready line and waits for stdin to
/// close. `library`: loaded by `ctypes` before connecting. Clean environment (cargo
/// puts `LD_LIBRARY_PATH` in the environment of the tests) plus `extra`.
pub const CLIENT_PY: &str = r#"
import socket, sys
if len(sys.argv) > 2:
    import ctypes
    ctypes.CDLL(sys.argv[2])
s = socket.socket(socket.AF_UNIX)
s.connect(sys.argv[1])
sys.stdout.write("conectado\n"); sys.stdout.flush()
sys.stdin.read()
"#;

pub struct TestChild {
    pub child: Child,
    pub output: BufReader<ChildStdout>,
}

impl TestChild {
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Waits for the line `expected` on the child's stdout.
    pub fn wait_for(&mut self, expected: &str) {
        let mut line = String::new();
        loop {
            line.clear();
            let n = self.output.read_line(&mut line).unwrap();
            assert!(n > 0, "filho saiu sem escrever {expected:?}");
            if line.trim() == expected {
                return;
            }
        }
    }

    /// Closes stdin (the child exits) and waits.
    pub fn finish(mut self) {
        drop(self.child.stdin.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Builds the `Command` with a clean environment.
pub fn clean_command(program: &Path) -> Command {
    let mut c = Command::new(program);
    c.env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    c
}

pub fn start(mut c: Command) -> TestChild {
    let mut child = spawn_with_retry(&mut c);
    let output = BufReader::new(child.stdout.take().unwrap());
    TestChild { child, output }
}

/// `spawn` that retries on `ETXTBSY` ("Text file busy"). A freshly copied binary (the
/// `fs::copy` of the attacks) may have its write fd open in a child that another test
/// thread created by `fork` and that has not reached `execve` yet (`O_CLOEXEC` only
/// closes at `execve`); in that window, running the copy gives `ETXTBSY`. The window is
/// short: up to 100 attempts with 20 ms between them. Any other error fails right away.
pub fn spawn_with_retry(c: &mut Command) -> Child {
    let mut attempts = 0;
    loop {
        match c.spawn() {
            Ok(f) => return f,
            Err(e) if e.raw_os_error() == Some(libc::ETXTBSY) && attempts < 100 => {
                attempts += 1;
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(e) => panic!("subir filho: {e}"),
        }
    }
}

/// Copies a binary to `destination` and syncs the copy before closing the write fd
/// (the `ETXTBSY` that is left comes from an fd inherited by `fork`; see
/// [`spawn_with_retry`]).
pub fn copy_binary(origin: &Path, destination: &Path) {
    std::fs::copy(origin, destination).unwrap();
    std::fs::File::open(destination)
        .unwrap()
        .sync_all()
        .unwrap();
}

/// `python -I -c CLIENT_PY <socket> [library]` with the given `program`.
pub fn python_client(program: &Path, socket: &Path, library: Option<&Path>) -> Command {
    let mut c = clean_command(program);
    c.arg("-I").arg("-c").arg(CLIENT_PY).arg(socket);
    if let Some(b) = library {
        c.arg(b);
    }
    c
}
