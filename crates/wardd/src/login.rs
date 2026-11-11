//! `ward login` (Phase 43; R14, D9; S1, S13): the daemon takes the request to the Chrome
//! extension, which checks the origin of the active tab, fills, submits and clears the
//! fields.
//!
//! The daemon does not talk to the extension on its own: the extension's service worker
//! keeps `ward-nm` open and asks for the next login request (long wait, up to
//! [`LoginDeadlines::wait`]; [`Daemon::await_login`]). The path of a request:
//!
//! 1. the agent asks `login <entry>`: type `login` checked before the approval, then
//!    the level and the approval (the daemon's usage authorization: blocked =
//!    nonexistent, V3.17);
//! 2. the daemon creates a request with a random id (32 letters and digits, from the
//!    system RNG) and hands it to the first extension that asks: the login request
//!    response carries only the id and the entry name, no value;
//! 3. the extension sends the fill operation ([`Daemon::fill_login`]) with the id and the
//!    origin of the main frame document of the active tab. The daemon compares the
//!    origin with the entry URL through the same fill of Phase 40 (D9): if different, the
//!    request ends in the "different origin" error and no value leaves; if equal, the
//!    fill response leaves (only for the extension role). An id fills only once;
//! 4. the extension reports the outcome ([`Daemon::login_result`]) and the agent gets
//!    `ok` or the error.
//!
//! One request at a time (the second agent gets the "busy" error). Nobody took the
//! request within [`LoginDeadlines::pickup`], or the extension did not finish within
//! [`LoginDeadlines::total`]: the agent gets the "browser failed" error and the id
//! dies. Vault
//! locked in the middle, or the CLI dropped: the request dies and nothing else is
//! filled. The log carries only the entry name and the outcome, never the origin nor
//! values (S13).

use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use ward_core::entry::{AccessLevel, Data};
use ward_core::generator;
use ward_core::protocol::{LoginResult, Request, ResponseError, Resposta, Role};

use crate::aprovacao::Cliente;
use crate::aviso;
use crate::nucleo::Daemon;

/// Deadlines of `ward login`.
#[derive(Debug, Clone, Copy)]
pub struct LoginDeadlines {
    /// How long the extension's wait ([`Daemon::await_login`]) waits for a request before answering
    /// `ok` (empty) and asking again.
    pub wait: Duration,
    /// How long the request waits for an extension to take it.
    pub pickup: Duration,
    /// How long the request waits for the outcome, counted from when the extension took
    /// it (the submission and the next page loading).
    pub total: Duration,
}

impl Default for LoginDeadlines {
    fn default() -> Self {
        Self {
            wait: Duration::from_secs(25),
            pickup: Duration::from_secs(10),
            total: Duration::from_secs(60),
        }
    }
}

/// How often the waiting agent checks whether the CLI dropped or the vault locked.
const STEP: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Waiting for an extension to take it.
    Waiting,
    /// An extension took it; not filled yet.
    Delivered,
    /// The fill went out; the outcome is missing.
    Filled,
}

struct Job {
    /// Random id of the request (compared in constant time).
    id: String,
    entry: String,
    phase: Phase,
    result: Option<Result<(), ResponseError>>,
}

/// One-slot queue of the `ward login` requests.
pub struct Logins {
    current: Mutex<Option<Job>>,
    signal: Condvar,
    deadlines: LoginDeadlines,
}

impl Default for Logins {
    fn default() -> Self {
        Self::new(LoginDeadlines::default())
    }
}

impl Logins {
    pub fn new(deadlines: LoginDeadlines) -> Self {
        Self {
            current: Mutex::new(None),
            signal: Condvar::new(),
            deadlines,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<Job>> {
        self.current.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The current request, if the id is its id.
    fn by_id<'a>(current: &'a mut Option<Job>, id: &str) -> Option<&'a mut Job> {
        current
            .as_mut()
            .filter(|t| generator::tokens_equal(t.id.as_bytes(), id.as_bytes()))
    }

    /// Finishes the request `id` (if it is still the current one and has not finished)
    /// and wakes up whoever waits.
    fn finish(&self, id: &str, r: Result<(), ResponseError>) -> bool {
        let mut g = self.lock();
        let Some(t) = Self::by_id(&mut g, id) else {
            return false;
        };
        if t.result.is_some() {
            return false;
        }
        t.result = Some(r);
        self.signal.notify_all();
        true
    }
}

/// Deletes the request when leaving `login` (also on error or panic).
struct DeleteOnDrop<'a>(&'a Logins);

