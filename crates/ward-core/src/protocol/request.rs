//! Client requests to the daemon: the fields of each operation, the strict reading and
//! the writing (client side).
//!
//! Format and limit are checked here; business rules (does the entry exist? does the PIN
//! match? does the origin match?) stay in the daemon. Every new or edited entry still
//! goes through [`Draft::validate`] in the daemon.

use zeroize::Zeroizing;

use super::Operation;
use super::errors::ResponseError;
use super::fields::{Fields, R, bytes_of, secret_of, text_of, texts_of};
use super::json::{InvalidJson, Value, Writer};
use crate::entry::{
    AccessLevel, AttachmentDraft, Auth, Data, DataDraft, Draft, Entry, EntryType, ExtraDraft,
    ExtraValue, Secret,
};
use crate::limits::{
    MAX_ARGUMENT, MAX_ARGUMENTS, MAX_ATTACHMENT_NAME, MAX_ATTACHMENTS, MAX_ATTACHMENTS_TOTAL,
    MAX_BACKUP_UUID, MAX_ENV_VARIABLES, MAX_EXTRA_NAME, MAX_EXTRA_VALUE, MAX_EXTRAS,
    MAX_LOCK_MINUTES, MAX_NAME, MAX_NOTE, MAX_PIN_DIGITS, MAX_SECRET, MAX_TAG, MAX_TAGS, MAX_TEXT,
    MAX_URL, MIN_LOCK_MINUTES, MIN_PIN_DIGITS,
};

/// Bytes that zero on drop (argument, directory, variable name).
pub type Bytes = Zeroizing<Vec<u8>>;

/// A variable of the CLI environment (D13: the daemon strips `LD_*` and the like in
/// Phase 26).
#[derive(Debug)]
pub struct Variable {
    pub name: Bytes,
    pub value: Secret,
}

/// `ward run`: entry, command, directory and environment of the CLI, in bytes (they may
/// not be UTF-8). No NUL in any of them, no `=` in a variable name.
#[derive(Debug)]
pub struct RunRequest {
    pub entry: String,
    pub argv: Vec<Bytes>,
    pub cwd: Bytes,
    pub env: Vec<Variable>,
    /// Phase 54b (D68): what the standard input of the CLI is.
    pub stdin_kind: StdinKind,
}

/// Phase 54b (D68): what the standard input of `ward run`/`ward ssh` is, by the `fstat`
/// of the CLI. The client says it: the daemon has no way to check `terminal` (a client
/// that lies, or a pty of the agent itself, passes as a terminal); the null one it
/// enforces (the child's input closes and data that arrives kills the command). Pipe,
/// file and the rest go to the window with a notice and, at the "with approval" level,
/// only pass with the extra confirmation ([`Request::AnswerApproval`] with
/// `confirmed_entry`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdinKind {
    /// A terminal (`isatty`).
    Terminal,
    /// `/dev/null` or closed: no data goes to the command.
    Null,
    /// Pipe or socket.
    Pipe,
    /// Regular file.
    File,
    /// Another device (`/dev/zero`, `/dev/urandom`…) or what could not be seen.
    Other,
}

impl StdinKind {
    /// All, in the frame order.
    pub const ALL: [StdinKind; 5] = [
        StdinKind::Terminal,
        StdinKind::Null,
        StdinKind::Pipe,
        StdinKind::File,
        StdinKind::Other,
    ];

    /// Name in the frame.
    pub fn name(self) -> &'static str {
        match self {
            StdinKind::Terminal => "terminal",
            StdinKind::Null => "nula",
            StdinKind::Pipe => "pipe",
            StdinKind::File => "arquivo",
            StdinKind::Other => "outra",
        }
    }

    /// By the frame name.
    pub fn from_name(n: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.name() == n)
    }

    /// Does the command receive data that the approval window does not show (neither
    /// terminal nor null)?
    pub fn hidden(self) -> bool {
        !matches!(self, StdinKind::Terminal | StdinKind::Null)
    }
}

/// Terminal size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSize {
    pub rows: u16,
    pub cols: u16,
}

/// Which secret of the entry to reveal or copy: a field of the type (by the protocol
/// identifier, e.g. the password or the private key) or a secret extra by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Field(String),
    Extra(String),
}

impl Target {
    /// The secret of the entry this target points to (D65 b: reveal/copy of the app), or
    /// `None` if the type does not have the field, the field is not filled in or the
    /// extra does not exist or is not secret. An attachment is not a target.
    pub fn secret_in<'a>(&self, e: &'a Entry) -> Option<&'a Secret> {
        match self {
            Target::Extra(name) => e.extras().iter().find_map(|x| match x.value() {
                ExtraValue::Secret(v) if x.name() == name => Some(v),
                _ => None,
            }),
            Target::Field(id) => field_secret(e.data(), id),
        }
    }
}

/// Secret field of the type by the protocol identifier ([`SECRET_FIELDS`]).
fn field_secret<'a>(d: &'a Data, id: &str) -> Option<&'a Secret> {
    let auth = |a: &'a Auth| match (a, id) {
        (Auth::Password(s), "senha") => Some(s),
        (Auth::Key { private_key, .. }, "chave_privada") => Some(private_key),
        (Auth::Key { passphrase, .. }, "passphrase") => passphrase.as_ref(),
        _ => None,
    };
    match (d, id) {
        (Data::Login(l), "senha") => Some(&l.password),
        (Data::Login(l), "totp_semente") => l.totp_seed.as_ref(),
        (Data::Database(b), "senha") => Some(&b.password),
        (Data::Server(v), _) => auth(&v.auth),
        (Data::FtpSftp(f), _) => auth(&f.auth),
        (Data::Api(a), "token") => Some(&a.token),
        (Data::Aws(a), "secret_access_key") => Some(&a.secret_access_key),
        (Data::Aws(a), "session_token") => a.session_token.as_ref(),
        (Data::Smtp(m), "senha") => Some(&m.password),
        (Data::SecureNote(n), "texto") => Some(&n.text),
        _ => None,
    }
}

