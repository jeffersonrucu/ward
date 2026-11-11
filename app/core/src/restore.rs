//! Restore backup (Phase 47, V7.7; D17, D39, S9).
//!
//! - [`VersionView`]: a version of the local backup as the screen shows it: the exact
//!   file name (`ward-AAAAMMDD-HHMMSS[-N].ward`, the same one that comes back in the
//!   request), the instant in UTC seconds, taken from the name itself (the screen shows it
//!   in the local time zone), and the marks the daemon checked (Phase 49a). Nothing of
//!   the content nor of the generation (D39).
//! - **The date in the name is not trustworthy** (Phase 49a): any process of the user can
//!   put in the folder an old copy with a new date name. The daemon compares the sealed
//!   (authenticated) generations and marks the version whose generation is lower than the
//!   one of a version with an earlier date ([`VersionView::out_of_order`]); the screen
//!   warns.
//! - [`Restore`]: what the screen sends on confirming: the version, the master password
//!   of that version (master password type, zeroed on drop) and the current PIN ([`Pin`],
//!   zeroed on drop; the daemon requires the PIN of the current vault, because the
//!   version comes back with other levels and another PIN, S9). An extra field is refused
//!   on reading.
//! - [`Restored`]: the response: restored and how many entries the version has (read
//!   again from the daemon). No value, no path, no generation.
//!
//! The version coming from the interface is untrusted: the format is checked here
//! ([`ward_core::backup::valid_name`]) and the service only sends the restore backup
//! request if the name is in the list the daemon just gave.

use serde::{Deserialize, Serialize};

use ward_core::protocol::VersaoBackup;

use crate::AppError;
use crate::destrave::SenhaMestra;
use crate::nivel::Pin;

/// A version of the local backup, without content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VersionView {
    /// The exact file name, as the daemon gave it.
    #[serde(rename = "nome")]
    pub name: String,
    /// Seconds since 1970 in UTC, from the name (date and time; the `-N` suffix does not
    /// change it).
    #[serde(rename = "instante")]
    pub instant: i64,
    /// The version opens with the key of the open vault (Phase 49a); `false`: from
    /// another master password (the daemon did not check its order).
    #[serde(rename = "conferida")]
    pub verified: bool,
    /// The write is older than the one of a version with an earlier date: the date in
    /// the name does not match the order of the writes (old copy with a new name, or the
    /// clock went backwards). Marked by the daemon.
    #[serde(rename = "fora_de_ordem")]
    pub out_of_order: bool,
}

impl VersionView {
    /// Reads a name that came from the daemon. Outside the strict D17 format it is `None`
    /// (the strict reading of the protocol already refused it; this is the second layer).
    pub fn from_name(name: &str) -> Option<VersionView> {
        if !name_ok(name) {
            return None;
        }
        let b = name.as_bytes();
        let num = |from: usize, to: usize| -> Option<i64> {
            std::str::from_utf8(&b[from..to]).ok()?.parse::<i64>().ok()
        };
        // `ward-AAAAMMDD-HHMMSS…`: date in 5..13, time in 14..20.
        let (y, m, d) = (num(5, 9)?, num(9, 11)?, num(11, 13)?);
        let (h, mi, s) = (num(14, 16)?, num(16, 18)?, num(18, 20)?);
        let instant = days_from_civil(y, m, d) * 86_400 + h * 3600 + mi * 60 + s;
        Some(VersionView {
            name: name.to_owned(),
            instant,
            verified: false,
            out_of_order: false,
        })
    }
}

/// Days since 1970-01-01 of a civil date (Howard Hinnant's algorithm, integers only).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let year_of_era = y - era * 400;
    let mp = (m + 9) % 12;
    let day_of_year = (153 * mp + 2) / 5 + d - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(unix)]
fn name_ok(name: &str) -> bool {
    ward_core::backup::valid_name(name)
}

#[cfg(not(unix))]
fn name_ok(_name: &str) -> bool {
    false
}

/// The backups response becomes versions, with the daemon's marks; a name outside the
/// format (or "out of order" without "verified") makes the whole response be refused
/// (the daemon does not send that).
pub fn versions_from(list: &[VersaoBackup]) -> Result<Vec<VersionView>, AppError> {
    list.iter()
        .map(|b| {
            if b.fora_de_ordem && !b.conferida {
                return Err(AppError::InvalidResponse);
            }
            let mut v = VersionView::from_name(&b.nome).ok_or(AppError::InvalidResponse)?;
            v.verified = b.conferida;
            v.out_of_order = b.fora_de_ordem;
            Ok(v)
        })
        .collect()
}

