//! Vault body (D26, D30, D39, D65 d): the configuration, the generation and the entries
//! as bytes, for [`crate::format`] to encrypt.
//!
//! Own binary encoding, versioned and canonical, with no new dependency (S14).
//! Integers in little-endian; every bytes field carries its length first (u32 LE).
//!
//! ```text
//! body    = version (u16) = 5 ‖ configuration ‖ number of entries (u32) ‖ entry × n
//! configuration = generation (u64, ≥ 1)
//!           ‖ PIN hash (field: Argon2id PHC text, required)
//!           ‖ lock minutes (u32) ‖ backup enabled (u8 0|1)
//!           ‖ USB drive UUID (field; empty = none)
//! entry   = type ‖ level ‖ name ‖ group ‖ environment ‖ project
//!           ‖ number of tags (u32) ‖ tag × n
//!           ‖ type fields (see `schema`)
//!           ‖ number of extras (u32) ‖ (name ‖ secret: u8 0|1 ‖ value) × n
//!           ‖ number of attachments (u32) ‖ (name ‖ content) × n
//!           ‖ created at (u64) ‖ modified at (u64)
//! field   = length (u32) ‖ bytes
//! ```
//!
//! - **Version 3** (D30, D39): the configuration ([`VaultConfig`]) comes before the
//!   entries and is required, and it starts with the **generation**: the number of the
//!   written version, which every write from an [`OpenVault`] raises by 1. It blocks
//!   swapping the file for an older authentic version ([`open_entries_since`] and
//!   [`OpenError::Rollback`]). Versions 1 (entries only) and 2 (no generation) are
//!   unknown and refused like any other; generation 0, a body without a PIN hash, with
//!   a hash out of format or with a cost out of range too. There is no migration: no
//!   real vault exists in the old versions.
//! - **Version 4** (D65 d, provisional): each entry ends with the creation and
//!   modification dates ([`Dates`]), in UTC seconds; 0 is "no date". Reading checks the
//!   [`Dates`] rule (range from 1 to [`MAX_DATE`], creation only with modification and
//!   not after it). Version 3 (no dates) is refused as an unknown version, without
//!   migration: there is no installed vault, and `make dev-vault` rewrites the test one.
//! - **Version 5** (m2:DT4): the same layout as version 4; only the stored names of
//!   type, level and SMTP security are the English ones (the `name()` of each enum:
//!   `database`, `approval`, `none`…). Reading accepts v4 and v5; **writing only
//!   produces v5** ([`BODY_VERSION`]). A v4 body is read through the closed table of
//!   `legacy_v4` (only on the v4 reading path) and comes back as the same entries, so
//!   serializing it again gives the v5 body (the migration of m2:DT5 is the daemon's).
//!   A name outside the table of its version (a v5 name in a v4 body, a v4 name in a v5
//!   body, any other text) refuses the whole body (m2:RM2).
//! - Type, level, environment, engine, security and protocol go by the stable
//!   identifiers of Phase 15 (the `name()` of each enum, e.g. `login`). The port goes in
//!   2 bytes (u16). A missing optional field is an empty field.
//! - **Canonical**: entries go out in name order; the same entries always give the same
//!   bytes. Reading requires the strict order (which also blocks a repeated name), so
//!   every accepted v5 body, serialized again, gives the same bytes (a v4 one gives its
//!   v5 form). The names are the same set of bytes in both orders: only the stored
//!   type/level/security names change, never an entry name.
//! - **Write** (S8): the final size is computed first, and the buffer is a
//!   `Zeroizing<Vec<u8>>` with that exact capacity; no reallocation leaves a cleartext
//!   copy. A body above [`MAX_BODY`] or with more than [`MAX_ENTRIES`] is refused before
//!   allocating. Entries inconsistent with each other (repeated name, invalid tunnel or
//!   jump) are refused too: what is written always reopens.
//! - **Read** (the disk is untrusted input, even when authenticated): in two passes.
//!   The first one only walks the buffer, allocating nothing, and checks the version, the
//!   whole configuration (the PHC text is read and written again on a stack), counts,
//!   each length against what is left and against the limits of [`crate::limits`]
//!   (including the sum of the attachments of each entry, D29), type, level, name order
//!   and leftover bytes. The second one builds the configuration and each entry through
//!   [`Draft::validate`], applies the stored level through [`Entry::set_level`] and
//!   checks the whole vault through [`Entry::verify_in_vault`]. Any failure refuses the
//!   whole body with [`BodyError::Invalid`], with no user value (S13); a vault is never
//!   opened halfway.

use core::fmt;

use zeroize::Zeroizing;

use crate::config::{self, ExternalBackup, PinHash, VaultConfig};
use crate::entry::{
    AccessLevel, AttachmentDraft, Auth, Data, DataDraft, Dates, Draft, Entry, EntryType,
    ExtraDraft, ExtraValue, Secret,
};
use crate::format::{self, FormatError, MasterPassword, OpenVault};

mod legacy_v4;
#[cfg(test)]
mod tests;

