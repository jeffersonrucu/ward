//! Lançador do app (D57; D47, D48; S1, S8): o daemon é o único pai aceito do `ward-app`
//! e o abre por `fork` + `execve` do binário instalado, com um ambiente **montado do
//! zero** ([`ambiente_do_app`]), que não herda nada do daemon (sem `LISTEN_*`) nem do
//! pedido. O app confere o mesmo ambiente antes do GTK ([`conferir_ambiente_do_app`]).
//!
//! O ambiente:
//! - `XDG_RUNTIME_DIR=/run/user/<uid>`: pasta do uid, 0700, sem link no caminho;
//! - `WAYLAND_DISPLAY=wayland-<n>`: nome sem `/` do **único** socket `wayland-<n>` do uid
//!   nessa pasta, sem link. Nenhum ou mais de um: o app não abre (a operação responde
//!   "aprovação indisponível", D21);
//! - `DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/<uid>/bus`, socket do uid, sem link;
//! - `XDG_SESSION_TYPE=wayland`, `GDK_BACKEND=wayland`; sem `DISPLAY`;
//! - `HOME` do `getpwuid_r`, `PATH=/usr/local/bin:/usr/bin:/bin`;
//! - `LANG`/`LC_*` do próprio daemon, só com valor de nome de locale (sem `/`);
//! - as variáveis que desligam fontes de módulo de fora do root, conferidas com o
//!   WebKitGTK real na Fase 29a (D48, handoff 29a):
//!   - `GST_PLUGIN_SYSTEM_PATH` = pasta de plugins do sistema: sem ela, o processo web
//!     do WebKitGTK carrega os plugins de `~/.local/share/gstreamer-1.0/plugins`;
//!   - `NO_AT_BRIDGE=1` e `WEBKIT_A11Y_BUS_ADDRESS=unix:path=/dev/null`: sem elas, o GTK
//!     e o processo web publicam a árvore de acessibilidade (o texto da tela, com o
//!     valor revelado) no barramento AT-SPI da sessão, que qualquer processo do uid lê;
//!   - `CUDA_MPS_PIPE_DIRECTORY=/proc/ward-sem-mps` (D64): sem ela, o cliente MPS da
//!     biblioteca da NVIDIA, no processo web, conecta a `/tmp/nvidia-mps/control`, onde
//!     um processo do mesmo uid pode escutar. Em `/proc` ninguém cria a pasta, nem o
//!     root; o efeito é o de uma máquina sem servidor MPS.
//!
//! Os valores não são segredo; as mensagens de erro levam só o nome da variável.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};

use crate::endurecimento::nome_para_mensagem;
use crate::papel::Instalacao;
use crate::processo::{BinarioPermitido, caminho_absoluto_simples, conferir_arquivo_do_root};
use crate::sys;

/// `PATH` do app.
pub const PATH_DO_APP: &str = "/usr/local/bin:/usr/bin:/bin";

/// Pasta dos plugins do GStreamer do sistema (Ubuntu/Pop!_OS, multiarch).
#[cfg(target_arch = "x86_64")]
pub const PLUGINS_GST_DO_SISTEMA: &str = "/usr/lib/x86_64-linux-gnu/gstreamer-1.0";
#[cfg(target_arch = "aarch64")]
pub const PLUGINS_GST_DO_SISTEMA: &str = "/usr/lib/aarch64-linux-gnu/gstreamer-1.0";

/// Variáveis de valor fixo.
pub const VARIAVEIS_FIXAS: &[(&str, &str)] = &[
    ("XDG_SESSION_TYPE", "wayland"),
    ("GDK_BACKEND", "wayland"),
    ("PATH", PATH_DO_APP),
    ("GST_PLUGIN_SYSTEM_PATH", PLUGINS_GST_DO_SISTEMA),
    ("NO_AT_BRIDGE", "1"),
    ("WEBKIT_A11Y_BUS_ADDRESS", "unix:path=/dev/null"),
    ("CUDA_MPS_PIPE_DIRECTORY", PASTA_MPS_INEXISTENTE),
];

/// Pasta do canal do MPS da NVIDIA (D64): dentro de `/proc`, onde nenhum processo, nem
/// o root, cria pasta. Fixa: o cliente MPS não acha o canal e segue sem MPS.
pub const PASTA_MPS_INEXISTENTE: &str = "/proc/ward-sem-mps";

/// Variáveis de locale aceitas (valor do próprio daemon).
pub const VARIAVEIS_DE_LOCALE: &[&str] = &[
    "LANG",
    "LC_ALL",
    "LC_ADDRESS",
    "LC_COLLATE",
    "LC_CTYPE",
    "LC_IDENTIFICATION",
    "LC_MEASUREMENT",
    "LC_MESSAGES",
    "LC_MONETARY",
    "LC_NAME",
    "LC_NUMERIC",
    "LC_PAPER",
    "LC_TELEPHONE",
    "LC_TIME",
];

/// Maior valor de locale aceito.
const MAX_LOCALE: usize = 64;
/// Maior número de dígitos em `wayland-<n>`.
const MAX_DIGITOS_WAYLAND: usize = 9;

/// Ambiente do app, na ordem dos nomes. Os valores não são segredo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AmbienteApp {
    vars: BTreeMap<&'static str, String>,
}

impl AmbienteApp {
    /// `(nome, valor)`, na ordem dos nomes.
    pub fn variaveis(&self) -> impl Iterator<Item = (&'static str, &str)> {
        self.vars.iter().map(|(k, v)| (*k, v.as_str()))
    }

    pub fn valor(&self, nome: &str) -> Option<&str> {
        self.vars.get(nome).map(String::as_str)
    }
}

/// Sessão ou ambiente fora de D57. Sem valor: só o nome da variável (higienizado
/// quando vem de fora).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErroSessao {
    /// `/run/user/<uid>` não é pasta do uid, 0700, sem link, com as de cima sem escrita
    /// de outros.
    RuntimeNaoConfiavel,
    /// Nenhum socket `wayland-<n>` do uid.
    SemWayland,
    /// Mais de um `wayland-<n>`: não dá para saber qual é o do compositor.
    VariosWayland,
    /// Um `wayland-<n>` que é link, não é socket ou não é do uid.
    WaylandNaoConfiavel,
    /// `/run/user/<uid>/bus` ausente, link, não socket ou de outro dono.
    BarramentoNaoConfiavel,
    /// Sem diretório do usuário no `getpwuid_r`, ou fora da forma de caminho absoluto.
    HomeIndisponivel,
    /// Falta uma variável obrigatória.
    Faltando(&'static str),
    /// Uma variável que não é de D57.
    AMais(String),
    /// Uma variável de D57 com outro valor (ou com valor de locale inválido).
    Diferente(&'static str),
    /// A mesma variável duas vezes no ambiente.
    Repetida(String),
}

impl ErroSessao {
    /// O lançador não abre o app por falta de sessão Wayland única e confiável: a
    /// operação `abrir_app` (Fase 23) responde "aprovação indisponível" (D21).
    pub fn sessao_indisponivel(&self) -> bool {
        !matches!(
            self,
            ErroSessao::Faltando(_)
                | ErroSessao::AMais(_)
                | ErroSessao::Diferente(_)
                | ErroSessao::Repetida(_)
        )
    }
}

impl fmt::Display for ErroSessao {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErroSessao::RuntimeNaoConfiavel => {
                f.write_str("a pasta da sessão (/run/user/<uid>) não é 0700 do usuário")
            }
            ErroSessao::SemWayland => f.write_str("nenhum socket Wayland da sessão"),
            ErroSessao::VariosWayland => {
                f.write_str("mais de um socket Wayland na sessão; o app não escolhe")
            }
            ErroSessao::WaylandNaoConfiavel => {
                f.write_str("socket Wayland da sessão é link, não é socket ou é de outro dono")
            }
            ErroSessao::BarramentoNaoConfiavel => {
                f.write_str("barramento da sessão ausente, link ou de outro dono")
            }
            ErroSessao::HomeIndisponivel => f.write_str("diretório do usuário indisponível"),
            ErroSessao::Faltando(n) => write!(f, "variável {n} ausente no ambiente do app"),
            ErroSessao::AMais(n) => write!(f, "variável {n} não é do ambiente do app"),
            ErroSessao::Diferente(n) => write!(f, "variável {n} com valor fora do esperado"),
            ErroSessao::Repetida(n) => write!(f, "variável {n} repetida no ambiente"),
        }
    }
}

