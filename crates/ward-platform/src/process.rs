//! Identification of the process on the other side of the socket (D5, S1, S5) and the
//! process chain shown in the approval.
//!
//! Role by group (D47, [`PeerProcess::classify`]): `ward-app` and `ward-nm` are
//! installed setgid to the `ward-app` and `ward-nm` groups. The setgid `exec` makes the
//! process non-dumpable from the first instruction and puts `ld.so` in secure mode
//! (`AT_SECURE`), and the kernel pins the egid in `SO_PEERCRED` at `connect`. The daemon
//! gives:
//! - the app role to egid `ward-app`, with the parent equal to the daemon itself and no
//!   debugger. The daemon is the only accepted parent (D57): `systemd --user` runs the
//!   units in `~/.config/systemd/user/`, which a user package writes, and checking it
//!   through `/proc` (name, `cmdline`, cgroup) can be forged;
//! - the extension role to egid `ward-nm`, with no debugger and the parent equal to the
//!   installed Chrome, verified through the parent's `/proc`;
//! - the common role (agent) to anyone else;
//! - and no special role with `kernel.yama.ptrace_scope` 0 or `fs.suid_dumpable` 1
//!   ([`KernelPolicy`]).
//!
//! Reading `exe`, `environ` and `maps` ([`PeerProcess::identify`]) is only for the
//! Chrome parent and for tests; it never grants a role on its own:
//! - `/proc/<pid>/exe` is exactly the installed path, and the file it opens is the same
//!   inode as the path (a binary copied to `/tmp` does not pass);
//! - the binary and every directory above it are owned by root and not writable by
//!   group and others;
//! - `TracerPid: 0`;
//! - no `LD_*` variable in the initial environment (`/proc/<pid>/environ`);
//! - every library mapped as executable is a root file, not writable by group and
//!   others, and is the same inode as the mapping (for the Chrome parent, `maps` is not
//!   read).
//!
//! Any failure, read failures included, falls to the common role (agent): the rule fails
//! closed. Against pid reuse, the daemon uses the pidfd from `SO_PEERPIDFD`: it opens
//! `/proc/<pid>` and only then verifies through the pidfd that the original process is
//! still alive; from then on everything is read relative to that open directory.

use std::collections::HashSet;
use std::ffi::CStr;
use std::fmt;
use std::fs::{File, Metadata};
use std::io::{self, Read};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};

use crate::role::{Install, KernelPolicy};
use crate::sys;

/// Limit for reading `/proc/<pid>/status`.
const MAX_STATUS: usize = 64 * 1024;
/// Limit for reading `/proc/<pid>/environ`. A larger environment is refused.
pub const MAX_ENVIRONMENT: usize = 1024 * 1024;
/// Limit for reading `/proc/<pid>/maps`. A larger map is refused.
pub const MAX_MAPS: usize = 8 * 1024 * 1024;
/// Maximum depth of the displayed process chain.
pub const MAX_CHAIN: usize = 32;
/// Maximum length of the name of each chain element, in characters.
pub const MAX_CHAIN_NAME: usize = 64;

/// Why a process was not identified as the installed binary. No path, no variable value
/// and no content read from the process: it may go to the log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The peer's uid differs from the owner of this process.
    UidMismatch,
    /// The process exited (or the pid already belongs to another) during the
    /// verification.
    ProcessVanished,
    /// No permission to read `/proc/<pid>` (non-dumpable process or another owner).
    NoAccess,
    /// Another `/proc` read error, or content out of format.
    Read,
    /// `/proc/<pid>/exe` is not the installed path.
    ExeNotAllowed,
    /// The path is the installed one, but the running file is another inode.
    ExeSwapped,
    /// The installed binary (or a directory above it) is not owned by root, is writable
    /// by group/others, is a symbolic link or does not exist.
    UntrustedInstall,
    /// `TracerPid` != 0.
    Debugger,
    /// `LD_*` variable in the environment (only the known name, or `LD_*`).
    LdVariable(&'static str),
    /// Environment above [`MAX_ENVIRONMENT`].
    LargeEnvironment,
    /// Executable library outside root, deleted, `memfd` or map too large.
    UntrustedLibrary,
    /// The parent of the extension host is not the installed Chrome.
    ParentNotAllowed,
    /// The peer's egid is neither the app group nor the host group (D47): common client.
    CommonGroup,
    /// `ptrace_scope` 0, `suid_dumpable` 1 or unreadable: no special role (D47).
    KernelPolicy,
    /// The app's parent is not the daemon (D47, D57).
    AppParentNotAllowed,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Refusal::UidMismatch => "uid do processo diferente do dono do daemon",
            Refusal::ProcessVanished => "o processo saiu durante a conferência",
            Refusal::NoAccess => "sem permissão para ler /proc do processo",
            Refusal::Read => "não deu para ler /proc do processo",
            Refusal::ExeNotAllowed => "o executável não é o binário instalado",
            Refusal::ExeSwapped => "o executável em execução não é o arquivo instalado",
            Refusal::UntrustedInstall => {
                "o binário instalado não é do root ou aceita escrita de outros"
            }
            Refusal::Debugger => "processo sob depurador (TracerPid ≠ 0)",
            Refusal::LdVariable(name) => return write!(f, "variável {name} no ambiente"),
            Refusal::LargeEnvironment => "ambiente do processo grande demais",
            Refusal::UntrustedLibrary => "biblioteca executável fora do root",
            Refusal::ParentNotAllowed => "o processo pai não é o Chrome instalado",
            Refusal::CommonGroup => "o grupo do processo não é o do app nem o da extensão",
            Refusal::KernelPolicy => {
                "ptrace_scope 0 ou suid_dumpable 1: papel de app e extensão desligado"
            }
            Refusal::AppParentNotAllowed => "o pai do app não é o daemon",
        };
        f.write_str(s)
    }
}

