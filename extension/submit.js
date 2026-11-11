// Content script of Phase 42 (V6.8): sees the submit of a form with a password in the
// main frame and tells the service worker, which asks the daemon whether the login is
// new and offers to save it. Runs in the extension's isolated world, only in the main
// frame (`all_frames: false`) and only on http/https.
//
// What it does NOT do (V6.6, S1):
// - it does not listen to `message`/`postMessage` nor to any event the page can send;
// - it receives nothing from the service worker (the submit has no reply) and fills
//   nothing;
// - it keeps nothing (no page storage nor extension storage).
//
// Submit forged by the page: only a trusted `submit` (`isTrusted`) with user activation
// in progress (`navigator.userActivation.isActive`) counts: a `requestSubmit()` from the
// page script without a user click or key press offers nothing.
//
// DOM clobbering: the DOM accesses go through the isolated world's prototypes (as in
// `page.js`), which the page cannot reach; `form.querySelectorAll`, `form.elements` and
// the like can be shadowed by fields with that `name`.
(() => {
  'use strict';
  // D25: username and password up to 2 KiB; the daemon checks again.
  const MAX = 2048;
  const MINIMUM = 4;
  const desc = (proto, name) => Object.getOwnPropertyDescriptor(proto, name);
  const getter = (proto, name) => {
    const d = desc(proto, name);
    return (el) => d.get.call(el);
  };
  const typeOf = getter(HTMLInputElement.prototype, 'type');
  const valueOf = getter(HTMLInputElement.prototype, 'value');
  const disabled = getter(HTMLInputElement.prototype, 'disabled');
  const attribute = (el, name) => Element.prototype.getAttribute.call(el, name) || '';
  const within = (form, sel) =>
    Array.prototype.slice.call(Element.prototype.querySelectorAll.call(form, sel));
  const rect = (el) => Element.prototype.getBoundingClientRect.call(el);
  const before = (a, b) =>
    (Node.prototype.compareDocumentPosition.call(a, b) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0;
  const isForm = (el) => {
    try {
      return Object.prototype.toString.call(el) === '[object HTMLFormElement]';
    } catch {
      return false;
    }
  };

  // Only a field the user sees (the same rule as the fill, without the "covered" one,
  // which does not matter on submit: the value was typed into it).
  const visible = (el) => {
    if (disabled(el)) return false;
    if (
      !Element.prototype.checkVisibility.call(el, {
        checkOpacity: true,
        checkVisibilityCSS: true,
      })
    ) {
      return false;
    }
    const r = rect(el);
    return r.width >= MINIMUM && r.height >= MINIMUM;
  };

  const isText = (v) => typeof v === 'string' && v.length > 0 && v.length <= MAX && !v.includes('\0');

  function onSubmit(ev) {
    try {
      if (!ev.isTrusted || window !== window.top) return;
      const activation = navigator.userActivation;
      if (!activation || activation.isActive !== true) return;
      const form = ev.target;
      if (!isForm(form)) return;
      const inputs = within(form, 'input');
      const passwords = inputs.filter(
        (c) => typeOf(c) === 'password' && visible(c) && valueOf(c) !== '',
      );
      if (passwords.length === 0) return;
      // Password change ("current" and "new" differ): ambiguous, offers nothing.
      const values = new Set(passwords.map(valueOf));
      if (values.size !== 1) return;
      const password = valueOf(passwords[0]);
      const usernameTypes = new Set(['text', 'email', 'tel']);
      const candidates = inputs.filter(
        (c) =>
          usernameTypes.has(typeOf(c)) &&
          before(c, passwords[0]) &&
          visible(c) &&
          valueOf(c) !== '',
      );
      const marked = candidates.filter((c) => attribute(c, 'autocomplete').split(/\s+/).includes('username'));
      const usernameField = marked.length ? marked[0] : candidates[candidates.length - 1];
      const username = usernameField ? valueOf(usernameField).trim() : '';
      if (!isText(username) || !isText(password)) return;
      // No reply and no callback: the service worker checks who sent it (tab, main
      // frame, origin) by the `sender`, never by the message content.
      chrome.runtime.sendMessage({ t: 'enviado', 'usuario': username, 'senha': password }).catch(() => {});
    } catch {
      // Odd page: nothing is offered.
    }
  }

  // Through the prototype: an `<img name="addEventListener">` shadows
  // `window.addEventListener` (the named properties of `Window` come before
  // `EventTarget.prototype`).
  EventTarget.prototype.addEventListener.call(window, 'submit', onSubmit, true);
})();
