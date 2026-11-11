//! Unlock through the app (Phase 31; D16, D20, D62, D65 a).
//!
//! - [`MasterPassword`]: the password that comes from the interface (untrusted). Zeroed
//!   on drop, no `Clone`, `Display` or `Serialize`, and the `Debug` does not show the
//!   value. Reading it (`Deserialize`) refuses empty and above
//!   `ward_core::limits::MAX_SECRET` with a fixed message, without echoing what came in.
//!   The password only leaves here for the protocol's unlock request; no response
//!   returns it and no log writes it.
//! - [`PendingRequest`]: the unlock request notice the daemon sent (an agent use waits for
//!   the unlock, D16). The notice may arrive before the interface exists (the daemon
//!   opens the app because of it and delivers it as soon as the connection subscribes),
//!   so the Rust side keeps the deadline and the screen reads it in the vault status reply.
//!   Phase 39a: the notice carries the entry the agent asked for, the operation, the exact
//!   command and the seconds left by the daemon's clock (at most [`REQUEST_TIMEOUT`]).
//!   Entry and command come from the agent: the daemon already formats them with a
//!   visible escape (D51) and here they go through `neutralize` again before reaching the
//!   screen, which shows them as text, never as HTML.
//! - [`watch`]: the thread that takes the unlock request notices out of the
//!   connection's notice queue (D65 a), marks the [`PendingRequest`] and tells the
//!   interface (in the app, through the Tauri channel; in the test bridge, the screen
//!   polls the status).

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde::de::{self, Deserialize, Deserializer, Visitor};
use ward_core::display::neutralize;
use ward_core::entry::Secret as CoreSecret;
use ward_core::limits::MAX_SECRET;
use ward_core::protocol::UnlockRequest;
use zeroize::Zeroizing;

use crate::AppError;
use crate::daemon::NoticeQueue;

/// How long the agent's request waits for the unlock (D16; the same timeout as the
/// `wardd` one).
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// Fixed message of a refused read (does not echo the value).
const OUT_OF_FORMAT: &str = "senha mestra fora do formato";

/// Master password typed on the unlock screen.
pub struct MasterPassword(Zeroizing<String>);

impl MasterPassword {
    /// Checks the size and takes ownership of the text (no copy left without zeroing).
    pub fn new(text: String) -> Result<MasterPassword, AppError> {
        let text = Zeroizing::new(text);
        if !text.is_empty() && text.len() <= MAX_SECRET {
            Ok(MasterPassword(text))
        } else {
            Err(AppError::InvalidRequest)
        }
    }

    /// The password in the protocol type (also zeroed on drop).
    pub(crate) fn to_protocol(&self) -> CoreSecret {
        CoreSecret::from_bytes(self.0.as_bytes().to_vec())
    }
}

impl fmt::Debug for MasterPassword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MasterPassword(••••)")
    }
}

impl<'de> Deserialize<'de> for MasterPassword {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = MasterPassword;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("senha mestra")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<MasterPassword, E> {
                if v.len() > MAX_SECRET {
                    return Err(E::custom(OUT_OF_FORMAT));
                }
                self.visit_string(v.to_owned())
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<MasterPassword, E> {
                MasterPassword::new(v).map_err(|_| E::custom(OUT_OF_FORMAT))
            }
            // Number, boolean or null instead of the text: serde's default error would
            // echo the value.
            fn visit_u64<E: de::Error>(self, _: u64) -> Result<MasterPassword, E> {
                Err(E::custom(OUT_OF_FORMAT))
            }
            fn visit_i64<E: de::Error>(self, _: i64) -> Result<MasterPassword, E> {
                Err(E::custom(OUT_OF_FORMAT))
            }
            fn visit_f64<E: de::Error>(self, _: f64) -> Result<MasterPassword, E> {
                Err(E::custom(OUT_OF_FORMAT))
            }
            fn visit_bool<E: de::Error>(self, _: bool) -> Result<MasterPassword, E> {
                Err(E::custom(OUT_OF_FORMAT))
            }
            fn visit_unit<E: de::Error>(self) -> Result<MasterPassword, E> {
                Err(E::custom(OUT_OF_FORMAT))
            }
        }
        d.deserialize_any(V)
    }
}

/// What the screen gets about the agent's request: a number that changes on each notice
/// (so the screen's "not now" applies only to that one), the seconds left and, with a visible escape
/// (D51), the requested entry, the operation and the exact command (Phase 39a). No type
/// or environment: with the vault locked the daemon does not know them. No secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RequestInfo {
    pub id: u64,
    #[serde(rename = "restante")]
    pub remaining: u64,
    #[serde(rename = "entrada")]
    pub entry: String,
    #[serde(rename = "operacao")]
    pub operation: &'static str,
    #[serde(rename = "comando")]
    pub command: String,
}

