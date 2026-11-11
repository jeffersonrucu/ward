//! Ponte SSH do salto e do túnel (Fase 28; D12, D13, D19, D43; S1–S4, S8, S13).
//!
//! **Por que não `ProxyJump`.** No OpenSSH, o `ProxyJump` vira um `ProxyCommand`
//! (`ssh -l <usuário> -p <porta> -F <config> -W '[%h]:%p' <salto>`) que o `ssh` executa por
//! `$SHELL -c "exec …"`, e as opções de `-o` da linha de comando não chegam ao `ssh` do
//! salto (só o `-F`). Isso põe um `sh -c` na cadeia (V4.35 proíbe) e deixa o `ssh` do
//! salto com o `known_hosts`, as identidades e o agente padrão do `~/.ssh` (D13, R19).
//! Aqui o salto e o túnel são o mesmo mecanismo, sem shell:
//!
//! - o daemon escuta em `127.0.0.1:<porta livre>` (a porta é do kernel, presa pelo
//!   socket do daemon: ninguém a toma antes);
//! - cada conexão aceita só passa se o par for do mesmo uid **e** de um processo do grupo
//!   de processos **e da sessão** do filho da sessão (o `ssh` principal, ou o comando do
//!   `ward run`; o filho é líder de sessão por `setsid`), conferido em `/proc/net/tcp` e
//!   nos fds de `/proc/<pid>/fd`; qualquer outra é fechada sem nenhum byte e sem lançar
//!   nada. Só o grupo não bastava: outro processo do usuário entrava nele por `setpgid`
//!   (auditoria do M4); a sessão não se troca por uma que já existe. O pid exato não
//!   serve: quem conecta é um descendente (`sshpass` → `ssh`, `sftp` → `ssh`, um shell →
//!   `psql`);
//! - para a conexão aceita, o daemon lança por `execve` (sem shell) um `ssh -W
//!   <alvo>:<porta>` até a entrada de salto/túnel, com o mesmo endurecimento do `ward
//!   ssh` (`-F /dev/null`, `known_hosts` do Ward, ambiente do zero, usuário só por `-l`,
//!   host depois de `--`), a senha só em `SSHPASS` (`sshpass -e`) ou a chave só no agente
//!   de D12 deste `ssh`;
//! - os bytes passam entre a conexão e o `stdin`/`stdout` desse `ssh`; o `stderr` dele
//!   passa pela máscara das duas entradas (D19) antes de chegar à CLI;
//! - a ponte acaba com a sessão (o `ssh` de cada conexão é morto) e para de aceitar
//!   quando o cofre trava (contador `travas`, como o agente de D12).
//!
//! O `ssh` principal do salto conecta em `127.0.0.1:<porta>` com `HostKeyAlias` igual ao
//! nome que o `known_hosts` teria numa conexão direta (`host` na 22, `[host]:porta` nas
//! outras), então a chave do host de destino continua conferida (D13).

use std::fs;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use ward_core::mascara::Mascara;
use ward_platform::child::{self, Command, Group};
use zeroize::Zeroizing;

use crate::agente_ssh::{AgenteSsh, Permitido};
use crate::execucao;
use crate::ssh::{Autent, Cred, Destino, Programa, ProgramasSsh, montar_argv};
use crate::{aviso, depurar};

/// De quanto em quanto tempo a escuta olha se deve parar.
const PASSO_MS: i32 = 100;
/// Pedaço copiado de um lado para o outro.
const PEDACO: usize = 16 * 1024;
/// `PATH` do `ssh` da ponte (os programas vão por caminho absoluto).
const PATH_FILHO: &[u8] = b"/usr/bin:/bin";

/// Quantas conexões a ponte aceita.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Limites {
    /// Ao mesmo tempo.
    pub(crate) simultaneas: usize,
    /// No total da sessão.
    pub(crate) total: usize,
}

/// Salto do `ward ssh`: uma conexão só (a do `ssh` principal).
pub(crate) const LIMITES_SALTO: Limites = Limites {
    simultaneas: 1,
    total: 1,
};
/// Túnel do `banco` no `ward run`: o cliente pode abrir algumas conexões.
pub(crate) const LIMITES_TUNEL: Limites = Limites {
    simultaneas: 8,
    total: 64,
};

