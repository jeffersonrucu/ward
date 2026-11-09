//! Endurecimento do próprio processo (S8, V3.6, V3.7).
//!
//! O `wardd` chama [`iniciar_endurecido`] como primeira coisa do `main`, antes de abrir
//! socket ou cofre. A ordem importa: primeiro `PR_SET_DUMPABLE=0` (daí em diante outro
//! processo do usuário não lê `/proc/<pid>/mem`/`environ` nem faz `ptrace`), depois a
//! conferência do depurador (um que já estivesse preso antes continua visível no
//! `TracerPid`) e das variáveis `LD_*`.

use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::os::unix::ffi::OsStrExt;

use crate::processo::{ler_tracer_pid, nome_ld_para_log};
use crate::sys;

/// Falha ao endurecer o processo. Sem valor de variável nem de memória.
#[derive(Debug)]
pub enum ErroInicio {
    /// `PR_SET_DUMPABLE` ou `RLIMIT_CORE` não ficaram em 0.
    Endurecer(io::Error),
    /// Variável `LD_*` no ambiente (só o nome, nunca o valor).
    VariavelLd(&'static str),
    /// Há um depurador (`TracerPid` ≠ 0), por exemplo `strace` ou `gdb`.
    Depurador,
    /// Não deu para ler `/proc/self/status`.
    LerStatus(io::Error),
}

impl fmt::Display for ErroInicio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErroInicio::Endurecer(e) => write!(f, "não deu para endurecer o processo: {e}"),
            ErroInicio::VariavelLd(nome) => write!(
                f,
                "variável {nome} definida no ambiente; o processo não sobe com LD_*"
            ),
            ErroInicio::Depurador => {
                write!(f, "processo sob depurador (TracerPid ≠ 0); não sobe")
            }
            ErroInicio::LerStatus(e) => write!(f, "não deu para ler /proc/self/status: {e}"),
        }
    }
}

impl std::error::Error for ErroInicio {}

/// `PR_SET_DUMPABLE=0` e `RLIMIT_CORE` 0 (atual e máximo), conferidos depois de
/// aplicar. Com o processo não-dumpable o kernel também não gera core, mesmo com o
/// `core_pattern` apontando para um programa (apport, systemd-coredump).
pub fn endurecer_processo() -> io::Result<()> {
    sys::desligar_dumpable()?;
    sys::zerar_rlimit_core()?;
    if sys::rlimit_core()? != (0, 0) {
        return Err(io::Error::other("RLIMIT_CORE não ficou em 0"));
    }
    Ok(())
}

/// Confere o próprio ambiente: nenhuma variável `LD_*` e nenhum depurador.
pub fn conferir_ambiente_proprio() -> Result<(), ErroInicio> {
    if let Some(nome) = primeira_variavel_ld(std::env::vars_os().map(|(k, _)| k)) {
        return Err(ErroInicio::VariavelLd(nome));
    }
    let status = std::fs::read("/proc/self/status").map_err(ErroInicio::LerStatus)?;
    match ler_tracer_pid(&status) {
        Some(0) => Ok(()),
        Some(_) => Err(ErroInicio::Depurador),
        None => Err(ErroInicio::LerStatus(io::ErrorKind::InvalidData.into())),
    }
}

/// [`endurecer_processo`] e depois [`conferir_ambiente_proprio`]. Erro: o processo
/// deve sair ≠ 0 com a mensagem, sem criar socket (V3.7).
pub fn iniciar_endurecido() -> Result<(), ErroInicio> {
    endurecer_processo().map_err(ErroInicio::Endurecer)?;
    conferir_ambiente_proprio()
}

fn primeira_variavel_ld<I, S>(nomes: I) -> Option<&'static str>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    nomes
        .into_iter()
        .find(|n| n.as_ref().as_bytes().starts_with(b"LD_"))
        .map(|n| nome_ld_para_log(n.as_ref().as_bytes()))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn acha_qualquer_variavel_ld() {
        assert_eq!(primeira_variavel_ld(["PATH", "HOME"]), None);
        assert_eq!(
            primeira_variavel_ld(["PATH", "LD_PRELOAD"]),
            Some("LD_PRELOAD")
        );
        assert_eq!(primeira_variavel_ld(["LD_AUDIT"]), Some("LD_AUDIT"));
        assert_eq!(
            primeira_variavel_ld(["LD_LIBRARY_PATH"]),
            Some("LD_LIBRARY_PATH")
        );
        // Outro LD_* também recusa, mas o nome inventado não vai para o log.
        assert_eq!(primeira_variavel_ld(["LD_falso_segredo"]), Some("LD_*"));
        // Só o prefixo exato: `OLD_PWD` e `ld_preload` não são do carregador.
        assert_eq!(primeira_variavel_ld(["OLD_PWD", "ld_preload"]), None);
    }

    #[test]
    fn mensagem_nao_tem_valor() {
        let m = ErroInicio::VariavelLd("LD_PRELOAD").to_string();
        assert!(m.contains("LD_PRELOAD"));
        assert!(!m.contains('/'));
    }
}
