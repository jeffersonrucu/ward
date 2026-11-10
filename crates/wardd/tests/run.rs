//! `ward run` pelo binário `wardd` (modo de teste), de ponta a ponta pelo socket (Fase
//! 26): entrega de D10, máscara (S4), código de saída, ambiente só da CLI sem `LD_*`
//! (D13, D60), anexos 0600 em `<rt>tmp/` apagados em toda saída e na partida (S2,
//! V4.15), a CLI que cai mata o grupo do filho (D13) e o registro de RA1.
//!
//! O cofre é de teste, criado neste diretório pelo próprio núcleo, só com valores
//! falsos; o socket (e o `<rt>`) fica num diretório só deste teste em `/run/user/<uid>/`.

mod comum;

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use comum::*;
use ward_core::protocolo::{self, ErroResposta, Fluxo, Papel, Resposta};

const WARDD: &str = env!("CARGO_BIN_EXE_wardd");
/// Senha do `t-livre` (falsa).
const SENHA_LIVRE: &str = SEGREDOS[3];
const LICENCA: &str = "falso-licenca-run-26";
const ANEXO: &str = "FALSO_ENV=falso-anexo-run-26";

struct DirSocket(PathBuf);

impl DirSocket {
    fn novo(rotulo: &str) -> DirSocket {
        let p = ward_platform::socket::diretorio_runtime()
            .join(format!("wardd-r-{}-{rotulo}", std::process::id()));
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

fn subir(dev: &Dir, sock: &DirSocket) -> Wardd {
    subir_com(dev, sock, &[])
}

/// O `wardd` de teste com variáveis a mais (Fase 54b: `WARD_DEV_APP`).
fn subir_com(dev: &Dir, sock: &DirSocket, extra: &[(&str, &std::ffi::OsStr)]) -> Wardd {
    let log = dev.0.join("wardd.log");
    let mut cmd = Command::new(WARDD);
    cmd.env_clear();
    for (k, v) in extra {
        cmd.env(k, v);
    }
    let filho = cmd
        .env("PATH", "/usr/bin:/bin")
        .env("WARD_DEV_DIR", &dev.0)
        .env("WARD_DEV_SOCKET_DIR", &sock.0)
        .env("RUST_LOG", "trace")
        .env("VARIAVEL_DO_DAEMON", "nao-pode-vazar")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log)
                .unwrap(),
        )
        .spawn()
        .unwrap();
    let socket = sock.0.join(ward_platform::socket::NOME_SOCKET);
    let fim = Instant::now() + Duration::from_secs(20);
    while UnixStream::connect(&socket).is_err() {
        assert!(Instant::now() < fim, "o wardd não criou o socket");
        std::thread::sleep(Duration::from_millis(20));
    }
    Wardd { filho, log, socket }
}

fn mandar(s: &mut UnixStream, json: &str) {
    s.write_all(&(json.len() as u32).to_be_bytes()).unwrap();
    s.write_all(json.as_bytes()).unwrap();
}

fn receber(s: &mut UnixStream) -> Option<Resposta> {
    let mut n = [0u8; 4];
    s.read_exact(&mut n).ok()?;
    let mut b = vec![0u8; u32::from_be_bytes(n) as usize];
    s.read_exact(&mut b).ok()?;
    Some(protocolo::decodificar_resposta(Papel::Agente, &b).unwrap())
}

fn conectar(w: &Wardd) -> UnixStream {
    let s = UnixStream::connect(&w.socket).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(60))).unwrap();
    s
}

fn destravar_por(w: &Wardd) {
    let mut s = conectar(w);
    mandar(
        &mut s,
        &format!(
            r#"{{"v":1,"op":"destravar","dados":{{"senha":"{}"}}}}"#,
            b64(SENHA.as_bytes())
        ),
    );
    assert!(matches!(receber(&mut s), Some(Resposta::Ok)));
}

fn pedido_executar(entrada: &str, argv: &[&str], amb: &[(&str, &str)]) -> String {
    pedido_executar_com(entrada, argv, amb, "pipe")
}

/// O `executar` com o tipo da entrada padrão (Fase 54b).
fn pedido_executar_com(entrada: &str, argv: &[&str], amb: &[(&str, &str)], tipo: &str) -> String {
    let argv: Vec<String> = argv
        .iter()
        .map(|a| format!("\"{}\"", b64(a.as_bytes())))
        .collect();
    let amb: Vec<String> = amb
        .iter()
        .map(|(k, v)| {
            format!(
                r#"{{"nome":"{}","valor":"{}"}}"#,
                b64(k.as_bytes()),
                b64(v.as_bytes())
            )
        })
        .collect();
    format!(
        r#"{{"v":1,"op":"executar","dados":{{"entrada":"{entrada}","argv":[{}],"cwd":"{}","tipo_entrada":"{tipo}","ambiente":[{}]}}}}"#,
        argv.join(","),
        b64(b"/"),
        amb.join(",")
    )
}

fn entrada_padrao(s: &mut UnixStream, dados: &[u8]) {
    mandar(
        s,
        &format!(
            r#"{{"v":1,"op":"entrada_padrao","dados":{{"dados":"{}"}}}}"#,
            b64(dados)
        ),
    );
}

/// O que a sessão devolveu: stdout, stderr e o código (ou o erro).
#[derive(Debug, Default)]
struct Sessao {
    saida: Vec<u8>,
    erro: Vec<u8>,
    fim: Option<u8>,
    falha: Option<ErroResposta>,
}

fn ler_sessao(s: &mut UnixStream) -> Sessao {
    let mut r = Sessao::default();
    loop {
        match receber(s) {
            Some(Resposta::Saida { fluxo, dados }) => match fluxo {
                Fluxo::Saida => r.saida.extend_from_slice(&dados),
                Fluxo::Erro => r.erro.extend_from_slice(&dados),
            },
            Some(Resposta::Fim { codigo }) => {
                r.fim = Some(codigo);
                break;
            }
            Some(Resposta::Erro(e)) => {
                r.falha = Some(e);
                break;
            }
            Some(outra) => panic!("resposta inesperada: {outra:?}"),
            None => break,
        }
    }
    // Depois do fim, o daemon fecha a conexão; recusado antes de lançar, ela segue.
    if r.fim.is_some() {
        assert!(receber(s).is_none());
    }
    r
}

