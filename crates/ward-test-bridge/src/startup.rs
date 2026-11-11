//! Startup of the bridge against the test daemon (D63, Phase 23a).
//!
//! The bridge only gets the app role when the test daemon opens it through the launcher
//! (the open-app operation with `WARD_DEV_APP` pointing to it): the daemon's `test-mode`
//! does not require the group egid, but it requires the pid the launcher opened and
//! parent = daemon. So:
//! - it connects **once** ([`DaemonSocket`], no lazy connection) and never reconnects;
//! - the first request is the status one, and the bridge checks the role in the reply.
//!   Without `app` (run by hand, a copy, another parent), it exits with [`NO_APP_ROLE`]
//!   before opening the HTTP port: the interface never gets to talk to an agent
//!   connection.
//!
//! Opened by the daemon, the bridge receives no arguments (nothing of the request reaches
//! it) and the environment is the fixed minimum of the test launcher; the test-mode build
//! of the interface comes from `WARD_DEV_DIR` (`<repository>/.dev` →
//! `<repository>/app/<test build>`).

use std::ffi::OsString;
use std::path::PathBuf;

use ward_app_core::cliente;
use ward_app_core::daemon::DaemonSocket;

/// Message for when the daemon did not give the app role (V3.38).
pub const NO_APP_ROLE: &str = "o daemon não deu o papel app";

/// Connects (once) and checks the role in the first reply. The error message is fixed.
pub fn connect_with_app_role(mut daemon: DaemonSocket) -> Result<DaemonSocket, String> {
    daemon.conectar_agora().map_err(|e| e.to_string())?;
    match cliente::tem_papel_app(&mut daemon) {
        Ok(true) => Ok(daemon),
        Ok(false) => Err(NO_APP_ROLE.to_owned()),
        Err(e) => Err(e.to_string()),
    }
}

/// The test-mode build of the bridge opened by the daemon, under `<WARD_DEV_DIR>/../app/`.
/// `WARD_DEV_DIR` must be absolute, without `.`/`..`, and end in `/.dev`.
pub fn repository_dist(dev: Option<OsString>) -> Option<PathBuf> {
    Some(repository(dev)?.join("app").join("dist-teste"))
}

/// The repository of `WARD_DEV_DIR` (`<repository>/.dev`), with the same rule as
/// [`repository_dist`].
pub fn repository(dev: Option<OsString>) -> Option<PathBuf> {
    use std::path::Component;
    let dev = PathBuf::from(dev?);
    let clean = dev.is_absolute()
        && dev
            .components()
            .all(|c| matches!(c, Component::RootDir | Component::Normal(_)));
    if !clean || dev.file_name()? != ".dev" {
        return None;
    }
    Some(dev.parent()?.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dist_only_from_absolute_dev() {
        assert_eq!(
            repository_dist(Some("/r/x/.dev".into())),
            Some(PathBuf::from("/r/x/app/dist-teste"))
        );
        assert_eq!(
            repository(Some("/r/x/.dev".into())),
            Some(PathBuf::from("/r/x"))
        );
        for bad in ["", "rel/.dev", "/r/../.dev", "/r/x/dev", "/.dev/.."] {
            assert_eq!(repository_dist(Some(bad.into())), None, "{bad}");
            assert_eq!(repository(Some(bad.into())), None, "{bad}");
        }
        assert_eq!(repository_dist(None), None);
    }
}
