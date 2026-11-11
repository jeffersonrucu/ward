//! Validation of what the app (or the extension, in "save new login") sends to save
//! (V2.12).
//!
//! A [`Draft`] is the form as it arrived: raw texts and secrets already in [`Secret`].
//! [`Draft::validate`] checks everything, gathers **all** the errors and only then returns
//! the [`Entry`]. If it fails, the draft is discarded and the secrets are zeroed.
//!
//! The errors only have fixed texts (`&'static str`): no typed value goes into a message,
//! not even the ones that are not secret.

use core::fmt;

use super::{
    AccessLevel, Api, Attachment, Auth, Aws, Data, Database, Dates, Engine, Entry, EntryType,
    Environment, ExtraField, ExtraValue, FtpSftp, Login, Protocol, Secret, SecureNote, Server,
    Smtp, SmtpSecurity,
};

// Limits (D25, D29): in a single source, `crate::limits` (D32). Everything that comes from
// the socket is untrusted (size, format).
use crate::limits::{
    MAX_ACCESS_KEY, MAX_ATTACHMENT_NAME, MAX_ATTACHMENTS, MAX_ATTACHMENTS_TOTAL, MAX_DB_NAME,
    MAX_EXTRA_NAME, MAX_EXTRA_VALUE, MAX_EXTRAS, MAX_HOST, MAX_LINE, MAX_NAME, MAX_NOTE,
    MAX_REDIS_DB_NAME, MAX_REGION, MAX_SECRET, MAX_TAG, MAX_TAGS, MAX_TOTP, MAX_URL, MAX_USERNAME,
    MIN_ACCESS_KEY, MIN_TOTP,
};
use crate::mascara::{self, ErroMascara};

// Messages (fixed texts, without user values).
const REQUIRED: &str = "obrigatório";
const TOO_LONG: &str = "longo demais";
/// Text outside the allow list of the field (D38).
const CHARACTER: &str = "caractere não permitido";
const SPACE_AT_ENDS: &str = "não pode começar nem terminar com espaço";
const NAME_FORMAT: &str =
    "use letras minúsculas, números, '.', '_' ou '-', começando por letra ou número";
const ENVIRONMENT: &str = "deve ser dev, stage ou prod";
const ENGINE: &str = "fora da lista: postgres, mysql, mongo, redis ou sqlserver";
const PORT: &str = "deve ser um número de 1 a 65535";
const HOST: &str = "use letras, números, '.', '-', '_', ':' ou colchetes, sem começar por '-'";
const USERNAME: &str = "use só ASCII visível, sem espaço e sem começar por '-'";
const SSH_USERNAME: &str =
    "use letras sem acento, números, '.', '_', '@' ou '-', sem começar por '-'";
const URL: &str = "use https:// (http:// só para localhost ou 127.0.0.1), com host válido em \
     minúsculas, sem usuário na URL, porta de 1 a 65535 e só ASCII visível";
const PASSWORD_AND_KEY: &str = "use senha ou chave, não as duas";
const NO_AUTH: &str = "informe a senha ou a chave";
const PASSPHRASE_WITHOUT_KEY: &str = "passphrase só vale com chave";
const FTP_WITH_KEY: &str = "ftp não aceita chave; use sftp ou senha";
const KEY_FORMAT: &str = "chave privada em formato PEM ou OpenSSH (-----BEGIN … PRIVATE KEY-----)";
const TOTP: &str = "semente TOTP em base32 (A–Z e 2–7), com 16 a 128 caracteres";
const NUL: &str = "não pode ter o caractere nulo";
const NOT_TEXT: &str = "precisa ser texto (UTF-8)";
const SECURITY: &str = "deve ser nenhuma, starttls ou tls";
const PROTOCOL: &str = "deve ser ftp ou sftp";
const VARIABLE: &str = "use maiúsculas, números e '_', começando por letra";
const VARIABLE_SUFFIX: &str =
    "a variável precisa terminar em _TOKEN, _KEY, _SECRET, _PASSWORD, _PAT ou _AUTH";
const FORBIDDEN_VARIABLE: &str =
    "variável reservada (LD_*, WARD_* ou variável que muda o comportamento do sistema)";
const ACCESS_KEY: &str = "use só letras maiúsculas e números (16 a 128)";
const REGION: &str = "use letras minúsculas, números e '-' (ex.: sa-east-1)";
const ARN: &str = "deve começar com arn: e ter só ASCII visível, sem espaço";
const TAG: &str = "tag sem espaço nem vírgula, até 32 caracteres";
const DUPLICATE_TAG: &str = "tag repetida";
const TOO_MANY_ITEMS: &str = "itens demais";
const GENERIC_WITHOUT_EXTRAS: &str = "o tipo genérico precisa de pelo menos um campo extra";
const NO_LETTER: &str = "precisa ter letra ou número";
const DUPLICATE_EXTRA: &str = "vira a mesma variável de outro campo extra";
const RESERVED_EXTRA: &str = "vira uma variável que o tipo já entrega";
const DUPLICATE_ATTACHMENT: &str = "vira a mesma variável de outro anexo";
const RESERVED_ATTACHMENT: &str = "vira uma variável que o tipo já entrega";
const ATTACHMENT_NAME: &str = "nome de arquivo sem '/', sem começar por '.' ou '-'";
const EMPTY_ATTACHMENT: &str = "anexo vazio";
const ATTACHMENTS_TOTAL: &str = "os anexos somam mais de 256 KiB";
const DB_NAME: &str = "use letras, números, '_' ou '-', com até 63 caracteres";
const REDIS_DB_NAME: &str = "no redis, a base é um número de 0 a 99999";
/// Message of D29 (V2.36).
pub const TOO_LARGE_FOR_MASK: &str = "entrada grande demais para a máscara do `ward run`";
const NO_DELIVERY: &str = "a entrega desta entrada não pôde ser montada";
const TYPE_CHANGED: &str = "o tipo não muda depois de criado";
const NAME_IN_USE: &str = "já existe outra entrada com este nome";
const SELF_REFERENCE: &str = "aponta para a própria entrada";
const NOT_A_SERVER: &str = "precisa ser uma entrada servidor existente";
const JUMP_WITH_JUMP: &str = "o servidor de salto não pode ter salto próprio";
const TUNNEL_WITH_JUMP: &str = "o servidor do túnel não pode ter salto";
const USED_AS_JUMP: &str =
    "outra entrada usa este servidor como salto ou túnel; ele não pode ter salto";

