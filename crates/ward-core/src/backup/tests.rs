//! Local backup tests (Phase 45): names and dates, 30 versions (V7.1), no plaintext and
//! tampering (V7.2), SIGKILL during the backup write (V7.3), 0600/0700 permissions, unsafe
//! directory, link planted with the name of a version and the authentic current vault
//! (D17). Fake passwords and secrets of the test vault (D15).
//!
//! Files only in `target/<profile>/ward-test-backup/` (inside the repository, outside
//! git), deleted at the end of each test.

use std::io::{BufRead, BufReader};
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use base64ct::{Base64, Encoding};

use super::*;
use crate as wc;
use crate::atomic_write::{self, change_password_with_backup, write_entries_with_backup};
use crate::body;
use crate::entry::Entry;
use crate::format::MasterPassword;

#[allow(clippy::duplicate_mod)]
#[path = "../body/test_vault.rs"]
mod test_vault;
use test_vault::{MASTER_PASSWORD, SECRETS, new_config, test_vault_entries};

const CHILD_NAME: &str = "backup::tests::child_backup";
const ENV_MODE: &str = "WARD_TESTE_BACKUP_MODO";
const ENV_DIR: &str = "WARD_TESTE_BACKUP_DIR";
const ENV_BASE: &str = "WARD_TESTE_BACKUP_BASE";
/// If the parent dies without killing the child, the child exits on its own.
const CHILD_LIFETIME: Duration = Duration::from_secs(90);
/// 2026-11-09 15:30:00 UTC.
const INSTANT: u64 = 1_794_238_200;

/// One real Argon2id (256 MiB) at a time in these tests.
static ARGON: Mutex<()> = Mutex::new(());

fn password(s: &str) -> MasterPassword {
    MasterPassword::new(s.to_owned())
}

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

fn uid() -> u32 {
    // The owner of the test directory we just created is the euid of the test.
    fs::metadata(std::env::current_exe().unwrap().parent().unwrap())
        .unwrap()
        .uid()
}

/// A new open vault with the test password (one Argon2id).
fn new_vault(s: &str) -> OpenVault {
    let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
    OpenVault::new(&password(s), new_config()).unwrap()
}

/// The entries of the test vault (with the fake secrets), built once.
fn entries() -> &'static [Entry] {
    static E: OnceLock<Vec<Entry>> = OnceLock::new();
    E.get_or_init(test_vault_entries)
}

fn open(file: &[u8], s: &str) -> Result<(OpenVault, Vec<Entry>, u64), body::OpenError> {
    let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
    body::open_entries_with_generation(file, &password(s), 0, 0)
}

fn random_hex() -> String {
    let mut b = [0u8; 12];
    crate::crypto::random(&mut b).unwrap();
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn random_u64() -> u64 {
    let mut b = [0u8; 8];
    crate::crypto::random(&mut b).unwrap();
    u64::from_le_bytes(b)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|j| j == needle)
}

