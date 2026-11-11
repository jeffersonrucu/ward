//! `ward playwright <entry>` (Phase 44; D6, D9; S1–S3, S8, S13).
//!
//! The daemon logs in to a `login` entry with the installed Chrome and returns only the
//! path of the storage state (session, never the password):
//!
//! 1. level and approval (the daemon's usage authorization); blocked = nonexistent;
//! 2. new temporary profile in `<rt>pw-<random>/` (0700, tmpfs), and Chrome in headless
//!    mode with `--remote-debugging-pipe` (commands on fd 3, responses on fd 4; no TCP
//!    port), environment built from scratch (`HOME` and `XDG_*` inside the profile,
//!    `--password-store=basic`: nothing from the keyring nor from the user's home), argv
//!    without secrets (S3);
//! 3. browsing context without disk (`Target.createBrowserContext`, like incognito):
//!    cookies, cache and session stay in Chrome's memory;
//! 4. navigates to the entry URL; every main frame document from another origin is
//!    refused in the browser (a redirect to the phishing site does not load, D9);
//!    checks the page origin (scheme, host and port) against the entry's;
//! 5. fills in an isolated world (the page scripts neither see nor change Ward's code):
//!    only with one visible password field, a `POST` form whose destination is the same
//!    origin, and no new window; submits;
//! 6. after the load, checks the origin again (D6) and that the login page is gone;
//! 7. reads the context cookies and the origin's `localStorage`, builds the storage
//!    state in Playwright's format and checks it with the entry's mask: if the password
//!    (or a variation of it) is there, nothing is written;
//! 8. writes `<rt>playwright/<entry>.json` (0600, temporary + `rename`) with the vault
//!    unlocked and no lock in the middle (lock count equal to the one at the start);
//! 9. kills Chrome (whole group) and deletes the profile **before** answering.
//!
//! The storage states are deleted on lock ([`delete_sessions`], called by the vault
//! lock) and at daemon startup ([`clean_up`]), together with the profiles a
//! dead daemon left behind.

use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use ward_core::entry::{AccessLevel, Data, url_origin};
use ward_core::mask::Mask;
use ward_core::protocol::{Request, ResponseError, Resposta};
use ward_core::totp::Totp;
use ward_platform::child::{Command, Group};

use crate::aprovacao::Cliente;
use crate::cdp::{Cdp, CdpError, Rule, Writer, worst_case};
use crate::nucleo::Daemon;
use crate::warning;

/// The installed Chrome (the same as the extension role, D47).
pub const CHROME: &str = "/opt/google/chrome/chrome";
/// `<rt>playwright/`: the storage states.
pub const SESSIONS_DIR: &str = "playwright";
/// `<rt>pw-<random>/`: the temporary profile of a session.
pub const PROFILE_PREFIX: &str = "pw-";
/// Deadline of the whole browser session (open, log in and read the session).
pub const SESSION_DEADLINE: Duration = Duration::from_secs(60);
/// Deadline of each page load.
const LOAD_DEADLINE: Duration = Duration::from_secs(20);
/// Chrome's `PATH` (it finds itself by the absolute path; this is for what it calls).
const CHILD_PATH: &[u8] = b"/usr/bin:/bin";

/// Preferences of the new profile: no password manager and no autofill (nothing from
/// the page goes to the profile files).
const PREFERENCES: &[u8] = br#"{"credentials_enable_service":false,"profile":{"password_manager_enabled":false},"autofill":{"enabled":false,"profile_enabled":false,"credit_card_enabled":false}}"#;