/// Variables that the core delivers for each type (suffix after `WARD_`, D10). An extra
/// field that normalizes to one of these names is refused. Phase 16 (delivery) must keep
/// this list equal to the `WARD_*` variables it builds.
pub(crate) fn type_variables(kind: EntryType) -> &'static [&'static str] {
    match kind {
        EntryType::Login => &["URL", "USER", "PASS", "TOTP"],
        EntryType::Server => &["HOST", "PORT", "USER", "PASS", "PASSPHRASE"],
        EntryType::Api => &["URL", "TOKEN"],
        EntryType::FtpSftp => &["PROTOCOLO", "HOST", "PORT", "USER", "PASS", "PASSPHRASE"],
        EntryType::Database
        | EntryType::Aws
        | EntryType::Smtp
        | EntryType::SecureNote
        | EntryType::Generic => &[],
    }
}

/// Variables of every type (`WARD_ENTRADA`, `WARD_GRUPO`, `WARD_AMBIENTE`).
pub(crate) const COMMON_VARIABLES: [&str; 3] = ["ENTRADA", "GRUPO", "AMBIENTE"];

/// Attachments that the type already delivers (suffix after `WARD_ARQUIVO_`): the key of
/// server and of `ftp/sftp` becomes `WARD_ARQUIVO_CHAVE`.
pub(crate) fn type_attachments(kind: EntryType) -> &'static [&'static str] {
    match kind {
        EntryType::Server | EntryType::FtpSftp => &["CHAVE"],
        _ => &[],
    }
}

// Variables that change the behavior of the system, of the shell or of tools that
// `ward run`/`ward ssh` call. The `api` cannot put the token in one of them.
const SYSTEM_VARIABLES: &[&str] = &[
    "PATH",
    "HOME",
    "SHELL",
    "USER",
    "LOGNAME",
    "PWD",
    "OLDPWD",
    "IFS",
    "ENV",
    "BASH_ENV",
    "BASHOPTS",
    "SHELLOPTS",
    "PS1",
    "PS2",
    "PS4",
    "PROMPT_COMMAND",
    "CDPATH",
    "GLOBIGNORE",
    "TMPDIR",
    "TERM",
    "LANG",
    "LANGUAGE",
    "SSHPASS",
    "SSH_AUTH_SOCK",
    "SSH_ASKPASS",
    "SSH_ASKPASS_REQUIRE",
    "GIT_SSH",
    "GIT_SSH_COMMAND",
    "GIT_ASKPASS",
    "GIT_EXEC_PATH",
    "GIT_CONFIG",
    "GIT_CONFIG_GLOBAL",
    "PYTHONPATH",
    "PYTHONSTARTUP",
    "PYTHONHOME",
    "NODE_OPTIONS",
    "NODE_PATH",
    "PERL5OPT",
    "PERL5LIB",
    "RUBYOPT",
    "RUBYLIB",
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
];
const FORBIDDEN_PREFIXES: &[&str] = &["LD_", "WARD_", "BASH_FUNC_", "GCONV_", "MALLOC_"];

/// Variable name from the name of an extra field or attachment (D10): ASCII letters and
/// digits in uppercase; the rest becomes `_`. E.g. `api-key` → `API_KEY`,
/// `access key.pem` → `ACCESS_KEY_PEM`.
pub fn variable_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

/// A validation problem: field, position in the list (extra fields, attachments, tags)
/// and reason. Only fixed texts; no user value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidationError {
    pub field: &'static str,
    /// Position (starting at 1) when the field is a list.
    pub position: Option<usize>,
    pub reason: &'static str,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.position {
            Some(n) => write!(f, "{} {}: {}", self.field, n, self.reason),
            None => write!(f, "{}: {}", self.field, self.reason),
        }
    }
}

/// All the problems found. Never empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationErrors(Vec<ValidationError>);

impl ValidationErrors {
    /// The problems, in the order of the form.
    pub fn list(&self) -> &[ValidationError] {
        &self.0
    }

    /// Any problem in this field?
    pub fn has(&self, field: &str) -> bool {
        self.0.iter().any(|e| e.field == field)
    }

    /// Any problem in this field with this reason?
    pub fn has_reason(&self, field: &str, reason: &str) -> bool {
        self.0
            .iter()
            .any(|e| e.field == field && e.reason == reason)
    }
}

