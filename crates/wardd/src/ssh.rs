//! `ward ssh` (Phase 27; D3, D12, D13, D34, D43, D56; S1–S4, S8, S13).
//!
//! - **Who called** (D3): the CLI is always the agent; the entry's level counts. The
//!   exception is the app's "Connect" token ([`Daemon::issue_token`]): random, single
//!   use, 30 s, bound to the entry. A token valid for the entry skips the level (never
//!   the blocked one: the entry stays invisible); an invalid, expired or other-entry one
//!   is ignored and the level counts. Every presented token that exists dies there, even
//!   when used on the wrong entry.
//! - **No shell** (S3, D43): the daemon runs `ssh` (or `sshpass` → `ssh`) through
//!   `execve`, with the argv built here: the user only as the own argument of
//!   `-l <user>`, the host after `--`, never `user@host` nor `-o User=`. In `sftp`, where
//!   `-l` is the bandwidth limit, the user goes as `-o` + `User=<user>` (D56), with
//!   `-F /dev/null` and the host after `--`.
//! - **Nothing from `~/.ssh`** (D13, R19): `-F /dev/null`, `IdentitiesOnly=yes`, no
//!   default identities (`IdentityFile`/`CertificateFile`), Ward's own `known_hosts`
//!   (`StrictHostKeyChecking=accept-new`: the first connection writes the host's key; a
//!   different key later refuses) and the child's environment built from scratch (no
//!   `SSH_AUTH_SOCK` of the user nor anything from the daemon).
//! - **Password**: `sshpass -e`, the password only in `SSHPASS` in the child's
//!   environment (never in argv), with `PubkeyAuthentication=no` and
//!   `IdentityAgent=none`.
//! - **Key**: the temporary agent of D12 ([`crate::ssh_agent`]), only for the child
//!   `ssh`; the private key never leaves the daemon.
//! - **Lock** (Phase 54c, S8): the open session dies when the vault locks, like
//!   `ward run`: `SIGKILL` on the group of `ssh`/`sshpass` and on the one of the `ssh`
//!   that `sshpass` creates with its own session; the CLI exits 137 with the notice. The
//!   agent of D12 and the jump's bridge also go down.
//! - **End of the session** (Phase 54l): the CLI that goes away (closed, SIGKILL), the
//!   frame outside the session and the end of the child use the same
//!   `kill_with_child_sessions` as the lock: no `ssh` with its own session is left
//!   behind waiting for the `SIGHUP` of the `sshpass` pty.
//! - **Interactive** (no command and with a window): the child runs in a pty; the output
//!   goes through the mask piece by piece, without holding or delaying anything (D34
//!   revised: holding the prefix became an oracle). With a command (or without a
//!   terminal), output and error in separate streams, as in `ward run`.
//! - **Jump** (Phase 28, D19): no `ProxyJump` (which puts an `sh -c` in the chain and does
//!   not pass the `-o` options to the jump's `ssh`). The daemon opens the bridge of the
//!   bridge module (an `ssh -W <host>:<port>` to the jump entry, through `execve`, only
//!   for the main `ssh`) and the main `ssh` connects to `127.0.0.1:<bridge port>` with
//!   `HostKeyAlias` equal to the destination's name in `known_hosts`. The mask covers
//!   both entries.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ward_core::entry::{AccessLevel, Auth, Data, Entry, Protocol, Secret};
use ward_core::mask::{Mask, MaskBuilder};
use ward_core::protocol::{Request, Response, ResponseError, Role};
use ward_platform::child::Command;
use zeroize::Zeroizing;

use crate::approval::Client;
use crate::bridge::{Bridge, Intermediate, JUMP_LIMITS, Opening};
use crate::core::Daemon;
use crate::execution::{DiesOnLock, Preparation, PreparedAgent};
use crate::ssh_agent::{AgentError, SshAgent};
use crate::{debug, warning};

/// Validity of the "Connect" token (D3).
pub const TOKEN_DEADLINE: Duration = Duration::from_secs(30);
/// Unused tokens kept at the same time (the oldest goes away).
const MAX_TOKENS: usize = 16;
/// The child's `PATH` (the programs go by absolute path; this is for `ssh` to find what
/// it calls itself, nothing from the session).
const CHILD_PATH: &[u8] = b"/usr/bin:/bin";
/// `TERM` of the pty (the protocol does not bring the CLI's terminal).
const TERM_PTY: &[u8] = b"xterm-256color";

