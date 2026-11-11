//! S11 (V5.1, V5.2, V5.6): a configuração do app Tauri e da interface, conferida no
//! `make check` (o crate do Tauri não compila sem o WebKitGTK de desenvolvimento).
//! Cada teste é um ataque de configuração: uma permissão a mais, uma CSP frouxa, um
//! armazenamento no navegador ou a ponte vazando para o build de produção.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use ward_app_core::commands::{COMMANDS, permission};

fn app() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn json(caminho: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(caminho).unwrap()).unwrap()
}

fn conf() -> Value {
    json(&app().join("src-tauri/tauri.conf.json"))
}

/// Fase 37: os limites que a tela do gerador mostra ("De 8 a 64", padrão 24) são os que o
/// Rust aceita.
#[test]
fn limites_do_gerador_na_interface_sao_os_do_rust() {
    use ward_app_core::generator::{MAX_LENGTH, MIN_LENGTH, DEFAULT_LENGTH};
    let js = fs::read_to_string(app().join("src/screens/PasswordGenerator.svelte")).unwrap();
    for (nome, valor) in [
        ("MIN_LENGTH", MIN_LENGTH),
        ("MAX_LENGTH", MAX_LENGTH),
        ("DEFAULT_LENGTH", DEFAULT_LENGTH),
    ] {
        assert!(
            js.contains(&format!("const {nome} = {valor};")),
            "{nome} diverge do Rust"
        );
    }
}

/// Fase 36: a janela passa a `Aprovação / Expirando` no mesmo limiar do Rust, e a tela
/// nunca põe o texto do pedido como HTML.
#[test]
fn janela_de_aprovacao_segue_o_rust_e_nao_usa_html() {
    use ward_app_core::approval::URGENT_SECONDS;
    let js = fs::read_to_string(app().join("src/screens/approval.js")).unwrap();
    assert!(
        js.contains(&format!("export const URGENT_SECONDS = {URGENT_SECONDS};")),
        "URGENT_SECONDS diverge do Rust"
    );
    let tela = fs::read_to_string(app().join("src/screens/ApprovalWindow.svelte")).unwrap();
    assert!(tela.contains("{request.comando}"), "comando como texto");
    assert!(tela.contains("{process}"), "cadeia como texto");
    assert!(tela.contains("{entry.nome}"), "entrada como texto");
    // Fase 44c (D9): a origem `http` também só como texto.
    assert!(tela.contains("{request.origem}"), "origem como texto");
    assert!(
        js.contains("return decision !== 'negar' && decision !== 'confirmar';"),
        "só negar e confirmar sem PIN"
    );
}

/// S11, Fase 36: duas capabilities, uma por janela; em comum só o `ouvir_avisos` (canal
/// por janela, aviso sem dado). A janela principal não responde aprovação; a de
/// aprovação só ouve o aviso, lê o pedido e responde.
#[test]
fn ataque_capability_com_permissao_a_mais() {
    use ward_app_core::commands::{APPROVAL_COMMANDS, MAIN_COMMANDS, SHARED_COMMANDS};
    let dir = app().join("src-tauri/capabilities");
    let mut arquivos: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    arquivos.sort();
    assert_eq!(
        arquivos,
        vec!["aprovacao.json", "principal.json"],
        "só as duas capabilities"
    );
    for (arquivo, janela, comandos) in [
        ("principal.json", "principal", MAIN_COMMANDS),
        ("aprovacao.json", "aprovacao", APPROVAL_COMMANDS),
    ] {
        let cap = json(&dir.join(arquivo));
        let esperadas: Vec<Value> = comandos
            .iter()
            .map(|c| Value::from(permission(c)))
            .collect();
        assert_eq!(
            cap["permissions"],
            Value::from(esperadas),
            "{arquivo}: só allow-<comando próprio da janela>"
        );
        assert_eq!(cap["identifier"], janela);
        assert_eq!(cap["windows"], serde_json::json!([janela]));
        assert_eq!(cap["local"], Value::Bool(true));
        assert!(cap.get("remote").is_none(), "nenhuma origem remota");
        assert!(cap.get("webviews").is_none());
        assert!(cap.get("platforms").is_none());
        for p in cap["permissions"].as_array().unwrap() {
            assert!(
                !p.as_str().unwrap().contains(':'),
                "permissão de plugin ou do núcleo: {p}"
            );
        }
    }
    assert_eq!(SHARED_COMMANDS, &["ouvir_avisos"]);
    let principal = json(&dir.join("principal.json"));
    for c in APPROVAL_COMMANDS
        .iter()
        .filter(|c| !SHARED_COMMANDS.contains(c))
    {
        assert!(
            !principal["permissions"]
                .as_array()
                .unwrap()
                .contains(&Value::from(permission(c))),
            "a janela principal não pode {c}"
        );
    }
    assert_eq!(
        conf()["app"]["security"]["capabilities"],
        serde_json::json!(["principal", "aprovacao"])
    );
}