/// A entrada `servidor` de salto ou de túnel, copiada sob a trava do estado.
pub(crate) struct Intermediario {
    pub(crate) host: String,
    pub(crate) porta: u16,
    pub(crate) usuario: String,
    pub(crate) cred: Cred,
}

/// Tudo que a ponte precisa para lançar o `ssh -W` de uma conexão.
struct Plano {
    via: Intermediario,
    alvo_host: String,
    alvo_porta: u16,
    programas: ProgramasSsh,
    known_hosts: PathBuf,
    rt: Option<PathBuf>,
}

/// A ponte escutando, ainda sem o pid do filho da sessão.
pub(crate) struct Ponte {
    ouvinte: TcpListener,
    porta: u16,
    plano: Plano,
    mascara: Arc<Mascara>,
    limites: Limites,
    travas: Arc<AtomicU64>,
    inicial: u64,
}

impl std::fmt::Debug for Ponte {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Ponte(127.0.0.1:{}, ••••)", self.porta)
    }
}

/// O que a ponte faz com o `stderr` já mascarado do `ssh` de uma conexão.
pub(crate) type SaidaDeErro = Arc<dyn Fn(&mut Zeroizing<Vec<u8>>) + Send + Sync>;

/// Os parâmetros de [`Ponte::abrir`].
pub(crate) struct Abertura<'a> {
    pub(crate) via: Intermediario,
    pub(crate) alvo_host: &'a str,
    pub(crate) alvo_porta: u16,
    pub(crate) programas: &'a ProgramasSsh,
    pub(crate) known_hosts: &'a Path,
    pub(crate) rt: Option<&'a Path>,
    pub(crate) mascara: Mascara,
    pub(crate) limites: Limites,
    pub(crate) travas: (Arc<AtomicU64>, u64),
}

