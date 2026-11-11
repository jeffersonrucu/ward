//! Vault file format (S6).
//!
//! ```text
//! offset size field
//!      0   8  magic "WARDCOFR"
//!      8   2  format version (u16 LE) = 1
//!     10   1  KDF  = 1 (Argon2id, version 0x13)
//!     11   1  AEAD = 1 (XChaCha20-Poly1305)
//!     12   4  Argon2id memory in KiB (u32 LE)
//!     16   4  Argon2id passes (u32 LE)
//!     20   4  Argon2id parallelism (u32 LE)
//!     24  32  salt
//!     56  24  nonce of the wrapped key
//!     80  48  wrapped data key (32 + 16 tag)
//!    128  24  data nonce
//!    152   8  size of the encrypted body with the tag (u64 LE)
//!    160   …  body: encrypted content ‖ tag
//! ```
//!
//! - KEK = Argon2id(master password, salt, header parameters).
//! - The data key is random (32 bytes from the system) and is wrapped by the KEK with
//!   XChaCha20-Poly1305; the AAD of the wrapping is bytes 0..80 of the header.
//! - The body is encrypted by the data key with XChaCha20-Poly1305; the AAD is the whole
//!   header (0..160). Every byte of the file is authenticated: the first 80 by both
//!   tags, the next 80 by the body tag, the body by its own tag, and the total size by
//!   the size field (which is in the AAD).
//! - Parameters below the minimum (256 MiB, 3 passes) or above the maximum are refused
//!   **before** deriving: a downgraded header does not make Argon2 run cheap, and an
//!   inflated one does not make the process allocate memory without end.
//! - Wrong password, tampered, truncated or extended file give the same error
//!   ([`FormatError::PasswordOrFile`]).
//!
//! No crypto here: the primitives are in [`crate::crypto`], which only calls audited
//! libraries.

use core::fmt;

use zeroize::{ZeroizeOnDrop, Zeroizing};

use crate::config::VaultConfig;
use crate::crypto::{self, KEY_SIZE, Key, NONCE_SIZE, TAG_SIZE};

/// Start of every Ward vault file.
pub const MAGIC: [u8; 8] = *b"WARDCOFR";
/// Current format version.
pub const VERSION: u16 = 1;
/// KDF identifier in the header: Argon2id version 0x13.
pub const KDF_ARGON2ID_13: u8 = 1;
/// AEAD identifier in the header: XChaCha20-Poly1305.
pub const AEAD_XCHACHA20POLY1305: u8 = 1;
/// Salt size, in bytes.
pub const SALT_SIZE: usize = 32;
/// Header size, in bytes.
pub const HEADER_SIZE: usize = 160;
/// Largest accepted content (in clear), in bytes: 512 MiB.
pub const MAX_CONTENT: usize = 512 * 1024 * 1024;

const WRAPPED_SIZE: usize = KEY_SIZE + TAG_SIZE;
// Field positions (see the drawing at the top of the module).
const P_VERSION: usize = 8;
const P_KDF: usize = 10;
const P_AEAD: usize = 11;
const P_MEMORY: usize = 12;
const P_PASSES: usize = 16;
const P_PARALLELISM: usize = 20;
const P_SALT: usize = 24;
const P_KEY_NONCE: usize = P_SALT + SALT_SIZE; // 56
const P_WRAPPED: usize = P_KEY_NONCE + NONCE_SIZE; // 80
const P_DATA_NONCE: usize = P_WRAPPED + WRAPPED_SIZE; // 128
const P_SIZE: usize = P_DATA_NONCE + NONCE_SIZE; // 152
const _: () = assert!(P_SIZE + 8 == HEADER_SIZE);

/// Argon2id parameters written in the header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    /// Memory, in KiB.
    pub memory_kib: u32,
    /// Passes (time cost).
    pub passes: u32,
    /// Parallelism (lanes).
    pub parallelism: u32,
}

