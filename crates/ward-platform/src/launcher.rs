//! App launcher (D57; D47, D48; S1, S8): the daemon is the only accepted parent of
//! `ward-app` and opens it by `fork` + `execve` of the installed binary, with an
//! environment **built from scratch** ([`app_environment`]) that inherits nothing from
//! the daemon (no `LISTEN_*`) nor from the request. The app verifies the same environment
//! before GTK ([`verify_app_environment`]).
//!
//! The environment:
//! - `XDG_RUNTIME_DIR=/run/user/<uid>`: the uid's dir, 0700, no link in the path;
//! - `WAYLAND_DISPLAY=wayland-<n>`: name without `/` of the **single** `wayland-<n>`
//!   socket of the uid in that dir, no link. None or more than one: the app does not open
//!   (the operation answers "approval unavailable", D21);
//! - `DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/<uid>/bus`, socket of the uid, no link;
//! - `XDG_SESSION_TYPE=wayland`, `GDK_BACKEND=wayland`; no `DISPLAY`;
//! - `HOME` from `getpwuid_r`, `PATH=/usr/local/bin:/usr/bin:/bin`;
//! - `LANG`/`LC_*` of the daemon itself, only with a locale name value (no `/`);
//! - the variables that turn off module sources outside root, verified with the real
//!   WebKitGTK in Phase 29a (D48, handoff 29a):
//!   - `GST_PLUGIN_SYSTEM_PATH` = system plugin dir: without it, the WebKitGTK web
//!     process loads the plugins from `~/.local/share/gstreamer-1.0/plugins`;
//!   - `NO_AT_BRIDGE=1` and `WEBKIT_A11Y_BUS_ADDRESS=unix:path=/dev/null`: without them,
//!     GTK and the web process publish the accessibility tree (the screen text, with the
//!     revealed value) on the session AT-SPI bus, which any process of the uid reads;
//!   - `CUDA_MPS_PIPE_DIRECTORY=/proc/ward-sem-mps` (D64): without it, the MPS client of
//!     the NVIDIA library, in the web process, connects to `/tmp/nvidia-mps/control`,
//!     where a process of the same uid can listen. Nobody creates the dir in `/proc`,
//!     not even root; the effect is that of a machine without an MPS server.
//!   - `GST_PLUGIN_LOADING_WHITELIST` = only `coreelements` and `app` from the system
//!     dir ([`ALLOWED_GST_PLUGINS`]; Phase 39c, channels of 29b): without it,
//!     `gst-plugin-scanner` (child of the web process) runs the `plugin_init` of every
//!     system plugin, and `frei0r`, `ladspa` and `libvisual` `dlopen` the `.so` files
//!     from `~/.frei0r-1/lib`, `~/.ladspa` and `~/.libvisual/actor` (attacker code);
//!     `nvcodec` loads libcuda, which opens `/dev/shm/cuda_injection_path_shm` (Nsight
//!     injection), `bind`s an abstract socket and creates `~/.nv/ComputeCache`; the Intel
//!     video ones read `~/.mfx_trace` and `/dev/shm/GFX_MEDIA_TRACE`. With the list, the
//!     `dlopen` of root's files still happens, but no `plugin_init` outside it runs. The
//!     app plays no media;
//!   - `__GL_APPLICATION_PROFILE=0` (Phase 39c): without it, the NVIDIA driver (EGL, in
//!     the app and in the web process) reads the application profiles from
//!     `~/.nv/nvidia-application-profiles-rc*`, which change driver options. What remains
//!     is the read of `~/.nv/nvidia-application-profile-globals-rc` (only the `enabled`
//!     key, which the variable overrides; RA8).
//!
//! The values are not secret; the error messages carry only the variable name.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};

use crate::hardening::name_for_message;
use crate::process::{AllowedBinary, simple_absolute_path, verify_root_file};
use crate::role::Install;
use crate::sys;

/// `PATH` of the app.
pub const APP_PATH: &str = "/usr/local/bin:/usr/bin:/bin";

/// System GStreamer plugin dir (Ubuntu/Pop!_OS, multiarch).
#[cfg(target_arch = "x86_64")]
pub const SYSTEM_GST_PLUGINS: &str = "/usr/lib/x86_64-linux-gnu/gstreamer-1.0";
#[cfg(target_arch = "aarch64")]
pub const SYSTEM_GST_PLUGINS: &str = "/usr/lib/aarch64-linux-gnu/gstreamer-1.0";

/// GStreamer plugins the web process may initialize (Phase 39c): `package@file`,
/// separated by `:` (format of `GST_PLUGIN_LOADING_WHITELIST`; the file is a prefix of
/// the plugin path). Only the basic elements and `appsink`/`appsrc`, both from the system
/// dir; none of those that load code or open a channel from HOME or `/dev/shm`.
#[cfg(target_arch = "x86_64")]
pub const ALLOWED_GST_PLUGINS: &str = "gstreamer@/usr/lib/x86_64-linux-gnu/gstreamer-1.0/libgstcoreelements.so:gst-plugins-base@/usr/lib/x86_64-linux-gnu/gstreamer-1.0/libgstapp.so";
#[cfg(target_arch = "aarch64")]
pub const ALLOWED_GST_PLUGINS: &str = "gstreamer@/usr/lib/aarch64-linux-gnu/gstreamer-1.0/libgstcoreelements.so:gst-plugins-base@/usr/lib/aarch64-linux-gnu/gstreamer-1.0/libgstapp.so";

/// Fixed-value variables.
pub const FIXED_VARIABLES: &[(&str, &str)] = &[
    ("XDG_SESSION_TYPE", "wayland"),
    ("GDK_BACKEND", "wayland"),
    ("PATH", APP_PATH),
    ("GST_PLUGIN_SYSTEM_PATH", SYSTEM_GST_PLUGINS),
    ("NO_AT_BRIDGE", "1"),
    ("WEBKIT_A11Y_BUS_ADDRESS", "unix:path=/dev/null"),
    ("CUDA_MPS_PIPE_DIRECTORY", MISSING_MPS_DIR),
    ("GST_PLUGIN_LOADING_WHITELIST", ALLOWED_GST_PLUGINS),
    ("__GL_APPLICATION_PROFILE", "0"),
];

/// Dir of the NVIDIA MPS channel (D64): inside `/proc`, where no process, not even root,
/// creates a dir. Fixed: the MPS client does not find the channel and goes on without MPS.
pub const MISSING_MPS_DIR: &str = "/proc/ward-sem-mps";

/// Accepted locale variables (value from the daemon itself).
pub const LOCALE_VARIABLES: &[&str] = &[
    "LANG",
    "LC_ALL",
    "LC_ADDRESS",
    "LC_COLLATE",
    "LC_CTYPE",
    "LC_IDENTIFICATION",
    "LC_MEASUREMENT",
    "LC_MESSAGES",
    "LC_MONETARY",
    "LC_NAME",
    "LC_NUMERIC",
    "LC_PAPER",
    "LC_TELEPHONE",
    "LC_TIME",
];

/// Largest accepted locale value.
const MAX_LOCALE: usize = 64;
/// Largest number of digits in `wayland-<n>`.
const MAX_WAYLAND_DIGITS: usize = 9;

/// Environment of the app, in name order. The values are not secret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppEnvironment {
    vars: BTreeMap<&'static str, String>,
}

impl AppEnvironment {
    /// `(name, value)`, in name order.
    pub fn variables(&self) -> impl Iterator<Item = (&'static str, &str)> {
        self.vars.iter().map(|(k, v)| (*k, v.as_str()))
    }

