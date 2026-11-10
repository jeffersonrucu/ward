//! Apoio dos testes de integração do `ward-platform`. Nada aqui toca no cofre real:
//! os diretórios de teste ficam em `/run/user/<uid>/ward-platform-teste-*` (tmpfs, o
//! mesmo lugar do socket de verdade, para valerem as mesmas regras de pasta) e são
//! apagados no fim; cópias de binário ficam em `CARGO_TARGET_TMPDIR`.

#![allow(dead_code)]

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use ward_platform::AllowedBinary;

static CONTADOR: AtomicU32 = AtomicU32::new(0);

/// Diretório só deste teste dentro de `/run/user/<uid>/`, apagado ao sair.
pub struct DirTeste {
    pub caminho: PathBuf,
}

impl DirTeste {
    /// Caminho ainda não criado.
    pub fn novo() -> DirTeste {
        let n = CONTADOR.fetch_add(1, Ordering::SeqCst);
        let caminho = ward_platform::socket::diretorio_runtime()
            .join(format!("ward-platform-teste-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&caminho);
        DirTeste { caminho }
    }
}

impl Drop for DirTeste {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.caminho);
    }
}

/// O python do sistema, pelo arquivo real (`/usr/bin/python3` é link).
pub fn python_real() -> PathBuf {
    std::fs::canonicalize("/usr/bin/python3").expect("python3 do sistema")
}

pub fn permitido(caminho: &Path) -> AllowedBinary {
    AllowedBinary::new(caminho.to_path_buf()).unwrap()
}

/// Cliente python que conecta em `socket`, escreve "conectado" e espera o stdin fechar.
/// `biblioteca`: carregada por `ctypes` antes de conectar. Ambiente limpo (o cargo põe
/// `LD_LIBRARY_PATH` no ambiente dos testes) mais `extra`.
pub const CLIENTE_PY: &str = r#"
import socket, sys
if len(sys.argv) > 2:
    import ctypes
    ctypes.CDLL(sys.argv[2])
s = socket.socket(socket.AF_UNIX)
s.connect(sys.argv[1])
sys.stdout.write("conectado\n"); sys.stdout.flush()
sys.stdin.read()
"#;

pub struct Filho {
    pub filho: Child,
    pub saida: BufReader<ChildStdout>,
}

impl Filho {
    pub fn pid(&self) -> u32 {
        self.filho.id()
    }

    /// Espera a linha `esperada` no stdout do filho.
    pub fn esperar(&mut self, esperada: &str) {
        let mut linha = String::new();
        loop {
            linha.clear();
            let n = self.saida.read_line(&mut linha).unwrap();
            assert!(n > 0, "filho saiu sem escrever {esperada:?}");
            if linha.trim() == esperada {
                return;
            }
        }
    }

    /// Fecha o stdin (o filho sai) e espera.
    pub fn encerrar(mut self) {
        drop(self.filho.stdin.take());
        let _ = self.filho.kill();
        let _ = self.filho.wait();
    }
}

/// Monta o `Command` com ambiente limpo.
pub fn comando_limpo(programa: &Path) -> Command {
    let mut c = Command::new(programa);
    c.env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    c
}

pub fn iniciar(mut c: Command) -> Filho {
    let mut filho = subir_com_nova_tentativa(&mut c);
    let saida = BufReader::new(filho.stdout.take().unwrap());
    Filho { filho, saida }
}

/// `spawn` que repete em `ETXTBSY` ("Text file busy"). Um binário recém-copiado (o
/// `fs::copy` dos ataques) pode estar com o fd de escrita aberto em um filho que outra
/// thread de teste criou por `fork` e que ainda não chegou ao `execve` (o `O_CLOEXEC` só
/// fecha no `execve`); nessa janela, executar a cópia dá `ETXTBSY`. A janela é curta:
/// até 100 tentativas com 20 ms entre elas. Qualquer outro erro falha na hora.
pub fn subir_com_nova_tentativa(c: &mut Command) -> Child {
    let mut tentativas = 0;
    loop {
        match c.spawn() {
            Ok(f) => return f,
            Err(e) if e.raw_os_error() == Some(libc::ETXTBSY) && tentativas < 100 => {
                tentativas += 1;
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(e) => panic!("subir filho: {e}"),
        }
    }
}

/// Copia um binário para `destino` e sincroniza a cópia antes de fechar o fd de
/// escrita (o `ETXTBSY` que sobra vem de fd herdado por `fork`; ver
/// [`subir_com_nova_tentativa`]).
pub fn copiar_binario(origem: &Path, destino: &Path) {
    std::fs::copy(origem, destino).unwrap();
    std::fs::File::open(destino).unwrap().sync_all().unwrap();
}

/// `python -I -c CLIENTE_PY <socket> [biblioteca]` pelo `programa` dado.
pub fn cliente_python(programa: &Path, socket: &Path, biblioteca: Option<&Path>) -> Command {
    let mut c = comando_limpo(programa);
    c.arg("-I").arg("-c").arg(CLIENTE_PY).arg(socket);
    if let Some(b) = biblioteca {
        c.arg(b);
    }
    c
}
