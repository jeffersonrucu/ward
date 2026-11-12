//! Secret value in transit on the app's Rust side (only the "copy", D4).
//!
//! Zeroed on drop, does not implement `Clone`, `Display` nor `Serialize`, and the `Debug`
//! does not show the value. There is no way to return it to the interface: no command
//! returns this type.

use std::fmt;

use zeroize::Zeroizing;

/// Bytes of a secret coming from the daemon.
pub struct Secret(Zeroizing<Vec<u8>>);

impl Secret {
    /// Takes ownership of the bytes (the original vector is moved, not copied).
    pub fn new(bytes: Vec<u8>) -> Self {
        Secret(Zeroizing::new(bytes))
    }

    /// The bytes, only for whoever hands the value to the clipboard.
    pub(crate) fn expose(&self) -> &[u8] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(••••)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attack_debug_does_not_show_the_value() {
        let s = Secret::new(b"falso-segredo-de-teste-123".to_vec());
        let text = format!("{s:?} {:?}", vec![&s]);
        assert!(!text.contains("falso"));
        assert!(!text.contains("123"));
        assert!(text.contains("••••"));
    }
}
