//! Socket local do daemon (S5, V3.1, V3.2).
//!
//! - [`preparar_diretorio`]: o diretório do socket é 0700 e do usuário. Se já existe com
//!   outra permissão, de outro dono ou como link simbólico, recusa (o daemon não sobe);
//!   nunca "conserta" o que achou. Cada pasta acima é do root ou do usuário, sem escrita
//!   para grupo e outros, sem link simbólico.
//! - [`DiretorioSocket::escutar`]: cria o socket 0600 dentro dele. Socket velho (ninguém
//!   escutando) é trocado; com outro daemon escutando, recusa.
//! - [`escutar_de_listen_fds`]: o daemon instalado não cria o socket. O systemd do
//!   sistema (`wardd@<uid>.socket`) cria `/run/ward/<uid>/wardd.sock` numa pasta do
//!   root e entrega o fd por `LISTEN_FDS` (D46); o daemon confere que o fd é um socket
//!   Unix em escuta no caminho esperado, numa pasta só do root, com o socket 0600 dele.
//! - [`Escuta::aceitar`] / [`conferir_conexao`]: fecha, sem responder, a conexão de uid
//!   diferente do dono, e devolve o motivo para o log. O pid e o pidfd do par seguem na
//!   [`Conexao`] para a identificação de D5 ([`crate::processo`]).

use std::fmt;
use std::fs::File;
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use crate::process::{PeerProcess, simple_absolute_path};
use crate::sys;

/// Pasta do Ward dentro de `/run/user/<uid>/`, com socket e temporários (S2).
pub const NOME_DIRETORIO: &str = "ward";
/// Pasta do daemon de teste (D7); nunca a mesma do daemon real.
pub const NOME_DIRETORIO_TESTE: &str = "ward-teste";
/// Nome do socket dentro da pasta.
pub const NOME_SOCKET: &str = "wardd.sock";
/// Pasta do root onde o systemd do sistema cria a pasta de cada usuário (D46).
pub const DIRETORIO_SISTEMA: &str = "/run/ward";
/// Maior nome de socket aceito.
const MAX_NOME_SOCKET: usize = 64;

/// `/run/user/<euid>`, pelo uid efetivo e não por `XDG_RUNTIME_DIR` (que vem do
/// ambiente).
pub fn diretorio_runtime() -> PathBuf {
    PathBuf::from(format!("/run/user/{}", sys::geteuid()))
}

/// `/run/ward/<euid>/wardd.sock`: o socket do daemon instalado (D46), pelo uid efetivo.
pub fn caminho_socket_sistema() -> PathBuf {
    PathBuf::from(format!(
        "{DIRETORIO_SISTEMA}/{}/{NOME_SOCKET}",
        sys::geteuid()
    ))
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
    if !simple_absolute_path(caminho) {
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
            id: Some((m.dev(), m.ino())),
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
    /// dev/inode do socket que este processo criou e apaga ao sair. `None` no socket
    /// do systemd, que não é nosso para apagar.
    id: Option<(u64, u64)>,
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
        if let Some(id) = self.id
            && let Ok(m) = std::fs::symlink_metadata(&self.caminho)
            && m.file_type().is_socket()
            && (m.dev(), m.ino()) == id
        {
            let _ = std::fs::remove_file(&self.caminho);
        }
    }
}

/// Problema com o socket recebido do systemd (D46). Sem valor de ambiente: pode ir para
/// o log.
#[derive(Debug)]
pub enum ErroAtivacao {
    /// Sem `LISTEN_PID`/`LISTEN_FDS` (o daemon não foi ativado pelo systemd).
    SemAtivacao,
    /// `LISTEN_PID` não é este processo, ou não é um número.
    PidDiferente,
    /// `LISTEN_FDS` diferente de 1.
    QuantidadeDeFds,
    /// O fd 3 já foi usado neste processo, ou não está aberto.
    FdIndisponivel(io::Error),
    /// O fd não é socket Unix de fluxo em escuta.
    NaoEscutaUnix,
    /// O socket escuta noutro caminho (ou num nome abstrato).
    CaminhoDiferente,
    /// Uma pasta do caminho não é do root, aceita escrita de grupo/outros ou é link.
    PastaNaoConfiavel(PathBuf),
    /// O arquivo do socket não é socket, não é deste usuário ou não é 0600.
    SocketNaoConfiavel,
    Io(io::Error),
}

