//! Error responses of the protocol. Each code has a fixed message in Portuguese; no error
//! carries a user value, an attempt count or the vault generation (S13, D20, D39). When
//! reading, the message that came in the frame is ignored and the fixed one of the code
//! comes back: not even a fake server manages to put its own text on the client screen
//! this way.

use core::fmt;

#[cfg(unix)]
use crate::atomic_write::WriteError;
use crate::body::OpenError;
use crate::entry::ValidationErrors;

/// A validation problem of the entry (field, position in the list and reason), copied
/// from [`crate::entry::ValidationError`], which only has fixed texts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub field: String,
    pub position: Option<u64>,
    pub reason: String,
}

/// Error returned to the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseError {
    /// The operation is not of the client's role (D5), or the request of another role.
    Unauthorized,
    /// The vault is locked.
    Locked,
    /// The request waited for the unlock and the time ran out (D16).
    LockedTimedOut,
    /// The frame is not a valid request (JSON, extra field, size, format).
    InvalidRequest,
    /// The operation does not exist.
    UnknownOperation,
    /// Protocol version different from the one of this build.
    UnknownVersion,
    /// The frame went over the role's limit or came empty (V3.4).
    InvalidFrame,
    /// The daemon is serving too many connections.
    Busy,
    /// Wrong password or tampered file (the format does not tell them apart, on purpose).
    WrongPassword,
    /// Decrypted, but the content is not a valid vault.
    InvalidVault,
    /// There is no vault yet (first use).
    VaultNotFound,
    /// A vault already exists.
    VaultAlreadyExists,
    /// D39: the vault on disk has a generation lower than the highest one already seen.
    Rollback,
    /// D39: the vault file changed outside Ward (dev, inode, `mtime` or size).
    VaultChanged,
    /// D39: the generation reached `u64::MAX`; nothing was written.
    GenerationExhausted,
    /// Failure writing the vault (the previous file stays).
    WriteFailed,
    /// Wrong PIN.
    WrongPin,
    /// Entry that does not exist or that the role cannot see: for the agent, a blocked
    /// one answers the same as a missing one (V3.17).
    EntryNotFound,
    /// The requested field does not exist in the entry.
    FieldNotFound,
    /// The approval was denied (or the window closed).
    Denied,
    /// The approval expired in 2 minutes (S10).
    Expired,
    /// There is no way to ask for approval now (D21).
    ApprovalUnavailable,
    /// The origin of the tab does not match the one of the entry (D9).
    OriginMismatch,
    /// `ward playwright` (Phase 44): the entry page has no login form that can be filled
    /// safely, or the site did not accept the login.
    LoginRefused,
    /// `ward playwright` (Phase 44): Chrome did not open, crashed or did not answer in
    /// time.
    BrowserFailed,
    /// `ward playwright` (Phase 44, S1): the site session kept the password (or a
    /// variation of it) in a cookie or `localStorage`; nothing was written.
    SessionHasSecret,
    /// The entry did not pass the validation.
    Validation(Vec<Problem>),
    /// `ward run` of a type without delivery (secure note, D10): nothing is run.
    TypeWithoutDelivery,
    /// The credential delivery could not be built (temporary file, TOTP code, tunnel):
    /// nothing is run.
    DeliveryFailed,
    /// The requested backup version does not exist in the folder (Phase 45).
    BackupNotFound,
    /// The backup folder is not accessible or is not a 0700 directory of the user, or
    /// the current vault could not be kept before restoring (Phase 45).
    BackupUnavailable,
    /// udisks2 did not answer: the USB drives cannot be seen (Phase 46).
    UsbDrivesUnavailable,
    /// Internal error of the daemon (without detail).
    Internal,
}

/// Fixed messages that other parts check (D39).
pub const MSG_ROLLBACK: &str = "o cofre no disco é mais antigo que o último aberto";
/// Fixed message of [`ResponseError::VaultChanged`] (D39).
pub const MSG_VAULT_CHANGED: &str = "o arquivo do cofre mudou fora do Ward";
/// Fixed message of [`ResponseError::GenerationExhausted`] (the same as the one of
/// [`crate::atomic_write::WriteError::GenerationExhausted`]).
pub const MSG_GENERATION_EXHAUSTED: &str =
    "recusado gravar: o contador de versões do cofre chegou ao fim";

impl ResponseError {
    /// All the codes without data (the validation one stays out).
    pub const SIMPLE: [ResponseError; 32] = [
        Self::Unauthorized,
        Self::Locked,
        Self::LockedTimedOut,
        Self::InvalidRequest,
        Self::UnknownOperation,
        Self::UnknownVersion,
        Self::InvalidFrame,
        Self::Busy,
        Self::WrongPassword,
        Self::InvalidVault,
        Self::VaultNotFound,
        Self::VaultAlreadyExists,
        Self::Rollback,
        Self::VaultChanged,
        Self::GenerationExhausted,
        Self::WriteFailed,
        Self::WrongPin,
        Self::EntryNotFound,
        Self::FieldNotFound,
        Self::Denied,
        Self::Expired,
        Self::ApprovalUnavailable,
        Self::OriginMismatch,
        Self::LoginRefused,
        Self::BrowserFailed,
        Self::SessionHasSecret,
        Self::TypeWithoutDelivery,
        Self::DeliveryFailed,
        Self::BackupNotFound,
        Self::BackupUnavailable,
        Self::UsbDrivesUnavailable,
        Self::Internal,
    ];

