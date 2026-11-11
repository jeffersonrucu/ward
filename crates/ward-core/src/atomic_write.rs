//! Atomic write of the vault file (S7): temporary file in the same directory → fsync of
//! the temporary file → rename over the destination → fsync of the directory.
//!
//! A write interrupted at any point (killed process, full disk, fsync failure) leaves
//! in the destination the whole previous version or the whole new one, never a mix. The
//! temporary file is created with `O_CREAT|O_EXCL` (does not follow a symbolic link nor
//! reuse an existing file), a random name and mode 0600.
//!
//! Only bytes that already are an encrypted vault enter here (S2). The only public
//! writes are [`write_entries`] (the daemon path) and [`change_password`] (D27, D44):
//! both start from an [`OpenVault`] or from the current password, serialize or validate
//! the body (with the PIN hash, D30) and seal a generation that was never sealed (D39,
//! D53). The variants [`write_entries_with_backup`] and [`change_password_with_backup`]
//! (Phase 45) also write, in the backups folder, the same bytes they have just sealed
//! ([`crate::backup`]). Sealing and writing loose bytes ([`OpenVault::seal`],
//! `write_vault`, `write`) is internal to the crate: `write` only checks the header
//! structure, not the authenticity (it does not have the key), and `write_vault` would
//! seal any body, including one without a PIN hash. No path of this module writes clear
//! content.
//!
//! From outside `ward-core`, writing loose bytes does not compile (V2.34):
//!
//! ```compile_fail,E0603
//! let mut bytes = Vec::from(*b"WARDCOFR");
//! bytes.resize(160 + 16, 0);
//! let _ = ward_core::atomic_write::write(std::path::Path::new("vault.ward"), &bytes);
//! ```
//!
//! Nor sealing and writing any body through the open vault (V2.50, D44):
//!
//! ```compile_fail,E0603
//! use ward_core::configuracao::{Configuracao, HashPin};
//! use ward_core::format::{OpenVault, MasterPassword};
//!
//! let password = MasterPassword::new("falso-mestra-ward-01".to_owned());
//! let config = Configuracao::nova(HashPin::criar(b"135790").unwrap());
//! let vault = OpenVault::new(&password, config).unwrap();
//! // Body without a PIN hash: only the version and zero entries.
//! let body = [3u8, 0, 0, 0, 0, 0];
//! let _ = ward_core::atomic_write::write_vault(std::path::Path::new("vault.ward"), &vault, &body);
//! ```
//!
//! The public path:
//!
//! ```no_run
//! use ward_core::configuracao::{Configuracao, HashPin};
//! use ward_core::format::{OpenVault, MasterPassword};
//! use ward_core::atomic_write::write_entries;
//!
//! let password = MasterPassword::new("falso-mestra-ward-01".to_owned());
//! let config = Configuracao::nova(HashPin::criar(b"135790").unwrap());
//! let mut vault = OpenVault::new(&password, config).unwrap();
//! write_entries(std::path::Path::new("vault.ward"), &mut vault, &[]).unwrap();
//! assert_eq!(vault.generation(), 1);
//! ```
//!
//! Identity (D62): [`write_entries`] and [`change_password`] return the
//! [`FileIdentity`] read by `fstat` of the temporary file's fd after the fsync and
//! before the rename (the rename changes neither dev, inode, `mtime` nor size). The
//! caller never needs a `stat` of the path after the rename, which would catch a file
//! swapped by another process in the meantime.

use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::backup::{ErroBackup, PastaBackup, Versao};
use crate::corpo::{self, ErroCorpo};
use crate::crypto;
use crate::entry::Entry;
use crate::format::{self, FormatError, MasterPassword, OpenVault};

/// Suffix of the temporary files: `.<destination name>.ward-tmp-<32 hex>`.
const TEMP_MARK: &str = ".ward-tmp-";
/// Mode of the vault and of the temporary file.
const MODE: u32 = 0o600;
/// Size of each `write` (and of each test point in the middle of the write).
const CHUNK: usize = 1024 * 1024;

/// Identity of a file (D39, D62): device, inode, `mtime` with nanoseconds and size, read
/// by `fstat`. Numbers only; no content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileIdentity {
    dev: u64,
    ino: u64,
    mtime: i64,
    mtime_nsec: i64,
    size: u64,
}

