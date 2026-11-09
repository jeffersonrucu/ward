//! Lado Rust do app desktop do Ward, sem Tauri (Fase 29).
//!
//! - [`comandos`]: a lista fechada dos comandos que a interface pode chamar. O
//!   `build.rs` do Tauri, as capabilities e a ponte de teste saem dela (S11).
//! - [`cliente`]: os pedidos do papel app pelo protocolo da Fase 20 (`estado`,
//!   `lista`, `detalhe`, `copiar`), com a leitura estrita da resposta.
//! - [`daemon`]: transporte até o `wardd` (socket conferido): quadros na conexão
//!   única, thread leitora e fila de avisos (D65 a, Fase 23a).
//! - [`daemon_falso`] (só testes e `modo-teste`): responde pelo protocolo com as
//!   entradas falsas do cofre de teste, para a ponte de teste.
//! - [`credenciais`]: lista e detalhe da tela Credenciais (Fase 32), sem segredo.
//! - [`area_transferencia`]: "copiar" resolvido aqui (D4). O valor vai do daemon para a
//!   área de transferência sem passar pela webview, e a limpeza em 30 s só apaga se a
//!   área ainda contém o valor copiado (S12).
//! - [`gerador`]: gerador de senha avulso (Fase 37). A senha nasce em
//!   `ward_core::gerador` e fica aqui; a interface recebe só tamanho, força e símbolos,
//!   e o valor só no "revelar".
//! - [`partida`]: partida do app setgid antes do GTK (D47, D57): ambiente de D57,
//!   conexão com o egid do grupo `ward-app`, largar o grupo, não-dumpable, reexecução.
//! - [`servico`]: o que cada comando faz. Nenhuma resposta leva segredo (S1, S11).

#![forbid(unsafe_code)]

pub mod area_transferencia;
pub mod cliente;
pub mod comandos;
pub mod credenciais;
pub mod daemon;
#[cfg(any(test, feature = "modo-teste"))]
pub mod daemon_falso;
mod erro;
pub mod gerador;
pub mod partida;
pub mod segredo;
pub mod servico;

pub use erro::ErroApp;
