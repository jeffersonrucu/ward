//! Tudo o que depende do sistema (Linux): socket local, `SO_PEERCRED`, identificação
//! do processo que chama, endurecimento do processo e memória travada (R3, D5, D14,
//! S5, S8), com o socket do systemd do sistema (D46) e o papel pelo grupo setgid
//! (D47), o lançador do app e a partida dele (D57, módulos [`lancador`] e [`app`]).
//! Iniciar com a sessão, bloqueio de tela e agente SSH chegam em fases
//! seguintes.
//!
//! Único crate onde `unsafe` pode aparecer: só em `sys` (chamadas à `libc`), em
//! `memoria` (a região mapeada) e em `teste` (só no `modo-teste`), sempre com
//! `#[allow(unsafe_code)]` no item, comentário `// SAFETY:` e teste. Os outros módulos
//! usam só as funções seguras de `sys`.
//!
//! Feature `modo-teste` (D7): acrescenta a forma de teste do servidor
//! (`ServidorEsperado::teste`), os construtores de teste de [`Instalacao`] e de
//! [`PoliticaKernel`] e o `teste::executar_com_socket`. Sem ela, nada disso existe no
//! binário.

pub mod app;
pub mod endurecimento;
pub mod lancador;
pub mod memoria;
pub mod papel;
pub mod processo;
pub mod relogio;
pub mod servidor;
pub mod socket;
mod sys;
pub mod terminal;
#[cfg(feature = "modo-teste")]
pub mod teste;

pub use endurecimento::{
    ErroInicio, conferir_ambiente_proprio, endurecer_processo, iniciar_endurecido,
};
pub use lancador::{
    AmbienteApp, ErroLancador, ErroSessao, Lancador, ambiente_do_app, conferir_ambiente_do_app,
};
pub use memoria::{BufferTravado, travar_regiao};
pub use papel::{ErroInstalacao, GRUPO_APP, GRUPO_EXTENSAO, Instalacao, PoliticaKernel};
pub use processo::{
    BinarioPermitido, ClassePar, CredenciaisPar, ElementoCadeia, ProcessoPar, Recusa,
    cadeia_de_processos,
};
pub use relogio::tempo_desde_boot;
pub use servidor::{RecusaServidor, ServidorEsperado, conectar_servidor, conferir_servidor};
pub use socket::{
    Conexao, DiretorioSocket, ErroAceitar, ErroAtivacao, Escuta, caminho_socket_sistema,
    conferir_conexao, escutar_de_listen_fds, preparar_diretorio,
};
pub use terminal::{ErroTerminal, ler_senha};

/// uid efetivo deste processo.
pub fn uid_efetivo() -> u32 {
    sys::geteuid()
}
