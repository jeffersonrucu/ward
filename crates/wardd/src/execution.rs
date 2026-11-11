//! `ward run` (Phase 26; D10, D11, D13, D16, D52, D60, D63; S1–S4, S8, S13).
//!
//! One session per connection of the agent role:
//! 1. a visible secure note answers "type without delivery" before any question;
//! 2. the core's use authorization: wait for the unlock (D16), level, approval (Phase 23);
//! 3. with the state locked, the delivery of D10 (built by ward-core):
//!    attachments and key in `<rt>tmp/<session>/`, 0600, in a 0700 dir of the session
//!    only (S2); and the mask of the entry and of the delivery (D11);
//! 4. the child's environment is built from scratch (Phase 26a; D13, D60): fixed `PATH`
//!    ([`COMMAND_PATH`], which also resolves a command without `/`), `HOME`, `USER` and
//!    `LOGNAME` from passwd and, from the CLI, only the list of [`FROM_CLI`] with the
//!    value verified (nothing that becomes a path); nothing from the daemon's
//!    environment (`LISTEN_*`); the delivery on top; locale with a UTF-8 `LC_CTYPE` (see
//!    [`adjust_locale`]). The working directory is the CLI's and goes to the approval
//!    window;
//! 5. the child ([`ward_platform::child`]): its own process group, no fd of the daemon,
//!    no shell (S3), dies with the connection's thread;
//! 6. stdout and stderr go through the mask, each in its own stream, and come back in
//!    output frames; the CLI's input arrives through stdin frames (empty = end);
//! 7. the child ends: the whole group is killed, the temporary is zeroed and deleted and
//!    the CLI receives the exit frame with the code (128 + signal, like the shell);
//! 8. the CLI goes away (end of the connection) or sends a frame outside the session:
//!    the child's group gets `SIGKILL` (D13) and the cleanup goes on the same way;
//! 9. Phase 54b (D68): with the standard input declared null, the child's input closes
//!    at launch and a stdin frame with data counts as a frame outside the session.
//!
//! The log has only numbers and fixed texts (S13). The temporaries a dead `wardd` left
//! behind are deleted at startup ([`clean_temp_files`]).

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use zeroize::Zeroizing;

use ward_core::delivery::{self, Delivery, DeliveryError, ResourceError, Resources};
use ward_core::entry::{AccessLevel, Data, Entry, Secret};
use ward_core::limits::{MAX_COMMON_FRAME, MAX_DISPLAYED_TEXT};
use ward_core::mask::{Mask, MaskBuilder};
use ward_core::protocolo::{
    self, ErroResposta, Fluxo, Janela, Papel, Pedido, Resposta, TipoEntradaPadrao, Variavel, quadro,
};
use ward_core::totp::Totp;
use ward_platform::child::{self, Command, Group, Pty};

use crate::ssh_agent::{Allowed, SshAgent};

use crate::aprovacao::Cliente;
use crate::nucleo::Daemon;
use crate::ponte::{Abertura, LIMITES_TUNEL, Ponte};
use crate::{aviso, depurar};

/// Dir of the temporaries inside `<rt>`.
pub const TEMP_DIR: &str = "tmp";
/// After the child ends, how long to wait for the output threads to deliver the rest.
const REST_OF_OUTPUT_DEADLINE: Duration = Duration::from_secs(2);
/// How often the output reader checks whether it must stop.
const READ_STEP_MS: i32 = 100;
/// Largest batch read from the child's output before masking (Phase 28g): the capacity
/// of the output pipes, so a single `write` that fit in the pipe is masked at once.
pub(crate) const BATCH: usize = child::OUTPUT_PIPE_CAPACITY;
/// Input frames kept waiting for the child to read (back pressure on the CLI).
const STDIN_QUEUE: usize = 8;
/// Locale the child gets when the CLI's is not UTF-8.
const LOCALE_UTF8: &[u8] = b"C.UTF-8";

static SESSIONS: AtomicU64 = AtomicU64::new(0);

// --- Temporaries (S2) -----------------------------------------------------------------

/// A file of the session, to verify when zeroing it.
struct SessionFile {
    path: PathBuf,
    dev: u64,
    ino: u64,
    size: u64,
}

/// The 0700 dir of a session in `<rt>tmp/`, with the 0600 attachments. On drop
/// (panic included), each file is zeroed and the dir is deleted.
pub struct TempFiles {
    dir: PathBuf,
    files: Vec<SessionFile>,
}

impl std::fmt::Debug for TempFiles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TempFiles({} arquivo(s))", self.files.len())
    }
}

/// `<rt>tmp/`, verified or created: `<rt>` and `tmp` 0700 of the user, no symbolic
/// link, with the dirs above owned by root or by the user (the rules of the socket
/// directory, V3.1).
pub fn temp_dir(rt: &Path) -> io::Result<PathBuf> {
    let error = |_| io::Error::from(io::ErrorKind::PermissionDenied);
    ward_platform::prepare_dir(rt).map_err(error)?;
    let tmp = rt.join(TEMP_DIR);
    ward_platform::prepare_dir(&tmp).map_err(error)?;
    Ok(tmp)
}

