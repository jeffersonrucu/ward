//! `wardd`: endurece o processo antes de tudo (V3.7), pega o socket do systemd
//! (`LISTEN_FDS`, D46) ou, só no daemon de teste, cria o seu num diretório 0700 do
//! usuário (D7), e atende.

#![forbid(unsafe_code)]

use std::process::ExitCode;
use std::sync::Arc;

use ward_platform::{AllowedBinary, Install, Listener, Memory};
use wardd::aprovacao::{Motor, RelogioMonotonico};
use wardd::config::Config;
use wardd::lancamento::{AbridorReal, AppDeTeste, ExtensaoDeTeste};
use wardd::nucleo::{Daemon, RelogioReal};
use wardd::servidor::{self, Papeis};
use wardd::{aviso, execution, log, trava, udisks};

/// Chrome instalado, pai aceito do host da extensão (D5, D47).
const CHROME: &str = "/opt/google/chrome/chrome";

fn escuta(config: &Config) -> Result<Listener, String> {
    if std::env::var_os("LISTEN_FDS").is_some() {
        return ward_platform::listen_from_listen_fds(&ward_platform::system_socket_path())
            .map_err(|e| format!("socket do systemd recusado: {e}"));
    }
    match &config.diretorio_socket_teste {
        Some(dir) => {
            let d = ward_platform::prepare_dir(dir)
                .map_err(|e| format!("diretório do socket de teste recusado: {e}"))?;
            d.listen(ward_platform::socket::SOCKET_NAME)
                .map_err(|e| format!("socket de teste recusado: {e}"))
        }
        None => Err("sem o socket do systemd (LISTEN_FDS); o wardd sobe pela unit".into()),
    }
}

/// Só no daemon de teste (D63): o abridor do `WARD_DEV_APP` e o papel app do processo
/// que ele abrir. No release, nenhum dos dois existe.
#[cfg(feature = "test-mode")]
fn abridor_de_teste(config: &Config) -> (Option<AbridorReal>, Option<AppDeTeste>) {
    match &config.app_de_teste {
        Some(b) => {
            aviso!("daemon de teste: abrir_app abre o app de teste da configuração");
            let a = AbridorReal::de_teste(b.clone());
            let app = AppDeTeste::novo(a.app_lancado());
            (Some(a), Some(app))
        }
        None => (None, None),
    }
}

/// Só no daemon de teste com `WARD_DEV_EXTENSAO=chrome` (Fase 41): o papel extensão
/// para o filho do Chrome instalado, sem o egid. No release, nunca.
#[cfg(feature = "test-mode")]
fn extensao_de_teste(config: &Config) -> Option<ExtensaoDeTeste> {
    if !config.extensao_de_teste {
        return None;
    }
    match AllowedBinary::new(CHROME) {
        Ok(c) => {
            aviso!("daemon de teste: papel extensão para o filho do Chrome instalado, sem o grupo");
            Some(ExtensaoDeTeste::novo(c))
        }
        Err(_) => {
            aviso!(
                "daemon de teste: Chrome instalado não achado; papel extensão de teste desligado"
            );
            None
        }
    }
}

#[cfg(not(feature = "test-mode"))]
fn extensao_de_teste(_: &Config) -> Option<ExtensaoDeTeste> {
    None
}

#[cfg(not(feature = "test-mode"))]
fn abridor_de_teste(_: &Config) -> (Option<AbridorReal>, Option<AppDeTeste>) {
    (None, None)
}

