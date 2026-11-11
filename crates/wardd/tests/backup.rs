//! Local backup and restore in the daemon (Phase 45; D17, D27, D39): each write goes to
//! the backup (V7.1), restore only through the app (V7.8), restored version sealed with
//! a new generation and the hand copy in rollback (V7.11), folder swapped for a link,
//! PIN and password of the version, vault swapped outside Ward, state that does not
//! write (S7, S9) and permissions.
//!
//! Own test vault, only with fake values (never the real vault nor the one in `.dev/`).

mod common;

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use common::*;
use ward_core::backup::Version;
use ward_core::entry::Secret;
use ward_core::protocol::{BackupVersion, MSG_ROLLBACK, Request, Response, ResponseError, Role};
use wardd::arquivos::Geracoes;
use wardd::nucleo::Daemon;

fn backup_dir(dir: &Dir) -> PathBuf {
    dir.0.join("backups")
}

fn mode(p: &Path) -> u32 {
    fs::symlink_metadata(p).unwrap().permissions().mode() & 0o7777
}

/// The names of the versions in the folder, from newest to oldest (through the app).
fn backups(d: &Daemon) -> Vec<String> {
    versions(d).into_iter().map(|v| v.name).collect()
}

/// The versions with the sealed generation (Phase 49a).
fn versions(d: &Daemon) -> Vec<BackupVersion> {
    match request(d, Role::App, "backups", "{}") {
        Response::Backups(l) => l,
        other => panic!("{other:?}"),
    }
}

fn restore(d: &Daemon, version: &str, password: &str, pin: &str) -> Response {
    request(
        d,
        Role::App,
        "restaurar_backup",
        &format!(
            r#"{{"versao":"{version}","senha":"{}","pin":"{}"}}"#,
            b64(password.as_bytes()),
            b64(pin.as_bytes())
        ),
    )
}

fn list_names(d: &Daemon) -> Vec<String> {
    match request(d, Role::App, "lista", "{}") {
        Response::List(l) => l.into_iter().map(|i| i.name).collect(),
        other => panic!("{other:?}"),
    }
}

