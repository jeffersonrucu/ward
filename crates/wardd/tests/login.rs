//! Phase 43 (`ward login`; R14, D9; V6.3, V6.10, V6.11, V6.15; S1, S13), in the daemon
//! core: the agent asks, the extension (here in process, through the same request handler
//! as the socket with the extension role) takes the request, sends the tab's origin,
//! receives the fill only if the origin matches and reports the outcome.
//!
//! Attacks: phishing and redirect origin (the credential does not leave, and the same
//! id does not fill again), forged id, filling twice, outcome before the fill, agent
//! and app on the extension's operations, blocked, wrong type, locked vault and CLI that
//! drops in the middle, denied approval (nothing reaches the extension), no extension
//! running. Only fake values, in a test vault of its own.

#![cfg(feature = "test-hooks")]

mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::*;
use ward_core::protocol::{
    ApprovalRequest, LoginResult, Operation, Request, Response, ResponseError, Role,
};
use wardd::approval::{AppOpener, Client, Engine, EngineClock, NoClient, OpenAppError, Subscriber};
use wardd::core::Daemon;
use wardd::login::LoginDeadlines;

/// The URL of the `login` entries of the common test vault (free, approval and
/// blocked).
const ORIGIN: &str = "https://clientex.invalid";

fn deadlines(pickup_ms: u64) -> LoginDeadlines {
    LoginDeadlines {
        wait: Duration::from_millis(300),
        pickup: Duration::from_millis(pickup_ms),
        total: Duration::from_secs(5),
    }
}

fn vault(label: &str, pickup_ms: u64) -> (Daemon, Dir) {
    let dir = Dir::new(label);
    let (d, _) = test_vault(&dir);
    (d.with_login_deadlines(deadlines(pickup_ms)), dir)
}