impl KdfParams {
    /// Used to create a new vault: 256 MiB, 3 passes, 4 lanes.
    pub const DEFAULT: Self = Self {
        memory_kib: 256 * 1024,
        passes: 3,
        parallelism: 4,
    };
    /// Minimum accepted on opening and on creation (S6: ~256 MiB / 3 passes).
    pub const MINIMUM: Self = Self {
        memory_kib: 256 * 1024,
        passes: 3,
        parallelism: 1,
    };
    /// Maximum accepted: 2 GiB, 16 passes, 16 lanes. A header with a larger value is
    /// refused without allocating anything.
    pub const MAXIMUM: Self = Self {
        memory_kib: 2 * 1024 * 1024,
        passes: 16,
        parallelism: 16,
    };

    fn verify(&self, minimum: &Self) -> Result<(), FormatError> {
        let max = Self::MAXIMUM;
        if self.memory_kib > max.memory_kib
            || self.passes > max.passes
            || self.parallelism > max.parallelism
        {
            return Err(FormatError::ParamsAboveMaximum);
        }
        if self.memory_kib < minimum.memory_kib
            || self.passes < minimum.passes
            || self.parallelism < minimum.parallelism.max(1)
            // Argon2 requirement: 8 blocks of 1 KiB per lane.
            || u64::from(self.memory_kib) < 8 * u64::from(self.parallelism)
        {
            return Err(FormatError::ParamsBelowMinimum);
        }
        Ok(())
    }
}

/// What the header declares. **Not authenticated**: it serves to check the parameters;
/// only [`open`] proves that the file is intact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// Format version.
    pub version: u16,
    /// Argon2id parameters.
    pub kdf: KdfParams,
    /// Size of the encrypted body (with the tag), in bytes.
    pub body_size: u64,
}

/// Format error. No variant carries vault data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatError {
    /// Wrong master password, or a file that is tampered, truncated, extended or not a
    /// Ward vault. A single error, on purpose: the answer does not say which case it was.
    PasswordOrFile,
    /// Format version this Ward does not know.
    UnsupportedVersion(u16),
    /// The header declares an Argon2id weaker than the minimum.
    ParamsBelowMinimum,
    /// The header declares an Argon2id above the accepted maximum.
    ParamsAboveMaximum,
    /// Content larger than [`MAX_CONTENT`].
    ContentTooLarge,
    /// Empty master password.
    EmptyPassword,
    /// The system did not deliver random bytes.
    Random,
    /// Internal failure while encrypting (memory, for example).
    Encrypt,
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PasswordOrFile => {
                f.write_str("senha mestra errada ou arquivo do cofre adulterado")
            }
            Self::UnsupportedVersion(v) => {
                write!(f, "versão {v} do formato do cofre não suportada")
            }
            Self::ParamsBelowMinimum => f.write_str(
                "parâmetros do Argon2id abaixo do mínimo (256 MiB de memória e 3 passes)",
            ),
            Self::ParamsAboveMaximum => {
                f.write_str("parâmetros do Argon2id acima do máximo aceito")
            }
            Self::ContentTooLarge => f.write_str("conteúdo do cofre grande demais"),
            Self::EmptyPassword => f.write_str("a senha mestra não pode ser vazia"),
            Self::Random => f.write_str("o sistema não entregou bytes aleatórios"),
            Self::Encrypt => f.write_str("falha ao cifrar o cofre"),
        }
    }
}

impl std::error::Error for FormatError {}

/// Master password in memory. Zeroes on drop; no `Clone`, and the `Debug` does not show
/// the value.
pub struct MasterPassword(Zeroizing<Vec<u8>>);

impl MasterPassword {
    /// Takes ownership of the text (no copy) and zeroes it on drop.
    pub fn new(text: String) -> Self {
        Self(Zeroizing::new(text.into_bytes()))
    }

    /// Takes ownership of the bytes, which already zero on drop.
    pub fn from_bytes(bytes: Zeroizing<Vec<u8>>) -> Self {
        Self(bytes)
    }

