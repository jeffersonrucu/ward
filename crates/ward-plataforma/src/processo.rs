//! Identificação do processo do outro lado do socket (D5, S1, S5) e a cadeia de
//! processos para exibir na aprovação.
//!
//! Papel pelo grupo (D47, [`ProcessoPar::classificar`]): o `ward-app` e o `ward-nm` são
//! instalados setgid para os grupos `ward-app` e `ward-nm`. O `exec` setgid deixa o
//! processo não-dumpable desde a primeira instrução e põe o `ld.so` em modo seguro
//! (`AT_SECURE`), e o kernel fixa o egid no `SO_PEERCRED` no `connect`. O daemon dá:
//! - papel app ao egid `ward-app`, com o pai igual ao próprio daemon ou ao
//!   `systemd --user` do usuário e sem depurador;
//! - papel extensão ao egid `ward-nm`, sem depurador e com o pai igual ao Chrome
//!   instalado, conferido pelo `/proc` do pai;
//! - papel comum (agente) a qualquer outro;
//! - e nenhum papel especial com `kernel.yama.ptrace_scope` 0 ou `fs.suid_dumpable` 1
//!   ([`PoliticaKernel`]).
//!
//! A leitura de `exe`, `environ` e `maps` ([`ProcessoPar::identificar`]) fica só para o
//! pai Chrome e para testes; nunca dá papel sozinha:
//! - `/proc/<pid>/exe` é exatamente o caminho instalado, e o arquivo aberto por ele é o
//!   mesmo inode do caminho (um binário copiado para `/tmp` não passa);
//! - o binário e cada pasta acima dele são do root e sem escrita para grupo e outros;
//! - `TracerPid: 0`;
//! - nenhuma variável `LD_*` no ambiente inicial (`/proc/<pid>/environ`);
//! - toda biblioteca mapeada como executável é arquivo do root, sem escrita para grupo
//!   e outros, e é o mesmo inode do mapeamento (no pai Chrome, o `maps` não é lido).
//!
//! Qualquer falha, inclusive de leitura, cai no papel comum (agente): a regra falha
//! fechada. Contra reaproveitamento de pid, o daemon usa o pidfd do `SO_PEERPIDFD`:
//! abre `/proc/<pid>` e só depois confere pelo pidfd que o processo original continua
//! vivo; daí em diante tudo é lido relativo àquele diretório aberto.

use std::collections::HashSet;
use std::ffi::CStr;
use std::fmt;
use std::fs::{File, Metadata};
use std::io::{self, Read};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};

use crate::papel::{Instalacao, PoliticaKernel};
use crate::sys;

/// Limite para ler `/proc/<pid>/status`.
const MAX_STATUS: usize = 64 * 1024;
/// Limite para ler `/proc/<pid>/environ`. Ambiente maior que isso recusa.
pub const MAX_AMBIENTE: usize = 1024 * 1024;
/// Limite para ler `/proc/<pid>/maps`. Mapa maior que isso recusa.
pub const MAX_MAPAS: usize = 8 * 1024 * 1024;
/// Profundidade máxima da cadeia de processos exibida.
pub const MAX_CADEIA: usize = 32;
/// Tamanho máximo do nome de cada elemento da cadeia, em caracteres.
pub const MAX_NOME_CADEIA: usize = 64;

/// Por que um processo não foi identificado como o binário instalado. Sem caminho, sem
/// valor de variável e sem conteúdo lido do processo: pode ir para o log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recusa {
    /// uid do par diferente do dono deste processo.
    UidDiferente,
    /// O processo saiu (ou o pid já é de outro) durante a conferência.
    ProcessoSumiu,
    /// Sem permissão para ler `/proc/<pid>` (processo não-dumpable ou de outro dono).
    SemAcesso,
    /// Outro erro de leitura do `/proc`, ou conteúdo fora do formato.
    Leitura,
    /// `/proc/<pid>/exe` não é o caminho instalado.
    ExeNaoPermitido,
    /// O caminho é o instalado, mas o arquivo em execução é outro inode.
    ExeTrocado,
    /// O binário instalado (ou uma pasta acima dele) não é do root, aceita escrita de
    /// grupo/outros, é link simbólico ou não existe.
    InstalacaoNaoConfiavel,
    /// `TracerPid` ≠ 0.
    Depurador,
    /// Variável `LD_*` no ambiente (só o nome conhecido, ou `LD_*`).
    VariavelLd(&'static str),
    /// Ambiente acima de [`MAX_AMBIENTE`].
    AmbienteGrande,
    /// Biblioteca executável fora do root, apagada, `memfd` ou mapa grande demais.
    BibliotecaNaoConfiavel,
    /// O pai do host da extensão não é o Chrome instalado.
    PaiNaoPermitido,
    /// O egid do par não é o grupo do app nem o do host (D47): cliente comum.
    GrupoComum,
    /// `ptrace_scope` 0, `suid_dumpable` 1 ou ilegíveis: sem papel especial (D47).
    PoliticaKernel,
    /// O pai do app não é o daemon nem o `systemd --user` do usuário (D47).
    PaiDoAppNaoPermitido,
}

