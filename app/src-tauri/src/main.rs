//! App desktop do Ward (Tauri 2). Só a janela e a ligação dos comandos: o que cada
//! comando faz está em `ward_app_nucleo::servico` (o mesmo que a ponte de teste usa).
//!
//! S11: capabilities só com os comandos próprios (`capabilities/principal.json` para a
//! janela principal, `capabilities/aprovacao.json` para a janela de aprovação; manifesto
//! no `build.rs`), CSP estrita no `tauri.conf.json`, interface embutida
//! (`custom-protocol`), navegação só dentro do app, sem arrastar arquivo para a
//! janela, devtools só com a feature `modo-teste`.
//!
//! D47/D57: a primeira coisa do `main` é a partida do app setgid
//! (`ward_app_nucleo::partida`), antes do GTK: ambiente de D57 conferido, conexão ao
//! daemon com o egid do grupo `ward-app`, grupo largado, não-dumpable de novo e o pai
//! conferido. Fora disso o app sai com a mensagem, sem abrir janela.
//!
//! Fase 36 (S10, D65 a): a vigia da fila de avisos tira cada `pedido_de_aprovacao` e
//! abre (ou traz para a frente) a janela `aprovacao`, sempre no topo. Só ela responde
//! (capability própria, e o comando confere o rótulo de novo); fechar a janela nega
//! todo pedido pendente.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{
    AppHandle, Builder, Manager, RunEvent, Runtime, State, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};
use ward_app_nucleo::ErroApp;
use ward_app_nucleo::anexos::{Escolha, Seletor};
use ward_app_nucleo::aprovacao::{self, PedidoExibido, ResultadoAprovacao};
use ward_app_nucleo::area_transferencia::{Copiador, Wayland};
use ward_app_nucleo::credenciais::{DetalheCredencial, ItemCredencial};
use ward_app_nucleo::daemon::DaemonSocket;
use ward_app_nucleo::destrave::{self, InfoPedido, SenhaMestra};
use ward_app_nucleo::formulario::Formulario;
use ward_app_nucleo::gerador::Opcoes;
use ward_app_nucleo::nivel::{NivelMudado, Pin};
use ward_app_nucleo::partida;
use ward_app_nucleo::primeiro_uso::NovoCofre;
use ward_app_nucleo::servico::{CodigoTotp, ResultadoCriar, ResultadoSalvar, Servico};
use ward_app_nucleo::terminal::TerminalDoSistema;

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

/// Fase 31: com o cofre travado, `pedido` é o pedido do agente que espera o destrave
/// (só um id e os segundos que faltam; o aviso não traz entrada nem comando).
#[derive(Serialize)]
struct RespostaEstado {
    estado: &'static str,
    pedido: Option<InfoPedido>,
}

/// O que o canal de avisos leva à tela: só o nome do aviso, sem dado (D65 a).
#[derive(Clone, Serialize)]
struct AvisoTela {
    aviso: &'static str,
}

/// Canais do Tauri que as telas abriram com `ouvir_avisos`, um por janela (o mais novo
/// de cada uma vale). A principal recebe `pedido-de-destrave` (Fase 31); a de aprovação,
/// `pedido-de-aprovacao` (Fase 36). Nenhum aviso leva dado: quem recebe consulta.
#[derive(Default)]
struct Canais {
    principal: Option<Channel<AvisoTela>>,
    aprovacao: Option<Channel<AvisoTela>>,
}

type CanalAvisos = Arc<Mutex<Canais>>;

fn travar_canais(canais: &CanalAvisos) -> std::sync::MutexGuard<'_, Canais> {
    canais.lock().unwrap_or_else(|e| e.into_inner())
}

/// Manda o aviso (só o nome) pelo canal da janela, se ela abriu um.
fn avisar(canais: &CanalAvisos, janela: &str, aviso: &'static str) {
    let c = travar_canais(canais);
    let canal = match janela {
        JANELA_PRINCIPAL => c.principal.as_ref(),
        JANELA_APROVACAO => c.aprovacao.as_ref(),
        _ => None,
    };
    if let Some(canal) = canal {
        let _ = canal.send(AvisoTela { aviso });
    }
}

/// Serviço montado na partida. Sem `wl-copy`/`wl-paste` confiáveis, os comandos
/// respondem o erro (o app abre e mostra a falha). `Arc`: a vigia da conexão limpa a
/// área de transferência antes de reexecutar o app.
struct Estado {
    servico: Arc<Result<ServicoApp, ErroApp>>,
    canal: CanalAvisos,
}

impl Estado {
    fn novo(servico: Arc<Result<ServicoApp, ErroApp>>) -> Estado {
        Estado {
            servico,
            canal: Arc::new(Mutex::new(Canais::default())),
        }
    }

    fn servico(&self) -> Result<&ServicoApp, Falha> {
        self.servico.as_ref().as_ref().map_err(|e| Falha::from(*e))
    }
}

