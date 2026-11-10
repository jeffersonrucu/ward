//! `ward playwright` no núcleo (Fase 44; D6, D9; S1–S3, S13), com um Chrome falso
//! (`tests/chrome_falso.py`) que fala o DevTools Protocol pelo pipe de depuração (fds 3
//! e 4) e nunca abre rede. O teste de ponta a ponta com o Chrome de verdade e os
//! serviços de teste é o `make test-playwright` (`dev/tests/playwright.sh`).
//!
//! O Chrome falso entra por `Daemon::com_chrome`, que só existe com a feature
//! `test-hooks` (o `make test` liga todas).

#![cfg(feature = "test-hooks")]

mod comum;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use comum::*;
use ward_core::protocolo::{ErroResposta, Papel, Resposta};
use wardd::aprovacao::SemCliente;
use wardd::nucleo::Daemon;

/// Senha da entrada `t-livre` (falsa).
const SENHA_LOGIN: &str = SEGREDOS[3];

/// `<rt>` só deste teste em `/run/user/<uid>/`.
struct Rt(PathBuf);

impl Rt {
    fn novo(rotulo: &str) -> Rt {
        let p = ward_platform::socket::runtime_dir()
            .join(format!("wpw-{}-{rotulo}", std::process::id()));
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
    _dir: Dir,
    rt: Rt,
    base: PathBuf,
}

fn cena(rotulo: &str, modo: &str) -> Cena {
    let dir = Dir::novo(rotulo);
    let (d, _) = cofre_de_teste(&dir);
    let base = dir.0.join("chrome");
    fs::create_dir(&base).unwrap();
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/chrome_falso.py");
    let chrome = base.join("chrome");
    fs::write(
        &chrome,
        format!(
            "#!/bin/sh\nexec /usr/bin/python3 -I '{}' '{}' \"$@\"\n",
            script.display(),
            base.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&chrome, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(base.join("modo"), modo).unwrap();
    let rt = Rt::novo(rotulo);
    let d = d.com_chrome(chrome).com_temporarios(rt.0.clone());
    Cena {
        d: Arc::new(d),
        _dir: dir,
        rt,
        base,
    }
}

impl Cena {
    fn playwright(&self, entrada: &str) -> Resposta {
        let p = ped(
            Papel::Agente,
            "playwright",
            &format!(r#"{{"entrada":"{entrada}"}}"#),
        )
        .unwrap();
        self.d.atender_de(&SemCliente, Papel::Agente, p)
    }

    fn registro(&self) -> Vec<serde_json::Value> {
        fs::read_to_string(self.base.join("registro"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn eventos(&self, nome: &str) -> Vec<serde_json::Value> {
        self.registro()
            .into_iter()
            .filter(|e| e["evento"] == nome)
            .collect()
    }

    fn sessao(&self, entrada: &str) -> PathBuf {
        self.rt.0.join("playwright").join(format!("{entrada}.json"))
    }

    /// Nenhum perfil `pw-*` ficou.
    fn sem_perfil(&self) -> bool {
        fs::read_dir(&self.rt.0).map_or(true, |it| {
            !it.flatten()
                .any(|e| e.file_name().to_string_lossy().starts_with("pw-"))
        })
    }
}

fn sha256_hex(b: &[u8]) -> String {
    let out = std::process::Command::new("/usr/bin/sha256sum")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut c| {
            use std::io::Write;
            c.stdin.take().unwrap().write_all(b)?;
            c.wait_with_output()
        })
        .unwrap();
    String::from_utf8(out.stdout).unwrap()[..64].to_owned()
}

/// Todas as formas da senha que não podem aparecer (literal, base64, URL).
fn formas_da_senha() -> Vec<Vec<u8>> {
    vec![
        SENHA_LOGIN.as_bytes().to_vec(),
        b64(SENHA_LOGIN.as_bytes()).into_bytes(),
        SENHA_LOGIN.as_bytes()[2..SENHA_LOGIN.len() - 2].to_vec(),
    ]
}

/// V6.12/V6.13/V6.14 com o Chrome falso: a resposta é só o caminho; o storage state é
/// 0600, no formato do Playwright e sem a senha; o argv tem o pipe e nenhum segredo; o
/// ambiente do Chrome é o montado do zero; só os fds 0–4 (0–2 em `/dev/null`); a senha
/// só chega ao preenchimento (conferida por sha256); o perfil some antes da resposta; e
/// travar apaga a sessão.
#[test]
fn v6_12_sessao_gravada_so_o_caminho_volta() {
    let c = cena("ok", "ok");
    let r = c.playwright("t-livre");
    let esperado = c.sessao("t-livre");
    match &r {
        Resposta::SessaoPlaywright { caminho } => {
            assert_eq!(Path::new(caminho), esperado);
        }
        outra => panic!("{outra:?}"),
    }
    let cod = codificado(Papel::Agente, &r);
    for f in formas_da_senha() {
        assert!(!contem(&cod, &f), "a resposta tem a senha");
    }
    let m = fs::symlink_metadata(&esperado).unwrap();
    assert!(m.file_type().is_file());
    assert_eq!(m.permissions().mode() & 0o777, 0o600);
    let pasta = fs::metadata(c.rt.0.join("playwright")).unwrap();
    assert_eq!(pasta.permissions().mode() & 0o777, 0o700);
    let estado = fs::read(&esperado).unwrap();
    for f in formas_da_senha() {
        assert!(!contem(&estado, &f), "o storage state tem a senha");
    }
    let v: serde_json::Value = serde_json::from_slice(&estado).unwrap();
    assert_eq!(v["cookies"][0]["name"], "sessao");
    assert_eq!(v["cookies"][0]["sameSite"], "Lax");
    assert_eq!(v["cookies"][0]["httpOnly"], true);
    assert!(v["origins"].as_array().unwrap().is_empty());
    // O Chrome: argv, ambiente, fds.
    let inicio = &c.eventos("inicio")[0];
    let argv: Vec<String> = inicio["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap().to_owned())
        .collect();
    assert!(argv.contains(&"--remote-debugging-pipe".to_owned()));
    assert!(argv.contains(&"--headless".to_owned()));
    assert!(!argv.iter().any(|a| a.contains("remote-debugging-port")));
    assert!(
        argv.iter()
            .any(|a| a.starts_with(&format!("--user-data-dir={}/pw-", c.rt.0.display())))
    );
    let tudo = serde_json::to_vec(inicio).unwrap();
    for f in formas_da_senha() {
        assert!(!contem(&tudo, &f), "argv ou ambiente com a senha");
    }
    let amb: Vec<&str> = inicio["ambiente"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    // D60: só o montado (o `sh` do falso acrescenta PWD).
    for n in &amb {
        assert!(
            [
                "PATH",
                "HOME",
                "XDG_CONFIG_HOME",
                "XDG_CACHE_HOME",
                "XDG_DATA_HOME",
                "TMPDIR",
                "LANG",
                "DBUS_SESSION_BUS_ADDRESS",
                "PWD",
                // O `python3` acrescenta este ao próprio ambiente.
                "LC_CTYPE",
            ]
            .contains(n),
            "variável a mais no Chrome: {n}"
        );
    }
    assert!(
        inicio["home"]
            .as_str()
            .unwrap()
            .starts_with(&format!("{}/pw-", c.rt.0.display()))
    );
    for i in ["0", "1", "2"] {
        assert_eq!(inicio["alvos"][i], "/dev/null", "fd {i}");
    }
    for f in inicio["fds"].as_array().unwrap() {
        // 0–4 e o que o próprio Python abriu para listar (o maior).
        let f = f.as_u64().unwrap();
        assert!(f <= 5, "fd {f} a mais no Chrome: {inicio}");
    }
    // A senha chegou só ao preenchimento, com a origem da entrada.
    let pre = &c.eventos("preencher")[0];
    assert_eq!(pre["sha256"][1], sha256_hex(SENHA_LOGIN.as_bytes()));
    assert_eq!(pre["origem"], "https://clientex.invalid");
    // O POST com a senha foi pausado e seguiu (mesma origem).
    let decisoes = c.eventos("decisao");
    assert!(decisoes.iter().all(|d| d["seguiu"] == true), "{decisoes:?}");
    assert!(c.sem_perfil(), "o perfil temporário ficou");
    // D6: travar apaga a sessão.
    c.d.travar();
    assert!(!esperado.exists(), "a sessão sobreviveu à trava");
}

/// V6.15 (D9): o redirecionamento para o phishing é recusado no navegador antes de a
/// página carregar; a senha nunca chega ao Chrome e nada é gravado.
#[test]
fn v6_15_redirecionamento_para_outra_origem_recusado() {
    let c = cena("redir", "redir");
    let r = c.playwright("t-livre");
    assert_eq!(erro(&r), Some(ErroResposta::OrigemDiferente), "{r:?}");
    let d = c.eventos("decisao");
    assert_eq!(d.len(), 2, "{d:?}");
    assert_eq!(d[0]["seguiu"], true);
    assert_eq!(d[1]["url"], "http://localhost:8082/wp-login.php");
    assert_eq!(d[1]["seguiu"], false);
    assert!(c.eventos("preencher").is_empty(), "a senha foi ao Chrome");
    assert!(!c.sessao("t-livre").exists());
    assert!(c.sem_perfil());
}

/// D9: a página que diz ser de outra origem não recebe nada; o envio do formulário
/// para outra origem é barrado no navegador; e o código de preencher recusa um
/// formulário que envia para fora.
#[test]
fn d9_origem_da_pagina_e_do_envio() {
    let c = cena("mente", "mente");
    assert_eq!(
        erro(&c.playwright("t-livre")),
        Some(ErroResposta::OrigemDiferente)
    );
    assert!(c.eventos("preencher").is_empty(), "a senha foi ao Chrome");
    assert!(c.sem_perfil());

    let c = cena("posoutra", "pos_outra");
    assert_eq!(
        erro(&c.playwright("t-livre")),
        Some(ErroResposta::OrigemDiferente)
    );
    let d = c.eventos("decisao");
    assert_eq!(
        d.last().unwrap()["url"],
        "http://localhost:8082/wp-login.php"
    );
    assert_eq!(
        d.last().unwrap()["seguiu"],
        false,
        "o POST saiu para outra origem"
    );
    assert!(!c.sessao("t-livre").exists());

    let c = cena("formoutra", "form_outra");
    assert_eq!(
        erro(&c.playwright("t-livre")),
        Some(ErroResposta::OrigemDiferente)
    );
    assert!(!c.sessao("t-livre").exists());
    assert!(c.sem_perfil());
}

/// S1/V6.13: um site que guarda a senha (base64) em cookie ou `localStorage` não vira
/// storage state: nada é gravado.
#[test]
fn s1_sessao_com_a_senha_nao_e_gravada() {
    for modo in ["cookie_senha", "ls_senha"] {
        let c = cena(modo, modo);
        assert_eq!(
            erro(&c.playwright("t-livre")),
            Some(ErroResposta::SessaoComSegredo),
            "{modo}"
        );
        assert!(!c.sessao("t-livre").exists(), "{modo}");
        assert!(
            !c.rt.0.join("playwright").exists()
                || fs::read_dir(c.rt.0.join("playwright")).unwrap().count() == 0,
            "{modo}"
        );
        assert!(c.sem_perfil(), "{modo}");
    }
}

/// O navegador que cai ou manda lixo grande: erro, o perfil some e nada fica rodando.
#[test]
fn navegador_que_cai_ou_manda_lixo() {
    for modo in ["morre", "grande"] {
        let c = cena(modo, modo);
        assert_eq!(
            erro(&c.playwright("t-livre")),
            Some(ErroResposta::NavegadorFalhou),
            "{modo}"
        );
        assert!(c.sem_perfil(), "{modo}");
        assert!(!c.sessao("t-livre").exists(), "{modo}");
    }
}

/// D6: travar no meio da sessão (antes de gravar) não deixa storage state.
#[test]
fn d6_travar_durante_a_sessao_nada_gravado() {
    let c = cena("lento", "lento");
    let d = c.d.clone();
    let t = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(700));
        d.travar();
    });
    let r = c.playwright("t-livre");
    t.join().unwrap();
    assert_eq!(erro(&r), Some(ErroResposta::Travado), "{r:?}");
    assert!(!c.sessao("t-livre").exists());
    assert!(c.sem_perfil());
}

/// Bloqueada = inexistente (V3.17), com aprovação sem app e tipo errado: nenhuma abre o
/// navegador.
#[test]
fn recusas_sem_abrir_o_navegador() {
    let c = cena("recusas", "ok");
    for (entrada, esperado) in [
        ("t-bloq", ErroResposta::EntradaInexistente),
        ("nao-existe", ErroResposta::EntradaInexistente),
        ("s1", ErroResposta::TipoSemEntrega),
    ] {
        assert_eq!(erro(&c.playwright(entrada)), Some(esperado), "{entrada}");
    }
    assert!(
        erro(&c.playwright("t-aprov")).is_some(),
        "com aprovação sem app"
    );
    assert!(c.eventos("inicio").is_empty(), "o navegador abriu");
    // O papel app não pede `playwright`.
    let p = ped(Papel::Agente, "playwright", r#"{"entrada":"t-livre"}"#).unwrap();
    assert_eq!(
        erro(&c.d.atender_de(&SemCliente, Papel::App, p)),
        Some(ErroResposta::NaoAutorizado)
    );
    assert!(c.eventos("inicio").is_empty());
}

/// Partida (D6): storage states e perfis de um daemon morto somem.
#[test]
fn d6_partida_limpa_sessoes_e_perfis() {
    let rt = Rt::novo("partida");
    fs::create_dir_all(rt.0.join("playwright")).unwrap();
    fs::create_dir_all(rt.0.join("pw-abc/Default")).unwrap();
    fs::write(rt.0.join("playwright/x.json"), b"{}").unwrap();
    fs::write(rt.0.join("pw-abc/Default/Cookies"), b"x").unwrap();
    fs::create_dir_all(rt.0.join("tmp")).unwrap();
    assert_eq!(wardd::playwright::limpar(&rt.0).unwrap(), 2);
    assert!(!rt.0.join("playwright/x.json").exists());
    assert!(!rt.0.join("pw-abc").exists());
    assert!(
        rt.0.join("tmp").exists(),
        "apagou o que não é do playwright"
    );
    // Sem `<rt>`: nada a fazer.
    let nada = Rt::novo("nada");
    assert_eq!(wardd::playwright::limpar(&nada.0).unwrap(), 0);
}

// --- Chrome de verdade (`make test-playwright`) ---------------------------------------
//
// Os testes abaixo usam o Chrome instalado contra dois sites locais em 127.0.0.1 (porta
// aleatória): o "site" da entrada e um "phishing" que registra tudo o que recebe. Ficam
// fora do `cargo test` comum (`#[ignore]`: precisam do Chrome) e rodam no
// `make test-playwright` com `--ignored`.

mod site {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};

    /// Um pedido recebido: método, caminho e corpo.
    #[derive(Debug, Clone)]
    pub struct Pedido {
        pub metodo: String,
        pub caminho: String,
        pub corpo: Vec<u8>,
    }

    pub struct Site {
        pub porta: u16,
        pub pedidos: Arc<Mutex<Vec<Pedido>>>,
    }

    pub type Rota = dyn Fn(&str, &str, &[u8]) -> (u16, Vec<(String, String)>, String) + Send + Sync;

    /// Sobe um site; `rota(método, caminho, corpo) -> (status, cabeçalhos extras, html)`.
    pub fn subir(rota: Box<Rota>) -> Site {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = l.local_addr().unwrap().port();
        let pedidos = Arc::new(Mutex::new(Vec::new()));
        let p = pedidos.clone();
        let rota: Arc<Rota> = Arc::from(rota);
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                let p = p.clone();
                let rota = rota.clone();
                std::thread::spawn(move || atender(s, &p, &*rota));
            }
        });
        Site { porta, pedidos }
    }

    fn atender(s: TcpStream, p: &Mutex<Vec<Pedido>>, rota: &Rota) {
        let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(10)));
        let mut r = BufReader::new(s.try_clone().unwrap());
        loop {
            let mut linha = String::new();
            if r.read_line(&mut linha).unwrap_or(0) == 0 {
                return;
            }
            let partes: Vec<&str> = linha.split_whitespace().collect();
            if partes.len() < 2 {
                return;
            }
            let (metodo, caminho) = (partes[0].to_owned(), partes[1].to_owned());
            let mut tamanho = 0usize;
            loop {
                let mut h = String::new();
                if r.read_line(&mut h).unwrap_or(0) == 0 {
                    return;
                }
                let h = h.trim_end();
                if h.is_empty() {
                    break;
                }
                if let Some((k, v)) = h.split_once(':')
                    && k.eq_ignore_ascii_case("content-length")
                {
                    tamanho = v.trim().parse().unwrap_or(0);
                }
            }
            let mut corpo = vec![0u8; tamanho.min(1 << 20)];
            if r.read_exact(&mut corpo).is_err() {
                return;
            }
            p.lock().unwrap().push(Pedido {
                metodo: metodo.clone(),
                caminho: caminho.clone(),
                corpo: corpo.clone(),
            });
            let (status, extras, html) = rota(&metodo, &caminho, &corpo);
            let mut resp = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\n",
                html.len()
            );
            for (k, v) in extras {
                resp.push_str(&format!("{k}: {v}\r\n"));
            }
            resp.push_str("\r\n");
            resp.push_str(&html);
            let mut w = &s;
            if w.write_all(resp.as_bytes()).is_err() {
                return;
            }
        }
    }
}

