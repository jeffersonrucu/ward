//! Approval engine (Phase 23): V3.14–V3.16, V3.18, V3.36 (classifier), D19, D21, D51,
//! D57 (opening the app).
//!
//! The approver is in process (D8): a fake subscriber receives what the engine sends to
//! the app and the answer comes back through the daemon request handler with the app
//! role, the same path as the socket (the request goes through the protocol parsing).
//! The engine clock is manual.

mod common;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::*;
use ward_core::entry::{AccessLevel, Environment};
use ward_core::protocol::{
    ApprovalRequest, Operation, Response, ResponseError, Role, UnlockRequest,
};
use ward_platform::{PeerClass, Refusal};
use wardd::approval::{AppOpener, Client, Engine, EngineClock, NoClient, OpenAppError, Subscriber};
use wardd::core::Daemon;
use wardd::server::role_from_class;
use wardd::usage::Authorization;

const DEADLINE: Duration = Duration::from_secs(5);

#[derive(Clone, Default)]
struct ManualClock(Arc<Mutex<Duration>>);

impl ManualClock {
    fn set(&self, t: Duration) {
        *self.0.lock().unwrap() = t;
    }
    fn advance(&self, d: Duration) {
        *self.0.lock().unwrap() += d;
    }
}

impl EngineClock for ManualClock {
    fn now(&self) -> Duration {
        *self.0.lock().unwrap()
    }
}

/// What the fake app received.
#[derive(Debug)]
enum Received {
    Approval(ApprovalRequest),
    Unlock(UnlockRequest),
    ShowWindow,
}

struct FakeApp(Sender<Received>);

impl Subscriber for FakeApp {
    fn deliver(&mut self, r: &Response) -> bool {
        let x = match r {
            Response::ApprovalRequest(p) => Received::Approval(p.clone()),
            Response::UnlockRequest(p) => Received::Unlock(p.clone()),
            Response::ShowWindow => Received::ShowWindow,
            other => panic!("o motor mandou ao app {other:?}"),
        };
        self.0.send(x).is_ok()
    }
}

type OnExit = Box<dyn FnOnce() + Send>;

/// Fake opener: counts the openings and keeps the `on_exit`.
#[derive(Clone)]
struct FakeOpener {
    result: Result<(), OpenAppError>,
    opens: Arc<AtomicU32>,
    on_exit: Arc<Mutex<Option<OnExit>>>,
}

impl FakeOpener {
    fn new(result: Result<(), OpenAppError>) -> Self {
        Self {
            result,
            opens: Arc::default(),
            on_exit: Arc::default(),
        }
    }
    fn exit(&self) {
        if let Some(f) = self.on_exit.lock().unwrap().take() {
            f();
        }
    }
}

impl AppOpener for FakeOpener {
    fn open(&self, on_exit: Box<dyn FnOnce() + Send>) -> Result<(), OpenAppError> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        if self.result.is_ok() {
            *self.on_exit.lock().unwrap() = Some(on_exit);
        }
        self.result
    }
}

struct Scene {
    d: Daemon,
    _dir: Dir,
    clock: ManualClock,
    opener: FakeOpener,
}

fn scene_with(label: &str, open: Result<(), OpenAppError>) -> Scene {
    let dir = Dir::new(label);
    let (d, _) = test_vault(&dir);
    let clock = ManualClock::default();
    let opener = FakeOpener::new(open);
    let d = d.with_engine(Engine::new(
        Arc::new(clock.clone()),
        Box::new(opener.clone()),
    ));
    Scene {
        d,
        _dir: dir,
        clock,
        opener,
    }
}

fn scene(label: &str) -> Scene {
    scene_with(label, Ok(()))
}

/// The fake app subscribes (like the app role connection in the server).
fn subscribe(d: &Daemon, id: u64) -> Receiver<Received> {
    let (tx, rx) = mpsc::channel();
    d.engine().subscribe(id, Box::new(FakeApp(tx)));
    rx
}

fn run_json(name: &str, argv: &[&[u8]]) -> String {
    run_json_in(name, argv, b"/")
}

fn run_json_in(name: &str, argv: &[&[u8]], cwd: &[u8]) -> String {
    let argv: Vec<String> = argv.iter().map(|a| format!("\"{}\"", b64(a))).collect();
    format!(
        r#"{{"entry":"{name}","argv":[{}],"cwd":"{}","stdin_kind":"null","environment":[]}}"#,
        argv.join(","),
        b64(cwd)
    )
}

fn use_entry(d: &Daemon, client: &dyn Client, name: &str) -> Result<Authorization, ResponseError> {
    let p = req(Role::Agent, "run", &run_json(name, &[b"true"])).unwrap();
    d.authorize_use(client, &p)
}

fn approval_request(rx: &Receiver<Received>) -> ApprovalRequest {
    match rx
        .recv_timeout(DEADLINE)
        .expect("o app não recebeu o pedido")
    {
        Received::Approval(p) => p,
        other => panic!("{other:?}"),
    }
}

fn answer(d: &Daemon, role: Role, id: u64, decision: &str, pin: Option<&str>) -> Response {
    let pin = pin
        .map(|p| format!(r#","pin":"{}""#, b64(p.as_bytes())))
        .unwrap_or_default();
    request(
        d,
        role,
        "answer_approval",
        &format!(r#"{{"request":{id},"decision":"{decision}"{pin}}}"#),
    )
}

fn level(d: &Daemon, name: &str) -> Option<AccessLevel> {
    match request(d, Role::App, "list", "{}") {
        Response::List(l) => l.into_iter().find(|i| i.name == name).and_then(|i| i.level),
        r => panic!("{r:?}"),
    }
}

/// If the test panics with a use waiting, the clock jumps past the deadline and the wait
/// ends (without this the `scope` would wait for the thread forever).
struct ExpireOnPanic<'a>(&'a ManualClock);

impl Drop for ExpireOnPanic<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.advance(Duration::from_secs(100_000));
        }
    }
}

/// A use that asks: returns the result after `respond` with the request.
fn with_approval_window(
    c: &Scene,
    rx: &Receiver<Received>,
    name: &str,
    answer: impl FnOnce(&ApprovalRequest),
) -> Result<Authorization, ResponseError> {
    std::thread::scope(|s| {
        let t = s.spawn(|| use_entry(&c.d, &NoClient, name));
        let _g = ExpireOnPanic(&c.clock);
        let p = approval_request(rx);
        answer(&p);
        t.join().unwrap()
    })
}

/// V3.14 (S10): without an answer, expires in 2 min by the injected clock (not before).
#[test]
fn v3_14_no_answer_expires_in_2_min() {
    let c = scene("expira");
    let rx = subscribe(&c.d, 1);
    let r = with_approval_window(&c, &rx, "t-aprov", |p| {
        c.clock.set(Duration::from_secs(119));
        std::thread::sleep(Duration::from_millis(150));
        assert!(
            c.d.engine().pending(p.id).is_some(),
            "expirou antes de 2 min"
        );
        c.clock.set(Duration::from_secs(120));
    });
    assert_eq!(r, Err(ResponseError::Expired));
    // Answering after it expired allows nothing.
    c.clock.set(Duration::ZERO);
    assert_eq!(
        error(&answer(&c.d, Role::App, 1, "once", Some(PIN))),
        Some(ResponseError::Expired)
    );
}

