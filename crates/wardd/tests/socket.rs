//! O binário `wardd` (modo de teste) de ponta a ponta pelo socket: sobe com o ambiente
//! limpo, recusa `TMPDIR`/`LD_*` (V3.30, V3.25), atende o agente, aguenta conexões
//! paradas e quadro grande demais (V3.4), e o log com `RUST_LOG=trace` não tem senha,
//! PIN nem segredo (V3.20, S8).
//!
//! O cofre é de teste, criado neste diretório pelo próprio núcleo; o socket fica num
//! diretório só deste teste em `$XDG_RUNTIME_DIR` (o caminho do socket não cabe em
//! `target/tmp`), apagado no fim.

mod comum;

use std::fs;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use comum::*;
use ward_core::protocolo::{self, ErroResposta, Papel, Resposta};

const WARDD: &str = env!("CARGO_BIN_EXE_wardd");

struct DirSocket(PathBuf);

impl DirSocket {
    fn novo(rotulo: &str) -> DirSocket {
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| {
                PathBuf::from(format!("/run/user/{}", ward_plataforma::uid_efetivo()))
            });
        let p = base.join(format!("wardd-t-{}-{rotulo}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        DirSocket(p)
    }
}

impl Drop for DirSocket {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Wardd {
    filho: Child,
    log: PathBuf,
    socket: PathBuf,
}

impl Drop for Wardd {
    fn drop(&mut self) {
        let _ = self.filho.kill();
        let _ = self.filho.wait();
    }
}

fn comando(dev: &Path, sock: &Path, log: &Path) -> Command {
    let mut c = Command::new(WARDD);
    c.env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("WARD_DEV_DIR", dev)
        .env("WARD_DEV_SOCKET_DIR", sock)
        .env("RUST_LOG", "trace")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(fs::File::create(log).unwrap());
    c
}

fn subir(dev: &Dir, sock: &DirSocket) -> Wardd {
    let log = dev.0.join("wardd.log");
    let filho = comando(&dev.0, &sock.0, &log).spawn().unwrap();
    let socket = sock.0.join(ward_plataforma::socket::NOME_SOCKET);
    let fim = Instant::now() + Duration::from_secs(20);
    while !socket.exists() {
        assert!(Instant::now() < fim, "o wardd não criou o socket");
        std::thread::sleep(Duration::from_millis(20));
    }
    Wardd { filho, log, socket }
}

fn conectar(w: &Wardd) -> UnixStream {
    let s = UnixStream::connect(&w.socket).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
    s
}

fn mandar(s: &mut UnixStream, json: &str) {
    if let Err(e) = s
        .write_all(&(json.len() as u32).to_be_bytes())
        .and_then(|()| s.write_all(json.as_bytes()))
    {
        panic!("escrita no socket: {e}");
    }
}

fn receber(s: &mut UnixStream) -> Option<Vec<u8>> {
    let mut n = [0u8; 4];
    s.read_exact(&mut n).ok()?;
    let mut b = vec![0u8; u32::from_be_bytes(n) as usize];
    s.read_exact(&mut b).ok()?;
    Some(b)
}

fn pedir_por(s: &mut UnixStream, op: &str, dados: &str) -> (Vec<u8>, Resposta) {
    mandar(s, &format!(r#"{{"v":1,"op":"{op}","dados":{dados}}}"#));
    let b = receber(s).expect("sem resposta");
    let r = protocolo::decodificar_resposta(Papel::Agente, &b).unwrap();
    (b, r)
}

fn senha_json(s: &str) -> String {
    format!(r#"{{"senha":"{}"}}"#, b64(s.as_bytes()))
}

#[test]
fn v3_30_ambiente_com_tmpdir_ou_ld_recusa_subir() {
    let dev = Dir::novo("amb");
    for (k, v) in [
        ("TMPDIR", "/tmp"),
        ("LD_LIBRARY_PATH", "/nao-existe"),
        ("LD_PRELOAD", "/nao-existe.so"),
    ] {
        let sock = DirSocket::novo("amb");
        let log = dev.0.join("amb.log");
        let st = comando(&dev.0, &sock.0, &log).env(k, v).status().unwrap();
        assert!(!st.success(), "{k}");
        assert!(
            !sock.0.join(ward_plataforma::socket::NOME_SOCKET).exists(),
            "{k}"
        );
        let texto = fs::read_to_string(&log).unwrap();
        assert!(texto.contains(k), "{k}: {texto}");
    }
    // Sem WARD_DEV_DIR o daemon de teste não sobe (nunca cai no cofre real).
    let sock = DirSocket::novo("amb2");
    let log = dev.0.join("amb2.log");
    let st = comando(&dev.0, &sock.0, &log)
        .env_remove("WARD_DEV_DIR")
        .status()
        .unwrap();
    assert!(!st.success());
}

/// V3.20, V3.17, V3.26, V3.9, V3.4: um ciclo inteiro pelo socket, com o log em `trace`.
#[test]
fn v3_20_ciclo_pelo_socket_sem_segredo_no_log() {
    let dev = Dir::novo("sock");
    {
        let (d, _) = cofre_de_teste(&dev);
        d.travar();
    }
    let sock = DirSocket::novo("ciclo");
    let w = subir(&dev, &sock);
    let mut s = conectar(&w);

    let (_, r) = pedir_por(&mut s, "estado", "{}");
    assert!(
        matches!(
            r,
            Resposta::Estado {
                travado: true,
                cofre_existe: true,
                papel: Papel::Agente,
            }
        ),
        "{r:?}"
    );
    // Senha errada: mesma resposta, sem contagem; a certa (que espera 1 s) destrava.
    let (q1, r) = pedir_por(&mut s, "destravar", &senha_json("falso-errada-01"));
    assert_eq!(protocolo_erro(&r), Some(ErroResposta::SenhaIncorreta));
    let (_, r) = pedir_por(&mut s, "destravar", &senha_json(SENHA));
    assert!(ok(&r), "{r:?}");

    // Lista do agente: sem a bloqueada e sem nível.
    let (q, r) = pedir_por(&mut s, "lista", "{}");
    let Resposta::Lista(l) = r else {
        panic!("{r:?}")
    };
    assert_eq!(l.len(), 4);
    assert!(!contem(&q, b"t-bloq") && !contem(&q, b"\"nivel\""));
    // Bloqueada = inexistente, byte a byte.
    let exec = |nome: &str| {
        format!(
            r#"{{"entrada":"{nome}","argv":["{}"],"cwd":"{}","ambiente":[]}}"#,
            b64(b"env"),
            b64(b"/")
        )
    };
    let (a, _) = pedir_por(&mut s, "executar", &exec("t-bloq"));
    let (b, _) = pedir_por(&mut s, "executar", &exec("nao-existe"));
    assert_eq!(a, b);
    // O agente não muda nível nem com o PIN certo.
    let (_, r) = pedir_por(
        &mut s,
        "mudar_nivel",
        &format!(
            r#"{{"entrada":"t-bloq","nivel":"livre","pin":"{}"}}"#,
            b64(PIN.as_bytes())
        ),
    );
    assert_eq!(protocolo_erro(&r), Some(ErroResposta::NaoAutorizado));
    // Fase 23: o agente não responde aprovação (e não há operação para receber os
    // avisos: só a conexão do papel app os recebe, D65); o
    // `abrir_app` sem instalação (daemon de teste, D63 sem a 23a) é "aprovação
    // indisponível", e o uso com aprovação também (D21).
    let (_, r) = pedir_por(&mut s, "receber_pedidos", "{}");
    assert_eq!(protocolo_erro(&r), Some(ErroResposta::OperacaoDesconhecida));
    let (_, r) = pedir_por(
        &mut s,
        "responder_aprovacao",
        &format!(
            r#"{{"pedido":1,"decisao":"uma_vez","pin":"{}"}}"#,
            b64(PIN.as_bytes())
        ),
    );
    assert_eq!(protocolo_erro(&r), Some(ErroResposta::NaoAutorizado));
    let (_, r) = pedir_por(&mut s, "abrir_app", "{}");
    assert_eq!(
        protocolo_erro(&r),
        Some(ErroResposta::AprovacaoIndisponivel)
    );
    let (_, r) = pedir_por(&mut s, "executar", &exec("t-aprov"));
    assert_eq!(
        protocolo_erro(&r),
        Some(ErroResposta::AprovacaoIndisponivel)
    );

    // V3.4: 70 conexões paradas não tiram o atendimento (a mais antiga parada sai).
    let paradas: Vec<UnixStream> = (0..70).map(|_| conectar(&w)).collect();
    std::thread::sleep(Duration::from_millis(200));
    let mut nova = conectar(&w);
    let (_, r) = pedir_por(&mut nova, "estado", "{}");
    assert!(
        matches!(r, Resposta::Estado { travado: false, .. }),
        "{r:?}"
    );
    drop(paradas);

    // Quadro maior que o limite: recusado, conexão fechada, daemon segue.
    let mut grande = conectar(&w);
    grande.write_all(&u32::MAX.to_be_bytes()).unwrap();
    let _ = receber(&mut grande);
    let mut b = [0u8; 1];
    assert_eq!(grande.read(&mut b).unwrap_or(0), 0);
    // JSON inválido, sem fechar o daemon.
    let mut lixo = conectar(&w);
    mandar(&mut lixo, "{nao e json");
    let _ = receber(&mut lixo);

    // A primeira conexão estava parada durante a enxurrada: foi a que saiu.
    let mut b = [0u8; 1];
    assert_eq!(
        s.read(&mut b).unwrap_or(0),
        0,
        "a conexão parada mais antiga saiu"
    );
    let mut s = conectar(&w);
    let (_, r) = pedir_por(&mut s, "travar", "{}");
    assert!(ok(&r));
    let (_, r) = pedir_por(&mut s, "lista", "{}");
    assert_eq!(protocolo_erro(&r), Some(ErroResposta::Travado));
    drop(s);
    drop(nova);

    // V3.20: o log em `trace` não tem senha, PIN nem segredo (nem em base64).
    let log = w.log.clone();
    drop(w);
    let texto = fs::read(&log).unwrap();
    assert!(
        contem(&texto, b"pronto"),
        "{}",
        String::from_utf8_lossy(&texto)
    );
    for s in SEGREDOS.iter().chain([&SENHA, &PIN, &"falso-errada-01"]) {
        assert!(!contem(&texto, s.as_bytes()), "{s} no log");
        assert!(
            !contem(&texto, b64(s.as_bytes()).as_bytes()),
            "{s} (b64) no log"
        );
    }
    assert!(!contem(&q1, b"falso"));
}

fn protocolo_erro(r: &Resposta) -> Option<ErroResposta> {
    erro(r)
}