impl fmt::Display for ErroAtivacao {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErroAtivacao::SemAtivacao => {
                f.write_str("sem socket do systemd (LISTEN_PID/LISTEN_FDS ausentes)")
            }
            ErroAtivacao::PidDiferente => f.write_str("LISTEN_PID não é este processo"),
            ErroAtivacao::QuantidadeDeFds => f.write_str("LISTEN_FDS tem de ser 1"),
            ErroAtivacao::FdIndisponivel(e) => write!(f, "fd 3 do systemd indisponível: {e}"),
            ErroAtivacao::NaoEscutaUnix => {
                f.write_str("o fd do systemd não é um socket Unix em escuta")
            }
            ErroAtivacao::CaminhoDiferente => {
                f.write_str("o socket do systemd não está no caminho esperado")
            }
            ErroAtivacao::PastaNaoConfiavel(p) => write!(
                f,
                "pasta do socket não é só do root (dono, permissão ou link): {}",
                p.display()
            ),
            ErroAtivacao::SocketNaoConfiavel => {
                f.write_str("o arquivo do socket não é um socket 0600 deste usuário")
            }
            ErroAtivacao::Io(e) => write!(f, "erro no socket do systemd: {e}"),
        }
    }
}

impl std::error::Error for ErroAtivacao {}

/// Número decimal simples (só dígitos, sem sinal nem espaço).
fn decimal(valor: &std::ffi::OsStr) -> Option<u32> {
    let b = valor.as_encoded_bytes();
    if b.is_empty() || b.len() > 10 || !b.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(b).ok()?.parse().ok()
}

/// Recebe o socket que o systemd do sistema criou e passou no fd 3 (D46) e confere:
/// - `LISTEN_PID` é este processo e `LISTEN_FDS` é 1;
/// - o fd é socket Unix de fluxo, em escuta (`SO_ACCEPTCONN`), com o nome
///   `caminho_esperado` exato (`/run/ward/<uid>/wardd.sock`, ver
///   [`caminho_socket_sistema`]);
/// - toda pasta do caminho é do root, sem escrita para grupo e outros, sem link;
/// - o arquivo do socket é socket, do uid efetivo e 0600.
///
/// O fd fica com `O_CLOEXEC` (nenhum filho do `ward run` herda o socket que escuta) e o
/// fd 3 original é fechado. Chamar no começo do `main`, uma vez. A [`Escuta`] devolvida
/// não apaga o socket ao sair (ele é do systemd). As variáveis `LISTEN_*` continuam no
/// ambiente: o daemon monta o ambiente de cada filho sem elas.
pub fn escutar_de_listen_fds(caminho_esperado: &Path) -> Result<Escuta, ErroAtivacao> {
    if !simple_absolute_path(caminho_esperado) {
        return Err(ErroAtivacao::CaminhoDiferente);
    }
    let pid = std::env::var_os("LISTEN_PID").ok_or(ErroAtivacao::SemAtivacao)?;
    let fds = std::env::var_os("LISTEN_FDS").ok_or(ErroAtivacao::SemAtivacao)?;
    if decimal(&pid) != Some(std::process::id()) {
        return Err(ErroAtivacao::PidDiferente);
    }
    if decimal(&fds) != Some(1) {
        return Err(ErroAtivacao::QuantidadeDeFds);
    }
    let copia = sys::dup_activation_fd().map_err(ErroAtivacao::FdIndisponivel)?;
    conferir_fd_de_escuta(copia.as_fd())?;
    let listener = UnixListener::from(copia);
    let nome = listener.local_addr().map_err(ErroAtivacao::Io)?;
    if nome.as_pathname() != Some(caminho_esperado) {
        return Err(ErroAtivacao::CaminhoDiferente);
    }
    conferir_caminho_do_sistema(caminho_esperado).map_err(|e| match e {
        RecusaCaminho::Pasta(p) => ErroAtivacao::PastaNaoConfiavel(p),
        RecusaCaminho::Socket | RecusaCaminho::Ausente => ErroAtivacao::SocketNaoConfiavel,
    })?;
    // A cópia é o mesmo socket do fd 3, então o original pode fechar.
    sys::close_activation_fd();
    Ok(Escuta {
        listener,
        caminho: caminho_esperado.to_path_buf(),
        id: None,
    })
}

