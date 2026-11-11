//! Phase 40 (D5, D9; V6.1, V6.2), in the daemon core: suggest and fill of the
//! extension role by the tab's origin. Attacks: look-alike origin (other host, port,
//! scheme, subdomain, userinfo), entry of another type, agent and app asking for the
//! extension's operations, and the extension asking for those of the app and the agent.
//! Only fake values, in a test vault of its own.

mod comum;

use std::time::Duration;

use comum::{Dir, PIN, SEGREDOS, b64, erro, ok, ped, pedir};
use ward_core::protocolo::{self, ErroResposta, Operacao, Papel, Resposta};
use ward_core::totp::Totp;
use wardd::nucleo::{Daemon, Relogio};

/// Fake TOTP seed (the one of `acme-totp-dev`, D28).
const SEED: &str = "JBSWY3DPEHPK3PXP";
const NOW: u64 = 1_700_000_000;

struct FixedClock;

impl Relogio for FixedClock {
    fn dormir(&self, _d: Duration) {}

    fn agora(&self) -> Option<Duration> {
        Some(Duration::from_secs(1000))
    }

    fn segundos_unix(&self) -> Option<u64> {
        Some(NOW)
    }
}

fn login(name: &str, url: &str, user: &str, password: &str, seed: Option<&str>) -> String {
    let totp = seed
        .map(|s| format!(r#","totp_semente":"{}""#, b64(s.as_bytes())))
        .unwrap_or_default();
    format!(
        r#"{{"entrada":{{"nome":"{name}","grupo":"acme","ambiente":"dev","tipo":"login","campos":{{"url":"{url}","usuario":"{user}","senha":"{}"{totp}}}}}}}"#,
        b64(password.as_bytes())
    )
}

/// Test vault with logins on `localhost:8081` (one of them blocked), one with TOTP on
/// `localhost:8083`, one on `https`, a server and a database.
fn vault(dir: &Dir) -> Daemon {
    let d = Daemon::novo(dir.caminhos(), Box::new(FixedClock));
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
            Some(SEED),
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

fn suggest(d: &Daemon, role: Papel, origin: &str) -> Resposta {
    pedir(d, role, "sugerir", &format!(r#"{{"origem":"{origin}"}}"#))
}

/// The popup's fill with no window at all (the test daemon's engine has no app).
fn fill_raw(d: &Daemon, role: Papel, entry: &str, origin: &str) -> Resposta {
    pedir(
        d,
        role,
        "preencher",
        &format!(r#"{{"entrada":"{entry}","origem":"{origin}"}}"#),
    )
}

/// The popup's fill with the user confirming the origin in the window (Phase
/// 44c: `http` entries only fill after that).
fn fill(d: &Daemon, role: Papel, entry: &str, origin: &str) -> Resposta {
    comum::com_janela(d, "confirmar", None, || fill_raw(d, role, entry, origin)).0
}

fn names(r: &Resposta) -> Vec<(String, String)> {
    match r {
        Resposta::Sugestoes(l) => l
            .iter()
            .map(|s| (s.nome.clone(), s.usuario.clone()))
            .collect(),
        other => panic!("esperava sugestões: {:?}", other.tipo()),
    }
}

/// No secret of the test vault in the bytes (in clear or in base64).
fn no_secret(bytes: &[u8]) {
    for s in SEGREDOS.iter().chain([&SEED]) {
        assert!(!comum::contem(bytes, s.as_bytes()), "segredo em claro");
        assert!(
            !comum::contem(bytes, b64(s.as_bytes()).as_bytes()),
            "segredo em base64"
        );
    }
}

/// Phase 41: the suggestion carries the environment and whether the entry has TOTP
/// (only the fact), so the popup shows the badge and "with TOTP"; no value at all.
#[test]
fn suggestion_carries_environment_and_whether_it_has_totp() {
    use ward_core::entry::Environment;
    let dir = Dir::novo("ext-sugerir-totp");
    let d = vault(&dir);
    let summary = |r: &Resposta| match r {
        Resposta::Sugestoes(l) => l
            .iter()
            .map(|s| (s.nome.clone(), s.ambiente, s.totp))
            .collect::<Vec<_>>(),
        other => panic!("esperava sugestões: {:?}", other.tipo()),
    };
    assert_eq!(
        summary(&suggest(&d, Papel::Extensao, "http://localhost:8083/login")),
        [("acme-totp-dev".to_owned(), Environment::Dev, true)]
    );
    let r = suggest(&d, Papel::Extensao, "http://localhost:8081/");
    assert_eq!(
        summary(&r),
        [
            ("acme-wp-dev".to_owned(), Environment::Dev, false),
            ("acme-wp-prod".to_owned(), Environment::Dev, false),
        ]
    );
    let q = comum::codificado(Papel::Extensao, &r);
    assert!(comum::contem(&q, br#""ambiente":"dev","totp":false"#));
    no_secret(&q);
}

#[test]
fn v6_2_suggest_only_the_tab_origin() {
    let dir = Dir::novo("ext-sugerir");
    let d = vault(&dir);
    let r = suggest(&d, Papel::Extensao, "http://localhost:8081/wp-login.php");
    assert_eq!(
        names(&r),
        [
            ("acme-wp-dev".to_owned(), "wp_admin".to_owned()),
            // Blocked too: the level governs the agent, not the user in the extension.
            ("acme-wp-prod".to_owned(), "wp_editor".to_owned()),
        ]
    );
    // The suggestion answer has no secret at all.
    no_secret(&comum::codificado(Papel::Extensao, &r));
    assert_eq!(
        names(&suggest(&d, Papel::Extensao, "http://localhost:8083/login")),
        [("acme-totp-dev".to_owned(), "wp_admin".to_owned())]
    );
    assert_eq!(
        names(&suggest(&d, Papel::Extensao, "https://app.clientex.com.br")).len(),
        1
    );
    // V6.2 and look-alike origins: nothing suggested.
    for tab in [
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
            names(&suggest(&d, Papel::Extensao, tab)).is_empty(),
            "{tab:?}"
        );
    }
}

#[test]
fn v6_2_fill_only_on_the_right_origin() {
    let dir = Dir::novo("ext-preencher");
    let d = vault(&dir);
    let r = fill(
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
            assert_eq!(senha.expose(), SEGREDOS[0].as_bytes());
            assert!(totp.is_none());
        }
        other => panic!("esperava preenchimento: {:?}", other.tipo()),
    }
    // TOTP: the current code of the seed, by the daemon's clock.
    let expected = Totp::from_seed(&ward_core::entry::Secret::from_bytes(
        SEED.as_bytes().to_vec(),
    ))
    .unwrap()
    .code_at(NOW);
    match fill(
        &d,
        Papel::Extensao,
        "acme-totp-dev",
        "http://localhost:8083/",
    ) {
        Resposta::Preenchimento { totp: Some(c), .. } => assert_eq!(&*c, expected.code()),
        other => panic!("esperava preenchimento com TOTP: {:?}", other.tipo()),
    }

    // V6.2 (D9): the same entry for another origin is refused, with no value at all.
    for tab in [
        "http://127.0.0.1:8081/wp-login.php",
        "http://localhost:8082/wp-login.php",
        "http://localhost:8083/login",
        "https://localhost:8081/wp-login.php",
        "http://localhost:8081@evil.invalid/",
        "http://evil.invalid/",
        "chrome-extension://abcdefghijklmnopabcdefghijklmnop/",
        "",
    ] {
        let r = fill(&d, Papel::Extensao, "acme-wp-dev", tab);
        assert_eq!(erro(&r), Some(ErroResposta::OrigemDiferente), "{tab:?}");
        no_secret(&comum::codificado(Papel::Extensao, &r));
    }
    // An entry that does not exist and an entry of another type answer the same.
    for name in ["nao-existe", "s1", "b1"] {
        let r = fill(&d, Papel::Extensao, name, "http://localhost:8081/");
        assert_eq!(erro(&r), Some(ErroResposta::EntradaInexistente), "{name}");
    }
    // Locked: nothing.
    d.travar();
    let r = fill(
        &d,
        Papel::Extensao,
        "acme-wp-dev",
        "http://localhost:8081/wp-login.php",
    );
    assert_eq!(erro(&r), Some(ErroResposta::Travado));
    let r = suggest(&d, Papel::Extensao, "http://localhost:8081/wp-login.php");
    assert_eq!(erro(&r), Some(ErroResposta::Travado));
}

#[test]
fn v6_1_agent_and_app_neither_suggest_nor_fill() {
    let dir = Dir::novo("ext-papeis");
    let d = vault(&dir);
    for role in [Papel::Agente, Papel::App] {
        // First layer: reading the request.
        assert_eq!(
            erro(&fill(
                &d,
                role,
                "acme-wp-dev",
                "http://localhost:8081/wp-login.php"
            )),
            Some(ErroResposta::NaoAutorizado)
        );
        assert_eq!(
            erro(&suggest(&d, role, "http://localhost:8081/")),
            Some(ErroResposta::NaoAutorizado)
        );
        // Second and third layers: the request already read as the extension, served
        // with another role (the role comes from the connection, never from the frame).
        let p = ped(
            Papel::Extensao,
            "preencher",
            r#"{"entrada":"acme-wp-dev","origem":"http://localhost:8081/"}"#,
        )
        .unwrap();
        let r = d.atender(role, p);
        assert_eq!(erro(&r), Some(ErroResposta::NaoAutorizado));
        no_secret(&comum::codificado(role, &r));
    }
}

#[test]
fn attack_extension_without_app_or_agent_operations() {
    let dir = Dir::novo("ext-tabela");
    let d = vault(&dir);
    let allowed = [
        Operacao::Estado,
        Operacao::Sugerir,
        Operacao::Preencher,
        Operacao::SalvarLogin,
        // Phase 43 (`ward login`): extension only.
        Operacao::AguardarLogin,
        Operacao::PreencherLogin,
        Operacao::ResultadoLogin,
        Operacao::AbrirApp,
    ];
    for op in Operacao::TODAS {
        assert_eq!(
            op.permitida(Papel::Extensao),
            allowed.contains(op),
            "{}",
            op.nome()
        );
        if !allowed.contains(op) {
            // Whatever the rest of the frame is.
            for data in ["{}", r#"{"entrada":"acme-wp-dev","campo":"senha"}"#] {
                assert_eq!(
                    ped(Papel::Extensao, op.nome(), data).unwrap_err(),
                    ErroResposta::NaoAutorizado,
                    "{}",
                    op.nome()
                );
            }
        }
    }
    // Well-formed app and agent requests, served with the extension role.
    for (role, op, data) in [
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
        let p = ped(role, op, data).unwrap_or_else(|e| panic!("{op}: {e:?}"));
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
    // What the extension receives: no secret, token or app list answer.
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

// --- Phase 42: save login (V6.8) ------------------------------------------------------------

/// Save-login request as the extension's service worker builds it: only name, group,
/// environment, type and the URL, user and password fields; `extra` goes raw into the
/// entry object (for the attacks).
fn save_login(d: &Daemon, role: Papel, name: &str, password: &str, extra: &str) -> Resposta {
    pedir(
        d,
        role,
        "salvar_login",
        &format!(
            r#"{{"entrada":{{"nome":"{name}","grupo":"localhost","ambiente":"dev","tipo":"login","campos":{{"url":"http://localhost:8083/cadastro","usuario":"novo.usuario","senha":"{}"}}{extra}}}}}"#,
            b64(password.as_bytes())
        ),
    )
}

/// Level of an entry in the app's list (`None` if it does not exist).
fn level_in_list(d: &Daemon, name: &str) -> Option<ward_core::entry::AccessLevel> {
    match pedir(d, Papel::App, "lista", "{}") {
        Resposta::Lista(l) => l.into_iter().find(|i| i.nome == name).and_then(|i| i.nivel),
        other => panic!("esperava a lista: {:?}", other.tipo()),
    }
}

/// New fake password, not among the common test secrets.
const NEW_PASSWORD: &str = "falso-cadastro-ward-42";

/// V6.8: the extension's new login shows up in the list with the "approval" level, is
/// suggested for the origin and fills with the password sent; the answer carries no
/// value.
#[test]
fn v6_8_save_login_starts_with_approval() {
    use ward_core::entry::AccessLevel;
    let dir = Dir::novo("ext-salvar");
    let d = vault(&dir);
    let r = save_login(&d, Papel::Extensao, "localhost-8083", NEW_PASSWORD, "");
    assert!(ok(&r), "{r:?}");
    assert!(!comum::contem(
        &comum::codificado(Papel::Extensao, &r),
        NEW_PASSWORD.as_bytes()
    ));
    assert_eq!(
        level_in_list(&d, "localhost-8083"),
        Some(AccessLevel::Approval)
    );
    assert_eq!(
        names(&suggest(&d, Papel::Extensao, "http://localhost:8083/")),
        [
            ("acme-totp-dev".to_owned(), "wp_admin".to_owned()),
            ("localhost-8083".to_owned(), "novo.usuario".to_owned()),
        ]
    );
    match fill(
        &d,
        Papel::Extensao,
        "localhost-8083",
        "http://localhost:8083",
    ) {
        Resposta::Preenchimento { senha, totp, .. } => {
            assert_eq!(senha.expose(), NEW_PASSWORD.as_bytes());
            assert!(totp.is_none());
        }
        other => panic!("esperava preenchimento: {:?}", other.tipo()),
    }
    // Saved on disk: after locking and unlocking, it is still there and with approval.
    assert!(ok(&pedir(&d, Papel::App, "travar", "{}")));
    assert!(ok(&comum::destravar(&d, comum::SENHA)));
    assert_eq!(
        level_in_list(&d, "localhost-8083"),
        Some(AccessLevel::Approval)
    );
}

/// Attack: the extension tries to overwrite an entry that already exists (change the
/// password of `acme-wp-prod`, which is blocked, or of `acme-wp-dev`): the repeated name
/// is a validation error, and the entry keeps the password and level it had.
#[test]
fn attack_save_login_does_not_overwrite_entry() {
    use ward_core::entry::AccessLevel;
    let dir = Dir::novo("ext-salvar-sobrescrever");
    let d = vault(&dir);
    for name in ["acme-wp-dev", "acme-wp-prod"] {
        let r = save_login(&d, Papel::Extensao, name, NEW_PASSWORD, "");
        assert!(
            matches!(erro(&r), Some(ErroResposta::Validacao(_))),
            "{name}: {r:?}"
        );
    }
    assert_eq!(
        level_in_list(&d, "acme-wp-prod"),
        Some(AccessLevel::Blocked)
    );
    assert_eq!(
        level_in_list(&d, "acme-wp-dev"),
        Some(AccessLevel::Approval)
    );
    match fill(&d, Papel::Extensao, "acme-wp-dev", "http://localhost:8081") {
        Resposta::Preenchimento { senha, .. } => assert_eq!(senha.expose(), SEGREDOS[0].as_bytes()),
        other => panic!("esperava preenchimento: {:?}", other.tipo()),
    }
}

/// Attack: save login only accepts what the popup asks for. Level in the request
/// (the frame has no such field), TOTP, extras, attachments, project, tags and another
/// type are refused and nothing is saved.
#[test]
fn attack_save_login_only_the_popup_fields() {
    let dir = Dir::novo("ext-salvar-campos");
    let d = vault(&dir);
    let seed = b64(SEED.as_bytes());
    let value = b64(b"x");
    let refused = [
        (r#","nivel":"livre""#.to_owned(), false),
        (r#","projeto":"p1""#.to_owned(), true),
        (r#","tags":["t1"]"#.to_owned(), true),
        (
            format!(r#","extras":[{{"nome":"API_KEY","valor":"{value}","segredo":true}}]"#),
            true,
        ),
        (
            format!(r#","anexos":[{{"nome":"a.txt","conteudo":"{value}"}}]"#),
            true,
        ),
    ];
    for (extra, reaches_daemon) in &refused {
        let r = save_login(&d, Papel::Extensao, "localhost-8083", NEW_PASSWORD, extra);
        let e = erro(&r).unwrap_or_else(|| panic!("{extra}: aceito"));
        if *reaches_daemon {
            assert_eq!(e, ErroResposta::PedidoInvalido, "{extra}");
        }
        assert_eq!(level_in_list(&d, "localhost-8083"), None, "{extra}");
    }
    // TOTP inside the login fields.
    let r = pedir(
        &d,
        Papel::Extensao,
        "salvar_login",
        &format!(
            r#"{{"entrada":{{"nome":"localhost-8083","grupo":"g","ambiente":"dev","tipo":"login","campos":{{"url":"http://localhost:8083/","usuario":"u","senha":"{}","totp_semente":"{seed}"}}}}}}"#,
            b64(NEW_PASSWORD.as_bytes())
        ),
    );
    assert_eq!(erro(&r), Some(ErroResposta::PedidoInvalido));
    // Another type (database) through save login: reading the request refuses it.
    assert!(ped(Papel::Extensao, "salvar_login", &comum::banco("b9", "s1")).is_err());
    assert_eq!(level_in_list(&d, "localhost-8083"), None);
    assert_eq!(level_in_list(&d, "b9"), None);
}

/// Attack: agent and app do not ask for save login, not even with the request
/// already read as the extension; with the vault locked, nothing is saved.
#[test]
fn attack_save_login_extension_only_and_unlocked() {
    let dir = Dir::novo("ext-salvar-papeis");
    let d = vault(&dir);
    let p = |name: &str| {
        format!(
            r#"{{"entrada":{{"nome":"{name}","grupo":"g","ambiente":"dev","tipo":"login","campos":{{"url":"http://localhost:8083/","usuario":"u","senha":"{}"}}}}}}"#,
            b64(NEW_PASSWORD.as_bytes())
        )
    };
    for role in [Papel::Agente, Papel::App] {
        assert_eq!(
            erro(&pedir(&d, role, "salvar_login", &p("x1"))),
            Some(ErroResposta::NaoAutorizado)
        );
        let read = ped(Papel::Extensao, "salvar_login", &p("x2")).unwrap();
        assert_eq!(
            erro(&d.atender(role, read)),
            Some(ErroResposta::NaoAutorizado)
        );
    }
    assert_eq!(level_in_list(&d, "x1"), None);
    assert_eq!(level_in_list(&d, "x2"), None);
    assert!(ok(&pedir(&d, Papel::App, "travar", "{}")));
    assert_eq!(
        erro(&pedir(&d, Papel::Extensao, "salvar_login", &p("x3"))),
        Some(ErroResposta::Travado)
    );
    assert!(ok(&comum::destravar(&d, comum::SENHA)));
    assert_eq!(level_in_list(&d, "x3"), None);
}

// --- Phase 44c (D9): `http` origin only with the confirmation in the window ---------------

/// Attack (M6 audit, `[::1]`): another process of the uid listens on `[::1]:8081` with
/// WordPress only on `127.0.0.1`, and Chrome sends the `http://localhost:8081` tab to
/// it. The origin matches, so the daemon has no way to tell; that is why the popup only
/// fills an `http` entry after the user confirms the origin in the window. Without an
/// app: "approval unavailable"; denied: nothing; the request shows the origin, comes
/// from the popup (fill) and does not ask for a PIN. `https` goes on without a
/// window.
#[test]
fn attack_fill_http_without_confirming_nothing_leaves() {
    let dir = Dir::novo("ext-http-confirma");
    let d = vault(&dir);
    let tab = "http://localhost:8081/wp-login.php";
    // No app at all.
    let r = fill_raw(&d, Papel::Extensao, "acme-wp-dev", tab);
    assert_eq!(erro(&r), Some(ErroResposta::AprovacaoIndisponivel));
    no_secret(&comum::codificado(Papel::Extensao, &r));
    // Denied in the window.
    let (r, seen) = comum::com_janela(&d, "negar", None, || {
        fill_raw(&d, Papel::Extensao, "acme-wp-dev", tab)
    });
    assert_eq!(erro(&r), Some(ErroResposta::Negado));
    no_secret(&comum::codificado(Papel::Extensao, &r));
    assert_eq!(seen.len(), 1);
    let p = &seen[0];
    assert_eq!(p.operacao, Operacao::Preencher);
    assert_eq!(p.entradas, ["acme-wp-dev".to_owned()]);
    assert_eq!(p.origem, "http://localhost:8081");
    assert!(p.confirmacao);
    no_secret(&comum::codificado(
        Papel::App,
        &Resposta::PedidoDeAprovacao(p.clone()),
    ));
    // The "Allow" answers with a PIN do not decide a confirmation (the forged window
    // does not skip the format): the request stays pending and expires; here the
    // window loop only tries.
    let (r, _) = comum::com_janela(&d, "uma_vez", Some(PIN), || {
        // With no accepted decision, the request would stay pending until it expired
        // (2 min): the test ends it through the client that leaves.
        struct LeavesSoon(std::sync::atomic::AtomicU32);
        impl wardd::aprovacao::Cliente for LeavesSoon {
            fn cadeia(&self) -> Vec<String> {
                vec!["chrome".into(), "ward-nm".into()]
            }
            fn saiu(&self) -> bool {
                self.0.fetch_add(1, std::sync::atomic::Ordering::AcqRel) > 40
            }
        }
        let p = ped(
            Papel::Extensao,
            "preencher",
            &format!(r#"{{"entrada":"acme-wp-dev","origem":"{tab}"}}"#),
        )
        .unwrap();
        d.atender_de(&LeavesSoon(Default::default()), Papel::Extensao, p)
    });
    assert_eq!(
        erro(&r),
        Some(ErroResposta::Negado),
        "uma_vez decidiu uma confirmação"
    );
    no_secret(&comum::codificado(Papel::Extensao, &r));
    // Confirmed: it fills.
    let (r, seen) = comum::com_janela(&d, "confirmar", None, || {
        fill_raw(&d, Papel::Extensao, "acme-wp-dev", tab)
    });
    assert!(
        matches!(r, Resposta::Preenchimento { .. }),
        "{:?}",
        r.tipo()
    );
    assert_eq!(seen.len(), 1);
    // `https`: no window.
    let (r, seen) = comum::com_janela(&d, "negar", None, || {
        fill_raw(
            &d,
            Papel::Extensao,
            "clientex-portal",
            "https://app.clientex.com.br/login",
        )
    });
    assert!(
        matches!(r, Resposta::Preenchimento { .. }),
        "{:?}",
        r.tipo()
    );
    assert!(seen.is_empty(), "janela num preencher https");
    // Different origin: refused before asking.
    let (r, seen) = comum::com_janela(&d, "confirmar", None, || {
        fill_raw(&d, Papel::Extensao, "acme-wp-dev", "http://127.0.0.1:8081/")
    });
    assert_eq!(erro(&r), Some(ErroResposta::OrigemDiferente));
    assert!(seen.is_empty());
}

/// Phase 44e (Medium of the 2nd M6 audit): the popup on a loopback `https` entry
/// (`https://localhost`, `https://[::1]`) also only fills after the confirmation. With a
/// local CA planted (mkcert, `~/.pki`), the process on `[::1]` passes TLS. Without an
/// app: "approval unavailable"; denied: no value; the request shows the origin.
#[test]
fn attack_fill_https_loopback_without_confirming_nothing_leaves() {
    let dir = Dir::novo("ext-https-loopback");
    let d = vault(&dir);
    for (i, (url, origin)) in [
        ("https://localhost:8443/login", "https://localhost:8443"),
        ("https://[::1]:8443/login", "https://[::1]:8443"),
        ("https://127.0.0.1/login", "https://127.0.0.1"),
    ]
    .into_iter()
    .enumerate()
    {
        let name = format!("l-{i}");
        let r = pedir(
            &d,
            Papel::App,
            "salvar_entrada",
            &login(&name, url, "wp_admin", SEGREDOS[0], None),
        );
        assert!(ok(&r), "{r:?}");
        let r = fill_raw(&d, Papel::Extensao, &name, url);
        assert_eq!(erro(&r), Some(ErroResposta::AprovacaoIndisponivel), "{url}");
        no_secret(&comum::codificado(Papel::Extensao, &r));
        let (r, seen) = comum::com_janela(&d, "negar", None, || {
            fill_raw(&d, Papel::Extensao, &name, url)
        });
        assert_eq!(erro(&r), Some(ErroResposta::Negado), "{url}");
        no_secret(&comum::codificado(Papel::Extensao, &r));
        assert_eq!(seen.len(), 1, "{url}");
        assert_eq!(seen[0].operacao, Operacao::Preencher);
        assert_eq!(seen[0].origem, origin, "{url}");
        assert!(seen[0].confirmacao, "{url}");
        let (r, _) = comum::com_janela(&d, "confirmar", None, || {
            fill_raw(&d, Papel::Extensao, &name, url)
        });
        assert!(matches!(r, Resposta::Preenchimento { .. }), "{url}");
    }
}
