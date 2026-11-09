//! Núcleo do Ward: formato do arquivo, criptografia, tipos, máscara de saída e
//! protocolo. Não depende de Linux (R2, R3).

#![forbid(unsafe_code)]

// Configuração do cofre: hash do PIN, tempo de trava, backup (D30, Fase 19b).
pub mod configuracao;
// Corpo do cofre: configuração e entradas ↔ bytes (D26, D30; Fases 15a e 19b).
pub mod corpo;
mod cripto;
pub mod entrada;
// Entrega por variáveis de ambiente (D10, Fase 16).
pub mod entrega;
pub mod formato;
// Limites numa fonte só (D32, Fase 19a).
pub mod limites;
// Máscara de saída em fluxo (D11, Fase 17).
pub mod mascara;
// TOTP (RFC 6238) e gerador de senha (Fase 18).
pub mod gerador;
pub mod totp;
// Gravação atômica: usa só a std com as extensões Unix (modo 0600, fsync de diretório).
#[cfg(unix)]
pub mod gravacao;
