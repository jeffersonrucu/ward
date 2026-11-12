//! Output mask (D11, S4): replaces with `••••` every secret value that shows up in the
//! command output (stdout and stderr, each in its own [`Stream`]), including when the
//! value arrives whole in one chunk of the output (D34 revised: nothing is held; see
//! "Stream").
//!
//! What goes into the mask, for each secret of the entry (secret fields of the type,
//! secret extras, passphrase, TOTP seed) and for each variable that the
//! [`Delivery`] marked as secret (`Variable::is_secret`, which includes the whole
//! `DATABASE_URL`):
//! - the literal value;
//! - percent-encoding in any form (upper case, lower case, `+` for space, partial
//!   encoding like the one of `urllib.parse.quote`): it does not become a pattern, the
//!   output is also read decoded (see "Views");
//! - standard and URL-safe base64, with and without `=`, and the core of the base64 of
//!   the secret in the three alignments (finds the secret inside a larger base64, like
//!   `x:token`);
//! - base64 of `username:password` (Basic auth) with the username of the entry;
//! - the **text forms** (D37, D54), each one only when it differs from the literal
//!   value: the standard JSON escape (`\"`, `\\`, controls), with and without `\/`; Go's
//!   JSON (`&`, `<`, `>`, U+2028 and U+2029 as `\u0026`, `\u003c`, `\u003e`, `\u2028`,
//!   `\u2029`); Python's JSON with `ensure_ascii` (every character outside ASCII as
//!   `\uXXXX` in lower case, with a surrogate pair above U+FFFF); Go's JSON with
//!   `ensure_ascii` (D58: both together, only when the value has `&`, `<` or `>` and a
//!   character outside ASCII); PHP's `json_encode`
//!   (`\/` and `\uXXXX` together, only when the value has `/` and a character outside
//!   ASCII); the shell double quotes (`declare -p`, `export -p`: `\` before `$`, `` ` ``,
//!   `"` and `\`); the shell single quotes (`${X@Q}`, `set`, `set -x`: `'` becomes
//!   `'\''`, only when the value has `'`); and bash's `printf %q` (backslash before each
//!   special character, only for a value in visible ASCII and space; with a control or
//!   outside ASCII bash switches to `$'…'`, which stays in RA3).
//!
//! Attachments: the whole content (and without the final newline) and its base64, and
//! the line pass. Every field, extra or delivery secret with `\n` (the private key of
//! the server and ftp/sftp types, a service account JSON in an extra, a multi-line
//! `api` token) also goes through the line pass (D36, D54), besides whole. The line pass
//! covers: each line with 6 characters or more, without the trailing `\r` and also
//! trimmed, with its text forms (no base64 nor cores, D54); the 6+ fields of a line with
//! tab (`cut -f`, TSV); and the value of `KEY=value` lines (with or without `export`),
//! `"key": "value"` (JSON, key with any character but `"`, also glued `"k":"v"`) and
//! `key: value` (YAML, also `key : value` and `key:\tvalue`), with and without quotes,
//! with 6 characters or more (D34). The line value and the tab field have all the
//! variations of a whole secret.
//!
//! Re-audit of 19e (D61), in the same line pass:
//! - **JSON in the line:** in every text line, and in a single-line secret that, trimmed,
//!   starts with `{` or `[`, each pair `"key"` + `:` + JSON string, at any position of
//!   the line (minified object, nested, two keys in the line), gives the string decoded
//!   by the JSON escapes as a line value (6+; an invalid escape discards the pair). An
//!   array element without key and the YAML flow stay in RA3;
//! - **trailing comment** (YAML and dotenv): unquoted value up to before the first space
//!   or tab followed by `#`; quoted value followed by spaces and `# …` gives the core;
//! - **compound YAML key** (`private key:`, `aws/secret:`): the key goes up to the first
//!   `:` followed by space, tab or end, without `#` nor quotes, and does not start with
//!   `-`, `{`, `[`;
//! - **YAML `''`:** in a single-quoted value, `''` becomes `'`;
//! - **tab fields of a line that is not text:** each field of 6+ bytes only by the
//!   literal (trimmed and raw) and by the percent, without text forms nor base64.
//!
//! Each extra candidate only goes in when it differs from what the line already gives.
//!
//! Budget (D29): [`entry_cost`] counts the patterns of an entry and of its delivery by
//! the same [`Generator`]; validation refuses the entry above [`MASK_BUDGET`].
//!
//! The TOTP code is not masked (the agent receives it, R8): `WARD_TOTP` is not a secret
//! in the delivery. A short secret (fewer than 6 characters) is masked all the same;
//! [`Mask::has_short_secret`] warns the app.
//!
//! # Views
//! Each [`Stream`] passes the bytes through three readings of the same output in the
//! same automaton: raw, with `%XX` decoded and with `%XX` and `+` decoded. A match in a
//! decoded view is mapped back to the original bytes. So no combination of
//! percent-encoding escapes, without multiplying patterns.
//!
//! # Stream
//! The stream **never holds nor delays** output (D34 revised, M4 audit): each chunk read
//! from the command goes out right away, whole, with `••••` only in place of what
//! completed a pattern (the whole secret, or a whole variation) up to the end of that
//! chunk. Holding the end that could be the start of a secret became an oracle:
//! releasing the prefix as a mark (in the pty) or only after more output (without pty)
//! told the command, by timing or by what showed up, whether a prefix was the start of
//! the password, and the agent recovered the password character by character. Now the
//! output of a chunk depends only on its bytes and on the previous ones, never on how
//! much of them matches half a secret.
//!
//! The automaton state carries over from one chunk to the next: a secret that arrives
//! split between two chunks has the end chunk masked (the start, which already went out,
//! stays as it went). The S4 guarantee is the whole secret inside one same chunk (D34: a
//! secret printed in pieces is not a usage scenario). This opens no new oracle: the mark
//! only shows up when the output already contains the whole pattern.
//!
//! Covered excerpts that overlap or touch become a single `••••`. What is not secret
//! goes out byte for byte the same (invalid UTF-8 and binary included). The chunk being
//! worked on stays in memory that is zeroed.
//!
//! Outside the threat model (the scope's security section): a process made to dodge the mask
//! (reversing the password, printing half on stdout and half on stderr). What stops it
//! is the approval.

mod automaton;

use core::fmt;
use std::collections::VecDeque;

