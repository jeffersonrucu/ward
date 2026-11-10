//! Caminhos do daemon.
//!
//! - **Release:** cofre em `$HOME/.local/share/ward/cofre.ward` e estado em
//!   `$HOME/.local/share/ward/estado` (D39). O `HOME` vem da unit do systemd do sistema
//!   (`User=`), não da sessão (D46); tem de ser absoluto e sem `..`. Socket só pelo
//!   `LISTEN_FDS` do systemd, em `/run/ward/<uid>/wardd.sock`.
//! - **`test-mode`** (D7): cofre `$WARD_DEV_DIR/test-vault.ward` e estado
//!   `$WARD_DEV_DIR/test-vault.state`; `WARD_DEV_DIR` é obrigatório, então o daemon de
//!   teste nunca toca em `~/.local/share/ward/`. Sem `LISTEN_FDS`, o socket fica num
//!   diretório 0700 do usuário: `/run/user/<uid>/ward-teste/` ou `WARD_DEV_SOCKET_DIR`.
//!   Com `WARD_DEV_APP` (D63, Fase 23a), o `abrir_app` abre esse binário (o `ward-app`
//!   de teste ou a ponte): caminho absoluto de arquivo regular do uid, sem link, dentro
//!   do repositório (a pasta acima de `WARD_DEV_DIR`), conferido aqui e a cada
//!   abertura; a saída vai para `$WARD_DEV_DIR/logs/<binário>.log` (0600).
//!   Com `WARD_DEV_EXTENSAO=chrome` (Fase 41), o par filho do Chrome instalado ganha o
//!   papel extensão sem o egid do grupo `ward-nm` (ver [`crate::lancamento::ExtensaoDeTeste`]).
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
    /// O binário que o `abrir_app` do daemon de teste abre (D63).
    #[cfg(feature = "test-mode")]
    pub app_de_teste: Option<ward_platform::TestBinary>,
    /// `WARD_DEV_EXTENSAO=chrome` (Fase 41): o daemon de teste dá o papel extensão ao
    /// par filho do Chrome instalado sem o egid do grupo `ward-nm`. Qualquer outro valor
    /// é recusado na partida.
    #[cfg(feature = "test-mode")]
    pub extensao_de_teste: bool,
    /// `<rt>` (Fase 26, S2): os temporários do `ward run` ficam em `<rt>tmp/`. Release:
    /// `/run/user/<uid>/ward/` (pelo uid efetivo, não por `XDG_RUNTIME_DIR`); daemon de
    /// teste: o diretório do socket de teste.
    pub rt: PathBuf,
}

/// `WARD_DEV_APP` (D63): conferido como [`ward_platform::TestBinary`], dentro do
/// repositório (a pasta acima de `dev`). O ambiente mínimo da ponte leva `WARD_DEV_DIR`
/// e, se veio, `WARD_DEV_SOCKET_DIR`: são as variáveis do `test-mode`, da configuração
/// do daemon, nunca do pedido.
#[cfg(feature = "test-mode")]
fn app_de_teste(
    v: OsString,
    dev: &Path,
    socket: Option<&Path>,
) -> Result<ward_platform::TestBinary, &'static str> {
    let caminho = absoluto_limpo(v).ok_or("daemon de teste: WARD_DEV_APP inválido")?;
    let raiz = dev
        .parent()
        .filter(|r| r.parent().is_some())
        .ok_or("daemon de teste: WARD_DEV_DIR sem repositório acima")?;
    let texto = |p: &Path| {
        p.to_str()
            .map(str::to_owned)
            .ok_or("daemon de teste: caminho fora de UTF-8")
    };
    let mut extras = vec![("WARD_DEV_DIR", texto(dev)?)];
    if let Some(s) = socket {
        extras.push(("WARD_DEV_SOCKET_DIR", texto(s)?));
    }
    ward_platform::TestBinary::new(caminho, raiz.to_path_buf(), dev.join("logs"), extras)
        .map_err(|e| e.message())
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

    #[cfg(not(feature = "test-mode"))]
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
            rt: ward_platform::socket::diretorio_runtime()
                .join(ward_platform::socket::NOME_DIRETORIO),
        })
    }

    #[cfg(feature = "test-mode")]
    fn de(var: impl Fn(&str) -> Option<OsString>) -> Result<Config, &'static str> {
        let dev = var("WARD_DEV_DIR")
            .and_then(absoluto_limpo)
            .ok_or("daemon de teste: WARD_DEV_DIR ausente ou não absoluto")?;
        let socket_do_ambiente = match var("WARD_DEV_SOCKET_DIR") {
            Some(v) => {
                Some(absoluto_limpo(v).ok_or("daemon de teste: WARD_DEV_SOCKET_DIR inválido")?)
            }
            None => None,
        };
        let app_de_teste = match var("WARD_DEV_APP") {
            Some(v) => Some(app_de_teste(v, &dev, socket_do_ambiente.as_deref())?),
            None => None,
        };
        let extensao_de_teste = match var("WARD_DEV_EXTENSAO") {
            None => false,
            Some(v) if v == "chrome" => true,
            Some(_) => return Err("daemon de teste: WARD_DEV_EXTENSAO só aceita \"chrome\""),
        };
        let socket = socket_do_ambiente.unwrap_or_else(|| {
            ward_platform::socket::diretorio_runtime()
                .join(ward_platform::socket::NOME_DIRETORIO_TESTE)
        });
        Ok(Config {
            caminhos: caminhos_de_teste(&dev),
            rt: socket.clone(),
            diretorio_socket_teste: Some(socket),
            app_de_teste,
            extensao_de_teste,
        })
    }
}

/// Cofre e estado do daemon de teste dentro de `dev` (D7).
pub fn caminhos_de_teste(dev: &Path) -> Caminhos {
    Caminhos {
        cofre: dev.join("test-vault.ward"),
        estado: dev.join("test-vault.state"),
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
    #[cfg(feature = "test-mode")]
    fn modo_teste_exige_dev_dir() {
        assert!(Config::de(|_| None).is_err());
        let c = Config::de(|k| (k == "WARD_DEV_DIR").then(|| "/x/.dev".into())).unwrap();
        assert_eq!(c.caminhos.cofre, Path::new("/x/.dev/test-vault.ward"));
        assert_eq!(c.caminhos.estado, Path::new("/x/.dev/test-vault.state"));
        assert!(c.diretorio_socket_teste.unwrap().ends_with("ward-teste"));
        assert!(!c.extensao_de_teste);
    }

    /// Fase 41: a exceção da extensão só liga com o valor exato `chrome`.
    #[test]
    #[cfg(feature = "test-mode")]
    fn extensao_de_teste_so_com_chrome() {
        let com = |v: &'static str| {
            Config::de(move |k| match k {
                "WARD_DEV_DIR" => Some("/x/.dev".into()),
                "WARD_DEV_EXTENSAO" => Some(v.into()),
                _ => None,
            })
        };
        assert!(com("chrome").unwrap().extensao_de_teste);
        for ruim in ["", "1", "Chrome", "chrome ", "/opt/google/chrome/chrome"] {
            assert!(com(ruim).is_err(), "{ruim:?}");
        }
    }
}
