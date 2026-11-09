//! Seletor de arquivo e terminal da ponte de teste (Fase 34). O Chrome de teste não tem
//! o diálogo do GTK nem abre terminal; a ponte troca os dois por versões que o QA
//! controla sem dar à interface nenhum poder novo:
//!
//! - [`SeletorDePasta`]: "escolhe" o único arquivo regular de uma pasta dada na linha de
//!   comando de quem subiu a ponte (`--anexos <pasta>`). A interface não manda caminho;
//!   o QA põe na pasta o arquivo da vez antes de clicar em "Adicionar arquivo".
//! - [`TerminalDeTeste`]: não abre nada; escreve na saída de erro da ponte que o
//!   "Conectar" pediu o terminal para a entrada, com o tamanho do token (o token não).
//! - [`TerminalDoRepositorio`] (Fase 39c): com o daemon de teste, o "Conectar" de
//!   verdade. É o mesmo [`TerminalDoSistema`] do app (token por socket herdado, ambiente
//!   do zero, só o número do fd em argv), com o "terminal" do repositório
//!   (`dev/testes/terminal-teste.py`: roda o comando do `-e` num pty, sem janela) e a CLI
//!   de teste (`target/debug/ward`, `modo-teste`). Leva ao terminal só `WARD_DEV_DIR` e
//!   `WARD_DEV_SOCKET_DIR`, que vêm da configuração do daemon de teste.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use ward_app_nucleo::ErroApp;
use ward_app_nucleo::anexos::Seletor;
use ward_app_nucleo::terminal::{Terminal, TerminalDoSistema, Token};

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

pub struct TerminalDoRepositorio {
    sistema: TerminalDoSistema,
    extras: Vec<(&'static str, OsString)>,
}

/// Arquivo regular (sem seguir link) que o dono pode executar.
fn executavel(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::symlink_metadata(p)
        .is_ok_and(|m| m.file_type().is_file() && m.permissions().mode() & 0o100 != 0)
}

impl TerminalDoRepositorio {
    /// O terminal e a CLI de teste do repositório de `WARD_DEV_DIR`; `None` se faltar um
    /// dos dois (a ponte usa o [`TerminalDeTeste`]).
    pub fn do_repositorio(
        dev: Option<OsString>,
        socket: Option<OsString>,
    ) -> Option<TerminalDoRepositorio> {
        let raiz = crate::partida::repositorio(dev.clone())?;
        let emulador = raiz.join("dev/testes/terminal-teste.py");
        let ward = raiz.join("target/debug/ward");
        if !executavel(&emulador) || !executavel(&ward) {
            return None;
        }
        let mut extras = vec![("WARD_DEV_DIR", dev?)];
        if let Some(s) = socket {
            extras.push(("WARD_DEV_SOCKET_DIR", s));
        }
        Some(TerminalDoRepositorio {
            sistema: TerminalDoSistema::com_caminhos(emulador, ward),
            extras,
        })
    }
}

impl Terminal for TerminalDoRepositorio {
    fn abrir(&self, entrada: &str, token: &Token) -> Result<(), ErroApp> {
        let mut a = self.sistema.comando(entrada, token)?;
        for (k, v) in &self.extras {
            a.comando.env(k, v);
        }
        let mut filho = a.abrir().map_err(|_| ErroApp::TerminalIndisponivel)?;
        eprintln!("ward-ponte-teste: conectar {entrada} pelo terminal do repositório");
        std::thread::spawn(move || {
            let _ = filho.wait();
        });
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// Fase 39c: o "Conectar" da ponte só usa o terminal e a CLI de um repositório dado
    /// por um `WARD_DEV_DIR` limpo (`<repositório>/.dev`), cada um arquivo regular
    /// executável; link no lugar (um terminal do atacante) ou caminho torto: sem terminal
    /// de verdade (a ponte cai no que só registra).
    #[test]
    fn ataque_terminal_do_repositorio_so_com_arquivos_do_repositorio() {
        let base = std::env::temp_dir().join(format!("ward-ponte-terminal-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("dev/testes")).unwrap();
        fs::create_dir_all(base.join("target/debug")).unwrap();
        fs::create_dir_all(base.join(".dev")).unwrap();
        let dev = OsString::from(base.join(".dev"));
        let emulador = base.join("dev/testes/terminal-teste.py");
        let ward = base.join("target/debug/ward");
        // Faltando os dois.
        assert!(TerminalDoRepositorio::do_repositorio(Some(dev.clone()), None).is_none());
        for f in [&emulador, &ward] {
            fs::write(f, "#!/bin/sh\n").unwrap();
            fs::set_permissions(f, fs::Permissions::from_mode(0o700)).unwrap();
        }
        assert!(TerminalDoRepositorio::do_repositorio(Some(dev.clone()), None).is_some());
        // Caminhos tortos de WARD_DEV_DIR.
        let torto = format!("{}/x/../.dev", base.display());
        for ruim in [
            None,
            Some(OsString::from("rel/.dev")),
            Some(OsString::from(torto)),
        ] {
            assert!(TerminalDoRepositorio::do_repositorio(ruim, None).is_none());
        }
        // Sem execução para o dono.
        fs::set_permissions(&ward, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(TerminalDoRepositorio::do_repositorio(Some(dev.clone()), None).is_none());
        fs::set_permissions(&ward, fs::Permissions::from_mode(0o700)).unwrap();
        // Link no lugar do terminal.
        let alvo = base.join("terminal-do-atacante");
        fs::write(&alvo, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&alvo, fs::Permissions::from_mode(0o700)).unwrap();
        fs::remove_file(&emulador).unwrap();
        std::os::unix::fs::symlink(&alvo, &emulador).unwrap();
        assert!(TerminalDoRepositorio::do_repositorio(Some(dev), None).is_none());
        let _ = fs::remove_dir_all(&base);
    }
}
