//! Format tests (V2.1–V2.8). All with the real Argon2id parameters, except where the
//! test says otherwise. Fake test passwords (D15).

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Condvar, Mutex};

use super::*;

const PASSWORD: &str = "falso-mestra-ward-01";
const WRONG_PASSWORD: &str = "falso-mestra-ward-02";
const NEW_PASSWORD: &str = "falso-mestra-ward-03";
const CONTENT: &[u8] = br#"{"entradas":[{"nome":"acme-pg-dev","senha":"falso-PG:s3nha/01@#"}]}"#;

/// Each real Argon2id uses 256 MiB: at most 4 at the same time across all tests
/// (`cargo test` runs the tests in parallel).
static SLOTS: Mutex<usize> = Mutex::new(4);
static NOTICE: Condvar = Condvar::new();

fn with_slot<T>(f: impl FnOnce() -> T) -> T {
    {
        let mut v = NOTICE
            .wait_while(SLOTS.lock().unwrap(), |v| *v == 0)
            .unwrap();
        *v -= 1;
    }
    struct GiveBack;
    impl Drop for GiveBack {
        fn drop(&mut self) {
            *SLOTS.lock().unwrap_or_else(|e| e.into_inner()) += 1;
            NOTICE.notify_one();
        }
    }
    let _d = GiveBack;
    f()
}

fn password(s: &str) -> MasterPassword {
    MasterPassword::new(s.to_owned())
}

fn create(content: &[u8]) -> Vec<u8> {
    with_slot(|| {
        VaultKeys::new_with(&password(PASSWORD), KdfParams::DEFAULT)
            .unwrap()
            .seal(content)
            .unwrap()
    })
}

fn open_t(file: &[u8], s: &str) -> Result<Zeroizing<Vec<u8>>, FormatError> {
    with_slot(|| open(file, &password(s)).map(|(_, c)| c))
}

/// Opens each file in up to 4 threads; returns, per file, whether it opened and whether
/// there was a panic.
fn open_many(files: Vec<Vec<u8>>) -> Vec<(bool, bool)> {
    let queue = Mutex::new(files.into_iter().enumerate().collect::<Vec<_>>());
    let output = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                loop {
                    let Some((i, file)) = queue.lock().unwrap().pop() else {
                        break;
                    };
                    let r = catch_unwind(AssertUnwindSafe(|| open_t(&file, PASSWORD).is_ok()));
                    let item = match r {
                        Ok(opened) => (i, opened, false),
                        Err(_) => (i, false, true),
                    };
                    output.lock().unwrap().push(item);
                }
            });
        }
    });
    let mut v = output.into_inner().unwrap();
    v.sort_by_key(|x| x.0);
    v.into_iter().map(|(_, a, p)| (a, p)).collect()
}

#[test]
fn round_trip() {
    let file = create(CONTENT);
    assert_eq!(file.len(), HEADER_SIZE + CONTENT.len() + TAG_SIZE);
    assert_eq!(&open_t(&file, PASSWORD).unwrap()[..], CONTENT);
}

#[test]
fn empty_content_opens() {
    let file = create(b"");
    assert_eq!(open_t(&file, PASSWORD).unwrap().len(), 0);
}

/// Proof of S2 in the format: no stretch of the content (8 bytes or more) appears in the
/// encrypted file.
#[test]
fn nothing_in_clear_in_the_file() {
    let file = create(CONTENT);
    for window in CONTENT.windows(8) {
        assert!(!file.windows(8).any(|w| w == window));
    }
    assert!(
        !file
            .windows(PASSWORD.len())
            .any(|w| w == PASSWORD.as_bytes())
    );
}

/// V2.1: a wrong password gives the same error as a tampered file, and nothing is
/// decrypted.
#[test]
fn v2_1_wrong_password_same_error_as_tampered() {
    let file = create(CONTENT);
    let wrong = open_t(&file, WRONG_PASSWORD);
    assert_eq!(wrong, Err(FormatError::PasswordOrFile));

    let mut tampered = file.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 0x01;
    let tamp = open_t(&tampered, PASSWORD);
    assert_eq!(wrong, tamp);
    assert_eq!(
        wrong.unwrap_err().to_string(),
        "senha mestra errada ou arquivo do cofre adulterado"
    );
}

