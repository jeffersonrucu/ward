//! Minimal Chrome DevTools Protocol client over the debugging pipe (`ward playwright`,
//! Phase 44; D6, D9; S1–S3, S8, S13).
//!
//! - **Pipe only** (S3, D6): Chrome gets `--remote-debugging-pipe`, reads the commands
//!   on fd 3 and writes the responses on fd 4 ([`ward_platform::child::Command::launch_with_debugging`]).
//!   No TCP port and no socket: another process cannot find the browser to send it
//!   commands. The daemon's ends are `CLOEXEC`.
//! - **Messages**: JSON terminated by `\0`. Each message read lands in a `Zeroizing`
//!   buffer whose capacity is reserved once ([`MAX_MESSAGE`]): it never reallocates, and
//!   whatever went through it (the login POST body shows up in `Fetch.requestPaused`)
//!   is zeroed at the end. A message over the limit ends the session.
//! - **Reading**: only the fields in use are read (`serde` with our own structs); the
//!   rest is skipped by `serde_json` without a copy (`IgnoredAny`), so the POST body never
//!   becomes a `String`.
//! - **Writing**: the [`Writer`] builds each command in a `Zeroizing<Vec<u8>>` with the
//!   capacity computed beforehand; going past it is an error (it never reallocates with
//!   the password inside).
//! - **Main frame navigation** (D9): every document request of the main frame goes
//!   through `Fetch`; an origin other than the entry's is refused with
//!   `Fetch.failRequest` before it leaves the browser (the redirect to the phishing site
//!   never gets there, and the POST with the password only goes to the right origin).
//! - **Deadline**: every wait has the session deadline; once it passes, the session ends.
//! - Errors carry no text from the page or the browser (S13).

use std::fs::File;
use std::io::{self, Read, Write};
use std::time::Instant;

use serde::Deserialize;
use serde::de::IgnoredAny;
use zeroize::Zeroizing;

/// Largest message accepted from Chrome (the cookies response of a common site is much
/// smaller).
pub const MAX_MESSAGE: usize = 4 * 1024 * 1024;
/// How much is read from the pipe at a time.
const CHUNK: usize = 64 * 1024;

/// Why the conversation with the browser ended. No text from the browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CdpError {
    /// The session deadline passed.
    Deadline,
    /// The browser closed the pipe, crashed or let go of the tab.
    Closed,
    /// Message out of format or too large, or command too large.
    Protocol,
    /// The browser answered a command with an error.
    Command,
    /// Writing or reading failed.
    Io,
}

/// JSON writer for the commands: only what is needed, without reallocating.
pub struct Writer {
    b: Zeroizing<Vec<u8>>,
    /// A comma is needed before the next item.
    comma: bool,
    overflowed: bool,
}

impl Writer {
    /// Writer with a fixed capacity of `capacity` bytes.
    pub fn new(capacity: usize) -> Self {
        Writer {
            b: Zeroizing::new(Vec::with_capacity(capacity)),
            comma: false,
            overflowed: false,
        }
    }

    fn put(&mut self, s: &[u8]) {
        if self.overflowed || self.b.len() + s.len() > self.b.capacity() {
            self.overflowed = true;
            return;
        }
        self.b.extend_from_slice(s);
    }

    fn separate(&mut self) {
        if self.comma {
            self.put(b",");
        }
    }

    pub fn open_object(&mut self) {
        self.separate();
        self.put(b"{");
        self.comma = false;
    }

    pub fn close_object(&mut self) {
        self.put(b"}");
        self.comma = true;
    }

    pub fn open_list(&mut self) {
        self.separate();
        self.put(b"[");
        self.comma = false;
    }

    pub fn close_list(&mut self) {
        self.put(b"]");
        self.comma = true;
    }

    pub fn key(&mut self, k: &str) {
        self.separate();
        self.raw_text(k);
        self.put(b":");
        self.comma = false;
    }

    /// A text with JSON escaping (quotes, backslash and control characters).
    pub fn text(&mut self, s: &str) {
        self.separate();
        self.raw_text(s);
        self.comma = true;
    }

