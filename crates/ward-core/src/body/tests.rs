//! Tests of the vault body (D26, D30): round trip (V2.30), tampering with the decrypted
//! body (V2.31), the configuration section of body v2 (V2.39) and write refusals. The
//! tests that need a global allocator (V2.32 with counting, V2.33 searching for a secret
//! in a freed block) are in the allocator integration test in `tests/`. The body v5 and
//! the reading of v4 (m2:DT4) are at the end. Only fake values (D15, D28).

use std::panic::{AssertUnwindSafe, catch_unwind};

use super::*;
use crate as wc;
use crate::entry::{AttachmentDraft, DataDraft, Environment, ExtraField, Login, SmtpSecurity};
use crate::format::KdfParams as PinKdfParams;

#[path = "test_vault.rs"]
mod test_vault;
use test_vault::*;

// ---- Helpers ----

/// The configuration of the test vault (hash of the test PIN, 240 minutes, backup
/// disabled), computed once.
fn cfg() -> &'static VaultConfig {
    test_config()
}

fn test_body() -> Zeroizing<Vec<u8>> {
    serialize(cfg(), 1, &test_vault_entries()).expect("o cofre de teste serializa")
}

/// Reads without letting a panic through: a panic becomes a test failure with its own message.
fn read_all(body: &[u8]) -> Result<(VaultConfig, Vec<Entry>), BodyError> {
    match catch_unwind(AssertUnwindSafe(|| deserialize(body))) {
        Ok(r) => r.map(|(c, _, e)| (c, e)),
        Err(_) => panic!("o decodificador entrou em pânico"),
    }
}

/// Only the entries (the configuration is checked in the V2.39 tests).
fn read(body: &[u8]) -> Result<Vec<Entry>, BodyError> {
    read_all(body).map(|(_, e)| e)
}

/// The body must be refused with the generic error, without the forged value in the error.
fn refused(body: &[u8], case: &str, value: &str) {
    match read(body) {
        Ok(e) => panic!("{case}: aceito com {} entradas", e.len()),
        Err(error) => {
            assert_eq!(error, BodyError::Invalid, "{case}");
            for text in [
                format!("{error}"),
                format!("{error:?}"),
                format!("{error:#?}"),
            ] {
                assert!(
                    !value.is_empty() && !text.contains(value),
                    "{case}: erro com valor"
                );
                assert!(!text.contains("falso"), "{case}: erro com segredo");
            }
        }
    }
}

fn same_secret(a: &Secret, b: &Secret) -> bool {
    a.expose() == b.expose()
}

fn same_optional(a: &Option<Secret>, b: &Option<Secret>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => same_secret(a, b),
        _ => false,
    }
}

fn same_auth(a: &Auth, b: &Auth) -> bool {
    match (a, b) {
        (Auth::Password(a), Auth::Password(b)) => same_secret(a, b),
        (
            Auth::Key {
                private_key: ca,
                passphrase: pa,
            },
            Auth::Key {
                private_key: cb,
                passphrase: pb,
            },
        ) => same_secret(ca, cb) && same_optional(pa, pb),
        _ => false,
    }
}

fn same_data(a: &Data, b: &Data) -> bool {
    match (a, b) {
        (Data::Login(a), Data::Login(b)) => {
            let Login {
                url,
                username,
                password,
                totp_seed,
            } = a;
            *url == b.url
                && *username == b.username
                && same_secret(password, &b.password)
                && same_optional(totp_seed, &b.totp_seed)
        }
        (Data::Database(a), Data::Database(b)) => {
            a.engine == b.engine
                && a.host == b.host
                && a.port == b.port
                && a.db_name == b.db_name
                && a.username == b.username
                && same_secret(&a.password, &b.password)
                && a.tunnel == b.tunnel
        }
        (Data::Server(a), Data::Server(b)) => {
            a.host == b.host
                && a.port == b.port
                && a.username == b.username
                && same_auth(&a.auth, &b.auth)
                && a.jump == b.jump
        }
        (Data::Api(a), Data::Api(b)) => {
            a.url == b.url && same_secret(&a.token, &b.token) && a.variable == b.variable
        }
        (Data::Aws(a), Data::Aws(b)) => {
            a.access_key_id == b.access_key_id
                && same_secret(&a.secret_access_key, &b.secret_access_key)
                && a.region == b.region
                && a.role_arn == b.role_arn
                && same_optional(&a.session_token, &b.session_token)
        }
        (Data::Smtp(a), Data::Smtp(b)) => {
            a.host == b.host
                && a.port == b.port
                && a.username == b.username
                && same_secret(&a.password, &b.password)
                && a.security == b.security
        }
        (Data::FtpSftp(a), Data::FtpSftp(b)) => {
            a.protocol == b.protocol
                && a.host == b.host
                && a.port == b.port
                && a.username == b.username
                && same_auth(&a.auth, &b.auth)
        }
        (Data::SecureNote(a), Data::SecureNote(b)) => same_secret(&a.text, &b.text),
        (Data::Generic, Data::Generic) => true,
        _ => false,
    }
}

fn same_extra(a: &ExtraField, b: &ExtraField) -> bool {
    a.name() == b.name()
        && match (a.value(), b.value()) {
            (ExtraValue::Text(x), ExtraValue::Text(y)) => x == y,
            (ExtraValue::Secret(x), ExtraValue::Secret(y)) => same_secret(x, y),
            _ => false,
        }
}

/// Field by field; secrets compared by their bytes, never printing them.
fn check_equal(a: &Entry, b: &Entry) {
    let name = a.name();
    assert_eq!(a.name(), b.name());
    assert_eq!(a.kind(), b.kind(), "{name}: tipo");
    assert_eq!(a.level(), b.level(), "{name}: nível");
    assert_eq!(a.group(), b.group(), "{name}: grupo");
    assert_eq!(a.environment(), b.environment(), "{name}: ambiente");
    assert_eq!(a.project(), b.project(), "{name}: projeto");
    assert_eq!(a.tags(), b.tags(), "{name}: tags");
    assert_eq!(a.dates(), b.dates(), "{name}: datas");
    assert!(same_data(a.data(), b.data()), "{name}: dados");
    assert_eq!(a.extras().len(), b.extras().len(), "{name}: extras");
    for (x, y) in a.extras().iter().zip(b.extras()) {
        assert!(same_extra(x, y), "{name}: extra {}", x.name());
    }
    assert_eq!(
        a.attachments().len(),
        b.attachments().len(),
        "{name}: anexos"
    );
    for (x, y) in a.attachments().iter().zip(b.attachments()) {
        assert_eq!(x.name(), y.name(), "{name}: anexo");
        assert!(
            same_secret(x.content(), y.content()),
            "{name}: conteúdo do anexo {}",
            x.name()
        );
    }
}

fn check_round_trip(originals: &[Entry], returned: &[Entry]) {
    assert_eq!(originals.len(), returned.len());
    let mut order: Vec<&Entry> = originals.iter().collect();
    order.sort_by(|a, b| a.name().cmp(b.name()));
    for (a, b) in order.iter().zip(returned) {
        check_equal(a, b);
    }
}

/// What the decoder accepted must be a valid vault: the configuration within the rules,
/// each entry checks against all the others (the full, quadratic check, without the
/// module index) and serializing again gives the same bytes.
fn check_accepted(body: &[u8], config: &VaultConfig, entries: &[Entry]) {
    // D39: the accepted generation is ≥ 1 and comes back equal (it sits right after the version).
    let generation = deserialize(body).map(|(_, g, _)| g).unwrap();
    assert!(generation >= 1, "aceitou geração 0");
    assert!(config::valid_minutes(config.lock_minutes()));
    assert!(config::verify_text(config.pin_hash().bytes()));
    assert!(!config.backup().enabled() || config.backup().uuid().is_some());
    for (i, e) in entries.iter().enumerate() {
        let others = entries
            .iter()
            .enumerate()
            .filter(move |(j, _)| *j != i)
            .map(|(_, o)| o);
        assert!(
            e.verify_in_vault(others).is_ok(),
            "aceitou entrada que não confere no cofre"
        );
    }
    let again = serialize(config, generation, entries).expect("o aceito serializa");
    if body[..2] == LEGACY_BODY_VERSION.to_le_bytes() {
        // m2:DT4: an accepted v4 body is written as v5, and the v5 form reads back to
        // the same entries.
        assert!(
            again[..2] == BODY_VERSION.to_le_bytes(),
            "v4 aceito gravado como v4"
        );
        let (_, g, back) = deserialize(&again).expect("a forma v5 do v4 aceito abre");
        assert_eq!(g, generation);
        check_round_trip(entries, &back);
    } else {
        assert!(again[..] == *body, "aceito não é canônico");
    }
}

// ---- Raw body, for forging ----

/// An entry as raw bytes, to forge fields that validation would never let exist in an
/// [`Entry`].
#[derive(Clone)]
struct RawRecord {
    kind: Vec<u8>,
    level: Vec<u8>,
    name: Vec<u8>,
    group: Vec<u8>,
    environment: Vec<u8>,
    project: Vec<u8>,
    tags: Vec<Vec<u8>>,
    data: Vec<Vec<u8>>,
    extras: Vec<(Vec<u8>, u8, Vec<u8>)>,
    attachments: Vec<(Vec<u8>, Vec<u8>)>,
    /// D65 d: creation and modification, 0 = none.
    created: u64,
    modified: u64,
}

/// The configuration section as raw bytes, for forging.
#[derive(Clone)]
struct RawCfg {
    /// The generation (D39); `None` builds the section without it, as in body v2.
    generation: Option<u64>,
    pin_hash: Vec<u8>,
    minutes: u32,
    enabled: u8,
    uuid: Vec<u8>,
}

impl RawCfg {
    /// The configuration of the test vault.
    fn standard() -> Self {
        Self::of(cfg())
    }

    fn of(c: &VaultConfig) -> Self {
        Self {
            generation: Some(1),
            pin_hash: c.pin_hash().bytes().to_vec(),
            minutes: c.lock_minutes(),
            enabled: u8::from(c.backup().enabled()),
            uuid: c.backup().uuid().unwrap_or("").as_bytes().to_vec(),
        }
    }
}

/// Reads a well-formed body into raw entries (through the module reader).
fn raw_records(body: &[u8]) -> Vec<RawRecord> {
    let mut l = Reader { rest: body };
    let (version, _, n) = read_header(&mut l).unwrap();
    assert_eq!(version, Version::V5, "raw_records lê só corpo v5");
    (0..n)
        .map(|_| {
            let b = read_raw(&mut l, version).unwrap();
            let mut lt = Reader { rest: b.tags.bytes };
            let mut le = Reader {
                rest: b.extras.bytes,
            };
            let mut la = Reader {
                rest: b.attachments.bytes,
            };
            RawRecord {
                kind: b.kind.name().as_bytes().to_vec(),
                level: b.level.name().as_bytes().to_vec(),
                name: b.name.to_vec(),
                group: b.group.to_vec(),
                environment: b.environment.to_vec(),
                project: b.project.to_vec(),
                tags: (0..b.tags.n)
                    .map(|_| read_tag(&mut lt).unwrap().to_vec())
                    .collect(),
                data: b.data[..schema(b.kind).len()]
                    .iter()
                    .map(|d| d.to_vec())
                    .collect(),
                extras: (0..b.extras.n)
                    .map(|_| {
                        let (n, s, v) = read_extra(&mut le).unwrap();
                        (n.to_vec(), u8::from(s), v.to_vec())
                    })
                    .collect(),
                attachments: (0..b.attachments.n)
                    .map(|_| {
                        let (n, c) = read_attachment(&mut la).unwrap();
                        (n.to_vec(), c.to_vec())
                    })
                    .collect(),
                created: b.dates.created_at().unwrap_or(0),
                modified: b.dates.modified_at().unwrap_or(0),
            }
        })
        .collect()
}

/// Writer independent of the module (the format at the top of `body.rs`, by hand), with
/// the configuration of the test vault.
fn forge_with(version: u16, n: u32, entries: &[RawRecord]) -> Vec<u8> {
    forge_cfg(version, Some(&RawCfg::standard()), n, entries)
}

/// Independent writer, with the given configuration section (or without it, `None`).
fn forge_cfg(version: u16, cfg: Option<&RawCfg>, n: u32, entries: &[RawRecord]) -> Vec<u8> {
    forge_raw(version, cfg, n, entries, true)
}

