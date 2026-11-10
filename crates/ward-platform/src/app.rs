//! Chamadas do lado do app setgid (D47, D57; Fase 29a). A ordem da partida fica no
//! `app/core` (`partida`); aqui ficam só as operações do sistema, seguras para
//! chamar:
//! - [`largar_grupo`]: `setresgid` para o gid real, conferido (o GTK 3 se recusa a
//!   iniciar com setgid, e o grupo só serve para o `connect`);
//! - [`Pai`]: o pai do começo, com o pidfd aberto, para saber depois se ele continua
//!   sendo o pai (a reexecução de D47 só vale com o `PPid` igual ao daemon);
//! - [`tem_cloexec`] e [`esperar_fechamento`] do socket;
//! - [`reexecutar`]: `execve` de `/proc/self/exe` com o ambiente conferido e todo fd ≥ 3
//!   fechado, para o kernel aplicar o setgid de novo.

use std::ffi::OsString;
use std::fmt;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::Command;

use crate::launcher::AppEnvironment;
use crate::sys;

/// gid real, efetivo e salvo.
pub fn gids() -> io::Result<(u32, u32, u32)> {
    sys::getresgid()
}

/// Larga o grupo do binário setgid: os três gids no real. Sem setgid, não muda nada.
pub fn largar_grupo() -> io::Result<()> {
    sys::largar_grupo_efetivo()
}

/// `true` quando o processo está dumpable (`PR_GET_DUMPABLE` ≠ 0).
pub fn dumpable() -> io::Result<bool> {
    sys::dumpable()
}

/// O valor de `PR_GET_DUMPABLE`: 0, 1 ou 2 (2 depois do `exec` setgid com
/// `fs.suid_dumpable=2`).
pub fn valor_dumpable() -> io::Result<i32> {
    sys::valor_dumpable()
}

/// O fd tem `FD_CLOEXEC` (o socket do app não pode ir para os processos do WebKitGTK).
pub fn tem_cloexec(fd: &impl AsFd) -> io::Result<bool> {
    sys::tem_cloexec(fd.as_fd())
}

/// Bloqueia até o outro lado do socket fechar, sem ler nada.
pub fn esperar_fechamento(fd: &impl AsFd) -> io::Result<()> {
    sys::esperar_fechamento(fd.as_fd())
}

/// Por que o pai não parece ser o daemon. Sem conteúdo do `/proc`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaiDiferente {
    /// `PPid` 0 ou 1 (órfão, ou o pai saiu).
    Orfao,
    /// O processo pai não é `wardd` do mesmo uid.
    NaoEoDaemon,
    /// O pai é dumpable: o `wardd` nunca é (S8).
    PaiDumpable,
    /// O pai saiu, ou o pid já é de outro.
    Saiu,
    /// Não deu para ler o `/proc` do pai.
    Leitura,
}

impl fmt::Display for PaiDiferente {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PaiDiferente::Orfao => "o app não tem pai (o daemon saiu)",
            PaiDiferente::NaoEoDaemon => "o app não foi aberto pelo daemon do Ward",
            PaiDiferente::PaiDumpable => "o processo pai não é o daemon endurecido do Ward",
            PaiDiferente::Saiu => "o processo pai saiu",
            PaiDiferente::Leitura => "não deu para conferir o processo pai",
        })
    }
}

impl std::error::Error for PaiDiferente {}

/// Nome do daemon no `status` (`comm`).
const NOME_DO_DAEMON: &[u8] = b"Name:\twardd";

/// O pai deste processo no momento de [`Pai::atual`], com o pidfd dele.
#[derive(Debug)]
pub struct Pai {
    pid: u32,
    pidfd: OwnedFd,
}

