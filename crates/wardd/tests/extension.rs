//! Phase 40 (D5, D9; V6.1, V6.2), in the daemon core: suggest and fill of the
//! extension role by the tab's origin. Attacks: look-alike origin (other host, port,
//! scheme, subdomain, userinfo), entry of another type, agent and app asking for the
//! extension's operations, and the extension asking for those of the app and the agent.
//! Only fake values, in a test vault of its own.

mod common;

use std::time::Duration;

use common::{Dir, PIN, SECRETS, b64, error, ok, req, request};
use ward_core::protocol::{self, Operation, ResponseError, Resposta, Role};
use ward_core::totp::Totp;
use wardd::core::{Clock, Daemon};

/// Fake TOTP seed (the one of `acme-totp-dev`, D28).
const SEED: &str = "JBSWY3DPEHPK3PXP";
const NOW: u64 = 1_700_000_000;

struct FixedClock;

impl Clock for FixedClock {
    fn sleep(&self, _d: Duration) {}

    fn now(&self) -> Option<Duration> {
        Some(Duration::from_secs(1000))
    }

    fn unix_seconds(&self) -> Option<u64> {
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
    let d = Daemon::new(dir.paths(), Box::new(FixedClock));
    let create = request(
        &d,
        Role::App,
        "criar_cofre",
        &format!(
            r#"{{"senha":"{}","pin":"{}"}}"#,
            b64(common::PASSWORD.as_bytes()),
            b64(PIN.as_bytes())
        ),
    );
    assert!(ok(&create), "{create:?}");
    for q in [
        login(
            "acme-wp-dev",
            "http://localhost:8081/wp-login.php",
            "wp_admin",
            SECRETS[0],
            None,
        ),
        login(
            "acme-wp-prod",
            "http://localhost:8081/wp-login.php",
            "wp_editor",
            SECRETS[3],
            None,
        ),
        login(
            "acme-totp-dev",
            "http://localhost:8083/login",
            "wp_admin",
            SECRETS[0],
            Some(SEED),
        ),
        login(
            "clientex-portal",
            "https://app.clientex.com.br/login",
            "admin",
            SECRETS[1],
            None,
        ),
        common::server("s1"),
        common::database("b1", "s1"),
    ] {
        let r = request(&d, Role::App, "salvar_entrada", &q);
        assert!(ok(&r), "{r:?}");
    }
    assert!(ok(&common::set_level(
        &d,
        Role::App,
        "acme-wp-prod",
        "bloqueado",
        PIN
    )));
    d
}

fn suggest(d: &Daemon, role: Role, origin: &str) -> Resposta {
    request(d, role, "sugerir", &format!(r#"{{"origem":"{origin}"}}"#))
}

/// The popup's fill with no window at all (the test daemon's engine has no app).
fn fill_raw(d: &Daemon, role: Role, entry: &str, origin: &str) -> Resposta {
    request(
        d,
        role,
        "preencher",
        &format!(r#"{{"entrada":"{entry}","origem":"{origin}"}}"#),
    )
}

/// The popup's fill with the user confirming the origin in the window (Phase
/// 44c: `http` entries only fill after that).
fn fill(d: &Daemon, role: Role, entry: &str, origin: &str) -> Resposta {
    common::with_window(d, "confirmar", None, || fill_raw(d, role, entry, origin)).0
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
    for s in SECRETS.iter().chain([&SEED]) {
        assert!(!common::contains(bytes, s.as_bytes()), "segredo em claro");
        assert!(
            !common::contains(bytes, b64(s.as_bytes()).as_bytes()),
            "segredo em base64"
        );
    }
}

/// Phase 41: the suggestion carries the environment and whether the entry has TOTP
/// (only the fact), so the popup shows the badge and "with TOTP"; no value at all.
#[test]
fn suggestion_carries_environment_and_whether_it_has_totp() {
    use ward_core::entry::Environment;
    let dir = Dir::new("ext-sugerir-totp");
    let d = vault(&dir);
    let summary = |r: &Resposta| match r {
        Resposta::Sugestoes(l) => l
            .iter()
            .map(|s| (s.nome.clone(), s.ambiente, s.totp))
            .collect::<Vec<_>>(),
        other => panic!("esperava sugestões: {:?}", other.tipo()),
    };
    assert_eq!(
        summary(&suggest(&d, Role::Extension, "http://localhost:8083/login")),
        [("acme-totp-dev".to_owned(), Environment::Dev, true)]
    );
    let r = suggest(&d, Role::Extension, "http://localhost:8081/");
    assert_eq!(
        summary(&r),
        [
            ("acme-wp-dev".to_owned(), Environment::Dev, false),
            ("acme-wp-prod".to_owned(), Environment::Dev, false),
        ]
    );
    let q = common::encoded(Role::Extension, &r);
    assert!(common::contains(&q, br#""ambiente":"dev","totp":false"#));
    no_secret(&q);
}

#[test]
fn v6_2_suggest_only_the_tab_origin() {
    let dir = Dir::new("ext-sugerir");
    let d = vault(&dir);
    let r = suggest(&d, Role::Extension, "http://localhost:8081/wp-login.php");
    assert_eq!(
        names(&r),
        [
            ("acme-wp-dev".to_owned(), "wp_admin".to_owned()),
            // Blocked too: the level governs the agent, not the user in the extension.
            ("acme-wp-prod".to_owned(), "wp_editor".to_owned()),
        ]
    );
    // The suggestion answer has no secret at all.
    no_secret(&common::encoded(Role::Extension, &r));
    assert_eq!(
        names(&suggest(&d, Role::Extension, "http://localhost:8083/login")),
        [("acme-totp-dev".to_owned(), "wp_admin".to_owned())]
    );
    assert_eq!(
        names(&suggest(&d, Role::Extension, "https://app.clientex.com.br")).len(),
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
            names(&suggest(&d, Role::Extension, tab)).is_empty(),
            "{tab:?}"
        );
    }
}

#[test]
fn v6_2_fill_only_on_the_right_origin() {
    let dir = Dir::new("ext-preencher");
    let d = vault(&dir);
    let r = fill(
        &d,
        Role::Extension,
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
            assert_eq!(senha.expose(), SECRETS[0].as_bytes());
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
        Role::Extension,
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
        let r = fill(&d, Role::Extension, "acme-wp-dev", tab);
        assert_eq!(error(&r), Some(ResponseError::OriginMismatch), "{tab:?}");
        no_secret(&common::encoded(Role::Extension, &r));
    }
    // An entry that does not exist and an entry of another type answer the same.
    for name in ["nao-existe", "s1", "b1"] {
        let r = fill(&d, Role::Extension, name, "http://localhost:8081/");
        assert_eq!(error(&r), Some(ResponseError::EntryNotFound), "{name}");
    }
    // Locked: nothing.
    d.lock();
    let r = fill(
        &d,
        Role::Extension,
        "acme-wp-dev",
        "http://localhost:8081/wp-login.php",
    );
    assert_eq!(error(&r), Some(ResponseError::Locked));
    let r = suggest(&d, Role::Extension, "http://localhost:8081/wp-login.php");
    assert_eq!(error(&r), Some(ResponseError::Locked));
}

#[test]
fn v6_1_agent_and_app_neither_suggest_nor_fill() {
    let dir = Dir::new("ext-papeis");
    let d = vault(&dir);
    for role in [Role::Agent, Role::App] {
        // First layer: reading the request.
        assert_eq!(
            error(&fill(
                &d,
                role,
                "acme-wp-dev",
                "http://localhost:8081/wp-login.php"
            )),
            Some(ResponseError::Unauthorized)
        );
        assert_eq!(
            error(&suggest(&d, role, "http://localhost:8081/")),
            Some(ResponseError::Unauthorized)
        );
        // Second and third layers: the request already read as the extension, served
        // with another role (the role comes from the connection, never from the frame).
        let p = req(
            Role::Extension,
            "preencher",
            r#"{"entrada":"acme-wp-dev","origem":"http://localhost:8081/"}"#,
        )
        .unwrap();
        let r = d.serve(role, p);
        assert_eq!(error(&r), Some(ResponseError::Unauthorized));
        no_secret(&common::encoded(role, &r));
    }
}

#[test]
fn attack_extension_without_app_or_agent_operations() {
    let dir = Dir::new("ext-tabela");
    let d = vault(&dir);
    let allowed = [
        Operation::Status,
        Operation::Suggest,
        Operation::Fill,
        Operation::SaveLogin,
        // Phase 43 (`ward login`): extension only.
        Operation::AwaitLogin,
        Operation::FillLogin,
        Operation::LoginResult,
        Operation::OpenApp,
    ];
    for op in Operation::ALL {
        assert_eq!(
            op.allowed(Role::Extension),
            allowed.contains(op),
            "{}",
            op.name()
        );
        if !allowed.contains(op) {
            // Whatever the rest of the frame is.
            for data in ["{}", r#"{"entrada":"acme-wp-dev","campo":"senha"}"#] {
                assert_eq!(
                    req(Role::Extension, op.name(), data).unwrap_err(),
                    ResponseError::Unauthorized,
                    "{}",
                    op.name()
                );
            }
        }
    }
    // Well-formed app and agent requests, served with the extension role.
    for (role, op, data) in [
        (
            Role::App,
            "revelar",
            r#"{"entrada":"acme-wp-dev","campo":"senha"}"#,
        ),
        (
            Role::App,
            "copiar",
            r#"{"entrada":"acme-wp-dev","campo":"senha"}"#,
        ),
        (Role::App, "totp", r#"{"entrada":"acme-totp-dev"}"#),
        (Role::App, "detalhe", r#"{"entrada":"acme-wp-dev"}"#),
        (Role::App, "lista", "{}"),
        (Role::Agent, "lista", "{}"),
        (Role::App, "emitir_token", r#"{"entrada":"s1"}"#),
        (Role::Agent, "login", r#"{"entrada":"acme-wp-dev"}"#),
        (Role::Agent, "playwright", r#"{"entrada":"acme-wp-dev"}"#),
    ] {
        let p = req(role, op, data).unwrap_or_else(|e| panic!("{op}: {e:?}"));
        let r = d.serve(Role::Extension, p);
        assert_eq!(error(&r), Some(ResponseError::Unauthorized), "{op}");
    }
    let r = d.serve(
        Role::Extension,
        req(
            Role::App,
            "mudar_nivel",
            &format!(
                r#"{{"entrada":"acme-wp-dev","nivel":"livre","pin":"{}"}}"#,
                b64(PIN.as_bytes())
            ),
        )
        .unwrap(),
    );
    assert_eq!(error(&r), Some(ResponseError::Unauthorized));
    // What the extension receives: no secret, token or app list answer.
    for t in [
        protocol::ResponseType::Secret,
        protocol::ResponseType::Token,
        protocol::ResponseType::List,
        protocol::ResponseType::Detail,
        protocol::ResponseType::Output,
    ] {
        assert!(!t.allowed(Role::Extension), "{}", t.name());
    }
}

// --- Phase 42: save login (V6.8) ------------------------------------------------------------

/// Save-login request as the extension's service worker builds it: only name, group,
/// environment, type and the URL, user and password fields; `extra` goes raw into the
/// entry object (for the attacks).
fn save_login(d: &Daemon, role: Role, name: &str, password: &str, extra: &str) -> Resposta {
    request(
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
    match request(d, Role::App, "lista", "{}") {
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
    let dir = Dir::new("ext-salvar");
    let d = vault(&dir);
    let r = save_login(&d, Role::Extension, "localhost-8083", NEW_PASSWORD, "");
    assert!(ok(&r), "{r:?}");
    assert!(!common::contains(
        &common::encoded(Role::Extension, &r),
        NEW_PASSWORD.as_bytes()
    ));
    assert_eq!(
        level_in_list(&d, "localhost-8083"),
        Some(AccessLevel::Approval)
    );
    assert_eq!(
        names(&suggest(&d, Role::Extension, "http://localhost:8083/")),
        [
            ("acme-totp-dev".to_owned(), "wp_admin".to_owned()),
            ("localhost-8083".to_owned(), "novo.usuario".to_owned()),
        ]
    );
    match fill(
        &d,
        Role::Extension,
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
    assert!(ok(&request(&d, Role::App, "travar", "{}")));
    assert!(ok(&common::unlock(&d, common::PASSWORD)));
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
    let dir = Dir::new("ext-salvar-sobrescrever");
    let d = vault(&dir);
    for name in ["acme-wp-dev", "acme-wp-prod"] {
        let r = save_login(&d, Role::Extension, name, NEW_PASSWORD, "");
        assert!(
            matches!(error(&r), Some(ResponseError::Validation(_))),
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
    match fill(&d, Role::Extension, "acme-wp-dev", "http://localhost:8081") {
        Resposta::Preenchimento { senha, .. } => assert_eq!(senha.expose(), SECRETS[0].as_bytes()),
        other => panic!("esperava preenchimento: {:?}", other.tipo()),
    }
}

/// Attack: save login only accepts what the popup asks for. Level in the request
/// (the frame has no such field), TOTP, extras, attachments, project, tags and another
/// type are refused and nothing is saved.
#[test]
fn attack_save_login_only_the_popup_fields() {
    let dir = Dir::new("ext-salvar-campos");
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
        let r = save_login(&d, Role::Extension, "localhost-8083", NEW_PASSWORD, extra);
        let e = error(&r).unwrap_or_else(|| panic!("{extra}: aceito"));
        if *reaches_daemon {
            assert_eq!(e, ResponseError::InvalidRequest, "{extra}");
        }
        assert_eq!(level_in_list(&d, "localhost-8083"), None, "{extra}");
    }
    // TOTP inside the login fields.
    let r = request(
        &d,
        Role::Extension,
        "salvar_login",
        &format!(
            r#"{{"entrada":{{"nome":"localhost-8083","grupo":"g","ambiente":"dev","tipo":"login","campos":{{"url":"http://localhost:8083/","usuario":"u","senha":"{}","totp_semente":"{seed}"}}}}}}"#,
            b64(NEW_PASSWORD.as_bytes())
        ),
    );
    assert_eq!(error(&r), Some(ResponseError::InvalidRequest));
    // Another type (database) through save login: reading the request refuses it.
    assert!(
        req(
            Role::Extension,
            "salvar_login",
            &common::database("b9", "s1")
        )
        .is_err()
    );
    assert_eq!(level_in_list(&d, "localhost-8083"), None);
    assert_eq!(level_in_list(&d, "b9"), None);
}

/// Attack: agent and app do not ask for save login, not even with the request
/// already read as the extension; with the vault locked, nothing is saved.
#[test]
fn attack_save_login_extension_only_and_unlocked() {
    let dir = Dir::new("ext-salvar-papeis");
    let d = vault(&dir);
    let p = |name: &str| {
        format!(
            r#"{{"entrada":{{"nome":"{name}","grupo":"g","ambiente":"dev","tipo":"login","campos":{{"url":"http://localhost:8083/","usuario":"u","senha":"{}"}}}}}}"#,
            b64(NEW_PASSWORD.as_bytes())
        )
    };
    for role in [Role::Agent, Role::App] {
        assert_eq!(
            error(&request(&d, role, "salvar_login", &p("x1"))),
            Some(ResponseError::Unauthorized)
        );
        let read = req(Role::Extension, "salvar_login", &p("x2")).unwrap();
        assert_eq!(
            error(&d.serve(role, read)),
            Some(ResponseError::Unauthorized)
        );
    }
    assert_eq!(level_in_list(&d, "x1"), None);
    assert_eq!(level_in_list(&d, "x2"), None);
    assert!(ok(&request(&d, Role::App, "travar", "{}")));
    assert_eq!(
        error(&request(&d, Role::Extension, "salvar_login", &p("x3"))),
        Some(ResponseError::Locked)
    );
    assert!(ok(&common::unlock(&d, common::PASSWORD)));
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
    let dir = Dir::new("ext-http-confirma");
    let d = vault(&dir);
    let tab = "http://localhost:8081/wp-login.php";
    // No app at all.
    let r = fill_raw(&d, Role::Extension, "acme-wp-dev", tab);
    assert_eq!(error(&r), Some(ResponseError::ApprovalUnavailable));
    no_secret(&common::encoded(Role::Extension, &r));
    // Denied in the window.
    let (r, seen) = common::with_window(&d, "negar", None, || {
        fill_raw(&d, Role::Extension, "acme-wp-dev", tab)
    });
    assert_eq!(error(&r), Some(ResponseError::Denied));
    no_secret(&common::encoded(Role::Extension, &r));
    assert_eq!(seen.len(), 1);
    let p = &seen[0];
    assert_eq!(p.operacao, Operation::Fill);
    assert_eq!(p.entradas, ["acme-wp-dev".to_owned()]);
    assert_eq!(p.origem, "http://localhost:8081");
    assert!(p.confirmacao);
    no_secret(&common::encoded(
        Role::App,
        &Resposta::PedidoDeAprovacao(p.clone()),
    ));
    // The "Allow" answers with a PIN do not decide a confirmation (the forged window
    // does not skip the format): the request stays pending and expires; here the
    // window loop only tries.
    let (r, _) = common::with_window(&d, "uma_vez", Some(PIN), || {
        // With no accepted decision, the request would stay pending until it expired
        // (2 min): the test ends it through the client that leaves.
        struct LeavesSoon(std::sync::atomic::AtomicU32);
        impl wardd::approval::Client for LeavesSoon {
            fn chain(&self) -> Vec<String> {
                vec!["chrome".into(), "ward-nm".into()]
            }
            fn exited(&self) -> bool {
                self.0.fetch_add(1, std::sync::atomic::Ordering::AcqRel) > 40
            }
        }
        let p = req(
            Role::Extension,
            "preencher",
            &format!(r#"{{"entrada":"acme-wp-dev","origem":"{tab}"}}"#),
        )
        .unwrap();
        d.serve_from(&LeavesSoon(Default::default()), Role::Extension, p)
    });
    assert_eq!(
        error(&r),
        Some(ResponseError::Denied),
        "uma_vez decidiu uma confirmação"
    );
    no_secret(&common::encoded(Role::Extension, &r));
    // Confirmed: it fills.
    let (r, seen) = common::with_window(&d, "confirmar", None, || {
        fill_raw(&d, Role::Extension, "acme-wp-dev", tab)
    });
    assert!(
        matches!(r, Resposta::Preenchimento { .. }),
        "{:?}",
        r.tipo()
    );
    assert_eq!(seen.len(), 1);
    // `https`: no window.
    let (r, seen) = common::with_window(&d, "negar", None, || {
        fill_raw(
            &d,
            Role::Extension,
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
    let (r, seen) = common::with_window(&d, "confirmar", None, || {
        fill_raw(&d, Role::Extension, "acme-wp-dev", "http://127.0.0.1:8081/")
    });
    assert_eq!(error(&r), Some(ResponseError::OriginMismatch));
    assert!(seen.is_empty());
}

/// Phase 44e (Medium of the 2nd M6 audit): the popup on a loopback `https` entry
/// (`https://localhost`, `https://[::1]`) also only fills after the confirmation. With a
/// local CA planted (mkcert, `~/.pki`), the process on `[::1]` passes TLS. Without an
/// app: "approval unavailable"; denied: no value; the request shows the origin.
#[test]
fn attack_fill_https_loopback_without_confirming_nothing_leaves() {
    let dir = Dir::new("ext-https-loopback");
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
        let r = request(
            &d,
            Role::App,
            "salvar_entrada",
            &login(&name, url, "wp_admin", SECRETS[0], None),
        );
        assert!(ok(&r), "{r:?}");
        let r = fill_raw(&d, Role::Extension, &name, url);
        assert_eq!(error(&r), Some(ResponseError::ApprovalUnavailable), "{url}");
        no_secret(&common::encoded(Role::Extension, &r));
        let (r, seen) = common::with_window(&d, "negar", None, || {
            fill_raw(&d, Role::Extension, &name, url)
        });
        assert_eq!(error(&r), Some(ResponseError::Denied), "{url}");
        no_secret(&common::encoded(Role::Extension, &r));
        assert_eq!(seen.len(), 1, "{url}");
        assert_eq!(seen[0].operacao, Operation::Fill);
        assert_eq!(seen[0].origem, origin, "{url}");
        assert!(seen[0].confirmacao, "{url}");
        let (r, _) = common::with_window(&d, "confirmar", None, || {
            fill_raw(&d, Role::Extension, &name, url)
        });
        assert!(matches!(r, Resposta::Preenchimento { .. }), "{url}");
    }
}