fn lock_timeout(d: &Daemon, minutes: u32) {
    let r = request(
        d,
        Role::App,
        "tempo_trava",
        &format!(r#"{{"minutos":{minutes}}}"#),
    );
    assert!(ok(&r), "{r:?}");
}

/// V7.1 (D17): each daemon write (create, save, change level) leaves a version in the
/// `backups/` folder next to the vault, 0700, with 0600 files equal to the written vault
/// and without a secret in clear.
#[test]
fn v7_1_each_daemon_write_goes_to_the_backup() {
    let dir = Dir::new("bk-cada");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    // Vault creation + 5 entries + 2 level changes.
    let list = backups(&d);
    assert_eq!(list.len(), 8, "{list:?}");
    assert_eq!(mode(&backup_dir(&dir)), 0o700);
    for n in &list {
        assert!(Version::from_name(n).is_some(), "{n}");
        let p = backup_dir(&dir).join(n);
        assert_eq!(mode(&p), 0o600, "{n}");
        let b = fs::read(&p).unwrap();
        for s in SECRETS.iter().chain([PASSWORD, PIN].iter()) {
            assert!(!contains(&b, s.as_bytes()), "segredo em claro em {n}");
        }
    }
    assert_eq!(
        fs::read(backup_dir(&dir).join(&list[0])).unwrap(),
        fs::read(&c.cofre).unwrap(),
        "a mais nova é o cofre atual"
    );
    // The generation of each version: 1 (create) up to 8, from oldest to newest.
    let mut g: Vec<u64> = list
        .iter()
        .map(|n| generation_on_disk(&backup_dir(&dir).join(n), PASSWORD))
        .collect();
    g.reverse();
    assert_eq!(g, (1..=8).collect::<Vec<_>>());
    // The reply to the app has only names and the marks (Phase 49a): all checked and in
    // order; no path nor generation (D39).
    let v = versions(&d);
    assert!(v.iter().all(|x| x.verified && !x.out_of_order), "{v:?}");
    let bytes = encoded(Role::App, &Response::Backups(v));
    assert!(!contains(&bytes, dir.0.to_string_lossy().as_bytes()));
    assert!(!contains(&bytes, b"geracao"));
}

/// V7.8 (D17): the agent and the extension asking to restore or list backups get "not
/// authorized" in the protocol and in the daemon (second layer), with the vault unlocked
/// and the right PIN; nothing changes on disk.
#[test]
fn v7_8_agent_and_extension_do_not_restore() {
    let dir = Dir::new("bk-papel");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    let target = backups(&d).pop().unwrap();
    let before = fs::read(&c.cofre).unwrap();
    let state_before = fs::read(&c.estado).unwrap();
    for role in [Role::Agent, Role::Extension] {
        let r = request(
            &d,
            role,
            "restaurar_backup",
            &format!(
                r#"{{"versao":"{target}","senha":"{}","pin":"{}"}}"#,
                b64(PASSWORD.as_bytes()),
                b64(PIN.as_bytes())
            ),
        );
        assert_eq!(
            error(&r),
            Some(ResponseError::Unauthorized),
            "{}",
            role.name()
        );
        let r = request(&d, role, "backups", "{}");
        assert_eq!(
            error(&r),
            Some(ResponseError::Unauthorized),
            "{}",
            role.name()
        );
        // Second layer: the request built in memory, without going through the protocol.
        let r = d.atender(
            role,
            Request::RestoreBackup {
                version: target.clone(),
                password: Secret::from_text(PASSWORD.to_owned()),
                pin: Secret::from_text(PIN.to_owned()),
            },
        );
        assert_eq!(
            error(&r),
            Some(ResponseError::Unauthorized),
            "{}",
            role.name()
        );
        assert_eq!(
            error(&d.atender(role, Request::Backups)),
            Some(ResponseError::Unauthorized)
        );
    }
    assert_eq!(fs::read(&c.cofre).unwrap(), before);
    assert_eq!(fs::read(&c.estado).unwrap(), state_before);
    assert!(!d.travado());
}

/// V7.11 (D39, D17; S7, S9): vault at generation n+2 and the backup of generation n.
/// Restoring through the app seals the version again at n+3 (vault and state), the
/// current one (n+2) becomes a backup, the daemon unlocks with it and the entries are
/// those of version n. The same backup copied by hand over the vault falls into the
/// rollback (V3.22), also in a new daemon.
#[test]
fn v7_11_restore_seals_a_new_generation_and_hand_copy_is_rollback() {
    let dir = Dir::new("bk-v7-11");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    let n = generation_on_disk(&c.cofre, PASSWORD);
    let version_n = backups(&d)[0].clone();
    assert_eq!(
        generation_on_disk(&backup_dir(&dir).join(&version_n), PASSWORD),
        n
    );
    let copy_n = fs::read(backup_dir(&dir).join(&version_n)).unwrap();
    let entries_n = list_names(&d);

    // Two writes: a new entry and the lock timeout.
    let r = request(
        &d,
        Role::App,
        "salvar_entrada",
        &login("t-nova", SECRETS[0]),
    );
    assert!(ok(&r), "{r:?}");
    lock_timeout(&d, 30);
    assert_eq!(generation_on_disk(&c.cofre, PASSWORD), n + 2);
    let current_n2 = fs::read(&c.cofre).unwrap();
    assert_ne!(list_names(&d), entries_n);

    let r = restore(&d, &version_n, PASSWORD, PIN);
    assert!(ok(&r), "{r:?}");
    assert_eq!(generation_on_disk(&c.cofre, PASSWORD), n + 3);
    assert_eq!(
        generations(&dir),
        Geracoes {
            vista: n + 3,
            queimada: n + 3
        }
    );
    assert_eq!(list_names(&d), entries_n);
    // The previous current one (n+2) stayed in the folder, equal byte for byte, and the
    // new version too.
    let list = backups(&d);
    assert_eq!(
        fs::read(backup_dir(&dir).join(&list[0])).unwrap(),
        fs::read(&c.cofre).unwrap()
    );
    assert!(
        list.iter()
            .any(|v| fs::read(backup_dir(&dir).join(v)).unwrap() == current_n2),
        "o cofre de antes não virou backup"
    );
    // The restored version n is still in the folder, unchanged.
    assert_eq!(fs::read(backup_dir(&dir).join(&version_n)).unwrap(), copy_n);

    // Lock and unlock with the version's password: the entries of version n.
    d.travar();
    let r = unlock(&d, PASSWORD);
    assert!(ok(&r), "{r:?}");
    assert_eq!(list_names(&d), entries_n);
    d.travar();

    // Hand copy of the same backup: generation n is lower than the seen one (n+3).
    fs::write(&c.cofre, &copy_n).unwrap();
    let r = unlock(&d, PASSWORD);
    assert_eq!(error(&r), Some(ResponseError::Rollback));
    assert!(contains(&encoded(Role::Agent, &r), MSG_ROLLBACK.as_bytes()));
    assert!(d.travado());
    // In a new daemon, straight to the rollback; the copied file is not rewritten.
    drop(d);
    let (d2, _) = daemon_at(dir.paths());
    assert_eq!(error(&unlock(&d2, PASSWORD)), Some(ResponseError::Rollback));
    assert_eq!(fs::read(&c.cofre).unwrap(), copy_n);
}

/// S9: restoring requires the vault unlocked and the current PIN (the error counts
/// towards the 5 that lock) and the version's password; a name that does not exist, a
/// wrong password or a wrong PIN touch nothing.
#[test]
fn attack_restore_without_pin_wrong_password_and_locked() {
    let dir = Dir::new("bk-pin");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    let target = backups(&d).pop().unwrap();
    let before = fs::read(&c.cofre).unwrap();
    let state = fs::read(&c.estado).unwrap();
    let n_backups = backups(&d).len();

    let r = restore(&d, &target, PASSWORD, WRONG_PIN);
    assert_eq!(error(&r), Some(ResponseError::WrongPin));
    let r = restore(&d, &target, "falso-mestra-ward-02", PIN);
    assert_eq!(error(&r), Some(ResponseError::WrongPassword));
    let r = restore(&d, "ward-20000101-000000.ward", PASSWORD, PIN);
    assert_eq!(error(&r), Some(ResponseError::BackupNotFound));
    let r = restore(&d, "../test-vault.ward", PASSWORD, PIN);
    assert_eq!(error(&r), Some(ResponseError::InvalidRequest));
    assert_eq!(fs::read(&c.cofre).unwrap(), before);
    assert_eq!(fs::read(&c.estado).unwrap(), state);
    assert_eq!(backups(&d).len(), n_backups);

    // 5 wrong PINs in a row through the restore lock the vault.
    for i in 0..5 {
        let r = restore(&d, &target, PASSWORD, WRONG_PIN);
        assert_eq!(error(&r), Some(ResponseError::WrongPin), "tentativa {i}");
    }
    assert!(d.travado());
    // Locked: neither lists nor restores, with the right PIN.
    assert_eq!(
        error(&request(&d, Role::App, "backups", "{}")),
        Some(ResponseError::Locked)
    );
    assert_eq!(
        error(&restore(&d, &target, PASSWORD, PIN)),
        Some(ResponseError::Locked)
    );
    assert_eq!(fs::read(&c.cofre).unwrap(), before);
}

/// The backups folder swapped for a symbolic link to another folder (with an authentic
/// copy of an old version): listing and restoring answer "unavailable", and the vault
/// write goes on without writing anything at the link target. The same with the folder
/// at 0755.
#[test]
fn attack_backups_folder_swapped_for_a_link() {
    let dir = Dir::new("bk-link");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    let target_version = backups(&d).pop().unwrap();
    let old = fs::read(backup_dir(&dir).join(&target_version)).unwrap();

    let other = dir.0.join("outra");
    fs::create_dir(&other).unwrap();
    fs::set_permissions(&other, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(other.join(&target_version), &old).unwrap();
    fs::set_permissions(
        other.join(&target_version),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    fs::rename(backup_dir(&dir), dir.0.join("backups-verdadeira")).unwrap();
    symlink(&other, backup_dir(&dir)).unwrap();

    assert_eq!(
        error(&request(&d, Role::App, "backups", "{}")),
        Some(ResponseError::BackupUnavailable)
    );
    let before = fs::read(&c.cofre).unwrap();
    assert_eq!(
        error(&restore(&d, &target_version, PASSWORD, PIN)),
        Some(ResponseError::BackupUnavailable)
    );
    assert_eq!(fs::read(&c.cofre).unwrap(), before);
    // The write goes on and does not write at the link target.
    lock_timeout(&d, 45);
    assert_ne!(fs::read(&c.cofre).unwrap(), before);
    let at_target: Vec<_> = fs::read_dir(&other).unwrap().collect();
    assert_eq!(at_target.len(), 1, "o backup foi gravado através do link");

    // The real folder back, but 0755: also refused.
    fs::remove_file(backup_dir(&dir)).unwrap();
    fs::rename(dir.0.join("backups-verdadeira"), backup_dir(&dir)).unwrap();
    fs::set_permissions(backup_dir(&dir), fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        error(&request(&d, Role::App, "backups", "{}")),
        Some(ResponseError::BackupUnavailable)
    );
    fs::set_permissions(backup_dir(&dir), fs::Permissions::from_mode(0o700)).unwrap();
    assert!(!backups(&d).is_empty());
}

/// A version in the folder swapped for a symbolic link to an authentic copy outside
/// the folder: the daemon does not follow it (`O_NOFOLLOW`) and does not restore; the
/// list ignores it.
#[test]
fn attack_version_swapped_for_a_link() {
    let dir = Dir::new("bk-versao-link");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    let v = backups(&d).pop().unwrap();
    let outside = dir.0.join("copia-fora.ward");
    fs::rename(backup_dir(&dir).join(&v), &outside).unwrap();
    symlink(&outside, backup_dir(&dir).join(&v)).unwrap();
    assert!(!backups(&d).contains(&v));
    let before = fs::read(&c.cofre).unwrap();
    // Phase 49a: the restore only accepts a version from the list (which ignores links);
    // the read with `O_NOFOLLOW` stays for the swap between the list and the open.
    assert_eq!(
        error(&restore(&d, &v, PASSWORD, PIN)),
        Some(ResponseError::BackupNotFound)
    );
    assert_eq!(fs::read(&c.cofre).unwrap(), before);
    assert!(!d.travado());
}

/// D39: the vault file swapped outside Ward before the restore (another inode, same
/// bytes): nothing is restored, the vault locks and the swapped file stays as it is.
#[test]
fn attack_restore_with_vault_swapped_outside_ward() {
    let dir = Dir::new("bk-mudou");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    let v = backups(&d).pop().unwrap();
    let bytes = fs::read(&c.cofre).unwrap();
    let tmp = dir.0.join("troca");
    fs::write(&tmp, &bytes).unwrap();
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600)).unwrap();
    fs::rename(&tmp, &c.cofre).unwrap();
    let n_backups = backups(&d).len();
    assert_eq!(
        error(&restore(&d, &v, PASSWORD, PIN)),
        Some(ResponseError::VaultChanged)
    );
    assert!(d.travado());
    assert_eq!(fs::read(&c.cofre).unwrap(), bytes);
    let (d2, _) = daemon_at(dir.paths());
    assert!(ok(&unlock(&d2, PASSWORD)));
    assert_eq!(backups(&d2).len(), n_backups, "nada novo na pasta");
}

/// S7/D53: with the state file unable to be written, the restore does not swap the
/// vault and the daemon goes on with the previous entries; nothing of the version goes
/// to the vault's disk.
#[test]
fn attack_restore_without_writing_the_state_does_not_swap_the_vault() {
    let dir = Dir::new("bk-estado");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    let v = backups(&d).pop().unwrap();
    let before = fs::read(&c.cofre).unwrap();
    let list_before = list_names(&d);
    // The vault directory without write permission: the state's temp file is not born.
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o500)).unwrap();
    let r = restore(&d, &v, PASSWORD, PIN);
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(error(&r), Some(ResponseError::WriteFailed));
    assert_eq!(fs::read(&c.cofre).unwrap(), before);
    assert!(!d.travado());
    assert_eq!(list_names(&d), list_before);
    // The next common write works and the generation does not repeat.
    let g = generation_on_disk(&c.cofre, PASSWORD);
    lock_timeout(&d, 50);
    assert!(generation_on_disk(&c.cofre, PASSWORD) > g);
}

