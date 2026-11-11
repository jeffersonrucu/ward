//! Ward daemon (`wardd`): the only process that touches secrets (R1, Phase 22).
//!
//! - [`nucleo`]: locked/unlocked state and the rule of each operation.
//! - [`server`]: the socket loop (peer uid, role, frames, limits).
//! - [`files`]: reading the vault with identity by `fstat` and the state file
//!   (D39, D53).
//! - [`config`]: release and test daemon paths (D7).
//! - [`aprovacao`]: approval engine and the wait for the unlock (Phase 23); [`usage`]:
//!   the agent's use (combined level, 1 hour grant, buttons with PIN);
//!   [`launch`]: the daemon opens the app through the launcher (D57).
//! - [`lock`]: automatic lock on inactivity and through logind (Phase 24).
//! - [`execution`]: `ward run` (Phase 26): delivery, mask, child and temp files.
//! - [`ssh`]: `ward ssh` (Phase 27); [`bridge`]: the jump and the tunnel without a shell
//!   (Phase 28).
//! - [`login`]: `ward login` (Phase 43), taken to the Chrome extension.
//! - [`playwright`]: `ward playwright` (Phase 44) through Chrome with the debugging
//!   pipe ([`cdp`]).
//! - [`external`]: external backup on a USB drive (Phase 46); [`udisks`]: the USB drives
//!   through udisks2 on the system bus.
//!
//! No `unsafe`: what depends on the system is in `ward-platform`.

#![forbid(unsafe_code)]

pub mod log;

pub mod aprovacao;
pub mod bridge;
pub mod cdp;
pub mod config;
pub mod execution;
pub mod external;
pub mod files;
pub mod launch;
pub mod lock;
pub mod login;
pub mod nucleo;
pub mod playwright;
pub mod server;
pub mod ssh;
pub mod ssh_agent;
pub mod udisks;
pub mod usage;