/// Independent writer; `with_dates: false` builds the entries as in body v3, without
/// the 16 bytes of the dates (D65 d).
fn forge_raw(
    version: u16,
    cfg: Option<&RawCfg>,
    n: u32,
    entries: &[RawRecord],
    with_dates: bool,
) -> Vec<u8> {
    fn u32le(v: &mut Vec<u8>, n: usize) {
        v.extend_from_slice(&(n as u32).to_le_bytes());
    }
    fn field(v: &mut Vec<u8>, b: &[u8]) {
        u32le(v, b.len());
        v.extend_from_slice(b);
    }
    let mut v = Vec::new();
    v.extend_from_slice(&version.to_le_bytes());
    if let Some(c) = cfg {
        if let Some(g) = c.generation {
            v.extend_from_slice(&g.to_le_bytes());
        }
        field(&mut v, &c.pin_hash);
        v.extend_from_slice(&c.minutes.to_le_bytes());
        v.push(c.enabled);
        field(&mut v, &c.uuid);
    }
    v.extend_from_slice(&n.to_le_bytes());
    for e in entries {
        for c in [
            &e.kind,
            &e.level,
            &e.name,
            &e.group,
            &e.environment,
            &e.project,
        ] {
            field(&mut v, c);
        }
        u32le(&mut v, e.tags.len());
        for tag in &e.tags {
            field(&mut v, tag);
        }
        for d in &e.data {
            field(&mut v, d);
        }
        u32le(&mut v, e.extras.len());
        for (name, s, value) in &e.extras {
            field(&mut v, name);
            v.push(*s);
            field(&mut v, value);
        }
        u32le(&mut v, e.attachments.len());
        for (name, content) in &e.attachments {
            field(&mut v, name);
            field(&mut v, content);
        }
        if with_dates {
            v.extend_from_slice(&e.created.to_le_bytes());
            v.extend_from_slice(&e.modified.to_le_bytes());
        }
    }
    v
}

fn forge(entries: &[RawRecord]) -> Vec<u8> {
    forge_with(BODY_VERSION, entries.len() as u32, entries)
}

fn index(raw_records: &[RawRecord], name: &str) -> usize {
    raw_records
        .iter()
        .position(|c| c.name == name.as_bytes())
        .unwrap()
}

/// Pseudo-random generator only for the mutations (fixed seed, reproducible). Not
/// crypto and never leaves the tests.
struct SplitMix(u64);

impl SplitMix {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

// ---- V2.30: round trip ----

#[test]
fn v2_30_round_trip_of_the_test_vault() {
    let mut originals = test_vault_entries();
    assert_eq!(originals.len(), 22);
    // D65 d: dates in the three forms (both, modification only, none).
    originals[0].set_dates(Dates::new_stamp(1_762_000_000).unwrap());
    originals[1].set_dates(
        Dates::new_stamp(1_762_000_000)
            .unwrap()
            .modified(1_762_500_000)
            .unwrap(),
    );
    originals[2].set_dates(Dates::NONE.modified(MAX_DATE).unwrap());
    let body = serialize(cfg(), 1, &originals).unwrap();
    let returned = read(&body).unwrap();
    check_round_trip(&originals, &returned);

    // The three levels of D15 come back as they were.
    let level = |name: &str| {
        returned
            .iter()
            .find(|e| e.name() == name)
            .map(Entry::level)
            .unwrap()
    };
    assert_eq!(level("gama-bloqueada"), AccessLevel::Blocked);
    assert_eq!(level("acme-pg-aprovacao"), AccessLevel::Approval);
    assert_eq!(level("acme-pg-dev"), AccessLevel::Free);
    assert_eq!(level("acme-totp-dev"), AccessLevel::Free);

    // Canonical: again gives the same bytes, and the input order does not matter.
    assert!(serialize(cfg(), 1, &returned).unwrap()[..] == body[..]);
    let mut reversed = read(&body).unwrap();
    reversed.reverse();
    assert!(serialize(cfg(), 1, &reversed).unwrap()[..] == body[..]);
    // They go out in name order.
    assert!(returned.windows(2).all(|w| w[0].name() < w[1].name()));
    // The tests' independent writer reaches the same bytes.
    assert!(forge(&raw_records(&body)) == body[..]);
}

#[test]
fn v2_30_one_of_each_type_at_the_model_maximum() {
    let originals = one_of_each_full();
    for kind in EntryType::ALL {
        assert!(originals.iter().any(|e| e.kind() == kind), "falta {kind}");
    }
    for e in &originals {
        assert_eq!(e.tags().len(), 16);
        assert_eq!(e.extras().len(), 64);
        assert!(e.project().is_some());
    }
    let body = serialize(cfg(), 1, &originals).unwrap();
    let returned = read(&body).unwrap();
    check_round_trip(&originals, &returned);
    assert!(serialize(cfg(), 1, &returned).unwrap()[..] == body[..]);
}

#[test]
fn v2_30_empty_vault() {
    // Only the configuration: version 5 (m2:DT4), generation 1, PIN hash, 240 minutes,
    // backup disabled without UUID, zero entries.
    let body = serialize(cfg(), 1, &[]).unwrap();
    let hash = cfg().pin_hash().bytes();
    let mut expected = vec![5, 0];
    expected.extend_from_slice(&1u64.to_le_bytes());
    expected.extend_from_slice(&(hash.len() as u32).to_le_bytes());
    expected.extend_from_slice(hash);
    expected.extend_from_slice(&240u32.to_le_bytes());
    expected.extend_from_slice(&[0, 0, 0, 0, 0]);
    expected.extend_from_slice(&[0, 0, 0, 0]);
    assert!(body[..] == expected[..]);
    let (c, entries) = read_all(&body).unwrap();
    assert!(entries.is_empty());
    assert!(c.verify_pin(TEST_PIN.as_bytes()));
    assert!(serialize(&c, 1, &[]).unwrap()[..] == body[..]);
}

/// Test directory in `target/<profile>/ward-test-body/`, removed at the end.
#[cfg(unix)]
struct Dir(std::path::PathBuf);

#[cfg(unix)]
impl Dir {
    fn new(name: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let exe = std::env::current_exe().unwrap();
        let base = exe
            .parent()
            .and_then(std::path::Path::parent)
            .unwrap()
            .join("ward-test-body");
        std::fs::create_dir_all(&base).unwrap();
        let mut rand_bytes = [0u8; 8];
        crate::crypto::random(&mut rand_bytes).unwrap();
        let d = base.join(format!(
            "{name}-{}-{}",
            std::process::id(),
            u64::from_le_bytes(rand_bytes)
        ));
        std::fs::create_dir(&d).unwrap();
        std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self(d)
    }
}

#[cfg(unix)]
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
#[test]
fn v2_30_round_trip_through_disk_with_attachments_at_the_limit() {
    use crate::atomic_write::write_entries;

    let dir = Dir::new("round-trip");
    let destination = dir.0.join("cofre.ward");
    let password = MasterPassword::new(MASTER_PASSWORD.to_owned());
    let mut vault = OpenVault::new(&password, new_config()).unwrap();

    // 22 from the test vault + one full entry of each type, each with attachments that add
    // up to the D29 limit (256 KiB).
    let mut originals = test_vault_entries();
    originals.extend(one_of_each_full());
    for e in originals
        .iter()
        .filter(|e| e.name().starts_with("zz-cheio-") && e.name() != "zz-cheio-api-token")
    {
        let sum: usize = e.attachments().iter().map(|a| a.content().len()).sum();
        assert_eq!(sum, crate::limits::MAX_ATTACHMENTS_TOTAL);
    }
    let expected = serialize(vault.config(), 1, &originals).unwrap();
    assert!(
        expected.len() > 8 * crate::limits::MAX_ATTACHMENTS_TOTAL && expected.len() <= MAX_BODY
    );

    write_entries(&destination, &mut vault, &originals).unwrap();
    let file = std::fs::read(&destination).unwrap();
    let hash = vault.config().pin_hash().bytes().to_vec();
    let hash = &hash[..];
    let markers = SECRETS
        .iter()
        .filter(|m| **m != MASTER_PASSWORD)
        .map(|m| m.as_bytes())
        .chain([TEST_PIN.as_bytes(), hash, &hash[..9]]);
    for marker in markers {
        assert!(
            !file.windows(marker.len()).any(|j| j == marker),
            "segredo em claro no arquivo"
        );
    }
    let (reopened, returned) = open_entries(&file, &password).unwrap();
    check_round_trip(&originals, &returned);
    // The configuration comes back equal (D30), and the test PIN checks against the hash read.
    assert!(serialize(reopened.config(), 1, &returned).unwrap()[..] == expected[..]);
    assert!(reopened.config().pin_hash().bytes() == hash);
    assert!(reopened.config().verify_pin(TEST_PIN.as_bytes()));

    // Empty vault (configuration only) also round-trips through the disk.
    write_entries(&destination, &mut vault, &[]).unwrap();
    let file = std::fs::read(&destination).unwrap();
    let (reopened, returned) = open_entries(&file, &password).unwrap();
    assert!(returned.is_empty());
    assert!(reopened.config().pin_hash().bytes() == hash);

    // Wrong password: a format error, not a body error.
    let wrong = MasterPassword::new("falso-mestra-ward-02".to_owned());
    assert_eq!(
        open_entries(&file, &wrong).unwrap_err(),
        OpenError::Format(FormatError::PasswordOrFile)
    );
}

#[cfg(unix)]
#[test]
fn password_change_seals_and_writes_through_the_same_path() {
    use crate::atomic_write::{WriteError, change_password, write_entries};

    let dir = Dir::new("change");
    let destination = dir.0.join("cofre.ward");
    let current = MasterPassword::new(MASTER_PASSWORD.to_owned());
    let new = MasterPassword::new("falso-mestra-ward-02".to_owned());
    let mut vault = OpenVault::new(&current, new_config()).unwrap();
    let hash = vault.config().pin_hash().bytes().to_vec();
    let originals = test_vault_entries();
    write_entries(&destination, &mut vault, &originals).unwrap();
    let before = std::fs::read(&destination).unwrap();

    // Wrong current password: refuses and does not touch the file.
    assert_eq!(
        change_password(&destination, &new, &new, 0, 0).unwrap_err(),
        WriteError::Format(FormatError::PasswordOrFile)
    );
    assert!(std::fs::read(&destination).unwrap() == before);

    let (mut new_vault, _) = change_password(&destination, &current, &new, 0, 0).unwrap();
    // The password change keeps the configuration (the same PIN hash).
    assert!(new_vault.config().pin_hash().bytes() == &hash[..]);
    let after = std::fs::read(&destination).unwrap();
    assert!(open_entries(&after, &current).is_err());
    let (_, returned) = open_entries(&after, &new).unwrap();
    check_round_trip(&originals, &returned);
    // The returned vault writes with the new password.
    write_entries(&destination, &mut new_vault, &returned).unwrap();
    let (_, again) = open_entries(&std::fs::read(&destination).unwrap(), &new).unwrap();
    check_round_trip(&originals, &again);
}

#[test]
fn known_format_of_one_entry() {
    // A generic entry written by hand, byte by byte, as in the module design: version 5,
    // configuration (generation 7, hash, 240 minutes, backup disabled, no UUID), 1 entry,
    // with the dates at the end (D65 d).
    let v = one_generic_entry(5, b"generic", b"free");

    let entries = read(&v).unwrap();
    check_one_generic_entry(&entries);
    assert_eq!(deserialize(&v).map(|(_, g, _)| g), Ok(7));
    assert!(serialize(cfg(), 7, &entries).unwrap()[..] == v[..]);
}

/// m2:DT4, m2:V4.8: the same entry written by hand as a body v4 (0.1.x names) reads as
/// the same entry, and is written again as exactly the v5 bytes above: only the version
/// and the two stored names change.
#[test]
fn known_format_of_one_entry_v4_is_written_as_v5() {
    let v4 = one_generic_entry(4, b"generico", b"livre");
    let v5 = one_generic_entry(5, b"generic", b"free");
    let entries = read(&v4).unwrap();
    check_one_generic_entry(&entries);
    assert_eq!(deserialize(&v4).map(|(_, g, _)| g), Ok(7));
    let written = serialize(cfg(), 7, &entries).unwrap();
    assert!(
        written[..] == v5[..],
        "v4 não foi gravado como o v5 esperado"
    );
    // Crossed names are refused: v5 names in a v4 body and v4 names in a v5 body.
    refused(
        &one_generic_entry(4, b"generic", b"free"),
        "v4 com nomes v5",
        "generic",
    );
    refused(
        &one_generic_entry(4, b"generico", b"free"),
        "v4 com nível v5",
        "free",
    );
    refused(
        &one_generic_entry(4, b"generic", b"livre"),
        "v4 com tipo v5",
        "generic",
    );
    refused(
        &one_generic_entry(5, b"generico", b"livre"),
        "v5 com nomes v4",
        "generico",
    );
    refused(
        &one_generic_entry(5, b"generic", b"livre"),
        "v5 com nível v4",
        "livre",
    );
    refused(
        &one_generic_entry(5, b"generico", b"free"),
        "v5 com tipo v4",
        "generico",
    );
    // Other versions around them stay unknown.
    for version in [3u16, 6] {
        refused(
            &one_generic_entry(version, b"generic", b"free"),
            "versão vizinha",
            "x-1",
        );
        refused(
            &one_generic_entry(version, b"generico", b"livre"),
            "versão vizinha",
            "x-1",
        );
    }
}

/// The body of [`known_format_of_one_entry`], with the version and the stored type and
/// level names given (written by hand, independent of the module).
fn one_generic_entry(version: u16, kind: &[u8], level: &[u8]) -> Vec<u8> {
    let mut v: Vec<u8> = version.to_le_bytes().to_vec();
    v.extend_from_slice(&7u64.to_le_bytes());
    let field = |v: &mut Vec<u8>, b: &[u8]| {
        v.extend_from_slice(&(b.len() as u32).to_le_bytes());
        v.extend_from_slice(b);
    };
    field(&mut v, cfg().pin_hash().bytes());
    v.extend_from_slice(&240u32.to_le_bytes());
    v.push(0);
    field(&mut v, b"");
    v.extend_from_slice(&1u32.to_le_bytes());
    field(&mut v, kind);
    field(&mut v, level);
    field(&mut v, b"x-1");
    field(&mut v, b"grupo");
    field(&mut v, b"dev");
    field(&mut v, b"");
    v.extend_from_slice(&1u32.to_le_bytes());
    field(&mut v, b"tag");
    v.extend_from_slice(&1u32.to_le_bytes());
    field(&mut v, b"CAMPO");
    v.push(0);
    field(&mut v, b"valor");
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&1_762_000_000u64.to_le_bytes());
    v.extend_from_slice(&1_762_086_400u64.to_le_bytes());
    v
}

