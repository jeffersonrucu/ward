// Phase 39f (D51, V5.29): `bidiParts` (app/src/lib/bidi.js) isolates each run of RTL
// letters and neither removes nor adds anything to the text. Runs in `make app-front` with
// nvm's node: `node dev/tests/bidi.mjs`. Exits 1 on the first failure.
import { bidiParts } from '../../app/src/lib/bidi.js';

let failures = 0;
function equal(actual, expected, name) {
  const a = JSON.stringify(actual);
  const b = JSON.stringify(expected);
  if (a !== b) {
    console.error(`FALHA ${name}: ${a} != ${b}`);
    failures += 1;
  }
}
const L = (text) => ({ text, rtl: false });
const R = (text) => ({ text, rtl: true });

// LTR only (including Rust's visible escape): one part, unchanged.
equal(bidiParts('"psql" "-c" "select 1"'), [L('"psql" "-c" "select 1"')], 'ltr');
equal(bidiParts('"echo \\u{202E}txt.exe"'), [L('"echo \\u{202E}txt.exe"')], 'escape');
equal(bidiParts(''), [L('')], 'vazio');

// Attack: Hebrew letters around a number. Without isolation, the browser would show
// `echo \u{5D1} 1 \u{5D0}` (the two letters swapped); with each run isolated, the number and the spaces stay in place.
equal(
  bidiParts('"sh" "-c" "echo א 1 ב"'),
  [L('"sh" "-c" "echo '), R('א'), L(' 1 '), R('ב'), L('"')],
  'hebraico em volta de número'
);
// Arabic with Arabic-Indic digits and presentation forms.
equal(bidiParts('rm مرحبا١٢ -rf'), [L('rm '), R('مرحبا١٢'), L(' -rf')], 'árabe');
equal(bidiParts('aיִbﹰc'), [L('a'), R('יִ'), L('b'), R('ﹰ'), L('c')], 'apresentação');
// Phase 39g: U+FEFD and U+FEFE are unassigned, but ICU gives them as AL.
equal(bidiParts('a\u{FEFD} 1 \u{FEFE}b'), [L('a'), R('\u{FEFD}'), L(' 1 '), R('\u{FEFE}'), L('b')], 'FEFD/FEFE');
// Supplementary plane (Phoenician, Adlam).
equal(bidiParts('x\u{10900}\u{1E900}y'), [L('x'), R('\u{10900}\u{1E900}'), L('y')], 'suplementar');
// Process name that is RTL only.
equal(bidiParts('קלוד'), [R('קלוד')], 'só rtl');

// Nothing disappears or changes order: joining the parts gives the original text.
for (const t of [
  '"sh" "-c" "echo א 1 ב"',
  'claude\\u{202E} → bash',
  'abcאב 123 ال xyz',
  '\u{1E900}',
]) {
  const joined = bidiParts(t)
    .map((p) => p.text)
    .join('');
  equal(joined, t, `sem perda: ${JSON.stringify(t)}`);
  // Neighbouring parts never have the same direction (the RTL part is the whole run).
  const parts = bidiParts(t);
  for (let i = 1; i < parts.length; i += 1) {
    if (parts[i].rtl === parts[i - 1].rtl) {
      console.error(`FALHA trechos vizinhos com a mesma direção: ${JSON.stringify(t)}`);
      failures += 1;
    }
  }
}

if (failures > 0) process.exit(1);
console.log('bidi: ok');