impl Drop for DeleteOnDrop<'_> {
    fn drop(&mut self) {
        *self.0.lock() = None;
        self.0.signal.notify_all();
    }
}

impl Daemon {
    /// The agent's `login <entry>` (Phase 43). Only `ok` or an error come back; no
    /// value.
    pub(crate) fn login(
        &self,
        client: &dyn Cliente,
        request: &Request,
    ) -> Result<Resposta, ResponseError> {
        let Request::Login { entry } = request else {
            return Err(ResponseError::InvalidRequest);
        };
        // Type without login: refuse before asking for approval (like `playwright`).
        if let Some(a) = self.estado().aberto.as_ref()
            && a.entradas
                .iter()
                .find(|e| e.name() == entry && e.level() != AccessLevel::Blocked)
                .is_some_and(|e| !matches!(e.data(), Data::Login(_)))
        {
            return Err(ResponseError::TypeWithoutDelivery);
        }
        self.autorizar_uso(client, request)?;
        self.registrar_uso();
        // The entry may have changed during the approval: check again.
        let name = {
            let st = self.estado();
            let a = st.aberto.as_ref().ok_or(ResponseError::Locked)?;
            let e = a
                .entradas
                .iter()
                .find(|e| e.name() == entry && e.level() != AccessLevel::Blocked)
                .ok_or(ResponseError::EntryNotFound)?;
            if !matches!(e.data(), Data::Login(_)) {
                return Err(ResponseError::TypeWithoutDelivery);
            }
            e.name().to_owned()
        };
        let id = {
            let t = generator::connect_token().map_err(|_| ResponseError::Internal)?;
            String::from_utf8(t.expose().to_vec()).map_err(|_| ResponseError::Internal)?
        };
        let logins = self.logins();
        {
            let mut g = logins.lock();
            if g.is_some() {
                return Err(ResponseError::Busy);
            }
            *g = Some(Job {
                id: id.clone(),
                entry: name.clone(),
                phase: Phase::Waiting,
                result: None,
            });
            logins.signal.notify_all();
        }
        let _delete = DeleteOnDrop(logins);
        aviso!("login {name}: pedido à extensão");
        let r = self.wait_outcome(logins, &id, client);
        match &r {
            Ok(()) => aviso!("login {name}: formulário enviado"),
            Err(e) => aviso!("login {name}: {}", e.code()),
        }
        r.map(|()| Resposta::Ok)
    }

    fn wait_outcome(
        &self,
        logins: &Logins,
        id: &str,
        client: &dyn Cliente,
    ) -> Result<(), ResponseError> {
        let start = Instant::now();
        let mut taken: Option<Instant> = None;
        let mut g = logins.lock();
        loop {
            let Some(t) = Logins::by_id(&mut g, id) else {
                return Err(ResponseError::Internal);
            };
            if let Some(r) = t.result.clone() {
                return r;
            }
            if t.phase != Phase::Waiting && taken.is_none() {
                taken = Some(Instant::now());
            }
            let expired = match taken {
                None => start.elapsed() >= logins.deadlines.pickup,
                Some(p) => p.elapsed() >= logins.deadlines.total,
            };
            if expired {
                // Nobody fills with this id anymore.
                t.result = Some(Err(ResponseError::BrowserFailed));
                return Err(ResponseError::BrowserFailed);
            }
            drop(g);
            // Outside the requests lock: the vault state has its own lock.
            if client.saiu() {
                logins.finish(id, Err(ResponseError::Denied));
                return Err(ResponseError::Denied);
            }
            if self.travado() {
                logins.finish(id, Err(ResponseError::Locked));
                return Err(ResponseError::Locked);
            }
            g = logins.lock();
            if Logins::by_id(&mut g, id).is_some_and(|t| t.result.is_none()) {
                g = logins
                    .signal
                    .wait_timeout(g, STEP)
                    .unwrap_or_else(|e| e.into_inner())
                    .0;
            }
        }
    }

