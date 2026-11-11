//! Approval engine (Phase 23; S9, S10; D2, D8, D16, D19, D21, D57).
//!
//! - A use "with approval" becomes a pending request with an id. The request goes to the
//!   app that subscribed ([`Engine::subscribe`]: the connection of the app role, D65 a);
//!   without a subscribed app, the engine opens the app through the launcher
//!   ([`AppOpener`]). With no way to open it (Wayland session outside D57, untrusted
//!   binary, no installation) the request is denied at once with "approval unavailable"
//!   (D21).
//! - The request waits for the app's answer up to [`APPROVAL_DEADLINE`] (2 min, S10) by
//!   the injected clock; past it, "expired". The app connection that dropped with the
//!   request delivered denies ("closing denies"); the app that exited without having
//!   received the request gives "approval unavailable". The client (CLI) that drops while
//!   waiting cancels.
//! - The answer comes only through the app role (the protocol table, V3.15). The PIN, the
//!   effect of each button and the 1 hour grant are in [`crate::core`]; here only the
//!   request is decided ([`Engine::decide`]).
//! - Vault locked on an agent use: the engine asks the app for the unlock screen and
//!   waits up to [`UNLOCK_DEADLINE`] (D16), counted apart from the approval. The notice
//!   carries the entry, the operation and the command already with visible escape (D51)
//!   and the seconds left by the engine's clock (Phase 39a). With more than one use
//!   waiting, the app receives the newest (one notice per new wait; on subscribing, only
//!   the newest).
//!
//! There is no approver in the binary: the tests (D8) subscribe a [`Subscriber`] in
//! process and answer through [`crate::core::Daemon::serve`] with the app role, the same
//! path as the socket. No feature, variable or argument changes that.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use ward_core::protocol::{
    MAX_RESTANTE_DESTRAVE, PedidoDeAprovacao, PedidoDeDestrave, ResponseError, Resposta,
};

use crate::{debug, warning};

/// Deadline for the approval window's answer (S10).
pub const APPROVAL_DEADLINE: Duration = Duration::from_secs(120);
/// Deadline for the wait for the unlock (D16).
pub const UNLOCK_DEADLINE: Duration = Duration::from_secs(120);
/// "Allow for 1 hour" (D2).
pub const ONE_HOUR: Duration = Duration::from_secs(3600);
/// How often (in real time) the wait looks at the clock, the client and the vault.
const STEP: Duration = Duration::from_millis(25);

/// The engine's clock (monotonic; injectable in the tests).
pub trait EngineClock: Send + Sync {
    /// Time since some fixed point.
    fn now(&self) -> Duration;
}

/// The system's monotonic clock.
pub struct MonotonicClock(Instant);

impl MonotonicClock {
    pub fn new() -> Self {
        Self(Instant::now())
    }
}

impl Default for MonotonicClock {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineClock for MonotonicClock {
    fn now(&self) -> Duration {
        self.0.elapsed()
    }
}

/// Why the app was not opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenAppError {
    /// There is already an instance (one at a time, D57); it will subscribe.
    AlreadyOpen,
    /// There is no way to open it (D21).
    Unavailable,
}

/// How the engine opens the app (the D57 launcher in the daemon; a fake in the tests).
pub trait AppOpener: Send + Sync {
    /// Opens the app. `on_exit` is called once, when the app opened by this call
    /// exits.
    fn open(&self, on_exit: Box<dyn FnOnce() + Send>) -> Result<(), OpenAppError>;
}

/// No app: everything that needs the window is "approval unavailable" (D21).
pub struct NoApp;

impl AppOpener for NoApp {
    fn open(&self, _on_exit: Box<dyn FnOnce() + Send>) -> Result<(), OpenAppError> {
        Err(OpenAppError::Unavailable)
    }
}

/// Who receives the engine's notices: the connection of the app role (D65 a).
pub trait Subscriber: Send {
    /// Delivers an approval request or an unlock request. `false`: it did not work
    /// (the connection dropped); the engine treats it as a closed app.
    fn deliver(&mut self, r: &Resposta) -> bool;
}

/// The client that made the agent's request (the CLI connection).
pub trait Client {
    /// Process chain to display (`claude → bash → ward`), from the oldest to the
    /// newest.
    fn chain(&self) -> Vec<String>;
    /// Did the connection drop? (the wait is cancelled.)
    fn exited(&self) -> bool;
}