fn run(w: &Wardd, entrada: &str, argv: &[&str], amb: &[(&str, &str)]) -> Sessao {
    let mut s = conectar(w);
    mandar(&mut s, &pedido_executar(entrada, argv, amb));
    entrada_padrao(&mut s, b"");
    ler_sessao(&mut s)
}

fn sh(script: &str) -> Vec<&str> {
    vec!["/bin/sh", "-c", script]
}

fn sha256(dados: &[u8]) -> String {
    let mut c = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    c.stdin.take().unwrap().write_all(dados).unwrap();
    let out = c.wait_with_output().unwrap();
    String::from_utf8(out.stdout).unwrap()
}

/// Cofre de teste com, além do de `comum`, `t-nota` (nota segura) e `t-gen` (genérico
/// com extra secreto e anexo), os dois livres. Travado no fim (o binário o abre).
fn cofre(dev: &Dir) {
    let (d, _) = cofre_de_teste(dev);
    for q in [
        format!(
            r#"{{"entrada":{{"nome":"t-nota","grupo":"clientex","ambiente":"dev","tipo":"nota-segura","campos":{{"texto":"{}"}}}}}}"#,
            b64(b"falso-nota-run-26")
        ),
        format!(
            r#"{{"entrada":{{"nome":"t-gen","grupo":"clientex","ambiente":"stage","tipo":"generico","extras":[{{"nome":"REGIAO","valor":"{}","segredo":false}},{{"nome":"CHAVE_LICENCA","valor":"{}","segredo":true}}],"anexos":[{{"nome":"app.env","conteudo":"{}"}}]}}}}"#,
            b64(b"r-01"),
            b64(LICENCA.as_bytes()),
            b64(ANEXO.as_bytes())
        ),
    ] {
        let r = pedir(&d, Papel::App, "salvar_entrada", &q);
        assert!(ok(&r), "{r:?}");
    }
    for n in ["t-nota", "t-gen"] {
        assert!(ok(&mudar_nivel(&d, Papel::App, n, "livre", PIN)));
    }
    d.travar();
}