impl TempFiles {
    /// New dir of the session (never reuses one that already exists).
    pub fn create(rt: &Path) -> io::Result<TempFiles> {
        let base = temp_dir(rt)?;
        let n = SESSIONS.fetch_add(1, Ordering::Relaxed);
        let dir = base.join(format!("s{}-{n}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&dir)?;
        Ok(TempFiles {
            dir,
            files: Vec::new(),
        })
    }

    /// Writes `content` to `<dir>/<name>`, 0600, a new file (without following links).
    fn write(&mut self, name: &str, content: &[u8]) -> io::Result<String> {
        if name.is_empty() || name.contains('/') || name.starts_with('.') || name.contains('\0') {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let path = self.dir.join(name);
        let text = path.to_str().ok_or(io::ErrorKind::InvalidInput)?.to_owned();
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)?;
        let m = f.metadata()?;
        self.files.push(SessionFile {
            path,
            dev: m.dev(),
            ino: m.ino(),
            size: content.len() as u64,
        });
        f.write_all(content)?;
        Ok(text)
    }
}

/// Zeroes the session's file, if it is still the same one (same inode, a single link,
/// regular): a file the child swapped (a link to another one) is not touched.
fn zero(a: &SessionFile) {
    let Ok(mut f) = fs::OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&a.path)
    else {
        return;
    };
    let Ok(m) = f.metadata() else { return };
    if !m.file_type().is_file() || m.dev() != a.dev || m.ino() != a.ino || m.nlink() != 1 {
        return;
    }
    let zeros = [0u8; 4096];
    let mut left = a.size.min(m.len());
    while left > 0 {
        let n = usize::try_from(left.min(zeros.len() as u64)).unwrap_or(zeros.len());
        if f.write_all(&zeros[..n]).is_err() {
            return;
        }
        left -= n as u64;
    }
}

impl Drop for TempFiles {
    fn drop(&mut self) {
        for a in &self.files {
            zero(a);
        }
        // `remove_dir_all` does not follow symbolic links.
        if fs::remove_dir_all(&self.dir).is_err() {
            aviso!("não deu para apagar a pasta temporária da sessão");
        }
    }
}

/// Daemon startup (S2, V4.15): deletes what a dead `wardd` left in `<rt>tmp/`, zeroing
/// first the regular files of each session dir. Returns how many entries it deleted.
pub fn clean_temp_files(rt: &Path) -> io::Result<usize> {
    let base = temp_dir(rt)?;
    let mut n = 0;
    for item in fs::read_dir(&base)? {
        let item = item?;
        let kind = item.file_type()?;
        let path = item.path();
        if kind.is_dir() {
            if let Ok(inside) = fs::read_dir(&path) {
                for a in inside.flatten() {
                    if let Ok(m) = fs::symlink_metadata(a.path()) {
                        zero(&SessionFile {
                            path: a.path(),
                            dev: m.dev(),
                            ino: m.ino(),
                            size: m.len(),
                        });
                    }
                }
            }
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
        n += 1;
    }
    Ok(n)
}

// --- Delivery (D10) -------------------------------------------------------------------

/// The system resources of the delivery: TOTP code, temporary files and the tunnel.
struct RunResources<'a> {
    rt: Option<&'a Path>,
    temp_files: Option<TempFiles>,
    /// For the database tunnel (Phase 28): the daemon, the entries (with the state
    /// locked), the main entry and the lock count from when the session was authorized.
    tunnel: Option<TunnelContext<'a>>,
    /// The tunnel's bridge, opened by the delivery.
    bridge: Option<Ponte>,
    /// The tunnel's server entry (for the mask of both, D19).
    tunnel_server: Option<&'a Entry>,
}

/// What the tunnel needs from the daemon.
struct TunnelContext<'a> {
    daemon: &'a Daemon,
    entries: &'a [Entry],
    main: &'a Entry,
    locks: (Arc<std::sync::atomic::AtomicU64>, u64),
}

impl Resources for RunResources<'_> {
    fn totp_code(&mut self, seed: &Secret) -> Result<Zeroizing<String>, ResourceError> {
        let c = Totp::from_seed(seed)
            .and_then(|t| t.code_now())
            .map_err(|_| ResourceError("código TOTP"))?;
        let mut s = Zeroizing::new(String::with_capacity(c.code().len()));
        s.push_str(c.code());
        Ok(s)
    }

    fn file(&mut self, name: &str, content: &Secret) -> Result<String, ResourceError> {
        let rt = self.rt.ok_or(ResourceError("sem pasta de temporários"))?;
        if self.temp_files.is_none() {
            self.temp_files =
                Some(TempFiles::create(rt).map_err(|_| ResourceError("pasta de temporários"))?);
        }
        let t = self
            .temp_files
            .as_mut()
            .ok_or(ResourceError("pasta de temporários"))?;
        t.write(name, content.expose())
            .map_err(|_| ResourceError("arquivo temporário"))
    }

    /// D10/D19 (Phase 28): the database tunnel through the referenced server entry is
    /// the bridge module's bridge (`ssh -W <host>:<port>` per connection, no shell),
    /// listening on `127.0.0.1:<free port>` and accepting only the command's process
    /// group.
    fn tunnel(&mut self, server: &str, host: &str, port: u16) -> Result<u16, ResourceError> {
        if self.bridge.is_some() {
            return Err(ResourceError("um túnel só por entrega"));
        }
        let ctx = self
            .tunnel
            .as_ref()
            .ok_or(ResourceError("túnel indisponível"))?;
        let (es, via) = crate::ssh::intermediate(ctx.entries, server)
            .map_err(|_| ResourceError("servidor do túnel"))?;
        let mask = MaskBuilder::new()
            .entry(ctx.main)
            .entry(es)
            .build()
            .map_err(|_| ResourceError("máscara do túnel"))?;
        let bridge = Ponte::abrir(Abertura {
            via,
            alvo_host: host,
            alvo_porta: port,
            programas: ctx.daemon.programas_ssh(),
            known_hosts: &ctx.daemon.known_hosts().map_err(ResourceError)?,
            rt: ctx.daemon.temporarios(),
            mascara: mask,
            limites: LIMITES_TUNEL,
            travas: ctx.locks.clone(),
        })
        .map_err(ResourceError)?;
        let p = bridge.porta();
        self.bridge = Some(bridge);
        self.tunnel_server = Some(es);
        Ok(p)
    }
}

/// Everything ready to launch: the command (with the environment, which has secrets),
/// the mask, the temporaries (deleted on drop) and, in `ward ssh` (Phase 27), the pty
/// and the SSH agent.
pub(crate) struct Preparation {
    pub(crate) command: Command,
    pub(crate) mask: Mask,
    pub(crate) temp_files: Option<TempFiles>,
    /// Jump of `ward ssh` or database tunnel (Phase 28): the bridge, which starts
    /// accepting when the child's pid is known.
    pub(crate) bridge: Option<Ponte>,
    /// Interactive `ward ssh`: the child in a pty of this size (D13, D34).
    pub(crate) pty: Option<Janela>,
    /// `ward ssh` with a key: the agent of D12, which serves only the child.
    pub(crate) agent: Option<PreparedAgent>,
    /// `ward run` and `ward ssh` (Phase 54 and 54c, S8): the child's group dies if the
    /// vault locks.
    pub(crate) lock: Option<DiesOnLock>,
    /// Phase 54b (D68): the CLI said the standard input is null (`/dev/null`). The
    /// child's input closes at launch and a stdin frame with data kills the group:
    /// whoever says "null" sends nothing afterwards (the window's notice was not shown).
    pub(crate) null_stdin: bool,
}

/// The registry of the `ward run` and `ward ssh` in progress and the lock count from
/// when the session was authorized (Phase 54 and 54c, S8).
pub(crate) struct DiesOnLock {
    pub(crate) sessions: Arc<crate::nucleo::SessoesRun>,
    pub(crate) locks: Arc<AtomicU64>,
    pub(crate) initial: u64,
}