/// V2.2: one bit flipped in **each** byte of the file (one file per position): every
/// opening fails, without panic.
#[test]
fn v2_2_bit_flipped_in_each_byte() {
    let file = create(b"falso-nota-01");
    let files: Vec<Vec<u8>> = (0..file.len())
        .map(|i| {
            let mut a = file.clone();
            a[i] ^= 1 << (i % 8);
            a
        })
        .collect();
    let result = open_many(files);
    assert_eq!(result.len(), file.len());
    for (i, (opened, panicked)) in result.into_iter().enumerate() {
        assert!(!panicked, "pânico com o byte {i} trocado");
        assert!(!opened, "abriu com o byte {i} trocado");
    }
    // Control: the original opens.
    assert!(open_t(&file, PASSWORD).is_ok());
}

/// V2.3: truncated at each size from 0 to n−1 and extended by 1 byte: every opening
/// fails.
#[test]
fn v2_3_truncated_and_extended() {
    let file = create(b"falso-nota-01");
    for size in 0..file.len() {
        let r = catch_unwind(|| open_t(&file[..size], PASSWORD));
        assert!(matches!(r, Ok(Err(_))), "truncado em {size} não falhou");
    }
    for extra in [0x00u8, 0x01, 0xff] {
        let mut a = file.clone();
        a.push(extra);
        assert_eq!(open_t(&a, PASSWORD), Err(FormatError::PasswordOrFile));
    }
    // Extended and with the size field "fixed": the body tag catches it (the size is in
    // the AAD and the body changed).
    let mut a = file.clone();
    a.push(0x00);
    let new = (a.len() - HEADER_SIZE) as u64;
    a[P_SIZE..HEADER_SIZE].copy_from_slice(&new.to_le_bytes());
    assert_eq!(open_t(&a, PASSWORD), Err(FormatError::PasswordOrFile));
    assert!(open_t(&file, PASSWORD).is_ok());
}

/// V2.4: the header records Argon2id with memory ≥ 262144 KiB, passes ≥ 3 and the
/// parallelism.
#[test]
fn v2_4_header_records_params() {
    let file = create(CONTENT);
    let header = read_header(&file).unwrap();
    assert_eq!(&file[..8], b"WARDCOFR");
    assert_eq!(file[P_KDF], KDF_ARGON2ID_13);
    assert_eq!(header.version, 1);
    assert!(header.kdf.memory_kib >= 262_144);
    assert!(header.kdf.passes >= 3);
    assert!(header.kdf.parallelism >= 1);
    assert_eq!(header.kdf, KdfParams::DEFAULT);
}

fn with_params(file: &[u8], kdf: KdfParams) -> Vec<u8> {
    let mut a = file.to_vec();
    a[P_MEMORY..P_PASSES].copy_from_slice(&kdf.memory_kib.to_le_bytes());
    a[P_PASSES..P_PARALLELISM].copy_from_slice(&kdf.passes.to_le_bytes());
    a[P_PARALLELISM..P_SALT].copy_from_slice(&kdf.parallelism.to_le_bytes());
    a
}

const EIGHT_MIB: KdfParams = KdfParams {
    memory_kib: 8 * 1024,
    passes: 3,
    parallelism: 4,
};

/// V2.4, downgrade attack: the attacker rewrites the header with 8 MiB. The opening
/// refuses (below the minimum) and, even if the minimum accepted 8 MiB, the
/// authentication catches it (the header is authenticated data and goes into the
/// derivation).
#[test]
fn v2_4_downgrade_to_8_mib_fails() {
    let file = create(CONTENT);
    let downgraded = with_params(&file, EIGHT_MIB);
    assert_eq!(
        open_t(&downgraded, PASSWORD),
        Err(FormatError::ParamsBelowMinimum)
    );
    let r = open_with_minimum(&downgraded, &password(PASSWORD), &EIGHT_MIB).map(|_| ());
    assert_eq!(r, Err(FormatError::PasswordOrFile));

    // Downgrading only the passes too.
    let p = KdfParams {
        passes: 1,
        ..KdfParams::DEFAULT
    };
    assert_eq!(
        open_t(&with_params(&file, p), PASSWORD),
        Err(FormatError::ParamsBelowMinimum)
    );
}

/// V2.4: a well-formed (authentic) file that declares parameters below the minimum is
/// refused. Control: with a test minimum it opens, so it is well formed.
#[test]
fn v2_4_well_formed_below_minimum_refused() {
    let s = password(PASSWORD);
    let weak = VaultKeys::new_internal(&s, EIGHT_MIB, &EIGHT_MIB)
        .unwrap()
        .seal(CONTENT)
        .unwrap();
    assert_eq!(
        open(&weak, &s).map(|_| ()),
        Err(FormatError::ParamsBelowMinimum)
    );
    let (_, c) = open_with_minimum(&weak, &s, &EIGHT_MIB).unwrap();
    assert_eq!(&c[..], CONTENT);
    // The public API does not create a weak vault.
    assert_eq!(
        VaultKeys::new_with(&s, EIGHT_MIB).map(|_| ()),
        Err(FormatError::ParamsBelowMinimum)
    );
}

