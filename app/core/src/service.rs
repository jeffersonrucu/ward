//! What each app command does. Tauri (`app/src-tauri`) and the test bridge
//! (`ward-test-bridge`) call only this, so both have the same set of commands and the
//! same responses. No response carries a secret (S1, S11, D4).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;
use ward_core::entry::{AttachmentDraft, Draft, EntryType, Secret as CoreSecret};
use ward_core::protocolo::Alvo;

use crate::AppError;
use crate::anexos::{Anexos, Base, EdicaoDosAnexos, Escolha, Seletor, SemSeletor};
use crate::aprovacao::{self, Aprovacoes, PedidoExibido, ResultadoAprovacao};
use crate::area_transferencia::{AreaTransferencia, Copiador};
use crate::backup_externo::{self, BackupExternoVisto, EscolhaDoPendrive, PendriveVisto};
use crate::chave::{self, CAMPOS_CARREGAVEIS, Carregamento};
use crate::cliente;
use crate::comandos::{Referencia, validar_nome_entrada};
use crate::configuracoes::{Configuracoes, ResultadoTroca, TrocaPin, TrocaSenha, minutos_validos};
use crate::credenciais::{DetalheCredencial, ItemCredencial};
use crate::daemon::{Daemon, EstadoCofre};
use crate::destrave::{InfoPedido, PedidoPendente, SenhaMestra};
use crate::formulario::{self, ErroCampo, Falta, Fontes, Formulario};
use crate::gerador::{Gerada, Gerador, Opcoes, Revelada};
use crate::nivel::{NivelMudado, Pin, nivel_de_nome};
use crate::primeiro_uso::NovoCofre;
use crate::restaurar::{self, Restauracao, Restaurado, VersaoVista};
use crate::secret::Secret;
use crate::terminal::{SemTerminal, Terminal, entrada_para_argv};

/// Form fields that accept "Use this password" (in the design, the secret field with
/// generate is only the password one).
const GENERATABLE_FIELDS: &[&str] = &["senha"];

/// Response of the save command: the saved name, or the errors per field. No value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SaveResult {
    #[serde(rename = "salvo")]
    pub saved: Option<String>,
    #[serde(rename = "erros")]
    pub errors: Vec<ErroCampo>,
}

/// Response of the vault creation: created, or the errors per field. No value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreateResult {
    #[serde(rename = "criado")]
    pub created: bool,
    #[serde(rename = "erros")]
    pub errors: Vec<ErroCampo>,
}

/// TOTP code for the interface (the code is shown, the seed never).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TotpCode {
    #[serde(rename = "codigo")]
    pub code: String,
    #[serde(rename = "restante")]
    pub remaining: u64,
}

pub struct Service<D: Daemon, A: AreaTransferencia> {
    daemon: Mutex<D>,
    copier: Copiador<A>,
    /// Standalone password of the password generator screen (Phase 37), in memory only.
    generator: Mutex<Gerador>,
    /// Passwords the form generator handed to a field ("Use this password"), in memory
    /// only, until save, cancel or exit.
    used: Mutex<BTreeMap<String, Secret>>,
    /// Private keys read by "Load from file" (Phase 53a), per field, in memory only,
    /// until save, cancel or exit. The content never goes to the interface.
    loaded: Mutex<BTreeMap<String, Secret>>,
    /// Attachments chosen in the open form (Phase 34), in memory only, until save, cancel
    /// or exit. The content never goes to the interface.
    attachments: Mutex<Anexos>,
    /// Who chooses the attachment file (GTK dialog in the app; folder in the bridge).
    picker: Box<dyn Seletor>,
    /// Who opens the "Connect" terminal (D3).
    terminal: Box<dyn Terminal>,
    /// The approval requests waiting for the window (Phase 36, D65 a).
    approvals: Arc<Aprovacoes>,
    /// The agent's unlock request that is waiting (Phase 31, D65 a).
    pending_unlock: Arc<PedidoPendente>,
}

/// The state of the external backup: the config and, with a chosen UUID, whether it is
/// among the USB drives seen. udisks2 without an answer is not an error here: the card
/// says it could not see them.
fn external_backup_of<D: Daemon + ?Sized>(d: &mut D) -> Result<BackupExternoVisto, AppError> {
    let (enabled, uuid) = cliente::backup_externo(d)?;
    let (usb_drive, drives_visible) = match &uuid {
        None => (None, true),
        Some(u) => match cliente::pendrives(d) {
            Ok(list) => (backup_externo::achar(&list, u).map(PendriveVisto::de), true),
            Err(AppError::UsbDrivesUnavailable) => (None, false),
            Err(e) => return Err(e),
        },
    };
    Ok(BackupExternoVisto {
        ligado: enabled,
        uuid,
        pendrive: usb_drive,
        pendrives_visiveis: drives_visible,
    })
}

/// Values the form asks for without going through the interface.
struct ServiceSources<'a, D: Daemon> {
    daemon: &'a mut D,
    entry: Option<&'a str>,
    used: &'a BTreeMap<String, Secret>,
    loaded: &'a BTreeMap<String, Secret>,
}

impl<D: Daemon> Fontes for ServiceSources<'_, D> {
    fn guardado(&mut self, target: Alvo) -> Result<CoreSecret, AppError> {
        let entry = self.entry.ok_or(AppError::InvalidRequest)?;
        let s = cliente::revelar(self.daemon, entry, target)?;
        Ok(CoreSecret::from_bytes(s.expose().to_vec()))
    }

    fn gerada(&mut self, field: &str) -> Result<CoreSecret, AppError> {
        self.used
            .get(field)
            .map(|s| CoreSecret::from_bytes(s.expose().to_vec()))
            .ok_or(AppError::NoGeneratedPassword)
    }

    fn carregada(&mut self, field: &str) -> Result<CoreSecret, AppError> {
        self.loaded
            .get(field)
            .map(|s| CoreSecret::from_bytes(s.expose().to_vec()))
            .ok_or(AppError::InvalidRequest)
    }
}

impl<D: Daemon, A: AreaTransferencia> Service<D, A> {
    pub fn new(daemon: D, copier: Copiador<A>) -> Self {
        Service {
            daemon: Mutex::new(daemon),
            copier,
            generator: Mutex::new(Gerador::default()),
            used: Mutex::new(BTreeMap::new()),
            loaded: Mutex::new(BTreeMap::new()),
            attachments: Mutex::new(Anexos::default()),
            picker: Box::new(SemSeletor),
            terminal: Box::new(SemTerminal),
            approvals: Arc::new(Aprovacoes::default()),
            pending_unlock: Arc::new(PedidoPendente::default()),
        }
    }

    /// The unlock request the notice queue watcher marks
    /// ([`crate::destrave::vigiar`]).
    pub fn unlock_request(&self) -> Arc<PedidoPendente> {
        Arc::clone(&self.pending_unlock)
    }

    /// The agent request waiting for the unlock, if the vault is locked and the request
    /// is still within its deadline. No entry nor command (the notice does not carry
    /// them).
    pub fn request_info(&self, status: EstadoCofre) -> Option<InfoPedido> {
        match status {
            EstadoCofre::Travado => self.pending_unlock.info(),
            _ => None,
        }
    }

    /// Unlock (Phase 31): the password arrives already checked for format, in a
    /// [`SenhaMestra`] that zeroes, and only goes to the daemon. The D20 wait is the
    /// daemon's (before looking at the password), and the response does not say how many
    /// attempts there were. Once unlocked, the agent request proceeds on its own in the
    /// daemon and leaves the screen.
    pub fn unlock(&self, password: SenhaMestra) -> Result<(), AppError> {
        let r = cliente::destravar(&mut *self.daemon()?, &password);
        drop(password);
        r?;
        self.pending_unlock.limpar();
        Ok(())
    }

    /// The approval requests the notice queue watcher fills
    /// ([`crate::aprovacao::vigiar`]).
    pub fn approvals(&self) -> Arc<Aprovacoes> {
        Arc::clone(&self.approvals)
    }

    /// Approval request (Phase 36): the current request as the window shows it, with all
    /// agent text neutralized (D51), or `None`. The entry types come from the detail (once
    /// per request; without it, the screen shows the generic icon).
    pub fn approval_request(&self) -> Result<Option<PedidoExibido>, AppError> {
        let Some(mut current) = self.approvals.atual() else {
            return Ok(None);
        };
        if current.tipos.is_none() && !current.bloqueado {
            let mut d = self.daemon()?;
            let kinds: Vec<Option<&'static str>> = current
                .pedido
                .entradas
                .iter()
                .map(|n| cliente::detalhe(&mut *d, n).ok().map(|x| x.tipo.name()))
                .collect();
            drop(d);
            self.approvals
                .guardar_tipos(current.pedido.id, kinds.clone());
            current.tipos = Some(kinds);
        }
        Ok(Some(aprovacao::exibir(&current)))
    }

    /// Answer an approval (Phase 36, S9, S10). Only a request the daemon sent to this app
    /// and that is still here; "Deny" without PIN, the three "Allow" with PIN (already
    /// checked for format, in a [`Pin`] that zeroes).
    /// - Wrong PIN: the request stays pending; if the vault locked (5th error), the daemon
    ///   already denied and the window shows it blocked.
    /// - "Deny" on a blocked request only removes it from the window (the daemon already
    ///   denied).
    /// - Phase 54b (D68): `acknowledged` is the default entry box checked. On a request
    ///   with the hidden entry notice, an "Allow" without it is refused here (and again in
    ///   the daemon); `acknowledged` only with an "Allow" of a request with the notice.
    pub fn answer_approval(
        &self,
        id: u64,
        decision: &str,
        pin: Option<Pin>,
        acknowledged: bool,
    ) -> Result<ResultadoAprovacao, AppError> {
        let decision = aprovacao::decisao_de_nome(decision)?;
        if decision.pede_pin() != pin.is_some() || (acknowledged && !decision.pede_pin()) {
            return Err(AppError::InvalidRequest);
        }
        let result = match self.approvals.situacao(id) {
            None => return Err(AppError::InvalidRequest),
            Some(true) => {
                if decision.pede_pin() {
                    return Err(AppError::Locked);
                }
                self.approvals.remover(id);
                "respondido"
            }
            Some(false) => {
                // Phase 44c: the origin confirmation is only decided by "Deny" or
                // "Confirm"; the other requests never by "Confirm" (the daemon checks
                // again).
                let confirmation = self.approvals.confirmacao(id).unwrap_or(false);
                if decision != ward_core::protocolo::Decisao::Negar
                    && confirmation != (decision == ward_core::protocolo::Decisao::Confirmar)
                {
                    return Err(AppError::InvalidRequest);
                }
                // Phase 54b: the default entry box only on the request that has the
                // notice, and mandatory on an "Allow" of it.
                let hidden = self.approvals.entrada_oculta(id).unwrap_or(false);
                if decision.pede_pin() && hidden != acknowledged {
                    return Err(AppError::InvalidRequest);
                }
                let r = cliente::responder_aprovacao(
                    &mut *self.daemon()?,
                    id,
                    decision,
                    pin.as_ref(),
                    acknowledged,
                );
                drop(pin);
                match r? {
                    cliente::Respondida::Ok => {
                        self.approvals.remover(id);
                        "respondido"
                    }
                    cliente::Respondida::Expirado => {
                        self.approvals.remover(id);
                        "expirado"
                    }
                    cliente::Respondida::PinIncorreto => {
                        if self.vault_status()? == EstadoCofre::Destravado {
                            "pin-incorreto"
                        } else {
                            // The 5th error locked the vault: the daemon denied the request (S9).
                            self.approvals.marcar_bloqueado(id);
                            "bloqueado"
                        }
                    }
                }
            }
        };
        Ok(ResultadoAprovacao {
            resultado: result,
            ha_outro: self.approvals.ha_pedido(),
        })
    }

    /// Closing the window denies (S10): every pending request here gets "Deny". The
    /// blocked ones just leave (the daemon already denied). A failure talking to the
    /// daemon opens nothing: without an answer, the request expires and is denied in the
    /// daemon.
    pub fn deny_pending_approvals(&self) {
        for id in self.approvals.pendentes() {
            if let Ok(mut d) = self.daemon() {
                let _ = cliente::responder_aprovacao(
                    &mut *d,
                    id,
                    ward_core::protocolo::Decisao::Negar,
                    None,
                    false,
                );
            }
            self.approvals.remover(id);
        }
        self.approvals.esvaziar();
    }

    /// Swaps the file picker (without it, [`Self::choose_attachment`] answers the error).
    pub fn with_picker(mut self, picker: impl Seletor + 'static) -> Self {
        self.picker = Box::new(picker);
        self
    }

    /// Swaps the "Connect" terminal (without it, [`Self::connect`] answers the error).
    pub fn with_terminal(mut self, terminal: impl Terminal + 'static) -> Self {
        self.terminal = Box::new(terminal);
        self
    }

