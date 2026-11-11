//! SSH bridge of the jump host and of the tunnel (Phase 28; D12, D13, D19, D43; S1–S4,
//! S8, S13).
//!
//! **Why not `ProxyJump`.** In OpenSSH, `ProxyJump` becomes a `ProxyCommand`
//! (`ssh -l <user> -p <port> -F <config> -W '[%h]:%p' <jump>`) that `ssh` runs through
//! `$SHELL -c "exec …"`, and the command line `-o` options do not reach the jump `ssh`
//! (only `-F` does). That puts an `sh -c` in the chain (V4.35 forbids it) and leaves the
//! jump `ssh` with the `known_hosts`, the identities and the default agent of `~/.ssh`
//! (D13, R19). Here the jump and the tunnel are the same mechanism, without a shell:
//!
//! - the daemon listens on `127.0.0.1:<free port>` (the port comes from the kernel and is
//!   held by the daemon's socket: nobody takes it first);
//! - each accepted connection only goes through if the peer is of the same uid **and**
//!   a process of the process group **and of the session** of the session's child (the
//!   main `ssh`, or the `ward run` command; the child is a session leader through
//!   `setsid`), checked in `/proc/net/tcp` and in the fds of `/proc/<pid>/fd`; any other
//!   one is closed without a single byte and without launching anything. The group alone
//!   was not enough: another process of the user joined it through `setpgid` (M4
//!   audit); the session cannot be switched to one that already exists. The exact pid
//!   does not work: whoever connects is a descendant (`sshpass` → `ssh`, `sftp` →
//!   `ssh`, a shell → `psql`);
//! - for the accepted connection, the daemon launches through `execve` (no shell) an
//!   `ssh -W <target>:<port>` to the jump/tunnel entry, with the same hardening as
//!   `ward ssh` (`-F /dev/null`, Ward's `known_hosts`, environment from scratch, user
//!   only through `-l`, host after `--`), the password only in `SSHPASS` (`sshpass -e`)
//!   or the key only in this `ssh`'s D12 agent;
//! - the bytes flow between the connection and that `ssh`'s `stdin`/`stdout`; its
//!   `stderr` goes through the mask of both entries (D19) before reaching the CLI;
//! - the bridge ends with the session (each connection's `ssh` is killed) and stops
//!   accepting when the vault locks (lock counter, like the D12 agent).
//!
//! The main `ssh` of the jump connects to `127.0.0.1:<port>` with `HostKeyAlias` equal
//! to the name `known_hosts` would have on a direct connection (`host` on 22,
//! `[host]:port` on the others), so the destination host key is still checked (D13).

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

/// How often the listener checks whether it should stop.
const STEP_MS: i32 = 100;
/// Chunk copied from one side to the other.
const CHUNK: usize = 16 * 1024;
/// `PATH` of the bridge's `ssh` (the programs go by absolute path).
const CHILD_PATH: &[u8] = b"/usr/bin:/bin";

/// How many connections the bridge accepts.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Limits {
    /// At the same time.
    pub(crate) concurrent: usize,
    /// In the whole session.
    pub(crate) total: usize,
}

/// Jump of `ward ssh`: a single connection (the main `ssh`'s).
pub(crate) const JUMP_LIMITS: Limits = Limits {
    concurrent: 1,
    total: 1,
};
/// Tunnel of the database entry in `ward run`: the client may open a few connections.
pub(crate) const TUNNEL_LIMITS: Limits = Limits {
    concurrent: 8,
    total: 64,
};

/// The jump or tunnel server entry, copied under the state lock.
pub(crate) struct Intermediate {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) username: String,
    pub(crate) cred: Cred,
}

/// Everything the bridge needs to launch the `ssh -W` of a connection.
struct Plan {
    via: Intermediate,
    target_host: String,
    target_port: u16,
    programs: ProgramasSsh,
    known_hosts: PathBuf,
    rt: Option<PathBuf>,
}

/// The listening bridge, still without the pid of the session's child.
pub(crate) struct Bridge {
    listener: TcpListener,
    port: u16,
    plan: Plan,
    mask: Arc<Mascara>,
    limits: Limits,
    locks: Arc<AtomicU64>,
    initial: u64,
}

