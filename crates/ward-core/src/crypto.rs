//! Format primitives, only through audited libraries (S6, V2.7).
//!
//! This module does **not** implement crypto: it only calls `argon2` (Argon2id, RFC
//! 9106), `chacha20poly1305` (XChaCha20-Poly1305, `draft-irtf-cfrg-xchacha`) and
//! `getrandom` (system randomness). The tag comparison is done inside
//! `chacha20poly1305`. What stays here is the glue: fixed sizes, buffers that zero on
//! drop and a single error that does not say what failed.
//!
//! The vault file format uses only these functions; the known vectors (V2.5) go through
//! them too.

use core::fmt;

use argon2::password_hash::phc::{Output, ParamsString, PasswordHash, Salt};
use argon2::{ARGON2ID_IDENT, Algorithm, Argon2, AssociatedData, Block, ParamsBuilder, Version};
use chacha20poly1305::aead::AeadInOut;
use chacha20poly1305::{Key as AeadKey, KeyInit, Tag, XChaCha20Poly1305, XNonce};
use zeroize::{ZeroizeOnDrop, Zeroizing};

use crate::formato::ParametrosKdf;
use crate::limits::MAX_PIN_HASH;

/// Key size (derived KEK and data key), in bytes.
pub const KEY_SIZE: usize = 32;
/// XChaCha20-Poly1305 nonce size, in bytes.
pub const NONCE_SIZE: usize = 24;
/// Poly1305 tag size, in bytes.
pub const TAG_SIZE: usize = 16;

/// 256-bit symmetric key (the data key and the KEK) that zeroes on drop. No `Debug`,
/// `Display` or `Clone`.
///
/// It lives on the heap (D40): inside it is a `Box<Zeroizing<[u8; 32]>>`. Moving a
/// `Key` moves only the pointer, and the 32 bytes live in a single block, which zeroes
/// when freed (the V2.33 test allocator sees it). A `Zeroizing<[u8; 32]>` by value
/// would leave a copy on the stack at each move, which nobody zeroes. The own type
/// (instead of an alias) exists to declare [`ZeroizeOnDrop`] (V2.28), which `zeroize`
/// does not give to any `Box`.
pub(crate) struct Key(Box<Zeroizing<[u8; KEY_SIZE]>>);

// D40, V2.46: the `Key` has the size of a pointer (the bytes never go by value).
const _: () = assert!(core::mem::size_of::<Key>() == core::mem::size_of::<usize>());

// The only field is a `Zeroizing` on the heap: zeroes on drop (V2.28).
impl ZeroizeOnDrop for Key {}

impl core::ops::Deref for Key {
    type Target = [u8; KEY_SIZE];

    fn deref(&self) -> &[u8; KEY_SIZE] {
        &self.0
    }
}

impl core::ops::DerefMut for Key {
    fn deref_mut(&mut self) -> &mut [u8; KEY_SIZE] {
        &mut self.0
    }
}

impl Key {
    /// Zeroed key, already on the heap: the secret is written later, in the place where
    /// it will stay. (The zeros of `Box::new` are not a secret.)
    fn zeroed() -> Self {
        Self(Box::new(Zeroizing::new([0u8; KEY_SIZE])))
    }

    /// Key copied from `bytes` (what `open` decrypted) straight to the heap.
    pub(crate) fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoFailure> {
        if bytes.len() != KEY_SIZE {
            return Err(CryptoFailure);
        }
        let mut key = Self::zeroed();
        key.copy_from_slice(bytes);
        Ok(key)
    }
}

/// Crypto failure. It does not say whether it was the tag, the size or the parameter:
/// the caller turns it into a generic format error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CryptoFailure;

