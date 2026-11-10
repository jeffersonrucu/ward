//! Valor secreto em trânsito no lado Rust do app (só o "copiar", D4).
//!
//! Zera ao sair, não implementa `Clone`, `Display` nem `Serialize`, e o `Debug` não
//! mostra o valor. Não há como devolvê-lo à interface: nenhum comando retorna este tipo.

use std::fmt;

use zeroize::Zeroizing;

/// Bytes de um segredo vindo do daemon.
pub struct Segredo(Zeroizing<Vec<u8>>);

impl Segredo {
    /// Toma posse dos bytes (o vetor original é movido, não copiado).
    pub fn novo(bytes: Vec<u8>) -> Self {
        Segredo(Zeroizing::new(bytes))
    }

    /// Os bytes, só para quem entrega o valor à área de transferência.
    pub(crate) fn expor(&self) -> &[u8] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Segredo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Segredo(••••)")
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ataque_debug_nao_mostra_o_valor() {
        let s = Segredo::novo(b"falso-segredo-de-teste-123".to_vec());
        let texto = format!("{s:?} {:?}", vec![&s]);
        assert!(!texto.contains("falso"));
        assert!(!texto.contains("123"));
        assert!(texto.contains("••••"));
    }
}