/// Current body version, the only one written (D30: with the configuration section; D39:
/// with the generation; D65 d: with the dates of each entry; m2:DT4: with the English
/// stored names).
pub const BODY_VERSION: u16 = 5;

/// Previous body version (0.1.x, pt-BR stored names), accepted only on reading
/// (m2:DT4). Never written.
pub const LEGACY_BODY_VERSION: u16 = 4;

/// Version of the body being read: picks the table of the stored names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Version {
    /// [`LEGACY_BODY_VERSION`]: names through `legacy_v4`.
    V4,
    /// [`BODY_VERSION`]: names through the `from_name()` of each enum.
    V5,
}

// The limits live in `crate::limits` (D32): reading refuses above them before
// allocating, and the entry validation checks again, with the exact limit of each field.
use crate::limits::{
    MAX_ATTACHMENTS, MAX_ATTACHMENTS_TOTAL, MAX_BACKUP_UUID, MAX_DATE, MAX_EXTRA_VALUE, MAX_EXTRAS,
    MAX_NOTE, MAX_PIN_HASH, MAX_SECRET, MAX_TAGS, MAX_TEXT, MIB,
};
pub use crate::limits::{MAX_BODY, MAX_ENTRIES};
/// Most fields of a type (database, server, ftp/sftp). Structural, not a limit.
const MAX_DATA_FIELDS: usize = 7;

/// Body error. No variant carries vault data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyError {
    /// Read: the decrypted body is not a valid body (truncated, with leftovers, unknown
    /// version, type or level, field over the limit, invalid entry, repeated name,
    /// broken reference…). A single error, on purpose.
    Invalid,
    /// Write: above [`MAX_BODY`] bytes or [`MAX_ENTRIES`] entries.
    TooLarge,
    /// Write: repeated name or tunnel/jump that does not resolve in the vault (it would
    /// not reopen).
    Inconsistent,
}

impl fmt::Display for BodyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid => f.write_str("conteúdo do cofre inválido ou corrompido"),
            // Built from `crate::limits` (D45): the message follows the constant. The
            // thousands go with a space, like in the rest of the text.
            Self::TooLarge => write!(
                f,
                "cofre grande demais (limite de {} MiB e {} entradas)",
                MAX_BODY / MIB,
                Thousands(MAX_ENTRIES)
            ),
            Self::Inconsistent => f.write_str(
                "entradas inconsistentes: nome repetido ou túnel/salto que não aponta para um servidor válido",
            ),
        }
    }
}

impl std::error::Error for BodyError {}

/// Number with a space between the thousands (`10 000`), for the messages.
struct Thousands(usize);

impl fmt::Display for Thousands {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (high, low) = (self.0 / 1000, self.0 % 1000);
        if high == 0 {
            write!(f, "{low}")
        } else {
            write!(f, "{} {low:03}", Thousands(high))
        }
    }
}

/// Error when opening the vault and reading the entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenError {
    /// Wrong password, tampered file or refused parameters.
    Format(FormatError),
    /// Decrypted, but the content is not a valid body.
    Body(BodyError),
    /// Decrypted and the body is valid, but its generation is lower than the minimum
    /// asked for ([`open_entries_since`], D39): the file was swapped for an older
    /// authentic version (a backup, for example). Carries no value, not even the
    /// generations.
    Rollback,
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Format(e) => fmt::Display::fmt(e, f),
            Self::Body(e) => fmt::Display::fmt(e, f),
            Self::Rollback => f.write_str("o cofre no disco é mais antigo que o último aberto"),
        }
    }
}

impl std::error::Error for OpenError {}

/// Opens the vault file and reads the configuration, the generation and the entries (the
/// `make dev-vault` path). The returned [`OpenVault`] carries the configuration and the
/// generation read (D30, D39). The decrypted content is zeroed on drop; the entries come
/// back in name order.
///
/// Accepts any generation. The daemon (Phase 22) opens through [`open_entries_since`],
/// with the highest generation it has seen and the highest it has burned.
pub fn open_entries(
    file: &[u8],
    password: &MasterPassword,
) -> Result<(OpenVault, Vec<Entry>), OpenError> {
    open_entries_since(file, password, 0, 0)
}

/// Like [`open_entries`], but refuses with [`OpenError::Rollback`] a body whose
/// generation is lower than `min_generation` (D39): swapping the file for an older
/// authentic version. The check comes after decrypting and validating the whole body,
/// and no entry is returned.
///
/// `burned_generation` (D53) is the highest generation already sealed or attempted that
/// the caller knows of (a write that failed after sealing). The returned [`OpenVault`]
/// gets max(body generation, `burned_generation`), and the next write uses a number that
/// was never sealed.
pub fn open_entries_since(
    file: &[u8],
    password: &MasterPassword,
    min_generation: u64,
    burned_generation: u64,
) -> Result<(OpenVault, Vec<Entry>), OpenError> {
    open_entries_with_generation(file, password, min_generation, burned_generation)
        .map(|(vault, entries, _)| (vault, entries))
}

