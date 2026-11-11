//! Standalone password generator (Phase 37, screen `Gerador de senha / Avulso`).
//!
//! The password is born only in `ward_core::generator` (system RNG, no bias, D35
//! symbols), never in JavaScript. It stays here, on the Rust side, in a [`Secret`] that
//! is zeroed on drop:
//!
//! - `gerar_senha` returns to the interface only what is not secret: the size, the
//!   strength estimated from the options and the D35 symbol set (so the screen shows the
//!   same set the generator uses);
//! - `copiar_senha_gerada` sends the password to the clipboard through the same
//!   [`crate::clipboard::Copier`] as the "copy" (S12, D4): the response is `()`;
//! - `revelar_senha_gerada` is the only path of the value to the webview, and only on the
//!   "reveal" click the design shows (S11). The screen hides it again in 30 s or on
//!   leaving;
//! - `descartar_senha_gerada` zeroes the password on leaving the screen. Nothing is
//!   stored: the standalone password stays neither in the vault nor on disk.

use std::fmt;

use ward_core::generator::{self as core_generator, GeneratorOptions};
use zeroize::Zeroizing;

use crate::AppError;
use crate::secret::Secret;

/// Smallest size on the screen (the design says "De 8 a 64"). Stays within
/// `ward_core::generator::MIN_SIZE..=MAX_SIZE`.
pub const MIN_LENGTH: usize = 8;
/// Largest size on the screen.
pub const MAX_LENGTH: usize = 64;
/// Initial size (the same as the core generator and the design: "24 caracteres").
pub const DEFAULT_LENGTH: usize = core_generator::DEFAULT_SIZE;

/// Options that came from the interface (untrusted): the size is checked here and again
/// in the core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub size: u64,
    pub lowercase: bool,
    pub uppercase: bool,
    pub digits: bool,
    pub symbols: bool,
}

impl Options {
    fn validate(self) -> Result<GeneratorOptions, AppError> {
        let size = usize::try_from(self.size).map_err(|_| AppError::InvalidRequest)?;
        if !(MIN_LENGTH..=MAX_LENGTH).contains(&size) {
            return Err(AppError::InvalidRequest);
        }
        if !(self.lowercase || self.uppercase || self.digits || self.symbols) {
            return Err(AppError::InvalidRequest);
        }
        Ok(GeneratorOptions {
            size,
            lowercase: self.lowercase,
            uppercase: self.uppercase,
            digits: self.digits,
            symbols: self.symbols,
        })
    }
}

/// Strength from the entropy of the options (not from the generated value, which the
/// interface does not see).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strength {
    Weak,
    Fair,
    Strong,
    VeryStrong,
}

impl Strength {
    fn from_bits(bits: u32) -> Strength {
        match bits {
            0..60 => Strength::Weak,
            60..80 => Strength::Fair,
            80..128 => Strength::Strong,
            _ => Strength::VeryStrong,
        }
    }

    /// Stable code for the interface.
    pub fn code(self) -> &'static str {
        match self {
            Strength::Weak => "fraca",
            Strength::Fair => "razoavel",
            Strength::Strong => "forte",
            Strength::VeryStrong => "muito-forte",
        }
    }
}

/// What `gerar_senha` returns: nothing of the value (not a single character).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Generated {
    pub size: usize,
    /// Entropy rounded down, in steps of 10 ("cerca de 150 bits").
    pub bits: u32,
    pub strength: Strength,
    /// D35 symbols, from `ward_core::generator::SYMBOLS`.
    pub symbols: &'static str,
}

/// D35 symbols as text (they are ASCII).
pub fn symbols() -> &'static str {
    // `SYMBOLS` is ASCII by construction; the test checks it.
    core::str::from_utf8(core_generator::SYMBOLS).unwrap_or("")
}

