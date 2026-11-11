// The only door of the interface to the Rust side. In the app, the Tauri IPC; in Chrome in
// test mode (D7), the HTTP bridge of `ward-test-bridge`. No reply carries a secret (the
// "copy" is resolved in Rust, D4), and nothing here keeps a reply in global state, in the
// browser storage or in the URL.
import { Channel, invoke } from '@tauri-apps/api/core';
import { COMMANDS } from './commands.js';

let transport = null;

async function getTransport() {
  if (transport) return transport;
  // `import.meta.env.MODE` is replaced at build time: outside test mode this branch
  // leaves the bundle and the bridge code does not even enter `dist/` (checked by
  // `make app-check`).
  if (import.meta.env.MODE === 'teste' && !('__TAURI_INTERNALS__' in window)) {
    const { createTransport } = await import('./test-bridge.js');
    transport = createTransport();
  } else {
    transport = (command, args) => invoke(command, args);
  }
  return transport;
}

/** Command error with the wire fields of code and message, with no input value. */
function failure(code, message) {
  return Object.freeze({ 'code': code, 'message': message });
}

export async function call(command, args = {}) {
  if (!COMMANDS.includes(command)) {
    throw failure('comando-desconhecido', 'comando desconhecido');
  }
  const t = await getTransport();
  try {
    return await t(command, args);
  } catch (e) {
    if (e && typeof e.code === 'string' && typeof e.message === 'string') {
      throw failure(e.code, e.message);
    }
    throw failure('falha', 'não foi possível falar com o app');
  }
}

/** The notices the Rust side sends through the channel (D65 a), one per window. */
export const NOTICES = Object.freeze(['unlock-request', 'approval-request', 'status-changed']);

/**
 * Notices from the Rust side (D65 a): `onArrive()` is called when the notice arrives —
 * in the main window, the unlock request (Phase 31); in the approval window, the
 * approval request (Phase 36); in the main window also the status change (Phase 39: the
 * vault locked through the tray, by itself or through the CLI). The notice carries no
 * data; whoever receives it asks. In the app, through the Tauri channel (the listen
 * command, one per window); in the test bridge, which has no channel, by periodic
 * polling. Returns the function that stops listening.
 * @param {() => Promise<void> | void} onArrive
 * @param {string | string[]} [notice]
 * @returns {Promise<() => void>}
 */
export async function listenNotices(onArrive, notice = ['unlock-request', 'status-changed']) {
  const notices = Array.isArray(notice) ? notice : [notice];
  if (notices.length === 0 || !notices.every((a) => NOTICES.includes(a))) {
    throw failure('aviso-desconhecido', 'aviso desconhecido');
  }
  if (import.meta.env.MODE === 'teste' && !('__TAURI_INTERNALS__' in window)) {
    const { listenByPolling } = await import('./test-bridge.js');
    return listenByPolling(onArrive);
  }
  let active = true;
  const channel = new Channel();
  channel.onmessage = (m) => {
    if (active && m && notices.includes(m.notice)) onArrive();
  };
  try {
    await invoke('listen_notices', { 'channel': channel });
  } catch {
    // Without the channel, the screen still sees the request when it asks again.
  }
  return () => {
    active = false;
  };
}
