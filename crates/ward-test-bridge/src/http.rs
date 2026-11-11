//! Minimal HTTP server of the test bridge (standard library only). See the top of the crate.

use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use ward_app_core::AppError;
use ward_app_core::anexos::EdicaoDosAnexos;
use ward_app_core::area_transferencia::AreaTransferencia;
use ward_app_core::comandos::Comando;
use ward_app_core::configuracoes::{TrocaPin, TrocaSenha};
use ward_app_core::daemon::Daemon;
use ward_app_core::destrave::SenhaMestra;
use ward_app_core::formulario::Formulario;
use ward_app_core::gerador::Opcoes;
use ward_app_core::nivel::Pin;
use ward_app_core::primeiro_uso::NovoCofre;
use ward_app_core::restaurar::Restauracao;
use ward_app_core::service::Service;
use zeroize::Zeroizing;

/// Request head (line + headers).
pub const MAX_HEADER: usize = 8 * 1024;
/// Body of a `POST` to a bridge command.
pub const MAX_BODY: usize = 16 * 1024;
/// Connections open at the same time.
pub const MAX_CONNECTIONS: usize = 16;
const TIMEOUT: Duration = Duration::from_secs(5);

/// Same CSP as the app (`tauri.conf.json`), with `connect-src 'self'` in place of the IPC.
pub const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; \
font-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; \
frame-src 'none'; worker-src 'none'; object-src 'none'; media-src 'none'; manifest-src 'none'";

/// Bridge bound to an app service and to the directory of the test-mode build.
pub struct Bridge<D: Daemon, A: AreaTransferencia> {
    service: Service<D, A>,
    dist: PathBuf,
    token: Zeroizing<String>,
    host: String,
    origin: String,
    open_conns: AtomicUsize,
}