/// Like [`open_entries_since`], and also returns the generation **read from the body**
/// (Phase 22, D53). [`OpenVault::generation`] gets max(read, `burned_generation`); the
/// daemon needs the read one to raise the highest generation confirmed on disk (`vista`)
/// without mistaking it for a burned one that never reached the disk.
pub fn open_entries_with_generation(
    file: &[u8],
    password: &MasterPassword,
    min_generation: u64,
    burned_generation: u64,
) -> Result<(OpenVault, Vec<Entry>, u64), OpenError> {
    let (keys, content) = format::open(file, password).map_err(OpenError::Format)?;
    let (config, generation, entries) = deserialize(&content).map_err(OpenError::Body)?;
    drop(content);
    if generation < min_generation {
        // The entries and the configuration read are zeroed here.
        return Err(OpenError::Rollback);
    }
    Ok((
        OpenVault::from_parts(keys, config, generation.max(burned_generation)),
        entries,
        generation,
    ))
}

/// Serializes the configuration, the generation (from 1 to `u64::MAX`, D39) and the
/// entries (in any order; they go out in name order). The buffer is zeroed on drop and
/// is born with the final capacity. Generation 0 is refused with
/// [`BodyError::Inconsistent`].
///
/// Serializing does not write: outside the crate, only
/// [`crate::atomic_write::write_entries`] and [`crate::atomic_write::change_password`]
/// seal and write, and it is the write that picks the generation (the open vault's + 1).
pub fn serialize(
    config: &VaultConfig,
    generation: u64,
    entries: &[Entry],
) -> Result<Zeroizing<Vec<u8>>, BodyError> {
    if generation == 0 {
        return Err(BodyError::Inconsistent);
    }
    if entries.len() > MAX_ENTRIES {
        return Err(BodyError::TooLarge);
    }
    let mut order: Vec<&Entry> = entries.iter().collect();
    order.sort_unstable_by(|a, b| a.name().cmp(b.name()));
    if !check_sorted(&order) {
        return Err(BodyError::Inconsistent);
    }

    let mut count = Output {
        total: 0,
        buf: None,
        error: false,
    };
    write_body(&mut count, config, generation, &order);
    if count.error {
        return Err(BodyError::Inconsistent);
    }
    let total = count.total;
    if total > MAX_BODY {
        return Err(BodyError::TooLarge);
    }

    let mut buf: Zeroizing<Vec<u8>> = Zeroizing::new(Vec::new());
    buf.try_reserve_exact(total)
        .map_err(|_| BodyError::TooLarge)?;
    let mut output = Output {
        total: 0,
        buf: Some(&mut buf),
        error: false,
    };
    write_body(&mut output, config, generation, &order);
    if output.error || buf.len() != total {
        return Err(BodyError::Inconsistent);
    }
    Ok(buf)
}

/// Body read: the configuration, the generation (≥ 1) and the entries, in name order.
pub type ReadBody = (VaultConfig, u64, Vec<Entry>);

/// Reads the decrypted body: the configuration, the generation and the entries. Refuses
/// the whole body with [`BodyError::Invalid`] on any problem.
pub fn deserialize(body: &[u8]) -> Result<ReadBody, BodyError> {
    if body.len() > MAX_BODY {
        return Err(BodyError::Invalid);
    }
    // 1st pass: the whole structure, without allocating.
    let n = check_structure(body)?;

    // 2nd pass: builds and validates each entry (the `n` are already known to be in the
    // buffer).
    let mut entries: Vec<Entry> = Vec::new();
    entries
        .try_reserve_exact(n)
        .map_err(|_| BodyError::Invalid)?;
    let mut l = Reader { rest: body };
    let (version, raw_cfg, n2) = read_header(&mut l)?;
    if n2 != n {
        return Err(BodyError::Invalid);
    }
    let config = build_config(&raw_cfg)?;
    for _ in 0..n {
        let raw = read_raw(&mut l, version)?;
        entries.push(build(&raw)?);
    }
    if !l.rest.is_empty() {
        return Err(BodyError::Invalid);
    }

    let order: Vec<&Entry> = entries.iter().collect();
    if !check_sorted(&order) {
        return Err(BodyError::Invalid);
    }
    drop(order);
    Ok((config, raw_cfg.generation, entries))
}

/// Only the generation of a decrypted body (Phase 49a), after checking the whole
/// structure (without building entries or allocating secrets). `None` on any problem.
pub(crate) fn body_generation(body: &[u8]) -> Option<u64> {
    if body.len() > MAX_BODY {
        return None;
    }
    check_structure(body).ok()?;
    let mut l = Reader { rest: body };
    let (_, cfg, _) = read_header(&mut l).ok()?;
    Some(cfg.generation)
}

// ---- Vault check ----

