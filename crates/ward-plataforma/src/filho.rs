//! Filho do `ward run` (Fase 26; D13, D60; S2, S3, S8).
//!
//! O daemon cria o processo com o argv, o diretório e o ambiente que monta, e só eles:
//! - **ambiente exato**: nada do ambiente do daemon chega ao filho (`LISTEN_*`, D60);
//!   o `envp` é só o que [`Comando::variavel`] recebeu;
//! - **sem cópia solta de segredo** (S8): argv, ambiente e diretório ficam em
//!   `Zeroizing`, alocados uma vez no tamanho exato, e o `fork` + `execve` é feito aqui
//!   (`sys::fork_exec`), sem o `std::process::Command`, que copia o ambiente para
//!   buffers que não zeram;
//! - **sem shell** (S3): o argv vai direto ao `execve`; a busca no `PATH` (o `PATH` do
//!   ambiente do filho, como o `execvpe`) é feita antes, sem recurso ao `/bin/sh`;
//! - **grupo próprio**: o filho é líder de um grupo de processos, e [`Grupo::matar`]
//!   mata o grupo inteiro (netos inclusive) quando a CLI cai ou o comando termina;
//! - **morre junto**: `PR_SET_PDEATHSIG` com `SIGKILL`, ligado à thread do daemon que
//!   o criou (a da conexão, que vive até o fim do comando);
//! - **nenhum fd do daemon**: todo fd ≥ 3 fecha no `exec`; o filho recebe só os pipes
//!   em 0, 1 e 2;
//! - sinais no padrão e máscara vazia (o runtime do Rust ignora o `SIGPIPE`).
//!
//! O kernel deixa `/proc/<pid>/environ` do filho legível por outro processo do mesmo
//! uid enquanto ele roda (RA1, registrado no handoff da Fase 26): o `execve` volta o
//! `dumpable` a 1 para qualquer programa comum, e o `PR_SET_DUMPABLE` do daemon não
//! passa do `exec`.

use std::ffi::CStr;
use std::fs::File;
use std::io;
use std::sync::Mutex;

use zeroize::Zeroizing;

use crate::sys;

/// `PATH` do `execvpe` da glibc quando o ambiente não tem um.
const PATH_PADRAO: &[u8] = b"/bin:/usr/bin";

/// Por que o comando não foi montado. Só textos fixos (nunca o valor).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroComando {
    /// Programa ou diretório vazio.
    Vazio,
    /// Byte nulo num argumento, nome, valor ou diretório.
    ByteNulo,
    /// Nome de variável vazio ou com `=`.
    NomeInvalido,
}

impl std::fmt::Display for ErroComando {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ErroComando::Vazio => "programa ou diretório vazio",
            ErroComando::ByteNulo => "byte nulo no comando ou no ambiente",
            ErroComando::NomeInvalido => "nome de variável inválido",
        })
    }
}

impl std::error::Error for ErroComando {}

/// Por que o filho não foi criado. Sem valor do comando.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroLancar {
    /// Falha do daemon (pipe, `fork`).
    Sistema(io::ErrorKind),
    /// Preparo do filho (`prctl`, `dup2`, `close_range`), com o `errno`.
    Preparo(i32),
    /// O `chdir` no diretório da CLI falhou.
    Diretorio(i32),
    /// O `execve` falhou em todos os caminhos.
    Programa(i32),
}

impl ErroLancar {
    /// Código de saída que a CLI devolve, como o shell: 127 para "não encontrado", 126
    /// para "achado, mas não executável".
    pub fn codigo_de_saida(self) -> u8 {
        match self {
            ErroLancar::Programa(e) if e == libc::ENOENT || e == libc::ENOTDIR => 127,
            ErroLancar::Programa(_) => 126,
            _ => 127,
        }
    }

