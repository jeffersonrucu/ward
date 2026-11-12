//! Automatic local backup (R27, D17, D27; Phase 45).
//!
//! - On every vault write that succeeded, the **same sealed version** (the encrypted
//!   bytes that just went to the vault) goes to the backups directory as
//!   `ward-YYYYMMDD-HHMMSS.ward`: same format, encrypted with the master password of that
//!   version. No plaintext byte passes through here (S2), and nothing is sealed again:
//!   the backup neither burns nor repeats a generation (D53).
//! - Date and time in **UTC** (the core has no time zone; the interface shows them in the
//!   local time zone). Two versions in the same second get a suffix:
//!   `ward-YYYYMMDD-HHMMSS-1.ward` up to `-99`, in that order.
//! - Atomic write through the internal path of [`crate::atomic_write`] (D27): temp file
//!   `O_EXCL` 0600 → fsync → `link(2)` to the final name, which **never overwrites** →
//!   directory fsync. A write interrupted at any point does not touch the backups that
//!   were already there (S7, V7.3); the temp file left behind is encrypted and is deleted
//!   at the start of the next write.
//! - Keeps [`VERSIONS`] versions: after writing, deletes those of the oldest generation,
//!   never the one just written, and only regular files with the exact name.
//! - **Only what verifies counts and leaves** (Phase 54c): the rotation reads each
//!   version and verifies the sealed generation with the key of the open vault
//!   ([`to_delete_verified`]). The [`VERSIONS`] highest generations stay, each one
//!   counted once; junk with a version name, a version from another vault, a copy of a
//!   real version under a new name and a version from before a master password change
//!   (another key) do not take the place of the real ones. What does not verify is never
//!   deleted by the rotation: only the user deletes it.
//! - **The date in the name is not trusted** (Phase 49a): any process of the user can put
//!   in the directory a file named with a future date. The rotation only counts and only
//!   deletes versions dated **up to the second of the version just written**: a version
//!   with a past date is never deleted to keep a future one, and the future ones are
//!   neither counted nor deleted. The list for the restore ([`BackupDir::list_until`])
//!   ignores the versions dated after now + [`FUTURE_MARGIN`], and the daemon verifies the
//!   sealed generation of each version ([`sealed_generation`], with the key of the open
//!   vault) and marks those that are out of order ([`out_of_order`]); the generation
//!   itself does not leave the daemon.
//! - The directory must be a directory of the user, with no permission for group and
//!   others (0700). The daemon opens it with `O_NOFOLLOW | O_DIRECTORY` and passes the
//!   path through the fd (`/proc/self/fd/<n>`): swapping the directory for a symbolic
//!   link, or for another directory in the middle of the operation, diverts nothing
//!   (V7.x, "directory swapped for a link").
//!
//! Restoring belongs to the daemon, only through the app role (D17, D39, V7.8): it opens
//! the version with its password ([`crate::body::open_entries_with_generation`] with no
//! minimum generation), keeps the current vault as a backup ([`keep_current`]) and seals
//! the version again with generation = highest burned + 1 through the normal write.

use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::atomic_write::{self, Point, WriteError};
use crate::format::OpenVault;

/// How many versions stay in the directory (D17).
pub const VERSIONS: usize = 30;
/// Highest suffix for versions in the same second.
pub const MAX_SUFFIX: u32 = 99;
/// Base of the temp file names in the directory (`.backup.ward-tmp-<hex>`).
const TEMP_BASE: &str = "backup";
/// Length of the longest valid name: `ward-YYYYMMDD-HHMMSS-NN.ward`.
pub const MAX_NAME: usize = 28;
/// Slack for the clock (Phase 49a): a version dated up to now + this margin still shows
/// in the restore list (a small backwards adjustment of the clock does not hide the
/// newest write). Beyond that, the version is ignored until the clock gets there.
pub const FUTURE_MARGIN: Duration = Duration::from_secs(5 * 60);

/// A backup version, identified by the file name. The order is the order of time: date,
/// time and suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    /// YYYYMMDD.
    date: u32,
    /// HHMMSS.
    time: u32,
    /// 0 = no suffix.
    suffix: u32,
}

