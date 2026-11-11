//! Entry form (the new and edit form frames of the design) of the 9 types: `login`,
//! `api`, `aws`, `smtp`, secure note and generic (Phase 33); database, server and
//! `ftp/sftp` (Phase 34). The attachments do not come from the interface: the form only
//! says which of the attachments chosen on the Rust side (choose attachment command) go
//! into the draft.
//!
//! What the interface sends ([`Form`]) is untrusted: a field key outside the list of the
//! type, a list too large or a text too long are refused here (`InvalidRequest`), before
//! any request to the daemon. The format of each field is checked by the daemon (the
//! draft validation), which returns the problems per field ([`FieldError`]).
//!
//! Secrets (S11, D4; the wire names of [`SecretValue`] stay until m2:Phase 54):
//! - [`SecretValue::Empty`]: no value (optional field or not filled in);
//! - [`SecretValue::New`]: what the user typed now, in a [`SecretText`] that zeroes on
//!   drop and has no `Debug` of the value;
//! - [`SecretValue::Keep`]: when editing, the value already in the vault. It does not
//!   pass through the interface: the Rust side asks the daemon for it (reveal) and puts
//!   it straight into the draft;
//! - [`SecretValue::Generated`]: the password the form generator kept on the Rust side
//!   (the "use this password" button). It does not pass through the interface either.
//! - [`SecretValue::Loaded`] (Phase 53a): the private key read from the file by the "load
//!   from file" button and kept on the Rust side. It does not pass through the interface
//!   either.
//!
//! TOTP (D35): the typed seed is normalized here (no spaces, upper case); a pasted
//! `otpauth://` is read and refused with [`TOTP_UNSUPPORTED`] if it has `algorithm`,
//! `digits` or `period` other than SHA1, 6 and 30.

use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize};
use ward_core::entry::{
    AttachmentDraft, DataDraft, Draft, EntryType, ExtraDraft, Secret as CoreSecret,
    TOO_LARGE_FOR_MASK,
};
use ward_core::limits;
use ward_core::protocolo::{ErroResposta, Problema};
use zeroize::Zeroizing;

use crate::AppError;

/// D35 message for an `otpauth://` with algorithm, digits or period other than SHA1, 6
/// and 30.
pub const TOTP_UNSUPPORTED: &str = "parâmetros de TOTP não suportados";
/// `otpauth://` without `secret` or malformed.
pub const TOTP_INVALID_URI: &str = "endereço otpauth:// sem semente válida";

/// Secret text typed in the interface. Zeroed on drop; the `Debug` does not show the
/// value.
pub struct SecretText(Zeroizing<String>);

impl fmt::Debug for SecretText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretText(••••)")
    }
}

impl<'de> Deserialize<'de> for SecretText {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = SecretText;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("texto")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<SecretText, E> {
                Ok(SecretText(Zeroizing::new(v.to_owned())))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<SecretText, E> {
                // Takes ownership of the text already read: no copy left unzeroed.
                Ok(SecretText(Zeroizing::new(v)))
            }
        }
        d.deserialize_string(V)
    }
}

/// Value of a secret field of the type. Externally tagged enum: serde reads it directly,
/// without keeping an intermediate copy of the text.
#[derive(Debug, Deserialize)]
pub enum SecretValue {
    #[serde(rename = "vazio")]
    Empty,
    #[serde(rename = "manter")]
    Keep,
    #[serde(rename = "gerada")]
    Generated,
    #[serde(rename = "carregada")]
    Loaded,
    #[serde(rename = "novo")]
    New(SecretText),
}

