//! Fase 40 (D5, D9; V6.1, V6.2), no núcleo do daemon: `sugerir` e `preencher` do papel
//! extensão pela origem da aba. Ataques: origem parecida (outro host, porta, esquema,
//! subdomínio, userinfo), entrada de outro tipo, agente e app pedindo as operações da
//! extensão, e a extensão pedindo as do app e do agente. Só valores falsos, num cofre de
//! teste próprio.

mod comum;

use std::time::Duration;

use comum::{Dir, PIN, SEGREDOS, b64, erro, ok, ped, pedir};
use ward_core::protocolo::{self, ErroResposta, Operacao, Papel, Resposta};
use ward_core::totp::Totp;
use wardd::nucleo::{Daemon, Relogio};

/// Semente TOTP falsa (a de `acme-totp-dev`, D28).
const SEMENTE: &str = "JBSWY3DPEHPK3PXP";
const AGORA: u64 = 1_700_000_000;

struct RelogioFixo;

impl Relogio for RelogioFixo {
    fn dormir(&self, _d: Duration) {}

    fn agora(&self) -> Option<Duration> {
        Some(Duration::from_secs(1000))
    }

    fn segundos_unix(&self) -> Option<u64> {
        Some(AGORA)
    }
}

fn login(nome: &str, url: &str, usuario: &str, senha: &str, semente: Option<&str>) -> String {
    let totp = semente
        .map(|s| format!(r#","totp_semente":"{}""#, b64(s.as_bytes())))
        .unwrap_or_default();
    format!(
        r#"{{"entrada":{{"nome":"{nome}","grupo":"acme","ambiente":"dev","tipo":"login","campos":{{"url":"{url}","usuario":"{usuario}","senha":"{}"{totp}}}}}}}"#,
        b64(senha.as_bytes())
    )
}

/// Cofre de teste com logins em `localhost:8081` (um deles bloqueado), um com TOTP em
/// `localhost:8083`, um em `https`, um servidor e um banco.
fn cofre(dir: &Dir) -> Daemon {
    let d = Daemon::novo(dir.caminhos(), Box::new(RelogioFixo));
    let criar = pedir(
        &d,
        Papel::App,
        "criar_cofre",
        &format!(
            r#"{{"senha":"{}","pin":"{}"}}"#,
            b64(comum::SENHA.as_bytes()),
            b64(PIN.as_bytes())
        ),
    );
    assert!(ok(&criar), "{criar:?}");
    for q in [
        login(
            "acme-wp-dev",
            "http://localhost:8081/wp-login.php",
            "wp_admin",
            SEGREDOS[0],
            None,
        ),
        login(
            "acme-wp-prod",
            "http://localhost:8081/wp-login.php",
            "wp_editor",
            SEGREDOS[3],
            None,
        ),
        login(
            "acme-totp-dev",
            "http://localhost:8083/login",
            "wp_admin",
            SEGREDOS[0],
            Some(SEMENTE),
        ),
        login(
            "clientex-portal",
            "https://app.clientex.com.br/login",
            "admin",
            SEGREDOS[1],
            None,
        ),
        comum::servidor("s1"),
        comum::banco("b1", "s1"),
    ] {
        let r = pedir(&d, Papel::App, "salvar_entrada", &q);
        assert!(ok(&r), "{r:?}");
    }
    assert!(ok(&comum::mudar_nivel(
        &d,
        Papel::App,
        "acme-wp-prod",
        "bloqueado",
        PIN
    )));
    d
}

fn sugerir(d: &Daemon, papel: Papel, origem: &str) -> Resposta {
    pedir(d, papel, "sugerir", &format!(r#"{{"origem":"{origem}"}}"#))
}

fn preencher(d: &Daemon, papel: Papel, entrada: &str, origem: &str) -> Resposta {
    pedir(
        d,
        papel,
        "preencher",
        &format!(r#"{{"entrada":"{entrada}","origem":"{origem}"}}"#),
    )
}

fn nomes(r: &Resposta) -> Vec<(String, String)> {
    match r {
        Resposta::Sugestoes(l) => l
            .iter()
            .map(|s| (s.nome.clone(), s.usuario.clone()))
            .collect(),
        outra => panic!("esperava sugestões: {:?}", outra.tipo()),
    }
}

/// Nenhum segredo do cofre de teste nos bytes (em claro ou em base64).
fn sem_segredo(bytes: &[u8]) {
    for s in SEGREDOS.iter().chain([&SEMENTE]) {
        assert!(!comum::contem(bytes, s.as_bytes()), "segredo em claro");
        assert!(
            !comum::contem(bytes, b64(s.as_bytes()).as_bytes()),
            "segredo em base64"
        );
    }
}

#[test]
fn v6_2_sugerir_so_a_origem_da_aba() {
    let dir = Dir::novo("ext-sugerir");
    let d = cofre(&dir);
    let r = sugerir(&d, Papel::Extensao, "http://localhost:8081/wp-login.php");
    assert_eq!(
        nomes(&r),
        [
            ("acme-wp-dev".to_owned(), "wp_admin".to_owned()),
            // Bloqueada também: o nível regula o agente, não o usuário na extensão.
            ("acme-wp-prod".to_owned(), "wp_editor".to_owned()),
        ]
    );
    // A resposta de sugerir não tem segredo nenhum.
    sem_segredo(&comum::codificado(Papel::Extensao, &r));
    assert_eq!(
        nomes(&sugerir(&d, Papel::Extensao, "http://localhost:8083/login")),
        [("acme-totp-dev".to_owned(), "wp_admin".to_owned())]
    );
    assert_eq!(
        nomes(&sugerir(&d, Papel::Extensao, "https://app.clientex.com.br")).len(),
        1
    );
    // V6.2 e origens parecidas: nada sugerido.
    for aba in [
        "http://127.0.0.1:8081/wp-login.php",
        "http://localhost:8082/wp-login.php",
        "http://localhost:8084/",
        "https://localhost:8081/",
        "http://localhost:8081@127.0.0.1/",
        "https://evil.app.clientex.com.br/login",
        "https://app.clientex.com.br.evil.invalid/",
        "http://app.clientex.com.br/login",
        "chrome://settings",
        "file:///etc/passwd",
        "",
    ] {
        assert!(
            nomes(&sugerir(&d, Papel::Extensao, aba)).is_empty(),
            "{aba:?}"
        );
    }
}

#[test]
fn v6_2_preencher_so_na_origem_certa() {
    let dir = Dir::novo("ext-preencher");
    let d = cofre(&dir);
    let r = preencher(
        &d,
        Papel::Extensao,
        "acme-wp-dev",
        "http://localhost:8081/wp-login.php",
    );
    match &r {
        Resposta::Preenchimento {
            usuario,
            senha,
            totp,
        } => {
            assert_eq!(usuario, "wp_admin");
            assert_eq!(senha.expor(), SEGREDOS[0].as_bytes());
            assert!(totp.is_none());
        }
        outra => panic!("esperava preenchimento: {:?}", outra.tipo()),
    }
    // TOTP: o código atual da semente, pelo relógio do daemon.
    let esperado = Totp::de_semente(&ward_core::entrada::Segredo::de_bytes(
        SEMENTE.as_bytes().to_vec(),
    ))
    .unwrap()
    .codigo_em(AGORA);
    match preencher(
        &d,
        Papel::Extensao,
        "acme-totp-dev",
        "http://localhost:8083/",
    ) {
        Resposta::Preenchimento { totp: Some(c), .. } => assert_eq!(&*c, esperado.codigo()),
        outra => panic!("esperava preenchimento com TOTP: {:?}", outra.tipo()),
    }

    // V6.2 (D9): a mesma entrada para outra origem é recusada, sem valor nenhum.
    for aba in [
        "http://127.0.0.1:8081/wp-login.php",
        "http://localhost:8082/wp-login.php",
        "http://localhost:8083/login",
        "https://localhost:8081/wp-login.php",
        "http://localhost:8081@evil.invalid/",
        "http://evil.invalid/",
        "chrome-extension://abcdefghijklmnopabcdefghijklmnop/",
        "",
    ] {
        let r = preencher(&d, Papel::Extensao, "acme-wp-dev", aba);
        assert_eq!(erro(&r), Some(ErroResposta::OrigemDiferente), "{aba:?}");
        sem_segredo(&comum::codificado(Papel::Extensao, &r));
    }
    // Entrada que não existe e entrada de outro tipo respondem igual.
    for nome in ["nao-existe", "s1", "b1"] {
        let r = preencher(&d, Papel::Extensao, nome, "http://localhost:8081/");
        assert_eq!(erro(&r), Some(ErroResposta::EntradaInexistente), "{nome}");
    }
    // Travado: nada.
    d.travar();
    let r = preencher(
        &d,
        Papel::Extensao,
        "acme-wp-dev",
        "http://localhost:8081/wp-login.php",
    );
    assert_eq!(erro(&r), Some(ErroResposta::Travado));
    let r = sugerir(&d, Papel::Extensao, "http://localhost:8081/wp-login.php");
    assert_eq!(erro(&r), Some(ErroResposta::Travado));
}

#[test]
fn v6_1_agente_e_app_nao_sugerem_nem_preenchem() {
    let dir = Dir::novo("ext-papeis");
    let d = cofre(&dir);
    for papel in [Papel::Agente, Papel::App] {
        // Primeira camada: a leitura do pedido.
        assert_eq!(
            erro(&preencher(
                &d,
                papel,
                "acme-wp-dev",
                "http://localhost:8081/wp-login.php"
            )),
            Some(ErroResposta::NaoAutorizado)
        );
        assert_eq!(
            erro(&sugerir(&d, papel, "http://localhost:8081/")),
            Some(ErroResposta::NaoAutorizado)
        );
        // Segunda e terceira camadas: o pedido já lido como extensão, atendido com
        // outro papel (o papel vem da conexão, nunca do quadro).
        let p = ped(
            Papel::Extensao,
            "preencher",
            r#"{"entrada":"acme-wp-dev","origem":"http://localhost:8081/"}"#,
        )
        .unwrap();
        let r = d.atender(papel, p);
        assert_eq!(erro(&r), Some(ErroResposta::NaoAutorizado));
        sem_segredo(&comum::codificado(papel, &r));
    }
}

#[test]
fn ataque_extensao_sem_operacoes_de_app_nem_de_agente() {
    let dir = Dir::novo("ext-tabela");
    let d = cofre(&dir);
    let permitidas = [
        Operacao::Estado,
        Operacao::Sugerir,
        Operacao::Preencher,
        Operacao::SalvarLogin,
        Operacao::AbrirApp,
    ];
    for op in Operacao::TODAS {
        assert_eq!(
            op.permitida(Papel::Extensao),
            permitidas.contains(op),
            "{}",
            op.nome()
        );
        if !permitidas.contains(op) {
            // Qualquer que seja o resto do quadro.
            for dados in ["{}", r#"{"entrada":"acme-wp-dev","campo":"senha"}"#] {
                assert_eq!(
                    ped(Papel::Extensao, op.nome(), dados).unwrap_err(),
                    ErroResposta::NaoAutorizado,
                    "{}",
                    op.nome()
                );
            }
        }
    }
    // Pedidos de app e de agente bem formados, atendidos com o papel extensão.
    for (papel, op, dados) in [
        (
            Papel::App,
            "revelar",
            r#"{"entrada":"acme-wp-dev","campo":"senha"}"#,
        ),
        (
            Papel::App,
            "copiar",
            r#"{"entrada":"acme-wp-dev","campo":"senha"}"#,
        ),
        (Papel::App, "totp", r#"{"entrada":"acme-totp-dev"}"#),
        (Papel::App, "detalhe", r#"{"entrada":"acme-wp-dev"}"#),
        (Papel::App, "lista", "{}"),
        (Papel::Agente, "lista", "{}"),
        (Papel::App, "emitir_token", r#"{"entrada":"s1"}"#),
        (Papel::Agente, "login", r#"{"entrada":"acme-wp-dev"}"#),
        (Papel::Agente, "playwright", r#"{"entrada":"acme-wp-dev"}"#),
    ] {
        let p = ped(papel, op, dados).unwrap_or_else(|e| panic!("{op}: {e:?}"));
        let r = d.atender(Papel::Extensao, p);
        assert_eq!(erro(&r), Some(ErroResposta::NaoAutorizado), "{op}");
    }
    let r = d.atender(
        Papel::Extensao,
        ped(
            Papel::App,
            "mudar_nivel",
            &format!(
                r#"{{"entrada":"acme-wp-dev","nivel":"livre","pin":"{}"}}"#,
                b64(PIN.as_bytes())
            ),
        )
        .unwrap(),
    );
    assert_eq!(erro(&r), Some(ErroResposta::NaoAutorizado));
    // O que a extensão recebe: nenhuma resposta de segredo, token ou lista do app.
    for t in [
        protocolo::TipoResposta::Segredo,
        protocolo::TipoResposta::Token,
        protocolo::TipoResposta::Lista,
        protocolo::TipoResposta::Detalhe,
        protocolo::TipoResposta::Saida,
    ] {
        assert!(!t.permitida(Papel::Extensao), "{}", t.nome());
    }
}