/// The request text as the screen shows it (D51).
#[derive(Default)]
struct Text {
    entry: String,
    operation: &'static str,
    command: String,
}

#[derive(Default)]
struct State {
    id: u64,
    deadline: Option<Instant>,
    text: Text,
}

/// The unlock request that is waiting, if there is one.
#[derive(Default)]
pub struct PendingRequest {
    state: Mutex<State>,
}

impl PendingRequest {
    fn lock_state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// An unlock request notice arrived now: the shown request becomes this one (the
    /// newest), with the deadline the daemon sent (never above [`REQUEST_TIMEOUT`]).
    pub fn mark(&self, p: &UnlockRequest) {
        self.mark_at(p, Instant::now());
    }

    fn mark_at(&self, p: &UnlockRequest, now: Instant) {
        let text = Text {
            entry: neutralize(&p.entry),
            operation: p.operation.name(),
            command: neutralize(&p.command),
        };
        let left = Duration::from_secs(p.remaining).min(REQUEST_TIMEOUT);
        let mut e = self.lock_state();
        e.id = e.id.wrapping_add(1);
        e.deadline = Some(now + left);
        e.text = text;
    }

    /// The request still within the deadline, or `None`.
    pub fn info(&self) -> Option<RequestInfo> {
        self.info_at(Instant::now())
    }

    fn info_at(&self, now: Instant) -> Option<RequestInfo> {
        let mut e = self.lock_state();
        let left = e
            .deadline?
            .checked_duration_since(now)
            .filter(|f| !f.is_zero());
        match left {
            // Rounds up: with 0.4 s there is still a request.
            Some(f) => Some(RequestInfo {
                id: e.id,
                remaining: f.as_secs() + u64::from(f.subsec_nanos() > 0),
                entry: e.text.entry.clone(),
                operation: e.text.operation,
                command: e.text.command.clone(),
            }),
            None => {
                e.deadline = None;
                e.text = Text::default();
                None
            }
        }
    }

    /// The vault unlocked: the request goes on by itself in the daemon and the screen no
    /// longer shows it.
    pub fn clear(&self) {
        let mut e = self.lock_state();
        e.deadline = None;
        e.text = Text::default();
    }
}

