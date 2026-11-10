// Service worker da extensão (Fase 42; V6.8; S1, S13): guarda, só em memória e por
// pouco tempo, o login que o content script (`envio.js`) viu ser enviado, e o salva no
// cofre quando o usuário confirma no popup.
//
// - A senha nunca sai daqui para o popup: o popup recebe só a origem e o usuário, e no
//   "Salvar" manda nome, usuário, grupo e ambiente; quem junta a senha e manda
//   `salvar_login` ao `ward-nm` é este service worker.
// - Nada é guardado em disco (sem a permissão `storage`, sem armazenamento da página);
//   o pendente some ao salvar, no "Agora não", ao fechar a aba, em 3 minutos ou quando o
//   Chrome encerra o service worker.
// - Quem manda cada mensagem sai do `sender` que o Chrome preenche, nunca do conteúdo:
//   o envio só vale do content script do quadro principal de uma aba http/https; o
//   pedido, o salvar e o descartar só valem do `popup.html` da própria extensão. Página
//   nenhuma fala com a extensão (sem `externally_connectable`, V6.6).
// - Só oferece login que o cofre não tem: o daemon responde `sugerir` para a origem, e
//   o usuário enviado não pode estar entre as sugestões. Com o cofre travado (ou sem o
//   Ward), o envio é descartado na hora.
// - Fase 43: o `ward login` mora em `login.js` (porta de native messaging aberta com o
//   `ward-nm`, que também mantém este service worker vivo).
// - Fase 44c (D9): o preencher do popup numa origem `http` (`localhost`/`127.0.0.1`)
//   espera o usuário confirmar a origem na janela do Ward (até 2 min). O popup fecha
//   quando a janela pega o foco, então quem espera e preenche é este service worker: o
//   popup manda a aba, o `documentId` que a sonda dele achou, a entrada e a origem; a
//   resposta do daemon vai direto como argumento ao preenchimento naquele documento e é
//   apagada logo depois. Nada volta ao popup além de "ok" ou o código do erro.

import { iniciarLogin } from './login.js';
import { preencherNaPagina } from './pagina.js';

const HOST = 'br.ward.nm';
const POPUP = chrome.runtime.getURL('popup.html');
const PRAZO_MS = 3 * 60 * 1000;
// D25: usuário e senha até 2 KiB; nome, grupo e o resto o daemon valida.
const MAX = 2048;
const AMBIENTES = new Set(['dev', 'stage', 'prod']);

/** tabId → { origem, url, usuario, senha, timer }. Um por aba; o novo troca o velho. */
const pendentes = new Map();

function descartar(aba) {
  const p = pendentes.get(aba);
  if (!p) return;
  clearTimeout(p.timer);
  p.senha = '';
  p.usuario = '';
  pendentes.delete(aba);
}

const texto = (v, max) =>
  typeof v === 'string' && v.length > 0 && v.length <= max && !v.includes('\0');

async function enviar(pedido) {
  try {
    const r = await chrome.runtime.sendNativeMessage(HOST, pedido);
    return r && typeof r === 'object' ? r : null;
  } catch {
    return null;
  }
}

/** Origem http/https de uma URL, ou `null`. */
function origemDa(url) {
  try {
    const u = new URL(url);
    if (u.protocol !== 'https:' && u.protocol !== 'http:') return null;
    return u.origin;
  } catch {
    return null;
  }
}

/** A URL da entrada: origem e caminho do documento enviado, sem consulta nem fragmento. */
function urlDaEntrada(url) {
  const u = new URL(url);
  return u.origin + u.pathname;
}

/** O envio veio do content script do quadro principal de uma aba http/https? */
function doContentScript(sender) {
  if (sender.id !== chrome.runtime.id || !sender.tab || sender.tab.incognito) return null;
  if (sender.frameId !== 0 || !Number.isInteger(sender.tab.id) || sender.tab.id < 0) return null;
  if (sender.documentLifecycle !== undefined && sender.documentLifecycle !== 'active') return null;
  const origem = origemDa(sender.url);
  if (!origem || sender.origin !== origem) return null;
  return { aba: sender.tab.id, janela: sender.tab.windowId, origem, url: urlDaEntrada(sender.url) };
}

/** A mensagem veio do popup da própria extensão (página da extensão, sem aba)? */
function doPopup(sender) {
  return sender.id === chrome.runtime.id && !sender.tab && sender.url === POPUP;
}

async function aoEnviar(msg, de) {
  if (!texto(msg.usuario, MAX) || !texto(msg.senha, MAX)) return;
  const r = await enviar({ v: 1, op: 'sugerir', dados: { origem: de.origem } });
  if (!r || r.tipo !== 'sugestoes' || !r.dados || !Array.isArray(r.dados.itens)) return;
  if (r.dados.itens.some((s) => s && s.usuario === msg.usuario)) return;
  descartar(de.aba);
  const p = {
    origem: de.origem,
    url: de.url,
    usuario: msg.usuario,
    senha: msg.senha,
    timer: setTimeout(() => descartar(de.aba), PRAZO_MS),
  };
  pendentes.set(de.aba, p);
  try {
    await chrome.action.openPopup({ windowId: de.janela });
  } catch {
    // Sem janela em foco: o popup mostra o pendente quando o usuário clicar no ícone.
  }
}

/** Quanto esperar a resposta do `preencher` com a janela (o daemon espera 2 min). */
const PRAZO_JANELA_MS = 130 * 1000;

