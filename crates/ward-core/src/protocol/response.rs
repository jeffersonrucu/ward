//! Daemon responses and notices to the client: the fields of each type, the writing
//! (daemon side) and the strict reading (client side).
//!
//! The types that carry a credential ([`super::ResponseType::carries_credential`]) are
//! app or extension only; [`super::encode_response`] checks the role before writing.
//! [`ListItem`] and [`EntryDetail`] are built from the [`Entry`] and by construction
//! have no secret: the secret extra goes without its value, and of the type's secrets
//! only the name of the filled field goes.

use zeroize::Zeroizing;

use super::errors::{Problem, ResponseError};
use super::fields::{Fields, R, secret_of, text_of, texts_of};
use super::json::{InvalidJson, Writer};
use super::request::SECRET_FIELDS;
use super::{Operation, ResponseType, Role};
use crate::entry::{
    AccessLevel, Auth, Data, Dates, Entry, EntryType, Environment, ExtraValue, Secret,
};
use crate::limits::{
    MAX_ATTACHMENT_NAME, MAX_ATTACHMENTS, MAX_ATTACHMENTS_TOTAL, MAX_DISPLAYED_TEXT, MAX_ENTRIES,
    MAX_EXTRA_NAME, MAX_EXTRA_VALUE, MAX_EXTRAS, MAX_LINE, MAX_LOCK_MINUTES, MAX_NAME, MAX_SECRET,
    MAX_TAG, MAX_TAGS, MAX_TEXT, MAX_URL, MIN_LOCK_MINUTES,
};

/// Stream of a piece of output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

impl Stream {
    fn name(self) -> &'static str {
        match self {
            Stream::Stdout => "stdout",
            Stream::Stderr => "stderr",
        }
    }
}

/// An item of the list: what the agent may see (the scope's commands section: name, type,
/// group, environment, project, URL/host), plus tags; the level only goes to the app (D50).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    pub name: String,
    pub kind: EntryType,
    pub group: String,
    pub environment: Environment,
    pub project: Option<String>,
    pub tags: Vec<String>,
    /// Access level: `Some` only in the app's list. The agent's goes without it (D50):
    /// [`super::encode_response`] refuses to send the agent an item with a level, and the
    /// agent's reading refuses an item that carries one.
    pub level: Option<AccessLevel>,
    /// URL (login, API) or host (database, server, SMTP, FTP/SFTP).
    pub address: Option<String>,
    /// Summary for the 2nd line and the search of the app (D65 c): `Some` only in the app's
    /// list, in the same pattern as the level (D50). [`super::encode_response`] refuses to
    /// send the agent an item with a summary, and the agent's reading refuses an item that
    /// carries one.
    pub summary: Option<ItemSummary>,
}

/// What the app's list carries beyond the header (D65 c), so the screen builds the 2nd
/// line and the search by username without one detail request per entry. No secret by
/// construction: the same text fields as the [`EntryDetail`], only the identifier of the
/// filled secrets and the count of extras and attachments (not even their names).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSummary {
    /// Filled text fields of the type (protocol identifier, value).
    pub fields: Vec<(String, String)>,
    /// Filled secret fields of the type (only the identifier).
    pub secrets: Vec<String>,
    /// Number of extra fields.
    pub extras: u64,
    /// Number of attachments.
    pub attachments: u64,
}

impl ItemSummary {
    /// The summary of an entry.
    pub fn from_entry(e: &Entry) -> Self {
        let (fields, secrets) = fields_and_secrets(e);
        Self {
            fields,
            secrets,
            extras: e.extras().len() as u64,
            attachments: e.attachments().len() as u64,
        }
    }
}

/// JSON values of a summary, at most: the object, the fields, the secrets, the extras,
/// the attachments and the items of the fields and of the secrets (for the node limit
/// of the response).
pub(crate) const MAX_SUMMARY_NODES: usize = 5 + TEXT_FIELDS.len() + SECRET_FIELDS.len();

impl ListItem {
    /// The item of an entry for the role: with the level only for the app (D50). Filtering
    /// out the blocked ones for the agent is the daemon's job.
    pub fn from_entry(e: &Entry, role: Role) -> Self {
        let address = match e.data() {
            Data::Login(l) => Some(l.url.clone()),
            Data::Api(a) => a.url.clone(),
            Data::Database(b) => Some(b.host.clone()),
            Data::Server(s) => Some(s.host.clone()),
            Data::Smtp(s) => Some(s.host.clone()),
            Data::FtpSftp(f) => Some(f.host.clone()),
            Data::Aws(_) | Data::SecureNote(_) | Data::Generic => None,
        };
        Self {
            name: e.name().to_owned(),
            kind: e.kind(),
            group: e.group().to_owned(),
            environment: e.environment(),
            project: e.project().map(str::to_owned),
            tags: e.tags().to_vec(),
            level: (role == Role::App).then(|| e.level()),
            address,
            summary: (role == Role::App).then(|| ItemSummary::from_entry(e)),
        }
    }
}

/// Extra in the detail: the value only when it is not a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraDetail {
    pub name: String,
    /// `None` when the extra is secret (the value only comes out through reveal).
    pub value: Option<String>,
}

/// Attachment in the detail: name and size, without the content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentDetail {
    pub name: String,
    pub size: u64,
}