/// Names in strict order (no repeats) and each entry checked by
/// [`Entry::verify_in_vault`] against the ones that matter to it: the one it points to
/// and the ones that point to it. These are exactly the ones `verify_in_vault` looks at
/// (same name, reference target, who references this one), without a quadratic cost
/// over 10 000 entries.
fn check_sorted(order: &[&Entry]) -> bool {
    if order.windows(2).any(|w| w[0].name() >= w[1].name()) {
        return false;
    }
    let find = |name: &str| order.binary_search_by(|e| e.name().cmp(name)).ok();
    // (target, who points), in target order.
    let mut pointing: Vec<(usize, usize)> = order
        .iter()
        .enumerate()
        .filter_map(|(j, e)| e.reference().and_then(find).map(|target| (target, j)))
        .collect();
    pointing.sort_unstable();
    let mut neighbors: Vec<usize> = Vec::new();
    for (i, e) in order.iter().enumerate() {
        neighbors.clear();
        if let Some(target) = e.reference().and_then(find)
            && target != i
        {
            neighbors.push(target);
        }
        let start = pointing.partition_point(|&(target, _)| target < i);
        for &(target, j) in &pointing[start..] {
            if target != i {
                break;
            }
            if j != i && !neighbors.contains(&j) {
                neighbors.push(j);
            }
        }
        if e.verify_in_vault(neighbors.iter().map(|&j| order[j]))
            .is_err()
        {
            return false;
        }
    }
    true
}

// ---- Schema of the fields of each type ----

/// Kind of a type field, with the read limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    /// Text that is not a secret.
    Text,
    /// Port: exactly 2 bytes (u16 LE).
    Port,
    /// Text secret (password, token, key, seed…).
    Secret,
    /// Text of the secure note.
    Note,
}

impl Field {
    fn max(self) -> usize {
        match self {
            Field::Text => MAX_TEXT,
            Field::Port => 2,
            Field::Secret => MAX_SECRET,
            Field::Note => MAX_NOTE,
        }
    }
}

/// Position of the security among the fields of the `smtp` type (see [`schema`]).
const SMTP_SECURITY: usize = 4;

/// Order of the fields of each type in the body. Reading and writing use this same
/// table.
fn schema(kind: EntryType) -> &'static [Field] {
    use Field::{Note, Port, Secret as S, Text as T};
    match kind {
        // url, username, password, TOTP seed
        EntryType::Login => &[T, T, S, S],
        // engine, host, port, database name, username, password, tunnel
        EntryType::Database => &[T, T, Port, T, T, S, T],
        // host, port, username, password, private key, passphrase, jump
        EntryType::Server => &[T, Port, T, S, S, S, T],
        // url, token, variable
        EntryType::Api => &[T, S, T],
        // access key ID, secret, region, role ARN, session token
        EntryType::Aws => &[T, S, T, T, S],
        // host, port, username, password, security
        EntryType::Smtp => &[T, Port, T, S, T],
        // protocol, host, port, username, password, private key, passphrase
        EntryType::FtpSftp => &[T, T, Port, T, S, S, S],
        // text
        EntryType::SecureNote => &[Note],
        EntryType::Generic => &[],
    }
}

// ---- Writing ----

/// Write destination: only counts (first pass) or copies into a buffer that already has
/// the final capacity (second). Never reallocates: if room ran out, it flags an error and
/// stops copying.
struct Output<'b> {
    total: usize,
    buf: Option<&'b mut Vec<u8>>,
    error: bool,
}

impl Output<'_> {
    fn put(&mut self, bytes: &[u8]) {
        self.total = self.total.saturating_add(bytes.len());
        if let Some(buf) = self.buf.as_deref_mut() {
            if buf.capacity() - buf.len() >= bytes.len() {
                buf.extend_from_slice(bytes);
            } else {
                self.error = true;
            }
        }
    }

    fn u32(&mut self, n: usize) {
        match u32::try_from(n) {
            Ok(n) => self.put(&n.to_le_bytes()),
            Err(_) => self.error = true,
        }
    }

    /// Length-prefixed field, checked against the read limit (what is written always
    /// reads back).
    fn field(&mut self, bytes: &[u8], max: usize) {
        if bytes.len() > max {
            self.error = true;
        }
        self.u32(bytes.len());
        self.put(bytes);
    }
}

fn write_body(s: &mut Output<'_>, config: &VaultConfig, generation: u64, order: &[&Entry]) {
    s.put(&BODY_VERSION.to_le_bytes());
    if generation == 0 {
        s.error = true;
    }
    s.put(&generation.to_le_bytes());
    write_config(s, config);
    s.u32(order.len());
    for e in order {
        write_entry(s, e);
    }
}

fn write_config(s: &mut Output<'_>, c: &VaultConfig) {
    s.field(c.pin_hash().bytes(), MAX_PIN_HASH);
    let minutes = c.lock_minutes();
    if !config::valid_minutes(minutes) {
        s.error = true;
    }
    s.put(&minutes.to_le_bytes());
    s.put(&[u8::from(c.backup().enabled())]);
    s.field(c.backup().uuid().unwrap_or("").as_bytes(), MAX_BACKUP_UUID);
}

fn optional(s: &Option<Secret>) -> &[u8] {
    s.as_ref().map_or(&[], Secret::expose)
}