    /// Master password that came from whoever typed it (app, CLI, socket), already in
    /// NFC (Phase 39c).
    ///
    /// The same text typed in an input method that composes (U+00E9) and in another that
    /// decomposes ("e" + U+0301) has different bytes; without normalizing, the password
    /// created in the app would not open through the unlock command of the CLI. Every
    /// daemon path goes through here, so the client does not need to normalize. ASCII
    /// (the common case) is already NFC and is not copied. Bytes that are not UTF-8 stay
    /// as they came (there is nothing to normalize; the new password refuses them,
    /// [`MasterPassword::is_valid_new`]).
    ///
    /// The normalized text goes to a `Zeroizing` buffer with the final capacity reserved
    /// beforehand (NFC grows at most 3× in UTF-8), so as not to leave a copy in a
    /// reallocation. Residue: the `unicode-normalization` iterator keeps on the heap,
    /// without zeroing, only sequences of more than 4 combining marks in a row.
    pub fn typed(bytes: Zeroizing<Vec<u8>>) -> Self {
        Self(nfc(bytes))
    }

    /// Is it good as a new master password? Valid UTF-8 and
    /// [`MIN_MASTER_PASSWORD_CHARS`] characters or more (code points, after NFC). The
    /// maximum size is the protocol's (`MAX_SECRET`).
    ///
    /// [`MIN_MASTER_PASSWORD_CHARS`]: crate::limits::MIN_MASTER_PASSWORD_CHARS
    pub fn is_valid_new(&self) -> bool {
        std::str::from_utf8(self.bytes())
            .is_ok_and(|t| t.chars().count() >= crate::limits::MIN_MASTER_PASSWORD_CHARS)
    }

    fn bytes(&self) -> &[u8] {
        &self.0
    }
}

/// NFC of the bytes, if they are UTF-8 (see [`MasterPassword::typed`]).
fn nfc(bytes: Zeroizing<Vec<u8>>) -> Zeroizing<Vec<u8>> {
    use unicode_normalization::{IsNormalized, UnicodeNormalization, is_nfc_quick};
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return bytes;
    };
    if text.is_ascii() || is_nfc_quick(text.chars()) == IsNormalized::Yes {
        return bytes;
    }
    let mut output = Zeroizing::new(String::with_capacity(
        bytes.len().saturating_mul(3).saturating_add(4),
    ));
    for c in text.nfc() {
        output.push(c);
    }
    // `into_bytes` moves the buffer (no copy); the outer `Zeroizing` zeroes the new one.
    Zeroizing::new(std::mem::take(&mut *output).into_bytes())
}

// The only field is a `Zeroizing`: the password zeroes on drop (V2.28).
impl ZeroizeOnDrop for MasterPassword {}

impl fmt::Debug for MasterPassword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MasterPassword(••••)")
    }
}

/// Unlocked vault (D30): the file keys, the vault config (PIN hash, lock timeout,
/// external backup) and the generation (D39). Every write of the entries
/// ([`crate::atomic_write::write_entries`]) carries this config, so no path writes a
/// body without the PIN hash. No `Clone`; the `Debug` does not show the key or the hash.
///
/// The generation (D39, D53) is the number of the last version read or sealed: every
/// write seals the next one and the `OpenVault` takes it right when sealing, before
/// writing the temporary file, and keeps it even if the write, the fsync, the rename or
/// the directory fsync fail (burned generation). So no pair of sealed files has the
/// same generation. It can be read ([`OpenVault::generation`]), but there is no public
/// setter, not even through the config:
///
/// ```compile_fail,E0609
/// # use ward_core::config::{PinHash, VaultConfig};
/// # use ward_core::format::{OpenVault, MasterPassword};
/// # let password = MasterPassword::new("falso-mestra-ward-01".to_owned());
/// # let cfg = VaultConfig::new(PinHash::create(b"135790").unwrap());
/// let mut vault = OpenVault::new(&password, cfg).unwrap();
/// vault.config_mut().generation = 0; // does not exist
/// ```
///
/// ```compile_fail,E0599
/// # use ward_core::config::{PinHash, VaultConfig};
/// # use ward_core::format::{OpenVault, MasterPassword};
/// # let password = MasterPassword::new("falso-mestra-ward-01".to_owned());
/// # let cfg = VaultConfig::new(PinHash::create(b"135790").unwrap());
/// let mut vault = OpenVault::new(&password, cfg).unwrap();
/// vault.config_mut().set_generation(0); // does not exist
/// ```
///
/// ```compile_fail,E0616
/// # use ward_core::config::{PinHash, VaultConfig};
/// # use ward_core::format::{OpenVault, MasterPassword};
/// # let password = MasterPassword::new("falso-mestra-ward-01".to_owned());
/// # let cfg = VaultConfig::new(PinHash::create(b"135790").unwrap());
/// let mut vault = OpenVault::new(&password, cfg).unwrap();
/// vault.generation = 0; // private field
/// ```
///
/// Sealing loose bytes is not public either (D44): from outside, the path is
/// `write_entries`, which serializes the body (with the PIN hash) first.
///
/// ```compile_fail,E0624
/// # use ward_core::config::{PinHash, VaultConfig};
/// # use ward_core::format::{OpenVault, MasterPassword};
/// # let password = MasterPassword::new("falso-mestra-ward-01".to_owned());
/// # let cfg = VaultConfig::new(PinHash::create(b"135790").unwrap());
/// let vault = OpenVault::new(&password, cfg).unwrap();
/// let _ = vault.seal(b"body without a PIN hash");
/// ```
pub struct OpenVault {
    keys: VaultKeys,
    config: VaultConfig,
    generation: u64,
}

