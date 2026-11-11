//! Ward desktop app (Tauri 2). Only the window and the command wiring: what each command
//! does is in `ward_app_core::service` (the same one the test bridge uses).
//!
//! S11: capabilities only with the app's own commands (`capabilities/principal.json` for
//! the main window, the approval capability file for the approval window; manifest in
//! `build.rs`), strict CSP in `tauri.conf.json`, embedded interface (`custom-protocol`),
//! navigation only inside the app, no file drag into the window, devtools only with the
//! `test-mode` feature.
//!
//! D47/D57: the first thing in `main` is the startup of the setgid app
//! (`ward_app_core::startup`), before GTK: D57 environment checked, connection to the
//! daemon with the egid of the `ward-app` group, group dropped, non-dumpable again and
//! the parent checked. Otherwise the app exits with the message, without opening a
//! window.
//!
//! Phase 36 (S10, D65 a): the notice queue watcher takes each approval request notice and
//! opens (or brings to the front) the approval window, always on top. Only that window
//! answers (its own capability, and the command checks the label again); closing the
//! window denies every pending request.
//!
//! Phase 39 (V5.23): tray icon and menu (AppIndicator, `ward_app_core::tray`): open,
//! lock now / unlock, and quit. With the tray active, closing the main
//! window destroys it (the webview and what the screen had go away) and the app stays in
//! the tray; the open item or an unlock request notice create it again. Without the
//! AppIndicator library or without a safe folder for the icon, the app opens without a
//! tray and closing the window closes the app, as before.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod gtk_clipboard;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gtk_clipboard::GtkClipboard;
use serde::Serialize;
use tauri::image::Image;
use tauri::ipc::Channel;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{
    AppHandle, Builder, Manager, RunEvent, Runtime, State, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};
use ward_app_core::AppError;
use ward_app_core::approval::{self, ApprovalResult, DisplayedRequest};
use ward_app_core::attachments::{AttachmentEdit, Choice, Picker};
use ward_app_core::clipboard::Copier;
use ward_app_core::credentials::{CredentialDetail, CredentialItem};
use ward_app_core::daemon::DaemonSocket;
use ward_app_core::external_backup::{ExternalBackupView, UsbDriveView};
use ward_app_core::first_use::NewVault;
use ward_app_core::form::Form;
use ward_app_core::generator::Options;
use ward_app_core::key::Loading;
use ward_app_core::level::{LevelChanged, Pin};
use ward_app_core::restore::{Restore, Restored, VersionView};
use ward_app_core::service::{CreateResult, SaveResult, Service, TotpCode};
use ward_app_core::settings::{ChangeResult, PasswordChange, PinChange, Settings};
use ward_app_core::startup;
use ward_app_core::terminal::SystemTerminal;
use ward_app_core::tray::{self, Action, Icon, TrayState};
use ward_app_core::unlock::{self, MasterPassword, RequestInfo};

type AppService = Service<DaemonSocket, GtkClipboard>;

/// What the interface gets on an error: a stable code and a fixed message, no value.
#[derive(Serialize)]
struct Failure {
    #[serde(rename = "codigo")]
    code: &'static str,
    #[serde(rename = "mensagem")]
    message: String,
}

impl From<AppError> for Failure {
    fn from(e: AppError) -> Self {
        Failure {
            code: e.code(),
            message: e.to_string(),
        }
    }
}

/// Phase 31: with the vault locked, `request` is the agent's request waiting for the
/// unlock (id, seconds left and, Phase 39a, entry, operation and command with D51).
#[derive(Serialize)]
struct StatusResponse {
    #[serde(rename = "estado")]
    status: &'static str,
    #[serde(rename = "pedido")]
    request: Option<RequestInfo>,
}

/// What the notice channel takes to the screen: only the notice name, no data (D65 a).
#[derive(Clone, Serialize)]
struct ScreenNotice {
    #[serde(rename = "aviso")]
    notice: &'static str,
}

/// Tauri channels the screens opened with the listen command, one per window (the newest
/// of each one counts). The main one gets the unlock request notice (Phase 31); the
/// approval one, the approval request notice (Phase 36). No notice carries data: whoever gets it asks.
#[derive(Default)]
struct Channels {
    main: Option<Channel<ScreenNotice>>,
    approval: Option<Channel<ScreenNotice>>,
}

type NoticeChannels = Arc<Mutex<Channels>>;

fn lock_channels(channels: &NoticeChannels) -> std::sync::MutexGuard<'_, Channels> {
    channels.lock().unwrap_or_else(|e| e.into_inner())
}

/// Sends the notice (only the name) through the window's channel, if it opened one.
fn notify(channels: &NoticeChannels, window: &str, notice: &'static str) {
    let c = lock_channels(channels);
    let channel = match window {
        MAIN_WINDOW => c.main.as_ref(),
        APPROVAL_WINDOW => c.approval.as_ref(),
        _ => None,
    };
    if let Some(channel) = channel {
        let _ = channel.send(ScreenNotice { notice });
    }
}

/// Service built at startup. Without the daemon, the commands answer the error (the app
/// opens and shows the failure). `Arc`: the connection watcher clears the clipboard
/// before re-executing the app.
struct AppState {
    service: Arc<Result<AppService, AppError>>,
    channels: NoticeChannels,
    /// The last vault status the watcher saw (Phase 39g), with or without a tray: it is
    /// through it that a lock coming from outside clears the clipboard and the form right
    /// away. Before, it only existed inside the `Tray`, and without a tray nothing
    /// cleared.
    seen: Mutex<TrayState>,
}

impl AppState {
    fn new(service: Arc<Result<AppService, AppError>>) -> AppState {
        AppState {
            service,
            channels: Arc::new(Mutex::new(Channels::default())),
            seen: Mutex::new(TrayState::NoDaemon),
        }
    }

    fn service(&self) -> Result<&AppService, Failure> {
        self.service
            .as_ref()
            .as_ref()
            .map_err(|e| Failure::from(*e))
    }
}

#[tauri::command]
fn estado_do_cofre(state: State<'_, AppState>) -> Result<StatusResponse, Failure> {
    let s = state.service()?;
    let e = s.vault_status()?;
    Ok(StatusResponse {
        status: e.code(),
        request: s.request_info(e),
    })
}

/// Unlock (Phase 31). The password arrives in a `MasterPassword` (zeroed on drop; empty
/// or above the limit is refused on reading, without echo) and only goes to the daemon;
/// the response neither carries it nor says how many attempts there were (D20).
#[tauri::command]
fn destravar(senha: MasterPassword, state: State<'_, AppState>) -> Result<(), Failure> {
    Ok(state.service()?.unlock(senha)?)
}

/// "Lock now" of the side navigation (Phase 39): clears the clipboard and the
/// form, locks, and the tray starts showing locked.
#[tauri::command]
fn travar<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>) -> Result<(), Failure> {
    let r = state.service()?.lock();
    update_tray(&app);
    Ok(r?)
}

/// D65 a: the screen passes the channel through which the Rust side notifies it: the
/// main one, of an unlock request; the approval one (Phase 36), of an approval
/// request. The channel stays with the window that opened it (label given
/// by Tauri, not by the screen), and a window does not get the other's notice. Tauri
/// channel (not `emit`/`listen`): the capability gets no core permission
/// (`core:event:*`).
#[tauri::command]
fn ouvir_avisos<R: Runtime>(
    canal: Channel<ScreenNotice>,
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<(), Failure> {
    let mut c = lock_channels(&state.channels);
    match window.label() {
        MAIN_WINDOW => c.main = Some(canal),
        APPROVAL_WINDOW => c.approval = Some(canal),
        _ => return Err(Failure::from(AppError::Unauthorized)),
    }
    Ok(())
}

/// The connection's notice queue watcher (D65 a): each unlock request notice marks the
/// request in the service and notifies the screen through the channel, if it already
/// opened one. Phase 39: with the app only in the tray, the request also brings the main
/// window back (D16: the app opens and asks for the master password).
fn watch_notices<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    queue: Option<Arc<ward_app_core::daemon::NoticeQueue>>,
) {
    let (Some(queue), Ok(s)) = (queue, state.service()) else {
        return;
    };
    let channels = Arc::clone(&state.channels);
    let h = app.clone();
    let r = unlock::watch(queue, s.unlock_request(), move || {
        let h2 = h.clone();
        let _ = h.run_on_main_thread(move || show_main(&h2));
        notify(&channels, MAIN_WINDOW, "pedido-de-destrave");
    });
    if r.is_err() {
        eprintln!("ward-app: não foi possível vigiar os avisos do daemon");
    }
}