impl fmt::Display for Recusa {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Recusa::UidDiferente => "uid do processo diferente do dono do daemon",
            Recusa::ProcessoSumiu => "o processo saiu durante a conferência",
            Recusa::SemAcesso => "sem permissão para ler /proc do processo",
            Recusa::Leitura => "não deu para ler /proc do processo",
            Recusa::ExeNaoPermitido => "o executável não é o binário instalado",
            Recusa::ExeTrocado => "o executável em execução não é o arquivo instalado",
            Recusa::InstalacaoNaoConfiavel => {
                "o binário instalado não é do root ou aceita escrita de outros"
            }
            Recusa::Depurador => "processo sob depurador (TracerPid ≠ 0)",
            Recusa::VariavelLd(nome) => return write!(f, "variável {nome} no ambiente"),
            Recusa::AmbienteGrande => "ambiente do processo grande demais",
            Recusa::BibliotecaNaoConfiavel => "biblioteca executável fora do root",
            Recusa::PaiNaoPermitido => "o processo pai não é o Chrome instalado",
            Recusa::GrupoComum => "o grupo do processo não é o do app nem o da extensão",
            Recusa::PoliticaKernel => {
                "ptrace_scope 0 ou suid_dumpable 1: papel de app e extensão desligado"
            }
            Recusa::PaiDoAppNaoPermitido => {
                "o pai do app não é o daemon nem o systemd --user do usuário"
            }
        };
        f.write_str(s)
    }
}

impl std::error::Error for Recusa {}

fn recusa_de_io(e: &io::Error) -> Recusa {
    match e.raw_os_error() {
        Some(libc::EACCES) | Some(libc::EPERM) => Recusa::SemAcesso,
        Some(libc::ENOENT) | Some(libc::ESRCH) => Recusa::ProcessoSumiu,
        _ => Recusa::Leitura,
    }
}

/// Nome de variável `LD_*` que pode ir para log: os do carregador que importam pelo
/// nome, ou `LD_*` para qualquer outro (o nome inventado não vai para o log).
pub(crate) fn nome_ld_para_log(nome: &[u8]) -> &'static str {
    match nome {
        b"LD_PRELOAD" => "LD_PRELOAD",
        b"LD_AUDIT" => "LD_AUDIT",
        b"LD_LIBRARY_PATH" => "LD_LIBRARY_PATH",
        _ => "LD_*",
    }
}

/// Valor numérico de um campo de `/proc/<pid>/status` (`"TracerPid:"`, `"PPid:"`).
fn campo_status(status: &[u8], campo: &[u8]) -> Option<u64> {
    status.split(|&b| b == b'\n').find_map(|linha| {
        let resto = linha.strip_prefix(campo)?;
        let texto = std::str::from_utf8(resto).ok()?.trim();
        texto.parse().ok()
    })
}

/// `TracerPid` de um `/proc/<pid>/status`.
pub(crate) fn ler_tracer_pid(status: &[u8]) -> Option<u64> {
    campo_status(status, b"TracerPid:")
}

/// Binário instalado aceito para um papel: caminho absoluto, sem `.`/`..`. Só guarda o
/// caminho; o dono e as permissões são conferidos a cada identificação.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BinarioPermitido {
    caminho: PathBuf,
}

/// Caminho de binário permitido inválido.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaminhoInvalido;

impl fmt::Display for CaminhoInvalido {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("caminho de binário inválido: tem de ser absoluto, sem . nem ..")
    }
}

impl std::error::Error for CaminhoInvalido {}

/// Confere a forma do caminho: absoluto, só componentes normais, com pelo menos um.
pub(crate) fn caminho_absoluto_simples(caminho: &Path) -> bool {
    let bytes = caminho.as_os_str().as_bytes();
    if !bytes.starts_with(b"/") || bytes.ends_with(b"/") || bytes.contains(&0) {
        return false;
    }
    let mut componentes = caminho.components();
    if componentes.next() != Some(Component::RootDir) {
        return false;
    }
    let mut algum = false;
    for c in componentes {
        match c {
            Component::Normal(_) => algum = true,
            _ => return false,
        }
    }
    // `components()` esconde `a//b` e `a/./b`; a forma tem de ser a canônica.
    algum
        && caminho
            .components()
            .collect::<PathBuf>()
            .as_os_str()
            .as_bytes()
            == bytes
}

