//! Native messaging host of the Chrome extension (Phase 40; D5, D9, D47; S1, S8, S13).
//!
//! Chrome executes the installed `ward-nm` (root:`ward-nm` 2755, D47) as a direct child
//! of the browser process, with a single argument, the origin of the extension
//! (`chrome-extension://<id>/`), and talks to it over stdin/stdout in native messaging
//! messages: 4 bytes of size in native order and the JSON body in UTF-8.
//!
//! Each message of the extension is a request of the daemon protocol
//! (`{"v":1,"op":"…",…}`), parsed with the same strict parsing as the daemon in the
//! extension role. Only the extension's operations pass ([`OPERATIONS`]); the request is
//! re-encoded (canonical form) and goes to `wardd` on a connection of its own, checked
//! before the first byte (D46). The daemon decides the role, by the egid and by the
//! Chrome parent (D47): opened by any other process, `ward-nm` becomes an agent and the
//! daemon refuses (V6.1). The origin of the tab is compared in the daemon (D9), never
//! here.
//!
//! The daemon's response is parsed in the extension role (a response with a secret, a
//! token or the app's list does not decode), checked against the requested operation
//! ([`response_fits`]) and re-encoded for the extension. Nothing else goes out on stdout.
//!
//! Limits: a message from the extension up to [`MAX_MESSAGE_IN`]; larger than that,
//! empty or truncated, the host answers the error and exits (the stream lost its step).
//! Response to the extension up to [`MAX_MESSAGE_OUT`] (Chrome's limit). Messages and
//! frames with credentials stay in `Zeroizing`. The stderr (which Chrome joins to its
//! log) only carries fixed texts, never the content of a message (S13).

#![forbid(unsafe_code)]

use std::io::{self, Read, Write};

use zeroize::Zeroizing;

use ward_core::limits::KIB;
use ward_core::protocol::{self, Operation, Response, ResponseError, ResponseType, Role};

/// Largest message accepted from the extension. The largest legitimate request is a
/// login save with the URL, the username and the password (each up to 2 KiB, D25) in
/// base64.
pub const MAX_MESSAGE_IN: usize = 64 * KIB;
/// Largest message Chrome accepts from the host (1 MiB).
pub const MAX_MESSAGE_OUT: usize = 1024 * KIB;

/// Operations the host takes to the daemon: those of the extension role in the protocol
/// table. Explicit list: a new operation in the table does not pass without changing it
/// here (the test `operations_are_the_extensions` ties the two).
pub const OPERATIONS: [Operation; 8] = [
    Operation::Status,
    Operation::Suggest,
    Operation::Fill,
    Operation::SaveLogin,
    Operation::AwaitLogin,
    Operation::FillLogin,
    Operation::LoginResult,
    Operation::OpenApp,
];

/// Does the daemon's response fit the requested operation? An error fits all of them;
/// apart from that, each operation has a single type (a fill never answers a
/// suggestion request).
pub fn response_fits(op: Operation, kind: ResponseType) -> bool {
    kind == ResponseType::Error
        || matches!(
            (op, kind),
            (Operation::Status, ResponseType::Status)
                | (Operation::Suggest, ResponseType::Suggestions)
                | (Operation::Fill, ResponseType::Fill)
                | (Operation::SaveLogin, ResponseType::Ok)
                // `ward login` (Phase 43): `ok` is the wait that ended without a request.
                | (Operation::AwaitLogin, ResponseType::LoginRequest | ResponseType::Ok)
                | (Operation::FillLogin, ResponseType::Fill)
                | (Operation::LoginResult, ResponseType::Ok)
                | (Operation::OpenApp, ResponseType::Ok)
        )
}

/// Id of the Ward extension (Phase 41): it comes from the public key of the `key` field
/// of `extension/manifest.json` (sha-256 of the key in DER, first 32 hex digits in
/// `a`–`p`). `dev/extension/build.py` refuses to build the extension if the manifest's
/// id is not this one.
pub const EXTENSION_ID: &str = "nhkehjnmlgkebhmelajfpmpblepaichf";

/// The argument Chrome passes: `chrome-extension://<id>/`. Only Ward's id
/// ([`EXTENSION_ID`]) passes: another extension that has a native messaging manifest
/// pointing to `ward-nm` (in the user's profile, for instance) does not reach the
/// daemon. Anything else (another scheme, a path, another id) is refused.
pub fn valid_extension_origin(arg: &[u8]) -> bool {
    let Some(rest) = arg.strip_prefix(b"chrome-extension://") else {
        return false;
    };
    let Some(id) = rest.strip_suffix(b"/") else {
        return false;
    };
    id.len() == 32 && id.iter().all(|c| (b'a'..=b'p').contains(c)) && id == EXTENSION_ID.as_bytes()
}

/// Why reading a message from the extension stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageError {
    /// End of stdin before the first byte: Chrome closed (normal end).
    End,
    /// Truncated in the middle of the size or of the body.
    Truncated,
    /// Size 0.
    Empty,
    /// Above [`MAX_MESSAGE_IN`].
    TooLarge,
    /// I/O error.
    Io(io::ErrorKind),
}

