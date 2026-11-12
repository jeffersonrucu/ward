//! Temporary SSH agent of `ward ssh` with a key (D12, Phase 27; S1, S2, S8, S13).
//!
//! - **Only that connection:** socket at `<rt>ssh-<random>/agent.sock`, in a 0700 dir
//!   created for the session; the child `ssh` gets it through `IdentityAgent` (D13).
//! - **Only the child `ssh`:** each connection goes through `SO_PEERCRED` (same uid) and
//!   the peer's pid must be the session child's. In `sftp`, the agent is **held to the
//!   first accepted peer** whose parent is the child `sftp` (the `ssh` that `sftp`
//!   creates, which connects before any command: with `BatchMode=yes`, there is no
//!   session without the agent) and, from then on, only that exact pid passes. A `!cmd`
//!   of `sftp` (another child of it: `ssh-add -L`, `ssh` to another host with the key) is
//!   refused (M4 audit). Any other process (an `ssh-add` of the same user) has the
//!   connection closed without a single byte.
//! - **Only lists and signs:** `REQUEST_IDENTITIES` returns only the entry's key and
//!   `SIGN_REQUEST` signs only with it. Add, remove, lock, unlock and the extensions
//!   answer `FAILURE`.
//! - **The private key never leaves the daemon:** it is read from the entry (OpenSSH,
//!   with the passphrase through RustCrypto's `bcrypt-pbkdf`) and signs with
//!   `ed25519-dalek`. Ed25519 only (RSA would come with RUSTSEC-2023-0071). No file with
//!   the private key is written: the dir has only the socket and the **public** key
//!   (`id.pub`), which `ssh` uses to pick the agent's key with `IdentitiesOnly=yes`.
//! - **Goes away:** socket, public key and dir go away when the session ends (the `ssh`
//!   ends, fails or is killed, or the CLI goes away) and when the vault locks (the core's
//!   lock count changes); the decrypted key is zeroed on drop.
//! - The log has only the message type and the pid (S13).

use std::fs;
use std::io::{self, Read, Write};
use std::os::fd::AsFd;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use ssh_key::encoding::Encode;
use ssh_key::{Algorithm, PrivateKey, SigningKey};
use zeroize::Zeroizing;

use ward_core::entry::Secret;
use ward_platform::{AcceptError, Listener, SocketDir};

use crate::{debug, warning};

/// Name of the socket inside the session's dir.
pub const SOCKET_NAME: &str = "agent.sock";
/// Name of the public key inside the session's dir.
pub const PUBLIC_NAME: &str = "id.pub";
/// Prefix of the session's dir in `<rt>`.
pub const DIR_PREFIX: &str = "ssh-";
/// Largest accepted message (the one of OpenSSH's `ssh-agent`).
const MAX_MESSAGE: usize = 256 * 1024;
/// `ssh` connections served at the same time.
const MAX_CONNECTIONS: usize = 4;
/// How often the threads check whether they must stop (lock, end of the session).
const STEP_MS: i32 = 100;
/// Deadline to read a message already started.
const MESSAGE_DEADLINE: Duration = Duration::from_secs(10);

// Numbers of the agent protocol (draft-miller-ssh-agent).
const FAILURE: u8 = 5;
const REQUEST_IDENTITIES: u8 = 11;
const IDENTITIES_ANSWER: u8 = 12;
const SIGN_REQUEST: u8 = 13;
const SIGN_RESPONSE: u8 = 14;

/// Why the agent did not start. No value (may go to the log).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentError {
    /// The entry's key is not a readable OpenSSH key.
    InvalidKey,
    /// The key has a passphrase and it is not in the entry, or it does not match.
    Passphrase,
    /// Algorithm the agent does not sign (Ed25519 only).
    UnsupportedAlgorithm,
    /// Dir, socket or public key were not created.
    System,
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            AgentError::InvalidKey => "a chave privada da entrada não é uma chave OpenSSH",
            AgentError::Passphrase => "a passphrase da chave falta ou não confere",
            AgentError::UnsupportedAlgorithm => "só chaves Ed25519 são aceitas pelo agente",
            AgentError::System => "não deu para criar o socket do agente",
        })
    }
}