/// D4: the value goes from the daemon to the clipboard here; the response does not carry
/// it.
#[tauri::command]
fn copiar_campo(entrada: String, campo: String, state: State<'_, AppState>) -> Result<(), Failure> {
    Ok(state.service()?.copy_field(&entrada, &campo)?)
}

/// Credentials screen (Phase 32): the list, no secret.
#[tauri::command]
fn listar_credenciais(state: State<'_, AppState>) -> Result<Vec<CredentialItem>, Failure> {
    Ok(state.service()?.list_credentials()?)
}

/// Credentials screen (Phase 32): the detail of an entry, no secret (only the names of
/// the secret fields). The name comes from the interface and is validated in the core.
#[tauri::command]
fn detalhe_credencial(
    entrada: String,
    state: State<'_, AppState>,
) -> Result<CredentialDetail, Failure> {
    Ok(state.service()?.credential_detail(&entrada)?)
}

#[derive(Serialize)]
struct GeneratedResponse {
    #[serde(rename = "tamanho")]
    size: usize,
    bits: u32,
    #[serde(rename = "forca")]
    strength: &'static str,
    #[serde(rename = "simbolos")]
    symbols: &'static str,
}

/// Standalone generator (Phase 37): the password is born in `ward_core::generator` and
/// stays in Rust; the response does not carry the value. The arguments come from the
/// interface (untrusted) and are checked in the core (size 8–64, at least one set).
#[tauri::command]
fn gerar_senha(
    tamanho: u64,
    minusculas: bool,
    maiusculas: bool,
    digitos: bool,
    simbolos: bool,
    state: State<'_, AppState>,
) -> Result<GeneratedResponse, Failure> {
    let g = state.service()?.generate_password(Options {
        size: tamanho,
        lowercase: minusculas,
        uppercase: maiusculas,
        digits: digitos,
        symbols: simbolos,
    })?;
    Ok(GeneratedResponse {
        size: g.size,
        bits: g.bits,
        strength: g.strength.code(),
        symbols: g.symbols,
    })
}

/// D4: the standalone password goes to the clipboard here; the response does not carry
/// it.
#[tauri::command]
fn copiar_senha_gerada(state: State<'_, AppState>) -> Result<(), Failure> {
    Ok(state.service()?.copy_generated_password()?)
}

#[derive(Serialize)]
struct RevealedResponse {
    #[serde(rename = "valor")]
    value: String,
}

/// S11: the only command that takes the value to the webview, on the "reveal" click.
#[tauri::command]
fn revelar_senha_gerada(state: State<'_, AppState>) -> Result<RevealedResponse, Failure> {
    let r = state.service()?.reveal_generated_password()?;
    Ok(RevealedResponse {
        value: r.expose().to_owned(),
    })
}

#[tauri::command]
fn descartar_senha_gerada(state: State<'_, AppState>) -> Result<(), Failure> {
    Ok(state.service()?.discard_generated_password()?)
}

/// S11 (D65 b): the value of an entry secret, only on the "reveal" click.
#[tauri::command]
fn revelar_campo(
    entrada: String,
    campo: String,
    state: State<'_, AppState>,
) -> Result<RevealedResponse, Failure> {
    let r = state.service()?.reveal_field(&entrada, &campo)?;
    Ok(RevealedResponse {
        value: r.expose().to_owned(),
    })
}

/// Form (Phase 33): the current TOTP code; the seed does not leave the daemon.
#[tauri::command]
fn codigo_totp(entrada: String, state: State<'_, AppState>) -> Result<TotpCode, Failure> {
    Ok(state.service()?.totp_code(&entrada)?)
}

/// D4: the TOTP code goes to the clipboard here.
#[tauri::command]
fn copiar_codigo_totp(entrada: String, state: State<'_, AppState>) -> Result<(), Failure> {
    Ok(state.service()?.copy_totp_code(&entrada)?)
}

#[derive(Serialize)]
struct UsedResponse {
    #[serde(rename = "tamanho")]
    size: usize,
}

/// "Use this password": the generated password goes to the field on the Rust side; only
/// the size comes back.
#[tauri::command]
fn usar_senha_gerada(campo: String, state: State<'_, AppState>) -> Result<UsedResponse, Failure> {
    let size = state.service()?.use_generated_password(&campo)?;
    Ok(UsedResponse { size })
}

#[tauri::command]
fn descartar_formulario(state: State<'_, AppState>) -> Result<(), Failure> {
    state.service()?.discard_form();
    Ok(())
}

/// Form (Phase 33): creates or edits. The form comes from the interface (untrusted) and
/// is checked in the core; the response is the saved name or the errors per field.
#[tauri::command]
fn salvar_credencial(
    anterior: Option<String>,
    formulario: Form,
    state: State<'_, AppState>,
) -> Result<SaveResult, Failure> {
    Ok(state
        .service()?
        .save_credential(anterior.as_deref(), formulario)?)
}

/// Access level (Phase 35, S9). The PIN arrives in a `Pin` (zeroed on drop; the format is
/// checked on reading, before the service) and only goes to the daemon; the response is
/// the new level.
#[tauri::command]
fn mudar_nivel(
    entrada: String,
    nivel: String,
    pin: Pin,
    state: State<'_, AppState>,
) -> Result<LevelChanged, Failure> {
    Ok(state.service()?.set_level(&entrada, &nivel, pin)?)
}

/// Attachment (Phase 34): the GTK file dialog chooses the file and the core reads it;
/// the response carries id, name and size or the notice, never the content (S11, V5.12).
/// Synchronous command: runs on the main thread, where GTK can open the dialog.
/// Phase 39b: when editing, the edit argument (entry and kept attachments; the ones that
/// remain count in the limits; the service checks the names in the detail).
#[tauri::command]
fn escolher_anexo(
    edicao: Option<AttachmentEdit>,
    state: State<'_, AppState>,
) -> Result<Choice, Failure> {
    Ok(state.service()?.choose_attachment(edicao.as_ref())?)
}

#[tauri::command]
fn remover_anexo(id: u64, state: State<'_, AppState>) -> Result<(), Failure> {
    Ok(state.service()?.remove_attachment(id)?)
}

/// "Load from file" of the private key (Phase 53a, S2, S11): the GTK dialog chooses
/// the file and the core reads and checks it (no link, regular file only, up to 64 KiB,
/// OpenSSH/PEM key only). The key stays on the Rust side until saving; the response
/// carries only the size or the notice. Synchronous, like the choose attachment command (GTK opens
/// the dialog on the main thread).
#[tauri::command]
fn carregar_chave(campo: String, state: State<'_, AppState>) -> Result<Loading, Failure> {
    Ok(state.service()?.load_key(&campo)?)
}

/// "Connect" (D3): single-use token and the default terminal; the token does not come
/// back.
#[tauri::command]
fn conectar(entrada: String, state: State<'_, AppState>) -> Result<(), Failure> {
    Ok(state.service()?.connect(&entrada)?)
}

/// First use (Phase 30): master password, PIN and the confirmations, checked in the
/// core; the daemon stores only the PIN hash (D30). The response carries no value.
#[tauri::command]
fn criar_cofre(dados: NewVault, state: State<'_, AppState>) -> Result<CreateResult, Failure> {
    Ok(state.service()?.create_vault(dados)?)
}

/// Settings (Phase 38): lock timeout and external backup, no hash or UUID.
#[tauri::command]
fn configuracoes<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<Settings, Failure> {
    let c = state.service()?.settings()?;
    // Phase 39: the screen opened (user action); the tray reuses the time read.
    tray_lock_time(&app, c.lock_minutes);
    Ok(c)
}

/// Change the PIN (Phase 38, V5.22, S9): current, new and confirmation in `Typed`
/// (zeroed); checked in the core, the current one by the daemon. The response carries no
/// value.
#[tauri::command]
fn trocar_pin(dados: PinChange, state: State<'_, AppState>) -> Result<ChangeResult, Failure> {
    Ok(state.service()?.change_pin(dados)?)
}

/// Change the master password (Phase 38, V5.22): the current one checked by the daemon;
/// the new one with the first use rule. The response carries no value.
#[tauri::command]
fn trocar_senha_mestra(
    dados: PasswordChange,
    state: State<'_, AppState>,
) -> Result<ChangeResult, Failure> {
    Ok(state.service()?.change_master_password(dados)?)
}