fn check_one_generic_entry(entries: &[Entry]) {
    assert_eq!(entries.len(), 1);
    let e = &entries[0];
    assert_eq!(e.name(), "x-1");
    assert_eq!(e.kind(), EntryType::Generic);
    assert_eq!(e.level(), AccessLevel::Free);
    assert_eq!(e.environment(), Environment::Dev);
    assert_eq!(e.project(), None);
    assert_eq!(e.tags(), ["tag".to_owned()]);
    assert!(matches!(e.extras()[0].value(), ExtraValue::Text(t) if t == "valor"));
    assert_eq!(e.dates().created_at(), Some(1_762_000_000));
    assert_eq!(e.dates().modified_at(), Some(1_762_086_400));
}

// ---- Write refusals ----

#[test]
fn serialize_refuses_vault_that_would_not_reopen() {
    // Repeated name.
    let mut entries = test_vault_entries();
    entries.extend(test_vault_entries().into_iter().take(1));
    assert_eq!(
        serialize(cfg(), 1, &entries).unwrap_err(),
        BodyError::Inconsistent
    );

    // Tunnel to a server that is not in the vault.
    let entries: Vec<Entry> = test_vault_entries()
        .into_iter()
        .filter(|e| e.name() != "acme-ssh-senha-stage")
        .collect();
    assert_eq!(
        serialize(cfg(), 1, &entries).unwrap_err(),
        BodyError::Inconsistent
    );

    // Jump to a server that has a jump (chain).
    let mut entries: Vec<Entry> = test_vault_entries()
        .into_iter()
        .filter(|e| e.name() != "acme-ssh-salto")
        .collect();
    let mut r = Draft {
        name: "acme-ssh-salto".to_owned(),
        group: "acme".to_owned(),
        environment: "prod".to_owned(),
        project: String::new(),
        tags: Vec::new(),
        data: DataDraft::Server {
            host: "127.0.0.1".to_owned(),
            port: "2224".to_owned(),
            username: "salto".to_owned(),
            password: Some(Secret::from_text("falso-SALTO-01".to_owned())),
            private_key: None,
            passphrase: None,
            jump: "acme-ssh-senha-stage".to_owned(),
        },
        extras: Vec::new(),
        attachments: Vec::new(),
    };
    r.tags.push("salto".to_owned());
    entries.push(validated(r, AccessLevel::Approval));
    assert_eq!(
        serialize(cfg(), 1, &entries).unwrap_err(),
        BodyError::Inconsistent
    );
}

#[test]
fn serialize_refuses_more_than_10_000_entries() {
    let entries: Vec<Entry> = (0..=MAX_ENTRIES)
        .map(|i| {
            let mut r = Draft {
                name: format!("e{i:05}"),
                group: "g".to_owned(),
                environment: "dev".to_owned(),
                project: String::new(),
                tags: Vec::new(),
                data: DataDraft::Generic,
                extras: Vec::new(),
                attachments: Vec::new(),
            };
            r.extras.push(crate::entry::ExtraDraft {
                name: "A".to_owned(),
                value: Secret::from_text("v".to_owned()),
                secret: false,
            });
            validated(r, AccessLevel::Free)
        })
        .collect();
    assert_eq!(
        serialize(cfg(), 1, &entries).unwrap_err(),
        BodyError::TooLarge
    );
    // 10 000 fit and come back.
    let body = serialize(cfg(), 1, &entries[..MAX_ENTRIES]).unwrap();
    assert_eq!(read(&body).unwrap().len(), MAX_ENTRIES);
}

#[test]
fn v2_30_empty_attachment_refused_on_save_and_in_the_body() {
    // D29: an empty attachment is refused on save (it never becomes an `Entry`) and in the
    // forged body.
    let mut r = Draft {
        name: "anexo-vazio".to_owned(),
        group: "g".to_owned(),
        environment: "dev".to_owned(),
        project: String::new(),
        tags: Vec::new(),
        data: DataDraft::Generic,
        extras: Vec::new(),
        attachments: Vec::new(),
    };
    r.attachments.push(AttachmentDraft {
        name: "vazio.txt".to_owned(),
        content: Secret::from_bytes(Vec::new()),
    });
    assert!(r.validate().is_err());

    let body = test_body();
    let mut c = raw_records(&body);
    let k = index(&c, "gama-generico");
    c[k].attachments[0].1.clear();
    refused(&forge(&c), "anexo vazio", "FALSO_ENV");
}

// ---- V2.31: tampered body ----

#[test]
fn v2_31_truncated_at_every_size_and_extra_byte() {
    let body = test_body();
    for n in 0..body.len() {
        refused(&body[..n], "truncado", "falso");
    }
    for extra in [0x00u8, 0x01, 0xff] {
        let mut v = body.to_vec();
        v.push(extra);
        refused(&v, "byte a mais", "falso");
    }
    assert!(read(&body).is_ok());
}

#[test]
fn v2_31_forged_fields() {
    let body = test_body();
    let base = raw_records(&body);
    assert!(forge(&base) == body[..]);
    let n = base.len() as u32;

    // (case, forged body, value that must not appear in the error)
    let mut cases: Vec<(&str, Vec<u8>, &str)> = Vec::new();
    // Version 1 (entries only, Phase 15a) is unknown since D30, version 2 (no generation)
    // since D39, and version 3 (no dates) since D65 d. Version 6 does not exist yet.
    for version in [0u16, 1, 2, 3, 6, 0xffff] {
        cases.push((
            "versão desconhecida",
            forge_with(version, n, &base),
            "versão",
        ));
    }
    // m2:DT4: the v5 body relabeled as v4 keeps the v5 names, which the v4 table does
    // not know (m2:V4.8).
    cases.push((
        "v5 rotulado 4",
        forge_with(LEGACY_BODY_VERSION, n, &base),
        "approval",
    ));
    cases.push((
        "contagem a mais",
        forge_with(BODY_VERSION, n + 1, &base),
        "contagem",
    ));
    cases.push((
        "contagem a menos",
        forge_with(BODY_VERSION, n - 1, &base),
        "contagem",
    ));

    let mut with = |case: &'static str, value: &'static str, f: &dyn Fn(&mut Vec<RawRecord>)| {
        let mut c = base.clone();
        f(&mut c);
        cases.push((case, forge(&c), value));
    };
    with("tipo fora da lista", "customizado", &|c| {
        c[0].kind = b"customizado".to_vec()
    });
    with("tipo trocado", "database", &|c| {
        let k = index(c, "beta-github-api");
        c[k].kind = b"database".to_vec();
    });
    for level in ["admin", "", "Free", "free ", "Livre", "livre "] {
        with("nível fora da lista", "admin", &move |c| {
            c[1].level = level.as_bytes().to_vec()
        });
    }
    // m2:DT4, m2:V4.8: the v4 (0.1.x) names inside a v5 body are unknown.
    for kind in ["banco", "servidor", "nota-segura", "generico"] {
        with("tipo v4 no corpo v5", "banco", &move |c| {
            c[0].kind = kind.as_bytes().to_vec()
        });
    }
    for level in ["bloqueado", "com-aprovacao", "livre"] {
        with("nível v4 no corpo v5", "bloqueado", &move |c| {
            c[1].level = level.as_bytes().to_vec()
        });
    }
    with("segurança SMTP v4 no corpo v5", "nenhuma", &|c| {
        let k = index(c, "beta-smtp-stage");
        c[k].data[SMTP_SECURITY] = b"nenhuma".to_vec();
    });
    with("nome repetido", "acme-pg-dev", &|c| {
        let k = index(c, "acme-pg-prod");
        c[k].name = b"acme-pg-dev".to_vec();
    });
    with("nome repetido seguido", "acme-pg-aprovacao", &|c| {
        let k = index(c, "acme-pg-aprovacao");
        let copy = c[k].clone();
        c.insert(k + 1, copy);
    });
    with("fora de ordem", "acme", &|c| c.swap(0, 1));
    with("salto inexistente", "nao-existe", &|c| {
        let k = index(c, "acme-ssh-interno");
        c[k].data[6] = b"nao-existe".to_vec();
    });
    with(
        "salto para servidor com salto",
        "acme-ssh-interno-aprov",
        &|c| {
            let k = index(c, "acme-ssh-interno");
            c[k].data[6] = b"acme-ssh-interno-aprov".to_vec();
        },
    );
    with("salto para si mesmo", "acme-ssh-interno", &|c| {
        let k = index(c, "acme-ssh-interno");
        c[k].data[6] = b"acme-ssh-interno".to_vec();
    });
    with(
        "servidor usado como salto ganha salto",
        "acme-ssh-senha-stage",
        &|c| {
            let k = index(c, "acme-ssh-salto");
            c[k].data[6] = b"acme-ssh-senha-stage".to_vec();
        },
    );
    with(
        "túnel para entrada que não é servidor",
        "acme-pg-dev",
        &|c| {
            let k = index(c, "acme-pg-tunel");
            c[k].data[6] = b"acme-pg-dev".to_vec();
        },
    );
    with("-oProxyCommand no host", "ProxyCommand", &|c| {
        let k = index(c, "acme-ssh-senha-stage");
        c[k].data[0] = b"-oProxyCommand=sh -c id".to_vec();
    });
    with("-J no usuário", "-Jx", &|c| {
        let k = index(c, "acme-ssh-senha-stage");
        c[k].data[2] = b"-Jx".to_vec();
    });
    with("U+202E no grupo", "\u{202E}", &|c| {
        c[2].group = "ac\u{202E}me".as_bytes().to_vec()
    });
    with("nulo num segredo", "falso", &|c| {
        let k = index(c, "acme-ssh-senha-stage");
        c[k].data[3] = b"falso\0SSH".to_vec();
    });
    with("senha e chave juntas", "falso", &|c| {
        let k = index(c, "acme-ssh-senha-stage");
        c[k].data[4] = fake_key().into_bytes();
    });
    with("porta 0", "porta", &|c| {
        let k = index(c, "acme-ssh-senha-stage");
        c[k].data[1] = vec![0, 0];
    });
    with("porta com 3 bytes", "porta", &|c| {
        let k = index(c, "acme-ssh-senha-stage");
        c[k].data[1] = vec![0x22, 0x08, 0];
    });
    with("ambiente fora da lista", "PROD", &|c| {
        c[0].environment = b"PROD".to_vec()
    });
    with("nome não UTF-8", "\u{FFFD}", &|c| {
        c[0].name = vec![b'a', 0xff]
    });
    with("marca de segredo do extra inválida", "falso", &|c| {
        let k = index(c, "gama-generico");
        c[k].extras[1].1 = 2;
    });
    with("17 tags", "tag-17", &|c| {
        c[0].tags = (1..=17)
            .map(|n| format!("tag-{n:02}").into_bytes())
            .collect()
    });
    with("65 extras", "extra_65", &|c| {
        c[0].extras = (1..=65)
            .map(|n| (format!("extra_{n:02}").into_bytes(), 0, b"v".to_vec()))
            .collect()
    });
    with("33 anexos", "a33", &|c| {
        c[0].attachments = (1..=33)
            .map(|n| (format!("a{n:02}").into_bytes(), b"x".to_vec()))
            .collect()
    });
    with("grupo acima do limite", "gggg", &|c| {
        c[0].group = vec![b'g'; 2049]
    });
    with("segredo acima do limite", "falso", &|c| {
        let k = index(c, "acme-ssh-senha-stage");
        c[k].data[3] = vec![b's'; 64 * 1024 + 1];
    });
    // The attachment named like the key is accepted on read since m2 Phase 54b (refused
    // on save); a name that 0.1.x already reserved is still refused on read.
    with("extra do servidor chamado host", "HOST", &|c| {
        let k = index(c, "acme-ssh-senha-stage");
        c[k].extras.push((b"HOST".to_vec(), 0, b"x".to_vec()));
    });

    assert!(cases.len() >= 30);
    for (case, forged, value) in &cases {
        refused(forged, case, value);
    }
}