/// Fills and submits, in the isolated world (the page scripts reach neither this code
/// nor its prototypes). Arguments: username, password, TOTP code (or `null`) and the
/// entry's origin. Returns `"ok"`, or one of two refusal codes: other origin, or no
/// safe form (checked in [`log_in`]). The JavaScript is a literal and keeps its pt-BR
/// names (m2: PRD rule 5).
///
/// DOM clobbering (as in `extension/page.js`): `document` and the `<form>` have named
/// properties coming from the page (an `<img name="querySelectorAll">` replaces
/// `document.querySelectorAll`; an `<input name="action">`, `form.action`). In today's
/// Chrome the `<form>` ones reach the isolated world and the `document` ones do not
/// (checked in Phase 44a), but the script does not depend on that: lookup, getters and
/// methods come from the isolated world's prototypes
/// (`Document.prototype.querySelectorAll.call(document, …)`, descriptors of
/// `HTMLFormElement`/`HTMLInputElement`/`HTMLButtonElement`, `Element.prototype`).
const FILL_SCRIPT: &str = r#"function (usuario, senha, codigo, origem) {
  if (location.origin !== origem) return "origem";
  const desc = (P, k) => Object.getOwnPropertyDescriptor(P, k);
  const pegar = (P, k) => { const g = desc(P, k).get; return o => g.call(o); };
  const F = HTMLFormElement.prototype, I = HTMLInputElement.prototype, B = HTMLButtonElement.prototype;
  const E = Element.prototype, H = HTMLElement.prototype, T = EventTarget.prototype;
  const prop = (o, k) => desc(F, k).get.call(o);
  const tipoDe = pegar(I, "type"), formDe = pegar(I, "form"), autoDe = pegar(I, "autocomplete");
  const desabilitado = pegar(I, "disabled"), soLeitura = pegar(I, "readOnly");
  const botaoTipo = pegar(B, "type"), botaoDesabilitado = pegar(B, "disabled");
  const entrada = c => c instanceof HTMLInputElement;
  const ehBotao = c => c instanceof HTMLButtonElement;
  const visivel = c => {
    if (E.getClientRects.call(c).length === 0) return false;
    if (entrada(c)) return !desabilitado(c) && !soLeitura(c);
    if (ehBotao(c)) return !botaoDesabilitado(c);
    return false;
  };
  const lista = x => Array.prototype.slice.call(x);
  const senhas = lista(Document.prototype.querySelectorAll.call(document, 'input[type="password"]')).filter(visivel);
  if (senhas.length !== 1) return "formulario";
  const pw = senhas[0];
  const form = formDe(pw);
  if (!form) return "formulario";
  const destino = (acao, metodo, alvo) => {
    let u;
    try { u = new URL(acao, location.href); } catch (e) { return "formulario"; }
    if (u.origin !== origem) return "origem";
    if (String(metodo).toLowerCase() !== "post") return "formulario";
    if (alvo && alvo !== "_self") return "formulario";
    return "ok";
  };
  let r = destino(prop(form, "action"), prop(form, "method"), prop(form, "target"));
  if (r !== "ok") return r;
  const els = lista(prop(form, "elements"));
  let user = els.find(c => entrada(c) && /\busername\b/.test(autoDe(c) || "") && visivel(c));
  if (!user) {
    const antes = els.slice(0, els.indexOf(pw)).filter(c => entrada(c) && ["text", "email", "tel"].includes(tipoDe(c)) && visivel(c));
    user = antes[antes.length - 1];
  }
  if (!user) return "formulario";
  const totp = codigo === null ? null : els.find(c => entrada(c) && /\bone-time-code\b/.test(autoDe(c) || "") && visivel(c));
  const botao = els.find(c => ((ehBotao(c) && botaoTipo(c) === "submit") || (entrada(c) && (tipoDe(c) === "submit" || tipoDe(c) === "image"))) && visivel(c));
  if (botao) {
    const P = ehBotao(botao) ? B : I;
    const g = k => desc(P, k).get.call(botao);
    const tem = k => E.hasAttribute.call(botao, k);
    r = destino(tem("formaction") ? g("formAction") : prop(form, "action"), tem("formmethod") ? g("formMethod") : prop(form, "method"), tem("formtarget") ? g("formTarget") : prop(form, "target"));
    if (r !== "ok") return r;
  }
  const valor = desc(I, "value").set;
  const definir = (c, v) => {
    H.focus.call(c);
    valor.call(c, v);
    T.dispatchEvent.call(c, new Event("input", { bubbles: true }));
    T.dispatchEvent.call(c, new Event("change", { bubbles: true }));
  };
  definir(user, usuario);
  definir(pw, senha);
  if (totp) definir(totp, codigo);
  F.requestSubmit.call(form, botao || null);
  return "ok";
}"#;

/// After the login: whether there is still a visible password field (the login page
/// came back) and the origin's `localStorage`, in JSON. Also through the prototypes (DOM
/// clobbering: an `<img name="querySelectorAll">` on the page after the submission does
/// not hide the password).
const READ_PAGE_SCRIPT: &str = r#"function () {
  const vis = c => Element.prototype.getClientRects.call(c).length > 0;
  const senha = Array.prototype.slice.call(
    Document.prototype.querySelectorAll.call(document, 'input[type="password"]')).some(vis);
  const ls = [];
  try {
    const S = Storage.prototype;
    const n = Object.getOwnPropertyDescriptor(S, "length").get.call(localStorage);
    for (let i = 0; i < n; i++) {
      const k = S.key.call(localStorage, i);
      ls.push([k, S.getItem.call(localStorage, k)]);
    }
  } catch (e) {}
  return JSON.stringify({ senha: senha, ls: ls });
}"#;

// --- Files (S2) -------------------------------------------------------------------------

/// `<rt>playwright/`.
pub fn sessions_dir(rt: &Path) -> PathBuf {
    rt.join(SESSIONS_DIR)
}

