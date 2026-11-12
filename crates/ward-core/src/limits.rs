//! Vault limits in a single source (D25, D29, D32).
//!
//! Entry validation, reading and writing the vault body and the output mask use these
//! constants; so do the protocol (Phase 20) and the app (Phase 34). None of those modules repeats a number
//! from here. A limit is not a secret: everything is `pub`.
//!
//! Everything that comes from the socket, the extension and the disk is untrusted; these
//! limits are checked before allocating (body) and on save (validation).

/// Bytes in 1 KiB.
pub const KIB: usize = 1024;
/// Bytes in 1 MiB.
pub const MIB: usize = 1024 * KIB;

// ---- Text (UTF-8 bytes) ----

/// Entry name (slug), the `api` variable and the tunnel/jump reference.
pub const MAX_NAME: usize = 64;
/// Common one-line text (group, project, username…).
pub const MAX_LINE: usize = 256;
/// Username of any type (the one of the server and ftp/sftp types still follows the D43
/// format, `[A-Za-z0-9._-]`).
pub const MAX_USERNAME: usize = MAX_LINE;
/// URL and role ARN: the largest text of D25.
pub const MAX_URL: usize = 2048;
/// Host (the DNS limit).
pub const MAX_HOST: usize = 253;
/// One tag.
pub const MAX_TAG: usize = 32;
/// Name of an extra field.
pub const MAX_EXTRA_NAME: usize = 64;
/// Name of an attachment.
pub const MAX_ATTACHMENT_NAME: usize = 128;
/// AWS access key ID: from 16 to 128 characters.
pub const MIN_ACCESS_KEY: usize = 16;
/// AWS access key ID: from 16 to 128 characters.
pub const MAX_ACCESS_KEY: usize = 128;
/// AWS region (e.g. `sa-east-1`).
pub const MAX_REGION: usize = 32;
/// Database name of the database type (every engine but `redis`): `[A-Za-z0-9_-]`, from 1 to 63
/// (the Postgres limit, D33).
pub const MAX_DB_NAME: usize = 63;
/// Database of `redis`: a number from 0 to 99999 (D33).
pub const MAX_REDIS_DB_NAME: u32 = 99_999;

/// Largest text of any common field: reading the body refuses above this before
/// allocating, and validation checks again with the exact limit of each field.
pub const MAX_TEXT: usize = MAX_URL;

// ---- Secrets ----

/// A text secret (password, token, private key, passphrase…).
pub const MAX_SECRET: usize = 64 * KIB;
/// Value of an extra field (secret or not).
pub const MAX_EXTRA_VALUE: usize = MAX_SECRET;
/// Text of the secure note.
pub const MAX_NOTE: usize = MIB;
/// TOTP seed in base32: at least 80 bits (16 characters, D35).
pub const MIN_TOTP: usize = 16;
/// TOTP seed in base32: at most 128 characters.
pub const MAX_TOTP: usize = 128;

// ---- Lists ----

/// Tags per entry.
pub const MAX_TAGS: usize = 16;
/// Extra fields per entry.
pub const MAX_EXTRAS: usize = 64;
/// Attachments per entry.
pub const MAX_ATTACHMENTS: usize = 32;

// ---- Attachments and mask (D29) ----

/// Sum of the attachments of an entry (and so each attachment): 256 KiB. SSH key,
/// `.pem`, `.env`, `.ovpn` and kubeconfig stay far below it.
pub const MAX_ATTACHMENTS_TOTAL: usize = 256 * KIB;

/// Limit of the sum of the patterns of the built mask (D11). Above this the mask is not
/// built and the command does not run (fail closed).
pub const PATTERN_LIMIT: usize = 8 * MIB;

/// Mask pattern budget per entry with delivery (D29): half the limit, so that the two
/// entries of a combined use (jump or tunnel, D19) fit together.
pub const MASK_BUDGET: usize = PATTERN_LIMIT / 2;

/// Length, in characters, a value must have for the mask to be reliable (D11). It is
/// also the minimum of an attachment line and of the value of `KEY=value` (D34) to
/// become a pattern of its own.
pub const TRUSTED_MINIMUM: usize = 6;

// ---- Body (D26) ----

/// Largest decrypted body accepted, in bytes (provisional).
pub const MAX_BODY: usize = 64 * MIB;
/// Largest number of entries in a vault.
pub const MAX_ENTRIES: usize = 10_000;

// ---- Vault configuration (D30) ----

/// New master password (create the vault, change the password): 12 characters or more,
/// counted in code points after NFC (help text of the first-use master password design
/// screen; handoff 30; Phase 39c). Applies in the app and in the daemon; unlock does
/// not check it.
pub const MIN_MASTER_PASSWORD_CHARS: usize = 12;