#[tauri::command]
fn estado_do_cofre(estado: State<'_, Estado>) -> Result<RespostaEstado, Falha> {
    let s = estado.servico()?;
    let e = s.estado_do_cofre()?;
    Ok(RespostaEstado {
        estado: e.codigo(),
        pedido: s.info_do_pedido(e),
    })
}

/// Destravar (Fase 31). A senha chega num `SenhaMestra` (zera ao sair; vazio ou acima
/// do limite é recusado na leitura, sem eco) e só vai ao daemon; a resposta não a traz
/// nem diz quantas tentativas houve (D20).
#[tauri::command]
fn destravar(senha: SenhaMestra, estado: State<'_, Estado>) -> Result<(), Falha> {
    Ok(estado.servico()?.destravar(senha)?)
}

/// D65 a: a tela passa o canal por onde o lado Rust a avisa: a principal, de um
/// `pedido_de_destrave`; a de aprovação (Fase 36), de um `pedido_de_aprovacao`. O canal
/// fica com a janela que o abriu (rótulo dado pelo Tauri, não pela tela), e uma janela
/// não recebe o aviso da outra. Canal do Tauri (não `emit`/`listen`): a capability não
/// ganha permissão do núcleo (`core:event:*`).
#[tauri::command]
fn ouvir_avisos<R: Runtime>(
    canal: Channel<AvisoTela>,
    janela: WebviewWindow<R>,
    estado: State<'_, Estado>,
) -> Result<(), Falha> {
    let mut c = travar_canais(&estado.canal);
    match janela.label() {
        JANELA_PRINCIPAL => c.principal = Some(canal),
        JANELA_APROVACAO => c.aprovacao = Some(canal),
        _ => return Err(Falha::from(ErroApp::NaoAutorizado)),
    }
    Ok(())
}

/// A vigia da fila de avisos da conexão (D65 a): cada `pedido_de_destrave` marca o
/// pedido no serviço e avisa a tela pelo canal, se ela já abriu um.
fn vigiar_avisos(estado: &Estado, fila: Option<Arc<ward_app_nucleo::daemon::FilaAvisos>>) {
    let (Some(fila), Ok(s)) = (fila, estado.servico()) else {
        return;
    };
    let canais = Arc::clone(&estado.canal);
    let r = destrave::vigiar(fila, s.pedido_de_destrave(), move || {
        avisar(&canais, JANELA_PRINCIPAL, "pedido-de-destrave");
    });
    if r.is_err() {
        eprintln!("ward-app: não foi possível vigiar os avisos do daemon");
    }
}

/// D4: o valor vai do daemon para a área de transferência aqui; a resposta não o traz.
#[tauri::command]
fn copiar_campo(entrada: String, campo: String, estado: State<'_, Estado>) -> Result<(), Falha> {
    Ok(estado.servico()?.copiar_campo(&entrada, &campo)?)
}

/// Tela Credenciais (Fase 32): a lista, sem segredo.
#[tauri::command]
fn listar_credenciais(estado: State<'_, Estado>) -> Result<Vec<ItemCredencial>, Falha> {
    Ok(estado.servico()?.listar_credenciais()?)
}

/// Tela Credenciais (Fase 32): o detalhe de uma entrada, sem segredo (só os nomes dos
/// campos secretos). O nome vem da interface e é validado no núcleo.
#[tauri::command]
fn detalhe_credencial(
    entrada: String,
    estado: State<'_, Estado>,
) -> Result<DetalheCredencial, Falha> {
    Ok(estado.servico()?.detalhe_credencial(&entrada)?)
}

#[derive(Serialize)]
struct RespostaGerada {
    tamanho: usize,
    bits: u32,
    forca: &'static str,
    simbolos: &'static str,
}

/// Gerador avulso (Fase 37): a senha nasce no `ward_core::gerador` e fica no Rust; a
/// resposta não traz o valor. Os argumentos vêm da interface (não confiável) e são
/// conferidos no núcleo (tamanho 8–64, pelo menos um conjunto).
#[tauri::command]
fn gerar_senha(
    tamanho: u64,
    minusculas: bool,
    maiusculas: bool,
    digitos: bool,
    simbolos: bool,
    estado: State<'_, Estado>,
) -> Result<RespostaGerada, Falha> {
    let g = estado.servico()?.gerar_senha(Opcoes {
        tamanho,
        minusculas,
        maiusculas,
        digitos,
        simbolos,
    })?;
    Ok(RespostaGerada {
        tamanho: g.tamanho,
        bits: g.bits,
        forca: g.forca.codigo(),
        simbolos: g.simbolos,
    })
}

/// D4: a senha avulsa vai para a área de transferência aqui; a resposta não a traz.
#[tauri::command]
fn copiar_senha_gerada(estado: State<'_, Estado>) -> Result<(), Falha> {
    Ok(estado.servico()?.copiar_senha_gerada()?)
}

#[derive(Serialize)]
struct RespostaRevelada {
    valor: String,
}