impl<D: Daemon + 'static, A: AreaTransferencia> Bridge<D, A> {
    /// Opens `127.0.0.1:0` and draws the token.
    pub fn open(service: Service<D, A>, dist: &Path) -> io::Result<(Arc<Self>, TcpListener)> {
        let dist = fs::canonicalize(dist)?;
        if !dist.join("index.html").is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "dist sem index.html",
            ));
        }
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
        let port = listener.local_addr()?.port();
        let mut raw = Zeroizing::new([0u8; 32]);
        getrandom::fill(&mut *raw).map_err(|_| io::Error::other("sem aleatório do sistema"))?;
        let mut token = Zeroizing::new(String::with_capacity(64));
        for b in raw.iter() {
            token.push(char::from(b"0123456789abcdef"[usize::from(b >> 4)]));
            token.push(char::from(b"0123456789abcdef"[usize::from(b & 0xf)]));
        }
        let host = format!("127.0.0.1:{port}");
        let origin = format!("http://{host}");
        let bridge = Arc::new(Bridge {
            service,
            dist,
            token,
            host,
            origin,
            open_conns: AtomicUsize::new(0),
        });
        Ok((bridge, listener))
    }

    /// URL that Chrome opens (with the token in the fragment).
    pub fn opening_url(&self) -> String {
        format!("{}/#token={}", self.origin, *self.token)
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// Only for the bridge tests.
    #[doc(hidden)]
    pub fn token_for_test(&self) -> &str {
        &self.token
    }

    /// Serves until the process ends.
    pub fn serve(self: &Arc<Self>, listener: TcpListener) {
        for conn in listener.incoming() {
            let Ok(conn) = conn else { continue };
            if self.open_conns.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                self.open_conns.fetch_sub(1, Ordering::SeqCst);
                continue; // drops the connection
            }
            let bridge = Arc::clone(self);
            let r = thread::Builder::new()
                .name("ward-ponte".into())
                .spawn(move || {
                    let _ = bridge.handle(conn);
                    bridge.open_conns.fetch_sub(1, Ordering::SeqCst);
                });
            if r.is_err() {
                self.open_conns.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }

    fn handle(&self, conn: TcpStream) -> io::Result<()> {
        conn.set_read_timeout(Some(TIMEOUT))?;
        conn.set_write_timeout(Some(TIMEOUT))?;
        let mut output = conn.try_clone()?;
        let response = match read_request(conn) {
            Ok(p) => self.respond(&p),
            Err(status) => Response::text(status),
        };
        response.write_to(&mut output)
    }

    /// Decides the response to an already read request.
    pub fn respond(&self, p: &Request) -> Response {
        if p.header("host") != Some(self.host.as_str()) {
            return Response::text(421);
        }
        match (p.method.as_str(), p.path.as_str()) {
            ("GET", path) => self.static_file(path),
            ("POST", path) if path.starts_with("/ponte/") => {
                self.command(p, &path["/ponte/".len()..])
            }
            _ => Response::text(405),
        }
    }

    fn static_file(&self, path: &str) -> Response {
        let file = match path {
            "/" | "/index.html" => self.dist.join("index.html"),
            c => match c.strip_prefix("/assets/") {
                Some(name) if valid_asset_name(name) => self.dist.join("assets").join(name),
                _ => return Response::text(404),
            },
        };
        // Without following a link out of the dist.
        let Ok(real) = fs::canonicalize(&file) else {
            return Response::text(404);
        };
        if !real.starts_with(&self.dist) || !real.is_file() {
            return Response::text(404);
        }
        match fs::read(&real) {
            Ok(body) => Response {
                status: 200,
                content_type: content_type_of(&real),
                body,
            },
            Err(_) => Response::text(404),
        }
    }

    fn command(&self, p: &Request, name: &str) -> Response {
        if p.header("origin") != Some(self.origin.as_str()) {
            return Response::text(403);
        }
        let authorized = p
            .header("authorization")
            .and_then(|v| v.strip_prefix("Bearer "))
            .is_some_and(|t| ct_equal(t.as_bytes(), self.token.as_bytes()));
        if !authorized {
            return Response::text(401);
        }
        if !p
            .header("content-type")
            .is_some_and(|t| t == "application/json" || t.starts_with("application/json;"))
        {
            return Response::text(415);
        }
        let Some(command) = Comando::de_nome(name) else {
            return Response::error(404, AppError::InvalidRequest);
        };
        let Ok(Value::Object(mut args)) = serde_json::from_slice::<Value>(&p.body) else {
            return Response::error(400, AppError::InvalidRequest);
        };
        // Phase 35: the PIN leaves the map (ownership, no copy) and becomes a `Pin` that zeroes.
        let pin = match command {
            Comando::MudarNivel => args.remove("pin").map(serde_json::from_value::<Pin>),
            // Phase 36: `null` (Deny) stays `None`; the rest becomes `Pin` or an error.
            Comando::ResponderAprovacao => match args.remove("pin") {
                None | Some(Value::Null) => None,
                Some(v) => Some(serde_json::from_value::<Pin>(v)),
            },
            _ => None,
        };
        // Phase 31: the master password also leaves the map (ownership) and becomes the
        // master password type of the app core, which zeroes.
        let password = match command {
            Comando::Destravar => args
                .remove("senha")
                .map(serde_json::from_value::<SenhaMestra>),
            _ => None,
        };
        // Phase 38: the data of the changes also leave the map (ownership) and become typed
        // values that zero.
        let pin_change = match command {
            Comando::TrocarPin => args.remove("dados").map(serde_json::from_value::<TrocaPin>),
            _ => None,
        };
        let password_change = match command {
            Comando::TrocarSenhaMestra => args
                .remove("dados")
                .map(serde_json::from_value::<TrocaSenha>),
            _ => None,
        };
        // Phase 47: the version password and the current PIN also leave the map (ownership).
        let restore = match command {
            Comando::RestaurarBackup => args
                .remove("dados")
                .map(serde_json::from_value::<Restauracao>),
            _ => None,
        };
        let text = |key: &str| args.get(key).and_then(Value::as_str);
        let flag = |key: &str| args.get(key).and_then(Value::as_bool);
        let result = match command {
            // Phase 31: with the vault locked, the request of the waiting agent (only id and the
            // seconds left).
            Comando::EstadoDoCofre => self
                .service
                .vault_status()
                .map(|e| json!({ "estado": e.codigo(), "pedido": self.service.request_info(e) })),
            Comando::CopiarCampo => match (text("entrada"), text("campo")) {
                (Some(e), Some(c)) => self.service.copy_field(e, c).map(|()| Value::Null),
                _ => Err(AppError::InvalidRequest),
            },
            Comando::ListarCredenciais => self
                .service
                .list_credentials()
                .and_then(|l| serde_json::to_value(l).map_err(|_| AppError::InvalidResponse)),
            Comando::DetalheCredencial => match text("entrada") {
                Some(e) => self
                    .service
                    .credential_detail(e)
                    .and_then(|d| serde_json::to_value(d).map_err(|_| AppError::InvalidResponse)),
                None => Err(AppError::InvalidRequest),
            },
            Comando::GerarSenha => match (
                args.get("tamanho").and_then(Value::as_u64),
                flag("minusculas"),
                flag("maiusculas"),
                flag("digitos"),
                flag("simbolos"),
            ) {
                (Some(length), Some(lowercase), Some(uppercase), Some(digits), Some(symbols)) => {
                    self.service
                        .generate_password(Opcoes {
                            tamanho: length,
                            minusculas: lowercase,
                            maiusculas: uppercase,
                            digitos: digits,
                            simbolos: symbols,
                        })
                        .map(|g| {
                            json!({
                                "tamanho": g.tamanho,
                                "bits": g.bits,
                                "forca": g.forca.codigo(),
                                "simbolos": g.simbolos,
                            })
                        })
                }
                _ => Err(AppError::InvalidRequest),
            },
            Comando::CopiarSenhaGerada => {
                self.service.copy_generated_password().map(|()| Value::Null)
            }
            // S11: the value only comes out on "reveal", as in the app.
            Comando::RevelarSenhaGerada => self
                .service
                .reveal_generated_password()
                .map(|r| json!({ "valor": r.expor() })),
            Comando::DescartarSenhaGerada => self
                .service
                .discard_generated_password()
                .map(|()| Value::Null),
            // S11 (D65 b): the value only on "reveal", as in the app.
            Comando::RevelarCampo => match (text("entrada"), text("campo")) {
                (Some(e), Some(c)) => self
                    .service
                    .reveal_field(e, c)
                    .map(|r| json!({ "valor": r.expor() })),
                _ => Err(AppError::InvalidRequest),
            },
            Comando::CodigoTotp => match text("entrada") {
                Some(e) => self
                    .service
                    .totp_code(e)
                    .and_then(|c| serde_json::to_value(c).map_err(|_| AppError::InvalidResponse)),
                None => Err(AppError::InvalidRequest),
            },
            Comando::CopiarCodigoTotp => match text("entrada") {
                Some(e) => self.service.copy_totp_code(e).map(|()| Value::Null),
                None => Err(AppError::InvalidRequest),
            },
            Comando::UsarSenhaGerada => match text("campo") {
                Some(c) => self
                    .service
                    .use_generated_password(c)
                    .map(|t| json!({ "tamanho": t })),
                None => Err(AppError::InvalidRequest),
            },
            Comando::DescartarFormulario => {
                self.service.discard_form();
                Ok(Value::Null)
            }
            Comando::SalvarCredencial => {
                let previous = match args.get("anterior") {
                    None | Some(Value::Null) => Ok(None),
                    Some(Value::String(a)) => Ok(Some(a.as_str())),
                    Some(_) => Err(AppError::InvalidRequest),
                };
                let form = args
                    .get("formulario")
                    .cloned()
                    .map(serde_json::from_value::<Formulario>)
                    .and_then(Result::ok)
                    .ok_or(AppError::InvalidRequest);
                match (previous, form) {
                    (Ok(a), Ok(f)) => self.service.save_credential(a, f).and_then(|r| {
                        serde_json::to_value(r).map_err(|_| AppError::InvalidResponse)
                    }),
                    _ => Err(AppError::InvalidRequest),
                }
            }
            Comando::MudarNivel => match (text("entrada"), text("nivel"), pin) {
                (Some(e), Some(n), Some(Ok(pin))) => self
                    .service
                    .set_level(e, n, pin)
                    .and_then(|r| serde_json::to_value(r).map_err(|_| AppError::InvalidResponse)),
                _ => Err(AppError::InvalidRequest),
            },
            // Phase 34: the file is picked and read on the Rust side; returns id, name and
            // size, or the notice (S11, V5.12).
            // Phase 39b: when editing, the edit object with the entry and the kept ids (the
            // stored attachments that remain count in the limits).
            Comando::EscolherAnexo => {
                let edit = match args.get("edicao") {
                    None | Some(Value::Null) => Ok(None),
                    Some(v) => serde_json::from_value::<EdicaoDosAnexos>(v.clone())
                        .map(Some)
                        .map_err(|_| AppError::InvalidRequest),
                };
                edit.and_then(|ed| self.service.choose_attachment(ed.as_ref()))
                    .and_then(|e| serde_json::to_value(e).map_err(|_| AppError::InvalidResponse))
            }
            Comando::RemoverAnexo => match args.get("id").and_then(Value::as_u64) {
                Some(id) => self.service.remove_attachment(id).map(|()| Value::Null),
                None => Err(AppError::InvalidRequest),
            },
            // Phase 53a: "load from file" of the private key; the file is picked,
            // read and checked on the Rust side; returns only the size, or the notice (S2, S11).
            Comando::CarregarChave => match text("campo") {
                Some(c) if args.len() == 1 => self
                    .service
                    .load_key(c)
                    .and_then(|r| serde_json::to_value(r).map_err(|_| AppError::InvalidResponse)),
                _ => Err(AppError::InvalidRequest),
            },
            // D3: the token does not go back to the interface.
            Comando::Conectar => match text("entrada") {
                Some(e) => self.service.connect(e).map(|()| Value::Null),
                None => Err(AppError::InvalidRequest),
            },
            // Phase 31: the response carries neither the password nor how many attempts there were.
            Comando::Destravar => match password {
                Some(Ok(password)) if args.is_empty() => {
                    self.service.unlock(password).map(|()| Value::Null)
                }
                _ => Err(AppError::InvalidRequest),
            },
            // Phase 39: "lock now"; clears the clipboard and the form first.
            Comando::Travar if args.is_empty() => self.service.lock().map(|()| Value::Null),
            Comando::Travar => Err(AppError::InvalidRequest),
            // The Tauri channel does not exist in the bridge: the test screen polls the status
            // (`test-bridge.js` › `listenByPolling`).
            Comando::OuvirAvisos => Err(AppError::InvalidRequest),
            // Phase 36: the approval window. The bridge has no separate windows (the per-window
            // ACL is Tauri's); the service checks id, decision and PIN the same way.
            Comando::PedidoDeAprovacao => self
                .service
                .approval_request()
                .and_then(|r| serde_json::to_value(r).map_err(|_| AppError::InvalidResponse)),
            // Phase 54b: the acknowledgement field (the stdin checkbox) only as
            // `true`/`false`.
            Comando::ResponderAprovacao => match (
                args.get("id").and_then(Value::as_u64),
                text("decisao"),
                pin,
                args.get("ciente").map(Value::as_bool),
            ) {
                (Some(id), Some(d), None, None | Some(Some(false))) => {
                    self.service.answer_approval(id, d, None, false)
                }
                (Some(id), Some(d), Some(Ok(p)), c @ (None | Some(Some(_)))) => self
                    .service
                    .answer_approval(id, d, Some(p), c == Some(Some(true))),
                _ => Err(AppError::InvalidRequest),
            }
            .and_then(|r| serde_json::to_value(r).map_err(|_| AppError::InvalidResponse)),
            // Phase 30: master password and PIN checked on the Rust side; the response carries
            // no value. The data field leaves the object through `remove`, without copying
            // the text.
            Comando::CriarCofre => match args
                .remove("dados")
                .map(serde_json::from_value::<NovoCofre>)
            {
                Some(Ok(n)) => self
                    .service
                    .create_vault(n)
                    .and_then(|r| serde_json::to_value(r).map_err(|_| AppError::InvalidResponse)),
                _ => Err(AppError::InvalidRequest),
            },
            // Phase 38 (V5.22): no response carries a PIN, a password or a hash.
            Comando::Configuracoes if args.is_empty() => self
                .service
                .settings()
                .and_then(|c| serde_json::to_value(c).map_err(|_| AppError::InvalidResponse)),
            Comando::Configuracoes => Err(AppError::InvalidRequest),
            Comando::TrocarPin => match pin_change {
                Some(Ok(t)) if args.is_empty() => self
                    .service
                    .change_pin(t)
                    .and_then(|r| serde_json::to_value(r).map_err(|_| AppError::InvalidResponse)),
                _ => Err(AppError::InvalidRequest),
            },
            Comando::TrocarSenhaMestra => match password_change {
                Some(Ok(t)) if args.is_empty() => self
                    .service
                    .change_master_password(t)
                    .and_then(|r| serde_json::to_value(r).map_err(|_| AppError::InvalidResponse)),
                _ => Err(AppError::InvalidRequest),
            },
            Comando::DefinirTempoTrava => match args.get("minutos").and_then(Value::as_u64) {
                Some(m) if args.len() == 1 => self
                    .service
                    .set_lock_timeout(m)
                    .and_then(|c| serde_json::to_value(c).map_err(|_| AppError::InvalidResponse)),
                _ => Err(AppError::InvalidRequest),
            },
            // Phase 48 (V7.9): no mount path; the UUID is checked in the service
            // (format and the daemon's list).
            Comando::BackupExterno if args.is_empty() => self
                .service
                .external_backup()
                .and_then(|b| serde_json::to_value(b).map_err(|_| AppError::InvalidResponse)),
            Comando::ListarPendrives if args.is_empty() => self
                .service
                .list_usb_drives()
                .and_then(|l| serde_json::to_value(l).map_err(|_| AppError::InvalidResponse)),
            Comando::BackupExterno | Comando::ListarPendrives => Err(AppError::InvalidRequest),
            // Phase 47 (V7.7): only name and instant of the versions; the restore response only
            // says restored and how many entries. The version is checked in the service
            // (format and the daemon's list).
            Comando::ListarBackups if args.is_empty() => self
                .service
                .list_backups()
                .and_then(|l| serde_json::to_value(l).map_err(|_| AppError::InvalidResponse)),
            Comando::ListarBackups => Err(AppError::InvalidRequest),
            Comando::RestaurarBackup => match restore {
                Some(Ok(r)) if args.is_empty() => self
                    .service
                    .restore_backup(r)
                    .and_then(|r| serde_json::to_value(r).map_err(|_| AppError::InvalidResponse)),
                _ => Err(AppError::InvalidRequest),
            },
            Comando::DefinirBackupExterno => {
                let uuid = match args.get("uuid") {
                    None | Some(Value::Null) => Ok(None),
                    Some(Value::String(u)) => Ok(Some(u.as_str())),
                    Some(_) => Err(AppError::InvalidRequest),
                };
                let only_known = args.keys().all(|k| k == "ligado" || k == "uuid");
                match (flag("ligado"), uuid) {
                    (Some(l), Ok(u)) if only_known => {
                        self.service.set_external_backup(l, u).and_then(|b| {
                            serde_json::to_value(b).map_err(|_| AppError::InvalidResponse)
                        })
                    }
                    _ => Err(AppError::InvalidRequest),
                }
            }
        };
        match result {
            Ok(value) => Response::json(200, &json!({ "ok": true, "valor": value })),
            Err(e) => Response::error(200, e),
        }
    }
}

fn ct_equal(a: &[u8], b: &[u8]) -> bool {
    use ctutils::CtEq;
    a.ct_eq(b).to_bool()
}

fn valid_asset_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

fn content_type_of(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    }
}

