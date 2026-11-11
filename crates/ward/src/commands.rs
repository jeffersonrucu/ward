//! The CLI commands (Phase 25): list (`--json` too), lock and unlock; and `run`
//! (Phase 26).
//!
//! Order of the unlock (S5, D46, V4.2):
//! 1. connects and checks the server; refused, it exits without reading the password and
//!    without sending anything;
//! 2. asks for the state (no secret); already unlocked or no vault, it exits without
//!    reading;
//! 3. hardens the process (non-dumpable, no core) and only then reads the password from
//!    the terminal;
//! 4. checks the server again on the same socket and only then sends the password.

use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ward_core::entry::Secret;
use ward_core::limits::{KIB, MAX_ARGUMENT, MAX_ARGUMENTS, MAX_ENV_VARIABLES, MAX_SECRET};
use ward_core::protocol::{
    EncodeError, Request, Response, ResponseError, Role, RunRequest, StdinKind, Stream, Variable,
    WindowSize,
};
use ward_platform::connect::ChannelError;
use ward_platform::{ExpectedServer, TerminalError};
use zeroize::Zeroizing;

use crate::args::{Command, HELP};
use crate::client::{Connection, ConnectionError, Session, SessionWriter};
use crate::text;

/// Exit code: success.
pub const EXIT_OK: u8 = 0;
/// Exit code: error (daemon, server, locked, password).
pub const EXIT_ERROR: u8 = 1;
/// Exit code: wrong usage.
pub const EXIT_USAGE: u8 = 2;
/// Exit code of `ward run` when the error is Ward's, not the command's (like `env` and
/// `docker run`): daemon, server, locked, denied, type without delivery.
pub const EXIT_RUN_ERROR: u8 = 125;

/// D52: argv + environment of `ward run` fit in the 1 MiB frame after base64.
pub const MAX_RUN: usize = 750 * KIB;
/// D52: the message of the limit, without any value.
pub const MSG_RUN_TOO_LARGE: &str =
    "o comando e o ambiente passam de 750 KiB; reduza o ambiente (ex.: `env -i`)";
/// Message of an argument or variable above the limit of one field.
pub const MSG_RUN_FIELD: &str = "um argumento ou uma variável do ambiente passa de 128 KiB, \
     ou há argumentos ou variáveis demais; reduza o ambiente (ex.: `env -i`)";
/// Notice for when the vault is locked: the request waits for the unlock (D16).
pub const MSG_RUN_LOCKED: &str = "cofre travado: o comando espera o destrave por até 2 min \
     (`/usr/local/bin/ward destravar` em outro terminal, ou pelo app)";
/// Chunk of the standard input per frame.
const STDIN_CHUNK: usize = 64 * KIB;

/// What the commands use from the outside world (the tests swap it).
pub trait Env {
    /// Connects and checks the server.
    fn connect(&mut self) -> Result<Connection, ConnectionError>;
    /// Turns off the process dump before the password exists in memory.
    fn harden(&mut self) -> io::Result<()>;
    /// Reads the master password from the terminal.
    fn read_password(&mut self) -> Result<Zeroizing<Vec<u8>>, TerminalError>;
    fn stdout(&mut self) -> &mut dyn Write;
    fn stderr(&mut self) -> &mut dyn Write;
    /// Environment of the CLI, for `ward run` (D13).
    fn environment(&self) -> Vec<(OsString, OsString)> {
        std::env::vars_os().collect()
    }
    /// Current directory, for `ward run`.
    fn current_dir(&self) -> io::Result<PathBuf> {
        std::env::current_dir()
    }
    /// Standard input, forwarded to the command of `ward run`.
    fn stdin(&mut self) -> Box<dyn Read + Send> {
        Box::new(io::stdin())
    }
    /// Phase 54b (D68): what the standard input is (`fstat` of fd 0), so the daemon
    /// warns in the window when the command receives data the window does not show.
    fn stdin_kind(&self) -> StdinKind {
        detect_stdin_kind()
    }
    /// Interactive `ward ssh` (Phase 27): the terminal size, if standard input and
    /// output are terminals; `None` without a terminal (pipe, file).
    fn terminal(&self) -> Option<WindowSize> {
        if !ward_platform::terminal::is_terminal(io::stdin())
            || !ward_platform::terminal::is_terminal(io::stdout())
        {
            return None;
        }
        Some(window_from(ward_platform::terminal::size(io::stdin())))
    }
    /// The terminal size now (the thread that follows the window asks again).
    fn terminal_size(&self) -> Box<dyn Fn() -> Option<(u16, u16)> + Send> {
        Box::new(|| ward_platform::terminal::size(io::stdin()))
    }
    /// Puts the input terminal in raw mode; it goes back when the value is dropped.
    fn raw_mode(&mut self) -> Option<ward_platform::RawMode> {
        ward_platform::RawMode::enable(io::stdin()).ok()
    }
    /// `ward ssh --token-fd <n>` (Phase 27a): reads the token of the Connect button from the fd
    /// inherited from the app and closes the fd.
    fn read_token(&mut self, fd: u32) -> Result<Zeroizing<Vec<u8>>, ChannelError> {
        ward_platform::connect::read_inherited_token(fd)
    }
}