#[test]
fn v2_31_bit_flipped_in_every_byte() {
    let body = test_body();
    let mut accepted = 0;
    for i in 0..body.len() {
        let mut v = body.to_vec();
        v[i] ^= 1 << (i % 8);
        if let Ok((c, entries)) = read_all(&v) {
            check_accepted(&v, &c, &entries);
            accepted += 1;
        }
    }
    // Flipping a bit of a plain text sometimes still gives a valid entry (it is the AEAD
    // that blocks that in the file); what matters is that nothing invalid gets through.
    assert!(accepted < body.len());
}

#[test]
fn v2_31_100_000_random_mutations() {
    let body = test_body();
    const TOTAL: usize = 100_000;
    const THREADS: usize = 4;
    let interesting: [u32; 8] = [
        0,
        1,
        2,
        16,
        0x7fff_ffff,
        0xffff_ffff,
        body.len() as u32,
        (body.len() + 1) as u32,
    ];
    let accepted: usize = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..THREADS)
            .map(|f| {
                let body = &body;
                scope.spawn(move || {
                    let mut rng = SplitMix(0x5741_5244_0015_a000 + f as u64);
                    let mut accepted = 0;
                    for _ in 0..TOTAL / THREADS {
                        let mut v = body.to_vec();
                        for _ in 0..1 + rng.below(3) {
                            let pos = rng.below(v.len().max(1));
                            match rng.below(7) {
                                0 => v[pos] ^= 1 << rng.below(8),
                                1 => v[pos] = rng.next_u64() as u8,
                                2 if pos + 4 <= v.len() => {
                                    let x = interesting[rng.below(interesting.len())];
                                    v[pos..pos + 4].copy_from_slice(&x.to_le_bytes());
                                }
                                3 => {
                                    let end = (pos + 1 + rng.below(64)).min(v.len());
                                    v.drain(pos..end);
                                }
                                4 => {
                                    let end = (pos + 1 + rng.below(64)).min(v.len());
                                    let chunk = v[pos..end].to_vec();
                                    let at = rng.below(v.len() + 1);
                                    v.splice(at..at, chunk);
                                }
                                5 => v.insert(pos, rng.next_u64() as u8),
                                _ => {
                                    let other = rng.below(v.len().max(1));
                                    v.swap(pos, other);
                                }
                            }
                            if v.is_empty() {
                                break;
                            }
                        }
                        if let Ok((c, entries)) = read_all(&v) {
                            check_accepted(&v, &c, &entries);
                            accepted += 1;
                        }
                    }
                    accepted
                })
            })
            .collect();
        threads.into_iter().map(|f| f.join().unwrap()).sum()
    });
    assert!(accepted < TOTAL);
}

#[test]
fn errors_without_vault_data() {
    for e in [
        BodyError::Invalid,
        BodyError::TooLarge,
        BodyError::Inconsistent,
    ] {
        let text = format!("{e} {e:?}");
        assert!(!text.contains("falso"));
        assert!(!format!("{e}").is_empty());
    }
    let e = OpenError::Body(BodyError::Invalid);
    assert_eq!(format!("{e}"), "conteúdo do cofre inválido ou corrompido");
}

#[test]
fn read_at_the_limit_accepts_and_above_refuses() {
    // A body above 64 MiB is refused before any read.
    let mut v = vec![0u8; MAX_BODY + 1];
    v[0] = 2;
    refused(&v, "acima de 64 MiB", "falso");
    // The read limits are not lower than the validation ones: the full entry (2048 URL,
    // 64 KiB token, 1 MiB note, attachments adding up to 256 KiB) comes back.
    let full = one_of_each_full();
    let body = serialize(cfg(), 1, &full).unwrap();
    check_round_trip(&full, &read(&body).unwrap());
    // Environment read back through the identifier.
    assert!(full.iter().all(|e| e.environment() == Environment::Stage));
}

// ---- V2.36: too large entry in the forged body (D29) ----

#[test]
fn v2_36_forged_body_with_too_large_entry() {
    use crate::limits::{MAX_ATTACHMENTS_TOTAL, MAX_EXTRA_VALUE, MAX_EXTRAS};

    let body = test_body();
    let base = raw_records(&body);

    // One attachment of 256 KiB + 1 byte (already refused in the 1st pass).
    let mut c = base.clone();
    let k = index(&c, "gama-generico");
    c[k].attachments = vec![(
        b"grande.bin".to_vec(),
        vec![0x5a; MAX_ATTACHMENTS_TOTAL + 1],
    )];
    refused(&forge(&c), "anexo de 256 KiB + 1", "FALSO_ENV");

    // Two attachments of 200 KiB: each one fits, the sum does not.
    let mut c = base.clone();
    c[k].attachments = vec![
        (b"a.bin".to_vec(), vec![0x5a; 200 * 1024]),
        (b"b.bin".to_vec(), vec![0x5a; 200 * 1024]),
    ];
    refused(&forge(&c), "dois anexos de 200 KiB", "FALSO_ENV");

    // `api` with 64 secret extras of 64 KiB: each field fits the limits, but the entry
    // goes over the mask budget (refused in the 2nd pass validation).
    let mut c = base.clone();
    let k = index(&c, "beta-github-api");
    c[k].extras = (0..MAX_EXTRAS)
        .map(|i| {
            (
                format!("e{i:02}").into_bytes(),
                1,
                vec![b'k'; MAX_EXTRA_VALUE],
            )
        })
        .collect();
    refused(&forge(&c), "api acima do orçamento", "kkkk");

    // Control: at the limit (one 256 KiB attachment, a few large extras) the body opens.
    let mut c = base.clone();
    let k = index(&c, "gama-generico");
    c[k].attachments = vec![(b"grande.bin".to_vec(), vec![0x5a; MAX_ATTACHMENTS_TOTAL])];
    let k = index(&c, "beta-github-api");
    c[k].extras = (0..2)
        .map(|i| {
            (
                format!("e{i:02}").into_bytes(),
                1,
                vec![b'k'; MAX_EXTRA_VALUE],
            )
        })
        .collect();
    let forged = forge(&c);
    let (read_cfg, read_entries) = read_all(&forged).expect("no limite o corpo abre");
    check_accepted(&forged, &read_cfg, &read_entries);
}

// ---- V2.39 / V2.30: the configuration section of body v2 (D30) ----

/// PHC text with the given fields (to forge hashes that creation would never make).
fn phc(alg: &str, version: &str, params: &str, salt: &str, hash: &str) -> Vec<u8> {
    let mut t = format!("${alg}");
    if !version.is_empty() {
        t.push_str(&format!("$v={version}"));
    }
    t.push_str(&format!("${params}${salt}${hash}"));
    t.into_bytes()
}

/// The `salt` and `hash` parts (PHC B64) of the test vault hash.
fn hash_parts() -> (String, String) {
    let text = core::str::from_utf8(cfg().pin_hash().bytes()).unwrap();
    let fields: Vec<&str> = text.split('$').collect();
    assert_eq!(fields.len(), 6, "hash PHC com 5 campos");
    (fields[4].to_owned(), fields[5].to_owned())
}

#[test]
fn v2_30_config_comes_back_equal() {
    // Non-default values to prove that every field goes and comes back.
    let mut c = new_config();
    c.set_lock_minutes(30).unwrap();
    c.set_backup(ExternalBackup::new(true, Some("1234-ABCD")).unwrap());
    let body = serialize(&c, 1, &test_vault_entries()).unwrap();
    let (read_cfg, entries) = read_all(&body).unwrap();
    assert_eq!(entries.len(), 22);
    assert_eq!(read_cfg.lock_minutes(), 30);
    assert_eq!(
        read_cfg.backup(),
        &ExternalBackup::new(true, Some("1234-ABCD")).unwrap()
    );
    assert!(read_cfg.pin_hash().bytes() == c.pin_hash().bytes());
    assert!(read_cfg.verify_pin(TEST_PIN.as_bytes()));
    assert!(!read_cfg.verify_pin(b"135791"));
    assert!(serialize(&read_cfg, 1, &entries).unwrap()[..] == body[..]);

    // Backup disabled with a stored UUID also comes back (the USB drive already chosen).
    c.set_backup(ExternalBackup::new(false, Some("0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0")).unwrap());
    let body = serialize(&c, 1, &[]).unwrap();
    let (read_cfg, _) = read_all(&body).unwrap();
    assert_eq!(read_cfg.backup(), c.backup());

    // Test vault default: 240 minutes and backup disabled without UUID.
    let (read_cfg, _) = read_all(&test_body()).unwrap();
    assert_eq!(read_cfg.lock_minutes(), 240);
    assert_eq!(read_cfg.backup(), &ExternalBackup::disabled());
}

/// V2.39: the test PIN does not appear in cleartext in the decrypted body; only the PHC
/// hash, with parameters and salt.
#[test]
fn v2_39_pin_only_as_hash_in_the_body() {
    let body = test_body();
    assert!(
        !body
            .windows(TEST_PIN.len())
            .any(|j| j == TEST_PIN.as_bytes())
    );
    let hash = cfg().pin_hash().bytes();
    assert!(hash.starts_with(b"$argon2id$v=19$m=65536,t=3,p=1$"));
    assert!(body.windows(hash.len()).any(|j| j == hash));
    let (salt, output) = hash_parts();
    assert_eq!(salt.len(), 22, "sal de 16 bytes");
    assert_eq!(output.len(), 43, "saída de 32 bytes");
}

/// V2.39: body v1 (the one from Phase 15a, entries only), body v2 without the section and
/// a body without a PIN hash are refused with the generic error.
#[test]
fn v2_39_body_v1_without_section_or_without_hash_refused() {
    let body = test_body();
    let base = raw_records(&body);
    let n = base.len() as u32;
    refused(&forge_cfg(1, None, n, &base), "corpo v1", "falso");
    refused(&forge_cfg(1, None, 0, &[]), "corpo v1 vazio", "falso");
    refused(
        &forge_cfg(1, Some(&RawCfg::standard()), n, &base),
        "v1 com configuração",
        "falso",
    );
    refused(
        &forge_cfg(BODY_VERSION, None, n, &base),
        "sem a seção",
        "falso",
    );
    refused(
        &forge_cfg(BODY_VERSION, None, 0, &[]),
        "sem a seção, vazio",
        "falso",
    );
    let no_hash = RawCfg {
        pin_hash: Vec::new(),
        ..RawCfg::standard()
    };
    refused(
        &forge_cfg(BODY_VERSION, Some(&no_hash), n, &base),
        "sem hash",
        "falso",
    );
    refused(
        &forge_cfg(BODY_VERSION, Some(&no_hash), 0, &[]),
        "sem hash, vazio",
        "falso",
    );
    // Control: the same assembly with the right section opens.
    assert!(
        read(&forge_cfg(
            BODY_VERSION,
            Some(&RawCfg::standard()),
            n,
            &base
        ))
        .is_ok()
    );
}