/// HTTP request already read and within the limits.
pub struct Request {
    pub method: String,
    pub path: String,
    headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    /// Builds a request (tests).
    pub fn new(method: &str, path: &str, headers: &[(&str, &str)], body: &[u8]) -> Request {
        Request {
            method: method.into(),
            path: path.into(),
            headers: headers
                .iter()
                .map(|(n, v)| (n.to_ascii_lowercase(), (*v).into()))
                .collect(),
            body: body.to_vec(),
        }
    }

    /// Header value; a repeated one counts as absent (there is no way to know which one holds).
    pub fn header(&self, name: &str) -> Option<&str> {
        let mut found = self.headers.iter().filter(|(n, _)| n == name);
        let first = found.next()?;
        if found.next().is_some() {
            None
        } else {
            Some(first.1.as_str())
        }
    }
}

/// Reads line, headers and body within the limits; the error is the HTTP status.
pub fn read_request<R: Read>(input: R) -> Result<Request, u16> {
    let mut reader = BufReader::new(input);
    let mut used = 0usize;
    // One line, without going past what is left of the head limit.
    let mut read_line = |reader: &mut BufReader<R>| -> Result<String, u16> {
        let mut raw = Vec::new();
        let remaining = (MAX_HEADER - used) as u64;
        let n = (&mut *reader)
            .take(remaining)
            .read_until(b'\n', &mut raw)
            .map_err(|_| 400u16)?;
        used += n;
        if n == 0 || !raw.ends_with(b"\n") {
            return Err(431); // head limit or connection closed midway
        }
        let text = String::from_utf8(raw).map_err(|_| 400u16)?;
        Ok(text.trim_end_matches(['\r', '\n']).to_owned())
    };
    let line = read_line(&mut reader)?;
    let mut parts = line.split(' ');
    let (Some(method), Some(path), Some("HTTP/1.1"), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(400);
    };
    if !path.starts_with('/') || path.contains(['?', '#', '%', '\\']) || path.contains("..") {
        return Err(400);
    }
    let mut headers = Vec::new();
    loop {
        let l = read_line(&mut reader)?;
        if l.is_empty() {
            break;
        }
        let (n, v) = l.split_once(':').ok_or(400u16)?;
        if n.is_empty() || !n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return Err(400);
        }
        headers.push((n.to_ascii_lowercase(), v.trim().to_owned()));
    }
    let mut request = Request {
        method: method.into(),
        path: path.into(),
        headers,
        body: Vec::new(),
    };
    if request
        .headers
        .iter()
        .any(|(n, _)| n == "transfer-encoding")
    {
        return Err(501);
    }
    let length = match request.header("content-length") {
        None if request.headers.iter().any(|(n, _)| n == "content-length") => return Err(400),
        None => 0,
        Some(v) if !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()) => {
            v.parse::<usize>().map_err(|_| 413u16)?
        }
        Some(_) => return Err(400),
    };
    if length > MAX_BODY {
        return Err(413);
    }
    let mut body = Vec::with_capacity(length);
    reader
        .take(length as u64)
        .read_to_end(&mut body)
        .map_err(|_| 400u16)?;
    if body.len() != length {
        return Err(400);
    }
    request.body = body;
    Ok(request)
}