impl Ponte {
    /// Escuta em `127.0.0.1:0` (a porta sai do kernel) e confere já o argv do `ssh` da
    /// ponte (valor fora do formato não abre nada).
    pub(crate) fn abrir(a: Abertura<'_>) -> Result<Ponte, &'static str> {
        let plano = Plano {
            via: a.via,
            alvo_host: a.alvo_host.to_owned(),
            alvo_porta: a.alvo_porta,
            programas: a.programas.clone(),
            known_hosts: a.known_hosts.to_path_buf(),
            rt: a.rt.map(Path::to_path_buf),
        };
        // O argv com caminhos de exemplo: só para recusar antes de escutar.
        let exemplo = Path::new("/nonexistent/agent.sock");
        let autent = match plano.via.cred {
            Cred::Senha(_) => Autent::Senha,
            Cred::Chave { .. } => Autent::Chave {
                agente: exemplo,
                publica: exemplo,
            },
        };
        if argv_da_ponte(&plano, autent).is_none() {
            return Err("salto ou túnel: host, usuário ou porta fora do formato");
        }
        let ouvinte = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .map_err(|_| "salto ou túnel: não deu para escutar em 127.0.0.1")?;
        ouvinte
            .set_nonblocking(true)
            .map_err(|_| "salto ou túnel: escuta")?;
        let porta = match ouvinte.local_addr() {
            Ok(SocketAddr::V4(e)) if e.port() != 0 => e.port(),
            _ => return Err("salto ou túnel: porta local"),
        };
        Ok(Ponte {
            ouvinte,
            porta,
            plano,
            mascara: Arc::new(a.mascara),
            limites: a.limites,
            travas: a.travas.0,
            inicial: a.travas.1,
        })
    }

    /// A porta local (`127.0.0.1:<porta>`).
    pub(crate) fn porta(&self) -> u16 {
        self.porta
    }

    /// Começa a aceitar conexões do grupo de processos `grupo` (o filho da sessão é o
    /// líder). O `stderr` mascarado de cada `ssh` da ponte vai para `erro`.
    pub(crate) fn iniciar(self, grupo: u32, erro: SaidaDeErro) -> PonteRodando {
        let parar = Arc::new(AtomicBool::new(false));
        let grupos: Arc<Mutex<Vec<Arc<Group>>>> = Arc::new(Mutex::new(Vec::new()));
        let (p, g) = (parar.clone(), grupos.clone());
        let thread = std::thread::Builder::new()
            .name("wardd-ponte".into())
            .spawn(move || escutar(self, grupo, &p, &g, &erro));
        if thread.is_err() {
            aviso!("ponte: a thread não foi criada");
        }
        PonteRodando {
            parar,
            grupos,
            thread: thread.ok(),
        }
    }
}

/// A ponte aceitando. Ao sair, para de aceitar e mata o `ssh` de cada conexão.
pub(crate) struct PonteRodando {
    parar: Arc<AtomicBool>,
    grupos: Arc<Mutex<Vec<Arc<Group>>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl std::fmt::Debug for PonteRodando {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PonteRodando")
    }
}

impl Drop for PonteRodando {
    fn drop(&mut self) {
        self.parar.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        for g in self.grupos.lock().unwrap_or_else(|e| e.into_inner()).iter() {
            g.kill();
        }
    }
}

/// O argv do `ssh -W` da ponte (`None`: valor fora do formato).
fn argv_da_ponte(plano: &Plano, autent: Autent<'_>) -> Option<Vec<Vec<u8>>> {
    montar_argv(
        &plano.programas,
        &plano.known_hosts,
        Destino {
            host: &plano.via.host,
            porta: plano.via.porta,
            usuario: &plano.via.usuario,
        },
        autent,
        Programa::Encaminhar {
            host: &plano.alvo_host,
            porta: plano.alvo_porta,
        },
    )
}

fn escutar(
    ponte: Ponte,
    grupo: u32,
    parar: &AtomicBool,
    grupos: &Mutex<Vec<Arc<Group>>>,
    erro: &SaidaDeErro,
) {
    let ponte = Arc::new(ponte);
    let ativas = Arc::new(AtomicUsize::new(0));
    let mut aceitas = 0usize;
    let uid = ward_platform::effective_uid();
    loop {
        if parar.load(Ordering::Acquire) {
            return;
        }
        if ponte.travas.load(Ordering::Acquire) != ponte.inicial {
            depurar!("ponte: o cofre travou; a ponte para de aceitar");
            return;
        }
        match child::ready_to_read_fd(ponte.ouvinte.as_fd(), PASSO_MS) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(_) => return,
        }
        let (tcp, par) = match ponte.ouvinte.accept() {
            Ok(x) => x,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
            Err(_) => return,
        };
        let SocketAddr::V4(par) = par else {
            continue;
        };
        let local = SocketAddrV4::new(Ipv4Addr::LOCALHOST, ponte.porta);
        if !par_do_grupo(par, local, uid, grupo) {
            // Sem nenhum byte e sem lançar nada.
            aviso!("ponte: conexão de fora da sessão recusada");
            let _ = tcp.shutdown(Shutdown::Both);
            continue;
        }
        if aceitas >= ponte.limites.total
            || ativas.load(Ordering::Acquire) >= ponte.limites.simultaneas
        {
            aviso!("ponte: limite de conexões da sessão; conexão recusada");
            let _ = tcp.shutdown(Shutdown::Both);
            continue;
        }
        aceitas += 1;
        ativas.fetch_add(1, Ordering::AcqRel);
        let (p, a, e) = (ponte.clone(), ativas.clone(), erro.clone());
        let lancado = conectar(&p, tcp, a, e);
        match lancado {
            Some(g) => grupos.lock().unwrap_or_else(|e| e.into_inner()).push(g),
            None => {
                ativas.fetch_sub(1, Ordering::AcqRel);
            }
        }
    }
}

