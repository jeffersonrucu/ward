//! Mask tests (D11, S4): V2.17–V2.24 and the attacks (split chunks, encodings that
//! the D11 list does not mention, pathological output, secret in `Debug`).
//! Only fake values of the test vault (D15).

use super::*;
use crate::delivery::{ResourceError, Resources, build};
use crate::entry::{AttachmentDraft, DataDraft, Draft, ExtraDraft, Secret};

// ---- Helpers ----

const PG_PASSWORD: &str = "falso-PG:s3nha/01@#";
const PG_URL: &str = "postgresql://app:falso-PG%3As3nha%2F01%40%23@127.0.0.1:55432/app_teste";
const M: &str = MARK;

fn t(v: &str) -> String {
    v.to_owned()
}

fn s(v: &str) -> Option<Secret> {
    Some(Secret::from_text(t(v)))
}

fn draft(name: &str, data: DataDraft) -> Draft {
    Draft {
        name: t(name),
        group: t("acme"),
        environment: t("dev"),
        project: String::new(),
        tags: Vec::new(),
        data,
        extras: Vec::new(),
        attachments: Vec::new(),
    }
}

fn extra(name: &str, value: &str, secret: bool) -> ExtraDraft {
    ExtraDraft {
        name: t(name),
        value: Secret::from_text(t(value)),
        secret,
    }
}

fn attachment(name: &str, content: &[u8]) -> AttachmentDraft {
    AttachmentDraft {
        name: t(name),
        content: Secret::from_bytes(content.to_vec()),
    }
}

fn valid(r: Draft) -> Entry {
    r.validate()
        .unwrap_or_else(|e| panic!("devia validar: {e}"))
}

fn pg_dev() -> Entry {
    valid(draft(
        "acme-pg-dev",
        DataDraft::Database {
            engine: t("postgres"),
            host: t("127.0.0.1"),
            port: t("55432"),
            db_name: t("app_teste"),
            username: t("app"),
            password: s(PG_PASSWORD),
            tunnel: String::new(),
        },
    ))
}

fn gama_generic() -> Entry {
    let mut r = draft("gama-generico", DataDraft::Generic);
    r.group = t("gama");
    r.environment = t("stage");
    r.extras = vec![
        extra("REGIAO", "r-01", false),
        extra("CHAVE_LICENCA", "falso-licenca-01", true),
    ];
    r.attachments = vec![attachment("app.env", b"FALSO_ENV=falso-anexo-01\n")];
    valid(r)
}

/// Generic entry with secret extras only (for the tests with several secrets).
fn generic(secrets: &[&str]) -> Entry {
    let mut r = draft("teste-generico", DataDraft::Generic);
    r.extras = secrets
        .iter()
        .enumerate()
        .map(|(i, v)| extra(&format!("S{i}"), v, true))
        .collect();
    valid(r)
}

const FAKE_CODE: &str = "123456";

struct Fakes;

impl Resources for Fakes {
    fn totp_code(&mut self, _: &Secret) -> Result<Zeroizing<String>, ResourceError> {
        Ok(Zeroizing::new(t(FAKE_CODE)))
    }
    fn file(&mut self, name: &str, _: &Secret) -> Result<String, ResourceError> {
        Ok(format!("/run/user/1000/ward/tmp/1-{name}"))
    }
    fn tunnel(&mut self, _: &str, _: &str, _: u16) -> Result<u16, ResourceError> {
        Ok(40123)
    }
}

fn delivery_of(e: &Entry) -> Delivery {
    build(e, &mut Fakes).unwrap_or_else(|error| panic!("{error}"))
}

/// Mask of the entry with its delivery (as `ward run` builds it).
fn mask_of(e: &Entry) -> Mask {
    let g = delivery_of(e);
    Mask::from_entry(e, Some(&g)).expect("máscara")
}

/// Passes the output in chunks (cuts at increasing positions) and joins the result.
/// Checks, at each chunk, that nothing was held (D34 revised): the stream returned
/// everything it read.
fn in_chunks(m: &Mask, output: &[u8], cuts: &[usize]) -> Vec<u8> {
    let mut f = m.stream();
    let mut out = Vec::new();
    let mut start = 0;
    for &c in cuts.iter().chain(core::iter::once(&output.len())) {
        f.mask(&output[start..c], &mut out);
        nothing_held(&f);
        start = c;
    }
    f.finish(&mut out);
    out
}

/// The stream keeps nothing for later (D34 revised: it never holds nor delays).
fn nothing_held(f: &Stream<'_>) {
    assert_eq!(f.emitted, f.consumed, "o fluxo segurou saída");
    assert!(f.pending.is_empty(), "o fluxo segurou saída");
}

