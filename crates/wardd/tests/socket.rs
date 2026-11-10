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
                PathBuf::from(format!("/run/user/{}", ward_platform::effective_uid()))
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
    let socket = sock.0.join(ward_platform::socket::SOCKET_NAME);
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
            !sock.0.join(ward_platform::socket::SOCKET_NAME).exists(),
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
            r#"{{"entrada":"{nome}","argv":["{}"],"cwd":"{}","tipo_entrada":"nula","ambiente":[]}}"#,
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
    // Sem `WARD_DEV_APP`, o daemon de teste usa o lançador do `ward-app` instalado: numa
    // máquina sem o Ward é "aprovação indisponível"; com ele instalado, abre (Ok).
    let instalado = std::path::Path::new("/usr/local/bin/ward-app").exists();
    let e = protocolo_erro(&r);
    assert!(
        e == Some(ErroResposta::AprovacaoIndisponivel) || (instalado && e.is_none()),
        "abrir_app: {e:?}"
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

/// V3.33 (D62, S9) pelo binário: `kill -9` entre as senhas erradas do agente não zera
/// a espera. A falha já está no arquivo de estado quando a resposta chega, e o daemon
/// que sobe de novo espera pelo contador do disco (contado da partida).
#[test]
fn v3_33_kill_9_entre_as_senhas_erradas() {
    let dev = Dir::novo("kill9");
    {
        let (d, _) = cofre_de_teste(&dev);
        d.travar();
    }
    let estado = dev.caminhos().estado;
    let falhas = || {
        wardd::arquivos::ler_estado_completo(&estado)
            .unwrap()
            .falhas_agente
    };
    let sock = DirSocket::novo("kill9");
    let mut esperas = Vec::new();
    for i in 0..3u32 {
        let mut w = subir(&dev, &sock);
        let mut s = conectar(&w);
        let t = Instant::now();
        let (_, r) = pedir_por(&mut s, "destravar", &senha_json("falso-errada-k9"));
        esperas.push(t.elapsed());
        assert_eq!(protocolo_erro(&r), Some(ErroResposta::SenhaIncorreta));
        // Gravado (com fsync) antes da resposta: o `kill -9` logo depois não perde.
        w.filho.kill().unwrap();
        let _ = w.filho.wait();
        assert_eq!(falhas(), i + 1);
        let _ = fs::remove_dir_all(&sock.0);
    }
    // 0 s, 1 s e 2 s, menos o tempo de subida (contado da partida). A primeira só tem
    // o Argon2 (com margem para máquina carregada); que ela não esperou 1 s se vê pela
    // distância até a terceira, que espera 2 s menos a subida.
    assert!(esperas[0] < Duration::from_millis(1500), "{esperas:?}");
    assert!(
        esperas[2] >= esperas[0] + Duration::from_millis(1200),
        "{esperas:?}"
    );
    assert!(esperas[1] >= Duration::from_millis(500), "{esperas:?}");
    assert!(esperas[2] >= Duration::from_millis(1500), "{esperas:?}");
    // O log não tem a senha.
    let log = fs::read(dev.0.join("wardd.log")).unwrap();
    // V3.34 (D62): com o limite comum do usuário, o daemon de teste sobe com aviso.
    let infinito = fs::read_to_string("/proc/self/limits")
        .unwrap()
        .lines()
        .any(|l| l.starts_with("Max locked memory") && l.contains("unlimited"));
    if !infinito {
        assert!(
            contem(&log, "memória sem travar".as_bytes()),
            "{}",
            String::from_utf8_lossy(&log)
        );
    }
    for s in [SENHA, "falso-errada-k9"] {
        assert!(!contem(&log, s.as_bytes()));
        assert!(!contem(&log, b64(s.as_bytes()).as_bytes()));
    }
}

/// Lê `/proc/<pid>/<arquivo>` por outro processo do mesmo uid (`/usr/bin/cat`): só
/// diz se conseguiu ler algum byte, nunca o conteúdo.
fn outro_processo_le(pid: u32, arquivo: &str) -> bool {
    let o = Command::new("/usr/bin/cat")
        .arg(format!("/proc/{pid}/{arquivo}"))
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .unwrap();
    o.status.success() || !o.stdout.is_empty()
}

/// V3.6 (S8) no binário (scrutiny do M3): o `wardd` de teste no ar e destravado (com
/// segredos em memória). Outro processo do mesmo uid não lê `/proc/<pid>/mem`,
/// `environ` nem `maps`, e este teste (o pai, que o Yama deixaria) também não;
/// `strace -p` e `gdb -p` (se houver) não se prendem; `/proc/<pid>/limits` mostra core
/// 0 (atual e máximo); e `kill -SEGV` não gera core (o kernel não marca core no status
/// e não sobra arquivo). Controle: o mesmo `cat` lê o `environ` de um processo comum.
#[test]
fn v3_6_wardd_no_ar_nega_proc_ptrace_e_core() {
    use std::os::unix::process::ExitStatusExt;

    let mut controle = Command::new("/usr/bin/sleep")
        .arg("30")
        .env_clear()
        .env("WARD_CONTROLE", "1")
        .spawn()
        .unwrap();
    // O `environ` só fica certo depois do exec: tenta por um instante.
    let fim = Instant::now() + Duration::from_secs(5);
    while !outro_processo_le(controle.id(), "environ") {
        assert!(
            Instant::now() < fim,
            "controle: cat não leu o environ de um processo comum"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = controle.kill();
    let _ = controle.wait();

    let dev = Dir::novo("v36");
    {
        let (d, _) = cofre_de_teste(&dev);
        d.travar();
    }
    let sock = DirSocket::novo("v36");
    let log = dev.0.join("wardd.log");
    let mut c = comando(&dev.0, &sock.0, &log);
    // Diretório de trabalho do daemon: onde um core em arquivo cairia.
    c.current_dir(&dev.0);
    let filho = c.spawn().unwrap();
    let socket = sock.0.join(ward_platform::socket::SOCKET_NAME);
    let fim = Instant::now() + Duration::from_secs(20);
    while !socket.exists() {
        assert!(Instant::now() < fim, "o wardd não criou o socket");
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut w = Wardd { filho, log, socket };
    let mut s = conectar(&w);
    let (_, r) = pedir_por(&mut s, "destravar", &senha_json(SENHA));
    assert!(ok(&r), "{r:?}");
    let pid = w.filho.id();

    for arquivo in ["mem", "environ", "maps"] {
        assert!(
            !outro_processo_le(pid, arquivo),
            "/proc/<pid>/{arquivo} legível por outro processo"
        );
        let r = fs::read(format!("/proc/{pid}/{arquivo}"));
        assert_eq!(
            r.as_ref().map_err(|e| e.kind()).err(),
            Some(std::io::ErrorKind::PermissionDenied),
            "/proc/<pid>/{arquivo} legível pelo pai"
        );
    }
    // `strace -p`: não se prende (sai ≠ 0 com EPERM).
    let st = Command::new("/usr/bin/strace")
        .args(["-o", "/dev/null", "-p", &pid.to_string()])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(!st.success(), "strace -p conseguiu");
    // `gdb -p`, se houver gdb: o attach falha por permissão.
    if Path::new("/usr/bin/gdb").exists() {
        let o = Command::new("/usr/bin/gdb")
            .args([
                "-nx",
                "-batch",
                "-p",
                &pid.to_string(),
                "-ex",
                "info registers",
            ])
            .env_clear()
            .env("HOME", &dev.0)
            .env("LANG", "C")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let texto =
            String::from_utf8_lossy(&o.stderr).into_owned() + &String::from_utf8_lossy(&o.stdout);
        assert!(
            texto.contains("Operation not permitted"),
            "gdb -p não recusou: {texto}"
        );
    } else {
        eprintln!("v3_6: sem /usr/bin/gdb; o strace -p cobre o mesmo ptrace");
    }
    // Core desligado: `Max core file size 0 0`.
    let limits = fs::read_to_string(format!("/proc/{pid}/limits")).unwrap();
    let core = limits
        .lines()
        .find(|l| l.starts_with("Max core file size"))
        .unwrap();
    let campos: Vec<&str> = core.split_whitespace().collect();
    assert_eq!(&campos[4..6], &["0", "0"], "{core}");
    // O daemon segue atendendo depois das tentativas.
    let (_, r) = pedir_por(&mut s, "estado", "{}");
    assert!(
        matches!(r, Resposta::Estado { travado: false, .. }),
        "{r:?}"
    );

    // `kill -SEGV` com o cofre destravado: morre pelo sinal, sem core. O primeiro
    // SIGSEGV mandado por `kill` cai no tratador do std do Rust (o de estouro de
    // pilha), que volta o tratador ao padrão e retorna; o seguinte mata. Manda de novo
    // a cada 200 ms até morrer.
    let fim = Instant::now() + Duration::from_secs(10);
    let saida = loop {
        if let Some(st) = w.filho.try_wait().unwrap() {
            break st;
        }
        assert!(Instant::now() < fim, "o wardd não morreu com SIGSEGV");
        let st = Command::new("/usr/bin/kill")
            .args(["-SEGV", &pid.to_string()])
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(st.success() || w.filho.try_wait().unwrap().is_some());
        std::thread::sleep(Duration::from_millis(200));
    };
    assert_eq!(saida.signal(), Some(11), "{saida:?}");
    assert!(!saida.core_dumped(), "o kernel gerou core do wardd");
    for e in fs::read_dir(&dev.0).unwrap() {
        let nome = e.unwrap().file_name().to_string_lossy().into_owned();
        assert!(!nome.starts_with("core"), "core no disco: {nome}");
    }
    drop(s);
}
