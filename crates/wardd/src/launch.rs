//! The daemon opens the app (D57) through the `ward-platform` launcher (Phase 29a):
//! `fork` + `execve` of the installed `ward-app`, environment built from scratch, one
//! instance at a time.
//!
//! The app has 0–2 on `/dev/null`, so its "did not open" message shows up nowhere. That
//! is why a watcher reaps the child and records in the daemon log how it exited (code or
//! signal; leftover from 29a). Nothing from the request goes into the app's
//! environment: the open-app operation has no field.
//!
//! Test daemon (D63, Phase 23a): the opener opens the test binary from the configuration
//! (the test `ward-app` or the bridge) through the same launcher, and [`TestApp`] gives
//! that process the app role **without** the egid of the group (the launcher's pid and
//! parent = daemon are still required). In the release, [`TestApp`] has no possible
//! value: the type can only be built with the `test-mode` feature.

use std::os::unix::process::ExitStatusExt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ward_platform::{AllowedBinary, Install, Launcher, LauncherError, PeerClass, PeerProcess};

use crate::approval::{AppOpener, OpenAppError};
use crate::warning;

/// Fixed path of the installed app (D18).
pub const INSTALLED_APP: &str = "/usr/local/bin/ward-app";

/// How often the watcher checks whether the app exited.
const WATCH_STEP: Duration = Duration::from_millis(250);

/// Opens the installed `ward-app` through the [`Launcher`].
pub struct RealOpener {
    launcher: Arc<Mutex<Launcher>>,
}

impl RealOpener {
    /// With the installed binary and the install groups.
    pub fn installed(inst: &Install) -> Option<Self> {
        let bin = AllowedBinary::new(INSTALLED_APP).ok()?;
        Some(Self {
            launcher: Arc::new(Mutex::new(Launcher::new(bin, inst))),
        })
    }
}

impl RealOpener {
    /// Only in the test daemon (D63): opens the test binary checked by the configuration.
    #[cfg(feature = "test-mode")]
    pub fn for_test(binary: ward_platform::TestBinary) -> Self {
        Self {
            launcher: Arc::new(Mutex::new(Launcher::for_test(binary))),
        }
    }
}

/// Whoever the launcher opened (D63): the app role only holds for that process, alive.
#[derive(Clone)]
pub struct LaunchedApp(Arc<Mutex<Launcher>>);

impl LaunchedApp {
    /// Is `pid` the app that the launcher opened and that is still alive?
    pub fn is_the_app(&self, pid: u32) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_launched_app(pid)
    }
}

impl RealOpener {
    /// For classifying the peers (D63).
    pub fn launched_app(&self) -> LaunchedApp {
        LaunchedApp(self.launcher.clone())
    }
}

/// The test app (D63): the process that the test launcher opened gets the app role
/// without the egid of the `ward-app` group. Without the `test-mode` feature, the type
/// has no value (`Infallible`): the release has no way to give the app role without the
/// group.
pub struct TestApp {
    #[cfg(feature = "test-mode")]
    app: LaunchedApp,
    #[cfg(not(feature = "test-mode"))]
    never: std::convert::Infallible,
}

impl TestApp {
    #[cfg(feature = "test-mode")]
    pub fn new(app: LaunchedApp) -> Self {
        Self { app }
    }

    /// Is `pid` the app that the test launcher opened, alive?
    pub fn is_the_app(&self, pid: u32) -> bool {
        #[cfg(feature = "test-mode")]
        return self.app.is_the_app(pid);
        #[cfg(not(feature = "test-mode"))]
        {
            let _ = pid;
            match self.never {}
        }
    }

    /// The peer's class without the egid (same uid, no debugger, parent = daemon).
    pub fn classify(&self, peer: &PeerProcess) -> PeerClass {
        #[cfg(feature = "test-mode")]
        return peer.classify_test_app();
        #[cfg(not(feature = "test-mode"))]
        {
            let _ = peer;
            match self.never {}
        }
    }
}

