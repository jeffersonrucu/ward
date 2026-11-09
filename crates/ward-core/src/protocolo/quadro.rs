//! Quadros do socket: 4 bytes de tamanho (u32 big-endian) e o corpo, um objeto JSON.
//!
//! - O tamanho é conferido contra o limite do papel **antes** de ler o corpo e de alocar
//!   ([`limite_do_quadro`], V3.4). Tamanho 0 é recusado. Um "tamanho negativo" (o
//!   `-1` de 32 bits de um cliente que escreve com sinal) chega como `0xFFFF_FFFF` e é
//!   recusado como grande demais.
//! - O corpo é lido em blocos de [`BLOCO`] que só existem depois que os bytes chegam: um
//!   cliente que declara um quadro grande e fica calado não faz o daemon reservar a
//!   memória toda. No fim os blocos são juntados num buffer de capacidade exata e
//!   zerados (o quadro pode ter segredo, S8).
//! - Os erros não carregam bytes do quadro.

use std::io::{self, Read, Write};

use zeroize::Zeroizing;

use crate::limites::{KIB, MAX_QUADRO_APP, MAX_QUADRO_COMUM, MAX_QUADRO_RESPOSTA};
use crate::protocolo::Papel;

/// Tamanho do bloco de leitura.
pub const BLOCO: usize = 64 * KIB;

/// Bytes do cabeçalho do quadro.
pub const CABECALHO: usize = 4;

/// Maior quadro que o daemon lê de um cliente com este papel (D5, D29). O papel vem
/// da conexão (`SO_PEERCRED`, Fase 21), nunca do quadro.
pub const fn limite_do_quadro(papel: Papel) -> usize {
    match papel {
        Papel::App => MAX_QUADRO_APP,
        Papel::Agente | Papel::Extensao => MAX_QUADRO_COMUM,
    }
}

/// Maior quadro que um cliente lê do daemon.
pub const LIMITE_RESPOSTA: usize = MAX_QUADRO_RESPOSTA;

/// Por que um quadro foi recusado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroQuadro {
    /// O outro lado fechou antes do primeiro byte do cabeçalho (fim normal).
    Fechado,
    /// O outro lado fechou no meio do cabeçalho ou do corpo.
    Truncado,
    /// Tamanho 0.
    Vazio,
    /// Tamanho acima do limite (inclui o "negativo").
    GrandeDemais,
    /// Falha de E/S (tempo esgotado, conexão quebrada…).
    Io(io::ErrorKind),
}

impl core::fmt::Display for ErroQuadro {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Fechado => f.write_str("conexão fechada"),
            Self::Truncado => f.write_str("quadro incompleto"),
            Self::Vazio => f.write_str("quadro vazio"),
            Self::GrandeDemais => f.write_str("quadro grande demais"),
            Self::Io(k) => write!(f, "falha de E/S no quadro ({k})"),
        }
    }
}

impl std::error::Error for ErroQuadro {}

/// Lê o cabeçalho e devolve o tamanho conferido contra `limite`.
pub fn ler_cabecalho(r: &mut impl Read, limite: usize) -> Result<usize, ErroQuadro> {
    let mut cab = [0u8; CABECALHO];
    let mut lidos = 0;
    while lidos < CABECALHO {
        match r.read(&mut cab[lidos..]) {
            Ok(0) if lidos == 0 => return Err(ErroQuadro::Fechado),
            Ok(0) => return Err(ErroQuadro::Truncado),
            Ok(n) => lidos += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(ErroQuadro::Io(e.kind())),
        }
    }
    validar_tamanho(u32::from_be_bytes(cab), limite)
}

/// Confere o tamanho declarado.
pub fn validar_tamanho(declarado: u32, limite: usize) -> Result<usize, ErroQuadro> {
    let n = usize::try_from(declarado).map_err(|_| ErroQuadro::GrandeDemais)?;
    if n == 0 {
        Err(ErroQuadro::Vazio)
    } else if n > limite {
        Err(ErroQuadro::GrandeDemais)
    } else {
        Ok(n)
    }
}

/// Lê um quadro inteiro de até `limite` bytes. O corpo volta num buffer que zera ao
/// sair, com a capacidade exata.
pub fn ler_quadro(r: &mut impl Read, limite: usize) -> Result<Zeroizing<Vec<u8>>, ErroQuadro> {
    let total = ler_cabecalho(r, limite)?;
    // Blocos que zeram ao sair; cada um só é alocado quando há bytes para ele.
    let mut blocos: Vec<Zeroizing<Vec<u8>>> = Vec::new();
    let mut faltam = total;
    while faltam > 0 {
        let n = faltam.min(BLOCO);
        let mut bloco = Zeroizing::new(vec![0u8; n]);
        let mut cheio = 0;
        while cheio < n {
            match r.read(&mut bloco[cheio..]) {
                Ok(0) => return Err(ErroQuadro::Truncado),
                Ok(k) => cheio += k,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(ErroQuadro::Io(e.kind())),
            }
        }
        blocos.push(bloco);
        faltam -= n;
    }
    if blocos.len() == 1 {
        return Ok(blocos.pop().unwrap_or_default());
    }
    let mut corpo = Zeroizing::new(Vec::with_capacity(total));
    for b in &blocos {
        corpo.extend_from_slice(b);
    }
    Ok(corpo)
}

/// Escreve um quadro. Recusa corpo vazio ou acima de `limite` sem escrever nada.
pub fn escrever_quadro(w: &mut impl Write, corpo: &[u8], limite: usize) -> Result<(), ErroQuadro> {
    let n = u32::try_from(corpo.len()).map_err(|_| ErroQuadro::GrandeDemais)?;
    validar_tamanho(n, limite)?;
    let io = |e: io::Error| ErroQuadro::Io(e.kind());
    w.write_all(&n.to_be_bytes()).map_err(io)?;
    w.write_all(corpo).map_err(io)?;
    w.flush().map_err(io)
}
