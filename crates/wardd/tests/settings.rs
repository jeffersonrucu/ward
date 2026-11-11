//! Settings screen in the daemon (Phase 38, V5.22; D30, D39, S9): the settings
//! operation (lock timeout and external backup on or off) and the changes of PIN,
//! master password and lock timeout by the app role.
//!
//! Attacks: the agent and the extension neither read nor change the settings; locked,
//! nothing goes out; the reply does not carry the PIN hash; changing the PIN without the
//! current PIN and the password without the current password changes nothing; each
//! change writes a new generation (D39).
//!
//! Own test vault, only with fake values (never the real vault nor the one in `.dev/`).

mod common;

use common::*;
use ward_core::limits::{DEFAULT_LOCK_MINUTES, MAX_LOCK_MINUTES, MIN_LOCK_MINUTES};
use ward_core::protocol::{Response, ResponseError, Role};

fn settings(d: &wardd::core::Daemon) -> (u32, bool) {
    match request(d, Role::App, "configuracao", "{}") {
        Response::Config {
            lock_minutes,
            external_backup,
            local_backup,
        } => {
            // The test vault has the backups folder at 0700 (Phase 49a).
            assert!(local_backup);
            (lock_minutes, external_backup)
        }
        other => panic!("{other:?}"),
    }
}

