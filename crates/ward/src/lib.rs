//! Ward CLI: client of `wardd` over the local socket, always in the agent role (D3). No
//! command returns a secret (S1): the CLI only lists, locks and unlocks.
//!
//! - Every connection checks the server before the first byte ([`client`], D46, S5).
//! - The master password of the unlock comes only from the terminal, never from argv
//!   nor stdin (S3), and stays in types that zero themselves (S8).
//! - Texts from the vault go out neutralized ([`text`]).
//! - Messages in Portuguese, never repeating an argument nor frame content.

#![forbid(unsafe_code)]

pub mod args;
pub mod client;
pub mod commands;
pub mod text;

#[cfg(test)]
mod tests;

use std::ffi::OsString;

use args::{HELP, UsageError};
use commands::{EXIT_USAGE, Env};

/// Parses the arguments (without the program name) and runs the command.
pub fn main(args: impl IntoIterator<Item = OsString>, e: &mut dyn Env) -> u8 {
    match args::parse(args) {
        Ok(cmd) => commands::execute(cmd, e),
        Err(UsageError::NoCommand) => {
            let _ = e.stderr().write_all(HELP.as_bytes());
            EXIT_USAGE
        }
        Err(u) => {
            let _ = writeln!(e.stderr(), "ward: {}; veja `ward --help`", u.message());
            EXIT_USAGE
        }
    }
}
