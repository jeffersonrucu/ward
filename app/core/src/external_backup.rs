//! Settings › external backup on a USB drive (Phase 48, V7.9; D17, D30).
//!
//! The daemon (Phase 46) has the external backup, set external backup (on, optional
//! UUID) and USB drives operations, only for the app role (V7.8). Here is what the screen receives and
//! what it may request:
//!
//! - [`UsbDriveView`]: UUID, label and whether it is mounted. The label comes from
//!   udisks2 (the name anyone writes on the USB drive), so it is **untrusted**: the daemon
//!   already removes control characters and cuts at the protocol label limit; here the invisible ones
//!   (text direction, zero width, tags) become a visible escape through
//!   [`ward_core::display::neutralize`] (D51), and the screen shows it as text. No mount
//!   path reaches the interface.
//! - [`ExternalBackupView`]: on or off, the chosen UUID and, if it is connected, the USB
//!   drive seen.
//! - [`UsbDriveChoice`]: what the interface sends. Turning on requires a UUID in the D45
//!   format **and** that it is one of the USB drives the daemon sees at that moment (the
//!   daemon is asked again; a UUID forged by the interface does not even reach
//!   the set request). Turning off carries no UUID: the Rust side keeps the one
//!   that was chosen.

use serde::Serialize;
use ward_core::config::ExternalBackup;
use ward_core::limits::MAX_BACKUP_UUID;
use ward_core::protocol::Pendrive;

use crate::AppError;

/// A USB drive for the screen: the label already neutralized (D51).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UsbDriveView {
    pub uuid: String,
    #[serde(rename = "rotulo")]
    pub label: String,
    #[serde(rename = "montado")]
    pub mounted: bool,
}

impl UsbDriveView {
    pub(crate) fn from_drive(p: &Pendrive) -> UsbDriveView {
        UsbDriveView {
            uuid: p.uuid.clone(),
            label: ward_core::display::neutralize(p.rotulo.trim()),
            mounted: p.montado,
        }
    }
}

/// What the external backup card shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExternalBackupView {
    #[serde(rename = "ligado")]
    pub enabled: bool,
    /// UUID of the chosen USB drive (the daemon keeps it even when off).
    pub uuid: Option<String>,
    /// The chosen USB drive, if it is connected now.
    #[serde(rename = "pendrive")]
    pub usb_drive: Option<UsbDriveView>,
    /// `false` if udisks2 did not answer (no way to say whether it is connected).
    #[serde(rename = "pendrives_visiveis")]
    pub visible_usb_drives: bool,
}

/// Request from the interface (untrusted): turn on with the chosen USB drive, or turn off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsbDriveChoice {
    Enable(String),
    Disable,
}

impl UsbDriveChoice {
    /// On/off and UUID as they came from the interface. Turning on without a UUID, a
    /// UUID out of the D45 format or turning off with a UUID: [`AppError::InvalidRequest`],
    /// before the daemon.
    pub fn new(enabled: bool, uuid: Option<&str>) -> Result<UsbDriveChoice, AppError> {
        match (enabled, uuid) {
            (true, Some(u)) if valid_uuid(u) => Ok(UsbDriveChoice::Enable(u.to_owned())),
            (false, None) => Ok(UsbDriveChoice::Disable),
            _ => Err(AppError::InvalidRequest),
        }
    }
}

/// UUID in the D45 format (`[0-9A-Fa-f-]`, 1 to 36, no `-` at the ends).
pub fn valid_uuid(uuid: &str) -> bool {
    uuid.len() <= MAX_BACKUP_UUID && ExternalBackup::new(true, Some(uuid)).is_ok()
}