/// Button of the approval window. The three "Allow" ones ask for the PIN; "Deny" and
/// "Confirm" do not.
///
/// "Confirm" (Phase 44c, D9) is only valid in an approval request that the daemon marked
/// as a confirmation: the fill on an `http` origin (`localhost`/`127.0.0.1`) of an entry
/// that does not ask for the PIN (free level, or the extension popup). The daemon checks
/// that; here it is only the format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Deny,
    AllowOnce,
    AllowOneHour,
    AllowAlways,
    Confirm,
}

impl Decision {
    const ALL: [Decision; 5] = [
        Decision::Deny,
        Decision::AllowOnce,
        Decision::AllowOneHour,
        Decision::AllowAlways,
        Decision::Confirm,
    ];

    /// Name in the frame.
    pub fn name(self) -> &'static str {
        match self {
            Decision::Deny => "negar",
            Decision::AllowOnce => "uma_vez",
            Decision::AllowOneHour => "uma_hora",
            Decision::AllowAlways => "sempre",
            Decision::Confirm => "confirmar",
        }
    }

    /// Does it ask for the PIN?
    pub fn requires_pin(self) -> bool {
        !matches!(self, Decision::Deny | Decision::Confirm)
    }
}

/// `ward login` (Phase 43): what the extension did with the login request, told to the
/// daemon in the login result. No value, only the outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginResult {
    /// Filled in, submitted the form and cleared the fields; the next page no longer has
    /// a visible password field.
    Submitted,
    /// Submitted, but the next page (of the same origin) still asks for the password:
    /// the site did not accept the login.
    Refused,
    /// The tab has no login form that can be submitted safely (none or more than one
    /// visible password field, `GET`, destination or `target` outside).
    NoForm,
    /// The tab navigated (or the document changed) between the probe and the fill.
    TabChanged,
    /// Any other failure of the extension (injection refused, tab closed).
    Failure,
}

impl LoginResult {
    /// All, in the frame order.
    pub const ALL: [LoginResult; 5] = [
        LoginResult::Submitted,
        LoginResult::Refused,
        LoginResult::NoForm,
        LoginResult::TabChanged,
        LoginResult::Failure,
    ];

    /// Name in the frame.
    pub fn name(self) -> &'static str {
        match self {
            LoginResult::Submitted => "enviado",
            LoginResult::Refused => "recusado",
            LoginResult::NoForm => "sem_formulario",
            LoginResult::TabChanged => "aba_mudou",
            LoginResult::Failure => "falha",
        }
    }

    /// By the frame name.
    pub fn from_name(n: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.name() == n)
    }
}