impl fmt::Display for ValidationErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("entrada inválida: ")?;
        for (i, e) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str("; ")?;
            }
            write!(f, "{e}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ValidationErrors {}

/// Extra field as it came from the form. The value already arrives in a [`Secret`] (even
/// when it is not secret), so as not to leave a copy unzeroed if the validation fails.
pub struct ExtraDraft {
    pub name: String,
    pub value: Secret,
    pub secret: bool,
}

/// Attachment as it came from the form.
pub struct AttachmentDraft {
    pub name: String,
    pub content: Secret,
}

/// Fields of the type as they came from the form. Empty text and empty secret count as
/// "not given". An empty port uses the default of the engine, protocol or security.
pub enum DataDraft {
    Login {
        url: String,
        username: String,
        password: Option<Secret>,
        totp_seed: Option<Secret>,
    },
    Database {
        engine: String,
        host: String,
        port: String,
        db_name: String,
        username: String,
        password: Option<Secret>,
        tunnel: String,
    },
    Server {
        host: String,
        port: String,
        username: String,
        password: Option<Secret>,
        private_key: Option<Secret>,
        passphrase: Option<Secret>,
        jump: String,
    },
    Api {
        url: String,
        token: Option<Secret>,
        variable: String,
    },
    Aws {
        access_key_id: String,
        secret_access_key: Option<Secret>,
        region: String,
        role_arn: String,
        session_token: Option<Secret>,
    },
    Smtp {
        host: String,
        port: String,
        username: String,
        password: Option<Secret>,
        security: String,
    },
    FtpSftp {
        protocol: String,
        host: String,
        port: String,
        username: String,
        password: Option<Secret>,
        private_key: Option<Secret>,
        passphrase: Option<Secret>,
    },
    SecureNote {
        text: Option<Secret>,
    },
    Generic,
}

impl DataDraft {
    /// The type this draft creates.
    pub fn kind(&self) -> EntryType {
        match self {
            DataDraft::Login { .. } => EntryType::Login,
            DataDraft::Database { .. } => EntryType::Database,
            DataDraft::Server { .. } => EntryType::Server,
            DataDraft::Api { .. } => EntryType::Api,
            DataDraft::Aws { .. } => EntryType::Aws,
            DataDraft::Smtp { .. } => EntryType::Smtp,
            DataDraft::FtpSftp { .. } => EntryType::FtpSftp,
            DataDraft::SecureNote { .. } => EntryType::SecureNote,
            DataDraft::Generic => EntryType::Generic,
        }
    }
}

/// The whole form as it arrived. No level: a new entry always starts "with approval"
/// (S9), and changing the level is another operation, which asks for the PIN.
pub struct Draft {
    pub name: String,
    pub group: String,
    pub environment: String,
    pub project: String,
    pub tags: Vec<String>,
    pub data: DataDraft,
    pub extras: Vec<ExtraDraft>,
    pub attachments: Vec<AttachmentDraft>,
}

impl fmt::Debug for ExtraDraft {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtraDraft")
            .field("name", &self.name)
            .field("value", &self.value)
            .field("secret", &self.secret)
            .finish()
    }
}

impl fmt::Debug for AttachmentDraft {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AttachmentDraft")
            .field("name", &self.name)
            .field("content", &self.content)
            .finish()
    }
}

impl fmt::Debug for DataDraft {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Only the type: the raw texts can be anything and the secrets already print
        // `••••`, but there is no reason to log a form.
        write!(f, "DataDraft({})", self.kind().name())
    }
}

impl fmt::Debug for Draft {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Draft")
            .field("kind", &self.data.kind().name())
            .finish_non_exhaustive()
    }
}

/// Gathers the errors in the order of the form.
struct Collector(Vec<ValidationError>);

impl Collector {
    fn error(&mut self, field: &'static str, reason: &'static str) {
        self.0.push(ValidationError {
            field,
            position: None,
            reason,
        });
    }

    fn error_at(&mut self, field: &'static str, index: usize, reason: &'static str) {
        self.0.push(ValidationError {
            field,
            position: Some(index + 1),
            reason,
        });
    }

    /// Required one-line text.
    fn line(&mut self, field: &'static str, value: String, max: usize) -> Option<String> {
        match line_reason(&value, max) {
            Some(m) => {
                self.error(field, m);
                None
            }
            None => Some(value),
        }
    }

    /// Optional one-line text (empty = absent).
    fn optional_line(
        &mut self,
        field: &'static str,
        value: String,
        max: usize,
    ) -> Result<Option<String>, ()> {
        if value.is_empty() {
            return Ok(None);
        }
        self.line(field, value, max).map(Some).ok_or(())
    }

    /// Required text that goes through a format rule.
    fn format(
        &mut self,
        field: &'static str,
        value: String,
        max: usize,
        rule: fn(&str) -> bool,
        reason: &'static str,
    ) -> Option<String> {
        let value = self.line(field, value, max)?;
        if rule(&value) {
            Some(value)
        } else {
            self.error(field, reason);
            None
        }
    }

    /// Optional with a format rule.
    fn optional_format(
        &mut self,
        field: &'static str,
        value: String,
        max: usize,
        rule: fn(&str) -> bool,
        reason: &'static str,
    ) -> Result<Option<String>, ()> {
        if value.is_empty() {
            return Ok(None);
        }
        self.format(field, value, max, rule, reason)
            .map(Some)
            .ok_or(())
    }

    fn host(&mut self, value: String) -> Option<String> {
        self.format("host", value, MAX_HOST, valid_host, HOST)
    }

    /// Username of the database and of `smtp`: technical field, visible ASCII only (D38).
    fn strict_username(&mut self, value: String) -> Option<String> {
        self.format("usuário", value, MAX_USERNAME, valid_username, USERNAME)
    }

    /// Username of the server and of `ftp/sftp` (D43): `[A-Za-z0-9._@-]`, no `-` at the
    /// start. It goes to `ssh`/`sftp`; no `%`, `;`, quotes or space.
    fn ssh_username(&mut self, value: String) -> Option<String> {
        self.format(
            "usuário",
            value,
            MAX_USERNAME,
            valid_ssh_username,
            SSH_USERNAME,
        )
    }

    fn url(&mut self, value: String) -> Option<String> {
        self.format("url", value, MAX_URL, valid_url, URL)
    }

    fn port(&mut self, value: &str, default: Option<u16>) -> Option<u16> {
        if value.is_empty() {
            if default.is_none() {
                self.error("porta", REQUIRED);
            }
            return default;
        }
        let ok = value.len() <= 5 && value.bytes().all(|b| b.is_ascii_digit());
        match value.parse::<u16>() {
            Ok(p) if ok && p != 0 => Some(p),
            _ => {
                self.error("porta", PORT);
                None
            }
        }
    }

    /// Required text secret (empty = absent).
    fn secret(&mut self, field: &'static str, value: Option<Secret>, max: usize) -> Option<Secret> {
        match self.optional_secret(field, value, max) {
            Ok(Some(s)) => Some(s),
            Ok(None) => {
                self.error(field, REQUIRED);
                None
            }
            Err(()) => None,
        }
    }

    /// Optional text secret: UTF-8 text, without NUL (it goes into the environment), up
    /// to `max`.
    fn optional_secret(
        &mut self,
        field: &'static str,
        value: Option<Secret>,
        max: usize,
    ) -> Result<Option<Secret>, ()> {
        let Some(value) = value.filter(|s| !s.is_empty()) else {
            return Ok(None);
        };
        match secret_reason(&value, max) {
            Some(m) => {
                self.error(field, m);
                Err(())
            }
            None => Ok(Some(value)),
        }
    }

