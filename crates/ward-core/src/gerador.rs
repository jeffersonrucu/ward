//! Gerador de senha (R10): tamanho de 4 a 128 e conjuntos configuráveis (há painéis que
//! não aceitam símbolo).
//!
//! - A fonte é só o RNG do sistema (`getrandom`, por `cripto::aleatorio`). Se ele falhar,
//!   o gerador falha: não existe RNG reserva nem semente fixa.
//! - Cada caractere sai uniforme do alfabeto marcado, por rejeição (byte acima do maior
//!   múltiplo do tamanho do alfabeto é descartado), sem viés de módulo.
//! - "Pelo menos um de cada conjunto marcado" também por rejeição: a senha inteira é
//!   sorteada de novo até ter todos. Isso mantém a distribuição uniforme entre as senhas
//!   válidas (sem posição fixa para cada conjunto).
//! - A senha sai como [`Segredo`]; os bytes aleatórios e as tentativas descartadas zeram.

use core::fmt;

use zeroize::{Zeroize, Zeroizing};

use crate::cripto::{self, FalhaCripto};
use crate::entrada::Segredo;

/// Menor tamanho aceito.
pub const TAMANHO_MIN: usize = 4;
/// Maior tamanho aceito.
pub const TAMANHO_MAX: usize = 128;
/// Tamanho padrão do app (bate com o design: "24 caracteres").
pub const TAMANHO_PADRAO: usize = 24;

/// Letras minúsculas.
pub const MINUSCULAS: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
/// Letras maiúsculas.
pub const MAIUSCULAS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
/// Dígitos.
pub const DIGITOS: &[u8] = b"0123456789";
/// Símbolos ASCII. Ficam de fora espaço, aspas (`"` `'`), crase e barra invertida: eles
/// quebram aspas de shell, `.env` e arquivos de configuração onde a senha vai parar.
pub const SIMBOLOS: &[u8] = b"!#$%&()*+,-./:;<=>?@[]^_{|}~";

/// Teto de tentativas para cumprir "um de cada conjunto". No pior caso (tamanho 4 com os
/// 4 conjuntos) a chance por tentativa é ~7 %, então 10 000 tentativas falhando é
/// impossível na prática; o teto só impede laço sem fim com um RNG quebrado.
const MAX_TENTATIVAS: u32 = 10_000;

/// Opções do gerador. Padrão: 24 caracteres com os 4 conjuntos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpcoesGerador {
    /// Tamanho, de 4 a 128.
    pub tamanho: usize,
    /// a–z.
    pub minusculas: bool,
    /// A–Z.
    pub maiusculas: bool,
    /// 0–9.
    pub digitos: bool,
    /// [`SIMBOLOS`].
    pub simbolos: bool,
}

impl Default for OpcoesGerador {
    fn default() -> Self {
        Self {
            tamanho: TAMANHO_PADRAO,
            minusculas: true,
            maiusculas: true,
            digitos: true,
            simbolos: true,
        }
    }
}

/// Erro do gerador, em português, sem dado gerado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroGerador {
    /// Tamanho fora de 4–128.
    TamanhoInvalido,
    /// Nenhum conjunto marcado.
    NenhumConjunto,
    /// O RNG do sistema não entregou bytes.
    AleatorioIndisponivel,
}

impl fmt::Display for ErroGerador {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::TamanhoInvalido => "o tamanho da senha vai de 4 a 128 caracteres",
            Self::NenhumConjunto => "marque pelo menos um conjunto de caracteres",
            Self::AleatorioIndisponivel => "o gerador aleatório do sistema não respondeu",
        })
    }
}

impl std::error::Error for ErroGerador {}

impl OpcoesGerador {
    /// Os conjuntos marcados, na ordem fixa.
    fn conjuntos(&self) -> Vec<&'static [u8]> {
        [
            (self.minusculas, MINUSCULAS),
            (self.maiusculas, MAIUSCULAS),
            (self.digitos, DIGITOS),
            (self.simbolos, SIMBOLOS),
        ]
        .into_iter()
        .filter_map(|(marcado, c)| marcado.then_some(c))
        .collect()
    }
}