/// Value of an extra field: typed now, or (when editing, only for a secret extra) the one
/// already in the vault, by the name the extra had.
#[derive(Debug, Deserialize)]
pub enum ExtraValue {
    #[serde(rename = "novo")]
    New(SecretText),
    #[serde(rename = "manter")]
    Keep(String),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormExtra {
    #[serde(rename = "nome")]
    pub name: String,
    #[serde(rename = "segredo")]
    pub secret: bool,
    #[serde(rename = "valor")]
    pub value: ExtraValue,
}

/// The form as it came from the interface.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Form {
    #[serde(rename = "tipo")]
    pub kind: String,
    #[serde(rename = "nome")]
    pub name: String,
    #[serde(rename = "grupo")]
    pub group: String,
    #[serde(rename = "ambiente")]
    pub environment: String,
    #[serde(rename = "projeto")]
    pub project: String,
    pub tags: Vec<String>,
    #[serde(rename = "campos")]
    pub fields: BTreeMap<String, String>,
    #[serde(rename = "segredos")]
    pub secrets: BTreeMap<String, SecretValue>,
    pub extras: Vec<FormExtra>,
    /// Ids of the attachments chosen on the Rust side that go into the draft, in the
    /// order of the screen. The content never passes through the interface (S11, V5.12).
    #[serde(default, rename = "anexos")]
    pub attachments: Vec<u64>,
    /// Phase 39b (V5.12): when editing, the names of the stored attachments that stay in
    /// the entry. The daemon copies their content (the keep attachments field of the save
    /// entry request); a stored attachment outside this list is removed on save.
    #[serde(default, rename = "anexos_mantidos")]
    pub kept_attachments: Vec<String>,
}

/// A problem shown on the field: the field id in the form (the protocol ids of
/// [`fields_of_kind`], `extras`…, or the whole form when it is not about one field), the
/// position in the list (extras and tags, from 1) and the reason, a fixed text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldError {
    #[serde(rename = "campo")]
    pub field: &'static str,
    #[serde(rename = "posicao")]
    pub position: Option<u64>,
    #[serde(rename = "motivo")]
    pub reason: String,
}

/// Type of the form: the 9 types (Phases 33 and 34).
pub fn form_kind(name: &str) -> Option<EntryType> {
    EntryType::from_name(name)
}

/// Text fields and secrets of each type, with the protocol ids.
pub fn fields_of_kind(kind: EntryType) -> (&'static [&'static str], &'static [&'static str]) {
    match kind {
        EntryType::Login => (&["url", "usuario"], &["senha", "totp_semente"]),
        EntryType::Api => (&["url", "variavel"], &["token"]),
        EntryType::Aws => (
            &["access_key_id", "regiao", "role_arn"],
            &["secret_access_key", "session_token"],
        ),
        EntryType::Smtp => (&["host", "porta", "usuario", "seguranca"], &["senha"]),
        EntryType::SecureNote => (&[], &["texto"]),
        EntryType::Database => (
            &["motor", "host", "porta", "base", "usuario", "tunel"],
            &["senha"],
        ),
        EntryType::Server => (
            &["host", "porta", "usuario", "salto"],
            &["senha", "chave_privada", "passphrase"],
        ),
        EntryType::FtpSftp => (
            &["protocolo", "host", "porta", "usuario"],
            &["senha", "chave_privada", "passphrase"],
        ),
        EntryType::Generic => (&[], &[]),
    }
}

/// Form field from the name `ward-core` uses in the validation problem. What is not
/// about one field goes to the whole form.
fn problem_field(name: &str) -> &'static str {
    match name {
        "nome" => "nome",
        "grupo" => "grupo",
        "ambiente" => "ambiente",
        "projeto" => "projeto",
        "tags" => "tags",
        "url" => "url",
        "usuário" => "usuario",
        "senha" => "senha",
        "semente TOTP" => "totp_semente",
        "token" => "token",
        "variável" => "variavel",
        "access key ID" => "access_key_id",
        "secret access key" => "secret_access_key",
        "região" => "regiao",
        "role ARN" => "role_arn",
        "session token" => "session_token",
        "host" => "host",
        "porta" => "porta",
        "segurança" => "seguranca",
        "texto" => "texto",
        "campos extras" => "extras",
        "motor" => "motor",
        "base" => "base",
        "túnel SSH" => "tunel",
        "servidor de salto" => "salto",
        "protocolo" => "protocolo",
        "chave privada" => "chave_privada",
        "passphrase" => "passphrase",
        "autenticação" => "autenticacao",
        "anexos" => "anexos",
        _ => "formulario",
    }
}