impl DiesOnLock {
    fn locked(&self) -> bool {
        self.locks.load(Ordering::Acquire) != self.initial
    }
}

/// The session's SSH agent, ready to receive the child's pid (D12).
pub(crate) struct PreparedAgent {
    pub(crate) agent: SshAgent,
    /// `sftp`: the `ssh` is a child of the child.
    pub(crate) child_of_child: bool,
    /// The core's lock count and its value when the session was authorized.
    pub(crate) locks: Arc<AtomicU64>,
    pub(crate) initial: u64,
}

/// `PATH` of the `ward run` child (Phase 26a): fixed, never the CLI's. A command without
/// `/` is looked up only here; otherwise it goes with an absolute path (or relative to
/// the working directory, which the approval window shows).
pub(crate) const COMMAND_PATH: &[u8] = b"/usr/local/bin:/usr/bin:/bin";

/// Names the daemon sets and that neither the CLI nor the delivery change.
const FROM_DAEMON: [&[u8]; 4] = [b"PATH", b"HOME", b"USER", b"LOGNAME"];

/// How the value of a CLI variable is verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shape {
    /// Locale name, never a path ([`ward_platform::launcher::valid_locale`]).
    Locale,
    /// Terminal name (`TERM`, `COLORTERM`): no `/`, not starting with `.`.
    Terminal,
    /// Timezone by name (`UTC`, `Europe/Lisbon`, `EST5EDT`): no `.` (no `..` nor a
    /// file outside `zoneinfo`) and not starting with `/`.
    Timezone,
    /// Just a flag (`NO_COLOR`): letters and digits.
    Flag,
}

/// What passes from the CLI's environment to the child (Phase 26a), and how the value
/// is verified. A value outside the shape makes the variable stay out (fail closed, no
/// error).
pub(crate) const FROM_CLI: &[(&str, Shape)] = &[
    ("LANG", Shape::Locale),
    ("LC_ALL", Shape::Locale),
    ("LC_ADDRESS", Shape::Locale),
    ("LC_COLLATE", Shape::Locale),
    ("LC_CTYPE", Shape::Locale),
    ("LC_IDENTIFICATION", Shape::Locale),
    ("LC_MEASUREMENT", Shape::Locale),
    ("LC_MESSAGES", Shape::Locale),
    ("LC_MONETARY", Shape::Locale),
    ("LC_NAME", Shape::Locale),
    ("LC_NUMERIC", Shape::Locale),
    ("LC_PAPER", Shape::Locale),
    ("LC_TELEPHONE", Shape::Locale),
    ("LC_TIME", Shape::Locale),
    ("TERM", Shape::Terminal),
    ("COLORTERM", Shape::Terminal),
    ("TZ", Shape::Timezone),
    ("NO_COLOR", Shape::Flag),
];

/// Largest accepted value of a variable of the list.
const MAX_LIST_VALUE: usize = 64;

/// Does the CLI's value have the variable's shape?
fn value_accepted(shape: Shape, v: &[u8]) -> bool {
    if v.is_empty() || v.len() > MAX_LIST_VALUE {
        return false;
    }
    let simple = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'+');
    match shape {
        Shape::Locale => ward_platform::launcher::valid_locale(v),
        Shape::Terminal => v[0] != b'.' && v.iter().all(|&b| simple(b) || b == b'.'),
        Shape::Timezone => {
            let v = v.strip_prefix(b":").unwrap_or(v);
            !v.is_empty()
                && v.split(|&b| b == b'/')
                    .all(|part| !part.is_empty() && part.iter().all(|&b| simple(b)))
        }
        Shape::Flag => v.iter().all(u8::is_ascii_alphanumeric),
    }
}

/// Does the CLI's variable enter the child? Only the names of [`FROM_CLI`], with the
/// value in shape.
fn enters_from_cli(name: &[u8], value: &[u8]) -> bool {
    FROM_CLI
        .iter()
        .find(|(n, _)| n.as_bytes() == name)
        .is_some_and(|&(_, shape)| value_accepted(shape, value))
}

fn is_utf8(value: &[u8]) -> bool {
    let v = value.to_ascii_lowercase();
    v.windows(5).any(|w| w == b"utf-8") || v.windows(4).any(|w| w == b"utf8")
}

/// The child's locale (handoff 22a, RA3): in the C locale, bash writes `${X@Q}` and
/// `declare -p` of a non-ASCII value as `$'…\303\247…'`, which the mask does not cover.
/// If the CLI's effective `LC_CTYPE` (`LC_ALL`, else `LC_CTYPE`, else `LANG`) is not
/// UTF-8, the child gets `C.UTF-8` in that variable only (the CLI's messages and formats
/// stay as they are). Returns the environment with the adjustment: (name, value).
pub fn adjust_locale<'a>(env: Vec<(&'a [u8], &'a [u8])>) -> Vec<(&'a [u8], &'a [u8])> {
    let find =
        |env: &[(&[u8], &[u8])], n: &[u8]| env.iter().position(|(k, v)| *k == n && !v.is_empty());
    let mut env = env;
    if let Some(i) = find(&env, b"LC_ALL") {
        if !is_utf8(env[i].1) {
            env[i].1 = LOCALE_UTF8;
        }
        return env;
    }
    if let Some(i) = find(&env, b"LC_CTYPE") {
        if !is_utf8(env[i].1) {
            env[i].1 = LOCALE_UTF8;
        }
        return env;
    }
    let lang_utf8 = find(&env, b"LANG").is_some_and(|i| is_utf8(env[i].1));
    if !lang_utf8 {
        // An empty `LC_CTYPE=` goes away, so it is not repeated.
        env.retain(|(k, _)| *k != b"LC_CTYPE");
        env.push((b"LC_CTYPE", LOCALE_UTF8));
    }
    env
}

/// The child's `HOME`, `USER` and `LOGNAME`: the ones from passwd (never the CLI's).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Account {
    pub(crate) user: Vec<u8>,
    pub(crate) home: Vec<u8>,
}

impl Account {
    /// The one of the daemon's uid (the same as the client's, D49).
    pub(crate) fn from_passwd() -> Result<Account, ErroResposta> {
        use std::os::unix::ffi::OsStringExt;
        let (user, home) = ward_platform::user_account().map_err(|_| {
            aviso!("executar: usuário sem entrada no passwd");
            ErroResposta::EntregaFalhou
        })?;
        Ok(Account {
            user: user.into_vec(),
            home: home.into_vec(),
        })
    }
}