/// `/dev/null` (`makedev(1, 3)` on Linux).
const DEV_NULL: u64 = (1 << 8) | 3;

/// Phase 54b (D68): the kind of the standard input by the `fstat` of fd 0 (a copy of
/// the fd, without `unsafe`). Closed (no fd 0) counts as null.
pub fn detect_stdin_kind() -> StdinKind {
    use std::io::IsTerminal;
    use std::os::fd::AsFd;
    let meta = io::stdin()
        .as_fd()
        .try_clone_to_owned()
        .map(std::fs::File::from)
        .and_then(|f| f.metadata());
    classify_stdin(meta.as_ref().ok(), io::stdin().is_terminal())
}

/// Phase 54b: the classification, separate for the tests. `None`: the `fstat` failed
/// (fd 0 closed).
pub fn classify_stdin(meta: Option<&std::fs::Metadata>, terminal: bool) -> StdinKind {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    let Some(m) = meta else {
        return StdinKind::Null;
    };
    let t = m.file_type();
    if t.is_char_device() && m.rdev() == DEV_NULL {
        StdinKind::Null
    } else if terminal && t.is_char_device() {
        StdinKind::Terminal
    } else if t.is_fifo() || t.is_socket() {
        StdinKind::Pipe
    } else if t.is_file() {
        StdinKind::File
    } else {
        StdinKind::Other
    }
}

/// Terminal size for the protocol (24×80 when the terminal does not say).
fn window_from(t: Option<(u16, u16)>) -> WindowSize {
    let (rows, columns) = t.unwrap_or((24, 80));
    WindowSize {
        rows: rows.clamp(1, 10_000),
        cols: columns.clamp(1, 10_000),
    }
}

/// The real environment: socket of the [`ExpectedServer`], `/dev/tty`, stdout and
/// stderr.
pub struct System {
    pub expected: ExpectedServer,
    pub stdout: io::Stdout,
    pub stderr: io::Stderr,
}

impl Env for System {
    fn connect(&mut self) -> Result<Connection, ConnectionError> {
        Connection::connect(&self.expected)
    }
    fn harden(&mut self) -> io::Result<()> {
        ward_platform::harden_process()
    }
    fn read_password(&mut self) -> Result<Zeroizing<Vec<u8>>, TerminalError> {
        ward_platform::read_password("Senha mestra do Ward: ", MAX_SECRET)
    }
    fn stdout(&mut self) -> &mut dyn Write {
        &mut self.stdout
    }
    fn stderr(&mut self) -> &mut dyn Write {
        &mut self.stderr
    }
}

fn fail(e: &mut dyn Env, msg: &str) -> u8 {
    let _ = writeln!(e.stderr(), "ward: {msg}");
    EXIT_ERROR
}

/// Message of a daemon error, with the hint of what to do when there is one.
pub fn daemon_message(error: &ResponseError) -> String {
    match error {
        // Phase 54a (RA8): by the full path, not by the `ward` of the `PATH`.
        ResponseError::Locked => {
            "travado: destrave com `/usr/local/bin/ward destravar` ou pelo app".to_owned()
        }
        ResponseError::VaultNotFound => "ainda não existe cofre; crie pelo app".to_owned(),
        e => e.message().to_owned(),
    }
}

