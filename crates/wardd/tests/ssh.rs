//! `ward ssh` no núcleo (Fase 27), com `ssh`, `sshpass` e `sftp` falsos
//! (`tests/ssh_falso.py`) que registram o argv e nunca conectam: D3 (token do
//! "Conectar", V5.24, V4.32), D12 (agente só do filho, V4.24), D13 (`-F /dev/null`,
//! `known_hosts` próprio, ambiente do zero), D34 (pausa no pty), D43 (usuário com `@`,
//! V4.35) e D56 (`sftp` com `User=`, V4.37).
//!
//! A sessão roda como no servidor (`wardd::ssh::sessao` numa ponta de um par de
//! sockets); a outra ponta faz o papel da CLI. O cofre é de teste, criado pelo núcleo
//! com valores falsos; a chave SSH é gerada aqui, com passphrase aleatória.

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
    salvar(&d, &sftp_json("f-sftp", "sftp", &auth_senha()));
    salvar(&d, &sftp_json("f-sftp-chave", "sftp", &auth_chave));
    salvar(&d, &sftp_json("f-ftp", "ftp", &auth_senha()));
    for n in [
        "s-livre",
        "s-chave",
        "s-salto",
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
    let (mut lado_cli, lado_daemon) = UnixStream::pair().unwrap();
    lado_cli
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    let d = c.d.clone();
    let t = std::thread::spawn(move || wardd::ssh::sessao(&d, &SemCliente, &lado_daemon, pedido));
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

fn fechar_entrada(s: &mut UnixStream) {
    mandar(
        s,
        &Pedido::EntradaPadrao {
            dados: Segredo::de_bytes(Vec::new()),
        },
    );
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
/// (inexistente), com aprovação sem app (vale o nível) e salto (Fase 28).
#[test]
fn recusas_sem_executar_nada() {
    let c = cena("recusas");
    for (entrada, erro) in [
        ("t-livre", ErroResposta::TipoSemEntrega),
        ("f-ftp", ErroResposta::TipoSemEntrega),
        ("s-bloq", ErroResposta::EntradaInexistente),
        ("nao-existe", ErroResposta::EntradaInexistente),
        ("s-outro", ErroResposta::AprovacaoIndisponivel),
        ("s-salto", ErroResposta::EntregaFalhou),
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

/// D12: travar o cofre derruba o agente no meio da sessão.
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
    assert_eq!(r.fim, Some(7), "{r:?}");
    let ssh = c.registro("ssh");
    assert_eq!(ssh.um("agente.lista"), "12");
    assert_ne!(ssh.um("depois"), "12", "{ssh:?}");
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

/// D34 (pty): o começo da senha parado vira `••••` 300 ms depois da última saída,
/// sem esperar o resto; o resto, quando chega, continua escondido (o casamento segue).
/// O filho tem terminal do tamanho pedido, `TERM` e acompanha a janela nova.
#[test]
fn d34_pty_prefixo_parado_vira_marca_e_o_resto_nao_sai() {
    let c = cena("d34");
    c.modo("pty");
    let janela = Janela {
        linhas: 30,
        colunas: 100,
    };
    let r = sessao(&c, pedido_ssh("s-livre", &[], None, Some(janela)), |s| {
        std::thread::sleep(Duration::from_millis(300));
        mandar(
            s,
            &Pedido::Janela(Janela {
                linhas: 40,
                colunas: 120,
            }),
        );
    });
    assert_eq!(r.fim, Some(0), "{r:?}");
    let t = r.texto();
    assert!(t.contains("tty=True/True 30x100"), "{t}");
    assert!(t.contains("fim 40x120"), "{t}");
    assert!(t.contains("[••••"), "{t}");
    let s = SENHA_SSH.as_bytes();
    for k in 6..=s.len() {
        for w in s.windows(k) {
            assert!(!contem(&r.saida, w), "pedaço da senha na saída: {t}");
        }
    }
    // A marca chegou na pausa (≈300 ms depois do prefixo), antes do resto (1 s depois).
    let (t_tty, _) = r.pedacos.iter().find(|(_, p)| contem(p, b"[")).unwrap();
    let (t_marca, _) = r
        .pedacos
        .iter()
        .find(|(_, p)| contem(p, "••••".as_bytes()))
        .unwrap();
    let dt = t_marca.saturating_sub(*t_tty);
    assert!(
        dt >= Duration::from_millis(250) && dt < Duration::from_millis(950),
        "marca em {dt:?}: {:?}",
        r.pedacos
            .iter()
            .map(|(t, p)| (t, String::from_utf8_lossy(p)))
            .collect::<Vec<_>>()
    );
    let sp = c.registro("sshpass");
    assert!(env(&sp).contains("TERM"));
    // Interativo: sem comando remoto (nada depois do host).
    let a = argv(&sp);
    assert_eq!(a.last().unwrap(), "127.0.0.1");
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
