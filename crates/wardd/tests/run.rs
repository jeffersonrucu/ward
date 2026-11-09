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
        let p = ward_plataforma::socket::diretorio_runtime()
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
    let log = dev.0.join("wardd.log");
    let filho = Command::new(WARDD)
        .env_clear()
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
    let socket = sock.0.join(ward_plataforma::socket::NOME_SOCKET);
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
        r#"{{"v":1,"op":"executar","dados":{{"entrada":"{entrada}","argv":[{}],"cwd":"{}","ambiente":[{}]}}}}"#,
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

    // V4.17: puro, base64, percent, em dois pedaços com pausa e no stderr.
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
    let r = run(
        &w,
        "t-livre",
        &["comando-que-nao-existe-26"],
        &[("PATH", "/usr/bin:/bin")],
    );
    assert_eq!(r.fim, Some(127));
    assert!(texto(&r.erro).contains("comando não encontrado"), "{r:?}");

    // D13 e D60: o ambiente é só o do pedido, sem `LD_*`, e nada do daemon; locale C
    // vira LC_CTYPE=C.UTF-8; a entrega por cima do mesmo nome vindo da CLI.
    let r = run(
        &w,
        "t-livre",
        &["/usr/bin/env"],
        &[
            ("LD_PRELOAD", "/nao/existe.so"),
            ("LD_LIBRARY_PATH", "/tmp"),
            ("LANG", "C"),
            ("DA_CLI", "sim"),
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
    ] {
        assert!(
            !env.contains(proibido),
            "{proibido} no ambiente do filho:\n{env}"
        );
    }
    for esperado in [
        "DA_CLI=sim",
        "LANG=C\n",
        "LC_CTYPE=C.UTF-8",
        "WARD_PASS=••••",
        "WARD_ENTRADA=t-livre",
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