/// Argon2id v0x13 (RFC 9106) with all the inputs of the RFC. The format calls it with
/// empty `secret` and `assoc_data`; the RFC vectors (V2.5) go through here with them
/// filled in.
///
/// The working memory is allocated here and zeroed on drop (`argon2` 0.6 does not zero
/// the memory it allocates itself in `hash_password_into`).
pub(crate) fn argon2id(
    password: &[u8],
    salt: &[u8],
    secret: &[u8],
    assoc_data: &[u8],
    cost: &ParametrosKdf,
    output: &mut [u8],
) -> Result<(), CryptoFailure> {
    let mut builder = ParamsBuilder::new();
    builder
        .m_cost(cost.memoria_kib)
        .t_cost(cost.passes)
        .p_cost(cost.paralelismo)
        .output_len(output.len());
    if !assoc_data.is_empty() {
        builder.data(AssociatedData::new(assoc_data).map_err(|_| CryptoFailure)?);
    }
    let params = builder.build().map_err(|_| CryptoFailure)?;
    let context = if secret.is_empty() {
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
    } else {
        Argon2::new_with_secret(secret, Algorithm::Argon2id, Version::V0x13, params)
            .map_err(|_| CryptoFailure)?
    };
    let blocks = context.params().block_count();
    let mut memory: Zeroizing<Vec<Block>> = Zeroizing::new(Vec::new());
    memory
        .try_reserve_exact(blocks)
        .map_err(|_| CryptoFailure)?;
    memory.resize(blocks, Block::new());
    let result = context.hash_password_into_with_memory(password, salt, output, &mut memory[..]);
    if result.is_err() {
        zeroize::Zeroize::zeroize(output);
        return Err(CryptoFailure);
    }
    Ok(())
}

/// Derives the 32-byte KEK from the master password (Argon2id without secret or
/// associated data). The parameter limits are checked by the format before.
pub(crate) fn derive_kek(
    password: &[u8],
    salt: &[u8],
    cost: &ParametrosKdf,
) -> Result<Key, CryptoFailure> {
    let mut kek = Key::zeroed();
    argon2id(password, salt, &[], &[], cost, &mut kek[..])?;
    Ok(kek)
}

fn cipher(key: &[u8; KEY_SIZE]) -> Result<XChaCha20Poly1305, CryptoFailure> {
    // Reference to the key without a copy; the copy inside `XChaCha20Poly1305` zeroes
    // on drop (feature `zeroize`).
    let key = <&AeadKey>::try_from(&key[..]).map_err(|_| CryptoFailure)?;
    Ok(XChaCha20Poly1305::new(key))
}

/// XChaCha20-Poly1305: encrypts `text` with `aad` authenticated. Output = ciphertext ‖
/// tag.
///
/// The output buffer is allocated at its final size before receiving the text, so
/// there is no reallocation that leaves a plaintext copy in freed memory.
pub(crate) fn seal(
    key: &[u8; KEY_SIZE],
    nonce: &[u8; NONCE_SIZE],
    aad: &[u8],
    text: &[u8],
) -> Result<Vec<u8>, CryptoFailure> {
    let total = text.len().checked_add(TAG_SIZE).ok_or(CryptoFailure)?;
    let nonce = <&XNonce>::try_from(&nonce[..]).map_err(|_| CryptoFailure)?;
    let cipher = cipher(key)?;
    let mut output: Vec<u8> = Vec::new();
    output.try_reserve_exact(total).map_err(|_| CryptoFailure)?;
    output.extend_from_slice(text);
    match cipher.encrypt_inout_detached(nonce, aad, (&mut output[..]).into()) {
        Ok(tag) => {
            output.extend_from_slice(tag.as_slice());
            Ok(output)
        }
        Err(_) => {
            zeroize::Zeroize::zeroize(&mut output);
            Err(CryptoFailure)
        }
    }
}

/// XChaCha20-Poly1305: verifies the tag (inside `chacha20poly1305`) and decrypts. The
/// plaintext only exists in a buffer that zeroes on drop, and is only returned if the
/// tag matches.
pub(crate) fn open(
    key: &[u8; KEY_SIZE],
    nonce: &[u8; NONCE_SIZE],
    aad: &[u8],
    ciphertext: &[u8],
) -> Result<Zeroizing<Vec<u8>>, CryptoFailure> {
    let body = ciphertext
        .len()
        .checked_sub(TAG_SIZE)
        .ok_or(CryptoFailure)?;
    let (msg, tag) = ciphertext.split_at(body);
    let tag = <&Tag>::try_from(tag).map_err(|_| CryptoFailure)?;
    let nonce = <&XNonce>::try_from(&nonce[..]).map_err(|_| CryptoFailure)?;
    let cipher = cipher(key)?;
    let mut text: Zeroizing<Vec<u8>> = Zeroizing::new(Vec::new());
    text.try_reserve_exact(body).map_err(|_| CryptoFailure)?;
    text.extend_from_slice(msg);
    cipher
        .decrypt_inout_detached(nonce, aad, (&mut text[..]).into(), tag)
        .map_err(|_| CryptoFailure)?;
    Ok(text)
}

