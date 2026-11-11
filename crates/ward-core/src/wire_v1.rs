//! Wire tests against the 0.1.4 baseline (m2:DT18, m2:V2.2; Phase 3 of mission 2).
//!
//! The fixtures in `dev/fixtures/legacy/` were produced by the code of the tag `v0.1.4`
//! (handoff of Phase 3) and are never regenerated:
//!
//! - `protocol-v1.jsonl`: one line per request (every operation), per response sample
//!   (every response type) and per simple error code, each one
//!   `{"kind":"<request|response|error>","role":"<role>","frame":<frame>}` with the
//!   frame exactly as the 0.1.4 code wrote it. Since Phase 50 (m2:DT8) the protocol is
//!   v2: every v1 request answers `unknown_version` whatever the role, and no v1
//!   response is accepted by a v2 client (m2:V4.13). The byte-for-byte reading and
//!   writing moved to `protocol-v2.jsonl` (`wire_v2`).
//! - `vault-0.1.4.ward`: the test vault (D15 entries, fake values only), body v4,
//!   generation 3. It opens with the test password; since Phase 48 (m2:DT4) the body v4
//!   is only read: its names, types and levels come back by the table of m2:DT4, and
//!   written again it gives a body v5 that reads back to the same vault.
//! - `backup-0.1.4/`: the two backup versions of the same vault (generations 2 and 3);
//!   the newest is the vault itself.
//!
//! Only fake values (D15). Nothing here prints a value: the comparisons are on bytes.

use crate::body;
use crate::format::{self, MasterPassword};
use crate::protocol::{self, Operation, Response, ResponseError, Role, VERSION};

const PROTOCOL_V1: &[u8] = include_bytes!("../../../dev/fixtures/legacy/protocol-v1.jsonl");
const VAULT: &[u8] = include_bytes!("../../../dev/fixtures/legacy/vault-0.1.4.ward");
const BACKUP_GEN_2: &[u8] =
    include_bytes!("../../../dev/fixtures/legacy/backup-0.1.4/ward-20261110-213053.ward");
const BACKUP_GEN_3: &[u8] =
    include_bytes!("../../../dev/fixtures/legacy/backup-0.1.4/ward-20261110-213054.ward");

/// Test master password (D15).
const TEST_PASSWORD: &str = "falso-mestra-ward-01";

/// Entries of the test vault as 0.1.4 wrote them: name, stored type name and stored
/// level name (body v4).
const ENTRIES_V4: [(&str, &str, &str); 22] = [
    ("acme-pg-aprovacao", "banco", "com-aprovacao"),
    ("acme-pg-dev", "banco", "livre"),
    ("acme-pg-prod", "banco", "com-aprovacao"),
    ("acme-pg-tunel", "banco", "livre"),
    ("acme-ssh-chave-prod", "servidor", "livre"),
    ("acme-ssh-interno", "servidor", "livre"),
    ("acme-ssh-interno-aprov", "servidor", "livre"),
    ("acme-ssh-salto", "servidor", "com-aprovacao"),
    ("acme-ssh-salto-livre", "servidor", "livre"),
    ("acme-ssh-senha-stage", "servidor", "livre"),
    ("acme-totp-dev", "login", "livre"),
    ("acme-wp-dev", "login", "com-aprovacao"),
    ("acme-wp-prod", "login", "livre"),
    ("beta-aws-dev", "aws", "livre"),
    ("beta-github-api", "api", "livre"),
    ("beta-sftp-dev", "ftp-sftp", "livre"),
    ("beta-smtp-stage", "smtp", "livre"),
    ("delta-api-simbolos", "api", "livre"),
    ("delta-redir", "login", "livre"),
    ("gama-bloqueada", "login", "bloqueado"),
    ("gama-generico", "generico", "livre"),
    ("gama-nota", "nota-segura", "livre"),
];

/// The v4 → v5 names of m2:DT4 that the test vault uses (types and levels), written
/// here by hand, independent of `body::legacy_v4`.
const V4_TO_V5: [(&str, &str); 12] = [
    ("login", "login"),
    ("banco", "database"),
    ("servidor", "server"),
    ("api", "api"),
    ("aws", "aws"),
    ("ftp-sftp", "ftp-sftp"),
    ("smtp", "smtp"),
    ("nota-segura", "secure-note"),
    ("generico", "generic"),
    ("bloqueado", "blocked"),
    ("com-aprovacao", "approval"),
    ("livre", "free"),
];