/// Entropy in bits of a uniform password with these options, rounded down in steps of
/// 10. Ignores the small discount of "one of each set" (less than 1 bit).
fn bits(o: &GeneratorOptions) -> u32 {
    let alphabet = [
        (o.lowercase, core_generator::LOWERCASE.len()),
        (o.uppercase, core_generator::UPPERCASE.len()),
        (o.digits, core_generator::DIGITS.len()),
        (o.symbols, core_generator::SYMBOLS.len()),
    ]
    .iter()
    .filter(|(m, _)| *m)
    .map(|(_, n)| *n)
    .sum::<usize>();
    // size ≤ 64 and alphabet ≤ 90: fits with room to spare in f64 and in u32.
    let total = f64::from(u32::try_from(o.size).unwrap_or(0))
        * f64::from(u32::try_from(alphabet).unwrap_or(0)).log2();
    let whole = total.floor().clamp(0.0, 10_000.0) as u32;
    whole - whole % 10
}

/// Revealed value. Zeroed on drop; the `Debug` does not show the value. Only the
/// `revelar_senha_gerada` command creates it.
pub struct Revealed(Zeroizing<String>);

impl Revealed {
    /// Value of a vault secret for the form's "reveal" (Phase 33). UTF-8 only.
    pub(crate) fn from_secret(s: &Secret) -> Option<Revealed> {
        let text = core::str::from_utf8(s.expose()).ok()?;
        Some(Revealed(Zeroizing::new(text.to_owned())))
    }

    /// The text, only for serializing the "reveal" response.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Revealed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Revealed(••••)")
    }
}

/// The current standalone password, only in memory on the Rust side.
#[derive(Default)]
pub struct Generator {
    current: Option<Secret>,
}

impl Generator {
    /// Generates a new password (the previous one is zeroed) and returns only the data
    /// without a secret.
    pub fn generate(&mut self, options: Options) -> Result<Generated, AppError> {
        let o = options.validate()?;
        // Before generating: if the RNG fails, the old password does not remain with new
        // options.
        self.current = None;
        let password = core_generator::generate(&o).map_err(|e| match e {
            core_generator::GeneratorError::RandomUnavailable => AppError::RandomUnavailable,
            core_generator::GeneratorError::InvalidSize | core_generator::GeneratorError::NoSet => {
                AppError::InvalidRequest
            }
        })?;
        // Copies to the app type, which is zeroed on drop; the `ward_core` secret is
        // zeroed on drop.
        self.current = Some(Secret::new(password.expose().to_vec()));
        drop(password);
        let bits = bits(&o);
        Ok(Generated {
            size: o.size,
            bits,
            strength: Strength::from_bits(bits),
            symbols: symbols(),
        })
    }

    /// A copy for the clipboard (the `Copier` takes ownership and zeroes it).
    pub fn for_copy(&self) -> Result<Secret, AppError> {
        self.current
            .as_ref()
            .map(|s| Secret::new(s.expose().to_vec()))
            .ok_or(AppError::NoGeneratedPassword)
    }

    /// The value for the "reveal".
    pub fn reveal(&self) -> Result<Revealed, AppError> {
        let s = self.current.as_ref().ok_or(AppError::NoGeneratedPassword)?;
        let text = core::str::from_utf8(s.expose()).map_err(|_| AppError::NoGeneratedPassword)?;
        Ok(Revealed(Zeroizing::new(text.to_owned())))
    }

    /// Zeroes the password (on leaving the screen, locking or closing the app).
    pub fn discard(&mut self) {
        self.current = None;
    }