use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::delivery::{self, Delivery, DeliveryError, ResourceError, Resources};
use crate::entry::{Data, Entry, Secret, SecretOrigin};
use automaton::{Automaton, reserve};

#[cfg(test)]
mod tests;

/// What replaces each secret excerpt.
pub const MARK: &str = "••••";

// The limits live in `crate::limits` (D32); they are re-exported here too.
pub use crate::limits::{MASK_BUDGET, PATTERN_LIMIT, TRUSTED_MINIMUM};

/// Why the mask was not built. Fixed text only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskError {
    /// The secrets (usually, large attachments) go over [`PATTERN_LIMIT`] when building,
    /// or over [`MASK_BUDGET`] in [`entry_cost`].
    TooLarge,
    /// [`entry_cost`]: the delivery of the entry could not be built (defense in depth;
    /// validation should have stopped it before).
    NoDelivery,
}

impl fmt::Display for MaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MaskError::TooLarge => {
                f.write_str("máscara não montada: os segredos da entrada são grandes demais")
            }
            MaskError::NoDelivery => {
                f.write_str("máscara não montada: a entrega da entrada falhou")
            }
        }
    }
}

impl std::error::Error for MaskError {}

/// Gathers the entries (the main one and the one of the jump or of the tunnel) and the
/// deliveries whose secrets the mask covers.
#[derive(Default)]
pub struct MaskBuilder<'a> {
    entries: Vec<&'a Entry>,
    deliveries: Vec<&'a Delivery>,
}

impl fmt::Debug for MaskBuilder<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MaskBuilder(••••)")
    }
}

impl<'a> MaskBuilder<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Covers the secrets of the entry (V2.13) and the Basic auth with its username.
    pub fn entry(mut self, entry: &'a Entry) -> Self {
        self.entries.push(entry);
        self
    }

    /// Covers the values that the delivery marked as secret (`DATABASE_URL`, password…).
    pub fn delivery(mut self, delivery: &'a Delivery) -> Self {
        self.deliveries.push(delivery);
        self
    }

    pub fn build(self) -> Result<Mask, MaskError> {
        // First pass: only sums, to allocate the automaton once at the right size.
        let total = sum_patterns(&self, PATTERN_LIMIT)?;
        let mut automaton = Automaton::with_capacity(total + 1);
        visit(&self, &mut |p| {
            automaton.insert(p);
            Ok(())
        })?;
        automaton.finalize();
        let short = self.entries.iter().any(|e| {
            e.secrets().iter().any(|s| {
                !matches!(s.origin, SecretOrigin::Attachment(_))
                    && char_count(s.value.expose()) < TRUSTED_MINIMUM
            })
        });
        Ok(Mask { automaton, short })
    }
}

/// The built mask. No `Clone`; zeroed on drop; `Debug` shows nothing.
pub struct Mask {
    automaton: Automaton,
    short: bool,
}

// All the automaton tables are `Zeroizing`.
impl ZeroizeOnDrop for Mask {}

impl fmt::Debug for Mask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Mask(••••)")
    }
}

impl Mask {
    /// Shortcut: the mask of an entry and, if any, of its delivery.
    pub fn from_entry(entry: &Entry, delivery: Option<&Delivery>) -> Result<Self, MaskError> {
        let mut c = MaskBuilder::new().entry(entry);
        if let Some(g) = delivery {
            c = c.delivery(g);
        }
        c.build()
    }

    /// Some secret (besides the attachments) has fewer than 6 characters: the app warns
    /// that the mask becomes unreliable (D11).
    pub fn has_short_secret(&self) -> bool {
        self.short
    }

    /// A new stream. One for stdout and another for stderr (D11: separately).
    pub fn stream(&self) -> Stream<'_> {
        Stream {
            mask: self,
            pending: Zeroizing::new(Vec::new()),
            emitted: 0,
            consumed: 0,
            views: [
                View::new(Mode::Raw),
                View::new(Mode::Percent),
                View::new(Mode::Form),
            ],
            covered: Vec::new(),
            in_mark: false,
        }
    }

    /// Masks a whole output at once (same as a stream with a single chunk).
    pub fn mask_all(&self, command_output: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(command_output.len());
        let mut f = self.stream();
        f.mask(command_output, &mut out);
        f.finish(&mut out);
        out
    }
}

// ---------------------------------------------------------------------------------
// Budget (D29)
// ---------------------------------------------------------------------------------

/// Sum of the patterns that [`MaskBuilder::build`] would generate, by the same function
/// ([`visit`] and the [`Generator`]). Stops as soon as it goes over `limit`.
pub(crate) fn sum_patterns(c: &MaskBuilder<'_>, limit: usize) -> Result<usize, MaskError> {
    let mut total: usize = 0;
    visit(c, &mut |p| {
        total = total
            .checked_add(p.len())
            .filter(|t| *t <= limit)
            .ok_or(MaskError::TooLarge)?;
        Ok(())
    })?;
    Ok(total)
}

/// Fake resources to measure the cost of the delivery without touching the system. The
/// values they give are not secret (D11: the TOTP code and the path do not go into the
/// mask); the tunnel port is the longest one, so the measured `DATABASE_URL` is never
/// shorter than the one of `ward run` (with a tunnel the host is always `127.0.0.1`).
struct BudgetResources;

impl Resources for BudgetResources {
    fn totp_code(&mut self, _seed: &Secret) -> Result<Zeroizing<String>, ResourceError> {
        Ok(Zeroizing::new(String::from("000000")))
    }

    fn file(&mut self, _name: &str, _content: &Secret) -> Result<String, ResourceError> {
        Ok(String::from("/orcamento"))
    }

    fn tunnel(&mut self, _server: &str, _host: &str, _port: u16) -> Result<u16, ResourceError> {
        Ok(u16::MAX)
    }
}

/// Cost of the entry in the mask (D29): the sum of the patterns of the entry and of its
/// delivery, counted by the same [`Generator`] that builds the mask of `ward run`.
/// Refuses with [`MaskError::TooLarge`] above [`MASK_BUDGET`]. The secure note has no
/// delivery (D10), so it stays out of the budget and costs 0.
///
/// Since each entry fits in half of [`PATTERN_LIMIT`], the two of a combined use (D19)
/// fit together: no entry accepted by the vault is left without `ward run`.
pub fn entry_cost(entry: &Entry) -> Result<usize, MaskError> {
    cost(entry, false)
}

