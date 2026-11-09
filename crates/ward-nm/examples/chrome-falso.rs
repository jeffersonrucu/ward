//! "Chrome" do contêiner de `make teste-docker-plataforma` (Fase 40): instalado como
//! `/opt/google/chrome/chrome` (root, 0755) só dentro do contêiner, faz o que o Chrome
//! faz com um host de native messaging: executa o host como filho direto, com a origem
//! da extensão de argumento, manda as mensagens e lê as respostas.
//!
//! Uso: `chrome-falso <host> <arquivo>`: cada linha do arquivo é uma mensagem JSON; a
//! saída é uma linha por resposta, e no fim `saida=<código>`. Sem segredo próprio: as
//! respostas são o que o daemon de teste mandou.

use std::io::{BufRead, Read, Write};
use std::process::{Command, ExitCode, Stdio};

const ORIGEM: &str = "chrome-extension://dkacdineofgfabdognjgeoakekaenaei/";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let (Some(host), Some(arquivo)) = (args.get(1), args.get(2)) else {
        eprintln!("uso: chrome-falso <host> <arquivo>");
        return ExitCode::from(2);
    };
    let Ok(f) = std::fs::File::open(arquivo) else {
        return ExitCode::from(2);
    };
    let mut entrada = Vec::new();
    for linha in std::io::BufReader::new(f).lines().map_while(Result::ok) {
        if linha.is_empty() {
            continue;
        }
        entrada.extend_from_slice(&(linha.len() as u32).to_ne_bytes());
        entrada.extend_from_slice(linha.as_bytes());
    }
    let Ok(mut filho) = Command::new(host)
        .arg(ORIGEM)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    else {
        println!("spawn falhou");
        return ExitCode::FAILURE;
    };
    if let Some(mut i) = filho.stdin.take() {
        let _ = i.write_all(&entrada);
    }
    let mut saida = Vec::new();
    if let Some(mut o) = filho.stdout.take() {
        let _ = o.read_to_end(&mut saida);
    }
    let status = filho.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
    let mut resto = &saida[..];
    while resto.len() >= 4 {
        let n = u32::from_ne_bytes([resto[0], resto[1], resto[2], resto[3]]) as usize;
        let fim = (4 + n).min(resto.len());
        println!("{}", String::from_utf8_lossy(&resto[4..fim]));
        resto = &resto[fim..];
    }
    println!("saida={status}");
    ExitCode::SUCCESS
}