/// Id of a `ward login` request (Phase 43): 32 ASCII letters and digits, from the
/// system RNG (the same generator of the "Connect" token).
pub fn login_id_ok(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// Client request. Secrets in [`Secret`] (zeroes on drop, `Debug` without the value).
#[derive(Debug)]
pub enum Request {
    Status,
    Unlock {
        password: Secret,
    },
    Lock,
    List,
    Run(RunRequest),
    Ssh {
        entry: String,
        /// Remote command (empty = interactive shell).
        command: Vec<Bytes>,
        /// Token of "Connect" (D3); without a token the level applies.
        token: Option<Secret>,
        window_size: Option<WindowSize>,
        /// Phase 54b (D68): what the standard input of the CLI is.
        stdin_kind: StdinKind,
    },
    Login {
        entry: String,
    },
    Playwright {
        entry: String,
    },
    Stdin {
        data: Secret,
    },
    Resize(WindowSize),
    CreateVault {
        password: Secret,
        pin: Secret,
    },
    Detail {
        entry: String,
    },
    Reveal {
        entry: String,
        target: Target,
    },
    Copy {
        entry: String,
        target: Target,
    },
    Totp {
        entry: String,
    },
    SaveEntry {
        /// Current name of the edited entry; `None` creates.
        previous: Option<String>,
        draft: Draft,
        /// Phase 39b (V5.12): names of the attachments of the edited entry (`previous`)
        /// that stay in it. The daemon copies their content into the draft, ahead of the
        /// new attachments; the content of a stored attachment never leaves the daemon.
        /// An attachment of the old entry outside this list is removed. Empty when
        /// creating.
        keep_attachments: Vec<String>,
    },
    RemoveEntry {
        entry: String,
    },
    SetLevel {
        entry: String,
        level: AccessLevel,
        pin: Secret,
    },
    AnswerApproval {
        request: u64,
        decision: Decision,
        /// Present if and only if the decision asks for the PIN.
        pin: Option<Secret>,
        /// Phase 54b (D68): the user ticked, in the window, that they allow the standard
        /// input it does not show. Only with an "Allow"; the daemon requires `true` in a
        /// request with a hidden input and refuses `true` in the others.
        confirmed_entry: bool,
    },
    IssueToken {
        entry: String,
    },
    ChangePin {
        current: Secret,
        new: Secret,
    },
    ChangePassword {
        current: Secret,
        new: Secret,
    },
    LockTimeout {
        minutes: u32,
    },
    Suggest {
        origin: String,
    },
    Fill {
        entry: String,
        origin: String,
    },
    SaveLogin {
        /// Always `DataDraft::Login`.
        draft: Draft,
    },
    /// `ward login` (Phase 43): the extension waits for the next login request, without
    /// fields.
    AwaitLogin,
    /// `ward login` (Phase 43): the extension asks for the username, password and TOTP of
    /// the request `id` for the origin of the document of the active tab; the daemon
    /// compares the origin (D9).
    FillLogin {
        id: String,
        origin: String,
    },
    /// `ward login` (Phase 43): the outcome of the request `id` in the tab.
    LoginResult {
        id: String,
        result: LoginResult,
    },
    /// D57: open the true instance of the app (child of the daemon), without any field.
    OpenApp,
    /// Versions of the local backup (Phase 45), without fields.
    Backups,
    /// Lock time and external backup on or off (Phase 38), without fields.
    Config,
    /// Restore a version of the local backup (D17, D39; Phase 45): the exact file name
    /// (`ward-AAAAMMDD-HHMMSS[-N].ward`, never a path), the master password of that
    /// version and the PIN of the current vault (S9: restoring brings back levels and
    /// PIN).
    RestoreBackup {
        version: String,
        password: Secret,
        pin: Secret,
    },
    /// Configuration of the external backup (Phase 46), without fields.
    ExternalBackup,
    /// Turns the external backup on or off (D17, D30; Phase 46). On requires the UUID of
    /// the file system of the USB drive (`[0-9A-Fa-f-]`, up to 36, without `-` at the
    /// ends); off may keep the UUID or not.
    SetExternalBackup {
        enabled: bool,
        uuid: Option<String>,
    },
    /// USB drives seen by udisks2 (Phase 46), without fields.
    UsbDrives,
}

impl Request {
    /// The operation of this request.
    pub fn operation(&self) -> Operation {
        match self {
            Request::Status => Operation::Status,
            Request::Unlock { .. } => Operation::Unlock,
            Request::Lock => Operation::Lock,
            Request::List => Operation::List,
            Request::Run(_) => Operation::Run,
            Request::Ssh { .. } => Operation::Ssh,
            Request::Login { .. } => Operation::Login,
            Request::Playwright { .. } => Operation::Playwright,
            Request::Stdin { .. } => Operation::Stdin,
            Request::Resize(_) => Operation::Resize,
            Request::CreateVault { .. } => Operation::CreateVault,
            Request::Detail { .. } => Operation::Detail,
            Request::Reveal { .. } => Operation::Reveal,
            Request::Copy { .. } => Operation::Copy,
            Request::Totp { .. } => Operation::Totp,
            Request::SaveEntry { .. } => Operation::SaveEntry,
            Request::RemoveEntry { .. } => Operation::RemoveEntry,
            Request::SetLevel { .. } => Operation::SetLevel,
            Request::AnswerApproval { .. } => Operation::AnswerApproval,
            Request::IssueToken { .. } => Operation::IssueToken,
            Request::ChangePin { .. } => Operation::ChangePin,
            Request::ChangePassword { .. } => Operation::ChangePassword,
            Request::LockTimeout { .. } => Operation::LockTimeout,
            Request::Suggest { .. } => Operation::Suggest,
            Request::Fill { .. } => Operation::Fill,
            Request::SaveLogin { .. } => Operation::SaveLogin,
            Request::AwaitLogin => Operation::AwaitLogin,
            Request::FillLogin { .. } => Operation::FillLogin,
            Request::LoginResult { .. } => Operation::LoginResult,
            Request::OpenApp => Operation::OpenApp,
            Request::Backups => Operation::Backups,
            Request::Config => Operation::Config,
            Request::RestoreBackup { .. } => Operation::RestoreBackup,
            Request::ExternalBackup => Operation::ExternalBackup,
            Request::SetExternalBackup { .. } => Operation::SetExternalBackup,
            Request::UsbDrives => Operation::UsbDrives,
        }
    }

    /// Checks what the reading would check, before writing (the client does not send
    /// what the daemon would refuse by format).
    pub(crate) fn check(&self) -> R<()> {
        let ok = |c: bool| if c { Ok(()) } else { Err(InvalidJson) };
        match self {
            Request::Unlock { password } => ok(password_ok(password)),
            Request::CreateVault { password, pin } => ok(password_ok(password) && pin_ok(pin)),
            Request::SetLevel { entry, pin, .. } => ok(name_ok(entry) && pin_ok(pin)),
            Request::ChangePin { current, new } => ok(pin_ok(current) && pin_ok(new)),
            Request::ChangePassword { current, new } => {
                ok(password_ok(current) && password_ok(new))
            }
            Request::AnswerApproval {
                decision,
                pin,
                confirmed_entry,
                ..
            } => ok((match pin {
                Some(p) => decision.requires_pin() && pin_ok(p),
                None => !decision.requires_pin(),
            }) && (!confirmed_entry || decision.requires_pin())),
            Request::LockTimeout { minutes } => {
                ok((MIN_LOCK_MINUTES..=MAX_LOCK_MINUTES).contains(minutes))
            }
            Request::Run(x) => ok(name_ok(&x.entry)
                && command_ok(&x.argv, true)
                && !x.cwd.is_empty()
                && x.cwd.len() <= MAX_ARGUMENT
                && no_nul(&x.cwd)
                && x.env.len() <= MAX_ENV_VARIABLES
                && x.env.iter().all(variable_ok)),
            Request::Ssh {
                entry,
                command,
                token,
                window_size,
                ..
            } => ok(name_ok(entry)
                && command_ok(command, false)
                && token
                    .as_ref()
                    .is_none_or(|t| !t.is_empty() && t.len() <= MAX_NAME)
                && window_size.is_none_or(window_size_ok)),
            Request::Resize(j) => ok(window_size_ok(*j)),
            Request::Stdin { data } => ok(data.len() <= MAX_ARGUMENT),
            Request::Login { entry }
            | Request::Playwright { entry }
            | Request::Detail { entry }
            | Request::Totp { entry }
            | Request::RemoveEntry { entry }
            | Request::IssueToken { entry } => ok(name_ok(entry)),
            Request::Reveal { entry, target } | Request::Copy { entry, target } => {
                ok(name_ok(entry) && target_ok(target))
            }
            Request::Suggest { origin } => ok(origin.len() <= MAX_URL),
            Request::Fill { entry, origin } => ok(name_ok(entry) && origin.len() <= MAX_URL),
            Request::FillLogin { id, origin } => ok(login_id_ok(id) && origin.len() <= MAX_URL),
            Request::LoginResult { id, .. } => ok(login_id_ok(id)),
            Request::SaveEntry {
                previous,
                draft,
                keep_attachments,
            } => ok(previous.as_deref().is_none_or(name_ok)
                && draft_ok(draft)
                && keep_attachments_ok(previous.is_some(), keep_attachments, draft)),
            Request::SaveLogin { draft } => {
                ok(matches!(draft.data, DataDraft::Login { .. }) && draft_ok(draft))
            }
            Request::RestoreBackup {
                version,
                password,
                pin,
            } => ok(version_ok(version) && password_ok(password) && pin_ok(pin)),
            Request::SetExternalBackup { enabled, uuid } => ok(external_backup_ok(*enabled, uuid)),
            Request::Status
            | Request::Lock
            | Request::List
            | Request::OpenApp
            | Request::Backups
            | Request::Config
            | Request::ExternalBackup
            | Request::AwaitLogin
            | Request::UsbDrives => Ok(()),
        }
    }

    /// Writes the data object of the request.
    pub(crate) fn write(&self, e: &mut Writer) {
        match self {
            Request::Status
            | Request::Lock
            | Request::List
            | Request::OpenApp
            | Request::Backups
            | Request::Config
            | Request::ExternalBackup
            | Request::AwaitLogin
            | Request::UsbDrives => {}
            Request::SetExternalBackup { enabled, uuid } => {
                e.key("ligado");
                e.boolean(*enabled);
                if let Some(u) = uuid {
                    e.key("uuid");
                    e.text(u);
                }
            }
            Request::RestoreBackup {
                version,
                password,
                pin,
            } => {
                e.key("versao");
                e.text(version);
                e.key("senha");
                e.base64(password.expose());
                e.key("pin");
                e.base64(pin.expose());
            }
            Request::Unlock { password } => {
                e.key("senha");
                e.base64(password.expose());
            }
            Request::Run(x) => {
                e.key("entrada");
                e.text(&x.entry);
                e.key("argv");
                write_bytes(e, &x.argv);
                e.key("cwd");
                e.base64(&x.cwd);
                e.key("tipo_entrada");
                e.text(x.stdin_kind.name());
                e.key("ambiente");
                e.list(|e| {
                    for v in &x.env {
                        e.item();
                        e.object(|e| {
                            e.key("nome");
                            e.base64(&v.name);
                            e.key("valor");
                            e.base64(v.value.expose());
                        });
                    }
                });
            }
            Request::Ssh {
                entry,
                command,
                token,
                window_size,
                stdin_kind,
            } => {
                e.key("entrada");
                e.text(entry);
                e.key("comando");
                write_bytes(e, command);
                e.key("tipo_entrada");
                e.text(stdin_kind.name());
                if let Some(t) = token {
                    e.key("token");
                    e.base64(t.expose());
                }
                if let Some(j) = window_size {
                    e.key("janela");
                    e.object(|e| write_window_size(e, *j));
                }
            }
            Request::Login { entry }
            | Request::Playwright { entry }
            | Request::Detail { entry }
            | Request::Totp { entry }
            | Request::RemoveEntry { entry }
            | Request::IssueToken { entry } => {
                e.key("entrada");
                e.text(entry);
            }
            Request::Stdin { data } => {
                e.key("dados");
                e.base64(data.expose());
            }
            Request::Resize(j) => write_window_size(e, *j),
            Request::CreateVault { password, pin } => {
                e.key("senha");
                e.base64(password.expose());
                e.key("pin");
                e.base64(pin.expose());
            }
            Request::Reveal { entry, target } | Request::Copy { entry, target } => {
                e.key("entrada");
                e.text(entry);
                match target {
                    Target::Field(c) => {
                        e.key("campo");
                        e.text(c);
                    }
                    Target::Extra(x) => {
                        e.key("extra");
                        e.text(x);
                    }
                }
            }
            Request::SaveEntry {
                previous,
                draft,
                keep_attachments,
            } => {
                if let Some(a) = previous {
                    e.key("anterior");
                    e.text(a);
                }
                if !keep_attachments.is_empty() {
                    e.key("manter_anexos");
                    e.list(|e| {
                        for n in keep_attachments {
                            e.item();
                            e.text(n);
                        }
                    });
                }
                e.key("entrada");
                e.object(|e| write_draft(e, draft));
            }
            Request::SaveLogin { draft } => {
                e.key("entrada");
                e.object(|e| write_draft(e, draft));
            }
            Request::SetLevel { entry, level, pin } => {
                e.key("entrada");
                e.text(entry);
                e.key("nivel");
                e.text(level.name());
                e.key("pin");
                e.base64(pin.expose());
            }
            Request::AnswerApproval {
                request,
                decision,
                pin,
                confirmed_entry,
            } => {
                e.key("pedido");
                e.integer(*request);
                e.key("decisao");
                e.text(decision.name());
                if let Some(p) = pin {
                    e.key("pin");
                    e.base64(p.expose());
                }
                if *confirmed_entry {
                    e.key("entrada_confirmada");
                    e.boolean(true);
                }
            }
            Request::ChangePin { current, new } => {
                e.key("atual");
                e.base64(current.expose());
                e.key("novo");
                e.base64(new.expose());
            }
            Request::ChangePassword { current, new } => {
                e.key("atual");
                e.base64(current.expose());
                e.key("nova");
                e.base64(new.expose());
            }
            Request::LockTimeout { minutes } => {
                e.key("minutos");
                e.integer(u64::from(*minutes));
            }
            Request::Suggest { origin } => {
                e.key("origem");
                e.text(origin);
            }
            Request::Fill { entry, origin } => {
                e.key("entrada");
                e.text(entry);
                e.key("origem");
                e.text(origin);
            }
            Request::FillLogin { id, origin } => {
                e.key("id");
                e.text(id);
                e.key("origem");
                e.text(origin);
            }
            Request::LoginResult { id, result } => {
                e.key("id");
                e.text(id);
                e.key("resultado");
                e.text(result.name());
            }
        }
    }
}

// ---- Format checks (the same in the reading and in the writing) ----

fn name_ok(n: &str) -> bool {
    !n.is_empty() && n.len() <= MAX_NAME
}

/// Exact name of a backup version (Phase 45): no slash, no `..`, no path.
fn version_ok(v: &str) -> bool {
    #[cfg(unix)]
    {
        crate::backup::valid_name(v)
    }
    #[cfg(not(unix))]
    {
        let _ = v;
        false
    }
}

/// External backup in the format the configuration accepts (D45): UUID `[0-9A-Fa-f-]`,
/// from 1 to 36, without `-` at the ends; on requires the UUID.
fn external_backup_ok(enabled: bool, uuid: &Option<String>) -> bool {
    crate::config::ExternalBackup::new(enabled, uuid.as_deref()).is_ok()
}

fn password_ok(s: &Secret) -> bool {
    !s.is_empty() && s.len() <= MAX_SECRET
}

/// PIN: from 4 to 6 ASCII digits. The daemon checks it against the hash (S9).
fn pin_ok(p: &Secret) -> bool {
    let b = p.expose();
    (MIN_PIN_DIGITS..=MAX_PIN_DIGITS).contains(&b.len()) && b.iter().all(u8::is_ascii_digit)
}

fn no_nul(b: &[u8]) -> bool {
    !b.contains(&0)
}

fn command_ok(argv: &[Bytes], required: bool) -> bool {
    (!required || !argv.is_empty())
        && argv.len() <= MAX_ARGUMENTS
        && argv.iter().all(|a| a.len() <= MAX_ARGUMENT && no_nul(a))
}

fn variable_ok(v: &Variable) -> bool {
    !v.name.is_empty()
        && v.name.len() <= MAX_ARGUMENT
        && no_nul(&v.name)
        && !v.name.contains(&b'=')
        && v.value.len() <= MAX_ARGUMENT
        && no_nul(v.value.expose())
}

fn window_size_ok(j: WindowSize) -> bool {
    j.rows > 0 && j.cols > 0
}

fn target_ok(a: &Target) -> bool {
    match a {
        Target::Field(c) => SECRET_FIELDS.contains(&c.as_str()),
        Target::Extra(x) => !x.is_empty() && x.len() <= MAX_EXTRA_NAME,
    }
}

/// Identifiers of the secret fields of all types (the same as the draft's).
pub(crate) const SECRET_FIELDS: &[&str] = &[
    "senha",
    "totp_semente",
    "chave_privada",
    "passphrase",
    "token",
    "secret_access_key",
    "session_token",
    "texto",
];

fn draft_ok(r: &Draft) -> bool {
    let attachments: usize = r.attachments.iter().map(|a| a.content.len()).sum();
    let texts = [&r.group, &r.environment, &r.project];
    name_ok(&r.name)
        && texts.iter().all(|t| t.len() <= MAX_TEXT)
        && r.tags.len() <= MAX_TAGS
        && r.tags.iter().all(|t| t.len() <= MAX_TAG)
        && r.extras.len() <= MAX_EXTRAS
        && r.extras
            .iter()
            .all(|x| x.name.len() <= MAX_EXTRA_NAME && x.value.len() <= MAX_EXTRA_VALUE)
        && r.attachments.len() <= MAX_ATTACHMENTS
        && r.attachments
            .iter()
            .all(|a| a.name.len() <= MAX_ATTACHMENT_NAME)
        && attachments <= MAX_ATTACHMENTS_TOTAL
        && draft_fields(&r.data).iter().all(|c| match c {
            DraftField::Text(_, t) => t.len() <= MAX_TEXT,
            DraftField::Secret(k, s) => s.len() <= max_secret(k),
        })
}

/// Phase 39b: "keep attachment" only when editing; non-empty names, within the limit,
/// without repetition, and kept plus new fit in [`MAX_ATTACHMENTS`]. Whether the name
/// exists in the edited entry is a daemon rule ([`merge_kept_attachments`]).
fn keep_attachments_ok(editing: bool, names: &[String], r: &Draft) -> bool {
    (editing || names.is_empty())
        && names.len().saturating_add(r.attachments.len()) <= MAX_ATTACHMENTS
        && names.iter().enumerate().all(|(i, n)| {
            !n.is_empty() && n.len() <= MAX_ATTACHMENT_NAME && !names[..i].contains(n)
        })
}

/// Phase 39b (V5.12): puts ahead of the new attachments of the draft a copy of each
/// attachment of `old` asked for in `names`, in the order asked. A name the edited entry
/// does not have, or a repeated one, is [`ResponseError::InvalidRequest`] (nothing changes
/// in the draft). After this the draft goes through the whole [`Draft::validate`]: sum of
/// D29, name repeated with a new attachment, mask budget. The content is copied inside
/// the daemon; there is no operation that returns it (S1, S11).
pub fn merge_kept_attachments(
    draft: &mut Draft,
    old: &Entry,
    names: &[String],
) -> Result<(), ResponseError> {
    let mut kept = Vec::with_capacity(names.len());
    for (i, name) in names.iter().enumerate() {
        if names[..i].contains(name) {
            return Err(ResponseError::InvalidRequest);
        }
        let a = old
            .attachments()
            .iter()
            .find(|a| a.name() == name)
            .ok_or(ResponseError::InvalidRequest)?;
        kept.push(AttachmentDraft {
            name: a.name().to_owned(),
            content: Secret::from_bytes(a.content().expose().to_vec()),
        });
    }
    if kept.len().saturating_add(draft.attachments.len()) > MAX_ATTACHMENTS {
        return Err(ResponseError::InvalidRequest);
    }
    kept.append(&mut draft.attachments);
    draft.attachments = kept;
    Ok(())
}

fn max_secret(field: &str) -> usize {
    if field == "texto" {
        MAX_NOTE
    } else {
        MAX_SECRET
    }
}

// ---- Reading ----

/// Reads the data object of the operation (the role was already checked).
pub(crate) fn read(op: Operation, mut c: Fields) -> R<Request> {
    let p = match op {
        Operation::Status => Request::Status,
        Operation::Lock => Request::Lock,
        Operation::List => Request::List,
        Operation::OpenApp => Request::OpenApp,
        Operation::Backups => Request::Backups,
        Operation::Config => Request::Config,
        Operation::ExternalBackup => Request::ExternalBackup,
        Operation::UsbDrives => Request::UsbDrives,
        Operation::AwaitLogin => Request::AwaitLogin,
        Operation::FillLogin => Request::FillLogin {
            id: c.text("id", MAX_NAME)?,
            origin: c.text("origem", MAX_URL)?,
        },
        Operation::LoginResult => Request::LoginResult {
            id: c.text("id", MAX_NAME)?,
            result: LoginResult::from_name(&c.text("resultado", MAX_NAME)?).ok_or(InvalidJson)?,
        },
        Operation::SetExternalBackup => Request::SetExternalBackup {
            enabled: c.boolean("ligado")?,
            uuid: c.text_opt("uuid", MAX_BACKUP_UUID)?,
        },
        Operation::RestoreBackup => Request::RestoreBackup {
            version: c.text("versao", MAX_NAME)?,
            password: c.secret("senha", MAX_SECRET)?,
            pin: c.secret("pin", MAX_PIN_DIGITS)?,
        },
        Operation::Unlock => Request::Unlock {
            password: c.secret("senha", MAX_SECRET)?,
        },
        Operation::Run => {
            let entry = c.text("entrada", MAX_NAME)?;
            let argv = read_bytes(c.list("argv", MAX_ARGUMENTS)?)?;
            let cwd = c.bytes("cwd", MAX_ARGUMENT)?;
            let stdin_kind = read_stdin_kind(&mut c)?;
            let mut env = Vec::new();
            for v in c.list("ambiente", MAX_ENV_VARIABLES)? {
                let mut o = Fields::from_value(v)?;
                let name = o.bytes("nome", MAX_ARGUMENT)?;
                let value = o.secret("valor", MAX_ARGUMENT)?;
                o.finish()?;
                env.push(Variable { name, value });
            }
            Request::Run(RunRequest {
                entry,
                argv,
                cwd,
                env,
                stdin_kind,
            })
        }
        Operation::Ssh => {
            let entry = c.text("entrada", MAX_NAME)?;
            let command = read_bytes(c.list_or_empty("comando", MAX_ARGUMENTS)?)?;
            let token = c.secret_opt("token", MAX_NAME)?;
            let window_size = match c.take("janela") {
                Some(v) => Some(read_window_size(Fields::from_value(v)?)?),
                None => None,
            };
            let stdin_kind = read_stdin_kind(&mut c)?;
            Request::Ssh {
                entry,
                command,
                token,
                window_size,
                stdin_kind,
            }
        }
        Operation::Login => Request::Login {
            entry: c.text("entrada", MAX_NAME)?,
        },
        Operation::Playwright => Request::Playwright {
            entry: c.text("entrada", MAX_NAME)?,
        },
        Operation::Stdin => Request::Stdin {
            data: c.secret("dados", MAX_ARGUMENT)?,
        },
        Operation::Resize => {
            let j = read_window_size(c)?;
            return checked(Request::Resize(j));
        }
        Operation::CreateVault => Request::CreateVault {
            password: c.secret("senha", MAX_SECRET)?,
            pin: c.secret("pin", MAX_PIN_DIGITS)?,
        },
        Operation::Detail => Request::Detail {
            entry: c.text("entrada", MAX_NAME)?,
        },
        Operation::Reveal | Operation::Copy => {
            let entry = c.text("entrada", MAX_NAME)?;
            let target = match (c.take("campo"), c.take("extra")) {
                (Some(v), None) => Target::Field(text_of(v, MAX_NAME)?),
                (None, Some(v)) => Target::Extra(text_of(v, MAX_EXTRA_NAME)?),
                _ => return Err(InvalidJson),
            };
            if op == Operation::Reveal {
                Request::Reveal { entry, target }
            } else {
                Request::Copy { entry, target }
            }
        }
        Operation::Totp => Request::Totp {
            entry: c.text("entrada", MAX_NAME)?,
        },
        Operation::SaveEntry => {
            let previous = c.text_opt("anterior", MAX_NAME)?;
            let keep_attachments = texts_of(
                c.list_or_empty("manter_anexos", MAX_ATTACHMENTS)?,
                MAX_ATTACHMENT_NAME,
            )?;
            let draft = read_draft(c.object("entrada")?)?;
            Request::SaveEntry {
                previous,
                draft,
                keep_attachments,
            }
        }
        Operation::RemoveEntry => Request::RemoveEntry {
            entry: c.text("entrada", MAX_NAME)?,
        },
        Operation::SetLevel => {
            let entry = c.text("entrada", MAX_NAME)?;
            let level = AccessLevel::from_name(&c.text("nivel", MAX_NAME)?).ok_or(InvalidJson)?;
            let pin = c.secret("pin", MAX_PIN_DIGITS)?;
            Request::SetLevel { entry, level, pin }
        }
        Operation::AnswerApproval => {
            let request = c.integer("pedido")?;
            let name = c.text("decisao", MAX_NAME)?;
            let decision = Decision::ALL
                .into_iter()
                .find(|d| d.name() == name)
                .ok_or(InvalidJson)?;
            let pin = c.secret_opt("pin", MAX_PIN_DIGITS)?;
            // Phase 54b: absent = not ticked; present only as `true`.
            let confirmed_entry = match c.take("entrada_confirmada") {
                None => false,
                Some(Value::Bool(true)) => true,
                Some(_) => return Err(InvalidJson),
            };
            Request::AnswerApproval {
                request,
                decision,
                pin,
                confirmed_entry,
            }
        }
        Operation::IssueToken => Request::IssueToken {
            entry: c.text("entrada", MAX_NAME)?,
        },
        Operation::ChangePin => Request::ChangePin {
            current: c.secret("atual", MAX_PIN_DIGITS)?,
            new: c.secret("novo", MAX_PIN_DIGITS)?,
        },
        Operation::ChangePassword => Request::ChangePassword {
            current: c.secret("atual", MAX_SECRET)?,
            new: c.secret("nova", MAX_SECRET)?,
        },
        Operation::LockTimeout => Request::LockTimeout {
            minutes: u32::try_from(c.integer("minutos")?).map_err(|_| InvalidJson)?,
        },
        Operation::Suggest => Request::Suggest {
            origin: c.text("origem", MAX_URL)?,
        },
        Operation::Fill => Request::Fill {
            entry: c.text("entrada", MAX_NAME)?,
            origin: c.text("origem", MAX_URL)?,
        },
        Operation::SaveLogin => Request::SaveLogin {
            draft: read_draft(c.object("entrada")?)?,
        },
    };
    c.finish()?;
    checked(p)
}

fn checked(p: Request) -> R<Request> {
    p.check()?;
    Ok(p)
}

/// Phase 54b (D68): the kind of the standard input, required (no default: the client
/// says it).
fn read_stdin_kind(c: &mut Fields) -> R<StdinKind> {
    StdinKind::from_name(&c.text("tipo_entrada", MAX_NAME)?).ok_or(InvalidJson)
}

fn read_bytes(v: Vec<Value>) -> R<Vec<Bytes>> {
    v.into_iter().map(|a| bytes_of(a, MAX_ARGUMENT)).collect()
}

fn write_bytes(e: &mut Writer, l: &[Bytes]) {
    e.list(|e| {
        for a in l {
            e.item();
            e.base64(a);
        }
    });
}

fn read_window_size(mut c: Fields) -> R<WindowSize> {
    let rows = u16::try_from(c.integer("linhas")?).map_err(|_| InvalidJson)?;
    let cols = u16::try_from(c.integer("colunas")?).map_err(|_| InvalidJson)?;
    c.finish()?;
    Ok(WindowSize { rows, cols })
}

fn write_window_size(e: &mut Writer, j: WindowSize) {
    e.key("linhas");
    e.integer(u64::from(j.rows));
    e.key("colunas");
    e.integer(u64::from(j.cols));
}

// ---- Draft (entry form) ----
//
// An object with the name, group, environment, optional project, optional tags, the
// type, an optional object with the type fields (text or base64), optional extras (name,
// value, secret flag) and optional attachments (name, content).
//
// Without a level: a new entry is born "with approval" and editing keeps the level (S9).
// An absent type field is "not given" (empty text or `None` secret).

/// A type field as the protocol sees it.
enum DraftField<'a> {
    Text(&'static str, &'a String),
    Secret(&'static str, &'a Secret),
}

