//! Tudo o que depende do sistema (Linux): socket local, `SO_PEERCRED`, identificação
//! do processo que chama, endurecimento do processo e memória travada (R3, D5, D14,
//! S5, S8), com o socket do systemd do sistema (D46) e o papel pelo grupo setgid
//! (D47), o lançador do app e a partida dele (D57, módulos [`lancador`] e [`app`]) e o
//! canal do token do "Conectar" (Fase 27a, [`conectar`]).
//! Iniciar com a sessão, bloqueio de tela e agente SSH chegam em fases
//! seguintes.
//!
//! Único crate onde `unsafe` pode aparecer: só em `sys` (chamadas à `libc`), em
//! `memoria` (a região mapeada) e em `teste` (só no `test-mode`), sempre com
//! `#[allow(unsafe_code)]` no item, comentário `// SAFETY:` e teste. Os outros módulos
//! usam só as funções seguras de `sys`.
//!
//! Feature `test-mode` (D7): acrescenta a forma de teste do servidor
//! (`ServidorEsperado::teste`), os construtores de teste de [`Instalacao`] e de
//! [`PoliticaKernel`] e o `teste::executar_com_socket`. Sem ela, nada disso existe no
//! binário.

pub mod app;
pub mod conectar;
pub mod descendentes;
pub mod endurecimento;
pub mod filho;
pub mod lancador;
pub mod memoria;
pub mod papel;
pub mod pasta;
pub mod processo;
pub mod relogio;
pub mod servidor;
pub mod socket;
mod sys;
pub mod terminal;
#[cfg(feature = "test-mode")]
pub mod teste;

pub use endurecimento::{
    ErroInicio, Memoria, conferir_ambiente_proprio, endurecer_processo, iniciar_endurecido,
};
pub use lancador::{
    AmbienteApp, ErroLancador, ErroSessao, Lancador, ambiente_do_app, conferir_ambiente_do_app,
};
#[cfg(feature = "test-mode")]
pub use lancador::{BinarioDeTeste, ErroBinarioDeTeste};
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
    conferir_conexao, escutar_de_listen_fds, par_fechou, preparar_diretorio,
};
pub use terminal::{ErroTerminal, ModoCru, ler_senha};

/// uid efetivo deste processo.
pub fn uid_efetivo() -> u32 {
    sys::geteuid()
}

/// Pasta pessoal do uid efetivo pelo `getpwuid_r` (nunca pelo `$HOME`): o diretório de
/// trabalho do `ssh`/`sftp` filho do daemon (Fase 27).
pub fn pasta_pessoal() -> std::io::Result<std::ffi::OsString> {
    sys::home_do_uid(sys::geteuid())
}

/// Nome e pasta pessoal do uid efetivo pelo `getpwuid_r` (nunca por `$USER`/`$HOME`):
/// `USER`, `LOGNAME` e `HOME` do filho do `ward run` (Fase 26a).
pub fn conta_do_usuario() -> std::io::Result<(std::ffi::OsString, std::ffi::OsString)> {
    sys::conta_do_uid(sys::geteuid())
}
