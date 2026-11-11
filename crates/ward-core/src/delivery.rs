//! Delivery through environment variables (D10): from a validated [`Entry`], builds the
//! exact list of variables that `ward run` puts in the child's environment.
//!
//! The core does no I/O and does not depend on Linux. What needs the system stays with
//! the caller, through the [`Resources`] trait:
//! - the current TOTP code (the seed never becomes a variable);
//! - the temp file of each attachment and of the key of the server and FTP/SFTP types
//!   (the attachment file variable receives only the path);
//! - the local port of the SSH tunnel of the database type.
//!
//! Security rules of this layer:
//! - Every value lives in a `Zeroizing<String>` allocated only once, at the exact size
//!   (URLs included), so no copy of the secret is left in freed memory (S8).
//! - No `Clone`; `Debug` shows only the names, never a value (S13). The errors have only
//!   fixed texts.
//! - User, password and database name inside the URLs go with percent-encoding: a
//!   password with `@`, `/` or `#`, or a database name with `?host=…`, does not change
//!   where the client connects.
//! - Before returning, the list is verified again: variable name in `[A-Z_][A-Z0-9_]*`,
//!   no `LD_*`, no repeated name and no null byte in a value. What comes from
//!   [`Resources`] is verified too (TOTP code with digits only, path without null, port
//!   other than 0).
//! - The secure note has no delivery: the refusal comes before any call to
//!   [`Resources`], so no file is created. The same for an entry with an extra or
//!   attachment named like a variable of the type (m2 Phase 54b: possible only in an
//!   entry read from a vault of 0.1.x).
//!
//! If the build fails after [`Resources::file`] created files, the caller deletes what it
//! created (`ward run` already cleans up on every exit, S2).

use core::fmt;

use zeroize::{ZeroizeOnDrop, Zeroizing};

use crate::entry::{
    Auth, Data, Database, Engine, Entry, ExtraValue, Secret, has_reserved_name, variable_name,
};

#[cfg(test)]
mod tests;

/// Failure of a system resource (TOTP, temp file, tunnel). Fixed text only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceError(pub &'static str);

/// What the delivery needs from the system. `ward run` (daemon) implements it; the tests
/// use a fake.
pub trait Resources {
    /// Current TOTP code of the seed (RFC 6238). Digits only, 6 to 8 of them.
    fn totp_code(&mut self, seed: &Secret) -> Result<Zeroizing<String>, ResourceError>;

    /// Writes the content to a 0600 temp file in `/run/user/<uid>/ward/` and returns the
    /// path. `name` is the attachment name (already validated: no `/`, not starting with
    /// `.` or `-`) or the fixed name of the type's private key.
    fn file(&mut self, name: &str, content: &Secret) -> Result<String, ResourceError>;

    /// Opens the SSH tunnel through the server entry named `server` up to `host:port` and
    /// returns the local port on `127.0.0.1`.
    fn tunnel(&mut self, server: &str, host: &str, port: u16) -> Result<u16, ResourceError>;
}

/// Why the delivery did not come out. Fixed texts only; no value of the entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryError {
    /// Secure note: `ward run` refuses without running the command.
    TypeWithoutDelivery,
    /// A system resource failed.
    Resource(ResourceError),
    /// The resource returned something out of format (TOTP code, path or port).
    InvalidResource(&'static str),
    /// Defense in depth: a value or name that the validation should have stopped.
    Invalid(&'static str),
    /// An extra or attachment named like a variable that the type delivers (or may
    /// deliver): the delivery is refused whole, before any call to [`Resources`], so no
    /// extra or attachment replaces or impersonates a `WARD_*` variable. Validation
    /// refuses the name when the entry is saved; an entry read from a vault of 0.1.x may
    /// still carry one that only 0.2.0 reserves (m2 Phase 54b).
    ReservedName,
}

impl fmt::Display for DeliveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeliveryError::TypeWithoutDelivery => f.write_str("tipo sem entrega"),
            DeliveryError::Resource(ResourceError(m)) => write!(f, "entrega falhou: {m}"),
            DeliveryError::InvalidResource(m) => write!(f, "entrega falhou: {m}"),
            DeliveryError::Invalid(m) => write!(f, "entrada sem entrega válida: {m}"),
            DeliveryError::ReservedName => {
                f.write_str("entrada sem entrega válida: campo extra ou anexo com nome reservado")
            }
        }
    }
}

