//! `ward ssh` no núcleo (Fase 27), com `ssh`, `sshpass` e `sftp` falsos
//! (`tests/ssh_falso.py`) que registram o argv e nunca conectam: D3 (token do
//! "Conectar", V5.24, V4.32), D12 (agente só do filho, V4.24), D13 (`-F /dev/null`,
//! `known_hosts` próprio, ambiente do zero), D34 revisto (sem oráculo de prefixo), D43 (usuário com `@`,
//! V4.35) e D56 (`sftp` com `User=`, V4.37).
//!
//! A sessão roda como no servidor (`wardd::ssh::sessao` numa ponta de um par de
//! sockets); a outra ponta faz o papel da CLI. O cofre é de teste, criado pelo núcleo
//! com valores falsos; a chave SSH é gerada aqui, com passphrase aleatória.
//!
//! Os programas falsos entram por `Daemon::com_programas_ssh`, que só existe com a
//! feature `ganchos-teste` (o `make test` liga todas).

#![cfg(feature = "ganchos-teste")]

mod comum;

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use comum::*;
use ward_core::entrada::Segredo;
use ward_core::limites::MAX_QUADRO_COMUM;
use ward_core::protocolo::quadro::{self, LIMITE_RESPOSTA};
use ward_core::protocolo::{self, ErroResposta, Fluxo, Janela, Papel, Pedido, Resposta};
use wardd::aprovacao::{Motor, RelogioMotor, SemApp, SemCliente};
use wardd::nucleo::Daemon;
use wardd::ssh::ProgramasSsh;
use zeroize::Zeroizing;

/// Senha dos servidores de teste (a de `comum::servidor`, falsa).
const SENHA_SSH: &str = SEGREDOS[1];
/// Senha do `banco` do túnel (falsa).
const SENHA_BANCO: &str = SEGREDOS[2];

#[derive(Clone, Default)]
struct RelogioManual(Arc<Mutex<Duration>>);

impl RelogioManual {
    fn avancar(&self, d: Duration) {
        *self.0.lock().unwrap() += d;
    }
}

impl RelogioMotor for RelogioManual {
    fn agora(&self) -> Duration {
        *self.0.lock().unwrap()
    }
}

/// `<rt>` só deste teste em `/run/user/<uid>/` (caminho curto: o socket do agente cabe
/// no `sun_path`).
struct Rt(PathBuf);