/// The child's environment (Phase 26a; D13, D60), built from scratch: fixed `PATH`,
/// `HOME`, `USER` and `LOGNAME` from passwd, from the CLI only [`FROM_CLI`] with the
/// value in shape (no repeated name: the first one stays; without the names the delivery
/// brings), the locale adjusted and the delivery on top. Nothing comes from the daemon's
/// environment. Returns how many CLI variables were left out.
fn build_environment(
    cmd: &mut Command,
    cli: &[Variavel],
    account: &Account,
    delivery: &Delivery,
) -> Result<usize, ErroResposta> {
    let from_delivery: HashSet<&[u8]> = delivery.names().map(str::as_bytes).collect();
    // The delivery never changes what the daemon fixes (D42 already refuses; second
    // layer).
    if FROM_DAEMON.iter().any(|n| from_delivery.contains(n)) {
        aviso!("executar: a entrega traz uma variável fixa do daemon; nada foi executado");
        return Err(ErroResposta::EntregaFalhou);
    }
    let mut seen: HashSet<&[u8]> = HashSet::new();
    let mut removed = 0;
    let mut env: Vec<(&[u8], &[u8])> = Vec::with_capacity(FROM_CLI.len() + 1);
    for v in cli {
        let name: &[u8] = &v.nome;
        if !enters_from_cli(name, v.valor.expose()) {
            removed += 1;
            continue;
        }
        if from_delivery.contains(name) || !seen.insert(name) {
            continue;
        }
        env.push((name, v.valor.expose()));
    }
    let fixed: [(&[u8], &[u8]); 4] = [
        (b"PATH", COMMAND_PATH),
        (b"HOME", &account.home),
        (b"USER", &account.user),
        (b"LOGNAME", &account.user),
    ];
    for (k, v) in fixed.into_iter().chain(adjust_locale(env)) {
        cmd.variable(k, v)
            .map_err(|_| ErroResposta::PedidoInvalido)?;
    }
    for v in delivery.variables() {
        cmd.variable(v.name().as_bytes(), v.expose().as_bytes())
            .map_err(|_| ErroResposta::EntregaFalhou)?;
    }
    Ok(removed)
}

/// The visible entry (blocked = nonexistent, V3.17).
fn visible<'a>(entries: &'a [Entry], name: &str) -> Option<&'a Entry> {
    entries
        .iter()
        .find(|e| e.name() == name && e.level() != AccessLevel::Blocked)
}

impl Daemon {
    /// Steps 1 to 4: authorizes, builds the delivery, the mask and the command.
    fn prepare_run(
        &self,
        client: &dyn Cliente,
        request: Pedido,
    ) -> Result<Preparation, ErroResposta> {
        let Pedido::Executar(x) = &request else {
            return Err(ErroResposta::PedidoInvalido);
        };
        // Phase 54b: only the kind (fixed text), for the CLI suite to verify the `fstat`.
        depurar!("executar: entrada padrão {}", x.tipo_entrada.nome());
        // A visible secure note: refuses before asking for approval (V4.11).
        if let Some(a) = self.estado().aberto.as_ref()
            && visible(&a.entradas, &x.entrada)
                .is_some_and(|e| matches!(e.data(), Data::SecureNote(_)))
        {
            return Err(ErroResposta::TipoSemEntrega);
        }
        self.autorizar_uso(client, &request)?;
        let Pedido::Executar(x) = request else {
            return Err(ErroResposta::PedidoInvalido);
        };
        self.registrar_uso();
        let (argv0, cwd) = match (x.argv.first(), x.cwd.is_empty()) {
            (Some(a), false) => (a, &x.cwd),
            _ => return Err(ErroResposta::PedidoInvalido),
        };
        let mut command = Command::new(argv0, cwd).map_err(|_| ErroResposta::PedidoInvalido)?;
        for a in &x.argv[1..] {
            command
                .argument(a)
                .map_err(|_| ErroResposta::PedidoInvalido)?;
        }
        let st = self.estado();
        let a = st.aberto.as_ref().ok_or(ErroResposta::Travado)?;
        // The entry may have changed during the approval: verify again.
        let e = visible(&a.entradas, &x.entrada).ok_or(ErroResposta::EntradaInexistente)?;
        let (locks, initial) = self.travas_agora(&st);
        let lock = DiesOnLock {
            sessions: self.sessoes_run(&st),
            locks,
            initial,
        };
        let mut resources = RunResources {
            rt: self.temporarios(),
            temp_files: None,
            tunnel: Some(TunnelContext {
                daemon: self,
                entries: &a.entradas,
                main: e,
                locks: self.travas_agora(&st),
            }),
            bridge: None,
            tunnel_server: None,
        };
        let delivery = match delivery::build(e, &mut resources) {
            Ok(g) => g,
            Err(DeliveryError::TypeWithoutDelivery) => return Err(ErroResposta::TipoSemEntrega),
            Err(err) => {
                aviso!("executar: {err}");
                return Err(ErroResposta::EntregaFalhou);
            }
        };
        // D19: with a tunnel, the mask also covers the tunnel's server entry.
        let mut builder = MaskBuilder::new().entry(e).delivery(&delivery);
        if let Some(s) = resources.tunnel_server {
            builder = builder.entry(s);
        }
        let mask = builder.build().map_err(|err| {
            aviso!("executar: {err}");
            ErroResposta::EntregaFalhou
        })?;
        let RunResources {
            temp_files, bridge, ..
        } = resources;
        drop(st);
        let account = Account::from_passwd()?;
        let removed = build_environment(&mut command, &x.ambiente, &account, &delivery)?;
        if removed > 0 {
            depurar!("executar: {removed} variável(is) da CLI fora da lista (Fase 26a)");
        }
        if bridge.is_some() {
            depurar!("executar: com túnel (ponte em 127.0.0.1)");
        }
        Ok(Preparation {
            command,
            mask,
            temp_files,
            bridge,
            pty: None,
            agent: None,
            lock: Some(lock),
            null_stdin: x.tipo_entrada == TipoEntradaPadrao::Nula,
        })
    }
}

// --- Session ---------------------------------------------------------------------------

/// The CLI side: frames written by several threads, one whole frame at a time.
#[derive(Clone)]
pub(crate) struct Cli(Arc<Mutex<UnixStream>>);

