//! Approval window (Phase 36; S9, S10, D51, D65 a).
//!
//! - [`Approvals`]: the approval requests the daemon sent that still wait for an answer,
//!   in arrival order, each one with the clock of [`APPROVAL_TIMEOUT`] counted from the
//!   arrival (the notice does not carry the deadline; the daemon counts its own, S10, and
//!   denies by itself). At most [`MAX_PENDING`]: the excess is dropped and expires in the
//!   daemon (fail closed).
//! - [`watch`]: the thread that takes the approval requests out of the connection's
//!   notice queue (D65 a) and tells whoever opens the window. The unlock requests stay in
//!   the queue for Phase 31.
//! - [`display`]: what the window receives. The text comes from the agent (command,
//!   process chain) and is **untrusted**: it goes through `ward_core::display::neutralize`
//!   (D51) here, on the Rust side, before going to the webview; the screen puts it as
//!   text (never as HTML). Control, bidi, zero width and tags become a visible escape;
//!   nothing is removed nor reordered. The arrow `→` that separates the chain on the
//!   screen is reserved: one that comes inside a process name becomes `\u{2192}`, so one
//!   process cannot pass for two.
//! - [`ApprovalResult`]: what the answer turned into. The PIN never comes back in
//!   anything from here.
//!
//! Only the approval window answers (its own capability in Tauri; the command checks the
//! window label again). The agent role never answers: the daemon only accepts the answer
//! approval request from the app role (V3.15, V5.20).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::Serialize;
use ward_core::display::neutralize;
use ward_core::protocol::{Decision, PedidoDeAprovacao};

use crate::AppError;
use crate::daemon::{MAX_NOTICES, NoticeQueue};

/// How long the request waits for an answer (S10; the same deadline as `wardd`).
pub const APPROVAL_TIMEOUT: Duration = Duration::from_secs(120);

/// From how many remaining seconds on the window turns into the expiring state of the
/// design.
pub const URGENT_SECONDS: u64 = 20;

/// Requests kept at the same time (the same limit as the notice queue, D65 a).
pub const MAX_PENDING: usize = MAX_NOTICES;

/// Separator of the chain on the screen; inside a process name it becomes an escape.
pub const CHAIN_SEPARATOR: char = '\u{2192}';

/// The decision by the name that comes from the interface (untrusted).
pub fn decision_from_name(name: &str) -> Result<Decision, AppError> {
    match name {
        "negar" => Ok(Decision::Deny),
        "uma_vez" => Ok(Decision::AllowOnce),
        "uma_hora" => Ok(Decision::AllowOneHour),
        "sempre" => Ok(Decision::AllowAlways),
        // Phase 44c: only the confirmation of the `http` origin (no PIN).
        "confirmar" => Ok(Decision::Confirm),
        _ => Err(AppError::InvalidRequest),
    }
}

/// One entry of the request, for the screen. `kind` is the one of the detail (only the
/// id); without it the screen shows the generic icon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DisplayedEntry {
    #[serde(rename = "nome")]
    pub name: String,
    #[serde(rename = "tipo")]
    pub kind: Option<&'static str>,
}

/// The request as the window shows it. Every text that came from the agent is already
/// neutralized (D51). No secret: the protocol request has none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DisplayedRequest {
    pub id: u64,
    /// The entry and, in the combined use (D19), the one of the jump or of the tunnel.
    #[serde(rename = "entradas")]
    pub entries: Vec<DisplayedEntry>,
    #[serde(rename = "ambiente")]
    pub environment: &'static str,
    #[serde(rename = "operacao")]
    pub operation: &'static str,
    /// The exact command, each argument in quotes (D51).
    #[serde(rename = "comando")]
    pub command: String,
    /// Where the command runs (run operation, Phase 26a), in quotes (D51); empty in the
    /// other operations.
    #[serde(rename = "diretorio")]
    pub directory: String,
    /// Who asked, from the farthest to the closest.
    #[serde(rename = "cadeia")]
    pub chain: Vec<String>,
    /// Phase 44c (D9): the `http` origin where username and password go, with a visible
    /// escape (D51); empty when there is none.
    #[serde(rename = "origem")]
    pub origin: String,
    /// Phase 44c: only the confirmation of the origin ("Deny" and "Confirm", no PIN).
    #[serde(rename = "confirmacao")]
    pub confirmation: bool,
    /// Phase 54b (D68): the command receives on the standard input (pipe, file…) data the
    /// window does not show: highlighted notice and the "Allow" buttons only with the
    /// box checked.
    #[serde(rename = "entrada_oculta")]
    pub hidden_entry: bool,
    /// Seconds until it expires (0 when `status` is blocked).
    #[serde(rename = "restante")]
    pub remaining: u64,
    /// Pending or blocked (the 5th wrong PIN locked the vault and the daemon
    /// denied; the window only shows what happened until the user closes it).
    #[serde(rename = "estado")]
    pub status: &'static str,
}

