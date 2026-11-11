//! Test HTTP bridge (D7): connects the Svelte interface in Chrome to the test daemon.
//! It only exists with the `test-mode` feature; the release build does not contain it and
//! `make install` refuses artifacts with `test-mode`.
//!
//! Phase 29. The bridge:
//! - listens only on `127.0.0.1`, random port, with a random 32-byte token that goes in
//!   the opening URL (`#token=…`, fragment: it never reaches the server nor a log) and
//!   comes back in the `Authorization` header of each call (compared in constant time);
//! - checks `Host` (against DNS rebinding) and `Origin` (against another Chrome page);
//! - serves the test-mode build of the interface (only `index.html` and
//!   `assets/<name>`) with the same CSP as the app;
//! - exposes as `POST` only the commands of the app core's command list, through the
//!   same service as Tauri: no reply carries a secret (the "copy" is resolved in Rust,
//!   D4).

#![forbid(unsafe_code)]

#[cfg(feature = "test-mode")]
pub mod http;
#[cfg(feature = "test-mode")]
pub mod simulated;
#[cfg(feature = "test-mode")]
pub mod startup;
#[cfg(feature = "test-mode")]
pub mod wl_clipboard;