/**
 * Um pedido ao host por uma porta própria: o `connectNative` mantém o service worker
 * vivo enquanto a janela de aprovação espera. `null` se o host não respondeu.
 */
function pedirPorPorta(pedido) {
  return new Promise((ok) => {
    let p;
    try {
      p = chrome.runtime.connectNative(HOST);
    } catch {
      ok(null);
      return;
    }
    let feito = false;
    let prazo = null;
    const fim = (r) => {
      if (feito) return;
      feito = true;
      clearTimeout(prazo);
      try {
        p.disconnect();
      } catch {
        // já fechada
      }
      ok(r);
    };
    prazo = setTimeout(() => fim(null), PRAZO_JANELA_MS);
    p.onMessage.addListener((m) => fim(m && typeof m === 'object' ? m : null));
    p.onDisconnect.addListener(() => {
      void chrome.runtime.lastError;
      fim(null);
    });
    try {
      p.postMessage(pedido);
    } catch {
      fim(null);
    }
  });
}

/** Fase 44c: o preencher do popup numa origem `http`, com a janela do Ward. */
async function preencherComJanela(msg) {
  const origem = typeof msg.origem === 'string' ? origemDa(msg.origem) : null;
  if (
    !texto(msg.entrada, MAX) ||
    typeof msg.documento !== 'string' ||
    msg.documento.length === 0 ||
    msg.documento.length > MAX ||
    !origem ||
    origem !== msg.origem ||
    new URL(origem).protocol !== 'http:'
  ) {
    return { erro: 'pedido_invalido' };
  }
  let r = await pedirPorPorta({ v: 1, op: 'preencher', dados: { entrada: msg.entrada, origem } });
  if (!r || r.tipo !== 'preenchimento' || !r.dados) {
    const codigo = r && r.tipo === 'erro' && r.dados ? String(r.dados.codigo) : 'sem_resposta';
    return { erro: codigo };
  }
  const d = r.dados;
  const args = [origem, String(d.usuario ?? ''), String(d.senha ?? ''), String(d.totp ?? '')];
  d.senha = '';
  d.totp = '';
  d.usuario = '';
  r = null;
  let feito = null;
  try {
    [feito] = await chrome.scripting.executeScript({
      target: { tabId: msg.aba, documentIds: [msg.documento] },
      world: 'ISOLATED',
      func: preencherNaPagina,
      args,
    });
  } catch {
    feito = null;
  } finally {
    args.fill('');
  }
  const v = feito && feito.result;
  if (v && v.ok === true) return { ok: true };
  return { erro: (v && typeof v.motivo === 'string' && v.motivo) || 'origem' };
}

/** Base64 dos bytes UTF-8 (como o protocolo leva os segredos). */
function base64(s) {
  const bytes = new TextEncoder().encode(s);
  let bin = '';
  for (const b of bytes) bin += String.fromCharCode(b);
  bytes.fill(0);
  return btoa(bin);
}

async function salvar(msg) {
  const p = pendentes.get(msg.aba);
  if (!p) return { erro: 'expirou' };
  if (
    !texto(msg.nome, MAX) ||
    !texto(msg.usuario, MAX) ||
    typeof msg.grupo !== 'string' ||
    msg.grupo.length > MAX ||
    !AMBIENTES.has(msg.ambiente)
  ) {
    return { erro: 'pedido_invalido' };
  }
  const pedido = {
    v: 1,
    op: 'salvar_login',
    dados: {
      entrada: {
        nome: msg.nome,
        grupo: msg.grupo,
        ambiente: msg.ambiente,
        tipo: 'login',
        campos: { url: p.url, usuario: msg.usuario, senha: base64(p.senha) },
      },
    },
  };
  const r = await enviar(pedido);
  pedido.dados.entrada.campos.senha = '';
  if (r && r.tipo === 'ok') {
    descartar(msg.aba);
    return { ok: true };
  }
  const d = r && r.tipo === 'erro' && r.dados ? r.dados : null;
  if (!d) return { erro: 'sem_resposta' };
  const problemas = Array.isArray(d.problemas)
    ? d.problemas
        .filter((x) => x && typeof x.campo === 'string' && typeof x.motivo === 'string')
        .map((x) => ({ campo: x.campo, motivo: x.motivo }))
    : [];
  return { erro: String(d.codigo), problemas };
}

chrome.tabs.onRemoved.addListener((aba) => descartar(aba));

iniciarLogin();

chrome.runtime.onMessage.addListener((msg, sender, responder) => {
  if (!msg || typeof msg !== 'object' || typeof msg.t !== 'string') return false;
  if (sender.tab) {
    // Content script: só o envio, e sem resposta nenhuma.
    const de = msg.t === 'enviado' ? doContentScript(sender) : null;
    if (de) aoEnviar(msg, de);
    return false;
  }
  if (!doPopup(sender) || !Number.isInteger(msg.aba)) return false;
  switch (msg.t) {
    case 'pendente': {
      const p = pendentes.get(msg.aba);
      responder(p ? { origem: p.origem, usuario: p.usuario } : null);
      return false;
    }
    case 'manter':
      // O popup aberto mantém o service worker vivo enquanto o usuário preenche.
      responder(pendentes.has(msg.aba));
      return false;
    case 'descartar':
      descartar(msg.aba);
      responder(true);
      return false;
    case 'salvar':
      salvar(msg).then(responder, () => responder({ erro: 'sem_resposta' }));
      return true;
    case 'preencher':
      preencherComJanela(msg).then(responder, () => responder({ erro: 'sem_resposta' }));
      return true;
    default:
      return false;
  }
});
