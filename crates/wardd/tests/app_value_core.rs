//! D4, D65 b and c (Phase 22c), in the daemon core: reveal, copy and TOTP of
//! the app role (without a PIN, without approval and without looking at the level) and
//! the summary in the app's list. The agent and the extension get "not authorized"
//! (V3.39); the agent's list stays without level or summary (V3.40). Only fake values,
//! in a test vault of its own.

mod comum;

use std::time::Duration;

use comum::{Dir, PIN, SEGREDOS, b64, erro, ok, pedir};
use ward_core::protocolo::{self, Alvo, ErroResposta, Papel, Pedido, Resposta};
use ward_core::totp::Totp;
use wardd::nucleo::{Daemon, Relogio};

/// Fake TOTP seed (the one of `acme-totp-dev`, D28).
const SEED: &str = "JBSWY3DPEHPK3PXP";
/// Value of the test entry's secret extra.
const EXTRA: &str = "falso-extra-ward-10";

/// Fixed wall clock (or unreadable, with `None`).
struct FixedClock(Option<u64>);

impl Relogio for FixedClock {
    fn dormir(&self, _d: Duration) {}

    fn agora(&self) -> Option<Duration> {
        Some(Duration::from_secs(1000))
    }

    fn segundos_unix(&self) -> Option<u64> {
        self.0
    }
}

fn with_totp(name: &str) -> String {
    format!(
        r#"{{"entrada":{{"nome":"{name}","grupo":"clientex","ambiente":"dev","tipo":"login","campos":{{"url":"https://clientex.invalid/login","usuario":"admin","senha":"{}","totp_semente":"{}"}},"extras":[{{"nome":"CHAVE","segredo":true,"valor":"{}"}},{{"nome":"REGIAO","segredo":false,"valor":"{}"}}]}}}}"#,
        b64(SEGREDOS[0].as_bytes()),
        b64(SEED.as_bytes()),
        b64(EXTRA.as_bytes()),
        b64(b"r-01"),
    )
}

/// The common test vault in a daemon with the given clock, plus
/// `t-totp` (login with seed and extras), blocked.
fn vault(dir: &Dir, unix: Option<u64>) -> Daemon {
    let d = Daemon::novo(dir.caminhos(), Box::new(FixedClock(unix)));
    let create = pedir(
        &d,
        Papel::App,
        "criar_cofre",
        &format!(
            r#"{{"senha":"{}","pin":"{}"}}"#,
            b64(comum::SENHA.as_bytes()),
            b64(PIN.as_bytes())
        ),
    );
    assert!(ok(&create), "{create:?}");
    for q in [
        comum::login("t-aprov", SEGREDOS[0]),
        comum::login("t-bloq", SEGREDOS[3]),
        comum::servidor("s1"),
        with_totp("t-totp"),
    ] {
        let r = pedir(&d, Papel::App, "salvar_entrada", &q);
        assert!(ok(&r), "{r:?}");
    }
    for n in ["t-bloq", "t-totp"] {
        assert!(ok(&comum::mudar_nivel(&d, Papel::App, n, "bloqueado", PIN)));
    }
    d
}

