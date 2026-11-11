//! Daemon paths.
//!
//! - **Release:** vault and state in `$HOME/.local/share/ward/`, under the 0.1.x file
//!   names, which never change (D39, m2:PRD rule 11). `HOME` comes from the system systemd unit
//!   (`User=`), not from the session (D46); it must be absolute and without `..`. Socket
//!   only through the systemd `LISTEN_FDS`, at `/run/ward/<uid>/wardd.sock`.
//! - **`test-mode`** (D7): vault `$WARD_DEV_DIR/test-vault.ward` and state
//!   `$WARD_DEV_DIR/test-vault.state`; `WARD_DEV_DIR` is required, so the test daemon
//!   never touches `~/.local/share/ward/`. Without `LISTEN_FDS`, the socket lives in a
//!   0700 directory of the user: `/run/user/<uid>/ward-teste/` or `WARD_DEV_SOCKET_DIR`.
//!   With `WARD_DEV_APP` (D63, Phase 23a), the open-app operation opens that binary (the
//!   test `ward-app` or the bridge): absolute path of a regular file of the uid, without
//!   a link, inside the repository (the folder above `WARD_DEV_DIR`), checked here and
//!   on every opening; the output goes to `$WARD_DEV_DIR/logs/<binary>.log` (0600).
//!   With `WARD_DEV_EXTENSAO=chrome` (Phase 41), the peer that is a child of the
//!   installed Chrome gets the extension role without the egid of the `ward-nm` group
//!   (see [`crate::launch::TestExtension`]).
//!   The release build reads no `WARD_DEV_*` variable.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use crate::core::Paths;

/// Configuration read from the environment.
#[derive(Debug, Clone)]
pub struct Config {
    pub paths: Paths,
    /// Socket directory of the test daemon (`None` in the release).
    pub test_socket_dir: Option<PathBuf>,
    /// The binary that the test daemon's open-app operation opens (D63).
    #[cfg(feature = "test-mode")]
    pub test_app: Option<ward_platform::TestBinary>,
    /// `WARD_DEV_EXTENSAO=chrome` (Phase 41): the test daemon gives the extension role to
    /// the peer that is a child of the installed Chrome without the egid of the `ward-nm`
    /// group. Any other value is refused at startup.
    #[cfg(feature = "test-mode")]
    pub test_extension: bool,
    /// `<rt>` (Phase 26, S2): the `ward run` temp files live in `<rt>tmp/`. Release:
    /// `/run/user/<uid>/ward/` (by the effective uid, not by `XDG_RUNTIME_DIR`); test
    /// daemon: the test socket directory.
    pub rt: PathBuf,
}

/// `WARD_DEV_APP` (D63): checked as a [`ward_platform::TestBinary`], inside the
/// repository (the folder above `dev`). The bridge's minimal environment carries
/// `WARD_DEV_DIR` and, if it came, `WARD_DEV_SOCKET_DIR`: they are the `test-mode`
/// variables, from the daemon configuration, never from the request.
#[cfg(feature = "test-mode")]
fn test_app_binary(
    v: OsString,
    dev: &Path,
    socket: Option<&Path>,
) -> Result<ward_platform::TestBinary, &'static str> {
    let path = clean_absolute(v).ok_or("daemon de teste: WARD_DEV_APP inválido")?;
    let root = dev
        .parent()
        .filter(|r| r.parent().is_some())
        .ok_or("daemon de teste: WARD_DEV_DIR sem repositório acima")?;
    let text = |p: &Path| {
        p.to_str()
            .map(str::to_owned)
            .ok_or("daemon de teste: caminho fora de UTF-8")
    };
    let mut extras = vec![("WARD_DEV_DIR", text(dev)?)];
    if let Some(s) = socket {
        extras.push(("WARD_DEV_SOCKET_DIR", text(s)?));
    }
    ward_platform::TestBinary::new(path, root.to_path_buf(), dev.join("logs"), extras)
        .map_err(|e| e.message())
}

/// Absolute path without `.`, `..` or `//`, checked on the bytes (Rust's `components()`
/// drops `.` and `//` in the middle, so it alone is not enough).
fn clean_absolute(v: OsString) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let p = PathBuf::from(v);
    let bytes = p.as_os_str().as_bytes();
    let segments_ok = bytes.strip_prefix(b"/").is_some_and(|rest| {
        rest.split(|b| *b == b'/')
            .all(|s| !s.is_empty() && s != b"." && s != b"..")
    });
    let clean = segments_ok
        && p.components()
            .all(|c| matches!(c, Component::RootDir | Component::Normal(_)));
    clean.then_some(p)
}