// The data key and the PIN hash zero on drop (V2.28).
impl ZeroizeOnDrop for OpenVault {}

impl fmt::Debug for OpenVault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenVault")
            .field("keys", &self.keys)
            .field("config", &self.config)
            .field("generation", &self.generation)
            .finish()
    }
}

impl OpenVault {
    /// New vault with [`KdfParams::DEFAULT`]: new salt, nonce and data key, and the given
    /// config (which already carries the PIN hash). Not written yet: generation 0, and
    /// the first write writes 1 (D39).
    pub fn new(password: &MasterPassword, config: VaultConfig) -> Result<Self, FormatError> {
        Self::new_with(password, KdfParams::DEFAULT, config)
    }

    /// New vault with the given parameters (refuses below the minimum and above the
    /// maximum).
    pub fn new_with(
        password: &MasterPassword,
        kdf: KdfParams,
        config: VaultConfig,
    ) -> Result<Self, FormatError> {
        Ok(Self {
            keys: VaultKeys::new_with(password, kdf)?,
            config,
            generation: 0,
        })
    }

    /// New vault that already knows the `burned` generation (D62): the first write seals
    /// `burned + 1`. For the daemon that creates a vault where an old state file already
    /// burned generations: an old copy of a previous vault, put in place later, stays
    /// below the seen generation and is refused as a rollback. Never lowers the generation.
    pub fn after_generation(mut self, burned: u64) -> Self {
        self.generation = self.generation.max(burned);
        self
    }

    /// Joins the keys of an opened file to the config and the generation read from its
    /// body.
    pub(crate) fn from_parts(keys: VaultKeys, config: VaultConfig, generation: u64) -> Self {
        Self {
            keys,
            config,
            generation,
        }
    }

    /// Generation of this vault (D39, D53): the last one read or sealed. On opening, it
    /// is the larger of the body's and the burned generation the caller knows; after each
    /// write, the one it sealed, whether the write succeeded or not. 0 in a new vault not
    /// written yet. Read only.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The generation the next write writes. `None` at `u64::MAX`: the write is refused
    /// before touching the disk (D39).
    pub(crate) fn next_generation(&self) -> Option<u64> {
        self.generation.checked_add(1)
    }

    /// Burns the generation `generation` (D53): called right after sealing the body with
    /// it, before writing the temporary file. The counter never goes down.
    pub(crate) fn burn(&mut self, generation: u64) {
        debug_assert!(generation > self.generation);
        self.generation = self.generation.max(generation);
    }

    /// Argon2id parameters of this vault.
    pub fn params(&self) -> KdfParams {
        self.keys.kdf
    }

    /// Vault config (D30).
    pub fn config(&self) -> &VaultConfig {
        &self.config
    }

    /// Vault config, to change (PIN, lock timeout, backup). Goes to disk on the next
    /// write of the entries. The generation is not in the config and does not change
    /// through here (D39).
    pub fn config_mut(&mut self) -> &mut VaultConfig {
        &mut self.config
    }