/// The version chosen in the interface: the name must be in the format. Does not check
/// the list (that belongs to the service, with the daemon).
pub fn requested_version(name: &str) -> Result<&str, AppError> {
    if name_ok(name) {
        Ok(name)
    } else {
        Err(AppError::InvalidRequest)
    }
}

/// "Restore this version": the version, its master password and the current PIN.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Restore {
    #[serde(rename = "versao")]
    pub version: String,
    #[serde(rename = "senha")]
    pub password: SenhaMestra,
    pub pin: Pin,
}

/// Response of the restore backup command: no value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Restored {
    #[serde(rename = "restaurado")]
    pub restored: bool,
    /// How many entries the vault has after the swap (the ones of the version).
    #[serde(rename = "entradas")]
    pub entries: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_becomes_utc_instant() {
        let v = VersionView::from_name("ward-19700101-000000.ward").unwrap();
        assert_eq!(v.instant, 0);
        // 2026-11-08 14:32:00 UTC.
        let v = VersionView::from_name("ward-20261108-143200.ward").unwrap();
        assert_eq!(v.instant, 1_794_148_320);
        // The suffix does not change the instant.
        let s = VersionView::from_name("ward-20261108-143200-2.ward").unwrap();
        assert_eq!(s.instant, v.instant);
        assert_eq!(s.name, "ward-20261108-143200-2.ward");
        // Leap year and turn of the year.
        assert_eq!(
            VersionView::from_name("ward-20240229-235959.ward")
                .unwrap()
                .instant,
            1_709_251_199
        );
        assert_eq!(
            VersionView::from_name("ward-20251231-235959.ward")
                .unwrap()
                .instant
                + 1,
            VersionView::from_name("ward-20260101-000000.ward")
                .unwrap()
                .instant
        );
    }

    #[test]
    fn attack_forged_version_outside_the_format_is_refused() {
        for bad in [
            "",
            "ward-20261108-143200",
            "../ward-20261108-143200.ward",
            "/home/x/ward-20261108-143200.ward",
            "backups/ward-20261108-143200.ward",
            "ward-20261108-143200.ward/",
            "ward-20261108-143200-01.ward",
            "ward-20261108-143200-100.ward",
            "ward-20261308-143200.ward",
            "ward-20261108-246000.ward",
            "WARD-20261108-143200.ward",
            "ward-20261108-143200.ward\n",
            " ward-20261108-143200.ward",
            "ward-2026１108-143200.ward",
            "ward-20261108-143200\u{202E}.ward",
            "cofre.ward",
        ] {
            assert_eq!(
                requested_version(bad),
                Err(AppError::InvalidRequest),
                "{bad:?}"
            );
            assert_eq!(VersionView::from_name(bad), None, "{bad:?}");
        }
        assert!(requested_version("ward-20261108-143200-99.ward").is_ok());
        assert_eq!(
            versions_from(&[
                vb("ward-20261108-143200.ward", true, false),
                vb("../x", true, false)
            ]),
            Err(AppError::InvalidResponse)
        );
        assert_eq!(
            versions_from(&[vb("ward-20261108-143200.ward", false, true)]),
            Err(AppError::InvalidResponse)
        );
    }

    fn vb(name: &str, verified: bool, out_of_order: bool) -> VersaoBackup {
        VersaoBackup {
            nome: name.into(),
            conferida: verified,
            fora_de_ordem: out_of_order,
        }
    }

    #[test]
    fn attack_restore_with_extra_field_or_bad_pin_is_refused() {
        let ok = serde_json::json!({
            "versao": "ward-20261108-143200.ward",
            "senha": "falso-mestra-ward-01",
            "pin": "135790",
        });
        let r: Restore = serde_json::from_value(ok.clone()).unwrap();
        assert_eq!(r.version, "ward-20261108-143200.ward");
        // The Debug shows nothing typed.
        let d = format!("{r:?}");
        assert!(!d.contains("falso-mestra") && !d.contains("135790"), "{d}");
        for (key, value) in [
            ("geracao", serde_json::json!(9)),
            ("caminho", serde_json::json!("/tmp/x.ward")),
            // m2 Phase 32: the new Rust names are not wire names (m2:DT3).
            ("version", serde_json::json!("ward-20261108-143200.ward")),
            ("password", serde_json::json!("falso-mestra-ward-01")),
        ] {
            let mut v = ok.clone();
            v[key] = value;
            assert!(serde_json::from_value::<Restore>(v).is_err(), "{key}");
        }
        for (key, value) in [
            ("pin", serde_json::json!("12ab")),
            ("pin", serde_json::json!("123")),
            ("pin", serde_json::json!(135790)),
            ("senha", serde_json::json!("")),
            ("versao", serde_json::json!(1)),
        ] {
            let mut v = ok.clone();
            v[key] = value;
            assert!(serde_json::from_value::<Restore>(v).is_err(), "{key}");
        }
        for missing in ["versao", "senha", "pin"] {
            let mut v = ok.clone();
            v.as_object_mut().unwrap().remove(missing);
            assert!(serde_json::from_value::<Restore>(v).is_err(), "{missing}");
        }
    }

    /// m2 Phase 32: the screen receives the old keys (m2:DT3, `rename` until Phase 54).
    #[test]
    fn attack_new_rust_names_are_not_wire_names() {
        let v = VersionView::from_name("ward-19700101-000000.ward").unwrap();
        assert_eq!(
            serde_json::to_string(&v).unwrap(),
            r#"{"nome":"ward-19700101-000000.ward","instante":0,"conferida":false,"fora_de_ordem":false}"#
        );
        let r = Restored {
            restored: true,
            entries: 3,
        };
        assert_eq!(
            serde_json::to_string(&r).unwrap(),
            r#"{"restaurado":true,"entradas":3}"#
        );
    }

    mod through_the_service {
        use std::sync::{Arc, Mutex};

        use ward_core::protocol::{self, ResponseError, Resposta, Role};
        use zeroize::Zeroizing;

        use super::super::*;
        use crate::area_transferencia::{AreaTransferencia, Copiador};
        use crate::daemon::Daemon;
        use crate::fake_daemon::{FakeDaemon, SECRETS};
        use crate::service::Service;

        type Frames = Arc<Mutex<Vec<Vec<u8>>>>;

        struct NoClipboard;
        impl AreaTransferencia for NoClipboard {
            fn escrever(&self, _: &[u8]) -> Result<(), AppError> {
                Ok(())
            }
            fn ler(&self) -> Result<Option<Zeroizing<Vec<u8>>>, AppError> {
                Ok(None)
            }
            fn limpar(&self) -> Result<(), AppError> {
                Ok(())
            }
        }

        /// Fake daemon that keeps each frame received.
        #[derive(Clone)]
        struct Spy(Arc<Mutex<FakeDaemon>>, Frames);
        impl Daemon for Spy {
            fn exchange(&mut self, request: &[u8]) -> Result<Zeroizing<Vec<u8>>, AppError> {
                self.1.lock().unwrap().push(request.to_vec());
                self.0.lock().unwrap().exchange(request)
            }
        }

        fn service(fake: FakeDaemon) -> (Service<Spy, NoClipboard>, Spy) {
            let e = Spy(
                Arc::new(Mutex::new(fake.without_wait())),
                Arc::new(Mutex::new(Vec::new())),
            );
            (Service::new(e.clone(), Copiador::novo(NoClipboard)), e)
        }

        fn restores(e: &Spy) -> usize {
            e.1.lock()
                .unwrap()
                .iter()
                .filter(|q| String::from_utf8_lossy(q).contains("\"restaurar_backup\""))
                .count()
        }

        fn request(version: &str, password: &str, pin: &str) -> Restore {
            serde_json::from_value(serde_json::json!({
                "versao": version, "senha": password, "pin": pin,
            }))
            .unwrap()
        }

        const PASSWORD: &str = "falso-mestra-ward-01";
        const PIN: &str = "135790";

        /// No response to the app carries a secret, password, PIN or path (the
        /// generation of the list is on purpose, Phase 49a).
        fn no_secret(json: &str) {
            for s in SECRETS {
                assert!(!json.contains(s), "segredo em {json}");
            }
            for needle in [PIN, PASSWORD, "/home", ".local", "backups/"] {
                assert!(!json.contains(needle), "{needle} em {json}");
            }
        }

        #[test]
        fn v7_7_chooses_version_confirms_and_the_version_entries_show_up() {
            let (s, e) = service(FakeDaemon::full());
            let before = s.list_credentials().unwrap().len();
            let versions = s.list_backups().unwrap();
            assert_eq!(versions.len(), 30);
            no_secret(&serde_json::to_string(&versions).unwrap());
            // From the newest to the oldest.
            assert!(versions.windows(2).all(|w| w[0].instant > w[1].instant));
            let chosen = versions[2].clone();
            let r = s
                .restore_backup(request(&chosen.name, PASSWORD, PIN))
                .unwrap();
            assert!(r.restored);
            // Version 2 has two entries fewer than the test vault.
            assert_eq!(r.entries, before - 2);
            assert_eq!(s.list_credentials().unwrap().len(), before - 2);
            no_secret(&serde_json::to_string(&r).unwrap());
            // The current one became the newest version; there are still 30.
            let after = s.list_backups().unwrap();
            assert_eq!(after.len(), 30);
            assert_ne!(after[0].name, versions[0].name);
            assert_eq!(restores(&e), 1);
            // The stored current one comes back with the old password and the old
            // entries.
            let r = s
                .restore_backup(request(&after[0].name, PASSWORD, PIN))
                .unwrap();
            assert_eq!(r.entries, before);
        }

        /// Phase 49a: the old copy planted with a new date name reaches the top of the
        /// list, but marked; the real ones do not. After restoring, the stored current
        /// one is not marked. No generation reaches the interface (D39).
        #[test]
        fn attack_planted_copy_arrives_marked_by_the_generation() {
            let (s, _) = service(FakeDaemon::planted_backup());
            let v = s.list_backups().unwrap();
            assert_eq!(v.len(), 31);
            assert!(v[0].out_of_order && v[0].verified);
            assert!(v[1..].iter().all(|x| !x.out_of_order && x.verified));
            assert!(!serde_json::to_string(&v).unwrap().contains("geracao"));
            let r = s
                .restore_backup(request(&v[2].name, PASSWORD, PIN))
                .unwrap();
            assert!(r.restored);
            let after = s.list_backups().unwrap();
            let marked: Vec<&str> = after
                .iter()
                .filter(|x| x.out_of_order)
                .map(|x| x.name.as_str())
                .collect();
            assert_eq!(marked, [v[0].name.as_str()]);
        }

        #[test]
        fn attack_wrong_pin_does_not_restore_and_the_fifth_locks() {
            let (s, e) = service(FakeDaemon::full());
            let v = s.list_backups().unwrap()[3].name.clone();
            let before = e.0.lock().unwrap().entry_count();
            for n in 1..=4 {
                assert_eq!(
                    s.restore_backup(request(&v, PASSWORD, "000000")),
                    Err(AppError::WrongPin)
                );
                assert_eq!(e.0.lock().unwrap().pin_errors(), n);
            }
            assert_eq!(e.0.lock().unwrap().entry_count(), before);
            assert_eq!(
                s.restore_backup(request(&v, PASSWORD, "000000")),
                Err(AppError::WrongPin)
            );
            // The 5th locks: nothing else is listed nor restored, not even with the
            // right PIN.
            assert_eq!(s.list_backups(), Err(AppError::Locked));
            assert_eq!(
                s.restore_backup(request(&v, PASSWORD, PIN)),
                Err(AppError::Locked)
            );
            assert_eq!(e.0.lock().unwrap().entry_count(), before);
        }

        #[test]
        fn attack_wrong_version_password_changes_nothing() {
            let (s, e) = service(FakeDaemon::full());
            let names = e.0.lock().unwrap().backup_names();
            let before = e.0.lock().unwrap().entry_count();
            for wrong in ["falso-mestra-ward-02", "falso-mestra-ward-0", "x"] {
                assert_eq!(
                    s.restore_backup(request(&names[1], wrong, PIN)),
                    Err(AppError::WrongPassword)
                );
            }
            assert_eq!(e.0.lock().unwrap().entry_count(), before);
            assert_eq!(e.0.lock().unwrap().backup_names(), names);
            // The right PIN reset the count (as in `wardd`).
            assert_eq!(e.0.lock().unwrap().pin_errors(), 0);
        }

        #[test]
        fn attack_missing_or_forged_version_does_not_reach_the_daemon() {
            let (s, e) = service(FakeDaemon::full());
            // Outside the format: refused before any frame.
            for bad in [
                "../cofre.ward",
                "/home/usuario/.local/share/ward/cofre.ward",
                "ward-20261108-143200.ward/../../x",
                "ward-20261108-143200\u{202E}.ward",
            ] {
                assert_eq!(
                    s.restore_backup(request(bad, PASSWORD, PIN)),
                    Err(AppError::InvalidRequest),
                    "{bad:?}"
                );
            }
            assert!(e.1.lock().unwrap().is_empty());
            // In the format, but outside the daemon's list: no restore request.
            assert_eq!(
                s.restore_backup(request("ward-19990101-000000.ward", PASSWORD, PIN)),
                Err(AppError::BackupNotFound)
            );
            assert_eq!(restores(&e), 0);
            // A client that skips the service and sends it directly: the fake answers
            // like `wardd` (backup not found), and nothing changes.
            let before = e.0.lock().unwrap().entry_count();
            let r = crate::client::restore_backup(
                &mut e.clone(),
                "ward-19990101-000000.ward",
                &crate::destrave::SenhaMestra::nova(PASSWORD.into()).unwrap(),
                &crate::nivel::Pin::novo(PIN.into()).unwrap(),
            );
            assert_eq!(r, Err(AppError::BackupNotFound));
            assert_eq!(e.0.lock().unwrap().entry_count(), before);
        }

        #[test]
        fn attack_vault_locked_in_the_middle_does_not_restore() {
            let (s, e) = service(FakeDaemon::full());
            let v = s.list_backups().unwrap()[1].name.clone();
            let before = e.0.lock().unwrap().entry_count();
            // Locked (timeout, tray, another window) between choosing and confirming.
            s.lock().unwrap();
            assert_eq!(
                s.restore_backup(request(&v, PASSWORD, PIN)),
                Err(AppError::Locked)
            );
            assert_eq!(restores(&e), 0);
            assert_eq!(e.0.lock().unwrap().entry_count(), before);
        }

        #[test]
        fn unavailable_dir_and_no_versions() {
            let (s, _) = service(FakeDaemon::full().with_backups(None));
            assert_eq!(s.list_backups(), Err(AppError::BackupUnavailable));
            let (s, _) = service(FakeDaemon::full().with_backups(Some(Vec::new())));
            assert_eq!(s.list_backups(), Ok(Vec::new()));
        }

        /// The daemon that sees this connection as an agent (`wardd` with another role,
        /// D5).
        #[derive(Clone)]
        struct AsAgent(Frames);
        impl Daemon for AsAgent {
            fn exchange(&mut self, request: &[u8]) -> Result<Zeroizing<Vec<u8>>, AppError> {
                self.0.lock().unwrap().push(request.to_vec());
                let r = match protocol::decode_request(Role::Agent, request) {
                    Ok(_) => Resposta::Erro(ResponseError::Internal),
                    Err(e) => Resposta::Erro(e),
                };
                protocol::encode_response(Role::Agent, &r).map_err(|_| AppError::InvalidResponse)
            }
        }

        #[test]
        fn v7_8_agent_role_gets_unauthorized() {
            let q: Frames = Arc::new(Mutex::new(Vec::new()));
            let mut d = AsAgent(Arc::clone(&q));
            assert_eq!(crate::client::backups(&mut d), Err(AppError::Unauthorized));
            let r = crate::client::restore_backup(
                &mut d,
                "ward-20261108-143200.ward",
                &crate::destrave::SenhaMestra::nova(PASSWORD.into()).unwrap(),
                &crate::nivel::Pin::novo(PIN.into()).unwrap(),
            );
            assert_eq!(r, Err(AppError::Unauthorized));
            // Through the service too: the list is already refused and the restore is
            // not even requested.
            let s = Service::new(AsAgent(Arc::clone(&q)), Copiador::novo(NoClipboard));
            assert_eq!(s.list_backups(), Err(AppError::Unauthorized));
            assert_eq!(
                s.restore_backup(request("ward-20261108-143200.ward", PASSWORD, PIN)),
                Err(AppError::Unauthorized)
            );
            assert_eq!(q.lock().unwrap().len(), 4);
        }
    }
}
