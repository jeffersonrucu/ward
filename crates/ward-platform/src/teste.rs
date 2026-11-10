//! Só no `test-mode`: apoio para os testes de D46 no host e no contêiner.

use std::os::fd::AsFd;
use std::os::unix::net::UnixListener;
use std::os::unix::process::CommandExt;
use std::process::Command;

use crate::sys;

/// Faz o papel do systemd: põe `listener` no fd 3 sem `O_CLOEXEC` e executa `cmd` no
/// mesmo pid, com `LISTEN_PID` = este pid e `LISTEN_FDS=1`. Só volta se o `exec`
/// falhar, e aí o processo sai com 127 sem usar o fd 3 de novo.
pub fn executar_com_socket(listener: &UnixListener, mut cmd: Command) -> ! {
    cmd.env("LISTEN_PID", std::process::id().to_string())
        .env("LISTEN_FDS", "1")
        .env_remove("LISTEN_FDNAMES");
    // SAFETY: o contrato de `por_no_fd_ativacao` pede `exec` ou saída logo depois de
    // trocar o fd 3: o `exec` vem na linha seguinte e, se falhar, o processo sai sem
    // usar o fd 3.
    #[allow(unsafe_code)]
    let r = unsafe { sys::por_no_fd_ativacao(listener.as_fd()) };
    let erro = match r {
        Ok(()) => cmd.exec(),
        Err(e) => e,
    };
    eprintln!("executar_com_socket: {erro}");
    std::process::exit(127)
}
