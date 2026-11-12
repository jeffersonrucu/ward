// Functions the popup injects into the tab with `chrome.scripting.executeScript`
// (Phase 41; D9; V6.4, V6.6). They run in the extension's isolated world, only in the
// main frame: the page neither sees nor changes the variables here, and none of them
// leaves a message listener (`postMessage` or `chrome.runtime`) behind.
//
// Chrome serializes each function (`func.toString()`): it cannot use anything from
// outside itself. That is why they repeat the same helpers.
//
// DOM clobbering: page elements whose `name`/`id` equals a property shadow it. The
// named properties of the `<form>` reach the isolated world (an `<input name="action">`
// replaces `form.action`, an `<input name="elements">` replaces `form.elements`); those
// of the `document` do not reach it in today's Chrome (an `<img name="querySelectorAll">`
// does not replace `document.querySelectorAll` here, checked in Phase 44a). The code does
// not rely on that: every DOM access goes through the isolated world's prototypes
// (`Document.prototype.querySelectorAll.call(...)`, descriptors of
// `HTMLFormElement.prototype` and `HTMLInputElement.prototype`), which the page cannot
// reach.

/**
 * Phase 44e (D9): does the fill for this origin go through the Ward window? Yes for
 * `http` and for every host of this machine in any scheme (`localhost`, `*.localhost`,
 * `127.x.x.x`, `0.x.x.x` and every IPv6 literal). This one is not injected: only the
 * popup and the service worker use it to pick the path. The daemon decides (the local
 * origin check of the entry URL in `ward_core`); the rule here is wider on purpose: the
 * service worker path serves any origin (it only waits longer), and the direct path on
 * an origin the daemon confirms would lose the fill (the popup closes when the window
 * takes the focus).
 */
export function needsWindow(origin) {
  let u;
  try {
    u = new URL(origin);
  } catch {
    return false;
  }
  if (u.protocol === 'http:') return true;
  if (u.protocol !== 'https:') return false;
  const h = u.hostname;
  if (h === 'localhost' || h.endsWith('.localhost')) return true;
  if (h.startsWith('[')) return true;
  const m = /^([0-9]{1,3})\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}$/.exec(h);
  return m !== null && (m[1] === '127' || m[1] === '0');
}

/** Probes the main frame document: its origin and whether it really is the top. */
export function probe() {
  try {
    return {
      origin: String(window.location.origin),
      top: window === window.top,
    };
  } catch {
    return { origin: '', top: false };
  }
}

/**
 * After the `ward login` submit (Phase 43): the origin of the main frame document,
 * whether it has finished loading and whether a visible password field is still there
 * (the site refused the login). No value is read.
 */
export function probeAfterSubmit() {
  try {
    const typeOf = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'type').get;
    const readyState = Object.getOwnPropertyDescriptor(Document.prototype, 'readyState').get;
    const inputs = Array.prototype.slice.call(
      Document.prototype.querySelectorAll.call(document, 'input'),
    );
    const password = inputs.some((el) => {
      if (typeOf.call(el) !== 'password') return false;
      if (
        !Element.prototype.checkVisibility.call(el, { checkOpacity: true, checkVisibilityCSS: true })
      ) {
        return false;
      }
      const r = Element.prototype.getBoundingClientRect.call(el);
      return r.width >= 4 && r.height >= 4;
    });
    return {
      origin: String(window.location.origin),
      top: window === window.top,
      ready: readyState.call(document) === 'complete',
      password,
    };
  } catch {
    return { origin: '', top: false, ready: false, password: false };
  }
}

/**
 * Fills username, password and TOTP in the main frame document, only if its origin is
 * the expected one. Returns only what it filled (never values). The password arrives in
 * base64 (as in the protocol); the decoded bytes are zeroed right after becoming text.
 *
 * `submit` (only `ward login`, Phase 43): before writing, it requires a login form that
 * can be submitted safely: exactly one visible password field, inside a `<form>` with
 * method POST, destination (`action` and the button's `formaction`) on the same origin
 * and no `target` to another context, and the username field when there is a username.
 * Then it submits (`requestSubmit`, which builds the submitted data right away) and
 * clears the fields.
 */