    /// Mensagem fixa em português.
    pub fn mensagem(self) -> &'static str {
        match self {
            ErroLancar::Programa(e) if e == libc::ENOENT || e == libc::ENOTDIR => {
                "comando não encontrado"
            }
            ErroLancar::Programa(e) if e == libc::EACCES => "sem permissão para executar o comando",
            ErroLancar::Programa(e) if e == libc::ENOEXEC => {
                "o arquivo não é um executável (sem `#!`?)"
            }
            ErroLancar::Programa(_) => "o comando não pôde ser executado",
            ErroLancar::Diretorio(_) => "o diretório de trabalho não existe ou não é acessível",
            ErroLancar::Preparo(_) | ErroLancar::Sistema(_) => "não deu para criar o processo",
        }
    }
}

impl std::fmt::Display for ErroLancar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.mensagem())
    }
}

impl std::error::Error for ErroLancar {}

/// Bytes com o nulo do fim, alocados uma vez no tamanho exato.
fn c_de(partes: &[&[u8]]) -> Result<Zeroizing<Vec<u8>>, ErroComando> {
    let n: usize = partes.iter().map(|p| p.len()).sum();
    let mut v = Zeroizing::new(Vec::with_capacity(n + 1));
    for p in partes {
        if p.contains(&0) {
            return Err(ErroComando::ByteNulo);
        }
        v.extend_from_slice(p);
    }
    v.push(0);
    Ok(v)
}

/// O comando a lançar: argv, diretório e o ambiente inteiro (nada vem do daemon).
pub struct Comando {
    argv: Vec<Zeroizing<Vec<u8>>>,
    ambiente: Vec<Zeroizing<Vec<u8>>>,
    cwd: Zeroizing<Vec<u8>>,
}

impl std::fmt::Debug for Comando {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Comando({} argumentos, {} variáveis, ••••)",
            self.argv.len(),
            self.ambiente.len()
        )
    }
}

impl Comando {
    /// `programa` é o `argv[0]`: com `/`, é o caminho (relativo ao `cwd`); sem `/`, é
    /// procurado no `PATH` do ambiente do filho.
    pub fn novo(programa: &[u8], cwd: &[u8]) -> Result<Comando, ErroComando> {
        if programa.is_empty() || cwd.is_empty() {
            return Err(ErroComando::Vazio);
        }
        Ok(Comando {
            argv: vec![c_de(&[programa])?],
            ambiente: Vec::new(),
            cwd: c_de(&[cwd])?,
        })
    }

    /// Mais um argumento.
    pub fn argumento(&mut self, a: &[u8]) -> Result<(), ErroComando> {
        self.argv.push(c_de(&[a])?);
        Ok(())
    }

    /// Mais uma variável do ambiente do filho (`nome=valor`).
    pub fn variavel(&mut self, nome: &[u8], valor: &[u8]) -> Result<(), ErroComando> {
        if nome.is_empty() || nome.contains(&b'=') {
            return Err(ErroComando::NomeInvalido);
        }
        self.ambiente.push(c_de(&[nome, b"=", valor])?);
        Ok(())
    }

    /// Os caminhos a tentar, como o `execvpe`.
    fn caminhos(&self) -> Vec<Zeroizing<Vec<u8>>> {
        let prog = &self.argv[0][..self.argv[0].len() - 1];
        if prog.contains(&b'/') {
            return vec![self.argv[0].clone()];
        }
        let path = self
            .ambiente
            .iter()
            .find_map(|v| v.strip_prefix(b"PATH="))
            .map(|v| &v[..v.len() - 1])
            .unwrap_or(PATH_PADRAO);
        path.split(|b| *b == b':')
            .filter_map(|dir| {
                let dir: &[u8] = if dir.is_empty() { b"." } else { dir };
                c_de(&[dir, b"/", prog]).ok()
            })
            .collect()
    }