/// The programs of `ward ssh`, by absolute path.
#[derive(Debug, Clone)]
pub struct SshPrograms {
    pub ssh: PathBuf,
    pub sshpass: PathBuf,
    pub sftp: PathBuf,
}

impl SshPrograms {
    /// The system's (D13: `sshpass` is a system dependency of the installation).
    pub fn from_system() -> Self {
        SshPrograms {
            ssh: PathBuf::from("/usr/bin/ssh"),
            sshpass: PathBuf::from("/usr/bin/sshpass"),
            sftp: PathBuf::from("/usr/bin/sftp"),
        }
    }
}

/// A "Connect" token issued and not used yet.
pub(crate) struct ConnectToken {
    value: Secret,
    entry: String,
    until: Duration,
}

// --- Argv (S3, D13, D43, D56) ----------------------------------------------------------

/// Where to connect (values already validated by the core; verified again here).
#[derive(Debug, Clone, Copy)]
pub struct Destination<'a> {
    pub host: &'a str,
    pub port: u16,
    pub username: &'a str,
}

/// How to authenticate.
#[derive(Debug, Clone, Copy)]
pub enum SshAuth<'a> {
    /// `sshpass -e`; the password goes in `SSHPASS`, never here.
    Password,
    /// Agent of D12: the socket and the public key (for `IdentitiesOnly=yes`).
    Key {
        agent: &'a Path,
        public_key: &'a Path,
    },
}

/// Which program.
#[derive(Debug, Clone, Copy)]
pub enum Program<'a> {
    /// `ssh`, with the remote command (empty: shell).
    Ssh { command: &'a [Zeroizing<Vec<u8>>] },
    /// `sftp` (the `ftp/sftp` entry with the `sftp` protocol).
    Sftp,
    /// `ssh -W <host>:<port>`: the bridge of the jump and of the tunnel (Phase 28), with
    /// the standard input and output tied to the bridge's connection.
    Forward { host: &'a str, port: u16 },
}

/// The destination's name in `known_hosts` on a direct connection (`host` on 22,
/// `[host]:port` on the others), for the `HostKeyAlias` of the `ssh` that connects
/// through the jump's bridge.
pub fn destination_alias(host: &str, port: u16) -> Option<String> {
    if !simple_host(host) || port == 0 {
        return None;
    }
    Some(if port == 22 {
        host.to_owned()
    } else {
        format!("[{host}]:{port}")
    })
}

/// Verified alias: what [`destination_alias`] produces, nothing else.
fn alias_ok(a: &str) -> bool {
    if let Some(rest) = a.strip_prefix('[') {
        let Some((h, p)) = rest.split_once("]:") else {
            return false;
        };
        return simple_host(h) && p.parse::<u16>().is_ok_and(|p| p != 0) && !p.starts_with('0');
    }
    simple_host(a)
}

/// User of the server and FTP/SFTP entries (D43): `[A-Za-z0-9._@-]`, no `-` at the start.
fn username_ok(u: &str) -> bool {
    !u.is_empty()
        && !u.starts_with('-')
        && u.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'@' | b'-'))
}

/// Host (D43): the core's, without `@` and without `-` at the start.
fn host_ok(h: &str) -> bool {
    !h.is_empty()
        && !h.starts_with('-')
        && h.bytes().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_' | b':' | b'[' | b']')
        })
}

/// Host of the bridge's target and of the alias: the one of [`host_ok`], without
/// brackets (they are put here, when the host is IPv6).
fn simple_host(h: &str) -> bool {
    host_ok(h) && !h.contains(['[', ']'])
}

fn text(p: &Path) -> Option<&str> {
    p.to_str()
        .filter(|s| !s.contains(['\n', '\0', ',', ' ', '%', '"', '\'']))
}

/// The child's whole argv (`argv[0]` is the program). `None` if some value does not pass
/// the verification (nothing is executed). No shell: each item goes as one argument.
pub fn build_argv(
    progs: &SshPrograms,
    known_hosts: &Path,
    destination: Destination<'_>,
    auth: SshAuth<'_>,
    program: Program<'_>,
) -> Option<Vec<Vec<u8>>> {
    build_argv_with_alias(progs, known_hosts, destination, auth, program, None)
}