/// Deletes the storage states (`<rt>playwright/*`). Called on lock. Does not follow
/// links: the directory is opened with `O_DIRECTORY | O_NOFOLLOW` (a link planted in
/// place of `<rt>playwright` does not open, and nothing is deleted from its target) and
/// the content is listed and deleted through the fd (Phase 44e;
/// `ward_platform::dir::delete_contents`): swapping the directory for a link after it
/// was opened does not change what is deleted. Each item is deleted as it is (a link
/// becomes just the deleted link).
pub fn delete_sessions(rt: &Path) -> usize {
    use std::os::fd::AsFd;
    let dir = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(sessions_dir(rt))
    {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return 0,
        Err(_) => {
            warning!("playwright: a pasta das sessões não é uma pasta; nada apagado nela");
            return 0;
        }
    };
    ward_platform::dir::delete_contents(dir.as_fd())
}

/// Daemon startup (D6): deletes the storage states and the `pw-*` profiles a dead
/// daemon left behind. Returns how many items it deleted.
pub fn clean_up(rt: &Path) -> io::Result<usize> {
    let mut n = delete_sessions(rt);
    let items = match fs::read_dir(rt) {
        Ok(i) => i,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(n),
        Err(e) => return Err(e),
    };
    for item in items.flatten() {
        let name = item.file_name();
        if name
            .as_encoded_bytes()
            .starts_with(PROFILE_PREFIX.as_bytes())
            && item.file_type().is_ok_and(|t| t.is_dir())
        {
            fs::remove_dir_all(item.path())?;
            n += 1;
        }
    }
    Ok(n)
}

/// The session's Chrome and profile: on drop, kills the whole group, reaps it and
/// deletes the profile.
struct Browser {
    group: Option<Group>,
    profile: PathBuf,
}

impl Drop for Browser {
    fn drop(&mut self) {
        if let Some(g) = self.group.take() {
            g.kill();
            let _ = g.collect();
        }
        // `remove_dir_all` does not follow symbolic links.
        if fs::remove_dir_all(&self.profile).is_err() && self.profile.exists() {
            warning!("playwright: não deu para apagar o perfil temporário");
        }
    }
}

/// Chrome's argv: only fixed options and the profile path (never a secret, S3).
pub fn chrome_argv(chrome: &Path, profile: &Path) -> Option<Vec<Vec<u8>>> {
    use std::os::unix::ffi::OsStrExt;
    let mut dir = b"--user-data-dir=".to_vec();
    dir.extend_from_slice(profile.as_os_str().as_bytes());
    let mut v = vec![chrome.as_os_str().as_bytes().to_vec()];
    for a in [
        "--headless",
        "--remote-debugging-pipe",
        "--no-first-run",
        "--no-default-browser-check",
        "--password-store=basic",
        "--disable-extensions",
        "--disable-component-extensions-with-background-pages",
        "--disable-background-networking",
        "--disable-component-update",
        "--disable-default-apps",
        "--disable-sync",
        "--disable-breakpad",
        "--disable-client-side-phishing-detection",
        "--disable-features=Translate,MediaRouter,OptimizationHints,AutofillServerCommunication,PasswordManagerOnboarding",
        "--deny-permission-prompts",
        // Phase 44c (D9): `localhost` always at 127.0.0.1. Without this Chrome tries
        // `[::1]` first, and another process of the uid listening there (with the
        // service only on 127.0.0.1) gets the password on an origin that matches.
        "--host-resolver-rules=MAP localhost 127.0.0.1",
        "--no-pings",
        "--metrics-recording-only",
        "--mute-audio",
    ] {
        v.push(a.as_bytes().to_vec());
    }
    v.push(dir);
    v.push(b"about:blank".to_vec());
    v.iter().all(|a| !a.contains(&0)).then_some(v)
}