impl Cli {
    /// Writes the response; `false` if the CLI is no longer there.
    fn send(&self, r: &Resposta) -> bool {
        let Ok(q) = protocolo::codificar_resposta(Papel::Agente, r) else {
            aviso!("executar: resposta recusada para o agente");
            return false;
        };
        let mut s = self.0.lock().unwrap_or_else(|e| e.into_inner());
        quadro::escrever_quadro(&mut *s, &q, quadro::LIMITE_RESPOSTA).is_ok()
    }

    /// Sends the already masked output in frames of up to `MAX_DISPLAYED_TEXT` and empties
    /// `out`.
    fn send_output(&self, stream: Fluxo, out: &mut Zeroizing<Vec<u8>>) -> bool {
        let mut ok = true;
        for piece in out.chunks(MAX_DISPLAYED_TEXT) {
            let mut data = Zeroizing::new(Vec::with_capacity(piece.len()));
            data.extend_from_slice(piece);
            if !self.send(&Resposta::Saida {
                fluxo: stream,
                dados: data,
            }) {
                ok = false;
                break;
            }
        }
        zeroize::Zeroize::zeroize(&mut **out);
        out.clear();
        ok
    }
}

/// Reads the child's output, masks it and sends it to the CLI (D11; one stream per
/// output). Stops at the end of the pipe or when `stop` turns on (a grandchild outside
/// the group holding the pipe). Each piece read goes out right away, already masked (D34
/// revised, M4 audit): nothing is held or delayed, with or without a pty, so the output
/// does not become a prefix oracle.
fn read_output(
    mut f: File,
    stream: Fluxo,
    mask: Arc<Mask>,
    cli: Cli,
    stop: Arc<AtomicBool>,
    ready: mpsc::Sender<()>,
) {
    let mut m = mask.stream();
    let mut buf = Zeroizing::new(vec![0u8; BATCH]);
    let mut out = Zeroizing::new(Vec::with_capacity(BATCH + BATCH / 4));
    let mut cli_ok = true;
    loop {
        match child::ready_to_read(&f, READ_STEP_MS) {
            Ok(true) => {}
            Ok(false) if !stop.load(Ordering::Acquire) => continue,
            _ => break,
        }
        let (n, end) = read_batch(&mut f, &mut buf);
        if n > 0 {
            m.mask(&buf[..n], &mut out);
            zeroize::Zeroize::zeroize(&mut buf[..n]);
            if cli_ok {
                cli_ok = cli.send_output(stream, &mut out);
            } else {
                zeroize::Zeroize::zeroize(&mut *out);
            }
        }
        if end {
            break;
        }
    }
    // Nothing was held; just closes the stream (zeroes).
    m.finish(&mut out);
    if cli_ok {
        cli.send_output(stream, &mut out);
    }
    let _ = ready.send(());
}

/// Reads from `f` everything that is already there, until `buf` is full (Phase 28g, 2nd
/// M4 audit).
///
/// The first read may block; the following ones only happen if `poll` says, without
/// waiting at all, that there is more (it never waits for new data: D34 revised, nothing
/// held or delayed). So what the child wrote at once, and the kernel delivered in more
/// than one read (64 KiB pipe, 4 KiB pty), is masked in a single batch. The fd stays
/// blocking: the pty master is the same open file as the input's write side, and
/// `O_NONBLOCK` on it would apply to both.
///
/// Returns the bytes read and whether the output ended (end, `EIO` from the pty when the
/// session closes the slave, or an error).
pub(crate) fn read_batch(f: &mut File, buf: &mut [u8]) -> (usize, bool) {
    let mut n = 0;
    loop {
        match f.read(&mut buf[n..]) {
            Ok(0) => return (n, true),
            Ok(k) => n += k,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return (n, true),
        }
        if n == buf.len() {
            return (n, false);
        }
        match child::ready_to_read(f, 0) {
            Ok(true) => {}
            Ok(false) => return (n, false),
            Err(_) => return (n, true),
        }
    }
}

/// Passes the CLI's input to the child; `None` (or the closed queue) closes its input.
fn write_stdin(mut f: File, queue: Receiver<Option<Secret>>) {
    while let Ok(Some(data)) = queue.recv() {
        if f.write_all(data.expose()).is_err() {
            break;
        }
    }
}

/// Reads the CLI's frames during the session: a stdin frame goes to the child; a window
/// frame changes the pty's size (without a pty, as in `ward run`, it is ignored); any
/// other frame, or the connection going away, kills the child's group (D13) and, since
/// Phase 54l, the group of each direct child with its own session (the `ssh` that
/// `sshpass` creates). After the normal end, reading ends without killing anything. In
/// the pty, empty input closes nothing (the end of input is the Ctrl+D the terminal sends
/// as a byte). With `null_stdin` (Phase 54b), only the empty end frame is accepted; data
/// counts as a frame outside the session.
fn read_from_cli(
    mut s: UnixStream,
    group: Arc<Group>,
    queue: SyncSender<Option<Secret>>,
    end: Arc<AtomicBool>,
    pty: Option<Arc<Pty>>,
    null_stdin: bool,
) {
    let mut stdin_open = !null_stdin;
    loop {
        let bytes = match quadro::ler_quadro(&mut s, MAX_COMMON_FRAME) {
            Ok(b) => b,
            Err(_) => {
                if !end.load(Ordering::Acquire) {
                    aviso!("executar: a CLI saiu; o grupo do comando foi morto (D13)");
                    group.kill_with_child_sessions();
                }
                return;
            }
        };
        match protocolo::decodificar_pedido(Papel::Agente, &bytes) {
            Ok(Pedido::EntradaPadrao { dados }) if null_stdin && !dados.is_empty() => {
                if !end.load(Ordering::Acquire) {
                    aviso!(
                        "executar: dado na entrada padrão declarada nula; o grupo do comando foi morto (D68)"
                    );
                    group.kill_with_child_sessions();
                }
                return;
            }
            Ok(Pedido::EntradaPadrao { dados }) => {
                if !stdin_open || (dados.is_empty() && pty.is_some()) {
                    continue;
                }
                let item = if dados.is_empty() {
                    stdin_open = false;
                    None
                } else {
                    Some(dados)
                };
                if !enqueue(&queue, item, &s, &end, &group) {
                    return;
                }
            }
            Ok(Pedido::Janela(j)) => {
                if let Some(p) = &pty {
                    let _ = p.set_window_size(j.linhas, j.colunas);
                }
            }
            _ => {
                if !end.load(Ordering::Acquire) {
                    aviso!("executar: quadro fora da sessão; o grupo do comando foi morto");
                    group.kill_with_child_sessions();
                }
                return;
            }
        }
    }
}