fn optional_text(t: &Option<String>) -> &[u8] {
    t.as_deref().map_or(&[], str::as_bytes)
}

/// Password, private key and passphrase (the missing ones empty).
fn auth_fields(a: &Auth) -> [&[u8]; 3] {
    match a {
        Auth::Password(password) => [password.expose(), &[], &[]],
        Auth::Key {
            private_key,
            passphrase,
        } => [&[], private_key.expose(), optional(passphrase)],
    }
}

/// The type fields in [`schema`] order.
fn data_fields<'e>(data: &'e Data, port: &'e mut [u8; 2]) -> [&'e [u8]; MAX_DATA_FIELDS] {
    let mut c: [&[u8]; MAX_DATA_FIELDS] = [&[]; MAX_DATA_FIELDS];
    match data {
        Data::Login(l) => {
            c[..4].copy_from_slice(&[
                l.url.as_bytes(),
                l.username.as_bytes(),
                l.password.expose(),
                optional(&l.totp_seed),
            ]);
        }
        Data::Database(b) => {
            *port = b.port.to_le_bytes();
            c.copy_from_slice(&[
                b.engine.name().as_bytes(),
                b.host.as_bytes(),
                &port[..],
                optional_text(&b.db_name),
                b.username.as_bytes(),
                b.password.expose(),
                optional_text(&b.tunnel),
            ]);
        }
        Data::Server(sv) => {
            *port = sv.port.to_le_bytes();
            let [password, key, passphrase] = auth_fields(&sv.auth);
            c.copy_from_slice(&[
                sv.host.as_bytes(),
                &port[..],
                sv.username.as_bytes(),
                password,
                key,
                passphrase,
                optional_text(&sv.jump),
            ]);
        }
        Data::Api(a) => {
            c[..3].copy_from_slice(&[
                optional_text(&a.url),
                a.token.expose(),
                optional_text(&a.variable),
            ]);
        }
        Data::Aws(a) => {
            c[..5].copy_from_slice(&[
                a.access_key_id.as_bytes(),
                a.secret_access_key.expose(),
                a.region.as_bytes(),
                optional_text(&a.role_arn),
                optional(&a.session_token),
            ]);
        }
        Data::Smtp(sm) => {
            *port = sm.port.to_le_bytes();
            c[..5].copy_from_slice(&[
                sm.host.as_bytes(),
                &port[..],
                sm.username.as_bytes(),
                sm.password.expose(),
                sm.security.name().as_bytes(),
            ]);
        }
        Data::FtpSftp(f) => {
            *port = f.port.to_le_bytes();
            let [password, key, passphrase] = auth_fields(&f.auth);
            c.copy_from_slice(&[
                f.protocol.name().as_bytes(),
                f.host.as_bytes(),
                &port[..],
                f.username.as_bytes(),
                password,
                key,
                passphrase,
            ]);
        }
        Data::SecureNote(n) => c[0] = n.text.expose(),
        Data::Generic => {}
    }
    c
}

fn write_entry(s: &mut Output<'_>, e: &Entry) {
    s.field(e.kind().name().as_bytes(), MAX_TEXT);
    s.field(e.level().name().as_bytes(), MAX_TEXT);
    s.field(e.name().as_bytes(), MAX_TEXT);
    s.field(e.group().as_bytes(), MAX_TEXT);
    s.field(e.environment().name().as_bytes(), MAX_TEXT);
    s.field(e.project().unwrap_or("").as_bytes(), MAX_TEXT);
    s.u32(e.tags().len());
    for tag in e.tags() {
        s.field(tag.as_bytes(), MAX_TEXT);
    }
    let mut port = [0u8; 2];
    let values = data_fields(e.data(), &mut port);
    for (field, value) in schema(e.kind()).iter().zip(values) {
        s.field(value, field.max());
    }
    s.u32(e.extras().len());
    for extra in e.extras() {
        s.field(extra.name().as_bytes(), MAX_TEXT);
        match extra.value() {
            ExtraValue::Text(t) => {
                s.put(&[0]);
                s.field(t.as_bytes(), MAX_EXTRA_VALUE);
            }
            ExtraValue::Secret(v) => {
                s.put(&[1]);
                s.field(v.expose(), MAX_EXTRA_VALUE);
            }
        }
    }
    s.u32(e.attachments().len());
    for attachment in e.attachments() {
        s.field(attachment.name().as_bytes(), MAX_TEXT);
        s.field(attachment.content().expose(), MAX_ATTACHMENTS_TOTAL);
    }
    // D65 d: 0 = no date. A `Dates` only exists within the rule; the range is checked
    // again so that what is written always reopens.
    let dates = e.dates();
    for d in [dates.created_at(), dates.modified_at()] {
        let d = d.unwrap_or(0);
        if d > MAX_DATE {
            s.error = true;
        }
        s.put(&d.to_le_bytes());
    }
}

// ---- Reading ----

/// Reader over the body: every read checks what is left first; nothing allocates.
struct Reader<'a> {
    rest: &'a [u8],
}

