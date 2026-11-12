//! Direct calls to the kernel through `libc`. Together with `memory`, it is the only place
//! in Ward with `unsafe`: each block carries `// SAFETY:` and each function is covered by
//! the crate tests. The rest of the crate only uses these functions, which already return
//! safe types (`OwnedFd`, `io::Result`).

use std::ffi::CStr;
use std::io;
use std::mem::size_of;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::ptr::NonNull;

/// `SO_PEERPIDFD` (Linux 6.5): pidfd of the process on the other side of the socket. The
/// value is the one from `asm-generic/socket.h` (x86_64 and aarch64, the Ward targets);
/// `libc` 0.2.191 does not export it for glibc.
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
const SO_PEERPIDFD: libc::c_int = 77;

fn os_error() -> io::Error {
    io::Error::last_os_error()
}

/// `SO_PEERCRED` of the socket: pid, uid and gid of the process on the other side at the
/// time of the `connect` (or of the `listen`, as seen by the client).
#[allow(unsafe_code)]
pub(crate) fn peercred(fd: BorrowedFd<'_>) -> io::Result<libc::ucred> {
    let mut cred = libc::ucred {
        pid: 0,
        uid: u32::MAX,
        gid: u32::MAX,
    };
    let size = size_of::<libc::ucred>();
    let mut len = libc::socklen_t::try_from(size).map_err(|_| io::ErrorKind::InvalidInput)?;
    // SAFETY: `fd` is an open descriptor (the `BorrowedFd` guarantees it for the length
    // of the borrow); `cred` and `len` are locals, `len` is exactly the size of `ucred`,
    // and the kernel writes at most `len` bytes into `cred`.
    let r = unsafe {
        libc::getsockopt(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&raw mut cred).cast(),
            &raw mut len,
        )
    };
    if r != 0 {
        return Err(os_error());
    }
    if usize::try_from(len).ok() != Some(size) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(cred)
}

/// `SO_PEERPIDFD`: a new pidfd of the process that did the `connect`. Unlike the pid, the
/// pidfd does not start pointing to another process if the number is reused.
#[allow(unsafe_code)]
pub(crate) fn peer_pidfd(fd: BorrowedFd<'_>) -> io::Result<OwnedFd> {
    let mut pidfd: libc::c_int = -1;
    let size = size_of::<libc::c_int>();
    let mut len = libc::socklen_t::try_from(size).map_err(|_| io::ErrorKind::InvalidInput)?;
    // SAFETY: `fd` is open through the borrow; `pidfd` and `len` are locals and `len` has
    // the size of a `c_int`, which is what the kernel writes for `SO_PEERPIDFD`.
    let r = unsafe {
        libc::getsockopt(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            SO_PEERPIDFD,
            (&raw mut pidfd).cast(),
            &raw mut len,
        )
    };
    if r != 0 {
        return Err(os_error());
    }
    if usize::try_from(len).ok() != Some(size) || pidfd < 0 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    // SAFETY: `getsockopt` has just installed `pidfd` in the table of this process, and
    // no other code knows it; the `OwnedFd` becomes the only owner and closes it.
    Ok(unsafe { OwnedFd::from_raw_fd(pidfd) })
}

/// `pidfd_open(pid, 0)`.
#[allow(unsafe_code)]
pub(crate) fn pidfd_open(pid: libc::pid_t) -> io::Result<OwnedFd> {
    if pid <= 0 {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    // SAFETY: syscall without pointers: only an integer (pid) and flags 0.
    let r = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0 as libc::c_uint) };
    if r < 0 {
        return Err(os_error());
    }
    let fd = libc::c_int::try_from(r).map_err(|_| io::ErrorKind::InvalidData)?;
    // SAFETY: the kernel has just created `fd` for this process and nobody else knows it.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// Checks whether the process of the pidfd still exists (signal 0 through
/// `pidfd_send_signal`, which delivers nothing). `Ok(false)` when it has already exited.
#[allow(unsafe_code)]
pub(crate) fn pidfd_alive(pidfd: BorrowedFd<'_>) -> io::Result<bool> {
    // SAFETY: `pidfd` is open through the borrow; a null `info` is allowed and the flags
    // are 0; signal 0 only tests existence and permission.
    let r = unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            pidfd.as_raw_fd(),
            0 as libc::c_int,
            std::ptr::null::<libc::siginfo_t>(),
            0 as libc::c_uint,
        )
    };
    if r == 0 {
        return Ok(true);
    }
    let e = os_error();
    if e.raw_os_error() == Some(libc::ESRCH) {
        Ok(false)
    } else {
        Err(e)
    }
}