/// Lança o `ssh -W` desta conexão e as threads que passam os bytes. Devolve o grupo do
/// `ssh` (para a ponte matar no fim).
fn conectar(
    ponte: &Arc<Ponte>,
    tcp: TcpStream,
    ativas: Arc<AtomicUsize>,
    erro: SaidaDeErro,
) -> Option<Arc<Group>> {
    let _ = tcp.set_nonblocking(false);
    let plano = &ponte.plano;
    let (senha, agente) = match &plano.via.cred {
        Cred::Senha(s) => (Some(s), None),
        Cred::Chave { chave, passphrase } => {
            let rt = plano.rt.as_deref()?;
            match AgenteSsh::criar(rt, chave, passphrase.as_ref()) {
                Ok(a) => (None, Some(a)),
                Err(e) => {
                    aviso!("ponte: {e}");
                    return None;
                }
            }
        }
    };
    let caminhos = agente.as_ref().map(|a| (a.socket(), a.publica()));
    let autent = match &caminhos {
        None => Autent::Senha,
        Some((sock, publica)) => Autent::Chave {
            agente: sock,
            publica,
        },
    };
    let Some(argv) = argv_da_ponte(plano, autent) else {
        aviso!("ponte: argv fora do formato; nada foi executado");
        return None;
    };
    let mut cmd = Command::new(&argv[0], b"/").ok()?;
    for x in &argv[1..] {
        cmd.argument(x).ok()?;
    }
    cmd.variable(b"PATH", PATH_FILHO).ok()?;
    cmd.variable(b"LANG", b"C.UTF-8").ok()?;
    if let Some(s) = senha {
        // `sshpass -e`: a senha só no ambiente do filho, nunca em argv (S3).
        cmd.variable(b"SSHPASS", s.expose()).ok()?;
    }
    let lancado = cmd.launch();
    // O ambiente com segredo zera agora.
    drop(cmd);
    let (grupo, pipes) = match lancado {
        Ok(x) => x,
        Err(e) => {
            aviso!(
                "ponte: o ssh do salto/túnel não foi criado ({})",
                e.message()
            );
            return None;
        }
    };
    let grupo = Arc::new(grupo);
    depurar!("ponte: ssh do salto/túnel pid {}", grupo.pid());
    let agente = agente.map(|a| {
        a.iniciar(
            Permitido {
                pid: grupo.pid(),
                filho_do_filho: false,
            },
            ponte.travas.clone(),
            ponte.inicial,
        )
    });
    let child::Pipes {
        stdin: entrada,
        stdout: saida,
        stderr: err,
    } = pipes;
    let (Some(entrada), Some(saida), Ok(tcp_leitura)) = (entrada, saida, tcp.try_clone()) else {
        grupo.kill();
        let _ = grupo.collect();
        return None;
    };
    let tcp_escrita = tcp;
    let tcp_fim = tcp_escrita.try_clone().ok();
    // conexão → stdin do ssh
    let ida = std::thread::Builder::new()
        .name("wardd-ponte-ida".into())
        .spawn(move || copiar(tcp_leitura, entrada, None));
    // stdout do ssh → conexão
    let volta = std::thread::Builder::new()
        .name("wardd-ponte-volta".into())
        .spawn(move || copiar(saida, tcp_escrita, Some(())));
    if let Some(e) = err {
        let m = ponte.mascara.clone();
        let _ = std::thread::Builder::new()
            .name("wardd-ponte-erro".into())
            .spawn(move || ler_erro(e, &m, &erro));
    }
    if ida.is_err() || volta.is_err() {
        grupo.kill();
    }
    let g = grupo.clone();
    let vigia = std::thread::Builder::new()
        .name("wardd-ponte-fim".into())
        .spawn(move || {
            let _ = g.wait_for_end();
            g.kill();
            match g.collect() {
                Ok(f) => depurar!("ponte: ssh do salto/túnel saiu ({})", f.exit_code()),
                Err(_) => depurar!("ponte: ssh do salto/túnel já colhido"),
            }
            drop(agente);
            if let Some(t) = tcp_fim {
                let _ = t.shutdown(Shutdown::Both);
            }
            ativas.fetch_sub(1, Ordering::AcqRel);
        });
    if vigia.is_err() {
        grupo.kill();
    }
    Some(grupo)
}