    /// Password **or** key (+ passphrase), V2.12.
    fn auth(
        &mut self,
        password: Option<Secret>,
        key: Option<Secret>,
        passphrase: Option<Secret>,
        accepts_key: bool,
    ) -> Option<Auth> {
        let password = self.optional_secret("senha", password, MAX_SECRET);
        let key = self.optional_secret("chave privada", key, MAX_SECRET);
        let passphrase = self.optional_secret("passphrase", passphrase, MAX_SECRET);
        let (Ok(password), Ok(key), Ok(passphrase)) = (password, key, passphrase) else {
            return None;
        };
        match (password, key) {
            (Some(_), Some(_)) => {
                self.error("autenticação", PASSWORD_AND_KEY);
                None
            }
            (None, None) => {
                self.error("autenticação", NO_AUTH);
                None
            }
            (Some(password), None) => {
                if passphrase.is_some() {
                    self.error("passphrase", PASSPHRASE_WITHOUT_KEY);
                    return None;
                }
                Some(Auth::Password(password))
            }
            (None, Some(key)) => {
                if !accepts_key {
                    self.error("autenticação", FTP_WITH_KEY);
                    return None;
                }
                if !looks_like_private_key(&key) {
                    self.error("chave privada", KEY_FORMAT);
                    return None;
                }
                Some(Auth::Key {
                    private_key: key,
                    passphrase,
                })
            }
        }
    }
}

impl Draft {
    /// Checks the whole form. Returns the entry with level "with approval" or all the
    /// problems found.
    pub fn validate(self) -> Result<Entry, ValidationErrors> {
        let mut c = Collector(Vec::new());
        let kind = self.data.kind();

        let name = c.format("nome", self.name, MAX_NAME, valid_entry_name, NAME_FORMAT);
        let group = c.line("grupo", self.group, MAX_LINE);
        let environment = if self.environment.is_empty() {
            c.error("ambiente", REQUIRED);
            None
        } else {
            let a = Environment::from_name(&self.environment);
            if a.is_none() {
                c.error("ambiente", ENVIRONMENT);
            }
            a
        };
        let project = c.optional_line("projeto", self.project, MAX_LINE);
        let tags = validate_tags(&mut c, self.tags);
        let data = validate_data(&mut c, self.data);
        let extras = validate_extras(&mut c, kind, self.extras);
        let attachments = validate_attachments(&mut c, kind, self.attachments);
        if let Some(extras) = &extras
            && kind == EntryType::Generic
            && extras.is_empty()
        {
            c.error("campos extras", GENERIC_WITHOUT_EXTRAS);
        }

        match (
            name,
            group,
            environment,
            project,
            tags,
            data,
            extras,
            attachments,
        ) {
            (
                Some(name),
                Some(group),
                Some(environment),
                Ok(project),
                Some(tags),
                Some(data),
                Some(extras),
                Some(attachments),
            ) if c.0.is_empty() => {
                let entry = Entry {
                    name,
                    group,
                    environment,
                    project,
                    tags,
                    level: AccessLevel::Approval,
                    // D65 d: only the daemon stamps (its own clock); the draft has no date.
                    dates: Dates::NONE,
                    data,
                    extras,
                    attachments,
                };
                // Mask budget (D29), by the same function that builds the automaton: no
                // accepted entry is left without `ward run`. If it refuses, the entry is
                // discarded here and the secrets are zeroed.
                match mascara::custo_da_entrada(&entry) {
                    Ok(_) => Ok(entry),
                    Err(e) => {
                        let reason = match e {
                            ErroMascara::GrandeDemais => TOO_LARGE_FOR_MASK,
                            ErroMascara::SemEntrega => NO_DELIVERY,
                        };
                        c.error("entrada", reason);
                        Err(ValidationErrors(c.0))
                    }
                }
            }
            _ => {
                debug_assert!(!c.0.is_empty());
                Err(ValidationErrors(c.0))
            }
        }
    }
}

fn validate_tags(c: &mut Collector, tags: Vec<String>) -> Option<Vec<String>> {
    if tags.len() > MAX_TAGS {
        c.error("tags", TOO_MANY_ITEMS);
        return None;
    }
    let before = c.0.len();
    let mut out: Vec<String> = Vec::with_capacity(tags.len());
    for (i, tag) in tags.into_iter().enumerate() {
        if let Some(m) = line_reason(&tag, MAX_TAG) {
            c.error_at("tags", i, m);
        } else if tag.chars().any(|ch| ch.is_whitespace() || ch == ',') {
            c.error_at("tags", i, TAG);
        } else if out.contains(&tag) {
            c.error_at("tags", i, DUPLICATE_TAG);
        } else {
            out.push(tag);
        }
    }
    (c.0.len() == before).then_some(out)
}

