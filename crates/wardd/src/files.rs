//! The daemon's files on disk: the vault (only reading and identity; the writer is
//! `ward_core::atomic_write`) and the state file with the generations of D39/D53.
//!
//! - Everything is opened with `O_NOFOLLOW`: a symbolic link in place of the vault or of
//!   the state is refused. Only a regular file of the user himself.
//! - The vault identity (`dev`, inode, `mtime` with nanoseconds and size) comes from the
//!   `fstat` of the same fd that was read (D39); after writing, from the fd of the temp
//!   file that `ward_core::atomic_write` returns (D62), never from a `stat` after the
//!   rename.
//! - The state is small text, without secrets, written atomically (temp file with
//!   `O_EXCL` + fsync + rename + fsync of the directory), 0600. It keeps the generations
//!   (D53) and the agent role's consecutive wrong passwords (D20 revised by D62).
//! - State format (m2:DT6): the write only produces the state v2 (`ward-state 2`;
//!   `seen`, `burned`, `agent_failures`, all required); the reading also accepts the
//!   0.1.x one, the state v1 (pt-BR keys, the failures line optional), with the same
//!   meaning. The file name on disk does not change.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
pub use ward_core::atomic_write::FileIdentity;

use ward_core::format;

/// Largest vault file the format accepts (header + content + tag).
pub(crate) const MAX_VAULT_FILE: u64 = (format::HEADER_SIZE + format::MAX_CONTENT + 16) as u64;
/// Largest state file accepted.
const MAX_STATE: u64 = 256;
/// First line of the state file, the only one written (m2:DT6).
const STATE_HEADER: &str = "ward-state 2";
/// First line of the 0.1.x state file, accepted only on reading (m2:DT6).
const LEGACY_STATE_HEADER: &str = "ward-estado 1";

/// The line keys of one state format (m2:DT6). Closed: a key of the other format, or any
/// other text, refuses the whole file.
struct StateKeys {
    seen: &'static str,
    burned: &'static str,
    agent_failures: &'static str,
    /// The agent failures line may be missing (only in the old format of Phase 22,
    /// read as 0; D62).
    failures_optional: bool,
}

/// Keys of the state v2 (`ward-state 2`), the format written.
const STATE_KEYS: StateKeys = StateKeys {
    seen: "seen",
    burned: "burned",
    agent_failures: "agent_failures",
    failures_optional: false,
};

/// Keys of the state v1 (0.1.x), only read.
const LEGACY_STATE_KEYS: StateKeys = StateKeys {
    seen: "vista",
    burned: "queimada",
    agent_failures: "falhas_agente",
    failures_optional: true,
};

/// Opens for reading without following a link (`O_NOFOLLOW`) and without waiting
/// (Phase 54l): with `O_NONBLOCK`, a FIFO put in place of the vault, of the state or of
/// a backup version (between the list's `readdir` and the `open`) opens at once and is
/// refused by the `fstat` in [`verify_file`], instead of holding the thread (and the
/// daemon state) until someone opens the other end. `O_NOCTTY`: a terminal in place does
/// not become the daemon's terminal. On a regular file `O_NONBLOCK` does not change the
/// read.
fn open_read(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_NOCTTY | libc::O_CLOEXEC)
        .open(path)
}

/// Only a regular file of the user himself, without write for group and others.
fn verify_file(m: &fs::Metadata) -> io::Result<()> {
    if !m.file_type().is_file()
        || m.uid() != ward_platform::effective_uid()
        || m.mode() & 0o022 != 0
    {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(())
}

/// Ward's `known_hosts` (D13; M4 audit): the folder must belong to the effective user,
/// be a real directory and have no group/other permission (0700); the file, if it
/// exists, regular, without a link, of the user and without group/other permission
/// (0600). If it does not exist, it is created empty and 0600 (without following a
/// link), so that `ssh` does not create it with the `umask`.
/// Anything else: `PermissionDenied`.
pub fn prepare_known_hosts(path: &Path) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    let fd = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_CLOEXEC)
        .open(dir)?;
    let m = fd.metadata()?;
    if !m.is_dir() || m.uid() != ward_platform::effective_uid() || m.mode() & 0o077 != 0 {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let f = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(path)
            .map_err(|_| io::Error::from(io::ErrorKind::PermissionDenied))?,
        Err(e) => return Err(e),
    };
    let m = f.metadata()?;
    if !m.file_type().is_file()
        || m.uid() != ward_platform::effective_uid()
        || m.mode() & 0o077 != 0
        || m.nlink() != 1
    {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(())
}