    fn raw_text(&mut self, s: &str) {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        self.put(b"\"");
        for &c in s.as_bytes() {
            match c {
                b'"' => self.put(b"\\\""),
                b'\\' => self.put(b"\\\\"),
                c if c < 0x20 => {
                    self.put(&[
                        b'\\',
                        b'u',
                        b'0',
                        b'0',
                        HEX[usize::from(c >> 4)],
                        HEX[usize::from(c & 0xf)],
                    ]);
                }
                c => self.put(&[c]),
            }
        }
        self.put(b"\"");
    }

    pub fn integer(&mut self, n: u64) {
        self.separate();
        let mut buf = [0u8; 20];
        let mut i = buf.len();
        let mut n = n;
        loop {
            i -= 1;
            buf[i] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        self.put(&buf[i..]);
        self.comma = true;
    }

    pub fn boolean(&mut self, v: bool) {
        self.separate();
        self.put(if v { b"true" } else { b"false" });
        self.comma = true;
    }

    pub fn null(&mut self) {
        self.separate();
        self.put(b"null");
        self.comma = true;
    }

    /// The bytes, or `None` if the capacity was exceeded.
    pub fn finish(self) -> Option<Zeroizing<Vec<u8>>> {
        (!self.overflowed).then_some(self.b)
    }
}

/// Size of the text `s` after escaping, in the worst case (to reserve the capacity).
pub fn worst_case(s: &str) -> usize {
    s.len() * 6 + 2
}

// --- Reading ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct Envelope {
    id: Option<u64>,
    method: Option<String>,
    #[serde(rename = "sessionId")]
    session: Option<String>,
    error: Option<IgnoredAny>,
}

#[derive(Deserialize)]
struct WithResult<T> {
    result: T,
}

#[derive(Deserialize)]
struct WithParams<T> {
    params: T,
}

#[derive(Deserialize)]
struct Pause {
    #[serde(rename = "requestId")]
    request_id: String,
    request: PausedRequest,
    #[serde(rename = "resourceType")]
    resource_type: String,
    #[serde(rename = "frameId")]
    frame_id: String,
}

#[derive(Deserialize)]
struct PausedRequest {
    url: String,
}

/// Navigation rule of the main frame (D9).
#[derive(Debug, Clone)]
pub struct Rule {
    /// The entry's origin (`scheme://host[:port]`).
    pub origin: String,
    /// The id of the tab's main frame.
    pub frame: String,
}

/// The conversation with a Chrome over the pipe.
pub struct Cdp {
    write_end: File,
    read_end: File,
    buf: Zeroizing<Vec<u8>>,
    next_id: u64,
    deadline: Instant,
    /// Session of the tab (`Target.attachToTarget` with `flatten`).
    pub session: Option<String>,
    /// Rule for the main frame documents; without it, every request goes on.
    pub rule: Option<Rule>,
    /// Some main frame document was refused because of its origin.
    pub blocked: bool,
    /// How many `Page.loadEventFired` of the tab arrived.
    pub loads: u64,
}

impl std::fmt::Debug for Cdp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cdp({} loads, ••••)", self.loads)
    }
}

impl Cdp {
    /// `write_end` goes to Chrome's fd 3; `read_end` comes from fd 4.
    pub fn new(write_end: File, read_end: File, deadline: Instant) -> Self {
        Cdp {
            write_end,
            read_end,
            buf: Zeroizing::new(Vec::with_capacity(MAX_MESSAGE + CHUNK)),
            next_id: 1,
            deadline,
            session: None,
            rule: None,
            blocked: false,
            loads: 0,
        }
    }

    /// Sends a command with the parameters already in JSON (an object) and returns the
    /// id. `in_tab`: with the tab's `sessionId`.
    pub fn send(&mut self, method: &str, params: &[u8], in_tab: bool) -> Result<u64, CdpError> {
        let id = self.next_id;
        self.next_id += 1;
        let session = if in_tab {
            Some(self.session.clone().ok_or(CdpError::Protocol)?)
        } else {
            None
        };
        let cap = 96 + method.len() + params.len() + session.as_ref().map_or(0, |s| worst_case(s));
        let mut e = Writer::new(cap);
        e.open_object();
        e.key("id");
        e.integer(id);
        e.key("method");
        e.text(method);
        if let Some(s) = &session {
            e.key("sessionId");
            e.text(s);
        }
        e.key("params");
        e.comma = false;
        e.put(params);
        e.comma = true;
        e.close_object();
        e.put(b"\0");
        let msg = e.finish().ok_or(CdpError::Protocol)?;
        self.write_end.write_all(&msg).map_err(|_| CdpError::Io)?;
        Ok(id)
    }

