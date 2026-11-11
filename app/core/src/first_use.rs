//! First use (Phase 30): master password and PIN of the new vault.
//!
//! The interface sends the master password, its confirmation, the PIN and the PIN
//! confirmation. Everything arrives as [`Typed`] (zeroed on drop, `Debug` without the
//! value, a `Deserialize` that takes ownership of the read text) and is checked here, on
//! the Rust side, before going to the daemon:
//!
//! - master password with [`MIN_PASSWORD_CHARS`] characters or more (design
//!   `Primeiro uso / Senha mestra`) and up to `MAX_SECRET` bytes;
//! - PIN with ASCII digits only, from `MIN_PIN_DIGITS` to `MAX_PIN_DIGITS` (R24);
//! - equal confirmations, compared in constant time.
//!
//! The daemon gets only `criar_cofre { senha, pin }` and stores the Argon2id hash of the
//! PIN in the config section of the body (D30); the PIN is never stored in the clear. The
//! errors come back per field, with fixed text, without any value.

use std::fmt;

use ctutils::CtEq;
use serde::Deserialize;
use serde::de::{self, Deserializer, Visitor};
use ward_core::limits::{MAX_PIN_DIGITS, MAX_SECRET, MIN_PIN_DIGITS};
use zeroize::Zeroizing;

use crate::form::FieldError;

/// Minimum characters of the master password (help text of the design), the same as the
/// daemon's (Phase 39c): counted in code points after NFC.
pub const MIN_PASSWORD_CHARS: usize = ward_core::limits::MIN_MASTER_PASSWORD_CHARS;

pub const PASSWORD_SHORT: &str = "a senha mestra precisa de 12 caracteres ou mais";
pub const PASSWORD_LONG: &str = "a senha mestra passou do tamanho máximo";
pub const PASSWORD_MISMATCH: &str = "as duas senhas não são iguais";
pub const PIN_INVALID: &str = "o PIN tem de ter de 4 a 6 dígitos, só números";
pub const PIN_MISMATCH: &str = "os dois PINs não são iguais";

/// Text typed in the interface (master password or PIN).
pub struct Typed(Zeroizing<String>);

impl Typed {
    pub fn new(text: String) -> Self {
        Typed(Zeroizing::new(text))
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// The daemon's rule for a new master password (12 characters after NFC), on a copy
    /// that is zeroed on drop. The daemon checks again; here it is for the field error.
    pub(crate) fn valid_as_new(&self) -> bool {
        ward_core::format::MasterPassword::typed(Zeroizing::new(self.bytes().to_vec()))
            .is_valid_new()
    }

    pub(crate) fn equals(&self, other: &Typed) -> bool {
        // The size is not a secret here (both came from the same person, on the same
        // screen).
        self.bytes().len() == other.bytes().len() && self.bytes().ct_eq(other.bytes()).to_bool()
    }
}

impl fmt::Debug for Typed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Typed(••••)")
    }
}

impl<'de> Deserialize<'de> for Typed {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = Typed;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("texto")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Typed, E> {
                Ok(Typed::new(v.to_owned()))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Typed, E> {
                // Takes ownership of the text already read: no copy left without zeroing.
                Ok(Typed::new(v))
            }
        }
        d.deserialize_string(V)
    }
}

/// What the `Primeiro uso` screen sends. An extra field is refused.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewVault {
    #[serde(rename = "senha")]
    pub password: Typed,
    #[serde(rename = "confirmacao")]
    pub confirmation: Typed,
    pub pin: Typed,
    #[serde(rename = "confirmacao_pin")]
    pub pin_confirmation: Typed,
}

fn field_error(field: &'static str, reason: &str) -> FieldError {
    FieldError {
        field,
        position: None,
        reason: reason.to_owned(),
    }
}

/// Errors of the master password (step 2), without looking at the PIN.
pub fn password_errors(password: &Typed, confirmation: &Typed) -> Vec<FieldError> {
    let mut errors = Vec::new();
    if password.bytes().len() > MAX_SECRET {
        errors.push(field_error("senha", PASSWORD_LONG));
    } else if !password.valid_as_new() {
        errors.push(field_error("senha", PASSWORD_SHORT));
    } else if !password.equals(confirmation) {
        errors.push(field_error("confirmacao", PASSWORD_MISMATCH));
    }
    errors
}

/// PIN of 4 to 6 ASCII digits (the same rule as the protocol, R24).
pub fn valid_pin(pin: &Typed) -> bool {
    let b = pin.bytes();
    (MIN_PIN_DIGITS..=MAX_PIN_DIGITS).contains(&b.len()) && b.iter().all(u8::is_ascii_digit)
}