/// The decrypted key and what goes to `ssh` (the public blob).
struct Key {
    private: PrivateKey,
    blob: Vec<u8>,
}

impl Key {
    fn read(text: &Secret, passphrase: Option<&Secret>) -> Result<Key, AgentError> {
        let text = std::str::from_utf8(text.expose()).map_err(|_| AgentError::InvalidKey)?;
        let read = PrivateKey::from_openssh(text).map_err(|_| AgentError::InvalidKey)?;
        let private = if read.is_encrypted() {
            let p = passphrase.ok_or(AgentError::Passphrase)?;
            read.decrypt(p.expose())
                .map_err(|_| AgentError::Passphrase)?
        } else {
            read
        };
        if private.algorithm() != Algorithm::Ed25519 {
            return Err(AgentError::UnsupportedAlgorithm);
        }
        let blob = private
            .public_key()
            .key_data()
            .encode_vec()
            .map_err(|_| AgentError::InvalidKey)?;
        Ok(Key { private, blob })
    }

    /// The public key as text (`ssh-ed25519 AAAA… ward`), without the entry's comment.
    fn public_as_text(&self) -> Result<String, AgentError> {
        let mut p = self.private.public_key().clone();
        p.set_comment("ward");
        p.to_openssh().map_err(|_| AgentError::InvalidKey)
    }

    fn sign(&self, data: &[u8]) -> Option<Vec<u8>> {
        sign_with(&self.private, data)?.encode_vec().ok()
    }
}

/// Signs through the library (the `Signer` of `ssh-key`, Ed25519 from `ed25519-dalek`).
fn sign_with<S: SigningKey>(key: &S, data: &[u8]) -> Option<ssh_key::Signature> {
    key.try_sign(data).ok()
}

/// Who may use the agent: the pid of the session's child.
#[derive(Debug, Clone, Copy)]
pub struct Allowed {
    pub pid: u32,
    /// `sftp`: the `ssh` is a grandchild of the daemon; the process whose parent is the
    /// child is the one that counts.
    pub child_of_child: bool,
}

/// Agent created (dir, socket and public key ready), waiting for the child's pid. On
/// drop (without having been started, or at the end of the thread), the key is zeroed
/// and the dir goes away.
pub struct SshAgent {
    path: PathBuf,
    dir: Option<SocketDir>,
    listener: Option<Listener>,
    key: Option<Arc<Key>>,
}

impl Drop for SshAgent {
    fn drop(&mut self) {
        // The socket goes away with the `Listener`; the decrypted key is zeroed on drop.
        self.listener.take();
        self.key.take();
        self.dir.take();
        delete_dir(&self.path);
    }
}

impl std::fmt::Debug for SshAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SshAgent(••••)")
    }
}

/// Deletes the whole session dir (without following links).
fn delete_dir(dir: &Path) {
    if let Err(e) = fs::remove_dir_all(dir)
        && e.kind() != io::ErrorKind::NotFound
    {
        warning!(
            "agente SSH: não deu para apagar a pasta da sessão ({})",
            e.kind()
        );
    }
}

/// Daemon startup (Phase 28g, 2nd M4 audit): deletes the `<rt>ssh-*/` dirs a dead
/// `wardd` (SIGKILL) left behind. They have only the agent's socket and the public key,
/// but must not pile up. What has the prefix and is not a dir (link, file) also goes
/// away, without following links (`remove_dir_all` and `remove_file` do not follow).
/// `<rt>` is verified first (0700 of the user, no link). Returns how many entries it
/// deleted.
pub fn clean_dirs(rt: &Path) -> io::Result<usize> {
    ward_platform::prepare_dir(rt).map_err(|_| io::Error::from(io::ErrorKind::PermissionDenied))?;
    let mut n = 0;
    for item in fs::read_dir(rt)? {
        let item = item?;
        if !item
            .file_name()
            .as_encoded_bytes()
            .starts_with(DIR_PREFIX.as_bytes())
        {
            continue;
        }
        let path = item.path();
        if item.file_type()?.is_dir() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
        n += 1;
    }
    Ok(n)
}

