//! `ward playwright <entrada>` (Fase 44; D6, D9; S1–S3, S8, S13).
//!
//! O daemon faz o login numa entrada `login` com o Chrome instalado e devolve só o
//! caminho do storage state (sessão, nunca a senha):
//!
//! 1. nível e aprovação ([`Daemon::autorizar_uso`]); bloqueada = inexistente;
//! 2. perfil temporário novo em `<rt>pw-<aleatório>/` (0700, tmpfs), e o Chrome em
//!    headless com `--remote-debugging-pipe` (comandos no fd 3, respostas no fd 4; sem
//!    porta TCP), ambiente montado do zero (`HOME` e `XDG_*` dentro do perfil,
//!    `--password-store=basic`: nada do chaveiro nem do home do usuário), argv sem
//!    segredo (S3);
//! 3. contexto de navegação sem disco (`Target.createBrowserContext`, como o anônimo):
//!    cookies, cache e sessão ficam na memória do Chrome;
//! 4. navega até a URL da entrada; todo documento do quadro principal de outra origem
//!    é recusado no navegador (redirecionamento para o phishing não carrega, D9);
//!    confere a origem da página (esquema, host e porta) contra a da entrada;
//! 5. preenche num mundo isolado (os scripts da página não veem nem trocam o código
//!    do Ward): só com um campo de senha visível, formulário `POST` cujo destino é da
//!    mesma origem e sem janela nova; envia;
//! 6. depois da carga, confere de novo a origem (D6) e que a página de login sumiu;
//! 7. lê os cookies do contexto e o `localStorage` da origem, monta o storage state no
//!    formato do Playwright e confere com a máscara da entrada: se a senha (ou uma
//!    variação dela) estiver lá, nada é gravado;
//! 8. grava `<rt>playwright/<entrada>.json` (0600, temporário + `rename`) com o cofre
//!    destravado e sem trava no meio (contagem de travas igual à do começo);
//! 9. mata o Chrome (grupo inteiro) e apaga o perfil **antes** de responder.
//!
//! Os storage states são apagados ao travar ([`apagar_sessoes`], chamado pelo
//! `Estado::travar`) e na partida do daemon ([`limpar`]), com os perfis que um daemon
//! morto deixou.

use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use ward_core::entry::{AccessLevel, Data, url_origin};
use ward_core::mask::Mask;
use ward_core::protocolo::{ErroResposta, Pedido, Resposta};
use ward_core::totp::Totp;
use ward_platform::child::{Command, Group};

use crate::aprovacao::Cliente;
use crate::aviso;
use crate::cdp::{Cdp, ErroCdp, Escritor, Regra, pior_caso};
use crate::nucleo::Daemon;

/// O Chrome instalado (o mesmo do papel extensão, D47).
pub const CHROME: &str = "/opt/google/chrome/chrome";
/// `<rt>playwright/`: os storage states.
pub const PASTA_SESSOES: &str = "playwright";
/// `<rt>pw-<aleatório>/`: o perfil temporário de uma sessão.
pub const PREFIXO_PERFIL: &str = "pw-";
/// Prazo da sessão inteira do navegador (abrir, entrar e ler a sessão).
pub const PRAZO_SESSAO: Duration = Duration::from_secs(60);
/// Prazo de cada carga de página.
const PRAZO_CARGA: Duration = Duration::from_secs(20);
/// `PATH` do Chrome (ele se acha pelo caminho absoluto; isto é para o que ele chama).
const PATH_FILHO: &[u8] = b"/usr/bin:/bin";

/// Preferências do perfil novo: sem gerenciador de senhas e sem preenchimento
/// automático (nada da página vai para os arquivos do perfil).
const PREFERENCIAS: &[u8] = br#"{"credentials_enable_service":false,"profile":{"password_manager_enabled":false},"autofill":{"enabled":false,"profile_enabled":false,"credit_card_enabled":false}}"#;

