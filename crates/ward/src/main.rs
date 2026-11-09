//! CLI do Ward: cliente do `wardd` pelo socket local; nunca recebe segredo (S1). Vazia na Fase 11.

#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    // Ainda sem implementação: falha em vez de fingir sucesso.
    eprintln!("ward: ainda não implementado");
    ExitCode::FAILURE
}