/// The extension's test host (Phase 41, exception of the D63 kind): in the test daemon
/// with `WARD_DEV_EXTENSION=chrome`, a peer whose parent is the installed Chrome (checked
/// through `/proc`, as in the release) gets the extension role **without** the egid of
/// the `ward-nm` group. Without the `test-mode` feature, the type has no value
/// (`Infallible`): the release has no way to give the extension role without the group.
pub struct TestExtension {
    #[cfg(feature = "test-mode")]
    chrome: AllowedBinary,
    #[cfg(not(feature = "test-mode"))]
    never: std::convert::Infallible,
}

impl TestExtension {
    /// With the installed Chrome (the same parent that the release accepts).
    #[cfg(feature = "test-mode")]
    pub fn new(chrome: AllowedBinary) -> Self {
        Self { chrome }
    }

    /// With the Chrome installed at `chrome`, if it exists; `None` without the feature
    /// (in the release, there is never an exception).
    pub fn from_chrome(chrome: &str) -> Option<Self> {
        #[cfg(feature = "test-mode")]
        return AllowedBinary::new(chrome).ok().map(Self::new);
        #[cfg(not(feature = "test-mode"))]
        {
            let _ = chrome;
            None
        }
    }

    /// The peer's class without the egid (same uid, kernel policy, no debugger, parent =
    /// installed Chrome).
    pub fn classify(&self, peer: &PeerProcess) -> PeerClass {
        #[cfg(feature = "test-mode")]
        return peer
            .classify_test_extension_host(&self.chrome, &ward_platform::KernelPolicy::read());
        #[cfg(not(feature = "test-mode"))]
        {
            let _ = peer;
            match self.never {}
        }
    }
}

/// The log line for how the app exited (with nothing but numbers).
pub fn describe_exit(pid: u32, st: std::process::ExitStatus) -> String {
    match (st.code(), st.signal()) {
        (Some(c), _) => format!("app (pid {pid}) saiu com código {c}"),
        (None, Some(s)) => format!("app (pid {pid}) encerrado pelo sinal {s}"),
        _ => format!("app (pid {pid}) saiu"),
    }
}

impl AppOpener for RealOpener {
    fn open(&self, on_exit: Box<dyn FnOnce() + Send>) -> Result<(), OpenAppError> {
        let mut l = self.launcher.lock().unwrap_or_else(|e| e.into_inner());
        let pid = match l.open() {
            Ok(pid) => pid,
            Err(LauncherError::AlreadyOpen) => return Err(OpenAppError::AlreadyOpen),
            Err(e) => {
                // `LauncherError` carries no path or value (only variable names).
                warning!("app não aberto: {e}");
                return Err(OpenAppError::Unavailable);
            }
        };
        drop(l);
        warning!("app aberto (pid {pid})");
        let launcher = self.launcher.clone();
        let watcher = std::thread::Builder::new()
            .name("wardd-vigia-app".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(WATCH_STEP);
                    let mut l = launcher.lock().unwrap_or_else(|e| e.into_inner());
                    // Another pid: this app was already reaped by a following opening.
                    if !l.is_open() || l.pid() != Some(pid) {
                        match l.take_exit_status() {
                            Some(st) => warning!("{}", describe_exit(pid, st)),
                            None => warning!("app (pid {pid}) saiu"),
                        }
                        break;
                    }
                }
                on_exit();
            });
        if watcher.is_err() {
            warning!("não deu para criar a vigia do app");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn exit_goes_to_the_log_only_with_numbers() {
        let st = Command::new("/usr/bin/false").status().unwrap();
        assert_eq!(describe_exit(7, st), "app (pid 7) saiu com código 1");
        let mut c = Command::new("/usr/bin/yes")
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        c.kill().unwrap();
        let st = c.wait().unwrap();
        assert_eq!(describe_exit(8, st), "app (pid 8) encerrado pelo sinal 9");
    }
}