/// Copia até o fim de `de` (ou erro). `meio_fechamento`: ao acabar, fecha só a escrita
/// do socket (o outro lado vê o fim e ainda pode mandar o resto).
fn copiar<R: Read, W: Write + FimDeEscrita>(mut de: R, mut para: W, meio_fechamento: Option<()>) {
    let mut buf = Zeroizing::new(vec![0u8; PEDACO]);
    loop {
        let n = match de.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        if para.write_all(&buf[..n]).is_err() {
            break;
        }
    }
    zeroize::Zeroize::zeroize(&mut *buf);
    if meio_fechamento.is_some() {
        para.fechar_escrita();
    }
}

/// Fecha o lado de escrita (no `File` do pipe, cai ao sair).
trait FimDeEscrita {
    fn fechar_escrita(&mut self) {}
}

impl FimDeEscrita for TcpStream {
    fn fechar_escrita(&mut self) {
        let _ = self.shutdown(Shutdown::Write);
    }
}

impl FimDeEscrita for fs::File {}

/// O `stderr` do `ssh` da ponte: mascarado (as duas entradas, D19) e entregue.
fn ler_erro(mut f: fs::File, mascara: &Mascara, saida: &SaidaDeErro) {
    let mut m = mascara.fluxo();
    // Fase 28g: o que já está no pipe vai num lote só (como a saída do `ward run`).
    let mut buf = Zeroizing::new(vec![0u8; execucao::LOTE]);
    let mut out = Zeroizing::new(Vec::with_capacity(2 * PEDACO));
    loop {
        let (n, fim) = execucao::ler_lote(&mut f, &mut buf);
        if n > 0 {
            m.mascarar(&buf[..n], &mut out);
            zeroize::Zeroize::zeroize(&mut buf[..n]);
            saida(&mut out);
        }
        if fim {
            break;
        }
    }
    m.terminar(&mut out);
    saida(&mut out);
}

// --- Quem está do outro lado da conexão ---------------------------------------------

/// A conexão vem de um processo do uid `uid` no grupo de processos `grupo` e na sessão
/// `grupo` (o filho é líder dos dois)? O socket do cliente é achado em `/proc/net/tcp`
/// (endereço local = o par, remoto = a ponte) e o inode dele, nos fds dos processos do
/// grupo.
pub(crate) fn par_do_grupo(par: SocketAddrV4, ponte: SocketAddrV4, uid: u32, grupo: u32) -> bool {
    let Ok(tabela) = fs::read_to_string("/proc/net/tcp") else {
        return false;
    };
    let Some(inode) = socket_do_par(&tabela, par, ponte, uid) else {
        return false;
    };
    processo_do_grupo_com_socket(Path::new("/proc"), grupo, inode)
}

/// `/proc/net/tcp`: o inode do socket com endereço local `par` e remoto `ponte`, se for
/// do `uid`. Endereço em hexadecimal na ordem da memória (`%08X` do `__be32`), porta em
/// hexadecimal (`%04X`).
pub(crate) fn socket_do_par(
    tabela: &str,
    par: SocketAddrV4,
    ponte: SocketAddrV4,
    uid: u32,
) -> Option<u64> {
    let mut achado = None;
    for linha in tabela.lines().skip(1) {
        let c: Vec<&str> = linha.split_whitespace().collect();
        if c.len() < 10 {
            continue;
        }
        if endereco(c[1]) != Some(par) || endereco(c[2]) != Some(ponte) {
            continue;
        }
        if c[7].parse::<u32>().ok() != Some(uid) {
            return None;
        }
        let inode = c[9].parse::<u64>().ok().filter(|i| *i != 0)?;
        if achado.is_some() {
            // Dois sockets com o mesmo par de endereços: não decide.
            return None;
        }
        achado = Some(inode);
    }
    achado
}

fn endereco(s: &str) -> Option<SocketAddrV4> {
    let (ip, porta) = s.split_once(':')?;
    if ip.len() != 8 || porta.len() != 4 {
        return None;
    }
    let ip = u32::from_str_radix(ip, 16).ok()?;
    let porta = u16::from_str_radix(porta, 16).ok()?;
    let [a, b, c, d] = ip.to_ne_bytes();
    Some(SocketAddrV4::new(Ipv4Addr::new(a, b, c, d), porta))
}

