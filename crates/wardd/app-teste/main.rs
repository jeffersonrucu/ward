//! `wardd-app-teste`: minimal test app of the `wardd` tests (D63, Phase 23a). It only
//! exists with the `test-mode` feature (`required-features`), never in the release.
//!
//! Connects to the test daemon (`$WARD_DEV_SOCKET_DIR/wardd.sock`, or the test socket
//! under `/run/user/<uid>/`), asks for the status and for a reveal and prints, one
//! `key=value` per line, only what the test checks (no value at all; the keys are the
//! literals below):
//! - the role, from the first response;
//! - the reveal result (kind or error code);
//! - the environment (names of the variables, in order);
//! - the number of arguments;
//! - D65 b and c (Phase 22c, V3.39 and V3.40), only when the role is app, against the
//!   test vault of `crates/ward-core/src/body/test_vault.rs` (the test writes the same):
//!   reveal of the database entry and of the blocked one, and copy of the database entry
//!   (equal when the value matches the entry's, different, or the kind/code), TOTP
//!   (equal when it matches the local code of the same period), a missing field (the
//!   code), the list (summary when every item has a summary and no secret, or the
//!   reason), and at last lock and a reveal while locked. No value goes out: only the
//!   comparison;
//! - the end line.
//!
//! Opened by the open-app request of the test daemon (with `WARD_DEV_APP` pointing to it)
//! the output goes to the test app log under `.dev/logs/`; run by hand, to the terminal.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use ward_core::entry::Entry;
use ward_core::protocol::{self, Request, Response, Role, Target};
use ward_core::totp::Totp;

// The test vault (fake values only) through the public API, like the app's fake daemon.
use ward_core as wc;
#[path = "../../ward-core/src/body/test_vault.rs"]
mod test_vault;

fn ask(s: &mut UnixStream, p: &Request) -> Option<Vec<u8>> {
    let body = protocol::encode_request(Role::App, p).ok()?;
    s.write_all(&u32::try_from(body.len()).ok()?.to_be_bytes())
        .and_then(|()| s.write_all(&body))
        .ok()?;
    let mut n = [0u8; 4];
    s.read_exact(&mut n).ok()?;
    let size = usize::try_from(u32::from_be_bytes(n)).ok()?;
    let mut body = vec![0u8; size.min(1 << 20)];
    s.read_exact(&mut body).ok()?;
    Some(body)
}

/// The role comes from the status; the rest of the reading is the agent's (it accepts
/// everything the daemon sends to any role) for the kind of the reveal.
fn describe(body: &[u8]) -> String {
    for role in [Role::App, Role::Agent] {
        match protocol::decode_response(role, body) {
            Ok(Response::Error(e)) => return e.code().to_owned(),
            Ok(r) => return r.kind().name().to_owned(),
            Err(_) => {}
        }
    }
    "invalida".to_owned()
}

/// The secret of the test vault entry (to compare, never to print).
fn expected<'a>(entries: &'a [Entry], name: &str, target: &Target) -> Option<&'a [u8]> {
    let e = entries.iter().find(|e| e.name() == name)?;
    target.secret_in(e).map(|s| s.expose())
}