/// Attack: PIN hash forged in the body (downgraded to run cheaply, inflated to hang the
/// daemon, of another algorithm, with an extra parameter, short salt or output, out of
/// the canonical form, the cleartext PIN in place of the hash…), and the other fields of
/// the section outside the rule. All refused with the same error, without the value in
/// the error.
#[test]
fn v2_39_forged_config_section() {
    let (salt, output) = hash_parts();
    let (s, h) = (salt.as_str(), output.as_str());
    let p = "m=65536,t=3,p=1";
    let hashes: Vec<(&str, Vec<u8>)> = vec![
        ("PIN em claro", TEST_PIN.as_bytes().to_vec()),
        ("argon2i", phc("argon2i", "19", p, s, h)),
        ("argon2d", phc("argon2d", "19", p, s, h)),
        ("scrypt", phc("scrypt", "19", p, s, h)),
        ("versão 16", phc("argon2id", "16", p, s, h)),
        ("sem versão", phc("argon2id", "", p, s, h)),
        (
            "memória rebaixada",
            phc("argon2id", "19", "m=8,t=3,p=1", s, h),
        ),
        (
            "passes rebaixados",
            phc("argon2id", "19", "m=65536,t=1,p=1", s, h),
        ),
        (
            "paralelismo zero",
            phc("argon2id", "19", "m=65536,t=3,p=0", s, h),
        ),
        (
            "memória inflada",
            phc("argon2id", "19", "m=4194304,t=3,p=1", s, h),
        ),
        (
            "passes inflados",
            phc("argon2id", "19", "m=65536,t=4294967295,p=1", s, h),
        ),
        (
            "paralelismo inflado",
            phc("argon2id", "19", "m=65536,t=3,p=255", s, h),
        ),
        (
            "parâmetro a mais",
            phc("argon2id", "19", "m=65536,t=3,p=1,keyid=AAAA", s, h),
        ),
        (
            "parâmetro faltando",
            phc("argon2id", "19", "m=65536,t=3", s, h),
        ),
        (
            "fora de ordem",
            phc("argon2id", "19", "t=3,m=65536,p=1", s, h),
        ),
        (
            "zero à esquerda",
            phc("argon2id", "19", "m=065536,t=3,p=1", s, h),
        ),
        ("sal curto", phc("argon2id", "19", p, "AAAAAAAAAAA", h)),
        ("saída curta", phc("argon2id", "19", p, s, &h[..22])),
        ("sem saída", format!("$argon2id$v=19${p}${s}").into_bytes()),
        (
            "sobra",
            [phc("argon2id", "19", p, s, h), b"$x".to_vec()].concat(),
        ),
        (
            "espaço no fim",
            [phc("argon2id", "19", p, s, h), b" ".to_vec()].concat(),
        ),
        (
            "não ASCII",
            [phc("argon2id", "19", p, s, h), "é".as_bytes().to_vec()].concat(),
        ),
        (
            "acima do limite",
            vec![b'$'; crate::limits::MAX_PIN_HASH + 1],
        ),
    ];
    let body = test_body();
    let base = raw_records(&body);
    let n = base.len() as u32;
    for (case, hash) in &hashes {
        let c = RawCfg {
            pin_hash: hash.clone(),
            ..RawCfg::standard()
        };
        let start = std::time::Instant::now();
        refused(&forge_cfg(BODY_VERSION, Some(&c), n, &base), case, h);
        // None of these gets to run Argon2 (the inflated one does not allocate 4 GiB).
        assert!(
            start.elapsed() < std::time::Duration::from_secs(5),
            "{case}: demorou"
        );
    }
    // Control: the test vault hash, rebuilt by the test `phc`, opens.
    let c = RawCfg {
        pin_hash: phc("argon2id", "19", p, s, h),
        ..RawCfg::standard()
    };
    assert!(read(&forge_cfg(BODY_VERSION, Some(&c), n, &base)).is_ok());

    let others: Vec<(&str, RawCfg)> = vec![
        (
            "trava 0",
            RawCfg {
                minutes: 0,
                ..RawCfg::standard()
            },
        ),
        (
            "trava 1441",
            RawCfg {
                minutes: 1441,
                ..RawCfg::standard()
            },
        ),
        (
            "trava u32::MAX",
            RawCfg {
                minutes: u32::MAX,
                ..RawCfg::standard()
            },
        ),
        (
            "marca do backup 2",
            RawCfg {
                enabled: 2,
                ..RawCfg::standard()
            },
        ),
        (
            "backup ligado sem UUID",
            RawCfg {
                enabled: 1,
                ..RawCfg::standard()
            },
        ),
        (
            "UUID com ..",
            RawCfg {
                uuid: b"../../x".to_vec(),
                ..RawCfg::standard()
            },
        ),
        (
            "UUID com - no começo",
            RawCfg {
                uuid: b"-oX".to_vec(),
                ..RawCfg::standard()
            },
        ),
        (
            "UUID com espaço",
            RawCfg {
                uuid: b"1234 ABCD".to_vec(),
                ..RawCfg::standard()
            },
        ),
        (
            "UUID de 37",
            RawCfg {
                uuid: vec![b'a'; 37],
                ..RawCfg::standard()
            },
        ),
        (
            "UUID com U+202E",
            RawCfg {
                uuid: "12\u{202E}34".as_bytes().to_vec(),
                ..RawCfg::standard()
            },
        ),
    ];
    for (case, c) in &others {
        refused(&forge_cfg(BODY_VERSION, Some(c), n, &base), case, "falso");
    }
    // Controls: the accepted limits open.
    for minutes in [1, 240, 1440] {
        let c = RawCfg {
            minutes,
            ..RawCfg::standard()
        };
        assert!(
            read(&forge_cfg(BODY_VERSION, Some(&c), n, &base)).is_ok(),
            "{minutes}"
        );
    }
    let c = RawCfg {
        enabled: 1,
        uuid: b"1234-ABCD".to_vec(),
        ..RawCfg::standard()
    };
    assert!(read(&forge_cfg(BODY_VERSION, Some(&c), n, &base)).is_ok());
}

/// Attack: a downgraded hash (low cost, made for the right PIN) neither opens the vault
/// nor becomes valid: reading refuses before any check.
#[test]
fn v2_39_downgraded_hash_of_the_right_pin_refused() {
    let weak = PinKdfParams {
        memory_kib: 8,
        passes: 1,
        parallelism: 1,
    };
    let downgraded = crate::crypto::hash_pin(TEST_PIN.as_bytes(), &weak).unwrap();
    // The downgraded hash is a valid Argon2id PHC (the library checks it)…
    {
        use argon2::password_hash::PasswordVerifier;
        assert!(
            argon2::Argon2::default()
                .verify_password(TEST_PIN.as_bytes(), downgraded.as_str())
                .is_ok()
        );
    }
    // …but the body that carries it is refused, and the `PinHash` is not born from it.
    let body = test_body();
    let base = raw_records(&body);
    let c = RawCfg {
        pin_hash: downgraded.as_bytes().to_vec(),
        ..RawCfg::standard()
    };
    refused(
        &forge_cfg(BODY_VERSION, Some(&c), base.len() as u32, &base),
        "hash rebaixado",
        "falso",
    );
    assert!(PinHash::from_phc(downgraded.as_bytes()).is_none());
}

/// The encoder also refuses a configuration that would not reopen (a lock time out of
/// range only exists if someone tampers from inside the crate; the body never writes it).
#[test]
fn serialize_refuses_config_outside_the_rule() {
    let c = VaultConfig::from_parts(
        PinHash::from_phc(cfg().pin_hash().bytes()).unwrap(),
        0,
        ExternalBackup::disabled(),
    );
    assert_eq!(serialize(&c, 1, &[]).unwrap_err(), BodyError::Inconsistent);
}

// ---- Phase 19c: D39 (generation, anti-rollback) ----

/// V2.44 (D39, D53), medium finding 2 (the auditor's write repro rewrote the same
/// open vault with nothing to tell the versions apart): every write raises the
/// generation, and a write that fails after sealing burns its own; an older authentic
/// version copied over is refused with `Rollback`, with no entry and no value in the
/// error; the password change also raises it and also refuses.
#[cfg(unix)]
#[test]
fn v2_44_generation_rises_and_rollback_is_refused() {
    use crate::atomic_write::{WriteError, change_password, write_entries};

    let dir = Dir::new("generation");
    let destination = dir.0.join("cofre.ward");
    let password = MasterPassword::new(MASTER_PASSWORD.to_owned());
    let new = MasterPassword::new("falso-mestra-ward-02".to_owned());
    let entries = test_vault_entries();
    let file_generation = |s: &MasterPassword| {
        let file = std::fs::read(&destination).unwrap();
        open_entries(&file, s).unwrap().0.generation()
    };

    let mut vault = OpenVault::new(&password, new_config()).unwrap();
    assert_eq!(vault.generation(), 0);
    write_entries(&destination, &mut vault, &entries).unwrap();
    let n = vault.generation();
    assert_eq!(n, 1);
    assert_eq!(file_generation(&password), n);
    let old = std::fs::read(&destination).unwrap();

    // n+1 through `write_entries`.
    write_entries(&destination, &mut vault, &entries).unwrap();
    assert_eq!(vault.generation(), n + 1);
    assert_eq!(file_generation(&password), n + 1);

    // Write failure after sealing (the destination becomes a directory, the rename fails):
    // the file stays at n+1 and the open vault keeps the generation it sealed (D53).
    let directory = dir.0.join("sou-diretorio");
    std::fs::create_dir(&directory).unwrap();
    assert!(write_entries(&directory, &mut vault, &entries).is_err());
    assert_eq!(vault.generation(), n + 2);
    assert_eq!(file_generation(&password), n + 1);

    // The password change (minimum n+1, what the daemon saw; burned n+2) seals n+3.
    let (changed, _) =
        change_password(&destination, &password, &new, n + 1, vault.generation()).unwrap();
    assert_eq!(changed.generation(), n + 3);
    assert_eq!(file_generation(&new), n + 3);
    let at_n3 = std::fs::read(&destination).unwrap();
    let (_, read_entries) = open_entries_since(&at_n3, &new, n + 3, n + 3).unwrap();
    check_round_trip(&entries, &read_entries);

    // Rollback with an authentic version: the file of generation n copied over.
    std::fs::write(&destination, &old).unwrap();
    let file = std::fs::read(&destination).unwrap();
    match open_entries_since(&file, &password, n + 3, n + 3) {
        Ok(_) => panic!("abriu um cofre mais antigo que o último visto"),
        Err(e) => {
            assert_eq!(e, OpenError::Rollback);
            for text in [format!("{e}"), format!("{e:?}")] {
                assert!(!text.contains("falso") && !text.contains("acme"), "{text}");
            }
        }
    }
    // Without a minimum (0, first opening) it opens; the same generation as the minimum too.
    assert!(open_entries_since(&file, &password, 0, 0).is_ok());
    assert!(open_entries_since(&file, &password, n, 0).is_ok());
    // The password change over the old file also refuses, and does not touch the disk.
    assert_eq!(
        change_password(&destination, &password, &new, n + 3, n + 3).unwrap_err(),
        WriteError::Rollback
    );
    assert!(std::fs::read(&destination).unwrap() == old);
}

/// V2.44 (D39): the vault open at generation `u64::MAX` refuses to write without touching the disk.
#[cfg(unix)]
#[test]
fn v2_44_exhausted_generation_does_not_touch_the_disk() {
    use crate::atomic_write::{WriteError, write_entries};

    let dir = Dir::new("exhausted");
    let destination = dir.0.join("cofre.ward");
    let password = MasterPassword::new(MASTER_PASSWORD.to_owned());
    let mut vault = OpenVault::new(&password, new_config()).unwrap();
    write_entries(&destination, &mut vault, &[]).unwrap();
    let before = std::fs::read(&destination).unwrap();
    vault.burn(u64::MAX);
    assert_eq!(
        write_entries(&destination, &mut vault, &test_vault_entries()).unwrap_err(),
        WriteError::GenerationExhausted
    );
    assert_eq!(vault.generation(), u64::MAX);
    assert!(std::fs::read(&destination).unwrap() == before);
    let names: Vec<_> = std::fs::read_dir(&dir.0).unwrap().collect();
    assert_eq!(names.len(), 1, "sobrou temporário");
}