/// Preenche e envia, no mundo isolado (os scripts da página não alcançam este código
/// nem os protótipos dele). Argumentos: usuário, senha, código TOTP (ou `null`) e a
/// origem da entrada. Volta `"ok"`, `"origem"` ou `"formulario"`.
///
/// DOM clobbering (como em `extension/page.js`): o `document` e o `<form>` têm
/// propriedades com nome vindas da página (um `<img name="querySelectorAll">` troca o
/// `document.querySelectorAll`; um `<input name="action">`, o `form.action`). No Chrome
/// de hoje as do `<form>` chegam ao mundo isolado e as do `document` não (conferido na
/// Fase 44a), mas o script não depende disso: busca, getters e métodos saem dos
/// protótipos do mundo isolado (`Document.prototype.querySelectorAll.call(document, …)`,
/// descritores de `HTMLFormElement`/`HTMLInputElement`/`HTMLButtonElement`,
/// `Element.prototype`).
const PREENCHER: &str = r#"function (usuario, senha, codigo, origem) {
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

/// Depois do login: se ainda há campo de senha visível (a página de login voltou) e o
/// `localStorage` da origem, em JSON. Também pelos protótipos (DOM clobbering: um
/// `<img name="querySelectorAll">` na página depois do envio não esconde a senha).
const LER_PAGINA: &str = r#"function () {
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

// --- Arquivos (S2) -----------------------------------------------------------------------

/// `<rt>playwright/`.
pub fn pasta_sessoes(rt: &Path) -> PathBuf {
    rt.join(PASTA_SESSOES)
}

/// Apaga os storage states (`<rt>playwright/*`). Chamado ao travar. Não segue link: a
/// pasta é aberta com `O_DIRECTORY | O_NOFOLLOW` (um link plantado no lugar de
/// `<rt>playwright` não abre, e nada é apagado do alvo dele) e o conteúdo é listado e
/// apagado pelo fd (Fase 44e; `ward_platform::dir::delete_contents`): trocar a pasta
/// por um link depois de aberta não muda o que é apagado. Cada item é apagado como ele
/// é (um link vira só o link apagado).
pub fn apagar_sessoes(rt: &Path) -> usize {
    use std::os::fd::AsFd;
    let pasta = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(pasta_sessoes(rt))
    {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return 0,
        Err(_) => {
            aviso!("playwright: a pasta das sessões não é uma pasta; nada apagado nela");
            return 0;
        }
    };
    ward_platform::dir::delete_contents(pasta.as_fd())
}

/// Partida do daemon (D6): apaga os storage states e os perfis `pw-*` que um daemon
/// morto deixou. Devolve quantos itens apagou.
pub fn limpar(rt: &Path) -> io::Result<usize> {
    let mut n = apagar_sessoes(rt);
    let itens = match fs::read_dir(rt) {
        Ok(i) => i,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(n),
        Err(e) => return Err(e),
    };
    for item in itens.flatten() {
        let nome = item.file_name();
        if nome
            .as_encoded_bytes()
            .starts_with(PREFIXO_PERFIL.as_bytes())
            && item.file_type().is_ok_and(|t| t.is_dir())
        {
            fs::remove_dir_all(item.path())?;
            n += 1;
        }
    }
    Ok(n)
}

/// O Chrome da sessão e o perfil: ao sair, mata o grupo inteiro, colhe e apaga o perfil.
struct Navegador {
    grupo: Option<Group>,
    perfil: PathBuf,
}

impl Drop for Navegador {
    fn drop(&mut self) {
        if let Some(g) = self.grupo.take() {
            g.kill();
            let _ = g.collect();
        }
        // `remove_dir_all` não segue link simbólico.
        if fs::remove_dir_all(&self.perfil).is_err() && self.perfil.exists() {
            aviso!("playwright: não deu para apagar o perfil temporário");
        }
    }
}