/// Like [`build_argv`], with the `HostKeyAlias` of the `ssh` that connects through the
/// jump's bridge (the destination is `127.0.0.1:<bridge port>`; the verified key is the
/// real host's).
pub fn build_argv_with_alias(
    progs: &SshPrograms,
    known_hosts: &Path,
    destination: Destination<'_>,
    auth: SshAuth<'_>,
    program: Program<'_>,
    alias: Option<&str>,
) -> Option<Vec<Vec<u8>>> {
    if !username_ok(destination.username) || !host_ok(destination.host) || destination.port == 0 {
        return None;
    }
    if let Program::Forward { host, port } = program
        && (!simple_host(host) || port == 0)
    {
        return None;
    }
    if alias.is_some_and(|a| !alias_ok(a)) {
        return None;
    }
    let kh = text(known_hosts)?;
    let mut a = Argv(Vec::new());
    let prog = match program {
        Program::Ssh { .. } | Program::Forward { .. } => &progs.ssh,
        Program::Sftp => &progs.sftp,
    };
    if let SshAuth::Password = auth {
        a.s(text(&progs.sshpass)?);
        a.s("-e");
    }
    a.s(text(prog)?);
    // Nothing from the user's nor the system's configuration.
    a.s("-F");
    a.s("/dev/null");
    a.o("IdentitiesOnly", "yes");
    a.o("CertificateFile", "/dev/null");
    a.o("UserKnownHostsFile", kh);
    a.o("GlobalKnownHostsFile", "/dev/null");
    a.o("StrictHostKeyChecking", "accept-new");
    a.o("UpdateHostKeys", "no");
    a.o("ControlMaster", "no");
    a.o("ControlPath", "none");
    a.o("ForwardAgent", "no");
    a.o("ForwardX11", "no");
    a.o("PermitLocalCommand", "no");
    a.o("ProxyCommand", "none");
    if let Some(al) = alias {
        // The jump's bridge: the verified key is the destination's, not 127.0.0.1's.
        a.o("HostKeyAlias", al);
        a.o("CheckHostIP", "no");
    }
    match auth {
        SshAuth::Password => {
            a.o("PubkeyAuthentication", "no");
            a.o("IdentityAgent", "none");
            a.o("IdentityFile", "none");
        }
        SshAuth::Key { agent, public_key } => {
            a.o("PubkeyAuthentication", "yes");
            a.o("IdentityAgent", text(agent)?);
            a.o("IdentityFile", text(public_key)?);
            a.o("PasswordAuthentication", "no");
            a.o("KbdInteractiveAuthentication", "no");
            a.o("BatchMode", "yes");
        }
    }
    let port = destination.port.to_string();
    match program {
        Program::Ssh { command } => {
            a.s("-p");
            a.s(&port);
            // D43: the user only as the own argument of `-l`.
            a.s("-l");
            a.s(destination.username);
            a.s("--");
            a.s(destination.host);
            for c in command {
                a.0.push(c.to_vec());
            }
        }
        Program::Forward { host, port: target } => {
            a.o("ConnectTimeout", "15");
            a.o("ExitOnForwardFailure", "yes");
            if let SshAuth::Password = auth {
                a.o("NumberOfPasswordPrompts", "1");
            }
            a.s("-W");
            // `-W` reads `host:port`; IPv6 goes between brackets.
            if host.contains(':') {
                a.s(&format!("[{host}]:{target}"));
            } else {
                a.s(&format!("{host}:{target}"));
            }
            a.s("-p");
            a.s(&port);
            a.s("-l");
            a.s(destination.username);
            a.s("--");
            a.s(destination.host);
        }
        Program::Sftp => {
            // D56: in `sftp`, `-l` is the bandwidth limit; the user goes as `-o` +
            // `User=<user>` (the set of D43 has no `%`, space, quotes, `=`, `,`).
            a.o("User", destination.username);
            a.s("-P");
            a.s(&port);
            a.s("--");
            a.s(destination.host);
        }
    }
    Some(a.0)
}

/// The argv being built.
struct Argv(Vec<Vec<u8>>);

impl Argv {
    fn s(&mut self, x: &str) {
        self.0.push(x.as_bytes().to_vec());
    }

    /// `-o` and `Option=value` in two arguments.
    fn o(&mut self, op: &str, v: &str) {
        self.s("-o");
        self.0.push(format!("{op}={v}").into_bytes());
    }
}

