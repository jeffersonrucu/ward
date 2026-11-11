//! The daemon's state and the business rule of each operation (Phase 22).
//!
//! - **Locked / unlocked.** Unlocked, the daemon holds a single [`OpenVault`] (single
//!   owner, no copy) and the entries. Locking drops both: the data key, the PIN hash and
//!   the secrets are zeroed on drop (S8, V3.8).
//! - **Unlock** (D20, revised by D62): the attempt waits 0 s, and each consecutive wrong
//!   password doubles the wait of the next one starting from 1 s, up to 30 s. There are
//!   two counters, each for all the connections of the role: the agent's (includes
//!   the CLI unlock) and the app's. An agent error does not delay the app. The agent's
//!   goes to the state file, with fsync before the error response, and comes back on
//!   startup: killing the `wardd` does not reset the wait (the first attempt after
//!   startup waits what is left, counted from startup). The app's stays in memory. The
//!   right unlock from any role resets both. The response does not say how many attempts
//!   there were.
//! - **PIN** (S9): verified against the vault config's hash (`PinHash`, in constant
//!   time, D30); 5 errors in a row lock the vault, even when reconnecting.
//! - **Generations** (D39, D53): the state file keeps `seen` and `burned`. Opens with
//!   (`seen`, `burned`); before each write it writes `burned` = generation + 1 and
//!   only then writes the vault; after the write that succeeded, `seen` = generation.
//!   Without a state file and with the vault present, it does not unlock.
//! - **Migration** (m2:DT5): the unlock of a vault with a v4 body (0.1.x) writes it as v5
//!   through that same write (generation + 1, state, local backup) before answering
//!   "ok"; if the write fails, the unlock answers the error and the v4 stays. A v5 body is
//!   never written again just for being opened.
//! - **Identity** (D39, D62): `dev`/inode/`mtime`/size of the vault by `fstat` of the fd
//!   read and, after writing, of the temp file's fd (returned by `ward_core`), never by a
//!   `stat` of the path after the rename; before writing (and when reopening) it is
//!   verified. Changed: does not write, locks and answers [`ResponseError::VaultChanged`].
//! - **Create with an old state** (D62): without a vault and with a state of `burned` q,
//!   the new vault is born at generation q+1, and a copy of a previous vault stays below
//!   `seen`.
//! - No response to the agent carries a level or a summary (D50, D65 c), and a blocked
//!   entry answers the same as a missing one (V3.17).
//! - **The app's reveal, copy and TOTP** (D4, D65 b): only the app role, with the vault
//!   unlocked, without PIN, without the approval window and without looking at the level
//!   (the blocked entry too). The log records the operation (reveal, copy or
//!   totp), the entry and the field without the value (S13); the value goes only in the
//!   response, in a secret that is zeroed on drop.
//! - The agent's use and the approval (Phase 23): in [`crate::usage`], with the engine of
//!   [`crate::approval`].
//! - **Local backup** (Phase 45, D17): each vault write that succeeded writes the same
//!   sealed version in `<vault dir>/backups/`, opened by the fd with `O_NOFOLLOW`
//!   ([`files::open_backup_dir`]); 30 versions. If the dir does not open or is not safe,
//!   the vault is written anyway and the log warns. **Restore** is only from the app,
//!   with the vault unlocked, the current PIN (S9) and the version's master password: the
//!   current vault becomes a backup first, and the version is sealed again with
//!   generation = highest burned + 1 (D39), with the state updated before and after, as in
//!   a normal write.
//! - **External backup** (Phase 46, D17, D30): enabled/disabled and the UUID are in the
//!   vault config (the external backup operations, app only; disabled by
//!   default). With the vault unlocked and the external one enabled,
//!   [`Daemon::sync_external`] copies the versions of the local backup to the USB drive
//!   of the UUID ([`crate::external`]); the [`crate::udisks`] thread calls it on each USB
//!   drive plugged in, and the daemon pokes it after unlocking and after writing.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, Weak};
use std::time::{Duration, SystemTime};

use zeroize::Zeroizing;

use ward_core::atomic_write::{self, Step, WriteError};
use ward_core::backup::{self, BackupError, Version};
use ward_core::body::{self, BodyError, OpenError};
use ward_core::config::{ExternalBackup, PinHash, VaultConfig};
use ward_core::entry::{AccessLevel, Data, DataDraft, Dates, Draft, Entry, Secret};
use ward_core::format::{MasterPassword, OpenVault};
use ward_core::origin::Origin;
use ward_core::protocol::{
    ApprovalRequest, BackupVersion, EntryDetail, ListItem, MAX_BACKUPS, MAX_USB_DRIVES, Operation,
    Problem, Request, Response, ResponseError, Role, Suggestion, Target, UsbDrive,
};
use ward_core::totp::Totp;

use crate::approval::{Client, Engine, NoClient};
use crate::external::{self, BackupCopy, ExternalError, UsbDriveSource};
use crate::files::{self, FileIdentity, Generations, StateFile};
use crate::{debug, warning};
use ward_platform::child::Group;

/// Consecutive PIN errors that lock the vault (S9).
pub const MAX_PIN_ERRORS: u32 = 5;
/// Longest unlock wait (D20).
pub const MAX_WAIT: Duration = Duration::from_secs(30);

/// Wait before the next unlock attempt, after `failures` consecutive wrong passwords
/// (D20): 0, 1, 2, 4, 8, 16, 30, 30… seconds.
pub fn unlock_wait(failures: u32) -> Duration {
    if failures == 0 {
        return Duration::ZERO;
    }
    let seconds = 1u64.checked_shl(failures - 1).unwrap_or(u64::MAX);
    Duration::from_secs(seconds).min(MAX_WAIT)
}

/// The daemon's clock (injectable in the tests).
pub trait Clock: Send + Sync {
    fn sleep(&self, d: Duration);

    /// Monotonic time that counts the suspend (`CLOCK_BOOTTIME`), for the inactivity lock
    /// (Phase 24). `None` (could not read it) locks: fail closed.
    fn now(&self) -> Option<Duration> {
        ward_platform::time_since_boot().ok()
    }

    /// Seconds since 1970 by the wall clock, for the TOTP code (D65 b). `None` (clock
    /// before 1970): the `totp` answers an error, never a code.
    fn unix_seconds(&self) -> Option<u64> {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs())
    }
}

/// The system clock.
pub struct RealClock;

impl Clock for RealClock {
    fn sleep(&self, d: Duration) {
        if !d.is_zero() {
            std::thread::sleep(d);
        }
    }
}

/// Where the vault and the state file are.
#[derive(Debug, Clone)]
pub struct Paths {
    pub vault: PathBuf,
    pub state: PathBuf,
}

/// Unlocked vault.
pub(crate) struct Open {
    vault: OpenVault,
    /// In name order.
    pub(crate) entries: Vec<Entry>,
    /// "Allow for 1 hour" (D2): entry → until when, on the engine's clock. Goes away on
    /// lock (it goes together with the `Open`), on a level change and on remove or rename.
    pub(crate) grants: HashMap<String, Duration>,
    /// "Connect" tokens not used yet (D3, Phase 27): they go away on lock, together with
    /// the `Open`.
    pub(crate) tokens: Vec<crate::ssh::ConnectToken>,
    /// The file's identity after the last read or write. `None`: it could not be read
    /// after writing, and the next write refuses it as "changed".
    identity: Option<FileIdentity>,
    generations: Generations,
}

pub(crate) struct State {
    pub(crate) open: Option<Open>,
    /// The vault's identity on lock, to verify on reopen (D39).
    known_identity: Option<FileIdentity>,
    pin_errors: u32,
    /// Last use with the vault unlocked, on the [`Clock::now`] clock (Phase 24).
    last_use: Option<Duration>,
    /// Consecutive wrong passwords of the agent role (D20, D62); mirror of the state
    /// file. Locking does not touch them.
    agent_failures: u32,
    /// Consecutive wrong passwords of the app role (D62), only in memory.
    app_failures: u32,
    /// The agent's next attempt is the first since startup: the wait counts from
    /// startup (D62).
    agent_first: bool,
    /// How many times the vault locked (Phase 27): the SSH agent of a session goes away
    /// when the number changes (D12).
    locks: Arc<AtomicU64>,
    /// `<rt>` (Phase 44, D6): on lock, the `ward playwright` storage states go away.
    rt_playwright: Option<PathBuf>,
    /// Groups of the `ward run` and `ward ssh` in progress (Phase 54 and 54c, S8): the
    /// lock kills them, like the SSH agent and the bridge.
    run_sessions: Arc<RunSessions>,
}

/// The process groups of the `ward run` and `ward ssh` in progress (Phase 54 and 54c,
/// S8). The command received the secret in the environment (or in an attachment), or the
/// SSH session was opened with it: locking the vault kills the group, so the secret does
/// not stay in use after the lock. The mutex here also orders the lock count
/// with the registration: a group registered after a lock it did not see
/// dies on registration.
#[derive(Default)]
pub(crate) struct RunSessions {
    groups: Mutex<Vec<Weak<Group>>>,
}

impl RunSessions {
    /// Registers the group of an authorized `ward run` with the lock count at
    /// `initial`. If the vault locked since then, kills the group at once and returns
    /// `false`.
    pub(crate) fn register(&self, group: &Arc<Group>, locks: &AtomicU64, initial: u64) -> bool {
        let mut v = self.groups.lock().unwrap_or_else(|e| e.into_inner());
        v.retain(|w| w.strong_count() > 0);
        if locks.load(Ordering::Acquire) != initial {
            group.kill_with_child_sessions();
            return false;
        }
        v.push(Arc::downgrade(group));
        true
    }