impl NewVault {
    /// All the errors per field; empty if it may go to the daemon.
    pub fn errors(&self) -> Vec<FieldError> {
        let mut errors = password_errors(&self.password, &self.confirmation);
        if !valid_pin(&self.pin) {
            errors.push(field_error("pin", PIN_INVALID));
        } else if !self.pin.equals(&self.pin_confirmation) {
            errors.push(field_error("confirmacao_pin", PIN_MISMATCH));
        }
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Typed {
        Typed::new(s.to_owned())
    }

    fn new_vault(password: &str, conf: &str, pin: &str, conf_pin: &str) -> NewVault {
        NewVault {
            password: d(password),
            confirmation: d(conf),
            pin: d(pin),
            pin_confirmation: d(conf_pin),
        }
    }

    fn fields(n: &NewVault) -> Vec<&'static str> {
        n.errors().iter().map(|e| e.field).collect()
    }

    const PASSWORD: &str = "falso-mestra-ward-01";

    #[test]
    fn valid_password_and_pin_pass() {
        assert!(fields(&new_vault(PASSWORD, PASSWORD, "135790", "135790")).is_empty());
        assert!(fields(&new_vault(PASSWORD, PASSWORD, "1357", "1357")).is_empty());
        // 12 characters with accents count as characters, not bytes.
        assert!(fields(&new_vault("ááááááááááá1", "ááááááááááá1", "0000", "0000")).is_empty());
    }

    #[test]
    fn attack_pin_with_3_or_7_digits_or_letters_is_refused() {
        for pin in [
            "",
            "135",
            "1357901",
            "13579a",
            "abcd",
            "１３５７",
            "135 79",
            "-1357",
            "1357\n",
            "13\u{0}57",
            "+1357",
        ] {
            assert_eq!(
                fields(&new_vault(PASSWORD, PASSWORD, pin, pin)),
                vec!["pin"],
                "{pin:?}"
            );
        }
    }

    #[test]
    fn attack_different_confirmation_is_refused() {
        assert_eq!(
            fields(&new_vault(PASSWORD, "falso-mestra-ward-02", "135790", "135790")),
            vec!["confirmacao"]
        );
        assert_eq!(
            fields(&new_vault(PASSWORD, PASSWORD, "135790", "135791")),
            vec!["confirmacao_pin"]
        );
        // A prefix of the other is not equal.
        assert_eq!(
            fields(&new_vault(PASSWORD, PASSWORD, "135790", "1357")),
            vec!["confirmacao_pin"]
        );
    }

    #[test]
    fn attack_password_too_short_or_too_large_is_refused() {
        assert_eq!(
            fields(&new_vault("curta-11chr", "curta-11chr", "1357", "1357")),
            vec!["senha"]
        );
        assert_eq!(fields(&new_vault("", "", "1357", "1357")), vec!["senha"]);
        let large = "a".repeat(MAX_SECRET + 1);
        let e = new_vault(&large, &large, "1357", "1357").errors();
        assert_eq!(e[0].reason, PASSWORD_LONG);
    }

    #[test]
    fn attack_errors_and_debug_do_not_show_value() {
        let n = new_vault(PASSWORD, "falso-mestra-ward-02", "13579a", "13579a");
        let text = format!("{n:?} {:?}", n.errors());
        for needle in ["falso", "mestra", "13579"] {
            assert!(!text.contains(needle), "{text}");
        }
    }

    #[test]
    fn attack_extra_field_or_wrong_type_is_refused() {
        let ok = r#"{"senha":"x","confirmacao":"x","pin":"1","confirmacao_pin":"1"}"#;
        assert!(serde_json::from_str::<NewVault>(ok).is_ok());
        for bad in [
            r#"{"senha":"x","confirmacao":"x","pin":"1","confirmacao_pin":"1","nivel":"livre"}"#,
            r#"{"senha":"x","confirmacao":"x","pin":1357,"confirmacao_pin":"1"}"#,
            r#"{"senha":"x","pin":"1","confirmacao_pin":"1"}"#,
            r#"{"senha":["x"],"confirmacao":"x","pin":"1","confirmacao_pin":"1"}"#,
        ] {
            assert!(serde_json::from_str::<NewVault>(bad).is_err(), "{bad}");
        }
    }

    /// The new Rust names are not wire names (frozen until m2:Phase 54): the webview
    /// still sends the old keys, and the new ones are refused as unknown fields.
    #[test]
    fn attack_new_rust_names_are_not_wire_names() {
        let new = r#"{"password":"x","confirmation":"x","pin":"1","pin_confirmation":"1"}"#;
        assert!(serde_json::from_str::<NewVault>(new).is_err());
    }

