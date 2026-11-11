//! Vault entry model (R5–R9, D10, D24): the 9 types, secret fields, extra fields,
//! attachments, group/environment/project/tags and access level.
//!
//! - An [`Entry`] is only born validated: from a [`Draft`] through [`Draft::validate`].
//!   The fields are private, so no code builds an invalid entry.
//! - Every secret value is in a [`Secret`], which is zeroed on drop, has no `Clone` and
//!   prints `••••` (S8, S13). No type here has `Clone`.
//! - The validation errors ([`ValidationError`]) only carry fixed Portuguese texts, never
//!   a typed value: an error does not leak a secret, not even by mistake.
//! - A new entry always starts "with approval" (S9). [`Entry::set_level`] exists for the
//!   daemon, which requires the PIN before calling it.
//!
//! What the validation blocks on purpose (input from the app and the extension is
//! untrusted): sizes, control and Unicode formatting characters (which would forge the
//! text of the approval window), host or username starting with `-` (it would become an
//! `ssh` option), extra field and attachment names that turn into the same variable
//! (D10) or match a variable of the type, and `LD_*`/`WARD_*` as the `api` variable.

use core::fmt;

use zeroize::ZeroizeOnDrop;

mod secret;
pub(crate) mod validation;

#[cfg(test)]
mod tests;

pub use secret::Secret;
pub use validation::{
    AttachmentDraft, DataDraft, Draft, ExtraDraft, TOO_LARGE_FOR_MASK, ValidationError,
    ValidationErrors, local_url_origin, url_origin, variable_name,
};
// Reserved names that the delivery (D10) builds; the delivery test checks that they match.
#[cfg(test)]
pub(crate) use validation::{COMMON_VARIABLES, type_attachments, type_variables};

/// Environment of the entry (required). Organizes and filters; changes no rule (R16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Environment {
    Dev,
    Stage,
    Prod,
}

impl Environment {
    /// All of them, in the order of the interface.
    pub const ALL: [Environment; 3] = [Environment::Dev, Environment::Stage, Environment::Prod];

    /// Name used in the interface, in the CLI and in `WARD_ENVIRONMENT`.
    pub fn name(self) -> &'static str {
        match self {
            Environment::Dev => "dev",
            Environment::Stage => "stage",
            Environment::Prod => "prod",
        }
    }

    /// Reads the exact name (`dev`, `stage` or `prod`).
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.name() == name)
    }
}

impl fmt::Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Access level of the entry for the agent (R15). The default is
/// [`AccessLevel::Approval`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AccessLevel {
    /// The agent does not even see the entry.
    Blocked,
    /// Each use opens the approval window.
    #[default]
    Approval,
    /// Used without asking.
    Free,
}

impl AccessLevel {
    /// All of them, from the most restrictive to the least restrictive.
    pub const ALL: [AccessLevel; 3] = [
        AccessLevel::Blocked,
        AccessLevel::Approval,
        AccessLevel::Free,
    ];

    /// Stable identifier (`blocked`, `approval`, `free`; written in the vault body v5,
    /// m2:DT4). The v4 names are read only by `body::legacy_v4`.
    pub fn name(self) -> &'static str {
        match self {
            AccessLevel::Blocked => "blocked",
            AccessLevel::Approval => "approval",
            AccessLevel::Free => "free",
        }
    }

    /// Interface text.
    pub fn label(self) -> &'static str {
        match self {
            AccessLevel::Blocked => "bloqueado",
            AccessLevel::Approval => "com aprovação",
            AccessLevel::Free => "livre",
        }
    }

    /// Reads the exact identifier.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|n| n.name() == name)
    }

    /// The most restrictive of the two (combined level, D19).
    pub fn most_restrictive(self, other: Self) -> Self {
        fn order(n: AccessLevel) -> u8 {
            match n {
                AccessLevel::Blocked => 0,
                AccessLevel::Approval => 1,
                AccessLevel::Free => 2,
            }
        }
        if order(other) < order(self) {
            other
        } else {
            self
        }
    }
}

