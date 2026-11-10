//! Everything that depends on the system (Linux): local socket, `SO_PEERCRED`,
//! identification of the calling process, process hardening and locked memory (R3, D5,
//! D14, S5, S8), with the system systemd socket (D46) and the role by the setgid group
//! (D47), the app launcher and its startup (D57, modules [`launcher`] and [`app`]) and
//! the channel of the "Conectar" token (Phase 27a, [`conectar`]).
//! Starting with the session, screen lock and SSH agent come in later phases.
//!
//! The only crate where `unsafe` may appear: only in `sys` (calls to `libc`), in
//! `memory` (the mapped region) and in `testing` (only in `test-mode`), always with
//! `#[allow(unsafe_code)]` on the item, a `// SAFETY:` comment and a test. The other
//! modules use only the safe functions of `sys`.
//!
//! Feature `test-mode` (D7): adds the test form of the server
//! (`ServidorEsperado::teste`), the test constructors of [`Instalacao`] and of
//! [`PoliticaKernel`] and `testing::run_with_socket`. Without it, none of that exists in
//! the binary.

pub mod app;
pub mod child;
pub mod clock;
pub mod conectar;
pub mod descendants;
pub mod endurecimento;
pub mod launcher;
pub mod memory;
pub mod papel;
pub mod pasta;
pub mod process;
pub mod servidor;
pub mod socket;
mod sys;
pub mod terminal;
#[cfg(feature = "test-mode")]
pub mod testing;

pub use clock::time_since_boot;
pub use endurecimento::{
    ErroInicio, Memoria, conferir_ambiente_proprio, endurecer_processo, iniciar_endurecido,
};
pub use launcher::{
    AppEnvironment, Launcher, LauncherError, SessionError, app_environment, verify_app_environment,
};
#[cfg(feature = "test-mode")]
pub use launcher::{TestBinary, TestBinaryError};
pub use memory::{LockedBuffer, lock_region};
pub use papel::{ErroInstalacao, GRUPO_APP, GRUPO_EXTENSAO, Instalacao, PoliticaKernel};
pub use process::{
    AllowedBinary, ChainElement, PeerClass, PeerCredentials, PeerProcess, Refusal, process_chain,
};
pub use servidor::{RecusaServidor, ServidorEsperado, conectar_servidor, conferir_servidor};
pub use socket::{
    Conexao, DiretorioSocket, ErroAceitar, ErroAtivacao, Escuta, caminho_socket_sistema,
    conferir_conexao, escutar_de_listen_fds, par_fechou, preparar_diretorio,
};
pub use terminal::{ErroTerminal, ModoCru, ler_senha};

/// Effective uid of this process.
pub fn effective_uid() -> u32 {
    sys::geteuid()
}

/// Home directory of the effective uid through `getpwuid_r` (never through `$HOME`): the
/// working directory of the `ssh`/`sftp` child of the daemon (Phase 27).
pub fn home_dir() -> std::io::Result<std::ffi::OsString> {
    sys::uid_home(sys::geteuid())
}

/// Name and home directory of the effective uid through `getpwuid_r` (never through
/// `$USER`/`$HOME`): `USER`, `LOGNAME` and `HOME` of the `ward run` child (Phase 26a).
pub fn user_account() -> std::io::Result<(std::ffi::OsString, std::ffi::OsString)> {
    sys::uid_account(sys::geteuid())
}