    fn lock_attachments(&self) -> MutexGuard<'_, Anexos> {
        // Panic with the lock held: the chosen attachments are discarded.
        self.attachments.lock().unwrap_or_else(|e| {
            let mut a = e.into_inner();
            a.limpar();
            a
        })
    }

    /// Choose attachment: the Rust side picker chooses the file and it is read here
    /// (limits of D29/D32). The response carries id, name and size, or the notice; never
    /// the content (S11, V5.12).
    ///
    /// Phase 39b: when editing (`edit`), the stored attachments the screen keeps count in
    /// the limits; the names are checked in the entry detail before opening the picker
    /// (unknown or repeated name is an invalid request).
    pub fn choose_attachment(&self, edit: Option<&EdicaoDosAnexos>) -> Result<Escolha, AppError> {
        let base = match edit {
            None => Base::default(),
            Some(ed) => self.edit_base(ed)?,
        };
        match self.picker.escolher()? {
            None => Ok(Escolha::Cancelado),
            Some(path) => Ok(self.lock_attachments().anexar_sobre(&path, base)),
        }
    }

    /// Sum and count of the stored attachments of the edited entry that the screen keeps.
    fn edit_base(&self, ed: &EdicaoDosAnexos) -> Result<Base, AppError> {
        validar_nome_entrada(&ed.entrada)?;
        if ed.mantidos.len() > ward_core::limits::MAX_ATTACHMENTS {
            return Err(AppError::InvalidRequest);
        }
        let det = cliente::detalhe(&mut *self.daemon()?, &ed.entrada)?;
        let mut base = Base::default();
        for (i, n) in ed.mantidos.iter().enumerate() {
            let stored = det.anexos.iter().find(|a| &a.nome == n);
            match stored {
                Some(a) if !ed.mantidos[..i].contains(n) => {
                    let t = usize::try_from(a.tamanho).map_err(|_| AppError::InvalidResponse)?;
                    base.soma = base.soma.saturating_add(t);
                    base.quantidade += 1;
                }
                _ => return Err(AppError::InvalidRequest),
            }
        }
        Ok(base)
    }

    /// Load key (Phase 53a): "Load from file" of the "Private key" field. The field is
    /// checked before opening the picker; the file is read and checked here
    /// ([`chave::ler_chave`]: no link, regular file only, up to 64 KiB, OpenSSH/PEM key
    /// only) and kept until save, cancel or exit. The response carries only the size, or
    /// the notice; never the content nor the path (S2, S11). A notice or a cancel do not
    /// erase the key loaded before.
    pub fn load_key(&self, field: &str) -> Result<Carregamento, AppError> {
        let field = CAMPOS_CARREGAVEIS
            .iter()
            .find(|c| **c == field)
            .ok_or(AppError::InvalidRequest)?;
        let Some(path) = self.picker.escolher_chave()? else {
            return Ok(Carregamento::Cancelado);
        };
        match chave::ler_chave(&path) {
            Ok(s) => {
                let size = s.len() as u64;
                self.lock_loaded().insert((*field).to_owned(), s);
                Ok(Carregamento::Carregada { tamanho: size })
            }
            Err(reason) => Ok(Carregamento::Aviso { motivo: reason }),
        }
    }

    fn lock_loaded(&self) -> MutexGuard<'_, BTreeMap<String, Secret>> {
        // Panic with the lock held: the loaded keys are discarded.
        self.loaded.lock().unwrap_or_else(|e| {
            let mut m = e.into_inner();
            m.clear();
            m
        })
    }

    /// Remove attachment: removes a chosen attachment not saved yet.
    pub fn remove_attachment(&self, id: u64) -> Result<(), AppError> {
        self.lock_attachments().remover(id)
    }

    /// Connect (D3): server entries only. The daemon issues the single-use token, the
    /// terminal opens `ward ssh --token-fd <n> <entry>` and the token goes through the fd
    /// channel (Phase 27a), never in argv. The response does not carry the token.
    pub fn connect(&self, entry: &str) -> Result<(), AppError> {
        validar_nome_entrada(entry)?;
        if !entrada_para_argv(entry) {
            return Err(AppError::InvalidRequest);
        }
        let token = {
            let mut d = self.daemon()?;
            if cliente::detalhe(&mut *d, entry)?.tipo != EntryType::Server {
                return Err(AppError::NotAServer);
            }
            cliente::emitir_token(&mut *d, entry)?
        };
        self.terminal.abrir(entry, &token)
    }

    fn daemon(&self) -> Result<MutexGuard<'_, D>, AppError> {
        self.daemon.lock().map_err(|_| AppError::DaemonUnavailable)
    }

    /// Vault status.
    pub fn vault_status(&self) -> Result<EstadoCofre, AppError> {
        cliente::estado(&mut *self.daemon()?)
    }

    /// Create vault (first use, Phase 30): checks master password, PIN and the two
    /// confirmations here and only then asks the daemon. With a field error, nothing goes
    /// to the daemon. The response carries no value at all; `new_vault` is zeroed on
    /// drop.
    pub fn create_vault(&self, new_vault: NovoCofre) -> Result<CreateResult, AppError> {
        let errors = new_vault.erros();
        if !errors.is_empty() {
            return Ok(CreateResult {
                created: false,
                errors,
            });
        }
        cliente::criar_cofre(
            &mut *self.daemon()?,
            new_vault.senha.bytes(),
            new_vault.pin.bytes(),
        )?;
        Ok(CreateResult {
            created: true,
            errors: Vec::new(),
        })
    }

    /// List credentials: only the list request, in one exchange; the second line of each
    /// item (username, port, engine) and the search username come from the app role
    /// summary (D65 c), without one detail request per entry. No secret. An item without
    /// summary or without level refuses the whole response.
    pub fn list_credentials(&self) -> Result<Vec<ItemCredencial>, AppError> {
        let items = cliente::lista(&mut *self.daemon()?)?;
        items
            .iter()
            .map(|item| ItemCredencial::novo(item).ok_or(AppError::InvalidResponse))
            .collect()
    }

    /// Credential detail: the panel of one entry. The name comes from the interface and is
    /// validated before going to the daemon.
    pub fn credential_detail(&self, entry: &str) -> Result<DetalheCredencial, AppError> {
        validar_nome_entrada(entry)?;
        let d = cliente::detalhe(&mut *self.daemon()?, entry)?;
        Ok(DetalheCredencial::novo(&d))
    }

    /// Copy field: the value goes from the daemon straight to the clipboard. The response
    /// is only "it worked" or the error; the value does not come back.
    ///
    /// The field is looked up in the entry detail: a secret of the type or a secret extra
    /// goes through the protocol copy request; a text field or a plain extra, by the value
    /// the daemon gave in the detail (never by what the interface sent).
    pub fn copy_field(&self, entry: &str, field: &str) -> Result<(), AppError> {
        let reference = Referencia::nova(entry, field)?;
        let value = {
            let mut d = self.daemon()?;
            let detail = cliente::detalhe(&mut *d, reference.entrada())?;
            let field = reference.campo();
            if detail.segredos.iter().any(|s| s == field) {
                cliente::copiar(&mut *d, reference.entrada(), Alvo::Campo(field.to_owned()))?
            } else if let Some((_, v)) = detail.campos.iter().find(|(k, _)| k == field) {
                Secret::new(v.as_bytes().to_vec())
            } else if let Some(x) = detail.extras.iter().find(|x| x.nome == field) {
                match &x.valor {
                    Some(v) => Secret::new(v.as_bytes().to_vec()),
                    None => cliente::copiar(
                        &mut *d,
                        reference.entrada(),
                        Alvo::Extra(field.to_owned()),
                    )?,
                }
            } else {
                return Err(AppError::FieldNotFound);
            }
        };
        self.copier.copiar(value)
    }

    /// Generate password: generates in `ward_core::generator` and keeps it here. The
    /// response has only size, strength and symbols; the value does not come back.
    pub fn generate_password(&self, options: Opcoes) -> Result<Gerada, AppError> {
        self.lock_generator()?.gerar(options)
    }

    /// Copy the generated password: same path as [`Self::copy_field`] (D4, S12). Response
    /// `()`.
    pub fn copy_generated_password(&self) -> Result<(), AppError> {
        let value = self.lock_generator()?.para_copiar()?;
        self.copier.copiar(value)
    }

    /// Reveal the generated password: the only command that takes the value to the
    /// interface, on the click on "reveal" (S11).
    pub fn reveal_generated_password(&self) -> Result<Revelada, AppError> {
        self.lock_generator()?.revelar()
    }

    /// Discard the generated password: on leaving the screen, the password is zeroed.
    pub fn discard_generated_password(&self) -> Result<(), AppError> {
        self.lock_generator()?.descartar();
        Ok(())
    }

    /// Reveal field: the value of a secret of the entry (field of the type or secret
    /// extra), only on the click on "reveal" (S11). A text field does not go through
    /// here.
    pub fn reveal_field(&self, entry: &str, field: &str) -> Result<Revelada, AppError> {
        let reference = Referencia::nova(entry, field)?;
        let mut d = self.daemon()?;
        let detail = cliente::detalhe(&mut *d, reference.entrada())?;
        let field = reference.campo();
        let target = if detail.segredos.iter().any(|s| s == field) {
            Alvo::Campo(field.to_owned())
        } else if detail
            .extras
            .iter()
            .any(|x| x.nome == field && x.valor.is_none())
        {
            Alvo::Extra(field.to_owned())
        } else {
            return Err(AppError::FieldNotFound);
        };
        let value = cliente::revelar(&mut *d, reference.entrada(), target)?;
        Revelada::de_segredo(&value).ok_or(AppError::Refused)
    }

    /// TOTP code: the current code of the entry and the seconds until it changes.
    pub fn totp_code(&self, entry: &str) -> Result<TotpCode, AppError> {
        validar_nome_entrada(entry)?;
        let c = cliente::totp(&mut *self.daemon()?, entry)?;
        Ok(TotpCode {
            code: c.codigo.as_str().to_owned(),
            remaining: c.restante,
        })
    }

    /// Copy the TOTP code: the code goes to the clipboard (D4).
    pub fn copy_totp_code(&self, entry: &str) -> Result<(), AppError> {
        validar_nome_entrada(entry)?;
        let c = cliente::totp(&mut *self.daemon()?, entry)?;
        self.copier
            .copiar(Secret::new(c.codigo.as_bytes().to_vec()))
    }

    /// Use the generated password: the generator password goes to the form field, on the
    /// Rust side. The interface receives only the size (for the mask).
    pub fn use_generated_password(&self, field: &str) -> Result<usize, AppError> {
        let field = GENERATABLE_FIELDS
            .iter()
            .find(|c| **c == field)
            .ok_or(AppError::InvalidRequest)?;
        let password = self.lock_generator()?.para_copiar()?;
        let size = password.len();
        self.lock_used().insert((*field).to_owned(), password);
        Ok(size)
    }

    /// Discard form: on canceling or leaving the form, zeroes the used passwords and the
    /// generator one.
    pub fn discard_form(&self) {
        self.lock_used().clear();
        self.lock_loaded().clear();
        self.lock_attachments().limpar();
        if let Ok(mut g) = self.lock_generator() {
            g.descartar();
        }
    }

    /// Save credential: builds the draft on the Rust side (kept secrets and generated
    /// password do not go through the interface) and sends the save entry request. The
    /// response is the saved name or the errors per field.
    pub fn save_credential(
        &self,
        previous: Option<&str>,
        mut f: Formulario,
    ) -> Result<SaveResult, AppError> {
        let kind = formulario::tipo_do_formulario(&f.tipo).ok_or(AppError::InvalidRequest)?;
        if let Some(a) = previous {
            validar_nome_entrada(a)?;
        }
        let mut d = self.daemon()?;
        let edited = match previous {
            Some(a) => {
                let det = cliente::detalhe(&mut *d, a)?;
                if det.tipo != kind {
                    return Err(AppError::InvalidRequest);
                }
                Some(det)
            }
            None => None,
        };
        // Phase 39b (V5.12): kept attachments only when editing, only the ones the entry
        // has and each one once (the interface is not trusted). The content stays in the
        // daemon.
        let kept = std::mem::take(&mut f.anexos_mantidos);
        let stored: &[ward_core::protocolo::AnexoDetalhe] =
            edited.as_ref().map_or(&[], |d| d.anexos.as_slice());
        for (i, n) in kept.iter().enumerate() {
            if kept[..i].contains(n) || !stored.iter().any(|a| &a.nome == n) {
                return Err(AppError::InvalidRequest);
            }
        }
        let stored_extras: Vec<String> = edited
            .as_ref()
            .map(|det| {
                det.extras
                    .iter()
                    .filter(|x| x.valor.is_none())
                    .map(|x| x.nome.clone())
                    .collect()
            })
            .unwrap_or_default();
        let editing = edited
            .as_ref()
            .map(|det| (det.segredos.as_slice(), stored_extras.as_slice()));
        let used = self.lock_used();
        let loaded = self.lock_loaded();
        let mut sources = ServiceSources {
            daemon: &mut *d,
            entry: previous,
            used: &used,
            loaded: &loaded,
        };
        let built = match formulario::montar(&mut f, kind, editing, &mut sources) {
            Ok(m) => m,
            Err(Falta::App(e)) => return Err(e),
            Err(Falta::Campos(errors)) => {
                return Ok(SaveResult {
                    saved: None,
                    errors,
                });
            }
        };
        drop(used);
        drop(loaded);
        // The requested attachments, copied: if the daemon refuses, they stay chosen for
        // the next save.
        let attachments: Vec<AttachmentDraft> = self
            .lock_attachments()
            .escolhidos(&f.anexos)?
            .into_iter()
            .map(|p| AttachmentDraft {
                name: p.nome.clone(),
                content: CoreSecret::from_bytes(p.conteudo.expose().to_vec()),
            })
            .collect();
        let name = f.nome.clone();
        let draft = Draft {
            name: f.nome,
            group: f.grupo,
            environment: f.ambiente,
            project: f.projeto,
            tags: f.tags,
            data: built.dados,
            extras: built.extras,
            attachments: attachments,
        };
        if let Err(errors) = formulario::validar_local(&draft) {
            return Ok(SaveResult {
                saved: None,
                errors,
            });
        }
        match cliente::salvar(&mut *d, previous.map(str::to_owned), draft, kept)? {
            cliente::Salvamento::Salvo => {
                drop(d);
                self.discard_form();
                Ok(SaveResult {
                    saved: Some(name),
                    errors: Vec::new(),
                })
            }
            cliente::Salvamento::Problemas(p) => Ok(SaveResult {
                saved: None,
                errors: formulario::erros_do_daemon(&p),
            }),
        }
    }

    /// Set level (Phase 35, S9): the name and the level come from the interface and are
    /// checked here; the PIN arrives already checked for format, in a [`Pin`] that zeroes.
    /// The response is the new level, without PIN. A wrong PIN is [`AppError::WrongPin`]
    /// (the screen queries the status afterwards: the 5th error locks).
    pub fn set_level(&self, entry: &str, level: &str, pin: Pin) -> Result<NivelMudado, AppError> {
        validar_nome_entrada(entry)?;
        let level = nivel_de_nome(level)?;
        cliente::mudar_nivel(&mut *self.daemon()?, entry, level, &pin)?;
        drop(pin);
        Ok(NivelMudado {
            entrada: entry.to_owned(),
            nivel: level.name(),
        })
    }

    /// Settings (Phase 38): lock timeout and external backup, no hash nor UUID.
    pub fn settings(&self) -> Result<Configuracoes, AppError> {
        cliente::configuracao(&mut *self.daemon()?)
    }

    /// Change PIN (Phase 38, V5.22, S9): checks the format of the three fields and the
    /// confirmation here; with a field error, nothing goes to the daemon. The daemon
    /// checks the current PIN (the 5th error in a row locks). The response carries no
    /// value.
    pub fn change_pin(&self, change: TrocaPin) -> Result<ResultadoTroca, AppError> {
        let errors = change.erros();
        if !errors.is_empty() {
            return Ok(ResultadoTroca::com_erros(errors));
        }
        cliente::trocar_pin(
            &mut *self.daemon()?,
            change.atual.bytes(),
            change.novo.bytes(),
        )?;
        Ok(ResultadoTroca::trocado())
    }

    /// Change master password (Phase 38, V5.22): the new one follows the first use rule;
    /// the daemon checks the current one. The response carries no value.
    pub fn change_master_password(&self, change: TrocaSenha) -> Result<ResultadoTroca, AppError> {
        let errors = change.erros();
        if !errors.is_empty() {
            return Ok(ResultadoTroca::com_erros(errors));
        }
        cliente::trocar_senha(
            &mut *self.daemon()?,
            change.atual.bytes(),
            change.nova.bytes(),
        )?;
        Ok(ResultadoTroca::trocado())
    }

    /// Set lock timeout (Phase 38, D30, D45): outside 1 to 1440 it does not even reach the
    /// daemon. The response is the config read again from the daemon.
    pub fn set_lock_timeout(&self, minutes: u64) -> Result<Configuracoes, AppError> {
        let minutes = minutos_validos(minutes)?;
        let mut d = self.daemon()?;
        cliente::tempo_trava(&mut *d, minutes)?;
        cliente::configuracao(&mut *d)
    }

    /// External backup (Phase 48, V7.9): enabled or not, the chosen UUID and, if it is
    /// connected, the USB drive (label neutralized, D51). No mount path.
    pub fn external_backup(&self) -> Result<BackupExternoVisto, AppError> {
        external_backup_of(&mut *self.daemon()?)
    }

    /// List USB drives (Phase 48): the USB drives the daemon sees, to choose from.
    pub fn list_usb_drives(&self) -> Result<Vec<PendriveVisto>, AppError> {
        let list = cliente::pendrives(&mut *self.daemon()?)?;
        Ok(list.iter().map(PendriveVisto::de).collect())
    }

    /// Set external backup (Phase 48, V7.9; D17, D30). Enable: the UUID (format of D45,
    /// checked in [`EscolhaDoPendrive::nova`]) must be of a USB drive the daemon sees now;
    /// otherwise nothing goes to the daemon. The UUID goes as the daemon gave it. Disable
    /// keeps the stored UUID. The response is the state read again.
    pub fn set_external_backup(
        &self,
        enabled: bool,
        uuid: Option<&str>,
    ) -> Result<BackupExternoVisto, AppError> {
        let choice = EscolhaDoPendrive::nova(enabled, uuid)?;
        let mut d = self.daemon()?;
        match choice {
            EscolhaDoPendrive::Ligar(u) => {
                let list = cliente::pendrives(&mut *d)?;
                let p = backup_externo::achar(&list, &u).ok_or(AppError::UsbDriveNotConnected)?;
                cliente::definir_backup_externo(&mut *d, true, Some(p.uuid.clone()))?;
            }
            EscolhaDoPendrive::Desligar => {
                let (_, current) = cliente::backup_externo(&mut *d)?;
                cliente::definir_backup_externo(&mut *d, false, current)?;
            }
        }
        external_backup_of(&mut *d)
    }

    /// List backups (Phase 47, V7.7): the versions of the local backup, from newest to
    /// oldest, only with the name and the instant (from the name). No content, size nor
    /// path: the protocol does not carry them.
    pub fn list_backups(&self) -> Result<Vec<VersaoVista>, AppError> {
        let list = cliente::backups(&mut *self.daemon()?)?;
        restaurar::versoes_de(&list)
    }

    /// Restore backup (Phase 47, V7.7; D17, D39, S9). The version must be in the format
    /// and in the list the daemon gives now; otherwise nothing goes to the daemon
    /// ([`AppError::BackupNotFound`]). The version password and the current PIN only go
    /// in the request and are zeroed on drop. After the swap, the open form (of the vault
    /// from before) is discarded and the response carries only how many entries the
    /// version has.
    pub fn restore_backup(&self, request: Restauracao) -> Result<Restaurado, AppError> {
        let Restauracao {
            versao: version,
            senha: password,
            pin,
        } = request;
        restaurar::versao_pedida(&version)?;
        let mut d = self.daemon()?;
        if !cliente::backups(&mut *d)?.iter().any(|b| b.nome == version) {
            return Err(AppError::BackupNotFound);
        }
        let r = cliente::restaurar_backup(&mut *d, &version, &password, &pin);
        drop(password);
        drop(pin);
        r?;
        let entries = cliente::lista(&mut *d)?.len();
        drop(d);
        self.discard_form();
        Ok(Restaurado {
            restaurado: true,
            entradas: entries,
        })
    }

    fn lock_used(&self) -> MutexGuard<'_, BTreeMap<String, Secret>> {
        // Panic with the lock held: the used passwords are discarded.
        self.used.lock().unwrap_or_else(|e| {
            let mut m = e.into_inner();
            m.clear();
            m
        })
    }

    fn lock_generator(&self) -> Result<std::sync::MutexGuard<'_, Gerador>, AppError> {
        // A panic with the lock held does not leave the password stuck: the state is
        // discarded.
        Ok(self.generator.lock().unwrap_or_else(|e| {
            let mut g = e.into_inner();
            g.descartar();
            g
        }))
    }

    /// "Lock now" (window and tray, Phase 39): clears the clipboard (if it still has the
    /// copied value) and what the form and the generator kept **before** asking for the
    /// lock, and the agent request that was waiting for the unlock leaves the screen. Even
    /// with the daemon down, what is local is cleared.
    pub fn lock(&self) -> Result<(), AppError> {
        self.on_exit();
        let r = cliente::travar(&mut *self.daemon()?);
        self.on_exit();
        r
    }

    /// Status for the tray (Phase 39): only the status request, which the daemon does not
    /// count as use.
    pub fn tray_state(&self) -> crate::bandeja::EstadoBandeja {
        crate::bandeja::EstadoBandeja::de(self.vault_status())
    }

    /// The tray saw the status change (Phase 53d): if the vault stopped being unlocked
    /// (lock coming from outside the app), zeroes what the form kept. Phase 39f (S12): and
    /// clears the clipboard at once, if it still has the Ward value, like the app's "Lock
    /// now".
    pub fn on_tray_transition(
        &self,
        before: crate::bandeja::EstadoBandeja,
        after: crate::bandeja::EstadoBandeja,
    ) {
        if crate::bandeja::descartar_na_passagem(before, after) {
            self.on_exit();
        }
    }

    /// The app watcher queries the status (Phase 39g): swaps the last status seen for the
    /// current one and, on the transition from unlocked to another, clears the clipboard
    /// and the form ([`Self::on_tray_transition`]). Works with or without a tray: `seen`
    /// belongs to the app, not to the menu. Returns `(before, now)`.
    pub fn check_transition(
        &self,
        seen: &Mutex<crate::bandeja::EstadoBandeja>,
    ) -> (crate::bandeja::EstadoBandeja, crate::bandeja::EstadoBandeja) {
        let now = self.tray_state();
        let before = {
            let mut v = seen.lock().unwrap_or_else(|e| e.into_inner());
            core::mem::replace(&mut *v, now)
        };
        self.on_tray_transition(before, now);
        (before, now)
    }

    /// On lock or app close: clears the clipboard if it still has the copied value and
    /// zeroes the standalone password.
    pub fn on_exit(&self) {
        self.copier.limpar_agora();
        self.discard_form();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon_falso::{DaemonFalso, SEGREDOS};
    use std::sync::Arc;
    use zeroize::Zeroizing;

    #[derive(Default)]
    struct Area(Mutex<Option<Vec<u8>>>);

    impl AreaTransferencia for Arc<Area> {
        fn escrever(&self, v: &[u8]) -> Result<(), AppError> {
            *self.0.lock().unwrap() = Some(v.to_vec());
            Ok(())
        }
        fn ler(&self) -> Result<Option<Zeroizing<Vec<u8>>>, AppError> {
            Ok(self.0.lock().unwrap().clone().map(Zeroizing::new))
        }
        fn limpar(&self) -> Result<(), AppError> {
            *self.0.lock().unwrap() = None;
            Ok(())
        }
    }

    fn service() -> (Service<DaemonFalso, Arc<Area>>, Arc<Area>) {
        let area = Arc::new(Area::default());
        let s = Service::new(DaemonFalso::cheio(), Copiador::novo(Arc::clone(&area)));
        (s, area)
    }

    /// The forms in which a secret could appear in the JSON that goes to the interface.
    fn forms(s: &str) -> Vec<String> {
        let json = serde_json::to_string(s).unwrap();
        let mut v = vec![s.to_owned(), json[1..json.len() - 1].to_owned()];
        // Line by line (the fake private key has several).
        v.extend(s.lines().filter(|l| l.len() >= 6).map(str::to_owned));
        v
    }

    fn assert_no_secret(json: &str, place: &str) {
        for s in SEGREDOS {
            for f in forms(s) {
                assert!(!json.contains(&f), "segredo em {place}");
            }
        }
    }

    #[test]
    fn attack_list_and_details_of_all_entries_without_secret() {
        // V5.3 on the Rust side: the response of each command of the Credentials screen,
        // as it goes to the interface (JSON), has no secret of the test vault, in any form.
        let (s, _) = service();
        let list = s.list_credentials().unwrap();
        assert_eq!(
            list.len(),
            serde_json::from_str::<serde_json::Value>(include_str!(
                "../../../dev/fixtures/entries.json"
            ))
            .unwrap()["entradas"]
                .as_array()
                .unwrap()
                .len()
        );
        assert_no_secret(&serde_json::to_string(&list).unwrap(), "lista");
        for item in &list {
            let d = s.credential_detail(&item.nome).unwrap();
            let json = serde_json::to_string(&d).unwrap();
            assert_no_secret(&json, &item.nome);
            assert!(!json.contains("valor\":\"falso"), "{}", item.nome);
        }
        // The secret extra goes only with the name.
        let g = s.credential_detail("gama-generico").unwrap();
        let license = g.extras.iter().find(|x| x.nome == "CHAVE_LICENCA").unwrap();
        assert!(license.segredo && license.valor.is_none());
        assert_eq!(g.anexos.len(), 1);
    }

    #[test]
    fn list_and_detail_of_acme_pg_dev_as_in_the_frame() {
        let (s, _) = service();
        let list = s.list_credentials().unwrap();
        let pg = list.iter().find(|i| i.nome == "acme-pg-dev").unwrap();
        assert_eq!(pg.resumo, "postgres · app@127.0.0.1:55432");
        assert_eq!((pg.tipo, pg.ambiente, pg.nivel), ("banco", "dev", "livre"));
        let tunnel = list.iter().find(|i| i.nome == "acme-pg-tunel").unwrap();
        assert_eq!(tunnel.resumo, "postgres · postgres:5432 · túnel SSH");
        let d = s.credential_detail("acme-pg-dev").unwrap();
        assert_eq!(d.subtitulo, "grupo acme · postgres");
        let labels: Vec<_> = d.campos.iter().map(|c| c.rotulo.as_str()).collect();
        assert_eq!(labels, ["Host", "Porta", "Base", "Usuário"]);
        assert_eq!(d.segredos.len(), 1);
        assert_eq!(d.segredos[0].rotulo, "Senha");
    }

    #[test]
    fn attack_copy_does_not_return_the_value() {
        let (s, area) = service();
        // The response type is `()`: there is no value for the interface to see.
        let response: Result<(), AppError> = s.copy_field("acme-pg-dev", "senha");
        assert_eq!(response, Ok(()));
        assert_eq!(format!("{response:?}"), "Ok(())");
        assert_eq!(
            area.0.lock().unwrap().as_deref(),
            Some(&b"falso-PG:s3nha/01@#"[..])
        );
        // Text field: the value comes from the daemon detail.
        s.copy_field("acme-pg-dev", "host").unwrap();
        assert_eq!(area.0.lock().unwrap().as_deref(), Some(&b"127.0.0.1"[..]));
        // Secret extra, through the copy request with an extra target.
        s.copy_field("gama-generico", "CHAVE_LICENCA").unwrap();
        assert_eq!(
            area.0.lock().unwrap().as_deref(),
            Some(&b"falso-licenca-01"[..])
        );
        s.on_exit();
        assert_eq!(*area.0.lock().unwrap(), None);
    }

    fn options() -> crate::gerador::Opcoes {
        crate::gerador::Opcoes {
            tamanho: 24,
            minusculas: true,
            maiusculas: true,
            digitos: true,
            simbolos: true,
        }
    }

    /// V5.21 with V5.5: the generator "copy" goes through the same `Copiador` (the
    /// clipboard receives the generated value, the response is `()`), and "generate" does
    /// not return the value.
    #[test]
    fn attack_generator_copies_without_returning_the_value() {
        let area = Arc::new(Area::default());
        let s = Service::new(DaemonFalso::cheio(), Copiador::novo(Arc::clone(&area)));
        assert_eq!(
            s.copy_generated_password(),
            Err(AppError::NoGeneratedPassword)
        );
        let g = s.generate_password(options()).unwrap();
        let response: Result<(), AppError> = s.copy_generated_password();
        assert_eq!(response, Ok(()));
        let copied = area.0.lock().unwrap().clone().unwrap();
        assert_eq!(copied.len(), 24);
        assert!(!format!("{g:?}").contains(core::str::from_utf8(&copied).unwrap()));
        assert_eq!(
            s.reveal_generated_password().unwrap().expor().as_bytes(),
            &copied[..]
        );
        // Leaving the screen zeroes; "copy" and "reveal" no longer have anything to hand.
        s.discard_generated_password().unwrap();
        assert_eq!(
            s.copy_generated_password(),
            Err(AppError::NoGeneratedPassword)
        );
        assert_eq!(
            s.reveal_generated_password().err(),
            Some(AppError::NoGeneratedPassword)
        );
        // Closing the app clears the clipboard (still with the value) and zeroes the
        // password.
        s.generate_password(options()).unwrap();
        s.copy_generated_password().unwrap();
        s.on_exit();
        assert_eq!(*area.0.lock().unwrap(), None);
        assert_eq!(
            s.copy_generated_password(),
            Err(AppError::NoGeneratedPassword)
        );
    }

    /// V5.5 in the generator: the text the user copied afterwards is not erased by the
    /// cleanup.
    #[test]
    fn attack_generator_cleanup_does_not_erase_user_text() {
        let area = Arc::new(Area::default());
        let s = Service::new(
            DaemonFalso::cheio(),
            Copiador::com_prazo(Arc::clone(&area), std::time::Duration::from_millis(50)),
        );
        s.generate_password(options()).unwrap();
        s.copy_generated_password().unwrap();
        *area.0.lock().unwrap() = Some(b"texto do usuario".to_vec());
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert_eq!(
            area.0.lock().unwrap().as_deref(),
            Some(&b"texto do usuario"[..])
        );
        // Without a new copy by the user, the cleanup erases the generated value.
        s.copy_generated_password().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert_eq!(*area.0.lock().unwrap(), None);
    }

    /// Phase 39f (S12, low finding of the M5 audit): the lock that comes from outside
    /// (inactivity, the CLI lock, screen locked), seen by the tray, clears the clipboard
    /// at once, if it still has the Ward value; the text the user copied afterwards
    /// stays; the periodic poll with the vault unlocked does not clear.
    #[test]
    fn attack_outside_lock_clears_the_clipboard_at_once() {
        use crate::bandeja::EstadoBandeja::*;
        for after in [Travado, SemDaemon, SemCofre] {
            let (s, area) = service();
            s.copy_field("acme-pg-dev", "senha").unwrap();
            s.on_tray_transition(Destravado, Destravado);
            s.on_tray_transition(Travado, Destravado);
            assert!(area.0.lock().unwrap().is_some(), "{after:?}");
            s.on_tray_transition(Destravado, after);
            assert_eq!(*area.0.lock().unwrap(), None, "{after:?}");

            s.copy_field("acme-pg-dev", "senha").unwrap();
            *area.0.lock().unwrap() = Some(b"texto do usuario".to_vec());
            s.on_tray_transition(Destravado, after);
            assert_eq!(
                area.0.lock().unwrap().as_deref(),
                Some(&b"texto do usuario"[..]),
                "{after:?}"
            );
        }
    }

    /// Attack (Phase 39g): the vault locks from outside (the CLI lock, inactivity) with
    /// the value copied, in an app **without a tray**. The watcher sees the transition
    /// through the app's `seen` and clears the clipboard at once; before, without a tray,
    /// nothing cleared until the 30 s deadline.
    #[test]
    fn attack_outside_lock_without_tray_clears_the_clipboard_at_once() {
        use crate::bandeja::EstadoBandeja::*;
        let (s, area) = service();
        let seen = Mutex::new(SemDaemon);
        assert_eq!(s.check_transition(&seen), (SemDaemon, Destravado));
        s.copy_field("acme-pg-dev", "senha").unwrap();
        // A poll without change does not clear.
        assert_eq!(s.check_transition(&seen), (Destravado, Destravado));
        assert!(area.0.lock().unwrap().is_some());
        // The lock comes from outside the app: straight in the daemon, without going
        // through its `lock`.
        cliente::travar(&mut *s.daemon().unwrap()).unwrap();
        assert!(area.0.lock().unwrap().is_some());
        assert_eq!(s.check_transition(&seen), (Destravado, Travado));
        assert_eq!(*area.0.lock().unwrap(), None);
        assert_eq!(*seen.lock().unwrap(), Travado);
    }

    #[test]
    fn attack_error_does_not_carry_the_request() {
        let (s, _) = service();
        let e = s.copy_field("acme-pg-dev\n", "senha").unwrap_err();
        assert_eq!(e, AppError::InvalidRequest);
        assert!(!e.to_string().contains("acme"));
        assert_eq!(
            s.copy_field("acme-pg-dev", "inventado").unwrap_err(),
            AppError::FieldNotFound
        );
        assert_eq!(s.vault_status(), Ok(EstadoCofre::Destravado));
    }

    #[test]
    fn attack_malformed_entry_name_does_not_reach_the_daemon() {
        let (s, _) = service();
        let long = "a".repeat(ward_core::limits::MAX_NAME + 1);
        for name in [
            "",
            " acme-pg-dev",
            "acme-pg-dev\n",
            "acme\u{202E}gp",
            "acme\u{0}",
            long.as_str(),
        ] {
            assert_eq!(
                s.credential_detail(name).unwrap_err(),
                AppError::InvalidRequest,
                "{name:?}"
            );
        }
        assert_eq!(
            s.credential_detail("nao-existe").unwrap_err(),
            AppError::EntryNotFound
        );
    }

    fn form(json: serde_json::Value) -> Formulario {
        serde_json::from_value(json).unwrap()
    }

    fn new_login(name: &str) -> serde_json::Value {
        serde_json::json!({
            "tipo": "login", "nome": name, "grupo": "acme", "ambiente": "dev",
            "projeto": "site", "tags": ["wordpress", "painel"],
            "campos": {"url": "https://painel.exemplo.com/login", "usuario": "wp_novo"},
            "segredos": {"senha": {"novo": "falso-nova-senha-77"},
                          "totp_semente": {"novo": "jbsw y3dp ehpk 3pxp"}},
            "extras": [{"nome": "SITE_ID", "segredo": false, "valor": {"novo": "acme-site-01"}}]
        })
    }

    /// Phase 33 (V5.10): creates a `login` (with the normalized seed) and edits it keeping
    /// the stored password, without the value coming back to the interface.
    #[test]
    fn save_new_and_edit_keeping_secret() {
        let (s, _) = service();
        let r = s
            .save_credential(None, form(new_login("acme-wp-novo")))
            .unwrap();
        assert_eq!(r.saved.as_deref(), Some("acme-wp-novo"), "{r:?}");
        let d = s.credential_detail("acme-wp-novo").unwrap();
        assert_eq!(d.nivel, "com-aprovacao");
        assert_eq!(d.tags, ["wordpress", "painel"]);
        assert_eq!(
            s.reveal_field("acme-wp-novo", "totp_semente")
                .unwrap()
                .expor(),
            "JBSWY3DPEHPK3PXP"
        );
        assert_eq!(s.totp_code("acme-wp-novo").unwrap().code.len(), 6);
        // Edit of a `livre` entry: keeps the password, renames, the level stays.
        let edit = serde_json::json!({
            "tipo": "login", "nome": "acme-totp-dev2", "grupo": "acme", "ambiente": "dev",
            "projeto": "", "tags": [],
            "campos": {"url": "http://localhost:8083/login", "usuario": "wp_admin"},
            "segredos": {"senha": "manter", "totp_semente": "manter"},
            "extras": []
        });
        let r = s
            .save_credential(Some("acme-totp-dev"), form(edit))
            .unwrap();
        assert_eq!(r.saved.as_deref(), Some("acme-totp-dev2"), "{r:?}");
        assert_eq!(
            s.credential_detail("acme-totp-dev2").unwrap().nivel,
            "livre"
        );
        assert_eq!(
            s.reveal_field("acme-totp-dev2", "senha").unwrap().expor(),
            "falso-WP-senha-01"
        );
        assert_eq!(
            s.credential_detail("acme-totp-dev").unwrap_err(),
            AppError::EntryNotFound
        );
    }

    /// V5.10: saving without a required field returns the error on the field (from
    /// `ward-core`), without value.
    #[test]
    fn save_without_required_shows_field_error() {
        let (s, _) = service();
        let vazio = serde_json::json!({
            "tipo": "login", "nome": "", "grupo": "", "ambiente": "", "projeto": "",
            "tags": [], "campos": {}, "segredos": {"senha": {"novo": "falso-x-123456"}}, "extras": []
        });
        let r = s.save_credential(None, form(vazio)).unwrap();
        assert_eq!(r.saved, None);
        let campos: Vec<_> = r.errors.iter().map(|e| e.campo).collect();
        for c in ["nome", "grupo", "ambiente", "url", "usuario"] {
            assert!(campos.contains(&c), "{c}: {r:?}");
        }
        assert!(!serde_json::to_string(&r).unwrap().contains("falso-x"));
        // Repeated name: daemon error on the name field.
        let r = s
            .save_credential(None, form(new_login("acme-wp-dev")))
            .unwrap();
        assert!(r.errors.iter().any(|e| e.campo == "nome"), "{r:?}");
    }

    /// D35/V5.13: `otpauth://` with SHA256, 8 digits or 60 s is refused on save, with the
    /// fixed message on the seed field, and nothing is stored.
    #[test]
    fn attack_unsupported_otpauth_is_not_saved() {
        let (s, _) = service();
        for uri in [
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&algorithm=SHA256",
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&digits=8",
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&period=60",
        ] {
            let mut j = new_login("acme-otp-novo");
            j["segredos"]["totp_semente"] = serde_json::json!({ "novo": uri });
            let r = s.save_credential(None, form(j)).unwrap();
            assert_eq!(r.saved, None);
            assert_eq!(r.errors[0].campo, "totp_semente");
            assert_eq!(r.errors[0].motivo, "parâmetros de TOTP não suportados");
            assert_eq!(
                s.credential_detail("acme-otp-novo").unwrap_err(),
                AppError::EntryNotFound
            );
        }
        let mut j = new_login("acme-otp-novo");
        j["segredos"]["totp_semente"] = serde_json::json!({
            "novo": "otpauth://totp/Acme?secret=JBSWY3DPEHPK3PXP&algorithm=SHA1&digits=6&period=30"
        });
        assert!(s.save_credential(None, form(j)).unwrap().saved.is_some());
    }

    /// V5.14: the form generator fills the password on the Rust side: "use" returns only
    /// the size, and what is saved is the generated password.
    #[test]
    fn form_generator_fills_the_password_without_passing_through_the_interface() {
        let (s, _) = service();
        assert_eq!(
            s.use_generated_password("senha"),
            Err(AppError::NoGeneratedPassword)
        );
        s.generate_password(crate::gerador::Opcoes {
            tamanho: 32,
            ..options()
        })
        .unwrap();
        let gerada = s.reveal_generated_password().unwrap().expor().to_owned();
        assert_eq!(s.use_generated_password("senha"), Ok(32));
        for field in ["token", "totp_semente", "", "senha "] {
            assert_eq!(
                s.use_generated_password(field),
                Err(AppError::InvalidRequest),
                "{field:?}"
            );
        }
        let mut j = new_login("acme-gerada");
        j["segredos"]["senha"] = serde_json::json!("gerada");
        let r = s.save_credential(None, form(j)).unwrap();
        assert_eq!(r.saved.as_deref(), Some("acme-gerada"));
        assert_eq!(
            s.reveal_field("acme-gerada", "senha").unwrap().expor(),
            gerada
        );
        // After saving, the used password goes away.
        let mut j = new_login("acme-gerada-2");
        j["segredos"]["senha"] = serde_json::json!("gerada");
        assert_eq!(
            s.save_credential(None, form(j)),
            Err(AppError::NoGeneratedPassword)
        );
    }

    /// S11: "keep" is only valid when editing and only for a secret the edited entry has;
    /// a secret of another entry cannot be pulled into a new one. The type does not
    /// change when editing, a field key of another type is refused, and an entry with
    /// attachments is not edited (the attachments would be erased).
    #[test]
    fn attack_form_forged_by_the_interface() {
        let (s, _) = service();
        let mut j = new_login("acme-roubo");
        j["segredos"]["senha"] = serde_json::json!("manter");
        assert_eq!(
            s.save_credential(None, form(j)),
            Err(AppError::InvalidRequest)
        );
        let mut j = new_login("acme-roubo");
        j["extras"] = serde_json::json!([
            {"nome": "X", "segredo": false, "valor": {"manter": "CHAVE_LICENCA"}}
        ]);
        assert_eq!(
            s.save_credential(None, form(j)),
            Err(AppError::InvalidRequest)
        );
        // When editing another entry, the extra it does not have is not valid either.
        let mut e = new_login("acme-wp-dev");
        e["extras"] = serde_json::json!([
            {"nome": "X", "segredo": true, "valor": {"manter": "CHAVE_LICENCA"}}
        ]);
        assert_eq!(
            s.save_credential(Some("acme-wp-dev"), form(e)),
            Err(AppError::InvalidRequest)
        );
        // Type swapped when editing.
        let mut t = new_login("beta-github-api");
        t["campos"] = serde_json::json!({});
        assert_eq!(
            s.save_credential(Some("beta-github-api"), form(t)),
            Err(AppError::InvalidRequest)
        );
        // Field of another type.
        let mut c = new_login("acme-x");
        c["campos"]["host"] = serde_json::json!("127.0.0.1");
        assert_eq!(
            s.save_credential(None, form(c)),
            Err(AppError::InvalidRequest)
        );
        // Login fields in a type of Phase 34, and a made-up type.
        for kind in ["banco", "servidor", "ftp-sftp", "root", ""] {
            let mut c = new_login("acme-x");
            c["tipo"] = serde_json::json!(kind);
            assert_eq!(
                s.save_credential(None, form(c)),
                Err(AppError::InvalidRequest),
                "{kind}"
            );
        }
        // Forged kept attachments (Phase 39b): a name the entry does not have, repeated
        // or on a new entry; nothing is stored.
        for (previous, kept) in [
            (Some("gama-generico"), serde_json::json!(["outro.env"])),
            (
                Some("gama-generico"),
                serde_json::json!(["app.env", "app.env"]),
            ),
            (Some("gama-generico"), serde_json::json!(["../app.env"])),
            (None, serde_json::json!(["app.env"])),
        ] {
            let mut g = serde_json::json!({
                "tipo": "generico", "nome": "gama-generico-2", "grupo": "gama",
                "ambiente": "stage", "projeto": "", "tags": [], "campos": {}, "segredos": {},
                "extras": [{"nome": "REGIAO", "segredo": false, "valor": {"novo": "r-02"}}]
            });
            g["anexos_mantidos"] = kept;
            assert_eq!(
                s.save_credential(previous, form(g)),
                Err(AppError::InvalidRequest),
                "{previous:?}"
            );
        }
        assert_eq!(
            s.credential_detail("gama-generico").unwrap().anexos.len(),
            1
        );
        assert_eq!(
            s.credential_detail("gama-generico-2").unwrap_err(),
            AppError::EntryNotFound
        );
        // A malformed previous name does not even reach the daemon.
        assert_eq!(
            s.save_credential(Some("acme\n"), form(new_login("x"))),
            Err(AppError::InvalidRequest)
        );
    }

    /// V5.3/V5.11 (D65 b): only the reveal field command carries a value; no other form
    /// response (save, TOTP) carries a vault secret. A text field is not "revealed".
    #[test]
    fn attack_reveal_only_secrets_and_responses_without_secret() {
        let (s, area) = service();
        assert_eq!(
            s.reveal_field("acme-pg-dev", "senha").unwrap().expor(),
            "falso-PG:s3nha/01@#"
        );
        assert_eq!(
            s.reveal_field("gama-generico", "CHAVE_LICENCA")
                .unwrap()
                .expor(),
            "falso-licenca-01"
        );
        assert_eq!(
            s.reveal_field("gama-generico", "REGIAO").unwrap_err(),
            AppError::FieldNotFound
        );
        assert_eq!(
            s.reveal_field("acme-pg-dev", "host").unwrap_err(),
            AppError::FieldNotFound
        );
        assert_eq!(
            s.reveal_field("acme-pg-dev\n", "senha").unwrap_err(),
            AppError::InvalidRequest
        );
        assert!(
            !format!("{:?}", s.reveal_field("acme-pg-dev", "senha").unwrap()).contains("falso")
        );
        let totp = serde_json::to_string(&s.totp_code("acme-wp-dev").unwrap()).unwrap();
        assert_no_secret(&totp, "totp");
        assert_eq!(
            s.totp_code("acme-pg-dev").unwrap_err(),
            AppError::FieldNotFound
        );
        s.copy_totp_code("acme-wp-dev").unwrap();
        assert_eq!(area.0.lock().unwrap().as_ref().map(Vec::len), Some(6));
        let mut j = new_login("acme-edit");
        j["segredos"]["senha"] = serde_json::json!({"novo": "falso-WP-senha-01"});
        let r = serde_json::to_string(&s.save_credential(None, form(j)).unwrap()).unwrap();
        assert_no_secret(&r, "salvar");
        for item in s.list_credentials().unwrap() {
            let d = serde_json::to_string(&s.credential_detail(&item.nome).unwrap()).unwrap();
            assert_no_secret(&d, &item.nome);
        }
    }

    /// D65 c: the screen list comes from a single exchange (without one detail request
    /// per entry), with the 2nd line and the search username taken from the summary.
    #[test]
    fn d65_list_in_a_single_exchange_with_summary_and_username() {
        struct Counter(DaemonFalso, Arc<std::sync::atomic::AtomicUsize>);
        impl Daemon for Counter {
            fn trocar(&mut self, p: &[u8]) -> Result<Zeroizing<Vec<u8>>, AppError> {
                self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                self.0.trocar(p)
            }
        }
        let n = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let s = Service::new(
            Counter(DaemonFalso::cheio(), Arc::clone(&n)),
            Copiador::novo(Arc::new(Area::default())),
        );
        let list = s.list_credentials().unwrap();
        assert_eq!(n.load(std::sync::atomic::Ordering::SeqCst), 1);
        let pg = list.iter().find(|i| i.nome == "acme-pg-dev").unwrap();
        assert_eq!(pg.usuario.as_deref(), Some("app"));
        let totp = list.iter().find(|i| i.nome == "acme-totp-dev").unwrap();
        assert_eq!(totp.usuario.as_deref(), Some("wp_admin"));
        assert!(totp.resumo.ends_with("TOTP"), "{}", totp.resumo);
        let g = list.iter().find(|i| i.nome == "gama-generico").unwrap();
        assert_eq!(g.resumo, "2 campos extras · 1 anexo");
        assert_no_secret(&serde_json::to_string(&list).unwrap(), "lista");
    }

    #[test]
    fn empty_vault() {
        let area = Arc::new(Area::default());
        let s = Service::new(DaemonFalso::vazio(), Copiador::novo(area));
        assert!(s.list_credentials().unwrap().is_empty());
    }

    const RIGHT_PIN: &str = "135790";
    const WRONG_PIN: &str = "000000";

    fn pin(p: &str) -> Pin {
        Pin::novo(p.to_owned()).unwrap()
    }

    fn level_of(s: &Service<DaemonFalso, Arc<Area>>, entry: &str) -> &'static str {
        s.credential_detail(entry).unwrap().nivel
    }

    #[test]
    fn set_level_with_pin_and_the_three_levels_also_in_prod() {
        // V5.15: with the right PIN the level changes, in the list and in the detail; in
        // prod the three levels work the same (the environment does not change the
        // rules).
        let (s, _) = service();
        assert_eq!(level_of(&s, "acme-pg-prod"), "com-aprovacao");
        for level in ["livre", "bloqueado", "com-aprovacao", "livre"] {
            let r = s.set_level("acme-pg-prod", level, pin(RIGHT_PIN)).unwrap();
            assert_eq!((r.entrada.as_str(), r.nivel), ("acme-pg-prod", level));
            assert_eq!(level_of(&s, "acme-pg-prod"), level);
            let item = s
                .list_credentials()
                .unwrap()
                .into_iter()
                .find(|i| i.nome == "acme-pg-prod")
                .unwrap();
            assert_eq!(item.nivel, level);
        }
        assert_eq!(
            s.set_level("nao-existe", "livre", pin(RIGHT_PIN))
                .unwrap_err(),
            AppError::EntryNotFound
        );
    }

    #[test]
    fn attack_wrong_pin_does_not_change_and_the_fifth_error_locks() {
        // S9 / V5.15: a wrong PIN answers the wrong PIN message and the level does not
        // change; 4 errors and 1 success reset the count; the 5th error in a row locks the
        // vault, and once locked not even the right PIN changes the level.
        let (s, _) = service();
        let fail = || {
            s.set_level("acme-pg-dev", "bloqueado", pin(WRONG_PIN))
                .unwrap_err()
        };
        for _ in 0..4 {
            assert_eq!(fail(), AppError::WrongPin);
            assert_eq!(level_of(&s, "acme-pg-dev"), "livre");
            assert_eq!(s.vault_status(), Ok(EstadoCofre::Destravado));
        }
        s.set_level("acme-pg-dev", "livre", pin(RIGHT_PIN)).unwrap();
        for _ in 0..4 {
            assert_eq!(fail(), AppError::WrongPin);
        }
        assert_eq!(s.vault_status(), Ok(EstadoCofre::Destravado));
        assert_eq!(level_of(&s, "acme-pg-dev"), "livre");
        assert_eq!(fail(), AppError::WrongPin);
        assert_eq!(s.vault_status(), Ok(EstadoCofre::Travado));
        assert_eq!(
            s.set_level("acme-pg-dev", "bloqueado", pin(RIGHT_PIN))
                .unwrap_err(),
            AppError::Locked
        );
        assert_eq!(
            s.credential_detail("acme-pg-dev").unwrap_err(),
            AppError::Locked
        );
    }

    #[test]
    fn attack_forged_name_or_level_does_not_reach_the_daemon() {
        // A malformed name or a made-up level are refused before the daemon: they change
        // nothing and do not count as a PIN error (not even with the wrong PIN).
        let (s, _) = service();
        for _ in 0..6 {
            for (entry, level) in [
                ("acme-pg-dev", "admin"),
                ("acme-pg-dev", "Livre"),
                ("acme-pg-dev", ""),
                ("acme\npg", "bloqueado"),
                ("", "bloqueado"),
                (" acme-pg-dev", "bloqueado"),
            ] {
                assert_eq!(
                    s.set_level(entry, level, pin(WRONG_PIN)).unwrap_err(),
                    AppError::InvalidRequest,
                    "{entry:?} {level:?}"
                );
            }
        }
        assert_eq!(s.vault_status(), Ok(EstadoCofre::Destravado));
        assert_eq!(level_of(&s, "acme-pg-dev"), "livre");
    }

    #[test]
    fn attack_set_level_response_and_error_without_pin() {
        // The response (JSON that goes to the interface) and the error message do not
        // carry the PIN nor a vault secret.
        let (s, _) = service();
        let r = s.set_level("acme-wp-dev", "livre", pin(RIGHT_PIN)).unwrap();
        let ok = serde_json::to_string(&r).unwrap();
        assert!(!ok.contains(RIGHT_PIN), "{ok}");
        assert_no_secret(&ok, "mudar_nivel");
        let e = s
            .set_level("acme-wp-dev", "livre", pin(WRONG_PIN))
            .unwrap_err();
        let text = format!("{e} {e:?} {}", e.code());
        assert!(!text.chars().any(|c| c.is_ascii_digit()), "{text}");
    }

    // ---- Phase 34: database, server, ftp/sftp, attachments and "Connect" ----

    use crate::anexos::Escolha;
    use crate::terminal::Token;
    use std::path::PathBuf;

    /// Test picker: returns the path the test set.
    #[derive(Clone, Default)]
    struct FixedPicker(Arc<Mutex<Option<PathBuf>>>);

    impl crate::anexos::Seletor for FixedPicker {
        fn escolher(&self) -> Result<Option<PathBuf>, AppError> {
            Ok(self.0.lock().unwrap().clone())
        }
    }

    /// Test terminal: keeps the requested entry and token.
    #[derive(Clone, Default)]
    struct FixedTerminal(Arc<Mutex<Vec<(String, String)>>>);

    impl crate::terminal::Terminal for FixedTerminal {
        fn abrir(&self, entry: &str, token: &Token) -> Result<(), AppError> {
            // `Token` does not expose the value outside the module; the test only sees
            // the size and the Debug.
            self.0
                .lock()
                .unwrap()
                .push((entry.to_owned(), format!("{token:?}:{}", token.len())));
            Ok(())
        }
    }

    fn service_34() -> (Service<DaemonFalso, Arc<Area>>, FixedPicker, FixedTerminal) {
        let (s, _) = service();
        let sel = FixedPicker::default();
        let term = FixedTerminal::default();
        (
            s.with_picker(sel.clone()).with_terminal(term.clone()),
            sel,
            term,
        )
    }

    fn dir_34(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ward-servico-34-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn test_key() -> String {
        // Built in pieces: the whole text of a key does not stay in the repository.
        [
            "-----BEGIN ",
            "OPENSSH",
            " PRIVATE",
            " KEY-----\n",
            "falso-chave-fase34\n",
            "-----END ",
            "OPENSSH",
            " PRIVATE",
            " KEY-----\n",
        ]
        .concat()
    }

    fn new_server(name: &str) -> serde_json::Value {
        serde_json::json!({
            "tipo": "servidor", "nome": name, "grupo": "clientex", "ambiente": "stage",
            "projeto": "", "tags": [],
            "campos": {"host": "127.0.0.1", "porta": "2222", "usuario": "deploy", "salto": ""},
            "segredos": {"senha": "vazio", "chave_privada": {"novo": test_key()},
                "passphrase": {"novo": "falso-frase-34"}},
            "extras": [], "anexos": []
        })
    }

    /// V5.10: the three types of the phase save and edit (keeping the secret, the level
    /// stays).
    #[test]
    fn database_server_and_ftp_save_and_edit() {
        let (s, _, _) = service_34();
        let database = serde_json::json!({
            "tipo": "banco", "nome": "clientex-pg-stage", "grupo": "clientex", "ambiente": "stage",
            "projeto": "", "tags": [],
            "campos": {"motor": "postgres", "host": "127.0.0.1", "porta": "", "base": "app_teste",
                "usuario": "app", "tunel": "acme-ssh-senha-stage"},
            "segredos": {"senha": {"novo": "falso-banco-34"}},
            "extras": [], "anexos": []
        });
        let r = s.save_credential(None, form(database)).unwrap();
        assert_eq!(r.saved.as_deref(), Some("clientex-pg-stage"), "{r:?}");
        let d = s.credential_detail("clientex-pg-stage").unwrap();
        assert!(
            d.campos
                .iter()
                .any(|c| c.id == "porta" && c.valor == "5432")
        );
        assert!(
            d.campos
                .iter()
                .any(|c| c.id == "tunel" && c.valor == "acme-ssh-senha-stage")
        );

        let r = s
            .save_credential(None, form(new_server("clientex-ssh-stage")))
            .unwrap();
        assert_eq!(r.saved.as_deref(), Some("clientex-ssh-stage"), "{r:?}");
        assert_eq!(
            s.credential_detail("clientex-ssh-stage")
                .unwrap()
                .segredos
                .len(),
            2
        );

        let ftp = serde_json::json!({
            "tipo": "ftp-sftp", "nome": "clientex-ftp", "grupo": "clientex", "ambiente": "dev",
            "projeto": "", "tags": [],
            "campos": {"protocolo": "ftp", "host": "127.0.0.1", "porta": "", "usuario": "sftpuser"},
            "segredos": {"senha": {"novo": "falso-ftp-34"}, "chave_privada": "vazio", "passphrase": "vazio"},
            "extras": [], "anexos": []
        });
        let r = s.save_credential(None, form(ftp)).unwrap();
        assert_eq!(r.saved.as_deref(), Some("clientex-ftp"), "{r:?}");

        // Edit of the server with a test vault key, keeping key and passphrase.
        let ed = serde_json::json!({
            "tipo": "servidor", "nome": "acme-ssh-chave-prod", "grupo": "acme", "ambiente": "prod",
            "projeto": "", "tags": [],
            "campos": {"host": "127.0.0.1", "porta": "2223", "usuario": "deploy", "salto": ""},
            "segredos": {"senha": "vazio", "chave_privada": "manter", "passphrase": "manter"},
            "extras": [], "anexos": []
        });
        let r = s
            .save_credential(Some("acme-ssh-chave-prod"), form(ed))
            .unwrap();
        assert_eq!(r.saved.as_deref(), Some("acme-ssh-chave-prod"), "{r:?}");
        let key = s
            .reveal_field("acme-ssh-chave-prod", "chave_privada")
            .unwrap();
        assert!(key.expor().contains("falso-chave-ed25519-teste"));
    }

    /// V5.10 and D33: the errors of the new types come back on the right field, without
    /// value.
    #[test]
    fn new_type_errors_on_the_field() {
        let (s, _, _) = service_34();
        let database = serde_json::json!({
            "tipo": "banco", "nome": "clientex-pg", "grupo": "clientex", "ambiente": "dev",
            "projeto": "", "tags": [],
            "campos": {"motor": "postgres", "host": "127.0.0.1", "porta": "99999", "base": "app.teste?x",
                "usuario": "app", "tunel": "acme-pg-dev"},
            "segredos": {"senha": {"novo": "falso-banco-erro"}},
            "extras": [], "anexos": []
        });
        let r = s.save_credential(None, form(database)).unwrap();
        let campos: Vec<_> = r.errors.iter().map(|e| e.campo).collect();
        assert!(
            campos.contains(&"base") && campos.contains(&"porta"),
            "{r:?}"
        );
        assert!(!serde_json::to_string(&r).unwrap().contains("falso-banco"));
        // A tunnel that is not a server: daemon error on the tunnel field.
        let mut b = serde_json::json!({
            "tipo": "banco", "nome": "clientex-pg", "grupo": "clientex", "ambiente": "dev",
            "projeto": "", "tags": [],
            "campos": {"motor": "postgres", "host": "127.0.0.1", "porta": "", "base": "app",
                "usuario": "app", "tunel": "acme-pg-dev"},
            "segredos": {"senha": {"novo": "falso-banco-erro"}},
            "extras": [], "anexos": []
        });
        let r = s.save_credential(None, form(b.clone())).unwrap();
        assert_eq!(r.errors.first().map(|e| e.campo), Some("tunel"), "{r:?}");
        b["campos"]["motor"] = serde_json::json!("oracle");
        b["campos"]["tunel"] = serde_json::json!("");
        let r = s.save_credential(None, form(b)).unwrap();
        assert!(r.errors.iter().any(|e| e.campo == "motor"), "{r:?}");
        // No password nor key on the server: error on the auth field.
        let mut v = new_server("clientex-ssh");
        v["segredos"] =
            serde_json::json!({"senha": "vazio", "chave_privada": "vazio", "passphrase": "vazio"});
        let r = s.save_credential(None, form(v)).unwrap();
        assert!(r.errors.iter().any(|e| e.campo == "autenticacao"), "{r:?}");
    }

    /// S1: the interface forging both methods, or a key on ftp, is refused by the
    /// validation; "keep" of the key on an entry that only has a password is an invalid
    /// request.
    #[test]
    fn attack_auth_forged_by_the_interface() {
        let (s, _, _) = service_34();
        let mut v = new_server("clientex-ssh");
        v["segredos"]["senha"] = serde_json::json!({"novo": "falso-as-duas-34"});
        let r = s.save_credential(None, form(v)).unwrap();
        assert!(r.errors.iter().any(|e| e.campo == "autenticacao"), "{r:?}");
        let mut f = new_server("clientex-ftp");
        f["tipo"] = serde_json::json!("ftp-sftp");
        f["campos"] = serde_json::json!({"protocolo": "ftp", "host": "127.0.0.1", "porta": "", "usuario": "u"});
        let r = s.save_credential(None, form(f)).unwrap();
        assert!(r.errors.iter().any(|e| e.campo == "autenticacao"), "{r:?}");
        let ed = serde_json::json!({
            "tipo": "servidor", "nome": "acme-ssh-senha-stage", "grupo": "acme", "ambiente": "stage",
            "projeto": "", "tags": [],
            "campos": {"host": "127.0.0.1", "porta": "2222", "usuario": "deploy", "salto": ""},
            "segredos": {"senha": "vazio", "chave_privada": "manter", "passphrase": "vazio"},
            "extras": [], "anexos": []
        });
        assert_eq!(
            s.save_credential(Some("acme-ssh-senha-stage"), form(ed)),
            Err(AppError::InvalidRequest)
        );
    }

    /// V5.12 (S11): the attachment is chosen and read in Rust; the response carries only
    /// id, name and size; it goes in the draft by the id, and the detail shows name and
    /// size.
    #[test]
    fn attack_attachment_does_not_return_to_the_interface_and_goes_in_the_draft() {
        let (s, sel, _) = service_34();
        let d = dir_34("anexo");
        let content = "falso-conteudo-do-anexo-34\nLINHA=falso-valor-34\n";
        std::fs::write(d.join("acme-vpn.ovpn"), content).unwrap();
        *sel.0.lock().unwrap() = Some(d.join("acme-vpn.ovpn"));
        let e = s.choose_attachment(None).unwrap();
        let json = serde_json::to_string(&e).unwrap();
        assert!(
            !json.contains("falso-conteudo") && !json.contains("falso-valor"),
            "{json}"
        );
        let Escolha::Anexado {
            id, tamanho: size, ..
        } = e
        else {
            panic!("{e:?}")
        };
        assert_eq!(size, content.len() as u64);
        let mut v = new_server("clientex-ssh-anexo");
        v["anexos"] = serde_json::json!([id]);
        let r = s.save_credential(None, form(v)).unwrap();
        assert_eq!(r.saved.as_deref(), Some("clientex-ssh-anexo"), "{r:?}");
        let det = s.credential_detail("clientex-ssh-anexo").unwrap();
        assert_eq!(det.anexos.len(), 1);
        assert_eq!(det.anexos[0].nome, "acme-vpn.ovpn");
        assert_eq!(det.anexos[0].tamanho, content.len() as u64);
        assert_no_secret(&serde_json::to_string(&det).unwrap(), "detalhe com anexo");
        assert!(
            !serde_json::to_string(&det)
                .unwrap()
                .contains("falso-conteudo")
        );
        // Once saved, the chosen attachments are discarded: the id is not valid again.
        let mut v = new_server("clientex-ssh-anexo2");
        v["anexos"] = serde_json::json!([id]);
        assert_eq!(
            s.save_credential(None, form(v)),
            Err(AppError::InvalidRequest)
        );
        let _ = std::fs::remove_dir_all(d);
    }

    fn key_53a(body: &str) -> String {
        let label = ["OPENSSH", " PRIVATE", " KEY"].concat();
        format!("-----BEGIN {label}-----\n{body}\n-----END {label}-----\n")
    }

    fn loaded_server(name: &str) -> serde_json::Value {
        let mut v = new_server(name);
        v["segredos"]["chave_privada"] = serde_json::json!("carregada");
        v
    }

    /// Phase 53a (S2, S11): "Load from file" reads the key in Rust; the response carries
    /// only the size; saving with `"carregada"` stores exactly the file in the vault (the
    /// daemon reveal returns the same); saving discards the loaded key.
    #[test]
    fn load_key_goes_to_the_daemon_without_passing_through_the_interface() {
        let (s, sel, _) = service_34();
        let d = dir_34("chave53a");
        let k = key_53a("ZmFsc28tY2hhdmUtc2VydmljbzUzYQ==");
        std::fs::write(d.join("deploy_ed25519"), &k).unwrap();
        *sel.0.lock().unwrap() = Some(d.join("deploy_ed25519"));
        let r = s.load_key("chave_privada").unwrap();
        assert_eq!(
            r,
            Carregamento::Carregada {
                tamanho: k.len() as u64
            }
        );
        let json = serde_json::to_string(&r).unwrap();
        assert!(
            !json.contains("ZmFsc28") && !json.contains("deploy"),
            "{json}"
        );
        let r = s
            .save_credential(None, form(loaded_server("clientex-ssh-53a")))
            .unwrap();
        assert_eq!(r.saved.as_deref(), Some("clientex-ssh-53a"), "{r:?}");
        let v = s.reveal_field("clientex-ssh-53a", "chave_privada").unwrap();
        assert_eq!(v.expor(), k.as_str());
        // Once saved, the key was discarded: "carregada" again is an invalid request.
        assert_eq!(
            s.save_credential(None, form(loaded_server("clientex-ssh-53b"))),
            Err(AppError::InvalidRequest)
        );
        let _ = std::fs::remove_dir_all(d);
    }

    /// Phase 53d (2nd audit of M10): the lock that comes from outside (inactivity, the
    /// CLI lock, screen locked) arrives through the tray and discards the loaded key; the
    /// periodic poll with the vault unlocked does not discard.
    #[test]
    fn attack_outside_lock_discards_the_loaded_key() {
        use crate::bandeja::EstadoBandeja::*;
        let (s, sel, _) = service_34();
        let d = dir_34("chave53d");
        std::fs::write(d.join("chave"), key_53a("ZmFsc28tNTNk")).unwrap();
        *sel.0.lock().unwrap() = Some(d.join("chave"));
        for after in [Travado, SemDaemon, SemCofre] {
            assert!(matches!(
                s.load_key("chave_privada").unwrap(),
                Carregamento::Carregada { .. }
            ));
            s.on_tray_transition(Destravado, after);
            assert_eq!(
                s.save_credential(None, form(loaded_server("clientex-ssh-53d"))),
                Err(AppError::InvalidRequest),
                "{after:?}"
            );
        }
        assert!(matches!(
            s.load_key("chave_privada").unwrap(),
            Carregamento::Carregada { .. }
        ));
        s.on_tray_transition(Destravado, Destravado);
        s.on_tray_transition(Travado, Destravado);
        let r = s
            .save_credential(None, form(loaded_server("clientex-ssh-53d")))
            .unwrap();
        assert_eq!(r.saved.as_deref(), Some("clientex-ssh-53d"), "{r:?}");
        let _ = std::fs::remove_dir_all(d);
    }

    /// Phase 53a: a field outside the list does not even open the picker; cancel, notice
    /// and [`Service::discard_form`] leave no key for the save; the notice does not carry
    /// the content nor the path; a notice does not erase the key loaded before.
    #[test]
    fn attack_load_key_forged_field_notice_and_discard() {
        let (s, sel, _) = service_34();
        let d = dir_34("chave53a-ataque");
        let text = d.join("segredo-qualquer.txt");
        std::fs::write(&text, "falso-texto-53a=valor\n").unwrap();
        *sel.0.lock().unwrap() = Some(text.clone());
        for field in ["senha", "passphrase", "totp_semente", "", "chave_privada "] {
            assert_eq!(
                s.load_key(field),
                Err(AppError::InvalidRequest),
                "{field:?}"
            );
        }
        let r = s.load_key("chave_privada").unwrap();
        assert_eq!(
            r,
            Carregamento::Aviso {
                motivo: chave::AVISO_NAO_E_CHAVE.into()
            }
        );
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("falso-texto") && !json.contains("segredo-qualquer"));
        assert_eq!(
            s.save_credential(None, form(loaded_server("clientex-ssh-53c"))),
            Err(AppError::InvalidRequest)
        );
        // Link to the key: refused (the picker returns the path of the link).
        let k = key_53a("ZmFsc28tbGluaw==");
        std::fs::write(d.join("chave"), &k).unwrap();
        std::os::unix::fs::symlink(d.join("chave"), d.join("link")).unwrap();
        *sel.0.lock().unwrap() = Some(d.join("link"));
        assert_eq!(
            s.load_key("chave_privada").unwrap(),
            Carregamento::Aviso {
                motivo: chave::AVISO_LINK.into()
            }
        );
        // Loaded and then a notice: the loaded one stays; discarding erases it.
        *sel.0.lock().unwrap() = Some(d.join("chave"));
        assert!(matches!(
            s.load_key("chave_privada").unwrap(),
            Carregamento::Carregada { .. }
        ));
        *sel.0.lock().unwrap() = Some(text);
        assert!(matches!(
            s.load_key("chave_privada").unwrap(),
            Carregamento::Aviso { .. }
        ));
        s.discard_form();
        assert_eq!(
            s.save_credential(None, form(loaded_server("clientex-ssh-53c"))),
            Err(AppError::InvalidRequest)
        );
        // Locking also discards.
        *sel.0.lock().unwrap() = Some(d.join("chave"));
        assert!(matches!(
            s.load_key("chave_privada").unwrap(),
            Carregamento::Carregada { .. }
        ));
        s.on_exit();
        assert_eq!(
            s.save_credential(None, form(loaded_server("clientex-ssh-53c"))),
            Err(AppError::InvalidRequest)
        );
        // Canceled: nothing.
        *sel.0.lock().unwrap() = None;
        assert_eq!(
            s.load_key("chave_privada").unwrap(),
            Carregamento::Cancelado
        );
        // "carregada" on a field that is not a key one: invalid request.
        let mut v = new_server("clientex-ssh-53d");
        v["segredos"]["passphrase"] = serde_json::json!("carregada");
        assert_eq!(
            s.save_credential(None, form(v)),
            Err(AppError::InvalidRequest)
        );
        let _ = std::fs::remove_dir_all(d);
    }

    /// V5.12 (D29): 300 KiB and a sum above 256 KiB give a notice and are not kept; empty
    /// is refused; a forged or repeated id is an invalid request; cancel keeps nothing;
    /// the reserved name (`CHAVE` on the server) gives the error on the attachments
    /// field.
    #[test]
    fn attack_large_empty_attachment_and_forged_id() {
        let (s, sel, _) = service_34();
        let d = dir_34("limites");
        let escolher = |name: &str, size: usize| {
            std::fs::write(d.join(name), vec![b'a'; size]).unwrap();
            *sel.0.lock().unwrap() = Some(d.join(name));
            s.choose_attachment(None).unwrap()
        };
        assert!(matches!(
            escolher("grande.bin", 300 * 1024),
            Escolha::Aviso { .. }
        ));
        assert!(matches!(escolher("vazio.txt", 0), Escolha::Aviso { .. }));
        let Escolha::Anexado { id, .. } = escolher("a.pem", 200 * 1024) else {
            panic!()
        };
        assert!(matches!(
            escolher("b.pem", 100 * 1024),
            Escolha::Aviso { .. }
        ));
        *sel.0.lock().unwrap() = None;
        assert_eq!(s.choose_attachment(None).unwrap(), Escolha::Cancelado);
        for ids in [serde_json::json!([id, id]), serde_json::json!([id + 99])] {
            let mut v = new_server("clientex-ssh-x");
            v["anexos"] = ids;
            assert_eq!(
                s.save_credential(None, form(v)),
                Err(AppError::InvalidRequest)
            );
        }
        s.remove_attachment(id).unwrap();
        assert_eq!(s.remove_attachment(id), Err(AppError::InvalidRequest));
        let Escolha::Anexado { id, .. } = escolher("chave", 10) else {
            panic!()
        };
        let mut v = new_server("clientex-ssh-x");
        v["anexos"] = serde_json::json!([id]);
        let r = s.save_credential(None, form(v)).unwrap();
        assert!(r.errors.iter().any(|e| e.campo == "anexos"), "{r:?}");
        // Canceling the form discards the chosen ones.
        s.discard_form();
        assert_eq!(s.remove_attachment(id), Err(AppError::InvalidRequest));
        let _ = std::fs::remove_dir_all(d);
    }

    /// Phase 39b (V5.12, S11): edit `gama-generico` keeping the `app.env`, removing it and
    /// putting another. The frame the app sends to the daemon carries only the name of
    /// the kept attachment, never the content; no response to the interface carries it.
    #[test]
    fn attack_edit_keeps_removes_and_adds_attachment_without_the_content_passing_through_the_app() {
        struct Spy(DaemonFalso, Arc<Mutex<Vec<Vec<u8>>>>);
        impl Daemon for Spy {
            fn trocar(&mut self, request: &[u8]) -> Result<Zeroizing<Vec<u8>>, AppError> {
                self.1.lock().unwrap().push(request.to_vec());
                let r = self.0.trocar(request)?;
                self.1.lock().unwrap().push(r.to_vec());
                Ok(r)
            }
        }
        let frames = Arc::new(Mutex::new(Vec::new()));
        let sel = FixedPicker::default();
        let s = Service::new(
            Spy(DaemonFalso::cheio(), Arc::clone(&frames)),
            Copiador::novo(Arc::new(Area::default())),
        )
        .with_picker(sel.clone());
        let content = "FALSO_ENV=falso-anexo-01";
        let edit = |kept: serde_json::Value| {
            let mut g = serde_json::json!({
                "tipo": "generico", "nome": "gama-generico", "grupo": "gama",
                "ambiente": "stage", "projeto": "", "tags": [], "campos": {}, "segredos": {},
                "extras": [
                    {"nome": "REGIAO", "segredo": false, "valor": {"novo": "r-02"}},
                    {"nome": "CHAVE_LICENCA", "segredo": true,
                     "valor": {"manter": "CHAVE_LICENCA"}}
                ]
            });
            g["anexos_mantidos"] = kept;
            g
        };
        // Keep: the attachment stays, with the same size.
        let before = s.credential_detail("gama-generico").unwrap().anexos;
        assert_eq!(before.len(), 1);
        let r = s
            .save_credential(
                Some("gama-generico"),
                form(edit(serde_json::json!(["app.env"]))),
            )
            .unwrap();
        assert_eq!(r.saved.as_deref(), Some("gama-generico"), "{r:?}");
        let det = s.credential_detail("gama-generico").unwrap();
        assert_eq!(det.anexos.len(), 1);
        assert_eq!(det.anexos[0].nome, "app.env");
        assert_eq!(det.anexos[0].tamanho, before[0].tamanho);
        assert_eq!(
            det.extras
                .iter()
                .find(|x| x.nome == "REGIAO")
                .unwrap()
                .valor
                .as_deref(),
            Some("r-02")
        );
        // Add another, counting the kept one in the limits.
        let d = dir_34("edicao");
        std::fs::write(d.join(".env"), "OUTRO=falso-outro-39b\n").unwrap();
        *sel.0.lock().unwrap() = Some(d.join(".env"));
        let ed = EdicaoDosAnexos {
            entrada: "gama-generico".into(),
            mantidos: vec!["app.env".into()],
        };
        let Escolha::Anexado { id, nome: name, .. } = s.choose_attachment(Some(&ed)).unwrap()
        else {
            panic!()
        };
        assert_eq!(name, "env");
        let mut g = edit(serde_json::json!(["app.env"]));
        g["anexos"] = serde_json::json!([id]);
        let r = s.save_credential(Some("gama-generico"), form(g)).unwrap();
        assert_eq!(r.saved.as_deref(), Some("gama-generico"), "{r:?}");
        let names: Vec<String> = s
            .credential_detail("gama-generico")
            .unwrap()
            .anexos
            .into_iter()
            .map(|a| a.nome)
            .collect();
        assert_eq!(names, ["app.env", "env"]);
        // Remove: what is not in the kept attachments leaves.
        let r = s
            .save_credential(
                Some("gama-generico"),
                form(edit(serde_json::json!(["env"]))),
            )
            .unwrap();
        assert_eq!(r.saved.as_deref(), Some("gama-generico"), "{r:?}");
        let det = s.credential_detail("gama-generico").unwrap();
        assert_eq!(det.anexos.len(), 1);
        assert_eq!(det.anexos[0].nome, "env");
        // An unknown kept name in [`Service::choose_attachment`] is an invalid request.
        let forged = EdicaoDosAnexos {
            entrada: "gama-generico".into(),
            mantidos: vec!["app.env".into()],
        };
        assert_eq!(
            s.choose_attachment(Some(&forged)),
            Err(AppError::InvalidRequest)
        );
        // No frame (request or response) carried the content of `app.env`, in clear or
        // in base64.
        // base64 of "FALSO_ENV=falso-anexo-01" (24 bytes: prefix of the base64 of any
        // content that starts with it).
        let b64 = "RkFMU09fRU5WPWZhbHNvLWFuZXhvLTAx";
        for q in frames.lock().unwrap().iter() {
            let t = String::from_utf8_lossy(q);
            assert!(!t.contains(content) && !t.contains(b64), "{t}");
            assert!(!t.contains("falso-anexo-01"), "{t}");
        }
        let _ = std::fs::remove_dir_all(d);
    }

    /// V5.24 on the app side (D3): "Connect" only for a server entry; the token goes to the
    /// terminal and never to the response; a malformed name does not even reach the
    /// daemon. (Reuse, deadline and swapped entry are the daemon's and the CLI's, Phase
    /// 27.)
    #[test]
    fn attack_connect_only_server_and_no_token_in_the_response() {
        let (s, _, term) = service_34();
        s.connect("acme-ssh-salto").unwrap();
        let opened = term.0.lock().unwrap().clone();
        assert_eq!(opened.len(), 1);
        assert_eq!(opened[0].0, "acme-ssh-salto");
        assert_eq!(opened[0].1, "Token(••••):32");
        assert_eq!(s.connect("acme-pg-aprovacao"), Err(AppError::NotAServer));
        assert_eq!(s.connect("nao-existe"), Err(AppError::EntryNotFound));
        for name in ["-oProxyCommand=x", "", "acme ssh", "Acme"] {
            assert_eq!(s.connect(name), Err(AppError::InvalidRequest), "{name}");
        }
        assert_eq!(term.0.lock().unwrap().len(), 1);
        // Without a terminal set up, the fixed error.
        let (bare, _) = service();
        assert_eq!(
            bare.connect("acme-ssh-salto"),
            Err(AppError::TerminalUnavailable)
        );
        assert_eq!(
            bare.choose_attachment(None),
            Err(AppError::PickerUnavailable)
        );
    }
    /// Phase 39 (V5.23, S12): "Lock now" clears the clipboard that still has the copied
    /// value and the form, and the vault stays locked: afterwards, reveal and copy answer
    /// "locked".
    #[test]
    fn attack_lock_clears_the_clipboard_and_locks() {
        let (s, area) = service();
        s.copy_field("acme-pg-dev", "senha").unwrap();
        assert!(area.0.lock().unwrap().is_some());
        assert_eq!(s.tray_state(), crate::bandeja::EstadoBandeja::Destravado);
        s.lock().unwrap();
        assert!(area.0.lock().unwrap().is_none(), "área não foi limpa");
        assert_eq!(s.vault_status(), Ok(EstadoCofre::Travado));
        assert_eq!(s.tray_state(), crate::bandeja::EstadoBandeja::Travado);
        assert_eq!(
            s.reveal_field("acme-pg-dev", "senha").map(|_| ()),
            Err(AppError::Locked)
        );
        assert!(s.copy_field("acme-pg-dev", "senha").is_err());
        // Locking again does not fail.
        s.lock().unwrap();
    }

    /// Daemon that records the operation of each request (for the tray poll test).
    struct Recorder(DaemonFalso, Arc<Mutex<Vec<String>>>);

    impl Daemon for Recorder {
        fn trocar(&mut self, request: &[u8]) -> Result<Zeroizing<Vec<u8>>, AppError> {
            let v: serde_json::Value = serde_json::from_slice(request).unwrap();
            self.1
                .lock()
                .unwrap()
                .push(v["op"].as_str().unwrap_or("?").to_owned());
            self.0.trocar(request)
        }
    }

    /// Attack (Phase 39): the periodic tray poll must not hold the vault open. It only
    /// sends the status request, which `wardd` does not count as use (the use rule of
    /// the `wardd` lock); no other operation leaves it.
    #[test]
    fn attack_tray_poll_only_asks_status() {
        let ops = Arc::new(Mutex::new(Vec::new()));
        let s = Service::new(
            Recorder(DaemonFalso::cheio(), Arc::clone(&ops)),
            Copiador::novo(Arc::new(Area::default())),
        );
        for _ in 0..5 {
            s.tray_state();
        }
        assert_eq!(*ops.lock().unwrap(), vec!["estado"; 5]);
    }

    /// Without a daemon, locking still clears what is local and answers the error.
    #[test]
    fn lock_without_daemon_clears_and_answers_error() {
        struct Down;
        impl Daemon for Down {
            fn trocar(&mut self, _: &[u8]) -> Result<Zeroizing<Vec<u8>>, AppError> {
                Err(AppError::DaemonUnavailable)
            }
        }
        let area = Arc::new(Area::default());
        let s = Service::new(Down, Copiador::novo(Arc::clone(&area)));
        *area.0.lock().unwrap() = Some(b"qualquer".to_vec());
        assert!(s.lock().is_err());
        assert_eq!(s.tray_state(), crate::bandeja::EstadoBandeja::SemDaemon);
    }
}