/// V3.14 (S10): closing the window denies (the "Deny" button, closing the window, which
/// sends a deny, and the app connection that drops with the request delivered).
#[test]
fn v3_14_deny_and_close_deny() {
    let c = scene("fechar");
    let rx = subscribe(&c.d, 1);
    let r = with_approval_window(&c, &rx, "t-aprov", |p| {
        assert!(ok(&answer(&c.d, Role::App, p.id, "deny", None)));
    });
    assert_eq!(r, Err(ResponseError::Denied));
    let r = with_approval_window(&c, &rx, "t-aprov", |_| c.d.engine().unsubscribe(1));
    assert_eq!(r, Err(ResponseError::Denied));
    assert!(!c.d.engine().has_subscriber());
}

/// V3.14: "Allow once" holds only for that request; the next one asks again.
#[test]
fn v3_14_once_only_that_request() {
    let c = scene("uma-vez");
    let rx = subscribe(&c.d, 1);
    let r = with_approval_window(&c, &rx, "t-aprov", |p| {
        assert!(ok(&answer(&c.d, Role::App, p.id, "once", Some(PIN))));
    });
    assert_eq!(r, Ok(Authorization::Approved));
    let r = with_approval_window(&c, &rx, "t-aprov", |p| {
        assert!(ok(&answer(&c.d, Role::App, p.id, "deny", None)));
    });
    assert_eq!(r, Err(ResponseError::Denied));
}

/// V3.14 (D2): "Allow for 1 hour" holds for the entry until 59:59; at 60:01 it asks;
/// locking ends the grant; changing the level too.
#[test]
fn v3_14_one_hour_until_59_59_and_lock_ends_it() {
    let c = scene("uma-hora");
    let rx = subscribe(&c.d, 1);
    c.clock.set(Duration::from_secs(1000));
    let r = with_approval_window(&c, &rx, "t-aprov", |p| {
        assert!(ok(&answer(&c.d, Role::App, p.id, "one_hour", Some(PIN))));
    });
    assert_eq!(r, Ok(Authorization::Approved));
    for t in [1001, 2000, 1000 + 3599] {
        c.clock.set(Duration::from_secs(t));
        assert_eq!(
            use_entry(&c.d, &NoClient, "t-aprov"),
            Ok(Authorization::WithoutAsking)
        );
    }
    assert!(rx.try_recv().is_err(), "perguntou dentro da hora");
    // Only that entry.
    let r = with_approval_window(&c, &rx, "s1", |p| {
        assert!(ok(&answer(&c.d, Role::App, p.id, "deny", None)));
    });
    assert_eq!(r, Err(ResponseError::Denied));
    c.clock.set(Duration::from_secs(1000 + 3601));
    let r = with_approval_window(&c, &rx, "t-aprov", |p| {
        assert!(ok(&answer(&c.d, Role::App, p.id, "one_hour", Some(PIN))));
    });
    assert_eq!(r, Ok(Authorization::Approved));
    assert_eq!(
        use_entry(&c.d, &NoClient, "t-aprov"),
        Ok(Authorization::WithoutAsking)
    );
    // Lock and unlock: asks again.
    assert!(ok(&request(&c.d, Role::Agent, "lock", "{}")));
    assert!(ok(&unlock(&c.d, PASSWORD)));
    let r = with_approval_window(&c, &rx, "t-aprov", |p| {
        assert!(ok(&answer(&c.d, Role::App, p.id, "one_hour", Some(PIN))));
    });
    assert_eq!(r, Ok(Authorization::Approved));
    // Changing the level (and back) also ends it.
    assert!(ok(&set_level(&c.d, Role::App, "t-aprov", "free", PIN)));
    assert!(ok(&set_level(&c.d, Role::App, "t-aprov", "approval", PIN)));
    let r = with_approval_window(&c, &rx, "t-aprov", |p| {
        assert!(ok(&answer(&c.d, Role::App, p.id, "deny", None)));
    });
    assert_eq!(r, Err(ResponseError::Denied));
}

/// V3.14: "Always allow" leaves the entry free in the file (holds after locking).
#[test]
fn v3_14_always_leaves_free_in_the_file() {
    let c = scene("sempre");
    let rx = subscribe(&c.d, 1);
    let r = with_approval_window(&c, &rx, "t-aprov", |p| {
        assert!(ok(&answer(&c.d, Role::App, p.id, "always", Some(PIN))));
    });
    assert_eq!(r, Ok(Authorization::Approved));
    assert!(ok(&request(&c.d, Role::Agent, "lock", "{}")));
    assert!(ok(&unlock(&c.d, PASSWORD)));
    assert_eq!(level(&c.d, "t-aprov"), Some(AccessLevel::Free));
    assert_eq!(
        use_entry(&c.d, &NoClient, "t-aprov"),
        Ok(Authorization::WithoutAsking)
    );
}

/// Phase 49a (low finding of M7): restoring with a pending approval request denies the
/// request; the "always allow" that arrives later (with the right PIN) answers "expired"
/// and does not change the level of the entry with the same name in the restored
/// version.
#[test]
fn attack_49a_restore_cancels_pending_request() {
    let c = scene("restaurar-pendente");
    let rx = subscribe(&c.d, 1);
    let version = match request(&c.d, Role::App, "backups", "{}") {
        Response::Backups(l) => l[0].name.clone(),
        r => panic!("{r:?}"),
    };
    let r = std::thread::scope(|s| {
        let t = s.spawn(|| use_entry(&c.d, &NoClient, "t-aprov"));
        let _g = ExpireOnPanic(&c.clock);
        let p = approval_request(&rx);
        let r = request(
            &c.d,
            Role::App,
            "restore_backup",
            &format!(
                r#"{{"version":"{version}","password":"{}","pin":"{}"}}"#,
                b64(PASSWORD.as_bytes()),
                b64(PIN.as_bytes())
            ),
        );
        assert!(ok(&r), "{r:?}");
        let result = t.join().unwrap();
        let late = answer(&c.d, Role::App, p.id, "always", Some(PIN));
        assert_eq!(error(&late), Some(ResponseError::Expired));
        result
    });
    assert_eq!(r, Err(ResponseError::Denied));
    assert_eq!(level(&c.d, "t-aprov"), Some(AccessLevel::Approval));
    assert!(ok(&request(&c.d, Role::Agent, "lock", "{}")));
    assert!(ok(&unlock(&c.d, PASSWORD)));
    assert_eq!(level(&c.d, "t-aprov"), Some(AccessLevel::Approval));
}

