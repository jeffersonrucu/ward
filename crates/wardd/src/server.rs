//! Socket loop: accepts, checks the peer's uid (`SO_PEERCRED`, S5), gives the role by
//! the group (D47), reads frames with the role's limit and answers.
//!
//! - Different uid, or no process identity: closes without sending a single byte and
//!   records `UidRecusado` with uid and pid (V3.2).
//! - At most [`MAX_CONNECTIONS`] agent and extension connections at the same time, one
//!   thread each. Full: the oldest idle connection (waiting for a frame) among them is
//!   dropped to make room for the new one; with no idle one, the new one gets the busy error
//!   and closes. That way 10 000 silent connections do not keep a normal client from
//!   being served (V3.4).
//! - The app role has slots of its own ([`MAX_APP_CONNECTIONS`], M3 scrutiny): its
//!   connection, which waits for notices in silence (subscriber), is never evicted by
//!   connections of another role, and an agent with the 64 slots taken does not take
//!   the app's slot. Only the process that the launcher opened is app (D63), so nobody
//!   else fills those slots.
//! - Each read has a deadline ([`READ_DEADLINE`]); a frame refused by the reading gets
//!   the invalid frame error and the connection closes.
//! - Before each app or extension request, the daemon checks again that the client is
//!   still alive and without a debugger; if not, it closes.
//! - App role (D63): besides the group and the parent (D47), the peer has to be the
//!   process that the launcher opened, alive ([`role_from_class`], the only point of the
//!   classification).
//! - Connection of the app role (D65 a): receives the engine's notices
//!   (approval and unlock requests), each one in a whole frame, never in
//!   the middle of a response (one write lock per connection); the responses follow the
//!   order of the requests. It has no read deadline; when it closes, what it received
//!   and did not answer is denied. Only the app role's connection receives notices.

use std::io;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ward_core::protocol::{
    self, Request, ResponseError, Resposta, Role,
    frame::{self, FrameError},
};
use ward_platform::{AcceptError, Install, KernelPolicy, Listener, PeerClass, PeerProcess};

use crate::approval::{Client, Subscriber};
use crate::core::Daemon;
use crate::launch::{LaunchedApp, TestApp, TestExtension};
use crate::{debug, warning};

/// Agent and extension connections served at the same time.
pub const MAX_CONNECTIONS: usize = 64;
/// Connections of the app role at the same time, apart from the [`MAX_CONNECTIONS`].
pub const MAX_APP_CONNECTIONS: usize = 8;
/// Deadline of each socket read.
pub const READ_DEADLINE: Duration = Duration::from_secs(60);
/// Deadline of each write.
pub const WRITE_DEADLINE: Duration = Duration::from_secs(10);

struct Slot {
    id: u64,
    /// Slot of the app role: outside the count and the eviction of the others.
    app: bool,
    stream: UnixStream,
    idle: Arc<AtomicBool>,
    since: Instant,
}

#[derive(Default)]
struct Slots {
    list: Mutex<Vec<Slot>>,
    next_id: AtomicU64,
}

impl Slots {
    /// Reserves a slot in the role's group (app or the rest). Full: in the group of the
    /// rest, drops its oldest idle one; in the app's, refuses. A connection never drops
    /// one of another group.
    fn reserve(&self, stream: &UnixStream, role: Role) -> Option<(u64, Arc<AtomicBool>)> {
        let app = role == Role::App;
        let mut list = self.list.lock().unwrap_or_else(|e| e.into_inner());
        let limit = if app {
            MAX_APP_CONNECTIONS
        } else {
            MAX_CONNECTIONS
        };
        if list.iter().filter(|v| v.app == app).count() >= limit {
            if app {
                return None;
            }
            let oldest = list
                .iter()
                .enumerate()
                .filter(|(_, v)| !v.app && v.idle.load(Ordering::Acquire))
                .min_by_key(|(_, v)| v.since)
                .map(|(i, _)| i)?;
            let v = list.swap_remove(oldest);
            let _ = v.stream.shutdown(Shutdown::Both);
        }
        let copy = stream.try_clone().ok()?;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let idle = Arc::new(AtomicBool::new(true));
        list.push(Slot {
            id,
            app,
            stream: copy,
            idle: idle.clone(),
            since: Instant::now(),
        });
        Some((id, idle))
    }

    fn mark(&self, id: u64, idle: bool) {
        let mut list = self.list.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(v) = list.iter_mut().find(|v| v.id == id) {
            v.idle.store(idle, Ordering::Release);
            v.since = Instant::now();
        }
    }

