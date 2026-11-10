//! Ataques à ponte de teste (D7, S11): cada um tem de falhar.
#![cfg(feature = "test-mode")]

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ward_app_core::ErroApp;
use ward_app_core::area_transferencia::{AreaTransferencia, Copiador};
use ward_app_core::daemon_falso::{DaemonFalso, SEGREDOS};
use ward_app_core::servico::Servico;
use ward_test_bridge::http::{MAX_CABECALHO, MAX_CORPO, Pedido, Ponte, ler_pedido};
use zeroize::Zeroizing;

/// A senha de `acme-pg-dev` no cofre de teste (D15), a que o "copiar" pede.
const SEGREDO: &[u8] = b"falso-PG:s3nha/01@#";

#[derive(Default, Clone)]
struct Area(Arc<Mutex<Option<Vec<u8>>>>);
impl AreaTransferencia for Area {
    fn escrever(&self, v: &[u8]) -> Result<(), ErroApp> {
        *self.0.lock().unwrap() = Some(v.to_vec());
        Ok(())
    }
    fn ler(&self) -> Result<Option<Zeroizing<Vec<u8>>>, ErroApp> {
        Ok(self.0.lock().unwrap().clone().map(Zeroizing::new))
    }
    fn limpar(&self) -> Result<(), ErroApp> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

fn dist(nome: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!("ward-ponte-{}-{nome}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(base.join("dist/assets")).unwrap();
    fs::write(
        base.join("dist/index.html"),
        b"<!doctype html><title>Ward</title>",
    )
    .unwrap();
    fs::write(base.join("dist/assets/index-abc.js"), b"console.log(1)").unwrap();
    fs::write(base.join("fora.txt"), b"arquivo fora do dist").unwrap();
    std::os::unix::fs::symlink(base.join("fora.txt"), base.join("dist/assets/link.js")).unwrap();
    base
}

fn ponte(
    nome: &str,
) -> (
    Arc<Ponte<DaemonFalso, Area>>,
    std::net::TcpListener,
    Area,
    PathBuf,
) {
    let base = dist(nome);
    let area = Area::default();
    let servico = Servico::novo(DaemonFalso::cheio(), Copiador::novo(area.clone()));
    let (p, l) = Ponte::abrir(servico, &base.join("dist")).unwrap();
    (p, l, area, base)
}

fn post(
    p: &Ponte<DaemonFalso, Area>,
    comando: &str,
    corpo: &str,
    mudar: &[(&str, Option<&str>)],
) -> Pedido {
    let host = p.origem().trim_start_matches("http://").to_owned();
    let auth = format!("Bearer {}", p.token_para_teste());
    let mut cab: Vec<(String, String)> = vec![
        ("Host".into(), host),
        ("Origin".into(), p.origem().into()),
        ("Authorization".into(), auth),
        ("Content-Type".into(), "application/json".into()),
    ];
    for (n, v) in mudar {
        cab.retain(|(x, _)| !x.eq_ignore_ascii_case(n));
        if let Some(v) = v {
            cab.push(((*n).into(), (*v).into()));
        }
    }
    let refs: Vec<(&str, &str)> = cab.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    Pedido::novo(
        "POST",
        &format!("/ponte/{comando}"),
        &refs,
        corpo.as_bytes(),
    )
}

#[test]
fn copiar_passa_e_a_resposta_nao_tem_o_valor() {
    let (p, _l, area, base) = ponte("copiar");
    let r = p.responder(&post(
        &p,
        "copiar_campo",
        r#"{"entrada":"acme-pg-dev","campo":"senha"}"#,
        &[],
    ));
    assert_eq!(r.status, 200);
    assert_eq!(
        r.corpo_json().unwrap(),
        serde_json::json!({ "ok": true, "valor": null })
    );
    assert!(!r.corpo.windows(SEGREDO.len()).any(|j| j == SEGREDO));
    assert_eq!(area.0.lock().unwrap().as_deref(), Some(SEGREDO));
    let r = p.responder(&post(&p, "estado_do_cofre", "{}", &[]));
    assert_eq!(r.corpo_json().unwrap()["valor"]["estado"], "destravado");
    fs::remove_dir_all(base).unwrap();
}

/// Algum segredo do cofre de teste nos bytes da resposta (inteiro, escapado em JSON ou
/// por linha)?
fn tem_segredo(corpo: &[u8]) -> bool {
    let texto = String::from_utf8_lossy(corpo);
    SEGREDOS.iter().any(|s| {
        let json = serde_json::to_string(s).unwrap();
        texto.contains(s)
            || texto.contains(&json[1..json.len() - 1])
            || s.lines().any(|l| l.len() >= 6 && texto.contains(l))
    })
}

#[test]
fn ataque_respostas_da_tela_credenciais_sem_segredo() {
    // V5.3 pela ponte: a lista e o detalhe de todas as entradas do cofre de teste, como
    // chegam à interface, não têm nenhum segredo.
    let (p, _l, _a, base) = ponte("credenciais");
    let r = p.responder(&post(&p, "listar_credenciais", "{}", &[]));
    assert_eq!(r.status, 200);
    assert!(!tem_segredo(&r.corpo));
    let lista = r.corpo_json().unwrap()["valor"].clone();
    let nomes: Vec<String> = lista
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["nome"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        nomes.len(),
        serde_json::from_str::<serde_json::Value>(include_str!(
            "../../../dev/fixtures/entries.json"
        ))
        .unwrap()["entradas"]
            .as_array()
            .unwrap()
            .len()
    );
    for nome in &nomes {
        let corpo = serde_json::json!({ "entrada": nome }).to_string();
        let r = p.responder(&post(&p, "detalhe_credencial", &corpo, &[]));
        assert_eq!(r.corpo_json().unwrap()["ok"], true, "{nome}");
        assert!(!tem_segredo(&r.corpo), "segredo no detalhe de {nome}");
    }
    // Nome fora do formato, de outro tipo ou ausente: erro sem eco do pedido.
    for corpo in [
        r#"{"entrada":"acme-pg-dev\n"}"#,
        r#"{"entrada":"<img src=x onerror=alert(1)>"}"#,
        r#"{"entrada":1}"#,
        r#"{}"#,
    ] {
        let r = p.responder(&post(&p, "detalhe_credencial", corpo, &[]));
        let v = r.corpo_json().unwrap();
        assert_eq!(v["ok"], false, "{corpo}");
        assert!(!String::from_utf8_lossy(&r.corpo).contains("img"));
    }
    fs::remove_dir_all(base).unwrap();
}

/// Fase 33 pela ponte (V5.3, V5.10, V5.11, V5.13, V5.14): salvar, TOTP e "usar esta
/// senha" não devolvem segredo; só o `revelar_campo` traz o valor; formulário forjado
/// (campo desconhecido, nível, "manter" numa entrada nova, tipo de outro jeito) é
/// recusado sem eco.
#[test]
fn ataque_formulario_pela_ponte() {
    let (p, _l, _a, base) = ponte("formulario");
    let novo = serde_json::json!({ "formulario": {
        "tipo": "api", "nome": "beta-gitlab-api", "grupo": "beta", "ambiente": "dev",
        "projeto": "", "tags": [], "campos": {"url": "https://gitlab.exemplo.com", "variavel": "GITLAB_TOKEN"},
        "segredos": {"token": {"novo": "falso-token-gitlab-99"}}, "extras": []
    }});
    let r = p.responder(&post(&p, "salvar_credencial", &novo.to_string(), &[]));
    let v = r.corpo_json().unwrap();
    assert_eq!(v["valor"]["salvo"], "beta-gitlab-api", "{v}");
    assert!(!String::from_utf8_lossy(&r.corpo).contains("falso-token-gitlab"));
    let r = p.responder(&post(
        &p,
        "revelar_campo",
        r#"{"entrada":"beta-gitlab-api","campo":"token"}"#,
        &[],
    ));
    assert_eq!(
        r.corpo_json().unwrap()["valor"]["valor"],
        "falso-token-gitlab-99"
    );
    let r = p.responder(&post(
        &p,
        "codigo_totp",
        r#"{"entrada":"acme-wp-dev"}"#,
        &[],
    ));
    let v = r.corpo_json().unwrap();
    assert_eq!(v["valor"]["codigo"].as_str().unwrap().len(), 6, "{v}");
    assert!(!tem_segredo(&r.corpo));
    let gerar =
        r#"{"tamanho":20,"minusculas":true,"maiusculas":true,"digitos":true,"simbolos":true}"#;
    p.responder(&post(&p, "gerar_senha", gerar, &[]));
    let r = p.responder(&post(&p, "usar_senha_gerada", r#"{"campo":"senha"}"#, &[]));
    assert_eq!(
        r.corpo_json().unwrap()["valor"],
        serde_json::json!({ "tamanho": 20 })
    );
    for corpo in [
        r#"{"formulario":{"tipo":"login","nome":"x","grupo":"g","ambiente":"dev","projeto":"","tags":[],"campos":{},"segredos":{},"extras":[],"nivel":"livre"}}"#,
        r#"{"formulario":{"tipo":"login","nome":"acme-x","grupo":"g","ambiente":"dev","projeto":"","tags":[],"campos":{},"segredos":{"senha":"manter"},"extras":[]}}"#,
        r#"{"anterior":5,"formulario":{"tipo":"login","nome":"x","grupo":"g","ambiente":"dev","projeto":"","tags":[],"campos":{},"segredos":{},"extras":[]}}"#,
        r#"{"formulario":"<img src=x>"}"#,
        r#"{}"#,
    ] {
        let r = p.responder(&post(&p, "salvar_credencial", corpo, &[]));
        let v = r.corpo_json().unwrap();
        assert_eq!(v["ok"], false, "{corpo}");
        assert!(!String::from_utf8_lossy(&r.corpo).contains("img"));
    }
    fs::remove_dir_all(base).unwrap();
}

/// V5.15 pela ponte (S9): `mudar_nivel` com o PIN certo muda e responde só o nível;
/// PIN errado responde "pin-incorreto" e não muda; PIN fora do formato, nível
/// inventado e campo faltando são recusados sem contar erro; o 5º erro de PIN seguido
/// trava, e daí tudo responde "travado". Nenhuma resposta traz o PIN.
#[test]
fn ataque_mudar_nivel_pela_ponte() {
    let (p, _l, _a, base) = ponte("nivel");
    let nivel = |p: &Ponte<DaemonFalso, Area>| {
        let r = p.responder(&post(
            p,
            "detalhe_credencial",
            r#"{"entrada":"acme-pg-prod"}"#,
            &[],
        ));
        r.corpo_json().unwrap()["valor"]["nivel"].clone()
    };
    let mudar = |p: &Ponte<DaemonFalso, Area>, corpo: &str| {
        let r = p.responder(&post(p, "mudar_nivel", corpo, &[]));
        assert!(
            !String::from_utf8_lossy(&r.corpo).contains("135790"),
            "PIN na resposta"
        );
        r.corpo_json().unwrap()
    };
    let v = mudar(
        &p,
        r#"{"entrada":"acme-pg-prod","nivel":"livre","pin":"135790"}"#,
    );
    assert_eq!(
        v,
        serde_json::json!({"ok": true, "valor": {"entrada": "acme-pg-prod", "nivel": "livre"}})
    );
    assert_eq!(nivel(&p), "livre");
    // Recusados antes do daemon: não contam erro de PIN (seriam 6 × 6 erros).
    for _ in 0..6 {
        for corpo in [
            r#"{"entrada":"acme-pg-prod","nivel":"bloqueado","pin":"12"}"#,
            r#"{"entrada":"acme-pg-prod","nivel":"bloqueado","pin":135790}"#,
            r#"{"entrada":"acme-pg-prod","nivel":"bloqueado","pin":"13579a"}"#,
            r#"{"entrada":"acme-pg-prod","nivel":"admin","pin":"000000"}"#,
            r#"{"entrada":"acme-pg-prod","nivel":"bloqueado"}"#,
            r#"{"entrada":"acme\npg","nivel":"bloqueado","pin":"000000"}"#,
        ] {
            let v = mudar(&p, corpo);
            assert_eq!(v["ok"], false, "{corpo}");
            assert_eq!(v["codigo"], "pedido-invalido", "{corpo}: {v}");
        }
    }
    assert_eq!(nivel(&p), "livre");
    for k in 1..=5 {
        let v = mudar(
            &p,
            r#"{"entrada":"acme-pg-prod","nivel":"bloqueado","pin":"000000"}"#,
        );
        assert_eq!(v["codigo"], "pin-incorreto", "erro {k}: {v}");
        if k < 5 {
            assert_eq!(nivel(&p), "livre");
        }
    }
    let r = p.responder(&post(&p, "estado_do_cofre", "{}", &[]));
    assert_eq!(r.corpo_json().unwrap()["valor"]["estado"], "travado");
    let v = mudar(
        &p,
        r#"{"entrada":"acme-pg-prod","nivel":"bloqueado","pin":"135790"}"#,
    );
    assert_eq!(v["codigo"], "travado", "{v}");
    fs::remove_dir_all(base).unwrap();
}

/// V5.21 pela ponte: "gerar" devolve só tamanho, força e símbolos de D35; "copiar"
/// devolve `null` e a senha vai para a área; o valor só sai no "revelar"; opções fora do
/// formato (tamanho fora de 8–64, tipo errado, campo faltando) são recusadas.
#[test]
fn ataque_gerador_avulso_pela_ponte() {
    let (p, _l, area, base) = ponte("gerador");
    let opcoes =
        r#"{"tamanho":24,"minusculas":true,"maiusculas":true,"digitos":true,"simbolos":true}"#;
    let r = p.responder(&post(&p, "gerar_senha", opcoes, &[]));
    assert_eq!(
        r.corpo_json().unwrap(),
        serde_json::json!({ "ok": true, "valor": {
            "tamanho": 24, "bits": 150, "forca": "muito-forte",
            "simbolos": "!#$%&()*+,-./:;<=>?@[]^_{|}~"
        }})
    );
    let r = p.responder(&post(&p, "copiar_senha_gerada", "{}", &[]));
    assert_eq!(
        r.corpo_json().unwrap(),
        serde_json::json!({ "ok": true, "valor": null })
    );
    let copiado = area.0.lock().unwrap().clone().unwrap();
    assert_eq!(copiado.len(), 24);
    let r = p.responder(&post(&p, "revelar_senha_gerada", "{}", &[]));
    assert_eq!(
        r.corpo_json().unwrap()["valor"]["valor"]
            .as_str()
            .unwrap()
            .as_bytes(),
        &copiado[..]
    );
    let r = p.responder(&post(&p, "descartar_senha_gerada", "{}", &[]));
    assert_eq!(r.corpo_json().unwrap()["ok"], true);
    let r = p.responder(&post(&p, "revelar_senha_gerada", "{}", &[]));
    assert_eq!(r.corpo_json().unwrap()["codigo"], "sem-senha-gerada");
    for corpo in [
        r#"{"tamanho":7,"minusculas":true,"maiusculas":true,"digitos":true,"simbolos":true}"#,
        r#"{"tamanho":65,"minusculas":true,"maiusculas":true,"digitos":true,"simbolos":true}"#,
        r#"{"tamanho":128,"minusculas":true,"maiusculas":true,"digitos":true,"simbolos":true}"#,
        r#"{"tamanho":-1,"minusculas":true,"maiusculas":true,"digitos":true,"simbolos":true}"#,
        r#"{"tamanho":24.5,"minusculas":true,"maiusculas":true,"digitos":true,"simbolos":true}"#,
        r#"{"tamanho":"24","minusculas":true,"maiusculas":true,"digitos":true,"simbolos":true}"#,
        r#"{"tamanho":24,"minusculas":"sim","maiusculas":true,"digitos":true,"simbolos":true}"#,
        r#"{"tamanho":24,"maiusculas":true,"digitos":true,"simbolos":true}"#,
        r#"{"tamanho":24,"minusculas":false,"maiusculas":false,"digitos":false,"simbolos":false}"#,
    ] {
        let r = p.responder(&post(&p, "gerar_senha", corpo, &[]));
        assert_eq!(
            r.corpo_json().unwrap()["codigo"],
            "pedido-invalido",
            "{corpo}"
        );
    }
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn ataque_sem_token_token_errado_ou_repetido() {
    let (p, _l, area, base) = ponte("token");
    let corpo = r#"{"entrada":"acme-pg-dev","campo":"senha"}"#;
    let errado = format!("Bearer {}", "0".repeat(64));
    let prefixo = format!("Bearer {}", &p.token_para_teste()[..63]);
    for mudar in [
        vec![("Authorization", None)],
        vec![("Authorization", Some(errado.as_str()))],
        vec![("Authorization", Some(prefixo.as_str()))],
        vec![("Authorization", Some(p.token_para_teste()))],
    ] {
        assert_eq!(
            p.responder(&post(&p, "copiar_campo", corpo, &mudar)).status,
            401,
            "{mudar:?}"
        );
    }
    assert_eq!(*area.0.lock().unwrap(), None, "nada copiado sem token");
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn ataque_host_ou_origem_de_outro_site() {
    let (p, _l, _a, base) = ponte("origem");
    for (mudar, status) in [
        (vec![("Host", Some("evil.example:80"))], 421),
        (vec![("Host", Some("localhost"))], 421),
        (vec![("Host", None)], 421),
        (vec![("Origin", Some("https://example.com"))], 403),
        (vec![("Origin", Some("null"))], 403),
        (vec![("Origin", None)], 403),
        (vec![("Content-Type", Some("text/plain"))], 415),
    ] {
        assert_eq!(
            p.responder(&post(&p, "estado_do_cofre", "{}", &mudar))
                .status,
            status,
            "{mudar:?}"
        );
    }
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn ataque_comando_de_plugin_ou_inventado() {
    let (p, _l, _a, base) = ponte("comando");
    for c in [
        "plugin:fs|read_text_file",
        "plugin:shell|execute",
        "plugin:http|fetch",
        "inventado",
        "revelar",
    ] {
        let r = p.responder(&post(&p, c, "{}", &[]));
        assert_eq!(r.status, 404, "{c}");
        assert_eq!(r.corpo_json().unwrap()["ok"], false);
    }
    for corpo in [
        "",
        "[]",
        "nao json",
        r#"{"entrada":1,"campo":"senha"}"#,
        r#"{"entrada":"acme-pg-dev\n","campo":"senha"}"#,
    ] {
        let r = p.responder(&post(&p, "copiar_campo", corpo, &[]));
        assert_eq!(
            r.corpo_json().map(|v| v["ok"].clone()),
            Some(serde_json::json!(false)),
            "{corpo}"
        );
    }
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn ataque_arquivo_fora_do_dist() {
    let (p, _l, _a, base) = ponte("estatico");
    let host = p.origem().trim_start_matches("http://").to_owned();
    let get = |c: &str| p.responder(&Pedido::novo("GET", c, &[("Host", &host)], b""));
    assert_eq!(get("/").status, 200);
    assert_eq!(get("/assets/index-abc.js").status, 200);
    for c in [
        "/assets/link.js",
        "/fora.txt",
        "/assets/.oculto",
        "/assets/",
        "/src/main.js",
        "/index.html/",
    ] {
        assert_eq!(get(c).status, 404, "{c}");
    }
    assert_eq!(
        p.responder(&Pedido::novo("DELETE", "/", &[("Host", &host)], b""))
            .status,
        405
    );
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn ataque_pedido_fora_dos_limites() {
    for (bruto, status) in [
        (b"GET /../etc/passwd HTTP/1.1\r\n\r\n".to_vec(), 400),
        (b"GET /%2e%2e/x HTTP/1.1\r\n\r\n".to_vec(), 400),
        (b"GET / HTTP/1.0\r\n\r\n".to_vec(), 400),
        (
            b"POST /ponte/x HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec(),
            501,
        ),
        (
            b"POST /ponte/x HTTP/1.1\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\nab".to_vec(),
            400,
        ),
        (
            b"POST /ponte/x HTTP/1.1\r\nContent-Length: -1\r\n\r\n".to_vec(),
            400,
        ),
        (
            format!(
                "POST /ponte/x HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
                MAX_CORPO + 1
            )
            .into_bytes(),
            413,
        ),
        (
            format!("GET / HTTP/1.1\r\nX: {}\r\n\r\n", "a".repeat(MAX_CABECALHO)).into_bytes(),
            431,
        ),
        (
            b"POST /ponte/x HTTP/1.1\r\nContent-Length: 10\r\n\r\nabc".to_vec(),
            400,
        ),
    ] {
        assert_eq!(
            ler_pedido(&bruto[..]).err(),
            Some(status),
            "{}",
            String::from_utf8_lossy(&bruto[..40.min(bruto.len())])
        );
    }
    let ok = ler_pedido(&b"POST /ponte/x HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}"[..])
        .ok()
        .unwrap();
    assert_eq!(ok.corpo, b"{}");
}

/// A CSP da ponte é a do app (`tauri.conf.json`), trocando só a IPC por `'self'`.
#[test]
fn csp_da_ponte_e_a_do_app() {
    let conf: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../app/src-tauri/tauri.conf.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let mut esperado: Vec<String> = conf["app"]["security"]["csp"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(d, v)| {
            if d == "connect-src" {
                "connect-src 'self'".into()
            } else {
                format!("{d} {}", v.as_str().unwrap())
            }
        })
        .collect();
    esperado.push("frame-ancestors 'none'".into());
    esperado.sort();
    let mut da_ponte: Vec<String> = ward_test_bridge::http::CSP
        .split("; ")
        .map(str::to_owned)
        .collect();
    da_ponte.sort();
    assert_eq!(da_ponte, esperado);
}

#[test]
fn tcp_so_em_127_0_0_1_com_cabecalhos_de_seguranca() {
    let (p, l, _a, base) = ponte("tcp");
    assert!(l.local_addr().unwrap().ip().is_loopback());
    assert!(p.url_de_abertura().starts_with("http://127.0.0.1:"));
    assert!(p.url_de_abertura().contains("/#token="));
    let endereco = l.local_addr().unwrap();
    let servidor = Arc::clone(&p);
    std::thread::spawn(move || servidor.servir(l));
    let mut s = TcpStream::connect(endereco).unwrap();
    write!(
        s,
        "GET / HTTP/1.1\r\nHost: {}\r\n\r\n",
        p.origem().trim_start_matches("http://")
    )
    .unwrap();
    let mut resposta = String::new();
    s.read_to_string(&mut resposta).unwrap();
    assert!(resposta.starts_with("HTTP/1.1 200 OK"));
    for h in [
        "Content-Security-Policy: default-src 'none'",
        "X-Content-Type-Options: nosniff",
        "Cache-Control: no-store",
    ] {
        assert!(resposta.contains(h), "{h}");
    }
    assert!(!resposta.contains("unsafe-inline") && !resposta.contains("unsafe-eval"));
    fs::remove_dir_all(base).unwrap();
}

/// Fase 34 (V5.12, V5.24, S11) pela ponte: `escolher_anexo` devolve só id, nome e
/// tamanho (o conteúdo nunca), o anexo vai no salvar pelo id, `remover_anexo` com id
/// forjado é recusado, e `conectar` devolve `null` (o token não vem) e só vale para
/// `servidor`. A pasta do seletor tem mais de um arquivo: nada é escolhido.
#[test]
fn ataque_anexos_e_conectar_pela_ponte() {
    use ward_test_bridge::simulados::{SeletorDePasta, TerminalDeTeste};
    let base = dist("anexos34");
    let pasta = base.join("anexos");
    fs::create_dir_all(&pasta).unwrap();
    fs::write(pasta.join("deploy.pem"), "falso-anexo-ponte-34\n").unwrap();
    let servico = Servico::novo(DaemonFalso::cheio(), Copiador::novo(Area::default()))
        .com_seletor(SeletorDePasta(pasta.clone()))
        .com_terminal(TerminalDeTeste);
    let (p, _l) = Ponte::abrir(servico, &base.join("dist")).unwrap();
    let r = p.responder(&post(&p, "escolher_anexo", "{}", &[]));
    let v = r.corpo_json().unwrap();
    assert_eq!(
        v["valor"],
        serde_json::json!({"resultado": "anexado", "id": 0, "nome": "deploy.pem", "tamanho": 21}),
        "{v}"
    );
    assert!(!String::from_utf8_lossy(&r.corpo).contains("falso-anexo"));
    let novo = serde_json::json!({ "formulario": {
        "tipo": "generico", "nome": "gama-anexo", "grupo": "gama", "ambiente": "dev",
        "projeto": "", "tags": [], "campos": {}, "segredos": {},
        "extras": [{"nome": "REGIAO", "segredo": false, "valor": {"novo": "r-01"}}],
        "anexos": [0]
    }});
    let r = p.responder(&post(&p, "salvar_credencial", &novo.to_string(), &[]));
    assert_eq!(r.corpo_json().unwrap()["valor"]["salvo"], "gama-anexo");
    let r = p.responder(&post(
        &p,
        "detalhe_credencial",
        r#"{"entrada":"gama-anexo"}"#,
        &[],
    ));
    assert!(!String::from_utf8_lossy(&r.corpo).contains("falso-anexo"));
    assert_eq!(r.corpo_json().unwrap()["valor"]["anexos"][0]["tamanho"], 21);
    for corpo in [r#"{"id":"0"}"#, r#"{"id":-1}"#, r#"{"id":99}"#, r#"{}"#] {
        let r = p.responder(&post(&p, "remover_anexo", corpo, &[]));
        assert_eq!(r.corpo_json().unwrap()["ok"], false, "{corpo}");
    }
    fs::write(pasta.join("outro.txt"), "x").unwrap();
    let r = p.responder(&post(&p, "escolher_anexo", "{}", &[]));
    assert_eq!(r.corpo_json().unwrap()["ok"], false);

    let r = p.responder(&post(
        &p,
        "conectar",
        r#"{"entrada":"acme-ssh-salto"}"#,
        &[],
    ));
    assert_eq!(
        r.corpo_json().unwrap(),
        serde_json::json!({"ok": true, "valor": null})
    );
    for corpo in [
        r#"{"entrada":"acme-pg-aprovacao"}"#,
        r#"{"entrada":"-oProxyCommand=x"}"#,
        r#"{"entrada":5}"#,
    ] {
        let r = p.responder(&post(&p, "conectar", corpo, &[]));
        assert_eq!(r.corpo_json().unwrap()["ok"], false, "{corpo}");
    }
    fs::remove_dir_all(base).unwrap();
}

/// Fase 53a (S2, S11) pela ponte: `carregar_chave` devolve só o tamanho (nunca a chave
/// nem o caminho); a chave vai no salvar como `"carregada"` e o daemon a guarda; campo
/// fora da lista, argumento a mais ou de outro tipo são recusados antes do seletor;
/// texto que não é chave dá o aviso fixo; `"carregada"` sem nada carregado (ou depois de
/// salvo, que descarta) é pedido inválido.
#[test]
fn ataque_carregar_chave_pela_ponte() {
    use ward_test_bridge::simulados::SeletorDePasta;
    let base = dist("chave53a");
    let pasta = base.join("chave");
    fs::create_dir_all(&pasta).unwrap();
    let miolo = "ZmFsc28tY2hhdmUtcG9udGUtNTNh";
    let rotulo = ["OPENSSH", " PRIVATE", " KEY"].concat();
    let chave = format!("-----BEGIN {rotulo}-----\n{miolo}\n-----END {rotulo}-----\n");
    fs::write(pasta.join("deploy_ed25519"), &chave).unwrap();
    let servico = Servico::novo(DaemonFalso::cheio(), Copiador::novo(Area::default()))
        .com_seletor(SeletorDePasta(pasta.clone()));
    let (p, _l) = Ponte::abrir(servico, &base.join("dist")).unwrap();
    for corpo in [
        r#"{}"#,
        r#"{"campo":"senha"}"#,
        r#"{"campo":"passphrase"}"#,
        r#"{"campo":5}"#,
        r#"{"campo":"chave_privada","caminho":"/etc/shadow"}"#,
    ] {
        let r = p.responder(&post(&p, "carregar_chave", corpo, &[]));
        assert_eq!(r.corpo_json().unwrap()["ok"], false, "{corpo}");
    }
    let r = p.responder(&post(
        &p,
        "carregar_chave",
        r#"{"campo":"chave_privada"}"#,
        &[],
    ));
    let corpo = String::from_utf8_lossy(&r.corpo).into_owned();
    assert!(
        !corpo.contains(miolo) && !corpo.contains("deploy_ed25519") && !corpo.contains('/'),
        "{corpo}"
    );
    assert_eq!(
        r.corpo_json().unwrap()["valor"],
        serde_json::json!({"resultado": "carregada", "tamanho": chave.len()})
    );
    let novo = |nome: &str| {
        serde_json::json!({ "formulario": {
            "tipo": "servidor", "nome": nome, "grupo": "clientex", "ambiente": "stage",
            "projeto": "", "tags": [],
            "campos": {"host": "127.0.0.1", "porta": "2223", "usuario": "deploy", "salto": ""},
            "segredos": {"senha": "vazio", "chave_privada": "carregada", "passphrase": "vazio"},
            "extras": [], "anexos": []
        }})
        .to_string()
    };
    let r = p.responder(&post(
        &p,
        "salvar_credencial",
        &novo("clientex-ssh-53a"),
        &[],
    ));
    assert_eq!(
        r.corpo_json().unwrap()["valor"]["salvo"],
        "clientex-ssh-53a",
        "{}",
        String::from_utf8_lossy(&r.corpo)
    );
    let r = p.responder(&post(
        &p,
        "detalhe_credencial",
        r#"{"entrada":"clientex-ssh-53a"}"#,
        &[],
    ));
    assert!(!String::from_utf8_lossy(&r.corpo).contains(miolo));
    // Salvo, a chave carregada foi descartada: "carregada" de novo é pedido inválido.
    let r = p.responder(&post(
        &p,
        "salvar_credencial",
        &novo("clientex-ssh-53b"),
        &[],
    ));
    assert_eq!(r.corpo_json().unwrap()["ok"], false);
    // Texto que não é chave: aviso fixo, nada guardado.
    fs::write(pasta.join("deploy_ed25519"), "falso-texto-53a\n").unwrap();
    let r = p.responder(&post(
        &p,
        "carregar_chave",
        r#"{"campo":"chave_privada"}"#,
        &[],
    ));
    assert!(!String::from_utf8_lossy(&r.corpo).contains("falso-texto"));
    assert_eq!(
        r.corpo_json().unwrap()["valor"]["resultado"],
        "aviso",
        "{}",
        String::from_utf8_lossy(&r.corpo)
    );
    let r = p.responder(&post(
        &p,
        "salvar_credencial",
        &novo("clientex-ssh-53b"),
        &[],
    ));
    assert_eq!(r.corpo_json().unwrap()["ok"], false);
    fs::remove_dir_all(base).unwrap();
}

/// Fase 30 (V5.7): primeiro uso pela ponte, com o daemon falso sem cofre. PIN com 3 ou 7
/// dígitos, com letras e confirmação diferente voltam como erro de campo sem ir ao
/// daemon; dados com campo a mais ou fora do formato são recusados; nenhuma resposta
/// traz a senha ou o PIN; depois de criado, o cofre está destravado e vazio, e criar de
/// novo é recusado.
#[test]
fn ataque_primeiro_uso_pela_ponte() {
    let base = dist("primeiro-uso");
    let servico = Servico::novo(DaemonFalso::sem_cofre(), Copiador::novo(Area::default()));
    let (p, _l) = Ponte::abrir(servico, &base.join("dist")).unwrap();
    let estado = || {
        p.responder(&post(&p, "estado_do_cofre", "{}", &[]))
            .corpo_json()
            .unwrap()["valor"]["estado"]
            .clone()
    };
    assert_eq!(estado(), "sem-cofre");
    let dados = |senha: &str, conf: &str, pin: &str, conf_pin: &str| {
        serde_json::json!({ "dados": {
            "senha": senha, "confirmacao": conf, "pin": pin, "confirmacao_pin": conf_pin
        }})
        .to_string()
    };
    let senha = "falso-mestra-ward-01";
    for (corpo, campo) in [
        (dados(senha, senha, "135", "135"), "pin"),
        (dados(senha, senha, "1357901", "1357901"), "pin"),
        (dados(senha, senha, "13579a", "13579a"), "pin"),
        (dados(senha, senha, "135790", "135791"), "confirmacao_pin"),
        (
            dados(senha, "falso-mestra-ward-02", "135790", "135790"),
            "confirmacao",
        ),
        (
            dados("falso-curta", "falso-curta", "135790", "135790"),
            "senha",
        ),
    ] {
        let r = p.responder(&post(&p, "criar_cofre", &corpo, &[]));
        let texto = String::from_utf8_lossy(&r.corpo).into_owned();
        let v = r.corpo_json().unwrap();
        assert_eq!(v["valor"]["criado"], false, "{v}");
        assert_eq!(v["valor"]["erros"][0]["campo"], campo, "{v}");
        for agulha in ["falso-", "1357"] {
            assert!(!texto.contains(agulha), "{texto}");
        }
    }
    assert_eq!(estado(), "sem-cofre");
    for corpo in [
        r#"{"dados":{"senha":"falso-mestra-ward-01","confirmacao":"falso-mestra-ward-01","pin":"135790","confirmacao_pin":"135790","nivel":"livre"}}"#,
        r#"{"dados":{"senha":"falso-mestra-ward-01","confirmacao":"falso-mestra-ward-01","pin":135790,"confirmacao_pin":"135790"}}"#,
        r#"{"senha":"falso-mestra-ward-01","pin":"135790"}"#,
        r#"{"dados":"falso-mestra-ward-01"}"#,
    ] {
        let r = p.responder(&post(&p, "criar_cofre", corpo, &[]));
        let v = r.corpo_json().unwrap();
        assert_eq!(v["codigo"], "pedido-invalido", "{v}");
        assert!(!String::from_utf8_lossy(&r.corpo).contains("falso-"));
    }
    let r = p.responder(&post(
        &p,
        "criar_cofre",
        &dados(senha, senha, "135790", "135790"),
        &[],
    ));
    assert_eq!(r.corpo_json().unwrap()["valor"]["criado"], true);
    assert!(!String::from_utf8_lossy(&r.corpo).contains("falso-"));
    assert_eq!(estado(), "destravado");
    let r = p.responder(&post(&p, "listar_credenciais", "{}", &[]));
    assert_eq!(r.corpo_json().unwrap()["valor"], serde_json::json!([]));
    let r = p.responder(&post(
        &p,
        "criar_cofre",
        &dados(senha, senha, "2468", "2468"),
        &[],
    ));
    assert_eq!(r.corpo_json().unwrap()["codigo"], "cofre-ja-existe");
    fs::remove_dir_all(base).unwrap();
}

/// V3.37/V3.38 (D63): a ponte executada à mão (ou uma cópia, ou com outro pai) recebe
/// `agente` do daemon de teste e sai na primeira resposta, sem abrir a porta HTTP; com
/// `app`, segue. O "daemon" aqui é um servidor do mesmo uid numa pasta 0700, que
/// responde `estado` com o papel pedido e anota o que recebeu.
#[test]
fn ataque_ponte_sem_papel_app_sai_na_primeira_resposta() {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    use std::os::unix::net::UnixListener;
    use ward_app_core::daemon::DaemonSocket;
    use ward_test_bridge::partida::{SEM_PAPEL_APP, conectar_com_papel_app};

    let uid = fs::metadata("/proc/self").unwrap().uid();
    for papel in ["agente", "extensao", "app"] {
        let dir = PathBuf::from(format!(
            "/run/user/{uid}/ward-ponte-papel-{}-{papel}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let sock = dir.join("wardd.sock");
        let l = UnixListener::bind(&sock).unwrap();
        fs::set_permissions(&sock, fs::Permissions::from_mode(0o600)).unwrap();
        let servidor = std::thread::spawn(move || {
            let (mut c, _) = l.accept().unwrap();
            let mut recebidos = Vec::new();
            loop {
                let mut n = [0u8; 4];
                if c.read_exact(&mut n).is_err() {
                    break;
                }
                let mut b = vec![0u8; u32::from_be_bytes(n) as usize];
                c.read_exact(&mut b).unwrap();
                recebidos.push(String::from_utf8(b).unwrap());
                let r = format!(
                    r#"{{"v":1,"tipo":"estado","dados":{{"travado":true,"cofre_existe":true,"papel":"{papel}"}}}}"#
                );
                c.write_all(&(r.len() as u32).to_be_bytes()).unwrap();
                c.write_all(r.as_bytes()).unwrap();
            }
            recebidos
        });
        let r = conectar_com_papel_app(DaemonSocket::teste_em(sock));
        if papel == "app" {
            assert!(r.is_ok(), "{:?}", r.err());
        } else {
            assert_eq!(r.as_ref().err().map(String::as_str), Some(SEM_PAPEL_APP));
        }
        drop(r);
        let recebidos = servidor.join().unwrap();
        assert_eq!(recebidos.len(), 1, "{recebidos:?}");
        assert!(recebidos[0].contains(r#""op":"estado""#), "{recebidos:?}");
        let _ = fs::remove_dir_all(&dir);
    }
}

/// Fase 31 (S13, D20): destravar pela ponte. A senha sai do corpo como `SenhaMestra`; o
/// formato errado nem chega ao daemon; a resposta não ecoa a senha nem diz quantas
/// tentativas houve; o `estado_do_cofre` travado traz só `pedido` (aqui `null`).
#[test]
fn ataque_destravar_pela_ponte() {
    let (p, _l, _a, base) = ponte("destravar");
    let estado = |p: &Ponte<DaemonFalso, Area>| {
        p.responder(&post(p, "estado_do_cofre", "{}", &[]))
            .corpo_json()
            .unwrap()["valor"]
            .clone()
    };
    // Trava pelo 5º PIN errado (Fase 35), como no app.
    for _ in 0..5 {
        p.responder(&post(
            &p,
            "mudar_nivel",
            r#"{"entrada":"acme-pg-prod","nivel":"livre","pin":"000000"}"#,
            &[],
        ));
    }
    assert_eq!(
        estado(&p),
        serde_json::json!({"estado": "travado", "pedido": null})
    );
    let destravar = |p: &Ponte<DaemonFalso, Area>, corpo: &str| {
        let r = p.responder(&post(p, "destravar", corpo, &[]));
        let texto = String::from_utf8_lossy(&r.corpo).into_owned();
        assert!(
            !texto.contains("falso-mestra"),
            "senha na resposta: {texto}"
        );
        r.corpo_json().unwrap()
    };
    for corpo in [
        r#"{}"#,
        r#"{"senha":""}"#,
        r#"{"senha":135790}"#,
        r#"{"senha":null}"#,
        r#"{"senha":["falso-mestra-ward-01"]}"#,
        r#"{"senha":"falso-mestra-ward-01","extra":1}"#,
    ] {
        let v = destravar(&p, corpo);
        assert_eq!(v["codigo"], "pedido-invalido", "{corpo}: {v}");
    }
    assert_eq!(estado(&p)["estado"], "travado");
    let v = destravar(&p, r#"{"senha":"falso-mestra-ward-00"}"#);
    assert_eq!(v["codigo"], "senha-incorreta", "{v}");
    let msg = v["mensagem"].as_str().unwrap().to_owned();
    assert!(!msg.chars().any(|c| c.is_ascii_digit()), "{msg}");
    assert_eq!(estado(&p)["estado"], "travado");
    // A 2ª tentativa espera 1 s no daemon (D20) e destrava.
    let v = destravar(&p, r#"{"senha":"falso-mestra-ward-01"}"#);
    assert_eq!(v, serde_json::json!({"ok": true, "valor": null}));
    assert_eq!(
        estado(&p),
        serde_json::json!({"estado": "destravado", "pedido": null})
    );
    // `ouvir_avisos` não existe na ponte (o canal é do Tauri).
    let r = p.responder(&post(&p, "ouvir_avisos", "{}", &[]));
    assert_eq!(r.corpo_json().unwrap()["codigo"], "pedido-invalido");
    let _ = fs::remove_dir_all(base);
}

/// Fase 39 (V5.23, S12): "Travar agora" pela ponte. Argumento qualquer é recusado; o
/// travar limpa a área de transferência que ainda tem o valor copiado e deixa o cofre
/// travado (revelar responde "travado"); a resposta não traz valor.
#[test]
fn ataque_travar_pela_ponte() {
    let (p, _l, area, base) = ponte("travar");
    let r = p.responder(&post(&p, "travar", r#"{"x":1}"#, &[]));
    assert_eq!(r.corpo_json().unwrap()["codigo"], "pedido-invalido");
    let estado = |p: &Ponte<DaemonFalso, Area>| {
        p.responder(&post(p, "estado_do_cofre", "{}", &[]))
            .corpo_json()
            .unwrap()["valor"]["estado"]
            .clone()
    };
    assert_eq!(estado(&p), "destravado");
    p.responder(&post(
        &p,
        "copiar_campo",
        r#"{"entrada":"acme-pg-dev","campo":"senha"}"#,
        &[],
    ));
    let copiado = area.0.lock().unwrap().is_some();
    let r = p.responder(&post(&p, "travar", "{}", &[]));
    let texto = String::from_utf8_lossy(&r.corpo).into_owned();
    assert_eq!(
        r.corpo_json().unwrap(),
        serde_json::json!({"ok": true, "valor": null})
    );
    assert!(!texto.contains("falso-"), "{texto}");
    if copiado {
        assert!(area.0.lock().unwrap().is_none(), "área não foi limpa");
    }
    assert_eq!(estado(&p), "travado");
    let r = p.responder(&post(
        &p,
        "revelar_campo",
        r#"{"entrada":"acme-pg-dev","campo":"senha"}"#,
        &[],
    ));
    assert_eq!(r.corpo_json().unwrap()["codigo"], "travado");
    let _ = fs::remove_dir_all(base);
}

/// Fase 38 (V5.22, S9): a tela Configurações pela ponte. Padrão 4 h e backup desligado;
/// tempo fora de 1 a 1440, campo a mais e tipo errado recusados; trocar o PIN pede o
/// atual e trocar a senha pede a atual; nenhuma resposta traz PIN, senha ou hash.
#[test]
fn ataque_configuracoes_pela_ponte() {
    let base = dist("configuracoes");
    let servico = Servico::novo(DaemonFalso::cheio(), Copiador::novo(Area::default()));
    let (p, _l) = Ponte::abrir(servico, &base.join("dist")).unwrap();
    let pedir = |comando: &str, corpo: &str| {
        let r = p.responder(&post(&p, comando, corpo, &[]));
        let texto = String::from_utf8_lossy(&r.corpo).into_owned();
        for agulha in ["falso-", "1357", "2468", "argon2", "$m="] {
            assert!(!texto.contains(agulha), "{comando}: {texto}");
        }
        r.corpo_json().unwrap()
    };
    let v = pedir("configuracoes", "{}");
    assert_eq!(v["valor"]["minutos_trava"], 240, "{v}");
    assert_eq!(v["valor"]["backup_externo"], false, "{v}");
    assert_eq!(
        pedir("configuracoes", r#"{"hash":1}"#)["codigo"],
        "pedido-invalido"
    );
    let v = pedir("definir_tempo_trava", r#"{"minutos":60}"#);
    assert_eq!(v["valor"]["minutos_trava"], 60, "{v}");
    for corpo in [
        r#"{"minutos":0}"#,
        r#"{"minutos":1441}"#,
        r#"{"minutos":"60"}"#,
        r#"{"minutos":-1}"#,
        r#"{"minutos":60,"pin":"135790"}"#,
        r#"{}"#,
    ] {
        assert_eq!(
            pedir("definir_tempo_trava", corpo)["codigo"],
            "pedido-invalido",
            "{corpo}"
        );
    }
    assert_eq!(pedir("configuracoes", "{}")["valor"]["minutos_trava"], 60);
    let troca_pin = |atual: &str, novo: &str, conf: &str| {
        serde_json::json!({ "dados": { "atual": atual, "novo": novo, "confirmacao": conf } })
            .to_string()
    };
    for (corpo, campo) in [
        (troca_pin("135", "2468", "2468"), "atual"),
        (troca_pin("135790", "13579a", "13579a"), "novo"),
        (troca_pin("135790", "2468", "2469"), "confirmacao"),
    ] {
        let v = pedir("trocar_pin", &corpo);
        assert_eq!(v["valor"]["trocado"], false, "{v}");
        assert_eq!(v["valor"]["erros"][0]["campo"], campo, "{v}");
    }
    for corpo in [
        r#"{"dados":{"atual":"135790","novo":"2468","confirmacao":"2468","nivel":"livre"}}"#,
        r#"{"dados":{"atual":135790,"novo":"2468","confirmacao":"2468"}}"#,
        r#"{"dados":{"atual":"135790","novo":"2468","confirmacao":"2468"},"x":1}"#,
        r#"{"atual":"135790","novo":"2468"}"#,
    ] {
        assert_eq!(
            pedir("trocar_pin", corpo)["codigo"],
            "pedido-invalido",
            "{corpo}"
        );
    }
    assert_eq!(
        pedir("trocar_pin", &troca_pin("246801", "2468", "2468"))["codigo"],
        "pin-incorreto"
    );
    assert_eq!(
        pedir("trocar_pin", &troca_pin("135790", "2468", "2468"))["valor"]["trocado"],
        true
    );
    let troca_senha = |atual: &str, nova: &str| {
        serde_json::json!({ "dados": { "atual": atual, "nova": nova, "confirmacao": nova } })
            .to_string()
    };
    let v = pedir(
        "trocar_senha_mestra",
        &troca_senha("falso-mestra-ward-01", "falso-curta"),
    );
    assert_eq!(v["valor"]["erros"][0]["campo"], "nova", "{v}");
    assert_eq!(
        pedir(
            "trocar_senha_mestra",
            &troca_senha("falso-errada-mestra-09", "falso-nova-mestra-02")
        )["codigo"],
        "senha-incorreta"
    );
    assert_eq!(
        pedir(
            "trocar_senha_mestra",
            &troca_senha("falso-mestra-ward-01", "falso-nova-mestra-02")
        )["valor"]["trocado"],
        true
    );
    let _ = fs::remove_dir_all(base);
}

/// Fase 48 (V7.9): backup externo pela ponte. Ligar com um pendrive da lista, desligar
/// mantendo o UUID; UUID forjado (fora do formato ou fora da lista), tipo errado e campo
/// a mais recusados; nenhuma resposta traz caminho de montagem; o rótulo estranho vem
/// com o invisível em escape.
#[test]
fn ataque_backup_externo_pela_ponte() {
    let base = dist("backup-externo");
    let servico = Servico::novo(DaemonFalso::cheio(), Copiador::novo(Area::default()));
    let (p, _l) = Ponte::abrir(servico, &base.join("dist")).unwrap();
    let pedir = |comando: &str, corpo: &str| {
        let r = p.responder(&post(&p, comando, corpo, &[]));
        let texto = String::from_utf8_lossy(&r.corpo).into_owned();
        for agulha in ["/media", "/run/", "montagem", "\u{202E}", "\u{200B}"] {
            assert!(!texto.contains(agulha), "{comando}: {texto}");
        }
        r.corpo_json().unwrap()
    };
    let v = pedir("backup_externo", "{}");
    assert_eq!(v["valor"]["ligado"], false, "{v}");
    assert!(v["valor"]["uuid"].is_null(), "{v}");
    let v = pedir("listar_pendrives", "{}");
    let lista = v["valor"].as_array().unwrap();
    assert_eq!(lista.len(), 4, "{v}");
    assert!(
        lista
            .iter()
            .any(|p| p["rotulo"].as_str().unwrap().contains("\\u{202E}")),
        "{v}"
    );
    for (comando, corpo) in [
        ("backup_externo", r#"{"uuid":"7C1E-42A9"}"#),
        ("listar_pendrives", r#"{"todos":true}"#),
        ("definir_backup_externo", r#"{"ligado":true}"#),
        (
            "definir_backup_externo",
            r#"{"ligado":"true","uuid":"7C1E-42A9"}"#,
        ),
        ("definir_backup_externo", r#"{"ligado":true,"uuid":4242}"#),
        (
            "definir_backup_externo",
            r#"{"ligado":true,"uuid":"../etc"}"#,
        ),
        (
            "definir_backup_externo",
            r#"{"ligado":true,"uuid":"7C1E-42A9","montagem":"/media/x"}"#,
        ),
        (
            "definir_backup_externo",
            r#"{"ligado":false,"uuid":"DEAD-BEEF"}"#,
        ),
        ("definir_backup_externo", r#"{"uuid":"7C1E-42A9"}"#),
    ] {
        assert_eq!(
            pedir(comando, corpo)["codigo"],
            "pedido-invalido",
            "{comando} {corpo}"
        );
    }
    assert_eq!(
        pedir(
            "definir_backup_externo",
            r#"{"ligado":true,"uuid":"DEAD-BEEF"}"#
        )["codigo"],
        "pendrive-nao-conectado"
    );
    assert_eq!(pedir("backup_externo", "{}")["valor"]["ligado"], false);
    let v = pedir(
        "definir_backup_externo",
        r#"{"ligado":true,"uuid":"7C1E-42A9"}"#,
    );
    assert_eq!(v["valor"]["ligado"], true, "{v}");
    assert_eq!(v["valor"]["pendrive"]["rotulo"], "WARD-BACKUP", "{v}");
    assert_eq!(v["valor"]["pendrive"]["montado"], true, "{v}");
    assert_eq!(
        pedir("configuracoes", "{}")["valor"]["backup_externo"],
        true
    );
    let v = pedir("definir_backup_externo", r#"{"ligado":false,"uuid":null}"#);
    assert_eq!(v["valor"]["ligado"], false, "{v}");
    assert_eq!(v["valor"]["uuid"], "7C1E-42A9", "{v}");
    let _ = fs::remove_dir_all(base);
}

/// Fase 47 (V7.7): restaurar backup pela ponte. A lista traz só nome, instante e (Fase
/// 49a) as marcas conferida/fora de ordem; versão forjada (caminho, `..`, fora da
/// lista), PIN errado, senha da versão errada, tipo errado e campo a mais não mudam
/// nada; nenhuma resposta traz segredo, PIN, senha, caminho ou geração; o restaurar
/// certo troca as entradas pelas da versão.
#[test]
fn ataque_restaurar_backup_pela_ponte() {
    let base = dist("restaurar-backup");
    let servico = Servico::novo(DaemonFalso::cheio(), Copiador::novo(Area::default()));
    let (p, _l) = Ponte::abrir(servico, &base.join("dist")).unwrap();
    let pedir = |comando: &str, corpo: &str| {
        let r = p.responder(&post(&p, comando, corpo, &[]));
        let texto = String::from_utf8_lossy(&r.corpo).into_owned();
        for s in SEGREDOS {
            assert!(!texto.contains(s), "{comando}: segredo na resposta");
        }
        for agulha in [
            "135790",
            "falso-mestra-ward-01",
            "/home",
            ".local",
            "backups/",
            "geracao",
        ] {
            assert!(!texto.contains(agulha), "{comando}: {agulha} em {texto}");
        }
        r.corpo_json().unwrap()
    };
    let corpo = |versao: &str, senha: &str, pin: &str| {
        serde_json::json!({ "dados": { "versao": versao, "senha": senha, "pin": pin } }).to_string()
    };
    let entradas = || {
        pedir("listar_credenciais", "{}")["valor"]
            .as_array()
            .unwrap()
            .len()
    };
    let antes = entradas();
    let v = pedir("listar_backups", "{}");
    let lista = v["valor"].as_array().unwrap().clone();
    assert_eq!(lista.len(), 30, "{v}");
    for item in &lista {
        let chaves: Vec<_> = item.as_object().unwrap().keys().cloned().collect();
        assert_eq!(
            chaves,
            ["conferida", "fora_de_ordem", "instante", "nome"],
            "{item}"
        );
    }
    let alvo = lista[4]["nome"].as_str().unwrap().to_owned();
    assert_eq!(
        pedir("listar_backups", r#"{"todos":true}"#)["codigo"],
        "pedido-invalido"
    );
    // Formato, tipo e campo a mais: recusados antes do daemon.
    let a_mais = serde_json::json!({ "dados": {
        "versao": alvo, "senha": "falso-mestra-ward-01", "pin": "135790", "geracao": 9 } });
    let pin_numero = serde_json::json!({ "dados": {
        "versao": alvo, "senha": "falso-mestra-ward-01", "pin": 135790 } });
    let fora = serde_json::json!({ "dados": {
        "versao": alvo, "senha": "falso-mestra-ward-01", "pin": "135790" }, "a_mais": 1 });
    for c in [
        corpo("../cofre.ward", "falso-mestra-ward-01", "135790"),
        corpo(
            "/home/usuario/.local/share/ward/cofre.ward",
            "falso-mestra-ward-01",
            "135790",
        ),
        corpo(&alvo, "falso-mestra-ward-01", "12ab"),
        corpo(&alvo, "", "135790"),
        a_mais.to_string(),
        pin_numero.to_string(),
        fora.to_string(),
        r#"{"versao":"ward-20261108-143200.ward"}"#.to_owned(),
    ] {
        assert_eq!(
            pedir("restaurar_backup", &c)["codigo"],
            "pedido-invalido",
            "{c}"
        );
    }
    // Versão no formato, mas inexistente.
    let inexistente = corpo(
        "ward-19990101-000000.ward",
        "falso-mestra-ward-01",
        "135790",
    );
    assert_eq!(
        pedir("restaurar_backup", &inexistente)["codigo"],
        "backup-inexistente"
    );
    // PIN errado e senha da versão errada.
    let pin_errado = corpo(&alvo, "falso-mestra-ward-01", "000000");
    assert_eq!(
        pedir("restaurar_backup", &pin_errado)["codigo"],
        "pin-incorreto"
    );
    let senha_errada = corpo(&alvo, "falso-mestra-ward-09", "135790");
    assert_eq!(
        pedir("restaurar_backup", &senha_errada)["codigo"],
        "senha-incorreta"
    );
    assert_eq!(entradas(), antes);
    // O certo: a versão 4 tem quatro entradas a menos.
    let v = pedir(
        "restaurar_backup",
        &corpo(&alvo, "falso-mestra-ward-01", "135790"),
    );
    assert_eq!(v["valor"]["restaurado"], true, "{v}");
    assert_eq!(v["valor"]["entradas"], antes - 4, "{v}");
    assert_eq!(entradas(), antes - 4);
    let _ = fs::remove_dir_all(base);
}