impl SshAgent {
    /// Reads the key and creates `<rt>ssh-<random>/` (0700) with `agent.sock` (0600) and
    /// `id.pub`. The key is verified before any file exists.
    pub fn create(
        rt: &Path,
        key: &Secret,
        passphrase: Option<&Secret>,
    ) -> Result<SshAgent, AgentError> {
        let key = Key::read(key, passphrase)?;
        let public = key.public_as_text()?;
        ward_platform::prepare_dir(rt).map_err(|_| AgentError::System)?;
        let suffix = ward_core::generator::connect_token().map_err(|_| AgentError::System)?;
        let suffix = std::str::from_utf8(&suffix.expose()[..16]).map_err(|_| AgentError::System)?;
        let path = rt.join(format!("{DIR_PREFIX}{suffix}"));
        // New dir: never reuses one that already exists.
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .map_err(|_| AgentError::System)?;
        let build = || -> Result<(SocketDir, Listener), AgentError> {
            let dir = ward_platform::prepare_dir(&path).map_err(|_| AgentError::System)?;
            let listener = dir.listen(SOCKET_NAME).map_err(|_| AgentError::System)?;
            listener
                .listener()
                .set_nonblocking(true)
                .map_err(|_| AgentError::System)?;
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(path.join(PUBLIC_NAME))
                .map_err(|_| AgentError::System)?;
            f.write_all(public.as_bytes())
                .and_then(|()| f.write_all(b"\n"))
                .map_err(|_| AgentError::System)?;
            Ok((dir, listener))
        };
        match build() {
            Ok((dir, listener)) => Ok(SshAgent {
                path,
                dir: Some(dir),
                listener: Some(listener),
                key: Some(Arc::new(key)),
            }),
            Err(e) => {
                delete_dir(&path);
                Err(e)
            }
        }
    }

    /// Path of the socket (goes to `ssh` through `IdentityAgent`).
    pub fn socket(&self) -> PathBuf {
        self.path.join(SOCKET_NAME)
    }

    /// Path of the public key (goes to `ssh` through `IdentityFile`, with
    /// `IdentitiesOnly=yes`).
    pub fn public_key(&self) -> PathBuf {
        self.path.join(PUBLIC_NAME)
    }

    /// Starts serving only `allowed`, until the [`Running`] is dropped or the core's lock
    /// count changes (`locks` with the value `initial_lock` from when the session was
    /// authorized).
    pub fn start(self, allowed: Allowed, locks: Arc<AtomicU64>, initial_lock: u64) -> Running {
        let stop = Arc::new(AtomicBool::new(false));
        let path = self.path.clone();
        let p = stop.clone();
        let thread = std::thread::Builder::new()
            .name("wardd-agente-ssh".into())
            .spawn(move || serve_loop(self, allowed, &p, &locks, initial_lock));
        if thread.is_err() {
            warning!("agente SSH: a thread não foi criada");
            delete_dir(&path);
        }
        Running {
            stop,
            thread: thread.ok(),
            path,
        }
    }
}

/// The agent serving. On drop, stops the threads and deletes the session's dir.
pub struct Running {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    path: PathBuf,
}

impl std::fmt::Debug for Running {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Running(agente SSH)")
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        delete_dir(&self.path);
    }
}

/// Must stop: the session ended or the vault locked since the session began.
fn should_stop(stop: &AtomicBool, locks: &AtomicU64, initial: u64) -> bool {
    stop.load(Ordering::Acquire) || locks.load(Ordering::Acquire) != initial
}

/// Is the peer the session's `ssh`? In `sftp`, `held` keeps the pid of the first accepted
/// child of `sftp`; after it, only that pid passes (no other child of `sftp`).
fn peer_allowed(peer: &ward_platform::PeerProcess, p: Allowed, held: &mut Option<u32>) -> bool {
    peer_allowed_pid(peer.credentials().pid, || peer.parent_pid(), p, held)
}

/// [`peer_allowed`] with the peer's pid and its parent already read (testable without a
/// process).
fn peer_allowed_pid(
    pid: u32,
    parent: impl FnOnce() -> Option<u32>,
    p: Allowed,
    held: &mut Option<u32>,
) -> bool {
    if !p.child_of_child {
        return pid == p.pid;
    }
    match *held {
        Some(fixed) => pid == fixed,
        None => {
            if pid != p.pid && parent() == Some(p.pid) {
                *held = Some(pid);
                true
            } else {
                false
            }
        }
    }
}