impl Pai {
    /// O pai agora (`getppid`), com o pidfd aberto e o `PPid` relido depois.
    pub fn atual() -> Result<Pai, PaiDiferente> {
        let pid = sys::getppid();
        if pid <= 1 {
            return Err(PaiDiferente::Orfao);
        }
        let p = libc::pid_t::try_from(pid).map_err(|_| PaiDiferente::Leitura)?;
        let pidfd = sys::pidfd_open(p).map_err(|_| PaiDiferente::Saiu)?;
        if sys::getppid() != pid {
            return Err(PaiDiferente::Saiu);
        }
        Ok(Pai { pid, pidfd })
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// O mesmo processo continua vivo e continua sendo o pai deste.
    pub fn continua_pai(&self) -> bool {
        matches!(sys::pidfd_vivo(self.pidfd.as_fd()), Ok(true)) && sys::getppid() == self.pid
    }

    /// Lado do app, só para não seguir quando o pai claramente não é o daemon: o pai
    /// tem `Name: wardd`, os quatro uids iguais ao deste processo e é não-dumpable (o
    /// `environ` dele dá `EACCES`; chamar depois de [`largar_grupo`], senão o grupo
    /// diferente já daria `EACCES`). Não é prova: quem decide o papel é o daemon, pelo
    /// pidfd do `SO_PEERPIDFD` (D47, D57).
    pub fn conferir_parece_o_daemon(&self) -> Result<(), PaiDiferente> {
        let status =
            std::fs::read(format!("/proc/{}/status", self.pid)).map_err(|_| PaiDiferente::Saiu)?;
        let uid = sys::geteuid();
        let nome_ok = status.split(|&b| b == b'\n').any(|l| l == NOME_DO_DAEMON);
        let uids_ok = status
            .split(|&b| b == b'\n')
            .find_map(|l| l.strip_prefix(b"Uid:"))
            .and_then(|l| std::str::from_utf8(l).ok())
            .is_some_and(|l| {
                let v: Vec<&str> = l.split_whitespace().collect();
                v.len() == 4 && v.iter().all(|u| u.parse::<u32>().ok() == Some(uid))
            });
        if !nome_ok || !uids_ok {
            return Err(PaiDiferente::NaoEoDaemon);
        }
        match std::fs::File::open(format!("/proc/{}/environ", self.pid)) {
            Err(e) if e.raw_os_error() == Some(libc::EACCES) => {}
            Ok(_) => return Err(PaiDiferente::PaiDumpable),
            Err(_) => return Err(PaiDiferente::Leitura),
        }
        if !self.continua_pai() {
            return Err(PaiDiferente::Saiu);
        }
        Ok(())
    }
}

/// `execve("/proc/self/exe")` com `argumentos` (sem o `argv[0]`), só `ambiente` e
/// todo fd ≥ 3 fechado. O kernel aplica de novo o setgid do arquivo. Só volta em erro.
pub fn reexecutar(argumentos: &[OsString], ambiente: &AppEnvironment) -> io::Error {
    let mut cmd = Command::new("/proc/self/exe");
    cmd.args(argumentos)
        .env_clear()
        .envs(ambiente.variables())
        .current_dir("/");
    sys::fechar_fds_no_exec(&mut cmd);
    cmd.exec()
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::os::unix::net::UnixStream;

    #[test]
    fn sem_setgid_largar_grupo_nao_muda_nada() {
        let antes = gids().unwrap();
        largar_grupo().unwrap();
        let (r, e, s) = gids().unwrap();
        assert_eq!((r, e, s), (antes.0, antes.0, antes.0));
    }

    #[test]
    fn socket_do_std_tem_cloexec_e_o_fechamento_e_visto() {
        let (a, b) = UnixStream::pair().unwrap();
        assert!(tem_cloexec(&a).unwrap());
        let t = std::thread::spawn(move || esperar_fechamento(&a));
        std::thread::sleep(std::time::Duration::from_millis(50));
        drop(b);
        assert!(t.join().unwrap().is_ok());
    }

    #[test]
    fn ataque_pai_que_nao_e_o_daemon_e_recusado() {
        // O pai do teste é o cargo (ou o shell): nem `wardd`, nem endurecido.
        let pai = Pai::atual().unwrap();
        assert!(pai.continua_pai());
        assert_eq!(
            pai.conferir_parece_o_daemon(),
            Err(PaiDiferente::NaoEoDaemon)
        );
        assert!(!PaiDiferente::NaoEoDaemon.to_string().contains('/'));
    }
}