/// Denial of service attack: a header that asks for huge memory or passes is refused
/// before deriving (allocates nothing).
#[test]
fn params_above_maximum_refused_without_deriving() {
    let file = create(b"falso-nota-01");
    for kdf in [
        KdfParams {
            memory_kib: u32::MAX,
            ..KdfParams::DEFAULT
        },
        KdfParams {
            passes: u32::MAX,
            ..KdfParams::DEFAULT
        },
        KdfParams {
            parallelism: 0xFF_FFFF,
            ..KdfParams::DEFAULT
        },
    ] {
        let start = std::time::Instant::now();
        let r = open(&with_params(&file, kdf), &password(PASSWORD)).map(|_| ());
        assert_eq!(r, Err(FormatError::ParamsAboveMaximum));
        assert!(start.elapsed() < std::time::Duration::from_millis(100));
    }
    let zero = KdfParams {
        parallelism: 0,
        ..KdfParams::DEFAULT
    };
    assert_eq!(
        open(&with_params(&file, zero), &password(PASSWORD)).map(|_| ()),
        Err(FormatError::ParamsBelowMinimum)
    );
    assert_eq!(
        VaultKeys::new_with(
            &password(PASSWORD),
            KdfParams {
                memory_kib: u32::MAX,
                ..KdfParams::DEFAULT
            }
        )
        .map(|_| ()),
        Err(FormatError::ParamsAboveMaximum)
    );
}

/// The body is bound to the whole header by the AAD: with the right data key, the body
/// does not open if any header byte changes.
#[test]
fn header_is_the_body_aad() {
    let s = password(PASSWORD);
    let original = create(CONTENT);
    let (vault, _) = with_slot(|| open(&original, &s).unwrap());
    let file = vault.seal(CONTENT).unwrap();
    let mut nonce = [0u8; NONCE_SIZE];
    nonce.copy_from_slice(&file[P_DATA_NONCE..P_SIZE]);
    let body = &file[HEADER_SIZE..];
    assert!(crypto::open(&vault.data_key, &nonce, &file[..HEADER_SIZE], body).is_ok());
    for i in 0..HEADER_SIZE {
        let mut header = file[..HEADER_SIZE].to_vec();
        header[i] ^= 0x01;
        assert!(
            crypto::open(&vault.data_key, &nonce, &header, body).is_err(),
            "byte {i} do cabeçalho fora do AAD"
        );
    }
}

/// Splicing attack: the header of one vault with the body of another (same password)
/// does not open. Nor the body of an earlier write of the same vault with the header of
/// another.
#[test]
fn splicing_between_vaults_and_writes_fails() {
    let a = create(b"falso-conteudo-a");
    let b = create(b"falso-conteudo-b");
    let mut mixed = a[..HEADER_SIZE].to_vec();
    mixed.extend_from_slice(&b[HEADER_SIZE..]);
    assert_eq!(open_t(&mixed, PASSWORD), Err(FormatError::PasswordOrFile));

    let (vault, _) = with_slot(|| open(&a, &password(PASSWORD)).unwrap());
    let g1 = vault.seal(b"falso-versao-1").unwrap();
    let g2 = vault.seal(b"falso-versao-2").unwrap();
    let mut mixed = g1[..HEADER_SIZE].to_vec();
    mixed.extend_from_slice(&g2[HEADER_SIZE..]);
    assert_eq!(open_t(&mixed, PASSWORD), Err(FormatError::PasswordOrFile));
    // And the wrapped key of one vault in the header of the other.
    let mut mixed = a.clone();
    mixed[P_WRAPPED..P_DATA_NONCE].copy_from_slice(&b[P_WRAPPED..P_DATA_NONCE]);
    assert_eq!(open_t(&mixed, PASSWORD), Err(FormatError::PasswordOrFile));
}

