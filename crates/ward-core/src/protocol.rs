//! Protocol of the `wardd` socket (R1, D5, Phase 20): frames with a size limit,
//! versioned messages in strict JSON, roles and the operation × role table.
//!
//! # Frame
//!
//! 4 size bytes (big-endian u32) and the body, a JSON object in UTF-8 ([`frame`]). The
//! limit depends on the role, which the daemon takes from the connection
//! (`SO_PEERCRED`, Phase 21) **before** reading the frame: 1 MiB for the common client
//! (agent) and the extension ([`limits::MAX_COMMON_FRAME`], V3.4), and a larger frame
//! only for the identified app ([`limits::MAX_APP_FRAME`]), which fits the largest entry
//! the validation accepts, with the 256 KiB of attachments of D29.
//!
//! # Messages
//!
//! ```text
//! request  = {"v": 2, "op": "<operation>", "data": {…}}
//! response = {"v": 2, "type": "<type>", "data": {…}}
//! error    = {"v": 2, "type": "error", "data": {"code": "…", "message": "…"}}
//! ```
//!
//! - `v` is the version ([`VERSION`], 2 since m2:DT8); another one is refused with
//!   [`ResponseError::UnknownVersion`] before anything else of the frame is looked at:
//!   a 0.1.x client (version 1) gets that error and no data. The data object may be
//!   missing when empty.
//! - Every name (operation, response type, field, error code, role and value) is
//!   English `snake_case` (m2:DT8). The version 1 names are not known: an old field
//!   name (the version 1 name of `entry`) is an unknown field and refuses the request;
//!   it is never read as the new one nor ignored.
//! - Strict reading: a field the request does not have (a `true` reveal flag, a field
//!   name in a list request, a level in a new entry, a generation anywhere) refuses the
//!   whole request. A repeated key too (two `op` keys in the same frame).
//! - **Every secret goes in base64** (master password, PIN, secrets of the entry,
//!   attachments, command bytes and environment): without JSON escapes, with a
//!   predictable size, and the text read zeroes on drop. Plain text goes as JSON text.
//! - No message has the vault generation (D39): the daemon does not send it, and a
//!   generation field coming from the client is not accepted.
//!
//! # Roles (D5)
//!
//! The role comes from the connection, never from the frame. Each operation has the
//! roles that can request it ([`Operation::roles`]) and each response type the roles
//! that can receive it ([`ResponseType::roles`]). The order of reading the request is:
//! JSON → version → known operation → **role** → fields. So an app or extension
//! operation requested by the agent answers [`ResponseError::Unauthorized`] whatever
//! the rest of the frame is.
//!
//! The agent's list goes without the level of each entry (D50) and without the summary
//! (D65 c); the app's carries both.
//!
//! **No response that carries a credential goes to the agent** (S1): the ones that carry
//! one ([`ResponseType::carries_credential`]) are only for the app or the extension, and
//! [`encode_response`] refuses to send them to another role. The compile-time assertion
//! at the end of this file and the table test pin this. Approving, answering an approval
//! and changing a level are app only (S9, S10, V3.15); there is no feature, variable or
//! argument that changes the table (D8).

use zeroize::Zeroizing;

use crate::limits;

mod errors;
mod fields;
pub mod frame;
mod json;
mod request;
mod response;

#[cfg(test)]
mod tests;

pub use errors::{
    MSG_GENERATION_EXHAUSTED, MSG_ROLLBACK, MSG_VAULT_CHANGED, Problem, ResponseError,
};
pub use request::{
    Decision, LoginResult, Request, RunRequest, StdinKind, Target, Variable, WindowSize,
    login_id_ok, merge_kept_attachments,
};
pub use response::{
    ApprovalRequest, AttachmentDetail, BackupVersion, EntryDetail, ExtraDetail, ItemSummary,
    ListItem, MAX_BACKUPS, MAX_LABEL, MAX_UNLOCK_REMAINING, MAX_USB_DRIVES, Response, Stream,
    Suggestion, UnlockRequest, UsbDrive,
};

/// Protocol version (m2:DT8: 1 in 0.1.x, 2 with the English names).
pub const VERSION: u64 = 2;

/// Who is on the other side of the socket (D5). The daemon decides by the connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// Any client that is neither the installed app nor the installed extension host:
    /// the CLI, Claude, a script (D3).
    Agent,
    /// The installed app, checked (root-owned exe, no debugger, no `LD_*`).
    App,
    /// The installed native messaging host, child of the installed Chrome.
    Extension,
}

