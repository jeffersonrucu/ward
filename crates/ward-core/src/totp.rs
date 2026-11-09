//! TOTP (RFC 6238 sobre o HOTP do RFC 4226), R8: o cofre gera o código; quem recebe o
//! código (agente, extensão, app) nunca recebe a semente.
//!
//! - HMAC-SHA1 só pelos crates `hmac` e `sha1` do RustCrypto (S6). Aqui fica só a cola:
//!   contador em big-endian, truncamento dinâmico (RFC 4226 §5.3) e os dígitos.
//! - A semente decodificada vive num `Zeroizing<Vec<u8>>`; o `Debug` de [`Totp`] imprime
//!   `••••`. O estado do HMAC zera ao sair (feature `zeroize` de `hmac`/`sha1`) e a saída
//!   do HMAC também.
//! - Os erros são textos fixos: a semente nunca entra numa mensagem.
//! - O código não é segredo do cofre (o agente o recebe, R8), por isso [`CodigoTotp`]
//!   imprime os dígitos.

use core::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;
use zeroize::{Zeroize, Zeroizing};

use crate::entrada::Segredo;

/// Período do código, em segundos (RFC 6238, padrão de todo autenticador).
pub const PERIODO: u64 = 30;
/// Dígitos do código entregue (`WARD_TOTP`, extensão, app).
pub const DIGITOS_PADRAO: u32 = 6;
/// Menor semente aceita, em caracteres base32 (80 bits), igual à validação da entrada.
pub const MIN_SEMENTE: usize = 16;
/// Maior semente aceita, em caracteres base32, igual à validação da entrada.
pub const MAX_SEMENTE: usize = 128;

/// Erro do TOTP. Só textos fixos, nunca a semente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroTotp {
    /// A semente não é base32 (A–Z, 2–7, `=` só no fim) ou tem tamanho fora de 16–128.
    SementeInvalida,
    /// Número de dígitos fora de 6–8.
    DigitosInvalidos,
    /// O relógio do sistema está antes de 1970.
    RelogioInvalido,
}

impl fmt::Display for ErroTotp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SementeInvalida => {
                "semente TOTP inválida: use base32 (A–Z e 2–7) com 16 a 128 caracteres"
            }
            Self::DigitosInvalidos => "o código TOTP tem de 6 a 8 dígitos",
            Self::RelogioInvalido => "o relógio do sistema está antes de 1970",
        })
    }
}

impl std::error::Error for ErroTotp {}

/// Gerador de códigos de uma semente. Sem `Clone`; a semente zera ao sair.
pub struct Totp {
    chave: Zeroizing<Vec<u8>>,
    digitos: u32,
}

impl fmt::Debug for Totp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Totp")
            .field("semente", &"••••")
            .field("digitos", &self.digitos)
            .finish()
    }
}

/// Código de um período. Não é segredo do cofre (R8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodigoTotp {
    codigo: String,
    restante: u64,
}

impl CodigoTotp {
    /// Os dígitos, com zeros à esquerda.
    pub fn codigo(&self) -> &str {
        &self.codigo
    }

    /// Segundos até o código mudar (1 a 30), para a contagem do app.
    pub fn restante(&self) -> u64 {
        self.restante
    }
}

impl fmt::Display for CodigoTotp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.codigo)
    }
}

impl Totp {
    /// Lê a semente base32 da entrada (6 dígitos, 30 s).
    pub fn de_semente(semente: &Segredo) -> Result<Self, ErroTotp> {
        Self::com_digitos(semente, DIGITOS_PADRAO)
    }

    /// Lê a semente base32 com outro número de dígitos (6 a 8).
    pub fn com_digitos(semente: &Segredo, digitos: u32) -> Result<Self, ErroTotp> {
        if !(6..=8).contains(&digitos) {
            return Err(ErroTotp::DigitosInvalidos);
        }
        let chave = decodificar_base32(semente.expor())?;
        Ok(Self { chave, digitos })
    }

    /// Código do período que contém `segundos_unix`.
    pub fn codigo_em(&self, segundos_unix: u64) -> CodigoTotp {
        let contador = segundos_unix / PERIODO;
        let restante = PERIODO - segundos_unix % PERIODO;
        let valor = hotp(&self.chave, contador) % 10u32.pow(self.digitos);
        CodigoTotp {
            codigo: format!("{valor:0width$}", width = self.digitos as usize),
            restante,
        }
    }

    /// Código de agora, pelo relógio do sistema.
    pub fn codigo_agora(&self) -> Result<CodigoTotp, ErroTotp> {
        let agora = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ErroTotp::RelogioInvalido)?;
        Ok(self.codigo_em(agora.as_secs()))
    }
}

/// HOTP (RFC 4226 §5.3): HMAC-SHA1 do contador e truncamento dinâmico, em 31 bits.
fn hotp(chave: &[u8], contador: u64) -> u32 {
    // HMAC aceita chave de qualquer tamanho; `new_from_slice` não falha para HMAC.
    let mut mac = match <Hmac<Sha1> as KeyInit>::new_from_slice(chave) {
        Ok(m) => m,
        Err(_) => unreachable!("HMAC aceita chave de qualquer tamanho"),
    };
    mac.update(&contador.to_be_bytes());
    let mut saida: [u8; 20] = mac.finalize().into_bytes().into();
    let deslocamento = (saida[19] & 0x0f) as usize;
    let trecho = [
        saida[deslocamento],
        saida[deslocamento + 1],
        saida[deslocamento + 2],
        saida[deslocamento + 3],
    ];
    saida.zeroize();
    u32::from_be_bytes(trecho) & 0x7fff_ffff
}

/// Valor de um caractere base32 (RFC 4648 §6), ou `None`.
fn valor_base32(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'2'..=b'7' => Some(c - b'2' + 26),
        _ => None,
    }
}

/// Decodifica base32 maiúsculo, com `=` opcional só no fim; os bits que sobram no fim
/// (menos de 8) são descartados, como no `oathtool` e nos autenticadores. Mesmas regras
/// de `semente_totp_valida` em `entrada::validacao`.
fn decodificar_base32(texto: &[u8]) -> Result<Zeroizing<Vec<u8>>, ErroTotp> {
    let uteis = texto.iter().take_while(|&&c| c != b'=').count();
    let (dados, enchimento) = texto.split_at(uteis);
    if !(MIN_SEMENTE..=MAX_SEMENTE).contains(&uteis) || enchimento.iter().any(|&c| c != b'=') {
        return Err(ErroTotp::SementeInvalida);
    }
    let mut saida = Zeroizing::new(Vec::with_capacity(uteis * 5 / 8));
    let mut acumulado: u32 = 0;
    let mut bits: u32 = 0;
    for &c in dados {
        let v = valor_base32(c).ok_or(ErroTotp::SementeInvalida)?;
        acumulado = ((acumulado << 5) | u32::from(v)) & 0x1fff;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            saida.push((acumulado >> bits) as u8);
        }
    }
    acumulado.zeroize();
    Ok(saida)
}

#[cfg(test)]
mod testes;
