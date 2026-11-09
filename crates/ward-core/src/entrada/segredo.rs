//! Valor secreto de uma entrada (S8, S13).
//!
//! - Os bytes zeram ao sair, inclusive a capacidade sobrando do `Vec` (o `zeroize` do
//!   `Vec<u8>` zera a alocação inteira).
//! - Sem `Clone`, `PartialEq`, `Hash` nem `Deref`: copiar ou comparar um segredo é
//!   sempre explícito ([`Segredo::expor`]).
//! - `Debug` e `Display` imprimem `••••` e nunca o valor nem o tamanho.

use core::fmt;

use zeroize::{Zeroize, ZeroizeOnDrop};

/// Valor secreto (senha, token, chave privada, semente TOTP, extra secreto, conteúdo de
/// anexo). Zera ao sair, sem `Clone`.
pub struct Segredo(Vec<u8>);

impl Segredo {
    /// Toma posse do texto, sem cópia. O texto zera quando o segredo sai.
    pub fn de_texto(texto: String) -> Self {
        Self(texto.into_bytes())
    }

    /// Toma posse dos bytes, sem cópia (conteúdo de anexo, que pode ser binário).
    pub fn de_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Os bytes do segredo. Quem chama não deve copiar para fora de um tipo que zera.
    pub fn expor(&self) -> &[u8] {
        &self.0
    }

    /// O segredo como texto, se for UTF-8 válido. Os campos de texto (senha, token…)
    /// sempre são, porque nascem de `String`.
    pub fn expor_texto(&self) -> Option<&str> {
        core::str::from_utf8(&self.0).ok()
    }

    /// Tamanho em bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Vazio?
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Drop for Segredo {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl ZeroizeOnDrop for Segredo {}

impl fmt::Debug for Segredo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Segredo(••••)")
    }
}

impl fmt::Display for Segredo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("••••")
    }
}
