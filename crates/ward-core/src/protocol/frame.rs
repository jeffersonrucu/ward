//! Socket frames: 4 size bytes (big-endian u32) and the body, a JSON object.
//!
//! - The size is checked against the role's limit **before** reading the body and
//!   allocating ([`frame_limit`], V3.4). Size 0 is refused. A "negative size" (the 32-bit
//!   `-1` of a client that writes it signed) arrives as `0xFFFF_FFFF` and is refused as
//!   too large.
//! - The body is read in blocks of [`BLOCK`] that only exist after the bytes arrive: a
//!   client that declares a large frame and goes silent does not make the daemon reserve
//!   all the memory. At the end the blocks are joined in a buffer of exact capacity and
//!   zeroed (the frame may carry a secret, S8).
//! - The errors carry no bytes of the frame.

use std::io::{self, Read, Write};

use zeroize::Zeroizing;

use crate::limits::{KIB, MAX_APP_FRAME, MAX_COMMON_FRAME, MAX_RESPONSE_FRAME};
use crate::protocol::Role;

/// Read block size.
pub const BLOCK: usize = 64 * KIB;

/// Bytes of the frame header.
pub const HEADER: usize = 4;

/// Largest frame the daemon reads from a client with this role (D5, D29). The role
/// comes from the connection (`SO_PEERCRED`, Phase 21), never from the frame.
pub const fn frame_limit(role: Role) -> usize {
    match role {
        Role::App => MAX_APP_FRAME,
        Role::Agent | Role::Extension => MAX_COMMON_FRAME,
    }
}

/// Largest frame a client reads from the daemon.
pub const RESPONSE_LIMIT: usize = MAX_RESPONSE_FRAME;

/// Why a frame was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// The other side closed before the first header byte (normal end).
    Closed,
    /// The other side closed in the middle of the header or of the body.
    Truncated,
    /// Size 0.
    Empty,
    /// Size above the limit (includes the "negative" one).
    TooLarge,
    /// I/O failure (timeout, broken connection…).
    Io(io::ErrorKind),
}

impl core::fmt::Display for FrameError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Closed => f.write_str("conexão fechada"),
            Self::Truncated => f.write_str("quadro incompleto"),
            Self::Empty => f.write_str("quadro vazio"),
            Self::TooLarge => f.write_str("quadro grande demais"),
            Self::Io(k) => write!(f, "falha de E/S no quadro ({k})"),
        }
    }
}

impl std::error::Error for FrameError {}

/// Reads the header and returns the size checked against `limit`.
pub fn read_header(r: &mut impl Read, limit: usize) -> Result<usize, FrameError> {
    let mut header = [0u8; HEADER];
    let mut read = 0;
    while read < HEADER {
        match r.read(&mut header[read..]) {
            Ok(0) if read == 0 => return Err(FrameError::Closed),
            Ok(0) => return Err(FrameError::Truncated),
            Ok(n) => read += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(FrameError::Io(e.kind())),
        }
    }
    validate_size(u32::from_be_bytes(header), limit)
}

/// Checks the declared size.
pub fn validate_size(declared: u32, limit: usize) -> Result<usize, FrameError> {
    let n = usize::try_from(declared).map_err(|_| FrameError::TooLarge)?;
    if n == 0 {
        Err(FrameError::Empty)
    } else if n > limit {
        Err(FrameError::TooLarge)
    } else {
        Ok(n)
    }
}

/// Reads a whole frame of up to `limit` bytes. The body comes back in a buffer that
/// zeroes on drop, with the exact capacity.
pub fn read_frame(r: &mut impl Read, limit: usize) -> Result<Zeroizing<Vec<u8>>, FrameError> {
    let total = read_header(r, limit)?;
    // Blocks that zero on drop; each one is only allocated when there are bytes for it.
    let mut blocks: Vec<Zeroizing<Vec<u8>>> = Vec::new();
    let mut missing = total;
    while missing > 0 {
        let n = missing.min(BLOCK);
        let mut block = Zeroizing::new(vec![0u8; n]);
        let mut filled = 0;
        while filled < n {
            match r.read(&mut block[filled..]) {
                Ok(0) => return Err(FrameError::Truncated),
                Ok(k) => filled += k,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(FrameError::Io(e.kind())),
            }
        }
        blocks.push(block);
        missing -= n;
    }
    if blocks.len() == 1 {
        return Ok(blocks.pop().unwrap_or_default());
    }
    let mut body = Zeroizing::new(Vec::with_capacity(total));
    for b in &blocks {
        body.extend_from_slice(b);
    }
    Ok(body)
}

/// Writes a frame. Refuses an empty body or one above `limit` without writing anything.
pub fn write_frame(w: &mut impl Write, body: &[u8], limit: usize) -> Result<(), FrameError> {
    let n = u32::try_from(body.len()).map_err(|_| FrameError::TooLarge)?;
    validate_size(n, limit)?;
    let io = |e: io::Error| FrameError::Io(e.kind());
    w.write_all(&n.to_be_bytes()).map_err(io)?;
    w.write_all(body).map_err(io)?;
    w.flush().map_err(io)
}