/// Random bytes from the operating system (`getrandom`). Fails instead of going on with
/// weak randomness.
pub(crate) fn random(destination: &mut [u8]) -> Result<(), CryptoFailure> {
    getrandom::fill(destination).map_err(|_| CryptoFailure)
}

/// New random key (data key).
pub(crate) fn random_key() -> Result<Key, CryptoFailure> {
    let mut key = Key::zeroed();
    random(&mut key[..])?;
    Ok(key)
}

// ---- PIN hash (D30) ----
//
// The hash is computed by [`argon2id`] (working memory zeroed, note of Phase 13), and
// the PHC text is built and read by the `phc` types (the same as `argon2`'s). The hash
// comparison is the `CtEq` of `ctutils`, in constant time (D40). None of this is done
// by hand. The `Copy` types of `phc` (`Output`, `Salt`) only serve to read and write
// the stored PHC text, which already is the content of the `HashPin`; the recomputed
// output stays in a `Zeroizing`.

/// Salt of the PIN hash, in bytes (the one PHC recommends).
pub(crate) const PIN_SALT_SIZE: usize = 16;
/// Output of the PIN hash, in bytes.
pub(crate) const PIN_HASH_SIZE: usize = 32;

/// PIN hash already read from the PHC text: parameters, salt and output. Does not leave
/// the crate.
pub(crate) struct PinPhc {
    pub(crate) cost: ParametrosKdf,
    salt: Salt,
    hash: Output,
}

/// Writes into a fixed buffer, without allocating; refuses what does not fit.
struct FixedBuffer<'b> {
    buf: &'b mut [u8],
    used: usize,
}

impl fmt::Write for FixedBuffer<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self
            .used
            .checked_add(s.len())
            .filter(|f| *f <= self.buf.len())
            .ok_or(fmt::Error)?;
        self.buf[self.used..end].copy_from_slice(s.as_bytes());
        self.used = end;
        Ok(())
    }
}

/// Canonical PHC text of (parameters, salt, output), written into `buf`. Returns how
/// many bytes it used. Does not allocate.
fn write_phc(
    cost: &ParametrosKdf,
    salt: &[u8],
    output: &[u8],
    buf: &mut [u8; MAX_PIN_HASH],
) -> Result<usize, CryptoFailure> {
    let mut params = ParamsString::new();
    for (name, value) in [
        ("m", cost.memoria_kib),
        ("t", cost.passes),
        ("p", cost.paralelismo),
    ] {
        params.add_decimal(name, value).map_err(|_| CryptoFailure)?;
    }
    let phc = PasswordHash {
        algorithm: ARGON2ID_IDENT,
        version: Some(Version::V0x13.into()),
        params,
        salt: Some(Salt::new(salt).map_err(|_| CryptoFailure)?),
        hash: Some(Output::new(output).map_err(|_| CryptoFailure)?),
    };
    let mut writer = FixedBuffer { buf, used: 0 };
    fmt::write(&mut writer, format_args!("{phc}")).map_err(|_| CryptoFailure)?;
    Ok(writer.used)
}

/// Argon2id hash of the PIN with a new random salt, in PHC text. The text is born in an
/// allocation of the exact size, which zeroes on drop.
pub(crate) fn hash_pin(
    pin: &[u8],
    cost: &ParametrosKdf,
) -> Result<Zeroizing<String>, CryptoFailure> {
    let mut salt = [0u8; PIN_SALT_SIZE];
    random(&mut salt)?;
    let mut output: Zeroizing<[u8; PIN_HASH_SIZE]> = Zeroizing::new([0u8; PIN_HASH_SIZE]);
    argon2id(pin, &salt, &[], &[], cost, &mut output[..])?;
    let mut buf: Zeroizing<[u8; MAX_PIN_HASH]> = Zeroizing::new([0u8; MAX_PIN_HASH]);
    let n = write_phc(cost, &salt, &output[..], &mut buf)?;
    let text = core::str::from_utf8(&buf[..n]).map_err(|_| CryptoFailure)?;
    let mut s: Zeroizing<String> = Zeroizing::new(String::new());
    s.try_reserve_exact(n).map_err(|_| CryptoFailure)?;
    s.push_str(text);
    Ok(s)
}