/// The password change goes to the backup with the new password, and restoring the
/// previous version asks for the old password (D17: the password of that version).
#[test]
fn restore_asks_for_the_version_password() {
    let dir = Dir::new("bk-senha");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    let old = backups(&d)[0].clone();
    let new_password = "falso-mestra-ward-05";
    let r = request(
        &d,
        Role::App,
        "trocar_senha",
        &format!(
            r#"{{"atual":"{}","nova":"{}"}}"#,
            b64(PASSWORD.as_bytes()),
            b64(new_password.as_bytes())
        ),
    );
    assert!(ok(&r), "{r:?}");
    let after = backups(&d)[0].clone();
    assert_ne!(after, old);
    assert_eq!(
        fs::read(backup_dir(&dir).join(&after)).unwrap(),
        fs::read(&c.cofre).unwrap()
    );
    // The old version only opens with the old password.
    assert_eq!(
        error(&restore(&d, &old, new_password, PIN)),
        Some(ResponseError::WrongPassword)
    );
    assert!(ok(&restore(&d, &old, PASSWORD, PIN)));
    d.travar();
    assert_eq!(
        error(&unlock(&d, new_password)),
        Some(ResponseError::WrongPassword)
    );
    let (d2, _) = daemon_at(dir.paths());
    assert!(ok(&unlock(&d2, PASSWORD)));
}

