//! Trava automática (Fase 24, V3.19): inatividade com relógio injetado, lida do tempo de
//! trava da configuração do cofre, e o logind (`Lock`, `LockedHint`, `PrepareForSleep`)
//! num **barramento falso**: um `dbus-daemon` privado, só deste teste, num diretório
//! 0700 de `$XDG_RUNTIME_DIR`, com um logind falso feito com o próprio `zbus`. Nunca o
//! barramento do sistema nem o logind real.
//!
//! Ataques (todos falham do lado do atacante):
//! - agente que consulta `estado` ou manda pedidos com erro não segura o cofre aberto;
//! - relógio que volta ou que não se lê trava;
//! - barramento que cai com o cofre aberto trava;
//! - sinal forjado por quem não é o logind não destrava nem desfaz a trava;
//! - a suspensão não passa à frente da trava (o inibidor só é solto depois).

mod comum;

use std::collections::HashMap;
use std::fs;
use std::io::{PipeReader, Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use comum::*;
use ward_core::protocolo::{self, ErroResposta, Papel, Resposta};
use wardd::nucleo::{Daemon, Relogio};
use zbus::Connection;
use zbus::Variant as Value;
use zbus::connection::Builder;

/// `emit_signal` do zbus, esperado aqui mesmo (o teste não tem runtime próprio).
macro_rules! emitir {
    ($c:expr, $($a:expr),+ $(,)?) => {
        zbus::block_on($c.emit_signal(None::<&str>, $($a),+)).unwrap()
    };
}

const H: u64 = 3600;
const LOGIND: &str = "org.freedesktop.login1";
const SESSAO: &str = "/org/freedesktop/login1/session/_32";
const IFACE_SESSAO: &str = "org.freedesktop.login1.Session";
const WARDD: &str = env!("CARGO_BIN_EXE_wardd");

// ---------------------------------------------------------------- relógio injetado

/// Relógio manual: `agora` devolve o que o teste pôs (`None` = ilegível); `dormir` só
/// anota a espera do D20.
#[derive(Clone)]
struct RelogioManual(Arc<Mutex<Option<Duration>>>, Arc<Mutex<Vec<Duration>>>);

impl RelogioManual {
    fn novo() -> Self {
        RelogioManual(Arc::new(Mutex::new(Some(Duration::ZERO))), Arc::default())
    }
    fn esperas(&self) -> Vec<Duration> {
        std::mem::take(&mut *self.1.lock().unwrap())
    }
    fn por(&self, segundos: u64) {
        *self.0.lock().unwrap() = Some(Duration::from_secs(segundos));
    }
    fn ilegivel(&self) {
        *self.0.lock().unwrap() = None;
    }
}

impl Relogio for RelogioManual {
    fn dormir(&self, d: Duration) {
        self.1.lock().unwrap().push(d);
    }
    fn agora(&self) -> Option<Duration> {
        *self.0.lock().unwrap()
    }
}

/// Cofre de teste novo (destravado em t = 0) no `dir`, com o relógio manual.
fn cofre_com_relogio(dir: &Dir) -> (Daemon, RelogioManual) {
    let r = RelogioManual::novo();
    let d = Daemon::novo(dir.caminhos(), Box::new(r.clone()));
    let criar = pedir(
        &d,
        Papel::App,
        "criar_cofre",
        &format!(
            r#"{{"senha":"{}","pin":"{}"}}"#,
            b64(SENHA.as_bytes()),
            b64(PIN.as_bytes())
        ),
    );
    assert!(ok(&criar), "{criar:?}");
    let s = pedir(
        &d,
        Papel::App,
        "salvar_entrada",
        &login("t-aprov", SEGREDOS[0]),
    );
    assert!(ok(&s), "{s:?}");
    (d, r)
}

fn lista_ok(d: &Daemon) {
    let r = pedir(d, Papel::Agente, "lista", "{}");
    assert!(matches!(r, Resposta::Lista(_)), "{r:?}");
}

#[test]
fn v3_19_inatividade_com_relogio_injetado() {
    let dir = Dir::novo("inat");
    let (d, r) = cofre_com_relogio(&dir);
    // Padrão da configuração (D30): 240 min.
    r.por(4 * H - 60);
    assert!(!d.travar_se_inativo(), "3h59 sem uso não trava");
    assert!(!d.travado());
    r.por(4 * H);
    assert!(d.travar_se_inativo(), "4h00 sem uso trava");
    assert!(d.travado());
    assert_eq!(
        erro(&pedir(&d, Papel::Agente, "lista", "{}")),
        Some(ErroResposta::Travado)
    );
    // Travado, nada mais a fazer.
    assert!(!d.travar_se_inativo());

    // Um uso às 3h zera a contagem.
    let t0 = 10 * H;
    r.por(t0);
    assert!(ok(&destravar(&d, SENHA)));
    r.por(t0 + 3 * H);
    lista_ok(&d);
    r.por(t0 + 4 * H);
    assert!(!d.travar_se_inativo(), "o uso às 3h zerou");
    r.por(t0 + 3 * H + 4 * H - 1);
    assert!(!d.travar_se_inativo());
    r.por(t0 + 7 * H);
    assert!(d.travar_se_inativo());
}

#[test]
fn v3_19_tempo_lido_da_configuracao_do_cofre() {
    let dir = Dir::novo("cfg");
    let (d, r) = cofre_com_relogio(&dir);
    let t = pedir(&d, Papel::App, "tempo_trava", r#"{"minutos":30}"#);
    assert!(ok(&t), "{t:?}");
    r.por(30 * 60 - 1);
    assert!(!d.travar_se_inativo());
    r.por(30 * 60);
    assert!(d.travar_se_inativo(), "30 min da configuração");
    // Vale depois de reabrir: o tempo está no cofre, não na memória.
    let d2 = Daemon::novo(dir.caminhos(), Box::new(r.clone()));
    r.por(100 * H);
    assert!(ok(&destravar(&d2, SENHA)));
    r.por(100 * H + 29 * 60);
    assert!(!d2.travar_se_inativo());
    r.por(100 * H + 30 * 60);
    assert!(d2.travar_se_inativo());
}

#[test]
fn ataque_agente_nao_segura_o_cofre_aberto_com_estado_ou_erro() {
    let dir = Dir::novo("segura");
    let (d, r) = cofre_com_relogio(&dir);
    // Durante 4 h, a cada 10 min: `estado`, entrada inexistente, `destravar` repetido,
    // pedido de app pelo agente e, quatro vezes, PIN errado (o quinto travaria pelo S9).
    let mut t = 0;
    while t < 4 * H {
        r.por(t);
        assert!(matches!(
            pedir(&d, Papel::Agente, "estado", "{}"),
            Resposta::Estado { travado: false, .. }
        ));
        assert_eq!(
            erro(&pedir(
                &d,
                Papel::Agente,
                "executar",
                &executar("nao-existe")
            )),
            Some(ErroResposta::EntradaInexistente)
        );
        // `destravar` com o cofre já aberto (qualquer senha) não é uso.
        assert!(ok(&destravar(&d, "qualquer-coisa")));
        assert!(
            erro(&pedir(
                &d,
                Papel::Agente,
                "detalhe",
                r#"{"entrada":"t-aprov"}"#
            ))
            .is_some()
        );
        if t % 2400 == 0 && t / 2400 < 4 {
            let m = mudar_nivel(&d, Papel::App, "t-aprov", "livre", PIN_ERRADO);
            assert_eq!(erro(&m), Some(ErroResposta::PinIncorreto));
        }
        assert!(!d.travar_se_inativo(), "t = {t}");
        t += 600;
    }
    r.por(4 * H);
    assert!(
        d.travar_se_inativo(),
        "nenhum desses pedidos conta como uso"
    );
}

fn executar(entrada: &str) -> String {
    format!(
        r#"{{"entrada":"{entrada}","argv":["{}"],"cwd":"{}","ambiente":[]}}"#,
        b64(b"true"),
        b64(b"/")
    )
}

/// D62: a trava automática não mexe no contador de espera do D20 (nem zera a espera de
/// quem errou a senha, nem cria espera para o usuário).
#[test]
fn d62_trava_automatica_nao_mexe_na_espera_do_d20() {
    let dir = Dir::novo("d20");
    let (d, r) = cofre_com_relogio(&dir);
    r.por(4 * H);
    assert!(d.travar_se_inativo());
    r.esperas();
    // Depois da trava automática, a primeira tentativa não espera.
    assert_eq!(
        erro(&destravar(&d, "senha-errada")),
        Some(ErroResposta::SenhaIncorreta)
    );
    assert_eq!(
        erro(&destravar(&d, "senha-errada")),
        Some(ErroResposta::SenhaIncorreta)
    );
    assert_eq!(r.esperas(), [Duration::ZERO, Duration::from_secs(1)]);
    // Travado, a conferência de inatividade não zera as duas falhas: a próxima espera 2 s.
    r.por(100 * H);
    assert!(!d.travar_se_inativo());
    assert!(ok(&destravar(&d, SENHA)));
    assert_eq!(r.esperas(), [Duration::from_secs(2)]);
}

#[test]
fn ataque_relogio_que_volta_ou_ilegivel_trava() {
    let dir = Dir::novo("relogio");
    let (d, r) = cofre_com_relogio(&dir);
    r.por(2 * H);
    lista_ok(&d);
    r.por(H);
    assert!(d.travar_se_inativo(), "relógio voltou: trava");

    r.por(5 * H);
    assert!(ok(&destravar(&d, SENHA)));
    r.ilegivel();
    assert!(d.travar_se_inativo(), "relógio ilegível: trava");

    // Uso anotado sem relógio também trava na conferência seguinte.
    r.por(6 * H);
    assert!(ok(&destravar(&d, SENHA)));
    r.ilegivel();
    let _ = pedir(&d, Papel::Agente, "lista", "{}");
    r.por(6 * H + 1);
    assert!(d.travar_se_inativo());
}

// ---------------------------------------------------------------- barramento falso

static N: AtomicU32 = AtomicU32::new(0);

/// `dbus-daemon` privado, só deste teste (morto no `Drop`).
struct BarramentoFalso {
    dir: PathBuf,
    socket: PathBuf,
    filho: Option<Child>,
}

impl BarramentoFalso {
    fn subir() -> BarramentoFalso {
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| {
                PathBuf::from(format!("/run/user/{}", ward_plataforma::uid_efetivo()))
            });
        let dir = base.join(format!(
            "wardd-bus-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        let socket = dir.join("bus");
        let config = dir.join("bus.conf");
        fs::write(
            &config,
            format!(
                r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <listen>unix:path={}</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#,
                socket.display()
            ),
        )
        .unwrap();
        let filho = Command::new("/usr/bin/dbus-daemon")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--nopidfile", "--nosyslog"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("dbus-daemon");
        let fim = Instant::now() + Duration::from_secs(10);
        while UnixStream::connect(&socket).is_err() {
            assert!(Instant::now() < fim, "o dbus-daemon falso não subiu");
            std::thread::sleep(Duration::from_millis(20));
        }
        BarramentoFalso {
            dir,
            socket,
            filho: Some(filho),
        }
    }

    fn endereco(&self) -> String {
        format!("unix:path={}", self.socket.display())
    }

    fn cliente(&self) -> Connection {
        zbus::block_on(Builder::address(self.endereco().as_str()).build()).unwrap()
    }

    fn matar(&mut self) {
        if let Some(mut f) = self.filho.take() {
            let _ = f.kill();
            let _ = f.wait();
        }
    }
}

impl Drop for BarramentoFalso {
    fn drop(&mut self) {
        self.matar();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Inibidores entregues pelo logind falso: (what, mode, ponta de leitura do pipe).
type Inibidores = Arc<Mutex<Vec<(String, String, PipeReader)>>>;

struct Manager {
    inibidores: Inibidores,
}

#[zbus::interface(name = "org.freedesktop.login1.Manager")]
impl Manager {
    fn inhibit(
        &self,
        what: &str,
        _who: &str,
        _why: &str,
        mode: &str,
    ) -> zbus::fdo::Result<zbus::OwnedFd> {
        let (r, w) = std::io::pipe().map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        self.inibidores
            .lock()
            .unwrap()
            .push((what.to_owned(), mode.to_owned(), r));
        Ok(OwnedFd::from(w).into())
    }
}

/// O logind falso: dono de `org.freedesktop.login1` no barramento falso.
struct LogindFalso {
    conn: Connection,
    inibidores: Inibidores,
}

impl LogindFalso {
    fn novo(b: &BarramentoFalso) -> LogindFalso {
        let inibidores = Inibidores::default();
        let conn = zbus::block_on(
            Builder::address(b.endereco().as_str())
                .name(LOGIND)
                .serve_at(
                    "/org/freedesktop/login1",
                    Manager {
                        inibidores: inibidores.clone(),
                    },
                )
                .build(),
        )
        .unwrap();
        LogindFalso { conn, inibidores }
    }

    fn lock(&self) {
        emitir!(self.conn, SESSAO, IFACE_SESSAO, "Lock", &());
    }

    fn locked_hint(&self, valor: bool) {
        let mut mudou: HashMap<&str, Value<'_>> = HashMap::new();
        mudou.insert("LockedHint", Value::Bool(valor));
        emitir!(
            self.conn,
            SESSAO,
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
            &(IFACE_SESSAO, mudou, Vec::<&str>::new()),
        );
    }

    fn prepare_for_sleep(&self, valor: bool) {
        emitir!(
            self.conn,
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
            "PrepareForSleep",
            &valor,
        );
    }

    /// Espera o `n`-ésimo inibidor e devolve um canal que recebe quando ele fecha.
    fn inibidor(&self, n: usize) -> mpsc::Receiver<usize> {
        let fim = Instant::now() + Duration::from_secs(10);
        loop {
            {
                let mut v = self.inibidores.lock().unwrap();
                if v.len() > n {
                    let (what, mode, r) = &mut v[n];
                    assert_eq!((what.as_str(), mode.as_str()), ("sleep", "delay"));
                    let mut r = r.try_clone().unwrap();
                    let (tx, rx) = mpsc::channel();
                    std::thread::spawn(move || {
                        let mut b = [0u8; 1];
                        let _ = tx.send(r.read(&mut b).unwrap_or(0));
                    });
                    return rx;
                }
            }
            assert!(Instant::now() < fim, "o daemon não pediu o inibidor {n}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// Daemon de teste com o vigia do logind ligado ao barramento falso.
fn daemon_vigiado(dir: &Dir, b: &BarramentoFalso) -> (Arc<Daemon>, mpsc::Receiver<()>) {
    let (d, _r) = cofre_de_teste(dir);
    let d = Arc::new(d);
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    wardd::trava::vigiar_logind(
        d.clone(),
        b.socket.clone(),
        Box::new(move || {
            let _ = tx.lock().unwrap().send(());
        }),
    )
    .unwrap();
    rx.recv_timeout(Duration::from_secs(10))
        .expect("o vigia não conectou ao barramento falso");
    (d, rx)
}

fn espera_travar(d: &Daemon, limite: Duration) -> Duration {
    let ini = Instant::now();
    while !d.travado() {
        assert!(
            ini.elapsed() < limite,
            "não travou em {limite:?} (V3.19: até 2 s)"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    ini.elapsed()
}

#[test]
fn v3_19_lock_e_locked_hint_travam_em_ate_2s() {
    let b = BarramentoFalso::subir();
    let logind = LogindFalso::novo(&b);
    let dir = Dir::novo("lock");
    let (d, _pronto) = daemon_vigiado(&dir, &b);
    assert!(!d.travado());

    // `loginctl lock-session` → `Lock` na sessão.
    logind.lock();
    espera_travar(&d, Duration::from_secs(2));

    // `LockedHint` falso não trava; verdadeiro trava.
    assert!(ok(&destravar(&d, SENHA)));
    logind.locked_hint(false);
    std::thread::sleep(Duration::from_millis(300));
    assert!(!d.travado(), "LockedHint falso não trava");
    logind.locked_hint(true);
    espera_travar(&d, Duration::from_secs(2));
}

#[test]
fn v3_19_prepare_for_sleep_trava_antes_de_soltar_o_inibidor() {
    let b = BarramentoFalso::subir();
    let logind = LogindFalso::novo(&b);
    let dir = Dir::novo("sleep");
    let (d, _pronto) = daemon_vigiado(&dir, &b);
    let fechou = logind.inibidor(0);
    assert!(
        fechou.recv_timeout(Duration::from_millis(300)).is_err(),
        "o inibidor fica aberto enquanto não há suspensão"
    );
    assert!(!d.travado());
    logind.prepare_for_sleep(true);
    fechou
        .recv_timeout(Duration::from_secs(2))
        .expect("o inibidor não foi solto");
    // Quando o logind vê o fd fechar, o cofre já está travado.
    assert!(d.travado(), "a suspensão passou à frente da trava");

    // Ao acordar, o daemon pega outro inibidor; a próxima suspensão trava de novo.
    logind.prepare_for_sleep(false);
    let fechou2 = logind.inibidor(1);
    assert!(ok(&destravar(&d, SENHA)));
    logind.prepare_for_sleep(true);
    fechou2.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(d.travado());
}

#[test]
fn ataque_barramento_cai_com_o_cofre_aberto_trava() {
    let mut b = BarramentoFalso::subir();
    let _logind = LogindFalso::novo(&b);
    let dir = Dir::novo("cai");
    let (d, _pronto) = daemon_vigiado(&dir, &b);
    assert!(!d.travado());
    b.matar();
    espera_travar(&d, Duration::from_secs(2));
}

#[test]
fn ataque_sinal_forjado_nao_destrava_nem_segura() {
    let b = BarramentoFalso::subir();
    let logind = LogindFalso::novo(&b);
    let dir = Dir::novo("forja");
    let (d, _pronto) = daemon_vigiado(&dir, &b);
    // Um processo qualquer no barramento (não é o dono de org.freedesktop.login1).
    let forjador = b.cliente();
    logind.lock();
    espera_travar(&d, Duration::from_secs(2));
    for (caminho, iface, membro) in [
        (SESSAO, IFACE_SESSAO, "Unlock"),
        (
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
            "PrepareForSleep",
        ),
    ] {
        if membro == "PrepareForSleep" {
            emitir!(forjador, caminho, iface, membro, &false);
        } else {
            emitir!(forjador, caminho, iface, membro, &());
        }
    }
    let mut mudou: HashMap<&str, Value<'_>> = HashMap::new();
    mudou.insert("LockedHint", Value::Bool(false));
    emitir!(
        forjador,
        SESSAO,
        "org.freedesktop.DBus.Properties",
        "PropertiesChanged",
        &(IFACE_SESSAO, mudou, Vec::<&str>::new()),
    );
    std::thread::sleep(Duration::from_millis(300));
    assert!(d.travado(), "nada do barramento destrava");
    assert_eq!(
        erro(&pedir(&d, Papel::Agente, "lista", "{}")),
        Some(ErroResposta::Travado)
    );
}

// ---------------------------------------------------------------- binário

fn mandar(s: &mut UnixStream, json: &str) {
    s.write_all(&(json.len() as u32).to_be_bytes()).unwrap();
    s.write_all(json.as_bytes()).unwrap();
}

fn pedir_por(s: &mut UnixStream, op: &str, dados: &str) -> Resposta {
    mandar(s, &format!(r#"{{"v":1,"op":"{op}","dados":{dados}}}"#));
    let mut n = [0u8; 4];
    s.read_exact(&mut n).unwrap();
    let mut q = vec![0u8; u32::from_be_bytes(n) as usize];
    s.read_exact(&mut q).unwrap();
    protocolo::decodificar_resposta(Papel::Agente, &q).unwrap()
}

struct Filho(Child);

impl Drop for Filho {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn esperar_no_log(log: &Path, texto: &str) {
    let fim = Instant::now() + Duration::from_secs(20);
    loop {
        if fs::read_to_string(log).is_ok_and(|t| t.contains(texto)) {
            return;
        }
        assert!(Instant::now() < fim, "o log não mostrou {texto:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn v3_19_binario_trava_com_lock_session_do_logind_falso() {
    let b = BarramentoFalso::subir();
    let logind = LogindFalso::novo(&b);
    let dev = Dir::novo("bin");
    // O cofre de teste é criado pelo núcleo e depois servido pelo binário.
    drop(cofre_de_teste(&dev));
    let sock = b.dir.join("sock");
    let log = dev.0.join("wardd.log");
    let _w = Filho(
        Command::new(WARDD)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("WARD_DEV_DIR", &dev.0)
            .env("WARD_DEV_SOCKET_DIR", &sock)
            .env("WARD_DEV_DBUS_SISTEMA", &b.socket)
            .env("RUST_LOG", "trace")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(fs::File::create(&log).unwrap())
            .spawn()
            .unwrap(),
    );
    esperar_no_log(&log, "logind: vigiando");
    // O runtime do zbus não toma o SIGCHLD do daemon (o `ward run` e o lançador do app
    // esperam os próprios filhos).
    let status = fs::read_to_string(format!("/proc/{}/status", _w.0.id())).unwrap();
    let cgt = status
        .lines()
        .find_map(|l| l.strip_prefix("SigCgt:"))
        .map(|v| u64::from_str_radix(v.trim(), 16).unwrap())
        .unwrap();
    assert_eq!(cgt & (1 << (17 - 1)), 0, "SIGCHLD capturado: {cgt:x}");
    let fim = Instant::now() + Duration::from_secs(20);
    let caminho = sock.join(ward_plataforma::socket::NOME_SOCKET);
    while !caminho.exists() {
        assert!(Instant::now() < fim);
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut s = UnixStream::connect(&caminho).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(60))).unwrap();
    let r = pedir_por(
        &mut s,
        "destravar",
        &format!(r#"{{"senha":"{}"}}"#, b64(SENHA.as_bytes())),
    );
    assert!(ok(&r), "{r:?}");
    logind.lock();
    let ini = Instant::now();
    loop {
        match pedir_por(&mut s, "estado", "{}") {
            Resposta::Estado { travado: true, .. } => break,
            Resposta::Estado { .. } => {}
            outra => panic!("{outra:?}"),
        }
        assert!(ini.elapsed() < Duration::from_secs(2), "V3.19: até 2 s");
        std::thread::sleep(Duration::from_millis(20));
    }
    esperar_no_log(&log, "tela bloqueada: cofre travado");
    // O log não tem senha nem segredo (S13).
    let texto = fs::read(&log).unwrap();
    for seg in [SENHA, PIN].iter().chain(SEGREDOS.iter()) {
        assert!(!contem(&texto, seg.as_bytes()), "segredo no log");
    }
}

#[test]
fn daemon_de_teste_sem_barramento_falso_nao_liga_o_logind() {
    let dev = Dir::novo("semdbus");
    let b = BarramentoFalso::subir(); // só para ter um diretório curto do socket
    let sock = b.dir.join("sock2");
    let log = dev.0.join("wardd.log");
    let _w = Filho(
        Command::new(WARDD)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("WARD_DEV_DIR", &dev.0)
            .env("WARD_DEV_SOCKET_DIR", &sock)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(fs::File::create(&log).unwrap())
            .spawn()
            .unwrap(),
    );
    esperar_no_log(&log, "logind desligado");
    // Caminho estranho na variável: recusa subir.
    let log2 = dev.0.join("wardd2.log");
    let st = Command::new(WARDD)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("WARD_DEV_DIR", &dev.0)
        .env("WARD_DEV_SOCKET_DIR", b.dir.join("sock3"))
        .env("WARD_DEV_DBUS_SISTEMA", "/run/dbus/../x;unix:path=/tmp/x")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(fs::File::create(&log2).unwrap())
        .status()
        .unwrap();
    assert!(!st.success());
    assert!(
        fs::read_to_string(&log2)
            .unwrap()
            .contains("WARD_DEV_DBUS_SISTEMA recusado")
    );
}