/// The filled-in fields of the draft, in the type order.
fn draft_fields(d: &DataDraft) -> Vec<DraftField<'_>> {
    use DraftField::{Secret as S, Text as T};
    let mut v = Vec::new();
    macro_rules! t {
        ($k:literal, $x:expr) => {
            if !$x.is_empty() {
                v.push(T($k, $x));
            }
        };
    }
    macro_rules! s {
        ($k:literal, $x:expr) => {
            if let Some(x) = $x {
                v.push(S($k, x));
            }
        };
    }
    match d {
        DataDraft::Login {
            url,
            username,
            password,
            totp_seed,
        } => {
            t!("url", url);
            t!("usuario", username);
            s!("senha", password);
            s!("totp_semente", totp_seed);
        }
        DataDraft::Database {
            engine,
            host,
            port,
            db_name,
            username,
            password,
            tunnel,
        } => {
            t!("motor", engine);
            t!("host", host);
            t!("porta", port);
            t!("base", db_name);
            t!("usuario", username);
            s!("senha", password);
            t!("tunel", tunnel);
        }
        DataDraft::Server {
            host,
            port,
            username,
            password,
            private_key,
            passphrase,
            jump,
        } => {
            t!("host", host);
            t!("porta", port);
            t!("usuario", username);
            s!("senha", password);
            s!("chave_privada", private_key);
            s!("passphrase", passphrase);
            t!("salto", jump);
        }
        DataDraft::Api {
            url,
            token,
            variable,
        } => {
            t!("url", url);
            s!("token", token);
            t!("variavel", variable);
        }
        DataDraft::Aws {
            access_key_id,
            secret_access_key,
            region,
            role_arn,
            session_token,
        } => {
            t!("access_key_id", access_key_id);
            s!("secret_access_key", secret_access_key);
            t!("regiao", region);
            t!("role_arn", role_arn);
            s!("session_token", session_token);
        }
        DataDraft::Smtp {
            host,
            port,
            username,
            password,
            security,
        } => {
            t!("host", host);
            t!("porta", port);
            t!("usuario", username);
            s!("senha", password);
            t!("seguranca", security);
        }
        DataDraft::FtpSftp {
            protocol,
            host,
            port,
            username,
            password,
            private_key,
            passphrase,
        } => {
            t!("protocolo", protocol);
            t!("host", host);
            t!("porta", port);
            t!("usuario", username);
            s!("senha", password);
            s!("chave_privada", private_key);
            s!("passphrase", passphrase);
        }
        DataDraft::SecureNote { text } => {
            s!("texto", text);
        }
        DataDraft::Generic => {}
    }
    v
}