impl std::error::Error for Refusal {}

fn refusal_from_io(e: &io::Error) -> Refusal {
    match e.raw_os_error() {
        Some(libc::EACCES) | Some(libc::EPERM) => Refusal::NoAccess,
        Some(libc::ENOENT) | Some(libc::ESRCH) => Refusal::ProcessVanished,
        _ => Refusal::Read,
    }
}

/// Name of an `LD_*` variable that may go to the log: the loader ones that matter by
/// name, or `LD_*` for any other (an invented name does not go to the log).
pub(crate) fn ld_name_for_log(name: &[u8]) -> &'static str {
    match name {
        b"LD_PRELOAD" => "LD_PRELOAD",
        b"LD_AUDIT" => "LD_AUDIT",
        b"LD_LIBRARY_PATH" => "LD_LIBRARY_PATH",
        _ => "LD_*",
    }
}

/// Numeric value of a `/proc/<pid>/status` field (`"TracerPid:"`, `"PPid:"`).
fn status_field(status: &[u8], field: &[u8]) -> Option<u64> {
    status.split(|&b| b == b'\n').find_map(|line| {
        let rest = line.strip_prefix(field)?;
        let text = std::str::from_utf8(rest).ok()?.trim();
        text.parse().ok()
    })
}

/// `TracerPid` of a `/proc/<pid>/status`.
pub(crate) fn read_tracer_pid(status: &[u8]) -> Option<u64> {
    status_field(status, b"TracerPid:")
}

/// Installed binary accepted for a role: absolute path, without `.`/`..`. It only keeps
/// the path; owner and permissions are verified at each identification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AllowedBinary {
    path: PathBuf,
}

/// Invalid allowed-binary path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidPath;

impl fmt::Display for InvalidPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("caminho de binário inválido: tem de ser absoluto, sem . nem ..")
    }
}

impl std::error::Error for InvalidPath {}

/// Verifies the shape of the path: absolute, only normal components, at least one.
pub(crate) fn simple_absolute_path(path: &Path) -> bool {
    let bytes = path.as_os_str().as_bytes();
    if !bytes.starts_with(b"/") || bytes.ends_with(b"/") || bytes.contains(&0) {
        return false;
    }
    let mut components = path.components();
    if components.next() != Some(Component::RootDir) {
        return false;
    }
    let mut any = false;
    for c in components {
        match c {
            Component::Normal(_) => any = true,
            _ => return false,
        }
    }
    // `components()` hides `a//b` and `a/./b`; the shape must be the canonical one.
    any && path
        .components()
        .collect::<PathBuf>()
        .as_os_str()
        .as_bytes()
        == bytes
}

