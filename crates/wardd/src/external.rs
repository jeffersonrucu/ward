//! External backup on a USB drive (R27, D17, D30; Phase 46).
//!
//! - **Off by default.** On/off and the USB drive's UUID live in the vault config (D30),
//!   inside the encrypted part: the daemon only knows which USB drive it is with the
//!   vault unlocked. Locked or off, nothing is copied.
//! - **Which USB drive.** udisks2 ([`crate::udisks`]) says which file systems exist, the
//!   UUID of each one, the device number and where they are mounted. Ward never mounts
//!   anything: it copies when the chosen USB drive shows up mounted (by the desktop
//!   environment or by the user). The UUID has to match a single file system (two
//!   cloned USB drives with the same UUID: nothing is copied).
//! - **Swapped USB drive.** The mount point is opened with `O_NOFOLLOW | O_DIRECTORY`
//!   and the fd's `st_dev` has to be the device that udisks2 gave for that UUID. Once it
//!   is open, udisks2 is queried again and has to say the same (UUID, device and mount
//!   point). From then on everything goes through the fd: swapping the USB drive, or
//!   mounting another one on the same path, does not divert the copy to it.
//! - **Directory `<mount>/ward-backup/`.** Created if missing, opened with `O_NOFOLLOW`
//!   (a symbolic link in its place, on an ext4 USB drive, is refused), on the same device
//!   and owned by the user. The permission is not checked: vfat has no owner nor mode
//!   per file (they come from the mount options). The files are the encrypted backup
//!   (S2), and nothing besides them goes to the USB drive.
//! - **What goes.** The versions of the local backup (Phase 45, already encrypted with
//!   the master password of each version) that the USB drive does not have yet, up to
//!   [`VERSIONS`]: only the user's regular files, with the exact name, the format
//!   header and, since Phase 54, the generation sealed by the key of the open vault, not
//!   out of order. The daemon checks the generation (decrypts and zeroes); nothing is
//!   sealed here and the bytes copied are the file's.
//! - **No `link(2)`** (vfat does not have it): temporary `O_EXCL` 0600 → fsync →
//!   `renameat2(RENAME_NOREPLACE)` to the final name ([`rename_no_replace`]) → fsync of
//!   the directory. A version that is already on the USB drive is never overwritten; an
//!   interrupted copy leaves only a temporary file with the encrypted prefix, deleted on
//!   the next one.
//! - After copying, the USB drive keeps the [`VERSIONS`] newest (by the date in the
//!   name), never deleting what it has just copied nor what has no version name.
//!   **Future dates** (Phase 49a): only versions dated up to now count and can be
//!   deleted; the ones planted with a future date (on the USB drive or in the local
//!   backup) do not push the real ones out. From the local backup only the versions up
//!   to now + [`ward_core::backup::FUTURE_MARGIN`] go.
//! - **Only what checks out counts and leaves** (Phase 54c): the USB drive rotation
//!   reads each version from there and checks the generation sealed by the key of the
//!   open vault ([`ward_core::backup::to_delete_verified`]): the [`VERSIONS`] highest
//!   generations stay, each one counted once. Planted names (on vfat, another user of the
//!   machine can write) with a recent past date, junk, copies and versions from before a
//!   master password change do not count and are not deleted.
//! - **Clock that goes back** (Phase 54c): a version written with the clock behind has a
//!   name older than versions of a lower generation; it comes out "out of order" and
//!   **does not go** to the USB drive (fail closed: nothing on the USB drive is deleted
//!   because of it). When the clock is right again, the next versions go normally.
//! - **The USB drive is recognized only by the file system UUID** (Phase 49a). The UUID
//!   is not a secret nor proof of anything: whoever has a USB drive can format another
//!   one with the same UUID (`mkfs.vfat -i`, `tune2fs -U`) and, with it connected and
//!   mounted in place of the chosen one, receives the copies. That opens nothing,
//!   because each copy is the file encrypted with the version's master password (S2,
//!   S6), but it is one more copy out of your hands. Two USB drives with the same UUID
//!   connected at the same time receive nothing (`Ambiguous`). There is no marker in the
//!   `ward-backup/` directory: a file planted on the USB drive would be no harder to
//!   copy than the UUID.
//!
//! The log only has fixed texts ([`ExternalError::code`]): no UUID, no label, no path.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use ward_core::backup::{Version, to_delete_verified};
use ward_core::format;
use ward_platform::dir::rename_no_replace;

use crate::arquivos::{self, PastaAberta};