impl Rt {
    fn novo(rotulo: &str) -> Rt {
        let p = ward_plataforma::socket::diretorio_runtime()
            .join(format!("wsh-{}-{rotulo}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        Rt(p)
    }
}

impl Drop for Rt {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Cena {
    d: Arc<Daemon>,
    dir: Dir,
    rt: Rt,
    /// Onde ficam os programas falsos, o `modo` e os registros.
    base: PathBuf,
    relogio: RelogioManual,
}

fn salvar(d: &Daemon, json: &str) {
    let r = pedir(d, Papel::App, "salvar_entrada", json);
    assert!(ok(&r), "{r:?}");
}

fn servidor_json(nome: &str, usuario: &str, campos_auth: &str, extra: &str) -> String {
    format!(
        r#"{{"entrada":{{"nome":"{nome}","grupo":"clientex","ambiente":"stage","tipo":"servidor","campos":{{"host":"127.0.0.1","porta":"2222","usuario":"{usuario}",{campos_auth}{extra}}}}}}}"#
    )
}

fn sftp_json(nome: &str, protocolo: &str, campos_auth: &str) -> String {
    format!(
        r#"{{"entrada":{{"nome":"{nome}","grupo":"clientex","ambiente":"dev","tipo":"ftp-sftp","campos":{{"protocolo":"{protocolo}","host":"127.0.0.1","porta":"2222","usuario":"sftpuser",{campos_auth}}}}}}}"#
    )
}

fn auth_senha() -> String {
    format!(r#""senha":"{}""#, b64(SENHA_SSH.as_bytes()))
}

/// Chave Ed25519 nova com passphrase aleatória (nada fixo no repositório).
fn gerar_chave(dir: &Path) -> (Vec<u8>, String) {
    let pp = format!(
        "pp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let arq = dir.join("chave_teste");
    let st = Command::new("/usr/bin/ssh-keygen")
        .env_clear()
        .env("HOME", dir)
        .env("PATH", "/usr/bin:/bin")
        .args(["-q", "-t", "ed25519", "-C", "teste-fase27", "-N"])
        .arg(&pp)
        .arg("-f")
        .arg(&arq)
        .status()
        .unwrap();
    assert!(st.success());
    let chave = fs::read(&arq).unwrap();
    fs::remove_file(&arq).unwrap();
    let _ = fs::remove_file(arq.with_extension("pub"));
    (chave, pp)
}

fn cena(rotulo: &str) -> Cena {
    let dir = Dir::novo(rotulo);
    let (d, _) = cofre_de_teste(&dir);
    let base = dir.0.join("falsos");
    fs::create_dir(&base).unwrap();
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ssh_falso.py");
    for nome in ["ssh", "sshpass", "sftp"] {
        let p = base.join(nome);
        fs::write(
            &p,
            format!(
                "#!/bin/sh\nexec /usr/bin/python3 -I '{}' {nome} '{}' \"$@\"\n",
                script.display(),
                base.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let rt = Rt::novo(rotulo);
    let relogio = RelogioManual::default();
    let d = d
        .com_motor(Motor::novo(Arc::new(relogio.clone()), Box::new(SemApp)))
        .com_programas_ssh(ProgramasSsh {
            ssh: base.join("ssh"),
            sshpass: base.join("sshpass"),
            sftp: base.join("sftp"),
        })
        .com_temporarios(rt.0.clone());
    let (chave, pp) = gerar_chave(&dir.0);
    let auth_chave = format!(
        r#""chave_privada":"{}","passphrase":"{}""#,
        b64(&chave),
        b64(pp.as_bytes())
    );
    salvar(
        &d,
        &servidor_json("s-livre", "deploy@cliente.com.br", &auth_senha(), ""),
    );
    salvar(&d, &servidor_json("s-outro", "deploy", &auth_senha(), ""));
    salvar(&d, &servidor_json("s-bloq", "deploy", &auth_senha(), ""));
    salvar(&d, &servidor_json("s-chave", "deploy", &auth_chave, ""));
    salvar(
        &d,
        &servidor_json("s-salto", "deploy", &auth_senha(), r#","salto":"s-livre""#),
    );
    // Fase 28: salto com chave no principal, salto com chave no intermediário, salto com
    // aprovação ou bloqueado (D19) e o túnel do `banco`.
    salvar(
        &d,
        &servidor_json(
            "s-salto-chave",
            "deploy",
            &auth_chave,
            r#","salto":"s-livre""#,
        ),
    );
    salvar(
        &d,
        &servidor_json(
            "s-via-chave",
            "deploy",
            &auth_senha(),
            r#","salto":"s-chave""#,
        ),
    );
    salvar(
        &d,
        &servidor_json(
            "s-salto-aprov",
            "deploy",
            &auth_senha(),
            r#","salto":"s-outro""#,
        ),
    );
    salvar(
        &d,
        &servidor_json(
            "s-salto-bloq",
            "deploy",
            &auth_senha(),
            r#","salto":"s-bloq""#,
        ),
    );
    salvar(
        &d,
        &format!(
            r#"{{"entrada":{{"nome":"b-tunel","grupo":"clientex","ambiente":"stage","tipo":"banco","campos":{{"motor":"postgres","host":"db.interno","porta":"5432","base":"app","usuario":"app","senha":"{}","tunel":"s-livre"}}}}}}"#,
            b64(SENHA_BANCO.as_bytes())
        ),
    );
    salvar(&d, &sftp_json("f-sftp", "sftp", &auth_senha()));
    salvar(&d, &sftp_json("f-sftp-chave", "sftp", &auth_chave));
    salvar(&d, &sftp_json("f-ftp", "ftp", &auth_senha()));
    for n in [
        "s-livre",
        "s-chave",
        "s-salto",
        "s-salto-chave",
        "s-via-chave",
        "s-salto-aprov",
        "s-salto-bloq",
        "b-tunel",
        "f-sftp",
        "f-sftp-chave",
        "f-ftp",
    ] {
        assert!(ok(&mudar_nivel(&d, Papel::App, n, "livre", PIN)));
    }
    assert!(ok(&mudar_nivel(&d, Papel::App, "s-bloq", "bloqueado", PIN)));
    Cena {
        d: Arc::new(d),
        dir,
        rt,
        base,
        relogio,
    }
}

impl Cena {
    fn modo(&self, m: &str) {
        fs::write(self.base.join("modo"), m).unwrap();
    }

    /// Os registros do programa falso `nome`.
    fn registros(&self, nome: &str) -> Vec<Reg> {
        let mut v = Vec::new();
        for e in fs::read_dir(&self.base).unwrap() {
            let n = e.unwrap().file_name().into_string().unwrap();
            if n.starts_with(&format!("registro-{nome}-")) && n.ends_with(".txt") {
                let t = fs::read_to_string(self.base.join(&n)).unwrap();
                v.push(Reg(t
                    .lines()
                    .map(|l| {
                        let (k, v) = l.split_once('\t').unwrap();
                        (k.to_owned(), v.to_owned())
                    })
                    .collect()));
            }
        }
        v
    }

    fn registro(&self, nome: &str) -> Reg {
        let r = self.registros(nome);
        assert_eq!(r.len(), 1, "registros de {nome}: {r:?}");
        r.into_iter().next().unwrap()
    }

    fn nada_executado(&self) -> bool {
        ["ssh", "sshpass", "sftp"]
            .iter()
            .all(|n| self.registros(n).is_empty())
    }

    fn token(&self, entrada: &str) -> Segredo {
        match self.d.atender(
            Papel::App,
            Pedido::EmitirToken {
                entrada: entrada.into(),
            },
        ) {
            Resposta::Token(t) => t,
            outra => panic!("emitir_token: {outra:?}"),
        }
    }
}

fn pedido_ssh(
    entrada: &str,
    comando: &[&str],
    token: Option<&Segredo>,
    janela: Option<Janela>,
) -> Pedido {
    Pedido::Ssh {
        entrada: entrada.into(),
        comando: comando
            .iter()
            .map(|c| Zeroizing::new(c.as_bytes().to_vec()))
            .collect(),
        token: token.map(|t| Segredo::de_bytes(t.expor().to_vec())),
        janela,
        // Fase 54b: os testes mandam a entrada pelo socket (como um pipe), ou digitam
        // no pty.
        tipo_entrada: if janela.is_some() {
            ward_core::protocolo::TipoEntradaPadrao::Terminal
        } else {
            ward_core::protocolo::TipoEntradaPadrao::Pipe
        },
    }
}

/// O que a sessão devolveu, com a hora de cada pedaço de saída.
#[derive(Debug, Default)]
struct Sessao {
    saida: Vec<u8>,
    erro: Vec<u8>,
    pedacos: Vec<(Duration, Vec<u8>)>,
    fim: Option<u8>,
    falha: Option<ErroResposta>,
}

impl Sessao {
    fn texto(&self) -> String {
        String::from_utf8_lossy(&self.saida).into_owned()
    }
}

fn mandar(s: &mut UnixStream, p: &Pedido) {
    let corpo = protocolo::codificar_pedido(Papel::Agente, p).unwrap();
    quadro::escrever_quadro(s, &corpo, MAX_QUADRO_COMUM).unwrap();
}

/// Roda a sessão como o servidor rodaria. `cli` recebe a ponta da CLI logo depois do
/// pedido (manda entrada, janela, mexe no cofre) e a sessão é lida até o fim.
fn sessao(c: &Cena, pedido: Pedido, cli: impl FnOnce(&mut UnixStream)) -> Sessao {
    sessao_de(c, pedido, cli, false)
}

/// Como [`sessao`]; `run`: a sessão do `ward run` (`wardd::execucao::sessao`).
fn sessao_de(c: &Cena, pedido: Pedido, cli: impl FnOnce(&mut UnixStream), run: bool) -> Sessao {
    let (mut lado_cli, lado_daemon) = UnixStream::pair().unwrap();
    lado_cli
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    let d = c.d.clone();
    let t = std::thread::spawn(move || {
        if run {
            wardd::execucao::sessao(&d, &SemCliente, &lado_daemon, pedido)
        } else {
            wardd::ssh::sessao(&d, &SemCliente, &lado_daemon, pedido)
        }
    });
    let inicio = Instant::now();
    cli(&mut lado_cli);
    let mut r = Sessao::default();
    loop {
        let Ok(b) = quadro::ler_quadro(&mut lado_cli, LIMITE_RESPOSTA) else {
            break;
        };
        match protocolo::decodificar_resposta(Papel::Agente, &b).unwrap() {
            Resposta::Saida { fluxo, dados } => {
                r.pedacos.push((inicio.elapsed(), dados.to_vec()));
                match fluxo {
                    Fluxo::Saida => r.saida.extend_from_slice(&dados),
                    Fluxo::Erro => r.erro.extend_from_slice(&dados),
                }
            }
            Resposta::Fim { codigo } => {
                r.fim = Some(codigo);
                break;
            }
            Resposta::Erro(e) => {
                r.falha = Some(e);
                break;
            }
            outra => panic!("resposta inesperada: {outra:?}"),
        }
    }
    let seguiu = t.join().unwrap();
    // Recusado antes de lançar: a conexão segue; depois da sessão, acaba.
    assert_eq!(seguiu, r.falha.is_some(), "{r:?}");
    r
}

/// Fecha a entrada do comando. Quando o pedido é recusado antes de lançar (token
/// inválido, nível), o daemon pode já ter respondido e soltado a ponta: a escrita que
/// falha aí não é erro do teste (a resposta é conferida depois).
fn fechar_entrada(s: &mut UnixStream) {
    let corpo = protocolo::codificar_pedido(
        Papel::Agente,
        &Pedido::EntradaPadrao {
            dados: Segredo::de_bytes(Vec::new()),
        },
    )
    .unwrap();
    let _ = quadro::escrever_quadro(s, &corpo, MAX_QUADRO_COMUM);
}

/// O registro de um programa falso: `campo → valores`, na ordem.
#[derive(Debug)]
struct Reg(Vec<(String, String)>);

impl Reg {
    fn todos(&self, campo: &str) -> Vec<String> {
        self.0
            .iter()
            .filter(|(k, _)| k == campo)
            .map(|(_, v)| v.clone())
            .collect()
    }

    /// O último valor do campo (o registro do agente é regravado com `depois`).
    fn um(&self, campo: &str) -> String {
        self.0
            .iter()
            .rev()
            .find(|(k, _)| k == campo)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| panic!("sem {campo}: {self:?}"))
    }
}

fn argv(reg: &Reg) -> Vec<String> {
    reg.todos("argv")
}

/// Os nomes do ambiente, menos o `PWD` que o `/bin/sh` do programa falso põe.
fn env(reg: &Reg) -> BTreeSet<String> {
    reg.todos("env")
        .into_iter()
        .filter(|n| n != "PWD")
        .collect()
}

fn opcoes(a: &[String]) -> Vec<String> {
    a.windows(2)
        .filter(|w| w[0] == "-o")
        .map(|w| w[1].clone())
        .collect()
}

fn sha256(dados: &[u8]) -> String {
    let out = Command::new("/usr/bin/sha256sum")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut c| {
            use std::io::Write;
            c.stdin.take().unwrap().write_all(dados)?;
            c.wait_with_output()
        })
        .unwrap();
    String::from_utf8(out.stdout).unwrap()[..64].to_owned()
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

fn rt_sem_agente(c: &Cena) -> bool {
    fs::read_dir(&c.rt.0).map_or(true, |d| {
        d.filter_map(Result::ok)
            .all(|e| !e.file_name().to_string_lossy().starts_with("ssh-"))
    })
}

/// D43 + D13 (V4.35, V4.23): usuário com `@` inteiro depois de `-l`, host depois de
/// `--`, nunca `usuário@host` nem `-o User=`; senha só em `SSHPASS` (nunca em argv);
/// `-F /dev/null`, `known_hosts` do Ward e ambiente do zero; a saída que repete a
/// senha volta mascarada; o código do comando remoto passa.
#[test]
fn v4_35_d43_usuario_com_arroba_e_senha_so_no_ambiente() {
    let c = cena("d43");
    c.modo("normal");
    let r = sessao(
        &c,
        pedido_ssh("s-livre", &["whoami", "--", "-l", "x@y"], None, None),
        fechar_entrada,
    );
    assert_eq!(r.fim, Some(7), "{r:?}");
    let sp = c.registro("sshpass");
    let ssh = c.registro("ssh");
    let a = argv(&sp);
    assert_eq!(a[0], "-e");
    assert_eq!(a[1], c.base.join("ssh").to_str().unwrap());
    assert_eq!(&argv(&ssh)[..], &a[2..]);
    let a = &a[2..];
    assert_eq!(&a[..2], ["-F", "/dev/null"]);
    // O usuário inteiro, como argumento próprio de `-l`, antes do `--`.
    let fim_opcoes = a.iter().position(|x| x == "--").unwrap();
    assert_eq!(
        &a[fim_opcoes - 4..],
        [
            "-p",
            "2222",
            "-l",
            "deploy@cliente.com.br",
            "--",
            "127.0.0.1",
            "whoami",
            "--",
            "-l",
            "x@y"
        ]
    );
    for x in a {
        assert!(
            !x.contains("@127.0.0.1") && !x.contains("@cliente.com.br@"),
            "{x}"
        );
        assert!(!x.starts_with("User=") && !x.contains(SENHA_SSH), "{x}");
    }
    let o = opcoes(a);
    let kh = c.dir.0.join("known_hosts");
    for esperado in [
        "IdentitiesOnly=yes".to_owned(),
        "CertificateFile=/dev/null".to_owned(),
        format!("UserKnownHostsFile={}", kh.display()),
        "GlobalKnownHostsFile=/dev/null".to_owned(),
        "StrictHostKeyChecking=accept-new".to_owned(),
        "PubkeyAuthentication=no".to_owned(),
        "IdentityAgent=none".to_owned(),
        "IdentityFile=none".to_owned(),
        "ForwardAgent=no".to_owned(),
        "ControlMaster=no".to_owned(),
        "ProxyCommand=none".to_owned(),
    ] {
        assert!(o.contains(&esperado), "falta {esperado}: {o:?}");
    }
    // Ambiente do zero: nada do teste nem do daemon (nem SSH_AUTH_SOCK, nem HOME).
    let esperado: BTreeSet<String> = ["LANG", "PATH", "SSHPASS"].map(String::from).into();
    assert_eq!(env(&sp), esperado);
    assert_eq!(sp.um("sshpass_sha256"), sha256(SENHA_SSH.as_bytes()));
    assert_eq!(sp.um("tty"), "0");
    // S4: o filho imprimiu a senha; a CLI só vê a marca.
    let t = r.texto();
    assert!(t.contains("SSHPASS=••••"), "{t}");
    assert!(!contem(&r.saida, SENHA_SSH.as_bytes()) && !contem(&r.erro, SENHA_SSH.as_bytes()));
    assert!(t.contains("remoto=whoami -- -l x@y"), "{t}");
}

/// D56 (V4.37): no `sftp`, `-o User=<usuário>`, `-P`, `-F /dev/null` e o host depois de
/// `--`; nunca `-l` (limite de banda no `sftp`). Os comandos vêm da entrada padrão;
/// `sftp` com comando é recusado sem executar nada.
#[test]
fn v4_37_d56_sftp_com_user_e_host_depois_de_traco_traco() {
    let c = cena("d56");
    let r = sessao(&c, pedido_ssh("f-sftp", &[], None, None), |s| {
        mandar(
            s,
            &Pedido::EntradaPadrao {
                dados: Segredo::de_bytes(b"pwd\nls\n".to_vec()),
            },
        );
        fechar_entrada(s);
    });
    assert_eq!(r.fim, Some(0), "{r:?}");
    assert_eq!(r.texto(), "sftp> pwd\nsftp> ls\n");
    let a = argv(&c.registro("sshpass"));
    assert_eq!(&a[..2], ["-e", c.base.join("sftp").to_str().unwrap()]);
    let a = &a[2..];
    assert_eq!(&a[..2], ["-F", "/dev/null"]);
    assert_eq!(
        &a[a.len() - 6..],
        ["-o", "User=sftpuser", "-P", "2222", "--", "127.0.0.1"]
    );
    assert!(!a.iter().any(|x| x == "-l" || x.contains('@')), "{a:?}");

    let c = cena("d56b");
    let r = sessao(&c, pedido_ssh("f-sftp", &["ls"], None, None), |_| {});
    assert_eq!(r.falha, Some(ErroResposta::PedidoInvalido));
    assert!(c.nada_executado());
}

/// Recusas antes de executar: tipo sem `ward ssh`, `ftp` (não `sftp`), bloqueada
/// (inexistente), com aprovação sem app (vale o nível) e o nível combinado de D19
/// (V4.30): livre com salto "com aprovação" sem app dá `aprovacao_indisponivel`, e
/// livre com salto bloqueado é negado sem perguntar.
#[test]
fn recusas_sem_executar_nada() {
    let c = cena("recusas");
    for (entrada, erro) in [
        ("t-livre", ErroResposta::TipoSemEntrega),
        ("f-ftp", ErroResposta::TipoSemEntrega),
        ("s-bloq", ErroResposta::EntradaInexistente),
        ("nao-existe", ErroResposta::EntradaInexistente),
        ("s-outro", ErroResposta::AprovacaoIndisponivel),
        ("s-salto-aprov", ErroResposta::AprovacaoIndisponivel),
        ("s-salto-bloq", ErroResposta::Negado),
    ] {
        let r = sessao(&c, pedido_ssh(entrada, &["id"], None, None), |_| {});
        assert_eq!(r.falha, Some(erro), "{entrada}: {r:?}");
    }
    assert!(c.nada_executado());
    assert!(rt_sem_agente(&c));
}

/// D12 (V4.24, ataque): o agente lista a chave e assina só para o `ssh` filho; recusa
/// adicionar, remover e travar; um neto do `ssh` e o próprio teste (mesmo uid, outro
/// pid) são fechados sem nenhum byte. Pasta 0700, socket 0600, sem `SSH_AUTH_SOCK` no
/// ambiente; tudo some no fim da sessão.
#[test]
fn v4_24_d12_agente_so_do_filho() {
    let c = cena("d12");
    c.modo("normal");
    fs::write(c.base.join("esperar"), "").unwrap();
    let base = c.base.clone();
    let rt = c.rt.0.clone();
    let r = sessao(&c, pedido_ssh("s-chave", &["true"], None, None), move |s| {
        fechar_entrada(s);
        // Espera o `ssh` falso registrar o argv e atacar o socket daqui.
        assert!(esperar(Duration::from_secs(30), || {
            fs::read_dir(&base).unwrap().any(|e| {
                e.unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("registro-ssh-")
            })
        }));
        let pasta = fs::read_dir(&rt)
            .unwrap()
            .filter_map(Result::ok)
            .find(|e| e.file_name().to_string_lossy().starts_with("ssh-"))
            .unwrap()
            .path();
        let modo = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(modo(&pasta), 0o700);
        assert_eq!(modo(&pasta.join("agent.sock")), 0o600);
        assert_eq!(modo(&pasta.join("id.pub")), 0o600);
        let publica = fs::read_to_string(pasta.join("id.pub")).unwrap();
        assert!(publica.starts_with("ssh-ed25519 "), "{publica}");
        // Ataque: outro processo do mesmo usuário.
        let mut a = UnixStream::connect(pasta.join("agent.sock")).unwrap();
        a.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        use std::io::{Read, Write};
        let _ = a.write_all(&[0, 0, 0, 1, 11]);
        let mut buf = [0u8; 16];
        assert_eq!(
            a.read(&mut buf).unwrap_or(0),
            0,
            "o agente respondeu a um estranho"
        );
        fs::write(base.join("continuar"), "").unwrap();
    });
    assert_eq!(r.fim, Some(7), "{r:?}");
    let ssh = c.registro("ssh");
    assert_eq!(ssh.um("agente.lista"), "12", "{ssh:?}");
    assert_eq!(ssh.um("agente.quantas"), "1");
    assert_eq!(ssh.um("agente.assina"), "14", "{ssh:?}");
    assert!(ssh.um("agente.assinatura").len() > 100);
    for op in ["adiciona", "remove", "trava"] {
        assert_eq!(ssh.um(&format!("agente.{op}")), "5", "{op}: {ssh:?}");
    }
    assert_eq!(ssh.um("agente.neto"), "recusada", "{ssh:?}");
    assert_eq!(ssh.um("depois"), "12");
    let a = argv(&ssh);
    let o = opcoes(&a);
    assert!(o.contains(&"PubkeyAuthentication=yes".to_owned()));
    assert!(o.contains(&"IdentitiesOnly=yes".to_owned()));
    assert!(o.contains(&"PasswordAuthentication=no".to_owned()));
    assert!(
        o.iter()
            .any(|x| x.starts_with("IdentityAgent=") && x.ends_with("/agent.sock"))
    );
    assert!(
        o.iter()
            .any(|x| x.starts_with("IdentityFile=") && x.ends_with("/id.pub"))
    );
    assert!(c.registros("sshpass").is_empty());
    let esperado: BTreeSet<String> = ["LANG", "PATH"].map(String::from).into();
    assert_eq!(env(&ssh), esperado);
    // Fim da sessão: o agente e a pasta somem.
    assert!(rt_sem_agente(&c));
    let sock = o
        .iter()
        .find_map(|x| x.strip_prefix("IdentityAgent="))
        .unwrap();
    assert!(!Path::new(sock).exists());
}

/// D12: travar o cofre derruba o agente no meio da sessão; desde a Fase 54c (S8), a
/// sessão inteira (o `ssh` morre antes de pedir ao agente de novo, e a CLI recebe 137).
#[test]
fn d12_travar_derruba_o_agente() {
    let c = cena("d12trava");
    fs::write(c.base.join("esperar"), "").unwrap();
    let (base, d, rt) = (c.base.clone(), c.d.clone(), c.rt.0.clone());
    let r = sessao(&c, pedido_ssh("s-chave", &["true"], None, None), move |s| {
        fechar_entrada(s);
        assert!(esperar(Duration::from_secs(30), || {
            fs::read_dir(&base).unwrap().any(|e| {
                e.unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("registro-ssh-")
            })
        }));
        d.travar();
        assert!(esperar(Duration::from_secs(5), || {
            fs::read_dir(&rt).map_or(true, |d| {
                d.filter_map(Result::ok)
                    .all(|e| !e.file_name().to_string_lossy().starts_with("ssh-"))
            })
        }));
        fs::write(base.join("continuar"), "").unwrap();
    });
    assert_eq!(r.fim, Some(137), "{r:?}");
    let ssh = c.registro("ssh");
    assert_eq!(ssh.um("agente.lista"), "12");
    assert!(
        ssh.0.iter().all(|(k, v)| k != "depois" || v != "12"),
        "{ssh:?}"
    );
}

/// Fase 54c (S8; decisão do usuário, 2026-11-10), ataque: o agente abre `ward ssh` e a
/// sessão fica de pé (o `ssh` falso dorme 30 s, conectado). O usuário trava o cofre: a
/// sessão morre na hora, por senha (o `ssh` que o `sshpass` cria com `setsid()`, fora
/// do grupo, também) e por chave; a CLI recebe o aviso e 137, e nada do que o `ssh`
/// faria depois chega. Antes da correção, a sessão seguia até o fim (código 7, 30 s).
#[test]
fn f54c_ataque_ward_ssh_morre_na_trava() {
    for entrada in ["s-livre", "s-chave"] {
        let c = cena(&format!("f54c-{entrada}"));
        c.modo("parado");
        let travou = std::cell::Cell::new(None);
        let r = sessao(&c, pedido_ssh(entrada, &["sleep", "20"], None, None), |s| {
            fechar_entrada(s);
            assert!(esperar(Duration::from_secs(30), || {
                !c.registros("ssh").is_empty()
            }));
            std::thread::sleep(Duration::from_millis(300));
            c.d.travar();
            travou.set(Some(Instant::now()));
        });
        let depois = travou.get().unwrap().elapsed();
        assert_eq!(r.fim, Some(137), "{entrada}: {r:?}");
        assert!(depois < Duration::from_secs(5), "{entrada}: {depois:?}");
        let t = format!("{}{}", r.texto(), String::from_utf8_lossy(&r.erro));
        assert!(t.contains("conectado"), "{entrada}: {t}");
        assert!(
            t.contains("ward: o cofre travou; o comando foi encerrado"),
            "{entrada}: {t}"
        );
        assert!(!t.contains("depois-da-trava"), "{entrada}: {t}");
        // Nenhum processo da sessão sobra: nem o `ssh` (filho do `sshpass` com sessão
        // própria, na senha), nem o `sshpass`.
        for nome in ["ssh", "sshpass"] {
            for reg in c.registros(nome) {
                let pid = reg.um("pid");
                assert!(
                    esperar(Duration::from_secs(5), || {
                        fs::read_to_string(format!("/proc/{pid}/stat"))
                            .map_or(true, |s| s.contains(") Z "))
                    }),
                    "{entrada}: {nome} {pid} sobreviveu à trava"
                );
            }
        }
        if entrada == "s-livre" {
            assert_eq!(c.registros("sshpass").len(), 1);
        }
        assert!(rt_sem_agente(&c));
    }
}

/// D12 no `sftp` com chave: o `ssh` que o `sftp` chama (filho do filho) é atendido; o
/// neto dele não.
#[test]
fn d12_sftp_com_chave_atende_o_ssh_do_sftp() {
    let c = cena("d12sftp");
    let r = sessao(
        &c,
        pedido_ssh("f-sftp-chave", &[], None, None),
        fechar_entrada,
    );
    assert_eq!(r.fim, Some(0), "{r:?}");
    let ssh = c.registro("ssh");
    assert_eq!(ssh.um("agente.lista"), "12", "{ssh:?}");
    assert_eq!(ssh.um("agente.assina"), "14");
    assert_eq!(ssh.um("agente.neto"), "recusada");
    assert_eq!(ssh.um("ppid"), c.registro("sftp").um("pid"));
    assert!(rt_sem_agente(&c));
}

/// D34 revisto (auditoria do M4, oráculo de prefixo no pty): o filho imprime um
/// prefixo errado de 10 caracteres e para 1 s, depois o prefixo certo da senha e para
/// 1 s, depois o resto. Os dois prefixos chegam à CLI do mesmo jeito: em claro e na
/// hora (nada de `••••` no lugar do certo, nada segurado até a pausa ou até o resto).
/// O pedaço que completa a senha sai marcado. O filho tem terminal do tamanho pedido,
/// `TERM` e acompanha a janela nova.
#[test]
fn d34_ataque_pty_prefixo_certo_e_errado_saem_iguais_e_na_hora() {
    let c = cena("d34");
    c.modo("pty");
    let janela = Janela {
        linhas: 30,
        colunas: 100,
    };
    let pronto = c.base.join("pty-pronto");
    let r = sessao(
        &c,
        pedido_ssh("s-livre", &[], None, Some(janela)),
        move |s| {
            assert!(esperar(Duration::from_secs(60), || pronto.exists()));
            mandar(
                s,
                &Pedido::Janela(Janela {
                    linhas: 40,
                    colunas: 120,
                }),
            );
        },
    );
    assert_eq!(r.fim, Some(0), "{r:?}");
    let t = r.texto();
    let linha_do_tempo = || {
        r.pedacos
            .iter()
            .map(|(t, p)| (t, String::from_utf8_lossy(p).into_owned()))
            .collect::<Vec<_>>()
    };
    assert!(t.contains("tty=True/True 30x100"), "{t}");
    assert!(t.contains("fim 40x120"), "{t}");
    let certo = format!("[{}", &SENHA_SSH[..10]);
    let errado: String = SENHA_SSH[..10]
        .chars()
        .map(|a| if a == 'X' { 'Y' } else { 'X' })
        .collect();
    let errado = format!("<{errado}");
    // Os dois prefixos saem em claro, iguais ao que o filho escreveu.
    assert!(t.contains(&errado), "{t}");
    assert!(t.contains(&certo), "o prefixo certo foi trocado: {t}");
    // A senha inteira nunca sai; o pedaço que a completa vira marca.
    assert!(!contem(&r.saida, SENHA_SSH.as_bytes()), "{t}");
    assert!(t.contains(&format!("{certo}••••]")), "{t}");
    // Tempo: cada prefixo chega à CLI antes da pausa de 1 s acabar (nada segurado até
    // a saída seguinte); a chegada não depende de ser o prefixo certo.
    let quando = |marca: &str| {
        r.pedacos
            .iter()
            .find(|(_, p)| contem(p, marca.as_bytes()))
            .map(|(t, _)| *t)
            .unwrap_or_else(|| panic!("sem {marca}: {:?}", linha_do_tempo()))
    };
    let (t_errado, t_fecha) = (quando(&errado), quando(">"));
    let (t_certo, t_resto) = (quando(&certo), quando("••••"));
    for (nome, t0, t1) in [("errado", t_errado, t_fecha), ("certo", t_certo, t_resto)] {
        assert!(
            t1.saturating_sub(t0) >= Duration::from_millis(600),
            "prefixo {nome} segurado até a saída seguinte: {:?}",
            linha_do_tempo()
        );
    }
    let sp = c.registro("sshpass");
    assert!(env(&sp).contains("TERM"));
    // Interativo: sem comando remoto (nada depois do host).
    let a = argv(&sp);
    assert_eq!(a.last().unwrap(), "127.0.0.1");
}

/// D34 revisto (auditoria do M4, `tempo.py`): sem pty (`ward ssh` com comando), o
/// filho escreve `A`, para, um candidato a prefixo, para, e `Z`. O candidato chega à
/// CLI antes do `Z`, seja ou não o começo da senha (antes, o começo da senha ficava
/// segurado até o `Z`: oráculo por tempo).
#[test]
fn d34_ataque_tempo_sem_pty_prefixo_nao_atrasa() {
    let c = cena("d34tempo");
    c.modo("tempo");
    let r = sessao(
        &c,
        pedido_ssh("s-livre", &["x"], None, None),
        fechar_entrada,
    );
    assert_eq!(r.fim, Some(0), "{r:?}");
    let linha_do_tempo: Vec<_> = r
        .pedacos
        .iter()
        .map(|(t, p)| (*t, String::from_utf8_lossy(p).into_owned()))
        .collect();
    let quando = |marca: &str| {
        linha_do_tempo
            .iter()
            .find(|(_, p)| p.contains(marca))
            .map(|(t, _)| *t)
            .unwrap_or_else(|| panic!("sem {marca}: {linha_do_tempo:?}"))
    };
    for i in 0..4 {
        let (t_cand, t_z) = (quando(&format!("C{i}=")), quando(&format!("Z{i}")));
        assert!(
            t_z.saturating_sub(t_cand) >= Duration::from_millis(400),
            "candidato {i} segurado até o Z: {linha_do_tempo:?}"
        );
    }
    assert!(!contem(&r.saida, SENHA_SSH.as_bytes()));
}

/// Auditoria do M4 (baixo): `known_hosts` do Ward frouxo (outro usuário poderia ler os
/// hosts ou, com escrita, plantar uma chave de host) recusa antes de executar qualquer
/// coisa; se não existe, nasce 0600.
#[test]
fn ataque_known_hosts_frouxo_recusa_sem_executar() {
    let c = cena("khfrouxo");
    c.modo("normal");
    let kh = c.dir.0.join("known_hosts");
    let r = sessao(
        &c,
        pedido_ssh("s-livre", &["true"], None, None),
        fechar_entrada,
    );
    assert_eq!(r.fim, Some(7), "{r:?}");
    assert_eq!(
        fs::metadata(&kh).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let antes = c.registros("ssh").len();
    for modo in [0o644, 0o660, 0o606] {
        fs::set_permissions(&kh, fs::Permissions::from_mode(modo)).unwrap();
        let r = sessao(
            &c,
            pedido_ssh("s-livre", &["true"], None, None),
            fechar_entrada,
        );
        assert_eq!(
            r.falha,
            Some(ErroResposta::EntregaFalhou),
            "{modo:o}: {r:?}"
        );
    }
    fs::set_permissions(&kh, fs::Permissions::from_mode(0o600)).unwrap();
    fs::set_permissions(&c.dir.0, fs::Permissions::from_mode(0o755)).unwrap();
    let r = sessao(
        &c,
        pedido_ssh("s-livre", &["true"], None, None),
        fechar_entrada,
    );
    fs::set_permissions(&c.dir.0, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(r.falha, Some(ErroResposta::EntregaFalhou), "{r:?}");
    assert_eq!(
        c.registros("ssh").len(),
        antes,
        "rodou com known_hosts frouxo"
    );
}

/// Auditoria do M4 (alto): no `sftp` com chave, o `!cmd` do `sftp` é outro filho dele
/// e usava o agente (`ssh-add -L`, login com a chave noutro host). O agente fica preso
/// ao `ssh` que o `sftp` criou: o `!cmd` é recusado com o `ssh` vivo e depois que ele
/// saiu; o `ssh` continua atendido.
#[test]
fn ataque_sftp_bang_nao_usa_o_agente() {
    let c = cena("sftpbang");
    c.modo("sftp-bang");
    fs::write(c.base.join("esperar"), "").unwrap();
    let r = sessao(
        &c,
        pedido_ssh("f-sftp-chave", &[], None, None),
        fechar_entrada,
    );
    assert_eq!(r.fim, Some(0), "{r:?}");
    let ssh = c.registro("ssh");
    assert_eq!(ssh.um("agente.lista"), "12", "{ssh:?}");
    assert_eq!(
        ssh.um("depois"),
        "12",
        "o ssh do sftp perdeu o agente: {ssh:?}"
    );
    let sftp = c.registro("sftp");
    assert_eq!(sftp.um("bang_durante"), "recusada", "{sftp:?}");
    assert_eq!(sftp.um("bang_depois"), "recusada", "{sftp:?}");
    assert!(rt_sem_agente(&c));
}

/// D3 / V5.24 (ataques): o token do "Conectar" é de uso único, vale 30 s, só para a
/// entrada dele e só o app emite. Inválido, vencido, repetido ou de outra entrada:
/// vale o nível (aqui, com aprovação sem app: `aprovacao_indisponivel`), nada roda.
#[test]
fn v5_24_token_do_conectar() {
    let c = cena("token");
    c.modo("normal");
    let rodar = |token: Option<&Segredo>, entrada: &str| {
        sessao(
            &c,
            pedido_ssh(entrada, &["true"], token, None),
            fechar_entrada,
        )
    };
    let indisponivel = Some(ErroResposta::AprovacaoIndisponivel);

    // Só o app emite; nada de token para tipo sem `ward ssh`.
    for papel in [Papel::Agente, Papel::Extensao] {
        let r = c.d.atender(
            papel,
            Pedido::EmitirToken {
                entrada: "s-outro".into(),
            },
        );
        assert_eq!(erro(&r), Some(ErroResposta::NaoAutorizado), "{papel:?}");
    }
    let r = c.d.atender(
        Papel::App,
        Pedido::EmitirToken {
            entrada: "t-livre".into(),
        },
    );
    assert_eq!(erro(&r), Some(ErroResposta::PedidoInvalido));

    // Válido: pula a aprovação uma vez.
    let t = c.token("s-outro");
    assert_eq!(t.expor().len(), 32);
    assert!(t.expor().iter().all(u8::is_ascii_alphanumeric));
    assert_eq!(rodar(Some(&t), "s-outro").fim, Some(7));
    // Repetido: vale o nível.
    assert_eq!(rodar(Some(&t), "s-outro").falha, indisponivel);
    assert_eq!(c.registros("ssh").len(), 1);

    // Inventado (no formato certo).
    let falso = Segredo::de_texto("AbCdEfGhIjKlMnOpQrStUvWxYz012345".into());
    assert_eq!(rodar(Some(&falso), "s-outro").falha, indisponivel);

    // De outra entrada: não vale e morre ali.
    let t = c.token("s-livre");
    assert_eq!(rodar(Some(&t), "s-outro").falha, indisponivel);
    let antes = c.registros("ssh").len();
    let r = rodar(Some(&t), "s-livre");
    assert_eq!(r.fim, Some(7), "s-livre é livre: roda pelo nível");
    assert_eq!(c.registros("ssh").len(), antes + 1);

    // Vencido: 30 s no relógio do motor.
    let t = c.token("s-outro");
    c.relogio.avancar(Duration::from_secs(31));
    assert_eq!(rodar(Some(&t), "s-outro").falha, indisponivel);
    let t = c.token("s-outro");
    c.relogio.avancar(Duration::from_secs(29));
    assert_eq!(rodar(Some(&t), "s-outro").fim, Some(7));

    // Travar apaga os tokens.
    let t = c.token("s-outro");
    c.d.travar();
    assert!(ok(&destravar(&c.d, SENHA)));
    assert_eq!(rodar(Some(&t), "s-outro").falha, indisponivel);

    // Bloqueada continua inexistente, mesmo com token válido.
    let t = c.token("s-bloq");
    assert_eq!(
        rodar(Some(&t), "s-bloq").falha,
        Some(ErroResposta::EntradaInexistente)
    );

    // Tokens demais: o mais antigo sai.
    let primeiro = c.token("s-outro");
    for _ in 0..16 {
        let _ = c.token("s-outro");
    }
    assert_eq!(rodar(Some(&primeiro), "s-outro").falha, indisponivel);
}

// --- Fase 28: salto e túnel pela ponte (sem `ProxyJump`, sem shell) --------------------

/// Espera o registro do programa falso `nome` cujo argv tem `marca` (inteiro ou como
/// começo de um argumento).
fn esperar_registro(c: &Cena, nome: &str, marca: &str) -> Reg {
    let mut achado = None;
    assert!(
        esperar(Duration::from_secs(30), || {
            achado = c
                .registros(nome)
                .into_iter()
                .find(|r| argv(r).iter().any(|a| a.starts_with(marca)));
            achado.is_some()
        }),
        "sem registro de {nome} com {marca}"
    );
    achado.unwrap()
}

/// O valor depois de `op` no argv.
fn depois_de(a: &[String], op: &str) -> String {
    let i = a.iter().position(|x| x == op).unwrap();
    a[i + 1].clone()
}

/// Ataque: um processo de fora da sessão (este teste: mesmo uid, outro grupo de
/// processos) conecta na ponte. É fechado sem nenhum byte e nada é lançado.
fn estranho_e_recusado(porta: u16) {
    use std::io::{Read, Write};
    let mut t = std::net::TcpStream::connect(("127.0.0.1", porta)).unwrap();
    t.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let _ = t.write_all(b"SSH-2.0-estranho\r\n");
    let mut buf = [0u8; 64];
    assert_eq!(
        t.read(&mut buf).unwrap_or(0),
        0,
        "a ponte respondeu a um estranho"
    );
}

/// O inode do socket que escuta em `127.0.0.1:<porta>` (`/proc/net/tcp`, estado `0A`).
/// A conferência do fim usa o inode, não a porta: com os testes em paralelo, outra
/// ponte pode receber do kernel o mesmo número de porta logo depois.
fn inode_da_escuta(porta: u16) -> u64 {
    let t = fs::read_to_string("/proc/net/tcp").unwrap();
    let local = format!("0100007F:{porta:04X}");
    t.lines()
        .skip(1)
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .find(|c| c.len() > 9 && c[1] == local && c[3] == "0A")
        .and_then(|c| c[9].parse().ok())
        .unwrap_or_else(|| panic!("ninguém escuta em 127.0.0.1:{porta}"))
}

/// A escuta da ponte (pelo inode) foi fechada? (O daemon roda neste processo.)
fn escuta_fechada(inode: u64) -> bool {
    let alvo = format!("socket:[{inode}]");
    esperar(Duration::from_secs(5), || {
        !fs::read_dir("/proc/self/fd")
            .unwrap()
            .any(|e| fs::read_link(e.unwrap().path()).is_ok_and(|l| l.as_os_str() == alvo.as_str()))
    })
}

fn ninguem_com_w(c: &Cena) -> bool {
    !c.registros("ssh")
        .iter()
        .any(|r| argv(r).contains(&"-W".to_owned()))
}

/// Confere o `ssh -W` da ponte (D13, D43): `-F /dev/null`, `known_hosts` do Ward,
/// usuário só depois de `-l`, host depois de `--`, alvo em `-W`, nada de `ProxyJump`,
/// `User=` ou `usuário@host`.
fn conferir_argv_da_ponte(c: &Cena, a: &[String], alvo: &str, usuario: &str) {
    // O registro tem o argv sem o programa (o `ssh` falso de `c.base`).
    let _ = c;
    assert_eq!(&a[0..2], ["-F", "/dev/null"], "{a:?}");
    let o = opcoes(a);
    for x in [
        "IdentitiesOnly=yes",
        "CertificateFile=/dev/null",
        "GlobalKnownHostsFile=/dev/null",
        "StrictHostKeyChecking=accept-new",
        "ProxyCommand=none",
        "ControlMaster=no",
        "ForwardAgent=no",
        "PermitLocalCommand=no",
        "ExitOnForwardFailure=yes",
    ] {
        assert!(o.contains(&x.to_owned()), "{x}: {a:?}");
    }
    assert!(o.iter().any(|x| x.starts_with("UserKnownHostsFile=")
        && x.ends_with("/known_hosts")
        && !x.contains("/.ssh/")));
    let fim: Vec<String> = ["-W", alvo, "-p", "2222", "-l", usuario, "--", "127.0.0.1"]
        .map(String::from)
        .into();
    assert_eq!(&a[a.len() - 8..], fim.as_slice(), "{a:?}");
    assert!(
        !a.iter().any(|x| x.contains("ProxyJump")
            || x.starts_with("User=")
            || x.ends_with("@127.0.0.1"))
    );
}

/// Fase 28 / V4.25 / V4.35 (D13, D19, D43; S1, S3, S4): `ward ssh` de um `servidor`
/// com salto, senha nos dois. A ponte só aceita o `ssh` principal (o teste, de fora,
/// é recusado sem byte e sem lançar nada; a segunda conexão também); o `ssh` do salto
/// recebe a senha do salto só em `SSHPASS`; o principal conecta em `127.0.0.1:<porta>`
/// com `HostKeyAlias` do destino; o `stderr` do salto chega mascarado; no fim a ponte
/// fecha.
#[test]
fn fase28_salto_com_senha_pela_ponte() {
    let c = cena("salto");
    c.modo("normal");
    fs::write(c.base.join("esperar-ponte"), "").unwrap();
    let porta = std::cell::Cell::new(0u16);
    let inode = std::cell::Cell::new(0u64);
    let r = sessao(&c, pedido_ssh("s-salto", &["hostname"], None, None), |s| {
        fechar_entrada(s);
        let principal = esperar_registro(&c, "ssh", "HostKeyAlias=");
        let pt: u16 = depois_de(&argv(&principal), "-p").parse().unwrap();
        porta.set(pt);
        inode.set(inode_da_escuta(pt));
        estranho_e_recusado(pt);
        assert!(
            ninguem_com_w(&c),
            "a conexão do estranho lançou o ssh do salto"
        );
        fs::write(c.base.join("continuar-ponte"), "").unwrap();
    });
    assert_eq!(r.fim, Some(0), "{r:?}");
    let t = r.texto();
    assert!(t.contains("ponte=eco:127.0.0.1:2222:oi"), "{t}");
    assert!(
        t.contains("segunda=fechada"),
        "a ponte do salto aceitou duas: {t}"
    );
    let erro = String::from_utf8_lossy(&r.erro).into_owned();
    assert!(erro.contains("erro-salto ••••"), "{erro}");
    assert!(!contem(&r.saida, SENHA_SSH.as_bytes()));
    assert!(!contem(&r.erro, SENHA_SSH.as_bytes()));
    let pt = porta.get();
    assert!(
        escuta_fechada(inode.get()),
        "a ponte ficou aberta depois da sessão"
    );

    // O principal: `sshpass -e ssh … -p <ponte> -l deploy -- 127.0.0.1 hostname`.
    let principal = esperar_registro(&c, "ssh", "HostKeyAlias=");
    let a = argv(&principal);
    let o = opcoes(&a);
    assert!(
        o.contains(&"HostKeyAlias=[127.0.0.1]:2222".to_owned()),
        "{a:?}"
    );
    assert!(o.contains(&"CheckHostIP=no".to_owned()));
    assert!(o.contains(&"ProxyCommand=none".to_owned()));
    let fim: Vec<String> = [
        "-p",
        &pt.to_string(),
        "-l",
        "deploy",
        "--",
        "127.0.0.1",
        "hostname",
    ]
    .map(String::from)
    .into();
    assert_eq!(&a[a.len() - 7..], fim.as_slice());
    assert!(
        !a.iter()
            .any(|x| x.contains("ProxyJump") || x.starts_with("User="))
    );

    // O do salto: usuário com `@` inteiro depois de `-l` (D43), senha só no ambiente.
    let salto = esperar_registro(&c, "ssh", "-W");
    let a = argv(&salto);
    conferir_argv_da_ponte(&c, &a, "127.0.0.1:2222", "deploy@cliente.com.br");
    let o = opcoes(&a);
    assert!(o.contains(&"PubkeyAuthentication=no".to_owned()));
    assert!(o.contains(&"IdentityAgent=none".to_owned()));
    assert!(o.contains(&"NumberOfPasswordPrompts=1".to_owned()));
    let esperado: BTreeSet<String> = ["LANG", "PATH", "SSHPASS"].map(String::from).into();
    assert_eq!(env(&salto), esperado);
    assert_eq!(salto.um("sshpass_sha256"), sha256(SENHA_SSH.as_bytes()));
    // Dois `sshpass -e` (o principal e o do salto), nenhum com a senha em argv.
    let sp = c.registros("sshpass");
    assert_eq!(sp.len(), 2, "{sp:?}");
    for r in &sp {
        assert_eq!(argv(r)[0], "-e");
        assert!(!argv(r).iter().any(|x| x.contains(SENHA_SSH)));
    }
    assert!(rt_sem_agente(&c));
}

/// Fase 28 / D12: chave no principal (como `acme-ssh-interno`) e chave no salto. Cada
/// `ssh` tem o seu agente, que só atende a ele (o neto é recusado); tudo some no fim.
#[test]
fn fase28_salto_com_chave_agente_de_cada_ssh() {
    // Principal com chave, salto com senha.
    let c = cena("saltochave");
    let r = sessao(
        &c,
        pedido_ssh("s-salto-chave", &["hostname"], None, None),
        fechar_entrada,
    );
    assert_eq!(r.fim, Some(0), "{r:?}");
    assert!(r.texto().contains("ponte=eco:127.0.0.1:2222:oi"), "{r:?}");
    let principal = esperar_registro(&c, "ssh", "HostKeyAlias=");
    assert_eq!(principal.um("agente.lista"), "12", "{principal:?}");
    assert_eq!(principal.um("agente.assina"), "14");
    assert_eq!(principal.um("agente.neto"), "recusada");
    let o = opcoes(&argv(&principal));
    assert!(o.contains(&"PubkeyAuthentication=yes".to_owned()));
    assert_eq!(c.registros("sshpass").len(), 1, "só o salto usa sshpass");
    assert!(rt_sem_agente(&c));

    // Principal com senha, salto com chave.
    let c = cena("viachave");
    let r = sessao(
        &c,
        pedido_ssh("s-via-chave", &["hostname"], None, None),
        fechar_entrada,
    );
    assert_eq!(r.fim, Some(0), "{r:?}");
    assert!(r.texto().contains("ponte=eco:127.0.0.1:2222:oi"), "{r:?}");
    let salto = esperar_registro(&c, "ssh", "-W");
    assert_eq!(salto.um("agente.lista"), "12", "{salto:?}");
    assert_eq!(salto.um("agente.assina"), "14");
    for op in ["adiciona", "remove", "trava"] {
        assert_eq!(salto.um(&format!("agente.{op}")), "5", "{op}");
    }
    assert_eq!(salto.um("agente.neto"), "recusada");
    conferir_argv_da_ponte(&c, &argv(&salto), "127.0.0.1:2222", "deploy");
    let esperado: BTreeSet<String> = ["LANG", "PATH"].map(String::from).into();
    assert_eq!(env(&salto), esperado, "sem SSHPASS nem SSH_AUTH_SOCK");
    assert!(rt_sem_agente(&c));
}

/// Fase 28 / V4.29 (D10, D19): `ward run` de um `banco` com túnel. O comando recebe
/// `PGHOST=127.0.0.1` e a porta da ponte e conversa com o alvo pelo `ssh -W` do
/// `servidor` do túnel (duas conexões); um estranho é recusado sem lançar nada; a
/// senha do banco e a do servidor do túnel saem mascaradas (D19); a ponte fecha no fim.
#[test]
fn fase28_tunel_do_banco_no_run() {
    let c = cena("tunel");
    let marca = c.base.join("continuar-tunel");
    let arq_porta = c.base.join("porta-tunel");
    let script = format!(
        r#"
import os, socket, time
porta = int(os.environ["PGPORT"])
open({arq:?}, "w").write(str(porta))
print("PGHOST=" + os.environ["PGHOST"], flush=True)
print("PGPORT=" + os.environ["PGPORT"], flush=True)
print("url=" + os.environ["DATABASE_URL"], flush=True)
print("senha=" + os.environ["PGPASSWORD"], flush=True)
print("servidor={servidor}", flush=True)
fim = time.time() + 30
while time.time() < fim and not os.path.exists({marca:?}):
    time.sleep(0.02)
for i in range(2):
    s = socket.create_connection(("127.0.0.1", porta), timeout=60)
    s.sendall(b"select %d\n" % i)
    d = b""
    while not d.endswith(b"\n"):
        x = s.recv(4096)
        if not x:
            break
        d += x
    print("resposta=" + d.decode().strip(), flush=True)
    s.close()
"#,
        arq = arq_porta.to_str().unwrap(),
        marca = marca.to_str().unwrap(),
        servidor = SENHA_SSH,
    );
    let pedido = Pedido::Executar(ward_core::protocolo::PedidoExecutar {
        entrada: "b-tunel".into(),
        argv: ["/usr/bin/python3", "-I", "-c", script.as_str()]
            .iter()
            .map(|x| Zeroizing::new(x.as_bytes().to_vec()))
            .collect(),
        cwd: Zeroizing::new(b"/".to_vec()),
        tipo_entrada: ward_core::protocolo::TipoEntradaPadrao::Nula,
        ambiente: vec![ward_core::protocolo::Variavel {
            nome: Zeroizing::new(b"PATH".to_vec()),
            valor: Segredo::de_texto("/usr/bin:/bin".into()),
        }],
    });
    let porta = std::cell::Cell::new(0u16);
    let inode = std::cell::Cell::new(0u64);
    let r = sessao_de(
        &c,
        pedido,
        |s| {
            fechar_entrada(s);
            assert!(esperar(Duration::from_secs(30), || {
                fs::read_to_string(&arq_porta).is_ok_and(|t| t.parse::<u16>().is_ok())
            }));
            let pt: u16 = fs::read_to_string(&arq_porta).unwrap().parse().unwrap();
            porta.set(pt);
            inode.set(inode_da_escuta(pt));
            estranho_e_recusado(pt);
            assert!(
                ninguem_com_w(&c),
                "a conexão do estranho lançou o ssh do túnel"
            );
            fs::write(&marca, "").unwrap();
        },
        true,
    );
    assert_eq!(r.fim, Some(0), "{r:?}");
    let t = r.texto();
    let pt = porta.get();
    assert!(t.contains("PGHOST=127.0.0.1"), "{t}");
    assert!(t.contains(&format!("PGPORT={pt}\n")), "{t}");
    // O `DATABASE_URL` inteiro é segredo (D11): sai mascarado.
    assert!(t.contains("url=••••"), "{t}");
    assert!(t.contains("resposta=eco:db.interno:5432:select 0"), "{t}");
    assert!(t.contains("resposta=eco:db.interno:5432:select 1"), "{t}");
    assert!(t.contains("senha=••••"), "{t}");
    assert!(
        t.contains("servidor=••••"),
        "D19: a senha do túnel na máscara: {t}"
    );
    for segredo in [SENHA_SSH, SENHA_BANCO] {
        assert!(!contem(&r.saida, segredo.as_bytes()));
        assert!(!contem(&r.erro, segredo.as_bytes()));
    }
    let erro = String::from_utf8_lossy(&r.erro).into_owned();
    assert!(erro.contains("erro-salto ••••"), "{erro}");
    assert!(
        escuta_fechada(inode.get()),
        "o túnel ficou aberto depois do comando"
    );
    let tuneis: Vec<Reg> = c
        .registros("ssh")
        .into_iter()
        .filter(|r| argv(r).contains(&"-W".to_owned()))
        .collect();
    assert_eq!(tuneis.len(), 2, "um ssh -W por conexão");
    for r in &tuneis {
        conferir_argv_da_ponte(&c, &argv(r), "db.interno:5432", "deploy@cliente.com.br");
        assert_eq!(r.um("sshpass_sha256"), sha256(SENHA_SSH.as_bytes()));
    }
    assert!(rt_sem_agente(&c));
}

/// Fase 28 (D12 + D19): travar o cofre durante o `ward run` com túnel faz a ponte parar
/// de aceitar conexões novas; desde a Fase 54 (S8), o próprio comando morre (137).
#[test]
fn fase28_travar_fecha_o_tunel_para_conexoes_novas() {
    let c = cena("tuneltrava");
    let marca = c.base.join("continuar-tunel");
    let arq_porta = c.base.join("porta-tunel");
    let script = format!(
        r#"
import os, socket, time
porta = int(os.environ["PGPORT"])
open({arq:?}, "w").write(str(porta))
fim = time.time() + 30
while time.time() < fim and not os.path.exists({marca:?}):
    time.sleep(0.02)
try:
    s = socket.create_connection(("127.0.0.1", porta), timeout=5)
    s.sendall(b"x\n")
    print("depois=" + (s.recv(64).decode() or "fechada"), flush=True)
except OSError as e:
    print("depois=" + type(e).__name__, flush=True)
"#,
        arq = arq_porta.to_str().unwrap(),
        marca = marca.to_str().unwrap(),
    );
    let pedido = Pedido::Executar(ward_core::protocolo::PedidoExecutar {
        entrada: "b-tunel".into(),
        argv: ["/usr/bin/python3", "-I", "-c", script.as_str()]
            .iter()
            .map(|x| Zeroizing::new(x.as_bytes().to_vec()))
            .collect(),
        cwd: Zeroizing::new(b"/".to_vec()),
        ambiente: Vec::new(),
        tipo_entrada: ward_core::protocolo::TipoEntradaPadrao::Nula,
    });
    let r = sessao_de(
        &c,
        pedido,
        |s| {
            fechar_entrada(s);
            assert!(esperar(Duration::from_secs(30), || {
                fs::read_to_string(&arq_porta).is_ok_and(|t| t.parse::<u16>().is_ok())
            }));
            c.d.travar();
            std::thread::sleep(Duration::from_millis(400));
            fs::write(&marca, "").unwrap();
        },
        true,
    );
    // Fase 54 (S8): a trava mata o `ward run` inteiro (137), além de fechar a ponte.
    assert_eq!(r.fim, Some(137), "{r:?}");
    let t = r.texto();
    assert!(
        !t.contains("depois=eco"),
        "a ponte aceitou depois de travar: {t}"
    );
    assert!(ninguem_com_w(&c));
}