impl AllowedBinary {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, InvalidPath> {
        let path = path.into();
        if !simple_absolute_path(&path) {
            return Err(InvalidPath);
        }
        Ok(AllowedBinary { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The installed file and every directory above it: owned by root, not writable by
    /// group and others, no symbolic link in the path. Returns the file's metadata.
    fn verify_installed(&self) -> Result<Metadata, Refusal> {
        verify_root_file(&self.path).map_err(|_| Refusal::UntrustedInstall)
    }
}

/// Regular file owned by root, not writable by group/others, with every directory above
/// it also owned by root and not writable by group/others, no symbolic link in the path.
pub(crate) fn verify_root_file(path: &Path) -> Result<Metadata, ()> {
    let meta = std::fs::symlink_metadata(path).map_err(|_| ())?;
    if !meta.file_type().is_file() || !root_owned_no_write(&meta) {
        return Err(());
    }
    let mut current = path.parent();
    while let Some(dir) = current {
        let m = std::fs::symlink_metadata(dir).map_err(|_| ())?;
        if !m.file_type().is_dir() || !root_owned_no_write(&m) {
            return Err(());
        }
        current = dir.parent();
    }
    Ok(meta)
}

fn root_owned_no_write(m: &Metadata) -> bool {
    m.uid() == 0 && m.mode() & 0o022 == 0
}

/// Credentials from `SO_PEERCRED`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeerCredentials {
    pub pid: u32,
    pub uid: u32,
    pub gid: u32,
}

/// Live process pointed to by pid + pidfd.
#[derive(Debug)]
struct Target {
    pid: u32,
    pidfd: OwnedFd,
}

impl Target {
    fn from_pid(pid: u32) -> io::Result<Target> {
        let p = libc::pid_t::try_from(pid).map_err(|_| io::ErrorKind::InvalidInput)?;
        let pidfd = sys::pidfd_open(p)?;
        Ok(Target { pid, pidfd })
    }

    /// Opens `/proc/<pid>` and verifies, after opening, that the pidfd's process is still
    /// alive: so the directory is its own, and not that of another that inherited the
    /// number.
    fn view(&self) -> Result<View, Refusal> {
        let dir = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(format!("/proc/{}", self.pid))
            .map_err(|e| refusal_from_io(&e))?;
        self.verify_alive()?;
        Ok(View { dir: dir.into() })
    }

    fn verify_alive(&self) -> Result<(), Refusal> {
        match sys::pidfd_alive(self.pidfd.as_fd()) {
            Ok(true) => Ok(()),
            Ok(false) => Err(Refusal::ProcessVanished),
            Err(e) => Err(refusal_from_io(&e)),
        }
    }
}

/// Open `/proc/<pid>`; everything is read relative to it.
struct View {
    dir: OwnedFd,
}

impl View {
    fn read(&self, name: &CStr, limit: usize) -> Result<Vec<u8>, Refusal> {
        let fd = sys::openat(self.dir.as_fd(), name, libc::O_RDONLY | libc::O_NOFOLLOW)
            .map_err(|e| refusal_from_io(&e))?;
        read_limited(File::from(fd), limit)
    }