/// O argv do Chrome: só opções fixas e o caminho do perfil (nunca segredo, S3).
pub fn argv_chrome(chrome: &Path, perfil: &Path) -> Option<Vec<Vec<u8>>> {
    use std::os::unix::ffi::OsStrExt;
    let mut dir = b"--user-data-dir=".to_vec();
    dir.extend_from_slice(perfil.as_os_str().as_bytes());
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
        // Fase 44c (D9): `localhost` sempre em 127.0.0.1. Sem isto o Chrome tenta `[::1]`
        // antes, e outro processo do uid que escute ali (com o serviço só em 127.0.0.1)
        // recebe a senha numa origem que confere.
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

/// Cria o perfil e abre o Chrome com o pipe de depuração.
fn abrir_navegador(chrome: &Path, rt: &Path) -> Result<(Navegador, Cdp), ErroResposta> {
    use std::os::unix::ffi::OsStrExt;
    let falhou = |_| ErroResposta::NavegadorFalhou;
    ward_platform::prepare_dir(rt).map_err(|_| ErroResposta::EntregaFalhou)?;
    let sufixo = ward_core::generator::connect_token().map_err(|_| ErroResposta::Interno)?;
    let sufixo = std::str::from_utf8(&sufixo.expose()[..16]).map_err(|_| ErroResposta::Interno)?;
    let perfil = rt.join(format!("{PREFIXO_PERFIL}{sufixo}"));
    // Pasta nova: nunca reaproveita uma que já existe.
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&perfil)
        .map_err(falhou)?;
    let mut nav = Navegador {
        grupo: None,
        perfil: perfil.clone(),
    };
    // Nomes curtos: o Chrome cria o `SingletonSocket` dentro do `TMPDIR`, e o caminho de
    // um socket Unix tem no máximo 107 bytes.
    for sub in ["h", "c", "k", "d", "t", "Default"] {
        fs::DirBuilder::new()
            .mode(0o700)
            .create(perfil.join(sub))
            .map_err(falhou)?;
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(perfil.join("Default/Preferences"))
        .and_then(|mut f| f.write_all(PREFERENCIAS))
        .map_err(falhou)?;
    let argv = argv_chrome(chrome, &perfil).ok_or(ErroResposta::NavegadorFalhou)?;
    let p = perfil.as_os_str().as_bytes();
    let mut c = Command::new(&argv[0], p).map_err(|_| ErroResposta::NavegadorFalhou)?;
    for a in &argv[1..] {
        c.argument(a).map_err(|_| ErroResposta::NavegadorFalhou)?;
    }
    let junto = |s: &[u8]| [p, s].concat();
    let ambiente: [(&[u8], Vec<u8>); 8] = [
        (b"PATH", PATH_FILHO.to_vec()),
        (b"HOME", junto(b"/h")),
        (b"XDG_CONFIG_HOME", junto(b"/c")),
        (b"XDG_CACHE_HOME", junto(b"/k")),
        (b"XDG_DATA_HOME", junto(b"/d")),
        (b"TMPDIR", junto(b"/t")),
        (b"LANG", b"C.UTF-8".to_vec()),
        // Sem barramento de sessão: nada do chaveiro nem de serviços do usuário.
        (b"DBUS_SESSION_BUS_ADDRESS", b"disabled:".to_vec()),
    ];
    for (k, v) in &ambiente {
        c.variable(k, v)
            .map_err(|_| ErroResposta::NavegadorFalhou)?;
    }
    let (grupo, pipe) = c.launch_with_debugging().map_err(|e| {
        aviso!("playwright: o Chrome não abriu ({})", e.message());
        ErroResposta::NavegadorFalhou
    })?;
    nav.grupo = Some(grupo);
    let cdp = Cdp::novo(pipe.commands, pipe.responses, Instant::now() + PRAZO_SESSAO);
    Ok((nav, cdp))
}

// --- Sessão no navegador (D6, D9) -------------------------------------------------------

/// O que vai à página: usuário, senha e código TOTP (se a entrada tem semente).
struct Credencial {
    usuario: String,
    senha: Zeroizing<String>,
    codigo: Option<Zeroizing<String>>,
}

#[derive(Deserialize)]
struct Contexto {
    #[serde(rename = "browserContextId")]
    id: String,
}

#[derive(Deserialize)]
struct Alvo {
    #[serde(rename = "targetId")]
    id: String,
}

#[derive(Deserialize)]
struct Anexo {
    #[serde(rename = "sessionId")]
    sessao: String,
}

#[derive(Deserialize)]
struct Arvore {
    #[serde(rename = "frameTree")]
    arvore: NoQuadro,
}

#[derive(Deserialize)]
struct NoQuadro {
    frame: Quadro,
}

#[derive(Deserialize)]
struct Quadro {
    id: String,
    url: String,
    #[serde(rename = "securityOrigin", default)]
    origem: String,
}

#[derive(Deserialize)]
struct Navegacao {
    #[serde(rename = "errorText")]
    erro: Option<serde::de::IgnoredAny>,
}

#[derive(Deserialize)]
struct Mundo {
    #[serde(rename = "executionContextId")]
    id: u64,
}

#[derive(Deserialize)]
struct Avaliacao {
    result: ValorRemoto,
    #[serde(rename = "exceptionDetails")]
    excecao: Option<serde::de::IgnoredAny>,
}

#[derive(Deserialize)]
struct ValorRemoto {
    value: Option<String>,
}

#[derive(Deserialize)]
struct Pagina {
    senha: bool,
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

/// Storage state no formato do Playwright (`browser.new_context(storage_state=…)`).
#[derive(Serialize)]
struct EstadoArmazenado<'a> {
    cookies: Vec<CookiePw<'a>>,
    origins: Vec<OrigemPw<'a>>,
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
struct OrigemPw<'a> {
    origin: &'a str,
    local_storage: Vec<ItemPw<'a>>,
}

#[derive(Serialize)]
struct ItemPw<'a> {
    name: &'a str,
    value: &'a str,
}

