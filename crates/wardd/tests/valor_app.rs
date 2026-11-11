//! D4, D65 b e c (Fase 22c), no núcleo do daemon: `revelar`, `copiar` e `totp` do papel
//! app (sem PIN, sem aprovação e sem olhar o nível) e o resumo na `lista` do app. O
//! agente e a extensão recebem "não autorizado" (V3.39); a lista do agente segue sem
//! nível nem resumo (V3.40). Só valores falsos, num cofre de teste próprio.

mod common;

use std::time::Duration;

use common::{Dir, PIN, SECRETS, b64, error, ok, request};
use ward_core::protocolo::{self, Alvo, ErroResposta, Papel, Pedido, Resposta};
use ward_core::totp::Totp;
use wardd::nucleo::{Daemon, Relogio};

/// Semente TOTP falsa (a de `acme-totp-dev`, D28).
const SEMENTE: &str = "JBSWY3DPEHPK3PXP";
/// Valor do extra secreto da entrada de teste.
const EXTRA: &str = "falso-extra-ward-10";

/// Relógio de parede fixo (ou ilegível, com `None`).
struct RelogioFixo(Option<u64>);

impl Relogio for RelogioFixo {
    fn dormir(&self, _d: Duration) {}

    fn agora(&self) -> Option<Duration> {
        Some(Duration::from_secs(1000))
    }

    fn segundos_unix(&self) -> Option<u64> {
        self.0
    }
}

fn com_totp(nome: &str) -> String {
    format!(
        r#"{{"entrada":{{"nome":"{nome}","grupo":"clientex","ambiente":"dev","tipo":"login","campos":{{"url":"https://clientex.invalid/login","usuario":"admin","senha":"{}","totp_semente":"{}"}},"extras":[{{"nome":"CHAVE","segredo":true,"valor":"{}"}},{{"nome":"REGIAO","segredo":false,"valor":"{}"}}]}}}}"#,
        b64(SECRETS[0].as_bytes()),
        b64(SEMENTE.as_bytes()),
        b64(EXTRA.as_bytes()),
        b64(b"r-01"),
    )
}

/// O cofre de `common::test_vault` num daemon com o relógio dado, mais `t-totp`
/// (login com semente e extras) bloqueado.
fn cofre(dir: &Dir, unix: Option<u64>) -> Daemon {
    let d = Daemon::novo(dir.paths(), Box::new(RelogioFixo(unix)));
    let criar = request(
        &d,
        Papel::App,
        "criar_cofre",
        &format!(
            r#"{{"senha":"{}","pin":"{}"}}"#,
            b64(common::PASSWORD.as_bytes()),
            b64(PIN.as_bytes())
        ),
    );
    assert!(ok(&criar), "{criar:?}");
    for q in [
        common::login("t-aprov", SECRETS[0]),
        common::login("t-bloq", SECRETS[3]),
        common::server("s1"),
        com_totp("t-totp"),
    ] {
        let r = request(&d, Papel::App, "salvar_entrada", &q);
        assert!(ok(&r), "{r:?}");
    }
    for n in ["t-bloq", "t-totp"] {
        assert!(ok(&common::set_level(&d, Papel::App, n, "bloqueado", PIN)));
    }
    d
}