/// Runs a command and returns the exit code.
pub fn execute(cmd: Command, e: &mut dyn Env) -> u8 {
    match cmd {
        Command::Help => {
            let _ = e.stdout().write_all(HELP.as_bytes());
            EXIT_OK
        }
        Command::Version => {
            let _ = writeln!(e.stdout(), "ward {}", env!("CARGO_PKG_VERSION"));
            EXIT_OK
        }
        Command::List { json } => list(e, json),
        Command::Lock => lock(e),
        Command::Unlock => unlock(e),
        Command::Run { entry, argv } => run(e, entry, argv),
        Command::Ssh {
            entry,
            token_fd,
            command,
        } => ssh(e, entry, token_fd, command),
        Command::Playwright { entry } => playwright(e, entry),
        Command::Login { entry } => login(e, entry),
    }
}

fn request(e: &mut dyn Env, c: &mut Connection, p: &Request) -> Result<Response, u8> {
    match c.request(p) {
        Ok(Response::Error(error)) => Err(fail(e, &daemon_message(&error))),
        Ok(r) => Ok(r),
        Err(error) => Err(fail(e, &error.message())),
    }
}

fn connect(e: &mut dyn Env) -> Result<Connection, u8> {
    e.connect().map_err(|error| fail(e, &error.message()))
}

fn list(e: &mut dyn Env, json: bool) -> u8 {
    let mut c = match connect(e) {
        Ok(c) => c,
        Err(code) => return code,
    };
    match request(e, &mut c, &Request::List) {
        Ok(Response::List(items)) => {
            let out = if json {
                text::list_json(&items)
            } else {
                text::list_table(&items)
            };
            if e.stdout().write_all(out.as_bytes()).is_err() {
                return EXIT_ERROR;
            }
            EXIT_OK
        }
        Ok(_) => fail(e, "resposta inesperada do daemon"),
        Err(code) => code,
    }
}

fn lock(e: &mut dyn Env) -> u8 {
    let mut c = match connect(e) {
        Ok(c) => c,
        Err(code) => return code,
    };
    match request(e, &mut c, &Request::Lock) {
        Ok(Response::Ok) => {
            let _ = writeln!(e.stdout(), "cofre travado");
            EXIT_OK
        }
        Ok(_) => fail(e, "resposta inesperada do daemon"),
        Err(code) => code,
    }
}

fn unlock(e: &mut dyn Env) -> u8 {
    // 1. Server checked before anything else.
    let mut c = match connect(e) {
        Ok(c) => c,
        Err(code) => return code,
    };
    // 2. State, without secret.
    match request(e, &mut c, &Request::Status) {
        Ok(Response::Status {
            vault_exists: false,
            ..
        }) => return fail(e, &daemon_message(&ResponseError::VaultNotFound)),
        Ok(Response::Status { locked: false, .. }) => {
            let _ = writeln!(e.stdout(), "o cofre já está destravado");
            return EXIT_OK;
        }
        Ok(Response::Status { .. }) => {}
        Ok(_) => return fail(e, "resposta inesperada do daemon"),
        Err(code) => return code,
    }
    // 3. Process hardened before the password exists.
    if e.harden().is_err() {
        return fail(
            e,
            "não deu para proteger a memória do processo; nada foi lido",
        );
    }
    let mut read = match e.read_password() {
        Ok(s) => s,
        Err(TerminalError::NoTerminal) => {
            return fail(
                e,
                "`ward destravar` precisa de um terminal para ler a senha",
            );
        }
        Err(TerminalError::Canceled) => return fail(e, "cancelado; nada foi enviado"),
        Err(TerminalError::TooLong) => return fail(e, "senha longa demais; nada foi enviado"),
        Err(TerminalError::Io(_)) => return fail(e, "erro no terminal; nada foi enviado"),
    };
    if read.is_empty() {
        return fail(e, "senha vazia; nada foi enviado");
    }
    // 4. Server checked again, on the same socket, right before sending.
    if let Err(error) = c.reverify() {
        return fail(e, &error.message());
    }
    // The password moves from the `Zeroizing` to the `Secret` without a copy.
    let password = Secret::from_bytes(std::mem::take(&mut *read));
    drop(read);
    let r = request(e, &mut c, &Request::Unlock { password });
    match r {
        Ok(Response::Ok) => {
            let _ = writeln!(e.stdout(), "cofre destravado");
            EXIT_OK
        }
        Ok(_) => fail(e, "resposta inesperada do daemon"),
        Err(code) => code,
    }
}