/// [`entry_cost`] of an entry read from the vault body (m2 Phase 54b). The only
/// difference: an entry with an extra or attachment of 0.1.x named like a variable that
/// only 0.2.0 delivers is accepted with the cost of the entry alone. `ward run` refuses
/// it ([`DeliveryError::ReservedName`]), so no
/// mask with its delivery is ever built; the masks of `ward ssh`, of the tunnel and of
/// the jump take the entry alone, and that cost stays under [`MASK_BUDGET`] too.
pub(crate) fn stored_entry_cost(entry: &Entry) -> Result<usize, MaskError> {
    cost(entry, true)
}

fn cost(entry: &Entry, stored: bool) -> Result<usize, MaskError> {
    let delivery = match delivery::build(entry, &mut BudgetResources) {
        Ok(g) => g,
        Err(DeliveryError::TypeWithoutDelivery) => return Ok(0),
        Err(DeliveryError::ReservedName) if stored => {
            return sum_patterns(&MaskBuilder::new().entry(entry), MASK_BUDGET);
        }
        Err(_) => return Err(MaskError::NoDelivery),
    };
    let c = MaskBuilder::new().entry(entry).delivery(&delivery);
    sum_patterns(&c, MASK_BUDGET)
}

// ---------------------------------------------------------------------------------
// Patterns
// ---------------------------------------------------------------------------------

type Visitor<'f> = dyn FnMut(&[u8]) -> Result<(), MaskError> + 'f;

fn visit(c: &MaskBuilder<'_>, f: &mut Visitor<'_>) -> Result<(), MaskError> {
    let mut g = Generator::new();
    for e in &c.entries {
        let username = username_of(e.data());
        for s in e.secrets() {
            let value = s.value.expose();
            match s.origin {
                SecretOrigin::Attachment(_) => g.attachment(value, f)?,
                // D36: a multi-line secret also by line, like the attachments.
                SecretOrigin::Extra(_) => g.field_secret(value, f)?,
                SecretOrigin::Field(_) => {
                    g.field_secret(value, f)?;
                    if let Some(u) = username {
                        g.basic(u.as_bytes(), value, f)?;
                    }
                }
            }
        }
    }
    for g_ in &c.deliveries {
        for v in g_.variables().iter().filter(|v| v.is_secret()) {
            // D54: a secret delivery value with `\n` (the multi-line `api` token) also
            // by line, even without `.entry(…)` in the builder; and the single-line
            // JSON (D61), the same way.
            g.field_secret(v.expose().as_bytes(), f)?;
        }
    }
    Ok(())
}

fn username_of(d: &Data) -> Option<&str> {
    match d {
        Data::Login(l) => Some(&l.username),
        Data::Database(b) => Some(&b.username),
        Data::Server(s) => Some(&s.username),
        Data::Smtp(s) => Some(&s.username),
        Data::FtpSftp(s) => Some(&s.username),
        Data::Api(_) | Data::Aws(_) | Data::SecureNote(_) | Data::Generic => None,
    }
}

const B64_STD: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const B64_URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Base64 (RFC 4648) of `data`, appended to `out`. Not crypto: just encoding.
fn base64(data: &[u8], alphabet: &[u8; 64], with_padding: bool, out: &mut Zeroizing<Vec<u8>>) {
    reserve(out, data.len().div_ceil(3) * 4);
    for block in data.chunks(3) {
        let b0 = block[0];
        let b1 = block.get(1).copied().unwrap_or(0);
        let b2 = block.get(2).copied().unwrap_or(0);
        let n = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        let chars = block.len() + 1;
        for i in 0..4 {
            if i < chars {
                out.push(alphabet[((n >> (18 - 6 * i)) & 63) as usize]);
            } else if with_padding {
                out.push(b'=');
            }
        }
    }
}

/// Builds the patterns of each secret in buffers that are zeroed (and that grow without
/// leaving a copy: [`reserve`]).
struct Generator {
    tmp: Zeroizing<Vec<u8>>,
    tmp2: Zeroizing<Vec<u8>>,
    /// Decoded value (JSON string, YAML `''`) that becomes a whole secret. It leaves the
    /// `Generator` while in use ([`core::mem::take`]) and comes back zeroed.
    tmp3: Zeroizing<Vec<u8>>,
}

impl Generator {
    fn new() -> Self {
        Generator {
            tmp: Zeroizing::new(Vec::new()),
            tmp2: Zeroizing::new(Vec::new()),
            tmp3: Zeroizing::new(Vec::new()),
        }
    }

    /// Field, extra or delivery secret: whole; by line if it has `\n` (D36, D54); and, if
    /// it is a single text line that, trimmed, starts with `{` or `[`, its JSON values
    /// (D61).
    fn field_secret(&mut self, s: &[u8], f: &mut Visitor<'_>) -> Result<(), MaskError> {
        self.secret(s, f)?;
        if s.contains(&b'\n') {
            self.lines(s, f)?;
        } else if matches!(s.trim_ascii().first(), Some(b'{' | b'['))
            && core::str::from_utf8(s).is_ok()
        {
            self.line_json(s, None, f)?;
        }
        Ok(())
    }

    /// Line value (D34): a whole secret, if it has the minimum.
    fn min_value(&mut self, v: &[u8], f: &mut Visitor<'_>) -> Result<(), MaskError> {
        if char_count(v) >= TRUSTED_MINIMUM {
            self.secret(v, f)?;
        }
        Ok(())
    }

    /// The `"key": "string"` pairs of the text line (D61), at any position: each decoded
    /// string with 6+ characters is a line value. `already` is the value that the line
    /// already gave through the `key: value` parser (not repeated).
    fn line_json(
        &mut self,
        line: &[u8],
        already: Option<&[u8]>,
        f: &mut Visitor<'_>,
    ) -> Result<(), MaskError> {
        let mut buf = core::mem::take(&mut self.tmp3);
        let mut r = Ok(());
        let mut i = 0usize;
        while let Some(p) = line[i..].iter().position(|b| *b == b'"') {
            let Some(key_end) = json_string_end(line, i + p) else {
                break;
            };
            let j = skip_json_blank(line, key_end);
            if line.get(j) != Some(&b':') {
                i = key_end;
                continue;
            }
            let j = skip_json_blank(line, j + 1);
            if line.get(j) != Some(&b'"') {
                i = key_end;
                continue;
            }
            let Some(value_end) = json_string_end(line, j) else {
                break;
            };
            buf.clear();
            if decode_json_string(&line[j + 1..value_end - 1], &mut buf)
                && already != Some(&buf[..])
            {
                r = self.min_value(&buf, f);
                if r.is_err() {
                    break;
                }
            }
            i = value_end;
        }
        buf.zeroize();
        self.tmp3 = buf;
        r
    }