    fn release(&self, id: u64) {
        let mut list = self.list.lock().unwrap_or_else(|e| e.into_inner());
        list.retain(|v| v.id != id);
    }
}

struct ReleaseOnDrop(Arc<Slots>, u64);

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release(self.1);
    }
}

/// How the daemon decides the role of each connection.
pub struct Roles {
    /// `None`: install groups missing or refused; every client is an agent.
    pub install: Option<Install>,
    /// The app that the launcher opened (D63). `None`: nobody is app.
    pub app: Option<LaunchedApp>,
    /// Only in the test daemon (D63): the test app, app without the egid. In the release
    /// it is always `None` (the type is not built without the feature).
    pub test_app: Option<TestApp>,
    /// Only in the test daemon with `WARD_DEV_EXTENSAO=chrome` (Phase 41): the
    /// extension's test host, extension without the egid. In the release it is always
    /// `None` (the type is not built without the feature).
    pub test_extension: Option<TestExtension>,
}

/// The role of a peer already classified by the group and by the parent (D47). App role
/// only for the process `pid` that the launcher opened and that is still alive (D63):
/// another child of the daemon with the app's group (a `ward run` of `ward-app`, Phase
/// 26) stays an agent.
pub fn role_from_class(class: PeerClass, pid: u32, is_the_app: impl Fn(u32) -> bool) -> Role {
    match class {
        PeerClass::App if is_the_app(pid) => Role::App,
        PeerClass::App => {
            warning!("papel app recusado (pid {pid}): não é o app aberto pelo lançador (D63)");
            Role::Agent
        }
        PeerClass::ExtensionHost => Role::Extension,
        PeerClass::Common { .. } => Role::Agent,
    }
}

impl Roles {
    fn role(&self, peer: &PeerProcess) -> Role {
        let pid = peer.credentials().pid;
        // D63, test daemon: the launcher's pid and parent = daemon, without the egid. The
        // role comes from the same `role_from_class`.
        if let Some(t) = &self.test_app
            && t.is_the_app(pid)
        {
            match t.classify(peer) {
                c @ PeerClass::App => return role_from_class(c, pid, |p| t.is_the_app(p)),
                PeerClass::Common { reason } => {
                    warning!("papel app de teste recusado (pid {pid}): {reason}");
                }
                PeerClass::ExtensionHost => {}
            }
        }
        // Phase 41, test daemon: parent = installed Chrome through `/proc`, without the
        // egid. Whoever does not pass follows the release path (without the groups,
        // agent).
        if let Some(x) = &self.test_extension
            && let c @ PeerClass::ExtensionHost = x.classify(peer)
        {
            warning!("papel extensão de teste (pid {pid}): pai é o Chrome instalado");
            return role_from_class(c, pid, |_| false);
        }
        let Some(inst) = &self.install else {
            return Role::Agent;
        };
        match peer.classify(inst, &KernelPolicy::read()) {
            c @ (PeerClass::App | PeerClass::ExtensionHost) => role_from_class(c, pid, |p| {
                self.app.as_ref().is_some_and(|a| a.is_the_app(p))
            }),
            PeerClass::Common { reason } => {
                // Only records when the client tried a special group.
                let cred = peer.credentials();
                if cred.gid == inst.app_gid() || cred.gid == inst.extension_gid() {
                    warning!("papel especial recusado (pid {}): {reason}", cred.pid);
                }
                Role::Agent
            }
        }
    }
}

/// Writes a response for the role. A response that the role cannot receive turns into
/// an internal error (no credential ever goes out to the agent, S1).
fn answer(stream: &mut UnixStream, role: Role, r: &Resposta) -> Result<(), FrameError> {
    let q = match protocol::encode_response(role, r) {
        Ok(q) => q,
        Err(_) => {
            warning!("resposta recusada para o papel {}", role.name());
            protocol::encode_response(role, &Resposta::Erro(ResponseError::Internal))
                .map_err(|_| FrameError::Io(io::ErrorKind::InvalidData))?
        }
    };
    frame::write_frame(stream, &q, frame::RESPONSE_LIMIT)
}

/// The connection of the app that subscribed: the engine writes on it with the same
/// lock as the responses.
struct SocketSubscriber {
    stream: UnixStream,
    write_lock: Arc<Mutex<()>>,
}

