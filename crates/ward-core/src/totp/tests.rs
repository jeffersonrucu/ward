use super::*;

fn seed(s: &str) -> Secret {
    Secret::from_text(s.to_owned())
}

/// RFC 6238 seed (appendix B) for SHA1: ASCII "12345678901234567890" in base32.
const RFC_SHA1: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
/// Fake seed of the contract (D15, `acme-wp-dev`/`acme-wp-prod`).
const D15_SEED: &str = "JBSWY3DPEHPK3PXP";

#[test]
fn v2_25_rfc6238_sha1_vectors() {
    // RFC 6238, appendix B, SHA1 column (8 digits).
    let t = Totp::with_digits(&seed(RFC_SHA1), 8).unwrap();
    for (time, expected) in [
        (59, "94287082"),
        (1_111_111_109, "07081804"),
        (1_111_111_111, "14050471"),
        (1_234_567_890, "89005924"),
        (2_000_000_000, "69279037"),
        (20_000_000_000, "65353130"),
    ] {
        assert_eq!(t.code_at(time).code(), expected, "T = {time}");
    }
}

#[test]
fn v2_25_rfc4226_hotp_vectors() {
    // RFC 4226, appendix D: HOTP with the same key, counters 0–9, 6 digits.
    let key = b"12345678901234567890";
    let expected_codes = [
        755_224, 287_082, 359_152, 969_429, 338_314, 254_676, 287_922, 162_583, 399_871, 520_489,
    ];
    for (counter, expected) in expected_codes.iter().enumerate() {
        assert_eq!(hotp(key, counter as u64) % 1_000_000, *expected);
    }
}

/// Codes of `JBSWY3DPEHPK3PXP` checked outside Ward (HMAC-SHA1 of the Python standard
/// library, the same computation as `oathtool --totp -b`).
const D15_KNOWN: [(u64, &str); 10] = [
    (0, "282760"),
    (29, "282760"),
    (30, "996554"),
    (59, "996554"),
    (1_111_111_109, "071271"),
    (1_234_567_890, "742275"),
    (1_762_560_000, "975040"),
    (2_000_000_000, "890699"),
    (4_102_444_800, "573258"),
    (20_000_000_000, "752434"),
];

#[test]
fn v2_25_contract_seed_known_codes() {
    let t = Totp::from_seed(&seed(D15_SEED)).unwrap();
    for (time, expected) in D15_KNOWN {
        assert_eq!(t.code_at(time).code(), expected, "T = {time}");
    }
}

/// Compares with the installed `oathtool`, in the same period. Without `oathtool` in
/// the PATH, the test warns and passes, unless the variable that requires
/// `oathtool` is 1 (the validator turns it on). The seed is the fake one of D15, so it may go in argv only
/// in this test.
#[test]
fn v2_25_matches_oathtool() {
    let require = std::env::var_os("WARD_REQUIRE_OATHTOOL").is_some_and(|v| v == "1");
    let t = Totp::from_seed(&seed(D15_SEED)).unwrap();
    let mut times: Vec<u64> = D15_KNOWN.iter().map(|(t, _)| *t).collect();
    times.push(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    );
    for time in times {
        let output = std::process::Command::new("oathtool")
            .args(["--totp", "-b", &format!("--now=@{time}"), D15_SEED])
            .env("TZ", "UTC")
            .output();
        let output = match output {
            Ok(s) => s,
            Err(_) if !require => {
                eprintln!("oathtool ausente: comparação pulada (WARD_REQUIRE_OATHTOOL=1 exige)");
                return;
            }
            Err(e) => panic!("oathtool exigido e ausente: {e}"),
        };
        assert!(output.status.success(), "oathtool falhou em T = {time}");
        let from_oathtool = String::from_utf8(output.stdout).unwrap();
        assert_eq!(t.code_at(time).code(), from_oathtool.trim(), "T = {time}");
    }
}

