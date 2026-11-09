//! Ataques à ponte de teste (D7, S11): cada um tem de falhar.
#![cfg(feature = "modo-teste")]

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ward_app_nucleo::ErroApp;
use ward_app_nucleo::area_transferencia::{AreaTransferencia, Copiador};
use ward_app_nucleo::daemon_falso::{DaemonFalso, SEGREDOS};
use ward_app_nucleo::servico::Servico;
use ward_ponte_teste::http::{MAX_CABECALHO, MAX_CORPO, Pedido, Ponte, ler_pedido};
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
    assert_eq!(nomes.len(), 21);
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
    let mut da_ponte: Vec<String> = ward_ponte_teste::http::CSP
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