impl std::fmt::Debug for Bridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bridge(127.0.0.1:{}, ••••)", self.port)
    }
}

/// What the bridge does with the already masked `stderr` of a connection's `ssh`.
pub(crate) type Stderr = Arc<dyn Fn(&mut Zeroizing<Vec<u8>>) + Send + Sync>;

/// The parameters of [`Bridge::open`].
pub(crate) struct Opening<'a> {
    pub(crate) via: Intermediate,
    pub(crate) target_host: &'a str,
    pub(crate) target_port: u16,
    pub(crate) programs: &'a ProgramasSsh,
    pub(crate) known_hosts: &'a Path,
    pub(crate) rt: Option<&'a Path>,
    pub(crate) mask: Mascara,
    pub(crate) limits: Limits,
    pub(crate) locks: (Arc<AtomicU64>, u64),
}

impl Bridge {
    /// Listens on `127.0.0.1:0` (the port comes from the kernel) and checks right away
    /// the argv of the bridge's `ssh` (a value out of format opens nothing).
    pub(crate) fn open(a: Opening<'_>) -> Result<Bridge, &'static str> {
        let plan = Plan {
            via: a.via,
            target_host: a.target_host.to_owned(),
            target_port: a.target_port,
            programs: a.programs.clone(),
            known_hosts: a.known_hosts.to_path_buf(),
            rt: a.rt.map(Path::to_path_buf),
        };
        // The argv with example paths: only to refuse before listening.
        let example = Path::new("/nonexistent/agent.sock");
        let auth = match plan.via.cred {
            Cred::Senha(_) => Autent::Senha,
            Cred::Chave { .. } => Autent::Chave {
                agente: example,
                publica: example,
            },
        };
        if bridge_argv(&plan, auth).is_none() {
            return Err("salto ou túnel: host, usuário ou porta fora do formato");
        }
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .map_err(|_| "salto ou túnel: não deu para escutar em 127.0.0.1")?;
        listener
            .set_nonblocking(true)
            .map_err(|_| "salto ou túnel: escuta")?;
        let port = match listener.local_addr() {
            Ok(SocketAddr::V4(e)) if e.port() != 0 => e.port(),
            _ => return Err("salto ou túnel: porta local"),
        };
        Ok(Bridge {
            listener,
            port,
            plan,
            mask: Arc::new(a.mask),
            limits: a.limits,
            locks: a.locks.0,
            initial: a.locks.1,
        })
    }

    /// The local port (`127.0.0.1:<port>`).
    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    /// Starts accepting connections from the process group `group` (the session's
    /// child is the leader). The masked `stderr` of each bridge `ssh` goes to `stderr`.
    pub(crate) fn start(self, group: u32, stderr: Stderr) -> RunningBridge {
        let stop = Arc::new(AtomicBool::new(false));
        let groups: Arc<Mutex<Vec<Arc<Group>>>> = Arc::new(Mutex::new(Vec::new()));
        let (s, g) = (stop.clone(), groups.clone());
        let thread = std::thread::Builder::new()
            .name("wardd-ponte".into())
            .spawn(move || listen(self, group, &s, &g, &stderr));
        if thread.is_err() {
            aviso!("ponte: a thread não foi criada");
        }
        RunningBridge {
            stop,
            groups,
            thread: thread.ok(),
        }
    }
}

/// The accepting bridge. On drop, it stops accepting and kills each connection's `ssh`.
pub(crate) struct RunningBridge {
    stop: Arc<AtomicBool>,
    groups: Arc<Mutex<Vec<Arc<Group>>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl std::fmt::Debug for RunningBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RunningBridge")
    }
}

impl Drop for RunningBridge {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        for g in self.groups.lock().unwrap_or_else(|e| e.into_inner()).iter() {
            g.kill();
        }
    }
}

/// The argv of the bridge's `ssh -W` (`None`: value out of format).
fn bridge_argv(plan: &Plan, auth: Autent<'_>) -> Option<Vec<Vec<u8>>> {
    montar_argv(
        &plan.programs,
        &plan.known_hosts,
        Destino {
            host: &plan.via.host,
            porta: plan.via.port,
            usuario: &plan.via.username,
        },
        auth,
        Programa::Encaminhar {
            host: &plan.target_host,
            porta: plan.target_port,
        },
    )
}