fn serve_loop(
    mut agent: SshAgent,
    allowed: Allowed,
    stop: &AtomicBool,
    locks: &AtomicU64,
    initial: u64,
) {
    let (Some(listener), Some(key)) = (agent.listener.take(), agent.key.take()) else {
        return;
    };
    let open = Arc::new(AtomicUsize::new(0));
    let stop_connections = Arc::new(AtomicBool::new(false));
    let mut connections = Vec::new();
    let mut held = None;
    while !should_stop(stop, locks, initial) {
        match ward_platform::child::ready_to_read_fd(listener.listener().as_fd(), STEP_MS) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(_) => break,
        }
        let stream = match listener.listener().accept() {
            Ok((s, _)) => s,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        let peer = match ward_platform::verify_connection(&stream) {
            Ok(p) => p,
            Err(AcceptError::UidRefused { uid, pid }) => {
                warning!("agente SSH: conexão recusada (uid {uid}, pid {pid}) (D12)");
                continue;
            }
            Err(_) => {
                warning!("agente SSH: conexão recusada (sem identidade) (D12)");
                continue;
            }
        };
        let pid = peer.credentials().pid;
        if !peer_allowed(&peer, allowed, &mut held) {
            // Closes without a single byte: only the session's `ssh` uses the agent.
            warning!("agente SSH: conexão do pid {pid} recusada; só o ssh da sessão (D12)");
            continue;
        }
        if open.load(Ordering::Acquire) >= MAX_CONNECTIONS {
            warning!("agente SSH: conexões demais do pid {pid}; fechada");
            continue;
        }
        debug!("agente SSH: conexão do pid {pid}");
        open.fetch_add(1, Ordering::AcqRel);
        let (c, a, pc) = (key.clone(), open.clone(), stop_connections.clone());
        let t = std::thread::Builder::new()
            .name("wardd-agente-con".into())
            .spawn(move || {
                serve(stream, &c, &pc);
                a.fetch_sub(1, Ordering::AcqRel);
            });
        match t {
            Ok(t) => connections.push(t),
            Err(_) => {
                open.fetch_sub(1, Ordering::AcqRel);
            }
        }
    }
    if locks.load(Ordering::Acquire) != initial {
        warning!("agente SSH: o cofre travou; agente derrubado (D12)");
    }
    stop_connections.store(true, Ordering::Release);
    for t in connections {
        let _ = t.join();
    }
    // The decrypted key is zeroed on drop; the socket goes away with the `Listener`, and
    // the dir, in the agent's `Drop`.
    drop(key);
    drop(listener);
    drop(agent);
}

/// Reads exactly `buf.len()` bytes, checking `stop` between the waits. `false`: end,
/// error, deadline or stop.
fn read_exact(s: &mut UnixStream, buf: &mut [u8], stop: &AtomicBool, deadline: bool) -> bool {
    let mut read = 0;
    let limit = std::time::Instant::now() + MESSAGE_DEADLINE;
    while read < buf.len() {
        if stop.load(Ordering::Acquire) || (deadline && std::time::Instant::now() > limit) {
            return false;
        }
        match ward_platform::child::ready_to_read_fd(s.as_fd(), STEP_MS) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(_) => return false,
        }
        match s.read(&mut buf[read..]) {
            Ok(0) => return false,
            Ok(n) => read += n,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
                ) => {}
            Err(_) => return false,
        }
    }
    true
}

/// Serves a connection of the session's `ssh` until it closes or the agent stops.
fn serve(mut s: UnixStream, key: &Key, stop: &AtomicBool) {
    let _ = s.set_nonblocking(false);
    let _ = s.set_write_timeout(Some(MESSAGE_DEADLINE));
    loop {
        let mut n = [0u8; 4];
        if !read_exact(&mut s, &mut n, stop, false) {
            return;
        }
        let size = u32::from_be_bytes(n) as usize;
        if size == 0 || size > MAX_MESSAGE {
            warning!("agente SSH: mensagem de tamanho inválido; conexão fechada");
            return;
        }
        let mut msg = Zeroizing::new(vec![0u8; size]);
        if !read_exact(&mut s, &mut msg, stop, true) {
            return;
        }
        let response = answer(&msg, key);
        let mut frame = Zeroizing::new(Vec::with_capacity(response.len() + 4));
        frame.extend_from_slice(&u32::try_from(response.len()).unwrap_or(0).to_be_bytes());
        frame.extend_from_slice(&response);
        if s.write_all(&frame).is_err() {
            return;
        }
    }
}

