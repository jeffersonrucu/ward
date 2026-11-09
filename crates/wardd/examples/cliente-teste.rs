//! Cliente mínimo do contêiner de `make teste-docker-uid` (V3.2, D49): o `ubuntu:24.04`
//! não tem Python. Conecta no socket, manda um pedido sem segredo (`estado` ou `lista`)
//! e imprime uma linha só, para o script conferir:
//!
//! - `connect: <errno>` se o `connect` falhou (ex.: `EACCES`, 13);
//! - `bytes: 0` se o daemon fechou sem mandar nada;
//! - `resposta: <tipo>` (ou `resposta: erro <codigo>`) se respondeu.
//!
//! Uso: `wardd-cliente-teste <socket> <estado|lista|destravar>`. Com `destravar`
//! (`make teste-units`, V3.34), a senha **de teste** vem do stdin (nunca de argv) e vai
//! num `Segredo`, que zera ao sair; nenhum outro segredo passa por aqui.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::ExitCode;
use std::time::Duration;

use ward_core::entrada::Segredo;
use ward_core::protocolo::{self, Papel, Pedido, Resposta};

/// Maior senha lida do stdin.
const MAX_SENHA: u64 = 1024;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let (Some(caminho), Some(op)) = (args.get(1), args.get(2)) else {
        eprintln!("uso: wardd-cliente-teste <socket> <estado|lista|destravar>");
        return ExitCode::from(2);
    };
    let pedido = match op.as_str() {
        "estado" => Pedido::Estado,
        "lista" => Pedido::Lista,
        "destravar" => {
            // Capacidade fixa: sem realocação, sem cópia solta da senha.
            let mut senha = Vec::with_capacity(MAX_SENHA as usize);
            if std::io::stdin()
                .take(MAX_SENHA)
                .read_to_end(&mut senha)
                .is_err()
            {
                return ExitCode::from(2);
            }
            Pedido::Destravar {
                senha: Segredo::de_bytes(senha),
            }
        }
        _ => return ExitCode::from(2),
    };
    let mut s = match UnixStream::connect(caminho) {
        Ok(s) => s,
        Err(e) => {
            println!("connect: {}", e.raw_os_error().unwrap_or(-1));
            return ExitCode::SUCCESS;
        }
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(10)));
    let corpo = protocolo::codificar_pedido(Papel::Agente, &pedido).expect("pedido");
    // Se o daemon já fechou, a escrita falha (EPIPE): conta como fechado sem bytes.
    let _ = s
        .write_all(&(corpo.len() as u32).to_be_bytes())
        .and_then(|()| s.write_all(&corpo));
    // Um quadro só (o daemon deixa a conexão aberta para o próximo pedido).
    let mut n = [0u8; 4];
    if s.read_exact(&mut n).is_err() {
        println!("bytes: 0");
        return ExitCode::SUCCESS;
    }
    let tamanho = u32::from_be_bytes(n) as usize;
    let mut corpo = vec![0u8; tamanho.min(1 << 20)];
    let r = s
        .read_exact(&mut corpo)
        .ok()
        .and_then(|()| protocolo::decodificar_resposta(Papel::Agente, &corpo).ok());
    match r {
        Some(Resposta::Erro(e)) => println!("resposta: erro {}", e.codigo()),
        Some(r) => println!("resposta: {}", r.tipo().nome()),
        None => println!("resposta: invalida"),
    }
    ExitCode::SUCCESS
}