/// An entry for the app's form, with no secret at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryDetail {
    pub name: String,
    pub kind: EntryType,
    pub group: String,
    pub environment: Environment,
    pub project: Option<String>,
    pub tags: Vec<String>,
    pub level: AccessLevel,
    /// Filled text fields of the type (protocol identifier, value).
    pub fields: Vec<(String, String)>,
    /// Filled secret fields of the type (only the identifier).
    pub secrets: Vec<String>,
    pub extras: Vec<ExtraDetail>,
    pub attachments: Vec<AttachmentDetail>,
    /// Creation and modification dates (D65 d), stamped by the daemon. The detail is app
    /// role only; the agent never gets dates. Without a date, the key does not go.
    pub dates: Dates,
}

impl EntryDetail {
    /// The detail of an entry.
    pub fn from_entry(e: &Entry) -> Self {
        let (fields, secrets) = fields_and_secrets(e);
        Self {
            name: e.name().to_owned(),
            kind: e.kind(),
            group: e.group().to_owned(),
            environment: e.environment(),
            project: e.project().map(str::to_owned),
            tags: e.tags().to_vec(),
            level: e.level(),
            fields,
            secrets,
            extras: e
                .extras()
                .iter()
                .map(|x| ExtraDetail {
                    name: x.name().to_owned(),
                    value: match x.value() {
                        ExtraValue::Text(v) => Some(v.clone()),
                        ExtraValue::Secret(_) => None,
                    },
                })
                .collect(),
            attachments: e
                .attachments()
                .iter()
                .map(|a| AttachmentDetail {
                    name: a.name().to_owned(),
                    size: a.content().len() as u64,
                })
                .collect(),
            dates: e.dates(),
        }
    }
}

/// Text fields (identifier, value) and identifiers of the filled secrets.
type FieldsAndSecrets = (Vec<(String, String)>, Vec<String>);

/// The filled text fields of the type (identifier, value) and the identifiers of the
/// filled secret fields, without the value (of the detail and of the list summary).
fn fields_and_secrets(e: &Entry) -> FieldsAndSecrets {
    let mut fields: Vec<(String, String)> = Vec::new();
    let mut secrets: Vec<String> = Vec::new();
    let mut t = |k: &str, v: &str| fields.push((k.to_owned(), v.to_owned()));
    let mut s = |k: &str| secrets.push(k.to_owned());
    let auth = |a: &Auth, s: &mut dyn FnMut(&str)| match a {
        Auth::Password(_) => s("password"),
        Auth::Key { passphrase, .. } => {
            s("private_key");
            if passphrase.is_some() {
                s("passphrase");
            }
        }
    };
    match e.data() {
        Data::Login(l) => {
            t("url", &l.url);
            t("username", &l.username);
            s("password");
            if l.totp_seed.is_some() {
                s("totp_seed");
            }
        }
        Data::Database(b) => {
            t("engine", b.engine.name());
            t("host", &b.host);
            t("port", &b.port.to_string());
            if let Some(x) = &b.db_name {
                t("db_name", x);
            }
            t("username", &b.username);
            if let Some(x) = &b.tunnel {
                t("tunnel", x);
            }
            s("password");
        }
        Data::Server(v) => {
            t("host", &v.host);
            t("port", &v.port.to_string());
            t("username", &v.username);
            if let Some(x) = &v.jump {
                t("jump", x);
            }
            auth(&v.auth, &mut s);
        }
        Data::Api(a) => {
            if let Some(x) = &a.url {
                t("url", x);
            }
            if let Some(x) = &a.variable {
                t("variable", x);
            }
            s("token");
        }
        Data::Aws(a) => {
            t("access_key_id", &a.access_key_id);
            t("region", &a.region);
            if let Some(x) = &a.role_arn {
                t("role_arn", x);
            }
            s("secret_access_key");
            if a.session_token.is_some() {
                s("session_token");
            }
        }
        Data::Smtp(m) => {
            t("host", &m.host);
            t("port", &m.port.to_string());
            t("username", &m.username);
            t("security", m.security.name());
            s("password");
        }
        Data::FtpSftp(f) => {
            t("protocol", f.protocol.name());
            t("host", &f.host);
            t("port", &f.port.to_string());
            t("username", &f.username);
            auth(&f.auth, &mut s);
        }
        Data::SecureNote(_) => s("text"),
        Data::Generic => {}
    }
    (fields, secrets)
}

/// Identifiers of the text fields of every type.
pub(super) const TEXT_FIELDS: &[&str] = &[
    "url",
    "username",
    "engine",
    "host",
    "port",
    "db_name",
    "tunnel",
    "jump",
    "variable",
    "access_key_id",
    "region",
    "role_arn",
    "security",
    "protocol",
];

/// The daemon asks the app for the approval window (the text is the daemon's, already
/// built for display; the app still treats it as untrusted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequest {
    pub id: u64,
    /// The entry and, in combined use (D19), the jump or tunnel one.
    pub entries: Vec<String>,
    pub environment: Environment,
    /// [`Operation::Run`], `Ssh`, `Login` or `Playwright`.
    pub operation: Operation,
    /// The exact command.
    pub command: String,
    /// The working directory of the command in [`Operation::Run`] (Phase 26a: the CLI's,
    /// where the command runs), already formatted for display (D51); empty in the other
    /// operations.
    pub directory: String,
    /// Who asked (e.g. `claude → bash → ward run`).
    pub chain: Vec<String>,
    /// Phase 44c (D9): the origin username and password go to, when it is `http`
    /// (`localhost`/`127.0.0.1`: Ward does not know which process answers there); empty in
    /// the other cases. Built by the daemon from the URL of the entry.
    pub origin: String,
    /// Phase 44c: the request is only a confirmation of the origin (the entry does not ask
    /// for a PIN: free level, 1 hour release or the extension popup). The window shows the
    /// deny and confirm buttons, without a PIN; the daemon refuses the three allow
    /// decisions in such a request and refuses the confirm one in a request that is not.
    pub confirmation: bool,
    /// Phase 54b (D68): the standard input of the run/SSH is neither a terminal nor null
    /// (pipe, file…): the command gets data the window does not show. The window warns and
    /// only allows with the extra confirmation; the daemon refuses an allow decision without
    /// the confirmed entry flag.
    pub hidden_entry: bool,
}

