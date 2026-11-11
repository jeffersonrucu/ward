//! The agent's use (`ward run`, `ssh`, `login`, `playwright`) and the answer of the
//! approval window (Phase 23; S1, S9, S10; D2, D16, D19, D21, D51).
//!
//! Order of a use:
//! 1. vault locked: the engine asks the app for the unlock and waits up to 2 min (D16);
//! 2. the entry (blocked = missing, V3.17) and, in the combined use, the database's
//!    tunnel entry or the server's jump entry (D19);
//! 3. the level that holds is the most restrictive of the two; "with approval" covered
//!    by a 1 hour grant still within the deadline counts as free (D2);
//! 4. with approval: the request goes to the app with the command and the chain
//!    formatted by the D51 function (without ambiguity, with visible escape) and waits up
//!    to 2 min (S10).
//!
//! The three "Allow" buttons require the PIN, with the core's error count (S9): the 5th
//! error locks the vault and denies the request; before that the request stays pending.
//!
//! Phase 44c (D9): in the `login` and `playwright` of an entry whose URL is `http`
//! (`localhost`/`127.0.0.1`), and since Phase 44e of every entry whose host is a
//! loopback one in any scheme (`https://localhost`, `https://[::1]`…), the request
//! always goes to the window with the origin in view, even at the free level or covered
//! by the 1 hour grant: Ward does not know which process answers at that origin (another
//! process of the uid may listen on `[::1]` or on the port of a stopped service). Free
//! level: only a confirmation, without PIN ("Deny" or "Confirm"). "With approval": the
//! usual window, with the PIN and the origin. Without the app, "approval unavailable".
//! On `https` of an outside host nothing changes.
//!
//! Phase 54b (D68): the run and the `ssh` requests carry the kind of the CLI's standard
//! input. Pipe, file or other (neither terminal nor `/dev/null`): the command receives
//! data that the window does not show. On an entry "with approval" the request always
//! goes to the window, even covered by the 1 hour grant, with the warning (hidden
//! standard input), and the three "Allow" buttons only hold with the standard input
//! confirmed (the box checked in the window); without it, refused before the PIN. Free
//! level: nothing changes. The kind is told by the client: `terminal` is not checked
//! (D68, RA11).

use ward_core::display;
use ward_core::entry::{AccessLevel, Data, Entry, Environment, Secret};
use ward_core::limits::{MAX_DISPLAYED_TEXT, MAX_TEXT};
use ward_core::protocol::{
    Decision, Operation, PedidoDeAprovacao, PedidoDeDestrave, Request, ResponseError, Resposta,
    StdinKind,
};

use crate::aprovacao::{Cliente, UMA_HORA};
use crate::nucleo::{Aberto, Daemon};
use crate::warning;

/// Largest number of processes of the chain that goes to the app.
const MAX_CHAIN: usize = 64;

/// The use was authorized: by the free level (or the 1 hour grant) or by the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Authorization {
    /// Free, or covered by the 1 hour grant.
    WithoutAsking,
    /// Allowed in the approval window.
    Approved,
}

/// The most restrictive of two levels (D19).
pub fn most_restrictive(a: AccessLevel, b: AccessLevel) -> AccessLevel {
    let rank = |n| match n {
        AccessLevel::Blocked => 2,
        AccessLevel::Approval => 1,
        AccessLevel::Free => 0,
    };
    if rank(a) >= rank(b) { a } else { b }
}

/// The most sensitive environment of two, for the window's highlight.
fn most_sensitive(a: Environment, b: Environment) -> Environment {
    let rank = |x| match x {
        Environment::Prod => 2,
        Environment::Stage => 1,
        Environment::Dev => 0,
    };
    if rank(a) >= rank(b) { a } else { b }
}

/// The other entry of a combined use (D19): the database's tunnel, the server's jump.
fn second_entry(e: &Entry, op: Operation) -> Option<&str> {
    match (e.data(), op) {
        (Data::Database(b), Operation::Run) => b.tunnel.as_deref(),
        (Data::Server(s), Operation::Run | Operation::Ssh) => s.jump.as_deref(),
        _ => None,
    }
}

