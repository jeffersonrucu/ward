//! CLI do Ward: cliente do `wardd` pelo socket local; nunca recebe segredo (S1).

#![forbid(unsafe_code)]

use std::process::ExitCode;

use ward::comandos::Sistema;

fn main() -> ExitCode {
    let mut e = Sistema {
        esperado: ward::cliente::servidor_esperado(),
        stdout: std::io::stdout(),
        stderr: std::io::stderr(),
    };
    ExitCode::from(ward::principal(std::env::args_os().skip(1), &mut e))
}