impl BinarioPermitido {
    pub fn novo(caminho: impl Into<PathBuf>) -> Result<Self, CaminhoInvalido> {
        let caminho = caminho.into();
        if !caminho_absoluto_simples(&caminho) {
            return Err(CaminhoInvalido);
        }
        Ok(BinarioPermitido { caminho })
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    /// O arquivo instalado e cada pasta acima dele: do root, sem escrita para grupo e
    /// outros, sem link simbólico no caminho. Devolve os metadados do arquivo.
    fn conferir_instalado(&self) -> Result<Metadata, Recusa> {
        conferir_arquivo_do_root(&self.caminho).map_err(|_| Recusa::InstalacaoNaoConfiavel)
    }
}

/// Arquivo regular do root, sem escrita para grupo/outros, com todas as pastas acima
/// também do root e sem escrita para grupo/outros, sem link simbólico no caminho.
fn conferir_arquivo_do_root(caminho: &Path) -> Result<Metadata, ()> {
    let meta = std::fs::symlink_metadata(caminho).map_err(|_| ())?;
    if !meta.file_type().is_file() || !do_root_sem_escrita(&meta) {
        return Err(());
    }
    let mut atual = caminho.parent();
    while let Some(pasta) = atual {
        let m = std::fs::symlink_metadata(pasta).map_err(|_| ())?;
        if !m.file_type().is_dir() || !do_root_sem_escrita(&m) {
            return Err(());
        }
        atual = pasta.parent();
    }
    Ok(meta)
}

fn do_root_sem_escrita(m: &Metadata) -> bool {
    m.uid() == 0 && m.mode() & 0o022 == 0
}

/// Credenciais do `SO_PEERCRED`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CredenciaisPar {
    pub pid: u32,
    pub uid: u32,
    pub gid: u32,
}

/// Processo vivo apontado por pid + pidfd.
#[derive(Debug)]
struct Alvo {
    pid: u32,
    pidfd: OwnedFd,
}

impl Alvo {
    fn do_pid(pid: u32) -> io::Result<Alvo> {
        let p = libc::pid_t::try_from(pid).map_err(|_| io::ErrorKind::InvalidInput)?;
        let pidfd = sys::pidfd_open(p)?;
        Ok(Alvo { pid, pidfd })
    }

    /// Abre `/proc/<pid>` e confere, depois de abrir, que o processo do pidfd continua
    /// vivo: assim o diretório é dele, e não de outro que herdou o número.
    fn vista(&self) -> Result<Vista, Recusa> {
        let dir = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(format!("/proc/{}", self.pid))
            .map_err(|e| recusa_de_io(&e))?;
        self.conferir_vivo()?;
        Ok(Vista { dir: dir.into() })
    }

    fn conferir_vivo(&self) -> Result<(), Recusa> {
        match sys::pidfd_vivo(self.pidfd.as_fd()) {
            Ok(true) => Ok(()),
            Ok(false) => Err(Recusa::ProcessoSumiu),
            Err(e) => Err(recusa_de_io(&e)),
        }
    }
}

/// `/proc/<pid>` aberto; tudo é lido relativo a ele.
struct Vista {
    dir: OwnedFd,
}

impl Vista {
    fn ler(&self, nome: &CStr, limite: usize) -> Result<Vec<u8>, Recusa> {
        let fd = sys::openat(self.dir.as_fd(), nome, libc::O_RDONLY | libc::O_NOFOLLOW)
            .map_err(|e| recusa_de_io(&e))?;
        ler_limitado(File::from(fd), limite)
    }

    fn status(&self) -> Result<Vec<u8>, Recusa> {
        self.ler(c"status", MAX_STATUS)
    }
}

/// Lê no máximo `limite` bytes; mais que isso é [`Recusa::Leitura`].
fn ler_limitado(arquivo: impl Read, limite: usize) -> Result<Vec<u8>, Recusa> {
    let mut buf = Vec::new();
    let teto = u64::try_from(limite)
        .map_err(|_| Recusa::Leitura)?
        .saturating_add(1);
    arquivo
        .take(teto)
        .read_to_end(&mut buf)
        .map_err(|e| recusa_de_io(&e))?;
    if buf.len() > limite {
        return Err(Recusa::Leitura);
    }
    Ok(buf)
}

/// pid que o kernel associa ao pidfd (`/proc/self/fdinfo/<fd>`, linha `Pid:`). `None`
/// quando o processo já saiu (`-1`) ou está noutro namespace de pid (`0`).
fn pid_do_pidfd(pidfd: BorrowedFd<'_>) -> io::Result<Option<u32>> {
    let info = std::fs::read(format!("/proc/self/fdinfo/{}", pidfd.as_raw_fd()))?;
    let linha = info
        .split(|&b| b == b'\n')
        .find_map(|l| l.strip_prefix(b"Pid:"))
        .ok_or(io::ErrorKind::InvalidData)?;
    let texto = std::str::from_utf8(linha).map_err(|_| io::ErrorKind::InvalidData)?;
    let pid: i64 = texto
        .trim()
        .parse()
        .map_err(|_| io::ErrorKind::InvalidData)?;
    Ok(u32::try_from(pid).ok().filter(|&p| p > 0))
}