    /// The extra candidates of a `KEY=value` or `key: value` value (D61): trailing
    /// comment and, in YAML (`yaml`), the `''` of single quotes.
    fn comment_and_quotes(
        &mut self,
        value: &[u8],
        yaml: bool,
        f: &mut Visitor<'_>,
    ) -> Result<(), MaskError> {
        let Some(&q) = value.first() else {
            return Ok(());
        };
        if !matches!(q, b'"' | b'\'') {
            if let Some(c) = before_comment(value) {
                self.min_value(c, f)?;
            }
            return Ok(());
        }
        let base = unquoted(value);
        let quoted_core = core_before_comment(value);
        if let Some(m) = quoted_core
            && Some(m) != base
        {
            self.min_value(m, f)?;
        }
        if yaml
            && q == b'\''
            && let Some(inside) = quoted_core.or(base)
            && inside.windows(2).any(|w| w == b"''")
        {
            let mut buf = core::mem::take(&mut self.tmp3);
            buf.clear();
            reserve(&mut buf, inside.len());
            let mut k = 0;
            while k < inside.len() {
                buf.push(inside[k]);
                k += if inside[k] == b'\'' && inside.get(k + 1) == Some(&b'\'') {
                    2
                } else {
                    1
                };
            }
            let r = self.min_value(&buf, f);
            buf.zeroize();
            self.tmp3 = buf;
            r?;
        }
        Ok(())
    }

    fn secret(&mut self, s: &[u8], f: &mut Visitor<'_>) -> Result<(), MaskError> {
        if s.is_empty() {
            return Ok(());
        }
        f(s)?;
        for alphabet in [B64_STD, B64_URL] {
            self.tmp.clear();
            base64(s, alphabet, true, &mut self.tmp);
            f(&self.tmp)?;
            let no_padding = self
                .tmp
                .iter()
                .rposition(|b| *b != b'=')
                .map_or(0, |i| i + 1);
            f(&self.tmp[..no_padding])?;
        }
        // Core of the base64 in each alignment: the characters that depend only on the
        // bytes of the secret, to find it inside a larger base64 (`username:token`,
        // JSON…).
        if s.len() >= TRUSTED_MINIMUM {
            for offset in 0..3usize {
                self.tmp2.clear();
                reserve(&mut self.tmp2, offset + s.len());
                self.tmp2.extend(core::iter::repeat_n(0u8, offset));
                self.tmp2.extend_from_slice(s);
                let start = (8 * offset).div_ceil(6);
                let end = 8 * (offset + s.len()) / 6;
                for alphabet in [B64_STD, B64_URL] {
                    self.tmp.clear();
                    base64(&self.tmp2, alphabet, false, &mut self.tmp);
                    if end > start && end - start >= 8 {
                        f(&self.tmp[start..end])?;
                    }
                }
            }
        }
        self.text_forms(s, f)
    }

    /// The text forms of D37 and D54, each one only when it differs from the literal
    /// (and, for JSON, from the standard escape).
    fn text_forms(&mut self, s: &[u8], f: &mut Visitor<'_>) -> Result<(), MaskError> {
        for slash in [false, true] {
            if json_changes(s, slash) {
                self.tmp.clear();
                json(s, slash, &mut self.tmp);
                f(&self.tmp)?;
            }
        }
        if let Ok(text) = core::str::from_utf8(s) {
            for mode in [
                EscapedJson::Go,
                EscapedJson::Ascii,
                EscapedJson::Php,
                EscapedJson::GoAscii,
            ] {
                if escaped_json_changes(text, mode) {
                    self.tmp.clear();
                    escaped_json(text, mode, &mut self.tmp);
                    f(&self.tmp)?;
                }
            }
        }
        if s.iter().any(|b| double_quotes_escapes(*b)) {
            self.tmp.clear();
            double_quotes(s, &mut self.tmp);
            f(&self.tmp)?;
        }
        if s.contains(&b'\'') {
            self.tmp.clear();
            single_quotes(s, &mut self.tmp);
            f(&self.tmp)?;
        }
        if printf_q_changes(s) {
            self.tmp.clear();
            printf_q(s, &mut self.tmp);
            f(&self.tmp)?;
        }
        Ok(())
    }

    /// Basic auth: base64 of `username:password`.
    fn basic(&mut self, u: &[u8], s: &[u8], f: &mut Visitor<'_>) -> Result<(), MaskError> {
        if s.is_empty() {
            return Ok(());
        }
        self.tmp2.clear();
        reserve(&mut self.tmp2, u.len() + 1 + s.len());
        self.tmp2.extend_from_slice(u);
        self.tmp2.push(b':');
        self.tmp2.extend_from_slice(s);
        for alphabet in [B64_STD, B64_URL] {
            self.tmp.clear();
            base64(&self.tmp2, alphabet, true, &mut self.tmp);
            f(&self.tmp)?;
            let no_padding = self
                .tmp
                .iter()
                .rposition(|b| *b != b'=')
                .map_or(0, |i| i + 1);
            f(&self.tmp[..no_padding])?;
        }
        Ok(())
    }

    fn attachment(&mut self, c: &[u8], f: &mut Visitor<'_>) -> Result<(), MaskError> {
        if c.is_empty() {
            return Ok(());
        }
        f(c)?;
        f(without_final_newline(c))?;
        for alphabet in [B64_STD, B64_URL] {
            self.tmp.clear();
            base64(c, alphabet, true, &mut self.tmp);
            f(&self.tmp)?;
            let no_padding = self
                .tmp
                .iter()
                .rposition(|b| *b != b'=')
                .map_or(0, |i| i + 1);
            f(&self.tmp[..no_padding])?;
        }
        self.lines(c, f)
    }

