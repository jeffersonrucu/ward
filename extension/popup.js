// Extension popup (Phase 41; D5, D9; V6.2, V6.4–V6.7): the domain suggestions.
//
// Flow, all inside the popup (there is no service worker, fixed content script nor
// message listener in it; the page has no one to talk to, V6.6):
//  1. the active tab origin comes from `tab.url` (the `activeTab` permission, given by
//     the click on the icon) and goes to `ward-nm` in the suggest operation; the daemon
//     is the one that compares it with the entries' URL (D9);
//  2. on a click on an entry, a probe runs in the main frame (`frameIds: [0]`) and
//     returns the document origin and its `documentId`; the origin must be the same as
//     in step 1;
//  3. the fill operation goes to the daemon with that origin; the reply (password in
//     base64) goes straight as an argument to the fill, injected only in that
//     `documentId` (if the tab navigated, Chrome refuses) and which checks the origin
//     again before writing;
//  4. the reply is dropped and the popup closes (its context goes away).
//
// Phase 44c (D9): on an `http` origin (`localhost`/`127.0.0.1`), and since Phase 44e on
// every host of this machine in `https` (`needsWindow`, in `page.js`), the daemon only
// replies after the user confirms the origin in the Ward window, and the popup closes
// when the window takes the focus. So, on that origin, steps 3 and 4 belong to the
// service worker (`service.js`): the popup sends the tab, the probe's `documentId`, the
// entry and the origin, and the password never passes through here.
//
// Phase 42 (V6.8): if the service worker has a pending new login for this tab (the
// content script saw the submit and the daemon said it is not in the vault), the popup
// shows the save-new-login form in place of the suggestions. The password never reaches
// the popup: the service worker returns only the origin and the username, and on "Save"
// the popup sends name, username, group and environment; the service worker is the one
// that adds the password and talks to the daemon.
//
// Nothing is kept: no `storage` permission (the extension storage API does not even
// exist here), no page storage, nothing in the popup DOM besides name, username and
// environment. The texts come from the daemon and go in only through `textContent`
// (never HTML).

import { ICONS } from './icons.js';
import { SYMBOL } from './brand.js';
import { needsWindow, fillInPage, probe } from './page.js';

const HOST = 'br.ward.nm';
const SVG = 'http://www.w3.org/2000/svg';
const root = document.getElementById('popup');

/** Element with classes and children (text goes in as a text node, never HTML). */
function el(tag, classes, children = []) {
  const e = document.createElement(tag);
  if (classes) e.className = classes;
  for (const c of children) e.append(c);
  return e;
}

function text(tag, classes, content) {
  const e = el(tag, classes);
  e.textContent = content;
  return e;
}

/** Icon `<name>` (20×20, stroke 1.5) in the requested tone. */
function icon(name, size, tone) {
  const svg = document.createElementNS(SVG, 'svg');
  svg.setAttribute('class', `icon ${size}`);
  svg.setAttribute('viewBox', '0 0 20 20');
  svg.setAttribute('fill', 'none');
  svg.setAttribute('aria-hidden', 'true');
  if (tone) svg.setAttribute('data-icon-tone', tone);
  for (const c of ICONS[name] ?? []) {
    const p = document.createElementNS(SVG, 'path');
    p.setAttribute('transform', c.t);
    p.setAttribute('d', c.d);
    p.setAttribute('stroke', 'currentColor');
    p.setAttribute('stroke-linecap', 'round');
    p.setAttribute('stroke-linejoin', 'round');
    svg.append(p);
  }
  return svg;
}

/** The Ward symbol (logo symbol component), in the element's text color. */
function symbol(cls) {
  const svg = document.createElementNS(SVG, 'svg');
  svg.setAttribute('class', cls);
  svg.setAttribute('viewBox', SYMBOL.box);
  svg.setAttribute('aria-hidden', 'true');
  const p = document.createElementNS(SVG, 'path');
  p.setAttribute('transform', SYMBOL.t);
  p.setAttribute('d', SYMBOL.d);
  p.setAttribute('fill', 'currentColor');
  svg.append(p);
  return svg;
}

/** Text of the status badge (shown as is). */
const STATUS_LABEL = { unlocked: 'destravado', locked: 'travado' };

/** Popup header (node 17234): unlocked or locked. */
function header(status) {
  const children = [symbol('symbol'), text('span', 'product-name title-section', 'ward')];
  if (status) children.push(text('span', `status ${status} text-badge`, STATUS_LABEL[status]));
  return el('header', 'header', children);
}

