//! Minimal client of the container of `make test-docker-uid` (V3.2, D49): the
//! `ubuntu:24.04` image has no Python. Connects to the socket, sends a request without a
//! secret (status or list) and prints a single line, for the script to verify:
//!
//! - `connect: <errno>` if the `connect` failed (e.g. `EACCES`, 13);
//! - `bytes: 0` if the daemon closed without sending anything;
//! - the reply line with the reply type (or the error code) if it answered; the status
//!   reply also carries the role the daemon gave to the connection (V4.38).
//!
//! Usage: `wardd-test-client <socket> <operation>`, with the status, list or unlock
//! operation. With unlock (`make test-units`, V3.34), the **test** password comes from
//! stdin (never from argv) and goes into the core's zeroing secret type, which is zeroed
//! on exit; no other secret goes through here. The operation names are the names of the
//! protocol operations (frozen until m2:Phase 51).

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::ExitCode;
use std::time::Duration;

use ward_core::entry::Secret;
use ward_core::protocolo::{self, Papel, Pedido, Resposta};

/// Largest password read from stdin.
const MAX_PASSWORD: u64 = 1024;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let (Some(path), Some(op)) = (args.get(1), args.get(2)) else {
        eprintln!("uso: wardd-test-client <socket> <estado|lista|destravar>");
        return ExitCode::from(2);
    };
    let request = match op.as_str() {
        "estado" => Pedido::Estado,
        "lista" => Pedido::Lista,
        "destravar" => {
            // Fixed capacity: no reallocation, no stray copy of the password.
            let mut password = Vec::with_capacity(MAX_PASSWORD as usize);
            if std::io::stdin()
                .take(MAX_PASSWORD)
                .read_to_end(&mut password)
                .is_err()
            {
                return ExitCode::from(2);
            }
            Pedido::Destravar {
                senha: Secret::from_bytes(password),
            }
        }
        _ => return ExitCode::from(2),
    };
    let mut s = match UnixStream::connect(path) {
        Ok(s) => s,
        Err(e) => {
            println!("connect: {}", e.raw_os_error().unwrap_or(-1));
            return ExitCode::SUCCESS;
        }
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(10)));
    let body = protocolo::codificar_pedido(Papel::Agente, &request).expect("pedido");
    // If the daemon already closed, the write fails (EPIPE): it counts as closed without
    // bytes.
    let _ = s
        .write_all(&(body.len() as u32).to_be_bytes())
        .and_then(|()| s.write_all(&body));
    // A single frame (the daemon keeps the connection open for the next request).
    let mut n = [0u8; 4];
    if s.read_exact(&mut n).is_err() {
        println!("bytes: 0");
        return ExitCode::SUCCESS;
    }
    let size = u32::from_be_bytes(n) as usize;
    let mut body = vec![0u8; size.min(1 << 20)];
    let r = s
        .read_exact(&mut body)
        .ok()
        .and_then(|()| protocolo::decodificar_resposta(Papel::Agente, &body).ok());
    match r {
        Some(Resposta::Erro(e)) => println!("resposta: erro {}", e.codigo()),
        Some(Resposta::Estado { papel, .. }) => println!("resposta: estado papel={}", papel.nome()),
        Some(r) => println!("resposta: {}", r.tipo().nome()),
        None => println!("resposta: invalida"),
    }
    ExitCode::SUCCESS
}