// --- "Connect" token (D3) -------------------------------------------------------------

impl Daemon {
    /// Token issuing (app only; D3): random single-use token, 30 s, bound to a
    /// server entry. Goes away on lock. The value only goes in the response to the
    /// app.
    pub(crate) fn issue_token(&self, role: Role, name: &str) -> Result<Response, ResponseError> {
        if role != Role::App {
            return Err(ResponseError::Unauthorized);
        }
        let now = self.approval.now();
        let mut st = self.state();
        let a = st.open.as_mut().ok_or(ResponseError::Locked)?;
        let e = a
            .entries
            .iter()
            .find(|e| e.name() == name)
            .ok_or(ResponseError::EntryNotFound)?;
        if !matches!(e.data(), Data::Server(_)) {
            return Err(ResponseError::InvalidRequest);
        }
        let value = ward_core::generator::connect_token().map_err(|_| ResponseError::Internal)?;
        a.tokens.retain(|t| now < t.until);
        if a.tokens.len() >= MAX_TOKENS {
            a.tokens.remove(0);
        }
        a.tokens.push(ConnectToken {
            value: Secret::from_bytes(value.expose().to_vec()),
            entry: name.to_owned(),
            until: now.saturating_add(TOKEN_DEADLINE),
        });
        warning!("token do Conectar emitido para uma entrada servidor (D3)");
        Ok(Response::Token(value))
    }

    /// Is the token valid for `entry`? Every token that exists dies here (single use),
    /// even when presented with another entry. Constant-time comparison.
    fn use_token(&self, token: &Secret, entry: &str) -> bool {
        let now = self.approval.now();
        let mut st = self.state();
        let Some(a) = st.open.as_mut() else {
            return false;
        };
        a.tokens.retain(|t| now < t.until);
        let mut found = None;
        for (i, t) in a.tokens.iter().enumerate() {
            if ward_core::generator::tokens_equal(t.value.expose(), token.expose()) {
                found = Some(i);
            }
        }
        match found {
            Some(i) => a.tokens.remove(i).entry == entry,
            None => false,
        }
    }
}

// --- Preparation -----------------------------------------------------------------------

/// What the session needs from the entry, copied under the state's lock.
pub(crate) enum Cred {
    Password(Secret),
    Key {
        key: Secret,
        passphrase: Option<Secret>,
    },
}

fn copy_secret(s: &Secret) -> Secret {
    Secret::from_bytes(s.expose().to_vec())
}

pub(crate) fn cred_from(a: &Auth) -> Cred {
    match a {
        Auth::Password(s) => Cred::Password(copy_secret(s)),
        Auth::Key {
            private_key,
            passphrase,
        } => Cred::Key {
            key: copy_secret(private_key),
            passphrase: passphrase.as_ref().map(copy_secret),
        },
    }
}

/// The jump or tunnel entry (D19, D25): a visible server entry, without its own jump.
/// Blocked (or gone) after the authorization denies; another kind, or with a jump, fails.
pub(crate) fn intermediate<'a>(
    entries: &'a [Entry],
    name: &str,
) -> Result<(&'a Entry, Intermediate), ResponseError> {
    let Some(e) = visible(entries, name) else {
        warning!("salto ou túnel: a outra entrada sumiu ou foi bloqueada; negado (D19)");
        return Err(ResponseError::Denied);
    };
    match e.data() {
        Data::Server(s) if s.jump.is_none() => Ok((
            e,
            Intermediate {
                host: s.host.clone(),
                port: s.port,
                username: s.username.clone(),
                cred: cred_from(&s.auth),
            },
        )),
        _ => {
            warning!("salto ou túnel: a outra entrada não é um servidor sem salto");
            Err(ResponseError::DeliveryFailed)
        }
    }
}

/// The visible entry (blocked = nonexistent, V3.17).
fn visible<'a>(entries: &'a [Entry], name: &str) -> Option<&'a Entry> {
    entries
        .iter()
        .find(|e| e.name() == name && e.level() != AccessLevel::Blocked)
}

/// Does the kind accept `ward ssh`? A server entry, or FTP/SFTP with `sftp` and without a
/// command (`sftp` reads the commands from the input). The others: "type without delivery".
fn verify_kind(e: &Entry, with_command: bool) -> Result<(), ResponseError> {
    match e.data() {
        Data::Server(_) => Ok(()),
        Data::FtpSftp(f) if f.protocol == Protocol::Sftp => {
            if with_command {
                Err(ResponseError::InvalidRequest)
            } else {
                Ok(())
            }
        }
        _ => Err(ResponseError::TypeWithoutDelivery),
    }
}