impl<'a> Reader<'a> {
    fn bytes(&mut self, n: usize) -> Result<&'a [u8], BodyError> {
        if n > self.rest.len() {
            return Err(BodyError::Invalid);
        }
        let (chunk, rest) = self.rest.split_at(n);
        self.rest = rest;
        Ok(chunk)
    }

    fn u8(&mut self) -> Result<u8, BodyError> {
        Ok(self.bytes(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, BodyError> {
        let b = self.bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u64(&mut self) -> Result<u64, BodyError> {
        let b = self.bytes(8)?;
        let mut b8 = [0u8; 8];
        b8.copy_from_slice(b);
        Ok(u64::from_le_bytes(b8))
    }

    fn u32(&mut self) -> Result<usize, BodyError> {
        let b = self.bytes(4)?;
        usize::try_from(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .map_err(|_| BodyError::Invalid)
    }

    /// Count of a list, refused above the limit before anything else.
    fn count(&mut self, max: usize) -> Result<usize, BodyError> {
        let n = self.u32()?;
        if n > max {
            return Err(BodyError::Invalid);
        }
        Ok(n)
    }

    /// Length-prefixed field: the length is checked against the limit and against what
    /// is left of the buffer before anything else.
    fn field(&mut self, max: usize) -> Result<&'a [u8], BodyError> {
        let n = self.u32()?;
        if n > max {
            return Err(BodyError::Invalid);
        }
        self.bytes(n)
    }
}

/// A list (tags, extras, attachments): how many items and their bytes, already walked.
#[derive(Clone, Copy)]
struct List<'a> {
    n: usize,
    bytes: &'a [u8],
}

/// An entry as it is in the buffer: only slices, nothing allocated. Everything already
/// checked against the limits; the content has not been validated yet.
struct RawEntry<'a> {
    kind: EntryType,
    level: AccessLevel,
    name: &'a [u8],
    group: &'a [u8],
    environment: &'a [u8],
    project: &'a [u8],
    tags: List<'a>,
    data: [&'a [u8]; MAX_DATA_FIELDS],
    extras: List<'a>,
    attachments: List<'a>,
    /// Already checked by the [`Dates`] rule (D65 d).
    dates: Dates,
}

/// The configuration section as it is in the buffer, already fully checked (without
/// allocating).
struct RawConfig<'a> {
    generation: u64,
    pin_hash: &'a [u8],
    lock_minutes: u32,
    enabled: bool,
    uuid: &'a [u8],
}

/// Version, configuration and number of entries. Only v5 and v4 (m2:DT4) are known;
/// the configuration and the count have the same layout in both.
fn read_header<'a>(l: &mut Reader<'a>) -> Result<(Version, RawConfig<'a>, usize), BodyError> {
    let version = match l.u16()? {
        BODY_VERSION => Version::V5,
        LEGACY_BODY_VERSION => Version::V4,
        _ => return Err(BodyError::Invalid),
    };
    let cfg = read_config(l)?;
    Ok((version, cfg, l.count(MAX_ENTRIES)?))
}

/// The configuration section (D30, D39): generation ≥ 1, PHC hash fully checked
/// (Argon2id, cost in range, salt and output of the right size, canonical text), lock
/// time in range, flag 0|1 and valid UUID; enabled without UUID is refused. Nothing
/// allocates.
fn read_config<'a>(l: &mut Reader<'a>) -> Result<RawConfig<'a>, BodyError> {
    // D39: the generation starts at 1; 0 is an invalid body (the generic error).
    let generation = l.u64()?;
    if generation == 0 {
        return Err(BodyError::Invalid);
    }
    let pin_hash = l.field(MAX_PIN_HASH)?;
    if !config::verify_text(pin_hash) {
        return Err(BodyError::Invalid);
    }
    let b = l.bytes(4)?;
    let lock_minutes = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    if !config::valid_minutes(lock_minutes) {
        return Err(BodyError::Invalid);
    }
    let enabled = match l.u8()? {
        0 => false,
        1 => true,
        _ => return Err(BodyError::Invalid),
    };
    let uuid = l.field(MAX_BACKUP_UUID)?;
    if (!uuid.is_empty() && !config::valid_uuid(uuid)) || (enabled && uuid.is_empty()) {
        return Err(BodyError::Invalid);
    }
    Ok(RawConfig {
        generation,
        pin_hash,
        lock_minutes,
        enabled,
        uuid,
    })
}

/// 2nd pass of the configuration: copies the hash into a type that is zeroed on drop.
fn build_config(b: &RawConfig<'_>) -> Result<VaultConfig, BodyError> {
    let pin_hash = PinHash::from_phc(b.pin_hash).ok_or(BodyError::Invalid)?;
    let uuid = if b.uuid.is_empty() {
        None
    } else {
        Some(core::str::from_utf8(b.uuid).map_err(|_| BodyError::Invalid)?)
    };
    let backup = ExternalBackup::new(b.enabled, uuid).map_err(|_| BodyError::Invalid)?;
    Ok(VaultConfig::from_parts(pin_hash, b.lock_minutes, backup))
}

