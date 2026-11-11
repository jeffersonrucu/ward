//! The app's requests to the daemon over the Phase 20 protocol (the `ward-core` protocol module),
//! with the app role (D5): status, list, detail, copy, reveal, totp, save entry, set
//! level (Phase 35), unlock (Phase 31) and, on the Settings screen (Phase 38), config,
//! change PIN, change password and lock timeout; in the external backup (Phase 48),
//! external backup, USB drives and set external backup; in the restore (Phase 47),
//! backups and restore backup.
//!
//! Here the request becomes the frame body (the request encoder) and the response is read
//! with the client's strict reading (the response decoder), which refuses a type of
//! another role and a forged detail (secret extra with a value, secret field in the text
//! fields). The transport is the [`Daemon`]: the `wardd` socket
//! ([`crate::daemon::DaemonSocket`]) or, in the tests and in the test bridge, the fake
//! daemon ([`crate::fake_daemon`]).
//!
//! No function here returns a secret to the interface: [`copy`] and [`reveal`] read a
//! value into a [`Secret`] (zeroed on drop); the [`crate::service`] decides where it
//! goes: the clipboard (D4), the "reveal" on click (S11) or the draft that goes back to
//! the daemon on edit (Phase 33).

use ward_core::entry::{AccessLevel, Draft, Secret as CoreSecret};
use ward_core::protocol::{
    self, DetalheEntrada, ItemLista, Pendrive, Problem, Request, ResponseError, Resposta, Role,
    Target,
};
use zeroize::Zeroizing;

use crate::AppError;
use crate::daemon::{Daemon, VaultStatus};
use crate::secret::Secret;
use crate::terminal::Token;

/// One request and the response already read. A daemon error becomes an [`AppError`]
/// with a fixed message (the protocol code does not reach the interface).
fn ask<D: Daemon + ?Sized>(daemon: &mut D, request: &Request) -> Result<Resposta, AppError> {
    let frame =
        protocol::encode_request(Role::App, request).map_err(|_| AppError::InvalidRequest)?;
    let body = daemon.exchange(&frame)?;
    match protocol::decode_response(Role::App, &body) {
        Ok(Resposta::Erro(e)) => Err(daemon_error(&e)),
        Ok(r) => Ok(r),
        Err(_) => Err(AppError::InvalidResponse),
    }
}

/// The protocol errors the screen tells apart; the rest is "refused".
fn daemon_error(e: &ResponseError) -> AppError {
    match e {
        ResponseError::Locked | ResponseError::LockedTimedOut => AppError::Locked,
        ResponseError::EntryNotFound => AppError::EntryNotFound,
        ResponseError::FieldNotFound => AppError::FieldNotFound,
        ResponseError::Unauthorized => AppError::Unauthorized,
        ResponseError::VaultAlreadyExists => AppError::VaultAlreadyExists,
        ResponseError::WriteFailed => AppError::VaultNotWritten,
        ResponseError::WrongPin => AppError::WrongPin,
        ResponseError::WrongPassword => AppError::WrongPassword,
        ResponseError::Rollback => AppError::VaultRollback,
        ResponseError::UsbDrivesUnavailable => AppError::UsbDrivesUnavailable,
        ResponseError::BackupNotFound => AppError::BackupNotFound,
        ResponseError::BackupUnavailable => AppError::BackupUnavailable,
        ResponseError::VaultChanged => AppError::VaultChanged,
        _ => AppError::Refused,
    }
}

/// Vault status.
pub fn status<D: Daemon + ?Sized>(daemon: &mut D) -> Result<VaultStatus, AppError> {
    match ask(daemon, &Request::Status)? {
        Resposta::Estado {
            cofre_existe: false,
            ..
        } => Ok(VaultStatus::NoVault),
        Resposta::Estado { travado: true, .. } => Ok(VaultStatus::Locked),
        Resposta::Estado { travado: false, .. } => Ok(VaultStatus::Unlocked),
        _ => Err(AppError::InvalidResponse),
    }
}

/// Create vault (first use, Phase 30): the master password and the PIN already checked
/// by the first use module. The daemon stores the PIN hash in the body config (D30)
/// and leaves the vault unlocked. The values go in a core `Secret` (zeroed on drop) and
/// the frame is zeroed after the exchange.
pub fn create_vault<D: Daemon + ?Sized>(
    daemon: &mut D,
    password: &[u8],
    pin: &[u8],
) -> Result<(), AppError> {
    let request = Request::CreateVault {
        password: CoreSecret::from_bytes(password.to_vec()),
        pin: CoreSecret::from_bytes(pin.to_vec()),
    };
    match ask(daemon, &request)? {
        Resposta::Ok => Ok(()),
        _ => Err(AppError::InvalidResponse),
    }
}