fn erro_cdp(cdp: &Cdp, e: ErroCdp) -> ErroResposta {
    if cdp.bloqueou {
        return ErroResposta::OrigemDiferente;
    }
    aviso!(
        "playwright: a conversa com o navegador acabou ({})",
        match e {
            ErroCdp::Prazo => "prazo",
            ErroCdp::Fechado => "fechado",
            ErroCdp::Protocolo => "protocolo",
            ErroCdp::Comando => "comando recusado",
            ErroCdp::Io => "leitura ou escrita",
        }
    );
    ErroResposta::NavegadorFalhou
}

/// Um objeto JSON de parâmetros com pares de texto.
fn params(pares: &[(&str, &str)]) -> Result<Zeroizing<Vec<u8>>, ErroResposta> {
    let cap = 2 + pares
        .iter()
        .map(|(k, v)| pior_caso(k) + pior_caso(v) + 2)
        .sum::<usize>();
    let mut e = Escritor::novo(cap);
    e.abre_objeto();
    for (k, v) in pares {
        e.chave(k);
        e.texto(v);
    }
    e.fecha_objeto();
    e.fim().ok_or(ErroResposta::Interno)
}

/// O quadro principal: id, e se a URL e a origem dele são as da entrada.
fn origem_confere(cdp: &mut Cdp, origem: &str) -> Result<(String, bool), ErroResposta> {
    let a: Arvore = cdp
        .comando_de("Page.getFrameTree", b"{}", true)
        .map_err(|e| erro_cdp(cdp, e))?;
    let q = a.arvore.frame;
    let url = Zeroizing::new(q.url);
    let ok = url_origin(&url).is_some_and(|o| o == origem) && q.origem == origem;
    Ok((q.id, ok))
}

/// Uma função no mundo isolado do quadro principal; volta o texto que ela devolveu.
fn no_mundo_isolado(
    cdp: &mut Cdp,
    quadro: &str,
    funcao: &str,
    argumentos: &[Option<&str>],
) -> Result<Zeroizing<String>, ErroResposta> {
    let p = params(&[("frameId", quadro), ("worldName", "ward")])?;
    let m: Mundo = cdp
        .comando_de("Page.createIsolatedWorld", &p, true)
        .map_err(|e| erro_cdp(cdp, e))?;
    let cap = 256
        + pior_caso(funcao)
        + argumentos
            .iter()
            .map(|a| a.map_or(16, pior_caso) + 16)
            .sum::<usize>();
    let mut e = Escritor::novo(cap);
    e.abre_objeto();
    e.chave("functionDeclaration");
    e.texto(funcao);
    e.chave("executionContextId");
    e.inteiro(m.id);
    e.chave("arguments");
    e.abre_lista();
    for a in argumentos {
        e.abre_objeto();
        e.chave("value");
        match a {
            Some(v) => e.texto(v),
            None => e.nulo(),
        }
        e.fecha_objeto();
    }
    e.fecha_lista();
    e.chave("returnByValue");
    e.logico(true);
    e.chave("silent");
    e.logico(true);
    e.fecha_objeto();
    let p = e.fim().ok_or(ErroResposta::Interno)?;
    let msg = cdp
        .comando("Runtime.callFunctionOn", &p, true)
        .map_err(|e| erro_cdp(cdp, e))?;
    drop(p);
    #[derive(Deserialize)]
    struct R {
        result: Avaliacao,
    }
    let r: R = serde_json::from_slice(&msg).map_err(|_| ErroResposta::NavegadorFalhou)?;
    if r.result.excecao.is_some() {
        return Err(ErroResposta::LoginRecusado);
    }
    Ok(Zeroizing::new(
        r.result.result.value.ok_or(ErroResposta::LoginRecusado)?,
    ))
}