impl Version {
    /// The version of an instant (UTC), with no suffix. `None` before 1970 or after
    /// 9999.
    pub fn from_instant(t: SystemTime) -> Option<Self> {
        let s = t.duration_since(UNIX_EPOCH).ok()?.as_secs();
        let days = i64::try_from(s / 86_400).ok()?;
        let rest = s % 86_400;
        let (y, m, d) = civil_from_days(days);
        if !(1970..=9999).contains(&y) {
            return None;
        }
        let date = u32::try_from(y * 10_000 + m * 100 + d).ok()?;
        let time =
            u32::try_from((rest / 3600) * 10_000 + (rest % 3600 / 60) * 100 + rest % 60).ok()?;
        Some(Self {
            date,
            time,
            suffix: 0,
        })
    }

    /// Reads the file name, strictly: `ward-YYYYMMDD-HHMMSS.ward` or with `-N` (1 to 99,
    /// no leading zero) before the `.ward`; a possible date and time. Anything else
    /// (slash, `..`, space, uppercase) is `None`.
    pub fn from_name(name: &str) -> Option<Self> {
        if name.len() > MAX_NAME || !name.is_ascii() {
            return None;
        }
        let core = name.strip_prefix("ward-")?.strip_suffix(".ward")?;
        let b = core.as_bytes();
        if b.len() < 15 || b[8] != b'-' {
            return None;
        }
        let date = digits(&b[..8])?;
        let time = digits(&b[9..15])?;
        let suffix = match &b[15..] {
            [] => 0,
            [b'-', rest @ ..] if !rest.is_empty() && rest.len() <= 2 && rest[0] != b'0' => {
                digits(rest)?
            }
            _ => return None,
        };
        let v = Self { date, time, suffix };
        v.is_possible().then_some(v)
    }

    /// File name.
    pub fn name(&self) -> String {
        if self.suffix == 0 {
            format!("ward-{:08}-{:06}.ward", self.date, self.time)
        } else {
            format!(
                "ward-{:08}-{:06}-{}.ward",
                self.date, self.time, self.suffix
            )
        }
    }

    /// The highest possible version in the same second as `self` (suffix
    /// [`MAX_SUFFIX`]): the ceiling of the versions "up to this instant".
    pub fn end_of_second(self) -> Self {
        self.with_suffix(MAX_SUFFIX)
    }

    fn with_suffix(self, suffix: u32) -> Self {
        Self { suffix, ..self }
    }

    fn is_possible(&self) -> bool {
        let (y, m, d) = (self.date / 10_000, self.date / 100 % 100, self.date % 100);
        let (h, mi, s) = (self.time / 10_000, self.time / 100 % 100, self.time % 100);
        (1970..=9999).contains(&y)
            && (1..=12).contains(&m)
            && (1..=31).contains(&d)
            && h < 24
            && mi < 60
            && s < 60
            && self.suffix <= MAX_SUFFIX
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name())
    }
}

fn digits(b: &[u8]) -> Option<u32> {
    if b.is_empty() || !b.iter().all(u8::is_ascii_digit) {
        return None;
    }
    b.iter().try_fold(0u32, |n, &c| {
        n.checked_mul(10)?.checked_add(u32::from(c - b'0'))
    })
}

/// Days since 1970-01-01 → (year, month, day) in the proleptic Gregorian calendar
/// (Howard Hinnant's `civil_from_days` algorithm; integer arithmetic only).
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// Backup error. Carries no path and no content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupError {
    /// The directory is not a 0700 directory of the user (or was swapped for a link).
    UnsafeDir,
    /// The current vault file was not sealed by this vault ([`keep_current`]).
    NotAuthentic,
    /// The clock is before 1970 or after 9999.
    Clock,
    /// The 100 names of the same second already exist.
    NamesExhausted,
    /// The atomic write failed (what was in the directory stays the same).
    Write(WriteError),
    /// I/O failure while listing or deleting old versions.
    Io(io::ErrorKind),
}