    pub fn value(&self, name: &str) -> Option<&str> {
        self.vars.get(name).map(String::as_str)
    }
}

/// Session or environment outside D57. No value: only the variable name (sanitized when
/// it comes from outside).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionError {
    /// `/run/user/<uid>` is not a dir of the uid, 0700, without link, with the ones above
    /// not writable by others.
    UntrustedRuntime,
    /// No `wayland-<n>` socket of the uid.
    NoWayland,
    /// More than one `wayland-<n>`: no way to know which one is the compositor's.
    SeveralWayland,
    /// A `wayland-<n>` that is a link, is not a socket or is not the uid's.
    UntrustedWayland,
    /// `/run/user/<uid>/bus` missing, link, not a socket or of another owner.
    UntrustedBus,
    /// No user directory in `getpwuid_r`, or not in the form of a simple absolute path.
    HomeUnavailable,
    /// A required variable is missing.
    Missing(&'static str),
    /// A variable that is not from D57.
    Extra(String),
    /// A D57 variable with another value (or with an invalid locale value).
    Different(&'static str),
    /// The same variable twice in the environment.
    Repeated(String),
}

impl SessionError {
    /// The launcher does not open the app for lack of a single trusted Wayland session:
    /// the open-app operation (Phase 23) answers "approval unavailable" (D21).
    pub fn session_unavailable(&self) -> bool {
        !matches!(
            self,
            SessionError::Missing(_)
                | SessionError::Extra(_)
                | SessionError::Different(_)
                | SessionError::Repeated(_)
        )
    }
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::UntrustedRuntime => {
                f.write_str("a pasta da sessão (/run/user/<uid>) não é 0700 do usuário")
            }
            SessionError::NoWayland => f.write_str("nenhum socket Wayland da sessão"),
            SessionError::SeveralWayland => {
                f.write_str("mais de um socket Wayland na sessão; o app não escolhe")
            }
            SessionError::UntrustedWayland => {
                f.write_str("socket Wayland da sessão é link, não é socket ou é de outro dono")
            }
            SessionError::UntrustedBus => {
                f.write_str("barramento da sessão ausente, link ou de outro dono")
            }
            SessionError::HomeUnavailable => f.write_str("diretório do usuário indisponível"),
            SessionError::Missing(n) => write!(f, "variável {n} ausente no ambiente do app"),
            SessionError::Extra(n) => write!(f, "variável {n} não é do ambiente do app"),
            SessionError::Different(n) => write!(f, "variável {n} com valor fora do esperado"),
            SessionError::Repeated(n) => write!(f, "variável {n} repetida no ambiente"),
        }
    }
}

impl std::error::Error for SessionError {}

/// `/run/user/<uid>`.
fn session_dir(uid: u32) -> PathBuf {
    PathBuf::from(format!("/run/user/{uid}"))
}

/// Environment of the app for the uid of this process (the daemon): session verified on
/// disk, `HOME` from `getpwuid_r` and the locale of this process itself (invalid values
/// are left out). Nothing else from this process's environment gets in.
pub fn app_environment() -> Result<AppEnvironment, SessionError> {
    let uid = sys::geteuid();
    let home = sys::uid_home(uid).map_err(|_| SessionError::HomeUnavailable)?;
    let locale = LOCALE_VARIABLES
        .iter()
        .filter_map(|&name| {
            let v = std::env::var_os(name)?;
            valid_locale(v.as_bytes()).then(|| (name, v.to_string_lossy().into_owned()))
        })
        .collect();
    build(&session_dir(uid), uid, &home, locale)
}

/// Builds the environment from the session dir `rt`.
fn build(
    rt: &Path,
    uid: u32,
    home: &OsStr,
    locale: Vec<(&'static str, String)>,
) -> Result<AppEnvironment, SessionError> {
    verify_session_dir(rt, uid)?;
    let wayland = single_wayland_socket(rt, uid)?;
    let bus = rt.join("bus");
    if !uid_socket(&bus, uid) {
        return Err(SessionError::UntrustedBus);
    }
    let home = home.to_str().ok_or(SessionError::HomeUnavailable)?;
    if !simple_absolute_path(Path::new(home)) {
        return Err(SessionError::HomeUnavailable);
    }
    let rt_text = rt.to_str().ok_or(SessionError::UntrustedRuntime)?;
    let mut vars: BTreeMap<&'static str, String> = BTreeMap::new();
    vars.insert("XDG_RUNTIME_DIR", rt_text.to_owned());
    vars.insert("WAYLAND_DISPLAY", wayland);
    vars.insert(
        "DBUS_SESSION_BUS_ADDRESS",
        format!("unix:path={rt_text}/bus"),
    );
    vars.insert("HOME", home.to_owned());
    for &(name, value) in FIXED_VARIABLES {
        vars.insert(name, value.to_owned());
    }
    for (name, value) in locale {
        vars.insert(name, value);
    }
    Ok(AppEnvironment { vars })
}

/// Locale value: 1 to 64 bytes of `[A-Za-z0-9_.@-]`, not starting with `.` and without
/// `..` (never a path: glibc would load the locale from any dir). Also used for the
/// environment of `ward run` (Phase 26a).
pub fn valid_locale(v: &[u8]) -> bool {
    !v.is_empty()
        && v.len() <= MAX_LOCALE
        && v[0] != b'.'
        && !v.windows(2).any(|w| w == b"..")
        && v.iter()
            .all(|&b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'@' | b'-'))
}

/// The dir is the uid's, 0700 and not a link; the ones above are not links, belong to
/// root or to the uid and are not writable by group and others.
fn verify_session_dir(rt: &Path, uid: u32) -> Result<(), SessionError> {
    let bad = SessionError::UntrustedRuntime;
    if !simple_absolute_path(rt) {
        return Err(bad);
    }
    let m = std::fs::symlink_metadata(rt).map_err(|_| SessionError::UntrustedRuntime)?;
    if !m.file_type().is_dir() || m.uid() != uid || m.mode() & 0o777 != 0o700 {
        return Err(bad);
    }
    let mut current = rt.parent();
    while let Some(p) = current {
        let m = std::fs::symlink_metadata(p).map_err(|_| SessionError::UntrustedRuntime)?;
        if !m.file_type().is_dir() || (m.uid() != 0 && m.uid() != uid) || m.mode() & 0o022 != 0 {
            return Err(SessionError::UntrustedRuntime);
        }
        current = p.parent();
    }
    Ok(())
}

/// The path is a socket of the uid (without following links).
fn uid_socket(path: &Path, uid: u32) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_socket() && m.uid() == uid)
}

/// `wayland-<n>` (digits only, up to 9).
fn wayland_name(name: &[u8]) -> bool {
    name.strip_prefix(b"wayland-").is_some_and(|n| {
        !n.is_empty() && n.len() <= MAX_WAYLAND_DIGITS && n.iter().all(u8::is_ascii_digit)
    })
}

/// The name of the single `wayland-<n>` socket of the uid in `rt`. A link, a file that is
/// not a socket or of another owner with that name also refuses (fail closed).
fn single_wayland_socket(rt: &Path, uid: u32) -> Result<String, SessionError> {
    let mut found: Option<String> = None;
    let dir = std::fs::read_dir(rt).map_err(|_| SessionError::UntrustedRuntime)?;
    for item in dir {
        let item = item.map_err(|_| SessionError::UntrustedRuntime)?;
        let name = item.file_name();
        if !wayland_name(name.as_bytes()) {
            continue;
        }
        if !uid_socket(&item.path(), uid) {
            return Err(SessionError::UntrustedWayland);
        }
        if found.is_some() {
            return Err(SessionError::SeveralWayland);
        }
        found = Some(name.to_string_lossy().into_owned());
    }
    found.ok_or(SessionError::NoWayland)
}

