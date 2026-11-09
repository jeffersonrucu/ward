//! Tudo o que depende do sistema (Linux): socket local, `SO_PEERCRED`, identificação
//! do processo que chama, endurecimento do processo e memória travada (R3, D5, D14,
//! S5, S8). Iniciar com a sessão, bloqueio de tela e agente SSH chegam em fases
//! seguintes.
//!
//! Único crate onde `unsafe` pode aparecer: só em `sys` (chamadas à `libc`) e em
//! `memoria` (a região mapeada), sempre com `#[allow(unsafe_code)]` no item, comentário
//! `// SAFETY:` e teste. Os outros módulos usam só as funções seguras de `sys`.

pub mod endurecimento;
pub mod memoria;
pub mod processo;
pub mod socket;
mod sys;

pub use endurecimento::{
    ErroInicio, conferir_ambiente_proprio, endurecer_processo, iniciar_endurecido,
};
pub use memoria::{BufferTravado, travar_regiao};
pub use processo::{
    BinarioPermitido, ClassePar, CredenciaisPar, ElementoCadeia, Instalacao, ProcessoPar, Recusa,
    cadeia_de_processos, conferir_servidor,
};
pub use socket::{
    Conexao, DiretorioSocket, ErroAceitar, Escuta, conferir_conexao, preparar_diretorio,
};

/// uid efetivo deste processo.
pub fn uid_efetivo() -> u32 {
    sys::geteuid()
}