/// O processo do outro lado de um socket Unix: credenciais do `SO_PEERCRED` e pidfd do
/// `SO_PEERPIDFD` (o mesmo processo, conferido pelo pid).
#[derive(Debug)]
pub struct ProcessoPar {
    cred: CredenciaisPar,
    alvo: Alvo,
}

impl ProcessoPar {
    /// Lê `SO_PEERCRED` e `SO_PEERPIDFD` do socket. Falha (o daemon deve fechar a
    /// conexão) quando o pid é desconhecido (outro namespace), quando o kernel não dá
    /// o pidfd ou quando o pidfd não é do mesmo pid.
    pub fn do_socket(socket: &impl AsFd) -> io::Result<ProcessoPar> {
        let fd = socket.as_fd();
        let c = sys::peercred(fd)?;
        let pid = u32::try_from(c.pid)
            .ok()
            .filter(|&p| p > 0)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "pid do par desconhecido"))?;
        let pidfd = sys::peer_pidfd(fd)?;
        if pid_do_pidfd(pidfd.as_fd())? != Some(pid) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "pidfd do par não confere com o pid",
            ));
        }
        Ok(ProcessoPar {
            cred: CredenciaisPar {
                pid,
                uid: c.uid,
                gid: c.gid,
            },
            alvo: Alvo { pid, pidfd },
        })
    }

    pub fn credenciais(&self) -> CredenciaisPar {
        self.cred
    }

    /// Conferência de D5 pelo `/proc` (exe, dono root, `TracerPid`, `LD_*`, `maps`).
    /// Desde D47 ela não dá papel: serve ao pai Chrome ([`conferir_pai`](Self::conferir_pai))
    /// e aos testes. Contra um processo não-dumpable (o app e o host setgid, o `wardd`)
    /// falha fechada com [`Recusa::SemAcesso`].
    pub fn identificar(&self, binario: &BinarioPermitido) -> Result<(), Recusa> {
        self.conferir_uid()?;
        identificar_imagem(&self.alvo, binario, true)
    }

    /// pid do pai, com o pidfd dele aberto e o `PPid` relido depois (se o pai tivesse
    /// saído e o número fosse de outro, o filho já teria outro `PPid`).
    fn pai(&self) -> Result<Alvo, Recusa> {
        let vista = self.alvo.vista()?;
        let ppid = campo_status(&vista.status()?, b"PPid:").ok_or(Recusa::Leitura)?;
        let ppid = u32::try_from(ppid).map_err(|_| Recusa::Leitura)?;
        if ppid == 0 {
            return Err(Recusa::Leitura);
        }
        let pai = Alvo::do_pid(ppid).map_err(|e| recusa_de_io(&e))?;
        let de_novo = campo_status(&vista.status()?, b"PPid:").ok_or(Recusa::Leitura)?;
        self.alvo.conferir_vivo()?;
        if de_novo != u64::from(ppid) {
            return Err(Recusa::ProcessoSumiu);
        }
        Ok(pai)
    }

    /// O pai do par é o `chrome` instalado (host da extensão): exe, dono root,
    /// depurador e `LD_*` lidos do `/proc` do pai. Do Chrome não se conferem as
    /// bibliotecas: ele mapeia componentes baixados para a pasta do usuário (Widevine).
    pub fn conferir_pai(&self, chrome: &BinarioPermitido) -> Result<(), Recusa> {
        let pai = self.pai().map_err(|_| Recusa::PaiNaoPermitido)?;
        if pai.pid <= 1 {
            return Err(Recusa::PaiNaoPermitido);
        }
        identificar_imagem(&pai, chrome, false).map_err(|_| Recusa::PaiNaoPermitido)?;
        self.alvo.conferir_vivo()
    }

    /// O pai do app é este processo (o daemon o abriu) ou o `systemd --user` do
    /// usuário (D47).
    fn conferir_pai_do_app(&self) -> Result<(), Recusa> {
        let pai = self.pai().map_err(|r| match r {
            Recusa::ProcessoSumiu => r,
            _ => Recusa::PaiDoAppNaoPermitido,
        })?;
        if pai.pid == std::process::id() {
            return self.alvo.conferir_vivo();
        }
        e_gerente_systemd_do_usuario(&pai)?;
        self.alvo.conferir_vivo()
    }

    /// Papel pelo egid do `SO_PEERCRED` (D47). Qualquer falha dá [`ClassePar::Comum`]
    /// com o motivo para o log.
    pub fn classificar(&self, inst: &Instalacao, politica: &PoliticaKernel) -> ClassePar {
        let gid = self.cred.gid;
        let r = if gid == inst.gid_app() {
            self.conferir_especial(politica)
                .and_then(|()| self.conferir_pai_do_app())
                .map(|()| ClassePar::App)
        } else if gid == inst.gid_extensao() {
            self.conferir_especial(politica)
                .and_then(|()| self.conferir_pai(inst.chrome()))
                .map(|()| ClassePar::HostExtensao)
        } else {
            Err(Recusa::GrupoComum)
        };
        // Depurador preso no meio da conferência também recusa.
        match r.and_then(|c| self.reconferir_sem_depurador().map(|()| c)) {
            Ok(c) => c,
            Err(motivo) => ClassePar::Comum { motivo },
        }
    }

    /// Antes de qualquer papel especial: mesmo uid, política do kernel e sem depurador.
    fn conferir_especial(&self, politica: &PoliticaKernel) -> Result<(), Recusa> {
        self.conferir_uid()?;
        politica.conferir()?;
        self.reconferir_sem_depurador()
    }

    /// Reconfere, antes de cada operação sensível de app ou extensão, que o processo
    /// continua vivo e sem depurador. `/proc/<pid>/status` é legível mesmo depois que
    /// o cliente fica não-dumpable.
    pub fn reconferir_sem_depurador(&self) -> Result<(), Recusa> {
        let vista = self.alvo.vista()?;
        match ler_tracer_pid(&vista.status()?) {
            Some(0) => self.alvo.conferir_vivo(),
            Some(_) => Err(Recusa::Depurador),
            None => Err(Recusa::Leitura),
        }
    }

    /// Cadeia de processos do par, para exibir (ver [`cadeia_de_processos`]).
    pub fn cadeia(&self) -> Vec<ElementoCadeia> {
        cadeia_de_processos(self.cred.pid)
    }

    fn conferir_uid(&self) -> Result<(), Recusa> {
        if self.cred.uid != sys::geteuid() {
            return Err(Recusa::UidDiferente);
        }
        Ok(())
    }
}