/// The output in a single chunk (the case S4 guarantees: the whole secret in the same
/// block). Also passes the same output byte by byte and checks that no byte was held and
/// that each byte went out right away, unchanged, up to the last byte of a pattern (half
/// a match changes nothing: no prefix oracle).
fn one_block(m: &Mask, output: &[u8]) -> Vec<u8> {
    let mut f = m.stream();
    let mut out = Vec::new();
    for b in output {
        let before = out.len();
        let mark_at_end = out.ends_with(M.as_bytes());
        f.mask(core::slice::from_ref(b), &mut out);
        nothing_held(&f);
        let new = &out[before..];
        // Each byte goes out right away: itself, the mark (a pattern ended here) or
        // nothing (the pattern continues glued to the mark that just went out).
        assert!(
            new == core::slice::from_ref(b)
                || new == M.as_bytes()
                || (new.is_empty() && mark_at_end),
            "byte segurado ou trocado"
        );
    }
    m.mask_all(output)
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// Test base64, independent of the mask's one (bit accumulator).
fn b64(data: &[u8], url: bool, padding: bool) -> String {
    let alphabet: Vec<char> = if url {
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
    } else {
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
    }
    .chars()
    .collect();
    let mut out = String::new();
    let (mut acc, mut bits) = (0u32, 0u32);
    for &b in data {
        acc = (acc << 8) | u32::from(b);
        bits += 8;
        while bits >= 6 {
            bits -= 6;
            out.push(alphabet[((acc >> bits) & 63) as usize]);
        }
    }
    if bits > 0 {
        out.push(alphabet[((acc << (6 - bits)) & 63) as usize]);
    }
    while padding && !out.len().is_multiple_of(4) {
        out.push('=');
    }
    out
}

/// Test percent-encoding: encodes everything outside `A-Za-z0-9-._~`; `lower` uses
/// lower-case hex; `plus` replaces space with `+`.
fn pct(v: &str, lower: bool, plus: bool) -> String {
    let mut out = String::new();
    for &b in v.as_bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else if b == b' ' && plus {
            out.push('+');
        } else if lower {
            out.push_str(&format!("%{b:02x}"));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Checks that no piece of the secret with 4 bytes or more is left in the output.
fn no_piece(output: &[u8], secret: &[u8]) {
    for n in 4..=secret.len() {
        for window in secret.windows(n) {
            assert!(
                !output.windows(n).any(|w| w == window),
                "sobrou pedaço do segredo na saída: {}",
                text(output)
            );
        }
    }
}

/// Test pseudo-random generator (xorshift64*). Not crypto: it only draws cuts.
struct Draw(u64);

impl Draw {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

// ---- V2.17 ----

#[test]
fn v2_17_literal() {
    let m = mask_of(&pg_dev());
    let output = m.mask_all(format!("x {PG_PASSWORD} y").as_bytes());
    assert_eq!(text(&output), format!("x {M} y"));
}

// ---- V2.18 ----

#[test]
fn v2_18_every_cut_point() {
    // D34 revised: nothing is held. The whole secret in one chunk goes out `••••`; split
    // between chunks, what already went out stays as it went and the chunk that
    // completes the secret goes out marked from the cut on (the match state carries
    // from one chunk to the next).
    let m = mask_of(&pg_dev());
    let output = format!("x {PG_PASSWORD} y");
    let b = output.as_bytes();
    let expected = format!("x {M} y");
    let start = 2;
    let end = start + PG_PASSWORD.len();
    // What goes out with the cuts: up to the last cut before the end of the secret in
    // plaintext (if it falls inside the secret), then the mark.
    let with_cuts = |cuts: &[usize]| {
        let c = cuts.iter().copied().filter(|&c| c > start && c < end).max();
        match c {
            None => expected.clone(),
            Some(c) => format!("x {}{M} y", &PG_PASSWORD[..c - start]),
        }
    };
    for k in 0..=b.len() {
        assert_eq!(text(&in_chunks(&m, b, &[k])), with_cuts(&[k]), "corte {k}");
        for j in k..=b.len() {
            assert_eq!(
                text(&in_chunks(&m, b, &[k, j])),
                with_cuts(&[k, j]),
                "cortes {k},{j}"
            );
        }
    }
    assert_eq!(text(&one_block(&m, b)), expected);
}

#[test]
fn d34_attack_prefix_at_chunk_end_goes_out_right_away_and_unchanged() {
    // M4 audit (prefix oracle): the start of the secret at the end of a chunk goes out
    // right away, like any other text; nothing is held nor becomes a mark. Right prefix
    // and wrong prefix of the same size give the same answer (the text that came in),
    // so the output says nothing about the secret.
    let m = mask_of(&pg_dev());
    let output = format!("x {PG_PASSWORD} y");
    let b = output.as_bytes();
    for k in 3..2 + PG_PASSWORD.len() {
        let mut f = m.stream();
        let mut out = Vec::new();
        f.mask(&b[..k], &mut out);
        nothing_held(&f);
        assert_eq!(
            &out[..],
            &b[..k],
            "corte {k}: o prefixo foi segurado ou trocado"
        );
        let mut wrong = b[..k].to_vec();
        *wrong.last_mut().unwrap() ^= 0x01;
        let mut g = m.stream();
        let mut out_wrong = Vec::new();
        g.mask(&wrong, &mut out_wrong);
        nothing_held(&g);
        assert_eq!(out_wrong, wrong);
    }
}

// ---- V2.19 ----

#[test]
fn v2_19_percent_encoding() {
    let m = mask_of(&pg_dev());
    let forms = [
        pct(PG_PASSWORD, false, false), // upper case (`%3A`, `%2F`, `%40`, `%23`)
        pct(PG_PASSWORD, true, false),  // lower case
        pct(PG_PASSWORD, false, true),  // form
        t("falso-PG%3As3nha/01%40%23"), // urllib.parse.quote (keeps the slash)
        t("falso-PG%3as3nha%2F01@%23"), // mixed
        t("%66%61%6C%73%6F-PG:s3nha/01@#"), // even the letters encoded
    ];
    assert_eq!(forms[0], "falso-PG%3As3nha%2F01%40%23");
    for form in &forms {
        let output = format!("x {form} y");
        let expected = format!("x {M} y");
        assert_eq!(text(&m.mask_all(output.as_bytes())), expected, "{form}");
        assert_eq!(text(&one_block(&m, output.as_bytes())), expected, "{form}");
    }
}

#[test]
fn v2_19_form_with_space() {
    let m = mask_of(&generic(&["falso senha 01/x"]));
    for form in [
        pct("falso senha 01/x", false, true),
        pct("falso senha 01/x", true, true),
        pct("falso senha 01/x", false, false),
        t("falso+senha+01%2fx"),
    ] {
        let output = format!("a={form}&b=1");
        assert_eq!(
            text(&one_block(&m, output.as_bytes())),
            format!("a={M}&b=1")
        );
    }
}

// ---- V2.20 ----

#[test]
fn v2_20_base64() {
    let m = mask_of(&pg_dev());
    let basic = b64(format!("app:{PG_PASSWORD}").as_bytes(), false, true);
    assert_eq!(basic, "YXBwOmZhbHNvLVBHOnMzbmhhLzAxQCM=");
    let forms = [
        b64(PG_PASSWORD.as_bytes(), false, true),
        b64(PG_PASSWORD.as_bytes(), false, false),
        b64(PG_PASSWORD.as_bytes(), true, true),
        b64(PG_PASSWORD.as_bytes(), true, false),
        basic.clone(),
        basic.trim_end_matches('=').to_owned(),
    ];
    for form in &forms {
        let output = format!("Authorization: Basic {form}\n");
        let expected = format!("Authorization: Basic {M}\n");
        assert_eq!(text(&m.mask_all(output.as_bytes())), expected, "{form}");
        assert_eq!(text(&one_block(&m, output.as_bytes())), expected, "{form}");
    }
}

#[test]
fn v2_20_base64_url_safe_differs_from_standard() {
    // Secret whose base64 has `+` and `/`: the URL-safe one replaces them with `-` and
    // `_`.
    let secret = "falso-PG:s3nha?01>>~";
    let m = mask_of(&generic(&[secret]));
    let standard = b64(secret.as_bytes(), false, true);
    let url = b64(secret.as_bytes(), true, true);
    assert_ne!(standard, url);
    for form in [
        &standard,
        &url,
        &t(standard.trim_end_matches('=')),
        &t(url.trim_end_matches('=')),
    ] {
        assert_eq!(text(&one_block(&m, form.as_bytes())), M, "{form}");
    }
}

#[test]
fn attack_secret_inside_larger_base64() {
    // `username:token` with any username, in any alignment: the core of the base64 of
    // the token is masked and no piece that decodes the token is left.
    let token = "falso-token-gh-01";
    let m = mask_of(&generic(&[token]));
    for prefix in ["x-access-token:", "x-access-tok:", "ab:", "a:", ""] {
        let encoded = b64(format!("{prefix}{token}").as_bytes(), false, true);
        let output = m.mask_all(encoded.as_bytes());
        assert!(text(&output).contains(M), "{prefix}: {}", text(&output));
        // What is left of the base64 has no more than 2 characters glued to the secret.
        let leftover: String = text(&output).replace(M, "");
        let limit = b64(prefix.as_bytes(), false, false).len() + 2 + 4;
        assert!(leftover.len() <= limit, "{prefix}: sobrou {leftover}");
    }
}

// ---- V2.21 ----

#[test]
fn v2_21_whole_database_url() {
    let e = pg_dev();
    let g = delivery_of(&e);
    assert_eq!(g.value("DATABASE_URL"), Some(PG_URL));
    let m = Mask::from_entry(&e, Some(&g)).unwrap();
    for output in [
        format!("DATABASE_URL={PG_URL}\n"),
        // The client may print the decoded URL.
        format!("DATABASE_URL=postgresql://app:{PG_PASSWORD}@127.0.0.1:55432/app_teste\n"),
    ] {
        for out in [
            m.mask_all(output.as_bytes()),
            one_block(&m, output.as_bytes()),
        ] {
            no_piece(&out, PG_PASSWORD.as_bytes());
            no_piece(&out, pct(PG_PASSWORD, false, false).as_bytes());
            assert!(!text(&out).contains("s3nha"), "{}", text(&out));
        }
    }
    let out = m.mask_all(format!("DATABASE_URL={PG_URL}\n").as_bytes());
    assert_eq!(text(&out), format!("DATABASE_URL={M}\n"));
}

#[test]
fn delivery_marks_what_is_secret() {
    // Only what the delivery marked as secret goes in: host, port, database and
    // username do not become `••••`.
    let e = pg_dev();
    let m = mask_of(&e);
    let output = "PGHOST=127.0.0.1 PGPORT=55432 PGDATABASE=app_teste PGUSER=app\n";
    assert_eq!(text(&m.mask_all(output.as_bytes())), output);
}

#[test]
fn totp_code_is_not_masked() {
    let e = valid(draft(
        "acme-wp-dev",
        DataDraft::Login {
            url: t("http://localhost:8081/wp-login.php"),
            username: t("wp_admin"),
            password: s("falso-WP-senha-01"),
            totp_seed: s("JBSWY3DPEHPK3PXP"),
        },
    ));
    let g = delivery_of(&e);
    assert_eq!(g.value("WARD_TOTP"), Some(FAKE_CODE));
    let m = Mask::from_entry(&e, Some(&g)).unwrap();
    let output = format!("codigo {FAKE_CODE} semente JBSWY3DPEHPK3PXP senha falso-WP-senha-01");
    assert_eq!(
        text(&m.mask_all(output.as_bytes())),
        format!("codigo {FAKE_CODE} semente {M} senha {M}")
    );
}

// ---- V2.22 ----

#[test]
fn v2_22_secret_that_contains_another() {
    let larger = "falso-abc-123-xyz";
    let smaller = "abc-123";
    let m = mask_of(&generic(&[larger, smaller]));
    for glued in [
        format!("{larger}{smaller}"),
        format!("{smaller}{larger}"),
        format!("{smaller}{smaller}"),
        format!("{larger}{larger}"),
        format!("{smaller}{larger}{smaller}"),
    ] {
        let output = format!("<{glued}>");
        for out in [
            m.mask_all(output.as_bytes()),
            one_block(&m, output.as_bytes()),
        ] {
            assert_eq!(text(&out), format!("<{M}>"), "{glued}");
        }
    }
    // The smaller one inside the start of the larger one, which then diverges: the
    // automaton is in the middle of the larger one when the smaller one ends and needs
    // to find it by the suffix.
    let output = format!("<falso-{smaller}-!>");
    for out in [
        m.mask_all(output.as_bytes()),
        one_block(&m, output.as_bytes()),
    ] {
        assert_eq!(text(&out), format!("<falso-{M}-!>"));
    }
}

#[test]
fn attack_overlapping_secrets() {
    // The end of one is the start of the other: matching only the first would leave
    // tail of the second.
    let a = "falso-sobre-01";
    let b = "sobre-01-fim";
    let m = mask_of(&generic(&[a, b]));
    for glued in ["falso-sobre-01-fim", "falso-sobre-01-fimfalso-sobre-01"] {
        let output = format!("[{glued}]");
        for out in [
            m.mask_all(output.as_bytes()),
            one_block(&m, output.as_bytes()),
        ] {
            assert_eq!(text(&out), format!("[{M}]"), "{glued}");
            no_piece(&out, a.as_bytes());
            no_piece(&out, b.as_bytes());
        }
    }
}

// ---- V2.23 ----

#[test]
fn v2_23_whole_attachment_line_and_value() {
    let e = gama_generic();
    let m = mask_of(&e);
    let cases = [
        ("FALSO_ENV=falso-anexo-01\n", M.to_owned()),
        ("FALSO_ENV=falso-anexo-01", M.to_owned()),
        // The whole content includes the final newline.
        (
            "antes\nFALSO_ENV=falso-anexo-01\ndepois",
            format!("antes\n{M}depois"),
        ),
        (
            "antes FALSO_ENV=falso-anexo-01 depois",
            format!("antes {M} depois"),
        ),
        ("valor: falso-anexo-01.", format!("valor: {M}.")),
        ("licença falso-licenca-01", format!("licença {M}")),
        ("REGIAO=r-01", t("REGIAO=r-01")),
    ];
    for (output, expected) in cases {
        assert_eq!(text(&m.mask_all(output.as_bytes())), expected, "{output}");
        assert_eq!(
            text(&one_block(&m, output.as_bytes())),
            expected,
            "{output}"
        );
    }
    let b = b64(b"FALSO_ENV=falso-anexo-01\n", false, true);
    assert_eq!(text(&m.mask_all(b.as_bytes())), M);
}

#[test]
fn attachment_with_several_lines() {
    let content = "# comentário longo\n  export TOKEN=\"falso-tok-anexo\"\nA=1\nSEGREDO_X=falso-x-anexo-02\r\n";
    let mut r = draft("teste-anexo", DataDraft::Generic);
    r.extras = vec![extra("X", "y", false)];
    r.attachments = vec![attachment("app.env", content.as_bytes())];
    let m = mask_of(&valid(r));
    let cases = [
        (content.to_owned(), M.to_owned()),
        // Value with and without quotes, trimmed line, line with CRLF.
        (t("tok=falso-tok-anexo;"), format!("tok={M};")),
        (t("\"falso-tok-anexo\""), M.to_owned()),
        (t("export TOKEN=\"falso-tok-anexo\""), M.to_owned()),
        (t("SEGREDO_X=falso-x-anexo-02\n"), format!("{M}\n")),
        (t("v falso-x-anexo-02 v"), format!("v {M} v")),
        // A short line (< 6) does not go in alone.
        (t("A=1"), t("A=1")),
    ];
    for (output, expected) in cases {
        assert_eq!(
            text(&one_block(&m, output.as_bytes())),
            expected,
            "{output}"
        );
    }
}

// ---- V2.24 ----

#[test]
fn v2_24_output_without_secret_goes_out_unchanged() {
    let m = mask_of(&pg_dev());
    let mut draw = Draw(0x5eed_0001);
    let mut binary = vec![0u8; 1024 * 1024];
    for b in &mut binary {
        *b = draw.next() as u8;
    }
    // Escapes and prefixes of the secret scattered, without forming the secret.
    binary[100..106].copy_from_slice(b"%3A%2F");
    binary[2000..2008].copy_from_slice(b"falso-PG");
    let outputs: Vec<Vec<u8>> = vec![
        b"texto comum, nada de segredo aqui\n".to_vec(),
        vec![0xff, 0xfe, b'f', 0xc3, 0x28, b'%', b'4', 0x80, b'+'],
        binary,
        // The end that used to be held (start of the secret, half escape) goes out
        // unchanged.
        b"termina com falso-PG:s3".to_vec(),
        b"termina com %4".to_vec(),
        b"termina com %".to_vec(),
    ];
    for output in &outputs {
        assert_eq!(&m.mask_all(output), output);
        let cuts: Vec<usize> = (1..output.len()).step_by(4093).collect();
        assert_eq!(&in_chunks(&m, output, &cuts), output);
        if output.len() < 100 {
            assert_eq!(&one_block(&m, output), output);
        }
    }
}

#[test]
fn mask_without_secret_touches_nothing() {
    let mut r = draft("so-extras", DataDraft::Generic);
    r.extras = vec![extra("REGIAO", "r-01", false)];
    let e = valid(r);
    let m = mask_of(&e);
    assert_eq!(m.mask_all(b"r-01 \xff"), b"r-01 \xff");
}

// ---- Property: random cuts ----

#[test]
fn property_random_cuts() {
    let e = pg_dev();
    let g = delivery_of(&e);
    let m = Mask::from_entry(&e, Some(&g)).unwrap();
    let variations: Vec<String> = vec![
        t(PG_PASSWORD),
        pct(PG_PASSWORD, false, false),
        pct(PG_PASSWORD, true, false),
        b64(PG_PASSWORD.as_bytes(), false, true),
        b64(PG_PASSWORD.as_bytes(), true, false),
        b64(format!("app:{PG_PASSWORD}").as_bytes(), false, true),
        t(PG_URL),
    ];
    let noise: &[u8] = b"abc %2 % falso-PG: s3nha 01@# \n\xff\x00=+/";
    let mut draw = Draw(0x0dd_ba11);
    for round in 0..300 {
        let mut output = Vec::new();
        let mut excerpts = Vec::new();
        let mut inserted = 0;
        for _ in 0..1 + draw.below(8) {
            for _ in 0..draw.below(12) {
                output.push(noise[draw.below(noise.len())]);
            }
            // Separator: the noise does not end the variation some other way.
            output.push(b' ');
            let start = output.len();
            output.extend_from_slice(variations[draw.below(variations.len())].as_bytes());
            excerpts.push((start, output.len()));
            output.push(b' ');
            inserted += 1;
        }
        let whole = m.mask_all(&output);
        // D34 revised: S4 holds for the whole secret inside one chunk; the cuts fall
        // outside the variations (the noise between them can be cut anywhere, including
        // in the middle of a `%X` or of the start of a secret).
        let mut cuts: Vec<usize> = (0..draw.below(10))
            .map(|_| draw.below(output.len()))
            .filter(|&c| !excerpts.iter().any(|&(a, b)| a < c && c < b))
            .collect();
        cuts.sort_unstable();
        let chunks = in_chunks(&m, &output, &cuts);
        assert_eq!(chunks, whole, "rodada {round}: cortes mudaram o resultado");
        no_piece(&chunks, PG_PASSWORD.as_bytes());
        for v in &variations {
            assert!(
                !text(&chunks).contains(v.as_str()),
                "rodada {round}: sobrou {v}"
            );
        }
        let marks = text(&chunks).matches(M).count();
        assert!(marks >= 1 && marks <= inserted, "rodada {round}");
    }
}

// ---- Attacks ----

#[test]
fn attack_escaped_json() {
    let m = mask_of(&pg_dev());
    // PHP (`json_encode`) escapes the slash; common JSON does not.
    for output in [
        r#"{"senha":"falso-PG:s3nha\/01@#"}"#,
        r#"{"senha":"falso-PG:s3nha/01@#"}"#,
    ] {
        assert_eq!(
            text(&m.mask_all(output.as_bytes())),
            format!(r#"{{"senha":"{M}"}}"#)
        );
    }
    let m = mask_of(&generic(&["falso\"aspas\\01"]));
    assert_eq!(
        text(&m.mask_all(br#"{"v":"falso\"aspas\\01"}"#)),
        format!(r#"{{"v":"{M}"}}"#)
    );
}

#[test]
fn attack_pathological_output_stays_linear() {
    // Repetitive 64 KiB secret against 1 MiB of the same byte (raw and in %61): a naive
    // search would make 64 billion comparisons; the automaton is linear.
    let secret = "a".repeat(64 * 1024);
    let m = mask_of(&generic(&[&secret]));
    let output = vec![b'a'; 1024 * 1024];
    let mut f = m.stream();
    let mut out = Vec::new();
    for chunk in output.chunks(4096) {
        f.mask(chunk, &mut out);
        nothing_held(&f);
    }
    f.finish(&mut out);
    // D34 revised: the 15 chunks before the first match went out right away; from then
    // on each chunk completes the secret again and becomes a single mark.
    assert_eq!(text(&out), format!("{}{M}", "a".repeat(15 * 4096)));
    assert_eq!(text(&m.mask_all(&output)), M);
    let encoded = "%61".repeat(128 * 1024);
    assert_eq!(text(&m.mask_all(encoded.as_bytes())), M);
    // Output a bit smaller than the secret: it is not the secret, it goes out unchanged.
    let almost = vec![b'a'; 64 * 1024 - 1];
    assert_eq!(m.mask_all(&almost), almost);
}

#[test]
fn stdout_and_stderr_in_separate_streams() {
    let m = mask_of(&pg_dev());
    let (mut out, mut err) = (m.stream(), m.stream());
    let (mut so, mut se) = (Vec::new(), Vec::new());
    out.mask(b"o falso-PG:", &mut so);
    err.mask(b"e falso-PG:s3nha/01@# e", &mut se);
    out.mask(b"s3nha/01@# o", &mut so);
    out.finish(&mut so);
    err.finish(&mut se);
    // The stderr does not complete the match of the stdout; the start that went out on
    // the stdout stays as it went (D34 revised) and the chunk that completes the secret
    // becomes a mark.
    assert_eq!(text(&so), format!("o falso-PG:{M} o"));
    assert_eq!(text(&se), format!("e {M} e"));
}

#[test]
fn jump_and_tunnel_join_both_entries() {
    let a = pg_dev();
    let b = generic(&["falso-ssh-senha-01"]);
    let m = MaskBuilder::new().entry(&a).entry(&b).build().unwrap();
    assert_eq!(
        text(&m.mask_all(b"falso-ssh-senha-01 falso-PG:s3nha/01@#")),
        format!("{M} {M}")
    );
}

#[test]
fn short_secret_is_masked_and_warned() {
    let m = mask_of(&generic(&["ab12"]));
    assert!(m.has_short_secret());
    assert_eq!(text(&m.mask_all(b"x ab12 y")), format!("x {M} y"));
    assert!(!mask_of(&pg_dev()).has_short_secret());
    // An attachment does not count as a short secret (the short lines are what stays
    // out).
    assert!(!mask_of(&gama_generic()).has_short_secret());
}

#[test]
fn attack_attachment_too_large_fails_closed() {
    // Each entry fits in the budget (D29), but together they go over the mask limit:
    // above it the mask is not built; `ward run` does not run without a mask.
    let large: Vec<Entry> = (0..8)
        .map(|i| {
            let mut r = draft(&format!("anexo-grande-{i}"), DataDraft::Generic);
            r.extras = vec![extra("X", "y", false)];
            r.attachments = vec![attachment(
                "grande.bin",
                &vec![b'z'; crate::limits::MAX_ATTACHMENTS_TOTAL],
            )];
            valid(r)
        })
        .collect();
    let mut sum = 0;
    let mut used = 0;
    for e in &large {
        let cost = entry_cost(e).expect("cabe no orçamento");
        assert!(cost <= MASK_BUDGET);
        sum += cost;
        used += 1;
        if sum > PATTERN_LIMIT {
            break;
        }
    }
    assert!(sum > PATTERN_LIMIT, "o teste precisa passar do limite");
    let mut c = MaskBuilder::new();
    for e in &large[..used] {
        c = c.entry(e);
    }
    let error = c.build().unwrap_err();
    assert_eq!(error, MaskError::TooLarge);
    assert!(!error.to_string().contains("zzzz"));
}

#[test]
fn attack_debug_without_secret() {
    let e = pg_dev();
    let g = delivery_of(&e);
    let m = Mask::from_entry(&e, Some(&g)).unwrap();
    let mut f = m.stream();
    let mut out = Vec::new();
    f.mask(b"falso-PG:s3n", &mut out);
    let c = MaskBuilder::new().entry(&e).delivery(&g);
    for d in [
        format!("{m:?}"),
        format!("{f:?}"),
        format!("{c:?}"),
        format!("{m:#?}"),
    ] {
        assert!(!d.contains("falso"), "{d}");
        assert!(!d.contains("s3n"), "{d}");
    }
}

// ---- S8 / V2.28: zeroed on drop, no Clone ----

/// Fails the build if `T` implements `Clone` (the two impls become ambiguous).
trait NotClone<A> {
    fn check() {}
}
impl<T> NotClone<()> for T {}
#[allow(dead_code)]
struct ImplementsClone;
impl<T: Clone> NotClone<ImplementsClone> for T {}

fn zeroes_on_drop<T: ZeroizeOnDrop>() {}

#[test]
fn mask_and_stream_zero_and_do_not_clone() {
    <Mask as NotClone<_>>::check();
    <Stream<'static> as NotClone<_>>::check();
    <MaskBuilder<'static> as NotClone<_>>::check();
    zeroes_on_drop::<Mask>();
    zeroes_on_drop::<Stream<'static>>();
}

#[test]
fn automaton_allocated_once() {
    // The capacity computed before covers all the nodes: no reallocation leaves a copy
    // of the patterns in freed memory.
    let e = gama_generic();
    let g = delivery_of(&e);
    let c = MaskBuilder::new().entry(&e).delivery(&g);
    let mut total = 0;
    visit(&c, &mut |p| {
        total += p.len();
        Ok(())
    })
    .unwrap();
    let m = c.build().unwrap();
    assert!(m.automaton.nodes() <= total + 1);
}

// ---- Encodings (not crypto, but it has to match RFC 4648) ----

#[test]
fn base64_rfc4648() {
    let cases = [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ];
    for (input, expected) in cases {
        let mut out = Zeroizing::new(Vec::new());
        base64(input.as_bytes(), B64_STD, true, &mut out);
        assert_eq!(text(&out), expected);
        assert_eq!(b64(input.as_bytes(), false, true), expected);
    }
    let mut out = Zeroizing::new(Vec::new());
    base64(&[0xfb, 0xff], B64_URL, false, &mut out);
    assert_eq!(text(&out), "-_8");
}

// ---- V2.38: short `KEY=value` (D34) ----

#[test]
fn v2_38_short_key_value_does_not_become_pattern() {
    let mut r = draft("gama-generico", DataDraft::Generic);
    r.extras = vec![extra("REGIAO", "r-01", false)];
    r.attachments = vec![attachment(
        "app.env",
        b"PORT=8080\nDEBUG=1\nFALSO_ENV=falso-anexo-01\n",
    )];
    let m = mask_of(&valid(r));
    let cases = [
        // Short value alone in the output: goes out intact (RA3, accepted).
        (t("porta 8080 debug 1"), t("porta 8080 debug 1")),
        (t("8080"), t("8080")),
        (t("1"), t("1")),
        // The long value alone and the whole line become `••••`.
        (t("falso-anexo-01"), M.to_owned()),
        (t("PORT=8080"), M.to_owned()),
        (t("x PORT=8080 y"), format!("x {M} y")),
        (t("DEBUG=1"), M.to_owned()),
        (t("[FALSO_ENV=falso-anexo-01]"), format!("[{M}]")),
    ];
    for (output, expected) in cases {
        for out in [
            m.mask_all(output.as_bytes()),
            one_block(&m, output.as_bytes()),
        ] {
            assert_eq!(text(&out), expected, "{output}");
        }
    }
    // With quotes: `"abcd"` has 6 characters and becomes a pattern; without quotes (4),
    // it does not.
    let mut r = draft("teste-aspas", DataDraft::Generic);
    r.extras = vec![extra("X", "y", false)];
    r.attachments = vec![attachment("a.env", b"CURTO=\"abcd\"\n")];
    let m = mask_of(&valid(r));
    assert_eq!(text(&m.mask_all(b"v \"abcd\" v")), format!("v {M} v"));
    assert_eq!(text(&m.mask_all(b"v abcd v")), "v abcd v");

    // The field secret stays masked at any size (D11).
    let short = generic(&["ab12"]);
    let m = mask_of(&short);
    assert_eq!(text(&m.mask_all(b"x ab12 y")), format!("x {M} y"));
    assert_eq!(text(&one_block(&m, b"x ab12 y")), format!("x {M} y"));
}

#[test]
fn v2_38_mutation_without_minimum_would_mask_the_short_value() {
    // Control of the test above: the same short value as a field secret is masked, so
    // the intact output of V2.38 comes from the D34 rule, not from a search defect.
    let m = mask_of(&generic(&["8080"]));
    assert_eq!(text(&m.mask_all(b"porta 8080")), format!("porta {M}"));
}

// ---- Phase 19c: D36 (by line) and D37 (escaped forms) ----

/// Longest contiguous excerpt of the secret left in the output (the auditor's mask
/// repro).
fn longest_excerpt(output: &[u8], secret: &[u8]) -> usize {
    let mut best = 0;
    for i in 0..secret.len() {
        for j in (i + best + 1)..=secret.len() {
            if output.windows(j - i).any(|w| w == &secret[i..j]) {
                best = j - i;
            } else {
                break;
            }
        }
    }
    best
}

/// The last base64 line of the test key: short (fewer than 6 characters), like the
/// leftover at the end of a real key. It is the only excerpt that may be left (V2.41).
const SHORT_END: &str = "AQID";

/// Fake private key in the OpenSSH format, in several lines of 70 characters of
/// pseudo-random base64 (fixed seed) and a short last line. Built at run time (the
/// header text is not whole in the repository). Not a real key: only fake bytes with
/// the shape of one.
fn multi_line_key() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let label = ["OPENSSH", "PRIVATE", "KEY"].join(" ");
    let mut draw = Draw(0x0c4a_7e19);
    let mut c = format!("-----BEGIN {label}-----\n");
    for _ in 0..6 {
        for _ in 0..70 {
            c.push(char::from(ALPHABET[draw.below(64)]));
        }
        c.push('\n');
    }
    c.push_str(SHORT_END);
    c.push('\n');
    c.push_str(&format!("-----END {label}-----\n"));
    c
}

fn server_with_key(key: &str) -> Entry {
    valid(draft(
        "acme-ssh-chave-prod",
        DataDraft::Server {
            host: t("127.0.0.1"),
            port: t("2223"),
            username: t("deploy"),
            password: None,
            private_key: s(key),
            passphrase: s("falso-passphrase-01"),
            jump: String::new(),
        },
    ))
}

/// Every line of the key with 6 characters or more is gone (no piece of 6 of it is
/// left), and the longest excerpt of the key that is left is the short line at the end
/// with the newlines next to it.
fn no_key_line(case: &str, output: &[u8], key: &str) {
    for line in key.lines().filter(|l| l.chars().count() >= TRUSTED_MINIMUM) {
        for window in line.as_bytes().windows(TRUSTED_MINIMUM) {
            assert!(
                !output.windows(window.len()).any(|w| w == window),
                "{case}: sobrou pedaço de uma linha da chave: {}",
                text(output)
            );
        }
    }
    let longest = longest_excerpt(output, key.as_bytes());
    assert!(
        longest <= SHORT_END.len() + 2,
        "{case}: sobraram {longest} bytes seguidos da chave: {}",
        text(output)
    );
}

/// V2.41 (D36), the auditor's medium finding 1: the private key in a field (not an
/// attachment) was masked only whole. `grep -v -- -----`, `head -n 4`, the pty CRLF and
/// each line alone let almost everything through.
#[test]
fn v2_41_key_in_field_masked_by_line() {
    let key = multi_line_key();
    let e = server_with_key(&key);
    let m = mask_of(&e);
    let lines: Vec<&str> = key.lines().collect();

    let crlf = key.replace('\n', "\r\n");
    let head: String = lines[..4].iter().map(|l| format!("{l}\n")).collect();
    let middle: String = lines[1..lines.len() - 2]
        .iter()
        .map(|l| format!("{l}\n"))
        .collect();
    let mut cases: Vec<(String, String)> = vec![
        (t("cat"), key.clone()),
        (t("CRLF do pty"), crlf.clone()),
        (t("head -n 4"), head),
        (t("grep -v -- -----"), middle.clone()),
        (t("grep -v com CRLF"), middle.replace('\n', "\r\n")),
    ];
    for (i, l) in lines.iter().enumerate() {
        cases.push((format!("linha {i} sozinha"), format!("x {l} y\n")));
        cases.push((format!("linha {i} com CR"), format!("{l}\r\n")));
        cases.push((format!("linha {i} aparada"), format!("   {l}   \n")));
    }
    for (case, output) in &cases {
        no_key_line(case, &m.mask_all(output.as_bytes()), &key);
        no_key_line(case, &one_block(&m, output.as_bytes()), &key);
    }
    // The whole key is still a single `••••`.
    assert_eq!(text(&m.mask_all(key.as_bytes())), M);
}

/// V2.41 (D36): multi-line secret extra (service account JSON) and the value of
/// `"key": "value"`; and the `token: value` line of a YAML attachment.
#[test]
fn v2_41_multi_line_json_and_yaml() {
    const VALUE: &str = "falso-chave-de-conta-de-servico-0123456789";
    let json = format!("{{\n  \"type\": \"service_account\",\n  \"private_key\": \"{VALUE}\"\n}}");
    let mut r = draft("x-gen", DataDraft::Generic);
    r.extras = vec![extra("GCP", &json, true)];
    let m = mask_of(&valid(r));
    let line = format!("  \"private_key\": \"{VALUE}\"");
    for (case, output, expected) in [
        ("JSON inteiro", json.clone(), M.to_owned()),
        ("a linha", format!("{line}\n"), format!("{M}\n")),
        ("jq -r .private_key", format!("{VALUE}\n"), format!("{M}\n")),
        ("o valor com aspas", format!("\"{VALUE}\""), M.to_owned()),
        (
            "o valor no meio do texto",
            format!("chave={VALUE};"),
            format!("chave={M};"),
        ),
    ] {
        assert_eq!(text(&m.mask_all(output.as_bytes())), expected, "{case}");
        assert_eq!(text(&one_block(&m, output.as_bytes())), expected, "{case}");
    }

    // YAML attachment (kubeconfig): `token: value`, with and without quotes, and a list
    // item.
    let kube = "apiVersion: v1\nusers:\n- name: dev\n  user:\n    token: falso-token-kube-01\n    senha: \"falso-senha-kube-02\"\n- item-da-lista: falso-dados-kube-03\n";
    let mut r = draft("x-kube", DataDraft::Generic);
    r.extras = vec![extra("X", "y", false)];
    r.attachments = vec![attachment("kubeconfig", kube.as_bytes())];
    let m = mask_of(&valid(r));
    for value in [
        "falso-token-kube-01",
        "falso-senha-kube-02",
        "falso-dados-kube-03",
    ] {
        for output in [
            format!("{value}\n"),
            format!("v={value}."),
            format!("'{value}'"),
        ] {
            let masked = text(&m.mask_all(output.as_bytes()));
            assert!(!masked.contains(value), "{output} -> {masked}");
            assert!(masked.contains(M), "{output}");
        }
    }
    // `host:port` and a URL do not become `key: value` (no space after the colon).
    let mut r = draft("x-url", DataDraft::Generic);
    r.extras = vec![extra("X", "y", false)];
    r.attachments = vec![attachment("hosts.txt", b"db:5432\n")];
    let m = mask_of(&valid(r));
    assert_eq!(text(&m.mask_all(b"porta 5432")), "porta 5432");
}

/// D36 control: without the line pass on the field secrets, `grep -v` would let the key
/// through (the auditor's finding). The mask built only with the whole text, as it used
/// to be, leaves the lines.
#[test]
fn v2_41_control_without_line_pass_would_leak() {
    let key = multi_line_key();
    let e = server_with_key(&key);
    // Only the whole secret and its variations, without `lines` (the old code).
    let mut g = Generator::new();
    let mut patterns: Vec<Vec<u8>> = Vec::new();
    for sg in e.secrets() {
        g.secret(sg.value.expose(), &mut |p| {
            patterns.push(p.to_vec());
            Ok(())
        })
        .unwrap();
    }
    let lines: Vec<&str> = key.lines().collect();
    let middle: String = lines[1..lines.len() - 2]
        .iter()
        .map(|l| format!("{l}\n"))
        .collect();
    // No old pattern matches the middle: it would go out in plaintext.
    assert!(
        !patterns
            .iter()
            .any(|p| middle.as_bytes().windows(p.len()).any(|w| w == &p[..]))
    );
    // With the real mask, nothing is left.
    no_key_line("controle", &mask_of(&e).mask_all(middle.as_bytes()), &key);
}

fn database_with_password(password: &str) -> Entry {
    valid(draft(
        "x-pg",
        DataDraft::Database {
            engine: t("postgres"),
            host: t("127.0.0.1"),
            port: String::new(),
            db_name: t("app"),
            username: t("app"),
            password: s(password),
            tunnel: String::new(),
        },
    ))
}

/// V2.42 (D37), the low finding 5: Go's JSON (`docker inspect`), bash's `declare -p` and
/// Python's `json.dumps` write the password another way.
#[test]
fn v2_42_escaped_forms() {
    let password = "Ab3$k&<9>Zq!";
    let m = mask_of(&database_with_password(password));
    for (case, output, expected) in [
        (
            "JSON do Go",
            r#"{"Env":["PGPASSWORD=Ab3$k\u0026\u003c9\u003eZq!"]}"#,
            format!(r#"{{"Env":["PGPASSWORD={M}"]}}"#),
        ),
        (
            "declare -p",
            r#"declare -x PGPASSWORD="Ab3\$k&<9>Zq!""#,
            format!(r#"declare -x PGPASSWORD="{M}""#),
        ),
        ("literal", password, M.to_owned()),
        (
            "JSON padrão",
            r#"{"p":"Ab3$k&<9>Zq!"}"#,
            format!(r#"{{"p":"{M}"}}"#),
        ),
    ] {
        let masked = m.mask_all(output.as_bytes());
        assert_eq!(text(&masked), expected, "{case}");
        no_piece(&masked, password.as_bytes());
        assert_eq!(text(&one_block(&m, output.as_bytes())), expected, "{case}");
    }

    // `json.dumps` with `ensure_ascii` (Python's default): outside ASCII becomes `\uXXXX`.
    let m = mask_of(&database_with_password("senha-ação-01"));
    assert_eq!(
        text(&m.mask_all(br#"{"p": "senha-a\u00e7\u00e3o-01"}"#)),
        format!(r#"{{"p": "{M}"}}"#)
    );
    assert_eq!(text(&m.mask_all("senha-ação-01".as_bytes())), M);
    // Above U+FFFF, the surrogate pair.
    let m = mask_of(&database_with_password("falso-😀-01"));
    assert_eq!(text(&m.mask_all(br"falso-\ud83d\ude00-01")), M);
    assert_eq!(text(&m.mask_all("falso-😀-01".as_bytes())), M);
    // Go: U+2028 and U+2029 too.
    let m = mask_of(&database_with_password("falso\u{2028}linha\u{2029}01"));
    assert_eq!(text(&m.mask_all(br"falso\u2028linha\u202901")), M);

    // Output without secret with `&`, `<`, `$`, `\u0026` and quotes goes out unchanged.
    let m = mask_of(&database_with_password(password));
    for clean in [
        "a & b <c> $HOME \"x\" \\u0026 `y`",
        r#"{"Env":["HOME=/root","X=a\u0026b"]}"#,
        "declare -x PATH=\"/usr/bin\"",
    ] {
        assert_eq!(text(&m.mask_all(clean.as_bytes())), clean);
    }
}

/// D37: each form only exists when it differs from the literal (otherwise the pattern
/// would be repeated and would weigh on the budget for nothing).
#[test]
fn escaped_forms_only_when_they_change() {
    let count = |value: &str| {
        let mut g = Generator::new();
        let mut n = 0usize;
        g.secret(value.as_bytes(), &mut |_| {
            n += 1;
            Ok(())
        })
        .unwrap();
        n
    };
    let simple = count("falso-simples-01");
    assert_eq!(count("falso-simples-02"), simple);
    // `&`: Go's form and the `printf %q` one (D54). `$`: the double-quotes one and the
    // `printf %q` one. a non-ASCII letter: only the `ensure_ascii` one (`printf %q` does not apply
    // outside ASCII).
    assert_eq!(count("falso-simples&01"), simple + 2);
    assert_eq!(count("falso-simples$01"), simple + 2);
    assert_eq!(count("falso-simplesç01"), simple + 1);
}

// ---- Phase 19e: D54 (M2 re-audit, the auditor's repro) ----

const BASH: &str = "/bin/bash";
const PYTHON: &str = "/usr/bin/python3";

/// Runs `prog` with a clean environment (only `PATH`, UTF-8 locale and the given
/// variables) and returns the stdout. `None` when the program does not exist on this
/// machine (the test warns and goes on with the hand-built cases).
fn run(prog: &str, args: &[&str], env: &[(&str, &str)]) -> Option<Vec<u8>> {
    if !std::path::Path::new(prog).exists() {
        eprintln!("{prog} não existe aqui: caso do programa real pulado");
        return None;
    }
    let mut c = std::process::Command::new(prog);
    c.args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C.UTF-8");
    for (k, v) in env {
        c.env(k, v);
    }
    let output = c.output().expect("rodar o programa de teste");
    assert!(output.status.success(), "{prog} falhou");
    Some(output.stdout)
}

fn bash(script: &str, env: &[(&str, &str)]) -> Option<Vec<u8>> {
    run(BASH, &["--norc", "--noprofile", "-c", script], env)
}

fn python(script: &str, env: &[(&str, &str)]) -> Option<Vec<u8>> {
    run(PYTHON, &["-I", "-c", script], env)
}

/// The output as a reader would see it after undoing the JSON escapes (`\uXXXX`, with
/// surrogate pair, `\"`, `\\`, `\/`, `\n`, `\t`) and the shell backslash (`\x` becomes
/// `x`). Only to check that no excerpt of the secret is left, not even in the decoded
/// form.
fn decoded(output: &[u8]) -> Vec<u8> {
    let s = String::from_utf8_lossy(output);
    let c: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let hex4 = |c: &[char], i: usize| -> Option<u32> {
        let h: String = c.get(i..i + 4)?.iter().collect();
        u32::from_str_radix(&h, 16).ok()
    };
    while i < c.len() {
        if c[i] != '\\' || i + 1 >= c.len() {
            out.push(c[i]);
            i += 1;
            continue;
        }
        match c[i + 1] {
            'u' => match hex4(&c, i + 2) {
                Some(high @ 0xd800..=0xdbff)
                    if c.get(i + 6) == Some(&'\\') && c.get(i + 7) == Some(&'u') =>
                {
                    let low = hex4(&c, i + 8).unwrap_or(0);
                    let n = 0x10000 + ((high - 0xd800) << 10) + (low.wrapping_sub(0xdc00));
                    out.push(char::from_u32(n).unwrap_or('?'));
                    i += 12;
                }
                Some(n) => {
                    out.push(char::from_u32(n).unwrap_or('?'));
                    i += 6;
                }
                None => {
                    out.push('u');
                    i += 2;
                }
            },
            'n' => {
                out.push('\n');
                i += 2;
            }
            't' => {
                out.push('\t');
                i += 2;
            }
            other => {
                out.push(other);
                i += 2;
            }
        }
    }
    // `'\''` (shell single quotes) becomes `'` again.
    out.replace("'''", "'").into_bytes()
}

/// No excerpt of 6 bytes or more of the target is left in the masked output, neither raw
/// nor decoded. The message says the case, never the value.
fn no_excerpt_of_6(case: &str, m: &Mask, output: &[u8], target: &str) {
    let masked = m.mask_all(output);
    let target = target.as_bytes();
    assert!(
        longest_excerpt(&masked, target) < 6,
        "{case}: sobrou trecho do segredo"
    );
    assert!(
        longest_excerpt(&decoded(&masked), target) < 6,
        "{case}: sobrou trecho do segredo na forma decodificada"
    );
}

fn multi_line_api(token: &str) -> Entry {
    valid(draft(
        "beta-api-varias",
        DataDraft::Api {
            url: String::new(),
            token: s(token),
            variable: t("GITHUB_TOKEN"),
        },
    ))
}

/// V2.54 (D54): the multi-line `api` token goes through the line pass already by the
/// delivery, without the entry in the builder (`head -n1 <<<"$GITHUB_TOKEN"`, each line
/// alone, with `\r` and trimmed). The same with `Mask::from_entry`.
#[test]
fn v2_54_multi_line_delivery_by_line() {
    let one = "falso-tok-linha-um-01";
    let two = "falso-tok-linha-dois-02";
    let token = format!("{one}\n{two}");
    let e = multi_line_api(&token);
    let g = delivery_of(&e);
    let delivery_only = MaskBuilder::new().delivery(&g).build().unwrap();
    let full = Mask::from_entry(&e, Some(&g)).unwrap();
    let mut outputs: Vec<(String, Vec<u8>)> = vec![
        ("linha 1".into(), format!("{one}\n").into_bytes()),
        ("linha 2".into(), format!("{two}\n").into_bytes()),
        ("linha 1 com CR".into(), format!("{one}\r\n").into_bytes()),
        (
            "linha 2 aparada".into(),
            format!("  {two}  \n").into_bytes(),
        ),
    ];
    if let Some(o) = bash("head -n1 <<<\"$GITHUB_TOKEN\"", &[("GITHUB_TOKEN", &token)]) {
        assert_eq!(o, format!("{one}\n").into_bytes());
        outputs.push(("head -n1 (bash)".into(), o));
    }
    if let Some(o) = bash("tail -n1 <<<\"$GITHUB_TOKEN\"", &[("GITHUB_TOKEN", &token)]) {
        outputs.push(("tail -n1 (bash)".into(), o));
    }
    for (name, m) in [("só a entrega", &delivery_only), ("da_entrada", &full)] {
        for (case, output) in &outputs {
            for target in [one, two] {
                no_excerpt_of_6(&format!("{name}: {case}"), m, output, target);
            }
        }
    }
    // Control: without the line pass of the delivery (only the whole value), line 1
    // alone was left whole.
    let mut g_ = Generator::new();
    let mut patterns: Vec<Vec<u8>> = Vec::new();
    g_.secret(token.as_bytes(), &mut |p| {
        patterns.push(p.to_vec());
        Ok(())
    })
    .unwrap();
    assert!(!patterns.iter().any(|p| p == one.as_bytes()));
}

/// V2.55 (D54): the text forms apply by line. The lines of a multi-line secret extra,
/// written by Python's `json.dumps` (list of lines and one line in an object,
/// `ensure_ascii`), by bash's `declare -p` of a line with `$` and by Go's JSON, become
/// `••••`, with no 6+ excerpt even in the decoded form; and the output without secret
/// with `\u00e7` stays intact.
#[test]
fn v2_55_text_forms_by_line() {
    let lines = [
        "linha-um-ação-01",
        "linha-dois-ação-02",
        "linha-tres-Ab3$k&<9>Zq!",
    ];
    let note = lines.join("\n");
    let e = generic(&[&note]);
    let m = mask_of(&e);
    let env = [("NOTA", note.as_str())];
    let mut outputs: Vec<(&str, Vec<u8>)> = vec![
        // Go's JSON of line 3, built by hand (`encoding/json`).
        (
            "Go linha 3",
            br#"{"l":"linha-tres-Ab3$k\u0026\u003c9\u003eZq!"}"#.to_vec(),
        ),
        // `ensure_ascii` by hand, in case python does not exist.
        (
            "ensure_ascii linha 1",
            br#"["linha-um-a\u00e7\u00e3o-01"]"#.to_vec(),
        ),
        (
            "aspas duplas linha 3",
            br#"declare -- L="linha-tres-Ab3\$k&<9>Zq!""#.to_vec(),
        ),
    ];
    if let Some(o) = python(
        "import os,json;print(json.dumps(os.environ['NOTA'].splitlines()))",
        &env,
    ) {
        assert!(
            o.windows(6).any(|w| w == br"\u00e7"),
            "python sem ensure_ascii"
        );
        outputs.push(("json.dumps(splitlines())", o));
    }
    if let Some(o) = python(
        "import os,json;print(json.dumps({'l': os.environ['NOTA'].splitlines()[0]}))",
        &env,
    ) {
        outputs.push(("json.dumps({l: linha 1})", o));
    }
    if let Some(o) = bash("L=$(sed -n 3p <<<\"$NOTA\"); declare -p L", &env) {
        assert!(o.windows(2).any(|w| w == br"\$"), "declare -p sem a barra");
        outputs.push(("declare -p L (linha 3)", o));
    }
    for (case, output) in &outputs {
        for target in lines {
            no_excerpt_of_6(case, &m, output, target);
        }
    }
    for clean in [
        r#"{"cidade": "S\u00e3o Paulo", "a\u00e7\u00e3o": 1}"#,
        r#"["linha-um", "a\u00e7\u00e3o-02"]"#,
    ] {
        assert_eq!(text(&m.mask_all(clean.as_bytes())), clean);
    }
}

/// V2.56 (D54): `printf %q` (real bash) of the generator password; `${X@Q}`, `set` and
/// `set -x` with `'`; PHP's `json_encode` (`\/` + `\uXXXX`); and, in a multi-line extra,
/// the lines `key : value`, `key:\tvalue`, glued `"k":"v"` and a JSON key with a space
/// and with `/`, whole and only the value, plus the `cut -f1` of a line with tab.
/// Everything becomes `••••` and no 6+ excerpt is left. The RA3 cases are only logged.
#[test]
fn v2_56_new_forms_and_line_parser() {
    // `printf %q`.
    let password = "Ab3$k&<9>Zq!";
    let m = mask_of(&generic(&[password]));
    let mut outputs: Vec<(&str, Vec<u8>)> =
        vec![("printf %q à mão", br"Ab3\$k\&\<9\>Zq\!".to_vec())];
    if let Some(o) = bash("printf '%q\\n' \"$NOTA\"", &[("NOTA", password)]) {
        outputs.push(("printf %q (bash)", o));
    }
    for (case, output) in &outputs {
        no_excerpt_of_6(case, &m, output, password);
        assert_eq!(text(&m.mask_all(output)).trim_end(), M, "{case}");
    }

    // Shell single quotes.
    let quote = "falso-it's-01x";
    let m = mask_of(&generic(&[quote]));
    let mut outputs: Vec<(&str, Vec<u8>)> = vec![("@Q à mão", br"'falso-it'\''s-01x'".to_vec())];
    let env = [("NOTA", quote)];
    for (case, script) in [
        ("${NOTA@Q}", "echo \"${NOTA@Q}\""),
        ("set", "set | grep ^NOTA="),
        ("set -x stderr", "exec 2>&1; set -x; : \"$NOTA\""),
    ] {
        if let Some(o) = bash(script, &env) {
            outputs.push((case, o));
        }
    }
    for (case, output) in &outputs {
        no_excerpt_of_6(case, &m, output, quote);
    }

    // PHP's `json_encode`.
    let php = "falso/ação/01";
    let m = mask_of(&generic(&[php]));
    let mut outputs: Vec<(&str, Vec<u8>)> =
        vec![("PHP à mão", br#""falso\/a\u00e7\u00e3o\/01""#.to_vec())];
    if let Some(o) = python(
        "import os,json;print(json.dumps(os.environ['NOTA']).replace('/','\\\\/'))",
        &[("NOTA", php)],
    ) {
        outputs.push(("PHP simulado (python)", o));
    }
    for (case, output) in &outputs {
        assert_eq!(
            text(&m.mask_all(output)).trim_end(),
            format!("\"{M}\""),
            "{case}"
        );
        no_excerpt_of_6(case, &m, output, "falso");
    }

    // Line parser and tab fields.
    let lines = [
        ("senha2 : falso-yaml-esp-02", "falso-yaml-esp-02"),
        ("senha:\tfalso-yaml-tab-01", "falso-yaml-tab-01"),
        ("\"k\":\"falso-json-colado-03\"", "falso-json-colado-03"),
        (
            "  \"private key\": \"falso-valor-com-espaco-na-chave-01\",",
            "falso-valor-com-espaco-na-chave-01",
        ),
        (
            "  \"x/y\": \"falso-valor-barra-chave-02\"",
            "falso-valor-barra-chave-02",
        ),
        ("falso-um-linha\tcom-tab", "falso-um-linha"),
    ];
    let note: String = lines.iter().map(|(l, _)| *l).collect::<Vec<_>>().join("\n");
    let m = mask_of(&generic(&[&note]));
    for (line, value) in lines {
        assert_eq!(
            text(&m.mask_all(format!("{line}\n").as_bytes())),
            format!("{M}\n")
        );
        assert_eq!(
            text(&m.mask_all(format!("{value}\n").as_bytes())),
            format!("{M}\n")
        );
        no_excerpt_of_6("linha inteira", &m, line.as_bytes(), value);
    }
    if let Some(o) = bash("sed -n 6p <<<\"$NOTA\" | cut -f1", &[("NOTA", &note)]) {
        assert_eq!(o, b"falso-um-linha\n");
        assert_eq!(text(&m.mask_all(&o)), format!("{M}\n"));
    }
    if let Some(o) = bash("sed -n 6p <<<\"$NOTA\" | cut -f2", &[("NOTA", &note)]) {
        assert_eq!(text(&m.mask_all(&o)), format!("{M}\n"));
    }

    // RA3: logged, without requiring a mask (no value in the log).
    let ctl = "falso-ctl\u{1b}-01x";
    let m = mask_of(&generic(&[ctl]));
    if let Some(o) = bash("printf '%q\\n' \"$NOTA\"", &[("NOTA", ctl)]) {
        eprintln!(
            "RA3 printf %q com controle: mascarou = {}",
            m.mask_all(&o) != o
        );
    }
    let m = mask_of(&generic(&[&note]));
    if let Some(o) = bash("sed -n 6p <<<\"$NOTA\" | expand", &[("NOTA", &note)]) {
        eprintln!(
            "RA3 expand da linha com tab: sobra {} bytes",
            longest_excerpt(&m.mask_all(&o), b"falso-um-linha\tcom-tab")
        );
    }
    if let Some(o) = bash("sed -n 1p <<<\"$NOTA\" | base64 -w0", &[("NOTA", &note)]) {
        eprintln!(
            "RA3 base64 de uma linha: mascarou = {}",
            m.mask_all(&o) != o
        );
    }
}

/// D54: the mask's `printf %q` is equal to the real bash one for every visible ASCII
/// character and space, at the start, in the middle, at the end and after `=` and `:`;
/// and the single quotes, equal to the core of `${X@Q}`.
#[test]
fn v2_56_printf_q_equal_to_bash() {
    let mut values = Vec::new();
    for c in (0x20u8..=0x7e).map(char::from) {
        for v in [
            format!("a{c}b"),
            format!("{c}b"),
            format!("a{c}"),
            format!("a={c}b"),
            format!("a:{c}b"),
        ] {
            values.push(v);
        }
    }
    // A single process: bash prints one per line (no value has `\n`).
    let joined = values.join("\n");
    let Some(o) = bash(
        "while IFS= read -r v; do printf '%q\\n' \"$v\"; done <<<\"$VALORES\"",
        &[("VALORES", &joined)],
    ) else {
        return;
    };
    let from_bash: Vec<&[u8]> = o
        .strip_suffix(b"\n")
        .unwrap()
        .split(|b| *b == b'\n')
        .collect();
    assert_eq!(from_bash.len(), values.len());
    for (v, expected) in values.iter().zip(from_bash) {
        let mut ours = Zeroizing::new(Vec::new());
        printf_q(v.as_bytes(), &mut ours);
        assert_eq!(ours.as_slice(), expected, "printf %q de {v:?}");
        assert_eq!(
            printf_q_changes(v.as_bytes()),
            expected != v.as_bytes(),
            "{v:?}"
        );
    }
    let Some(o) = bash(
        "while IFS= read -r v; do echo \"${v@Q}\"; done <<<\"$VALORES\"",
        &[("VALORES", "falso-it's-01x\n'a''b'\nsem-aspa")],
    ) else {
        return;
    };
    let from_bash: Vec<&[u8]> = o
        .strip_suffix(b"\n")
        .unwrap()
        .split(|b| *b == b'\n')
        .collect();
    for (v, expected) in ["falso-it's-01x", "'a''b'", "sem-aspa"]
        .iter()
        .zip(from_bash)
    {
        let mut ours = Zeroizing::new(Vec::new());
        single_quotes(v.as_bytes(), &mut ours);
        let mut with_ends = b"'".to_vec();
        with_ends.extend_from_slice(&ours);
        with_ends.push(b'\'');
        assert_eq!(with_ends, expected, "@Q de {v:?}");
    }
}

/// D54: each new form only exists when it changes the value, and the loose line gets
/// no base64 nor cores (the base64 of a loose line stays in RA3).
#[test]
fn d54_new_forms_only_when_they_change_and_line_without_base64() {
    let count = |value: &str| {
        let mut g = Generator::new();
        let mut n = 0usize;
        g.text_forms(value.as_bytes(), &mut |_| {
            n += 1;
            Ok(())
        })
        .unwrap();
        n
    };
    assert_eq!(count("falso-simples-01"), 0);
    assert_eq!(count("falso-it's-01"), 2, "aspas simples e %q");
    assert_eq!(count("falso/simples/01"), 1, "só o JSON com \\/");
    assert_eq!(count("falso-ação-01"), 1, "só o ensure_ascii");
    assert_eq!(count("falso/ação/01"), 3, "\\/, ensure_ascii e PHP");
    assert_eq!(count("falso simples"), 1, "só o %q");
    assert_eq!(count("falso~a"), 0, "~ no meio não muda o %q");
    assert_eq!(count("~falso-a"), 1, "~ no começo muda o %q");

    let line = "falso-linha-solta-0123456789";
    let mut b64 = Zeroizing::new(Vec::new());
    base64(line.as_bytes(), B64_STD, false, &mut b64);
    let mut g = Generator::new();
    let mut patterns: Vec<Vec<u8>> = Vec::new();
    g.lines(
        format!("{line}\noutra-linha-falsa\n").as_bytes(),
        &mut |p| {
            patterns.push(p.to_vec());
            Ok(())
        },
    )
    .unwrap();
    assert!(patterns.iter().any(|p| p == line.as_bytes()));
    assert!(
        !patterns
            .iter()
            .any(|p| p.windows(8).any(|w| b64.windows(8).any(|x| x == w))),
        "a linha solta ganhou base64"
    );
}

/// V2.57 (D54), with values the fixtures do not have: the oracle (`dev/variations.py`,
/// run by python, without calling anything of the mask) generates the patterns of
/// values with `$`, `&`, `<`, `'`, `/`, accent, tab and key and value lines, and each
/// pattern, alone, goes out `••••` through the mask of its entry. The D54 table is the
/// same on both ends.
#[test]
fn v2_57_oracle_and_mask_with_d54_values() {
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dev/variations.py");
    if !std::path::Path::new(PYTHON).exists() {
        eprintln!("{PYTHON} não existe aqui: teste cruzado pulado");
        return;
    }
    let extras = [
        "falso-Ab3$k&<9>Zq!",
        "falso-it's-01x",
        "falso/ação/01",
        "falso com espaço ~ #01",
        "linha-um-ação-01\nlinha-dois-ação-02\nlinha-tres-Ab3$k&<9>Zq!",
        "senha2 : falso-yaml-esp-02\nsenha:\tfalso-yaml-tab-01\n\"k\":\"falso-json-colado-03\"\n  \"private key\": \"falso-valor-com-espaco-na-chave-01\",\n  \"x/y\": \"falso-valor-barra-chave-02\"\nfalso-um-linha\tcom-tab\nexport FALSO_X='falso-it''s-02'\n",
    ];
    let attachment_ =
        "FALSO_ENV=falso-anexo-01\nPORT=8080\nnome\tfalso-tsv-valor-01\tfalso/ação\r\n";
    let db_password = "falso-PG$&<x>'q/ç-01";
    let mut r = draft("teste-generico", DataDraft::Generic);
    r.extras = extras
        .iter()
        .enumerate()
        .map(|(i, v)| extra(&format!("S{i}"), v, true))
        .collect();
    r.attachments = vec![attachment("app.env", attachment_.as_bytes())];
    let generic = valid(r);
    let database = valid(draft(
        "teste-banco",
        DataDraft::Database {
            engine: t("postgres"),
            host: t("127.0.0.1"),
            port: t("55432"),
            db_name: t("app_teste"),
            username: t("app"),
            password: s(db_password),
            tunnel: String::new(),
        },
    ));
    // Request to the oracle through stdin: group, username (or empty), attachment (0/1),
    // value in hex.
    let mut request = String::new();
    for v in extras {
        request.push_str(&format!("g\t\t0\t{}\n", hex(v.as_bytes())));
    }
    request.push_str(&format!("g\t\t1\t{}\n", hex(attachment_.as_bytes())));
    request.push_str(&format!("b\tapp\t0\t{}\n", hex(db_password.as_bytes())));
    let script = "import importlib.util,sys\n\
        spec=importlib.util.spec_from_file_location('variations',sys.argv[1])\n\
        v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)\n\
        for line in sys.stdin.read().splitlines():\n\
        \x20   g,u,a,h=line.split('\\t')\n\
        \x20   for p in sorted(v.variations(bytes.fromhex(h),u or None,attachment=a=='1')):\n\
        \x20       print(g+'\\t'+p.hex())\n";
    let mut child = std::process::Command::new(PYTHON)
        .args(["-I", "-B", "-c", script, source])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write as _;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(request.as_bytes())
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "o oráculo falhou");
    let m_g = mask_of(&generic);
    let m_b = mask_of(&database);
    let mut n = 0usize;
    for line in output
        .stdout
        .split(|b| *b == b'\n')
        .filter(|l| !l.is_empty())
    {
        let (group, h) = line.split_at(1);
        let pattern = from_hex(&h[1..]);
        let m = if group == b"g" { &m_g } else { &m_b };
        assert_eq!(
            m.mask_all(&pattern),
            M.as_bytes(),
            "padrão nº {n} do oráculo não saiu {M}"
        );
        n += 1;
    }
    assert!(n > 200, "o oráculo gerou poucos padrões ({n})");
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn from_hex(h: &[u8]) -> Vec<u8> {
    h.chunks(2)
        .map(|p| u8::from_str_radix(core::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}

// ---- Phase 22a: D58 (Go with `ensure_ascii`), D59 and D61 (the auditor's 19e repro) ----

/// Runs bash with variables of arbitrary bytes (the Latin-1 attachment is not `&str`).
/// `None` without bash on this machine.
fn bash_bytes(script: &str, env: &[(&str, &[u8])]) -> Option<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt as _;
    if !std::path::Path::new(BASH).exists() {
        eprintln!("{BASH} não existe aqui: caso do programa real pulado");
        return None;
    }
    let mut c = std::process::Command::new(BASH);
    c.args(["--norc", "--noprofile", "-c", script])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C.UTF-8");
    for (k, v) in env {
        c.env(k, std::ffi::OsStr::from_bytes(v));
    }
    let output = c.output().expect("rodar o bash de teste");
    assert!(output.status.success(), "bash falhou");
    Some(output.stdout)
}

/// The oracle (`dev/variations.py`, by python, without the mask) generates the patterns
/// of each request (mask, username, attachment, value) and each pattern, alone, has to
/// go out `••••` through the mask of the request. Returns how many patterns it checked
/// (0 without python).
fn oracle_covers(requests: &[(&Mask, Option<&str>, bool, &[u8])]) -> usize {
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dev/variations.py");
    if !std::path::Path::new(PYTHON).exists() {
        eprintln!("{PYTHON} não existe aqui: teste cruzado pulado");
        return 0;
    }
    let mut request = String::new();
    for (i, (_, u, a, v)) in requests.iter().enumerate() {
        request.push_str(&format!(
            "{i}\t{}\t{}\t{}\n",
            u.unwrap_or(""),
            u8::from(*a),
            hex(v)
        ));
    }
    let script = "import importlib.util,sys\n\
        spec=importlib.util.spec_from_file_location('variations',sys.argv[1])\n\
        v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)\n\
        for line in sys.stdin.read().splitlines():\n\
        \x20   g,u,a,h=line.split('\\t')\n\
        \x20   for p in sorted(v.variations(bytes.fromhex(h),u or None,attachment=a=='1')):\n\
        \x20       print(g+'\\t'+p.hex())\n";
    let mut child = std::process::Command::new(PYTHON)
        .args(["-I", "-B", "-c", script, source])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write as _;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(request.as_bytes())
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "o oráculo falhou");
    let mut n = 0usize;
    for line in output
        .stdout
        .split(|b| *b == b'\n')
        .filter(|l| !l.is_empty())
    {
        let tab = line.iter().position(|b| *b == b'\t').unwrap();
        let i: usize = core::str::from_utf8(&line[..tab]).unwrap().parse().unwrap();
        let pattern = from_hex(&line[tab + 1..]);
        assert_eq!(
            requests[i].0.mask_all(&pattern),
            M.as_bytes(),
            "padrão nº {n} do oráculo (pedido {i}) não saiu {M}"
        );
        n += 1;
    }
    n
}

/// The patterns that the `Generator` builds for a multi-line secret (or attachment).
fn line_patterns(c: &[u8]) -> Vec<Vec<u8>> {
    let mut g = Generator::new();
    let mut patterns = Vec::new();
    g.lines(c, &mut |p| {
        patterns.push(p.to_vec());
        Ok(())
    })
    .unwrap();
    patterns
}

const TOKEN_D59: &str = "falso-Ab3$k&<9>Zq!'/ç-01";
const ASCII_D59: &str = "falso-Ab3$k&<9>Zq!'";

/// The D59 entry of the test vault, built as in the contract table.
fn delta_api_symbols() -> Entry {
    let mut r = draft(
        "delta-api-simbolos",
        DataDraft::Api {
            url: t("https://api.teste.invalid"),
            token: s(TOKEN_D59),
            variable: t("SIMBOLOS_TOKEN"),
        },
    );
    r.group = t("delta");
    r.extras = vec![extra("SEGREDO_ASCII", ASCII_D59, true)];
    valid(r)
}

/// Python that prints the value of `V` as "html-safe" JSON with `ensure_ascii` (the
/// default `json.dumps` plus `&<>` in `\u`, like Jinja's `htmlsafe_json_dumps` and
/// Django's `json_script`). `{}` is the expression of the value.
fn py_html_safe(expr: &str) -> String {
    format!(
        "import os,json;\
         h=lambda s:s.replace('&','\\\\u0026').replace('<','\\\\u003c').replace('>','\\\\u003e');\
         print(h(json.dumps({expr})))"
    )
}

/// V3.29 (D58, D59): Go with `ensure_ascii` of the token of the D59 entry (the
/// contract form and the real python one), the shell and PHP forms; each pattern of the
/// oracle of the entry goes out `••••`; the cost fits in the D29 budget.
#[test]
fn v3_29_delta_api_symbols_and_go_with_ensure_ascii() {
    let e = delta_api_symbols();
    let m = mask_of(&e);
    let cost = entry_cost(&e).unwrap();
    assert!(cost <= MASK_BUDGET, "D29: {cost}");
    let contract = br"falso-Ab3$k\u0026\u003c9\u003eZq!'/\u00e7-01".to_vec();
    let mut outputs: Vec<(&str, Vec<u8>)> = vec![
        ("Go + ensure_ascii (contrato)", contract.clone()),
        ("PHP à mão", br"falso-Ab3$k&<9>Zq!'\/\u00e7-01".to_vec()),
    ];
    for (case, script) in [
        (
            "html-safe + ensure_ascii (python)",
            py_html_safe("os.environ['V']"),
        ),
        (
            "ensure_ascii (python)",
            t("import os,json;print(json.dumps(os.environ['V']))"),
        ),
    ] {
        if let Some(o) = python(&script, &[("V", TOKEN_D59)]) {
            outputs.push((case, o));
        }
    }
    for (case, script) in [
        ("${V@Q}", "echo \"${V@Q}\""),
        ("declare -p", "declare -p V"),
    ] {
        if let Some(o) = bash(script, &[("V", TOKEN_D59)]) {
            outputs.push((case, o));
        }
    }
    for (case, output) in &outputs {
        no_excerpt_of_6(case, &m, output, TOKEN_D59);
    }
    if let Some(o) = bash("printf '%q\\n' \"$V\"", &[("V", ASCII_D59)]) {
        no_excerpt_of_6("printf %q do SEGREDO_ASCII", &m, &o, ASCII_D59);
    }
    // The whole contract form becomes a single mark.
    assert_eq!(text(&m.mask_all(&contract)), M);
    // The oracle of the entry (token and extra), alone, pattern by pattern.
    let n = oracle_covers(&[
        (&m, None, false, TOKEN_D59.as_bytes()),
        (&m, None, false, ASCII_D59.as_bytes()),
    ]);
    assert!(n == 0 || n > 20, "o oráculo gerou poucos padrões ({n})");
}

/// V3.29 (D58): Go with `ensure_ascii` also by line of a multi-line secret and in the
/// line value; and a binary attachment line stays masked by the literal, without text
/// forms (D54).
#[test]
fn v3_29_go_with_ensure_ascii_by_line_and_binary_line() {
    let lines = [
        "linha-um-ação-01",
        "linha-dois-Ab3$k&<9>Zq!-ação-02",
        "TOKEN=falso-tok&<ç>-03",
    ];
    let note = lines.join("\n");
    let e = generic(&[&note]);
    let m = mask_of(&e);
    let mut outputs: Vec<(&str, Vec<u8>)> = vec![(
        "linha 2 à mão",
        br#"["linha-dois-Ab3$k\u0026\u003c9\u003eZq!-a\u00e7\u00e3o-02"]"#.to_vec(),
    )];
    if let Some(o) = python(
        &py_html_safe("os.environ['V'].splitlines()"),
        &[("V", &note)],
    ) {
        outputs.push(("html-safe + ensure_ascii das linhas (python)", o));
    }
    if let Some(o) = python(
        &py_html_safe("os.environ['V']"),
        &[("V", "falso-tok&<ç>-03")],
    ) {
        outputs.push(("valor de linha (python)", o));
    }
    for (case, output) in &outputs {
        for target in lines.iter().chain(["falso-tok&<ç>-03"].iter()) {
            no_excerpt_of_6(case, &m, output, target);
        }
    }
    // Only when it changes: without `&<>` or without a character outside ASCII, no extra
    // form.
    let count = |value: &str| {
        let mut g = Generator::new();
        let mut n = 0usize;
        g.text_forms(value.as_bytes(), &mut |_| {
            n += 1;
            Ok(())
        })
        .unwrap();
        n
    };
    assert_eq!(count("falso-ação-01"), 1, "só o ensure_ascii");
    assert_eq!(count("falso-a&b-01"), 2, "Go e %q");
    assert_eq!(count("falso-a&ç-01"), 3, "Go, ensure_ascii e D58");
    assert_eq!(
        count("falso\u{2028}ç-01"),
        2,
        "U+2028 sem &<>: Go e ensure_ascii"
    );
    // Binary line: the literal (with and without `\r`, trimmed) and no text form.
    let binary: &[u8] = b"falso-bin-\xff&<x>-linha-01 \r\nFALSO_ENV=falso-anexo-01\n";
    let patterns = line_patterns(binary);
    assert!(
        patterns
            .iter()
            .any(|p| p == b"falso-bin-\xff&<x>-linha-01 ")
    );
    assert!(patterns.iter().any(|p| p == b"falso-bin-\xff&<x>-linha-01"));
    assert!(
        !patterns
            .iter()
            .any(|p| p.windows(6).any(|w| w == br"\u0026")),
        "linha binária ganhou forma de texto"
    );
    let mut r = draft("teste-binario", DataDraft::Generic);
    r.extras = vec![extra("OBS", "nada", false)];
    r.attachments = vec![attachment("cfg.bin", binary)];
    let mb = mask_of(&valid(r));
    assert_eq!(
        mb.mask_all(b"falso-bin-\xff&<x>-linha-01\n"),
        format!("{M}\n").as_bytes()
    );
}

/// V3.31 (D61): JSON in a line. The minified `client.json` attachment, the single-line
/// extra `{"user":…,"token":…}`, the nested object in the line and two keys in the line
/// of a multi-line extra, and a value with `\u00e7` and `\/` printed decoded, each one by
/// the real output of python's `json.loads`; no 6+ excerpt is left, and the oracle
/// generates the same patterns.
#[test]
fn v3_31_json_in_a_line() {
    let client: &[u8] =
        b"{\"installed\":{\"client_id\":\"x.apps\",\"client_secret\":\"falso-GOCSPX-minificado-01\"}}\n";
    let one = r#"{"user":"x","token":"falso-json-uma-linha-01"}"#;
    let several = "{\n  \"db\": {\"senha\": \"falso-json-aninhado-02\"},\n  \"a\": \"x\", \"b\": \"falso-json-duas-chaves-03\"\n}";
    let escaped = r#"{"k": "falso-a\u00e7\/b-json-04"}"#;
    let mut r = draft("teste-json", DataDraft::Generic);
    r.extras = vec![
        extra("UM", one, true),
        extra("VARIAS", several, true),
        extra("ESC", escaped, true),
    ];
    r.attachments = vec![attachment("client.json", client)];
    let e = valid(r);
    let m = mask_of(&e);
    let cost = entry_cost(&e).unwrap();
    assert!(cost <= MASK_BUDGET, "D29: {cost}");
    let client_text = core::str::from_utf8(client).unwrap();
    let cases: [(&str, &str, &str, &str); 5] = [
        (
            "anexo minificado, .installed.client_secret",
            client_text,
            "print(json.loads(os.environ['V'])['installed']['client_secret'])",
            "falso-GOCSPX-minificado-01",
        ),
        (
            "extra de uma linha, .token",
            one,
            "print(json.loads(os.environ['V'])['token'])",
            "falso-json-uma-linha-01",
        ),
        (
            "objeto aninhado na linha",
            several,
            "print(json.loads(os.environ['V'])['db']['senha'])",
            "falso-json-aninhado-02",
        ),
        (
            "duas chaves na linha",
            several,
            "print(json.loads(os.environ['V'])['b'])",
            "falso-json-duas-chaves-03",
        ),
        (
            "\\u00e7 e \\/ decodificados",
            escaped,
            "print(json.loads(os.environ['V'])['k'])",
            "falso-aç/b-json-04",
        ),
    ];
    for (case, v, script, target) in cases {
        let mut outputs = vec![format!("{target}\n").into_bytes()];
        if let Some(o) = python(&format!("import os,json;{script}"), &[("V", v)]) {
            assert_eq!(
                o,
                format!("{target}\n").into_bytes(),
                "{case}: o python leu outro valor"
            );
            outputs.push(o);
        }
        for output in &outputs {
            no_excerpt_of_6(case, &m, output, target);
            assert_eq!(text(&m.mask_all(output)), format!("{M}\n"), "{case}");
        }
    }
    // An invalid escape discards the pair; an array element without key stays in RA3
    // (only logged).
    let patterns =
        line_patterns(br#"  "k": "falso-invalido-\x-01", "s": "falso-sozinho-\ud800-02","#);
    assert!(!patterns.iter().any(|p| p == b"falso-invalido-x-01"));
    assert!(!patterns.iter().any(|p| p.starts_with(b"falso-sozinho-")));
    let arr = generic(&[r#"["falso-elemento-array-01"]"#]);
    let masked = mask_of(&arr).mask_all(b"falso-elemento-array-01\n");
    eprintln!(
        "RA3 (array JSON sem chave): {}",
        if longest_excerpt(&masked, b"falso-elemento-array-01") >= 6 {
            "não mascarado"
        } else {
            "mascarado"
        }
    );
    // The oracle gives the same patterns.
    let n = oracle_covers(&[
        (&m, None, true, client),
        (&m, None, false, one.as_bytes()),
        (&m, None, false, several.as_bytes()),
        (&m, None, false, escaped.as_bytes()),
    ]);
    assert!(n == 0 || n > 40, "o oráculo gerou poucos padrões ({n})");
}

/// V3.32 (D61): YAML and dotenv (trailing comment, key with a space and with `/`, `''`
/// of single quotes) as `yq` and bash's `source` print them, and the tab fields of a
/// Latin-1 attachment line by the real `cut -f2`/`-f3`; the cost of both entries fits
/// in the D29 budget, and the oracle generates the same patterns.
#[test]
fn v3_32_yaml_dotenv_and_tab_of_binary_line() {
    let note = "url: https://x.example\nsenha: falso-yaml-coment-01 # comentario\nprivate key: falso-yaml-chave-espaco-02\naws/secret: falso-yaml-chave-barra-03\n  aninhado:\n    senha: falso-yaml-indent-04\nSENHA_DB=\"falso-env-coment-05\" # dotenv\ntok: 'falso-it''s-yaml-06'";
    let e = generic(&[note]);
    let m = mask_of(&e);
    assert!(entry_cost(&e).unwrap() <= MASK_BUDGET);
    for target in [
        "falso-yaml-coment-01",
        "falso-yaml-chave-espaco-02",
        "falso-yaml-chave-barra-03",
        "falso-yaml-indent-04",
        "falso-it's-yaml-06",
    ] {
        // As `yq` prints the value (decoded, without quotes nor comment).
        let output = format!("{target}\n");
        no_excerpt_of_6(target, &m, output.as_bytes(), target);
        assert_eq!(text(&m.mask_all(output.as_bytes())), format!("{M}\n"));
    }
    if let Some(o) = bash(
        "eval \"$(sed -n 7p <<<\"$NOTA\")\"; echo \"$SENHA_DB\"",
        &[("NOTA", note)],
    ) {
        assert_eq!(o, b"falso-env-coment-05\n", "o source leu outro valor");
        no_excerpt_of_6("dotenv pelo source", &m, &o, "falso-env-coment-05");
    }
    // Latin-1 attachment (not UTF-8) with tab fields.
    let mut att: Vec<u8> = b"# config\n".to_vec();
    att.extend_from_slice(b"SENHA=falso-latin\xe7-01\tTOKEN=falso-campo-tab-02\n");
    att.extend_from_slice(b"nome\xe7\tfalso-campo-tsv-03\tfalso-campo-tsv-04\n");
    let mut r = draft("teste-latin1", DataDraft::Generic);
    r.extras = vec![extra("OBS", "nada", false)];
    r.attachments = vec![attachment("cfg.ini", &att)];
    let ea = valid(r);
    let ma = mask_of(&ea);
    assert!(entry_cost(&ea).unwrap() <= MASK_BUDGET);
    for (case, script, target) in [
        (
            "cut -f2 (KEY=v)",
            "printf '%s' \"$A\" | sed -n 2p | cut -f2",
            "TOKEN=falso-campo-tab-02",
        ),
        (
            "cut -f2 (TSV)",
            "printf '%s' \"$A\" | sed -n 3p | cut -f2",
            "falso-campo-tsv-03",
        ),
        (
            "cut -f3 (TSV)",
            "printf '%s' \"$A\" | sed -n 3p | cut -f3",
            "falso-campo-tsv-04",
        ),
    ] {
        let mut outputs = vec![format!("{target}\n").into_bytes()];
        if let Some(o) = bash_bytes(script, &[("A", &att)]) {
            assert_eq!(
                o,
                format!("{target}\n").into_bytes(),
                "{case}: o cut deu outro campo"
            );
            outputs.push(o);
        }
        for output in &outputs {
            assert_eq!(text(&ma.mask_all(output)), format!("{M}\n"), "{case}");
        }
    }
    // Only by the literal: the binary field gets no base64 nor text form.
    let patterns = line_patterns(&att);
    let mut b64 = Zeroizing::new(Vec::new());
    base64(b"falso-campo-tsv-04", B64_STD, true, &mut b64);
    assert!(
        !patterns.iter().any(|p| p[..] == b64[..]),
        "campo binário com base64"
    );
    // `iconv` and `repr()` stay in RA3: only logged.
    if let Some(o) = bash_bytes(
        "printf '%s' \"$A\" | sed -n 3p | iconv -f latin1 -t utf-8",
        &[("A", &att)],
    ) {
        let masked = ma.mask_all(&o);
        eprintln!(
            "RA3 (iconv da linha Latin-1): {}",
            if longest_excerpt(&masked, b"falso-campo-tsv-03") >= 6 {
                "não mascarado"
            } else {
                "mascarado"
            }
        );
    }
    let n = oracle_covers(&[(&m, None, false, note.as_bytes()), (&ma, None, true, &att)]);
    assert!(n == 0 || n > 40, "o oráculo gerou poucos padrões ({n})");
}

/// D61: each extra candidate only goes in when it differs from what the line already
/// gives (the D29 budget does not pay twice for the same value).
#[test]
fn d61_candidates_only_when_they_change() {
    let count = |line: &[u8], target: &[u8]| {
        line_patterns(line)
            .iter()
            .filter(|p| p[..] == *target)
            .count()
    };
    assert_eq!(
        count(b"senha: falso-yaml-simples-01", b"falso-yaml-simples-01"),
        1
    );
    assert_eq!(
        count(
            br#""k": "falso-json-simples-01","#,
            b"falso-json-simples-01"
        ),
        1
    );
    assert_eq!(
        count(b"SENHA=falso-env-simples-01", b"falso-env-simples-01"),
        1
    );
    // The compound key that gives the same value as the old parser does not repeat the
    // value.
    assert_eq!(
        count(b"chave_x : falso-yaml-esp-01", b"falso-yaml-esp-01"),
        1
    );
    // URL, comment and YAML flow do not become a compound key.
    assert!(compound_key_value(b"https://host.invalid:8080/x").is_none());
    assert!(compound_key_value(b"# nota: x").is_none());
    assert!(compound_key_value(b"{k: falso-flow-01}").is_none());
    assert_eq!(
        compound_key_value(b"  - private key: falso-x-01,"),
        Some(&b"falso-x-01"[..])
    );
}

// ---- D34 revised (M4 audit): no prefix oracle ----
//
// The audit attack (its pty oracle script): the command prints, in chunks separated by a
// pause, `\nQ` + prefix + candidate, and looks whether that chunk went out different
// from what came in (mark in its place, or held until the pause/until more output).
// With the mask that held the prefix, the right candidate was the only different one,
// and the depth-first search recovered the password. Here the same attack runs against
// the stream, with the mask of every pattern family (literal, percent, base64 and
// cores, Basic, URL, text forms, lines and line value of an attachment, JSON and YAML in
// the line, tab field, private key by line), and has to find nothing.

const SSH_PASSWORD: &str = "falso-SSH-senha-01";

/// The mask with one entry of each pattern family.
fn mask_of_every_family() -> Mask {
    let pg = pg_dev();
    let g = delivery_of(&pg);
    let gama = gama_generic();
    let mut r = draft("anexo-familias", DataDraft::Generic);
    r.extras = vec![
        extra("S_ASPAS", "falso\"a&b<c>'d$e", true),
        extra("S_SSH", SSH_PASSWORD, true),
    ];
    r.attachments = vec![attachment(
        "conf",
        b"{\"api_token\": \"falso-json-tok-77\"}\nchave yaml: falso-yaml-val-88\ncol1\tfalso-tab-campo-99\nexport DB_PASS=falso-dotenv-55 # nota\n",
    )];
    let families = valid(r);
    let key = server_with_key(&multi_line_key());
    MaskBuilder::new()
        .entry(&pg)
        .delivery(&g)
        .entry(&gama)
        .entry(&families)
        .entry(&key)
        .build()
        .unwrap()
}

/// The audit oracle: did the chunk `\nQ` + prefix + candidate go out different from
/// what came in, or was something held (which, in the daemon, would become a delay or a
/// mark after the pause)?
fn oracle_answers(f: &mut Stream<'_>, prefix: &[u8], c: u8) -> bool {
    let mut block = b"\nQ".to_vec();
    block.extend_from_slice(prefix);
    block.push(c);
    let mut out = Vec::new();
    f.mask(&block, &mut out);
    let held = f.emitted != f.consumed || !f.pending.is_empty();
    held || out != block
}

#[test]
fn d34_attack_audit_prefix_oracle_recovers_nothing() {
    let m = mask_of_every_family();
    let alphabet: Vec<u8> = (0x20..0x7f).collect();
    // Depth-first search like the one of the audit pty oracle script, in the same stream (the
    // interactive session is a single stream), starting from empty.
    let mut f = m.stream();
    let mut stack: Vec<Vec<u8>> = vec![Vec::new()];
    let mut queries = 0usize;
    let mut accepted = Vec::new();
    while let Some(p) = stack.pop() {
        // Ceiling: against the mask that held, the search went on for thousands of
        // queries.
        if p.len() >= 40 || queries > 20_000 {
            break;
        }
        for &c in &alphabet {
            queries += 1;
            if oracle_answers(&mut f, &p, c) {
                let mut q = p.clone();
                q.push(c);
                accepted.push(q.clone());
                stack.push(q);
            }
        }
    }
    assert!(queries >= alphabet.len());
    assert!(
        accepted.is_empty(),
        "o oráculo distinguiu {} prefixo(s)",
        accepted.len()
    );
}

#[test]
fn d34_attack_right_and_wrong_prefix_give_the_same_answer() {
    // The attacker who already knows k characters (of any variation) and tests the next
    // one: the answer is the same for the right candidate and for all the wrong ones, up
    // to the last character of the pattern (then they already had the whole secret, but
    // one character: it is the limit of any mask, D34). It holds for each form of the
    // single-line secret.
    let m = mask_of_every_family();
    let alphabet: Vec<u8> = (0x20..0x7f).collect();
    let go = |c: u8| format!("\\u{:04x}", c);
    // Slack at the end: in base64, the core of the secret (its own pattern) ends up to a
    // group of 4 and the `=` before the end; there the attacker already wrote the secret
    // but 1 or 2 bytes, which is the same limit.
    let base64 = [
        b64(PG_PASSWORD.as_bytes(), false, true),
        b64(PG_PASSWORD.as_bytes(), true, false),
        b64(format!("app:{PG_PASSWORD}").as_bytes(), false, true),
    ];
    let mut forms = vec![
        t(PG_PASSWORD),
        pct(PG_PASSWORD, false, false),
        pct(PG_PASSWORD, true, true),
        t(SSH_PASSWORD),
        // Go's JSON (`&`, `<`, `>` as `\u00XX`) and shell double quotes.
        format!(r#"falso\"a{}b{}c{}'d$e"#, go(b'&'), go(b'<'), go(b'>')),
        t(r#"falso\"a&b<c>'d\$e"#),
        t("falso-json-tok-77"),
        t("falso-yaml-val-88"),
        t("falso-tab-campo-99"),
        t("falso-dotenv-55"),
        t("falso-licenca-01"),
    ];
    forms.dedup();
    let forms = forms
        .into_iter()
        .map(|f| (f, 1))
        .chain(base64.into_iter().map(|f| (f, 7)));
    for (form, slack) in forms {
        let v = form.as_bytes();
        for k in 0..v.len().saturating_sub(slack) {
            // The right character (which does not end the form) changes nothing.
            let mut f = m.stream();
            assert!(
                !oracle_answers(&mut f, &v[..k], v[k]),
                "{form}: o prefixo de {} bytes mudou a saída ou foi segurado",
                k + 1
            );
            // A wrong one only changes the output when it itself closes the whole
            // secret by another form (the raw `#` after `%40` in the percent): near the
            // end, with the whole secret already written.
            for &c in alphabet.iter().filter(|&&c| c != v[k]) {
                let mut g = m.stream();
                assert!(
                    !oracle_answers(&mut g, &v[..k], c) || k + 3 >= v.len(),
                    "{form}: o candidato {:?} no caractere {k} mudou a saída",
                    char::from(c)
                );
            }
        }
    }
}

#[test]
fn d34_attack_timing_nothing_held_until_the_rest_arrives() {
    // The audit timing script (without pty): `A`, pause, candidate, pause, `Z`. With the
    // mask that held, the candidate that was the start of the password only arrived
    // together with the `Z`. Here each chunk goes out whole at the moment it is read.
    let m = mask_of_every_family();
    for candidate in [
        "qqq",
        "fal",
        "fax",
        "falso-S",
        "falso-X",
        "falso-PG:s3nha/01@",
    ] {
        let mut f = m.stream();
        for chunk in [&b"A"[..], candidate.as_bytes(), b"Z\n"] {
            let mut out = Vec::new();
            f.mask(chunk, &mut out);
            nothing_held(&f);
            assert_eq!(out, chunk, "{candidate}: o pedaço não saiu na hora");
        }
    }
}

#[test]
fn d34_pty_stopped_prefix_goes_out_in_plaintext_and_the_rest_becomes_mark() {
    // The prompt that ends in a password start goes out as it came (no mark in its
    // place, which was the oracle); if the rest arrives later, the chunk that completes
    // the password goes out marked; if it diverges, it goes out normal.
    let m = mask_of(&generic(&[SSH_PASSWORD]));
    let mut f = m.stream();
    let mut out = Vec::new();
    f.mask(b"senha? fals", &mut out);
    nothing_held(&f);
    assert_eq!(text(&out), "senha? fals");
    f.mask(b"o-SSH-senha-01 fim", &mut out);
    f.finish(&mut out);
    assert_eq!(text(&out), format!("senha? fals{M} fim"));

    let mut f = m.stream();
    let mut out = Vec::new();
    f.mask(b"x fals", &mut out);
    f.mask(b"e normal", &mut out);
    f.finish(&mut out);
    assert_eq!(text(&out), "x false normal");
}

#[test]
fn d34_half_percent_escape_at_chunk_end_goes_out_right_away() {
    // The half `%X` at the end of a chunk used to be held until the next one; now it
    // goes out.
    let m = mask_of(&pg_dev());
    for end in ["%", "%4", "falso-PG%3", "falso%2DP"] {
        let mut f = m.stream();
        let mut out = Vec::new();
        f.mask(end.as_bytes(), &mut out);
        nothing_held(&f);
        assert_eq!(text(&out), end);
    }
}
