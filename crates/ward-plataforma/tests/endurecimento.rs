//! Endurecimento do processo (V3.6, V3.7; S8).
//!
//! O processo endurecido é este mesmo binário de teste, chamado de novo com
//! `--exact filho_auxiliar` e a variável `WARD_PLATAFORMA_FILHO` dizendo o que fazer.
//! Ataques: ler `/proc/<pid>/mem` e `environ`, `ptrace` (inclusive pelo pai, que o
//! Yama deixaria), `kill -SEGV` procurando core, subir com `LD_*` ou sob `strace`.

mod comum;

use std::io::{BufRead, BufReader, Read};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const MODO: &str = "WARD_PLATAFORMA_FILHO";
const MARCA: &str = "RESULTADO:";

/// Não é teste de verdade: é o corpo do processo filho. Sem a variável, não faz nada.
#[test]
fn filho_auxiliar() {
    let Ok(modo) = std::env::var(MODO) else {
        return;
    };
    match modo.as_str() {
        "endurecer" | "endurecer-segv" | "sem-endurecer" => {
            if modo != "sem-endurecer" {
                ward_plataforma::endurecer_processo().unwrap();
            }
            if modo == "endurecer-segv" {
                sigsegv_padrao();
            }
            println!("{MARCA}pronto");
            // Espera o stdin fechar.
            let _ = std::io::stdin().read_to_end(&mut Vec::new());
        }
        "iniciar" => match ward_plataforma::iniciar_endurecido() {
            Ok(()) => println!("{MARCA}ok"),
            Err(e) => {
                println!("{MARCA}erro:{e}");
                std::process::exit(3);
            }
        },
        _ => panic!("modo desconhecido"),
    }
}

/// O `std` do Rust instala um tratador de SIGSEGV (para estouro de pilha) que, num
/// SIGSEGV mandado por `kill`, só volta e deixa o processo vivo. Para o teste do core,
/// o filho volta à ação padrão (morrer e, se permitido, gerar core).
#[allow(unsafe_code)]
fn sigsegv_padrao() {
    // SAFETY: troca a ação de um sinal pela padrão; nenhum ponteiro de função nosso.
    let r = unsafe { libc::signal(libc::SIGSEGV, libc::SIG_DFL) };
    assert_ne!(r, libc::SIG_ERR);
}

fn comando_filho(programa: Option<&Path>, modo: &str) -> Command {
    let eu = std::env::current_exe().unwrap();
    let mut c = match programa {
        Some(p) => {
            let mut c = Command::new(p);
            c.args(["-f", "-o", "/dev/null"]).arg(&eu);
            c
        }
        None => Command::new(&eu),
    };
    c.args([
        "--exact",
        "filho_auxiliar",
        "--nocapture",
        "--test-threads=1",
    ])
    .env_clear()
    .env("PATH", "/usr/bin:/bin")
    .env(MODO, modo)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
    c
}

/// Lê o stdout do filho até a linha com a marca e devolve o resto dela.
fn resultado(saida: &mut impl BufRead) -> String {
    let mut linha = String::new();
    let mut lidas = String::new();
    loop {
        linha.clear();
        assert!(
            saida.read_line(&mut linha).unwrap() > 0,
            "filho saiu sem resultado; saída: {lidas:?}"
        );
        lidas.push_str(&linha);
        // O libtest escreve "test filho_auxiliar ... " antes, na mesma linha.
        if let Some((_, r)) = linha.trim().split_once(MARCA) {
            return r.to_owned();
        }
    }
}

struct Endurecido {
    filho: std::process::Child,
    _saida: BufReader<std::process::ChildStdout>,
}

fn subir(modo: &str, cwd: Option<&Path>) -> Endurecido {
    let mut c = comando_filho(None, modo);
    if let Some(d) = cwd {
        c.current_dir(d);
    }
    let mut filho = c.spawn().unwrap();
    let mut saida = BufReader::new(filho.stdout.take().unwrap());
    assert_eq!(resultado(&mut saida), "pronto");
    Endurecido {
        filho,
        _saida: saida,
    }
}

fn ler_proc(pid: u32, arquivo: &str) -> std::io::Result<Vec<u8>> {
    let mut buf = vec![0u8; 64];
    let mut f = std::fs::File::open(format!("/proc/{pid}/{arquivo}"))?;
    let n = f.read(&mut buf)?;
    buf.truncate(n);
    Ok(buf)
}