    /// The line pass (D11, D34, D36, D54), of the attachments and of every multi-line
    /// secret: each line with [`TRUSTED_MINIMUM`] characters or more, without the
    /// trailing `\r` and also trimmed (the CRLF output of the pty, `head`, `grep -v`,
    /// `sed -n` take loose lines), with its text forms (`json.dumps(lines)`, `declare -p`
    /// of one line), without base64 nor cores (D54: the base64 of a loose line stays in
    /// RA3); the fields of a line with tab (`cut -f`); and the value of `KEY=value`,
    /// `"key": "value"` and `key: value`, with the same minimum and the variations of a
    /// secret (`cut -d=`, `jq -r`, `yq`).
    fn lines(&mut self, c: &[u8], f: &mut Visitor<'_>) -> Result<(), MaskError> {
        for line in c.split(|b| *b == b'\n') {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if char_count(line) >= TRUSTED_MINIMUM {
                // The text forms and the tab fields only apply to a line that is text
                // (valid UTF-8): a line of a binary attachment does not go through
                // `json.dumps`, `printf %q` nor `cut -f` without becoming something
                // else, and with them a 256 KiB binary would go over the D29 budget.
                let text = core::str::from_utf8(line).is_ok();
                f(line)?;
                if text {
                    self.text_forms(line, f)?;
                }
                let trimmed = line.trim_ascii();
                if trimmed.len() != line.len() && char_count(trimmed) >= TRUSTED_MINIMUM {
                    f(trimmed)?;
                    if text {
                        self.text_forms(trimmed, f)?;
                    }
                }
                // D54: each field of a text line with tab is a line value.
                if text && line.contains(&b'\t') {
                    for field in line.split(|b| *b == b'\t') {
                        self.line_value(field.trim_ascii(), f)?;
                    }
                }
                // D61: in the line that is not text, the tab field only by the literal
                // (and by the percent, which the automaton reads in every form),
                // trimmed and raw, 6+ bytes, when it differs from the line and from the
                // trimmed line.
                if !text && line.contains(&b'\t') {
                    for field in line.split(|b| *b == b'\t') {
                        let trimmed_field = field.trim_ascii();
                        let raw = (trimmed_field.len() != field.len()).then_some(field);
                        for c in core::iter::once(trimmed_field).chain(raw) {
                            if c.len() >= TRUSTED_MINIMUM && c != line && c != trimmed {
                                f(c)?;
                            }
                        }
                    }
                }
                // D34: the value only becomes its own pattern with the same minimum as
                // the lines; without it, `PORT=8080` would mask every `8080` of the
                // output. The whole line stays masked (above).
                if let Some(value) = key_value(line) {
                    self.line_value(value, f)?;
                    self.comment_and_quotes(value, false, f)?;
                }
                let colon = colon_value(line);
                if let Some(value) = colon {
                    self.line_value(value, f)?;
                    self.comment_and_quotes(value, true, f)?;
                }
                if let Some(value) = compound_key_value(line)
                    && Some(value) != colon
                {
                    self.line_value(value, f)?;
                    self.comment_and_quotes(value, true, f)?;
                }
                // D61: the `"key": "string"` pairs at any position of the line.
                if text {
                    self.line_json(line, colon.and_then(unquoted), f)?;
                }
            }
        }
        Ok(())
    }

    /// The value of a key and value line, with and without quotes, if it has the
    /// minimum.
    fn line_value(&mut self, value: &[u8], f: &mut Visitor<'_>) -> Result<(), MaskError> {
        if char_count(value) >= TRUSTED_MINIMUM {
            self.secret(value, f)?;
        }
        if let Some(unquoted) = unquoted(value)
            && char_count(unquoted) >= TRUSTED_MINIMUM
        {
            self.secret(unquoted, f)?;
        }
        Ok(())
    }
}

fn without_final_newline(c: &[u8]) -> &[u8] {
    let c = c.strip_suffix(b"\n").unwrap_or(c);
    c.strip_suffix(b"\r").unwrap_or(c)
}

/// Characters of valid UTF-8; bytes if it is not text.
fn char_count(b: &[u8]) -> usize {
    match core::str::from_utf8(b) {
        Ok(s) => s.chars().count(),
        Err(_) => b.len(),
    }
}

/// The value of a `KEY=value` line (or `export KEY=value`), trimmed.
fn key_value(line: &[u8]) -> Option<&[u8]> {
    let line = line.trim_ascii();
    let line = line.strip_prefix(b"export ").unwrap_or(line);
    let equals = line.iter().position(|b| *b == b'=')?;
    let key = line[..equals].trim_ascii();
    let first = *key.first()?;
    if first.is_ascii_digit()
        || !key
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
    {
        return None;
    }
    let value = line[equals + 1..].trim_ascii();
    (!value.is_empty()).then_some(value)
}

/// The value of a `"key": "value"` line (JSON, with or without the trailing comma, also
/// glued `"k":"v"`, with a key of any character but `"`) or `key: value` (YAML, also as
/// an item `- key: value`, `key : value` and `key:\tvalue`), trimmed. The YAML key is
/// `[A-Za-z0-9_.-]` (with space or tab before the colon) and needs a space, tab (or end)
/// after it, so as not to confuse it with `https://…` or `host:port`.
fn colon_value(line: &[u8]) -> Option<&[u8]> {
    let l = line.trim_ascii();
    let l = l.strip_prefix(b"- ").map_or(l, <[u8]>::trim_ascii_start);
    let rest = if let Some(after) = l.strip_prefix(b"\"") {
        let end = after.iter().position(|b| *b == b'"')?;
        if end == 0 {
            return None;
        }
        after[end + 1..].trim_ascii_start().strip_prefix(b":")?
    } else {
        let p = l.iter().position(|b| *b == b':')?;
        let key = l[..p].trim_ascii_end();
        if key.is_empty()
            || !key
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
        {
            return None;
        }
        let r = &l[p + 1..];
        if !r.is_empty() && !matches!(r[0], b' ' | b'\t') {
            return None;
        }
        r
    };
    let mut v = rest.trim_ascii();
    if let Some(no_comma) = v.strip_suffix(b",") {
        v = no_comma.trim_ascii_end();
    }
    (!v.is_empty()).then_some(v)
}

/// Compound YAML key (D61): `private key: v`, `aws/secret: v`. After the indent (and the
/// `- ` of an item), the key goes up to the first `:` followed by space, tab or end,
/// without `#` nor quotes, and does not start with `-`, `{` nor `[`. The value comes out
/// as in [`colon_value`] (trimmed, without the trailing comma).
fn compound_key_value(line: &[u8]) -> Option<&[u8]> {
    let l = line.trim_ascii();
    let l = l.strip_prefix(b"- ").map_or(l, <[u8]>::trim_ascii_start);
    let p = (0..l.len())
        .find(|&p| l[p] == b':' && l.get(p + 1).is_none_or(|b| matches!(b, b' ' | b'\t')))?;
    let key = &l[..p];
    let first = *key.first()?;
    if matches!(first, b'-' | b'{' | b'[') || key.iter().any(|b| matches!(b, b'#' | b'"' | b'\'')) {
        return None;
    }
    let mut v = l[p + 1..].trim_ascii();
    if let Some(no_comma) = v.strip_suffix(b",") {
        v = no_comma.trim_ascii_end();
    }
    (!v.is_empty()).then_some(v)
}

