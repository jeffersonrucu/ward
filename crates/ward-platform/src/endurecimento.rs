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
//!
//! Memória (D14 revista pela D62): por último, `mlockall(MCL_CURRENT | MCL_FUTURE |
//! MCL_ONFAULT)` tira o processo inteiro da swap (chave de dados, segredos e a memória
//! do Argon2). Sem `RLIMIT_MEMLOCK` infinito, o `MCL_FUTURE` faria a alocação do Argon2
//! falhar; então, sem o limite infinito ou com erro do `mlockall`, o build de release
//! falha fechado (sai ≠ 0 sem socket) e o `test-mode` só devolve o aviso
//! ([`Memoria::SemTravar`]), sem chamar o `mlockall`, para o `make dev-daemon` subir
//! com o limite comum do usuário. `fork` e `execve` não herdam o travamento.

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
    /// `RLIMIT_MEMLOCK` não é infinito (D62): com limite, o `mlockall` com
    /// `MCL_FUTURE` faria a alocação do Argon2 falhar. Traz o limite atual em bytes.
    LimiteMemlock(u64),
    /// O `mlockall` (ou a leitura do `RLIMIT_MEMLOCK`) falhou (D62).
    TravarMemoria(io::Error),
}

/// Como ficou a memória do processo depois de [`iniciar_endurecido`] (D62).
#[derive(Debug)]
pub enum Memoria {
    /// `mlockall(MCL_CURRENT | MCL_FUTURE | MCL_ONFAULT)` aplicado.
    Travada,
    /// Só no `test-mode`: a memória não foi travada; o erro diz o motivo, para o aviso
    /// no log. No release, o mesmo motivo é erro.
    SemTravar(ErroInicio),
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
            ErroInicio::LimiteMemlock(atual) => write!(
                f,
                "RLIMIT_MEMLOCK de {atual} bytes, e não infinito; o processo trava toda a \
                 memória (mlockall) e sobe pela unit com LimitMEMLOCK=infinity"
            ),
            ErroInicio::TravarMemoria(e) => {
                write!(
                    f,
                    "não deu para travar a memória do processo (mlockall): {e}"
                )
            }
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

/// [`endurecer_processo`], [`conferir_ambiente_proprio`] e o travamento da memória
/// (D62). Erro: o processo deve sair ≠ 0 com a mensagem, sem criar socket (V3.7,
/// V3.25, V3.34). [`Memoria::SemTravar`] só no `test-mode`: quem chama avisa no log.
pub fn iniciar_endurecido() -> Result<Memoria, ErroInicio> {
    endurecer_processo().map_err(ErroInicio::Endurecer)?;
    conferir_ambiente_proprio()?;
    let limite = sys::rlimit_memlock().map(|(atual, _)| atual);
    decidir_memoria(
        limite,
        cfg!(feature = "test-mode"),
        sys::travar_toda_memoria,
    )
}

/// A regra do D62, separada das chamadas ao sistema para o teste cobrir os dois builds:
/// limite infinito → `mlockall`; limite finito, erro na leitura ou no `mlockall` →
/// erro no release e [`Memoria::SemTravar`] no `test-mode`. Com limite finito, o
/// `mlockall` nunca é chamado (o `MCL_FUTURE` quebraria o Argon2).
fn decidir_memoria(
    limite: io::Result<u64>,
    modo_teste: bool,
    travar: impl FnOnce() -> io::Result<()>,
) -> Result<Memoria, ErroInicio> {
    let falha = match limite {
        Ok(libc::RLIM_INFINITY) => match travar() {
            Ok(()) => return Ok(Memoria::Travada),
            Err(e) => ErroInicio::TravarMemoria(e),
        },
        Ok(atual) => ErroInicio::LimiteMemlock(atual),
        Err(e) => ErroInicio::TravarMemoria(e),
    };
    if modo_teste {
        Ok(Memoria::SemTravar(falha))
    } else {
        Err(falha)
    }
}

/// O nome é inseguro pela lista ou pelo prefixo. Compara bytes exatos (`ld_preload`
/// não é do carregador).
fn insegura(nome: &[u8]) -> bool {
    VARIAVEIS_INSEGURAS.iter().any(|v| v.as_bytes() == nome)
        || PREFIXOS_INSEGUROS
            .iter()
            .any(|p| nome.starts_with(p.as_bytes()))
}

/// Nome para a mensagem (V3.25, D48): o prefixo do nome até o primeiro byte fora de
/// `[A-Za-z0-9_]` (exclusive), com no máximo [`MAX_NOME_VARIAVEL`] caracteres. Nada é
/// trocado nem acrescentado (sem `?`, sem `…`): a mensagem só leva bytes que vieram
/// do nome e são desse conjunto, então um `\n`, um `\x1b` ou um nome comprido não
/// forjam linha nem sequência de terminal no log.
pub(crate) fn nome_para_mensagem(nome: &[u8]) -> String {
    nome.iter()
        .take(MAX_NOME_VARIAVEL)
        .take_while(|&&b| b.is_ascii_alphanumeric() || b == b'_')
        .map(|&b| char::from(b))
        .collect()
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
        // V3.25: corta no primeiro byte fora de `[A-Za-z0-9_]`, sem trocar por `?`.
        let n = primeira_variavel_insegura([OsStr::from_bytes(b"LD_\x1b[31m\nx\xff")]).unwrap();
        assert_eq!(n, "LD_");
        let n = primeira_variavel_insegura([OsStr::from_bytes(b"LD_AB\nCD")]).unwrap();
        assert_eq!(n, "LD_AB");
        let n = primeira_variavel_insegura([OsStr::from_bytes(b"LD_X\xffY")]).unwrap();
        assert_eq!(n, "LD_X");
        let n = primeira_variavel_insegura([OsStr::from_bytes(b"LD_\xe2\x80\xa6")]).unwrap();
        assert_eq!(n, "LD_");
        // 200 caracteres: os 64 primeiros, sem `…`.
        let longo = format!("LD_{}", "A".repeat(200));
        let n = primeira_variavel_insegura([longo.as_str()]).unwrap();
        assert_eq!(n, longo[..MAX_NOME_VARIAVEL]);
        // Exatamente 64 passa inteiro; o corte por controle antes do 64º vale.
        let exato = format!("LD_{}", "B".repeat(MAX_NOME_VARIAVEL - 3));
        assert_eq!(nome_para_mensagem(exato.as_bytes()), exato);
        for nome in [
            &b"LD_\x1b]0;titulo\x07"[..],
            b"GLIBC_TUNABLES",
            b"LD_a-b",
            b"LD_\r\nwardd: forjado",
        ] {
            let m = nome_para_mensagem(nome);
            assert!(m.len() <= MAX_NOME_VARIAVEL);
            assert!(
                m.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
                "{m:?}"
            );
            assert!(nome.starts_with(m.as_bytes()), "{m:?} não é prefixo");
        }
    }