/// V2.44 (D39): `serialize` refuses generation 0, and reading refuses body v2 (no
/// generation) and generation 0 with the generic error.
#[test]
fn v2_44_generation_zero_and_body_v2_refused() {
    assert_eq!(
        serialize(cfg(), 0, &test_vault_entries()).unwrap_err(),
        BodyError::Inconsistent
    );
    let body = test_body();
    // Body v2: version 2 followed directly by the configuration (without the 8 generation bytes).
    let mut v2 = vec![2u8, 0];
    v2.extend_from_slice(&body[10..]);
    refused(&v2, "corpo v2", "falso-mestra");
    let mut zero = body.to_vec();
    zero[2..10].copy_from_slice(&0u64.to_le_bytes());
    refused(&zero, "geração 0", "geração");
    // Any other generation ≥ 1 opens and comes back equal.
    for g in [1u64, 2, 0x0102_0304_0506_0708, u64::MAX] {
        let mut v = body.to_vec();
        v[2..10].copy_from_slice(&g.to_le_bytes());
        let (_, read_gen, _) = deserialize(&v).unwrap();
        assert_eq!(read_gen, g);
    }
}

// ---- Phase 19c: D38, D42, D43 in the forged body ----

/// V2.45, V2.48 and V2.49: reading the body applies the same rules as validation. A
/// forged body (written from outside, encrypted with the right key) with text outside
/// the allow list, a variable without suffix or a dangerous `ssh` username is refused
/// with the generic error, without the value.
#[test]
fn attack_forged_body_with_text_variable_and_username() {
    let body = test_body();
    let base = raw_records(&body);
    let mut cases: Vec<(String, Vec<u8>, String)> = Vec::new();
    let mut with = |case: String, value: String, f: &dyn Fn(&mut Vec<RawRecord>)| {
        let mut c = base.clone();
        f(&mut c);
        cases.push((case, forge(&c), value));
    };
    for ch in [
        "\u{3164}",
        "\u{1D173}",
        "\u{2800}",
        "\u{FE0F}",
        "\u{00A0}",
        "a\u{0301}",
    ] {
        let fake = format!("acme{ch}prod");
        let b = fake.clone().into_bytes();
        let login = index(&base, "acme-wp-dev");
        let b1 = b.clone();
        with(format!("{ch:?} no projeto"), fake.clone(), &move |c| {
            c[login].project = b1.clone()
        });
        let b2 = b.clone();
        with(format!("{ch:?} na tag"), fake.clone(), &move |c| {
            c[login].tags = vec![b2.clone()]
        });
        let b3 = b.clone();
        with(
            format!("{ch:?} no usuário do login"),
            fake.clone(),
            &move |c| c[login].data[1] = b3.clone(),
        );
        let k = index(&base, "gama-generico");
        let b4 = b.clone();
        with(
            format!("{ch:?} no nome do extra"),
            fake.clone(),
            &move |c| c[k].extras[0].0 = b4.clone(),
        );
        let b5 = b.clone();
        with(
            format!("{ch:?} no nome do anexo"),
            fake.clone(),
            &move |c| c[k].attachments[0].0 = b5.clone(),
        );
    }
    let api = index(&base, "beta-github-api");
    for var in [
        "GIT_PROXY_COMMAND",
        "PAGER",
        "LD_PRELOAD",
        "HTTPS_PROXY",
        "GIT_ASKPASS",
    ] {
        with(format!("variável {var}"), var.to_owned(), &move |c| {
            c[api].data[2] = var.as_bytes().to_vec()
        });
    }
    let ssh = index(&base, "acme-ssh-senha-stage");
    let jump = index(&base, "acme-ssh-salto");
    let ftp = index(&base, "beta-sftp-dev");
    for username in ["u;id", "u'$(id)'", "u%h", "-deploy@x", "u v", "usuário"] {
        with(
            format!("usuário {username}"),
            username.to_owned(),
            &move |c| c[ssh].data[2] = username.as_bytes().to_vec(),
        );
        with(
            format!("usuário do salto {username}"),
            username.to_owned(),
            &move |c| c[jump].data[2] = username.as_bytes().to_vec(),
        );
        with(
            format!("usuário do ftp {username}"),
            username.to_owned(),
            &move |c| c[ftp].data[3] = username.as_bytes().to_vec(),
        );
    }
    for (case, forged, value) in &cases {
        refused(forged, case, value);
    }
    // Control: `@` in the username (D43) and a Latin accent in the project pass.
    let mut c = base.clone();
    c[ssh].data[2] = b"deploy@cliente.com.br".to_vec();
    c[ftp].data[3] = b"deploy@cliente.com.br".to_vec();
    let login = index(&base, "acme-wp-dev");
    c[login].project = "São Paulo".as_bytes().to_vec();
    assert!(read(&forge(&c)).is_ok());
}

/// V2.51 (D45): the `TooLarge` message comes from the `limits` constants (changing
/// `MAX_BODY` or `MAX_ENTRIES` changes the message, and this test checks against them).
#[test]
fn v2_51_too_large_message_follows_the_limits() {
    use crate::limits::{MAX_BODY, MAX_ENTRIES, MIB};
    let msg = format!("{}", BodyError::TooLarge);
    let thousands = |n: usize| {
        let s = n.to_string();
        let mut v = String::new();
        for (i, c) in s.chars().enumerate() {
            if i > 0 && (s.len() - i).is_multiple_of(3) {
                v.push(' ');
            }
            v.push(c);
        }
        v
    };
    assert_eq!(
        msg,
        format!(
            "cofre grande demais (limite de {} MiB e {} entradas)",
            MAX_BODY / MIB,
            thousands(MAX_ENTRIES)
        )
    );
    const {
        assert!(
            MAX_BODY.is_multiple_of(MIB),
            "o limite do corpo é um número inteiro de MiB"
        )
    };
}

/// V2.58 (D55): reading the body applies the last label rule. A forged body with the
/// `login` or `api` URL read as IPv4 by the browser is refused with the generic error,
/// without the value; a domain with a digit only at the start of a middle label opens.
#[test]
fn v2_58_forged_body_with_numeric_last_label() {
    let body = test_body();
    let base = raw_records(&body);
    let login = index(&base, "acme-wp-dev");
    let api = index(&base, "beta-github-api");
    for url in [
        "https://0x7f000001/",
        "https://0x7f.0.0.1/",
        "https://1.2.3.0x4/",
        "https://x.123/",
        "https://x.0x10/",
        "https://x.example.9z/",
    ] {
        for (i, name) in [(login, "login"), (api, "api")] {
            let mut c = base.clone();
            c[i].data[0] = url.as_bytes().to_vec();
            refused(&forge(&c), &format!("url {url} no {name}"), url);
        }
    }
    let mut c = base.clone();
    c[login].data[0] = b"https://1password.com/".to_vec();
    c[api].data[0] = b"https://x1.example2/".to_vec();
    assert!(read(&forge(&c)).is_ok());
}

// ---- Phase 22d: D65 d (creation and modification dates, body v4; V3.41) ----

/// The dates rule: range from 1 to `MAX_DATE`, creation only with modification and not
/// after it; the modification never goes down, not even with the clock going back.
#[test]
fn d65_dates_rule() {
    assert_eq!(Dates::new_stamp(0), None);
    assert_eq!(Dates::new_stamp(MAX_DATE + 1), None);
    assert_eq!(Dates::new_stamp(u64::MAX), None);
    let d = Dates::new_stamp(1_000).unwrap();
    assert_eq!(
        (d.created_at(), d.modified_at()),
        (Some(1_000), Some(1_000))
    );
    let after = d.modified(5_000).unwrap();
    assert_eq!(
        (after.created_at(), after.modified_at()),
        (Some(1_000), Some(5_000))
    );
    // Clock going back: the modification stays where it was, the creation does not change.
    let went_back = after.modified(10).unwrap();
    assert_eq!(
        (went_back.created_at(), went_back.modified_at()),
        (Some(1_000), Some(5_000))
    );
    assert_eq!(after.modified(0), None);
    assert_eq!(after.modified(MAX_DATE + 1), None);
    // No dates: the modification gets a date, the creation stays unknown.
    let modified_only = Dates::NONE.modified(77).unwrap();
    assert_eq!(
        (modified_only.created_at(), modified_only.modified_at()),
        (None, Some(77))
    );
    assert_eq!(Dates::from_parts(Some(5), Some(4)), None);
    assert_eq!(Dates::from_parts(Some(5), None), None);
    assert_eq!(Dates::from_parts(None, Some(0)), None);
    assert_eq!(Dates::from_parts(Some(0), Some(1)), None);
}

/// V3.41: body v3 (no dates) is refused as an unknown version, with the generic error;
/// the current body (v5 since m2:DT4) without the date bytes, or with the version
/// changed to 3, too.
#[test]
fn d65_body_v3_refused() {
    let body = test_body();
    let base = raw_records(&body);
    let n = base.len() as u32;
    let cfg = RawCfg::standard();
    refused(
        &forge_raw(3, Some(&cfg), n, &base, false),
        "corpo v3",
        "falso",
    );
    refused(
        &forge_raw(3, Some(&cfg), n, &base, true),
        "v5 rotulado 3",
        "falso",
    );
    refused(
        &forge_raw(BODY_VERSION, Some(&cfg), n, &base, false),
        "v5 sem datas",
        "falso",
    );
    refused(
        &forge_raw(
            LEGACY_BODY_VERSION,
            Some(&cfg),
            n,
            &v4_records(&base),
            false,
        ),
        "v4 sem datas",
        "falso",
    );
    refused(
        &forge_raw(6, Some(&cfg), n, &base, true),
        "versão 6",
        "falso",
    );
    // The empty v3 vault too: the version is checked before anything else.
    refused(
        &forge_raw(3, Some(&cfg), 0, &[], false),
        "v3 vazio",
        "falso",
    );
    // Control: the same assembly with version 5 and the dates opens, and so does the v4
    // one with the v4 names and the dates (m2:DT4).
    assert!(read(&forge_raw(BODY_VERSION, Some(&cfg), n, &base, true)).is_ok());
    assert!(
        read(&forge_raw(
            LEGACY_BODY_VERSION,
            Some(&cfg),
            n,
            &v4_records(&base),
            true
        ))
        .is_ok()
    );
    assert_eq!(body[..2], [5, 0]);
}

/// V3.41: dates forged in the body (out of range, creation after modification, creation
/// without modification) refuse the whole body with the generic error, without the
/// value; the valid forms open and come back equal.
#[test]
fn d65_forged_dates_in_the_body() {
    let body = test_body();
    let base = raw_records(&body);
    let i = index(&base, "acme-pg-dev");
    let with = |created: u64, modified: u64| {
        let mut c = base.clone();
        c[i].created = created;
        c[i].modified = modified;
        forge(&c)
    };
    for (case, created, modified) in [
        ("criação depois da alteração", 1_762_000_001, 1_762_000_000),
        ("criação sem alteração", 1_762_000_000, 0),
        ("criação acima do máximo", MAX_DATE + 1, MAX_DATE + 1),
        ("alteração acima do máximo", 0, MAX_DATE + 1),
        ("as duas em u64::MAX", u64::MAX, u64::MAX),
        ("alteração em u64::MAX", 1, u64::MAX),
    ] {
        refused(&with(created, modified), case, &modified.to_string());
    }
    for (created, modified) in [(0, 0), (0, 7), (7, 7), (1, MAX_DATE), (MAX_DATE, MAX_DATE)] {
        let v = with(created, modified);
        let (c, entries) = read_all(&v).unwrap();
        check_accepted(&v, &c, &entries);
        let e = entries.iter().find(|e| e.name() == "acme-pg-dev").unwrap();
        let opt = |d: u64| (d != 0).then_some(d);
        assert_eq!(e.dates().created_at(), opt(created));
        assert_eq!(e.dates().modified_at(), opt(modified));
    }
}

