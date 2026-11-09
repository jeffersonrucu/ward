//! D63 (Fase 23a), pelo binário: o daemon de teste com `WARD_DEV_APP` abre o app de
//! teste pelo `abrir_app` e só esse processo, filho dele, recebe o papel app (sem o
//! egid do grupo); o mesmo binário executado à mão, uma cópia dele e um cliente
//! qualquer ficam agente; `abrir_app` com campo no pedido não abre nada; o ambiente do
//! app é o fixo mínimo; a saída vai para `.dev/logs/<binário>.log` (0600) e o log do
//! daemon registra o código de saída, só o número (V3.36, V3.37, V3.38).
//!
//! O "repositório" é uma pasta só deste teste em `target/tmp`, com `.dev/` dentro e o
//! app de teste ligado por link físico (o mesmo arquivo, sem fd de escrita). Nenhum
//! cofre: os pedidos não precisam de um.

#![cfg(feature = "modo-teste")]

mod comum;

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use comum::Dir;
use ward_core::protocolo::{self, ErroResposta, Papel, Resposta};

const WARDD: &str = env!("CARGO_BIN_EXE_wardd");
const APP_TESTE: &str = env!("CARGO_BIN_EXE_wardd-app-teste");

struct DirSocket(PathBuf);

impl DirSocket {
    fn novo(rotulo: &str) -> DirSocket {
        let p = PathBuf::from(format!(
            "/run/user/{}/wardd-app-{}-{rotulo}",
            ward_plataforma::uid_efetivo(),
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        DirSocket(p)
    }
}

impl Drop for DirSocket {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Wardd(Child);

impl Drop for Wardd {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn mandar(s: &mut UnixStream, json: &str) {
    s.write_all(&(json.len() as u32).to_be_bytes())
        .and_then(|()| s.write_all(json.as_bytes()))
        .unwrap();
}

fn pedir(s: &mut UnixStream, op: &str, dados: &str) -> Resposta {
    mandar(s, &format!(r#"{{"v":1,"op":"{op}","dados":{dados}}}"#));
    let mut n = [0u8; 4];
    s.read_exact(&mut n).unwrap();
    let mut b = vec![0u8; u32::from_be_bytes(n) as usize];
    s.read_exact(&mut b).unwrap();
    protocolo::decodificar_resposta(Papel::Agente, &b).unwrap()
}

fn esperar(mut f: impl FnMut() -> bool, o_que: &str) {
    let fim = Instant::now() + Duration::from_secs(20);
    while !f() {
        assert!(Instant::now() < fim, "esperando {o_que}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn ler(p: &Path) -> String {
    fs::read_to_string(p).unwrap_or_default()
}

/// O app de teste executado à mão (filho deste teste, não do daemon). Uma cópia recém
/// escrita pode dar ETXTBSY se outro teste fizer `fork` nesse instante: tenta de novo.
fn a_mao(binario: &Path, sock: &Path) -> String {
    for _ in 0..50 {
        match Command::new(binario)
            .env_clear()
            .env("WARD_DEV_SOCKET_DIR", sock)
            .stdin(Stdio::null())
            .output()
        {
            Ok(o) => return String::from_utf8(o.stdout).unwrap(),
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => panic!("{e}"),
        }
    }
    panic!("ETXTBSY o tempo todo");
}

#[test]
fn v3_37_so_o_app_aberto_pelo_daemon_de_teste_e_app() {
    let repo = Dir::novo("repo-app");
    let dev = repo.0.join(".dev");
    fs::DirBuilder::new().mode(0o700).create(&dev).unwrap();
    fs::DirBuilder::new()
        .mode(0o700)
        .create(repo.0.join("bin"))
        .unwrap();
    let app = repo.0.join("bin/wardd-app-teste");
    fs::hard_link(APP_TESTE, &app).unwrap();
    let sock = DirSocket::novo("v337");
    let log = dev.join("wardd.log");

    // Fora do repositório, o daemon nem sobe.
    let st = Command::new(WARDD)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("WARD_DEV_DIR", &dev)
        .env("WARD_DEV_SOCKET_DIR", &sock.0)
        .env("WARD_DEV_APP", "/usr/bin/true")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(!st.success(), "WARD_DEV_APP fora do repositório aceito");

    let _w = Wardd(
        Command::new(WARDD)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C.UTF-8")
            .env("WARD_DEV_DIR", &dev)
            .env("WARD_DEV_SOCKET_DIR", &sock.0)
            .env("WARD_DEV_APP", &app)
            .env("RUST_LOG", "trace")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(fs::File::create(&log).unwrap())
            .spawn()
            .unwrap(),
    );
    let socket = sock.0.join("wardd.sock");
    esperar(|| socket.exists(), "o socket do daemon");
    let mut c = UnixStream::connect(&socket).unwrap();
    c.set_read_timeout(Some(Duration::from_secs(30))).unwrap();

    // Um cliente qualquer (este teste): agente.
    assert!(matches!(
        pedir(&mut c, "estado", "{}"),
        Resposta::Estado {
            papel: Papel::Agente,
            ..
        }
    ));
    // `abrir_app` com caminho, argumentos ou ambiente no pedido: recusado, nada abre.
    let saida_app = dev.join("logs/wardd-app-teste.log");
    for dados in [
        r#"{"caminho":"/usr/bin/sh"}"#,
        r#"{"argumentos":["-c","id"]}"#,
        r#"{"ambiente":{"LD_PRELOAD":"/tmp/x.so"}}"#,
    ] {
        let r = pedir(&mut c, "abrir_app", dados);
        assert!(matches!(r, Resposta::Erro(_)), "{dados}: {r:?}");
    }
    std::thread::sleep(Duration::from_millis(300));
    assert!(!saida_app.exists(), "abrir_app com campo abriu o app");

    // Sem campo: abre o `WARD_DEV_APP`, que recebe o papel app.
    assert!(matches!(pedir(&mut c, "abrir_app", "{}"), Resposta::Ok));
    esperar(|| ler(&saida_app).contains("revelar="), "a saída do app");
    let texto = ler(&saida_app);
    assert!(texto.contains("papel=app\n"), "{texto}");
    assert!(texto.contains("argumentos=0\n"), "{texto}");
    assert!(
        texto.contains("ambiente=HOME,LANG,PATH,WARD_DEV_DIR,WARD_DEV_SOCKET_DIR,XDG_RUNTIME_DIR\n"),
        "{texto}"
    );
    assert_eq!(
        fs::symlink_metadata(&saida_app).unwrap().mode() & 0o777,
        0o600
    );
    // V3.38: o log do daemon tem o código de saída, só o número.
    esperar(
        || ler(&log).contains(") saiu com código 0"),
        "o código de saída no log",
    );

    // À mão (o mesmo arquivo) e uma cópia: agente, e `revelar` não autorizado.
    let nao = ErroResposta::NaoAutorizado.codigo();
    let copia = repo.0.join("bin/copia-do-app");
    fs::copy(&app, &copia).unwrap();
    for b in [&app, &copia] {
        let saida = a_mao(b, &sock.0);
        assert!(saida.contains("papel=agente\n"), "{}: {saida}", b.display());
        assert!(
            saida.contains(&format!("revelar={nao}\n")),
            "{}: {saida}",
            b.display()
        );
    }
    let r = pedir(
        &mut c,
        "revelar",
        r#"{"entrada":"acme-pg-dev","alvo":{"campo":"senha"}}"#,
    );
    assert!(
        matches!(r, Resposta::Erro(ErroResposta::NaoAutorizado)),
        "{r:?}"
    );

    // O app saiu: aberto de novo, é app de novo (o pid novo é o do lançador).
    assert!(matches!(pedir(&mut c, "abrir_app", "{}"), Resposta::Ok));
    esperar(
        || ler(&saida_app).matches("papel=app\n").count() == 2,
        "a segunda abertura",
    );
    let daemon = ler(&log);
    assert!(!daemon.contains("falso-"), "valor no log do daemon");
}
