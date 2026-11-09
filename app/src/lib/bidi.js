// D51 (Fase 39f): texto do agente com letras de escrita da direita para a esquerda
// (hebraico, árabe, siríaco, thaana, n'ko e outras de classe bidi R/AL).
//
// O escape visível do Rust (D51) já troca os caracteres invisíveis e de direção
// (U+202E, U+2066…) por `\u{…}`, mas as letras RTL são visíveis e passam como estão.
// Num texto da esquerda para a direita, uma letra RTL puxa os vizinhos neutros
// (espaço, número, pontuação) para a ordem dela: `echo א 1 ב` aparece como
// `echo ב 1 א`, outro texto que não o comando executado. Aqui o texto é partido em
// trechos: cada sequência de letras RTL vai isolada (`<bdi dir="rtl">`, em
// `TextoDoAgente.svelte`), e o resto fica da esquerda para a direita. Um trecho isolado
// conta como um caractere neutro para o que está em volta: nada fora dele muda de
// lugar. Juntos, os trechos dão o texto original, sem tirar nem pôr nada.

// Blocos das escritas RTL (letras, marcas e dígitos delas): hebraico até os
// suplementos do árabe (U+0590–U+08FF), formas de apresentação do hebraico e do árabe
// (U+FB1D–U+FDFF, U+FE70–U+FEFC) e os blocos RTL dos planos suplementares.
const RTL = /[֐-ࣿיִ-﷿ﹰ-ﻼ\u{10800}-\u{10FFF}\u{1E800}-\u{1EFFF}]+/gu;

/**
 * Parte o texto em trechos LTR e RTL, na ordem.
 * @param {string} texto
 * @returns {{ texto: string, rtl: boolean }[]}
 */
export function partesBidi(texto) {
  const partes = [];
  let fim = 0;
  const s = String(texto);
  for (const m of s.matchAll(RTL)) {
    if (m.index > fim) partes.push({ texto: s.slice(fim, m.index), rtl: false });
    partes.push({ texto: m[0], rtl: true });
    fim = m.index + m[0].length;
  }
  if (fim < s.length || partes.length === 0) {
    partes.push({ texto: s.slice(fim), rtl: false });
  }
  return partes;
}
