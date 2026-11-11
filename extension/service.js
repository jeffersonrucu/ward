// Extension service worker (Phase 42; V6.8; S1, S13): keeps, only in memory and for a
// short time, the login the content script (`submit.js`) saw being submitted, and saves
// it to the vault when the user confirms in the popup.
//
// - The password never leaves here for the popup: the popup gets only the origin and
//   the username, and on "Save" it sends name, username, group and environment; this
//   service worker is the one that adds the password and sends the save-login operation
//   to `ward-nm`.
// - Nothing is kept on disk (no `storage` permission, no page storage); the pending
//   login goes away on save, on "Not now", when the tab closes, after 3 minutes or when
//   Chrome stops the service worker.
// - Who sends each message comes from the `sender` Chrome fills in, never from the
//   content: the submit only counts from the content script of the main frame of an
//   http/https tab; the pending query, the save and the discard only count from the
//   extension's own `popup.html`. No page talks to the extension (no
//   `externally_connectable`, V6.6).
// - It only offers a login the vault does not have: the daemon answers the suggest
//   operation for the origin, and the submitted username cannot be among the
//   suggestions. With the vault locked (or without Ward), the submit is dropped at once.
// - Phase 43: `ward login` lives in `login.js` (native messaging port opened with
//   `ward-nm`, which also keeps this service worker alive).
// - Phase 44c (D9): the popup fill on an `http` origin (`localhost`/`127.0.0.1`;
//   Phase 44e: and on every host of this machine in `https`, `needsWindow`) waits for
//   the user to confirm the origin in the Ward window (up to 2 min). The popup closes
//   when the window takes the focus, so this service worker is the one that waits and
//   fills: the popup sends the tab, the `documentId` its probe found, the entry and the
//   origin; the daemon reply goes straight as an argument to the fill in that document
//   and is wiped right after. Nothing goes back to the popup but "ok" or the error code.

import { startLogin } from './login.js';
import { needsWindow, fillInPage } from './page.js';

const HOST = 'br.ward.nm';
const POPUP = chrome.runtime.getURL('popup.html');
const TIMEOUT_MS = 3 * 60 * 1000;
// D25: username and password up to 2 KiB; name, group and the rest the daemon validates.
const MAX = 2048;
const ENVIRONMENTS = new Set(['dev', 'stage', 'prod']);

/** tabId → { origin, url, username, password, timer }. One per tab; the new one replaces
 * the old. */
const pendingLogins = new Map();

function discard(tab) {
  const p = pendingLogins.get(tab);
  if (!p) return;
  clearTimeout(p.timer);
  p.password = '';
  p.username = '';
  pendingLogins.delete(tab);
}

const isText = (v, max) =>
  typeof v === 'string' && v.length > 0 && v.length <= max && !v.includes('\0');

async function send(request) {
  try {
    const r = await chrome.runtime.sendNativeMessage(HOST, request);
    return r && typeof r === 'object' ? r : null;
  } catch {
    return null;
  }
}

/** http/https origin of a URL, or `null`. */
function originOf(url) {
  try {
    const u = new URL(url);
    if (u.protocol !== 'https:' && u.protocol !== 'http:') return null;
    return u.origin;
  } catch {
    return null;
  }
}

/** The entry URL: origin and path of the submitted document, without query or
 * fragment. */
function entryUrl(url) {
  const u = new URL(url);
  return u.origin + u.pathname;
}

/** Did the submit come from the content script of the main frame of an http/https tab? */
function fromContentScript(sender) {
  if (sender.id !== chrome.runtime.id || !sender.tab || sender.tab.incognito) return null;
  if (sender.frameId !== 0 || !Number.isInteger(sender.tab.id) || sender.tab.id < 0) return null;
  if (sender.documentLifecycle !== undefined && sender.documentLifecycle !== 'active') return null;
  const origin = originOf(sender.url);
  if (!origin || sender.origin !== origin) return null;
  return { tab: sender.tab.id, window: sender.tab.windowId, origin, url: entryUrl(sender.url) };
}

/** Did the message come from the extension's own popup (an extension page, no tab)? */
function fromPopup(sender) {
  return sender.id === chrome.runtime.id && !sender.tab && sender.url === POPUP;
}

async function onSubmitted(msg, from) {
  if (!isText(msg.usuario, MAX) || !isText(msg.senha, MAX)) return;
  const r = await send({ v: 1, op: 'sugerir', 'dados': { 'origem': from.origin } });
  if (!r || r.tipo !== 'sugestoes' || !r.dados || !Array.isArray(r.dados.itens)) return;
  if (r.dados.itens.some((s) => s && s.usuario === msg.usuario)) return;
  discard(from.tab);
  const p = {
    origin: from.origin,
    url: from.url,
    username: msg.usuario,
    password: msg.senha,
    timer: setTimeout(() => discard(from.tab), TIMEOUT_MS),
  };
  pendingLogins.set(from.tab, p);
  try {
    await chrome.action.openPopup({ windowId: from.window });
  } catch {
    // No focused window: the popup shows the pending login when the user clicks the icon.
  }
}

/** How long to wait for the reply of the fill with the window (the daemon waits 2 min). */
const WINDOW_TIMEOUT_MS = 130 * 1000;

/**
 * A request to the host over a port of its own: `connectNative` keeps the service worker
 * alive while the approval window waits. `null` if the host did not reply.
 */