    fn status(&self) -> Result<Vec<u8>, Refusal> {
        self.read(c"status", MAX_STATUS)
    }
}

/// Reads at most `limit` bytes; more than that is [`Refusal::Read`].
fn read_limited(file: impl Read, limit: usize) -> Result<Vec<u8>, Refusal> {
    let mut buf = Vec::new();
    let ceiling = u64::try_from(limit)
        .map_err(|_| Refusal::Read)?
        .saturating_add(1);
    file.take(ceiling)
        .read_to_end(&mut buf)
        .map_err(|e| refusal_from_io(&e))?;
    if buf.len() > limit {
        return Err(Refusal::Read);
    }
    Ok(buf)
}

/// pid the kernel associates with the pidfd (`/proc/self/fdinfo/<fd>`, line `Pid:`).
/// `None` when the process already exited (`-1`) or is in another pid namespace (`0`).
fn pidfd_pid(pidfd: BorrowedFd<'_>) -> io::Result<Option<u32>> {
    let info = std::fs::read(format!("/proc/self/fdinfo/{}", pidfd.as_raw_fd()))?;
    let line = info
        .split(|&b| b == b'\n')
        .find_map(|l| l.strip_prefix(b"Pid:"))
        .ok_or(io::ErrorKind::InvalidData)?;
    let text = std::str::from_utf8(line).map_err(|_| io::ErrorKind::InvalidData)?;
    let pid: i64 = text
        .trim()
        .parse()
        .map_err(|_| io::ErrorKind::InvalidData)?;
    Ok(u32::try_from(pid).ok().filter(|&p| p > 0))
}

/// The process on the other side of a Unix socket: credentials from `SO_PEERCRED` and
/// pidfd from `SO_PEERPIDFD` (the same process, verified by the pid).
#[derive(Debug)]
pub struct PeerProcess {
    cred: PeerCredentials,
    target: Target,
}

impl PeerProcess {
    /// Reads `SO_PEERCRED` and `SO_PEERPIDFD` from the socket. Fails (the daemon must
    /// close the connection) when the pid is unknown (another namespace), when the kernel
    /// does not give the pidfd or when the pidfd is not of the same pid.
    pub fn from_socket(socket: &impl AsFd) -> io::Result<PeerProcess> {
        let fd = socket.as_fd();
        let c = sys::peercred(fd)?;
        let pid = u32::try_from(c.pid)
            .ok()
            .filter(|&p| p > 0)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "pid do par desconhecido"))?;
        let pidfd = sys::peer_pidfd(fd)?;
        if pidfd_pid(pidfd.as_fd())? != Some(pid) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "pidfd do par não confere com o pid",
            ));
        }
        Ok(PeerProcess {
            cred: PeerCredentials {
                pid,
                uid: c.uid,
                gid: c.gid,
            },
            target: Target { pid, pidfd },
        })
    }

    pub fn credentials(&self) -> PeerCredentials {
        self.cred
    }

    /// D5 verification through `/proc` (exe, root owner, `TracerPid`, `LD_*`, `maps`).
    /// Since D47 it grants no role: it serves the Chrome parent
    /// ([`verify_parent`](Self::verify_parent)) and the tests. Against a non-dumpable
    /// process (the setgid app and host, `wardd`) it fails closed with
    /// [`Refusal::NoAccess`].
    pub fn identify(&self, binary: &AllowedBinary) -> Result<(), Refusal> {
        self.verify_uid()?;
        identify_image(&self.target, binary, true)
    }

    /// pid of the parent, with its pidfd open and `PPid` read again afterwards (if the
    /// parent had exited and the number belonged to another, the child would already
    /// have another `PPid`).
    fn parent(&self) -> Result<Target, Refusal> {
        let view = self.target.view()?;
        let ppid = status_field(&view.status()?, b"PPid:").ok_or(Refusal::Read)?;
        let ppid = u32::try_from(ppid).map_err(|_| Refusal::Read)?;
        if ppid == 0 {
            return Err(Refusal::Read);
        }
        let parent = Target::from_pid(ppid).map_err(|e| refusal_from_io(&e))?;
        let again = status_field(&view.status()?, b"PPid:").ok_or(Refusal::Read)?;
        self.target.verify_alive()?;
        if again != u64::from(ppid) {
            return Err(Refusal::ProcessVanished);
        }
        Ok(parent)
    }

    /// pid of the peer's parent, read through the pidfd (the same process, alive). For
    /// the SSH agent of `ward ssh` (D12, Phase 27): the `ssh` created by the daemon's
    /// `sftp` child.
    pub fn parent_pid(&self) -> Option<u32> {
        self.parent().ok().map(|a| a.pid)
    }

    /// The peer's parent is the installed `chrome` (extension host): exe, root owner,
    /// debugger and `LD_*` read from the parent's `/proc`. Chrome's libraries are not
    /// verified: it maps downloaded components into the user's directory (Widevine).
    pub fn verify_parent(&self, chrome: &AllowedBinary) -> Result<(), Refusal> {
        let parent = self.parent().map_err(|_| Refusal::ParentNotAllowed)?;
        if parent.pid <= 1 {
            return Err(Refusal::ParentNotAllowed);
        }
        identify_image(&parent, chrome, false).map_err(|_| Refusal::ParentNotAllowed)?;
        self.target.verify_alive()
    }

    /// The app's parent is this process: the daemon opened it through the launcher (D57).
    /// Any other parent, including one that imitates `systemd --user`, is refused.
    fn verify_app_parent(&self) -> Result<(), Refusal> {
        let parent = self.parent().map_err(|r| match r {
            Refusal::ProcessVanished => r,
            _ => Refusal::AppParentNotAllowed,
        })?;
        if parent.pid != std::process::id() {
            return Err(Refusal::AppParentNotAllowed);
        }
        self.target.verify_alive()
    }

    /// Role by the egid from `SO_PEERCRED` (D47). Any failure gives [`PeerClass::Common`]
    /// with the reason for the log.
    pub fn classify(&self, inst: &Install, policy: &KernelPolicy) -> PeerClass {
        let gid = self.cred.gid;
        let r = if gid == inst.app_gid() {
            self.verify_special(policy)
                .and_then(|()| self.verify_app_parent())
                .map(|()| PeerClass::App)
        } else if gid == inst.extension_gid() {
            self.verify_special(policy)
                .and_then(|()| self.verify_parent(inst.chrome()))
                .map(|()| PeerClass::ExtensionHost)
        } else {
            Err(Refusal::CommonGroup)
        };
        // A debugger attached in the middle of the verification is also refused.
        match r.and_then(|c| self.reverify_no_debugger().map(|()| c)) {
            Ok(c) => c,
            Err(reason) => PeerClass::Common { reason },
        }
    }

    /// Only in `test-mode` (D63): the class of the test app, **without** the egid of the
    /// `ward-app` group (on the development host the groups do not exist and the test
    /// binary is not setgid). It still requires the same uid, no debugger and the parent
    /// equal to this process (the daemon); verifying that the pid is the one the launcher
    /// opened is up to the daemon (`wardd`, Phase 23a). No kernel policy: the test app is
    /// dumpable and does not hold the real PIN (only the test vault exists, D7).
    #[cfg(feature = "test-mode")]
    pub fn classify_test_app(&self) -> PeerClass {
        let r = self
            .verify_uid()
            .and_then(|()| self.reverify_no_debugger())
            .and_then(|()| self.verify_app_parent())
            .and_then(|()| self.reverify_no_debugger());
        match r {
            Ok(()) => PeerClass::App,
            Err(reason) => PeerClass::Common { reason },
        }
    }

    /// Only in `test-mode` (Phase 41, exception of the D63 kind): the class of the
    /// extension's test host, **without** the egid of the `ward-nm` group (on the
    /// development host the groups do not exist and the test `ward-nm` is not setgid).
    /// Everything else of the release is still required: same uid, the kernel policy, no
    /// debugger and the parent equal to the installed Chrome verified through `/proc`
    /// ([`verify_parent`](Self::verify_parent)). It is "the release minus the egid", like
    /// the test app.
    #[cfg(feature = "test-mode")]
    pub fn classify_test_extension_host(
        &self,
        chrome: &AllowedBinary,
        policy: &KernelPolicy,
    ) -> PeerClass {
        let r = self
            .verify_special(policy)
            .and_then(|()| self.verify_parent(chrome))
            .and_then(|()| self.reverify_no_debugger());
        match r {
            Ok(()) => PeerClass::ExtensionHost,
            Err(reason) => PeerClass::Common { reason },
        }
    }

    /// Before any special role: same uid, kernel policy and no debugger.
    fn verify_special(&self, policy: &KernelPolicy) -> Result<(), Refusal> {
        self.verify_uid()?;
        policy.verify()?;
        self.reverify_no_debugger()
    }

    /// Verifies again, before each sensitive app or extension operation, that the process
    /// is still alive and without a debugger. `/proc/<pid>/status` is readable even after
    /// the client becomes non-dumpable.
    pub fn reverify_no_debugger(&self) -> Result<(), Refusal> {
        let view = self.target.view()?;
        match read_tracer_pid(&view.status()?) {
            Some(0) => self.target.verify_alive(),
            Some(_) => Err(Refusal::Debugger),
            None => Err(Refusal::Read),
        }
    }

    /// Process chain of the peer, for display (see [`process_chain`]).
    pub fn chain(&self) -> Vec<ChainElement> {
        process_chain(self.cred.pid)
    }

    fn verify_uid(&self) -> Result<(), Refusal> {
        if self.cred.uid != sys::geteuid() {
            return Err(Refusal::UidMismatch);
        }
        Ok(())
    }
}