/** Native messaging with `ward-nm`; `null` if the host did not reply. */
async function send(request) {
  try {
    const r = await chrome.runtime.sendNativeMessage(HOST, request);
    return r && typeof r === 'object' ? r : null;
  } catch {
    return null;
  }
}

const errorCode = (r) => (r && r.tipo === 'erro' && r.dados ? r.dados.codigo : null);

/** Only `http:`/`https:` have an origin to fill; the rest (chrome:, file:, about:) do not. */
function originOf(url) {
  try {
    const u = new URL(url);
    if (u.protocol !== 'https:' && u.protocol !== 'http:') return null;
    return u.origin;
  } catch {
    return null;
  }
}

function openWardButton(cls, label, withSymbol) {
  const children = withSymbol ? [symbol('button-symbol')] : [];
  children.push(text('span', 'text-body-strong', label));
  const b = el('button', `button ${cls}`, children);
  b.type = 'button';
  b.addEventListener('click', async () => {
    b.disabled = true;
    await send({ v: 1, op: 'abrir_app' });
    window.close();
  });
  return b;
}

function show(...children) {
  root.replaceChildren(...children);
}

/** Frame of the locked vault (node 17392). */
function showLocked(host) {
  const target = host ? `as entradas de ${host}` : 'as entradas desta aba';
  show(
    header('locked'),
    el('section', 'locked-body', [
      el('div', 'avatar-large', [icon('lock', 'md')]),
      text('h1', 'locked-title title-modal', 'O cofre está travado'),
      text('p', 'locked-text text-body', `Destrave no app do Ward para ver ${target}.`),
      openWardButton('primary', 'Abrir o Ward para destravar', false),
      text('p', 'note text-caption', 'A extensão nunca pede a senha mestra.'),
    ]),
  );
}

function showNotice(message) {
  show(
    header(null),
    el('section', 'body', [text('p', 'note text-small', message)]),
    el('footer', 'footer', [openWardButton('secondary-button', 'Abrir o Ward', true)]),
  );
}

const NOTE =
  'Clique numa entrada para preencher usuário, senha e TOTP nesta aba. Só aparecem entradas desta origem, e nada fica salvo no navegador.';

const EMPTY_NOTE =
  'Só aparecem entradas com a mesma origem desta aba (esquema, endereço e porta), e nada fica salvo no navegador.';

// Keys: the fill reasons of `fillInPage` and the daemon error codes (wire names).
const FAILURES = {
  'origem': 'A aba mudou de endereço. Abra o Ward de novo nesta página.',
  'quadro': 'A aba mudou de endereço. Abra o Ward de novo nesta página.',
  'sem_campo': 'Não achei um campo de senha visível nesta página. Nada foi preenchido.',
  'origem_diferente': 'A origem da aba não confere com a da entrada. Nada foi preenchido.',
  'entrada_inexistente': 'Essa entrada não existe mais no cofre.',
  // Phase 44c: the confirmation of the `http` origin in the Ward window.
  'negado': 'O preenchimento foi negado na janela do Ward. Nada foi enviado à página.',
  'expirado': 'A confirmação na janela do Ward expirou. Nada foi preenchido.',
  'aprovacao_indisponivel':
    'O app do Ward não abriu para confirmar a origem. Nada foi preenchido.',
};

const WAITING_WINDOW =
  'Endereço desta máquina: confirme a origem na janela do Ward para preencher.';
const DEFAULT_FAILURE = 'Não deu para preencher. Nada foi enviado à página.';

