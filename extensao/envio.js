// Content script da Fase 42 (V6.8): vê o envio de um formulário com senha no quadro
// principal e avisa o service worker, que confere no daemon se o login é novo e oferece
// salvar. Roda no mundo isolado da extensão, só no quadro principal (`all_frames:
// false`) e só em http/https.
//
// O que ele NÃO faz (V6.6, S1):
// - não escuta `message`/`postMessage` nem nenhum evento que a página possa mandar;
// - não recebe nada do service worker (o envio não tem resposta) e não preenche nada;
// - não guarda nada (sem armazenamento da página nem da extensão).
//
// Envio forjado pela página: só conta o `submit` confiável (`isTrusted`) com ativação
// do usuário em curso (`navigator.userActivation.isActive`): um `requestSubmit()` do
// script da página sem clique ou tecla do usuário não oferece nada.
//
// DOM clobbering: os acessos ao DOM vão pelos protótipos do mundo isolado (como em
// `pagina.js`), que a página não alcança; `form.querySelectorAll`, `form.elements` e
// cia. podem ser tapados por campos com esse `name`.
(() => {
  'use strict';
  // D25: usuário e senha até 2 KiB; o daemon confere de novo.
  const MAX = 2048;
  const MINIMO = 4;
  const desc = (proto, nome) => Object.getOwnPropertyDescriptor(proto, nome);
  const pegar = (proto, nome) => {
    const d = desc(proto, nome);
    return (el) => d.get.call(el);
  };
  const tipoDe = pegar(HTMLInputElement.prototype, 'type');
  const valorDe = pegar(HTMLInputElement.prototype, 'value');
  const desabilitado = pegar(HTMLInputElement.prototype, 'disabled');
  const atributo = (el, nome) => Element.prototype.getAttribute.call(el, nome) || '';
  const dentro = (form, sel) =>
    Array.prototype.slice.call(Element.prototype.querySelectorAll.call(form, sel));
  const retangulo = (el) => Element.prototype.getBoundingClientRect.call(el);
  const antes = (a, b) =>
    (Node.prototype.compareDocumentPosition.call(a, b) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0;
  const ehForm = (el) => {
    try {
      return Object.prototype.toString.call(el) === '[object HTMLFormElement]';
    } catch {
      return false;
    }
  };

  // Só campo que o usuário vê (o mesmo critério do preenchimento, sem o "coberto", que
  // no envio não importa: o valor foi digitado nele).
  const visivel = (el) => {
    if (desabilitado(el)) return false;
    if (
      !Element.prototype.checkVisibility.call(el, {
        checkOpacity: true,
        checkVisibilityCSS: true,
      })
    ) {
      return false;
    }
    const r = retangulo(el);
    return r.width >= MINIMO && r.height >= MINIMO;
  };

  const texto = (v) => typeof v === 'string' && v.length > 0 && v.length <= MAX && !v.includes('\0');

  function aoEnviar(ev) {
    try {
      if (!ev.isTrusted || window !== window.top) return;
      const ativacao = navigator.userActivation;
      if (!ativacao || ativacao.isActive !== true) return;
      const form = ev.target;
      if (!ehForm(form)) return;
      const campos = dentro(form, 'input');
      const senhas = campos.filter(
        (c) => tipoDe(c) === 'password' && visivel(c) && valorDe(c) !== '',
      );
      if (senhas.length === 0) return;
      // Troca de senha ("atual" e "nova" diferentes): ambíguo, não oferece nada.
      const valores = new Set(senhas.map(valorDe));
      if (valores.size !== 1) return;
      const senha = valorDe(senhas[0]);
      const tiposUsuario = new Set(['text', 'email', 'tel']);
      const candidatos = campos.filter(
        (c) =>
          tiposUsuario.has(tipoDe(c)) &&
          antes(c, senhas[0]) &&
          visivel(c) &&
          valorDe(c) !== '',
      );
      const marcado = candidatos.filter((c) => atributo(c, 'autocomplete').split(/\s+/).includes('username'));
      const campoUsuario = marcado.length ? marcado[0] : candidatos[candidatos.length - 1];
      const usuario = campoUsuario ? valorDe(campoUsuario).trim() : '';
      if (!texto(usuario) || !texto(senha)) return;
      // Sem resposta e sem callback: o service worker confere quem mandou (aba, quadro
      // principal, origem) pelo `sender`, nunca pelo conteúdo da mensagem.
      chrome.runtime.sendMessage({ t: 'enviado', usuario, senha }).catch(() => {});
    } catch {
      // Página estranha: nada é oferecido.
    }
  }

  // Pelo protótipo: um `<img name="addEventListener">` tapa o `window.addEventListener`
  // (as propriedades com nome do `Window` vêm antes do `EventTarget.prototype`).
  EventTarget.prototype.addEventListener.call(window, 'submit', aoEnviar, true);
})();