    /// Counts the lock and kills the registered groups (`SIGKILL` on the whole group and,
    /// since Phase 54c, on the group of each direct child with its own session, like the
    /// `ssh` that `sshpass` creates; a group already reaped gets no `kill`).
    fn lock(&self, locks: &AtomicU64) -> usize {
        let mut v = self.groups.lock().unwrap_or_else(|e| e.into_inner());
        locks.fetch_add(1, Ordering::AcqRel);
        let alive: Vec<Arc<Group>> = v.drain(..).filter_map(|w| w.upgrade()).collect();
        drop(v);
        for g in &alive {
            g.kill_with_child_sessions();
        }
        alive.len()
    }
}

impl State {
    pub(crate) fn lock(&mut self) {
        if let Some(a) = self.open.take() {
            // Phase 54 (S8): the `ward run` in progress die together with the lock.
            let killed = self.run_sessions.lock(&self.locks);
            if killed > 0 {
                warning!("{killed} ward run/ssh encerrado(s) ao travar (S8)");
            }
            // Phase 55 (S8): what left a session with `setsid` and became a child of the
            // daemon dies too, whether a session is in progress or not.
            let orphans = ward_platform::descendants::kill_orphans();
            if orphans.killed > 0 {
                warning!(
                    "{} processo(s) fora do grupo de um ward run/ssh encerrado(s) ao travar (S8)",
                    orphans.killed
                );
            }
            if orphans.remaining > 0 {
                warning!(
                    "{} processo(s) fora do grupo continuam vivos depois da trava (S8)",
                    orphans.remaining
                );
            }
            self.known_identity = a.identity;
            // `OpenVault` and `Entry` are zeroed on drop (data key, hash, secrets).
            drop(a);
            // D6: the `ward playwright` sessions do not survive the lock.
            if let Some(rt) = &self.rt_playwright {
                let n = crate::playwright::delete_sessions(rt);
                if n > 0 {
                    warning!("{n} sessão(ões) do ward playwright apagada(s) ao travar (D6)");
                }
            }
        }
        self.pin_errors = 0;
        self.last_use = None;
    }

    /// Locks because the file was swapped: forgets the identity, so the following unlock
    /// decides only by the generation.
    fn lock_changed(&mut self) {
        self.lock();
        self.known_identity = None;
    }
}

/// The daemon: locked/unlocked state and the operations.
pub struct Daemon {
    paths: Paths,
    state: Mutex<State>,
    /// One unlock attempt at a time in each role (D20, D62), from any connection, held
    /// during the wait and the attempt. One mutex per role: the agent's wait does not
    /// hold the app. The counters are in the [`State`].
    agent_turn: Mutex<()>,
    app_turn: Mutex<()>,
    /// Startup time on the [`Clock::now`] clock (D62).
    startup: Option<Duration>,
    clock: Box<dyn Clock>,
    /// Hook called right after each vault write, before the daemon looks at the result.
    /// Only the tests set it (V3.35: file swap right after the rename); the binary
    /// never.
    write_hook: Option<Hook>,
    /// Hook called at each [`MigrationPoint`] of the v4 → v5 migration (m2:DT5). Only the
    /// tests set it (m2:V4.3: death at each point); the binary never.
    migration_hook: Option<MigrationHook>,
    /// Approval engine (Phase 23).
    pub(crate) approval: Engine,
    /// `<rt>` (Phase 26): the `ward run` attachments are in `<rt>tmp/` (S2). `None`: the
    /// `ward run` of an entry with a file answers delivery failed.
    temp_root: Option<PathBuf>,
    /// The USB drives (udisks2) for the USB drives operation (Phase 46). `None`: no bus,
    /// answers USB drives unavailable.
    usb_drives: Option<Arc<dyn UsbDriveSource>>,
    /// Pokes the external copy thread (Phase 46), if enabled.
    poke: OnceLock<SyncSender<()>>,
    /// One external copy at a time.
    external_turn: Mutex<()>,
    /// `ssh`, `sshpass` and `sftp` of `ward ssh` (Phase 27): the system ones, by absolute
    /// path (no search in the `PATH`).
    ssh_programs: crate::ssh::SshPrograms,
    /// The Chrome of `ward playwright` (Phase 44): the installed one, by absolute path.
    chrome: PathBuf,
    /// `ward login` requests to the extension (Phase 43).
    logins: crate::login::Logins,
}

/// Test hook of [`Daemon::with_hook_after_write`].
pub type Hook = Box<dyn Fn() + Send + Sync>;

/// Point of the v4 → v5 migration on unlock (m2:DT5) where the test hook of
/// [`Daemon::with_migration_hook`] runs (the death tests of m2:V4.3 stop the daemon
/// there). The points inside the atomic write itself (temp file, write, fsync, rename,
/// directory fsync) are the core's, tested in `ward_core::atomic_write`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationPoint {
    /// The v4 is open and checked; nothing was written yet.
    BeforeBurn,
    /// The state has `burned` = generation + 1; the vault is still the v4 (before the
    /// temp file).
    BeforeWrite,
    /// The v5 is in place (rename and directory fsync done) and in the backup; the state
    /// still has the old `seen`.
    BeforeConfirm,
    /// The state has `seen` = the v5's generation; the unlock answers next.
    AfterConfirm,
}

/// Test hook of [`Daemon::with_migration_hook`].
pub type MigrationHook = Box<dyn Fn(MigrationPoint) + Send + Sync>;

enum WriteFailure {
    Response(ResponseError),
    /// The file changed outside Ward: lock.
    Changed,
}

fn problem(field: &str, reason: &str) -> ResponseError {
    ResponseError::Validation(vec![Problem {
        field: field.to_owned(),
        position: None,
        reason: reason.to_owned(),
    }])
}

/// The request's master password, in NFC (Phase 39c): unlock, create, change and restore
/// all go through here, so the app and the CLI open the same vault with the same text
/// composed or decomposed, without depending on the client.
fn password_from(s: &Secret) -> MasterPassword {
    MasterPassword::typed(Zeroizing::new(s.expose().to_vec()))
}

/// Fixed text (without value) of a new master password below the minimum, the same as
/// the app's.
const SHORT_PASSWORD: &str = "a senha mestra precisa de 12 caracteres ou mais";

/// New master password (create, change): the minimum also holds here, not only in the
/// app (Phase 39c; a client of the app role that skips the screen does not create a vault
/// with a short password).
fn new_password(s: &Secret, field: &str) -> Result<MasterPassword, ResponseError> {
    let password = password_from(s);
    if password.is_valid_new() {
        Ok(password)
    } else {
        warning!("senha mestra nova recusada: abaixo do mínimo de caracteres");
        Err(problem(field, SHORT_PASSWORD))
    }
}

/// Runs the migration test hook at `p`, if there is one (only the tests set it).
fn at_point(points: Option<&MigrationHook>, p: MigrationPoint) {
    if let Some(h) = points {
        h(p);
    }
}

/// Verifies the vault and writes the entries and the config of `a` (D39, D53). Every
/// write of an unlocked vault goes through here, the v4 → v5 migration too (m2:DT5,
/// which passes its `points`; the others pass `None`).
fn write_vault(
    paths: &Paths,
    a: &mut Open,
    hook: Option<&Hook>,
    points: Option<&MigrationHook>,
) -> Result<(), WriteFailure> {
    match files::identity(&paths.vault) {
        Ok(Some(id)) if Some(id) == a.identity => {}
        _ => return Err(WriteFailure::Changed),
    }
    // Would the set reopen? (repeated name, tunnel/jump broken by renaming or removing a
    // server in use). Without touching the disk nor burning a generation.
    if let Err(e) = body::serialize(a.vault.config(), 1, &a.entries) {
        return Err(WriteFailure::Response(match e {
            BodyError::TooLarge => problem("cofre", "o cofre passaria do limite de tamanho"),
            _ => problem(
                "nome",
                "outra entrada usa esta como túnel SSH ou servidor de salto",
            ),
        }));
    }
    let next = a
        .vault
        .generation()
        .checked_add(1)
        .ok_or(WriteFailure::Response(ResponseError::GenerationExhausted))?;
    let before = Generations {
        seen: a.generations.seen,
        burned: a.generations.burned.max(next),
    };
    at_point(points, MigrationPoint::BeforeBurn);
    // D53: `burned` goes to disk before the vault; without it, the vault is not written.
    if files::write_state(&paths.state, before).is_err() {
        warning!("gravação recusada: não deu para gravar o arquivo de estado");
        return Err(WriteFailure::Response(ResponseError::WriteFailed));
    }
    a.generations = before;
    at_point(points, MigrationPoint::BeforeWrite);
    let r = write_with_backup(paths, &mut a.vault, &a.entries);
    if let Some(g) = hook {
        g();
    }
    match r {
        Ok(id) => {
            at_point(points, MigrationPoint::BeforeConfirm);
            confirm_write(paths, a, id);
            at_point(points, MigrationPoint::AfterConfirm);
            Ok(())
        }
        Err(e) => {
            if let WriteError::Io {
                step: Step::FsyncDir,
                identity,
                ..
            } = e
            {
                // The rename happened: the file in place is the one Ward just wrote, with
                // the identity read from the temp file's fd (D62), without a `stat` of the
                // path.
                a.identity = identity;
            }
            warning!("falha ao gravar o cofre: {}", write_code(&e));
            Err(WriteFailure::Response(ResponseError::from(&e)))
        }
    }
}

/// Opens the backups dir next to the vault (Phase 45). A failure goes to the log, without
/// the path.
fn open_backups(paths: &Paths) -> Option<files::OpenDir> {
    let dir = files::backups_dir(&paths.vault)?;
    match files::open_backup_dir(&dir) {
        Ok(p) => Some(p),
        Err(_) => {
            warning!(
                "backup local: a pasta de backups não abre ou não é um diretório 0700 do usuário"
            );
            None
        }
    }
}