/// Unquoted value with a trailing comment (D61): what comes before the first space or
/// tab followed by `#`, trimmed.
fn before_comment(v: &[u8]) -> Option<&[u8]> {
    let k = (1..v.len()).find(|&k| v[k] == b'#' && matches!(v[k - 1], b' ' | b'\t'))?;
    let c = v[..k].trim_ascii_end();
    (!c.is_empty()).then_some(c)
}

/// Quoted value followed by spaces and `# …` (D61): the core of the quotes, up to the
/// first quote equal to the opening one followed by spaces or tabs and, after them, `#`.
fn core_before_comment(v: &[u8]) -> Option<&[u8]> {
    let q = *v.first()?;
    let k = (1..v.len()).find(|&k| {
        v[k] == q
            && matches!(v.get(k + 1), Some(b' ' | b'\t'))
            && v.get(skip_json_blank(v, k + 1)) == Some(&b'#')
    })?;
    Some(&v[1..k])
}

/// Space and tab between the elements of a JSON pair in the line.
fn skip_json_blank(l: &[u8], mut i: usize) -> usize {
    while matches!(l.get(i), Some(b' ' | b'\t')) {
        i += 1;
    }
    i
}

/// End (after the closing quote) of the JSON string that starts at the quote at
/// `start`. `\` and the next byte go together. `None` if the string does not close in
/// the line.
fn json_string_end(l: &[u8], start: usize) -> Option<usize> {
    let mut p = start + 1;
    while p < l.len() {
        match l[p] {
            b'\\' => p += 2,
            b'"' => return Some(p + 1),
            _ => p += 1,
        }
    }
    None
}

fn hex4_json(b: &[u8], i: usize) -> Option<u32> {
    b.get(i..i + 4)?
        .iter()
        .try_fold(0u32, |n, c| Some((n << 4) | u32::from(hex(*c)?)))
}

/// Decodes the core of a JSON string (D61): `\"`, `\\`, `\/`, `\b\f\n\r\t` and
/// `\uXXXX` (with surrogate pair). Invalid escape or lone surrogate: `false` (the pair
/// is discarded). The other bytes pass unchanged.
fn decode_json_string(raw: &[u8], out: &mut Zeroizing<Vec<u8>>) -> bool {
    reserve(out, raw.len());
    let mut utf8 = [0u8; 4];
    let mut k = 0usize;
    let mut ok = true;
    while k < raw.len() {
        let b = raw[k];
        if b != b'\\' {
            out.push(b);
            k += 1;
            continue;
        }
        let short = match raw.get(k + 1) {
            Some(b'"') => b'"',
            Some(b'\\') => b'\\',
            Some(b'/') => b'/',
            Some(b'b') => 0x08,
            Some(b'f') => 0x0c,
            Some(b'n') => b'\n',
            Some(b'r') => b'\r',
            Some(b't') => b'\t',
            Some(b'u') => {
                let n = match hex4_json(raw, k + 2) {
                    Some(high @ 0xd800..=0xdbff) => {
                        let low = if raw.get(k + 6..k + 8) == Some(b"\\u") {
                            hex4_json(raw, k + 8)
                        } else {
                            None
                        };
                        match low {
                            Some(low @ 0xdc00..=0xdfff) => {
                                k += 12;
                                0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00)
                            }
                            _ => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    Some(n @ (0..=0xd7ff | 0xe000..=0xffff)) => {
                        k += 6;
                        n
                    }
                    _ => {
                        ok = false;
                        break;
                    }
                };
                let Some(c) = char::from_u32(n) else {
                    ok = false;
                    break;
                };
                out.extend_from_slice(c.encode_utf8(&mut utf8).as_bytes());
                continue;
            }
            _ => {
                ok = false;
                break;
            }
        };
        out.push(short);
        k += 2;
    }
    utf8.zeroize();
    ok
}

fn unquoted(v: &[u8]) -> Option<&[u8]> {
    let (&a, rest) = v.split_first()?;
    let (&z, middle) = rest.split_last()?;
    (a == z && matches!(a, b'"' | b'\'') && !middle.is_empty()).then_some(middle)
}

fn json_escapes(b: u8, slash: bool) -> bool {
    b < 0x20 || b == b'"' || b == b'\\' || (slash && b == b'/')
}

fn json_changes(s: &[u8], slash: bool) -> bool {
    s.iter().any(|b| json_escapes(*b, slash))
}

/// The value as it is inside a JSON string (`slash`: also `\/`, as in PHP).
fn json(s: &[u8], slash: bool, out: &mut Zeroizing<Vec<u8>>) {
    reserve(out, s.len() * 6);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for &b in s {
        if !json_escapes(b, slash) {
            out.push(b);
            continue;
        }
        out.push(b'\\');
        match b {
            b'"' | b'\\' | b'/' => out.push(b),
            b'\n' => out.push(b'n'),
            b'\r' => out.push(b'r'),
            b'\t' => out.push(b't'),
            0x08 => out.push(b'b'),
            0x0c => out.push(b'f'),
            _ => {
                out.extend_from_slice(b"u00");
                out.push(HEX[usize::from(b >> 4)]);
                out.push(HEX[usize::from(b & 15)]);
            }
        }
    }
}

/// The JSON forms of D37, besides the standard escape of [`json`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum EscapedJson {
    /// Go's `encoding/json`: `&`, `<`, `>`, U+2028 and U+2029 also as `\uXXXX`.
    Go,
    /// Python's `json.dumps` with `ensure_ascii` (the default): every character outside
    /// visible ASCII and space as `\uXXXX` (surrogate pair above U+FFFF).
    Ascii,
    /// PHP's `json_encode` (the default): `/` as `\/` and every character outside ASCII
    /// as `\uXXXX` (D54). Only when the value has both; with only one, the JSON with `\/`
    /// or the `ensure_ascii` already cover it.
    Php,
    /// "html-safe" JSON with `ensure_ascii` (D58): the `ensure_ascii` plus `&`, `<` and
    /// `>` as `\uXXXX`. Only when the value has `&`, `<` or `>` and a character outside
    /// ASCII; with only one, Go or `ensure_ascii` already cover it (U+2028 and U+2029
    /// already come out as `\u` in `ensure_ascii`, so alone they change nothing here).
    GoAscii,
}