impl Daemon {
    /// Authorizes (token or level), builds the argv, the environment, the mask and the
    /// agent.
    pub(crate) fn prepare_ssh(
        &self,
        client: &dyn Client,
        request: Request,
    ) -> Result<Preparation, ResponseError> {
        let Request::Ssh {
            entry: entrada,
            command: comando,
            token,
            window_size: janela,
            stdin_kind: tipo_entrada,
        } = &request
        else {
            return Err(ResponseError::InvalidRequest);
        };
        let with_command = !comando.is_empty();
        // Pty only in the interactive one: no command and with the size of the CLI's
        // terminal.
        let window = if with_command { None } else { *janela };
        let name = entrada.clone();
        let null_stdin = *tipo_entrada == ward_core::protocol::StdinKind::Null;
        debug!("ssh: entrada padrão {}", tipo_entrada.name());
        // The level comes before the kind (V4.32: the made-up token on a database entry
        // with approval gives "approval unavailable"); the kind is verified afterwards,
        // under the lock.
        let by_token = token.as_ref().is_some_and(|t| self.use_token(t, &name));
        if by_token {
            warning!("ssh: token do Conectar aceito; sem nível nem aprovação (D3)");
        } else {
            if token.is_some() {
                warning!(
                    "ssh: token inválido, vencido, já usado ou de outra entrada; vale o nível (D3)"
                );
            }
            self.authorize_use(client, &request)?;
        }
        self.record_use();
        let Request::Ssh {
            command: comando, ..
        } = request
        else {
            return Err(ResponseError::InvalidRequest);
        };
        // With the state locked: the entry may have changed during the approval.
        let (host, port, username, cred, sftp, mask, locks, initial, jump, sessions) = {
            let st = self.state();
            let a = st.open.as_ref().ok_or(ResponseError::Locked)?;
            let e = visible(&a.entries, &name).ok_or(ResponseError::EntryNotFound)?;
            verify_kind(e, with_command)?;
            let (host, port, username, auth, sftp, jump_name) = match e.data() {
                Data::Server(s) => (
                    &s.host,
                    s.port,
                    &s.username,
                    &s.auth,
                    false,
                    s.jump.as_deref(),
                ),
                Data::FtpSftp(f) => (&f.host, f.port, &f.username, &f.auth, true, None),
                _ => return Err(ResponseError::TypeWithoutDelivery),
            };
            let failed = |err: ward_core::mask::MaskError| {
                warning!("ssh: {err}");
                ResponseError::DeliveryFailed
            };
            // D19: with a jump, the mask covers both entries (one for the main `ssh`'s
            // output, another for the `stderr` of the bridge's `ssh`).
            let (mask, jump) = match jump_name {
                None => (Mask::from_entry(e, None).map_err(failed)?, None),
                Some(jn) => {
                    let (es, via) = intermediate(&a.entries, jn)?;
                    let both = || {
                        MaskBuilder::new()
                            .entry(e)
                            .entry(es)
                            .build()
                            .map_err(failed)
                    };
                    (both()?, Some((via, both()?)))
                }
            };
            let (locks, initial) = self.locks_now(&st);
            (
                host.clone(),
                port,
                username.clone(),
                cred_from(auth),
                sftp,
                mask,
                locks,
                initial,
                jump,
                self.run_sessions(&st),
            )
        };
        // Ward's `known_hosts` verified (0700/0600, no link) before any `ssh`.
        let known_hosts = self.known_hosts().map_err(|m| {
            warning!("ssh: {m}");
            ResponseError::DeliveryFailed
        })?;
        // Jump: the bridge listens before the main `ssh` exists; the peer is only
        // accepted afterwards, when its pid is known.
        let bridge = match jump {
            None => None,
            Some((via, bridge_mask)) => Some(
                Bridge::open(Opening {
                    via,
                    target_host: &host,
                    target_port: port,
                    programs: self.ssh_programs(),
                    known_hosts: &known_hosts,
                    rt: self.temp_root(),
                    mask: bridge_mask,
                    limits: JUMP_LIMITS,
                    locks: (locks.clone(), initial),
                })
                .map_err(|m| {
                    warning!("ssh: {m}");
                    ResponseError::DeliveryFailed
                })?,
            ),
        };
        let alias = match &bridge {
            None => None,
            Some(_) => Some(destination_alias(&host, port).ok_or_else(|| {
                warning!("ssh: host fora do formato; nada foi executado");
                ResponseError::DeliveryFailed
            })?),
        };
        let (password, agent) = match cred {
            Cred::Password(s) => (Some(s), None),
            Cred::Key { key, passphrase } => {
                let rt = self.temp_root().ok_or(ResponseError::DeliveryFailed)?;
                let ag =
                    SshAgent::create(rt, &key, passphrase.as_ref()).map_err(|e: AgentError| {
                        warning!("ssh: {e}");
                        ResponseError::DeliveryFailed
                    })?;
                (None, Some(ag))
            }
        };
        let paths = agent.as_ref().map(|a| (a.socket(), a.public_key()));
        let auth = match &paths {
            None => SshAuth::Password,
            Some((sock, public_key)) => SshAuth::Key {
                agent: sock,
                public_key,
            },
        };
        let program = if sftp {
            Program::Sftp
        } else {
            Program::Ssh { command: &comando }
        };
        let destination = match &bridge {
            None => Destination {
                host: &host,
                port,
                username: &username,
            },
            Some(p) => Destination {
                host: "127.0.0.1",
                port: p.port(),
                username: &username,
            },
        };
        let argv = build_argv_with_alias(
            self.ssh_programs(),
            &known_hosts,
            destination,
            auth,
            program,
            alias.as_deref(),
        )
        .ok_or_else(|| {
            warning!("ssh: host, usuário ou caminho fora do formato; nada foi executado");
            ResponseError::DeliveryFailed
        })?;
        let cwd = {
            use std::os::unix::ffi::OsStringExt;
            ward_platform::home_dir()
                .map(OsStringExt::into_vec)
                .unwrap_or_else(|_| b"/".to_vec())
        };
        let invalid = |_| ResponseError::DeliveryFailed;
        let mut cmd = Command::new(&argv[0], &cwd).map_err(invalid)?;
        for x in &argv[1..] {
            cmd.argument(x).map_err(invalid)?;
        }
        // Environment from scratch (D13, R19): nothing from the daemon nor from the user's
        // session.
        cmd.variable(b"PATH", CHILD_PATH).map_err(invalid)?;
        cmd.variable(b"LANG", b"C.UTF-8").map_err(invalid)?;
        if window.is_some() {
            cmd.variable(b"TERM", TERM_PTY).map_err(invalid)?;
        }
        if let Some(s) = &password {
            // `sshpass -e`: the password only in the child's environment, never in argv
            // (S3).
            cmd.variable(b"SSHPASS", s.expose()).map_err(invalid)?;
        }
        drop(password);
        debug!(
            "ssh: {} argumento(s), {}, {}",
            argv.len(),
            if sftp { "sftp" } else { "ssh" },
            if window.is_some() { "pty" } else { "sem pty" }
        );
        if bridge.is_some() {
            debug!("ssh: com salto (ponte em 127.0.0.1)");
        }
        Ok(Preparation {
            command: cmd,
            mask,
            temp_files: None,
            bridge,
            pty: window,
            agent: agent.map(|a| PreparedAgent {
                agent: a,
                child_of_child: sftp,
                locks: locks.clone(),
                initial,
            }),
            // Phase 54c (S8; user's decision, 2026-11-10): the open SSH session dies on
            // lock, like `ward run` (the agent and the bridge already went down: D12).
            lock: Some(DiesOnLock {
                sessions,
                locks,
                initial,
            }),
            null_stdin,
        })
    }
}

/// Serves an `ssh` of the agent role, like the [`crate::execution::session`] of
/// `ward run`.
pub fn session(
    daemon: &Daemon,
    client: &dyn Client,
    stream: &std::os::unix::net::UnixStream,
    request: Request,
) -> bool {
    crate::execution::session_with(
        stream,
        |c| {
            let r = daemon.prepare_ssh(c, request);
            if let Err(e) = &r {
                debug!("ssh (agente): {}", e.code());
            }
            r
        },
        client,
    )
}

// Outside `src/ssh/`: V3.15 reads only the first-level files of `src/`.
#[cfg(test)]
#[path = "ssh_tests.rs"]
mod tests;