/// `renameat2(dir, from, dir, to, RENAME_NOREPLACE)`: renames inside `dir` without ever
/// overwriting (`EEXIST` if `to` exists). Works on vfat, which has no `link(2)`
/// (Phase 46).
#[allow(unsafe_code)]
pub(crate) fn renameat_no_replace(dir: BorrowedFd<'_>, from: &CStr, to: &CStr) -> io::Result<()> {
    // SAFETY: `dir` is open through the borrow; `from` and `to` are valid C strings until
    // the end of the call, and the kernel only reads them.
    let r = unsafe {
        libc::renameat2(
            dir.as_raw_fd(),
            from.as_ptr(),
            dir.as_raw_fd(),
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if r != 0 {
        return Err(os_error());
    }
    Ok(())
}

/// `openat(dir, name, flags | O_CLOEXEC)`. `name` is relative to `dir`.
#[allow(unsafe_code)]
pub(crate) fn openat(dir: BorrowedFd<'_>, name: &CStr, flags: libc::c_int) -> io::Result<OwnedFd> {
    // SAFETY: `dir` is open through the borrow and `name` is a valid C string until the
    // end of the call; the mode only matters with O_CREAT, which the caller does not pass.
    let fd = unsafe { libc::openat(dir.as_raw_fd(), name.as_ptr(), flags | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(os_error());
    }
    // SAFETY: new descriptor, created now by `openat` and without another owner.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// `unlinkat(dir, name, flags)`: deletes `name` inside `dir` (with `AT_REMOVEDIR`, an
/// empty directory). Never follows a link: a symbolic link is deleted as it is.
#[allow(unsafe_code)]
pub(crate) fn unlinkat(dir: BorrowedFd<'_>, name: &CStr, flags: libc::c_int) -> io::Result<()> {
    // SAFETY: `dir` is open through the borrow and `name` is a valid C string until the
    // end of the call; the kernel only reads it.
    let r = unsafe { libc::unlinkat(dir.as_raw_fd(), name.as_ptr(), flags) };
    if r != 0 {
        return Err(os_error());
    }
    Ok(())
}

/// The names inside the directory open in `dir` (without `.` and `..`), read through the
/// fd itself (`fdopendir` on an `F_DUPFD_CLOEXEC` copy, `readdir` and `closedir`): no path
/// is resolved again. The copy shares the read position with `dir`; that is why it goes
/// back to the start (`rewinddir`) before reading.
#[allow(unsafe_code)]
pub(crate) fn list_dir(dir: BorrowedFd<'_>) -> io::Result<Vec<std::ffi::CString>> {
    // SAFETY: `dir` is open through the borrow; `F_DUPFD_CLOEXEC` only creates another fd.
    let copy = unsafe { libc::fcntl(dir.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
    if copy < 0 {
        return Err(os_error());
    }
    // SAFETY: `copy` is a new fd, without another owner; `fdopendir` becomes its owner
    // (and the `closedir` below closes it). If it fails, the fd is still ours and closed.
    let d = unsafe { libc::fdopendir(copy) };
    if d.is_null() {
        let e = os_error();
        // SAFETY: `fdopendir` failed, so `copy` is still only ours.
        unsafe { libc::close(copy) };
        return Err(e);
    }
    // SAFETY: `d` is the valid `DIR*` returned above, used only in this thread.
    unsafe { libc::rewinddir(d) };
    let mut names = Vec::new();
    let result = loop {
        // SAFETY: zeroes the `errno` of this thread to tell the end of the list from an
        // error.
        unsafe { *libc::__errno_location() = 0 };
        // SAFETY: `d` is valid until the `closedir`; the returned pointer is valid until
        // the next `readdir` call on the same `DIR*`, and the name is copied before it.
        let entry = unsafe { libc::readdir(d) };
        if entry.is_null() {
            let e = io::Error::last_os_error();
            break if e.raw_os_error().unwrap_or(0) == 0 {
                Ok(())
            } else {
                Err(e)
            };
        }
        // SAFETY: `entry` points to a valid `dirent`, and `d_name` ends in NUL.
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
        let b = name.to_bytes();
        if b != b"." && b != b".." {
            names.push(name.to_owned());
        }
    };
    // SAFETY: `d` is valid and not used afterwards; it also closes the fd copy.
    unsafe { libc::closedir(d) };
    result.map(|()| names)
}

/// `readlinkat(dir, name)`, with at most `PATH_MAX` bytes. A truncated result is an error.
#[allow(unsafe_code)]
pub(crate) fn readlinkat(dir: BorrowedFd<'_>, name: &CStr) -> io::Result<Vec<u8>> {
    let capacity = usize::try_from(libc::PATH_MAX).map_err(|_| io::ErrorKind::InvalidData)?;
    let mut buf = vec![0u8; capacity + 1];
    // SAFETY: `dir` open, `name` a valid C string, and `buf` has `buf.len()` writable
    // bytes; the kernel writes at most that much and adds no NUL.
    let r = unsafe {
        libc::readlinkat(
            dir.as_raw_fd(),
            name.as_ptr(),
            buf.as_mut_ptr().cast(),
            buf.len(),
        )
    };
    if r < 0 {
        return Err(os_error());
    }
    let n = usize::try_from(r).map_err(|_| io::ErrorKind::InvalidData)?;
    if n > capacity {
        return Err(io::ErrorKind::InvalidData.into());
    }
    buf.truncate(n);
    Ok(buf)
}

#[allow(unsafe_code)]
pub(crate) fn geteuid() -> u32 {
    // SAFETY: no arguments and no side effect; never fails.
    unsafe { libc::geteuid() }
}

/// `prctl(PR_SET_DUMPABLE, 0)` and reading it back through `PR_GET_DUMPABLE`.
#[allow(unsafe_code)]
pub(crate) fn disable_dumpable() -> io::Result<()> {
    let zero: libc::c_ulong = 0;
    // SAFETY: `PR_SET_DUMPABLE` only reads the integers passed; the unused ones are 0.
    let r = unsafe { libc::prctl(libc::PR_SET_DUMPABLE, zero, zero, zero, zero) };
    if r != 0 {
        return Err(os_error());
    }
    if dumpable()? {
        return Err(io::Error::other("PR_SET_DUMPABLE não ficou em 0"));
    }
    Ok(())
}

/// `prctl(PR_GET_DUMPABLE)`: `true` when the process can be debugged or dump core.
pub(crate) fn dumpable() -> io::Result<bool> {
    Ok(dumpable_value()? != 0)
}

/// `prctl(PR_GET_DUMPABLE)` as the kernel returns it: 0, 1 or 2 (`SUID_DUMP_ROOT`).
#[allow(unsafe_code)]
pub(crate) fn dumpable_value() -> io::Result<i32> {
    let zero: libc::c_ulong = 0;
    // SAFETY: `PR_GET_DUMPABLE` takes no pointer; it returns the value as the result.
    let r = unsafe { libc::prctl(libc::PR_GET_DUMPABLE, zero, zero, zero, zero) };
    if r < 0 {
        return Err(os_error());
    }
    Ok(r)
}

/// `setrlimit(RLIMIT_CORE, 0, 0)`: the hard limit at 0 too, so the process cannot raise it
/// again.
#[allow(unsafe_code)]
pub(crate) fn zero_rlimit_core() -> io::Result<()> {
    let lim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: `lim` is a local and valid `rlimit`, only read by the kernel.
    let r = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &raw const lim) };
    if r != 0 {
        return Err(os_error());
    }
    Ok(())
}

/// `getrlimit(RLIMIT_CORE)` as `(current, maximum)`.
#[allow(unsafe_code)]
pub(crate) fn rlimit_core() -> io::Result<(u64, u64)> {
    let mut lim = libc::rlimit {
        rlim_cur: u64::MAX,
        rlim_max: u64::MAX,
    };
    // SAFETY: `lim` is local, the size of `rlimit`, and the kernel only writes into it.
    let r = unsafe { libc::getrlimit(libc::RLIMIT_CORE, &raw mut lim) };
    if r != 0 {
        return Err(os_error());
    }
    Ok((lim.rlim_cur, lim.rlim_max))
}

/// `getrlimit(RLIMIT_MEMLOCK)` as `(current, maximum)`; `u64::MAX` is `RLIM_INFINITY`.
#[allow(unsafe_code)]
pub(crate) fn rlimit_memlock() -> io::Result<(u64, u64)> {
    let mut lim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: `lim` is local, the size of `rlimit`, and the kernel only writes into it.
    let r = unsafe { libc::getrlimit(libc::RLIMIT_MEMLOCK, &raw mut lim) };
    if r != 0 {
        return Err(os_error());
    }
    Ok((lim.rlim_cur, lim.rlim_max))
}

/// `mlockall(MCL_CURRENT | MCL_FUTURE | MCL_ONFAULT)` (D62): every page of the process,
/// current and future, stays out of swap as soon as it is touched. Does not survive
/// `fork` or `execve`.
#[allow(unsafe_code)]
pub(crate) fn lock_all_memory() -> io::Result<()> {
    // SAFETY: `mlockall` only takes the flags; it neither reads nor writes process memory.
    let r = unsafe { libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE | libc::MCL_ONFAULT) };
    if r != 0 {
        return Err(os_error());
    }
    Ok(())
}

/// Page size.
#[allow(unsafe_code)]
pub(crate) fn page_size() -> usize {
    // SAFETY: `sysconf` only reads a system constant.
    let r = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    usize::try_from(r).ok().filter(|&p| p > 0).unwrap_or(4096)
}

/// Private anonymous region, only of this process, with `len` bytes (multiple of the page).
#[allow(unsafe_code)]
pub(crate) fn map_anonymous(len: usize) -> io::Result<NonNull<u8>> {
    // SAFETY: null address (the kernel chooses), no file (fd -1, offset 0); the new region
    // does not overlap anything already mapped.
    let p = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            len,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        )
    };
    if p == libc::MAP_FAILED {
        return Err(os_error());
    }
    NonNull::new(p.cast::<u8>()).ok_or_else(|| io::Error::other("mmap devolveu nulo"))
}

/// `munmap` of a region created by [`map_anonymous`].
///
/// # Safety
/// `p` and `len` must be exactly those of a region from [`map_anonymous`] not yet
/// unmapped, and no reference to it may outlive this call.
#[allow(unsafe_code)]
pub(crate) unsafe fn unmap(p: NonNull<u8>, len: usize) {
    // SAFETY: guaranteed by the function contract (own region, no live references).
    unsafe {
        libc::munmap(p.as_ptr().cast(), len);
    }
}

/// `madvise` on an own region. Neither reads nor writes the memory.
#[allow(unsafe_code)]
pub(crate) fn advise(p: NonNull<u8>, len: usize, advice: libc::c_int) -> io::Result<()> {
    // SAFETY: `MADV_DONTDUMP` and `MADV_WIPEONFORK` only change attributes of the pages of
    // the region of `p`, which the caller mapped; they do not touch the content.
    let r = unsafe { libc::madvise(p.as_ptr().cast(), len, advice) };
    if r != 0 {
        return Err(os_error());
    }
    Ok(())
}

/// `mlock` of the pages that contain `region`. Neither reads nor writes the memory; with
/// an invalid range the kernel only returns an error.
#[allow(unsafe_code)]
pub(crate) fn mlock(region: &[u8]) -> io::Result<()> {
    if region.is_empty() {
        return Ok(());
    }
    // SAFETY: the slice is valid memory of this process through the borrow; `mlock` only
    // marks the pages as resident.
    let r = unsafe { libc::mlock(region.as_ptr().cast(), region.len()) };
    if r != 0 {
        return Err(os_error());
    }
    Ok(())
}

/// `munlock` of the pages that contain `region`.
#[allow(unsafe_code)]
pub(crate) fn munlock(region: &[u8]) -> io::Result<()> {
    if region.is_empty() {
        return Ok(());
    }
    // SAFETY: as in [`mlock`]: only changes the attribute of the pages, without accessing
    // the content.
    let r = unsafe { libc::munlock(region.as_ptr().cast(), region.len()) };
    if r != 0 {
        return Err(os_error());
    }
    Ok(())
}

#[allow(unsafe_code)]
pub(crate) fn getegid() -> u32 {
    // SAFETY: no arguments and no side effect; never fails.
    unsafe { libc::getegid() }
}

#[allow(unsafe_code)]
pub(crate) fn getgid() -> u32 {
    // SAFETY: no arguments and no side effect; never fails.
    unsafe { libc::getgid() }
}

/// `getsockopt(SOL_SOCKET, option)` of an integer option (`SO_ACCEPTCONN`, `SO_DOMAIN`,
/// `SO_TYPE`).
#[allow(unsafe_code)]
pub(crate) fn sockopt_int(fd: BorrowedFd<'_>, option: libc::c_int) -> io::Result<libc::c_int> {
    let mut value: libc::c_int = -1;
    let size = size_of::<libc::c_int>();
    let mut len = libc::socklen_t::try_from(size).map_err(|_| io::ErrorKind::InvalidInput)?;
    // SAFETY: `fd` is open through the borrow; `value` and `len` are locals and `len` has
    // the size of a `c_int`, which is what the kernel writes for these options.
    let r = unsafe {
        libc::getsockopt(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            option,
            (&raw mut value).cast(),
            &raw mut len,
        )
    };
    if r != 0 {
        return Err(os_error());
    }
    if usize::try_from(len).ok() != Some(size) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(value)
}

/// First fd passed by systemd (`SD_LISTEN_FDS_START`).
pub(crate) const ACTIVATION_FD: libc::c_int = 3;

/// There has already been an attempt to take fd 3 in this process.
static ACTIVATION_FD_TAKEN: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Duplicates fd 3 (with `O_CLOEXEC`, so no child inherits the listening socket) without
/// taking ownership of it. Only once per process. The caller checks the copy and, if it
/// is the expected socket, closes the original through [`close_activation_fd`].
#[allow(unsafe_code)]
pub(crate) fn dup_activation_fd() -> io::Result<OwnedFd> {
    if ACTIVATION_FD_TAKEN.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "fd de ativação já usado",
        ));
    }
    // SAFETY: `F_GETFD` only queries the fd table; it takes no pointer.
    if unsafe { libc::fcntl(ACTIVATION_FD, libc::F_GETFD) } < 0 {
        return Err(os_error());
    }
    // SAFETY: fd 3 has just been checked as open, and the borrow only lasts until the
    // `try_clone_to_owned` below (an `F_DUPFD_CLOEXEC`), inside this function. Fd 3
    // belongs to systemd (`LISTEN_FDS`): the daemon calls this at the start of `main`,
    // before opening any other file, so no other code closes it in that interval.
    let borrowed = unsafe { BorrowedFd::borrow_raw(ACTIVATION_FD) };
    borrowed.try_clone_to_owned()
}

