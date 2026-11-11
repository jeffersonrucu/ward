//! Strict JSON of the protocol: own reader and writer, no new dependency (S14).
//!
//! The accepted subset is what the protocol uses, and nothing more:
//!
//! - the top is always an object;
//! - text in valid UTF-8, without a raw control character, with the JSON escapes
//!   (`\uXXXX` with a surrogate pair; a lone surrogate is refused);
//! - a number is only an unsigned integer, without a leading zero, without a fraction or
//!   an exponent, up to `u64::MAX` (negative is refused);
//! - `true`, `false`, `null`, lists and objects;
//! - a repeated key in the same object is refused, but by [`super::fields::Fields`], when
//!   reading the object, so that at the top this happens after the role check (two
//!   values for the same field
//!   would allow reading one and checking another);
//! - depth up to [`MAX_DEPTH`], up to [`MAX_NODES`] values in the frame and up to
//!   [`MAX_KEYS`] keys per object, so that a small frame does not become a lot of memory.
//!
//! Secret (S8): every text read goes into a `Zeroizing<String>` with the exact capacity
//! computed before (the decoded text is never larger than the raw one), so no
//! reallocation leaves a copy without zeroing. The writer computes the final size in a
//! pass that only counts and writes into a `Zeroizing<Vec<u8>>` with that capacity.
//!
//! Errors carry neither a position nor a piece of the frame: the frame may carry a
//! secret (S13).

use base64ct::{Base64, Encoding};
use zeroize::{Zeroize, Zeroizing};

/// Largest nesting depth (the top counts 1).
pub(crate) const MAX_DEPTH: usize = 8;
/// Largest number of values in a request (each text, number, item and object counts 1).
pub(crate) const MAX_NODES: usize = 20_000;
/// Largest number of values in a response: the list of a full vault (each item has up
/// to 9 values plus the tags and the app summary, D65 c).
pub(crate) const MAX_RESPONSE_NODES: usize = crate::limits::MAX_ENTRIES
    * (9 + crate::limits::MAX_TAGS + super::response::MAX_SUMMARY_NODES)
    + 1024;
/// Largest number of keys in an object.
pub(crate) const MAX_KEYS: usize = 32;

/// Refused JSON. No detail on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InvalidJson;

/// Value read. The text zeroes on drop.
pub(crate) enum Value {
    Null,
    Bool(bool),
    Integer(u64),
    Text(Zeroizing<String>),
    List(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl core::fmt::Debug for Value {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Never the content: the text may be a secret (S13).
        f.write_str(match self {
            Value::Null => "Null",
            Value::Bool(_) => "Bool",
            Value::Integer(_) => "Integer",
            Value::Text(_) => "Text(••••)",
            Value::List(_) => "List",
            Value::Object(_) => "Object",
        })
    }
}

/// Reads a whole frame, with up to `max_nodes` values. The top has to be an object, and
/// after it only blank space.
pub(crate) fn read(bytes: &[u8], max_nodes: usize) -> Result<Value, InvalidJson> {
    let text = core::str::from_utf8(bytes).map_err(|_| InvalidJson)?;
    let mut l = Reader {
        b: text.as_bytes(),
        i: 0,
        nodes: 0,
        max_nodes,
    };
    l.blank();
    if l.peek() != Some(b'{') {
        return Err(InvalidJson);
    }
    let v = l.value(1)?;
    l.blank();
    if l.i != l.b.len() {
        return Err(InvalidJson);
    }
    Ok(v)
}

struct Reader<'a> {
    /// UTF-8 already checked.
    b: &'a [u8],
    i: usize,
    nodes: usize,
    max_nodes: usize,
}