fn read_tag<'a>(l: &mut Reader<'a>) -> Result<&'a [u8], BodyError> {
    l.field(MAX_TEXT)
}

fn read_extra<'a>(l: &mut Reader<'a>) -> Result<(&'a [u8], bool, &'a [u8]), BodyError> {
    let name = l.field(MAX_TEXT)?;
    let secret = match l.u8()? {
        0 => false,
        1 => true,
        _ => return Err(BodyError::Invalid),
    };
    let value = l.field(MAX_EXTRA_VALUE)?;
    Ok((name, secret, value))
}

fn read_attachment<'a>(l: &mut Reader<'a>) -> Result<(&'a [u8], &'a [u8]), BodyError> {
    let name = l.field(MAX_TEXT)?;
    let content = l.field(MAX_ATTACHMENTS_TOTAL)?;
    Ok((name, content))
}

/// The attachments of an entry: up to [`MAX_ATTACHMENTS`] items that add up to at most
/// [`MAX_ATTACHMENTS_TOTAL`] (D29), checked before allocating.
fn read_attachments<'a>(l: &mut Reader<'a>) -> Result<List<'a>, BodyError> {
    let n = l.count(MAX_ATTACHMENTS)?;
    let start = l.rest;
    let mut sum: usize = 0;
    for _ in 0..n {
        let (_, content) = read_attachment(l)?;
        sum = sum
            .checked_add(content.len())
            .filter(|s| *s <= MAX_ATTACHMENTS_TOTAL)
            .ok_or(BodyError::Invalid)?;
    }
    let used = start.len() - l.rest.len();
    Ok(List {
        n,
        bytes: &start[..used],
    })
}

/// Walks a list of up to `max` items with `item`, without allocating.
fn read_list<'a, T>(
    l: &mut Reader<'a>,
    max: usize,
    item: fn(&mut Reader<'a>) -> Result<T, BodyError>,
) -> Result<List<'a>, BodyError> {
    let n = l.count(max)?;
    let start = l.rest;
    for _ in 0..n {
        item(l)?;
    }
    let used = start.len() - l.rest.len();
    Ok(List {
        n,
        bytes: &start[..used],
    })
}

/// Stored type: by the table of the body version (v4 only through `legacy_v4`).
fn stored_kind(version: Version, stored: &[u8]) -> Option<EntryType> {
    match version {
        Version::V5 => core::str::from_utf8(stored)
            .ok()
            .and_then(EntryType::from_name),
        Version::V4 => legacy_v4::kind(stored),
    }
}

/// Stored level: by the table of the body version (v4 only through `legacy_v4`).
fn stored_level(version: Version, stored: &[u8]) -> Option<AccessLevel> {
    match version {
        Version::V5 => core::str::from_utf8(stored)
            .ok()
            .and_then(AccessLevel::from_name),
        Version::V4 => legacy_v4::level(stored),
    }
}

fn read_raw<'a>(l: &mut Reader<'a>, version: Version) -> Result<RawEntry<'a>, BodyError> {
    let kind = stored_kind(version, l.field(MAX_TEXT)?).ok_or(BodyError::Invalid)?;
    let level = stored_level(version, l.field(MAX_TEXT)?).ok_or(BodyError::Invalid)?;
    let name = l.field(MAX_TEXT)?;
    let group = l.field(MAX_TEXT)?;
    let environment = l.field(MAX_TEXT)?;
    let project = l.field(MAX_TEXT)?;
    let tags = read_list(l, MAX_TAGS, read_tag)?;
    let mut data: [&[u8]; MAX_DATA_FIELDS] = [&[]; MAX_DATA_FIELDS];
    for (i, field) in schema(kind).iter().enumerate() {
        let value = l.field(field.max())?;
        if *field == Field::Port && value.len() != 2 {
            return Err(BodyError::Invalid);
        }
        data[i] = value;
    }
    // m2:DT4: the SMTP security of a v4 body becomes the v5 name here, by the closed
    // table; outside it the body is refused already in the 1st pass. In a v5 body the
    // stored name goes as it is to the validation, which only knows the v5 names.
    if version == Version::V4 && kind == EntryType::Smtp {
        data[SMTP_SECURITY] =
            legacy_v4::smtp_security(data[SMTP_SECURITY]).ok_or(BodyError::Invalid)?;
    }
    let extras = read_list(l, MAX_EXTRAS, read_extra)?;
    let attachments = read_attachments(l)?;
    let dates = read_dates(l)?;
    Ok(RawEntry {
        kind,
        level,
        name,
        group,
        environment,
        project,
        tags,
        data,
        extras,
        attachments,
        dates,
    })
}

/// The dates of the entry (D65 d): 0 = none; the rest by the [`Dates`] rule.
fn read_dates(l: &mut Reader<'_>) -> Result<Dates, BodyError> {
    let optional = |d: u64| (d != 0).then_some(d);
    let created = l.u64()?;
    let modified = l.u64()?;
    Dates::from_parts(optional(created), optional(modified)).ok_or(BodyError::Invalid)
}