    /// Encrypts `content` and returns the whole file. Each call uses a new data nonce;
    /// salt, wrapping and data key stay the same. Only inside the crate (D44): it would
    /// seal any body, including one without a PIN hash.
    pub(crate) fn seal(&self, content: &[u8]) -> Result<Vec<u8>, FormatError> {
        self.keys.seal(content)
    }

    /// Was the file sealed by this vault (same salt, same wrapping of the data key, and
    /// the body opens with the data key)? No Argon2. The decrypted content zeroes on
    /// drop. For the backup of the current vault (D17, Phase 45): only an authentic file
    /// goes to the backups folder.
    pub(crate) fn authenticates(&self, file: &[u8]) -> bool {
        self.keys.open_own(file).is_ok()
    }

    /// The generation sealed in the body of `file`, if it was sealed by this vault (same
    /// data key) and the whole body is valid. No Argon2; the decrypted content zeroes on
    /// drop. For the restore list (Phase 49a): the generation is authenticated, the date
    /// in the file name is not.
    pub(crate) fn sealed_generation(&self, file: &[u8]) -> Option<u64> {
        let content = self.keys.open_own(file).ok()?;
        crate::corpo::geracao_do_corpo(&content)
    }
}

/// Keys of a vault file: the data key (which zeroes on drop) and its wrapping, to write
/// again without running Argon2 again. Only inside the crate: from outside, the path is
/// [`OpenVault`], which also carries the config.
pub(crate) struct VaultKeys {
    kdf: KdfParams,
    salt: [u8; SALT_SIZE],
    key_nonce: [u8; NONCE_SIZE],
    wrapped: [u8; WRAPPED_SIZE],
    data_key: Key,
}

// The data key is `Zeroizing` and zeroes on drop; salt, nonce and wrapping are not
// secret (they are in the file header) (V2.28).
impl ZeroizeOnDrop for VaultKeys {}

impl fmt::Debug for VaultKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VaultKeys")
            .field("kdf", &self.kdf)
            .field("data_key", &"••••")
            .finish_non_exhaustive()
    }
}

impl VaultKeys {
    /// New keys with the given parameters (refuses below the minimum and above the
    /// maximum).
    pub(crate) fn new_with(password: &MasterPassword, kdf: KdfParams) -> Result<Self, FormatError> {
        Self::new_internal(password, kdf, &KdfParams::MINIMUM)
    }

    fn new_internal(
        password: &MasterPassword,
        kdf: KdfParams,
        minimum: &KdfParams,
    ) -> Result<Self, FormatError> {
        if password.bytes().is_empty() {
            return Err(FormatError::EmptyPassword);
        }
        kdf.verify(minimum)?;
        let mut salt = [0u8; SALT_SIZE];
        let mut key_nonce = [0u8; NONCE_SIZE];
        crypto::random(&mut salt).map_err(|_| FormatError::Random)?;
        crypto::random(&mut key_nonce).map_err(|_| FormatError::Random)?;
        let data_key = crypto::random_key().map_err(|_| FormatError::Random)?;
        let kek =
            crypto::derive_kek(password.bytes(), &salt, &kdf).map_err(|_| FormatError::Encrypt)?;
        let prefix = header_prefix(&kdf, &salt, &key_nonce);
        let wrapped = crypto::seal(&kek, &key_nonce, &prefix, &data_key[..])
            .map_err(|_| FormatError::Encrypt)?;
        let wrapped: [u8; WRAPPED_SIZE] = wrapped.try_into().map_err(|_| FormatError::Encrypt)?;
        Ok(Self {
            kdf,
            salt,
            key_nonce,
            wrapped,
            data_key,
        })
    }

    /// Argon2id parameters of these keys.
    pub(crate) fn params(&self) -> KdfParams {
        self.kdf
    }