/// Longest reason accepted from the daemon's response.
const MAX_REASON: usize = 200;

/// Daemon problems as field errors. The daemon's response is untrusted (S1): the field
/// comes from a fixed table and the reason only passes if it is a short text, without
/// control nor Unicode formatting characters; otherwise it becomes the fixed "invalid"
/// text.
pub fn daemon_errors(problems: &[Problema]) -> Vec<FieldError> {
    problems
        .iter()
        .take(limits::MAX_EXTRAS + 32)
        .map(|p| {
            let clean = p.motivo.chars().count() <= MAX_REASON
                && !p.motivo.is_empty()
                && !p
                    .motivo
                    .chars()
                    .any(|c| c.is_control() || is_format_char(c));
            // D29: the mask budget refusal comes from the whole entry; the screen shows
            // it on the attachments field, the one that weighs most on the budget.
            let field = if p.campo == "entrada" && p.motivo == TOO_LARGE_FOR_MASK {
                "anexos"
            } else {
                problem_field(&p.campo)
            };
            FieldError {
                field,
                position: p.posicao.filter(|n| (1..=1024).contains(n)),
                reason: if clean {
                    p.motivo.clone()
                } else {
                    "inválido".to_owned()
                },
            }
        })
        .collect()
}

fn is_format_char(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200F}' | '\u{2028}'..='\u{202E}' | '\u{2060}'..='\u{206F}' | '\u{FEFF}' | '\u{E0000}'..='\u{E007F}')
}

/// Secret ready for the draft, or "no value".
pub type BuiltSecret = Option<CoreSecret>;

/// Limits of what the interface may send, before any format validation. The fine
/// validation (and the message per field) belongs to `ward-core`; this only stops absurd
/// input (memory, frame).
fn check_limits(f: &Form) -> Result<(), AppError> {
    let text_ok = |s: &str| s.len() <= limits::MAX_TEXT;
    let ok = text_ok(&f.name)
        && text_ok(&f.group)
        && text_ok(&f.environment)
        && text_ok(&f.project)
        && f.kind.len() <= 32
        && f.tags.len() <= 2 * limits::MAX_TAGS
        && f.tags.iter().all(|t| text_ok(t))
        && f.fields.len() <= 8
        && f.fields.values().all(|v| text_ok(v))
        && f.secrets.len() <= 8
        && f.secrets.values().all(|v| match v {
            SecretValue::New(t) => t.0.len() <= limits::MAX_NOTE,
            _ => true,
        })
        && f.attachments.len() <= limits::MAX_ATTACHMENTS + 1
        && f.extras.len() <= limits::MAX_EXTRAS + 1
        && f.extras.iter().all(|x| {
            text_ok(&x.name)
                && match &x.value {
                    ExtraValue::New(t) => t.0.len() <= limits::MAX_EXTRA_VALUE,
                    ExtraValue::Keep(n) => text_ok(n),
                }
        });
    if ok {
        Ok(())
    } else {
        Err(AppError::InvalidRequest)
    }
}