fn agent_login(d: &Daemon, client: &dyn Client, name: &str) -> Response {
    match req(Role::Agent, "login", &format!(r#"{{"entrada":"{name}"}}"#)) {
        Ok(p) => d.serve_from(client, Role::Agent, p),
        Err(e) => Response::Error(e),
    }
}

/// The extension's await-login request: `Some((id, entry))` or `None` (empty wait).
fn await_login(d: &Daemon) -> Option<(String, String)> {
    match request(d, Role::Extension, "aguardar_login", "{}") {
        Response::LoginRequest { id, entry: entrada } => {
            // S1: the request taken to the extension has no value of the entry at all.
            let q = encoded(
                Role::Extension,
                &Response::LoginRequest {
                    id: id.clone(),
                    entry: entrada.clone(),
                },
            );
            for s in SECRETS {
                assert!(!contains(&q, s.as_bytes()));
                assert!(!contains(&q, b64(s.as_bytes()).as_bytes()));
            }
            Some((id, entrada))
        }
        Response::Ok => None,
        r => panic!("{r:?}"),
    }
}

/// Waits for the extension to see the request (the agent is in another thread).
fn take(d: &Daemon) -> (String, String) {
    for _ in 0..40 {
        if let Some(p) = await_login(d) {
            return p;
        }
    }
    panic!("o pedido não chegou à extensão");
}

fn fill(d: &Daemon, id: &str, origin: &str) -> Response {
    request(
        d,
        Role::Extension,
        "preencher_login",
        &format!(r#"{{"id":"{id}","origem":"{origin}"}}"#),
    )
}

fn outcome(d: &Daemon, id: &str, r: &str) -> Response {
    request(
        d,
        Role::Extension,
        "resultado_login",
        &format!(r#"{{"id":"{id}","resultado":"{r}"}}"#),
    )
}

fn no_value(r: &Response) {
    let q = encoded(Role::Extension, r);
    for s in SECRETS {
        assert!(!contains(&q, s.as_bytes()), "{r:?}");
        assert!(!contains(&q, b64(s.as_bytes()).as_bytes()), "{r:?}");
    }
}

/// V6.10 (core): free, right origin, fill once, sent → `ok`. The answer to the agent
/// has no value at all.
#[test]
fn v6_10_free_login_fills_on_the_right_origin_and_returns_ok() {
    let (d, _dir) = vault("login-ok", 2000);
    std::thread::scope(|s| {
        let t = s.spawn(|| agent_login(&d, &NoClient, "t-livre"));
        let (id, entry) = take(&d);
        assert_eq!(entry, "t-livre");
        assert_eq!(id.len(), 32);
        match fill(&d, &id, ORIGIN) {
            Response::Fill {
                username: usuario,
                password: senha,
                ..
            } => {
                assert_eq!(usuario, "admin");
                assert_eq!(senha.expose(), SECRETS[3].as_bytes());
            }
            r => panic!("{r:?}"),
        }
        // An id fills only once.
        let r = fill(&d, &id, ORIGIN);
        assert_eq!(error(&r), Some(ResponseError::Expired));
        no_value(&r);
        assert!(ok(&outcome(&d, &id, "enviado")));
        let r = t.join().unwrap();
        assert!(ok(&r), "{r:?}");
        // The agent's answer carries no credential (S1).
        let q = encoded(Role::Agent, &r);
        for secret in SECRETS {
            assert!(!contains(&q, secret.as_bytes()));
        }
    });
    // The request died: the id is no longer good and the queue is empty.
    assert_eq!(await_login(&d), None);
}

/// V6.3 and V6.15 (core): the tab on the phishing site (`localhost:8082`) or on the
/// page 8084 redirects to: "different origin" without a value, the agent gets the same
/// error, and the id does not fill afterwards even with the right origin.
#[test]
fn v6_3_v6_15_attack_different_origin_does_not_fill() {
    let (d, _dir) = vault("login-phish", 2000);
    for (i, tab) in [
        "http://localhost:8082",
        "http://localhost:8084",
        "https://clientex.invalid.phish.test",
        "https://sub.clientex.invalid",
        "http://clientex.invalid",
        "https://clientex.invalid:8443",
        "",
        "chrome://settings",
    ]
    .into_iter()
    .enumerate()
    {
        std::thread::scope(|s| {
            let t = s.spawn(|| agent_login(&d, &NoClient, "t-livre"));
            let (id, _) = take(&d);
            let r = fill(&d, &id, tab);
            assert_eq!(error(&r), Some(ResponseError::OriginMismatch), "{i}: {tab}");
            no_value(&r);
            let r = fill(&d, &id, ORIGIN);
            assert_eq!(error(&r), Some(ResponseError::Expired), "{tab}");
            no_value(&r);
            assert_eq!(
                error(&outcome(&d, &id, "enviado")),
                Some(ResponseError::Expired)
            );
            assert_eq!(
                error(&t.join().unwrap()),
                Some(ResponseError::OriginMismatch),
                "{tab}"
            );
        });
    }
}

/// Forged id (of another request, with one character changed, of another length):
/// nothing leaves, and the real request goes on.
#[test]
fn attack_forged_id_does_not_fill() {
    let (d, _dir) = vault("login-id", 2000);
    std::thread::scope(|s| {
        let t = s.spawn(|| agent_login(&d, &NoClient, "t-livre"));
        let (id, _) = take(&d);
        let mut changed = id.clone().into_bytes();
        changed[31] = if changed[31] == b'a' { b'b' } else { b'a' };
        let changed = String::from_utf8(changed).unwrap();
        for fake in [changed.as_str(), "AbCdEfGhIjKlMnOpQrStUvWxYz012345"] {
            let r = fill(&d, fake, ORIGIN);
            assert_eq!(error(&r), Some(ResponseError::Expired));
            no_value(&r);
            assert_eq!(
                error(&outcome(&d, fake, "enviado")),
                Some(ResponseError::Expired)
            );
        }
        for bad in ["", "x", &id[..31]] {
            let r = fill(&d, bad, ORIGIN);
            assert_eq!(error(&r), Some(ResponseError::InvalidRequest));
        }
        // "Sent" before filling does not count.
        assert_eq!(
            error(&outcome(&d, &id, "enviado")),
            Some(ResponseError::InvalidRequest)
        );
        assert!(matches!(fill(&d, &id, ORIGIN), Response::Fill { .. }));
        assert!(ok(&outcome(&d, &id, "recusado")));
        assert_eq!(error(&t.join().unwrap()), Some(ResponseError::LoginRefused));
    });
}

/// The extension's outcomes become the right error for the agent.
#[test]
fn extension_outcomes() {
    let (d, _dir) = vault("login-desfechos", 2000);
    for (r, expected) in [
        (LoginResult::NoForm, ResponseError::LoginRefused),
        (LoginResult::TabChanged, ResponseError::OriginMismatch),
        (LoginResult::Failure, ResponseError::BrowserFailed),
    ] {
        std::thread::scope(|s| {
            let t = s.spawn(|| agent_login(&d, &NoClient, "t-livre"));
            let (id, _) = take(&d);
            // Before the fill (the tab without a form, or that changed on the probe).
            assert!(ok(&outcome(&d, &id, r.name())));
            // Outcome again: changes nothing.
            assert_eq!(
                error(&outcome(&d, &id, "enviado")),
                Some(ResponseError::Expired)
            );
            assert_eq!(error(&t.join().unwrap()), Some(expected));
        });
    }
}

/// Agent and app do not take, do not fill and do not answer a login request (both
/// layers: the protocol reading and the core).
#[test]
fn attack_agent_and_app_on_the_extension_operations() {
    let (d, _dir) = vault("login-papeis", 2000);
    std::thread::scope(|s| {
        let t = s.spawn(|| agent_login(&d, &NoClient, "t-livre"));
        let (id, _) = take(&d);
        for role in [Role::Agent, Role::App] {
            for (op, data) in [
                ("aguardar_login", "{}".to_owned()),
                (
                    "preencher_login",
                    format!(r#"{{"id":"{id}","origem":"{ORIGIN}"}}"#),
                ),
                (
                    "resultado_login",
                    format!(r#"{{"id":"{id}","resultado":"enviado"}}"#),
                ),
            ] {
                let r = request(&d, role, op, &data);
                assert_eq!(error(&r), Some(ResponseError::Unauthorized), "{op}");
            }
            for p in [
                Request::AwaitLogin,
                Request::FillLogin {
                    id: id.clone(),
                    origin: ORIGIN.into(),
                },
                Request::LoginResult {
                    id: id.clone(),
                    result: LoginResult::Submitted,
                },
            ] {
                let r = d.serve(role, p);
                assert_eq!(error(&r), Some(ResponseError::Unauthorized));
            }
        }
        // The extension does not ask for `login` (only the agent).
        let r = request(&d, Role::Extension, "login", r#"{"entrada":"t-livre"}"#);
        assert_eq!(error(&r), Some(ResponseError::Unauthorized));
        // The real request goes on intact.
        assert!(matches!(fill(&d, &id, ORIGIN), Response::Fill { .. }));
        assert!(ok(&outcome(&d, &id, "enviado")));
        assert!(ok(&t.join().unwrap()));
    });
}

/// Blocked = nonexistent (V3.17), wrong type and nonexistent: refused before any
/// request to the extension.
#[test]
fn refusals_before_the_extension() {
    let (d, _dir) = vault("login-recusas", 2000);
    for (name, expected) in [
        ("t-bloq", ResponseError::EntryNotFound),
        ("nao-existe", ResponseError::EntryNotFound),
        ("s1", ResponseError::TypeWithoutDelivery),
    ] {
        let r = agent_login(&d, &NoClient, name);
        assert_eq!(error(&r), Some(expected), "{name}");
        assert_eq!(await_login(&d), None, "{name}");
    }
}

/// No extension running: the agent gets "browser failed" within the deadline and the
/// request dies (an extension that arrives later does not see it).
#[test]
fn without_extension_the_request_dies() {
    let (d, _dir) = vault("login-sem-ext", 300);
    let r = agent_login(&d, &NoClient, "t-livre");
    assert_eq!(error(&r), Some(ResponseError::BrowserFailed));
    assert_eq!(await_login(&d), None);
}

/// One request at a time: the second agent gets "busy".
#[test]
fn one_request_at_a_time() {
    let (d, _dir) = vault("login-ocupado", 2000);
    std::thread::scope(|s| {
        let t = s.spawn(|| agent_login(&d, &NoClient, "t-livre"));
        let (id, _) = take(&d);
        let r = agent_login(&d, &NoClient, "t-livre");
        assert_eq!(error(&r), Some(ResponseError::Busy));
        assert!(ok(&outcome(&d, &id, "falha")));
        assert_eq!(
            error(&t.join().unwrap()),
            Some(ResponseError::BrowserFailed)
        );
    });
}

/// Vault locked with the request delivered: the agent gets "locked" and the id does
/// not fill.
#[test]
fn lock_in_the_middle_kills_the_request() {
    let (d, _dir) = vault("login-travar", 2000);
    std::thread::scope(|s| {
        let t = s.spawn(|| agent_login(&d, &NoClient, "t-livre"));
        let (id, _) = take(&d);
        assert!(ok(&request(&d, Role::App, "travar", "{}")));
        assert_eq!(error(&t.join().unwrap()), Some(ResponseError::Locked));
        let r = fill(&d, &id, ORIGIN);
        assert!(error(&r).is_some(), "{r:?}");
        no_value(&r);
    });
}

struct LeavingClient(AtomicBool);

impl Client for LeavingClient {
    fn chain(&self) -> Vec<String> {
        vec!["claude".into(), "ward login".into()]
    }
    fn exited(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// The CLI that drops (Ctrl+C) with the request delivered: the request dies and nothing
/// is filled.
#[test]
fn cli_that_drops_kills_the_request() {
    let (d, _dir) = vault("login-cli", 2000);
    let client = LeavingClient(AtomicBool::new(false));
    std::thread::scope(|s| {
        let t = s.spawn(|| agent_login(&d, &client, "t-livre"));
        let (id, _) = take(&d);
        client.0.store(true, Ordering::Release);
        assert_eq!(error(&t.join().unwrap()), Some(ResponseError::Denied));
        let r = fill(&d, &id, ORIGIN);
        assert_eq!(error(&r), Some(ResponseError::Expired));
        no_value(&r);
    });
}

/// The extension's await-login request (every 25 s) does not count as use: it does not
/// hold off the inactivity lock (Phase 24).
#[test]
fn await_does_not_count_as_use() {
    assert!(!wardd::lock::counts_as_use(
        Role::Extension,
        Operation::AwaitLogin,
        &Response::Ok
    ));
    assert!(wardd::lock::counts_as_use(
        Role::Extension,
        Operation::FillLogin,
        &Response::Ok
    ));
}

// ---- V6.11: with approval, deny → nothing reaches the extension ----

#[derive(Clone, Default)]
struct StoppedClock;

impl EngineClock for StoppedClock {
    fn now(&self) -> Duration {
        Duration::ZERO
    }
}

struct FakeApp(Sender<ApprovalRequest>);

impl Subscriber for FakeApp {
    fn deliver(&mut self, r: &Response) -> bool {
        match r {
            Response::ApprovalRequest(p) => self.0.send(p.clone()).is_ok(),
            other => panic!("o motor mandou ao app {other:?}"),
        }
    }
}

struct NoOpen;

impl AppOpener for NoOpen {
    fn open(&self, _on_exit: Box<dyn FnOnce() + Send>) -> Result<(), OpenAppError> {
        Ok(())
    }
}

fn answer(d: &Daemon, id: u64, decision: &str, pin: Option<&str>) -> Response {
    let pin = pin
        .map(|p| format!(r#","pin":"{}""#, b64(p.as_bytes())))
        .unwrap_or_default();
    request(
        d,
        Role::App,
        "responder_aprovacao",
        &format!(r#"{{"pedido":{id},"decisao":"{decision}"{pin}}}"#),
    )
}

#[test]
fn v6_11_with_approval_deny_does_not_reach_the_extension_and_allow_does() {
    let (d, _dir) = vault("login-aprov", 2000);
    let d = d.with_engine(Engine::new(Arc::new(StoppedClock), Box::new(NoOpen)));
    let (tx, rx) = mpsc::channel();
    d.engine().subscribe(1, Box::new(FakeApp(tx)));
    let seen = Mutex::new(Vec::new());
    // Deny: the extension sees no request during or after.
    std::thread::scope(|s| {
        let t = s.spawn(|| agent_login(&d, &NoClient, "t-aprov"));
        let p = rx.recv_timeout(Duration::from_secs(5)).expect("sem janela");
        assert_eq!(p.operation, Operation::Login);
        assert_eq!(p.entries, vec!["t-aprov".to_owned()]);
        assert_eq!(await_login(&d), None);
        assert!(ok(&answer(&d, p.id, "negar", None)));
        assert_eq!(error(&t.join().unwrap()), Some(ResponseError::Denied));
        seen.lock().unwrap().push(await_login(&d));
    });
    assert_eq!(seen.lock().unwrap().as_slice(), &[None]);
    // Allow once (with the PIN): the request arrives and the login goes on.
    std::thread::scope(|s| {
        let t = s.spawn(|| agent_login(&d, &NoClient, "t-aprov"));
        let p = rx.recv_timeout(Duration::from_secs(5)).expect("sem janela");
        assert!(ok(&answer(&d, p.id, "uma_vez", Some(PIN))));
        let (id, entry) = take(&d);
        assert_eq!(entry, "t-aprov");
        assert!(matches!(fill(&d, &id, ORIGIN), Response::Fill { .. }));
        assert!(ok(&outcome(&d, &id, "enviado")));
        assert!(ok(&t.join().unwrap()));
    });
}

// ---- Phase 44c (D9): `http` origin always through the window ----

/// `login` entry on `http://localhost:8081` at the given level.
fn http_login(d: &Daemon, name: &str, level: &str) {
    login_at_url(d, name, level, "http://localhost:8081/wp-login.php");
}

/// `login` entry at the given URL and level.
fn login_at_url(d: &Daemon, name: &str, level: &str, url: &str) {
    let q = format!(
        r#"{{"entrada":{{"nome":"{name}","grupo":"acme","ambiente":"prod","tipo":"login","campos":{{"url":"{url}","usuario":"wp_editor","senha":"{}"}}}}}}"#,
        b64(SECRETS[3].as_bytes())
    );
    let r = request(d, Role::App, "salvar_entrada", &q);
    assert!(ok(&r), "{r:?}");
    // A new entry already starts with "approval" (D50).
    if level != "com-aprovacao" {
        let r = set_level(d, Role::App, name, level, PIN);
        assert!(ok(&r), "{r:?}");
    }
}

/// Client that drops after a few turns of the wait (to end a request nobody decides
/// without waiting the 2 min).
struct DropsLater(std::sync::atomic::AtomicU32);

impl Client for DropsLater {
    fn chain(&self) -> Vec<String> {
        vec!["claude".into(), "ward login".into()]
    }
    fn exited(&self) -> bool {
        self.0.fetch_add(1, Ordering::AcqRel) > 40
    }
}

/// The agent asks for `login`, the extension takes it, fills on the `http` origin and
/// says "sent".
fn full_http_login(d: &Daemon, name: &str) -> Response {
    std::thread::scope(|s| {
        let t = s.spawn(|| agent_login(d, &NoClient, name));
        let (id, _) = take(d);
        assert!(matches!(
            fill(d, &id, "http://localhost:8081"),
            Response::Fill { .. }
        ));
        assert!(ok(&outcome(d, &id, "enviado")));
        t.join().unwrap()
    })
}

/// Attack from the M6 audit (`[::1]:8081` with WordPress only on `127.0.0.1`): the tab's
/// origin matches, so the daemon cannot tell them apart. `ward login` on a **free**
/// `http` entry does not go to the extension without the confirmation in the window:
/// without an app, "approval unavailable"; denied, nothing; the request carries the
/// origin, is only a confirmation (no PIN), and an "Allow" with a PIN does not decide
/// it. Confirmed, it goes on as before.
#[test]
fn attack_login_http_free_only_with_confirmation() {
    let (d, _dir) = vault("login-http-livre", 2000);
    http_login(&d, "h-livre", "livre");
    // No app.
    let r = agent_login(&d, &NoClient, "h-livre");
    assert_eq!(error(&r), Some(ResponseError::ApprovalUnavailable));
    assert_eq!(await_login(&d), None);
    // Denied: the extension sees nothing.
    let (r, seen) = with_window(&d, "negar", None, || agent_login(&d, &NoClient, "h-livre"));
    assert_eq!(error(&r), Some(ResponseError::Denied));
    assert_eq!(await_login(&d), None);
    let p = &seen[0];
    assert_eq!(p.operation, Operation::Login);
    assert_eq!(p.entries, ["h-livre".to_owned()]);
    assert_eq!(p.origin, "http://localhost:8081");
    assert!(p.confirmation);
    // "Allow once" with a PIN on a confirmation: refused, and the request never arrives.
    let client = DropsLater(std::sync::atomic::AtomicU32::new(0));
    let (r, _) = with_window(&d, "uma_vez", Some(PIN), || {
        agent_login(&d, &client, "h-livre")
    });
    assert_eq!(error(&r), Some(ResponseError::Denied));
    assert_eq!(await_login(&d), None);
    // Confirmed: the request goes to the extension and fills on the right origin.
    let (r, seen) = with_window(&d, "confirmar", None, || full_http_login(&d, "h-livre"));
    assert!(ok(&r), "{r:?}");
    assert_eq!(seen.len(), 1);
}

/// "Approval" on an `http` origin: the usual window (with a PIN), now with the origin;
/// "Confirm" (without a PIN) does not decide it. The 1-hour release does not skip the
/// origin confirmation: the next use still opens the window (only the confirmation).
#[test]
fn attack_login_http_approval_asks_pin_and_shows_the_origin() {
    let (d, _dir) = vault("login-http-aprov", 2000);
    http_login(&d, "h-aprov", "com-aprovacao");
    let client = DropsLater(std::sync::atomic::AtomicU32::new(0));
    let (r, seen) = with_window(&d, "confirmar", None, || {
        agent_login(&d, &client, "h-aprov")
    });
    assert_eq!(
        error(&r),
        Some(ResponseError::Denied),
        "confirmar sem PIN decidiu"
    );
    assert_eq!(await_login(&d), None);
    assert!(!seen[0].confirmation);
    assert_eq!(seen[0].origin, "http://localhost:8081");
    // Allow for 1 hour (with a PIN): it goes on.
    let (r, seen) = with_window(&d, "uma_hora", Some(PIN), || full_http_login(&d, "h-aprov"));
    assert!(ok(&r), "{r:?}");
    assert!(!seen[0].confirmation);
    // Within the release: still the origin confirmation (without a PIN).
    let (r, seen) = with_window(&d, "confirmar", None, || full_http_login(&d, "h-aprov"));
    assert!(ok(&r), "{r:?}");
    assert_eq!(seen.len(), 1);
    assert!(seen[0].confirmation);
    // No app, nothing.
    let r = agent_login(&d, &NoClient, "h-aprov");
    assert_eq!(error(&r), Some(ResponseError::ApprovalUnavailable));
}

/// Phase 44e (Medium of the 2nd M6 audit): `https` on a loopback host also asks for the
/// confirmation. With a local CA (mkcert, `~/.pki`), a process of the uid on `[::1]`
/// presents a valid certificate for `localhost`; `https` does not prove who answers.
/// Free: without an app, "approval unavailable"; denied, the extension sees nothing;
/// the request shows the `https` origin and is only a confirmation.
#[test]
fn attack_login_https_loopback_free_only_with_confirmation() {
    let (d, _dir) = vault("login-https-loopback", 2000);
    for (i, (url, origin)) in [
        (
            "https://localhost:8443/wp-login.php",
            "https://localhost:8443",
        ),
        ("https://127.0.0.1/wp-login.php", "https://127.0.0.1"),
        ("https://[::1]:8443/wp-login.php", "https://[::1]:8443"),
        ("https://wp.localhost/wp-login.php", "https://wp.localhost"),
        ("https://[::ffff:127.0.0.1]/", "https://[::ffff:127.0.0.1]"),
    ]
    .into_iter()
    .enumerate()
    {
        let name = format!("l-livre-{i}");
        login_at_url(&d, &name, "livre", url);
        let r = agent_login(&d, &NoClient, &name);
        assert_eq!(error(&r), Some(ResponseError::ApprovalUnavailable), "{url}");
        assert_eq!(await_login(&d), None, "{url}");
        let (r, seen) = with_window(&d, "negar", None, || agent_login(&d, &NoClient, &name));
        assert_eq!(error(&r), Some(ResponseError::Denied), "{url}");
        assert_eq!(await_login(&d), None, "{url}");
        assert_eq!(seen.len(), 1, "{url}");
        assert_eq!(seen[0].origin, origin, "{url}");
        assert!(seen[0].confirmation, "{url}");
    }
}

/// Outside `https` does not change: free opens no window at all.
#[test]
fn login_https_free_without_window() {
    let (d, _dir) = vault("login-https-livre", 2000);
    let (r, seen) = with_window(&d, "negar", None, || {
        std::thread::scope(|s| {
            let t = s.spawn(|| agent_login(&d, &NoClient, "t-livre"));
            let (id, _) = take(&d);
            assert!(matches!(fill(&d, &id, ORIGIN), Response::Fill { .. }));
            assert!(ok(&outcome(&d, &id, "enviado")));
            t.join().unwrap()
        })
    });
    assert!(ok(&r), "{r:?}");
    assert!(seen.is_empty());
}
