//! `ward-test-bridge`: sobe a ponte (D7) e imprime a URL que o Chrome de teste abre. Só
//! existe com a feature `test-mode` (`required-features`). Fala com o daemon de teste
//! em `/run/user/<uid>/ward-teste/`, ou na pasta de `WARD_DEV_SOCKET_DIR` que o daemon
//! de teste lhe passa (Fase 39c; `DaemonSocket::padrao`).
//!
//! Com o daemon de verdade (D63, Fase 23a), quem abre a ponte é o daemon de teste
//! (`make app-bridge` pede `abrir_app` com `WARD_DEV_APP` apontando para ela): sem
//! argumentos, com o `dist-teste` do repositório de `WARD_DEV_DIR`, e a saída (com a
//! URL) em `.dev/logs/ward-test-bridge.log`. Com `--dist <dir>` ela também fala com o
//! socket, mas executada à mão o daemon dá o papel agente e ela sai na primeira
//! resposta ([`ward_test_bridge::partida`]).
//!
//! `--daemon-falso cheio|vazio|travado|travado-com-pedido|travado-com-pedido-ataque|sem-cofre` troca o socket pelo
//! daemon falso do núcleo (`ward_app_core::daemon_falso`, Fase 32), que responde pelo
//! protocolo com as entradas falsas do cofre de teste (ou com nenhuma, travado para o
//! destravar, ou sem cofre para o primeiro uso), para conferir as telas no Chrome
//! antes do `wardd` da Fase 22. Fase 48: `backup-externo-ligado` (o cheio com o backup
//! externo ligado no pendrive falso `WARD-BACKUP`) e `sem-udisks` (o cheio com o udisks2
//! sem resposta); os pendrives são só dados (`pendrives_de_teste`), nada é montado.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use std::sync::Arc;
use std::time::Duration;
use ward_app_core::area_transferencia::Copiador;

use ward_app_core::daemon::{Aviso, Daemon, DaemonSocket, FilaAvisos};
use ward_app_core::daemon_falso::{AprovacaoSimulada, DaemonFalso, aprovacoes_simuladas};
use ward_app_core::servico::Servico;
use ward_app_core::terminal::Terminal;
use ward_app_core::{aprovacao, destrave};
use ward_test_bridge::area_wl::WlCopy;
use ward_test_bridge::http::Ponte;
use ward_test_bridge::partida;
use ward_test_bridge::simulados::{SeletorDePasta, TerminalDeTeste, TerminalDoRepositorio};

