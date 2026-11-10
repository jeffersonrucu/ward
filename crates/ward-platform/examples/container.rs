//! Binário de teste do contêiner da Fase 21b (`make test-docker-platform`; D46, D47;
//! V3.5, V3.23, V3.24). Só com `test-mode`; nunca é instalado fora do contêiner.
//!
//! Um binário só, com subcomandos, instalado com nomes diferentes:
//! - `ativar <socket> <uid> -- <programa…>`: o papel do systemd do sistema (roda como
//!   root): cria o socket, passa para o uid, 0600, e executa o programa como esse uid
//!   (`setpriv`) com o socket no fd 3 e `LISTEN_FDS=1`;
//! - `servidor --chrome <exe> [--lancar <programa…>]`: o "wardd" de teste: endurece,
//!   recebe o socket por `LISTEN_FDS`, lê os grupos do `/etc/group` e responde a cada
//!   conexão uma linha com o papel; com `--lancar`, abre um filho (o app aberto pelo
//!   daemon). Se o cliente mandar um quadro logo (o app de teste da 23a pede `estado`
//!   na partida, D63), a resposta é o quadro `estado` com o papel, e a conexão fecha;
//! - `cliente [--sistema-em <socket>] [--pausar]`: confere o servidor (D46), conecta e
//!   imprime o papel; instalado como `ward-app` e `ward-nm` (setgid);
//! - `prender <programa…>`: lança o programa (um `cliente --pausar`) e tenta
//!   `PTRACE_ATTACH`, `PTRACE_SEIZE` e ler o `/proc/<pid>/mem` dele;
//! - `traceme <programa…>`: lança o programa já sob `ptrace` (`PTRACE_TRACEME`) e mostra
//!   o egid depois do `exec`: sob depurador o kernel não aplica o setgid;
//! - `bind <socket>`: tenta criar um socket (o atacante na pasta do root).
//!
//! Fase 29a (D57; V5.31), com o `ward-app` de teste do `app/core`
//! (`partida-conteiner`):
//! - `servidor … --abrir-app <bin>`: o daemon abre o app pelo lançador de D57 (e tenta
//!   abrir de novo logo em seguida: uma instância por vez);
//! - `servidor … --abrir-app-com <bin> <VAR=valor>`: o daemon abre o app com o
//!   ambiente de D57 e uma variável posta à força (teste do conferidor do app);
//! - `lancar-app <bin>`: só o lançador, para os casos de sessão errada (dois
//!   `wayland-<n>`, link);
//! - `escutar <socket>`: cria um socket e fica escutando (Wayland e barramento falsos);
//! - `ambiente`: imprime o ambiente de D57 (`VAR=valor` por linha);
//! - `--user` (chamado como `/usr/lib/systemd/systemd --user`): imita o gerente do
//!   usuário e abre o programa de `WARD_GERENTE_PROGRAMA` com o ambiente de D57.
//!
//! `unsafe` só para `ptrace`/`waitpid`/`prctl` dos ataques, com `// SAFETY:`.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ward_platform::{
    AllowedBinary, ExpectedServer, Install, KernelPolicy, Launcher, PeerClass, app_environment,
    connect_server, listen_from_listen_fds, start_hardened, system_socket_path, verify_connection,
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cmd, resto) = args
        .split_first()
        .map(|(c, r)| (c.as_str(), r))
        .unwrap_or(("", &[]));
    match cmd {
        "ativar" => ativar(resto),
        "servidor" => servidor(resto),
        "cliente" => cliente(resto),
        "prender" => prender(resto),
        "traceme" => traceme(resto),
        "bind" => bind(resto),
        "lancar-app" => lancar_app(resto),
        "escutar" => escutar(resto),
        "ambiente" => ambiente(),
        "--user" => gerente(),
        _ => {
            eprintln!(
                "uso: ativar|servidor|cliente|prender|traceme|bind|lancar-app|escutar|ambiente"
            );
            std::process::exit(2);
        }
    }
}

fn falhar(msg: impl std::fmt::Display) -> ! {
    eprintln!("erro: {msg}");
    std::process::exit(1)
}

