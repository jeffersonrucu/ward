//! `ward-test-bridge`: starts the bridge (D7) and prints the URL the test Chrome opens.
//! It only exists with the `test-mode` feature (`required-features`). It talks to the test
//! daemon in its socket folder under `/run/user/<uid>/`, or in the folder of
//! `WARD_DEV_SOCKET_DIR` that the test daemon hands it (Phase 39c; the default
//! `DaemonSocket`).
//!
//! With the real daemon (D63, Phase 23a), the bridge is opened by the test daemon
//! (`make app-bridge` asks for the open-app operation with `WARD_DEV_APP` pointing to it):
//! without arguments, with the test-mode build of the repository of `WARD_DEV_DIR`, and
//! the output (with the URL) in `.dev/logs/ward-test-bridge.log`. With `--dist <dir>` it
//! also talks to the socket, but run by hand the daemon gives it the agent role and it
//! exits on the first reply ([`ward_test_bridge::startup`]).
//!
//! The fake-daemon option (modes in [`USAGE`]) swaps the socket for the fake daemon of
//! the app core (Phase 32), which answers through the protocol with the fake entries of
//! the test vault (or with none, locked for the unlock screen, or without a vault for the
//! first use), to check the screens in Chrome before the `wardd` of Phase 22. Phase 48:
//! the mode with the external backup enabled on the fake USB drive `WARD-BACKUP`, and the
//! one with udisks2 not answering; the USB drives are only data, nothing is mounted.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use std::sync::Arc;
use std::time::Duration;
use ward_app_core::area_transferencia::Copiador;

use ward_app_core::daemon::{Daemon, DaemonSocket, Notice, NoticeQueue};
use ward_app_core::fake_daemon::{FakeDaemon, SimulatedApproval, simulated_approvals};
use ward_app_core::service::Service;
use ward_app_core::terminal::Terminal;
use ward_app_core::{approval, destrave};
use ward_test_bridge::http::Bridge;
use ward_test_bridge::simulated::{DirPicker, RepoTerminal, TestTerminal};
use ward_test_bridge::startup;
use ward_test_bridge::wl_clipboard::WlCopy;

