//! S11 (V5.1, V5.2, V5.6): the configuration of the Tauri app and of the interface,
//! checked in `make check` (the Tauri crate does not build without the development
//! WebKitGTK). Each test is a configuration attack: one permission too many, a loose CSP,
//! a browser storage or the test bridge leaking into the production build.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use ward_app_core::commands::{COMMANDS, permission};

fn app() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn conf() -> Value {
    json(&app().join("src-tauri/tauri.conf.json"))
}

/// Phase 37: the limits that the generator screen shows (from 8 to 64, default 24) are
/// the ones Rust accepts.
#[test]
fn generator_limits_in_the_interface_are_the_rust_ones() {
    use ward_app_core::generator::{DEFAULT_LENGTH, MAX_LENGTH, MIN_LENGTH};
    let js = fs::read_to_string(app().join("src/screens/PasswordGenerator.svelte")).unwrap();
    for (name, value) in [
        ("MIN_LENGTH", MIN_LENGTH),
        ("MAX_LENGTH", MAX_LENGTH),
        ("DEFAULT_LENGTH", DEFAULT_LENGTH),
    ] {
        assert!(
            js.contains(&format!("const {name} = {value};")),
            "{name} diverge do Rust"
        );
    }
}

/// Phase 36: the window turns into its expiring state at the same threshold as Rust, and
/// the screen never puts the request text as HTML.
#[test]
fn approval_window_follows_rust_and_does_not_use_html() {
    use ward_app_core::approval::URGENT_SECONDS;
    let js = fs::read_to_string(app().join("src/screens/approval.js")).unwrap();
    assert!(
        js.contains(&format!("export const URGENT_SECONDS = {URGENT_SECONDS};")),
        "URGENT_SECONDS diverge do Rust"
    );
    let screen = fs::read_to_string(app().join("src/screens/ApprovalWindow.svelte")).unwrap();
    assert!(screen.contains("{request.comando}"), "comando como texto");
    assert!(screen.contains("{process}"), "cadeia como texto");
    assert!(screen.contains("{entry.nome}"), "entrada como texto");
    // Phase 44c (D9): the `http` origin also only as text.
    assert!(screen.contains("{request.origem}"), "origem como texto");
    assert!(
        js.contains("return decision !== 'negar' && decision !== 'confirmar';"),
        "só negar e confirmar sem PIN"
    );
}

/// S11, Phase 36: two capabilities, one per window; the only shared one is the notice
/// listener (a channel per window, a notice without data). The main window does not
/// answer an approval; the approval one only listens to the notice, reads the request and
/// answers.
#[test]
fn attack_capability_with_extra_permission() {
    use ward_app_core::commands::{APPROVAL_COMMANDS, MAIN_COMMANDS, SHARED_COMMANDS};
    let dir = app().join("src-tauri/capabilities");
    let mut files: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    files.sort();
    assert_eq!(
        files,
        vec!["aprovacao.json", "principal.json"],
        "só as duas capabilities"
    );
    for (file, window, commands) in [
        ("principal.json", "principal", MAIN_COMMANDS),
        ("aprovacao.json", "aprovacao", APPROVAL_COMMANDS),
    ] {
        let cap = json(&dir.join(file));
        let expected: Vec<Value> = commands
            .iter()
            .map(|c| Value::from(permission(c)))
            .collect();
        assert_eq!(
            cap["permissions"],
            Value::from(expected),
            "{file}: só allow-<comando próprio da janela>"
        );
        assert_eq!(cap["identifier"], window);
        assert_eq!(cap["windows"], serde_json::json!([window]));
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
    let main = json(&dir.join("principal.json"));
    for c in APPROVAL_COMMANDS
        .iter()
        .filter(|c| !SHARED_COMMANDS.contains(c))
    {
        assert!(
            !main["permissions"]
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
fn attack_loose_csp_or_remote_content() {
    let c = conf();
    let sec = &c["app"]["security"];
    let csp = sec["csp"].as_object().expect("CSP por diretiva");
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
        for forbidden in [
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
            let found = v
                .split_whitespace()
                .any(|t| t.contains(forbidden) && t != "http://ipc.localhost");
            assert!(!found, "{d} contém {forbidden}");
        }
    }
    assert!(sec.get("devCsp").is_none(), "sem CSP de desenvolvimento");
    assert_eq!(sec["freezePrototype"], true);
    assert_eq!(sec["dangerousDisableAssetCspModification"], false);
    assert_eq!(sec["assetProtocol"]["enable"], false);
    assert!(sec.get("dangerousRemoteDomainIpcAccess").is_none());
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
fn attack_devtools_or_dynamic_acl_in_release() {
    let toml = fs::read_to_string(app().join("src-tauri/Cargo.toml")).unwrap();
    let tauri_line = toml.lines().find(|l| l.starts_with("tauri = ")).unwrap();
    assert!(
        tauri_line.contains("default-features = false"),
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
            !tauri_line.contains(&format!("\"{f}\"")),
            "feature {f} no build normal"
        );
    }
    // Phase 39: the tray (`tray-icon`, AppIndicator) came in; the feature list is exactly
    // this one, nothing more.
    assert!(
        tauri_line
            .contains(r#"features = ["wry", "compression", "custom-protocol", "tray-icon"] }"#),
        "features do tauri fora da lista: {tauri_line}"
    );
    let mode_line = toml
        .lines()
        .find(|l| l.starts_with("test-mode = "))
        .unwrap();
    assert!(
        mode_line.contains("tauri/devtools"),
        "devtools só com test-mode"
    );
    assert_eq!(
        toml.matches("devtools").count(),
        2,
        "devtools só no comentário e na feature test-mode"
    );
    let main = fs::read_to_string(app().join("src-tauri/src/main.rs")).unwrap();
    assert!(main.contains(".devtools(cfg!(feature = \"test-mode\"))"));
    assert!(main.contains(".on_navigation(navigation_allowed)"));
    assert!(!main.contains("WARD_DEV_DIR"));
}

#[test]
fn interface_command_list_is_the_same() {
    let js = fs::read_to_string(app().join("src/lib/bridge/commands.js")).unwrap();
    let list: String = COMMANDS
        .iter()
        .map(|c| format!("'{c}'"))
        .collect::<Vec<_>>()
        .join(", ");
    assert!(
        js.contains(&format!("Object.freeze([{list}])")),
        "commands.js diverge de COMMANDS"
    );
}

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            sources(&p, out);
        } else if matches!(
            p.extension().and_then(|e| e.to_str()),
            Some("js" | "svelte" | "html")
        ) {
            out.push(p);
        }
    }
}

#[test]
fn attack_interface_stores_or_injects_content() {
    let mut files = Vec::new();
    sources(&app().join("src"), &mut files);
    files.push(app().join("index.html"));
    assert!(files.len() > 10);
    for a in &files {
        let t = fs::read_to_string(a).unwrap();
        for forbidden in [
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
            assert!(!t.contains(forbidden), "{} contém {forbidden}", a.display());
        }
        // Inline script in the HTML (in the .svelte, `<script>` is the component block).
        if a.extension().is_some_and(|e| e == "html") {
            assert!(
                !t.contains("<script>") && !t.contains("<style"),
                "{} com script ou estilo inline",
                a.display()
            );
        }
    }
}
