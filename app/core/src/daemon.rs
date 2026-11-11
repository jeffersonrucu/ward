//! Client of the `wardd` socket (Rust side of the app; the webview never talks to the
//! socket).
//!
//! Before sending any byte, the app checks the server with the same rule as the CLI
//! (`ward_platform::connect_server`, S5, D46): the socket is
//! `/run/ward/<uid>/wardd.sock`, every folder of the path belongs to root, with no write
//! for group and others and no link, the socket belongs to the uid itself and is 0600,
//! and the peer's `SO_PEERCRED` has uid 0 (whoever did the `listen` was the system
//! systemd). Only in the test build (`test-mode`, D7) the socket is the test daemon's,
//! in its folder under `/run/user/<uid>/` (the user's 0700 folder and a peer with the same
//! uid); without the feature, that path and that exception do not even exist in the
//! binary, and no environment variable picks the path.
//!
//! The protocol (Phase 20) lives in [`crate::client`]: there the request becomes the
//! frame body and the response is read in the app role. This module is only the
//! transport.
//!
//! Transport (D65 a, Phase 23a): frame write and frame read on the single
//! connection.
//! - A reader thread is the only one that reads the socket. What arrives is read in the
//!   app role (the strict response decoder) and sorted:
//!   - the approval request and the unlock request are **notices** and go to the
//!     [`NoticeQueue`] (at most [`MAX_NOTICES`]), never in place of a response. With the
//!     queue full, the new notice is dropped with a log line without content (the
//!     agent's request expires and is denied in the daemon, S10);
//!   - any other type is the response of whoever asked;
//!   - a response with no request in flight, a frame above the response frame limit or a frame
//!     that does not decode in the app role (including an agent role type) **close the
//!     connection**: the app does not try to guess what the daemon meant.
//! - One request at a time, under a mutex: the protocol has no request id and the daemon
//!   answers in order. The response has [`RESPONSE_TIMEOUT`] (unlock with the D20 wait
//!   plus Argon2, D62); without it, the connection closes (a late response would be a
//!   "response with no request").
//! - The frames stay in `Zeroizing` (the copy brings a secret) and the limit is the one
//!   of `ward_core::limits`.
//!
//! Single connection (D47, Phase 29a): the setgid app connects **once**, before GTK,
//! with the egid of the `ward-app` group (the role comes from it), and then drops the
//! group (the startup module). A new connection after that would have the real gid and
//! fall into the agent role; that is why the startup's [`DaemonSocket`] (and the test
//! bridge's) never reconnects: without the connection, the requests answer
//! [`AppError::ConnectionLost`] and the app executes itself again. The
//! [`DaemonSocket::standard`] without [`DaemonSocket::connect_now`] (tests only)
//! connects on first use and, if the connection drops, on the next use.

use std::collections::VecDeque;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use ward_core::protocol::frame::{self, FrameError};
use ward_core::protocol::{self, ApprovalRequest, Response, Role, UnlockRequest};
use ward_platform::{ExpectedServer, ServerRefusal, connect_server};

use zeroize::Zeroizing;

use crate::AppError;

/// Socket name (the same as `wardd`'s).
pub const SOCKET: &str = "wardd.sock";

/// Timeout of each write to the socket.
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout of a request's response (D65 a): unlock waits up to 2 min for the D20 lock
/// and still runs Argon2 (D62).
pub const RESPONSE_TIMEOUT: Duration = Duration::from_secs(130);

/// Largest number of notices waiting in the queue (D65 a).
pub const MAX_NOTICES: usize = 16;

/// Vault status as the interface sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VaultStatus {
    NoVault,
    Locked,
    Unlocked,
}

impl VaultStatus {
    pub fn code(self) -> &'static str {
        match self {
            VaultStatus::NoVault => "sem-cofre",
            VaultStatus::Locked => "travado",
            VaultStatus::Unlocked => "destravado",
        }
    }
}

/// Transport to the daemon. Receives the body of a request already encoded in the app
/// role and returns the response body, not read yet (the reader is [`crate::client`],
/// with the protocol's strict reading). The tests and the test bridge swap the socket
/// for the [`crate::fake_daemon`].
pub trait Daemon: Send {
    fn exchange(&mut self, request: &[u8]) -> Result<Zeroizing<Vec<u8>>, AppError>;
}

/// What the daemon sends with no request, only on the app role connection (D65 a).
/// Neither has a secret: the request text (command, chain, entry) is displayed by
/// Phases 36 and 39a with D51.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    /// An agent use waits for approval (Phase 36).
    Approval(ApprovalRequest),
    /// An agent use waits for the unlock (Phases 31 and 39a).
    Unlock(UnlockRequest),
}

impl Notice {
    /// Tests and test bridge only: an unlock request with fake data (D15).
    #[cfg(any(test, feature = "test-mode"))]
    pub fn test_unlock(entry: &str) -> Notice {
        Notice::Unlock(UnlockRequest {
            entry: entry.to_owned(),
            operation: protocol::Operation::Run,
            command: r#""psql" "-c" "select count(*) from pedidos""#.to_owned(),
            remaining: 120,
        })
    }