/// Phase 31: unlock with the master password and the agent's unlock request.
#[cfg(test)]
mod unlock_tests {
    use super::*;
    use crate::daemon::{Aviso, FilaAvisos};
    use crate::daemon_falso::DaemonFalso;
    use std::time::Duration;
    use zeroize::Zeroizing;

    const RIGHT_PASSWORD: &str = "falso-mestra-ward-01";

    struct NoClipboard;
    impl AreaTransferencia for NoClipboard {
        fn escrever(&self, _: &[u8]) -> Result<(), AppError> {
            Err(AppError::Clipboard)
        }
        fn ler(&self) -> Result<Option<Zeroizing<Vec<u8>>>, AppError> {
            Ok(None)
        }
        fn limpar(&self) -> Result<(), AppError> {
            Ok(())
        }
    }

    fn service() -> Service<DaemonFalso, NoClipboard> {
        Service::new(
            DaemonFalso::travado().sem_espera(),
            Copiador::novo(NoClipboard),
        )
    }

    fn password(s: &str) -> SenhaMestra {
        SenhaMestra::nova(s.to_owned()).unwrap()
    }

    /// Keeps the frames the app sends and answers through the fake daemon.
    struct Spy(DaemonFalso, Arc<Mutex<Vec<Zeroizing<Vec<u8>>>>>);
    impl Daemon for Spy {
        fn trocar(&mut self, request: &[u8]) -> Result<Zeroizing<Vec<u8>>, AppError> {
            self.1
                .lock()
                .unwrap()
                .push(Zeroizing::new(request.to_vec()));
            self.0.trocar(request)
        }
    }

