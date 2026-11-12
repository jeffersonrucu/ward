//! Change the access level of an entry (Phase 35, S9).
//!
//! The PIN arrives from the interface (untrusted) and becomes a [`Pin`]: zeroed on drop,
//! no `Clone`, `Display` or `Serialize`, and the `Debug` does not show the value. Reading
//! it (`Deserialize`) already refuses what is not a PIN (4 to 6 ASCII digits, like
//! `ward_core::limits`), so a value out of format never reaches the daemon. The PIN only
//! leaves here for the protocol's set level request; no response returns it and no
//! log writes it.

use std::fmt;

use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize};
use ward_core::entry::{AccessLevel, Secret as CoreSecret};
use ward_core::limits::{MAX_PIN_DIGITS, MIN_PIN_DIGITS};
use zeroize::Zeroizing;

use crate::AppError;

/// PIN typed in the interface, only to confirm the level change.
pub struct Pin(Zeroizing<String>);

impl Pin {
    /// Checks the format and takes ownership of the text (no copy left without zeroing).
    pub fn new(text: String) -> Result<Pin, AppError> {
        let text = Zeroizing::new(text);
        if format_ok(&text) {
            Ok(Pin(text))
        } else {
            Err(AppError::InvalidRequest)
        }
    }

    /// The PIN in the protocol type (also zeroed on drop).
    pub(crate) fn to_protocol(&self) -> CoreSecret {
        CoreSecret::from_bytes(self.0.as_bytes().to_vec())
    }
}

fn format_ok(t: &str) -> bool {
    (MIN_PIN_DIGITS..=MAX_PIN_DIGITS).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_digit())
}

impl fmt::Debug for Pin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Pin(••••)")
    }
}

impl<'de> Deserialize<'de> for Pin {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = Pin;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("PIN de 4 a 6 dígitos")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Pin, E> {
                self.visit_string(v.to_owned())
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Pin, E> {
                // The error message is fixed: it does not echo what came in.
                Pin::new(v).map_err(|_| E::custom("PIN fora do formato"))
            }
            // A number instead of the text: serde's default error would echo the value.
            fn visit_u64<E: de::Error>(self, _: u64) -> Result<Pin, E> {
                Err(E::custom("PIN fora do formato"))
            }
            fn visit_i64<E: de::Error>(self, _: i64) -> Result<Pin, E> {
                Err(E::custom("PIN fora do formato"))
            }
            fn visit_f64<E: de::Error>(self, _: f64) -> Result<Pin, E> {
                Err(E::custom("PIN fora do formato"))
            }
        }
        // `any`: a number falls into `visit_u64` (fixed error), not into the default
        // error with the value.
        d.deserialize_any(V)
    }
}

/// Level requested by the interface, by one of the three protocol names (free, with
/// approval, blocked).
pub fn level_from_name(name: &str) -> Result<AccessLevel, AppError> {
    AccessLevel::from_name(name).ok_or(AppError::InvalidRequest)
}

/// Response of the set level command: the level the entry now has. No PIN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LevelChanged {
    pub entry: String,
    pub level: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attack_pin_out_of_format_is_refused_without_echo() {
        for bad in [
            "",
            "123",
            "1234567",
            "13579a",
            " 135790",
            "135790\n",
            "１３５７９０",
            "13579\u{0}",
            "-1357",
            "+13579",
        ] {
            assert_eq!(
                Pin::new(bad.to_owned()).unwrap_err(),
                AppError::InvalidRequest,
                "{bad:?}"
            );
            let json = serde_json::to_string(bad).unwrap();
            let e = serde_json::from_str::<Pin>(&json).unwrap_err().to_string();
            assert!(e.contains("PIN fora do formato"), "{e}");
            assert!(bad.len() < 4 || !e.contains(bad), "erro ecoa o valor: {e}");
        }
        // Number instead of text, object, null: refused.
        for json in ["135790", "{}", "null", "[\"135790\"]"] {
            let e = serde_json::from_str::<Pin>(json).unwrap_err().to_string();
            assert!(!e.contains("135790"), "{e}");
        }
        assert!(serde_json::from_str::<Pin>("\"1357\"").is_ok());
        assert!(serde_json::from_str::<Pin>("\"135790\"").is_ok());
    }

    #[test]
    fn attack_pin_debug_does_not_show_the_value() {
        let p = Pin::new("135790".to_owned()).unwrap();
        let text = format!("{p:?} {:?}", Some(&p));
        assert!(!text.contains("135790"));
        assert!(text.contains("••••"));
    }

    #[test]
    fn level_only_by_the_three_protocol_names() {
        assert_eq!(level_from_name("free"), Ok(AccessLevel::Free));
        assert_eq!(level_from_name("approval"), Ok(AccessLevel::Approval));
        assert_eq!(level_from_name("blocked"), Ok(AccessLevel::Blocked));
        // m2:DT4: the vault v4 names (0.1.x interface) are refused too.
        for bad in [
            "",
            "Free",
            "com aprovação",
            "with_approval",
            "admin",
            "free ",
            "livre",
            "com-aprovacao",
            "bloqueado",
        ] {
            assert_eq!(
                level_from_name(bad),
                Err(AppError::InvalidRequest),
                "{bad:?}"
            );
        }
    }

    /// m2:V4.23 (m2:Phase 54): the webview receives the English keys.
    #[test]
    fn webview_keys_are_the_english_ones() {
        let v = serde_json::to_value(LevelChanged {
            entry: "acme-pg-dev".into(),
            level: "free",
        })
        .unwrap();
        assert_eq!(
            v,
            serde_json::json!({"entry": "acme-pg-dev", "level": "free"})
        );
    }
}