/// The USB drive with that UUID in the daemon's list (case-insensitive, like the `wardd`
/// copy). Two with the same UUID: none (the `wardd` does not copy either).
pub(crate) fn find<'a>(list: &'a [Pendrive], uuid: &str) -> Option<&'a Pendrive> {
    let mut matches = list.iter().filter(|p| p.uuid.eq_ignore_ascii_case(uuid));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(uuid: &str, label: &str) -> Pendrive {
        Pendrive {
            uuid: uuid.to_owned(),
            rotulo: label.to_owned(),
            montado: true,
        }
    }

    #[test]
    fn valid_choice() {
        assert_eq!(
            UsbDriveChoice::new(true, Some("7C1E-42A9")),
            Ok(UsbDriveChoice::Enable("7C1E-42A9".into()))
        );
        assert_eq!(
            UsbDriveChoice::new(false, None),
            Ok(UsbDriveChoice::Disable)
        );
    }

    #[test]
    fn attack_forged_uuid_out_of_format_is_refused() {
        for bad in [
            "",
            "-7C1E",
            "7C1E-",
            "../etc",
            "/media/usuario/X",
            "7C1E 42A9",
            "7C1E\n42A9",
            "7C1E-42A9\u{202E}",
            "７C1E",
            "g000",
            "<img src=x>",
        ] {
            assert_eq!(
                UsbDriveChoice::new(true, Some(bad)),
                Err(AppError::InvalidRequest),
                "{bad:?}"
            );
        }
        let long = "A".repeat(MAX_BACKUP_UUID + 1);
        assert_eq!(
            UsbDriveChoice::new(true, Some(&long)),
            Err(AppError::InvalidRequest)
        );
        // Turning on without a USB drive, and turning off changing the stored UUID.
        assert_eq!(
            UsbDriveChoice::new(true, None),
            Err(AppError::InvalidRequest)
        );
        assert_eq!(
            UsbDriveChoice::new(false, Some("7C1E-42A9")),
            Err(AppError::InvalidRequest)
        );
    }

    #[test]
    fn attack_label_with_direction_and_zero_width_becomes_visible_escape() {
        let v = UsbDriveView::from_drive(&p(
            "7C1E-42A9",
            "<img src=x onerror=alert(1)>\u{202E}gpj.exe\u{200B}\u{2066}",
        ));
        assert!(
            !v.label
                .chars()
                .any(|c| ward_core::display::invisible(c) || c.is_control()),
            "{:?}",
            v.label
        );
        assert!(v.label.contains("\\u{202E}"), "{}", v.label);
        // The HTML stays text: the screen (Svelte) escapes it, and nothing is lost here.
        assert!(v.label.starts_with("<img src=x onerror=alert(1)>"));
    }

    #[test]
    fn find_case_insensitive_and_repeated_does_not_count() {
        let list = [p("7C1E-42A9", "A"), p("3F2A-9B10", "B")];
        assert_eq!(
            find(&list, "7c1e-42a9").map(|p| p.rotulo.as_str()),
            Some("A")
        );
        assert!(find(&list, "0000-0000").is_none());
        let repeated = [p("7C1E-42A9", "A"), p("7c1e-42a9", "clone")];
        assert!(find(&repeated, "7C1E-42A9").is_none());
    }

    /// m2:DT3: the new Rust names are not wire names. The webview keeps receiving the
    /// old fields until Phase 54.
    #[test]
    fn attack_new_rust_names_are_not_wire_names() {
        let v = ExternalBackupView {
            enabled: true,
            uuid: Some("7C1E-42A9".into()),
            usb_drive: Some(UsbDriveView::from_drive(&p("7C1E-42A9", "WARD"))),
            visible_usb_drives: true,
        };
        assert_eq!(
            serde_json::to_string(&v).unwrap(),
            r#"{"ligado":true,"uuid":"7C1E-42A9","pendrive":{"uuid":"7C1E-42A9","rotulo":"WARD","montado":true},"pendrives_visiveis":true}"#
        );
    }

    /// Response forged by the "daemon" (the server is the attacker): label with a control
    /// character, an extra mount path, UUID out of format, too many USB drives. The strict
    /// reading refuses all of it; nothing reaches the screen.
    #[test]
    fn attack_forged_usb_drives_response_is_refused() {
        use zeroize::Zeroizing;

        use crate::client;
        use crate::daemon::Daemon;

        struct Response1(Vec<u8>);
        impl Daemon for Response1 {
            fn exchange(&mut self, _request: &[u8]) -> Result<Zeroizing<Vec<u8>>, AppError> {
                Ok(Zeroizing::new(self.0.clone()))
            }
        }
        let item = |body: &str| {
            format!(r#"{{"v":1,"tipo":"pendrives","dados":{{"itens":[{body}]}}}}"#).into_bytes()
        };
        let ok = item(r#"{"uuid":"7C1E-42A9","rotulo":"WARD","montado":true}"#);
        assert_eq!(client::usb_drives(&mut Response1(ok)).unwrap().len(), 1);
        let many = (0..=ward_core::protocol::MAX_PENDRIVES)
            .map(|i| format!(r#"{{"uuid":"{i:04X}","rotulo":"x","montado":true}}"#))
            .collect::<Vec<_>>()
            .join(",");
        for bad in [
            item(r#"{"uuid":"7C1E-42A9","rotulo":"WARD\u001b[2J","montado":true}"#),
            item(r#"{"uuid":"7C1E-42A9","rotulo":"A\nB","montado":true}"#),
            item(
                r#"{"uuid":"7C1E-42A9","rotulo":"WARD","montado":true,"montagem":"/media/u/WARD"}"#,
            ),
            item(r#"{"uuid":"../etc","rotulo":"WARD","montado":true}"#),
            item(&format!(
                r#"{{"uuid":"7C1E-42A9","rotulo":"{}","montado":true}}"#,
                "a".repeat(ward_core::protocol::MAX_ROTULO + 1)
            )),
            item(&many),
        ] {
            assert_eq!(
                client::usb_drives(&mut Response1(bad.clone())).unwrap_err(),
                AppError::InvalidResponse,
                "{}",
                String::from_utf8_lossy(&bad)
            );
        }
    }

    mod via_service {
        use std::sync::{Arc, Mutex};

        use zeroize::Zeroizing;

        use super::super::*;
        use crate::clipboard::{Clipboard, Copier};
        use crate::daemon::Daemon;
        use crate::fake_daemon::FakeDaemon;
        use crate::level::Pin;
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

        /// Fake daemon that keeps every frame received and lets the config be seen.
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
            (Service::new(e.clone(), Copier::new(NoClipboard)), e)
        }

        fn set_requests(e: &Spy) -> usize {
            e.1.lock()
                .unwrap()
                .iter()
                .filter(|q| String::from_utf8_lossy(q).contains("\"definir_backup_externo\""))
                .count()
        }

        fn no_path(json: &str) {
            for needle in ["/media", "/run", "montagem", "/dev/"] {
                assert!(!json.contains(needle), "{needle} em {json}");
            }
        }

        #[test]
        fn v7_9_choose_turn_on_and_turn_off() {
            let (s, e) = service(FakeDaemon::full());
            // Off by default (D17), no USB drive chosen.
            let b = s.external_backup().unwrap();
            assert!(!b.enabled && b.uuid.is_none() && b.usb_drive.is_none());
            assert!(!s.settings().unwrap().external_backup);
            let list = s.list_usb_drives().unwrap();
            assert_eq!(list.len(), 4);
            no_path(&serde_json::to_string(&list).unwrap());
            // Turn on with a USB drive from the list (in lowercase: the daemon's UUID goes).
            let b = s.set_external_backup(true, Some("3f2a-9b10")).unwrap();
            assert!(b.enabled && b.visible_usb_drives);
            assert_eq!(b.uuid.as_deref(), Some("3F2A-9B10"));
            let p = b.usb_drive.as_ref().unwrap();
            assert_eq!((p.label.as_str(), p.mounted), ("FOTOS 2024", false));
            assert!(s.settings().unwrap().external_backup);
            assert_eq!(
                e.0.lock().unwrap().backup(),
                (true, Some("3F2A-9B10".into()))
            );
            no_path(&serde_json::to_string(&b).unwrap());
            // Switch USB drives.
            let b = s.set_external_backup(true, Some("7C1E-42A9")).unwrap();
            assert_eq!(b.usb_drive.unwrap().label, "WARD-BACKUP");
            // Turning off keeps the stored UUID; nothing is erased from the USB drive
            // (Phase 46).
            let b = s.set_external_backup(false, None).unwrap();
            assert!(!b.enabled);
            assert_eq!(b.uuid.as_deref(), Some("7C1E-42A9"));
            assert_eq!(
                e.0.lock().unwrap().backup(),
                (false, Some("7C1E-42A9".into()))
            );
            assert!(!s.settings().unwrap().external_backup);
            assert_eq!(set_requests(&e), 3);
        }

        #[test]
        fn attack_uuid_forged_by_the_interface_does_not_reach_the_daemon() {
            let (s, e) = service(FakeDaemon::full());
            // Out of format: not a single frame.
            for bad in [
                "../etc",
                "/media/usuario/WARD-BACKUP",
                "7C1E-42A9\u{202E}",
                "",
            ] {
                assert_eq!(
                    s.set_external_backup(true, Some(bad)),
                    Err(AppError::InvalidRequest),
                    "{bad:?}"
                );
            }
            assert_eq!(
                s.set_external_backup(true, None),
                Err(AppError::InvalidRequest)
            );
            assert_eq!(
                s.set_external_backup(false, Some("7C1E-42A9")),
                Err(AppError::InvalidRequest)
            );
            assert!(e.1.lock().unwrap().is_empty());
            // In the format, but not from a connected USB drive: the daemon is only asked
            // (USB drives list); the set request does not go out.
            for forged in ["DEAD-BEEF", "7C1E-42AA", "7C1E42A9"] {
                assert_eq!(
                    s.set_external_backup(true, Some(forged)),
                    Err(AppError::UsbDriveNotConnected),
                    "{forged}"
                );
            }
            assert_eq!(set_requests(&e), 0);
            assert_eq!(e.0.lock().unwrap().backup(), (false, None));
        }

        #[test]
        fn attack_cloned_usb_drive_with_repeated_uuid_is_not_chosen() {
            let mut list = crate::fake_daemon::test_usb_drives();
            let mut clone = list[0].clone();
            clone.uuid = clone.uuid.to_lowercase();
            list.push(clone);
            let (s, e) = service(FakeDaemon::full().with_usb_drives(Some(list)));
            assert_eq!(
                s.set_external_backup(true, Some("7C1E-42A9")),
                Err(AppError::UsbDriveNotConnected)
            );
            assert_eq!(set_requests(&e), 0);
        }

        #[test]
        fn without_udisks_does_not_turn_on_and_the_card_says_it_did_not_see() {
            let (s, e) = service(FakeDaemon::external_backup_enabled().with_usb_drives(None));
            assert_eq!(s.list_usb_drives(), Err(AppError::UsbDrivesUnavailable));
            let b = s.external_backup().unwrap();
            assert!(b.enabled && !b.visible_usb_drives && b.usb_drive.is_none());
            assert_eq!(
                s.set_external_backup(true, Some("3F2A-9B10")),
                Err(AppError::UsbDrivesUnavailable)
            );
            assert_eq!(set_requests(&e), 0);
            // Turning off does not depend on udisks2.
            assert!(!s.set_external_backup(false, None).unwrap().enabled);
        }

        #[test]
        fn attack_usb_drive_label_arrives_neutralized() {
            let (s, _) = service(FakeDaemon::full());
            let list = s.list_usb_drives().unwrap();
            let odd = list.iter().find(|p| p.uuid.starts_with("a1b2")).unwrap();
            assert!(
                !odd.label
                    .chars()
                    .any(|c| ward_core::display::invisible(c) || c.is_control()),
                "{:?}",
                odd.label
            );
            assert!(
                odd.label.contains("\\u{202E}") && odd.label.contains("\\u{200B}"),
                "{}",
                odd.label
            );
            let b = s.set_external_backup(true, Some(&odd.uuid)).unwrap();
            assert_eq!(b.usb_drive.unwrap().label, odd.label);
        }

        #[test]
        fn locked_does_not_read_nor_change() {
            let (s, e) = service(FakeDaemon::full());
            // Five wrong PINs lock the fake (S9).
            for _ in 0..5 {
                let _ = s.set_level("acme-pg-dev", "livre", Pin::new("0000".into()).unwrap());
            }
            assert_eq!(s.external_backup(), Err(AppError::Locked));
            assert_eq!(s.list_usb_drives(), Err(AppError::Locked));
            assert_eq!(
                s.set_external_backup(true, Some("7C1E-42A9")),
                Err(AppError::Locked)
            );
            assert_eq!(set_requests(&e), 0);
        }
    }
}
