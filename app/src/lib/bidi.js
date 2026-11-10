// D51 (Phase 39f): agent text with letters of right-to-left scripts (Hebrew, Arabic,
// Syriac, Thaana, N'Ko and others of bidi class R/AL).
//
// The visible escape of the Rust side (D51) already turns the invisible and direction
// characters (U+202E, U+2066…) into `\u{…}`, but RTL letters are visible and pass as
// they are. In a left-to-right text, an RTL letter pulls the neutral neighbors (space,
// digit, punctuation) into its own order: an `echo` with two Hebrew letters around a digit shows with the letters swapped, a text
// other than the command that runs. Here the text is split into parts: each run of RTL
// letters goes isolated (`<bdi dir="rtl">`, in `AgentText.svelte`), and the rest stays
// left to right. An isolated part counts as one neutral character for what is around
// it: nothing outside it moves. Together, the parts give the original text, with
// nothing added or removed.

// Blocks of the RTL scripts (their letters, marks and digits): Hebrew up to the Arabic
// supplements (U+0590–U+08FF), presentation forms of Hebrew and Arabic (U+FB1D–U+FDFF,
// U+FE70–U+FEFE) and the RTL blocks of the supplementary planes. U+FEFD and U+FEFE are
// not assigned, but ICU (the one of WebKitGTK) treats them as AL, like the whole block;
// they stay in the range (Phase 39g). U+FEFF is the BOM, which the Rust side already
// escapes. `dev/tests/bidi_icu.py` checks, against the ICU of the system, that no code
// point of a strong RTL class or of a bidi control is left out of this range and of
// the Rust escape.
const RTL = /[֐-ࣿיִ-﷿ﹰ-﻾\u{10800}-\u{10FFF}\u{1E800}-\u{1EFFF}]+/gu;

/**
 * Splits the text into LTR and RTL parts, in order.
 * @param {string} text
 * @returns {{ text: string, rtl: boolean }[]}
 */
export function bidiParts(text) {
  const parts = [];
  let end = 0;
  const s = String(text);
  for (const m of s.matchAll(RTL)) {
    if (m.index > end) parts.push({ text: s.slice(end, m.index), rtl: false });
    parts.push({ text: m[0], rtl: true });
    end = m.index + m[0].length;
  }
  if (end < s.length || parts.length === 0) {
    parts.push({ text: s.slice(end), rtl: false });
  }
  return parts;
}