fn lock_timeout(d: &wardd::core::Daemon, role: Role, minutes: u32) -> Response {
    request(
        d,
        role,
        "tempo_trava",
        &format!(r#"{{"minutos":{minutes}}}"#),
    )
}

fn change_pin(d: &wardd::core::Daemon, role: Role, current: &str, new: &str) -> Response {
    request(
        d,
        role,
        "trocar_pin",
        &format!(
            r#"{{"atual":"{}","novo":"{}"}}"#,
            b64(current.as_bytes()),
            b64(new.as_bytes())
        ),
    )
}

fn change_password(d: &wardd::core::Daemon, role: Role, current: &str, new: &str) -> Response {
    request(
        d,
        role,
        "trocar_senha",
        &format!(
            r#"{{"atual":"{}","nova":"{}"}}"#,
            b64(current.as_bytes()),
            b64(new.as_bytes())
        ),
    )
}

/// V5.22: default 4 h and external backup off (D30); the new timeout holds after
/// locking and unlocking (it is in the encrypted body, not only in memory) and writes a
/// new generation.
#[test]
fn v5_22_default_4h_off_and_changeable_timeout_saved_in_the_vault() {
    let dir = Dir::new("cfg-padrao");
    let (d, _) = test_vault(&dir);
    assert_eq!(settings(&d), (DEFAULT_LOCK_MINUTES, false));
    assert_eq!(DEFAULT_LOCK_MINUTES, 240);
    let before = generation_on_disk(&dir.paths().vault, PASSWORD);
    assert!(ok(&lock_timeout(&d, Role::App, 30)));
    assert_eq!(settings(&d), (30, false));
    assert_eq!(generation_on_disk(&dir.paths().vault, PASSWORD), before + 1);
    d.lock();
    assert!(ok(&unlock(&d, PASSWORD)));
    assert_eq!(settings(&d), (30, false));
    // The D45 bounds pass; outside them it does not even reach the daemon.
    assert!(ok(&lock_timeout(&d, Role::App, MIN_LOCK_MINUTES)));
    assert!(ok(&lock_timeout(&d, Role::App, MAX_LOCK_MINUTES)));
    for out_of_range in [0, MAX_LOCK_MINUTES + 1, u32::MAX] {
        assert_eq!(
            error(&lock_timeout(&d, Role::App, out_of_range)),
            Some(ResponseError::InvalidRequest),
            "{out_of_range}"
        );
    }
    assert_eq!(settings(&d), (MAX_LOCK_MINUTES, false));
}

/// Attack (S9, D5): the agent and the extension neither read the settings nor change
/// the PIN, the password or the lock. Nothing changes.
#[test]
fn attack_agent_and_extension_neither_read_nor_change_the_settings() {
    let dir = Dir::new("cfg-papel");
    let (d, _) = test_vault(&dir);
    let generation = generation_on_disk(&dir.paths().vault, PASSWORD);
    for role in [Role::Agent, Role::Extension] {
        let r = request(&d, role, "configuracao", "{}");
        assert_eq!(error(&r), Some(ResponseError::Unauthorized), "{role:?}");
        let r = lock_timeout(&d, role, 1);
        assert_eq!(error(&r), Some(ResponseError::Unauthorized), "{role:?}");
        let r = change_pin(&d, role, PIN, "2468");
        assert_eq!(error(&r), Some(ResponseError::Unauthorized), "{role:?}");
        let r = change_password(&d, role, PASSWORD, "falso-nova-mestra-02");
        assert_eq!(error(&r), Some(ResponseError::Unauthorized), "{role:?}");
    }
    assert_eq!(settings(&d), (DEFAULT_LOCK_MINUTES, false));
    assert_eq!(generation_on_disk(&dir.paths().vault, PASSWORD), generation);
    // The PIN and the password are still the same.
    assert!(ok(&set_level(&d, Role::App, "t-aprov", "livre", PIN)));
    d.lock();
    assert!(ok(&unlock(&d, PASSWORD)));
}

/// Attack: locked, the settings do not go out (not even to the app).
#[test]
fn attack_settings_with_the_vault_locked() {
    let dir = Dir::new("cfg-travado");
    let (d, _) = test_vault(&dir);
    d.lock();
    for (op, data) in [
        ("configuracao", "{}".to_owned()),
        ("tempo_trava", r#"{"minutos":30}"#.to_owned()),
    ] {
        let r = request(&d, Role::App, op, &data);
        assert_eq!(error(&r), Some(ResponseError::Locked), "{op}");
    }
    assert_eq!(
        error(&change_pin(&d, Role::App, PIN, "2468")),
        Some(ResponseError::Locked)
    );
}

/// Attack (D30, S1): the settings reply does not carry the PIN hash, the salt, the
/// PIN or the password; only the three fields (Phase 49a: `backup_local`).
#[test]
fn attack_settings_reply_without_hash_or_pin() {
    let dir = Dir::new("cfg-hash");
    let (d, _) = test_vault(&dir);
    let r = request(&d, Role::App, "configuracao", "{}");
    let bytes = encoded(Role::App, &r);
    let text = String::from_utf8(bytes.clone()).unwrap();
    for needle in [
        "argon2",
        "$m=",
        "hash",
        "uuid",
        PIN,
        PASSWORD,
        &b64(PIN.as_bytes()),
    ] {
        assert!(!contains(&bytes, needle.as_bytes()), "{needle}: {text}");
    }
    assert!(
        text.ends_with(
            r#""dados":{"minutos_trava":240,"backup_externo":false,"backup_local":true}}"#
        ),
        "{text}"
    );
}

/// V5.22 (S9): changing the PIN asks for the current PIN. A wrong one changes nothing
/// and counts towards the lock on the 5th error; a right one writes a new generation,
/// and from then on only the new PIN approves.
#[test]
fn v5_22_change_pin_asks_for_the_current_pin() {
    let dir = Dir::new("cfg-pin");
    let (d, _) = test_vault(&dir);
    let generation = generation_on_disk(&dir.paths().vault, PASSWORD);
    assert_eq!(
        error(&change_pin(&d, Role::App, WRONG_PIN, "2468")),
        Some(ResponseError::WrongPin)
    );
    assert_eq!(generation_on_disk(&dir.paths().vault, PASSWORD), generation);
    assert!(ok(&set_level(&d, Role::App, "t-aprov", "livre", PIN)));
    assert!(ok(&change_pin(&d, Role::App, PIN, "2468")));
    assert_eq!(
        generation_on_disk(&dir.paths().vault, PASSWORD),
        generation + 2,
        "mudar_nivel e trocar_pin gravam"
    );
    assert_eq!(
        error(&set_level(&d, Role::App, "t-aprov", "com-aprovacao", PIN)),
        Some(ResponseError::WrongPin)
    );
    assert!(ok(&set_level(
        &d,
        Role::App,
        "t-aprov",
        "com-aprovacao",
        "2468"
    )));
    // The new PIN is on disk: it holds after locking and unlocking.
    d.lock();
    assert!(ok(&unlock(&d, PASSWORD)));
    assert!(ok(&set_level(&d, Role::App, "t-aprov", "livre", "2468")));
    // A new PIN out of format does not even reach the daemon.
    for new in ["135", "1357901", "13579a", ""] {
        assert_eq!(
            error(&change_pin(&d, Role::App, "2468", new)),
            Some(ResponseError::InvalidRequest),
            "{new:?}"
        );
    }
}

/// Attack (S9): guessing the current PIN through the change locks on the 5th error,
/// like the level change.
#[test]
fn attack_guessing_the_pin_through_the_change_locks_on_the_fifth_error() {
    let dir = Dir::new("cfg-pin5");
    let (d, _) = test_vault(&dir);
    for i in 0..5 {
        let r = change_pin(&d, Role::App, &format!("00{i:02}"), "2468");
        assert_eq!(error(&r), Some(ResponseError::WrongPin));
    }
    assert!(d.is_locked());
    assert!(ok(&unlock(&d, PASSWORD)));
    // Nothing changed: the PIN is the old one.
    assert!(ok(&set_level(&d, Role::App, "t-aprov", "livre", PIN)));
}

/// V5.22: changing the master password asks for the current one. A wrong one changes
/// nothing; a right one writes a new generation and the vault opens only with the new
/// one; the lock timeout and the PIN stay.
#[test]
fn v5_22_change_password_asks_for_the_current_one_and_keeps_the_settings() {
    let dir = Dir::new("cfg-senha");
    let (d, _) = test_vault(&dir);
    assert!(ok(&lock_timeout(&d, Role::App, 60)));
    let generation = generation_on_disk(&dir.paths().vault, PASSWORD);
    assert_eq!(
        error(&change_password(
            &d,
            Role::App,
            "falso-errada-mestra-01",
            "falso-nova-mestra-02"
        )),
        Some(ResponseError::WrongPassword)
    );
    assert!(!d.is_locked());
    assert_eq!(generation_on_disk(&dir.paths().vault, PASSWORD), generation);
    assert!(ok(&change_password(
        &d,
        Role::App,
        PASSWORD,
        "falso-nova-mestra-02"
    )));
    assert!(generation_on_disk(&dir.paths().vault, "falso-nova-mestra-02") > generation);
    d.lock();
    assert_eq!(
        error(&unlock(&d, PASSWORD)),
        Some(ResponseError::WrongPassword)
    );
    assert!(ok(&unlock(&d, "falso-nova-mestra-02")));
    assert_eq!(settings(&d), (60, false));
    assert!(ok(&set_level(&d, Role::App, "t-aprov", "livre", PIN)));
}

/// Phase 39c: the 12-character minimum of the master password also holds in the daemon,
/// counted after NFC. Attack: an app-role client that skips the screen sends a short
/// password, or 11 accented letters decomposed (22 code points). Nothing is written, no
/// generation is burned and the current password stays valid.
#[test]
fn attack_short_new_master_password_refused_by_the_daemon() {
    let field = |r: &Response| match r {
        Response::Error(ResponseError::Validation(p)) => {
            assert!(p.iter().all(|p| p.reason.contains("12 caracteres")));
            p.iter().map(|p| p.field.clone()).collect::<Vec<_>>()
        }
        other => panic!("{other:?}"),
    };
    let disguised = "e\u{301}".repeat(11);

    // Creating the vault.
    let dir = Dir::new("cfg-senha-curta");
    let c = dir.paths();
    let (d, _) = daemon_at(c.clone());
    // (An empty one does not even get past the protocol: invalid request.)
    for short in ["falso-mest1", disguised.as_str()] {
        let r = request(
            &d,
            Role::App,
            "criar_cofre",
            &format!(
                r#"{{"senha":"{}","pin":"{}"}}"#,
                b64(short.as_bytes()),
                b64(PIN.as_bytes())
            ),
        );
        assert_eq!(field(&r), ["senha"], "{short:?}");
        assert!(!c.vault.exists());
        assert!(!c.state.exists());
    }

    // Changing the password.
    let dir = Dir::new("cfg-senha-curta-troca");
    let (d, _) = test_vault(&dir);
    let before = generations(&dir);
    for short in ["falso-mest1", disguised.as_str()] {
        assert_eq!(
            field(&change_password(&d, Role::App, PASSWORD, short)),
            ["nova"]
        );
    }
    assert_eq!(generations(&dir), before);
    d.lock();
    assert!(ok(&unlock(&d, PASSWORD)));
}

/// Phase 39c: the master password created composed opens decomposed (and the other way
/// round), through the same socket the app and the CLI use.
#[test]
fn composed_master_password_opens_decomposed_through_the_daemon() {
    let composed = "falso-mestra-\u{e9}\u{e7}\u{e3}o-01";
    let decomposed = "falso-mestra-e\u{301}c\u{327}a\u{303}o-01";
    let dir = Dir::new("cfg-senha-nfc");
    let (d, _) = test_vault(&dir);
    assert!(ok(&change_password(&d, Role::App, PASSWORD, decomposed)));
    d.lock();
    assert!(ok(&unlock(&d, composed)));
    d.lock();
    assert!(ok(&unlock(&d, decomposed)));
    assert!(ok(&change_password(&d, Role::App, composed, PASSWORD)));
}

/// Phase 55 (final re-audit, low): the password change read the vault again through
/// the standard library (`File::open`, follows links and waits for the other end of a
/// FIFO), after checking the identity through another `open`. A FIFO swapped in place
/// of the file between the two opens held the thread with the daemon state locked. Now
/// the change uses the bytes of the same fd as the check (`O_NOFOLLOW | O_NONBLOCK`).
/// Attack: a FIFO goes in and out of the vault's place nonstop while the app asks for
/// the change (with the wrong current password, so nothing is written); each request
/// returns at once.
#[test]
fn f55_attack_fifo_in_place_of_the_vault_does_not_hold_the_password_change() {
    use std::fs;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    let dir = Dir::new("troca-fifo");
    let (d, _) = test_vault(&dir);
    let d = Arc::new(d);
    let vault = dir.paths().vault;
    let real = dir.0.join("x-real");
    fs::hard_link(&vault, &real).unwrap();

    // A FIFO standing in place: refused at once and locks (the file changed outside
    // Ward).
    let fifo = dir.0.join("x-fifo");
    let st = std::process::Command::new("/usr/bin/mkfifo")
        .args(["-m", "600"])
        .arg(&fifo)
        .status()
        .unwrap();
    assert!(st.success());
    fs::rename(&fifo, &vault).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    {
        let d = d.clone();
        std::thread::spawn(move || {
            let _ = tx.send(change_password(
                &d,
                Role::App,
                PASSWORD,
                "falso-nova-mestra-55",
            ));
        });
    }
    let r = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("a troca de senha ficou presa no FIFO");
    assert_eq!(error(&r), Some(ResponseError::VaultChanged), "{r:?}");
    assert!(d.is_locked());
    let back = dir.0.join("x-volta");
    fs::hard_link(&real, &back).unwrap();
    fs::rename(&back, &vault).unwrap();
    assert!(ok(&unlock(&d, PASSWORD)));

    // A FIFO going in and out of place during the requests.
    let stop = Arc::new(AtomicBool::new(false));
    let swapper = {
        let (dir0, vault, real, stop) = (dir.0.clone(), vault.clone(), real.clone(), stop.clone());
        std::thread::spawn(move || {
            let mut n = 0u64;
            while !stop.load(Ordering::Acquire) {
                let fifo = dir0.join(format!("x-fifo-{n}"));
                let st = std::process::Command::new("/usr/bin/mkfifo")
                    .args(["-m", "600"])
                    .arg(&fifo)
                    .status()
                    .unwrap();
                assert!(st.success());
                fs::rename(&fifo, &vault).unwrap();
                std::thread::sleep(Duration::from_millis(3));
                let back = dir0.join(format!("x-volta-{n}"));
                fs::hard_link(&real, &back).unwrap();
                fs::rename(&back, &vault).unwrap();
                std::thread::sleep(Duration::from_millis(3));
                n += 1;
            }
            n
        })
    };
    let end = Instant::now() + Duration::from_secs(6);
    let mut requests = 0;
    let mut read_ok = 0;
    while Instant::now() < end {
        let (tx, rx) = std::sync::mpsc::channel();
        let d2 = d.clone();
        std::thread::spawn(move || {
            let r = change_password(
                &d2,
                Role::App,
                "falso-senha-errada-55",
                "falso-nova-mestra-55",
            );
            if error(&r) != Some(ResponseError::WrongPassword) {
                // Locked (the file changed or too many errors): unlock again (the
                // unlock also reads without following a FIFO; it tries until it gets
                // the real one).
                for _ in 0..50 {
                    if ok(&unlock(&d2, PASSWORD)) {
                        break;
                    }
                }
            }
            let _ = tx.send(r);
        });
        let r = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("a troca de senha ficou presa num FIFO (o daemon parou)");
        assert!(
            matches!(
                error(&r),
                Some(
                    ResponseError::WrongPassword
                        | ResponseError::VaultChanged
                        | ResponseError::Locked
                )
            ),
            "{r:?}"
        );
        if error(&r) == Some(ResponseError::WrongPassword) {
            read_ok += 1;
        }
        requests += 1;
    }
    stop.store(true, Ordering::Release);
    let swaps = swapper.join().unwrap();
    assert!(
        swaps > 50 && requests > 5 && read_ok >= 3,
        "{swaps} trocas, {requests} pedidos, {read_ok} lidos"
    );
    // The real file in place: the change with the right password works.
    if d.is_locked() {
        assert!(ok(&unlock(&d, PASSWORD)));
    }
    let r = change_password(&d, Role::App, PASSWORD, "falso-nova-mestra-55");
    assert!(ok(&r), "{r:?}");
}
