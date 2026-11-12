//! Errors the interface receives. Only fixed messages in Portuguese: no value of the
//! entry, no path and no system error text (S13).

use std::fmt;

/// Error of an app command. `Display` gives the message the screen shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppError {
    /// The daemon socket does not exist or did not accept the connection.
    DaemonUnavailable,
    /// The daemon directory or socket did not pass the check (owner, permission,
    /// type). The app does not talk to it.
    DaemonUntrusted,
    /// The connection opened at startup (D47) no longer exists; the app executes itself
    /// again.
    ConnectionLost,
    /// The interface request did not pass validation (size, format).
    InvalidRequest,
    /// The vault is locked.
    Locked,
    /// The clipboard did not accept the value.
    Clipboard,
    /// The entry does not exist (or the daemon does not show it to this role).
    EntryNotFound,
    /// The requested field does not exist in the entry.
    FieldNotFound,
    /// The daemon answered "unauthorized": the connection does not have the app role
    /// (D5, D47).
    Unauthorized,
    /// The daemon refused the request for another reason (the protocol code does not
    /// reach the interface).
    Refused,
    /// The daemon response is not the expected one for the request (type, format, role).
    InvalidResponse,
    /// The system RNG did not answer; the generator uses no fallback source.
    RandomUnavailable,
    /// "Copy" or "reveal" of the standalone generator without a generated password (or
    /// one already discarded).
    NoGeneratedPassword,
    /// The PIN does not match (S9). Nothing was changed; 5 errors in a row lock the vault
    /// (the daemon counts; the response does not say how many are left).
    WrongPin,
    /// There is no way to choose a file in this mode (no dialog, no test folder).
    PickerUnavailable,
    /// The "Connect" terminal did not open.
    TerminalUnavailable,
    /// "Connect" on an entry that is not a server.
    NotAServer,
    /// First use: the daemon already has a vault (Phase 30).
    VaultAlreadyExists,
    /// First use: the daemon could not write the new vault.
    VaultNotWritten,
    /// Unlock (Phase 31): wrong master password (or tampered file; the format does not
    /// tell them apart). The response does not say how many attempts there were (D20).
    WrongPassword,
    /// Unlock (D39): the vault on disk is older than the last one opened; nothing was
    /// opened.
    VaultRollback,
    /// External backup (Phase 48): udisks2 did not answer; the USB drives cannot be seen.
    UsbDrivesUnavailable,
    /// External backup (Phase 48): the requested UUID is not of a USB drive connected now
    /// (or there are two with the same UUID). Nothing was written.
    UsbDriveNotConnected,
    /// Restore (Phase 47): the version is not (any longer) in the backup folder. Nothing
    /// changed.
    BackupNotFound,
    /// Restore (Phase 47): the backup folder does not open or is not safe (0700 of the
    /// user, no link). Nothing changed.
    BackupUnavailable,
    /// Restore (D39): the vault file changed outside Ward; the daemon locked the vault.
    VaultChanged,
    /// The daemon speaks another protocol version (m2:DT8): the app stops before
    /// sending any password (m2:V4.16).
    IncompatibleVersion,
}

impl AppError {
    /// Stable code, for the interface to choose the screen without comparing text.
    pub fn code(self) -> &'static str {
        match self {
            AppError::DaemonUnavailable => "daemon-unavailable",
            AppError::DaemonUntrusted => "daemon-untrusted",
            AppError::ConnectionLost => "connection-lost",
            AppError::InvalidRequest => "invalid-request",
            AppError::Locked => "locked",
            AppError::Clipboard => "clipboard",
            AppError::EntryNotFound => "entry-not-found",
            AppError::FieldNotFound => "field-not-found",
            AppError::Unauthorized => "unauthorized",
            AppError::Refused => "refused",
            AppError::InvalidResponse => "invalid-response",
            AppError::RandomUnavailable => "random-unavailable",
            AppError::NoGeneratedPassword => "no-generated-password",
            AppError::WrongPin => "wrong-pin",
            AppError::PickerUnavailable => "picker-unavailable",
            AppError::TerminalUnavailable => "terminal-unavailable",
            AppError::NotAServer => "not-a-server",
            AppError::VaultAlreadyExists => "vault-already-exists",
            AppError::VaultNotWritten => "vault-not-written",
            AppError::WrongPassword => "wrong-password",
            AppError::VaultRollback => "vault-rollback",
            AppError::UsbDrivesUnavailable => "usb-drives-unavailable",
            AppError::UsbDriveNotConnected => "usb-drive-not-connected",
            AppError::BackupNotFound => "backup-not-found",
            AppError::BackupUnavailable => "backup-unavailable",
            AppError::VaultChanged => "vault-changed",
            AppError::IncompatibleVersion => "incompatible-version",
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            AppError::DaemonUnavailable => "o daemon do Ward não está rodando",
            AppError::DaemonUntrusted => "o socket do daemon não é confiável; o app não se conecta",
            AppError::ConnectionLost => "a conexão com o daemon caiu; o app vai se reabrir",
            AppError::InvalidRequest => "pedido inválido",
            AppError::Locked => "o cofre está travado",
            AppError::Clipboard => "não foi possível usar a área de transferência",
            AppError::EntryNotFound => "essa entrada não existe mais",
            AppError::FieldNotFound => "esse campo não existe na entrada",
            AppError::Unauthorized => "o daemon não reconheceu este app",
            AppError::Refused => "o daemon recusou o pedido",
            AppError::InvalidResponse => "resposta inválida do daemon",
            AppError::RandomUnavailable => "o gerador aleatório do sistema não respondeu",
            AppError::NoGeneratedPassword => "gere uma senha antes",
            AppError::WrongPin => "PIN incorreto. Nada foi alterado.",
            AppError::PickerUnavailable => "não foi possível abrir a escolha de arquivo",
            AppError::TerminalUnavailable => "não foi possível abrir o terminal",
            AppError::NotAServer => "Conectar só vale para entradas servidor",
            AppError::WrongPassword => "Senha mestra incorreta.",
            AppError::VaultRollback => {
                "o cofre no disco é mais antigo que o último aberto; nada foi aberto"
            }
            AppError::VaultAlreadyExists => "já existe um cofre neste computador",
            AppError::UsbDrivesUnavailable => {
                "não deu para ver os pendrives (o udisks não respondeu)"
            }
            AppError::UsbDriveNotConnected => {
                "esse pendrive não está conectado; conecte-o e atualize a lista"
            }
            AppError::BackupNotFound => {
                "essa versão não está mais na pasta de backups; nada mudou no cofre atual"
            }
            AppError::BackupUnavailable => {
                "a pasta de backups não está disponível ou não é segura; nada mudou no cofre atual"
            }
            AppError::VaultChanged => {
                "o arquivo do cofre mudou fora do Ward; o cofre foi travado e nada foi restaurado"
            }
            AppError::VaultNotWritten => {
                "não foi possível gravar o cofre; confira o espaço e as permissões da pasta do Ward"
            }
            AppError::IncompatibleVersion => {
                "versão do protocolo incompatível com o daemon: atualize o Ward"
            }
        })
    }
}

impl std::error::Error for AppError {}