/// Reads an `otpauth://totp/…?secret=…` (D35). Returns the normalized seed, or the reason
/// for the refusal. Unknown parameters (`issuer`, `image`…) are ignored; repeated ones are
/// refused.
fn seed_from_otpauth(uri: &str) -> Result<Zeroizing<String>, &'static str> {
    let rest = &uri["otpauth://".len()..];
    let (kind, after) = rest.split_once('/').ok_or(TOTP_INVALID_URI)?;
    if !kind.eq_ignore_ascii_case("totp") {
        return Err(TOTP_UNSUPPORTED);
    }
    let query = after.split_once('?').map(|(_, q)| q).unwrap_or("");
    let query = query.split_once('#').map(|(q, _)| q).unwrap_or(query);
    let mut seed: Option<Zeroizing<String>> = None;
    let mut seen: Vec<String> = Vec::new();
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let key = key.to_ascii_lowercase();
        if seen.contains(&key) {
            return Err(TOTP_INVALID_URI);
        }
        let value = decode_pct(value).ok_or(TOTP_INVALID_URI)?;
        match key.as_str() {
            "secret" => seed = Some(value),
            "algorithm" if !value.eq_ignore_ascii_case("SHA1") => return Err(TOTP_UNSUPPORTED),
            "digits" if value.as_str() != "6" => return Err(TOTP_UNSUPPORTED),
            "period" if value.as_str() != "30" => return Err(TOTP_UNSUPPORTED),
            _ => {}
        }
        seen.push(key);
    }
    let seed = seed.ok_or(TOTP_INVALID_URI)?;
    Ok(normalize_base32(&seed))
}

/// `%XX` → byte; `+` stays as it is (it is not a space in a seed). UTF-8 required.
fn decode_pct(v: &str) -> Option<Zeroizing<String>> {
    let b = v.as_bytes();
    let mut out = Zeroizing::new(Vec::with_capacity(b.len()));
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = b.get(i + 1..i + 3)?;
            let high = (hex[0] as char).to_digit(16)?;
            let low = (hex[1] as char).to_digit(16)?;
            out.push(u8::try_from(high * 16 + low).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    let text = core::str::from_utf8(&out).ok()?;
    Some(Zeroizing::new(text.to_owned()))
}

/// Base32 seed as the vault stores it (D25): no spaces and in upper case.
fn normalize_base32(s: &str) -> Zeroizing<String> {
    let mut out = Zeroizing::new(String::with_capacity(s.len()));
    for c in s.chars().filter(|c| !c.is_whitespace()) {
        out.push(c.to_ascii_uppercase());
    }
    out
}

/// The TOTP seed typed or pasted, ready for the draft (D25, D35).
pub fn normalize_seed(typed: &str) -> Result<Zeroizing<String>, &'static str> {
    let trimmed = typed.trim();
    let prefix = trimmed.get(..10).unwrap_or("");
    if prefix.eq_ignore_ascii_case("otpauth://") {
        seed_from_otpauth(trimmed)
    } else {
        Ok(normalize_base32(trimmed))
    }
}

/// Where the values that are not in the form come from: the vault (reveal, when editing)
/// and the password the form generator kept.
pub trait Sources {
    /// Stored value of a secret field (field target) or of a secret extra (extra target)
    /// of the edited entry.
    fn stored(&mut self, target: ward_core::protocolo::Alvo) -> Result<CoreSecret, AppError>;
    /// Password generated for this field (the "use this password" button).
    fn generated(&mut self, field: &str) -> Result<CoreSecret, AppError>;
    /// Key read from the file for this field (the "load from file" button, Phase 53a).
    fn loaded(&mut self, field: &str) -> Result<CoreSecret, AppError>;
}

/// What is missing to save: the draft of the data and of the extras, or the reason.
pub enum Missing {
    App(AppError),
    Fields(Vec<FieldError>),
}

impl From<AppError> for Missing {
    fn from(e: AppError) -> Self {
        Missing::App(e)
    }
}

/// Parts of the draft that depend on the type.
pub struct Built {
    pub data: DataDraft,
    pub extras: Vec<ExtraDraft>,
}