/// S11: o único comando que leva o valor à webview, no clique em "revelar".
#[tauri::command]
fn revelar_senha_gerada(estado: State<'_, Estado>) -> Result<RespostaRevelada, Falha> {
    let r = estado.servico()?.revelar_senha_gerada()?;
    Ok(RespostaRevelada {
        valor: r.expor().to_owned(),
    })
}

#[tauri::command]
fn descartar_senha_gerada(estado: State<'_, Estado>) -> Result<(), Falha> {
    Ok(estado.servico()?.descartar_senha_gerada()?)
}

/// S11 (D65 b): o valor de um segredo da entrada, só no clique em "revelar".
#[tauri::command]
fn revelar_campo(
    entrada: String,
    campo: String,
    estado: State<'_, Estado>,
) -> Result<RespostaRevelada, Falha> {
    let r = estado.servico()?.revelar_campo(&entrada, &campo)?;
    Ok(RespostaRevelada {
        valor: r.expor().to_owned(),
    })
}

/// Formulário (Fase 33): o código TOTP atual; a semente não sai do daemon.
#[tauri::command]
fn codigo_totp(entrada: String, estado: State<'_, Estado>) -> Result<CodigoTotp, Falha> {
    Ok(estado.servico()?.codigo_totp(&entrada)?)
}

/// D4: o código TOTP vai para a área de transferência aqui.
#[tauri::command]
fn copiar_codigo_totp(entrada: String, estado: State<'_, Estado>) -> Result<(), Falha> {
    Ok(estado.servico()?.copiar_codigo_totp(&entrada)?)
}

#[derive(Serialize)]
struct RespostaUsada {
    tamanho: usize,
}

/// "Usar esta senha": a senha gerada passa ao campo no lado Rust; volta só o tamanho.
#[tauri::command]
fn usar_senha_gerada(campo: String, estado: State<'_, Estado>) -> Result<RespostaUsada, Falha> {
    let tamanho = estado.servico()?.usar_senha_gerada(&campo)?;
    Ok(RespostaUsada { tamanho })
}

#[tauri::command]
fn descartar_formulario(estado: State<'_, Estado>) -> Result<(), Falha> {
    estado.servico()?.descartar_formulario();
    Ok(())
}

/// Formulário (Fase 33): cria ou edita. O formulário vem da interface (não confiável)
/// e é conferido no núcleo; a resposta é o nome salvo ou os erros por campo.
#[tauri::command]
fn salvar_credencial(
    anterior: Option<String>,
    formulario: Formulario,
    estado: State<'_, Estado>,
) -> Result<ResultadoSalvar, Falha> {
    Ok(estado
        .servico()?
        .salvar_credencial(anterior.as_deref(), formulario)?)
}

/// Nível de acesso (Fase 35, S9). O PIN chega num `Pin` (zera ao sair; o formato é
/// conferido na leitura, antes do serviço) e só vai ao daemon; a resposta é o nível novo.
#[tauri::command]
fn mudar_nivel(
    entrada: String,
    nivel: String,
    pin: Pin,
    estado: State<'_, Estado>,
) -> Result<NivelMudado, Falha> {
    Ok(estado.servico()?.mudar_nivel(&entrada, &nivel, pin)?)
}

/// Anexo (Fase 34): o diálogo de arquivo do GTK escolhe o arquivo e o núcleo o lê; a
/// resposta traz id, nome e tamanho ou o aviso, nunca o conteúdo (S11, V5.12). Comando
/// síncrono: roda na thread principal, onde o GTK pode abrir o diálogo.
#[tauri::command]
fn escolher_anexo(estado: State<'_, Estado>) -> Result<Escolha, Falha> {
    Ok(estado.servico()?.escolher_anexo()?)
}

#[tauri::command]
fn remover_anexo(id: u64, estado: State<'_, Estado>) -> Result<(), Falha> {
    Ok(estado.servico()?.remover_anexo(id)?)
}

/// "Conectar" (D3): token de uso único e o terminal padrão; o token não volta.
#[tauri::command]
fn conectar(entrada: String, estado: State<'_, Estado>) -> Result<(), Falha> {
    Ok(estado.servico()?.conectar(&entrada)?)
}

/// Primeiro uso (Fase 30): senha mestra, PIN e as confirmações, conferidos no núcleo;
/// o daemon grava só o hash do PIN (D30). A resposta não traz valor.
#[tauri::command]
fn criar_cofre(dados: NovoCofre, estado: State<'_, Estado>) -> Result<ResultadoCriar, Falha> {
    Ok(estado.servico()?.criar_cofre(dados)?)
}

/// Rótulo da janela principal (capability `principal`).
const JANELA_PRINCIPAL: &str = "principal";
/// Rótulo da janela de aprovação: a capability `aprovacao` vale só para ela.
const JANELA_APROVACAO: &str = "aprovacao";