impl Reader<'_> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn blank(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.peek() {
            self.i += 1;
        }
    }

    fn expect(&mut self, c: u8) -> Result<(), InvalidJson> {
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(InvalidJson)
        }
    }

    fn literal(&mut self, word: &[u8]) -> Result<(), InvalidJson> {
        if self.b[self.i..].starts_with(word) {
            self.i += word.len();
            Ok(())
        } else {
            Err(InvalidJson)
        }
    }

    fn count_node(&mut self) -> Result<(), InvalidJson> {
        self.nodes += 1;
        if self.nodes > self.max_nodes {
            Err(InvalidJson)
        } else {
            Ok(())
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, InvalidJson> {
        if depth > MAX_DEPTH {
            return Err(InvalidJson);
        }
        self.count_node()?;
        match self.peek().ok_or(InvalidJson)? {
            b'{' => self.object(depth),
            b'[' => self.list(depth),
            b'"' => Ok(Value::Text(self.text()?)),
            b't' => self.literal(b"true").map(|()| Value::Bool(true)),
            b'f' => self.literal(b"false").map(|()| Value::Bool(false)),
            b'n' => self.literal(b"null").map(|()| Value::Null),
            b'0'..=b'9' => self.integer().map(Value::Integer),
            _ => Err(InvalidJson),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, InvalidJson> {
        self.expect(b'{')?;
        let mut pairs: Vec<(String, Value)> = Vec::new();
        self.blank();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Value::Object(pairs));
        }
        loop {
            self.blank();
            if self.peek() != Some(b'"') {
                return Err(InvalidJson);
            }
            let mut key = self.text()?;
            let key = core::mem::take(&mut *key);
            if pairs.len() >= MAX_KEYS {
                return Err(InvalidJson);
            }
            self.blank();
            self.expect(b':')?;
            self.blank();
            let v = self.value(depth + 1)?;
            pairs.push((key, v));
            self.blank();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Value::Object(pairs));
                }
                _ => return Err(InvalidJson),
            }
        }
    }

    fn list(&mut self, depth: usize) -> Result<Value, InvalidJson> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.blank();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Value::List(items));
        }
        loop {
            self.blank();
            items.push(self.value(depth + 1)?);
            self.blank();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Value::List(items));
                }
                _ => return Err(InvalidJson),
            }
        }
    }

    fn integer(&mut self) -> Result<u64, InvalidJson> {
        let start = self.i;
        let mut n: u64 = 0;
        while let Some(c @ b'0'..=b'9') = self.peek() {
            n = n
                .checked_mul(10)
                .and_then(|n| n.checked_add(u64::from(c - b'0')))
                .ok_or(InvalidJson)?;
            self.i += 1;
        }
        let digits = self.i - start;
        // Leading zero, fraction and exponent stay out.
        if digits > 1 && self.b[start] == b'0' {
            return Err(InvalidJson);
        }
        if let Some(b'.' | b'e' | b'E' | b'-' | b'+') = self.peek() {
            return Err(InvalidJson);
        }
        Ok(n)
    }

    /// Reads a quoted text. First finds the end (the raw one bounds the decoded one),
    /// then decodes into a buffer with that capacity, without reallocating.
    fn text(&mut self) -> Result<Zeroizing<String>, InvalidJson> {
        self.expect(b'"')?;
        let start = self.i;
        let mut j = start;
        loop {
            match self.b.get(j).copied().ok_or(InvalidJson)? {
                b'"' => break,
                b'\\' => j += 2,
                c if c < 0x20 => return Err(InvalidJson),
                _ => j += 1,
            }
        }
        let end = j;
        let mut out = Zeroizing::new(String::with_capacity(end - start));
        let capacity = out.capacity();
        let mut k = start;
        while k < end {
            let c = self.b[k];
            if c != b'\\' {
                // Copies the stretch without escapes at once (the bounds fall on a
                // character boundary: `\\` and `"` are ASCII).
                let mut e = k;
                while e < end && self.b[e] != b'\\' {
                    e += 1;
                }
                let stretch = core::str::from_utf8(&self.b[k..e]).map_err(|_| InvalidJson)?;
                out.push_str(stretch);
                k = e;
                continue;
            }
            let esc = *self.b.get(k + 1).ok_or(InvalidJson)?;
            k += 2;
            let ch = match esc {
                b'"' => '"',
                b'\\' => '\\',
                b'/' => '/',
                b'b' => '\u{8}',
                b'f' => '\u{c}',
                b'n' => '\n',
                b'r' => '\r',
                b't' => '\t',
                b'u' => {
                    let high = hex4(self.b.get(k..k + 4).ok_or(InvalidJson)?)?;
                    k += 4;
                    let cp = if (0xD800..0xDC00).contains(&high) {
                        if self.b.get(k..k + 2) != Some(b"\\u") {
                            return Err(InvalidJson);
                        }
                        let low = hex4(self.b.get(k + 2..k + 6).ok_or(InvalidJson)?)?;
                        if !(0xDC00..0xE000).contains(&low) {
                            return Err(InvalidJson);
                        }
                        k += 6;
                        0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00)
                    } else if (0xDC00..0xE000).contains(&high) {
                        return Err(InvalidJson);
                    } else {
                        high
                    };
                    char::from_u32(cp).ok_or(InvalidJson)?
                }
                _ => return Err(InvalidJson),
            };
            out.push(ch);
        }
        debug_assert_eq!(out.capacity(), capacity, "o texto realocou");
        self.i = end + 1;
        Ok(out)
    }
}

