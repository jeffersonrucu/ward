//! Endurecimento do próprio processo (S8, V3.6, V3.7, V3.25).
//!
//! O `wardd` chama [`iniciar_endurecido`] como primeira coisa do `main`, antes de abrir
//! socket ou cofre. A ordem importa: primeiro `PR_SET_DUMPABLE=0` (daí em diante outro
//! processo do usuário não lê `/proc/<pid>/mem`/`environ` nem faz `ptrace`), depois a
//! conferência do depurador (um que já estivesse preso antes continua visível no
//! `TracerPid`) e do ambiente.
//!
//! Ambiente (D48): o daemon recusa qualquer `LD_*` e a lista insegura da glibc
//! (`UNSECURE_ENVVARS` do `unsecvars.h`, a que o `ld.so` apaga em modo seguro), mais os
//! apelidos `MALLOC_*` dos tunables. O ambiente do daemon instalado vem da unit, então
//! a regra estrita não custa nada.

use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::os::unix::ffi::OsStrExt;

use crate::processo::ler_tracer_pid;
use crate::sys;

/// Variáveis que a glibc 2.39 (Ubuntu 24.04) ignora em programas setuid/setgid
/// (`UNSECURE_ENVVARS`), fora as `LD_*`, que já recusam pelo prefixo.
const VARIAVEIS_INSEGURAS: &[&str] = &[
    "GCONV_PATH",
    "GETCONF_DIR",
    "GLIBC_TUNABLES",
    "HOSTALIASES",
    "LOCALDOMAIN",
    "LOCPATH",
    "MALLOC_TRACE",
    "NIS_PATH",
    "NLSPATH",
    "RESOLV_HOST_CONF",
    "RES_OPTIONS",
    "TMPDIR",
    "TZDIR",
];

/// Prefixos recusados: o carregador (`LD_`) e os apelidos dos tunables do `malloc`
/// (`MALLOC_ARENA_MAX`, `MALLOC_CHECK_`, `MALLOC_PERTURB_`…).
const PREFIXOS_INSEGUROS: &[&str] = &["LD_", "MALLOC_"];

/// Maior nome de variável mostrado na mensagem.
const MAX_NOME_VARIAVEL: usize = 64;

/// Falha ao endurecer o processo. Sem valor de variável nem de memória.
#[derive(Debug)]
pub enum ErroInicio {
    /// `PR_SET_DUMPABLE` ou `RLIMIT_CORE` não ficaram em 0.
    Endurecer(io::Error),
    /// Variável insegura no ambiente (D48). Só o nome, higienizado (letras, dígitos e
    /// `_`, até 64), nunca o valor.
    VariavelInsegura(String),
    /// Há um depurador (`TracerPid` ≠ 0), por exemplo `strace` ou `gdb`.
    Depurador,
    /// Não deu para ler `/proc/self/status`.
    LerStatus(io::Error),
}

