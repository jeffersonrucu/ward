//! Caminhos do daemon.
//!
//! - **Release:** cofre em `$HOME/.local/share/ward/cofre.ward` e estado em
//!   `$HOME/.local/share/ward/estado` (D39). O `HOME` vem da unit do systemd do sistema
//!   (`User=`), não da sessão (D46); tem de ser absoluto e sem `..`. Socket só pelo
//!   `LISTEN_FDS` do systemd, em `/run/ward/<uid>/wardd.sock`.
//! - **`modo-teste`** (D7): cofre `$WARD_DEV_DIR/cofre-teste.ward` e estado
//!   `$WARD_DEV_DIR/cofre-teste.estado`; `WARD_DEV_DIR` é obrigatório, então o daemon de
//!   teste nunca toca em `~/.local/share/ward/`. Sem `LISTEN_FDS`, o socket fica num
//!   diretório 0700 do usuário: `/run/user/<uid>/ward-teste/` ou `WARD_DEV_SOCKET_DIR`.
//!   O build de release não lê nenhuma variável `WARD_DEV_*`.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use crate::nucleo::Caminhos;

/// Configuração lida do ambiente.
#[derive(Debug, Clone)]
pub struct Config {
    pub caminhos: Caminhos,
    /// Diretório do socket do daemon de teste (`None` no release).
    pub diretorio_socket_teste: Option<PathBuf>,
}

/// Caminho absoluto sem `.`, `..` nem `//`, conferido nos bytes (o `components()` do
/// Rust some com `.` e `//` no meio, então ele sozinho não basta).
fn absoluto_limpo(v: OsString) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let p = PathBuf::from(v);
    let bytes = p.as_os_str().as_bytes();
    let segmentos_ok = bytes.strip_prefix(b"/").is_some_and(|resto| {
        resto
            .split(|b| *b == b'/')
            .all(|s| !s.is_empty() && s != b"." && s != b"..")
    });
    let limpo = segmentos_ok
        && p.components()
            .all(|c| matches!(c, Component::RootDir | Component::Normal(_)));
    limpo.then_some(p)
}

impl Config {
    /// Lê do ambiente. A mensagem de erro não leva valor de variável.
    pub fn do_ambiente() -> Result<Config, &'static str> {
        Self::de(|k| std::env::var_os(k))
    }

    #[cfg(not(feature = "modo-teste"))]
    fn de(var: impl Fn(&str) -> Option<OsString>) -> Result<Config, &'static str> {
        let home = var("HOME")
            .and_then(absoluto_limpo)
            .ok_or("HOME ausente ou não absoluto")?;
        let dir = home.join(".local/share/ward");
        Ok(Config {
            caminhos: Caminhos {
                cofre: dir.join("cofre.ward"),
                estado: dir.join("estado"),
            },
            diretorio_socket_teste: None,
        })
    }

    #[cfg(feature = "modo-teste")]
    fn de(var: impl Fn(&str) -> Option<OsString>) -> Result<Config, &'static str> {
        let dev = var("WARD_DEV_DIR")
            .and_then(absoluto_limpo)
            .ok_or("daemon de teste: WARD_DEV_DIR ausente ou não absoluto")?;
        let socket = match var("WARD_DEV_SOCKET_DIR") {
            Some(v) => absoluto_limpo(v).ok_or("daemon de teste: WARD_DEV_SOCKET_DIR inválido")?,
            None => ward_plataforma::socket::diretorio_runtime()
                .join(ward_plataforma::socket::NOME_DIRETORIO_TESTE),
        };
        Ok(Config {
            caminhos: caminhos_de_teste(&dev),
            diretorio_socket_teste: Some(socket),
        })
    }
}

/// Cofre e estado do daemon de teste dentro de `dev` (D7).
pub fn caminhos_de_teste(dev: &Path) -> Caminhos {
    Caminhos {
        cofre: dev.join("cofre-teste.ward"),
        estado: dev.join("cofre-teste.estado"),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn caminho_relativo_ou_com_ponto_ponto_recusado() {
        for ruim in [
            "relativo", "/a/../b", "./x", "", "/a/./b", "/a//b", "/a/b/", "/",
        ] {
            assert!(absoluto_limpo(ruim.into()).is_none(), "{ruim}");
        }
        assert!(absoluto_limpo("/home/x/.dev".into()).is_some());
    }

    #[test]
    #[cfg(feature = "modo-teste")]
    fn modo_teste_exige_dev_dir() {
        assert!(Config::de(|_| None).is_err());
        let c = Config::de(|k| (k == "WARD_DEV_DIR").then(|| "/x/.dev".into())).unwrap();
        assert_eq!(c.caminhos.cofre, Path::new("/x/.dev/cofre-teste.ward"));
        assert_eq!(c.caminhos.estado, Path::new("/x/.dev/cofre-teste.estado"));
        assert!(c.diretorio_socket_teste.unwrap().ends_with("ward-teste"));
    }
}