fn validate_data(c: &mut Collector, data: DataDraft) -> Option<Data> {
    match data {
        DataDraft::Login {
            url,
            username,
            password,
            totp_seed,
        } => {
            let url = c.url(url);
            let username = c.line("usuário", username, MAX_USERNAME);
            let password = c.secret("senha", password, MAX_SECRET);
            let totp = c.optional_secret("semente TOTP", totp_seed, MAX_TOTP);
            let totp = match totp {
                Ok(Some(s)) if !valid_totp_seed(&s) => {
                    c.error("semente TOTP", TOTP);
                    Err(())
                }
                other => other,
            };
            Some(Data::Login(Login {
                url: url?,
                username: username?,
                password: password?,
                totp_seed: totp.ok()?,
            }))
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
            let engine = if engine.is_empty() {
                c.error("motor", REQUIRED);
                None
            } else {
                let m = Engine::from_name(&engine);
                if m.is_none() {
                    c.error("motor", ENGINE);
                }
                m
            };
            let host = c.host(host);
            let port = c.port(&port, engine.map(Engine::default_port).or(Some(1)));
            let db_name = if engine == Some(Engine::Redis) {
                c.optional_format(
                    "base",
                    db_name,
                    MAX_DB_NAME,
                    valid_redis_db_name,
                    REDIS_DB_NAME,
                )
            } else {
                c.optional_format("base", db_name, MAX_DB_NAME, valid_db_name, DB_NAME)
            };
            let username = c.strict_username(username);
            let password = c.secret("senha", password, MAX_SECRET);
            let tunnel =
                c.optional_format("túnel SSH", tunnel, MAX_NAME, valid_entry_name, NAME_FORMAT);
            Some(Data::Database(Database {
                engine: engine?,
                host: host?,
                port: port?,
                db_name: db_name.ok()?,
                username: username?,
                password: password?,
                tunnel: tunnel.ok()?,
            }))
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
            let host = c.host(host);
            let port = c.port(&port, Some(22));
            let username = c.ssh_username(username);
            let auth = c.auth(password, private_key, passphrase, true);
            let jump = c.optional_format(
                "servidor de salto",
                jump,
                MAX_NAME,
                valid_entry_name,
                NAME_FORMAT,
            );
            Some(Data::Server(Server {
                host: host?,
                port: port?,
                username: username?,
                auth: auth?,
                jump: jump.ok()?,
            }))
        }
        DataDraft::Api {
            url,
            token,
            variable,
        } => {
            let url = if url.is_empty() {
                Ok(None)
            } else {
                c.url(url).map(Some).ok_or(())
            };
            let token = c.secret("token", token, MAX_SECRET);
            let variable =
                c.optional_format("variável", variable, MAX_NAME, valid_variable, VARIABLE);
            // D42: suffix allow list first; the deny list of D25 stays as a second layer.
            let variable = match variable {
                Ok(Some(v)) if !variable_with_suffix(&v) => {
                    c.error("variável", VARIABLE_SUFFIX);
                    Err(())
                }
                Ok(Some(v)) if forbidden_variable(&v) => {
                    c.error("variável", FORBIDDEN_VARIABLE);
                    Err(())
                }
                other => other,
            };
            Some(Data::Api(Api {
                url: url.ok()?,
                token: token?,
                variable: variable.ok()?,
            }))
        }
        DataDraft::Aws {
            access_key_id,
            secret_access_key,
            region,
            role_arn,
            session_token,
        } => {
            let access_key_id = c.format(
                "access key ID",
                access_key_id,
                MAX_ACCESS_KEY,
                valid_access_key,
                ACCESS_KEY,
            );
            let secret = c.secret("secret access key", secret_access_key, MAX_SECRET);
            let region = c.format("região", region, MAX_REGION, valid_region, REGION);
            let role_arn = c.optional_format("role ARN", role_arn, MAX_URL, valid_arn, ARN);
            let session = c.optional_secret("session token", session_token, MAX_SECRET);
            Some(Data::Aws(Aws {
                access_key_id: access_key_id?,
                secret_access_key: secret?,
                region: region?,
                role_arn: role_arn.ok()?,
                session_token: session.ok()?,
            }))
        }
        DataDraft::Smtp {
            host,
            port,
            username,
            password,
            security,
        } => {
            let host = c.host(host);
            let security = if security.is_empty() {
                c.error("segurança", REQUIRED);
                None
            } else {
                let s = SmtpSecurity::from_name(&security);
                if s.is_none() {
                    c.error("segurança", SECURITY);
                }
                s
            };
            let port = c.port(&port, security.map(SmtpSecurity::default_port).or(Some(1)));
            let username = c.strict_username(username);
            let password = c.secret("senha", password, MAX_SECRET);
            Some(Data::Smtp(Smtp {
                host: host?,
                port: port?,
                username: username?,
                password: password?,
                security: security?,
            }))
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
            let protocol = if protocol.is_empty() {
                c.error("protocolo", REQUIRED);
                None
            } else {
                let p = Protocol::from_name(&protocol);
                if p.is_none() {
                    c.error("protocolo", PROTOCOL);
                }
                p
            };
            let host = c.host(host);
            let port = c.port(&port, protocol.map(Protocol::default_port).or(Some(1)));
            let username = c.ssh_username(username);
            // Without a valid protocol, there is no way to know whether the key is
            // allowed: accept it here, and the protocol error already refuses the entry.
            let accepts_key = protocol != Some(Protocol::Ftp);
            let auth = c.auth(password, private_key, passphrase, accepts_key);
            Some(Data::FtpSftp(FtpSftp {
                protocol: protocol?,
                host: host?,
                port: port?,
                username: username?,
                auth: auth?,
            }))
        }
        DataDraft::SecureNote { text } => {
            let text = c.secret("texto", text, MAX_NOTE);
            Some(Data::SecureNote(SecureNote { text: text? }))
        }
        DataDraft::Generic => Some(Data::Generic),
    }
}

fn validate_extras(
    c: &mut Collector,
    kind: EntryType,
    extras: Vec<ExtraDraft>,
) -> Option<Vec<ExtraField>> {
    const FIELD: &str = "campos extras";
    if extras.len() > MAX_EXTRAS {
        c.error(FIELD, TOO_MANY_ITEMS);
        return None;
    }
    let before = c.0.len();
    let mut seen: Vec<String> = Vec::with_capacity(extras.len());
    let mut out = Vec::with_capacity(extras.len());
    for (i, extra) in extras.into_iter().enumerate() {
        let ExtraDraft {
            name,
            value,
            secret,
        } = extra;
        if let Some(m) = line_reason(&name, MAX_EXTRA_NAME) {
            c.error_at(FIELD, i, m);
            continue;
        }
        let var = variable_name(&name);
        if var.bytes().all(|b| b == b'_') {
            c.error_at(FIELD, i, NO_LETTER);
            continue;
        }
        if COMMON_VARIABLES.contains(&var.as_str())
            || type_variables(kind).contains(&var.as_str())
            || var == "ARQUIVO"
            || var.starts_with("ARQUIVO_")
        {
            c.error_at(FIELD, i, RESERVED_EXTRA);
            continue;
        }
        if seen.contains(&var) {
            c.error_at(FIELD, i, DUPLICATE_EXTRA);
            continue;
        }
        seen.push(var);
        if value.is_empty() {
            c.error_at(FIELD, i, REQUIRED);
            continue;
        }
        if let Some(m) = secret_reason(&value, MAX_EXTRA_VALUE) {
            c.error_at(FIELD, i, m);
            continue;
        }
        let value = if secret {
            ExtraValue::Secret(value)
        } else {
            // Not a secret: becomes plain one-line text (shown in the list and in the app).
            let text = value.expose_text().unwrap_or_default();
            if let Some(m) = line_reason(text, MAX_EXTRA_VALUE) {
                c.error_at(FIELD, i, m);
                continue;
            }
            ExtraValue::Text(text.to_owned())
        };
        out.push(ExtraField { name, value });
    }
    (c.0.len() == before).then_some(out)
}