impl From<&fs::Metadata> for FileIdentity {
    fn from(m: &fs::Metadata) -> Self {
        Self {
            dev: m.dev(),
            ino: m.ino(),
            mtime: m.mtime(),
            mtime_nsec: m.mtime_nsec(),
            size: m.len(),
        }
    }
}

/// Step of the write where the failure happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Read the current vault (password change).
    Read,
    /// Create the temporary file in the destination directory.
    CreateTemp,
    /// Write the temporary file.
    Write,
    /// fsync of the temporary file.
    FsyncTemp,
    /// rename of the temporary file over the destination.
    Rename,
    /// fsync of the directory (the rename already happened; the new version is in
    /// place, but may not have reached the disk yet).
    FsyncDir,
}

impl Step {
    fn name(self) -> &'static str {
        match self {
            Self::Read => "ler o arquivo do cofre",
            Self::CreateTemp => "criar o arquivo temporário",
            Self::Write => "escrever o arquivo temporário",
            Self::FsyncTemp => "sincronizar o arquivo temporário com o disco",
            Self::Rename => "trocar o cofre pelo arquivo novo",
            Self::FsyncDir => "sincronizar o diretório do cofre com o disco",
        }
    }
}

/// Write error. Carries no vault content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteError {
    /// The vault is already at the largest possible generation (`u64::MAX`, D39):
    /// nothing was written.
    GenerationExhausted,
    /// Password change: the file on disk has a generation lower than the minimum asked
    /// for ([`change_password`], D39). Nothing was written.
    Rollback,
    /// The bytes are not a well-formed encrypted vault (nothing was written).
    NotVault,
    /// Failure to seal the content, or to open the vault in the password change (nothing
    /// was written).
    Format(FormatError),
    /// The entries do not become a writable body (too large or inconsistent; nothing was
    /// written).
    Body(ErroCorpo),
    /// The destination has no file name (`/`, `..`, empty).
    InvalidDestination,
    /// I/O failure in a step. Before [`Step::FsyncDir`], the destination keeps the
    /// previous version and the temporary file was deleted.
    Io {
        /// Step that failed.
        step: Step,
        /// Kind of the system error.
        kind: io::ErrorKind,
        /// `Some` only in [`Step::FsyncDir`]: the rename happened, and this is the
        /// identity of the file Ward put in place, read from the temporary file's fd
        /// (D62). In the other steps, `None`.
        identity: Option<FileIdentity>,
    },
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GenerationExhausted => {
                f.write_str("recusado gravar: o contador de versões do cofre chegou ao fim")
            }
            Self::Rollback => f.write_str(
                "recusado trocar a senha: o cofre no disco é mais antigo que o último aberto",
            ),
            Self::NotVault => f.write_str("recusado gravar: os dados não são um cofre cifrado"),
            Self::Format(e) => write!(f, "falha ao cifrar o cofre para gravar: {e}"),
            Self::Body(e) => write!(f, "recusado gravar: {e}"),
            Self::InvalidDestination => f.write_str("caminho do cofre inválido"),
            Self::Io { step, kind, .. } => {
                write!(f, "falha ao {} ({kind})", step.name())
            }
        }
    }
}

impl std::error::Error for WriteError {}

/// Point of the write where the test can inject a failure or stop the process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Point {
    AfterCreate,
    /// After each chunk written, with the total written so far.
    Writing(usize),
    AfterWrite,
    AfterFsync,
    AfterRename,
    AfterFsyncDir,
}

/// Serializes the config of the open vault, the next generation (D39) and the entries
/// (D26, D30), seals with the key of the open vault and writes atomically. The clear
/// body zeroes on drop.
///
/// Burned generation (D53): as soon as the body is sealed, the `vault` moves to the new
/// generation, before writing the temporary file, and stays on it if any later step
/// fails (write, fsync, rename, directory fsync). The next write seals another one,
/// never the same. At `u64::MAX`, refuses with [`WriteError::GenerationExhausted`]
/// before sealing, without touching the disk and without burning; a body that does not
/// serialize does not burn either.
///
/// Returns the identity of the written file, read from the temporary file's fd (D62).
pub fn write_entries(
    destination: &Path,
    vault: &mut OpenVault,
    entries: &[Entry],
) -> Result<FileIdentity, WriteError> {
    write_entries_with_points(destination, vault, entries, &mut |_| Ok(()))
}