    /// Sends and waits for the response (the whole message, to read the `result`).
    pub fn command(
        &mut self,
        method: &str,
        params: &[u8],
        in_tab: bool,
    ) -> Result<Zeroizing<Vec<u8>>, CdpError> {
        let id = self.send(method, params, in_tab)?;
        loop {
            let msg = self.next_message()?;
            if let Some(r) = self.handle(&msg, Some(id))? {
                return Ok(r);
            }
        }
    }

    /// Like [`Cdp::command`], already reading the `result` as `T`.
    pub fn command_as<T: for<'de> Deserialize<'de>>(
        &mut self,
        method: &str,
        params: &[u8],
        in_tab: bool,
    ) -> Result<T, CdpError> {
        let msg = self.command(method, params, in_tab)?;
        serde_json::from_slice::<WithResult<T>>(&msg)
            .map(|r| r.result)
            .map_err(|_| CdpError::Protocol)
    }

    /// Waits until one more `Page.loadEventFired` of the tab arrives after `before`, or
    /// until the deadline `until` (the smaller of the two deadlines). `Ok(false)`:
    /// `until` passed.
    pub fn wait_load(&mut self, before: u64, until: Instant) -> Result<bool, CdpError> {
        while self.loads <= before {
            if Instant::now() >= until {
                return Ok(false);
            }
            match self.next_message_until(until.min(self.deadline)) {
                Ok(msg) => {
                    self.handle(&msg, None)?;
                }
                Err(CdpError::Deadline) if Instant::now() < self.deadline => return Ok(false),
                Err(e) => return Err(e),
            }
        }
        Ok(true)
    }

    /// Handles a message: the expected response comes back in `Some`; events are
    /// handled.
    fn handle(
        &mut self,
        msg: &Zeroizing<Vec<u8>>,
        expected: Option<u64>,
    ) -> Result<Option<Zeroizing<Vec<u8>>>, CdpError> {
        let env: Envelope = serde_json::from_slice(msg).map_err(|_| CdpError::Protocol)?;
        if let Some(id) = env.id {
            if Some(id) == expected {
                if env.error.is_some() {
                    return Err(CdpError::Command);
                }
                return Ok(Some(msg.clone()));
            }
            // Response to a `continueRequest`/`failRequest` already sent: nothing to do.
            return Ok(None);
        }
        let Some(method) = env.method.as_deref() else {
            return Err(CdpError::Protocol);
        };
        let in_tab = env.session.is_some() && env.session == self.session;
        match method {
            "Fetch.requestPaused" if in_tab => self.decide(msg)?,
            "Page.loadEventFired" if in_tab => self.loads += 1,
            "Page.javascriptDialogOpening" if in_tab => {
                // An `alert`/`confirm` would stall the page: close it without accepting.
                self.send("Page.handleJavaScriptDialog", b"{\"accept\":false}", true)?;
            }
            "Target.detachedFromTarget" | "Inspector.detached" | "Target.targetCrashed" => {
                return Err(CdpError::Closed);
            }
            _ => {}
        }
        Ok(None)
    }

    /// D9: main frame documents only from the entry's origin; the rest goes on.
    fn decide(&mut self, msg: &[u8]) -> Result<(), CdpError> {
        let p: WithParams<Pause> = serde_json::from_slice(msg).map_err(|_| CdpError::Protocol)?;
        let p = p.params;
        let url = Zeroizing::new(p.request.url);
        let allowed = match &self.rule {
            Some(r) if p.resource_type == "Document" && p.frame_id == r.frame => {
                ward_core::entry::url_origin(&url).is_some_and(|o| o == r.origin)
            }
            _ => true,
        };
        let mut e = Writer::new(64 + worst_case(&p.request_id));
        e.open_object();
        e.key("requestId");
        e.text(&p.request_id);
        if !allowed {
            e.key("errorReason");
            e.text("BlockedByClient");
        }
        e.close_object();
        let params = e.finish().ok_or(CdpError::Protocol)?;
        if allowed {
            self.send("Fetch.continueRequest", &params, true)?;
        } else {
            self.blocked = true;
            crate::aviso!("playwright: navegação para outra origem recusada (D9)");
            self.send("Fetch.failRequest", &params, true)?;
        }
        Ok(())
    }