/// Algum processo do filho `grupo` (em `proc`) tem o socket `inode` aberto? Passa:
/// - quem está no grupo `grupo` **e** na sessão `grupo` (o filho do daemon é líder dos
///   dois; auditoria do M4: um processo de fora que entrasse no grupo por `setpgid`
///   ficaria noutra sessão);
/// - o filho **direto** do líder que abriu sessão própria (Fase 28g): o `sshpass` faz
///   `setsid()` no `ssh` que ele cria, e sem isso um servidor com senha e salto era
///   recusado. Confere pelo pai (`ppid` = o líder, que ainda é líder do grupo e da
///   sessão `grupo`) e por ele ser líder da sessão nova (pid = grupo = sessão). Nenhum
///   processo de fora vira filho do líder (o pai só muda para um subreaper ou o init, e
///   o pid do líder não é reusado enquanto o grupo existe); um neto que sai da sessão
///   continua recusado.
pub(crate) fn processo_do_grupo_com_socket(proc: &Path, grupo: u32, inode: u64) -> bool {
    let Ok(dir) = fs::read_dir(proc) else {
        return false;
    };
    let alvo = format!("socket:[{inode}]");
    for e in dir.flatten() {
        let nome = e.file_name();
        let Some(pid) = nome.to_str().and_then(|n| n.parse::<u32>().ok()) else {
            continue;
        };
        let Ok(stat) = fs::read_to_string(proc.join(pid.to_string()).join("stat")) else {
            continue;
        };
        let do_grupo = grupo_do_stat(&stat) == Some(grupo) && sessao_do_stat(&stat) == Some(grupo);
        if !do_grupo && !filho_direto_com_sessao_propria(proc, pid, &stat, grupo) {
            continue;
        }
        let Ok(fds) = fs::read_dir(proc.join(pid.to_string()).join("fd")) else {
            continue;
        };
        for fd in fds.flatten() {
            if fs::read_link(fd.path()).is_ok_and(|l| l.as_os_str() == alvo.as_str()) {
                return true;
            }
        }
    }
    false
}

/// `pid` (com `stat`) é filho direto do líder `grupo` e líder de uma sessão própria
/// (pid = grupo = sessão), e o líder ainda é líder do grupo e da sessão `grupo`?
fn filho_direto_com_sessao_propria(proc: &Path, pid: u32, stat: &str, grupo: u32) -> bool {
    if pid == grupo
        || pai_do_stat(stat) != Some(grupo)
        || grupo_do_stat(stat) != Some(pid)
        || sessao_do_stat(stat) != Some(pid)
    {
        return false;
    }
    let Ok(lider) = fs::read_to_string(proc.join(grupo.to_string()).join("stat")) else {
        return false;
    };
    grupo_do_stat(&lider) == Some(grupo) && sessao_do_stat(&lider) == Some(grupo)
}

/// O `ppid` de `/proc/<pid>/stat` (o 4.º campo).
pub(crate) fn pai_do_stat(stat: &str) -> Option<u32> {
    campo_do_stat(stat, 1)
}

/// O `pgrp` de `/proc/<pid>/stat` (o 5.º campo; o nome do programa, entre parênteses,
/// pode ter espaço e `)`, então a conta começa depois do último `)`).
pub(crate) fn grupo_do_stat(stat: &str) -> Option<u32> {
    campo_do_stat(stat, 2)
}

/// O `session` de `/proc/<pid>/stat` (o 6.º campo).
pub(crate) fn sessao_do_stat(stat: &str) -> Option<u32> {
    campo_do_stat(stat, 3)
}