/// Fixed name of the backup error for the log (no path nor value).
fn backup_code(e: &BackupError) -> &'static str {
    match e {
        BackupError::UnsafeDir => "pasta insegura",
        BackupError::NotAuthentic => "cofre atual não autêntico",
        BackupError::Clock => "relógio fora do intervalo",
        BackupError::NamesExhausted => "nomes do segundo esgotados",
        BackupError::Write(g) => write_code(g),
        BackupError::Io(_) => "e/s na pasta",
    }
}

fn log_backup(r: &Result<Version, BackupError>) {
    match r {
        Ok(_) => debug!("backup local gravado"),
        Err(e) => warning!(
            "backup local não gravado ({}); o cofre foi gravado",
            backup_code(e)
        ),
    }
}

/// Writes the entries and, if the backups dir opens, the backup of the same version
/// (Phase 45). Without the dir, writes only the vault: the backup never prevents the
/// write.
fn write_with_backup(
    paths: &Paths,
    vault: &mut OpenVault,
    entries: &[Entry],
) -> Result<FileIdentity, WriteError> {
    match open_backups(paths) {
        Some(p) => {
            let g = atomic_write::write_entries_with_backup(
                &paths.vault,
                vault,
                entries,
                &p.backup_dir(),
                SystemTime::now(),
            )?;
            log_backup(&g.backup);
            Ok(g.identity)
        }
        None => atomic_write::write_entries(&paths.vault, vault, entries),
    }
}

/// After a write that succeeded: `seen` = written generation and the identity that the
/// write read from the temp file's fd (D62). No `stat` of the path after the rename: a
/// file swapped in the meantime gets another identity, and the next write refuses it as
/// "changed".
fn confirm_write(paths: &Paths, a: &mut Open, id: FileIdentity) {
    let g = a.vault.generation();
    a.generations = Generations {
        seen: g,
        burned: a.generations.burned.max(g),
    };
    if files::write_state(&paths.state, a.generations).is_err() {
        // The vault is on disk; the next write writes the state again first.
        warning!("não deu para atualizar o arquivo de estado depois de gravar");
    }
    a.identity = Some(id);
}

/// Operations that, when they succeed, may have something new for the USB drive
/// (Phase 46): the vault unlocked, was written or the external backup was enabled.
fn changes_external(op: Operation) -> bool {
    matches!(
        op,
        Operation::Unlock
            | Operation::CreateVault
            | Operation::SaveEntry
            | Operation::RemoveEntry
            | Operation::SetLevel
            | Operation::AnswerApproval
            | Operation::ChangePin
            | Operation::ChangePassword
            | Operation::LockTimeout
            | Operation::RestoreBackup
            | Operation::SetExternalBackup
            | Operation::SaveLogin
    )
}

/// Fixed name of the write error for the log (no path nor value).
fn write_code(e: &WriteError) -> &'static str {
    match e {
        WriteError::GenerationExhausted => "geração esgotada",
        WriteError::Rollback => "retrocesso",
        WriteError::NotVault => "não é cofre",
        WriteError::Format(_) => "formato",
        WriteError::Body(_) => "corpo",
        WriteError::InvalidDestination => "destino inválido",
        WriteError::Io { step, .. } => match step {
            Step::Read => "e/s ao ler",
            Step::CreateTemp => "e/s ao criar o temporário",
            Step::Write => "e/s ao escrever",
            Step::FsyncTemp => "e/s no fsync do temporário",
            Step::Rename => "e/s no rename",
            Step::FsyncDir => "e/s no fsync do diretório",
        },
    }
}

impl Daemon {
    /// Locked daemon. Reads the agent's wrong passwords from the state file (D62): the
    /// wait from before the restart still holds.
    pub fn new(paths: Paths, clock: Box<dyn Clock>) -> Self {
        let agent_failures = files::read_full_state(&paths.state)
            .map(|e| e.agent_failures)
            .unwrap_or(0);
        if agent_failures > 0 {
            warning!("{agent_failures} senha(s) errada(s) do agente antes da partida (D20)");
        }
        // Phase 49a: state temp files of an interrupted write (another process).
        match files::clean_state_temp_files(&paths.state) {
            Ok(0) => {}
            Ok(n) => warning!("{n} temporário(s) do arquivo de estado apagado(s) na partida"),
            Err(_) => warning!("não deu para limpar os temporários do arquivo de estado"),
        }
        let startup = clock.now();
        Self {
            paths,
            state: Mutex::new(State {
                open: None,
                known_identity: None,
                pin_errors: 0,
                last_use: None,
                agent_failures,
                app_failures: 0,
                agent_first: true,
                locks: Arc::new(AtomicU64::new(0)),
                rt_playwright: None,
                run_sessions: Arc::default(),
            }),
            agent_turn: Mutex::new(()),
            app_turn: Mutex::new(()),
            startup,
            clock,
            approval: Engine::without_app(),
            write_hook: None,
            migration_hook: None,
            temp_root: None,
            usb_drives: None,
            poke: OnceLock::new(),
            external_turn: Mutex::new(()),
            ssh_programs: crate::ssh::SshPrograms::from_system(),
            chrome: PathBuf::from(crate::playwright::CHROME),
            logins: crate::login::Logins::default(),
        }
    }

    /// Sets the source of the USB drives (udisks2; a list in the tests) of the
    /// USB drives operation (Phase 46).
    pub fn with_usb_drives(mut self, source: Arc<dyn UsbDriveSource>) -> Self {
        self.usb_drives = Some(source);
        self
    }

    /// Creates the channel of the external copy pokes and returns the read end (for the
    /// thread of [`crate::udisks::watch_copy`]). Only the first call enables it.
    pub fn enable_external_copy(&self) -> Option<Receiver<()>> {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        self.poke.set(tx).ok().map(|()| rx)
    }

    /// Asks its thread for an external copy (without waiting; pokes add up).
    pub fn poke_external(&self) {
        if let Some(tx) = self.poke.get() {
            match tx.try_send(()) {
                Ok(()) | Err(TrySendError::Full(())) => {}
                Err(TrySendError::Disconnected(())) => {
                    warning!("backup externo: a thread da cópia parou");
                }
            }
        }
    }

    /// UUID of the USB drive if the vault is unlocked and the external backup enabled.
    fn external_uuid(&self) -> Option<String> {
        let st = self.state();
        let b = st.open.as_ref()?.vault.config().backup();
        if b.enabled() {
            b.uuid().map(str::to_owned)
        } else {
            None
        }
    }

    /// Copies the local backup to the chosen USB drive (Phase 46, D17). `Ok(None)`: vault
    /// locked or external backup disabled (nothing is done, not even the udisks2 query).
    /// The state is not held during the copy: only the UUID is read from it.
    pub fn sync_external(
        &self,
        source: &dyn UsbDriveSource,
    ) -> Result<Option<BackupCopy>, ExternalError> {
        let _turn = self.external_turn.lock().unwrap_or_else(|e| e.into_inner());
        let Some(uuid) = self.external_uuid() else {
            return Ok(None);
        };
        let seen = source.usb_drives().ok_or(ExternalError::NoUdisks)?;
        let chosen = external::choose(&seen, &uuid)?.clone();
        let local = open_backups(&self.paths).ok_or(ExternalError::NoLocalBackup)?;
        // Phase 54: only versions sealed by the open vault's key go (locked in the middle:
        // none). The state is held only during the verification of each version.
        let generation = |b: &[u8]| {
            let st = self.state();
            backup::sealed_generation(&st.open.as_ref()?.vault, b)
        };
        external::copy(&local, &chosen, &uuid, source, &generation).map(Some)
    }

    /// Swaps the `ward ssh` programs (Phase 27). Only the crate's tests call it (with fake
    /// programs that record the argv); the binary always uses the system ones, and no
    /// socket request reaches here. Out of the release (M3 scrutiny): it only exists
    /// with the `test-hooks` feature, which turns on `test-mode` (V3.15, V9.2).
    #[cfg(feature = "test-hooks")]
    #[doc(hidden)]
    pub fn with_ssh_programs(mut self, p: crate::ssh::SshPrograms) -> Self {
        self.ssh_programs = p;
        self
    }

    /// Swaps the Chrome of `ward playwright` (Phase 44). Only the crate's tests call it
    /// (with a fake browser that speaks the protocol over the pipe); the binary always
    /// uses the installed one, and no socket request reaches here. Only with the
    /// `test-hooks` feature (V3.15, V9.2).
    #[cfg(feature = "test-hooks")]
    #[doc(hidden)]
    pub fn with_chrome(mut self, chrome: PathBuf) -> Self {
        self.chrome = chrome;
        self
    }

    /// Swaps the `ward login` deadlines (Phase 43). Only the crate's tests call it (short
    /// deadlines); the binary always uses the defaults, and no socket request reaches
    /// here. Only with the `test-hooks` feature (V3.15, V9.2).
    #[cfg(feature = "test-hooks")]
    #[doc(hidden)]
    pub fn with_login_deadlines(mut self, deadlines: crate::login::LoginDeadlines) -> Self {
        self.logins = crate::login::Logins::new(deadlines);
        self
    }

    pub(crate) fn logins(&self) -> &crate::login::Logins {
        &self.logins
    }

    pub(crate) fn chrome(&self) -> &std::path::Path {
        &self.chrome
    }

    pub(crate) fn ssh_programs(&self) -> &crate::ssh::SshPrograms {
        &self.ssh_programs
    }