    /// The extension's wait for a login request (extension only): the next request not
    /// yet delivered, or `ok`
    /// after [`LoginDeadlines::wait`].
    pub(crate) fn await_login(&self, role: Role) -> Result<Resposta, ResponseError> {
        if role != Role::Extension {
            return Err(ResponseError::Unauthorized);
        }
        let logins = self.logins();
        let end = Instant::now() + logins.deadlines.wait;
        let mut g = logins.lock();
        loop {
            if let Some(t) = g.as_mut()
                && t.phase == Phase::Waiting
                && t.result.is_none()
            {
                t.phase = Phase::Delivered;
                logins.signal.notify_all();
                return Ok(Resposta::PedidoDeLogin {
                    id: t.id.clone(),
                    entrada: t.entry.clone(),
                });
            }
            let now = Instant::now();
            if now >= end {
                return Ok(Resposta::Ok);
            }
            g = logins
                .signal
                .wait_timeout(g, end - now)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }

    /// The fill of a login request (extension only): the values of the request `id`,
    /// once, if the tab's origin is the entry's (D9, through the fill of Phase 40).
    pub(crate) fn fill_login(
        &self,
        role: Role,
        id: &str,
        origin: &str,
    ) -> Result<Resposta, ResponseError> {
        if role != Role::Extension {
            return Err(ResponseError::Unauthorized);
        }
        let logins = self.logins();
        let entry = {
            let mut g = logins.lock();
            let t = Logins::by_id(&mut g, id).ok_or(ResponseError::Expired)?;
            if t.phase != Phase::Delivered || t.result.is_some() {
                return Err(ResponseError::Expired);
            }
            // An id fills only once, whether it works or not.
            t.phase = Phase::Filled;
            t.entry.clone()
        };
        match self.preencher(role, &entry, origin) {
            Ok(r) => {
                // The request may have died (deadline, lock, CLI) while the value was
                // leaving the vault: then nothing leaves.
                let mut g = logins.lock();
                match Logins::by_id(&mut g, id) {
                    Some(t) if t.result.is_none() => Ok(r),
                    _ => Err(ResponseError::Expired),
                }
            }
            Err(e) => {
                logins.finish(id, Err(e.clone()));
                Err(e)
            }
        }
    }

    /// The outcome of a login request (extension only): the outcome of the request
    /// `id`.
    pub(crate) fn login_result(
        &self,
        role: Role,
        id: &str,
        outcome: LoginResult,
    ) -> Result<Resposta, ResponseError> {
        if role != Role::Extension {
            return Err(ResponseError::Unauthorized);
        }
        let logins = self.logins();
        {
            let mut g = logins.lock();
            let t = Logins::by_id(&mut g, id).ok_or(ResponseError::Expired)?;
            if t.result.is_some() || t.phase == Phase::Waiting {
                return Err(ResponseError::Expired);
            }
            // "Submitted" and "refused" only after the fill.
            if matches!(outcome, LoginResult::Submitted | LoginResult::Refused)
                && t.phase != Phase::Filled
            {
                return Err(ResponseError::InvalidRequest);
            }
        }
        let r = match outcome {
            LoginResult::Submitted => Ok(()),
            LoginResult::Refused | LoginResult::NoForm => Err(ResponseError::LoginRefused),
            LoginResult::TabChanged => Err(ResponseError::OriginMismatch),
            LoginResult::Failure => Err(ResponseError::BrowserFailed),
        };
        if logins.finish(id, r) {
            Ok(Resposta::Ok)
        } else {
            Err(ResponseError::Expired)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_of_another_request_does_not_match() {
        let l = Logins::default();
        *l.lock() = Some(Job {
            id: "a".repeat(32),
            entry: "acme-wp-dev".into(),
            phase: Phase::Delivered,
            result: None,
        });
        let mut g = l.lock();
        assert!(Logins::by_id(&mut g, &"a".repeat(32)).is_some());
        assert!(Logins::by_id(&mut g, &"b".repeat(32)).is_none());
        assert!(Logins::by_id(&mut g, &"a".repeat(31)).is_none());
        assert!(Logins::by_id(&mut g, "").is_none());
    }
}