/// The daemon asks the app for the unlock screen (D16; Phase 39a): an agent use waits
/// with the vault locked. With the vault locked the daemon knows neither type nor
/// environment (the index is encrypted): the notice carries only what the agent asked
/// for, already formatted for display by [`crate::display`] (D51), and the app still
/// treats it as untrusted (neutralize). No secret: the entry is the name the agent sent
/// (not even known to exist) and the command is the same the approval window would show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockRequest {
    /// The name the agent asked for, with visible escaping.
    pub entry: String,
    /// [`Operation::Run`], `Ssh`, `Login` or `Playwright`.
    pub operation: Operation,
    /// The exact command with visible escaping (D51); empty in `Login` and `Playwright`.
    pub command: String,
    /// Seconds left of the daemon's deadline (D16), from 1 to [`MAX_UNLOCK_REMAINING`].
    pub remaining: u64,
}

/// Largest `remaining` of an [`UnlockRequest`]: the 2 min of D16.
pub const MAX_UNLOCK_REMAINING: u64 = 120;

/// Entry suggested by the extension. No secret: name, username, environment and whether
/// the entry has a TOTP seed (only the fact, for the popup to show "com TOTP"; Phase 41).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub name: String,
    pub username: String,
    pub environment: Environment,
    pub totp: bool,
}

/// A USB drive seen by udisks2 (Phase 46): the UUID of the file system, the label
/// (without control characters) and whether it is mounted. No mount path goes to the
/// client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbDrive {
    pub uuid: String,
    pub label: String,
    pub mounted: bool,
}

/// A version of the local backup (Phase 45, Phase 49a): the file name and what the
/// daemon verified in it. The date of the name is only the name's (any process of the
/// user can choose it); the generation sealed in the body cannot be forged, but stays in
/// the daemon (D39: no response carries the generation). Only the marks go:
/// - `verified`: the version opens with the key of the open vault (AEAD); `false` for
///   versions of another master password;
/// - `out_of_order`: its generation is lower than that of a version with an earlier
///   date ([`crate::backup::out_of_order`]); only with `verified`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupVersion {
    pub name: String,
    pub verified: bool,
    pub out_of_order: bool,
}

/// Daemon response.
#[derive(Debug)]
pub enum Response {
    Ok,
    Status {
        locked: bool,
        vault_exists: bool,
        /// The role the daemon gave this connection (D63), with no other data. The app and
        /// the bridge check it on the first response and, without `app`, do not use the
        /// connection.
        role: Role,
    },
    Error(ResponseError),
    List(Vec<ListItem>),
    Output {
        stream: Stream,
        /// Output already masked (D11).
        data: Zeroizing<Vec<u8>>,
    },
    Exit {
        code: u8,
    },
    PlaywrightSession {
        path: String,
    },
    Detail(EntryDetail),
    Secret(Secret),
    Token(Secret),
    ApprovalRequest(ApprovalRequest),
    UnlockRequest(UnlockRequest),
    /// The app opened from the menu found the real instance already up: the daemon asks
    /// it to show the main window (port of 0.1.5, Phase 66a of mission 2). No data.
    ShowWindow,
    TotpCode {
        code: Zeroizing<String>,
        remaining: u64,
    },
    Suggestions(Vec<Suggestion>),
    Fill {
        username: String,
        password: Secret,
        totp: Option<Zeroizing<String>>,
    },
    /// `ward login` (Phase 43): the request the extension is going to serve. Only the id
    /// and the name of the entry, no value: the credential only comes out in
    /// [`Operation::FillLogin`], with the origin verified (D9).
    LoginRequest {
        id: String,
        entry: String,
    },
    /// Versions of the local backup, from newest to oldest by the date of the name
    /// (Phase 45), without the future ones (Phase 49a). Only names in the exact format and
    /// the sealed generation of each one; no path or content.
    Backups(Vec<BackupVersion>),
    /// The part of the vault config the Settings screen shows (Phase 38): the lock
    /// timeout, whether the external backup is enabled and whether the local backup
    /// directory is safe (Phase 49a: `false` when it does not open or is not a 0700
    /// directory of the user, and then the vault is written without backup). Never the PIN
    /// hash, the UUID of the USB drive or a path.
    Config {
        lock_minutes: u32,
        external_backup: bool,
        local_backup: bool,
    },
    /// External backup config (Phase 46): enabled or not and the UUID of the USB drive.
    ExternalBackup {
        enabled: bool,
        uuid: Option<String>,
    },
    /// USB drives seen by udisks2 (Phase 46), up to [`MAX_USB_DRIVES`].
    UsbDrives(Vec<UsbDrive>),
}

const APPROVED_OPERATIONS: [Operation; 4] = [
    Operation::Run,
    Operation::Ssh,
    Operation::Login,
    Operation::Playwright,
];

fn totp_digits(c: &str) -> bool {
    (6..=8).contains(&c.len()) && c.bytes().all(|b| b.is_ascii_digit())
}