impl fmt::Display for AccessLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// The 9 types of v1 (R5). Custom types are left for v2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryType {
    Login,
    Database,
    Server,
    Api,
    Aws,
    Smtp,
    FtpSftp,
    SecureNote,
    Generic,
}

impl EntryType {
    /// All of them, in the order of the scope.
    pub const ALL: [EntryType; 9] = [
        EntryType::Login,
        EntryType::Database,
        EntryType::Server,
        EntryType::Api,
        EntryType::Aws,
        EntryType::Smtp,
        EntryType::FtpSftp,
        EntryType::SecureNote,
        EntryType::Generic,
    ];

    /// Stable identifier, without spaces or accents (the one in
    /// `dev/fixtures/entries.json`; written in the vault body v5, m2:DT4). The v4 names
    /// are read only by `body::legacy_v4`.
    pub fn name(self) -> &'static str {
        match self {
            EntryType::Login => "login",
            EntryType::Database => "database",
            EntryType::Server => "server",
            EntryType::Api => "api",
            EntryType::Aws => "aws",
            EntryType::Smtp => "smtp",
            EntryType::FtpSftp => "ftp-sftp",
            EntryType::SecureNote => "secure-note",
            EntryType::Generic => "generic",
        }
    }

    /// Interface text (pt-BR; it does not follow [`EntryType::name`]).
    pub fn label(self) -> &'static str {
        match self {
            EntryType::Login => "login",
            EntryType::Database => "banco",
            EntryType::Server => "servidor",
            EntryType::Api => "api",
            EntryType::Aws => "aws",
            EntryType::Smtp => "smtp",
            EntryType::FtpSftp => "ftp/sftp",
            EntryType::SecureNote => "nota segura",
            EntryType::Generic => "genérico",
        }
    }

    /// Reads the exact identifier.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.name() == name)
    }
}

impl fmt::Display for EntryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Engine of the database type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Engine {
    Postgres,
    Mysql,
    Mongo,
    Redis,
    Sqlserver,
}

impl Engine {
    /// All of them, in the order of the scope.
    pub const ALL: [Engine; 5] = [
        Engine::Postgres,
        Engine::Mysql,
        Engine::Mongo,
        Engine::Redis,
        Engine::Sqlserver,
    ];

    /// Identifier.
    pub fn name(self) -> &'static str {
        match self {
            Engine::Postgres => "postgres",
            Engine::Mysql => "mysql",
            Engine::Mongo => "mongo",
            Engine::Redis => "redis",
            Engine::Sqlserver => "sqlserver",
        }
    }

    /// Reads the exact identifier.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == name)
    }

    /// Default port, used when the form brings no port.
    pub fn default_port(self) -> u16 {
        match self {
            Engine::Postgres => 5432,
            Engine::Mysql => 3306,
            Engine::Mongo => 27017,
            Engine::Redis => 6379,
            Engine::Sqlserver => 1433,
        }
    }
}

/// Connection security of the `smtp` type (`SMTP_SECURITY`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SmtpSecurity {
    None,
    Starttls,
    Tls,
}

impl SmtpSecurity {
    /// All of them.
    pub const ALL: [SmtpSecurity; 3] = [
        SmtpSecurity::None,
        SmtpSecurity::Starttls,
        SmtpSecurity::Tls,
    ];

    /// Identifier (the value of `SMTP_SECURITY`; written in the vault body v5, m2:DT4).
    /// The v4 name of [`SmtpSecurity::None`] is read only by `body::legacy_v4`.
    pub fn name(self) -> &'static str {
        match self {
            SmtpSecurity::None => "none",
            SmtpSecurity::Starttls => "starttls",
            SmtpSecurity::Tls => "tls",
        }
    }

    /// Reads the exact identifier.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == name)
    }

    /// Default port of each mode.
    pub fn default_port(self) -> u16 {
        match self {
            SmtpSecurity::None => 25,
            SmtpSecurity::Starttls => 587,
            SmtpSecurity::Tls => 465,
        }
    }
}

/// Protocol of the `ftp/sftp` type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
    Ftp,
    Sftp,
}

