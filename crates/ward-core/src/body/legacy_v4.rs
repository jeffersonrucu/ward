//! Closed conversion table of the body v4 names (0.1.x) to the v5 ones (m2:DT4).
//!
//! Only the reading of a body v4 calls this module: [`super::read_raw`] with
//! [`super::Version::V4`]. Nothing here writes, and no other path (protocol, CLI, app,
//! validation of a draft) knows the v4 names: the write only produces v5, through the
//! `name()` of each enum.
//!
//! - The tables are closed: a name outside them (a typo, another case, a trailing space,
//!   a v5 name inside a v4 body) gives `None`, and the whole body is refused with
//!   [`super::BodyError::Invalid`] (m2:RM2, m2:V4.8).
//! - Comparison is on the exact bytes; nothing is trimmed, folded or normalized.
//! - Environment, engine and FTP protocol have the same names in v4 and v5 and do not go
//!   through here.

use crate::entry::{AccessLevel, EntryType, SmtpSecurity};

/// Type stored in a body v4.
pub(super) fn kind(stored: &[u8]) -> Option<EntryType> {
    Some(match stored {
        b"login" => EntryType::Login,
        b"banco" => EntryType::Database,
        b"servidor" => EntryType::Server,
        b"api" => EntryType::Api,
        b"aws" => EntryType::Aws,
        b"smtp" => EntryType::Smtp,
        b"ftp-sftp" => EntryType::FtpSftp,
        b"nota-segura" => EntryType::SecureNote,
        b"generico" => EntryType::Generic,
        _ => return None,
    })
}

/// Access level stored in a body v4.
pub(super) fn level(stored: &[u8]) -> Option<AccessLevel> {
    Some(match stored {
        b"bloqueado" => AccessLevel::Blocked,
        b"com-aprovacao" => AccessLevel::Approval,
        b"livre" => AccessLevel::Free,
        _ => return None,
    })
}

/// SMTP security stored in a body v4, as the v5 name (the draft validation reads it with
/// [`SmtpSecurity::from_name`]).
pub(super) fn smtp_security(stored: &[u8]) -> Option<&'static [u8]> {
    let security = match stored {
        b"nenhuma" => SmtpSecurity::None,
        b"starttls" => SmtpSecurity::Starttls,
        b"tls" => SmtpSecurity::Tls,
        _ => return None,
    };
    Some(security.name().as_bytes())
}