fn validate_attachments(
    c: &mut Collector,
    kind: EntryType,
    attachments: Vec<AttachmentDraft>,
) -> Option<Vec<Attachment>> {
    const FIELD: &str = "anexos";
    if attachments.len() > MAX_ATTACHMENTS {
        c.error(FIELD, TOO_MANY_ITEMS);
        return None;
    }
    let before = c.0.len();
    let mut seen: Vec<String> = Vec::with_capacity(attachments.len());
    let mut out = Vec::with_capacity(attachments.len());
    // Sum of the attachments (D29). Each attachment also stays within the sum limit.
    let mut sum: usize = 0;
    for (i, attachment) in attachments.into_iter().enumerate() {
        let AttachmentDraft { name, content } = attachment;
        if let Some(m) = line_reason(&name, MAX_ATTACHMENT_NAME) {
            c.error_at(FIELD, i, m);
            continue;
        }
        if name.contains(['/', '\\']) || name.starts_with(['.', '-']) {
            c.error_at(FIELD, i, ATTACHMENT_NAME);
            continue;
        }
        let var = variable_name(&name);
        if var.bytes().all(|b| b == b'_') {
            c.error_at(FIELD, i, NO_LETTER);
            continue;
        }
        if type_attachments(kind).contains(&var.as_str()) {
            c.error_at(FIELD, i, RESERVED_ATTACHMENT);
            continue;
        }
        if seen.contains(&var) {
            c.error_at(FIELD, i, DUPLICATE_ATTACHMENT);
            continue;
        }
        seen.push(var);
        if content.is_empty() {
            c.error_at(FIELD, i, EMPTY_ATTACHMENT);
            continue;
        }
        if content.len() > MAX_ATTACHMENTS_TOTAL {
            c.error_at(FIELD, i, TOO_LONG);
            continue;
        }
        sum = sum.saturating_add(content.len());
        out.push(Attachment { name, content });
    }
    if sum > MAX_ATTACHMENTS_TOTAL {
        c.error(FIELD, ATTACHMENTS_TOTAL);
    }
    (c.0.len() == before).then_some(out)
}

impl Entry {
    /// Replaces the content with the edited version (already validated), keeping the
    /// level: editing does not change the level (S9). The type does not change after
    /// creation.
    pub fn replace_with(&mut self, edited: Entry) -> Result<(), ValidationErrors> {
        if edited.kind() != self.kind() {
            return Err(ValidationErrors(vec![ValidationError {
                field: "tipo",
                position: None,
                reason: TYPE_CHANGED,
            }]));
        }
        let level = self.level;
        *self = edited;
        self.level = level;
        Ok(())
    }

    /// Name of the other entry this one uses: the jump host of a server or the tunnel of
    /// a database.
    pub fn reference(&self) -> Option<&str> {
        match &self.data {
            Data::Server(s) => s.jump.as_deref(),
            Data::Database(d) => d.tunnel.as_deref(),
            _ => None,
        }
    }

    fn jump(&self) -> Option<&str> {
        match &self.data {
            Data::Server(s) => s.jump.as_deref(),
            _ => None,
        }
    }

    /// Checks the entry against the others in the vault (all but itself): unique name,
    /// tunnel and jump pointing to a server that exists and without a chain (a use
    /// involves at most two entries, D19).
    ///
    /// `others` is walked more than once (hence `Clone`), for example
    /// `entries.iter().filter(|o| o.name() != old_name)`.
    pub fn verify_in_vault<'a, I>(&self, others: I) -> Result<(), ValidationErrors>
    where
        I: IntoIterator<Item = &'a Entry>,
        I::IntoIter: Clone,
    {
        let others = others.into_iter();
        let mut c = Collector(Vec::new());
        if others.clone().any(|o| o.name == self.name) {
            c.error("nome", NAME_IN_USE);
        }
        if let Some(target) = self.reference() {
            let field = if self.kind() == EntryType::Database {
                "túnel SSH"
            } else {
                "servidor de salto"
            };
            if target == self.name {
                c.error(field, SELF_REFERENCE);
            } else {
                match others.clone().find(|o| o.name == target) {
                    Some(o) if o.kind() == EntryType::Server => {
                        if o.jump().is_some() {
                            let m = if self.kind() == EntryType::Database {
                                TUNNEL_WITH_JUMP
                            } else {
                                JUMP_WITH_JUMP
                            };
                            c.error(field, m);
                        }
                    }
                    _ => c.error(field, NOT_A_SERVER),
                }
            }
        }
        if self.jump().is_some() && others.clone().any(|o| o.reference() == Some(&self.name)) {
            c.error("servidor de salto", USED_AS_JUMP);
        }
        if c.0.is_empty() {
            Ok(())
        } else {
            Err(ValidationErrors(c.0))
        }
    }
}

// ---- Format rules (no user value in the messages) ----

/// Allow list of display text (D38): group, project, tags, extra and attachment
/// names, username of `login` and the value of an extra that is not secret. Only
/// visible ASCII, space (the ends are checked separately) and the precomposed Latin
/// letters from U+00C0 to U+017F, except `×` and `÷`. Everything else is left out:
/// control, formatting (bidi, zero width, Cf), fillers, variation selectors, combining
/// marks, non-ASCII spaces, emoji, Braille. Those characters would forge the text shown
/// in the approval window and in the list. The technical fields (host, URL, variable…) go
/// through here and then through their own format, which only accepts ASCII.
fn display_character(c: char) -> bool {
    matches!(c, ' '..='~' | '\u{C0}'..='\u{17F}') && c != '\u{D7}' && c != '\u{F7}'
}