/// Builds the data of the type and the extras. `editing` says whether this is an edit
/// (only then [`SecretValue::Keep`] is valid); the stored secrets and the stored extras are the ones
/// the detail of the edited entry lists.
pub fn build(
    f: &mut Form,
    kind: EntryType,
    editing: Option<(&[String], &[String])>,
    sources: &mut dyn Sources,
) -> Result<Built, Missing> {
    check_limits(f)?;
    let (texts, secrets) = fields_of_kind(kind);
    if f.fields.keys().any(|k| !texts.contains(&k.as_str()))
        || f.secrets.keys().any(|k| !secrets.contains(&k.as_str()))
    {
        return Err(AppError::InvalidRequest.into());
    }
    let mut errors = Vec::new();
    let mut take = |id: &str| f.fields.remove(id).unwrap_or_default();
    let (url, username, variable, access, region, arn, host, port, security) = (
        take("url"),
        take("usuario"),
        take("variavel"),
        take("access_key_id"),
        take("regiao"),
        take("role_arn"),
        take("host"),
        take("porta"),
        take("seguranca"),
    );
    let (engine, db_name, tunnel, jump, protocol) = (
        take("motor"),
        take("base"),
        take("tunel"),
        take("salto"),
        take("protocolo"),
    );
    let mut secret = |id: &'static str| -> Result<BuiltSecret, Missing> {
        let value = f.secrets.remove(id).unwrap_or(SecretValue::Empty);
        Ok(match value {
            SecretValue::Empty => None,
            SecretValue::New(t) if t.0.is_empty() => None,
            SecretValue::New(t) if id == "totp_semente" => match normalize_seed(&t.0) {
                Ok(s) => Some(CoreSecret::from_bytes(s.as_bytes().to_vec())),
                Err(reason) => {
                    errors.push(FieldError {
                        field: "totp_semente",
                        position: None,
                        reason: reason.to_owned(),
                    });
                    None
                }
            },
            SecretValue::New(t) => Some(CoreSecret::from_bytes(t.0.as_bytes().to_vec())),
            SecretValue::Keep => match editing {
                Some((stored, _)) if stored.iter().any(|g| g == id) => {
                    Some(sources.stored(ward_core::protocolo::Alvo::Campo(id.to_owned()))?)
                }
                _ => return Err(AppError::InvalidRequest.into()),
            },
            SecretValue::Generated => Some(sources.generated(id)?),
            SecretValue::Loaded => Some(sources.loaded(id)?),
        })
    };
    let data = match kind {
        EntryType::Login => DataDraft::Login {
            url,
            username,
            password: secret("senha")?,
            totp_seed: secret("totp_semente")?,
        },
        EntryType::Api => DataDraft::Api {
            url,
            token: secret("token")?,
            variable,
        },
        EntryType::Aws => DataDraft::Aws {
            access_key_id: access,
            secret_access_key: secret("secret_access_key")?,
            region,
            role_arn: arn,
            session_token: secret("session_token")?,
        },
        EntryType::Smtp => DataDraft::Smtp {
            host,
            port,
            username,
            password: secret("senha")?,
            security,
        },
        EntryType::SecureNote => DataDraft::SecureNote {
            text: secret("texto")?,
        },
        EntryType::Database => DataDraft::Database {
            engine,
            host,
            port,
            db_name,
            username,
            password: secret("senha")?,
            tunnel,
        },
        EntryType::Server => DataDraft::Server {
            host,
            port,
            username,
            password: secret("senha")?,
            private_key: secret("chave_privada")?,
            passphrase: secret("passphrase")?,
            jump,
        },
        EntryType::FtpSftp => DataDraft::FtpSftp {
            protocol,
            host,
            port,
            username,
            password: secret("senha")?,
            private_key: secret("chave_privada")?,
            passphrase: secret("passphrase")?,
        },
        EntryType::Generic => DataDraft::Generic,
    };
    let mut extras = Vec::with_capacity(f.extras.len());
    for x in f.extras.drain(..) {
        let value = match x.value {
            ExtraValue::New(t) => CoreSecret::from_bytes(t.0.as_bytes().to_vec()),
            ExtraValue::Keep(name) => match editing {
                Some((_, stored)) if stored.contains(&name) => {
                    sources.stored(ward_core::protocolo::Alvo::Extra(name))?
                }
                _ => return Err(AppError::InvalidRequest.into()),
            },
        };
        extras.push(ExtraDraft {
            name: x.name,
            value,
            secret: x.secret,
        });
    }
    if errors.is_empty() {
        Ok(Built { data, extras })
    } else {
        Err(Missing::Fields(errors))
    }
}