/// Segunda camada além da ACL: os comandos da aprovação só respondem na janela certa.
fn na_janela_de_aprovacao<R: Runtime>(janela: &WebviewWindow<R>) -> Result<(), Falha> {
    if janela.label() == JANELA_APROVACAO {
        Ok(())
    } else {
        Err(Falha::from(ErroApp::NaoAutorizado))
    }
}

/// Janela de aprovação (Fase 36): o pedido da vez, com o texto do agente já
/// neutralizado (D51), ou `null`. Sem pedido, a janela fecha (não há o que negar).
#[tauri::command]
fn pedido_de_aprovacao<R: Runtime>(
    janela: WebviewWindow<R>,
    estado: State<'_, Estado>,
) -> Result<Option<PedidoExibido>, Falha> {
    na_janela_de_aprovacao(&janela)?;
    let p = estado.servico()?.pedido_de_aprovacao()?;
    if p.is_none() {
        let _ = janela.destroy();
    }
    Ok(p)
}

/// Janela de aprovação (Fase 36, S9, S10): Negar sem PIN, os três Permitir com o PIN
/// (num `Pin` que zera; fora do formato é recusado na leitura, sem eco). A resposta não
/// traz o PIN. Sem outro pedido esperando, a janela fecha.
#[tauri::command]
fn responder_aprovacao<R: Runtime>(
    id: u64,
    decisao: String,
    pin: Option<Pin>,
    janela: WebviewWindow<R>,
    estado: State<'_, Estado>,
) -> Result<ResultadoAprovacao, Falha> {
    na_janela_de_aprovacao(&janela)?;
    let r = estado.servico()?.responder_aprovacao(id, &decisao, pin)?;
    if !r.ha_outro {
        let _ = janela.destroy();
    }
    Ok(r)
}

/// Abre a janela de aprovação ou a traz para a frente (S10: "abre em primeiro plano").
/// Na thread principal (quem chama usa `run_on_main_thread`).
fn abrir_janela_de_aprovacao<R: Runtime>(app: &AppHandle<R>) {
    if let Some(j) = app.get_webview_window(JANELA_APROVACAO) {
        let _ = j.unminimize();
        let _ = j.show();
        let _ = j.set_always_on_top(true);
        let _ = j.set_focus();
        return;
    }
    let r = WebviewWindowBuilder::new(app, JANELA_APROVACAO, WebviewUrl::App("index.html".into()))
        .title("Pedido de aprovação · Ward")
        .inner_size(560.0, 880.0)
        .min_inner_size(480.0, 560.0)
        .center()
        .always_on_top(true)
        .focused(true)
        .theme(Some(tauri::Theme::Dark))
        .disable_drag_drop_handler()
        .devtools(cfg!(feature = "modo-teste"))
        .on_navigation(navegacao_permitida)
        .build();
    if r.is_err() {
        // Sem a janela, ninguém responde: o pedido expira e nega no daemon (S10).
        eprintln!("ward-app: não foi possível abrir a janela de aprovação");
    }
}

/// Fechar a janela de aprovação nega (S10). Em outra thread: a conversa com o daemon
/// não prende o laço de eventos.
fn ao_fechar_a_aprovacao(servico: Arc<Result<ServicoApp, ErroApp>>) {
    let _ = std::thread::Builder::new()
        .name("ward-app-negar".into())
        .spawn(move || {
            if let Ok(s) = servico.as_ref() {
                s.negar_aprovacoes_pendentes();
            }
        });
}

/// O diálogo "Adicionar arquivo" do GTK, na janela do app.
struct SeletorGtk;

impl Seletor for SeletorGtk {
    fn escolher(&self) -> Result<Option<PathBuf>, ErroApp> {
        use gtk::prelude::{DialogExt, FileChooserExt, GtkWindowExt};
        if !gtk::is_initialized_main_thread() {
            return Err(ErroApp::SeletorIndisponivel);
        }
        let dialogo = gtk::FileChooserDialog::with_buttons(
            Some("Adicionar arquivo"),
            None::<&gtk::Window>,
            gtk::FileChooserAction::Open,
            &[
                ("Cancelar", gtk::ResponseType::Cancel),
                ("Anexar", gtk::ResponseType::Accept),
            ],
        );
        dialogo.set_modal(true);
        dialogo.set_select_multiple(false);
        dialogo.set_local_only(true);
        let resposta = dialogo.run();
        let arquivo = (resposta == gtk::ResponseType::Accept)
            .then(|| dialogo.filename())
            .flatten();
        // `close` pede o fechamento como o botão da janela: o diálogo é destruído.
        dialogo.close();
        Ok(arquivo)
    }
}

