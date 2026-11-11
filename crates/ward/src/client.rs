//! Connection to `wardd` (S1, S5, D46). Every connection goes through the server check
//! of `ward_platform::server` before the first byte: in the release build, root's
//! folders, a 0600 socket of the user's own uid, the peer address equal to the path and
//! a peer with uid 0. The CLI always talks as the agent role and only accepts a response
//! of that role: a response with a credential is refused on read (S1).

use std::io;
use std::os::unix::net::UnixStream;
use std::time::Duration;

use ward_core::limits::MAX_COMMON_FRAME;
use ward_core::protocolo::quadro::{self, ErroQuadro, LIMITE_RESPOSTA};
use ward_core::protocolo::{self, Papel, Pedido, Resposta};
use ward_platform::{ExpectedServer, ServerRefusal, connect_server, verify_server};

/// Time limit for the daemon to answer. The unlock can take the growing wait of D20
/// (up to 30 s) plus Argon2id.
const READ_TIMEOUT: Duration = Duration::from_secs(130);
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// The server this CLI accepts.
///
/// - Release: only the installed `wardd`, `/run/ward/<uid>/wardd.sock` (D46). No
///   environment variable nor argument changes the path or the rules.
/// - `test-mode`: the test daemon (D7) in `WARD_DEV_SOCKET_DIR` or the user's test
///   socket folder under `/run/user/<uid>/`.
pub fn expected_server() -> ExpectedServer {
    #[cfg(feature = "test-mode")]
    {
        if let Some(d) = std::env::var_os("WARD_DEV_SOCKET_DIR") {
            let d = std::path::PathBuf::from(d);
            return ExpectedServer::test_at(d.join(ward_platform::socket::SOCKET_NAME));
        }
        ExpectedServer::test()
    }
    #[cfg(not(feature = "test-mode"))]
    {
        ExpectedServer::system()
    }
}

/// Why the conversation with the daemon failed. Without frame content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionError {
    /// No daemon is listening.
    Missing,
    /// The server did not pass the check: nothing was sent.
    Untrusted(String),
    /// The daemon did not answer in time.
    NoResponse,
    /// The daemon closed the connection.
    Closed,
    /// Another I/O error.
    Io(io::ErrorKind),
    /// Response out of format, of another role or too large.
    InvalidResponse,
    /// Request the protocol refuses (should not happen).
    InvalidRequest,
}

impl ConnectionError {
    pub fn message(&self) -> String {
        match self {
            ConnectionError::Missing => "o daemon não está rodando".to_owned(),
            ConnectionError::Untrusted(m) => {
                format!("servidor não é o wardd ({m}); nada foi enviado")
            }
            ConnectionError::NoResponse => "o daemon não respondeu a tempo".to_owned(),
            ConnectionError::Closed => "o daemon fechou a conexão".to_owned(),
            ConnectionError::Io(k) => format!("erro ao falar com o daemon ({k})"),
            ConnectionError::InvalidResponse => "resposta inválida do daemon".to_owned(),
            ConnectionError::InvalidRequest => "pedido inválido".to_owned(),
        }
    }
}

impl From<ServerRefusal> for ConnectionError {
    fn from(r: ServerRefusal) -> Self {
        match r {
            ServerRefusal::Missing => ConnectionError::Missing,
            ServerRefusal::Io(k) => ConnectionError::Io(k),
            other => ConnectionError::Untrusted(other.to_string()),
        }
    }
}

fn io_error(e: &io::Error) -> ConnectionError {
    match e.kind() {
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => ConnectionError::NoResponse,
        io::ErrorKind::BrokenPipe
        | io::ErrorKind::ConnectionReset
        | io::ErrorKind::UnexpectedEof => ConnectionError::Closed,
        k => ConnectionError::Io(k),
    }
}

fn frame_error(e: ErroQuadro) -> ConnectionError {
    match e {
        ErroQuadro::Fechado | ErroQuadro::Truncado => ConnectionError::Closed,
        ErroQuadro::Vazio | ErroQuadro::GrandeDemais => ConnectionError::InvalidResponse,
        ErroQuadro::Io(k) => io_error(&k.into()),
    }
}

/// Checked connection with the daemon.
#[derive(Debug)]
pub struct Connection {
    stream: UnixStream,
    expected: ExpectedServer,
    /// `test-mode`: require the socket's process to be non-dumpable (see
    /// [`Connection::verify_extra`]). Only the CLI's own tests turn it off.
    #[cfg(feature = "test-mode")]
    require_non_dumpable: bool,
}

impl Connection {
    /// Checks the server, connects and checks again; no byte is written before.
    pub fn connect(expected: &ExpectedServer) -> Result<Connection, ConnectionError> {
        Connection::connect_with(expected, true)
    }

    fn connect_with(
        expected: &ExpectedServer,
        _require_non_dumpable: bool,
    ) -> Result<Connection, ConnectionError> {
        let stream = connect_server(expected)?;
        stream
            .set_read_timeout(Some(READ_TIMEOUT))
            .and_then(|()| stream.set_write_timeout(Some(WRITE_TIMEOUT)))
            .map_err(|e| io_error(&e))?;
        let c = Connection {
            stream,
            expected: expected.clone(),
            #[cfg(feature = "test-mode")]
            require_non_dumpable: _require_non_dumpable,
        };
        c.verify_extra()?;
        Ok(c)
    }

    /// Only in the CLI tests: connects without the extra `test-mode` check (the honest
    /// fake server of the test runs in the test process itself, which is dumpable).
    #[cfg(all(test, feature = "test-mode"))]
    pub(crate) fn connect_without_extra(
        expected: &ExpectedServer,
    ) -> Result<Connection, ConnectionError> {
        Connection::connect_with(expected, false)
    }