/// The role the daemon gave this connection, from the status response (D63). The app
/// and the test bridge ask this **first** and, without the app role, do not use the
/// connection.
pub fn role<D: Daemon + ?Sized>(daemon: &mut D) -> Result<Role, AppError> {
    match ask(daemon, &Request::Status)? {
        Resposta::Estado { papel, .. } => Ok(papel),
        _ => Err(AppError::InvalidResponse),
    }
}

/// Is this connection's role the app one? (The same as [`role`], for whoever does not
/// depend on `ward-core`, like the test bridge.)
pub fn has_app_role<D: Daemon + ?Sized>(daemon: &mut D) -> Result<bool, AppError> {
    role(daemon).map(|p| p == Role::App)
}

/// Open app (D57): asks the daemon to open the real app instance. No field: nothing from
/// the request reaches the process the daemon opens.
pub fn open_app<D: Daemon + ?Sized>(daemon: &mut D) -> Result<(), AppError> {
    match ask(daemon, &Request::OpenApp)? {
        Resposta::Ok => Ok(()),
        _ => Err(AppError::InvalidResponse),
    }
}

/// Open app over the connection of a plain client (the app opened from the menu, which
/// dropped the group before connecting): the daemon writes the response in the role it
/// gave, the agent one (D63), and it is read in that role.
pub fn open_app_as_agent<D: Daemon + ?Sized>(daemon: &mut D) -> Result<(), AppError> {
    let frame = protocol::encode_request(Role::Agent, &Request::OpenApp)
        .map_err(|_| AppError::InvalidRequest)?;
    let body = daemon.exchange(&frame)?;
    match protocol::decode_response(Role::Agent, &body) {
        Ok(Resposta::Ok) => Ok(()),
        Ok(Resposta::Erro(e)) => Err(daemon_error(&e)),
        _ => Err(AppError::InvalidResponse),
    }
}

/// List: every entry (the app also sees the blocked ones), no secret.
pub fn list<D: Daemon + ?Sized>(daemon: &mut D) -> Result<Vec<ItemLista>, AppError> {
    match ask(daemon, &Request::List)? {
        Resposta::Lista(items) => Ok(items),
        _ => Err(AppError::InvalidResponse),
    }
}

/// Detail of an entry: text fields, secret names, extras without the secret value and
/// attachments without the content. The response must be of the requested entry.
pub fn detail<D: Daemon + ?Sized>(daemon: &mut D, entry: &str) -> Result<DetalheEntrada, AppError> {
    let request = Request::Detail {
        entry: entry.to_owned(),
    };
    match ask(daemon, &request)? {
        Resposta::Detalhe(d) if d.nome == entry => Ok(d),
        _ => Err(AppError::InvalidResponse),
    }
}

/// Copy: the value of a secret, only for the clipboard (D4).
pub fn copy<D: Daemon + ?Sized>(
    daemon: &mut D,
    entry: &str,
    target: Target,
) -> Result<Secret, AppError> {
    let request = Request::Copy {
        entry: entry.to_owned(),
        target,
    };
    match ask(daemon, &request)? {
        Resposta::Segredo(s) => Ok(Secret::new(s.expose().to_vec())),
        _ => Err(AppError::InvalidResponse),
    }
}

/// Reveal: the value of a secret to show on the "reveal" click (S11) or to keep the
/// value when editing the entry (the draft carries every secret).
pub fn reveal<D: Daemon + ?Sized>(
    daemon: &mut D,
    entry: &str,
    target: Target,
) -> Result<Secret, AppError> {
    let request = Request::Reveal {
        entry: entry.to_owned(),
        target,
    };
    match ask(daemon, &request)? {
        Resposta::Segredo(s) => Ok(Secret::new(s.expose().to_vec())),
        _ => Err(AppError::InvalidResponse),
    }
}

/// Current TOTP code of an entry and the seconds until the next one.
pub struct TotpCode {
    pub code: Zeroizing<String>,
    pub remaining: u64,
}