/** Frame of the domain suggestions (node 17332). */
function showSuggestions(tab, origin, items) {
  const note = text('p', 'note text-caption', items.length ? NOTE : EMPTY_NOTE);
  const n = items.length;
  const label =
    n === 0
      ? 'Nenhuma entrada para esta origem'
      : `${n} ${n === 1 ? 'entrada' : 'entradas'} para esta origem`;
  const buttons = [];
  const list = el(
    'ul',
    'list',
    items.map((s) => {
      const detail = s.totp ? `${s.usuario} · com TOTP` : s.usuario;
      const b = el('button', 'item', [
        el('span', 'avatar', [icon('type-login', 'md')]),
        el('span', 'item-texts', [
          el('span', 'row', [
            text('span', 'item-name text-body-strong', s.nome),
            text('span', `badge-environment ${s.ambiente} text-badge`, s.ambiente),
          ]),
          el('span', 'row', [
            el('span', 'badge-type text-tag', [icon('type-login', 'xs'), 'login']),
            text('span', 'item-detail text-small', detail),
          ]),
        ]),
      ]);
      b.type = 'button';
      b.dataset.entry = s.nome;
      b.addEventListener('click', () => fill(tab, origin, s.nome, buttons, note));
      buttons.push(b);
      return el('li', null, [b]);
    }),
  );
  show(
    header('unlocked'),
    el('section', 'body', [
      el('div', 'origin', [
        icon('lock', 'md'),
        el('div', 'origin-texts', [
          text('span', 'origin-label text-caption', 'Origem da aba'),
          text('span', 'origin-value mono-small', origin),
        ]),
      ]),
      text('p', 'note secondary text-label', label),
      ...(n ? [list] : []),
      note,
    ]),
    el('footer', 'footer', [openWardButton('secondary-button', 'Abrir o Ward', true)]),
  );
}

function fail(note, buttons, reason) {
  note.textContent = FAILURES[reason] ?? DEFAULT_FAILURE;
  note.classList.add('error');
  for (const b of buttons) b.disabled = false;
}

async function fill(tab, origin, name, buttons, note) {
  for (const b of buttons) b.disabled = true;
  // 2. Probe: the main frame document, its origin and the document id.
  let probed;
  try {
    [probed] = await chrome.scripting.executeScript({
      target: { tabId: tab.id, frameIds: [0] },
      world: 'ISOLATED',
      func: probe,
    });
  } catch {
    fail(note, buttons, 'origem');
    return;
  }
  const doc = probed && probed.result;
  if (!doc || doc.top !== true || doc.origin !== origin || !probed.documentId) {
    fail(note, buttons, 'origem');
    return;
  }
  // Phase 44c: an `http` origin, with the Ward window, through the service worker.
  // Phase 44e: the same for every host of this machine in `https` (`needsWindow`).
  if (needsWindow(doc.origin)) {
    note.textContent = WAITING_WINDOW;
    note.classList.remove('error');
    const r = await askService({
      t: 'preencher',
      'aba': tab.id,
      'documento': probed.documentId,
      'entrada': name,
      'origem': doc.origin,
    });
    if (r && r.ok === true) {
      window.close();
      return;
    }
    const code = r && typeof r.erro === 'string' ? r.erro : null;
    if (code === 'travado') {
      showLocked(new URL(origin).host);
      return;
    }
    fail(note, buttons, code);
    return;
  }
  // 3. The daemon compares the document origin with the entry's (D9).
  let r = await send({ v: 1, op: 'preencher', 'dados': { 'entrada': name, 'origem': doc.origin } });
  if (!r || r.tipo !== 'preenchimento' || !r.dados) {
    const code = errorCode(r);
    r = null;
    if (code === 'travado') {
      showLocked(new URL(origin).host);
      return;
    }
    fail(note, buttons, code);
    return;
  }
  let outcome = null;
  try {
    const d = r.dados;
    [outcome] = await chrome.scripting.executeScript({
      target: { tabId: tab.id, documentIds: [probed.documentId] },
      world: 'ISOLATED',
      func: fillInPage,
      args: [doc.origin, String(d.usuario ?? ''), String(d.senha ?? ''), String(d.totp ?? '')],
    });
  } catch {
    outcome = null;
  } finally {
    // 4. The reply with the password stays nowhere in the popup.
    if (r.dados) {
      r.dados.senha = '';
      r.dados.totp = '';
      r.dados.usuario = '';
    }
    r = null;
  }
  const res = outcome && outcome.result;
  if (!res || res.ok !== true) {
    fail(note, buttons, res && res.reason);
    return;
  }
  window.close();
}

// --- Phase 42: save new login (frame 17414) ------------------------------------------

/** Message to the extension service worker; `null` if it did not reply. */
async function askService(msg) {
  try {
    const r = await chrome.runtime.sendMessage(msg);
    return r === undefined ? null : r;
  } catch {
    return null;
  }
}

const HOST_LABELS = { dev: ['dev', 'desenv', 'local', 'localhost', 'test', 'teste'],
  stage: ['stage', 'staging', 'hml', 'homolog', 'homologacao', 'qa', 'uat'] };