impl Role {
    /// All the roles.
    pub const ALL: [Role; 3] = [Role::Agent, Role::App, Role::Extension];

    /// Name for the log.
    pub fn name(self) -> &'static str {
        match self {
            Role::Agent => "agent",
            Role::App => "app",
            Role::Extension => "extension",
        }
    }

    /// By the name (the one of the status, D63).
    pub fn from_name(n: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|p| p.name() == n)
    }
}

/// Set of roles (constant, for the table).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Roles {
    pub agent: bool,
    pub app: bool,
    pub extension: bool,
}

impl Roles {
    const fn of(agent: bool, app: bool, extension: bool) -> Self {
        Self {
            agent,
            app,
            extension,
        }
    }

    /// Is the role in the set?
    pub const fn has(self, p: Role) -> bool {
        match p {
            Role::Agent => self.agent,
            Role::App => self.app,
            Role::Extension => self.extension,
        }
    }
}

const AGENT_AND_APP: Roles = Roles::of(true, true, false);
const ALL: Roles = Roles::of(true, true, true);
const AGENT_ONLY: Roles = Roles::of(true, false, false);
const APP_ONLY: Roles = Roles::of(false, true, false);
const EXTENSION_ONLY: Roles = Roles::of(false, false, true);
const APP_AND_EXTENSION: Roles = Roles::of(false, true, true);

macro_rules! table {
    ($name:ident, $doc:literal, $( $var:ident = $txt:literal, $roles:expr, $vdoc:literal; )+) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $( #[doc = $vdoc] $var, )+
        }

        impl $name {
            /// All, in the table order.
            pub const ALL: &'static [$name] = &[$( $name::$var, )+];

            /// Name in the frame.
            pub const fn name(self) -> &'static str {
                match self { $( $name::$var => $txt, )+ }
            }

            /// Allowed roles.
            pub const fn roles(self) -> Roles {
                match self { $( $name::$var => $roles, )+ }
            }

            /// By the frame name.
            pub fn from_name(n: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|o| o.name() == n)
            }

            /// Can this role?
            pub const fn allowed(self, p: Role) -> bool {
                self.roles().has(p)
            }
        }
    };
}

table! {
    Operation, "Operation of the protocol and the roles that can request it (D5).",
    Status = "status", ALL, "Locked or not, and whether the vault exists.";
    Unlock = "unlock", AGENT_AND_APP, "Unlock with the master password (CLI in the user's terminal or app).";
    Lock = "lock", AGENT_AND_APP, "Lock now.";
    List = "list", AGENT_AND_APP, "Visible entries (for the agent, without the blocked ones).";
    Run = "run", AGENT_ONLY, "`ward run`: the daemon runs the command with the credential; the masked output comes back.";
    Ssh = "ssh", AGENT_ONLY, "`ward ssh`.";
    Login = "login", AGENT_ONLY, "`ward login`, through the extension.";
    Playwright = "playwright", AGENT_ONLY, "`ward playwright`: only the path of the storage state comes back.";
    Stdin = "stdin", AGENT_ONLY, "Bytes for the standard input of the open session (empty = end).";
    Resize = "resize", AGENT_ONLY, "Terminal size of the open session.";
    CreateVault = "create_vault", APP_ONLY, "First use: master password and PIN.";
    Detail = "detail", APP_ONLY, "Fields of an entry for the form, without secrets.";
    Reveal = "reveal", APP_ONLY, "Value of a secret to show (D4).";
    Copy = "copy", APP_ONLY, "Value of a secret for the clipboard, by the Rust side of the app (D4).";
    Totp = "totp", APP_ONLY, "Current TOTP code of the entry.";
    SaveEntry = "save_entry", APP_ONLY, "Create or edit an entry (without level; S9).";
    RemoveEntry = "remove_entry", APP_ONLY, "Delete an entry.";
    SetLevel = "set_level", APP_ONLY, "Change the access level, with PIN (S9).";
    AnswerApproval = "answer_approval", APP_ONLY, "Answer of the approval window, with PIN to allow (S10).";
    IssueToken = "issue_token", APP_ONLY, "Single-use token of \"Connect\" (D3).";
    ChangePin = "change_pin", APP_ONLY, "Change the PIN.";
    ChangePassword = "change_password", APP_ONLY, "Change the master password.";
    LockTimeout = "lock_timeout", APP_ONLY, "Inactivity lock time, in minutes.";
    Backups = "backups", APP_ONLY, "Versions of the local backup, from the newest to the oldest (D17, Phase 45).";
    RestoreBackup = "restore_backup", APP_ONLY, "Restore a version of the local backup, with its master password and the current PIN (D17, D39, Phase 45).";
    Config = "config", APP_ONLY, "Lock time and external backup on or off, for the Settings screen (Phase 38); without the PIN hash or the UUID.";
    ExternalBackup = "external_backup", APP_ONLY, "Configuration of the external backup: on or off and the UUID of the USB drive (D17, D30, Phase 46).";
    SetExternalBackup = "set_external_backup", APP_ONLY, "Turn the external backup on or off and choose the USB drive by the UUID (D17, D30, Phase 46).";
    UsbDrives = "usb_drives", APP_ONLY, "USB drive file systems that udisks2 sees, to choose the one of the external backup (Phase 46).";
    Suggest = "suggest", EXTENSION_ONLY, "`login` entries of the tab origin.";
    Fill = "fill", EXTENSION_ONLY, "Username, password and TOTP to fill, if the origin matches (D9).";
    SaveLogin = "save_login", EXTENSION_ONLY, "Save a new login seen by the extension.";
    AwaitLogin = "await_login", EXTENSION_ONLY, "`ward login` (Phase 43): the extension waits for the next login request (up to 25 s).";
    FillLogin = "fill_login", EXTENSION_ONLY, "`ward login` (Phase 43): username, password and TOTP of the request, if the tab origin matches (D9).";
    LoginResult = "login_result", EXTENSION_ONLY, "`ward login` (Phase 43): the outcome of the request in the tab.";
    OpenApp = "open_app", ALL, "Open the true instance of the app, child of the daemon (D57); without fields and without secrets.";
}