    #[cfg(test)]
    pub(crate) fn has_password(&self) -> bool {
        self.current.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(size: u64) -> Options {
        Options {
            size,
            lowercase: true,
            uppercase: true,
            digits: true,
            symbols: true,
        }
    }

    #[test]
    fn screen_limits_within_the_core() {
        const { assert!(MIN_LENGTH >= core_generator::MIN_SIZE) };
        const { assert!(MAX_LENGTH <= core_generator::MAX_SIZE) };
        const { assert!(MIN_LENGTH <= DEFAULT_LENGTH && DEFAULT_LENGTH <= MAX_LENGTH) };
    }

    #[test]
    fn symbols_are_those_of_d35() {
        assert_eq!(symbols(), "!#$%&()*+,-./:;<=>?@[]^_{|}~");
        for forbidden in [' ', '"', '\'', '`', '\\'] {
            assert!(!symbols().contains(forbidden), "{forbidden:?}");
        }
    }

    #[test]
    fn design_strength() {
        // Design: 24 characters, four sets → "muito forte · cerca de 150 bits".
        let mut g = Generator::default();
        let r = g.generate(options(24)).unwrap();
        assert_eq!((r.size, r.bits, r.strength), (24, 150, Strength::VeryStrong));
        let digits_only = Options {
            size: 8,
            lowercase: false,
            uppercase: false,
            digits: true,
            symbols: false,
        };
        let r = g.generate(digits_only).unwrap();
        assert_eq!((r.bits, r.strength), (20, Strength::Weak));
        let r = g.generate(options(8)).unwrap();
        assert_eq!((r.bits, r.strength), (50, Strength::Weak));
        let r = g.generate(options(12)).unwrap();
        assert_eq!((r.bits, r.strength), (70, Strength::Fair));
        let r = g.generate(options(16)).unwrap();
        assert_eq!((r.bits, r.strength), (100, Strength::Strong));
        let r = g.generate(options(64)).unwrap();
        assert_eq!(r.strength, Strength::VeryStrong);
    }

    #[test]
    fn password_respects_size_and_sets() {
        let mut g = Generator::default();
        let without_symbols = Options {
            symbols: false,
            ..options(32)
        };
        for _ in 0..50 {
            g.generate(without_symbols).unwrap();
            let v = g.reveal().unwrap();
            assert_eq!(v.expose().len(), 32);
            assert!(v.expose().bytes().all(|b| b.is_ascii_alphanumeric()));
        }
        for _ in 0..50 {
            g.generate(options(8)).unwrap();
            let v = g.reveal().unwrap();
            assert!(
                v.expose()
                    .bytes()
                    .any(|b| symbols().as_bytes().contains(&b))
            );
        }
    }

    /// Interface input out of format: size outside 8–64 (including values the core would
    /// accept and a huge `u64`) and no set. Nothing is generated and the previous password
    /// does not become available again with options that failed.
    #[test]
    fn attack_options_out_of_format_are_refused() {
        let mut g = Generator::default();
        for t in [0, 4, 7, 65, 128, 129, u64::from(u32::MAX) + 1, u64::MAX] {
            assert_eq!(g.generate(options(t)), Err(AppError::InvalidRequest), "{t}");
        }
        let none = Options {
            size: 24,
            lowercase: false,
            uppercase: false,
            digits: false,
            symbols: false,
        };
        assert_eq!(g.generate(none), Err(AppError::InvalidRequest));
        assert!(!g.has_password());
        assert_eq!(g.reveal().err(), Some(AppError::NoGeneratedPassword));
        assert_eq!(g.for_copy().err(), Some(AppError::NoGeneratedPassword));
    }

    /// The "generate" response does not carry the value: neither in the type nor in the
    /// `Debug`.
    #[test]
    fn attack_generate_does_not_return_the_value() {
        let mut g = Generator::default();
        let r = g.generate(options(64)).unwrap();
        let v = g.reveal().unwrap();
        let text = format!("{r:?} {v:?}");
        assert!(!text.contains(v.expose()));
        assert!(text.contains("Revealed(••••)"));
        // No 6-character piece of the value appears in the response.
        for i in 0..v.expose().len() - 6 {
            assert!(!format!("{r:?}").contains(&v.expose()[i..i + 6]));
        }
    }

    #[test]
    fn discard_zeroes_and_generating_again_changes() {
        let mut g = Generator::default();
        g.generate(options(24)).unwrap();
        let a = g.reveal().unwrap().expose().to_owned();
        g.generate(options(24)).unwrap();
        let b = g.reveal().unwrap().expose().to_owned();
        assert_ne!(a, b);
        g.discard();
        assert_eq!(g.reveal().err(), Some(AppError::NoGeneratedPassword));
    }
}