    /// Encrypts `content` and returns the whole file (see [`OpenVault::seal`]).
    pub(crate) fn seal(&self, content: &[u8]) -> Result<Vec<u8>, FormatError> {
        if content.len() > MAX_CONTENT {
            return Err(FormatError::ContentTooLarge);
        }
        let mut data_nonce = [0u8; NONCE_SIZE];
        crypto::random(&mut data_nonce).map_err(|_| FormatError::Random)?;
        let body_size = (content.len() + TAG_SIZE) as u64;

        let mut header = [0u8; HEADER_SIZE];
        header[..P_WRAPPED].copy_from_slice(&header_prefix(&self.kdf, &self.salt, &self.key_nonce));
        header[P_WRAPPED..P_DATA_NONCE].copy_from_slice(&self.wrapped);
        header[P_DATA_NONCE..P_SIZE].copy_from_slice(&data_nonce);
        header[P_SIZE..].copy_from_slice(&body_size.to_le_bytes());

        let body = crypto::seal(&self.data_key, &data_nonce, &header, content)
            .map_err(|_| FormatError::Encrypt)?;
        let mut file = Vec::new();
        file.try_reserve_exact(HEADER_SIZE + body.len())
            .map_err(|_| FormatError::Encrypt)?;
        file.extend_from_slice(&header);
        file.extend_from_slice(&body);
        Ok(file)
    }

    /// Opens a file sealed by these keys: structure, the first 128 header bytes equal to
    /// these keys' (parameters, salt, nonce and wrapped key; they are public, the
    /// comparison needs no constant time) and the body by the AEAD with the data key. Any
    /// difference gives [`FormatError::PasswordOrFile`].
    fn open_own(&self, file: &[u8]) -> Result<Zeroizing<Vec<u8>>, FormatError> {
        read_header(file)?;
        let prefix = header_prefix(&self.kdf, &self.salt, &self.key_nonce);
        if file[..P_WRAPPED] != prefix || file[P_WRAPPED..P_DATA_NONCE] != self.wrapped {
            return Err(FormatError::PasswordOrFile);
        }
        let mut data_nonce = [0u8; NONCE_SIZE];
        data_nonce.copy_from_slice(&file[P_DATA_NONCE..P_SIZE]);
        crypto::open(
            &self.data_key,
            &data_nonce,
            &file[..HEADER_SIZE],
            &file[HEADER_SIZE..],
        )
        .map_err(|_| FormatError::PasswordOrFile)
    }
}

/// Bytes 0..80 of the header (the AAD of the key wrapping).
fn header_prefix(
    kdf: &KdfParams,
    salt: &[u8; SALT_SIZE],
    key_nonce: &[u8; NONCE_SIZE],
) -> [u8; P_WRAPPED] {
    let mut p = [0u8; P_WRAPPED];
    p[..P_VERSION].copy_from_slice(&MAGIC);
    p[P_VERSION..P_KDF].copy_from_slice(&VERSION.to_le_bytes());
    p[P_KDF] = KDF_ARGON2ID_13;
    p[P_AEAD] = AEAD_XCHACHA20POLY1305;
    p[P_MEMORY..P_PASSES].copy_from_slice(&kdf.memory_kib.to_le_bytes());
    p[P_PASSES..P_PARALLELISM].copy_from_slice(&kdf.passes.to_le_bytes());
    p[P_PARALLELISM..P_SALT].copy_from_slice(&kdf.parallelism.to_le_bytes());
    p[P_SALT..P_KEY_NONCE].copy_from_slice(salt);
    p[P_KEY_NONCE..].copy_from_slice(key_nonce);
    p
}

fn u32_at(file: &[u8], pos: usize) -> u32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&file[pos..pos + 4]);
    u32::from_le_bytes(b)
}

/// Reads and checks the header structure (no password and no authentication): magic,
/// version, algorithms and exact file size.
pub fn read_header(file: &[u8]) -> Result<Header, FormatError> {
    if file.len() < HEADER_SIZE || file[..P_VERSION] != MAGIC {
        return Err(FormatError::PasswordOrFile);
    }
    let version = u16::from_le_bytes([file[P_VERSION], file[P_VERSION + 1]]);
    if version != VERSION {
        return Err(FormatError::UnsupportedVersion(version));
    }
    if file[P_KDF] != KDF_ARGON2ID_13 || file[P_AEAD] != AEAD_XCHACHA20POLY1305 {
        return Err(FormatError::PasswordOrFile);
    }
    let mut t = [0u8; 8];
    t.copy_from_slice(&file[P_SIZE..HEADER_SIZE]);
    let body_size = u64::from_le_bytes(t);
    let real = (file.len() - HEADER_SIZE) as u64;
    if body_size != real
        || body_size < TAG_SIZE as u64
        || body_size > (MAX_CONTENT + TAG_SIZE) as u64
    {
        return Err(FormatError::PasswordOrFile);
    }
    Ok(Header {
        version,
        kdf: KdfParams {
            memory_kib: u32_at(file, P_MEMORY),
            passes: u32_at(file, P_PASSES),
            parallelism: u32_at(file, P_PARALLELISM),
        },
        body_size,
    })
}