table! {
    ResponseType, "Response type of the daemon and the roles that can receive it.",
    Ok = "ok", ALL, "Done, without data.";
    Status = "status", ALL, "Locked or not, and whether the vault exists.";
    Error = "error", ALL, "Error with code and fixed message.";
    List = "list", AGENT_AND_APP, "Items of the list (without secrets).";
    Output = "output", AGENT_ONLY, "Piece of the output, already masked.";
    Exit = "exit", AGENT_ONLY, "End of the run, with the exit code.";
    PlaywrightSession = "playwright_session", AGENT_ONLY, "Path of the storage state.";
    Detail = "detail", APP_ONLY, "Fields of an entry without secrets.";
    Secret = "secret", APP_ONLY, "Value of a secret (reveal/copy).";
    Token = "token", APP_ONLY, "Token of \"Connect\".";
    ApprovalRequest = "approval_request", APP_ONLY, "The daemon asks the app for the approval window.";
    UnlockRequest = "unlock_request", APP_ONLY, "The daemon asks the app for the unlock screen (D16): entry, operation, command with visible escapes (D51) and seconds left (Phase 39a).";
    ShowWindow = "show_window", APP_ONLY, "The daemon asks the open app to show the main window (the menu found the instance already up). No data.";
    TotpCode = "totp_code", APP_AND_EXTENSION, "Current TOTP code.";
    Suggestions = "suggestions", EXTENSION_ONLY, "Entries suggested for the tab.";
    Fill = "fill", EXTENSION_ONLY, "Username, password and TOTP to fill.";
    LoginRequest = "login_request", EXTENSION_ONLY, "`ward login` (Phase 43): id and name of the entry, without a value.";
    Backups = "backups", APP_ONLY, "Names of the local backup versions, without the ones with a future date, with the verified and out-of-order marks, without the generation (D17, D39, Phases 45 and 49a).";
    Config = "config", APP_ONLY, "Lock time in minutes, external backup on or off and local backup folder safe or not (Phases 38 and 49a).";
    ExternalBackup = "external_backup", APP_ONLY, "Configuration of the external backup (D17, D30, Phase 46).";
    UsbDrives = "usb_drives", APP_ONLY, "USB drives seen by udisks2: UUID, label and whether it is mounted (Phase 46).";
}

impl ResponseType {
    /// Does the response carry a credential value (secret, token or TOTP code)?
    pub const fn carries_credential(self) -> bool {
        matches!(
            self,
            ResponseType::Secret
                | ResponseType::Token
                | ResponseType::TotpCode
                | ResponseType::Fill
        )
    }
}