fn read_data(kind: EntryType, mut c: Fields) -> R<DataDraft> {
    let t = |c: &mut Fields, k: &str| c.text_or_empty(k, MAX_TEXT);
    let s = |c: &mut Fields, k: &str| c.secret_opt(k, max_secret(k));
    let d = match kind {
        EntryType::Login => DataDraft::Login {
            url: t(&mut c, "url")?,
            username: t(&mut c, "usuario")?,
            password: s(&mut c, "senha")?,
            totp_seed: s(&mut c, "totp_semente")?,
        },
        EntryType::Database => DataDraft::Database {
            engine: t(&mut c, "motor")?,
            host: t(&mut c, "host")?,
            port: t(&mut c, "porta")?,
            db_name: t(&mut c, "base")?,
            username: t(&mut c, "usuario")?,
            password: s(&mut c, "senha")?,
            tunnel: t(&mut c, "tunel")?,
        },
        EntryType::Server => DataDraft::Server {
            host: t(&mut c, "host")?,
            port: t(&mut c, "porta")?,
            username: t(&mut c, "usuario")?,
            password: s(&mut c, "senha")?,
            private_key: s(&mut c, "chave_privada")?,
            passphrase: s(&mut c, "passphrase")?,
            jump: t(&mut c, "salto")?,
        },
        EntryType::Api => DataDraft::Api {
            url: t(&mut c, "url")?,
            token: s(&mut c, "token")?,
            variable: t(&mut c, "variavel")?,
        },
        EntryType::Aws => DataDraft::Aws {
            access_key_id: t(&mut c, "access_key_id")?,
            secret_access_key: s(&mut c, "secret_access_key")?,
            region: t(&mut c, "regiao")?,
            role_arn: t(&mut c, "role_arn")?,
            session_token: s(&mut c, "session_token")?,
        },
        EntryType::Smtp => DataDraft::Smtp {
            host: t(&mut c, "host")?,
            port: t(&mut c, "porta")?,
            username: t(&mut c, "usuario")?,
            password: s(&mut c, "senha")?,
            security: t(&mut c, "seguranca")?,
        },
        EntryType::FtpSftp => DataDraft::FtpSftp {
            protocol: t(&mut c, "protocolo")?,
            host: t(&mut c, "host")?,
            port: t(&mut c, "porta")?,
            username: t(&mut c, "usuario")?,
            password: s(&mut c, "senha")?,
            private_key: s(&mut c, "chave_privada")?,
            passphrase: s(&mut c, "passphrase")?,
        },
        EntryType::SecureNote => DataDraft::SecureNote {
            text: s(&mut c, "texto")?,
        },
        EntryType::Generic => DataDraft::Generic,
    };
    // A field of another type (or an invented one, like a level) refuses.
    c.finish()?;
    Ok(d)
}

