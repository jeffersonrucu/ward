use super::*;

fn options(size: usize, mask: u8) -> GeneratorOptions {
    GeneratorOptions {
        size,
        lowercase: mask & 1 != 0,
        uppercase: mask & 2 != 0,
        digits: mask & 4 != 0,
        symbols: mask & 8 != 0,
    }
}

fn sets_of(mask: u8) -> Vec<&'static [u8]> {
    [LOWERCASE, UPPERCASE, DIGITS, SYMBOLS]
        .into_iter()
        .enumerate()
        .filter(|(i, _)| mask & (1 << i) != 0)
        .map(|(_, c)| c)
        .collect()
}

#[test]
fn sets_are_disjoint_and_visible_ascii() {
    let all = [LOWERCASE, UPPERCASE, DIGITS, SYMBOLS].concat();
    let mut sorted = all.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), all.len(), "conjuntos com caractere repetido");
    assert!(all.iter().all(|c| c.is_ascii_graphic()));
    for forbidden in *b" \"'`\\" {
        assert!(!all.contains(&forbidden));
    }
    assert!(SYMBOLS.iter().all(|c| c.is_ascii_punctuation()));
}

/// V2.26: for each of the 15 set combinations, 10 000 passwords with the sizes from 4 to
/// 128 in rotation (every size shows up in every combination). None has a character
/// outside the selected sets and all have at least one of each.
#[test]
fn v2_26_ten_thousand_passwords_per_combination() {
    for mask in 1u8..16 {
        let sets = sets_of(mask);
        let allowed = sets.concat();
        for i in 0..10_000usize {
            let size = MIN_SIZE + i % (MAX_SIZE - MIN_SIZE + 1);
            let password = generate(&options(size, mask)).unwrap();
            let b = password.expor();
            assert_eq!(b.len(), size);
            assert!(
                b.iter().all(|c| allowed.contains(c)),
                "caractere fora dos conjuntos (máscara {mask:04b})"
            );
            for c in &sets {
                assert!(
                    b.iter().any(|x| c.contains(x)),
                    "faltou um conjunto marcado (máscara {mask:04b}, tamanho {size})"
                );
            }
            if mask & 8 == 0 {
                assert!(
                    !b.iter().any(|c| SYMBOLS.contains(c)),
                    "símbolo sem símbolos"
                );
                assert!(b.iter().all(|c| c.is_ascii_alphanumeric()));
            }
        }
    }
}

#[test]
fn v2_26_invalid_size_and_sets() {
    for size in [0, 1, 3, 129, 1_000, usize::MAX] {
        assert_eq!(
            generate(&options(size, 15)).unwrap_err(),
            GeneratorError::InvalidSize
        );
    }
    assert_eq!(
        generate(&options(24, 0)).unwrap_err(),
        GeneratorError::NoSet
    );
    let default = generate(&GeneratorOptions::default()).unwrap();
    assert_eq!(default.len(), DEFAULT_SIZE);
}

/// V2.26 ("the source is the system RNG"): all randomness comes from the source. With
/// the same deterministic source, the same password; and the public function uses
/// `crypto::random` (`getrandom`), checked by the code of `generate` itself.
#[test]
fn v2_26_all_randomness_comes_from_the_source() {
    fn counter() -> impl FnMut(&mut [u8]) -> Result<(), CryptoFailure> {
        let mut n: u8 = 0;
        move |buf: &mut [u8]| {
            for b in buf {
                *b = n;
                n = n.wrapping_mul(13).wrapping_add(7);
            }
            Ok(())
        }
    }
    let o = options(32, 15);
    let a = generate_with(&o, &mut counter()).unwrap();
    let b = generate_with(&o, &mut counter()).unwrap();
    assert_eq!(a.expor(), b.expor());
    // The system source does not repeat.
    let x = generate(&o).unwrap();
    let y = generate(&o).unwrap();
    assert_ne!(x.expor(), y.expor());
    let source = include_str!("../generator.rs");
    assert!(source.contains("generate_with(options, &mut crypto::random)"));
}

#[test]
fn attack_failing_rng_does_not_generate_weak_password() {
    // Without RNG there is no "fallback" password: the generator fails.
    let mut broken = |_: &mut [u8]| Err(CryptoFailure);
    assert_eq!(
        generate_with(&options(24, 15), &mut broken).unwrap_err(),
        GeneratorError::RandomUnavailable
    );
    // Failure midway (after delivering bytes): also fails, without a partial password.
    let mut calls = 0;
    let mut tires = |buf: &mut [u8]| {
        calls += 1;
        if calls > 1 {
            return Err(CryptoFailure);
        }
        buf.fill(1);
        Ok(())
    };
    assert_eq!(
        generate_with(&options(128, 15), &mut tires).unwrap_err(),
        GeneratorError::RandomUnavailable
    );
}

#[test]
fn attack_stuck_rng_does_not_hang() {
    // Always the same high byte: every sample is rejected; the generator stops with an
    // error.
    let mut high = |buf: &mut [u8]| {
        buf.fill(0xff);
        Ok(())
    };
    assert_eq!(
        generate_with(&options(24, 15), &mut high).unwrap_err(),
        GeneratorError::RandomUnavailable
    );
    // Always zero: only "a" comes out, which never meets the 4 sets; stops with an
    // error.
    let mut zero = |buf: &mut [u8]| {
        buf.fill(0);
        Ok(())
    };
    assert_eq!(
        generate_with(&options(24, 15), &mut zero).unwrap_err(),
        GeneratorError::RandomUnavailable
    );
}

/// No modulo bias: with the 90-character alphabet (256 % 90 = 76), each character stays
/// near the mean.
#[test]
fn attack_modulo_bias() {
    let alphabet = [LOWERCASE, UPPERCASE, DIGITS, SYMBOLS].concat();
    let mut count = [0u64; 256];
    let mut total = 0u64;
    for _ in 0..4_000 {
        let s = generate(&options(128, 15)).unwrap();
        for &c in s.expor() {
            count[usize::from(c)] += 1;
            total += 1;
        }
    }
    let mean = total as f64 / alphabet.len() as f64;
    for &c in &alphabet {
        let n = count[usize::from(c)] as f64;
        assert!(
            (n - mean).abs() < mean * 0.08,
            "caractere {:?}: {n} contra média {mean}",
            c as char
        );
    }
    // With a naive `b % 90`, the last 14 characters would come out ~30 % below the
    // mean: the 8 % margin catches it. The standard deviation per character here is
    // ~1.3 % of the mean.
}

#[test]
fn attack_password_debug_and_errors_without_value() {
    let s = generate(&options(64, 15)).unwrap();
    let text = s.expor_texto().unwrap().to_owned();
    for output in [format!("{s:?}"), format!("{s}"), format!("{s:#?}")] {
        assert!(!output.contains(&text));
        assert!(output.contains("••••"));
    }
    for e in [
        GeneratorError::InvalidSize,
        GeneratorError::NoSet,
        GeneratorError::RandomUnavailable,
    ] {
        assert!(!e.to_string().is_empty());
    }
}

#[test]
fn v2_28_generated_password_zeroes() {
    fn zeroes<T: zeroize::ZeroizeOnDrop>(_: &T) {}
    let s = generate(&GeneratorOptions::default()).unwrap();
    zeroes(&s);
}
