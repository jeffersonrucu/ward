//! Partida da ponte contra o daemon de teste (D63, Fase 23a).
//!
//! A ponte só ganha o papel app quando o daemon de teste a abre pelo lançador
//! (`abrir_app` com `WARD_DEV_APP` apontando para ela): o `test-mode` do daemon não
//! exige o egid do grupo, mas exige o pid que o lançador abriu e o pai = daemon. Então:
//! - conecta **uma vez** ([`DaemonSocket::conectar_agora`]) e nunca reconecta;
//! - o primeiro pedido é `estado`, e a ponte confere o papel na resposta. Sem `app`
//!   (executada à mão, cópia, outro pai), sai com [`SEM_PAPEL_APP`] antes de abrir a
//!   porta HTTP: a interface nunca chega a falar com uma conexão de agente.
//!
//! Aberta pelo daemon, a ponte não recebe argumentos (nada do pedido chega a ela) e o
//! ambiente é o fixo mínimo do lançador de teste; o `dist-teste` sai de
//! `WARD_DEV_DIR` (`<repositório>/.dev` → `<repositório>/app/dist-teste`).

use std::ffi::OsString;
use std::path::PathBuf;

use ward_app_core::cliente;
use ward_app_core::daemon::DaemonSocket;

/// Mensagem de quando o daemon não deu o papel app (V3.38).
pub const SEM_PAPEL_APP: &str = "o daemon não deu o papel app";

/// Conecta (uma vez) e confere o papel na primeira resposta. A mensagem de erro é fixa.
pub fn conectar_com_papel_app(mut daemon: DaemonSocket) -> Result<DaemonSocket, String> {
    daemon.conectar_agora().map_err(|e| e.to_string())?;
    match cliente::tem_papel_app(&mut daemon) {
        Ok(true) => Ok(daemon),
        Ok(false) => Err(SEM_PAPEL_APP.to_owned()),
        Err(e) => Err(e.to_string()),
    }
}

/// O `dist-teste` da ponte aberta pelo daemon: `<WARD_DEV_DIR>/../app/dist-teste`.
/// `WARD_DEV_DIR` tem de ser absoluto, sem `.`/`..`, e terminar em `/.dev`.
pub fn dist_do_repositorio(dev: Option<OsString>) -> Option<PathBuf> {
    Some(repositorio(dev)?.join("app").join("dist-teste"))
}

/// O repositório de `WARD_DEV_DIR` (`<repositório>/.dev`), com a mesma regra do
/// [`dist_do_repositorio`].
pub fn repositorio(dev: Option<OsString>) -> Option<PathBuf> {
    use std::path::Component;
    let dev = PathBuf::from(dev?);
    let limpo = dev.is_absolute()
        && dev
            .components()
            .all(|c| matches!(c, Component::RootDir | Component::Normal(_)));
    if !limpo || dev.file_name()? != ".dev" {
        return None;
    }
    Some(dev.parent()?.to_path_buf())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn dist_so_de_dev_absoluto() {
        assert_eq!(
            dist_do_repositorio(Some("/r/x/.dev".into())),
            Some(PathBuf::from("/r/x/app/dist-teste"))
        );
        assert_eq!(
            repositorio(Some("/r/x/.dev".into())),
            Some(PathBuf::from("/r/x"))
        );
        for ruim in ["", "rel/.dev", "/r/../.dev", "/r/x/dev", "/.dev/.."] {
            assert_eq!(dist_do_repositorio(Some(ruim.into())), None, "{ruim}");
            assert_eq!(repositorio(Some(ruim.into())), None, "{ruim}");
        }
        assert_eq!(dist_do_repositorio(None), None);
    }
}