/// What the window's answer turned into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ApprovalResult {
    /// Answered, wrong PIN (the request stays pending), blocked (the 5th error locked
    /// the vault and the request was denied) or expired (the daemon no
    /// longer has it).
    #[serde(rename = "resultado")]
    pub result: &'static str,
    /// There is still a request waiting (the window stays open).
    #[serde(rename = "ha_outro")]
    pub has_other: bool,
}

struct Item {
    request: PedidoDeAprovacao,
    arrival: Instant,
    blocked: bool,
    /// Types of the entries, fetched once from the detail.
    kinds: Option<Vec<Option<&'static str>>>,
}

/// The current request, already taken out of the lock.
pub(crate) struct Current {
    pub request: PedidoDeAprovacao,
    pub remaining: u64,
    pub blocked: bool,
    pub kinds: Option<Vec<Option<&'static str>>>,
}

/// The requests that wait for the window.
#[derive(Default)]
pub struct Approvals {
    items: Mutex<VecDeque<Item>>,
}

impl Approvals {
    fn lock(&self) -> MutexGuard<'_, VecDeque<Item>> {
        self.items.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// An approval request arrived now.
    pub fn arrived(&self, p: PedidoDeAprovacao) {
        self.arrived_at(p, Instant::now());
    }

    /// Only in the tests and in the test bridge: a request that arrived `elapsed` ago
    /// (for the expiring state without waiting 2 min).
    #[cfg(any(test, feature = "test-mode"))]
    pub fn arrived_ago(&self, p: PedidoDeAprovacao, elapsed: Duration) {
        let now = Instant::now();
        self.arrived_at(p, now.checked_sub(elapsed).unwrap_or(now));
    }

    fn arrived_at(&self, p: PedidoDeAprovacao, arrival: Instant) {
        let mut f = self.lock();
        if f.iter().any(|i| i.request.id == p.id) {
            return;
        }
        if f.len() >= MAX_PENDING {
            drop(f);
            eprintln!("ward-app: pedido de aprovação descartado (muitos esperando)");
            return;
        }
        f.push_back(Item {
            request: p,
            arrival,
            blocked: false,
            kinds: None,
        });
    }

    /// Takes out the pending ones past the deadline (the daemon already denied them).
    fn drop_expired(f: &mut VecDeque<Item>, now: Instant) {
        f.retain(|i| i.blocked || remaining_secs(i.arrival, now) > 0);
    }

    /// The current request (the oldest one still within the deadline, or the blocked one
    /// the window shows).
    pub(crate) fn current(&self) -> Option<Current> {
        self.current_at(Instant::now())
    }

    fn current_at(&self, now: Instant) -> Option<Current> {
        let mut f = self.lock();
        Self::drop_expired(&mut f, now);
        f.front().map(|i| Current {
            request: i.request.clone(),
            remaining: if i.blocked {
                0
            } else {
                remaining_secs(i.arrival, now)
            },
            blocked: i.blocked,
            kinds: i.kinds.clone(),
        })
    }

