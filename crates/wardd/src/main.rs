//! Daemon do Ward: único processo que toca em segredo (R1). Vazio na Fase 11; o conteúdo chega a partir da Fase 22.

#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    // Ainda sem implementação: falha em vez de fingir sucesso.
    eprintln!("wardd: ainda não implementado");
    ExitCode::FAILURE
}
