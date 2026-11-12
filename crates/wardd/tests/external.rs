//! External backup on a USB drive (Phase 46; V7.4, V7.5, V7.6, V7.8).
//!
//! No device is mounted: the "USB drive" is a directory in `target/` and the source of
//! the USB drives is a list (or a fake udisks2 on a private `dbus-daemon` of this test,
//! like the fake logind of Phase 24). The `st_dev` given by the source is the
//! directory's, which is what udisks2 would give for a file system mounted there.
//!
//! Attacks (all fail on the attacker's side):
//! - wrong UUID, drive B in place of A, two with the same UUID, a device that does not
//!   match the mount and a udisks2 that changes its answer between the queries: nothing
//!   is written;
//! - a mount that is a symbolic link, a `ward-backup` that is a link or a file: nothing
//!   is written at the target;
//! - a version that is already on the drive (no `link(2)`, as on vfat) is never
//!   overwritten;
//! - off by default, locked or turned off later: nothing is queried nor written;
//! - the agent and the extension do not read, turn on or list USB drives (V7.8).

mod common;

use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::*;
use ward_core::backup::VERSIONS;
use ward_core::protocol::{Response, ResponseError, Role};
use wardd::core::Daemon;
use wardd::external::{ExternalError, Seen, UsbDriveSource};
use zbus::connection::Builder;
use zbus::{Connection, OwnedObjectPath, OwnedVariant, Variant};

const UUID_A: &str = "ABCD-1234";
const UUID_B: &str = "0F3C-9A51";

// ---------------------------------------------------------------- fake source

/// Source of USB drives: the list the test sets, counting the queries.
#[derive(Clone, Default)]
struct Source {
    seen: Arc<Mutex<Vec<Seen>>>,
    /// The next answers, one per query (to change its mind halfway).
    next: Arc<Mutex<Vec<Vec<Seen>>>>,
    queries: Arc<AtomicUsize>,
}

impl Source {
    fn with(seen: Vec<Seen>) -> Source {
        let f = Source::default();
        *f.seen.lock().unwrap() = seen;
        f
    }
    fn set(&self, seen: Vec<Seen>) {
        *self.seen.lock().unwrap() = seen;
    }
    fn queries(&self) -> usize {
        self.queries.load(Ordering::SeqCst)
    }
}

impl UsbDriveSource for Source {
    fn usb_drives(&self) -> Option<Vec<Seen>> {
        self.queries.fetch_add(1, Ordering::SeqCst);
        let mut p = self.next.lock().unwrap();
        if !p.is_empty() {
            return Some(p.remove(0));
        }
        Some(self.seen.lock().unwrap().clone())
    }
}

/// A "USB drive" mounted at `dir` (the device is the directory's `st_dev`).
fn usb_drive(uuid: &str, dir: &Path) -> Seen {
    Seen {
        uuid: uuid.into(),
        label: "PENDRIVE".into(),
        mount: Some(dir.to_path_buf()),
        device: fs::metadata(dir).unwrap().dev(),
    }
}

fn mount_point(dir: &Dir, name: &str) -> PathBuf {
    let p = dir.0.join(name);
    fs::create_dir_all(&p).unwrap();
    p
}

fn turn_on(d: &Daemon, role: Role, uuid: &str) -> Response {
    request(
        d,
        role,
        "set_external_backup",
        &format!(r#"{{"enabled":true,"uuid":"{uuid}"}}"#),
    )
}

/// Versions in the local backup, from newest to oldest.
fn local_versions(dir: &Dir) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir.0.join("backups"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| ward_core::backup::valid_name(n))
        .collect();
    v.sort_by_key(|n| std::cmp::Reverse(ward_core::backup::Version::from_name(n).unwrap()));
    v
}

/// Everything in the directory, recursive, with the content (links by their target,
/// without following).
fn tree(p: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut v = Vec::new();
    let Ok(items) = fs::read_dir(p) else {
        return v;
    };
    for e in items {
        let e = e.unwrap();
        let t = e.file_type().unwrap();
        if t.is_dir() {
            v.extend(tree(&e.path()));
        } else if t.is_symlink() {
            v.push((
                e.path(),
                fs::read_link(e.path()).unwrap().into_os_string().into_vec(),
            ));
        } else {
            v.push((e.path(), fs::read(e.path()).unwrap()));
        }
    }
    v.sort();
    v
}

use std::os::unix::ffi::OsStringExt;

fn external_dir(m: &Path) -> PathBuf {
    m.join("ward-backup")
}