impl fmt::Display for BackupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsafeDir => f.write_str("a pasta de backups não é um diretório 0700 do usuário"),
            Self::NotAuthentic => f.write_str("o arquivo do cofre não é o que o Ward gravou"),
            Self::Clock => f.write_str("o relógio do sistema está fora do intervalo"),
            Self::NamesExhausted => f.write_str("backups demais no mesmo segundo"),
            Self::Write(e) => write!(f, "falha ao gravar o backup: {e}"),
            Self::Io(k) => write!(f, "falha de E/S na pasta de backups ({k})"),
        }
    }
}

impl std::error::Error for BackupError {}

/// How the rotation reads a version of the directory to verify the sealed generation
/// (Phase 54c): `None` if it is not a regular file of the user with the size of the
/// format, or could not be read. The daemon passes a reader with
/// `O_NOFOLLOW | O_NONBLOCK` ([`BackupDir::with_reader`]).
pub type ReadVersion = fn(&Path, u32) -> Option<Vec<u8>>;

/// The backups directory and the expected owner uid.
#[derive(Debug, Clone)]
pub struct BackupDir {
    dir: PathBuf,
    uid: u32,
    read: ReadVersion,
}

impl BackupDir {
    /// The directory at `dir`, owned by `uid`. The daemon passes the path through the fd
    /// opened with `O_NOFOLLOW` (`/proc/self/fd/<n>`), so `dir` is followed here: whoever
    /// opened it guarantees it is not a swapped symbolic link. Even so, every operation
    /// verifies that it is a directory of `uid`, with no permission for group and others.
    pub fn new(dir: PathBuf, uid: u32) -> Self {
        Self {
            dir,
            uid,
            read: read_version_default,
        }
    }

    /// Replaces the reader of the rotation's versions (Phase 54c). The core has no
    /// `libc`; the daemon passes one that opens with `O_NOFOLLOW | O_NONBLOCK` (a FIFO
    /// swapped in place of a version does not block the write).
    pub fn with_reader(mut self, read: ReadVersion) -> Self {
        self.read = read;
        self
    }

    /// Path of the file of a version.
    pub fn path(&self, v: Version) -> PathBuf {
        self.dir.join(v.name())
    }