    #[test]
    fn wrong_password_does_not_unlock_and_the_right_one_unlocks() {
        let s = service();
        assert_eq!(s.vault_status(), Ok(EstadoCofre::Travado));
        assert_eq!(s.list_credentials().unwrap_err(), AppError::Locked);
        for wrong in [
            "falso-mestra-ward-02",
            "135790",
            "FALSO-MESTRA-WARD-01",
            " ",
        ] {
            assert_eq!(s.unlock(password(wrong)), Err(AppError::WrongPassword));
            assert_eq!(s.vault_status(), Ok(EstadoCofre::Travado));
        }
        s.unlock(password(RIGHT_PASSWORD)).unwrap();
        assert_eq!(s.vault_status(), Ok(EstadoCofre::Destravado));
        assert!(!s.list_credentials().unwrap().is_empty());
        // Already unlocked: `ok` again.
        s.unlock(password("qualquer")).unwrap();
    }

    #[test]
    fn attack_unlock_error_without_password_or_count() {
        // S13/D20: the error message is fixed, without the password and without how many
        // attempts there were (the 1st and the 5th say the same thing).
        let s = service();
        let first = s.unlock(password("falso-mestra-ward-99")).unwrap_err();
        for _ in 0..4 {
            assert_eq!(
                s.unlock(password("falso-mestra-ward-99")).unwrap_err(),
                first
            );
        }
        let text = format!("{first} {first:?} {}", first.code());
        assert!(!text.contains("falso"), "{text}");
        assert!(!text.chars().any(|c| c.is_ascii_digit()), "{text}");
    }

