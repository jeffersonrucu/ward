//! Fase 55 (S8; achado médio da reauditoria final): descendente que sai do grupo do
//! `ward run` não sobrevive ao fim do comando nem à trava.
//!
//! Binário de teste só deste arquivo (um processo só dele): o processo vira subreaper,
//! como o `wardd`. Um `#[test]` só, com os cenários em sequência, para nenhum outro
//! teste em paralelo ter filhos neste processo.
//!
//! Ataques (os dois passos da reauditoria), primeiro sem o subreaper (controle: o
//! fugitivo sobrevive) e depois com ele (morre):
//! 1. `setsid -f`: o comando cria um neto com sessão própria, o pai intermediário sai e
//!    o comando termina; o neto fica com o ambiente;
//! 2. `setpgid(0, 0)`: o filho sai do grupo (fica na sessão) e o comando termina.
//!
//! E o que não pode morrer: a árvore de outra sessão em curso e o órfão de um filho
//! comum deste processo (o app do `wardd`, que não faz `setsid`).

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use ward_plataforma::descendentes;
use ward_plataforma::filho::{Comando, Fim, Grupo, Pipes};

/// `python3 -I -c` com um modo:
/// - `setsid`: neto com `setsid` por dois `fork` (o do meio sai), imprime o pid dele e
///   sai (ou, com `fica`, dorme);
/// - `setpgid`: filho com `setpgid(0, 0)`, imprime o pid dele e sai;
/// - `arvore`: filho no mesmo grupo, imprime o pid e dorme.
const PY: &str = r#"
import os, sys, time
modo = sys.argv[1]
fica = len(sys.argv) > 2
r, w = os.pipe()
if modo == "setsid":
    a = os.fork()
    if a == 0:
        b = os.fork()
        if b == 0:
            os.setsid()
            os.write(w, (str(os.getpid()) + "\n").encode())
            time.sleep(60)
            os._exit(0)
        os._exit(0)
    os.waitpid(a, 0)
elif modo == "setpgid":
    a = os.fork()
    if a == 0:
        os.setpgid(0, 0)
        os.write(w, (str(os.getpid()) + "\n").encode())
        time.sleep(60)
        os._exit(0)
else:
    a = os.fork()
    if a == 0:
        os.write(w, (str(os.getpid()) + "\n").encode())
        time.sleep(60)
        os._exit(0)
print(os.read(r, 32).decode().strip(), flush=True)
if fica:
    time.sleep(60)
"#;

fn lancar(modo: &str, fica: bool) -> (Grupo, Pipes, i32) {
    let mut c = Comando::novo(b"/usr/bin/python3", b"/").unwrap();
    for a in [&b"-I"[..], b"-c", PY.as_bytes(), modo.as_bytes()] {
        c.argumento(a).unwrap();
    }
    if fica {
        c.argumento(b"fica").unwrap();
    }
    c.variavel(b"WARD_PASS", b"falso-senha-f55").unwrap();
    let (g, mut p) = c.lancar().unwrap();
    let mut linha = [0u8; 32];
    let n = p.saida.as_mut().unwrap().read(&mut linha).unwrap();
    let pid = std::str::from_utf8(&linha[..n])
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    (g, p, pid)
}

/// Fim normal do comando, como o `wardd`: espera, mata a sessão, colhe.
fn fim_do_comando(g: &Grupo) -> descendentes::Relatorio {
    g.esperar_fim().unwrap();
    let r = g.matar_com_sessoes_filhas();
    let _ = g.colher().unwrap();
    r
}

fn stat(pid: i32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()
}

fn vivo(pid: i32) -> bool {
    stat(pid).is_some_and(|s| !s.contains(") Z "))
}