/// TOTP: the current code (6 digits, D35) and how long until it changes (1 to 30 s).
pub fn totp<D: Daemon + ?Sized>(daemon: &mut D, entry: &str) -> Result<TotpCode, AppError> {
    let request = Request::Totp {
        entry: entry.to_owned(),
    };
    match ask(daemon, &request)? {
        Resposta::CodigoTotp { codigo, restante }
            if codigo.len() == 6 && (1..=30).contains(&restante) =>
        {
            Ok(TotpCode {
                code: codigo,
                remaining: restante,
            })
        }
        _ => Err(AppError::InvalidResponse),
    }
}

/// Issue token: the single-use token of "Connect" (D3), checked before it goes to an
/// argv (the token format check).
pub fn issue_token<D: Daemon + ?Sized>(daemon: &mut D, entry: &str) -> Result<Token, AppError> {
    let request = Request::IssueToken {
        entry: entry.to_owned(),
    };
    match ask(daemon, &request)? {
        Resposta::Token(t) => Token::from_response(t.expose()).ok_or(AppError::InvalidResponse),
        _ => Err(AppError::InvalidResponse),
    }
}

/// Result of saving an entry: saved, or the validation problems per field (only fixed
/// texts of `ward-core`; no user value).
pub enum Save {
    Saved,
    Problems(Vec<Problem>),
}

/// Save entry: creates (`previous` empty) or edits. The level does not go in the request
/// (S9). `kept_attachments` (Phase 39b): names of the edited entry's attachments that
/// stay in it; the daemon copies the content, which never goes through the app.
pub fn save<D: Daemon + ?Sized>(
    daemon: &mut D,
    previous: Option<String>,
    draft: Draft,
    kept_attachments: Vec<String>,
) -> Result<Save, AppError> {
    let request = Request::SaveEntry {
        previous,
        draft,
        keep_attachments: kept_attachments,
    };
    let frame =
        protocol::encode_request(Role::App, &request).map_err(|_| AppError::InvalidRequest)?;
    drop(request);
    let body = daemon.exchange(&frame)?;
    drop(frame);
    match protocol::decode_response(Role::App, &body) {
        Ok(Resposta::Ok) => Ok(Save::Saved),
        Ok(Resposta::Erro(ResponseError::Validation(p))) => Ok(Save::Problems(p)),
        Ok(Resposta::Erro(e)) => Err(daemon_error(&e)),
        _ => Err(AppError::InvalidResponse),
    }
}

/// Set level (S9): the PIN goes only in this request. The frame with the PIN is dropped
/// (and zeroed: the request encoder returns `Zeroizing`) right after the exchange. Wrong
/// PIN gives [`AppError::WrongPin`]; the count and the lock on the 5th error belong to
/// the daemon.
pub fn set_level<D: Daemon + ?Sized>(
    daemon: &mut D,
    entry: &str,
    level: AccessLevel,
    pin: &crate::nivel::Pin,
) -> Result<(), AppError> {
    let request = Request::SetLevel {
        entry: entry.to_owned(),
        level,
        pin: pin.para_o_protocolo(),
    };
    match ask(daemon, &request)? {
        Resposta::Ok => Ok(()),
        _ => Err(AppError::InvalidResponse),
    }
}

/// What the daemon said about the approval window's answer (Phase 36).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answered {
    Ok,
    WrongPin,
    /// The daemon no longer has the request (it expired, or another answer came first).
    Expired,
}

/// Answer approval (S10): the PIN goes only in this request, and only on the "Allow"
/// ones (the protocol refuses a PIN on "Deny" and "Allow" without a PIN). The frame with
/// the PIN is dropped (and zeroed) right after the exchange. The error count and the
/// lock on the 5th belong to the daemon.
pub fn answer_approval<D: Daemon + ?Sized>(
    daemon: &mut D,
    id: u64,
    decision: protocol::Decision,
    pin: Option<&crate::nivel::Pin>,
    stdin_confirmed: bool,
) -> Result<Answered, AppError> {
    if decision.requires_pin() != pin.is_some() || (stdin_confirmed && !decision.requires_pin()) {
        return Err(AppError::InvalidRequest);
    }
    let request = Request::AnswerApproval {
        request: id,
        decision,
        pin: pin.map(crate::nivel::Pin::para_o_protocolo),
        confirmed_entry: stdin_confirmed,
    };
    let frame =
        protocol::encode_request(Role::App, &request).map_err(|_| AppError::InvalidRequest)?;
    drop(request);
    let body = daemon.exchange(&frame)?;
    drop(frame);
    match protocol::decode_response(Role::App, &body) {
        Ok(Resposta::Ok) => Ok(Answered::Ok),
        Ok(Resposta::Erro(ResponseError::WrongPin)) => Ok(Answered::WrongPin),
        Ok(Resposta::Erro(ResponseError::Expired)) => Ok(Answered::Expired),
        Ok(Resposta::Erro(e)) => Err(daemon_error(&e)),
        Ok(_) => Err(AppError::InvalidResponse),
        Err(_) => Err(AppError::InvalidResponse),
    }
}