const USAGE: &str = "uso: ward-ponte-teste [--dist <diretório do build --mode teste> \
[--daemon-falso cheio|vazio|travado|travado-com-pedido|travado-com-pedido-ataque|sem-cofre|aprovacao|aprovacao-prod|\
aprovacao-expirando|aprovacao-ataque|aprovacao-origem|aprovacao-origem-popup|aprovacao-origem-pin|aprovacao-entrada|backup-externo-ligado|sem-udisks|sem-backups|\
backups-indisponiveis|backup-plantado|backup-local-inseguro]] [--anexos <pasta com um arquivo>]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut args: Vec<&str> = args.iter().map(String::as_str).collect();
    // Phase 34: the attachments option (with a folder) at the end swaps the file picker
    // (simulated.rs).
    let attachments = match args.as_slice() {
        [.., "--anexos", p] => {
            let p = PathBuf::from(*p);
            args.truncate(args.len() - 2);
            Some(p)
        }
        _ => None,
    };
    match args.as_slice() {
        // Opened by the test daemon (D63): no arguments.
        [] => match startup::repository_dist(std::env::var_os("WARD_DEV_DIR")) {
            Some(dist) => with_daemon(&dist, attachments),
            None => {
                eprintln!(
                    "ward-ponte-teste: WARD_DEV_DIR ausente ou fora da forma <repositório>/.dev"
                );
                ExitCode::FAILURE
            }
        },
        ["--dist", d] => with_daemon(Path::new(d), attachments),
        ["--dist", d, "--daemon-falso", "cheio"] => {
            serve(FakeDaemon::full(), None, Path::new(d), attachments, &[])
        }
        ["--dist", d, "--daemon-falso", "vazio"] => {
            serve(FakeDaemon::empty(), None, Path::new(d), attachments, &[])
        }
        // Phase 31: the unlock screens with the fake one locked; with the request mode, an
        // unlock request arrives 3 s after starting, as if a `ward run` were waiting.
        ["--dist", d, "--daemon-falso", "travado"] => {
            serve(FakeDaemon::locked(), None, Path::new(d), attachments, &[])
        }
        // Phase 39a: the notice carries entry and command; the attack mode sends both raw,
        // with control, bidi, zero-width and HTML (D51).
        [
            "--dist",
            d,
            "--daemon-falso",
            mode @ ("travado-com-pedido" | "travado-com-pedido-ataque"),
        ] => {
            let notice = if *mode == "travado-com-pedido" {
                Notice::test_unlock("acme-pg-aprovacao")
            } else {
                Notice::hostile_test_unlock()
            };
            let queue = Arc::new(NoticeQueue::default());
            let q = Arc::clone(&queue);
            let _ = std::thread::Builder::new().spawn(move || {
                std::thread::sleep(Duration::from_secs(3));
                q.simulate(notice);
            });
            serve(
                FakeDaemon::locked(),
                Some(queue),
                Path::new(d),
                attachments,
                &[],
            )
        }
        // Phase 48: the settings screen with the external backup enabled, and udisks2 not
        // answering.
        ["--dist", d, "--daemon-falso", "backup-externo-ligado"] => serve(
            FakeDaemon::external_backup_enabled(),
            None,
            Path::new(d),
            attachments,
            &[],
        ),
        ["--dist", d, "--daemon-falso", "sem-udisks"] => serve(
            FakeDaemon::external_backup_enabled().with_usb_drives(None),
            None,
            Path::new(d),
            attachments,
            &[],
        ),
        // Phase 47: restore with no version and with the backup folder unavailable.
        ["--dist", d, "--daemon-falso", "sem-backups"] => serve(
            FakeDaemon::full().with_backups(Some(Vec::new())),
            None,
            Path::new(d),
            attachments,
            &[],
        ),
        ["--dist", d, "--daemon-falso", "backups-indisponiveis"] => serve(
            FakeDaemon::full().with_backups(None),
            None,
            Path::new(d),
            attachments,
            &[],
        ),
        // Phase 49a: old copy planted with a new date, and an unsafe local backup folder.
        ["--dist", d, "--daemon-falso", "backup-plantado"] => serve(
            FakeDaemon::planted_backup(),
            None,
            Path::new(d),
            attachments,
            &[],
        ),
        ["--dist", d, "--daemon-falso", "backup-local-inseguro"] => serve(
            FakeDaemon::insecure_local_backup(),
            None,
            Path::new(d),
            attachments,
            &[],
        ),
        // Phase 30: no vault, for the first-use screens.
        ["--dist", d, "--daemon-falso", "sem-cofre"] => {
            serve(FakeDaemon::no_vault(), None, Path::new(d), attachments, &[])
        }
        // Phase 36: full vault with an approval request waiting for the window (the
        // approval screen of the opening URL).
        ["--dist", d, "--daemon-falso", mode] if mode.starts_with("aprovacao") => {
            match simulated_approvals(mode) {
                Some(requests) => {
                    let mut fake = FakeDaemon::full();
                    for (p, _) in &requests {
                        fake = fake.with_approval(p);
                    }
                    serve(fake, None, Path::new(d), attachments, &requests)
                }
                None => {
                    eprintln!("{USAGE}");
                    ExitCode::from(2)
                }
            }
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// The socket of the test daemon, with the role checked on the first reply (D63).
fn with_daemon(dist: &Path, attachments: Option<PathBuf>) -> ExitCode {
    let daemon = DaemonSocket::standard()
        .map_err(|e| e.to_string())
        .and_then(startup::connect_with_app_role)
        .and_then(|d| {
            // Without the connection the bridge is useless (and never reconnects): exit.
            d.watch_close(|| {
                eprintln!("ward-ponte-teste: a conexão com o daemon de teste caiu");
                std::process::exit(1)
            })
            .map(|()| d)
            .map_err(|e| e.to_string())
        });
    match daemon {
        Ok(d) => {
            let queue = d.notices();
            // Phase 39c: with the test daemon, "Connect" opens the real `ward ssh`.
            let terminal: Box<dyn Terminal> = match RepoTerminal::from_repository(
                std::env::var_os("WARD_DEV_DIR"),
                std::env::var_os("WARD_DEV_SOCKET_DIR"),
            ) {
                Some(t) => Box::new(t),
                None => {
                    eprintln!(
                        "ward-ponte-teste: sem dev/tests/terminal-test.py ou target/debug/ward; o Conectar só registra"
                    );
                    Box::new(TestTerminal)
                }
            };
            serve_with(d, Some(queue), dist, attachments, &[], terminal)
        }
        Err(e) => {
            eprintln!("ward-ponte-teste: {e}");
            ExitCode::FAILURE
        }
    }
}

fn serve<D: Daemon + 'static>(
    daemon: D,
    notices: Option<Arc<NoticeQueue>>,
    dist: &Path,
    attachments: Option<PathBuf>,
    approvals: &[SimulatedApproval],
) -> ExitCode {
    serve_with(
        daemon,
        notices,
        dist,
        attachments,
        approvals,
        Box::new(TestTerminal),
    )
}

fn serve_with<D: Daemon + 'static>(
    daemon: D,
    notices: Option<Arc<NoticeQueue>>,
    dist: &Path,
    attachments: Option<PathBuf>,
    approvals: &[SimulatedApproval],
    terminal: Box<dyn Terminal>,
) -> ExitCode {
    let dist: PathBuf = dist.to_path_buf();
    let service = match WlCopy::new() {
        Ok(a) => {
            let s = Service::new(daemon, Copiador::novo(a)).with_terminal(terminal);
            match attachments {
                Some(p) => s.with_picker(DirPicker(p)),
                None => s,
            }
        }
        Err(e) => {
            eprintln!("ward-ponte-teste: {e}");
            return ExitCode::FAILURE;
        }
    };
    for (p, elapsed) in approvals {
        service.approvals().arrived_ago(p.clone(), *elapsed);
    }
    // Phase 31 (D65 a): the unlock requests of the connection mark the request that the
    // screen reads in the vault status command (the bridge has no Tauri channel; the
    // screen polls). Phase 36: the approval requests of the same queue go to the approval
    // window, which polls its command every second.
    if let Some(queue) = notices
        && (approval::watch(Arc::clone(&queue), service.approvals(), || {}).is_err()
            || destrave::vigiar(queue, service.unlock_request(), || {}).is_err())
    {
        eprintln!("ward-ponte-teste: não foi possível vigiar os avisos do daemon");
        return ExitCode::FAILURE;
    }
    let (bridge, listener) = match Bridge::open(service, &dist) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("ward-ponte-teste: não foi possível abrir a ponte (dist sem index.html?)");
            return ExitCode::FAILURE;
        }
    };
    // The URL with the token goes only to the stdout of whoever started the bridge.
    println!("{}", bridge.opening_url());
    bridge.serve(listener);
    ExitCode::SUCCESS
}