/// O campo `n` (0 = o estado) depois do nome do programa em `/proc/<pid>/stat`.
fn campo_do_stat(stat: &str, n: usize) -> Option<u32> {
    let resto = &stat[stat.rfind(')')? + 1..];
    resto.split_whitespace().nth(n)?.parse().ok()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn endereco_do_proc_net_tcp() {
        // 127.0.0.1:8080 como o kernel imprime num x86 (little-endian).
        let e = if cfg!(target_endian = "little") {
            "0100007F:1F90"
        } else {
            "7F000001:1F90"
        };
        assert_eq!(
            endereco(e),
            Some(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8080))
        );
        assert_eq!(endereco("0100007F:1F9"), None);
        assert_eq!(endereco("zz00007F:1F90"), None);
    }

    fn linha(local: &str, remoto: &str, uid: u32, inode: u64) -> String {
        format!(
            "   1: {local} {remoto} 01 00000000:00000000 00:00000000 00000000  {uid}        0 {inode} 1 0000000000000000 20 4 30 10 -1"
        )
    }

    fn hex(a: SocketAddrV4) -> String {
        let o = a.ip().octets();
        format!("{:08X}:{:04X}", u32::from_ne_bytes(o), a.port())
    }

    /// O socket do par certo, do uid certo; outro uid ou par ambíguo: nada.
    #[test]
    fn ataque_socket_de_outro_uid_ou_ambiguo() {
        let par = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 40000);
        let ponte = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 41000);
        let cab = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n";
        let t = format!(
            "{cab}{}\n{}\n",
            linha(&hex(ponte), &hex(par), 1000, 11),
            linha(&hex(par), &hex(ponte), 1000, 22)
        );
        assert_eq!(socket_do_par(&t, par, ponte, 1000), Some(22));
        assert_eq!(socket_do_par(&t, par, ponte, 4242), None);
        let outro = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 40001);
        assert_eq!(socket_do_par(&t, outro, ponte, 1000), None);
        let dois = format!("{t}{}\n", linha(&hex(par), &hex(ponte), 1000, 33));
        assert_eq!(socket_do_par(&dois, par, ponte, 1000), None);
        let zero = format!("{cab}{}\n", linha(&hex(par), &hex(ponte), 1000, 0));
        assert_eq!(socket_do_par(&zero, par, ponte, 1000), None);
    }

    #[test]
    fn grupo_do_stat_com_parenteses_no_nome() {
        assert_eq!(grupo_do_stat("123 (ssh) S 1 456 456 0 -1"), Some(456));
        assert_eq!(grupo_do_stat("123 (a) b) c) S 1 789 789 0"), Some(789));
        assert_eq!(grupo_do_stat("123 (ssh"), None);
        assert_eq!(sessao_do_stat("123 (ssh) S 1 456 789 0 -1"), Some(789));
        assert_eq!(sessao_do_stat("123 (a) b) c) S 1 789 790 0"), Some(790));
    }

    /// Auditoria do M4 (`entrar_grupo.py`), na ponte: um processo que está no grupo do
    /// filho mas em outra sessão é recusado (pela tabela de um `/proc` falso).
    #[test]
    fn ataque_processo_no_grupo_de_outra_sessao_e_recusado() {
        let raiz = std::env::temp_dir().join(format!("ponte-proc-{}", std::process::id()));
        let _ = fs::remove_dir_all(&raiz);
        let criar = |pid: u32, grupo: u32, sessao: u32, inode: u64| {
            let d = raiz.join(pid.to_string());
            fs::create_dir_all(d.join("fd")).unwrap();
            fs::write(
                d.join("stat"),
                format!("{pid} (python3) S 1 {grupo} {sessao} 0 -1"),
            )
            .unwrap();
            std::os::unix::fs::symlink(format!("socket:[{inode}]"), d.join("fd").join("3"))
                .unwrap();
        };
        // O filho (líder do grupo e da sessão 500) e o atacante no grupo 500, sessão 900.
        criar(500, 500, 500, 11);
        criar(901, 500, 900, 22);
        assert!(processo_do_grupo_com_socket(&raiz, 500, 11));
        assert!(!processo_do_grupo_com_socket(&raiz, 500, 22));
        let _ = fs::remove_dir_all(&raiz);
    }

    /// Fase 28g: o `ssh` que o `sshpass` (líder 500) cria com `setsid()` passa; o resto
    /// que tem sessão própria, não: processo de fora (pai = init ou outro), neto do
    /// líder, filho que não é líder da sessão dele, e o filho de um "líder" que já não é
    /// líder do grupo e da sessão 500.
    #[test]
    fn f28g_filho_direto_do_lider_com_sessao_propria() {
        let raiz = std::env::temp_dir().join(format!("ponte-proc-28g-{}", std::process::id()));
        let _ = fs::remove_dir_all(&raiz);
        let criar = |raiz: &Path, pid: u32, pai: u32, grupo: u32, sessao: u32, inode: u64| {
            let d = raiz.join(pid.to_string());
            fs::create_dir_all(d.join("fd")).unwrap();
            fs::write(
                d.join("stat"),
                format!("{pid} (ssh) S {pai} {grupo} {sessao} 0 -1"),
            )
            .unwrap();
            std::os::unix::fs::symlink(format!("socket:[{inode}]"), d.join("fd").join("3"))
                .unwrap();
        };
        criar(&raiz, 500, 400, 500, 500, 10);
        // O `ssh` do `sshpass`: filho direto, sessão própria.
        criar(&raiz, 501, 500, 501, 501, 11);
        assert!(processo_do_grupo_com_socket(&raiz, 500, 11));
        // De fora, com sessão própria: pai init, pai outro processo.
        criar(&raiz, 700, 1, 700, 700, 22);
        criar(&raiz, 701, 699, 701, 701, 23);
        // Neto do líder (pai = o 501), sessão própria.
        criar(&raiz, 702, 501, 702, 702, 24);
        // Filho direto que não é líder da sessão dele (grupo próprio na sessão 900).
        criar(&raiz, 703, 500, 703, 900, 25);
        // Filho direto em outro grupo da sessão 500 (não é a sessão própria).
        criar(&raiz, 704, 500, 704, 500, 26);
        for inode in [22, 23, 24, 25, 26] {
            assert!(
                !processo_do_grupo_com_socket(&raiz, 500, inode),
                "inode {inode} passou"
            );
        }
        let _ = fs::remove_dir_all(&raiz);

        // O líder 500 saiu da sessão dele (ou o pid é de outro): o filho não passa.
        criar(&raiz, 500, 400, 500, 300, 10);
        criar(&raiz, 501, 500, 501, 501, 11);
        assert!(!processo_do_grupo_com_socket(&raiz, 500, 11));
        let _ = fs::remove_dir_all(&raiz);
        // Sem o líder no `/proc`: não passa.
        criar(&raiz, 501, 500, 501, 501, 11);
        assert!(!processo_do_grupo_com_socket(&raiz, 500, 11));
        let _ = fs::remove_dir_all(&raiz);
    }

    /// De verdade: o socket desta conexão é deste processo, e o grupo deste processo é
    /// o do teste; com outro grupo, recusa.
    #[test]
    fn par_do_grupo_de_verdade() {
        let l = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
        let SocketAddr::V4(local) = l.local_addr().unwrap() else {
            panic!()
        };
        let c = TcpStream::connect(local).unwrap();
        let (_s, par) = l.accept().unwrap();
        let SocketAddr::V4(par) = par else { panic!() };
        let stat = fs::read_to_string("/proc/self/stat").unwrap();
        let meu_grupo = grupo_do_stat(&stat).unwrap();
        let minha_sessao = sessao_do_stat(&stat).unwrap();
        let uid = ward_platform::effective_uid();
        // Só passa quem está no grupo **e** na sessão com o mesmo número (o filho do
        // daemon, líder dos dois). Um processo no grupo de outra sessão (o que o
        // `setpgid` da auditoria do M4 fazia) não passa.
        assert_eq!(
            par_do_grupo(par, local, uid, meu_grupo),
            meu_grupo == minha_sessao
        );
        assert_eq!(
            par_do_grupo(par, local, uid, minha_sessao),
            meu_grupo == minha_sessao
        );
        assert!(!par_do_grupo(par, local, uid, meu_grupo.wrapping_add(1)));
        assert!(!par_do_grupo(par, local, uid.wrapping_add(1), meu_grupo));
        drop(c);
    }
}
