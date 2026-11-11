//! `ward playwright` in the core (Phase 44; D6, D9; S1–S3, S13), with a fake Chrome
//! (`tests/fake_chrome.py`) that speaks the DevTools Protocol over the debugging pipe
//! (fds 3 and 4) and never opens the network. The end-to-end test with the real Chrome
//! and the test services is `make test-playwright` (`dev/tests/playwright.sh`).
//!
//! The fake Chrome comes in through `Daemon::with_chrome`, which only exists with the
//! `test-hooks` feature (`make test` turns them all on).

#![cfg(feature = "test-hooks")]

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use common::*;
use ward_core::protocol::{ResponseError, Resposta, Role};
use wardd::approval::NoClient;
use wardd::core::Daemon;

/// Password of the free login entry of the common test vault (fake).
const LOGIN_PASSWORD: &str = SECRETS[3];

/// `<rt>` of this test only, in `/run/user/<uid>/`.
struct Rt(PathBuf);

impl Rt {
    fn new(label: &str) -> Rt {
        let p = ward_platform::socket::runtime_dir()
            .join(format!("wpw-{}-{label}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        Rt(p)
    }
}

impl Drop for Rt {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Scene {
    d: Arc<Daemon>,
    _dir: Dir,
    rt: Rt,
    base: PathBuf,
}

fn scene(label: &str, mode: &str) -> Scene {
    let dir = Dir::new(label);
    let (d, _) = test_vault(&dir);
    let base = dir.0.join("chrome");
    fs::create_dir(&base).unwrap();
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fake_chrome.py");
    let chrome = base.join("chrome");
    fs::write(
        &chrome,
        format!(
            "#!/bin/sh\nexec /usr/bin/python3 -I '{}' '{}' \"$@\"\n",
            script.display(),
            base.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&chrome, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(base.join("modo"), mode).unwrap();
    let rt = Rt::new(label);
    let d = d.with_chrome(chrome).with_temp_root(rt.0.clone());
    Scene {
        d: Arc::new(d),
        _dir: dir,
        rt,
        base,
    }
}

impl Scene {
    fn playwright(&self, entry: &str) -> Resposta {
        let p = req(
            Role::Agent,
            "playwright",
            &format!(r#"{{"entrada":"{entry}"}}"#),
        )
        .unwrap();
        self.d.serve_from(&NoClient, Role::Agent, p)
    }

    fn records(&self) -> Vec<serde_json::Value> {
        fs::read_to_string(self.base.join("registro"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn events(&self, name: &str) -> Vec<serde_json::Value> {
        self.records()
            .into_iter()
            .filter(|e| e["evento"] == name)
            .collect()
    }

    fn session(&self, entry: &str) -> PathBuf {
        self.rt.0.join("playwright").join(format!("{entry}.json"))
    }

    /// No `pw-*` profile was left.
    fn no_profile(&self) -> bool {
        fs::read_dir(&self.rt.0).map_or(true, |it| {
            !it.flatten()
                .any(|e| e.file_name().to_string_lossy().starts_with("pw-"))
        })
    }
}

fn sha256_hex(b: &[u8]) -> String {
    let out = std::process::Command::new("/usr/bin/sha256sum")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut c| {
            use std::io::Write;
            c.stdin.take().unwrap().write_all(b)?;
            c.wait_with_output()
        })
        .unwrap();
    String::from_utf8(out.stdout).unwrap()[..64].to_owned()
}

/// Every form of the password that must not show up (literal, base64, URL).
fn password_forms() -> Vec<Vec<u8>> {
    vec![
        LOGIN_PASSWORD.as_bytes().to_vec(),
        b64(LOGIN_PASSWORD.as_bytes()).into_bytes(),
        LOGIN_PASSWORD.as_bytes()[2..LOGIN_PASSWORD.len() - 2].to_vec(),
    ]
}

/// V6.12/V6.13/V6.14 with the fake Chrome: the answer is only the path; the storage
/// state is 0600, in the Playwright format and without the password; the argv has the
/// pipe and no secret; Chrome's environment is the one built from scratch; only fds
/// 0–4 (0–2 on `/dev/null`); the password only reaches the fill (checked by sha256); the
/// profile is gone before the answer; and locking deletes the session.
#[test]
fn v6_12_session_saved_only_the_path_comes_back() {
    let c = scene("ok", "ok");
    let r = c.playwright("t-livre");
    let expected = c.session("t-livre");
    match &r {
        Resposta::SessaoPlaywright { caminho } => {
            assert_eq!(Path::new(caminho), expected);
        }
        other => panic!("{other:?}"),
    }
    let enc = encoded(Role::Agent, &r);
    for f in password_forms() {
        assert!(!contains(&enc, &f), "a resposta tem a senha");
    }
    let m = fs::symlink_metadata(&expected).unwrap();
    assert!(m.file_type().is_file());
    assert_eq!(m.permissions().mode() & 0o777, 0o600);
    let folder = fs::metadata(c.rt.0.join("playwright")).unwrap();
    assert_eq!(folder.permissions().mode() & 0o777, 0o700);
    let state = fs::read(&expected).unwrap();
    for f in password_forms() {
        assert!(!contains(&state, &f), "o storage state tem a senha");
    }
    let v: serde_json::Value = serde_json::from_slice(&state).unwrap();
    assert_eq!(v["cookies"][0]["name"], "sessao");
    assert_eq!(v["cookies"][0]["sameSite"], "Lax");
    assert_eq!(v["cookies"][0]["httpOnly"], true);
    assert!(v["origins"].as_array().unwrap().is_empty());
    // Chrome: argv, environment, fds.
    let start = &c.events("inicio")[0];
    let argv: Vec<String> = start["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap().to_owned())
        .collect();
    assert!(argv.contains(&"--remote-debugging-pipe".to_owned()));
    assert!(argv.contains(&"--headless".to_owned()));
    assert!(!argv.iter().any(|a| a.contains("remote-debugging-port")));
    assert!(
        argv.iter()
            .any(|a| a.starts_with(&format!("--user-data-dir={}/pw-", c.rt.0.display())))
    );
    let all = serde_json::to_vec(start).unwrap();
    for f in password_forms() {
        assert!(!contains(&all, &f), "argv ou ambiente com a senha");
    }
    let env: Vec<&str> = start["ambiente"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    // D60: only the built one (the fake's `sh` adds PWD).
    for n in &env {
        assert!(
            [
                "PATH",
                "HOME",
                "XDG_CONFIG_HOME",
                "XDG_CACHE_HOME",
                "XDG_DATA_HOME",
                "TMPDIR",
                "LANG",
                "DBUS_SESSION_BUS_ADDRESS",
                "PWD",
                // `python3` adds this one to its own environment.
                "LC_CTYPE",
            ]
            .contains(n),
            "variável a mais no Chrome: {n}"
        );
    }
    assert!(
        start["home"]
            .as_str()
            .unwrap()
            .starts_with(&format!("{}/pw-", c.rt.0.display()))
    );
    for i in ["0", "1", "2"] {
        assert_eq!(start["alvos"][i], "/dev/null", "fd {i}");
    }
    for f in start["fds"].as_array().unwrap() {
        // 0–4 and the one Python itself opened to list them (the highest).
        let f = f.as_u64().unwrap();
        assert!(f <= 5, "fd {f} a mais no Chrome: {start}");
    }
    // The password reached only the fill, with the entry's origin.
    let fill = &c.events("preencher")[0];
    assert_eq!(fill["sha256"][1], sha256_hex(LOGIN_PASSWORD.as_bytes()));
    assert_eq!(fill["origem"], "https://clientex.invalid");
    // The POST with the password was paused and went on (same origin).
    let decisions = c.events("decisao");
    assert!(
        decisions.iter().all(|d| d["seguiu"] == true),
        "{decisions:?}"
    );
    assert!(c.no_profile(), "o perfil temporário ficou");
    // D6: locking deletes the session.
    c.d.lock();
    assert!(!expected.exists(), "a sessão sobreviveu à trava");
}

/// V6.15 (D9): the redirect to the phishing site is refused in the browser before the
/// page loads; the password never reaches Chrome and nothing is saved.
#[test]
fn v6_15_redirect_to_another_origin_refused() {
    let c = scene("redir", "redir");
    let r = c.playwright("t-livre");
    assert_eq!(error(&r), Some(ResponseError::OriginMismatch), "{r:?}");
    let d = c.events("decisao");
    assert_eq!(d.len(), 2, "{d:?}");
    assert_eq!(d[0]["seguiu"], true);
    assert_eq!(d[1]["url"], "http://localhost:8082/wp-login.php");
    assert_eq!(d[1]["seguiu"], false);
    assert!(c.events("preencher").is_empty(), "a senha foi ao Chrome");
    assert!(!c.session("t-livre").exists());
    assert!(c.no_profile());
}

/// D9: the page that claims to be from another origin receives nothing; the form
/// submission to another origin is blocked in the browser; and the fill code refuses a
/// form that submits elsewhere.
#[test]
fn d9_origin_of_the_page_and_of_the_submission() {
    let c = scene("mente", "mente");
    assert_eq!(
        error(&c.playwright("t-livre")),
        Some(ResponseError::OriginMismatch)
    );
    assert!(c.events("preencher").is_empty(), "a senha foi ao Chrome");
    assert!(c.no_profile());

    let c = scene("posoutra", "pos_outra");
    assert_eq!(
        error(&c.playwright("t-livre")),
        Some(ResponseError::OriginMismatch)
    );
    let d = c.events("decisao");
    assert_eq!(
        d.last().unwrap()["url"],
        "http://localhost:8082/wp-login.php"
    );
    assert_eq!(
        d.last().unwrap()["seguiu"],
        false,
        "o POST saiu para outra origem"
    );
    assert!(!c.session("t-livre").exists());

    let c = scene("formoutra", "form_outra");
    assert_eq!(
        error(&c.playwright("t-livre")),
        Some(ResponseError::OriginMismatch)
    );
    assert!(!c.session("t-livre").exists());
    assert!(c.no_profile());
}

/// S1/V6.13: a site that keeps the password (base64) in a cookie or in `localStorage`
/// does not become a storage state: nothing is saved.
#[test]
fn s1_session_with_the_password_is_not_saved() {
    for mode in ["cookie_senha", "ls_senha"] {
        let c = scene(mode, mode);
        assert_eq!(
            error(&c.playwright("t-livre")),
            Some(ResponseError::SessionHasSecret),
            "{mode}"
        );
        assert!(!c.session("t-livre").exists(), "{mode}");
        assert!(
            !c.rt.0.join("playwright").exists()
                || fs::read_dir(c.rt.0.join("playwright")).unwrap().count() == 0,
            "{mode}"
        );
        assert!(c.no_profile(), "{mode}");
    }
}

/// The browser that crashes or sends large garbage: error, the profile is gone and
/// nothing is left running.
#[test]
fn browser_that_crashes_or_sends_garbage() {
    for mode in ["morre", "grande"] {
        let c = scene(mode, mode);
        assert_eq!(
            error(&c.playwright("t-livre")),
            Some(ResponseError::BrowserFailed),
            "{mode}"
        );
        assert!(c.no_profile(), "{mode}");
        assert!(!c.session("t-livre").exists(), "{mode}");
    }
}

/// D6: locking in the middle of the session (before saving) leaves no storage state.
#[test]
fn d6_lock_during_the_session_nothing_saved() {
    let c = scene("lento", "lento");
    let d = c.d.clone();
    let t = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(700));
        d.lock();
    });
    let r = c.playwright("t-livre");
    t.join().unwrap();
    assert_eq!(error(&r), Some(ResponseError::Locked), "{r:?}");
    assert!(!c.session("t-livre").exists());
    assert!(c.no_profile());
}

/// Blocked = nonexistent (V3.17), approval without an app and wrong type: none of them
/// opens the browser.
#[test]
fn refusals_without_opening_the_browser() {
    let c = scene("recusas", "ok");
    for (entry, expected) in [
        ("t-bloq", ResponseError::EntryNotFound),
        ("nao-existe", ResponseError::EntryNotFound),
        ("s1", ResponseError::TypeWithoutDelivery),
    ] {
        assert_eq!(error(&c.playwright(entry)), Some(expected), "{entry}");
    }
    assert!(
        error(&c.playwright("t-aprov")).is_some(),
        "com aprovação sem app"
    );
    assert!(c.events("inicio").is_empty(), "o navegador abriu");
    // The app role does not ask for `playwright`.
    let p = req(Role::Agent, "playwright", r#"{"entrada":"t-livre"}"#).unwrap();
    assert_eq!(
        error(&c.d.serve_from(&NoClient, Role::App, p)),
        Some(ResponseError::Unauthorized)
    );
    assert!(c.events("inicio").is_empty());
}

/// Startup (D6): storage states and profiles of a dead daemon are gone.
#[test]
fn d6_startup_cleans_sessions_and_profiles() {
    let rt = Rt::new("partida");
    fs::create_dir_all(rt.0.join("playwright")).unwrap();
    fs::create_dir_all(rt.0.join("pw-abc/Default")).unwrap();
    fs::write(rt.0.join("playwright/x.json"), b"{}").unwrap();
    fs::write(rt.0.join("pw-abc/Default/Cookies"), b"x").unwrap();
    fs::create_dir_all(rt.0.join("tmp")).unwrap();
    assert_eq!(wardd::playwright::clean_up(&rt.0).unwrap(), 2);
    assert!(!rt.0.join("playwright/x.json").exists());
    assert!(!rt.0.join("pw-abc").exists());
    assert!(
        rt.0.join("tmp").exists(),
        "apagou o que não é do playwright"
    );
    // Without `<rt>`: nothing to do.
    let none = Rt::new("nada");
    assert_eq!(wardd::playwright::clean_up(&none.0).unwrap(), 0);
}

// --- Real Chrome (`make test-playwright`) ---------------------------------------------
//
// The tests below use the installed Chrome against two local sites on 127.0.0.1
// (random port): the entry's "site" and a "phishing" one that records everything it
// receives. They stay out of the plain `cargo test` (`#[ignore]`: they need Chrome) and
// run in `make test-playwright` with `--ignored`.

mod site {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};

    /// A request received: method, path and body.
    #[derive(Debug, Clone)]
    pub struct Request {
        pub method: String,
        pub path: String,
        pub body: Vec<u8>,
    }

    pub struct Site {
        pub port: u16,
        pub requests: Arc<Mutex<Vec<Request>>>,
    }

    pub type Route =
        dyn Fn(&str, &str, &[u8]) -> (u16, Vec<(String, String)>, String) + Send + Sync;

    /// Starts a site; `route(method, path, body) -> (status, extra headers, html)`.
    pub fn start(route: Box<Route>) -> Site {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let p = requests.clone();
        let route: Arc<Route> = Arc::from(route);
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                let p = p.clone();
                let route = route.clone();
                std::thread::spawn(move || serve(s, &p, &*route));
            }
        });
        Site { port, requests }
    }

    fn serve(s: TcpStream, p: &Mutex<Vec<Request>>, route: &Route) {
        let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(10)));
        let mut r = BufReader::new(s.try_clone().unwrap());
        loop {
            let mut line = String::new();
            if r.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                return;
            }
            let (method, path) = (parts[0].to_owned(), parts[1].to_owned());
            let mut size = 0usize;
            loop {
                let mut h = String::new();
                if r.read_line(&mut h).unwrap_or(0) == 0 {
                    return;
                }
                let h = h.trim_end();
                if h.is_empty() {
                    break;
                }
                if let Some((k, v)) = h.split_once(':')
                    && k.eq_ignore_ascii_case("content-length")
                {
                    size = v.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0u8; size.min(1 << 20)];
            if r.read_exact(&mut body).is_err() {
                return;
            }
            p.lock().unwrap().push(Request {
                method: method.clone(),
                path: path.clone(),
                body: body.clone(),
            });
            let (status, extras, html) = route(&method, &path, &body);
            let mut resp = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\n",
                html.len()
            );
            for (k, v) in extras {
                resp.push_str(&format!("{k}: {v}\r\n"));
            }
            resp.push_str("\r\n");
            resp.push_str(&html);
            let mut w = &s;
            if w.write_all(resp.as_bytes()).is_err() {
                return;
            }
        }
    }
}

/// The login form (like WordPress's): `POST` to the login path, with extra `attributes` on
/// the `<form>`, `extra` inside it and a `script` on the page.
fn form(attributes: &str, extra: &str, script: &str) -> String {
    format!(
        r#"<!doctype html><html><body><form id="f" method="post" action="/entrar" {attributes}>
<input type="text" name="log" id="user_login" autocomplete="username">
<input type="password" name="pwd" id="user_pass" autocomplete="current-password">
{extra}
<button type="submit" id="b">Acessar</button></form><script>{script}</script></body></html>"#
    )
}

/// Puts named elements on the page that replace `document.querySelectorAll` and
/// `document.getElementsByTagName` (named properties of `document`).
fn with_clobber(html: &str) -> String {
    html.replacen(
        "<body>",
        r#"<body><img name="querySelectorAll" src="data:,"><img name="getElementsByTagName" src="data:,">"#,
        1,
    )
}

fn login_at(name: &str, url: &str, password: &str) -> String {
    format!(
        r#"{{"entrada":{{"nome":"{name}","grupo":"clientex","ambiente":"dev","tipo":"login","campos":{{"url":"{url}","usuario":"admin","senha":"{}"}}}}}}"#,
        b64(password.as_bytes())
    )
}

struct RealScene {
    d: Arc<Daemon>,
    _dir: Dir,
    rt: Rt,
    site: site::Site,
    phish: site::Site,
}

/// The site's cases and what the daemon must answer (`None`: session saved).
const CASES: [(&str, Option<ResponseError>); 13] = [
    ("ok", None),
    ("dom", None),
    ("dom-recusa", Some(ResponseError::LoginRefused)),
    ("iframe", None),
    ("acao-fora", Some(ResponseError::OriginMismatch)),
    ("clobber", Some(ResponseError::OriginMismatch)),
    ("formaction", Some(ResponseError::OriginMismatch)),
    ("redir", Some(ResponseError::OriginMismatch)),
    ("get", Some(ResponseError::LoginRefused)),
    ("alvo", Some(ResponseError::LoginRefused)),
    ("dois", Some(ResponseError::LoginRefused)),
    ("recusa", Some(ResponseError::LoginRefused)),
    ("ls", Some(ResponseError::SessionHasSecret)),
];

fn page(case: &str, outside: &str, pp: u16) -> (u16, Vec<(String, String)>, String) {
    let f = || form("", "", "");
    let html = match case {
        "ok" => f(),
        "acao-fora" => f().replace(r#"action="/entrar""#, &format!(r#"action="{outside}""#)),
        // The real target is the phishing site; the `<input name="action">` makes the
        // naive `form.action` return the field (a relative path of the same origin).
        "clobber" => form(
            "",
            r#"<input type="hidden" name="action" value="/entrar">"#,
            "",
        )
        .replace(r#"action="/entrar" "#, &format!(r#"action="{outside}" "#)),
        // DOM clobbering (Phase 44a): `<img name="querySelectorAll">` replaces the page's
        // `document.querySelectorAll` (today's Chrome does not carry the named
        // properties of `document` into the isolated world; those of `<form>`, it does)
        // and fields named after each property the script reads replace those of the
        // `<form>`. The right login must get in anyway.
        "dom" => with_clobber(&form(
            "",
            r#"<input type="hidden" name="elements" value="x"><input type="hidden" name="method" value="get"><input type="hidden" name="target" value="_blank"><input type="hidden" name="requestSubmit" value="x"><input type="hidden" name="querySelectorAll" value="x">"#,
            "",
        )),
        // The site refuses and returns the login page with the same clobbering: the
        // visible password must be seen after the submission (nothing saved).
        "dom-recusa" => form("", r#"<input type="hidden" name="recusa" value="2">"#, ""),
        "get" => f().replace(r#"method="post""#, r#"method="get""#),
        "formaction" => f().replace(
            r#"<button type="submit" id="b">"#,
            &format!(r#"<button type="submit" id="b" formaction="{outside}">"#),
        ),
        "alvo" => form(r#"target="_blank""#, "", ""),
        "dois" => form("", r#"<input type="password" name="pwd2">"#, ""),
        "recusa" => form("", r#"<input type="hidden" name="recusa" value="1">"#, ""),
        "ls" => form(
            "",
            "",
            "document.getElementById('f').addEventListener('submit', () => localStorage.setItem('p', btoa(document.getElementById('user_pass').value)));",
        ),
        "iframe" => form(
            "",
            &format!(r#"<iframe src="http://127.0.0.1:{pp}/frame"></iframe>"#),
            "",
        ),
        "redir" => {
            return (
                302,
                vec![("Location".to_owned(), outside.to_owned())],
                String::new(),
            );
        }
        _ => return (404, vec![], "<html>nada</html>".into()),
    };
    (200, vec![], html)
}

fn real_scene(label: &str) -> RealScene {
    let phish = site::start(Box::new(|_, _, _| {
        (200, vec![], "<html><body>falso</body></html>".into())
    }));
    let pp = phish.port;
    let site = site::start(Box::new(move |method, path, body| {
        let outside = format!("http://127.0.0.1:{pp}/x");
        if method == "POST" && path == "/entrar" {
            if contains(body, b"recusa=1") {
                return (200, vec![], form("", "", ""));
            }
            if contains(body, b"recusa=2") {
                return (200, vec![], with_clobber(&form("", "", "")));
            }
            return (
                200,
                vec![(
                    "Set-Cookie".to_owned(),
                    "sessao=token-falso-da-sessao; Path=/; HttpOnly".to_owned(),
                )],
                "<html><body><h1>Painel</h1></body></html>".to_owned(),
            );
        }
        page(path.trim_start_matches('/'), &outside, pp)
    }));
    let dir = Dir::new(label);
    let (d, _) = test_vault(&dir);
    let rt = Rt::new(label);
    let d = d.with_temp_root(rt.0.clone());
    for (case, _) in CASES {
        let url = format!("http://127.0.0.1:{}/{case}", site.port);
        let name = format!("w-{case}");
        let r = request(
            &d,
            Role::App,
            "salvar_entrada",
            &login_at(&name, &url, LOGIN_PASSWORD),
        );
        assert!(ok(&r), "{r:?}");
        assert!(ok(&set_level(&d, Role::App, &name, "livre", PIN)));
    }
    RealScene {
        d: Arc::new(d),
        _dir: dir,
        rt,
        site,
        phish,
    }
}

impl RealScene {
    fn playwright(&self, entry: &str) -> Resposta {
        let p = req(
            Role::Agent,
            "playwright",
            &format!(r#"{{"entrada":"{entry}"}}"#),
        )
        .unwrap();
        self.d.serve_from(&NoClient, Role::Agent, p)
    }

    /// The phishing site received no `POST` and never the password anywhere.
    fn phish_clean(&self) -> bool {
        self.phish.requests.lock().unwrap().iter().all(|p| {
            p.method != "POST"
                && password_forms()
                    .iter()
                    .all(|f| !contains(&p.body, f) && !contains(p.path.as_bytes(), f))
        })
    }

    /// No request to the site carried the password in the URL.
    fn password_out_of_url(&self) -> bool {
        self.site.requests.lock().unwrap().iter().all(|p| {
            password_forms()
                .iter()
                .all(|f| !contains(p.path.as_bytes(), f))
        })
    }

    fn no_profile(&self) -> bool {
        fs::read_dir(&self.rt.0).map_or(true, |it| {
            !it.flatten()
                .any(|e| e.file_name().to_string_lossy().starts_with("pw-"))
        })
    }
}

/// Real Chrome (D6, D9, S1): the right login saves the session with the site's cookie
/// and without the password, and the `POST` with the password only goes to the entry's
/// site. Attacks: form that submits elsewhere (through `action`, through the button's
/// `formaction` and with `action` "masked" by a field called `action`), redirect
/// elsewhere, `GET` form (the password would go in the URL), submission to another
/// window, two password fields, login refused by the site and site that keeps the
/// password in `localStorage`: each one refused, nothing saved, the phishing site
/// without a `POST` and the password out of every URL. The iframe of another origin
/// receives nothing. DOM clobbering of `document` (Phase 44a): the right login gets in
/// and the login page returned after the submission is still seen (refused).
#[test]
#[ignore = "precisa do Chrome instalado: make test-playwright"]
fn real_chrome_login_and_attacks() {
    let c = real_scene("real");
    for (case, expected) in CASES {
        let entry = format!("w-{case}");
        // Phase 44c: the sites are `http://127.0.0.1:<port>`, so every use goes through
        // the origin confirmation (the fake window confirms).
        let (r, seen) = with_window(&c.d, "confirmar", None, || c.playwright(&entry));
        assert!(
            seen.len() == 1
                && seen[0].confirmacao
                && seen[0].origem.starts_with("http://127.0.0.1:"),
            "{case}: {seen:?}"
        );
        let path = c.rt.0.join(format!("playwright/{entry}.json"));
        match &expected {
            None => {
                assert!(
                    matches!(r, Resposta::SessaoPlaywright { .. }),
                    "{case}: {r:?}"
                );
                let state = fs::read(&path).unwrap();
                assert!(contains(&state, b"token-falso-da-sessao"), "{case}");
                for f in password_forms() {
                    assert!(!contains(&state, &f), "{case}");
                }
            }
            Some(e) => {
                assert_eq!(error(&r).as_ref(), Some(e), "{case}: {r:?}");
                assert!(!path.exists(), "{case}");
            }
        }
        assert!(c.phish_clean(), "{case}: o phishing recebeu algo");
        assert!(c.password_out_of_url(), "{case}: a senha foi na URL");
        assert!(c.no_profile(), "{case}: o perfil ficou");
    }
    // The POSTs with the password went only to the site: one per case that gets to send.
    let posts = c
        .site
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|p| p.method == "POST")
        .count();
    assert_eq!(posts, 6, "ok, dom, dom-recusa, iframe, recusa e ls enviam");
    // The iframe loaded (the case tested something).
    assert!(
        c.phish
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|p| p.path == "/frame")
    );
}

/// Phase 44c (D9), attack from the M6 audit: another process of the uid listens on the
/// origin `http://localhost:<port>` (on `[::1]`, or on the port of a stopped service).
/// On a **free** `http` entry, `ward playwright` only opens Chrome after the origin is
/// confirmed in the window: without an app, "approval unavailable"; denied, nothing;
/// Chrome does not even start. The request shows the origin and does not ask for a
/// PIN. Confirmed, Chrome opens.
#[test]
fn attack_playwright_http_free_only_with_confirmation() {
    let c = scene("http", "ok");
    let q = format!(
        r#"{{"entrada":{{"nome":"h-livre","grupo":"acme","ambiente":"prod","tipo":"login","campos":{{"url":"http://localhost:8081/wp-login.php","usuario":"wp_editor","senha":"{}"}}}}}}"#,
        b64(LOGIN_PASSWORD.as_bytes())
    );
    assert!(ok(&request(&c.d, Role::App, "salvar_entrada", &q)));
    assert!(ok(&set_level(&c.d, Role::App, "h-livre", "livre", PIN)));
    let r = c.playwright("h-livre");
    assert_eq!(error(&r), Some(ResponseError::ApprovalUnavailable));
    assert!(c.events("inicio").is_empty(), "o Chrome abriu sem a janela");
    let (r, seen) = with_window(&c.d, "negar", None, || c.playwright("h-livre"));
    assert_eq!(error(&r), Some(ResponseError::Denied));
    assert!(
        c.events("inicio").is_empty(),
        "o Chrome abriu com a janela negada"
    );
    assert!(!c.session("h-livre").exists());
    let p = &seen[0];
    assert_eq!(p.operacao, ward_core::protocol::Operation::Playwright);
    assert_eq!(p.origem, "http://localhost:8081");
    assert!(p.confirmacao);
    let enc = encoded(Role::App, &Resposta::PedidoDeAprovacao(p.clone()));
    for f in password_forms() {
        assert!(!contains(&enc, &f), "o pedido da janela tem a senha");
    }
    let (_, seen) = with_window(&c.d, "confirmar", None, || c.playwright("h-livre"));
    assert_eq!(seen.len(), 1);
    assert!(
        !c.events("inicio").is_empty(),
        "confirmado, o Chrome não abriu"
    );
    assert!(c.no_profile());
}

/// Phase 44e (Medium of the 2nd M6 audit): `https` on a loopback host asks for the
/// confirmation like `http` (a local CA makes `[::1]` look like the service). Without
/// an app, denied: Chrome does not even open.
#[test]
fn attack_playwright_https_loopback_free_only_with_confirmation() {
    let c = scene("https-loopback", "ok");
    for (i, (url, origin)) in [
        (
            "https://localhost:8443/wp-login.php",
            "https://localhost:8443",
        ),
        ("https://[::1]:8443/wp-login.php", "https://[::1]:8443"),
        ("https://127.0.0.1/wp-login.php", "https://127.0.0.1"),
    ]
    .into_iter()
    .enumerate()
    {
        let name = format!("l-livre-{i}");
        let q = format!(
            r#"{{"entrada":{{"nome":"{name}","grupo":"acme","ambiente":"prod","tipo":"login","campos":{{"url":"{url}","usuario":"wp_editor","senha":"{}"}}}}}}"#,
            b64(LOGIN_PASSWORD.as_bytes())
        );
        assert!(ok(&request(&c.d, Role::App, "salvar_entrada", &q)), "{url}");
        assert!(
            ok(&set_level(&c.d, Role::App, &name, "livre", PIN)),
            "{url}"
        );
        let r = c.playwright(&name);
        assert_eq!(error(&r), Some(ResponseError::ApprovalUnavailable), "{url}");
        let (r, seen) = with_window(&c.d, "negar", None, || c.playwright(&name));
        assert_eq!(error(&r), Some(ResponseError::Denied), "{url}");
        assert!(c.events("inicio").is_empty(), "o Chrome abriu: {url}");
        assert!(!c.session(&name).exists(), "{url}");
        assert_eq!(seen.len(), 1, "{url}");
        assert_eq!(seen[0].origem, origin, "{url}");
        assert!(seen[0].confirmacao, "{url}");
    }
}

/// Free `https` goes on without any window.
#[test]
fn playwright_https_free_without_window() {
    let c = scene("https-livre", "ok");
    let (r, seen) = with_window(&c.d, "negar", None, || c.playwright("t-livre"));
    assert!(matches!(r, Resposta::SessaoPlaywright { .. }), "{r:?}");
    assert!(seen.is_empty());
}