/// `string` of the SSH protocol: u32 + bytes.
fn read_string<'a>(b: &mut &'a [u8]) -> Option<&'a [u8]> {
    let (n, rest) = b.split_first_chunk::<4>()?;
    let n = u32::from_be_bytes(*n) as usize;
    if rest.len() < n {
        return None;
    }
    let (s, rest) = rest.split_at(n);
    *b = rest;
    Some(s)
}

fn put_string(v: &mut Vec<u8>, s: &[u8]) {
    v.extend_from_slice(&u32::try_from(s.len()).unwrap_or(0).to_be_bytes());
    v.extend_from_slice(s);
}

/// The answer to an agent message (without the size). Only lists and signs (D12).
pub(crate) fn answer_message(
    msg: &[u8],
    blob: &[u8],
    sign: &dyn Fn(&[u8]) -> Option<Vec<u8>>,
) -> Vec<u8> {
    let Some((&kind, mut body)) = msg.split_first() else {
        return vec![FAILURE];
    };
    match kind {
        REQUEST_IDENTITIES if body.is_empty() => {
            debug!("agente SSH: lista de chaves");
            let mut r = vec![IDENTITIES_ANSWER];
            r.extend_from_slice(&1u32.to_be_bytes());
            put_string(&mut r, blob);
            put_string(&mut r, b"ward");
            r
        }
        SIGN_REQUEST => {
            let (Some(key), Some(data)) = (read_string(&mut body), read_string(&mut body)) else {
                return vec![FAILURE];
            };
            // flags (u32): Ed25519 does not use them.
            if body.len() != 4 || key != blob {
                return vec![FAILURE];
            }
            match sign(data) {
                Some(signature) => {
                    debug!("agente SSH: assinatura feita");
                    let mut r = vec![SIGN_RESPONSE];
                    put_string(&mut r, &signature);
                    r
                }
                None => vec![FAILURE],
            }
        }
        other => {
            // Add, remove, lock, unlock, extensions: refused (D12).
            warning!("agente SSH: mensagem {other} recusada (só lista e assina, D12)");
            vec![FAILURE]
        }
    }
}