/// Unlock with the password (Phase 31): the master password only goes to the request,
/// in a core `Secret` (which zeroes). `ok` unlocks; a wrong password is
/// [`AppError::WrongPassword`], with no count (D20: the wait belongs to the daemon,
/// before it looks at the password).
pub fn unlock<D: Daemon + ?Sized>(
    daemon: &mut D,
    password: &crate::destrave::SenhaMestra,
) -> Result<(), AppError> {
    let request = Request::Unlock {
        password: password.para_o_protocolo(),
    };
    match ask(daemon, &request)? {
        Resposta::Ok => Ok(()),
        _ => Err(AppError::InvalidResponse),
    }
}

/// Config (Phase 38): lock timeout and external backup on or off. The strict reading
/// already refused minutes outside D45 and an extra field.
pub fn settings<D: Daemon + ?Sized>(daemon: &mut D) -> Result<crate::settings::Settings, AppError> {
    match ask(daemon, &Request::Config)? {
        Resposta::Configuracao {
            minutos_trava,
            backup_externo,
            backup_local,
        } => Ok(crate::settings::Settings {
            lock_minutes: minutos_trava,
            external_backup: backup_externo,
            local_backup: backup_local,
        }),
        _ => Err(AppError::InvalidResponse),
    }
}

/// Change PIN with the current and the new one (Phase 38, S9): both PINs already checked
/// in format by the settings module, in a core `Secret` (zeroes). A wrong current
/// PIN is [`AppError::WrongPin`]; the count and the lock on the 5th error belong to the
/// daemon.
pub fn change_pin<D: Daemon + ?Sized>(
    daemon: &mut D,
    current: &[u8],
    new: &[u8],
) -> Result<(), AppError> {
    let request = Request::ChangePin {
        current: CoreSecret::from_bytes(current.to_vec()),
        new: CoreSecret::from_bytes(new.to_vec()),
    };
    match ask(daemon, &request)? {
        Resposta::Ok => Ok(()),
        _ => Err(AppError::InvalidResponse),
    }
}

/// Change password with the current and the new one (Phase 38): a wrong current password
/// is [`AppError::WrongPassword`] (D52); the daemon writes the vault with the new key
/// and a new generation (D39, D53).
pub fn change_password<D: Daemon + ?Sized>(
    daemon: &mut D,
    current: &[u8],
    new: &[u8],
) -> Result<(), AppError> {
    let request = Request::ChangePassword {
        current: CoreSecret::from_bytes(current.to_vec()),
        new: CoreSecret::from_bytes(new.to_vec()),
    };
    match ask(daemon, &request)? {
        Resposta::Ok => Ok(()),
        _ => Err(AppError::InvalidResponse),
    }
}

/// Lock timeout in minutes (Phase 38, D30): the daemon stores it in the vault config.
pub fn set_lock_timeout<D: Daemon + ?Sized>(daemon: &mut D, minutes: u32) -> Result<(), AppError> {
    match ask(daemon, &Request::LockTimeout { minutes })? {
        Resposta::Ok => Ok(()),
        _ => Err(AppError::InvalidResponse),
    }
}

/// Lock ("Lock now" of the window and of the tray, Phase 39): no field. The daemon
/// zeroes the key and answers `ok` (already locked, also `ok`).
pub fn lock<D: Daemon + ?Sized>(daemon: &mut D) -> Result<(), AppError> {
    match ask(daemon, &Request::Lock)? {
        Resposta::Ok => Ok(()),
        _ => Err(AppError::InvalidResponse),
    }
}

/// External backup (Phase 46/48): on or off and the chosen UUID (the strict reading
/// already checked the D45 format and that "on" has a UUID).
pub fn external_backup<D: Daemon + ?Sized>(
    daemon: &mut D,
) -> Result<(bool, Option<String>), AppError> {
    match ask(daemon, &Request::ExternalBackup)? {
        Resposta::BackupExterno { ligado, uuid } => Ok((ligado, uuid)),
        _ => Err(AppError::InvalidResponse),
    }
}