/// Closes fd 3 after the copy from [`dup_activation_fd`] was checked.
#[allow(unsafe_code)]
pub(crate) fn close_activation_fd() {
    // SAFETY: fd 3 is the systemd socket, checked through the copy; nobody else in this
    // process uses it (the daemon goes on with the copy only), so the `OwnedFd` becomes
    // the only owner and closes it when it goes out of scope.
    drop(unsafe { OwnedFd::from_raw_fd(ACTIVATION_FD) });
}

/// Only in `test-mode`: puts `fd` on number 3 without `O_CLOEXEC`, as systemd does, so the
/// next `exec` receives the socket.
///
/// # Safety
/// Closes whatever was on fd 3 of this process. The caller must `exec` (or exit) right
/// afterwards, without any other code using fd 3.
#[cfg(feature = "test-mode")]
#[allow(unsafe_code)]
pub(crate) unsafe fn put_on_activation_fd(fd: BorrowedFd<'_>) -> io::Result<()> {
    if fd.as_raw_fd() == ACTIVATION_FD {
        // SAFETY: `F_SETFD` with 0 only clears `FD_CLOEXEC` on fd 3, which is `fd`.
        if unsafe { libc::fcntl(ACTIVATION_FD, libc::F_SETFD, 0) } < 0 {
            return Err(os_error());
        }
        return Ok(());
    }
    // SAFETY: `fd` is open through the borrow; `dup2` replaces fd 3 (the function contract
    // says nobody else uses it) and the new one has no `FD_CLOEXEC`.
    if unsafe { libc::dup2(fd.as_raw_fd(), ACTIVATION_FD) } < 0 {
        return Err(os_error());
    }
    Ok(())
}

// --- App startup and launcher (Phase 29a; D47, D57) -----------------------------------

/// `getresgid`: real, effective and saved gid.
#[allow(unsafe_code)]
pub(crate) fn getresgid() -> io::Result<(u32, u32, u32)> {
    let (mut r, mut e, mut s) = (u32::MAX, u32::MAX, u32::MAX);
    // SAFETY: the three pointers are of valid local `gid_t`s; the kernel only writes them.
    let rc = unsafe { libc::getresgid(&raw mut r, &raw mut e, &raw mut s) };
    if rc != 0 {
        return Err(os_error());
    }
    Ok((r, e, s))
}

/// Releases the setgid group: `setresgid(real, real, real)`, checked by `getresgid`
/// afterwards. With all three equal to the real one, the process does not go back to the
/// binary's group (the saved one changed too).
#[allow(unsafe_code)]
pub(crate) fn release_effective_group() -> io::Result<()> {
    let (real, _, _) = getresgid()?;
    // SAFETY: integers only; the kernel lets an unprivileged process set the three gids to
    // any of the ones it already has (the real one is one of them).
    let rc = unsafe { libc::setresgid(real, real, real) };
    if rc != 0 {
        return Err(os_error());
    }
    if getresgid()? != (real, real, real) {
        return Err(io::Error::other(
            "setresgid não deixou os três gids no real",
        ));
    }
    Ok(())
}

#[allow(unsafe_code)]
pub(crate) fn getppid() -> u32 {
    // SAFETY: no arguments and no side effect; never fails.
    let p = unsafe { libc::getppid() };
    u32::try_from(p).unwrap_or(0)
}