/// Directory of the copies on the USB drive (D17).
pub const BACKUP_DIR: &str = "ward-backup";
/// Prefix of the temporary files in the USB drive's directory.
const TEMP_PREFIX: &str = ".ward-tmp-";

/// A file system that udisks2 shows (only what Ward uses).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    /// File system UUID (`IdUUID`), already checked against the D45 format.
    pub uuid: String,
    /// Label (`IdLabel`), without control characters.
    pub label: String,
    /// First mount point (`MountPoints`), absolute; `None` if not mounted.
    pub mount: Option<PathBuf>,
    /// Device number (`DeviceNumber`), the same `dev_t` as `st_dev`.
    pub device: u64,
}

/// Where the USB drives come from (udisks2 in the daemon; a list in the tests).
pub trait UsbDriveSource: Send + Sync {
    /// The USB drive file systems seen now; `None` if the source does not answer.
    fn usb_drives(&self) -> Option<Vec<Seen>>;
}

/// Why the copy did not happen. No UUID, label, path nor content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalError {
    /// udisks2 did not answer.
    NoUdisks,
    /// The chosen USB drive is not connected.
    NotConnected,
    /// More than one file system with the same UUID.
    Ambiguous,
    /// Connected, but not mounted.
    NotMounted,
    /// The opened mount point is not the one of the UUID's device (swapped USB drive,
    /// link, another file system on the path) or udisks2 changed its mind halfway.
    Swapped,
    /// `ward-backup` is not a user directory on the USB drive (link, another owner…).
    UnsafeDir,
    /// The local backup directory does not open (Phase 45).
    NoLocalBackup,
    /// I/O failure on the USB drive (full, removed halfway…).
    Io(io::ErrorKind),
}

impl ExternalError {
    /// Fixed text for the log.
    pub fn code(self) -> &'static str {
        match self {
            Self::NoUdisks => "udisks2 indisponível",
            Self::NotConnected => "pendrive não conectado",
            Self::Ambiguous => "mais de um pendrive com o mesmo UUID",
            Self::NotMounted => "pendrive não montado",
            Self::Swapped => "a montagem não é a do pendrive escolhido",
            Self::UnsafeDir => "pasta ward-backup insegura no pendrive",
            Self::NoLocalBackup => "pasta de backups local indisponível",
            Self::Io(_) => "e/s no pendrive",
        }
    }
}

/// What a copy did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackupCopy {
    /// Versions copied now (the ones the USB drive did not have).
    pub copied: Vec<Version>,
    /// Old versions deleted from the USB drive by the rotation.
    pub deleted: usize,
}

/// The only file system with the `uuid` (case-insensitive, as vfat's `IdUUID` comes).
pub fn choose<'a>(seen: &'a [Seen], uuid: &str) -> Result<&'a Seen, ExternalError> {
    let mut found = seen.iter().filter(|v| v.uuid.eq_ignore_ascii_case(uuid));
    let v = found.next().ok_or(ExternalError::NotConnected)?;
    if found.next().is_some() {
        return Err(ExternalError::Ambiguous);
    }
    Ok(v)
}

fn io(e: io::Error) -> ExternalError {
    ExternalError::Io(e.kind())
}

fn open_dir(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_CLOEXEC)
        .open(path)
}

fn via_fd(f: &File) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", f.as_raw_fd()))
}

/// Opens the USB drive's mount point and checks that it is the one of the UUID's
/// device, including asking udisks2 again after it is open.
fn open_mount(seen: &Seen, uuid: &str, source: &dyn UsbDriveSource) -> Result<File, ExternalError> {
    let mount = seen.mount.as_deref().ok_or(ExternalError::NotMounted)?;
    let fd = open_dir(mount).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => ExternalError::NotMounted,
        _ => ExternalError::Swapped,
    })?;
    let m = fd.metadata().map_err(io)?;
    if !m.is_dir() || m.dev() != seen.device {
        return Err(ExternalError::Swapped);
    }
    // Again, with the fd already open: what udisks2 says now has to be the same. A swap
    // before the opening shows up here; one after it does not change where the fd
    // points.
    let now = source.usb_drives().ok_or(ExternalError::NoUdisks)?;
    let again = choose(&now, uuid)?;
    if again.device != seen.device || again.mount != seen.mount {
        return Err(ExternalError::Swapped);
    }
    Ok(fd)
}