    /// Keeps the types of the entries of request `id`.
    pub(crate) fn store_kinds(&self, id: u64, kinds: Vec<Option<&'static str>>) {
        if let Some(i) = self.lock().iter_mut().find(|i| i.request.id == id) {
            i.kinds = Some(kinds);
        }
    }

    /// Is request `id` only the confirmation of the origin (Phase 44c)? `None` if it is
    /// not here.
    pub(crate) fn confirmation(&self, id: u64) -> Option<bool> {
        self.lock()
            .iter()
            .find(|i| i.request.id == id)
            .map(|i| i.request.confirmacao)
    }

    /// Does request `id` have the hidden standard input (Phase 54b)? `None` if it is not
    /// here.
    pub(crate) fn hidden_entry(&self, id: u64) -> Option<bool> {
        self.lock()
            .iter()
            .find(|i| i.request.id == id)
            .map(|i| i.request.entrada_oculta)
    }

    /// State of request `id` here: `Some(false)` pending, `Some(true)` blocked, `None`
    /// unknown or expired.
    pub(crate) fn state_of(&self, id: u64) -> Option<bool> {
        let mut f = self.lock();
        Self::drop_expired(&mut f, Instant::now());
        f.iter().find(|i| i.request.id == id).map(|i| i.blocked)
    }

    pub(crate) fn remove(&self, id: u64) {
        self.lock().retain(|i| i.request.id != id);
    }

    pub(crate) fn mark_blocked(&self, id: u64) {
        if let Some(i) = self.lock().iter_mut().find(|i| i.request.id == id) {
            i.blocked = true;
        }
    }

    /// Ids of the pending ones within the deadline (not the blocked ones).
    pub(crate) fn pending(&self) -> Vec<u64> {
        let mut f = self.lock();
        Self::drop_expired(&mut f, Instant::now());
        f.iter()
            .filter(|i| !i.blocked)
            .map(|i| i.request.id)
            .collect()
    }

    /// There is something for the window to show.
    pub fn has_request(&self) -> bool {
        let mut f = self.lock();
        Self::drop_expired(&mut f, Instant::now());
        !f.is_empty()
    }

    /// Everything out (the app is about to quit).
    pub fn clear(&self) {
        self.lock().clear();
    }
}

/// Seconds left, rounding up (with 0.4 s there is still a request).
fn remaining_secs(arrival: Instant, now: Instant) -> u64 {
    match (arrival + APPROVAL_TIMEOUT).checked_duration_since(now) {
        Some(f) if !f.is_zero() => f.as_secs() + u64::from(f.subsec_nanos() > 0),
        _ => 0,
    }
}

/// A process name of the chain: visible escape (D51) and the reserved arrow as an escape.
fn display_process(name: &str) -> String {
    neutralize(name).replace(CHAIN_SEPARATOR, "\\u{2192}")
}

/// The request as the window shows it (D51 on every text that came from the agent or
/// from the daemon).
pub(crate) fn display(a: &Current) -> DisplayedRequest {
    let p = &a.request;
    let entries = p
        .entradas
        .iter()
        .enumerate()
        .map(|(i, n)| DisplayedEntry {
            name: neutralize(n),
            kind: a.kinds.as_ref().and_then(|t| t.get(i).copied().flatten()),
        })
        .collect();
    DisplayedRequest {
        id: p.id,
        entries,
        environment: p.ambiente.name(),
        operation: p.operacao.name(),
        command: neutralize(&p.comando),
        directory: neutralize(&p.diretorio),
        chain: p.cadeia.iter().map(|c| display_process(c)).collect(),
        origin: neutralize(&p.origem),
        confirmation: p.confirmacao,
        hidden_entry: p.entrada_oculta,
        remaining: a.remaining,
        status: if a.blocked { "bloqueado" } else { "pendente" },
    }
}