export function fillInPage(expectedOrigin, username, passwordB64, totp, submit) {
  const desc = (proto, name) => Object.getOwnPropertyDescriptor(proto, name);
  const getter = (proto, name) => {
    const d = desc(proto, name);
    return (el) => d.get.call(el);
  };
  const typeOf = getter(HTMLInputElement.prototype, 'type');
  const formOf = getter(HTMLInputElement.prototype, 'form');
  const disabled = getter(HTMLInputElement.prototype, 'disabled');
  const readOnly = getter(HTMLInputElement.prototype, 'readOnly');
  const setValue = desc(HTMLInputElement.prototype, 'value').set;
  const attribute = (el, name) => Element.prototype.getAttribute.call(el, name) || '';
  const all = (sel) =>
    Array.prototype.slice.call(Document.prototype.querySelectorAll.call(document, sel));
  const rect = (el) => Element.prototype.getBoundingClientRect.call(el);
  const style = (el) => window.getComputedStyle(el);
  const before = (a, b) =>
    (Node.prototype.compareDocumentPosition.call(a, b) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0;
  const dispatch = (el, type) =>
    EventTarget.prototype.dispatchEvent.call(el, new Event(type, { bubbles: true }));

  // Hidden password field (autofill trap): only a field the user sees counts. Out:
  // `display`/`visibility`/`opacity` on any ancestor, smaller than 4×4, outside the page,
  // clipped by `clip`/`clip-path`, covered by another element at the center (when the
  // center is on screen), disabled or read-only.
  const MINIMUM = 4;
  const visible = (el) => {
    if (disabled(el) || readOnly(el)) return false;
    if (
      !Element.prototype.checkVisibility.call(el, {
        checkOpacity: true,
        checkVisibilityCSS: true,
        contentVisibilityAuto: true,
      })
    ) {
      return false;
    }
    const r = rect(el);
    if (r.width < MINIMUM || r.height < MINIMUM) return false;
    const root = getter(Document.prototype, 'documentElement')(document);
    const width = Math.max(root.scrollWidth, window.innerWidth);
    const height = Math.max(root.scrollHeight, window.innerHeight);
    const x0 = r.left + window.scrollX;
    const y0 = r.top + window.scrollY;
    if (x0 + r.width <= 0 || y0 + r.height <= 0 || x0 >= width || y0 >= height) return false;
    const s = style(el);
    if (s.clipPath !== 'none' || (s.clip && s.clip !== 'auto')) return false;
    const cx = r.left + r.width / 2;
    const cy = r.top + r.height / 2;
    if (cx >= 0 && cy >= 0 && cx < window.innerWidth && cy < window.innerHeight) {
      const hit = Document.prototype.elementFromPoint.call(document, cx, cy);
      if (hit !== el && !(hit && Node.prototype.contains.call(el, hit))) {
        // A `<label>` over its own field is common; another element is not.
        const label = hit && Element.prototype.closest.call(hit, 'label');
        const controls = label && getter(HTMLLabelElement.prototype, 'control')(label);
        if (controls !== el) return false;
      }
    }
    return true;
  };

  const write = (el, value) => {
    setValue.call(el, value);
    dispatch(el, 'input');
    dispatch(el, 'change');
  };

  let bytes = null;
  try {
    if (window !== window.top) return { ok: false, reason: 'frame' };
    if (String(window.location.origin) !== expectedOrigin) return { ok: false, reason: 'origin' };

    const inputs = all('input').filter((el) => visible(el));
    const passwords = inputs.filter((el) => typeOf(el) === 'password');
    if (passwords.length === 0) return { ok: false, reason: 'no_field' };
    const passwordField = passwords[0];
    const form = formOf(passwordField);
    const sameForm = (el) => formOf(el) === form;

    const TEXT = ['text', 'email', 'tel', ''];
    const isTotp = (el) => {
      const ac = attribute(el, 'autocomplete').toLowerCase();
      if (ac.split(/\s+/).includes('one-time-code')) return true;
      const name = `${attribute(el, 'name')} ${attribute(el, 'id')}`.toLowerCase();
      return /(^|[^a-z])(otp|totp|2fa|mfa|codigo|code|token)([^a-z]|$)/.test(name);
    };
    const texts = inputs.filter(
      (el) => TEXT.includes(typeOf(el)) && sameForm(el) && !isTotp(el),
    );
    const beforePassword = texts.filter((el) => before(el, passwordField));
    const usernameField =
      beforePassword.find((el) => /\busername\b/.test(attribute(el, 'autocomplete').toLowerCase())) ||
      beforePassword[beforePassword.length - 1] ||
      null;
    const totpField = totp
      ? inputs.find(
          (el) =>
            ['text', 'tel', 'number', ''].includes(typeOf(el)) && sameForm(el) && isTotp(el),
        ) || null
      : null;

    const bin = atob(passwordB64);
    bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i += 1) bytes[i] = bin.charCodeAt(i);
    const password = new TextDecoder('utf-8', { fatal: true }).decode(bytes);

    let doSubmit = null;
    if (submit === true) {
      if (passwords.length !== 1 || !form) return { ok: false, reason: 'form' };
      if (username && !usernameField) return { ok: false, reason: 'form' };
      const F = HTMLFormElement.prototype;
      const ofForm = (k) => desc(F, k).get.call(form);
      const destinationOk = (action, method, target) => {
        let u;
        try {
          u = new URL(String(action), String(window.location.href));
        } catch {
          return false;
        }
        return (
          u.origin === expectedOrigin &&
          String(method).toLowerCase() === 'post' &&
          (!target || String(target) === '_self')
        );
      };
      if (!destinationOk(ofForm('action'), ofForm('method'), ofForm('target'))) {
        return { ok: false, reason: 'form' };
      }
      const B = HTMLButtonElement.prototype;
      const isButton = (c) =>
        (c instanceof HTMLButtonElement && desc(B, 'type').get.call(c) === 'submit') ||
        (c instanceof HTMLInputElement && ['submit', 'image'].includes(typeOf(c)));
      const buttonVisible = (c) =>
        !desc(c instanceof HTMLButtonElement ? B : HTMLInputElement.prototype, 'disabled').get.call(c) &&
        Element.prototype.checkVisibility.call(c, { checkOpacity: true, checkVisibilityCSS: true });
      const elements = Array.prototype.slice.call(ofForm('elements'));
      const button = elements.find((c) => isButton(c) && buttonVisible(c)) || null;
      if (button) {
        const P = button instanceof HTMLButtonElement ? B : HTMLInputElement.prototype;
        const g = (k) => desc(P, k).get.call(button);
        const has = (a) => Element.prototype.hasAttribute.call(button, a);
        if (
          !destinationOk(
            has('formaction') ? g('formAction') : ofForm('action'),
            has('formmethod') ? g('formMethod') : ofForm('method'),
            has('formtarget') ? g('formTarget') : ofForm('target'),
          )
        ) {
          return { ok: false, reason: 'form' };
        }
      }
      doSubmit = () => F.requestSubmit.call(form, button);
    }

    if (usernameField && username) write(usernameField, username);
    write(passwordField, password);
    if (totpField) write(totpField, totp);
    if (doSubmit) {
      // The submitted data is built inside `requestSubmit`; then the fields go back to
      // empty (the window in which the page can read them stays as small as possible).
      try {
        doSubmit();
      } finally {
        if (usernameField && username) write(usernameField, '');
        write(passwordField, '');
        if (totpField) write(totpField, '');
      }
      return { ok: true, submitted: true };
    }
    return { ok: true, username: Boolean(usernameField && username), totp: Boolean(totpField) };
  } catch {
    return { ok: false, reason: 'failure' };
  } finally {
    if (bytes) bytes.fill(0);
    bytes = null;
    passwordB64 = '';
    totp = '';
    username = '';
  }
}