/// D13: the CLI does not send `LD_*` (the daemon strips them again).
fn variable_sent(name: &[u8]) -> bool {
    !name.is_empty() && !name.starts_with(b"LD_") && !name.contains(&b'=') && !name.contains(&0)
}

/// The run request, or the message of why it does not fit (D52). Only sizes are
/// measured; no value goes into the message.
pub fn run_request(
    entry: String,
    argv: Vec<OsString>,
    cwd: PathBuf,
    environment: Vec<(OsString, OsString)>,
    stdin_kind: StdinKind,
) -> Result<Request, &'static str> {
    use std::os::unix::ffi::OsStringExt;
    let argv: Vec<Vec<u8>> = argv.into_iter().map(OsStringExt::into_vec).collect();
    let cwd = cwd.into_os_string().into_vec();
    let environment: Vec<(Vec<u8>, Vec<u8>)> = environment
        .into_iter()
        .map(|(k, v)| (k.into_vec(), v.into_vec()))
        .filter(|(k, _)| variable_sent(k))
        .collect();
    let total: usize = argv.iter().map(Vec::len).sum::<usize>()
        + cwd.len()
        + environment
            .iter()
            .map(|(k, v)| k.len() + v.len() + 1)
            .sum::<usize>();
    if total > MAX_RUN {
        return Err(MSG_RUN_TOO_LARGE);
    }
    let field_too_large = argv
        .iter()
        .chain(std::iter::once(&cwd))
        .any(|a| a.len() > MAX_ARGUMENT)
        || environment
            .iter()
            .any(|(k, v)| k.len() > MAX_ARGUMENT || v.len() > MAX_ARGUMENT);
    if field_too_large || argv.len() > MAX_ARGUMENTS || environment.len() > MAX_ENV_VARIABLES {
        return Err(MSG_RUN_FIELD);
    }
    let request = Request::Run(RunRequest {
        entry,
        argv: argv.into_iter().map(Zeroizing::new).collect(),
        cwd: Zeroizing::new(cwd),
        env: environment
            .into_iter()
            .map(|(k, v)| Variable {
                name: Zeroizing::new(k),
                value: Secret::from_bytes(v),
            })
            .collect(),
        stdin_kind,
    });
    // The whole frame (with the JSON names) also fits in the limit.
    match ward_core::protocol::encode_request(Role::Agent, &request) {
        Ok(_) => Ok(request),
        Err(EncodeError::TooLarge) => Err(MSG_RUN_TOO_LARGE),
        Err(_) => Err(MSG_RUN_FIELD),
    }
}

fn fail_run(e: &mut dyn Env, msg: &str) -> u8 {
    let _ = writeln!(e.stderr(), "ward: {msg}");
    EXIT_RUN_ERROR
}