impl Protocol {
    /// Identifier (the value of `WARD_PROTOCOL`).
    pub fn name(self) -> &'static str {
        match self {
            Protocol::Ftp => "ftp",
            Protocol::Sftp => "sftp",
        }
    }

    /// Reads the exact identifier.
    pub fn from_name(name: &str) -> Option<Self> {
        [Protocol::Ftp, Protocol::Sftp]
            .into_iter()
            .find(|p| p.name() == name)
    }

    /// Default port.
    pub fn default_port(self) -> u16 {
        match self {
            Protocol::Ftp => 21,
            Protocol::Sftp => 22,
        }
    }
}

/// Password **or** key (with optional passphrase), never both (V2.12).
#[derive(Debug)]
pub enum Auth {
    Password(Secret),
    Key {
        private_key: Secret,
        passphrase: Option<Secret>,
    },
}

/// `login`: URL, username, password and optional TOTP seed.
#[derive(Debug)]
pub struct Login {
    pub url: String,
    pub username: String,
    pub password: Secret,
    pub totp_seed: Option<Secret>,
}

/// Database: engine, host, port, database name, username, password and optional SSH
/// tunnel (name of a server entry).
#[derive(Debug)]
pub struct Database {
    pub engine: Engine,
    pub host: String,
    pub port: u16,
    pub db_name: Option<String>,
    pub username: String,
    pub password: Secret,
    pub tunnel: Option<String>,
}

/// Server: host, port, username, password or key and optional jump host (name of
/// another server entry).
#[derive(Debug)]
pub struct Server {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: Auth,
    pub jump: Option<String>,
}

/// `api`: base URL, token and the extra variable that receives the token (e.g.
/// `GITHUB_TOKEN`).
#[derive(Debug)]
pub struct Api {
    pub url: Option<String>,
    pub token: Secret,
    pub variable: Option<String>,
}

/// `aws`: the access key ID is an identifier, not a secret (M0 decision).
#[derive(Debug)]
pub struct Aws {
    pub access_key_id: String,
    pub secret_access_key: Secret,
    pub region: String,
    pub role_arn: Option<String>,
    pub session_token: Option<Secret>,
}

/// `smtp`.
#[derive(Debug)]
pub struct Smtp {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Secret,
    pub security: SmtpSecurity,
}

/// `ftp/sftp`. Key only with `sftp`.
#[derive(Debug)]
pub struct FtpSftp {
    pub protocol: Protocol,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: Auth,
}

/// Secure note: only the text, no delivery.
#[derive(Debug)]
pub struct SecureNote {
    pub text: Secret,
}

/// The fields specific to each type.
#[derive(Debug)]
pub enum Data {
    Login(Login),
    Database(Database),
    Server(Server),
    Api(Api),
    Aws(Aws),
    Smtp(Smtp),
    FtpSftp(FtpSftp),
    SecureNote(SecureNote),
    /// Only extra fields.
    Generic,
}

impl Data {
    /// The type of this data.
    pub fn kind(&self) -> EntryType {
        match self {
            Data::Login(_) => EntryType::Login,
            Data::Database(_) => EntryType::Database,
            Data::Server(_) => EntryType::Server,
            Data::Api(_) => EntryType::Api,
            Data::Aws(_) => EntryType::Aws,
            Data::Smtp(_) => EntryType::Smtp,
            Data::FtpSftp(_) => EntryType::FtpSftp,
            Data::SecureNote(_) => EntryType::SecureNote,
            Data::Generic => EntryType::Generic,
        }
    }
}

/// Value of an extra field: plain text or secret.
#[derive(Debug)]
pub enum ExtraValue {
    Text(String),
    Secret(Secret),
}

/// Extra field (name, value, secret yes/no). Becomes `WARD_<NAME>` (D10).
#[derive(Debug)]
pub struct ExtraField {
    name: String,
    value: ExtraValue,
}

impl ExtraField {
    /// Name as the user typed it.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Value.
    pub fn value(&self) -> &ExtraValue {
        &self.value
    }

    /// Is it a secret?
    pub fn is_secret(&self) -> bool {
        matches!(self.value, ExtraValue::Secret(_))
    }
}

