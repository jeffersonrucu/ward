//! Daemon do Ward (`wardd`): único processo que toca em segredo (R1, Fase 22).
//!
//! - [`nucleo`]: estado travado/destravado e a regra de cada operação.
//! - [`servidor`]: o laço do socket (uid do par, papel, quadros, limites).
//! - [`arquivos`]: leitura do cofre com identidade por `fstat` e o arquivo de estado
//!   (D39, D53).
//! - [`config`]: caminhos de release e do daemon de teste (D7).
//! - [`aprovacao`]: motor de aprovação e espera do destrave (Fase 23); [`uso`]: o
//!   uso do agente (nível combinado, liberação de 1 hora, botões com PIN);
//!   [`lancamento`]: o daemon abre o app pelo lançador (D57).
//! - [`trava`]: trava automática por inatividade e pelo logind (Fase 24).
//!
//! Sem `unsafe`: o que depende do sistema está no `ward-plataforma`.

#![forbid(unsafe_code)]

pub mod log;

pub mod aprovacao;
pub mod arquivos;
pub mod config;
pub mod lancamento;
pub mod nucleo;
pub mod servidor;
pub mod trava;
pub mod uso;