/// `ward run <entry> -- <command…>` (Phase 26). The daemon runs the command with the
/// credential and returns the output already masked and the exit code; the CLI
/// forwards the standard input. No secret passes through here (S1).
fn run(e: &mut dyn Env, entry: String, argv: Vec<OsString>) -> u8 {
    let cwd = match e.current_dir() {
        Ok(d) => d,
        Err(_) => return fail_run(e, "não deu para ler o diretório atual"),
    };
    let environment = e.environment();
    let kind = e.stdin_kind();
    // D52: measured before connecting; nothing is executed.
    let request = match run_request(entry, argv, cwd, environment, kind) {
        Ok(p) => p,
        Err(msg) => return fail_run(e, msg),
    };
    let mut c = match e.connect() {
        Ok(c) => c,
        Err(error) => return fail_run(e, &error.message()),
    };
    match c.request(&Request::Status) {
        Ok(Response::Status {
            vault_exists: false,
            ..
        }) => return fail_run(e, &daemon_message(&ResponseError::VaultNotFound)),
        Ok(Response::Status { locked: true, .. }) => {
            let _ = writeln!(e.stderr(), "ward: {MSG_RUN_LOCKED}");
        }
        Ok(Response::Status { .. }) => {}
        Ok(Response::Error(error)) => return fail_run(e, &daemon_message(&error)),
        Ok(_) => return fail_run(e, "resposta inesperada do daemon"),
        Err(error) => return fail_run(e, &error.message()),
    }
    let (session, writer) = match c.open_session(&request) {
        Ok(s) => s,
        Err(error) => return fail_run(e, &error.message()),
    };
    drop(request);
    let writer = Arc::new(Mutex::new(writer));
    forward_stdin(e, writer, kind);
    receive_session(e, session)
}

/// The standard input goes to the command in a thread; at its end, an empty frame.
/// Phase 54b: declared null, nothing is read (fd 0 may even be another file, if it was
/// closed) and only the empty frame goes.
fn forward_stdin(e: &mut dyn Env, writer: Arc<Mutex<SessionWriter>>, kind: StdinKind) {
    let mut source = if kind == StdinKind::Null {
        Box::new(io::empty())
    } else {
        e.stdin()
    };
    let _ = std::thread::Builder::new()
        .name("ward-entrada".into())
        .spawn(move || {
            let mut buf = Zeroizing::new(vec![0u8; STDIN_CHUNK]);
            loop {
                let n = match source.read(&mut buf) {
                    Ok(n) => n,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => 0,
                };
                let data = Secret::from_bytes(buf[..n].to_vec());
                let mut w = writer.lock().unwrap_or_else(|e| e.into_inner());
                if w.send_stdin(data).is_err() || n == 0 {
                    return;
                }
            }
        });
}

/// Forwards the session output until the end frame and returns the code.
fn receive_session(e: &mut dyn Env, mut session: Session) -> u8 {
    loop {
        match session.receive() {
            Ok(Response::Output {
                stream: fluxo,
                data: dados,
            }) => {
                let dest = match fluxo {
                    Stream::Stdout => e.stdout(),
                    Stream::Stderr => e.stderr(),
                };
                // Output closed on the CLI side (`| head`): the rest is discarded.
                let _ = dest.write_all(&dados).and_then(|()| dest.flush());
            }
            Ok(Response::Exit { code: codigo }) => {
                let _ = e.stdout().flush();
                return codigo;
            }
            Ok(Response::Error(error)) => return fail_run(e, &daemon_message(&error)),
            Ok(_) => return fail_run(e, "resposta inesperada do daemon"),
            Err(error) => return fail_run(e, &error.message()),
        }
    }
}

/// How often the interactive `ward ssh` looks at the terminal size.
const WINDOW_STEP: Duration = Duration::from_millis(250);

/// The ssh request, or the message of why it does not fit (the same limits as
/// `ward run`, D52). No value goes into the message.
pub fn ssh_request(
    entry: String,
    token: Option<Secret>,
    command: Vec<OsString>,
    window: Option<WindowSize>,
    stdin_kind: StdinKind,
) -> Result<Request, &'static str> {
    use std::os::unix::ffi::OsStringExt;
    let command: Vec<Vec<u8>> = command.into_iter().map(OsStringExt::into_vec).collect();
    if command.iter().map(Vec::len).sum::<usize>() > MAX_RUN {
        return Err(MSG_RUN_TOO_LARGE);
    }
    if command.len() > MAX_ARGUMENTS || command.iter().any(|a| a.len() > MAX_ARGUMENT) {
        return Err(MSG_RUN_FIELD);
    }
    let request = Request::Ssh {
        entry,
        command: command.into_iter().map(Zeroizing::new).collect(),
        token,
        window_size: window,
        stdin_kind,
    };
    match ward_core::protocol::encode_request(Role::Agent, &request) {
        Ok(_) => Ok(request),
        Err(EncodeError::TooLarge) => Err(MSG_RUN_TOO_LARGE),
        Err(_) => Err(MSG_RUN_FIELD),
    }
}

