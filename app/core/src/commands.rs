//! The app's own commands: the single list of what the interface may request (S11).
//!
//! The Tauri `build.rs` registers exactly these names in the app manifest (without it,
//! every registered command would be allowed), the main window capability allows only
//! the `allow-<command>` permissions of [`MAIN_COMMANDS`], the approval window capability
//! (Phase 36) only those of [`APPROVAL_COMMANDS`], and the test bridge refuses any other
//! name. In common, only [`LISTEN_NOTICES`] (channel per window, notice without data):
//! the main window does not answer approvals, and the approval window does not read
//! credentials nor reveal anything.
//! Later phases add commands here, one per screen that needs it.

use ward_core::limits;

use crate::AppError;

/// Vault status: locked, unlocked or no vault. No argument.
pub const VAULT_STATUS: &str = "estado_do_cofre";
/// Copy a field (entry and field): the value goes from the daemon to the clipboard on
/// the Rust side (D4); the response does not carry the value.
pub const COPY_FIELD: &str = "copiar_campo";

/// List credentials: the entries for the Credentials screen (Phase 32), no secret. No
/// argument.
pub const LIST_CREDENTIALS: &str = "listar_credenciais";
/// Credential detail (entry): the detail panel of an entry (Phase 32): text fields,
/// only the names of the secrets, extras without the secret value and attachments
/// without the content.
pub const CREDENTIAL_DETAIL: &str = "detalhe_credencial";
/// Generate password (size, lowercase, uppercase, digits, symbols): standalone
/// generator (Phase 37). The password stays on the Rust side; the response carries only
/// size, strength and symbols.
pub const GENERATE_PASSWORD: &str = "gerar_senha";
/// Copy the generated password: the standalone password goes to the clipboard (D4); the
/// response does not carry the value.
pub const COPY_GENERATED_PASSWORD: &str = "copiar_senha_gerada";
/// Reveal the generated password: the value, only on the "reveal" click (S11).
pub const REVEAL_GENERATED_PASSWORD: &str = "revelar_senha_gerada";
/// Discard the generated password: zeroes the standalone password when leaving the
/// screen.
pub const DISCARD_GENERATED_PASSWORD: &str = "descartar_senha_gerada";
/// Reveal a field (entry and field): the value of a secret of the entry, only on the
/// "reveal" click (S11, D65 b). The screen removes it in 30 s, on hide or on leaving.
pub const REVEAL_FIELD: &str = "revelar_campo";
/// TOTP code (entry): the current TOTP code and the seconds until it changes (Phase
/// 33). The seed never goes to the interface.
pub const TOTP_CODE: &str = "codigo_totp";
/// Copy the TOTP code (entry): the code goes to the clipboard (D4).
pub const COPY_TOTP_CODE: &str = "copiar_codigo_totp";
/// Use the generated password (field): "Use this password" of the form generator. The
/// password goes to the field on the Rust side; the response carries only the size.
pub const USE_GENERATED_PASSWORD: &str = "usar_senha_gerada";
/// Discard the form: on cancelling or leaving the form, zeroes the used passwords.
pub const DISCARD_FORM: &str = "descartar_formulario";
/// Save a credential (previous name and form): creates or edits an entry (Phase 33). A
/// kept secret and a generated password are put in the draft on the Rust side.
pub const SAVE_CREDENTIAL: &str = "salvar_credencial";
/// Set the level (entry, level and PIN): changes the access level with the PIN (Phase
/// 35, S9). The PIN goes only to the daemon; the response carries the new level.
pub const SET_LEVEL: &str = "mudar_nivel";
/// Choose an attachment: the Rust side chooses and reads the file (Phase 34); the
/// response carries id, name and size, or the notice. No argument: the interface does
/// not choose a path.
pub const CHOOSE_ATTACHMENT: &str = "escolher_anexo";
/// Remove an attachment (id): removes an attachment chosen and not yet saved.
pub const REMOVE_ATTACHMENT: &str = "remover_anexo";
/// Load a key (field): "Load from file" of the "Private key" field (Phase 53a). The Rust
/// side chooses, reads and checks the file and keeps the key until saving; the response
/// carries only the size, or the notice. The interface sends no path.
pub const LOAD_KEY: &str = "carregar_chave";
/// Connect (entry): "Connect" of a server entry (D3); the token does not come back.
pub const CONNECT: &str = "conectar";
/// Create the vault (data: password, confirmation, PIN and PIN confirmation): first use
/// (Phase 30). Checked on the Rust side; the daemon stores only the PIN hash (D30). The
/// response carries "created" or the errors per field, never the value.
pub const CREATE_VAULT: &str = "criar_cofre";
/// Unlock (password): unlocks with the master password (Phase 31). The password goes
/// only to the daemon, in a type that zeroes; the response neither carries it nor says
/// how many attempts there were.
pub const UNLOCK: &str = "destravar";
/// Lock: "Lock now" of the side navigation (Phase 39; the tray locks on the Rust side).
/// No argument; clears the clipboard and the form first.
pub const LOCK: &str = "travar";
/// Listen to notices (channel): the Tauri channel through which the Rust side tells the
/// screen about an unlock request (Phase 31, D65 a). The notice carries only its name,
/// no data.
pub const LISTEN_NOTICES: &str = "ouvir_avisos";
/// Approval request: the current request for the approval window (Phase 36), with the
/// agent's text already neutralized (D51), or `null`. Only the approval window.
pub const APPROVAL_REQUEST: &str = "pedido_de_aprovacao";
/// Answer an approval (id, decision, PIN, acknowledged): Deny (no PIN) or one of the
/// three Allow (with PIN, S9, S10). Only the approval window; the response does not
/// carry the PIN. The acknowledgement (Phase 54b, D68) is the box of the hidden default
/// entry, required in an Allow of a request with a hidden entry and refused in the
/// others.
pub const ANSWER_APPROVAL: &str = "responder_aprovacao";