/// Attack (V3.14, S9/S10): the three "Allow" without a valid PIN do not allow; a wrong
/// PIN leaves the request pending; the 5th error locks the vault and denies the
/// request.
#[test]
fn attack_allow_without_valid_pin() {
    let c = scene("sem-pin");
    let rx = subscribe(&c.d, 1);
    let r = with_approval_window(&c, &rx, "t-aprov", |p| {
        for decision in ["once", "one_hour", "always"] {
            // No PIN: the protocol refuses the whole request.
            assert_eq!(
                error(&answer(&c.d, Role::App, p.id, decision, None)),
                Some(ResponseError::InvalidRequest)
            );
            // PIN out of format.
            assert_eq!(
                error(&answer(&c.d, Role::App, p.id, decision, Some("12"))),
                Some(ResponseError::InvalidRequest)
            );
        }
        for (i, decision) in ["once", "one_hour", "always", "once"].iter().enumerate() {
            assert_eq!(
                error(&answer(&c.d, Role::App, p.id, decision, Some(WRONG_PIN))),
                Some(ResponseError::WrongPin),
                "{i}"
            );
            assert!(
                c.d.engine().pending(p.id).is_some(),
                "o pedido saiu no erro {i}"
            );
        }
        assert_eq!(
            error(&answer(&c.d, Role::App, p.id, "always", Some(WRONG_PIN))),
            Some(ResponseError::WrongPin)
        );
    });
    assert_eq!(r, Err(ResponseError::Denied));
    assert!(c.d.is_locked());
    assert!(ok(&unlock(&c.d, PASSWORD)));
    assert_eq!(level(&c.d, "t-aprov"), Some(AccessLevel::Approval));
}

/// Attack (V3.15, V5.20): the agent (and the extension) send "answer approval" with the
/// right PIN: "not authorized", and the request stays pending until the app decides.
#[test]
fn attack_agent_answers_the_approval() {
    let c = scene("agente-responde");
    let rx = subscribe(&c.d, 1);
    let r = with_approval_window(&c, &rx, "t-aprov", |p| {
        for role in [Role::Agent, Role::Extension] {
            for decision in ["once", "one_hour", "always"] {
                assert_eq!(
                    error(&answer(&c.d, role, p.id, decision, Some(PIN))),
                    Some(ResponseError::Unauthorized)
                );
            }
            // Not even through the request handler directly with the request built by hand.
            let q = ward_core::protocol::Request::AnswerApproval {
                request: p.id,
                decision: ward_core::protocol::Decision::AllowOnce,
                pin: Some(ward_core::entry::Secret::from_text(PIN.to_owned())),
                confirmed_entry: false,
            };
            assert_eq!(
                error(&c.d.serve(role, q)),
                Some(ResponseError::Unauthorized)
            );
        }
        assert!(c.d.engine().pending(p.id).is_some());
        assert!(ok(&answer(&c.d, Role::App, p.id, "deny", None)));
    });
    assert_eq!(r, Err(ResponseError::Denied));
}

/// V3.15 (D8): the daemon code has no approver nor any variable/argument that changes
/// the approval. Only the config, log, main and lock modules (at the top level) read the
/// environment, and no file in `src/` (subfolders included, M3 scrutiny) has
/// `cfg(feature` other than the `test-mode` of the paths and the `test-hooks` of the
/// core test hooks (listed in `TEST_HOOKS`), each right before its own `pub fn`.
#[test]
fn v3_15_no_approver_in_the_binary() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let files = rs_sources(&src);
    assert!(files.len() > 10, "poucos arquivos em src/: {files:?}");
    let mut hooks = Vec::new();
    for p in &files {
        let rel = p.strip_prefix(&src).unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(p).unwrap();
        check_v3_15(&rel, &text, &mut hooks);
    }
    hooks.sort();
    assert_eq!(hooks, TEST_HOOKS, "ganchos de teste fora do esperado");
}

/// All the `.rs` files of `dir`, descending into subfolders; a link is refused.
fn rs_sources(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let e = e.unwrap();
        let kind = e.file_type().unwrap();
        let p = e.path();
        assert!(!kind.is_symlink(), "link em src/: {}", p.display());
        if kind.is_dir() {
            v.extend(rs_sources(&p));
        } else if p.extension().is_some_and(|x| x == "rs") {
            v.push(p);
        }
    }
    v
}

/// In alphabetical order (the test compares with the list found, sorted).
/// The login deadlines hook (Phase 43) only shortens the `ward login` waits in the tests;
/// it does not touch the approval nor deliver a value.
/// The migration hook (m2:DT5) only observes or stops the v4 → v5 write in the tests
/// (m2:V4.3); it changes nothing in the write.
const TEST_HOOKS: [&str; 5] = [
    "with_chrome",
    "with_hook_after_write",
    "with_login_deadlines",
    "with_migration_hook",
    "with_ssh_programs",
];
const CFG_HOOKS: &str = "#[cfg(feature = \"test-hooks\")]";

fn check_v3_15(rel: &str, text: &str, hooks: &mut Vec<&'static str>) {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    // m2:Phase 56: the old (pt-BR) names and the new ones of the test approver.
    for forbidden in [
        "env::args",
        "aprovador_teste",
        "WARD_APROV",
        "auto_aprov",
        "test_approver",
        "test-approver",
        "WARD_APPROV",
        "auto_approv",
    ] {
        assert!(!text.contains(forbidden), "{rel}: {forbidden}");
    }
    // The lock module (Phase 24): only the test logind bus, outside the approval.
    if rel.contains('/') || !["config.rs", "log.rs", "main.rs", "lock.rs"].contains(&name) {
        assert!(!text.contains("env::var"), "{rel} lê o ambiente");
    }
    for (i, _) in text.match_indices("cfg(feature") {
        let rest = &text[i..];
        if rest.starts_with("cfg(feature = \"test-mode\")") {
            assert!(
                !["approval.rs", "usage.rs", "server.rs", "core.rs"].contains(&rel),
                "{rel}: test-mode no caminho da aprovação"
            );
            continue;
        }
        assert!(
            rest.starts_with(&CFG_HOOKS[2..]),
            "{rel}: feature desconhecida"
        );
        assert_eq!(rel, "core.rs", "test-hooks fora do núcleo");
        // The first line after the attribute that is neither an attribute nor a comment
        // must be the `pub fn` of one of the hooks.
        let after = rest.split_once('\n').map(|(_, d)| d).unwrap_or_default();
        let signature = after
            .lines()
            .map(str::trim)
            .find(|l| !l.starts_with("#[") && !l.starts_with("//"))
            .unwrap_or_default();
        let hook = TEST_HOOKS
            .iter()
            .find(|g| signature.starts_with(&format!("pub fn {g}(")))
            .unwrap_or_else(|| panic!("{rel}: test-hooks em outro item: {signature}"));
        hooks.push(hook);
    }
    // Every hook definition comes with the feature, in one of the 3 lines above.
    for g in TEST_HOOKS {
        for (i, _) in text.match_indices(&format!("fn {g}(")) {
            let with_feature = text[..i]
                .lines()
                .rev()
                .take(3)
                .any(|l| l.trim() == CFG_HOOKS);
            assert!(with_feature, "{rel}: {g} sem a feature test-hooks");
        }
    }
}

