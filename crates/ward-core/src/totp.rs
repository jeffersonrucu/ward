//! TOTP (RFC 6238 over the HOTP of RFC 4226), R8: the vault generates the code; whoever
//! receives the code (agent, extension, app) never receives the seed.
//!
//! - HMAC-SHA1 only through the RustCrypto `hmac` and `sha1` crates (S6). Only the glue
//!   stays here: big-endian counter, dynamic truncation (RFC 4226 §5.3) and the digits.
//! - The decoded seed lives in a `Zeroizing<Vec<u8>>`; the `Debug` of [`Totp`] prints
//!   `••••`. The HMAC state zeroes on drop (feature `zeroize` of `hmac`/`sha1`) and so
//!   does the HMAC output.
//! - The errors are fixed texts: the seed never goes into a message.
//! - The code is not a vault secret (the agent receives it, R8), which is why
//!   [`TotpCode`] prints the digits.

use core::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;
use zeroize::{Zeroize, Zeroizing};

use crate::entrada::Segredo;

/// Code period, in seconds (RFC 6238, default of every authenticator).
pub const PERIOD: u64 = 30;
/// Digits of the delivered code (`WARD_TOTP`, extension, app).
pub const DEFAULT_DIGITS: u32 = 6;
/// Smallest accepted seed, in base32 characters (80 bits), the same as entry
/// validation.
pub const MIN_SEED: usize = 16;
/// Largest accepted seed, in base32 characters, the same as entry validation.
pub const MAX_SEED: usize = 128;

/// TOTP error. Fixed texts only, never the seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TotpError {
    /// The seed is not base32 (A–Z, 2–7, `=` only at the end) or its size is outside
    /// 16–128.
    InvalidSeed,
    /// Number of digits outside 6–8.
    InvalidDigits,
    /// The system clock is before 1970.
    InvalidClock,
}

impl fmt::Display for TotpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidSeed => {
                "semente TOTP inválida: use base32 (A–Z e 2–7) com 16 a 128 caracteres"
            }
            Self::InvalidDigits => "o código TOTP tem de 6 a 8 dígitos",
            Self::InvalidClock => "o relógio do sistema está antes de 1970",
        })
    }
}

impl std::error::Error for TotpError {}

/// Code generator of a seed. No `Clone`; the seed zeroes on drop.
pub struct Totp {
    key: Zeroizing<Vec<u8>>,
    digits: u32,
}

impl fmt::Debug for Totp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Totp")
            .field("seed", &"••••")
            .field("digits", &self.digits)
            .finish()
    }
}

/// Code of one period. Not a vault secret (R8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TotpCode {
    code: String,
    remaining: u64,
}

impl TotpCode {
    /// The digits, with leading zeros.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Seconds until the code changes (1 to 30), for the app countdown.
    pub fn remaining(&self) -> u64 {
        self.remaining
    }
}

impl fmt::Display for TotpCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.code)
    }
}

impl Totp {
    /// Reads the base32 seed of the entry (6 digits, 30 s).
    pub fn from_seed(seed: &Segredo) -> Result<Self, TotpError> {
        Self::with_digits(seed, DEFAULT_DIGITS)
    }

    /// Reads the base32 seed with another number of digits (6 to 8).
    pub fn with_digits(seed: &Segredo, digits: u32) -> Result<Self, TotpError> {
        if !(6..=8).contains(&digits) {
            return Err(TotpError::InvalidDigits);
        }
        let key = decode_base32(seed.expor())?;
        Ok(Self { key, digits })
    }

    /// Code of the period that contains `unix_seconds`.
    pub fn code_at(&self, unix_seconds: u64) -> TotpCode {
        let counter = unix_seconds / PERIOD;
        let remaining = PERIOD - unix_seconds % PERIOD;
        let value = hotp(&self.key, counter) % 10u32.pow(self.digits);
        TotpCode {
            code: format!("{value:0width$}", width = self.digits as usize),
            remaining,
        }
    }

    /// Code of now, by the system clock.
    pub fn code_now(&self) -> Result<TotpCode, TotpError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| TotpError::InvalidClock)?;
        Ok(self.code_at(now.as_secs()))
    }
}

/// HOTP (RFC 4226 §5.3): HMAC-SHA1 of the counter and dynamic truncation, in 31 bits.
fn hotp(key: &[u8], counter: u64) -> u32 {
    // HMAC accepts a key of any size; `new_from_slice` does not fail for HMAC.
    let mut mac = match <Hmac<Sha1> as KeyInit>::new_from_slice(key) {
        Ok(m) => m,
        Err(_) => unreachable!("HMAC aceita chave de qualquer tamanho"),
    };
    mac.update(&counter.to_be_bytes());
    let mut output: [u8; 20] = mac.finalize().into_bytes().into();
    let offset = (output[19] & 0x0f) as usize;
    let excerpt = [
        output[offset],
        output[offset + 1],
        output[offset + 2],
        output[offset + 3],
    ];
    output.zeroize();
    u32::from_be_bytes(excerpt) & 0x7fff_ffff
}

/// Value of a base32 character (RFC 4648 §6), or `None`.
fn base32_value(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'2'..=b'7' => Some(c - b'2' + 26),
        _ => None,
    }
}

/// Decodes uppercase base32, with optional `=` only at the end; the bits left over at
/// the end (fewer than 8) are dropped, as in `oathtool` and the authenticators. Same
/// rules as the TOTP seed check of entry validation.
fn decode_base32(text: &[u8]) -> Result<Zeroizing<Vec<u8>>, TotpError> {
    let useful = text.iter().take_while(|&&c| c != b'=').count();
    let (data, padding) = text.split_at(useful);
    if !(MIN_SEED..=MAX_SEED).contains(&useful) || padding.iter().any(|&c| c != b'=') {
        return Err(TotpError::InvalidSeed);
    }
    let mut output = Zeroizing::new(Vec::with_capacity(useful * 5 / 8));
    let mut accumulated: u32 = 0;
    let mut bits: u32 = 0;
    for &c in data {
        let v = base32_value(c).ok_or(TotpError::InvalidSeed)?;
        accumulated = ((accumulated << 5) | u32::from(v)) & 0x1fff;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            output.push((accumulated >> bits) as u8);
        }
    }
    accumulated.zeroize();
    Ok(output)
}

#[cfg(test)]
mod tests;