/// Creates the profile and opens Chrome with the debugging pipe.
fn open_browser(chrome: &Path, rt: &Path) -> Result<(Browser, Cdp), ResponseError> {
    use std::os::unix::ffi::OsStrExt;
    let failed = |_| ResponseError::BrowserFailed;
    ward_platform::prepare_dir(rt).map_err(|_| ResponseError::DeliveryFailed)?;
    let suffix = ward_core::generator::connect_token().map_err(|_| ResponseError::Internal)?;
    let suffix =
        std::str::from_utf8(&suffix.expose()[..16]).map_err(|_| ResponseError::Internal)?;
    let profile = rt.join(format!("{PROFILE_PREFIX}{suffix}"));
    // New directory: never reuses one that already exists.
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&profile)
        .map_err(failed)?;
    let mut browser = Browser {
        group: None,
        profile: profile.clone(),
    };
    // Short names: Chrome creates the `SingletonSocket` inside `TMPDIR`, and the path of
    // a Unix socket has at most 107 bytes.
    for sub in ["h", "c", "k", "d", "t", "Default"] {
        fs::DirBuilder::new()
            .mode(0o700)
            .create(profile.join(sub))
            .map_err(failed)?;
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(profile.join("Default/Preferences"))
        .and_then(|mut f| f.write_all(PREFERENCES))
        .map_err(failed)?;
    let argv = chrome_argv(chrome, &profile).ok_or(ResponseError::BrowserFailed)?;
    let p = profile.as_os_str().as_bytes();
    let mut c = Command::new(&argv[0], p).map_err(|_| ResponseError::BrowserFailed)?;
    for a in &argv[1..] {
        c.argument(a).map_err(|_| ResponseError::BrowserFailed)?;
    }
    let joined = |s: &[u8]| [p, s].concat();
    let environment: [(&[u8], Vec<u8>); 8] = [
        (b"PATH", CHILD_PATH.to_vec()),
        (b"HOME", joined(b"/h")),
        (b"XDG_CONFIG_HOME", joined(b"/c")),
        (b"XDG_CACHE_HOME", joined(b"/k")),
        (b"XDG_DATA_HOME", joined(b"/d")),
        (b"TMPDIR", joined(b"/t")),
        (b"LANG", b"C.UTF-8".to_vec()),
        // No session bus: nothing from the keyring nor from the user's services.
        (b"DBUS_SESSION_BUS_ADDRESS", b"disabled:".to_vec()),
    ];
    for (k, v) in &environment {
        c.variable(k, v).map_err(|_| ResponseError::BrowserFailed)?;
    }
    let (group, pipe) = c.launch_with_debugging().map_err(|e| {
        warning!("playwright: o Chrome não abriu ({})", e.message());
        ResponseError::BrowserFailed
    })?;
    browser.group = Some(group);
    let cdp = Cdp::new(
        pipe.commands,
        pipe.responses,
        Instant::now() + SESSION_DEADLINE,
    );
    Ok((browser, cdp))
}

// --- Session in the browser (D6, D9) ----------------------------------------------------

/// What goes to the page: username, password and TOTP code (if the entry has a seed).
struct Credential {
    username: String,
    password: Zeroizing<String>,
    code: Option<Zeroizing<String>>,
}

#[derive(Deserialize)]
struct Context {
    #[serde(rename = "browserContextId")]
    id: String,
}

#[derive(Deserialize)]
struct Target {
    #[serde(rename = "targetId")]
    id: String,
}

#[derive(Deserialize)]
struct Attached {
    #[serde(rename = "sessionId")]
    session: String,
}

#[derive(Deserialize)]
struct FrameTree {
    #[serde(rename = "frameTree")]
    tree: FrameNode,
}

#[derive(Deserialize)]
struct FrameNode {
    frame: Frame,
}

#[derive(Deserialize)]
struct Frame {
    id: String,
    url: String,
    #[serde(rename = "securityOrigin", default)]
    security_origin: String,
}

#[derive(Deserialize)]
struct Navigation {
    #[serde(rename = "errorText")]
    error_text: Option<serde::de::IgnoredAny>,
}

#[derive(Deserialize)]
struct World {
    #[serde(rename = "executionContextId")]
    id: u64,
}

#[derive(Deserialize)]
struct Evaluation {
    result: RemoteValue,
    #[serde(rename = "exceptionDetails")]
    exception: Option<serde::de::IgnoredAny>,
}

#[derive(Deserialize)]
struct RemoteValue {
    value: Option<String>,
}

/// What [`READ_PAGE_SCRIPT`] returns (keys of the JavaScript literal).
#[derive(Deserialize)]
struct PageState {
    #[serde(rename = "senha")]
    password: bool,
    ls: Vec<(String, String)>,
}

#[derive(Deserialize)]
struct Cookies {
    cookies: Vec<CookieCdp>,
}

#[derive(Deserialize)]
struct CookieCdp {
    name: String,
    value: String,
    domain: String,
    path: String,
    expires: f64,
    #[serde(rename = "httpOnly")]
    http_only: bool,
    secure: bool,
    #[serde(rename = "sameSite")]
    same_site: Option<String>,
}

/// Storage state in Playwright's format (`browser.new_context(storage_state=…)`).
#[derive(Serialize)]
struct StorageState<'a> {
    cookies: Vec<CookiePw<'a>>,
    origins: Vec<OriginPw<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CookiePw<'a> {
    name: &'a str,
    value: &'a str,
    domain: &'a str,
    path: &'a str,
    expires: f64,
    http_only: bool,
    secure: bool,
    same_site: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OriginPw<'a> {
    origin: &'a str,
    local_storage: Vec<ItemPw<'a>>,
}

#[derive(Serialize)]
struct ItemPw<'a> {
    name: &'a str,
    value: &'a str,
}

fn cdp_error(cdp: &Cdp, e: CdpError) -> ResponseError {
    if cdp.blocked {
        return ResponseError::OriginMismatch;
    }
    warning!(
        "playwright: a conversa com o navegador acabou ({})",
        match e {
            CdpError::Deadline => "prazo",
            CdpError::Closed => "fechado",
            CdpError::Protocol => "protocolo",
            CdpError::Command => "comando recusado",
            CdpError::Io => "leitura ou escrita",
        }
    );
    ResponseError::BrowserFailed
}