fn write_entries_with_points(
    destination: &Path,
    vault: &mut OpenVault,
    entries: &[Entry],
    point: &mut dyn FnMut(Point) -> io::Result<()>,
) -> Result<FileIdentity, WriteError> {
    write_entries_sealed(destination, vault, entries, point).map(|(id, _)| id)
}

/// Like [`write_entries_with_points`], and also returns the sealed (encrypted) file, for
/// the backup to write the same version (Phase 45).
fn write_entries_sealed(
    destination: &Path,
    vault: &mut OpenVault,
    entries: &[Entry],
    point: &mut dyn FnMut(Point) -> io::Result<()>,
) -> Result<(FileIdentity, Vec<u8>), WriteError> {
    let generation = vault
        .next_generation()
        .ok_or(WriteError::GenerationExhausted)?;
    let content =
        corpo::serializar(vault.config(), generation, entries).map_err(WriteError::Body)?;
    let file = vault.seal(&content).map_err(WriteError::Format)?;
    drop(content);
    // D53: the generation is sealed; from here on it is not used again.
    vault.burn(generation);
    let id = write_with_points(destination, &file, point)?;
    Ok((id, file))
}

/// Result of a write with backup (Phase 45): the identity of the written vault (D62) and
/// what happened with the backup. The backup failing does not undo the vault write,
/// which is already in place; the caller records it in the log.
#[derive(Debug)]
pub struct Written {
    /// Identity of the vault file, read from the temporary file's fd (D62).
    pub identity: FileIdentity,
    /// The version written in the backups folder, or why it was not.
    pub backup: Result<Versao, ErroBackup>,
}

/// [`write_entries`] and, after the vault write succeeded in full (up to the directory
/// fsync), writes **the same sealed version** as a backup in `dir` (D17, D27): same
/// format, encrypted, atomic write without overwriting, 30 versions. No clear byte goes
/// through here. Error in the vault write: no backup is made.
pub fn write_entries_with_backup(
    destination: &Path,
    vault: &mut OpenVault,
    entries: &[Entry],
    dir: &PastaBackup,
    instant: SystemTime,
) -> Result<Written, WriteError> {
    let (identity, file) = write_entries_sealed(destination, vault, entries, &mut |_| Ok(()))?;
    Ok(Written {
        identity,
        backup: dir.guardar(&file, instant, vault),
    })
}

/// Changes the master password of the vault at `destination` (V2.8): with the `file`
/// already read by the caller, requires the current password, checks that the body is
/// valid and reads its config and entries (D30), seals again with new salt, data key and
/// nonces and writes atomically. Returns the vault opened with the new password, the
/// same config and the new generation.
///
/// Phase 55: the core does not open the vault to read it (it has no `libc`). The daemon
/// reads with `O_NOFOLLOW | O_NONBLOCK`, only a regular file of the user, and checks the
/// identity of the same fd: a link or a FIFO swapped in place is neither followed nor
/// holds the thread.
///
/// `min_generation` (D39): a file with a lower generation (swapped for an older authentic
/// version) is refused with [`WriteError::Rollback`]. The daemon passes the largest
/// generation confirmed on disk; 0 accepts any.
///
/// `burned_generation` (D53): the largest generation already sealed or attempted that
/// the caller knows. The change seals max(file generation, `burned_generation`) + 1, so
/// it never repeats a number that a failed write already sealed. Invalid body, rollback
/// or exhausted generation: nothing is written. An error after sealing loses the
/// returned open vault; the caller locks and reopens with the burned generation it kept.
///
/// Also returns the identity of the written file (D62, as in [`write_entries`]).
pub fn change_password_from_bytes(
    destination: &Path,
    file: &[u8],
    current: &MasterPassword,
    new: &MasterPassword,
    min_generation: u64,
    burned_generation: u64,
) -> Result<(OpenVault, FileIdentity), WriteError> {
    change_password_sealed(
        destination,
        file,
        current,
        new,
        min_generation,
        burned_generation,
    )
    .map(|(vault, id, _)| (vault, id))
}

