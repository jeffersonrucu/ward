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
//!
//! Fase 39 (V5.23): ícone e menu na bandeja (AppIndicator, `ward_app_nucleo::bandeja`):
//! "Abrir o Ward", "Travar agora"/"Destravar…" e "Sair". Com a bandeja ativa, fechar a
//! janela principal a destrói (a webview e o que a tela tinha somem) e o app fica na
//! bandeja; "Abrir o Ward" ou um `pedido_de_destrave` a criam de novo. Sem a biblioteca
//! do AppIndicator ou sem pasta segura para o ícone, o app abre sem bandeja e fechar a
//! janela fecha o app, como antes.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::image::Image;
use tauri::ipc::Channel;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{
    AppHandle, Builder, Manager, RunEvent, Runtime, State, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};
use ward_app_nucleo::ErroApp;
use ward_app_nucleo::anexos::{EdicaoDosAnexos, Escolha, Seletor};
use ward_app_nucleo::aprovacao::{self, PedidoExibido, ResultadoAprovacao};
use ward_app_nucleo::area_transferencia::{Copiador, Wayland};
use ward_app_nucleo::backup_externo::{BackupExternoVisto, PendriveVisto};
use ward_app_nucleo::bandeja::{self, Acao, EstadoBandeja, Icone};
use ward_app_nucleo::configuracoes::{Configuracoes, ResultadoTroca, TrocaPin, TrocaSenha};
use ward_app_nucleo::credenciais::{DetalheCredencial, ItemCredencial};
use ward_app_nucleo::daemon::DaemonSocket;
use ward_app_nucleo::destrave::{self, InfoPedido, SenhaMestra};
use ward_app_nucleo::formulario::Formulario;
use ward_app_nucleo::gerador::Opcoes;
use ward_app_nucleo::nivel::{NivelMudado, Pin};
use ward_app_nucleo::partida;
use ward_app_nucleo::primeiro_uso::NovoCofre;
use ward_app_nucleo::restaurar::{Restauracao, Restaurado, VersaoVista};
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
/// (id, segundos que faltam e, Fase 39a, entrada, operação e comando com D51).
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

