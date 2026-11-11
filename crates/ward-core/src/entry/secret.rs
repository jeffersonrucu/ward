//! Secret value of an entry (S8, S13).
//!
//! - The bytes are zeroed on drop, including the spare capacity of the `Vec` (the
//!   `zeroize` of `Vec<u8>` zeroes the whole allocation).
//! - No `Clone`, `PartialEq`, `Hash` or `Deref`: copying or comparing a secret is always
//!   explicit ([`Secret::expose`]).
//! - `Debug` and `Display` print `••••` and never the value or the length.

use core::fmt;

use zeroize::{Zeroize, ZeroizeOnDrop};

/// Secret value (password, token, private key, TOTP seed, secret extra, attachment
/// content). Zeroed on drop, no `Clone`.
pub struct Secret(Vec<u8>);

impl Secret {
    /// Takes ownership of the text, without copying. The text is zeroed when the secret
    /// is dropped.
    pub fn from_text(text: String) -> Self {
        Self(text.into_bytes())
    }

    /// Takes ownership of the bytes, without copying (attachment content, which may be
    /// binary).
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// The bytes of the secret. The caller must not copy them out of a type that zeroes.
    pub fn expose(&self) -> &[u8] {
        &self.0
    }

    /// The secret as text, if it is valid UTF-8. Text fields (password, token…) always
    /// are, because they are born from a `String`.
    pub fn expose_text(&self) -> Option<&str> {
        core::str::from_utf8(&self.0).ok()
    }

    /// Length in bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Empty?
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl ZeroizeOnDrop for Secret {}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(••••)")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("••••")
    }
}