/// Does the fd have `FD_CLOEXEC`?
#[allow(unsafe_code)]
pub(crate) fn has_cloexec(fd: BorrowedFd<'_>) -> io::Result<bool> {
    // SAFETY: `F_GETFD` only queries the fd table; `fd` is open through the borrow.
    let r = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFD) };
    if r < 0 {
        return Err(os_error());
    }
    Ok(r & libc::FD_CLOEXEC != 0)
}

/// `tcgetattr`: attributes of the terminal on `fd` (Phase 25, password of the unlock
/// command). `ENOTTY` when `fd` is not a terminal.
#[allow(unsafe_code)]
pub(crate) fn tcgetattr(fd: BorrowedFd<'_>) -> io::Result<libc::termios> {
    // SAFETY: `termios` is a C struct of integers and integer arrays only; zeroed it is a
    // valid value, and `tcgetattr` fills it entirely.
    let mut t: libc::termios = unsafe { std::mem::zeroed() };
    // SAFETY: `t` is local and writable; `fd` is open through the borrow.
    if unsafe { libc::tcgetattr(fd.as_raw_fd(), &raw mut t) } != 0 {
        return Err(os_error());
    }
    Ok(t)
}

/// `tcsetattr(fd, action, t)` (`TCSANOW`, `TCSAFLUSH`...).
#[allow(unsafe_code)]
pub(crate) fn tcsetattr(
    fd: BorrowedFd<'_>,
    action: libc::c_int,
    t: &libc::termios,
) -> io::Result<()> {
    // SAFETY: `t` points to a valid `termios` that is only read; `fd` is open through the
    // borrow.
    if unsafe { libc::tcsetattr(fd.as_raw_fd(), action, t) } != 0 {
        return Err(os_error());
    }
    Ok(())
}

/// Opens a new pseudoterminal and returns (master, slave), both with `O_NOCTTY` and
/// `O_CLOEXEC`: the pty of the interactive `ward ssh` (Phase 27) and, in the tests, the
/// password reading without a real terminal.
#[allow(unsafe_code)]
pub(crate) fn open_pty() -> io::Result<(OwnedFd, OwnedFd)> {
    let flags = libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC;
    // SAFETY: `posix_openpt` only takes the flags and returns a new fd or -1.
    let m = unsafe { libc::posix_openpt(flags) };
    if m < 0 {
        return Err(os_error());
    }
    // SAFETY: `m` has just been opened by `posix_openpt` and only this `OwnedFd` closes it.
    let master = unsafe { OwnedFd::from_raw_fd(m) };
    // SAFETY: `grantpt`/`unlockpt` only take the master fd, opened above.
    if unsafe { libc::grantpt(m) } != 0 || unsafe { libc::unlockpt(m) } != 0 {
        return Err(os_error());
    }
    let mut name = [0u8; 128];
    // SAFETY: `name` is local, with `name.len()` writable bytes; `ptsname_r` writes a
    // zero-terminated C string within that size or returns an error.
    let rc = unsafe { libc::ptsname_r(m, name.as_mut_ptr().cast(), name.len()) };
    if rc != 0 {
        return Err(io::Error::from_raw_os_error(rc));
    }
    let name = CStr::from_bytes_until_nul(&name).map_err(|_| io::ErrorKind::InvalidData)?;
    // SAFETY: `name` is a valid C string; `open` returns a new fd or -1.
    let e = unsafe { libc::open(name.as_ptr(), flags) };
    if e < 0 {
        return Err(os_error());
    }
    // SAFETY: `e` has just been opened and only this `OwnedFd` closes it.
    let slave = unsafe { OwnedFd::from_raw_fd(e) };
    Ok((master, slave))
}

/// `TIOCSWINSZ`: window size of the terminal on `fd` (the master of the `ward ssh` pty;
/// the kernel sends `SIGWINCH` to the foreground group).
#[allow(unsafe_code)]
pub(crate) fn set_window_size(fd: BorrowedFd<'_>, rows: u16, cols: u16) -> io::Result<()> {
    let w = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: `w` is a local `winsize`, only read by the kernel; `fd` is open through the
    // borrow.
    if unsafe { libc::ioctl(fd.as_raw_fd(), libc::TIOCSWINSZ, &raw const w) } != 0 {
        return Err(os_error());
    }
    Ok(())
}

/// `TIOCGWINSZ`: (rows, cols) of the terminal on `fd`. `ENOTTY` if it is not a terminal.
#[allow(unsafe_code)]
pub(crate) fn read_window_size(fd: BorrowedFd<'_>) -> io::Result<(u16, u16)> {
    let mut w = libc::winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: `w` is a local and writable `winsize`, the size `TIOCGWINSZ` writes; `fd` is
    // open through the borrow.
    if unsafe { libc::ioctl(fd.as_raw_fd(), libc::TIOCGWINSZ, &raw mut w) } != 0 {
        return Err(os_error());
    }
    Ok((w.ws_row, w.ws_col))
}

/// `cfmakeraw` on `t` (raw mode of the CLI terminal in the interactive `ward ssh`).
#[allow(unsafe_code)]
pub(crate) fn make_raw(t: &mut libc::termios) {
    // SAFETY: `t` is a valid and writable `termios`; `cfmakeraw` only changes the fields.
    unsafe { libc::cfmakeraw(t) };
}

/// Tests only: clears `FD_CLOEXEC` on `fd`, to simulate an fd that `exec` would inherit.
#[cfg(test)]
#[allow(unsafe_code)]
pub(crate) fn clear_cloexec(fd: BorrowedFd<'_>) -> io::Result<()> {
    // SAFETY: `F_SETFD` with 0 only changes the flag of the fd, open through the borrow.
    if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, 0) } < 0 {
        return Err(os_error());
    }
    Ok(())
}

/// Waits until the other side of the socket closes (`POLLRDHUP`/`POLLHUP`/`POLLERR`),
/// without reading anything: it does not consume the bytes of a protocol response.
pub(crate) fn wait_for_close(fd: BorrowedFd<'_>) -> io::Result<()> {
    while !is_closed(fd, -1)? {}
    Ok(())
}

/// Has the other side of the socket closed? Without reading anything; `wait_ms` is the
/// `poll` timeout (0 does not wait, -1 waits without a timeout).
#[allow(unsafe_code)]
pub(crate) fn is_closed(fd: BorrowedFd<'_>, wait_ms: libc::c_int) -> io::Result<bool> {
    loop {
        let mut pfd = libc::pollfd {
            fd: fd.as_raw_fd(),
            events: libc::POLLRDHUP,
            revents: 0,
        };
        // SAFETY: `pfd` is a local and valid `pollfd`, and the size passed is 1; the fd is
        // open through the borrow; the timeout is an integer.
        let r = unsafe { libc::poll(&raw mut pfd, 1, wait_ms) };
        if r < 0 {
            let e = os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e);
        }
        let end = libc::POLLRDHUP | libc::POLLHUP | libc::POLLERR | libc::POLLNVAL;
        return Ok(pfd.revents & end != 0);
    }
}

/// Home directory of user `uid` through `getpwuid_r` (passwd or the system NSS), never
/// through `$HOME`. No entry or an empty home is an error.
pub(crate) fn uid_home(uid: u32) -> io::Result<std::ffi::OsString> {
    uid_account(uid).map(|(_, home)| home)
}

