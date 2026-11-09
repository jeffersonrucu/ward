//! Cliente do socket do `wardd` (lado Rust do app; a webview nunca fala com o socket).
//!
//! Pontos de ligação com as fases do daemon (registrados no handoff da Fase 29):
//! - Fase 20 (protocolo): `DaemonSocket::estado` e `DaemonSocket::valor_para_copiar`
//!   passam a montar o quadro do protocolo; hoje devolvem `ErroApp::SemProtocolo`
//!   depois de conferir e abrir o socket.
//! - Fase 21 (plataforma): antes de mandar qualquer pedido (e, em especial, a senha
//!   mestra), conferir por `SO_PEERCRED` que o outro lado é o `wardd` instalado (D5,
//!   S5). Aqui só há a conferência do diretório e do socket (dono, 0700, tipo).
//! - Nome do arquivo do socket: `wardd.sock` é provisório; vale o que a Fase 20/21
//!   definir.

use std::fs;
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::ErroApp;
use crate::comandos::Referencia;
use crate::segredo::Segredo;

/// Diretório do socket dentro de `/run/user/<uid>/`. O build de teste usa outro
/// diretório (D7) e nunca lê variável de ambiente para escolher.
#[cfg(not(feature = "modo-teste"))]
pub const DIRETORIO: &str = "ward";
#[cfg(feature = "modo-teste")]
pub const DIRETORIO: &str = "ward-teste";

/// Nome provisório do socket (ver o topo do módulo).
pub const SOCKET: &str = "wardd.sock";

/// Prazo de leitura e escrita no socket.
const PRAZO: Duration = Duration::from_secs(5);

/// Estado do cofre como a interface vê.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EstadoCofre {
    SemCofre,
    Travado,
    Destravado,
}

impl EstadoCofre {
    pub fn codigo(self) -> &'static str {
        match self {
            EstadoCofre::SemCofre => "sem-cofre",
            EstadoCofre::Travado => "travado",
            EstadoCofre::Destravado => "destravado",
        }
    }
}

/// O que o app pede ao daemon. Os testes trocam por um daemon falso.
pub trait Daemon: Send {
    fn estado(&mut self) -> Result<EstadoCofre, ErroApp>;
    /// Valor de um campo secreto, só para o "copiar" (D4). Nunca sai para a webview.
    fn valor_para_copiar(&mut self, referencia: &Referencia) -> Result<Segredo, ErroApp>;
}

/// Daemon real, pelo socket local.
pub struct DaemonSocket {
    caminho: PathBuf,
    uid: u32,
}

impl DaemonSocket {
    /// `/run/user/<uid>/<DIRETORIO>/wardd.sock`. O uid vem do dono de `/proc/self`
    /// (o processo atual), não de `XDG_RUNTIME_DIR` nem de outra variável.
    pub fn padrao() -> Result<DaemonSocket, ErroApp> {
        let uid = fs::metadata("/proc/self")
            .map_err(|_| ErroApp::DaemonIndisponivel)?
            .uid();
        let caminho = Path::new("/run/user")
            .join(uid.to_string())
            .join(DIRETORIO)
            .join(SOCKET);
        Ok(DaemonSocket { caminho, uid })
    }

    /// Para os testes: outro caminho e outro dono esperado.
    pub fn em(caminho: PathBuf, uid: u32) -> DaemonSocket {
        DaemonSocket { caminho, uid }
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    /// Confere o diretório e o socket antes de conectar: diretório de verdade (não link),
    /// do usuário, sem permissão para grupo e outros (S5); socket de verdade, do usuário.
    pub fn conferir(&self) -> Result<(), ErroApp> {
        let diretorio = self.caminho.parent().ok_or(ErroApp::DaemonNaoConfiavel)?;
        let md = match fs::symlink_metadata(diretorio) {
            Ok(md) => md,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(ErroApp::DaemonIndisponivel);
            }
            Err(_) => return Err(ErroApp::DaemonNaoConfiavel),
        };
        if !md.file_type().is_dir() || md.uid() != self.uid || md.permissions().mode() & 0o077 != 0
        {
            return Err(ErroApp::DaemonNaoConfiavel);
        }
        let md = match fs::symlink_metadata(&self.caminho) {
            Ok(md) => md,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(ErroApp::DaemonIndisponivel);
            }
            Err(_) => return Err(ErroApp::DaemonNaoConfiavel),
        };
        if !md.file_type().is_socket() || md.uid() != self.uid {
            return Err(ErroApp::DaemonNaoConfiavel);
        }
        Ok(())
    }

    fn conectar(&self) -> Result<UnixStream, ErroApp> {
        self.conferir()?;
        let s = UnixStream::connect(&self.caminho).map_err(|_| ErroApp::DaemonIndisponivel)?;
        s.set_read_timeout(Some(PRAZO))
            .map_err(|_| ErroApp::DaemonIndisponivel)?;
        s.set_write_timeout(Some(PRAZO))
            .map_err(|_| ErroApp::DaemonIndisponivel)?;
        // Fase 21: conferir aqui o par por SO_PEERCRED (wardd instalado, D5) antes de
        // escrever qualquer coisa no socket.
        Ok(s)
    }
}

