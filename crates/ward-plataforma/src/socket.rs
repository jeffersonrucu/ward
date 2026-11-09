//! Socket local do daemon (S5, V3.1, V3.2).
//!
//! - [`preparar_diretorio`]: o diretório do socket é 0700 e do usuário. Se já existe com
//!   outra permissão, de outro dono ou como link simbólico, recusa (o daemon não sobe);
//!   nunca "conserta" o que achou. Cada pasta acima é do root ou do usuário, sem escrita
//!   para grupo e outros, sem link simbólico.
//! - [`DiretorioSocket::escutar`]: cria o socket 0600 dentro dele. Socket velho (ninguém
//!   escutando) é trocado; com outro daemon escutando, recusa.
//! - [`Escuta::aceitar`] / [`conferir_conexao`]: fecha, sem responder, a conexão de uid
//!   diferente do dono, e devolve o motivo para o log. O pid e o pidfd do par seguem na
//!   [`Conexao`] para a identificação de D5 ([`crate::processo`]).

use std::fmt;
use std::fs::File;
use std::io;
use std::os::fd::AsFd;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use crate::processo::{ProcessoPar, caminho_absoluto_simples};
use crate::sys;

/// Pasta do Ward dentro de `/run/user/<uid>/`, com socket e temporários (S2).
pub const NOME_DIRETORIO: &str = "ward";
/// Pasta do daemon de teste (D7); nunca a mesma do daemon real.
pub const NOME_DIRETORIO_TESTE: &str = "ward-teste";
/// Nome do socket dentro da pasta.
pub const NOME_SOCKET: &str = "wardd.sock";
/// Maior nome de socket aceito.
const MAX_NOME_SOCKET: usize = 64;

/// `/run/user/<euid>`, pelo uid efetivo e não por `XDG_RUNTIME_DIR` (que vem do
/// ambiente).
pub fn diretorio_runtime() -> PathBuf {
    PathBuf::from(format!("/run/user/{}", sys::geteuid()))
}

/// Problema com o diretório do socket. Sem dado de segredo; pode ir para o log.
#[derive(Debug)]
pub enum ErroDiretorio {
    /// Caminho não absoluto, com `.`/`..` ou sem pasta acima.
    CaminhoInvalido,
    /// Uma pasta acima não é do root nem do usuário, aceita escrita de grupo/outros ou
    /// é link simbólico.
    PaiNaoConfiavel(PathBuf),
    /// Existe e não é diretório (inclui link simbólico).
    NaoEDiretorio,
    /// Existe e é de outro dono.
    DonoErrado {
        uid: u32,
    },
    /// Existe com permissão diferente de 0700.
    PermissaoErrada {
        modo: u32,
    },
    /// O diretório mudou entre a conferência e o uso.
    Trocado,
    Io(io::Error),
}

impl fmt::Display for ErroDiretorio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErroDiretorio::CaminhoInvalido => {
                f.write_str("caminho do diretório do socket inválido")
            }
            ErroDiretorio::PaiNaoConfiavel(p) => write!(
                f,
                "pasta acima do socket não confiável (dono ou permissão): {}",
                p.display()
            ),
            ErroDiretorio::NaoEDiretorio => {
                f.write_str("o caminho do socket existe e não é diretório")
            }
            ErroDiretorio::DonoErrado { uid } => {
                write!(f, "o diretório do socket é do uid {uid}, não deste usuário")
            }
            ErroDiretorio::PermissaoErrada { modo } => {
                write!(f, "o diretório do socket tem permissão {modo:o}, não 700")
            }
            ErroDiretorio::Trocado => f.write_str("o diretório do socket mudou durante o uso"),
            ErroDiretorio::Io(e) => write!(f, "erro no diretório do socket: {e}"),
        }
    }
}

impl std::error::Error for ErroDiretorio {}

impl From<io::Error> for ErroDiretorio {
    fn from(e: io::Error) -> Self {
        ErroDiretorio::Io(e)
    }
}

/// Diretório do socket conferido (0700, do usuário), mantido aberto.
#[derive(Debug)]
pub struct DiretorioSocket {
    caminho: PathBuf,
    aberto: File,
}

/// Prepara o diretório do socket: cria com 0700 se não existe; se existe, só aceita
/// diretório do usuário com 0700 exato.
pub fn preparar_diretorio(caminho: &Path) -> Result<DiretorioSocket, ErroDiretorio> {
    if !caminho_absoluto_simples(caminho) {
        return Err(ErroDiretorio::CaminhoInvalido);
    }
    let euid = sys::geteuid();
    let pai = caminho.parent().ok_or(ErroDiretorio::CaminhoInvalido)?;
    conferir_pastas_acima(pai, euid)?;

    match std::fs::DirBuilder::new().mode(0o700).create(caminho) {
        Ok(()) => {
            // O umask só tira bits; sobra no máximo 0700. Acerta para 0700 exato num
            // diretório que acabamos de criar (e é nosso).
            let meta = std::fs::symlink_metadata(caminho)?;
            if meta.file_type().is_dir() && meta.uid() == euid && meta.mode() & 0o7777 != 0o700 {
                std::fs::set_permissions(caminho, std::fs::Permissions::from_mode(0o700))?;
            }
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.into()),
    }

    let aberto = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(caminho)
        .map_err(|e| match e.raw_os_error() {
            Some(libc::ELOOP) | Some(libc::ENOTDIR) => ErroDiretorio::NaoEDiretorio,
            _ => ErroDiretorio::Io(e),
        })?;
    let dir = DiretorioSocket {
        caminho: caminho.to_path_buf(),
        aberto,
    };
    dir.conferir()?;
    Ok(dir)
}