/// Starts the thread that takes the approval requests out of the `queue` (D65 a): each
/// one goes to `approvals` and calls `on_arrival` (which opens the window or brings it to
/// the front).
pub fn watch(
    queue: Arc<NoticeQueue>,
    approvals: Arc<Approvals>,
    on_arrival: impl Fn() + Send + 'static,
) -> Result<(), AppError> {
    std::thread::Builder::new()
        .name("ward-app-aprovacao".into())
        .spawn(move || {
            loop {
                if let Some(p) = queue.wait_approval(Duration::from_secs(3600)) {
                    approvals.arrived(p);
                    on_arrival();
                }
            }
        })
        .map(|_| ())
        .map_err(|_| AppError::ConnectionLost)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::Notice;
    use ward_core::entry::Environment;
    use ward_core::protocol::Operation;

    pub(crate) fn request(id: u64, command: &str, chain: &[&str]) -> PedidoDeAprovacao {
        PedidoDeAprovacao {
            id,
            entradas: vec!["acme-pg-aprovacao".into()],
            ambiente: Environment::Stage,
            operacao: Operation::Run,
            comando: command.into(),
            diretorio: "\"/home/dev/projetos/acme\"".into(),
            cadeia: chain.iter().map(|c| (*c).to_owned()).collect(),
            origem: String::new(),
            confirmacao: false,
            entrada_oculta: false,
        }
    }

    #[test]
    fn decision_only_by_the_five_names() {
        assert_eq!(decision_from_name("negar"), Ok(Decision::Deny));
        assert_eq!(decision_from_name("uma_vez"), Ok(Decision::AllowOnce));
        assert_eq!(decision_from_name("uma_hora"), Ok(Decision::AllowOneHour));
        assert_eq!(decision_from_name("sempre"), Ok(Decision::AllowAlways));
        assert_eq!(decision_from_name("confirmar"), Ok(Decision::Confirm));
        for bad in ["", "Negar", "permitir", "uma-vez", "sempre ", "livre"] {
            assert_eq!(
                decision_from_name(bad),
                Err(AppError::InvalidRequest),
                "{bad}"
            );
        }
    }

    /// Phase 26a: the working directory goes to the window and also goes through D51 (the
    /// daemon already formats it, but the window does not trust it).
    #[test]
    fn attack_directory_with_bidi_becomes_visible_escape() {
        let mut p = request(1, "\"make\"", &["claude"]);
        p.diretorio = "\"/home/dev/\u{202E}emca\u{1b}[2J\"".into();
        let e = display(&Current {
            request: p,
            remaining: 90,
            blocked: false,
            kinds: None,
        });
        assert!(
            e.directory
                .chars()
                .all(|c| !ward_core::display::invisible(c))
        );
        assert_eq!(e.directory, "\"/home/dev/\\u{202E}emca\\x1b[2J\"");
    }

    /// V5.29 (D51): HTML, control, bidi, zero width, tags and a fake `\n` show up as a
    /// visible escape; nothing disappears nor changes order; the chain arrow cannot be
    /// forged.
    #[test]
    fn attack_agent_text_becomes_visible_escape() {
        let command = "\"sh\" \"-c\" \"echo <img src=x onerror=alert(1)>\u{202E}fdp.exe\u{2066}x\u{200B}y\u{1b}[2J\nward run acme-pg-dev -- ls\u{E0041}\"";
        let p = request(1, command, &["claude\u{202E}", "ba\u{2192}sh", "ward run"]);
        let a = Current {
            request: p,
            remaining: 90,
            blocked: false,
            kinds: Some(vec![Some("banco")]),
        };
        let e = display(&a);
        for c in e
            .command
            .chars()
            .chain(e.chain.iter().flat_map(|c| c.chars()))
        {
            assert!(
                !ward_core::display::invisible(c),
                "caractere invisível {:?} passou",
                c
            );
        }
        assert!(e.command.contains("\\u{202E}fdp.exe"), "{}", e.command);
        assert!(e.command.contains("\\u{2066}x\\u{200B}y"), "{}", e.command);
        assert!(e.command.contains("\\x1b[2J\\nward run"), "{}", e.command);
        assert!(e.command.contains("\\u{E0041}"), "{}", e.command);
        // The HTML passes as text (the screen never uses `{@html}`): nothing disappears.
        assert!(e.command.contains("<img src=x onerror=alert(1)>"));
        // Order and content: taking the escapes out, the original visible command is left.
        assert!(e.command.starts_with("\"sh\" \"-c\" \"echo <img"));
        assert_eq!(e.chain, ["claude\\u{202E}", "ba\\u{2192}sh", "ward run"]);
        assert_eq!(e.entries[0].kind, Some("banco"));
        assert_eq!(e.status, "pendente");
        // The entry name also goes through D51 (the daemon already refuses it, but the
        // window does not trust it).
        let mut p = request(2, "\"ls\"", &[]);
        p.entradas = vec!["acme\u{202E}gp".into()];
        let a = Current {
            request: p,
            remaining: 1,
            blocked: true,
            kinds: None,
        };
        let e = display(&a);
        assert_eq!(e.entries[0].name, "acme\\u{202E}gp");
        assert_eq!(e.status, "bloqueado");
    }

    #[test]
    fn deadline_of_120_s_and_expired_leaves() {
        let a = Approvals::default();
        let t0 = Instant::now();
        a.arrived_at(request(1, "\"ls\"", &[]), t0);
        assert_eq!(a.current_at(t0).unwrap().remaining, 120);
        assert_eq!(
            a.current_at(t0 + Duration::from_millis(100_400))
                .unwrap()
                .remaining,
            20
        );
        assert!(a.current_at(t0 + APPROVAL_TIMEOUT).is_none());
        // Expired does not come back.
        assert!(a.current_at(t0).is_none());
    }

    #[test]
    fn arrival_order_without_repeating_and_with_limit() {
        let a = Approvals::default();
        a.arrived(request(1, "\"a\"", &[]));
        a.arrived(request(2, "\"b\"", &[]));
        a.arrived(request(1, "\"outro\"", &[]));
        assert_eq!(a.pending(), [1, 2]);
        assert_eq!(a.current().unwrap().request.comando, "\"a\"");
        a.mark_blocked(1);
        assert_eq!(a.pending(), [2]);
        assert_eq!(a.state_of(1), Some(true));
        assert_eq!(a.state_of(2), Some(false));
        assert_eq!(a.state_of(3), None);
        a.remove(1);
        assert_eq!(a.current().unwrap().request.id, 2);
        for i in 10..100 {
            a.arrived(request(i, "\"x\"", &[]));
        }
        assert_eq!(a.pending().len(), MAX_PENDING);
        a.clear();
        assert!(!a.has_request());
    }

    #[test]
    fn watcher_takes_the_approval_and_leaves_the_unlock() {
        let queue = Arc::new(NoticeQueue::default());
        let approvals = Arc::new(Approvals::default());
        let (tx, rx) = std::sync::mpsc::channel();
        watch(Arc::clone(&queue), Arc::clone(&approvals), move || {
            let _ = tx.send(());
        })
        .unwrap();
        queue.simulate(Notice::test_unlock("acme-pg-aprovacao"));
        queue.simulate(Notice::Approval(request(9, "\"ls\"", &["claude"])));
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(approvals.pending(), [9]);
        assert_eq!(queue.take(), Some(Notice::test_unlock("acme-pg-aprovacao")));
    }

    /// m2 Phase 32: the window receives the old keys (m2:DT3, `rename` until Phase 54).
    #[test]
    fn attack_new_rust_names_are_not_wire_names() {
        let e = display(&Current {
            request: request(1, "\"ls\"", &["claude"]),
            remaining: 90,
            blocked: false,
            kinds: Some(vec![Some("banco")]),
        });
        let v = serde_json::to_value(&e).unwrap();
        let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "ambiente",
                "cadeia",
                "comando",
                "confirmacao",
                "diretorio",
                "entrada_oculta",
                "entradas",
                "estado",
                "id",
                "operacao",
                "origem",
                "restante"
            ]
        );
        assert_eq!(v["entradas"][0]["nome"], "acme-pg-aprovacao");
        assert_eq!(v["entradas"][0]["tipo"], "banco");
        let r = serde_json::to_string(&ApprovalResult {
            result: "respondido",
            has_other: false,
        })
        .unwrap();
        assert_eq!(r, r#"{"resultado":"respondido","ha_outro":false}"#);
    }
}