fn montar_servico(daemon: Result<DaemonSocket, ErroApp>) -> Result<ServicoApp, ErroApp> {
    let daemon = daemon?;
    let area = Wayland::novo()?;
    Ok(Servico::novo(daemon, Copiador::novo(area))
        .com_seletor(SeletorGtk)
        .com_terminal(TerminalDoSistema::default()))
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
        .invoke_handler(tauri::generate_handler![
            estado_do_cofre,
            copiar_campo,
            listar_credenciais,
            detalhe_credencial,
            gerar_senha,
            copiar_senha_gerada,
            revelar_senha_gerada,
            descartar_senha_gerada,
            revelar_campo,
            codigo_totp,
            copiar_codigo_totp,
            usar_senha_gerada,
            descartar_formulario,
            salvar_credencial,
            mudar_nivel,
            escolher_anexo,
            remover_anexo,
            conectar,
            criar_cofre,
            destravar,
            ouvir_avisos,
            pedido_de_aprovacao,
            responder_aprovacao
        ])
}

fn main() {
    // Antes do GTK (D47, D57).
    let mut partida = partida::iniciar().unwrap_or_else(|e| {
        eprintln!("ward-app: {e}");
        std::process::exit(1)
    });
    let daemon = partida.tomar_daemon().ok_or(ErroApp::ConexaoPerdida);
    let fila = daemon.as_ref().ok().map(DaemonSocket::avisos);
    let servico = Arc::new(montar_servico(daemon));
    let na_aprovacao = Arc::clone(&servico);
    let na_vigia = Arc::clone(&servico);
    partida.vigiar(move || {
        if let Ok(s) = na_vigia.as_ref() {
            s.ao_sair();
        }
    });

    let estado = Estado::novo(servico);
    let canais = Arc::clone(&estado.canal);
    vigiar_avisos(&estado, fila.clone());
    let app = com_comandos(tauri::Builder::default(), estado)
        .on_window_event(|janela, evento| {
            if janela.label() == JANELA_APROVACAO && matches!(evento, WindowEvent::Destroyed) {
                let estado = janela.state::<Estado>();
                travar_canais(&estado.canal).aprovacao = None;
                ao_fechar_a_aprovacao(Arc::clone(&estado.servico));
            }
        })
        .setup(move |app| {
            // D65 a: cada `pedido_de_aprovacao` da fila abre (ou traz para a frente) a
            // janela de aprovação e a avisa pelo canal dela (`ouvir_avisos`), sem dado.
            if let (Some(fila), Ok(s)) = (fila, na_aprovacao.as_ref()) {
                let h = app.handle().clone();
                let r = aprovacao::vigiar(fila, s.aprovacoes(), move || {
                    let h2 = h.clone();
                    let _ = h.run_on_main_thread(move || abrir_janela_de_aprovacao(&h2));
                    avisar(&canais, JANELA_APROVACAO, "pedido-de-aprovacao");
                });
                if r.is_err() {
                    eprintln!("ward-app: não foi possível vigiar os pedidos de aprovação");
                }
            }
            WebviewWindowBuilder::new(app, JANELA_PRINCIPAL, WebviewUrl::App("index.html".into()))
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
        let estado = Estado::novo(Arc::new(montar_servico(DaemonSocket::padrao())));
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

    /// Fase 37 (V5.21, S11): os comandos do gerador passam pela ACL só na janela
    /// `principal` e com a origem local; "gerar" não devolve o valor; tamanho fora de
    /// 8–64 ou de tipo errado é recusado.
    #[test]
    fn ataque_gerador_so_na_janela_certa_e_sem_valor_no_gerar() {
        let app = app_de_teste();
        let principal = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let opcoes = serde_json::json!({
            "tamanho": 24, "minusculas": true, "maiusculas": true,
            "digitos": true, "simbolos": true
        });
        match chamar(&principal, "gerar_senha", ORIGEM, opcoes.clone()) {
            // Sem `wl-copy` confiável o serviço não monta e responde o erro dele.
            Err(falha) => assert!(falha["codigo"].is_string(), "erro da ACL: {falha}"),
            Ok(r) => {
                let chaves: Vec<&String> = r.as_object().unwrap().keys().collect();
                assert_eq!(chaves, ["bits", "forca", "simbolos", "tamanho"]);
                let v = chamar(&principal, "revelar_senha_gerada", ORIGEM, opcoes.clone()).unwrap();
                let valor = v["valor"].as_str().unwrap();
                assert_eq!(valor.len(), 24);
                assert!(!r.to_string().contains(valor));
                for ruim in [
                    serde_json::json!({ "tamanho": 7, "minusculas": true, "maiusculas": true, "digitos": true, "simbolos": true }),
                    serde_json::json!({ "tamanho": 65, "minusculas": true, "maiusculas": true, "digitos": true, "simbolos": true }),
                    serde_json::json!({ "tamanho": "24", "minusculas": true, "maiusculas": true, "digitos": true, "simbolos": true }),
                    serde_json::json!({ "tamanho": -1, "minusculas": true, "maiusculas": true, "digitos": true, "simbolos": true }),
                    serde_json::json!({ "tamanho": 24 }),
                ] {
                    assert!(
                        chamar(&principal, "gerar_senha", ORIGEM, ruim.clone()).is_err(),
                        "{ruim}"
                    );
                }
            }
        }
        let outra = WebviewWindowBuilder::new(&app, "outra", Default::default())
            .build()
            .unwrap();
        for comando in [
            "gerar_senha",
            "revelar_senha_gerada",
            "copiar_senha_gerada",
            "descartar_senha_gerada",
        ] {
            let r = chamar(&outra, comando, ORIGEM, opcoes.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{comando} em janela sem capability: {r:?}"
            );
            let r = chamar(&principal, comando, "https://example.com", opcoes.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{comando} com origem remota: {r:?}"
            );
        }
    }

    /// Fase 33 (S11): os comandos do formulário e o `revelar_campo` só passam pela ACL
    /// na janela `principal` com a origem local; formulário fora do formato é recusado
    /// antes do serviço.
    #[test]
    fn ataque_formulario_so_na_janela_certa() {
        let app = app_de_teste();
        let principal = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let outra = WebviewWindowBuilder::new(&app, "outra", Default::default())
            .build()
            .unwrap();
        let formulario = serde_json::json!({
            "anterior": null,
            "formulario": {"tipo": "login", "nome": "x", "grupo": "g", "ambiente": "dev",
                "projeto": "", "tags": [], "campos": {}, "segredos": {}, "extras": []},
            "entrada": "acme-wp-dev", "campo": "senha", "id": 0
        });
        for comando in [
            "revelar_campo",
            "codigo_totp",
            "copiar_codigo_totp",
            "usar_senha_gerada",
            "descartar_formulario",
            "salvar_credencial",
            // Fase 34: anexos e "Conectar" seguem a mesma regra.
            "escolher_anexo",
            "remover_anexo",
            "conectar",
        ] {
            let r = chamar(&outra, comando, ORIGEM, formulario.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{comando} em janela sem capability: {r:?}"
            );
            let r = chamar(
                &principal,
                comando,
                "https://example.com",
                formulario.clone(),
            );
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{comando} com origem remota: {r:?}"
            );
        }
        let forjado = serde_json::json!({
            "formulario": {"tipo": "login", "nome": "x", "grupo": "g", "ambiente": "dev",
                "projeto": "", "tags": [], "campos": {}, "segredos": {}, "extras": [],
                "nivel": "livre"}
        });
        let r = chamar(&principal, "salvar_credencial", ORIGEM, forjado);
        assert!(
            r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
            "campo a mais: {r:?}"
        );
    }

    /// Fase 35 (S9): `mudar_nivel` só passa pela ACL na janela `principal` com a origem
    /// local, e um PIN fora do formato (ou um campo a mais) é recusado na leitura dos
    /// argumentos, antes do serviço, sem ecoar o valor.
    #[test]
    fn ataque_mudar_nivel_so_na_janela_certa_e_pin_no_formato() {
        let app = app_de_teste();
        let principal = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let outra = WebviewWindowBuilder::new(&app, "outra", Default::default())
            .build()
            .unwrap();
        let pedido =
            serde_json::json!({"entrada": "acme-pg-dev", "nivel": "bloqueado", "pin": "135790"});
        let r = chamar(&outra, "mudar_nivel", ORIGEM, pedido.clone());
        assert!(
            r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
            "janela sem capability: {r:?}"
        );
        let r = chamar(
            &principal,
            "mudar_nivel",
            "https://example.com",
            pedido.clone(),
        );
        assert!(
            r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
            "origem remota: {r:?}"
        );
        for pin in [
            serde_json::json!("12"),
            serde_json::json!("1357901"),
            serde_json::json!("13579a"),
            serde_json::json!(135790),
            serde_json::Value::Null,
        ] {
            let corpo =
                serde_json::json!({"entrada": "acme-pg-dev", "nivel": "bloqueado", "pin": pin});
            let r = chamar(&principal, "mudar_nivel", ORIGEM, corpo);
            let texto = format!("{r:?}");
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "PIN fora do formato passou da leitura: {texto}"
            );
            assert!(
                !texto.contains("135790") && !texto.contains("13579a"),
                "{texto}"
            );
        }
        // Na janela certa, o pedido bem formado chega ao serviço (aqui sem daemon: o
        // erro é do serviço, com código, e não traz o PIN).
        let r = chamar(&principal, "mudar_nivel", ORIGEM, pedido);
        let texto = format!("{r:?}");
        assert!(
            r.as_ref().err().is_some_and(|e| e["codigo"].is_string()),
            "{texto}"
        );
        assert!(!texto.contains("135790"), "{texto}");
    }

    /// Fase 30: `criar_cofre` só na janela `principal` com a origem local; campo a mais
    /// é recusado antes do serviço; PIN fora de 4 a 6 dígitos volta como erro de campo
    /// sem ir ao daemon (que nem está rodando aqui) e sem valor na resposta.
    #[test]
    fn ataque_criar_cofre_so_na_janela_certa_e_conferido_no_rust() {
        let app = app_de_teste();
        let principal = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let outra = WebviewWindowBuilder::new(&app, "outra", Default::default())
            .build()
            .unwrap();
        let dados = |pin: &str| {
            serde_json::json!({ "dados": {
                "senha": "falso-mestra-ward-01", "confirmacao": "falso-mestra-ward-01",
                "pin": pin, "confirmacao_pin": pin } })
        };
        let r = chamar(&outra, "criar_cofre", ORIGEM, dados("135790"));
        assert!(
            r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
            "janela sem capability: {r:?}"
        );
        let r = chamar(
            &principal,
            "criar_cofre",
            "https://example.com",
            dados("135790"),
        );
        assert!(
            r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
            "origem remota: {r:?}"
        );
        for pin in ["135", "1357901", "13579a"] {
            let r = chamar(&principal, "criar_cofre", ORIGEM, dados(pin)).unwrap();
            assert_eq!(r["criado"], false, "{pin}");
            assert_eq!(r["erros"][0]["campo"], "pin", "{pin}");
            let texto = r.to_string();
            assert!(
                !texto.contains("falso-mestra") && !texto.contains(pin),
                "{texto}"
            );
        }
        let mut forjado = dados("135790");
        forjado["dados"]["nivel"] = "livre".into();
        let r = chamar(&principal, "criar_cofre", ORIGEM, forjado);
        assert!(
            r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
            "campo a mais: {r:?}"
        );
    }

    /// Fase 31 (S13, D65 a): `destravar` e `ouvir_avisos` só passam pela ACL na janela
    /// `principal` com a origem local; a senha fora do formato é recusada na leitura dos
    /// argumentos, sem eco; o canal de avisos não abre `plugin:event|*`.
    #[test]
    fn ataque_destravar_so_na_janela_certa_e_senha_sem_eco() {
        let app = app_de_teste();
        let principal = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let outra = WebviewWindowBuilder::new(&app, "outra", Default::default())
            .build()
            .unwrap();
        let pedido = serde_json::json!({"senha": "falso-mestra-ward-01"});
        for comando in ["destravar", "ouvir_avisos"] {
            let r = chamar(&outra, comando, ORIGEM, pedido.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{comando} em janela sem capability: {r:?}"
            );
            let r = chamar(&principal, comando, "https://example.com", pedido.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{comando} com origem remota: {r:?}"
            );
        }
        let longa = "x".repeat(ward_core_max_segredo() + 1);
        for senha in [
            serde_json::json!(""),
            serde_json::json!(longa),
            serde_json::json!(135790),
            serde_json::Value::Null,
            serde_json::json!(["falso-mestra-ward-01"]),
        ] {
            let r = chamar(
                &principal,
                "destravar",
                ORIGEM,
                serde_json::json!({"senha": senha}),
            );
            let texto = format!("{r:?}");
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "senha fora do formato passou da leitura: {}",
                texto.len()
            );
            assert!(!texto.contains("falso-mestra") && !texto.contains("xxxx"));
        }
        // Bem formada, chega ao serviço (sem daemon: erro com código, sem a senha).
        let r = chamar(&principal, "destravar", ORIGEM, pedido);
        let texto = format!("{r:?}");
        assert!(
            r.as_ref().err().is_some_and(|e| e["codigo"].is_string()),
            "{texto}"
        );
        assert!(!texto.contains("falso-mestra"), "{texto}");
        // O canal não libera os comandos de evento do núcleo.
        for comando in [
            "plugin:event|listen",
            "plugin:event|emit",
            "plugin:event|emit_to",
        ] {
            let r = chamar(
                &principal,
                comando,
                ORIGEM,
                serde_json::json!({"event": "pedido-de-destrave", "target": {"kind": "Any"}, "handler": 0}),
            );
            assert!(r.is_err(), "{comando} deveria ser negado: {r:?}");
        }
    }

    fn ward_core_max_segredo() -> usize {
        64 * 1024
    }

    /// D65 a, Fase 36: o canal de avisos é por janela. O aviso de aprovação não chega à
    /// principal nem o de destrave à de aprovação, e nenhum leva dado além do nome.
    #[test]
    fn ataque_aviso_de_uma_janela_nao_vai_para_a_outra() {
        use std::sync::mpsc;
        let canal = |tx: mpsc::Sender<String>| {
            Channel::<AvisoTela>::new(move |corpo| {
                let texto = match corpo {
                    tauri::ipc::InvokeResponseBody::Json(j) => j,
                    tauri::ipc::InvokeResponseBody::Raw(_) => "bruto".to_owned(),
                };
                let _ = tx.send(texto);
                Ok(())
            })
        };
        let (tx_p, rx_p) = mpsc::channel();
        let (tx_a, rx_a) = mpsc::channel();
        let canais: CanalAvisos = Arc::default();
        travar_canais(&canais).principal = Some(canal(tx_p));
        travar_canais(&canais).aprovacao = Some(canal(tx_a));
        avisar(&canais, JANELA_APROVACAO, "pedido-de-aprovacao");
        avisar(&canais, JANELA_PRINCIPAL, "pedido-de-destrave");
        avisar(&canais, "outra", "pedido-de-aprovacao");
        assert_eq!(
            rx_a.try_iter().collect::<Vec<_>>(),
            vec![r#"{"aviso":"pedido-de-aprovacao"}"#]
        );
        assert_eq!(
            rx_p.try_iter().collect::<Vec<_>>(),
            vec![r#"{"aviso":"pedido-de-destrave"}"#]
        );
        // Janela de aprovação destruída: o canal dela sai, e o aviso não vai a ninguém.
        travar_canais(&canais).aprovacao = None;
        avisar(&canais, JANELA_APROVACAO, "pedido-de-aprovacao");
        assert!(rx_a.try_iter().next().is_none() && rx_p.try_iter().next().is_none());
    }

    /// Fase 36 (S10, S11): responder aprovação só na janela `aprovacao`, com a origem
    /// local. A janela principal (e qualquer outra) não lê o pedido nem responde; a de
    /// aprovação não alcança nenhum outro comando nem plugin. Resposta forjada (id que o
    /// daemon não mandou) é recusada pelo serviço; PIN fora do formato, na leitura, sem
    /// eco.
    #[test]
    fn ataque_aprovacao_so_pela_janela_certa() {
        let app = app_de_teste();
        let principal = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let outra = WebviewWindowBuilder::new(&app, "outra", Default::default())
            .build()
            .unwrap();
        let aprovacao = WebviewWindowBuilder::new(&app, JANELA_APROVACAO, Default::default())
            .build()
            .unwrap();
        let negado_pela_acl = |r: &Result<serde_json::Value, serde_json::Value>| {
            r.as_ref().err().is_some_and(|e| !e["codigo"].is_string())
        };
        let resposta = serde_json::json!({"id": 1, "decisao": "uma_vez", "pin": "135790"});
        for janela in [&principal, &outra] {
            for comando in ["pedido_de_aprovacao", "responder_aprovacao"] {
                let r = chamar(janela, comando, ORIGEM, resposta.clone());
                assert!(
                    negado_pela_acl(&r),
                    "{comando} em {}: {r:?}",
                    janela.label()
                );
                assert!(!format!("{r:?}").contains("135790"));
            }
        }
        for comando in ["pedido_de_aprovacao", "responder_aprovacao"] {
            let r = chamar(&aprovacao, comando, "https://example.com", resposta.clone());
            assert!(negado_pela_acl(&r), "{comando} com origem remota: {r:?}");
        }
        // A janela de aprovação não alcança mais nada.
        for comando in [
            "estado_do_cofre",
            "copiar_campo",
            "listar_credenciais",
            "detalhe_credencial",
            "revelar_campo",
            "revelar_senha_gerada",
            "mudar_nivel",
            "salvar_credencial",
            "conectar",
            "criar_cofre",
            "plugin:window|close",
            "plugin:window|set_always_on_top",
            "plugin:webview|create_webview_window",
            "plugin:event|listen",
            "plugin:fs|read_text_file",
            "plugin:shell|execute",
        ] {
            let r = chamar(
                &aprovacao,
                comando,
                ORIGEM,
                serde_json::json!({"entrada": "acme-pg-aprovacao", "campo": "senha", "nivel": "livre", "pin": "135790"}),
            );
            assert!(
                negado_pela_acl(&r),
                "{comando} na janela de aprovação: {r:?}"
            );
        }
        // PIN fora do formato e decisão de outro tipo: recusados na leitura, sem eco.
        for corpo in [
            serde_json::json!({"id": 1, "decisao": "uma_vez", "pin": "12"}),
            serde_json::json!({"id": 1, "decisao": "uma_vez", "pin": "13579a"}),
            serde_json::json!({"id": 1, "decisao": "uma_vez", "pin": 135790}),
            serde_json::json!({"id": "1", "decisao": "uma_vez", "pin": "135790"}),
            serde_json::json!({"id": -1, "decisao": "negar", "pin": null}),
            serde_json::json!({"id": 1, "decisao": 1, "pin": null}),
        ] {
            let r = chamar(&aprovacao, "responder_aprovacao", ORIGEM, corpo);
            let texto = format!("{r:?}");
            assert!(negado_pela_acl(&r), "{texto}");
            assert!(
                !texto.contains("135790") && !texto.contains("13579a"),
                "{texto}"
            );
        }
        // Bem formada, mas o daemon nunca mandou o pedido 1: o serviço recusa (ou nem
        // montou, sem área de transferência). Erro com código, sem o PIN.
        let r = chamar(&aprovacao, "responder_aprovacao", ORIGEM, resposta);
        let texto = format!("{r:?}");
        assert!(
            r.as_ref().err().is_some_and(|e| e["codigo"].is_string()),
            "{texto}"
        );
        assert!(!texto.contains("135790"), "{texto}");
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