impl Subscriber for SocketSubscriber {
    fn deliver(&mut self, r: &Resposta) -> bool {
        let _g = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        let ok = answer(&mut self.stream, Role::App, r).is_ok();
        if !ok {
            let _ = self.stream.shutdown(Shutdown::Both);
        }
        ok
    }
}

/// The connection's client, for the engine: process chain and whether it went away.
struct SocketClient<'a> {
    stream: &'a UnixStream,
    peer: &'a PeerProcess,
}

impl Client for SocketClient<'_> {
    fn chain(&self) -> Vec<String> {
        self.peer.chain().into_iter().map(|e| e.name).collect()
    }

    /// Looks without reading nor waiting (`poll`): end of file or error is a client that
    /// went away.
    fn exited(&self) -> bool {
        ward_platform::peer_closed(self.stream)
    }
}

fn serve_connection(
    mut stream: UnixStream,
    peer: PeerProcess,
    role: Role,
    daemon: &Daemon,
    slots: &Slots,
    id: u64,
) {
    let _ = stream.set_read_timeout(Some(READ_DEADLINE));
    let _ = stream.set_write_timeout(Some(WRITE_DEADLINE));
    let limit = frame::frame_limit(role);
    let write_lock = Arc::new(Mutex::new(()));
    // D65 (a): only the app role's connection receives the engine's notices, and it has
    // no read deadline (open wait).
    let mut subscribed = false;
    if role == Role::App
        && let Ok(copy) = stream.try_clone()
    {
        let _ = stream.set_read_timeout(None);
        daemon.engine().subscribe(
            id,
            Box::new(SocketSubscriber {
                stream: copy,
                write_lock: write_lock.clone(),
            }),
        );
        subscribed = true;
    }
    loop {
        slots.mark(id, true);
        let bytes = match frame::read_frame(&mut stream, limit) {
            Ok(b) => b,
            Err(FrameError::Closed) | Err(FrameError::Io(_)) | Err(FrameError::Truncated) => {
                break;
            }
            Err(e) => {
                debug!("quadro recusado ({}): {e}", role.name());
                let _ = answer(&mut stream, role, &Resposta::Erro(protocol::frame_error(e)));
                break;
            }
        };
        slots.mark(id, false);
        if role != Role::Agent && peer.reverify_no_debugger().is_err() {
            warning!(
                "cliente {} sob depurador ou encerrado: conexão fechada",
                role.name()
            );
            break;
        }
        let response = match protocol::decode_request(role, &bytes) {
            Ok(p) => {
                drop(bytes);
                let client = SocketClient {
                    stream: &stream,
                    peer: &peer,
                };
                // `ward run` (Phase 26): the session writes the responses; after a command
                // is launched, the connection ends. Refused before that, it goes on.
                if role == Role::Agent && matches!(p, Request::Run(_)) {
                    if crate::execution::session(daemon, &client, &stream, p) {
                        continue;
                    }
                    break;
                }
                // `ward ssh` (Phase 27): the same session shape.
                if role == Role::Agent && matches!(p, Request::Ssh { .. }) {
                    if crate::ssh::session(daemon, &client, &stream, p) {
                        continue;
                    }
                    break;
                }
                daemon.serve_from(&client, role, p)
            }
            Err(e) => {
                debug!("pedido recusado ({}): {}", role.name(), e.code());
                Resposta::Erro(e)
            }
        };
        let _g = write_lock.lock().unwrap_or_else(|e| e.into_inner());
        if answer(&mut stream, role, &response).is_err() {
            break;
        }
    }
    if subscribed {
        // Closing the window (or the app) denies what it received (S10).
        daemon.engine().unsubscribe(id);
    }
}