/// A JSON object of parameters with text pairs.
fn params(pairs: &[(&str, &str)]) -> Result<Zeroizing<Vec<u8>>, ResponseError> {
    let cap = 2 + pairs
        .iter()
        .map(|(k, v)| worst_case(k) + worst_case(v) + 2)
        .sum::<usize>();
    let mut e = Writer::new(cap);
    e.open_object();
    for (k, v) in pairs {
        e.key(k);
        e.text(v);
    }
    e.close_object();
    e.finish().ok_or(ResponseError::Internal)
}

/// The main frame: its id, and whether its URL and origin are the entry's.
fn origin_matches(cdp: &mut Cdp, origin: &str) -> Result<(String, bool), ResponseError> {
    let a: FrameTree = cdp
        .command_as("Page.getFrameTree", b"{}", true)
        .map_err(|e| cdp_error(cdp, e))?;
    let f = a.tree.frame;
    let url = Zeroizing::new(f.url);
    let ok = url_origin(&url).is_some_and(|o| o == origin) && f.security_origin == origin;
    Ok((f.id, ok))
}

/// A function in the isolated world of the main frame; returns the text it returned.
fn in_isolated_world(
    cdp: &mut Cdp,
    frame: &str,
    function: &str,
    arguments: &[Option<&str>],
) -> Result<Zeroizing<String>, ResponseError> {
    let p = params(&[("frameId", frame), ("worldName", "ward")])?;
    let w: World = cdp
        .command_as("Page.createIsolatedWorld", &p, true)
        .map_err(|e| cdp_error(cdp, e))?;
    let cap = 256
        + worst_case(function)
        + arguments
            .iter()
            .map(|a| a.map_or(16, worst_case) + 16)
            .sum::<usize>();
    let mut e = Writer::new(cap);
    e.open_object();
    e.key("functionDeclaration");
    e.text(function);
    e.key("executionContextId");
    e.integer(w.id);
    e.key("arguments");
    e.open_list();
    for a in arguments {
        e.open_object();
        e.key("value");
        match a {
            Some(v) => e.text(v),
            None => e.null(),
        }
        e.close_object();
    }
    e.close_list();
    e.key("returnByValue");
    e.boolean(true);
    e.key("silent");
    e.boolean(true);
    e.close_object();
    let p = e.finish().ok_or(ResponseError::Internal)?;
    let msg = cdp
        .command("Runtime.callFunctionOn", &p, true)
        .map_err(|e| cdp_error(cdp, e))?;
    drop(p);
    #[derive(Deserialize)]
    struct R {
        result: Evaluation,
    }
    let r: R = serde_json::from_slice(&msg).map_err(|_| ResponseError::BrowserFailed)?;
    if r.result.exception.is_some() {
        return Err(ResponseError::LoginRefused);
    }
    Ok(Zeroizing::new(
        r.result.result.value.ok_or(ResponseError::LoginRefused)?,
    ))
}