// ---------------------------------------------------------------- Phase 49a

fn version_at(t: SystemTime) -> String {
    Version::from_instant(t).unwrap().name()
}

fn plant(dir: &Dir, name: &str, bytes: &[u8]) {
    let p = backup_dir(dir).join(name);
    fs::write(&p, bytes).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
}

/// Phase 49a (medium finding of M7): 30 old authentic copies planted with a future-date
/// name (any process of the user can write in the folder) do not make the daemon delete
/// the real versions on the next write; the ones past the margin do not show in the
/// list nor restore; the ones within the margin show with the old generation, which
/// gives them away (the date in the name is not trusted, the generation is
/// authenticated).
#[test]
fn attack_future_dated_versions_neither_delete_the_real_ones_nor_restore() {
    let dir = Dir::new("bk-futuro");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    let real = backups(&d);
    assert_eq!(real.len(), 8);
    let oldest = fs::read(backup_dir(&dir).join(real.last().unwrap())).unwrap();
    let now = SystemTime::now();
    let mut far = Vec::new();
    let mut near = Vec::new();
    for i in 0..30u64 {
        let n = version_at(now + Duration::from_secs(10 * 365 * 86_400 + i * 3600));
        plant(&dir, &n, &oldest);
        far.push(n);
        let n = version_at(now + Duration::from_secs(60 + i));
        plant(&dir, &n, &oldest);
        near.push(n);
    }
    // A real write: the rotation deletes neither a real nor a planted one.
    lock_timeout(&d, 45);
    for n in real.iter().chain(&far).chain(&near) {
        assert!(backup_dir(&dir).join(n).exists(), "{n} sumiu");
    }
    let v = versions(&d);
    // The ones past the margin do not show; the ones within come out marked (generation
    // 1, the old one, lower than that of the real ones with an earlier date); the real
    // ones do not.
    assert!(v.iter().all(|x| !far.contains(&x.name)));
    assert_eq!(v.len(), real.len() + 1 + near.len());
    for x in &v {
        assert!(x.verified, "{}", x.name);
        assert_eq!(x.out_of_order, near.contains(&x.name), "{}", x.name);
    }
    // Restoring one past the margin: nonexistent, nothing changes.
    let before = fs::read(&c.cofre).unwrap();
    let state = fs::read(&c.estado).unwrap();
    assert_eq!(
        error(&restore(&d, &far[0], PASSWORD, PIN)),
        Some(ResponseError::BackupNotFound)
    );
    assert_eq!(fs::read(&c.cofre).unwrap(), before);
    assert_eq!(fs::read(&c.estado).unwrap(), state);
    // 40 more writes: 30 dated up to now remain; the 2036 ones are still there.
    for i in 0..40u32 {
        lock_timeout(&d, 30 + (i % 2));
    }
    let up_to_now = fs::read_dir(backup_dir(&dir))
        .unwrap()
        .filter_map(|e| Version::from_name(&e.unwrap().file_name().into_string().unwrap()))
        .filter(|v| {
            *v <= Version::from_instant(SystemTime::now())
                .unwrap()
                .end_of_second()
        })
        .count();
    assert_eq!(up_to_now, ward_core::backup::VERSIONS);
    for n in &far {
        assert!(backup_dir(&dir).join(n).exists(), "{n} sumiu");
    }
}