/// Socket Unix de fluxo em escuta.
fn conferir_fd_de_escuta(fd: BorrowedFd<'_>) -> Result<(), ErroAtivacao> {
    let e_socket = fd
        .try_clone_to_owned()
        .map(File::from)
        .and_then(|f| f.metadata())
        .map(|m| m.file_type().is_socket())
        .map_err(ErroAtivacao::Io)?;
    let dominio = sys::sockopt_int(fd, libc::SO_DOMAIN).map_err(|_| ErroAtivacao::NaoEscutaUnix)?;
    let tipo = sys::sockopt_int(fd, libc::SO_TYPE).map_err(|_| ErroAtivacao::NaoEscutaUnix)?;
    let escuta =
        sys::sockopt_int(fd, libc::SO_ACCEPTCONN).map_err(|_| ErroAtivacao::NaoEscutaUnix)?;
    if !e_socket || dominio != libc::AF_UNIX || tipo != libc::SOCK_STREAM || escuta != 1 {
        return Err(ErroAtivacao::NaoEscutaUnix);
    }
    Ok(())
}

/// Por que o caminho do socket não é confiável.
#[derive(Debug)]
pub(crate) enum RecusaCaminho {
    /// O socket (ou uma pasta) não existe.
    Ausente,
    /// Pasta que não é do root (ou do usuário, no modo de teste), com escrita para grupo
    /// e outros, ou link.
    Pasta(PathBuf),
    /// O arquivo não é socket, não é do uid efetivo ou não é 0600.
    Socket,
}

/// D46: toda pasta acima do socket é do root, sem escrita para grupo e outros, sem
/// link; o socket é do uid efetivo e 0600.
pub(crate) fn conferir_caminho_do_sistema(socket: &Path) -> Result<(), RecusaCaminho> {
    let pasta = socket.parent().ok_or(RecusaCaminho::Socket)?;
    conferir_pastas(pasta, |m| m.uid() == 0)?;
    conferir_arquivo_socket(socket)
}

/// Daemon de teste (D7): a pasta do socket é do usuário e 0700; as de cima, do root ou
/// do usuário, sem escrita para grupo e outros; o socket é do usuário e 0600.
#[cfg(feature = "test-mode")]
pub(crate) fn conferir_caminho_de_teste(socket: &Path) -> Result<(), RecusaCaminho> {
    let euid = sys::geteuid();
    let pasta = socket.parent().ok_or(RecusaCaminho::Socket)?;
    let m = lstat(pasta)?;
    if !m.file_type().is_dir() || m.uid() != euid || m.mode() & 0o7777 != 0o700 {
        return Err(RecusaCaminho::Pasta(pasta.to_path_buf()));
    }
    if let Some(acima) = pasta.parent() {
        conferir_pastas(acima, |m| m.uid() == 0 || m.uid() == euid)?;
    }
    conferir_arquivo_socket(socket)
}

fn lstat(p: &Path) -> Result<std::fs::Metadata, RecusaCaminho> {
    std::fs::symlink_metadata(p).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            RecusaCaminho::Ausente
        } else {
            RecusaCaminho::Pasta(p.to_path_buf())
        }
    })
}

/// Cada pasta de `pasta` até `/`: diretório (não link), com dono aceito por `dono_ok`,
/// sem escrita para grupo e outros.
fn conferir_pastas(
    pasta: &Path,
    dono_ok: impl Fn(&std::fs::Metadata) -> bool,
) -> Result<(), RecusaCaminho> {
    if !simple_absolute_path(pasta) && pasta != Path::new("/") {
        return Err(RecusaCaminho::Pasta(pasta.to_path_buf()));
    }
    let mut atual = Some(pasta);
    while let Some(p) = atual {
        let m = lstat(p)?;
        if !m.file_type().is_dir() || !dono_ok(&m) || m.mode() & 0o022 != 0 {
            return Err(RecusaCaminho::Pasta(p.to_path_buf()));
        }
        atual = p.parent();
    }
    Ok(())
}

fn conferir_arquivo_socket(socket: &Path) -> Result<(), RecusaCaminho> {
    let m = match std::fs::symlink_metadata(socket) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(RecusaCaminho::Ausente),
        Err(_) => return Err(RecusaCaminho::Socket),
    };
    if !m.file_type().is_socket() || m.uid() != sys::geteuid() || m.mode() & 0o7777 != 0o600 {
        return Err(RecusaCaminho::Socket);
    }
    Ok(())
}