/// Attack on V3.15: a hook without the feature, the feature on another item or file, an
/// unknown feature, `env::var` in a subfolder and `test-mode` on the approval path are
/// refused.
#[test]
fn attack_v3_15_hook_without_feature_or_out_of_place() {
    let right =
        "#[cfg(feature = \"test-hooks\")]\n#[doc(hidden)]\npub fn with_ssh_programs(x) {}\n";
    let mut g = Vec::new();
    check_v3_15("core.rs", right, &mut g);
    assert_eq!(g, ["with_ssh_programs"]);
    let cases = [
        ("core.rs", "pub fn with_hook_after_write(x) {}\n"),
        (
            "core.rs",
            "#[cfg(feature = \"test-hooks\")]\npub fn aprovar_tudo() {}\n",
        ),
        ("server.rs", right),
        ("core.rs", "#[cfg(feature = \"aprovador\")]\nfn x() {}\n"),
        ("sub/config.rs", "let _ = std::env::var(\"X\");\n"),
        ("usage.rs", "#[cfg(feature = \"test-mode\")]\nfn x() {}\n"),
        // m2:V1.8: the pre-rename feature names are unknown features now.
        ("config.rs", "#[cfg(feature = \"modo-teste\")]\nfn x() {}\n"),
        (
            "core.rs",
            "#[cfg(feature = \"ganchos-teste\")]\n#[doc(hidden)]\npub fn with_ssh_programs(x) {}\n",
        ),
    ];
    for (rel, text) in cases {
        let r = std::panic::catch_unwind(|| check_v3_15(rel, text, &mut Vec::new()));
        assert!(r.is_err(), "aceitou {rel}: {text:?}");
    }
}