/// Lock timeout (Phase 38, D30, D45): 1 to 1440 minutes, stored in the vault by the
/// daemon.
#[tauri::command]
fn definir_tempo_trava<R: Runtime>(
    minutos: u64,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<Settings, Failure> {
    let c = state.service()?.set_lock_timeout(minutos)?;
    // Phase 39: the tray subtitle starts saying the new time.
    tray_lock_time(&app, c.lock_minutes);
    Ok(c)
}

/// External backup (Phase 48, V7.9): on or off, the UUID and, if connected, the USB drive
/// (neutralized label, D51). No mount path.
#[tauri::command]
fn backup_externo(state: State<'_, AppState>) -> Result<ExternalBackupView, Failure> {
    Ok(state.service()?.external_backup()?)
}

/// The USB drives the daemon sees, to choose the external backup one (Phase 48).
#[tauri::command]
fn listar_pendrives(state: State<'_, AppState>) -> Result<Vec<UsbDriveView>, Failure> {
    Ok(state.service()?.list_usb_drives()?)
}

/// Turn on (with a USB drive the daemon sees now; the UUID from the interface is checked
/// again in the core) or turn off the external backup (Phase 48, D17, D30).
#[tauri::command]
fn definir_backup_externo(
    ligado: bool,
    uuid: Option<String>,
    state: State<'_, AppState>,
) -> Result<ExternalBackupView, Failure> {
    Ok(state
        .service()?
        .set_external_backup(ligado, uuid.as_deref())?)
}

/// Versions of the local backup (Phase 47, V7.7): name and instant (from the name), from
/// the newest to the oldest. No content or path.
#[tauri::command]
fn listar_backups(state: State<'_, AppState>) -> Result<Vec<VersionView>, Failure> {
    Ok(state.service()?.list_backups()?)
}

/// Restore a version (Phase 47, V7.7; D17, D39, S9): the version (format checked and
/// present in the daemon's list), its master password (`MasterPassword`, zeroed) and the
/// current PIN (`Pin`, zeroed). The response carries only the restored flag and the entry
/// count.
#[tauri::command]
fn restaurar_backup<R: Runtime>(
    dados: Restore,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<Restored, Failure> {
    let service = state.service()?;
    let r = service.restore_backup(dados)?;
    // The version carries its own config (D30): the tray starts saying its time.
    if let Ok(c) = service.settings() {
        tray_lock_time(&app, c.lock_minutes);
    }
    Ok(r)
}

/// Label of the main window (capability `principal`).
const MAIN_WINDOW: &str = "principal";
/// Label of the approval window: the approval capability applies only to it.
const APPROVAL_WINDOW: &str = "aprovacao";

/// Second layer beyond the ACL: the approval commands only answer in the right window.
fn in_approval_window<R: Runtime>(window: &WebviewWindow<R>) -> Result<(), Failure> {
    if window.label() == APPROVAL_WINDOW {
        Ok(())
    } else {
        Err(Failure::from(AppError::Unauthorized))
    }
}

/// Approval window (Phase 36): the current request, with the agent's text already
/// neutralized (D51), or `null`. Without a request, the window closes (there is nothing
/// to deny).
#[tauri::command]
fn pedido_de_aprovacao<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<Option<DisplayedRequest>, Failure> {
    in_approval_window(&window)?;
    let p = state.service()?.approval_request()?;
    if p.is_none() {
        let _ = window.destroy();
    }
    Ok(p)
}

/// Approval window (Phase 36, S9, S10): Deny without a PIN, the three Allow with the PIN
/// (in a `Pin` that is zeroed; out of format is refused on reading, without echo). The
/// response does not carry the PIN. With no other request waiting, the window closes.
/// Phase 54b (D68): the acknowledged argument is the stdin checkbox (absent = unchecked); the service and
/// the daemon check it.
#[tauri::command]
fn responder_aprovacao<R: Runtime>(
    id: u64,
    decisao: String,
    pin: Option<Pin>,
    ciente: Option<bool>,
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<ApprovalResult, Failure> {
    in_approval_window(&window)?;
    let r = state
        .service()?
        .answer_approval(id, &decisao, pin, ciente == Some(true))?;
    if !r.has_other {
        let _ = window.destroy();
    }
    Ok(r)
}

/// Opens the approval window or brings it to the front (S10: it opens in the foreground).
/// On the main thread (the caller uses `run_on_main_thread`).
fn open_approval_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(j) = app.get_webview_window(APPROVAL_WINDOW) {
        let _ = j.unminimize();
        let _ = j.show();
        let _ = j.set_always_on_top(true);
        let _ = j.set_focus();
        return;
    }
    let r = WebviewWindowBuilder::new(app, APPROVAL_WINDOW, WebviewUrl::App("index.html".into()))
        .title("Pedido de aprovação · Ward")
        .inner_size(560.0, 880.0)
        .min_inner_size(480.0, 560.0)
        .center()
        .always_on_top(true)
        .focused(true)
        .theme(Some(tauri::Theme::Dark))
        .disable_drag_drop_handler()
        .devtools(cfg!(feature = "test-mode"))
        .on_navigation(navigation_allowed)
        .build();
    if r.is_err() {
        // Without the window, nobody answers: the request expires and is denied in the
        // daemon (S10).
        eprintln!("ward-app: não foi possível abrir a janela de aprovação");
    }
}

/// Closing the approval window denies (S10). In another thread: the conversation with
/// the daemon does not hold the event loop.
fn on_approval_closed(service: Arc<Result<AppService, AppError>>) {
    let _ = std::thread::Builder::new()
        .name("ward-app-negar".into())
        .spawn(move || {
            if let Ok(s) = service.as_ref() {
                s.deny_pending_approvals();
            }
        });
}

/// The GTK "Add file" dialog, in the app window.
struct GtkPicker;

impl Picker for GtkPicker {
    fn choose(&self) -> Result<Option<PathBuf>, AppError> {
        file_dialog("Adicionar arquivo", "Anexar")
    }

    /// Phase 53a: the same dialog, with the title and the button of "Load from
    /// file".
    fn choose_key(&self) -> Result<Option<PathBuf>, AppError> {
        file_dialog("Carregar chave privada", "Carregar")
    }
}

/// The GTK open file dialog, modal, a single file, local only. `Ok(None)`: canceled.
fn file_dialog(title: &str, button: &str) -> Result<Option<PathBuf>, AppError> {
    use gtk::prelude::{DialogExt, FileChooserExt, GtkSettingsExt, GtkWindowExt};
    if !gtk::is_initialized_main_thread() {
        return Err(AppError::PickerUnavailable);
    }
    // Phase 53d: the dialog does not store the chosen path (the key in the in-memory
    // folder, an attachment) in the user's `recently-used.xbel`. The app's value beats
    // the system's (XSETTINGS, `settings.ini`); checked back, and without it the dialog
    // does not open.
    let Some(gtk_settings) = gtk::Settings::default() else {
        return Err(AppError::PickerUnavailable);
    };
    gtk_settings.set_gtk_recent_files_enabled(false);
    if gtk_settings.is_gtk_recent_files_enabled() {
        return Err(AppError::PickerUnavailable);
    }
    let dialog = gtk::FileChooserDialog::with_buttons(
        Some(title),
        None::<&gtk::Window>,
        gtk::FileChooserAction::Open,
        &[
            ("Cancelar", gtk::ResponseType::Cancel),
            (button, gtk::ResponseType::Accept),
        ],
    );
    dialog.set_modal(true);
    dialog.set_select_multiple(false);
    dialog.set_local_only(true);
    let response = dialog.run();
    let file = (response == gtk::ResponseType::Accept)
        .then(|| dialog.filename())
        .flatten();
    // `close` requests closing like the window button: the dialog is destroyed.
    dialog.close();
    Ok(file)
}

/// How often the tray asks for the status (only the status request, which the daemon does not count
/// as use: the query does not keep the vault open). Catches the lock by itself
/// (inactivity, screen lock, suspend, the CLI lock command).
const TRAY_INTERVAL: Duration = Duration::from_secs(2);

/// The tray menu and the status it shows. The text items stay disabled.
struct Tray<R: Runtime> {
    state: Mutex<TrayState>,
    /// Lock timeout read from the config, only after a user action
    /// (`tray::read_settings_on_transition`, Settings screen).
    minutes: Mutex<Option<u32>>,
    title: MenuItem<R>,
    subtitle: MenuItem<R>,
    action: MenuItem<R>,
    icon: TrayIcon<R>,
}

fn tray_icon_image(i: Icon) -> Image<'static> {
    match i {
        Icon::Unlocked => tauri::include_image!("icons/tray-unlocked-32.png"),
        Icon::Locked => tauri::include_image!("icons/tray-locked-32.png"),
    }
}

/// Creates the icon and the menu (tray frames, unlocked `17269` and locked
/// `17300`). Without the AppIndicator library (`tray-icon` would panic, and the release
/// aborts) or without a safe folder for the icon PNG, does not create: the app goes on
/// without a tray.
fn create_tray<R: Runtime>(app: &AppHandle<R>, initial: TrayState) -> Result<(), &'static str> {
    if !tray::system_library() {
        return Err("sem a biblioteca do AppIndicator");
    }
    let dir = tray::session_icons_dir().map_err(|_| "sem pasta segura para o ícone")?;
    let a = tray::appearance(initial);
    let error = |_| "menu da bandeja";
    let title =
        MenuItem::with_id(app, tray::ID_TITLE, a.title, false, None::<&str>).map_err(error)?;
    let subtitle = MenuItem::with_id(app, tray::ID_SUBTITLE, a.subtitle, false, None::<&str>)
        .map_err(error)?;
    let open = MenuItem::with_id(app, tray::ID_OPEN, tray::OPEN_TEXT, true, None::<&str>)
        .map_err(error)?;
    let action = MenuItem::with_id(
        app,
        tray::ID_ACTION,
        a.action,
        a.action_enabled,
        None::<&str>,
    )
    .map_err(error)?;
    let quit = MenuItem::with_id(app, tray::ID_QUIT, tray::QUIT_TEXT, true, None::<&str>)
        .map_err(error)?;
    let sep1 = PredefinedMenuItem::separator(app).map_err(error)?;
    let sep2 = PredefinedMenuItem::separator(app).map_err(error)?;
    let menu = Menu::with_items(
        app,
        &[&title, &subtitle, &sep1, &open, &action, &sep2, &quit],
    )
    .map_err(error)?;
    let icon = TrayIconBuilder::with_id("ward")
        .icon(tray_icon_image(a.icon))
        .menu(&menu)
        .show_menu_on_left_click(true)
        .temp_dir_path(dir)
        .on_menu_event(|app, event| on_tray_click(app, event.id().as_ref()))
        .build(app)
        .map_err(|_| "ícone da bandeja")?;
    app.manage(Tray {
        state: Mutex::new(initial),
        minutes: Mutex::new(None),
        title,
        subtitle,
        action,
        icon,
    });
    Ok(())
}

