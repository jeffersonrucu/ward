// Local rules of the first use screen (Phase 30). They are the same as the first use
// module of `app/core`, which checks everything again before the create vault
// command: here they only make the error show in the field without a trip to the Rust
// side. Nothing here keeps the value: the functions take the text and return only a
// message or a level.

export const MIN_PASSWORD_CHARS = 12;
export const MIN_PIN_DIGITS = 4;
export const MAX_PIN_DIGITS = 6;

export const MESSAGES = Object.freeze({
  passwordShort: 'A senha mestra precisa de 12 caracteres ou mais.',
  passwordMismatch: 'As duas senhas não são iguais.',
  pinInvalid: 'O PIN tem de ter de 4 a 6 dígitos, só números.',
  pinWithLetter: 'Só números: letras e símbolos não entram no PIN.',
  pinLong: 'O PIN tem no máximo 6 dígitos.',
  pinMismatch: 'Os dois PINs não são iguais.',
});

/** Characters (code points), like the Rust `chars().count()`. */
export function charCount(text) {
  return [...text].length;
}

export function passwordError(password) {
  return charCount(password) < MIN_PASSWORD_CHARS ? MESSAGES.passwordShort : '';
}

export function isValidPin(pin) {
  return new RegExp(`^[0-9]{${MIN_PIN_DIGITS},${MAX_PIN_DIGITS}}$`).test(pin);
}

// Estimated strength of the typed password: length × log2 of the alphabet used, with the
// thresholds of the generator (D65 f: weak < 60 bits, fair < 80, strong < 128, very
// strong ≥ 128). It is only a hint; the rule that blocks is the minimum of 12
// characters.
const STRENGTHS = [
  { upTo: 60, code: 'weak', label: 'fraca', segments: 1, tone: 'danger' },
  { upTo: 80, code: 'fair', label: 'razoável', segments: 2, tone: 'warning' },
  { upTo: 128, code: 'strong', label: 'forte', segments: 3, tone: 'success' },
  { upTo: Infinity, code: 'very-strong', label: 'muito forte', segments: 4, tone: 'success' },
];

export function passwordStrength(password) {
  if (!password) return null;
  let alphabet = 0;
  if (/[a-z]/.test(password)) alphabet += 26;
  if (/[A-Z]/.test(password)) alphabet += 26;
  if (/[0-9]/.test(password)) alphabet += 10;
  if (/[\x21-\x2f\x3a-\x40\x5b-\x60\x7b-\x7e ]/.test(password)) alphabet += 33;
  if (/[^\x00-\x7f]/.test(password)) alphabet += 64;
  const bits = charCount(password) * Math.log2(Math.max(alphabet, 2));
  return STRENGTHS.find((f) => bits < f.upTo);
}

/** Error message from the Rust side (the reason, a fixed text) with the first letter in
 * uppercase. */
export function rustMessage(reason) {
  const t = String(reason ?? '');
  return t ? `${t[0].toUpperCase()}${t.slice(1)}.` : '';
}