fn texto(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn tmp_vazio(sock: &DirSocket) -> bool {
    fs::read_dir(sock.0.join("tmp")).map_or(true, |mut d| d.next().is_none())
}

fn esperar(prazo: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let fim = Instant::now() + prazo;
    while Instant::now() < fim {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    cond()
}

fn vivo(pid: u32) -> bool {
    fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|s| !s.contains(") Z "))
}

/// V4.3–V4.11, V4.17, V4.19, V4.20 com as entradas deste teste, mais D13, D60 e o
/// locale (handoff 22a).
#[test]
fn v4_entrega_mascara_codigo_e_recusas() {
    let dev = Dir::novo("run");
    cofre(&dev);
    let sock = DirSocket::novo("run");
    let w = subir(&dev, &sock);
    destravar_por(&w);

    // Entrega: o hash da senha bate (V4.3); a senha impressa sai `••••` (S4).
    let r = run(
        &w,
        "t-livre",
        &sh("printf %s \"$WARD_PASS\" | sha256sum"),
        &[],
    );
    assert_eq!(r.fim, Some(0), "{r:?}");
    assert_eq!(texto(&r.saida), sha256(SENHA_LIVRE.as_bytes()));
    let r = run(
        &w,
        "t-livre",
        &sh(
            "printf '%s|%s|%s|%s' \"$WARD_ENTRADA\" \"$WARD_GRUPO\" \"$WARD_AMBIENTE\" \"$WARD_USER\"",
        ),
        &[],
    );
    assert_eq!(texto(&r.saida), "t-livre|clientex|dev|admin");

    // V4.17: puro, base64, percent, em dois pedaços com pausa e no stderr. Em dois
    // pedaços (D34 revisto), o começo sai como saiu e o pedaço que completa a senha
    // vira marca: a senha inteira nunca sai.
    for script in [
        "printf %s \"$WARD_PASS\"",
        "printf %s \"$WARD_PASS\" | base64",
        "printf 'falso-li'; sleep 0.2; printf 'vre-ward-09'",
        "printf %s \"$WARD_PASS\" >&2",
        "printf 'falso-li' >&2; sleep 0.2; printf 'vre-ward-09' >&2",
    ] {
        let r = run(&w, "t-livre", &sh(script), &[]);
        assert_eq!(r.fim, Some(0), "{script}: {r:?}");
        let tudo = [r.saida.as_slice(), r.erro.as_slice()].concat();
        assert!(!contem(&tudo, b"falso-livre"), "{script}: {}", texto(&tudo));
        assert!(!contem(&tudo, b"ZmFsc28tbGl2cmU"), "{script}");
        assert!(
            contem(&tudo, "••••".as_bytes()),
            "{script}: {}",
            texto(&tudo)
        );
    }

    // V4.19: código do comando; morto por SIGTERM, 143.
    assert_eq!(run(&w, "t-livre", &sh("exit 7"), &[]).fim, Some(7));
    assert_eq!(run(&w, "t-livre", &sh("kill -TERM $$"), &[]).fim, Some(143));
    // Comando que não existe: como o shell (127), com mensagem fixa.
    let r = run(&w, "t-livre", &["comando-que-nao-existe-26"], &[]);
    assert_eq!(r.fim, Some(127));
    assert!(texto(&r.erro).contains("comando não encontrado"), "{r:?}");

    // D13, D60 e Fase 26a: o ambiente é montado do zero (só a lista da CLI, sem `LD_*`,
    // e nada do daemon); locale C vira LC_CTYPE=C.UTF-8; a entrega por cima do mesmo
    // nome vindo da CLI.
    let r = run(
        &w,
        "t-livre",
        &["/usr/bin/env"],
        &[
            ("LD_PRELOAD", "/nao/existe.so"),
            ("LD_LIBRARY_PATH", "/tmp"),
            ("LANG", "C"),
            ("DA_CLI", "nao-pode-passar"),
            ("WARD_PASS", "valor-da-cli"),
        ],
    );
    let env = texto(&r.saida);
    assert_eq!(r.fim, Some(0), "{r:?}");
    for proibido in [
        "LD_",
        "VARIAVEL_DO_DAEMON",
        "WARD_DEV_",
        "RUST_LOG",
        "LISTEN_",
        "valor-da-cli",
        "DA_CLI",
    ] {
        assert!(
            !env.contains(proibido),
            "{proibido} no ambiente do filho:\n{env}"
        );
    }
    for esperado in [
        "LANG=C\n",
        "LC_CTYPE=C.UTF-8",
        "WARD_PASS=••••",
        "WARD_ENTRADA=t-livre",
        "PATH=/usr/local/bin:/usr/bin:/bin\n",
    ] {
        assert!(env.contains(esperado), "{esperado} faltando:\n{env}");
    }
    // Locale UTF-8: o `${X@Q}` do bash com acento usa aspas (a máscara cobre); fica
    // `$'…'` (RA3) só em locale C.
    let r = run(
        &w,
        "t-livre",
        &[
            "/bin/bash",
            "-c",
            "X=\"$WARD_PASS-ç\"; echo \"${X@Q}\"; locale charmap",
        ],
        &[("LANG", "C")],
    );
    assert!(texto(&r.saida).contains("UTF-8"), "{r:?}");
    assert!(!contem(&r.saida, b"falso-livre"), "{r:?}");

    // Entrada padrão (D13).
    let mut s = conectar(&w);
    mandar(&mut s, &pedido_executar("t-livre", &["/bin/cat"], &[]));
    entrada_padrao(&mut s, b"linha 1\n");
    entrada_padrao(&mut s, b"linha 2\n");
    entrada_padrao(&mut s, b"");
    let r = ler_sessao(&mut s);
    assert_eq!(
        (r.fim, texto(&r.saida).as_str()),
        (Some(0), "linha 1\nlinha 2\n")
    );

    // Anexo e extra secreto (V4.10, V4.5): arquivo 0600 em <rt>tmp/, conteúdo e
    // licença mascarados, e o arquivo some no fim (V4.15).
    let r = run(
        &w,
        "t-gen",
        &sh(
            "f=\"$WARD_ARQUIVO_APP_ENV\"; stat -c '%a %U' \"$f\"; stat -c '%a' \"$(dirname \"$f\")\"; \
             printf '%s\\n' \"$f\"; cat \"$f\"; echo; printf '%s|%s\\n' \"$WARD_REGIAO\" \"$WARD_CHAVE_LICENCA\"; \
             sha256sum < \"$f\"",
        ),
        &[],
    );
    assert_eq!(r.fim, Some(0), "{r:?}");
    let out = texto(&r.saida);
    let linhas: Vec<&str> = out.lines().collect();
    assert!(linhas[0].starts_with("600 "), "{out}");
    assert_eq!(linhas[1], "700", "{out}");
    let caminho = PathBuf::from(linhas[2]);
    assert!(caminho.starts_with(sock.0.join("tmp")), "{out}");
    assert!(caminho.ends_with("app.env"), "{out}");
    assert!(
        !out.contains("falso-anexo") && !out.contains(LICENCA),
        "{out}"
    );
    assert!(out.contains("r-01|••••"), "{out}");
    assert!(
        out.contains(sha256(ANEXO.as_bytes()).split(' ').next().unwrap()),
        "{out}"
    );
    assert!(!caminho.exists(), "o anexo ficou depois do fim");
    assert!(tmp_vazio(&sock));

    // V4.11: nota segura não executa nada.
    let marca = dev.0.join("marca-nota");
    let r = run(
        &w,
        "t-nota",
        &["/usr/bin/touch", marca.to_str().unwrap()],
        &[],
    );
    assert_eq!(r.falha, Some(ErroResposta::TipoSemEntrega), "{r:?}");
    assert_eq!(ErroResposta::TipoSemEntrega.mensagem(), "tipo sem entrega");
    assert!(!marca.exists());
    // V4.20: com aprovação e sem app → "aprovação indisponível", nada executado.
    let r = run(
        &w,
        "t-aprov",
        &["/usr/bin/touch", marca.to_str().unwrap()],
        &[],
    );
    assert_eq!(r.falha, Some(ErroResposta::AprovacaoIndisponivel), "{r:?}");
    assert!(!marca.exists());
    // Bloqueada = inexistente (V3.17); o túnel do banco é da Fase 28 (nada executado).
    let r = run(
        &w,
        "t-bloq",
        &["/usr/bin/touch", marca.to_str().unwrap()],
        &[],
    );
    assert_eq!(r.falha, Some(ErroResposta::EntradaInexistente), "{r:?}");
    assert!(!marca.exists());
    // Recusado antes de lançar, a conexão segue para o próximo pedido.
    let mut s = conectar(&w);
    mandar(&mut s, &pedido_executar("t-nota", &["/bin/true"], &[]));
    assert!(matches!(
        receber(&mut s),
        Some(Resposta::Erro(ErroResposta::TipoSemEntrega))
    ));
    mandar(&mut s, &pedido_executar("t-livre", &sh("exit 4"), &[]));
    entrada_padrao(&mut s, b"");
    assert_eq!(ler_sessao(&mut s).fim, Some(4));

    // Um quadro fora da sessão (outra operação no meio) encerra o comando.
    let mut s = conectar(&w);
    mandar(&mut s, &pedido_executar("t-livre", &sh("sleep 30"), &[]));
    std::thread::sleep(Duration::from_millis(300));
    mandar(&mut s, r#"{"v":1,"op":"lista","dados":{}}"#);
    let t = Instant::now();
    let r = ler_sessao(&mut s);
    assert_eq!(r.fim, Some(137), "{r:?}");
    assert!(t.elapsed() < Duration::from_secs(5));

    // S13: nada de segredo no log (`RUST_LOG=trace`).
    let log = fs::read(&w.log).unwrap();
    for segredo in [SENHA, SENHA_LIVRE, LICENCA, ANEXO, "falso-nota-run-26"] {
        assert!(!contem(&log, segredo.as_bytes()), "{segredo} no log");
        assert!(
            !contem(&log, b64(segredo.as_bytes()).as_bytes()),
            "{segredo} (b64) no log"
        );
    }
}

/// D13 e V4.15: a CLI que cai mata o grupo inteiro do filho (e os netos) e o anexo some
/// em até 1 s; o filho morto por SIGKILL também apaga o anexo; o `wardd` morto por
/// SIGKILL leva o filho junto (`PR_SET_PDEATHSIG`) e, ao subir de novo, esvazia
/// `<rt>tmp/`.
/// Fase 26a (achado médio da auditoria do M3): o ambiente da CLI não muda o comando
/// aprovado. `PATH` com a pasta do atacante na frente, `BASH_ENV`/`ENV`,
/// `NODE_OPTIONS=--require`, `GIT_SSH_COMMAND`, `PYTHONSTARTUP`, `PGSERVICEFILE`,
/// `HOME` e locale/fuso/terminal apontando para arquivo do atacante: nada disso chega
/// ao filho, e o comando sem `/` é achado só pelo `PATH` fixo. O diretório de
/// trabalho continua o da CLI.
#[test]
fn f26a_ataque_ambiente_da_cli_nao_muda_o_comando() {
    let dev = Dir::novo("f26a");
    cofre(&dev);
    let sock = DirSocket::novo("f26a");
    let w = subir(&dev, &sock);
    destravar_por(&w);

    // A pasta do atacante: `true`, `sh`, `env` e `wardfalso-26a` falsos que deixam uma
    // marca; um rc para o `BASH_ENV`/`ENV`; um módulo para o `--require`.
    let atacante = dev.0.join("atacante");
    fs::create_dir_all(&atacante).unwrap();
    let marca = dev.0.join("marca-26a");
    let falso = format!("#!/bin/sh\necho pego >> '{}'\n", marca.display());
    for nome in ["true", "sh", "env", "id", "wardfalso-26a"] {
        let p = atacante.join(nome);
        fs::write(&p, &falso).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let rc = atacante.join("rc.sh");
    fs::write(&rc, format!("echo pego >> '{}'\n", marca.display())).unwrap();
    let a = |n: &str| atacante.join(n).display().to_string();
    let path_atacante = format!("{}:/usr/bin:/bin", atacante.display());
    let rc_s = rc.display().to_string();
    let require = format!("--require {}", a("mod.js"));
    let ssh_cmd = format!("{} -x", a("true"));
    let home = atacante.display().to_string();
    let amb: Vec<(&str, &str)> = vec![
        ("PATH", path_atacante.as_str()),
        ("BASH_ENV", rc_s.as_str()),
        ("ENV", rc_s.as_str()),
        ("NODE_OPTIONS", require.as_str()),
        ("GIT_SSH_COMMAND", ssh_cmd.as_str()),
        ("PYTHONSTARTUP", rc_s.as_str()),
        ("PYTHONPATH", home.as_str()),
        ("PERL5LIB", home.as_str()),
        ("PGSERVICEFILE", rc_s.as_str()),
        ("SSH_AUTH_SOCK", rc_s.as_str()),
        ("HOME", home.as_str()),
        ("USER", "atacante"),
        ("LOCPATH", home.as_str()),
        ("LANG", "/tmp/atacante-locale"),
        ("TZ", "../../../tmp/atacante-fuso"),
        ("TERM", "../atacante"),
        ("TERMINFO", home.as_str()),
        ("COLORTERM", "truecolor"),
        ("NO_COLOR", "1"),
    ];

    // O comando sem `/` vem do `PATH` fixo: `true`, `sh` e `env` do sistema; o que só
    // existe na pasta do atacante não é achado (127).
    let r = run(&w, "t-livre", &["true"], &amb);
    assert_eq!(r.fim, Some(0), "{r:?}");
    let r = run(&w, "t-livre", &["wardfalso-26a"], &amb);
    assert_eq!(r.fim, Some(127), "{r:?}");
    // `bash -c` (não interativo) leria o `BASH_ENV`; `sh -c` com `ENV`.
    let r = run(&w, "t-livre", &["bash", "-c", "exit 0"], &amb);
    assert_eq!(r.fim, Some(0), "{r:?}");
    let r = run(&w, "t-livre", &["sh", "-c", "exit 0"], &amb);
    assert_eq!(r.fim, Some(0), "{r:?}");
    let r = run(&w, "t-livre", &["sh", "-i", "-c", "exit 0"], &amb);
    assert_eq!(r.fim, Some(0), "{r:?}");
    assert!(!marca.exists(), "o comando do atacante rodou");

    // O ambiente do filho: só o fixo, a lista e a entrega.
    let r = run(&w, "t-livre", &["env"], &amb);
    assert_eq!(r.fim, Some(0), "{r:?}");
    let env = texto(&r.saida);
    let (usuario, pessoal) = ward_platform::conta_do_usuario().unwrap();
    for esperado in [
        "PATH=/usr/local/bin:/usr/bin:/bin\n".to_owned(),
        format!("HOME={}\n", pessoal.to_string_lossy()),
        format!("USER={}\n", usuario.to_string_lossy()),
        format!("LOGNAME={}\n", usuario.to_string_lossy()),
        "COLORTERM=truecolor\n".to_owned(),
        "NO_COLOR=1\n".to_owned(),
        "LC_CTYPE=C.UTF-8\n".to_owned(),
        "WARD_ENTRADA=t-livre\n".to_owned(),
    ] {
        assert!(env.contains(&esperado), "{esperado:?} faltando:\n{env}");
    }
    for proibido in [
        "atacante",
        "BASH_ENV",
        "ENV=",
        "NODE_OPTIONS",
        "GIT_",
        "PYTHON",
        "PERL5LIB",
        "PGSERVICEFILE",
        "SSH_",
        "LOCPATH",
        "LANG=",
        "TZ=",
        "TERM=",
        "TERMINFO",
    ] {
        assert!(
            !env.lines()
                .any(|l| l.starts_with(proibido) || l.contains("atacante")),
            "{proibido} no ambiente do filho:\n{env}"
        );
    }
    let linhas: Vec<&str> = env.lines().collect();
    let nomes: Vec<&str> = linhas
        .iter()
        .map(|l| l.split_once('=').map_or(*l, |(n, _)| n))
        .collect();
    for n in &nomes {
        assert!(
            [
                "PATH",
                "HOME",
                "USER",
                "LOGNAME",
                "COLORTERM",
                "NO_COLOR",
                "LC_CTYPE"
            ]
            .contains(n)
                || n.starts_with("WARD_"),
            "{n} não devia estar no ambiente:\n{env}"
        );
    }

    // Valores aceitos passam como vieram; o diretório de trabalho é o da CLI.
    let r = run(
        &w,
        "t-livre",
        &[
            "sh",
            "-c",
            "printf '%s|%s|%s|' \"$TZ\" \"$TERM\" \"$LANG\"; pwd",
        ],
        &[
            ("TZ", "America/Sao_Paulo"),
            ("TERM", "xterm-256color"),
            ("LANG", "pt_BR.UTF-8"),
        ],
    );
    assert_eq!(
        texto(&r.saida),
        "America/Sao_Paulo|xterm-256color|pt_BR.UTF-8|/\n",
        "{r:?}"
    );
    assert!(!marca.exists(), "o comando do atacante rodou");
    // O log do daemon conta as variáveis tiradas, sem nome nem valor.
    let log = fs::read_to_string(&w.log).unwrap();
    assert!(!log.contains("atacante"), "{log}");
}

#[test]
fn v4_15_cli_que_cai_e_daemon_morto() {
    let dev = Dir::novo("cai");
    cofre(&dev);
    let sock = DirSocket::novo("cai");
    let mut w = subir(&dev, &sock);
    destravar_por(&w);
    let script = "echo \"$$ $WARD_ARQUIVO_APP_ENV\"; sleep 60 & echo $!; wait";

    // 1. A CLI cai (fecha a conexão) com o comando e um neto rodando.
    let mut s = conectar(&w);
    mandar(&mut s, &pedido_executar("t-gen", &sh(script), &[]));
    let mut lido = Vec::new();
    while lido.iter().filter(|b| **b == b'\n').count() < 2 {
        match receber(&mut s) {
            Some(Resposta::Saida { dados, .. }) => lido.extend_from_slice(&dados),
            r => panic!("{r:?}"),
        }
    }
    let t = texto(&lido);
    let mut it = t.split_whitespace();
    let filho: u32 = it.next().unwrap().parse().unwrap();
    let arquivo = PathBuf::from(it.next().unwrap());
    let neto: u32 = it.next().unwrap().parse().unwrap();
    assert!(vivo(filho) && vivo(neto) && arquivo.exists());
    drop(s);
    assert!(
        esperar(Duration::from_secs(1), || !vivo(filho) && !vivo(neto)),
        "o grupo segue vivo"
    );
    assert!(
        esperar(Duration::from_secs(1), || !arquivo.exists()),
        "o anexo ficou"
    );

    // 2. O filho leva SIGKILL: 137, e o anexo some.
    let r = run(
        &w,
        "t-gen",
        &sh("test -f \"$WARD_ARQUIVO_APP_ENV\" && kill -9 $$"),
        &[],
    );
    assert_eq!(r.fim, Some(137), "{r:?}");
    assert!(tmp_vazio(&sock));

    // 3. O `wardd` leva SIGKILL no meio: o filho morre junto; o anexo fica até a
    //    próxima partida, que o apaga.
    let mut s = conectar(&w);
    mandar(&mut s, &pedido_executar("t-gen", &sh(script), &[]));
    let mut lido = Vec::new();
    while !lido.contains(&b'\n') {
        match receber(&mut s) {
            Some(Resposta::Saida { dados, .. }) => lido.extend_from_slice(&dados),
            r => panic!("{r:?}"),
        }
    }
    let filho: u32 = texto(&lido)
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    w.filho.kill().unwrap();
    let _ = w.filho.wait();
    assert!(
        esperar(Duration::from_secs(2), || !vivo(filho)),
        "o filho sobreviveu ao daemon"
    );
    assert!(
        !tmp_vazio(&sock),
        "sem o daemon, o temporário ainda está lá"
    );
    drop(s);
    drop(w);
    let w = subir(&dev, &sock);
    assert!(tmp_vazio(&sock), "a partida não limpou <rt>tmp/");
    assert_eq!(
        fs::metadata(sock.0.join("tmp"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    drop(w);
}

/// RA1 (registro, sem mitigação genérica): enquanto o filho roda, outro processo do
/// mesmo uid lê `/proc/<pid>/environ` dele e o anexo 0600. O `execve` volta o
/// `dumpable` a 1 para qualquer programa comum; nem namespace de usuário nem o Yama
/// (só o modo `ATTACH`) mudam a leitura do `environ` (`PTRACE_MODE_READ`). O que vale:
/// o `wardd` (não-dumpable) não é legível, e a janela é só a do comando. Este teste
/// prende o estado registrado no handoff (V4.13 reprova e vai ao usuário): se um dia o
/// `environ` do filho deixar de ser legível, ele falha e o registro muda.
#[test]
fn ra1_registro_environ_do_filho_legivel_pelo_mesmo_uid() {
    let dev = Dir::novo("ra1");
    cofre(&dev);
    let sock = DirSocket::novo("ra1");
    let w = subir(&dev, &sock);
    destravar_por(&w);
    let mut s = conectar(&w);
    mandar(
        &mut s,
        &pedido_executar("t-gen", &sh("echo $$; sleep 30"), &[]),
    );
    let pid: u32 = match receber(&mut s) {
        Some(Resposta::Saida { dados, .. }) => texto(&dados).trim().parse().unwrap(),
        r => panic!("{r:?}"),
    };
    let environ = fs::read(format!("/proc/{pid}/environ")).unwrap_or_default();
    assert!(
        contem(&environ, LICENCA.as_bytes()),
        "RA1 mudou: o environ do filho deixou de ser legível pelo mesmo uid"
    );
    // O daemon, não-dumpable (S8), não é legível.
    let daemon = w.filho.id();
    assert!(fs::read(format!("/proc/{daemon}/environ")).is_err());
    assert!(fs::File::open(format!("/proc/{daemon}/mem")).is_err());
    drop(s);
    assert!(esperar(Duration::from_secs(1), || !vivo(pid)));
    drop(w);
}

/// O tamanho do pedido (D52): acima do quadro, o daemon recusa com `quadro_invalido` e
/// não executa nada (a CLI mede antes; aqui é a segunda camada).
#[test]
fn d52_pedido_grande_demais_recusado_pelo_daemon() {
    let dev = Dir::novo("d52");
    cofre(&dev);
    let sock = DirSocket::novo("d52");
    let w = subir(&dev, &sock);
    destravar_por(&w);
    let marca = dev.0.join("marca-d52");
    let grande = "x".repeat(100 * 1024);
    let amb: Vec<(String, String)> = (0..9).map(|i| (format!("V{i}"), grande.clone())).collect();
    let amb: Vec<(&str, &str)> = amb.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let mut s = conectar(&w);
    let json = pedido_executar(
        "t-livre",
        &["/usr/bin/touch", marca.to_str().unwrap()],
        &amb,
    );
    // O daemon pode fechar antes de ler o corpo todo (pipe quebrado aqui).
    let _ = s
        .write_all(&(json.len() as u32).to_be_bytes())
        .and_then(|()| s.write_all(json.as_bytes()));
    let r = ler_sessao(&mut s);
    assert!(r.fim.is_none(), "{r:?}");
    assert!(
        matches!(r.falha, None | Some(ErroResposta::QuadroInvalido)),
        "{r:?}"
    );
    std::thread::sleep(Duration::from_millis(200));
    assert!(!marca.exists());
    assert!(tmp_vazio(&sock));
}

/// V4.21 (D16): com o cofre travado e sem app, o `ward run` espera; o `destravar` por
/// outra conexão (o `ward destravar`) solta o pedido, que roda com a entrega.
#[test]
fn v4_21_travado_espera_o_destravar_da_cli() {
    let dev = Dir::novo("v421");
    cofre(&dev);
    let sock = DirSocket::novo("v421");
    let w = subir(&dev, &sock);
    let mut s = conectar(&w);
    mandar(
        &mut s,
        &pedido_executar("t-livre", &sh("printf %s \"$WARD_PASS\" | wc -c"), &[]),
    );
    entrada_padrao(&mut s, b"");
    std::thread::sleep(Duration::from_millis(1500));
    destravar_por(&w);
    let r = ler_sessao(&mut s);
    assert_eq!(r.fim, Some(0), "{r:?}");
    assert_eq!(texto(&r.saida).trim(), SENHA_LIVRE.len().to_string());
}

/// Fase 28g, ataque da 2ª auditoria do M4: um `write` único de `n` bytes + a senha, com
/// `n` em volta das fronteiras de 16 KiB (buffer antigo) e 64 KiB (pipe padrão), saía
/// com o começo da senha em claro (o leitor cortava no tamanho do buffer). Agora o
/// pipe tem 256 KiB e o daemon lê o lote inteiro antes de mascarar: a senha sai só
/// `••••`, na saída e no erro.
#[test]
fn f28g_ataque_write_unico_grande_nao_mostra_o_comeco_da_senha() {
    let dev = Dir::novo("run-28g");
    cofre(&dev);
    let sock = DirSocket::novo("28g");
    let w = subir(&dev, &sock);
    destravar_por(&w);
    let mut casos: Vec<usize> = (16_370..16_390).collect();
    casos.extend(65_520..65_540);
    casos.extend([131_067, 200_000]);
    for fd in [1, 2] {
        for &n in &casos {
            let script = format!(
                "import os\nos.write({fd}, b'b' * {n} + os.environ['WARD_PASS'].encode() + b'\\n')\n"
            );
            let r = run(&w, "t-livre", &["/usr/bin/python3", "-c", &script], &[]);
            assert_eq!(r.fim, Some(0), "fd={fd} n={n}: {:?}", r.falha);
            let tudo = if fd == 1 { &r.saida } else { &r.erro };
            assert!(
                !contem(tudo, b"falso-"),
                "fd={fd} n={n}: o começo da senha saiu em claro"
            );
            let mut esperado = vec![b'b'; n];
            esperado.extend_from_slice("••••\n".as_bytes());
            assert!(*tudo == esperado, "fd={fd} n={n}: saída inesperada");
        }
    }
    let log = fs::read(&w.log).unwrap();
    assert!(!contem(&log, SENHA_LIVRE.as_bytes()));
}

/// Fase 54 (S8), ataque da auditoria final: o comando do `ward run` recebeu a senha no
/// ambiente e espera a trava para usá-la (`sleep; printenv … > arquivo`, fora da
/// máscara). Travar no meio mata o grupo inteiro (o `sh` e o neto): o arquivo nunca
/// nasce, a sessão termina com 137 e o aviso, o anexo some. Um `ward run` depois da
/// trava espera o destravar, como o V4.21.
#[test]
fn f54_ataque_ward_run_morre_na_trava() {
    let dev = Dir::novo("run-54");
    cofre(&dev);
    let sock = DirSocket::novo("54");
    let w = subir(&dev, &sock);
    destravar_por(&w);
    let marca = dev.0.join("marca-f54");
    let neto = dev.0.join("marca-f54-neto");
    let script = format!(
        "echo $$; (sleep 4; printenv WARD_PASS > '{n}') & sleep 4; printenv WARD_PASS > '{m}'; wait",
        n = neto.display(),
        m = marca.display()
    );
    let mut s = conectar(&w);
    mandar(&mut s, &pedido_executar("t-livre", &sh(&script), &[]));
    entrada_padrao(&mut s, b"");
    let pid: u32 = match receber(&mut s) {
        Some(Resposta::Saida { dados, .. }) => texto(&dados).trim().parse().unwrap(),
        r => panic!("{r:?}"),
    };
    assert!(vivo(pid));
    let antes = Instant::now();
    let mut t = conectar(&w);
    mandar(&mut t, r#"{"v":1,"op":"travar","dados":{}}"#);
    assert!(matches!(receber(&mut t), Some(Resposta::Ok)));
    let r = ler_sessao(&mut s);
    assert!(
        antes.elapsed() < Duration::from_secs(3),
        "a sessão só acabou com o fim do comando"
    );
    assert_eq!(r.fim, Some(137), "{r:?}");
    assert!(
        texto(&r.erro).contains("o cofre travou"),
        "sem o aviso: {r:?}"
    );
    assert!(!vivo(pid));
    // Depois do prazo do `sleep`, nada escreveu a senha.
    std::thread::sleep(Duration::from_secs(5));
    assert!(!marca.exists(), "o comando usou a senha depois da trava");
    assert!(!neto.exists(), "o neto usou a senha depois da trava");
    assert!(tmp_vazio(&sock));

    // Travado, um `ward run` novo não roda até destravar (V4.21): solto pelo destravar,
    // e uma trava antes do lançamento também o pega (o registro confere a contagem).
    let mut s = conectar(&w);
    mandar(
        &mut s,
        &pedido_executar("t-livre", &sh("printf %s \"$WARD_PASS\" | wc -c"), &[]),
    );
    entrada_padrao(&mut s, b"");
    std::thread::sleep(Duration::from_millis(500));
    destravar_por(&w);
    let r = ler_sessao(&mut s);
    assert_eq!(r.fim, Some(0), "{r:?}");
    assert_eq!(texto(&r.saida).trim(), SENHA_LIVRE.len().to_string());

    // Um `ward run` que termina antes da trava não leva aviso nem muda de código.
    let r = run(&w, "t-livre", &sh("exit 3"), &[]);
    assert_eq!(r.fim, Some(3), "{r:?}");
    assert!(r.erro.is_empty(), "{r:?}");
    let log = fs::read(&w.log).unwrap();
    assert!(!contem(&log, SENHA_LIVRE.as_bytes()));
    assert!(contem(&log, "encerrado(s) ao travar".as_bytes()));
}

/// Fase 55 (S8), os dois passos da reauditoria final contra o `wardd` de verdade:
/// 1. `setsid -f`: o comando deixa um neto com sessão própria e termina; o neto
///    escreveria a senha 3 s depois;
/// 2. `setpgid(0, 0)`: o filho sai do grupo, o comando dorme e o cofre trava; o filho
///    escreveria a senha 3 s depois.
///
/// Antes da correção, os dois escreviam (o grupo morria, eles não). Agora: nada
/// escreve, nenhum processo sobra, a CLI ouve "encerrado" na trava. O controle sem a
/// correção (o fugitivo sobrevive) está em `ward-platform/tests/subreaper.rs`.
#[test]
fn f55_ataque_descendente_fora_do_grupo_morre_no_fim_e_na_trava() {
    let dev = Dir::novo("run-55");
    cofre(&dev);
    let sock = DirSocket::novo("55");
    let w = subir(&dev, &sock);
    destravar_por(&w);

    // 1. `setsid -f` e o comando termina.
    let marca1 = dev.0.join("marca-f55-setsid");
    let script = format!(
        "setsid -f /bin/sh -c 'echo $$ > \"{p}\"; sleep 3; printenv WARD_PASS > \"{m}\"'; \
         i=0; while [ ! -s \"{p}\" ] && [ $i -lt 100 ]; do sleep 0.05; i=$((i+1)); done; cat \"{p}\"",
        p = dev.0.join("pid-f55-setsid").display(),
        m = marca1.display()
    );
    let r = run(&w, "t-livre", &sh(&script), &[]);
    assert_eq!(r.fim, Some(0), "{r:?}");
    let fugitivo: u32 = texto(&r.saida).trim().parse().unwrap();
    assert!(
        esperar(Duration::from_secs(3), || !vivo(fugitivo)),
        "o neto com setsid sobreviveu ao fim do comando"
    );
    assert!(r.erro.is_empty(), "{r:?}");

    // 2. `setpgid(0, 0)` e a trava com o comando vivo.
    let marca2 = dev.0.join("marca-f55-setpgid");
    let py = format!(
        "import os,time\npid=os.fork()\nif pid==0:\n os.setpgid(0,0)\n time.sleep(3)\n open({m:?},'w').write(os.environ.get('WARD_PASS',''))\n os._exit(0)\nprint(pid,flush=True)\ntime.sleep(30)\n",
        m = marca2.display().to_string()
    );
    let mut s = conectar(&w);
    mandar(
        &mut s,
        &pedido_executar("t-livre", &["/usr/bin/python3", "-I", "-c", &py], &[]),
    );
    entrada_padrao(&mut s, b"");
    let filho: u32 = match receber(&mut s) {
        Some(Resposta::Saida { dados, .. }) => texto(&dados).trim().parse().unwrap(),
        r => panic!("{r:?}"),
    };
    assert!(vivo(filho));
    let mut t = conectar(&w);
    mandar(&mut t, r#"{"v":1,"op":"travar","dados":{}}"#);
    assert!(matches!(receber(&mut t), Some(Resposta::Ok)));
    let r = ler_sessao(&mut s);
    assert_eq!(r.fim, Some(137), "{r:?}");
    assert!(
        texto(&r.erro).contains("o comando foi encerrado"),
        "sem o aviso de encerrado: {r:?}"
    );
    assert!(!vivo(filho), "o filho fora do grupo sobreviveu à trava");

    // Depois do prazo dos dois, nada escreveu a senha.
    std::thread::sleep(Duration::from_secs(4));
    assert!(
        !marca1.exists(),
        "o neto com setsid usou a senha depois do fim"
    );
    assert!(
        !marca2.exists(),
        "o filho com setpgid usou a senha depois da trava"
    );
    let log = fs::read(&w.log).unwrap();
    assert!(!contem(&log, SENHA_LIVRE.as_bytes()));
}

/// Fase 54b (D68): quem declara a entrada padrão nula não manda nada depois. A entrada
/// do filho fecha no lançamento (o `cat` acaba na hora, sem esperar o fim da CLI) e um
/// `entrada_padrao` com dados mata o grupo, sem o dado chegar ao comando.
#[test]
fn f54b_ataque_entrada_nula_com_dado_mata_o_comando() {
    let dev = Dir::novo("run-54b-nula");
    cofre(&dev);
    let sock = DirSocket::novo("54b-nula");
    let w = subir(&dev, &sock);
    destravar_por(&w);
    // Sem dado: o `cat` vê o fim já, antes do quadro vazio da CLI.
    let mut s = conectar(&w);
    mandar(
        &mut s,
        &pedido_executar_com("t-livre", &sh("cat; echo fim"), &[], "nula"),
    );
    let r = ler_sessao(&mut s);
    assert_eq!(r.fim, Some(0), "{r:?}");
    assert_eq!(texto(&r.saida), "fim\n");
    // Com dado: o comando morre e o dado não chega.
    let mut s = conectar(&w);
    mandar(
        &mut s,
        &pedido_executar_com("t-livre", &sh("sleep 3; cat; echo fim"), &[], "nula"),
    );
    let antes = Instant::now();
    entrada_padrao(&mut s, b"falso-script-do-agente-54b\n");
    let r = ler_sessao(&mut s);
    assert!(antes.elapsed() < Duration::from_secs(3), "o comando seguiu");
    assert_eq!(r.fim, Some(137), "{r:?}");
    let tudo = [r.saida.as_slice(), r.erro.as_slice()].concat();
    assert!(!contem(&tudo, b"falso-script-do-agente"), "{r:?}");
    assert!(!contem(&tudo, b"fim"), "{r:?}");
    let log = fs::read(&w.log).unwrap();
    assert!(contem(&log, "entrada padrão declarada nula".as_bytes()));
    assert!(!contem(&log, b"falso-script-do-agente"));
}

const APROVADOR: &str = env!("CARGO_BIN_EXE_wardd-aprovador-teste");

/// Ataque da auditoria final, de ponta a ponta pelo binário (D68): `printf <script> |
/// ward run t-aprov -- python3`, com a janela de teste (`wardd-aprovador-teste`, aberta
/// pelo daemon por `WARD_DEV_APP`) respondendo "Permitir uma vez" com o PIN certo. Sem
/// a caixa da entrada padrão, o daemon recusa a resposta, o `python3` nunca roda e nada
/// sai; com a caixa, roda. Com terminal, "Permitir uma vez" passa como antes.
#[test]
fn f54b_ataque_script_por_pipe_na_aprovacao() {
    let dev = Dir::novo("run-54b-aprov");
    cofre(&dev);
    let app = dev.0.join("wardd-aprovador-teste");
    fs::hard_link(APROVADOR, &app).unwrap();
    let modo = dev.0.join("aprovador.modo");
    let log_app = dev.0.join("logs/wardd-aprovador-teste.log");
    let sock = DirSocket::novo("54b-aprov");
    let w = subir_com(&dev, &sock, &[("WARD_DEV_APP", app.as_os_str())]);
    destravar_por(&w);
    // O script mede a senha (nada dela sai, nem mascarado).
    let script =
        b"import os, sys\nsys.stdout.write('ENTREGUE:%d' % len(os.environ['WARD_PASS']))\n";
    let respostas = |m: &str| {
        fs::read_to_string(&log_app)
            .unwrap_or_default()
            .matches(m)
            .count()
    };

    // 1. Sem a caixa: recusado; o pedido segue pendente e nada roda. O `python3` da
    // auditoria e um leitor de comandos pela entrada (o `sh`, no lugar do `psql`, que
    // lê o SQL do mesmo jeito; a decisão não olha o argv), por pipe e por arquivo.
    let casos: [(&[&str], &[u8], &str); 3] = [
        (&["/usr/bin/python3", "-I"], script, "pipe"),
        (
            &["/bin/sh"],
            b"printf 'ENTREGUE:%s' \"${#WARD_PASS}\"\n",
            "pipe",
        ),
        (
            &["/bin/sh"],
            b"printf 'ENTREGUE:%s' \"${#WARD_PASS}\"\n",
            "arquivo",
        ),
    ];
    for (n, (argv, dados, tipo)) in casos.iter().enumerate() {
        fs::write(&modo, format!("uma_vez {PIN}\n")).unwrap();
        let mut s = conectar(&w);
        mandar(&mut s, &pedido_executar_com("t-aprov", argv, &[], tipo));
        entrada_padrao(&mut s, dados);
        entrada_padrao(&mut s, b"");
        assert!(
            esperar(Duration::from_secs(20), || respostas(
                "resposta=pedido_invalido"
            ) > n),
            "a janela de teste não respondeu: {}",
            fs::read_to_string(&log_app).unwrap_or_default()
        );
        let linhas = fs::read_to_string(&log_app).unwrap();
        assert!(
        linhas.contains("pedido operacao=executar entradas=t-aprov origem=- confirmacao=0 entrada_oculta=1 decisao=uma_vez"),
        "{linhas}"
    );
        s.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        assert!(
            receber(&mut s).is_none(),
            "o comando rodou sem a confirmação"
        );
        // A CLI desiste: o pedido é cancelado, nada rodou.
        drop(s);
        assert!(esperar(Duration::from_secs(5), || {
            String::from_utf8_lossy(&fs::read(&w.log).unwrap())
                .matches("o cliente saiu; cancelada")
                .count()
                > n
        }));
    }

    // 2. Com a caixa: roda.
    fs::write(&modo, format!("uma_vez_ciente {PIN}\n")).unwrap();
    let mut s = conectar(&w);
    mandar(
        &mut s,
        &pedido_executar_com("t-aprov", &["/usr/bin/python3", "-I"], &[], "pipe"),
    );
    entrada_padrao(&mut s, script);
    entrada_padrao(&mut s, b"");
    let r = ler_sessao(&mut s);
    assert_eq!(r.fim, Some(0), "{r:?}");
    assert_eq!(texto(&r.saida), format!("ENTREGUE:{}", SEGREDOS[0].len()));

    // 3. Terminal: "Permitir uma vez" sem a caixa, como antes.
    fs::write(&modo, format!("uma_vez {PIN}\n")).unwrap();
    let mut s = conectar(&w);
    mandar(
        &mut s,
        &pedido_executar_com(
            "t-aprov",
            &sh("printf %s \"$WARD_PASS\" | wc -c"),
            &[],
            "terminal",
        ),
    );
    entrada_padrao(&mut s, b"");
    let r = ler_sessao(&mut s);
    assert_eq!(r.fim, Some(0), "{r:?}");
    assert_eq!(texto(&r.saida).trim(), SEGREDOS[0].len().to_string());

    let log = fs::read(&w.log).unwrap();
    for segredo in SEGREDOS {
        assert!(!contem(&log, segredo.as_bytes()));
        assert!(!contem(&fs::read(&log_app).unwrap(), segredo.as_bytes()));
    }
    assert!(contem(
        &log,
        b"confirma\xc3\xa7\xc3\xa3o da entrada padr\xc3\xa3o"
    ));
}