impl fmt::Display for ErroInicio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErroInicio::Endurecer(e) => write!(f, "não deu para endurecer o processo: {e}"),
            ErroInicio::VariavelInsegura(nome) => write!(
                f,
                "variável {nome} definida no ambiente; o processo não sobe com LD_*, \
                 GLIBC_TUNABLES, GCONV_PATH e as outras da lista insegura da glibc"
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

/// Confere o próprio ambiente: nenhuma variável insegura (D48) e nenhum depurador.
pub fn conferir_ambiente_proprio() -> Result<(), ErroInicio> {
    if let Some(nome) = primeira_variavel_insegura(std::env::vars_os().map(|(k, _)| k)) {
        return Err(ErroInicio::VariavelInsegura(nome));
    }
    let status = std::fs::read("/proc/self/status").map_err(ErroInicio::LerStatus)?;
    match ler_tracer_pid(&status) {
        Some(0) => Ok(()),
        Some(_) => Err(ErroInicio::Depurador),
        None => Err(ErroInicio::LerStatus(io::ErrorKind::InvalidData.into())),
    }
}

/// [`endurecer_processo`] e depois [`conferir_ambiente_proprio`]. Erro: o processo
/// deve sair ≠ 0 com a mensagem, sem criar socket (V3.7, V3.25).
pub fn iniciar_endurecido() -> Result<(), ErroInicio> {
    endurecer_processo().map_err(ErroInicio::Endurecer)?;
    conferir_ambiente_proprio()
}

/// O nome é inseguro pela lista ou pelo prefixo. Compara bytes exatos (`ld_preload`
/// não é do carregador).
fn insegura(nome: &[u8]) -> bool {
    VARIAVEIS_INSEGURAS.iter().any(|v| v.as_bytes() == nome)
        || PREFIXOS_INSEGUROS
            .iter()
            .any(|p| nome.starts_with(p.as_bytes()))
}

/// Nome para a mensagem: só letras ASCII, dígitos e `_` (o resto vira `?`), até
/// [`MAX_NOME_VARIAVEL`] caracteres.
fn nome_para_mensagem(nome: &[u8]) -> String {
    let mut s: String = nome
        .iter()
        .take(MAX_NOME_VARIAVEL)
        .map(|&b| {
            if b.is_ascii_alphanumeric() || b == b'_' {
                char::from(b)
            } else {
                '?'
            }
        })
        .collect();
    if nome.len() > MAX_NOME_VARIAVEL {
        s.push('…');
    }
    s
}

fn primeira_variavel_insegura<I, S>(nomes: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    nomes
        .into_iter()
        .find(|n| insegura(n.as_ref().as_bytes()))
        .map(|n| nome_para_mensagem(n.as_ref().as_bytes()))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn acha_qualquer_variavel_ld() {
        assert_eq!(primeira_variavel_insegura(["PATH", "HOME"]), None);
        for nome in ["LD_PRELOAD", "LD_AUDIT", "LD_LIBRARY_PATH", "LD_BIND_NOW"] {
            assert_eq!(
                primeira_variavel_insegura(["PATH", nome]).as_deref(),
                Some(nome)
            );
        }
        // Nome inventado também recusa, e o nome vai para a mensagem (V3.25).
        assert_eq!(
            primeira_variavel_insegura(["LD_nome_inventado"]).as_deref(),
            Some("LD_nome_inventado")
        );
        // Só o prefixo exato: `OLD_PWD` e `ld_preload` não são do carregador.
        assert_eq!(primeira_variavel_insegura(["OLD_PWD", "ld_preload"]), None);
    }

    #[test]
    fn ataque_lista_insegura_da_glibc() {
        for nome in VARIAVEIS_INSEGURAS {
            assert_eq!(
                primeira_variavel_insegura([*nome]).as_deref(),
                Some(*nome),
                "{nome}"
            );
        }
        for nome in ["MALLOC_ARENA_MAX", "MALLOC_CHECK_", "MALLOC_PERTURB_"] {
            assert_eq!(primeira_variavel_insegura([nome]).as_deref(), Some(nome));
        }
        // Parecidas, mas fora da lista.
        assert_eq!(
            primeira_variavel_insegura(["GLIBC_TUNABLES_X", "XTMPDIR", "LANG", "TZ"]),
            None
        );
    }

    #[test]
    fn ataque_nome_com_controle_nao_vai_cru_para_a_mensagem() {
        let n = primeira_variavel_insegura([OsStr::from_bytes(b"LD_\x1b[31m\nx\xff")]).unwrap();
        assert_eq!(n, "LD_??31m?x?");
        let longo = format!("LD_{}", "A".repeat(200));
        let n = primeira_variavel_insegura([longo.as_str()]).unwrap();
        assert_eq!(n.chars().count(), MAX_NOME_VARIAVEL + 1);
    }

    #[test]
    fn mensagem_nao_tem_valor() {
        let m = ErroInicio::VariavelInsegura("LD_PRELOAD".to_owned()).to_string();
        assert!(m.contains("LD_PRELOAD"));
        assert!(!m.contains('/'));
    }
}