#[test]
fn remaining_and_code_now() {
    let t = Totp::from_seed(&seed(D15_SEED)).unwrap();
    assert_eq!(t.code_at(0).remaining(), 30);
    assert_eq!(t.code_at(29).remaining(), 1);
    assert_eq!(t.code_at(30).remaining(), 30);
    let now = t.code_now().unwrap();
    assert_eq!(now.code().len(), 6);
    assert!(now.code().bytes().all(|c| c.is_ascii_digit()));
    assert!((1..=30).contains(&now.remaining()));
    // Leading zeros are kept (T = 1111111109 gives "071271").
    assert_eq!(t.code_at(1_111_111_109).to_string(), "071271");
}

#[test]
fn padding_at_the_end_is_accepted() {
    let with = Totp::from_seed(&seed("JBSWY3DPEHPK3PXP======")).unwrap();
    let without = Totp::from_seed(&seed(D15_SEED)).unwrap();
    assert_eq!(with.code_at(59), without.code_at(59));
}

#[test]
fn base32_rfc4648_vectors() {
    // RFC 4648 §10, without the empty one (below the minimum of 16): "foobar" repeated
    // to go over the minimum, decoded and compared.
    let text = b"foobarfoobar";
    let b32 = "MZXW6YTBOJTG633CMFZA====";
    assert_eq!(&**decode_base32(b32.as_bytes()).unwrap(), text);
    assert_eq!(
        &**decode_base32(RFC_SHA1.as_bytes()).unwrap(),
        b"12345678901234567890"
    );
}

#[test]
fn attack_malformed_seed_is_refused() {
    let bad = [
        "",
        "JBSWY3DPEHPK3PX",                // 15 characters
        "jbswy3dpehpk3pxp",               // lowercase (the entry requires it normalized)
        "JBSWY3DPEHPK3PX1",               // 1 is not base32
        "JBSWY3DPEHPK3PX8",               // 8 is not base32
        "JBSWY3DP=EHPK3PXP",              // = in the middle
        "JBSWY3DPEHPK3PXP=A",             // data after the =
        "JBSWY3DP EHPK3PXP",              // space
        "JBSWY3DPEHPK3PXP\n",             // line break
        "JBSWY3DPEHPK3PXP\0",             // NUL
        "JBSWY3DPEHPK3PXPÁ",              // non-ASCII
        "================",               // padding only
        "otpauth://totp/x?secret=JBSWY3", // URI, not a seed
    ];
    for r in bad {
        assert_eq!(
            Totp::from_seed(&seed(r)).unwrap_err(),
            TotpError::InvalidSeed,
            "{r:?}"
        );
    }
    let long = "A".repeat(MAX_SEED + 1);
    assert!(Totp::from_seed(&seed(&long)).is_err());
    let at_limit = "A".repeat(MAX_SEED);
    assert!(Totp::from_seed(&seed(&at_limit)).is_ok());
    // Invalid UTF-8 bytes (the seed is a secret, it can come from anywhere).
    assert!(Totp::from_seed(&Secret::from_bytes(vec![0xff; 20])).is_err());
}

#[test]
fn attack_digits_out_of_range() {
    for d in [0, 1, 5, 9, 10, u32::MAX] {
        assert_eq!(
            Totp::with_digits(&seed(D15_SEED), d).unwrap_err(),
            TotpError::InvalidDigits
        );
    }
}

#[test]
fn attack_extreme_time_does_not_panic() {
    let t = Totp::with_digits(&seed(D15_SEED), 8).unwrap();
    for time in [0, 1, u64::MAX, u64::MAX - 1, u64::MAX / PERIOD * PERIOD] {
        let c = t.code_at(time);
        assert_eq!(c.code().len(), 8);
        assert!((1..=PERIOD).contains(&c.remaining()));
    }
}