/// Name and home directory of user `uid` through `getpwuid_r` (passwd or the system NSS),
/// never through `$USER`/`$HOME`. No entry, an empty name or an empty home is an error.
#[allow(unsafe_code)]
pub(crate) fn uid_account(uid: u32) -> io::Result<(std::ffi::OsString, std::ffi::OsString)> {
    use std::os::unix::ffi::OsStrExt;
    let mut size = 4096usize;
    loop {
        let mut buf = vec![0u8; size];
        // SAFETY: `passwd` is a C struct of pointers and integers only; zeroed it is a
        // valid value (null pointers), and `getpwuid_r` fills it.
        let mut pw: libc::passwd = unsafe { std::mem::zeroed() };
        let mut result: *mut libc::passwd = std::ptr::null_mut();
        // SAFETY: `pw` and `result` are locals; `buf` has `buf.len()` writable bytes and
        // lives until the end of this loop iteration, while the pointers of `pw` point
        // into it.
        let rc = unsafe {
            libc::getpwuid_r(
                uid,
                &raw mut pw,
                buf.as_mut_ptr().cast(),
                buf.len(),
                &raw mut result,
            )
        };
        if rc == libc::ERANGE && size < 1024 * 1024 {
            size *= 4;
            continue;
        }
        if rc != 0 {
            return Err(io::Error::from_raw_os_error(rc));
        }
        if result.is_null() || pw.pw_dir.is_null() || pw.pw_name.is_null() {
            return Err(io::ErrorKind::NotFound.into());
        }
        // SAFETY: `pw_dir` and `pw_name` are not null and point to C strings inside `buf`,
        // which is still alive; the copies leave before `buf` is freed.
        let (dir, name) = unsafe { (CStr::from_ptr(pw.pw_dir), CStr::from_ptr(pw.pw_name)) };
        if dir.to_bytes().is_empty() || name.to_bytes().is_empty() {
            return Err(io::ErrorKind::NotFound.into());
        }
        return Ok((
            std::ffi::OsStr::from_bytes(name.to_bytes()).to_owned(),
            std::ffi::OsStr::from_bytes(dir.to_bytes()).to_owned(),
        ));
    }
}

/// Makes the `exec` of `cmd` close every fd ≥ 3: after the `fork`, marks all of them with
/// `FD_CLOEXEC` (`close_range` with `CLOSE_RANGE_CLOEXEC`), and the `exec` closes them;
/// the program gets only 0-2. Marking instead of closing keeps the `std` channel that
/// reports an `exec` failure to the parent. A kernel without the option (< 5.11) makes
/// the `spawn`/`exec` fail (fail closed).
#[allow(unsafe_code)]
pub(crate) fn close_fds_on_exec(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    // SAFETY: the closing runs in the child between the `fork` and the `exec` (or in the
    // process itself, right before the `exec`) and makes a single syscall
    // (`close_range`), which is async-signal-safe: it neither allocates nor takes a lock.
    unsafe {
        cmd.pre_exec(|| {
            let r = libc::syscall(
                libc::SYS_close_range,
                3 as libc::c_uint,
                libc::c_uint::MAX,
                libc::CLOSE_RANGE_CLOEXEC,
            );
            if r != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

/// Like [`close_fds_on_exec`], but leaves `fd` open in the program (Phase 27a, channel of
/// the connect token): after the `fork`, marks every fd ≥ 3 with `FD_CLOEXEC` and then
/// clears the mark only on `fd`. The number does not change (no `dup2`), so it does not
/// step on the `std` channel that reports an `exec` failure. The caller keeps `fd` open
/// until the `spawn` returns.
#[allow(unsafe_code)]
pub(crate) fn inherit_only_on_exec(cmd: &mut std::process::Command, fd: libc::c_int) {
    use std::os::unix::process::CommandExt;
    // SAFETY: runs in the child between the `fork` and the `exec` and makes only two
    // syscalls (`close_range` and `fcntl(F_SETFD)`), both async-signal-safe: they neither
    // allocate nor take a lock. `fd` is only an integer copied into the closure.
    unsafe {
        cmd.pre_exec(move || {
            let r = libc::syscall(
                libc::SYS_close_range,
                3 as libc::c_uint,
                libc::c_uint::MAX,
                libc::CLOSE_RANGE_CLOEXEC,
            );
            if r != 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::fcntl(fd, libc::F_SETFD, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

/// Takes ownership of an inherited fd by number (Phase 27a: `ward ssh --token-fd <n>`).
/// Only accepts `n ≥ 3` (never standard input and outputs) and an open fd that is a socket
/// (`SO_DOMAIN` answers); anything else is refused **without** taking ownership. The fd
/// gets `FD_CLOEXEC` before returning.
///
/// The caller does this once, at the start of the process, with the number the parent
/// passed: nothing in this process opened that fd, so the returned `OwnedFd` is the only
/// owner.
#[allow(unsafe_code)]
pub(crate) fn take_inherited_socket(n: libc::c_int) -> io::Result<OwnedFd> {
    if n < 3 {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    // SAFETY: `F_GETFD` only queries the fd table; it takes no pointer.
    if unsafe { libc::fcntl(n, libc::F_GETFD) } < 0 {
        return Err(os_error());
    }
    // SAFETY: fd `n` has just been checked as open, and the borrow only lives in this
    // function, which does not close it.
    let borrowed = unsafe { BorrowedFd::borrow_raw(n) };
    sockopt_int(borrowed, libc::SO_DOMAIN)?;
    // SAFETY: `n` is open, is a socket, came from the parent of this process (argv) and
    // nothing in this process opened or keeps it (contract above): the `OwnedFd` becomes
    // the only owner and closes it on exit.
    let owner = unsafe { OwnedFd::from_raw_fd(n) };
    // SAFETY: `F_SETFD` with `FD_CLOEXEC` only changes the flag of the fd, which is ours.
    if unsafe { libc::fcntl(owner.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        return Err(os_error());
    }
    Ok(owner)
}

/// `clock_gettime(CLOCK_BOOTTIME)`: monotonic clock that **counts suspended time** (the
/// Rust `Instant` uses `CLOCK_MONOTONIC`, which stops during suspend). Used by the
/// inactivity lock (Phase 24): 4 h with the machine suspended count as 4 h without use.
#[allow(unsafe_code)]
pub(crate) fn boot_clock() -> io::Result<std::time::Duration> {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is local and of the type `clock_gettime` writes; `CLOCK_BOOTTIME` is a
    // valid clock on Linux (2.6.39+). The call only writes into `ts`.
    let r = unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &raw mut ts) };
    if r != 0 {
        return Err(os_error());
    }
    let secs = u64::try_from(ts.tv_sec).map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
    let nanos =
        u32::try_from(ts.tv_nsec).map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
    if nanos >= 1_000_000_000 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(std::time::Duration::new(secs, nanos))
}

// --- Child of `ward run` (Phase 26; S3, S8, D13, D60) ---------------------------------

/// `pipe2` with `O_CLOEXEC`: (read, write), both ≥ 3 (an fd 0-2 becomes a copy ≥ 3), so
/// the child's `dup2` never steps on an fd it still has to use.
#[allow(unsafe_code)]
fn pipe_cloexec() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: `fds` is a local array of two `c_int`, which is what `pipe2` writes.
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        return Err(os_error());
    }
    // SAFETY: both fds have just been created by `pipe2` and are only ours.
    let (r, w) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    Ok((above_2(r)?, above_2(w)?))
}

/// Capacity requested for the child's output pipes (Phase 28g, 2nd audit of M4).
///
/// With the default capacity (64 KiB), a single `write` larger than it enters the pipe in
/// two steps: the kernel copies 64 KiB, wakes the reader and waits for room; the daemon
/// reads and masks what is there, and a secret that crosses the boundary leaves with its
/// beginning in clear. With 256 KiB, a `write` of up to that size is copied whole with the
/// pipe lock held, and the reader only sees it complete. 256 KiB and no more: the kernel
/// counts pipe pages per user (`pipe-user-pages-soft`, 64 MiB), and 64 connections with
/// two pipes each stay at 32 MiB; above the limit, the new pipes of every program of the
/// user shrink.
pub const OUTPUT_PIPE_CAPACITY: usize = 256 * 1024;

/// `F_SETPIPE_SZ` on the pipe of `fd`, without failing: the kernel may refuse (above
/// `pipe-max-size` or with the user at the page limit), and then the pipe keeps the
/// default capacity. It only worsens the mask cut (D34 revised), never exposes a secret.
#[allow(unsafe_code)]
pub(crate) fn widen_pipe(fd: BorrowedFd<'_>, bytes: usize) -> Option<usize> {
    let requested = libc::c_int::try_from(bytes).ok()?;
    // SAFETY: `F_SETPIPE_SZ` only changes the capacity of the open pipe (alive through the
    // borrow); it neither reads nor writes process memory.
    let r = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETPIPE_SZ, requested) };
    usize::try_from(r).ok()
}

/// Current capacity of the pipe of `fd` (`F_GETPIPE_SZ`; tests only).
#[cfg(test)]
#[allow(unsafe_code)]
pub(crate) fn pipe_capacity(fd: BorrowedFd<'_>) -> io::Result<usize> {
    // SAFETY: `F_GETPIPE_SZ` only reads the capacity of the open pipe (alive through the
    // borrow).
    let r = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETPIPE_SZ) };
    usize::try_from(r).map_err(|_| os_error())
}

/// The same fd, or a `CLOEXEC` copy ≥ 3 if it is 0, 1 or 2.
fn above_2(fd: OwnedFd) -> io::Result<OwnedFd> {
    above(fd, 3)
}

/// The same fd, or a `CLOEXEC` copy ≥ `minimum` if it is lower (the original closes).
#[allow(unsafe_code)]
fn above(fd: OwnedFd, minimum: libc::c_int) -> io::Result<OwnedFd> {
    if fd.as_raw_fd() >= minimum {
        return Ok(fd);
    }
    // SAFETY: `F_DUPFD_CLOEXEC` only duplicates the open fd (alive through `fd`) to the
    // lowest free number ≥ `minimum`.
    let new_fd = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, minimum) };
    if new_fd < 0 {
        return Err(os_error());
    }
    // SAFETY: `new_fd` has just been created by `fcntl` and is only ours.
    Ok(unsafe { OwnedFd::from_raw_fd(new_fd) })
}

/// `/dev/null` for reading and writing, `CLOEXEC`, ≥ 5 (the input and outputs of the
/// Chrome of `ward playwright`: nothing it writes reaches the daemon or the log).
fn dev_null_above_4() -> io::Result<OwnedFd> {
    use std::os::unix::fs::OpenOptionsExt;
    let f = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_CLOEXEC)
        .open("/dev/null")?;
    above(OwnedFd::from(f), FIRST_FREE_DEBUG_FD)
}