/// Equal/different for a secret; the kind or the error code for the rest.
fn compare(body: Option<&[u8]>, expected: Option<&[u8]>) -> String {
    let Some(body) = body else {
        return String::new();
    };
    match protocol::decode_response(Role::App, body) {
        Ok(Response::Secret(s)) if Some(s.expose()) == expected => "igual".to_owned(),
        Ok(Response::Secret(_)) => "diferente".to_owned(),
        _ => describe(body),
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}

/// D65 b and c against the test vault: what the app gets, compared without printing.
fn app_values(s: &mut UnixStream) {
    let entries = test_vault::test_vault_entries();
    let password = || Target::Field("password".into());
    for (label, reveal, name) in [
        ("revelar_pg", true, "acme-pg-dev"),
        ("revelar_bloqueada", true, "gama-bloqueada"),
        ("copiar_pg", false, "acme-pg-dev"),
    ] {
        let p = if reveal {
            Request::Reveal {
                entry: name.into(),
                target: password(),
            }
        } else {
            Request::Copy {
                entry: name.into(),
                target: password(),
            }
        };
        let r = ask(s, &p);
        println!(
            "{label}={}",
            compare(r.as_deref(), expected(&entries, name, &password()))
        );
    }
    let seed = expected(
        &entries,
        "acme-totp-dev",
        &Target::Field("totp_seed".into()),
    )
    .map(|v| wc::entry::Secret::from_bytes(v.to_vec()));
    let local = || {
        seed.as_ref()
            .and_then(|s| Totp::from_seed(s).ok())
            .and_then(|t| t.code_now().ok())
            .map(|c| c.code().to_owned())
    };
    let before = local();
    let r = ask(
        s,
        &Request::Totp {
            entry: "acme-totp-dev".into(),
        },
    );
    let after = local();
    let totp = match r
        .as_deref()
        .map(|c| protocol::decode_response(Role::App, c))
    {
        Some(Ok(Response::TotpCode { code, remaining })) => {
            let matches = [&before, &after]
                .iter()
                .any(|l| l.as_deref() == Some(code.as_str()));
            if matches && (1..=30).contains(&remaining) {
                "igual".to_owned()
            } else {
                "diferente".to_owned()
            }
        }
        _ => r.as_deref().map(describe).unwrap_or_default(),
    };
    println!("totp={totp}");
    let r = ask(
        s,
        &Request::Reveal {
            entry: "acme-pg-dev".into(),
            target: Target::Field("private_key".into()),
        },
    );
    println!(
        "campo_inexistente={}",
        r.as_deref().map(describe).unwrap_or_default()
    );
    let r = ask(s, &Request::List);
    let list = match r
        .as_deref()
        .map(|c| (c, protocol::decode_response(Role::App, c)))
    {
        Some((c, Ok(Response::List(items)))) => {
            if items.len() != entries.len() {
                "contagem".to_owned()
            } else if !items
                .iter()
                .all(|i| i.summary.is_some() && i.level.is_some())
            {
                "sem_resumo".to_owned()
            } else if test_vault::SECRETS
                .iter()
                .any(|v| contains(c, v.as_bytes()))
            {
                "com_segredo".to_owned()
            } else {
                "resumo".to_owned()
            }
        }
        _ => r.as_deref().map(describe).unwrap_or_default(),
    };
    println!("lista={list}");
    let r = ask(s, &Request::Lock);
    println!("travar={}", r.as_deref().map(describe).unwrap_or_default());
    let r = ask(
        s,
        &Request::Reveal {
            entry: "acme-pg-dev".into(),
            target: password(),
        },
    );
    println!(
        "revelar_travado={}",
        r.as_deref().map(describe).unwrap_or_default()
    );
}

fn main() -> ExitCode {
    let mut names: Vec<String> = std::env::vars_os()
        .map(|(k, _)| k.to_string_lossy().into_owned())
        .collect();
    names.sort();
    println!("ambiente={}", names.join(","));
    println!(
        "argumentos={}",
        std::env::args_os().count().saturating_sub(1)
    );
    let dir = std::env::var_os("WARD_DEV_SOCKET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(format!(
                "/run/user/{}/ward-teste",
                ward_platform::effective_uid()
            ))
        });
    let mut s = match UnixStream::connect(dir.join("wardd.sock")) {
        Ok(s) => s,
        Err(e) => {
            println!("connect={}", e.raw_os_error().unwrap_or(-1));
            return ExitCode::FAILURE;
        }
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(10)));
    let role = ask(&mut s, &Request::Status).and_then(|c| {
        Role::ALL
            .into_iter()
            .find_map(|p| match protocol::decode_response(p, &c) {
                Ok(Response::Status { role, .. }) => Some(role),
                _ => None,
            })
    });
    match role {
        Some(p) => println!("papel={}", p.name()),
        None => println!("papel=?"),
    }
    let r = ask(
        &mut s,
        &Request::Reveal {
            entry: "acme-pg-dev".into(),
            target: Target::Field("password".into()),
        },
    );
    println!("revelar={}", r.as_deref().map(describe).unwrap_or_default());
    // Only the test app opened by the daemon goes past here: run by hand (agent), a lock
    // would touch the daemon of whoever is testing.
    if role == Some(Role::App) {
        app_values(&mut s);
    }
    println!("fim=1");
    let _ = std::io::stdout().flush();
    ExitCode::SUCCESS
}
