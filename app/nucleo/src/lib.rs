//! Lado Rust do app desktop do Ward, sem Tauri (Fase 29).
//!
//! - [`comandos`]: a lista fechada dos comandos que a interface pode chamar. O
//!   `build.rs` do Tauri, as capabilities e a ponte de teste saem dela (S11).
//! - [`daemon`]: cliente do socket do `wardd`. O protocolo chega na Fase 20; até lá,
//!   toda operação responde [`ErroApp::SemProtocolo`].
//! - [`area_transferencia`]: "copiar" resolvido aqui (D4). O valor vai do daemon para a
//!   área de transferência sem passar pela webview, e a limpeza em 30 s só apaga se a
//!   área ainda contém o valor copiado (S12).
//! - [`partida`]: partida do app setgid antes do GTK (D47, D57): ambiente de D57,
//!   conexão com o egid do grupo `ward-app`, largar o grupo, não-dumpable, reexecução.
//! - [`servico`]: o que cada comando faz. Nenhuma resposta leva segredo (S1, S11).

#![forbid(unsafe_code)]

pub mod area_transferencia;
pub mod comandos;
pub mod daemon;
mod erro;
pub mod partida;
pub mod segredo;
pub mod servico;

pub use erro::ErroApp;