impl std::error::Error for DeliveryError {}

impl From<ResourceError> for DeliveryError {
    fn from(e: ResourceError) -> Self {
        DeliveryError::Resource(e)
    }
}

/// An environment variable of the delivery.
pub struct Variable {
    name: String,
    value: Zeroizing<String>,
    secret: bool,
}

impl ZeroizeOnDrop for Variable {}

impl Variable {
    /// Name (not a secret).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The value. May be a secret: the caller passes it straight to the child's
    /// environment, without copying it out of a type that zeroes and without logging.
    pub fn expose(&self) -> &str {
        &self.value
    }

    /// Does the value come from a secret (password, token, URL with a password…)? The
    /// TOTP code and the file path are not (D11).
    pub fn is_secret(&self) -> bool {
        self.secret
    }
}

impl fmt::Debug for Variable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}=••••", self.name)
    }
}

/// The built environment, in order: common variables, type variables, extras and
/// attachments.
pub struct Delivery {
    variables: Vec<Variable>,
}

impl ZeroizeOnDrop for Delivery {}

impl Delivery {
    /// The variables, in order.
    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }

    /// Value of a variable by name.
    pub fn value(&self, name: &str) -> Option<&str> {
        self.variables
            .iter()
            .find(|v| v.name == name)
            .map(Variable::expose)
    }

    /// The names, in order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.variables.iter().map(Variable::name)
    }
}

impl fmt::Debug for Delivery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.variables.iter()).finish()
    }
}

/// Builds the environment of the entry (D10).
pub fn build(entry: &Entry, resources: &mut dyn Resources) -> Result<Delivery, DeliveryError> {
    if matches!(entry.data(), Data::SecureNote(_)) {
        return Err(DeliveryError::TypeWithoutDelivery);
    }
    if has_reserved_name(entry) {
        return Err(DeliveryError::ReservedName);
    }
    let mut m = Builder::default();
    m.text("WARD_ENTRY", entry.name());
    m.text("WARD_GROUP", entry.group());
    m.text("WARD_ENVIRONMENT", entry.environment().name());

    match entry.data() {
        Data::Login(l) => {
            m.text("WARD_URL", &l.url);
            m.text("WARD_USER", &l.username);
            m.secret("WARD_PASS", &l.password)?;
            if let Some(seed) = &l.totp_seed {
                let code = resources.totp_code(seed)?;
                if !(6..=8).contains(&code.len()) || !code.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(DeliveryError::InvalidResource(
                        "código TOTP fora do formato",
                    ));
                }
                m.push("WARD_TOTP", code, false);
            }
        }
        Data::Database(b) => database(&mut m, b, resources)?,
        Data::Server(s) => {
            m.text("WARD_HOST", &s.host);
            m.text("WARD_PORT", &s.port.to_string());
            m.text("WARD_USER", &s.username);
            auth(&mut m, &s.auth, resources)?;
        }
        Data::Api(a) => {
            if let Some(url) = &a.url {
                m.text("WARD_URL", url);
            }
            m.secret("WARD_TOKEN", &a.token)?;
            if let Some(var) = &a.variable {
                m.secret(var, &a.token)?;
            }
        }
        Data::Aws(a) => {
            m.text("AWS_ACCESS_KEY_ID", &a.access_key_id);
            m.secret("AWS_SECRET_ACCESS_KEY", &a.secret_access_key)?;
            if let Some(t) = &a.session_token {
                m.secret("AWS_SESSION_TOKEN", t)?;
            }
            m.text("AWS_REGION", &a.region);
            m.text("AWS_DEFAULT_REGION", &a.region);
            if let Some(arn) = &a.role_arn {
                m.text("AWS_ROLE_ARN", arn);
            }
        }
        Data::Smtp(s) => {
            m.text("SMTP_HOST", &s.host);
            m.text("SMTP_PORT", &s.port.to_string());
            m.text("SMTP_USER", &s.username);
            m.secret("SMTP_PASS", &s.password)?;
            m.text("SMTP_SECURITY", s.security.name());
        }
        Data::FtpSftp(f) => {
            m.text("WARD_PROTOCOL", f.protocol.name());
            m.text("WARD_HOST", &f.host);
            m.text("WARD_PORT", &f.port.to_string());
            m.text("WARD_USER", &f.username);
            auth(&mut m, &f.auth, resources)?;
        }
        Data::SecureNote(_) => return Err(DeliveryError::TypeWithoutDelivery),
        Data::Generic => {}
    }

    for extra in entry.extras() {
        let name = prefixed("WARD_", extra.name());
        match extra.value() {
            ExtraValue::Text(v) => m.text(&name, v),
            ExtraValue::Secret(v) => m.secret(&name, v)?,
        }
    }
    for attachment in entry.attachments() {
        let name = prefixed("WARD_FILE_", attachment.name());
        m.file(&name, attachment.name(), attachment.content(), resources)?;
    }

    m.check()?;
    Ok(Delivery {
        variables: m.variables,
    })
}