/**
 * Name, group and environment suggestions from the host (the user checks and changes).
 * `admin.stage.clientex.test` → `clientex-stage-admin`, group `clientex`, `stage`;
 * `localhost:8083` → `localhost-8083`, group `localhost`, `dev`. With no hint in the
 * host, the suggested environment is `prod` (the one that stresses approval the most).
 */
function hostSuggestions(origin) {
  const u = new URL(origin);
  const host = u.hostname.toLowerCase();
  const ip = /^[0-9.]+$/.test(host) || host.startsWith('[');
  let labels = ip ? [host.replace(/[^0-9a-z]+/g, '-')] : host.split('.').filter(Boolean);
  if (labels.length > 1) labels = labels.slice(0, -1);
  const reversed = [...labels].reverse();
  const clean = (t) => t.replace(/[^a-z0-9._-]+/g, '-').replace(/^[^a-z0-9]+/, '').slice(0, 64);
  const name = clean([...reversed, ...(u.port ? [u.port] : [])].join('-'));
  const group = clean(reversed[0] || '');
  let environment = 'prod';
  if (ip || labels.some((r) => HOST_LABELS.dev.includes(r))) environment = 'dev';
  else if (labels.some((r) => HOST_LABELS.stage.includes(r))) environment = 'stage';
  return { name, group, environment };
}

/** Text field (component 340): label, text input and the help line (hidden until an
 * error). */
function textField(id, label, value) {
  const input = el('input', 'field-input text-body');
  input.id = id;
  input.type = 'text';
  input.value = value;
  input.autocomplete = 'off';
  input.spellcheck = false;
  const help = text('p', 'help text-caption', '');
  help.hidden = true;
  const l = text('label', 'label text-label', label);
  l.htmlFor = id;
  return el('div', 'field', [l, input, help]);
}

/** `Select` (422): label, native `<select>` with the design chevron. */
function selectField(id, label, options, value) {
  const sel = el('select', 'select text-body');
  sel.id = id;
  for (const o of options) {
    const op = text('option', null, o);
    op.value = o;
    sel.append(op);
  }
  sel.value = value;
  const l = text('label', 'label text-label', label);
  l.htmlFor = id;
  const help = text('p', 'help text-caption', '');
  help.hidden = true;
  return el('div', 'field', [l, el('div', 'select-input', [sel, icon('chevron-down', 'md')]), help]);
}

/** Field of a daemon validation problem (wire name) → field of the form. */
const FIELD_OF_PROBLEM = { 'nome': 'name', 'grupo': 'group', 'ambiente': 'environment', 'usuario': 'username', 'senha': 'password', url: 'url' };

// Keys: local codes of the service worker (wire names until m2:Phase 52).
const SAVE_FAILURES = {
  'expirou': 'O pedido para salvar expirou. Envie o formulário de novo.',
  'pedido_invalido': 'Confira nome, usuário e ambiente.',
  'sem_resposta': 'O Ward não respondeu. Nada foi salvo.',
};