impl Response {
    /// The type of this response.
    pub fn kind(&self) -> ResponseType {
        match self {
            Response::Ok => ResponseType::Ok,
            Response::Status { .. } => ResponseType::Status,
            Response::Error(_) => ResponseType::Error,
            Response::List(_) => ResponseType::List,
            Response::Output { .. } => ResponseType::Output,
            Response::Exit { .. } => ResponseType::Exit,
            Response::PlaywrightSession { .. } => ResponseType::PlaywrightSession,
            Response::Detail(_) => ResponseType::Detail,
            Response::Secret(_) => ResponseType::Secret,
            Response::Token(_) => ResponseType::Token,
            Response::ApprovalRequest(_) => ResponseType::ApprovalRequest,
            Response::UnlockRequest(_) => ResponseType::UnlockRequest,
            Response::ShowWindow => ResponseType::ShowWindow,
            Response::TotpCode { .. } => ResponseType::TotpCode,
            Response::Suggestions(_) => ResponseType::Suggestions,
            Response::Fill { .. } => ResponseType::Fill,
            Response::LoginRequest { .. } => ResponseType::LoginRequest,
            Response::Backups(_) => ResponseType::Backups,
            Response::Config { .. } => ResponseType::Config,
            Response::ExternalBackup { .. } => ResponseType::ExternalBackup,
            Response::UsbDrives(_) => ResponseType::UsbDrives,
        }
    }

    /// Checks the limits the reading checks, and the level and the summary of the list by
    /// the role (D50, D65 c: the app always gets them, the agent never).
    pub(crate) fn check(&self, role: Role) -> R<()> {
        let ok = |c: bool| if c { Ok(()) } else { Err(InvalidJson) };
        let item_ok = |i: &ListItem| {
            i.name.len() <= MAX_NAME
                && i.group.len() <= MAX_TEXT
                && i.project.as_ref().is_none_or(|p| p.len() <= MAX_TEXT)
                && i.tags.len() <= MAX_TAGS
                && i.tags.iter().all(|t| t.len() <= MAX_TAG)
                && i.address.as_ref().is_none_or(|p| p.len() <= MAX_URL)
                && i.summary.as_ref().is_none_or(|r| {
                    fields_ok(&r.fields, &r.secrets)
                        && r.extras <= MAX_EXTRAS as u64
                        && r.attachments <= MAX_ATTACHMENTS as u64
                })
        };
        match self {
            // The role of the status is only checked on writing (`encode_response`): the
            // client reads what came and decides (D63).
            Response::Ok | Response::Status { .. } | Response::ShowWindow => Ok(()),
            Response::UnlockRequest(p) => ok(!p.entry.is_empty()
                && p.entry.len() <= MAX_TEXT
                && APPROVED_OPERATIONS.contains(&p.operation)
                && p.command.len() <= MAX_DISPLAYED_TEXT
                && (1..=MAX_UNLOCK_REMAINING).contains(&p.remaining)),
            Response::Exit { .. } => Ok(()),
            Response::Error(e) => ok(match e {
                ResponseError::Validation(p) => {
                    p.len() <= MAX_PROBLEMS
                        && p.iter()
                            .all(|p| p.field.len() <= MAX_LINE && p.reason.len() <= MAX_LINE)
                }
                _ => true,
            }),
            Response::List(l) => ok(l.len() <= MAX_ENTRIES
                && l.iter().all(item_ok)
                && l.iter().all(|i| i.level.is_some() == (role == Role::App))
                && l.iter().all(|i| i.summary.is_some() == (role == Role::App))),
            Response::Output { data, .. } => ok(data.len() <= MAX_DISPLAYED_TEXT),
            Response::PlaywrightSession { path } => ok(!path.is_empty() && path.len() <= MAX_TEXT),
            Response::Detail(d) => ok(d.name.len() <= MAX_NAME
                && d.group.len() <= MAX_TEXT
                && d.project.as_ref().is_none_or(|p| p.len() <= MAX_TEXT)
                && d.tags.len() <= MAX_TAGS
                && d.tags.iter().all(|t| t.len() <= MAX_TAG)
                && fields_ok(&d.fields, &d.secrets)
                && d.extras.len() <= MAX_EXTRAS
                && d.extras.iter().all(|x| {
                    x.name.len() <= MAX_EXTRA_NAME
                        && x.value.as_ref().is_none_or(|v| v.len() <= MAX_EXTRA_VALUE)
                })
                && d.attachments.len() <= MAX_ATTACHMENTS
                && d.attachments.iter().all(|a| {
                    a.name.len() <= MAX_ATTACHMENT_NAME && a.size <= MAX_ATTACHMENTS_TOTAL as u64
                })
                && Dates::from_parts(d.dates.created_at(), d.dates.modified_at()).is_some()),
            Response::Secret(s) => ok(s.len() <= crate::limits::MAX_NOTE),
            Response::Token(t) => ok(!t.is_empty() && t.len() <= MAX_NAME),
            Response::ApprovalRequest(p) => ok((1..=2).contains(&p.entries.len())
                && p.entries.iter().all(|n| n.len() <= MAX_NAME)
                && (APPROVED_OPERATIONS.contains(&p.operation) || p.operation == Operation::Fill)
                && p.origin.len() <= MAX_TEXT
                // Phase 44c: the origin only exists in the fill, in `Login` and in
                // `Playwright`; the confirmation always has an origin; the popup only confirms.
                && (p.origin.is_empty()
                    || matches!(
                        p.operation,
                        Operation::Login | Operation::Playwright | Operation::Fill
                    ))
                && (!p.confirmation || !p.origin.is_empty())
                && (p.operation != Operation::Fill || p.confirmation)
                // Phase 54b: only `Run` and `Ssh` have a standard input, and never in an
                // origin confirmation.
                && (!p.hidden_entry
                    || (matches!(p.operation, Operation::Run | Operation::Ssh)
                        && !p.confirmation))
                && p.command.len() <= MAX_DISPLAYED_TEXT
                && p.directory.len() <= MAX_DISPLAYED_TEXT
                && p.chain.len() <= MAX_CHAIN
                && p.chain.iter().all(|c| c.len() <= MAX_TEXT)),
            Response::TotpCode { code, remaining } => {
                ok(totp_digits(code) && (1..=30).contains(remaining))
            }
            Response::Backups(l) => ok(l.len() <= MAX_BACKUPS
                && l.iter()
                    .all(|v| backup_ok(&v.name) && (v.verified || !v.out_of_order))),
            Response::Config { lock_minutes, .. } => {
                ok((MIN_LOCK_MINUTES..=MAX_LOCK_MINUTES).contains(lock_minutes))
            }
            Response::ExternalBackup { enabled, uuid } => {
                ok(crate::config::ExternalBackup::new(*enabled, uuid.as_deref()).is_ok())
            }
            Response::UsbDrives(l) => ok(l.len() <= MAX_USB_DRIVES
                && l.iter()
                    .all(|p| crate::config::valid_uuid(p.uuid.as_bytes()) && label_ok(&p.label))),
            Response::Suggestions(l) => ok(l.len() <= MAX_ENTRIES
                && l.iter()
                    .all(|s| s.name.len() <= MAX_NAME && s.username.len() <= MAX_TEXT)),
            Response::Fill {
                username,
                password,
                totp,
            } => ok(username.len() <= MAX_TEXT
                && password.len() <= MAX_SECRET
                && totp.as_ref().is_none_or(|c| totp_digits(c))),
            Response::LoginRequest { id, entry } => {
                ok(super::login_id_ok(id) && !entry.is_empty() && entry.len() <= MAX_NAME)
            }
        }
    }