/// Steps 3 to 7 of the module: logs in and returns the storage state in JSON.
fn log_in(
    cdp: &mut Cdp,
    url: &str,
    origin: &str,
    cred: &Credential,
) -> Result<Zeroizing<Vec<u8>>, ResponseError> {
    let ctx: Context = cdp
        .command_as(
            "Target.createBrowserContext",
            b"{\"disposeOnDetach\":true}",
            false,
        )
        .map_err(|e| cdp_error(cdp, e))?;
    let p = params(&[("url", "about:blank"), ("browserContextId", &ctx.id)])?;
    let target: Target = cdp
        .command_as("Target.createTarget", &p, false)
        .map_err(|e| cdp_error(cdp, e))?;
    let mut e = Writer::new(64 + worst_case(&target.id));
    e.open_object();
    e.key("targetId");
    e.text(&target.id);
    e.key("flatten");
    e.boolean(true);
    e.close_object();
    let p = e.finish().ok_or(ResponseError::Internal)?;
    let attached: Attached = cdp
        .command_as("Target.attachToTarget", &p, false)
        .map_err(|e| cdp_error(cdp, e))?;
    cdp.session = Some(attached.session);
    let p = params(&[("behavior", "deny"), ("browserContextId", &ctx.id)])?;
    cdp.command("Browser.setDownloadBehavior", &p, false)
        .map_err(|e| cdp_error(cdp, e))?;
    cdp.command("Page.enable", b"{}", true)
        .map_err(|e| cdp_error(cdp, e))?;
    let (frame, _) = origin_matches(cdp, origin)?;
    cdp.rule = Some(Rule {
        origin: origin.to_owned(),
        frame: frame.clone(),
    });
    cdp.command(
        "Fetch.enable",
        br#"{"patterns":[{"urlPattern":"*","resourceType":"Document","requestStage":"Request"}]}"#,
        true,
    )
    .map_err(|e| cdp_error(cdp, e))?;
    // Login page.
    let before = cdp.loads;
    let p = params(&[("url", url)])?;
    let nav: Navigation = cdp
        .command_as("Page.navigate", &p, true)
        .map_err(|e| cdp_error(cdp, e))?;
    if cdp.blocked {
        return Err(ResponseError::OriginMismatch);
    }
    if nav.error_text.is_some() {
        warning!("playwright: a página da entrada não abriu");
        return Err(ResponseError::BrowserFailed);
    }
    let loaded = cdp
        .wait_load(before, Instant::now() + LOAD_DEADLINE)
        .map_err(|e| cdp_error(cdp, e))?;
    if cdp.blocked {
        return Err(ResponseError::OriginMismatch);
    }
    if !loaded {
        return Err(ResponseError::BrowserFailed);
    }
    let (f, ok) = origin_matches(cdp, origin)?;
    if !ok || f != frame {
        warning!("playwright: a página não é da origem da entrada (D9)");
        return Err(ResponseError::OriginMismatch);
    }
    // Fills and submits.
    let before = cdp.loads;
    let r = in_isolated_world(
        cdp,
        &frame,
        FILL_SCRIPT,
        &[
            Some(cred.username.as_str()),
            Some(cred.password.as_str()),
            cred.code.as_deref().map(String::as_str),
            Some(origin),
        ],
    )?;
    match r.as_str() {
        "ok" => {}
        "origem" => {
            warning!("playwright: o formulário envia para outra origem (D9)");
            return Err(ResponseError::OriginMismatch);
        }
        _ => {
            warning!("playwright: a página não tem um formulário de login seguro");
            return Err(ResponseError::LoginRefused);
        }
    }
    let loaded = cdp
        .wait_load(before, Instant::now() + LOAD_DEADLINE)
        .map_err(|e| cdp_error(cdp, e))?;
    if cdp.blocked {
        return Err(ResponseError::OriginMismatch);
    }
    if !loaded {
        warning!("playwright: nenhuma página nova depois do envio");
        return Err(ResponseError::LoginRefused);
    }
    // D6: the final origin is still the entry's.
    let (f, ok) = origin_matches(cdp, origin)?;
    if !ok {
        warning!("playwright: a página final não é da origem da entrada (D9)");
        return Err(ResponseError::OriginMismatch);
    }
    let page = in_isolated_world(cdp, &f, READ_PAGE_SCRIPT, &[])?;
    let page: PageState = serde_json::from_str(&page).map_err(|_| ResponseError::BrowserFailed)?;
    if page.password {
        warning!("playwright: a página de login continua aberta (login recusado)");
        return Err(ResponseError::LoginRefused);
    }
    let p = params(&[("browserContextId", &ctx.id)])?;
    let c: Cookies = cdp
        .command_as("Storage.getCookies", &p, false)
        .map_err(|e| cdp_error(cdp, e))?;
    build_state(&c.cookies, origin, &page.ls)
}

/// The storage state in JSON (Playwright's format).
fn build_state(
    cookies: &[CookieCdp],
    origin: &str,
    ls: &[(String, String)],
) -> Result<Zeroizing<Vec<u8>>, ResponseError> {
    let mut list = Vec::with_capacity(cookies.len());
    for c in cookies {
        let same_site = match c.same_site.as_deref() {
            Some("Strict") => "Strict",
            Some("None") => "None",
            _ => "Lax",
        };
        let expires = if c.expires.is_finite() && c.expires > 0.0 {
            c.expires
        } else {
            -1.0
        };
        list.push(CookiePw {
            name: &c.name,
            value: &c.value,
            domain: &c.domain,
            path: &c.path,
            expires,
            http_only: c.http_only,
            secure: c.secure,
            same_site,
        });
    }
    let mut origins = Vec::new();
    if !ls.is_empty() {
        origins.push(OriginPw {
            origin,
            local_storage: ls
                .iter()
                .map(|(k, v)| ItemPw { name: k, value: v })
                .collect(),
        });
    }
    let state = StorageState {
        cookies: list,
        origins,
    };
    serde_json::to_vec_pretty(&state)
        .map(Zeroizing::new)
        .map_err(|_| ResponseError::Internal)
}

/// S1/V6.13: the storage state must not have the password nor a variation of it
/// (through the entry's mask: literal, URL, base64, text forms).
pub fn has_secret(mask: &Mask, state: &[u8]) -> bool {
    let masked = Zeroizing::new(mask.mask_all(state));
    masked.as_slice() != state
}