/// O formulário de login (como o do WordPress): `POST` em `/entrar`, com `atributos`
/// a mais no `<form>`, `extra` dentro dele e um `script` na página.
fn formulario(atributos: &str, extra: &str, script: &str) -> String {
    format!(
        r#"<!doctype html><html><body><form id="f" method="post" action="/entrar" {atributos}>
<input type="text" name="log" id="user_login" autocomplete="username">
<input type="password" name="pwd" id="user_pass" autocomplete="current-password">
{extra}
<button type="submit" id="b">Acessar</button></form><script>{script}</script></body></html>"#
    )
}

/// Põe na página elementos com nome que trocam `document.querySelectorAll` e
/// `document.getElementsByTagName` (propriedades com nome do `document`).
fn com_clobber(html: &str) -> String {
    html.replacen(
        "<body>",
        r#"<body><img name="querySelectorAll" src="data:,"><img name="getElementsByTagName" src="data:,">"#,
        1,
    )
}

fn login_em(nome: &str, url: &str, senha: &str) -> String {
    format!(
        r#"{{"entrada":{{"nome":"{nome}","grupo":"clientex","ambiente":"dev","tipo":"login","campos":{{"url":"{url}","usuario":"admin","senha":"{}"}}}}}}"#,
        b64(senha.as_bytes())
    )
}