fn read_draft(mut c: Fields) -> R<Draft> {
    let name = c.text("nome", MAX_NAME)?;
    let group = c.text_or_empty("grupo", MAX_TEXT)?;
    let environment = c.text_or_empty("ambiente", MAX_TEXT)?;
    let project = c.text_or_empty("projeto", MAX_TEXT)?;
    let tags = texts_of(c.list_or_empty("tags", MAX_TAGS)?, MAX_TAG)?;
    let kind = EntryType::from_name(&c.text("tipo", MAX_NAME)?).ok_or(InvalidJson)?;
    let data = read_data(kind, c.object_or_empty("campos")?)?;
    let mut extras = Vec::new();
    for v in c.list_or_empty("extras", MAX_EXTRAS)? {
        let mut o = Fields::from_value(v)?;
        let name = o.text("nome", MAX_EXTRA_NAME)?;
        let value = o.secret("valor", MAX_EXTRA_VALUE)?;
        let secret = o.boolean("segredo")?;
        o.finish()?;
        extras.push(ExtraDraft {
            name,
            value,
            secret,
        });
    }
    let mut attachments = Vec::new();
    let mut sum = 0usize;
    for v in c.list_or_empty("anexos", MAX_ATTACHMENTS)? {
        let mut o = Fields::from_value(v)?;
        let name = o.text("nome", MAX_ATTACHMENT_NAME)?;
        // The limit of each one is what still fits in the sum (D29).
        let content = secret_of(
            o.take("conteudo").ok_or(InvalidJson)?,
            MAX_ATTACHMENTS_TOTAL - sum,
        )?;
        o.finish()?;
        sum += content.len();
        attachments.push(AttachmentDraft { name, content });
    }
    c.finish()?;
    Ok(Draft {
        name,
        group,
        environment,
        project,
        tags,
        data,
        extras,
        attachments,
    })
}