/// Reads a native messaging message (size in native order). The body is read only after
/// checking the size; nothing beyond the declared size is read.
pub fn read_message(r: &mut impl Read) -> Result<Zeroizing<Vec<u8>>, MessageError> {
    let mut header = [0u8; 4];
    let mut read = 0;
    while read < header.len() {
        match r.read(&mut header[read..]) {
            Ok(0) if read == 0 => return Err(MessageError::End),
            Ok(0) => return Err(MessageError::Truncated),
            Ok(n) => read += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(MessageError::Io(e.kind())),
        }
    }
    let n = usize::try_from(u32::from_ne_bytes(header)).map_err(|_| MessageError::TooLarge)?;
    if n == 0 {
        return Err(MessageError::Empty);
    }
    if n > MAX_MESSAGE_IN {
        return Err(MessageError::TooLarge);
    }
    let mut body = Zeroizing::new(vec![0u8; n]);
    r.read_exact(&mut body).map_err(|e| match e.kind() {
        io::ErrorKind::UnexpectedEof => MessageError::Truncated,
        k => MessageError::Io(k),
    })?;
    Ok(body)
}

/// Writes a native messaging message. Above [`MAX_MESSAGE_OUT`] nothing goes out.
pub fn write_message(w: &mut impl Write, body: &[u8]) -> io::Result<()> {
    if body.is_empty() || body.len() > MAX_MESSAGE_OUT {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let n = u32::try_from(body.len()).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let mut all = Zeroizing::new(Vec::with_capacity(4 + body.len()));
    all.extend_from_slice(&n.to_ne_bytes());
    all.extend_from_slice(body);
    w.write_all(&all)?;
    w.flush()
}

/// Failure talking to the daemon (only for the log; the extension gets the internal error).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonFailure {
    /// No daemon, or the server did not pass the check (nothing was sent).
    Unavailable,
    /// Connection dropped, time limit, bad frame.
    Conversation,
}

impl DaemonFailure {
    pub fn text(self) -> &'static str {
        match self {
            DaemonFailure::Unavailable => "daemon indisponível ou não conferido",
            DaemonFailure::Conversation => "a conversa com o daemon falhou",
        }
    }
}

/// The path to the daemon: sends the request frame and returns the response body.
pub trait Daemon {
    fn exchange(&mut self, request: &[u8]) -> Result<Zeroizing<Vec<u8>>, DaemonFailure>;
}

/// Body of the error response for the extension.
fn error_body(e: ResponseError) -> Zeroizing<Vec<u8>> {
    protocol::encode_response(Role::Extension, &Response::Error(e)).unwrap_or_else(|_| {
        Zeroizing::new(
            br#"{"v":1,"tipo":"erro","dados":{"codigo":"interno","mensagem":"erro interno"}}"#
                .to_vec(),
        )
    })
}

/// Handles one message of the extension and returns the response body (always a
/// response of the extension role). The log text, when there is one, goes to `log`,
/// without content.
pub fn handle(
    message: &[u8],
    daemon: &mut dyn Daemon,
    log: &mut dyn FnMut(&str),
) -> Zeroizing<Vec<u8>> {
    let request = match protocol::decode_request(Role::Extension, message) {
        Ok(p) => p,
        Err(e) => {
            log("mensagem da extensão recusada pela leitura do protocolo");
            return error_body(e);
        }
    };
    let op = request.operation();
    if !OPERATIONS.contains(&op) {
        log("operação fora das da extensão");
        return error_body(ResponseError::Unauthorized);
    }
    let Ok(frame) = protocol::encode_request(Role::Extension, &request) else {
        return error_body(ResponseError::InvalidRequest);
    };
    drop(request);
    let body = match daemon.exchange(&frame) {
        Ok(c) => c,
        Err(f) => {
            log(f.text());
            return error_body(ResponseError::Internal);
        }
    };
    drop(frame);
    let response = match protocol::decode_response(Role::Extension, &body) {
        Ok(r) if response_fits(op, r.kind()) => r,
        _ => {
            log("resposta do daemon fora do papel extensão ou da operação");
            return error_body(ResponseError::Internal);
        }
    };
    // D63: the state response carries the role the daemon gave to this connection.
    // Without the extension role (the host was not opened by the installed Chrome, or it
    // is not the setgid binary), the extension gets only the unauthorized error.
    if let Response::Status { role: papel, .. } = &response
        && *papel != Role::Extension
    {
        log("o daemon não deu o papel extensão a este host");
        return error_body(ResponseError::Unauthorized);
    }
    match protocol::encode_response(Role::Extension, &response) {
        Ok(c) if c.len() <= MAX_MESSAGE_OUT => c,
        _ => {
            log("resposta grande demais para a extensão");
            error_body(ResponseError::Internal)
        }
    }
}

/// Loop of the host: reads each message, handles it and answers. Returns `Ok` at the
/// end of stdin; `Err` (after answering the error, when possible) if the message came
/// empty, too large or truncated, or if stdout failed.
pub fn serve(
    input: &mut impl Read,
    output: &mut impl Write,
    daemon: &mut dyn Daemon,
    log: &mut dyn FnMut(&str),
) -> Result<(), MessageError> {
    loop {
        let message = match read_message(input) {
            Ok(m) => m,
            Err(MessageError::End) => return Ok(()),
            Err(e) => {
                log("mensagem da extensão vazia, grande demais ou cortada");
                if matches!(e, MessageError::Empty | MessageError::TooLarge) {
                    let _ = write_message(output, &error_body(ResponseError::InvalidFrame));
                }
                return Err(e);
            }
        };
        let response = handle(&message, daemon, log);
        drop(message);
        write_message(output, &response).map_err(|e| MessageError::Io(e.kind()))?;
    }
}

#[cfg(test)]
mod tests;