/** Frame of the save new login form (node 17414; 18850 with the whole content). */
function showSave(tab, origin, username) {
  const host = new URL(origin).host;
  const sug = hostSuggestions(origin);
  const name = textField('entry-name', 'Nome', sug.name);
  const usernameField = textField('entry-username', 'Usuário', username);
  const password = el('div', 'field', [
    text('span', 'label text-label', 'Senha'),
    // Fixed mask: neither the value nor the length of the password reaches the popup.
    el('div', 'field-input field-input-mask', [text('span', 'mask text-body', '••••••••••••')]),
    text('p', 'help help-fixed text-caption', 'Vai direto para o cofre; a extensão não mostra nem guarda.'),
  ]);
  const type = selectField('entry-type', 'Tipo', ['login'], 'login');
  const environment = selectField('entry-environment', 'Ambiente', ['dev', 'stage', 'prod'], sug.environment);
  const group = textField('entry-group', 'Grupo', sug.group);
  const fields = { name, username: usernameField, group, environment };
  const note = text('p', 'note error text-caption', '');
  note.hidden = true;

  const cancel = el('button', 'button secondary-button', [text('span', 'text-body-strong', 'Agora não')]);
  cancel.type = 'button';
  const saveButton = el('button', 'button primary', [text('span', 'text-body-strong', 'Salvar no Ward')]);
  saveButton.type = 'submit';

  const form = el('form', 'popup-save', [
    el('section', 'save-body', [
      el('div', 'intro', [
        text('h1', 'save-title title-modal', 'Salvar este login no Ward?'),
        text('p', 'save-text text-small',
          `O formulário de ${host} foi enviado com um login que ainda não está no cofre.`),
      ]),
      name,
      usernameField,
      password,
      el('div', 'double-row', [type, environment]),
      group,
      el('div', 'initial-level', [
        icon('lock', 'md'),
        el('div', 'level-texts', [
          text('span', 'level-title text-body-strong', 'Nível inicial: com aprovação'),
          text('span', 'level-text text-caption',
            'O Claude só usa depois que você aprovar. Para mudar, use o app, com PIN.'),
        ]),
      ]),
      note,
    ]),
    el('footer', 'footer save-footer', [cancel, saveButton]),
  ]);
  form.noValidate = true;

  const value = (c) => c.querySelector('input, select').value;
  const clearErrors = () => {
    note.hidden = true;
    for (const c of Object.values(fields)) {
      c.classList.remove('with-error');
      const a = c.querySelector('.help');
      a.hidden = true;
      a.textContent = '';
    }
  };

  // The service worker stays alive while the popup is open (it keeps the pending login).
  const keepAlive = setInterval(() => askService({ t: 'manter', 'aba': tab.id }), 20000);

  cancel.addEventListener('click', async () => {
    clearInterval(keepAlive);
    await askService({ t: 'descartar', 'aba': tab.id });
    window.close();
  });

  form.addEventListener('submit', async (ev) => {
    ev.preventDefault();
    clearErrors();
    saveButton.disabled = true;
    cancel.disabled = true;
    const r = await askService({
      t: 'salvar',
      'aba': tab.id,
      'nome': value(name).trim(),
      'usuario': value(usernameField).trim(),
      'grupo': value(group).trim(),
      'ambiente': value(environment),
    });
    saveButton.disabled = false;
    cancel.disabled = false;
    if (r && r.ok === true) {
      clearInterval(keepAlive);
      window.close();
      return;
    }
    const code = r && typeof r.erro === 'string' ? r.erro : 'sem_resposta';
    if (code === 'travado') {
      clearInterval(keepAlive);
      showLocked(host);
      return;
    }
    let general = code === 'validacao' ? '' : (SAVE_FAILURES[code] ?? 'Não deu para salvar. Nada foi gravado.');
    for (const p of (r && Array.isArray(r.problemas) ? r.problemas : [])) {
      const c = fields[FIELD_OF_PROBLEM[p.campo]];
      if (c) {
        c.classList.add('with-error');
        const a = c.querySelector('.help');
        a.textContent = p.motivo;
        a.hidden = false;
      } else if (!general) {
        general = `${p.campo}: ${p.motivo}`;
      }
    }
    if (general) {
      note.textContent = general;
      note.hidden = false;
    }
  });

  show(header('unlocked'), form);
  root.classList.add('with-form');
}

async function start() {
  let tab = null;
  try {
    [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  } catch {
    tab = null;
  }
  const pending = tab ? await askService({ t: 'pendente', 'aba': tab.id }) : null;
  if (pending && typeof pending.origem === 'string' && typeof pending.usuario === 'string') {
    const o = originOf(pending.origem);
    if (o) {
      showSave(tab, o, pending.usuario);
      return;
    }
  }
  const origin = tab && tab.url ? originOf(tab.url) : null;
  if (!tab || !origin) {
    showNotice('Esta aba não tem uma origem http ou https para preencher.');
    return;
  }
  // 1. Suggestions for the tab origin; the daemon compares (D9).
  const r = await send({ v: 1, op: 'sugerir', 'dados': { 'origem': origin } });
  if (r && r.tipo === 'sugestoes' && r.dados && Array.isArray(r.dados.itens)) {
    const items = r.dados.itens.filter(
      (s) =>
        s &&
        typeof s.nome === 'string' &&
        typeof s.usuario === 'string' &&
        ['dev', 'stage', 'prod'].includes(s.ambiente),
    );
    showSuggestions(tab, origin, items);
    return;
  }
  const code = errorCode(r);
  if (code === 'travado') {
    showLocked(new URL(origin).host);
  } else if (code === 'nao_autorizado') {
    showNotice('O Ward não reconheceu esta extensão. Confira a instalação do Ward.');
  } else {
    showNotice('O Ward não respondeu. Confira se o Ward está instalado e rodando.');
  }
}

start();