    /// Checks the server again on the socket already open, right before sending the
    /// password.
    pub fn reverify(&self) -> Result<(), ConnectionError> {
        verify_server(&self.stream, &self.expected)?;
        self.verify_extra()
    }

    /// `test-mode`: without the uid 0 check (the test daemon belongs to the user
    /// themself, D46), the CLI requires at least that the process on the other side is
    /// non-dumpable, like `wardd` (S8): its `/proc/<pid>/exe` gives `EACCES`. A common
    /// fake server (a Python script, a `socat`) is dumpable and is refused. It is not a
    /// guarantee: a fake made for this can mark itself non-dumpable. The guarantee is
    /// the release one (uid 0).
    fn verify_extra(&self) -> Result<(), ConnectionError> {
        #[cfg(feature = "test-mode")]
        if self.require_non_dumpable {
            let peer =
                ward_platform::PeerProcess::from_socket(&self.stream).map_err(|e| io_error(&e))?;
            let pid = peer.credentials().pid;
            match std::fs::read_link(format!("/proc/{pid}/exe")) {
                Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {}
                _ => {
                    return Err(ConnectionError::Untrusted(
                        "o processo do socket pode ser depurado; o wardd não pode".to_owned(),
                    ));
                }
            }
            // The same process, alive and without a debugger.
            peer.reverify_no_debugger().map_err(|_| {
                ConnectionError::Untrusted(
                    "o processo do socket mudou ou está sob depurador".to_owned(),
                )
            })?;
        }
        Ok(())
    }

    /// Like [`Connection::request`], waiting for the response up to `timeout` (the
    /// `ward playwright` waits for the unlock, the approval and the browser). The time
    /// limit goes back to normal afterwards.
    pub fn request_with_timeout(
        &mut self,
        request: &Pedido,
        timeout: Duration,
    ) -> Result<Resposta, ConnectionError> {
        self.stream
            .set_read_timeout(Some(timeout))
            .map_err(|e| io_error(&e))?;
        let r = self.request(request);
        let _ = self.stream.set_read_timeout(Some(READ_TIMEOUT));
        r
    }

    /// Sends a request of the agent role and reads a response of the agent role.
    pub fn request(&mut self, request: &Pedido) -> Result<Resposta, ConnectionError> {
        let body = protocolo::codificar_pedido(Papel::Agente, request)
            .map_err(|_| ConnectionError::InvalidRequest)?;
        quadro::escrever_quadro(&mut self.stream, &body, MAX_COMMON_FRAME).map_err(frame_error)?;
        drop(body);
        let response =
            quadro::ler_quadro(&mut self.stream, LIMITE_RESPOSTA).map_err(frame_error)?;
        protocolo::decodificar_resposta(Papel::Agente, &response)
            .map_err(|_| ConnectionError::InvalidResponse)
    }
}

/// Session of `ward run` (Phase 26): after the run request, the daemon sends output
/// frames and the end frame; the CLI sends the standard input in stdin frames (empty =
/// end). No read nor write time limit: the command lasts as long as it lasts, and the
/// pipe pressure goes back to the child.
#[derive(Debug)]
pub struct Session {
    reader: UnixStream,
}

/// The write end of the session (the standard input thread).
#[derive(Debug)]
pub struct SessionWriter {
    writer: UnixStream,
}

impl Connection {
    /// Sends the request that opens the session (the run request) and returns both
    /// ends.
    pub fn open_session(
        self,
        request: &Pedido,
    ) -> Result<(Session, SessionWriter), ConnectionError> {
        let mut stream = self.stream;
        let body = protocolo::codificar_pedido(Papel::Agente, request)
            .map_err(|_| ConnectionError::InvalidRequest)?;
        quadro::escrever_quadro(&mut stream, &body, MAX_COMMON_FRAME).map_err(frame_error)?;
        drop(body);
        stream
            .set_read_timeout(None)
            .and_then(|()| stream.set_write_timeout(None))
            .map_err(|e| io_error(&e))?;
        let writer = stream.try_clone().map_err(|e| io_error(&e))?;
        Ok((Session { reader: stream }, SessionWriter { writer }))
    }
}

impl Session {
    /// The next response of the daemon (of the agent role: one with a credential is
    /// refused).
    pub fn receive(&mut self) -> Result<Resposta, ConnectionError> {
        let frame = quadro::ler_quadro(&mut self.reader, LIMITE_RESPOSTA).map_err(frame_error)?;
        protocolo::decodificar_resposta(Papel::Agente, &frame)
            .map_err(|_| ConnectionError::InvalidResponse)
    }
}

impl SessionWriter {
    /// Sends a chunk of the standard input (empty = end of input).
    pub fn send_stdin(&mut self, data: ward_core::entry::Secret) -> Result<(), ConnectionError> {
        let body =
            protocolo::codificar_pedido(Papel::Agente, &Pedido::EntradaPadrao { dados: data })
                .map_err(|_| ConnectionError::InvalidRequest)?;
        quadro::escrever_quadro(&mut self.writer, &body, MAX_COMMON_FRAME).map_err(frame_error)
    }

    /// Sends the new terminal size (interactive `ward ssh`, Phase 27).
    pub fn send_window(&mut self, w: ward_core::protocolo::Janela) -> Result<(), ConnectionError> {
        let body = protocolo::codificar_pedido(Papel::Agente, &Pedido::Janela(w))
            .map_err(|_| ConnectionError::InvalidRequest)?;
        quadro::escrever_quadro(&mut self.writer, &body, MAX_COMMON_FRAME).map_err(frame_error)
    }
}
