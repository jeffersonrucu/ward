//! Seletor de arquivo e terminal da ponte de teste (Fase 34). O Chrome de teste não tem
//! o diálogo do GTK nem abre terminal; a ponte troca os dois por versões que o QA
//! controla sem dar à interface nenhum poder novo:
//!
//! - [`SeletorDePasta`]: "escolhe" o único arquivo regular de uma pasta dada na linha de
//!   comando de quem subiu a ponte (`--anexos <pasta>`). A interface não manda caminho;
//!   o QA põe na pasta o arquivo da vez antes de clicar em "Adicionar arquivo".
//! - [`TerminalDeTeste`]: não abre nada; escreve na saída de erro da ponte que o
//!   "Conectar" pediu o terminal para a entrada, com o tamanho do token (o token não).

use std::fs;
use std::path::PathBuf;

use ward_app_nucleo::ErroApp;
use ward_app_nucleo::anexos::Seletor;
use ward_app_nucleo::terminal::{Terminal, Token};

pub struct SeletorDePasta(pub PathBuf);

impl Seletor for SeletorDePasta {
    fn escolher(&self) -> Result<Option<PathBuf>, ErroApp> {
        let mut arquivos = Vec::new();
        for e in fs::read_dir(&self.0).map_err(|_| ErroApp::SeletorIndisponivel)? {
            let e = e.map_err(|_| ErroApp::SeletorIndisponivel)?;
            // Sem seguir link: `file_type` do `readdir` não segue.
            if e.file_type().is_ok_and(|t| t.is_file()) {
                arquivos.push(e.path());
            }
        }
        match arquivos.len() {
            0 => Ok(None),
            1 => Ok(arquivos.pop()),
            _ => Err(ErroApp::SeletorIndisponivel),
        }
    }
}

pub struct TerminalDeTeste;

impl Terminal for TerminalDeTeste {
    fn abrir(&self, entrada: &str, token: &Token) -> Result<(), ErroApp> {
        eprintln!(
            "ward-ponte-teste: conectar {entrada} (token de uso único com {} caracteres)",
            token.len()
        );
        Ok(())
    }
}
