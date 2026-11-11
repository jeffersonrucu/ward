// Transport of the test bridge (D7). Only enters the test-mode build.
//
// `ward-test-bridge` serves this interface and opens Chrome at
// `127.0.0.1:<port>/#token=<random>` (same origin). The token lives only in this module
// variable (never in the browser storage, in a cookie or in the URL after it is read) and
// goes in the `Authorization` header of each call.

let token = null;
let openingScreen = null;

function readToken() {
  if (token !== null) return token;
  // The optional screen parameter after the token (test mode only): opens straight on a
  // screen for QA in Chrome, before the unlock screens existed. Leaves the URL together
  // with the token.
  const m = /^#token=([0-9a-f]{64})(?:&tela=([a-z-]{1,32}))?$/.exec(window.location.hash);
  token = m ? m[1] : '';
  openingScreen = m && m[2] ? m[2] : null;
  // Takes the token out of the address bar and of the history.
  window.history.replaceState(null, '', window.location.pathname);
  return token;
}

export function createTransport() {
  readToken();
  return async (command, args) => {
    let reply;
    try {
      reply = await fetch(`/ponte/${command}`, {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${token}`,
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(args ?? {}),
        credentials: 'omit',
        cache: 'no-store',
        redirect: 'error',
        referrerPolicy: 'no-referrer',
      });
    } catch {
      throw { 'code': 'ponte-indisponivel', 'message': 'a ponte de teste não respondeu' };
    }
    let body;
    try {
      body = await reply.json();
    } catch {
      throw { 'code': 'ponte-invalida', 'message': 'resposta inválida da ponte de teste' };
    }
    if (body && body.ok === true) return body.value ?? null;
    throw {
      'code': String(body?.code ?? 'falha'),
      'message': String(body?.message ?? 'falha na ponte de teste'),
    };
  };
}

/** Screen asked for by the opening URL (its optional screen parameter), or `null`. Test mode only. */
export function requestedScreen() {
  readToken();
  return openingScreen;
}

/**
 * The bridge does not have the Tauri notice channel: it polls every 2 s, one poll at a
 * time (the previous one has to finish), and stops when the screen leaves. Test mode
 * only.
 * @param {() => Promise<void> | void} onArrive
 */
export function listenByPolling(onArrive) {
  let busy = false;
  const id = setInterval(async () => {
    if (busy) return;
    busy = true;
    try {
      await onArrive();
    } finally {
      busy = false;
    }
  }, 2000);
  return () => clearInterval(id);
}
