//! Área de transferência da ponte de teste: `wl-copy`/`wl-paste` do sistema.
//!
//! Só na ponte (`test-mode`), que só vê valores falsos do cofre de teste. O app usa a
//! área do GTK (Fase 39f, S2): o `wl-copy` grava o valor em `/tmp/wl-copy-buffer-*/`,
//! no disco, e por isso não entra no caminho do app.
//!
//! Binários de `/usr/bin` de dono root sem escrita para grupo e outros, valor pela
//! entrada padrão (nunca em argv, S3) e ambiente limpo (só `WAYLAND_DISPLAY` e
//! `XDG_RUNTIME_DIR`).

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::process::{Command, Stdio};

use ward_app_core::AppError;
use ward_app_core::area_transferencia::{AreaTransferencia, MAX_VALOR};
use zeroize::Zeroizing;

/// `wl-copy`/`wl-paste` do sistema.
pub struct WlCopy {
    _privado: (),
}

const WL_COPY: &str = "/usr/bin/wl-copy";
const WL_PASTE: &str = "/usr/bin/wl-paste";
const TIPO: &str = "text/plain;charset=utf-8";

impl WlCopy {
    /// Confere os dois binários (dono root, arquivo comum, sem escrita para grupo e
    /// outros, sem link simbólico) antes de usar.
    pub fn novo() -> Result<WlCopy, AppError> {
        for b in [WL_COPY, WL_PASTE] {
            binario_confiavel(Path::new(b))?;
        }
        Ok(WlCopy { _privado: () })
    }

    fn comando(programa: &str) -> Command {
        let mut c = Command::new(programa);
        c.env_clear();
        for v in ["WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"] {
            if let Some(valor) = std::env::var_os(v) {
                c.env(v, valor);
            }
        }
        c.stderr(Stdio::null());
        c
    }
}

fn binario_confiavel(caminho: &Path) -> Result<(), AppError> {
    let md = fs::symlink_metadata(caminho).map_err(|_| AppError::Clipboard)?;
    if !md.file_type().is_file() || md.uid() != 0 || md.permissions().mode() & 0o022 != 0 {
        return Err(AppError::Clipboard);
    }
    Ok(())
}

impl AreaTransferencia for WlCopy {
    fn escrever(&self, valor: &[u8]) -> Result<(), AppError> {
        let mut filho = Self::comando(WL_COPY)
            .args(["--type", TIPO])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .map_err(|_| AppError::Clipboard)?;
        {
            let mut entrada = filho.stdin.take().ok_or(AppError::Clipboard)?;
            entrada.write_all(valor).map_err(|_| AppError::Clipboard)?;
        }
        let estado = filho.wait().map_err(|_| AppError::Clipboard)?;
        if estado.success() {
            Ok(())
        } else {
            Err(AppError::Clipboard)
        }
    }

    fn ler(&self) -> Result<Option<Zeroizing<Vec<u8>>>, AppError> {
        let mut filho = Self::comando(WL_PASTE)
            .args(["--no-newline", "--type", "text/plain"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|_| AppError::Clipboard)?;
        let mut saida = Zeroizing::new(Vec::new());
        let lido = filho
            .stdout
            .take()
            .ok_or(AppError::Clipboard)?
            .take(MAX_VALOR as u64 + 1)
            .read_to_end(&mut saida);
        // Mais que o limite: o resto fica no cano e o wl-paste é encerrado.
        let _ = filho.kill();
        let estado = filho.wait().map_err(|_| AppError::Clipboard)?;
        if lido.is_err() || (!estado.success() && saida.is_empty()) {
            return Ok(None); // vazia ou sem texto
        }
        Ok(Some(saida))
    }

    fn limpar(&self) -> Result<(), AppError> {
        let estado = Self::comando(WL_COPY)
            .arg("--clear")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .map_err(|_| AppError::Clipboard)?;
        if estado.success() {
            Ok(())
        } else {
            Err(AppError::Clipboard)
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ataque_binario_fora_do_padrao_e_recusado() {
        let base = std::env::temp_dir().join(format!("ward-ponte-wl-{}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        let falso = base.join("wl-copy");
        fs::write(&falso, b"#!/bin/sh\n").unwrap();
        fs::set_permissions(&falso, fs::Permissions::from_mode(0o755)).unwrap();
        // Arquivo do usuário (não root) e link simbólico: os dois recusados.
        assert_eq!(binario_confiavel(&falso), Err(AppError::Clipboard));
        let link = base.join("link");
        std::os::unix::fs::symlink("/usr/bin/env", &link).unwrap();
        assert_eq!(binario_confiavel(&link), Err(AppError::Clipboard));
        fs::remove_dir_all(base).unwrap();
    }
}
