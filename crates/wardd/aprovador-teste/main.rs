//! `wardd-aprovador-teste`: test approval window of the M6 end-to-end suites (Phase 44c;
//! D8, D9, D63). It only exists with the `test-mode` feature (`required-features`), never
//! in the release.
//!
//! The test daemon opens it through its open-app request when a request needs the window
//! (with `WARD_DEV_APP` pointing to a copy of it inside the repository): it gets the app
//! role (D63), connects to the test socket (`$WARD_DEV_SOCKET_DIR/wardd.sock`, or the
//! test socket under `/run/user/<uid>/`) and answers each approval request with what the
//! suite wrote in the mode file under `$WARD_DEV_DIR` (read on every request): confirm,
//! deny, allow once with the PIN of the test vault, the same with the standard input box
//! checked (Phase 54b), or nothing (it does not answer: the request expires, or the
//! client gives up). The mode words are the literals of [`mode`].
//!
//! It prints, one line per event, only what the suite checks (no value of the vault: the
//! approval request has none): the role, the request (operation, entries, origin,
//! confirmation, the hidden standard input flag when set, and the decision) and the
//! response (kind or code). The output goes to `$WARD_DEV_DIR/logs/<binary>.log` (the
//! launcher of the test daemon). Run by hand, it gets the agent role, the daemon refuses
//! the answer and it exits.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::ExitCode;

use ward_core::entry::Secret;
use ward_core::protocol::{self, Decision, Request, Response, Role};

fn socket() -> Option<PathBuf> {
    match std::env::var_os("WARD_DEV_SOCKET_DIR") {
        Some(d) => Some(PathBuf::from(d).join("wardd.sock")),
        None => Some(PathBuf::from(format!(
            "/run/user/{}/ward-teste/wardd.sock",
            ward_platform::effective_uid()
        ))),
    }
}

fn send(s: &mut UnixStream, p: &Request) -> bool {
    let Ok(body) = protocol::encode_request(Role::App, p) else {
        return false;
    };
    let Ok(n) = u32::try_from(body.len()) else {
        return false;
    };
    s.write_all(&n.to_be_bytes())
        .and_then(|()| s.write_all(&body))
        .is_ok()
}

fn read(s: &mut UnixStream) -> Option<Vec<u8>> {
    let mut n = [0u8; 4];
    s.read_exact(&mut n).ok()?;
    let size = usize::try_from(u32::from_be_bytes(n)).ok()?;
    let mut body = vec![0u8; size.min(1 << 20)];
    s.read_exact(&mut body).ok()?;
    Some(body)
}

/// The decision of the mode file (the suite switches between one case and another):
/// decision, PIN and the standard input box (Phase 54b).
fn mode() -> Option<(Decision, Option<Secret>, bool)> {
    let dev = PathBuf::from(std::env::var_os("WARD_DEV_DIR")?);
    let text = std::fs::read_to_string(dev.join("aprovador.modo")).ok()?;
    let line = text.lines().next()?.trim();
    match line.split_once(' ') {
        Some((m @ ("uma_vez" | "uma_vez_ciente"), pin)) => Some((
            Decision::AllowOnce,
            Some(Secret::from_bytes(pin.trim().as_bytes().to_vec())),
            m == "uma_vez_ciente",
        )),
        _ => match line {
            "confirmar" => Some((Decision::Confirm, None, false)),
            "negar" => Some((Decision::Deny, None, false)),
            _ => None,
        },
    }
}

fn main() -> ExitCode {
    let Some(path) = socket() else {
        return ExitCode::FAILURE;
    };
    let Ok(mut s) = UnixStream::connect(&path) else {
        println!("erro=conectar");
        return ExitCode::FAILURE;
    };
    if !send(&mut s, &Request::Status) {
        return ExitCode::FAILURE;
    }
    while let Some(body) = read(&mut s) {
        match protocol::decode_response(Role::App, &body) {
            Ok(Response::Status { role, .. }) => println!("papel={}", role.name()),
            Ok(Response::ApprovalRequest(p)) => {
                let choice = mode();
                println!(
                    "pedido operacao={} entradas={} origem={} confirmacao={}{} decisao={}",
                    p.operation.name(),
                    p.entries.join(","),
                    if p.origin.is_empty() { "-" } else { &p.origin },
                    u8::from(p.confirmation),
                    if p.hidden_entry {
                        " entrada_oculta=1"
                    } else {
                        ""
                    },
                    choice.as_ref().map_or("nada", |(d, _, _)| d.name()),
                );
                if let Some((decision, pin, confirmed_entry)) = choice {
                    let request = Request::AnswerApproval {
                        request: p.id,
                        decision,
                        pin,
                        confirmed_entry,
                    };
                    if !send(&mut s, &request) {
                        break;
                    }
                }
            }
            Ok(Response::UnlockRequest(_)) => println!("destrave"),
            Ok(Response::Error(e)) => println!("resposta={}", e.code()),
            Ok(r) => println!("resposta={}", r.kind().name()),
            Err(_) => println!("resposta=invalida"),
        }
        let _ = std::io::stdout().flush();
    }
    println!("fim=1");
    ExitCode::SUCCESS
}