/// `<mount>/ward-backup`, created if missing, through the mount point's fd.
fn open_destination(mount: &File, device: u64) -> Result<File, ExternalError> {
    let path = via_fd(mount).join(BACKUP_DIR);
    match fs::DirBuilder::new().mode(0o700).create(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(io(e)),
    }
    let fd = open_dir(&path).map_err(|e| match e.kind() {
        // `ELOOP` (symbolic link) and `ENOTDIR` (a file in its place).
        io::ErrorKind::NotADirectory => ExternalError::UnsafeDir,
        _ if e.raw_os_error() == Some(libc::ELOOP) => ExternalError::UnsafeDir,
        _ => io(e),
    })?;
    let m = fd.metadata().map_err(io)?;
    if !m.is_dir() || m.dev() != device || m.uid() != ward_platform::effective_uid() {
        return Err(ExternalError::UnsafeDir);
    }
    Ok(fd)
}

/// Versions with an exact name that are regular files in the directory (without
/// following links).
fn versions_in(dir: &Path) -> Result<Vec<Version>, ExternalError> {
    let mut v = Vec::new();
    for item in fs::read_dir(dir).map_err(io)? {
        let item = item.map_err(io)?;
        let Some(version) = item.file_name().to_str().and_then(Version::from_name) else {
            continue;
        };
        if item.file_type().map_err(io)?.is_file() {
            v.push(version);
        }
    }
    v.sort_unstable_by(|a, b| b.cmp(a));
    Ok(v)
}

/// The generation sealed by the key of the open vault of each version in `dir` (Phases
/// 54 and 54c), in the order of `versions`: so as not to copy again a generation that
/// already went (under another name) and for the rotation. A file that does not open
/// (junk, another vault, link, FIFO, too big) gets `None`.
fn generations_in<'a>(
    dir: &Path,
    versions: impl IntoIterator<Item = &'a Version>,
    generation: &dyn Fn(&[u8]) -> Option<u64>,
) -> Vec<(Version, Option<u64>)> {
    versions
        .into_iter()
        .map(|v| {
            (
                *v,
                read_version(&dir.join(v.name())).and_then(|b| generation(&b)),
            )
        })
        .collect()
}

/// Reads a version from the USB drive: `O_NOFOLLOW | O_NONBLOCK` (link and FIFO hold
/// nothing), regular file, up to the format's size.
fn read_version(path: &Path) -> Option<Vec<u8>> {
    let f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .ok()?;
    let regular = f
        .metadata()
        .is_ok_and(|m| m.file_type().is_file() && m.len() <= arquivos::MAX_ARQUIVO_COFRE);
    if !regular {
        return None;
    }
    let mut bytes = Vec::new();
    (&f).take(arquivos::MAX_ARQUIVO_COFRE + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= arquivos::MAX_ARQUIVO_COFRE).then_some(bytes)
}

/// Deletes the temporary files of an interrupted copy (only regular files with the
/// prefix).
fn clean_temp_files(dir: &Path) {
    let Ok(items) = fs::read_dir(dir) else {
        return;
    };
    for item in items.flatten() {
        let name = item.file_name();
        let is_temp = name.to_str().is_some_and(|n| n.starts_with(TEMP_PREFIX));
        if is_temp && item.file_type().is_ok_and(|t| t.is_file()) {
            let _ = fs::remove_file(dir.join(name));
        }
    }
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_name() -> OsString {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    format!(
        "{TEMP_PREFIX}{}-{}-{nanos:08x}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
    .into()
}

/// Writes `bytes` as version `v` in the directory, without overwriting. `Ok(false)`: the
/// name already existed (nothing changed).
fn write_version(destination: &File, v: Version, bytes: &[u8]) -> io::Result<bool> {
    let dir = via_fd(destination);
    let tmp = temp_name();
    let tmp_path = dir.join(&tmp);
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&tmp_path)?;
    let written = f.write_all(bytes).and_then(|()| f.sync_all());
    drop(f);
    let r = written.and_then(|()| rename_no_replace(destination.as_fd(), &tmp, v.name().as_ref()));
    match r {
        Ok(()) => Ok(true),
        Err(e) => {
            let _ = fs::remove_file(&tmp_path);
            if e.kind() == io::ErrorKind::AlreadyExists {
                Ok(false)
            } else {
                Err(e)
            }
        }
    }
}

/// Deletes the oldest-generation versions from the USB drive until
/// [`ward_core::backup::VERSIONS`] generations dated up to `cap` (now) remain, never one
/// of the `new_ones`, never one dated after the `cap` (Phase 49a), never one that does
/// not check out with the key of the open vault (Phase 54c) and only regular files.
fn rotate(
    destination: &File,
    new_ones: &[Version],
    cap: Version,
    generation: &dyn Fn(&[u8]) -> Option<u64>,
) -> Result<usize, ExternalError> {
    let dir = via_fd(destination);
    let up_to: Vec<Version> = versions_in(&dir)?
        .into_iter()
        .filter(|v| *v <= cap)
        .collect();
    let list = generations_in(&dir, &up_to, generation);
    let mut deleted = 0;
    for v in to_delete_verified(&list, cap, new_ones) {
        let path = dir.join(v.name());
        if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_file()) {
            fs::remove_file(&path).map_err(io)?;
            deleted += 1;
        }
    }
    Ok(deleted)
}