/// Starts the thread that takes the unlock request notices out of the `queue`
/// (D65 a): the newest marks the `request` and calls `on_arrival` (which gets nothing
/// from the notice: the screen reads the request in the vault status reply). The approval
/// notices stay in the queue for the approval window (Phase 36).
pub fn watch(
    queue: Arc<NoticeQueue>,
    request: Arc<PendingRequest>,
    on_arrival: impl Fn() + Send + 'static,
) -> Result<(), AppError> {
    std::thread::Builder::new()
        .name("ward-app-destrave".into())
        .spawn(move || {
            loop {
                if let Some(p) = queue.wait_unlock(Duration::from_secs(3600)) {
                    request.mark(&p);
                    on_arrival();
                }
            }
        })
        .map(|_| ())
        .map_err(|_| AppError::ConnectionLost)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::Notice;

    #[test]
    fn attack_password_out_of_format_is_refused_without_echo() {
        let long = format!("\"{}\"", "x".repeat(MAX_SECRET + 1));
        for body in [
            "\"\"",
            long.as_str(),
            "135790",
            "-1",
            "1.5",
            "true",
            "null",
            "[\"falso-mestra-ward-01\"]",
            "{\"senha\":\"falso-mestra-ward-01\"}",
        ] {
            let e = serde_json::from_str::<MasterPassword>(body).unwrap_err();
            let text = e.to_string();
            assert!(!text.contains("falso"), "{text}");
            assert!(!text.contains("135790"), "{text}");
            assert!(!text.contains("xxxx"), "{text}");
        }
        let s: MasterPassword = serde_json::from_str("\"falso-mestra-ward-01\"").unwrap();
        assert_eq!(s.to_protocol().expose(), b"falso-mestra-ward-01");
        // With an accent and a space: the password is what came in, not normalized.
        let s: MasterPassword = serde_json::from_str("\" senha é\\u0000x \"").unwrap();
        assert_eq!(s.to_protocol().expose(), " senha é\0x ".as_bytes());
    }

    #[test]
    fn attack_password_debug_does_not_show_the_value() {
        let s = MasterPassword::new("falso-mestra-ward-01".into()).unwrap();
        let d = format!("{s:?}");
        assert!(!d.contains("falso"), "{d}");
        assert_eq!(
            MasterPassword::new(String::new()).unwrap_err(),
            AppError::InvalidRequest
        );
    }

    fn notice(entry: &str, remaining: u64) -> UnlockRequest {
        match Notice::test_unlock(entry) {
            Notice::Unlock(mut p) => {
                p.remaining = remaining;
                p
            }
            Notice::Approval(_) => unreachable!(),
        }
    }

    #[test]
    fn request_counts_from_120_s_restarts_and_disappears() {
        let p = PendingRequest::default();
        assert_eq!(p.info(), None);
        let t0 = Instant::now();
        p.mark_at(&notice("acme-pg-aprovacao", 120), t0);
        let i = p.info_at(t0).unwrap();
        assert_eq!(i.remaining, 120);
        let i = p.info_at(t0 + Duration::from_millis(18_400)).unwrap();
        assert_eq!(i.remaining, 102);
        assert_eq!(p.info_at(t0 + REQUEST_TIMEOUT), None);
        // Expired does not come back.
        assert_eq!(p.info_at(t0), None);
        // A new notice has another id and restarts.
        p.mark_at(
            &notice("acme-pg-aprovacao", 120),
            t0 + Duration::from_secs(10),
        );
        let j = p.info_at(t0 + Duration::from_secs(10)).unwrap();
        assert_ne!(j.id, i.id);
        assert_eq!(j.remaining, 120);
        p.clear();
        assert_eq!(p.info(), None);
    }

    #[test]
    fn watcher_takes_the_unlock_and_marks_the_request() {
        let queue = Arc::new(NoticeQueue::default());
        let request = Arc::new(PendingRequest::default());
        let (tx, rx) = std::sync::mpsc::channel();
        watch(queue.clone(), request.clone(), move || {
            let _ = tx.send(());
        })
        .unwrap();
        queue.simulate(Notice::test_unlock("acme-pg-aprovacao"));
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let i = request.info().unwrap();
        assert_eq!(i.entry, "acme-pg-aprovacao");
        assert_eq!(i.operation, "run");
        assert_eq!(i.command, r#""psql" "-c" "select count(*) from pedidos""#);
        assert_eq!(queue.size(), 0);
    }

    /// Phase 39a (V5.8): the deadline is the one the daemon sent (the app that subscribed
    /// later does not show a full 2 min), never above 2 min; the newest request takes the
    /// place.
    #[test]
    fn deadline_comes_from_the_daemon_and_never_exceeds_2_min() {
        let p = PendingRequest::default();
        let t0 = Instant::now();
        p.mark_at(&notice("acme-pg-aprovacao", 70), t0);
        assert_eq!(p.info_at(t0).unwrap().remaining, 70);
        assert_eq!(p.info_at(t0 + Duration::from_secs(70)), None);
        p.mark_at(&notice("acme-pg-aprovacao", u64::MAX), t0);
        assert_eq!(p.info_at(t0).unwrap().remaining, 120);
        p.mark_at(&notice("acme-ssh-prod", 30), t0);
        assert_eq!(p.info_at(t0).unwrap().entry, "acme-ssh-prod");
        p.clear();
        assert_eq!(p.info_at(t0), None);
    }

    /// Phase 39a (D51): attack with entry and command carrying raw control, bidi, zero
    /// width and Unicode tag characters (a daemon that did not format, or a forged
    /// frame): nothing invisible reaches the screen; what was already an escape passes
    /// unchanged.
    #[test]
    fn attack_raw_entry_and_command_arrive_neutralized() {
        let p = PendingRequest::default();
        let mut a = notice("acme\u{202E}gp-pg\u{1b}[2K\u{200B}\u{E0041}\n", 120);
        a.command = "\"psql\" \"-c\" \"x\u{2066}\u{feff}\u{9b}\u{7f}\"".into();
        p.mark(&a);
        let i = p.info().unwrap();
        for t in [&i.entry, &i.command] {
            assert!(!t.chars().any(ward_core::display::invisible), "{t:?}");
        }
        assert_eq!(i.entry, r"acme\u{202E}gp-pg\x1b[2K\u{200B}\u{E0041}\n");
        assert_eq!(i.command, r#""psql" "-c" "x\u{2066}\u{FEFF}\x9b\x7f""#);
        // The form already neutralized by the daemon passes unchanged (no double escape).
        let mut b = notice(r"acme\u{202E}", 120);
        b.command = r#""a\x1b""#.into();
        p.mark(&b);
        let j = p.info().unwrap();
        assert_eq!(
            (j.entry.as_str(), j.command.as_str()),
            (r"acme\u{202E}", r#""a\x1b""#)
        );
        // Serialized for the screen: only the known fields.
        let v = serde_json::to_value(&j).unwrap();
        let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["comando", "entrada", "id", "operacao", "restante"]);
    }

    /// The new Rust names are not wire names (frozen until m2:Phase 54): the screen still
    /// gets the old keys.
    #[test]
    fn attack_new_rust_names_are_not_wire_names() {
        let p = PendingRequest::default();
        p.mark(&notice("acme-pg-aprovacao", 120));
        let v = serde_json::to_value(p.info().unwrap()).unwrap();
        for new in ["remaining", "entry", "operation", "command"] {
            assert!(v.get(new).is_none(), "{new}");
        }
    }
}
