//! Tests of the vault configuration (D30): PIN hash (create, verify, PHC format and
//! attacks on the hash), lock timeout and external backup. Fake values only (D15 PIN).

use std::sync::OnceLock;

use super::*;
use crate::format::{MasterPassword, OpenVault};

const PIN: &[u8] = b"135790";

/// A hash of the test PIN, computed once (the PIN's Argon2id costs 64 MiB).
fn hash() -> &'static PinHash {
    static H: OnceLock<PinHash> = OnceLock::new();
    H.get_or_init(|| PinHash::create(PIN).unwrap())
}

fn text(h: &PinHash) -> &str {
    core::str::from_utf8(h.bytes()).unwrap()
}

#[test]
fn pin_hash_accepts_the_right_pin_and_refuses_the_others() {
    let h = hash();
    assert!(h.verify(PIN));
    for wrong in [
        &b"135791"[..],
        b"035790",
        b"13579",
        b"1357900",
        b"",
        b"abcdef",
        b"13579 ",
        b" 135790",
        b"135790\0",
        "١٣٥٧٩٠".as_bytes(),
    ] {
        assert!(!h.verify(wrong), "aceitou um PIN errado");
    }
}

#[test]
fn pin_of_4_to_6_digits() {
    for invalid in [
        &b""[..],
        b"1",
        b"123",
        b"1234567",
        b"12a4",
        b"12 34",
        b"-1234",
        b"+1234",
    ] {
        assert_eq!(
            PinHash::create(invalid).map(|_| ()),
            Err(ConfigError::InvalidPin)
        );
    }
    let four = PinHash::create(b"0042").unwrap();
    assert!(four.verify(b"0042"));
    assert!(!four.verify(b"042"));
    assert!(!four.verify(b"00420"));
}

/// The hash is PHC Argon2id v=19 with the parameters of [`KDF_PIN`], a 16-byte salt and
/// a 32-byte output; it fits in the limit; and the library (`argon2::PasswordVerifier`,
/// which parses the PHC on its own) agrees with it.
#[test]
fn hash_is_standard_argon2id_phc() {
    use argon2::password_hash::PasswordVerifier;

    let t = text(hash());
    assert!(t.starts_with("$argon2id$v=19$m=65536,t=3,p=1$"));
    let fields: Vec<&str> = t.split('$').collect();
    assert_eq!(fields.len(), 6);
    assert_eq!(fields[4].len(), 22);
    assert_eq!(fields[5].len(), 43);
    assert!(t.len() <= MAX_PIN_HASH);
    assert_eq!(hash().params(), KDF_PIN);
    let library = argon2::Argon2::default();
    assert!(library.verify_password(PIN, t).is_ok());
    assert!(library.verify_password(b"135791", t).is_err());
}

#[test]
fn each_hash_has_its_own_salt() {
    let other = PinHash::create(PIN).unwrap();
    assert_ne!(text(&other), text(hash()));
    assert!(other.verify(PIN));
}

/// Attack: any character changed in the hash (salt or output) makes the right PIN fail
/// to verify (or the hash not even be accepted).
#[test]
fn tampered_hash_does_not_verify() {
    let t = text(hash()).to_owned();
    let salt_start = t.rfind('$').unwrap() - 22;
    for pos in [salt_start, salt_start + 10, t.len() - 2, t.len() - 10] {
        let mut b = t.clone().into_bytes();
        b[pos] = if b[pos] == b'A' { b'B' } else { b'A' };
        if let Some(h) = PinHash::from_phc(&b) {
            assert!(!h.verify(PIN), "hash adulterado conferiu");
        }
    }
    // Without tampering, the re-read text verifies.
    assert!(PinHash::from_phc(t.as_bytes()).unwrap().verify(PIN));
}

/// Attack: a hash with an inflated cost (4 GiB of memory) is refused without running
/// Argon2.
#[test]
fn inflated_hash_refused_fast() {
    let t = text(hash()).replace("m=65536", "m=4194304");
    let start = std::time::Instant::now();
    assert!(PinHash::from_phc(t.as_bytes()).is_none());
    assert!(!verify_text(t.as_bytes()));
    assert!(start.elapsed() < std::time::Duration::from_millis(100));
}