fn write_draft(e: &mut Writer, r: &Draft) {
    e.key("nome");
    e.text(&r.name);
    e.key("grupo");
    e.text(&r.group);
    e.key("ambiente");
    e.text(&r.environment);
    if !r.project.is_empty() {
        e.key("projeto");
        e.text(&r.project);
    }
    e.key("tags");
    e.list(|e| {
        for t in &r.tags {
            e.item();
            e.text(t);
        }
    });
    e.key("tipo");
    e.text(r.data.kind().name());
    e.key("campos");
    e.object(|e| {
        for c in draft_fields(&r.data) {
            match c {
                DraftField::Text(k, t) => {
                    e.key(k);
                    e.text(t);
                }
                DraftField::Secret(k, s) => {
                    e.key(k);
                    e.base64(s.expose());
                }
            }
        }
    });
    e.key("extras");
    e.list(|e| {
        for x in &r.extras {
            e.item();
            e.object(|e| {
                e.key("nome");
                e.text(&x.name);
                e.key("valor");
                e.base64(x.value.expose());
                e.key("segredo");
                e.boolean(x.secret);
            });
        }
    });
    e.key("anexos");
    e.list(|e| {
        for a in &r.attachments {
            e.item();
            e.object(|e| {
                e.key("nome");
                e.text(&a.name);
                e.key("conteudo");
                e.base64(a.content.expose());
            });
        }
    });
}
