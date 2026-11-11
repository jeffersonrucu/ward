//! `wardd`: hardens the process before anything else (V3.7), takes the systemd socket
//! (`LISTEN_FDS`, D46) or, only in the test daemon, creates its own in a 0700 directory
//! of the user (D7), and serves.

#![forbid(unsafe_code)]

use std::process::ExitCode;
use std::sync::Arc;

use ward_platform::{AllowedBinary, Install, Listener, Memory};
use wardd::approval::{Engine, MonotonicClock};
use wardd::config::Config;
use wardd::core::{Daemon, RealClock};
use wardd::launch::{RealOpener, TestApp, TestExtension};
use wardd::server::{self, Roles};
use wardd::{execution, lock, log, udisks, warning};

/// Installed Chrome, accepted parent of the extension host (D5, D47).
const CHROME: &str = "/opt/google/chrome/chrome";

fn listener(config: &Config) -> Result<Listener, String> {
    if std::env::var_os("LISTEN_FDS").is_some() {
        return ward_platform::listen_from_listen_fds(&ward_platform::system_socket_path())
            .map_err(|e| format!("socket do systemd recusado: {e}"));
    }
    match &config.test_socket_dir {
        Some(dir) => {
            let d = ward_platform::prepare_dir(dir)
                .map_err(|e| format!("diretório do socket de teste recusado: {e}"))?;
            d.listen(ward_platform::socket::SOCKET_NAME)
                .map_err(|e| format!("socket de teste recusado: {e}"))
        }
        None => Err("sem o socket do systemd (LISTEN_FDS); o wardd sobe pela unit".into()),
    }
}

/// Only in the test daemon (D63): the opener of `WARD_DEV_APP` and the app role of the
/// process it opens. In the release, neither exists.
#[cfg(feature = "test-mode")]
fn test_opener(config: &Config) -> (Option<RealOpener>, Option<TestApp>) {
    match &config.test_app {
        Some(b) => {
            warning!("daemon de teste: abrir_app abre o app de teste da configuração");
            let a = RealOpener::for_test(b.clone());
            let app = TestApp::new(a.launched_app());
            (Some(a), Some(app))
        }
        None => (None, None),
    }
}

/// Only in the test daemon with `WARD_DEV_EXTENSION=chrome` (Phase 41): the extension
/// role for the child of the installed Chrome, without the egid. In the release, never.
#[cfg(feature = "test-mode")]
fn test_extension(config: &Config) -> Option<TestExtension> {
    if !config.test_extension {
        return None;
    }
    match AllowedBinary::new(CHROME) {
        Ok(c) => {
            warning!(
                "daemon de teste: papel extensão para o filho do Chrome instalado, sem o grupo"
            );
            Some(TestExtension::new(c))
        }
        Err(_) => {
            warning!(
                "daemon de teste: Chrome instalado não achado; papel extensão de teste desligado"
            );
            None
        }
    }
}

#[cfg(not(feature = "test-mode"))]
fn test_extension(_: &Config) -> Option<TestExtension> {
    None
}

#[cfg(not(feature = "test-mode"))]
fn test_opener(_: &Config) -> (Option<RealOpener>, Option<TestApp>) {
    (None, None)
}