/// With the debugging pipe, the child gets 0-4; the rest closes at `exec`.
const FIRST_FREE_DEBUG_FD: libc::c_int = 5;

/// Everything the child uses, built before the `fork`: after it, the child only makes
/// async-signal-safe calls, without allocating or taking a lock (the daemon has several
/// threads).
pub(crate) struct ExecPreparation<'a> {
    /// Paths to try in `execve`, in order (the `PATH` search already done).
    pub paths: &'a [*const libc::c_char],
    /// `argv`, null-terminated.
    pub argv: &'a [*const libc::c_char],
    /// `envp`, null-terminated.
    pub envp: &'a [*const libc::c_char],
    /// Working directory.
    pub cwd: &'a CStr,
}

/// How the child gets 0, 1 and 2.
pub(crate) enum Stdio {
    /// Three new pipes (`ward run`, `ward ssh` with a command).
    Pipes,
    /// The slave of a pty on all three, as the controlling terminal of a new session
    /// (`setsid` + `TIOCSCTTY`; the interactive `ward ssh`). The parent keeps the master.
    Pty(OwnedFd),
    /// `ward playwright` (Phase 44, D6): `/dev/null` on 0-2 and the Chrome debugging pipe
    /// (`--remote-debugging-pipe`): the child reads the commands on fd 3 and writes the
    /// responses on fd 4. The parent's ends are `CLOEXEC` and go to no other child.
    DebugPipe,
}

/// The parent's ends.
pub(crate) enum ParentEnds {
    Pipes {
        stdin: OwnedFd,
        stdout: OwnedFd,
        stderr: OwnedFd,
    },
    /// The parent does not keep the slave: the end of the session shows on the master
    /// (`EIO`).
    Pty,
    /// The writing of the commands (fd 3 of the child) and the reading of the responses
    /// (fd 4 of the child).
    DebugPipe {
        commands: OwnedFd,
        responses: OwnedFd,
    },
}

/// The launched child: pid and the parent's ends.
pub(crate) struct RawChild {
    pub pid: libc::pid_t,
    pub ends: ParentEnds,
}

/// Why the child did not reach `execve`.
#[derive(Debug)]
pub(crate) enum ExecError {
    /// Failure in the parent (pipe, fork).
    System(io::Error),
    /// The child reported through the pipe: step ([`STEP_DIRECTORY`]...) and `errno`.
    Child { step: u8, errno: i32 },
}

pub(crate) const STEP_PREPARATION: u8 = 1;
pub(crate) const STEP_DIRECTORY: u8 = 2;
pub(crate) const STEP_EXEC: u8 = 3;

/// `errno` of the thread, without allocating.
#[allow(unsafe_code)]
fn errno() -> i32 {
    // SAFETY: `__errno_location` returns the (valid) pointer to the thread's `errno`.
    unsafe { *libc::__errno_location() }
}

