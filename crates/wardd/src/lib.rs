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
//! - [`execucao`]: o `ward run` (Fase 26): entrega, máscara, filho e temporários.
//! - [`playwright`]: o `ward playwright` (Fase 44) pelo Chrome com o pipe de
//!   depuração ([`cdp`]).
//! - [`externo`]: backup externo em pendrive (Fase 46); [`udisks`]: os pendrives pelo
//!   udisks2 no barramento do sistema.
//!
//! Sem `unsafe`: o que depende do sistema está no `ward-plataforma`.

#![forbid(unsafe_code)]

pub mod log;

pub mod agente_ssh;
pub mod aprovacao;
pub mod arquivos;
pub mod cdp;
pub mod config;
pub mod execucao;
pub mod externo;
pub mod lancamento;
pub mod nucleo;
pub mod playwright;
pub mod servidor;
pub mod ssh;
pub mod trava;
pub mod udisks;
pub mod uso;
