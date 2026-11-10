// `ward login` (Fase 43; R14, D9; V6.3, V6.10, V6.15; S1, S13), no service worker.
//
// O daemon não chama a extensão: este módulo mantém uma porta de native messaging aberta
// com o `ward-nm` (`connectNative`, que também mantém o service worker vivo) e pergunta
// `aguardar_login` em laço (o daemon segura a pergunta até 25 s). Ao receber um pedido
// (`{id, entrada}`, sem valor nenhum):
//
// 1. a aba ativa da última janela em foco; a sonda roda no quadro principal (`frameIds:
//    [0]`) e devolve a origem do documento e o `documentId`;
// 2. `preencher_login` com o id e essa origem: quem compara a origem com a da entrada é
//    o daemon (D9); diferente, nada sai e o pedido acaba ("domínio não confere");
// 3. com o preenchimento, `preencherNaPagina(..., enviar = true)` roda só naquele
//    `documentId` (se a aba navegou, o Chrome recusa a injeção), confere a origem de
//    novo, exige um formulário POST da mesma origem, preenche, envia e limpa os campos;
//    a resposta do daemon é apagada logo depois;
// 4. espera a página seguinte carregar e olha se ainda há campo de senha visível
//    (o site recusou) e diz o desfecho ao daemon (`resultado_login`).
//
// Nada aqui escuta a página (sem ouvinte de mensagem; a página não tem canal até a
// extensão, V6.6), nada é guardado e nenhum valor vai para o console ou para o log.

import { preencherNaPagina, sondar, sondarDepois } from './page.js';

const HOST = 'br.ward.nm';
/** Espera pela resposta do host: a do `aguardar_login` vem em até 25 s. */
const PRAZO_RESPOSTA_MS = 60 * 1000;
/** Quanto esperar a página seguinte ao envio. */
const PRAZO_PAGINA_MS = 15 * 1000;
/** Sem navegação depois disso, o documento enviado é o que se confere. */
const SEM_NAVEGACAO_MS = 3 * 1000;
const PASSO_MS = 250;

let porta = null;
/** Quem espera a próxima mensagem do host (ele responde uma por vez, em ordem). */
let esperando = [];

const dormir = (ms) => new Promise((ok) => setTimeout(ok, ms));

function pedir(p, msg) {
  return new Promise((ok) => {
    const t = setTimeout(() => {
      // O host não respondeu: a porta perdeu o compasso; fecha e reconecta.
      fechar(p);
    }, PRAZO_RESPOSTA_MS);
    esperando.push((r) => {
      clearTimeout(t);
      ok(r);
    });
    try {
      p.postMessage(msg);
    } catch {
      fechar(p);
    }
  });
}

function fechar(p) {
  try {
    p.disconnect();
  } catch {
    // já fechada
  }
  caiu(p);
}

function caiu(p) {
  if (porta !== p) return;
  porta = null;
  const pendentes = esperando;
  esperando = [];
  for (const r of pendentes) r(null);
  setTimeout(conectar, 5000);
}

function conectar() {
  if (porta) return;
  let p;
  try {
    p = chrome.runtime.connectNative(HOST);
  } catch {
    setTimeout(conectar, 30 * 1000);
    return;
  }
  porta = p;
  esperando = [];
  p.onMessage.addListener((m) => {
    const r = esperando.shift();
    if (r) r(m && typeof m === 'object' ? m : null);
  });
  p.onDisconnect.addListener(() => {
    // Lê o erro para o Chrome não reclamar dele no console (sem conteúdo nenhum).
    void chrome.runtime.lastError;
    caiu(p);
  });
  laco(p);
}

async function laco(p) {
  while (porta === p) {
    const r = await pedir(p, { v: 1, op: 'aguardar_login' });
    if (porta !== p || !r) return;
    if (r.tipo === 'pedido_de_login' && r.dados && typeof r.dados.id === 'string') {
      await atender(p, r.dados.id);
    } else if (r.tipo === 'erro') {
      // Travado, daemon fora do ar, ou este host não tem o papel extensão.
      const codigo = r.dados && r.dados.codigo;
      await dormir(codigo === 'nao_autorizado' ? 60 * 1000 : 5 * 1000);
    }
  }
}

async function injetar(alvo, func, args) {
  try {
    const [r] = await chrome.scripting.executeScript({ target: alvo, func, args: args || [] });
    return r || null;
  } catch {
    return null;
  }
}

/** Espera o documento seguinte ao envio e diz se o site ainda pede a senha. */
async function depoisDoEnvio(aba, documentoEnviado, origem) {
  const inicio = Date.now();
  while (Date.now() - inicio < PRAZO_PAGINA_MS) {
    await dormir(PASSO_MS);
    let carregando = false;
    try {
      carregando = (await chrome.tabs.get(aba)).status === 'loading';
    } catch {
      return 'falha';
    }
    // A aba ainda carrega (o envio, um redirecionamento ou a página seguinte).
    if (carregando) continue;
    const r = await injetar({ tabId: aba, frameIds: [0] }, sondarDepois);
    if (!r || !r.result || !r.result.topo) continue;
    const novo = r.documentId !== documentoEnviado;
    if (!novo && Date.now() - inicio < SEM_NAVEGACAO_MS) continue;
    if (!r.result.pronto) continue;
    // Página de outra origem depois do envio: o login foi enviado à origem certa, mas
    // não dá para dizer se entrou.
    if (r.result.origem !== origem) return 'enviado';
    return r.result.senha ? 'recusado' : 'enviado';
  }
  return 'enviado';
}

async function atender(p, id) {
  let resultado = 'falha';
  try {
    const [aba] = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
    if (!aba || !Number.isInteger(aba.id) || aba.id < 0 || aba.incognito) return;
    const sonda = await injetar({ tabId: aba.id, frameIds: [0] }, sondar);
    // Sem sonda (página do Chrome, aba sem permissão), a origem vai vazia e o daemon
    // recusa como origem diferente.
    const origem = sonda && sonda.result && sonda.result.topo ? String(sonda.result.origem) : '';
    const documento = sonda ? sonda.documentId : undefined;
    const r = await pedir(p, { v: 1, op: 'preencher_login', dados: { id, origem } });
    if (!r || r.tipo !== 'preenchimento' || !r.dados || typeof documento !== 'string') {
      // O daemon já decidiu (origem diferente, expirado) ou a resposta não veio.
      resultado = r && r.tipo === 'erro' ? null : 'falha';
      return;
    }
    const d = r.dados;
    const args = [origem, String(d.usuario || ''), String(d.senha || ''), String(d.totp || ''), true];
    d.senha = '';
    d.totp = '';
    r.dados = null;
    const feito = await injetar({ tabId: aba.id, documentIds: [documento] }, preencherNaPagina, args);
    args.fill('');
    const v = feito && feito.result;
    if (!v) {
      resultado = 'aba_mudou';
    } else if (!v.ok) {
      resultado =
        v.motivo === 'origem' || v.motivo === 'quadro'
          ? 'aba_mudou'
          : v.motivo === 'formulario' || v.motivo === 'sem_campo'
            ? 'sem_formulario'
            : 'falha';
    } else {
      resultado = await depoisDoEnvio(aba.id, documento, origem);
    }
  } catch {
    resultado = 'falha';
  } finally {
    if (resultado && porta === p) {
      await pedir(p, { v: 1, op: 'resultado_login', dados: { id, resultado } });
    }
  }
}

/** Liga o `ward login` (chamado na partida do service worker). */
export function iniciarLogin() {
  conectar();
}