/// Executável e linha de comando do gerente de serviços do usuário.
const CMDLINE_SYSTEMD_USUARIO: &[u8] = b"/usr/lib/systemd/systemd\0--user\0";

/// `pai` é o `systemd --user` deste usuário: `Name: systemd`, os quatro uids iguais ao
/// do daemon, `PPid: 1`, sem depurador, `cmdline` exata e o cgroup `init.scope` do
/// `user@<uid>.service`. O gerente é não-dumpable (o `exe` dele não se lê), então essa
/// é a conferência possível pelo `/proc`; ela não é prova forte (ver o handoff da
/// Fase 21b): o próprio `systemd --user` roda as units de `~/.config/systemd/user/`.
fn e_gerente_systemd_do_usuario(pai: &Alvo) -> Result<(), Recusa> {
    let recusa = |_| Recusa::PaiDoAppNaoPermitido;
    let vista = pai.vista().map_err(recusa)?;
    let status = vista.status().map_err(recusa)?;
    let uid = sys::geteuid();
    let nome_ok = status
        .split(|&b| b == b'\n')
        .any(|l| l == b"Name:\tsystemd");
    let uids_ok = status
        .split(|&b| b == b'\n')
        .find_map(|l| l.strip_prefix(b"Uid:"))
        .and_then(|l| std::str::from_utf8(l).ok())
        .is_some_and(|l| {
            let v: Vec<&str> = l.split_whitespace().collect();
            v.len() == 4 && v.iter().all(|u| u.parse::<u32>().ok() == Some(uid))
        });
    let pai_ok = campo_status(&status, b"PPid:") == Some(1);
    let tracer_ok = ler_tracer_pid(&status) == Some(0);
    let cmdline = vista.ler(c"cmdline", 4096).map_err(recusa)?;
    let cgroup = vista.ler(c"cgroup", 4096).map_err(recusa)?;
    let cgroup_esperado =
        format!("0::/user.slice/user-{uid}.slice/user@{uid}.service/init.scope\n");
    if nome_ok
        && uids_ok
        && pai_ok
        && tracer_ok
        && cmdline == CMDLINE_SYSTEMD_USUARIO
        && cgroup == cgroup_esperado.as_bytes()
    {
        pai.conferir_vivo().map_err(recusa)
    } else {
        Err(Recusa::PaiDoAppNaoPermitido)
    }
}

/// Papel que a plataforma reconhece no par. O protocolo (Fase 20) mapeia: `App` → papel
/// app, `HostExtensao` → papel extensão, `Comum` → papel agente.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassePar {
    App,
    HostExtensao,
    /// Cliente comum, com o motivo (para o log).
    Comum {
        motivo: Recusa,
    },
}