    /// Cria o filho com três pipes. Volta depois do `execve` (ou da falha dele, com o
    /// motivo).
    pub fn lancar(&self) -> Result<(Grupo, Pipes), ErroLancar> {
        let (grupo, pontas) = self.lancar_com(sys::Stdio::Pipes)?;
        match pontas {
            sys::PontasPai::Pipes {
                entrada,
                saida,
                erro,
            } => Ok((
                grupo,
                Pipes {
                    entrada: Some(File::from(entrada)),
                    saida: Some(File::from(saida)),
                    erro: Some(File::from(erro)),
                },
            )),
            sys::PontasPai::Pty => Err(ErroLancar::Sistema(io::ErrorKind::InvalidData)),
        }
    }

    /// Cria o filho num pty novo (`ward ssh` interativo, Fase 27; D13): o filho é líder
    /// de uma sessão nova com o escravo como terminal de controle em 0, 1 e 2, e o
    /// tamanho da janela já vale no `exec`. O daemon fica só com o mestre.
    pub fn lancar_em_pty(&self, linhas: u16, colunas: u16) -> Result<(Grupo, Pty), ErroLancar> {
        let (mestre, escravo) = sys::abrir_pty().map_err(|e| ErroLancar::Sistema(e.kind()))?;
        sys::definir_janela(std::os::fd::AsFd::as_fd(&mestre), linhas, colunas)
            .map_err(|e| ErroLancar::Sistema(e.kind()))?;
        let (grupo, _) = self.lancar_com(sys::Stdio::Pty(escravo))?;
        Ok((
            grupo,
            Pty {
                mestre: File::from(mestre),
            },
        ))
    }

    fn lancar_com(&self, stdio: sys::Stdio) -> Result<(Grupo, sys::PontasPai), ErroLancar> {
        let caminhos = self.caminhos();
        let ptr = |v: &Zeroizing<Vec<u8>>| v.as_ptr().cast::<libc::c_char>();
        let lista_c: Vec<*const libc::c_char> = caminhos.iter().map(ptr).collect();
        let mut argv: Vec<*const libc::c_char> = self.argv.iter().map(ptr).collect();
        argv.push(std::ptr::null());
        let mut envp: Vec<*const libc::c_char> = self.ambiente.iter().map(ptr).collect();
        envp.push(std::ptr::null());
        let cwd = CStr::from_bytes_with_nul(&self.cwd)
            .map_err(|_| ErroLancar::Sistema(io::ErrorKind::InvalidInput))?;
        let preparo = sys::PreparoExec {
            caminhos: &lista_c,
            argv: &argv,
            envp: &envp,
            cwd,
        };
        match sys::fork_exec(&preparo, stdio) {
            Ok(f) => Ok((
                Grupo {
                    pid: f.pid,
                    colhido: Mutex::new(false),
                },
                f.pontas,
            )),
            Err(sys::ErroExec::Sistema(e)) => Err(ErroLancar::Sistema(e.kind())),
            Err(sys::ErroExec::Filho { etapa, errno }) => Err(match etapa {
                sys::ETAPA_DIRETORIO => ErroLancar::Diretorio(errno),
                sys::ETAPA_EXEC => ErroLancar::Programa(errno),
                _ => ErroLancar::Preparo(errno),
            }),
        }
    }
}

/// As pontas do daemon dos pipes do filho: escrita na entrada, leitura da saída e do
/// erro.
#[derive(Debug)]
pub struct Pipes {
    pub entrada: Option<File>,
    pub saida: Option<File>,
    pub erro: Option<File>,
}

/// O mestre do pty do filho: o daemon lê a saída e escreve a entrada nele.
#[derive(Debug)]
pub struct Pty {
    pub mestre: File,
}

impl Pty {
    /// Novo tamanho da janela (o kernel avisa o grupo da frente com `SIGWINCH`).
    pub fn definir_janela(&self, linhas: u16, colunas: u16) -> io::Result<()> {
        use std::os::fd::AsFd;
        sys::definir_janela(self.mestre.as_fd(), linhas, colunas)
    }
}