// S1, at compile time: no response with a credential can go to the agent, and no
// approval or level operation can come from it (S9, S10, V3.15).
const _: () = {
    let mut i = 0;
    while i < ResponseType::ALL.len() {
        let t = ResponseType::ALL[i];
        assert!(!(t.carries_credential() && t.roles().agent));
        i += 1;
    }
    assert!(!Operation::AnswerApproval.roles().agent);
    assert!(!Operation::SetLevel.roles().agent);
    assert!(!Operation::Reveal.roles().agent);
    assert!(!Operation::Copy.roles().agent);
    assert!(!Operation::IssueToken.roles().agent);
    assert!(!Operation::Fill.roles().agent);
    // Phase 43: only the extension waits for, fills and answers the `ward login`.
    assert!(!Operation::AwaitLogin.roles().agent);
    assert!(!Operation::AwaitLogin.roles().app);
    assert!(!Operation::FillLogin.roles().agent);
    assert!(!Operation::FillLogin.roles().app);
    assert!(!Operation::LoginResult.roles().agent);
    assert!(!Operation::LoginResult.roles().app);
    assert!(!ResponseType::LoginRequest.roles().agent);
    // V7.8: restoring and listing backups only by the app (D17).
    assert!(!Operation::RestoreBackup.roles().agent);
    assert!(!Operation::RestoreBackup.roles().extension);
    assert!(!Operation::Backups.roles().agent);
    assert!(!Operation::Backups.roles().extension);
    // Phase 38: the vault configuration and the PIN, password and lock changes only by
    // the app.
    assert!(!Operation::Config.roles().agent);
    assert!(!Operation::Config.roles().extension);
    assert!(!Operation::ChangePin.roles().agent);
    assert!(!Operation::ChangePassword.roles().agent);
    assert!(!Operation::LockTimeout.roles().agent);
    // V7.8: turning on, reading the external backup and seeing the USB drives only by
    // the app (Phase 46).
    assert!(!Operation::ExternalBackup.roles().agent);
    assert!(!Operation::ExternalBackup.roles().extension);
    assert!(!Operation::SetExternalBackup.roles().agent);
    assert!(!Operation::SetExternalBackup.roles().extension);
    assert!(!Operation::UsbDrives.roles().agent);
    assert!(!Operation::UsbDrives.roles().extension);
    // The app frame fits the largest entry (D29) in base64 and the escaped texts.
    assert!(
        json::base64_len(limits::MAX_ENTRY) + 2 * limits::MAX_ENTRY_TEXTS + 64 * limits::KIB
            <= limits::MAX_APP_FRAME
    );
    assert!(limits::MAX_COMMON_FRAME < 2 * limits::MIB);
};

/// Why it was not possible to encode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// The message is not of this role.
    NotAllowedForRole,
    /// It would go over the frame limit.
    TooLarge,
    /// Field out of format (text above the limit, for example): the other side would
    /// refuse it.
    Invalid,
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::NotAllowedForRole => "mensagem não permitida a este papel",
            Self::TooLarge => "mensagem grande demais para o quadro",
            Self::Invalid => "mensagem fora do formato",
        })
    }
}

impl std::error::Error for EncodeError {}

/// Daemon response that the client refused (format, version or type of another role).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidResponse;

impl core::fmt::Display for InvalidResponse {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("resposta inválida do Ward")
    }
}

impl std::error::Error for InvalidResponse {}

/// Envelope read up to the name: the rest of the top and the data stay for after the
/// role.
struct Envelope {
    name: String,
    top: fields::Fields,
}

enum EnvelopeError {
    Json,
    Version,
}

/// Reads `{"v", <key>, …}`: JSON, version and name.
fn read_envelope(bytes: &[u8], key: &str, max_nodes: usize) -> Result<Envelope, EnvelopeError> {
    let v = json::read(bytes, max_nodes).map_err(|_| EnvelopeError::Json)?;
    let mut top = fields::Fields::unchecked(v).map_err(|_| EnvelopeError::Json)?;
    match top.integer("v") {
        Ok(VERSION) => {}
        Ok(_) => return Err(EnvelopeError::Version),
        Err(_) => return Err(EnvelopeError::Json),
    }
    let name = top.text(key, 64).map_err(|_| EnvelopeError::Json)?;
    Ok(Envelope { name, top })
}

impl Envelope {
    /// The data object (absent = empty). Any other key at the top (a `true` reveal
    /// flag) refuses.
    fn data(mut self) -> Result<fields::Fields, json::InvalidJson> {
        // `v` and the name already left; a second copy of them stayed and falls here or
        // in `finish`.
        self.top.no_duplicates()?;
        let data = self.top.object_or_empty("data")?;
        self.top.finish()?;
        Ok(data)
    }
}