/// Version names on the USB drive.
fn on_drive(m: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(external_dir(m))
        .map(|l| {
            l.map(|e| e.unwrap().file_name().into_string().unwrap())
                .filter(|n| ward_core::backup::valid_name(n))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

// ---------------------------------------------------------------- V7.4

/// V7.4: new vault, USB drive connected and mounted: nothing is copied nor queried (off
/// by default), and the settings say off, without a UUID.
#[test]
fn v7_4_off_by_default_writes_nothing() {
    let dir = Dir::new("ext-padrao");
    let (d, _) = test_vault(&dir);
    let a = mount_point(&dir, "pendrive-a");
    let source = Source::with(vec![usb_drive(UUID_A, &a)]);
    let d = d.with_usb_drives(Arc::new(source.clone()));
    match request(&d, Role::App, "external_backup", "{}") {
        Response::ExternalBackup { enabled, uuid } => {
            assert!(!enabled);
            assert_eq!(uuid, None);
        }
        r => panic!("{r:?}"),
    }
    assert_eq!(d.sync_external(&source), Ok(None));
    assert_eq!(source.queries(), 0, "desligado nem consulta o udisks2");
    assert!(tree(&a).is_empty(), "nada no pendrive");
    // The list of USB drives (for the app to choose from) works and writes nothing.
    match request(&d, Role::App, "usb_drives", "{}") {
        Response::UsbDrives(l) => {
            assert_eq!(l.len(), 1);
            assert_eq!(l[0].uuid, UUID_A);
            assert!(l[0].mounted);
        }
        r => panic!("{r:?}"),
    }
    assert!(tree(&a).is_empty());
}

// ---------------------------------------------------------------- V7.5, V7.6

/// V7.5: on with A's UUID, the versions go to `<mount A>/ward-backup/`, equal to the
/// local backup's; B connected (alone or together) gets nothing. V7.6: no secret,
/// password or PIN in clear or in base64 on the drive, and each copy opens with the
/// test password.
#[test]
fn v7_5_v7_6_copies_only_to_the_chosen_drive_and_without_secret() {
    let dir = Dir::new("ext-a-b");
    let (d, _) = test_vault(&dir);
    let a = mount_point(&dir, "pendrive-a");
    let b = mount_point(&dir, "pendrive-b");
    let source = Source::with(vec![usb_drive(UUID_B, &b)]);
    assert!(ok(&turn_on(&d, Role::App, UUID_A)));
    match request(&d, Role::App, "external_backup", "{}") {
        Response::ExternalBackup { enabled, uuid } => {
            assert!(enabled);
            assert_eq!(uuid.as_deref(), Some(UUID_A));
        }
        r => panic!("{r:?}"),
    }
    // Only B connected: nothing.
    assert_eq!(d.sync_external(&source), Err(ExternalError::NotConnected));
    assert!(tree(&b).is_empty());
    // A and B connected: only A gets it.
    source.set(vec![usb_drive(UUID_A, &a), usb_drive(UUID_B, &b)]);
    let c = d.sync_external(&source).unwrap().unwrap();
    let locals = local_versions(&dir);
    assert_eq!(c.copied.len(), locals.len());
    assert!(tree(&b).is_empty(), "o pendrive B não recebe nada");
    let mut expected = locals.clone();
    expected.sort();
    assert_eq!(on_drive(&a), expected);
    for n in &locals {
        assert_eq!(
            fs::read(external_dir(&a).join(n)).unwrap(),
            fs::read(dir.0.join("backups").join(n)).unwrap(),
            "{n}"
        );
    }
    // Nothing besides the versions (not even a temp file).
    assert_eq!(tree(&a).len(), locals.len());
    // Again: nothing to copy, nothing changes.
    let before = tree(&a);
    assert!(d.sync_external(&source).unwrap().unwrap().copied.is_empty());
    assert_eq!(tree(&a), before);
    // A new write: only it goes.
    let r = request(&d, Role::App, "save_entry", &login("t-nova", SECRETS[3]));
    assert!(ok(&r), "{r:?}");
    let c = d.sync_external(&source).unwrap().unwrap();
    assert_eq!(c.copied.len(), 1);
    assert_eq!(on_drive(&a).len(), locals.len() + 1);
    // UUID in lowercase from udisks2 (vfat comes in uppercase; either way).
    source.set(vec![usb_drive(&UUID_A.to_lowercase(), &a)]);
    assert!(d.sync_external(&source).unwrap().is_some());

    // V7.6: the auditor looks for secrets in the drive's files.
    let mut needles: Vec<Vec<u8>> = Vec::new();
    for s in SECRETS.iter().chain([&PASSWORD, &PIN]) {
        needles.push(s.as_bytes().to_vec());
        needles.push(b64(s.as_bytes()).trim_end_matches('=').as_bytes().to_vec());
    }
    for (path, bytes) in tree(&a) {
        for needle in &needles {
            assert!(
                !contains(&bytes, needle),
                "{} tem um valor de teste em claro",
                path.display()
            );
        }
        assert!(generation_on_disk(&path, PASSWORD) >= 1);
    }
}

// ---------------------------------------------------------------- attacks

/// Swapped drive: the source says A is mounted on a directory of another device (B in
/// its place, or something else mounted on the path), or changes its answer between the
/// first query and the one after opening. Two file systems with the same UUID also get
/// nothing.
#[test]
fn attack_swapped_drive_or_repeated_uuid_gets_nothing() {
    let dir = Dir::new("ext-trocado");
    let (d, _) = test_vault(&dir);
    let a = mount_point(&dir, "pendrive-a");
    let b = mount_point(&dir, "pendrive-b");
    assert!(ok(&turn_on(&d, Role::App, UUID_A)));
    // A device that is not the mounted directory's.
    let mut fake = usb_drive(UUID_A, &a);
    fake.device ^= 0x55;
    let source = Source::with(vec![fake]);
    assert_eq!(d.sync_external(&source), Err(ExternalError::Swapped));
    // The answer changes after opening: another device, another mount.
    for after in [
        {
            let mut v = usb_drive(UUID_A, &a);
            v.device ^= 1;
            v
        },
        usb_drive(UUID_A, &b),
    ] {
        let source = Source::with(vec![usb_drive(UUID_A, &a)]);
        source
            .next
            .lock()
            .unwrap()
            .extend([vec![usb_drive(UUID_A, &a)], vec![after]]);
        assert_eq!(d.sync_external(&source), Err(ExternalError::Swapped));
    }
    // Drive disconnected between the queries.
    let source = Source::with(vec![usb_drive(UUID_A, &a)]);
    source
        .next
        .lock()
        .unwrap()
        .extend([vec![usb_drive(UUID_A, &a)], vec![]]);
    assert_eq!(d.sync_external(&source), Err(ExternalError::NotConnected));
    // Two with the same UUID (cloned drive).
    let source = Source::with(vec![usb_drive(UUID_A, &a), usb_drive(UUID_A, &b)]);
    assert_eq!(d.sync_external(&source), Err(ExternalError::Ambiguous));
    // Connected without mounting.
    let mut unmounted = usb_drive(UUID_A, &a);
    unmounted.mount = None;
    assert_eq!(
        d.sync_external(&Source::with(vec![unmounted])),
        Err(ExternalError::NotMounted)
    );
    assert!(tree(&a).is_empty() && tree(&b).is_empty());
}

/// The mount that is a symbolic link (to a directory on the same device) and the
/// `ward-backup` that is a link or a file: nothing is written at the target.
#[test]
fn attack_mount_or_folder_that_is_a_link() {
    let dir = Dir::new("ext-link");
    let (d, _) = test_vault(&dir);
    assert!(ok(&turn_on(&d, Role::App, UUID_A)));
    let target = mount_point(&dir, "alvo");
    let link = dir.0.join("montagem-link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let mut v = usb_drive(UUID_A, &target);
    v.mount = Some(link.clone());
    assert_eq!(
        d.sync_external(&Source::with(vec![v])),
        Err(ExternalError::Swapped)
    );
    assert!(tree(&target).is_empty());

    // `ward-backup` is a link to another folder.
    let a = mount_point(&dir, "pendrive-a");
    let other = mount_point(&dir, "outra");
    std::os::unix::fs::symlink(&other, external_dir(&a)).unwrap();
    let source = Source::with(vec![usb_drive(UUID_A, &a)]);
    assert_eq!(d.sync_external(&source), Err(ExternalError::UnsafeDir));
    assert!(tree(&other).is_empty());
    assert!(fs::symlink_metadata(external_dir(&a)).unwrap().is_symlink());
    // `ward-backup` is a file.
    fs::remove_file(external_dir(&a)).unwrap();
    fs::write(external_dir(&a), b"nao e pasta").unwrap();
    assert_eq!(d.sync_external(&source), Err(ExternalError::UnsafeDir));
    assert_eq!(fs::read(external_dir(&a)).unwrap(), b"nao e pasta");
}

/// Without `link(2)` (vfat): a version that is already on the drive is never
/// overwritten, a link with a version name is not followed, the temp file of an
/// interrupted copy goes away and what has no version name stays.
#[test]
fn attack_existing_version_on_the_drive_is_not_overwritten() {
    let dir = Dir::new("ext-existe");
    let (d, _) = test_vault(&dir);
    assert!(ok(&turn_on(&d, Role::App, UUID_A)));
    let a = mount_point(&dir, "pendrive-a");
    let destination = external_dir(&a);
    fs::create_dir(&destination).unwrap();
    let locals = local_versions(&dir);
    let newest = &locals[0];
    let second = &locals[1];
    fs::write(destination.join(newest), b"versao que ja estava").unwrap();
    let outside = mount_point(&dir, "fora");
    std::os::unix::fs::symlink(outside.join("x"), destination.join(second)).unwrap();
    fs::write(destination.join(".ward-tmp-1-2-00000000"), b"pedaco").unwrap();
    fs::write(destination.join("LEIA-ME.txt"), b"do usuario").unwrap();
    let c = d
        .sync_external(&Source::with(vec![usb_drive(UUID_A, &a)]))
        .unwrap()
        .unwrap();
    assert_eq!(
        fs::read(destination.join(newest)).unwrap(),
        b"versao que ja estava"
    );
    assert!(
        fs::symlink_metadata(destination.join(second))
            .unwrap()
            .is_symlink()
    );
    assert!(!outside.join("x").exists(), "o link não foi seguido");
    assert!(!destination.join(".ward-tmp-1-2-00000000").exists());
    assert_eq!(
        fs::read(destination.join("LEIA-ME.txt")).unwrap(),
        b"do usuario"
    );
    // The link with a version name does not count as a copy: it stays, and the version
    // does not go to a name that exists.
    assert_eq!(c.copied.len(), locals.len() - 2);
}

/// Rotation on the drive: with more than 30 real generations, the 30 newest stay (the
/// same as the local backup's), without deleting what was just copied nor what is not
/// a version. Since Phase 54c, files with a version name that do not check against the
/// vault key (here, the old filler) do not count and do not leave.
#[test]
fn drive_keeps_30_versions() {
    let dir = Dir::new("ext-rodar");
    let (d, _) = test_vault(&dir);
    assert!(ok(&turn_on(&d, Role::App, UUID_A)));
    let a = mount_point(&dir, "pendrive-a");
    let destination = external_dir(&a);
    fs::create_dir(&destination).unwrap();
    let mut junk = Vec::new();
    for i in 0..5u32 {
        let n = format!("ward-20200101-{:06}.ward", i * 100);
        fs::write(destination.join(&n), b"antiga").unwrap();
        junk.push(n);
    }
    fs::write(destination.join("notas.txt"), b"x").unwrap();
    let source = Source::with(vec![usb_drive(UUID_A, &a)]);
    let mut deleted = 0;
    for i in 0..VERSIONS + 5 {
        let r = request(
            &d,
            Role::App,
            "save_entry",
            &login(&format!("t-rodar-{i}"), SECRETS[3]),
        );
        assert!(ok(&r), "{r:?}");
        let c = d.sync_external(&source).unwrap().unwrap();
        let all = on_drive(&a);
        for v in &c.copied {
            assert!(all.contains(&v.name()), "{v}");
        }
        deleted += c.deleted;
    }
    let mut real: Vec<String> = on_drive(&a)
        .into_iter()
        .filter(|n| !junk.contains(n))
        .collect();
    real.sort();
    let mut expected = local_versions(&dir);
    expected.sort();
    assert_eq!(expected.len(), VERSIONS);
    assert_eq!(real, expected, "o pendrive não tem as 30 mais novas");
    assert!(deleted > 0);
    for n in &junk {
        assert!(
            destination.join(n).exists(),
            "{n}: a rotação apagou o que não confere"
        );
    }
    assert!(destination.join("notas.txt").exists());
}

/// Phase 54c (finding of Phase 54), attack: whoever writes on the drive (vfat 0777,
/// another user of the machine) plants version names with a recent past date (after
/// the real copies, before now): junk, a version of another vault and copies of a real
/// version with a new name. Before the fix, the next copy rotated the drive by date and
/// deleted the real copies. Now nothing real leaves, and what does not check stays.
#[test]
fn f54c_attack_names_planted_on_the_drive_do_not_delete_real_copies() {
    let dir = Dir::new("ext-plantados");
    let (d, _) = test_vault(&dir);
    assert!(ok(&turn_on(&d, Role::App, UUID_A)));
    let a = mount_point(&dir, "pendrive-a");
    let source = Source::with(vec![usb_drive(UUID_A, &a)]);
    let c = d.sync_external(&source).unwrap().unwrap();
    let real = on_drive(&a);
    assert_eq!(real.len(), c.copied.len());
    assert!(!real.is_empty());
    let destination = external_dir(&a);
    let real_bytes = fs::read(destination.join(&real[0])).unwrap();
    let other = Dir::new("ext-plantados-outro");
    let _ = test_vault(&other);
    let from_other = fs::read(other.0.join("backups").join(&local_versions(&other)[0])).unwrap();
    std::thread::sleep(Duration::from_millis(1100));
    let base = ward_core::backup::Version::from_instant(std::time::SystemTime::now())
        .unwrap()
        .name();
    let mut planted = Vec::new();
    for i in 1..=VERSIONS + 5 {
        let n = base.replace(".ward", &format!("-{i}.ward"));
        let content = match i % 3 {
            0 => b"lixo com nome de versao".to_vec(),
            1 => from_other.clone(),
            _ => real_bytes.clone(),
        };
        fs::write(destination.join(&n), &content).unwrap();
        planted.push(n);
    }
    // A real write and the copy: only it goes; nothing leaves.
    std::thread::sleep(Duration::from_millis(1100));
    let r = request(
        &d,
        Role::App,
        "save_entry",
        &login("t-plantados", SECRETS[3]),
    );
    assert!(ok(&r), "{r:?}");
    let c = d.sync_external(&source).unwrap().unwrap();
    assert_eq!(c.copied.len(), 1, "{:?}", c.copied);
    assert_eq!(c.deleted, 0, "os plantados giraram o pendrive");
    let now = on_drive(&a);
    for n in real.iter().chain(&planted) {
        assert!(now.contains(n), "{n} sumiu do pendrive");
    }
}

/// Phase 49a (medium finding of M7): on the drive (vfat 0777: another user of the
/// machine can write) and in the local backup, files with a future-date name do not
/// make the rotation delete the real copies; the future ones neither count nor are
/// deleted; from the local backup, the ones dated past the margin do not go to the
/// drive.
#[test]
fn attack_future_dated_versions_on_the_drive_do_not_delete_the_real_ones() {
    let dir = Dir::new("ext-futuro");
    let (d, _) = test_vault(&dir);
    assert!(ok(&turn_on(&d, Role::App, UUID_A)));
    let a = mount_point(&dir, "pendrive-a");
    let source = Source::with(vec![usb_drive(UUID_A, &a)]);
    let c = d.sync_external(&source).unwrap().unwrap();
    let real = on_drive(&a);
    assert_eq!(real.len(), c.copied.len());
    let destination = external_dir(&a);
    // 30 from 2099 on the drive and one in the local backup, 10 years ahead.
    for i in 0..30u32 {
        fs::write(
            destination.join(format!("ward-20990101-{:06}.ward", i * 100)),
            b"plantada",
        )
        .unwrap();
    }
    let far = ward_core::backup::Version::from_instant(
        std::time::SystemTime::now() + Duration::from_secs(10 * 365 * 86_400),
    )
    .unwrap()
    .name();
    let local = dir.0.join("backups").join(&far);
    fs::copy(dir.0.join("backups").join(&local_versions(&dir)[0]), &local).unwrap();
    fs::set_permissions(&local, fs::Permissions::from_mode(0o600)).unwrap();
    // A new write and the copy: no real one leaves, the planted local one does not go.
    let r = request(&d, Role::App, "save_entry", &login("t-futuro", SECRETS[3]));
    assert!(ok(&r), "{r:?}");
    let c = d.sync_external(&source).unwrap().unwrap();
    assert_eq!(c.deleted, 0);
    assert!(c.copied.iter().all(|v| v.name() != far));
    let now = on_drive(&a);
    for n in &real {
        assert!(now.contains(n), "{n} sumiu do pendrive");
    }
    assert!(!now.contains(&far));
    assert_eq!(
        now.iter().filter(|n| n.starts_with("ward-2099")).count(),
        30
    );
}

/// Phase 54 (low of the final audit): junk planted in the local backup (valid header
/// with the body swapped, a version of another vault and the copy of a real old version
/// with a new name), dated newer than the real ones, does not go to the drive and does
/// not make the rotation there delete anything. A real write afterwards goes, alone.
#[test]
fn attack_junk_in_the_local_backup_does_not_rotate_the_drive() {
    let dir = Dir::new("ext-lixo");
    let (d, _) = test_vault(&dir);
    assert!(ok(&turn_on(&d, Role::App, UUID_A)));
    let a = mount_point(&dir, "pendrive-a");
    let destination = external_dir(&a);
    fs::create_dir(&destination).unwrap();
    // The drive already full: the current real ones plus old ones from 2020 up to
    // `VERSIONS`.
    let source = Source::with(vec![usb_drive(UUID_A, &a)]);
    let real = local_versions(&dir);
    for i in 0..(VERSIONS - real.len()) as u32 {
        fs::write(
            destination.join(format!("ward-20200101-{:06}.ward", i * 100)),
            b"antiga",
        )
        .unwrap();
    }
    let c = d.sync_external(&source).unwrap().unwrap();
    assert_eq!(c.copied.len(), real.len());
    assert_eq!(c.deleted, 0);
    let before = tree(&a);
    assert_eq!(on_drive(&a).len(), VERSIONS);

    // The attacker (same uid) plants in the local folder, dated newer than the real ones.
    std::thread::sleep(Duration::from_millis(1100));
    let base = ward_core::backup::Version::from_instant(std::time::SystemTime::now())
        .unwrap()
        .name();
    let name = |i: usize| base.replace(".ward", &format!("-{i}.ward"));
    let backups = dir.0.join("backups");
    let real_bytes = fs::read(backups.join(&real[0])).unwrap();
    let other = Dir::new("ext-lixo-outro");
    let _ = test_vault(&other);
    let from_other = fs::read(other.0.join("backups").join(&local_versions(&other)[0])).unwrap();
    let oldest = fs::read(backups.join(real.last().unwrap())).unwrap();
    let mut planted = Vec::new();
    for i in 1..=VERSIONS + 5 {
        let content = match i % 3 {
            0 => {
                // Valid header, body swapped.
                let mut b = real_bytes.clone();
                let n = b.len();
                b[n - 1 - i] ^= 0x5a;
                b
            }
            1 => from_other.clone(),
            _ => oldest.clone(),
        };
        assert!(ward_core::format::read_header(&content).is_ok());
        let p = backups.join(name(i));
        fs::write(&p, &content).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
        planted.push(name(i));
    }
    assert!(ward_core::backup::valid_name(&planted[0]));
    let c = d.sync_external(&source).unwrap().unwrap();
    assert!(c.copied.is_empty(), "{:?}", c.copied);
    assert_eq!(c.deleted, 0, "o lixo girou o pendrive");
    assert_eq!(tree(&a), before, "o pendrive mudou");

    // A real write: only it goes, even with the newer junk around.
    std::thread::sleep(Duration::from_millis(1100));
    let r = request(&d, Role::App, "save_entry", &login("t-lixo", SECRETS[3]));
    assert!(ok(&r), "{r:?}");
    let c = d.sync_external(&source).unwrap().unwrap();
    assert_eq!(c.copied.len(), 1, "{:?}", c.copied);
    assert!(!planted.contains(&c.copied[0].name()));
    // Phase 54c: the drive's old filler does not check: it neither counts nor
    // leaves.
    assert_eq!(c.deleted, 0);
    for n in on_drive(&a) {
        assert!(!planted.contains(&n), "{n} plantado foi para o pendrive");
    }
    for n in &real {
        assert!(on_drive(&a).contains(n), "{n} real sumiu do pendrive");
    }
}

/// Locked or turned off after being turned on: nothing is queried nor written. Turning
/// off does not delete what is already on the drive.
#[test]
fn locked_or_off_does_not_copy() {
    let dir = Dir::new("ext-travado");
    let (d, _) = test_vault(&dir);
    let a = mount_point(&dir, "pendrive-a");
    let source = Source::with(vec![usb_drive(UUID_A, &a)]);
    assert!(ok(&turn_on(&d, Role::App, UUID_A)));
    d.sync_external(&source).unwrap().unwrap();
    let before = tree(&a);
    assert!(!before.is_empty());
    let n = source.queries();
    d.lock();
    assert_eq!(d.sync_external(&source), Ok(None));
    assert_eq!(source.queries(), n);
    assert_eq!(
        error(&request(&d, Role::App, "external_backup", "{}")),
        Some(ResponseError::Locked)
    );
    assert_eq!(
        error(&request(&d, Role::App, "usb_drives", "{}")),
        Some(ResponseError::Locked)
    );
    assert!(ok(&unlock(&d, PASSWORD)));
    // The setting turned on came back with the vault (it is in the encrypted part).
    assert!(d.sync_external(&source).unwrap().is_some());
    let r = request(
        &d,
        Role::App,
        "set_external_backup",
        &format!(r#"{{"enabled":false,"uuid":"{UUID_A}"}}"#),
    );
    assert!(ok(&r), "{r:?}");
    let r = request(&d, Role::App, "save_entry", &login("t-depois", SECRETS[3]));
    assert!(ok(&r));
    let n = source.queries();
    assert_eq!(d.sync_external(&source), Ok(None));
    assert_eq!(source.queries(), n);
    assert_eq!(tree(&a), before, "nada novo, nada apagado");
    match request(&d, Role::App, "external_backup", "{}") {
        Response::ExternalBackup { enabled, uuid } => {
            assert!(!enabled);
            assert_eq!(uuid.as_deref(), Some(UUID_A));
        }
        r => panic!("{r:?}"),
    }
}

/// V7.8: the agent and the extension asking to read, turn on or list USB drives get
/// "not authorized", directly in the daemon (the second layer) and through the protocol
/// read; the vault stays the same byte for byte and the external backup, off.
#[test]
fn v7_8_agent_and_extension_neither_turn_on_nor_read() {
    let dir = Dir::new("ext-papel");
    let (d, _) = test_vault(&dir);
    let a = mount_point(&dir, "pendrive-a");
    let source = Source::with(vec![usb_drive(UUID_A, &a)]);
    let d = d.with_usb_drives(Arc::new(source.clone()));
    let vault = fs::read(&dir.paths().vault).unwrap();
    for role in [Role::Agent, Role::Extension] {
        for (op, data) in [
            ("external_backup", "{}".to_owned()),
            ("usb_drives", "{}".to_owned()),
            (
                "set_external_backup",
                format!(r#"{{"enabled":true,"uuid":"{UUID_A}"}}"#),
            ),
        ] {
            // Through the protocol read.
            assert_eq!(
                error(&request(&d, role, op, &data)),
                Some(ResponseError::Unauthorized),
                "{op} pelo {}",
                role.name()
            );
            // Directly in the daemon, with the request read as the app.
            let p = req(Role::App, op, &data).unwrap();
            assert_eq!(error(&d.serve(role, p)), Some(ResponseError::Unauthorized));
        }
    }
    assert_eq!(fs::read(&dir.paths().vault).unwrap(), vault);
    assert_eq!(source.queries(), 0);
    assert_eq!(d.sync_external(&source), Ok(None));
    assert!(tree(&a).is_empty());
}

/// Without udisks2 (test daemon without a bus), the USB drive list answers that the
/// drives are unavailable; a UUID out of format is refused without writing.
#[test]
fn usb_drives_without_udisks_and_invalid_uuid() {
    let dir = Dir::new("ext-sem");
    let (d, _) = test_vault(&dir);
    assert_eq!(
        error(&request(&d, Role::App, "usb_drives", "{}")),
        Some(ResponseError::UsbDrivesUnavailable)
    );
    let vault = fs::read(&dir.paths().vault).unwrap();
    for uuid in ["../x", "ABCD 1234", "-AB", ""] {
        assert_eq!(
            error(&turn_on(&d, Role::App, uuid)),
            Some(ResponseError::InvalidRequest),
            "{uuid}"
        );
    }
    assert_eq!(
        error(&request(
            &d,
            Role::App,
            "set_external_backup",
            r#"{"enabled":true}"#
        )),
        Some(ResponseError::InvalidRequest)
    );
    assert_eq!(fs::read(&dir.paths().vault).unwrap(), vault);
}

// ---------------------------------------------------------------- fake udisks2

static N: AtomicU32 = AtomicU32::new(0);

/// Private `dbus-daemon`, only for this test (killed on `Drop`).
struct FakeBus {
    dir: PathBuf,
    socket: PathBuf,
    child: Option<Child>,
}

impl FakeBus {
    fn start() -> FakeBus {
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| {
                PathBuf::from(format!("/run/user/{}", ward_platform::effective_uid()))
            });
        let dir = base.join(format!(
            "wardd-udisks-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        let socket = dir.join("bus");
        let config = dir.join("bus.conf");
        fs::write(
            &config,
            format!(
                r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <listen>unix:path={}</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#,
                socket.display()
            ),
        )
        .unwrap();
        let child = Command::new("/usr/bin/dbus-daemon")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--nopidfile", "--nosyslog"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("dbus-daemon");
        let end = Instant::now() + Duration::from_secs(10);
        while UnixStream::connect(&socket).is_err() {
            assert!(Instant::now() < end, "o dbus-daemon falso não subiu");
            std::thread::sleep(Duration::from_millis(20));
        }
        FakeBus {
            dir,
            socket,
            child: Some(child),
        }
    }

    fn address(&self) -> String {
        format!("unix:path={}", self.socket.display())
    }
}

impl Drop for FakeBus {
    fn drop(&mut self) {
        if let Some(mut f) = self.child.take() {
            let _ = f.kill();
            let _ = f.wait();
        }
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// An object of the fake udisks2.
#[derive(Clone)]
struct Object {
    path: &'static str,
    uuid: String,
    label: String,
    device: u64,
    mount: Option<Vec<u8>>,
    system: bool,
    is_loop: bool,
}

type Objects = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedVariant>>>;

fn ov<'a>(v: impl Into<Variant<'a>>) -> OwnedVariant {
    OwnedVariant::try_from(v.into()).unwrap()
}

fn objects(list: &[Object]) -> Objects {
    let mut m = Objects::new();
    for o in list {
        let mut block = HashMap::new();
        block.insert("IdUsage".into(), ov("filesystem"));
        block.insert("IdUUID".into(), ov(o.uuid.as_str()));
        block.insert("IdLabel".into(), ov(o.label.as_str()));
        block.insert("DeviceNumber".into(), ov(o.device));
        block.insert("HintIgnore".into(), ov(false));
        block.insert("HintSystem".into(), ov(o.system));
        let mut fs_ = HashMap::new();
        let points: Vec<Vec<u8>> = o.mount.iter().cloned().collect();
        fs_.insert("MountPoints".into(), ov(points));
        let mut ifaces = HashMap::new();
        ifaces.insert("org.freedesktop.UDisks2.Block".to_owned(), block);
        ifaces.insert("org.freedesktop.UDisks2.Filesystem".to_owned(), fs_);
        if o.is_loop {
            ifaces.insert("org.freedesktop.UDisks2.Loop".to_owned(), HashMap::new());
        }
        m.insert(OwnedObjectPath::try_from(o.path).unwrap(), ifaces);
    }
    m
}

struct Manager {
    list: Arc<Mutex<Vec<Object>>>,
}

#[zbus::interface(name = "org.freedesktop.DBus.ObjectManager")]
impl Manager {
    fn get_managed_objects(&self) -> Objects {
        objects(&self.list.lock().unwrap())
    }
}

/// The fake udisks2: owner of `org.freedesktop.UDisks2` on the fake bus.
struct FakeUdisks {
    conn: Connection,
    list: Arc<Mutex<Vec<Object>>>,
}

impl FakeUdisks {
    fn new(b: &FakeBus, list: Vec<Object>) -> FakeUdisks {
        let list = Arc::new(Mutex::new(list));
        let conn = zbus::block_on(
            Builder::address(b.address().as_str())
                .name("org.freedesktop.UDisks2")
                .serve_at("/org/freedesktop/UDisks2", Manager { list: list.clone() })
                .build(),
        )
        .unwrap();
        FakeUdisks { conn, list }
    }

    /// `PropertiesChanged` of `MountPoints` (what udisks2 sends on mount).
    fn mounted(&self, path: &str) {
        let changed: HashMap<&str, Variant<'_>> = HashMap::new();
        zbus::block_on(self.conn.emit_signal(
            None::<&str>,
            path,
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
            &(
                "org.freedesktop.UDisks2.Filesystem",
                changed,
                vec!["MountPoints"],
            ),
        ))
        .unwrap();
    }
}

fn object(path: &'static str, uuid: &str, dir: Option<&Path>) -> Object {
    Object {
        path,
        uuid: uuid.into(),
        label: "PEN".into(),
        device: dir.map_or(0, |d| fs::metadata(d).unwrap().dev()),
        mount: dir.map(|d| {
            let mut b = d.as_os_str().as_encoded_bytes().to_vec();
            b.push(0);
            b
        }),
        system: false,
        is_loop: false,
    }
}

/// What comes out of `GetManagedObjects`: only a USB drive (not system) or loop, UUID
/// in format, label without control characters, mount checked.
#[test]
fn fake_udisks_is_read_with_the_checks() {
    let dir = Dir::new("ext-udisks-ler");
    let a = mount_point(&dir, "pendrive-a");
    let b = FakeBus::start();
    let mut system = object(
        "/org/freedesktop/UDisks2/block_devices/nvme0n1p2",
        "1111-2222",
        Some(&a),
    );
    system.system = true;
    let mut loop_dev = object("/org/freedesktop/UDisks2/block_devices/loop7", UUID_B, None);
    loop_dev.system = true;
    loop_dev.is_loop = true;
    let mut dirty = object(
        "/org/freedesktop/UDisks2/block_devices/sdc1",
        UUID_A,
        Some(&a),
    );
    dirty.label = "PEN\u{1b}[2J".into();
    let bad = object(
        "/org/freedesktop/UDisks2/block_devices/sdd1",
        "../etc",
        Some(&a),
    );
    let mut strange = object(
        "/org/freedesktop/UDisks2/block_devices/sde1",
        "AAAA-BBBB",
        None,
    );
    strange.mount = Some(b"/media/u/../../etc\0".to_vec());
    let _u = FakeUdisks::new(&b, vec![system, loop_dev, dirty, bad, strange]);
    let source = wardd::udisks::Udisks::new(b.socket.clone());
    let seen = source.usb_drives().expect("udisks2 falso respondeu");
    let uuids: Vec<&str> = seen.iter().map(|v| v.uuid.as_str()).collect();
    assert_eq!(uuids, ["0F3C-9A51", "AAAA-BBBB", "ABCD-1234"]);
    let pa = seen.iter().find(|v| v.uuid == UUID_A).unwrap();
    assert_eq!(pa.label, "PEN[2J");
    assert_eq!(pa.mount.as_deref(), Some(a.as_path()));
    assert_eq!(pa.device, fs::metadata(&a).unwrap().dev());
    assert!(
        seen.iter()
            .all(|v| v.uuid != "AAAA-BBBB" || v.mount.is_none())
    );
    // A bus without udisks2: `None`.
    let b2 = FakeBus::start();
    assert_eq!(
        wardd::udisks::Udisks::new(b2.socket.clone()).usb_drives(),
        None
    );
}

/// End to end with the daemon threads: on with A's UUID, nothing while it is not
/// mounted; the udisks2 mount signal makes the copy reach A, and B mounted at the same
/// time gets nothing.
#[test]
fn v7_5_udisks_signal_copies_to_the_mounted_drive() {
    let dir = Dir::new("ext-udisks-sinal");
    let (d, _) = test_vault(&dir);
    let a = mount_point(&dir, "pendrive-a");
    let bdir = mount_point(&dir, "pendrive-b");
    let b = FakeBus::start();
    let u = FakeUdisks::new(
        &b,
        vec![
            object("/org/freedesktop/UDisks2/block_devices/sdb1", UUID_A, None),
            object(
                "/org/freedesktop/UDisks2/block_devices/sdc1",
                UUID_B,
                Some(&bdir),
            ),
        ],
    );
    let source = Arc::new(wardd::udisks::Udisks::new(b.socket.clone()));
    let d = Arc::new(d.with_usb_drives(source.clone()));
    assert!(ok(&turn_on(&d, Role::App, UUID_A)));
    let nudges = d.enable_external_copy().unwrap();
    assert!(d.enable_external_copy().is_none(), "só uma thread de cópia");
    wardd::udisks::watch_copy(d.clone(), source, nudges).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    wardd::udisks::watch_udisks(
        d.clone(),
        b.socket.clone(),
        Box::new(move || {
            let _ = tx.send(());
        }),
    )
    .unwrap();
    rx.recv_timeout(Duration::from_secs(10))
        .expect("vigia conectou");
    match request(&d, Role::App, "usb_drives", "{}") {
        Response::UsbDrives(l) => {
            assert_eq!(l.len(), 2);
            assert!(!l.iter().find(|p| p.uuid == UUID_A).unwrap().mounted);
        }
        r => panic!("{r:?}"),
    }
    std::thread::sleep(wardd::udisks::BURST_WAIT * 3);
    assert!(tree(&a).is_empty() && tree(&bdir).is_empty());
    // A is mounted.
    u.list.lock().unwrap()[0] = object(
        "/org/freedesktop/UDisks2/block_devices/sdb1",
        UUID_A,
        Some(&a),
    );
    u.mounted("/org/freedesktop/UDisks2/block_devices/sdb1");
    let end = Instant::now() + Duration::from_secs(15);
    let total = local_versions(&dir).len();
    while on_drive(&a).len() < total {
        assert!(Instant::now() < end, "a cópia não chegou ao pendrive A");
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(tree(&bdir).is_empty(), "o B não recebe nada");
    // A write nudges the copy again.
    let r = request(&d, Role::App, "save_entry", &login("t-sinal", SECRETS[3]));
    assert!(ok(&r));
    let end = Instant::now() + Duration::from_secs(15);
    while on_drive(&a).len() < total + 1 {
        assert!(
            Instant::now() < end,
            "a gravação nova não chegou ao pendrive"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// No `link(2)` on the drive's path (vfat has none): the copy publishes only through
/// `renameat2(RENAME_NOREPLACE)`.
#[test]
fn external_does_not_use_link() {
    let source = include_str!("../src/external.rs");
    for forbidden in ["hard_link", "linkat", "libc::link", "fs::symlink("] {
        assert!(!source.contains(forbidden), "{forbidden} em external.rs");
    }
    assert!(source.contains("rename_no_replace(destination.as_fd()"));
}
