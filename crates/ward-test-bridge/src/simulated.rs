//! File picker and terminal of the test bridge (Phase 34). The test Chrome has neither
//! the GTK dialog nor opens a terminal; the bridge swaps both for versions the QA
//! controls without giving the interface any new power:
//!
//! - [`DirPicker`]: "picks" the only regular file of a folder given on the command line
//!   of whoever started the bridge (the attachments option). The interface sends no
//!   path; the QA puts the file of the moment in the folder before clicking "add file".
//! - [`TestTerminal`]: opens nothing; writes to the bridge's stderr that "Connect"
//!   asked for the terminal for the entry, with the token length (not the token).
//! - [`RepoTerminal`] (Phase 39c): with the test daemon, the real "Connect". It is the
//!   same system terminal as the app (token through an inherited socket,
//!   environment from scratch, only the fd number in argv), with the repository
//!   "terminal" (`dev/tests/terminal-test.py`: runs the `-e` command in a pty, no window)
//!   and the test CLI (`target/debug/ward`, `test-mode`). It passes to the terminal only
//!   `WARD_DEV_DIR` and `WARD_DEV_SOCKET_DIR`, which come from the test daemon's config.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use ward_app_core::AppError;
use ward_app_core::attachments::Picker;
use ward_app_core::terminal::{Terminal, TerminalDoSistema, Token};

pub struct DirPicker(pub PathBuf);

impl Picker for DirPicker {
    fn choose(&self) -> Result<Option<PathBuf>, AppError> {
        let mut files = Vec::new();
        for e in fs::read_dir(&self.0).map_err(|_| AppError::PickerUnavailable)? {
            let e = e.map_err(|_| AppError::PickerUnavailable)?;
            // Without following links: `file_type` of `readdir` does not follow.
            if e.file_type().is_ok_and(|t| t.is_file()) {
                files.push(e.path());
            }
        }
        match files.len() {
            0 => Ok(None),
            1 => Ok(files.pop()),
            _ => Err(AppError::PickerUnavailable),
        }
    }
}

pub struct TestTerminal;

impl Terminal for TestTerminal {
    fn abrir(&self, entry: &str, token: &Token) -> Result<(), AppError> {
        eprintln!(
            "ward-ponte-teste: conectar {entry} (token de uso único com {} caracteres)",
            token.len()
        );
        Ok(())
    }
}

pub struct RepoTerminal {
    system: TerminalDoSistema,
    extras: Vec<(&'static str, OsString)>,
}

/// Regular file (without following links) that the owner can execute.
fn executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::symlink_metadata(p)
        .is_ok_and(|m| m.file_type().is_file() && m.permissions().mode() & 0o100 != 0)
}

impl RepoTerminal {
    /// The terminal and the test CLI of the repository of `WARD_DEV_DIR`; `None` if one
    /// of the two is missing (the bridge uses the [`TestTerminal`]).
    pub fn from_repository(
        dev: Option<OsString>,
        socket: Option<OsString>,
    ) -> Option<RepoTerminal> {
        let root = crate::startup::repository(dev.clone())?;
        let emulator = root.join("dev/tests/terminal-test.py");
        let ward = root.join("target/debug/ward");
        if !executable(&emulator) || !executable(&ward) {
            return None;
        }
        let mut extras = vec![("WARD_DEV_DIR", dev?)];
        if let Some(s) = socket {
            extras.push(("WARD_DEV_SOCKET_DIR", s));
        }
        Some(RepoTerminal {
            system: TerminalDoSistema::com_caminhos(emulator, ward),
            extras,
        })
    }
}

impl Terminal for RepoTerminal {
    fn abrir(&self, entry: &str, token: &Token) -> Result<(), AppError> {
        let mut a = self.system.comando(entry, token)?;
        for (k, v) in &self.extras {
            a.comando.env(k, v);
        }
        let mut child = a.abrir().map_err(|_| AppError::TerminalUnavailable)?;
        eprintln!("ward-ponte-teste: conectar {entry} pelo terminal do repositório");
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// Phase 39c: the bridge's "Connect" only uses the terminal and the CLI of a
    /// repository given by a clean `WARD_DEV_DIR` (`<repository>/.dev`), each one a regular
    /// executable file; a link in its place (an attacker's terminal) or a crooked path: no
    /// real terminal (the bridge falls back to the one that only logs).
    #[test]
    fn attack_repo_terminal_only_with_repository_files() {
        let base =
            std::env::temp_dir().join(format!("ward-bridge-terminal-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("dev/tests")).unwrap();
        fs::create_dir_all(base.join("target/debug")).unwrap();
        fs::create_dir_all(base.join(".dev")).unwrap();
        let dev = OsString::from(base.join(".dev"));
        let emulator = base.join("dev/tests/terminal-test.py");
        let ward = base.join("target/debug/ward");
        // Both missing.
        assert!(RepoTerminal::from_repository(Some(dev.clone()), None).is_none());
        for f in [&emulator, &ward] {
            fs::write(f, "#!/bin/sh\n").unwrap();
            fs::set_permissions(f, fs::Permissions::from_mode(0o700)).unwrap();
        }
        assert!(RepoTerminal::from_repository(Some(dev.clone()), None).is_some());
        // Crooked WARD_DEV_DIR paths.
        let crooked = format!("{}/x/../.dev", base.display());
        for bad in [
            None,
            Some(OsString::from("rel/.dev")),
            Some(OsString::from(crooked)),
        ] {
            assert!(RepoTerminal::from_repository(bad, None).is_none());
        }
        // No execute permission for the owner.
        fs::set_permissions(&ward, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(RepoTerminal::from_repository(Some(dev.clone()), None).is_none());
        fs::set_permissions(&ward, fs::Permissions::from_mode(0o700)).unwrap();
        // Link in place of the terminal.
        let target = base.join("attacker-terminal");
        fs::write(&target, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
        fs::remove_file(&emulator).unwrap();
        std::os::unix::fs::symlink(&target, &emulator).unwrap();
        assert!(RepoTerminal::from_repository(Some(dev), None).is_none());
        let _ = fs::remove_dir_all(&base);
    }
}