/// Gera uma senha com o RNG do sistema.
pub fn gerar(opcoes: &OpcoesGerador) -> Result<Segredo, ErroGerador> {
    gerar_com(opcoes, &mut cripto::aleatorio)
}

/// Bytes aleatórios lidos em lote, que zeram ao sair.
struct Reserva<'a> {
    fonte: &'a mut dyn FnMut(&mut [u8]) -> Result<(), FalhaCripto>,
    bytes: Zeroizing<[u8; 256]>,
    usados: usize,
}

impl<'a> Reserva<'a> {
    fn nova(fonte: &'a mut dyn FnMut(&mut [u8]) -> Result<(), FalhaCripto>) -> Self {
        Self {
            fonte,
            bytes: Zeroizing::new([0; 256]),
            usados: 256,
        }
    }

    fn byte(&mut self) -> Result<u8, ErroGerador> {
        if self.usados == self.bytes.len() {
            (self.fonte)(&mut self.bytes[..]).map_err(|_| ErroGerador::AleatorioIndisponivel)?;
            self.usados = 0;
        }
        let b = self.bytes[self.usados];
        self.bytes[self.usados] = 0;
        self.usados += 1;
        Ok(b)
    }

    /// Índice uniforme em `0..n` (1 ≤ n ≤ 256), por rejeição.
    fn indice(&mut self, n: usize) -> Result<usize, ErroGerador> {
        debug_assert!((1..=256).contains(&n));
        let limite = 256 - 256 % n;
        // Com n ≥ 10 a chance de rejeitar é < 40 %; 1 000 rejeições seguidas só vêm de
        // uma fonte quebrada (sempre o mesmo byte alto), e aí o gerador falha.
        for _ in 0..1_000 {
            let b = usize::from(self.byte()?);
            if b < limite {
                return Ok(b % n);
            }
        }
        Err(ErroGerador::AleatorioIndisponivel)
    }
}

pub(crate) fn gerar_com(
    opcoes: &OpcoesGerador,
    // Fonte de bytes aleatórios; só `cripto::aleatorio` fora dos testes.
    fonte: &mut dyn FnMut(&mut [u8]) -> Result<(), FalhaCripto>,
) -> Result<Segredo, ErroGerador> {
    if !(TAMANHO_MIN..=TAMANHO_MAX).contains(&opcoes.tamanho) {
        return Err(ErroGerador::TamanhoInvalido);
    }
    let conjuntos = opcoes.conjuntos();
    if conjuntos.is_empty() {
        return Err(ErroGerador::NenhumConjunto);
    }
    let alfabeto: Vec<u8> = conjuntos.concat();
    let mut reserva = Reserva::nova(fonte);
    let mut senha = Zeroizing::new(Vec::with_capacity(opcoes.tamanho));
    for _ in 0..MAX_TENTATIVAS {
        senha.zeroize();
        for _ in 0..opcoes.tamanho {
            senha.push(alfabeto[reserva.indice(alfabeto.len())?]);
        }
        if conjuntos
            .iter()
            .all(|c| senha.iter().any(|b| c.contains(b)))
        {
            // Toma posse do buffer sem copiar; o `Zeroizing` fica com um vetor vazio.
            return Ok(Segredo::de_bytes(core::mem::take(&mut *senha)));
        }
    }
    Err(ErroGerador::AleatorioIndisponivel)
}

/// Tamanho do token do "Conectar" (D3): 32 caracteres de `[A-Za-z0-9]` (~190 bits).
pub const TAMANHO_TOKEN: usize = 32;

/// Token de uso único do "Conectar" (D3, Fase 27): aleatório pelo RNG do sistema, só
/// letras e dígitos (cabe no argv do terminal sem aspas e no formato que o app aceita).
pub fn token_conectar() -> Result<Segredo, ErroGerador> {
    gerar(&OpcoesGerador {
        tamanho: TAMANHO_TOKEN,
        minusculas: true,
        maiusculas: true,
        digitos: true,
        simbolos: false,
    })
}

/// Igualdade de dois tokens em tempo constante, pela biblioteca (o `CtEq` do
/// `ctutils`). Tamanhos diferentes dão `false`.
pub fn tokens_iguais(a: &[u8], b: &[u8]) -> bool {
    cripto::iguais_em_tempo_constante(a, b)
}

#[cfg(test)]
mod testes;
