//! Agent text to display in the approval window (D51, Phase 23).
//!
//! The command and the process chain come from the agent: they may have a control
//! character (`ESC` that erases the line in a terminal), a text direction character
//! (U+202E reverses what comes after), zero width characters or Unicode tags, which make
//! the window show something other than the executed command. Here nothing is hidden or
//! removed: what is not visible becomes a **visible escape** and the rest passes as is.
//!
//! - [`display_args`]: each argument in double quotes, separated by a space; `"`
//!   becomes `\"` and `\` becomes `\\`, so the form is unambiguous (one can go back to
//!   the exact bytes). A byte that is not valid UTF-8 becomes `\xNN`.
//! - [`display_text`]: the same without the quotes (process name, a single text).
//! - [`neutralize`]: only replaces the invisible with an escape (does not touch `\` or
//!   quotes). For whoever receives an already built text and treats it as untrusted
//!   (the window, Phase 36): over the output of the two above it changes nothing.
//!
//! Escapes: `\n`, `\r`, `\t`; other C0 control, `DEL` and C1 as `\xNN`; Unicode
//! formatting and separators as `\u{XXXX}`. Text above the limit is cut at a point that
//! does not split an escape or a character, with the notice [`CUT_NOTICE`] and the
//! number of bytes left out; never silently.

use core::fmt::Write as _;

/// Start of the cut notice (the number of bytes comes after).
pub const CUT_NOTICE: &str = " … [cortado: mais ";

/// Room reserved in the limit for the cut notice.
const NOTICE_RESERVE: usize = 64;

/// The character is not visible or changes the order/shape of the text around it:
/// control (C0, C1, `DEL`), Unicode formatting (bidi, zero width, BOM, tags, joiners),
/// line/paragraph separators, variation selectors and Hangul fillers.
pub fn invisible(c: char) -> bool {
    let u = c as u32;
    c.is_control()
        || matches!(
            u,
            0x00AD                  // soft hyphen
            | 0x034F                // grapheme joiner
            | 0x0600..=0x0605       // Arabic number marks
            | 0x061C                // Arabic letter mark (bidi)
            | 0x06DD
            | 0x070F
            | 0x08E2
            | 0x115F | 0x1160      // Hangul fillers
            | 0x17B4 | 0x17B5
            | 0x180B..=0x180F       // Mongolian selectors and U+180E
            | 0x200B..=0x200F       // zero width, ZWJ/ZWNJ, LRM/RLM
            | 0x2028 | 0x2029      // line and paragraph separators
            | 0x202A..=0x202E       // bidi: embed/override
            | 0x2060..=0x206F       // word joiner, invisibles, bidi isolates
            | 0x3164                // Hangul filler
            | 0xFE00..=0xFE0F       // variation selectors
            | 0xFEFD | 0xFEFE      // unassigned; ICU gives them as AL (Phase 39g)
            | 0xFEFF                // BOM / zero width no-break
            | 0xFFA0
            | 0xFFF9..=0xFFFB       // interlinear annotation
            | 0x110BD | 0x110CD
            | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0000..=0xE007F     // tags
            | 0xE0100..=0xE01EF // supplementary variation selectors
        )
}

/// Writes `c` into `output`, with an escape if it is invisible.
fn write_char(output: &mut String, c: char) {
    match c {
        '\n' => output.push_str("\\n"),
        '\r' => output.push_str("\\r"),
        '\t' => output.push_str("\\t"),
        c if (c as u32) < 0x100 && invisible(c) => {
            let _ = write!(output, "\\x{:02x}", c as u32);
        }
        c if invisible(c) => {
            let _ = write!(output, "\\u{{{:04X}}}", c as u32);
        }
        c => output.push(c),
    }
}

/// The pieces (already escaped) of `bytes`: one per character or invalid byte.
fn chunks(bytes: &[u8], quotes: bool, mut each: impl FnMut(&str) -> bool) {
    let mut buf = String::new();
    for chunk in bytes.utf8_chunks() {
        for c in chunk.valid().chars() {
            buf.clear();
            match c {
                '\\' => buf.push_str("\\\\"),
                '"' if quotes => buf.push_str("\\\""),
                c => write_char(&mut buf, c),
            }
            if !each(&buf) {
                return;
            }
        }
        for b in chunk.invalid() {
            buf.clear();
            let _ = write!(buf, "\\x{b:02x}");
            if !each(&buf) {
                return;
            }
        }
    }
}

