//! m2:DT5 and m2:DT6 in the daemon (Phase 49): the first unlock of a 0.1.x vault (body
//! v4) writes it as v5 through the normal write (generation + 1, state, local backup),
//! and the state file v1 (0.1.x) is read and written again as v2 (`ward-state 2`).
//!
//! - m2:V4.3: a write that fails keeps the v4 byte for byte and the unlock answers the
//!   error; SIGKILL at each point of the daemon's migration (child process, with the
//!   `test-hooks` hook) leaves the v4 or the whole v5, and the next unlock works. The
//!   points inside the atomic write are the core's
//!   (`ward_core::atomic_write::tests::dt5_sigkill_at_each_point_v4_to_v5`).
//! - m2:V4.5: same configuration and entries, byte for byte in the values.
//! - m2:V4.6: the v5 is not written again.
//! - m2:V4.7: the v4 copy put back after the migration is a rollback.
//! - m2:V4.10: a v4 version of the backup restores and is written as v5; the local and
//!   USB drive rotations count v4 and v5 by the sealed generation.
//! - m2:V4.11: the state v1 is read with the same values and becomes the state v2.
//!
//! Only the 0.1.4 fixtures of `dev/fixtures/legacy/` (fake values, test password and PIN
//! of D15), copied into a directory of each test in `target/tmp`. Never the real vault
//! nor the one in `.dev/`.

mod common;

use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use common::*;
use ward_core::backup::VERSIONS;
use ward_core::body::{self, BODY_VERSION, BodyRead, LEGACY_BODY_VERSION};
use ward_core::entry::Entry;
use ward_core::format::{MasterPassword, OpenVault};
use ward_core::protocol::{BackupVersion, Response, ResponseError, Role};
use wardd::core::{Daemon, Paths};
use wardd::external::{Seen, UsbDriveSource};
use wardd::files::{self, Generations};

/// The 0.1.4 vault: body v4, generation 3.
const VAULT_014: &[u8] = include_bytes!("../../../dev/fixtures/legacy/vault-0.1.4.ward");
/// The 0.1.4 state: v1, seen 3, burned 3, no agent failure.
const STATE_014: &[u8] = include_bytes!("../../../dev/fixtures/legacy/state-0.1.4");
/// The two versions of the 0.1.4 local backup (body v4, same key as the vault).
const BACKUP_014: [&[u8]; 2] = [
    include_bytes!("../../../dev/fixtures/legacy/backup-0.1.4/ward-20261110-213053.ward"),
    include_bytes!("../../../dev/fixtures/legacy/backup-0.1.4/ward-20261110-213054.ward"),
];
/// Names in the past for the 0.1.4 versions (the fixture names are from the day the
/// baseline was made; a name after now + 5 min would leave the list, Phase 49a).
const BACKUP_014_NAMES: [&str; 2] = ["ward-20260101-000001.ward", "ward-20260101-000002.ward"];
const GENERATION_014: u64 = 3;

fn put(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

/// Swaps the file at `path` the way another process of the user would: a new file
/// renamed over it (another inode).
fn swap(path: &Path, bytes: &[u8]) {
    let tmp = path.with_extension("troca");
    put(&tmp, bytes);
    fs::rename(&tmp, path).unwrap();
}

/// The vault and the state as the 0.1.4 left them.
fn install_014(c: &Paths) {
    put(&c.vault, VAULT_014);
    put(&c.state, STATE_014);
}

/// The 0.1.4 backup versions, under past names, in `<vault dir>/backups` (0700).
fn install_014_backup(c: &Paths) -> PathBuf {
    let dir = c.vault.parent().unwrap().join("backups");
    fs::create_dir_all(&dir).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
    for (bytes, name) in BACKUP_014.iter().zip(BACKUP_014_NAMES) {
        put(&dir.join(name), bytes);
    }
    dir
}

fn open_bytes(bytes: &[u8]) -> (OpenVault, Vec<Entry>, BodyRead) {
    let password = MasterPassword::new(PASSWORD.to_owned());
    body::open_entries_with_version(bytes, &password, 0, 0).unwrap()
}

/// Configuration and entries of a vault file as the canonical v5 body at a fixed
/// generation: equal bytes = same configuration (PIN hash, lock time, external
/// backup) and the same entries (names, groups, fields, extras, attachments, TOTP seeds,
/// dates, levels), byte for byte in the values.
fn canonical(bytes: &[u8]) -> Vec<u8> {
    let (vault, entries, _) = open_bytes(bytes);
    body::serialize(vault.config(), 1, &entries)
        .unwrap()
        .to_vec()
}

fn identity(p: &Path) -> (u64, u64, i64, i64, u64) {
    let m = fs::symlink_metadata(p).unwrap();
    (m.dev(), m.ino(), m.mtime(), m.mtime_nsec(), m.len())
}

fn mode(p: &Path) -> u32 {
    fs::symlink_metadata(p).unwrap().mode() & 0o7777
}

fn state_text(c: &Paths) -> String {
    fs::read_to_string(&c.state).unwrap()
}

fn v2_text(seen: u64, burned: u64) -> String {
    format!("ward-state 2\nseen {seen}\nburned {burned}\nagent_failures 0\n")
}

/// Names in the folder, sorted.
fn names_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    v.sort();
    v
}