/// Cada pasta de `pasta` até `/`: diretório (não link), do root ou de `euid`, sem
/// escrita para grupo e outros.
fn conferir_pastas_acima(pasta: &Path, euid: u32) -> Result<(), ErroDiretorio> {
    let mut atual = Some(pasta);
    while let Some(p) = atual {
        let m = std::fs::symlink_metadata(p)
            .map_err(|_| ErroDiretorio::PaiNaoConfiavel(p.to_path_buf()))?;
        let dono_ok = m.uid() == 0 || m.uid() == euid;
        if !m.file_type().is_dir() || !dono_ok || m.mode() & 0o022 != 0 {
            return Err(ErroDiretorio::PaiNaoConfiavel(p.to_path_buf()));
        }
        atual = p.parent();
    }
    Ok(())
}

impl DiretorioSocket {
    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    /// Confere o diretório aberto e que o caminho ainda leva a ele.
    fn conferir(&self) -> Result<(), ErroDiretorio> {
        let aberto = self.aberto.metadata()?;
        if !aberto.file_type().is_dir() {
            return Err(ErroDiretorio::NaoEDiretorio);
        }
        if aberto.uid() != sys::geteuid() {
            return Err(ErroDiretorio::DonoErrado { uid: aberto.uid() });
        }
        let modo = aberto.mode() & 0o7777;
        if modo != 0o700 {
            return Err(ErroDiretorio::PermissaoErrada { modo });
        }
        let pelo_nome = std::fs::symlink_metadata(&self.caminho)?;
        if !pelo_nome.file_type().is_dir()
            || (pelo_nome.dev(), pelo_nome.ino()) != (aberto.dev(), aberto.ino())
        {
            return Err(ErroDiretorio::Trocado);
        }
        Ok(())
    }

    /// Cria o socket `nome` (0600) e escuta nele.
    pub fn escutar(&self, nome: &str) -> Result<Escuta, ErroEscuta> {
        if nome.is_empty()
            || nome.len() > MAX_NOME_SOCKET
            || nome == "."
            || nome == ".."
            || nome.contains(['/', '\0'])
        {
            return Err(ErroEscuta::NomeInvalido);
        }
        self.conferir()?;
        let caminho = self.caminho.join(nome);
        let euid = sys::geteuid();

        match std::fs::symlink_metadata(&caminho) {
            Ok(m) => {
                if !m.file_type().is_socket() || m.uid() != euid {
                    return Err(ErroEscuta::CaminhoOcupado);
                }
                match UnixStream::connect(&caminho) {
                    Ok(_) => return Err(ErroEscuta::JaEmUso),
                    Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => {
                        std::fs::remove_file(&caminho)?;
                    }
                    Err(e) => return Err(e.into()),
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }

        let listener = UnixListener::bind(&caminho)?;
        // O diretório 0700 já barra os outros; o 0600 é a segunda camada (V3.1).
        std::fs::set_permissions(&caminho, std::fs::Permissions::from_mode(0o600))?;
        let m = std::fs::symlink_metadata(&caminho)?;
        if !m.file_type().is_socket() || m.uid() != euid || m.mode() & 0o7777 != 0o600 {
            let _ = std::fs::remove_file(&caminho);
            return Err(ErroEscuta::CaminhoOcupado);
        }
        self.conferir()?;
        Ok(Escuta {
            listener,
            caminho,
            id: (m.dev(), m.ino()),
        })
    }
}

/// Problema ao criar o socket.
#[derive(Debug)]
pub enum ErroEscuta {
    NomeInvalido,
    /// Outro daemon já escuta no socket.
    JaEmUso,
    /// O caminho existe e não é um socket deste usuário.
    CaminhoOcupado,
    Diretorio(ErroDiretorio),
    Io(io::Error),
}

impl fmt::Display for ErroEscuta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErroEscuta::NomeInvalido => f.write_str("nome de socket inválido"),
            ErroEscuta::JaEmUso => f.write_str("já há um daemon escutando no socket"),
            ErroEscuta::CaminhoOcupado => {
                f.write_str("o caminho do socket existe e não é um socket deste usuário")
            }
            ErroEscuta::Diretorio(e) => e.fmt(f),
            ErroEscuta::Io(e) => write!(f, "erro ao criar o socket: {e}"),
        }
    }
}

impl std::error::Error for ErroEscuta {}

impl From<io::Error> for ErroEscuta {
    fn from(e: io::Error) -> Self {
        ErroEscuta::Io(e)
    }
}