fn listen(
    bridge: Bridge,
    group: u32,
    stop: &AtomicBool,
    groups: &Mutex<Vec<Arc<Group>>>,
    stderr: &Stderr,
) {
    let bridge = Arc::new(bridge);
    let active = Arc::new(AtomicUsize::new(0));
    let mut accepted = 0usize;
    let uid = ward_platform::effective_uid();
    loop {
        if stop.load(Ordering::Acquire) {
            return;
        }
        if bridge.locks.load(Ordering::Acquire) != bridge.initial {
            depurar!("ponte: o cofre travou; a ponte para de aceitar");
            return;
        }
        match child::ready_to_read_fd(bridge.listener.as_fd(), STEP_MS) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(_) => return,
        }
        let (tcp, peer) = match bridge.listener.accept() {
            Ok(x) => x,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
            Err(_) => return,
        };
        let SocketAddr::V4(peer) = peer else {
            continue;
        };
        let local = SocketAddrV4::new(Ipv4Addr::LOCALHOST, bridge.port);
        if !peer_in_group(peer, local, uid, group) {
            // Without a single byte and without launching anything.
            aviso!("ponte: conexão de fora da sessão recusada");
            let _ = tcp.shutdown(Shutdown::Both);
            continue;
        }
        if accepted >= bridge.limits.total
            || active.load(Ordering::Acquire) >= bridge.limits.concurrent
        {
            aviso!("ponte: limite de conexões da sessão; conexão recusada");
            let _ = tcp.shutdown(Shutdown::Both);
            continue;
        }
        accepted += 1;
        active.fetch_add(1, Ordering::AcqRel);
        let (b, a, e) = (bridge.clone(), active.clone(), stderr.clone());
        let launched = connect(&b, tcp, a, e);
        match launched {
            Some(g) => groups.lock().unwrap_or_else(|e| e.into_inner()).push(g),
            None => {
                active.fetch_sub(1, Ordering::AcqRel);
            }
        }
    }
}

/// Launches this connection's `ssh -W` and the threads that move the bytes. Returns the
/// `ssh` group (for the bridge to kill at the end).
fn connect(
    bridge: &Arc<Bridge>,
    tcp: TcpStream,
    active: Arc<AtomicUsize>,
    stderr: Stderr,
) -> Option<Arc<Group>> {
    let _ = tcp.set_nonblocking(false);
    let plan = &bridge.plan;
    let (password, agent) = match &plan.via.cred {
        Cred::Senha(s) => (Some(s), None),
        Cred::Chave { chave, passphrase } => {
            let rt = plan.rt.as_deref()?;
            match AgenteSsh::criar(rt, chave, passphrase.as_ref()) {
                Ok(a) => (None, Some(a)),
                Err(e) => {
                    aviso!("ponte: {e}");
                    return None;
                }
            }
        }
    };
    let paths = agent.as_ref().map(|a| (a.socket(), a.publica()));
    let auth = match &paths {
        None => Autent::Senha,
        Some((sock, publica)) => Autent::Chave {
            agente: sock,
            publica,
        },
    };
    let Some(argv) = bridge_argv(plan, auth) else {
        aviso!("ponte: argv fora do formato; nada foi executado");
        return None;
    };
    let mut cmd = Command::new(&argv[0], b"/").ok()?;
    for x in &argv[1..] {
        cmd.argument(x).ok()?;
    }
    cmd.variable(b"PATH", CHILD_PATH).ok()?;
    cmd.variable(b"LANG", b"C.UTF-8").ok()?;
    if let Some(s) = password {
        // `sshpass -e`: the password only in the child's environment, never in argv
        // (S3).
        cmd.variable(b"SSHPASS", s.expose()).ok()?;
    }
    let launched = cmd.launch();
    // The environment with the secret is zeroed now.
    drop(cmd);
    let (group, pipes) = match launched {
        Ok(x) => x,
        Err(e) => {
            aviso!(
                "ponte: o ssh do salto/túnel não foi criado ({})",
                e.message()
            );
            return None;
        }
    };
    let group = Arc::new(group);
    depurar!("ponte: ssh do salto/túnel pid {}", group.pid());
    let agent = agent.map(|a| {
        a.iniciar(
            Permitido {
                pid: group.pid(),
                filho_do_filho: false,
            },
            bridge.locks.clone(),
            bridge.initial,
        )
    });
    let child::Pipes {
        stdin: input,
        stdout: output,
        stderr: err,
    } = pipes;
    let (Some(input), Some(output), Ok(tcp_read)) = (input, output, tcp.try_clone()) else {
        group.kill();
        let _ = group.collect();
        return None;
    };
    let tcp_write = tcp;
    let tcp_end = tcp_write.try_clone().ok();
    // connection → ssh stdin
    let forward = std::thread::Builder::new()
        .name("wardd-ponte-ida".into())
        .spawn(move || copy(tcp_read, input, None));
    // ssh stdout → connection
    let back = std::thread::Builder::new()
        .name("wardd-ponte-volta".into())
        .spawn(move || copy(output, tcp_write, Some(())));
    if let Some(e) = err {
        let m = bridge.mask.clone();
        let _ = std::thread::Builder::new()
            .name("wardd-ponte-erro".into())
            .spawn(move || read_stderr(e, &m, &stderr));
    }
    if forward.is_err() || back.is_err() {
        group.kill();
    }
    let g = group.clone();
    let watcher = std::thread::Builder::new()
        .name("wardd-ponte-fim".into())
        .spawn(move || {
            let _ = g.wait_for_end();
            g.kill();
            match g.collect() {
                Ok(f) => depurar!("ponte: ssh do salto/túnel saiu ({})", f.exit_code()),
                Err(_) => depurar!("ponte: ssh do salto/túnel já colhido"),
            }
            drop(agent);
            if let Some(t) = tcp_end {
                let _ = t.shutdown(Shutdown::Both);
            }
            active.fetch_sub(1, Ordering::AcqRel);
        });
    if watcher.is_err() {
        group.kill();
    }
    Some(group)
}