/// Reads a PHC text of a PIN hash and checks everything, without allocating: Argon2id
/// v0x13, only `m`, `t` and `p`, within `[minimum, maximum]`, salt of [`PIN_SALT_SIZE`]
/// bytes, output of [`PIN_HASH_SIZE`] bytes and canonical text (written again, it gives
/// the same bytes). An inflated hash does not make Argon2 run expensive; a downgraded
/// one does not run cheap.
pub(crate) fn read_pin_phc(
    text: &[u8],
    minimum: &ParametrosKdf,
    maximum: &ParametrosKdf,
) -> Option<PinPhc> {
    if text.len() > MAX_PIN_HASH || !text.is_ascii() {
        return None;
    }
    let text = core::str::from_utf8(text).ok()?;
    let phc = PasswordHash::new(text).ok()?;
    if phc.algorithm != ARGON2ID_IDENT || phc.version != Some(Version::V0x13.into()) {
        return None;
    }
    if phc.params.iter().count() != 3 {
        return None;
    }
    let cost = ParametrosKdf {
        memoria_kib: phc.params.get_decimal("m")?,
        passes: phc.params.get_decimal("t")?,
        paralelismo: phc.params.get_decimal("p")?,
    };
    let within = |v: u32, min: u32, max: u32| (min..=max).contains(&v);
    if !within(cost.memoria_kib, minimum.memoria_kib, maximum.memoria_kib)
        || !within(cost.passes, minimum.passes, maximum.passes)
        || !within(
            cost.paralelismo,
            minimum.paralelismo.max(1),
            maximum.paralelismo,
        )
        || u64::from(cost.memoria_kib) < 8 * u64::from(cost.paralelismo)
    {
        return None;
    }
    let salt = phc.salt?;
    let hash = phc.hash?;
    if salt.len() != PIN_SALT_SIZE || hash.len() != PIN_HASH_SIZE {
        return None;
    }
    let mut buf: Zeroizing<[u8; MAX_PIN_HASH]> = Zeroizing::new([0u8; MAX_PIN_HASH]);
    let n = write_phc(&cost, &salt, hash.as_bytes(), &mut buf).ok()?;
    if &buf[..n] != text.as_bytes() {
        return None;
    }
    Some(PinPhc { cost, salt, hash })
}

/// Verifies the PIN against the hash: recomputes by [`argon2id`] with the salt and the
/// parameters of the hash, in a `Zeroizing`, and compares with the stored output by the
/// `CtEq` of `ctutils` (constant time, by the library; D40, V2.7).
pub(crate) fn verify_pin(pin: &[u8], phc: &PinPhc) -> bool {
    let mut output: Zeroizing<[u8; PIN_HASH_SIZE]> = Zeroizing::new([0u8; PIN_HASH_SIZE]);
    if argon2id(pin, &phc.salt, &[], &[], &phc.cost, &mut output[..]).is_err() {
        return false;
    }
    constant_time_eq(&output[..], phc.hash.as_bytes())
}