fn ativar(args: &[String]) {
    let [socket, uid, separador, programa @ ..] = args else {
        falhar("ativar <socket> <uid> -- <programa…>");
    };
    if separador != "--" || programa.is_empty() {
        falhar("falta -- <programa…>");
    }
    let uid: u32 = uid.parse().unwrap_or_else(|_| falhar("uid"));
    let _ = std::fs::remove_file(socket);
    let listener = UnixListener::bind(socket).unwrap_or_else(|e| falhar(e));
    std::os::unix::fs::chown(socket, Some(uid), None).unwrap_or_else(|e| falhar(e));
    std::fs::set_permissions(socket, std::os::unix::fs::PermissionsExt::from_mode(0o600))
        .unwrap_or_else(|e| falhar(e));
    let mut cmd = Command::new("/usr/bin/setpriv");
    cmd.arg(format!("--reuid={uid}"))
        .arg(format!("--regid={uid}"))
        .arg("--clear-groups")
        .arg("--")
        .args(programa)
        .env_clear()
        .env("PATH", "/usr/bin:/bin");
    ward_platform::testing::run_with_socket(&listener, cmd);
}

/// fd de `listener` em `/proc/self/fd`, fd 3 ainda aberto e `O_CLOEXEC` do fd.
fn estado_dos_fds(listener: &UnixListener) -> String {
    use std::os::fd::AsRawFd;
    let fd = listener.as_raw_fd();
    let info = std::fs::read_to_string(format!("/proc/self/fdinfo/{fd}")).unwrap_or_default();
    let flags = info
        .lines()
        .find_map(|l| l.strip_prefix("flags:"))
        .and_then(|f| u32::from_str_radix(f.trim(), 8).ok())
        .unwrap_or(0);
    let cloexec = flags & 0o2_000_000 != 0;
    let fd3 = Path::new("/proc/self/fd/3").exists();
    format!("fd3_aberto={fd3} cloexec={cloexec}")
}

/// Como o app será aberto pelo servidor (Fase 29a).
enum Abertura {
    /// Pelo lançador de D57.
    Launcher(String),
    /// Ambiente de D57 com `VAR=valor` posto à força.
    Com(String, String),
}

fn servidor(args: &[String]) {
    let mut chrome = None;
    let mut lancar: Vec<String> = Vec::new();
    let mut aberturas: Vec<Abertura> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--chrome" => {
                chrome = args.get(i + 1).cloned();
                i += 2;
            }
            "--abrir-app" => {
                let bin = args
                    .get(i + 1)
                    .cloned()
                    .unwrap_or_else(|| falhar("--abrir-app"));
                aberturas.push(Abertura::Launcher(bin));
                i += 2;
            }
            "--abrir-app-com" => {
                let (Some(bin), Some(var)) = (args.get(i + 1), args.get(i + 2)) else {
                    falhar("--abrir-app-com <bin> <VAR=valor>");
                };
                aberturas.push(Abertura::Com(bin.clone(), var.clone()));
                i += 3;
            }
            "--lancar" => {
                lancar = args[i + 1..].to_vec();
                break;
            }
            _ => falhar("argumento"),
        }
    }
    if let Err(e) = start_hardened() {
        falhar(e);
    }
    let escuta = listen_from_listen_fds(&system_socket_path()).unwrap_or_else(|e| falhar(e));
    let chrome = AllowedBinary::new(chrome.unwrap_or_else(|| falhar("--chrome")))
        .unwrap_or_else(|e| falhar(e));
    let inst = Install::from_system(chrome).unwrap_or_else(|e| falhar(e));
    let politica = KernelPolicy::read();
    println!(
        "servidor: pronto {} gid_app={} gid_nm={} politica={:?} vmlck={}",
        estado_dos_fds(escuta.listener()),
        inst.app_gid(),
        inst.extension_gid(),
        politica.verify(),
        status("VmLck:")
    );
    let _ = std::io::stdout().flush();
    if let Some((programa, resto)) = lancar.split_first() {
        let saida = Command::new(programa)
            .args(resto)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn();
        match saida {
            Ok(filho) => {
                std::thread::spawn(move || {
                    let saida = filho
                        .wait_with_output()
                        .map(|o| o.stdout)
                        .unwrap_or_default();
                    println!("app-filho: {}", String::from_utf8_lossy(&saida).trim());
                    let _ = std::io::stdout().flush();
                });
            }
            Err(e) => println!("app-filho: erro {e}"),
        }
    }
    let _lancadores = abrir_apps(&aberturas, &inst);
    for conexao in escuta.listener().incoming() {
        let Ok(mut stream) = conexao else { continue };
        let par = match verify_connection(&stream) {
            Ok(p) => p,
            Err(e) => {
                println!("servidor: recusado {e:?}");
                let _ = std::io::stdout().flush();
                continue;
            }
        };
        let classe = par.classify(&inst, &politica);
        let linha = match classe {
            PeerClass::App => "papel=app".to_owned(),
            PeerClass::ExtensionHost => "papel=extensao".to_owned(),
            PeerClass::Common { reason: motivo } => format!("papel=agente motivo={motivo:?}"),
        };
        println!(
            "servidor: pid={} gid={} {linha}",
            par.credentials().pid,
            par.credentials().gid
        );
        let _ = std::io::stdout().flush();
        let papel = match classe {
            PeerClass::App => "app",
            PeerClass::ExtensionHost => "extensao",
            PeerClass::Common { .. } => "agente",
        };
        if !responder_estado(&mut stream, papel) {
            let _ = writeln!(stream, "{linha}");
        }
    }
}