    /// Ward's own `known_hosts` (D13), next to the vault: in the release,
    /// `~/.local/share/ward/known_hosts`; in the test daemon, inside `WARD_DEV_DIR`.
    /// Verified before each use (M4 audit): the dir belongs to the daemon and is 0700, the
    /// file belongs to the daemon, is regular (no link) and has no group/other
    /// permission; if it does not exist, it is created empty and 0600. Loose: refuses (the
    /// `ssh` does not run).
    pub(crate) fn known_hosts(&self) -> Result<PathBuf, &'static str> {
        let path = self
            .paths
            .vault
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.join("known_hosts"))
            .ok_or("known_hosts do Ward sem pasta")?;
        files::prepare_known_hosts(&path).map_err(|_| {
            "known_hosts do Ward recusado: a pasta tem de ser 0700 e o arquivo 0600, do usuário, sem link"
        })?;
        Ok(path)
    }

    /// The lock count and its value now, read with the state locked (Phase 27, D12): the
    /// SSH agent of a session goes away when the number changes.
    pub(crate) fn locks_now(&self, st: &State) -> (Arc<AtomicU64>, u64) {
        (st.locks.clone(), st.locks.load(Ordering::Acquire))
    }

    /// The `ward run` in progress (Phase 54, S8), to register the group of a session.
    pub(crate) fn run_sessions(&self, st: &State) -> Arc<RunSessions> {
        st.run_sessions.clone()
    }

    /// Sets the `<rt>` of the `ward run` temp files (Phase 26, S2): `<rt>tmp/`.
    pub fn with_temp_root(mut self, rt: PathBuf) -> Self {
        // D6 (Phase 44): the same `<rt>` keeps the storage states, deleted on lock.
        self.state
            .get_mut()
            .unwrap_or_else(|e| e.into_inner())
            .rt_playwright = Some(rt.clone());
        self.temp_root = Some(rt);
        self
    }

    /// The `<rt>` of the temp files, if any.
    pub(crate) fn temp_root(&self) -> Option<&std::path::Path> {
        self.temp_root.as_deref()
    }

    /// Sets the test hook called right after each vault write (V3.35). The binary does
    /// not call it, and no socket request reaches here. Out of the release (M3
    /// scrutiny): it only exists with the `test-hooks` feature (V3.15, V9.2).
    #[cfg(feature = "test-hooks")]
    #[doc(hidden)]
    pub fn with_hook_after_write(mut self, hook: Hook) -> Self {
        self.write_hook = Some(hook);
        self
    }

    /// Sets the test hook called at each [`MigrationPoint`] of the v4 → v5 migration on
    /// unlock (m2:DT5, m2:V4.3). It observes or stops the process; it changes nothing in
    /// the write. The binary does not call it, and no socket request reaches here. Only
    /// with the `test-hooks` feature (V3.15, V9.2).
    #[cfg(feature = "test-hooks")]
    #[doc(hidden)]
    pub fn with_migration_hook(mut self, hook: MigrationHook) -> Self {
        self.migration_hook = Some(hook);
        self
    }

    /// Swaps the approval engine (the daemon's opens the app through the launcher; the
    /// tests use a fake clock and app).
    pub fn with_engine(mut self, engine: Engine) -> Self {
        self.approval = engine;
        self
    }

    /// The approval engine (the server subscribes the app connection to it).
    pub fn engine(&self) -> &Engine {
        &self.approval
    }

    /// The state; if a thread fell in the middle of an operation, locks (fail closed).
    pub(crate) fn state(&self) -> MutexGuard<'_, State> {
        match self.state.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                let mut g = poisoned.into_inner();
                g.lock();
                self.state.clear_poison();
                g
            }
        }
    }

    /// Is the vault locked?
    pub fn is_locked(&self) -> bool {
        self.state().open.is_none()
    }

    /// Locks now (S8: the key and the entries are zeroed).
    pub fn lock(&self) {
        self.state().lock();
    }

    /// Locks if the vault went unused for the config's time (Phase 24, D30). Returns
    /// `true` if it locked now. An unreadable clock or one that went back also locks.
    pub fn lock_if_idle(&self) -> bool {
        let mut st = self.state();
        let Some(a) = st.open.as_ref() else {
            return false;
        };
        // Read with the state locked: a use from another thread does not stay "in the
        // future".
        let now = self.clock.now();
        let minutes = a.vault.config().lock_minutes();
        if !crate::lock::expired(st.last_use, now, minutes) {
            return false;
        }
        st.lock();
        warning!("{minutes} min sem uso: cofre travado");
        true
    }

    /// Records a use (Phase 24): only with the vault unlocked.
    pub(crate) fn record_use(&self) {
        let mut st = self.state();
        if st.open.is_some() {
            st.last_use = self.clock.now();
        }
    }

    /// Serves a request already decoded for the connection's role.
    pub fn serve(&self, role: Role, request: Request) -> Response {
        self.serve_from(&NoClient, role, request)
    }

    /// Like [`Daemon::serve`], with the connection's client (process chain for the
    /// approval window and whether it is still there).
    pub fn serve_from(&self, client: &dyn Client, role: Role, request: Request) -> Response {
        let op = request.operation();
        // The protocol already refused; here is the second layer.
        if !op.allowed(role) {
            return Response::Error(ResponseError::Unauthorized);
        }
        let r = self.serve_allowed(client, role, request);
        if crate::lock::counts_as_use(role, op, &r) {
            self.record_use();
        }
        if changes_external(op) && !matches!(r, Response::Error(_)) {
            self.poke_external();
        }
        debug!(
            "{} ({}): {}",
            op.name(),
            role.name(),
            match &r {
                Response::Error(e) => e.code(),
                other => other.kind().name(),
            }
        );
        r
    }

    fn serve_allowed(&self, client: &dyn Client, role: Role, request: Request) -> Response {
        let r = match request {
            Request::Status => {
                let locked = self.is_locked();
                Ok(Response::Status {
                    locked,
                    vault_exists: !locked || files::vault_exists(&self.paths.vault),
                    // D63: this connection's role, without any other data.
                    role,
                })
            }
            Request::Unlock { password } => self.unlock(role, &password),
            Request::Lock => {
                self.lock();
                Ok(Response::Ok)
            }
            Request::List => self.list(role),
            // D6 (Phase 44): only the storage state path comes back.
            Request::Playwright { .. } => self.playwright(client, &request),
            // Phase 43: the daemon takes the request to the extension; only `ok` or an
            // error come back.
            Request::Login { .. } => self.login(client, &request),
            Request::AwaitLogin => self.await_login(role),
            Request::FillLogin { id, origin } => self.fill_login(role, &id, &origin),
            Request::LoginResult { id, result } => self.login_result(role, &id, result),
            // Phase 23: level, unlock and approval. The run and the `ssh` are served
            // by the server's session (Phases 26/27); here the authorized use answers an
            // internal error.
            Request::Run(_) | Request::Ssh { .. } => self
                .authorize_use(client, &request)
                .and(Err(ResponseError::Internal)),
            Request::AnswerApproval {
                request,
                decision,
                pin,
                confirmed_entry,
            } => self.answer_approval(request, decision, pin.as_ref(), confirmed_entry),
            Request::OpenApp => self.approval.open_app().map(|()| Response::Ok),
            // There is no open session in this phase (Phases 26/27).
            Request::Stdin { .. } | Request::Resize(_) => Err(ResponseError::InvalidRequest),
            Request::CreateVault { password, pin } => self.create_vault(&password, &pin),
            Request::Detail { entry } => self.detail(&entry),
            Request::SaveEntry {
                previous,
                draft,
                keep_attachments,
            } => self.save_entry(previous.as_deref(), draft, &keep_attachments),
            Request::RemoveEntry { entry } => self.remove_entry(&entry),
            Request::SetLevel { entry, level, pin } => self.set_level(&entry, level, &pin),
            Request::ChangePin { current, new } => self.change_pin(&current, &new),
            Request::ChangePassword { current, new } => self.change_password(&current, &new),
            Request::LockTimeout { minutes } => self.lock_timeout(minutes),
            // D4, D65 b: only the app, without PIN, approval nor level.
            Request::Reveal { entry, target } => {
                self.app_value(role, Operation::Reveal, &entry, &target)
            }
            Request::Copy { entry, target } => {
                self.app_value(role, Operation::Copy, &entry, &target)
            }
            Request::Totp { entry } => self.app_totp(role, &entry),
            Request::Backups => self.backups(),
            Request::Config => self.settings(),
            Request::ExternalBackup => self.external_backup(),
            Request::SetExternalBackup { enabled, uuid } => {
                self.set_external_backup(enabled, uuid.as_deref())
            }
            Request::UsbDrives => self.usb_drives(),
            Request::RestoreBackup {
                version,
                password,
                pin,
            } => self.restore_backup(&version, &password, &pin),
            // D3 (Phase 27): single-use "Connect" token, only for the app.
            Request::IssueToken { entry } => self.issue_token(role, &entry),
            // Phase 40 (D9): suggest and fill by the tab's origin, only for the extension.
            Request::Suggest { origin } => self.suggest(role, &origin),
            // Phase 44c: on an `http` origin, only after the confirmation in the window.
            Request::Fill { entry, origin } => self.fill_from_popup(client, role, &entry, &origin),
            // Phase 42: new login seen by the extension, always "with approval" (D50, S9).
            Request::SaveLogin { draft } => self.save_login(role, draft),
        };
        r.unwrap_or_else(Response::Error)
    }

    /// Wait of the role's next attempt (D20, D62). The agent's first after startup waits
    /// what is left of the wait of the failures read from the state, counted from startup
    /// (unreadable clock: the whole wait).
    fn wait_for(&self, st: &mut State, agent: bool) -> Duration {
        if !agent {
            return unlock_wait(st.app_failures);
        }
        let wait = unlock_wait(st.agent_failures);
        if !std::mem::take(&mut st.agent_first) {
            return wait;
        }
        match (self.startup, self.clock.now()) {
            (Some(p), Some(now)) if now >= p => wait.saturating_sub(now - p),
            _ => wait,
        }
    }

    fn unlock(&self, role: Role, password: &Secret) -> Result<Response, ResponseError> {
        if !self.is_locked() {
            return Ok(Response::Ok);
        }
        // D62: the extension role does not unlock (the protocol refuses); any other that
        // is not the app counts as the agent.
        let agent = role != Role::App;
        let turn = if agent {
            &self.agent_turn
        } else {
            &self.app_turn
        };
        let _turn = turn.lock().unwrap_or_else(|e| e.into_inner());
        // D20: the wait comes before looking at the password, right or wrong.
        let wait = {
            let mut st = self.state();
            self.wait_for(&mut st, agent)
        };
        self.clock.sleep(wait);
        let mut st = self.state();
        if st.open.is_some() {
            return Ok(Response::Ok);
        }
        let (bytes, id) = match files::read_vault(&self.paths.vault) {
            Ok(Some(x)) => x,
            Ok(None) => return Err(ResponseError::VaultNotFound),
            Err(_) => {
                warning!("destravar recusado: o arquivo do cofre não pôde ser lido");
                return Err(ResponseError::InvalidVault);
            }
        };
        let (g, failures_on_disk) = match files::read_full_state(&self.paths.state) {
            Ok(e) => (e.generations, e.agent_failures),
            Err(_) => {
                warning!("destravar recusado: arquivo de estado ausente ou inválido (D39)");
                return Err(ResponseError::Rollback);
            }
        };
        let password = password_from(password);
        let (vault, entries, body_read) =
            match body::open_entries_with_version(&bytes, &password, g.seen, g.burned) {
                Ok(x) => x,
                Err(e) => {
                    match e {
                        OpenError::Format(_) => self.count_failure(&mut st, agent, g),
                        OpenError::Rollback => {
                            warning!("destravar recusado: o cofre no disco é mais antigo (D39)")
                        }
                        OpenError::Body(_) => warning!("destravar recusado: corpo inválido"),
                    }
                    return Err(ResponseError::from(&e));
                }
            };
        drop(password);
        drop(bytes);
        let read = body_read.generation;
        if let Some(known) = st.known_identity.take()
            && known != id
        {
            warning!("destravar recusado: o arquivo do cofre mudou fora do Ward (D39)");
            return Err(ResponseError::VaultChanged);
        }
        let new = Generations {
            seen: g.seen.max(read),
            burned: g.burned.max(read),
        };
        // D62: the right unlock also resets the agent's failures on disk.
        if (new != g || st.agent_failures != 0 || failures_on_disk != 0)
            && files::write_state(&self.paths.state, new).is_err()
        {
            warning!("destravar recusado: não deu para atualizar o arquivo de estado");
            return Err(ResponseError::WriteFailed);
        }
        match atomic_write::clean_temp_files(&self.paths.vault) {
            Ok(0) => {}
            Ok(n) => warning!("{n} temporário(s) de gravação interrompida apagado(s)"),
            Err(_) => warning!("não deu para limpar os temporários do cofre"),
        }
        // The password was right: the counters go to 0 (on disk they already went), even
        // if the migration below fails.
        st.agent_failures = 0;
        st.app_failures = 0;
        st.pin_errors = 0;
        let mut open = Open {
            vault,
            entries,
            grants: HashMap::new(),
            tokens: Vec::new(),
            identity: Some(id),
            generations: new,
        };
        if body_read.is_legacy() {
            self.migrate(&mut st, &mut open)?;
        }
        st.last_use = self.clock.now();
        st.open = Some(open);
        Ok(Response::Ok)
    }

    /// m2:DT5: the vault just opened has a v4 body (0.1.x). It is written again as v5
    /// through the normal write path ([`write_vault`]: `burned` in the state first, temp
    /// file with `O_EXCL`, fsync, rename, directory fsync, version in the local backup,
    /// `seen` after), with generation + 1, before the unlock answers. The v4 versions
    /// already in the backup stay. A v5 body never comes here: opening it again writes
    /// nothing.
    ///
    /// On error the vault stays locked and the unlock answers the error; nothing is
    /// deleted. Before the rename the file on disk is the v4, intact, and the next
    /// unlock tries again (the burned generation is not reused, D53). If only the
    /// directory fsync failed, the v5 is already in place and the next unlock opens it
    /// without migrating. The log has only fixed text (S13).
    fn migrate(&self, st: &mut State, open: &mut Open) -> Result<(), ResponseError> {
        warning!("cofre no formato da 0.1.x (corpo v4): gravando no formato novo (v5)");
        match write_vault(
            &self.paths,
            open,
            self.write_hook.as_ref(),
            self.migration_hook.as_ref(),
        ) {
            Ok(()) => {
                warning!("cofre migrado para o corpo v5");
                Ok(())
            }
            Err(WriteFailure::Response(e)) => {
                // The identity is the v4's (nothing renamed) or, after the rename, the
                // v5's that Ward put in place (`write_vault` took it from the temp file's
                // fd): the next unlock checks the file against it.
                st.known_identity = open.identity;
                warning!("migração para o corpo v5 falhou: cofre travado, nada apagado");
                Err(e)
            }
            Err(WriteFailure::Changed) => {
                warning!("migração recusada: o arquivo do cofre mudou fora do Ward (D39)");
                st.known_identity = None;
                Err(ResponseError::VaultChanged)
            }
        }
    }

    /// A wrong password (D20, D62). The agent's goes to the state file, with fsync,
    /// before the error response goes out; if it cannot be written, it stays in memory
    /// and goes to the log.
    fn count_failure(&self, st: &mut State, agent: bool, g: Generations) {
        if !agent {
            st.app_failures = st.app_failures.saturating_add(1);
            return;
        }
        st.agent_failures = st.agent_failures.saturating_add(1);
        let e = StateFile {
            generations: g,
            agent_failures: st.agent_failures,
        };
        if files::write_full_state(&self.paths.state, e).is_err() {
            warning!("não deu para gravar a senha errada do agente no arquivo de estado (D62)");
        }
    }

    fn list(&self, role: Role) -> Result<Response, ResponseError> {
        let st = self.state();
        let a = st.open.as_ref().ok_or(ResponseError::Locked)?;
        let items = a
            .entries
            .iter()
            .filter(|e| role == Role::App || e.level() != AccessLevel::Blocked)
            .map(|e| ListItem::from_entry(e, role))
            .collect();
        Ok(Response::List(items))
    }

    /// Reveal and copy (D4, D65 b): the value of an entry's secret for the app.
    /// Only the app role (the protocol table and `serve_from` already verified; this is
    /// the third layer), with the vault unlocked, without PIN, without approval and
    /// without looking at the level: the blocked entry can also be revealed in the app.
    /// The log carries the operation, the entry's name (from the vault, already
    /// validated) and the field, never the value (S13).
    fn app_value(
        &self,
        role: Role,
        op: Operation,
        name: &str,
        target: &Target,
    ) -> Result<Response, ResponseError> {
        if role != Role::App {
            return Err(ResponseError::Unauthorized);
        }
        let st = self.state();
        let a = st.open.as_ref().ok_or(ResponseError::Locked)?;
        let e = a
            .entries
            .iter()
            .find(|e| e.name() == name)
            .ok_or(ResponseError::EntryNotFound)?;
        let value = target.secret_in(e).ok_or(ResponseError::FieldNotFound)?;
        // The type's field already went through the protocol's fixed list; the extra's
        // name comes from the entry and goes out escaped (no line break nor control in the
        // log).
        match target {
            Target::Field(c) => warning!("{} {} {c}", op.name(), e.name()),
            Target::Extra(x) => warning!("{} {} extra {x:?}", op.name(), e.name()),
        }
        Ok(Response::Secret(Secret::from_bytes(
            value.expose().to_vec(),
        )))
    }

    /// The app's `totp` (D65 b): the current code of the `login` entry's seed, by the
    /// daemon's clock, with the same rules as the reveal. Without a seed: "field not
    /// found". The code is not a vault secret (R8), but it goes in a `Zeroizing`.
    fn app_totp(&self, role: Role, name: &str) -> Result<Response, ResponseError> {
        if role != Role::App {
            return Err(ResponseError::Unauthorized);
        }
        let st = self.state();
        let a = st.open.as_ref().ok_or(ResponseError::Locked)?;
        let e = a
            .entries
            .iter()
            .find(|e| e.name() == name)
            .ok_or(ResponseError::EntryNotFound)?;
        let seed = Target::Field("totp_semente".to_owned())
            .secret_in(e)
            .ok_or(ResponseError::FieldNotFound)?;
        let now = self.clock.unix_seconds().ok_or(ResponseError::Internal)?;
        let c = Totp::from_seed(seed)
            .map_err(|_| ResponseError::Internal)?
            .code_at(now);
        warning!("totp {}", e.name());
        Ok(Response::TotpCode {
            code: Zeroizing::new(c.code().to_owned()),
            remaining: c.remaining(),
        })
    }

    /// Suggest (Phase 40, D9): the `login` entries whose URL has the same origin as the
    /// tab (scheme, host and port, [`Origin`]). Only the extension role (third layer, as
    /// in [`Daemon::app_value`]). No secret: name and username. The level does not count
    /// (it regulates the agent; whoever uses the extension is the user), so the blocked
    /// one also shows up. An origin that the strict parser refuses matches nothing: empty
    /// list. The origin comes from the extension (untrusted) and never goes to the log.
    fn suggest(&self, role: Role, origin: &str) -> Result<Response, ResponseError> {
        if role != Role::Extension {
            return Err(ResponseError::Unauthorized);
        }
        let st = self.state();
        let a = st.open.as_ref().ok_or(ResponseError::Locked)?;
        let Some(tab) = Origin::from_url(origin) else {
            return Ok(Response::Suggestions(Vec::new()));
        };
        let mut items: Vec<Suggestion> = a
            .entries
            .iter()
            .filter_map(|e| match e.data() {
                Data::Login(l) if Origin::from_url(&l.url).as_ref() == Some(&tab) => {
                    Some(Suggestion {
                        name: e.name().to_owned(),
                        username: l.username.clone(),
                        environment: e.environment(),
                        totp: l.totp_seed.is_some(),
                    })
                }
                _ => None,
            })
            .collect();
        items.sort_by(|x, y| x.name.cmp(&y.name));
        debug!("sugerir: {} entrada(s)", items.len());
        Ok(Response::Suggestions(items))
    }

    /// Fill from the extension popup (Phase 44c, D9): on an entry whose URL is local
    /// (`http` of `localhost`/`127.0.0.1` and, since Phase 44e, every loopback host on
    /// `https`), the values only go out after the user confirms the
    /// origin in the app window (without PIN: whoever clicked the popup is the user, and
    /// the level does not count for the extension). Without the app, "approval
    /// unavailable". A different origin answers before asking anything. On an outside
    /// `https`, [`Daemon::fill`] directly.
    fn fill_from_popup(
        &self,
        client: &dyn Client,
        role: Role,
        name: &str,
        origin: &str,
    ) -> Result<Response, ResponseError> {
        if role != Role::Extension {
            return Err(ResponseError::Unauthorized);
        }
        let request = {
            let st = self.state();
            let a = st.open.as_ref().ok_or(ResponseError::Locked)?;
            let e = a
                .entries
                .iter()
                .find(|e| e.name() == name)
                .ok_or(ResponseError::EntryNotFound)?;
            let Data::Login(l) = e.data() else {
                return Err(ResponseError::EntryNotFound);
            };
            let matches = match (Origin::from_url(&l.url), Origin::from_url(origin)) {
                (Some(x), Some(y)) => x == y,
                _ => false,
            };
            if !matches {
                warning!("preencher {} recusado: origem diferente (D9)", e.name());
                return Err(ResponseError::OriginMismatch);
            }
            crate::usage::origin_to_confirm(e, Operation::Fill).map(|o| ApprovalRequest {
                id: 0,
                entries: vec![e.name().to_owned()],
                environment: e.environment(),
                operation: Operation::Fill,
                command: String::new(),
                directory: String::new(),
                chain: crate::usage::display_chain(client),
                origin: o,
                confirmation: true,
                hidden_entry: false,
            })
        };
        if let Some(p) = request {
            warning!("preencher {name}: origem local, confirmação na janela (D9)");
            self.approval.request_approval(p, client)?;
        }
        // The entry may have changed during the wait: `fill` verifies everything again.
        self.fill(role, name, origin)
    }

    /// Fill (Phase 40, D9): username, password and the current TOTP code of the
    /// `login` entry, only if the tab's origin is the one of the entry's URL. Only the
    /// extension role. A missing entry and one that is not `login` answer the same; a
    /// different origin answers origin mismatch without any value. The log carries the
    /// entry's name (from the vault), never the origin nor the values (S13).
    pub(crate) fn fill(
        &self,
        role: Role,
        name: &str,
        origin: &str,
    ) -> Result<Response, ResponseError> {
        if role != Role::Extension {
            return Err(ResponseError::Unauthorized);
        }
        let st = self.state();
        let a = st.open.as_ref().ok_or(ResponseError::Locked)?;
        let e = a
            .entries
            .iter()
            .find(|e| e.name() == name)
            .ok_or(ResponseError::EntryNotFound)?;
        let Data::Login(l) = e.data() else {
            return Err(ResponseError::EntryNotFound);
        };
        let matches = match (Origin::from_url(&l.url), Origin::from_url(origin)) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        };
        if !matches {
            warning!("preencher {} recusado: origem diferente (D9)", e.name());
            return Err(ResponseError::OriginMismatch);
        }
        let totp = match &l.totp_seed {
            Some(seed) => {
                let now = self.clock.unix_seconds().ok_or(ResponseError::Internal)?;
                let c = Totp::from_seed(seed)
                    .map_err(|_| ResponseError::Internal)?
                    .code_at(now);
                Some(Zeroizing::new(c.code().to_owned()))
            }
            None => None,
        };
        warning!("preencher {}", e.name());
        Ok(Response::Fill {
            username: l.username.clone(),
            password: Secret::from_bytes(l.password.expose().to_vec()),
            totp,
        })
    }

    fn create_vault(&self, password: &Secret, pin: &Secret) -> Result<Response, ResponseError> {
        let password = new_password(password, "senha")?;
        let mut st = self.state();
        if st.open.is_some() || files::vault_exists(&self.paths.vault) {
            return Err(ResponseError::VaultAlreadyExists);
        }
        let hash = PinHash::create(pin.expose()).map_err(|_| ResponseError::InvalidRequest)?;
        // D62: an old state (deleted vault) does not go down. The new vault is born at
        // generation `burned` + 1, and a copy of a previous vault stays below `seen`.
        let old = match files::read_state(&self.paths.state) {
            Ok(g) => Some(g),
            Err(files::StateError::Missing) => None,
            Err(files::StateError::Invalid) => {
                warning!("criar cofre recusado: arquivo de estado inválido (D62)");
                return Err(ResponseError::WriteFailed);
            }
        };
        let q = old.map_or(0, |g| g.burned);
        let first = q.checked_add(1).ok_or(ResponseError::GenerationExhausted)?;
        if old.is_some() {
            warning!("criar cofre: estado anterior na geração {q}; o cofre novo nasce na {first}");
        }
        let mut vault = OpenVault::new(&password, VaultConfig::new(hash))
            .map_err(|_| ResponseError::Internal)?
            .after_generation(q);
        if let Some(dir) = self.paths.vault.parent() {
            use std::os::unix::fs::DirBuilderExt;
            let _ = std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir);
        }
        // First use: the state is born (or continues) here (D39), with the first
        // generation burned before.
        let before = Generations {
            seen: old.map_or(0, |g| g.seen),
            burned: first,
        };
        if files::write_state(&self.paths.state, before).is_err() {
            warning!("criar cofre: não deu para gravar o arquivo de estado");
            return Err(ResponseError::WriteFailed);
        }
        let r = write_with_backup(&self.paths, &mut vault, &[]);
        if let Some(g) = &self.write_hook {
            g();
        }
        let id = match r {
            Ok(id) => id,
            Err(e) => {
                warning!("criar cofre: falha ao gravar ({})", write_code(&e));
                return Err(ResponseError::WriteFailed);
            }
        };
        let mut a = Open {
            vault,
            entries: Vec::new(),
            grants: HashMap::new(),
            tokens: Vec::new(),
            identity: None,
            generations: before,
        };
        confirm_write(&self.paths, &mut a, id);
        st.known_identity = None;
        st.agent_failures = 0;
        st.app_failures = 0;
        st.pin_errors = 0;
        st.last_use = self.clock.now();
        st.open = Some(a);
        Ok(Response::Ok)
    }

    fn detail(&self, name: &str) -> Result<Response, ResponseError> {
        let st = self.state();
        let a = st.open.as_ref().ok_or(ResponseError::Locked)?;
        let e = a
            .entries
            .iter()
            .find(|e| e.name() == name)
            .ok_or(ResponseError::EntryNotFound)?;
        Ok(Response::Detail(EntryDetail::from_entry(e)))
    }

    /// Writes; on error undoes the change in memory with `undo`. File swapped: locks.
    pub(crate) fn write_or_undo(
        &self,
        st: &mut State,
        undo: impl FnOnce(&mut Open),
    ) -> Result<Response, ResponseError> {
        let Some(a) = st.open.as_mut() else {
            return Err(ResponseError::Locked);
        };
        match write_vault(&self.paths, a, self.write_hook.as_ref(), None) {
            Ok(()) => Ok(Response::Ok),
            Err(WriteFailure::Response(r)) => {
                undo(a);
                Err(r)
            }
            Err(WriteFailure::Changed) => {
                warning!("gravação recusada: o arquivo do cofre mudou fora do Ward; travado (D39)");
                st.lock_changed();
                Err(ResponseError::VaultChanged)
            }
        }
    }

    /// UTC seconds of the daemon's clock to stamp an entry (D65 d). An unreadable clock or
    /// one outside the range of [`Dates`]: `None`, and the write goes on without a new
    /// stamp (the dates are only information; no security decision uses them).
    fn now_for_dates(&self) -> Option<u64> {
        let now = self.clock.unix_seconds().and_then(Dates::instant);
        if now.is_none() {
            warning!("relógio de parede ilegível: a entrada é gravada sem data nova (D65 d)");
        }
        now
    }

    /// The entry's dates after a change at `now` (the same without a clock).
    fn modified(dates: Dates, now: Option<u64>) -> Dates {
        now.and_then(|a| dates.modified(a)).unwrap_or(dates)
    }

    fn save_entry(
        &self,
        previous: Option<&str>,
        mut draft: Draft,
        keep_attachments: &[String],
    ) -> Result<Response, ResponseError> {
        let now = self.now_for_dates();
        let mut st = self.state();
        let a = st.open.as_mut().ok_or(ResponseError::Locked)?;
        // Phase 39b (V5.12): the kept attachments are copied from the edited entry in
        // here, before the whole validation (D29 sum, names, mask budget).
        if !keep_attachments.is_empty() {
            let name = previous.ok_or(ResponseError::InvalidRequest)?;
            let old = a
                .entries
                .iter()
                .find(|e| e.name() == name)
                .ok_or(ResponseError::EntryNotFound)?;
            ward_core::protocol::merge_kept_attachments(&mut draft, old, keep_attachments)?;
        }
        let mut new = draft
            .validate()
            .map_err(|e| ResponseError::from_validation(&e))?;
        let old = match previous {
            None => {
                // D50: every new entry is born "with approval"; another level only through
                // the level change, with the PIN (S9).
                new.set_level(AccessLevel::Approval);
                // D65 d: the new entry gets both dates, from the daemon's clock.
                new.set_dates(now.and_then(Dates::new_stamp).unwrap_or(Dates::NONE));
                new.verify_in_vault(a.entries.iter())
                    .map_err(|e| ResponseError::from_validation(&e))?;
                None
            }
            Some(name) => {
                let i = a
                    .entries
                    .iter()
                    .position(|e| e.name() == name)
                    .ok_or(ResponseError::EntryNotFound)?;
                // Editing does not change the level. Renaming removes the 1 hour grant
                // (D2).
                new.set_level(a.entries[i].level());
                // D65 d: the creation stays the edited entry's; only the modification
                // changes.
                new.set_dates(Self::modified(a.entries[i].dates(), now));
                if new.name() != name {
                    a.grants.remove(name);
                }
                new.verify_in_vault(a.entries.iter().filter(|e| e.name() != name))
                    .map_err(|e| ResponseError::from_validation(&e))?;
                Some(a.entries.remove(i))
            }
        };
        let new_name = new.name().to_owned();
        let pos = a
            .entries
            .binary_search_by(|e| e.name().cmp(&new_name))
            .unwrap_or_else(|p| p);
        a.entries.insert(pos, new);
        self.write_or_undo(&mut st, move |a| {
            if let Some(i) = a.entries.iter().position(|e| e.name() == new_name) {
                drop(a.entries.remove(i));
            }
            if let Some(old) = old {
                let p = a
                    .entries
                    .binary_search_by(|e| e.name().cmp(old.name()))
                    .unwrap_or_else(|p| p);
                a.entries.insert(p, old);
            }
        })
    }

    /// Save login (Phase 42, V6.8): the new login that the extension saw being sent,
    /// with name, group and environment from the popup. Only the extension role; only a
    /// new entry (it never replaces an existing one: the repeated name is a validation
    /// error) and only the fields of the popup form — URL, username and password. TOTP,
    /// extras, attachments, project and tags are refused without writing anything (the
    /// app is the place to complete the entry). The level is always "with approval" (D50,
    /// S9), as in the app's entry save. The log carries neither the name nor the
    /// URL (they came from the page).
    fn save_login(&self, role: Role, draft: Draft) -> Result<Response, ResponseError> {
        if role != Role::Extension {
            return Err(ResponseError::Unauthorized);
        }
        let DataDraft::Login { totp_seed, .. } = &draft.data else {
            return Err(ResponseError::InvalidRequest);
        };
        if totp_seed.is_some()
            || !draft.extras.is_empty()
            || !draft.attachments.is_empty()
            || !draft.project.is_empty()
            || !draft.tags.is_empty()
        {
            return Err(ResponseError::InvalidRequest);
        }
        let r = self.save_entry(None, draft, &[]);
        if r.is_ok() {
            warning!("salvar_login: login novo gravado pela extensão (com aprovação)");
        }
        r
    }

    fn remove_entry(&self, name: &str) -> Result<Response, ResponseError> {
        let mut st = self.state();
        let a = st.open.as_mut().ok_or(ResponseError::Locked)?;
        let i = a
            .entries
            .iter()
            .position(|e| e.name() == name)
            .ok_or(ResponseError::EntryNotFound)?;
        let old = a.entries.remove(i);
        a.grants.remove(name);
        self.write_or_undo(&mut st, move |a| a.entries.insert(i, old))
    }

    /// Verifies the PIN (S9): a hit resets the count; the 5th error in a row locks.
    pub(crate) fn verify_pin(st: &mut State, pin: &Secret) -> Result<(), ResponseError> {
        let a = st.open.as_ref().ok_or(ResponseError::Locked)?;
        if a.vault.config().verify_pin(pin.expose()) {
            st.pin_errors = 0;
            return Ok(());
        }
        st.pin_errors = st.pin_errors.saturating_add(1);
        if st.pin_errors >= MAX_PIN_ERRORS {
            warning!("{MAX_PIN_ERRORS} erros de PIN seguidos: cofre travado (S9)");
            st.lock();
        }
        Err(ResponseError::WrongPin)
    }

    fn set_level(
        &self,
        name: &str,
        level: AccessLevel,
        pin: &Secret,
    ) -> Result<Response, ResponseError> {
        let now = self.now_for_dates();
        let mut st = self.state();
        Self::verify_pin(&mut st, pin)?;
        let a = st.open.as_mut().ok_or(ResponseError::Locked)?;
        let e = a
            .entries
            .iter_mut()
            .find(|e| e.name() == name)
            .ok_or(ResponseError::EntryNotFound)?;
        let before = e.level();
        let dates_before = e.dates();
        // D65 d: changing the level is modifying the entry.
        let dates = Self::modified(dates_before, now);
        e.set_level(level);
        e.set_dates(dates);
        // Going back is changing the level: the 1 hour grant ends (D2).
        a.grants.remove(name);
        let name = name.to_owned();
        self.write_or_undo(&mut st, move |a| {
            if let Some(e) = a.entries.iter_mut().find(|e| e.name() == name) {
                e.set_level(before);
                e.set_dates(dates_before);
            }
        })
    }

    fn change_pin(&self, current: &Secret, new: &Secret) -> Result<Response, ResponseError> {
        let mut st = self.state();
        Self::verify_pin(&mut st, current)?;
        let hash = PinHash::create(new.expose()).map_err(|_| ResponseError::InvalidRequest)?;
        let a = st.open.as_mut().ok_or(ResponseError::Locked)?;
        a.vault.config_mut().change_pin_hash(hash);
        match self.write_or_undo(&mut st, |_| {}) {
            Ok(r) => Ok(r),
            Err(e) => {
                // The old hash was already zeroed: locks, and the unlock rereads the PIN
                // from disk.
                warning!("troca de PIN não gravada: cofre travado");
                st.lock();
                Err(e)
            }
        }
    }

    fn lock_timeout(&self, minutes: u32) -> Result<Response, ResponseError> {
        let mut st = self.state();
        let a = st.open.as_mut().ok_or(ResponseError::Locked)?;
        let before = a.vault.config().lock_minutes();
        a.vault
            .config_mut()
            .set_lock_minutes(minutes)
            .map_err(|_| ResponseError::InvalidRequest)?;
        self.write_or_undo(&mut st, move |a| {
            let _ = a.vault.config_mut().set_lock_minutes(before);
        })
    }

    /// Changes the master password (D53): `burned` first, `ward_core`'s password change
    /// with (`seen`, current generation), and an error after the password was verified
    /// locks the vault.
    fn change_password(&self, current: &Secret, new: &Secret) -> Result<Response, ResponseError> {
        // Before anything: a short new password neither burns a generation nor touches
        // the disk.
        let new = new_password(new, "nova")?;
        let mut st = self.state();
        let a = st.open.as_mut().ok_or(ResponseError::Locked)?;
        // Phase 55: the file read for the change is the one of the verified identity, by
        // the same fd (`O_NOFOLLOW | O_NONBLOCK`, only the user's regular file): a link or
        // a FIFO swapped in its place is neither followed nor holds the thread (and the
        // daemon's state).
        let file = match files::read_vault(&self.paths.vault) {
            Ok(Some((bytes, id))) if Some(id) == a.identity => bytes,
            _ => {
                warning!("troca de senha recusada: o arquivo do cofre mudou fora do Ward; travado");
                st.lock_changed();
                return Err(ResponseError::VaultChanged);
            }
        };
        let generation = a.vault.generation();
        let next = generation
            .checked_add(1)
            .ok_or(ResponseError::GenerationExhausted)?;
        let before = Generations {
            seen: a.generations.seen,
            burned: a.generations.burned.max(next),
        };
        if files::write_state(&self.paths.state, before).is_err() {
            warning!("troca de senha recusada: não deu para gravar o arquivo de estado");
            return Err(ResponseError::WriteFailed);
        }
        a.generations = before;
        let current = password_from(current);
        let r = match open_backups(&self.paths) {
            Some(p) => atomic_write::change_password_with_backup_from_bytes(
                &self.paths.vault,
                &file,
                &current,
                &new,
                a.generations.seen,
                generation,
                &p.backup_dir(),
                SystemTime::now(),
            )
            .map(|(changed, g)| {
                log_backup(&g.backup);
                (changed, g.identity)
            }),
            None => atomic_write::change_password_from_bytes(
                &self.paths.vault,
                &file,
                &current,
                &new,
                a.generations.seen,
                generation,
            ),
        };
        drop((current, new));
        if let Some(g) = &self.write_hook {
            g();
        }
        match r {
            Ok((changed, id)) => {
                // The old vault (old key) is zeroed here.
                a.vault = changed;
                confirm_write(&self.paths, a, id);
                Ok(Response::Ok)
            }
            // D52: in the password change, `Format` is the wrong current password;
            // nothing was sealed.
            Err(WriteError::Format(_)) => Err(ResponseError::WrongPassword),
            Err(e) => {
                // D53: the stored vault did not go through the change's burn; locks, and
                // the unlock reopens with the state's `burned`.
                warning!("troca de senha falhou ({}): cofre travado", write_code(&e));
                st.lock_changed();
                Err(ResponseError::from(&e))
            }
        }
    }

    /// Lock time and whether the external backup is enabled (Phase 38), for the Settings
    /// screen. Only with the vault unlocked and only through the app; never the PIN hash
    /// nor the USB drive's UUID.
    fn settings(&self) -> Result<Response, ResponseError> {
        let st = self.state();
        let a = st.open.as_ref().ok_or(ResponseError::Locked)?;
        let c = a.vault.config();
        // Phase 49a: the screen warns when the local backup dir does not open or is not
        // safe (the vault is written without a backup). Only a yes/no: no path.
        let local_backup = open_backups(&self.paths).is_some_and(|p| p.backup_dir().list().is_ok());
        Ok(Response::Config {
            lock_minutes: c.lock_minutes(),
            external_backup: c.backup().enabled(),
            local_backup,
        })
    }

    /// Versions of the local backup (Phase 45), from the newest to the oldest. Only with
    /// the vault unlocked and only through the app (the table and [`Daemon::serve_from`]
    /// verify).
    ///
    /// Phase 49a: without the versions dated after now + margin (the name's date is from
    /// whoever created the file), and with the sealed generation of each version under
    /// the open vault's key ([`backup::sealed_generation`], authenticated), which becomes
    /// the verified and out-of-order marks ([`backup::out_of_order`]): an old copy
    /// with a new name comes out marked. The generation does not go in the response (D39).
    fn backups(&self) -> Result<Response, ResponseError> {
        let st = self.state();
        let a = st.open.as_ref().ok_or(ResponseError::Locked)?;
        let dir = open_backups(&self.paths).ok_or(ResponseError::BackupUnavailable)?;
        let versions = dir
            .backup_dir()
            .list_until(SystemTime::now())
            .map_err(|e| {
                warning!("backup local: não deu para listar ({})", backup_code(&e));
                ResponseError::BackupUnavailable
            })?;
        let with_generation: Vec<(Version, Option<u64>)> = versions
            .into_iter()
            .take(MAX_BACKUPS)
            .map(|v| {
                let generation = match files::read_vault(&dir.backup_dir().path(v)) {
                    Ok(Some((b, _))) => backup::sealed_generation(&a.vault, &b),
                    _ => None,
                };
                (v, generation)
            })
            .collect();
        let marks = backup::out_of_order(&with_generation);
        let list = with_generation
            .iter()
            .zip(marks)
            .map(|((v, g), out)| BackupVersion {
                name: v.name(),
                verified: g.is_some(),
                out_of_order: out,
            })
            .collect();
        Ok(Response::Backups(list))
    }

    /// External backup config (Phase 46): only with the vault unlocked.
    fn external_backup(&self) -> Result<Response, ResponseError> {
        let st = self.state();
        let a = st.open.as_ref().ok_or(ResponseError::Locked)?;
        let b = a.vault.config().backup();
        Ok(Response::ExternalBackup {
            enabled: b.enabled(),
            uuid: b.uuid().map(str::to_owned),
        })
    }

    /// Enables or disables the external backup (Phase 46, D30): writes the config in the
    /// vault through the normal write; an error undoes. Disabling deletes nothing from the
    /// USB drive.
    fn set_external_backup(
        &self,
        enabled: bool,
        uuid: Option<&str>,
    ) -> Result<Response, ResponseError> {
        let new = ExternalBackup::new(enabled, uuid).map_err(|_| ResponseError::InvalidRequest)?;
        let mut st = self.state();
        let a = st.open.as_mut().ok_or(ResponseError::Locked)?;
        let before = a.vault.config().backup().clone();
        a.vault.config_mut().set_backup(new);
        let r = self.write_or_undo(&mut st, move |a| {
            a.vault.config_mut().set_backup(before);
        });
        if r.is_ok() {
            warning!(
                "backup externo {}",
                if enabled { "ligado" } else { "desligado" }
            );
        }
        r
    }

    /// USB drives that udisks2 sees (Phase 46): only with the vault unlocked. The state is
    /// not held during the query.
    fn usb_drives(&self) -> Result<Response, ResponseError> {
        if self.is_locked() {
            return Err(ResponseError::Locked);
        }
        let source = self
            .usb_drives
            .as_ref()
            .ok_or(ResponseError::UsbDrivesUnavailable)?;
        let seen = source
            .usb_drives()
            .ok_or(ResponseError::UsbDrivesUnavailable)?;
        Ok(Response::UsbDrives(
            seen.into_iter()
                .take(MAX_USB_DRIVES)
                .map(|v| UsbDrive {
                    mounted: v.mount.is_some(),
                    uuid: v.uuid,
                    label: v.label,
                })
                .collect(),
        ))
    }

    /// Restores a version of the local backup (D17, D39; Phase 45). Requires the vault
    /// unlocked and the current PIN (S9: the version brings other levels and another PIN;
    /// the error counts toward the 5 that lock) and the version's master password. Order:
    ///
    /// 1. opens the version through the dir's fd, without following a link, with its
    ///    password and without a minimum generation (every backup is older);
    /// 2. verifies that the vault file is the one Ward wrote last (identity) and keeps it
    ///    as a backup, authenticated by the open vault's key; without that, it does not
    ///    swap;
    /// 3. burns in the state the new generation = highest burned + 1 and seals the version
    ///    with it through the normal write (with the new version's backup); afterwards,
    ///    `seen` = it.
    ///
    /// A version with a v4 body (0.1.x) opens with its data and is sealed again as v5,
    /// like any write (m2:DT5, m2:V4.10): the write only produces v5.
    ///
    /// Restoring never lowers the generation: the hand copy of the same backup stays in
    /// rollback (V7.11). An error after sealing locks the vault, as in the password
    /// change.
    fn restore_backup(
        &self,
        version: &str,
        password: &Secret,
        pin: &Secret,
    ) -> Result<Response, ResponseError> {
        let version = Version::from_name(version).ok_or(ResponseError::InvalidRequest)?;
        let mut st = self.state();
        Self::verify_pin(&mut st, pin)?;
        let dir = open_backups(&self.paths).ok_or(ResponseError::BackupUnavailable)?;
        // Phase 49a: the version must be in the list the app sees (no future date).
        let in_list = dir
            .backup_dir()
            .list_until(SystemTime::now())
            .map_err(|_| ResponseError::BackupUnavailable)?
            .contains(&version);
        if !in_list {
            warning!(
                "restaurar recusado: a versão não está na lista (inexistente ou com data futura)"
            );
            return Err(ResponseError::BackupNotFound);
        }
        let bytes = match files::read_vault(&dir.backup_dir().path(version)) {
            Ok(Some((b, _))) => b,
            Ok(None) => return Err(ResponseError::BackupNotFound),
            Err(_) => {
                warning!("restaurar recusado: a versão não é um arquivo comum do usuário");
                return Err(ResponseError::BackupUnavailable);
            }
        };
        let (mut restored, entries, _) =
            body::open_entries_with_generation(&bytes, &password_from(password), 0, 0).map_err(
                |e| {
                    warning!("restaurar recusado: a versão não abriu");
                    ResponseError::from(&e)
                },
            )?;
        drop(bytes);
        let Some(a) = st.open.as_mut() else {
            return Err(ResponseError::Locked);
        };
        // The current vault must be the one Ward wrote last (D39, D62).
        let current = match files::read_vault(&self.paths.vault) {
            Ok(Some((b, id))) if Some(id) == a.identity => b,
            _ => {
                warning!(
                    "restaurar recusado: o arquivo do cofre mudou fora do Ward; travado (D39)"
                );
                st.lock_changed();
                return Err(ResponseError::VaultChanged);
            }
        };
        let now = SystemTime::now();
        // D17: the current one becomes a backup before the swap; without that, no swap.
        if let Err(e) = backup::keep_current(&dir.backup_dir(), &a.vault, &current, now) {
            warning!(
                "restaurar recusado: o cofre atual não foi guardado ({})",
                backup_code(&e)
            );
            return Err(ResponseError::BackupUnavailable);
        }
        drop(current);
        // Phase 49a: the lock time and the external backup stay the current ones (the
        // version brings its own; restoring an old one does not turn off the copy to the
        // USB drive without notice). PIN, password and entries are the version's (D17).
        {
            let current = a.vault.config();
            let minutes = current.lock_minutes();
            let external = current.backup().clone();
            let c = restored.config_mut();
            if c.set_lock_minutes(minutes).is_err() {
                warning!("restaurar: tempo de trava atual fora da faixa; ficou o da versão");
            }
            c.set_backup(external);
        }
        // D39: generation = highest burned + 1, burned in the state before sealing (D53).
        let q = a.generations.burned.max(a.vault.generation());
        let next = q.checked_add(1).ok_or(ResponseError::GenerationExhausted)?;
        restored = restored.after_generation(q);
        let before = Generations {
            seen: a.generations.seen,
            burned: next,
        };
        if files::write_state(&self.paths.state, before).is_err() {
            warning!("restaurar recusado: não deu para gravar o arquivo de estado");
            return Err(ResponseError::WriteFailed);
        }
        a.generations = before;
        let r = atomic_write::write_entries_with_backup(
            &self.paths.vault,
            &mut restored,
            &entries,
            &dir.backup_dir(),
            now,
        );
        if let Some(g) = &self.write_hook {
            g();
        }
        match r {
            Ok(g) => {
                log_backup(&g.backup);
                // The previous vault (key, PIN, secrets) is zeroed here.
                a.vault = restored;
                a.entries = entries;
                a.grants.clear();
                confirm_write(&self.paths, a, g.identity);
                st.pin_errors = 0;
                // Phase 49a: an open approval request was about the previous vault; an
                // answer arriving now would change the same-named entry of the restored
                // version. All are denied, with the state still held.
                let n = self.approval.cancel_pending();
                if n > 0 {
                    warning!("{n} pedido(s) de aprovação cancelado(s) pelo restaurar");
                }
                warning!("backup restaurado; o cofre anterior ficou na pasta de backups");
                Ok(Response::Ok)
            }
            Err(e) => {
                if let WriteError::Io {
                    step: Step::FsyncDir,
                    identity,
                    ..
                } = e
                {
                    // The restored version is in place (the rename happened).
                    a.identity = identity;
                }
                warning!("restaurar falhou ({}): cofre travado", write_code(&e));
                // The burned generation is in the state; the unlock reopens with it (D53).
                st.lock();
                Err(ResponseError::from(&e))
            }
        }
    }
}