    /// Tests and test bridge only (Phase 39a, D51): an unlock request with a raw entry
    /// and command, like a daemon that did not format or a forged frame: `ESC`, bidi,
    /// zero width, quotes and an HTML tag. The screen must show everything as a visible
    /// escape and text, never as HTML.
    #[cfg(any(test, feature = "test-mode"))]
    pub fn hostile_test_unlock() -> Notice {
        Notice::Unlock(UnlockRequest {
            entry: "acme-pg-\u{202E}vorpa\u{200B}<b>x</b>".to_owned(),
            operation: protocol::Operation::Run,
            command: "\"psql\" \"-c\" \"select 1\u{1b}[2K\" \"<img src=x onerror=alert(1)>\""
                .to_owned(),
            remaining: 120,
        })
    }
}

#[derive(Default)]
struct Queue {
    items: VecDeque<Notice>,
    dropped: u64,
}

/// Bounded queue of the notices (D65 a). The reader thread pushes; the unlock (Phase 31)
/// and approval (Phase 36) screens take.
#[derive(Default)]
pub struct NoticeQueue {
    queue: Mutex<Queue>,
    arrived: Condvar,
}

impl NoticeQueue {
    fn lock(&self) -> MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Pushes to the end. Full: drops the new notice, with a log without content.
    fn push(&self, notice: Notice) {
        let mut q = self.lock();
        if q.items.len() >= MAX_NOTICES {
            q.dropped = q.dropped.saturating_add(1);
            drop(q);
            drop(notice);
            eprintln!("ward-app: aviso do daemon descartado (fila cheia)");
            return;
        }
        q.items.push_back(notice);
        drop(q);
        self.arrived.notify_all();
    }

    /// The oldest notice, without waiting.
    pub fn take(&self) -> Option<Notice> {
        self.lock().items.pop_front()
    }

