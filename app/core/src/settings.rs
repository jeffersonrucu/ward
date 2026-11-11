//! Settings screen (Phase 38, V5.22; D30, D39, S9).
//!
//! - [`PinChange`]: current PIN, new PIN and the confirmation. The daemon checks the
//!   current PIN (change PIN operation; the error count and the lock on the 5th are its own) and
//!   stores the Argon2id hash of the new one in the vault config, with a new generation
//!   (D30, D39).
//! - [`PasswordChange`]: current master password, new one and the confirmation. The new
//!   one follows the same rule as the first use
//!   ([`crate::first_use::password_errors`]: 12 characters or more, up to `MAX_SECRET`
//!   bytes); the current one only needs to exist and fit the limit (the daemon checks
//!   it, change password operation).
//! - Lock timeout: from `MIN_LOCK_MINUTES` to `MAX_LOCK_MINUTES` (D45), stored in the
//!   vault config by the daemon (lock timeout operation).
//! - [`Settings`]: what the screen shows (lock timeout and external backup on or off),
//!   read by the config operation; never the PIN hash nor the UUID.
//!
//! Everything typed arrives as [`Typed`] (zeroes on drop, `Debug` without the value)
//! and is checked here before going to the daemon. Errors per field, with fixed text, no
//! value.

use serde::{Deserialize, Serialize};
use ward_core::limits::{MAX_LOCK_MINUTES, MAX_SECRET, MIN_LOCK_MINUTES};

use crate::AppError;
use crate::first_use::{PIN_INVALID, PIN_MISMATCH, Typed, password_errors, valid_pin};
use crate::form::FieldError;

pub const CURRENT_PIN_INVALID: &str = "o PIN atual tem de 4 a 6 dígitos, só números";
pub const CURRENT_PASSWORD_EMPTY: &str = "digite a senha mestra atual";

/// What the Settings screen shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Settings {
    #[serde(rename = "minutos_trava")]
    pub lock_minutes: u32,
    #[serde(rename = "backup_externo")]
    pub external_backup: bool,
    /// The local backup folder opens and is safe (Phase 49a); `false`: the vault is
    /// written without backup and the "Backup local" row warns.
    #[serde(rename = "backup_local")]
    pub local_backup: bool,
}

/// Response of the PIN and password changes: changed, or the errors per field. No value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangeResult {
    #[serde(rename = "trocado")]
    pub changed: bool,
    #[serde(rename = "erros")]
    pub errors: Vec<FieldError>,
}

impl ChangeResult {
    pub(crate) fn with_errors(errors: Vec<FieldError>) -> Self {
        ChangeResult {
            changed: false,
            errors,
        }
    }

    pub(crate) fn changed() -> Self {
        ChangeResult {
            changed: true,
            errors: Vec::new(),
        }
    }
}

fn field_error(field: &'static str, reason: &str) -> FieldError {
    FieldError {
        field,
        position: None,
        reason: reason.to_owned(),
    }
}

/// "Change PIN": the current one and the new one twice. An extra field is refused.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinChange {
    #[serde(rename = "atual")]
    pub current: Typed,
    #[serde(rename = "novo")]
    pub new: Typed,
    #[serde(rename = "confirmacao")]
    pub confirmation: Typed,
}

impl PinChange {
    /// All the errors per field; empty if it may go to the daemon.
    pub fn errors(&self) -> Vec<FieldError> {
        let mut errors = Vec::new();
        if !valid_pin(&self.current) {
            errors.push(field_error("atual", CURRENT_PIN_INVALID));
        }
        if !valid_pin(&self.new) {
            errors.push(field_error("novo", PIN_INVALID));
        } else if !self.new.equals(&self.confirmation) {
            errors.push(field_error("confirmacao", PIN_MISMATCH));
        }
        errors
    }
}

/// "Change master password": the current one and the new one twice. An extra field is
/// refused.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PasswordChange {
    #[serde(rename = "atual")]
    pub current: Typed,
    #[serde(rename = "nova")]
    pub new: Typed,
    #[serde(rename = "confirmacao")]
    pub confirmation: Typed,
}