fn hex4(b: &[u8]) -> Result<u32, InvalidJson> {
    let mut n = 0u32;
    for &c in b {
        let d = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => return Err(InvalidJson),
        };
        n = n * 16 + u32::from(d);
    }
    Ok(n)
}

/// Decodes standard base64 (with `=`) into a buffer that zeroes on drop, with the exact
/// capacity. Refuses above `maximum` decoded bytes before allocating. `base64ct`
/// (RustCrypto) is constant-time and refuses a non-canonical form.
pub(crate) fn from_base64(text: &str, maximum: usize) -> Result<Zeroizing<Vec<u8>>, InvalidJson> {
    let n = text.len();
    if !n.is_multiple_of(4) {
        return Err(InvalidJson);
    }
    let limit = n / 4 * 3;
    if limit > maximum.saturating_add(2) {
        return Err(InvalidJson);
    }
    let mut buf = Zeroizing::new(vec![0u8; limit]);
    let size = Base64::decode(text.as_bytes(), &mut buf)
        .map_err(|_| InvalidJson)?
        .len();
    if size > maximum {
        return Err(InvalidJson);
    }
    // `truncate` does not reallocate; the leftover bytes zero along with the buffer.
    buf[size..].zeroize();
    buf.truncate(size);
    Ok(buf)
}

/// Size of the standard base64 of `n` bytes.
pub(crate) const fn base64_len(n: usize) -> usize {
    n.div_ceil(3) * 4
}

// ---- Writer ----

/// Writes JSON in two passes: [`encode`] calls the function once counting and once
/// writing, into a buffer with the exact capacity.
pub(crate) struct Writer {
    /// `None` in the pass that only counts.
    buf: Option<Zeroizing<Vec<u8>>>,
    size: usize,
    /// Per open level: no item written yet?
    first: Vec<bool>,
}

/// The result would go over the limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TooLarge;

/// Encodes with `write`, refusing above `limit` bytes before allocating.
pub(crate) fn encode(
    limit: usize,
    write: impl Fn(&mut Writer),
) -> Result<Zeroizing<Vec<u8>>, TooLarge> {
    let mut counter = Writer {
        buf: None,
        size: 0,
        first: Vec::new(),
    };
    write(&mut counter);
    let total = counter.size;
    if total > limit {
        return Err(TooLarge);
    }
    let mut e = Writer {
        buf: Some(Zeroizing::new(Vec::with_capacity(total))),
        size: 0,
        first: Vec::new(),
    };
    write(&mut e);
    let buf = e.buf.take().unwrap_or_default();
    // The two passes are the same function; if they diverge, it is a programming error.
    assert_eq!(buf.len(), total, "as passadas do escritor divergiram");
    assert_eq!(buf.capacity(), total, "o escritor realocou");
    Ok(buf)
}