/// The character goes as `\uXXXX` in this form (besides the short escapes of standard
/// JSON).
fn escaped_json_u(c: char, mode: EscapedJson) -> bool {
    match mode {
        EscapedJson::Go => matches!(c, '&' | '<' | '>' | '\u{2028}' | '\u{2029}'),
        EscapedJson::Ascii => !c.is_ascii() || c == '\u{7f}',
        EscapedJson::Php => !c.is_ascii(),
        EscapedJson::GoAscii => !c.is_ascii() || matches!(c, '\u{7f}' | '&' | '<' | '>'),
    }
}

/// The form differs from the standard escape (which is already its own pattern when it
/// changes something).
fn escaped_json_changes(s: &str, mode: EscapedJson) -> bool {
    match mode {
        EscapedJson::Php => s.contains('/') && !s.is_ascii(),
        EscapedJson::GoAscii => s.contains(['&', '<', '>']) && !s.is_ascii(),
        _ => s.chars().any(|c| escaped_json_u(c, mode)),
    }
}

/// The value as Go or Python write it inside a JSON string. Lower-case hex, as both do
/// (upper-case hex stays in RA3).
fn escaped_json(s: &str, mode: EscapedJson, out: &mut Zeroizing<Vec<u8>>) {
    // Worst case: a control byte becomes 6; a 4-byte character becomes 12.
    reserve(out, s.len() * 6);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let u = |out: &mut Zeroizing<Vec<u8>>, n: u16| {
        out.extend_from_slice(b"\\u");
        for offset in [12, 8, 4, 0] {
            out.push(HEX[usize::from((n >> offset) & 15)]);
        }
    };
    // The pieces of the character go through these two stack buffers, zeroed at the end.
    let mut utf16 = [0u16; 2];
    let mut utf8 = [0u8; 4];
    for c in s.chars() {
        if escaped_json_u(c, mode) {
            for n in c.encode_utf16(&mut utf16).iter() {
                u(out, *n);
            }
            continue;
        }
        let bytes = c.encode_utf8(&mut utf8).as_bytes();
        let slash = mode == EscapedJson::Php;
        if bytes.len() == 1 && json_escapes(bytes[0], slash) {
            json(bytes, slash, out);
        } else {
            out.extend_from_slice(bytes);
        }
    }
    utf16.zeroize();
    utf8.zeroize();
}

/// Needs a `\` before it inside shell double quotes (`declare -p`, `export -p`).
fn double_quotes_escapes(b: u8) -> bool {
    matches!(b, b'$' | b'`' | b'"' | b'\\')
}

/// The value as bash writes it inside double quotes.
fn double_quotes(s: &[u8], out: &mut Zeroizing<Vec<u8>>) {
    reserve(out, s.len() * 2);
    for &b in s {
        if double_quotes_escapes(b) {
            out.push(b'\\');
        }
        out.push(b);
    }
}

/// The value as bash writes it inside single quotes (`${X@Q}`, `set`, `set -x`): each
/// `'` becomes `'\''` (D54). The quotes at the ends stay out of the pattern.
fn single_quotes(s: &[u8], out: &mut Zeroizing<Vec<u8>>) {
    reserve(out, s.len() * 4);
    for &b in s {
        if b == b'\'' {
            out.extend_from_slice(b"'\\''");
        } else {
            out.push(b);
        }
    }
}

/// bash 5.2's `printf %q` puts a `\` before this character at position `i` of `s` (D54,
/// checked against the real bash in `v2_56_printf_q_equal_to_bash`): the shell specials
/// always; `#` only at the start; `~` at the start or after `=` or `:`.
fn printf_q_escapes(s: &[u8], i: usize) -> bool {
    match s[i] {
        b' ' | b'!' | b'"' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b',' | b';' | b'<'
        | b'>' | b'?' | b'[' | b'\\' | b']' | b'^' | b'`' | b'{' | b'|' | b'}' => true,
        b'#' => i == 0,
        b'~' => i == 0 || matches!(s[i - 1], b'=' | b':'),
        _ => false,
    }
}

/// `printf %q` only applies here to a value in visible ASCII and space: with a control
/// or outside ASCII bash switches to `$'…'` (RA3).
fn printf_q_changes(s: &[u8]) -> bool {
    s.iter().all(|b| (0x20..=0x7e).contains(b)) && (0..s.len()).any(|i| printf_q_escapes(s, i))
}

/// The value as bash's `printf %q` writes it.
fn printf_q(s: &[u8], out: &mut Zeroizing<Vec<u8>>) {
    reserve(out, s.len() * 2);
    for (i, &b) in s.iter().enumerate() {
        if printf_q_escapes(s, i) {
            out.push(b'\\');
        }
        out.push(b);
    }
}

// ---------------------------------------------------------------------------------
// Stream
// ---------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// The bytes as they came.
    Raw,
    /// `%XX` (any case) decoded.
    Percent,
    /// `%XX` and `+` → space (form).
    Form,
}

/// `%` (and maybe a hex digit) waiting for the rest.
#[derive(Clone, Copy)]
enum Escape {
    Nothing,
    Percent { start: u64 },
    Hex { start: u64, high: u8, high_char: u8 },
}

/// One reading of the output: decodes and walks the automaton.
struct View {
    mode: Mode,
    state: u32,
    /// Original position of the start of each decoded byte still in a match (the last
    /// `depth` positions). Not secret: only offsets.
    positions: VecDeque<u64>,
    escape: Escape,
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

impl View {
    fn new(mode: Mode) -> Self {
        View {
            mode,
            state: 0,
            positions: VecDeque::new(),
            escape: Escape::Nothing,
        }
    }

    /// Walks one decoded byte `[start, end)` and returns the covered original excerpt.
    fn walk(&mut self, a: &Automaton, b: u8, start: u64, end: u64) -> Option<(u64, u64)> {
        self.state = a.step(self.state, b);
        self.positions.push_back(start);
        let depth = a.depth(self.state);
        while self.positions.len() > depth {
            self.positions.pop_front();
        }
        let n = a.output(self.state);
        if n == 0 {
            return None;
        }
        let begin = self.positions[self.positions.len() - n];
        Some((begin, end))
    }

