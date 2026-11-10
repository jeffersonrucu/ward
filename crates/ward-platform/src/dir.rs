//! Rename without overwriting inside an open directory (Phase 46) and delete what is in
//! an open directory through the fd, without following links (Phase 44e).
//!
//! The external backup goes to a USB drive, almost always vfat, which has no `link(2)`:
//! the "temporary → `link` to the final name" of the local backup (Phase 45) does not
//! work there. [`rename_no_replace`] uses `renameat2(RENAME_NOREPLACE)` relative to the
//! fd of the directory: the final name only shows up whole, and a version that already
//! exists is never replaced (`AlreadyExists`).

use std::ffi::{CString, OsStr};
use std::io;
use std::os::fd::BorrowedFd;
use std::os::unix::ffi::OsStrExt;

use crate::sys;

/// Plain name (no `/`, no NUL, not `.`/`..`) for a `*at` call.
fn c_name(name: &OsStr) -> io::Result<CString> {
    let b = name.as_bytes();
    if b.is_empty() || b == b"." || b == b".." || b.contains(&b'/') {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    CString::new(b).map_err(|_| io::ErrorKind::InvalidInput.into())
}

/// Renames `from` to `to`, both inside the directory `dir`, without overwriting: if
/// `to` exists (also a symbolic link, which is not followed), `AlreadyExists` and
/// nothing changes. Names with `/`, NUL, `.` or `..` are refused (`InvalidInput`): the
/// operation never leaves the directory.
pub fn rename_no_replace(dir: BorrowedFd<'_>, from: &OsStr, to: &OsStr) -> io::Result<()> {
    let (from, to) = (c_name(from)?, c_name(to)?);
    sys::renameat_no_replace(dir, &from, &to)
}

/// Deeper than this, [`delete_contents`] stops (Ward's directories have one level).
const MAX_DEPTH: usize = 16;

/// Deletes everything inside the directory open in `dir`, through the fd (`readdir` on
/// the fd, `unlinkat` relative to it): nothing is resolved by the path again, so
/// swapping the directory for a link after it was opened does not lead to deleting
/// another one. A link inside it is deleted as it is (the target stays); a
/// subdirectory is opened with `O_DIRECTORY | O_NOFOLLOW` relative to the fd and emptied
/// the same way. Returns how many first-level items went away; what fails stays (and
/// does not stop the others).
pub fn delete_contents(dir: BorrowedFd<'_>) -> usize {
    delete_contents_at(dir, 0)
}

fn delete_contents_at(dir: BorrowedFd<'_>, depth: usize) -> usize {
    let Ok(names) = sys::list_dir(dir) else {
        return 0;
    };
    names
        .iter()
        .filter(|name| delete_item(dir, name, depth).is_ok())
        .count()
}

fn delete_item(dir: BorrowedFd<'_>, name: &std::ffi::CStr, depth: usize) -> io::Result<()> {
    match sys::unlinkat(dir, name, 0) {
        Err(e) if e.raw_os_error() == Some(libc::EISDIR) => {}
        r => return r,
    }
    if depth >= MAX_DEPTH {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let sub = sys::openat(
        dir,
        name,
        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW,
    )?;
    use std::os::fd::AsFd;
    delete_contents_at(sub.as_fd(), depth + 1);
    sys::unlinkat(dir, name, libc::AT_REMOVEDIR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::fd::AsFd;
    use std::path::PathBuf;

    fn dir(label: &str) -> PathBuf {
        let d = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/teste-plataforma")
            .join(format!("pasta-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn renames_and_never_overwrites() {
        let d = dir("renomear");
        fs::write(d.join("tmp"), b"novo").unwrap();
        fs::write(d.join("versao"), b"antiga").unwrap();
        let f = fs::File::open(&d).unwrap();
        let e = rename_no_replace(f.as_fd(), "tmp".as_ref(), "versao".as_ref()).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(d.join("versao")).unwrap(), b"antiga");
        assert_eq!(fs::read(d.join("tmp")).unwrap(), b"novo");
        rename_no_replace(f.as_fd(), "tmp".as_ref(), "outra".as_ref()).unwrap();
        assert_eq!(fs::read(d.join("outra")).unwrap(), b"novo");
        assert!(!d.join("tmp").exists());
        // A symbolic link at the final name also "exists" and is not followed.
        std::os::unix::fs::symlink("/nao/existe", d.join("link")).unwrap();
        fs::write(d.join("tmp2"), b"x").unwrap();
        let e = rename_no_replace(f.as_fd(), "tmp2".as_ref(), "link".as_ref()).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::AlreadyExists);
        assert!(fs::symlink_metadata(d.join("link")).unwrap().is_symlink());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn attack_name_that_leaves_the_dir_refused() {
        let d = dir("fora");
        fs::write(d.join("tmp"), b"x").unwrap();
        let f = fs::File::open(&d).unwrap();
        for bad in ["../fora", "a/b", "..", ".", "", "a\0b"] {
            let e = rename_no_replace(f.as_fd(), "tmp".as_ref(), bad.as_ref()).unwrap_err();
            assert_eq!(e.kind(), io::ErrorKind::InvalidInput, "{bad:?}");
            let e = rename_no_replace(f.as_fd(), bad.as_ref(), "x".as_ref()).unwrap_err();
            assert_eq!(e.kind(), io::ErrorKind::InvalidInput, "{bad:?}");
        }
        assert!(d.join("tmp").exists());
        assert!(!d.join("../fora").exists());
        fs::remove_dir_all(&d).unwrap();
    }

    /// Phase 44e: the contents go away through the fd; a link inside the directory is
    /// deleted without following, and the subdirectory too.
    #[test]
    fn deletes_the_contents_through_the_fd_without_following_links() {
        let d = dir("apagar");
        let target = d.join("alvo");
        fs::create_dir_all(target.join("sub")).unwrap();
        fs::write(target.join("precioso.txt"), b"x").unwrap();
        let p = d.join("pasta");
        fs::create_dir_all(p.join("sub/mais")).unwrap();
        fs::write(p.join("a.json"), b"{}").unwrap();
        fs::write(p.join("sub/b.json"), b"{}").unwrap();
        fs::write(p.join("sub/mais/c.json"), b"{}").unwrap();
        std::os::unix::fs::symlink(&target, p.join("ligacao")).unwrap();
        std::os::unix::fs::symlink(&target, p.join("sub/ligacao")).unwrap();
        let f = fs::File::open(&p).unwrap();
        assert_eq!(delete_contents(f.as_fd()), 3);
        assert_eq!(fs::read_dir(&p).unwrap().count(), 0);
        assert!(target.join("precioso.txt").exists());
        assert!(target.join("sub").is_dir());
        // Again, empty: nothing to delete.
        assert_eq!(delete_contents(f.as_fd()), 0);
        fs::remove_dir_all(&d).unwrap();
    }

    /// Attack (Phase 44e): the directory is opened and then swapped for a link to
    /// another; the fd stays on the original (already moved away), and the other one
    /// stays intact.
    #[test]
    fn attack_dir_swapped_for_a_link_after_being_opened() {
        let d = dir("trocada");
        let target = d.join("alvo");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("precioso.txt"), b"x").unwrap();
        let p = d.join("pasta");
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("a.json"), b"{}").unwrap();
        let f = fs::File::open(&p).unwrap();
        fs::rename(&p, d.join("movida")).unwrap();
        std::os::unix::fs::symlink(&target, &p).unwrap();
        assert_eq!(delete_contents(f.as_fd()), 1);
        assert!(target.join("precioso.txt").exists());
        assert!(!d.join("movida/a.json").exists());
        fs::remove_dir_all(&d).unwrap();
    }
}
