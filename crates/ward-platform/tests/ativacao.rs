//! Socket entregue pelo systemd (`LISTEN_FDS`, D46; V3.23; S5), no host e sem root.
//!
//! Sem root não há pasta do root para o socket, então aqui só cabem os ataques: um
//! "systemd" do próprio usuário que passa um socket da pasta dele, `LISTEN_PID` de
//! outro processo, `LISTEN_FDS` diferente de 1, caminho esperado diferente e ambiente
//! sem ativação. O caminho feliz (pasta root:root 0755 em `/run/ward/1000/`) roda no
//! contêiner (`make test-docker-platform`).
//!
//! Os dois lados são este binário de teste, chamado de novo com `--exact
//! filho_auxiliar` e `WARD_ATIVACAO` dizendo o que fazer.

#![cfg(feature = "test-mode")]

mod comum;

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::process::{Command, Stdio};

use comum::DirTeste;
use ward_platform::socket::preparar_diretorio;

const MODO: &str = "WARD_ATIVACAO";
const MARCA: &str = "RESULTADO:";

/// Corpo dos processos filhos. Sem a variável, não faz nada.
#[test]
fn filho_auxiliar() {
    let Ok(modo) = std::env::var(MODO) else {
        return;
    };
    let caminho = std::env::var_os("WARD_CAMINHO").expect("WARD_CAMINHO");
    match modo.as_str() {
        // Faz o papel do systemd: cria o socket e se executa de novo como "receber".
        "ativar" => {
            let listener = UnixListener::bind(&caminho).unwrap();
            let mut cmd = filho("receber");
            // No `exec`, o mesmo stdout deste processo (o pipe do teste).
            cmd.env("WARD_CAMINHO", &caminho)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());
            if let Some(esperado) = std::env::var_os("WARD_ESPERADO") {
                cmd.env("WARD_ESPERADO", esperado);
            }
            ward_platform::testing::run_with_socket(&listener, cmd);
        }
        "receber" => {
            let esperado = std::env::var_os("WARD_ESPERADO").unwrap_or(caminho);
            match ward_platform::escutar_de_listen_fds(Path::new(&esperado)) {
                Ok(_) => println!("{MARCA}ok"),
                Err(e) => println!("{MARCA}{e:?}"),
            }
        }
        _ => panic!("modo desconhecido"),
    }
}

fn filho(modo: &str) -> Command {
    let mut c = Command::new(std::env::current_exe().unwrap());
    c.args([
        "--exact",
        "filho_auxiliar",
        "--nocapture",
        "--test-threads=1",
    ])
    .env_clear()
    .env("PATH", "/usr/bin:/bin")
    .env(MODO, modo)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
    c
}

fn resultado(mut c: Command) -> String {
    let mut f = c.spawn().unwrap();
    let saida = BufReader::new(f.stdout.take().unwrap());
    let mut r = String::new();
    for linha in saida.lines() {
        let linha = linha.unwrap();
        // O harness escreve "test filho_auxiliar ... " antes, na mesma linha.
        if let Some(i) = linha.find(MARCA) {
            r = linha[i + MARCA.len()..].to_owned();
        }
    }
    f.wait().unwrap();
    r
}

#[test]
fn ataque_systemd_falso_do_usuario_com_socket_na_pasta_dele() {
    let d = DirTeste::novo();
    let dir = preparar_diretorio(&d.caminho).unwrap();
    let caminho = dir.caminho().join("wardd.sock");
    let mut c = filho("ativar");
    c.env("WARD_CAMINHO", &caminho);
    let r = resultado(c);
    // O socket é de verdade e escuta no caminho certo; a pasta é que não é do root.
    assert!(r.starts_with("PastaNaoConfiavel"), "{r}");
}

#[test]
fn ataque_socket_em_outro_caminho() {
    let d = DirTeste::novo();
    let dir = preparar_diretorio(&d.caminho).unwrap();
    let caminho = dir.caminho().join("wardd.sock");
    let mut c = filho("ativar");
    c.env("WARD_CAMINHO", &caminho)
        .env("WARD_ESPERADO", "/run/ward/1000/wardd.sock");
    assert_eq!(resultado(c), "CaminhoDiferente");
}

#[test]
fn ataque_listen_pid_e_listen_fds_errados() {
    let caminho = "/run/ward/1000/wardd.sock";
    // Sem as variáveis.
    let mut c = filho("receber");
    c.env("WARD_CAMINHO", caminho);
    assert_eq!(resultado(c), "SemAtivacao");
    // LISTEN_PID de outro processo (o fd 3 seria de outro), ou fora do formato.
    for pid in ["1", "+1", "", "abc"] {
        let mut c = filho("receber");
        c.env("WARD_CAMINHO", caminho)
            .env("LISTEN_PID", pid)
            .env("LISTEN_FDS", "1");
        assert_eq!(resultado(c), "PidDiferente", "{pid:?}");
    }
}

#[test]
fn ataque_mais_de_um_fd() {
    // O `sh` põe o próprio pid em LISTEN_PID e se executa como o receptor (mesmo pid).
    let caminho = "/run/ward/1000/wardd.sock";
    let eu = std::env::current_exe().unwrap();
    let mut c = Command::new("/usr/bin/sh");
    c.arg("-c")
        .arg("LISTEN_PID=$$ LISTEN_FDS=2 exec \"$0\" --exact filho_auxiliar --nocapture")
        .arg(&eu)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env(MODO, "receber")
        .env("WARD_CAMINHO", caminho)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    assert_eq!(resultado(c), "QuantidadeDeFds");
}
