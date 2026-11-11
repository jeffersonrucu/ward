//! D4, D65 b and c (Phase 22c), in the daemon core: reveal, copy and TOTP of
//! the app role (without a PIN, without approval and without looking at the level) and
//! the summary in the app's list. The agent and the extension get "not authorized"
//! (V3.39); the agent's list stays without level or summary (V3.40). Only fake values,
//! in a test vault of its own.

mod common;

use std::time::Duration;

use common::{Dir, PIN, SECRETS, b64, error, ok, request};
use ward_core::protocol::{self, Request, Response, ResponseError, Role, Target};
use ward_core::totp::Totp;
use wardd::core::{Clock, Daemon};

/// Fake TOTP seed (the one of `acme-totp-dev`, D28).
const SEED: &str = "JBSWY3DPEHPK3PXP";
/// Value of the test entry's secret extra.
const EXTRA: &str = "falso-extra-ward-10";

/// Fixed wall clock (or unreadable, with `None`).
struct FixedClock(Option<u64>);

impl Clock for FixedClock {
    fn sleep(&self, _d: Duration) {}

    fn now(&self) -> Option<Duration> {
        Some(Duration::from_secs(1000))
    }

    fn unix_seconds(&self) -> Option<u64> {
        self.0
    }
}

fn with_totp(name: &str) -> String {
    format!(
        r#"{{"entry":{{"name":"{name}","group":"clientex","environment":"dev","type":"login","fields":{{"url":"https://clientex.invalid/login","username":"admin","password":"{}","totp_seed":"{}"}},"extras":[{{"name":"CHAVE","secret":true,"value":"{}"}},{{"name":"REGIAO","secret":false,"value":"{}"}}]}}}}"#,
        b64(SECRETS[0].as_bytes()),
        b64(SEED.as_bytes()),
        b64(EXTRA.as_bytes()),
        b64(b"r-01"),
    )
}

/// The common test vault in a daemon with the given clock, plus
/// `t-totp` (login with seed and extras), blocked.
fn vault(dir: &Dir, unix: Option<u64>) -> Daemon {
    let d = Daemon::new(dir.paths(), Box::new(FixedClock(unix)));
    let create = request(
        &d,
        Role::App,
        "create_vault",
        &format!(
            r#"{{"password":"{}","pin":"{}"}}"#,
            b64(common::PASSWORD.as_bytes()),
            b64(PIN.as_bytes())
        ),
    );
    assert!(ok(&create), "{create:?}");
    for q in [
        common::login("t-aprov", SECRETS[0]),
        common::login("t-bloq", SECRETS[3]),
        common::server("s1"),
        with_totp("t-totp"),
    ] {
        let r = request(&d, Role::App, "save_entry", &q);
        assert!(ok(&r), "{r:?}");
    }
    for n in ["t-bloq", "t-totp"] {
        assert!(ok(&common::set_level(&d, Role::App, n, "blocked", PIN)));
    }
    d
}