impl From<ErroDiretorio> for ErroEscuta {
    fn from(e: ErroDiretorio) -> Self {
        ErroEscuta::Diretorio(e)
    }
}

/// Socket escutando. Ao sair, apaga o arquivo do socket se ainda for o mesmo.
#[derive(Debug)]
pub struct Escuta {
    listener: UnixListener,
    caminho: PathBuf,
    id: (u64, u64),
}

impl Escuta {
    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    /// O `UnixListener` (por exemplo, para um laço assíncrono). Cada conexão aceita
    /// por ele passa por [`conferir_conexao`] antes de qualquer leitura.
    pub fn listener(&self) -> &UnixListener {
        &self.listener
    }

    /// Aceita uma conexão e confere o uid do par. Uid diferente: a conexão já foi
    /// fechada sem resposta quando o erro volta.
    pub fn aceitar(&self) -> Result<Conexao, ErroAceitar> {
        let (stream, _) = self.listener.accept().map_err(ErroAceitar::Io)?;
        let par = conferir_conexao(&stream)?;
        Ok(Conexao { stream, par })
    }
}

impl Drop for Escuta {
    fn drop(&mut self) {
        if let Ok(m) = std::fs::symlink_metadata(&self.caminho)
            && m.file_type().is_socket()
            && (m.dev(), m.ino()) == self.id
        {
            let _ = std::fs::remove_file(&self.caminho);
        }
    }
}

/// Conexão aceita, com o processo do outro lado.
#[derive(Debug)]
pub struct Conexao {
    pub stream: UnixStream,
    pub par: ProcessoPar,
}

/// Conexão recusada no `accept`. Só uid e pid (não são segredo) para o log.
#[derive(Debug)]
pub enum ErroAceitar {
    /// uid do par diferente do dono do daemon (V3.2).
    UidRecusado {
        uid: u32,
        pid: i32,
    },
    /// Sem `SO_PEERCRED`/`SO_PEERPIDFD` confiável (pid noutro namespace, kernel antigo).
    SemIdentidade(io::Error),
    Io(io::Error),
}

impl fmt::Display for ErroAceitar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErroAceitar::UidRecusado { uid, pid } => write!(
                f,
                "conexão recusada: uid {uid} (pid {pid}) diferente do dono do daemon"
            ),
            ErroAceitar::SemIdentidade(e) => {
                write!(f, "conexão recusada: sem identidade do processo ({e})")
            }
            ErroAceitar::Io(e) => write!(f, "erro ao aceitar conexão: {e}"),
        }
    }
}

impl std::error::Error for ErroAceitar {}

/// Decide pelo uid: só o dono do daemon passa.
fn uid_aceito(uid_par: u32, euid: u32) -> bool {
    uid_par == euid
}

/// Confere uma conexão recém-aceita (de qualquer laço, síncrono ou não): uid do par
/// igual ao do daemon e identidade do processo por pidfd. Em erro, quem chama fecha a
/// conexão sem responder (o `stream` é solto). O papel (D5) sai de
/// [`ProcessoPar::classificar`].
pub fn conferir_conexao(stream: &impl AsFd) -> Result<ProcessoPar, ErroAceitar> {
    let cred = sys::peercred(stream.as_fd()).map_err(ErroAceitar::SemIdentidade)?;
    if !uid_aceito(cred.uid, sys::geteuid()) {
        return Err(ErroAceitar::UidRecusado {
            uid: cred.uid,
            pid: cred.pid,
        });
    }
    let par = ProcessoPar::do_socket(stream).map_err(ErroAceitar::SemIdentidade)?;
    // O `SO_PEERCRED` é fixo no `connect`; as duas leituras têm de bater.
    if par.credenciais().uid != cred.uid {
        return Err(ErroAceitar::UidRecusado {
            uid: par.credenciais().uid,
            pid: cred.pid,
        });
    }
    Ok(par)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn uid_diferente_recusa() {
        assert!(uid_aceito(1000, 1000));
        assert!(!uid_aceito(4242, 1000));
        assert!(!uid_aceito(0, 1000));
        assert!(!uid_aceito(u32::MAX, 1000));
    }

    #[test]
    fn caminho_do_diretorio_invalido_recusa() {
        for ruim in ["", "relativo/ward", "/", "/run/user/../ward", "/run//ward"] {
            assert!(matches!(
                preparar_diretorio(Path::new(ruim)),
                Err(ErroDiretorio::CaminhoInvalido)
            ));
        }
    }

    #[test]
    fn pai_gravavel_por_outros_recusa() {
        // /tmp é 1777: qualquer um renomeia o que está dentro.
        let r = preparar_diretorio(Path::new("/tmp/ward-plataforma-falso-nao-criar"));
        assert!(matches!(r, Err(ErroDiretorio::PaiNaoConfiavel(_))), "{r:?}");
        assert!(!Path::new("/tmp/ward-plataforma-falso-nao-criar").exists());
    }

    #[test]
    fn runtime_pelo_euid() {
        assert_eq!(
            diretorio_runtime(),
            PathBuf::from(format!("/run/user/{}", sys::geteuid()))
        );
    }
}