impl Writer {
    fn raw(&mut self, b: &[u8]) {
        self.size += b.len();
        if let Some(buf) = self.buf.as_mut() {
            // Never grows: the capacity was computed by the pass that counts.
            if buf.capacity() - buf.len() >= b.len() {
                buf.extend_from_slice(b);
            }
        }
    }

    fn separator(&mut self) {
        if let Some(p) = self.first.last_mut() {
            if *p {
                *p = false;
            } else {
                self.raw(b",");
            }
        }
    }

    pub(crate) fn open_object(&mut self) {
        self.raw(b"{");
        self.first.push(true);
    }

    pub(crate) fn close_object(&mut self) {
        self.first.pop();
        self.raw(b"}");
    }

    pub(crate) fn open_list(&mut self) {
        self.raw(b"[");
        self.first.push(true);
    }

    pub(crate) fn close_list(&mut self) {
        self.first.pop();
        self.raw(b"]");
    }

    /// Object key (the keys are fixed by the protocol).
    pub(crate) fn key(&mut self, k: &str) {
        self.separator();
        self.raw_text(k);
        self.raw(b":");
        // The value that follows takes no comma.
        self.first.push(true);
    }

    /// Before each list item.
    pub(crate) fn item(&mut self) {
        self.separator();
        self.first.push(true);
    }

    /// After the value of a key or of an item.
    fn end_value(&mut self) {
        // The scalar and compound values below close the level of `key`/`item`.
        self.first.pop();
    }

    pub(crate) fn text(&mut self, s: &str) {
        self.raw_text(s);
        self.end_value();
    }

    pub(crate) fn integer(&mut self, n: u64) {
        let mut tmp = [0u8; 20];
        let mut i = tmp.len();
        let mut v = n;
        loop {
            i -= 1;
            tmp[i] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        self.raw(&tmp[i..]);
        self.end_value();
    }

    pub(crate) fn boolean(&mut self, b: bool) {
        self.raw(if b { b"true" } else { b"false" });
        self.end_value();
    }

    /// Bytes in standard base64. The piece on the stack zeroes at the end.
    pub(crate) fn base64(&mut self, data: &[u8]) {
        self.raw(b"\"");
        let n = base64_len(data.len());
        self.size += n;
        if let Some(buf) = self.buf.as_mut()
            && buf.capacity() - buf.len() >= n
        {
            let mut tmp = Zeroizing::new([0u8; 1024]);
            for piece in data.chunks(768) {
                if let Ok(s) = Base64::encode(piece, &mut tmp[..]) {
                    buf.extend_from_slice(s.as_bytes());
                }
            }
        }
        self.raw(b"\"");
        self.end_value();
    }

    /// Opens an object as the value of a key or an item.
    pub(crate) fn object(&mut self, f: impl FnOnce(&mut Writer)) {
        self.open_object();
        f(self);
        self.close_object();
        self.end_value();
    }

    /// Opens a list as the value of a key or an item.
    pub(crate) fn list(&mut self, f: impl FnOnce(&mut Writer)) {
        self.open_list();
        f(self);
        self.close_list();
        self.end_value();
    }

    fn raw_text(&mut self, s: &str) {
        self.raw(b"\"");
        let b = s.as_bytes();
        let mut start = 0;
        for (i, &c) in b.iter().enumerate() {
            let esc: Option<&[u8]> = match c {
                b'"' => Some(b"\\\""),
                b'\\' => Some(b"\\\\"),
                b'\n' => Some(b"\\n"),
                b'\r' => Some(b"\\r"),
                b'\t' => Some(b"\\t"),
                c if c < 0x20 => None,
                _ => continue,
            };
            self.raw(&b[start..i]);
            match esc {
                Some(e) => self.raw(e),
                None => {
                    const HEX: &[u8; 16] = b"0123456789abcdef";
                    self.raw(&[
                        b'\\',
                        b'u',
                        b'0',
                        b'0',
                        HEX[usize::from(c >> 4)],
                        HEX[usize::from(c & 0xf)],
                    ]);
                }
            }
            start = i + 1;
        }
        self.raw(&b[start..]);
        self.raw(b"\"");
    }
}
