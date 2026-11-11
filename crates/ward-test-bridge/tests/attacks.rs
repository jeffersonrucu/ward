//! Attacks on the test bridge (D7, S11): each one must fail.

// m2:Phase 54a: without `test-mode` the binaries this test runs are the release ones,
// which only know the system socket (`/run/ward/<uid>/wardd.sock`, the installed
// daemon). The build fails, loudly, instead of a test reaching it.
#[cfg(not(feature = "test-mode"))]
compile_error!(
    "este teste exige a feature test-mode (cargo test --all-features ou --features test-mode): sem ela os binários do teste são os de release, que só conhecem o socket do sistema"
);

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ward_app_core::AppError;
use ward_app_core::clipboard::{Clipboard, Copier};
use ward_app_core::fake_daemon::{FakeDaemon, SECRETS};
use ward_app_core::service::Service;
use ward_test_bridge::http::{Bridge, MAX_BODY, MAX_HEADER, Request, read_request};
use zeroize::Zeroizing;

/// The password of `acme-pg-dev` in the test vault (D15), the one the "copy" asks for.
const SECRET: &[u8] = b"falso-PG:s3nha/01@#";

#[derive(Default, Clone)]
struct FakeClipboard(Arc<Mutex<Option<Vec<u8>>>>);
impl Clipboard for FakeClipboard {
    fn write(&self, v: &[u8]) -> Result<(), AppError> {
        *self.0.lock().unwrap() = Some(v.to_vec());
        Ok(())
    }
    fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>, AppError> {
        Ok(self.0.lock().unwrap().clone().map(Zeroizing::new))
    }
    fn clear(&self) -> Result<(), AppError> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

fn dist(name: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!("ward-bridge-{}-{name}", std::process::id()));
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

fn bridge(
    name: &str,
) -> (
    Arc<Bridge<FakeDaemon, FakeClipboard>>,
    std::net::TcpListener,
    FakeClipboard,
    PathBuf,
) {
    let base = dist(name);
    let clipboard = FakeClipboard::default();
    let service = Service::new(FakeDaemon::full(), Copier::new(clipboard.clone()));
    let (p, l) = Bridge::open(service, &base.join("dist")).unwrap();
    (p, l, clipboard, base)
}

fn post(
    p: &Bridge<FakeDaemon, FakeClipboard>,
    command: &str,
    body: &str,
    changes: &[(&str, Option<&str>)],
) -> Request {
    let host = p.origin().trim_start_matches("http://").to_owned();
    let auth = format!("Bearer {}", p.token_for_test());
    let mut headers: Vec<(String, String)> = vec![
        ("Host".into(), host),
        ("Origin".into(), p.origin().into()),
        ("Authorization".into(), auth),
        ("Content-Type".into(), "application/json".into()),
    ];
    for (n, v) in changes {
        headers.retain(|(x, _)| !x.eq_ignore_ascii_case(n));
        if let Some(v) = v {
            headers.push(((*n).into(), (*v).into()));
        }
    }
    let refs: Vec<(&str, &str)> = headers
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    Request::new("POST", &format!("/ponte/{command}"), &refs, body.as_bytes())
}

#[test]
fn copy_works_and_the_response_has_no_value() {
    let (p, _l, clipboard, base) = bridge("copiar");
    let r = p.respond(&post(
        &p,
        "copiar_campo",
        r#"{"entrada":"acme-pg-dev","campo":"password"}"#,
        &[],
    ));
    assert_eq!(r.status, 200);
    assert_eq!(
        r.body_json().unwrap(),
        serde_json::json!({ "ok": true, "valor": null })
    );
    assert!(!r.body.windows(SECRET.len()).any(|j| j == SECRET));
    assert_eq!(clipboard.0.lock().unwrap().as_deref(), Some(SECRET));
    let r = p.respond(&post(&p, "estado_do_cofre", "{}", &[]));
    assert_eq!(r.body_json().unwrap()["valor"]["estado"], "destravado");
    fs::remove_dir_all(base).unwrap();
}

/// Any secret of the test vault in the response bytes (whole, JSON-escaped or per
/// line)?
fn has_secret(body: &[u8]) -> bool {
    let text = String::from_utf8_lossy(body);
    SECRETS.iter().any(|s| {
        let json = serde_json::to_string(s).unwrap();
        text.contains(s)
            || text.contains(&json[1..json.len() - 1])
            || s.lines().any(|l| l.len() >= 6 && text.contains(l))
    })
}

#[test]
fn attack_credentials_screen_responses_without_secret() {
    // V5.3 through the bridge: the list and the detail of all entries of the test vault,
    // as they reach the interface, have no secret.
    let (p, _l, _a, base) = bridge("credenciais");
    let r = p.respond(&post(&p, "listar_credenciais", "{}", &[]));
    assert_eq!(r.status, 200);
    assert!(!has_secret(&r.body));
    let list = r.body_json().unwrap()["valor"].clone();
    let names: Vec<String> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["nome"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        names.len(),
        serde_json::from_str::<serde_json::Value>(include_str!(
            "../../../dev/fixtures/entries.json"
        ))
        .unwrap()["entries"]
            .as_array()
            .unwrap()
            .len()
    );
    for name in &names {
        let body = serde_json::json!({ "entrada": name }).to_string();
        let r = p.respond(&post(&p, "detalhe_credencial", &body, &[]));
        assert_eq!(r.body_json().unwrap()["ok"], true, "{name}");
        assert!(!has_secret(&r.body), "segredo no detalhe de {name}");
    }
    // Name off the format, of another type or absent: error without echoing the request.
    for body in [
        r#"{"entrada":"acme-pg-dev\n"}"#,
        r#"{"entrada":"<img src=x onerror=alert(1)>"}"#,
        r#"{"entrada":1}"#,
        r#"{}"#,
    ] {
        let r = p.respond(&post(&p, "detalhe_credencial", body, &[]));
        let v = r.body_json().unwrap();
        assert_eq!(v["ok"], false, "{body}");
        assert!(!String::from_utf8_lossy(&r.body).contains("img"));
    }
    fs::remove_dir_all(base).unwrap();
}

/// Phase 33 through the bridge (V5.3, V5.10, V5.11, V5.13, V5.14): save, TOTP and "use
/// this password" return no secret; only the field reveal carries the value; a forged
/// form (unknown field, level, "keep" on a new entry, type spelled another way) is
/// refused without echo.
#[test]
fn attack_form_through_the_bridge() {
    let (p, _l, _a, base) = bridge("formulario");
    let new_entry = serde_json::json!({ "formulario": {
        "tipo": "api", "nome": "beta-gitlab-api", "grupo": "beta", "ambiente": "dev",
        "projeto": "", "tags": [], "campos": {"url": "https://gitlab.exemplo.com", "variable": "GITLAB_TOKEN"},
        "segredos": {"token": {"novo": "falso-token-gitlab-99"}}, "extras": []
    }});
    let r = p.respond(&post(&p, "salvar_credencial", &new_entry.to_string(), &[]));
    let v = r.body_json().unwrap();
    assert_eq!(v["valor"]["salvo"], "beta-gitlab-api", "{v}");
    assert!(!String::from_utf8_lossy(&r.body).contains("falso-token-gitlab"));
    let r = p.respond(&post(
        &p,
        "revelar_campo",
        r#"{"entrada":"beta-gitlab-api","campo":"token"}"#,
        &[],
    ));
    assert_eq!(
        r.body_json().unwrap()["valor"]["valor"],
        "falso-token-gitlab-99"
    );
    let r = p.respond(&post(
        &p,
        "codigo_totp",
        r#"{"entrada":"acme-wp-dev"}"#,
        &[],
    ));
    let v = r.body_json().unwrap();
    assert_eq!(v["valor"]["codigo"].as_str().unwrap().len(), 6, "{v}");
    assert!(!has_secret(&r.body));
    let generate =
        r#"{"tamanho":20,"minusculas":true,"maiusculas":true,"digitos":true,"simbolos":true}"#;
    p.respond(&post(&p, "gerar_senha", generate, &[]));
    let r = p.respond(&post(
        &p,
        "usar_senha_gerada",
        r#"{"campo":"password"}"#,
        &[],
    ));
    assert_eq!(
        r.body_json().unwrap()["valor"],
        serde_json::json!({ "tamanho": 20 })
    );
    for body in [
        r#"{"formulario":{"tipo":"login","nome":"x","grupo":"g","ambiente":"dev","projeto":"","tags":[],"campos":{},"segredos":{},"extras":[],"nivel":"free"}}"#,
        r#"{"formulario":{"tipo":"login","nome":"acme-x","grupo":"g","ambiente":"dev","projeto":"","tags":[],"campos":{},"segredos":{"password":"manter"},"extras":[]}}"#,
        r#"{"anterior":5,"formulario":{"tipo":"login","nome":"x","grupo":"g","ambiente":"dev","projeto":"","tags":[],"campos":{},"segredos":{},"extras":[]}}"#,
        r#"{"formulario":"<img src=x>"}"#,
        r#"{}"#,
    ] {
        let r = p.respond(&post(&p, "salvar_credencial", body, &[]));
        let v = r.body_json().unwrap();
        assert_eq!(v["ok"], false, "{body}");
        assert!(!String::from_utf8_lossy(&r.body).contains("img"));
    }
    fs::remove_dir_all(base).unwrap();
}

/// V5.15 through the bridge (S9): the set-level command with the right PIN changes and
/// answers only the level; a wrong PIN answers the wrong-PIN code and does not change;
/// a PIN off the format, an invented level and a missing field are refused without
/// counting an error; the 5th wrong PIN in a row locks, and from then on everything
/// answers the locked code. No response carries the PIN.
#[test]
fn attack_set_level_through_the_bridge() {
    let (p, _l, _a, base) = bridge("nivel");
    let level = |p: &Bridge<FakeDaemon, FakeClipboard>| {
        let r = p.respond(&post(
            p,
            "detalhe_credencial",
            r#"{"entrada":"acme-pg-prod"}"#,
            &[],
        ));
        r.body_json().unwrap()["valor"]["nivel"].clone()
    };
    let set_level = |p: &Bridge<FakeDaemon, FakeClipboard>, body: &str| {
        let r = p.respond(&post(p, "mudar_nivel", body, &[]));
        assert!(
            !String::from_utf8_lossy(&r.body).contains("135790"),
            "PIN na resposta"
        );
        r.body_json().unwrap()
    };
    let v = set_level(
        &p,
        r#"{"entrada":"acme-pg-prod","nivel":"free","pin":"135790"}"#,
    );
    assert_eq!(
        v,
        serde_json::json!({"ok": true, "valor": {"entrada": "acme-pg-prod", "nivel": "free"}})
    );
    assert_eq!(level(&p), "free");
    // Refused before the daemon: they do not count as PIN errors (it would be 6 × 6).
    for _ in 0..6 {
        for body in [
            r#"{"entrada":"acme-pg-prod","nivel":"blocked","pin":"12"}"#,
            r#"{"entrada":"acme-pg-prod","nivel":"blocked","pin":135790}"#,
            r#"{"entrada":"acme-pg-prod","nivel":"blocked","pin":"13579a"}"#,
            r#"{"entrada":"acme-pg-prod","nivel":"admin","pin":"000000"}"#,
            r#"{"entrada":"acme-pg-prod","nivel":"blocked"}"#,
            r#"{"entrada":"acme\npg","nivel":"blocked","pin":"000000"}"#,
        ] {
            let v = set_level(&p, body);
            assert_eq!(v["ok"], false, "{body}");
            assert_eq!(v["codigo"], "pedido-invalido", "{body}: {v}");
        }
    }
    assert_eq!(level(&p), "free");
    for k in 1..=5 {
        let v = set_level(
            &p,
            r#"{"entrada":"acme-pg-prod","nivel":"blocked","pin":"000000"}"#,
        );
        assert_eq!(v["codigo"], "pin-incorreto", "erro {k}: {v}");
        if k < 5 {
            assert_eq!(level(&p), "free");
        }
    }
    let r = p.respond(&post(&p, "estado_do_cofre", "{}", &[]));
    assert_eq!(r.body_json().unwrap()["valor"]["estado"], "travado");
    let v = set_level(
        &p,
        r#"{"entrada":"acme-pg-prod","nivel":"blocked","pin":"135790"}"#,
    );
    assert_eq!(v["codigo"], "travado", "{v}");
    fs::remove_dir_all(base).unwrap();
}

/// V5.21 through the bridge: "generate" returns only length, strength and the symbols of
/// D35; "copy" returns `null` and the password goes to the clipboard; the value only
/// comes out on "reveal"; options off the format (length outside 8–64, wrong type,
/// missing field) are refused.
#[test]
fn attack_standalone_generator_through_the_bridge() {
    let (p, _l, clipboard, base) = bridge("gerador");
    let options =
        r#"{"tamanho":24,"minusculas":true,"maiusculas":true,"digitos":true,"simbolos":true}"#;
    let r = p.respond(&post(&p, "gerar_senha", options, &[]));
    assert_eq!(
        r.body_json().unwrap(),
        serde_json::json!({ "ok": true, "valor": {
            "tamanho": 24, "bits": 150, "forca": "muito-forte",
            "simbolos": "!#$%&()*+,-./:;<=>?@[]^_{|}~"
        }})
    );
    let r = p.respond(&post(&p, "copiar_senha_gerada", "{}", &[]));
    assert_eq!(
        r.body_json().unwrap(),
        serde_json::json!({ "ok": true, "valor": null })
    );
    let copied = clipboard.0.lock().unwrap().clone().unwrap();
    assert_eq!(copied.len(), 24);
    let r = p.respond(&post(&p, "revelar_senha_gerada", "{}", &[]));
    assert_eq!(
        r.body_json().unwrap()["valor"]["valor"]
            .as_str()
            .unwrap()
            .as_bytes(),
        &copied[..]
    );
    let r = p.respond(&post(&p, "descartar_senha_gerada", "{}", &[]));
    assert_eq!(r.body_json().unwrap()["ok"], true);
    let r = p.respond(&post(&p, "revelar_senha_gerada", "{}", &[]));
    assert_eq!(r.body_json().unwrap()["codigo"], "sem-senha-gerada");
    for body in [
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
        let r = p.respond(&post(&p, "gerar_senha", body, &[]));
        assert_eq!(
            r.body_json().unwrap()["codigo"],
            "pedido-invalido",
            "{body}"
        );
    }
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn attack_no_token_wrong_token_or_repeated() {
    let (p, _l, clipboard, base) = bridge("token");
    let body = r#"{"entrada":"acme-pg-dev","campo":"password"}"#;
    let wrong = format!("Bearer {}", "0".repeat(64));
    let prefix = format!("Bearer {}", &p.token_for_test()[..63]);
    for changes in [
        vec![("Authorization", None)],
        vec![("Authorization", Some(wrong.as_str()))],
        vec![("Authorization", Some(prefix.as_str()))],
        vec![("Authorization", Some(p.token_for_test()))],
    ] {
        assert_eq!(
            p.respond(&post(&p, "copiar_campo", body, &changes)).status,
            401,
            "{changes:?}"
        );
    }
    assert_eq!(*clipboard.0.lock().unwrap(), None, "nada copiado sem token");
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn attack_host_or_origin_of_another_site() {
    let (p, _l, _a, base) = bridge("origem");
    for (changes, status) in [
        (vec![("Host", Some("evil.example:80"))], 421),
        (vec![("Host", Some("localhost"))], 421),
        (vec![("Host", None)], 421),
        (vec![("Origin", Some("https://example.com"))], 403),
        (vec![("Origin", Some("null"))], 403),
        (vec![("Origin", None)], 403),
        (vec![("Content-Type", Some("text/plain"))], 415),
    ] {
        assert_eq!(
            p.respond(&post(&p, "estado_do_cofre", "{}", &changes))
                .status,
            status,
            "{changes:?}"
        );
    }
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn attack_plugin_or_invented_command() {
    let (p, _l, _a, base) = bridge("comando");
    for c in [
        "plugin:fs|read_text_file",
        "plugin:shell|execute",
        "plugin:http|fetch",
        "inventado",
        "revelar",
    ] {
        let r = p.respond(&post(&p, c, "{}", &[]));
        assert_eq!(r.status, 404, "{c}");
        assert_eq!(r.body_json().unwrap()["ok"], false);
    }
    for body in [
        "",
        "[]",
        "nao json",
        r#"{"entrada":1,"campo":"password"}"#,
        r#"{"entrada":"acme-pg-dev\n","campo":"password"}"#,
    ] {
        let r = p.respond(&post(&p, "copiar_campo", body, &[]));
        assert_eq!(
            r.body_json().map(|v| v["ok"].clone()),
            Some(serde_json::json!(false)),
            "{body}"
        );
    }
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn attack_file_outside_the_dist() {
    let (p, _l, _a, base) = bridge("estatico");
    let host = p.origin().trim_start_matches("http://").to_owned();
    let get = |c: &str| p.respond(&Request::new("GET", c, &[("Host", &host)], b""));
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
        p.respond(&Request::new("DELETE", "/", &[("Host", &host)], b""))
            .status,
        405
    );
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn attack_request_outside_the_limits() {
    for (raw, status) in [
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
                MAX_BODY + 1
            )
            .into_bytes(),
            413,
        ),
        (
            format!("GET / HTTP/1.1\r\nX: {}\r\n\r\n", "a".repeat(MAX_HEADER)).into_bytes(),
            431,
        ),
        (
            b"POST /ponte/x HTTP/1.1\r\nContent-Length: 10\r\n\r\nabc".to_vec(),
            400,
        ),
    ] {
        assert_eq!(
            read_request(&raw[..]).err(),
            Some(status),
            "{}",
            String::from_utf8_lossy(&raw[..40.min(raw.len())])
        );
    }
    let ok = read_request(&b"POST /ponte/x HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}"[..])
        .ok()
        .unwrap();
    assert_eq!(ok.body, b"{}");
}

/// The bridge CSP is the app one (`tauri.conf.json`), swapping only the IPC for `'self'`.
#[test]
fn bridge_csp_is_the_app_one() {
    let conf: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../app/src-tauri/tauri.conf.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let mut expected: Vec<String> = conf["app"]["security"]["csp"]
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
    expected.push("frame-ancestors 'none'".into());
    expected.sort();
    let mut from_bridge: Vec<String> = ward_test_bridge::http::CSP
        .split("; ")
        .map(str::to_owned)
        .collect();
    from_bridge.sort();
    assert_eq!(from_bridge, expected);
}

#[test]
fn tcp_only_on_127_0_0_1_with_security_headers() {
    let (p, l, _a, base) = bridge("tcp");
    assert!(l.local_addr().unwrap().ip().is_loopback());
    assert!(p.opening_url().starts_with("http://127.0.0.1:"));
    assert!(p.opening_url().contains("/#token="));
    let address = l.local_addr().unwrap();
    let server = Arc::clone(&p);
    std::thread::spawn(move || server.serve(l));
    let mut s = TcpStream::connect(address).unwrap();
    write!(
        s,
        "GET / HTTP/1.1\r\nHost: {}\r\n\r\n",
        p.origin().trim_start_matches("http://")
    )
    .unwrap();
    let mut response = String::new();
    s.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    for h in [
        "Content-Security-Policy: default-src 'none'",
        "X-Content-Type-Options: nosniff",
        "Cache-Control: no-store",
    ] {
        assert!(response.contains(h), "{h}");
    }
    assert!(!response.contains("unsafe-inline") && !response.contains("unsafe-eval"));
    fs::remove_dir_all(base).unwrap();
}

/// Phase 34 (V5.12, V5.24, S11) through the bridge: picking an attachment returns only
/// id, name and size (never the content), the attachment goes in the save by id,
/// removing an attachment with a forged id is refused, and connect returns `null` (the
/// token does not come) and only works for a server entry. The picker folder has more
/// than one file: nothing is picked.
#[test]
fn attack_attachments_and_connect_through_the_bridge() {
    use ward_test_bridge::simulated::{DirPicker, TestTerminal};
    let base = dist("anexos34");
    let folder = base.join("anexos");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("deploy.pem"), "falso-anexo-ponte-34\n").unwrap();
    let service = Service::new(FakeDaemon::full(), Copier::new(FakeClipboard::default()))
        .with_picker(DirPicker(folder.clone()))
        .with_terminal(TestTerminal);
    let (p, _l) = Bridge::open(service, &base.join("dist")).unwrap();
    let r = p.respond(&post(&p, "escolher_anexo", "{}", &[]));
    let v = r.body_json().unwrap();
    assert_eq!(
        v["valor"],
        serde_json::json!({"resultado": "anexado", "id": 0, "nome": "deploy.pem", "tamanho": 21}),
        "{v}"
    );
    assert!(!String::from_utf8_lossy(&r.body).contains("falso-anexo"));
    let new_entry = serde_json::json!({ "formulario": {
        "tipo": "generic", "nome": "gama-anexo", "grupo": "gama", "ambiente": "dev",
        "projeto": "", "tags": [], "campos": {}, "segredos": {},
        "extras": [{"nome": "REGIAO", "segredo": false, "valor": {"novo": "r-01"}}],
        "anexos": [0]
    }});
    let r = p.respond(&post(&p, "salvar_credencial", &new_entry.to_string(), &[]));
    assert_eq!(r.body_json().unwrap()["valor"]["salvo"], "gama-anexo");
    let r = p.respond(&post(
        &p,
        "detalhe_credencial",
        r#"{"entrada":"gama-anexo"}"#,
        &[],
    ));
    assert!(!String::from_utf8_lossy(&r.body).contains("falso-anexo"));
    assert_eq!(r.body_json().unwrap()["valor"]["anexos"][0]["tamanho"], 21);
    for body in [r#"{"id":"0"}"#, r#"{"id":-1}"#, r#"{"id":99}"#, r#"{}"#] {
        let r = p.respond(&post(&p, "remover_anexo", body, &[]));
        assert_eq!(r.body_json().unwrap()["ok"], false, "{body}");
    }
    fs::write(folder.join("outro.txt"), "x").unwrap();
    let r = p.respond(&post(&p, "escolher_anexo", "{}", &[]));
    assert_eq!(r.body_json().unwrap()["ok"], false);

    let r = p.respond(&post(
        &p,
        "conectar",
        r#"{"entrada":"acme-ssh-salto"}"#,
        &[],
    ));
    assert_eq!(
        r.body_json().unwrap(),
        serde_json::json!({"ok": true, "valor": null})
    );
    for body in [
        r#"{"entrada":"acme-pg-aprovacao"}"#,
        r#"{"entrada":"-oProxyCommand=x"}"#,
        r#"{"entrada":5}"#,
    ] {
        let r = p.respond(&post(&p, "conectar", body, &[]));
        assert_eq!(r.body_json().unwrap()["ok"], false, "{body}");
    }
    fs::remove_dir_all(base).unwrap();
}

/// Phase 53a (S2, S11) through the bridge: loading the key returns only the size (never
/// the key nor the path); the key goes in the save as the "loaded" marker and the
/// daemon stores it; a field off the list, an extra argument or one of another type are
/// refused before the picker; text that is not a key gives the fixed notice; the
/// "loaded" marker with nothing loaded (or after saving, which discards it) is an
/// invalid request.
#[test]
fn attack_load_key_through_the_bridge() {
    use ward_test_bridge::simulated::DirPicker;
    let base = dist("chave53a");
    let folder = base.join("chave");
    fs::create_dir_all(&folder).unwrap();
    let key_body = "ZmFsc28tV1Atc2VuaGEtMDE=";
    let label = ["OPENSSH", " PRIVATE", " KEY"].concat();
    let key = format!("-----BEGIN {label}-----\n{key_body}\n-----END {label}-----\n");
    fs::write(folder.join("deploy_ed25519"), &key).unwrap();
    let service = Service::new(FakeDaemon::full(), Copier::new(FakeClipboard::default()))
        .with_picker(DirPicker(folder.clone()));
    let (p, _l) = Bridge::open(service, &base.join("dist")).unwrap();
    for body in [
        r#"{}"#,
        r#"{"campo":"password"}"#,
        r#"{"campo":"passphrase"}"#,
        r#"{"campo":5}"#,
        r#"{"campo":"private_key","caminho":"/etc/shadow"}"#,
    ] {
        let r = p.respond(&post(&p, "carregar_chave", body, &[]));
        assert_eq!(r.body_json().unwrap()["ok"], false, "{body}");
    }
    let r = p.respond(&post(
        &p,
        "carregar_chave",
        r#"{"campo":"private_key"}"#,
        &[],
    ));
    let body = String::from_utf8_lossy(&r.body).into_owned();
    assert!(
        !body.contains(key_body) && !body.contains("deploy_ed25519") && !body.contains('/'),
        "{body}"
    );
    assert_eq!(
        r.body_json().unwrap()["valor"],
        serde_json::json!({"resultado": "carregada", "tamanho": key.len()})
    );
    let new_entry = |name: &str| {
        serde_json::json!({ "formulario": {
            "tipo": "server", "nome": name, "grupo": "clientex", "ambiente": "stage",
            "projeto": "", "tags": [],
            "campos": {"host": "127.0.0.1", "port": "2223", "username": "deploy", "jump": ""},
            "segredos": {"password": "vazio", "private_key": "carregada", "passphrase": "vazio"},
            "extras": [], "anexos": []
        }})
        .to_string()
    };
    let r = p.respond(&post(
        &p,
        "salvar_credencial",
        &new_entry("clientex-ssh-53a"),
        &[],
    ));
    assert_eq!(
        r.body_json().unwrap()["valor"]["salvo"],
        "clientex-ssh-53a",
        "{}",
        String::from_utf8_lossy(&r.body)
    );
    let r = p.respond(&post(
        &p,
        "detalhe_credencial",
        r#"{"entrada":"clientex-ssh-53a"}"#,
        &[],
    ));
    assert!(!String::from_utf8_lossy(&r.body).contains(key_body));
    // Saved, the loaded key was discarded: the "loaded" marker again is an invalid
    // request.
    let r = p.respond(&post(
        &p,
        "salvar_credencial",
        &new_entry("clientex-ssh-53b"),
        &[],
    ));
    assert_eq!(r.body_json().unwrap()["ok"], false);
    // Text that is not a key: fixed notice, nothing stored.
    fs::write(folder.join("deploy_ed25519"), "falso-texto-53a\n").unwrap();
    let r = p.respond(&post(
        &p,
        "carregar_chave",
        r#"{"campo":"private_key"}"#,
        &[],
    ));
    assert!(!String::from_utf8_lossy(&r.body).contains("falso-texto"));
    assert_eq!(
        r.body_json().unwrap()["valor"]["resultado"],
        "aviso",
        "{}",
        String::from_utf8_lossy(&r.body)
    );
    let r = p.respond(&post(
        &p,
        "salvar_credencial",
        &new_entry("clientex-ssh-53b"),
        &[],
    ));
    assert_eq!(r.body_json().unwrap()["ok"], false);
    fs::remove_dir_all(base).unwrap();
}

/// Phase 30 (V5.7): first use through the bridge, with the fake daemon without a vault.
/// A PIN with 3 or 7 digits, with letters and a different confirmation come back as a
/// field error without going to the daemon; data with an extra field or off the format
/// are refused; no response carries the password or the PIN; once created, the vault is
/// unlocked and empty, and creating it again is refused.
#[test]
fn attack_first_use_through_the_bridge() {
    let base = dist("primeiro-uso");
    let service = Service::new(
        FakeDaemon::no_vault(),
        Copier::new(FakeClipboard::default()),
    );
    let (p, _l) = Bridge::open(service, &base.join("dist")).unwrap();
    let vault_status = || {
        p.respond(&post(&p, "estado_do_cofre", "{}", &[]))
            .body_json()
            .unwrap()["valor"]["estado"]
            .clone()
    };
    assert_eq!(vault_status(), "sem-cofre");
    let data = |password: &str, conf: &str, pin: &str, conf_pin: &str| {
        serde_json::json!({ "dados": {
            "senha": password, "confirmacao": conf, "pin": pin, "confirmacao_pin": conf_pin
        }})
        .to_string()
    };
    let password = "falso-mestra-ward-01";
    for (body, field) in [
        (data(password, password, "135", "135"), "pin"),
        (data(password, password, "1357901", "1357901"), "pin"),
        (data(password, password, "13579a", "13579a"), "pin"),
        (
            data(password, password, "135790", "135791"),
            "confirmacao_pin",
        ),
        (
            data(password, "falso-WP-senha-01", "135790", "135790"),
            "confirmacao",
        ),
        (
            data("falso-curta", "falso-curta", "135790", "135790"),
            "senha",
        ),
    ] {
        let r = p.respond(&post(&p, "criar_cofre", &body, &[]));
        let text = String::from_utf8_lossy(&r.body).into_owned();
        let v = r.body_json().unwrap();
        assert_eq!(v["valor"]["criado"], false, "{v}");
        assert_eq!(v["valor"]["erros"][0]["campo"], field, "{v}");
        for needle in ["falso-", "1357"] {
            assert!(!text.contains(needle), "{text}");
        }
    }
    assert_eq!(vault_status(), "sem-cofre");
    for body in [
        r#"{"dados":{"senha":"falso-mestra-ward-01","confirmacao":"falso-mestra-ward-01","pin":"135790","confirmacao_pin":"135790","nivel":"free"}}"#,
        r#"{"dados":{"senha":"falso-mestra-ward-01","confirmacao":"falso-mestra-ward-01","pin":135790,"confirmacao_pin":"135790"}}"#,
        r#"{"senha":"falso-mestra-ward-01","pin":"135790"}"#,
        r#"{"dados":"falso-mestra-ward-01"}"#,
    ] {
        let r = p.respond(&post(&p, "criar_cofre", body, &[]));
        let v = r.body_json().unwrap();
        assert_eq!(v["codigo"], "pedido-invalido", "{v}");
        assert!(!String::from_utf8_lossy(&r.body).contains("falso-"));
    }
    let r = p.respond(&post(
        &p,
        "criar_cofre",
        &data(password, password, "135790", "135790"),
        &[],
    ));
    assert_eq!(r.body_json().unwrap()["valor"]["criado"], true);
    assert!(!String::from_utf8_lossy(&r.body).contains("falso-"));
    assert_eq!(vault_status(), "destravado");
    let r = p.respond(&post(&p, "listar_credenciais", "{}", &[]));
    assert_eq!(r.body_json().unwrap()["valor"], serde_json::json!([]));
    let r = p.respond(&post(
        &p,
        "criar_cofre",
        &data(password, password, "2468", "2468"),
        &[],
    ));
    assert_eq!(r.body_json().unwrap()["codigo"], "cofre-ja-existe");
    fs::remove_dir_all(base).unwrap();
}

/// V3.37/V3.38 (D63): the bridge run by hand (or a copy, or with another parent) gets the
/// agent role from the test daemon and exits on the first reply, without opening the
/// HTTP port; with `app`, it goes on. The "daemon" here is a server of the same uid in a
/// 0700 folder, which answers the status request with the asked role and records what it
/// received.
#[test]
fn attack_bridge_without_app_role_exits_on_first_reply() {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    use std::os::unix::net::UnixListener;
    use ward_app_core::daemon::DaemonSocket;
    use ward_test_bridge::startup::{NO_APP_ROLE, connect_with_app_role};

    let uid = fs::metadata("/proc/self").unwrap().uid();
    for role in ["agent", "extension", "app"] {
        let dir = PathBuf::from(format!(
            "/run/user/{uid}/ward-bridge-role-{}-{role}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let sock = dir.join("wardd.sock");
        let l = UnixListener::bind(&sock).unwrap();
        fs::set_permissions(&sock, fs::Permissions::from_mode(0o600)).unwrap();
        let server = std::thread::spawn(move || {
            let (mut c, _) = l.accept().unwrap();
            let mut received = Vec::new();
            loop {
                let mut n = [0u8; 4];
                if c.read_exact(&mut n).is_err() {
                    break;
                }
                let mut b = vec![0u8; u32::from_be_bytes(n) as usize];
                c.read_exact(&mut b).unwrap();
                received.push(String::from_utf8(b).unwrap());
                let r = format!(
                    r#"{{"v":2,"type":"status","data":{{"locked":true,"vault_exists":true,"role":"{role}"}}}}"#
                );
                c.write_all(&(r.len() as u32).to_be_bytes()).unwrap();
                c.write_all(r.as_bytes()).unwrap();
            }
            received
        });
        let r = connect_with_app_role(DaemonSocket::test_at(sock));
        if role == "app" {
            assert!(r.is_ok(), "{:?}", r.err());
        } else {
            assert_eq!(r.as_ref().err().map(String::as_str), Some(NO_APP_ROLE));
        }
        drop(r);
        let received = server.join().unwrap();
        assert_eq!(received.len(), 1, "{received:?}");
        assert!(received[0].contains(r#""op":"status""#), "{received:?}");
        let _ = fs::remove_dir_all(&dir);
    }
}

/// Phase 31 (S13, D20): unlock through the bridge. The password leaves the body as the
/// master password type; the wrong format does not even reach the daemon; the response
/// neither echoes the password nor says how many attempts there were; the locked vault
/// status carries only the request (here `null`).
#[test]
fn attack_unlock_through_the_bridge() {
    let (p, _l, _a, base) = bridge("destravar");
    let vault_status = |p: &Bridge<FakeDaemon, FakeClipboard>| {
        p.respond(&post(p, "estado_do_cofre", "{}", &[]))
            .body_json()
            .unwrap()["valor"]
            .clone()
    };
    // Locks through the 5th wrong PIN (Phase 35), as in the app.
    for _ in 0..5 {
        p.respond(&post(
            &p,
            "mudar_nivel",
            r#"{"entrada":"acme-pg-prod","nivel":"free","pin":"000000"}"#,
            &[],
        ));
    }
    assert_eq!(
        vault_status(&p),
        serde_json::json!({"estado": "travado", "pedido": null})
    );
    let unlock = |p: &Bridge<FakeDaemon, FakeClipboard>, body: &str| {
        let r = p.respond(&post(p, "destravar", body, &[]));
        let text = String::from_utf8_lossy(&r.body).into_owned();
        assert!(!text.contains("falso-mestra"), "senha na resposta: {text}");
        r.body_json().unwrap()
    };
    for body in [
        r#"{}"#,
        r#"{"senha":""}"#,
        r#"{"senha":135790}"#,
        r#"{"senha":null}"#,
        r#"{"senha":["falso-mestra-ward-01"]}"#,
        r#"{"senha":"falso-mestra-ward-01","extra":1}"#,
    ] {
        let v = unlock(&p, body);
        assert_eq!(v["codigo"], "pedido-invalido", "{body}: {v}");
    }
    assert_eq!(vault_status(&p)["estado"], "travado");
    let v = unlock(&p, r#"{"senha":"falso-mestra-ward-00"}"#);
    assert_eq!(v["codigo"], "senha-incorreta", "{v}");
    let msg = v["mensagem"].as_str().unwrap().to_owned();
    assert!(!msg.chars().any(|c| c.is_ascii_digit()), "{msg}");
    assert_eq!(vault_status(&p)["estado"], "travado");
    // The 2nd attempt waits 1 s in the daemon (D20) and unlocks.
    let v = unlock(&p, r#"{"senha":"falso-mestra-ward-01"}"#);
    assert_eq!(v, serde_json::json!({"ok": true, "valor": null}));
    assert_eq!(
        vault_status(&p),
        serde_json::json!({"estado": "destravado", "pedido": null})
    );
    // The notice-listening command does not exist in the bridge (the channel is Tauri's).
    let r = p.respond(&post(&p, "ouvir_avisos", "{}", &[]));
    assert_eq!(r.body_json().unwrap()["codigo"], "pedido-invalido");
    let _ = fs::remove_dir_all(base);
}

/// Phase 39 (V5.23, S12): "lock now" through the bridge. Any argument is refused; the
/// lock clears the clipboard that still has the copied value and leaves the vault locked
/// (reveal answers the locked code); the response carries no value.
#[test]
fn attack_lock_through_the_bridge() {
    let (p, _l, clipboard, base) = bridge("travar");
    let r = p.respond(&post(&p, "travar", r#"{"x":1}"#, &[]));
    assert_eq!(r.body_json().unwrap()["codigo"], "pedido-invalido");
    let vault_status = |p: &Bridge<FakeDaemon, FakeClipboard>| {
        p.respond(&post(p, "estado_do_cofre", "{}", &[]))
            .body_json()
            .unwrap()["valor"]["estado"]
            .clone()
    };
    assert_eq!(vault_status(&p), "destravado");
    p.respond(&post(
        &p,
        "copiar_campo",
        r#"{"entrada":"acme-pg-dev","campo":"password"}"#,
        &[],
    ));
    let copied = clipboard.0.lock().unwrap().is_some();
    let r = p.respond(&post(&p, "travar", "{}", &[]));
    let text = String::from_utf8_lossy(&r.body).into_owned();
    assert_eq!(
        r.body_json().unwrap(),
        serde_json::json!({"ok": true, "valor": null})
    );
    assert!(!text.contains("falso-"), "{text}");
    if copied {
        assert!(clipboard.0.lock().unwrap().is_none(), "área não foi limpa");
    }
    assert_eq!(vault_status(&p), "travado");
    let r = p.respond(&post(
        &p,
        "revelar_campo",
        r#"{"entrada":"acme-pg-dev","campo":"password"}"#,
        &[],
    ));
    assert_eq!(r.body_json().unwrap()["codigo"], "travado");
    let _ = fs::remove_dir_all(base);
}

/// Phase 38 (V5.22, S9): the settings screen through the bridge. Default 4 h and backup
/// disabled; a timeout outside 1 to 1440, an extra field and a wrong type refused;
/// changing the PIN asks for the current one and changing the password asks for the
/// current one; no response carries a PIN, a password or a hash.
#[test]
fn attack_settings_through_the_bridge() {
    let base = dist("configuracoes");
    let service = Service::new(FakeDaemon::full(), Copier::new(FakeClipboard::default()));
    let (p, _l) = Bridge::open(service, &base.join("dist")).unwrap();
    let ask = |command: &str, body: &str| {
        let r = p.respond(&post(&p, command, body, &[]));
        let text = String::from_utf8_lossy(&r.body).into_owned();
        for needle in ["falso-", "1357", "2468", "argon2", "$m="] {
            assert!(!text.contains(needle), "{command}: {text}");
        }
        r.body_json().unwrap()
    };
    let v = ask("configuracoes", "{}");
    assert_eq!(v["valor"]["minutos_trava"], 240, "{v}");
    assert_eq!(v["valor"]["backup_externo"], false, "{v}");
    assert_eq!(
        ask("configuracoes", r#"{"hash":1}"#)["codigo"],
        "pedido-invalido"
    );
    let v = ask("definir_tempo_trava", r#"{"minutos":60}"#);
    assert_eq!(v["valor"]["minutos_trava"], 60, "{v}");
    for body in [
        r#"{"minutos":0}"#,
        r#"{"minutos":1441}"#,
        r#"{"minutos":"60"}"#,
        r#"{"minutos":-1}"#,
        r#"{"minutos":60,"pin":"135790"}"#,
        r#"{}"#,
    ] {
        assert_eq!(
            ask("definir_tempo_trava", body)["codigo"],
            "pedido-invalido",
            "{body}"
        );
    }
    assert_eq!(ask("configuracoes", "{}")["valor"]["minutos_trava"], 60);
    let pin_change = |current: &str, new_pin: &str, conf: &str| {
        serde_json::json!({ "dados": { "atual": current, "novo": new_pin, "confirmacao": conf } })
            .to_string()
    };
    for (body, field) in [
        (pin_change("135", "2468", "2468"), "atual"),
        (pin_change("135790", "13579a", "13579a"), "novo"),
        (pin_change("135790", "2468", "2469"), "confirmacao"),
    ] {
        let v = ask("trocar_pin", &body);
        assert_eq!(v["valor"]["trocado"], false, "{v}");
        assert_eq!(v["valor"]["erros"][0]["campo"], field, "{v}");
    }
    for body in [
        r#"{"dados":{"atual":"135790","novo":"2468","confirmacao":"2468","nivel":"free"}}"#,
        r#"{"dados":{"atual":135790,"novo":"2468","confirmacao":"2468"}}"#,
        r#"{"dados":{"atual":"135790","novo":"2468","confirmacao":"2468"},"x":1}"#,
        r#"{"atual":"135790","novo":"2468"}"#,
    ] {
        assert_eq!(
            ask("trocar_pin", body)["codigo"],
            "pedido-invalido",
            "{body}"
        );
    }
    assert_eq!(
        ask("trocar_pin", &pin_change("246801", "2468", "2468"))["codigo"],
        "pin-incorreto"
    );
    assert_eq!(
        ask("trocar_pin", &pin_change("135790", "2468", "2468"))["valor"]["trocado"],
        true
    );
    let password_change = |current: &str, new_password: &str| {
        serde_json::json!({ "dados": { "atual": current, "nova": new_password, "confirmacao": new_password } })
            .to_string()
    };
    let v = ask(
        "trocar_senha_mestra",
        &password_change("falso-mestra-ward-01", "falso-curta"),
    );
    assert_eq!(v["valor"]["erros"][0]["campo"], "nova", "{v}");
    assert_eq!(
        ask(
            "trocar_senha_mestra",
            &password_change("falso-errada-mestra-09", "falso-nova-mestra-02")
        )["codigo"],
        "senha-incorreta"
    );
    assert_eq!(
        ask(
            "trocar_senha_mestra",
            &password_change("falso-mestra-ward-01", "falso-nova-mestra-02")
        )["valor"]["trocado"],
        true
    );
    let _ = fs::remove_dir_all(base);
}

/// Phase 48 (V7.9): external backup through the bridge. Enable with a USB drive of the
/// list, disable keeping the UUID; a forged UUID (off the format or off the list), a
/// wrong type and an extra field refused; no response carries a mount path; the odd label
/// comes with the invisible character escaped.
#[test]
fn attack_external_backup_through_the_bridge() {
    let base = dist("backup-externo");
    let service = Service::new(FakeDaemon::full(), Copier::new(FakeClipboard::default()));
    let (p, _l) = Bridge::open(service, &base.join("dist")).unwrap();
    let ask = |command: &str, body: &str| {
        let r = p.respond(&post(&p, command, body, &[]));
        let text = String::from_utf8_lossy(&r.body).into_owned();
        for needle in ["/media", "/run/", "montagem", "\u{202E}", "\u{200B}"] {
            assert!(!text.contains(needle), "{command}: {text}");
        }
        r.body_json().unwrap()
    };
    let v = ask("backup_externo", "{}");
    assert_eq!(v["valor"]["ligado"], false, "{v}");
    assert!(v["valor"]["uuid"].is_null(), "{v}");
    let v = ask("listar_pendrives", "{}");
    let list = v["valor"].as_array().unwrap();
    assert_eq!(list.len(), 4, "{v}");
    assert!(
        list.iter()
            .any(|p| p["rotulo"].as_str().unwrap().contains("\\u{202E}")),
        "{v}"
    );
    for (command, body) in [
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
            ask(command, body)["codigo"],
            "pedido-invalido",
            "{command} {body}"
        );
    }
    assert_eq!(
        ask(
            "definir_backup_externo",
            r#"{"ligado":true,"uuid":"DEAD-BEEF"}"#
        )["codigo"],
        "pendrive-nao-conectado"
    );
    assert_eq!(ask("backup_externo", "{}")["valor"]["ligado"], false);
    let v = ask(
        "definir_backup_externo",
        r#"{"ligado":true,"uuid":"7C1E-42A9"}"#,
    );
    assert_eq!(v["valor"]["ligado"], true, "{v}");
    assert_eq!(v["valor"]["pendrive"]["rotulo"], "WARD-BACKUP", "{v}");
    assert_eq!(v["valor"]["pendrive"]["montado"], true, "{v}");
    assert_eq!(ask("configuracoes", "{}")["valor"]["backup_externo"], true);
    let v = ask("definir_backup_externo", r#"{"ligado":false,"uuid":null}"#);
    assert_eq!(v["valor"]["ligado"], false, "{v}");
    assert_eq!(v["valor"]["uuid"], "7C1E-42A9", "{v}");
    let _ = fs::remove_dir_all(base);
}

/// Phase 47 (V7.7): restore backup through the bridge. The list carries only name,
/// instant and (Phase 49a) the verified/out-of-order marks; a forged version (path, `..`,
/// off the list), a wrong PIN, a wrong version password, a wrong type and an extra field
/// change nothing; no response carries a secret, PIN, password, path or generation; the
/// right restore swaps the entries for the version ones.
#[test]
fn attack_restore_backup_through_the_bridge() {
    let base = dist("restaurar-backup");
    let service = Service::new(FakeDaemon::full(), Copier::new(FakeClipboard::default()));
    let (p, _l) = Bridge::open(service, &base.join("dist")).unwrap();
    let ask = |command: &str, body: &str| {
        let r = p.respond(&post(&p, command, body, &[]));
        let text = String::from_utf8_lossy(&r.body).into_owned();
        for s in SECRETS {
            assert!(!text.contains(s), "{command}: segredo na resposta");
        }
        for needle in [
            "135790",
            "falso-mestra-ward-01",
            "/home",
            ".local",
            "backups/",
            "geracao",
        ] {
            assert!(!text.contains(needle), "{command}: {needle} em {text}");
        }
        r.body_json().unwrap()
    };
    let body = |version: &str, password: &str, pin: &str| {
        serde_json::json!({ "dados": { "versao": version, "senha": password, "pin": pin } })
            .to_string()
    };
    let entry_count = || {
        ask("listar_credenciais", "{}")["valor"]
            .as_array()
            .unwrap()
            .len()
    };
    let before = entry_count();
    let v = ask("listar_backups", "{}");
    let list = v["valor"].as_array().unwrap().clone();
    assert_eq!(list.len(), 30, "{v}");
    for item in &list {
        let keys: Vec<_> = item.as_object().unwrap().keys().cloned().collect();
        assert_eq!(
            keys,
            ["conferida", "fora_de_ordem", "instante", "nome"],
            "{item}"
        );
    }
    let target = list[4]["nome"].as_str().unwrap().to_owned();
    assert_eq!(
        ask("listar_backups", r#"{"todos":true}"#)["codigo"],
        "pedido-invalido"
    );
    // Format, type and extra field: refused before the daemon.
    let extra_field = serde_json::json!({ "dados": {
        "versao": target, "senha": "falso-mestra-ward-01", "pin": "135790", "geracao": 9 } });
    let numeric_pin = serde_json::json!({ "dados": {
        "versao": target, "senha": "falso-mestra-ward-01", "pin": 135790 } });
    let outside = serde_json::json!({ "dados": {
        "versao": target, "senha": "falso-mestra-ward-01", "pin": "135790" }, "a_mais": 1 });
    for c in [
        body("../cofre.ward", "falso-mestra-ward-01", "135790"),
        body(
            "/home/usuario/.local/share/ward/cofre.ward",
            "falso-mestra-ward-01",
            "135790",
        ),
        body(&target, "falso-mestra-ward-01", "12ab"),
        body(&target, "", "135790"),
        extra_field.to_string(),
        numeric_pin.to_string(),
        outside.to_string(),
        r#"{"versao":"ward-20261108-143200.ward"}"#.to_owned(),
    ] {
        assert_eq!(
            ask("restaurar_backup", &c)["codigo"],
            "pedido-invalido",
            "{c}"
        );
    }
    // Version in the format, but missing.
    let missing = body(
        "ward-19990101-000000.ward",
        "falso-mestra-ward-01",
        "135790",
    );
    assert_eq!(
        ask("restaurar_backup", &missing)["codigo"],
        "backup-inexistente"
    );
    // Wrong PIN and wrong version password.
    let wrong_pin = body(&target, "falso-mestra-ward-01", "000000");
    assert_eq!(
        ask("restaurar_backup", &wrong_pin)["codigo"],
        "pin-incorreto"
    );
    let wrong_password = body(&target, "falso-mestra-ward-09", "135790");
    assert_eq!(
        ask("restaurar_backup", &wrong_password)["codigo"],
        "senha-incorreta"
    );
    assert_eq!(entry_count(), before);
    // The right one: version 4 has four entries fewer.
    let v = ask(
        "restaurar_backup",
        &body(&target, "falso-mestra-ward-01", "135790"),
    );
    assert_eq!(v["valor"]["restaurado"], true, "{v}");
    assert_eq!(v["valor"]["entradas"], before - 4, "{v}");
    assert_eq!(entry_count(), before - 4);
    let _ = fs::remove_dir_all(base);
}