/// Como o filho terminou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fim {
    Codigo(u8),
    Sinal(i32),
}

impl Fim {
    /// Código de saída como o shell: o próprio, ou 128 + o sinal.
    pub fn codigo_de_saida(self) -> u8 {
        match self {
            Fim::Codigo(c) => c,
            Fim::Sinal(s) => u8::try_from(128 + s.clamp(0, 127)).unwrap_or(255),
        }
    }
}

/// Há o que ler no pipe (dados ou fim) dentro de `espera_ms`? Sem ler: a thread que lê a
/// saída do filho olha de tempos em tempos se deve parar (um neto fora do grupo pode
/// segurar o pipe aberto para sempre).
pub fn pronto_para_ler(f: &File, espera_ms: i32) -> io::Result<bool> {
    use std::os::fd::AsFd;
    sys::esperar_leitura(f.as_fd(), espera_ms)
}

/// Como [`pronto_para_ler`], para qualquer fd (o socket do agente SSH, Fase 27).
pub fn pronto_para_ler_fd(fd: std::os::fd::BorrowedFd<'_>, espera_ms: i32) -> io::Result<bool> {
    sys::esperar_leitura(fd, espera_ms)
}

/// O grupo de processos do filho (o filho é o líder). Pode ser usado de várias threads:
/// uma espera o fim, outra mata o grupo quando a CLI cai. Ao sair sem ter colhido, mata
/// o grupo e colhe (nenhum processo com segredo no ambiente fica para trás).
#[derive(Debug)]
pub struct Grupo {
    pid: libc::pid_t,
    /// Já colhido: o pid (e o grupo) pode ser de outro processo; nada de `kill`.
    colhido: Mutex<bool>,
}

impl Grupo {
    /// Pid do filho (= id do grupo).
    pub fn pid(&self) -> u32 {
        self.pid.unsigned_abs()
    }

    /// Espera o filho terminar, sem colher: até colher, o grupo continua com este id.
    pub fn esperar_fim(&self) -> io::Result<()> {
        sys::esperar_sem_colher(self.pid)
    }

    /// `SIGKILL` no grupo inteiro (o filho e o que ele criou no mesmo grupo). Depois de
    /// colhido, não faz nada.
    pub fn matar(&self) {
        let colhido = self.colhido.lock().unwrap_or_else(|e| e.into_inner());
        if !*colhido {
            let _ = sys::matar_grupo(self.pid);
        }
    }

