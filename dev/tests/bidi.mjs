// Fase 39f (D51, V5.29): `partesBidi` (app/src/lib/bidi.js) isola cada sequência de
// letras RTL e não tira nem põe nada no texto. Roda no `make app-front` com o node do
// nvm: `node dev/tests/bidi.mjs`. Sai 1 na primeira falha.
import { partesBidi } from '../../app/src/lib/bidi.js';

let falhas = 0;
function igual(obtido, esperado, nome) {
  const a = JSON.stringify(obtido);
  const b = JSON.stringify(esperado);
  if (a !== b) {
    console.error(`FALHA ${nome}: ${a} != ${b}`);
    falhas += 1;
  }
}
const L = (texto) => ({ texto, rtl: false });
const R = (texto) => ({ texto, rtl: true });

// Só LTR (inclusive o escape visível do Rust): um trecho, igual.
igual(partesBidi('"psql" "-c" "select 1"'), [L('"psql" "-c" "select 1"')], 'ltr');
igual(partesBidi('"echo \\u{202E}txt.exe"'), [L('"echo \\u{202E}txt.exe"')], 'escape');
igual(partesBidi(''), [L('')], 'vazio');

// Ataque: letras hebraicas em volta de um número. Sem isolar, o navegador mostraria
// `echo ב 1 א`; com cada sequência isolada, o número e os espaços ficam no lugar.
igual(
  partesBidi('"sh" "-c" "echo א 1 ב"'),
  [L('"sh" "-c" "echo '), R('א'), L(' 1 '), R('ב'), L('"')],
  'hebraico em volta de número'
);
// Árabe com dígitos arábico-índicos e formas de apresentação.
igual(partesBidi('rm مرحبا١٢ -rf'), [L('rm '), R('مرحبا١٢'), L(' -rf')], 'árabe');
igual(partesBidi('aיִbﹰc'), [L('a'), R('יִ'), L('b'), R('ﹰ'), L('c')], 'apresentação');
// Fase 39g: U+FEFD e U+FEFE não são atribuídos, mas a ICU os dá como AL.
igual(partesBidi('a\u{FEFD} 1 \u{FEFE}b'), [L('a'), R('\u{FEFD}'), L(' 1 '), R('\u{FEFE}'), L('b')], 'FEFD/FEFE');
// Plano suplementar (fenício, adlam).
igual(partesBidi('x\u{10900}\u{1E900}y'), [L('x'), R('\u{10900}\u{1E900}'), L('y')], 'suplementar');
// Nome de processo só RTL.
igual(partesBidi('קלוד'), [R('קלוד')], 'só rtl');

// Nada some nem muda de ordem: juntar os trechos dá o texto original.
for (const t of [
  '"sh" "-c" "echo א 1 ב"',
  'claude\\u{202E} → bash',
  'abcאב 123 ال xyz',
  '\u{1E900}',
]) {
  const junto = partesBidi(t)
    .map((p) => p.texto)
    .join('');
  igual(junto, t, `sem perda: ${JSON.stringify(t)}`);
  // Trechos vizinhos nunca têm a mesma direção (o RTL é a sequência inteira).
  const partes = partesBidi(t);
  for (let i = 1; i < partes.length; i += 1) {
    if (partes[i].rtl === partes[i - 1].rtl) {
      console.error(`FALHA trechos vizinhos com a mesma direção: ${JSON.stringify(t)}`);
      falhas += 1;
    }
  }
}

if (falhas > 0) process.exit(1);
console.log('bidi: ok');