/// Settings: lock timeout and external backup on or off (Phase 38). No argument; no PIN
/// hash nor UUID.
pub const SETTINGS: &str = "configuracoes";
/// Change the PIN (data: current, new and confirmation): changes the PIN asking for the
/// current one (Phase 38, V5.22, S9). The response carries "changed" or the errors per
/// field, never the value.
pub const CHANGE_PIN: &str = "trocar_pin";
/// Change the master password (data: current, new and confirmation): changes the master
/// password asking for the current one (Phase 38, V5.22). The response carries "changed"
/// or the errors per field.
pub const CHANGE_MASTER_PASSWORD: &str = "trocar_senha_mestra";
/// Set the lock timeout (minutes): inactivity lock timeout, stored in the vault config
/// (Phase 38, D30, D45).
pub const SET_LOCK_TIMEOUT: &str = "definir_tempo_trava";
/// External backup: on or off, the chosen UUID and, if connected, the USB drive (label
/// neutralized, D51). Never the mount path (Phase 48, V7.9).
pub const EXTERNAL_BACKUP: &str = "backup_externo";
/// List USB drives: the USB drives the daemon sees, to choose from (Phase 48).
pub const LIST_USB_DRIVES: &str = "listar_pendrives";
/// Set the external backup (on, optional UUID): turn on with a USB drive the daemon sees
/// now (the Rust side checks again) or turn off keeping the UUID (Phase 48, D17, D30).
pub const SET_EXTERNAL_BACKUP: &str = "definir_backup_externo";
/// List backups: the versions of the local backup (name and instant, never content),
/// from the newest to the oldest (Phase 47, V7.7).
pub const LIST_BACKUPS: &str = "listar_backups";
/// Restore a backup (data: version, password and PIN): restores a version with its
/// master password and the current PIN (Phase 47, V7.7; D17, D39, S9). The response
/// carries only "restored" and the entry count.
pub const RESTORE_BACKUP: &str = "restaurar_backup";

