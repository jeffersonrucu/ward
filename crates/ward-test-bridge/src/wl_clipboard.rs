//! Clipboard of the test bridge: the system `wl-copy`/`wl-paste`.
//!
//! Only in the bridge (`test-mode`), which only sees fake values of the test vault. The
//! app uses the GTK clipboard (Phase 39f, S2): `wl-copy` writes the value to
//! `/tmp/wl-copy-buffer-*/`, on disk, and that is why it stays out of the app's path.
//!
//! Binaries in `/usr/bin` owned by root without write for group and others, value through
//! stdin (never in argv, S3) and a clean environment (only `WAYLAND_DISPLAY` and
//! `XDG_RUNTIME_DIR`).

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::process::{Command, Stdio};

use ward_app_core::AppError;
use ward_app_core::clipboard::{Clipboard, MAX_VALUE};
use zeroize::Zeroizing;

/// The system `wl-copy`/`wl-paste`.
pub struct WlCopy {
    _private: (),
}

const WL_COPY: &str = "/usr/bin/wl-copy";
const WL_PASTE: &str = "/usr/bin/wl-paste";
const MIME_TYPE: &str = "text/plain;charset=utf-8";

impl WlCopy {
    /// Checks both binaries (owned by root, regular file, no write for group and others,
    /// not a symbolic link) before using them.
    pub fn new() -> Result<WlCopy, AppError> {
        for b in [WL_COPY, WL_PASTE] {
            trusted_binary(Path::new(b))?;
        }
        Ok(WlCopy { _private: () })
    }

    fn command(program: &str) -> Command {
        let mut c = Command::new(program);
        c.env_clear();
        for v in ["WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"] {
            if let Some(value) = std::env::var_os(v) {
                c.env(v, value);
            }
        }
        c.stderr(Stdio::null());
        c
    }
}

fn trusted_binary(path: &Path) -> Result<(), AppError> {
    let md = fs::symlink_metadata(path).map_err(|_| AppError::Clipboard)?;
    if !md.file_type().is_file() || md.uid() != 0 || md.permissions().mode() & 0o022 != 0 {
        return Err(AppError::Clipboard);
    }
    Ok(())
}

impl Clipboard for WlCopy {
    fn write(&self, value: &[u8]) -> Result<(), AppError> {
        let mut child = Self::command(WL_COPY)
            .args(["--type", MIME_TYPE])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .map_err(|_| AppError::Clipboard)?;
        {
            let mut stdin = child.stdin.take().ok_or(AppError::Clipboard)?;
            stdin.write_all(value).map_err(|_| AppError::Clipboard)?;
        }
        let status = child.wait().map_err(|_| AppError::Clipboard)?;
        if status.success() {
            Ok(())
        } else {
            Err(AppError::Clipboard)
        }
    }

    fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>, AppError> {
        let mut child = Self::command(WL_PASTE)
            .args(["--no-newline", "--type", "text/plain"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|_| AppError::Clipboard)?;
        let mut output = Zeroizing::new(Vec::new());
        let read = child
            .stdout
            .take()
            .ok_or(AppError::Clipboard)?
            .take(MAX_VALUE as u64 + 1)
            .read_to_end(&mut output);
        // More than the limit: the rest stays in the pipe and wl-paste is killed.
        let _ = child.kill();
        let status = child.wait().map_err(|_| AppError::Clipboard)?;
        if read.is_err() || (!status.success() && output.is_empty()) {
            return Ok(None); // empty or no text
        }
        Ok(Some(output))
    }

    fn clear(&self) -> Result<(), AppError> {
        let status = Self::command(WL_COPY)
            .arg("--clear")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .map_err(|_| AppError::Clipboard)?;
        if status.success() {
            Ok(())
        } else {
            Err(AppError::Clipboard)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attack_binary_off_the_rules_is_refused() {
        let base = std::env::temp_dir().join(format!("ward-bridge-wl-{}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        let fake = base.join("wl-copy");
        fs::write(&fake, b"#!/bin/sh\n").unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
        // File of the user (not root) and symbolic link: both refused.
        assert_eq!(trusted_binary(&fake), Err(AppError::Clipboard));
        let link = base.join("link");
        std::os::unix::fs::symlink("/usr/bin/env", &link).unwrap();
        assert_eq!(trusted_binary(&link), Err(AppError::Clipboard));
        fs::remove_dir_all(base).unwrap();
    }
}