/// Conferências de [`identificar_imagem`] sobre um processo vivo.
fn identificar_imagem(
    alvo: &Alvo,
    binario: &BinarioPermitido,
    conferir_mapas: bool,
) -> Result<(), Recusa> {
    let vista = alvo.vista()?;
    conferir_tracer(&vista)?;

    let exe = sys::readlinkat(vista.dir.as_fd(), c"exe").map_err(|e| recusa_de_io(&e))?;
    if exe != binario.caminho.as_os_str().as_bytes() {
        return Err(Recusa::ExeNaoPermitido);
    }
    let instalado = binario.conferir_instalado()?;
    // O arquivo de fato em execução (o link mágico aberto com O_PATH), não o nome.
    let em_execucao = sys::openat(vista.dir.as_fd(), c"exe", libc::O_PATH)
        .map_err(|e| recusa_de_io(&e))
        .map(File::from)?
        .metadata()
        .map_err(|e| recusa_de_io(&e))?;
    if (em_execucao.dev(), em_execucao.ino()) != (instalado.dev(), instalado.ino())
        || !do_root_sem_escrita(&em_execucao)
    {
        return Err(Recusa::ExeTrocado);
    }

    let ambiente = vista.ler(c"environ", MAX_AMBIENTE).map_err(|r| {
        if r == Recusa::Leitura {
            Recusa::AmbienteGrande
        } else {
            r
        }
    })?;
    conferir_ambiente(&ambiente)?;

    if conferir_mapas {
        let mapas = vista.ler(c"maps", MAX_MAPAS).map_err(|r| {
            if r == Recusa::Leitura {
                Recusa::BibliotecaNaoConfiavel
            } else {
                r
            }
        })?;
        conferir_mapas_executaveis(&mapas)?;
    }

    // De novo no fim: um depurador preso no meio da conferência também recusa.
    conferir_tracer(&vista)?;
    alvo.conferir_vivo()
}

fn conferir_tracer(vista: &Vista) -> Result<(), Recusa> {
    match ler_tracer_pid(&vista.status()?) {
        Some(0) => Ok(()),
        Some(_) => Err(Recusa::Depurador),
        None => Err(Recusa::Leitura),
    }
}

/// `environ` é `NOME=valor\0…`. Recusa qualquer nome com prefixo `LD_`. O valor nunca
/// é olhado.
fn conferir_ambiente(ambiente: &[u8]) -> Result<(), Recusa> {
    for item in ambiente.split(|&b| b == 0) {
        let nome = item.split(|&b| b == b'=').next().unwrap_or_default();
        if nome.starts_with(b"LD_") {
            return Err(Recusa::VariavelLd(nome_ld_para_log(nome)));
        }
    }
    Ok(())
}

/// Linha de `/proc/<pid>/maps`: `início-fim perms offset dev inode caminho`.
struct Mapa<'a> {
    executavel: bool,
    inode: u64,
    caminho: &'a [u8],
}

fn ler_linha_mapa(linha: &[u8]) -> Option<Mapa<'_>> {
    let mut resto = linha;
    let mut campos = [&b""[..]; 5];
    for campo in &mut campos {
        let inicio = resto.iter().position(|&b| b != b' ')?;
        resto = &resto[inicio..];
        let fim = resto.iter().position(|&b| b == b' ').unwrap_or(resto.len());
        *campo = &resto[..fim];
        resto = &resto[fim..];
    }
    let perms = campos[1];
    let inode = std::str::from_utf8(campos[4]).ok()?.parse().ok()?;
    let inicio = resto.iter().position(|&b| b != b' ').unwrap_or(resto.len());
    Some(Mapa {
        executavel: perms.get(2) == Some(&b'x'),
        inode,
        caminho: &resto[inicio..],
    })
}

fn conferir_mapas_executaveis(mapas: &[u8]) -> Result<(), Recusa> {
    let mut conferidos: HashSet<(&[u8], u64)> = HashSet::new();
    for linha in mapas.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        let mapa = ler_linha_mapa(linha).ok_or(Recusa::BibliotecaNaoConfiavel)?;
        // Anônimo, `[vdso]`, `[stack]`: sem arquivo para conferir.
        if !mapa.executavel || !mapa.caminho.starts_with(b"/") {
            continue;
        }
        if !conferidos.insert((mapa.caminho, mapa.inode)) {
            continue;
        }
        // Apagado, trocado ou `memfd:` (aparece como "/memfd:… (deleted)").
        if mapa.caminho.ends_with(b" (deleted)") {
            return Err(Recusa::BibliotecaNaoConfiavel);
        }
        let caminho = Path::new(std::ffi::OsStr::from_bytes(mapa.caminho));
        let meta = conferir_arquivo_do_root(caminho).map_err(|_| Recusa::BibliotecaNaoConfiavel)?;
        if meta.ino() != mapa.inode {
            return Err(Recusa::BibliotecaNaoConfiavel);
        }
    }
    Ok(())
}

/// Um processo da cadeia, para exibir. O nome é o `comm` do kernel, que o próprio
/// processo pode mudar: serve para mostrar ao usuário, nunca para decidir.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementoCadeia {
    pub pid: u32,
    pub nome: String,
}