    mod through_service {
        use std::sync::{Arc, Mutex};

        use zeroize::Zeroizing;

        use super::*;
        use crate::AppError;
        use crate::client;
        use crate::clipboard::{Clipboard, Copier};
        use crate::daemon::{Daemon, VaultStatus};
        use crate::fake_daemon::FakeDaemon;
        use crate::service::Service;

        type Frames = Arc<Mutex<Vec<Vec<u8>>>>;

        struct NoClipboard;
        impl Clipboard for NoClipboard {
            fn write(&self, _: &[u8]) -> Result<(), AppError> {
                Ok(())
            }
            fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>, AppError> {
                Ok(None)
            }
            fn clear(&self) -> Result<(), AppError> {
                Ok(())
            }
        }

        /// Fake daemon that keeps every frame received (the "daemon" is the one looking).
        struct Spy(FakeDaemon, Frames);
        impl Daemon for Spy {
            fn exchange(&mut self, request: &[u8]) -> Result<Zeroizing<Vec<u8>>, AppError> {
                self.1.lock().unwrap().push(request.to_vec());
                self.0.exchange(request)
            }
        }

        fn service() -> (Service<Spy, NoClipboard>, Frames) {
            let frames = Arc::new(Mutex::new(Vec::new()));
            let d = Spy(FakeDaemon::no_vault(), Arc::clone(&frames));
            (Service::new(d, Copier::new(NoClipboard)), frames)
        }

        #[test]
        fn creates_the_vault_and_stays_unlocked() {
            let (s, frames) = service();
            assert_eq!(s.vault_status(), Ok(VaultStatus::NoVault));
            let r = s
                .create_vault(new_vault(PASSWORD, PASSWORD, "135790", "135790"))
                .unwrap();
            assert!(r.created && r.errors.is_empty());
            assert_eq!(s.vault_status(), Ok(VaultStatus::Unlocked));
            assert!(s.list_credentials().unwrap().is_empty());
            // Only password and PIN go to the daemon; the confirmations stay in the app.
            let q = frames.lock().unwrap();
            let create: Vec<_> = q
                .iter()
                .filter(|q| String::from_utf8_lossy(q).contains("\"criar_cofre\""))
                .collect();
            assert_eq!(create.len(), 1);
            let t = String::from_utf8_lossy(create[0]);
            assert!(!t.contains("confirmacao"), "{t}");
        }

        #[test]
        fn daemon_keeps_only_the_pin_hash() {
            let mut d = FakeDaemon::no_vault();
            client::create_vault(&mut d, PASSWORD.as_bytes(), b"135790").unwrap();
            assert!(d.pin_matches(b"135790"));
            assert!(!d.pin_matches(b"135791"));
        }

        #[test]
        fn attack_invalid_pin_or_confirmation_do_not_reach_the_daemon() {
            let (s, frames) = service();
            for n in [
                new_vault(PASSWORD, PASSWORD, "135", "135"),
                new_vault(PASSWORD, PASSWORD, "1357901", "1357901"),
                new_vault(PASSWORD, PASSWORD, "13579a", "13579a"),
                new_vault(PASSWORD, PASSWORD, "135790", "135791"),
                new_vault(PASSWORD, "outra-senha-qualquer", "135790", "135790"),
                new_vault("curta", "curta", "135790", "135790"),
            ] {
                let r = s.create_vault(n).unwrap();
                assert!(!r.created && !r.errors.is_empty());
            }
            assert!(frames.lock().unwrap().is_empty());
            assert_eq!(s.vault_status(), Ok(VaultStatus::NoVault));
        }

        #[test]
        fn attack_creating_over_an_existing_vault_is_refused() {
            let (s, _) = service();
            assert!(
                s.create_vault(new_vault(PASSWORD, PASSWORD, "135790", "135790"))
                    .unwrap()
                    .created
            );
            assert_eq!(
                s.create_vault(new_vault(PASSWORD, PASSWORD, "2468", "2468")),
                Err(AppError::VaultAlreadyExists)
            );
            let full = Service::new(FakeDaemon::full(), Copier::new(NoClipboard));
            assert_eq!(
                full.create_vault(new_vault(PASSWORD, PASSWORD, "2468", "2468")),
                Err(AppError::VaultAlreadyExists)
            );
        }

        #[test]
        fn without_a_vault_the_rest_is_refused() {
            let (s, _) = service();
            assert!(s.list_credentials().is_err());
            assert!(s.credential_detail("acme-pg-dev").is_err());
        }
    }
}
