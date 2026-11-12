//! Vault configuration (D30): the section the v2 body writes before the entries, inside
//! the encrypted part.
//!
//! - **PIN hash** ([`PinHash`]): Argon2id as PHC text (`$argon2id$v=19$m=…`), with
//!   parameters and salt, computed by `crypto::argon2id` (working memory zeroed). The
//!   PIN is never written in the clear, not even inside the body. [`PinHash::verify`]
//!   compares in constant time through the library; the daemon (Phase 22) only calls it.
//! - **Lock timeout** for inactivity, in minutes: default 240 (R23, D16), from
//!   [`MIN_LOCK_MINUTES`] to [`MAX_LOCK_MINUTES`].
//! - **External backup** ([`ExternalBackup`]): disabled by default, with the optional
//!   UUID of the USB drive (D17). Enabled requires the UUID.
//!
//! A vault without a PIN hash does not exist: [`VaultConfig::new`] requires one, and
//! reading the body refuses the section without it.

use core::fmt;

use zeroize::{ZeroizeOnDrop, Zeroizing};

use crate::crypto;
use crate::format::KdfParams;
use crate::limits::{
    DEFAULT_LOCK_MINUTES, MAX_BACKUP_UUID, MAX_LOCK_MINUTES, MAX_PIN_DIGITS, MAX_PIN_HASH,
    MIN_LOCK_MINUTES, MIN_PIN_DIGITS,
};

#[cfg(test)]
mod tests;

/// Argon2id of the PIN for a new hash: 64 MiB, 3 passes, 1 lane. The daemon verifies it
/// on every approval, so the cost stays below the master password's (which is 256 MiB).
/// The hash only exists inside the encrypted part of the vault.
pub const KDF_PIN: KdfParams = KdfParams {
    memory_kib: 64 * 1024,
    passes: 3,
    parallelism: 1,
};
/// Lowest cost accepted when reading the PIN hash (the same as [`KDF_PIN`]).
pub const KDF_PIN_MINIMUM: KdfParams = KDF_PIN;
/// Highest cost accepted when reading: a hash with an inflated cost is refused without
/// running Argon2 (or allocating its memory).
pub const KDF_PIN_MAXIMUM: KdfParams = KdfParams {
    memory_kib: 256 * 1024,
    passes: 16,
    parallelism: 4,
};

/// Configuration error. No variant carries the PIN, the hash or any other value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    /// The PIN does not have 4 to 6 digits.
    InvalidPin,
    /// Failure computing the PIN hash (system randomness or memory).
    Hash,
    /// Lock timeout outside the accepted range.
    LockTimeout,
    /// USB drive UUID empty, too long or with a character outside `[0-9A-Fa-f-]`.
    InvalidUuid,
    /// External backup enabled without the USB drive UUID.
    BackupWithoutUuid,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidPin => "o PIN tem de ter de 4 a 6 dígitos",
            Self::Hash => "falha ao calcular o hash do PIN",
            Self::LockTimeout => "o tempo de trava vai de 1 a 1440 minutos",
            Self::InvalidUuid => {
                "UUID do pendrive inválido (até 36 caracteres, só hexadecimal e hífen)"
            }
            Self::BackupWithoutUuid => "o backup externo ligado precisa do UUID do pendrive",
        })
    }
}

impl std::error::Error for ConfigError {}

/// PIN in the accepted format: [`MIN_PIN_DIGITS`] to [`MAX_PIN_DIGITS`] ASCII digits.
fn valid_pin(pin: &[u8]) -> bool {
    (MIN_PIN_DIGITS..=MAX_PIN_DIGITS).contains(&pin.len()) && pin.iter().all(u8::is_ascii_digit)
}

/// Argon2id hash of the PIN as PHC text. Zeroed on drop; no `Clone`, no `Display`, and
/// `Debug` shows `••••`. The text is treated as a secret: a 6-digit PIN falls out of the
/// hash by brute force, so it only exists inside the encrypted part of the vault.
pub struct PinHash {
    phc: Zeroizing<String>,
}

// The only field is a `Zeroizing` (V2.28).
impl ZeroizeOnDrop for PinHash {}

impl fmt::Debug for PinHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PinHash(••••)")
    }
}

impl PinHash {
    /// New hash of the PIN (4 to 6 digits), with a random salt and [`KDF_PIN`].
    pub fn create(pin: &[u8]) -> Result<Self, ConfigError> {
        if !valid_pin(pin) {
            return Err(ConfigError::InvalidPin);
        }
        let phc = crypto::hash_pin(pin, &KDF_PIN).map_err(|_| ConfigError::Hash)?;
        Ok(Self { phc })
    }

    /// Verifies the PIN against the hash, in constant time through the library. A PIN
    /// outside the format gives `false` without running Argon2 (the format is not secret).
    pub fn verify(&self, pin: &[u8]) -> bool {
        if !valid_pin(pin) {
            return false;
        }
        match read(self.phc.as_bytes()) {
            Some(phc) => crypto::verify_pin(pin, &phc),
            None => false,
        }
    }

    /// Argon2id parameters of this hash (not secret).
    pub fn params(&self) -> KdfParams {
        // The text was verified at creation; `KDF_PIN` only if something very wrong happens.
        read(self.phc.as_bytes()).map_or(KDF_PIN, |p| p.cost)
    }