    /// Reads the original byte `b` at position `p`.
    fn read(&mut self, a: &Automaton, b: u8, p: u64, cover: &mut dyn FnMut(u64, u64)) {
        let mut walk = |v: &mut View, byte: u8, start: u64, end: u64| {
            if let Some((i, f)) = v.walk(a, byte, start, end) {
                cover(i, f);
            }
        };
        if self.mode == Mode::Raw {
            walk(self, b, p, p + 1);
            return;
        }
        match self.escape {
            Escape::Percent { start } => {
                if let Some(high) = hex(b) {
                    self.escape = Escape::Hex {
                        start,
                        high,
                        high_char: b,
                    };
                    return;
                }
                self.escape = Escape::Nothing;
                walk(self, b'%', start, start + 1);
            }
            Escape::Hex {
                start,
                high,
                high_char,
            } => {
                self.escape = Escape::Nothing;
                if let Some(low) = hex(b) {
                    walk(self, (high << 4) | low, start, p + 1);
                    return;
                }
                walk(self, b'%', start, start + 1);
                walk(self, high_char, start + 1, start + 2);
            }
            Escape::Nothing => {}
        }
        // Normal byte (or what came after an escape that did not close).
        if b == b'%' {
            self.escape = Escape::Percent { start: p };
        } else if b == b'+' && self.mode == Mode::Form {
            walk(self, b' ', p, p + 1);
        } else {
            walk(self, b, p, p + 1);
        }
    }

    /// End of the output: a half escape becomes common bytes.
    fn close(&mut self, a: &Automaton, cover: &mut dyn FnMut(u64, u64)) {
        let escape = core::mem::replace(&mut self.escape, Escape::Nothing);
        let mut walk = |v: &mut View, byte: u8, start: u64, end: u64| {
            if let Some((i, f)) = v.walk(a, byte, start, end) {
                cover(i, f);
            }
        };
        match escape {
            Escape::Nothing => {}
            Escape::Percent { start } => walk(self, b'%', start, start + 1),
            Escape::Hex {
                start, high_char, ..
            } => {
                walk(self, b'%', start, start + 1);
                walk(self, high_char, start + 1, start + 2);
            }
        }
    }
}

/// Joins `[start, end)` to the sorted list of covered excerpts (disjoint and not
/// touching).
fn cover(covered: &mut Vec<(u64, u64)>, start: u64, end: u64) {
    let mut i = covered.len();
    while i > 0 && covered[i - 1].1 >= start {
        i -= 1;
    }
    let mut j = i;
    while j < covered.len() && covered[j].0 <= end {
        j += 1;
    }
    let (mut a, mut b) = (start, end);
    if j > i {
        a = a.min(covered[i].0);
        b = b.max(covered[j - 1].1);
    }
    covered.splice(i..j, core::iter::once((a, b)));
}

/// The mask of an output stream (stdout or stderr). No `Clone`; zeroed on drop.
pub struct Stream<'m> {
    mask: &'m Mask,
    /// Original bytes from `emitted` to `consumed`: only the chunk being worked on
    /// (emptied at the end of each [`Stream::mask`]).
    pending: Zeroizing<Vec<u8>>,
    emitted: u64,
    consumed: u64,
    views: [View; 3],
    covered: Vec<(u64, u64)>,
    /// The last byte returned was covered (an excerpt that continues does not repeat the
    /// mark).
    in_mark: bool,
}

impl ZeroizeOnDrop for Stream<'_> {}

impl fmt::Debug for Stream<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Stream(••••)")
    }
}

impl Stream<'_> {
    /// Receives one more chunk of the output and appends to `out` what is already safe
    /// to return, masked.
    pub fn mask(&mut self, chunk: &[u8], out: &mut Vec<u8>) {
        reserve(&mut self.pending, chunk.len());
        self.pending.extend_from_slice(chunk);
        let a = &self.mask.automaton;
        let covered = &mut self.covered;
        let mut covers = |i: u64, f: u64| cover(covered, i, f);
        for &b in chunk {
            for v in &mut self.views {
                v.read(a, b, self.consumed, &mut covers);
            }
            self.consumed += 1;
        }
        // D34 revised: nothing is held; the whole chunk goes out now.
        let consumed = self.consumed;
        self.emit_until(consumed, out);
    }

    /// End of the output. Nothing was held (D34 revised): a half `%X` at the end already
    /// went out as it came; here it only closes the views and zeroes the stream.
    pub fn finish(mut self, out: &mut Vec<u8>) {
        let a = &self.mask.automaton;
        let covered = &mut self.covered;
        let mut covers = |i: u64, f: u64| cover(covered, i, f);
        for v in &mut self.views {
            v.close(a, &mut covers);
        }
        let consumed = self.consumed;
        self.emit_until(consumed, out);
    }

    fn emit_until(&mut self, safe: u64, out: &mut Vec<u8>) {
        debug_assert!(safe >= self.emitted && safe <= self.consumed);
        let safe = safe.clamp(self.emitted, self.consumed);
        let base = self.emitted;
        let mut cursor = base;
        let pull = |from: u64, to: u64, out: &mut Vec<u8>, pending: &[u8]| {
            let (from, to) = ((from - base) as usize, (to - base) as usize);
            out.extend_from_slice(&pending[from..to]);
        };
        let mut used = 0;
        for &(start, end) in &self.covered {
            if start >= safe {
                break;
            }
            // An excerpt that started in a previous chunk (secret split between chunks):
            // what already went out stays as it went; from here on, mark (D34 revised).
            let start = start.max(cursor);
            if start > cursor {
                pull(cursor, start, out, &self.pending);
                self.in_mark = false;
            }
            let end_here = end.min(safe);
            if end_here > start {
                if !self.in_mark {
                    out.extend_from_slice(MARK.as_bytes());
                    self.in_mark = true;
                }
                cursor = end_here;
            }
            if end <= safe {
                used += 1;
            } else {
                break;
            }
        }
        if safe > cursor {
            pull(cursor, safe, out, &self.pending);
            self.in_mark = false;
        }
        self.covered.drain(..used);
        // Removes from the front what went out and zeroes the leftover at the end of the
        // used area.
        let n = (safe - base) as usize;
        let total = self.pending.len();
        self.pending.copy_within(n.., 0);
        self.pending[total - n..].zeroize();
        self.pending.truncate(total - n);
        self.emitted = safe;
    }
}