/// [`change_password_from_bytes`] and, after it succeeded in full, the backup of the new
/// version (with the new password) in `dir`, as in [`write_entries_with_backup`]
/// (Phase 45).
#[allow(clippy::too_many_arguments)]
pub fn change_password_with_backup_from_bytes(
    destination: &Path,
    file: &[u8],
    current: &MasterPassword,
    new: &MasterPassword,
    min_generation: u64,
    burned_generation: u64,
    dir: &PastaBackup,
    instant: SystemTime,
) -> Result<(OpenVault, Written), WriteError> {
    let (vault, identity, file) = change_password_sealed(
        destination,
        file,
        current,
        new,
        min_generation,
        burned_generation,
    )?;
    let backup = dir.guardar(&file, instant, &vault);
    Ok((vault, Written { identity, backup }))
}

/// Only in the core tests: [`change_password_from_bytes`] reading the file through the
/// standard library.
#[cfg(test)]
pub(crate) fn change_password(
    destination: &Path,
    current: &MasterPassword,
    new: &MasterPassword,
    min_generation: u64,
    burned_generation: u64,
) -> Result<(OpenVault, FileIdentity), WriteError> {
    let file = read_file(destination)?;
    change_password_from_bytes(
        destination,
        &file,
        current,
        new,
        min_generation,
        burned_generation,
    )
}

/// Only in the core tests: [`change_password_with_backup_from_bytes`] reading the file
/// through the standard library.
#[cfg(test)]
pub(crate) fn change_password_with_backup(
    destination: &Path,
    current: &MasterPassword,
    new: &MasterPassword,
    min_generation: u64,
    burned_generation: u64,
    dir: &PastaBackup,
    instant: SystemTime,
) -> Result<(OpenVault, Written), WriteError> {
    let file = read_file(destination)?;
    change_password_with_backup_from_bytes(
        destination,
        &file,
        current,
        new,
        min_generation,
        burned_generation,
        dir,
        instant,
    )
}

fn change_password_sealed(
    destination: &Path,
    file: &[u8],
    current: &MasterPassword,
    new: &MasterPassword,
    min_generation: u64,
    burned_generation: u64,
) -> Result<(OpenVault, FileIdentity, Vec<u8>), WriteError> {
    let limit = (format::HEADER_SIZE + format::MAX_CONTENT + crypto::TAG_SIZE) as u64;
    if file.len() as u64 > limit {
        return Err(WriteError::Format(FormatError::PasswordOrFile));
    }
    let (keys, content) =
        format::change_password(file, current, new).map_err(WriteError::Format)?;
    let (config, generation, entries) = corpo::desserializar(&content).map_err(WriteError::Body)?;
    drop(content);
    if generation < min_generation {
        return Err(WriteError::Rollback);
    }
    let mut vault = OpenVault::from_parts(keys, config, generation.max(burned_generation));
    let (identity, sealed) =
        write_entries_sealed(destination, &mut vault, &entries, &mut |_| Ok(()))?;
    Ok((vault, identity, sealed))
}

/// Reads the vault file (already encrypted) without going past the largest possible
/// size of a vault.
#[cfg(test)]
fn read_file(destination: &Path) -> Result<Vec<u8>, WriteError> {
    let limit = (format::HEADER_SIZE + format::MAX_CONTENT + crypto::TAG_SIZE) as u64;
    let f = File::open(destination).map_err(error(Step::Read))?;
    let mut file = Vec::new();
    io::Read::read_to_end(&mut io::Read::take(f, limit + 1), &mut file)
        .map_err(error(Step::Read))?;
    if file.len() as u64 > limit {
        return Err(WriteError::Format(FormatError::PasswordOrFile));
    }
    Ok(file)
}

/// Seals `content` with the key of the open vault and writes the file atomically. Only
/// inside the crate (D44): it would seal any body, including one without a PIN hash, and
/// it does not burn the generation (D53). From outside, the path is [`write_entries`].
/// Only the crate tests call it directly (the backup writes the bytes already sealed by
/// the vault write).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn write_vault(
    destination: &Path,
    vault: &OpenVault,
    content: &[u8],
) -> Result<FileIdentity, WriteError> {
    write_vault_with_points(destination, vault, content, &mut |_| Ok(()))
}