/// V3.41: the dates neither store nor leak secrets: the body with dates has the same
/// bytes as the body without them outside the 16 bytes of each entry, and touching only
/// the dates changes no other field.
#[test]
fn d65_dates_only_change_the_16_bytes_of_the_entry() {
    let without = test_body();
    let mut entries = test_vault_entries();
    for e in &mut entries {
        e.set_dates(Dates::new_stamp(1_762_000_000).unwrap());
    }
    let with = serialize(cfg(), 1, &entries).unwrap();
    assert_eq!(without.len(), with.len());
    let differing = without
        .iter()
        .zip(with.iter())
        .filter(|(a, b)| a != b)
        .count();
    // 1_762_000_000 = 0x6905_C680: 4 non-zero bytes in each of the 2 dates.
    assert_eq!(differing, entries.len() * 2 * 4);
}

/// V3.41: an authentic v3 vault on disk (sealed with the right key, body v3 without
/// dates) is refused by the daemon's opening (`open_entries_with_generation`) as an
/// invalid body, not as a wrong password, and the opening does not touch the file nor
/// leave a temporary one.
#[cfg(unix)]
#[test]
fn d65_v3_vault_on_disk_refused_without_touching_the_file() {
    let dir = Dir::new("v3");
    let destination = dir.0.join("cofre.ward");
    let password = MasterPassword::new(MASTER_PASSWORD.to_owned());
    let vault = OpenVault::new(&password, new_config()).unwrap();
    let mut c = RawCfg::of(vault.config());
    c.generation = Some(1);
    let base = raw_records(&test_body());
    let v3 = forge_raw(3, Some(&c), base.len() as u32, &base, false);
    let file = vault.seal(&v3).unwrap();
    crate::atomic_write::write(&destination, &file).unwrap();
    let before = std::fs::read(&destination).unwrap();
    let meta = std::fs::metadata(&destination).unwrap();
    for (minimum, burned) in [(0, 0), (1, 1), (5, 9)] {
        let read_bytes = std::fs::read(&destination).unwrap();
        assert_eq!(
            open_entries_with_generation(&read_bytes, &password, minimum, burned).map(|_| ()),
            Err(OpenError::Body(BodyError::Invalid))
        );
    }
    let after = std::fs::metadata(&destination).unwrap();
    assert!(
        std::fs::read(&destination).unwrap() == before,
        "o arquivo mudou"
    );
    assert_eq!(meta.modified().unwrap(), after.modified().unwrap());
    let names: Vec<_> = std::fs::read_dir(&dir.0).unwrap().collect();
    assert_eq!(names.len(), 1, "sobrou arquivo além do cofre");
    // Control: the same vault with body v5 opens, and with body v4 (v4 names) too.
    let v5 = forge_raw(BODY_VERSION, Some(&c), base.len() as u32, &base, true);
    assert!(open_entries(&vault.seal(&v5).unwrap(), &password).is_ok());
    let v4 = forge_raw(
        LEGACY_BODY_VERSION,
        Some(&c),
        base.len() as u32,
        &v4_records(&base),
        true,
    );
    assert!(open_entries(&vault.seal(&v4).unwrap(), &password).is_ok());
}

// ---- m2:DT4: body v5 and reading of v4 (m2:V4.1, m2:V4.2, m2:V4.8, m2:V4.9) ----

/// v5 → v4 stored names of type, level and SMTP security, written here by hand from the
/// table of m2:DT4 (independent of `legacy_v4`; the tests compare the two).
const V5_TO_V4: [(&str, &str); 15] = [
    ("login", "login"),
    ("database", "banco"),
    ("server", "servidor"),
    ("api", "api"),
    ("aws", "aws"),
    ("smtp", "smtp"),
    ("ftp-sftp", "ftp-sftp"),
    ("secure-note", "nota-segura"),
    ("generic", "generico"),
    ("blocked", "bloqueado"),
    ("approval", "com-aprovacao"),
    ("free", "livre"),
    ("none", "nenhuma"),
    ("starttls", "starttls"),
    ("tls", "tls"),
];

fn v4_name(v5: &[u8]) -> Vec<u8> {
    V5_TO_V4
        .iter()
        .find(|(n5, _)| n5.as_bytes() == v5)
        .map(|(_, n4)| n4.as_bytes().to_vec())
        .expect("nome v5 fora da tabela")
}

/// The raw entries of a v5 body with the v4 names, as 0.1.x wrote them.
fn v4_records(base: &[RawRecord]) -> Vec<RawRecord> {
    base.iter()
        .cloned()
        .map(|mut c| {
            if c.kind == b"smtp" {
                c.data[SMTP_SECURITY] = v4_name(&c.data[SMTP_SECURITY]);
            }
            c.kind = v4_name(&c.kind);
            c.level = v4_name(&c.level);
            c
        })
        .collect()
}

/// The test vault as a body v4 (generation 1), forged independently of the module.
fn v4_test_body() -> Vec<u8> {
    let base = raw_records(&test_body());
    forge_with(LEGACY_BODY_VERSION, base.len() as u32, &v4_records(&base))
}

/// The two tables: each v5 name is the `name()` of its enum, the v4 table gives the same
/// enum back from the v4 name, and neither version knows the names of the other one. The
/// v4 table is closed: nothing near a v4 name passes.
#[test]
fn dt4_tables_of_the_two_versions() {
    assert_eq!(BODY_VERSION, 5);
    assert_eq!(LEGACY_BODY_VERSION, 4);
    for (v5, v4) in V5_TO_V4 {
        let (b5, b4) = (v5.as_bytes(), v4.as_bytes());
        if let Some(k) = EntryType::from_name(v5) {
            assert_eq!(k.name(), v5);
            assert_eq!(legacy_v4::kind(b4), Some(k), "{v4}");
            assert_eq!(legacy_v4::level(b4), None, "{v4}");
            if v4 != v5 {
                assert_eq!(legacy_v4::kind(b5), None, "v5 {v5} aceito no v4");
                assert_eq!(EntryType::from_name(v4), None, "v4 {v4} aceito no v5");
            }
        } else if let Some(l) = AccessLevel::from_name(v5) {
            assert_eq!(l.name(), v5);
            assert_eq!(legacy_v4::level(b4), Some(l), "{v4}");
            assert_eq!(legacy_v4::kind(b4), None, "{v4}");
            assert_eq!(legacy_v4::level(b5), None, "v5 {v5} aceito no v4");
            assert_eq!(AccessLevel::from_name(v4), None, "v4 {v4} aceito no v5");
        } else {
            let s = SmtpSecurity::from_name(v5).expect("nome v5 sem enum");
            assert_eq!(s.name(), v5);
            assert_eq!(legacy_v4::smtp_security(b4), Some(b5), "{v4}");
            if v4 != v5 {
                assert_eq!(legacy_v4::smtp_security(b5), None, "v5 {v5} aceito no v4");
                assert_eq!(SmtpSecurity::from_name(v4), None, "v4 {v4} aceito no v5");
            }
        }
    }
    // Every value of the three enums is in the table, once.
    assert_eq!(
        EntryType::ALL.len() + AccessLevel::ALL.len() + SmtpSecurity::ALL.len(),
        V5_TO_V4.len()
    );
    let names: Vec<&str> = EntryType::ALL
        .iter()
        .map(|k| k.name())
        .chain(AccessLevel::ALL.iter().map(|l| l.name()))
        .chain(SmtpSecurity::ALL.iter().map(|s| s.name()))
        .collect();
    for (v5, _) in V5_TO_V4 {
        assert_eq!(names.iter().filter(|n| **n == v5).count(), 1, "{v5}");
    }
    // Closed: case, spaces, separators, accents, suffixes, NUL and empty are refused.
    for near in [
        "Banco",
        "BANCO",
        " banco",
        "banco ",
        "banco2",
        "nota_segura",
        "nota segura",
        "genérico",
        "Livre",
        "livre ",
        "com_aprovacao",
        "com aprovação",
        "Bloqueado",
        "Nenhuma",
        "nenhuma\0",
        "STARTTLS",
        "",
    ] {
        let b = near.as_bytes();
        assert_eq!(legacy_v4::kind(b), None, "{near:?}");
        assert_eq!(legacy_v4::level(b), None, "{near:?}");
        assert_eq!(legacy_v4::smtp_security(b), None, "{near:?}");
    }
}

/// m2:DT4: the test vault as a body v4 reads as the same configuration, generation and
/// entries (every field, secret, extra, attachment and date), and is written again as
/// exactly the v5 body of the same vault.
#[test]
fn dt4_v4_body_reads_equal_and_is_written_as_v5() {
    let v4 = v4_test_body();
    let v5 = test_body();
    assert_eq!(v4[..2], [4, 0]);
    assert_eq!(v5[..2], [5, 0]);
    let (config, generation, entries) = deserialize(&v4).unwrap();
    assert_eq!(generation, 1);
    assert!(config.pin_hash().bytes() == cfg().pin_hash().bytes());
    assert!(config.verify_pin(TEST_PIN.as_bytes()));
    assert_eq!(config.lock_minutes(), cfg().lock_minutes());
    assert_eq!(config.backup().enabled(), cfg().backup().enabled());
    assert_eq!(config.backup().uuid(), cfg().backup().uuid());
    check_round_trip(&test_vault_entries(), &entries);
    let written = serialize(&config, generation, &entries).unwrap();
    assert!(
        written[..] == v5[..],
        "v4 gravado diferente do v5 do mesmo cofre"
    );
    // Only the stored names change size: the v5 body is the v4 one plus the difference
    // of each renamed name.
    let size = |b: &[u8]| b.len() as isize;
    let delta: isize = raw_records(&v5)
        .iter()
        .map(|c| {
            let mut d =
                size(&c.kind) - size(&v4_name(&c.kind)) + size(&c.level) - size(&v4_name(&c.level));
            if c.kind == b"smtp" {
                let s = &c.data[SMTP_SECURITY];
                d += size(s) - size(&v4_name(s));
            }
            d
        })
        .sum();
    assert_eq!(size(&v5), size(&v4) + delta);
    // The generation of a backup version (Phase 49a) is read from both.
    assert_eq!(body_generation(&v4), Some(1));
    assert_eq!(body_generation(&v5), Some(1));
}

/// m2:DT4: the only renamed value inside the type fields, the v4 name of the SMTP
/// security `none`, becomes `none`; `none` in a v4 body and the v4 name in a v5 body are
/// refused.
#[test]
fn dt4_v4_smtp_security_none_becomes_v5_none() {
    let base = raw_records(&test_body());
    let k = index(&base, "beta-smtp-stage");
    let mut v5 = base.clone();
    v5[k].data[SMTP_SECURITY] = b"none".to_vec();
    let n = v5.len() as u32;
    let v5_body = forge(&v5);
    let v4_body = forge_with(LEGACY_BODY_VERSION, n, &v4_records(&v5));
    assert!(v4_body.windows(7).any(|w| w == b"nenhuma"));
    let entries = read(&v4_body).unwrap();
    let e = entries
        .iter()
        .find(|e| e.name() == "beta-smtp-stage")
        .unwrap();
    assert!(matches!(e.data(), Data::Smtp(s) if s.security == SmtpSecurity::None));
    let written = serialize(cfg(), 1, &entries).unwrap();
    assert!(written[..] == v5_body[..]);
    assert!(!written.windows(7).any(|w| w == b"nenhuma"));

    let mut crossed = v4_records(&v5);
    crossed[k].data[SMTP_SECURITY] = b"none".to_vec();
    refused(
        &forge_with(LEGACY_BODY_VERSION, n, &crossed),
        "segurança v5 no corpo v4",
        "none",
    );
    for near in ["Nenhuma", "nenhuma ", "", "ssl"] {
        let mut c = v4_records(&v5);
        c[k].data[SMTP_SECURITY] = near.as_bytes().to_vec();
        refused(
            &forge_with(LEGACY_BODY_VERSION, n, &c),
            "segurança fora da tabela v4",
            "beta-smtp-stage",
        );
    }
    let mut crossed = v5.clone();
    crossed[k].data[SMTP_SECURITY] = b"nenhuma".to_vec();
    refused(&forge(&crossed), "segurança v4 no corpo v5", "nenhuma");
}

