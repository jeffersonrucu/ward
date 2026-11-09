// Funções que o popup injeta na aba com `chrome.scripting.executeScript` (Fase 41; D9;
// V6.4, V6.6). Rodam no mundo isolado da extensão, só no quadro principal: a página
// não vê nem altera as variáveis daqui, e nenhuma delas deixa ouvinte de mensagem
// (`postMessage` ou `chrome.runtime`) para trás.
//
// Cada função é serializada pelo Chrome (`func.toString()`): não pode usar nada de fora
// dela. Por isso as duas repetem os mesmos auxiliares.
//
// DOM clobbering: `document.querySelectorAll`, `form.elements`, `input.form` e cia. podem
// ser trocados por elementos com `name`/`id` iguais (os getters com nome do `Document` e
// do `HTMLFormElement` passam na frente dos métodos). Aqui todo acesso ao DOM vai pelos
// protótipos do mundo isolado (`Document.prototype.querySelectorAll.call(...)`,
// descritores de `HTMLInputElement.prototype`), que a página não alcança.

/** Sonda o documento do quadro principal: a origem dele e se é mesmo o topo. */
export function sondar() {
  try {
    return {
      origem: String(window.location.origin),
      topo: window === window.top,
    };
  } catch {
    return { origem: '', topo: false };
  }
}

/**
 * Preenche usuário, senha e TOTP no documento do quadro principal, só se a origem dele
 * for a esperada. Devolve só o que preencheu (nunca valores). A senha chega em base64
 * (como no protocolo); os bytes decodificados são zerados logo depois de virar texto.
 */
export function preencherNaPagina(origemEsperada, usuario, senhaB64, totp) {
  const desc = (proto, nome) => Object.getOwnPropertyDescriptor(proto, nome);
  const pegar = (proto, nome) => {
    const d = desc(proto, nome);
    return (el) => d.get.call(el);
  };
  const tipoDe = pegar(HTMLInputElement.prototype, 'type');
  const formDe = pegar(HTMLInputElement.prototype, 'form');
  const desabilitado = pegar(HTMLInputElement.prototype, 'disabled');
  const soLeitura = pegar(HTMLInputElement.prototype, 'readOnly');
  const setValor = desc(HTMLInputElement.prototype, 'value').set;
  const atributo = (el, nome) => Element.prototype.getAttribute.call(el, nome) || '';
  const todos = (sel) =>
    Array.prototype.slice.call(Document.prototype.querySelectorAll.call(document, sel));
  const retangulo = (el) => Element.prototype.getBoundingClientRect.call(el);
  const estilo = (el) => window.getComputedStyle(el);
  const antes = (a, b) =>
    (Node.prototype.compareDocumentPosition.call(a, b) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0;
  const disparar = (el, tipo) =>
    EventTarget.prototype.dispatchEvent.call(el, new Event(tipo, { bubbles: true }));

  // Campo de senha escondido (armadilha de preenchimento automático): só entra campo
  // que o usuário vê. Fora: `display`/`visibility`/`opacity` em qualquer ancestral,
  // menor que 4×4, fora da página, recortado por `clip`/`clip-path`, coberto por outro
  // elemento no centro (quando o centro está na tela), desabilitado ou só leitura.
  const MINIMO = 4;
  const visivel = (el) => {
    if (desabilitado(el) || soLeitura(el)) return false;
    if (
      !Element.prototype.checkVisibility.call(el, {
        checkOpacity: true,
        checkVisibilityCSS: true,
        contentVisibilityAuto: true,
      })
    ) {
      return false;
    }
    const r = retangulo(el);
    if (r.width < MINIMO || r.height < MINIMO) return false;
    const raiz = pegar(Document.prototype, 'documentElement')(document);
    const largura = Math.max(raiz.scrollWidth, window.innerWidth);
    const altura = Math.max(raiz.scrollHeight, window.innerHeight);
    const x0 = r.left + window.scrollX;
    const y0 = r.top + window.scrollY;
    if (x0 + r.width <= 0 || y0 + r.height <= 0 || x0 >= largura || y0 >= altura) return false;
    const s = estilo(el);
    if (s.clipPath !== 'none' || (s.clip && s.clip !== 'auto')) return false;
    const cx = r.left + r.width / 2;
    const cy = r.top + r.height / 2;
    if (cx >= 0 && cy >= 0 && cx < window.innerWidth && cy < window.innerHeight) {
      const topo = Document.prototype.elementFromPoint.call(document, cx, cy);
      if (topo !== el && !(topo && Node.prototype.contains.call(el, topo))) {
        // Um `<label>` por cima do próprio campo é comum; outro elemento, não.
        const rotulo = topo && Element.prototype.closest.call(topo, 'label');
        const controla = rotulo && pegar(HTMLLabelElement.prototype, 'control')(rotulo);
        if (controla !== el) return false;
      }
    }
    return true;
  };

  const escrever = (el, valor) => {
    setValor.call(el, valor);
    disparar(el, 'input');
    disparar(el, 'change');
  };

  let bytes = null;
  try {
    if (window !== window.top) return { ok: false, motivo: 'quadro' };
    if (String(window.location.origin) !== origemEsperada) return { ok: false, motivo: 'origem' };

    const campos = todos('input').filter((el) => visivel(el));
    const senhas = campos.filter((el) => tipoDe(el) === 'password');
    if (senhas.length === 0) return { ok: false, motivo: 'sem_campo' };
    const senhaCampo = senhas[0];
    const form = formDe(senhaCampo);
    const mesmoForm = (el) => formDe(el) === form;

    const TEXTO = ['text', 'email', 'tel', ''];
    const eTotp = (el) => {
      const ac = atributo(el, 'autocomplete').toLowerCase();
      if (ac.split(/\s+/).includes('one-time-code')) return true;
      const nome = `${atributo(el, 'name')} ${atributo(el, 'id')}`.toLowerCase();
      return /(^|[^a-z])(otp|totp|2fa|mfa|codigo|code|token)([^a-z]|$)/.test(nome);
    };
    const textos = campos.filter(
      (el) => TEXTO.includes(tipoDe(el)) && mesmoForm(el) && !eTotp(el),
    );
    const antesDaSenha = textos.filter((el) => antes(el, senhaCampo));
    const usuarioCampo =
      antesDaSenha.find((el) => /\busername\b/.test(atributo(el, 'autocomplete').toLowerCase())) ||
      antesDaSenha[antesDaSenha.length - 1] ||
      null;
    const totpCampo = totp
      ? campos.find(
          (el) =>
            ['text', 'tel', 'number', ''].includes(tipoDe(el)) && mesmoForm(el) && eTotp(el),
        ) || null
      : null;

    const bin = atob(senhaB64);
    bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i += 1) bytes[i] = bin.charCodeAt(i);
    const senha = new TextDecoder('utf-8', { fatal: true }).decode(bytes);

    if (usuarioCampo && usuario) escrever(usuarioCampo, usuario);
    escrever(senhaCampo, senha);
    if (totpCampo) escrever(totpCampo, totp);
    return { ok: true, usuario: Boolean(usuarioCampo && usuario), totp: Boolean(totpCampo) };
  } catch {
    return { ok: false, motivo: 'falha' };
  } finally {
    if (bytes) bytes.fill(0);
    bytes = null;
    senhaB64 = '';
    totp = '';
    usuario = '';
  }
}