/// PIN: from 4 to 6 digits (scope, approval section).
pub const MIN_PIN_DIGITS: usize = 4;
/// PIN: from 4 to 6 digits (scope, approval section).
pub const MAX_PIN_DIGITS: usize = 6;
/// PIN hash in PHC text (`$argon2id$v=19$m=…,t=…,p=…$<salt>$<hash>`), in bytes. Today's
/// hash has about 100; reading refuses above this before anything else.
pub const MAX_PIN_HASH: usize = 128;
/// Default inactivity lock timeout: 4 h (R23, D16).
pub const DEFAULT_LOCK_MINUTES: u32 = 240;
/// Smallest lock timeout accepted, in minutes. There is no "never lock".
pub const MIN_LOCK_MINUTES: u32 = 1;
/// Largest lock timeout accepted, in minutes: 24 h.
pub const MAX_LOCK_MINUTES: u32 = 24 * 60;
/// File system UUID of the external backup USB drive (D17): up to 36 characters (the
/// 128-bit UUID with hyphens; FAT and exFAT use `XXXX-XXXX`).
pub const MAX_BACKUP_UUID: usize = 36;
/// Largest creation or change date of an entry (D65 d), in UTC seconds since 1970:
/// 9999-12-31 23:59:59. Fits with room to spare in the JavaScript `Number` (2^53).
pub const MAX_DATE: u64 = 253_402_300_799;

// ---- Socket protocol (Phase 20) ----

/// Sum of the secret bytes an entry can have (note, two field secrets, the values of
/// the extras and the attachments summed). Sizes the app frame.
pub const MAX_ENTRY: usize =
    MAX_NOTE + 2 * MAX_SECRET + MAX_EXTRAS * MAX_EXTRA_VALUE + MAX_ATTACHMENTS_TOTAL;
/// Sum of the common texts of an entry (with room to spare: 32 fields at the text
/// limit).
pub const MAX_ENTRY_TEXTS: usize = 32 * MAX_TEXT
    + MAX_TAGS * MAX_TAG
    + MAX_EXTRAS * MAX_EXTRA_NAME
    + MAX_ATTACHMENTS * MAX_ATTACHMENT_NAME;
/// Largest frame of the common client (agent) and of the extension host (V3.4). It fits
/// `ward run` with the argv and the environment of the CLI; a 2 MiB frame is refused.
pub const MAX_COMMON_FRAME: usize = MIB;
/// Largest frame of the already identified app (D5, D29): the largest entry validation
/// accepts, with the secrets and the 256 KiB of attachments in base64. The protocol
/// module checks at compile time that it fits.
pub const MAX_APP_FRAME: usize = 8 * MIB;
/// Largest frame from the daemon to the client: the list of a full vault (the text of
/// the entries adds up to at most the body, and JSON escaping at most doubles it) with
/// room per entry.
pub const MAX_RESPONSE_FRAME: usize = 2 * MAX_BODY + MAX_ENTRIES * KIB;
/// One argument or environment variable value of `ward run` (Linux `MAX_ARG_STRLEN`).
pub const MAX_ARGUMENT: usize = 128 * KIB;
/// Arguments of the `ward run`/`ward ssh` command.
pub const MAX_ARGUMENTS: usize = 2048;
/// Environment variables the CLI passes on.
pub const MAX_ENV_VARIABLES: usize = 2048;
/// Text shown in the approval window (the exact command) and a chunk of output.
pub const MAX_DISPLAYED_TEXT: usize = 64 * KIB;

// Consistency between the limits, checked at compile time.
const _: () = {
    assert!(MAX_NAME <= MAX_TEXT);
    assert!(MAX_LINE <= MAX_TEXT);
    assert!(MAX_USERNAME <= MAX_TEXT);
    assert!(MAX_HOST <= MAX_TEXT);
    assert!(MAX_TAG <= MAX_TEXT);
    assert!(MAX_EXTRA_NAME <= MAX_TEXT);
    assert!(MAX_ATTACHMENT_NAME <= MAX_TEXT);
    assert!(MAX_ACCESS_KEY <= MAX_TEXT);
    assert!(MAX_REGION <= MAX_TEXT);
    assert!(MAX_DB_NAME <= MAX_TEXT);
    assert!(MIN_ACCESS_KEY <= MAX_ACCESS_KEY);
    assert!(MIN_TOTP <= MAX_TOTP);
    assert!(MAX_TOTP <= MAX_SECRET);
    assert!(MASK_BUDGET * 2 <= PATTERN_LIMIT);
    assert!(MIN_PIN_DIGITS <= MAX_PIN_DIGITS);
    assert!(MIN_LOCK_MINUTES <= DEFAULT_LOCK_MINUTES);
    assert!(DEFAULT_LOCK_MINUTES <= MAX_LOCK_MINUTES);
    assert!(MAX_PIN_HASH <= MAX_TEXT);
    assert!(MAX_BACKUP_UUID <= MAX_TEXT);
    assert!(MAX_COMMON_FRAME < MAX_APP_FRAME);
    assert!(MAX_RESPONSE_FRAME <= u32::MAX as usize);
    assert!(MAX_ARGUMENT < MAX_COMMON_FRAME);
    // The length of every field fits in the body's u32.
    assert!(MAX_BODY <= u32::MAX as usize);
};

#[cfg(test)]
mod tests;