/// USB drives (Phase 46/48): UUID, label (no control characters, up to the label limit) and
/// mounted; never the mount path. udisks2 without an answer is
/// [`AppError::UsbDrivesUnavailable`].
pub fn usb_drives<D: Daemon + ?Sized>(daemon: &mut D) -> Result<Vec<Pendrive>, AppError> {
    match ask(daemon, &Request::UsbDrives)? {
        Resposta::Pendrives(l) => Ok(l),
        _ => Err(AppError::InvalidResponse),
    }
}

/// Set external backup with on/off and an optional UUID (Phase 46/48): the daemon stores
/// it in the vault config (new generation, D39).
pub fn set_external_backup<D: Daemon + ?Sized>(
    daemon: &mut D,
    enabled: bool,
    uuid: Option<String>,
) -> Result<(), AppError> {
    match ask(daemon, &Request::SetExternalBackup { enabled, uuid })? {
        Resposta::Ok => Ok(()),
        _ => Err(AppError::InvalidResponse),
    }
}

/// Backups (Phase 45/47/49a): the local backup versions (name and sealed generation),
/// from the newest to the oldest. The strict reading already refused a name out of
/// format, generation 0, an extra field and more than `MAX_BACKUPS`.
pub fn backups<D: Daemon + ?Sized>(
    daemon: &mut D,
) -> Result<Vec<ward_core::protocol::VersaoBackup>, AppError> {
    match ask(daemon, &Request::Backups)? {
        Resposta::Backups(l) => Ok(l),
        _ => Err(AppError::InvalidResponse),
    }
}