fn write_vault_with_points(
    destination: &Path,
    vault: &OpenVault,
    content: &[u8],
    point: &mut dyn FnMut(Point) -> io::Result<()>,
) -> Result<FileIdentity, WriteError> {
    let file = vault.seal(content).map_err(WriteError::Format)?;
    write_with_points(destination, &file, point)
}

/// Atomically writes a vault file that is already encrypted (from [`OpenVault::seal`]
/// or [`format::change_password`]). Refuses, without touching the disk, bytes that do
/// not have the structure of a vault.
///
/// Only inside the crate (D27): it checks the structure, not the authenticity. The
/// backup (Phase 45) uses the same internal path ([`write_temp`] and the directory
/// fsync), only publishing by link without overwriting ([`write_new_with_points`]).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn write(destination: &Path, file: &[u8]) -> Result<FileIdentity, WriteError> {
    write_with_points(destination, file, &mut |_| Ok(()))
}

/// Deletes the temporary files that an interrupted write left next to `destination`
/// (only regular files with the destination's prefix; symbolic links and directories
/// stay). The daemon calls it when opening the vault, before the first write. Returns
/// how many it deleted.
pub fn clean_temp_files(destination: &Path) -> io::Result<usize> {
    let (dir, name) = split(destination).ok_or(io::ErrorKind::InvalidInput)?;
    let mut prefix = OsString::from(".");
    prefix.push(name);
    prefix.push(TEMP_MARK);
    let prefix = prefix.as_encoded_bytes().to_vec();
    let mut deleted = 0;
    for item in fs::read_dir(&dir)? {
        let item = item?;
        if !item.file_name().as_encoded_bytes().starts_with(&prefix) {
            continue;
        }
        // `DirEntry::file_type` does not follow a symbolic link.
        if item.file_type()?.is_file() {
            fs::remove_file(item.path())?;
            deleted += 1;
        }
    }
    Ok(deleted)
}

/// Directory (`.` when the destination has none) and file name.
fn split(destination: &Path) -> Option<(PathBuf, OsString)> {
    let name = destination.file_name()?.to_os_string();
    let dir = match destination.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    Some((dir, name))
}

fn error(step: Step) -> impl FnOnce(io::Error) -> WriteError {
    move |e| WriteError::Io {
        step,
        kind: e.kind(),
        identity: None,
    }
}

/// Creates the temporary file: `O_WRONLY|O_CREAT|O_EXCL|O_CLOEXEC`, mode 0600. With
/// `O_EXCL`, a symbolic link or an existing file with the same name makes the creation
/// fail.
fn create_temp(path: &Path) -> io::Result<File> {
    let f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(MODE)
        .open(path)?;
    // The umask only removes bits; even so, set 0600 on the descriptor itself.
    f.set_permissions(fs::Permissions::from_mode(MODE))?;
    Ok(f)
}

