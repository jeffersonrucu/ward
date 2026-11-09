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
    assert_eq!(
        nomes.len(),
        serde_json::from_str::<serde_json::Value>(include_str!(
            "../../../dev/fixtures/entradas.json"
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

/// Fase 34 (V5.12, V5.24, S11) pela ponte: `escolher_anexo` devolve só id, nome e
/// tamanho (o conteúdo nunca), o anexo vai no salvar pelo id, `remover_anexo` com id
/// forjado é recusado, e `conectar` devolve `null` (o token não vem) e só vale para
/// `servidor`. A pasta do seletor tem mais de um arquivo: nada é escolhido.
#[test]
fn ataque_anexos_e_conectar_pela_ponte() {
    use ward_ponte_teste::simulados::{SeletorDePasta, TerminalDeTeste};
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
    use ward_app_nucleo::daemon::DaemonSocket;
    use ward_ponte_teste::partida::{SEM_PAPEL_APP, conectar_com_papel_app};

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