/// Os passos 3 a 7 do módulo: entra e devolve o storage state em JSON.
fn entrar(
    cdp: &mut Cdp,
    url: &str,
    origem: &str,
    cred: &Credencial,
) -> Result<Zeroizing<Vec<u8>>, ErroResposta> {
    let ctx: Contexto = cdp
        .comando_de(
            "Target.createBrowserContext",
            b"{\"disposeOnDetach\":true}",
            false,
        )
        .map_err(|e| erro_cdp(cdp, e))?;
    let p = params(&[("url", "about:blank"), ("browserContextId", &ctx.id)])?;
    let alvo: Alvo = cdp
        .comando_de("Target.createTarget", &p, false)
        .map_err(|e| erro_cdp(cdp, e))?;
    let mut e = Escritor::novo(64 + pior_caso(&alvo.id));
    e.abre_objeto();
    e.chave("targetId");
    e.texto(&alvo.id);
    e.chave("flatten");
    e.logico(true);
    e.fecha_objeto();
    let p = e.fim().ok_or(ErroResposta::Interno)?;
    let anexo: Anexo = cdp
        .comando_de("Target.attachToTarget", &p, false)
        .map_err(|e| erro_cdp(cdp, e))?;
    cdp.sessao = Some(anexo.sessao);
    let p = params(&[("behavior", "deny"), ("browserContextId", &ctx.id)])?;
    cdp.comando("Browser.setDownloadBehavior", &p, false)
        .map_err(|e| erro_cdp(cdp, e))?;
    cdp.comando("Page.enable", b"{}", true)
        .map_err(|e| erro_cdp(cdp, e))?;
    let (quadro, _) = origem_confere(cdp, origem)?;
    cdp.regra = Some(Regra {
        origem: origem.to_owned(),
        quadro: quadro.clone(),
    });
    cdp.comando(
        "Fetch.enable",
        br#"{"patterns":[{"urlPattern":"*","resourceType":"Document","requestStage":"Request"}]}"#,
        true,
    )
    .map_err(|e| erro_cdp(cdp, e))?;
    // Página de login.
    let antes = cdp.cargas;
    let p = params(&[("url", url)])?;
    let nav: Navegacao = cdp
        .comando_de("Page.navigate", &p, true)
        .map_err(|e| erro_cdp(cdp, e))?;
    if cdp.bloqueou {
        return Err(ErroResposta::OrigemDiferente);
    }
    if nav.erro.is_some() {
        aviso!("playwright: a página da entrada não abriu");
        return Err(ErroResposta::NavegadorFalhou);
    }
    let carregou = cdp
        .esperar_carga(antes, Instant::now() + PRAZO_CARGA)
        .map_err(|e| erro_cdp(cdp, e))?;
    if cdp.bloqueou {
        return Err(ErroResposta::OrigemDiferente);
    }
    if !carregou {
        return Err(ErroResposta::NavegadorFalhou);
    }
    let (q, ok) = origem_confere(cdp, origem)?;
    if !ok || q != quadro {
        aviso!("playwright: a página não é da origem da entrada (D9)");
        return Err(ErroResposta::OrigemDiferente);
    }
    // Preenche e envia.
    let antes = cdp.cargas;
    let r = no_mundo_isolado(
        cdp,
        &quadro,
        PREENCHER,
        &[
            Some(cred.usuario.as_str()),
            Some(cred.senha.as_str()),
            cred.codigo.as_deref().map(String::as_str),
            Some(origem),
        ],
    )?;
    match r.as_str() {
        "ok" => {}
        "origem" => {
            aviso!("playwright: o formulário envia para outra origem (D9)");
            return Err(ErroResposta::OrigemDiferente);
        }
        _ => {
            aviso!("playwright: a página não tem um formulário de login seguro");
            return Err(ErroResposta::LoginRecusado);
        }
    }
    let carregou = cdp
        .esperar_carga(antes, Instant::now() + PRAZO_CARGA)
        .map_err(|e| erro_cdp(cdp, e))?;
    if cdp.bloqueou {
        return Err(ErroResposta::OrigemDiferente);
    }
    if !carregou {
        aviso!("playwright: nenhuma página nova depois do envio");
        return Err(ErroResposta::LoginRecusado);
    }
    // D6: a origem final continua a da entrada.
    let (q, ok) = origem_confere(cdp, origem)?;
    if !ok {
        aviso!("playwright: a página final não é da origem da entrada (D9)");
        return Err(ErroResposta::OrigemDiferente);
    }
    let pagina = no_mundo_isolado(cdp, &q, LER_PAGINA, &[])?;
    let pagina: Pagina =
        serde_json::from_str(&pagina).map_err(|_| ErroResposta::NavegadorFalhou)?;
    if pagina.senha {
        aviso!("playwright: a página de login continua aberta (login recusado)");
        return Err(ErroResposta::LoginRecusado);
    }
    let p = params(&[("browserContextId", &ctx.id)])?;
    let c: Cookies = cdp
        .comando_de("Storage.getCookies", &p, false)
        .map_err(|e| erro_cdp(cdp, e))?;
    montar_estado(&c.cookies, origem, &pagina.ls)
}