/// Deletes the temporary file when the write does not reach the end (rename or link).
struct TempFile {
    path: PathBuf,
    active: bool,
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if self.active {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Writes `file` (already encrypted, with the structure checked) to a new temporary file
/// `.<name>.ward-tmp-<32 hex>` in `dir`: `O_EXCL`, 0600, written in chunks and fsync.
/// Returns the guard of the temporary file (deletes on drop, if still active) and the
/// identity read from the fd after the fsync (D62).
fn write_temp(
    dir: &Path,
    name: &std::ffi::OsStr,
    file: &[u8],
    point: &mut dyn FnMut(Point) -> io::Result<()>,
) -> Result<(TempFile, FileIdentity), WriteError> {
    // S2: only a well-formed encrypted vault goes to disk.
    format::read_header(file).map_err(|_| WriteError::NotVault)?;

    let mut random = [0u8; 16];
    crypto::random(&mut random).map_err(|_| WriteError::Io {
        step: Step::CreateTemp,
        kind: io::ErrorKind::Other,
        identity: None,
    })?;
    let mut tmp_name = OsString::from(".");
    tmp_name.push(name);
    tmp_name.push(TEMP_MARK);
    for b in random {
        tmp_name.push(format!("{b:02x}"));
    }
    let tmp_path = dir.join(tmp_name);

    let mut f = create_temp(&tmp_path).map_err(error(Step::CreateTemp))?;
    let tmp = TempFile {
        path: tmp_path,
        active: true,
    };
    point(Point::AfterCreate).map_err(error(Step::CreateTemp))?;

    let mut written = 0;
    for chunk in file.chunks(CHUNK) {
        f.write_all(chunk).map_err(error(Step::Write))?;
        written += chunk.len();
        point(Point::Writing(written)).map_err(error(Step::Write))?;
    }
    point(Point::AfterWrite).map_err(error(Step::Write))?;

    // `sync_all` is fsync(2): data and metadata of the temporary file on disk before the
    // rename.
    f.sync_all().map_err(error(Step::FsyncTemp))?;
    point(Point::AfterFsync).map_err(error(Step::FsyncTemp))?;
    // D62: the identity comes from the `fstat` of the temporary file's fd, already
    // synced. The rename changes neither dev, inode, `mtime` nor size; after it, nothing
    // else is read through the path (another process can swap the file right after the
    // rename).
    let identity = f
        .metadata()
        .map(|m| FileIdentity::from(&m))
        .map_err(error(Step::FsyncTemp))?;
    drop(f);
    Ok((tmp, identity))
}

/// fsync of the directory after the rename (or the link), with the test points.
fn sync_dir(
    dir: &Path,
    identity: FileIdentity,
    point: &mut dyn FnMut(Point) -> io::Result<()>,
) -> Result<(), WriteError> {
    let after_rename = move |e: io::Error| WriteError::Io {
        step: Step::FsyncDir,
        kind: e.kind(),
        identity: Some(identity),
    };
    point(Point::AfterRename).map_err(after_rename)?;
    // fsync of the directory: the name change also reaches the disk.
    File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(after_rename)?;
    point(Point::AfterFsyncDir).map_err(after_rename)?;
    Ok(())
}

fn write_with_points(
    destination: &Path,
    file: &[u8],
    point: &mut dyn FnMut(Point) -> io::Result<()>,
) -> Result<FileIdentity, WriteError> {
    format::read_header(file).map_err(|_| WriteError::NotVault)?;
    let (dir, name) = split(destination).ok_or(WriteError::InvalidDestination)?;
    let (mut tmp, identity) = write_temp(&dir, &name, file, point)?;
    fs::rename(&tmp.path, destination).map_err(error(Step::Rename))?;
    tmp.active = false;
    sync_dir(&dir, identity, point)?;
    Ok(identity)
}

/// Atomically writes a file that is already encrypted in `dir` with the first free name
/// of `candidates`, **without overwriting** anything (backup, Phase 45): temporary file
/// (like [`write`]) → fsync → `link(2)` to the final name, which fails if the name
/// already exists (file, symbolic link or whatever) → deletes the temporary file → fsync
/// of the directory. The final name only appears with the whole file synced. Returns the
/// index of the name used and the identity (D62). No free name:
/// `Io { Rename, AlreadyExists }`.
pub(crate) fn write_new_with_points(
    dir: &Path,
    tmp_name: &std::ffi::OsStr,
    candidates: &[OsString],
    file: &[u8],
    point: &mut dyn FnMut(Point) -> io::Result<()>,
) -> Result<(usize, FileIdentity), WriteError> {
    let (tmp, identity) = write_temp(dir, tmp_name, file, point)?;
    for (i, name) in candidates.iter().enumerate() {
        // `linkat(…, 0)`: does not follow a symbolic link at the source; existing
        // destination → EEXIST.
        match fs::hard_link(&tmp.path, dir.join(name)) {
            Ok(()) => {
                // The guard deletes the temporary file; the final name keeps the inode.
                drop(tmp);
                sync_dir(dir, identity, point)?;
                return Ok((i, identity));
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(error(Step::Rename)(e)),
        }
    }
    Err(WriteError::Io {
        step: Step::Rename,
        kind: io::ErrorKind::AlreadyExists,
        identity: None,
    })
}

#[cfg(test)]
mod tests;