fn morre(pid: i32) -> bool {
    let fim = Instant::now() + Duration::from_secs(5);
    while vivo(pid) {
        if Instant::now() > fim {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    true
}

fn matar(pid: i32) {
    let _ = Command::new("/bin/kill")
        .args(["-9", &pid.to_string()])
        .status();
    assert!(morre(pid));
}

#[test]
fn ataque_f55_descendente_fora_do_grupo_nao_sobrevive() {
    // Controle, sem o subreaper: os dois fugitivos sobrevivem ao fim do comando.
    assert!(!descendentes::e_subreaper());
    for modo in ["setsid", "setpgid"] {
        let (g, _p, fugitivo) = lancar(modo, false);
        let r = fim_do_comando(&g);
        assert_eq!(r.sobraram, 0, "{r:?}");
        std::thread::sleep(Duration::from_millis(300));
        assert!(vivo(fugitivo), "controle: {modo} não fugiu");
        matar(fugitivo);
    }

    descendentes::virar_subreaper().unwrap();
    assert!(descendentes::e_subreaper());

    // 1 e 2: com o subreaper, o fugitivo morre no fim do comando e é colhido.
    for modo in ["setsid", "setpgid"] {
        let (g, _p, fugitivo) = lancar(modo, false);
        let r = fim_do_comando(&g);
        assert_eq!(r.sobraram, 0, "{r:?}");
        assert!(r.mortos >= 1, "{modo}: {r:?}");
        assert!(morre(fugitivo), "{modo}: o fugitivo sobreviveu ao fim");
        let fim = Instant::now() + Duration::from_secs(5);
        while stat(fugitivo).is_some() {
            assert!(Instant::now() < fim, "{modo}: o zumbi não foi colhido");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    // A trava com o comando vivo: a sessão inteira morre (o fugitivo com `setsid`
    // também, pelo grupo e pela varredura dos órfãos).
    let (g, _p, fugitivo) = lancar("setsid", true);
    let r = g.matar_com_sessoes_filhas();
    assert_eq!(r.sobraram, 0, "{r:?}");
    assert_eq!(g.sobraram(), 0);
    assert_eq!(g.colher().unwrap(), Fim::Sinal(libc::SIGKILL));
    assert!(morre(fugitivo), "trava: o fugitivo sobreviveu");

    // A trava sem sessão (só os órfãos): o fugitivo de uma sessão viva morre; a sessão
    // em si (líder e o filho no grupo) continua.
    let (g, _p, fugitivo) = lancar("setsid", true);
    let (outra, _q, filho_da_outra) = lancar("arvore", true);
    let r = descendentes::matar_orfaos();
    assert_eq!(r.sobraram, 0, "{r:?}");
    assert!(morre(fugitivo), "matar_orfaos: o fugitivo sobreviveu");
    assert!(vivo(g.pid() as i32), "matar_orfaos matou o líder");
    assert!(
        vivo(filho_da_outra),
        "matar_orfaos matou a árvore de outra sessão"
    );

    // O fim de uma sessão não leva a árvore de outra em curso.
    let r = g.matar_com_sessoes_filhas();
    assert_eq!(r.sobraram, 0, "{r:?}");
    let _ = g.colher();
    assert!(vivo(filho_da_outra), "o fim de uma sessão matou outra");
    assert!(vivo(outra.pid() as i32));

    // O órfão de um filho comum deste processo (sem `setsid`: a sessão daqui, como o
    // app do `wardd`) não é de sessão nenhuma e não leva sinal.
    let mut comum = Command::new("/usr/bin/python3")
        .args(["-I", "-c", PY, "arvore"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    // Uma linha só: o neto segura a saída aberta.
    let mut linha = String::new();
    std::io::BufRead::read_line(
        &mut std::io::BufReader::new(comum.stdout.take().unwrap()),
        &mut linha,
    )
    .unwrap();
    let neto_comum: i32 = linha.trim().parse().unwrap();
    let _ = comum.wait();
    let fim = Instant::now() + Duration::from_secs(5);
    while !stat(neto_comum).is_some_and(|s| s.contains(&format!(") S {} ", std::process::id()))) {
        assert!(
            Instant::now() < fim,
            "o neto comum não virou filho deste processo"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let r = outra.matar_com_sessoes_filhas();
    assert_eq!(r.sobraram, 0, "{r:?}");
    let _ = outra.colher();
    assert!(morre(filho_da_outra));
    assert!(vivo(neto_comum), "o órfão de um filho comum levou o sinal");
    let r = descendentes::matar_orfaos();
    assert_eq!(r.mortos, 0, "{r:?}");
    assert!(vivo(neto_comum));
    matar(neto_comum);
}