/// Phase 44c (D9): the local origin to which the use takes user and password (only in
/// the `login`, in the `playwright` and in the popup's fill of a `login` entry), or
/// `None` (outside host, other kind, other operation). Phase 44e (2nd M6 audit): every
/// loopback host counts, in any scheme (`https://localhost`, `https://127.0.0.1`,
/// `https://[::1]`…): a certificate from a local CA (mkcert, `~/.pki`) does not prove
/// who answers ([`ward_core::entry::local_url_origin`]).
pub(crate) fn origin_to_confirm(e: &Entry, op: Operation) -> Option<String> {
    match (e.data(), op) {
        (Data::Login(l), Operation::Login | Operation::Playwright | Operation::Fill) => {
            ward_core::entry::local_url_origin(&l.url)
        }
        _ => None,
    }
}

/// The level of an entry for this use, with the 1 hour grant (D2). With
/// `without_grant` (Phase 54b: hidden standard input), the grant does not count.
fn effective_level(
    a: &Aberto,
    e: &Entry,
    now: std::time::Duration,
    without_grant: bool,
) -> AccessLevel {
    match e.level() {
        AccessLevel::Approval
            if !without_grant && a.liberacoes.get(e.name()).is_some_and(|until| now < *until) =>
        {
            AccessLevel::Free
        }
        n => n,
    }
}

/// What the agent's request wants, for display.
struct Description<'a> {
    operation: Operation,
    entry: &'a str,
    /// The exact command (D51).
    command: String,
    /// Where the command runs: in the run request, the CLI's directory (Phase 26a),
    /// quoted and with visible escape; empty in the other operations (`ssh` runs in the
    /// passwd home folder, fixed).
    directory: String,
    /// Phase 54b (D68): the command receives through the standard input data that the
    /// window does not show (only the run request and `ssh`).
    hidden_stdin: bool,
}

/// What the agent's request wants: operation, entry, the exact command and the
/// directory.
fn describe(request: &Request) -> Option<Description<'_>> {
    let hidden = |t: &StdinKind| t.hidden();
    let (operation, entry, command, directory, hidden_stdin) = match request {
        Request::Run(x) => (
            Operation::Run,
            x.entry.as_str(),
            display::display_args(&x.argv, MAX_DISPLAYED_TEXT),
            display::display_args(&[x.cwd.as_slice()], MAX_DISPLAYED_TEXT),
            hidden(&x.stdin_kind),
        ),
        Request::Ssh {
            entry,
            command,
            stdin_kind,
            ..
        } => (
            Operation::Ssh,
            entry.as_str(),
            display::display_args(command, MAX_DISPLAYED_TEXT),
            String::new(),
            hidden(stdin_kind),
        ),
        Request::Login { entry } => (
            Operation::Login,
            entry.as_str(),
            String::new(),
            String::new(),
            false,
        ),
        Request::Playwright { entry } => (
            Operation::Playwright,
            entry.as_str(),
            String::new(),
            String::new(),
            false,
        ),
        _ => return None,
    };
    Some(Description {
        operation,
        entry,
        command,
        directory,
        hidden_stdin,
    })
}

/// Chain for display (D51): names with visible escape, within the protocol limit.
pub(crate) fn display_chain(client: &dyn Cliente) -> Vec<String> {
    let mut c = client.cadeia();
    if c.len() > MAX_CHAIN {
        c.drain(..c.len() - MAX_CHAIN);
    }
    c.iter()
        .map(|n| display::display_text(n.as_bytes(), MAX_TEXT))
        .collect()
}