/// Message of a channel token out of the daemon's format (without the content).
pub const MSG_TOKEN_FORMAT: &str = "o token do Conectar veio fora do formato";

/// The token of the Connect button through the inherited fd (Phase 27a). Before reading, the
/// process becomes non-dumpable and without core (the token starts to exist in its
/// memory). The fd closes on read, before any connection. The content never goes into a
/// message.
fn token_from_channel(e: &mut dyn Env, fd: u32) -> Result<Secret, &'static str> {
    e.harden()
        .map_err(|_| "não foi possível proteger o processo antes de ler o token")?;
    let mut bytes = e.read_token(fd).map_err(ChannelError::message)?;
    if !crate::args::token_ok(&bytes) {
        return Err(MSG_TOKEN_FORMAT);
    }
    Ok(Secret::from_bytes(std::mem::take(&mut *bytes)))
}

/// `ward ssh [--token-fd <n>] <entry> [command…]` (Phases 27 and 27a). The daemon
/// connects with the credential (password by `sshpass`, key by the temporary agent of
/// D12) and returns the output already masked and the code. Without a command and with a
/// terminal: remote shell in a pty, with the local terminal in raw mode and the size
/// followed. No secret passes through here (S1). The token of the Connect button (D3) arrives
/// through an fd inherited from the app, never through argv or environment, and is read
/// and the fd closed before connecting.
fn ssh(e: &mut dyn Env, entry: String, token_fd: Option<u32>, command: Vec<OsString>) -> u8 {
    let token = match token_fd.map(|fd| token_from_channel(e, fd)).transpose() {
        Ok(t) => t,
        Err(msg) => return fail_run(e, msg),
    };
    let window = if command.is_empty() {
        e.terminal()
    } else {
        None
    };
    let kind = e.stdin_kind();
    let request = match ssh_request(entry, token, command, window, kind) {
        Ok(p) => p,
        Err(msg) => return fail_run(e, msg),
    };
    let mut c = match e.connect() {
        Ok(c) => c,
        Err(error) => return fail_run(e, &error.message()),
    };
    match c.request(&Request::Status) {
        Ok(Response::Status {
            vault_exists: false,
            ..
        }) => return fail_run(e, &daemon_message(&ResponseError::VaultNotFound)),
        Ok(Response::Status { locked: true, .. }) => {
            let _ = writeln!(e.stderr(), "ward: {MSG_RUN_LOCKED}");
        }
        Ok(Response::Status { .. }) => {}
        Ok(Response::Error(error)) => return fail_run(e, &daemon_message(&error)),
        Ok(_) => return fail_run(e, "resposta inesperada do daemon"),
        Err(error) => return fail_run(e, &error.message()),
    }
    let (session, writer) = match c.open_session(&request) {
        Ok(s) => s,
        Err(error) => return fail_run(e, &error.message()),
    };
    drop(request);
    let writer = Arc::new(Mutex::new(writer));
    // Interactive: raw terminal while the session lasts (back on `Drop`, even on error).
    let raw = window.and_then(|_| e.raw_mode());
    let done = Arc::new(AtomicBool::new(false));
    if let Some(initial) = window {
        track_window(e, initial, writer.clone(), done.clone());
    }
    forward_stdin(e, writer, kind);
    let code = receive_session(e, session);
    done.store(true, Ordering::Release);
    drop(raw);
    code
}

