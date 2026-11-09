// Popup da extensão (Fase 41; D5, D9; V6.2, V6.4–V6.7): "Sugestões do domínio".
//
// Fluxo, todo dentro do popup (não há service worker, content script fixo nem ouvinte de
// mensagem; a página não tem com quem falar, V6.6):
//  1. a origem da aba ativa sai do `tab.url` (permissão `activeTab`, dada pelo clique no
//     ícone) e vai ao `ward-nm` em `sugerir`; quem compara com a URL das entradas é o
//     daemon (D9);
//  2. no clique numa entrada, uma sonda roda no quadro principal (`frameIds: [0]`) e
//     devolve a origem do documento e o `documentId` dele; a origem tem de ser a mesma
//     do passo 1;
//  3. `preencher` vai ao daemon com essa origem; a resposta (senha em base64) segue
//     direto como argumento para o preenchimento, injetado só naquele `documentId` (se a
//     aba navegou, o Chrome recusa) e que confere a origem de novo antes de escrever;
//  4. a resposta é descartada e o popup fecha (o contexto dele some).
//
// Nada é guardado: sem a permissão `storage` (a API de armazenamento da extensão nem
// existe aqui), sem armazenamento da página, nada no DOM do popup além de nome, usuário
// e ambiente. Os textos vêm
// do daemon e entram só por `textContent` (nunca HTML).

import { ICONES } from './icones.js';
import { SIMBOLO } from './marca.js';
import { preencherNaPagina, sondar } from './pagina.js';

const HOST = 'br.ward.nm';
const SVG = 'http://www.w3.org/2000/svg';
const raiz = document.getElementById('popup');

/** Elemento com classes e filhos (texto entra como nó de texto, nunca HTML). */
function el(tag, classes, filhos = []) {
  const e = document.createElement(tag);
  if (classes) e.className = classes;
  for (const f of filhos) e.append(f);
  return e;
}

function texto(tag, classes, conteudo) {
  const e = el(tag, classes);
  e.textContent = conteudo;
  return e;
}