impl Daemon {
    /// May the agent use the entry now? Waits for the unlock and the window when needed.
    /// Blocked answers the same as missing (V3.17); in the combined use, the other entry
    /// blocked (or gone) denies without asking.
    pub fn authorize_use(
        &self,
        client: &dyn Cliente,
        request: &Request,
    ) -> Result<Authorization, ResponseError> {
        let Description {
            operation: op,
            entry: name,
            command,
            directory,
            hidden_stdin,
        } = describe(request).ok_or(ResponseError::InvalidRequest)?;
        // An empty name is never an entry's (`ward-core` requires a non-empty name): it
        // answers as missing before any wait. Phase 39a: that way the unlock notice never
        // goes out of the format (empty entry), which would close the app's connection
        // and deny what it had on screen.
        if name.is_empty() {
            return Err(ResponseError::EntryNotFound);
        }
        if self.travado() {
            // Phase 39a (D51): the unlock screen shows what the agent asked for, with
            // visible escape. With the vault locked it is not known whether the entry
            // exists, nor its kind and environment; the name goes as the agent sent it.
            let notice = PedidoDeDestrave {
                entrada: display::display_text(name.as_bytes(), MAX_TEXT),
                operacao: op,
                comando: command.clone(),
                restante: 0,
            };
            self.aprovacao
                .esperar_destrave(notice, &|| self.travado(), client)?;
        }
        let now = self.aprovacao.agora();
        let (level, entries, environment, origin) = {
            let state = self.estado();
            let a = state.aberto.as_ref().ok_or(ResponseError::Locked)?;
            let e = a
                .entradas
                .iter()
                .find(|e| e.name() == name && e.level() != AccessLevel::Blocked)
                .ok_or(ResponseError::EntryNotFound)?;
            // Phase 54b: with the hidden standard input, the 1 hour grant does not cover.
            let mut level = effective_level(a, e, now, hidden_stdin);
            let origin = origin_to_confirm(e, op);
            let mut entries = vec![e.name().to_owned()];
            let mut environment = e.environment();
            if let Some(other) = second_entry(e, op) {
                match a.entradas.iter().find(|x| x.name() == other) {
                    Some(x) => {
                        level = most_restrictive(level, effective_level(a, x, now, hidden_stdin));
                        environment = most_sensitive(environment, x.environment());
                        entries.push(x.name().to_owned());
                    }
                    None => level = AccessLevel::Blocked,
                }
            }
            (level, entries, environment, origin)
        };
        let window_request = |confirmation: bool, origin: Option<String>| PedidoDeAprovacao {
            id: 0,
            entradas: entries.clone(),
            ambiente: environment,
            operacao: op,
            comando: command.clone(),
            diretorio: directory.clone(),
            cadeia: display_chain(client),
            origem: origin.unwrap_or_default(),
            confirmacao: confirmation,
            // Phase 54b: only in the request with PIN (the origin confirmation has no
            // standard input).
            entrada_oculta: hidden_stdin && !confirmation,
        };
        match level {
            AccessLevel::Free => match origin {
                None => Ok(Authorization::WithoutAsking),
                // Phase 44c/44e: local origin, only the confirmation (without PIN).
                Some(o) => {
                    warning!("uso numa origem local: confirmação na janela (D9)");
                    self.aprovacao
                        .pedir_aprovacao(window_request(true, Some(o)), client)?;
                    Ok(Authorization::Approved)
                }
            },
            AccessLevel::Blocked => {
                warning!("uso combinado com entrada bloqueada: negado sem perguntar (D19)");
                Err(ResponseError::Denied)
            }
            AccessLevel::Approval => {
                if hidden_stdin {
                    warning!(
                        "uso com a entrada padrão que a janela não mostra: aviso e confirmação na janela (D68)"
                    );
                }
                self.aprovacao
                    .pedir_aprovacao(window_request(false, origin), client)?;
                Ok(Authorization::Approved)
            }
        }
    }