fn prefixed(prefix: &str, name: &str) -> String {
    let mut s = String::from(prefix);
    s.push_str(&variable_name(name));
    s
}

fn auth(m: &mut Builder, a: &Auth, resources: &mut dyn Resources) -> Result<(), DeliveryError> {
    match a {
        Auth::Password(password) => m.secret("WARD_PASS", password),
        Auth::Key {
            private_key,
            passphrase,
        } => {
            m.file("WARD_FILE_KEY", "chave", private_key, resources)?;
            if let Some(p) = passphrase {
                m.secret("WARD_PASSPHRASE", p)?;
            }
            Ok(())
        }
    }
}

fn database(
    m: &mut Builder,
    b: &Database,
    resources: &mut dyn Resources,
) -> Result<(), DeliveryError> {
    // With a tunnel, the client connects to the local end (D10).
    let (host, port) = match &b.tunnel {
        Some(server) => {
            let local = resources.tunnel(server, &b.host, b.port)?;
            if local == 0 {
                return Err(DeliveryError::InvalidResource("porta do túnel inválida"));
            }
            ("127.0.0.1", local)
        }
        None => (b.host.as_str(), b.port),
    };
    let port = port.to_string();
    let password = secret_text(&b.password)?;
    let db_name = b.db_name.as_deref();
    let url = |scheme: &str, name_in_path: Option<&str>, name_in_query: Option<&str>| {
        database_url(
            scheme,
            &b.username,
            password,
            host,
            &port,
            name_in_path,
            name_in_query,
        )
    };

    match b.engine {
        Engine::Postgres => {
            m.text("PGHOST", host);
            m.text("PGPORT", &port);
            if let Some(db_name) = db_name {
                m.text("PGDATABASE", db_name);
            }
            m.text("PGUSER", &b.username);
            m.secret("PGPASSWORD", &b.password)?;
            m.push("DATABASE_URL", url("postgresql", db_name, None), true);
        }
        Engine::Mysql => {
            m.text("MYSQL_HOST", host);
            m.text("MYSQL_TCP_PORT", &port);
            m.text("MYSQL_USER", &b.username);
            m.secret("MYSQL_PWD", &b.password)?;
            if let Some(db_name) = db_name {
                m.text("MYSQL_DATABASE", db_name);
            }
            m.push("DATABASE_URL", url("mysql", db_name, None), true);
        }
        Engine::Redis => {
            // In redis the URL path is the database number; a database name that is not a
            // number stays out of the URL (redis-cli would refuse it).
            let number = db_name.filter(|b| !b.is_empty() && b.bytes().all(|c| c.is_ascii_digit()));
            m.text("REDIS_HOST", host);
            m.text("REDIS_PORT", &port);
            m.secret("REDISCLI_AUTH", &b.password)?;
            m.push("REDIS_URL", url("redis", number, None), true);
            m.push("DATABASE_URL", url("redis", number, None), true);
        }
        Engine::Mongo => {
            m.push("MONGODB_URI", url("mongodb", db_name, None), true);
            m.push("DATABASE_URL", url("mongodb", db_name, None), true);
        }
        Engine::Sqlserver => {
            let mut server = String::with_capacity(host.len() + 1 + port.len());
            server.push_str(host);
            server.push(',');
            server.push_str(&port);
            m.text("SQLCMDSERVER", &server);
            m.text("SQLCMDUSER", &b.username);
            m.secret("SQLCMDPASSWORD", &b.password)?;
            if let Some(db_name) = db_name {
                m.text("SQLCMDDBNAME", db_name);
            }
            m.push("DATABASE_URL", url("sqlserver", None, db_name), true);
        }
    }
    Ok(())
}