/// O storage state em JSON (formato do Playwright).
fn montar_estado(
    cookies: &[CookieCdp],
    origem: &str,
    ls: &[(String, String)],
) -> Result<Zeroizing<Vec<u8>>, ErroResposta> {
    let mut lista = Vec::with_capacity(cookies.len());
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
        lista.push(CookiePw {
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
        origins.push(OrigemPw {
            origin: origem,
            local_storage: ls
                .iter()
                .map(|(k, v)| ItemPw { name: k, value: v })
                .collect(),
        });
    }
    let estado = EstadoArmazenado {
        cookies: lista,
        origins,
    };
    serde_json::to_vec_pretty(&estado)
        .map(Zeroizing::new)
        .map_err(|_| ErroResposta::Interno)
}

/// S1/V6.13: o storage state não pode ter a senha nem variação dela (pela máscara da
/// entrada: literal, URL, base64, formas de texto).
pub fn tem_segredo(mascara: &Mask, estado: &[u8]) -> bool {
    let mascarado = Zeroizing::new(mascara.mask_all(estado));
    mascarado.as_slice() != estado
}

/// Grava `<rt>playwright/<entrada>.json` (0600) por temporário + `rename`.
fn gravar_estado(rt: &Path, entrada: &str, estado: &[u8]) -> io::Result<PathBuf> {
    let nome_ok = !entrada.is_empty()
        && !entrada.starts_with('.')
        && entrada.bytes().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
        });
    if !nome_ok {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let erro = |_| io::Error::from(io::ErrorKind::PermissionDenied);
    ward_platform::prepare_dir(rt).map_err(erro)?;
    let pasta = pasta_sessoes(rt);
    ward_platform::prepare_dir(&pasta).map_err(erro)?;
    let sufixo =
        ward_core::generator::connect_token().map_err(|_| io::Error::other("aleatório"))?;
    let sufixo =
        std::str::from_utf8(&sufixo.expose()[..12]).map_err(|_| io::Error::other("aleatório"))?;
    let tmp = pasta.join(format!(".{entrada}.{sufixo}.tmp"));
    let destino = pasta.join(format!("{entrada}.json"));
    let r = (|| {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&tmp)?;
        f.write_all(estado)?;
        f.sync_all()?;
        fs::rename(&tmp, &destino)
    })();
    if r.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    r.map(|()| destino)
}