fn answer(msg: &[u8], key: &Key) -> Vec<u8> {
    answer_message(msg, &key.blob, &|d| key.sign(d))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ident(blob: &[u8]) -> Vec<u8> {
        answer_message(&[REQUEST_IDENTITIES], blob, &|_| Some(b"sig".to_vec()))
    }

    /// Phase 28g: at startup, the `ssh-*` dirs a dead `wardd` left go away; an `ssh-*`
    /// that is a link goes away without its target being touched; the rest of `<rt>`
    /// stays.
    #[test]
    fn f28g_startup_deletes_leftover_ssh_dirs() {
        use std::os::unix::fs::PermissionsExt;
        let base = ward_platform::socket::runtime_dir()
            .join(format!("wardd-agente-28g-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let rt = base.join("rt");
        let outside = base.join("fora");
        fs::create_dir_all(&rt).unwrap();
        fs::create_dir_all(&outside).unwrap();
        for d in [&base, &rt, &outside] {
            fs::set_permissions(d, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let leftover = rt.join(format!("{DIR_PREFIX}abcdefghijklmnop"));
        fs::create_dir(&leftover).unwrap();
        fs::write(leftover.join(PUBLIC_NAME), b"ssh-ed25519 AAAA falso\n").unwrap();
        std::os::unix::net::UnixListener::bind(leftover.join(SOCKET_NAME)).unwrap();
        fs::write(outside.join("alvo"), b"nao-apagar").unwrap();
        std::os::unix::fs::symlink(&outside, rt.join(format!("{DIR_PREFIX}link"))).unwrap();
        fs::write(rt.join("wardd.sock-outro"), b"").unwrap();
        fs::create_dir(rt.join("tmp")).unwrap();
        assert_eq!(clean_dirs(&rt).unwrap(), 2);
        assert!(!leftover.exists());
        assert!(fs::symlink_metadata(rt.join(format!("{DIR_PREFIX}link"))).is_err());
        assert_eq!(fs::read(outside.join("alvo")).unwrap(), b"nao-apagar");
        assert!(rt.join("wardd.sock-outro").exists() && rt.join("tmp").is_dir());
        assert_eq!(clean_dirs(&rt).unwrap(), 0);
        // Open `<rt>`: refuses without deleting anything.
        fs::create_dir(&leftover).unwrap();
        fs::set_permissions(&rt, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(clean_dirs(&rt).is_err());
        assert!(leftover.exists());
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn attack_sftp_bang_other_child_of_sftp_is_refused() {
        // M4 audit: the `!cmd` of `sftp` is another child of it. Held to the first one.
        let sftp = 500;
        let p = Allowed {
            pid: sftp,
            child_of_child: true,
        };
        let mut held = None;
        // Neither `sftp` itself nor a stranger holds the agent.
        assert!(!peer_allowed_pid(sftp, || Some(1), p, &mut held));
        assert!(!peer_allowed_pid(77, || Some(1), p, &mut held));
        assert_eq!(held, None);
        // The `ssh` that `sftp` created connects first and holds it.
        assert!(peer_allowed_pid(501, || Some(sftp), p, &mut held));
        assert_eq!(held, Some(501));
        assert!(peer_allowed_pid(501, || Some(sftp), p, &mut held));
        // `!ssh-add -L`, `!exec ssh …`: another child of `sftp`, refused.
        assert!(!peer_allowed_pid(502, || Some(sftp), p, &mut held));
        assert!(!peer_allowed_pid(503, || Some(sftp), p, &mut held));
        assert_eq!(held, Some(501));
        // Without `sftp`: only the child's exact pid.
        let q = Allowed {
            pid: 600,
            child_of_child: false,
        };
        let mut none = None;
        assert!(peer_allowed_pid(600, || None, q, &mut none));
        assert!(!peer_allowed_pid(601, || Some(600), q, &mut none));
        assert_eq!(none, None);
    }

    #[test]
    fn only_lists_and_signs() {
        let blob = b"blob-publico";
        let r = ident(blob);
        assert_eq!(r[0], IDENTITIES_ANSWER);
        assert_eq!(&r[1..5], &1u32.to_be_bytes());
        // Signature only with the entry's key.
        let mut request = vec![SIGN_REQUEST];
        put_string(&mut request, blob);
        put_string(&mut request, b"dados");
        request.extend_from_slice(&0u32.to_be_bytes());
        let r = answer_message(&request, blob, &|d| {
            assert_eq!(d, b"dados");
            Some(b"sig".to_vec())
        });
        assert_eq!(r[0], SIGN_RESPONSE);
        let mut other = vec![SIGN_REQUEST];
        put_string(&mut other, b"outra-chave");
        put_string(&mut other, b"dados");
        other.extend_from_slice(&0u32.to_be_bytes());
        assert_eq!(
            answer_message(&other, blob, &|_| panic!("não assina")),
            vec![FAILURE]
        );
    }

    #[test]
    fn attack_add_remove_lock_and_garbage_refused() {
        let blob = b"blob";
        // 17 add, 18 remove, 19 remove all, 22 lock, 23 unlock, 25 add constrained,
        // 27 extension, 9 (v1), empty and truncated message.
        for m in [
            &[17u8, 0, 0, 0, 3, b'x', b'y', b'z'][..],
            &[18, 0, 0, 0, 4, b'b', b'l', b'o', b'b'],
            &[19],
            &[22, 0, 0, 0, 1, b'p'],
            &[23, 0, 0, 0, 1, b'p'],
            &[25],
            &[27, 0, 0, 0, 1, b'e'],
            &[9],
            &[],
            &[SIGN_REQUEST, 0, 0, 0, 9, b'b'],
            &[REQUEST_IDENTITIES, 1],
        ] {
            assert_eq!(
                answer_message(m, blob, &|_| panic!("não assina")),
                vec![FAILURE],
                "{m:?}"
            );
        }
    }
}