#[test]
fn attack_debug_and_errors_without_seed() {
    // R8 / S13: neither the Debug of the generator nor the errors show the seed or its
    // bytes.
    let t = Totp::from_seed(&seed(D15_SEED)).unwrap();
    let key = decode_base32(D15_SEED.as_bytes()).unwrap();
    let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
    let debug_bytes = format!("{:?}", &key[..]);
    for text in [format!("{t:?}"), format!("{t:#?}")] {
        assert!(!text.contains(D15_SEED), "{text}");
        assert!(!text.contains(&hex), "{text}");
        assert!(
            !text.contains(&debug_bytes[1..debug_bytes.len() - 1]),
            "{text}"
        );
        assert!(text.contains("••••"));
    }
    // Almost valid seed (one wrong character): the error does not repeat it.
    let almost = "JBSWY3DPEHPK3PX1";
    let e = Totp::from_seed(&seed(almost)).unwrap_err();
    for text in [e.to_string(), format!("{e:?}"), format!("{e:#?}")] {
        assert!(!text.contains(almost));
        assert!(!text.contains("JBSWY3"));
    }
}

#[test]
fn v2_28_totp_without_clone_and_seed_zeroes() {
    // At compile time: if `Totp` gets `Clone`, the call becomes ambiguous (E0283).
    trait NoClone<A> {
        fn x() {}
    }
    impl<T: ?Sized> NoClone<()> for T {}
    struct WithClone;
    impl<T: Clone> NoClone<WithClone> for T {}
    <Totp as NoClone<_>>::x();

    // The key stays in a type that zeroes on drop.
    fn zeroes<T: zeroize::ZeroizeOnDrop>(_: &T) {}
    let t = Totp::from_seed(&seed(D15_SEED)).unwrap();
    zeroes(&t.key);
}

/// Phase 16 + 18: the `WARD_TOTP` that delivery puts in the environment is the
/// code of this module, with the RFC 6238 vectors (8 digits) and the D15 seed (6
/// digits), on a fixed clock. The file and tunnel resources are not used by a `login`.
#[test]
fn v2_25_delivered_ward_totp_comes_from_this_module() {
    use crate::delivery::{ResourceError, Resources, build};
    use crate::entry::{DataDraft, Draft};

    struct FixedClock {
        time: u64,
        digits: u32,
    }
    impl Resources for FixedClock {
        fn totp_code(&mut self, seed: &Secret) -> Result<Zeroizing<String>, ResourceError> {
            let t = Totp::with_digits(seed, self.digits)
                .map_err(|_| ResourceError("semente TOTP inválida"))?;
            Ok(Zeroizing::new(t.code_at(self.time).code().to_owned()))
        }
        fn file(&mut self, _: &str, _: &Secret) -> Result<String, ResourceError> {
            Err(ResourceError("sem arquivo no teste"))
        }
        fn tunnel(&mut self, _: &str, _: &str, _: u16) -> Result<u16, ResourceError> {
            Err(ResourceError("sem túnel no teste"))
        }
    }

    fn login(b32_seed: &str) -> crate::entry::Entry {
        let r = Draft {
            name: "acme-wp-prod".to_owned(),
            group: "acme".to_owned(),
            environment: "prod".to_owned(),
            project: String::new(),
            tags: Vec::new(),
            data: DataDraft::Login {
                url: "http://localhost:8081/wp-login.php".to_owned(),
                username: "wp_editor".to_owned(),
                password: Some(seed("falso-WP-senha-02")),
                totp_seed: Some(seed(b32_seed)),
            },
            extras: Vec::new(),
            attachments: Vec::new(),
        };
        match r.validate() {
            Ok(e) => e,
            Err(errors) => panic!("devia validar: {errors}"),
        }
    }

    let rfc = login(RFC_SHA1);
    for (time, expected) in [
        (59, "94287082"),
        (1_111_111_109, "07081804"),
        (2_000_000_000, "69279037"),
    ] {
        let mut r = FixedClock { time, digits: 8 };
        let e = build(&rfc, &mut r).unwrap();
        assert_eq!(e.value("WARD_TOTP"), Some(expected), "T = {time}");
    }
    let d15 = login(D15_SEED);
    for (time, expected) in D15_KNOWN {
        let mut r = FixedClock { time, digits: 6 };
        let e = build(&d15, &mut r).unwrap();
        assert_eq!(e.value("WARD_TOTP"), Some(expected), "T = {time}");
        // The seed does not go to the environment (R8): no variable contains it.
        for v in e.variables() {
            assert!(!v.expose().contains(D15_SEED), "{}", v.name());
        }
    }
}