/// `Debug` shows neither the hash nor the PIN, not in the configuration nor in the open
/// vault, and the errors carry no value (S13, V2.27).
#[test]
fn debug_and_errors_without_secret() {
    let t = text(hash()).to_owned();
    let output = t.rsplit('$').next().unwrap().to_owned();
    let d = format!("{:?} {:#?}", hash(), hash());
    assert!(d.contains("••••"));
    let cfg = VaultConfig::new(PinHash::from_phc(t.as_bytes()).unwrap());
    let dc = format!("{cfg:?} {cfg:#?}");
    let password = MasterPassword::new("falso-mestra-ward-01".to_owned());
    let vault = OpenVault::new(&password, cfg).unwrap();
    let dvault = format!("{vault:?} {vault:#?}");
    for text in [&d, &dc, &dvault] {
        assert!(!text.contains(&output), "Debug com o hash");
        assert!(!text.contains("135790"), "Debug com o PIN");
        assert!(!text.contains("argon2id$"), "Debug com o texto PHC");
        assert!(!text.contains("falso-mestra"), "Debug com a senha");
    }
    assert!(dvault.contains("lock_minutes: 240"));
    for e in [
        ConfigError::InvalidPin,
        ConfigError::Hash,
        ConfigError::LockTimeout,
        ConfigError::InvalidUuid,
        ConfigError::BackupWithoutUuid,
    ] {
        let s = format!("{e} {e:?}");
        assert!(!s.contains("135790") && !format!("{e}").is_empty());
    }
}

#[test]
fn new_config_has_the_defaults() {
    let cfg = VaultConfig::new(PinHash::from_phc(hash().bytes()).unwrap());
    assert_eq!(cfg.lock_minutes(), 240);
    assert_eq!(cfg.lock_minutes(), crate::limits::DEFAULT_LOCK_MINUTES);
    assert!(!cfg.backup().enabled());
    assert_eq!(cfg.backup().uuid(), None);
    assert!(cfg.verify_pin(PIN));
}

#[test]
fn lock_timeout_in_range() {
    let mut cfg = VaultConfig::new(PinHash::from_phc(hash().bytes()).unwrap());
    for invalid in [0, MAX_LOCK_MINUTES + 1, u32::MAX] {
        assert_eq!(cfg.set_lock_minutes(invalid), Err(ConfigError::LockTimeout));
        assert_eq!(cfg.lock_minutes(), 240, "valor recusado não muda nada");
    }
    for valid in [MIN_LOCK_MINUTES, 30, MAX_LOCK_MINUTES] {
        cfg.set_lock_minutes(valid).unwrap();
        assert_eq!(cfg.lock_minutes(), valid);
    }
}

#[test]
fn external_backup() {
    assert_eq!(
        ExternalBackup::new(true, None),
        Err(ConfigError::BackupWithoutUuid)
    );
    for invalid in [
        "",
        "-1234",
        "1234-",
        "12 34",
        "../x",
        "1234/ABCD",
        "ç",
        &"a".repeat(37),
    ] {
        assert_eq!(
            ExternalBackup::new(false, Some(invalid)),
            Err(ConfigError::InvalidUuid),
            "{invalid}"
        );
    }
    for valid in [
        "1234-ABCD",
        "0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0",
        "A1B2C3D4E5F60718",
    ] {
        let b = ExternalBackup::new(true, Some(valid)).unwrap();
        assert!(b.enabled());
        assert_eq!(b.uuid(), Some(valid));
    }
    let b = ExternalBackup::new(false, Some("1234-ABCD")).unwrap();
    assert!(!b.enabled());
    assert_eq!(
        ExternalBackup::disabled(),
        ExternalBackup::new(false, None).unwrap()
    );
}

#[test]
fn change_the_pin() {
    let mut cfg = VaultConfig::new(PinHash::from_phc(hash().bytes()).unwrap());
    cfg.change_pin_hash(PinHash::create(b"2468").unwrap());
    assert!(cfg.verify_pin(b"2468"));
    assert!(!cfg.verify_pin(PIN));
}