    /// Writes the data of the response.
    pub(crate) fn write(&self, e: &mut Writer) {
        match self {
            Response::Ok | Response::ShowWindow => {}
            Response::UnlockRequest(p) => {
                e.key("entry");
                e.text(&p.entry);
                e.key("operation");
                e.text(p.operation.name());
                e.key("command");
                e.text(&p.command);
                e.key("remaining");
                e.integer(p.remaining);
            }
            Response::Status {
                locked,
                vault_exists,
                role,
            } => {
                e.key("locked");
                e.boolean(*locked);
                e.key("vault_exists");
                e.boolean(*vault_exists);
                e.key("role");
                e.text(role.name());
            }
            Response::Error(x) => {
                e.key("code");
                e.text(x.code());
                e.key("message");
                e.text(x.message());
                if let ResponseError::Validation(p) = x {
                    e.key("problems");
                    e.list(|e| {
                        for p in p {
                            e.item();
                            e.object(|e| {
                                e.key("field");
                                e.text(&p.field);
                                if let Some(n) = p.position {
                                    e.key("position");
                                    e.integer(n);
                                }
                                e.key("reason");
                                e.text(&p.reason);
                            });
                        }
                    });
                }
            }
            Response::List(l) => {
                e.key("items");
                e.list(|e| {
                    for i in l {
                        e.item();
                        e.object(|e| {
                            write_header(
                                e,
                                &i.name,
                                i.kind,
                                &i.group,
                                i.environment,
                                &i.project,
                                &i.tags,
                                i.level,
                            );
                            if let Some(x) = &i.address {
                                e.key("address");
                                e.text(x);
                            }
                            if let Some(r) = &i.summary {
                                e.key("summary");
                                e.object(|e| {
                                    write_fields(e, &r.fields, &r.secrets);
                                    e.key("extras");
                                    e.integer(r.extras);
                                    e.key("attachments");
                                    e.integer(r.attachments);
                                });
                            }
                        });
                    }
                });
            }
            Response::Output { stream, data } => {
                e.key("stream");
                e.text(stream.name());
                e.key("data");
                e.base64(data);
            }
            Response::Exit { code } => {
                e.key("code");
                e.integer(u64::from(*code));
            }
            Response::PlaywrightSession { path } => {
                e.key("path");
                e.text(path);
            }
            Response::Detail(d) => {
                write_header(
                    e,
                    &d.name,
                    d.kind,
                    &d.group,
                    d.environment,
                    &d.project,
                    &d.tags,
                    Some(d.level),
                );
                write_fields(e, &d.fields, &d.secrets);
                e.key("extras");
                e.list(|e| {
                    for x in &d.extras {
                        e.item();
                        e.object(|e| {
                            e.key("name");
                            e.text(&x.name);
                            e.key("secret");
                            e.boolean(x.value.is_none());
                            if let Some(v) = &x.value {
                                e.key("value");
                                e.text(v);
                            }
                        });
                    }
                });
                e.key("attachments");
                e.list(|e| {
                    for a in &d.attachments {
                        e.item();
                        e.object(|e| {
                            e.key("name");
                            e.text(&a.name);
                            e.key("size");
                            e.integer(a.size);
                        });
                    }
                });
                // D65 d: only the dates that exist.
                if let Some(c) = d.dates.created_at() {
                    e.key("created_at");
                    e.integer(c);
                }
                if let Some(a) = d.dates.modified_at() {
                    e.key("modified_at");
                    e.integer(a);
                }
            }
            Response::Secret(s) => {
                e.key("value");
                e.base64(s.expose());
            }
            Response::Token(t) => {
                e.key("token");
                e.base64(t.expose());
            }
            Response::ApprovalRequest(p) => {
                e.key("id");
                e.integer(p.id);
                e.key("entries");
                write_texts(e, &p.entries);
                e.key("environment");
                e.text(p.environment.name());
                e.key("operation");
                e.text(p.operation.name());
                e.key("command");
                e.text(&p.command);
                e.key("directory");
                e.text(&p.directory);
                e.key("chain");
                write_texts(e, &p.chain);
                e.key("origin");
                e.text(&p.origin);
                e.key("confirmation");
                e.boolean(p.confirmation);
                e.key("hidden_entry");
                e.boolean(p.hidden_entry);
            }
            Response::TotpCode { code, remaining } => {
                e.key("code");
                e.text(code);
                e.key("remaining");
                e.integer(*remaining);
            }
            Response::Backups(l) => {
                e.key("versions");
                e.list(|e| {
                    for v in l {
                        e.item();
                        e.object(|e| {
                            e.key("name");
                            e.text(&v.name);
                            e.key("verified");
                            e.boolean(v.verified);
                            e.key("out_of_order");
                            e.boolean(v.out_of_order);
                        });
                    }
                });
            }
            Response::Config {
                lock_minutes,
                external_backup,
                local_backup,
            } => {
                e.key("lock_minutes");
                e.integer(u64::from(*lock_minutes));
                e.key("external_backup");
                e.boolean(*external_backup);
                e.key("local_backup");
                e.boolean(*local_backup);
            }
            Response::ExternalBackup { enabled, uuid } => {
                e.key("enabled");
                e.boolean(*enabled);
                if let Some(u) = uuid {
                    e.key("uuid");
                    e.text(u);
                }
            }
            Response::UsbDrives(l) => {
                e.key("items");
                e.list(|e| {
                    for p in l {
                        e.item();
                        e.object(|e| {
                            e.key("uuid");
                            e.text(&p.uuid);
                            e.key("label");
                            e.text(&p.label);
                            e.key("mounted");
                            e.boolean(p.mounted);
                        });
                    }
                });
            }
            Response::Suggestions(l) => {
                e.key("items");
                e.list(|e| {
                    for s in l {
                        e.item();
                        e.object(|e| {
                            e.key("name");
                            e.text(&s.name);
                            e.key("username");
                            e.text(&s.username);
                            e.key("environment");
                            e.text(s.environment.name());
                            e.key("totp");
                            e.boolean(s.totp);
                        });
                    }
                });
            }
            Response::Fill {
                username,
                password,
                totp,
            } => {
                e.key("username");
                e.text(username);
                e.key("password");
                e.base64(password.expose());
                if let Some(c) = totp {
                    e.key("totp");
                    e.text(c);
                }
            }
            Response::LoginRequest { id, entry } => {
                e.key("id");
                e.text(id);
                e.key("entry");
                e.text(entry);
            }
        }
    }
}