    /// D62 (V3.34): release falha fechado sem limite infinito ou com erro do
    /// `mlockall`; o `test-mode` só avisa; com limite finito o `mlockall` nunca é
    /// chamado.
    #[test]
    fn d62_memoria_release_falha_fechado_e_modo_teste_avisa() {
        let oito_mib = 8 * 1024 * 1024;
        for modo_teste in [false, true] {
            let mut chamou = false;
            let r = decidir_memoria(Ok(oito_mib), modo_teste, || {
                chamou = true;
                Ok(())
            });
            assert!(!chamou, "mlockall com limite finito");
            match (modo_teste, r) {
                (false, Err(ErroInicio::LimiteMemlock(n))) => assert_eq!(n, oito_mib),
                (true, Ok(Memoria::SemTravar(ErroInicio::LimiteMemlock(n)))) => {
                    assert_eq!(n, oito_mib)
                }
                (m, r) => panic!("modo_teste={m}: {r:?}"),
            }
            let r = decidir_memoria(Ok(libc::RLIM_INFINITY), modo_teste, || {
                Err(io::Error::from_raw_os_error(libc::ENOMEM))
            });
            match (modo_teste, r) {
                (false, Err(ErroInicio::TravarMemoria(_))) => {}
                (true, Ok(Memoria::SemTravar(ErroInicio::TravarMemoria(_)))) => {}
                (m, r) => panic!("modo_teste={m}: {r:?}"),
            }
            let r = decidir_memoria(Err(io::ErrorKind::Other.into()), modo_teste, || Ok(()));
            assert_eq!(r.is_err(), !modo_teste);
            let r = decidir_memoria(Ok(libc::RLIM_INFINITY), modo_teste, || Ok(()));
            assert!(matches!(r, Ok(Memoria::Travada)));
        }
        let m = ErroInicio::LimiteMemlock(oito_mib).to_string();
        assert!(
            m.contains("LimitMEMLOCK=infinity") && m.contains("8388608"),
            "{m}"
        );
    }

    #[test]
    fn mensagem_nao_tem_valor() {
        let m = ErroInicio::VariavelInsegura("LD_PRELOAD".to_owned()).to_string();
        assert!(m.contains("LD_PRELOAD"));
        assert!(!m.contains('/'));
    }
}