fn main() -> ExitCode {
    // V3.7, V3.25: antes de qualquer outra coisa (inclusive de abrir arquivo).
    // D62: o release só segue com toda a memória travada (`mlockall`); o `test-mode`
    // com o limite comum do usuário segue com aviso.
    let memoria = match ward_platform::start_hardened() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("wardd: {e}");
            return ExitCode::FAILURE;
        }
    };
    log::configurar();
    // Fase 55 (S8): o órfão de um `ward run`/`ward ssh` (quem saiu do grupo com
    // `setsid -f` ou `setpgid`) vira filho do daemon, que o mata no fim da sessão e na
    // trava. Sem isso, o daemon não sobe.
    if let Err(e) = ward_platform::descendants::become_subreaper() {
        aviso!(
            "não deu para virar subreaper (PR_SET_CHILD_SUBREAPER): {}",
            e.kind()
        );
        return ExitCode::FAILURE;
    }
    if let Memory::NotLocked(e) = &memoria {
        aviso!("daemon de teste com a memória sem travar: {e}");
    }
    let config = match Config::do_ambiente() {
        Ok(c) => c,
        Err(e) => {
            aviso!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let escuta = match escuta(&config) {
        Ok(e) => e,
        Err(e) => {
            aviso!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let instalacao = match AllowedBinary::new(CHROME).map(Install::from_system) {
        Ok(Ok(i)) => Some(i),
        Ok(Err(e)) => {
            aviso!("papéis app e extensão desligados: {e}");
            None
        }
        Err(_) => None,
    };
    if cfg!(feature = "test-mode") {
        aviso!("daemon de TESTE (test-mode): cofre de .dev/, nunca o real");
    }
    // D57/D63: o app só é aberto pelo lançador, e só o processo que ele abriu é app.
    // No daemon de teste com `WARD_DEV_APP`, o lançador abre o binário de teste, e só
    // ele é app (sem o egid).
    let (abridor, app_de_teste) = abridor_de_teste(&config);
    let extensao_de_teste = extensao_de_teste(&config);
    let (abridor, app) = match abridor {
        Some(a) => (Some(a), None),
        None => {
            let a = instalacao.as_ref().and_then(AbridorReal::instalado);
            let app = a.as_ref().map(AbridorReal::app_lancado);
            (a, app)
        }
    };
    // S2, V4.15: o que um `wardd` morto deixou em `<rt>tmp/` some na partida.
    match execution::clean_temp_files(&config.rt) {
        Ok(0) => {}
        Ok(n) => aviso!("{n} pasta(s) temporária(s) de antes da partida apagada(s) (S2)"),
        Err(e) => aviso!(
            "pasta de temporários do ward run indisponível ({})",
            e.kind()
        ),
    }
    // Fase 28g: pastas do agente SSH (`<rt>ssh-*/`) que um `wardd` morto deixou.
    match wardd::ssh_agent::clean_dirs(&config.rt) {
        Ok(0) => {}
        Ok(n) => aviso!("{n} pasta(s) do agente SSH de antes da partida apagada(s)"),
        Err(e) => aviso!("pastas do agente SSH indisponíveis ({})", e.kind()),
    }
    // D6 (Fase 44): storage states e perfis do `ward playwright` de antes da partida.
    match wardd::playwright::limpar(&config.rt) {
        Ok(0) => {}
        Ok(n) => aviso!(
            "{n} sessão(ões)/perfil(is) do ward playwright de antes da partida apagado(s) (D6)"
        ),
        Err(e) => aviso!("pasta do ward playwright indisponível ({})", e.kind()),
    }
    let mut daemon =
        Daemon::novo(config.caminhos, Box::new(RelogioReal)).com_temporarios(config.rt);
    // V4.21: travado e sem app, o pedido do agente espera o `ward destravar` (D16).
    let motor = match abridor {
        Some(a) => Motor::novo(Arc::new(RelogioMonotonico::novo()), Box::new(a)),
        None => Motor::sem_app(),
    };
    daemon = daemon.com_motor(motor.esperando_sem_app());
    // Fase 24 (logind) e Fase 46 (udisks2): o mesmo barramento do sistema.
    let barramento = match trava::barramento_do_daemon() {
        Ok(b) => b,
        Err(e) => {
            aviso!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let udisks: Option<Arc<udisks::Udisks>> = barramento
        .as_ref()
        .map(|s| Arc::new(udisks::Udisks::novo(s.clone())));
    if let Some(u) = &udisks {
        daemon = daemon.com_pendrives(u.clone());
    }
    let daemon = Arc::new(daemon);
    // Fase 24: trava por inatividade e pelo logind (bloqueio de tela, suspensão).
    if trava::vigiar_inatividade(daemon.clone(), trava::INTERVALO_INATIVIDADE).is_err() {
        aviso!("não deu para criar a thread da trava por inatividade");
        return ExitCode::FAILURE;
    }
    match (barramento, udisks) {
        (Some(socket), Some(fonte)) => {
            if trava::vigiar_logind(daemon.clone(), socket.clone(), Box::new(|| {})).is_err() {
                aviso!("não deu para criar a thread do logind");
                return ExitCode::FAILURE;
            }
            // Fase 46: backup externo. A cópia só acontece com o cofre destravado e o
            // externo ligado na configuração do cofre (desligado por padrão).
            let Some(cutucoes) = daemon.ligar_copia_externa() else {
                aviso!("cópia externa já ligada");
                return ExitCode::FAILURE;
            };
            if udisks::vigiar_copia(daemon.clone(), fonte, cutucoes).is_err()
                || udisks::vigiar_udisks(daemon.clone(), socket, Box::new(|| {})).is_err()
            {
                aviso!("não deu para criar as threads do backup externo");
                return ExitCode::FAILURE;
            }
        }
        // Só o daemon de teste chega aqui (o release sempre tem o barramento do sistema).
        // A mensagem não cita a variável: o texto `WARD_DEV_*` só existe com `test-mode`.
        _ => aviso!(
            "daemon sem barramento do sistema: logind desligado; udisks2 (backup externo) desligado"
        ),
    }
    aviso!("pronto");
    let papeis = Papeis {
        instalacao,
        app,
        app_de_teste,
        extensao_de_teste,
    };
    match servidor::servir(&escuta, daemon, Arc::new(papeis)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            aviso!("laço do socket parou ({})", e.kind());
            ExitCode::FAILURE
        }
    }
}