impl PasswordChange {
    /// All the errors per field; empty if it may go to the daemon. The new one follows
    /// the first use rule (the same error text).
    pub fn errors(&self) -> Vec<FieldError> {
        let mut errors = Vec::new();
        let current = self.current.bytes();
        if current.is_empty() || current.len() > MAX_SECRET {
            errors.push(field_error("atual", CURRENT_PASSWORD_EMPTY));
        }
        for mut e in password_errors(&self.new, &self.confirmation) {
            if e.field == "senha" {
                e.field = "nova";
            }
            errors.push(e);
        }
        errors
    }
}

/// Lock minutes coming from the interface (untrusted), in the D45 range.
pub fn valid_minutes(minutes: u64) -> Result<u32, AppError> {
    u32::try_from(minutes)
        .ok()
        .filter(|m| (MIN_LOCK_MINUTES..=MAX_LOCK_MINUTES).contains(m))
        .ok_or(AppError::InvalidRequest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Typed {
        Typed::new(s.to_owned())
    }

    fn pin(current: &str, new: &str, conf: &str) -> PinChange {
        PinChange {
            current: d(current),
            new: d(new),
            confirmation: d(conf),
        }
    }

    fn password(current: &str, new: &str, conf: &str) -> PasswordChange {
        PasswordChange {
            current: d(current),
            new: d(new),
            confirmation: d(conf),
        }
    }

    fn fields(e: Vec<FieldError>) -> Vec<&'static str> {
        e.iter().map(|e| e.field).collect()
    }

    const PASSWORD: &str = "falso-mestra-ward-01";
    const NEW: &str = "falso-nova-mestra-02";

    #[test]
    fn valid_changes_pass() {
        assert!(pin("135790", "2468", "2468").errors().is_empty());
        assert!(password(PASSWORD, NEW, NEW).errors().is_empty());
        // The current one does not follow the minimum of 12 (the daemon checks it, not
        // the new rule).
        assert!(password("curta", NEW, NEW).errors().is_empty());
    }

    #[test]
    fn attack_current_or_new_pin_out_of_format_is_refused() {
        for bad in [
            "",
            "135",
            "1357901",
            "13579a",
            "１３５７",
            "13 57",
            "1357\n",
            "+1357",
        ] {
            assert_eq!(
                fields(pin(bad, "2468", "2468").errors()),
                ["atual"],
                "{bad:?}"
            );
            assert_eq!(
                fields(pin("135790", bad, bad).errors()),
                ["novo"],
                "{bad:?}"
            );
        }
        assert_eq!(
            fields(pin("135790", "2468", "2469").errors()),
            ["confirmacao"]
        );
        assert_eq!(
            fields(pin("135790", "2468", "24680").errors()),
            ["confirmacao"]
        );
    }

    #[test]
    fn attack_short_or_different_new_password_and_empty_current_refused() {
        assert_eq!(
            fields(password(PASSWORD, "curta-11chr", "curta-11chr").errors()),
            ["nova"]
        );
        assert_eq!(
            fields(password(PASSWORD, NEW, PASSWORD).errors()),
            ["confirmacao"]
        );
        assert_eq!(fields(password("", NEW, NEW).errors()), ["atual"]);
        let big = "a".repeat(MAX_SECRET + 1);
        assert_eq!(fields(password(&big, NEW, NEW).errors()), ["atual"]);
        assert_eq!(fields(password(PASSWORD, &big, &big).errors()), ["nova"]);
    }

    #[test]
    fn attack_errors_and_debug_do_not_show_value() {
        let p = pin("135790", "13579a", "13579a");
        let s = password(PASSWORD, NEW, "falso-outra-mestra-03");
        let text = format!("{p:?} {:?} {s:?} {:?}", p.errors(), s.errors());
        for needle in ["falso", "mestra", "13579", "2468"] {
            assert!(!text.contains(needle), "{text}");
        }
    }

    #[test]
    fn attack_extra_field_or_wrong_type_is_refused() {
        let ok = r#"{"atual":"1","novo":"2","confirmacao":"2"}"#;
        assert!(serde_json::from_str::<PinChange>(ok).is_ok());
        for bad in [
            r#"{"atual":"1","novo":"2","confirmacao":"2","pin":"3"}"#,
            r#"{"atual":1357,"novo":"2","confirmacao":"2"}"#,
            r#"{"novo":"2","confirmacao":"2"}"#,
        ] {
            assert!(serde_json::from_str::<PinChange>(bad).is_err(), "{bad}");
        }
        let ok = r#"{"atual":"1","nova":"2","confirmacao":"2"}"#;
        assert!(serde_json::from_str::<PasswordChange>(ok).is_ok());
        for bad in [
            r#"{"atual":"1","nova":"2","confirmacao":"2","geracao":0}"#,
            r#"{"atual":"1","nova":["2"],"confirmacao":"2"}"#,
            r#"{"atual":"1","confirmacao":"2"}"#,
        ] {
            assert!(
                serde_json::from_str::<PasswordChange>(bad).is_err(),
                "{bad}"
            );
        }
    }

    /// m2:DT3: the new Rust names are not wire names. The webview keeps sending and
    /// receiving the old fields until Phase 54.
    #[test]
    fn attack_new_rust_names_are_not_wire_names() {
        for bad in [
            r#"{"current":"1","new":"2","confirmation":"2"}"#,
            r#"{"current":"1","novo":"2","confirmacao":"2"}"#,
        ] {
            assert!(serde_json::from_str::<PinChange>(bad).is_err(), "{bad}");
            assert!(
                serde_json::from_str::<PasswordChange>(bad).is_err(),
                "{bad}"
            );
        }
        let s = Settings {
            lock_minutes: 30,
            external_backup: true,
            local_backup: false,
        };
        assert_eq!(
            serde_json::to_string(&s).unwrap(),
            r#"{"minutos_trava":30,"backup_externo":true,"backup_local":false}"#
        );
        assert_eq!(
            serde_json::to_string(&ChangeResult::changed()).unwrap(),
            r#"{"trocado":true,"erros":[]}"#
        );
    }

    #[test]
    fn attack_minutes_out_of_range() {
        for m in [
            0,
            u64::from(MAX_LOCK_MINUTES) + 1,
            u64::from(u32::MAX) + 1,
            u64::MAX,
        ] {
            assert_eq!(valid_minutes(m), Err(AppError::InvalidRequest), "{m}");
        }
        assert_eq!(valid_minutes(1), Ok(1));
        assert_eq!(valid_minutes(240), Ok(240));
        assert_eq!(valid_minutes(1440), Ok(1440));
    }

    mod via_service {
        use std::sync::{Arc, Mutex};

        use zeroize::Zeroizing;

        use super::*;
        use crate::clipboard::{Clipboard, Copier};
        use crate::daemon::{Daemon, VaultStatus};
        use crate::fake_daemon::{FakeDaemon, SECRETS};
        use crate::service::Service;
        use crate::unlock::MasterPassword;

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
            let d = Spy(FakeDaemon::full().without_wait(), Arc::clone(&frames));
            (Service::new(d, Copier::new(NoClipboard)), frames)
        }

        fn no_value(json: &str) {
            for needle in ["135790", "2468", "falso", "argon2", "$m="]
                .iter()
                .copied()
                .chain(SECRETS.iter().map(|s| &s[..s.len().min(12)]))
            {
                assert!(!json.contains(needle), "{needle} em {json}");
            }
        }

        fn master_password(s: &str) -> MasterPassword {
            MasterPassword::new(s.to_owned()).unwrap()
        }

        #[test]
        fn v5_22_default_4h_off_and_timeout_changeable() {
            let (s, _) = service();
            let c = s.settings().unwrap();
            assert_eq!((c.lock_minutes, c.external_backup), (240, false));
            let c = s.set_lock_timeout(30).unwrap();
            assert_eq!(c.lock_minutes, 30);
            assert_eq!(s.settings().unwrap().lock_minutes, 30);
            no_value(&serde_json::to_string(&c).unwrap());
        }

        #[test]
        fn attack_timeout_out_of_range_does_not_even_reach_the_daemon() {
            let (s, frames) = service();
            for m in [0, 1441, u64::MAX] {
                assert_eq!(s.set_lock_timeout(m), Err(AppError::InvalidRequest));
            }
            assert!(frames.lock().unwrap().is_empty());
        }

        #[test]
        fn v5_22_change_pin_asks_for_the_current_pin() {
            let (s, frames) = service();
            assert_eq!(
                s.change_pin(pin("246801", "2468", "2468")),
                Err(AppError::WrongPin)
            );
            let r = s.change_pin(pin("135790", "2468", "2468")).unwrap();
            assert!(r.changed && r.errors.is_empty());
            no_value(&serde_json::to_string(&r).unwrap());
            // From then on only the new PIN approves.
            let set = |p: &str| {
                s.set_level(
                    "acme-pg-dev",
                    "blocked",
                    crate::level::Pin::new(p.into()).unwrap(),
                )
            };
            assert_eq!(set("135790"), Err(AppError::WrongPin));
            assert!(set("2468").is_ok());
            // The frame carries only the current and the new PIN (the confirmation stays in
            // the app).
            let q = frames.lock().unwrap();
            let changes: Vec<_> = q
                .iter()
                .map(|q| String::from_utf8_lossy(q).into_owned())
                .filter(|q| q.contains("\"change_pin\""))
                .collect();
            assert_eq!(changes.len(), 2);
            assert!(changes.iter().all(|t| !t.contains("confirmacao")));
        }

        #[test]
        fn attack_pin_or_password_outside_the_rule_do_not_reach_the_daemon() {
            let (s, frames) = service();
            for t in [
                pin("135", "2468", "2468"),
                pin("135790", "13579a", "13579a"),
                pin("135790", "2468", "2469"),
            ] {
                let r = s.change_pin(t).unwrap();
                assert!(!r.changed && !r.errors.is_empty());
            }
            for t in [
                password("", NEW, NEW),
                password(PASSWORD, "curta", "curta"),
                password(PASSWORD, NEW, "falso-outra-mestra-03"),
            ] {
                let r = s.change_master_password(t).unwrap();
                assert!(!r.changed && !r.errors.is_empty());
                no_value(&serde_json::to_string(&r).unwrap());
            }
            assert!(frames.lock().unwrap().is_empty());
        }

        #[test]
        fn attack_guessing_the_pin_through_the_change_locks_on_the_fifth_error() {
            let (s, _) = service();
            for i in 0..5 {
                assert_eq!(
                    s.change_pin(pin(&format!("00{i:02}"), "2468", "2468")),
                    Err(AppError::WrongPin)
                );
            }
            assert_eq!(s.vault_status(), Ok(VaultStatus::Locked));
            assert_eq!(
                s.change_pin(pin("135790", "2468", "2468")),
                Err(AppError::Locked)
            );
            assert_eq!(s.settings(), Err(AppError::Locked));
        }

        #[test]
        fn v5_22_change_password_asks_for_the_current_one_and_the_new_one_unlocks() {
            let (s, _) = service();
            assert_eq!(
                s.change_master_password(password("falso-errada-mestra-09", NEW, NEW)),
                Err(AppError::WrongPassword)
            );
            assert_eq!(s.vault_status(), Ok(VaultStatus::Unlocked));
            let r = s
                .change_master_password(password(PASSWORD, NEW, NEW))
                .unwrap();
            assert!(r.changed);
            // Locks (5 wrong PINs) and checks which password unlocks.
            for _ in 0..5 {
                let _ = s.change_pin(pin("000000", "2468", "2468"));
            }
            assert_eq!(s.vault_status(), Ok(VaultStatus::Locked));
            assert_eq!(
                s.unlock(master_password(PASSWORD)),
                Err(AppError::WrongPassword)
            );
            assert!(s.unlock(master_password(NEW)).is_ok());
        }
    }
}