/// `ward playwright <entry>` (Phase 44, D6): the daemon logs in on Chrome and returns
/// only the path of the storage state, which goes to the output. No secret passes
/// through here (S1).
fn playwright(e: &mut dyn Env, entry: String) -> u8 {
    let mut c = match connect(e) {
        Ok(c) => c,
        Err(code) => return code,
    };
    match c.request(&Request::Status) {
        Ok(Response::Status {
            vault_exists: false,
            ..
        }) => return fail(e, &daemon_message(&ResponseError::VaultNotFound)),
        Ok(Response::Status { locked: true, .. }) => {
            let _ = writeln!(e.stderr(), "ward: {MSG_RUN_LOCKED}");
        }
        Ok(Response::Status { .. }) => {}
        Ok(Response::Error(error)) => return fail(e, &daemon_message(&error)),
        Ok(_) => return fail(e, "resposta inesperada do daemon"),
        Err(error) => return fail(e, &error.message()),
    }
    // Waits for the unlock and the approval (2 min each, D16, S10) plus the browser.
    match c.request_with_timeout(&Request::Playwright { entry }, PLAYWRIGHT_TIMEOUT) {
        Ok(Response::PlaywrightSession { path: caminho })
            if caminho.starts_with('/') && !caminho.chars().any(char::is_control) =>
        {
            if writeln!(e.stdout(), "{caminho}").is_err() {
                return EXIT_ERROR;
            }
            EXIT_OK
        }
        Ok(Response::Error(error)) => fail(e, &daemon_message(&error)),
        Ok(_) => fail(e, "resposta inesperada do daemon"),
        Err(error) => fail(e, &error.message()),
    }
}

/// `ward login <entry>` (Phase 43): the daemon takes the request to the Chrome
/// extension, which checks the origin of the active tab (D9), fills, submits and clears
/// the fields. Only `ok` or the error comes back; no secret passes through here (S1).
fn login(e: &mut dyn Env, entry: String) -> u8 {
    let mut c = match connect(e) {
        Ok(c) => c,
        Err(code) => return code,
    };
    match c.request(&Request::Status) {
        Ok(Response::Status {
            vault_exists: false,
            ..
        }) => return fail(e, &daemon_message(&ResponseError::VaultNotFound)),
        Ok(Response::Status { locked: true, .. }) => {
            let _ = writeln!(e.stderr(), "ward: {MSG_RUN_LOCKED}");
        }
        Ok(Response::Status { .. }) => {}
        Ok(Response::Error(error)) => return fail(e, &daemon_message(&error)),
        Ok(_) => return fail(e, "resposta inesperada do daemon"),
        Err(error) => return fail(e, &error.message()),
    }
    match c.request_with_timeout(
        &Request::Login {
            entry: entry.clone(),
        },
        PLAYWRIGHT_TIMEOUT,
    ) {
        Ok(Response::Ok) => {
            if writeln!(e.stdout(), "login enviado: {entry} (aba ativa do Chrome)").is_err() {
                return EXIT_ERROR;
            }
            EXIT_OK
        }
        Ok(Response::Error(error)) => fail(e, &login_message(&error)),
        Ok(_) => fail(e, "resposta inesperada do daemon"),
        Err(error) => fail(e, &error.message()),
    }
}

/// Message of a `ward login` error: the origin and the extension get their own text.
pub fn login_message(error: &ResponseError) -> String {
    match error {
        ResponseError::OriginMismatch => {
            "domínio não confere: a origem da aba ativa não é a da entrada; nada foi preenchido"
                .to_owned()
        }
        ResponseError::BrowserFailed => {
            "a extensão do Ward no Chrome não respondeu (o Chrome está aberto?)".to_owned()
        }
        e => daemon_message(e),
    }
}

/// Time limit of the `ward playwright` response: unlock (2 min) + approval (2 min) + the
/// browser (1 min) and slack.
const PLAYWRIGHT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(330);

/// Sends the new terminal size when it changes (without a signal handler: it looks
/// every [`WINDOW_STEP`]).
fn track_window(
    e: &mut dyn Env,
    initial: WindowSize,
    writer: Arc<Mutex<SessionWriter>>,
    done: Arc<AtomicBool>,
) {
    let size = e.terminal_size();
    let _ = std::thread::Builder::new()
        .name("ward-janela".into())
        .spawn(move || {
            let mut current = initial;
            while !done.load(Ordering::Acquire) {
                std::thread::sleep(WINDOW_STEP);
                let new_size = window_from(size());
                if new_size != current {
                    current = new_size;
                    let mut w = writer.lock().unwrap_or_else(|e| e.into_inner());
                    if w.send_window(new_size).is_err() {
                        return;
                    }
                }
            }
        });
}