/// Phase 49a (low finding): restoring an old version keeps the current lock timeout and
/// external backup (the version carries its own; restoring does not turn off the copy
/// to the USB drive without warning). Entries, password and PIN are the version's.
#[test]
fn restore_keeps_the_current_lock_timeout_and_external_backup() {
    let dir = Dir::new("bk-preserva");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    let version = backups(&d)[0].clone();
    let entries = list_names(&d);
    lock_timeout(&d, 30);
    let r = request(
        &d,
        Role::App,
        "definir_backup_externo",
        r#"{"ligado":true,"uuid":"ABCD-1234"}"#,
    );
    assert!(ok(&r), "{r:?}");
    assert!(ok(&restore(&d, &version, PASSWORD, PIN)));
    assert_eq!(list_names(&d), entries);
    match request(&d, Role::App, "configuracao", "{}") {
        Response::Config {
            lock_minutes: minutos_trava,
            external_backup: backup_externo,
            local_backup: backup_local,
        } => {
            assert_eq!(minutos_trava, 30);
            assert!(backup_externo);
            assert!(backup_local);
        }
        r => panic!("{r:?}"),
    }
    match request(&d, Role::App, "backup_externo", "{}") {
        Response::ExternalBackup {
            enabled: ligado,
            uuid,
        } => {
            assert!(ligado);
            assert_eq!(uuid.as_deref(), Some("ABCD-1234"));
        }
        r => panic!("{r:?}"),
    }
    // On disk too (the unlock reads the settings from the file).
    let bytes = fs::read(&c.cofre).unwrap();
    let s = ward_core::format::MasterPassword::new(PASSWORD.to_owned());
    let (opened, _, _) = ward_core::body::open_entries_with_generation(&bytes, &s, 0, 0).unwrap();
    assert_eq!(opened.config().lock_minutes(), 30);
    assert!(opened.config().backup().enabled());
}