const USO: &str = "uso: ward-ponte-teste [--dist <diretório do build --mode teste> \
[--daemon-falso cheio|vazio|travado|travado-com-pedido|travado-com-pedido-ataque|sem-cofre|aprovacao|aprovacao-prod|\
aprovacao-expirando|aprovacao-ataque|aprovacao-origem|aprovacao-origem-popup|aprovacao-origem-pin|aprovacao-entrada|backup-externo-ligado|sem-udisks|sem-backups|\
backups-indisponiveis|backup-plantado|backup-local-inseguro]] [--anexos <pasta com um arquivo>]";

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
            servir(DaemonFalso::cheio(), None, Path::new(d), anexos, &[])
        }
        ["--dist", d, "--daemon-falso", "vazio"] => {
            servir(DaemonFalso::vazio(), None, Path::new(d), anexos, &[])
        }
        // Fase 31: `Destravar / *` com o falso travado; com `-com-pedido`, um
        // `pedido_de_destrave` chega 3 s depois de subir, como se um `ward run` esperasse.
        ["--dist", d, "--daemon-falso", "travado"] => {
            servir(DaemonFalso::travado(), None, Path::new(d), anexos, &[])
        }
        // Fase 39a: o aviso traz entrada e comando; `-ataque` manda os dois crus, com
        // controle, bidi, largura zero e HTML (D51).
        [
            "--dist",
            d,
            "--daemon-falso",
            modo @ ("travado-com-pedido" | "travado-com-pedido-ataque"),
        ] => {
            let aviso = if *modo == "travado-com-pedido" {
                Aviso::destrave_de_teste("acme-pg-aprovacao")
            } else {
                Aviso::destrave_hostil_de_teste()
            };
            let fila = Arc::new(FilaAvisos::default());
            let f = Arc::clone(&fila);
            let _ = std::thread::Builder::new().spawn(move || {
                std::thread::sleep(Duration::from_secs(3));
                f.simular(aviso);
            });
            servir(
                DaemonFalso::travado(),
                Some(fila),
                Path::new(d),
                anexos,
                &[],
            )
        }
        // Fase 48: `Configurações / Backup externo ligado` e o udisks2 sem resposta.
        ["--dist", d, "--daemon-falso", "backup-externo-ligado"] => servir(
            DaemonFalso::backup_externo_ligado(),
            None,
            Path::new(d),
            anexos,
            &[],
        ),
        ["--dist", d, "--daemon-falso", "sem-udisks"] => servir(
            DaemonFalso::backup_externo_ligado().com_pendrives(None),
            None,
            Path::new(d),
            anexos,
            &[],
        ),
        // Fase 47: restaurar sem nenhuma versão e com a pasta de backups indisponível.
        ["--dist", d, "--daemon-falso", "sem-backups"] => servir(
            DaemonFalso::cheio().com_backups(Some(Vec::new())),
            None,
            Path::new(d),
            anexos,
            &[],
        ),
        ["--dist", d, "--daemon-falso", "backups-indisponiveis"] => servir(
            DaemonFalso::cheio().com_backups(None),
            None,
            Path::new(d),
            anexos,
            &[],
        ),
        // Fase 49a: cópia antiga plantada com data nova e pasta do backup local insegura.
        ["--dist", d, "--daemon-falso", "backup-plantado"] => servir(
            DaemonFalso::backup_plantado(),
            None,
            Path::new(d),
            anexos,
            &[],
        ),
        ["--dist", d, "--daemon-falso", "backup-local-inseguro"] => servir(
            DaemonFalso::backup_local_inseguro(),
            None,
            Path::new(d),
            anexos,
            &[],
        ),
        // Fase 30: sem cofre, para o `Primeiro uso / *`.
        ["--dist", d, "--daemon-falso", "sem-cofre"] => {
            servir(DaemonFalso::sem_cofre(), None, Path::new(d), anexos, &[])
        }
        // Fase 36: cofre cheio com um pedido de aprovação esperando a janela
        // (`#token=…&tela=aprovacao`).
        ["--dist", d, "--daemon-falso", modo] if modo.starts_with("aprovacao") => {
            match aprovacoes_simuladas(modo) {
                Some(pedidos) => {
                    let mut falso = DaemonFalso::cheio();
                    for (p, _) in &pedidos {
                        falso = falso.com_aprovacao(p);
                    }
                    servir(falso, None, Path::new(d), anexos, &pedidos)
                }
                None => {
                    eprintln!("{USO}");
                    ExitCode::from(2)
                }
            }
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
        Ok(d) => {
            let fila = d.avisos();
            // Fase 39c: com o daemon de teste, o "Conectar" abre o `ward ssh` de verdade.
            let terminal: Box<dyn Terminal> = match TerminalDoRepositorio::do_repositorio(
                std::env::var_os("WARD_DEV_DIR"),
                std::env::var_os("WARD_DEV_SOCKET_DIR"),
            ) {
                Some(t) => Box::new(t),
                None => {
                    eprintln!(
                        "ward-ponte-teste: sem dev/tests/terminal-test.py ou target/debug/ward; o Conectar só registra"
                    );
                    Box::new(TerminalDeTeste)
                }
            };
            servir_com(d, Some(fila), dist, anexos, &[], terminal)
        }
        Err(e) => {
            eprintln!("ward-ponte-teste: {e}");
            ExitCode::FAILURE
        }
    }
}

fn servir<D: Daemon + 'static>(
    daemon: D,
    avisos: Option<Arc<FilaAvisos>>,
    dist: &Path,
    anexos: Option<PathBuf>,
    aprovacoes: &[AprovacaoSimulada],
) -> ExitCode {
    servir_com(
        daemon,
        avisos,
        dist,
        anexos,
        aprovacoes,
        Box::new(TerminalDeTeste),
    )
}

fn servir_com<D: Daemon + 'static>(
    daemon: D,
    avisos: Option<Arc<FilaAvisos>>,
    dist: &Path,
    anexos: Option<PathBuf>,
    aprovacoes: &[AprovacaoSimulada],
    terminal: Box<dyn Terminal>,
) -> ExitCode {
    let dist: PathBuf = dist.to_path_buf();
    let servico = match WlCopy::novo() {
        Ok(a) => {
            let s = Servico::novo(daemon, Copiador::novo(a)).com_terminal(terminal);
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
    for (p, decorrido) in aprovacoes {
        servico.aprovacoes().chegou_ha(p.clone(), *decorrido);
    }
    // Fase 31 (D65 a): os `pedido_de_destrave` da conexão marcam o pedido que a tela lê
    // no `estado_do_cofre` (a ponte não tem o canal do Tauri; a tela consulta).
    // Fase 36: os `pedido_de_aprovacao` da mesma fila vão para a janela de aprovação
    // (`&tela=aprovacao`), que consulta `pedido_de_aprovacao` a cada segundo.
    if let Some(fila) = avisos
        && (aprovacao::vigiar(Arc::clone(&fila), servico.aprovacoes(), || {}).is_err()
            || destrave::vigiar(fila, servico.pedido_de_destrave(), || {}).is_err())
    {
        eprintln!("ward-ponte-teste: não foi possível vigiar os avisos do daemon");
        return ExitCode::FAILURE;
    }
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