/// Validation problems in a response.
const MAX_PROBLEMS: usize = 512;
/// Most versions in a backups response: the 30 of the rotation and room for files with
/// a valid name put by hand in the directory (the list comes out cut to the newest).
pub const MAX_BACKUPS: usize = 64;

/// Most USB drives in a USB drives response (Phase 46).
pub const MAX_USB_DRIVES: usize = 32;
/// Largest USB drive label, in bytes (ext4's has 16, NTFS's up to 128 in UTF-16).
pub const MAX_LABEL: usize = 128;

/// USB drive label: up to [`MAX_LABEL`] bytes, without control characters.
fn label_ok(r: &str) -> bool {
    r.len() <= MAX_LABEL && !r.chars().any(char::is_control)
}

/// Backup version name in the exact format (Phase 45).
fn backup_ok(n: &str) -> bool {
    #[cfg(unix)]
    {
        crate::backup::valid_name(n)
    }
    #[cfg(not(unix))]
    {
        let _ = n;
        false
    }
}

/// Processes in the chain shown in the approval.
const MAX_CHAIN: usize = 64;

/// The text fields and the identifiers of the secrets, without value: known text
/// identifier, value within the limit and only known secret identifiers (the same as
/// the detail and the list summary).
fn fields_ok(fields: &[(String, String)], secrets: &[String]) -> bool {
    fields.len() <= TEXT_FIELDS.len()
        && fields
            .iter()
            .all(|(k, v)| TEXT_FIELDS.contains(&k.as_str()) && v.len() <= MAX_TEXT)
        && secrets.len() <= SECRET_FIELDS.len()
        && secrets.iter().all(|k| SECRET_FIELDS.contains(&k.as_str()))
}

fn write_fields(e: &mut Writer, fields: &[(String, String)], secrets: &[String]) {
    e.key("fields");
    e.object(|e| {
        for (k, v) in fields {
            e.key(k);
            e.text(v);
        }
    });
    e.key("secrets");
    write_texts(e, secrets);
}

/// Reads the fields and the secrets (of the detail and of the summary): in the order of
/// the frame, only the known text fields (the repeated key the JSON already refused);
/// the check at the end limits the count and the identifiers of the secrets.
fn read_fields(c: &mut Fields) -> R<FieldsAndSecrets> {
    let mut fields = Vec::new();
    for (k, v) in c.object("fields")?.in_order() {
        if !TEXT_FIELDS.contains(&k.as_str()) {
            return Err(InvalidJson);
        }
        fields.push((k, text_of(v, MAX_TEXT)?));
    }
    let secrets = texts_of(c.list("secrets", SECRET_FIELDS.len())?, MAX_NAME)?;
    Ok((fields, secrets))
}