/// Reads a request from the client with this role. The error is already the response to
/// send.
///
/// Order: JSON → version → known operation → **role** → rest of the top and fields. The
/// role is checked before looking at the rest, so no extra, missing or malformed field
/// changes the "unauthorized" of an operation of another role (V3.3).
pub fn decode_request(role: Role, bytes: &[u8]) -> Result<Request, ResponseError> {
    let env = read_envelope(bytes, "op", json::MAX_NODES).map_err(|e| match e {
        EnvelopeError::Json => ResponseError::InvalidRequest,
        EnvelopeError::Version => ResponseError::UnknownVersion,
    })?;
    let op = Operation::from_name(&env.name).ok_or(ResponseError::UnknownOperation)?;
    if !op.allowed(role) {
        return Err(ResponseError::Unauthorized);
    }
    let data = env.data().map_err(|_| ResponseError::InvalidRequest)?;
    request::read(op, data).map_err(|_| ResponseError::InvalidRequest)
}

/// Encodes a request (client side). Refuses an operation of another role and a frame
/// above the role's limit.
pub fn encode_request(role: Role, p: &Request) -> Result<Zeroizing<Vec<u8>>, EncodeError> {
    let op = p.operation();
    if !op.allowed(role) {
        return Err(EncodeError::NotAllowedForRole);
    }
    p.check().map_err(|_| EncodeError::Invalid)?;
    json::encode(frame::frame_limit(role), |e| {
        e.open_object();
        e.key("v");
        e.integer(VERSION);
        e.key("op");
        e.text(op.name());
        e.key("data");
        e.object(|e| p.write(e));
        e.close_object();
    })
    .map_err(|_| EncodeError::TooLarge)
}

/// Encodes a daemon response for the client with this role. Refuses the type that is
/// not of the role: a response with a credential never goes out to the agent (S1).
pub fn encode_response(role: Role, r: &Response) -> Result<Zeroizing<Vec<u8>>, EncodeError> {
    let kind = r.kind();
    if !kind.allowed(role) {
        return Err(EncodeError::NotAllowedForRole);
    }
    // D50: the access level never goes to the agent in the list; D65 c: nor the summary.
    if let Response::List(items) = r
        && role != Role::App
        && items
            .iter()
            .any(|i| i.level.is_some() || i.summary.is_some())
    {
        return Err(EncodeError::NotAllowedForRole);
    }
    // D63: the status says the role of the connection that receives it, never another.
    if let Response::Status { role: p, .. } = r
        && *p != role
    {
        return Err(EncodeError::Invalid);
    }
    r.check(role).map_err(|_| EncodeError::Invalid)?;
    json::encode(frame::RESPONSE_LIMIT, |e| {
        e.open_object();
        e.key("v");
        e.integer(VERSION);
        e.key("type");
        e.text(kind.name());
        e.key("data");
        e.object(|e| r.write(e));
        e.close_object();
    })
    .map_err(|_| EncodeError::TooLarge)
}

/// Reads a daemon response (client side with this role). Refuses the type that is not
/// of the role: the CLI never accepts a response with a credential.
pub fn decode_response(role: Role, bytes: &[u8]) -> Result<Response, InvalidResponse> {
    let env =
        read_envelope(bytes, "type", json::MAX_RESPONSE_NODES).map_err(|_| InvalidResponse)?;
    let kind = ResponseType::from_name(&env.name).ok_or(InvalidResponse)?;
    if !kind.allowed(role) {
        return Err(InvalidResponse);
    }
    let data = env.data().map_err(|_| InvalidResponse)?;
    response::read(role, kind, data).map_err(|_| InvalidResponse)
}

/// The version a daemon frame says it has, when [`decode_response`] refused it (m2:DT8):
/// `Some(n)` with `n` ≠ [`VERSION`] tells the client that the server speaks another
/// version, so the CLI and the app stop with the version message before sending the
/// master password. Only the JSON and the `v` key are read; nothing else of the frame is
/// trusted or returned.
pub fn frame_version(bytes: &[u8]) -> Option<u64> {
    let v = json::read(bytes, json::MAX_RESPONSE_NODES).ok()?;
    fields::Fields::unchecked(v).ok()?.integer("v").ok()
}

/// The error response for a frame refused by the reading (V3.4). The daemon sends this
/// one and closes the connection.
pub fn frame_error(_e: frame::FrameError) -> ResponseError {
    ResponseError::InvalidFrame
}