/// "Travar agora" da `Navegação lateral` (Fase 39): limpa a área de transferência e o
/// formulário, trava, e a bandeja passa a mostrar travado.
#[tauri::command]
fn travar<R: Runtime>(app: AppHandle<R>, estado: State<'_, Estado>) -> Result<(), Falha> {
    let r = estado.servico()?.travar();
    atualizar_bandeja(&app);
    Ok(r?)
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
/// Fase 39: com o app só na bandeja, o pedido também traz a janela principal de volta
/// (D16: "o app abre e pede a senha mestra").
fn vigiar_avisos<R: Runtime>(
    app: &AppHandle<R>,
    estado: &Estado,
    fila: Option<Arc<ward_app_nucleo::daemon::FilaAvisos>>,
) {
    let (Some(fila), Ok(s)) = (fila, estado.servico()) else {
        return;
    };
    let canais = Arc::clone(&estado.canal);
    let h = app.clone();
    let r = destrave::vigiar(fila, s.pedido_de_destrave(), move || {
        let h2 = h.clone();
        let _ = h.run_on_main_thread(move || mostrar_principal(&h2));
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
/// Fase 39b: na edição, `edicao { entrada, mantidos }` (os anexos guardados que
/// continuam contam nos limites; o serviço confere os nomes no detalhe).
#[tauri::command]
fn escolher_anexo(
    edicao: Option<EdicaoDosAnexos>,
    estado: State<'_, Estado>,
) -> Result<Escolha, Falha> {
    Ok(estado.servico()?.escolher_anexo(edicao.as_ref())?)
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

/// Configurações (Fase 38): tempo de trava e backup externo, sem hash nem UUID.
#[tauri::command]
fn configuracoes<R: Runtime>(
    app: AppHandle<R>,
    estado: State<'_, Estado>,
) -> Result<Configuracoes, Falha> {
    let c = estado.servico()?.configuracoes()?;
    // Fase 39: a tela abriu (ação do usuário); a bandeja aproveita o tempo lido.
    tempo_da_bandeja(&app, c.minutos_trava);
    Ok(c)
}

/// Trocar o PIN (Fase 38, V5.22, S9): atual, novo e confirmação em `Digitado` (zera);
/// conferidos no núcleo, o atual pelo daemon. A resposta não traz valor.
#[tauri::command]
fn trocar_pin(dados: TrocaPin, estado: State<'_, Estado>) -> Result<ResultadoTroca, Falha> {
    Ok(estado.servico()?.trocar_pin(dados)?)
}

/// Trocar a senha mestra (Fase 38, V5.22): a atual conferida pelo daemon; a nova com a
/// regra do primeiro uso. A resposta não traz valor.
#[tauri::command]
fn trocar_senha_mestra(
    dados: TrocaSenha,
    estado: State<'_, Estado>,
) -> Result<ResultadoTroca, Falha> {
    Ok(estado.servico()?.trocar_senha_mestra(dados)?)
}

/// Tempo de trava (Fase 38, D30, D45): 1 a 1440 minutos, gravado no cofre pelo daemon.
#[tauri::command]
fn definir_tempo_trava<R: Runtime>(
    minutos: u64,
    app: AppHandle<R>,
    estado: State<'_, Estado>,
) -> Result<Configuracoes, Falha> {
    let c = estado.servico()?.definir_tempo_trava(minutos)?;
    // Fase 39: o subtítulo da bandeja passa a dizer o tempo novo.
    tempo_da_bandeja(&app, c.minutos_trava);
    Ok(c)
}

/// Backup externo (Fase 48, V7.9): ligado ou não, o UUID e, se conectado, o pendrive
/// (rótulo neutralizado, D51). Nenhum caminho de montagem.
#[tauri::command]
fn backup_externo(estado: State<'_, Estado>) -> Result<BackupExternoVisto, Falha> {
    Ok(estado.servico()?.backup_externo()?)
}

/// Os pendrives que o daemon vê, para escolher o do backup externo (Fase 48).
#[tauri::command]
fn listar_pendrives(estado: State<'_, Estado>) -> Result<Vec<PendriveVisto>, Falha> {
    Ok(estado.servico()?.listar_pendrives()?)
}

/// Ligar (com um pendrive que o daemon vê agora; o UUID da interface é conferido de novo
/// no núcleo) ou desligar o backup externo (Fase 48, D17, D30).
#[tauri::command]
fn definir_backup_externo(
    ligado: bool,
    uuid: Option<String>,
    estado: State<'_, Estado>,
) -> Result<BackupExternoVisto, Falha> {
    Ok(estado
        .servico()?
        .definir_backup_externo(ligado, uuid.as_deref())?)
}

/// Versões do backup local (Fase 47, V7.7): nome e instante (do nome), da mais nova
/// para a mais antiga. Nenhum conteúdo nem caminho.
#[tauri::command]
fn listar_backups(estado: State<'_, Estado>) -> Result<Vec<VersaoVista>, Falha> {
    Ok(estado.servico()?.listar_backups()?)
}

/// Restaurar uma versão (Fase 47, V7.7; D17, D39, S9): a versão (formato conferido e
/// presença na lista do daemon), a senha mestra dela (`SenhaMestra`, zera) e o PIN atual
/// (`Pin`, zera). A resposta traz só `restaurado` e a contagem de entradas.
#[tauri::command]
fn restaurar_backup<R: Runtime>(
    dados: Restauracao,
    app: AppHandle<R>,
    estado: State<'_, Estado>,
) -> Result<Restaurado, Falha> {
    let servico = estado.servico()?;
    let r = servico.restaurar_backup(dados)?;
    // A versão traz a configuração dela (D30): a bandeja passa a dizer o tempo dela.
    if let Ok(c) = servico.configuracoes() {
        tempo_da_bandeja(&app, c.minutos_trava);
    }
    Ok(r)
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

/// De quanto em quanto tempo a bandeja consulta o estado (só `estado`, que o daemon não
/// conta como uso: a consulta não segura o cofre aberto). Pega a trava sozinha
/// (inatividade, bloqueio de tela, suspensão, `ward travar`).
const INTERVALO_DA_BANDEJA: Duration = Duration::from_secs(2);

/// O menu da bandeja e o estado que ele mostra. Os itens de texto ficam desabilitados.
struct Bandeja<R: Runtime> {
    estado: Mutex<EstadoBandeja>,
    /// Tempo de trava lido da configuração, só depois de uma ação do usuário
    /// (`bandeja::ler_configuracao_na_passagem`, tela Configurações).
    minutos: Mutex<Option<u32>>,
    titulo: MenuItem<R>,
    subtitulo: MenuItem<R>,
    acao: MenuItem<R>,
    icone: TrayIcon<R>,
}

fn icone_da_bandeja(i: Icone) -> Image<'static> {
    match i {
        Icone::Destravado => tauri::include_image!("icons/bandeja-destravado-32.png"),
        Icone::Travado => tauri::include_image!("icons/bandeja-travado-32.png"),
    }
}

/// Cria o ícone e o menu (frames `Bandeja / Destravado` `17269` e `/ Travado` `17300`).
/// Sem a biblioteca do AppIndicator (o `tray-icon` entraria em pânico, e o release
/// aborta) ou sem pasta segura para o PNG do ícone, não cria: o app segue sem bandeja.
fn criar_bandeja<R: Runtime>(
    app: &AppHandle<R>,
    inicial: EstadoBandeja,
) -> Result<(), &'static str> {
    if !bandeja::biblioteca_do_sistema() {
        return Err("sem a biblioteca do AppIndicator");
    }
    let pasta =
        bandeja::pasta_dos_icones_da_sessao().map_err(|_| "sem pasta segura para o ícone")?;
    let a = bandeja::aparencia(inicial);
    let erro = |_| "menu da bandeja";
    let titulo =
        MenuItem::with_id(app, bandeja::ID_TITULO, a.titulo, false, None::<&str>).map_err(erro)?;
    let subtitulo = MenuItem::with_id(app, bandeja::ID_SUBTITULO, a.subtitulo, false, None::<&str>)
        .map_err(erro)?;
    let abrir = MenuItem::with_id(
        app,
        bandeja::ID_ABRIR,
        bandeja::TEXTO_ABRIR,
        true,
        None::<&str>,
    )
    .map_err(erro)?;
    let acao = MenuItem::with_id(
        app,
        bandeja::ID_ACAO,
        a.acao,
        a.acao_habilitada,
        None::<&str>,
    )
    .map_err(erro)?;
    let sair = MenuItem::with_id(
        app,
        bandeja::ID_SAIR,
        bandeja::TEXTO_SAIR,
        true,
        None::<&str>,
    )
    .map_err(erro)?;
    let sep1 = PredefinedMenuItem::separator(app).map_err(erro)?;
    let sep2 = PredefinedMenuItem::separator(app).map_err(erro)?;
    let menu = Menu::with_items(
        app,
        &[&titulo, &subtitulo, &sep1, &abrir, &acao, &sep2, &sair],
    )
    .map_err(erro)?;
    let icone = TrayIconBuilder::with_id("ward")
        .icon(icone_da_bandeja(a.icone))
        .menu(&menu)
        .show_menu_on_left_click(true)
        .temp_dir_path(pasta)
        .on_menu_event(|app, evento| ao_clicar_na_bandeja(app, evento.id().as_ref()))
        .build(app)
        .map_err(|_| "ícone da bandeja")?;
    app.manage(Bandeja {
        estado: Mutex::new(inicial),
        minutos: Mutex::new(None),
        titulo,
        subtitulo,
        acao,
        icone,
    });
    Ok(())
}

fn estado_mostrado<R: Runtime>(b: &Bandeja<R>) -> std::sync::MutexGuard<'_, EstadoBandeja> {
    b.estado.lock().unwrap_or_else(|e| e.into_inner())
}

/// Consulta o estado e, se mudou, troca textos e ícone da bandeja e avisa a janela
/// principal (que relê o estado e vai para `Destravar` se travou). Sem bandeja, só avisa.
fn atualizar_bandeja<R: Runtime>(app: &AppHandle<R>) {
    let estado = app.state::<Estado>();
    let Ok(s) = estado.servico() else { return };
    let novo = s.estado_da_bandeja();
    let Some(b) = app.try_state::<Bandeja<R>>() else {
        avisar(&estado.canal, JANELA_PRINCIPAL, "estado-mudou");
        return;
    };
    let mut atual = estado_mostrado(&b);
    if *atual == novo {
        return;
    }
    let a = bandeja::aparencia(novo);
    // `configuracao` conta como uso no daemon: só na passagem que o usuário fez
    // (destravou ou criou o cofre), nunca na consulta periódica nem na partida.
    let mut minutos = b.minutos.lock().unwrap_or_else(|e| e.into_inner());
    if bandeja::ler_configuracao_na_passagem(*atual, novo) {
        *minutos = s.configuracoes().ok().map(|c| c.minutos_trava);
    }
    let subtitulo = match novo {
        EstadoBandeja::Destravado => bandeja::subtitulo_destravado(*minutos),
        _ => a.subtitulo.to_owned(),
    };
    drop(minutos);
    let _ = b.titulo.set_text(a.titulo);
    let _ = b.subtitulo.set_text(subtitulo);
    let _ = b.acao.set_text(a.acao);
    let _ = b.acao.set_enabled(a.acao_habilitada);
    let _ = b.icone.set_icon(Some(icone_da_bandeja(a.icone)));
    *atual = novo;
    drop(atual);
    avisar(&estado.canal, JANELA_PRINCIPAL, "estado-mudou");
}

/// O tempo de trava que a tela Configurações leu ou gravou (ação do usuário): o
/// subtítulo do destravado passa a dizê-lo.
fn tempo_da_bandeja<R: Runtime>(app: &AppHandle<R>, minutos_trava: u32) {
    let Some(b) = app.try_state::<Bandeja<R>>() else {
        return;
    };
    *b.minutos.lock().unwrap_or_else(|e| e.into_inner()) = Some(minutos_trava);
    if *estado_mostrado(&b) == EstadoBandeja::Destravado {
        let _ = b
            .subtitulo
            .set_text(bandeja::subtitulo_destravado(Some(minutos_trava)));
    }
}

/// A vigia da bandeja: consulta o estado a cada [`INTERVALO_DA_BANDEJA`].
fn vigiar_bandeja<R: Runtime>(app: AppHandle<R>) {
    let r = std::thread::Builder::new()
        .name("ward-app-bandeja".into())
        .spawn(move || {
            loop {
                std::thread::sleep(INTERVALO_DA_BANDEJA);
                atualizar_bandeja(&app);
            }
        });
    if r.is_err() {
        eprintln!("ward-app: não foi possível vigiar o estado para a bandeja");
    }
}

/// Clique num item (S10, S13): o id é conferido em `bandeja::acao` contra o estado que o
/// menu mostrava; nenhuma ação revela, copia ou aprova. Na thread principal.
fn ao_clicar_na_bandeja<R: Runtime>(app: &AppHandle<R>, id: &str) {
    let Some(b) = app.try_state::<Bandeja<R>>() else {
        return;
    };
    let mostrado = *estado_mostrado(&b);
    match bandeja::acao(id, mostrado) {
        Some(Acao::Abrir | Acao::Destravar) => mostrar_principal(app),
        Some(Acao::Travar) => {
            // A conversa com o daemon fora da thread principal.
            let h = app.clone();
            let _ = std::thread::Builder::new()
                .name("ward-app-travar".into())
                .spawn(move || {
                    if let Ok(s) = h.state::<Estado>().servico()
                        && s.travar().is_err()
                    {
                        eprintln!("ward-app: o daemon não respondeu ao travar");
                    }
                    atualizar_bandeja(&h);
                });
        }
        Some(Acao::Sair) => app.exit(0),
        None => {}
    }
}

/// Mostra a janela principal ou a cria de novo (fechada com a bandeja ativa). Na thread
/// principal.
fn mostrar_principal<R: Runtime>(app: &AppHandle<R>) {
    if let Some(j) = app.get_webview_window(JANELA_PRINCIPAL) {
        let _ = j.unminimize();
        let _ = j.show();
        let _ = j.set_focus();
        return;
    }
    if criar_janela_principal(app).is_err() {
        eprintln!("ward-app: não foi possível abrir a janela principal");
    }
}

fn criar_janela_principal<R: Runtime, M: Manager<R>>(app: &M) -> tauri::Result<WebviewWindow<R>> {
    WebviewWindowBuilder::new(app, JANELA_PRINCIPAL, WebviewUrl::App("index.html".into()))
        .title("Ward")
        .inner_size(1280.0, 800.0)
        .min_inner_size(1024.0, 640.0)
        .theme(Some(tauri::Theme::Dark))
        .disable_drag_drop_handler()
        .devtools(cfg!(feature = "modo-teste"))
        .on_navigation(navegacao_permitida)
        .focused(true)
        .build()
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
            configuracoes,
            trocar_pin,
            trocar_senha_mestra,
            definir_tempo_trava,
            backup_externo,
            listar_pendrives,
            definir_backup_externo,
            listar_backups,
            restaurar_backup,
            pedido_de_aprovacao,
            responder_aprovacao,
            travar
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
    let app = com_comandos(tauri::Builder::default(), estado)
        .on_window_event(|janela, evento| {
            if janela.label() == JANELA_APROVACAO && matches!(evento, WindowEvent::Destroyed) {
                let estado = janela.state::<Estado>();
                travar_canais(&estado.canal).aprovacao = None;
                ao_fechar_a_aprovacao(Arc::clone(&estado.servico));
            }
            // Fase 39: a janela principal fechada (com a bandeja o app continua): o canal
            // dela sai e o que o formulário e o gerador guardavam é zerado. A área de
            // transferência segue com o prazo de 30 s.
            if janela.label() == JANELA_PRINCIPAL && matches!(evento, WindowEvent::Destroyed) {
                let estado = janela.state::<Estado>();
                travar_canais(&estado.canal).principal = None;
                if let Ok(s) = estado.servico() {
                    s.descartar_formulario();
                }
            }
        })
        .setup(move |app| {
            vigiar_avisos(app.handle(), &app.state::<Estado>(), fila.clone());
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
            criar_janela_principal(app)?;
            // Fase 39 (V5.23): a bandeja, com o estado de agora; sem ela o app segue.
            let inicial = app
                .state::<Estado>()
                .servico()
                .map(|s| s.estado_da_bandeja())
                .unwrap_or(EstadoBandeja::SemDaemon);
            match criar_bandeja(app.handle(), inicial) {
                Ok(()) => vigiar_bandeja(app.handle().clone()),
                Err(motivo) => eprintln!("ward-app: sem bandeja ({motivo})"),
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("o app do Ward não pôde iniciar");

    app.run(|handle, evento| match evento {
        // Fase 39: com a bandeja, fechar a última janela não fecha o app ("Sair" fecha,
        // com código).
        RunEvent::ExitRequested {
            code: None, api, ..
        } if handle.try_state::<Bandeja<tauri::Wry>>().is_some() => api.prevent_exit(),
        RunEvent::Exit => {
            if let Ok(s) = handle.state::<Estado>().servico() {
                s.ao_sair();
            }
        }
        _ => {}
    });
}

#[cfg(test)]
mod testes {
    use super::*;
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder};
    use tauri::webview::InvokeRequest;

    /// Sem daemon: um socket num caminho que não existe, com as regras do daemon
    /// instalado (pasta do root). Nunca o `DaemonSocket::padrao()`: no `modo-teste` ele
    /// é o socket de teste do uid, `/run/user/<uid>/ward-teste/`, que pode ter um
    /// `wardd` de teste de outro worktree no ar (e o teste falaria com ele).
    fn sem_daemon() -> DaemonSocket {
        let caminho = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("sem-daemon-{}", std::process::id()))
            .join("wardd.sock");
        assert!(!caminho.exists());
        DaemonSocket::sistema_em(caminho)
    }

    fn app_de_teste() -> tauri::App<MockRuntime> {
        let estado = Estado::novo(Arc::new(montar_servico(Ok(sem_daemon()))));
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

    /// Fase 38 (V5.22, S9): os comandos da tela Configurações só passam pela ACL na
    /// janela `principal` com a origem local; campo a mais e tipo errado são recusados
    /// na leitura, e PIN ou senha fora da regra voltam como erro de campo sem ir ao
    /// daemon e sem valor na resposta.
    #[test]
    fn ataque_configuracoes_so_na_janela_certa_e_conferidas_no_rust() {
        let app = app_de_teste();
        let principal = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let outra = WebviewWindowBuilder::new(&app, "outra", Default::default())
            .build()
            .unwrap();
        let pin = |atual: &str, novo: &str| {
            serde_json::json!({ "dados": {
                "atual": atual, "novo": novo, "confirmacao": novo } })
        };
        let senha = |atual: &str, nova: &str| {
            serde_json::json!({ "dados": {
                "atual": atual, "nova": nova, "confirmacao": nova } })
        };
        for (comando, corpo) in [
            ("configuracoes", serde_json::json!({})),
            ("trocar_pin", pin("135790", "2468")),
            (
                "trocar_senha_mestra",
                senha("falso-mestra-ward-01", "falso-nova-mestra-02"),
            ),
            ("definir_tempo_trava", serde_json::json!({ "minutos": 30 })),
        ] {
            let r = chamar(&outra, comando, ORIGEM, corpo.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{comando} em janela sem capability: {r:?}"
            );
            let r = chamar(&principal, comando, "https://example.com", corpo.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{comando} com origem remota: {r:?}"
            );
        }
        for (atual, novo, campo) in [
            ("135", "2468", "atual"),
            ("135790", "13579a", "novo"),
            ("135790", "1357901", "novo"),
        ] {
            let r = chamar(&principal, "trocar_pin", ORIGEM, pin(atual, novo)).unwrap();
            assert_eq!(r["trocado"], false);
            assert_eq!(r["erros"][0]["campo"], campo, "{atual} {novo}");
            let texto = r.to_string();
            assert!(!texto.contains(atual) && !texto.contains(novo), "{texto}");
        }
        let r = chamar(
            &principal,
            "trocar_senha_mestra",
            ORIGEM,
            senha("falso-mestra-ward-01", "falso-curta"),
        )
        .unwrap();
        assert_eq!(r["erros"][0]["campo"], "nova");
        assert!(!r.to_string().contains("falso"), "{r}");
        let mut forjado = pin("135790", "2468");
        forjado["dados"]["nivel"] = "livre".into();
        let r = chamar(&principal, "trocar_pin", ORIGEM, forjado);
        assert!(
            r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
            "campo a mais: {r:?}"
        );
        for minutos in [
            serde_json::json!("30"),
            serde_json::json!(-1),
            serde_json::Value::Null,
        ] {
            let r = chamar(
                &principal,
                "definir_tempo_trava",
                ORIGEM,
                serde_json::json!({ "minutos": minutos }),
            );
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{minutos}: {r:?}"
            );
        }
        // Fora de 1 a 1440: recusado no serviço, antes do daemon (que nem existe aqui).
        for minutos in [0, 1441] {
            let r = chamar(
                &principal,
                "definir_tempo_trava",
                ORIGEM,
                serde_json::json!({ "minutos": minutos }),
            );
            assert!(
                r.as_ref().err().is_some_and(|e| e["codigo"].is_string()),
                "{minutos}: {r:?}"
            );
        }
    }

    /// Fase 48 (V7.9): os comandos do backup externo só na janela `principal` com a
    /// origem local; o UUID forjado pela interface (fora do formato, ligar sem UUID,
    /// desligar trocando o UUID, tipo errado) é recusado no Rust antes do daemon.
    #[test]
    fn ataque_backup_externo_so_na_janela_certa_e_uuid_conferido_no_rust() {
        let app = app_de_teste();
        let principal = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let outra = WebviewWindowBuilder::new(&app, "outra", Default::default())
            .build()
            .unwrap();
        let aprovacao = WebviewWindowBuilder::new(&app, "aprovacao", Default::default())
            .build()
            .unwrap();
        for (comando, corpo) in [
            ("backup_externo", serde_json::json!({})),
            ("listar_pendrives", serde_json::json!({})),
            (
                "definir_backup_externo",
                serde_json::json!({ "ligado": true, "uuid": "7C1E-42A9" }),
            ),
        ] {
            for janela in [&outra, &aprovacao] {
                let r = chamar(janela, comando, ORIGEM, corpo.clone());
                assert!(
                    r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                    "{comando} em {}: {r:?}",
                    janela.label()
                );
            }
            let r = chamar(&principal, comando, "https://example.com", corpo.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{comando} com origem remota: {r:?}"
            );
        }
        // Formato errado: o núcleo recusa antes de abrir o daemon (`pedido-invalido`,
        // não `daemon-indisponivel`).
        for corpo in [
            serde_json::json!({ "ligado": true, "uuid": "../etc" }),
            serde_json::json!({ "ligado": true, "uuid": "/media/usuario/WARD-BACKUP" }),
            serde_json::json!({ "ligado": true, "uuid": "7C1E-42A9\u{202E}" }),
            serde_json::json!({ "ligado": true, "uuid": "" }),
            serde_json::json!({ "ligado": true }),
            serde_json::json!({ "ligado": false, "uuid": "DEAD-BEEF" }),
        ] {
            let r = chamar(&principal, "definir_backup_externo", ORIGEM, corpo.clone());
            assert_eq!(
                r.as_ref().err().map(|e| e["codigo"].clone()),
                Some(serde_json::json!("pedido-invalido")),
                "{corpo}: {r:?}"
            );
        }
        for corpo in [
            serde_json::json!({ "ligado": "sim", "uuid": "7C1E-42A9" }),
            serde_json::json!({ "ligado": true, "uuid": 4242 }),
            serde_json::json!({ "uuid": "7C1E-42A9" }),
        ] {
            let r = chamar(&principal, "definir_backup_externo", ORIGEM, corpo.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{corpo}: {r:?}"
            );
        }
    }

    /// Fase 47 (V7.7, V7.8): listar e restaurar backup só na janela `principal` com a
    /// origem local; versão forjada (caminho, `..`, fora do formato), PIN fora do
    /// formato, senha vazia e campo a mais (`geracao`) são recusados no Rust antes do
    /// daemon.
    #[test]
    fn ataque_restaurar_backup_so_na_janela_certa_e_versao_conferida_no_rust() {
        let app = app_de_teste();
        let principal = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let outra = WebviewWindowBuilder::new(&app, "outra", Default::default())
            .build()
            .unwrap();
        let aprovacao = WebviewWindowBuilder::new(&app, "aprovacao", Default::default())
            .build()
            .unwrap();
        let dados = |versao: &str, senha: &str, pin: &str| serde_json::json!({ "dados": { "versao": versao, "senha": senha, "pin": pin } });
        let bom = dados(
            "ward-20261108-143200.ward",
            "falso-mestra-ward-01",
            "135790",
        );
        for (comando, corpo) in [
            ("listar_backups", serde_json::json!({})),
            ("restaurar_backup", bom.clone()),
        ] {
            for janela in [&outra, &aprovacao] {
                let r = chamar(janela, comando, ORIGEM, corpo.clone());
                assert!(
                    r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                    "{comando} em {}: {r:?}",
                    janela.label()
                );
            }
            let r = chamar(&principal, comando, "https://example.com", corpo.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{comando} com origem remota: {r:?}"
            );
        }
        // Na janela certa, sem daemon: passa da leitura e do formato e para no socket.
        let r = chamar(&principal, "restaurar_backup", ORIGEM, bom.clone());
        let codigo = r.as_ref().err().map(|e| e["codigo"].clone());
        assert!(
            codigo
                .as_ref()
                .is_some_and(|c| c.is_string() && c != "pedido-invalido"),
            "{r:?}"
        );
        // Versão fora do formato: o núcleo recusa antes de abrir o daemon.
        for versao in [
            "../cofre.ward",
            "/home/usuario/.local/share/ward/cofre.ward",
            "backups/ward-20261108-143200.ward",
            "ward-20261108-143200.ward\u{202E}",
            "",
        ] {
            let corpo = dados(versao, "falso-mestra-ward-01", "135790");
            let r = chamar(&principal, "restaurar_backup", ORIGEM, corpo);
            assert_eq!(
                r.as_ref().err().map(|e| e["codigo"].clone()),
                Some(serde_json::json!("pedido-invalido")),
                "{versao:?}: {r:?}"
            );
        }
        // PIN e senha fora do formato, campo a mais ou faltando: recusados na leitura.
        let mut a_mais = bom.clone();
        a_mais["dados"]["geracao"] = serde_json::json!(9);
        for corpo in [
            dados("ward-20261108-143200.ward", "falso-mestra-ward-01", "12ab"),
            dados("ward-20261108-143200.ward", "falso-mestra-ward-01", "123"),
            dados("ward-20261108-143200.ward", "", "135790"),
            a_mais,
            serde_json::json!({ "dados": { "versao": "ward-20261108-143200.ward" } }),
            serde_json::json!({ "versao": "ward-20261108-143200.ward" }),
        ] {
            let r = chamar(&principal, "restaurar_backup", ORIGEM, corpo.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{corpo}: {r:?}"
            );
            // A recusa não ecoa o que veio.
            let texto = format!("{r:?}");
            assert!(
                !texto.contains("falso-mestra") && !texto.contains("135790"),
                "{texto}"
            );
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

    /// Fase 39 (V5.23): `travar` passa pela ACL só na janela principal e com a origem
    /// local; a janela de aprovação e uma janela qualquer não travam (nem com argumento
    /// inventado). Sem daemon, o serviço responde o erro dele, com código.
    #[test]
    fn ataque_travar_so_na_janela_principal() {
        let app = app_de_teste();
        let principal = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let r = chamar(&principal, "travar", ORIGEM, serde_json::json!({}));
        assert!(
            r.as_ref().err().is_some_and(|e| e["codigo"].is_string()),
            "erro do serviço, não da ACL: {r:?}"
        );
        let r = chamar(
            &principal,
            "travar",
            "https://example.com",
            serde_json::json!({}),
        );
        assert!(
            r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
            "origem remota: {r:?}"
        );
        for rotulo in ["aprovacao", "outra"] {
            let janela = WebviewWindowBuilder::new(&app, rotulo, Default::default())
                .build()
                .unwrap();
            let r = chamar(&janela, "travar", ORIGEM, serde_json::json!({}));
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{rotulo}: {r:?}"
            );
        }
    }

    /// Fase 39: sem a bandeja (o teste não cria uma), atualizar só avisa a principal e
    /// não entra em pânico; um clique forjado sem bandeja não faz nada.
    #[test]
    fn bandeja_ausente_nao_quebra() {
        let app = app_de_teste();
        atualizar_bandeja(app.handle());
        ao_clicar_na_bandeja(app.handle(), bandeja::ID_SAIR);
        ao_clicar_na_bandeja(app.handle(), "inventado");
        assert!(app.try_state::<Bandeja<MockRuntime>>().is_none());
    }
}