/// V3.16 (R16): free `prod` passes without asking; `prod` with approval offers the four
/// buttons, including "Always allow", and the environment is highlighted.
#[test]
fn v3_16_prod_has_the_same_levels_and_buttons() {
    let c = scene("prod");
    for name in ["p-livre", "p-aprov"] {
        let q =
            login(name, SECRETS[0]).replace(r#""environment":"dev""#, r#""environment":"prod""#);
        assert!(ok(&request(&c.d, Role::App, "save_entry", &q)));
    }
    assert!(ok(&set_level(&c.d, Role::App, "p-livre", "free", PIN)));
    let rx = subscribe(&c.d, 1);
    assert_eq!(
        use_entry(&c.d, &NoClient, "p-livre"),
        Ok(Authorization::WithoutAsking)
    );
    assert!(rx.try_recv().is_err());
    for decision in ["deny", "once", "one_hour", "always"] {
        let r = with_approval_window(&c, &rx, "p-aprov", |p| {
            assert_eq!(p.environment, Environment::Prod);
            assert_eq!(p.entries, ["p-aprov"]);
            let pin = (decision != "deny").then_some(PIN);
            assert!(
                ok(&answer(&c.d, Role::App, p.id, decision, pin)),
                "{decision}"
            );
        });
        if decision == "deny" {
            assert_eq!(r, Err(ResponseError::Denied));
        } else {
            assert_eq!(r, Ok(Authorization::Approved), "{decision}");
        }
        if decision == "one_hour" {
            // So that "always" still asks.
            assert!(ok(&request(&c.d, Role::Agent, "lock", "{}")));
            assert!(ok(&unlock(&c.d, PASSWORD)));
        }
    }
    assert_eq!(level(&c.d, "p-aprov"), Some(AccessLevel::Free));
}

/// D19: a database with a tunnel takes the most restrictive level of the two; the
/// window shows both and the most sensitive environment; the other one blocked denies
/// without asking.
#[test]
fn d19_combined_level() {
    let c = scene("combinado");
    let rx = subscribe(&c.d, 1);
    // b1 (prod, with approval) + s1 (stage, with approval).
    let r = with_approval_window(&c, &rx, "b1", |p| {
        assert_eq!(p.entries, ["b1", "s1"]);
        assert_eq!(p.environment, Environment::Prod);
        assert!(ok(&answer(&c.d, Role::App, p.id, "one_hour", Some(PIN))));
    });
    assert_eq!(r, Ok(Authorization::Approved));
    // The grant holds for both.
    assert_eq!(
        use_entry(&c.d, &NoClient, "b1"),
        Ok(Authorization::WithoutAsking)
    );
    assert!(ok(&request(&c.d, Role::Agent, "lock", "{}")));
    assert!(ok(&unlock(&c.d, PASSWORD)));
    // b1 free, s1 with approval: asks.
    assert!(ok(&set_level(&c.d, Role::App, "b1", "free", PIN)));
    let r = with_approval_window(&c, &rx, "b1", |p| {
        assert_eq!(p.entries, ["b1", "s1"]);
        assert!(ok(&answer(&c.d, Role::App, p.id, "deny", None)));
    });
    assert_eq!(r, Err(ResponseError::Denied));
    // Both free: passes.
    assert!(ok(&set_level(&c.d, Role::App, "s1", "free", PIN)));
    assert_eq!(
        use_entry(&c.d, &NoClient, "b1"),
        Ok(Authorization::WithoutAsking)
    );
    // The other one blocked: denies without opening a window.
    assert!(ok(&set_level(&c.d, Role::App, "s1", "blocked", PIN)));
    assert_eq!(use_entry(&c.d, &NoClient, "b1"), Err(ResponseError::Denied));
    assert!(rx.try_recv().is_err());
    // The blocked one itself is still the same as a missing one (V3.17).
    assert_eq!(
        use_entry(&c.d, &NoClient, "s1"),
        use_entry(&c.d, &NoClient, "nao-existe")
    );
    // "Always" on the combined use leaves free both that asked for approval.
    assert!(ok(&set_level(&c.d, Role::App, "s1", "approval", PIN)));
    assert!(ok(&set_level(&c.d, Role::App, "b1", "approval", PIN)));
    let r = with_approval_window(&c, &rx, "b1", |p| {
        assert!(ok(&answer(&c.d, Role::App, p.id, "always", Some(PIN))));
    });
    assert_eq!(r, Ok(Authorization::Approved));
    assert_eq!(level(&c.d, "b1"), Some(AccessLevel::Free));
    assert_eq!(level(&c.d, "s1"), Some(AccessLevel::Free));
}

/// D21: with no way to open the app, the request with approval is denied at once; the
/// app that opens and exits without receiving the request also gives "approval
/// unavailable".
#[test]
fn d21_approval_unavailable() {
    let c = scene_with("indisponivel", Err(OpenAppError::Unavailable));
    assert_eq!(
        use_entry(&c.d, &NoClient, "t-aprov"),
        Err(ResponseError::ApprovalUnavailable)
    );
    assert_eq!(c.opener.opens.load(Ordering::SeqCst), 1);
    // Free does not need the app.
    assert_eq!(
        use_entry(&c.d, &NoClient, "t-livre"),
        Ok(Authorization::WithoutAsking)
    );
    // The daemon without an engine (plain constructor) too.
    let dir = Dir::new("sem-motor");
    let (d, _) = test_vault(&dir);
    assert_eq!(
        use_entry(&d, &NoClient, "t-aprov"),
        Err(ResponseError::ApprovalUnavailable)
    );

    let c = scene("app-saiu");
    let r = std::thread::scope(|s| {
        let t = s.spawn(|| use_entry(&c.d, &NoClient, "t-aprov"));
        while c.opener.opens.load(Ordering::SeqCst) == 0 {
            std::thread::sleep(Duration::from_millis(5));
        }
        std::thread::sleep(Duration::from_millis(50));
        c.opener.exit();
        t.join().unwrap()
    });
    assert_eq!(r, Err(ResponseError::ApprovalUnavailable));
}

/// The app opened by the engine receives on subscribe the request that was already waiting.
#[test]
fn opened_app_receives_the_request_on_subscribe() {
    let c = scene("abre-e-assina");
    let r = std::thread::scope(|s| {
        let t = s.spawn(|| use_entry(&c.d, &NoClient, "t-aprov"));
        while c.opener.opens.load(Ordering::SeqCst) == 0 {
            std::thread::sleep(Duration::from_millis(5));
        }
        let rx = subscribe(&c.d, 7);
        let p = approval_request(&rx);
        assert!(ok(&answer(&c.d, Role::App, p.id, "once", Some(PIN))));
        t.join().unwrap()
    });
    assert_eq!(r, Ok(Authorization::Approved));
}

/// D51: the command and the chain go to the app formatted without ambiguity, with
/// visible escapes (the agent cannot show a command other than the one run).
#[test]
fn d51_command_and_chain_with_visible_escape() {
    struct ForgedClient;
    impl Client for ForgedClient {
        fn chain(&self) -> Vec<String> {
            vec!["claude".into(), "ba\u{202E}hs".into(), "ward\x1b[2K".into()]
        }
        fn exited(&self) -> bool {
            false
        }
    }
    let c = scene("d51");
    let rx = subscribe(&c.d, 1);
    let r = std::thread::scope(|s| {
        let t = s.spawn(|| {
            let q = run_json(
                "t-aprov",
                &[b"rm", "-rf /\x1b[2K\rls \u{202E}".as_bytes(), b"\xff"],
            );
            let p = req(Role::Agent, "run", &q).unwrap();
            c.d.authorize_use(&ForgedClient, &p)
        });
        let _g = ExpireOnPanic(&c.clock);
        let p = approval_request(&rx);
        assert_eq!(p.operation, Operation::Run);
        assert_eq!(p.command, r#""rm" "-rf /\x1b[2K\rls \u{202E}" "\xff""#);
        assert_eq!(p.chain, ["claude", r"ba\u{202E}hs", r"ward\x1b[2K"]);
        assert!(ok(&answer(&c.d, Role::App, p.id, "deny", None)));
        t.join().unwrap()
    });
    assert_eq!(r, Err(ResponseError::Denied));
}

/// Phase 26a: the run operation runs in the CLI directory, and the window shows that
/// exact directory, quoted and with visible escapes (D51): the agent that runs
/// `./deploy.sh` in a forged folder does not hide where the command runs. `ssh` (the
/// passwd home folder, fixed) goes without a directory.
#[test]
fn f26a_working_directory_goes_to_the_window_with_visible_escape() {
    let c = scene("f26a");
    let rx = subscribe(&c.d, 1);
    let r = std::thread::scope(|s| {
        let t = s.spawn(|| {
            let q = run_json_in(
                "t-aprov",
                &[b"./deploy.sh"],
                "/tmp/atacante/\u{202E}hs.yolped\x1b[2K /home/dev".as_bytes(),
            );
            let p = req(Role::Agent, "run", &q).unwrap();
            c.d.authorize_use(&NoClient, &p)
        });
        let _g = ExpireOnPanic(&c.clock);
        let p = approval_request(&rx);
        assert_eq!(p.command, r#""./deploy.sh""#);
        assert_eq!(
            p.directory,
            r#""/tmp/atacante/\u{202E}hs.yolped\x1b[2K /home/dev""#
        );
        assert!(ok(&answer(&c.d, Role::App, p.id, "deny", None)));
        t.join().unwrap()
    });
    assert_eq!(r, Err(ResponseError::Denied));
}

/// The client (CLI) that drops while waiting cancels the request.
#[test]
fn client_that_drops_cancels() {
    struct Dropped(std::sync::atomic::AtomicBool);
    impl Client for Dropped {
        fn chain(&self) -> Vec<String> {
            Vec::new()
        }
        fn exited(&self) -> bool {
            self.0.load(Ordering::SeqCst)
        }
    }
    let c = scene("caiu");
    let rx = subscribe(&c.d, 1);
    let cli = Dropped(false.into());
    let r = std::thread::scope(|s| {
        let t = s.spawn(|| use_entry(&c.d, &cli, "t-aprov"));
        let p = approval_request(&rx);
        cli.0.store(true, Ordering::SeqCst);
        let r = t.join().unwrap();
        assert!(c.d.engine().pending(p.id).is_none());
        r
    });
    assert_eq!(r, Err(ResponseError::Denied));
}

/// V3.18 (D16): locked vault: the app receives the unlock request and the use waits;
/// unlocked at 60 s it goes on (and then asks for the approval, which counts
/// separately); after 120 s, "locked: timed out". Without an app, "locked" at once.
#[test]
fn v3_18_waits_for_unlock_up_to_2_min() {
    let c = scene("destrave");
    let rx = subscribe(&c.d, 1);
    assert!(ok(&request(&c.d, Role::Agent, "lock", "{}")));
    let r = std::thread::scope(|s| {
        let t = s.spawn(|| use_entry(&c.d, &NoClient, "t-livre"));
        assert!(matches!(
            rx.recv_timeout(DEADLINE).unwrap(),
            Received::Unlock(_)
        ));
        c.clock.set(Duration::from_secs(60));
        std::thread::sleep(Duration::from_millis(100));
        assert!(!t.is_finished());
        assert!(ok(&unlock(&c.d, PASSWORD)));
        t.join().unwrap()
    });
    assert_eq!(r, Ok(Authorization::WithoutAsking));

    // Unlock and then approval: the two deadlines separately.
    assert!(ok(&request(&c.d, Role::Agent, "lock", "{}")));
    let r = std::thread::scope(|s| {
        let t = s.spawn(|| use_entry(&c.d, &NoClient, "t-aprov"));
        assert!(matches!(
            rx.recv_timeout(DEADLINE).unwrap(),
            Received::Unlock(_)
        ));
        c.clock.advance(Duration::from_secs(110));
        assert!(ok(&unlock(&c.d, PASSWORD)));
        let p = approval_request(&rx);
        c.clock.advance(Duration::from_secs(100));
        std::thread::sleep(Duration::from_millis(100));
        assert!(c.d.engine().pending(p.id).is_some());
        assert!(ok(&answer(&c.d, Role::App, p.id, "once", Some(PIN))));
        t.join().unwrap()
    });
    assert_eq!(r, Ok(Authorization::Approved));

    assert!(ok(&request(&c.d, Role::Agent, "lock", "{}")));
    let r = std::thread::scope(|s| {
        let t = s.spawn(|| use_entry(&c.d, &NoClient, "t-livre"));
        assert!(matches!(
            rx.recv_timeout(DEADLINE).unwrap(),
            Received::Unlock(_)
        ));
        c.clock.advance(Duration::from_secs(120));
        t.join().unwrap()
    });
    assert_eq!(r, Err(ResponseError::LockedTimedOut));
    // Through the protocol, the same answer.
    let s2 = scene_with("destrave-sem-app", Err(OpenAppError::Unavailable));
    assert!(ok(&request(&s2.d, Role::Agent, "lock", "{}")));
    assert_eq!(
        error(&request(
            &s2.d,
            Role::Agent,
            "run",
            &run_json("t-livre", &[b"true"])
        )),
        Some(ResponseError::Locked)
    );
}

fn unlock_request(rx: &Receiver<Received>) -> UnlockRequest {
    match rx
        .recv_timeout(DEADLINE)
        .expect("o app não recebeu o pedido de destrave")
    {
        Received::Unlock(p) => p,
        other => panic!("{other:?}"),
    }
}

/// Phase 39a (V5.8, D51): the unlock request carries the entry the agent asked for, the
/// operation, the exact command with visible escapes and the seconds left by the engine
/// clock. Attack: name and argv with `ESC`, bidi (U+202E), zero width and Unicode tag,
/// and quotes that pretend to close the argument. Nothing invisible reaches the app, the
/// boundary of each argument stays clear, and the app that subscribes later receives
/// the newest one with the right deadline.
#[test]
fn attack_phase_39a_unlock_with_entry_and_command_neutralized() {
    let c = scene("destrave-39a");
    let rx = subscribe(&c.d, 1);
    assert!(ok(&request(&c.d, Role::Agent, "lock", "{}")));
    let invisible = |t: &str| t.chars().any(ward_core::display::invisible);
    let name_json = "t-livre\\u202e\\u001b[2K\\u200b\\udb40\\udc41";
    let argv: [&[u8]; 3] = [
        b"psql",
        b"-c",
        b"select 1\" \"; rm -rf /\x1b[2K\xe2\x80\xae",
    ];
    let r = std::thread::scope(|s| {
        let t = s.spawn(|| {
            let p = req(Role::Agent, "run", &run_json(name_json, &argv)).unwrap();
            c.d.authorize_use(&NoClient, &p)
        });
        let p = unlock_request(&rx);
        assert_eq!(p.operation, Operation::Run);
        assert_eq!(p.remaining, 120);
        assert!(!invisible(&p.entry), "{:?}", p.entry);
        assert!(!invisible(&p.command), "{:?}", p.command);
        assert_eq!(p.entry, r"t-livre\u{202E}\x1b[2K\u{200B}\u{E0041}");
        assert_eq!(
            p.command,
            r#""psql" "-c" "select 1\" \"; rm -rf /\x1b[2K\u{202E}""#
        );
        // The app that subscribes later (window reopened) receives the same request, with
        // what is left by the engine clock.
        c.clock.set(Duration::from_millis(50_400));
        let rx2 = subscribe(&c.d, 2);
        let q = unlock_request(&rx2);
        assert_eq!(q.remaining, 70);
        assert_eq!(
            (q.entry.as_str(), q.command.as_str()),
            (p.entry.as_str(), p.command.as_str())
        );
        // A second use waiting: the app receives the newest (ssh, no argv: empty command).
        let t2 = s.spawn(|| {
            let p = req(
                Role::Agent,
                "ssh",
                r#"{"entry":"t-aprov","stdin_kind":"terminal"}"#,
            )
            .unwrap();
            c.d.authorize_use(&NoClient, &p)
        });
        let n = unlock_request(&rx2);
        assert_eq!(
            (n.entry.as_str(), n.operation, n.remaining),
            ("t-aprov", Operation::Ssh, 120)
        );
        // Subscribing again delivers only the newest.
        let rx3 = subscribe(&c.d, 3);
        assert_eq!(unlock_request(&rx3).entry, "t-aprov");
        assert!(rx3.recv_timeout(Duration::from_millis(200)).is_err());
        c.clock.advance(Duration::from_secs(120));
        (t.join().unwrap(), t2.join().unwrap())
    });
    assert_eq!(r.0, Err(ResponseError::LockedTimedOut));
    assert_eq!(r.1, Err(ResponseError::LockedTimedOut));
    // With nobody waiting, whoever subscribes receives no unlock request.
    let rx4 = subscribe(&c.d, 4);
    assert!(rx4.recv_timeout(Duration::from_millis(200)).is_err());
}

/// Phase 39a: denial attack. With the vault locked, an agent sends a use with an empty
/// entry: the unlock notice would go out of format and the delivery failure would bring
/// down the app connection (and deny the approval request it shows). The protocol
/// parsing already refuses ("invalid request"); inside (second layer), the daemon
/// answers "missing entry" at once, without a notice, and the app stays subscribed.
#[test]
fn attack_phase_39a_empty_entry_does_not_bring_down_the_app() {
    let c = scene("destrave-vazia");
    let rx = subscribe(&c.d, 1);
    assert!(ok(&request(&c.d, Role::Agent, "lock", "{}")));
    assert_eq!(
        error(&request(
            &c.d,
            Role::Agent,
            "run",
            &run_json("", &[b"true"])
        )),
        Some(ResponseError::InvalidRequest)
    );
    for p in [
        ward_core::protocol::Request::Login {
            entry: String::new(),
        },
        ward_core::protocol::Request::Playwright {
            entry: String::new(),
        },
    ] {
        assert_eq!(
            c.d.authorize_use(&NoClient, &p),
            Err(ResponseError::EntryNotFound)
        );
    }
    assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
    assert!(c.d.engine().has_subscriber());
    // The app keeps receiving the next request, with a name.
    let r = std::thread::scope(|s| {
        let t = s.spawn(|| use_entry(&c.d, &NoClient, "t-livre"));
        assert_eq!(unlock_request(&rx).entry, "t-livre");
        assert!(ok(&unlock(&c.d, PASSWORD)));
        t.join().unwrap()
    });
    assert_eq!(r, Ok(Authorization::WithoutAsking));
}

/// D57: opening the app from any role, without fields; opens through the launcher;
/// without a session, "approval unavailable". A field in the request (path,
/// environment) refuses the request.
#[test]
fn d57_open_app() {
    let c = scene("abrir-app");
    for role in Role::ALL {
        assert!(ok(&request(&c.d, role, "open_app", "{}")), "{role:?}");
    }
    assert_eq!(c.opener.opens.load(Ordering::SeqCst), 3);
    for data in [
        r#"{"binario":"/tmp/x"}"#,
        r#"{"environment":[{"name":"V0FZTEFORF9ESVNQTEFZ","value":"L3RtcC9wcm94eQ=="}]}"#,
        r#"{"argv":[]}"#,
    ] {
        assert_eq!(
            error(&request(&c.d, Role::Agent, "open_app", data)),
            Some(ResponseError::InvalidRequest)
        );
    }
    assert_eq!(c.opener.opens.load(Ordering::SeqCst), 3);
    let s = scene_with("abrir-app-sem", Err(OpenAppError::Unavailable));
    assert_eq!(
        error(&request(&s.d, Role::Agent, "open_app", "{}")),
        Some(ResponseError::ApprovalUnavailable)
    );
    let s = scene_with("abrir-app-ja", Err(OpenAppError::AlreadyOpen));
    assert!(ok(&request(&s.d, Role::Agent, "open_app", "{}")));
    // Already open (window closed, in the tray): the subscribed app receives
    // `show_window`, without data; the menu brings the window back (port of 0.1.5).
    let rx = subscribe(&s.d, 7);
    assert!(ok(&request(&s.d, Role::Agent, "open_app", "{}")));
    assert!(matches!(
        rx.recv_timeout(DEADLINE),
        Ok(Received::ShowWindow)
    ));
    // Opened now by the launcher: nothing goes to the subscriber.
    let c = scene("abrir-app-novo");
    let rx = subscribe(&c.d, 8);
    assert!(ok(&request(&c.d, Role::Agent, "open_app", "{}")));
    assert!(rx.try_recv().is_err());
}

/// Phase 66a of mission 2 (port of 0.1.5), attack: whoever asks `open_app` (agent,
/// extension or app) only gets a bare `ok`; `show_window` goes only to the subscriber
/// (the app role connection), once per request, with nothing in it; no field of the
/// request points it elsewhere (a pid, a window, a binary: refused, nothing delivered);
/// without a subscriber nothing is sent; and a subscriber whose delivery fails leaves.
#[test]
fn attack_show_window_only_to_the_subscribed_app() {
    let s = scene_with("mostrar-janela", Err(OpenAppError::AlreadyOpen));
    // No subscriber: ok, nothing to deliver, nothing kept for later.
    assert!(ok(&request(&s.d, Role::Agent, "open_app", "{}")));
    let rx = subscribe(&s.d, 11);
    assert!(rx.try_recv().is_err(), "show_window guardado para depois");
    for role in Role::ALL {
        let r = request(&s.d, role, "open_app", "{}");
        assert!(ok(&r), "{role:?}");
        assert!(!matches!(r, Response::ShowWindow), "{role:?}");
        assert!(matches!(
            rx.recv_timeout(DEADLINE),
            Ok(Received::ShowWindow)
        ));
        assert!(rx.try_recv().is_err(), "{role:?}: mais de um aviso");
    }
    for role in [Role::Agent, Role::Extension] {
        for data in [
            r#"{"pid":4242}"#,
            r#"{"window":"main"}"#,
            r#"{"id":1}"#,
            r#"{"binario":"/tmp/x"}"#,
            r#"{"entry":"acme-pg-prod"}"#,
        ] {
            assert_eq!(
                error(&request(&s.d, role, "open_app", data)),
                Some(ResponseError::InvalidRequest),
                "{role:?} {data}"
            );
        }
    }
    assert!(rx.try_recv().is_err(), "pedido recusado chegou ao app");
    // A new subscriber takes the place: only it receives.
    let rx2 = subscribe(&s.d, 12);
    assert!(ok(&request(&s.d, Role::Extension, "open_app", "{}")));
    assert!(matches!(
        rx2.recv_timeout(DEADLINE),
        Ok(Received::ShowWindow)
    ));
    assert!(rx.try_recv().is_err(), "o assinante antigo recebeu");
    // Delivery fails (the app connection dropped): the subscriber leaves.
    drop(rx2);
    assert!(ok(&request(&s.d, Role::Agent, "open_app", "{}")));
    assert!(!s.d.engine().has_subscriber());
}

/// V3.36 (D63, the classifier part): with the app group and the right parent, only the
/// pid the launcher opened is app; another child of the daemon (the `ward run` of Phase
/// 26) stays agent.
#[test]
fn v3_36_only_the_launcher_pid_is_app() {
    let launched = 4242;
    let e = |p: u32| p == launched;
    assert_eq!(role_from_class(PeerClass::App, launched, e), Role::App);
    assert_eq!(role_from_class(PeerClass::App, 4243, e), Role::Agent);
    assert_eq!(
        role_from_class(PeerClass::App, launched, |_| false),
        Role::Agent
    );
    assert_eq!(
        role_from_class(
            PeerClass::Common {
                reason: Refusal::CommonGroup
            },
            launched,
            e
        ),
        Role::Agent
    );
    assert_eq!(
        role_from_class(PeerClass::ExtensionHost, 1, e),
        Role::Extension
    );
}

/// V3.38 (D63): the status carries the role of the connection.
#[test]
fn v3_38_status_carries_the_role() {
    let c = scene("estado-papel");
    for role in Role::ALL {
        match request(&c.d, role, "status", "{}") {
            Response::Status { role: p, .. } => assert_eq!(p, role),
            r => panic!("{r:?}"),
        }
    }
}

// --- Phase 54b (D68): standard input the window does not show ----------------------

/// The run operation with standard input of kind `kind` (`argv` = a lone `python3`).
fn use_with_stdin(d: &Daemon, name: &str, kind: &str) -> Result<Authorization, ResponseError> {
    let q = run_json(name, &[b"python3"]).replace(
        r#""stdin_kind":"null""#,
        &format!(r#""stdin_kind":"{kind}""#),
    );
    let p = req(Role::Agent, "run", &q).unwrap();
    d.authorize_use(&NoClient, &p)
}

/// A use with input `kind` that asks: the result after `respond`.
fn with_window_and_stdin(
    c: &Scene,
    rx: &Receiver<Received>,
    name: &str,
    kind: &str,
    answer: impl FnOnce(&ApprovalRequest),
) -> Result<Authorization, ResponseError> {
    std::thread::scope(|s| {
        let t = s.spawn(|| use_with_stdin(&c.d, name, kind));
        let _g = ExpireOnPanic(&c.clock);
        let p = approval_request(rx);
        answer(&p);
        t.join().unwrap()
    })
}

fn answer_confirmed(d: &Daemon, id: u64, decision: &str, pin: &str) -> Response {
    request(
        d,
        Role::App,
        "answer_approval",
        &format!(
            r#"{{"request":{id},"decision":"{decision}","pin":"{}","confirmed_entry":true}}"#,
            b64(pin.as_bytes())
        ),
    )
}

/// Final audit attack (D68): `printf <script> | ward run <entry> -- python3`. The window
/// receives the notice (hidden input flag); the three "Allow" with the right PIN and
/// without the box checked are refused, before the PIN (six attempts do not lock the
/// vault: nothing counts as a PIN error), and the request stays pending; only with the
/// box does it pass. Pipe, file and other count the same.
#[test]
fn f54b_attack_pipe_on_approval_without_the_box_does_not_pass() {
    let c = scene("f54b-pipe");
    let rx = subscribe(&c.d, 1);
    for kind in ["pipe", "file", "other"] {
        let r = with_window_and_stdin(&c, &rx, "t-aprov", kind, |p| {
            assert!(p.hidden_entry, "{kind}: a janela não recebeu o aviso");
            assert_eq!(p.command, "\"python3\"");
            for _ in 0..2 {
                for decision in ["once", "one_hour", "always"] {
                    assert_eq!(
                        error(&answer(&c.d, Role::App, p.id, decision, Some(PIN))),
                        Some(ResponseError::InvalidRequest),
                        "{kind} {decision}"
                    );
                }
            }
            assert!(c.d.engine().pending(p.id).is_some(), "{kind}");
            // "Confirm" (no PIN) does not decide such a request either.
            assert!(error(&answer(&c.d, Role::App, p.id, "confirm", None)).is_some());
            assert!(ok(&answer_confirmed(&c.d, p.id, "once", PIN)));
        });
        assert_eq!(r, Ok(Authorization::Approved), "{kind}");
    }
    // None of the "Allow" without the box counted as a wrong PIN: the vault stays open.
    assert!(!c.d.is_locked());
    // Deny still needs neither PIN nor box.
    let r = with_window_and_stdin(&c, &rx, "t-aprov", "pipe", |p| {
        assert!(ok(&answer(&c.d, Role::App, p.id, "deny", None)));
    });
    assert_eq!(r, Err(ResponseError::Denied));
    // The box does not override the PIN: wrong is still wrong.
    let r = with_window_and_stdin(&c, &rx, "t-aprov", "pipe", |p| {
        assert_eq!(
            error(&answer_confirmed(&c.d, p.id, "once", "000000")),
            Some(ResponseError::WrongPin)
        );
        assert!(ok(&answer(&c.d, Role::App, p.id, "deny", None)));
    });
    assert_eq!(r, Err(ResponseError::Denied));
}

/// D68: with a terminal or `/dev/null` (Claude's case), nothing changes: without a
/// notice, "Allow" without the box passes as before, and the box checked outside the
/// notice is refused (the window does not decide on its own).
#[test]
fn f54b_terminal_and_null_pass_as_before() {
    let c = scene("f54b-tty");
    let rx = subscribe(&c.d, 1);
    for kind in ["terminal", "null"] {
        let r = with_window_and_stdin(&c, &rx, "t-aprov", kind, |p| {
            assert!(!p.hidden_entry, "{kind}");
            assert_eq!(
                error(&answer_confirmed(&c.d, p.id, "once", PIN)),
                Some(ResponseError::InvalidRequest),
                "{kind}"
            );
            assert!(ok(&answer(&c.d, Role::App, p.id, "once", Some(PIN))));
        });
        assert_eq!(r, Ok(Authorization::Approved), "{kind}");
    }
}

/// D68: the 1-hour grant does not cover hidden standard input (the window comes back,
/// with the notice and the PIN); the free level does not change (nor ask); on the
/// combined use (D19) the notice goes along.
#[test]
fn f54b_1_hour_grant_does_not_cover_pipe() {
    let c = scene("f54b-hora");
    let rx = subscribe(&c.d, 1);
    let r = with_window_and_stdin(&c, &rx, "t-aprov", "terminal", |p| {
        assert!(ok(&answer(&c.d, Role::App, p.id, "one_hour", Some(PIN))));
    });
    assert_eq!(r, Ok(Authorization::Approved));
    assert_eq!(
        use_with_stdin(&c.d, "t-aprov", "terminal"),
        Ok(Authorization::WithoutAsking)
    );
    assert_eq!(
        use_with_stdin(&c.d, "t-aprov", "null"),
        Ok(Authorization::WithoutAsking)
    );
    let r = with_window_and_stdin(&c, &rx, "t-aprov", "pipe", |p| {
        assert!(p.hidden_entry);
        assert_eq!(
            error(&answer(&c.d, Role::App, p.id, "once", Some(PIN))),
            Some(ResponseError::InvalidRequest)
        );
        assert!(ok(&answer(&c.d, Role::App, p.id, "deny", None)));
    });
    assert_eq!(r, Err(ResponseError::Denied));
    // Free: the agent can already read the secret (scope › level), so nothing changes.
    assert_eq!(
        use_with_stdin(&c.d, "t-livre", "pipe"),
        Ok(Authorization::WithoutAsking)
    );
    assert!(rx.try_recv().is_err());
    // Combined: b1 + s1, both with approval.
    let r = with_window_and_stdin(&c, &rx, "b1", "file", |p| {
        assert_eq!(p.entries, ["b1", "s1"]);
        assert!(p.hidden_entry);
        assert!(ok(&answer_confirmed(&c.d, p.id, "one_hour", PIN)));
    });
    assert_eq!(r, Ok(Authorization::Approved));
    // The grant given with the box checked does not cover the next pipe either.
    let r = with_window_and_stdin(&c, &rx, "b1", "pipe", |p| {
        assert!(p.hidden_entry);
        assert!(ok(&answer(&c.d, Role::App, p.id, "deny", None)));
    });
    assert_eq!(r, Err(ResponseError::Denied));
    assert_eq!(
        use_with_stdin(&c.d, "b1", "terminal"),
        Ok(Authorization::WithoutAsking)
    );
}

/// D68: `ssh` carries the same notice.
#[test]
fn f54b_ssh_with_pipe_warns() {
    let c = scene("f54b-ssh");
    let rx = subscribe(&c.d, 1);
    let r = std::thread::scope(|s| {
        let t = s.spawn(|| {
            let p = req(Role::Agent, "ssh", r#"{"entry":"s1","stdin_kind":"pipe"}"#).unwrap();
            c.d.authorize_use(&NoClient, &p)
        });
        let _g = ExpireOnPanic(&c.clock);
        let p = approval_request(&rx);
        assert_eq!(p.operation, Operation::Ssh);
        assert!(p.hidden_entry);
        assert_eq!(
            error(&answer(&c.d, Role::App, p.id, "once", Some(PIN))),
            Some(ResponseError::InvalidRequest)
        );
        assert!(ok(&answer_confirmed(&c.d, p.id, "once", PIN)));
        t.join().unwrap()
    });
    assert_eq!(r, Ok(Authorization::Approved));
}