fn field_target(c: &str) -> String {
    format!(r#""campo":"{c}""#)
}

fn value(r: &Resposta) -> Vec<u8> {
    match r {
        Resposta::Segredo(s) => s.expose().to_vec(),
        other => panic!("esperava segredo: {:?}", other.tipo()),
    }
}

fn value_request(entry: &str, target: &str) -> String {
    format!(r#"{{"entrada":"{entry}",{target}}}"#)
}

#[test]
fn v3_39_app_reveals_and_copies_without_pin_or_approval_even_the_blocked_one() {
    let dir = Dir::novo("valor-app");
    let d = vault(&dir, Some(59));
    for op in ["revelar", "copiar"] {
        // Approval and blocked: the app gets the value, without a PIN in the request
        // (the protocol does not even accept one) and without a window (no approval
        // engine turned on).
        let r = pedir(
            &d,
            Papel::App,
            op,
            &value_request("t-aprov", &field_target("senha")),
        );
        assert_eq!(value(&r), SEGREDOS[0].as_bytes(), "{op}");
        let r = pedir(
            &d,
            Papel::App,
            op,
            &value_request("t-bloq", &field_target("senha")),
        );
        assert_eq!(value(&r), SEGREDOS[3].as_bytes(), "{op}");
        // The TOTP seed and the secret extra are secrets of the entry too.
        let r = pedir(
            &d,
            Papel::App,
            op,
            &value_request("t-totp", &field_target("totp_semente")),
        );
        assert_eq!(value(&r), SEED.as_bytes());
        let r = pedir(
            &d,
            Papel::App,
            op,
            &value_request("t-totp", r#""extra":"CHAVE""#),
        );
        assert_eq!(value(&r), EXTRA.as_bytes());
        let r = pedir(
            &d,
            Papel::App,
            op,
            &value_request("s1", &field_target("senha")),
        );
        assert_eq!(value(&r), SEGREDOS[1].as_bytes());

        // Field the type does not have, plain extra, extra that does not exist: "field
        // does not exist"; entry that does not exist: "entry does not exist".
        for (entry, target) in [
            ("t-aprov", field_target("chave_privada")),
            ("t-aprov", field_target("totp_semente")),
            ("t-aprov", field_target("token")),
            ("s1", field_target("passphrase")),
            ("t-totp", r#""extra":"REGIAO""#.to_owned()),
            ("t-totp", r#""extra":"NAO_EXISTE""#.to_owned()),
        ] {
            let r = pedir(&d, Papel::App, op, &value_request(entry, &target));
            assert_eq!(
                erro(&r),
                Some(ErroResposta::CampoInexistente),
                "{op} {entry} {target}"
            );
        }
        let r = pedir(
            &d,
            Papel::App,
            op,
            &value_request("nao-existe", &field_target("senha")),
        );
        assert_eq!(erro(&r), Some(ErroResposta::EntradaInexistente));
    }
    // The answer encoded for the app carries the value in base64; for the agent, it
    // does not go out.
    let r = pedir(
        &d,
        Papel::App,
        "revelar",
        &value_request("t-aprov", &field_target("senha")),
    );
    assert!(protocolo::codificar_resposta(Papel::App, &r).is_ok());
    assert!(protocolo::codificar_resposta(Papel::Agente, &r).is_err());
    assert!(protocolo::codificar_resposta(Papel::Extensao, &r).is_err());

    // Locked: "locked", without a value.
    d.travar();
    for op in ["revelar", "copiar"] {
        let r = pedir(
            &d,
            Papel::App,
            op,
            &value_request("t-aprov", &field_target("senha")),
        );
        assert_eq!(erro(&r), Some(ErroResposta::Travado), "{op}");
    }
    let r = pedir(&d, Papel::App, "totp", r#"{"entrada":"t-totp"}"#);
    assert_eq!(erro(&r), Some(ErroResposta::Travado));
}

#[test]
fn v3_39_app_totp_by_the_daemon_clock() {
    let dir = Dir::novo("totp-app");
    // RFC 6238: at t = 59 s the period is 1, with 1 s left.
    let d = vault(&dir, Some(59));
    let expected = Totp::from_seed(&ward_core::entry::Secret::from_text(SEED.into()))
        .unwrap()
        .code_at(59);
    match pedir(&d, Papel::App, "totp", r#"{"entrada":"t-totp"}"#) {
        Resposta::CodigoTotp { codigo, restante } => {
            assert_eq!(codigo.as_str(), expected.code());
            assert_eq!(codigo.len(), 6);
            assert_eq!(restante, 1);
        }
        other => panic!("esperava código: {:?}", other.tipo()),
    }
    // No seed: "field does not exist"; no entry: "entry does not exist".
    let r = pedir(&d, Papel::App, "totp", r#"{"entrada":"t-aprov"}"#);
    assert_eq!(erro(&r), Some(ErroResposta::CampoInexistente));
    let r = pedir(&d, Papel::App, "totp", r#"{"entrada":"nao-existe"}"#);
    assert_eq!(erro(&r), Some(ErroResposta::EntradaInexistente));

    // Unreadable wall clock: error, never a code (fails closed).
    let dir2 = Dir::novo("totp-sem-relogio");
    let d2 = vault(&dir2, None);
    let r = pedir(&d2, Papel::App, "totp", r#"{"entrada":"t-totp"}"#);
    assert_eq!(erro(&r), Some(ErroResposta::Interno));
}

#[test]
fn attack_v3_39_agent_and_extension_neither_reveal_nor_copy_nor_take_totp() {
    let dir = Dir::novo("valor-agente");
    let d = vault(&dir, Some(59));
    let requests = [
        ("revelar", value_request("t-aprov", &field_target("senha"))),
        ("revelar", value_request("t-bloq", &field_target("senha"))),
        ("copiar", value_request("t-aprov", &field_target("senha"))),
        ("copiar", value_request("t-bloq", &field_target("senha"))),
        ("totp", r#"{"entrada":"t-totp"}"#.to_owned()),
        // With a PIN in the request (the agent that thinks the PIN would open it): the
        // same.
        (
            "revelar",
            format!(
                r#"{{"entrada":"t-aprov","campo":"senha","pin":"{}"}}"#,
                b64(PIN.as_bytes())
            ),
        ),
    ];
    for role in [Papel::Agente, Papel::Extensao] {
        for (op, data) in &requests {
            // Through the protocol (as the socket reads it).
            let r = pedir(&d, role, op, data);
            assert_eq!(
                erro(&r),
                Some(ErroResposta::NaoAutorizado),
                "{} {op}",
                role.nome()
            );
            let q = comum::codificado(role, &r);
            for s in SEGREDOS.iter().chain([&SEED, &EXTRA]) {
                assert!(!comum::contem(&q, s.as_bytes()));
                assert!(!comum::contem(&q, b64(s.as_bytes()).as_bytes()));
            }
        }
        // Request already decoded, handed straight to the core (second layer).
        for p in [
            Pedido::Revelar {
                entrada: "t-aprov".into(),
                alvo: Alvo::Campo("senha".into()),
            },
            Pedido::Copiar {
                entrada: "t-bloq".into(),
                alvo: Alvo::Campo("senha".into()),
            },
            Pedido::Totp {
                entrada: "t-totp".into(),
            },
        ] {
            let r = d.atender(role, p);
            assert_eq!(erro(&r), Some(ErroResposta::NaoAutorizado));
        }
    }
    // The vault stays unlocked and the app still gets values (nothing locked or used
    // up a PIN).
    let r = pedir(
        &d,
        Papel::App,
        "revelar",
        &value_request("t-aprov", &field_target("senha")),
    );
    assert_eq!(value(&r), SEGREDOS[0].as_bytes());
}

#[test]
fn v3_40_app_list_with_summary_and_agent_list_without_level_or_summary() {
    let dir = Dir::novo("lista-resumo");
    let d = vault(&dir, Some(59));
    let Resposta::Lista(items) = pedir(&d, Papel::App, "lista", "{}") else {
        panic!("não é lista")
    };
    assert_eq!(items.len(), 4);
    let t = items.iter().find(|i| i.nome == "t-totp").unwrap();
    let r = t.resumo.as_ref().unwrap();
    assert!(
        r.campos
            .contains(&("usuario".to_owned(), "admin".to_owned()))
    );
    assert_eq!(
        r.segredos,
        vec!["senha".to_owned(), "totp_semente".to_owned()]
    );
    assert_eq!((r.extras, r.anexos), (2, 0));
    let q = comum::codificado(Papel::App, &Resposta::Lista(items));
    for s in SEGREDOS.iter().chain([&SEED, &EXTRA]) {
        assert!(!comum::contem(&q, s.as_bytes()), "segredo na lista do app");
        assert!(!comum::contem(&q, b64(s.as_bytes()).as_bytes()));
    }
    // Agent: the non-blocked ones, without level, summary or user.
    let r = pedir(&d, Papel::Agente, "lista", "{}");
    let q = comum::codificado(Papel::Agente, &r);
    let Resposta::Lista(items) = r else {
        panic!("não é lista")
    };
    let names: Vec<&str> = items.iter().map(|i| i.nome.as_str()).collect();
    assert_eq!(names, ["s1", "t-aprov"]);
    assert!(
        items
            .iter()
            .all(|i| i.nivel.is_none() && i.resumo.is_none())
    );
    for needle in [&b"\"resumo\""[..], b"\"nivel\"", b"\"usuario\"", b"deploy"] {
        assert!(!comum::contem(&q, needle));
    }
}