/// Phase 49a (low finding): the unsafe backups folder becomes a warning on the screen
/// (the settings reply with the local backup flag off), without a path; safe again, it
/// comes back.
#[test]
fn unsafe_backups_folder_shows_in_the_settings() {
    let dir = Dir::new("bk-aviso");
    let (d, _) = test_vault(&dir);
    let local = |d: &Daemon| match request(d, Role::App, "configuracao", "{}") {
        Response::Config {
            local_backup: backup_local,
            ..
        } => backup_local,
        r => panic!("{r:?}"),
    };
    assert!(local(&d));
    for m in [0o755, 0o750, 0o707] {
        fs::set_permissions(backup_dir(&dir), fs::Permissions::from_mode(m)).unwrap();
        assert!(!local(&d), "{m:o}");
    }
    fs::set_permissions(backup_dir(&dir), fs::Permissions::from_mode(0o700)).unwrap();
    assert!(local(&d));
    // Swapped for a link to another 0700 folder: unsafe.
    let other = dir.0.join("outra");
    fs::create_dir(&other).unwrap();
    fs::set_permissions(&other, fs::Permissions::from_mode(0o700)).unwrap();
    fs::rename(backup_dir(&dir), dir.0.join("verdadeira")).unwrap();
    symlink(&other, backup_dir(&dir)).unwrap();
    assert!(!local(&d));
    let bytes = encoded(Role::App, &request(&d, Role::App, "configuracao", "{}"));
    assert!(!contains(&bytes, dir.0.to_string_lossy().as_bytes()));
}

