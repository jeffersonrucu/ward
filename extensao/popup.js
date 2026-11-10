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
// Fase 44c (D9): numa origem `http` (`localhost`/`127.0.0.1`) o daemon só responde
// depois de o usuário confirmar a origem na janela do Ward, e o popup fecha quando a
// janela pega o foco. Por isso, nessa origem, os passos 3 e 4 ficam com o service
// worker (`servico.js`): o popup manda a aba, o `documentId` da sonda, a entrada e a
// origem, e a senha nunca passa por aqui.
//
// Fase 42 (V6.8): se o service worker tem um login novo pendente para esta aba (o
// content script viu o envio e o daemon disse que ele não está no cofre), o popup
// mostra "Salvar login novo" no lugar das sugestões. A senha nunca chega ao popup: o
// service worker devolve só a origem e o usuário, e no "Salvar" o popup manda nome,
// usuário, grupo e ambiente; quem junta a senha e fala com o daemon é o service worker.
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
  // Fase 44c: a confirmação da origem `http` na janela do Ward.
  negado: 'O preenchimento foi negado na janela do Ward. Nada foi enviado à página.',
  expirado: 'A confirmação na janela do Ward expirou. Nada foi preenchido.',
  aprovacao_indisponivel:
    'O app do Ward não abriu para confirmar a origem. Nada foi preenchido.',
};