fn main() -> ExitCode {
    // V3.7, V3.25: before anything else (including opening a file).
    // D62: the release only goes on with all the memory locked (`mlockall`); `test-mode`
    // with the user's usual limit goes on with a warning.
    let memory = match ward_platform::start_hardened() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("wardd: {e}");
            return ExitCode::FAILURE;
        }
    };
    log::configure();
    // Phase 55 (S8): the orphan of a `ward run`/`ward ssh` (whoever left the group with
    // `setsid -f` or `setpgid`) becomes a child of the daemon, which kills it at the end
    // of the session and on lock. Without this, the daemon does not start.
    if let Err(e) = ward_platform::descendants::become_subreaper() {
        warning!(
            "não deu para virar subreaper (PR_SET_CHILD_SUBREAPER): {}",
            e.kind()
        );
        return ExitCode::FAILURE;
    }
    if let Memory::NotLocked(e) = &memory {
        warning!("daemon de teste com a memória sem travar: {e}");
    }
    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            warning!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let listener = match listener(&config) {
        Ok(e) => e,
        Err(e) => {
            warning!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let install = match AllowedBinary::new(CHROME).map(Install::from_system) {
        Ok(Ok(i)) => Some(i),
        Ok(Err(e)) => {
            warning!("papéis app e extensão desligados: {e}");
            None
        }
        Err(_) => None,
    };
    if cfg!(feature = "test-mode") {
        warning!("daemon de TESTE (test-mode): cofre de .dev/, nunca o real");
    }
    // D57/D63: the app is only opened by the launcher, and only the process it opened is
    // app. In the test daemon with `WARD_DEV_APP`, the launcher opens the test binary,
    // and only it is app (without the egid).
    let (opener, test_app) = test_opener(&config);
    let test_extension = test_extension(&config);
    let (opener, app) = match opener {
        Some(a) => (Some(a), None),
        None => {
            let a = install.as_ref().and_then(RealOpener::installed);
            let app = a.as_ref().map(RealOpener::launched_app);
            (a, app)
        }
    };
    // S2, V4.15: what a dead `wardd` left in `<rt>tmp/` goes away at startup.
    match execution::clean_temp_files(&config.rt) {
        Ok(0) => {}
        Ok(n) => warning!("{n} pasta(s) temporária(s) de antes da partida apagada(s) (S2)"),
        Err(e) => warning!(
            "pasta de temporários do ward run indisponível ({})",
            e.kind()
        ),
    }
    // Phase 28g: SSH agent dirs (`<rt>ssh-*/`) that a dead `wardd` left.
    match wardd::ssh_agent::clean_dirs(&config.rt) {
        Ok(0) => {}
        Ok(n) => warning!("{n} pasta(s) do agente SSH de antes da partida apagada(s)"),
        Err(e) => warning!("pastas do agente SSH indisponíveis ({})", e.kind()),
    }
    // D6 (Phase 44): `ward playwright` storage states and profiles from before startup.
    match wardd::playwright::clean_up(&config.rt) {
        Ok(0) => {}
        Ok(n) => warning!(
            "{n} sessão(ões)/perfil(is) do ward playwright de antes da partida apagado(s) (D6)"
        ),
        Err(e) => warning!("pasta do ward playwright indisponível ({})", e.kind()),
    }
    let mut daemon = Daemon::new(config.paths, Box::new(RealClock)).with_temp_root(config.rt);
    // V4.21: locked and without the app, the agent's request waits for the unlock
    // command (D16).
    let engine = match opener {
        Some(a) => Engine::new(Arc::new(MonotonicClock::new()), Box::new(a)),
        None => Engine::without_app(),
    };
    daemon = daemon.with_engine(engine.waiting_without_app());
    // Phase 24 (logind) and Phase 46 (udisks2): the same system bus.
    let bus = match lock::daemon_bus() {
        Ok(b) => b,
        Err(e) => {
            warning!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let udisks: Option<Arc<udisks::Udisks>> = bus
        .as_ref()
        .map(|s| Arc::new(udisks::Udisks::new(s.clone())));
    if let Some(u) = &udisks {
        daemon = daemon.with_usb_drives(u.clone());
    }
    let daemon = Arc::new(daemon);
    // Phase 24: lock on inactivity and through logind (screen lock, suspend).
    if lock::watch_inactivity(daemon.clone(), lock::INACTIVITY_INTERVAL).is_err() {
        warning!("não deu para criar a thread da trava por inatividade");
        return ExitCode::FAILURE;
    }
    match (bus, udisks) {
        (Some(socket), Some(source)) => {
            if lock::watch_logind(daemon.clone(), socket.clone(), Box::new(|| {})).is_err() {
                warning!("não deu para criar a thread do logind");
                return ExitCode::FAILURE;
            }
            // Phase 46: external backup. The copy only happens with the vault unlocked
            // and the external backup turned on in the vault settings (off by default).
            let Some(pokes) = daemon.enable_external_copy() else {
                warning!("cópia externa já ligada");
                return ExitCode::FAILURE;
            };
            if udisks::watch_copy(daemon.clone(), source, pokes).is_err()
                || udisks::watch_udisks(daemon.clone(), socket, Box::new(|| {})).is_err()
            {
                warning!("não deu para criar as threads do backup externo");
                return ExitCode::FAILURE;
            }
        }
        // Only the test daemon gets here (the release always has the system bus).
        // The message does not name the variable: the `WARD_DEV_*` text only exists with
        // `test-mode`.
        _ => warning!(
            "daemon sem barramento do sistema: logind desligado; udisks2 (backup externo) desligado"
        ),
    }
    warning!("pronto");
    let roles = Roles {
        install,
        app,
        test_app,
        test_extension,
    };
    match server::serve(&listener, daemon, Arc::new(roles)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            warning!("laço do socket parou ({})", e.kind());
            ExitCode::FAILURE
        }
    }
}