#[test]
fn ataque_csp_frouxa_ou_conteudo_remoto() {
    let c = conf();
    let seg = &c["app"]["security"];
    let csp = seg["csp"].as_object().expect("CSP por diretiva");
    assert_eq!(csp["default-src"], "'none'");
    for d in ["script-src", "style-src", "img-src", "font-src"] {
        assert_eq!(csp[d], "'self'", "{d}");
    }
    for d in [
        "base-uri",
        "form-action",
        "frame-src",
        "worker-src",
        "object-src",
        "media-src",
        "manifest-src",
    ] {
        assert_eq!(csp[d], "'none'", "{d}");
    }
    assert_eq!(
        csp["connect-src"], "ipc: http://ipc.localhost",
        "só a IPC do Tauri"
    );
    for (d, v) in csp {
        let v = v.as_str().unwrap();
        for proibido in [
            "unsafe-inline",
            "unsafe-eval",
            "unsafe-hashes",
            "wasm-unsafe-eval",
            "*",
            "data:",
            "blob:",
            "https:",
            "http:",
        ] {
            let achou = v
                .split_whitespace()
                .any(|t| t.contains(proibido) && t != "http://ipc.localhost");
            assert!(!achou, "{d} contém {proibido}");
        }
    }
    assert!(seg.get("devCsp").is_none(), "sem CSP de desenvolvimento");
    assert_eq!(seg["freezePrototype"], true);
    assert_eq!(seg["dangerousDisableAssetCspModification"], false);
    assert_eq!(seg["assetProtocol"]["enable"], false);
    assert!(seg.get("dangerousRemoteDomainIpcAccess").is_none());
    assert_eq!(c["app"]["withGlobalTauri"], false);
    assert!(
        c["build"].get("devUrl").is_none(),
        "interface só embutida, nunca de servidor"
    );
    assert!(c["build"].get("beforeDevCommand").is_none());
    assert_eq!(c["plugins"], serde_json::json!({}), "nenhum plugin");
    assert_eq!(
        c["app"]["windows"],
        serde_json::json!([]),
        "janela criada no código, com navegação restrita"
    );
}

#[test]
fn ataque_devtools_ou_acl_dinamica_no_release() {
    let toml = fs::read_to_string(app().join("src-tauri/Cargo.toml")).unwrap();
    let linha_tauri = toml.lines().find(|l| l.starts_with("tauri = ")).unwrap();
    assert!(
        linha_tauri.contains("default-features = false"),
        "sem as features padrão (dynamic-acl)"
    );
    for f in [
        "devtools",
        "dynamic-acl",
        "protocol-asset",
        "webview-data-url",
        "image-png",
        "image-ico",
        "test",
    ] {
        assert!(
            !linha_tauri.contains(&format!("\"{f}\"")),
            "feature {f} no build normal"
        );
    }
    // Fase 39: a bandeja (`tray-icon`, AppIndicator) entrou; a lista de features é
    // exatamente esta, nada mais.
    assert!(
        linha_tauri
            .contains(r#"features = ["wry", "compression", "custom-protocol", "tray-icon"] }"#),
        "features do tauri fora da lista: {linha_tauri}"
    );
    let modo = toml
        .lines()
        .find(|l| l.starts_with("test-mode = "))
        .unwrap();
    assert!(modo.contains("tauri/devtools"), "devtools só com test-mode");
    assert_eq!(
        toml.matches("devtools").count(),
        2,
        "devtools só no comentário e na feature test-mode"
    );
    let main = fs::read_to_string(app().join("src-tauri/src/main.rs")).unwrap();
    assert!(main.contains(".devtools(cfg!(feature = \"test-mode\"))"));
    assert!(main.contains(".on_navigation(navegacao_permitida)"));
    assert!(!main.contains("WARD_DEV_DIR"));
}

#[test]
fn lista_de_comandos_da_interface_e_a_mesma() {
    let js = fs::read_to_string(app().join("src/lib/bridge/commands.js")).unwrap();
    let lista: String = COMMANDS
        .iter()
        .map(|c| format!("'{c}'"))
        .collect::<Vec<_>>()
        .join(", ");
    assert!(
        js.contains(&format!("Object.freeze([{lista}])")),
        "commands.js diverge de COMMANDS"
    );
}

fn fontes(dir: &Path, saida: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            fontes(&p, saida);
        } else if matches!(
            p.extension().and_then(|e| e.to_str()),
            Some("js" | "svelte" | "html")
        ) {
            saida.push(p);
        }
    }
}

#[test]
fn ataque_interface_guarda_ou_injeta_conteudo() {
    let mut arquivos = Vec::new();
    fontes(&app().join("src"), &mut arquivos);
    arquivos.push(app().join("index.html"));
    assert!(arquivos.len() > 10);
    for a in &arquivos {
        let t = fs::read_to_string(a).unwrap();
        for proibido in [
            "localStorage",
            "sessionStorage",
            "indexedDB",
            "document.cookie",
            "innerHTML",
            "outerHTML",
            "{@html",
            "eval(",
            "new Function",
            "postMessage",
            "window.open",
            "style=\"",
            "http://",
            "https://",
        ] {
            assert!(!t.contains(proibido), "{} contém {proibido}", a.display());
        }
        // Script inline no HTML (no .svelte, `<script>` é o bloco do componente).
        if a.extension().is_some_and(|e| e == "html") {
            assert!(
                !t.contains("<script>") && !t.contains("<style"),
                "{} com script ou estilo inline",
                a.display()
            );
        }
    }
}
