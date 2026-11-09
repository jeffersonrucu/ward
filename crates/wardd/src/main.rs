//! `wardd`: endurece o processo antes de tudo (V3.7), pega o socket do systemd
//! (`LISTEN_FDS`, D46) ou, só no daemon de teste, cria o seu num diretório 0700 do
//! usuário (D7), e atende.

#![forbid(unsafe_code)]

use std::process::ExitCode;
use std::sync::Arc;

use ward_plataforma::{BinarioPermitido, Escuta, Instalacao};
use wardd::config::Config;
use wardd::nucleo::{Daemon, RelogioReal};
use wardd::servidor::{self, Papeis};
use wardd::{aviso, log};

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

fn main() -> ExitCode {
    // V3.7, V3.25: antes de qualquer outra coisa (inclusive de abrir arquivo).
    if let Err(e) = ward_plataforma::iniciar_endurecido() {
        eprintln!("wardd: {e}");
        return ExitCode::FAILURE;
    }
    log::configurar();
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
    let daemon = Arc::new(Daemon::novo(config.caminhos, Box::new(RelogioReal)));
    aviso!("pronto");
    match servidor::servir(&escuta, daemon, Arc::new(Papeis { instalacao })) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            aviso!("laço do socket parou ({})", e.kind());
            ExitCode::FAILURE
        }
    }
}