fn copy_secret(s: &CoreSecret) -> CoreSecret {
    CoreSecret::from_bytes(s.expose().to_vec())
}

fn copy_optional(s: &Option<CoreSecret>) -> Option<CoreSecret> {
    s.as_ref().map(copy_secret)
}

/// Copy of the draft (the copied secrets also zero on drop) for the local validation
/// before the request: the errors of all the fields show up together and with the fixed
/// messages of `ward-core`, and a draft outside the protocol format (empty name, too
/// many tags) does not even become a request. The daemon validates again.
pub fn copy_draft(r: &Draft) -> Draft {
    let data = match &r.data {
        DataDraft::Login {
            url,
            username,
            password,
            totp_seed,
        } => DataDraft::Login {
            url: url.clone(),
            username: username.clone(),
            password: copy_optional(password),
            totp_seed: copy_optional(totp_seed),
        },
        DataDraft::Api {
            url,
            token,
            variable,
        } => DataDraft::Api {
            url: url.clone(),
            token: copy_optional(token),
            variable: variable.clone(),
        },
        DataDraft::Aws {
            access_key_id,
            secret_access_key,
            region,
            role_arn,
            session_token,
        } => DataDraft::Aws {
            access_key_id: access_key_id.clone(),
            secret_access_key: copy_optional(secret_access_key),
            region: region.clone(),
            role_arn: role_arn.clone(),
            session_token: copy_optional(session_token),
        },
        DataDraft::Smtp {
            host,
            port,
            username,
            password,
            security,
        } => DataDraft::Smtp {
            host: host.clone(),
            port: port.clone(),
            username: username.clone(),
            password: copy_optional(password),
            security: security.clone(),
        },
        DataDraft::SecureNote { text } => DataDraft::SecureNote {
            text: copy_optional(text),
        },
        DataDraft::Database {
            engine,
            host,
            port,
            db_name,
            username,
            password,
            tunnel,
        } => DataDraft::Database {
            engine: engine.clone(),
            host: host.clone(),
            port: port.clone(),
            db_name: db_name.clone(),
            username: username.clone(),
            password: copy_optional(password),
            tunnel: tunnel.clone(),
        },
        DataDraft::Server {
            host,
            port,
            username,
            password,
            private_key,
            passphrase,
            jump,
        } => DataDraft::Server {
            host: host.clone(),
            port: port.clone(),
            username: username.clone(),
            password: copy_optional(password),
            private_key: copy_optional(private_key),
            passphrase: copy_optional(passphrase),
            jump: jump.clone(),
        },
        DataDraft::FtpSftp {
            protocol,
            host,
            port,
            username,
            password,
            private_key,
            passphrase,
        } => DataDraft::FtpSftp {
            protocol: protocol.clone(),
            host: host.clone(),
            port: port.clone(),
            username: username.clone(),
            password: copy_optional(password),
            private_key: copy_optional(private_key),
            passphrase: copy_optional(passphrase),
        },
        DataDraft::Generic => DataDraft::Generic,
    };
    Draft {
        name: r.name.clone(),
        group: r.group.clone(),
        environment: r.environment.clone(),
        project: r.project.clone(),
        tags: r.tags.clone(),
        data,
        extras: r
            .extras
            .iter()
            .map(|x| ExtraDraft {
                name: x.name.clone(),
                value: copy_secret(&x.value),
                secret: x.secret,
            })
            .collect(),
        attachments: r
            .attachments
            .iter()
            .map(|a| AttachmentDraft {
                name: a.name.clone(),
                content: copy_secret(&a.content),
            })
            .collect(),
    }
}

