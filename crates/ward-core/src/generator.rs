//! Password generator (R10): size from 4 to 128 and configurable sets (some panels do
//! not accept symbols).
//!
//! - The source is only the system RNG (`getrandom`, through `crypto::random`). If it
//!   fails, the generator fails: there is no fallback RNG nor fixed seed.
//! - Each character comes out uniform from the selected alphabet, by rejection (a byte
//!   above the largest multiple of the alphabet size is dropped), without modulo bias.
//! - "At least one of each selected set" also by rejection: the whole password is drawn
//!   again until it has them all. That keeps the distribution uniform among the valid
//!   passwords (no fixed position for each set).
//! - The password comes out as a secret that zeroes on drop; the random bytes and the
//!   dropped attempts are zeroed.

use core::fmt;

use zeroize::{Zeroize, Zeroizing};

use crate::crypto::{self, CryptoFailure};
use crate::entry::Secret;

/// Smallest accepted size.
pub const MIN_SIZE: usize = 4;
/// Largest accepted size.
pub const MAX_SIZE: usize = 128;
/// Default size of the app (matches the design: 24 characters).
pub const DEFAULT_SIZE: usize = 24;

/// Lowercase letters.
pub const LOWERCASE: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
/// Uppercase letters.
pub const UPPERCASE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
/// Digits.
pub const DIGITS: &[u8] = b"0123456789";
/// ASCII symbols. Space, quotes (`"` `'`), backtick and backslash are left out: they
/// break shell quoting, `.env` and the configuration files where the password ends up.
pub const SYMBOLS: &[u8] = b"!#$%&()*+,-./:;<=>?@[]^_{|}~";

/// Ceiling of attempts to meet "one of each set". In the worst case (size 4 with the 4
/// sets) the chance per attempt is ~7 %, so 10 000 failing attempts is impossible in
/// practice; the ceiling only prevents an endless loop with a broken RNG.
const MAX_ATTEMPTS: u32 = 10_000;

/// Generator options. Default: 24 characters with the 4 sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneratorOptions {
    /// Size, from 4 to 128.
    pub size: usize,
    /// a–z.
    pub lowercase: bool,
    /// A–Z.
    pub uppercase: bool,
    /// 0–9.
    pub digits: bool,
    /// [`SYMBOLS`].
    pub symbols: bool,
}

impl Default for GeneratorOptions {
    fn default() -> Self {
        Self {
            size: DEFAULT_SIZE,
            lowercase: true,
            uppercase: true,
            digits: true,
            symbols: true,
        }
    }
}

/// Generator error, in Portuguese, without generated data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratorError {
    /// Size outside 4–128.
    InvalidSize,
    /// No set selected.
    NoSet,
    /// The system RNG did not deliver bytes.
    RandomUnavailable,
}

impl fmt::Display for GeneratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidSize => "o tamanho da senha vai de 4 a 128 caracteres",
            Self::NoSet => "marque pelo menos um conjunto de caracteres",
            Self::RandomUnavailable => "o gerador aleatório do sistema não respondeu",
        })
    }
}

impl std::error::Error for GeneratorError {}

impl GeneratorOptions {
    /// The selected sets, in fixed order.
    fn sets(&self) -> Vec<&'static [u8]> {
        [
            (self.lowercase, LOWERCASE),
            (self.uppercase, UPPERCASE),
            (self.digits, DIGITS),
            (self.symbols, SYMBOLS),
        ]
        .into_iter()
        .filter_map(|(selected, c)| selected.then_some(c))
        .collect()
    }
}

/// Generates a password with the system RNG.
pub fn generate(options: &GeneratorOptions) -> Result<Secret, GeneratorError> {
    generate_with(options, &mut crypto::random)
}

/// Random bytes read in batches, which zero on drop.
struct Reserve<'a> {
    source: &'a mut dyn FnMut(&mut [u8]) -> Result<(), CryptoFailure>,
    bytes: Zeroizing<[u8; 256]>,
    used: usize,
}

impl<'a> Reserve<'a> {
    fn new(source: &'a mut dyn FnMut(&mut [u8]) -> Result<(), CryptoFailure>) -> Self {
        Self {
            source,
            bytes: Zeroizing::new([0; 256]),
            used: 256,
        }
    }

    fn byte(&mut self) -> Result<u8, GeneratorError> {
        if self.used == self.bytes.len() {
            (self.source)(&mut self.bytes[..]).map_err(|_| GeneratorError::RandomUnavailable)?;
            self.used = 0;
        }
        let b = self.bytes[self.used];
        self.bytes[self.used] = 0;
        self.used += 1;
        Ok(b)
    }

    /// Uniform index in `0..n` (1 ≤ n ≤ 256), by rejection.
    fn index(&mut self, n: usize) -> Result<usize, GeneratorError> {
        debug_assert!((1..=256).contains(&n));
        let limit = 256 - 256 % n;
        // With n ≥ 10 the chance of rejecting is < 40 %; 1 000 rejections in a row only
        // come from a broken source (always the same high byte), and then the generator
        // fails.
        for _ in 0..1_000 {
            let b = usize::from(self.byte()?);
            if b < limit {
                return Ok(b % n);
            }
        }
        Err(GeneratorError::RandomUnavailable)
    }
}

pub(crate) fn generate_with(
    options: &GeneratorOptions,
    // Source of random bytes; only `crypto::random` outside the tests.
    source: &mut dyn FnMut(&mut [u8]) -> Result<(), CryptoFailure>,
) -> Result<Secret, GeneratorError> {
    if !(MIN_SIZE..=MAX_SIZE).contains(&options.size) {
        return Err(GeneratorError::InvalidSize);
    }
    let sets = options.sets();
    if sets.is_empty() {
        return Err(GeneratorError::NoSet);
    }
    let alphabet: Vec<u8> = sets.concat();
    let mut reserve = Reserve::new(source);
    let mut password = Zeroizing::new(Vec::with_capacity(options.size));
    for _ in 0..MAX_ATTEMPTS {
        password.zeroize();
        for _ in 0..options.size {
            password.push(alphabet[reserve.index(alphabet.len())?]);
        }
        if sets.iter().all(|c| password.iter().any(|b| c.contains(b))) {
            // Takes ownership of the buffer without copying; the `Zeroizing` keeps an
            // empty vector.
            return Ok(Secret::from_bytes(core::mem::take(&mut *password)));
        }
    }
    Err(GeneratorError::RandomUnavailable)
}

/// Size of the Connect button token (D3): 32 characters of `[A-Za-z0-9]` (~190 bits).
pub const TOKEN_SIZE: usize = 32;

/// Single-use token of the Connect button (D3, Phase 27): random by the system RNG, only letters
/// and digits (it fits in the terminal argv without quotes and in the format the app
/// accepts).
pub fn connect_token() -> Result<Secret, GeneratorError> {
    generate(&GeneratorOptions {
        size: TOKEN_SIZE,
        lowercase: true,
        uppercase: true,
        digits: true,
        symbols: false,
    })
}

/// Equality of two tokens in constant time, by the library (the `CtEq` of `ctutils`).
/// Different sizes give `false`.
pub fn tokens_equal(a: &[u8], b: &[u8]) -> bool {
    crypto::constant_time_eq(a, b)
}

#[cfg(test)]
mod tests;