/// V2.5 through the format path: the file is exactly Argon2id (no extras) → wrapping
/// XChaCha20-Poly1305 with AAD = bytes 0..80 → body XChaCha20-Poly1305 with AAD =
/// header. Redone here only with the `crypto` functions (the same as the vectors').
#[test]
fn v2_5_format_composition() {
    let file = create(CONTENT);
    let header = read_header(&file).unwrap();
    let kek = with_slot(|| {
        crypto::derive_kek(PASSWORD.as_bytes(), &file[P_SALT..P_KEY_NONCE], &header.kdf).unwrap()
    });
    let key_nonce: [u8; NONCE_SIZE] = file[P_KEY_NONCE..P_WRAPPED].try_into().unwrap();
    let key = crypto::open(
        &kek,
        &key_nonce,
        &file[..P_WRAPPED],
        &file[P_WRAPPED..P_DATA_NONCE],
    )
    .unwrap();
    let key: [u8; KEY_SIZE] = key[..].try_into().unwrap();
    let data_nonce: [u8; NONCE_SIZE] = file[P_DATA_NONCE..P_SIZE].try_into().unwrap();
    let text = crypto::open(
        &key,
        &data_nonce,
        &file[..HEADER_SIZE],
        &file[HEADER_SIZE..],
    )
    .unwrap();
    assert_eq!(&text[..], CONTENT);
}

/// V2.6: two vaults with the same password and the same content have different salt,
/// nonces, wrapped key and ciphertext.
#[test]
fn v2_6_two_equal_vaults_are_different() {
    let a = create(CONTENT);
    let b = create(CONTENT);
    assert_eq!(a.len(), b.len());
    let fields = [
        ("sal", P_SALT..P_KEY_NONCE),
        ("nonce da chave", P_KEY_NONCE..P_WRAPPED),
        ("chave embrulhada", P_WRAPPED..P_DATA_NONCE),
        ("nonce dos dados", P_DATA_NONCE..P_SIZE),
        ("texto cifrado", HEADER_SIZE..a.len()),
    ];
    for (name, range) in fields {
        assert_ne!(a[range.clone()], b[range], "{name} igual nos dois cofres");
    }
    // The data key is also different (not only the wrapping).
    let (ka, _) = with_slot(|| open(&a, &password(PASSWORD)).unwrap());
    let (kb, _) = with_slot(|| open(&b, &password(PASSWORD)).unwrap());
    assert_ne!(&ka.data_key[..], &kb.data_key[..]);
}

/// Writing again through the open vault (the daemon path): a new data nonce on each
/// write, and the file opens with the password.
#[test]
fn sealing_again_reopens_and_changes_the_nonce() {
    let file = create(CONTENT);
    let (vault, _) = with_slot(|| open(&file, &password(PASSWORD)).unwrap());
    let g1 = vault.seal(b"falso-versao-1").unwrap();
    let g2 = vault.seal(b"falso-versao-1").unwrap();
    assert_ne!(g1[P_DATA_NONCE..P_SIZE], g2[P_DATA_NONCE..P_SIZE]);
    assert_eq!(g1[..P_DATA_NONCE], file[..P_DATA_NONCE]);
    assert_eq!(&open_t(&g2, PASSWORD).unwrap()[..], b"falso-versao-1");
}

/// V2.8: changing the master password: the old one fails, the new one opens, the
/// content is the same; and the new file does not reuse the salt or the data key.
#[test]
fn v2_8_password_change() {
    let file = create(CONTENT);
    let (new_vault, changed_content) =
        with_slot(|| change_password(&file, &password(PASSWORD), &password(NEW_PASSWORD)).unwrap());
    let new = new_vault.seal(&changed_content).unwrap();
    assert_eq!(open_t(&new, PASSWORD), Err(FormatError::PasswordOrFile));
    assert_eq!(&open_t(&new, NEW_PASSWORD).unwrap()[..], CONTENT);
    assert_eq!(&changed_content[..], CONTENT);
    assert_ne!(file[P_SALT..P_KEY_NONCE], new[P_SALT..P_KEY_NONCE]);
    let (old, _) = with_slot(|| open(&file, &password(PASSWORD)).unwrap());
    assert_ne!(&old.data_key[..], &new_vault.data_key[..]);
    assert_eq!(read_header(&new).unwrap().kdf, KdfParams::DEFAULT);
}

/// Attack: changing the password without knowing the current one does not work; an
/// empty new password is refused.
#[test]
fn password_change_requires_the_current_one() {
    let file = create(CONTENT);
    assert_eq!(
        with_slot(|| {
            change_password(&file, &password(WRONG_PASSWORD), &password(NEW_PASSWORD)).map(|_| ())
        }),
        Err(FormatError::PasswordOrFile)
    );
    assert_eq!(
        change_password(&file, &password(PASSWORD), &password("")).map(|_| ()),
        Err(FormatError::EmptyPassword)
    );
    assert_eq!(
        VaultKeys::new_with(&password(""), KdfParams::DEFAULT).map(|_| ()),
        Err(FormatError::EmptyPassword)
    );
}