impl std::error::Error for ErroSessao {}

/// `/run/user/<uid>`.
fn pasta_da_sessao(uid: u32) -> PathBuf {
    PathBuf::from(format!("/run/user/{uid}"))
}

/// Ambiente do app para o uid deste processo (o daemon): sessão conferida no disco,
/// `HOME` do `getpwuid_r` e o locale do próprio processo (valores inválidos ficam de
/// fora). Nada mais do ambiente deste processo entra.
pub fn ambiente_do_app() -> Result<AmbienteApp, ErroSessao> {
    let uid = sys::geteuid();
    let home = sys::home_do_uid(uid).map_err(|_| ErroSessao::HomeIndisponivel)?;
    let locale = VARIAVEIS_DE_LOCALE
        .iter()
        .filter_map(|&nome| {
            let v = std::env::var_os(nome)?;
            locale_valido(v.as_bytes()).then(|| (nome, v.to_string_lossy().into_owned()))
        })
        .collect();
    montar(&pasta_da_sessao(uid), uid, &home, locale)
}

/// Monta o ambiente a partir da pasta da sessão `rt`.
fn montar(
    rt: &Path,
    uid: u32,
    home: &OsStr,
    locale: Vec<(&'static str, String)>,
) -> Result<AmbienteApp, ErroSessao> {
    conferir_pasta_da_sessao(rt, uid)?;
    let wayland = socket_wayland_unico(rt, uid)?;
    let barramento = rt.join("bus");
    if !socket_do_uid(&barramento, uid) {
        return Err(ErroSessao::BarramentoNaoConfiavel);
    }
    let home = home.to_str().ok_or(ErroSessao::HomeIndisponivel)?;
    if !caminho_absoluto_simples(Path::new(home)) {
        return Err(ErroSessao::HomeIndisponivel);
    }
    let rt_texto = rt.to_str().ok_or(ErroSessao::RuntimeNaoConfiavel)?;
    let mut vars: BTreeMap<&'static str, String> = BTreeMap::new();
    vars.insert("XDG_RUNTIME_DIR", rt_texto.to_owned());
    vars.insert("WAYLAND_DISPLAY", wayland);
    vars.insert(
        "DBUS_SESSION_BUS_ADDRESS",
        format!("unix:path={rt_texto}/bus"),
    );
    vars.insert("HOME", home.to_owned());
    for &(nome, valor) in VARIAVEIS_FIXAS {
        vars.insert(nome, valor.to_owned());
    }
    for (nome, valor) in locale {
        vars.insert(nome, valor);
    }
    Ok(AmbienteApp { vars })
}

/// Valor de locale: 1 a 64 bytes de `[A-Za-z0-9_.@-]`, sem começar por `.` nem ter
/// `..` (nunca um caminho: a glibc carregaria o locale de qualquer pasta).
fn locale_valido(v: &[u8]) -> bool {
    !v.is_empty()
        && v.len() <= MAX_LOCALE
        && v[0] != b'.'
        && !v.windows(2).any(|w| w == b"..")
        && v.iter()
            .all(|&b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'@' | b'-'))
}

/// A pasta é do uid, 0700 e não é link; as de cima não são link, são do root ou do
/// uid e sem escrita para grupo e outros.
fn conferir_pasta_da_sessao(rt: &Path, uid: u32) -> Result<(), ErroSessao> {
    let ruim = ErroSessao::RuntimeNaoConfiavel;
    if !caminho_absoluto_simples(rt) {
        return Err(ruim);
    }
    let m = std::fs::symlink_metadata(rt).map_err(|_| ErroSessao::RuntimeNaoConfiavel)?;
    if !m.file_type().is_dir() || m.uid() != uid || m.mode() & 0o777 != 0o700 {
        return Err(ruim);
    }
    let mut atual = rt.parent();
    while let Some(p) = atual {
        let m = std::fs::symlink_metadata(p).map_err(|_| ErroSessao::RuntimeNaoConfiavel)?;
        if !m.file_type().is_dir() || (m.uid() != 0 && m.uid() != uid) || m.mode() & 0o022 != 0 {
            return Err(ErroSessao::RuntimeNaoConfiavel);
        }
        atual = p.parent();
    }
    Ok(())
}

/// O caminho é um socket do uid (sem seguir link).
fn socket_do_uid(caminho: &Path, uid: u32) -> bool {
    std::fs::symlink_metadata(caminho).is_ok_and(|m| m.file_type().is_socket() && m.uid() == uid)
}

/// `wayland-<n>` (só dígitos, até 9).
fn nome_wayland(nome: &[u8]) -> bool {
    nome.strip_prefix(b"wayland-").is_some_and(|n| {
        !n.is_empty() && n.len() <= MAX_DIGITOS_WAYLAND && n.iter().all(u8::is_ascii_digit)
    })
}

/// O nome do único socket `wayland-<n>` do uid em `rt`. Um link, um arquivo que não é
/// socket ou de outro dono com esse nome também recusa (falha fechada).
fn socket_wayland_unico(rt: &Path, uid: u32) -> Result<String, ErroSessao> {
    let mut achado: Option<String> = None;
    let dir = std::fs::read_dir(rt).map_err(|_| ErroSessao::RuntimeNaoConfiavel)?;
    for item in dir {
        let item = item.map_err(|_| ErroSessao::RuntimeNaoConfiavel)?;
        let nome = item.file_name();
        if !nome_wayland(nome.as_bytes()) {
            continue;
        }
        if !socket_do_uid(&item.path(), uid) {
            return Err(ErroSessao::WaylandNaoConfiavel);
        }
        if achado.is_some() {
            return Err(ErroSessao::VariosWayland);
        }
        achado = Some(nome.to_string_lossy().into_owned());
    }
    achado.ok_or(ErroSessao::SemWayland)
}

/// Lado do app (D57): confere, antes do GTK, que o ambiente recebido tem exatamente as
/// variáveis de D57, com os valores que o daemon montaria agora (pasta da sessão, o
/// único socket Wayland, barramento, `HOME` do `getpwuid_r`, valores fixos) e só locale
/// válido. Qualquer variável a mais, a menos, repetida ou diferente recusa.
pub fn conferir_ambiente_do_app<I>(recebido: I) -> Result<AmbienteApp, ErroSessao>
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    let uid = sys::geteuid();
    let home = sys::home_do_uid(uid).map_err(|_| ErroSessao::HomeIndisponivel)?;
    conferir_em(recebido, &pasta_da_sessao(uid), uid, &home)
}

