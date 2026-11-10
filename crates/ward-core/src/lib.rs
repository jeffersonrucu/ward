//! Ward core: file format, cryptography, types, output mask and protocol. Does not
//! depend on Linux (R2, R3).

#![forbid(unsafe_code)]

// Automatic local backup and the version to restore (D17, D27; Phase 45).
#[cfg(unix)]
pub mod backup;

// Vault configuration: PIN hash, lock timeout, backup (D30, Phase 19b).
pub mod configuracao;
// Vault body: configuration and entries ↔ bytes (D26, D30; Phases 15a and 19b).
pub mod corpo;
mod crypto;
// Agent text to display in the approval window (D51, Phase 23).
pub mod display;
pub mod entrada;
// Delivery through environment variables (D10, Phase 16).
pub mod entrega;
pub mod formato;
// Limits in a single source (D32, Phase 19a).
pub mod limits;
// Streaming output mask (D11, Phase 17).
pub mod mascara;
pub mod origin;
// Socket protocol: frames, messages, roles and the operation × role table (Phase 20).
pub mod protocolo;
// TOTP (RFC 6238) and password generator (Phase 18).
pub mod generator;
pub mod totp;
// Atomic write: uses only std with the Unix extensions (mode 0600, directory fsync).
#[cfg(unix)]
pub mod gravacao;

// Wire tests against the 0.1.4 baseline in `dev/fixtures/legacy/` (m2:DT18, m2:V2.2).
#[cfg(all(test, unix))]
mod wire_v1;