/// Reason to refuse a required one-line text, or `None` if it is fine.
fn line_reason(value: &str, max: usize) -> Option<&'static str> {
    if value.is_empty() {
        Some(REQUIRED)
    } else if value.len() > max {
        Some(TOO_LONG)
    } else if !value.chars().all(display_character) {
        Some(CHARACTER)
    } else if value.trim() != value {
        Some(SPACE_AT_ENDS)
    } else {
        None
    }
}

/// Reason to refuse a text secret (already non-empty).
fn secret_reason(value: &Secret, max: usize) -> Option<&'static str> {
    if value.len() > max {
        Some(TOO_LONG)
    } else if value.expose_text().is_none() {
        Some(NOT_TEXT)
    } else if value.expose().contains(&0) {
        Some(NUL)
    } else {
        None
    }
}

fn valid_entry_name(v: &str) -> bool {
    let mut b = v.bytes();
    b.next()
        .is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit())
        && b.all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
        })
}

/// Database name of the database type (D33): `[A-Za-z0-9_-]`, from 1 to [`MAX_DB_NAME`] (the length
/// was already checked). No `?`, `#`, `/`, `.`, space or `%`: `PGDATABASE` and the others
/// always receive a clean name.
fn valid_db_name(v: &str) -> bool {
    !v.is_empty()
        && v.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
}

/// Database of `redis` (D33): number from 0 to [`MAX_REDIS_DB_NAME`], ASCII digits only
/// and no leading zero (a single form for each number).
fn valid_redis_db_name(v: &str) -> bool {
    !v.is_empty()
        && v.bytes().all(|c| c.is_ascii_digit())
        && (v == "0" || !v.starts_with('0'))
        && v.parse::<u32>().is_ok_and(|n| n <= MAX_REDIS_DB_NAME)
}

fn valid_host(v: &str) -> bool {
    !v.starts_with('-')
        && v.bytes().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_' | b':' | b'[' | b']')
        })
}

/// Visible ASCII (U+0021 to U+007E), without space.
fn visible_ascii(v: &str) -> bool {
    v.bytes().all(|c| c.is_ascii_graphic())
}

/// Username of the database and of `smtp` (D38): visible ASCII only (the smtp one is often an
/// e-mail), no `-` at the start.
fn valid_username(v: &str) -> bool {
    !v.starts_with('-') && visible_ascii(v)
}

/// Username of the server and of `ftp/sftp` (D43): `[A-Za-z0-9._@-]`, no `-` at the start.
/// The `@` is allowed for a domain account (`deploy@example.com`): the delivery (Phase
/// 27) passes the username only by `-l`, with the host after `--`, never as
/// `username@host`. It also holds for the jump host's, which is another server.
fn valid_ssh_username(v: &str) -> bool {
    !v.is_empty()
        && !v.starts_with('-')
        && v.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'@' | b'-'))
}

/// URL of `login` and `api` by a strict parser (D41, D9), without a new dependency:
/// - `https://`, or `http://` only with host `localhost` or `127.0.0.1` (the scheme in
///   lowercase: the URL arrives normalized by the app);
/// - visible ASCII only;
/// - authority without `@`, `\` or `%` (no userinfo, not even disguised);
/// - host: lowercase domain (labels `[a-z0-9-]` from 1 to 63, no `-` at the ends, up to
///   253 in total; international name only in punycode `xn--`), IPv4 with 4 octets
///   without leading zeros, or IPv6 in brackets (both read by `std::net`). A host of
///   digits and dots only is always read as IPv4; a domain whose last label starts with a
///   digit (`0x7f000001`, `1.2.3.0x4`, `x.123`) is refused (D55);
/// - optional port from 1 to 65535, without leading zero;
/// - path, query and fragment without `\`, and every `%` followed by two hex digits.
pub(crate) fn valid_url(v: &str) -> bool {
    if !visible_ascii(v) {
        return false;
    }
    let (https, rest) = if let Some(r) = v.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = v.strip_prefix("http://") {
        (false, r)
    } else {
        return false;
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, after) = rest.split_at(end);
    if authority.contains(['@', '\\', '%']) {
        return false;
    }
    let Some((host, port)) = split_host_port(authority) else {
        return false;
    };
    if !valid_url_host(host) || !port.is_none_or(valid_url_port) {
        return false;
    }
    if !https && host != "localhost" && host != "127.0.0.1" {
        return false;
    }
    valid_url_path(after)
}

/// Origin of a URL (D9): `scheme://host[:port]`, without the default port of the scheme
/// (80 for `http`, 443 for `https`), by the same strict parser of D41. `None` for any URL
/// the parser refuses (including `http` outside `localhost`/`127.0.0.1`), and whoever
/// compares origins treats `None` as a different origin. The host already comes in
/// lowercase and in punycode (the parser refuses the rest), so two equal origins have the
/// same text. A subdomain is another origin.
pub fn url_origin(v: &str) -> Option<String> {
    if !valid_url(v) {
        return None;
    }
    let (scheme, rest) = v.split_once("://")?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (host, port) = split_host_port(&rest[..end])?;
    let default = if scheme == "https" { "443" } else { "80" };
    Some(match port {
        Some(p) if p != default => format!("{scheme}://{host}:{p}"),
        _ => format!("{scheme}://{host}"),
    })
}

/// Phase 44e (D9, threat 4): the origin of a URL whose host points to this machine, in
/// any scheme, or `None` (outside host, or a URL the parser refuses). On a local origin,
/// any process of the uid can listen in place of the expected service (on `[::1]`, on
/// the port of a stopped service, or with a certificate from a local CA such as mkcert's
/// or one planted in `~/.pki`); `https` does not prove who answers. Whoever uses the
/// entry (`login`, `playwright` and the popup's fill) asks for confirmation in
/// the window before sending username and password.
pub fn local_url_origin(v: &str) -> Option<String> {
    let origin = url_origin(v)?;
    let (_, rest) = origin.split_once("://")?;
    let (host, _) = split_host_port(rest)?;
    local_host(host).then_some(origin)
}