    fn check(&self) -> Result<(), BackupError> {
        let m = fs::metadata(&self.dir).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => BackupError::UnsafeDir,
            k => BackupError::Io(k),
        })?;
        if !m.is_dir() || m.uid() != self.uid || m.permissions().mode() & 0o077 != 0 {
            return Err(BackupError::UnsafeDir);
        }
        Ok(())
    }

    /// The versions in the directory, from the newest to the oldest. Only regular files
    /// (without following a symbolic link) with the exact name; the rest is ignored.
    pub fn list(&self) -> Result<Vec<Version>, BackupError> {
        self.check()?;
        let mut v = Vec::new();
        for item in fs::read_dir(&self.dir).map_err(|e| BackupError::Io(e.kind()))? {
            let item = item.map_err(|e| BackupError::Io(e.kind()))?;
            let Some(version) = item.file_name().to_str().and_then(Version::from_name) else {
                continue;
            };
            // `DirEntry::file_type` does not follow a symbolic link.
            if item
                .file_type()
                .map_err(|e| BackupError::Io(e.kind()))?
                .is_file()
            {
                v.push(version);
            }
        }
        v.sort_unstable_by(|a, b| b.cmp(a));
        Ok(v)
    }

    /// The versions of the directory that are not in the future: dated up to `now` +
    /// [`FUTURE_MARGIN`] (Phase 49a), from the newest to the oldest. A version planted
    /// with a future date does not show as the newest in the restore nor go to the USB
    /// drive.
    pub fn list_until(&self, now: SystemTime) -> Result<Vec<Version>, BackupError> {
        let ceiling = ceiling_of(now)?;
        let mut v = self.list()?;
        v.retain(|x| *x <= ceiling);
        Ok(v)
    }

    /// Writes `file` (already sealed by `vault` and with the structure verified) as the
    /// version of `instant` and deletes those beyond [`VERSIONS`] among the ones that
    /// verify with the key of `vault` (Phase 54c, [`to_delete_verified`]).
    pub(crate) fn keep(
        &self,
        file: &[u8],
        instant: SystemTime,
        vault: &OpenVault,
    ) -> Result<Version, BackupError> {
        self.keep_with_points(
            file,
            instant,
            &|b: &[u8]| sealed_generation(vault, b),
            &mut |_| Ok(()),
        )
    }

    /// [`BackupDir::keep`] with the generation check (`generation`) and the write points
    /// passed separately (the V7.3 tests replace them).
    pub(crate) fn keep_with_points(
        &self,
        file: &[u8],
        instant: SystemTime,
        generation: &dyn Fn(&[u8]) -> Option<u64>,
        point: &mut dyn FnMut(Point) -> io::Result<()>,
    ) -> Result<Version, BackupError> {
        self.check()?;
        let base = Version::from_instant(instant).ok_or(BackupError::Clock)?;
        // Temp files of an interrupted write (encrypted), only those of the directory.
        // Before writing: never more than one is left. Only the daemon writes, one at a
        // time.
        let _ = atomic_write::clean_temp_files(&self.dir.join(TEMP_BASE));
        let versions: Vec<Version> = (0..=MAX_SUFFIX).map(|s| base.with_suffix(s)).collect();
        let names: Vec<OsString> = versions.iter().map(|v| v.name().into()).collect();
        let (i, _) =
            atomic_write::write_new_with_points(&self.dir, TEMP_BASE.as_ref(), &names, file, point)
                .map_err(|e| match e {
                    WriteError::Io {
                        kind: io::ErrorKind::AlreadyExists,
                        ..
                    } => BackupError::NamesExhausted,
                    e => BackupError::Write(e),
                })?;
        let new = versions[i];
        self.rotate(new, generation)?;
        Ok(new)
    }

    /// Deletes the versions of the oldest generation until [`VERSIONS`] generations dated
    /// up to the second of `new` are left, never `new`, never one dated after that second
    /// and never one that does not verify with the key of `vault`
    /// ([`to_delete_verified`], Phase 54c). The ceiling is the end of the second (suffix
    /// 99), not `new`: the rotation may have freed a name with a lower suffix in the same
    /// second, which `new` reuses. Only the versions up to the ceiling are read and
    /// decrypted (the content is zeroed on drop; only the generation stays).
    fn rotate(
        &self,
        new: Version,
        generation: &dyn Fn(&[u8]) -> Option<u64>,
    ) -> Result<(), BackupError> {
        let ceiling = new.end_of_second();
        let list: Vec<(Version, Option<u64>)> = self
            .list()?
            .into_iter()
            .filter(|v| *v <= ceiling)
            .map(|v| {
                let g = (self.read)(&self.path(v), self.uid).and_then(|b| generation(&b));
                (v, g)
            })
            .collect();
        for v in to_delete_verified(&list, ceiling, &[new]) {
            let path = self.path(v);
            // Regular file only: a link put in its place is neither followed nor deleted.
            match fs::symlink_metadata(&path) {
                Ok(m) if m.file_type().is_file() => {
                    fs::remove_file(&path).map_err(|e| BackupError::Io(e.kind()))?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// The ceiling of the versions accepted at `now`: the end of the second of now +
/// [`FUTURE_MARGIN`].
fn ceiling_of(now: SystemTime) -> Result<Version, BackupError> {
    let t = now.checked_add(FUTURE_MARGIN).ok_or(BackupError::Clock)?;
    Version::from_instant(t)
        .map(Version::end_of_second)
        .ok_or(BackupError::Clock)
}

/// The rotation since Phase 54c (local and on the USB drive): of the versions in `list`,
/// with the sealed generation of each one ([`sealed_generation`]; `None` = does not
/// verify with the key of the open vault), which to delete. **The date in the name decides
/// nothing beyond the ceiling**, and what does not verify never counts nor leaves:
/// - only the versions dated up to `ceiling` (Phase 49a) with a known generation count;
/// - the [`VERSIONS`] highest **generations** stay (the generation is authenticated by
///   the AEAD and never repeats, D53); each generation counts once, so copies of a real
///   version under new names take no place;
/// - the versions (originals and copies) of the lowest generations leave, except the
///   `protected` ones (what was just written or copied);
/// - junk, a version from another vault and a version from before a master password
///   change (another key) neither count nor are deleted: only the user deletes them.
///
/// A clock that went back deletes nothing more: the new version, with an older date, has
/// the highest generation. Returns from the lowest generation to the highest (and, within
/// the same generation, from the oldest date to the newest).
pub fn to_delete_verified(
    list: &[(Version, Option<u64>)],
    ceiling: Version,
    protected: &[Version],
) -> Vec<Version> {
    let mut verified: Vec<(u64, Version)> = list
        .iter()
        .filter(|(v, _)| *v <= ceiling)
        .filter_map(|(v, g)| g.map(|g| (g, *v)))
        .collect();
    verified.sort_unstable();
    verified.dedup();
    let mut generations: Vec<u64> = verified.iter().map(|(g, _)| *g).collect();
    generations.dedup();
    let excess = generations.len().saturating_sub(VERSIONS);
    let Some(&last_out) = excess.checked_sub(1).and_then(|i| generations.get(i)) else {
        return Vec::new();
    };
    verified
        .into_iter()
        .filter(|(g, v)| *g <= last_out && !protected.contains(v))
        .map(|(_, v)| v)
        .collect()
}

/// Reads a version for the rotation with the standard library only (the default of
/// [`BackupDir::new`]; the daemon replaces it with one using `O_NOFOLLOW | O_NONBLOCK`):
/// a regular file of `uid` (without following a link), up to the size of the format, and
/// the opened file must be the same as the one from `lstat`.
fn read_version_default(path: &Path, uid: u32) -> Option<Vec<u8>> {
    use std::io::Read;
    let limit =
        (crate::format::HEADER_SIZE + crate::format::MAX_CONTENT + crate::crypto::TAG_SIZE) as u64;
    let before = fs::symlink_metadata(path).ok()?;
    if !before.file_type().is_file() || before.uid() != uid || before.len() > limit {
        return None;
    }
    let f = fs::File::open(path).ok()?;
    let m = f.metadata().ok()?;
    if !m.file_type().is_file() || m.dev() != before.dev() || m.ino() != before.ino() {
        return None;
    }
    let mut bytes = Vec::new();
    f.take(limit + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= limit).then_some(bytes)
}

/// Which versions are out of order (Phase 49a): by the date in the name, from the oldest
/// to the newest, a version whose sealed generation is lower than that of some version
/// with an earlier date (an old copy under a new name, or the clock that went backwards).
/// Versions with no known generation (`None`: another key) neither count nor come out
/// marked. Returns one mark per item, in the order of `list`. The generation stays in the
/// daemon: only the mark goes to the app (no response carries the generation, D39).
pub fn out_of_order(list: &[(Version, Option<u64>)]) -> Vec<bool> {
    let mut order: Vec<usize> = (0..list.len()).collect();
    order.sort_by_key(|&i| list[i].0);
    let mut marks = vec![false; list.len()];
    let mut highest: Option<u64> = None;
    for i in order {
        if let Some(g) = list[i].1 {
            marks[i] = highest.is_some_and(|m| g < m);
            highest = Some(highest.map_or(g, |m| m.max(g)));
        }
    }
    marks
}

/// The generation sealed in the body of `file` (Phase 49a), if it was sealed with the key
/// of the open `vault`: authenticated by the AEAD, so it cannot be forged without the
/// key. `None` when it is from another key (another master password, from before a
/// change) or does not open. The decrypted content is zeroed on drop; only the number
/// leaves here.
pub fn sealed_generation(vault: &OpenVault, file: &[u8]) -> Option<u64> {
    vault.sealed_generation(file)
}

/// Keeps the file of the current vault as a backup before restoring another version
/// (D17). Only if `file` was sealed by `vault` (same data key, authentic body): the daemon
/// passes what it read from the vault path, with the identity verified, and the open
/// vault it has in memory.
pub fn keep_current(
    dir: &BackupDir,
    vault: &OpenVault,
    file: &[u8],
    instant: SystemTime,
) -> Result<Version, BackupError> {
    if !vault.authenticates(file) {
        return Err(BackupError::NotAuthentic);
    }
    dir.keep(file, instant, vault)
}

/// Is the name of a version valid? (For the protocol to check the request.)
pub fn valid_name(name: &str) -> bool {
    Version::from_name(name).is_some()
}

#[cfg(test)]
mod tests;