    #[test]
    fn attack_the_password_only_goes_in_the_unlock_request() {
        // The password leaves the app only in the unlock frame; no other frame carries
        // it.
        let frames = Arc::new(Mutex::new(Vec::new()));
        let s = Service::new(
            Spy(DaemonFalso::travado().sem_espera(), frames.clone()),
            Copiador::novo(NoClipboard),
        );
        let _ = s.vault_status();
        s.unlock(password(RIGHT_PASSWORD)).unwrap();
        let _ = s.list_credentials();
        let q = frames.lock().unwrap();
        assert!(q.len() >= 3);
        let texts: Vec<String> = q
            .iter()
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .collect();
        assert_eq!(
            texts.iter().filter(|t| t.contains("\"destravar\"")).count(),
            1
        );
        for t in texts.iter().filter(|t| !t.contains("\"destravar\"")) {
            assert!(!t.contains("falso-mestra"), "{t}");
            // base64 of "falso-mestra-ward-01" (as the protocol carries the password).
            assert!(!t.contains("ZmFsc28tbWVzdHJhLXdhcmQtMDE"), "{t}");
        }
    }

    #[test]
    fn agent_request_shows_only_locked_and_goes_away_on_unlock() {
        let s = service();
        let queue = Arc::new(FilaAvisos::default());
        let (tx, rx) = std::sync::mpsc::channel();
        crate::destrave::vigiar(queue.clone(), s.unlock_request(), move || {
            let _ = tx.send(());
        })
        .unwrap();
        assert_eq!(s.request_info(EstadoCofre::Travado), None);
        queue.simular(Aviso::destrave_de_teste("acme-pg-aprovacao"));
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let info = s.request_info(EstadoCofre::Travado).unwrap();
        assert!(info.restante > 110 && info.restante <= 120, "{info:?}");
        assert_eq!(s.request_info(EstadoCofre::Destravado), None);
        assert_eq!(s.request_info(EstadoCofre::SemCofre), None);
        // A wrong password does not remove the request.
        let _ = s.unlock(password("errada"));
        assert!(s.request_info(EstadoCofre::Travado).is_some());
        s.unlock(password(RIGHT_PASSWORD)).unwrap();
        assert_eq!(s.request_info(EstadoCofre::Travado), None);
        // The JSON that goes to the screen: id, remaining and the request text (Phase
        // 39a), nothing else.
        queue.simular(Aviso::destrave_de_teste("acme-pg-aprovacao"));
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let j = serde_json::to_value(s.request_info(EstadoCofre::Travado).unwrap()).unwrap();
        let mut keys: Vec<&String> = j.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(keys, ["comando", "entrada", "id", "operacao", "restante"]);
    }

    #[test]
    fn fake_daemon_waits_d20_before_looking_at_the_password() {
        // The 2nd attempt (after 1 error) waits 1 s even with the right password.
        let s = Service::new(DaemonFalso::travado(), Copiador::novo(NoClipboard));
        assert_eq!(s.unlock(password("errada")), Err(AppError::WrongPassword));
        let t = std::time::Instant::now();
        s.unlock(password(RIGHT_PASSWORD)).unwrap();
        assert!(
            t.elapsed() >= Duration::from_millis(990),
            "{:?}",
            t.elapsed()
        );
    }
}