/// Does the host (already accepted by the strict parser) point to this machine? When in
/// doubt, yes:
/// - `localhost` and every `*.localhost` (Chrome resolves both to the loopback);
/// - IPv4 `127.0.0.0/8` and `0.0.0.0/8` (Linux binds `0.0.0.0` to the machine itself);
/// - IPv6 `::1`, `::`, the mapped form (`::ffff:a.b.c.d`) of a local IPv4 and the
///   compatible one (`::127.x.x.x`).
///
/// A public domain whose DNS answers `127.0.0.1` is not caught here (there is no DNS in
/// the parser); it is in the scope's "Out of the model".
fn local_host(host: &str) -> bool {
    fn local_ipv4(ip: std::net::Ipv4Addr) -> bool {
        ip.is_loopback() || ip.octets()[0] == 0
    }
    if host == "localhost" || host.ends_with(".localhost") {
        return true;
    }
    if let Some(inside) = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        return inside.parse::<std::net::Ipv6Addr>().is_ok_and(|ip| {
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.to_ipv4_mapped().is_some_and(local_ipv4)
                // The compatible form (`::a.b.c.d`) only in 127/8: `::2` is not `0.0.0.2`.
                || ip.to_ipv4().is_some_and(|v4| v4.is_loopback())
        });
    }
    host.parse::<std::net::Ipv4Addr>().is_ok_and(local_ipv4)
}

/// `host` and `port` (without the `:`) of the authority; IPv6 comes with the brackets.
pub(crate) fn split_host_port(authority: &str) -> Option<(&str, Option<&str>)> {
    if authority.starts_with('[') {
        let close = authority.find(']')?;
        let (host, rest) = authority.split_at(close + 1);
        match rest.strip_prefix(':') {
            Some(p) => Some((host, Some(p))),
            None if rest.is_empty() => Some((host, None)),
            None => None,
        }
    } else {
        match authority.split_once(':') {
            Some((h, p)) => Some((h, Some(p))),
            None => Some((authority, None)),
        }
    }
}

fn valid_url_host(host: &str) -> bool {
    if let Some(inside) = host.strip_prefix('[') {
        // IPv6 in brackets, lowercase, read by `std::net`.
        return inside.strip_suffix(']').is_some_and(|ip| {
            !ip.bytes().any(|c| c.is_ascii_uppercase()) && ip.parse::<std::net::Ipv6Addr>().is_ok()
        });
    }
    if host.bytes().all(|c| c.is_ascii_digit() || c == b'.') {
        // Digits and dots only: IPv4, never a domain. `std::net` requires 4 octets and
        // refuses leading zeros; the check below repeats the rule for safety.
        return host.parse::<std::net::Ipv4Addr>().is_ok()
            && host.split('.').all(|o| o.len() == 1 || !o.starts_with('0'));
    }
    // D55: the last label of a domain does not start with a digit. Chrome and `curl`
    // read `0x7f000001`, `0x7f.0.0.1` and `1.2.3.0x4` as IPv4 (WHATWG hex and numeric
    // forms), and `x.123` is not a domain. This includes every final `0x…` label.
    if host
        .rsplit('.')
        .next()
        .is_none_or(|last| last.starts_with(|c: char| c.is_ascii_digit()))
    {
        return false;
    }
    host.len() <= MAX_HOST
        && host.split('.').all(|label| {
            (1..=63).contains(&label.len())
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        })
}

fn valid_url_port(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 5
        && !p.starts_with('0')
        && p.bytes().all(|c| c.is_ascii_digit())
        && p.parse::<u16>().is_ok_and(|n| n != 0)
}

/// Path, query and fragment (already in visible ASCII): without `\`, and every `%` with
/// two hex digits.
fn valid_url_path(after: &str) -> bool {
    let b = after.as_bytes();
    !b.contains(&b'\\')
        && b.iter().enumerate().all(|(i, &c)| {
            c != b'%'
                || (b.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
                    && b.get(i + 2).is_some_and(u8::is_ascii_hexdigit))
        })
}

fn valid_variable(v: &str) -> bool {
    let mut b = v.bytes();
    b.next().is_some_and(|p| p.is_ascii_uppercase())
        && b.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
}

/// Suffixes accepted in the `api` variable (D42): the name says it holds a credential.
const CREDENTIAL_SUFFIXES: &[&str] = &["_TOKEN", "_KEY", "_SECRET", "_PASSWORD", "_PAT", "_AUTH"];

/// Allow list of D42: ends in a credential suffix (and has something before it).
fn variable_with_suffix(v: &str) -> bool {
    CREDENTIAL_SUFFIXES
        .iter()
        .any(|s| v.len() > s.len() && v.ends_with(s))
}

/// Second layer (D25): system variables and reserved prefixes, and every name with
/// `ASKPASS` (`ssh`, `git` and `sudo` run the program it points to).
fn forbidden_variable(v: &str) -> bool {
    SYSTEM_VARIABLES.contains(&v)
        || FORBIDDEN_PREFIXES.iter().any(|p| v.starts_with(p))
        || v.contains("ASKPASS")
}

fn valid_access_key(v: &str) -> bool {
    (MIN_ACCESS_KEY..=MAX_ACCESS_KEY).contains(&v.len())
        && v.bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

fn valid_region(v: &str) -> bool {
    !v.starts_with('-')
        && v.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

fn valid_arn(v: &str) -> bool {
    v.starts_with("arn:") && visible_ascii(v)
}

/// RFC 4648 base32 in uppercase, `=` only at the end, at least 16 useful characters.
fn valid_totp_seed(s: &Secret) -> bool {
    let b = s.expose();
    let useful = b
        .iter()
        .take_while(|c| c.is_ascii_uppercase() || (b'2'..=b'7').contains(c))
        .count();
    let rest = &b[useful..];
    useful >= MIN_TOTP && rest.iter().all(|&c| c == b'=')
}

/// Starts with `-----BEGIN ` and has `PRIVATE KEY-----` (PEM or OpenSSH). Does not
/// decode: whoever reads the key is `ssh`/the agent.
fn looks_like_private_key(s: &Secret) -> bool {
    s.expose_text().is_some_and(|t| {
        let t = t.trim_start();
        t.starts_with("-----BEGIN ") && t.contains("PRIVATE KEY-----")
    })
}