/// Validates the draft on the app side (copy); `Err` with the errors per field.
pub fn validate_local(r: &Draft) -> Result<(), Vec<FieldError>> {
    match copy_draft(r).validate() {
        Ok(entry) => {
            drop(entry);
            Ok(())
        }
        Err(e) => Err(daemon_errors(&match ErroResposta::de_validacao(&e) {
            ErroResposta::Validacao(p) => p,
            _ => Vec::new(),
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_seed_is_normalized() {
        assert_eq!(
            normalize_seed(" jbsw y3dp ehpk 3pxp ").unwrap().as_str(),
            "JBSWY3DPEHPK3PXP"
        );
        assert_eq!(
            normalize_seed("otpauth://totp/Acme:wp?secret=jbswy3dpehpk3pxp&issuer=Acme")
                .unwrap()
                .as_str(),
            "JBSWY3DPEHPK3PXP"
        );
        assert_eq!(
            normalize_seed(
                "OTPAUTH://TOTP/x?SECRET=JBSWY3DPEHPK3PXP&algorithm=sha1&digits=6&period=30"
            )
            .unwrap()
            .as_str(),
            "JBSWY3DPEHPK3PXP"
        );
    }

    /// D35/V5.13: `otpauth://` other than SHA1/6/30 is refused with the fixed message; HOTP
    /// too. A URI without seed, with a repeated one or with a broken `%` is refused.
    #[test]
    fn attack_otpauth_other_than_sha1_6_30_is_refused() {
        for uri in [
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&algorithm=SHA256",
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&algorithm=SHA512",
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&digits=8",
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&digits=06",
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&period=60",
            "otpauth://totp/x?period=60&secret=JBSWY3DPEHPK3PXP",
            "otpauth://hotp/x?secret=JBSWY3DPEHPK3PXP&counter=1",
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&algorithm=%53HA256",
        ] {
            assert_eq!(normalize_seed(uri), Err(TOTP_UNSUPPORTED), "{uri}");
        }
        for uri in [
            "otpauth://totp/x?issuer=Acme",
            "otpauth://totp",
            "otpauth://totp/x?secret=AAAA&secret=BBBB",
            "otpauth://totp/x?secret=JBSW%Z3",
            "otpauth://totp/x?secret=%ff%fe",
        ] {
            assert_eq!(normalize_seed(uri), Err(TOTP_INVALID_URI), "{uri}");
        }
    }

    #[test]
    fn attack_problem_forged_by_the_daemon_does_not_become_free_text() {
        let p = |field: &str, reason: &str| Problema {
            campo: field.to_owned(),
            posicao: None,
            motivo: reason.to_owned(),
        };
        let e = daemon_errors(&[
            p("senha", "obrigatório"),
            p("<img src=x>", "obrigatório"),
            p("usuário", "a\u{202E}b"),
            p("nome", &"x".repeat(500)),
            p("url", "linha\nnova"),
        ]);
        assert_eq!(e[0].field, "senha");
        assert_eq!(e[0].reason, "obrigatório");
        assert_eq!(e[1].field, "formulario");
        assert_eq!(e[2].reason, "inválido");
        assert_eq!(e[3].reason, "inválido");
        assert_eq!(e[4].reason, "inválido");
    }

    /// D29/V5.12: the mask budget refusal (on the whole entry) shows up on the
    /// attachments field; another reason on the whole entry does not become an
    /// attachment error.
    #[test]
    fn mask_budget_refusal_goes_to_the_attachments() {
        let p = |field: &str, reason: &str| Problema {
            campo: field.to_owned(),
            posicao: None,
            motivo: reason.to_owned(),
        };
        let e = daemon_errors(&[
            p("entrada", TOO_LARGE_FOR_MASK),
            p("entrada", "outra coisa"),
            p("túnel SSH", "precisa ser uma entrada servidor existente"),
            p("autenticação", "informe a senha ou a chave"),
        ]);
        assert_eq!(e[0].field, "anexos");
        assert_eq!(e[0].reason, TOO_LARGE_FOR_MASK);
        assert_eq!(e[1].field, "formulario");
        assert_eq!(e[2].field, "tunel");
        assert_eq!(e[3].field, "autenticacao");
    }

    #[test]
    fn attack_debug_of_the_secret_text_does_not_show_the_value() {
        let f: Form = serde_json::from_str(
            r#"{"tipo":"login","nome":"x","grupo":"g","ambiente":"dev","projeto":"","tags":[],
                "campos":{},"segredos":{"senha":{"novo":"falso-segredo-123"}},"extras":[
                {"nome":"A","segredo":true,"valor":{"novo":"falso-extra-456"}}]}"#,
        )
        .unwrap();
        let text = format!("{f:?}");
        assert!(!text.contains("falso"), "{text}");
        assert!(text.contains("••••"));
    }

    #[test]
    fn attack_form_with_unknown_key_is_refused() {
        for json in [
            r#"{"tipo":"login","nome":"x","grupo":"g","ambiente":"dev","projeto":"","tags":[],"campos":{},"segredos":{},"extras":[],"nivel":"livre"}"#,
            r#"{"tipo":"login","nome":"x","grupo":"g","ambiente":"dev","projeto":"","tags":[],"campos":{},"segredos":{"senha":{"revelar":"x"}},"extras":[]}"#,
            r#"{"tipo":"login","nome":"x","grupo":"g","ambiente":"dev","projeto":"","tags":[],"campos":{},"segredos":{},"extras":[{"nome":"A","segredo":true,"valor":{"novo":"v"},"x":1}]}"#,
            r#"{"tipo":"login","nome":"x","grupo":"g","ambiente":"dev","projeto":"","tags":[],"campos":{},"segredos":{"senha":{"novo":5}},"extras":[]}"#,
        ] {
            assert!(serde_json::from_str::<Form>(json).is_err(), "{json}");
        }
    }

    /// m2 Phase 32: the `rename`s keep the webview wire (m2:DT3) until Phase 54: the old
    /// keys and values are read and written, the new Rust names are not.
    #[test]
    fn attack_new_rust_names_are_not_wire_names() {
        let ok = r#"{"tipo":"login","nome":"x","grupo":"g","ambiente":"dev","projeto":"","tags":[],"campos":{},"segredos":{"senha":"vazio","totp_semente":"gerada"},"extras":[{"nome":"A","segredo":true,"valor":{"manter":"A"}}],"anexos":[1],"anexos_mantidos":["a"]}"#;
        let f: Form = serde_json::from_str(ok).unwrap();
        assert_eq!((f.kind.as_str(), f.attachments.len()), ("login", 1));
        for json in [
            r#"{"kind":"login","nome":"x","grupo":"g","ambiente":"dev","projeto":"","tags":[],"campos":{},"segredos":{},"extras":[]}"#,
            r#"{"tipo":"login","nome":"x","grupo":"g","ambiente":"dev","projeto":"","tags":[],"campos":{},"segredos":{"senha":"empty"},"extras":[]}"#,
            r#"{"tipo":"login","nome":"x","grupo":"g","ambiente":"dev","projeto":"","tags":[],"campos":{},"segredos":{"senha":{"new":"v"}},"extras":[]}"#,
        ] {
            assert!(serde_json::from_str::<Form>(json).is_err(), "{json}");
        }
        let e = serde_json::to_string(&FieldError {
            field: "nome",
            position: Some(1),
            reason: "x".into(),
        })
        .unwrap();
        assert_eq!(e, r#"{"campo":"nome","posicao":1,"motivo":"x"}"#);
    }
}