/// m2:V4.8 (m2:RM2): a body v4 with an unknown name (the cases of the contract, the
/// other v5 names and names near the v4 ones) is refused whole with the generic error,
/// without the value.
#[test]
fn dt4_v4_body_with_unknown_name_refused() {
    let base = v4_records(&raw_records(&test_body()));
    let n = base.len() as u32;
    let mut cases: Vec<(&str, Vec<u8>, &str)> = Vec::new();
    let mut with = |case: &'static str, value: &'static str, f: &dyn Fn(&mut Vec<RawRecord>)| {
        let mut c = base.clone();
        f(&mut c);
        cases.push((case, forge_with(LEGACY_BODY_VERSION, n, &c), value));
    };
    // The cases of the contract.
    with("tipo banco2", "banco2", &|c| {
        let k = index(c, "acme-pg-dev");
        c[k].kind = b"banco2".to_vec();
    });
    with("nível livre com espaço", "livre ", &|c| {
        let k = index(c, "acme-pg-dev");
        c[k].level = b"livre ".to_vec();
    });
    with("nível free no v4", "free", &|c| {
        let k = index(c, "acme-pg-dev");
        c[k].level = b"free".to_vec();
    });
    with("tipo database no v4", "database", &|c| {
        let k = index(c, "acme-pg-dev");
        c[k].kind = b"database".to_vec();
    });
    // The other v5 names, in an entry of the matching type.
    for (name, kind) in [
        ("acme-ssh-senha-stage", "server"),
        ("gama-nota", "secure-note"),
        ("gama-generico", "generic"),
    ] {
        with("tipo v5 no v4", "tipo", &move |c| {
            let k = index(c, name);
            c[k].kind = kind.as_bytes().to_vec();
        });
    }
    for level in ["approval", "blocked"] {
        with("nível v5 no v4", "nivel", &move |c| {
            c[0].level = level.as_bytes().to_vec()
        });
    }
    // Near the v4 names.
    for kind in ["Banco", "banco ", " banco", "nota_segura", "genérico", ""] {
        with("tipo perto do v4", "tipo", &move |c| {
            let k = index(c, "acme-pg-dev");
            c[k].kind = kind.as_bytes().to_vec();
        });
    }
    for level in ["Livre", "com_aprovacao", "com aprovação", "bloqueada", ""] {
        with("nível perto do v4", "nivel", &move |c| {
            c[0].level = level.as_bytes().to_vec()
        });
    }
    for (case, forged, value) in &cases {
        refused(forged, case, value);
    }
    // Control: the v4 body without changes opens.
    assert!(read(&forge_with(LEGACY_BODY_VERSION, n, &base)).is_ok());
}

/// m2:V4.2 at the body level: one bit flipped in each byte of the decrypted v4 body;
/// whatever is accepted is a valid vault and is written as v5 (`check_accepted`). (In
/// the file, the AEAD already refuses any changed byte: `format` V2.2.)
#[test]
fn dt4_v4_bit_flipped_in_every_byte() {
    let body = v4_test_body();
    let mut accepted = 0;
    for i in 0..body.len() {
        let mut v = body.clone();
        v[i] ^= 1 << (i % 8);
        if let Ok((c, entries)) = read_all(&v) {
            check_accepted(&v, &c, &entries);
            accepted += 1;
        }
    }
    assert!(accepted < body.len());
    // The version bytes flipped never give a known version with the v4 names: 4 ^ 1 = 5
    // (v5 does not know them) and the others are unknown.
    for bit in 0..16 {
        let mut v = body.clone();
        v[bit / 8] ^= 1 << (bit % 8);
        refused(&v, "versão do v4 trocada", "falso");
    }
}

/// m2:V4.1 and m2:DT4 on disk: a vault sealed with a body v4 refuses the wrong password
/// as a format error; opening it with the right one reads the entries and touches
/// nothing (bytes, mtime, no temporary file); writing the entries read gives a body v5
/// with generation + 1.
#[cfg(unix)]
#[test]
fn dt4_v4_vault_on_disk_opens_without_writing_and_any_write_is_v5() {
    use crate::atomic_write::write_entries;

    let dir = Dir::new("v4");
    let destination = dir.0.join("cofre.ward");
    let password = MasterPassword::new(MASTER_PASSWORD.to_owned());
    let vault = OpenVault::new(&password, new_config()).unwrap();
    let mut c = RawCfg::of(vault.config());
    c.generation = Some(3);
    let base = v4_records(&raw_records(&test_body()));
    let v4 = forge_raw(
        LEGACY_BODY_VERSION,
        Some(&c),
        base.len() as u32,
        &base,
        true,
    );
    crate::atomic_write::write(&destination, &vault.seal(&v4).unwrap()).unwrap();
    let before = std::fs::read(&destination).unwrap();
    let modified = std::fs::metadata(&destination).unwrap().modified().unwrap();
    let untouched = || {
        assert!(
            std::fs::read(&destination).unwrap() == before,
            "o arquivo mudou"
        );
        let now = std::fs::metadata(&destination).unwrap().modified().unwrap();
        assert_eq!(now, modified);
        let names: Vec<_> = std::fs::read_dir(&dir.0).unwrap().collect();
        assert_eq!(names.len(), 1, "sobrou arquivo além do cofre");
    };

    let wrong = MasterPassword::new("falso-mestra-ward-02".to_owned());
    assert_eq!(
        open_entries(&before, &wrong).map(|_| ()),
        Err(OpenError::Format(FormatError::PasswordOrFile))
    );
    untouched();

    let (mut opened, entries, generation) =
        open_entries_with_generation(&before, &password, 0, 0).unwrap();
    assert_eq!(generation, 3);
    check_round_trip(&test_vault_entries(), &entries);
    untouched();

    write_entries(&destination, &mut opened, &entries).unwrap();
    let after = std::fs::read(&destination).unwrap();
    let (_, content) = format::open(&after, &password).unwrap();
    assert_eq!(content[..2], [5, 0]);
    let (_, g, again) = deserialize(&content).unwrap();
    assert_eq!(g, 4);
    check_round_trip(&test_vault_entries(), &again);
}

// ---- m2 Phase 54b: names reserved only since 0.2.0, accepted on read ----

/// Resources that count the calls and refuse them all: the delivery of an entry with a
/// reserved name must not reach any of them (no temp file, no TOTP, no tunnel).
#[derive(Default)]
struct CountingResources(usize);

impl crate::delivery::Resources for CountingResources {
    fn totp_code(
        &mut self,
        _seed: &Secret,
    ) -> Result<Zeroizing<String>, crate::delivery::ResourceError> {
        self.0 += 1;
        Err(crate::delivery::ResourceError("recurso pedido"))
    }

    fn file(
        &mut self,
        _name: &str,
        _content: &Secret,
    ) -> Result<String, crate::delivery::ResourceError> {
        self.0 += 1;
        Err(crate::delivery::ResourceError("recurso pedido"))
    }

    fn tunnel(
        &mut self,
        _server: &str,
        _host: &str,
        _port: u16,
    ) -> Result<u16, crate::delivery::ResourceError> {
        self.0 += 1;
        Err(crate::delivery::ResourceError("recurso pedido"))
    }
}

/// The raw entries of the test vault with extras (not secret) and attachments added to
/// one entry.
fn with_names(entry: &str, extras: &[&str], attachments: &[&str]) -> Vec<RawRecord> {
    let mut base = raw_records(&test_body());
    let k = index(&base, entry);
    for name in extras {
        base[k]
            .extras
            .push((name.as_bytes().to_vec(), 0, b"valor-de-teste".to_vec()));
    }
    for name in attachments {
        base[k]
            .attachments
            .push((name.as_bytes().to_vec(), b"conteudo-de-teste".to_vec()));
    }
    base
}

/// m2 Phase 54b (S1, m2:V4.1, m2:V4.19): a vault of 0.1.x (v4), or one already migrated
/// (v5), with an extra or attachment named like a variable that only 0.2.0 delivers
/// opens; the v4 is written again as exactly the v5 bytes (it migrates); and the delivery
/// of that entry is refused whole before any resource, so no extra replaces
/// `WARD_ENTRY`/`WARD_GROUP`/`WARD_ENVIRONMENT`/`WARD_PROTOCOL` and no attachment
/// replaces or impersonates `WARD_FILE_KEY`.
#[test]
fn m2_54b_names_reserved_since_v5_open_and_migrate() {
    let cases: [(&str, &[&str], &[&str]); 12] = [
        ("gama-generico", &["ENTRY"], &[]),
        ("gama-generico", &["group"], &[]),
        ("gama-generico", &["Environment"], &[]),
        ("gama-generico", &["FILE"], &[]),
        // Same variable as the attachment `app.env` (WARD_FILE_APP_ENV).
        ("gama-generico", &["file_app_env"], &[]),
        ("gama-generico", &["File-Key"], &[]),
        (
            "acme-wp-dev",
            &["entry", "GROUP", "environment", "file_x"],
            &[],
        ),
        ("beta-sftp-dev", &["PROTOCOL"], &[]),
        ("beta-sftp-dev", &[], &["key"]),
        ("acme-ssh-chave-prod", &[], &["key"]),
        ("acme-ssh-senha-stage", &[], &["KEY"]),
        ("gama-nota", &["ENTRY"], &[]),
    ];
    for (entry, extras, attachments) in cases {
        let case = format!("{entry} {extras:?} {attachments:?}");
        let records = with_names(entry, extras, attachments);
        let n = records.len() as u32;
        let v5 = forge(&records);
        let v4 = forge_with(LEGACY_BODY_VERSION, n, &v4_records(&records));
        for (body, version) in [(&v4, "v4"), (&v5, "v5")] {
            let entries = read(body).unwrap_or_else(|e| panic!("{case} {version}: {e}"));
            assert_eq!(entries.len(), n as usize, "{case} {version}");
            let written = serialize(cfg(), 1, &entries).unwrap();
            assert!(written[..] == v5[..], "{case} {version}: não migrou igual");
            let e = entries.iter().find(|e| e.name() == entry).unwrap();
            for name in extras {
                assert!(
                    e.extras().iter().any(|x| x.name() == *name),
                    "{case} {version}: extra perdido"
                );
            }
            for name in attachments {
                assert!(
                    e.attachments().iter().any(|a| a.name() == *name),
                    "{case} {version}: anexo perdido"
                );
            }
            // `ward run`: refused whole, before any resource.
            assert!(crate::entry::has_reserved_name(e), "{case}");
            let mut resources = CountingResources::default();
            let delivered = crate::delivery::build(e, &mut resources);
            let expected = if e.kind() == EntryType::SecureNote {
                crate::delivery::DeliveryError::TypeWithoutDelivery
            } else {
                crate::delivery::DeliveryError::ReservedName
            };
            assert_eq!(delivered.err(), Some(expected), "{case} {version}");
            assert_eq!(resources.0, 0, "{case} {version}: recurso pedido");
            // The other entries are untouched.
            for other in entries.iter().filter(|o| o.name() != entry) {
                assert!(!crate::entry::has_reserved_name(other), "{case}");
            }
        }
    }
}

/// m2 Phase 54b: the leniency is only for the names that 0.1.x left free. A name that
/// 0.1.x already reserved (a variable of the type), and two extras or attachments that
/// become the same variable, are still refused on read, in v4 and in v5.
#[test]
fn m2_54b_other_reserved_names_still_refused_on_read() {
    let cases: [(&str, &[&str], &[&str]); 10] = [
        ("acme-wp-dev", &["PASS"], &[]),
        ("acme-wp-dev", &["url"], &[]),
        ("acme-wp-dev", &["Totp"], &[]),
        ("acme-ssh-senha-stage", &["HOST"], &[]),
        ("acme-ssh-chave-prod", &["passphrase"], &[]),
        ("beta-sftp-dev", &["user"], &[]),
        ("beta-github-api", &["TOKEN"], &[]),
        ("gama-generico", &["ENTRY", "entry"], &[]),
        ("gama-generico", &["FILE_X", "file-x"], &[]),
        ("acme-ssh-chave-prod", &[], &["key", "Key"]),
    ];
    for (entry, extras, attachments) in cases {
        let case = format!("{entry} {extras:?} {attachments:?}");
        let records = with_names(entry, extras, attachments);
        let n = records.len() as u32;
        refused(&forge(&records), &format!("{case} v5"), entry);
        refused(
            &forge_with(LEGACY_BODY_VERSION, n, &v4_records(&records)),
            &format!("{case} v4"),
            entry,
        );
    }
}