    /// Espera o fim e colhe. Uma vez só.
    pub fn colher(&self) -> io::Result<Fim> {
        self.esperar_fim()?;
        let mut colhido = self.colhido.lock().unwrap_or_else(|e| e.into_inner());
        if *colhido {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let status = sys::colher(self.pid)?;
        *colhido = true;
        if libc::WIFSIGNALED(status) {
            Ok(Fim::Sinal(libc::WTERMSIG(status)))
        } else {
            Ok(Fim::Codigo(
                u8::try_from(libc::WEXITSTATUS(status) & 0xff).unwrap_or(255),
            ))
        }
    }
}

impl Drop for Grupo {
    fn drop(&mut self) {
        let colhido = *self.colhido.lock().unwrap_or_else(|e| e.into_inner());
        if !colhido {
            self.matar();
            let _ = self.colher();
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::io::{Read, Write};
    use std::os::fd::AsFd;
    use std::time::{Duration, Instant};

    fn sh(script: &str) -> Comando {
        let mut c = Comando::novo(b"/bin/sh", b"/").unwrap();
        c.argumento(b"-c").unwrap();
        c.argumento(script.as_bytes()).unwrap();
        c
    }

    fn rodar(c: &Comando) -> (Fim, Vec<u8>, Vec<u8>) {
        let (g, mut p) = c.lancar().unwrap();
        drop(p.entrada.take());
        let mut out = Vec::new();
        let mut err = Vec::new();
        p.saida.take().unwrap().read_to_end(&mut out).unwrap();
        p.erro.take().unwrap().read_to_end(&mut err).unwrap();
        (g.colher().unwrap(), out, err)
    }

    /// Lê o mestre do pty até o `EIO` do fim da sessão (ou o prazo).
    fn ler_pty(p: &mut Pty, prazo: Duration) -> Vec<u8> {
        let fim = Instant::now() + prazo;
        let mut out = Vec::new();
        let mut buf = [0u8; 4096];
        while Instant::now() < fim {
            if !pronto_para_ler(&p.mestre, 100).unwrap() {
                continue;
            }
            match p.mestre.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => out.extend_from_slice(&buf[..n]),
            }
        }
        out
    }

    #[test]
    fn pty_terminal_de_controle_janela_e_entrada() {
        // O filho é líder de sessão, tem o escravo como terminal de controle em 0–2, a
        // janela pedida vale desde o `exec` e muda depois (com `SIGWINCH`).
        let c = sh(
            "tty >/dev/null && echo T; [ \"$(ps -o sid= -p $$ | tr -d ' ')\" = $$ ] && echo S; \
             stty size; trap 'stty size' WINCH; echo pronto; read x || read x; echo \"lido:$x\"",
        );
        let (g, mut p) = c.lancar_em_pty(24, 80).unwrap();
        let mut visto = Vec::new();
        let fim = Instant::now() + Duration::from_secs(10);
        let mut buf = [0u8; 4096];
        while !String::from_utf8_lossy(&visto).contains("pronto") {
            assert!(Instant::now() < fim, "{}", String::from_utf8_lossy(&visto));
            if pronto_para_ler(&p.mestre, 100).unwrap() {
                let n = p.mestre.read(&mut buf).unwrap();
                visto.extend_from_slice(&buf[..n]);
            }
        }
        p.definir_janela(40, 132).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        p.mestre.write_all(b"oi\n").unwrap();
        visto.extend(ler_pty(&mut p, Duration::from_secs(10)));
        let s = String::from_utf8_lossy(&visto);
        assert!(s.contains("T\r\nS\r\n24 80"), "{s}");
        assert!(s.contains("40 132"), "{s}");
        assert!(s.contains("lido:oi"), "{s}");
        assert_eq!(g.colher().unwrap(), Fim::Codigo(0));
    }

    #[test]
    fn pty_matar_o_grupo_leva_a_sessao_e_o_ambiente_e_exato() {
        let mut c = sh("env; sleep 30 & sleep 30");
        c.variavel(b"SO", b"isto").unwrap();
        let (g, mut p) = c.lancar_em_pty(24, 80).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        g.matar();
        let out = ler_pty(&mut p, Duration::from_secs(5));
        assert_eq!(g.colher().unwrap(), Fim::Sinal(libc::SIGKILL));
        let s = String::from_utf8_lossy(&out);
        assert!(s.contains("SO=isto"), "{s}");
        // D60: nada do ambiente deste processo (o `sh` põe PWD, SHLVL, `_`).
        for linha in s.lines() {
            let nome = linha.split('=').next().unwrap();
            assert!(
                ["SO", "PWD", "SHLVL", "_", ""].contains(&nome.trim()),
                "{s}"
            );
        }
    }

    #[test]
    fn argv_cwd_e_ambiente_exatos() {
        let mut c = Comando::novo(b"/bin/sh", b"/tmp").unwrap();
        for a in [
            &b"-c"[..],
            b"printf '%s|' \"$0\" \"$1\" \"$X\"; pwd; env | sort",
            b"a b",
            b"$(nada)",
        ] {
            c.argumento(a).unwrap();
        }
        c.variavel(b"X", b"valor com espa\xc3\xa7o=e igual")
            .unwrap();
        let (fim, out, _) = rodar(&c);
        assert_eq!(fim, Fim::Codigo(0));
        let out = String::from_utf8(out).unwrap();
        assert!(
            out.starts_with("a b|$(nada)|valor com espaço=e igual|/tmp\n"),
            "{out}"
        );
        // D60: só o que foi passado (o `sh` acrescenta PWD, SHLVL e `_`).
        let nomes: Vec<&str> = out
            .lines()
            .skip(1)
            .map(|l| l.split('=').next().unwrap())
            .collect();
        for n in &nomes {
            assert!(
                ["X", "PWD", "SHLVL", "_", "OLDPWD"].contains(n),
                "variável herdada: {n}"
            );
        }
    }

    /// D60: nada do ambiente deste processo (HOME, PATH, CARGO_*; no daemon, `LISTEN_*`)
    /// chega ao filho.
    #[test]
    fn ataque_ambiente_do_daemon_nao_chega_ao_filho() {
        let c = sh("env");
        let (_, out, _) = rodar(&c);
        let out = String::from_utf8(out).unwrap();
        for proibido in ["LISTEN_", "HOME=", "PATH=/", "CARGO", "RUST"] {
            assert!(
                !out.contains(proibido),
                "{proibido} no ambiente do filho: {out}"
            );
        }
    }

    #[test]
    fn busca_no_path_do_filho_sem_shell() {
        let mut c = Comando::novo(b"printf", b"/").unwrap();
        c.argumento(b"%s").unwrap();
        c.argumento(b"ok;echo injetado").unwrap();
        c.variavel(b"PATH", b"/nao-existe:/usr/bin:/bin").unwrap();
        let (fim, out, _) = rodar(&c);
        assert_eq!(fim, Fim::Codigo(0));
        assert_eq!(out, b"ok;echo injetado");
        // Sem PATH: o padrão do `execvpe` (`/bin:/usr/bin`).
        let mut c = Comando::novo(b"true", b"/").unwrap();
        assert_eq!(rodar(&c).0, Fim::Codigo(0));
        c.variavel(b"PATH", b"/nao-existe").unwrap();
        assert_eq!(c.lancar().unwrap_err(), ErroLancar::Programa(libc::ENOENT));
    }

    #[test]
    fn falhas_antes_do_exec() {
        let c = Comando::novo(b"/nao/existe", b"/").unwrap();
        let e = c.lancar().unwrap_err();
        assert_eq!(e, ErroLancar::Programa(libc::ENOENT));
        assert_eq!(e.codigo_de_saida(), 127);
        let c = Comando::novo(b"/bin/true", b"/nao/existe").unwrap();
        assert!(matches!(c.lancar().unwrap_err(), ErroLancar::Diretorio(_)));
        let c = Comando::novo(b"/etc/hostname", b"/").unwrap();
        let e = c.lancar().unwrap_err();
        assert_eq!(e.codigo_de_saida(), 126, "{e:?}");
        assert_eq!(Comando::novo(b"", b"/").unwrap_err(), ErroComando::Vazio);
        assert_eq!(
            Comando::novo(b"a\0b", b"/").unwrap_err(),
            ErroComando::ByteNulo
        );
        let mut c = Comando::novo(b"/bin/true", b"/").unwrap();
        assert_eq!(
            c.variavel(b"A=B", b"x").unwrap_err(),
            ErroComando::NomeInvalido
        );
        assert_eq!(
            c.variavel(b"", b"x").unwrap_err(),
            ErroComando::NomeInvalido
        );
        assert_eq!(
            c.variavel(b"A", b"x\0y").unwrap_err(),
            ErroComando::ByteNulo
        );
        assert_eq!(c.argumento(b"\0").unwrap_err(), ErroComando::ByteNulo);
    }

    #[test]
    fn codigo_e_sinal() {
        assert_eq!(rodar(&sh("exit 7")).0, Fim::Codigo(7));
        let fim = rodar(&sh("kill -TERM $$")).0;
        assert_eq!(fim, Fim::Sinal(libc::SIGTERM));
        assert_eq!(fim.codigo_de_saida(), 143);
    }

    /// Nenhum fd do daemon chega ao filho, nem um sem `CLOEXEC`.
    #[test]
    fn ataque_fd_do_daemon_nao_chega_ao_filho() {
        let f = File::open("/etc/hostname").unwrap();
        sys::tirar_cloexec(f.as_fd()).unwrap();
        let (_, out, _) = rodar(&sh("ls /proc/self/fd"));
        let fds: Vec<String> = String::from_utf8(out)
            .unwrap()
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        // 0–2 e o fd que o `ls` abre para ler o diretório.
        assert!(fds.len() <= 4, "{fds:?}");
        for fd in ["0", "1", "2"] {
            assert!(fds.iter().any(|x| x == fd), "{fds:?}");
        }
        drop(f);
    }

    /// O runtime do Rust ignora o `SIGPIPE`; o filho não herda isso.
    #[test]
    fn sinais_no_padrao() {
        let (_, out, _) = rodar(&sh("grep -E '^Sig(Ign|Blk):' /proc/self/status"));
        let out = String::from_utf8(out).unwrap();
        assert!(out.contains("SigBlk:\t0000000000000000"), "{out}");
        assert!(out.contains("SigIgn:\t0000000000000000"), "{out}");
    }

    /// Matar o grupo mata também o neto que o filho deixou rodando.
    #[test]
    fn matar_grupo_leva_os_netos() {
        let (g, mut p) = sh("sleep 60 & echo $!; wait").lancar().unwrap();
        let mut linha = [0u8; 32];
        let n = p.saida.as_mut().unwrap().read(&mut linha).unwrap();
        let neto: u32 = std::str::from_utf8(&linha[..n])
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(std::path::Path::new(&format!("/proc/{neto}")).exists());
        g.matar();
        assert_eq!(g.colher().unwrap(), Fim::Sinal(libc::SIGKILL));
        let fim = Instant::now() + Duration::from_secs(5);
        // O neto vira zumbi do init e some.
        while std::fs::read_to_string(format!("/proc/{neto}/stat"))
            .is_ok_and(|s| !s.contains(") Z "))
        {
            assert!(Instant::now() < fim, "o neto continua vivo");
            std::thread::sleep(Duration::from_millis(20));
        }
        // Depois de colhido, matar não faz nada (o pid pode ser de outro).
        g.matar();
    }

    /// `PR_SET_PDEATHSIG`: a thread que criou o filho morre, o filho leva `SIGKILL`.
    #[test]
    fn filho_morre_com_a_thread_que_o_criou() {
        let pid = std::thread::spawn(|| {
            let (g, p) = sh("sleep 60").lancar().unwrap();
            let pid = g.pid;
            drop(p);
            std::mem::forget(g);
            pid
        })
        .join()
        .unwrap();
        let status = sys::colher(pid).unwrap();
        assert!(libc::WIFSIGNALED(status));
        assert_eq!(libc::WTERMSIG(status), libc::SIGKILL);
    }

    #[test]
    fn entrada_chega_ao_filho() {
        let (g, mut p) = sh("cat").lancar().unwrap();
        let mut e = p.entrada.take().unwrap();
        e.write_all(b"linha\n").unwrap();
        drop(e);
        let mut out = Vec::new();
        p.saida.take().unwrap().read_to_end(&mut out).unwrap();
        assert_eq!(out, b"linha\n");
        assert_eq!(g.colher().unwrap(), Fim::Codigo(0));
    }

    /// O `Debug` não mostra argv nem ambiente.
    #[test]
    fn debug_sem_valor() {
        let mut c = Comando::novo(b"/bin/true", b"/").unwrap();
        c.variavel(b"SENHA", b"falso-PG:s3nha/01@#").unwrap();
        let d = format!("{c:?}");
        assert!(!d.contains("falso") && !d.contains("SENHA"), "{d}");
    }
}