/// Role the platform recognizes in the peer. The protocol (Phase 20) maps: `App` -> app
/// role, `ExtensionHost` -> extension role, `Common` -> agent role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerClass {
    App,
    ExtensionHost,
    /// Common client, with the reason (for the log).
    Common {
        reason: Refusal,
    },
}

/// Verifications of [`identify_image`] on a live process.
fn identify_image(
    target: &Target,
    binary: &AllowedBinary,
    verify_maps: bool,
) -> Result<(), Refusal> {
    let view = target.view()?;
    verify_tracer(&view)?;

    let exe = sys::readlinkat(view.dir.as_fd(), c"exe").map_err(|e| refusal_from_io(&e))?;
    if exe != binary.path.as_os_str().as_bytes() {
        return Err(Refusal::ExeNotAllowed);
    }
    let installed = binary.verify_installed()?;
    // The file actually running (the magic link opened with O_PATH), not the name.
    let running = sys::openat(view.dir.as_fd(), c"exe", libc::O_PATH)
        .map_err(|e| refusal_from_io(&e))
        .map(File::from)?
        .metadata()
        .map_err(|e| refusal_from_io(&e))?;
    if (running.dev(), running.ino()) != (installed.dev(), installed.ino())
        || !root_owned_no_write(&running)
    {
        return Err(Refusal::ExeSwapped);
    }

    let environment = view.read(c"environ", MAX_ENVIRONMENT).map_err(|r| {
        if r == Refusal::Read {
            Refusal::LargeEnvironment
        } else {
            r
        }
    })?;
    verify_environment(&environment)?;

    if verify_maps {
        let maps = view.read(c"maps", MAX_MAPS).map_err(|r| {
            if r == Refusal::Read {
                Refusal::UntrustedLibrary
            } else {
                r
            }
        })?;
        verify_executable_maps(&maps)?;
    }

    // Again at the end: a debugger attached in the middle of the verification is also
    // refused.
    verify_tracer(&view)?;
    target.verify_alive()
}

