//! Ward CLI: client of `wardd` over the local socket; never receives a secret (S1).

#![forbid(unsafe_code)]

use std::process::ExitCode;

use ward::commands::System;

fn main() -> ExitCode {
    let mut e = System {
        expected: ward::client::expected_server(),
        stdout: std::io::stdout(),
        stderr: std::io::stderr(),
    };
    ExitCode::from(ward::main(std::env::args_os().skip(1), &mut e))
}