/// The backup versions in the folder (valid names only), sorted by name.
fn version_names(dir: &Path) -> Vec<String> {
    names_in(dir)
        .into_iter()
        .filter(|n| ward_core::backup::valid_name(n))
        .collect()
}

fn list_names(d: &Daemon) -> Vec<String> {
    match request(d, Role::App, "list", "{}") {
        Response::List(l) => l.into_iter().map(|i| i.name).collect(),
        other => panic!("{other:?}"),
    }
}

fn versions(d: &Daemon) -> Vec<BackupVersion> {
    match request(d, Role::App, "backups", "{}") {
        Response::Backups(l) => l,
        other => panic!("{other:?}"),
    }
}

fn lock_timeout(d: &Daemon, minutes: u32) -> Response {
    request(
        d,
        Role::App,
        "lock_timeout",
        &format!(r#"{{"minutes":{minutes}}}"#),
    )
}

fn restore(d: &Daemon, version: &str) -> Response {
    request(
        d,
        Role::App,
        "restore_backup",
        &format!(
            r#"{{"version":"{version}","password":"{}","pin":"{}"}}"#,
            b64(PASSWORD.as_bytes()),
            b64(PIN.as_bytes())
        ),
    )
}

fn is_root() -> bool {
    ward_platform::effective_uid() == 0
}

/// m2:V4.3, m2:V4.5, m2:V4.11 (S7): the first unlock of the 0.1.4 vault answers "ok"
/// with the v5 already on disk: generation + 1, the same configuration and entries, PIN
/// that still checks, the state v2 with the new generation, the v5 in the local backup,
/// everything 0600 and no temp file left.
#[test]
fn v4_3_first_unlock_writes_the_v5_with_the_same_data() {
    let dir = Dir::new("dt5-migra");
    let c = dir.paths();
    install_014(&c);
    let (_, entries_014, read_014) = open_bytes(VAULT_014);
    assert_eq!(read_014.version, LEGACY_BODY_VERSION);
    assert_eq!(read_014.generation, GENERATION_014);
    let names_014: Vec<String> = entries_014.iter().map(|e| e.name().to_owned()).collect();
    let before = canonical(VAULT_014);

    let (d, _) = daemon_at(c.clone());
    let r = unlock(&d, PASSWORD);
    assert!(ok(&r), "{r:?}");
    assert!(!d.is_locked());

    let on_disk = fs::read(&c.vault).unwrap();
    assert!(on_disk.starts_with(b"WARDCOFR"));
    let (vault, _, read) = open_bytes(&on_disk);
    assert_eq!(read.version, BODY_VERSION);
    assert!(!read.is_legacy());
    assert_eq!(read.generation, GENERATION_014 + 1);
    assert!(canonical(&on_disk) == before, "a migração mudou algum dado");
    assert!(
        vault.config().verify_pin(PIN.as_bytes()),
        "o PIN não confere"
    );
    assert_eq!(vault.config().lock_minutes(), 240);
    assert!(!vault.config().backup().enabled());
    assert_eq!(list_names(&d), names_014);

    assert_eq!(state_text(&c), v2_text(4, 4));
    assert_eq!(mode(&c.vault), 0o600);
    assert_eq!(mode(&c.state), 0o600);
    // The v5 went to the local backup (the same sealed bytes).
    let backups = dir.0.join("backups");
    let names = version_names(&backups);
    assert_eq!(names.len(), 1, "{names:?}");
    assert!(fs::read(backups.join(&names[0])).unwrap() == on_disk);
    assert_eq!(mode(&backups.join(&names[0])), 0o600);
    // Nothing else in the folder: no temp file of the vault or of the state.
    assert_eq!(
        names_in(&dir.0),
        ["backups", "test-vault.state", "test-vault.ward"]
    );
    // Neither the password, the PIN nor the type and level names in clear (m2:V4.4).
    for f in [c.vault.clone(), c.state.clone(), backups.join(&names[0])] {
        let b = fs::read(&f).unwrap();
        for s in [
            PASSWORD, PIN, "banco", "servidor", "livre", "database", "server",
        ] {
            assert!(!contains(&b, s.as_bytes()), "{s} em {}", f.display());
        }
        for n in &names_014 {
            assert!(!contains(&b, n.as_bytes()), "{n} em {}", f.display());
        }
    }
}

/// m2:V4.6: the migrated vault, locked and unlocked three times (and by a new daemon),
/// is not written again: same identity, bytes, generation, state and backups.
#[test]
fn v4_6_migrated_vault_is_not_written_again() {
    let dir = Dir::new("dt5-uma-vez");
    let c = dir.paths();
    install_014(&c);
    let (d, _) = daemon_at(c.clone());
    assert!(ok(&unlock(&d, PASSWORD)));
    let backups = dir.0.join("backups");
    let id = identity(&c.vault);
    let bytes = fs::read(&c.vault).unwrap();
    let state = state_text(&c);
    let listed = names_in(&backups);
    let check = |label: &str| {
        assert_eq!(identity(&c.vault), id, "{label}");
        assert!(fs::read(&c.vault).unwrap() == bytes, "{label}");
        assert_eq!(state_text(&c), state, "{label}");
        assert_eq!(names_in(&backups), listed, "{label}");
    };
    for k in 0..3 {
        d.lock();
        assert!(ok(&unlock(&d, PASSWORD)), "rodada {k}");
        check(&format!("rodada {k}"));
    }
    drop(d);
    let (d2, _) = daemon_at(c.clone());
    assert!(ok(&unlock(&d2, PASSWORD)));
    check("daemon novo");
    let (_, _, read) = open_bytes(&bytes);
    assert_eq!(read.version, BODY_VERSION);
    assert_eq!(read.generation, GENERATION_014 + 1);
}

/// m2:V4.7 (attack, D39/D53): after the migration (generation 4), the v4 copy
/// (generation 3) put back in place. With the daemon unlocked, the next write answers
/// "vault changed", locks and does not write over it; the unlock (same daemon or a new
/// one, as with the daemon stopped) answers rollback. The v4 is never migrated again
/// nor written over, and the state keeps generation 4.
#[test]
fn v4_7_attack_v4_copy_after_the_migration() {
    let dir = Dir::new("dt5-retrocesso");
    let c = dir.paths();
    install_014(&c);
    let (d, _) = daemon_at(c.clone());
    assert!(ok(&unlock(&d, PASSWORD)));
    assert_eq!(state_text(&c), v2_text(4, 4));

    swap(&c.vault, VAULT_014);
    assert_eq!(
        error(&lock_timeout(&d, 30)),
        Some(ResponseError::VaultChanged)
    );
    assert!(d.is_locked());
    assert!(fs::read(&c.vault).unwrap() == VAULT_014, "gravou por cima");
    assert_eq!(error(&unlock(&d, PASSWORD)), Some(ResponseError::Rollback));
    drop(d);
    let (d2, _) = daemon_at(c.clone());
    assert_eq!(error(&unlock(&d2, PASSWORD)), Some(ResponseError::Rollback));
    assert!(d2.is_locked());
    assert!(fs::read(&c.vault).unwrap() == VAULT_014);
    assert_eq!(
        files::read_state(&c.state).unwrap(),
        Generations { seen: 4, burned: 4 }
    );
}

/// m2:V4.1 in the daemon: the wrong password on the v4 migrates nothing. Through the app
/// nothing is written at all (vault, state and folder the same); through the agent only
/// the failure goes to the state (D62), already as the state v2, and the vault stays.
#[test]
fn v4_1_wrong_password_on_the_v4_writes_no_vault() {
    let dir = Dir::new("dt5-errada");
    let c = dir.paths();
    install_014(&c);
    let id = identity(&c.vault);
    let listed = names_in(&dir.0);
    let (d, _) = daemon_at(c.clone());
    let wrong = format!(r#"{{"password":"{}"}}"#, b64(b"falso-errada-ward-02"));
    let r = request(&d, Role::App, "unlock", &wrong);
    assert_eq!(error(&r), Some(ResponseError::WrongPassword));
    assert_eq!(identity(&c.vault), id);
    assert!(fs::read(&c.state).unwrap() == STATE_014);
    assert_eq!(names_in(&dir.0), listed);
    let r = request(&d, Role::Agent, "unlock", &wrong);
    assert_eq!(error(&r), Some(ResponseError::WrongPassword));
    assert_eq!(identity(&c.vault), id);
    assert_eq!(
        state_text(&c),
        "ward-state 2\nseen 3\nburned 3\nagent_failures 1\n"
    );
    assert_eq!(names_in(&dir.0), listed);
}

/// m2:V4.3 (S7, D53): the vault write of the migration fails (the vault folder without
/// write permission; the state is in another one). The unlock answers the error and
/// stays locked, the v4 is intact byte for byte, nothing is deleted, and the state has
/// only the burned generation. Fixed, the next unlock migrates with a generation never
/// sealed before (5), and only once.
#[test]
fn v4_3_write_failure_keeps_the_v4_and_answers_the_error() {
    if is_root() {
        eprintln!("pulado: o root ignora as permissões");
        return;
    }
    let dir = Dir::new("dt5-falha");
    let vault_dir = dir.0.join("cofre");
    let state_dir = dir.0.join("estado");
    for p in [&vault_dir, &state_dir] {
        fs::create_dir(p).unwrap();
        fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let c = Paths {
        vault: vault_dir.join("cofre.ward"),
        state: state_dir.join("estado"),
    };
    install_014(&c);
    let id = identity(&c.vault);
    fs::set_permissions(&vault_dir, fs::Permissions::from_mode(0o500)).unwrap();
    let (d, _) = daemon_at(c.clone());
    let r = unlock(&d, PASSWORD);
    assert_eq!(error(&r), Some(ResponseError::WriteFailed));
    assert!(d.is_locked());
    assert_eq!(identity(&c.vault), id);
    assert!(fs::read(&c.vault).unwrap() == VAULT_014);
    assert_eq!(names_in(&vault_dir), ["cofre.ward"]);
    // D53: the generation 4 was burned before the write; seen stays 3.
    assert_eq!(state_text(&c), v2_text(3, 4));
    // The list answers locked: nothing of the v4 stayed open.
    assert_eq!(
        error(&request(&d, Role::App, "list", "{}")),
        Some(ResponseError::Locked)
    );

    fs::set_permissions(&vault_dir, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(ok(&unlock(&d, PASSWORD)));
    let (_, _, read) = open_bytes(&fs::read(&c.vault).unwrap());
    assert_eq!(read.version, BODY_VERSION);
    assert_eq!(read.generation, 5, "a geração queimada 4 não pode voltar");
    assert_eq!(state_text(&c), v2_text(5, 5));
    assert!(canonical(&fs::read(&c.vault).unwrap()) == canonical(VAULT_014));
    // Once: lock and unlock again write nothing.
    let id = identity(&c.vault);
    d.lock();
    assert!(ok(&unlock(&d, PASSWORD)));
    assert_eq!(identity(&c.vault), id);
}

/// m2:V4.3 (S7, D53): the state write that burns the generation fails (the state folder
/// without write permission). The unlock answers the error, the vault is the v4 intact
/// and the state the 0.1.4 one, byte for byte. Fixed, the next unlock migrates.
#[test]
fn v4_3_state_failure_keeps_the_v4_and_the_old_state() {
    if is_root() {
        eprintln!("pulado: o root ignora as permissões");
        return;
    }
    let dir = Dir::new("dt5-falha-estado");
    let vault_dir = dir.0.join("cofre");
    let state_dir = dir.0.join("estado");
    for p in [&vault_dir, &state_dir] {
        fs::create_dir(p).unwrap();
        fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let c = Paths {
        vault: vault_dir.join("cofre.ward"),
        state: state_dir.join("estado"),
    };
    install_014(&c);
    let id = identity(&c.vault);
    fs::set_permissions(&state_dir, fs::Permissions::from_mode(0o500)).unwrap();
    let (d, _) = daemon_at(c.clone());
    assert_eq!(
        error(&unlock(&d, PASSWORD)),
        Some(ResponseError::WriteFailed)
    );
    assert!(d.is_locked());
    assert_eq!(identity(&c.vault), id);
    assert!(fs::read(&c.vault).unwrap() == VAULT_014);
    assert!(fs::read(&c.state).unwrap() == STATE_014);
    assert_eq!(names_in(&vault_dir), ["cofre.ward"]);
    fs::set_permissions(&state_dir, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(ok(&unlock(&d, PASSWORD)));
    let (_, _, read) = open_bytes(&fs::read(&c.vault).unwrap());
    assert_eq!(read.version, BODY_VERSION);
    assert_eq!(read.generation, GENERATION_014 + 1);
    assert_eq!(state_text(&c), v2_text(4, 4));
}

/// m2:V4.11: the daemon starts with a state v1 with agent failures (D62): the wait comes
/// from it, and the right unlock writes the state v2 with the same generations raised by
/// the migration and the failures at 0. A state with a mixed format is refused as
/// invalid (rollback, D39) without migrating.
#[test]
fn v4_11_old_state_with_failures_and_mixed_state() {
    let dir = Dir::new("dt6-estado");
    let c = dir.paths();
    install_014(&c);
    put(
        &c.state,
        b"ward-estado 1\nvista 3\nqueimada 3\nfalhas_agente 2\n",
    );
    let (d, clock) = daemon_at(c.clone());
    assert!(ok(&unlock(&d, PASSWORD)));
    // The first attempt after startup waits what is left of the 2 failures (D62).
    assert!(
        clock
            .0
            .lock()
            .unwrap()
            .iter()
            .all(|w| *w <= wardd::core::unlock_wait(2))
    );
    assert_eq!(state_text(&c), v2_text(4, 4));

    let dir2 = Dir::new("dt6-misturado");
    let c2 = dir2.paths();
    install_014(&c2);
    put(
        &c2.state,
        b"ward-state 2\nvista 3\nqueimada 3\nagent_failures 0\n",
    );
    let (d2, _) = daemon_at(c2.clone());
    assert_eq!(error(&unlock(&d2, PASSWORD)), Some(ResponseError::Rollback));
    assert!(fs::read(&c2.vault).unwrap() == VAULT_014);
}

/// m2:V4.10 (S7, D17, D39): a v4 version of the 0.1.4 backup, restored after the
/// migration, opens with its data and is sealed again as v5 with a new generation (the
/// highest burned + 1); the vault before it stays in the backup.
#[test]
fn v4_10_restoring_a_v4_version_writes_the_v5() {
    let dir = Dir::new("dt5-restaurar");
    let c = dir.paths();
    install_014(&c);
    let backups = install_014_backup(&c);
    let (d, _) = daemon_at(c.clone());
    assert!(ok(&unlock(&d, PASSWORD)));
    // The two v4 versions count and check with the key of the open vault, in order.
    let listed = versions(&d);
    assert_eq!(listed.len(), 3, "{listed:?}");
    assert!(
        listed.iter().all(|v| v.verified && !v.out_of_order),
        "{listed:?}"
    );
    let migrated = fs::read(&c.vault).unwrap();

    let target = BACKUP_014_NAMES[0];
    let (_, version_entries, version_read) = open_bytes(BACKUP_014[0]);
    assert_eq!(version_read.version, LEGACY_BODY_VERSION);
    let r = restore(&d, target);
    assert!(ok(&r), "{r:?}");
    let on_disk = fs::read(&c.vault).unwrap();
    let (restored, entries, read) = open_bytes(&on_disk);
    assert_eq!(read.version, BODY_VERSION);
    assert_eq!(read.generation, GENERATION_014 + 2);
    // The entries are the version's, byte for byte (the same configuration serializes
    // both; lock time and external backup stay the current ones, Phase 49a).
    let config = restored.config();
    assert!(
        body::serialize(config, 1, &entries).unwrap()
            == body::serialize(config, 1, &version_entries).unwrap(),
        "a versão restaurada mudou"
    );
    assert!(config.verify_pin(PIN.as_bytes()));
    assert_eq!(
        list_names(&d),
        entries
            .iter()
            .map(|e| e.name().to_owned())
            .collect::<Vec<_>>()
    );
    assert_eq!(state_text(&c), v2_text(5, 5));
    // The migrated vault was kept, the v4 versions are still there, untouched.
    let files: Vec<Vec<u8>> = version_names(&backups)
        .iter()
        .map(|n| fs::read(backups.join(n)).unwrap())
        .collect();
    assert!(
        files.contains(&migrated),
        "o cofre migrado não ficou no backup"
    );
    assert!(
        files.contains(&on_disk),
        "a versão nova não foi para o backup"
    );
    for (bytes, name) in BACKUP_014.iter().zip(BACKUP_014_NAMES) {
        assert!(fs::read(backups.join(name)).unwrap() == *bytes, "{name}");
    }
}

/// m2:V4.10 (rotation, Phase 49a/54c): after the migration, the local rotation counts
/// the v4 and v5 versions by the sealed generation. Junk with a version name and the
/// copy of a v4 under a new name take no place; when the 30 highest generations are
/// v5, the v4 ones (and their copy) leave, and the junk stays (only the user deletes it).
#[test]
fn v4_10_local_rotation_counts_v4_and_v5() {
    let dir = Dir::new("dt5-rotacao");
    let c = dir.paths();
    install_014(&c);
    let backups = install_014_backup(&c);
    let junk = "ward-20260101-000003.ward";
    let mut b = VAULT_014[..ward_core::format::HEADER_SIZE].to_vec();
    b.extend_from_slice(&[0x5a; 256]);
    put(&backups.join(junk), &b);
    let copy = "ward-20260102-000000.ward";
    put(&backups.join(copy), BACKUP_014[0]);
    let (d, _) = daemon_at(c.clone());
    assert!(ok(&unlock(&d, PASSWORD)));
    let listed = versions(&d);
    let by_name = |n: &str| listed.iter().find(|v| v.name == n).cloned().unwrap();
    assert!(!by_name(junk).verified);
    assert!(by_name(copy).verified && by_name(copy).out_of_order);
    assert!(by_name(BACKUP_014_NAMES[1]).verified);

    // Enough writes for the v5 versions to fill the 30 generations.
    let mut minutes = 30;
    while versions(&d)
        .iter()
        .filter(|v| v.verified && !v.out_of_order)
        .count()
        < VERSIONS
        || backups.join(BACKUP_014_NAMES[1]).exists()
    {
        minutes += 1;
        assert!(
            minutes < 30 + 2 * VERSIONS as u32,
            "a rotação não tirou a v4"
        );
        let r = lock_timeout(&d, minutes);
        assert!(ok(&r), "{r:?}");
    }
    let left = version_names(&backups);
    assert!(left.contains(&junk.to_owned()), "o lixo só o usuário apaga");
    for n in BACKUP_014_NAMES.iter().chain([&copy]) {
        assert!(!left.contains(&(*n).to_owned()), "{n} ficou");
    }
    let real: Vec<&String> = left.iter().filter(|n| n.as_str() != junk).collect();
    assert_eq!(real.len(), VERSIONS, "{left:?}");
    for n in real {
        let (_, _, read) = open_bytes(&fs::read(backups.join(n)).unwrap());
        assert_eq!(read.version, BODY_VERSION, "{n}");
    }
}

/// Source of one USB drive mounted at a directory (the device is the directory's).
struct OneDrive(Seen);

impl UsbDriveSource for OneDrive {
    fn usb_drives(&self) -> Option<Vec<Seen>> {
        Some(vec![self.0.clone()])
    }
}

/// m2:V4.10 (USB drive): the copy to the USB drive takes the v4 versions of the 0.1.4
/// backup and the v5 ones of the migration (all check with the key of the open vault);
/// the USB drive rotation also counts by the sealed generation, and the v4 ones leave
/// when the 30 highest are v5.
#[test]
fn v4_10_usb_drive_rotation_counts_v4_and_v5() {
    const UUID: &str = "ABCD-1234";
    let dir = Dir::new("dt5-pendrive");
    let c = dir.paths();
    install_014(&c);
    install_014_backup(&c);
    let mount = dir.0.join("pendrive");
    fs::create_dir(&mount).unwrap();
    let source = OneDrive(Seen {
        uuid: UUID.into(),
        label: "PENDRIVE".into(),
        mount: Some(mount.clone()),
        device: fs::metadata(&mount).unwrap().dev(),
    });
    let (d, _) = daemon_at(c.clone());
    assert!(ok(&unlock(&d, PASSWORD)));
    let r = request(
        &d,
        Role::App,
        "set_external_backup",
        &format!(r#"{{"enabled":true,"uuid":"{UUID}"}}"#),
    );
    assert!(ok(&r), "{r:?}");
    let copied = d.sync_external(&source).unwrap().unwrap();
    assert!(!copied.copied.is_empty());
    let on_drive = mount.join("ward-backup");
    let there = version_names(&on_drive);
    for n in BACKUP_014_NAMES {
        assert!(there.contains(&n.to_owned()), "{n} não foi para o pendrive");
    }
    let mut minutes = 30;
    loop {
        minutes += 1;
        assert!(
            minutes < 30 + 2 * VERSIONS as u32,
            "a rotação do pendrive não tirou a v4"
        );
        assert!(ok(&lock_timeout(&d, minutes)));
        d.sync_external(&source).unwrap();
        let there = version_names(&on_drive);
        if BACKUP_014_NAMES
            .iter()
            .all(|n| !there.contains(&(*n).to_owned()))
        {
            assert_eq!(there.len(), VERSIONS, "{there:?}");
            for n in &there {
                let (_, _, read) = open_bytes(&fs::read(on_drive.join(n)).unwrap());
                assert_eq!(read.version, BODY_VERSION, "{n}");
            }
            break;
        }
        // While a v4 is there, every version on the drive opens (v4 or v5).
        for n in &there {
            let _ = open_bytes(&fs::read(on_drive.join(n)).unwrap());
        }
    }
}

// ---- Death at each point of the daemon's migration (m2:V4.3; `test-hooks`) ----

#[cfg(feature = "test-hooks")]
mod death {
    use super::*;
    use std::io::{BufRead, BufReader, Read as _};
    use std::process::{Child, Command, Stdio};
    use std::sync::mpsc;
    use std::time::Duration;
    use wardd::core::MigrationPoint;

    const CHILD_NAME: &str = "death::dt5_child_migration";
    const ENV_POINT: &str = "WARD_TEST_MIGRATION_POINT";
    const ENV_DIR: &str = "WARD_TEST_MIGRATION_DIR";
    /// If the parent dies without killing it, the child exits on its own.
    const CHILD_LIFETIME: Duration = Duration::from_secs(90);
    const POINTS: [MigrationPoint; 4] = [
        MigrationPoint::BeforeBurn,
        MigrationPoint::BeforeWrite,
        MigrationPoint::BeforeConfirm,
        MigrationPoint::AfterConfirm,
    ];
    /// The first two come before the vault write; the v5 is in place from the third on.
    const BEFORE_WRITE: usize = 2;

    /// Child process: unlocks the 0.1.4 vault in `ENV_DIR` with the hook stopping at the
    /// point of `ENV_POINT`. Does nothing without the environment.
    #[test]
    #[ignore = "só roda como processo filho dos testes de morte da migração"]
    fn dt5_child_migration() {
        let Ok(i) = std::env::var(ENV_POINT) else {
            return;
        };
        let target = POINTS[i.parse::<usize>().unwrap()];
        let dir = PathBuf::from(std::env::var_os(ENV_DIR).unwrap());
        let d = Daemon::new(
            wardd::config::test_paths(&dir),
            Box::new(FakeClock::default()),
        )
        .with_migration_hook(Box::new(move |p| {
            if p == target {
                println!("\nWARD-POINT");
                // Waits for the parent's SIGKILL; without it, exits without going on.
                std::thread::sleep(CHILD_LIFETIME);
                std::process::exit(3);
            }
        }));
        let r = unlock(&d, PASSWORD);
        println!("\nWARD-END {}", ok(&r));
    }

    fn child(i: usize, dir: &Path) -> Child {
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                CHILD_NAME,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(ENV_POINT, i.to_string())
            .env(ENV_DIR, dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    }

    /// Waits for the `expected` line in the child's output (up to 120 s: the child runs
    /// Argon2id of 256 MiB first).
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
        if rx.recv_timeout(Duration::from_secs(120)).is_err() {
            let _ = f.kill();
            let st = f.wait();
            panic!("o filho não chegou a {expected}: {st:?}");
        }
    }

    /// SIGKILL and the child's log (stderr).
    fn kill(mut f: Child) -> Vec<u8> {
        f.kill().unwrap();
        let st = f.wait().unwrap();
        assert!(!st.success(), "o filho tinha de morrer pelo SIGKILL");
        let mut log = Vec::new();
        f.stderr.take().unwrap().read_to_end(&mut log).unwrap();
        log
    }

    /// m2:V4.3 (S7, D53): SIGKILL of the daemon at each point of the migration (before
    /// burning, burned and before the temp file, v5 in place and before the state, after
    /// the state). The vault is the v4 byte for byte or the whole v5 (generation 4, same
    /// data), never anything else; the state always reads, with `seen` never above the
    /// vault's generation (no false rollback). The next unlock works, leaves a v5 with
    /// the same data, never with a generation already sealed, and no temp file. The log
    /// of the dead daemon has no password, PIN, entry name nor fake value.
    #[test]
    fn v4_3_sigkill_at_each_migration_point() {
        let (_, entries, _) = open_bytes(VAULT_014);
        let names: Vec<String> = entries.iter().map(|e| e.name().to_owned()).collect();
        let before = canonical(VAULT_014);
        for (i, target) in POINTS.iter().enumerate() {
            let dir = Dir::new("dt5-morte");
            let c = dir.paths();
            install_014(&c);
            let mut f = child(i, &dir.0);
            wait_line(&mut f, "WARD-POINT");
            let log = kill(f);

            let left = fs::read(&c.vault).unwrap();
            let g = files::read_state(&c.state)
                .unwrap_or_else(|e| panic!("{target:?}: estado ilegível {e:?}"));
            if i < BEFORE_WRITE {
                assert!(left == VAULT_014, "{target:?}: a v4 tem de ficar intacta");
                assert_eq!(g.seen, GENERATION_014, "{target:?}");
            } else {
                let (_, _, read) = open_bytes(&left);
                assert_eq!(read.version, BODY_VERSION, "{target:?}");
                assert_eq!(read.generation, GENERATION_014 + 1, "{target:?}");
                assert!(canonical(&left) == before, "{target:?}");
                assert!(g.seen <= read.generation, "{target:?}");
            }
            assert!(g.burned >= g.seen, "{target:?}");

            let (d, _) = daemon_at(c.clone());
            let r = unlock(&d, PASSWORD);
            assert!(ok(&r), "{target:?}: {r:?}");
            let now = fs::read(&c.vault).unwrap();
            let (_, _, read) = open_bytes(&now);
            assert_eq!(read.version, BODY_VERSION, "{target:?}");
            let expected_generation = match target {
                // Nothing burned: the migration of the next unlock seals 4.
                MigrationPoint::BeforeBurn => GENERATION_014 + 1,
                // 4 burned (and maybe sealed in a temp file): the next one seals 5.
                MigrationPoint::BeforeWrite => GENERATION_014 + 2,
                // Already v5: the next unlock does not write.
                _ => GENERATION_014 + 1,
            };
            assert_eq!(read.generation, expected_generation, "{target:?}");
            assert!(canonical(&now) == before, "{target:?}");
            let g = files::read_state(&c.state).unwrap();
            assert_eq!(
                g,
                Generations {
                    seen: expected_generation,
                    burned: expected_generation
                },
                "{target:?}"
            );
            assert!(state_text(&c).starts_with("ward-state 2\n"), "{target:?}");
            // The unlock cleans the vault's temp files; the startup, the state's.
            assert_eq!(
                names_in(&dir.0),
                ["backups", "test-vault.state", "test-vault.ward"],
                "{target:?}"
            );
            // Once: a new daemon opens the v5 without writing it.
            let id = identity(&c.vault);
            drop(d);
            let (d2, _) = daemon_at(c.clone());
            assert!(ok(&unlock(&d2, PASSWORD)));
            assert_eq!(identity(&c.vault), id, "{target:?}");

            let mut needles: Vec<Vec<u8>> = vec![
                PASSWORD.as_bytes().to_vec(),
                PIN.as_bytes().to_vec(),
                b64(PASSWORD.as_bytes()).into_bytes(),
                b"falso-".to_vec(),
            ];
            needles.extend(names.iter().map(|n| n.as_bytes().to_vec()));
            for n in &needles {
                assert!(
                    !contains(&log, n),
                    "{target:?}: {:?} no log",
                    String::from_utf8_lossy(n)
                );
            }
            assert!(
                contains(&log, b"corpo v4"),
                "{target:?}: o log não diz da migração"
            );
        }
    }
}