/// Commands of the main window, in the order of its capability.
pub const MAIN_COMMANDS: &[&str] = &[
    VAULT_STATUS,
    COPY_FIELD,
    LIST_CREDENTIALS,
    CREDENTIAL_DETAIL,
    GENERATE_PASSWORD,
    COPY_GENERATED_PASSWORD,
    REVEAL_GENERATED_PASSWORD,
    DISCARD_GENERATED_PASSWORD,
    REVEAL_FIELD,
    TOTP_CODE,
    COPY_TOTP_CODE,
    USE_GENERATED_PASSWORD,
    DISCARD_FORM,
    SAVE_CREDENTIAL,
    SET_LEVEL,
    CHOOSE_ATTACHMENT,
    REMOVE_ATTACHMENT,
    LOAD_KEY,
    CONNECT,
    CREATE_VAULT,
    UNLOCK,
    LISTEN_NOTICES,
    SETTINGS,
    CHANGE_PIN,
    CHANGE_MASTER_PASSWORD,
    SET_LOCK_TIMEOUT,
    EXTERNAL_BACKUP,
    LIST_USB_DRIVES,
    SET_EXTERNAL_BACKUP,
    LIST_BACKUPS,
    RESTORE_BACKUP,
    LOCK,
];

/// Commands of the approval window, in the order of its capability (Phase 36).
/// [`LISTEN_NOTICES`] is the only one in both windows: the channel is per window and the
/// notice carries no data (the main window receives the unlock request notice; the
/// approval window, the approval request one).
pub const APPROVAL_COMMANDS: &[&str] = &[LISTEN_NOTICES, APPROVAL_REQUEST, ANSWER_APPROVAL];

/// The commands both windows have (only the notice channel, no data).
pub const SHARED_COMMANDS: &[&str] = &[LISTEN_NOTICES];

/// All commands (app manifest and test bridge): those of the main window and those of
/// the approval window.
pub const COMMANDS: &[&str] = &[
    VAULT_STATUS,
    COPY_FIELD,
    LIST_CREDENTIALS,
    CREDENTIAL_DETAIL,
    GENERATE_PASSWORD,
    COPY_GENERATED_PASSWORD,
    REVEAL_GENERATED_PASSWORD,
    DISCARD_GENERATED_PASSWORD,
    REVEAL_FIELD,
    TOTP_CODE,
    COPY_TOTP_CODE,
    USE_GENERATED_PASSWORD,
    DISCARD_FORM,
    SAVE_CREDENTIAL,
    SET_LEVEL,
    CHOOSE_ATTACHMENT,
    REMOVE_ATTACHMENT,
    LOAD_KEY,
    CONNECT,
    CREATE_VAULT,
    UNLOCK,
    LISTEN_NOTICES,
    SETTINGS,
    CHANGE_PIN,
    CHANGE_MASTER_PASSWORD,
    SET_LOCK_TIMEOUT,
    EXTERNAL_BACKUP,
    LIST_USB_DRIVES,
    SET_EXTERNAL_BACKUP,
    LIST_BACKUPS,
    RESTORE_BACKUP,
    APPROVAL_REQUEST,
    ANSWER_APPROVAL,
    LOCK,
];

/// Name of the permission Tauri generates for the command (`allow-<command-with-hyphens>`).
pub fn permission(command: &str) -> String {
    format!("allow-{}", command.replace('_', "-"))
}