/// Fase 23a (D63): o app de teste manda `estado` logo depois de conectar e confere o
/// papel na resposta. Se um quadro chega em 1 s, responde o `estado` (sem segredo, só o
/// papel) e devolve `true`; senão (o `cliente`, que só lê), `false`.
fn responder_estado(stream: &mut UnixStream, papel: &str) -> bool {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(1)));
    let mut n = [0u8; 4];
    if stream.read_exact(&mut n).is_err() {
        return false;
    }
    let tamanho = u32::from_be_bytes(n) as usize;
    if tamanho == 0 || tamanho > 64 * 1024 {
        return true;
    }
    let mut corpo = vec![0u8; tamanho];
    if stream.read_exact(&mut corpo).is_err() {
        return true;
    }
    // O app aberto fora do daemon (o do menu) só pede `abrir_app`: "ok", como o daemon.
    let abrir_app = corpo.windows(9).any(|j| j == b"abrir_app");
    let r = if abrir_app {
        r#"{"v":1,"tipo":"ok","dados":{}}"#.to_owned()
    } else {
        format!(
            r#"{{"v":1,"tipo":"estado","dados":{{"travado":true,"cofre_existe":false,"papel":"{papel}"}}}}"#
        )
    };
    let _ = stream
        .write_all(&(r.len() as u32).to_be_bytes())
        .and_then(|()| stream.write_all(r.as_bytes()));
    true
}

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

fn cliente(args: &[String]) {
    let mut socket: Option<PathBuf> = None;
    let mut pausar = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--sistema-em" => {
                socket = args.get(i + 1).map(PathBuf::from);
                i += 2;
            }
            "--pausar" => {
                pausar = true;
                i += 1;
            }
            _ => falhar("argumento"),
        }
    }
    let esperado = match socket {
        Some(s) => ExpectedServer::system_at(s),
        None => ExpectedServer::system(),
    };
    let marca = std::env::var_os("LD_PRELOAD").is_some();
    if pausar {
        println!("pronto gid={}", status("Gid:"));
        let _ = std::io::stdout().flush();
        let mut linha = String::new();
        let _ = std::io::stdin().read_line(&mut linha);
    }
    let stream: UnixStream = match connect_server(&esperado) {
        Ok(s) => s,
        Err(e) => {
            println!("servidor={e:?}");
            return;
        }
    };
    let mut resposta = String::new();
    let _ = BufReader::new(stream).read_line(&mut resposta);
    println!(
        "servidor=ok {} gid={} dumpable={} ld_preload_no_ambiente={marca}",
        resposta.trim(),
        status("Gid:"),
        dumpable()
    );
}

#[allow(unsafe_code)]
fn dumpable() -> i32 {
    // SAFETY: `PR_GET_DUMPABLE` não recebe ponteiro; devolve o valor no retorno.
    unsafe { libc::prctl(libc::PR_GET_DUMPABLE, 0, 0, 0, 0) }
}

fn erro_de(r: libc::c_long) -> String {
    if r == 0 {
        "ok".to_owned()
    } else {
        let e = std::io::Error::last_os_error();
        match e.raw_os_error() {
            Some(libc::EPERM) => "EPERM".to_owned(),
            Some(libc::EACCES) => "EACCES".to_owned(),
            _ => format!("{e}"),
        }
    }
}