fn shown_state<R: Runtime>(b: &Tray<R>) -> std::sync::MutexGuard<'_, TrayState> {
    b.state.lock().unwrap_or_else(|e| e.into_inner())
}

/// Asks for the status and, if it changed, swaps the tray texts and icon and notifies the
/// main window (which reads the status again and goes to the unlock screen if it locked).
/// Without a tray, only notifies.
///
/// Phases 53d and 39g: a lock coming from outside (inactivity, the CLI lock command, locked
/// screen) clears the clipboard and zeroes the used passwords and the loaded keys of the
/// form right away, with or without a tray: the transition is measured by
/// `AppState::seen`, before looking at whether there is a tray.
fn update_tray<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    let Ok(s) = state.service() else { return };
    let (before, new) = s.check_transition(&state.seen);
    let Some(b) = app.try_state::<Tray<R>>() else {
        if before != new {
            notify(&state.channels, MAIN_WINDOW, "estado-mudou");
        }
        return;
    };
    let mut current = shown_state(&b);
    if *current == new {
        return;
    }
    let a = tray::appearance(new);
    // The settings request counts as use in the daemon: only on the transition the user made
    // (unlocked or created the vault), never on the periodic query nor at startup.
    let mut minutes = b.minutes.lock().unwrap_or_else(|e| e.into_inner());
    if tray::read_settings_on_transition(*current, new) {
        *minutes = s.settings().ok().map(|c| c.lock_minutes);
    }
    let subtitle = match new {
        TrayState::Unlocked => tray::unlocked_subtitle(*minutes),
        _ => a.subtitle.to_owned(),
    };
    drop(minutes);
    let _ = b.title.set_text(a.title);
    let _ = b.subtitle.set_text(subtitle);
    let _ = b.action.set_text(a.action);
    let _ = b.action.set_enabled(a.action_enabled);
    let _ = b.icon.set_icon(Some(tray_icon_image(a.icon)));
    *current = new;
    drop(current);
    notify(&state.channels, MAIN_WINDOW, "estado-mudou");
}

/// The lock timeout the Settings screen read or wrote (user action): the unlocked
/// subtitle starts saying it.
fn tray_lock_time<R: Runtime>(app: &AppHandle<R>, lock_minutes: u32) {
    let Some(b) = app.try_state::<Tray<R>>() else {
        return;
    };
    *b.minutes.lock().unwrap_or_else(|e| e.into_inner()) = Some(lock_minutes);
    if *shown_state(&b) == TrayState::Unlocked {
        let _ = b
            .subtitle
            .set_text(tray::unlocked_subtitle(Some(lock_minutes)));
    }
}

/// The tray watcher: asks for the status every [`TRAY_INTERVAL`].
fn watch_tray<R: Runtime>(app: AppHandle<R>) {
    let r = std::thread::Builder::new()
        .name("ward-app-bandeja".into())
        .spawn(move || {
            loop {
                std::thread::sleep(TRAY_INTERVAL);
                update_tray(&app);
            }
        });
    if r.is_err() {
        eprintln!("ward-app: não foi possível vigiar o estado para a bandeja");
    }
}

/// Click on an item (S10, S13): the id is checked in `tray::action` against the status
/// the menu showed; no action reveals, copies or approves. On the main thread.
fn on_tray_click<R: Runtime>(app: &AppHandle<R>, id: &str) {
    let Some(b) = app.try_state::<Tray<R>>() else {
        return;
    };
    let shown = *shown_state(&b);
    match tray::action(id, shown) {
        Some(Action::Open | Action::Unlock) => show_main(app),
        Some(Action::Lock) => {
            // The conversation with the daemon off the main thread.
            let h = app.clone();
            let _ = std::thread::Builder::new()
                .name("ward-app-travar".into())
                .spawn(move || {
                    if let Ok(s) = h.state::<AppState>().service()
                        && s.lock().is_err()
                    {
                        eprintln!("ward-app: o daemon não respondeu ao travar");
                    }
                    update_tray(&h);
                });
        }
        Some(Action::Quit) => app.exit(0),
        None => {}
    }
}

/// Shows the main window or creates it again (closed with the tray active). On the main
/// thread.
fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(j) = app.get_webview_window(MAIN_WINDOW) {
        let _ = j.unminimize();
        let _ = j.show();
        let _ = j.set_focus();
        return;
    }
    if create_main_window(app).is_err() {
        eprintln!("ward-app: não foi possível abrir a janela principal");
    }
}