    fn next_message(&mut self) -> Result<Zeroizing<Vec<u8>>, CdpError> {
        self.next_message_until(self.deadline)
    }

    /// The next whole message (without the `\0`), in a `Zeroizing` buffer of the exact
    /// size.
    fn next_message_until(&mut self, until: Instant) -> Result<Zeroizing<Vec<u8>>, CdpError> {
        loop {
            if let Some(end) = self.buf.iter().position(|&b| b == 0) {
                let mut msg = Zeroizing::new(Vec::with_capacity(end));
                msg.extend_from_slice(&self.buf[..end]);
                // `drain` moves the rest to the start of the same buffer (without
                // reallocating); the `Zeroizing` zeroes the whole capacity at the end.
                self.buf.drain(..=end);
                return Ok(msg);
            }
            if self.buf.len() >= MAX_MESSAGE {
                return Err(CdpError::Protocol);
            }
            let now = Instant::now();
            if now >= until {
                return Err(CdpError::Deadline);
            }
            let ms =
                i32::try_from(until.duration_since(now).as_millis().max(1)).unwrap_or(i32::MAX);
            match ward_platform::child::ready_to_read(&self.read_end, ms) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(CdpError::Io),
            }
            let before = self.buf.len();
            // Within the reserved capacity: the `resize` does not reallocate.
            self.buf.resize(before + CHUNK, 0);
            match self.read_end.read(&mut self.buf[before..]) {
                Ok(0) => {
                    self.buf.truncate(before);
                    return Err(CdpError::Closed);
                }
                Ok(n) => self.buf.truncate(before + n),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => self.buf.truncate(before),
                Err(_) => {
                    self.buf.truncate(before);
                    return Err(CdpError::Io);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writer_escapes_and_does_not_reallocate() {
        let mut e = Writer::new(256);
        e.open_object();
        e.key("a");
        e.text("x\"y\\z\n\u{1}é");
        e.key("b");
        e.open_list();
        e.integer(0);
        e.integer(18_446_744_073_709_551_615);
        e.boolean(true);
        e.null();
        e.close_list();
        e.close_object();
        let b = e.finish().unwrap();
        let v: serde_json::Value = serde_json::from_slice(&b).unwrap();
        assert_eq!(v["a"], "x\"y\\z\n\u{1}é");
        assert_eq!(v["b"][1], 18_446_744_073_709_551_615u64);
        assert_eq!(v["b"][2], true);
        assert!(v["b"][3].is_null());
        // Attack: a text that exceeds the capacity never reallocates (nothing half done).
        let mut e = Writer::new(8);
        e.text("falso-WP-senha-02");
        assert!(e.finish().is_none());
    }

    /// The POST body in `Fetch.requestPaused` is skipped without becoming a `String`:
    /// the reading only takes the fields in use.
    #[test]
    fn pause_reads_only_url_type_and_frame() {
        let msg = br#"{"method":"Fetch.requestPaused","sessionId":"S","params":{"requestId":"r1","request":{"url":"http://localhost:8081/wp-login.php","method":"POST","postData":"log=wp_editor&pwd=falso-WP-senha-02","headers":{}},"frameId":"F","resourceType":"Document","extra":[1,2.5,-3e2]}}"#;
        let p: WithParams<Pause> = serde_json::from_slice(msg).unwrap();
        assert_eq!(p.params.request_id, "r1");
        assert_eq!(p.params.resource_type, "Document");
        assert_eq!(p.params.frame_id, "F");
        assert_eq!(p.params.request.url, "http://localhost:8081/wp-login.php");
    }
}