fn verify_tracer(view: &View) -> Result<(), Refusal> {
    match read_tracer_pid(&view.status()?) {
        Some(0) => Ok(()),
        Some(_) => Err(Refusal::Debugger),
        None => Err(Refusal::Read),
    }
}

/// `environ` is `NAME=value\0...`. Refuses any name with the `LD_` prefix. The value is
/// never looked at.
fn verify_environment(environment: &[u8]) -> Result<(), Refusal> {
    for item in environment.split(|&b| b == 0) {
        let name = item.split(|&b| b == b'=').next().unwrap_or_default();
        if name.starts_with(b"LD_") {
            return Err(Refusal::LdVariable(ld_name_for_log(name)));
        }
    }
    Ok(())
}

/// Line of `/proc/<pid>/maps`: `start-end perms offset dev inode path`.
struct MapLine<'a> {
    executable: bool,
    inode: u64,
    path: &'a [u8],
}

fn parse_map_line(line: &[u8]) -> Option<MapLine<'_>> {
    let mut rest = line;
    let mut fields = [&b""[..]; 5];
    for field in &mut fields {
        let start = rest.iter().position(|&b| b != b' ')?;
        rest = &rest[start..];
        let end = rest.iter().position(|&b| b == b' ').unwrap_or(rest.len());
        *field = &rest[..end];
        rest = &rest[end..];
    }
    let perms = fields[1];
    let inode = std::str::from_utf8(fields[4]).ok()?.parse().ok()?;
    let start = rest.iter().position(|&b| b != b' ').unwrap_or(rest.len());
    Some(MapLine {
        executable: perms.get(2) == Some(&b'x'),
        inode,
        path: &rest[start..],
    })
}

fn verify_executable_maps(maps: &[u8]) -> Result<(), Refusal> {
    let mut verified: HashSet<(&[u8], u64)> = HashSet::new();
    for line in maps.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        let map = parse_map_line(line).ok_or(Refusal::UntrustedLibrary)?;
        // Anonymous, `[vdso]`, `[stack]`: no file to verify.
        if !map.executable || !map.path.starts_with(b"/") {
            continue;
        }
        if !verified.insert((map.path, map.inode)) {
            continue;
        }
        // Deleted, swapped or `memfd:` (shows up as "/memfd:... (deleted)").
        if map.path.ends_with(b" (deleted)") {
            return Err(Refusal::UntrustedLibrary);
        }
        let path = Path::new(std::ffi::OsStr::from_bytes(map.path));
        let meta = verify_root_file(path).map_err(|_| Refusal::UntrustedLibrary)?;
        if meta.ino() != map.inode {
            return Err(Refusal::UntrustedLibrary);
        }
    }
    Ok(())
}

/// A process of the chain, for display. The name is the kernel's `comm`, which the
/// process itself can change: it serves to show the user, never to decide.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainElement {
    pub pid: u32,
    pub name: String,
}

/// Process chain from `pid` up to the oldest one that can be read, from the oldest to
/// the newest (`claude -> bash -> ward`). Up to [`MAX_CHAIN`] elements; names up to
/// [`MAX_CHAIN_NAME`] characters, **without escaping**: whoever displays passes each name
/// through the visible escape of D51 (the display module of `ward-core`, in `wardd`).
/// Phase 39f: replacing a control or direction character with `?` hid what the process
/// put in its name (U+202E became `?`, the same as a real `?`). Only for display: between
/// one read and the next the pids may change.
pub fn process_chain(pid: u32) -> Vec<ChainElement> {
    let mut chain = Vec::new();
    let mut seen = HashSet::new();
    let mut current = pid;
    while current > 0 && chain.len() < MAX_CHAIN && seen.insert(current) {
        let Ok(file) = File::open(format!("/proc/{current}/status")) else {
            break;
        };
        let Ok(status) = read_limited(file, MAX_STATUS) else {
            break;
        };
        let name = status
            .split(|&b| b == b'\n')
            .find_map(|l| l.strip_prefix(b"Name:"))
            .map(name_from_status)
            .unwrap_or_else(|| "?".to_owned());
        chain.push(ChainElement { pid: current, name });
        let Some(ppid) = status_field(&status, b"PPid:").and_then(|p| u32::try_from(p).ok()) else {
            break;
        };
        current = ppid;
    }
    chain.reverse();
    chain
}