/// The whole service with the fake daemon (the same rule as `wardd`): S9, S10, D51.
#[cfg(test)]
mod service_tests {
    use super::tests::request;
    use super::*;
    use crate::area_transferencia::{AreaTransferencia, Copiador};
    use crate::fake_daemon::{FakeDaemon, SECRETS};
    use crate::nivel::Pin;
    use crate::service::Service;
    use std::sync::Mutex as M;
    use ward_core::entry::{AccessLevel, Environment};
    use ward_core::protocol;
    use zeroize::Zeroizing;

    struct NoClipboard;
    impl AreaTransferencia for NoClipboard {
        fn escrever(&self, _: &[u8]) -> Result<(), AppError> {
            Err(AppError::Clipboard)
        }
        fn ler(&self) -> Result<Option<Zeroizing<Vec<u8>>>, AppError> {
            Ok(None)
        }
        fn limpar(&self) -> Result<(), AppError> {
            Ok(())
        }
    }

    /// The fake daemon behind an `Arc<Mutex>`, so the test can look at its state.
    #[derive(Clone)]
    struct Fake(Arc<M<FakeDaemon>>);
    impl crate::daemon::Daemon for Fake {
        fn exchange(&mut self, p: &[u8]) -> Result<Zeroizing<Vec<u8>>, AppError> {
            self.0.lock().unwrap().exchange(p)
        }
    }