/// Accepts connections forever.
pub fn serve(listener: &Listener, daemon: Arc<Daemon>, roles: Arc<Roles>) -> io::Result<()> {
    let slots = Arc::new(Slots::default());
    loop {
        let stream = match listener.listener().accept() {
            Ok((s, _)) => s,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                warning!("erro ao aceitar conexão ({})", e.kind());
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
        };
        let peer = match ward_platform::verify_connection(&stream) {
            Ok(p) => p,
            Err(AcceptError::UidRefused { uid, pid }) => {
                // Closes without sending a single byte (V3.2).
                warning!("conexão recusada (UidRecusado): uid {uid}, pid {pid}");
                drop(stream);
                continue;
            }
            Err(AcceptError::NoIdentity(e)) => {
                warning!("conexão recusada (SemIdentidade): {}", e.kind());
                drop(stream);
                continue;
            }
            Err(AcceptError::Io(e)) => {
                warning!("conexão recusada (Io): {}", e.kind());
                drop(stream);
                continue;
            }
        };
        let role = roles.role(&peer);
        let Some((id, _)) = slots.reserve(&stream, role) else {
            let mut s = stream;
            let _ = s.set_write_timeout(Some(Duration::from_secs(1)));
            let _ = answer(&mut s, role, &Resposta::Erro(ResponseError::Busy));
            continue;
        };
        let daemon = daemon.clone();
        let slots_t = slots.clone();
        let r = std::thread::Builder::new()
            .name("wardd-conexao".into())
            .spawn(move || {
                // Releases the slot even if the thread panics.
                let _release = ReleaseOnDrop(slots_t.clone(), id);
                serve_connection(stream, peer, role, &daemon, &slots_t, id);
            });
        if r.is_err() {
            warning!("não deu para criar a thread da conexão");
            slots.release(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ward_core::entry::Environment;
    use ward_core::protocol::{Operation, PedidoDeAprovacao, PedidoDeDestrave};

    /// D63: without the group, the launcher's pid is not enough; with the group, only the
    /// launcher's pid. Holds in both builds (`make test-no-test-mode` runs without the
    /// feature).
    #[test]
    fn d63_without_the_group_the_launcher_pid_gives_no_app_role() {
        use ward_platform::Refusal;
        for reason in [
            Refusal::CommonGroup,
            Refusal::AppParentNotAllowed,
            Refusal::Debugger,
        ] {
            assert_eq!(
                role_from_class(PeerClass::Common { reason }, 42, |_| true),
                Role::Agent
            );
        }
        assert_eq!(role_from_class(PeerClass::App, 42, |_| false), Role::Agent);
        assert_eq!(role_from_class(PeerClass::App, 42, |p| p == 42), Role::App);
    }

    /// D63, through the socket: a child of this process (as the launcher would open it),
    /// without the egid of the `ward-app` group and without an install, stays an agent.
    /// Without the `test-mode` feature, `test_app` has no possible value at all; with it,
    /// without `WARD_DEV_APP` it is `None` and gives the same.
    #[test]
    fn d63_child_peer_without_egid_stays_agent() {
        use std::os::unix::fs::DirBuilderExt;
        let dir = std::path::PathBuf::from(format!(
            "/run/user/{}/wardd-server-d63-{}",
            ward_platform::effective_uid(),
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let path = dir.join("s.sock");
        let l = std::os::unix::net::UnixListener::bind(&path).unwrap();
        let mut child = std::process::Command::new("/usr/bin/python3")
            .args([
                "-I",
                "-c",
                "import socket,sys,time\ns=socket.socket(socket.AF_UNIX)\ns.connect(sys.argv[1])\ntime.sleep(10)",
            ])
            .arg(&path)
            .env_clear()
            .spawn()
            .unwrap();
        let (s, _) = l.accept().unwrap();
        let peer = ward_platform::verify_connection(&s).unwrap();
        assert_eq!(peer.credentials().pid, child.id());
        let roles = Roles {
            install: None,
            app: None,
            test_app: None,
            test_extension: None,
        };
        assert_eq!(roles.role(&peer), Role::Agent);
        // Phase 41, only in the test daemon: with the extension exception on and the
        // installed Chrome as an accepted parent, this peer (child of the test, not of
        // Chrome) stays an agent. Without the feature, the exception is not even built.
        let extension = TestExtension::from_chrome("/opt/google/chrome/chrome");
        if !cfg!(feature = "test-mode") {
            assert!(extension.is_none());
        }
        let roles = Roles {
            install: None,
            app: None,
            test_app: None,
            test_extension: extension,
        };
        assert_eq!(roles.role(&peer), Role::Agent);
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn alive(s: &UnixStream) -> bool {
        !ward_platform::peer_closed(s)
    }

    /// M3 scrutiny: the app's connection (subscriber, idle) is not evicted by 10 000
    /// silent agent connections; a full agent group does not take the app's slot; and a
    /// full app group does not evict an agent.
    #[test]
    fn attack_agent_connections_do_not_evict_the_app() {
        let slots = Slots::default();
        // The app connects first and stays silent (waiting for notices).
        let (app_d, app_c) = UnixStream::pair().unwrap();
        let (app_id, _) = slots.reserve(&app_d, Role::App).unwrap();
        // 10 000 silent agents: the old ones are evicted among themselves.
        let mut agents = Vec::new();
        for _ in 0..10_000 {
            let (d, c) = UnixStream::pair().unwrap();
            assert!(slots.reserve(&d, Role::Agent).is_some());
            agents.push(c);
            if agents.len() > MAX_CONNECTIONS * 2 {
                agents.remove(0);
            }
        }
        assert!(alive(&app_c), "a conexão do app foi derrubada");
        {
            let list = slots.list.lock().unwrap();
            assert!(list.iter().any(|v| v.id == app_id && v.app));
            assert_eq!(list.iter().filter(|v| !v.app).count(), MAX_CONNECTIONS);
        }
        // 64 busy agents (none idle): a new agent gets the busy error, the app still
        // connects.
        let ids: Vec<u64> = slots
            .list
            .lock()
            .unwrap()
            .iter()
            .filter(|v| !v.app)
            .map(|v| v.id)
            .collect();
        for id in &ids {
            slots.mark(*id, false);
        }
        let (d, _c) = UnixStream::pair().unwrap();
        assert!(slots.reserve(&d, Role::Agent).is_none());
        let (d, _c2) = UnixStream::pair().unwrap();
        assert!(slots.reserve(&d, Role::Extension).is_none());
        let mut apps = vec![(app_d, app_c)];
        for _ in 1..MAX_APP_CONNECTIONS {
            let (d, c) = UnixStream::pair().unwrap();
            assert!(slots.reserve(&d, Role::App).is_some());
            apps.push((d, c));
        }
        // Full app group: refuses and evicts nobody (neither app nor agent).
        let (d, _c3) = UnixStream::pair().unwrap();
        assert!(slots.reserve(&d, Role::App).is_none());
        for (_, c) in &apps {
            assert!(alive(c));
        }
        assert_eq!(
            slots.list.lock().unwrap().iter().filter(|v| !v.app).count(),
            MAX_CONNECTIONS
        );
        // Releasing an app slot makes room for another one.
        slots.release(app_id);
        let (d, _c4) = UnixStream::pair().unwrap();
        assert!(slots.reserve(&d, Role::App).is_some());
    }

    /// What the engine delivers to the app goes out as a whole response frame of the app
    /// role; with the connection closed, the delivery fails (the engine treats it as a
    /// closed window).
    #[test]
    fn subscriber_writes_a_whole_frame_and_fails_closed() {
        let (a, mut b) = UnixStream::pair().unwrap();
        let mut s = SocketSubscriber {
            stream: a,
            write_lock: Arc::new(Mutex::new(())),
        };
        let p = PedidoDeAprovacao {
            id: 3,
            entradas: vec!["t-aprov".into()],
            ambiente: Environment::Prod,
            operacao: Operation::Run,
            comando: "\"true\"".into(),
            diretorio: "\"/\"".into(),
            cadeia: vec!["claude".into()],
            origem: String::new(),
            confirmacao: false,
            entrada_oculta: true,
        };
        assert!(s.deliver(&Resposta::PedidoDeAprovacao(p.clone())));
        let d = PedidoDeDestrave {
            entrada: "t-aprov".into(),
            operacao: Operation::Run,
            comando: "\"true\"".into(),
            restante: 120,
        };
        assert!(s.deliver(&Resposta::PedidoDeDestrave(d.clone())));
        let q = frame::read_frame(&mut b, frame::RESPONSE_LIMIT).unwrap();
        match protocol::decode_response(Role::App, &q).unwrap() {
            Resposta::PedidoDeAprovacao(read) => assert_eq!(read, p),
            r => panic!("{r:?}"),
        }
        let q = frame::read_frame(&mut b, frame::RESPONSE_LIMIT).unwrap();
        match protocol::decode_response(Role::App, &q).unwrap() {
            Resposta::PedidoDeDestrave(read) => assert_eq!(read, d),
            r => panic!("{r:?}"),
        }
        // The agent does not read that frame.
        assert!(protocol::decode_response(Role::Agent, &q).is_err());
        drop(b);
        // Another test running in parallel that does `fork` holds a copy of the fd until
        // the `exec` (CLOEXEC): the failure comes right after.
        let failed = (0..200).any(|_| {
            let f = !s.deliver(&Resposta::PedidoDeDestrave(d.clone()));
            if !f {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            f
        });
        assert!(failed);
    }
}