/// Writes `<rt>playwright/<entry>.json` (0600) through temporary + `rename`.
fn write_state(rt: &Path, entry: &str, state: &[u8]) -> io::Result<PathBuf> {
    let name_ok = !entry.is_empty()
        && !entry.starts_with('.')
        && entry.bytes().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
        });
    if !name_ok {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let error = |_| io::Error::from(io::ErrorKind::PermissionDenied);
    ward_platform::prepare_dir(rt).map_err(error)?;
    let dir = sessions_dir(rt);
    ward_platform::prepare_dir(&dir).map_err(error)?;
    let suffix =
        ward_core::generator::connect_token().map_err(|_| io::Error::other("aleatório"))?;
    let suffix =
        std::str::from_utf8(&suffix.expose()[..12]).map_err(|_| io::Error::other("aleatório"))?;
    let tmp = dir.join(format!(".{entry}.{suffix}.tmp"));
    let destination = dir.join(format!("{entry}.json"));
    let r = (|| {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&tmp)?;
        f.write_all(state)?;
        f.sync_all()?;
        fs::rename(&tmp, &destination)
    })();
    if r.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    r.map(|()| destination)
}

impl Daemon {
    /// `ward playwright <entry>` (D6): the response is only the path of the storage
    /// state.
    pub(crate) fn playwright(
        &self,
        client: &dyn Cliente,
        request: &Request,
    ) -> Result<Resposta, ResponseError> {
        let Request::Playwright { entry } = request else {
            return Err(ResponseError::InvalidRequest);
        };
        // Type without a visible login: refuse before asking for approval.
        if let Some(a) = self.estado().aberto.as_ref()
            && a.entradas
                .iter()
                .find(|e| e.name() == entry && e.level() != AccessLevel::Blocked)
                .is_some_and(|e| !matches!(e.data(), Data::Login(_)))
        {
            return Err(ResponseError::TypeWithoutDelivery);
        }
        self.authorize_use(client, request)?;
        self.registrar_uso();
        let rt = self
            .temporarios()
            .ok_or(ResponseError::DeliveryFailed)?
            .to_path_buf();
        let (url, cred, mask, locks, initial) = {
            let st = self.estado();
            let a = st.aberto.as_ref().ok_or(ResponseError::Locked)?;
            // The entry may have changed during the approval: check again.
            let e = a
                .entradas
                .iter()
                .find(|e| e.name() == entry && e.level() != AccessLevel::Blocked)
                .ok_or(ResponseError::EntryNotFound)?;
            let Data::Login(l) = e.data() else {
                return Err(ResponseError::TypeWithoutDelivery);
            };
            let password = l
                .password
                .expose_text()
                .ok_or(ResponseError::DeliveryFailed)?;
            let mut s = Zeroizing::new(String::with_capacity(password.len()));
            s.push_str(password);
            let code = match &l.totp_seed {
                Some(seed) => {
                    let c = Totp::from_seed(seed)
                        .and_then(|t| t.code_now())
                        .map_err(|_| ResponseError::DeliveryFailed)?;
                    let mut z = Zeroizing::new(String::with_capacity(c.code().len()));
                    z.push_str(c.code());
                    Some(z)
                }
                None => None,
            };
            let mask = Mask::from_entry(e, None).map_err(|_| ResponseError::DeliveryFailed)?;
            let (locks, initial) = self.travas_agora(&st);
            (
                l.url.clone(),
                Credential {
                    username: l.username.clone(),
                    password: s,
                    code,
                },
                mask,
                locks,
                initial,
            )
        };
        let origin = url_origin(&url).ok_or(ResponseError::DeliveryFailed)?;
        warning!("playwright: abrindo o navegador");
        let (browser, mut cdp) = open_browser(self.chrome(), &rt)?;
        let result = log_in(&mut cdp, &url, &origin, &cred);
        drop(cred);
        // Chrome dies and the profile is gone before the response (D6).
        drop(cdp);
        drop(browser);
        let state = result?;
        if has_secret(&mask, &state) {
            warning!("playwright: a sessão do site guarda a senha; nada foi gravado (S1)");
            return Err(ResponseError::SessionHasSecret);
        }
        // Writes with the vault unlocked and no lock since the start; locking afterwards
        // deletes it.
        let st = self.estado();
        if st.aberto.is_none() || locks.load(std::sync::atomic::Ordering::Acquire) != initial {
            return Err(ResponseError::Locked);
        }
        let path = write_state(&rt, entry, &state).map_err(|e| {
            warning!("playwright: não deu para gravar a sessão ({})", e.kind());
            ResponseError::DeliveryFailed
        })?;
        drop(st);
        let path = path
            .into_os_string()
            .into_string()
            .map_err(|_| ResponseError::Internal)?;
        warning!("playwright: sessão gravada");
        Ok(Resposta::SessaoPlaywright { caminho: path })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_only_fixed_with_pipe_and_no_port() {
        let v = chrome_argv(Path::new(CHROME), Path::new("/run/user/1000/ward/pw-x")).unwrap();
        let t: Vec<String> = v
            .iter()
            .map(|a| String::from_utf8(a.clone()).unwrap())
            .collect();
        assert!(t.contains(&"--remote-debugging-pipe".to_owned()));
        assert!(t.contains(&"--headless".to_owned()));
        assert!(t.contains(&"--password-store=basic".to_owned()));
        assert!(t.iter().all(|a| !a.contains("remote-debugging-port")));
        assert!(t.iter().all(|a| !a.contains("no-sandbox")));
        // Phase 44c (D9): `localhost` never goes to `[::1]`.
        assert!(t.contains(&"--host-resolver-rules=MAP localhost 127.0.0.1".to_owned()));
        assert_eq!(t.last().unwrap(), "about:blank");
    }

    /// Phase 44c (low finding of the M6 audit): `<rt>playwright` swapped for a link to
    /// another directory of the uid. Locking deletes nothing on the other side; a link
    /// inside the sessions directory goes away without taking the target with it.
    #[test]
    fn attack_delete_sessions_does_not_follow_link() {
        use std::os::unix::fs::symlink;
        let base = std::env::temp_dir().join(format!("ward-pw-link-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let rt = base.join("rt");
        let target = base.join("target");
        fs::create_dir_all(&rt).unwrap();
        fs::create_dir_all(target.join("sub")).unwrap();
        fs::write(target.join("precious.txt"), b"nao apagar").unwrap();
        fs::write(target.join("sub/other.txt"), b"nao apagar").unwrap();
        symlink(&target, sessions_dir(&rt)).unwrap();
        assert_eq!(delete_sessions(&rt), 0);
        assert!(target.join("precious.txt").exists());
        assert!(target.join("sub/other.txt").exists());
        // The startup `clean_up` does not follow the link either.
        assert_eq!(clean_up(&rt).unwrap(), 0);
        assert!(target.join("precious.txt").exists());

        // A real directory with a link inside: the link goes away, the target stays.
        fs::remove_file(sessions_dir(&rt)).unwrap();
        fs::create_dir(sessions_dir(&rt)).unwrap();
        fs::write(sessions_dir(&rt).join("acme-wp-prod.json"), b"{}").unwrap();
        symlink(&target, sessions_dir(&rt).join("link")).unwrap();
        assert_eq!(delete_sessions(&rt), 2);
        assert!(!sessions_dir(&rt).join("acme-wp-prod.json").exists());
        assert!(fs::symlink_metadata(sessions_dir(&rt).join("link")).is_err());
        assert!(target.join("precious.txt").exists());
        assert!(target.join("sub/other.txt").exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn state_in_playwright_format() {
        let c = vec![CookieCdp {
            name: "wordpress_logged_in_x".into(),
            value: "wp_editor|123|abc".into(),
            domain: "localhost".into(),
            path: "/".into(),
            expires: -1.0,
            http_only: true,
            secure: false,
            same_site: None,
        }];
        let ls = vec![("k".to_owned(), "v".to_owned())];
        let b = build_state(&c, "http://localhost:8081", &ls).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&b).unwrap();
        assert_eq!(v["cookies"][0]["httpOnly"], true);
        assert_eq!(v["cookies"][0]["sameSite"], "Lax");
        assert_eq!(v["cookies"][0]["expires"], -1.0);
        assert_eq!(v["origins"][0]["origin"], "http://localhost:8081");
        assert_eq!(v["origins"][0]["localStorage"][0]["name"], "k");
        let b = build_state(&c, "http://localhost:8081", &[]).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&b).unwrap();
        assert_eq!(v["origins"].as_array().unwrap().len(), 0);
    }

    /// DOM clobbering (Phase 44a): the isolated world scripts read nothing through
    /// `document`, the `<form>` nor `localStorage` (which have named properties coming
    /// from the page); everything comes from the prototypes. The attack with the real
    /// Chrome is in `tests/playwright.rs` (the two DOM clobbering cases).
    #[test]
    fn attack_dom_clobbering_scripts_only_through_prototypes() {
        for (name, js) in [
            ("FILL_SCRIPT", FILL_SCRIPT),
            ("READ_PAGE_SCRIPT", READ_PAGE_SCRIPT),
        ] {
            for forbidden in [
                "document.",
                "form.",
                "localStorage.",
                "pw.",
                "c.type",
                "c.autocomplete",
                "c.disabled",
                "c.readOnly",
                "c.getClientRects",
                "c.focus",
                "c.dispatchEvent",
                "botao.",
                ".elements",
            ] {
                assert!(!js.contains(forbidden), "{name} lê `{forbidden}` direto");
            }
            assert!(
                js.contains("Document.prototype.querySelectorAll.call(document"),
                "{name}"
            );
        }
    }

    /// Attack: an entry name with `/` or starting with `.` does not leave the directory.
    #[test]
    fn attack_name_outside_dir_refused() {
        let d = std::env::temp_dir().join(format!("ward-pw-test-{}", std::process::id()));
        for bad in ["../x", "a/b", ".x", "", "A"] {
            assert!(write_state(&d, bad, b"{}").is_err(), "{bad}");
        }
        let _ = fs::remove_dir_all(&d);
    }
}