/// Test directory with the vault and the `backups/` directory (0700).
struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Self {
        let exe = std::env::current_exe().unwrap();
        let base = exe
            .parent()
            .and_then(Path::parent)
            .unwrap()
            .join("ward-test-backup");
        fs::create_dir_all(&base).unwrap();
        let d = base.join(format!("{name}-{}-{}", std::process::id(), random_hex()));
        fs::create_dir(&d).unwrap();
        fs::set_permissions(&d, fs::Permissions::from_mode(0o700)).unwrap();
        fs::create_dir(d.join("backups")).unwrap();
        fs::set_permissions(d.join("backups"), fs::Permissions::from_mode(0o700)).unwrap();
        Self(d)
    }

    fn vault(&self) -> PathBuf {
        self.0.join("cofre.ward")
    }

    fn backups(&self) -> PathBuf {
        self.0.join("backups")
    }

    fn backup_dir(&self) -> BackupDir {
        BackupDir::new(self.backups(), uid())
    }

    /// All the names in the backups directory, in order.
    fn names(&self) -> Vec<String> {
        let mut v: Vec<_> = fs::read_dir(self.backups())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    fn temp_files(&self) -> Vec<PathBuf> {
        self.names()
            .into_iter()
            .filter(|n| n.starts_with(".backup.ward-tmp-"))
            .map(|n| self.backups().join(n))
            .collect()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn mode(p: &Path) -> u32 {
    fs::symlink_metadata(p).unwrap().permissions().mode() & 0o7777
}

// --- Names ------------------------------------------------------------------------------

#[test]
fn name_of_the_instant_in_utc_and_round_trip() {
    let cases = [
        (0, "ward-19700101-000000.ward"),
        (INSTANT, "ward-20261109-153000.ward"),
        // 2000-02-29 23:59:59 (400-year leap year).
        (951_868_799, "ward-20000229-235959.ward"),
        // 2100-03-01 00:00:00 (2100 is not a leap year).
        (4_107_542_400, "ward-21000301-000000.ward"),
        // 2024-12-31 23:59:59.
        (1_735_689_599, "ward-20241231-235959.ward"),
    ];
    for (s, name) in cases {
        let v = Version::from_instant(at(s)).unwrap();
        assert_eq!(v.name(), name, "{s}");
        assert_eq!(Version::from_name(name), Some(v));
    }
    let v = Version::from_instant(at(INSTANT)).unwrap();
    for s in [1, 9, 10, 99] {
        let n = v.with_suffix(s).name();
        assert_eq!(Version::from_name(&n), Some(v.with_suffix(s)), "{n}");
        assert!(v.with_suffix(s) > v);
    }
    // The order is the order of time, not of the text (`-` comes before `.` in ASCII).
    assert!(v.with_suffix(1) > v);
    assert!(v.with_suffix(10) > v.with_suffix(9));
    // Before 1970: no name.
    assert_eq!(
        Version::from_instant(UNIX_EPOCH - Duration::from_secs(1)),
        None
    );
}

/// The name in the restore request never becomes a path: only the exact format.
#[test]
fn attack_name_out_of_format_refused() {
    for bad in [
        "",
        "ward-20261109-153000",
        "ward-20261109-153000.ward/",
        "../ward-20261109-153000.ward",
        "ward-20261109-153000.ward/../../cofre.ward",
        "/home/x/.local/share/ward/cofre.ward",
        "backups/ward-20261109-153000.ward",
        "ward-20261109-153000-0.ward",
        "ward-20261109-153000-01.ward",
        "ward-20261109-153000-100.ward",
        "ward-20261109-153000-.ward",
        "ward-20261109-153000.WARD",
        "WARD-20261109-153000.ward",
        "ward-20261109-1530000.ward",
        "ward-2026119-153000.ward",
        "ward-20261309-153000.ward",
        "ward-20261100-153000.ward",
        "ward-20261132-153000.ward",
        "ward-20261109-243000.ward",
        "ward-20261109-156000.ward",
        "ward-20261109-153060.ward",
        "ward-19691231-235959.ward",
        "ward-+0261109-153000.ward",
        "ward-20261109 153000.ward",
        "ward-20261109-153000.ward\0",
        "ward-２0261109-153000.ward",
        "ward-20261109-153000.ward.ward",
        " ward-20261109-153000.ward",
        ".ward-20261109-153000.ward",
    ] {
        assert!(!valid_name(bad), "{bad:?}");
    }
    assert!(valid_name("ward-20261109-153000.ward"));
    assert!(valid_name("ward-20261109-153000-99.ward"));
}

// --- V7.1: 30 versions -------------------------------------------------------------------

/// V7.1: 31 writes → 30 `ward-YYYYMMDD-HHMMSS.ward` files, the oldest one left; each
/// backup is the written version (same format, same generation), 0600 in a 0700
/// directory.
#[test]
fn v7_1_31_writes_leave_30_versions() {
    let d = Dir::new("v7-1");
    let mut vault = new_vault(MASTER_PASSWORD);
    let dir = d.backup_dir();
    let mut versions = Vec::new();
    for i in 0..31u64 {
        let g = write_entries_with_backup(
            &d.vault(),
            &mut vault,
            entries(),
            &dir,
            at(INSTANT + i * 61),
        )
        .unwrap();
        let v = g.backup.unwrap();
        // The backup is the same file that just went to the vault.
        assert_eq!(fs::read(dir.path(v)).unwrap(), fs::read(d.vault()).unwrap());
        versions.push(v);
    }
    let names = d.names();
    assert_eq!(names.len(), VERSIONS, "{names:?}");
    assert!(!names.contains(&versions[0].name()), "a mais antiga ficou");
    assert!(!fs::exists(dir.path(versions[0])).unwrap());
    for v in &versions[1..] {
        assert!(names.contains(&v.name()));
        let p = dir.path(*v);
        assert_eq!(mode(&p), 0o600, "{v}");
        assert!(vault.authenticates(&fs::read(&p).unwrap()), "{v}");
    }
    assert_eq!(mode(&d.backups()), 0o700);
    // The list comes from the newest to the oldest.
    let list = dir.list().unwrap();
    let mut expected: Vec<_> = versions[1..].to_vec();
    expected.reverse();
    assert_eq!(list, expected);
    // The newest opens with the password, at generation 31, with the 22 entries of the
    // fixture.
    let (_, read, generation) =
        open(&fs::read(dir.path(versions[30])).unwrap(), MASTER_PASSWORD).unwrap();
    assert_eq!(generation, 31);
    assert_eq!(read.len(), entries().len());
    // The second oldest that stayed is generation 2.
    let (_, _, g2) = open(&fs::read(dir.path(versions[1])).unwrap(), MASTER_PASSWORD).unwrap();
    assert_eq!(g2, 2);
}

/// Several versions in the same second get a suffix and none overwrites another.
#[test]
fn same_second_gets_suffix_without_overwriting() {
    let d = Dir::new("suffix");
    let mut vault = new_vault(MASTER_PASSWORD);
    let dir = d.backup_dir();
    let mut files = Vec::new();
    for _ in 0..3 {
        let g = write_entries_with_backup(&d.vault(), &mut vault, &[], &dir, at(INSTANT)).unwrap();
        files.push((g.backup.unwrap(), fs::read(d.vault()).unwrap()));
    }
    let names: Vec<_> = files.iter().map(|(v, _)| v.name()).collect();
    assert_eq!(
        names,
        [
            "ward-20261109-153000.ward",
            "ward-20261109-153000-1.ward",
            "ward-20261109-153000-2.ward"
        ]
    );
    for (v, bytes) in &files {
        assert_eq!(&fs::read(dir.path(*v)).unwrap(), bytes);
    }
    assert!(d.temp_files().is_empty());
}

// --- V7.2: no plaintext, wrong password and tampering -----------------------------------

/// V7.2 (S6): the backup has no secret in plaintext nor in base64; with the wrong
/// password it does not open; 1 byte changed at each position does not authenticate
/// (AEAD with the data key, with no Argon2 per position; the full password open of each
/// position is V2.2, in the same format) and, with the right password, a byte of each
/// field of the header and of the body makes the open fail.
#[test]
fn v7_2_backup_encrypted_wrong_password_and_changed_byte() {
    let d = Dir::new("v7-2");
    let mut vault = new_vault(MASTER_PASSWORD);
    let dir = d.backup_dir();
    let g =
        write_entries_with_backup(&d.vault(), &mut vault, entries(), &dir, at(INSTANT)).unwrap();
    let file = fs::read(dir.path(g.backup.unwrap())).unwrap();
    for s in SECRETS {
        assert!(!contains(&file, s.as_bytes()), "segredo em claro no backup");
        let b64 = Base64::encode_string(s.as_bytes());
        assert!(
            !contains(&file, &b64.as_bytes()[..b64.len() - 4]),
            "segredo em base64"
        );
    }
    assert!(!contains(&file, test_vault::TEST_PIN.as_bytes()));

    assert!(matches!(
        open(&file, "falso-mestra-ward-02"),
        Err(body::OpenError::Format(_))
    ));
    assert!(vault.authenticates(&file));
    for i in 0..file.len() {
        let mut a = file.clone();
        a[i] ^= 1 << (i % 8);
        assert!(
            !vault.authenticates(&a),
            "autenticou com o byte {i} trocado"
        );
    }
    // Password open with one byte changed in each field: magic, version, Argon2 memory,
    // salt, key nonce, wrapped key, data nonce, size, body and tag.
    for i in [0, 8, 12, 30, 60, 100, 140, 155, 200, file.len() - 1] {
        let mut a = file.clone();
        a[i] ^= 0x01;
        assert!(
            open(&a, MASTER_PASSWORD).is_err(),
            "abriu com o byte {i} trocado"
        );
    }
    // Control: the original opens.
    assert!(open(&file, MASTER_PASSWORD).is_ok());
}

/// Password change with backup: the new version only opens with the new password; the
/// previous one, which stayed in the directory, only with the old one (D17: restoring
/// asks for the password of that version).
#[test]
fn password_change_goes_to_the_backup_with_the_new_password() {
    let d = Dir::new("change");
    let mut vault = new_vault(MASTER_PASSWORD);
    let dir = d.backup_dir();
    let old = write_entries_with_backup(&d.vault(), &mut vault, &[], &dir, at(INSTANT))
        .unwrap()
        .backup
        .unwrap();
    let new_password = "falso-mestra-ward-03";
    let (new, g) = {
        let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
        change_password_with_backup(
            &d.vault(),
            &password(MASTER_PASSWORD),
            &password(new_password),
            1,
            1,
            &dir,
            at(INSTANT + 1),
        )
        .unwrap()
    };
    let v = g.backup.unwrap();
    let bytes = fs::read(dir.path(v)).unwrap();
    assert!(new.authenticates(&bytes));
    assert!(open(&bytes, MASTER_PASSWORD).is_err());
    assert_eq!(open(&bytes, new_password).unwrap().2, 2);
    assert_eq!(
        open(&fs::read(dir.path(old)).unwrap(), MASTER_PASSWORD)
            .unwrap()
            .2,
        1
    );
}

// --- Directory, links and the current vault ---------------------------------------------

/// Directory with permission for group or others: nothing is written nor listed, and the
/// vault is written anyway (the backup never blocks the write).
#[test]
fn attack_directory_with_open_permission_refused() {
    let d = Dir::new("open-dir");
    let mut vault = new_vault(MASTER_PASSWORD);
    let dir = d.backup_dir();
    for m in [0o750, 0o705, 0o755, 0o777, 0o701] {
        fs::set_permissions(d.backups(), fs::Permissions::from_mode(m)).unwrap();
        let g = write_entries_with_backup(&d.vault(), &mut vault, &[], &dir, at(INSTANT)).unwrap();
        assert_eq!(g.backup, Err(BackupError::UnsafeDir), "{m:o}");
        assert_eq!(dir.list(), Err(BackupError::UnsafeDir), "{m:o}");
        assert!(fs::exists(d.vault()).unwrap());
    }
    fs::set_permissions(d.backups(), fs::Permissions::from_mode(0o700)).unwrap();
    assert!(d.names().is_empty(), "{:?}", d.names());
    // Directory that does not exist and path that is a file.
    let missing = BackupDir::new(d.0.join("nao-existe"), uid());
    assert_eq!(missing.list(), Err(BackupError::UnsafeDir));
    let file = BackupDir::new(d.vault(), uid());
    assert_eq!(file.list(), Err(BackupError::UnsafeDir));
    // Another owner: the expected uid does not match.
    let other = BackupDir::new(d.backups(), uid().wrapping_add(1));
    assert_eq!(other.list(), Err(BackupError::UnsafeDir));
}

/// Symbolic link planted with the name of the version (and with that of an old one): the
/// backup neither follows nor overwrites the target, the list ignores the link and the
/// rotation does not delete it.
#[test]
fn attack_link_with_version_name_is_not_followed() {
    let d = Dir::new("version-link");
    let mut vault = new_vault(MASTER_PASSWORD);
    let dir = d.backup_dir();
    let target = d.0.join("alvo");
    fs::write(&target, b"conteudo do alvo").unwrap();
    let base = Version::from_instant(at(INSTANT)).unwrap();
    symlink(&target, dir.path(base)).unwrap();
    let old = Version::from_instant(at(1_000_000_000)).unwrap();
    symlink(&target, dir.path(old)).unwrap();

    let g = write_entries_with_backup(&d.vault(), &mut vault, &[], &dir, at(INSTANT)).unwrap();
    assert_eq!(g.backup, Ok(base.with_suffix(1)));
    assert_eq!(fs::read(&target).unwrap(), b"conteudo do alvo");
    assert_eq!(dir.list().unwrap(), [base.with_suffix(1)]);
    // Fills beyond the 30: the links are still there, and the target, intact.
    for i in 1..=35u64 {
        write_entries_with_backup(&d.vault(), &mut vault, &[], &dir, at(INSTANT + i))
            .unwrap()
            .backup
            .unwrap();
    }
    assert_eq!(dir.list().unwrap().len(), VERSIONS);
    assert!(
        fs::symlink_metadata(dir.path(old))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read(&target).unwrap(), b"conteudo do alvo");
}

/// D17: the current vault only goes to the backup if it was sealed by this open vault.
/// Another vault (another key), a tampered file or arbitrary bytes are refused.
#[test]
fn attack_keep_current_only_accepts_the_authentic_file() {
    let d = Dir::new("current");
    let mut vault = new_vault(MASTER_PASSWORD);
    let dir = d.backup_dir();
    atomic_write::write_entries(&d.vault(), &mut vault, &[]).unwrap();
    let current = fs::read(d.vault()).unwrap();

    let mut other = new_vault(MASTER_PASSWORD);
    let d2 = Dir::new("current-other");
    atomic_write::write_entries(&d2.vault(), &mut other, &[]).unwrap();
    let from_other = fs::read(d2.vault()).unwrap();
    let mut tampered = current.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 1;
    for bad in [&from_other, &tampered, &b"WARDCOFR qualquer".to_vec()] {
        assert_eq!(
            keep_current(&dir, &vault, bad, at(INSTANT)),
            Err(BackupError::NotAuthentic)
        );
    }
    assert!(d.names().is_empty());
    let v = keep_current(&dir, &vault, &current, at(INSTANT)).unwrap();
    assert_eq!(fs::read(dir.path(v)).unwrap(), current);
    assert_eq!(mode(&dir.path(v)), 0o600);
}

// --- Phase 49a: future dates in the name and sealed generation --------------------------

/// Phase 49a (medium finding of M7): 30 files named with a future date planted in the
/// directory (any process of the user can do it) do not make the rotation delete the real
/// versions, nor do the future ones count for the 30. The restore list ignores those
/// beyond now + [`FUTURE_MARGIN`]; the ones inside the margin show, but the sealed
/// generation gives them away.
#[test]
fn attack_versions_with_future_date_do_not_delete_the_real_ones() {
    let d = Dir::new("future");
    let mut vault = new_vault(MASTER_PASSWORD);
    let dir = d.backup_dir();
    let mut real = Vec::new();
    for i in 0..5u64 {
        let g = write_entries_with_backup(&d.vault(), &mut vault, &[], &dir, at(INSTANT + i * 60))
            .unwrap();
        real.push(g.backup.unwrap());
    }
    // An old authentic copy (generation 1) with names from the future: 30 beyond the
    // margin (2099) and 30 inside it (a few seconds after the next write).
    let old = fs::read(dir.path(real[0])).unwrap();
    let mut far = Vec::new();
    let mut near = Vec::new();
    for i in 0..30u64 {
        let v = Version::from_instant(at(4_070_908_800 + i * 3600)).unwrap();
        fs::write(dir.path(v), &old).unwrap();
        far.push(v);
        let v = Version::from_instant(at(INSTANT + 5 * 60 + 1 + i)).unwrap();
        fs::write(dir.path(v), &old).unwrap();
        near.push(v);
    }
    let now = at(INSTANT + 5 * 60);
    let g = write_entries_with_backup(&d.vault(), &mut vault, &[], &dir, now).unwrap();
    real.push(g.backup.unwrap());
    // No real one deleted, no planted one deleted.
    for v in real.iter().chain(&far).chain(&near) {
        assert!(fs::exists(dir.path(*v)).unwrap(), "{v} sumiu");
    }
    // The restore list: without the 2099 ones; the ones in the margin show on top.
    let list = dir.list_until(now).unwrap();
    assert!(far.iter().all(|v| !list.contains(v)));
    assert!(real.iter().all(|v| list.contains(v)));
    assert_eq!(list.len(), real.len() + near.len());
    // The sealed generation gives away the planted ones: generation 1, older than the
    // newest real one.
    let newest = fs::read(dir.path(*real.last().unwrap())).unwrap();
    assert_eq!(sealed_generation(&vault, &newest), Some(6));
    for v in &near {
        let b = fs::read(dir.path(*v)).unwrap();
        assert_eq!(sealed_generation(&vault, &b), Some(1), "{v}");
    }
    // 40 more real writes: the 30 newest real ones stay; the future ones do not count and
    // are still there (and the 2099 ones, never in the list).
    for i in 0..40u64 {
        let g = write_entries_with_backup(
            &d.vault(),
            &mut vault,
            &[],
            &dir,
            at(INSTANT + 3600 + i * 60),
        )
        .unwrap();
        real.push(g.backup.unwrap());
    }
    let until_now: Vec<Version> = dir
        .list()
        .unwrap()
        .into_iter()
        .filter(|v| *v <= *real.last().unwrap())
        .collect();
    assert_eq!(until_now.len(), VERSIONS);
    let mut expected: Vec<Version> = real[real.len() - VERSIONS..].to_vec();
    expected.reverse();
    assert_eq!(until_now, expected);
    for v in &far {
        assert!(fs::exists(dir.path(*v)).unwrap());
    }
}

/// Plants `bytes` in the directory with the name of version `v`, 0600 (any process of the
/// user can do it).
fn plant(dir: &BackupDir, v: Version, bytes: &[u8]) {
    let p = dir.path(v);
    fs::write(&p, bytes).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
}

/// Phase 54c (finding of Phase 54), attack: a process of the user plants in the
/// directory, with dates newer than the real versions (but past, up to the next write),
/// 36 files with a version name: valid header with the body swapped, a version of another
/// vault, copies of the oldest real version and copies of the newest. Before the fix, the
/// next write deleted the real ones (the rotation counted the junk by date). Now no real
/// one leaves; the junk and the other vault never leave; the copies only leave with their
/// generation, when it falls below the 30.
#[test]
fn f54c_attack_junk_with_newer_date_does_not_delete_real_versions() {
    let d = Dir::new("f54c-junk");
    let mut vault = new_vault(MASTER_PASSWORD);
    let dir = d.backup_dir();
    let write = |vault: &mut OpenVault, t: u64| {
        write_entries_with_backup(&d.vault(), vault, &[], &dir, at(t))
            .unwrap()
            .backup
            .unwrap()
    };
    let mut real: Vec<Version> = (0..5u64)
        .map(|i| write(&mut vault, INSTANT + i * 60))
        .collect();
    let old = fs::read(dir.path(real[0])).unwrap();
    let recent = fs::read(dir.path(real[4])).unwrap();
    let mut tampered = recent.clone();
    let n = tampered.len();
    tampered[n - 3] ^= 0x5a;
    let mut other = new_vault(MASTER_PASSWORD);
    let d2 = Dir::new("f54c-junk-other");
    atomic_write::write_entries(&d2.vault(), &mut other, &[]).unwrap();
    let from_other = fs::read(d2.vault()).unwrap();
    let (mut junk, mut copies) = (Vec::new(), Vec::new());
    for i in 0..36u64 {
        // Between the last real one (INSTANT + 240) and the next write (INSTANT + 600).
        let v = Version::from_instant(at(INSTANT + 300 + i)).unwrap();
        match i % 4 {
            0 => {
                plant(&dir, v, &tampered);
                junk.push(v);
            }
            1 => {
                plant(&dir, v, &from_other);
                junk.push(v);
            }
            2 => {
                plant(&dir, v, &old);
                copies.push(v);
            }
            _ => {
                plant(&dir, v, &recent);
                copies.push(v);
            }
        }
    }
    assert!(crate::format::read_header(&tampered).is_ok());
    // The next real write: nothing leaves (6 generations).
    real.push(write(&mut vault, INSTANT + 600));
    for v in real.iter().chain(&junk).chain(&copies) {
        assert!(fs::exists(dir.path(*v)).unwrap(), "{v} sumiu");
    }
    // 29 more real ones (35 generations): the 30 newest real ones stay; the junk and the
    // other vault remain; the copies of generations 1 and 5 leave with them.
    for i in 0..29u64 {
        real.push(write(&mut vault, INSTANT + 3600 + i * 60));
    }
    for v in &real[..5] {
        assert!(!fs::exists(dir.path(*v)).unwrap(), "{v} ficou");
    }
    for v in &real[5..] {
        assert!(fs::exists(dir.path(*v)).unwrap(), "{v} real sumiu");
    }
    for v in &junk {
        assert!(fs::exists(dir.path(*v)).unwrap(), "{v}: lixo apagado");
    }
    for v in &copies {
        assert!(!fs::exists(dir.path(*v)).unwrap(), "{v}: cópia ficou");
    }
    assert_eq!(dir.list().unwrap().len(), VERSIONS + junk.len());
}

/// Phase 54c: the versions from before a master password change (another data key) do
/// not verify with the new key: they do not count for the 30 and the rotation never
/// deletes them (the notice of the "change password" screen tells the user to delete
/// them, if the password leaked).
#[test]
fn f54c_versions_from_before_the_password_change_neither_count_nor_leave() {
    let d = Dir::new("f54c-change");
    let mut vault = new_vault(MASTER_PASSWORD);
    let dir = d.backup_dir();
    let old: Vec<Version> = (0..3u64)
        .map(|i| {
            write_entries_with_backup(&d.vault(), &mut vault, &[], &dir, at(INSTANT + i))
                .unwrap()
                .backup
                .unwrap()
        })
        .collect();
    let (mut new, g) = {
        let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
        change_password_with_backup(
            &d.vault(),
            &password(MASTER_PASSWORD),
            &password("falso-mestra-ward-03"),
            3,
            3,
            &dir,
            at(INSTANT + 60),
        )
        .unwrap()
    };
    let mut newer = vec![g.backup.unwrap()];
    for i in 0..35u64 {
        let g =
            write_entries_with_backup(&d.vault(), &mut new, &[], &dir, at(INSTANT + 120 + i * 60))
                .unwrap();
        newer.push(g.backup.unwrap());
    }
    for v in &old {
        assert!(fs::exists(dir.path(*v)).unwrap(), "{v}: antiga apagada");
    }
    for v in &newer[..newer.len() - VERSIONS] {
        assert!(!fs::exists(dir.path(*v)).unwrap(), "{v} ficou");
    }
    for v in &newer[newer.len() - VERSIONS..] {
        assert!(fs::exists(dir.path(*v)).unwrap(), "{v} sumiu");
    }
}

/// Phase 54c: a clock that goes back. The write with the clock one hour late deletes no
/// version (the ones dated after it do not count: fail closed) and stays; when the clock
/// is right again, the rotation follows the generation, and the late one (generation
/// higher than the ones before) stays among the 30, even with the oldest date in the
/// directory.
#[test]
fn f54c_clock_that_goes_back_does_not_delete_real_version() {
    let d = Dir::new("f54c-clock");
    let mut vault = new_vault(MASTER_PASSWORD);
    let dir = d.backup_dir();
    let mut write = |t: u64| {
        write_entries_with_backup(&d.vault(), &mut vault, &[], &dir, at(t))
            .unwrap()
            .backup
            .unwrap()
    };
    let mut real: Vec<Version> = (0..30u64).map(|i| write(INSTANT + i * 60)).collect();
    let late = write(INSTANT - 3600);
    real.push(late);
    for v in &real {
        assert!(fs::exists(dir.path(*v)).unwrap(), "{v} sumiu");
    }
    // The right clock again: one write (32 generations); generations 1 and 2 leave.
    real.push(write(INSTANT + 7200));
    for v in &real[..2] {
        assert!(!fs::exists(dir.path(*v)).unwrap(), "{v} ficou");
    }
    for v in &real[2..] {
        assert!(fs::exists(dir.path(*v)).unwrap(), "{v} sumiu");
    }
    assert_eq!(dir.list().unwrap().len(), VERSIONS);
}

/// The rotation rule, alone: past versions never leave because of the future ones, the
/// protected ones never leave, and with no future ones it is the rotation from before
/// (the oldest ones). Here the generation follows the date (version `i` has generation
/// `i + 1`).
#[test]
fn rotation_counts_only_up_to_the_ceiling() {
    let v = |s: u64| Version::from_instant(at(INSTANT + s)).unwrap();
    let with_g = |l: &[Version], g0: u64| -> Vec<(Version, Option<u64>)> {
        l.iter()
            .enumerate()
            .map(|(i, x)| (*x, Some(g0 + i as u64)))
            .collect()
    };
    let past: Vec<Version> = (0..35).map(|i| v(i * 10)).collect();
    let ceiling = past[34];
    let future: Vec<Version> = (1..=40).map(|i| v(340 + i)).collect();
    let mut list = with_g(&past, 1);
    list.extend(with_g(&future, 36));
    let out = to_delete_verified(&list, ceiling, &[ceiling]);
    assert_eq!(out, past[..5].to_vec());
    // Only future ones beyond the ceiling: nothing leaves.
    assert!(to_delete_verified(&with_g(&future, 36), ceiling, &[]).is_empty());
    // 31 past ones and the ceiling at the oldest: only it counts, nothing leaves.
    assert!(to_delete_verified(&with_g(&past[..31], 1), past[0], &[past[0]]).is_empty());
    // A protected one among the oldest: skips it and does not delete another in its place
    // (the rotation does not remove a generation that would be among the 30).
    let out = to_delete_verified(&with_g(&past[..31], 1), past[30], &[past[0]]);
    assert!(out.is_empty(), "{out:?}");
    // The ceiling of the second covers the suffixes of the same second.
    let base = v(0);
    assert!(base.with_suffix(99) <= base.end_of_second());
    assert!(v(1) > base.end_of_second());
}

/// Phase 54c, the rule alone: what does not verify (`None`) never counts nor leaves; each
/// generation counts once (copies under new names take no place) and leaves whole when it
/// falls below the 30; the date does not decide the order (the generation does), so a
/// clock that went back does not remove the new version.
#[test]
fn f54c_rotation_only_counts_and_only_deletes_what_verifies() {
    let v = |s: u64| Version::from_instant(at(INSTANT + s)).unwrap();
    // 30 real ones (generations 1..=30), dates 0..290.
    let real: Vec<(Version, Option<u64>)> = (0..30u64).map(|i| (v(i * 10), Some(i + 1))).collect();
    let ceiling = v(10_000);
    // 40 junk files with newer dates: nothing leaves.
    let mut list = real.clone();
    list.extend((0..40u64).map(|i| (v(300 + i), None)));
    assert!(to_delete_verified(&list, ceiling, &[]).is_empty());
    // 40 copies of the newest real one (generation 30) with newer dates: a single
    // generation.
    let mut list = real.clone();
    list.extend((0..40u64).map(|i| (v(400 + i), Some(30))));
    assert!(to_delete_verified(&list, ceiling, &[]).is_empty());
    // 40 copies of the oldest real one with newer dates and a new real one: the whole
    // generation 1 leaves (the original and the copies), no other real one.
    let mut list = real.clone();
    let copies: Vec<Version> = (0..40u64).map(|i| v(500 + i)).collect();
    list.extend(copies.iter().map(|c| (*c, Some(1))));
    let new = v(600);
    list.push((new, Some(31)));
    let mut expected = vec![real[0].0];
    expected.extend(&copies);
    assert_eq!(to_delete_verified(&list, ceiling, &[new]), expected);
    // A clock that went back: the new one (generation 31) with the oldest date of all
    // stays; generation 1 leaves.
    let mut list = real.clone();
    let late = Version::from_instant(at(INSTANT - 3600)).unwrap();
    list.push((late, Some(31)));
    assert_eq!(to_delete_verified(&list, ceiling, &[late]), [real[0].0]);
    // With nothing that verifies: nothing leaves.
    let junk: Vec<(Version, Option<u64>)> = (0..100u64).map(|i| (v(i), None)).collect();
    assert!(to_delete_verified(&junk, ceiling, &[]).is_empty());
    assert!(to_delete_verified(&[], ceiling, &[]).is_empty());
}

/// The out-of-order mark: the old copy (generation 3) under a new name comes out marked;
/// the real ones in order do not; with no generation (another key) it does not count; the
/// suffix breaks the tie within the same second; the order of the list does not matter.
#[test]
fn attack_old_copy_under_new_name_comes_out_marked() {
    let v = |s: u64| Version::from_instant(at(INSTANT + s)).unwrap();
    let list = [
        (v(3600), Some(3)),
        (v(60).with_suffix(1), Some(12)),
        (v(60), Some(11)),
        (v(30), None),
        (v(0), Some(10)),
        (v(1), Some(4)),
    ];
    assert_eq!(
        out_of_order(&list),
        [true, false, false, false, false, true]
    );
    let same = [(v(0), Some(12)), (v(0).with_suffix(1), Some(11))];
    assert_eq!(out_of_order(&same), [false, true]);
    assert!(out_of_order(&[]).is_empty());
}

/// The sealed generation only comes out of a file of the key of the open vault: another
/// vault (another password), a changed byte or arbitrary bytes give `None`.
#[test]
fn attack_sealed_generation_cannot_be_forged() {
    let d = Dir::new("generation");
    let mut vault = new_vault(MASTER_PASSWORD);
    for _ in 0..3 {
        atomic_write::write_entries(&d.vault(), &mut vault, entries()).unwrap();
    }
    let good = fs::read(d.vault()).unwrap();
    assert_eq!(sealed_generation(&vault, &good), Some(3));
    let mut other = new_vault(MASTER_PASSWORD);
    let d2 = Dir::new("generation-other");
    atomic_write::write_entries(&d2.vault(), &mut other, &[]).unwrap();
    assert_eq!(
        sealed_generation(&vault, &fs::read(d2.vault()).unwrap()),
        None
    );
    for i in [0, 100, 159, 160, good.len() - 1] {
        let mut bad = good.clone();
        bad[i] ^= 1;
        assert_eq!(sealed_generation(&vault, &bad), None, "byte {i}");
    }
    assert_eq!(sealed_generation(&vault, b"WARDCOFR qualquer"), None);
}

// --- V7.3: SIGKILL during the backup write ----------------------------------------------

/// Points of the backup write, in order; the first four come before the link.
const POINTS: [Point; 5] = [
    Point::AfterCreate,
    Point::AfterWrite,
    Point::AfterFsync,
    Point::AfterRename,
    Point::AfterFsyncDir,
];
const BEFORE_LINK: usize = 3;

/// Child process of the death tests. Does nothing without the mode in the environment.
/// Without the open vault (an Argon2 per child would be too slow), the generation of each
/// version comes from the `v0..` table: version `k` has generation `k + 1` (the parent
/// sealed them in order, with the same vault, [`sealed_versions`]).
#[test]
#[ignore = "só roda como processo filho dos testes de backup"]
fn child_backup() {
    let Ok(mode) = std::env::var(ENV_MODE) else {
        return;
    };
    let dir = PathBuf::from(std::env::var_os(ENV_DIR).unwrap());
    let base: u64 = std::env::var(ENV_BASE).unwrap().parse().unwrap();
    let backup_dir = BackupDir::new(dir.join("backups"), uid());
    let versions: Vec<Vec<u8>> = (0..)
        .map_while(|k| fs::read(dir.join(format!("v{k}"))).ok())
        .collect();
    let generation = |b: &[u8]| {
        versions
            .iter()
            .position(|v| v.as_slice() == b)
            .map(|i| i as u64 + 1)
    };
    if let Some(i) = mode.strip_prefix("ponto:") {
        let i: usize = i.parse().unwrap();
        let _ = backup_dir.keep_with_points(&versions[1], at(base), &generation, &mut |p| {
            if p == POINTS[i] {
                println!("\nWARD-PONTO");
                std::thread::sleep(CHILD_LIFETIME);
                std::process::exit(3);
            }
            Ok(())
        });
        println!("\nWARD-FIM");
    } else if mode == "laco" {
        // `base` is the first version to write; each one with the date of its number, so
        // the date follows the generation, as in the real write.
        println!("\nWARD-PRONTO");
        let start = Instant::now();
        for k in base as usize.. {
            let Some(v) = versions.get(k) else {
                println!("\nWARD-ACABOU");
                std::process::exit(4);
            };
            backup_dir
                .keep_with_points(v, at(INSTANT + k as u64), &generation, &mut |_| Ok(()))
                .unwrap();
            if start.elapsed() > CHILD_LIFETIME {
                std::process::exit(3);
            }
        }
    } else {
        panic!("modo desconhecido");
    }
}

fn child(mode: &str, dir: &Path, base: u64) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            CHILD_NAME,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(ENV_MODE, mode)
        .env(ENV_DIR, dir)
        .env(ENV_BASE, base.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

fn wait_line(f: &mut Child, expected: &'static str) {
    let output = f.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for l in BufReader::new(output).lines() {
            let Ok(l) = l else { break };
            if l.trim_end() == expected || l.ends_with(expected) {
                let _ = tx.send(());
            }
        }
    });
    if rx.recv_timeout(Duration::from_secs(60)).is_err() {
        let _ = f.kill();
        let st = f.wait();
        panic!("o filho não chegou a {expected}: {st:?}");
    }
}

fn kill(mut f: Child) {
    f.kill().unwrap(); // SIGKILL
    let st = f.wait().unwrap();
    assert!(!st.success(), "o filho tinha de morrer pelo SIGKILL");
}

/// Three sealed versions (with the fake secrets of the test vault) and the vault that
/// sealed them, written to `v0..v2` in the test directory.
fn three_versions(d: &Dir) -> (OpenVault, Vec<Vec<u8>>) {
    sealed_versions(d, 3)
}

/// `n` versions sealed in order by a new vault (version `k` with generation `k + 1`), in
/// `v0..` in the test directory. Seals directly, without writing the vault (no fsync per
/// version).
fn sealed_versions(d: &Dir, n: usize) -> (OpenVault, Vec<Vec<u8>>) {
    let mut vault = new_vault(MASTER_PASSWORD);
    let mut versions = Vec::new();
    for k in 0..n {
        let b = seal_next(&mut vault, entries());
        fs::write(d.0.join(format!("v{k}")), &b).unwrap();
        versions.push(b);
    }
    (vault, versions)
}

/// Seals the next generation of `vault` with `entries`, as the write does, without
/// writing.
fn seal_next(vault: &mut OpenVault, entries: &[Entry]) -> Vec<u8> {
    let g = vault.next_generation().unwrap();
    let content = body::serialize(vault.config(), g, entries).unwrap();
    let b = vault.seal(&content).unwrap();
    vault.burn(g);
    b
}

/// Checks the directory after a death: every file with a version name is one of the whole
/// versions (it opens), 0600; the temp file left behind is a prefix of the ciphertext.
fn check_dir(d: &Dir, versions: &[Vec<u8>]) -> (usize, usize) {
    let dir = d.backup_dir();
    let list = dir.list().unwrap();
    for v in &list {
        let p = dir.path(*v);
        let b = fs::read(&p).unwrap();
        assert!(
            versions.contains(&b),
            "{v}: o backup não é uma versão inteira"
        );
        assert_eq!(mode(&p), 0o600, "{v}");
    }
    let temps = d.temp_files();
    assert!(temps.len() <= 1, "{temps:?}");
    for t in &temps {
        let b = fs::read(t).unwrap();
        assert!(
            versions.iter().any(|v| v.starts_with(&b)),
            "temporário não é prefixo"
        );
        for s in SECRETS {
            assert!(!contains(&b, s.as_bytes()), "temporário em claro");
        }
        assert_eq!(mode(t), 0o600);
    }
    (list.len(), temps.len())
}

/// V7.3 (S7): SIGKILL at each point of the backup write. Before the link, no new name
/// shows up and the backups from before stay the same; after it, the new one is whole.
/// The next write deletes the temp file left behind.
#[test]
fn v7_3_sigkill_at_each_backup_point() {
    for (i, target) in POINTS.iter().enumerate() {
        let d = Dir::new("v7-3-point");
        let (vault, versions) = three_versions(&d);
        let dir = d.backup_dir();
        let before = dir.keep(&versions[0], at(INSTANT), &vault).unwrap();
        let mut f = child(&format!("ponto:{i}"), &d.0, INSTANT + 60);
        wait_line(&mut f, "WARD-PONTO");
        kill(f);
        let (n, temps) = check_dir(&d, &versions);
        assert_eq!(
            fs::read(dir.path(before)).unwrap(),
            versions[0],
            "{target:?}"
        );
        if i < BEFORE_LINK {
            assert_eq!(n, 1, "{target:?}");
            assert_eq!(temps, 1, "{target:?}");
        } else {
            assert_eq!(n, 2, "{target:?}");
            assert_eq!(temps, 0, "{target:?}");
        }
        dir.keep(&versions[2], at(INSTANT + 120), &vault).unwrap();
        assert!(d.temp_files().is_empty(), "{target:?}: o temporário ficou");
    }
}

/// V7.3 (S7): SIGKILL at 100 random points of a child that writes backups nonstop (with
/// the rotation of the 30): every backup that stayed is a whole version and opens; no
/// backup disappears beyond the rotation.
#[test]
fn v7_3_sigkill_at_100_random_points() {
    const DEATHS: u64 = 100;
    /// Distinct versions for the children to write (each child writes a few before
    /// dying; what is left is not used).
    const TABLE: usize = 5000;
    let d = Dir::new("v7-3-random");
    let (mut vault, versions) = sealed_versions(&d, TABLE);
    let dir = d.backup_dir();
    // Duration of one backup here (with the rotation reading the full directory), to
    // spread the deaths over several writes.
    for (k, v) in versions.iter().enumerate().take(VERSIONS) {
        dir.keep(v, at(INSTANT + k as u64), &vault).unwrap();
    }
    let t0 = Instant::now();
    for (k, v) in versions.iter().enumerate().skip(VERSIONS).take(3) {
        dir.keep(v, at(INSTANT + k as u64), &vault).unwrap();
    }
    let one = (t0.elapsed() / 3).max(Duration::from_millis(1));
    let window = (one * 4).as_micros() as u64;

    let mut with_temp = 0;
    let mut before = dir.list().unwrap().len();
    // The next version to write: after the highest one in the directory.
    let next = |p: &BackupDir| {
        p.list()
            .unwrap()
            .iter()
            .filter_map(|v| {
                let b = fs::read(p.path(*v)).ok()?;
                versions.iter().position(|x| *x == b)
            })
            .max()
            .unwrap()
            + 1
    };
    for m in 0..DEATHS {
        let start = next(&dir);
        assert!(start < TABLE, "a tabela acabou na morte {m}");
        let mut f = child("laco", &d.0, start as u64);
        wait_line(&mut f, "WARD-PRONTO");
        std::thread::sleep(Duration::from_micros(random_u64() % window));
        kill(f);
        let (n, temps) = check_dir(&d, &versions);
        // A version only leaves after the new one went in (link before the rotation).
        assert!(n >= before.min(VERSIONS), "morte {m}: {n} < {before}");
        // Killed between the link and the rotation, the directory has one more, which the
        // next whole write removes.
        assert!(n <= before.max(VERSIONS) + 1, "morte {m}: {n}");
        before = n;
        with_temp += temps;
    }
    eprintln!(
        "v7_3: {DEATHS} mortes, {with_temp} no meio de um backup, backup ~{one:?}, {} versões",
        next(&dir)
    );
    assert!(
        with_temp >= (DEATHS / 10) as usize,
        "poucas mortes no meio da gravação"
    );
    // All the ones that stayed open: the bytes are from one of the versions of the table,
    // and each one opens with the password.
    for v in &versions {
        assert!(vault.authenticates(v));
    }
    let (_, read, _) = open(&versions[2], MASTER_PASSWORD).unwrap();
    assert_eq!(read.len(), entries().len());
    // The next write (new generation) cleans the temp file and goes back to the 30.
    let new = seal_next(&mut vault, entries());
    dir.keep(&new, at(INSTANT + 999_999), &vault).unwrap();
    assert!(d.temp_files().is_empty());
    assert_eq!(dir.list().unwrap().len(), VERSIONS);
}