/// Restore backup with version, password and PIN (Phase 45/47; D17, D39, S9): that
/// version's master password and the current PIN go only in this request, in a core
/// `Secret` (zeroes), and the frame is dropped right after the exchange. A wrong PIN is
/// [`AppError::WrongPin`] (the daemon counts; the 5th locks); the wrong version
/// password, [`AppError::WrongPassword`].
pub fn restore_backup<D: Daemon + ?Sized>(
    daemon: &mut D,
    version: &str,
    password: &crate::destrave::SenhaMestra,
    pin: &crate::nivel::Pin,
) -> Result<(), AppError> {
    let request = Request::RestoreBackup {
        version: version.to_owned(),
        password: password.para_o_protocolo(),
        pin: pin.para_o_protocolo(),
    };
    let frame =
        protocol::encode_request(Role::App, &request).map_err(|_| AppError::InvalidRequest)?;
    drop(request);
    let body = daemon.exchange(&frame)?;
    drop(frame);
    match protocol::decode_response(Role::App, &body) {
        Ok(Resposta::Ok) => Ok(()),
        Ok(Resposta::Erro(e)) => Err(daemon_error(&e)),
        _ => Err(AppError::InvalidResponse),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake_daemon::FakeDaemon;

    /// Daemon that always returns the same body (the "server" is the attacker).
    struct FixedReply(Vec<u8>);

    impl Daemon for FixedReply {
        fn exchange(&mut self, _request: &[u8]) -> Result<Zeroizing<Vec<u8>>, AppError> {
            Ok(Zeroizing::new(self.0.clone()))
        }
    }

    #[test]
    fn status_list_and_detail_over_the_protocol() {
        let mut d = FakeDaemon::full();
        assert_eq!(status(&mut d), Ok(VaultStatus::Unlocked));
        let items = list(&mut d).unwrap();
        assert_eq!(
            items.len(),
            serde_json::from_str::<serde_json::Value>(include_str!(
                "../../../dev/fixtures/entries.json"
            ))
            .unwrap()["entradas"]
                .as_array()
                .unwrap()
                .len()
        );
        let det = detail(&mut d, "acme-pg-dev").unwrap();
        assert_eq!(det.segredos, vec!["senha".to_owned()]);
        assert_eq!(
            detail(&mut d, "nao-existe").unwrap_err(),
            AppError::EntryNotFound
        );
        assert_eq!(status(&mut FakeDaemon::empty()), Ok(VaultStatus::Unlocked));
        assert!(list(&mut FakeDaemon::empty()).unwrap().is_empty());
    }

    #[test]
    fn attack_response_of_another_type_or_forged_is_refused() {
        // A "daemon" that answers a credential to the list request, a detail of another
        // entry, a detail with the secret extra's value and a response of another
        // version: all become `InvalidResponse`, nothing reaches the interface.
        let secret_instead_of_list =
            br#"{"v":1,"tipo":"segredo","dados":{"valor":"ZmFsc28tUEc="}}"#.to_vec();
        assert_eq!(
            list(&mut FixedReply(secret_instead_of_list)).unwrap_err(),
            AppError::InvalidResponse
        );
        let mut fake = FakeDaemon::full();
        let frame = protocol::encode_request(
            Role::App,
            &Request::Detail {
                entry: "acme-pg-dev".into(),
            },
        )
        .unwrap();
        let other = fake.exchange(&frame).unwrap().to_vec();
        assert_eq!(
            detail(&mut FixedReply(other), "acme-pg-prod").unwrap_err(),
            AppError::InvalidResponse
        );
        // The real detail of the generic test entry, with the secret extra's value
        // slipped in by the "daemon".
        let frame = protocol::encode_request(
            Role::App,
            &Request::Detail {
                entry: "gama-generico".into(),
            },
        )
        .unwrap();
        let real = String::from_utf8(fake.exchange(&frame).unwrap().to_vec()).unwrap();
        let target = r#""segredo":true"#;
        assert!(real.contains(target), "{real}");
        let forged = real
            .replacen(target, r#""segredo":true,"valor":"falso-licenca-01""#, 1)
            .into_bytes();
        assert_eq!(
            detail(&mut FixedReply(forged), "gama-generico").unwrap_err(),
            AppError::InvalidResponse
        );
        let version = br#"{"v":2,"tipo":"lista","dados":{"itens":[]}}"#.to_vec();
        assert_eq!(
            list(&mut FixedReply(version)).unwrap_err(),
            AppError::InvalidResponse
        );
    }

    #[test]
    fn attack_d65_list_without_summary_or_with_secret_in_summary_is_refused() {
        // The app role list carries the summary of each item (D65 c), no secret.
        let mut fake = FakeDaemon::full();
        let items = list(&mut fake).unwrap();
        assert!(
            items
                .iter()
                .all(|i| i.resumo.is_some() && i.nivel.is_some())
        );
        let frame = protocol::encode_request(Role::App, &Request::List).unwrap();
        let real = String::from_utf8(fake.exchange(&frame).unwrap().to_vec()).unwrap();
        for s in crate::fake_daemon::SECRETS {
            assert!(!real.contains(s), "segredo na lista do app");
        }
        // The agent list (no level, no summary) does not serve the app.
        let agent_list: Vec<ItemLista> = items
            .iter()
            .map(|i| ItemLista {
                nivel: None,
                resumo: None,
                ..i.clone()
            })
            .collect();
        let q = protocol::encode_response(Role::Agent, &Resposta::Lista(agent_list))
            .unwrap()
            .to_vec();
        assert_eq!(
            list(&mut FixedReply(q)).unwrap_err(),
            AppError::InvalidResponse
        );
        // A "daemon" that slips a secret into the summary (secret field among the text
        // fields, or an extra key): refused whole, nothing reaches the interface.
        let target = r#""resumo":{"#;
        assert!(real.contains(target));
        for swap in [
            r#""resumo":{"campos":{"senha":"falso-PG:s3nha/01@#"},"segredos":[],"extras":0,"anexos":0},"x":{"#,
            r#""resumo":{"valor":"ZmFsc28tUEc=","#,
        ] {
            let f = real.replacen(target, swap, 1).into_bytes();
            assert_eq!(
                list(&mut FixedReply(f)).unwrap_err(),
                AppError::InvalidResponse,
                "{swap}"
            );
        }
    }

    #[test]
    fn daemon_errors_have_a_fixed_message() {
        let denied = br#"{"v":1,"tipo":"erro","dados":{"codigo":"nao_autorizado","mensagem":"<img src=x> falso-PG:s3nha/01@#"}}"#.to_vec();
        let e = list(&mut FixedReply(denied)).unwrap_err();
        assert_eq!(e, AppError::Unauthorized);
        assert!(!e.to_string().contains("img"));
        assert!(!e.to_string().contains("falso"));
        let locked =
            br#"{"v":1,"tipo":"erro","dados":{"codigo":"travado","mensagem":"x"}}"#.to_vec();
        assert_eq!(
            status(&mut FixedReply(locked)).unwrap_err(),
            AppError::Locked
        );
    }
}
