//! CLI do Ward: cliente do `wardd` pelo socket local, sempre no papel agente (D3). Não
//! existe comando que devolva segredo (S1): a CLI só lista, trava e destrava.
//!
//! - Toda conexão confere o servidor antes do primeiro byte ([`cliente`], D46, S5).
//! - A senha mestra do `destravar` vem só do terminal, nunca de argv nem de stdin (S3),
//!   e fica em tipos que zeram (S8).
//! - Textos do cofre saem neutralizados ([`texto`]).
//! - Mensagens em português, sem repetir argumento nem conteúdo de quadro.

#![forbid(unsafe_code)]

pub mod args;
pub mod cliente;
pub mod comandos;
pub mod texto;

#[cfg(test)]
mod testes;

use std::ffi::OsString;

use args::{AJUDA, ErroUso};
use comandos::{Entorno, SAIDA_USO};

/// Interpreta os argumentos (sem o nome do programa) e roda o comando.
pub fn principal(args: impl IntoIterator<Item = OsString>, e: &mut dyn Entorno) -> u8 {
    match args::interpretar(args) {
        Ok(cmd) => comandos::executar(cmd, e),
        Err(ErroUso::SemComando) => {
            let _ = e.erro().write_all(AJUDA.as_bytes());
            SAIDA_USO
        }
        Err(u) => {
            let _ = writeln!(e.erro(), "ward: {}; veja `ward --help`", u.mensagem());
            SAIDA_USO
        }
    }
}