/// App side (D57): verifies, before GTK, that the received environment has exactly the
/// D57 variables, with the values the daemon would build now (session dir, the single
/// Wayland socket, bus, `HOME` from `getpwuid_r`, fixed values) and only valid locale.
/// Any variable extra, missing, repeated or different refuses.
pub fn verify_app_environment<I>(received: I) -> Result<AppEnvironment, SessionError>
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    let uid = sys::geteuid();
    let home = sys::uid_home(uid).map_err(|_| SessionError::HomeUnavailable)?;
    verify_in(received, &session_dir(uid), uid, &home)
}

fn known_name(name: &[u8]) -> Option<&'static str> {
    const FROM_SESSION: &[&str] = &[
        "XDG_RUNTIME_DIR",
        "WAYLAND_DISPLAY",
        "DBUS_SESSION_BUS_ADDRESS",
        "HOME",
    ];
    FROM_SESSION
        .iter()
        .copied()
        .chain(FIXED_VARIABLES.iter().map(|(n, _)| *n))
        .chain(LOCALE_VARIABLES.iter().copied())
        .find(|n| n.as_bytes() == name)
}

fn verify_in<I>(
    received: I,
    rt: &Path,
    uid: u32,
    home: &OsStr,
) -> Result<AppEnvironment, SessionError>
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    let mut seen: BTreeMap<&'static str, OsString> = BTreeMap::new();
    for (name, value) in received {
        let Some(known) = known_name(name.as_bytes()) else {
            return Err(SessionError::Extra(name_for_message(name.as_bytes())));
        };
        if seen.insert(known, value).is_some() {
            return Err(SessionError::Repeated(known.to_owned()));
        }
    }
    let mut locale = Vec::new();
    for &name in LOCALE_VARIABLES {
        if let Some(v) = seen.get(name) {
            if !valid_locale(v.as_bytes()) {
                return Err(SessionError::Different(name));
            }
            locale.push((name, v.to_string_lossy().into_owned()));
        }
    }
    let expected = build(rt, uid, home, locale)?;
    for (name, value) in expected.variables() {
        match seen.get(name) {
            None => return Err(SessionError::Missing(name)),
            Some(v) if v.as_bytes() != value.as_bytes() => {
                return Err(SessionError::Different(name));
            }
            Some(_) => {}
        }
    }
    // Every seen name is known; a locale one that is not in the expected set already
    // refused above, and the others are required. Even so, equal count.
    if seen.len() != expected.vars.len() {
        return Err(SessionError::Extra("?".to_owned()));
    }
    Ok(expected)
}

/// Why the launcher did not open the app. No path nor value: may go to the log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LauncherError {
    /// There is already an instance opened by this launcher (one at a time, D57).
    AlreadyOpen,
    /// Session outside D57 ([`SessionError::session_unavailable`]: "approval
    /// unavailable", D21).
    Session(SessionError),
    /// The binary is not root's, is not setgid of the `ward-app` group, is writable by
    /// group or others, or there is a link in the path.
    UntrustedBinary,
    /// `fork`/`execve` failed.
    Io(io::ErrorKind),
}

impl fmt::Display for LauncherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LauncherError::AlreadyOpen => f.write_str("o app já está aberto"),
            LauncherError::Session(e) => write!(f, "aprovação indisponível: {e}"),
            LauncherError::UntrustedBinary => {
                f.write_str("o ward-app instalado não é do root com o grupo ward-app (setgid)")
            }
            LauncherError::Io(k) => write!(f, "não deu para abrir o app ({k})"),
        }
    }
}

impl std::error::Error for LauncherError {}

/// Opens the installed `ward-app` as a child of this process (the daemon). Keeps the
/// child: one instance at a time.
#[derive(Debug)]
pub struct Launcher {
    origin: Origin,
    child: Option<Child>,
    /// pidfd of the opened app (D63), taken right after the `fork`, with the child not
    /// yet collected (the pid cannot belong to another process). The app role requires
    /// this process.
    pidfd: Option<OwnedFd>,
    /// How the last collected app exited, until someone takes it
    /// ([`Launcher::take_exit_status`]).
    exit_status: Option<ExitStatus>,
}

/// What the launcher opens.
#[derive(Debug)]
enum Origin {
    /// The installed `ward-app`: root's, setgid of the `ward-app` group, D57 environment.
    Installed { binary: AllowedBinary, app_gid: u32 },
    /// Only in `test-mode` (D63): the `WARD_DEV_APP` of the test daemon.
    #[cfg(feature = "test-mode")]
    Test(TestBinary),
}

impl Launcher {
    pub fn new(binary: AllowedBinary, inst: &Install) -> Launcher {
        Launcher::from_origin(Origin::Installed {
            binary,
            app_gid: inst.app_gid(),
        })
    }

    /// Only in `test-mode` (D63): opens the test binary (the test `ward-app` or the
    /// bridge), verified again at every opening, with the output in the test log.
    #[cfg(feature = "test-mode")]
    pub fn for_test(binary: TestBinary) -> Launcher {
        Launcher::from_origin(Origin::Test(binary))
    }

    fn from_origin(origin: Origin) -> Launcher {
        Launcher {
            origin,
            child: None,
            pidfd: None,
            exit_status: None,
        }
    }

    /// The app opened by this launcher is still alive (and collects the one that exited,
    /// keeping how it exited for [`Launcher::take_exit_status`]).
    pub fn is_open(&mut self) -> bool {
        match self.child.as_mut().map(Child::try_wait) {
            Some(Ok(None)) => true,
            Some(r) => {
                if let Ok(Some(status)) = r {
                    self.exit_status = Some(status);
                }
                self.child = None;
                self.pidfd = None;
                false
            }
            None => false,
        }
    }

    /// Is process `pid` the app this launcher opened and that is still alive (D63)?
    /// Without collecting: the pid of the uncollected child does not go to another
    /// process, and the pidfd verifies that it has not exited.
    pub fn is_launched_app(&self, pid: u32) -> bool {
        match (&self.child, &self.pidfd) {
            (Some(f), Some(fd)) => {
                f.id() == pid && pid != 0 && sys::pidfd_alive(fd.as_fd()).unwrap_or(false)
            }
            _ => false,
        }
    }

    /// Pid of the app opened by this launcher (without collecting).
    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }

    /// How the last collected app exited (code or signal), only once. The app binds
    /// 0–2 to `/dev/null` (D57): the daemon records this in the log to know why it did
    /// not open.
    pub fn take_exit_status(&mut self) -> Option<ExitStatus> {
        self.is_open();
        self.exit_status.take()
    }

    /// Verifies the binary and the session, builds the environment from scratch and
    /// opens the app. Returns the pid of the app.
    pub fn open(&mut self) -> Result<u32, LauncherError> {
        if self.is_open() {
            return Err(LauncherError::AlreadyOpen);
        }
        let mut child = match &self.origin {
            Origin::Installed { binary, app_gid } => {
                verify_app_binary(binary, *app_gid)?;
                let env = app_environment().map_err(LauncherError::Session)?;
                launch(binary.path(), &env).map_err(|e| LauncherError::Io(e.kind()))?
            }
            #[cfg(feature = "test-mode")]
            Origin::Test(t) => t.launch()?,
        };
        let pid = child.id();
        // The child has not been collected yet: the pid is its own. Without a pidfd, the
        // app does not open (D63).
        let pidfd = match libc::pid_t::try_from(pid)
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))
            .and_then(sys::pidfd_open)
        {
            Ok(fd) => fd,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(LauncherError::Io(e.kind()));
            }
        };
        self.child = Some(child);
        self.pidfd = Some(pidfd);
        Ok(pid)
    }
}

