//! Mudar o nível de acesso de uma entrada (Fase 35, S9).
//!
//! O PIN chega da interface (não confiável) e vira um [`Pin`]: zera ao sair, não tem
//! `Clone`, `Display` nem `Serialize`, e o `Debug` não mostra o valor. A leitura
//! (`Deserialize`) já recusa o que não é PIN (4 a 6 dígitos ASCII, como o
//! `ward_core::limits`), então um valor fora do formato nem chega ao daemon. O PIN só
//! sai daqui para o pedido `mudar_nivel` do protocolo; nenhuma resposta o devolve e
//! nenhum log o escreve.

use std::fmt;

use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize};
use ward_core::entry::{AccessLevel, Secret as SegredoCore};
use ward_core::limits::{MAX_PIN_DIGITS, MIN_PIN_DIGITS};
use zeroize::Zeroizing;

use crate::AppError;

/// PIN digitado na interface, só para confirmar a mudança de nível.
pub struct Pin(Zeroizing<String>);

impl Pin {
    /// Confere o formato e toma posse do texto (sem cópia que fique sem zerar).
    pub fn novo(texto: String) -> Result<Pin, AppError> {
        let texto = Zeroizing::new(texto);
        if formato_ok(&texto) {
            Ok(Pin(texto))
        } else {
            Err(AppError::InvalidRequest)
        }
    }

    /// O PIN no tipo do protocolo (também zera ao sair).
    pub(crate) fn para_o_protocolo(&self) -> SegredoCore {
        SegredoCore::from_bytes(self.0.as_bytes().to_vec())
    }
}

fn formato_ok(t: &str) -> bool {
    (MIN_PIN_DIGITS..=MAX_PIN_DIGITS).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_digit())
}

impl fmt::Debug for Pin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Pin(••••)")
    }
}

impl<'de> Deserialize<'de> for Pin {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = Pin;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("PIN de 4 a 6 dígitos")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Pin, E> {
                self.visit_string(v.to_owned())
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Pin, E> {
                // A mensagem de erro é fixa: não ecoa o que veio.
                Pin::novo(v).map_err(|_| E::custom("PIN fora do formato"))
            }
            // Número no lugar do texto: o erro padrão do serde ecoaria o valor.
            fn visit_u64<E: de::Error>(self, _: u64) -> Result<Pin, E> {
                Err(E::custom("PIN fora do formato"))
            }
            fn visit_i64<E: de::Error>(self, _: i64) -> Result<Pin, E> {
                Err(E::custom("PIN fora do formato"))
            }
            fn visit_f64<E: de::Error>(self, _: f64) -> Result<Pin, E> {
                Err(E::custom("PIN fora do formato"))
            }
        }
        // `any`: um número cai no `visit_u64` (erro fixo), não no erro padrão com o valor.
        d.deserialize_any(V)
    }
}

/// Nível pedido pela interface, pelo nome do protocolo (`livre`, `com-aprovacao`,
/// `bloqueado`).
pub fn nivel_de_nome(nome: &str) -> Result<AccessLevel, AppError> {
    AccessLevel::from_name(nome).ok_or(AppError::InvalidRequest)
}

/// Resposta do `mudar_nivel`: o nível que a entrada passou a ter. Sem PIN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NivelMudado {
    pub entrada: String,
    pub nivel: &'static str,
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ataque_pin_fora_do_formato_e_recusado_sem_eco() {
        for ruim in [
            "",
            "123",
            "1234567",
            "13579a",
            " 135790",
            "135790\n",
            "１３５７９０",
            "13579\u{0}",
            "-1357",
            "+13579",
        ] {
            assert_eq!(
                Pin::novo(ruim.to_owned()).unwrap_err(),
                AppError::InvalidRequest,
                "{ruim:?}"
            );
            let json = serde_json::to_string(ruim).unwrap();
            let e = serde_json::from_str::<Pin>(&json).unwrap_err().to_string();
            assert!(e.contains("PIN fora do formato"), "{e}");
            assert!(
                ruim.len() < 4 || !e.contains(ruim),
                "erro ecoa o valor: {e}"
            );
        }
        // Número em vez de texto, objeto, nulo: recusados.
        for json in ["135790", "{}", "null", "[\"135790\"]"] {
            let e = serde_json::from_str::<Pin>(json).unwrap_err().to_string();
            assert!(!e.contains("135790"), "{e}");
        }
        assert!(serde_json::from_str::<Pin>("\"1357\"").is_ok());
        assert!(serde_json::from_str::<Pin>("\"135790\"").is_ok());
    }

    #[test]
    fn ataque_debug_do_pin_nao_mostra_o_valor() {
        let p = Pin::novo("135790".to_owned()).unwrap();
        let texto = format!("{p:?} {:?}", Some(&p));
        assert!(!texto.contains("135790"));
        assert!(texto.contains("••••"));
    }

    #[test]
    fn nivel_so_pelos_tres_nomes_do_protocolo() {
        assert_eq!(nivel_de_nome("livre"), Ok(AccessLevel::Free));
        assert_eq!(nivel_de_nome("com-aprovacao"), Ok(AccessLevel::Approval));
        assert_eq!(nivel_de_nome("bloqueado"), Ok(AccessLevel::Blocked));
        for ruim in [
            "",
            "Livre",
            "com aprovação",
            "com_aprovacao",
            "admin",
            "livre ",
        ] {
            assert_eq!(
                nivel_de_nome(ruim),
                Err(AppError::InvalidRequest),
                "{ruim:?}"
            );
        }
    }
}