    const RIGHT_PIN: &str = "135790";

    fn pin(s: &str) -> Option<Pin> {
        Some(Pin::novo(s.to_owned()).unwrap())
    }

    fn setup(requests: &[PedidoDeAprovacao]) -> (Service<Fake, NoClipboard>, Fake) {
        let mut d = FakeDaemon::full();
        for p in requests {
            d = d.with_approval(p);
        }
        let f = Fake(Arc::new(M::new(d)));
        let s = Service::new(f.clone(), Copiador::novo(NoClipboard));
        for p in requests {
            s.approvals().arrived(p.clone());
        }
        (s, f)
    }

    fn pending_in_daemon(f: &Fake) -> Vec<u64> {
        f.0.lock().unwrap().pending_approvals()
    }

    #[test]
    fn deny_without_pin_and_allow_once_with_pin() {
        let (s, f) = setup(&[request(1, "\"ls\"", &["claude"]), request(2, "\"ls\"", &[])]);
        let e = s.approval_request().unwrap().unwrap();
        assert_eq!((e.id, e.environment, e.operation), (1, "stage", "executar"));
        assert_eq!(e.entries[0].kind, Some("banco"));
        let r = s.answer_approval(1, "negar", None, false).unwrap();
        assert_eq!((r.result, r.has_other), ("respondido", true));
        assert_eq!(pending_in_daemon(&f), [2]);
        let r = s
            .answer_approval(2, "uma_vez", pin(RIGHT_PIN), false)
            .unwrap();
        assert_eq!((r.result, r.has_other), ("respondido", false));
        assert!(pending_in_daemon(&f).is_empty());
        assert_eq!(s.approval_request().unwrap(), None);
    }

    /// V5.18 (S9): a wrong PIN leaves it pending; the 5th locks the vault, denies and the
    /// window shows blocked; "Allow" on the blocked one does not go to the daemon;
    /// "Deny" only closes.
    #[test]
    fn attack_wrong_pin_five_times_locks_and_denies() {
        let (s, f) = setup(&[request(7, "\"ls\"", &[])]);
        for _ in 0..4 {
            let r = s
                .answer_approval(7, "uma_vez", pin("000000"), false)
                .unwrap();
            assert_eq!(r.result, "pin-incorreto");
            assert_eq!(pending_in_daemon(&f), [7]);
            assert_eq!(s.approval_request().unwrap().unwrap().status, "pendente");
        }
        let r = s
            .answer_approval(7, "sempre", pin("000000"), false)
            .unwrap();
        assert_eq!((r.result, r.has_other), ("bloqueado", true));
        assert!(pending_in_daemon(&f).is_empty(), "o daemon negou");
        let e = s.approval_request().unwrap().unwrap();
        assert_eq!((e.status, e.remaining), ("bloqueado", 0));
        assert_eq!(
            s.answer_approval(7, "uma_vez", pin(RIGHT_PIN), false),
            Err(AppError::Locked)
        );
        let r = s.answer_approval(7, "negar", None, false).unwrap();
        assert_eq!((r.result, r.has_other), ("respondido", false));
        // The entry did not become free.
        assert_eq!(
            f.0.lock().unwrap().level_of("acme-pg-aprovacao"),
            Some(AccessLevel::Approval)
        );
    }

