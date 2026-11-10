// `ward login` (Phase 43; R14, D9; V6.3, V6.10, V6.15; S1, S13), in the service worker.
//
// The daemon does not call the extension: this module keeps a native messaging port
// open with `ward-nm` (`connectNative`, which also keeps the service worker alive) and
// asks the wait-for-login operation in a loop (the daemon holds the question for up to
// 25 s). When it receives a request (`{id, entry}`, with no value at all):
//
// 1. the active tab of the last focused window; the probe runs in the main frame
//    (`frameIds: [0]`) and returns the document origin and the `documentId`;
// 2. the login fill operation with the id and that origin: the daemon is the one that
//    compares the origin with the entry's (D9); if it differs, nothing leaves and the
//    request ends ("domain does not match");
// 3. with the fill, `fillInPage(..., submit = true)` runs only in that `documentId`
//    (if the tab navigated, Chrome refuses the injection), checks the origin again,
//    requires a POST form on the same origin, fills, submits and clears the fields; the
//    daemon reply is wiped right after;
// 4. waits for the next page to load, looks whether a visible password field is still
//    there (the site refused) and tells the daemon the outcome (login result operation).
//
// Nothing here listens to the page (no message listener; the page has no channel to
// the extension, V6.6), nothing is kept and no value goes to the console or the log.

import { fillInPage, probe, probeAfterSubmit } from './page.js';

const HOST = 'br.ward.nm';
/** Wait for the host reply: the wait-for-login one comes within 25 s. */
const REPLY_TIMEOUT_MS = 60 * 1000;
/** How long to wait for the page after the submit. */
const PAGE_TIMEOUT_MS = 15 * 1000;
/** With no navigation after this, the submitted document is the one checked. */
const NO_NAVIGATION_MS = 3 * 1000;
const STEP_MS = 250;

let port = null;
/** Who waits for the next host message (it replies one at a time, in order). */
let waiting = [];

const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));

function request(p, msg) {
  return new Promise((ok) => {
    const t = setTimeout(() => {
      // The host did not reply: the port lost its pace; close and reconnect.
      close(p);
    }, REPLY_TIMEOUT_MS);
    waiting.push((r) => {
      clearTimeout(t);
      ok(r);
    });
    try {
      p.postMessage(msg);
    } catch {
      close(p);
    }
  });
}

function close(p) {
  try {
    p.disconnect();
  } catch {
    // already closed
  }
  dropped(p);
}

function dropped(p) {
  if (port !== p) return;
  port = null;
  const pending = waiting;
  waiting = [];
  for (const r of pending) r(null);
  setTimeout(connect, 5000);
}

function connect() {
  if (port) return;
  let p;
  try {
    p = chrome.runtime.connectNative(HOST);
  } catch {
    setTimeout(connect, 30 * 1000);
    return;
  }
  port = p;
  waiting = [];
  p.onMessage.addListener((m) => {
    const r = waiting.shift();
    if (r) r(m && typeof m === 'object' ? m : null);
  });
  p.onDisconnect.addListener(() => {
    // Reads the error so Chrome does not complain about it in the console (no content).
    void chrome.runtime.lastError;
    dropped(p);
  });
  loop(p);
}

async function loop(p) {
  while (port === p) {
    const r = await request(p, { v: 1, op: 'aguardar_login' });
    if (port !== p || !r) return;
    if (r.tipo === 'pedido_de_login' && r.dados && typeof r.dados.id === 'string') {
      await serve(p, r.dados.id);
    } else if (r.tipo === 'erro') {
      // Locked, daemon down, or this host does not have the extension role.
      const code = r.dados && r.dados.codigo;
      await sleep(code === 'nao_autorizado' ? 60 * 1000 : 5 * 1000);
    }
  }
}

async function inject(target, func, args) {
  try {
    const [r] = await chrome.scripting.executeScript({ target, func, args: args || [] });
    return r || null;
  } catch {
    return null;
  }
}

/** Waits for the document after the submit and says whether the site still asks for
 * the password. */
async function afterSubmit(tab, submittedDocument, origin) {
  const start = Date.now();
  while (Date.now() - start < PAGE_TIMEOUT_MS) {
    await sleep(STEP_MS);
    let loading = false;
    try {
      loading = (await chrome.tabs.get(tab)).status === 'loading';
    } catch {
      return 'falha';
    }
    // The tab is still loading (the submit, a redirect or the next page).
    if (loading) continue;
    const r = await inject({ tabId: tab, frameIds: [0] }, probeAfterSubmit);
    if (!r || !r.result || !r.result.top) continue;
    const isNew = r.documentId !== submittedDocument;
    if (!isNew && Date.now() - start < NO_NAVIGATION_MS) continue;
    if (!r.result.ready) continue;
    // A page of another origin after the submit: the login was sent to the right
    // origin, but there is no telling whether it got in.
    if (r.result.origin !== origin) return 'enviado';
    return r.result.password ? 'recusado' : 'enviado';
  }
  return 'enviado';
}

async function serve(p, id) {
  let outcome = 'falha';
  try {
    const [tab] = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
    if (!tab || !Number.isInteger(tab.id) || tab.id < 0 || tab.incognito) return;
    const probed = await inject({ tabId: tab.id, frameIds: [0] }, probe);
    // Without a probe (a Chrome page, a tab without permission), the origin goes empty
    // and the daemon refuses it as a different origin.
    const origin = probed && probed.result && probed.result.top ? String(probed.result.origin) : '';
    const documentId = probed ? probed.documentId : undefined;
    const r = await request(p, { v: 1, op: 'preencher_login', 'dados': { id, 'origem': origin } });
    if (!r || r.tipo !== 'preenchimento' || !r.dados || typeof documentId !== 'string') {
      // The daemon already decided (different origin, expired) or the reply did not come.
      outcome = r && r.tipo === 'erro' ? null : 'falha';
      return;
    }
    const d = r.dados;
    const args = [origin, String(d.usuario || ''), String(d.senha || ''), String(d.totp || ''), true];
    d.senha = '';
    d.totp = '';
    r.dados = null;
    const done = await inject({ tabId: tab.id, documentIds: [documentId] }, fillInPage, args);
    args.fill('');
    const v = done && done.result;
    if (!v) {
      outcome = 'aba_mudou';
    } else if (!v.ok) {
      outcome =
        v.reason === 'origem' || v.reason === 'quadro'
          ? 'aba_mudou'
          : v.reason === 'formulario' || v.reason === 'sem_campo'
            ? 'sem_formulario'
            : 'falha';
    } else {
      outcome = await afterSubmit(tab.id, documentId, origin);
    }
  } catch {
    outcome = 'falha';
  } finally {
    if (outcome && port === p) {
      await request(p, { v: 1, op: 'resultado_login', 'dados': { id, 'resultado': outcome } });
    }
  }
}

/** Starts `ward login` (called when the service worker starts). */
export function startLogin() {
  connect();
}