/// Client without a connection (in-process call): no chain, never exits.
pub struct NoClient;

impl Client for NoClient {
    fn chain(&self) -> Vec<String> {
        Vec::new()
    }
    fn exited(&self) -> bool {
        false
    }
}

struct Pending {
    request: PedidoDeAprovacao,
    /// Id of the subscriber that received it.
    delivered_to: Option<u64>,
    result: Option<Result<(), ResponseError>>,
}

/// An agent use waiting for the unlock: the notice (without the seconds left, computed
/// on delivery) and the deadline by the engine's clock.
struct UnlockWait {
    id: u64,
    deadline: Duration,
    notice: PedidoDeDestrave,
}

impl UnlockWait {
    /// The notice with the seconds left at `now`, rounded up, from
    /// 1 up to the protocol's maximum.
    fn notice_at(&self, now: Duration) -> Resposta {
        let left = self.deadline.saturating_sub(now);
        let s = left.as_secs() + u64::from(left.subsec_nanos() > 0);
        let mut a = self.notice.clone();
        a.restante = s.clamp(1, MAX_RESTANTE_DESTRAVE);
        Resposta::PedidoDeDestrave(a)
    }
}

struct State {
    next_id: u64,
    pending: Vec<Pending>,
    subscriber: Option<(u64, Box<dyn Subscriber>)>,
    /// Agent requests waiting for the unlock now, from the oldest to the newest.
    awaiting_unlock: Vec<UnlockWait>,
}

impl State {
    fn find_pending(&mut self, id: u64) -> Option<&mut Pending> {
        self.pending.iter_mut().find(|p| p.request.id == id)
    }

    fn take(&mut self, id: u64) -> Option<Pending> {
        let i = self.pending.iter().position(|p| p.request.id == id)?;
        Some(self.pending.remove(i))
    }

    /// Delivers to the subscriber everything not yet delivered (and the newest unlock,
    /// with the seconds left at `now`, if `unlock`). A failed delivery: the subscriber
    /// leaves.
    fn deliver_pending(&mut self, unlock: Option<Duration>) {
        let Some((sid, a)) = self.subscriber.as_mut() else {
            return;
        };
        let sid = *sid;
        let mut ok = true;
        if let (Some(now), Some(d)) = (unlock, self.awaiting_unlock.last()) {
            ok = a.deliver(&d.notice_at(now));
        }
        for p in self.pending.iter_mut() {
            if !ok {
                break;
            }
            if p.delivered_to.is_none() && p.result.is_none() {
                ok = a.deliver(&Resposta::PedidoDeAprovacao(p.request.clone()));
                if ok {
                    p.delivered_to = Some(sid);
                }
            }
        }
        if !ok {
            warning!("entrega ao app falhou: a janela conta como fechada");
            self.drop_subscriber(sid);
        }
    }

    /// The subscriber `sid` left: what it received and did not answer is denied (closing
    /// denies).
    fn drop_subscriber(&mut self, sid: u64) {
        if self.subscriber.as_ref().is_some_and(|(id, _)| *id == sid) {
            self.subscriber = None;
        }
        for p in self.pending.iter_mut() {
            if p.delivered_to == Some(sid) && p.result.is_none() {
                p.result = Some(Err(ResponseError::Denied));
            }
        }
    }
}

struct Inner {
    state: Mutex<State>,
    changed: Condvar,
}

impl Inner {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The opened app exited: a request nobody received is left without a window (D21).
    fn app_exited(&self) {
        let mut g = self.state();
        if g.subscriber.is_none() {
            for p in g.pending.iter_mut() {
                if p.delivered_to.is_none() && p.result.is_none() {
                    p.result = Some(Err(ResponseError::ApprovalUnavailable));
                }
            }
        }
        drop(g);
        self.changed.notify_all();
    }
}

/// The approval engine.
pub struct Engine {
    clock: Arc<dyn EngineClock>,
    opener: Box<dyn AppOpener>,
    inner: Arc<Inner>,
    /// Vault locked and no app to show the screen: waits the 2 min anyway (the user may
    /// unlock through the CLI, V4.21). Off, answers locked at once.
    wait_without_app: bool,
}

impl Engine {
    pub fn new(clock: Arc<dyn EngineClock>, opener: Box<dyn AppOpener>) -> Self {
        Self {
            clock,
            opener,
            inner: Arc::new(Inner {
                state: Mutex::new(State {
                    next_id: 1,
                    pending: Vec::new(),
                    subscriber: None,
                    awaiting_unlock: Vec::new(),
                }),
                changed: Condvar::new(),
            }),
            wait_without_app: false,
        }
    }

