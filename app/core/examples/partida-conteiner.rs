//! "ward-app" de teste do contêiner da Fase 29a (`make test-docker-platform`; D47,
//! D57; V5.31). Roda a mesma partida do app real (`ward_app_core::partida`), sem GTK
//! (a imagem do contêiner não tem WebKitGTK), e grava um relatório em
//! `/tmp/ward-app-relatorio.txt`, porque o lançador do daemon liga 0–2 a `/dev/null`.
//! Nunca é instalado fora do contêiner.
//!
//! Linhas do relatório (uma por execução, com o pid):
//! - `inicio pid=… gid=… dumpable=… tracer=…`: logo no começo do `main`, antes da
//!   partida (o que o `exec` setgid deixou);
//! - `depois pid=… gid=… dumpable=… tracer=… socket_cloexec=… env=…` ou
//!   `erro pid=… <mensagem>`.
//!
//! Na primeira execução de um pid, espera a conexão cair (o servidor de teste fecha
//! depois de responder) e se executa de novo pela vigia da partida; na segunda, sai.

use std::io::Write;

use ward_app_core::partida;

const RELATORIO: &str = "/tmp/ward-app-relatorio.txt";

fn status(campo: &str) -> String {
    std::fs::read_to_string("/proc/self/status")
        .unwrap_or_default()
        .lines()
        .find_map(|l| {
            l.strip_prefix(campo)
                .map(|v| v.split_whitespace().collect::<Vec<_>>().join(","))
        })
        .unwrap_or_default()
}

fn dumpable() -> String {
    ward_platform::app::valor_dumpable()
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "?".to_owned())
}

/// Todo fd que é socket tem `FD_CLOEXEC`? (lido do `fdinfo`, flag `02000000`).
fn sockets_com_cloexec() -> String {
    let mut sockets = 0;
    let mut todos = true;
    for fd in std::fs::read_dir("/proc/self/fd")
        .into_iter()
        .flatten()
        .flatten()
    {
        let alvo = std::fs::read_link(fd.path()).unwrap_or_default();
        if !alvo.to_string_lossy().starts_with("socket:") {
            continue;
        }
        sockets += 1;
        let info = std::fs::read_to_string(format!(
            "/proc/self/fdinfo/{}",
            fd.file_name().to_string_lossy()
        ))
        .unwrap_or_default();
        let flags = info
            .lines()
            .find_map(|l| l.strip_prefix("flags:"))
            .and_then(|f| u32::from_str_radix(f.trim(), 8).ok())
            .unwrap_or(0);
        todos &= flags & 0o2_000_000 != 0;
    }
    format!("{todos}/{sockets}")
}

/// fds abertos (o último é o do próprio `read_dir`) e para onde 0–2 apontam.
fn fds() -> String {
    let mut v: Vec<u32> = std::fs::read_dir("/proc/self/fd")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_string_lossy().parse().ok())
        .collect();
    v.sort_unstable();
    let std: Vec<String> = (0..3)
        .map(|n| {
            std::fs::read_link(format!("/proc/self/fd/{n}"))
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "-".to_owned())
        })
        .collect();
    let lista: Vec<String> = v.iter().map(u32::to_string).collect();
    format!("fds={} std={}", lista.join(","), std.join(","))
}

fn relatar(linha: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(RELATORIO)
    {
        let _ = writeln!(f, "{linha}");
    }
}

fn execucoes_deste_pid(pid: u32) -> usize {
    let prefixo = format!("inicio pid={pid} ");
    std::fs::read_to_string(RELATORIO)
        .unwrap_or_default()
        .lines()
        .filter(|l| l.starts_with(&prefixo))
        .count()
}

fn main() {
    let pid = std::process::id();
    let fds = fds();
    relatar(&format!(
        "inicio pid={pid} gid={} dumpable={} tracer={} {fds}",
        status("Gid:"),
        dumpable(),
        status("TracerPid:")
    ));
    let mut p = match partida::iniciar() {
        Ok(p) => p,
        Err(e) => {
            relatar(&format!("erro pid={pid} {e}"));
            std::process::exit(1);
        }
    };
    let mut nomes: Vec<String> = std::env::vars_os()
        .map(|(k, _)| k.to_string_lossy().into_owned())
        .collect();
    nomes.sort();
    relatar(&format!(
        "depois pid={pid} gid={} dumpable={} tracer={} socket_cloexec={} vmlck={} pai={} env={}",
        status("Gid:"),
        dumpable(),
        status("TracerPid:"),
        sockets_com_cloexec(),
        status("VmLck:"),
        p.pid_do_pai(),
        nomes.join(",")
    ));
    let _daemon = p.tomar_daemon();
    if execucoes_deste_pid(pid) < 2 {
        let _ = p
            .vigiar(move || relatar(&format!("queda pid={pid}")))
            .join();
    }
}
