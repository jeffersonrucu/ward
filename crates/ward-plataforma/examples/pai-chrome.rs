//! Conferência do pai Chrome com o Chrome real (Fase 40; D5, D47; `make teste-nm`).
//! Sem segredo nenhum.
//!
//! - `pai-chrome servir <socket> <chrome>`: escuta no socket (pasta do teste), aceita
//!   uma conexão e imprime uma linha: `pai=<Ok|motivo> ppid=<n> pai_dumpable=<sim|nao>
//!   cadeia=<a,b,…>`, com [`ProcessoPar::conferir_pai`] contra o binário dado.
//! - Como host de native messaging (o Chrome passa `chrome-extension://…/`): conecta no
//!   socket de `WARD_PAI_CHROME_SOCK`, espera o servidor fechar, responde uma mensagem
//!   `{"ok":true}` ao Chrome e sai. O mesmo vale executado à mão (controle).

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::ExitCode;

use ward_plataforma::{BinarioPermitido, ProcessoPar};

fn servir(sock: &str, chrome: &str) -> ExitCode {
    let _ = std::fs::remove_file(sock);
    let l = match UnixListener::bind(sock) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bind: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("pronto");
    let _ = std::io::stdout().flush();
    let Ok((s, _)) = l.accept() else {
        return ExitCode::FAILURE;
    };
    let par = match ProcessoPar::do_socket(&s) {
        Ok(p) => p,
        Err(e) => {
            println!("pai=SemIdentidade({})", e.kind());
            return ExitCode::SUCCESS;
        }
    };
    let chrome = BinarioPermitido::novo(chrome).expect("caminho do chrome");
    let r = par.conferir_pai(&chrome);
    let ppid = par.pid_do_pai().unwrap_or(0);
    let dump = if std::fs::read_link(format!("/proc/{ppid}/exe")).is_ok() {
        "sim"
    } else {
        "nao"
    };
    let cadeia: Vec<String> = par.cadeia().into_iter().map(|e| e.nome).collect();
    println!(
        "pai={} ppid={ppid} pai_dumpable={dump} cadeia={}",
        match r {
            Ok(()) => "Ok".to_owned(),
            Err(m) => format!("{m:?}"),
        },
        cadeia.join(",")
    );
    drop(s);
    ExitCode::SUCCESS
}

fn sondar() -> ExitCode {
    let Some(sock) = std::env::var_os("WARD_PAI_CHROME_SOCK") else {
        return ExitCode::FAILURE;
    };
    if let Ok(mut s) = UnixStream::connect(sock) {
        let mut b = [0u8; 1];
        let _ = s.read(&mut b);
    }
    let corpo = br#"{"ok":true}"#;
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(&(corpo.len() as u32).to_ne_bytes());
    let _ = out.write_all(corpo);
    let _ = out.flush();
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("servir") => match (args.get(2), args.get(3)) {
            (Some(s), Some(c)) => servir(s, c),
            _ => ExitCode::from(2),
        },
        _ => sondar(),
    }
}
