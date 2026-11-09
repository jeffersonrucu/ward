//! `ward-ponte-teste`: sobe a ponte (D7) e imprime a URL que o Chrome de teste abre. Só
//! existe com a feature `modo-teste` (`required-features`). Fala com o daemon de teste
//! em `/run/user/<uid>/ward-teste/` e nunca lê variável para escolher o socket.
//!
//! Com o daemon de verdade (D63, Fase 23a), quem abre a ponte é o daemon de teste
//! (`make app-ponte` pede `abrir_app` com `WARD_DEV_APP` apontando para ela): sem
//! argumentos, com o `dist-teste` do repositório de `WARD_DEV_DIR`, e a saída (com a
//! URL) em `.dev/logs/ward-ponte-teste.log`. Com `--dist <dir>` ela também fala com o
//! socket, mas executada à mão o daemon dá o papel agente e ela sai na primeira
//! resposta ([`ward_ponte_teste::partida`]).
//!
//! `--daemon-falso cheio|vazio|sem-cofre` troca o socket pelo daemon falso do núcleo
//! (`ward_app_nucleo::daemon_falso`, Fase 32), que responde pelo protocolo com as
//! entradas falsas do cofre de teste (ou com nenhuma, ou sem cofre para o primeiro uso), para conferir as telas no Chrome
//! antes do `wardd` da Fase 22.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ward_app_nucleo::area_transferencia::{Copiador, Wayland};
use ward_app_nucleo::daemon::{Daemon, DaemonSocket};
use ward_app_nucleo::daemon_falso::DaemonFalso;
use ward_app_nucleo::servico::Servico;
use ward_ponte_teste::http::Ponte;
use ward_ponte_teste::partida;
use ward_ponte_teste::simulados::{SeletorDePasta, TerminalDeTeste};

const USO: &str = "uso: ward-ponte-teste [--dist <diretório do build --mode teste> \
[--daemon-falso cheio|vazio|sem-cofre]] [--anexos <pasta com um arquivo>]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut args: Vec<&str> = args.iter().map(String::as_str).collect();
    // Fase 34: `--anexos <pasta>` no fim troca o seletor de arquivo (simulados.rs).
    let anexos = match args.as_slice() {
        [.., "--anexos", p] => {
            let p = PathBuf::from(*p);
            args.truncate(args.len() - 2);
            Some(p)
        }
        _ => None,
    };
    match args.as_slice() {
        // Aberta pelo daemon de teste (D63): sem argumentos.
        [] => match partida::dist_do_repositorio(std::env::var_os("WARD_DEV_DIR")) {
            Some(dist) => com_o_daemon(&dist, anexos),
            None => {
                eprintln!(
                    "ward-ponte-teste: WARD_DEV_DIR ausente ou fora da forma <repositório>/.dev"
                );
                ExitCode::FAILURE
            }
        },
        ["--dist", d] => com_o_daemon(Path::new(d), anexos),
        ["--dist", d, "--daemon-falso", "cheio"] => {
            servir(DaemonFalso::cheio(), Path::new(d), anexos)
        }
        ["--dist", d, "--daemon-falso", "vazio"] => {
            servir(DaemonFalso::vazio(), Path::new(d), anexos)
        }
        // Fase 30: sem cofre, para o `Primeiro uso / *`.
        ["--dist", d, "--daemon-falso", "sem-cofre"] => {
            servir(DaemonFalso::sem_cofre(), Path::new(d), anexos)
        }
        _ => {
            eprintln!("{USO}");
            ExitCode::from(2)
        }
    }
}

/// O socket do daemon de teste, com o papel conferido na primeira resposta (D63).
fn com_o_daemon(dist: &Path, anexos: Option<PathBuf>) -> ExitCode {
    let daemon = DaemonSocket::padrao()
        .map_err(|e| e.to_string())
        .and_then(partida::conectar_com_papel_app)
        .and_then(|d| {
            // Sem a conexão a ponte não serve para nada (e nunca reconecta): sai.
            d.vigiar_fechamento(|| {
                eprintln!("ward-ponte-teste: a conexão com o daemon de teste caiu");
                std::process::exit(1)
            })
            .map(|()| d)
            .map_err(|e| e.to_string())
        });
    match daemon {
        Ok(d) => servir(d, dist, anexos),
        Err(e) => {
            eprintln!("ward-ponte-teste: {e}");
            ExitCode::FAILURE
        }
    }
}

fn servir<D: Daemon + 'static>(daemon: D, dist: &Path, anexos: Option<PathBuf>) -> ExitCode {
    let dist: PathBuf = dist.to_path_buf();
    let servico = match Wayland::novo() {
        Ok(a) => {
            let s = Servico::novo(daemon, Copiador::novo(a)).com_terminal(TerminalDeTeste);
            match anexos {
                Some(p) => s.com_seletor(SeletorDePasta(p)),
                None => s,
            }
        }
        Err(e) => {
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