/// Copies until the end of `from` (or an error). `half_close`: when done, closes only
/// the write side of the socket (the other side sees the end and can still send the
/// rest).
fn copy<R: Read, W: Write + WriteShutdown>(mut from: R, mut to: W, half_close: Option<()>) {
    let mut buf = Zeroizing::new(vec![0u8; CHUNK]);
    loop {
        let n = match from.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        if to.write_all(&buf[..n]).is_err() {
            break;
        }
    }
    zeroize::Zeroize::zeroize(&mut *buf);
    if half_close.is_some() {
        to.close_write();
    }
}

/// Closes the write side (on the pipe's `File`, it goes away on drop).
trait WriteShutdown {
    fn close_write(&mut self) {}
}

impl WriteShutdown for TcpStream {
    fn close_write(&mut self) {
        let _ = self.shutdown(Shutdown::Write);
    }
}

impl WriteShutdown for fs::File {}

/// The `stderr` of the bridge's `ssh`: masked (both entries, D19) and delivered.
fn read_stderr(mut f: fs::File, mask: &Mascara, sink: &Stderr) {
    let mut m = mask.fluxo();
    // Phase 28g: what is already in the pipe goes in a single batch (like the `ward run`
    // output).
    let mut buf = Zeroizing::new(vec![0u8; execucao::LOTE]);
    let mut out = Zeroizing::new(Vec::with_capacity(2 * CHUNK));
    loop {
        let (n, end) = execucao::ler_lote(&mut f, &mut buf);
        if n > 0 {
            m.mascarar(&buf[..n], &mut out);
            zeroize::Zeroize::zeroize(&mut buf[..n]);
            sink(&mut out);
        }
        if end {
            break;
        }
    }
    m.terminar(&mut out);
    sink(&mut out);
}

// --- Who is on the other side of the connection -----------------------------------------

/// Does the connection come from a process of the uid `uid` in the process group `group`
/// and in the session `group` (the child is the leader of both)? The client's socket is
/// found in `/proc/net/tcp` (local address = the peer, remote = the bridge) and its
/// inode, in the fds of the group's processes.
pub(crate) fn peer_in_group(
    peer: SocketAddrV4,
    bridge: SocketAddrV4,
    uid: u32,
    group: u32,
) -> bool {
    let Ok(table) = fs::read_to_string("/proc/net/tcp") else {
        return false;
    };
    let Some(inode) = peer_socket(&table, peer, bridge, uid) else {
        return false;
    };
    group_process_has_socket(Path::new("/proc"), group, inode)
}