impl Config {
    /// Reads from the environment. The error message carries no variable value.
    pub fn from_env() -> Result<Config, &'static str> {
        Self::from_vars(|k| std::env::var_os(k))
    }

    #[cfg(not(feature = "test-mode"))]
    fn from_vars(var: impl Fn(&str) -> Option<OsString>) -> Result<Config, &'static str> {
        let home = var("HOME")
            .and_then(clean_absolute)
            .ok_or("HOME ausente ou não absoluto")?;
        let dir = home.join(".local/share/ward");
        Ok(Config {
            paths: Paths {
                vault: dir.join("cofre.ward"),
                state: dir.join("estado"),
            },
            test_socket_dir: None,
            rt: ward_platform::socket::runtime_dir().join(ward_platform::socket::DIR_NAME),
        })
    }

    #[cfg(feature = "test-mode")]
    fn from_vars(var: impl Fn(&str) -> Option<OsString>) -> Result<Config, &'static str> {
        let dev = var("WARD_DEV_DIR")
            .and_then(clean_absolute)
            .ok_or("daemon de teste: WARD_DEV_DIR ausente ou não absoluto")?;
        let env_socket = match var("WARD_DEV_SOCKET_DIR") {
            Some(v) => {
                Some(clean_absolute(v).ok_or("daemon de teste: WARD_DEV_SOCKET_DIR inválido")?)
            }
            None => None,
        };
        let test_app = match var("WARD_DEV_APP") {
            Some(v) => Some(test_app_binary(v, &dev, env_socket.as_deref())?),
            None => None,
        };
        let test_extension = match var("WARD_DEV_EXTENSAO") {
            None => false,
            Some(v) if v == "chrome" => true,
            Some(_) => return Err("daemon de teste: WARD_DEV_EXTENSAO só aceita \"chrome\""),
        };
        let socket = env_socket.unwrap_or_else(|| {
            ward_platform::socket::runtime_dir().join(ward_platform::socket::TEST_DIR_NAME)
        });
        Ok(Config {
            paths: test_paths(&dev),
            rt: socket.clone(),
            test_socket_dir: Some(socket),
            test_app,
            test_extension,
        })
    }
}

/// Vault and state of the test daemon inside `dev` (D7).
pub fn test_paths(dev: &Path) -> Paths {
    Paths {
        vault: dev.join("test-vault.ward"),
        state: dev.join("test-vault.state"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_or_dot_dot_path_refused() {
        for bad in [
            "relativo", "/a/../b", "./x", "", "/a/./b", "/a//b", "/a/b/", "/",
        ] {
            assert!(clean_absolute(bad.into()).is_none(), "{bad}");
        }
        assert!(clean_absolute("/home/x/.dev".into()).is_some());
    }

    #[test]
    #[cfg(feature = "test-mode")]
    fn test_mode_requires_dev_dir() {
        assert!(Config::from_vars(|_| None).is_err());
        let c = Config::from_vars(|k| (k == "WARD_DEV_DIR").then(|| "/x/.dev".into())).unwrap();
        assert_eq!(c.paths.vault, Path::new("/x/.dev/test-vault.ward"));
        assert_eq!(c.paths.state, Path::new("/x/.dev/test-vault.state"));
        assert!(c.test_socket_dir.unwrap().ends_with("ward-teste"));
        assert!(!c.test_extension);
    }

    /// Phase 41: the extension exception only turns on with the exact value `chrome`.
    #[test]
    #[cfg(feature = "test-mode")]
    fn test_extension_only_with_chrome() {
        let with_value = |v: &'static str| {
            Config::from_vars(move |k| match k {
                "WARD_DEV_DIR" => Some("/x/.dev".into()),
                "WARD_DEV_EXTENSAO" => Some(v.into()),
                _ => None,
            })
        };
        assert!(with_value("chrome").unwrap().test_extension);
        for bad in ["", "1", "Chrome", "chrome ", "/opt/google/chrome/chrome"] {
            assert!(with_value(bad).is_err(), "{bad:?}");
        }
    }
}