/// Attachment encrypted in the vault. The whole content is secret (V2.13). Becomes
/// `WARD_FILE_<NAME>` (D10, m2:DT11).
#[derive(Debug)]
pub struct Attachment {
    name: String,
    content: Secret,
}

impl Attachment {
    /// File name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Content (secret).
    pub fn content(&self) -> &Secret {
        &self.content
    }
}

/// Where a secret value of the entry comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretOrigin<'a> {
    /// Field of the type, by its fixed label (e.g. the password or the TOTP seed).
    Field(&'static str),
    /// Extra field marked as secret, by name.
    Extra(&'a str),
    /// Attachment, by name.
    Attachment(&'a str),
}

/// A secret value of the entry and where it comes from.
#[derive(Debug)]
pub struct EntrySecret<'a> {
    pub origin: SecretOrigin<'a>,
    pub value: &'a Secret,
}

/// Creation and modification dates of the entry (D65 d, provisional), in UTC seconds
/// since 1970.
///
/// - Only the daemon stamps them, by its own clock: neither the [`Draft`] nor the client
///   protocol carry a date. No security decision uses these dates (the rollback is D39,
///   by the generation).
/// - Rule (checked on construction and when reading the body): each date goes from 1 to
///   [`crate::limits::MAX_DATE`]; with `created_at`, `modified_at` exists and is not
///   smaller. An entry without dates (the one from `make dev-vault`, for example) may get
///   only the modification date when it is edited: the creation stays unknown.
/// - Outside the crate it is read only: the value is born from an [`Entry`] or from
///   [`Dates::new_stamp`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dates {
    created_at: Option<u64>,
    modified_at: Option<u64>,
}

impl Dates {
    /// No date at all.
    pub const NONE: Dates = Dates {
        created_at: None,
        modified_at: None,
    };

    /// A valid instant to stamp (from 1 to `MAX_DATE`), or `None`.
    pub fn instant(seconds: u64) -> Option<u64> {
        (1..=crate::limits::MAX_DATE)
            .contains(&seconds)
            .then_some(seconds)
    }

    /// Builds and checks the rule. `None` when it does not hold.
    pub(crate) fn from_parts(created_at: Option<u64>, modified_at: Option<u64>) -> Option<Dates> {
        let valid = |d: Option<u64>| d.is_none_or(|s| Self::instant(s).is_some());
        let order = match (created_at, modified_at) {
            (Some(c), Some(m)) => m >= c,
            (Some(_), None) => false,
            (None, _) => true,
        };
        (valid(created_at) && valid(modified_at) && order).then_some(Dates {
            created_at,
            modified_at,
        })
    }

    /// Creation, if known.
    pub fn created_at(self) -> Option<u64> {
        self.created_at
    }

    /// Last modification, if known.
    pub fn modified_at(self) -> Option<u64> {
        self.modified_at
    }

    /// The dates of a new entry: both at `now`. `None` with `now` out of range.
    pub fn new_stamp(now: u64) -> Option<Dates> {
        Self::from_parts(Some(now), Some(now))
    }

    /// The dates after a modification: the creation stays, and `modified_at` becomes
    /// `now`, never going down (a clock that goes back does not make the date go back).
    /// `None` with `now` out of range.
    pub fn modified(self, now: u64) -> Option<Dates> {
        let now = Self::instant(now)?;
        let modified = [Some(now), self.modified_at, self.created_at]
            .into_iter()
            .flatten()
            .max();
        Self::from_parts(self.created_at, modified)
    }
}

/// Validated vault entry. No `Clone`; the `Debug` shows the secrets as `••••`.
#[derive(Debug)]
pub struct Entry {
    name: String,
    group: String,
    environment: Environment,
    project: Option<String>,
    tags: Vec<String>,
    level: AccessLevel,
    dates: Dates,
    data: Data,
    extras: Vec<ExtraField>,
    attachments: Vec<Attachment>,
}

// Every secret value of the entry is in a `Secret`, which is zeroed on drop; the other
// fields are not secret. The marker holds for the whole entry (V2.28).
impl ZeroizeOnDrop for Entry {}
impl ZeroizeOnDrop for Data {}
impl ZeroizeOnDrop for ExtraField {}
impl ZeroizeOnDrop for Attachment {}
impl ZeroizeOnDrop for Auth {}