    /// Hash read from the body: verifies the whole text (see [`verify_text`]) and copies it
    /// into an allocation of the exact size, which is zeroed on drop.
    pub(crate) fn from_phc(text: &[u8]) -> Option<Self> {
        if !verify_text(text) {
            return None;
        }
        let text = core::str::from_utf8(text).ok()?;
        let mut phc: Zeroizing<String> = Zeroizing::new(String::new());
        phc.try_reserve_exact(text.len()).ok()?;
        phc.push_str(text);
        Some(Self { phc })
    }

    /// Bytes of the PHC text, for the body to write.
    pub(crate) fn bytes(&self) -> &[u8] {
        self.phc.as_bytes()
    }
}

fn read(text: &[u8]) -> Option<crypto::PinPhc> {
    crypto::read_pin_phc(text, &KDF_PIN_MINIMUM, &KDF_PIN_MAXIMUM)
}

/// Verifies the PHC text of the PIN hash without allocating (the body's 1st pass uses this).
pub(crate) fn verify_text(text: &[u8]) -> bool {
    text.len() <= MAX_PIN_HASH && read(text).is_some()
}

/// Lock timeout within the accepted range.
pub(crate) fn valid_minutes(minutes: u32) -> bool {
    (MIN_LOCK_MINUTES..=MAX_LOCK_MINUTES).contains(&minutes)
}

/// USB drive UUID: 1 to [`MAX_BACKUP_UUID`] characters `[0-9A-Fa-f-]`, without `-` at the
/// start or at the end. Not secret, but it comes from the disk and from the interface.
pub(crate) fn valid_uuid(uuid: &[u8]) -> bool {
    (1..=MAX_BACKUP_UUID).contains(&uuid.len())
        && uuid.iter().all(|b| b.is_ascii_hexdigit() || *b == b'-')
        && uuid.first() != Some(&b'-')
        && uuid.last() != Some(&b'-')
}

/// External backup (D17): enabled or not, and the filesystem UUID of the USB drive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalBackup {
    enabled: bool,
    uuid: Option<String>,
}

impl ExternalBackup {
    /// Disabled and without a USB drive (the default).
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            uuid: None,
        }
    }

    /// Backup with the given UUID (optional if disabled; required if enabled).
    pub fn new(enabled: bool, uuid: Option<&str>) -> Result<Self, ConfigError> {
        if let Some(u) = uuid
            && !valid_uuid(u.as_bytes())
        {
            return Err(ConfigError::InvalidUuid);
        }
        if enabled && uuid.is_none() {
            return Err(ConfigError::BackupWithoutUuid);
        }
        Ok(Self {
            enabled,
            uuid: uuid.map(str::to_owned),
        })
    }

    /// Whether the external backup is enabled.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// USB drive UUID, if chosen.
    pub fn uuid(&self) -> Option<&str> {
        self.uuid.as_deref()
    }
}

/// Vault configuration (D30). No `Clone`; `Debug` does not show the hash.
#[derive(Debug)]
pub struct VaultConfig {
    pin_hash: PinHash,
    lock_minutes: u32,
    backup: ExternalBackup,
}

// The PIN hash is zeroed on drop; lock timeout and backup are not secret (V2.28).
impl ZeroizeOnDrop for VaultConfig {}

impl VaultConfig {
    /// Configuration of a new vault: the given PIN hash, lock after
    /// [`DEFAULT_LOCK_MINUTES`] minutes and external backup disabled.
    pub fn new(pin_hash: PinHash) -> Self {
        Self {
            pin_hash,
            lock_minutes: DEFAULT_LOCK_MINUTES,
            backup: ExternalBackup::disabled(),
        }
    }

    /// Built by the body reader, with everything already verified.
    pub(crate) fn from_parts(pin_hash: PinHash, lock_minutes: u32, backup: ExternalBackup) -> Self {
        Self {
            pin_hash,
            lock_minutes,
            backup,
        }
    }

    /// Verifies the PIN against the stored hash, in constant time (see [`PinHash::verify`]).
    pub fn verify_pin(&self, pin: &[u8]) -> bool {
        self.pin_hash.verify(pin)
    }

    /// The PIN hash.
    pub fn pin_hash(&self) -> &PinHash {
        &self.pin_hash
    }

    /// Replaces the PIN hash (the caller already verified the current PIN, V5.22).
    pub fn change_pin_hash(&mut self, pin_hash: PinHash) {
        self.pin_hash = pin_hash;
    }

    /// Lock timeout for inactivity, in minutes.
    pub fn lock_minutes(&self) -> u32 {
        self.lock_minutes
    }

    /// Changes the lock timeout (from [`MIN_LOCK_MINUTES`] to [`MAX_LOCK_MINUTES`]).
    pub fn set_lock_minutes(&mut self, minutes: u32) -> Result<(), ConfigError> {
        if !valid_minutes(minutes) {
            return Err(ConfigError::LockTimeout);
        }
        self.lock_minutes = minutes;
        Ok(())
    }

    /// External backup.
    pub fn backup(&self) -> &ExternalBackup {
        &self.backup
    }

    /// Changes the external backup.
    pub fn set_backup(&mut self, backup: ExternalBackup) {
        self.backup = backup;
    }
}