/// `scheme://user:password@host:port[/db_name][?database=db_name]`, with user, password
/// and database name in percent-encoding. Allocated once, at the exact size.
fn database_url(
    scheme: &str,
    username: &str,
    password: &str,
    host: &str,
    port: &str,
    name_in_path: Option<&str>,
    name_in_query: Option<&str>,
) -> Zeroizing<String> {
    // IPv6 without brackets gets brackets; the rest of the host is already only
    // `[A-Za-z0-9._-:[]]`.
    let ipv6 = host.contains(':') && !host.starts_with('[');
    let mut parts: Vec<(&str, bool)> = vec![
        (scheme, false),
        ("://", false),
        (username, true),
        (":", false),
        (password, true),
        ("@", false),
    ];
    if ipv6 {
        parts.push(("[", false));
    }
    parts.push((host, false));
    if ipv6 {
        parts.push(("]", false));
    }
    parts.push((":", false));
    parts.push((port, false));
    if let Some(db_name) = name_in_path {
        parts.push(("/", false));
        parts.push((db_name, true));
    }
    if let Some(db_name) = name_in_query {
        parts.push(("?database=", false));
        parts.push((db_name, true));
    }
    join_parts(&parts)
}

/// Joins the parts into a text allocated only once; the marked ones go with
/// percent-encoding.
fn join_parts(parts: &[(&str, bool)]) -> Zeroizing<String> {
    let size: usize = parts
        .iter()
        .map(|(p, encode)| {
            if *encode {
                p.bytes().map(|b| if unreserved(b) { 1 } else { 3 }).sum()
            } else {
                p.len()
            }
        })
        .sum();
    let mut s = Zeroizing::new(String::with_capacity(size));
    for (p, encode) in parts {
        if *encode {
            encode_into(&mut s, p);
        } else {
            s.push_str(p);
        }
    }
    debug_assert_eq!(s.len(), size);
    s
}

/// "Unreserved" characters of RFC 3986, which need no percent-encoding.
fn unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~')
}

/// Percent-encoding (RFC 3986) with uppercase hexadecimal, byte by byte of the UTF-8.
fn encode_into(destination: &mut String, text: &str) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for b in text.bytes() {
        if unreserved(b) {
            destination.push(char::from(b));
        } else {
            destination.push('%');
            destination.push(char::from(HEX[usize::from(b >> 4)]));
            destination.push(char::from(HEX[usize::from(b & 0x0F)]));
        }
    }
}

fn secret_text(s: &Secret) -> Result<&str, DeliveryError> {
    s.expose_text()
        .ok_or(DeliveryError::Invalid("segredo que não é texto"))
}

#[derive(Default)]
struct Builder {
    variables: Vec<Variable>,
}

impl Builder {
    fn push(&mut self, name: &str, value: Zeroizing<String>, secret: bool) {
        self.variables.push(Variable {
            name: name.to_owned(),
            value,
            secret,
        });
    }

    fn text(&mut self, name: &str, value: &str) {
        self.push(name, Zeroizing::new(value.to_owned()), false);
    }

    fn secret(&mut self, name: &str, value: &Secret) -> Result<(), DeliveryError> {
        let v = secret_text(value)?;
        self.push(name, Zeroizing::new(v.to_owned()), true);
        Ok(())
    }

    fn file(
        &mut self,
        variable: &str,
        name: &str,
        content: &Secret,
        resources: &mut dyn Resources,
    ) -> Result<(), DeliveryError> {
        let path = resources.file(name, content)?;
        if path.is_empty() {
            return Err(DeliveryError::InvalidResource("caminho de arquivo vazio"));
        }
        self.push(variable, Zeroizing::new(path), false);
        Ok(())
    }

    /// Defense in depth over the final list.
    fn check(&self) -> Result<(), DeliveryError> {
        for (i, v) in self.variables.iter().enumerate() {
            if !valid_variable_name(&v.name) {
                return Err(DeliveryError::Invalid("nome de variável fora do formato"));
            }
            if v.name.starts_with("LD_") {
                return Err(DeliveryError::Invalid("variável LD_* na entrega"));
            }
            if self.variables[..i].iter().any(|o| o.name == v.name) {
                return Err(DeliveryError::Invalid("variável repetida"));
            }
            if v.value.as_bytes().contains(&0) {
                return Err(DeliveryError::Invalid("valor com byte nulo"));
            }
        }
        Ok(())
    }
}

fn valid_variable_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|c| c.is_ascii_uppercase() || c == b'_')
        && bytes.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
}