/// Byte equality in constant time, by the `CtEq` of `ctutils` (the same one `phc` uses).
/// Different lengths give `false`.
pub(crate) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    use ctutils::CtEq;
    a.ct_eq(b).to_bool()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cost(memory_kib: u32, passes: u32, parallelism: u32) -> ParametrosKdf {
        ParametrosKdf {
            memoria_kib: memory_kib,
            passes,
            paralelismo: parallelism,
        }
    }

    fn hex(s: &str) -> Vec<u8> {
        let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    /// RFC 9106, section 5.3 (Argon2id, version 0x13).
    #[test]
    fn v2_5_argon2id_rfc9106() {
        let mut output = [0u8; 32];
        argon2id(
            &[0x01; 32],
            &[0x02; 16],
            &[0x03; 8],
            &[0x04; 12],
            &cost(32, 3, 4),
            &mut output,
        )
        .unwrap();
        assert_eq!(
            output.to_vec(),
            hex("0d 64 0d f5 8d 78 76 6c 08 c0 37 a3 4a 8b 53 c9
                 d0 1e f0 45 2d 75 b6 5e b5 25 20 e9 6b 01 e6 59")
        );
    }

    /// `derive_kek` is `argon2id` without secret or associated data (it is the path the
    /// format uses).
    #[test]
    fn derive_kek_is_argon2id_without_extras() {
        let mut expected = [0u8; 32];
        argon2id(
            b"senha",
            &[0x02; 32],
            &[],
            &[],
            &cost(64, 3, 4),
            &mut expected,
        )
        .unwrap();
        let kek = derive_kek(b"senha", &[0x02; 32], &cost(64, 3, 4)).unwrap();
        assert_eq!(&kek[..], &expected[..]);
        assert_ne!(&kek[..], &[0u8; 32][..]);
    }

    // `draft-irtf-cfrg-xchacha-03`, appendix A.3.1.
    const XC_KEY: &str = "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f";
    const XC_NONCE: &str = "404142434445464748494a4b4c4d4e4f5051525354555657";
    const XC_AAD: &str = "50515253c0c1c2c3c4c5c6c7";
    const XC_TEXT: &[u8] = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
    const XC_CIPHERTEXT: &str = "bd6d179d3e83d43b9576579493c0e939572a1700252bfaccbed2902c21396cbb
        731c7f1b0b4aa6440bf3a82f4eda7e39ae64c6708c54c216cb96b72e1213b452
        2f8c9ba40db5d945b11b69b982c1bb9e3f3fac2bc369488f76b2383565d3fff9
        21f9664c97637da9768812f615c68b13b52e";
    const XC_TAG: &str = "c0875924c1c7987947deafd8780acf49";

    fn xc() -> ([u8; 32], [u8; 24], Vec<u8>, Vec<u8>) {
        let key: [u8; 32] = hex(XC_KEY).try_into().unwrap();
        let nonce: [u8; 24] = hex(XC_NONCE).try_into().unwrap();
        let mut expected = hex(XC_CIPHERTEXT);
        expected.extend(hex(XC_TAG));
        (key, nonce, hex(XC_AAD), expected)
    }

    #[test]
    fn v2_5_xchacha20poly1305_draft_seal() {
        let (key, nonce, aad, expected) = xc();
        assert_eq!(seal(&key, &nonce, &aad, XC_TEXT).unwrap(), expected);
    }

    #[test]
    fn v2_5_xchacha20poly1305_draft_open() {
        let (key, nonce, aad, ciphertext) = xc();
        assert_eq!(&open(&key, &nonce, &aad, &ciphertext).unwrap()[..], XC_TEXT);
    }

    /// Attack: any bit flipped in the ciphertext, the tag, the AAD or the nonce makes
    /// `open` refuse, and the error is the same.
    #[test]
    fn open_refuses_any_flipped_bit() {
        let (key, nonce, aad, ciphertext) = xc();
        for i in 0..ciphertext.len() {
            let mut c = ciphertext.clone();
            c[i] ^= 1 << (i % 8);
            assert_eq!(open(&key, &nonce, &aad, &c), Err(CryptoFailure));
        }
        for i in 0..aad.len() {
            let mut a = aad.clone();
            a[i] ^= 0x01;
            assert_eq!(open(&key, &nonce, &a, &ciphertext), Err(CryptoFailure));
        }
        for i in 0..nonce.len() {
            let mut n = nonce;
            n[i] ^= 0x80;
            assert_eq!(open(&key, &n, &aad, &ciphertext), Err(CryptoFailure));
        }
        let mut k = key;
        k[0] ^= 0x01;
        assert_eq!(open(&k, &nonce, &aad, &ciphertext), Err(CryptoFailure));
        for size in 0..ciphertext.len() {
            assert_eq!(
                open(&key, &nonce, &aad, &ciphertext[..size]),
                Err(CryptoFailure)
            );
        }
    }

    #[test]
    fn random_does_not_repeat() {
        let a = random_key().unwrap();
        let b = random_key().unwrap();
        assert_ne!(&a[..], &b[..]);
        assert_ne!(&a[..], &[0u8; 32][..]);
    }
}