#[allow(unsafe_code)]
fn prender(args: &[String]) {
    let Some((programa, resto)) = args.split_first() else {
        falhar("prender <programa…>");
    };
    let mut filho = Command::new(programa)
        .args(resto)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| falhar(e));
    let mut saida = BufReader::new(filho.stdout.take().unwrap());
    let mut pronto = String::new();
    saida.read_line(&mut pronto).unwrap_or_else(|e| falhar(e));
    let pid = libc::pid_t::try_from(filho.id()).unwrap_or_else(|_| falhar("pid"));
    // SAFETY: `PTRACE_ATTACH` num pid nosso, sem ponteiros (endereço e dado nulos).
    let attach = unsafe {
        libc::ptrace(
            libc::PTRACE_ATTACH,
            pid,
            std::ptr::null_mut::<libc::c_void>(),
            std::ptr::null_mut::<libc::c_void>(),
        )
    };
    let attach = erro_de(attach);
    if attach == "ok" {
        // Controle (binário sem setgid): espera a parada do attach e solta o filho.
        let mut estado: libc::c_int = 0;
        // SAFETY: `waitpid` de um filho nosso, com `estado` local.
        unsafe { libc::waitpid(pid, &raw mut estado, 0) };
        // SAFETY: solta o filho parado, sem sinal (dado nulo).
        unsafe {
            libc::ptrace(
                libc::PTRACE_DETACH,
                pid,
                std::ptr::null_mut::<libc::c_void>(),
                std::ptr::null_mut::<libc::c_void>(),
            )
        };
    }
    // SAFETY: como acima, com `PTRACE_SEIZE` e opções 0.
    let seize = unsafe {
        libc::ptrace(
            libc::PTRACE_SEIZE,
            pid,
            std::ptr::null_mut::<libc::c_void>(),
            std::ptr::null_mut::<libc::c_void>(),
        )
    };
    let seize = erro_de(seize);
    let mem = match std::fs::File::open(format!("/proc/{pid}/mem")) {
        Ok(_) => "ok".to_owned(),
        Err(e) if e.raw_os_error() == Some(libc::EACCES) => "EACCES".to_owned(),
        Err(e) if e.raw_os_error() == Some(libc::EPERM) => "EPERM".to_owned(),
        Err(e) => format!("{e}"),
    };
    let tracer = std::fs::read_to_string(format!("/proc/{pid}/status"))
        .unwrap_or_default()
        .lines()
        .find_map(|l| l.strip_prefix("TracerPid:").map(|v| v.trim().to_owned()))
        .unwrap_or_default();
    let _ = filho.stdin.take().unwrap().write_all(b"\n");
    let mut resto_saida = String::new();
    let _ = saida.read_to_string(&mut resto_saida);
    let _ = filho.wait();
    println!(
        "prender: {} attach={attach} seize={seize} mem={mem} tracer={tracer} | {}",
        pronto.trim(),
        resto_saida.trim()
    );
}