/// `fork` + `execve` with the `ExecPreparation` (Phase 26). In the child, in this order:
/// own session (`setsid`, which also creates the process group with the child's pid; M4
/// audit: in a group of the daemon's session, another process of the user got in through
/// `setpgid` and went through someone else's tunnel/jump bridge; across sessions the
/// kernel refuses `setpgid`), death together with the daemon thread that created it
/// (`PR_SET_PDEATHSIG`), default signals and empty mask, the pipes on 0-2, the `chdir`,
/// every fd ≥ 3 marked `CLOEXEC` (`close_range`) and the `execve` of each path. A failure
/// before `execve` comes back through the notice pipe (5 bytes: step and `errno`); success
/// closes the pipe at `exec`, with no bytes. Nothing goes through a shell (S3).
#[allow(unsafe_code)]
pub(crate) fn fork_exec(p: &ExecPreparation<'_>, stdio: Stdio) -> Result<RawChild, ExecError> {
    let is_null_terminated = |v: &[*const libc::c_char]| v.last().is_some_and(|x| x.is_null());
    if p.paths.is_empty() || !is_null_terminated(p.argv) || !is_null_terminated(p.envp) {
        return Err(ExecError::System(io::ErrorKind::InvalidInput.into()));
    }
    // (child fds, parent ends, whether it is a new session with a controlling terminal)
    let (child_fds, ends, session) = match stdio {
        Stdio::Pipes => {
            let (in_r, in_w) = pipe_cloexec().map_err(ExecError::System)?;
            let (out_r, out_w) = pipe_cloexec().map_err(ExecError::System)?;
            let (err_r, err_w) = pipe_cloexec().map_err(ExecError::System)?;
            // Phase 28g: a single `write` of up to `OUTPUT_PIPE_CAPACITY` enters the pipe
            // whole before the reader wakes up, and the daemon reads and masks it at once.
            widen_pipe(out_r.as_fd(), OUTPUT_PIPE_CAPACITY);
            widen_pipe(err_r.as_fd(), OUTPUT_PIPE_CAPACITY);
            (
                vec![in_r, out_w, err_w],
                ParentEnds::Pipes {
                    stdin: in_w,
                    stdout: out_r,
                    stderr: err_r,
                },
                false,
            )
        }
        Stdio::Pty(slave) => {
            let slave = above_2(slave).map_err(ExecError::System)?;
            let e2 = slave.try_clone().map_err(ExecError::System)?;
            let e3 = slave.try_clone().map_err(ExecError::System)?;
            let (e2, e3) = (
                above_2(e2).map_err(ExecError::System)?,
                above_2(e3).map_err(ExecError::System)?,
            );
            (vec![slave, e2, e3], ParentEnds::Pty, true)
        }
        Stdio::DebugPipe => {
            // Every `dup2` source stays ≥ 5: none is one of the targets 0-4 (a `dup2` of
            // an fd onto itself would leave `CLOEXEC` on, and a target would step on a
            // source not yet copied).
            let high = |fd: OwnedFd| above(fd, FIRST_FREE_DEBUG_FD);
            let null = dev_null_above_4().map_err(ExecError::System)?;
            let (n1, n2) = (
                null.try_clone().and_then(high).map_err(ExecError::System)?,
                null.try_clone().and_then(high).map_err(ExecError::System)?,
            );
            let (cmd_r, cmd_w) = pipe_cloexec().map_err(ExecError::System)?;
            let (resp_r, resp_w) = pipe_cloexec().map_err(ExecError::System)?;
            let (cmd_r, resp_w) = (
                high(cmd_r).map_err(ExecError::System)?,
                high(resp_w).map_err(ExecError::System)?,
            );
            (
                vec![null, n1, n2, cmd_r, resp_w],
                ParentEnds::DebugPipe {
                    commands: cmd_w,
                    responses: resp_r,
                },
                false,
            )
        }
    };
    let (notice_r, notice_w) = pipe_cloexec().map_err(ExecError::System)?;
    // SAFETY: `getpid` has no precondition.
    let parent = unsafe { libc::getpid() };
    // SAFETY: `sigset_t` and `sigaction` are C structs of integers; zeroed they are valid
    // values, and `sigemptyset` puts them in the right format.
    let (empty, default_action) = unsafe {
        let mut empty: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&raw mut empty);
        let mut default_action: libc::sigaction = std::mem::zeroed();
        default_action.sa_sigaction = libc::SIG_DFL;
        libc::sigemptyset(&raw mut default_action.sa_mask);
        (empty, default_action)
    };
    // (source, target) of each `dup2`; the first fd that `close_range` marks comes right
    // after the last target.
    let mut fds = [(-1 as libc::c_int, -1 as libc::c_int); 5];
    for (i, f) in child_fds.iter().enumerate().take(fds.len()) {
        fds[i] = (f.as_raw_fd(), i as libc::c_int);
    }
    let n_fds = child_fds.len().min(fds.len());
    let notice_fd = notice_w.as_raw_fd();
    // SAFETY: `fork` has no precondition; in the child only async-signal-safe calls
    // follow (`child_after_fork`), without allocating.
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return Err(ExecError::System(os_error()));
    }
    if pid == 0 {
        // SAFETY: we are in the child right after the `fork`; the pointers of the
        // `ExecPreparation` point to parent memory copied into the child, alive until the
        // `execve`/`_exit`.
        unsafe {
            child_after_fork(
                p,
                parent,
                &fds[..n_fds],
                session,
                notice_fd,
                &empty,
                &default_action,
            )
        }
    }
    drop((child_fds, notice_w));
    // The child is a session leader (`setsid`) before the `exec`: the group exists when
    // the notice pipe closes at `exec`, before the return (no race with the `kill`). No
    // `setpgid` here: with it before the child's `setsid`, the child would already be a
    // group leader and the `setsid` would fail.
    let mut buf = [0u8; 5];
    let mut read_len = 0;
    let mut f = std::fs::File::from(notice_r);
    while read_len < buf.len() {
        match io::Read::read(&mut f, &mut buf[read_len..]) {
            Ok(0) => break,
            Ok(n) => read_len += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    if read_len == 0 {
        return Ok(RawChild { pid, ends });
    }
    // The child did not reach `exec` and has already exited (or will): collect it.
    let _ = collect(pid);
    let errno = if read_len == buf.len() {
        i32::from_ne_bytes([buf[1], buf[2], buf[3], buf[4]])
    } else {
        libc::EIO
    };
    Err(ExecError::Child {
        step: buf[0],
        errno,
    })
}

/// The child's part, between the `fork` and the `execve`. Never returns.
///
/// # Safety
/// Only in the child just created by the `fork`, with a valid `ExecPreparation` (pointers
/// to null-terminated C strings, null-terminated lists).
#[allow(unsafe_code)]
unsafe fn child_after_fork(
    p: &ExecPreparation<'_>,
    parent: libc::pid_t,
    fds: &[(libc::c_int, libc::c_int)],
    session: bool,
    notice_fd: libc::c_int,
    empty: &libc::sigset_t,
    default_action: &libc::sigaction,
) -> ! {
    let report = |step: u8, e: i32| {
        let b = e.to_ne_bytes();
        let msg = [step, b[0], b[1], b[2], b[3]];
        // SAFETY: `msg` is local; `write` is async-signal-safe; the error does not matter
        // (the parent sees the pipe close without the 5 bytes).
        unsafe { libc::write(notice_fd, msg.as_ptr().cast(), msg.len()) };
    };
    // SAFETY: (the whole block) in the child of a `fork`, only async-signal-safe calls
    // (`setsid`, `ioctl(TIOCSCTTY)`, `prctl`, `getppid`, `sigaction`, `sigprocmask`,
    // `dup2`, `chdir`, `syscall(close_range)`, `execve`, `write`, `_exit`), on valid fds
    // and pointers (those of the `ExecPreparation`, alive in the copy of the parent
    // memory).
    unsafe {
        // New session (the group has the child's pid), with or without a pty: no outside
        // process joins its group.
        if libc::setsid() < 0 {
            report(STEP_PREPARATION, errno());
            libc::_exit(127);
        }
        // With a pty, the slave becomes the controlling terminal: `ssh` reads the window
        // size and the signals from its terminal.
        if session && libc::ioctl(fds[0].0, libc::TIOCSCTTY, 0 as libc::c_int) != 0 {
            report(STEP_PREPARATION, errno());
            libc::_exit(127);
        }
        if libc::prctl(
            libc::PR_SET_PDEATHSIG,
            libc::SIGKILL as libc::c_ulong,
            0,
            0,
            0,
        ) != 0
        {
            report(STEP_PREPARATION, errno());
            libc::_exit(127);
        }
        // The daemon thread already died before the `prctl`: do not go on as an orphan.
        if libc::getppid() != parent {
            libc::_exit(127);
        }
        // The Rust runtime ignores SIGPIPE; the child starts with everything at default.
        for signal in 1..65 {
            if signal != libc::SIGKILL && signal != libc::SIGSTOP {
                libc::sigaction(signal, default_action, std::ptr::null_mut());
            }
        }
        if libc::sigprocmask(libc::SIG_SETMASK, empty, std::ptr::null_mut()) != 0 {
            report(STEP_PREPARATION, errno());
            libc::_exit(127);
        }
        for &(from, to) in fds {
            // `from` ≥ 3 (`pipe_cloexec`), or ≥ 5 with the debugging pipe: never one of
            // the targets.
            if libc::dup2(from, to) < 0 {
                report(STEP_PREPARATION, errno());
                libc::_exit(127);
            }
        }
        if libc::chdir(p.cwd.as_ptr()) != 0 {
            report(STEP_DIRECTORY, errno());
            libc::_exit(127);
        }
        // No daemon fd (socket, vault, connections) reaches the program.
        if libc::syscall(
            libc::SYS_close_range,
            fds.len().max(3) as libc::c_uint,
            libc::c_uint::MAX,
            libc::CLOSE_RANGE_CLOEXEC,
        ) != 0
        {
            report(STEP_PREPARATION, errno());
            libc::_exit(127);
        }
        // The `execvpe` search: goes on through "not here" errors; with `EACCES` on any,
        // that is the final error. Without the `/bin/sh` fallback of `ENOEXEC`.
        let mut saw_eacces = false;
        let mut last = libc::ENOENT;
        for &c in p.paths {
            libc::execve(c, p.argv.as_ptr(), p.envp.as_ptr());
            last = errno();
            match last {
                libc::EACCES => saw_eacces = true,
                libc::ENOENT | libc::ENOTDIR | libc::ESTALE | libc::ENODEV | libc::ETIMEDOUT => {}
                _ => break,
            }
        }
        report(STEP_EXEC, if saw_eacces { libc::EACCES } else { last });
        libc::_exit(127)
    }
}

/// Waits for process `pid` to end **without collecting it** (`waitid` with `WNOWAIT`): the
/// zombie holds the pid (and the group), and the group `kill` does not hit another
/// process.
#[allow(unsafe_code)]
pub(crate) fn wait_without_collecting(pid: libc::pid_t) -> io::Result<()> {
    let id = libc::id_t::try_from(pid).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    loop {
        // SAFETY: `siginfo_t` is a C struct; zeroed it is valid, and `waitid` writes it.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // SAFETY: `info` is local; `P_PID` with a pid of ours; `WNOWAIT` does not collect.
        let r = unsafe {
            libc::waitid(
                libc::P_PID,
                id,
                &raw mut info,
                libc::WEXITED | libc::WNOWAIT,
            )
        };
        if r == 0 {
            return Ok(());
        }
        let e = os_error();
        if e.kind() != io::ErrorKind::Interrupted {
            return Err(e);
        }
    }
}

/// Collects child `pid` (`waitpid`) and returns the raw status.
#[allow(unsafe_code)]
pub(crate) fn collect(pid: libc::pid_t) -> io::Result<libc::c_int> {
    loop {
        let mut status: libc::c_int = 0;
        // SAFETY: `status` is local; the pid is of a child of ours.
        let r = unsafe { libc::waitpid(pid, &raw mut status, 0) };
        if r == pid {
            return Ok(status);
        }
        let e = os_error();
        if e.kind() != io::ErrorKind::Interrupted {
            return Err(e);
        }
    }
}

/// Is there something to read on `fd` (data, end or error) within `wait_ms`? Without
/// reading anything.
#[allow(unsafe_code)]
pub(crate) fn wait_readable(fd: BorrowedFd<'_>, wait_ms: libc::c_int) -> io::Result<bool> {
    loop {
        let mut pfd = libc::pollfd {
            fd: fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: `pfd` is a local and valid `pollfd`, and the size passed is 1; the fd is
        // open through the borrow.
        let r = unsafe { libc::poll(&raw mut pfd, 1, wait_ms) };
        if r < 0 {
            let e = os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e);
        }
        return Ok(r > 0);
    }
}

/// `SIGKILL` to the whole process group `pgid`. A group that no longer exists is not an
/// error.
pub(crate) fn kill_group(pgid: libc::pid_t) -> io::Result<()> {
    signal_group(pgid, libc::SIGKILL)
}

/// `SIGSTOP` to the whole group `pgid` (Phase 54c): stopped, the leader does not collect
/// its children, so their pid is not reused while the daemon looks for them and kills
/// them.
pub(crate) fn stop_group(pgid: libc::pid_t) -> io::Result<()> {
    signal_group(pgid, libc::SIGSTOP)
}

#[allow(unsafe_code)]
fn signal_group(pgid: libc::pid_t, signal: libc::c_int) -> io::Result<()> {
    if pgid <= 1 {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    // SAFETY: `kill` with `-pgid` (> 1) sends the signal only to the group; no pointers.
    if unsafe { libc::kill(-pgid, signal) } != 0 {
        let e = os_error();
        if e.raw_os_error() != Some(libc::ESRCH) {
            return Err(e);
        }
    }
    Ok(())
}

/// `prctl(PR_SET_CHILD_SUBREAPER, 1)` and reading it back (Phase 55, S8): the orphan of
/// any descendant becomes a child of this process, not of `init`/`systemd --user`.
#[allow(unsafe_code)]
pub(crate) fn become_subreaper() -> io::Result<()> {
    let (one, zero): (libc::c_ulong, libc::c_ulong) = (1, 0);
    // SAFETY: `PR_SET_CHILD_SUBREAPER` only reads the integer passed; the others are 0.
    if unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, one, zero, zero, zero) } != 0 {
        return Err(os_error());
    }
    let mut v: libc::c_int = 0;
    // SAFETY: `PR_GET_CHILD_SUBREAPER` writes an `int` through the pointer, which is `v`'s.
    let r = unsafe {
        libc::prctl(
            libc::PR_GET_CHILD_SUBREAPER,
            &raw mut v as libc::c_ulong,
            zero,
            zero,
            zero,
        )
    };
    if r != 0 {
        return Err(os_error());
    }
    if v != 1 {
        return Err(io::Error::other("PR_SET_CHILD_SUBREAPER não ficou ligado"));
    }
    Ok(())
}