/// Builds the text up to `limit` bytes; if it does not fit, cuts and adds the notice.
struct Assembly {
    output: String,
    limit: usize,
    total: usize,
    cut: bool,
}

impl Assembly {
    fn new(limit: usize) -> Assembly {
        Assembly {
            output: String::new(),
            // Small limit: half is left for the notice (the whole notice only fits from
            // ~64 bytes on).
            limit: limit.saturating_sub(NOTICE_RESERVE.min(limit / 2)),
            total: 0,
            cut: false,
        }
    }

    /// Appends `p`; `false` when it went over the limit (the rest only counts bytes).
    fn push(&mut self, p: &str) -> bool {
        self.total += p.len();
        if !self.cut && self.output.len() + p.len() <= self.limit {
            self.output.push_str(p);
        } else {
            self.cut = true;
        }
        true
    }

    fn finish(mut self) -> String {
        if self.cut {
            let left_out = self.total - self.output.len();
            let _ = write!(self.output, "{CUT_NOTICE}{left_out} bytes não mostrados]");
        }
        self.output
    }
}

/// The command (`argv`) to display: each argument in quotes, separated by a space, with
/// a visible escape, in up to `limit` bytes (with the cut notice if it goes over).
pub fn display_args<A: AsRef<[u8]>>(argv: &[A], limit: usize) -> String {
    let mut m = Assembly::new(limit);
    for (i, a) in argv.iter().enumerate() {
        if i > 0 {
            m.push(" ");
        }
        m.push("\"");
        chunks(a.as_ref(), true, |p| m.push(p));
        m.push("\"");
    }
    m.finish()
}

/// A text to display (without quotes), with a visible escape and `\` as `\\`, in up to
/// `limit` bytes (with the cut notice if it goes over).
pub fn display_text(text: &[u8], limit: usize) -> String {
    let mut m = Assembly::new(limit);
    chunks(text, false, |p| m.push(p));
    m.finish()
}