/// Phase 49a (low finding): the state file's temp files from an interrupted write go
/// away at startup; what is not exactly one of them (link, folder, similar name, the
/// one of another file) stays.
#[test]
fn state_temp_files_go_away_at_startup() {
    let dir = Dir::new("bk-estado-tmp");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    drop(d);
    let parent = c.estado.parent().unwrap().to_path_buf();
    let name = c.estado.file_name().unwrap().to_string_lossy().into_owned();
    let stale = parent.join(format!(".{name}.tmp-4194301-7"));
    let stale2 = parent.join(format!(".{name}.tmp-12-0"));
    fs::write(&stale, b"vista 3\nqueimada 3\n").unwrap();
    fs::write(&stale2, b"x").unwrap();
    let target = dir.0.join("alvo-do-link");
    fs::write(&target, b"nao apagar").unwrap();
    let link = parent.join(format!(".{name}.tmp-4194302-1"));
    symlink(&target, &link).unwrap();
    let same_named_dir = parent.join(format!(".{name}.tmp-4194303-1"));
    fs::create_dir(&same_named_dir).unwrap();
    let lookalikes = [
        parent.join(format!(".{name}.tmp-abc-1")),
        parent.join(format!(".{name}.tmp-1")),
        parent.join(format!(".{name}.tmp-1-2-3")),
        parent.join(format!("{name}.tmp-1-2")),
        parent.join(".outro.estado.tmp-1-2"),
    ];
    for p in &lookalikes {
        fs::write(p, b"fica").unwrap();
    }
    let (d, _) = daemon_at(dir.paths());
    assert!(!stale.exists() && !stale2.exists());
    assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
    assert_eq!(fs::read(&target).unwrap(), b"nao apagar");
    assert!(same_named_dir.is_dir());
    for p in &lookalikes {
        assert!(p.exists(), "{}", p.display());
    }
    // The real state stays and the daemon unlocks with it.
    assert!(ok(&unlock(&d, PASSWORD)));
}

/// Phase 54l (finding of 54c), attack: the same uid swaps a backup version for a FIFO
/// nonstop, between the `readdir` of the list (which only accepts a regular file) and
/// the `open` of each version (the restore list, the backup listing of the core). Before, the `open`
/// without `O_NONBLOCK` fell into the FIFO and stayed stuck holding the daemon state
/// (no request was served any more). Now each list returns at once, the version goes
/// away or shows unchecked while it is a FIFO, and the daemon keeps serving.
#[test]
fn f54l_attack_fifo_swapped_in_place_of_the_version_does_not_hold_the_daemon() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    let dir = Dir::new("bk-fifo");
    let (d, _) = test_vault(&dir);
    let d = Arc::new(d);
    let all = backups(&d);
    let total = all.len();
    let v = all.last().unwrap().clone();
    let p = backup_dir(&dir);
    let real = p.join("x-real");
    fs::hard_link(p.join(&v), &real).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let swapper = {
        let (p, v, real, stop) = (p.clone(), v.clone(), real.clone(), stop.clone());
        std::thread::spawn(move || {
            let mut n = 0u64;
            while !stop.load(Ordering::Acquire) {
                let fifo = p.join(format!("x-fifo-{n}"));
                let st = std::process::Command::new("/usr/bin/mkfifo")
                    .arg("-m")
                    .arg("600")
                    .arg(&fifo)
                    .status()
                    .unwrap();
                assert!(st.success());
                fs::rename(&fifo, p.join(&v)).unwrap();
                std::thread::sleep(Duration::from_micros(500));
                let back = p.join(format!("x-volta-{n}"));
                fs::hard_link(&real, &back).unwrap();
                fs::rename(&back, p.join(&v)).unwrap();
                std::thread::sleep(Duration::from_micros(500));
                n += 1;
            }
            // At the end, the real version in place.
            n
        })
    };
    let end = std::time::Instant::now() + Duration::from_secs(3);
    let mut lists = 0;
    while std::time::Instant::now() < end {
        let (tx, rx) = std::sync::mpsc::channel();
        let d2 = d.clone();
        std::thread::spawn(move || {
            let _ = tx.send(versions(&d2).len());
        });
        let n = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("a lista dos backups ficou presa num FIFO (o daemon parou)");
        assert!(n <= total, "{n} > {total}");
        lists += 1;
    }
    stop.store(true, Ordering::Release);
    let swaps = swapper.join().unwrap();
    assert!(swaps > 100 && lists > 100, "{swaps} trocas, {lists} listas");
    // The real version checks again; the daemon stays unlocked.
    let l = versions(&d);
    assert!(l.iter().any(|x| x.name == v && x.verified), "{l:?}");
    assert!(!d.travado());
}