fn nome_conhecido(nome: &[u8]) -> Option<&'static str> {
    const DA_SESSAO: &[&str] = &[
        "XDG_RUNTIME_DIR",
        "WAYLAND_DISPLAY",
        "DBUS_SESSION_BUS_ADDRESS",
        "HOME",
    ];
    DA_SESSAO
        .iter()
        .copied()
        .chain(VARIAVEIS_FIXAS.iter().map(|(n, _)| *n))
        .chain(VARIAVEIS_DE_LOCALE.iter().copied())
        .find(|n| n.as_bytes() == nome)
}

fn conferir_em<I>(recebido: I, rt: &Path, uid: u32, home: &OsStr) -> Result<AmbienteApp, ErroSessao>
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    let mut vistos: BTreeMap<&'static str, OsString> = BTreeMap::new();
    for (nome, valor) in recebido {
        let Some(conhecido) = nome_conhecido(nome.as_bytes()) else {
            return Err(ErroSessao::AMais(nome_para_mensagem(nome.as_bytes())));
        };
        if vistos.insert(conhecido, valor).is_some() {
            return Err(ErroSessao::Repetida(conhecido.to_owned()));
        }
    }
    let mut locale = Vec::new();
    for &nome in VARIAVEIS_DE_LOCALE {
        if let Some(v) = vistos.get(nome) {
            if !locale_valido(v.as_bytes()) {
                return Err(ErroSessao::Diferente(nome));
            }
            locale.push((nome, v.to_string_lossy().into_owned()));
        }
    }
    let esperado = montar(rt, uid, home, locale)?;
    for (nome, valor) in esperado.variaveis() {
        match vistos.get(nome) {
            None => return Err(ErroSessao::Faltando(nome)),
            Some(v) if v.as_bytes() != valor.as_bytes() => {
                return Err(ErroSessao::Diferente(nome));
            }
            Some(_) => {}
        }
    }
    // Todo nome visto é conhecido; um de locale que não está no esperado já recusou
    // acima, e os outros são obrigatórios. Mesmo assim, contagem igual.
    if vistos.len() != esperado.vars.len() {
        return Err(ErroSessao::AMais("?".to_owned()));
    }
    Ok(esperado)
}

/// Por que o lançador não abriu o app. Sem caminho nem valor: pode ir para o log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErroLancador {
    /// Já há uma instância aberta por este lançador (uma por vez, D57).
    JaAberto,
    /// Sessão fora de D57 ([`ErroSessao::sessao_indisponivel`]: "aprovação
    /// indisponível", D21).
    Sessao(ErroSessao),
    /// O binário não é do root, não é setgid do grupo `ward-app`, tem escrita de grupo
    /// ou outros, ou há link no caminho.
    BinarioNaoConfiavel,
    /// `fork`/`execve` falhou.
    Io(io::ErrorKind),
}

impl fmt::Display for ErroLancador {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErroLancador::JaAberto => f.write_str("o app já está aberto"),
            ErroLancador::Sessao(e) => write!(f, "aprovação indisponível: {e}"),
            ErroLancador::BinarioNaoConfiavel => {
                f.write_str("o ward-app instalado não é do root com o grupo ward-app (setgid)")
            }
            ErroLancador::Io(k) => write!(f, "não deu para abrir o app ({k})"),
        }
    }
}

impl std::error::Error for ErroLancador {}

/// Abre o `ward-app` instalado como filho deste processo (o daemon). Guarda o filho:
/// uma instância por vez.
#[derive(Debug)]
pub struct Lancador {
    origem: Origem,
    filho: Option<Child>,
    /// pidfd do app aberto (D63), tirado logo depois do `fork`, com o filho ainda sem
    /// colher (o pid não pode ser de outro processo). O papel app exige este processo.
    pidfd: Option<OwnedFd>,
    /// Como saiu o último app colhido, até alguém tomar ([`Lancador::tomar_saida`]).
    saida: Option<ExitStatus>,
}

/// O que o lançador abre.
#[derive(Debug)]
enum Origem {
    /// O `ward-app` instalado: do root, setgid do grupo `ward-app`, ambiente de D57.
    Instalado {
        binario: BinarioPermitido,
        gid_app: u32,
    },
    /// Só no `modo-teste` (D63): o `WARD_DEV_APP` do daemon de teste.
    #[cfg(feature = "modo-teste")]
    Teste(BinarioDeTeste),
}

impl Lancador {
    pub fn novo(binario: BinarioPermitido, inst: &Instalacao) -> Lancador {
        Lancador::de(Origem::Instalado {
            binario,
            gid_app: inst.gid_app(),
        })
    }

    /// Só no `modo-teste` (D63): abre o binário de teste (o `ward-app` de teste ou a
    /// ponte), conferido de novo a cada abertura, com a saída no log de teste.
    #[cfg(feature = "modo-teste")]
    pub fn de_teste(binario: BinarioDeTeste) -> Lancador {
        Lancador::de(Origem::Teste(binario))
    }

    fn de(origem: Origem) -> Lancador {
        Lancador {
            origem,
            filho: None,
            pidfd: None,
            saida: None,
        }
    }

    /// O app aberto por este lançador ainda está vivo (e colhe o que saiu, guardando
    /// como saiu para [`Lancador::tomar_saida`]).
    pub fn aberto(&mut self) -> bool {
        match self.filho.as_mut().map(Child::try_wait) {
            Some(Ok(None)) => true,
            Some(r) => {
                if let Ok(Some(status)) = r {
                    self.saida = Some(status);
                }
                self.filho = None;
                self.pidfd = None;
                false
            }
            None => false,
        }
    }

    /// O processo `pid` é o app que este lançador abriu e que continua vivo (D63)? Sem
    /// colher: o pid do filho não colhido não vai para outro processo, e o pidfd
    /// confere que ele não saiu.
    pub fn e_o_app_lancado(&self, pid: u32) -> bool {
        match (&self.filho, &self.pidfd) {
            (Some(f), Some(fd)) => {
                f.id() == pid && pid != 0 && sys::pidfd_vivo(fd.as_fd()).unwrap_or(false)
            }
            _ => false,
        }
    }

    /// Pid do app aberto por este lançador (sem colher).
    pub fn pid(&self) -> Option<u32> {
        self.filho.as_ref().map(Child::id)
    }

    /// Como saiu o último app colhido (código ou sinal), uma vez só. O app liga 0–2 a
    /// `/dev/null` (D57): o daemon registra isto no log para saber por que não abriu.
    pub fn tomar_saida(&mut self) -> Option<ExitStatus> {
        self.aberto();
        self.saida.take()
    }

    /// Confere o binário e a sessão, monta o ambiente do zero e abre o app. Devolve o
    /// pid do app.
    pub fn abrir(&mut self) -> Result<u32, ErroLancador> {
        if self.aberto() {
            return Err(ErroLancador::JaAberto);
        }
        let mut filho = match &self.origem {
            Origem::Instalado { binario, gid_app } => {
                conferir_binario_do_app(binario, *gid_app)?;
                let ambiente = ambiente_do_app().map_err(ErroLancador::Sessao)?;
                lancar(binario.caminho(), &ambiente).map_err(|e| ErroLancador::Io(e.kind()))?
            }
            #[cfg(feature = "modo-teste")]
            Origem::Teste(t) => t.lancar()?,
        };
        let pid = filho.id();
        // O filho ainda não foi colhido: o pid é dele. Sem pidfd, o app não abre (D63).
        let pidfd = match libc::pid_t::try_from(pid)
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))
            .and_then(sys::pidfd_open)
        {
            Ok(fd) => fd,
            Err(e) => {
                let _ = filho.kill();
                let _ = filho.wait();
                return Err(ErroLancador::Io(e.kind()));
            }
        };
        self.filho = Some(filho);
        self.pidfd = Some(pidfd);
        Ok(pid)
    }
}

