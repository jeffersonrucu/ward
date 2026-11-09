//! Binário de teste do contêiner da Fase 21b (`make teste-docker-plataforma`; D46, D47;
//! V3.5, V3.23, V3.24). Só com `modo-teste`; nunca é instalado fora do contêiner.
//!
//! Um binário só, com subcomandos, instalado com nomes diferentes:
//! - `ativar <socket> <uid> -- <programa…>`: o papel do systemd do sistema (roda como
//!   root): cria o socket, passa para o uid, 0600, e executa o programa como esse uid
//!   (`setpriv`) com o socket no fd 3 e `LISTEN_FDS=1`;
//! - `servidor --chrome <exe> [--lancar <programa…>]`: o "wardd" de teste: endurece,
//!   recebe o socket por `LISTEN_FDS`, lê os grupos do `/etc/group` e responde a cada
//!   conexão uma linha com o papel; com `--lancar`, abre um filho (o app aberto pelo
//!   daemon);
//! - `cliente [--sistema-em <socket>] [--pausar]`: confere o servidor (D46), conecta e
//!   imprime o papel; instalado como `ward-app` e `ward-nm` (setgid);
//! - `prender <programa…>`: lança o programa (um `cliente --pausar`) e tenta
//!   `PTRACE_ATTACH`, `PTRACE_SEIZE` e ler o `/proc/<pid>/mem` dele;
//! - `traceme <programa…>`: lança o programa já sob `ptrace` (`PTRACE_TRACEME`) e mostra
//!   o egid depois do `exec`: sob depurador o kernel não aplica o setgid;
//! - `bind <socket>`: tenta criar um socket (o atacante na pasta do root).
//!
//! `unsafe` só para `ptrace`/`waitpid`/`prctl` dos ataques, com `// SAFETY:`.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ward_plataforma::{
    BinarioPermitido, ClassePar, Instalacao, PoliticaKernel, ServidorEsperado,
    caminho_socket_sistema, conectar_servidor, conferir_conexao, escutar_de_listen_fds,
    iniciar_endurecido,
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
        _ => {
            eprintln!("uso: ativar|servidor|cliente|prender|traceme|bind");
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
    ward_plataforma::teste::executar_com_socket(&listener, cmd);
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

fn servidor(args: &[String]) {
    let mut chrome = None;
    let mut lancar: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--chrome" => {
                chrome = args.get(i + 1).cloned();
                i += 2;
            }
            "--lancar" => {
                lancar = args[i + 1..].to_vec();
                break;
            }
            _ => falhar("argumento"),
        }
    }
    if let Err(e) = iniciar_endurecido() {
        falhar(e);
    }
    let escuta = escutar_de_listen_fds(&caminho_socket_sistema()).unwrap_or_else(|e| falhar(e));
    let chrome = BinarioPermitido::novo(chrome.unwrap_or_else(|| falhar("--chrome")))
        .unwrap_or_else(|e| falhar(e));
    let inst = Instalacao::do_sistema(chrome).unwrap_or_else(|e| falhar(e));
    let politica = PoliticaKernel::ler();
    println!(
        "servidor: pronto {} gid_app={} gid_nm={} politica={:?}",
        estado_dos_fds(escuta.listener()),
        inst.gid_app(),
        inst.gid_extensao(),
        politica.conferir()
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
    for conexao in escuta.listener().incoming() {
        let Ok(mut stream) = conexao else { continue };
        let par = match conferir_conexao(&stream) {
            Ok(p) => p,
            Err(e) => {
                println!("servidor: recusado {e:?}");
                let _ = std::io::stdout().flush();
                continue;
            }
        };
        let classe = par.classificar(&inst, &politica);
        let linha = match classe {
            ClassePar::App => "papel=app".to_owned(),
            ClassePar::HostExtensao => "papel=extensao".to_owned(),
            ClassePar::Comum { motivo } => format!("papel=agente motivo={motivo:?}"),
        };
        println!(
            "servidor: pid={} gid={} {linha}",
            par.credenciais().pid,
            par.credenciais().gid
        );
        let _ = std::io::stdout().flush();
        let _ = writeln!(stream, "{linha}");
    }
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
        Some(s) => ServidorEsperado::sistema_em(s),
        None => ServidorEsperado::sistema(),
    };
    let marca = std::env::var_os("LD_PRELOAD").is_some();
    if pausar {
        println!("pronto gid={}", status("Gid:"));
        let _ = std::io::stdout().flush();
        let mut linha = String::new();
        let _ = std::io::stdin().read_line(&mut linha);
    }
    let stream: UnixStream = match conectar_servidor(&esperado) {
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