/// `Debug` shows neither password nor key (S13).
#[test]
fn debug_does_not_show_secret() {
    let s = password(PASSWORD);
    let d = format!("{s:?}");
    assert!(!d.contains(PASSWORD));
    assert!(d.contains("••••"));

    let original = create(CONTENT);
    let (vault, content) = with_slot(|| open(&original, &s).unwrap());
    let d = format!("{vault:?}");
    let key_hex: String = vault.data_key.iter().map(|b| format!("{b:02x}")).collect();
    let key_dec = format!("{:?}", &vault.data_key[..]);
    assert!(!d.contains(&key_hex));
    assert!(!d.contains(&key_dec[1..key_dec.len() - 1]));
    assert!(d.contains("••••"));
    drop(content);
}

/// Garbage, an empty file and a file of another format give the generic error, without
/// panic.
#[test]
fn garbage_does_not_open() {
    let s = password(PASSWORD);
    for garbage in [
        Vec::new(),
        b"WARDCOFR".to_vec(),
        vec![0u8; HEADER_SIZE + TAG_SIZE],
        vec![0xffu8; 4096],
    ] {
        assert_eq!(
            open(&garbage, &s).map(|_| ()),
            Err(FormatError::PasswordOrFile)
        );
    }
    let mut other_version = create(b"x");
    other_version[P_VERSION] = 2;
    assert_eq!(
        open(&other_version, &s).map(|_| ()),
        Err(FormatError::UnsupportedVersion(2))
    );
}

// --- Typed master password: NFC and minimum (Phase 39c) -------------------------------

fn typed(b: &[u8]) -> MasterPassword {
    MasterPassword::typed(Zeroizing::new(b.to_vec()))
}

/// The same text composed (app) or decomposed (another input method, CLI) becomes the
/// same bytes, and the vault created with one opens with the other.
#[test]
fn typed_password_composed_or_decomposed_opens_the_same_vault() {
    let composed = "falso-mestra-\u{e9}-\u{e7}\u{e3}o-01";
    let decomposed = "falso-mestra-e\u{301}-c\u{327}a\u{303}o-01";
    assert_ne!(composed.as_bytes(), decomposed.as_bytes());
    assert_eq!(
        typed(composed.as_bytes()).bytes(),
        typed(decomposed.as_bytes()).bytes()
    );
    assert_eq!(typed(decomposed.as_bytes()).bytes(), composed.as_bytes());
    let file = with_slot(|| {
        VaultKeys::new_with(&typed(composed.as_bytes()), KdfParams::DEFAULT)
            .unwrap()
            .seal(CONTENT)
            .unwrap()
    });
    let opened = with_slot(|| open(&file, &typed(decomposed.as_bytes())));
    assert_eq!(&**opened.unwrap().1, CONTENT);
    // Without NFC (what the daemon did before), the decomposed one does not open.
    let raw = with_slot(|| open(&file, &MasterPassword::new(decomposed.to_owned())));
    assert_eq!(raw.map(|_| ()), Err(FormatError::PasswordOrFile));
}

#[test]
fn typed_password_ascii_and_invalid_bytes_stay_the_same() {
    for b in [&b"falso-mestra-ward-01"[..], b"\xff\xfefalso\x80", b""] {
        assert_eq!(typed(b).bytes(), b);
    }
}

/// Attack: getting past the minimum of 12 with combining marks (11 accented letters
/// decomposed are 22 code points, and 11 after NFC), with bytes that are not UTF-8 or
/// with 11 characters. The daemon checks after NFC.
#[test]
fn attack_short_disguised_new_password_is_refused() {
    let decomposed: String = "e\u{301}".repeat(11);
    assert_eq!(decomposed.chars().count(), 22);
    assert!(!typed(decomposed.as_bytes()).is_valid_new());
    assert!(!typed(b"falso-mest1").is_valid_new());
    assert!(!typed(&[0xff; 13]).is_valid_new());
    assert!(!typed(b"").is_valid_new());
    assert!(typed(b"falso-mestr12").is_valid_new());
    assert!(typed("\u{e9}".repeat(12).as_bytes()).is_valid_new());
    assert!(typed("e\u{301}".repeat(12).as_bytes()).is_valid_new());
}
