//! `wardd`: endurece o processo antes de tudo (V3.7), pega o socket do systemd
//! (`LISTEN_FDS`, D46) ou, só no daemon de teste, cria o seu num diretório 0700 do
//! usuário (D7), e atende.

#![forbid(unsafe_code)]

use std::process::ExitCode;
use std::sync::Arc;

use ward_plataforma::{BinarioPermitido, Escuta, Instalacao, Memoria};
use wardd::aprovacao::{Motor, RelogioMonotonico};
use wardd::config::Config;
use wardd::lancamento::{AbridorReal, AppDeTeste};
use wardd::nucleo::{Daemon, RelogioReal};
use wardd::servidor::{self, Papeis};
use wardd::{aviso, execucao, log, trava};

/// Chrome instalado, pai aceito do host da extensão (D5, D47).
const CHROME: &str = "/opt/google/chrome/chrome";

fn escuta(config: &Config) -> Result<Escuta, String> {
    if std::env::var_os("LISTEN_FDS").is_some() {
        return ward_plataforma::escutar_de_listen_fds(&ward_plataforma::caminho_socket_sistema())
            .map_err(|e| format!("socket do systemd recusado: {e}"));
    }
    match &config.diretorio_socket_teste {
        Some(dir) => {
            let d = ward_plataforma::preparar_diretorio(dir)
                .map_err(|e| format!("diretório do socket de teste recusado: {e}"))?;
            d.escutar(ward_plataforma::socket::NOME_SOCKET)
                .map_err(|e| format!("socket de teste recusado: {e}"))
        }
        None => Err("sem o socket do systemd (LISTEN_FDS); o wardd sobe pela unit".into()),
    }
}

/// Só no daemon de teste (D63): o abridor do `WARD_DEV_APP` e o papel app do processo
/// que ele abrir. No release, nenhum dos dois existe.
#[cfg(feature = "modo-teste")]
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

#[cfg(not(feature = "modo-teste"))]
fn abridor_de_teste(_: &Config) -> (Option<AbridorReal>, Option<AppDeTeste>) {
    (None, None)
}

fn main() -> ExitCode {
    // V3.7, V3.25: antes de qualquer outra coisa (inclusive de abrir arquivo).
    // D62: o release só segue com toda a memória travada (`mlockall`); o `modo-teste`
    // com o limite comum do usuário segue com aviso.
    let memoria = match ward_plataforma::iniciar_endurecido() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("wardd: {e}");
            return ExitCode::FAILURE;
        }
    };
    log::configurar();
    if let Memoria::SemTravar(e) = &memoria {
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
    let instalacao = match BinarioPermitido::novo(CHROME).map(Instalacao::do_sistema) {
        Ok(Ok(i)) => Some(i),
        Ok(Err(e)) => {
            aviso!("papéis app e extensão desligados: {e}");
            None
        }
        Err(_) => None,
    };
    if cfg!(feature = "modo-teste") {
        aviso!("daemon de TESTE (modo-teste): cofre de .dev/, nunca o real");
    }
    // D57/D63: o app só é aberto pelo lançador, e só o processo que ele abriu é app.
    // No daemon de teste com `WARD_DEV_APP`, o lançador abre o binário de teste, e só
    // ele é app (sem o egid).
    let (abridor, app_de_teste) = abridor_de_teste(&config);
    let (abridor, app) = match abridor {
        Some(a) => (Some(a), None),
        None => {
            let a = instalacao.as_ref().and_then(AbridorReal::instalado);
            let app = a.as_ref().map(AbridorReal::app_lancado);
            (a, app)
        }
    };
    // S2, V4.15: o que um `wardd` morto deixou em `<rt>tmp/` some na partida.
    match execucao::limpar_temporarios(&config.rt) {
        Ok(0) => {}
        Ok(n) => aviso!("{n} pasta(s) temporária(s) de antes da partida apagada(s) (S2)"),
        Err(e) => aviso!(
            "pasta de temporários do ward run indisponível ({})",
            e.kind()
        ),
    }
    let mut daemon =
        Daemon::novo(config.caminhos, Box::new(RelogioReal)).com_temporarios(config.rt);
    // V4.21: travado e sem app, o pedido do agente espera o `ward destravar` (D16).
    let motor = match abridor {
        Some(a) => Motor::novo(Arc::new(RelogioMonotonico::novo()), Box::new(a)),
        None => Motor::sem_app(),
    };
    daemon = daemon.com_motor(motor.esperando_sem_app());
    let daemon = Arc::new(daemon);
    // Fase 24: trava por inatividade e pelo logind (bloqueio de tela, suspensão).
    if trava::vigiar_inatividade(daemon.clone(), trava::INTERVALO_INATIVIDADE).is_err() {
        aviso!("não deu para criar a thread da trava por inatividade");
        return ExitCode::FAILURE;
    }
    match trava::barramento_do_daemon() {
        Ok(Some(socket)) => {
            if trava::vigiar_logind(daemon.clone(), socket, Box::new(|| {})).is_err() {
                aviso!("não deu para criar a thread do logind");
                return ExitCode::FAILURE;
            }
        }
        Ok(None) => aviso!("daemon de teste sem WARD_DEV_DBUS_SISTEMA: logind desligado"),
        Err(e) => {
            aviso!("{e}");
            return ExitCode::FAILURE;
        }
    }
    aviso!("pronto");
    let papeis = Papeis {
        instalacao,
        app,
        app_de_teste,
    };
    match servidor::servir(&escuta, daemon, Arc::new(papeis)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            aviso!("laço do socket parou ({})", e.kind());
            ExitCode::FAILURE
        }
    }
}