fn alvo_campo(c: &str) -> String {
    format!(r#""campo":"{c}""#)
}

fn valor(r: &Resposta) -> Vec<u8> {
    match r {
        Resposta::Segredo(s) => s.expose().to_vec(),
        outra => panic!("esperava segredo: {:?}", outra.tipo()),
    }
}

fn pedido_valor(entrada: &str, alvo: &str) -> String {
    format!(r#"{{"entrada":"{entrada}",{alvo}}}"#)
}

#[test]
fn v3_39_app_revela_e_copia_sem_pin_nem_aprovacao_ate_a_bloqueada() {
    let dir = Dir::new("valor-app");
    let d = cofre(&dir, Some(59));
    for op in ["revelar", "copiar"] {
        // Com aprovação e bloqueada: o app recebe o valor, sem PIN no pedido (o
        // protocolo nem aceita um) e sem janela (sem motor de aprovação ligado).
        let r = request(
            &d,
            Papel::App,
            op,
            &pedido_valor("t-aprov", &alvo_campo("senha")),
        );
        assert_eq!(valor(&r), SECRETS[0].as_bytes(), "{op}");
        let r = request(
            &d,
            Papel::App,
            op,
            &pedido_valor("t-bloq", &alvo_campo("senha")),
        );
        assert_eq!(valor(&r), SECRETS[3].as_bytes(), "{op}");
        // Semente TOTP e extra secreto também são segredos da entrada.
        let r = request(
            &d,
            Papel::App,
            op,
            &pedido_valor("t-totp", &alvo_campo("totp_semente")),
        );
        assert_eq!(valor(&r), SEMENTE.as_bytes());
        let r = request(
            &d,
            Papel::App,
            op,
            &pedido_valor("t-totp", r#""extra":"CHAVE""#),
        );
        assert_eq!(valor(&r), EXTRA.as_bytes());
        let r = request(
            &d,
            Papel::App,
            op,
            &pedido_valor("s1", &alvo_campo("senha")),
        );
        assert_eq!(valor(&r), SECRETS[1].as_bytes());

        // Campo que o tipo não tem, extra comum, extra que não existe: "campo
        // inexistente"; entrada que não existe: "entrada inexistente".
        for (entrada, alvo) in [
            ("t-aprov", alvo_campo("chave_privada")),
            ("t-aprov", alvo_campo("totp_semente")),
            ("t-aprov", alvo_campo("token")),
            ("s1", alvo_campo("passphrase")),
            ("t-totp", r#""extra":"REGIAO""#.to_owned()),
            ("t-totp", r#""extra":"NAO_EXISTE""#.to_owned()),
        ] {
            let r = request(&d, Papel::App, op, &pedido_valor(entrada, &alvo));
            assert_eq!(
                error(&r),
                Some(ErroResposta::CampoInexistente),
                "{op} {entrada} {alvo}"
            );
        }
        let r = request(
            &d,
            Papel::App,
            op,
            &pedido_valor("nao-existe", &alvo_campo("senha")),
        );
        assert_eq!(error(&r), Some(ErroResposta::EntradaInexistente));
    }
    // A resposta codificada para o app leva o valor em base64; para o agente, não sai.
    let r = request(
        &d,
        Papel::App,
        "revelar",
        &pedido_valor("t-aprov", &alvo_campo("senha")),
    );
    assert!(protocolo::codificar_resposta(Papel::App, &r).is_ok());
    assert!(protocolo::codificar_resposta(Papel::Agente, &r).is_err());
    assert!(protocolo::codificar_resposta(Papel::Extensao, &r).is_err());

    // Travado: "travado", sem valor.
    d.travar();
    for op in ["revelar", "copiar"] {
        let r = request(
            &d,
            Papel::App,
            op,
            &pedido_valor("t-aprov", &alvo_campo("senha")),
        );
        assert_eq!(error(&r), Some(ErroResposta::Travado), "{op}");
    }
    let r = request(&d, Papel::App, "totp", r#"{"entrada":"t-totp"}"#);
    assert_eq!(error(&r), Some(ErroResposta::Travado));
}

#[test]
fn v3_39_totp_do_app_pelo_relogio_do_daemon() {
    let dir = Dir::new("totp-app");
    // RFC 6238: em t = 59 s o período é o 1, faltando 1 s.
    let d = cofre(&dir, Some(59));
    let esperado = Totp::from_seed(&ward_core::entry::Secret::from_text(SEMENTE.into()))
        .unwrap()
        .code_at(59);
    match request(&d, Papel::App, "totp", r#"{"entrada":"t-totp"}"#) {
        Resposta::CodigoTotp { codigo, restante } => {
            assert_eq!(codigo.as_str(), esperado.code());
            assert_eq!(codigo.len(), 6);
            assert_eq!(restante, 1);
        }
        outra => panic!("esperava código: {:?}", outra.tipo()),
    }
    // Sem semente: "campo inexistente"; sem entrada: "entrada inexistente".
    let r = request(&d, Papel::App, "totp", r#"{"entrada":"t-aprov"}"#);
    assert_eq!(error(&r), Some(ErroResposta::CampoInexistente));
    let r = request(&d, Papel::App, "totp", r#"{"entrada":"nao-existe"}"#);
    assert_eq!(error(&r), Some(ErroResposta::EntradaInexistente));

    // Relógio de parede ilegível: erro, nunca um código (falha fechada).
    let dir2 = Dir::new("totp-sem-relogio");
    let d2 = cofre(&dir2, None);
    let r = request(&d2, Papel::App, "totp", r#"{"entrada":"t-totp"}"#);
    assert_eq!(error(&r), Some(ErroResposta::Interno));
}

#[test]
fn ataque_v3_39_agente_e_extensao_nao_revelam_nem_copiam_nem_tiram_totp() {
    let dir = Dir::new("valor-agente");
    let d = cofre(&dir, Some(59));
    let pedidos = [
        ("revelar", pedido_valor("t-aprov", &alvo_campo("senha"))),
        ("revelar", pedido_valor("t-bloq", &alvo_campo("senha"))),
        ("copiar", pedido_valor("t-aprov", &alvo_campo("senha"))),
        ("copiar", pedido_valor("t-bloq", &alvo_campo("senha"))),
        ("totp", r#"{"entrada":"t-totp"}"#.to_owned()),
        // Com PIN no pedido (o agente que acha que o PIN abriria): o mesmo.
        (
            "revelar",
            format!(
                r#"{{"entrada":"t-aprov","campo":"senha","pin":"{}"}}"#,
                b64(PIN.as_bytes())
            ),
        ),
    ];
    for papel in [Papel::Agente, Papel::Extensao] {
        for (op, dados) in &pedidos {
            // Pelo protocolo (como o socket lê).
            let r = request(&d, papel, op, dados);
            assert_eq!(
                error(&r),
                Some(ErroResposta::NaoAutorizado),
                "{} {op}",
                papel.nome()
            );
            let q = common::encoded(papel, &r);
            for s in SECRETS.iter().chain([&SEMENTE, &EXTRA]) {
                assert!(!common::contains(&q, s.as_bytes()));
                assert!(!common::contains(&q, b64(s.as_bytes()).as_bytes()));
            }
        }
        // Pedido já decodificado entregue direto ao núcleo (segunda camada).
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
            let r = d.atender(papel, p);
            assert_eq!(error(&r), Some(ErroResposta::NaoAutorizado));
        }
    }
    // O cofre continua destravado e o app segue recebendo (nada travou nem gastou PIN).
    let r = request(
        &d,
        Papel::App,
        "revelar",
        &pedido_valor("t-aprov", &alvo_campo("senha")),
    );
    assert_eq!(valor(&r), SECRETS[0].as_bytes());
}

#[test]
fn v3_40_lista_do_app_com_resumo_e_do_agente_sem_nivel_nem_resumo() {
    let dir = Dir::new("lista-resumo");
    let d = cofre(&dir, Some(59));
    let Resposta::Lista(itens) = request(&d, Papel::App, "lista", "{}") else {
        panic!("não é lista")
    };
    assert_eq!(itens.len(), 4);
    let t = itens.iter().find(|i| i.nome == "t-totp").unwrap();
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
    let q = common::encoded(Papel::App, &Resposta::Lista(itens));
    for s in SECRETS.iter().chain([&SEMENTE, &EXTRA]) {
        assert!(
            !common::contains(&q, s.as_bytes()),
            "segredo na lista do app"
        );
        assert!(!common::contains(&q, b64(s.as_bytes()).as_bytes()));
    }
    // Agente: as não bloqueadas, sem nível, sem resumo, sem usuário.
    let r = request(&d, Papel::Agente, "lista", "{}");
    let q = common::encoded(Papel::Agente, &r);
    let Resposta::Lista(itens) = r else {
        panic!("não é lista")
    };
    let nomes: Vec<&str> = itens.iter().map(|i| i.nome.as_str()).collect();
    assert_eq!(nomes, ["s1", "t-aprov"]);
    assert!(
        itens
            .iter()
            .all(|i| i.nivel.is_none() && i.resumo.is_none())
    );
    for agulha in [&b"\"resumo\""[..], b"\"nivel\"", b"\"usuario\"", b"deploy"] {
        assert!(!common::contains(&q, agulha));
    }
}
