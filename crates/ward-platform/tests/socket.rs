//! Directory and socket of the daemon (V3.1, V3.2; S5). The attacks: 0755 directory,
//! directory of another owner, symbolic link in place of the directory, file in place
//! of the socket, second daemon on the same socket.

mod common;

use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::Path;

use common::TestDir;
use ward_platform::socket::{DirError, ListenError, prepare_dir};

fn mode(p: &Path) -> u32 {
    std::fs::symlink_metadata(p).unwrap().mode() & 0o7777
}

#[test]
fn new_dir_ends_up_0700_and_socket_0600_of_the_user() {
    let d = TestDir::new();
    let dir = prepare_dir(&d.path).unwrap();
    assert_eq!(mode(&d.path), 0o700);
    let listener = dir.listen("wardd.sock").unwrap();
    let m = std::fs::symlink_metadata(listener.path()).unwrap();
    assert!(m.file_type().is_socket());
    assert_eq!(m.mode() & 0o7777, 0o600);
    assert_eq!(m.uid(), ward_platform::effective_uid());
    assert_eq!(
        std::fs::symlink_metadata(&d.path).unwrap().uid(),
        ward_platform::effective_uid()
    );
    // Preparing again a 0700 of ours is accepted.
    prepare_dir(&d.path).unwrap();
}

#[test]
fn attack_existing_dir_0755_neither_starts_nor_fixes() {
    let d = TestDir::new();
    std::fs::create_dir(&d.path).unwrap();
    std::fs::set_permissions(&d.path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let r = prepare_dir(&d.path);
    assert!(
        matches!(r, Err(DirError::WrongMode { mode: 0o755 })),
        "{r:?}"
    );
    // Did not touch the mode nor create a socket.
    assert_eq!(mode(&d.path), 0o755);
    assert_eq!(std::fs::read_dir(&d.path).unwrap().count(), 0);

    for m in [0o750, 0o701, 0o777, 0o500] {
        std::fs::set_permissions(&d.path, std::fs::Permissions::from_mode(m)).unwrap();
        assert!(
            matches!(prepare_dir(&d.path), Err(DirError::WrongMode { .. })),
            "{m:o}"
        );
    }
    std::fs::set_permissions(&d.path, std::fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn attack_dir_of_another_owner_does_not_start() {
    // `/run/user` is root's (0755): it serves as "directory of another owner" without
    // needing root to set up the scenario. Nothing is created nor changed in it.
    let r = prepare_dir(Path::new("/run/user"));
    assert!(matches!(r, Err(DirError::WrongOwner { uid: 0 })), "{r:?}");
}

#[test]
fn attack_symbolic_link_in_place_of_the_dir() {
    let target = TestDir::new();
    std::fs::create_dir(&target.path).unwrap();
    std::fs::set_permissions(&target.path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let link = TestDir::new();
    std::os::unix::fs::symlink(&target.path, &link.path).unwrap();
    let r = prepare_dir(&link.path);
    assert!(matches!(r, Err(DirError::NotDirectory)), "{r:?}");
    // `remove_dir_all` on the link deletes only the link.
    std::fs::remove_file(&link.path).unwrap();
}

#[test]
fn attack_file_in_place_of_the_dir() {
    let d = TestDir::new();
    std::fs::write(&d.path, b"").unwrap();
    let r = prepare_dir(&d.path);
    assert!(matches!(r, Err(DirError::NotDirectory)), "{r:?}");
    std::fs::remove_file(&d.path).unwrap();
}

#[test]
fn attack_dir_above_writable_by_group() {
    let parent = TestDir::new();
    std::fs::create_dir(&parent.path).unwrap();
    std::fs::set_permissions(&parent.path, std::fs::Permissions::from_mode(0o770)).unwrap();
    let r = prepare_dir(&parent.path.join("ward"));
    assert!(matches!(r, Err(DirError::UntrustedParent(_))), "{r:?}");
    assert!(!parent.path.join("ward").exists());
}

#[test]
fn attack_file_or_link_in_place_of_the_socket() {
    let d = TestDir::new();
    let dir = prepare_dir(&d.path).unwrap();
    std::fs::write(d.path.join("s.sock"), b"isca").unwrap();
    assert!(matches!(dir.listen("s.sock"), Err(ListenError::PathTaken)));
    // The decoy file is still there: nothing is deleted without being a socket of ours.
    assert_eq!(std::fs::read(d.path.join("s.sock")).unwrap(), b"isca");

    std::os::unix::fs::symlink("/run/user", d.path.join("l.sock")).unwrap();
    assert!(matches!(dir.listen("l.sock"), Err(ListenError::PathTaken)));

    for bad in ["", ".", "..", "a/b", "../fora.sock"] {
        assert!(
            matches!(dir.listen(bad), Err(ListenError::InvalidName)),
            "{bad}"
        );
    }
}

#[test]
fn second_daemon_on_the_same_socket_refuses_and_stale_socket_is_replaced() {
    let d = TestDir::new();
    let dir = prepare_dir(&d.path).unwrap();
    let first = dir.listen("wardd.sock").unwrap();
    assert!(matches!(
        dir.listen("wardd.sock"),
        Err(ListenError::AlreadyInUse)
    ));

    // Stale socket, with nobody listening: `std` does not delete it when the listener
    // drops; the `Listener` does. Simulates a daemon that died without deleting it.
    let path = first.path().to_path_buf();
    let stale = std::os::unix::net::UnixListener::bind(d.path.join("velho.sock")).unwrap();
    drop(stale);
    assert!(d.path.join("velho.sock").exists());
    let new = dir.listen("velho.sock").unwrap();
    assert!(UnixStream::connect(new.path()).is_ok());

    drop(first);
    assert!(!path.exists(), "o Listener apaga o socket ao sair");
}

#[test]
fn dir_swapped_after_prepare_refuses_to_listen() {
    let d = TestDir::new();
    let dir = prepare_dir(&d.path).unwrap();
    // Swaps the directory for another one (also 0700 and ours) with the same name.
    let other = TestDir::new();
    std::fs::rename(&d.path, &other.path).unwrap();
    std::fs::create_dir(&d.path).unwrap();
    std::fs::set_permissions(&d.path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let r = dir.listen("wardd.sock");
    assert!(
        matches!(r, Err(ListenError::Dir(DirError::Swapped))),
        "{r:?}"
    );
}

#[test]
fn accept_carries_pid_and_uid_of_the_peer() {
    let d = TestDir::new();
    let dir = prepare_dir(&d.path).unwrap();
    let listener = dir.listen("wardd.sock").unwrap();
    let _client = UnixStream::connect(listener.path()).unwrap();
    let connection = listener.accept().unwrap();
    let cred = connection.peer.credentials();
    assert_eq!(cred.pid, std::process::id());
    assert_eq!(cred.uid, ward_platform::effective_uid());
    // The test binary itself (in target/, the user's) is not an installed binary.
    let me = std::env::current_exe().unwrap();
    assert!(connection.peer.identify(&common::allowed(&me)).is_err());
}