#[allow(unsafe_code)]
fn traceme(args: &[String]) {
    use std::os::unix::process::CommandExt;
    let Some((programa, resto)) = args.split_first() else {
        falhar("traceme <programa…>");
    };
    let mut cmd = Command::new(programa);
    cmd.args(resto)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .stdout(Stdio::piped());
    // SAFETY: o fechamento roda no filho entre o fork e o exec e só chama `ptrace`
    // (async-signal-safe), sem alocar nem tocar em trava.
    unsafe {
        cmd.pre_exec(|| {
            let r = libc::ptrace(
                libc::PTRACE_TRACEME,
                0,
                std::ptr::null_mut::<libc::c_void>(),
                std::ptr::null_mut::<libc::c_void>(),
            );
            if r != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut filho = cmd.spawn().unwrap_or_else(|e| falhar(e));
    let pid = libc::pid_t::try_from(filho.id()).unwrap_or_else(|_| falhar("pid"));
    let mut estado: libc::c_int = 0;
    // SAFETY: `waitpid` de um filho nosso, com `estado` local.
    let r = unsafe { libc::waitpid(pid, &raw mut estado, 0) };
    if r != pid || !libc::WIFSTOPPED(estado) {
        falhar("o filho não parou no exec");
    }
    let gid = std::fs::read_to_string(format!("/proc/{pid}/status"))
        .unwrap_or_default()
        .lines()
        .find_map(|l| {
            l.strip_prefix("Gid:")
                .map(|v| v.split_whitespace().collect::<Vec<_>>().join(","))
        })
        .unwrap_or_default();
    // SAFETY: solta o filho parado, sem sinal (dado 0).
    let r = unsafe {
        libc::ptrace(
            libc::PTRACE_DETACH,
            pid,
            std::ptr::null_mut::<libc::c_void>(),
            std::ptr::null_mut::<libc::c_void>(),
        )
    };
    if r != 0 {
        falhar("PTRACE_DETACH");
    }
    let mut saida = String::new();
    let _ = filho.stdout.take().unwrap().read_to_string(&mut saida);
    let _ = filho.wait();
    println!("traceme: gid_no_exec={gid} | {}", saida.trim());
}

fn bind(args: &[String]) {
    let [socket] = args else {
        falhar("bind <socket>")
    };
    match UnixListener::bind(socket) {
        Ok(_) => {
            let _ = std::fs::remove_file(socket);
            println!("bind=ok");
        }
        Err(e) if e.raw_os_error() == Some(libc::EACCES) => println!("bind=EACCES"),
        Err(e) => println!("bind={e}"),
    }
}

/// Abre os apps pedidos (Fase 29a). Devolve os lançadores, para os filhos não serem
/// colhidos antes da hora.
fn abrir_apps(aberturas: &[Abertura], inst: &Install) -> Vec<Launcher> {
    let mut lancadores = Vec::new();
    for a in aberturas {
        match a {
            Abertura::Launcher(bin) => {
                let b = AllowedBinary::new(bin).unwrap_or_else(|e| falhar(e));
                let mut l = Launcher::new(b, inst);
                match l.open() {
                    Ok(pid) => println!("lancador: aberto pid={pid}"),
                    Err(e) => println!("lancador: erro {e:?}"),
                }
                match l.open() {
                    Ok(pid) => println!("lancador: segundo aberto pid={pid}"),
                    Err(e) => println!("lancador: segundo {e:?}"),
                }
                lancadores.push(l);
            }
            Abertura::Com(bin, var) => {
                let Some((nome, valor)) = var.split_once('=') else {
                    falhar("VAR=valor");
                };
                let amb = app_environment().unwrap_or_else(|e| falhar(e));
                let filho = Command::new(bin)
                    .env_clear()
                    .envs(amb.variables().filter(|(k, _)| *k != nome))
                    .env(nome, valor)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn();
                match filho {
                    Ok(f) => println!("forcado: aberto pid={} {nome}", f.id()),
                    Err(e) => println!("forcado: erro {e}"),
                }
            }
        }
        let _ = std::io::stdout().flush();
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    lancadores
}

fn lancar_app(args: &[String]) {
    let [bin] = args else {
        falhar("lancar-app <bin>")
    };
    let chrome = AllowedBinary::new("/usr/bin/bash").unwrap_or_else(|e| falhar(e));
    let inst = Install::from_system(chrome).unwrap_or_else(|e| falhar(e));
    let b = AllowedBinary::new(bin).unwrap_or_else(|e| falhar(e));
    match Launcher::new(b, &inst).open() {
        Ok(pid) => println!("lancar-app: aberto pid={pid}"),
        Err(e) => println!(
            "lancar-app: {e:?} indisponivel={}",
            matches!(&e, ward_platform::LauncherError::Session(s) if s.session_unavailable())
        ),
    }
}

fn escutar(args: &[String]) {
    let [socket] = args else {
        falhar("escutar <socket>")
    };
    let _ = std::fs::remove_file(socket);
    let _l = UnixListener::bind(socket).unwrap_or_else(|e| falhar(e));
    println!("escutando");
    let _ = std::io::stdout().flush();
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}

fn ambiente() {
    let amb = app_environment().unwrap_or_else(|e| falhar(e));
    for (k, v) in amb.variables() {
        println!("{k}={v}");
    }
}

/// Imita o `systemd --user` (Fase 29a, V5.31): `Name: systemd`, `cmdline`
/// `/usr/lib/systemd/systemd --user`, órfão (`PPid: 1`). Abre o programa com o
/// ambiente de D57, como uma unit de `~/.config/systemd/user/` faria.
fn gerente() {
    let programa =
        std::env::var("WARD_GERENTE_PROGRAMA").unwrap_or_else(|_| falhar("WARD_GERENTE_PROGRAMA"));
    // O subshell que deixou este processo órfão sai logo; espera o `PPid` virar 1.
    std::thread::sleep(std::time::Duration::from_millis(300));
    let cmdline = std::fs::read("/proc/self/cmdline").unwrap_or_default();
    println!(
        "gerente: nome={} ppid={} cmdline={}",
        status("Name:"),
        status("PPid:"),
        String::from_utf8_lossy(&cmdline).replace('\0', " ").trim()
    );
    let amb = app_environment().unwrap_or_else(|e| falhar(e));
    let status = Command::new(programa)
        .env_clear()
        .envs(amb.variables())
        .stdin(Stdio::null())
        .status();
    println!("gerente: {status:?}");
}