    /// S10: an answer forged by the interface (id the daemon did not send, made-up
    /// decision, "Deny" with PIN, "Allow" without PIN) does not reach the daemon and
    /// the request stays pending.
    #[test]
    fn attack_forged_answer_does_not_reach_the_daemon() {
        let (s, f) = setup(&[request(3, "\"ls\"", &[])]);
        assert_eq!(
            s.answer_approval(4, "uma_vez", pin(RIGHT_PIN), false),
            Err(AppError::InvalidRequest)
        );
        assert_eq!(
            s.answer_approval(3, "livre", pin(RIGHT_PIN), false),
            Err(AppError::InvalidRequest)
        );
        assert_eq!(
            s.answer_approval(3, "negar", pin(RIGHT_PIN), false),
            Err(AppError::InvalidRequest)
        );
        for d in ["uma_vez", "uma_hora", "sempre"] {
            assert_eq!(
                s.answer_approval(3, d, None, false),
                Err(AppError::InvalidRequest),
                "{d}"
            );
        }
        assert_eq!(pending_in_daemon(&f), [3]);
        assert_eq!(s.approvals().pending(), [3]);
    }

    /// Phase 44c (D9): the confirmation of the `http` origin goes to the window with the
    /// origin (visible escape) and is only decided by "Deny" or "Confirm" (no PIN); a
    /// request with PIN is never decided by "Confirm". The forged interface does not
    /// reach the daemon.
    #[test]
    fn attack_confirmation_only_by_confirm_and_request_with_pin_never() {
        let mut c = request(11, "", &["claude", "ward playwright"]);
        c.operacao = ward_core::protocol::Operation::Playwright;
        c.diretorio.clear();
        c.origem = "http://localhost:8081\u{202E}".into();
        c.confirmacao = true;
        let n = request(12, "\"ls\"", &["claude"]);
        let (s, f) = setup(&[c, n]);
        let e = s.approval_request().unwrap().unwrap();
        assert_eq!(e.id, 11);
        assert!(e.confirmation);
        assert_eq!(e.origin, "http://localhost:8081\\u{202E}");
        for d in ["uma_vez", "uma_hora", "sempre"] {
            assert_eq!(
                s.answer_approval(11, d, pin(RIGHT_PIN), false),
                Err(AppError::InvalidRequest),
                "{d} numa confirmação"
            );
        }
        assert_eq!(
            s.answer_approval(11, "confirmar", pin(RIGHT_PIN), false),
            Err(AppError::InvalidRequest)
        );
        assert_eq!(
            s.answer_approval(12, "confirmar", None, false),
            Err(AppError::InvalidRequest),
            "confirmar num pedido com PIN"
        );
        assert_eq!(pending_in_daemon(&f), [11, 12]);
        let r = s.answer_approval(11, "confirmar", None, false).unwrap();
        assert_eq!(r.result, "respondido");
        assert_eq!(pending_in_daemon(&f), [12]);
    }