/** `Ícone/<nome>` (20×20, traço 1,5) no tom pedido. */
function icone(nome, tamanho, tom) {
  const svg = document.createElementNS(SVG, 'svg');
  svg.setAttribute('class', `icone ${tamanho}`);
  svg.setAttribute('viewBox', '0 0 20 20');
  svg.setAttribute('fill', 'none');
  svg.setAttribute('aria-hidden', 'true');
  if (tom) svg.setAttribute('data-tom-icone', tom);
  for (const c of ICONES[nome] ?? []) {
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

/** Símbolo do Ward (`Logo/símbolo`), na cor do texto do elemento. */
function simbolo(classe) {
  const svg = document.createElementNS(SVG, 'svg');
  svg.setAttribute('class', classe);
  svg.setAttribute('viewBox', SIMBOLO.caixa);
  svg.setAttribute('aria-hidden', 'true');
  const p = document.createElementNS(SVG, 'path');
  p.setAttribute('transform', SIMBOLO.t);
  p.setAttribute('d', SIMBOLO.d);
  p.setAttribute('fill', 'currentColor');
  svg.append(p);
  return svg;
}

/** `Cabeçalho do popup` (17234): destravado ou travado. */
function cabecalho(estado) {
  const filhos = [simbolo('simbolo'), texto('span', 'nome-produto titulo-secao', 'ward')];
  if (estado) filhos.push(texto('span', `estado ${estado} texto-selo`, estado));
  return el('header', 'cabecalho', filhos);
}

/** Native messaging com o `ward-nm`; `null` se o host não respondeu. */
async function enviar(pedido) {
  try {
    const r = await chrome.runtime.sendNativeMessage(HOST, pedido);
    return r && typeof r === 'object' ? r : null;
  } catch {
    return null;
  }
}

const codigoDe = (r) => (r && r.tipo === 'erro' && r.dados ? r.dados.codigo : null);

/** Só `http:`/`https:` têm origem para preencher; o resto (chrome:, file:, about:) não. */
function origemDa(url) {
  try {
    const u = new URL(url);
    if (u.protocol !== 'https:' && u.protocol !== 'http:') return null;
    return u.origin;
  } catch {
    return null;
  }
}

function botaoAbrirWard(classe, rotulo, comSimbolo) {
  const filhos = comSimbolo ? [simbolo('simbolo-botao')] : [];
  filhos.push(texto('span', 'texto-corpo-forte', rotulo));
  const b = el('button', `botao ${classe}`, filhos);
  b.type = 'button';
  b.addEventListener('click', async () => {
    b.disabled = true;
    await enviar({ v: 1, op: 'abrir_app' });
    window.close();
  });
  return b;
}

function mostrar(...filhos) {
  raiz.replaceChildren(...filhos);
}

/** Frame `Extensão / Cofre travado` (17392). */
function mostrarTravado(host) {
  const alvo = host ? `as entradas de ${host}` : 'as entradas desta aba';
  mostrar(
    cabecalho('travado'),
    el('section', 'corpo-travado', [
      el('div', 'avatar-grande', [icone('cadeado', 'md')]),
      texto('h1', 'titulo-travado titulo-modal', 'O cofre está travado'),
      texto('p', 'texto-travado texto-corpo', `Destrave no app do Ward para ver ${alvo}.`),
      botaoAbrirWard('primario', 'Abrir o Ward para destravar', false),
      texto('p', 'nota texto-legenda', 'A extensão nunca pede a senha mestra.'),
    ]),
  );
}

function mostrarAviso(mensagem) {
  mostrar(
    cabecalho(null),
    el('section', 'corpo', [texto('p', 'nota texto-pequeno', mensagem)]),
    el('footer', 'rodape', [botaoAbrirWard('secundario-botao', 'Abrir o Ward', true)]),
  );
}

const NOTA =
  'Clique numa entrada para preencher usuário, senha e TOTP nesta aba. Só aparecem entradas desta origem, e nada fica salvo no navegador.';

const NOTA_VAZIA =
  'Só aparecem entradas com a mesma origem desta aba (esquema, endereço e porta), e nada fica salvo no navegador.';

const FALHAS = {
  origem: 'A aba mudou de endereço. Abra o Ward de novo nesta página.',
  quadro: 'A aba mudou de endereço. Abra o Ward de novo nesta página.',
  sem_campo: 'Não achei um campo de senha visível nesta página. Nada foi preenchido.',
  origem_diferente: 'A origem da aba não confere com a da entrada. Nada foi preenchido.',
  entrada_inexistente: 'Essa entrada não existe mais no cofre.',
};
const FALHA_PADRAO = 'Não deu para preencher. Nada foi enviado à página.';

/** Frame `Extensão / Sugestões do domínio` (17332). */
function mostrarSugestoes(aba, origem, itens) {
  const nota = texto('p', 'nota texto-legenda', itens.length ? NOTA : NOTA_VAZIA);
  const n = itens.length;
  const rotulo =
    n === 0
      ? 'Nenhuma entrada para esta origem'
      : `${n} ${n === 1 ? 'entrada' : 'entradas'} para esta origem`;
  const botoes = [];
  const lista = el(
    'ul',
    'lista',
    itens.map((s) => {
      const detalhe = s.totp ? `${s.usuario} · com TOTP` : s.usuario;
      const b = el('button', 'item', [
        el('span', 'avatar', [icone('tipo-login', 'md')]),
        el('span', 'item-textos', [
          el('span', 'linha', [
            texto('span', 'item-nome texto-corpo-forte', s.nome),
            texto('span', `badge-ambiente ${s.ambiente} texto-selo`, s.ambiente),
          ]),
          el('span', 'linha', [
            el('span', 'badge-tipo texto-etiqueta', [icone('tipo-login', 'xs'), 'login']),
            texto('span', 'item-detalhe texto-pequeno', detalhe),
          ]),
        ]),
      ]);
      b.type = 'button';
      b.dataset.entrada = s.nome;
      b.addEventListener('click', () => preencher(aba, origem, s.nome, botoes, nota));
      botoes.push(b);
      return el('li', null, [b]);
    }),
  );
  mostrar(
    cabecalho('destravado'),
    el('section', 'corpo', [
      el('div', 'origem', [
        icone('cadeado', 'md'),
        el('div', 'origem-textos', [
          texto('span', 'origem-rotulo texto-legenda', 'Origem da aba'),
          texto('span', 'origem-valor mono-pequeno', origem),
        ]),
      ]),
      texto('p', 'nota secundario texto-rotulo', rotulo),
      ...(n ? [lista] : []),
      nota,
    ]),
    el('footer', 'rodape', [botaoAbrirWard('secundario-botao', 'Abrir o Ward', true)]),
  );
}

function falhou(nota, botoes, motivo) {
  nota.textContent = FALHAS[motivo] ?? FALHA_PADRAO;
  nota.classList.add('erro');
  for (const b of botoes) b.disabled = false;
}

async function preencher(aba, origem, nome, botoes, nota) {
  for (const b of botoes) b.disabled = true;
  // 2. Sonda: o documento do quadro principal, a origem dele e o id do documento.
  let sonda;
  try {
    [sonda] = await chrome.scripting.executeScript({
      target: { tabId: aba.id, frameIds: [0] },
      world: 'ISOLATED',
      func: sondar,
    });
  } catch {
    falhou(nota, botoes, 'origem');
    return;
  }
  const doc = sonda && sonda.result;
  if (!doc || doc.topo !== true || doc.origem !== origem || !sonda.documentId) {
    falhou(nota, botoes, 'origem');
    return;
  }
  // 3. O daemon compara a origem do documento com a da entrada (D9).
  let r = await enviar({ v: 1, op: 'preencher', dados: { entrada: nome, origem: doc.origem } });
  if (!r || r.tipo !== 'preenchimento' || !r.dados) {
    const codigo = codigoDe(r);
    r = null;
    if (codigo === 'travado') {
      mostrarTravado(new URL(origem).host);
      return;
    }
    falhou(nota, botoes, codigo);
    return;
  }
  let resultado = null;
  try {
    const d = r.dados;
    [resultado] = await chrome.scripting.executeScript({
      target: { tabId: aba.id, documentIds: [sonda.documentId] },
      world: 'ISOLATED',
      func: preencherNaPagina,
      args: [doc.origem, String(d.usuario ?? ''), String(d.senha ?? ''), String(d.totp ?? '')],
    });
  } catch {
    resultado = null;
  } finally {
    // 4. A resposta com a senha não fica em lugar nenhum do popup.
    if (r.dados) {
      r.dados.senha = '';
      r.dados.totp = '';
      r.dados.usuario = '';
    }
    r = null;
  }
  const res = resultado && resultado.result;
  if (!res || res.ok !== true) {
    falhou(nota, botoes, res && res.motivo);
    return;
  }
  window.close();
}

async function iniciar() {
  let aba = null;
  try {
    [aba] = await chrome.tabs.query({ active: true, currentWindow: true });
  } catch {
    aba = null;
  }
  const origem = aba && aba.url ? origemDa(aba.url) : null;
  if (!aba || !origem) {
    mostrarAviso('Esta aba não tem uma origem http ou https para preencher.');
    return;
  }
  // 1. Sugestões da origem da aba; o daemon compara (D9).
  const r = await enviar({ v: 1, op: 'sugerir', dados: { origem } });
  if (r && r.tipo === 'sugestoes' && r.dados && Array.isArray(r.dados.itens)) {
    const itens = r.dados.itens.filter(
      (s) =>
        s &&
        typeof s.nome === 'string' &&
        typeof s.usuario === 'string' &&
        ['dev', 'stage', 'prod'].includes(s.ambiente),
    );
    mostrarSugestoes(aba, origem, itens);
    return;
  }
  const codigo = codigoDe(r);
  if (codigo === 'travado') {
    mostrarTravado(new URL(origem).host);
  } else if (codigo === 'nao_autorizado') {
    mostrarAviso('O Ward não reconheceu esta extensão. Confira a instalação do Ward.');
  } else {
    mostrarAviso('O Ward não respondeu. Confira se o Ward está instalado e rodando.');
  }
}

iniciar();