/// Conexão aceita, com o processo do outro lado.
#[derive(Debug)]
pub struct Conexao {
    pub stream: UnixStream,
    pub par: PeerProcess,
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
/// [`PeerProcess::classificar`].
pub fn conferir_conexao(stream: &impl AsFd) -> Result<PeerProcess, ErroAceitar> {
    let cred = sys::peercred(stream.as_fd()).map_err(ErroAceitar::SemIdentidade)?;
    if !uid_aceito(cred.uid, sys::geteuid()) {
        return Err(ErroAceitar::UidRecusado {
            uid: cred.uid,
            pid: cred.pid,
        });
    }
    let par = PeerProcess::from_socket(stream).map_err(ErroAceitar::SemIdentidade)?;
    // O `SO_PEERCRED` é fixo no `connect`; as duas leituras têm de bater.
    if par.credentials().uid != cred.uid {
        return Err(ErroAceitar::UidRecusado {
            uid: par.credentials().uid,
            pid: cred.pid,
        });
    }
    Ok(par)
}

/// O cliente do outro lado fechou (ou deu erro)? Não lê nem espera: os bytes de um
/// pedido seguinte continuam no socket. Para o daemon cancelar uma espera longa
/// (aprovação, destrave) quando a CLI cai (Fase 23).
pub fn par_fechou(stream: &impl AsFd) -> bool {
    sys::is_closed(stream.as_fd(), 0).unwrap_or(true)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn par_fechou_sem_consumir_bytes() {
        use std::io::{Read, Write};
        let (a, mut b) = UnixStream::pair().unwrap();
        assert!(!par_fechou(&a));
        b.write_all(b"x").unwrap();
        assert!(!par_fechou(&a));
        drop(b);
        assert!(par_fechou(&a));
        // O byte não foi consumido.
        let mut buf = [0u8; 2];
        assert_eq!((&a).read(&mut buf).unwrap(), 1);
    }

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
        let r = preparar_diretorio(Path::new("/tmp/ward-platform-falso-nao-criar"));
        assert!(matches!(r, Err(ErroDiretorio::PaiNaoConfiavel(_))), "{r:?}");
        assert!(!Path::new("/tmp/ward-platform-falso-nao-criar").exists());
    }

    #[test]
    fn listen_pid_e_fds_so_decimal_simples() {
        use std::ffi::OsStr;
        assert_eq!(decimal(OsStr::new("1")), Some(1));
        assert_eq!(decimal(OsStr::new("4242")), Some(4242));
        for ruim in ["", "+1", " 1", "1 ", "-1", "0x1", "1a", "99999999999"] {
            assert_eq!(decimal(OsStr::new(ruim)), None, "{ruim:?}");
        }
    }

    #[test]
    fn caminho_do_sistema_pelo_euid() {
        assert_eq!(
            caminho_socket_sistema(),
            PathBuf::from(format!("/run/ward/{}/wardd.sock", sys::geteuid()))
        );
    }

    #[test]
    fn ataque_pasta_do_usuario_nao_passa_por_pasta_do_sistema() {
        // O diretório de runtime é do usuário: um socket ali nunca passa como o do
        // systemd (D46).
        let pasta = diretorio_runtime().join(format!("ward-platform-u-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        let dir = preparar_diretorio(&pasta).unwrap();
        let escuta = dir.escutar(NOME_SOCKET).unwrap();
        assert!(matches!(
            conferir_caminho_do_sistema(escuta.caminho()),
            Err(RecusaCaminho::Pasta(_))
        ));
        assert!(conferir_fd_de_escuta(escuta.listener().as_fd()).is_ok());
        // Um socket conectado (não em escuta) e um arquivo comum não passam.
        let cliente = UnixStream::connect(escuta.caminho()).unwrap();
        assert!(matches!(
            conferir_fd_de_escuta(cliente.as_fd()),
            Err(ErroAtivacao::NaoEscutaUnix)
        ));
        let arquivo = File::open("/proc/self/status").unwrap();
        assert!(conferir_fd_de_escuta(arquivo.as_fd()).is_err());
        drop(escuta);
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn pastas_do_root_sao_aceitas_e_link_nao() {
        // `/usr/bin` é do root e 0755; `/bin` é link para `usr/bin` no Ubuntu 24.04.
        assert!(conferir_pastas(Path::new("/usr/bin"), |m| m.uid() == 0).is_ok());
        assert!(matches!(
            conferir_pastas(Path::new("/bin"), |m| m.uid() == 0),
            Err(RecusaCaminho::Pasta(_))
        ));
        assert!(matches!(
            conferir_pastas(Path::new("/tmp"), |m| m.uid() == 0),
            Err(RecusaCaminho::Pasta(_))
        ));
        assert!(matches!(
            conferir_caminho_do_sistema(Path::new("/run/ward-nao-existe-falso/1/wardd.sock")),
            Err(RecusaCaminho::Ausente)
        ));
    }

    #[test]
    fn runtime_pelo_euid() {
        assert_eq!(
            diretorio_runtime(),
            PathBuf::from(format!("/run/user/{}", sys::geteuid()))
        );
    }
}