fn write_texts(e: &mut Writer, l: &[String]) {
    e.list(|e| {
        for t in l {
            e.item();
            e.text(t);
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn write_header(
    e: &mut Writer,
    name: &str,
    kind: EntryType,
    group: &str,
    environment: Environment,
    project: &Option<String>,
    tags: &[String],
    level: Option<AccessLevel>,
) {
    e.key("name");
    e.text(name);
    e.key("type");
    e.text(kind.name());
    e.key("group");
    e.text(group);
    e.key("environment");
    e.text(environment.name());
    if let Some(p) = project {
        e.key("project");
        e.text(p);
    }
    e.key("tags");
    write_texts(e, tags);
    if let Some(level) = level {
        e.key("level");
        e.text(level.name());
    }
}

struct Header {
    name: String,
    kind: EntryType,
    group: String,
    environment: Environment,
    project: Option<String>,
    tags: Vec<String>,
    level: Option<AccessLevel>,
}

/// Reads the header of an item or detail. `with_level`: the level is mandatory (app) or
/// forbidden (agent's list, D50).
fn read_header(c: &mut Fields, with_level: bool) -> R<Header> {
    Ok(Header {
        name: c.text("name", MAX_NAME)?,
        kind: EntryType::from_name(&c.text("type", MAX_NAME)?).ok_or(InvalidJson)?,
        group: c.text("group", MAX_TEXT)?,
        environment: Environment::from_name(&c.text("environment", MAX_NAME)?)
            .ok_or(InvalidJson)?,
        project: c.text_opt("project", MAX_TEXT)?,
        tags: texts_of(c.list("tags", MAX_TAGS)?, MAX_TAG)?,
        level: match c.text_opt("level", MAX_NAME)? {
            Some(n) if with_level => Some(AccessLevel::from_name(&n).ok_or(InvalidJson)?),
            None if !with_level => None,
            _ => return Err(InvalidJson),
        },
    })
}

/// Reads the data of a response of the type (the role was already checked).
pub(crate) fn read(role: Role, kind: ResponseType, mut c: Fields) -> R<Response> {
    let r = match kind {
        ResponseType::Ok => Response::Ok,
        ResponseType::ShowWindow => Response::ShowWindow,
        ResponseType::UnlockRequest => Response::UnlockRequest(UnlockRequest {
            entry: c.text("entry", MAX_TEXT)?,
            operation: Operation::from_name(&c.text("operation", MAX_NAME)?).ok_or(InvalidJson)?,
            command: c.text("command", MAX_DISPLAYED_TEXT)?,
            remaining: c.integer("remaining")?,
        }),
        ResponseType::Status => Response::Status {
            locked: c.boolean("locked")?,
            vault_exists: c.boolean("vault_exists")?,
            // Read as it came: the receiver checks whether it is the role it expected.
            role: Role::from_name(&c.text("role", MAX_NAME)?).ok_or(InvalidJson)?,
        },
        ResponseType::Error => {
            let code = c.text("code", MAX_NAME)?;
            // The message of the frame is ignored: the fixed one of the code is what counts.
            let _ = c.text("message", MAX_LINE)?;
            let mut error = ResponseError::from_code(&code).ok_or(InvalidJson)?;
            if let ResponseError::Validation(list) = &mut error {
                for v in c.list("problems", MAX_PROBLEMS)? {
                    let mut o = Fields::from_value(v)?;
                    let field = o.text("field", MAX_LINE)?;
                    let position = match o.take("position") {
                        Some(super::json::Value::Integer(n)) => Some(n),
                        Some(_) => return Err(InvalidJson),
                        None => None,
                    };
                    let reason = o.text("reason", MAX_LINE)?;
                    o.finish()?;
                    list.push(Problem {
                        field,
                        position,
                        reason,
                    });
                }
            }
            Response::Error(error)
        }
        ResponseType::List => {
            let mut items = Vec::new();
            for v in c.list("items", MAX_ENTRIES)? {
                let mut o = Fields::from_value(v)?;
                let k = read_header(&mut o, role == Role::App)?;
                let address = o.text_opt("address", MAX_URL)?;
                // D65 c: the summary is mandatory in the app's list and forbidden in the agent's.
                let summary = match o.take("summary") {
                    Some(v) if role == Role::App => {
                        let mut r = Fields::from_value(v)?;
                        let (fields, secrets) = read_fields(&mut r)?;
                        let extras = r.integer("extras")?;
                        let attachments = r.integer("attachments")?;
                        r.finish()?;
                        Some(ItemSummary {
                            fields,
                            secrets,
                            extras,
                            attachments,
                        })
                    }
                    None if role != Role::App => None,
                    _ => return Err(InvalidJson),
                };
                o.finish()?;
                items.push(ListItem {
                    name: k.name,
                    kind: k.kind,
                    group: k.group,
                    environment: k.environment,
                    project: k.project,
                    tags: k.tags,
                    level: k.level,
                    address,
                    summary,
                });
            }
            Response::List(items)
        }
        ResponseType::Output => {
            let stream = match c.text("stream", MAX_NAME)?.as_str() {
                "stdout" => Stream::Stdout,
                "stderr" => Stream::Stderr,
                _ => return Err(InvalidJson),
            };
            Response::Output {
                stream,
                data: c.bytes("data", MAX_DISPLAYED_TEXT)?,
            }
        }
        ResponseType::Exit => Response::Exit {
            code: u8::try_from(c.integer("code")?).map_err(|_| InvalidJson)?,
        },
        ResponseType::PlaywrightSession => Response::PlaywrightSession {
            path: c.text("path", MAX_TEXT)?,
        },
        ResponseType::Detail => {
            let k = read_header(&mut c, true)?;
            let (fields, secrets) = read_fields(&mut c)?;
            let mut extras = Vec::new();
            for v in c.list("extras", MAX_EXTRAS)? {
                let mut o = Fields::from_value(v)?;
                let name = o.text("name", MAX_EXTRA_NAME)?;
                let secret = o.boolean("secret")?;
                let value = o.text_opt("value", MAX_EXTRA_VALUE)?;
                // A secret extra never carries a value; the plain one always does.
                if secret == value.is_some() {
                    return Err(InvalidJson);
                }
                o.finish()?;
                extras.push(ExtraDetail { name, value });
            }
            let mut attachments = Vec::new();
            for v in c.list("attachments", MAX_ATTACHMENTS)? {
                let mut o = Fields::from_value(v)?;
                let name = o.text("name", MAX_ATTACHMENT_NAME)?;
                let size = o.integer("size")?;
                o.finish()?;
                attachments.push(AttachmentDetail { name, size });
            }
            // D65 d: optional dates, by the rule of `Dates` (range, creation only with
            // modification and not after it).
            let mut data = |k: &str| match c.take(k) {
                None => Ok(None),
                Some(super::json::Value::Integer(n)) => Ok(Some(n)),
                Some(_) => Err(InvalidJson),
            };
            let created = data("created_at")?;
            let modified = data("modified_at")?;
            let dates = Dates::from_parts(created, modified).ok_or(InvalidJson)?;
            Response::Detail(EntryDetail {
                name: k.name,
                kind: k.kind,
                group: k.group,
                environment: k.environment,
                project: k.project,
                tags: k.tags,
                level: k.level.ok_or(InvalidJson)?,
                fields,
                secrets,
                extras,
                attachments,
                dates,
            })
        }
        ResponseType::Secret => Response::Secret(c.secret("value", crate::limits::MAX_NOTE)?),
        ResponseType::Token => Response::Token(c.secret("token", MAX_NAME)?),
        ResponseType::ApprovalRequest => {
            let id = c.integer("id")?;
            let entries = texts_of(c.list("entries", 2)?, MAX_NAME)?;
            let environment =
                Environment::from_name(&c.text("environment", MAX_NAME)?).ok_or(InvalidJson)?;
            let operation =
                Operation::from_name(&c.text("operation", MAX_NAME)?).ok_or(InvalidJson)?;
            let command = c.text("command", MAX_DISPLAYED_TEXT)?;
            let directory = c.text("directory", MAX_DISPLAYED_TEXT)?;
            let chain = texts_of(c.list("chain", MAX_CHAIN)?, MAX_TEXT)?;
            let origin = c.text("origin", MAX_TEXT)?;
            let confirmation = c.boolean("confirmation")?;
            let hidden_entry = c.boolean("hidden_entry")?;
            Response::ApprovalRequest(ApprovalRequest {
                id,
                entries,
                environment,
                operation,
                command,
                directory,
                chain,
                origin,
                confirmation,
                hidden_entry,
            })
        }
        ResponseType::TotpCode => {
            let code = c.text("code", 8)?;
            Response::TotpCode {
                code: Zeroizing::new(code),
                remaining: c.integer("remaining")?,
            }
        }
        ResponseType::Suggestions => {
            let mut items = Vec::new();
            for v in c.list("items", MAX_ENTRIES)? {
                let mut o = Fields::from_value(v)?;
                let name = o.text("name", MAX_NAME)?;
                let username = o.text("username", MAX_TEXT)?;
                let environment =
                    Environment::from_name(&o.text("environment", MAX_NAME)?).ok_or(InvalidJson)?;
                let totp = o.boolean("totp")?;
                o.finish()?;
                items.push(Suggestion {
                    name,
                    username,
                    environment,
                    totp,
                });
            }
            Response::Suggestions(items)
        }
        ResponseType::Backups => {
            let mut versions = Vec::new();
            for v in c.list("versions", MAX_BACKUPS)? {
                let mut o = Fields::from_value(v)?;
                let name = o.text("name", crate::limits::MAX_NAME)?;
                let verified = o.boolean("verified")?;
                let out_of_order = o.boolean("out_of_order")?;
                o.finish()?;
                versions.push(BackupVersion {
                    name,
                    verified,
                    out_of_order,
                });
            }
            Response::Backups(versions)
        }
        ResponseType::Config => Response::Config {
            lock_minutes: u32::try_from(c.integer("lock_minutes")?).map_err(|_| InvalidJson)?,
            external_backup: c.boolean("external_backup")?,
            local_backup: c.boolean("local_backup")?,
        },
        ResponseType::ExternalBackup => Response::ExternalBackup {
            enabled: c.boolean("enabled")?,
            uuid: c.text_opt("uuid", crate::limits::MAX_BACKUP_UUID)?,
        },
        ResponseType::UsbDrives => {
            let mut items = Vec::new();
            for v in c.list("items", MAX_USB_DRIVES)? {
                let mut o = Fields::from_value(v)?;
                let uuid = o.text("uuid", crate::limits::MAX_BACKUP_UUID)?;
                let label = o.text("label", MAX_LABEL)?;
                let mounted = o.boolean("mounted")?;
                o.finish()?;
                items.push(UsbDrive {
                    uuid,
                    label,
                    mounted,
                });
            }
            Response::UsbDrives(items)
        }
        ResponseType::Fill => Response::Fill {
            username: c.text("username", MAX_TEXT)?,
            password: secret_of(c.take("password").ok_or(InvalidJson)?, MAX_SECRET)?,
            totp: c.text_opt("totp", 8)?.map(Zeroizing::new),
        },
        ResponseType::LoginRequest => Response::LoginRequest {
            id: c.text("id", MAX_NAME)?,
            entry: c.text("entry", MAX_NAME)?,
        },
    };
    c.finish()?;
    r.check(role)?;
    Ok(r)
}
