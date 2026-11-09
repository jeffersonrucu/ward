//! `ward-ponte-teste --dist app/dist-teste`: sobe a ponte (D7) e imprime a URL que o
//! Chrome de teste abre. Só existe com a feature `modo-teste` (`required-features`).
//! Fala com o daemon de teste em `/run/user/<uid>/ward-teste/` e nunca lê
//! `WARD_DEV_DIR` nem outra variável para escolher o caminho.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

use ward_app_nucleo::area_transferencia::{Copiador, Wayland};
use ward_app_nucleo::daemon::DaemonSocket;
use ward_app_nucleo::servico::Servico;
use ward_ponte_teste::http::Ponte;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let dist = match (args.next().as_deref(), args.next(), args.next()) {
        (Some("--dist"), Some(d), None) => PathBuf::from(d),
        _ => {
            eprintln!("uso: ward-ponte-teste --dist <diretório do build --mode teste>");
            return ExitCode::from(2);
        }
    };
    let servico = match (DaemonSocket::padrao(), Wayland::novo()) {
        (Ok(d), Ok(a)) => Servico::novo(d, Copiador::novo(a)),
        (Err(e), _) | (_, Err(e)) => {
            eprintln!("ward-ponte-teste: {e}");
            return ExitCode::FAILURE;
        }
    };
    let (ponte, ouvinte) = match Ponte::abrir(servico, &dist) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("ward-ponte-teste: não foi possível abrir a ponte (dist sem index.html?)");
            return ExitCode::FAILURE;
        }
    };
    // A URL com o token vai só para a saída padrão de quem subiu a ponte.
    println!("{}", ponte.url_de_abertura());
    ponte.servir(ouvinte);
    ExitCode::SUCCESS
}