/// Puts the item in the input queue without losing sight of the CLI: with the queue full
/// (the child does not read), checks whether the connection went away while waiting.
/// `false`: the session ended.
fn enqueue(
    queue: &SyncSender<Option<Secret>>,
    mut item: Option<Secret>,
    s: &UnixStream,
    end: &AtomicBool,
    group: &Group,
) -> bool {
    loop {
        match queue.try_send(item) {
            Ok(()) => return true,
            Err(TrySendError::Disconnected(_)) => return true,
            Err(TrySendError::Full(back)) => {
                item = back;
                if end.load(Ordering::Acquire) {
                    return false;
                }
                if ward_platform::peer_closed(s) {
                    aviso!("executar: a CLI saiu; o grupo do comando foi morto (D13)");
                    group.kill_with_child_sessions();
                    return false;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        }
    }
}

fn thread(name: &str, f: impl FnOnce() + Send + 'static) -> bool {
    std::thread::Builder::new()
        .name(name.into())
        .spawn(f)
        .is_ok()
}

/// Serves a run request of the agent role on the connection `stream`, until the command
/// ends. Every response (error, output, end) is written here. Returns `true` if the
/// request was refused before launching any process (the connection goes on, as in the
/// other operations); `false` when the session happened and the connection ended.
pub fn session(
    daemon: &Daemon,
    client: &dyn Cliente,
    stream: &UnixStream,
    request: Pedido,
) -> bool {
    session_with(
        stream,
        |c| {
            let r = daemon.prepare_run(c, request);
            if let Err(e) = &r {
                depurar!("executar (agente): {}", e.codigo());
            }
            r
        },
        client,
    )
}

/// The launch: the child with pipes or in a pty.
enum Launched {
    Pipes(child::Pipes),
    Pty(Arc<Pty>),
}

/// Prepares (with `prepare`) and runs the run or `ssh` session on the connection:
/// same return as [`session`].
pub(crate) fn session_with(
    stream: &UnixStream,
    prepare: impl FnOnce(&dyn Cliente) -> Result<Preparation, ErroResposta>,
    client: &dyn Cliente,
) -> bool {
    let Ok(writer) = stream.try_clone() else {
        return false;
    };
    let cli = Cli(Arc::new(Mutex::new(writer)));
    let preparation = match prepare(client) {
        Ok(p) => p,
        Err(e) => return cli.send(&Resposta::Erro(e)),
    };
    // The session lasts as long as the command; the CLI may stall (full pipe).
    let _ = stream.set_read_timeout(None);
    let _ = stream.set_write_timeout(None);
    let Preparation {
        command,
        mask,
        temp_files,
        bridge,
        pty,
        agent,
        lock,
        null_stdin,
    } = preparation;
    let launched = match pty {
        Some(j) => command
            .launch_in_pty(j.linhas, j.colunas)
            .map(|(g, p)| (g, Launched::Pty(Arc::new(p)))),
        None => command.launch().map(|(g, p)| (g, Launched::Pipes(p))),
    };
    // The environment with secrets is zeroed now, not at the end of the command.
    drop(command);
    let (group, launched) = match launched {
        Ok(g) => g,
        Err(e) => {
            depurar!("executar: o filho não foi criado ({e:?})");
            drop(bridge);
            drop(temp_files);
            drop(agent);
            let mut msg = Zeroizing::new(Vec::new());
            msg.extend_from_slice(b"ward: ");
            msg.extend_from_slice(e.message().as_bytes());
            msg.extend_from_slice(b"\n");
            cli.send(&Resposta::Saida {
                fluxo: Fluxo::Erro,
                dados: msg,
            });
            cli.send(&Resposta::Fim {
                codigo: e.exit_code(),
            });
            let _ = stream.shutdown(std::net::Shutdown::Both);
            return false;
        }
    };
    let pid = group.pid();
    depurar!("executar: filho pid {pid}");
    // D12: the agent serves only the child that has just been born (the pid is not
    // reused: the daemon only reaps the child at the end).
    let agent = agent.map(|a| {
        a.agent.start(
            Allowed {
                pid,
                child_of_child: a.child_of_child,
            },
            a.locks,
            a.initial,
        )
    });
    // The jump/tunnel bridge accepts only the child's group (Phase 28); the `stderr` of
    // its `ssh` reaches the CLI already masked.
    let bridge = bridge.map(|p| {
        let c = cli.clone();
        p.iniciar(
            pid,
            Arc::new(move |out: &mut Zeroizing<Vec<u8>>| {
                c.send_output(Fluxo::Erro, out);
            }),
        )
    });
    let group = Arc::new(group);
    // Phase 54 (S8): the lock kills the `ward run` group; if it came between the
    // authorization and here, the group dies right away.
    if let Some(t) = &lock
        && !t.sessions.registrar(&group, &t.locks, t.initial)
    {
        depurar!("executar: o cofre travou antes do lançamento; pid {pid} encerrado");
    }
    let mask = Arc::new(mask);
    let stop = Arc::new(AtomicBool::new(false));
    let end = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = mpsc::channel();
    let mut readers = 0;
    let (stdin, outputs, pty) = match launched {
        Launched::Pipes(child::Pipes {
            stdin,
            stdout,
            stderr,
        }) => (
            stdin,
            vec![
                (stdout, Fluxo::Saida, "wardd-run-saida"),
                (stderr, Fluxo::Erro, "wardd-run-erro"),
            ],
            None,
        ),
        Launched::Pty(p) => {
            let reader = p.master.try_clone().ok();
            let writer = p.master.try_clone().ok();
            if reader.is_none() || writer.is_none() {
                group.kill_with_child_sessions();
            }
            (
                writer,
                vec![(reader, Fluxo::Saida, "wardd-ssh-pty")],
                Some(p),
            )
        }
    };
    for (f, stream, name) in outputs {
        let Some(f) = f else { continue };
        let (m, c, p, t) = (mask.clone(), cli.clone(), stop.clone(), ready_tx.clone());
        if thread(name, move || read_output(f, stream, m, c, p, t)) {
            readers += 1;
        }
    }
    drop(ready_tx);
    let (queue_tx, queue_rx) = mpsc::sync_channel(STDIN_QUEUE);
    // Phase 54b: null input, the child's closes right away (end of input, like
    // `/dev/null`). In the pty the master is also the output and stays; data that arrives
    // kills the group.
    let stdin = if pty.is_none() && null_stdin {
        drop(stdin);
        None
    } else {
        stdin
    };
    if let Some(f) = stdin {
        thread("wardd-run-entrada", move || write_stdin(f, queue_rx));
    }
    let reader_ok = match stream.try_clone() {
        Ok(s) => {
            let (g, f, p) = (group.clone(), end.clone(), pty.clone());
            thread("wardd-run-cli", move || {
                read_from_cli(s, g, queue_tx, f, p, null_stdin)
            })
        }
        Err(_) => false,
    };
    if !reader_ok {
        // Without seeing the CLI, the command does not run (D13: the CLI that goes away
        // kills the group).
        group.kill_with_child_sessions();
    }

    // End of the child: the whole group dies, the temporary and the agent go away
    // (V4.15, D12) and the missing output is delivered.
    let _ = group.wait_for_end();
    group.kill_with_child_sessions();
    let result = group.collect();
    drop(bridge);
    drop(agent);
    drop(temp_files);
    let limit = Instant::now() + REST_OF_OUTPUT_DEADLINE;
    for _ in 0..readers {
        let left = limit.saturating_duration_since(Instant::now());
        if ready_rx.recv_timeout(left).is_err() {
            break;
        }
    }
    stop.store(true, Ordering::Release);
    drop(pty);
    let locked = lock.as_ref().is_some_and(DiesOnLock::locked);
    if let Some(data) = end_notice(locked, group.remaining()) {
        cli.send(&Resposta::Saida {
            fluxo: Fluxo::Erro,
            dados: data,
        });
    }
    let code = match result {
        Ok(f) => {
            match f {
                child::End::Code(c) => depurar!("executar: pid {pid} saiu com código {c}"),
                child::End::Signal(s) => depurar!("executar: pid {pid} encerrado pelo sinal {s}"),
            }
            f.exit_code()
        }
        Err(_) => {
            aviso!("executar: não deu para colher o pid {pid}");
            1
        }
    };
    end.store(true, Ordering::Release);
    cli.send(&Resposta::Fim { codigo: code });
    let _ = stream.shutdown(std::net::Shutdown::Both);
    false
}

/// The session's end notice for the CLI (Phase 54 and 55, S8): "ended" only when
/// nothing of the session (the command and what left its group) is still alive.
fn end_notice(locked: bool, remaining: usize) -> Option<Zeroizing<Vec<u8>>> {
    let text = match (locked, remaining) {
        (true, 0) => "ward: o cofre travou; o comando foi encerrado\n".to_owned(),
        (true, n) => {
            format!("ward: o cofre travou, mas {n} processo(s) do comando continuam vivos\n")
        }
        (false, 0) => return None,
        (false, n) => format!("ward: {n} processo(s) do comando continuam vivos depois do fim\n"),
    };
    Some(Zeroizing::new(text.into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Phase 55: the CLI only hears "ended" if nothing remained.
    #[test]
    fn end_notice_only_says_ended_with_nothing_left() {
        let t = |a: Option<Zeroizing<Vec<u8>>>| a.map(|v| String::from_utf8(v.to_vec()).unwrap());
        assert_eq!(
            t(end_notice(true, 0)).as_deref(),
            Some("ward: o cofre travou; o comando foi encerrado\n")
        );
        let one = t(end_notice(true, 1)).unwrap();
        assert!(
            !one.contains("encerrado") && one.contains("1 processo(s)"),
            "{one}"
        );
        assert_eq!(t(end_notice(false, 0)), None);
        let two = t(end_notice(false, 2)).unwrap();
        assert!(
            !two.contains("encerrado") && two.contains("2 processo(s)"),
            "{two}"
        );
    }

    /// Test dir in `/run/user/<uid>/` (the dirs above `<rt>` must be owned by root or by
    /// the user, without group write).
    fn test_rt(label: &str) -> PathBuf {
        ward_platform::socket::runtime_dir()
            .join(format!("wardd-exec-{}-{label}", std::process::id()))
    }

    fn env<'a>(v: &[(&'a str, &'a str)]) -> Vec<(&'a [u8], &'a [u8])> {
        v.iter()
            .map(|(k, x)| (k.as_bytes(), x.as_bytes()))
            .collect()
    }

    fn text(v: Vec<(&[u8], &[u8])>) -> Vec<(String, String)> {
        v.into_iter()
            .map(|(k, x)| {
                (
                    String::from_utf8(k.to_vec()).unwrap(),
                    String::from_utf8(x.to_vec()).unwrap(),
                )
            })
            .collect()
    }

    #[test]
    fn utf8_locale_stays_and_c_becomes_c_utf8() {
        let s = |v: &[(&str, &str)]| text(adjust_locale(env(v)));
        let pair = |k: &str, v: &str| (k.to_owned(), v.to_owned());
        assert_eq!(
            s(&[("LANG", "pt_BR.UTF-8")]),
            vec![pair("LANG", "pt_BR.UTF-8")]
        );
        assert_eq!(
            s(&[("LANG", "en_US.utf8")]),
            vec![pair("LANG", "en_US.utf8")]
        );
        assert_eq!(
            s(&[("LANG", "C")]),
            vec![pair("LANG", "C"), pair("LC_CTYPE", "C.UTF-8")]
        );
        assert_eq!(s(&[]), vec![pair("LC_CTYPE", "C.UTF-8")]);
        assert_eq!(
            s(&[("LC_ALL", "C"), ("LANG", "pt_BR.UTF-8")]),
            vec![pair("LC_ALL", "C.UTF-8"), pair("LANG", "pt_BR.UTF-8")]
        );
        assert_eq!(
            s(&[("LC_CTYPE", "POSIX"), ("LANG", "pt_BR.UTF-8")]),
            vec![pair("LC_CTYPE", "C.UTF-8"), pair("LANG", "pt_BR.UTF-8")]
        );
        assert_eq!(
            s(&[("LC_CTYPE", ""), ("LANG", "C")]),
            vec![pair("LANG", "C"), pair("LC_CTYPE", "C.UTF-8")]
        );
        assert_eq!(
            s(&[("LC_ALL", "pt_BR.UTF-8")]),
            vec![pair("LC_ALL", "pt_BR.UTF-8")]
        );
    }

    /// Phase 26a: only the list passes from the CLI; the rest (loader, shell,
    /// interpreters, git, ssh, libpq, `PATH`, `HOME`) stays out, with any value.
    #[test]
    fn attack_only_the_list_passes_from_the_cli() {
        for name in [
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
            "LD_AUDIT",
            "PATH",
            "HOME",
            "USER",
            "LOGNAME",
            "BASH_ENV",
            "ENV",
            "BASH_FUNC_ls%%",
            "SHELLOPTS",
            "PS4",
            "IFS",
            "NODE_OPTIONS",
            "NODE_PATH",
            "PYTHONPATH",
            "PYTHONSTARTUP",
            "PYTHONHOME",
            "PERL5LIB",
            "PERL5OPT",
            "RUBYOPT",
            "GIT_SSH_COMMAND",
            "GIT_CONFIG_GLOBAL",
            "GIT_DIR",
            "SSH_AUTH_SOCK",
            "SSH_ASKPASS",
            "PGSERVICEFILE",
            "PGPASSFILE",
            "PGHOST",
            "PSQLRC",
            "LOCPATH",
            "GCONV_PATH",
            "TERMINFO",
            "TERMINFO_DIRS",
            "TZDIR",
            "LANGUAGE",
            "NLSPATH",
            "TMPDIR",
            "XDG_CONFIG_HOME",
            "PAGER",
            "EDITOR",
            "WARD_PASS",
            "OLD_PWD",
            "LC_",
            "LC_TERMINAL",
        ] {
            assert!(!enters_from_cli(name.as_bytes(), b"C"), "{name} passou");
        }
        for (name, value) in [
            ("LANG", "pt_BR.UTF-8"),
            ("LC_ALL", "C.UTF-8"),
            ("LC_TIME", "en_US.utf8"),
            ("TERM", "xterm-256color"),
            ("TERM", "screen.xterm-256color"),
            ("COLORTERM", "truecolor"),
            ("TZ", "UTC"),
            ("TZ", "America/Sao_Paulo"),
            ("TZ", ":America/Sao_Paulo"),
            ("TZ", "Etc/GMT+3"),
            ("TZ", "EST5EDT"),
            ("NO_COLOR", "1"),
        ] {
            assert!(
                enters_from_cli(name.as_bytes(), value.as_bytes()),
                "{name}={value}"
            );
        }
    }

    /// Phase 26a: a value that would become a path outside the system dirs stays out
    /// (locale, terminfo and timezone load files; glibc accepts an absolute path and
    /// `..`).
    #[test]
    fn attack_value_that_becomes_a_path_stays_out() {
        let long = "a".repeat(MAX_LIST_VALUE + 1);
        for (name, value) in [
            ("LANG", "/tmp/atacante/locale"),
            ("LC_ALL", "../../../tmp/atacante"),
            ("LC_MESSAGES", "pt_BR/../../x"),
            ("LANG", ".."),
            ("LANG", ""),
            ("LANG", long.as_str()),
            ("TERM", "../../tmp/atacante/x"),
            ("TERM", "/tmp/atacante/x"),
            ("TERM", ".x"),
            ("TERM", "xterm\n"),
            ("COLORTERM", "a b"),
            ("TZ", "/tmp/atacante/fuso"),
            ("TZ", ":/tmp/atacante/fuso"),
            ("TZ", "../../../tmp/atacante/fuso"),
            ("TZ", "America/../../tmp/x"),
            ("TZ", "America//Sao_Paulo"),
            ("TZ", "America/"),
            ("TZ", ":"),
            ("TZ", "BRT3,M10.2.0"),
            ("TZ", long.as_str()),
            ("NO_COLOR", "$(id)"),
            ("NO_COLOR", ""),
        ] {
            assert!(
                !enters_from_cli(name.as_bytes(), value.as_bytes()),
                "{name}={value}"
            );
        }
    }

    /// S2: the file is 0600 in a 0700 dir of the session; on drop, zeroed and deleted;
    /// and a link swapped by the child does not lead the daemon to zero another file.
    #[test]
    fn temp_files_0600_zeroed_and_deleted() {
        use std::os::unix::fs::PermissionsExt;
        let rt = test_rt("rt");
        let _ = fs::remove_dir_all(&rt);
        fs::create_dir_all(&rt).unwrap();
        fs::set_permissions(&rt, fs::Permissions::from_mode(0o700)).unwrap();
        let target = rt.join("alvo-de-fora");
        fs::write(&target, b"nao-tocar").unwrap();
        let mut t = TempFiles::create(&rt).unwrap();
        let p = t.write("app.env", b"FALSO_ENV=falso-anexo-01").unwrap();
        let p2 = t.write("chave", b"falso-chave").unwrap();
        assert!(t.write("app.env", b"x").is_err(), "nome repetido");
        assert!(t.write("../fora", b"x").is_err());
        assert!(t.write(".oculto", b"x").is_err());
        let m = fs::metadata(&p).unwrap();
        assert_eq!(m.permissions().mode() & 0o777, 0o600);
        let dir = std::path::Path::new(&p).parent().unwrap().to_path_buf();
        assert_eq!(
            fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(rt.join("tmp")).unwrap().permissions().mode() & 0o777,
            0o700
        );
        // The "child" swaps the key for a link to an outside file.
        fs::remove_file(&p2).unwrap();
        std::os::unix::fs::symlink(&target, &p2).unwrap();
        drop(t);
        assert!(!dir.exists());
        assert_eq!(fs::read(&target).unwrap(), b"nao-tocar");
        fs::remove_dir_all(&rt).unwrap();
    }

    /// V4.15: what a dead daemon left goes away at startup.
    #[test]
    fn startup_cleans_what_was_left() {
        use std::os::unix::fs::PermissionsExt;
        let rt = test_rt("rt2");
        let _ = fs::remove_dir_all(&rt);
        fs::create_dir_all(&rt).unwrap();
        fs::set_permissions(&rt, fs::Permissions::from_mode(0o700)).unwrap();
        let mut t = TempFiles::create(&rt).unwrap();
        t.write("app.env", b"FALSO_ENV=falso-anexo-01").unwrap();
        std::mem::forget(t);
        assert_eq!(clean_temp_files(&rt).unwrap(), 1);
        assert_eq!(fs::read_dir(rt.join("tmp")).unwrap().count(), 0);
        fs::remove_dir_all(&rt).unwrap();
    }

    /// `<rt>` with open permissions or a symbolic link is refused (nothing is written).
    #[test]
    fn attack_open_or_linked_rt_refused() {
        use std::os::unix::fs::PermissionsExt;
        let base = test_rt("rt3");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        let rt = base.join("rt");
        fs::create_dir(&rt).unwrap();
        fs::set_permissions(&rt, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(TempFiles::create(&rt).is_err());
        let link = base.join("link");
        std::os::unix::fs::symlink(&rt, &link).unwrap();
        fs::set_permissions(&rt, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(TempFiles::create(&link).is_err());
        fs::remove_dir_all(&base).unwrap();
    }
}