/// 1st pass: the whole body, without allocating. Returns the number of entries.
fn check_structure(body: &[u8]) -> Result<usize, BodyError> {
    let mut l = Reader { rest: body };
    let (version, _, n) = read_header(&mut l)?;
    let mut previous: Option<&[u8]> = None;
    for _ in 0..n {
        let b = read_raw(&mut l, version)?;
        // Strict order of the names: canonical and with no repeated name.
        if previous.is_some_and(|a| a >= b.name) {
            return Err(BodyError::Invalid);
        }
        previous = Some(b.name);
    }
    if !l.rest.is_empty() {
        return Err(BodyError::Invalid);
    }
    Ok(n)
}

fn text(bytes: &[u8]) -> Result<String, BodyError> {
    core::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| BodyError::Invalid)
}

/// Secret copied from the buffer into an allocation of the exact size (empty = missing).
fn secret(bytes: &[u8]) -> Option<Secret> {
    (!bytes.is_empty()).then(|| Secret::from_bytes(bytes.to_vec()))
}

fn port(bytes: &[u8]) -> Result<String, BodyError> {
    match bytes {
        [a, b] => Ok(u16::from_le_bytes([*a, *b]).to_string()),
        _ => Err(BodyError::Invalid),
    }
}

fn build_data(kind: EntryType, d: &[&[u8]; MAX_DATA_FIELDS]) -> Result<DataDraft, BodyError> {
    Ok(match kind {
        EntryType::Login => DataDraft::Login {
            url: text(d[0])?,
            username: text(d[1])?,
            password: secret(d[2]),
            totp_seed: secret(d[3]),
        },
        EntryType::Database => DataDraft::Database {
            engine: text(d[0])?,
            host: text(d[1])?,
            port: port(d[2])?,
            db_name: text(d[3])?,
            username: text(d[4])?,
            password: secret(d[5]),
            tunnel: text(d[6])?,
        },
        EntryType::Server => DataDraft::Server {
            host: text(d[0])?,
            port: port(d[1])?,
            username: text(d[2])?,
            password: secret(d[3]),
            private_key: secret(d[4]),
            passphrase: secret(d[5]),
            jump: text(d[6])?,
        },
        EntryType::Api => DataDraft::Api {
            url: text(d[0])?,
            token: secret(d[1]),
            variable: text(d[2])?,
        },
        EntryType::Aws => DataDraft::Aws {
            access_key_id: text(d[0])?,
            secret_access_key: secret(d[1]),
            region: text(d[2])?,
            role_arn: text(d[3])?,
            session_token: secret(d[4]),
        },
        EntryType::Smtp => DataDraft::Smtp {
            host: text(d[0])?,
            port: port(d[1])?,
            username: text(d[2])?,
            password: secret(d[3]),
            security: text(d[4])?,
        },
        EntryType::FtpSftp => DataDraft::FtpSftp {
            protocol: text(d[0])?,
            host: text(d[1])?,
            port: port(d[2])?,
            username: text(d[3])?,
            password: secret(d[4]),
            private_key: secret(d[5]),
            passphrase: secret(d[6]),
        },
        EntryType::SecureNote => DataDraft::SecureNote { text: secret(d[0]) },
        EntryType::Generic => DataDraft::Generic,
    })
}

/// 2nd pass of an entry: draft → [`Draft::validate`] → stored level.
fn build(b: &RawEntry<'_>) -> Result<Entry, BodyError> {
    let mut tags = Vec::with_capacity(b.tags.n);
    let mut l = Reader { rest: b.tags.bytes };
    for _ in 0..b.tags.n {
        tags.push(text(read_tag(&mut l)?)?);
    }
    let mut extras = Vec::with_capacity(b.extras.n);
    let mut l = Reader {
        rest: b.extras.bytes,
    };
    for _ in 0..b.extras.n {
        let (name, secret, value) = read_extra(&mut l)?;
        extras.push(ExtraDraft {
            name: text(name)?,
            value: Secret::from_bytes(value.to_vec()),
            secret,
        });
    }
    let mut attachments = Vec::with_capacity(b.attachments.n);
    let mut l = Reader {
        rest: b.attachments.bytes,
    };
    for _ in 0..b.attachments.n {
        let (name, content) = read_attachment(&mut l)?;
        attachments.push(AttachmentDraft {
            name: text(name)?,
            content: Secret::from_bytes(content.to_vec()),
        });
    }
    let draft = Draft {
        name: text(b.name)?,
        group: text(b.group)?,
        environment: text(b.environment)?,
        project: text(b.project)?,
        tags,
        data: build_data(b.kind, &b.data)?,
        extras,
        attachments,
    };
    let mut entry = draft.validate().map_err(|_| BodyError::Invalid)?;
    if entry.kind() != b.kind {
        return Err(BodyError::Invalid);
    }
    entry.set_level(b.level);
    entry.set_dates(b.dates);
    Ok(entry)
}
