//! Daemon log on stderr (the journal, through the unit; `.dev/logs/` for the test daemon).
//!
//! S13: the messages are fixed text plus numbers, operation names, role and error
//! codes. No call receives a value from a request, from an entry, from a client path or
//! from an environment variable; callers pass only `&'static str` and integers. The
//! exception is the app's reveal, copy and TOTP operations (D65 b), which log the entry
//! name (from the vault, already validated) and the field (from the fixed protocol list,
//! or the extra's name escaped), never the value. With
//! `RUST_LOG` at `debug` or `trace`, there is also one line per request (operation, role
//! and result), never the content.

use std::sync::OnceLock;

static DETAILED: OnceLock<bool> = OnceLock::new();

/// Reads `RUST_LOG` once (only the level; the value is never printed).
pub fn configure() {
    let detailed = std::env::var_os("RUST_LOG").is_some_and(|v| {
        let v = v.as_encoded_bytes();
        [b"debug".as_slice(), b"trace".as_slice()]
            .iter()
            .any(|n| v.windows(n.len()).any(|w| w == *n))
    });
    let _ = DETAILED.set(detailed);
}

/// Log line always printed.
#[macro_export]
macro_rules! warning {
    ($($arg:tt)*) => {
        eprintln!("wardd: {}", format_args!($($arg)*))
    };
}

/// Line only with `RUST_LOG=debug|trace`.
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        if $crate::log::detailed() {
            eprintln!("wardd[debug]: {}", format_args!($($arg)*))
        }
    };
}

pub fn detailed() -> bool {
    DETAILED.get().copied().unwrap_or(false)
}