/// The GTK title bar (CSD). The app starts with the environment built by the daemon
/// (D57), without the session settings: GTK 3 falls back to the default `menu:close`
/// layout (close only) and to the light theme. Here go the three buttons, the dark theme
/// and the app background color (the neutral 950 primitive token), for both windows. Fixed CSS,
/// no data.
fn adjust_title_bar() {
    use gtk::prelude::{CssProviderExt, GtkSettingsExt};
    if !gtk::is_initialized_main_thread() {
        return;
    }
    if let Some(gtk_settings) = gtk::Settings::default() {
        gtk_settings.set_gtk_decoration_layout(Some("menu:minimize,maximize,close"));
        gtk_settings.set_gtk_application_prefer_dark_theme(true);
    }
    let css = gtk::CssProvider::new();
    let style = b"headerbar, .titlebar { background: #101114; background-image: none; \
color: #F7F8FA; border: none; box-shadow: none; } \
headerbar .title { color: #F7F8FA; } \
headerbar button.titlebutton { color: #9AA1AE; background: none; border: none; \
box-shadow: none; } \
headerbar button.titlebutton:hover { color: #F7F8FA; background: #24282F; }";
    if css.load_from_data(style).is_err() {
        return;
    }
    if let Some(screen) = gtk::gdk::Screen::default() {
        gtk::StyleContext::add_provider_for_screen(
            &screen,
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn create_main_window<R: Runtime, M: Manager<R>>(app: &M) -> tauri::Result<WebviewWindow<R>> {
    WebviewWindowBuilder::new(app, MAIN_WINDOW, WebviewUrl::App("index.html".into()))
        .title("Ward")
        .inner_size(1280.0, 800.0)
        .min_inner_size(1024.0, 640.0)
        .theme(Some(tauri::Theme::Dark))
        .disable_drag_drop_handler()
        .devtools(cfg!(feature = "test-mode"))
        .on_navigation(navigation_allowed)
        .focused(true)
        .build()
}

fn build_service(daemon: Result<DaemonSocket, AppError>) -> Result<AppService, AppError> {
    let daemon = daemon?;
    // Phase 39f (S2): the copied value stays in the app memory (GTK), without `wl-copy`.
    Ok(Service::new(daemon, Copier::new(GtkClipboard::new()))
        .with_picker(GtkPicker)
        .with_terminal(SystemTerminal::default()))
}

/// Only the embedded interface: `tauri://localhost` (Linux). Any other URL (link,
/// `location = …`, form) is refused by the webview.
fn navigation_allowed(url: &tauri::Url) -> bool {
    url.scheme() == "tauri" && url.host_str() == Some("localhost")
}

/// State and commands. The attack test builds the same builder with Tauri's fake runtime
/// and the real capabilities (`generate_context!`).
fn with_commands<R: Runtime>(builder: Builder<R>, state: AppState) -> Builder<R> {
    builder
        .manage(state)
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
            carregar_chave,
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
    // Before GTK (D47, D57).
    let mut app_startup = startup::start().unwrap_or_else(|e| {
        if matches!(e, startup::StartupError::Delegated) {
            // Opened from the menu: the daemon opens the real instance (D57).
            std::process::exit(0)
        }
        eprintln!("ward-app: {e}");
        std::process::exit(1)
    });
    let daemon = app_startup.take_daemon().ok_or(AppError::ConnectionLost);
    let queue = daemon.as_ref().ok().map(DaemonSocket::notices);
    let service = Arc::new(build_service(daemon));
    let for_approval = Arc::clone(&service);
    let for_watcher = Arc::clone(&service);
    app_startup.watch(move || {
        if let Ok(s) = for_watcher.as_ref() {
            s.on_exit();
        }
    });

    let state = AppState::new(service);
    let channels = Arc::clone(&state.channels);
    let app = with_commands(tauri::Builder::default(), state)
        .on_window_event(|window, event| {
            if window.label() == APPROVAL_WINDOW && matches!(event, WindowEvent::Destroyed) {
                let state = window.state::<AppState>();
                lock_channels(&state.channels).approval = None;
                on_approval_closed(Arc::clone(&state.service));
            }
            // Phase 39: the main window closed (with the tray the app goes on): its
            // channel goes away and what the form and the generator kept is zeroed. The
            // clipboard keeps its 30 s delay.
            if window.label() == MAIN_WINDOW && matches!(event, WindowEvent::Destroyed) {
                let state = window.state::<AppState>();
                lock_channels(&state.channels).main = None;
                if let Ok(s) = state.service() {
                    s.discard_form();
                }
            }
        })
        .setup(move |app| {
            watch_notices(app.handle(), &app.state::<AppState>(), queue.clone());
            // D65 a: each approval request notice of the queue opens (or brings to the
            // front) the approval window and notifies it through its channel
            // (the listen command), without data.
            if let (Some(queue), Ok(s)) = (queue, for_approval.as_ref()) {
                let h = app.handle().clone();
                let r = approval::watch(queue, s.approvals(), move || {
                    let h2 = h.clone();
                    let _ = h.run_on_main_thread(move || open_approval_window(&h2));
                    notify(&channels, APPROVAL_WINDOW, "pedido-de-aprovacao");
                });
                if r.is_err() {
                    eprintln!("ward-app: não foi possível vigiar os pedidos de aprovação");
                }
            }
            adjust_title_bar();
            create_main_window(app)?;
            // Phase 39 (V5.23): the tray, with the current status; without it the app
            // goes on.
            let initial = app
                .state::<AppState>()
                .service()
                .map(|s| s.tray_state())
                .unwrap_or(TrayState::NoDaemon);
            *app.state::<AppState>()
                .seen
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = initial;
            if let Err(reason) = create_tray(app.handle(), initial) {
                eprintln!("ward-app: sem bandeja ({reason})");
            }
            // Phase 39g: the watcher runs even without a tray, so a lock from outside
            // clears the clipboard right away.
            watch_tray(app.handle().clone());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("o app do Ward não pôde iniciar");

    app.run(|handle, event| match event {
        // Phase 39: with the tray, closing the last window does not close the app
        // ("Quit" closes, with a code).
        RunEvent::ExitRequested {
            code: None, api, ..
        } if handle.try_state::<Tray<tauri::Wry>>().is_some() => api.prevent_exit(),
        RunEvent::Exit => {
            if let Ok(s) = handle.state::<AppState>().service() {
                s.on_exit();
            }
        }
        _ => {}
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder};
    use tauri::webview::InvokeRequest;

    /// No daemon: a socket at a path that does not exist, with the installed daemon's
    /// rules (root folder). Never `DaemonSocket::standard()`: in `test-mode` it is the
    /// uid's test socket folder under `/run/user/<uid>/`, which may have a test `wardd` of
    /// another worktree running (and the test would talk to it).
    fn no_daemon() -> DaemonSocket {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("sem-daemon-{}", std::process::id()))
            .join("wardd.sock");
        assert!(!path.exists());
        DaemonSocket::system_at(path)
    }

    fn test_app() -> tauri::App<MockRuntime> {
        let state = AppState::new(Arc::new(build_service(Ok(no_daemon()))));
        with_commands(mock_builder(), state)
            .build(tauri::generate_context!())
            .expect("app de teste")
    }

    fn call(
        window: &tauri::WebviewWindow<MockRuntime>,
        command: &str,
        origin: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, serde_json::Value> {
        get_ipc_response(
            window,
            InvokeRequest {
                cmd: command.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: origin.parse().unwrap(),
                body: InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|b| b.deserialize::<serde_json::Value>().unwrap())
    }

    const ORIGIN: &str = "tauri://localhost";

    /// V5.1: file system, shell and http plugins and a made-up command are denied by the
    /// ACL (there is no plugin or core permission in the capability).
    #[test]
    fn attack_plugins_and_made_up_command_are_denied() {
        let app = test_app();
        let window = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        for command in [
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
            let r = call(
                &window,
                command,
                ORIGIN,
                serde_json::json!({ "path": "/etc/passwd" }),
            );
            assert!(r.is_err(), "{command} deveria ser negado: {r:?}");
        }
    }

    /// The app's own commands pass the ACL (the response is the service error, without a
    /// daemon), but only in the `principal` window and only with the local origin.
    #[test]
    fn attack_own_commands_only_in_the_right_window_and_origin() {
        let app = test_app();
        let main = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let r = call(&main, "estado_do_cofre", ORIGIN, serde_json::json!({}));
        let failure = r.expect_err("sem daemon o serviço responde erro");
        assert!(
            failure["codigo"].is_string(),
            "erro do serviço, não da ACL: {failure}"
        );

        let other = WebviewWindowBuilder::new(&app, "outra", Default::default())
            .build()
            .unwrap();
        let r = call(&other, "estado_do_cofre", ORIGIN, serde_json::json!({}));
        assert!(
            r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
            "janela sem capability: {r:?}"
        );

        let r = call(
            &main,
            "copiar_campo",
            "https://example.com",
            serde_json::json!({"entrada":"acme-pg-dev","campo":"senha"}),
        );
        assert!(
            r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
            "origem remota: {r:?}"
        );
    }

    /// Phase 37 (V5.21, S11): the generator commands pass the ACL only in the `principal`
    /// window and with the local origin; "generate" does not return the value; a size
    /// outside 8–64 or of the wrong type is refused.
    #[test]
    fn attack_generator_only_in_the_right_window_and_no_value_on_generate() {
        let app = test_app();
        let main = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let options = serde_json::json!({
            "tamanho": 24, "minusculas": true, "maiusculas": true,
            "digitos": true, "simbolos": true
        });
        match call(&main, "gerar_senha", ORIGIN, options.clone()) {
            // Without the daemon the service answers its error.
            Err(failure) => assert!(failure["codigo"].is_string(), "erro da ACL: {failure}"),
            Ok(r) => {
                let keys: Vec<&String> = r.as_object().unwrap().keys().collect();
                assert_eq!(keys, ["bits", "forca", "simbolos", "tamanho"]);
                let v = call(&main, "revelar_senha_gerada", ORIGIN, options.clone()).unwrap();
                let value = v["valor"].as_str().unwrap();
                assert_eq!(value.len(), 24);
                assert!(!r.to_string().contains(value));
                for bad in [
                    serde_json::json!({ "tamanho": 7, "minusculas": true, "maiusculas": true, "digitos": true, "simbolos": true }),
                    serde_json::json!({ "tamanho": 65, "minusculas": true, "maiusculas": true, "digitos": true, "simbolos": true }),
                    serde_json::json!({ "tamanho": "24", "minusculas": true, "maiusculas": true, "digitos": true, "simbolos": true }),
                    serde_json::json!({ "tamanho": -1, "minusculas": true, "maiusculas": true, "digitos": true, "simbolos": true }),
                    serde_json::json!({ "tamanho": 24 }),
                ] {
                    assert!(
                        call(&main, "gerar_senha", ORIGIN, bad.clone()).is_err(),
                        "{bad}"
                    );
                }
            }
        }
        let other = WebviewWindowBuilder::new(&app, "outra", Default::default())
            .build()
            .unwrap();
        for command in [
            "gerar_senha",
            "revelar_senha_gerada",
            "copiar_senha_gerada",
            "descartar_senha_gerada",
        ] {
            let r = call(&other, command, ORIGIN, options.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{command} em janela sem capability: {r:?}"
            );
            let r = call(&main, command, "https://example.com", options.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{command} com origem remota: {r:?}"
            );
        }
    }

    /// Phase 33 (S11): the form commands and reveal field only pass the ACL in the
    /// `principal` window with the local origin; a form out of format is refused before
    /// the service.
    #[test]
    fn attack_form_only_in_the_right_window() {
        let app = test_app();
        let main = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let other = WebviewWindowBuilder::new(&app, "outra", Default::default())
            .build()
            .unwrap();
        let form = serde_json::json!({
            "anterior": null,
            "formulario": {"tipo": "login", "nome": "x", "grupo": "g", "ambiente": "dev",
                "projeto": "", "tags": [], "campos": {}, "segredos": {}, "extras": []},
            "entrada": "acme-wp-dev", "campo": "senha", "id": 0
        });
        for command in [
            "revelar_campo",
            "codigo_totp",
            "copiar_codigo_totp",
            "usar_senha_gerada",
            "descartar_formulario",
            "salvar_credencial",
            // Phase 34: attachments and "Connect" follow the same rule.
            "escolher_anexo",
            "remover_anexo",
            // Phase 53a: "Load from file" of the private key.
            "carregar_chave",
            "conectar",
        ] {
            let r = call(&other, command, ORIGIN, form.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{command} em janela sem capability: {r:?}"
            );
            let r = call(&main, command, "https://example.com", form.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{command} com origem remota: {r:?}"
            );
        }
        let forged = serde_json::json!({
            "formulario": {"tipo": "login", "nome": "x", "grupo": "g", "ambiente": "dev",
                "projeto": "", "tags": [], "campos": {}, "segredos": {}, "extras": [],
                "nivel": "livre"}
        });
        let r = call(&main, "salvar_credencial", ORIGIN, forged);
        assert!(
            r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
            "campo a mais: {r:?}"
        );
    }

    /// Phase 35 (S9): set level only passes the ACL in the `principal` window with the
    /// local origin, and a PIN out of format (or an extra field) is refused when reading
    /// the arguments, before the service, without echoing the value.
    #[test]
    fn attack_set_level_only_in_the_right_window_and_pin_in_format() {
        let app = test_app();
        let main = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let other = WebviewWindowBuilder::new(&app, "outra", Default::default())
            .build()
            .unwrap();
        let request =
            serde_json::json!({"entrada": "acme-pg-dev", "nivel": "bloqueado", "pin": "135790"});
        let r = call(&other, "mudar_nivel", ORIGIN, request.clone());
        assert!(
            r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
            "janela sem capability: {r:?}"
        );
        let r = call(&main, "mudar_nivel", "https://example.com", request.clone());
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
            let body =
                serde_json::json!({"entrada": "acme-pg-dev", "nivel": "bloqueado", "pin": pin});
            let r = call(&main, "mudar_nivel", ORIGIN, body);
            let text = format!("{r:?}");
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "PIN fora do formato passou da leitura: {text}"
            );
            assert!(
                !text.contains("135790") && !text.contains("13579a"),
                "{text}"
            );
        }
        // In the right window, the well-formed request reaches the service (here without
        // a daemon: the error is the service's, with a code, and does not carry the PIN).
        let r = call(&main, "mudar_nivel", ORIGIN, request);
        let text = format!("{r:?}");
        assert!(
            r.as_ref().err().is_some_and(|e| e["codigo"].is_string()),
            "{text}"
        );
        assert!(!text.contains("135790"), "{text}");
    }

    /// Phase 30: create vault only in the `principal` window with the local origin; an
    /// extra field is refused before the service; a PIN outside 4 to 6 digits comes back
    /// as a field error without going to the daemon (which is not even running here) and
    /// without a value in the response.
    #[test]
    fn attack_create_vault_only_in_the_right_window_and_checked_in_rust() {
        let app = test_app();
        let main = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let other = WebviewWindowBuilder::new(&app, "outra", Default::default())
            .build()
            .unwrap();
        let data = |pin: &str| {
            serde_json::json!({ "dados": {
                "senha": "falso-mestra-ward-01", "confirmacao": "falso-mestra-ward-01",
                "pin": pin, "confirmacao_pin": pin } })
        };
        let r = call(&other, "criar_cofre", ORIGIN, data("135790"));
        assert!(
            r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
            "janela sem capability: {r:?}"
        );
        let r = call(&main, "criar_cofre", "https://example.com", data("135790"));
        assert!(
            r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
            "origem remota: {r:?}"
        );
        for pin in ["135", "1357901", "13579a"] {
            let r = call(&main, "criar_cofre", ORIGIN, data(pin)).unwrap();
            assert_eq!(r["criado"], false, "{pin}");
            assert_eq!(r["erros"][0]["campo"], "pin", "{pin}");
            let text = r.to_string();
            assert!(
                !text.contains("falso-mestra") && !text.contains(pin),
                "{text}"
            );
        }
        let mut forged = data("135790");
        forged["dados"]["nivel"] = "livre".into();
        let r = call(&main, "criar_cofre", ORIGIN, forged);
        assert!(
            r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
            "campo a mais: {r:?}"
        );
    }

    /// Phase 31 (S13, D65 a): unlock and the listen command only pass the ACL in the
    /// `principal` window with the local origin; a password out of format is refused when
    /// reading the arguments, without echo; the notice channel does not open
    /// `plugin:event|*`.
    #[test]
    fn attack_unlock_only_in_the_right_window_and_password_without_echo() {
        let app = test_app();
        let main = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let other = WebviewWindowBuilder::new(&app, "outra", Default::default())
            .build()
            .unwrap();
        let request = serde_json::json!({"senha": "falso-mestra-ward-01"});
        for command in ["destravar", "ouvir_avisos"] {
            let r = call(&other, command, ORIGIN, request.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{command} em janela sem capability: {r:?}"
            );
            let r = call(&main, command, "https://example.com", request.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{command} com origem remota: {r:?}"
            );
        }
        let long = "x".repeat(ward_core_max_secret() + 1);
        for password in [
            serde_json::json!(""),
            serde_json::json!(long),
            serde_json::json!(135790),
            serde_json::Value::Null,
            serde_json::json!(["falso-mestra-ward-01"]),
        ] {
            let r = call(
                &main,
                "destravar",
                ORIGIN,
                serde_json::json!({"senha": password}),
            );
            let text = format!("{r:?}");
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "senha fora do formato passou da leitura: {}",
                text.len()
            );
            assert!(!text.contains("falso-mestra") && !text.contains("xxxx"));
        }
        // Well formed, it reaches the service (without a daemon: error with a code,
        // without the password).
        let r = call(&main, "destravar", ORIGIN, request);
        let text = format!("{r:?}");
        assert!(
            r.as_ref().err().is_some_and(|e| e["codigo"].is_string()),
            "{text}"
        );
        assert!(!text.contains("falso-mestra"), "{text}");
        // The channel does not allow the core event commands.
        for command in [
            "plugin:event|listen",
            "plugin:event|emit",
            "plugin:event|emit_to",
        ] {
            let r = call(
                &main,
                command,
                ORIGIN,
                serde_json::json!({"event": "pedido-de-destrave", "target": {"kind": "Any"}, "handler": 0}),
            );
            assert!(r.is_err(), "{command} deveria ser negado: {r:?}");
        }
    }

    /// Phase 38 (V5.22, S9): the Settings screen commands only pass the ACL in the
    /// `principal` window with the local origin; an extra field and a wrong type are
    /// refused on reading, and a PIN or password outside the rule comes back as a field
    /// error without going to the daemon and without a value in the response.
    #[test]
    fn attack_settings_only_in_the_right_window_and_checked_in_rust() {
        let app = test_app();
        let main = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let other = WebviewWindowBuilder::new(&app, "outra", Default::default())
            .build()
            .unwrap();
        let pin = |current: &str, new: &str| {
            serde_json::json!({ "dados": {
                "atual": current, "novo": new, "confirmacao": new } })
        };
        let password = |current: &str, new: &str| {
            serde_json::json!({ "dados": {
                "atual": current, "nova": new, "confirmacao": new } })
        };
        for (command, body) in [
            ("configuracoes", serde_json::json!({})),
            ("trocar_pin", pin("135790", "2468")),
            (
                "trocar_senha_mestra",
                password("falso-mestra-ward-01", "falso-nova-mestra-02"),
            ),
            ("definir_tempo_trava", serde_json::json!({ "minutos": 30 })),
        ] {
            let r = call(&other, command, ORIGIN, body.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{command} em janela sem capability: {r:?}"
            );
            let r = call(&main, command, "https://example.com", body.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{command} com origem remota: {r:?}"
            );
        }
        for (current, new, field) in [
            ("135", "2468", "atual"),
            ("135790", "13579a", "novo"),
            ("135790", "1357901", "novo"),
        ] {
            let r = call(&main, "trocar_pin", ORIGIN, pin(current, new)).unwrap();
            assert_eq!(r["trocado"], false);
            assert_eq!(r["erros"][0]["campo"], field, "{current} {new}");
            let text = r.to_string();
            assert!(!text.contains(current) && !text.contains(new), "{text}");
        }
        let r = call(
            &main,
            "trocar_senha_mestra",
            ORIGIN,
            password("falso-mestra-ward-01", "falso-curta"),
        )
        .unwrap();
        assert_eq!(r["erros"][0]["campo"], "nova");
        assert!(!r.to_string().contains("falso"), "{r}");
        let mut forged = pin("135790", "2468");
        forged["dados"]["nivel"] = "livre".into();
        let r = call(&main, "trocar_pin", ORIGIN, forged);
        assert!(
            r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
            "campo a mais: {r:?}"
        );
        for minutes in [
            serde_json::json!("30"),
            serde_json::json!(-1),
            serde_json::Value::Null,
        ] {
            let r = call(
                &main,
                "definir_tempo_trava",
                ORIGIN,
                serde_json::json!({ "minutos": minutes }),
            );
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{minutes}: {r:?}"
            );
        }
        // Outside 1 to 1440: refused in the service, before the daemon (which does not
        // even exist here).
        for minutes in [0, 1441] {
            let r = call(
                &main,
                "definir_tempo_trava",
                ORIGIN,
                serde_json::json!({ "minutos": minutes }),
            );
            assert!(
                r.as_ref().err().is_some_and(|e| e["codigo"].is_string()),
                "{minutes}: {r:?}"
            );
        }
    }

    /// Phase 48 (V7.9): the external backup commands only in the `principal` window with
    /// the local origin; the UUID forged by the interface (out of format, turning on
    /// without a UUID, turning off while changing the UUID, wrong type) is refused in
    /// Rust before the daemon.
    #[test]
    fn attack_external_backup_only_in_the_right_window_and_uuid_checked_in_rust() {
        let app = test_app();
        let main = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let other = WebviewWindowBuilder::new(&app, "outra", Default::default())
            .build()
            .unwrap();
        let approval = WebviewWindowBuilder::new(&app, "aprovacao", Default::default())
            .build()
            .unwrap();
        for (command, body) in [
            ("backup_externo", serde_json::json!({})),
            ("listar_pendrives", serde_json::json!({})),
            (
                "definir_backup_externo",
                serde_json::json!({ "ligado": true, "uuid": "7C1E-42A9" }),
            ),
        ] {
            for window in [&other, &approval] {
                let r = call(window, command, ORIGIN, body.clone());
                assert!(
                    r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                    "{command} em {}: {r:?}",
                    window.label()
                );
            }
            let r = call(&main, command, "https://example.com", body.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{command} com origem remota: {r:?}"
            );
        }
        // Wrong format: the core refuses before opening the daemon (invalid request code,
        // not daemon unavailable).
        for body in [
            serde_json::json!({ "ligado": true, "uuid": "../etc" }),
            serde_json::json!({ "ligado": true, "uuid": "/media/usuario/WARD-BACKUP" }),
            serde_json::json!({ "ligado": true, "uuid": "7C1E-42A9\u{202E}" }),
            serde_json::json!({ "ligado": true, "uuid": "" }),
            serde_json::json!({ "ligado": true }),
            serde_json::json!({ "ligado": false, "uuid": "DEAD-BEEF" }),
        ] {
            let r = call(&main, "definir_backup_externo", ORIGIN, body.clone());
            assert_eq!(
                r.as_ref().err().map(|e| e["codigo"].clone()),
                Some(serde_json::json!("pedido-invalido")),
                "{body}: {r:?}"
            );
        }
        for body in [
            serde_json::json!({ "ligado": "sim", "uuid": "7C1E-42A9" }),
            serde_json::json!({ "ligado": true, "uuid": 4242 }),
            serde_json::json!({ "uuid": "7C1E-42A9" }),
        ] {
            let r = call(&main, "definir_backup_externo", ORIGIN, body.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{body}: {r:?}"
            );
        }
    }

    /// Phase 47 (V7.7, V7.8): list and restore backup only in the `principal` window with
    /// the local origin; a forged version (path, `..`, out of format), a PIN out of
    /// format, an empty password and an extra field (generation) are refused in Rust
    /// before the daemon.
    #[test]
    fn attack_restore_backup_only_in_the_right_window_and_version_checked_in_rust() {
        let app = test_app();
        let main = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let other = WebviewWindowBuilder::new(&app, "outra", Default::default())
            .build()
            .unwrap();
        let approval = WebviewWindowBuilder::new(&app, "aprovacao", Default::default())
            .build()
            .unwrap();
        let data = |version: &str, password: &str, pin: &str| serde_json::json!({ "dados": { "versao": version, "senha": password, "pin": pin } });
        let good = data(
            "ward-20261108-143200.ward",
            "falso-mestra-ward-01",
            "135790",
        );
        for (command, body) in [
            ("listar_backups", serde_json::json!({})),
            ("restaurar_backup", good.clone()),
        ] {
            for window in [&other, &approval] {
                let r = call(window, command, ORIGIN, body.clone());
                assert!(
                    r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                    "{command} em {}: {r:?}",
                    window.label()
                );
            }
            let r = call(&main, command, "https://example.com", body.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{command} com origem remota: {r:?}"
            );
        }
        // In the right window, without a daemon: passes the reading and the format and
        // stops at the socket.
        let r = call(&main, "restaurar_backup", ORIGIN, good.clone());
        let code = r.as_ref().err().map(|e| e["codigo"].clone());
        assert!(
            code.as_ref()
                .is_some_and(|c| c.is_string() && c != "pedido-invalido"),
            "{r:?}"
        );
        // Version out of format: the core refuses before opening the daemon.
        for version in [
            "../cofre.ward",
            "/home/usuario/.local/share/ward/cofre.ward",
            "backups/ward-20261108-143200.ward",
            "ward-20261108-143200.ward\u{202E}",
            "",
        ] {
            let body = data(version, "falso-mestra-ward-01", "135790");
            let r = call(&main, "restaurar_backup", ORIGIN, body);
            assert_eq!(
                r.as_ref().err().map(|e| e["codigo"].clone()),
                Some(serde_json::json!("pedido-invalido")),
                "{version:?}: {r:?}"
            );
        }
        // PIN and password out of format, extra or missing field: refused on reading.
        let mut extra = good.clone();
        extra["dados"]["geracao"] = serde_json::json!(9);
        for body in [
            data("ward-20261108-143200.ward", "falso-mestra-ward-01", "12ab"),
            data("ward-20261108-143200.ward", "falso-mestra-ward-01", "123"),
            data("ward-20261108-143200.ward", "", "135790"),
            extra,
            serde_json::json!({ "dados": { "versao": "ward-20261108-143200.ward" } }),
            serde_json::json!({ "versao": "ward-20261108-143200.ward" }),
        ] {
            let r = call(&main, "restaurar_backup", ORIGIN, body.clone());
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{body}: {r:?}"
            );
            // The refusal does not echo what came in.
            let text = format!("{r:?}");
            assert!(
                !text.contains("falso-mestra") && !text.contains("135790"),
                "{text}"
            );
        }
    }

    fn ward_core_max_secret() -> usize {
        64 * 1024
    }

    /// D65 a, Phase 36: the notice channel is per window. The approval notice does not
    /// reach the main window nor the unlock notice the approval one, and none carries
    /// data besides the name.
    #[test]
    fn attack_notice_of_one_window_does_not_go_to_the_other() {
        use std::sync::mpsc;
        let channel = |tx: mpsc::Sender<String>| {
            Channel::<ScreenNotice>::new(move |body| {
                let text = match body {
                    tauri::ipc::InvokeResponseBody::Json(j) => j,
                    tauri::ipc::InvokeResponseBody::Raw(_) => "bruto".to_owned(),
                };
                let _ = tx.send(text);
                Ok(())
            })
        };
        let (tx_m, rx_m) = mpsc::channel();
        let (tx_a, rx_a) = mpsc::channel();
        let channels: NoticeChannels = Arc::default();
        lock_channels(&channels).main = Some(channel(tx_m));
        lock_channels(&channels).approval = Some(channel(tx_a));
        notify(&channels, APPROVAL_WINDOW, "pedido-de-aprovacao");
        notify(&channels, MAIN_WINDOW, "pedido-de-destrave");
        notify(&channels, "outra", "pedido-de-aprovacao");
        assert_eq!(
            rx_a.try_iter().collect::<Vec<_>>(),
            vec![r#"{"aviso":"pedido-de-aprovacao"}"#]
        );
        assert_eq!(
            rx_m.try_iter().collect::<Vec<_>>(),
            vec![r#"{"aviso":"pedido-de-destrave"}"#]
        );
        // Approval window destroyed: its channel goes away, and the notice goes to
        // nobody.
        lock_channels(&channels).approval = None;
        notify(&channels, APPROVAL_WINDOW, "pedido-de-aprovacao");
        assert!(rx_a.try_iter().next().is_none() && rx_m.try_iter().next().is_none());
    }

    /// Phase 36 (S10, S11): answering an approval only in the approval window, with the
    /// local origin. The main window (and any other) neither reads the request nor
    /// answers; the approval one reaches no other command or plugin. A forged answer (an
    /// id the daemon did not send) is refused by the service; a PIN out of format, on
    /// reading, without echo.
    #[test]
    fn attack_approval_only_through_the_right_window() {
        let app = test_app();
        let main = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let other = WebviewWindowBuilder::new(&app, "outra", Default::default())
            .build()
            .unwrap();
        let approval = WebviewWindowBuilder::new(&app, APPROVAL_WINDOW, Default::default())
            .build()
            .unwrap();
        let denied_by_acl = |r: &Result<serde_json::Value, serde_json::Value>| {
            r.as_ref().err().is_some_and(|e| !e["codigo"].is_string())
        };
        let answer = serde_json::json!({"id": 1, "decisao": "uma_vez", "pin": "135790"});
        for window in [&main, &other] {
            for command in ["pedido_de_aprovacao", "responder_aprovacao"] {
                let r = call(window, command, ORIGIN, answer.clone());
                assert!(denied_by_acl(&r), "{command} em {}: {r:?}", window.label());
                assert!(!format!("{r:?}").contains("135790"));
            }
        }
        for command in ["pedido_de_aprovacao", "responder_aprovacao"] {
            let r = call(&approval, command, "https://example.com", answer.clone());
            assert!(denied_by_acl(&r), "{command} com origem remota: {r:?}");
        }
        // The approval window reaches nothing else.
        for command in [
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
            let r = call(
                &approval,
                command,
                ORIGIN,
                serde_json::json!({"entrada": "acme-pg-aprovacao", "campo": "senha", "nivel": "livre", "pin": "135790"}),
            );
            assert!(denied_by_acl(&r), "{command} na janela de aprovação: {r:?}");
        }
        // PIN out of format and a decision of another type: refused on reading, without
        // echo.
        for body in [
            serde_json::json!({"id": 1, "decisao": "uma_vez", "pin": "12"}),
            serde_json::json!({"id": 1, "decisao": "uma_vez", "pin": "13579a"}),
            serde_json::json!({"id": 1, "decisao": "uma_vez", "pin": 135790}),
            serde_json::json!({"id": "1", "decisao": "uma_vez", "pin": "135790"}),
            serde_json::json!({"id": -1, "decisao": "negar", "pin": null}),
            serde_json::json!({"id": 1, "decisao": 1, "pin": null}),
        ] {
            let r = call(&approval, "responder_aprovacao", ORIGIN, body);
            let text = format!("{r:?}");
            assert!(denied_by_acl(&r), "{text}");
            assert!(
                !text.contains("135790") && !text.contains("13579a"),
                "{text}"
            );
        }
        // Well formed, but the daemon never sent request 1: the service refuses (or did
        // not even build, without a clipboard). Error with a code, without the PIN.
        let r = call(&approval, "responder_aprovacao", ORIGIN, answer);
        let text = format!("{r:?}");
        assert!(
            r.as_ref().err().is_some_and(|e| e["codigo"].is_string()),
            "{text}"
        );
        assert!(!text.contains("135790"), "{text}");
    }

    #[test]
    fn attack_navigation_out_of_the_app_is_refused() {
        for url in [
            "https://example.com/",
            "http://localhost/",
            "http://127.0.0.1:1430/",
            "tauri://example.com/",
            "file:///etc/passwd",
            "data:text/html,<script>1</script>",
            "javascript:alert(1)",
        ] {
            assert!(!navigation_allowed(&url.parse().unwrap()), "{url}");
        }
        assert!(navigation_allowed(
            &"tauri://localhost/index.html".parse().unwrap()
        ));
    }

    #[test]
    fn attack_failure_carries_no_value() {
        let f = Failure::from(AppError::InvalidRequest);
        assert_eq!(f.code, "pedido-invalido");
        assert_eq!(f.message, "pedido inválido");
    }

    /// The new Rust names of the webview `serde` are not wire names (frozen until
    /// m2:Phase 54): the screen still gets the old keys.
    #[test]
    fn attack_new_rust_names_are_not_wire_names() {
        let f = serde_json::to_value(Failure::from(AppError::InvalidRequest)).unwrap();
        assert!(
            f.get("codigo").is_some() && f.get("mensagem").is_some(),
            "{f}"
        );
        let s = serde_json::to_value(StatusResponse {
            status: "travado",
            request: None,
        })
        .unwrap();
        assert_eq!(s, serde_json::json!({"estado": "travado", "pedido": null}));
        let n = serde_json::to_value(ScreenNotice {
            notice: "estado-mudou",
        })
        .unwrap();
        assert_eq!(n, serde_json::json!({"aviso": "estado-mudou"}));
        let g = serde_json::to_value(GeneratedResponse {
            size: 24,
            bits: 150,
            strength: "muito-forte",
            symbols: "!#",
        })
        .unwrap();
        let mut keys: Vec<_> = g.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["bits", "forca", "simbolos", "tamanho"]);
        let r = serde_json::to_value(RevealedResponse {
            value: "falso-valor".into(),
        })
        .unwrap();
        assert_eq!(r, serde_json::json!({"valor": "falso-valor"}));
        let u = serde_json::to_value(UsedResponse { size: 24 }).unwrap();
        assert_eq!(u, serde_json::json!({"tamanho": 24}));
    }

    /// Phase 39 (V5.23): lock passes the ACL only in the main window and with the
    /// local origin; the approval window and any window do not lock (not even with a
    /// made-up argument). Without a daemon, the service answers its error, with a code.
    #[test]
    fn attack_lock_only_in_the_main_window() {
        let app = test_app();
        let main = WebviewWindowBuilder::new(&app, "principal", Default::default())
            .build()
            .unwrap();
        let r = call(&main, "travar", ORIGIN, serde_json::json!({}));
        assert!(
            r.as_ref().err().is_some_and(|e| e["codigo"].is_string()),
            "erro do serviço, não da ACL: {r:?}"
        );
        let r = call(
            &main,
            "travar",
            "https://example.com",
            serde_json::json!({}),
        );
        assert!(
            r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
            "origem remota: {r:?}"
        );
        for label in ["aprovacao", "outra"] {
            let window = WebviewWindowBuilder::new(&app, label, Default::default())
                .build()
                .unwrap();
            let r = call(&window, "travar", ORIGIN, serde_json::json!({}));
            assert!(
                r.as_ref().err().is_some_and(|e| !e["codigo"].is_string()),
                "{label}: {r:?}"
            );
        }
    }

    /// Phase 39: without the tray (the test does not create one), updating only notifies
    /// the main window and does not panic; a forged click without a tray does nothing.
    #[test]
    fn missing_tray_does_not_break() {
        let app = test_app();
        update_tray(app.handle());
        on_tray_click(app.handle(), tray::ID_QUIT);
        on_tray_click(app.handle(), "inventado");
        assert!(app.try_state::<Tray<MockRuntime>>().is_none());
    }

    /// Attack (Phase 39g): without a tray, the watcher still measures the transition
    /// through `AppState::seen` (clearing the clipboard on that transition is tested in
    /// the core, `attack_outside_lock_without_tray_clears_the_clipboard_at_once`). Here
    /// the daemon goes away with the vault unlocked: `seen` becomes `NoDaemon`, without
    /// any tray.
    #[test]
    fn without_tray_the_watcher_measures_the_transition() {
        let app = test_app();
        assert!(app.try_state::<Tray<MockRuntime>>().is_none());
        let state = app.state::<AppState>();
        *state.seen.lock().unwrap() = TrayState::Unlocked;
        update_tray(app.handle());
        assert_eq!(*state.seen.lock().unwrap(), TrayState::NoDaemon);
    }
}