fn v5_name(v4: &str) -> &'static str {
    V4_TO_V5
        .iter()
        .find(|(n4, _)| *n4 == v4)
        .map(|(_, n5)| *n5)
        .expect("v4 name outside the table")
}

/// One line of `protocol-v1.jsonl`.
struct Line<'a> {
    kind: &'a str,
    /// The role of the line, by its v1 name.
    role: Role,
    frame: &'a [u8],
}

/// The v1 role names, written here by hand (the v2 names are other ones, m2:DT8).
fn v1_role(name: &str) -> Role {
    match name {
        "agente" => Role::Agent,
        "app" => Role::App,
        "extensao" => Role::Extension,
        _ => panic!("fixture role unknown"),
    }
}

/// Splits the fixed shape of a line. The fixture is ours (written by the generator of
/// Phase 3), so any other shape is a broken fixture.
fn parse_line(line: &[u8]) -> Line<'_> {
    let text = std::str::from_utf8(line).expect("fixture line is not UTF-8");
    let rest = text
        .strip_prefix(r#"{"kind":""#)
        .expect("fixture line without kind");
    let (kind, rest) = rest.split_once('"').expect("fixture kind not closed");
    let rest = rest
        .strip_prefix(r#","role":""#)
        .expect("fixture line without role");
    let (role, rest) = rest.split_once('"').expect("fixture role not closed");
    let frame = rest
        .strip_prefix(r#","frame":"#)
        .and_then(|f| f.strip_suffix('}'))
        .expect("fixture line without frame");
    Line {
        kind,
        role: v1_role(role),
        frame: frame.as_bytes(),
    }
}

fn lines() -> Vec<Line<'static>> {
    assert_eq!(
        PROTOCOL_V1.last(),
        Some(&b'\n'),
        "fixture must end with a newline"
    );
    PROTOCOL_V1[..PROTOCOL_V1.len() - 1]
        .split(|&b| b == b'\n')
        .map(parse_line)
        .collect()
}

#[test]
fn v1_fixture_is_version_1() {
    // Phase 50 moved the protocol to v2 on purpose (m2:DT8); the fixture stays v1.
    assert_eq!(VERSION, 2);
    for l in lines() {
        assert!(l.frame.starts_with(br#"{"v":1,"#));
    }
}

/// m2:V4.13 (S1, RM3): every v1 request answers `unknown_version`, for its own role and
/// for any other one, before the operation, the role or the fields are looked at.
#[test]
fn dt8_v1_requests_answer_unknown_version() {
    let mut count = 0;
    for l in lines().into_iter().filter(|l| l.kind == "request") {
        for role in Role::ALL {
            assert_eq!(
                protocol::decode_request(role, l.frame).unwrap_err(),
                ResponseError::UnknownVersion,
                "{}",
                String::from_utf8_lossy(l.frame)
            );
        }
        // The line's own role could ask for it in v1: the version comes first.
        assert_eq!(
            protocol::decode_request(l.role, l.frame).unwrap_err(),
            ResponseError::UnknownVersion
        );
        count += 1;
    }
    assert_eq!(count, Operation::ALL.len());
}

/// m2:V4.13, client side (m2:V4.16 in the CLI and the app): no v1 response or error is
/// accepted by a v2 client, whatever the role, and the version of the frame is readable
/// so the client can stop with the version message before sending the password.
#[test]
fn dt8_v1_responses_refused_by_the_v2_client() {
    let mut count = 0;
    for l in lines().into_iter().filter(|l| l.kind != "request") {
        for role in Role::ALL {
            assert!(protocol::decode_response(role, l.frame).is_err());
        }
        assert_eq!(protocol::frame_version(l.frame), Some(1));
        count += 1;
    }
    assert_eq!(count, 23 + ResponseError::SIMPLE.len());
    // Not JSON or without `v`: no version.
    assert_eq!(protocol::frame_version(b"{"), None);
    assert_eq!(protocol::frame_version(br#"{"tipo":"erro"}"#), None);
    assert_eq!(protocol::frame_version(br#"{"v":"1"}"#), None);
}

/// m2:DT8: the answer to a v1 frame is the v2 error with the fixed code and message
/// and nothing else (no data, no echo of the frame), for every role.
#[test]
fn dt8_unknown_version_error_has_no_data() {
    let expected = "{\"v\":2,\"type\":\"error\",\"data\":{\"code\":\"unknown_version\",\
                    \"message\":\"versão do protocolo incompatível: atualize o Ward\"}}";
    for role in Role::ALL {
        let q = protocol::encode_response(role, &Response::Error(ResponseError::UnknownVersion))
            .unwrap();
        assert_eq!(&q[..], expected.as_bytes());
    }
}

#[test]
fn v1_fixture_has_only_known_kinds() {
    let all = lines();
    assert_eq!(
        all.len(),
        Operation::ALL.len() + 23 + ResponseError::SIMPLE.len()
    );
    assert!(
        all.iter()
            .all(|l| matches!(l.kind, "request" | "response" | "error"))
    );
}

/// Opens a 0.1.4 file and returns the decrypted body (zeroed on drop).
fn body_of(file: &[u8]) -> zeroize::Zeroizing<Vec<u8>> {
    let password = MasterPassword::new(TEST_PASSWORD.to_owned());
    let (_keys, body) = format::open(file, &password).expect("0.1.4 file does not open");
    body
}

/// m2:DT4 (Phase 48): the 0.1.4 body is v4; it is read by the table of m2:DT4 (types and
/// levels as v5), and written again it is a body v5 that reads back to the same vault.
/// Only the version and the stored names change: the size differs exactly by the
/// renamed names.
#[test]
fn vault_014_body_v4_is_read_and_written_as_v5() {
    let body = body_of(VAULT);
    assert_eq!(body::BODY_VERSION, 5);
    assert_eq!(body::LEGACY_BODY_VERSION, 4);
    assert_eq!(u16::from_le_bytes([body[0], body[1]]), 4);
    let (config, generation, entries) = body::deserialize(&body).expect("body v4 refused");
    assert_eq!(generation, 3);
    let got: Vec<(&str, &str, &str)> = entries
        .iter()
        .map(|e| (e.name(), e.kind().name(), e.level().name()))
        .collect();
    let expected: Vec<(&str, &str, &str)> = ENTRIES_V4
        .iter()
        .map(|(name, kind, level)| (*name, v5_name(kind), v5_name(level)))
        .collect();
    assert_eq!(got, expected);
    assert_eq!(config.lock_minutes(), 240);
    assert!(!config.backup().enabled());
    assert!(config.verify_pin(b"135790"));

    let again = body::serialize(&config, generation, &entries).expect("body not written");
    assert_eq!(u16::from_le_bytes([again[0], again[1]]), 5);
    let delta: isize = ENTRIES_V4
        .iter()
        .map(|(_, kind, level)| {
            let size = |n: &str| n.len() as isize;
            size(v5_name(kind)) - size(kind) + size(v5_name(level)) - size(level)
        })
        .sum();
    assert_eq!(again.len() as isize, body.len() as isize + delta);
    // Outside the version and the names, the bytes are the same ones: the v4 body with
    // each stored name replaced (in its length-prefixed field) is the v5 body.
    let mut rebuilt = body.to_vec();
    rebuilt[..2].copy_from_slice(&5u16.to_le_bytes());
    for (v4, v5) in V4_TO_V5.iter().filter(|(a, b)| a != b) {
        rebuilt = replace_field(&rebuilt, v4.as_bytes(), v5.as_bytes());
    }
    assert!(
        rebuilt[..] == again[..],
        "v5 body differs from v4 beyond the names"
    );

    let (config2, generation2, entries2) = body::deserialize(&again).expect("body v5 refused");
    assert_eq!(generation2, 3);
    assert!(config2.pin_hash().bytes() == config.pin_hash().bytes());
    let got2: Vec<(&str, &str, &str)> = entries2
        .iter()
        .map(|e| (e.name(), e.kind().name(), e.level().name()))
        .collect();
    assert_eq!(got2, expected);
    let canonical = body::serialize(&config2, generation2, &entries2).expect("body not written");
    assert!(*canonical == *again, "body v5 not canonical");
}

/// Replaces every length-prefixed field equal to `from` by `to` (the u32 LE length plus
/// the bytes). The fake values of the test vault never have the exact bytes of a
/// prefixed stored name; `vault_014_body_v4_is_read_and_written_as_v5` checks the result
/// against the module output.
fn replace_field(body: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut pattern = (from.len() as u32).to_le_bytes().to_vec();
    pattern.extend_from_slice(from);
    let mut replacement = (to.len() as u32).to_le_bytes().to_vec();
    replacement.extend_from_slice(to);
    let mut out = Vec::with_capacity(body.len());
    let mut i = 0;
    while i < body.len() {
        if body[i..].starts_with(&pattern) {
            out.extend_from_slice(&replacement);
            i += pattern.len();
        } else {
            out.push(body[i]);
            i += 1;
        }
    }
    out
}

#[test]
fn backup_014_two_consecutive_generations() {
    assert_eq!(BACKUP_GEN_3, VAULT, "the newest backup is the vault itself");
    let older = body_of(BACKUP_GEN_2);
    let (config, generation, entries) = body::deserialize(&older).expect("backup refused");
    assert_eq!(generation, 2);
    // Same configuration and entries, one generation before: written at generation 3
    // they give the body of the vault (both written as v5 since m2:DT4).
    let again = body::serialize(&config, 3, &entries).expect("body not written");
    let newest = body_of(VAULT);
    let (config3, generation3, entries3) = body::deserialize(&newest).expect("vault refused");
    assert_eq!(generation3, 3);
    let vault_v5 = body::serialize(&config3, 3, &entries3).expect("body not written");
    assert!(*again == *vault_v5);
}

/// m2:V4.9: a vault v5 sealed now has the same header fields and sizes as the 0.1.4 one
/// (magic, format version 1, Argon2id ≥ 256 MiB / 3 passes, header size); only the
/// encrypted body changed version.
#[test]
fn vault_v5_header_same_as_014() {
    let password = MasterPassword::new(TEST_PASSWORD.to_owned());
    let old = format::read_header(VAULT).expect("0.1.4 header");
    let (config, generation, entries) = body::deserialize(&body_of(VAULT)).expect("body v4");
    let vault = format::OpenVault::new(&password, config).expect("new vault");
    let v5 = body::serialize(vault.config(), generation, &entries).expect("body v5");
    let file = vault.seal(&v5).expect("seal");
    let new = format::read_header(&file).expect("v5 header");
    assert_eq!(&file[..8], &VAULT[..8]);
    assert_eq!(&file[..8], &format::MAGIC);
    assert_eq!(new.version, old.version);
    assert_eq!(new.version, format::VERSION);
    assert_eq!(new.kdf, old.kdf);
    assert!(new.kdf.memory_kib >= 262_144 && new.kdf.passes >= 3);
    assert_eq!(new.body_size as usize, file.len() - format::HEADER_SIZE);
    assert_eq!(old.body_size as usize, VAULT.len() - format::HEADER_SIZE);
    let opened = body_of(&file);
    assert_eq!(u16::from_le_bytes([opened[0], opened[1]]), 5);
}

// ---- Attacks (S6, S2) ----

/// S6: one byte changed anywhere in the 0.1.4 file and it does not open.
#[test]
fn attack_vault_014_byte_flipped_does_not_open() {
    let password = MasterPassword::new(TEST_PASSWORD.to_owned());
    for pos in [0, 12, 100, VAULT.len() / 2, VAULT.len() - 1] {
        let mut file = VAULT.to_vec();
        file[pos] ^= 0x01;
        assert!(
            format::open(&file, &password).is_err(),
            "byte {pos} flipped opened"
        );
    }
    let wrong = MasterPassword::new("falso-mestra-ward-02".to_owned());
    assert!(format::open(VAULT, &wrong).is_err());
}

/// S2, m2:V1.7: the fixtures in git are sealed; no fake value of the test vault is in
/// them in clear.
#[test]
fn vault_and_backups_014_have_no_value_in_clear() {
    const VALUES: [&str; 12] = [
        TEST_PASSWORD,
        "falso-WP-senha-01",
        "falso-PG:s3nha/01@#",
        "falso-SSH-senha-01",
        "falso-passphrase-01",
        "falso_token_github_01",
        "falso-aws-secret-01",
        "falso-SMTP-01",
        "falso-nota-01",
        "falso-licenca-01",
        "falso-anexo-01",
        "JBSWY3DPEHPK3PXP",
    ];
    for file in [VAULT, BACKUP_GEN_2, BACKUP_GEN_3] {
        for v in VALUES {
            assert!(
                !file.windows(v.len()).any(|w| w == v.as_bytes()),
                "a fake value is in clear in a 0.1.4 file"
            );
        }
        assert!(!file.windows(5).any(|w| w == b"falso"));
        assert!(!file.windows(7).any(|w| w == b"OPENSSH"));
    }
}