#[allow(unsafe_code)]
fn ptrace_seize(pid: u32) -> std::io::Result<()> {
    let pid = libc::pid_t::try_from(pid).unwrap();
    // SAFETY: `PTRACE_SEIZE` só recebe inteiros (pid, endereço 0, opções 0); não toca
    // memória deste processo.
    let r = unsafe {
        libc::ptrace(
            libc::PTRACE_SEIZE,
            pid,
            std::ptr::null_mut::<libc::c_void>(),
            std::ptr::null_mut::<libc::c_void>(),
        )
    };
    if r != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // Se por acaso conseguiu, solta antes de seguir.
    // SAFETY: como acima; `PTRACE_DETACH` com sinal 0.
    unsafe {
        libc::ptrace(
            libc::PTRACE_DETACH,
            pid,
            std::ptr::null_mut::<libc::c_void>(),
            std::ptr::null_mut::<libc::c_void>(),
        );
    }
    Ok(())
}

#[test]
fn ataque_ler_memoria_ambiente_e_ptrace_do_processo_endurecido() {
    // Controle: sem endurecer, o mesmo uid lê o ambiente (o ataque funcionaria).
    let mut controle = subir("sem-endurecer", None);
    assert!(ler_proc(controle.filho.id(), "environ").is_ok());
    drop(controle.filho.stdin.take());
    controle.filho.wait().unwrap();

    let mut e = subir("endurecer", None);
    let pid = e.filho.id();
    for arquivo in ["mem", "environ", "maps"] {
        let r = ler_proc(pid, arquivo);
        assert_eq!(
            r.as_ref().map_err(|e| e.kind()).err(),
            Some(std::io::ErrorKind::PermissionDenied),
            "/proc/<pid>/{arquivo} legível"
        );
    }
    // O teste é o pai do filho: o Yama (ptrace_scope 1) deixaria; o dumpable=0 barra.
    let r = ptrace_seize(pid);
    assert_eq!(r.map_err(|e| e.raw_os_error()), Err(Some(libc::EPERM)));
    // `strace -p` e `gdb -p` dependem do mesmo ptrace.
    let st = Command::new("/usr/bin/strace")
        .args(["-o", "/dev/null", "-p", &pid.to_string()])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(!st.success(), "strace -p conseguiu");

    // Core desligado: `Max core file size 0 0`.
    let limits = std::fs::read_to_string(format!("/proc/{pid}/limits")).unwrap();
    let core = limits
        .lines()
        .find(|l| l.starts_with("Max core file size"))
        .unwrap();
    let campos: Vec<&str> = core.split_whitespace().collect();
    assert_eq!(&campos[4..6], &["0", "0"], "{core}");

    drop(e.filho.stdin.take());
    e.filho.wait().unwrap();
}

#[test]
fn ataque_sigsegv_nao_deixa_core() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("ward-plataforma-core-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut e = subir("endurecer-segv", Some(&dir));
    let pid = libc::pid_t::try_from(e.filho.id()).unwrap();
    // `kill` por /usr/bin/kill, sem unsafe aqui.
    let st = Command::new("/usr/bin/kill")
        .args(["-SEGV", &pid.to_string()])
        .status()
        .unwrap();
    assert!(st.success());
    let fim = e.filho.wait().unwrap();
    assert_eq!(fim.signal(), Some(libc::SIGSEGV));
    assert!(!fim.core_dumped(), "o kernel gerou core");
    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        0,
        "arquivo de core"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn iniciar_com(var: Option<(&str, &str)>, sob_strace: bool) -> (String, Option<i32>) {
    let strace = Path::new("/usr/bin/strace");
    let mut c = comando_filho(sob_strace.then_some(strace), "iniciar");
    if let Some((k, v)) = var {
        c.env(k, v);
    }
    let mut filho = c.spawn().unwrap();
    let mut saida = BufReader::new(filho.stdout.take().unwrap());
    let r = resultado(&mut saida);
    drop(filho.stdin.take());
    let st = filho.wait().unwrap();
    (r, st.code())
}

#[test]
fn sobe_limpo_sem_ld_e_sem_depurador() {
    let (r, codigo) = iniciar_com(None, false);
    assert_eq!(r, "ok");
    assert_eq!(codigo, Some(0));
}

#[test]
fn ataque_subir_com_ld_recusa() {
    for (nome, valor) in [
        ("LD_PRELOAD", ""),
        ("LD_AUDIT", ""),
        ("LD_LIBRARY_PATH", "/nao-existe-ward"),
    ] {
        let (r, codigo) = iniciar_com(Some((nome, valor)), false);
        assert!(r.starts_with("erro:"), "{nome}: {r}");
        assert!(r.contains(nome), "{r}");
        assert!(!r.contains("/nao-existe-ward"), "valor na mensagem: {r}");
        assert_ne!(codigo, Some(0));
    }
}

#[test]
fn ataque_subir_sob_strace_recusa() {
    let (r, codigo) = iniciar_com(None, true);
    assert!(r.starts_with("erro:") && r.contains("depurador"), "{r}");
    assert_ne!(codigo, Some(0));
}
