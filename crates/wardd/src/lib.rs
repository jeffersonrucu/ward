//! Daemon do Ward (`wardd`): único processo que toca em segredo (R1, Fase 22).
//!
//! - [`nucleo`]: estado travado/destravado e a regra de cada operação.
//! - [`servidor`]: o laço do socket (uid do par, papel, quadros, limites).
//! - [`arquivos`]: leitura do cofre com identidade por `fstat` e o arquivo de estado
//!   (D39, D53).
//! - [`config`]: caminhos de release e do daemon de teste (D7).
//!
//! Sem `unsafe`: o que depende do sistema está no `ward-plataforma`.

#![forbid(unsafe_code)]

pub mod log;

pub mod arquivos;
pub mod config;
pub mod nucleo;
pub mod servidor;
