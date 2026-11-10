//! `ward-nm`: host de native messaging da extensão (Fase 40). Ver a documentação do
//! crate (`lib.rs`).
//!
//! Uso (pelo Chrome): `ward-nm chrome-extension://<id>/`. Sai 0 quando o Chrome fecha
//! o stdin; ≠ 0 com argumento fora da forma, mensagem ruim ou falha no stdout.

#![forbid(unsafe_code)]

use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::UnixStream;
use std::process::ExitCode;
use std::time::Duration;

use zeroize::Zeroizing;

use ward_core::limites::MAX_QUADRO_COMUM;
use ward_core::protocolo::quadro::{self, LIMITE_RESPOSTA};
use ward_nm::{Daemon, FalhaDaemon};
use ward_platform::{ExpectedServer, connect_server};

/// Prazo para o daemon responder (o mesmo da CLI).
const PRAZO_LEITURA: Duration = Duration::from_secs(130);
const PRAZO_ESCRITA: Duration = Duration::from_secs(10);

/// O servidor que este host aceita. Release: só o `wardd` instalado (D46), sem
/// variável nem argumento que troque. `test-mode`: o daemon de teste.
fn servidor_esperado() -> ExpectedServer {
    #[cfg(feature = "test-mode")]
    {
        if let Some(d) = std::env::var_os("WARD_DEV_SOCKET_DIR") {
            let d = std::path::PathBuf::from(d);
            return ExpectedServer::test_at(d.join(ward_platform::socket::SOCKET_NAME));
        }
        ExpectedServer::test()
    }
    #[cfg(not(feature = "test-mode"))]
    {
        ExpectedServer::system()
    }
}

/// Uma conexão nova por mensagem, conferida antes do primeiro byte (D46). O host não
/// fica com conexão parada no daemon.
struct DaemonSocket {
    esperado: ExpectedServer,
}

impl DaemonSocket {
    fn conectar(&self) -> Result<UnixStream, FalhaDaemon> {
        let s = connect_server(&self.esperado).map_err(|_| FalhaDaemon::Indisponivel)?;
        s.set_read_timeout(Some(PRAZO_LEITURA))
            .and_then(|()| s.set_write_timeout(Some(PRAZO_ESCRITA)))
            .map_err(|_| FalhaDaemon::Conversa)?;
        conferir_extra(&s)?;
        Ok(s)
    }
}

/// `test-mode`: sem o uid 0 (o daemon de teste é do usuário), o processo do socket
/// tem de ser não-dumpable, como o `wardd` (a mesma regra da CLI de teste).
#[cfg(feature = "test-mode")]
fn conferir_extra(s: &UnixStream) -> Result<(), FalhaDaemon> {
    let par = ward_platform::PeerProcess::from_socket(s).map_err(|_| FalhaDaemon::Indisponivel)?;
    match std::fs::read_link(format!("/proc/{}/exe", par.credentials().pid)) {
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {}
        _ => return Err(FalhaDaemon::Indisponivel),
    }
    par.reverify_no_debugger()
        .map_err(|_| FalhaDaemon::Indisponivel)
}

#[cfg(not(feature = "test-mode"))]
fn conferir_extra(_: &UnixStream) -> Result<(), FalhaDaemon> {
    Ok(())
}

impl Daemon for DaemonSocket {
    fn trocar(&mut self, pedido: &[u8]) -> Result<Zeroizing<Vec<u8>>, FalhaDaemon> {
        let mut s = self.conectar()?;
        quadro::escrever_quadro(&mut s, pedido, MAX_QUADRO_COMUM)
            .map_err(|_| FalhaDaemon::Conversa)?;
        let r = quadro::ler_quadro(&mut s, LIMITE_RESPOSTA).map_err(|_| FalhaDaemon::Conversa)?;
        let _ = s.shutdown(std::net::Shutdown::Both);
        Ok(r)
    }
}

fn main() -> ExitCode {
    // S8: sem core e sem ptrace do mesmo uid (o exec setgid já deixou o processo
    // não-dumpable; aqui fica 0 também no build sem setgid).
    if ward_platform::harden_process().is_err() {
        eprintln!("ward-nm: não deu para endurecer o processo");
        return ExitCode::FAILURE;
    }
    // O Chrome passa só a origem da extensão.
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match args.as_slice() {
        [origem] if ward_nm::origem_da_extensao_valida(origem.as_bytes()) => {}
        _ => {
            eprintln!("ward-nm: uso: ward-nm chrome-extension://<id>/ (aberto pelo Chrome)");
            return ExitCode::from(2);
        }
    }
    let mut daemon = DaemonSocket {
        esperado: servidor_esperado(),
    };
    let mut log = |m: &str| eprintln!("ward-nm: {m}");
    let r = ward_nm::servir(
        &mut io::stdin().lock(),
        &mut io::stdout().lock(),
        &mut daemon,
        &mut log,
    );
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