    /// The window's answer (only from the app role, by the protocol table). "Deny" does
    /// not ask for the PIN; the three "Allow" do (S10). A wrong PIN leaves the request
    /// pending; the 5th error locks the vault and denies (S9). Phase 54b (D68): on a
    /// request with the hidden standard input, an "Allow" without the standard input
    /// confirmed is refused before the PIN (the request stays pending); the confirmation
    /// on a request without the warning too.
    pub(crate) fn answer_approval(
        &self,
        id: u64,
        decision: Decision,
        pin: Option<&Secret>,
        stdin_confirmed: bool,
    ) -> Result<Resposta, ResponseError> {
        let entries = self.aprovacao.pendente(id).ok_or(ResponseError::Expired)?;
        let confirmation = self
            .aprovacao
            .e_confirmacao(id)
            .ok_or(ResponseError::Expired)?;
        let hidden_stdin = self
            .aprovacao
            .e_entrada_oculta(id)
            .ok_or(ResponseError::Expired)?;
        if decision == Decision::Deny {
            return if self.aprovacao.decidir(id, Err(ResponseError::Denied)) {
                Ok(Resposta::Ok)
            } else {
                Err(ResponseError::Expired)
            };
        }
        // Phase 44c: the origin confirmation only accepts "Deny" and "Confirm"; a request
        // that asks for the PIN is never decided by "Confirm" (without PIN).
        if confirmation != (decision == Decision::Confirm) {
            warning!("aprovação {id}: decisão que não cabe no pedido; recusada");
            return Err(ResponseError::InvalidRequest);
        }
        if decision == Decision::Confirm {
            return if self.aprovacao.decidir(id, Ok(())) {
                warning!("aprovação {id}: origem confirmada");
                Ok(Resposta::Ok)
            } else {
                Err(ResponseError::Expired)
            };
        }
        // Phase 54b (D68): the standard input box, before the PIN (nothing counts as a
        // PIN error). The daemon checks; the window does not decide alone (S9).
        if hidden_stdin != stdin_confirmed {
            warning!(
                "aprovação {id}: permitir sem a confirmação da entrada padrão (ou com ela fora do aviso); recusado (D68)"
            );
            return Err(ResponseError::InvalidRequest);
        }
        // The protocol already requires the PIN on every "Allow"; this is the second
        // layer.
        let pin = pin.ok_or(ResponseError::InvalidRequest)?;
        let mut state = self.estado();
        // Phase 49a: with the state held, the request still has to be pending. A restore
        // between the read above and here canceled the request; without this check, the
        // "always allow" would change the same-named entry of the restored version.
        if self.aprovacao.pendente(id).as_ref() != Some(&entries) {
            return Err(ResponseError::Expired);
        }
        if let Err(e) = Self::conferir_pin(&mut state, pin) {
            if state.aberto.is_none() {
                // The 5th error locked (or the vault locked in the middle): the request is
                // denied.
                self.aprovacao.decidir(id, Err(ResponseError::Denied));
            }
            return Err(e);
        }
        match decision {
            Decision::Deny | Decision::AllowOnce | Decision::Confirm => {}
            Decision::AllowOneHour => {
                if !self.aprovacao.decidir(id, Ok(())) {
                    return Err(ResponseError::Expired);
                }
                let until = self.aprovacao.agora().saturating_add(UMA_HORA);
                let a = state.aberto.as_mut().ok_or(ResponseError::Locked)?;
                for n in entries {
                    a.liberacoes.insert(n, until);
                }
                warning!("aprovação {id}: permitida por 1 hora");
                return Ok(Resposta::Ok);
            }
            Decision::AllowAlways => {
                // The request's entries that are "with approval" become "free", on disk.
                let a = state.aberto.as_mut().ok_or(ResponseError::Locked)?;
                let mut changed = Vec::new();
                for e in a.entradas.iter_mut() {
                    if entries.iter().any(|n| n == e.name()) && e.level() == AccessLevel::Approval {
                        e.set_level(AccessLevel::Free);
                        changed.push(e.name().to_owned());
                    }
                }
                for n in &changed {
                    a.liberacoes.remove(n);
                }
                // Failed to write: the level goes back and the request stays pending.
                self.gravar_ou_desfazer(&mut state, move |a| {
                    for e in a.entradas.iter_mut() {
                        if changed.iter().any(|n| n == e.name()) {
                            e.set_level(AccessLevel::Approval);
                        }
                    }
                })?;
                warning!("aprovação {id}: permitida sempre (entrada agora livre)");
            }
        }
        if self.aprovacao.decidir(id, Ok(())) {
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
    fn combined_level_is_the_most_restrictive() {
        use AccessLevel::*;
        for (a, b, r) in [
            (Free, Free, Free),
            (Free, Approval, Approval),
            (Approval, Free, Approval),
            (Free, Blocked, Blocked),
            (Blocked, Approval, Blocked),
            (Approval, Approval, Approval),
        ] {
            assert_eq!(most_restrictive(a, b), r);
        }
    }
}