function requestByPort(request) {
  return new Promise((ok) => {
    let p;
    try {
      p = chrome.runtime.connectNative(HOST);
    } catch {
      ok(null);
      return;
    }
    let done = false;
    let timeout = null;
    const finish = (r) => {
      if (done) return;
      done = true;
      clearTimeout(timeout);
      try {
        p.disconnect();
      } catch {
        // already closed
      }
      ok(r);
    };
    timeout = setTimeout(() => finish(null), WINDOW_TIMEOUT_MS);
    p.onMessage.addListener((m) => finish(m && typeof m === 'object' ? m : null));
    p.onDisconnect.addListener(() => {
      void chrome.runtime.lastError;
      finish(null);
    });
    try {
      p.postMessage(request);
    } catch {
      finish(null);
    }
  });
}

/** Phase 44c: the popup fill on an `http` origin (Phase 44e: or on a host of this
 * machine in `https`), with the Ward window. */
async function fillWithWindow(msg) {
  const origin = typeof msg.origem === 'string' ? originOf(msg.origem) : null;
  if (
    !isText(msg.entrada, MAX) ||
    typeof msg.documento !== 'string' ||
    msg.documento.length === 0 ||
    msg.documento.length > MAX ||
    !origin ||
    origin !== msg.origem ||
    !needsWindow(origin)
  ) {
    return { 'erro': 'pedido_invalido' };
  }
  let r = await requestByPort({ v: 1, op: 'preencher', 'dados': { 'entrada': msg.entrada, 'origem': origin } });
  if (!r || r.tipo !== 'preenchimento' || !r.dados) {
    const code = r && r.tipo === 'erro' && r.dados ? String(r.dados.codigo) : 'sem_resposta';
    return { 'erro': code };
  }
  const d = r.dados;
  const args = [origin, String(d.usuario ?? ''), String(d.senha ?? ''), String(d.totp ?? '')];
  d.senha = '';
  d.totp = '';
  d.usuario = '';
  r = null;
  let done = null;
  try {
    [done] = await chrome.scripting.executeScript({
      target: { tabId: msg.aba, documentIds: [msg.documento] },
      world: 'ISOLATED',
      func: fillInPage,
      args,
    });
  } catch {
    done = null;
  } finally {
    args.fill('');
  }
  const v = done && done.result;
  if (v && v.ok === true) return { ok: true };
  return { 'erro': (v && typeof v.reason === 'string' && v.reason) || 'origem' };
}

/** Base64 of the UTF-8 bytes (how the protocol carries the secrets). */
function base64(s) {
  const bytes = new TextEncoder().encode(s);
  let bin = '';
  for (const b of bytes) bin += String.fromCharCode(b);
  bytes.fill(0);
  return btoa(bin);
}

async function save(msg) {
  const p = pendingLogins.get(msg.aba);
  if (!p) return { 'erro': 'expirou' };
  if (
    !isText(msg.nome, MAX) ||
    !isText(msg.usuario, MAX) ||
    typeof msg.grupo !== 'string' ||
    msg.grupo.length > MAX ||
    !ENVIRONMENTS.has(msg.ambiente)
  ) {
    return { 'erro': 'pedido_invalido' };
  }
  const request = {
    v: 1,
    op: 'salvar_login',
    'dados': {
      'entrada': {
        'nome': msg.nome,
        'grupo': msg.grupo,
        'ambiente': msg.ambiente,
        'tipo': 'login',
        'campos': { url: p.url, 'usuario': msg.usuario, 'senha': base64(p.password) },
      },
    },
  };
  const r = await send(request);
  request.dados.entrada.campos.senha = '';
  if (r && r.tipo === 'ok') {
    discard(msg.aba);
    return { ok: true };
  }
  const d = r && r.tipo === 'erro' && r.dados ? r.dados : null;
  if (!d) return { 'erro': 'sem_resposta' };
  const problems = Array.isArray(d.problemas)
    ? d.problemas
        .filter((x) => x && typeof x.campo === 'string' && typeof x.motivo === 'string')
        .map((x) => ({ 'campo': x.campo, 'motivo': x.motivo }))
    : [];
  return { 'erro': String(d.codigo), 'problemas': problems };
}

chrome.tabs.onRemoved.addListener((tab) => discard(tab));

startLogin();

chrome.runtime.onMessage.addListener((msg, sender, respond) => {
  if (!msg || typeof msg !== 'object' || typeof msg.t !== 'string') return false;
  if (sender.tab) {
    // Content script: only the submit, and no reply at all.
    const from = msg.t === 'enviado' ? fromContentScript(sender) : null;
    if (from) onSubmitted(msg, from);
    return false;
  }
  if (!fromPopup(sender) || !Number.isInteger(msg.aba)) return false;
  switch (msg.t) {
    case 'pendente': {
      const p = pendingLogins.get(msg.aba);
      respond(p ? { 'origem': p.origin, 'usuario': p.username } : null);
      return false;
    }
    case 'manter':
      // The open popup keeps the service worker alive while the user fills in.
      respond(pendingLogins.has(msg.aba));
      return false;
    case 'descartar':
      discard(msg.aba);
      respond(true);
      return false;
    case 'salvar':
      save(msg).then(respond, () => respond({ 'erro': 'sem_resposta' }));
      return true;
    case 'preencher':
      fillWithWindow(msg).then(respond, () => respond({ 'erro': 'sem_resposta' }));
      return true;
    default:
      return false;
  }
});
