//! App desktop do Ward (Tauri 2). Só a janela e a ligação dos comandos: o que cada
//! comando faz está em `ward_app_nucleo::servico` (o mesmo que a ponte de teste usa).
//!
//! S11: capabilities só com os comandos próprios (`capabilities/principal.json`,
//! manifesto no `build.rs`), CSP estrita no `tauri.conf.json`, interface embutida
//! (`custom-protocol`), navegação só dentro do app, sem arrastar arquivo para a
//! janela, devtools só com a feature `modo-teste`.
//!
//! D47/D57: a primeira coisa do `main` é a partida do app setgid
//! (`ward_app_nucleo::partida`), antes do GTK: ambiente de D57 conferido, conexão ao
//! daemon com o egid do grupo `ward-app`, grupo largado, não-dumpable de novo e o pai
//! conferido. Fora disso o app sai com a mensagem, sem abrir janela.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use serde::Serialize;
use tauri::{Builder, Manager, RunEvent, Runtime, State, WebviewUrl, WebviewWindowBuilder};
use ward_app_nucleo::ErroApp;
use ward_app_nucleo::area_transferencia::{Copiador, Wayland};
use ward_app_nucleo::daemon::DaemonSocket;
use ward_app_nucleo::partida;
use ward_app_nucleo::servico::Servico;

type ServicoApp = Servico<DaemonSocket, Wayland>;

/// O que a interface recebe num erro: código estável e mensagem fixa, sem valor.
#[derive(Serialize)]
struct Falha {
    codigo: &'static str,
    mensagem: String,
}

impl From<ErroApp> for Falha {
    fn from(e: ErroApp) -> Self {
        Falha {
            codigo: e.codigo(),
            mensagem: e.to_string(),
        }
    }
}

#[derive(Serialize)]
struct RespostaEstado {
    estado: &'static str,
}

/// Serviço montado na partida. Sem `wl-copy`/`wl-paste` confiáveis, os comandos
/// respondem o erro (o app abre e mostra a falha). `Arc`: a vigia da conexão limpa a
/// área de transferência antes de reexecutar o app.
struct Estado(Arc<Result<ServicoApp, ErroApp>>);

impl Estado {
    fn servico(&self) -> Result<&ServicoApp, Falha> {
        self.0.as_ref().as_ref().map_err(|e| Falha::from(*e))
    }
}

#[tauri::command]
fn estado_do_cofre(estado: State<'_, Estado>) -> Result<RespostaEstado, Falha> {
    let e = estado.servico()?.estado_do_cofre()?;
    Ok(RespostaEstado { estado: e.codigo() })
}

/// D4: o valor vai do daemon para a área de transferência aqui; a resposta não o traz.
#[tauri::command]
fn copiar_campo(entrada: String, campo: String, estado: State<'_, Estado>) -> Result<(), Falha> {
    Ok(estado.servico()?.copiar_campo(&entrada, &campo)?)
}

fn montar_servico(daemon: Result<DaemonSocket, ErroApp>) -> Result<ServicoApp, ErroApp> {
    let daemon = daemon?;
    let area = Wayland::novo()?;
    Ok(Servico::novo(daemon, Copiador::novo(area)))
}

/// Só a interface embutida: `tauri://localhost` (Linux). Qualquer outra URL (link,
/// `location = …`, formulário) é recusada pela webview.
fn navegacao_permitida(url: &tauri::Url) -> bool {
    url.scheme() == "tauri" && url.host_str() == Some("localhost")
}

/// Estado e comandos. O teste de ataque monta o mesmo builder com o runtime falso do
/// Tauri e as capabilities reais (`generate_context!`).
fn com_comandos<R: Runtime>(builder: Builder<R>, estado: Estado) -> Builder<R> {
    builder
        .manage(estado)
        .invoke_handler(tauri::generate_handler![estado_do_cofre, copiar_campo])
}