/// Binário do root, sem escrita de grupo/outros (o caminho todo), setgid do grupo do
/// app.
fn conferir_binario_do_app(binario: &BinarioPermitido, gid_app: u32) -> Result<(), ErroLancador> {
    let meta = conferir_arquivo_do_root(binario.caminho())
        .map_err(|()| ErroLancador::BinarioNaoConfiavel)?;
    if meta.mode() & 0o2000 == 0 || meta.gid() != gid_app {
        return Err(ErroLancador::BinarioNaoConfiavel);
    }
    Ok(())
}

/// `fork` + `execve(caminho)` sem shell, com só `ambiente`, pasta `/`, 0–2 em
/// `/dev/null` e todo fd ≥ 3 fechado no `exec`.
fn lancar(caminho: &Path, ambiente: &AmbienteApp) -> io::Result<Child> {
    let mut cmd = Command::new(caminho);
    cmd.env_clear()
        .envs(ambiente.variaveis())
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    sys::fechar_fds_no_exec(&mut cmd);
    cmd.spawn()
}

/// Só no `modo-teste` (contêiner, Fase 29a): o mesmo `fork` + `execve` com saída
/// redirecionada, para o teste ler o que o app de teste imprime. Não confere o binário.
#[cfg(feature = "modo-teste")]
pub fn lancar_para_teste(
    caminho: &Path,
    ambiente: &AmbienteApp,
    saida: std::fs::File,
) -> io::Result<Child> {
    let mut cmd = Command::new(caminho);
    cmd.env_clear()
        .envs(ambiente.variaveis())
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(saida.try_clone()?)
        .stderr(saida);
    sys::fechar_fds_no_exec(&mut cmd);
    cmd.spawn()
}

/// Nome do app Tauri de teste: com ele, o lançador de teste monta o ambiente de D57.
#[cfg(feature = "modo-teste")]
pub const NOME_APP_DE_TESTE: &str = "ward-app";

/// `PATH` do binário de teste que não é o app Tauri (a ponte).
#[cfg(feature = "modo-teste")]
pub const PATH_DE_TESTE: &str = "/usr/bin:/bin";

/// Por que o binário de teste foi recusado. Sem caminho (vai ao log).
#[cfg(feature = "modo-teste")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErroBinarioDeTeste {
    /// Caminho relativo, com `.`, `..`, `//` ou nome fora de `[A-Za-z0-9._-]`.
    Caminho,
    /// Fora do repositório (a pasta acima de `WARD_DEV_DIR`).
    ForaDoRepositorio,
    /// Link simbólico no caminho, ou algo que não é pasta no meio.
    Link,
    /// Não é arquivo regular executável.
    NaoArquivo,
    /// O arquivo não é do uid deste processo.
    OutroDono,
    /// Arquivo ou pasta do caminho com escrita para outros.
    Gravavel,
}

#[cfg(feature = "modo-teste")]
impl fmt::Display for ErroBinarioDeTeste {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.mensagem())
    }
}

#[cfg(feature = "modo-teste")]
impl ErroBinarioDeTeste {
    /// A mensagem, sem caminho.
    pub fn mensagem(&self) -> &'static str {
        match self {
            ErroBinarioDeTeste::Caminho => "caminho do app de teste fora da forma absoluta simples",
            ErroBinarioDeTeste::ForaDoRepositorio => {
                "o app de teste não está dentro do repositório"
            }
            ErroBinarioDeTeste::Link => "link simbólico no caminho do app de teste",
            ErroBinarioDeTeste::NaoArquivo => "o app de teste não é arquivo regular executável",
            ErroBinarioDeTeste::OutroDono => "o app de teste não é do usuário do daemon",
            ErroBinarioDeTeste::Gravavel => {
                "o app de teste (ou uma pasta dele) tem escrita para outros"
            }
        }
    }
}