    /// With the vault locked and no app, the agent's request waits for the unlock through
    /// the CLI (its unlock in another terminal) up to [`UNLOCK_DEADLINE`] (D16,
    /// V4.21), instead of answering locked at once. The `wardd` turns it on; the
    /// engine's tests do not.
    pub fn waiting_without_app(mut self) -> Self {
        self.wait_without_app = true;
        self
    }

    /// Engine without an app (D21), with the system clock.
    pub fn without_app() -> Self {
        Self::new(Arc::new(MonotonicClock::new()), Box::new(NoApp))
    }

    /// Now, by the engine's clock (1 hour grant).
    pub fn now(&self) -> Duration {
        self.clock.now()
    }

    /// Opens the app through the launcher. Already open is also a success.
    fn open(&self) -> Result<(), OpenAppError> {
        let inner = Arc::downgrade(&self.inner);
        let r = self.opener.open(Box::new(move || {
            if let Some(i) = inner.upgrade() {
                i.app_exited();
            }
        }));
        match r {
            Ok(()) | Err(OpenAppError::AlreadyOpen) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// The open app operation (D57): from any role, no field, no secret.
    pub fn open_app(&self) -> Result<(), ResponseError> {
        self.open().map_err(|_| ResponseError::ApprovalUnavailable)
    }

    /// The connection `id` of the app role starts receiving the notices. One at a
    /// time: the new one takes the place of the previous one, and what that one had
    /// received without answering goes again to the new one.
    pub fn subscribe(&self, id: u64, a: Box<dyn Subscriber>) {
        let mut g = self.inner.state();
        if let Some((old, _)) = g.subscriber.take() {
            for p in g.pending.iter_mut() {
                if p.delivered_to == Some(old) {
                    p.delivered_to = None;
                }
            }
        }
        g.subscriber = Some((id, a));
        g.deliver_pending(Some(self.clock.now()));
        drop(g);
        self.inner.changed.notify_all();
    }

    /// The app connection `id` closed.
    pub fn unsubscribe(&self, id: u64) {
        let mut g = self.inner.state();
        g.drop_subscriber(id);
        drop(g);
        self.inner.changed.notify_all();
    }

    /// Is there a subscribed app?
    pub fn has_subscriber(&self) -> bool {
        self.inner.state().subscriber.is_some()
    }

    /// Asks for the approval and waits (S10). `Ok`: allowed (by one of the three buttons,
    /// with the PIN verified by the core). The request's `id` is the engine's.
    pub fn request_approval(
        &self,
        mut request: PedidoDeAprovacao,
        client: &dyn Client,
    ) -> Result<(), ResponseError> {
        let mut g = self.inner.state();
        let id = g.next_id;
        g.next_id = g.next_id.saturating_add(1);
        request.id = id;
        let deadline = self.clock.now().saturating_add(APPROVAL_DEADLINE);
        g.pending.push(Pending {
            request,
            delivered_to: None,
            result: None,
        });
        if g.subscriber.is_some() {
            g.deliver_pending(None);
        } else {
            drop(g);
            if self.open().is_err() {
                warning!("aprovação {id}: app indisponível; negado (D21)");
                self.inner.state().take(id);
                return Err(ResponseError::ApprovalUnavailable);
            }
            g = self.inner.state();
        }
        debug!("aprovação {id}: pendente");
        loop {
            if let Some(r) = g.find_pending(id).and_then(|p| p.result.take()) {
                g.take(id);
                match &r {
                    Ok(()) => debug!("aprovação {id}: permitida"),
                    Err(e) => warning!("aprovação {id}: {}", e.code()),
                }
                return r;
            }
            if self.clock.now() >= deadline {
                g.take(id);
                warning!("aprovação {id}: expirou em 2 min (S10)");
                return Err(ResponseError::Expired);
            }
            if client.exited() {
                g.take(id);
                warning!("aprovação {id}: o cliente saiu; cancelada");
                return Err(ResponseError::Denied);
            }
            g = self
                .inner
                .changed
                .wait_timeout(g, STEP)
                .map(|(g, _)| g)
                .unwrap_or_else(|e| e.into_inner().0);
        }
    }

    /// Denies every pending approval request still without a decision (Phase 49a: the
    /// restore swaps the vault, and a late answer would apply to the new version).
    /// Whoever asked receives "denied"; the window that still shows the request receives
    /// "expired" when answering. Returns how many were denied.
    pub fn cancel_pending(&self) -> usize {
        let mut g = self.inner.state();
        let mut n = 0;
        for p in g.pending.iter_mut() {
            if p.result.is_none() {
                p.result = Some(Err(ResponseError::Denied));
                n += 1;
            }
        }
        drop(g);
        self.inner.changed.notify_all();
        n
    }

    /// The entries of the request `id`, if it is pending and still without a decision.
    pub fn pending(&self, id: u64) -> Option<Vec<String>> {
        let mut g = self.inner.state();
        g.find_pending(id)
            .filter(|p| p.result.is_none())
            .map(|p| p.request.entradas.clone())
    }

    /// Is the request `id`, pending and without a decision, only an origin confirmation
    /// (Phase 44c)?
    pub fn is_confirmation(&self, id: u64) -> Option<bool> {
        let mut g = self.inner.state();
        g.find_pending(id)
            .filter(|p| p.result.is_none())
            .map(|p| p.request.confirmacao)
    }

    /// Does the request `id`, pending and without a decision, have a standard input that
    /// the window does not show (Phase 54b, D68)?
    pub fn is_hidden_stdin(&self, id: u64) -> Option<bool> {
        let mut g = self.inner.state();
        g.find_pending(id)
            .filter(|p| p.result.is_none())
            .map(|p| p.request.entrada_oculta)
    }

    /// Decides the request `id`. `false`: it is no longer pending (expired, was cancelled
    /// or already decided).
    pub fn decide(&self, id: u64, r: Result<(), ResponseError>) -> bool {
        let mut g = self.inner.state();
        let Some(p) = g.find_pending(id).filter(|p| p.result.is_none()) else {
            return false;
        };
        p.result = Some(r);
        drop(g);
        self.inner.changed.notify_all();
        true
    }

    /// Vault locked on an agent use (D16): asks the app for the unlock screen and waits
    /// up to 2 min. `notice` is what the screen shows (entry, operation and command
    /// already with visible escape, D51; the seconds left are the engine's). `locked` is
    /// queried without holding any engine lock.
    pub fn wait_for_unlock(
        &self,
        notice: PedidoDeDestrave,
        locked: &dyn Fn() -> bool,
        client: &dyn Client,
    ) -> Result<(), ResponseError> {
        let now = self.clock.now();
        let deadline = now.saturating_add(UNLOCK_DEADLINE);
        let wait;
        {
            let mut g = self.inner.state();
            wait = g.next_id;
            g.next_id = g.next_id.saturating_add(1);
            let d = UnlockWait {
                id: wait,
                deadline,
                notice,
            };
            let response = d.notice_at(now);
            g.awaiting_unlock.push(d);
            if g.subscriber.is_some() {
                // Only the unlock request (the approval ones were already sent).
                let ok = g
                    .subscriber
                    .as_mut()
                    .is_some_and(|(_, a)| a.deliver(&response));
                if !ok && let Some((sid, _)) = g.subscriber.as_ref() {
                    let sid = *sid;
                    g.drop_subscriber(sid);
                }
            } else {
                drop(g);
                if self.open().is_err() {
                    if !self.wait_without_app {
                        // Nobody would show the screen; whoever asked learns it now.
                        self.leave_unlock_wait(wait);
                        warning!("pedido com o cofre travado: app indisponível");
                        return Err(ResponseError::Locked);
                    }
                    // V4.21: the user may unlock through the CLI; the CLI tells whoever
                    // asked.
                    warning!("pedido com o cofre travado: app indisponível; esperando a CLI");
                }
            }
        }
        warning!("pedido com o cofre travado: esperando o destrave (até 2 min, D16)");
        let r = loop {
            if !locked() {
                break Ok(());
            }
            if self.clock.now() >= deadline {
                warning!("pedido com o cofre travado: tempo esgotado");
                break Err(ResponseError::LockedTimedOut);
            }
            if client.exited() {
                break Err(ResponseError::Locked);
            }
            std::thread::sleep(STEP);
        };
        self.leave_unlock_wait(wait);
        r
    }

    fn leave_unlock_wait(&self, id: u64) {
        let mut g = self.inner.state();
        g.awaiting_unlock.retain(|d| d.id != id);
    }
}