    /// Phase 54b (D68): the request with the hidden standard input goes to the window
    /// with the notice; an "Allow" without the box (acknowledged) does not reach the
    /// daemon, nor the box on a request without the notice, nor with "Deny"; with the
    /// box, it passes. The fake daemon checks again.
    #[test]
    fn attack_hidden_entry_only_with_the_box() {
        let mut o = request(21, "\"python3\"", &["claude", "bash", "ward run"]);
        o.entrada_oculta = true;
        let n = request(22, "\"ls\"", &["claude"]);
        let (s, f) = setup(&[o, n]);
        let e = s.approval_request().unwrap().unwrap();
        assert_eq!(e.id, 21);
        assert!(e.hidden_entry);
        for d in ["uma_vez", "uma_hora", "sempre"] {
            assert_eq!(
                s.answer_approval(21, d, pin(RIGHT_PIN), false),
                Err(AppError::InvalidRequest),
                "{d} sem a caixa"
            );
        }
        assert_eq!(
            s.answer_approval(21, "negar", None, true),
            Err(AppError::InvalidRequest)
        );
        assert_eq!(
            s.answer_approval(22, "uma_vez", pin(RIGHT_PIN), true),
            Err(AppError::InvalidRequest),
            "caixa num pedido sem o aviso"
        );
        assert_eq!(pending_in_daemon(&f), [21, 22]);
        // The daemon also refuses (a forged interface that got past here).
        let q = protocol::encode_request(
            protocol::Role::App,
            &protocol::Request::AnswerApproval {
                request: 21,
                decision: protocol::Decision::AllowOnce,
                pin: Some(ward_core::entry::Secret::from_text(RIGHT_PIN.to_owned())),
                confirmed_entry: false,
            },
        )
        .unwrap();
        let r = crate::daemon::Daemon::exchange(&mut *f.0.lock().unwrap(), &q).unwrap();
        assert!(matches!(
            protocol::decode_response(protocol::Role::App, &r),
            Ok(protocol::Resposta::Erro(
                protocol::ResponseError::InvalidRequest
            ))
        ));
        let r = s
            .answer_approval(21, "uma_vez", pin(RIGHT_PIN), true)
            .unwrap();
        assert_eq!(r.result, "respondido");
        let r = s
            .answer_approval(22, "uma_vez", pin(RIGHT_PIN), false)
            .unwrap();
        assert_eq!(r.result, "respondido");
        assert!(pending_in_daemon(&f).is_empty());
    }

    /// V5.16: "Allow always" makes the entry free; with two requests, "close" denies
    /// all the pending ones.
    #[test]
    fn always_makes_free_and_close_denies_all() {
        let (s, f) = setup(&[request(5, "\"ls\"", &[])]);
        s.answer_approval(5, "sempre", pin(RIGHT_PIN), false)
            .unwrap();
        assert_eq!(
            f.0.lock().unwrap().level_of("acme-pg-aprovacao"),
            Some(AccessLevel::Free)
        );
        let mut prod = request(6, "\"ls\"", &[]);
        prod.entradas = vec!["acme-pg-prod".into()];
        prod.ambiente = Environment::Prod;
        let (s, f) = setup(&[prod, request(8, "\"ls\"", &[])]);
        assert_eq!(s.approval_request().unwrap().unwrap().environment, "prod");
        s.deny_pending_approvals();
        assert!(pending_in_daemon(&f).is_empty());
        assert_eq!(s.approval_request().unwrap(), None);
    }

    /// S1, S11: nothing the window receives has a PIN or a vault secret.
    #[test]
    fn attack_no_secret_or_pin_in_the_window() {
        let (s, _f) = setup(&[request(9, "\"psql\" \"-c\" \"select 1\"", &["claude"])]);
        let mut texts = vec![serde_json::to_string(&s.approval_request().unwrap()).unwrap()];
        for _ in 0..2 {
            let r = s
                .answer_approval(9, "uma_hora", pin("000000"), false)
                .unwrap();
            texts.push(serde_json::to_string(&r).unwrap());
        }
        let r = s
            .answer_approval(9, "uma_hora", pin(RIGHT_PIN), false)
            .unwrap();
        texts.push(serde_json::to_string(&r).unwrap());
        for t in &texts {
            assert!(!t.contains(RIGHT_PIN) && !t.contains("000000"), "{t}");
            for seg in SECRETS {
                assert!(!t.contains(seg), "segredo na janela: {t}");
            }
        }
    }
}