    /// Stable code of the frame.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unauthorized => "nao_autorizado",
            Self::Locked => "travado",
            Self::LockedTimedOut => "travado_tempo_esgotado",
            Self::InvalidRequest => "pedido_invalido",
            Self::UnknownOperation => "operacao_desconhecida",
            Self::UnknownVersion => "versao_desconhecida",
            Self::InvalidFrame => "quadro_invalido",
            Self::Busy => "ocupado",
            Self::WrongPassword => "senha_incorreta",
            Self::InvalidVault => "cofre_invalido",
            Self::VaultNotFound => "cofre_inexistente",
            Self::VaultAlreadyExists => "cofre_ja_existe",
            Self::Rollback => "retrocesso",
            Self::VaultChanged => "cofre_mudou",
            Self::GenerationExhausted => "geracao_esgotada",
            Self::WriteFailed => "falha_ao_gravar",
            Self::WrongPin => "pin_incorreto",
            Self::EntryNotFound => "entrada_inexistente",
            Self::FieldNotFound => "campo_inexistente",
            Self::Denied => "negado",
            Self::Expired => "expirado",
            Self::ApprovalUnavailable => "aprovacao_indisponivel",
            Self::OriginMismatch => "origem_diferente",
            Self::LoginRefused => "login_recusado",
            Self::BrowserFailed => "navegador_falhou",
            Self::SessionHasSecret => "sessao_com_segredo",
            Self::Validation(_) => "validacao",
            Self::TypeWithoutDelivery => "tipo_sem_entrega",
            Self::DeliveryFailed => "entrega_falhou",
            Self::BackupNotFound => "backup_inexistente",
            Self::BackupUnavailable => "backup_indisponivel",
            Self::UsbDrivesUnavailable => "pendrives_indisponiveis",
            Self::Internal => "interno",
        }
    }

    /// Fixed message, in Portuguese, without any value.
    pub fn message(&self) -> &'static str {
        match self {
            Self::Unauthorized => "não autorizado",
            Self::Locked => "travado",
            Self::LockedTimedOut => "travado: tempo esgotado",
            Self::InvalidRequest => "pedido inválido",
            Self::UnknownOperation => "operação desconhecida",
            Self::UnknownVersion => "versão do protocolo desconhecida",
            Self::InvalidFrame => "quadro vazio ou grande demais",
            Self::Busy => "o Ward está ocupado; tente de novo",
            Self::WrongPassword => "senha mestra incorreta ou arquivo do cofre adulterado",
            Self::InvalidVault => "o conteúdo do cofre é inválido",
            Self::VaultNotFound => "ainda não existe cofre",
            Self::VaultAlreadyExists => "o cofre já existe",
            Self::Rollback => MSG_ROLLBACK,
            Self::VaultChanged => MSG_VAULT_CHANGED,
            Self::GenerationExhausted => MSG_GENERATION_EXHAUSTED,
            Self::WriteFailed => "falha ao gravar o cofre; a versão anterior foi mantida",
            Self::WrongPin => "PIN incorreto",
            Self::EntryNotFound => "entrada inexistente",
            Self::FieldNotFound => "campo inexistente",
            Self::Denied => "pedido negado",
            Self::Expired => "pedido expirado",
            Self::ApprovalUnavailable => "aprovação indisponível",
            Self::OriginMismatch => "a origem da página não bate com a da entrada",
            Self::LoginRefused => {
                "a página não tem um formulário de login seguro ou o site não aceitou o login"
            }
            Self::BrowserFailed => "o navegador não abriu ou não respondeu a tempo",
            Self::SessionHasSecret => "a sessão do site guarda a senha; nada foi gravado",
            Self::Validation(_) => "entrada inválida",
            Self::TypeWithoutDelivery => "tipo sem entrega",
            Self::DeliveryFailed => "a entrega da credencial falhou; nada foi executado",
            Self::BackupNotFound => "a versão do backup não existe",
            Self::BackupUnavailable => "a pasta de backups não está disponível ou não é segura",
            Self::UsbDrivesUnavailable => "não deu para ver os pendrives (o udisks não respondeu)",
            Self::Internal => "erro interno do Ward",
        }
    }

    /// The error of a code read from the frame. The validation one comes back without
    /// problems; the reader fills them in.
    pub(crate) fn from_code(code: &str) -> Option<Self> {
        if code == "validacao" {
            return Some(Self::Validation(Vec::new()));
        }
        Self::SIMPLE.into_iter().find(|e| e.code() == code)
    }

    /// Validation error from the problems of the entry (fixed texts only).
    pub fn from_validation(errors: &ValidationErrors) -> Self {
        Self::Validation(
            errors
                .list()
                .iter()
                .map(|e| Problem {
                    field: e.field.to_owned(),
                    position: e.position.map(|p| p as u64),
                    reason: e.reason.to_owned(),
                })
                .collect(),
        )
    }
}

impl fmt::Display for ResponseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for ResponseError {}

/// Error of the unlock (D39): `Rollback` becomes its own notice, without the generations.
impl From<&OpenError> for ResponseError {
    fn from(e: &OpenError) -> Self {
        match e {
            OpenError::Format(_) => Self::WrongPassword,
            OpenError::Body(_) => Self::InvalidVault,
            OpenError::Rollback => Self::Rollback,
        }
    }
}

/// Error of the writes (D39): `GenerationExhausted` and `Rollback` (password change) with
/// their own messages; the rest, without detail. In the password change, `Format` is also
/// the wrong current password: the daemon, which knows which operation it did, answers
/// [`ResponseError::WrongPassword`] in that case.
#[cfg(unix)]
impl From<&WriteError> for ResponseError {
    fn from(e: &WriteError) -> Self {
        match e {
            WriteError::GenerationExhausted => Self::GenerationExhausted,
            WriteError::Rollback => Self::Rollback,
            WriteError::Format(_)
            | WriteError::NotVault
            | WriteError::Body(_)
            | WriteError::InvalidDestination
            | WriteError::Io { .. } => Self::WriteFailed,
        }
    }
}