/// Response ready to be written.
pub struct Response {
    pub status: u16,
    content_type: &'static str,
    pub body: Vec<u8>,
}

impl Response {
    fn text(status: u16) -> Response {
        Response {
            status,
            content_type: "text/plain; charset=utf-8",
            body: reason(status).as_bytes().to_vec(),
        }
    }

    fn json(status: u16, v: &Value) -> Response {
        Response {
            status,
            content_type: "application/json",
            body: v.to_string().into_bytes(),
        }
    }

    fn error(status: u16, e: AppError) -> Response {
        Response::json(
            status,
            &json!({ "ok": false, "codigo": e.code(), "mensagem": e.to_string() }),
        )
    }

    pub fn body_json(&self) -> Option<Value> {
        serde_json::from_slice(&self.body).ok()
    }

    fn write_to<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let header = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nContent-Security-Policy: {CSP}\r\n\
X-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nCache-Control: no-store\r\n\
Cross-Origin-Opener-Policy: same-origin\r\nCross-Origin-Resource-Policy: same-origin\r\n\
X-Frame-Options: DENY\r\nConnection: close\r\n\r\n",
            self.status,
            reason(self.status),
            self.content_type,
            self.body.len()
        );
        w.write_all(header.as_bytes())?;
        w.write_all(&self.body)?;
        w.flush()
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        421 => "Misdirected Request",
        431 => "Request Header Fields Too Large",
        501 => "Not Implemented",
        _ => "Internal Server Error",
    }
}