/// Binary owned by root, not writable by group/others (the whole path), setgid of the
/// app group.
fn verify_app_binary(binary: &AllowedBinary, app_gid: u32) -> Result<(), LauncherError> {
    let meta = verify_root_file(binary.path()).map_err(|()| LauncherError::UntrustedBinary)?;
    if meta.mode() & 0o2000 == 0 || meta.gid() != app_gid {
        return Err(LauncherError::UntrustedBinary);
    }
    Ok(())
}

/// `fork` + `execve(path)` without a shell, with only `env`, dir `/`, 0–2 on `/dev/null`
/// and every fd ≥ 3 closed on `exec`.
fn launch(path: &Path, env: &AppEnvironment) -> io::Result<Child> {
    let mut cmd = Command::new(path);
    cmd.env_clear()
        .envs(env.variables())
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    sys::close_fds_on_exec(&mut cmd);
    cmd.spawn()
}

/// Only in `test-mode` (container, Phase 29a): the same `fork` + `execve` with the output
/// redirected, for the test to read what the test app prints. Does not verify the binary.
#[cfg(feature = "test-mode")]
pub fn launch_for_test(
    path: &Path,
    env: &AppEnvironment,
    output: std::fs::File,
) -> io::Result<Child> {
    let mut cmd = Command::new(path);
    cmd.env_clear()
        .envs(env.variables())
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(output.try_clone()?)
        .stderr(output);
    sys::close_fds_on_exec(&mut cmd);
    cmd.spawn()
}

/// Name of the Tauri test app: with it, the test launcher builds the D57 environment.
#[cfg(feature = "test-mode")]
pub const TEST_APP_NAME: &str = "ward-app";

/// `PATH` of the test binary that is not the Tauri app (the bridge).
#[cfg(feature = "test-mode")]
pub const TEST_PATH: &str = "/usr/bin:/bin";

/// Why the test binary was refused. No path (goes to the log).
#[cfg(feature = "test-mode")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestBinaryError {
    /// Relative path, with `.`, `..`, `//` or a name outside `[A-Za-z0-9._-]`.
    Path,
    /// Outside the repository (the dir above `WARD_DEV_DIR`).
    OutsideRepo,
    /// Symbolic link in the path, or something that is not a dir in the middle.
    Link,
    /// Not an executable regular file.
    NotFile,
    /// The file does not belong to the uid of this process.
    OtherOwner,
    /// File or dir of the path writable by others.
    Writable,
}

#[cfg(feature = "test-mode")]
impl fmt::Display for TestBinaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

#[cfg(feature = "test-mode")]
impl TestBinaryError {
    /// The message, without path.
    pub fn message(&self) -> &'static str {
        match self {
            TestBinaryError::Path => "caminho do app de teste fora da forma absoluta simples",
            TestBinaryError::OutsideRepo => "o app de teste não está dentro do repositório",
            TestBinaryError::Link => "link simbólico no caminho do app de teste",
            TestBinaryError::NotFile => "o app de teste não é arquivo regular executável",
            TestBinaryError::OtherOwner => "o app de teste não é do usuário do daemon",
            TestBinaryError::Writable => {
                "o app de teste (ou uma pasta dele) tem escrita para outros"
            }
        }
    }
}