    /// The oldest notice, waiting up to `timeout`.
    pub fn wait(&self, timeout: Duration) -> Option<Notice> {
        let end = Instant::now() + timeout;
        let mut q = self.lock();
        loop {
            if let Some(a) = q.items.pop_front() {
                return Some(a);
            }
            let left = end.checked_duration_since(Instant::now())?;
            q = self
                .arrived
                .wait_timeout(q, left)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }

    /// Phase 31: waits up to `timeout` for an unlock request and takes **all** of those
    /// in the queue (several notices in a row are a single request on the screen). The
    /// approval notices stay where they are, in order, for the approval window (Phase
    /// 36). Phase 39a: returns the newest (the daemon sends the newest last); `None`:
    /// none arrived in time.
    pub fn wait_unlock(&self, timeout: Duration) -> Option<UnlockRequest> {
        let end = Instant::now() + timeout;
        let mut q = self.lock();
        loop {
            let mut last = None;
            q.items.retain(|a| match a {
                Notice::Unlock(p) => {
                    last = Some(p.clone());
                    false
                }
                Notice::Approval(_) => true,
            });
            if last.is_some() {
                return last;
            }
            let left = end.checked_duration_since(Instant::now())?;
            q = self
                .arrived
                .wait_timeout(q, left)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }

    /// Phase 36: waits up to `timeout` for the oldest approval request and takes it out
    /// of the queue. The unlock requests stay where they are, in order, for the unlock
    /// screen (Phase 31). `None`: none arrived in time.
    pub fn wait_approval(&self, timeout: Duration) -> Option<ApprovalRequest> {
        let end = Instant::now() + timeout;
        let mut q = self.lock();
        loop {
            if let Some(i) = q
                .items
                .iter()
                .position(|a| matches!(a, Notice::Approval(_)))
                && let Some(Notice::Approval(p)) = q.items.remove(i)
            {
                return Some(p);
            }
            let left = end.checked_duration_since(Instant::now())?;
            q = self
                .arrived
                .wait_timeout(q, left)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }

    /// Tests and test bridge only (the locked-with-request fake daemon mode): pushes a
    /// notice as if it came from the daemon.
    #[cfg(any(test, feature = "test-mode"))]
    pub fn simulate(&self, notice: Notice) {
        self.push(notice);
    }

    /// Notices waiting.
    pub fn size(&self) -> usize {
        self.lock().items.len()
    }

    /// Notices dropped with the queue full, since the start.
    pub fn dropped(&self) -> u64 {
        self.lock().dropped
    }
}

/// The asking side: the writer and the channel through which the reader thread delivers
/// the response.
struct Requester {
    writer: UnixStream,
    responses: Receiver<Zeroizing<Vec<u8>>>,
}

/// An open connection with its reader thread.
struct Transport {
    /// The connection (the startup checks `CLOEXEC` and watches the close on it).
    stream: UnixStream,
    /// One request at a time.
    requester: Mutex<Requester>,
    /// A request is waiting for a response (the reader only delivers if there is one).
    waiting: Arc<AtomicBool>,
    /// `false` after the connection closed (by the daemon or by a refused frame).
    open: Arc<AtomicBool>,
}

impl Transport {
    /// Attaches the reader thread to the already checked connection.
    fn new(stream: UnixStream, notices: Arc<NoticeQueue>) -> Result<Transport, AppError> {
        let lost = |_| AppError::ConnectionLost;
        // The reader waits with no timeout (notices arrive at any time); the response
        // timeout is the channel's.
        stream.set_read_timeout(None).map_err(lost)?;
        stream
            .set_write_timeout(Some(WRITE_TIMEOUT))
            .map_err(lost)?;
        let reader = stream.try_clone().map_err(lost)?;
        let writer = stream.try_clone().map_err(lost)?;
        let (tx, rx) = mpsc::sync_channel(1);
        let waiting = Arc::new(AtomicBool::new(false));
        let open = Arc::new(AtomicBool::new(true));
        let (w, o) = (waiting.clone(), open.clone());
        std::thread::Builder::new()
            .name("ward-app-leitora".into())
            .spawn(move || read_loop(reader, &tx, &w, &o, &notices))
            .map_err(lost)?;
        Ok(Transport {
            stream,
            requester: Mutex::new(Requester {
                writer,
                responses: rx,
            }),
            waiting,
            open,
        })
    }

    fn is_open(&self) -> bool {
        self.open.load(Ordering::Acquire)
    }

    fn close(&self) {
        self.open.store(false, Ordering::Release);
        let _ = self.stream.shutdown(Shutdown::Both);
    }

    /// One request and its response, with `timeout`.
    fn exchange(&self, request: &[u8], timeout: Duration) -> Result<Zeroizing<Vec<u8>>, AppError> {
        let mut p = self
            .requester
            .lock()
            .map_err(|_| AppError::ConnectionLost)?;
        if !self.is_open() {
            return Err(AppError::ConnectionLost);
        }
        // A leftover response cannot become this request's (the reader only delivers
        // with `waiting`, so this should find nothing).
        while p.responses.try_recv().is_ok() {}
        self.waiting.store(true, Ordering::Release);
        let limit = frame::frame_limit(Role::App);
        match frame::write_frame(&mut p.writer, request, limit) {
            Ok(()) => {}
            Err(FrameError::Empty | FrameError::TooLarge) => {
                // Nothing was written: the connection is still in order.
                self.waiting.store(false, Ordering::Release);
                return Err(AppError::InvalidRequest);
            }
            Err(_) => {
                self.close();
                return Err(AppError::ConnectionLost);
            }
        }
        match p.responses.recv_timeout(timeout) {
            Ok(r) => Ok(r),
            Err(_) => {
                // No response in time (or the reader closed): a late response would be
                // read as another request's.
                self.waiting.store(false, Ordering::Release);
                self.close();
                Err(AppError::ConnectionLost)
            }
        }
    }
}

impl Drop for Transport {
    /// The reader holds a copy of the fd: without the `shutdown`, the connection would
    /// stay open.
    fn drop(&mut self) {
        self.close();
    }
}

/// The reader thread: the only one that reads the socket. Leaves (and closes the
/// connection) at the end of the connection, on a refused frame or on a response with no
/// request.
fn read_loop(
    mut s: UnixStream,
    tx: &SyncSender<Zeroizing<Vec<u8>>>,
    waiting: &AtomicBool,
    open: &AtomicBool,
    notices: &NoticeQueue,
) {
    // Closed, truncated, above the limit or I/O error: leaves.
    while let Ok(q) = frame::read_frame(&mut s, frame::RESPONSE_LIMIT) {
        match protocol::decode_response(Role::App, &q) {
            Ok(Response::ApprovalRequest(p)) => notices.push(Notice::Approval(p)),
            Ok(Response::UnlockRequest(p)) => notices.push(Notice::Unlock(p)),
            Ok(_) => {
                // Response with no request: leaves.
                if !waiting.swap(false, Ordering::AcqRel) || tx.send(q).is_err() {
                    break;
                }
            }
            // Type of another role or forged frame.
            Err(_) => break,
        }
    }
    open.store(false, Ordering::Release);
    let _ = s.shutdown(Shutdown::Both);
}

/// Real daemon, over the local socket.
pub struct DaemonSocket {
    expected: ExpectedServer,
    transport: Option<Transport>,
    /// `false` after [`connect_now`](Self::connect_now): the connection is not redone in
    /// this process.
    can_connect: bool,
    notices: Arc<NoticeQueue>,
    response_timeout: Duration,
}

impl DaemonSocket {
    /// The installed daemon: `/run/ward/<uid>/wardd.sock`, checked as a socket of the
    /// system systemd. In the test build, the test daemon in
    /// its folder under `/run/user/<uid>/`. The uid is the process's effective one,
    /// not `XDG_RUNTIME_DIR` nor another variable.
    ///
    /// Only in `test-mode` (Phase 39c), `WARD_DEV_SOCKET_DIR` changes the test socket
    /// folder, as in the test CLI: the test daemon passes its folder to the app and to
    /// the bridge it opens (from the daemon's config, never from the request), and so a
    /// test daemon in another folder does not share the fixed socket with another
    /// checkout. The checks do not change (0700 folder of the uid, no link, peer with the
    /// same uid), and the app role still depends on the pid the launcher opened (D63).
    pub fn standard() -> Result<DaemonSocket, AppError> {
        #[cfg(not(feature = "test-mode"))]
        let expected = ExpectedServer::system();
        #[cfg(feature = "test-mode")]
        let expected = match std::env::var_os("WARD_DEV_SOCKET_DIR") {
            Some(d) => ExpectedServer::test_at(
                std::path::PathBuf::from(d).join(ward_platform::socket::SOCKET_NAME),
            ),
            None => ExpectedServer::test(),
        };
        Ok(DaemonSocket::unconnected(expected))
    }

    fn unconnected(expected: ExpectedServer) -> DaemonSocket {
        DaemonSocket {
            expected,
            transport: None,
            can_connect: true,
            notices: Arc::new(NoticeQueue::default()),
            response_timeout: RESPONSE_TIMEOUT,
        }
    }

    /// Connects now (with the full server check) and locks the connection: from here on
    /// this `DaemonSocket` never connects again. It is what the app startup calls before
    /// dropping the group (D47), and what the test bridge calls when it starts.
    pub fn connect_now(&mut self) -> Result<&UnixStream, AppError> {
        self.can_connect = false;
        self.transport = None;
        let t = self.connect()?;
        Ok(&self.transport.insert(t).stream)
    }

    /// In a thread: waits for the open connection to close (by the daemon, or by the app
    /// after a refused frame), reading nothing, and calls `on_close`. The test bridge
    /// leaves through here when the test daemon stops.
    pub fn watch_close(&self, on_close: impl FnOnce() + Send + 'static) -> Result<(), AppError> {
        let t = self.transport.as_ref().ok_or(AppError::ConnectionLost)?;
        let s = t.stream.try_clone().map_err(|_| AppError::ConnectionLost)?;
        std::thread::Builder::new()
            .name("ward-app-vigia".into())
            .spawn(move || {
                let _ = ward_platform::app::wait_for_close(&s);
                on_close();
            })
            .map(|_| ())
            .map_err(|_| AppError::ConnectionLost)
    }

    /// The notices the daemon sends with no request (D65 a). The same queue holds for the
    /// whole life of this `DaemonSocket`.
    pub fn notices(&self) -> Arc<NoticeQueue> {
        self.notices.clone()
    }

    /// The open connection. Without it: connects on first use, except after
    /// [`connect_now`](Self::connect_now), when it answers [`AppError::ConnectionLost`].
    fn transport(&mut self) -> Result<&Transport, AppError> {
        if self.transport.as_ref().is_some_and(|t| !t.is_open()) {
            self.transport = None;
        }
        if self.transport.is_none() {
            if !self.can_connect {
                return Err(AppError::ConnectionLost);
            }
            let t = self.connect()?;
            self.transport = Some(t);
        }
        self.transport.as_ref().ok_or(AppError::ConnectionLost)
    }

    /// Tests only: the already open connection `s` (a pair with the fake daemon), like
    /// the startup's (no reconnect).
    #[cfg(test)]
    pub(crate) fn from_pair(s: UnixStream, timeout: Duration) -> DaemonSocket {
        let mut d = DaemonSocket::system_at(PathBuf::from("/nao-usado/wardd.sock"));
        d.can_connect = false;
        d.response_timeout = timeout;
        d.transport = Some(Transport::new(s, d.notices.clone()).expect("transporte"));
        d
    }

    /// Another path, with the installed daemon's rules (root folder, uid 0 peer).
    pub fn system_at(path: PathBuf) -> DaemonSocket {
        DaemonSocket::unconnected(ExpectedServer::system_at(path))
    }

    /// Another path, with the test daemon's rules. Only in `test-mode`.
    #[cfg(feature = "test-mode")]
    pub fn test_at(path: PathBuf) -> DaemonSocket {
        DaemonSocket::unconnected(ExpectedServer::test_at(path))
    }

    pub fn path(&self) -> &Path {
        self.expected.path()
    }

    /// Checks the path, connects and checks the peer before any byte, and starts the
    /// reader thread. Nothing is written to the socket here.
    fn connect(&self) -> Result<Transport, AppError> {
        let s = connect_server(&self.expected).map_err(|r| match r {
            ServerRefusal::Missing | ServerRefusal::Io(_) => AppError::DaemonUnavailable,
            _ => AppError::DaemonUntrusted,
        })?;
        Transport::new(s, self.notices.clone())
    }
}

impl Daemon for DaemonSocket {
    fn exchange(&mut self, request: &[u8]) -> Result<Zeroizing<Vec<u8>>, AppError> {
        let timeout = self.response_timeout;
        self.transport()?.exchange(request, timeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::{Read, Write};
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt, symlink};
    use std::os::unix::net::UnixListener;

    use crate::client;
    use crate::commands::Reference;
    use ward_core::protocol::Target;

    impl DaemonSocket {
        fn status(&mut self) -> Result<VaultStatus, AppError> {
            client::status(self)
        }
        fn value_to_copy(&mut self, r: &Reference) -> Result<(), AppError> {
            client::copy(self, r.entry(), Target::Field(r.field().to_owned())).map(|_| ())
        }
    }

    fn my_uid() -> u32 {
        fs::metadata("/proc/self").unwrap().uid()
    }

    /// Folder of this test only in `/run/user/<uid>/` (tmpfs, the user's 0700; `/tmp` is
    /// 1777 and would pass no check at all). Removed on drop.
    struct TempBase(PathBuf);

    impl TempBase {
        fn new(name: &str) -> TempBase {
            let p = PathBuf::from(format!(
                "/run/user/{}/ward-app-core-{}-{name}",
                my_uid(),
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&p);
            fs::DirBuilder::new().mode(0o700).create(&p).unwrap();
            TempBase(p)
        }
    }

    impl Drop for TempBase {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Fake server of the same uid: 0600 socket in `dir`.
    fn fake_server(dir: &Path) -> UnixListener {
        let l = UnixListener::bind(dir.join(SOCKET)).unwrap();
        fs::set_permissions(dir.join(SOCKET), fs::Permissions::from_mode(0o600)).unwrap();
        l.set_nonblocking(true).unwrap();
        l
    }

    /// No byte reached the fake server (accepts whatever there is and reads without
    /// blocking).
    fn nothing_received(l: &UnixListener) {
        while let Ok((mut c, _)) = l.accept() {
            c.set_nonblocking(true).unwrap();
            let mut buf = [0u8; 16];
            assert!(
                !matches!(c.read(&mut buf), Ok(n) if n > 0),
                "o app mandou bytes a um servidor recusado"
            );
        }
    }

    #[test]
    fn default_path_without_environment_variable() {
        let d = DaemonSocket::standard().unwrap();
        #[cfg(not(feature = "test-mode"))]
        let expected = format!("/run/ward/{}/wardd.sock", my_uid());
        #[cfg(feature = "test-mode")]
        let expected = format!("/run/user/{}/ward-teste/wardd.sock", my_uid());
        assert_eq!(d.path(), Path::new(&expected));
        #[cfg(not(feature = "test-mode"))]
        assert!(!d.path().to_string_lossy().contains("ward-teste"));
    }

    #[test]
    fn missing_socket_is_unavailable() {
        let mut d =
            DaemonSocket::system_at(PathBuf::from("/run/ward-nao-existe-falso/1/wardd.sock"));
        assert_eq!(d.status(), Err(AppError::DaemonUnavailable));
        #[cfg(feature = "test-mode")]
        {
            let base = TempBase::new("ausente");
            let dir = base.0.join("ward-teste");
            fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
            let mut t = DaemonSocket::test_at(dir.join(SOCKET));
            assert_eq!(t.status(), Err(AppError::DaemonUnavailable));
        }
    }

    #[test]
    fn attack_fake_server_of_the_same_uid_is_refused_without_receiving_anything() {
        // V3.23: server of the same uid, 0600 socket, the user's 0700 folder. By the
        // installed daemon's rule, the folder does not belong to root and the peer is
        // not uid 0.
        let base = TempBase::new("falso");
        let dir = base.0.join("ward");
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let l = fake_server(&dir);
        let mut d = DaemonSocket::system_at(dir.join(SOCKET));
        assert_eq!(d.status(), Err(AppError::DaemonUntrusted));
        let r = Reference::new("acme-pg-dev", "senha").unwrap();
        assert!(matches!(
            d.value_to_copy(&r),
            Err(AppError::DaemonUntrusted)
        ));
        nothing_received(&l);
    }

    #[test]
    fn attack_without_the_startup_connection_does_not_reconnect() {
        // D47: after dropping the group, a new connection would have the real gid
        // (agent role). It failed at startup: no request connects afterwards.
        let base = TempBase::new("partida");
        let dir = base.0.join("ward");
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let mut d = DaemonSocket::system_at(dir.join(SOCKET));
        assert!(d.connect_now().is_err());
        let l = fake_server(&dir);
        assert_eq!(d.status(), Err(AppError::ConnectionLost));
        let r = Reference::new("acme-pg-dev", "senha").unwrap();
        assert!(matches!(d.value_to_copy(&r), Err(AppError::ConnectionLost)));
        assert!(l.accept().is_err(), "reconectou depois da partida");
        #[cfg(feature = "test-mode")]
        {
            // With the startup connection open, the requests use the same one, without
            // connecting again.
            let mut t = DaemonSocket::test_at(dir.join(SOCKET));
            let c = t.connect_now().unwrap();
            assert!(ward_platform::app::has_cloexec(c).unwrap());
            let (mut c, _) = l.accept().unwrap();
            let server = std::thread::spawn(move || {
                for _ in 0..2 {
                    read_request(&mut c);
                    send(&mut c, &app_status());
                }
                c
            });
            assert_eq!(t.status(), Ok(VaultStatus::Unlocked));
            assert_eq!(t.status(), Ok(VaultStatus::Unlocked));
            let _c = server.join().unwrap();
            assert!(l.accept().is_err(), "conectou de novo");
        }
    }

    #[test]
    fn attack_directory_open_to_others_is_refused() {
        let base = TempBase::new("aberto");
        let dir = base.0.join("ward");
        fs::DirBuilder::new().mode(0o755).create(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
        let l = fake_server(&dir);
        let mut d = DaemonSocket::system_at(dir.join(SOCKET));
        assert_eq!(d.status(), Err(AppError::DaemonUntrusted));
        #[cfg(feature = "test-mode")]
        {
            // Not even the test build accepts an open folder; with 0700 it is the test
            // daemon.
            let mut t = DaemonSocket::test_at(dir.join(SOCKET));
            assert_eq!(t.status(), Err(AppError::DaemonUntrusted));
            nothing_received(&l);
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
            t.response_timeout = Duration::from_millis(100);
            assert_eq!(t.status(), Err(AppError::ConnectionLost));
            let _ = l.accept();
        }
        nothing_received(&l);
    }

    #[test]
    fn attack_directory_through_symlink_is_refused() {
        let base = TempBase::new("link");
        let real = base.0.join("real");
        fs::DirBuilder::new().mode(0o700).create(&real).unwrap();
        let l = fake_server(&real);
        symlink(&real, base.0.join("ward")).unwrap();
        let mut d = DaemonSocket::system_at(base.0.join("ward").join(SOCKET));
        assert_eq!(d.status(), Err(AppError::DaemonUntrusted));
        #[cfg(feature = "test-mode")]
        {
            let mut t = DaemonSocket::test_at(base.0.join("ward").join(SOCKET));
            assert_eq!(t.status(), Err(AppError::DaemonUntrusted));
        }
        nothing_received(&l);
    }

    #[test]
    fn attack_regular_file_in_place_of_the_socket_is_refused() {
        let base = TempBase::new("arquivo");
        let dir = base.0.join("ward");
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        fs::write(dir.join(SOCKET), b"").unwrap();
        fs::set_permissions(dir.join(SOCKET), fs::Permissions::from_mode(0o600)).unwrap();
        let mut d = DaemonSocket::system_at(dir.join(SOCKET));
        assert_eq!(d.status(), Err(AppError::DaemonUntrusted));
        #[cfg(feature = "test-mode")]
        {
            let mut t = DaemonSocket::test_at(dir.join(SOCKET));
            assert_eq!(t.status(), Err(AppError::DaemonUntrusted));
        }
    }

    #[test]
    fn attack_socket_open_to_others_is_refused() {
        let base = TempBase::new("socket");
        let dir = base.0.join("ward");
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let l = fake_server(&dir);
        fs::set_permissions(dir.join(SOCKET), fs::Permissions::from_mode(0o666)).unwrap();
        let mut d = DaemonSocket::system_at(dir.join(SOCKET));
        assert_eq!(d.status(), Err(AppError::DaemonUntrusted));
        #[cfg(feature = "test-mode")]
        {
            let mut t = DaemonSocket::test_at(dir.join(SOCKET));
            assert_eq!(t.status(), Err(AppError::DaemonUntrusted));
        }
        nothing_received(&l);
    }

    // --- Transport (D65 a, V5.33) with a fake daemon on the other side of a pair -------

    fn send(s: &mut UnixStream, r: &Response) {
        let q = protocol::encode_response(Role::App, r).unwrap();
        frame::write_frame(s, &q, frame::RESPONSE_LIMIT).unwrap();
    }

    fn read_request(s: &mut UnixStream) -> ward_core::protocol::Request {
        let q = frame::read_frame(s, frame::frame_limit(Role::App)).unwrap();
        protocol::decode_request(Role::App, &q).unwrap()
    }

    fn app_status() -> Response {
        Response::Status {
            locked: false,
            vault_exists: true,
            role: Role::App,
        }
    }

    /// Phase 31: the unlock watcher takes only the unlock requests; the approval ones
    /// stay in order for Phase 36. With no unlock, waits the timeout and says `None`.
    #[test]
    fn wait_unlock_leaves_the_approval_in_the_queue() {
        let f = NoticeQueue::default();
        assert_eq!(f.wait_unlock(Duration::from_millis(30)), None);
        f.simulate(Notice::Approval(approval(1)));
        f.simulate(Notice::test_unlock("acme-pg-velho"));
        f.simulate(Notice::Approval(approval(2)));
        f.simulate(Notice::test_unlock("acme-pg-novo"));
        // Phase 39a: the newest.
        assert_eq!(
            f.wait_unlock(Duration::from_millis(30)).map(|p| p.entry),
            Some("acme-pg-novo".to_owned())
        );
        assert_eq!(f.wait_unlock(Duration::from_millis(30)), None);
        assert_eq!(f.take(), Some(Notice::Approval(approval(1))));
        assert_eq!(f.take(), Some(Notice::Approval(approval(2))));
        assert_eq!(f.take(), None);
        // Arrives from another thread while it waits.
        let f = Arc::new(f);
        let g = f.clone();
        let t = std::thread::spawn(move || g.wait_unlock(Duration::from_secs(10)));
        std::thread::sleep(Duration::from_millis(50));
        f.simulate(Notice::test_unlock("acme-pg-dev"));
        assert!(t.join().unwrap().is_some());
    }

    fn unlock_notice() -> Response {
        match Notice::test_unlock("acme-pg-aprovacao") {
            Notice::Unlock(p) => Response::UnlockRequest(p),
            Notice::Approval(_) => unreachable!(),
        }
    }

    fn approval(id: u64) -> ApprovalRequest {
        ApprovalRequest {
            id,
            entries: vec!["acme-pg-dev".into()],
            environment: ward_core::entry::Environment::Dev,
            operation: ward_core::protocol::Operation::Run,
            command: "\"psql\"".into(),
            directory: "\"/home/dev\"".into(),
            chain: vec!["claude".into(), "ward run".into()],
            origin: String::new(),
            confirmation: false,
            hidden_entry: false,
        }
    }

    /// Did the fake daemon close on its side? (the app closes the connection without
    /// sending anything)
    fn closed(s: &mut UnixStream) -> bool {
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut b = [0u8; 1];
        matches!(s.read(&mut b), Ok(0))
    }

    #[test]
    fn v5_33_notices_in_the_middle_go_to_the_queue_and_the_response_to_the_requester() {
        let (a, mut b) = UnixStream::pair().unwrap();
        let mut d = DaemonSocket::from_pair(a, Duration::from_secs(10));
        let notices = d.notices();
        let server = std::thread::spawn(move || {
            for _ in 0..2 {
                assert!(matches!(
                    read_request(&mut b),
                    ward_core::protocol::Request::Status
                ));
                send(&mut b, &Response::ApprovalRequest(approval(7)));
                send(&mut b, &unlock_notice());
                send(&mut b, &app_status());
            }
            b
        });
        assert_eq!(client::status(&mut d), Ok(VaultStatus::Unlocked));
        assert_eq!(client::role(&mut d), Ok(Role::App));
        let _b = server.join().unwrap();
        assert_eq!(notices.take(), Some(Notice::Approval(approval(7))));
        assert_eq!(
            notices.take(),
            Some(Notice::test_unlock("acme-pg-aprovacao"))
        );
        assert_eq!(notices.take(), Some(Notice::Approval(approval(7))));
        assert_eq!(
            notices.wait(Duration::from_millis(50)),
            Some(Notice::test_unlock("acme-pg-aprovacao"))
        );
        assert_eq!(notices.take(), None);
        assert_eq!(notices.dropped(), 0);
    }

    #[test]
    fn v5_33_a_thousand_notices_the_queue_stays_at_16_and_the_app_keeps_answering() {
        let (a, mut b) = UnixStream::pair().unwrap();
        let mut d = DaemonSocket::from_pair(a, Duration::from_secs(30));
        let notices = d.notices();
        let server = std::thread::spawn(move || {
            for i in 0..1000 {
                if i % 2 == 0 {
                    send(&mut b, &Response::ApprovalRequest(approval(i)));
                } else {
                    send(&mut b, &unlock_notice());
                }
            }
            for _ in 0..2 {
                read_request(&mut b);
                send(&mut b, &app_status());
            }
            b
        });
        assert_eq!(client::status(&mut d), Ok(VaultStatus::Unlocked));
        assert_eq!(client::status(&mut d), Ok(VaultStatus::Unlocked));
        let _b = server.join().unwrap();
        assert_eq!(notices.size(), MAX_NOTICES);
        assert_eq!(notices.dropped(), 1000 - 16);
        // The first 16 stay, in order.
        assert_eq!(notices.take(), Some(Notice::Approval(approval(0))));
        assert_eq!(
            notices.take(),
            Some(Notice::test_unlock("acme-pg-aprovacao"))
        );
    }

    /// Phase 36: the approval watcher takes only the approval requests, in order; the
    /// unlock ones stay for Phase 31. With no approval, waits the timeout and says
    /// `None`.
    #[test]
    fn wait_approval_leaves_the_unlock_in_the_queue() {
        let f = NoticeQueue::default();
        assert_eq!(f.wait_approval(Duration::from_millis(30)), None);
        f.push(Notice::test_unlock("acme-pg-aprovacao"));
        f.push(Notice::Approval(approval(1)));
        f.push(Notice::test_unlock("acme-pg-aprovacao"));
        f.push(Notice::Approval(approval(2)));
        assert_eq!(
            f.wait_approval(Duration::from_millis(30)),
            Some(approval(1))
        );
        assert_eq!(
            f.wait_approval(Duration::from_millis(30)),
            Some(approval(2))
        );
        assert_eq!(f.wait_approval(Duration::from_millis(30)), None);
        assert_eq!(f.take(), Some(Notice::test_unlock("acme-pg-aprovacao")));
        assert_eq!(f.take(), Some(Notice::test_unlock("acme-pg-aprovacao")));
        assert_eq!(f.take(), None);
        // Arrives from another thread while it waits.
        let f = Arc::new(f);
        let g = Arc::clone(&f);
        let t = std::thread::spawn(move || g.wait_approval(Duration::from_secs(10)));
        std::thread::sleep(Duration::from_millis(50));
        f.push(Notice::Approval(approval(3)));
        assert_eq!(t.join().unwrap(), Some(approval(3)));
    }

    #[test]
    fn v5_33_response_with_no_request_closes_the_connection() {
        let (a, mut b) = UnixStream::pair().unwrap();
        let mut d = DaemonSocket::from_pair(a, Duration::from_secs(10));
        send(&mut b, &app_status());
        assert!(closed(&mut b), "o app não fechou a conexão");
        assert_eq!(client::status(&mut d), Err(AppError::ConnectionLost));
        assert_eq!(d.notices().size(), 0);
    }

    #[test]
    fn v5_33_frame_above_the_limit_closes_the_connection() {
        let (a, mut b) = UnixStream::pair().unwrap();
        let mut d = DaemonSocket::from_pair(a, Duration::from_secs(10));
        let server = std::thread::spawn(move || {
            read_request(&mut b);
            let n = u32::try_from(frame::RESPONSE_LIMIT + 1).unwrap();
            b.write_all(&n.to_be_bytes()).unwrap();
            let _ = b.write_all(&[b'{'; 64]);
            closed(&mut b)
        });
        assert_eq!(client::status(&mut d), Err(AppError::ConnectionLost));
        assert!(server.join().unwrap(), "o app não fechou a conexão");
        assert_eq!(client::status(&mut d), Err(AppError::ConnectionLost));
    }

    #[test]
    fn v5_33_agent_role_type_or_forged_closes_without_showing_anything() {
        use ward_core::entry::{EntryType, Environment};
        use ward_core::protocol::ListItem;
        // The agent list comes without a level (D50): not a response the app accepts.
        let item = ListItem {
            name: "acme-pg-dev".into(),
            kind: EntryType::Database,
            group: "acme".into(),
            environment: Environment::Dev,
            project: None,
            tags: vec![],
            level: None,
            address: None,
            summary: None,
        };
        let agent_list = protocol::encode_response(Role::Agent, &Response::List(vec![item]))
            .unwrap()
            .to_vec();
        let forged = br#"{"v":1,"tipo":"estado","dados":{"travado":false,"cofre_existe":true,"papel":"app","x":1}}"#.to_vec();
        for body in [agent_list, forged] {
            let (a, mut b) = UnixStream::pair().unwrap();
            let mut d = DaemonSocket::from_pair(a, Duration::from_secs(10));
            let server = std::thread::spawn(move || {
                read_request(&mut b);
                frame::write_frame(&mut b, &body, frame::RESPONSE_LIMIT).unwrap();
                closed(&mut b)
            });
            assert_eq!(client::list(&mut d).err(), Some(AppError::ConnectionLost));
            assert!(server.join().unwrap(), "o app não fechou a conexão");
        }
    }

    #[test]
    fn no_response_in_time_closes_and_the_late_one_does_not_become_another() {
        let (a, mut b) = UnixStream::pair().unwrap();
        let mut d = DaemonSocket::from_pair(a, Duration::from_millis(200));
        let server = std::thread::spawn(move || {
            read_request(&mut b);
            std::thread::sleep(Duration::from_millis(400));
            // The late response: the connection already closed on the app side.
            let _ = frame::write_frame(
                &mut b,
                &protocol::encode_response(Role::App, &app_status()).unwrap(),
                frame::RESPONSE_LIMIT,
            );
            closed(&mut b)
        });
        assert_eq!(client::status(&mut d), Err(AppError::ConnectionLost));
        assert!(server.join().unwrap());
        assert_eq!(client::status(&mut d), Err(AppError::ConnectionLost));
    }

    #[test]
    fn request_too_large_does_not_go_out_and_the_connection_stays() {
        let (a, mut b) = UnixStream::pair().unwrap();
        let mut d = DaemonSocket::from_pair(a, Duration::from_secs(10));
        let large = vec![b'x'; frame::frame_limit(Role::App) + 1];
        assert_eq!(d.exchange(&large).err(), Some(AppError::InvalidRequest));
        let server = std::thread::spawn(move || {
            read_request(&mut b);
            send(&mut b, &app_status());
            b
        });
        assert_eq!(client::status(&mut d), Ok(VaultStatus::Unlocked));
        server.join().unwrap();
    }
}