fn field_target(c: &str) -> String {
    format!(r#""field":"{c}""#)
}

fn value(r: &Response) -> Vec<u8> {
    match r {
        Response::Secret(s) => s.expose().to_vec(),
        other => panic!("esperava segredo: {:?}", other.kind()),
    }
}

fn value_request(entry: &str, target: &str) -> String {
    format!(r#"{{"entry":"{entry}",{target}}}"#)
}

#[test]
fn v3_39_app_reveals_and_copies_without_pin_or_approval_even_the_blocked_one() {
    let dir = Dir::new("valor-app");
    let d = vault(&dir, Some(59));
    for op in ["reveal", "copy"] {
        // Approval and blocked: the app gets the value, without a PIN in the request
        // (the protocol does not even accept one) and without a window (no approval
        // engine turned on).
        let r = request(
            &d,
            Role::App,
            op,
            &value_request("t-aprov", &field_target("password")),
        );
        assert_eq!(value(&r), SECRETS[0].as_bytes(), "{op}");
        let r = request(
            &d,
            Role::App,
            op,
            &value_request("t-bloq", &field_target("password")),
        );
        assert_eq!(value(&r), SECRETS[3].as_bytes(), "{op}");
        // The TOTP seed and the secret extra are secrets of the entry too.
        let r = request(
            &d,
            Role::App,
            op,
            &value_request("t-totp", &field_target("totp_seed")),
        );
        assert_eq!(value(&r), SEED.as_bytes());
        let r = request(
            &d,
            Role::App,
            op,
            &value_request("t-totp", r#""extra":"CHAVE""#),
        );
        assert_eq!(value(&r), EXTRA.as_bytes());
        let r = request(
            &d,
            Role::App,
            op,
            &value_request("s1", &field_target("password")),
        );
        assert_eq!(value(&r), SECRETS[1].as_bytes());

        // Field the type does not have, plain extra, extra that does not exist: "field
        // does not exist"; entry that does not exist: "entry does not exist".
        for (entry, target) in [
            ("t-aprov", field_target("private_key")),
            ("t-aprov", field_target("totp_seed")),
            ("t-aprov", field_target("token")),
            ("s1", field_target("passphrase")),
            ("t-totp", r#""extra":"REGIAO""#.to_owned()),
            ("t-totp", r#""extra":"NAO_EXISTE""#.to_owned()),
        ] {
            let r = request(&d, Role::App, op, &value_request(entry, &target));
            assert_eq!(
                error(&r),
                Some(ResponseError::FieldNotFound),
                "{op} {entry} {target}"
            );
        }
        let r = request(
            &d,
            Role::App,
            op,
            &value_request("nao-existe", &field_target("password")),
        );
        assert_eq!(error(&r), Some(ResponseError::EntryNotFound));
    }
    // The answer encoded for the app carries the value in base64; for the agent, it
    // does not go out.
    let r = request(
        &d,
        Role::App,
        "reveal",
        &value_request("t-aprov", &field_target("password")),
    );
    assert!(protocol::encode_response(Role::App, &r).is_ok());
    assert!(protocol::encode_response(Role::Agent, &r).is_err());
    assert!(protocol::encode_response(Role::Extension, &r).is_err());

    // Locked: "locked", without a value.
    d.lock();
    for op in ["reveal", "copy"] {
        let r = request(
            &d,
            Role::App,
            op,
            &value_request("t-aprov", &field_target("password")),
        );
        assert_eq!(error(&r), Some(ResponseError::Locked), "{op}");
    }
    let r = request(&d, Role::App, "totp", r#"{"entry":"t-totp"}"#);
    assert_eq!(error(&r), Some(ResponseError::Locked));
}

#[test]
fn v3_39_app_totp_by_the_daemon_clock() {
    let dir = Dir::new("totp-app");
    // RFC 6238: at t = 59 s the period is 1, with 1 s left.
    let d = vault(&dir, Some(59));
    let expected = Totp::from_seed(&ward_core::entry::Secret::from_text(SEED.into()))
        .unwrap()
        .code_at(59);
    match request(&d, Role::App, "totp", r#"{"entry":"t-totp"}"#) {
        Response::TotpCode { code, remaining } => {
            assert_eq!(code.as_str(), expected.code());
            assert_eq!(code.len(), 6);
            assert_eq!(remaining, 1);
        }
        other => panic!("esperava código: {:?}", other.kind()),
    }
    // No seed: "field does not exist"; no entry: "entry does not exist".
    let r = request(&d, Role::App, "totp", r#"{"entry":"t-aprov"}"#);
    assert_eq!(error(&r), Some(ResponseError::FieldNotFound));
    let r = request(&d, Role::App, "totp", r#"{"entry":"nao-existe"}"#);
    assert_eq!(error(&r), Some(ResponseError::EntryNotFound));

    // Unreadable wall clock: error, never a code (fails closed).
    let dir2 = Dir::new("totp-sem-relogio");
    let d2 = vault(&dir2, None);
    let r = request(&d2, Role::App, "totp", r#"{"entry":"t-totp"}"#);
    assert_eq!(error(&r), Some(ResponseError::Internal));
}

#[test]
fn attack_v3_39_agent_and_extension_neither_reveal_nor_copy_nor_take_totp() {
    let dir = Dir::new("valor-agente");
    let d = vault(&dir, Some(59));
    let requests = [
        (
            "reveal",
            value_request("t-aprov", &field_target("password")),
        ),
        ("reveal", value_request("t-bloq", &field_target("password"))),
        ("copy", value_request("t-aprov", &field_target("password"))),
        ("copy", value_request("t-bloq", &field_target("password"))),
        ("totp", r#"{"entry":"t-totp"}"#.to_owned()),
        // With a PIN in the request (the agent that thinks the PIN would open it): the
        // same.
        (
            "reveal",
            format!(
                r#"{{"entry":"t-aprov","field":"password","pin":"{}"}}"#,
                b64(PIN.as_bytes())
            ),
        ),
    ];
    for role in [Role::Agent, Role::Extension] {
        for (op, data) in &requests {
            // Through the protocol (as the socket reads it).
            let r = request(&d, role, op, data);
            assert_eq!(
                error(&r),
                Some(ResponseError::Unauthorized),
                "{} {op}",
                role.name()
            );
            let q = common::encoded(role, &r);
            for s in SECRETS.iter().chain([&SEED, &EXTRA]) {
                assert!(!common::contains(&q, s.as_bytes()));
                assert!(!common::contains(&q, b64(s.as_bytes()).as_bytes()));
            }
        }
        // Request already decoded, handed straight to the core (second layer).
        for p in [
            Request::Reveal {
                entry: "t-aprov".into(),
                target: Target::Field("password".into()),
            },
            Request::Copy {
                entry: "t-bloq".into(),
                target: Target::Field("password".into()),
            },
            Request::Totp {
                entry: "t-totp".into(),
            },
        ] {
            let r = d.serve(role, p);
            assert_eq!(error(&r), Some(ResponseError::Unauthorized));
        }
    }
    // The vault stays unlocked and the app still gets values (nothing locked or used
    // up a PIN).
    let r = request(
        &d,
        Role::App,
        "reveal",
        &value_request("t-aprov", &field_target("password")),
    );
    assert_eq!(value(&r), SECRETS[0].as_bytes());
}

#[test]
fn v3_40_app_list_with_summary_and_agent_list_without_level_or_summary() {
    let dir = Dir::new("lista-resumo");
    let d = vault(&dir, Some(59));
    let Response::List(items) = request(&d, Role::App, "list", "{}") else {
        panic!("não é lista")
    };
    assert_eq!(items.len(), 4);
    let t = items.iter().find(|i| i.name == "t-totp").unwrap();
    let r = t.summary.as_ref().unwrap();
    assert!(
        r.fields
            .contains(&("username".to_owned(), "admin".to_owned()))
    );
    assert_eq!(
        r.secrets,
        vec!["password".to_owned(), "totp_seed".to_owned()]
    );
    assert_eq!((r.extras, r.attachments), (2, 0));
    let q = common::encoded(Role::App, &Response::List(items));
    for s in SECRETS.iter().chain([&SEED, &EXTRA]) {
        assert!(
            !common::contains(&q, s.as_bytes()),
            "segredo na lista do app"
        );
        assert!(!common::contains(&q, b64(s.as_bytes()).as_bytes()));
    }
    // Agent: the non-blocked ones, without level, summary or user.
    let r = request(&d, Role::Agent, "list", "{}");
    let q = common::encoded(Role::Agent, &r);
    let Response::List(items) = r else {
        panic!("não é lista")
    };
    let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["s1", "t-aprov"]);
    assert!(
        items
            .iter()
            .all(|i| i.level.is_none() && i.summary.is_none())
    );
    for needle in [
        &b"\"summary\""[..],
        b"\"level\"",
        b"\"username\"",
        b"deploy",
    ] {
        assert!(!common::contains(&q, needle));
    }
}
