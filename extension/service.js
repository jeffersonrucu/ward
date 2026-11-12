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
  if (!isText(msg.username, MAX) || !isText(msg.password, MAX)) return;
  const r = await send({ v: 2, op: 'suggest', data: { origin: from.origin } });
  if (!r || r.type !== 'suggestions' || !r.data || !Array.isArray(r.data.items)) return;
  if (r.data.items.some((s) => s && s.username === msg.username)) return;
  discard(from.tab);
  const p = {
    origin: from.origin,
    url: from.url,
    username: msg.username,
    password: msg.password,
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
  const origin = typeof msg.origin === 'string' ? originOf(msg.origin) : null;
  if (
    !isText(msg.entry, MAX) ||
    typeof msg.document !== 'string' ||
    msg.document.length === 0 ||
    msg.document.length > MAX ||
    !origin ||
    origin !== msg.origin ||
    !needsWindow(origin)
  ) {
    return { error: 'invalid_request' };
  }
  let r = await requestByPort({ v: 2, op: 'fill', data: { entry: msg.entry, origin } });
  if (!r || r.type !== 'fill' || !r.data) {
    const code = r && r.type === 'error' && r.data ? String(r.data.code) : 'no_response';
    return { error: code };
  }
  const d = r.data;
  const args = [origin, String(d.username ?? ''), String(d.password ?? ''), String(d.totp ?? '')];
  d.password = '';
  d.totp = '';
  d.username = '';
  r = null;
  let done = null;
  try {
    [done] = await chrome.scripting.executeScript({
      target: { tabId: msg.tab, documentIds: [msg.document] },
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
  return { error: (v && typeof v.reason === 'string' && v.reason) || 'origin' };
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
  const p = pendingLogins.get(msg.tab);
  if (!p) return { error: 'pending_expired' };
  if (
    !isText(msg.name, MAX) ||
    !isText(msg.username, MAX) ||
    typeof msg.group !== 'string' ||
    msg.group.length > MAX ||
    !ENVIRONMENTS.has(msg.environment)
  ) {
    return { error: 'invalid_request' };
  }
  const request = {
    v: 2,
    op: 'save_login',
    data: {
      entry: {
        name: msg.name,
        group: msg.group,
        environment: msg.environment,
        type: 'login',
        fields: { url: p.url, username: msg.username, password: base64(p.password) },
      },
    },
  };
  const r = await send(request);
  request.data.entry.fields.password = '';
  if (r && r.type === 'ok') {
    discard(msg.tab);
    return { ok: true };
  }
  const d = r && r.type === 'error' && r.data ? r.data : null;
  if (!d) return { error: 'no_response' };
  const problems = Array.isArray(d.problems)
    ? d.problems
        .filter((x) => x && typeof x.field === 'string' && typeof x.reason === 'string')
        .map((x) => ({ field: x.field, reason: x.reason }))
    : [];
  return { error: String(d.code), problems };
}

chrome.tabs.onRemoved.addListener((tab) => discard(tab));

startLogin();

chrome.runtime.onMessage.addListener((msg, sender, respond) => {
  if (!msg || typeof msg !== 'object' || typeof msg.t !== 'string') return false;
  if (sender.tab) {
    // Content script: only the submit, and no reply at all.
    const from = msg.t === 'submitted' ? fromContentScript(sender) : null;
    if (from) onSubmitted(msg, from);
    return false;
  }
  if (!fromPopup(sender) || !Number.isInteger(msg.tab)) return false;
  switch (msg.t) {
    case 'pending': {
      const p = pendingLogins.get(msg.tab);
      respond(p ? { origin: p.origin, username: p.username } : null);
      return false;
    }
    case 'keep':
      // The open popup keeps the service worker alive while the user fills in.
      respond(pendingLogins.has(msg.tab));
      return false;
    case 'discard':
      discard(msg.tab);
      respond(true);
      return false;
    case 'save':
      save(msg).then(respond, () => respond({ error: 'no_response' }));
      return true;
    case 'fill':
      fillWithWindow(msg).then(respond, () => respond({ error: 'no_response' }));
      return true;
    default:
      return false;
  }
});