/// Cadeia de processos de `pid` até o mais antigo que der para ler, do mais antigo para
/// o mais novo (`claude → bash → ward`). Até [`MAX_CADEIA`] elementos; nomes
/// higienizados (sem caractere de controle nem de direção de texto, até
/// [`MAX_NOME_CADEIA`] caracteres). Só para exibir: entre uma leitura e outra os pids
/// podem mudar.
pub fn cadeia_de_processos(pid: u32) -> Vec<ElementoCadeia> {
    let mut cadeia = Vec::new();
    let mut vistos = HashSet::new();
    let mut atual = pid;
    while atual > 0 && cadeia.len() < MAX_CADEIA && vistos.insert(atual) {
        let Ok(arquivo) = File::open(format!("/proc/{atual}/status")) else {
            break;
        };
        let Ok(status) = ler_limitado(arquivo, MAX_STATUS) else {
            break;
        };
        let nome = status
            .split(|&b| b == b'\n')
            .find_map(|l| l.strip_prefix(b"Name:"))
            .map(higienizar_nome)
            .unwrap_or_else(|| "?".to_owned());
        cadeia.push(ElementoCadeia { pid: atual, nome });
        let Some(ppid) = campo_status(&status, b"PPid:").and_then(|p| u32::try_from(p).ok()) else {
            break;
        };
        atual = ppid;
    }
    cadeia.reverse();
    cadeia
}