/// Known command, from the name that came from the interface (untrusted).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    VaultStatus,
    CopyField,
    ListCredentials,
    CredentialDetail,
    GeneratePassword,
    CopyGeneratedPassword,
    RevealGeneratedPassword,
    DiscardGeneratedPassword,
    RevealField,
    TotpCode,
    CopyTotpCode,
    UseGeneratedPassword,
    DiscardForm,
    SaveCredential,
    SetLevel,
    ChooseAttachment,
    RemoveAttachment,
    LoadKey,
    Connect,
    CreateVault,
    Unlock,
    ListenNotices,
    Settings,
    ChangePin,
    ChangeMasterPassword,
    SetLockTimeout,
    ExternalBackup,
    ListUsbDrives,
    SetExternalBackup,
    ListBackups,
    RestoreBackup,
    ApprovalRequest,
    AnswerApproval,
    Lock,
}

impl Command {
    pub fn from_name(name: &str) -> Option<Command> {
        match name {
            VAULT_STATUS => Some(Command::VaultStatus),
            COPY_FIELD => Some(Command::CopyField),
            LIST_CREDENTIALS => Some(Command::ListCredentials),
            CREDENTIAL_DETAIL => Some(Command::CredentialDetail),
            GENERATE_PASSWORD => Some(Command::GeneratePassword),
            COPY_GENERATED_PASSWORD => Some(Command::CopyGeneratedPassword),
            REVEAL_GENERATED_PASSWORD => Some(Command::RevealGeneratedPassword),
            DISCARD_GENERATED_PASSWORD => Some(Command::DiscardGeneratedPassword),
            REVEAL_FIELD => Some(Command::RevealField),
            TOTP_CODE => Some(Command::TotpCode),
            COPY_TOTP_CODE => Some(Command::CopyTotpCode),
            USE_GENERATED_PASSWORD => Some(Command::UseGeneratedPassword),
            DISCARD_FORM => Some(Command::DiscardForm),
            SAVE_CREDENTIAL => Some(Command::SaveCredential),
            SET_LEVEL => Some(Command::SetLevel),
            CHOOSE_ATTACHMENT => Some(Command::ChooseAttachment),
            REMOVE_ATTACHMENT => Some(Command::RemoveAttachment),
            LOAD_KEY => Some(Command::LoadKey),
            CONNECT => Some(Command::Connect),
            CREATE_VAULT => Some(Command::CreateVault),
            UNLOCK => Some(Command::Unlock),
            LISTEN_NOTICES => Some(Command::ListenNotices),
            SETTINGS => Some(Command::Settings),
            CHANGE_PIN => Some(Command::ChangePin),
            CHANGE_MASTER_PASSWORD => Some(Command::ChangeMasterPassword),
            SET_LOCK_TIMEOUT => Some(Command::SetLockTimeout),
            EXTERNAL_BACKUP => Some(Command::ExternalBackup),
            LIST_USB_DRIVES => Some(Command::ListUsbDrives),
            SET_EXTERNAL_BACKUP => Some(Command::SetExternalBackup),
            LIST_BACKUPS => Some(Command::ListBackups),
            RESTORE_BACKUP => Some(Command::RestoreBackup),
            APPROVAL_REQUEST => Some(Command::ApprovalRequest),
            ANSWER_APPROVAL => Some(Command::AnswerApproval),
            LOCK => Some(Command::Lock),
            _ => None,
        }
    }
}

/// Which field of which entry. Validated before reaching the daemon: entry name up to
/// `MAX_NAME`, field name up to `MAX_EXTRA_NAME`, no control and no space at the ends.
/// The daemon validates again (the input is not trusted there either).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    entry: String,
    field: String,
}

impl Reference {
    pub fn new(entry: &str, field: &str) -> Result<Reference, AppError> {
        if !valid_name(entry, limits::MAX_NAME) || !valid_name(field, limits::MAX_EXTRA_NAME) {
            return Err(AppError::InvalidRequest);
        }
        Ok(Reference {
            entry: entry.to_owned(),
            field: field.to_owned(),
        })
    }

    pub fn entry(&self) -> &str {
        &self.entry
    }

    pub fn field(&self) -> &str {
        &self.field
    }
}