/// `getpid()` and `getsid(0)`.
#[allow(unsafe_code)]
pub(crate) fn pid_and_session() -> (libc::pid_t, libc::pid_t) {
    // SAFETY: `getpid` and `getsid(0)` have no precondition and no pointers.
    unsafe { (libc::getpid(), libc::getsid(0)) }
}

/// Sends `signal` through the `pidfd` (`pidfd_send_signal`). `Ok(false)`: the process has
/// already exited.
#[allow(unsafe_code)]
pub(crate) fn pidfd_signal(pidfd: BorrowedFd<'_>, signal: libc::c_int) -> io::Result<bool> {
    // SAFETY: `pidfd` is open through the borrow; a null `info` is allowed and the flags
    // are 0.
    let r = unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            pidfd.as_raw_fd(),
            signal,
            std::ptr::null::<libc::siginfo_t>(),
            0 as libc::c_uint,
        )
    };
    if r == 0 {
        return Ok(true);
    }
    let e = os_error();
    if e.raw_os_error() == Some(libc::ESRCH) {
        Ok(false)
    } else {
        Err(e)
    }
}

/// `waitpid(pid, WNOHANG)`: collects child `pid` if it has already exited. `true` if it
/// collected.
#[allow(unsafe_code)]
pub(crate) fn collect_if_exited(pid: libc::pid_t) -> bool {
    if pid <= 0 {
        return false;
    }
    let mut status: libc::c_int = 0;
    // SAFETY: `status` is local; `pid` > 0 only hits that child (or gives `ECHILD`).
    unsafe { libc::waitpid(pid, &raw mut status, libc::WNOHANG) == pid }
}