/// `/proc/net/tcp`: the inode of the socket with local address `peer` and remote
/// `bridge`, if it belongs to `uid`. Address in hexadecimal in memory order (`%08X` of
/// the `__be32`), port in hexadecimal (`%04X`).
pub(crate) fn peer_socket(
    table: &str,
    peer: SocketAddrV4,
    bridge: SocketAddrV4,
    uid: u32,
) -> Option<u64> {
    let mut found = None;
    for line in table.lines().skip(1) {
        let c: Vec<&str> = line.split_whitespace().collect();
        if c.len() < 10 {
            continue;
        }
        if address(c[1]) != Some(peer) || address(c[2]) != Some(bridge) {
            continue;
        }
        if c[7].parse::<u32>().ok() != Some(uid) {
            return None;
        }
        let inode = c[9].parse::<u64>().ok().filter(|i| *i != 0)?;
        if found.is_some() {
            // Two sockets with the same address pair: no decision.
            return None;
        }
        found = Some(inode);
    }
    found
}

fn address(s: &str) -> Option<SocketAddrV4> {
    let (ip, port) = s.split_once(':')?;
    if ip.len() != 8 || port.len() != 4 {
        return None;
    }
    let ip = u32::from_str_radix(ip, 16).ok()?;
    let port = u16::from_str_radix(port, 16).ok()?;
    let [a, b, c, d] = ip.to_ne_bytes();
    Some(SocketAddrV4::new(Ipv4Addr::new(a, b, c, d), port))
}

/// Does some process of the child `group` (in `proc`) have the socket `inode` open?
/// Passes:
/// - whoever is in the group `group` **and** in the session `group` (the daemon's child
///   is the leader of both; M4 audit: an outside process that joined the group through
///   `setpgid` would be in another session);
/// - the **direct** child of the leader that opened its own session (Phase 28g):
///   `sshpass` calls `setsid()` in the `ssh` it creates, and without this a server with
///   password and jump was refused. Checked by the parent (`ppid` = the leader, which is
///   still leader of the group and of the session `group`) and by it being the leader
///   of the new session (pid = group = session). No outside process becomes a child of
///   the leader (the parent only changes to a subreaper or init, and the leader's pid is
///   not reused while the group exists); a grandchild that leaves the session is still
///   refused.
pub(crate) fn group_process_has_socket(proc: &Path, group: u32, inode: u64) -> bool {
    let Ok(dir) = fs::read_dir(proc) else {
        return false;
    };
    let target = format!("socket:[{inode}]");
    for e in dir.flatten() {
        let name = e.file_name();
        let Some(pid) = name.to_str().and_then(|n| n.parse::<u32>().ok()) else {
            continue;
        };
        let Ok(stat) = fs::read_to_string(proc.join(pid.to_string()).join("stat")) else {
            continue;
        };
        let in_group = stat_group(&stat) == Some(group) && stat_session(&stat) == Some(group);
        if !in_group && !direct_child_with_own_session(proc, pid, &stat, group) {
            continue;
        }
        let Ok(fds) = fs::read_dir(proc.join(pid.to_string()).join("fd")) else {
            continue;
        };
        for fd in fds.flatten() {
            if fs::read_link(fd.path()).is_ok_and(|l| l.as_os_str() == target.as_str()) {
                return true;
            }
        }
    }
    false
}

/// Is `pid` (with `stat`) a direct child of the leader `group` and the leader of its own
/// session (pid = group = session), and is the leader still the leader of the group and
/// of the session `group`?
fn direct_child_with_own_session(proc: &Path, pid: u32, stat: &str, group: u32) -> bool {
    if pid == group
        || stat_parent(stat) != Some(group)
        || stat_group(stat) != Some(pid)
        || stat_session(stat) != Some(pid)
    {
        return false;
    }
    let Ok(leader) = fs::read_to_string(proc.join(group.to_string()).join("stat")) else {
        return false;
    };
    stat_group(&leader) == Some(group) && stat_session(&leader) == Some(group)
}