/// Does the vault exist (anything at the path, including a link)?
pub fn vault_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

/// Reads the vault file (encrypted) and the identity from the same fd. `Ok(None)` if it
/// does not exist.
pub fn read_vault(path: &Path) -> io::Result<Option<(Vec<u8>, FileIdentity)>> {
    let f = match open_read(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let m = f.metadata()?;
    verify_file(&m)?;
    if m.len() > MAX_VAULT_FILE {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut bytes = Vec::new();
    (&f).take(MAX_VAULT_FILE + 1).read_to_end(&mut bytes)?;
    // What was read has to be the file of the `fstat` (no write in the middle).
    let after = f.metadata()?;
    if bytes.len() as u64 != after.len() || FileIdentity::from(&m) != FileIdentity::from(&after) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(Some((bytes, FileIdentity::from(&m))))
}

/// Current identity of the vault at the path. `Ok(None)` if it does not exist.
pub fn identity(path: &Path) -> io::Result<Option<FileIdentity>> {
    match open_read(path) {
        Ok(f) => {
            let m = f.metadata()?;
            verify_file(&m)?;
            Ok(Some(FileIdentity::from(&m)))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// The two generations of the state file (D53).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Generations {
    /// Largest generation confirmed on disk: the minimum generation of the opening.
    pub seen: u64,
    /// Largest generation sealed or attempted. Never smaller than `seen`.
    pub burned: u64,
}

/// The whole state file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateFile {
    pub generations: Generations,
    /// Consecutive wrong passwords of the agent role (D20, D62). The old format, without
    /// the line, is read as 0; every write rewrites in the new format.
    pub agent_failures: u32,
}

/// The state file is missing or not in the format. The daemon treats both as a
/// rollback (D39): it does not unlock an existing vault without a state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateError {
    Missing,
    Invalid,
}

fn number(line: &str, key: &str) -> Option<u64> {
    let rest = line.strip_prefix(key)?.strip_prefix(' ')?;
    // Single form: digits only, without a leading zero.
    if rest.is_empty()
        || !rest.bytes().all(|b| b.is_ascii_digit())
        || (rest.len() > 1 && rest.starts_with('0'))
    {
        return None;
    }
    rest.parse().ok()
}

/// Reads the generations from the state file (see [`read_full_state`]).
pub fn read_state(path: &Path) -> Result<Generations, StateError> {
    read_full_state(path).map(|e| e.generations)
}

/// Reads the state file (strict: the exact lines, `burned ≥ seen`). Accepts the state
/// v2 (`ward-state 2`) and the 0.1.x state v1, whose line with the agent's failures is
/// optional, to read the old format as 0 (m2:DT6).
pub fn read_full_state(path: &Path) -> Result<StateFile, StateError> {
    let f = match open_read(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(StateError::Missing),
        Err(_) => return Err(StateError::Invalid),
    };
    let m = f.metadata().map_err(|_| StateError::Invalid)?;
    if verify_file(&m).is_err() || m.mode() & 0o077 != 0 || m.len() > MAX_STATE {
        return Err(StateError::Invalid);
    }
    let mut text = String::new();
    (&f).take(MAX_STATE + 1)
        .read_to_string(&mut text)
        .map_err(|_| StateError::Invalid)?;
    parse_state(&text).ok_or(StateError::Invalid)
}

fn parse_state(text: &str) -> Option<StateFile> {
    let body = text.strip_suffix('\n')?;
    let mut lines = body.split('\n');
    let keys = match lines.next()? {
        STATE_HEADER => &STATE_KEYS,
        LEGACY_STATE_HEADER => &LEGACY_STATE_KEYS,
        _ => return None,
    };
    let seen = number(lines.next()?, keys.seen)?;
    let burned = number(lines.next()?, keys.burned)?;
    // Old format (Phase 22): without the line, 0 (D62). Required in the state v2.
    let agent_failures = match lines.next() {
        None if keys.failures_optional => 0,
        None => return None,
        Some(l) => u32::try_from(number(l, keys.agent_failures)?).ok()?,
    };
    if lines.next().is_some() || burned < seen {
        return None;
    }
    Some(StateFile {
        generations: Generations { seen, burned },
        agent_failures,
    })
}

/// The text written: always the state v2, `ward-state 2` (m2:DT6).
fn state_text(e: StateFile) -> String {
    let k = &STATE_KEYS;
    format!(
        "{STATE_HEADER}\n{} {}\n{} {}\n{} {}\n",
        k.seen,
        e.generations.seen,
        k.burned,
        e.generations.burned,
        k.agent_failures,
        e.agent_failures
    )
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Writes the state atomically (temp file `O_EXCL` 0600 in the same directory, fsync,
/// rename, fsync of the directory), with the agent's failures at 0: that is what holds
/// with the vault open or opening (the right unlock resets it, D62). On error, the temp
/// file is deleted and the previous state stays.
pub fn write_state(path: &Path, g: Generations) -> io::Result<()> {
    write_full_state(
        path,
        StateFile {
            generations: g,
            agent_failures: 0,
        },
    )
}

/// Like [`write_state`], with the given agent failures. When it returns `Ok`, the file
/// and the directory have already gone through fsync (D62: before the error answer).
pub fn write_full_state(path: &Path, e: StateFile) -> io::Result<()> {
    let g = e.generations;
    if g.burned < g.seen {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let dir = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .ok_or(io::ErrorKind::InvalidInput)?;
    let name = path.file_name().ok_or(io::ErrorKind::InvalidInput)?;
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut tmp_name = std::ffi::OsString::from(".");
    tmp_name.push(name);
    tmp_name.push(format!(".tmp-{}-{n}", std::process::id()));
    let tmp: PathBuf = dir.join(tmp_name);
    let r = (|| {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&tmp)?;
        f.write_all(state_text(e).as_bytes())?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)?;
        File::open(dir)?.sync_all()
    })();
    if r.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    r
}

/// Deletes the state file's temp files that an interrupted write left (Phase 49a):
/// `.<name>.tmp-<pid>-<n>` in the state folder, only regular files of the euid
/// (`DirEntry::file_type` does not follow a link), with the exact name (pid and counter
/// digits only) and from **another process** (pid ≠ ours: a temp file of ours may be
/// being written right now). Their content is only the state text (generations and
/// count), nothing secret, but they would pile up. Returns how many it deleted; an I/O
/// error stops in the middle.
pub fn clean_state_temp_files(path: &Path) -> io::Result<usize> {
    let dir = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .ok_or(io::ErrorKind::InvalidInput)?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or(io::ErrorKind::InvalidInput)?;
    let prefix = format!(".{name}.tmp-");
    let mine = std::process::id().to_string();
    let euid = ward_platform::effective_uid();
    let mut deleted = 0;
    for item in fs::read_dir(dir)? {
        let item = item?;
        let Some(n) = item.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some(rest) = n.strip_prefix(&prefix) else {
            continue;
        };
        let Some((pid, count)) = rest.split_once('-') else {
            continue;
        };
        let only_digits =
            |t: &str| !t.is_empty() && t.len() <= 20 && t.bytes().all(|b| b.is_ascii_digit());
        if !only_digits(pid) || !only_digits(count) || pid == mine {
            continue;
        }
        if !item.file_type()?.is_file() {
            continue;
        }
        if item.metadata()?.uid() != euid {
            continue;
        }
        fs::remove_file(item.path())?;
        deleted += 1;
    }
    Ok(deleted)
}

/// Backup folder opened by the fd (Phase 45): the directory was opened with
/// `O_NOFOLLOW | O_DIRECTORY` and checked by the `fstat` (directory of the euid, 0700).
/// Every access goes through `/proc/self/fd/<n>/…`, so swapping the folder for a
/// symbolic link or for another directory after it was opened diverts nothing.
pub struct OpenDir {
    fd: File,
}

impl OpenDir {
    /// The path through the fd, for `ward_core::backup`.
    pub fn path(&self) -> PathBuf {
        use std::os::fd::AsRawFd;
        PathBuf::from(format!("/proc/self/fd/{}", self.fd.as_raw_fd()))
    }

    /// The folder for the core, with the expected uid and the rotation reader
    /// (Phase 54c).
    pub fn backup_dir(&self) -> ward_core::backup::BackupDir {
        ward_core::backup::BackupDir::new(self.path(), ward_platform::effective_uid())
            .with_reader(read_backup_version)
    }
}

/// Reads a local backup version for the rotation to check the generation (Phase 54c):
/// `O_NOFOLLOW | O_NONBLOCK` (a link or a FIFO swapped in place is not followed nor
/// holds the write), regular file of the `uid`, up to the format size. `None` in any
/// other case.
fn read_backup_version(path: &Path, uid: u32) -> Option<Vec<u8>> {
    let f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .ok()?;
    let m = f.metadata().ok()?;
    if !m.file_type().is_file() || m.uid() != uid || m.len() > MAX_VAULT_FILE {
        return None;
    }
    let mut bytes = Vec::new();
    (&f).take(MAX_VAULT_FILE + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= MAX_VAULT_FILE).then_some(bytes)
}

/// Backup folder next to the vault: `<vault folder>/backups` (D17:
/// `~/.local/share/ward/backups`; in `test-mode`, `.dev/backups`).
pub fn backups_dir(vault: &Path) -> Option<PathBuf> {
    vault
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .map(|d| d.join("backups"))
}

/// Opens the backup folder (creates it 0700 if missing, without creating the parents).
/// A symbolic link in place, another owner or any group/other permission:
/// `PermissionDenied`.
pub fn open_backup_dir(dir: &Path) -> io::Result<OpenDir> {
    use std::os::unix::fs::DirBuilderExt;
    match fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    let fd = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_CLOEXEC)
        .open(dir)?;
    let m = fd.metadata()?;
    if !m.is_dir() || m.uid() != ward_platform::effective_uid() || m.mode() & 0o077 != 0 {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(OpenDir { fd })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `f` in a thread and waits at most `deadline`; `None` if it did not return
    /// (the stuck thread is left behind: the test fails).
    fn with_deadline<T: Send + 'static>(
        deadline: std::time::Duration,
        f: impl FnOnce() -> T + Send + 'static,
    ) -> Option<T> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        rx.recv_timeout(deadline).ok()
    }

    /// Phase 54l (finding of 54c), attack: the same uid puts a FIFO in place of the
    /// vault, of the state or of a backup version. Before, the `open` without
    /// `O_NONBLOCK` stayed stuck until someone opened the other end (the restore list and
    /// the external copy held the daemon state). Now the read returns at once with a
    /// refusal.
    #[test]
    fn f54l_attack_fifo_in_place_does_not_block_the_read() {
        use std::os::unix::fs::DirBuilderExt;
        let root = std::env::temp_dir().join(format!("ward-fifo-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let fifo = root.join("versao.ward");
        let st = std::process::Command::new("/usr/bin/mkfifo")
            .arg("-m")
            .arg("600")
            .arg(&fifo)
            .status()
            .unwrap();
        assert!(st.success());
        let deadline = std::time::Duration::from_secs(3);
        let f = fifo.clone();
        let r = with_deadline(deadline, move || read_vault(&f).map(|o| o.is_some()));
        let r = r.expect("ler_cofre ficou preso no FIFO");
        assert_eq!(r.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        let f = fifo.clone();
        let r = with_deadline(deadline, move || identity(&f).map(|o| o.is_some()));
        assert!(r.expect("identidade ficou presa no FIFO").is_err());
        let f = fifo.clone();
        let r = with_deadline(deadline, move || read_full_state(&f).is_err());
        assert!(r.expect("ler_estado ficou preso no FIFO"));
        // Control: a regular file is still read whole.
        let regular = root.join("comum.ward");
        fs::write(&regular, b"bytes-falsos").unwrap();
        fs::set_permissions(
            &regular,
            std::os::unix::fs::PermissionsExt::from_mode(0o600),
        )
        .unwrap();
        let (bytes, _) = read_vault(&regular).unwrap().unwrap();
        assert_eq!(bytes, b"bytes-falsos");
        let _ = fs::remove_dir_all(&root);
    }

    fn only_generations(text: &str) -> Option<Generations> {
        parse_state(text).map(|e| e.generations)
    }

    /// M4 audit (low): a loose or swapped `known_hosts` is refused.
    #[test]
    fn attack_loose_or_linked_known_hosts_is_refused() {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
        let root = std::env::temp_dir().join(format!("ward-kh-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let kh = root.join("known_hosts");
        // Does not exist: it is born empty and 0600.
        prepare_known_hosts(&kh).unwrap();
        let m = fs::symlink_metadata(&kh).unwrap();
        assert!(m.is_file() && m.len() == 0);
        assert_eq!(m.mode() & 0o777, 0o600);
        // Exists and is right: accepted again, untouched.
        fs::write(&kh, "[127.0.0.1]:2222 ssh-ed25519 AAAAfalso\n").unwrap();
        prepare_known_hosts(&kh).unwrap();
        // File readable or writable by others: refused.
        for mode in [0o644, 0o640, 0o602, 0o660] {
            fs::set_permissions(&kh, fs::Permissions::from_mode(mode)).unwrap();
            assert!(prepare_known_hosts(&kh).is_err(), "{mode:o}");
        }
        fs::set_permissions(&kh, fs::Permissions::from_mode(0o600)).unwrap();
        // Loose folder: refused.
        for mode in [0o755, 0o750, 0o711] {
            fs::set_permissions(&root, fs::Permissions::from_mode(mode)).unwrap();
            assert!(prepare_known_hosts(&kh).is_err(), "pasta {mode:o}");
        }
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        // Link in place of the file (to a 0600 file elsewhere): refused.
        let target = root.join("outro");
        fs::write(&target, "").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        fs::remove_file(&kh).unwrap();
        symlink(&target, &kh).unwrap();
        assert!(prepare_known_hosts(&kh).is_err());
        fs::remove_file(&kh).unwrap();
        // Dangling link: does not create the file on the other side.
        symlink(root.join("nao-existe"), &kh).unwrap();
        assert!(prepare_known_hosts(&kh).is_err());
        assert!(!root.join("nao-existe").exists());
        fs::remove_file(&kh).unwrap();
        // Hard link (another name for the same file): refused.
        fs::hard_link(&target, &kh).unwrap();
        assert!(prepare_known_hosts(&kh).is_err());
        fs::remove_file(&kh).unwrap();
        // Directory in place of the file: refused.
        fs::DirBuilder::new().mode(0o700).create(&kh).unwrap();
        assert!(prepare_known_hosts(&kh).is_err());
        fs::remove_dir(&kh).unwrap();
        // Folder that is a link: refused.
        let dir_link = std::env::temp_dir().join(format!("ward-kh-l-{}", std::process::id()));
        let _ = fs::remove_file(&dir_link);
        symlink(&root, &dir_link).unwrap();
        assert!(prepare_known_hosts(&dir_link.join("known_hosts")).is_err());
        fs::remove_file(&dir_link).unwrap();
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn strict_state() {
        let ok = only_generations("ward-estado 1\nvista 3\nqueimada 4\n").unwrap();
        assert_eq!(ok, Generations { seen: 3, burned: 4 });
        let ok = only_generations("ward-state 2\nseen 3\nburned 4\nagent_failures 0\n").unwrap();
        assert_eq!(ok, Generations { seen: 3, burned: 4 });
        let full = StateFile {
            generations: Generations {
                seen: 0,
                burned: u64::MAX,
            },
            agent_failures: u32::MAX,
        };
        assert_eq!(parse_state(&state_text(full)), Some(full));
        for bad in [
            "",
            "ward-estado 1\nvista 3\nqueimada 4",
            "ward-estado 2\nvista 3\nqueimada 4\n",
            "ward-estado 1\nvista 03\nqueimada 4\n",
            "ward-estado 1\nvista 5\nqueimada 4\n",
            "ward-estado 1\nvista -1\nqueimada 4\n",
            "ward-estado 1\nvista 3\nqueimada 4\nextra 1\n",
            "ward-estado 1\nqueimada 4\nvista 3\n",
            "ward-estado 1\nvista  3\nqueimada 4\n",
            "ward-estado 1\nvista 3\nqueimada 18446744073709551616\n",
            "ward-estado 1\r\nvista 3\nqueimada 4\n",
            // The agent failures line (D62): strict like the others.
            "ward-estado 1\nvista 3\nqueimada 4\nfalhas_agente 01\n",
            "ward-estado 1\nvista 3\nqueimada 4\nfalhas_agente -1\n",
            "ward-estado 1\nvista 3\nqueimada 4\nfalhas_agente 4294967296\n",
            "ward-estado 1\nvista 3\nqueimada 4\nfalhas_agente\n",
            "ward-estado 1\nvista 3\nqueimada 4\nfalhas 2\n",
            "ward-estado 1\nvista 3\nqueimada 4\nfalhas_agente 2\nfalhas_agente 2\n",
            "ward-estado 1\nfalhas_agente 2\nvista 3\nqueimada 4\n",
        ] {
            assert_eq!(parse_state(bad), None, "{bad:?}");
        }
    }

    /// D62: the old format (three lines) is read with 0 agent failures; the new one, with
    /// the line; the write always rewrites in the new format.
    #[test]
    fn d62_old_format_reads_zero_and_new_has_the_line() {
        let old = parse_state("ward-estado 1\nvista 3\nqueimada 4\n").unwrap();
        assert_eq!(old.agent_failures, 0);
        let new = parse_state("ward-estado 1\nvista 3\nqueimada 4\nfalhas_agente 10\n").unwrap();
        assert_eq!(new.agent_failures, 10);
        assert_eq!(new.generations, old.generations);
        assert_eq!(
            state_text(old),
            "ward-state 2\nseen 3\nburned 4\nagent_failures 0\n"
        );
    }

    /// m2:DT6: the state v2 reads with the same meaning as the state v1 (generations and
    /// the agent's failures), and the write only produces the state v2, which reads back
    /// the same.
    #[test]
    fn dt6_v1_and_v2_read_the_same_and_the_write_is_v2() {
        let v1 = parse_state("ward-estado 1\nvista 3\nqueimada 4\nfalhas_agente 2\n").unwrap();
        let v2 = parse_state("ward-state 2\nseen 3\nburned 4\nagent_failures 2\n").unwrap();
        assert_eq!(v1, v2);
        assert_eq!(v2.generations, Generations { seen: 3, burned: 4 });
        assert_eq!(v2.agent_failures, 2);
        let text = state_text(v1);
        assert!(text.starts_with("ward-state 2\n"), "{text:?}");
        assert_eq!(parse_state(&text), Some(v1));
        // The 0.1.4 fixture (the state v1, generation 3).
        let fixture = include_str!("../../../dev/fixtures/legacy/state-0.1.4");
        let old = parse_state(fixture).unwrap();
        assert_eq!(old.generations, Generations { seen: 3, burned: 3 });
        assert_eq!(old.agent_failures, 0);
        assert!(state_text(old).starts_with("ward-state 2\n"));
        assert!(!state_text(old).contains("vista"));
    }

    /// m2:DT6 (attack): the two formats do not mix. A key of the other format, a v2
    /// without the failures line, a repeated or reordered line, an unknown version or a
    /// v2 header with different spacing or case refuse the whole file.
    #[test]
    fn dt6_attack_mixed_or_forged_state_refused() {
        for bad in [
            "ward-state 2\nseen 3\nburned 4\n",
            "ward-state 2\nvista 3\nqueimada 4\nfalhas_agente 0\n",
            "ward-state 2\nseen 3\nqueimada 4\nagent_failures 0\n",
            "ward-state 2\nseen 3\nburned 4\nfalhas_agente 0\n",
            "ward-estado 1\nseen 3\nburned 4\nagent_failures 0\n",
            "ward-estado 1\nvista 3\nqueimada 4\nagent_failures 0\n",
            "ward-state 1\nseen 3\nburned 4\nagent_failures 0\n",
            "ward-estado 2\nseen 3\nburned 4\nagent_failures 0\n",
            "ward-state 3\nseen 3\nburned 4\nagent_failures 0\n",
            "ward-state  2\nseen 3\nburned 4\nagent_failures 0\n",
            "Ward-State 2\nseen 3\nburned 4\nagent_failures 0\n",
            "ward-state 2 \nseen 3\nburned 4\nagent_failures 0\n",
            "ward-state 2\nseen 5\nburned 4\nagent_failures 0\n",
            "ward-state 2\nseen 03\nburned 4\nagent_failures 0\n",
            "ward-state 2\nseen 3\nburned 4\nagent_failures 01\n",
            "ward-state 2\nseen 3\nburned 4\nagent_failures 4294967296\n",
            "ward-state 2\nburned 4\nseen 3\nagent_failures 0\n",
            "ward-state 2\nseen 3\nburned 4\nagent_failures 0\nagent_failures 0\n",
            "ward-state 2\nseen 3\nburned 4\nagent_failures 0",
            "ward-state 2\r\nseen 3\nburned 4\nagent_failures 0\n",
            "ward-state 2\nseen 3\nburned 4\nagent_failures 0\n\n",
        ] {
            assert_eq!(parse_state(bad), None, "{bad:?}");
        }
    }

    /// m2:DT6 / m2:V4.11: the write is atomic and 0600, the name on disk stays, a state v1
    /// becomes a state v2 on the next write with the same values, and a link, a FIFO or a
    /// loose mode in place of the state is refused as before.
    #[test]
    fn dt6_v1_state_on_disk_becomes_v2_and_strange_files_are_refused() {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
        let root = std::env::temp_dir().join(format!("ward-dt6-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let state = root.join("estado");
        fs::write(
            &state,
            "ward-estado 1\nvista 3\nqueimada 3\nfalhas_agente 4\n",
        )
        .unwrap();
        fs::set_permissions(&state, fs::Permissions::from_mode(0o600)).unwrap();
        let read = read_full_state(&state).unwrap();
        assert_eq!(read.generations, Generations { seen: 3, burned: 3 });
        assert_eq!(read.agent_failures, 4);
        write_full_state(&state, read).unwrap();
        assert_eq!(
            fs::read_to_string(&state).unwrap(),
            "ward-state 2\nseen 3\nburned 3\nagent_failures 4\n"
        );
        assert_eq!(fs::metadata(&state).unwrap().mode() & 0o777, 0o600);
        assert_eq!(read_full_state(&state).unwrap(), read);
        // Only the state itself in the folder: no temp file left behind.
        let names: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, [std::ffi::OsString::from("estado")]);
        // Loose mode, link and FIFO: refused, in both formats.
        for text in [
            "ward-state 2\nseen 3\nburned 3\nagent_failures 0\n",
            "ward-estado 1\nvista 3\nqueimada 3\n",
        ] {
            fs::write(&state, text).unwrap();
            fs::set_permissions(&state, fs::Permissions::from_mode(0o640)).unwrap();
            assert_eq!(read_full_state(&state), Err(StateError::Invalid));
            fs::set_permissions(&state, fs::Permissions::from_mode(0o600)).unwrap();
            assert!(read_full_state(&state).is_ok());
            let target = root.join("alvo");
            fs::write(&target, text).unwrap();
            fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
            let link = root.join("estado-link");
            let _ = fs::remove_file(&link);
            symlink(&target, &link).unwrap();
            assert_eq!(read_full_state(&link), Err(StateError::Invalid));
        }
        let fifo = root.join("estado-fifo");
        let st = std::process::Command::new("/usr/bin/mkfifo")
            .arg("-m")
            .arg("600")
            .arg(&fifo)
            .status()
            .unwrap();
        assert!(st.success());
        assert_eq!(read_full_state(&fifo), Err(StateError::Invalid));
        let _ = fs::remove_dir_all(&root);
    }
}