/// Opens the vault: checks structure and parameters, derives the KEK, unwraps the data
/// key and decrypts the content. The clear content zeroes on drop. Only inside the
/// crate: from outside, the path is [`crate::corpo::abrir_entradas`], which reads the
/// body and the config.
pub(crate) fn open(
    file: &[u8],
    password: &MasterPassword,
) -> Result<(VaultKeys, Zeroizing<Vec<u8>>), FormatError> {
    open_with_minimum(file, password, &KdfParams::MINIMUM)
}

fn open_with_minimum(
    file: &[u8],
    password: &MasterPassword,
    minimum: &KdfParams,
) -> Result<(VaultKeys, Zeroizing<Vec<u8>>), FormatError> {
    let header = read_header(file)?;
    header.kdf.verify(minimum)?;

    let mut salt = [0u8; SALT_SIZE];
    salt.copy_from_slice(&file[P_SALT..P_KEY_NONCE]);
    let mut key_nonce = [0u8; NONCE_SIZE];
    key_nonce.copy_from_slice(&file[P_KEY_NONCE..P_WRAPPED]);
    let mut wrapped = [0u8; WRAPPED_SIZE];
    wrapped.copy_from_slice(&file[P_WRAPPED..P_DATA_NONCE]);
    let mut data_nonce = [0u8; NONCE_SIZE];
    data_nonce.copy_from_slice(&file[P_DATA_NONCE..P_SIZE]);

    let kek = crypto::derive_kek(password.bytes(), &salt, &header.kdf)
        .map_err(|_| FormatError::PasswordOrFile)?;
    let key = crypto::open(&kek, &key_nonce, &file[..P_WRAPPED], &wrapped)
        .map_err(|_| FormatError::PasswordOrFile)?;
    drop(kek);
    // Straight from the decrypted buffer (which zeroes) to the heap (D40).
    let data_key = Key::from_bytes(&key).map_err(|_| FormatError::PasswordOrFile)?;
    drop(key);

    let content = crypto::open(
        &data_key,
        &data_nonce,
        &file[..HEADER_SIZE],
        &file[HEADER_SIZE..],
    )
    .map_err(|_| FormatError::PasswordOrFile)?;

    Ok((
        VaultKeys {
            kdf: header.kdf,
            salt,
            key_nonce,
            wrapped,
            data_key,
        },
        content,
    ))
}

/// Result of the password change: the new keys and the clear content (which zeroes on
/// drop).
pub(crate) type PasswordChanged = (VaultKeys, Zeroizing<Vec<u8>>);

/// Changes the master password. Requires the current password. Returns new keys (new
/// salt, data key and wrapping nonce: whoever has the old password and an old copy does
/// not open the new one), with the same parameters as the vault or the
/// [`KdfParams::DEFAULT`], whichever is larger, and the current content. Does not seal:
/// the caller ([`crate::atomic_write::change_password`]) reads the body, writes the next
/// generation (D39) and seals with the new keys.
pub(crate) fn change_password(
    file: &[u8],
    current: &MasterPassword,
    new: &MasterPassword,
) -> Result<PasswordChanged, FormatError> {
    if new.bytes().is_empty() {
        return Err(FormatError::EmptyPassword);
    }
    let (old, content) = open(file, current)?;
    let p = old.params();
    drop(old);
    let default = KdfParams::DEFAULT;
    let kdf = KdfParams {
        memory_kib: p.memory_kib.max(default.memory_kib),
        passes: p.passes.max(default.passes),
        parallelism: p.parallelism.max(default.parallelism),
    };
    let new_keys = VaultKeys::new_with(new, kdf)?;
    Ok((new_keys, content))
}

#[cfg(test)]
mod tests;