impl Daemon {
    /// `ward playwright <entrada>` (D6): a resposta é só o caminho do storage state.
    pub(crate) fn playwright(
        &self,
        cliente: &dyn Cliente,
        pedido: &Pedido,
    ) -> Result<Resposta, ErroResposta> {
        let Pedido::Playwright { entrada } = pedido else {
            return Err(ErroResposta::PedidoInvalido);
        };
        // Tipo sem login visível: recusa antes de pedir aprovação.
        if let Some(a) = self.estado().aberto.as_ref()
            && a.entradas
                .iter()
                .find(|e| e.name() == entrada && e.level() != AccessLevel::Blocked)
                .is_some_and(|e| !matches!(e.data(), Data::Login(_)))
        {
            return Err(ErroResposta::TipoSemEntrega);
        }
        self.autorizar_uso(cliente, pedido)?;
        self.registrar_uso();
        let rt = self
            .temporarios()
            .ok_or(ErroResposta::EntregaFalhou)?
            .to_path_buf();
        let (url, cred, mascara, travas, inicial) = {
            let est = self.estado();
            let a = est.aberto.as_ref().ok_or(ErroResposta::Travado)?;
            // A entrada pode ter mudado durante a aprovação: confere de novo.
            let e = a
                .entradas
                .iter()
                .find(|e| e.name() == entrada && e.level() != AccessLevel::Blocked)
                .ok_or(ErroResposta::EntradaInexistente)?;
            let Data::Login(l) = e.data() else {
                return Err(ErroResposta::TipoSemEntrega);
            };
            let senha = l
                .password
                .expose_text()
                .ok_or(ErroResposta::EntregaFalhou)?;
            let mut s = Zeroizing::new(String::with_capacity(senha.len()));
            s.push_str(senha);
            let codigo = match &l.totp_seed {
                Some(semente) => {
                    let c = Totp::from_seed(semente)
                        .and_then(|t| t.code_now())
                        .map_err(|_| ErroResposta::EntregaFalhou)?;
                    let mut z = Zeroizing::new(String::with_capacity(c.code().len()));
                    z.push_str(c.code());
                    Some(z)
                }
                None => None,
            };
            let mascara = Mask::from_entry(e, None).map_err(|_| ErroResposta::EntregaFalhou)?;
            let (travas, inicial) = self.travas_agora(&est);
            (
                l.url.clone(),
                Credencial {
                    usuario: l.username.clone(),
                    senha: s,
                    codigo,
                },
                mascara,
                travas,
                inicial,
            )
        };
        let origem = url_origin(&url).ok_or(ErroResposta::EntregaFalhou)?;
        aviso!("playwright: abrindo o navegador");
        let (navegador, mut cdp) = abrir_navegador(self.chrome(), &rt)?;
        let resultado = entrar(&mut cdp, &url, &origem, &cred);
        drop(cred);
        // O Chrome morre e o perfil some antes da resposta (D6).
        drop(cdp);
        drop(navegador);
        let estado = resultado?;
        if tem_segredo(&mascara, &estado) {
            aviso!("playwright: a sessão do site guarda a senha; nada foi gravado (S1)");
            return Err(ErroResposta::SessaoComSegredo);
        }
        // Grava com o cofre destravado e sem trava desde o começo; travar depois apaga.
        let est = self.estado();
        if est.aberto.is_none() || travas.load(std::sync::atomic::Ordering::Acquire) != inicial {
            return Err(ErroResposta::Travado);
        }
        let caminho = gravar_estado(&rt, entrada, &estado).map_err(|e| {
            aviso!("playwright: não deu para gravar a sessão ({})", e.kind());
            ErroResposta::EntregaFalhou
        })?;
        drop(est);
        let caminho = caminho
            .into_os_string()
            .into_string()
            .map_err(|_| ErroResposta::Interno)?;
        aviso!("playwright: sessão gravada");
        Ok(Resposta::SessaoPlaywright { caminho })
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn argv_so_fixo_com_pipe_e_sem_porta() {
        let v = argv_chrome(Path::new(CHROME), Path::new("/run/user/1000/ward/pw-x")).unwrap();
        let t: Vec<String> = v
            .iter()
            .map(|a| String::from_utf8(a.clone()).unwrap())
            .collect();
        assert!(t.contains(&"--remote-debugging-pipe".to_owned()));
        assert!(t.contains(&"--headless".to_owned()));
        assert!(t.contains(&"--password-store=basic".to_owned()));
        assert!(t.iter().all(|a| !a.contains("remote-debugging-port")));
        assert!(t.iter().all(|a| !a.contains("no-sandbox")));
        // Fase 44c (D9): `localhost` nunca vai a `[::1]`.
        assert!(t.contains(&"--host-resolver-rules=MAP localhost 127.0.0.1".to_owned()));
        assert_eq!(t.last().unwrap(), "about:blank");
    }

    /// Fase 44c (achado baixo da auditoria do M6): `<rt>playwright` trocado por um link
    /// para outra pasta do uid. Travar não apaga nada do outro lado; um link dentro da
    /// pasta das sessões some sem levar o alvo.
    #[test]
    fn ataque_apagar_sessoes_nao_segue_link() {
        use std::os::unix::fs::symlink;
        let base = std::env::temp_dir().join(format!("ward-pw-link-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let rt = base.join("rt");
        let alvo = base.join("alvo");
        fs::create_dir_all(&rt).unwrap();
        fs::create_dir_all(alvo.join("sub")).unwrap();
        fs::write(alvo.join("precioso.txt"), b"nao apagar").unwrap();
        fs::write(alvo.join("sub/outro.txt"), b"nao apagar").unwrap();
        symlink(&alvo, pasta_sessoes(&rt)).unwrap();
        assert_eq!(apagar_sessoes(&rt), 0);
        assert!(alvo.join("precioso.txt").exists());
        assert!(alvo.join("sub/outro.txt").exists());
        // O `limpar` da partida também não segue o link.
        assert_eq!(limpar(&rt).unwrap(), 0);
        assert!(alvo.join("precioso.txt").exists());

        // Pasta de verdade com um link dentro: o link some, o alvo fica.
        fs::remove_file(pasta_sessoes(&rt)).unwrap();
        fs::create_dir(pasta_sessoes(&rt)).unwrap();
        fs::write(pasta_sessoes(&rt).join("acme-wp-prod.json"), b"{}").unwrap();
        symlink(&alvo, pasta_sessoes(&rt).join("ligacao")).unwrap();
        assert_eq!(apagar_sessoes(&rt), 2);
        assert!(!pasta_sessoes(&rt).join("acme-wp-prod.json").exists());
        assert!(fs::symlink_metadata(pasta_sessoes(&rt).join("ligacao")).is_err());
        assert!(alvo.join("precioso.txt").exists());
        assert!(alvo.join("sub/outro.txt").exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn estado_no_formato_do_playwright() {
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
        let b = montar_estado(&c, "http://localhost:8081", &ls).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&b).unwrap();
        assert_eq!(v["cookies"][0]["httpOnly"], true);
        assert_eq!(v["cookies"][0]["sameSite"], "Lax");
        assert_eq!(v["cookies"][0]["expires"], -1.0);
        assert_eq!(v["origins"][0]["origin"], "http://localhost:8081");
        assert_eq!(v["origins"][0]["localStorage"][0]["name"], "k");
        let b = montar_estado(&c, "http://localhost:8081", &[]).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&b).unwrap();
        assert_eq!(v["origins"].as_array().unwrap().len(), 0);
    }

    /// DOM clobbering (Fase 44a): os scripts do mundo isolado não leem nada pelo
    /// `document`, pelo `<form>` nem pelo `localStorage` (que têm propriedades com nome
    /// vindas da página); tudo sai dos protótipos. O ataque com o Chrome de verdade está
    /// em `tests/playwright.rs` (casos `dom` e `dom-recusa`).
    #[test]
    fn ataque_dom_clobbering_scripts_so_pelos_prototipos() {
        for (nome, js) in [("PREENCHER", PREENCHER), ("LER_PAGINA", LER_PAGINA)] {
            for proibido in [
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
                assert!(!js.contains(proibido), "{nome} lê `{proibido}` direto");
            }
            assert!(
                js.contains("Document.prototype.querySelectorAll.call(document"),
                "{nome}"
            );
        }
    }

    /// Ataque: nome de entrada com `/` ou começando por `.` não sai da pasta.
    #[test]
    fn ataque_nome_fora_da_pasta_recusado() {
        let d = std::env::temp_dir().join(format!("ward-pw-teste-{}", std::process::id()));
        for ruim in ["../x", "a/b", ".x", "", "A"] {
            assert!(gravar_estado(&d, ruim, b"{}").is_err(), "{ruim}");
        }
        let _ = fs::remove_dir_all(&d);
    }
}