/// Only in `test-mode` (D63): the binary the test daemon opens through the open-app
/// operation (`WARD_DEV_APP`: the test `ward-app` or `ward-test-bridge`). Nothing of it
/// comes from the request: path, environment and output come from the daemon
/// configuration.
///
/// - simple absolute path, inside `root` (the repository), executable regular file of
///   the uid of this process, not writable by others, no link in any part of the path
///   (verified on creation and again at every opening);
/// - the test `ward-app` gets the D57 environment ([`app_environment`]); any other (the
///   bridge, which opens no window) gets the minimal fixed environment: `HOME` from
///   `getpwuid_r`, `PATH=/usr/bin:/bin`, `LANG` of the daemon (only a locale name), the
///   verified `XDG_RUNTIME_DIR` and the `test-mode` variables the daemon passes;
/// - the output goes to `<logs>/<binary name>.log`, 0600, without following links.
#[cfg(feature = "test-mode")]
#[derive(Clone, Debug)]
pub struct TestBinary {
    path: PathBuf,
    root: PathBuf,
    logs: PathBuf,
    extras: Vec<(&'static str, String)>,
}

#[cfg(feature = "test-mode")]
impl TestBinary {
    /// `extras`: the `test-mode` variables that go to the minimal environment (e.g.
    /// `WARD_DEV_DIR`). They do not go into the environment of the test `ward-app` (exact
    /// D57).
    pub fn new(
        path: PathBuf,
        root: PathBuf,
        logs: PathBuf,
        extras: Vec<(&'static str, String)>,
    ) -> Result<TestBinary, TestBinaryError> {
        let b = TestBinary {
            path,
            root,
            logs,
            extras,
        };
        b.verify()?;
        Ok(b)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn name(&self) -> Result<&str, TestBinaryError> {
        let name = self
            .path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or(TestBinaryError::Path)?;
        let ok = !name.is_empty()
            && name.len() <= 64
            && !name.starts_with(['.', '-'])
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
        if ok {
            Ok(name)
        } else {
            Err(TestBinaryError::Path)
        }
    }

    /// The checks of the binary (see the type).
    pub fn verify(&self) -> Result<(), TestBinaryError> {
        let uid = sys::geteuid();
        if !simple_absolute_path(&self.path) || !simple_absolute_path(&self.root) {
            return Err(TestBinaryError::Path);
        }
        self.name()?;
        if self.path == self.root || !self.path.starts_with(&self.root) {
            return Err(TestBinaryError::OutsideRepo);
        }
        let m = std::fs::symlink_metadata(&self.path).map_err(|_| TestBinaryError::NotFile)?;
        if m.file_type().is_symlink() {
            return Err(TestBinaryError::Link);
        }
        if !m.file_type().is_file() || m.mode() & 0o100 == 0 {
            return Err(TestBinaryError::NotFile);
        }
        if m.uid() != uid {
            return Err(TestBinaryError::OtherOwner);
        }
        if m.mode() & 0o002 != 0 {
            return Err(TestBinaryError::Writable);
        }
        let mut current = self.path.parent();
        while let Some(p) = current {
            let m = std::fs::symlink_metadata(p).map_err(|_| TestBinaryError::Link)?;
            if !m.file_type().is_dir() {
                return Err(TestBinaryError::Link);
            }
            if m.mode() & 0o002 != 0 {
                return Err(TestBinaryError::Writable);
            }
            current = p.parent();
        }
        Ok(())
    }

    /// The environment of the test binary (see the type). Nothing from this process's
    /// environment besides the locale.
    pub fn environment(&self) -> Result<AppEnvironment, SessionError> {
        if self.name() == Ok(TEST_APP_NAME) {
            return app_environment();
        }
        let uid = sys::geteuid();
        let rt = session_dir(uid);
        verify_session_dir(&rt, uid)?;
        let home = sys::uid_home(uid).map_err(|_| SessionError::HomeUnavailable)?;
        let home = home.to_str().ok_or(SessionError::HomeUnavailable)?;
        if !simple_absolute_path(Path::new(home)) {
            return Err(SessionError::HomeUnavailable);
        }
        let mut vars: BTreeMap<&'static str, String> = BTreeMap::new();
        vars.insert("HOME", home.to_owned());
        vars.insert("PATH", TEST_PATH.to_owned());
        vars.insert(
            "XDG_RUNTIME_DIR",
            rt.to_str()
                .ok_or(SessionError::UntrustedRuntime)?
                .to_owned(),
        );
        if let Some(v) = std::env::var_os("LANG")
            && valid_locale(v.as_bytes())
        {
            vars.insert("LANG", v.to_string_lossy().into_owned());
        }
        for (k, v) in &self.extras {
            vars.insert(k, v.clone());
        }
        Ok(AppEnvironment { vars })
    }

    /// `<logs>/<name>.log`, 0600, of the uid, without following links; the dir is
    /// created 0700.
    fn open_log(&self) -> io::Result<std::fs::File> {
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
        let uid = sys::geteuid();
        let name = self
            .name()
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        match std::fs::DirBuilder::new().mode(0o700).create(&self.logs) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        let m = std::fs::symlink_metadata(&self.logs)?;
        if !m.file_type().is_dir() || m.uid() != uid {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        let f = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(self.logs.join(format!("{name}.log")))?;
        let m = f.metadata()?;
        if !m.file_type().is_file() || m.uid() != uid {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        Ok(f)
    }

    /// Verifies again and opens, with the output in the log.
    fn launch(&self) -> Result<Child, LauncherError> {
        self.verify().map_err(|_| LauncherError::UntrustedBinary)?;
        let env = self.environment().map_err(LauncherError::Session)?;
        let log = self.open_log().map_err(|e| LauncherError::Io(e.kind()))?;
        launch_for_test(&self.path, &env, log).map_err(|e| LauncherError::Io(e.kind()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
    use std::os::unix::net::UnixListener;

    /// Fake session dir in `/run/user/<uid>/` (0700), removed on drop.
    struct Session {
        rt: PathBuf,
        _listeners: Vec<UnixListener>,
    }

    impl Session {
        fn new(name: &str) -> Session {
            let uid = sys::geteuid();
            let rt = PathBuf::from(format!(
                "/run/user/{uid}/ward-lancador-teste-{}-{name}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&rt);
            fs::DirBuilder::new().mode(0o700).create(&rt).unwrap();
            let mut s = Session {
                rt,
                _listeners: Vec::new(),
            };
            s.socket("wayland-1");
            s.socket("bus");
            fs::write(s.rt.join("wayland-1.lock"), b"").unwrap();
            s
        }

        fn socket(&mut self, name: &str) {
            self._listeners
                .push(UnixListener::bind(self.rt.join(name)).unwrap());
        }

        fn build(&self) -> Result<AppEnvironment, SessionError> {
            build(
                &self.rt,
                sys::geteuid(),
                OsStr::new("/home/teste"),
                vec![("LANG", "pt_BR.UTF-8".to_owned())],
            )
        }

        fn verify(&self, vars: &[(&str, &str)]) -> Result<AppEnvironment, SessionError> {
            verify_in(
                vars.iter()
                    .map(|(k, v)| (OsString::from(k), OsString::from(v))),
                &self.rt,
                sys::geteuid(),
                OsStr::new("/home/teste"),
            )
        }

        /// The environment the daemon would build, as pairs.
        fn correct(&self) -> Vec<(String, String)> {
            self.build()
                .unwrap()
                .variables()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect()
        }
    }

    impl Drop for Session {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.rt);
        }
    }

    fn pairs(v: &[(String, String)]) -> Vec<(&str, &str)> {
        v.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect()
    }

    /// Phase 39c: each item of the GStreamer list is `package@<system dir>/libgst…`, and
    /// none is one of the plugins that load code from HOME or open a GPU channel.
    #[test]
    fn attack_gstreamer_list_only_has_system_plugins() {
        let items: Vec<_> = ALLOWED_GST_PLUGINS.split(':').collect();
        assert_eq!(items.len(), 2);
        for item in items {
            let (package, file) = item.split_once('@').expect(item);
            assert!(
                ["gstreamer", "gst-plugins-base"].contains(&package),
                "{item}"
            );
            let name = file
                .strip_prefix(SYSTEM_GST_PLUGINS)
                .and_then(|r| r.strip_prefix("/libgst"))
                .expect(item);
            assert!(name.ends_with(".so") && !name.contains('/'), "{item}");
            for dangerous in [
                "frei0r", "ladspa", "visual", "lv2", "nv", "cuda", "va", "qsv", "msdk",
            ] {
                assert!(!name.starts_with(dangerous), "{item}");
            }
        }
    }

    #[test]
    fn built_environment_has_only_the_d57_variables() {
        let s = Session::new("montar");
        let a = s.build().unwrap();
        let rt = s.rt.to_str().unwrap();
        assert_eq!(a.value("XDG_RUNTIME_DIR"), Some(rt));
        assert_eq!(a.value("WAYLAND_DISPLAY"), Some("wayland-1"));
        assert_eq!(
            a.value("DBUS_SESSION_BUS_ADDRESS"),
            Some(format!("unix:path={rt}/bus").as_str())
        );
        assert_eq!(a.value("HOME"), Some("/home/teste"));
        assert_eq!(a.value("PATH"), Some(APP_PATH));
        assert_eq!(a.value("GDK_BACKEND"), Some("wayland"));
        assert_eq!(a.value("GST_PLUGIN_SYSTEM_PATH"), Some(SYSTEM_GST_PLUGINS));
        assert_eq!(a.value("NO_AT_BRIDGE"), Some("1"));
        assert_eq!(
            a.value("WEBKIT_A11Y_BUS_ADDRESS"),
            Some("unix:path=/dev/null")
        );
        assert_eq!(
            a.value("CUDA_MPS_PIPE_DIRECTORY"),
            Some("/proc/ward-sem-mps")
        );
        assert_eq!(
            a.value("GST_PLUGIN_LOADING_WHITELIST"),
            Some(ALLOWED_GST_PLUGINS)
        );
        assert_eq!(a.value("__GL_APPLICATION_PROFILE"), Some("0"));
        assert_eq!(a.value("LANG"), Some("pt_BR.UTF-8"));
        for forbidden in [
            "DISPLAY",
            "LISTEN_PID",
            "LISTEN_FDS",
            "GTK_MODULES",
            "LD_PRELOAD",
        ] {
            assert_eq!(a.value(forbidden), None, "{forbidden}");
        }
        assert_eq!(a.variables().count(), 4 + FIXED_VARIABLES.len() + 1);
        // What the daemon builds passes the app's verification.
        assert_eq!(s.verify(&pairs(&s.correct())), Ok(a));
    }

    #[test]
    fn attack_two_wayland_sockets_or_link_does_not_open() {
        // V5.31: two `wayland-<n>`.
        let mut s = Session::new("dois");
        s.socket("wayland-0");
        assert_eq!(s.build(), Err(SessionError::SeveralWayland));
        assert!(SessionError::SeveralWayland.session_unavailable());
        // A link to a socket somewhere else (the attacker's proxy).
        let s = Session::new("link");
        fs::remove_file(s.rt.join("wayland-1")).unwrap();
        let mut proxy = Session::new("proxy");
        proxy.socket("proxy.sock");
        symlink(proxy.rt.join("proxy.sock"), s.rt.join("wayland-1")).unwrap();
        assert_eq!(s.build(), Err(SessionError::UntrustedWayland));
        // Plain file with the name.
        let s = Session::new("arquivo");
        fs::write(s.rt.join("wayland-7"), b"").unwrap();
        assert_eq!(s.build(), Err(SessionError::UntrustedWayland));
        // None.
        let s = Session::new("nenhum");
        fs::remove_file(s.rt.join("wayland-1")).unwrap();
        assert_eq!(s.build(), Err(SessionError::NoWayland));
        // `wayland-1.lock` and similar names do not count.
        let mut s = Session::new("parecidos");
        s.socket("wayland-x");
        s.socket("wayland-");
        s.socket("xwayland-2");
        assert!(s.build().is_ok());
    }

    #[test]
    fn attack_bus_or_session_dir_swapped() {
        let s = Session::new("bus");
        fs::remove_file(s.rt.join("bus")).unwrap();
        assert_eq!(s.build(), Err(SessionError::UntrustedBus));
        let mut proxy = Session::new("bus-proxy");
        proxy.socket("x");
        symlink(proxy.rt.join("x"), s.rt.join("bus")).unwrap();
        assert_eq!(s.build(), Err(SessionError::UntrustedBus));
        // Dir open to others, or through a link.
        let s = Session::new("aberta");
        fs::set_permissions(&s.rt, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(s.build(), Err(SessionError::UntrustedRuntime));
        let s = Session::new("alvo");
        let link = s.rt.with_file_name(format!(
            "ward-lancador-teste-{}-link-rt",
            std::process::id()
        ));
        let _ = fs::remove_file(&link);
        symlink(&s.rt, &link).unwrap();
        let r = build(&link, sys::geteuid(), OsStr::new("/home/x"), vec![]);
        fs::remove_file(&link).unwrap();
        assert_eq!(r, Err(SessionError::UntrustedRuntime));
        // `/tmp` (1777) is not a session dir either.
        assert_eq!(
            build(Path::new("/tmp"), sys::geteuid(), OsStr::new("/h"), vec![]),
            Err(SessionError::UntrustedRuntime)
        );
    }

    #[test]
    fn attack_environment_forced_on_the_app_is_refused() {
        // V5.31: the app opened by the daemon, but with variables forced in.
        let s = Session::new("conferir");
        let correct = s.correct();
        let change = |name: &str, value: &str| {
            let mut v = correct.clone();
            for (k, val) in &mut v {
                if k == name {
                    *val = value.to_owned();
                }
            }
            v
        };
        for (name, value) in [
            ("WAYLAND_DISPLAY", "/tmp/proxy.sock"),
            ("WAYLAND_DISPLAY", "../x"),
            ("WAYLAND_DISPLAY", "wayland-0"),
            ("DBUS_SESSION_BUS_ADDRESS", "unix:path=/tmp/bus"),
            ("XDG_RUNTIME_DIR", "/tmp/x"),
            ("HOME", "/tmp/casa-falsa"),
            ("PATH", "/tmp/bin:/usr/bin"),
            ("GST_PLUGIN_SYSTEM_PATH", "/home/teste/.local/gst"),
            ("NO_AT_BRIDGE", "0"),
            // D64: the MPS channel taken back to `/tmp`, where the attacker listens.
            ("CUDA_MPS_PIPE_DIRECTORY", "/tmp/nvidia-mps"),
            ("CUDA_MPS_PIPE_DIRECTORY", "/run/user/1000/mps"),
            ("CUDA_MPS_PIPE_DIRECTORY", ""),
            // Phase 39c: the GStreamer list opened again (the plugins that load code
            // from HOME) or pointed at a dir of the uid, and the NVIDIA profiles.
            ("GST_PLUGIN_LOADING_WHITELIST", ""),
            (
                "GST_PLUGIN_LOADING_WHITELIST",
                "gstreamer:gst-plugins-base:gst-plugins-bad",
            ),
            (
                "GST_PLUGIN_LOADING_WHITELIST",
                "gst-plugins-base@/home/teste/.local/share/gstreamer-1.0/plugins",
            ),
            ("__GL_APPLICATION_PROFILE", "1"),
            ("__GL_APPLICATION_PROFILE", ""),
            ("LANG", "../../tmp/locale"),
            ("LANG", "/tmp/locale"),
        ] {
            let v = change(name, value);
            assert!(
                matches!(s.verify(&pairs(&v)), Err(SessionError::Different(_))),
                "{name}={value}"
            );
        }
        // Extra variables that WebKitGTK, GTK, GIO and GStreamer would read.
        for extra in [
            "GTK_MODULES",
            "GTK3_MODULES",
            "GTK_PATH",
            "GST_PLUGIN_PATH",
            "GIO_EXTRA_MODULES",
            "GIO_MODULE_DIR",
            "GDK_PIXBUF_MODULE_FILE",
            "WEBKIT_INJECTED_BUNDLE_PATH",
            "XDG_DATA_DIRS",
            "XDG_CONFIG_HOME",
            "DISPLAY",
            "LISTEN_PID",
            "LISTEN_FDS",
            "LD_PRELOAD",
        ] {
            let mut v = correct.clone();
            v.push((extra.to_owned(), "/tmp/x".to_owned()));
            assert_eq!(
                s.verify(&pairs(&v)),
                Err(SessionError::Extra(extra.to_owned())),
                "{extra}"
            );
        }
        // One missing, or the same one twice (the first one correct).
        let mut v = correct.clone();
        v.retain(|(k, _)| k != "NO_AT_BRIDGE");
        assert_eq!(
            s.verify(&pairs(&v)),
            Err(SessionError::Missing("NO_AT_BRIDGE"))
        );
        // Phase 39c: without the list, the GStreamer scanner runs the `plugin_init` of
        // frei0r, ladspa and libvisual (code from HOME); without the other, the NVIDIA
        // profiles.
        for name in ["GST_PLUGIN_LOADING_WHITELIST", "__GL_APPLICATION_PROFILE"] {
            let mut v = correct.clone();
            v.retain(|(k, _)| k != name);
            assert_eq!(s.verify(&pairs(&v)), Err(SessionError::Missing(name)));
        }
        // D64: without the MPS variable, the NVIDIA client goes back to `/tmp/nvidia-mps`.
        let mut v = correct.clone();
        v.retain(|(k, _)| k != "CUDA_MPS_PIPE_DIRECTORY");
        assert_eq!(
            s.verify(&pairs(&v)),
            Err(SessionError::Missing("CUDA_MPS_PIPE_DIRECTORY"))
        );
        let mut v = correct.clone();
        v.push(("WAYLAND_DISPLAY".to_owned(), "/tmp/proxy.sock".to_owned()));
        assert_eq!(
            s.verify(&pairs(&v)),
            Err(SessionError::Repeated("WAYLAND_DISPLAY".to_owned()))
        );
        // An extra, valid locale gets in; the name with a control char comes out
        // sanitized.
        let mut v = correct.clone();
        v.push(("LC_TIME".to_owned(), "C.UTF-8".to_owned()));
        assert!(s.verify(&pairs(&v)).is_ok());
        let r = verify_in(
            [(OsString::from("X\x1b[2J"), OsString::from("1"))],
            &s.rt,
            sys::geteuid(),
            OsStr::new("/home/teste"),
        );
        assert_eq!(r, Err(SessionError::Extra("X".to_owned())));
    }

    #[test]
    fn locale_only_locale_name() {
        for ok in [
            "C",
            "C.UTF-8",
            "pt_BR.UTF-8",
            "en_US.utf8@euro",
            "sr_RS@latin",
        ] {
            assert!(valid_locale(ok.as_bytes()), "{ok}");
        }
        for bad in [
            "",
            "/tmp/x",
            "../x",
            ".x",
            "a..b",
            "pt BR",
            "a\nb",
            &"a".repeat(65),
        ] {
            assert!(!valid_locale(bad.as_bytes()), "{bad:?}");
        }
    }

    #[test]
    fn home_comes_from_passwd() {
        let h = sys::uid_home(sys::geteuid()).unwrap();
        assert!(h.as_bytes().starts_with(b"/"));
        assert!(sys::uid_home(u32::MAX - 7).is_err());
    }

    #[test]
    fn attack_launcher_passes_neither_environment_nor_fd_of_the_daemon() {
        // The environment of this process (with whatever it has: `CARGO_*`, `PATH`, …)
        // does not reach the child, and an fd opened without `O_CLOEXEC` does not either.
        let s = Session::new("filho");
        let env = s.build().unwrap();
        let inheritable_fd = fs::File::open("/dev/null").unwrap();
        sys::clear_cloexec(inheritable_fd.as_fd()).unwrap();
        assert!(!sys::has_cloexec(inheritable_fd.as_fd()).unwrap());
        let output = s.rt.join("saida");
        let file = fs::File::create(&output).unwrap();
        let mut cmd = Command::new("/usr/bin/sh");
        cmd.arg("-c")
            .arg("env; echo ---; ls /proc/self/fd; echo ---; pwd")
            .env_clear()
            .envs(env.variables())
            .current_dir("/")
            .stdin(Stdio::null())
            .stdout(file);
        sys::close_fds_on_exec(&mut cmd);
        assert!(cmd.status().unwrap().success());
        let text = fs::read_to_string(&output).unwrap();
        let parts: Vec<&str> = text.split("---\n").collect();
        let mut names: Vec<&str> = parts[0]
            .lines()
            .filter_map(|l| l.split('=').next())
            .filter(|n| !matches!(*n, "PWD" | "SHLVL" | "_" | "OLDPWD"))
            .collect();
        names.sort_unstable();
        let mut expected: Vec<&str> = env.variables().map(|(k, _)| k).collect();
        expected.sort_unstable();
        assert_eq!(names, expected);
        // Only 0, 1, 2 and the fd of `ls` itself.
        let fds: Vec<&str> = parts[1].lines().collect();
        assert!(fds.len() <= 4, "fds herdados: {fds:?}");
        assert_eq!(parts[2].trim(), "/");
        drop(inheritable_fd);
    }

    #[test]
    fn launch_sets_dev_null_and_closes_fds() {
        let s = Session::new("lancar");
        let env = s.build().unwrap();
        let mut child = launch(Path::new("/usr/bin/true"), &env).unwrap();
        assert!(child.wait().unwrap().success());
        // Missing program: the `exec` error reaches the parent (the `std` channel was not
        // closed by `close_range`).
        assert!(launch(Path::new("/usr/bin/ward-nao-existe-falso"), &env).is_err());
    }

    /// Pending item of 29a: the exit code of the app (which has 0–2 on `/dev/null`) stays
    /// with the launcher until the daemon takes it and records it; only once. While alive,
    /// nothing.
    #[test]
    fn app_exit_status_is_kept_for_the_log() {
        let s = Session::new("saida");
        let env = s.build().unwrap();
        let mut l = Launcher::from_origin(Origin::Installed {
            binary: AllowedBinary::new("/usr/bin/false").unwrap(),
            app_gid: 4_000_000,
        });
        assert_eq!(l.take_exit_status(), None);
        l.child = Some(launch(Path::new("/usr/bin/false"), &env).unwrap());
        while l.is_open() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let st = l.take_exit_status().expect("saída guardada");
        assert_eq!(st.code(), Some(1));
        assert_eq!(l.take_exit_status(), None);
        // Killed by a signal (`yes` does not exit by itself with the output on `/dev/null`).
        use std::os::unix::process::ExitStatusExt;
        l.child = Some(
            launch(Path::new("/usr/bin/yes"), &env)
                .map(|mut c| {
                    c.kill().unwrap();
                    c
                })
                .unwrap(),
        );
        let st = loop {
            if let Some(st) = l.take_exit_status() {
                break st;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        assert_eq!(st.signal(), Some(9));
    }

    /// V3.36 (D63): only the process the launcher opened, alive, is "the launched app".
    /// Another pid (another child of the daemon, same group and same parent) is not; after
    /// the app dies, not even its pid.
    #[test]
    fn attack_only_the_launcher_pid_is_the_app() {
        let s = Session::new("pidfd");
        let env = s.build().unwrap();
        let mut l = Launcher::from_origin(Origin::Installed {
            binary: AllowedBinary::new("/usr/bin/yes").unwrap(),
            app_gid: 4_000_000,
        });
        assert!(!l.is_launched_app(std::process::id()));
        let child = launch(Path::new("/usr/bin/yes"), &env).unwrap();
        let pid = child.id();
        let fd = sys::pidfd_open(libc::pid_t::try_from(pid).unwrap()).unwrap();
        l.child = Some(child);
        l.pidfd = Some(fd);
        // Another child of this process (as a `ward run` would be), alive at the same time.
        let mut other = launch(Path::new("/usr/bin/yes"), &env).unwrap();
        assert!(l.is_launched_app(pid));
        assert!(!l.is_launched_app(other.id()));
        assert!(!l.is_launched_app(0));
        other.kill().unwrap();
        other.wait().unwrap();
        // The app dies: collected, the pid stops being the app's.
        l.child.as_mut().unwrap().kill().unwrap();
        while l.is_open() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!l.is_launched_app(pid));
        assert!(l.pidfd.is_none());
    }

    #[test]
    fn attack_user_binary_or_without_setgid_is_refused() {
        let inst_gid = 4_000_000;
        let me = AllowedBinary::new(std::env::current_exe().unwrap()).unwrap();
        assert_eq!(
            verify_app_binary(&me, inst_gid),
            Err(LauncherError::UntrustedBinary)
        );
        // Root's, but without setgid (`/usr/bin/true`).
        let t = AllowedBinary::new("/usr/bin/true").unwrap();
        assert_eq!(
            verify_app_binary(&t, inst_gid),
            Err(LauncherError::UntrustedBinary)
        );
    }

    #[test]
    fn messages_without_value() {
        let m = SessionError::Different("WAYLAND_DISPLAY").to_string();
        assert!(m.contains("WAYLAND_DISPLAY") && !m.contains("/tmp"));
        let m = LauncherError::Session(SessionError::SeveralWayland).to_string();
        assert!(m.starts_with("aprovação indisponível"));
    }

    /// Fake repository for the test binary (D63), in `/run/user/<uid>/` (0700), removed
    /// on drop.
    #[cfg(feature = "test-mode")]
    struct Repo(PathBuf);

    #[cfg(feature = "test-mode")]
    impl Repo {
        fn new(name: &str) -> Repo {
            let p = PathBuf::from(format!(
                "/run/user/{}/ward-lancador-repo-{}-{name}",
                sys::geteuid(),
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&p);
            fs::DirBuilder::new().mode(0o700).create(&p).unwrap();
            fs::DirBuilder::new()
                .mode(0o700)
                .create(p.join("bin"))
                .unwrap();
            fs::DirBuilder::new()
                .mode(0o700)
                .create(p.join(".dev"))
                .unwrap();
            Repo(p)
        }

        /// Executable script of the uid in `bin/<name>`, closed before any `exec`.
        fn script(&self, name: &str, body: &str) -> PathBuf {
            let c = self.0.join("bin").join(name);
            fs::write(&c, format!("#!/bin/sh\n{body}\n")).unwrap();
            fs::set_permissions(&c, fs::Permissions::from_mode(0o700)).unwrap();
            c
        }

        fn binary(&self, c: &Path) -> Result<TestBinary, TestBinaryError> {
            TestBinary::new(
                c.to_path_buf(),
                self.0.clone(),
                self.0.join(".dev/logs"),
                vec![("WARD_DEV_DIR", self.0.join(".dev").display().to_string())],
            )
        }
    }

    #[cfg(feature = "test-mode")]
    impl Drop for Repo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Opens and waits for the exit. Another test in parallel that does `fork` may hold
    /// the write fd of the script for an instant (ETXTBSY): tries again.
    #[cfg(feature = "test-mode")]
    fn open_and_wait(l: &mut Launcher) -> (u32, ExitStatus) {
        let pid = (0..50)
            .find_map(|_| match l.open() {
                Ok(pid) => Some(pid),
                Err(LauncherError::Io(io::ErrorKind::ExecutableFileBusy)) => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    None
                }
                Err(e) => panic!("{e}"),
            })
            .expect("ETXTBSY o tempo todo");
        loop {
            if let Some(st) = l.take_exit_status() {
                return (pid, st);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    #[cfg(feature = "test-mode")]
    fn attack_test_binary_outside_the_rules_is_refused() {
        use TestBinaryError as E;
        let r = Repo::new("regras");
        let good = r.script("ward-test-bridge", "exit 0");
        assert!(r.binary(&good).is_ok());
        // Outside the repository (the root does not count), relative, with `..`, odd name.
        let outside = TestBinary::new(
            good.clone(),
            r.0.join("bin/outra"),
            r.0.join(".dev/logs"),
            vec![],
        );
        assert_eq!(outside.err(), Some(E::OutsideRepo));
        assert_eq!(r.binary(&r.0).err(), Some(E::OutsideRepo));
        assert_eq!(r.binary(Path::new("bin/x")).err(), Some(E::Path));
        assert_eq!(
            r.binary(&r.0.join("bin/../bin/ward-test-bridge")).err(),
            Some(E::Path)
        );
        let odd = r.script("-x", "exit 0");
        assert_eq!(r.binary(&odd).err(), Some(E::Path));
        // Link to the binary, and link in a dir of the path.
        symlink(&good, r.0.join("bin/link")).unwrap();
        assert_eq!(r.binary(&r.0.join("bin/link")).err(), Some(E::Link));
        symlink(r.0.join("bin"), r.0.join("pasta-link")).unwrap();
        assert_eq!(
            r.binary(&r.0.join("pasta-link/ward-test-bridge")).err(),
            Some(E::Link)
        );
        // Of another owner (the system `true`, with the root in `/usr/bin`).
        let root = TestBinary::new(
            PathBuf::from("/usr/bin/true"),
            PathBuf::from("/usr/bin"),
            r.0.join(".dev/logs"),
            vec![],
        );
        assert_eq!(root.err(), Some(E::OtherOwner));
        // Not executable, dir and writable by others.
        let text = r.0.join("bin/texto");
        fs::write(&text, b"x").unwrap();
        fs::set_permissions(&text, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(r.binary(&text).err(), Some(E::NotFile));
        assert_eq!(r.binary(&r.0.join("bin")).err(), Some(E::NotFile));
        fs::set_permissions(&good, fs::Permissions::from_mode(0o702)).unwrap();
        assert_eq!(r.binary(&good).err(), Some(E::Writable));
        fs::set_permissions(&good, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(r.0.join("bin"), fs::Permissions::from_mode(0o703)).unwrap();
        assert_eq!(r.binary(&good).err(), Some(E::Writable));
        fs::set_permissions(r.0.join("bin"), fs::Permissions::from_mode(0o700)).unwrap();
        // Also verified at every opening: swapped for a link after being accepted, does
        // not open.
        let b = r.binary(&good).unwrap();
        fs::remove_file(&good).unwrap();
        symlink("/usr/bin/true", &good).unwrap();
        let mut l = Launcher::for_test(b);
        assert_eq!(l.open(), Err(LauncherError::UntrustedBinary));
    }

    #[test]
    #[cfg(feature = "test-mode")]
    fn test_bridge_gets_the_minimal_environment_and_the_log_is_0600() {
        let r = Repo::new("ambiente");
        let c = r.script("ward-test-bridge", "/usr/bin/env; echo argumentos=$#");
        let b = r.binary(&c).unwrap();
        let mut l = Launcher::for_test(b);
        let (pid, st) = open_and_wait(&mut l);
        assert!(st.success());
        assert!(!l.is_launched_app(pid), "o pid morto continua app");
        let log = r.0.join(".dev/logs/ward-test-bridge.log");
        let m = fs::symlink_metadata(&log).unwrap();
        assert_eq!(m.mode() & 0o777, 0o600);
        let text = fs::read_to_string(&log).unwrap();
        let mut names: Vec<&str> = text
            .lines()
            .filter_map(|l| l.split_once('=').map(|(k, _)| k))
            .filter(|k| *k != "argumentos" && *k != "PWD")
            .collect();
        names.sort_unstable();
        let mut expected = vec!["HOME", "PATH", "WARD_DEV_DIR", "XDG_RUNTIME_DIR"];
        if std::env::var_os("LANG").is_some_and(|v| valid_locale(v.as_bytes())) {
            expected.push("LANG");
        }
        expected.sort_unstable();
        assert_eq!(names, expected, "{text}");
        assert!(text.contains("PATH=/usr/bin:/bin\n"));
        assert!(
            text.contains("argumentos=0\n"),
            "argumentos do pedido chegaram"
        );
        // The log as a link to the outside: does not open (nothing is written there).
        fs::remove_file(&log).unwrap();
        let target = r.0.join("alvo.txt");
        fs::write(&target, b"").unwrap();
        symlink(&target, &log).unwrap();
        assert!(matches!(l.open(), Err(LauncherError::Io(_))));
        assert_eq!(fs::read(&target).unwrap(), b"");
    }

    #[test]
    #[cfg(feature = "test-mode")]
    fn test_app_gets_the_d57_environment() {
        let r = Repo::new("d57");
        let c = r.script(TEST_APP_NAME, "exit 0");
        let b = r.binary(&c).unwrap();
        // Without a single Wayland session on this host, both refuse the same way (D21).
        match (b.environment(), app_environment()) {
            (Ok(a), Ok(d57)) => {
                assert_eq!(a, d57);
                assert!(a.value("WARD_DEV_DIR").is_none());
            }
            (Err(a), Err(d57)) => assert_eq!(a, d57),
            (a, d57) => panic!("{a:?} × {d57:?}"),
        }
    }
}