fn higienizar_nome(bruto: &[u8]) -> String {
    let texto = String::from_utf8_lossy(bruto);
    let nome: String = texto
        .trim()
        .chars()
        .map(|c| {
            let direcao = matches!(c, '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}');
            if c.is_control() || direcao { '?' } else { c }
        })
        .take(MAX_NOME_CADEIA)
        .collect();
    if nome.is_empty() {
        "?".to_owned()
    } else {
        nome
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn caminho_permitido_so_absoluto_e_canonico() {
        assert!(BinarioPermitido::novo("/usr/local/bin/ward-app").is_ok());
        for ruim in [
            "",
            "/",
            "ward-app",
            "./ward-app",
            "/usr/local/../tmp/ward-app",
            "/usr/./bin/x",
            "/usr//bin/x",
            "/usr/bin/",
        ] {
            assert_eq!(BinarioPermitido::novo(ruim), Err(CaminhoInvalido), "{ruim}");
        }
    }

    #[test]
    fn ambiente_com_qualquer_ld_recusa_sem_valor() {
        assert_eq!(conferir_ambiente(b"PATH=/usr/bin\0HOME=/x\0"), Ok(()));
        assert_eq!(
            conferir_ambiente(b"PATH=/usr/bin\0LD_PRELOAD=/tmp/falso.so\0"),
            Err(Recusa::VariavelLd("LD_PRELOAD"))
        );
        assert_eq!(
            conferir_ambiente(b"LD_AUDIT=x\0"),
            Err(Recusa::VariavelLd("LD_AUDIT"))
        );
        assert_eq!(
            conferir_ambiente(b"A=LD_PRELOAD\0LD_LIBRARY_PATH=\0"),
            Err(Recusa::VariavelLd("LD_LIBRARY_PATH"))
        );
        assert_eq!(
            conferir_ambiente(b"LD_BIND_NOW=1\0"),
            Err(Recusa::VariavelLd("LD_*"))
        );
        // Variável vazia sem `=` também conta pelo nome.
        assert_eq!(
            conferir_ambiente(b"LD_PRELOAD\0"),
            Err(Recusa::VariavelLd("LD_PRELOAD"))
        );
        // O valor que contém "LD_" não é nome.
        assert_eq!(conferir_ambiente(b"X=LD_PRELOAD=1\0"), Ok(()));
    }

    #[test]
    fn tracer_pid_e_ppid_do_status() {
        let s = b"Name:\tbash\nPPid:\t42\nTracerPid:\t0\n";
        assert_eq!(ler_tracer_pid(s), Some(0));
        assert_eq!(campo_status(s, b"PPid:"), Some(42));
        assert_eq!(ler_tracer_pid(b"TracerPid:\t913\n"), Some(913));
        assert_eq!(ler_tracer_pid(b"Name:\tx\n"), None);
        assert_eq!(ler_tracer_pid(b"TracerPid:\tabc\n"), None);
    }

    #[test]
    fn linha_de_mapa() {
        let l = b"7f00-7f10 r-xp 00000000 fd:01 123456                     /usr/lib/x86_64-linux-gnu/libc.so.6";
        let m = ler_linha_mapa(l).unwrap();
        assert!(m.executavel);
        assert_eq!(m.inode, 123456);
        assert_eq!(m.caminho, b"/usr/lib/x86_64-linux-gnu/libc.so.6");
        let anon = ler_linha_mapa(b"7f00-7f10 rw-p 00000000 00:00 0 ").unwrap();
        assert!(!anon.executavel);
        assert_eq!(anon.caminho, b"");
        let espaco = ler_linha_mapa(b"1-2 r-xp 0 00:01 9 /tmp/a b.so").unwrap();
        assert_eq!(espaco.caminho, b"/tmp/a b.so");
        assert!(ler_linha_mapa(b"lixo").is_none());
    }

    #[test]
    fn mapas_recusam_biblioteca_fora_do_root() {
        // memfd e arquivo apagado.
        assert_eq!(
            conferir_mapas_executaveis(b"1-2 r-xp 0 00:01 7 /memfd:x (deleted)\n"),
            Err(Recusa::BibliotecaNaoConfiavel)
        );
        // Arquivo de usuário (o próprio binário de teste, em target/).
        let exe = std::env::current_exe().unwrap();
        let ino = std::fs::metadata(&exe).unwrap().ino();
        let linha = format!("1-2 r-xp 0 00:01 {ino} {}\n", exe.display());
        assert_eq!(
            conferir_mapas_executaveis(linha.as_bytes()),
            Err(Recusa::BibliotecaNaoConfiavel)
        );
        // Linha estragada recusa em vez de pular.
        assert_eq!(
            conferir_mapas_executaveis(b"lixo\n"),
            Err(Recusa::BibliotecaNaoConfiavel)
        );
        // Anônimo e [vdso] passam.
        assert_eq!(
            conferir_mapas_executaveis(b"1-2 r-xp 0 00:00 0 [vdso]\n3-4 rwxp 0 00:00 0 \n"),
            Ok(())
        );
    }

    #[test]
    fn mapas_recusam_inode_diferente_do_caminho() {
        let libc_real = std::fs::read("/proc/self/maps").unwrap();
        // Pega uma biblioteca do root de verdade e troca o inode.
        let texto = String::from_utf8_lossy(&libc_real);
        let linha = texto
            .lines()
            .filter_map(|l| {
                ler_linha_mapa(l.as_bytes()).map(|m| (l.to_owned(), m.inode, m.caminho.to_vec()))
            })
            .find(|(_, _, c)| c.starts_with(b"/usr/lib"))
            .expect("alguma biblioteca do sistema mapeada");
        let caminho = String::from_utf8(linha.2).unwrap();
        let certo = format!("1-2 r-xp 0 00:01 {} {caminho}\n", linha.1);
        assert_eq!(conferir_mapas_executaveis(certo.as_bytes()), Ok(()));
        let errado = format!("1-2 r-xp 0 00:01 {} {caminho}\n", linha.1 + 1);
        assert_eq!(
            conferir_mapas_executaveis(errado.as_bytes()),
            Err(Recusa::BibliotecaNaoConfiavel)
        );
    }

    #[test]
    fn nome_da_cadeia_higienizado() {
        assert_eq!(higienizar_nome(b"\tbash"), "bash");
        assert_eq!(higienizar_nome(b"a\x1b[31mb"), "a?[31mb");
        assert_eq!(higienizar_nome("x\u{202E}y".as_bytes()), "x?y");
        assert_eq!(higienizar_nome(b""), "?");
        assert_eq!(
            higienizar_nome(&[b'a'; 200]).chars().count(),
            MAX_NOME_CADEIA
        );
        assert_eq!(higienizar_nome(&[0xff, b'a']), "\u{FFFD}a");
    }

    #[test]
    fn cadeia_do_proprio_processo_termina_nele() {
        let cadeia = cadeia_de_processos(std::process::id());
        assert!(!cadeia.is_empty() && cadeia.len() <= MAX_CADEIA);
        assert_eq!(cadeia.last().unwrap().pid, std::process::id());
        // Pid inexistente: cadeia vazia, sem pânico.
        assert!(cadeia_de_processos(u32::MAX).is_empty());
        assert!(cadeia_de_processos(0).is_empty());
    }

    #[test]
    fn instalacao_recusa_arquivo_de_usuario_e_link() {
        let exe = std::env::current_exe().unwrap();
        let b = BinarioPermitido::novo(exe).unwrap();
        assert_eq!(
            b.conferir_instalado().unwrap_err(),
            Recusa::InstalacaoNaoConfiavel
        );
        // Inexistente.
        let b = BinarioPermitido::novo("/usr/bin/ward-nao-existe-falso").unwrap();
        assert_eq!(
            b.conferir_instalado().unwrap_err(),
            Recusa::InstalacaoNaoConfiavel
        );
    }

    #[test]
    fn recusa_nao_vaza_detalhe() {
        assert_eq!(
            Recusa::VariavelLd("LD_*").to_string(),
            "variável LD_* no ambiente"
        );
        assert!(!Recusa::ExeNaoPermitido.to_string().contains('/'));
    }
}