impl Daemon for DaemonSocket {
    fn estado(&mut self) -> Result<EstadoCofre, ErroApp> {
        let _conexao = self.conectar()?;
        // Fase 20: pedido "estado" do protocolo.
        Err(ErroApp::SemProtocolo)
    }

    fn valor_para_copiar(&mut self, _referencia: &Referencia) -> Result<Segredo, ErroApp> {
        let _conexao = self.conectar()?;
        // Fase 20: pedido "copiar" do papel app (D5), resposta com o valor num quadro
        // que vira `Segredo` direto, sem cópia intermediária em `String`.
        Err(ErroApp::SemProtocolo)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::os::unix::fs::{DirBuilderExt, symlink};
    use std::os::unix::net::UnixListener;

    fn temporario(nome: &str) -> PathBuf {
        let base =
            std::env::temp_dir().join(format!("ward-app-nucleo-{}-{nome}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
        base
    }

    fn meu_uid() -> u32 {
        fs::metadata("/proc/self").unwrap().uid()
    }

    #[test]
    fn caminho_padrao_sem_variavel_de_ambiente() {
        let d = DaemonSocket::padrao().unwrap();
        let esperado = format!("/run/user/{}/{}/wardd.sock", meu_uid(), DIRETORIO);
        assert_eq!(d.caminho(), Path::new(&esperado));
        #[cfg(not(feature = "modo-teste"))]
        assert!(!d.caminho().to_string_lossy().contains("ward-teste"));
    }

    #[test]
    fn socket_ausente_e_indisponivel() {
        let base = temporario("ausente");
        let mut d = DaemonSocket::em(base.join("ward").join(SOCKET), meu_uid());
        assert_eq!(d.estado(), Err(ErroApp::DaemonIndisponivel));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(base.join("ward"))
            .unwrap();
        assert_eq!(d.estado(), Err(ErroApp::DaemonIndisponivel));
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn ataque_diretorio_aberto_para_outros_e_recusado() {
        let base = temporario("aberto");
        let dir = base.join("ward");
        fs::DirBuilder::new().mode(0o755).create(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
        let _l = UnixListener::bind(dir.join(SOCKET)).unwrap();
        let mut d = DaemonSocket::em(dir.join(SOCKET), meu_uid());
        assert_eq!(d.estado(), Err(ErroApp::DaemonNaoConfiavel));
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(d.estado(), Err(ErroApp::SemProtocolo));
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn ataque_diretorio_por_link_simbolico_e_recusado() {
        let base = temporario("link");
        let real = base.join("real");
        fs::DirBuilder::new().mode(0o700).create(&real).unwrap();
        let _l = UnixListener::bind(real.join(SOCKET)).unwrap();
        symlink(&real, base.join("ward")).unwrap();
        let mut d = DaemonSocket::em(base.join("ward").join(SOCKET), meu_uid());
        assert_eq!(d.estado(), Err(ErroApp::DaemonNaoConfiavel));
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn ataque_arquivo_comum_no_lugar_do_socket_e_recusado() {
        let base = temporario("arquivo");
        let dir = base.join("ward");
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        fs::write(dir.join(SOCKET), b"").unwrap();
        let mut d = DaemonSocket::em(dir.join(SOCKET), meu_uid());
        assert_eq!(d.estado(), Err(ErroApp::DaemonNaoConfiavel));
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn ataque_dono_diferente_e_recusado() {
        let base = temporario("dono");
        let dir = base.join("ward");
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let _l = UnixListener::bind(dir.join(SOCKET)).unwrap();
        let mut d = DaemonSocket::em(dir.join(SOCKET), meu_uid().wrapping_add(1));
        assert_eq!(d.estado(), Err(ErroApp::DaemonNaoConfiavel));
        let r = Referencia::nova("acme-pg-dev", "senha").unwrap();
        assert!(matches!(
            d.valor_para_copiar(&r),
            Err(ErroApp::DaemonNaoConfiavel)
        ));
        fs::remove_dir_all(base).unwrap();
    }
}