struct CenaReal {
    d: Arc<Daemon>,
    _dir: Dir,
    rt: Rt,
    site: site::Site,
    phish: site::Site,
}

/// Os casos do site e o que o daemon deve responder (`None`: sessão gravada).
const CASOS: [(&str, Option<ErroResposta>); 13] = [
    ("ok", None),
    ("dom", None),
    ("dom-recusa", Some(ErroResposta::LoginRecusado)),
    ("iframe", None),
    ("acao-fora", Some(ErroResposta::OrigemDiferente)),
    ("clobber", Some(ErroResposta::OrigemDiferente)),
    ("formaction", Some(ErroResposta::OrigemDiferente)),
    ("redir", Some(ErroResposta::OrigemDiferente)),
    ("get", Some(ErroResposta::LoginRecusado)),
    ("alvo", Some(ErroResposta::LoginRecusado)),
    ("dois", Some(ErroResposta::LoginRecusado)),
    ("recusa", Some(ErroResposta::LoginRecusado)),
    ("ls", Some(ErroResposta::SessaoComSegredo)),
];

fn pagina(caso: &str, fora: &str, pp: u16) -> (u16, Vec<(String, String)>, String) {
    let f = || formulario("", "", "");
    let html = match caso {
        "ok" => f(),
        "acao-fora" => f().replace(r#"action="/entrar""#, &format!(r#"action="{fora}""#)),
        // O destino de verdade é o phishing; o `<input name="action">` faz o
        // `form.action` ingênuo devolver o campo (um caminho relativo da mesma origem).
        "clobber" => formulario(
            "",
            r#"<input type="hidden" name="action" value="/entrar">"#,
            "",
        )
        .replace(r#"action="/entrar" "#, &format!(r#"action="{fora}" "#)),
        // DOM clobbering (Fase 44a): `<img name="querySelectorAll">` troca o
        // `document.querySelectorAll` da página (o Chrome de hoje não leva as
        // propriedades com nome do `document` ao mundo isolado; as do `<form>`, sim) e
        // campos com o nome de cada propriedade que o script lê trocam as do `<form>`.
        // O login certo tem de entrar mesmo assim.
        "dom" => com_clobber(&formulario(
            "",
            r#"<input type="hidden" name="elements" value="x"><input type="hidden" name="method" value="get"><input type="hidden" name="target" value="_blank"><input type="hidden" name="requestSubmit" value="x"><input type="hidden" name="querySelectorAll" value="x">"#,
            "",
        )),
        // O site recusa e devolve o login com o mesmo clobbering: a senha visível
        // tem de ser vista depois do envio (nada gravado).
        "dom-recusa" => formulario("", r#"<input type="hidden" name="recusa" value="2">"#, ""),
        "get" => f().replace(r#"method="post""#, r#"method="get""#),
        "formaction" => f().replace(
            r#"<button type="submit" id="b">"#,
            &format!(r#"<button type="submit" id="b" formaction="{fora}">"#),
        ),
        "alvo" => formulario(r#"target="_blank""#, "", ""),
        "dois" => formulario("", r#"<input type="password" name="pwd2">"#, ""),
        "recusa" => formulario("", r#"<input type="hidden" name="recusa" value="1">"#, ""),
        "ls" => formulario(
            "",
            "",
            "document.getElementById('f').addEventListener('submit', () => localStorage.setItem('p', btoa(document.getElementById('user_pass').value)));",
        ),
        "iframe" => formulario(
            "",
            &format!(r#"<iframe src="http://127.0.0.1:{pp}/frame"></iframe>"#),
            "",
        ),
        "redir" => {
            return (
                302,
                vec![("Location".to_owned(), fora.to_owned())],
                String::new(),
            );
        }
        _ => return (404, vec![], "<html>nada</html>".into()),
    };
    (200, vec![], html)
}

fn cena_real(rotulo: &str) -> CenaReal {
    let phish = site::subir(Box::new(|_, _, _| {
        (200, vec![], "<html><body>falso</body></html>".into())
    }));
    let pp = phish.porta;
    let site = site::subir(Box::new(move |metodo, caminho, corpo| {
        let fora = format!("http://127.0.0.1:{pp}/x");
        if metodo == "POST" && caminho == "/entrar" {
            if contem(corpo, b"recusa=1") {
                return (200, vec![], formulario("", "", ""));
            }
            if contem(corpo, b"recusa=2") {
                return (200, vec![], com_clobber(&formulario("", "", "")));
            }
            return (
                200,
                vec![(
                    "Set-Cookie".to_owned(),
                    "sessao=token-falso-da-sessao; Path=/; HttpOnly".to_owned(),
                )],
                "<html><body><h1>Painel</h1></body></html>".to_owned(),
            );
        }
        pagina(caminho.trim_start_matches('/'), &fora, pp)
    }));
    let dir = Dir::novo(rotulo);
    let (d, _) = cofre_de_teste(&dir);
    let rt = Rt::novo(rotulo);
    let d = d.com_temporarios(rt.0.clone());
    for (caso, _) in CASOS {
        let url = format!("http://127.0.0.1:{}/{caso}", site.porta);
        let nome = format!("w-{caso}");
        let r = pedir(
            &d,
            Papel::App,
            "salvar_entrada",
            &login_em(&nome, &url, SENHA_LOGIN),
        );
        assert!(ok(&r), "{r:?}");
        assert!(ok(&mudar_nivel(&d, Papel::App, &nome, "livre", PIN)));
    }
    CenaReal {
        d: Arc::new(d),
        _dir: dir,
        rt,
        site,
        phish,
    }
}

impl CenaReal {
    fn playwright(&self, entrada: &str) -> Resposta {
        let p = ped(
            Papel::Agente,
            "playwright",
            &format!(r#"{{"entrada":"{entrada}"}}"#),
        )
        .unwrap();
        self.d.atender_de(&SemCliente, Papel::Agente, p)
    }

    /// O phishing não recebeu `POST` nem a senha em lugar nenhum.
    fn phish_limpo(&self) -> bool {
        self.phish.pedidos.lock().unwrap().iter().all(|p| {
            p.metodo != "POST"
                && formas_da_senha()
                    .iter()
                    .all(|f| !contem(&p.corpo, f) && !contem(p.caminho.as_bytes(), f))
        })
    }

    /// Nenhum pedido ao site levou a senha na URL.
    fn senha_fora_da_url(&self) -> bool {
        self.site.pedidos.lock().unwrap().iter().all(|p| {
            formas_da_senha()
                .iter()
                .all(|f| !contem(p.caminho.as_bytes(), f))
        })
    }

    fn sem_perfil(&self) -> bool {
        fs::read_dir(&self.rt.0).map_or(true, |it| {
            !it.flatten()
                .any(|e| e.file_name().to_string_lossy().starts_with("pw-"))
        })
    }
}

/// Chrome de verdade (D6, D9, S1): o login certo grava a sessão com o cookie do site e
/// sem a senha, e o `POST` com a senha só vai ao site da entrada. Ataques: formulário
/// que envia para fora (pelo `action`, pelo `formaction` do botão e com o `action`
/// "mascarado" por um campo chamado `action`), redirecionamento para fora, formulário
/// `GET` (a senha iria na URL), envio para outra janela, dois campos de senha, login
/// recusado pelo site e site que guarda a senha no `localStorage`: cada um recusado,
/// nada gravado, o phishing sem `POST` e a senha fora de toda URL. O iframe de outra
/// origem não recebe nada. DOM clobbering do `document` (Fase 44a): o login certo entra
/// e a página de login devolvida depois do envio continua vista (recusado).
#[test]
#[ignore = "precisa do Chrome instalado: make test-playwright"]
fn chrome_real_login_e_ataques() {
    let c = cena_real("real");
    for (caso, esperado) in CASOS {
        let entrada = format!("w-{caso}");
        // Fase 44c: os sites são `http://127.0.0.1:<porta>`, então cada uso passa pela
        // confirmação da origem (a janela falsa confirma).
        let (r, vistos) = com_janela(&c.d, "confirmar", None, || c.playwright(&entrada));
        assert!(
            vistos.len() == 1
                && vistos[0].confirmacao
                && vistos[0].origem.starts_with("http://127.0.0.1:"),
            "{caso}: {vistos:?}"
        );
        let caminho = c.rt.0.join(format!("playwright/{entrada}.json"));
        match &esperado {
            None => {
                assert!(
                    matches!(r, Resposta::SessaoPlaywright { .. }),
                    "{caso}: {r:?}"
                );
                let estado = fs::read(&caminho).unwrap();
                assert!(contem(&estado, b"token-falso-da-sessao"), "{caso}");
                for f in formas_da_senha() {
                    assert!(!contem(&estado, &f), "{caso}");
                }
            }
            Some(e) => {
                assert_eq!(erro(&r).as_ref(), Some(e), "{caso}: {r:?}");
                assert!(!caminho.exists(), "{caso}");
            }
        }
        assert!(c.phish_limpo(), "{caso}: o phishing recebeu algo");
        assert!(c.senha_fora_da_url(), "{caso}: a senha foi na URL");
        assert!(c.sem_perfil(), "{caso}: o perfil ficou");
    }
    // Os POST com a senha foram só ao site: um por caso que chega a enviar.
    let posts = c
        .site
        .pedidos
        .lock()
        .unwrap()
        .iter()
        .filter(|p| p.metodo == "POST")
        .count();
    assert_eq!(posts, 6, "ok, dom, dom-recusa, iframe, recusa e ls enviam");
    // O iframe carregou (o caso testou algo).
    assert!(
        c.phish
            .pedidos
            .lock()
            .unwrap()
            .iter()
            .any(|p| p.caminho == "/frame")
    );
}

/// Fase 44c (D9), ataque da auditoria do M6: outro processo do uid escuta na origem
/// `http://localhost:<porta>` (em `[::1]`, ou na porta de um serviço parado). Numa
/// entrada `http` **livre**, o `ward playwright` só abre o Chrome depois da confirmação
/// da origem na janela: sem app, "aprovação indisponível"; negado, nada; o Chrome nem
/// começa. O pedido mostra a origem e não pede PIN. Confirmado, o Chrome abre.
#[test]
fn ataque_playwright_http_livre_so_com_confirmacao() {
    let c = cena("http", "ok");
    let q = format!(
        r#"{{"entrada":{{"nome":"h-livre","grupo":"acme","ambiente":"prod","tipo":"login","campos":{{"url":"http://localhost:8081/wp-login.php","usuario":"wp_editor","senha":"{}"}}}}}}"#,
        b64(SENHA_LOGIN.as_bytes())
    );
    assert!(ok(&pedir(&c.d, Papel::App, "salvar_entrada", &q)));
    assert!(ok(&mudar_nivel(&c.d, Papel::App, "h-livre", "livre", PIN)));
    let r = c.playwright("h-livre");
    assert_eq!(erro(&r), Some(ErroResposta::AprovacaoIndisponivel));
    assert!(
        c.eventos("inicio").is_empty(),
        "o Chrome abriu sem a janela"
    );
    let (r, vistos) = com_janela(&c.d, "negar", None, || c.playwright("h-livre"));
    assert_eq!(erro(&r), Some(ErroResposta::Negado));
    assert!(
        c.eventos("inicio").is_empty(),
        "o Chrome abriu com a janela negada"
    );
    assert!(!c.sessao("h-livre").exists());
    let p = &vistos[0];
    assert_eq!(p.operacao, ward_core::protocolo::Operacao::Playwright);
    assert_eq!(p.origem, "http://localhost:8081");
    assert!(p.confirmacao);
    let cod = codificado(Papel::App, &Resposta::PedidoDeAprovacao(p.clone()));
    for f in formas_da_senha() {
        assert!(!contem(&cod, &f), "o pedido da janela tem a senha");
    }
    let (_, vistos) = com_janela(&c.d, "confirmar", None, || c.playwright("h-livre"));
    assert_eq!(vistos.len(), 1);
    assert!(
        !c.eventos("inicio").is_empty(),
        "confirmado, o Chrome não abriu"
    );
    assert!(c.sem_perfil());
}

/// `https` livre segue sem janela nenhuma.
/// Fase 44e (Médio da 2ª auditoria do M6): `https` num host de loopback pede a
/// confirmação como o `http` (uma CA local faz o `[::1]` parecer o serviço). Sem app,
/// negado: o Chrome nem abre.
#[test]
fn ataque_playwright_https_loopback_livre_so_com_confirmacao() {
    let c = cena("https-loopback", "ok");
    for (i, (url, origem)) in [
        (
            "https://localhost:8443/wp-login.php",
            "https://localhost:8443",
        ),
        ("https://[::1]:8443/wp-login.php", "https://[::1]:8443"),
        ("https://127.0.0.1/wp-login.php", "https://127.0.0.1"),
    ]
    .into_iter()
    .enumerate()
    {
        let nome = format!("l-livre-{i}");
        let q = format!(
            r#"{{"entrada":{{"nome":"{nome}","grupo":"acme","ambiente":"prod","tipo":"login","campos":{{"url":"{url}","usuario":"wp_editor","senha":"{}"}}}}}}"#,
            b64(SENHA_LOGIN.as_bytes())
        );
        assert!(ok(&pedir(&c.d, Papel::App, "salvar_entrada", &q)), "{url}");
        assert!(
            ok(&mudar_nivel(&c.d, Papel::App, &nome, "livre", PIN)),
            "{url}"
        );
        let r = c.playwright(&nome);
        assert_eq!(erro(&r), Some(ErroResposta::AprovacaoIndisponivel), "{url}");
        let (r, vistos) = com_janela(&c.d, "negar", None, || c.playwright(&nome));
        assert_eq!(erro(&r), Some(ErroResposta::Negado), "{url}");
        assert!(c.eventos("inicio").is_empty(), "o Chrome abriu: {url}");
        assert!(!c.sessao(&nome).exists(), "{url}");
        assert_eq!(vistos.len(), 1, "{url}");
        assert_eq!(vistos[0].origem, origem, "{url}");
        assert!(vistos[0].confirmacao, "{url}");
    }
}

#[test]
fn playwright_https_livre_sem_janela() {
    let c = cena("https-livre", "ok");
    let (r, vistos) = com_janela(&c.d, "negar", None, || c.playwright("t-livre"));
    assert!(matches!(r, Resposta::SessaoPlaywright { .. }), "{r:?}");
    assert!(vistos.is_empty());
}