fn main() {
    // Antes do GTK (D47, D57).
    let mut partida = partida::iniciar().unwrap_or_else(|e| {
        eprintln!("ward-app: {e}");
        std::process::exit(1)
    });
    let daemon = partida.tomar_daemon().ok_or(ErroApp::ConexaoPerdida);
    let servico = Arc::new(montar_servico(daemon));
    let na_vigia = Arc::clone(&servico);
    partida.vigiar(move || {
        if let Ok(s) = na_vigia.as_ref() {
            s.ao_sair();
        }
    });

    let app = com_comandos(tauri::Builder::default(), Estado(servico))
        .setup(|app| {
            WebviewWindowBuilder::new(app, "principal", WebviewUrl::App("index.html".into()))
                .title("Ward")
                .inner_size(1280.0, 800.0)
                .min_inner_size(1024.0, 640.0)
                .theme(Some(tauri::Theme::Dark))
                .disable_drag_drop_handler()
                .devtools(cfg!(feature = "modo-teste"))
                .on_navigation(navegacao_permitida)
                .build()?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("o app do Ward não pôde iniciar");

    app.run(|handle, evento| {
        if let RunEvent::Exit = evento
            && let Ok(s) = handle.state::<Estado>().servico()
        {
            s.ao_sair();
        }
    });
}

#[cfg(test)]
mod testes {
    use super::*;
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder};
    use tauri::webview::InvokeRequest;

    fn app_de_teste() -> tauri::App<MockRuntime> {
        let estado = Estado(Arc::new(montar_servico(DaemonSocket::padrao())));
        com_comandos(mock_builder(), estado)
            .build(tauri::generate_context!())
            .expect("app de teste")
    }

    fn chamar(
        janela: &tauri::WebviewWindow<MockRuntime>,
        comando: &str,
        origem: &str,
        corpo: serde_json::Value,
    ) -> Result<serde_json::Value, serde_json::Value> {
        get_ipc_response(
            janela,
            InvokeRequest {
                cmd: comando.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: origem.parse().unwrap(),
                body: InvokeBody::Json(corpo),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|b| b.deserialize::<serde_json::Value>().unwrap())
    }

    const ORIGEM: &str = "tauri://localhost";

    /// V5.1: plugin do sistema de arquivos, shell, http e comando inventado são negados
    /// pela ACL (não há permissão de plugin nem do núcleo na capability).
    #[test]
    fn ataque_plugins_e_comando_inventado_sao_negados() {
        let app = app_de_teste();
        let janela = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        for comando in [
            "plugin:fs|read_text_file",
            "plugin:shell|execute",
            "plugin:shell|open",
            "plugin:http|fetch",
            "plugin:clipboard-manager|read_text",
            "plugin:core|...",
            "plugin:webview|create_webview_window",
            "plugin:window|set_title",
            "plugin:event|listen",
            "plugin:app|version",
            "revelar_tudo",
            "inventado",
        ] {
            let r = chamar(
                &janela,
                comando,
                ORIGEM,
                serde_json::json!({ "path": "/etc/passwd" }),
            );
            assert!(r.is_err(), "{comando} deveria ser negado: {r:?}");
        }
    }

    /// Os comandos próprios passam pela ACL (a resposta é o erro do serviço, sem
    /// daemon), mas só na janela `principal` e só com a origem local.
    #[test]
    fn ataque_comandos_proprios_so_na_janela_e_origem_certas() {
        let app = app_de_teste();
        let principal = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let r = chamar(&principal, "estado_do_cofre", ORIGEM, serde_json::json!({}));
        let falha = r.expect_err("sem daemon o serviço responde erro");
        assert!(
            falha["codigo"].is_string(),
            "erro do serviço, não da ACL: {falha}"
        );

        let outra = WebviewWindowBuilder::new(&app, "outra", Default::default())
            .build()
            .unwrap();
        let r = chamar(&outra, "estado_do_cofre", ORIGEM, serde_json::json!({}));
        assert!(
            r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
            "janela sem capability: {r:?}"
        );

        let r = chamar(
            &principal,
            "copiar_campo",
            "https://example.com",
            serde_json::json!({"entrada":"acme-pg-dev","campo":"senha"}),
        );
        assert!(
            r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
            "origem remota: {r:?}"
        );
    }

    #[test]
    fn ataque_navegacao_para_fora_do_app_e_recusada() {
        for url in [
            "https://example.com/",
            "http://localhost/",
            "http://127.0.0.1:1430/",
            "tauri://example.com/",
            "file:///etc/passwd",
            "data:text/html,<script>1</script>",
            "javascript:alert(1)",
        ] {
            assert!(!navegacao_permitida(&url.parse().unwrap()), "{url}");
        }
        assert!(navegacao_permitida(
            &"tauri://localhost/index.html".parse().unwrap()
        ));
    }

    #[test]
    fn ataque_falha_nao_carrega_valor() {
        let f = Falha::from(ErroApp::PedidoInvalido);
        assert_eq!(f.codigo, "pedido-invalido");
        assert_eq!(f.mensagem, "pedido inválido");
    }
}