const AGUARDANDO_JANELA =
  'Endereço http desta máquina: confirme a origem na janela do Ward para preencher.';
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
  // Fase 44c: origem `http`, com a janela do Ward, pelo service worker.
  if (new URL(doc.origem).protocol === 'http:') {
    nota.textContent = AGUARDANDO_JANELA;
    nota.classList.remove('erro');
    const r = await pedirAoServico({
      t: 'preencher',
      aba: aba.id,
      documento: sonda.documentId,
      entrada: nome,
      origem: doc.origem,
    });
    if (r && r.ok === true) {
      window.close();
      return;
    }
    const codigo = r && typeof r.erro === 'string' ? r.erro : null;
    if (codigo === 'travado') {
      mostrarTravado(new URL(origem).host);
      return;
    }
    falhou(nota, botoes, codigo);
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

// --- Fase 42: "Salvar login novo" (frame 17414) -------------------------------------

/** Mensagem ao service worker da extensão; `null` se não respondeu. */
async function pedirAoServico(msg) {
  try {
    const r = await chrome.runtime.sendMessage(msg);
    return r === undefined ? null : r;
  } catch {
    return null;
  }
}

const ROTULOS_HOST = { dev: ['dev', 'desenv', 'local', 'localhost', 'test', 'teste'],
  stage: ['stage', 'staging', 'hml', 'homolog', 'homologacao', 'qa', 'uat'] };

/**
 * Sugestões de nome, grupo e ambiente a partir do host (o usuário confere e muda).
 * `painel.stage.clientex.test` → `clientex-stage-painel`, grupo `clientex`, `stage`;
 * `localhost:8083` → `localhost-8083`, grupo `localhost`, `dev`. Sem pista no host, o
 * ambiente sugerido é `prod` (o que mais destaca a aprovação).
 */
function sugestoesDoHost(origem) {
  const u = new URL(origem);
  const host = u.hostname.toLowerCase();
  const ip = /^[0-9.]+$/.test(host) || host.startsWith('[');
  let rotulos = ip ? [host.replace(/[^0-9a-z]+/g, '-')] : host.split('.').filter(Boolean);
  if (rotulos.length > 1) rotulos = rotulos.slice(0, -1);
  const invertidos = [...rotulos].reverse();
  const limpar = (t) => t.replace(/[^a-z0-9._-]+/g, '-').replace(/^[^a-z0-9]+/, '').slice(0, 64);
  const nome = limpar([...invertidos, ...(u.port ? [u.port] : [])].join('-'));
  const grupo = limpar(invertidos[0] || '');
  let ambiente = 'prod';
  if (ip || rotulos.some((r) => ROTULOS_HOST.dev.includes(r))) ambiente = 'dev';
  else if (rotulos.some((r) => ROTULOS_HOST.stage.includes(r))) ambiente = 'stage';
  return { nome, grupo, ambiente };
}

/** `Campo` (340): rótulo, entrada de texto e a linha de ajuda (oculta até um erro). */
function campoTexto(id, rotulo, valor) {
  const entrada = el('input', 'entrada texto-corpo');
  entrada.id = id;
  entrada.type = 'text';
  entrada.value = valor;
  entrada.autocomplete = 'off';
  entrada.spellcheck = false;
  const ajuda = texto('p', 'ajuda texto-legenda', '');
  ajuda.hidden = true;
  const r = texto('label', 'rotulo texto-rotulo', rotulo);
  r.htmlFor = id;
  return el('div', 'campo', [r, entrada, ajuda]);
}

/** `Select` (422): rótulo, `<select>` nativo com o chevron do design. */
function campoSelect(id, rotulo, opcoes, valor) {
  const sel = el('select', 'select texto-corpo');
  sel.id = id;
  for (const o of opcoes) {
    const op = texto('option', null, o);
    op.value = o;
    sel.append(op);
  }
  sel.value = valor;
  const r = texto('label', 'rotulo texto-rotulo', rotulo);
  r.htmlFor = id;
  const ajuda = texto('p', 'ajuda texto-legenda', '');
  ajuda.hidden = true;
  return el('div', 'campo', [r, el('div', 'entrada-select', [sel, icone('chevron-baixo', 'md')]), ajuda]);
}

const ROTULO_CAMPO = { nome: 'nome', grupo: 'grupo', ambiente: 'ambiente', usuario: 'usuario', senha: 'senha', url: 'url' };

const FALHAS_SALVAR = {
  expirou: 'O pedido para salvar expirou. Envie o formulário de novo.',
  pedido_invalido: 'Confira nome, usuário e ambiente.',
  sem_resposta: 'O Ward não respondeu. Nada foi salvo.',
};

/** Frame `Extensão / Salvar login novo` (17414; 18850 com o conteúdo inteiro). */
function mostrarSalvar(aba, origem, usuario) {
  const host = new URL(origem).host;
  const sug = sugestoesDoHost(origem);
  const nome = campoTexto('nome', 'Nome', sug.nome);
  const campoUsuario = campoTexto('usuario', 'Usuário', usuario);
  const senha = el('div', 'campo', [
    texto('span', 'rotulo texto-rotulo', 'Senha'),
    // Máscara fixa: nem o valor nem o tamanho da senha chegam ao popup.
    el('div', 'entrada entrada-mascara', [texto('span', 'mascara texto-corpo', '••••••••••••')]),
    texto('p', 'ajuda ajuda-fixa texto-legenda', 'Vai direto para o cofre; a extensão não mostra nem guarda.'),
  ]);
  const tipo = campoSelect('tipo', 'Tipo', ['login'], 'login');
  const ambiente = campoSelect('ambiente', 'Ambiente', ['dev', 'stage', 'prod'], sug.ambiente);
  const grupo = campoTexto('grupo', 'Grupo', sug.grupo);
  const campos = { nome, usuario: campoUsuario, grupo, ambiente };
  const nota = texto('p', 'nota erro texto-legenda', '');
  nota.hidden = true;

  const cancelar = el('button', 'botao secundario-botao', [texto('span', 'texto-corpo-forte', 'Agora não')]);
  cancelar.type = 'button';
  const salvarBotao = el('button', 'botao primario', [texto('span', 'texto-corpo-forte', 'Salvar no Ward')]);
  salvarBotao.type = 'submit';

  const form = el('form', 'popup-salvar', [
    el('section', 'corpo-salvar', [
      el('div', 'introducao', [
        texto('h1', 'titulo-salvar titulo-modal', 'Salvar este login no Ward?'),
        texto('p', 'texto-salvar texto-pequeno',
          `O formulário de ${host} foi enviado com um login que ainda não está no cofre.`),
      ]),
      nome,
      campoUsuario,
      senha,
      el('div', 'linha-dupla', [tipo, ambiente]),
      grupo,
      el('div', 'nivel-inicial', [
        icone('cadeado', 'md'),
        el('div', 'nivel-textos', [
          texto('span', 'nivel-titulo texto-corpo-forte', 'Nível inicial: com aprovação'),
          texto('span', 'nivel-texto texto-legenda',
            'O Claude só usa depois que você aprovar. Para mudar, use o app, com PIN.'),
        ]),
      ]),
      nota,
    ]),
    el('footer', 'rodape rodape-salvar', [cancelar, salvarBotao]),
  ]);
  form.noValidate = true;

  const valor = (c) => c.querySelector('input, select').value;
  const limparErros = () => {
    nota.hidden = true;
    for (const c of Object.values(campos)) {
      c.classList.remove('com-erro');
      const a = c.querySelector('.ajuda');
      a.hidden = true;
      a.textContent = '';
    }
  };

  // O service worker fica vivo enquanto o popup está aberto (ele guarda o pendente).
  const manter = setInterval(() => pedirAoServico({ t: 'manter', aba: aba.id }), 20000);

  cancelar.addEventListener('click', async () => {
    clearInterval(manter);
    await pedirAoServico({ t: 'descartar', aba: aba.id });
    window.close();
  });

  form.addEventListener('submit', async (ev) => {
    ev.preventDefault();
    limparErros();
    salvarBotao.disabled = true;
    cancelar.disabled = true;
    const r = await pedirAoServico({
      t: 'salvar',
      aba: aba.id,
      nome: valor(nome).trim(),
      usuario: valor(campoUsuario).trim(),
      grupo: valor(grupo).trim(),
      ambiente: valor(ambiente),
    });
    salvarBotao.disabled = false;
    cancelar.disabled = false;
    if (r && r.ok === true) {
      clearInterval(manter);
      window.close();
      return;
    }
    const codigo = r && typeof r.erro === 'string' ? r.erro : 'sem_resposta';
    if (codigo === 'travado') {
      clearInterval(manter);
      mostrarTravado(host);
      return;
    }
    let geral = codigo === 'validacao' ? '' : (FALHAS_SALVAR[codigo] ?? 'Não deu para salvar. Nada foi gravado.');
    for (const p of (r && Array.isArray(r.problemas) ? r.problemas : [])) {
      const c = campos[ROTULO_CAMPO[p.campo]];
      if (c) {
        c.classList.add('com-erro');
        const a = c.querySelector('.ajuda');
        a.textContent = p.motivo;
        a.hidden = false;
      } else if (!geral) {
        geral = `${p.campo}: ${p.motivo}`;
      }
    }
    if (geral) {
      nota.textContent = geral;
      nota.hidden = false;
    }
  });

  mostrar(cabecalho('destravado'), form);
  raiz.classList.add('com-formulario');
}

async function iniciar() {
  let aba = null;
  try {
    [aba] = await chrome.tabs.query({ active: true, currentWindow: true });
  } catch {
    aba = null;
  }
  const pendente = aba ? await pedirAoServico({ t: 'pendente', aba: aba.id }) : null;
  if (pendente && typeof pendente.origem === 'string' && typeof pendente.usuario === 'string') {
    const o = origemDa(pendente.origem);
    if (o) {
      mostrarSalvar(aba, o, pendente.usuario);
      return;
    }
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
