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
//!     valor revelado) no barramento AT-SPI da sessão, que qualquer processo do uid lê.
//!
//! Os valores não são segredo; as mensagens de erro levam só o nome da variável.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

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
];

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
    binario: BinarioPermitido,
    gid_app: u32,
    filho: Option<Child>,
}

impl Lancador {
    pub fn novo(binario: BinarioPermitido, inst: &Instalacao) -> Lancador {
        Lancador {
            binario,
            gid_app: inst.gid_app(),
            filho: None,
        }
    }

    /// O app aberto por este lançador ainda está vivo (e colhe o que saiu).
    pub fn aberto(&mut self) -> bool {
        match self.filho.as_mut().map(Child::try_wait) {
            Some(Ok(None)) => true,
            Some(_) => {
                self.filho = None;
                false
            }
            None => false,
        }
    }

    /// Confere o binário e a sessão, monta o ambiente do zero e abre o app. Devolve o
    /// pid do app.
    pub fn abrir(&mut self) -> Result<u32, ErroLancador> {
        if self.aberto() {
            return Err(ErroLancador::JaAberto);
        }
        conferir_binario_do_app(&self.binario, self.gid_app)?;
        let ambiente = ambiente_do_app().map_err(ErroLancador::Sessao)?;
        let filho =
            lancar(self.binario.caminho(), &ambiente).map_err(|e| ErroLancador::Io(e.kind()))?;
        let pid = filho.id();
        self.filho = Some(filho);
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
        use std::os::fd::AsFd;
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
}