/// Copies to the USB drive `seen` (the `uuid` one) the local backup versions it does not
/// have yet, and rotates to [`VERSIONS`]. Nothing is overwritten.
///
/// Phase 54 (final audit): only the versions that `generation` recognizes (the
/// generation sealed by the key of the open vault,
/// [`ward_core::backup::sealed_generation`]) and that are not out of order
/// ([`ward_core::backup::out_of_order`]) go. Junk with a valid header or the copy of an
/// old version under a new name, planted in the local directory, do not go to the USB
/// drive and, because of that, do not make the rotation delete the real copies there.
/// Without an open vault (`generation` returns `None` for everything), nothing is copied
/// nor deleted.
///
/// Phase 54c: the USB drive rotation also only counts and only deletes what
/// `generation` recognizes ([`rotate`]).
pub fn copy(
    local: &PastaAberta,
    seen: &Seen,
    uuid: &str,
    source: &dyn UsbDriveSource,
    generation: &dyn Fn(&[u8]) -> Option<u64>,
) -> Result<BackupCopy, ExternalError> {
    let now = SystemTime::now();
    let cap = Version::from_instant(now)
        .map(Version::end_of_second)
        .ok_or(ExternalError::NoLocalBackup)?;
    let mount = open_mount(seen, uuid, source)?;
    let destination = open_destination(&mount, seen.device)?;
    drop(mount);
    let dir = via_fd(&destination);
    clean_temp_files(&dir);
    let there: BTreeSet<Version> = versions_in(&dir)?.into_iter().collect();
    let local_dir = local.pasta();
    // Without the local versions with a future date (Phase 49a).
    let local_versions = local_dir
        .list_until(now)
        .map_err(|_| ExternalError::NoLocalBackup)?;
    // The newest `VERSIONS` that check out, read once (what is checked is what goes).
    // Planted junk does not take the place of the real ones.
    let mut authentic: Vec<(Version, u64, Vec<u8>)> = Vec::new();
    for v in local_versions {
        if authentic.len() == ward_core::backup::VERSIONS {
            break;
        }
        // The user's regular file, without link, of the format's size.
        let bytes = match arquivos::ler_cofre(&local_dir.path(v)) {
            Ok(Some((b, _))) => b,
            // Gone through the local rotation halfway, or not a file of the user.
            Ok(None) | Err(_) => continue,
        };
        if format::read_header(&bytes).is_err() {
            continue;
        }
        // Phase 54: sealed by the key of the open vault (the AEAD authenticates the
        // generation).
        let Some(g) = generation(&bytes) else {
            continue;
        };
        authentic.push((v, g, bytes));
    }
    // A generation only once (the oldest version with it is the original; the others
    // are copies under a new name).
    let mut firsts = BTreeSet::new();
    let authentic: Vec<(Version, u64, Vec<u8>)> = {
        let mut v: Vec<_> = authentic
            .into_iter()
            .rev()
            .filter(|(_, g, _)| firsts.insert(*g))
            .collect();
        v.reverse();
        v
    };
    let on_usb_drive: BTreeSet<u64> = generations_in(&dir, &there, generation)
        .into_iter()
        .filter_map(|(_, g)| g)
        .collect();
    let marks = ward_core::backup::out_of_order(
        &authentic
            .iter()
            .map(|(v, g, _)| (*v, Some(*g)))
            .collect::<Vec<_>>(),
    );
    let mut result = BackupCopy::default();
    // From the oldest to the newest.
    for ((v, g, bytes), misplaced) in authentic.into_iter().zip(marks).rev() {
        if misplaced || there.contains(&v) || on_usb_drive.contains(&g) {
            continue;
        }
        if write_version(&destination, v, &bytes).map_err(io)? {
            result.copied.push(v);
        }
    }
    result.deleted = rotate(&destination, &result.copied, cap, generation)?;
    // vfat accepts fsync of the directory; if another file system does not, the files
    // were already synced one by one.
    let _ = destination.sync_all();
    Ok(result)
}
