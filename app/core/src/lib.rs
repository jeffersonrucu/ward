//! Rust side of the Ward desktop app, without Tauri (Phase 29).
//!
//! - [`commands`]: the closed list of the commands the interface may call. The Tauri
//!   `build.rs`, the capabilities and the test bridge come from it (S11).
//! - [`client`]: the app role requests over the Phase 20 protocol (status, list, detail,
//!   copy), with the strict reading of the response.
//! - [`daemon`]: transport to `wardd` (checked socket): frames on the single connection,
//!   reader thread and notice queue (D65 a, Phase 23a).
//! - [`fake_daemon`] (tests and `test-mode` only): answers over the protocol with the
//!   fake entries of the test vault, for the test bridge.
//! - [`credentials`]: list and detail of the Credentials screen (Phase 32), no secret.
//! - [`clipboard`]: "copy" resolved here (D4). The value goes from the daemon to
//!   the clipboard without passing through the webview, and the cleanup in 30 s only
//!   erases if the clipboard still holds the copied value (S12).
//! - [`form`]: entry form (Phase 33). A kept secret and a generated password stay on the
//!   Rust side; the TOTP seed and the `otpauth://` are normalized here (D35).
//! - [`generator`]: standalone password generator (Phase 37). The password is born in
//!   `ward_core::generator` and stays here; the interface receives only size, strength
//!   and symbols, and the value only on "reveal".
//! - [`level`]: change the access level with PIN (Phase 35, S9); the PIN stays in a type
//!   that zeroes and only goes to the set level request.
//! - [`unlock`]: unlock with the master password (Phase 31; the password stays in a
//!   type that zeroes and only goes to the unlock request) and the agent's unlock
//!   request (D65 a).
//! - [`startup`]: startup of the setgid app before GTK (D47, D57): D57 environment,
//!   connection with the egid of the `ward-app` group, dropping the group, non-dumpable,
//!   re-execution.
//! - [`attachments`]: form attachments (Phase 34). The file is chosen and read on the
//!   Rust side; the interface receives only name and size (S11).
//! - [`terminal`]: "Connect" of a server entry (D3): single-use token and the default
//!   terminal.
//! - [`first_use`]: master password and PIN of the new vault (Phase 30), checked on
//!   the Rust side before the vault creation request; the daemon stores only the PIN
//!   hash (D30).
//! - [`settings`]: Settings screen (Phase 38): change PIN (asks for the current
//!   one), change master password (asks for the current one; the new one with the first
//!   use rule) and lock timeout, stored by the daemon in the vault config (D30, D39).
//! - [`external_backup`]: Settings › external backup (Phase 48): the chosen USB drive must
//!   be in the daemon's list; the label (untrusted) arrives neutralized (D51).
//! - [`restore`]: restore backup (Phase 47): versions only by name (date and time, no
//!   content); the requested version must be in the daemon's list; version password and
//!   current PIN in types that zero.
//! - [`approval`]: approval window (Phase 36): daemon requests with the agent's text
//!   neutralized (D51) and the answer with PIN, only through the approval window.
//! - [`tray`]: tray icon and menu (Phase 39): texts by state, actions (none reveals,
//!   copies or approves) and the 0700 folder of the icon.
//! - [`service`]: what each command does. No response carries a secret (S1, S11).
//! - [`secret`]: secret value in transit (only the "copy", D4), zeroed on drop.

#![forbid(unsafe_code)]

pub mod approval;
pub mod attachments;
pub mod client;
pub mod clipboard;
pub mod commands;
pub mod credentials;
pub mod daemon;
mod error;
pub mod external_backup;
#[cfg(any(test, feature = "test-mode"))]
pub mod fake_daemon;
pub mod first_use;
pub mod form;
pub mod generator;
pub mod key;
pub mod level;
pub mod restore;
pub mod secret;
pub mod service;
pub mod settings;
pub mod startup;
pub mod terminal;
pub mod tray;
pub mod unlock;

pub use error::AppError;