/// Só no `modo-teste` (D63): o binário que o daemon de teste abre pelo `abrir_app`
/// (`WARD_DEV_APP`: o `ward-app` de teste ou a `ward-ponte-teste`). Nada dele vem do
/// pedido: caminho, ambiente e saída são da configuração do daemon.
///
/// - caminho absoluto simples, dentro de `raiz` (o repositório), arquivo regular
///   executável do uid deste processo, sem escrita para outros, sem link em nenhuma
///   parte do caminho (conferido ao criar e de novo a cada abertura);
/// - o `ward-app` de teste recebe o ambiente de D57 ([`ambiente_do_app`]); qualquer
///   outro (a ponte, que não abre janela) recebe o ambiente fixo mínimo: `HOME` do
///   `getpwuid_r`, `PATH=/usr/bin:/bin`, `LANG` do daemon (só nome de locale),
///   `XDG_RUNTIME_DIR` conferido e as variáveis do `modo-teste` que o daemon passa;
/// - a saída vai para `<logs>/<nome do binário>.log`, 0600, sem seguir link.
#[cfg(feature = "modo-teste")]
#[derive(Clone, Debug)]
pub struct BinarioDeTeste {
    caminho: PathBuf,
    raiz: PathBuf,
    logs: PathBuf,
    extras: Vec<(&'static str, String)>,
}

#[cfg(feature = "modo-teste")]
impl BinarioDeTeste {
    /// `extras`: as variáveis do `modo-teste` que vão ao ambiente mínimo (ex.:
    /// `WARD_DEV_DIR`). Não entram no ambiente do `ward-app` de teste (D57 exato).
    pub fn novo(
        caminho: PathBuf,
        raiz: PathBuf,
        logs: PathBuf,
        extras: Vec<(&'static str, String)>,
    ) -> Result<BinarioDeTeste, ErroBinarioDeTeste> {
        let b = BinarioDeTeste {
            caminho,
            raiz,
            logs,
            extras,
        };
        b.conferir()?;
        Ok(b)
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    fn nome(&self) -> Result<&str, ErroBinarioDeTeste> {
        let nome = self
            .caminho
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or(ErroBinarioDeTeste::Caminho)?;
        let ok = !nome.is_empty()
            && nome.len() <= 64
            && !nome.starts_with(['.', '-'])
            && nome
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
        if ok {
            Ok(nome)
        } else {
            Err(ErroBinarioDeTeste::Caminho)
        }
    }

    /// As conferências do binário (ver o tipo).
    pub fn conferir(&self) -> Result<(), ErroBinarioDeTeste> {
        let uid = sys::geteuid();
        if !caminho_absoluto_simples(&self.caminho) || !caminho_absoluto_simples(&self.raiz) {
            return Err(ErroBinarioDeTeste::Caminho);
        }
        self.nome()?;
        if self.caminho == self.raiz || !self.caminho.starts_with(&self.raiz) {
            return Err(ErroBinarioDeTeste::ForaDoRepositorio);
        }
        let m =
            std::fs::symlink_metadata(&self.caminho).map_err(|_| ErroBinarioDeTeste::NaoArquivo)?;
        if m.file_type().is_symlink() {
            return Err(ErroBinarioDeTeste::Link);
        }
        if !m.file_type().is_file() || m.mode() & 0o100 == 0 {
            return Err(ErroBinarioDeTeste::NaoArquivo);
        }
        if m.uid() != uid {
            return Err(ErroBinarioDeTeste::OutroDono);
        }
        if m.mode() & 0o002 != 0 {
            return Err(ErroBinarioDeTeste::Gravavel);
        }
        let mut atual = self.caminho.parent();
        while let Some(p) = atual {
            let m = std::fs::symlink_metadata(p).map_err(|_| ErroBinarioDeTeste::Link)?;
            if !m.file_type().is_dir() {
                return Err(ErroBinarioDeTeste::Link);
            }
            if m.mode() & 0o002 != 0 {
                return Err(ErroBinarioDeTeste::Gravavel);
            }
            atual = p.parent();
        }
        Ok(())
    }

    /// O ambiente do binário de teste (ver o tipo). Nada do ambiente deste processo
    /// além do locale.
    pub fn ambiente(&self) -> Result<AmbienteApp, ErroSessao> {
        if self.nome() == Ok(NOME_APP_DE_TESTE) {
            return ambiente_do_app();
        }
        let uid = sys::geteuid();
        let rt = pasta_da_sessao(uid);
        conferir_pasta_da_sessao(&rt, uid)?;
        let home = sys::home_do_uid(uid).map_err(|_| ErroSessao::HomeIndisponivel)?;
        let home = home.to_str().ok_or(ErroSessao::HomeIndisponivel)?;
        if !caminho_absoluto_simples(Path::new(home)) {
            return Err(ErroSessao::HomeIndisponivel);
        }
        let mut vars: BTreeMap<&'static str, String> = BTreeMap::new();
        vars.insert("HOME", home.to_owned());
        vars.insert("PATH", PATH_DE_TESTE.to_owned());
        vars.insert(
            "XDG_RUNTIME_DIR",
            rt.to_str()
                .ok_or(ErroSessao::RuntimeNaoConfiavel)?
                .to_owned(),
        );
        if let Some(v) = std::env::var_os("LANG")
            && locale_valido(v.as_bytes())
        {
            vars.insert("LANG", v.to_string_lossy().into_owned());
        }
        for (k, v) in &self.extras {
            vars.insert(k, v.clone());
        }
        Ok(AmbienteApp { vars })
    }

    /// `<logs>/<nome>.log`, 0600, do uid, sem seguir link; a pasta é criada 0700.
    fn abrir_log(&self) -> io::Result<std::fs::File> {
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
        let uid = sys::geteuid();
        let nome = self
            .nome()
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        match std::fs::DirBuilder::new().mode(0o700).create(&self.logs) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        let m = std::fs::symlink_metadata(&self.logs)?;
        if !m.file_type().is_dir() || m.uid() != uid {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        let f = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(self.logs.join(format!("{nome}.log")))?;
        let m = f.metadata()?;
        if !m.file_type().is_file() || m.uid() != uid {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        Ok(f)
    }

    /// Confere de novo e abre, com a saída no log.
    fn lancar(&self) -> Result<Child, ErroLancador> {
        self.conferir()
            .map_err(|_| ErroLancador::BinarioNaoConfiavel)?;
        let ambiente = self.ambiente().map_err(ErroLancador::Sessao)?;
        let log = self.abrir_log().map_err(|e| ErroLancador::Io(e.kind()))?;
        lancar_para_teste(&self.caminho, &ambiente, log).map_err(|e| ErroLancador::Io(e.kind()))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
    use std::os::unix::net::UnixListener;

    /// Pasta de sessão falsa em `/run/user/<uid>/` (0700), apagada ao sair.
    struct Sessao {
        rt: PathBuf,
        _escutas: Vec<UnixListener>,
    }

    impl Sessao {
        fn nova(nome: &str) -> Sessao {
            let uid = sys::geteuid();
            let rt = PathBuf::from(format!(
                "/run/user/{uid}/ward-lancador-teste-{}-{nome}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&rt);
            fs::DirBuilder::new().mode(0o700).create(&rt).unwrap();
            let mut s = Sessao {
                rt,
                _escutas: Vec::new(),
            };
            s.socket("wayland-1");
            s.socket("bus");
            fs::write(s.rt.join("wayland-1.lock"), b"").unwrap();
            s
        }

        fn socket(&mut self, nome: &str) {
            self._escutas
                .push(UnixListener::bind(self.rt.join(nome)).unwrap());
        }

        fn montar(&self) -> Result<AmbienteApp, ErroSessao> {
            montar(
                &self.rt,
                sys::geteuid(),
                OsStr::new("/home/teste"),
                vec![("LANG", "pt_BR.UTF-8".to_owned())],
            )
        }

        fn conferir(&self, vars: &[(&str, &str)]) -> Result<AmbienteApp, ErroSessao> {
            conferir_em(
                vars.iter()
                    .map(|(k, v)| (OsString::from(k), OsString::from(v))),
                &self.rt,
                sys::geteuid(),
                OsStr::new("/home/teste"),
            )
        }

        /// O ambiente que o daemon montaria, como pares.
        fn certo(&self) -> Vec<(String, String)> {
            self.montar()
                .unwrap()
                .variaveis()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect()
        }
    }

    impl Drop for Sessao {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.rt);
        }
    }

    fn pares(v: &[(String, String)]) -> Vec<(&str, &str)> {
        v.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect()
    }

    #[test]
    fn ambiente_montado_tem_so_as_variaveis_de_d57() {
        let s = Sessao::nova("montar");
        let a = s.montar().unwrap();
        let rt = s.rt.to_str().unwrap();
        assert_eq!(a.valor("XDG_RUNTIME_DIR"), Some(rt));
        assert_eq!(a.valor("WAYLAND_DISPLAY"), Some("wayland-1"));
        assert_eq!(
            a.valor("DBUS_SESSION_BUS_ADDRESS"),
            Some(format!("unix:path={rt}/bus").as_str())
        );
        assert_eq!(a.valor("HOME"), Some("/home/teste"));
        assert_eq!(a.valor("PATH"), Some(PATH_DO_APP));
        assert_eq!(a.valor("GDK_BACKEND"), Some("wayland"));
        assert_eq!(
            a.valor("GST_PLUGIN_SYSTEM_PATH"),
            Some(PLUGINS_GST_DO_SISTEMA)
        );
        assert_eq!(a.valor("NO_AT_BRIDGE"), Some("1"));
        assert_eq!(
            a.valor("WEBKIT_A11Y_BUS_ADDRESS"),
            Some("unix:path=/dev/null")
        );
        assert_eq!(
            a.valor("CUDA_MPS_PIPE_DIRECTORY"),
            Some("/proc/ward-sem-mps")
        );
        assert_eq!(a.valor("LANG"), Some("pt_BR.UTF-8"));
        for proibida in [
            "DISPLAY",
            "LISTEN_PID",
            "LISTEN_FDS",
            "GTK_MODULES",
            "LD_PRELOAD",
        ] {
            assert_eq!(a.valor(proibida), None, "{proibida}");
        }
        assert_eq!(a.variaveis().count(), 4 + VARIAVEIS_FIXAS.len() + 1);
        // O que o daemon monta passa na conferência do app.
        assert_eq!(s.conferir(&pares(&s.certo())), Ok(a));
    }

    #[test]
    fn ataque_dois_sockets_wayland_ou_link_nao_abre() {
        // V5.31: dois `wayland-<n>`.
        let mut s = Sessao::nova("dois");
        s.socket("wayland-0");
        assert_eq!(s.montar(), Err(ErroSessao::VariosWayland));
        assert!(ErroSessao::VariosWayland.sessao_indisponivel());
        // Um link para um socket de outro lugar (o proxy do atacante).
        let s = Sessao::nova("link");
        fs::remove_file(s.rt.join("wayland-1")).unwrap();
        let mut proxy = Sessao::nova("proxy");
        proxy.socket("proxy.sock");
        symlink(proxy.rt.join("proxy.sock"), s.rt.join("wayland-1")).unwrap();
        assert_eq!(s.montar(), Err(ErroSessao::WaylandNaoConfiavel));
        // Arquivo comum com o nome.
        let s = Sessao::nova("arquivo");
        fs::write(s.rt.join("wayland-7"), b"").unwrap();
        assert_eq!(s.montar(), Err(ErroSessao::WaylandNaoConfiavel));
        // Nenhum.
        let s = Sessao::nova("nenhum");
        fs::remove_file(s.rt.join("wayland-1")).unwrap();
        assert_eq!(s.montar(), Err(ErroSessao::SemWayland));
        // `wayland-1.lock` e nomes parecidos não contam.
        let mut s = Sessao::nova("parecidos");
        s.socket("wayland-x");
        s.socket("wayland-");
        s.socket("xwayland-2");
        assert!(s.montar().is_ok());
    }

    #[test]
    fn ataque_barramento_ou_pasta_da_sessao_trocados() {
        let s = Sessao::nova("bus");
        fs::remove_file(s.rt.join("bus")).unwrap();
        assert_eq!(s.montar(), Err(ErroSessao::BarramentoNaoConfiavel));
        let mut proxy = Sessao::nova("bus-proxy");
        proxy.socket("x");
        symlink(proxy.rt.join("x"), s.rt.join("bus")).unwrap();
        assert_eq!(s.montar(), Err(ErroSessao::BarramentoNaoConfiavel));
        // Pasta aberta para outros, ou por link.
        let s = Sessao::nova("aberta");
        fs::set_permissions(&s.rt, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(s.montar(), Err(ErroSessao::RuntimeNaoConfiavel));
        let s = Sessao::nova("alvo");
        let link = s.rt.with_file_name(format!(
            "ward-lancador-teste-{}-link-rt",
            std::process::id()
        ));
        let _ = fs::remove_file(&link);
        symlink(&s.rt, &link).unwrap();
        let r = montar(&link, sys::geteuid(), OsStr::new("/home/x"), vec![]);
        fs::remove_file(&link).unwrap();
        assert_eq!(r, Err(ErroSessao::RuntimeNaoConfiavel));
        // `/tmp` (1777) também não é pasta de sessão.
        assert_eq!(
            montar(Path::new("/tmp"), sys::geteuid(), OsStr::new("/h"), vec![]),
            Err(ErroSessao::RuntimeNaoConfiavel)
        );
    }

    #[test]
    fn ataque_ambiente_forcado_no_app_e_recusado() {
        // V5.31: o app aberto pelo daemon, mas com variáveis postas à força.
        let s = Sessao::nova("conferir");
        let certo = s.certo();
        let trocar = |nome: &str, valor: &str| {
            let mut v = certo.clone();
            for (k, val) in &mut v {
                if k == nome {
                    *val = valor.to_owned();
                }
            }
            v
        };
        for (nome, valor) in [
            ("WAYLAND_DISPLAY", "/tmp/proxy.sock"),
            ("WAYLAND_DISPLAY", "../x"),
            ("WAYLAND_DISPLAY", "wayland-0"),
            ("DBUS_SESSION_BUS_ADDRESS", "unix:path=/tmp/bus"),
            ("XDG_RUNTIME_DIR", "/tmp/x"),
            ("HOME", "/tmp/casa-falsa"),
            ("PATH", "/tmp/bin:/usr/bin"),
            ("GST_PLUGIN_SYSTEM_PATH", "/home/teste/.local/gst"),
            ("NO_AT_BRIDGE", "0"),
            // D64: o canal do MPS levado de volta a `/tmp`, onde o atacante escuta.
            ("CUDA_MPS_PIPE_DIRECTORY", "/tmp/nvidia-mps"),
            ("CUDA_MPS_PIPE_DIRECTORY", "/run/user/1000/mps"),
            ("CUDA_MPS_PIPE_DIRECTORY", ""),
            ("LANG", "../../tmp/locale"),
            ("LANG", "/tmp/locale"),
        ] {
            let v = trocar(nome, valor);
            assert!(
                matches!(s.conferir(&pares(&v)), Err(ErroSessao::Diferente(_))),
                "{nome}={valor}"
            );
        }
        // Variáveis a mais que o WebKitGTK, o GTK, o GIO e o GStreamer leriam.
        for extra in [
            "GTK_MODULES",
            "GTK3_MODULES",
            "GTK_PATH",
            "GST_PLUGIN_PATH",
            "GIO_EXTRA_MODULES",
            "GIO_MODULE_DIR",
            "GDK_PIXBUF_MODULE_FILE",
            "WEBKIT_INJECTED_BUNDLE_PATH",
            "XDG_DATA_DIRS",
            "XDG_CONFIG_HOME",
            "DISPLAY",
            "LISTEN_PID",
            "LISTEN_FDS",
            "LD_PRELOAD",
        ] {
            let mut v = certo.clone();
            v.push((extra.to_owned(), "/tmp/x".to_owned()));
            assert_eq!(
                s.conferir(&pares(&v)),
                Err(ErroSessao::AMais(extra.to_owned())),
                "{extra}"
            );
        }
        // Faltando uma, ou a mesma duas vezes (a primeira certa).
        let mut v = certo.clone();
        v.retain(|(k, _)| k != "NO_AT_BRIDGE");
        assert_eq!(
            s.conferir(&pares(&v)),
            Err(ErroSessao::Faltando("NO_AT_BRIDGE"))
        );
        // D64: sem a variável do MPS, o cliente da NVIDIA volta ao `/tmp/nvidia-mps`.
        let mut v = certo.clone();
        v.retain(|(k, _)| k != "CUDA_MPS_PIPE_DIRECTORY");
        assert_eq!(
            s.conferir(&pares(&v)),
            Err(ErroSessao::Faltando("CUDA_MPS_PIPE_DIRECTORY"))
        );
        let mut v = certo.clone();
        v.push(("WAYLAND_DISPLAY".to_owned(), "/tmp/proxy.sock".to_owned()));
        assert_eq!(
            s.conferir(&pares(&v)),
            Err(ErroSessao::Repetida("WAYLAND_DISPLAY".to_owned()))
        );
        // Locale a mais, válido, entra; o nome com controle sai higienizado.
        let mut v = certo.clone();
        v.push(("LC_TIME".to_owned(), "C.UTF-8".to_owned()));
        assert!(s.conferir(&pares(&v)).is_ok());
        let r = conferir_em(
            [(OsString::from("X\x1b[2J"), OsString::from("1"))],
            &s.rt,
            sys::geteuid(),
            OsStr::new("/home/teste"),
        );
        assert_eq!(r, Err(ErroSessao::AMais("X??2J".to_owned())));
    }

    #[test]
    fn locale_so_nome_de_locale() {
        for ok in [
            "C",
            "C.UTF-8",
            "pt_BR.UTF-8",
            "en_US.utf8@euro",
            "sr_RS@latin",
        ] {
            assert!(locale_valido(ok.as_bytes()), "{ok}");
        }
        for ruim in [
            "",
            "/tmp/x",
            "../x",
            ".x",
            "a..b",
            "pt BR",
            "a\nb",
            &"a".repeat(65),
        ] {
            assert!(!locale_valido(ruim.as_bytes()), "{ruim:?}");
        }
    }

    #[test]
    fn home_vem_do_passwd() {
        let h = sys::home_do_uid(sys::geteuid()).unwrap();
        assert!(h.as_bytes().starts_with(b"/"));
        assert!(sys::home_do_uid(u32::MAX - 7).is_err());
    }

    #[test]
    fn ataque_lancador_nao_passa_ambiente_nem_fd_do_daemon() {
        // O ambiente deste processo (com o que estiver: `CARGO_*`, `PATH`, …) não
        // chega ao filho, e um fd aberto sem `O_CLOEXEC` também não.
        let s = Sessao::nova("filho");
        let amb = s.montar().unwrap();
        let fd_herdavel = fs::File::open("/dev/null").unwrap();
        sys::tirar_cloexec(fd_herdavel.as_fd()).unwrap();
        assert!(!sys::tem_cloexec(fd_herdavel.as_fd()).unwrap());
        let saida = s.rt.join("saida");
        let arquivo = fs::File::create(&saida).unwrap();
        let mut cmd = Command::new("/usr/bin/sh");
        cmd.arg("-c")
            .arg("env; echo ---; ls /proc/self/fd; echo ---; pwd")
            .env_clear()
            .envs(amb.variaveis())
            .current_dir("/")
            .stdin(Stdio::null())
            .stdout(arquivo);
        sys::fechar_fds_no_exec(&mut cmd);
        assert!(cmd.status().unwrap().success());
        let texto = fs::read_to_string(&saida).unwrap();
        let partes: Vec<&str> = texto.split("---\n").collect();
        let mut nomes: Vec<&str> = partes[0]
            .lines()
            .filter_map(|l| l.split('=').next())
            .filter(|n| !matches!(*n, "PWD" | "SHLVL" | "_" | "OLDPWD"))
            .collect();
        nomes.sort_unstable();
        let mut esperados: Vec<&str> = amb.variaveis().map(|(k, _)| k).collect();
        esperados.sort_unstable();
        assert_eq!(nomes, esperados);
        // Só 0, 1, 2 e o fd do próprio `ls`.
        let fds: Vec<&str> = partes[1].lines().collect();
        assert!(fds.len() <= 4, "fds herdados: {fds:?}");
        assert_eq!(partes[2].trim(), "/");
        drop(fd_herdavel);
    }

    #[test]
    fn lancar_poe_dev_null_e_fecha_fds() {
        let s = Sessao::nova("lancar");
        let amb = s.montar().unwrap();
        let mut filho = lancar(Path::new("/usr/bin/true"), &amb).unwrap();
        assert!(filho.wait().unwrap().success());
        // Programa inexistente: o erro do `exec` chega ao pai (o canal do `std` não
        // foi fechado pelo `close_range`).
        assert!(lancar(Path::new("/usr/bin/ward-nao-existe-falso"), &amb).is_err());
    }

    /// Pendência da 29a: o código de saída do app (que tem 0–2 em `/dev/null`) fica
    /// com o lançador até o daemon tomar e registrar; uma vez só. Enquanto vivo, nada.
    #[test]
    fn saida_do_app_fica_para_o_log() {
        let s = Sessao::nova("saida");
        let amb = s.montar().unwrap();
        let mut l = Lancador::de(Origem::Instalado {
            binario: BinarioPermitido::novo("/usr/bin/false").unwrap(),
            gid_app: 4_000_000,
        });
        assert_eq!(l.tomar_saida(), None);
        l.filho = Some(lancar(Path::new("/usr/bin/false"), &amb).unwrap());
        while l.aberto() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let st = l.tomar_saida().expect("saída guardada");
        assert_eq!(st.code(), Some(1));
        assert_eq!(l.tomar_saida(), None);
        // Morto por sinal (`yes` não sai sozinho com a saída em `/dev/null`).
        use std::os::unix::process::ExitStatusExt;
        l.filho = Some(
            lancar(Path::new("/usr/bin/yes"), &amb)
                .map(|mut c| {
                    c.kill().unwrap();
                    c
                })
                .unwrap(),
        );
        let st = loop {
            if let Some(st) = l.tomar_saida() {
                break st;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        assert_eq!(st.signal(), Some(9));
    }

    /// V3.36 (D63): só o processo que o lançador abriu, vivo, é "o app lançado". Outro
    /// pid (outro filho do daemon, mesmo grupo e mesmo pai) não é; depois que o app
    /// morre, nem o pid dele.
    #[test]
    fn ataque_so_o_pid_do_lancador_e_o_app() {
        let s = Sessao::nova("pidfd");
        let amb = s.montar().unwrap();
        let mut l = Lancador::de(Origem::Instalado {
            binario: BinarioPermitido::novo("/usr/bin/yes").unwrap(),
            gid_app: 4_000_000,
        });
        assert!(!l.e_o_app_lancado(std::process::id()));
        let filho = lancar(Path::new("/usr/bin/yes"), &amb).unwrap();
        let pid = filho.id();
        let fd = sys::pidfd_open(libc::pid_t::try_from(pid).unwrap()).unwrap();
        l.filho = Some(filho);
        l.pidfd = Some(fd);
        // Outro filho deste processo (como um `ward run` seria), vivo ao mesmo tempo.
        let mut outro = lancar(Path::new("/usr/bin/yes"), &amb).unwrap();
        assert!(l.e_o_app_lancado(pid));
        assert!(!l.e_o_app_lancado(outro.id()));
        assert!(!l.e_o_app_lancado(0));
        outro.kill().unwrap();
        outro.wait().unwrap();
        // O app morre: colhido, o pid deixa de ser o do app.
        l.filho.as_mut().unwrap().kill().unwrap();
        while l.aberto() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!l.e_o_app_lancado(pid));
        assert!(l.pidfd.is_none());
    }

    #[test]
    fn ataque_binario_de_usuario_ou_sem_setgid_e_recusado() {
        let inst_gid = 4_000_000;
        let eu = BinarioPermitido::novo(std::env::current_exe().unwrap()).unwrap();
        assert_eq!(
            conferir_binario_do_app(&eu, inst_gid),
            Err(ErroLancador::BinarioNaoConfiavel)
        );
        // Do root, mas sem setgid (`/usr/bin/true`).
        let t = BinarioPermitido::novo("/usr/bin/true").unwrap();
        assert_eq!(
            conferir_binario_do_app(&t, inst_gid),
            Err(ErroLancador::BinarioNaoConfiavel)
        );
    }

    #[test]
    fn mensagens_sem_valor() {
        let m = ErroSessao::Diferente("WAYLAND_DISPLAY").to_string();
        assert!(m.contains("WAYLAND_DISPLAY") && !m.contains("/tmp"));
        let m = ErroLancador::Sessao(ErroSessao::VariosWayland).to_string();
        assert!(m.starts_with("aprovação indisponível"));
    }

    /// Repositório falso para o binário de teste (D63), em `/run/user/<uid>/` (0700),
    /// apagado ao sair.
    #[cfg(feature = "modo-teste")]
    struct Repo(PathBuf);

    #[cfg(feature = "modo-teste")]
    impl Repo {
        fn novo(nome: &str) -> Repo {
            let p = PathBuf::from(format!(
                "/run/user/{}/ward-lancador-repo-{}-{nome}",
                sys::geteuid(),
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&p);
            fs::DirBuilder::new().mode(0o700).create(&p).unwrap();
            fs::DirBuilder::new()
                .mode(0o700)
                .create(p.join("bin"))
                .unwrap();
            fs::DirBuilder::new()
                .mode(0o700)
                .create(p.join(".dev"))
                .unwrap();
            Repo(p)
        }

        /// Script executável do uid em `bin/<nome>`, fechado antes de qualquer `exec`.
        fn script(&self, nome: &str, corpo: &str) -> PathBuf {
            let c = self.0.join("bin").join(nome);
            fs::write(&c, format!("#!/bin/sh\n{corpo}\n")).unwrap();
            fs::set_permissions(&c, fs::Permissions::from_mode(0o700)).unwrap();
            c
        }

        fn binario(&self, c: &Path) -> Result<BinarioDeTeste, ErroBinarioDeTeste> {
            BinarioDeTeste::novo(
                c.to_path_buf(),
                self.0.clone(),
                self.0.join(".dev/logs"),
                vec![("WARD_DEV_DIR", self.0.join(".dev").display().to_string())],
            )
        }
    }

    #[cfg(feature = "modo-teste")]
    impl Drop for Repo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Abre e espera sair. Outro teste em paralelo que faz `fork` pode segurar por um
    /// instante o fd de escrita do script (ETXTBSY): tenta de novo.
    #[cfg(feature = "modo-teste")]
    fn abrir_e_esperar(l: &mut Lancador) -> (u32, ExitStatus) {
        let pid = (0..50)
            .find_map(|_| match l.abrir() {
                Ok(pid) => Some(pid),
                Err(ErroLancador::Io(io::ErrorKind::ExecutableFileBusy)) => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    None
                }
                Err(e) => panic!("{e}"),
            })
            .expect("ETXTBSY o tempo todo");
        loop {
            if let Some(st) = l.tomar_saida() {
                return (pid, st);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    #[cfg(feature = "modo-teste")]
    fn ataque_binario_de_teste_fora_das_regras_e_recusado() {
        use ErroBinarioDeTeste as E;
        let r = Repo::novo("regras");
        let bom = r.script("ward-ponte-teste", "exit 0");
        assert!(r.binario(&bom).is_ok());
        // Fora do repositório (a raiz não conta), relativo, com `..`, nome estranho.
        let fora = BinarioDeTeste::novo(
            bom.clone(),
            r.0.join("bin/outra"),
            r.0.join(".dev/logs"),
            vec![],
        );
        assert_eq!(fora.err(), Some(E::ForaDoRepositorio));
        assert_eq!(r.binario(&r.0).err(), Some(E::ForaDoRepositorio));
        assert_eq!(r.binario(Path::new("bin/x")).err(), Some(E::Caminho));
        assert_eq!(
            r.binario(&r.0.join("bin/../bin/ward-ponte-teste")).err(),
            Some(E::Caminho)
        );
        let estranho = r.script("-x", "exit 0");
        assert_eq!(r.binario(&estranho).err(), Some(E::Caminho));
        // Link para o binário, e link numa pasta do caminho.
        symlink(&bom, r.0.join("bin/link")).unwrap();
        assert_eq!(r.binario(&r.0.join("bin/link")).err(), Some(E::Link));
        symlink(r.0.join("bin"), r.0.join("pasta-link")).unwrap();
        assert_eq!(
            r.binario(&r.0.join("pasta-link/ward-ponte-teste")).err(),
            Some(E::Link)
        );
        // De outro dono (o `true` do sistema, com a raiz em `/usr/bin`).
        let root = BinarioDeTeste::novo(
            PathBuf::from("/usr/bin/true"),
            PathBuf::from("/usr/bin"),
            r.0.join(".dev/logs"),
            vec![],
        );
        assert_eq!(root.err(), Some(E::OutroDono));
        // Não executável, pasta e escrita para outros.
        let texto = r.0.join("bin/texto");
        fs::write(&texto, b"x").unwrap();
        fs::set_permissions(&texto, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(r.binario(&texto).err(), Some(E::NaoArquivo));
        assert_eq!(r.binario(&r.0.join("bin")).err(), Some(E::NaoArquivo));
        fs::set_permissions(&bom, fs::Permissions::from_mode(0o702)).unwrap();
        assert_eq!(r.binario(&bom).err(), Some(E::Gravavel));
        fs::set_permissions(&bom, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(r.0.join("bin"), fs::Permissions::from_mode(0o703)).unwrap();
        assert_eq!(r.binario(&bom).err(), Some(E::Gravavel));
        fs::set_permissions(r.0.join("bin"), fs::Permissions::from_mode(0o700)).unwrap();
        // Conferido também a cada abertura: trocado por link depois de aceito, não abre.
        let b = r.binario(&bom).unwrap();
        fs::remove_file(&bom).unwrap();
        symlink("/usr/bin/true", &bom).unwrap();
        let mut l = Lancador::de_teste(b);
        assert_eq!(l.abrir(), Err(ErroLancador::BinarioNaoConfiavel));
    }

    #[test]
    #[cfg(feature = "modo-teste")]
    fn ponte_de_teste_recebe_o_ambiente_minimo_e_o_log_e_0600() {
        let r = Repo::novo("ambiente");
        let c = r.script("ward-ponte-teste", "/usr/bin/env; echo argumentos=$#");
        let b = r.binario(&c).unwrap();
        let mut l = Lancador::de_teste(b);
        let (pid, st) = abrir_e_esperar(&mut l);
        assert!(st.success());
        assert!(!l.e_o_app_lancado(pid), "o pid morto continua app");
        let log = r.0.join(".dev/logs/ward-ponte-teste.log");
        let m = fs::symlink_metadata(&log).unwrap();
        assert_eq!(m.mode() & 0o777, 0o600);
        let texto = fs::read_to_string(&log).unwrap();
        let mut nomes: Vec<&str> = texto
            .lines()
            .filter_map(|l| l.split_once('=').map(|(k, _)| k))
            .filter(|k| *k != "argumentos" && *k != "PWD")
            .collect();
        nomes.sort_unstable();
        let mut esperados = vec!["HOME", "PATH", "WARD_DEV_DIR", "XDG_RUNTIME_DIR"];
        if std::env::var_os("LANG").is_some_and(|v| locale_valido(v.as_bytes())) {
            esperados.push("LANG");
        }
        esperados.sort_unstable();
        assert_eq!(nomes, esperados, "{texto}");
        assert!(texto.contains("PATH=/usr/bin:/bin\n"));
        assert!(
            texto.contains("argumentos=0\n"),
            "argumentos do pedido chegaram"
        );
        // O log como link para fora: não abre (nada é escrito lá).
        fs::remove_file(&log).unwrap();
        let alvo = r.0.join("alvo.txt");
        fs::write(&alvo, b"").unwrap();
        symlink(&alvo, &log).unwrap();
        assert!(matches!(l.abrir(), Err(ErroLancador::Io(_))));
        assert_eq!(fs::read(&alvo).unwrap(), b"");
    }

    #[test]
    #[cfg(feature = "modo-teste")]
    fn app_de_teste_recebe_o_ambiente_de_d57() {
        let r = Repo::novo("d57");
        let c = r.script(NOME_APP_DE_TESTE, "exit 0");
        let b = r.binario(&c).unwrap();
        // Sem sessão Wayland única neste host, os dois recusam igual (D21).
        match (b.ambiente(), ambiente_do_app()) {
            (Ok(a), Ok(d57)) => {
                assert_eq!(a, d57);
                assert!(a.valor("WARD_DEV_DIR").is_none());
            }
            (Err(a), Err(d57)) => assert_eq!(a, d57),
            (a, d57) => panic!("{a:?} × {d57:?}"),
        }
    }
}