impl Entry {
    /// Unique name of the entry (used in `ward run <entry>`).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Group (folder), required.
    pub fn group(&self) -> &str {
        &self.group
    }

    /// Environment, required.
    pub fn environment(&self) -> Environment {
        self.environment
    }

    /// Project, optional.
    pub fn project(&self) -> Option<&str> {
        self.project.as_deref()
    }

    /// Tags, optional.
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Access level for the agent.
    pub fn level(&self) -> AccessLevel {
        self.level
    }

    /// Changes the level. **Only the daemon calls it, after checking the PIN and the app
    /// role (S9).**
    pub fn set_level(&mut self, level: AccessLevel) {
        self.level = level;
    }

    /// Creation and modification dates (D65 d).
    pub fn dates(&self) -> Dates {
        self.dates
    }

    /// Changes the dates. **Only the daemon calls it**, with dates taken from its own
    /// clock ([`Dates::new_stamp`], [`Dates::modified`]) or with the ones the entry
    /// already had (to undo a write that failed). A [`Dates`] only exists within the rule.
    pub fn set_dates(&mut self, dates: Dates) {
        self.dates = dates;
    }

    /// Type.
    pub fn kind(&self) -> EntryType {
        self.data.kind()
    }

    /// Fields specific to the type.
    pub fn data(&self) -> &Data {
        &self.data
    }

    /// Extra fields.
    pub fn extras(&self) -> &[ExtraField] {
        &self.extras
    }

    /// Attachments.
    pub fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }

    /// All the secret values of the entry (V2.13): the secret fields of the type, the
    /// extras marked as secret and all the attachments. It is the list the mask (D11)
    /// covers.
    pub fn secrets(&self) -> Vec<EntrySecret<'_>> {
        let mut list = Vec::new();
        type_secrets(&self.data, &mut list);
        for extra in &self.extras {
            if let ExtraValue::Secret(value) = &extra.value {
                list.push(EntrySecret {
                    origin: SecretOrigin::Extra(&extra.name),
                    value,
                });
            }
        }
        for attachment in &self.attachments {
            list.push(EntrySecret {
                origin: SecretOrigin::Attachment(&attachment.name),
                value: &attachment.content,
            });
        }
        list
    }
}

fn type_secrets<'a>(data: &'a Data, list: &mut Vec<EntrySecret<'a>>) {
    fn field<'a>(list: &mut Vec<EntrySecret<'a>>, name: &'static str, value: &'a Secret) {
        list.push(EntrySecret {
            origin: SecretOrigin::Field(name),
            value,
        });
    }
    fn auth<'a>(list: &mut Vec<EntrySecret<'a>>, a: &'a Auth) {
        match a {
            Auth::Password(password) => field(list, "senha", password),
            Auth::Key {
                private_key,
                passphrase,
            } => {
                field(list, "chave privada", private_key);
                if let Some(p) = passphrase {
                    field(list, "passphrase", p);
                }
            }
        }
    }
    match data {
        Data::Login(l) => {
            field(list, "senha", &l.password);
            if let Some(s) = &l.totp_seed {
                field(list, "semente TOTP", s);
            }
        }
        Data::Database(d) => field(list, "senha", &d.password),
        Data::Server(s) => auth(list, &s.auth),
        Data::Api(a) => field(list, "token", &a.token),
        Data::Aws(a) => {
            field(list, "secret access key", &a.secret_access_key);
            if let Some(t) = &a.session_token {
                field(list, "session token", t);
            }
        }
        Data::Smtp(s) => field(list, "senha", &s.password),
        Data::FtpSftp(f) => auth(list, &f.auth),
        Data::SecureNote(n) => field(list, "texto", &n.text),
        Data::Generic => {}
    }
}

/// Summary without secrets: `name (type, group, environment)`.
impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}, {}, {})",
            self.name,
            self.kind(),
            self.group,
            self.environment
        )
    }
}