/// The `ppid` of `/proc/<pid>/stat` (the 4th field).
pub(crate) fn stat_parent(stat: &str) -> Option<u32> {
    stat_field(stat, 1)
}

/// The `pgrp` of `/proc/<pid>/stat` (the 5th field; the program name, in parentheses,
/// may have spaces and `)`, so the count starts after the last `)`).
pub(crate) fn stat_group(stat: &str) -> Option<u32> {
    stat_field(stat, 2)
}

/// The `session` of `/proc/<pid>/stat` (the 6th field).
pub(crate) fn stat_session(stat: &str) -> Option<u32> {
    stat_field(stat, 3)
}

/// The field `n` (0 = the state) after the program name in `/proc/<pid>/stat`.
fn stat_field(stat: &str, n: usize) -> Option<u32> {
    let rest = &stat[stat.rfind(')')? + 1..];
    rest.split_whitespace().nth(n)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proc_net_tcp_address() {
        // 127.0.0.1:8080 as the kernel prints it on x86 (little-endian).
        let e = if cfg!(target_endian = "little") {
            "0100007F:1F90"
        } else {
            "7F000001:1F90"
        };
        assert_eq!(
            address(e),
            Some(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8080))
        );
        assert_eq!(address("0100007F:1F9"), None);
        assert_eq!(address("zz00007F:1F90"), None);
    }

    fn line(local: &str, remote: &str, uid: u32, inode: u64) -> String {
        format!(
            "   1: {local} {remote} 01 00000000:00000000 00:00000000 00000000  {uid}        0 {inode} 1 0000000000000000 20 4 30 10 -1"
        )
    }

    fn hex(a: SocketAddrV4) -> String {
        let o = a.ip().octets();
        format!("{:08X}:{:04X}", u32::from_ne_bytes(o), a.port())
    }

    /// The socket of the right peer, of the right uid; another uid or an ambiguous
    /// peer: nothing.
    #[test]
    fn attack_socket_of_another_uid_or_ambiguous() {
        let peer = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 40000);
        let bridge = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 41000);
        let head = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n";
        let t = format!(
            "{head}{}\n{}\n",
            line(&hex(bridge), &hex(peer), 1000, 11),
            line(&hex(peer), &hex(bridge), 1000, 22)
        );
        assert_eq!(peer_socket(&t, peer, bridge, 1000), Some(22));
        assert_eq!(peer_socket(&t, peer, bridge, 4242), None);
        let other = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 40001);
        assert_eq!(peer_socket(&t, other, bridge, 1000), None);
        let two = format!("{t}{}\n", line(&hex(peer), &hex(bridge), 1000, 33));
        assert_eq!(peer_socket(&two, peer, bridge, 1000), None);
        let zero = format!("{head}{}\n", line(&hex(peer), &hex(bridge), 1000, 0));
        assert_eq!(peer_socket(&zero, peer, bridge, 1000), None);
    }

    #[test]
    fn stat_group_with_parentheses_in_name() {
        assert_eq!(stat_group("123 (ssh) S 1 456 456 0 -1"), Some(456));
        assert_eq!(stat_group("123 (a) b) c) S 1 789 789 0"), Some(789));
        assert_eq!(stat_group("123 (ssh"), None);
        assert_eq!(stat_session("123 (ssh) S 1 456 789 0 -1"), Some(789));
        assert_eq!(stat_session("123 (a) b) c) S 1 789 790 0"), Some(790));
    }

    /// M4 audit (the group-joining script), on the bridge: a process that is in the child's
    /// group but in another session is refused (through the table of a fake `/proc`).
    #[test]
    fn attack_process_in_group_of_another_session_is_refused() {
        let root = std::env::temp_dir().join(format!("bridge-proc-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let create = |pid: u32, group: u32, session: u32, inode: u64| {
            let d = root.join(pid.to_string());
            fs::create_dir_all(d.join("fd")).unwrap();
            fs::write(
                d.join("stat"),
                format!("{pid} (python3) S 1 {group} {session} 0 -1"),
            )
            .unwrap();
            std::os::unix::fs::symlink(format!("socket:[{inode}]"), d.join("fd").join("3"))
                .unwrap();
        };
        // The child (leader of group and session 500) and the attacker in group 500,
        // session 900.
        create(500, 500, 500, 11);
        create(901, 500, 900, 22);
        assert!(group_process_has_socket(&root, 500, 11));
        assert!(!group_process_has_socket(&root, 500, 22));
        let _ = fs::remove_dir_all(&root);
    }

    /// Phase 28g: the `ssh` that `sshpass` (leader 500) creates with `setsid()` passes;
    /// the rest that has its own session does not: outside process (parent = init or
    /// another), grandchild of the leader, child that is not the leader of its session,
    /// and the child of a "leader" that is no longer leader of group and session 500.
    #[test]
    fn f28g_direct_child_of_leader_with_own_session() {
        let root = std::env::temp_dir().join(format!("bridge-proc-28g-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let create = |root: &Path, pid: u32, parent: u32, group: u32, session: u32, inode: u64| {
            let d = root.join(pid.to_string());
            fs::create_dir_all(d.join("fd")).unwrap();
            fs::write(
                d.join("stat"),
                format!("{pid} (ssh) S {parent} {group} {session} 0 -1"),
            )
            .unwrap();
            std::os::unix::fs::symlink(format!("socket:[{inode}]"), d.join("fd").join("3"))
                .unwrap();
        };
        create(&root, 500, 400, 500, 500, 10);
        // The `sshpass` `ssh`: direct child, own session.
        create(&root, 501, 500, 501, 501, 11);
        assert!(group_process_has_socket(&root, 500, 11));
        // From outside, with its own session: parent init, parent another process.
        create(&root, 700, 1, 700, 700, 22);
        create(&root, 701, 699, 701, 701, 23);
        // Grandchild of the leader (parent = 501), own session.
        create(&root, 702, 501, 702, 702, 24);
        // Direct child that is not the leader of its session (own group in session 900).
        create(&root, 703, 500, 703, 900, 25);
        // Direct child in another group of session 500 (not its own session).
        create(&root, 704, 500, 704, 500, 26);
        for inode in [22, 23, 24, 25, 26] {
            assert!(
                !group_process_has_socket(&root, 500, inode),
                "inode {inode} passou"
            );
        }
        let _ = fs::remove_dir_all(&root);

        // Leader 500 left its session (or the pid belongs to someone else): the child
        // does not pass.
        create(&root, 500, 400, 500, 300, 10);
        create(&root, 501, 500, 501, 501, 11);
        assert!(!group_process_has_socket(&root, 500, 11));
        let _ = fs::remove_dir_all(&root);
        // Without the leader in `/proc`: does not pass.
        create(&root, 501, 500, 501, 501, 11);
        assert!(!group_process_has_socket(&root, 500, 11));
        let _ = fs::remove_dir_all(&root);
    }

    /// For real: the socket of this connection belongs to this process, and the group of
    /// this process is the test's; with another group, it refuses.
    #[test]
    fn peer_in_group_for_real() {
        let l = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
        let SocketAddr::V4(local) = l.local_addr().unwrap() else {
            panic!()
        };
        let c = TcpStream::connect(local).unwrap();
        let (_s, peer) = l.accept().unwrap();
        let SocketAddr::V4(peer) = peer else { panic!() };
        let stat = fs::read_to_string("/proc/self/stat").unwrap();
        let my_group = stat_group(&stat).unwrap();
        let my_session = stat_session(&stat).unwrap();
        let uid = ward_platform::effective_uid();
        // Only whoever is in the group **and** in the session with the same number
        // passes (the daemon's child, leader of both). A process in the group of another
        // session (what the `setpgid` of the M4 audit did) does not pass.
        assert_eq!(
            peer_in_group(peer, local, uid, my_group),
            my_group == my_session
        );
        assert_eq!(
            peer_in_group(peer, local, uid, my_session),
            my_group == my_session
        );
        assert!(!peer_in_group(peer, local, uid, my_group.wrapping_add(1)));
        assert!(!peer_in_group(peer, local, uid.wrapping_add(1), my_group));
        drop(c);
    }
}