/// Entry name coming from the interface (untrusted), with the same rule as the
/// [`Reference`].
pub fn validate_entry_name(name: &str) -> Result<(), AppError> {
    if valid_name(name, limits::MAX_NAME) {
        Ok(())
    } else {
        Err(AppError::InvalidRequest)
    }
}

fn valid_name(name: &str, max: usize) -> bool {
    !name.is_empty()
        && name.chars().count() <= max
        && name.trim() == name
        && !name.chars().any(|c| c.is_control() || matches!(c, '\u{200B}'..='\u{200F}' | '\u{2028}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_and_permissions() {
        assert_eq!(
            COMMANDS,
            &[
                "estado_do_cofre",
                "copiar_campo",
                "listar_credenciais",
                "detalhe_credencial",
                "gerar_senha",
                "copiar_senha_gerada",
                "revelar_senha_gerada",
                "descartar_senha_gerada",
                "revelar_campo",
                "codigo_totp",
                "copiar_codigo_totp",
                "usar_senha_gerada",
                "descartar_formulario",
                "salvar_credencial",
                "mudar_nivel",
                "escolher_anexo",
                "remover_anexo",
                "carregar_chave",
                "conectar",
                "criar_cofre",
                "destravar",
                "ouvir_avisos",
                "configuracoes",
                "trocar_pin",
                "trocar_senha_mestra",
                "definir_tempo_trava",
                "backup_externo",
                "listar_pendrives",
                "definir_backup_externo",
                "listar_backups",
                "restaurar_backup",
                "pedido_de_aprovacao",
                "responder_aprovacao",
                "travar",
            ]
        );
        // Both windows together are all of them; in common, only the notice channel.
        for c in COMMANDS {
            assert!(
                MAIN_COMMANDS.contains(c) || APPROVAL_COMMANDS.contains(c),
                "{c}"
            );
            assert_eq!(
                MAIN_COMMANDS.contains(c) && APPROVAL_COMMANDS.contains(c),
                SHARED_COMMANDS.contains(c),
                "{c}"
            );
        }
        assert_eq!(
            APPROVAL_COMMANDS,
            &["ouvir_avisos", "pedido_de_aprovacao", "responder_aprovacao"]
        );
        assert_eq!(permission(COPY_FIELD), "allow-copiar-campo");
        for c in COMMANDS {
            assert!(Command::from_name(c).is_some());
        }
    }

    #[test]
    fn attack_invented_or_plugin_command_does_not_exist() {
        for name in [
            "",
            "plugin:fs|read_text_file",
            "plugin:shell|execute",
            "plugin:http|fetch",
            "revelar_tudo",
            "COPIAR_CAMPO",
            "copiar_campo ",
            "copiar-campo",
            "revelar",
            "detalhe",
            "lista",
            "detalhe_credencial ",
        ] {
            assert_eq!(Command::from_name(name), None, "{name}");
        }
    }

    #[test]
    fn attack_reference_out_of_format() {
        let long = "a".repeat(limits::MAX_NAME + 1);
        let long_field = "a".repeat(limits::MAX_EXTRA_NAME + 1);
        for (e, c) in [
            ("", "password"),
            ("acme-pg-dev", ""),
            (long.as_str(), "password"),
            ("acme-pg-dev", long_field.as_str()),
            (" acme-pg-dev", "password"),
            ("acme\npg", "password"),
            ("acme-pg-dev", "se\u{0}nha"),
            ("acme\u{202E}gp", "password"),
            ("acme-pg-dev", "senha\u{200B}"),
        ] {
            assert_eq!(
                Reference::new(e, c),
                Err(AppError::InvalidRequest),
                "{e:?} {c:?}"
            );
        }
        let r = Reference::new("acme-pg-dev", "password").unwrap();
        assert_eq!((r.entry(), r.field()), ("acme-pg-dev", "password"));
        assert!(Reference::new(&"á".repeat(limits::MAX_NAME), "password").is_ok());
    }
}