/// Only replaces the invisible character with a visible escape; the rest stays the
/// same. Over the output of [`display_args`] and [`display_text`] it changes nothing.
pub fn neutralize(text: &str) -> String {
    let mut s = String::with_capacity(text.len());
    for c in text.chars() {
        write_char(&mut s, c);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const L: usize = 64 * 1024;

    /// No invisible character in the output.
    fn only_visible(s: &str) -> bool {
        s.chars().all(|c| !invisible(c))
    }

    #[test]
    fn args_in_quotes_and_separated() {
        assert_eq!(
            display_args(&[b"psql".as_slice(), b"-c", b"select 1"], L),
            r#""psql" "-c" "select 1""#
        );
        assert_eq!(display_args::<&[u8]>(&[], L), "");
        assert_eq!(display_args(&[b""], L), r#""""#);
        // Quotes and backslash in the argument do not close the argument nor vanish.
        assert_eq!(
            display_args(&[br#"a" "b\c"#.as_slice()], L),
            r#""a\" \"b\\c""#
        );
    }

    /// Attack (D51): the agent puts ESC, CR, bidi, zero width and tags in the command so
    /// that the window shows another text. Everything shows up as a visible escape,
    /// nothing vanishes.
    #[test]
    fn attack_control_and_bidi_become_visible_escape() {
        let cases: &[(&str, &str)] = &[
            ("rm -rf /\x1b[2K\rls", r#""rm -rf /\x1b[2K\rls""#),
            ("a\nb", r#""a\nb""#),
            ("a\tb", r#""a\tb""#),
            ("\u{7f}", r#""\x7f""#),
            ("\u{85}", r#""\x85""#),
            ("\u{9b}31m", r#""\x9b31m""#),
            ("ls \u{202E}txt.hs", r#""ls \u{202E}txt.hs""#),
            ("\u{2066}x\u{2069}", r#""\u{2066}x\u{2069}""#),
            ("a\u{200B}b\u{200D}c", r#""a\u{200B}b\u{200D}c""#),
            ("\u{FEFF}bom", r#""\u{FEFF}bom""#),
            // Phase 39g: unassigned code points that ICU treats as AL (Arabic letter).
            ("ls \u{FEFD}1\u{FEFE}", r#""ls \u{FEFD}1\u{FEFE}""#),
            ("x\u{E0041}\u{E007F}", r#""x\u{E0041}\u{E007F}""#),
            ("l\u{2028}p\u{2029}", r#""l\u{2028}p\u{2029}""#),
            (
                "\u{00AD}\u{061C}\u{200E}\u{200F}",
                r#""\xad\u{061C}\u{200E}\u{200F}""#,
            ),
        ];
        for (input, expected) in cases {
            let s = display_args(&[input.as_bytes()], L);
            assert_eq!(&s, expected, "{input:?}");
            assert!(only_visible(&s), "{s:?}");
        }
        // Accent and emoji (visible) pass as they are.
        assert_eq!(
            display_args(&["ação ✓ 日本".as_bytes()], L),
            "\"ação ✓ 日本\""
        );
    }

    #[test]
    fn invalid_byte_becomes_x() {
        assert_eq!(display_args(&[b"a\xffb\xc3"], L), r#""a\xffb\xc3""#);
        assert_eq!(display_text(b"\x00", L), r"\x00");
    }

    /// The form is unambiguous: different arguments give different texts, even when one
    /// tries to imitate the escape of the other.
    #[test]
    fn attack_imitating_escape_or_separator_does_not_fool() {
        type Argv<'a> = &'a [&'a [u8]];
        let pairs: &[(Argv, Argv)] = &[
            (&[b"\x1b"], &[b"\\x1b"]),
            (&[b"a b"], &[b"a", b"b"]),
            (&[br#"a" "b"#], &[b"a", b"b"]),
            (&["\u{202E}".as_bytes()], &[b"\\u{202E}"]),
            (&[b"\n"], &[b"\\n"]),
            (&[b"\xff"], &[b"\\xff"]),
        ];
        for (a, b) in pairs {
            assert_ne!(display_args(a, L), display_args(b, L));
        }
    }

    /// Long text is cut with a notice, at the limit, without splitting a character or an
    /// escape.
    #[test]
    fn cut_with_notice_never_silently() {
        let long = "ç".repeat(10_000);
        let s = display_args(&[long.as_bytes()], 1000);
        assert!(s.len() <= 1000, "{}", s.len());
        assert!(s.contains(CUT_NOTICE), "{s}");
        assert!(s.ends_with(" bytes não mostrados]"));
        // What was shown is an exact start of the whole text.
        let whole = display_args(&[long.as_bytes()], usize::MAX);
        let shown = &s[..s.find(CUT_NOTICE).unwrap()];
        assert!(whole.starts_with(shown));
        let left_out: usize = s[s.find(CUT_NOTICE).unwrap() + CUT_NOTICE.len()..]
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(shown.len() + left_out, whole.len());
        // An escape is not split.
        let esc = "\u{1b}".repeat(1000);
        let s = display_text(esc.as_bytes(), 200);
        let shown = &s[..s.find(CUT_NOTICE).unwrap()];
        assert_eq!(shown.len() % 4, 0, "{shown}");
        assert!(s.len() <= 200);
        // Without a cut, without a notice.
        assert!(!display_text(b"curto", 200).contains(CUT_NOTICE));
    }

    #[test]
    fn neutralize_is_idempotent_over_the_output() {
        for t in [
            "rm\u{1b}[2K\u{202E}x\u{200B}\n\\\"".as_bytes(),
            b"\xff\x00plain",
        ] {
            let a = display_args(&[t], L);
            assert_eq!(neutralize(&a), a);
            let b = display_text(t, L);
            assert_eq!(neutralize(&b), b);
        }
        assert_eq!(neutralize("a\u{202E}b\u{1b}"), r"a\u{202E}b\x1b");
        assert_eq!(neutralize(r#"a\b"c"#), r#"a\b"c"#);
    }

    #[test]
    fn text_without_quotes() {
        assert_eq!(display_text(br#"claude "x" \"#, L), r#"claude "x" \\"#);
        assert_eq!(display_text("ba\u{202E}sh".as_bytes(), L), r"ba\u{202E}sh");
    }
}
