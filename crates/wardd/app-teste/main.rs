//! `wardd-app-teste`: app de teste mínimo dos testes do `wardd` (D63, Fase 23a). Só
//! existe com a feature `modo-teste` (`required-features`), nunca no release.
//!
//! Conecta ao daemon de teste (`$WARD_DEV_SOCKET_DIR/wardd.sock`, ou
//! `/run/user/<uid>/ward-teste/wardd.sock`), pede `estado` e `revelar` e imprime, uma
//! por linha, só o que o teste confere (sem valor nenhum):
//! - `papel=<agente|app|extensao>`, da primeira resposta;
//! - `revelar=<tipo ou código do erro>`;
//! - `ambiente=<nomes das variáveis, em ordem>`;
//! - `argumentos=<n>`.
//!
//! Aberto pelo `abrir_app` do daemon de teste (com `WARD_DEV_APP` apontando para ele) a
//! saída vai para `.dev/logs/wardd-app-teste.log`; executado à mão, para o terminal.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use ward_core::protocolo::{self, Alvo, Papel, Pedido, Resposta};

fn pedir(s: &mut UnixStream, p: &Pedido) -> Option<Vec<u8>> {
    let corpo = protocolo::codificar_pedido(Papel::App, p).ok()?;
    s.write_all(&u32::try_from(corpo.len()).ok()?.to_be_bytes())
        .and_then(|()| s.write_all(&corpo))
        .ok()?;
    let mut n = [0u8; 4];
    s.read_exact(&mut n).ok()?;
    let tamanho = usize::try_from(u32::from_be_bytes(n)).ok()?;
    let mut corpo = vec![0u8; tamanho.min(1 << 20)];
    s.read_exact(&mut corpo).ok()?;
    Some(corpo)
}

/// O papel vem do `estado`; o resto da leitura é a do agente (aceita tudo que o daemon
/// manda a qualquer papel) para o tipo do `revelar`.
fn descrever(corpo: &[u8]) -> String {
    for papel in [Papel::App, Papel::Agente] {
        match protocolo::decodificar_resposta(papel, corpo) {
            Ok(Resposta::Erro(e)) => return e.codigo().to_owned(),
            Ok(r) => return r.tipo().nome().to_owned(),
            Err(_) => {}
        }
    }
    "invalida".to_owned()
}

fn main() -> ExitCode {
    let mut nomes: Vec<String> = std::env::vars_os()
        .map(|(k, _)| k.to_string_lossy().into_owned())
        .collect();
    nomes.sort();
    println!("ambiente={}", nomes.join(","));
    println!(
        "argumentos={}",
        std::env::args_os().count().saturating_sub(1)
    );
    let dir = std::env::var_os("WARD_DEV_SOCKET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(format!(
                "/run/user/{}/ward-teste",
                ward_plataforma::uid_efetivo()
            ))
        });
    let mut s = match UnixStream::connect(dir.join("wardd.sock")) {
        Ok(s) => s,
        Err(e) => {
            println!("connect={}", e.raw_os_error().unwrap_or(-1));
            return ExitCode::FAILURE;
        }
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(10)));
    let papel = pedir(&mut s, &Pedido::Estado).and_then(|c| {
        Papel::TODOS
            .into_iter()
            .find_map(|p| match protocolo::decodificar_resposta(p, &c) {
                Ok(Resposta::Estado { papel, .. }) => Some(papel),
                _ => None,
            })
    });
    match papel {
        Some(p) => println!("papel={}", p.nome()),
        None => println!("papel=?"),
    }
    let r = pedir(
        &mut s,
        &Pedido::Revelar {
            entrada: "acme-pg-dev".into(),
            alvo: Alvo::Campo("senha".into()),
        },
    );
    println!(
        "revelar={}",
        r.as_deref().map(descrever).unwrap_or_default()
    );
    let _ = std::io::stdout().flush();
    ExitCode::SUCCESS
}