/// The name after `Name:` in `status` (the kernel separates it with a `\t`), as is, up
/// to [`MAX_CHAIN_NAME`] characters. A byte that is not UTF-8 becomes U+FFFD.
fn name_from_status(raw: &[u8]) -> String {
    let raw = raw.strip_prefix(b"\t").unwrap_or(raw);
    let name: String = String::from_utf8_lossy(raw)
        .chars()
        .take(MAX_CHAIN_NAME)
        .collect();
    if name.is_empty() {
        "?".to_owned()
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_path_only_absolute_and_canonical() {
        assert!(AllowedBinary::new("/usr/local/bin/ward-app").is_ok());
        for bad in [
            "",
            "/",
            "ward-app",
            "./ward-app",
            "/usr/local/../tmp/ward-app",
            "/usr/./bin/x",
            "/usr//bin/x",
            "/usr/bin/",
        ] {
            assert_eq!(AllowedBinary::new(bad), Err(InvalidPath), "{bad}");
        }
    }

    #[test]
    fn environment_with_any_ld_refuses_without_value() {
        assert_eq!(verify_environment(b"PATH=/usr/bin\0HOME=/x\0"), Ok(()));
        assert_eq!(
            verify_environment(b"PATH=/usr/bin\0LD_PRELOAD=/tmp/falso.so\0"),
            Err(Refusal::LdVariable("LD_PRELOAD"))
        );
        assert_eq!(
            verify_environment(b"LD_AUDIT=x\0"),
            Err(Refusal::LdVariable("LD_AUDIT"))
        );
        assert_eq!(
            verify_environment(b"A=LD_PRELOAD\0LD_LIBRARY_PATH=\0"),
            Err(Refusal::LdVariable("LD_LIBRARY_PATH"))
        );
        assert_eq!(
            verify_environment(b"LD_BIND_NOW=1\0"),
            Err(Refusal::LdVariable("LD_*"))
        );
        // An empty variable without `=` also counts by name.
        assert_eq!(
            verify_environment(b"LD_PRELOAD\0"),
            Err(Refusal::LdVariable("LD_PRELOAD"))
        );
        // A value containing "LD_" is not a name.
        assert_eq!(verify_environment(b"X=LD_PRELOAD=1\0"), Ok(()));
    }

    #[test]
    fn tracer_pid_and_ppid_from_status() {
        let s = b"Name:\tbash\nPPid:\t42\nTracerPid:\t0\n";
        assert_eq!(read_tracer_pid(s), Some(0));
        assert_eq!(status_field(s, b"PPid:"), Some(42));
        assert_eq!(read_tracer_pid(b"TracerPid:\t913\n"), Some(913));
        assert_eq!(read_tracer_pid(b"Name:\tx\n"), None);
        assert_eq!(read_tracer_pid(b"TracerPid:\tabc\n"), None);
    }

    #[test]
    fn map_line() {
        let l = b"7f00-7f10 r-xp 00000000 fd:01 123456                     /usr/lib/x86_64-linux-gnu/libc.so.6";
        let m = parse_map_line(l).unwrap();
        assert!(m.executable);
        assert_eq!(m.inode, 123456);
        assert_eq!(m.path, b"/usr/lib/x86_64-linux-gnu/libc.so.6");
        let anon = parse_map_line(b"7f00-7f10 rw-p 00000000 00:00 0 ").unwrap();
        assert!(!anon.executable);
        assert_eq!(anon.path, b"");
        let space = parse_map_line(b"1-2 r-xp 0 00:01 9 /tmp/a b.so").unwrap();
        assert_eq!(space.path, b"/tmp/a b.so");
        assert!(parse_map_line(b"lixo").is_none());
    }

    #[test]
    fn maps_refuse_library_outside_root() {
        // memfd and deleted file.
        assert_eq!(
            verify_executable_maps(b"1-2 r-xp 0 00:01 7 /memfd:x (deleted)\n"),
            Err(Refusal::UntrustedLibrary)
        );
        // User file (the test binary itself, in target/).
        let exe = std::env::current_exe().unwrap();
        let ino = std::fs::metadata(&exe).unwrap().ino();
        let line = format!("1-2 r-xp 0 00:01 {ino} {}\n", exe.display());
        assert_eq!(
            verify_executable_maps(line.as_bytes()),
            Err(Refusal::UntrustedLibrary)
        );
        // A broken line is refused instead of skipped.
        assert_eq!(
            verify_executable_maps(b"lixo\n"),
            Err(Refusal::UntrustedLibrary)
        );
        // Anonymous and [vdso] pass.
        assert_eq!(
            verify_executable_maps(b"1-2 r-xp 0 00:00 0 [vdso]\n3-4 rwxp 0 00:00 0 \n"),
            Ok(())
        );
    }

    #[test]
    fn maps_refuse_inode_different_from_path() {
        let real_maps = std::fs::read("/proc/self/maps").unwrap();
        // Takes a real root library and changes the inode.
        let text = String::from_utf8_lossy(&real_maps);
        let line = text
            .lines()
            .filter_map(|l| {
                parse_map_line(l.as_bytes()).map(|m| (l.to_owned(), m.inode, m.path.to_vec()))
            })
            .find(|(_, _, c)| c.starts_with(b"/usr/lib"))
            .expect("alguma biblioteca do sistema mapeada");
        let path = String::from_utf8(line.2).unwrap();
        let right = format!("1-2 r-xp 0 00:01 {} {path}\n", line.1);
        assert_eq!(verify_executable_maps(right.as_bytes()), Ok(()));
        let wrong = format!("1-2 r-xp 0 00:01 {} {path}\n", line.1 + 1);
        assert_eq!(
            verify_executable_maps(wrong.as_bytes()),
            Err(Refusal::UntrustedLibrary)
        );
    }

    /// Phase 39f (D51): the name goes as the process set it (U+202E, ESC and space do not
    /// become `?` nor vanish); the visible escape is up to whoever displays (`wardd`).
    #[test]
    fn chain_name_without_question_mark_replacement() {
        assert_eq!(name_from_status(b"\tbash"), "bash");
        assert_eq!(name_from_status(b"\ta\x1b[31mb"), "a\x1b[31mb");
        assert_eq!(name_from_status("\tx\u{202E}y".as_bytes()), "x\u{202E}y");
        assert_eq!(name_from_status("\t claude ".as_bytes()), " claude ");
        assert_eq!(name_from_status(b"\t"), "?");
        assert_eq!(
            name_from_status(&[b'a'; 200]).chars().count(),
            MAX_CHAIN_NAME
        );
        assert_eq!(name_from_status(&[0xff, b'a']), "\u{FFFD}a");
    }

    /// Phase 39f (D51): a real process with U+202E in its `comm` shows up in the chain
    /// with the U+202E, not with `?`.
    #[test]
    fn attack_process_with_u202e_in_name_reaches_chain_whole() {
        let dir = std::env::temp_dir().join(format!("ward-cadeia-39f-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // `comm` = name of the executable (up to 15 bytes).
        let exe = dir.join("cl\u{202E}edua");
        std::fs::copy("/usr/bin/sleep", &exe).unwrap();
        // `ETXTBSY`: a `fork` from another test running in parallel can hold the copy's
        // fd until its `exec`; try again (Phase 55: more tests with `fork`).
        let mut attempts = 0;
        let mut child = loop {
            match std::process::Command::new(&exe).arg("5").spawn() {
                Err(e) if e.raw_os_error() == Some(libc::ETXTBSY) && attempts < 50 => {
                    attempts += 1;
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                r => break r.unwrap(),
            }
        };
        let chain = process_chain(child.id());
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(&dir);
        let last = chain.last().expect("cadeia do filho");
        assert_eq!(last.pid, child.id());
        assert_eq!(last.name, "cl\u{202E}edua");
        assert!(!last.name.contains('?'));
    }

    #[test]
    fn chain_of_own_process_ends_in_it() {
        let chain = process_chain(std::process::id());
        assert!(!chain.is_empty() && chain.len() <= MAX_CHAIN);
        assert_eq!(chain.last().unwrap().pid, std::process::id());
        // Nonexistent pid: empty chain, no panic.
        assert!(process_chain(u32::MAX).is_empty());
        assert!(process_chain(0).is_empty());
    }

    #[test]
    fn install_refuses_user_file_and_link() {
        let exe = std::env::current_exe().unwrap();
        let b = AllowedBinary::new(exe).unwrap();
        assert_eq!(b.verify_installed().unwrap_err(), Refusal::UntrustedInstall);
        // Nonexistent.
        let b = AllowedBinary::new("/usr/bin/ward-nao-existe-falso").unwrap();
        assert_eq!(b.verify_installed().unwrap_err(), Refusal::UntrustedInstall);
    }

    #[test]
    fn refusal_does_not_leak_detail() {
        assert_eq!(
            Refusal::LdVariable("LD_*").to_string(),
            "variável LD_* no ambiente"
        );
        assert!(!Refusal::ExeNotAllowed.to_string().contains('/'));
    }
}
