//! O que o daemon precisa saber para dar papel pelo grupo (D47): os gids de
//! `ward-app` e `ward-nm`, o Chrome instalado ([`Instalacao`]) e a política do kernel
//! que torna o setgid seguro ([`PoliticaKernel`]).

use std::fmt;
use std::path::Path;

use crate::processo::{BinarioPermitido, Recusa};
use crate::sys;

/// Nome do grupo de sistema do app.
pub const GRUPO_APP: &str = "ward-app";
/// Nome do grupo de sistema do host da extensão.
pub const GRUPO_EXTENSAO: &str = "ward-nm";
/// gid que o kernel mostra para um gid sem mapeamento num namespace de usuário.
const GID_SEM_MAPA: u32 = 65534;
/// Maior arquivo de `/etc/group` ou `/etc/subgid` lido.
const MAX_ARQUIVO_GRUPOS: u64 = 4 * 1024 * 1024;

/// Grupos e Chrome da instalação. Construída só por [`Instalacao::nova`] (ou
/// [`Instalacao::do_sistema`]), que recusa gids que o próprio usuário consegue assumir.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instalacao {
    gid_app: u32,
    gid_extensao: u32,
    chrome: BinarioPermitido,
}

/// Instalação recusada. Sem dado do usuário: pode ir para o log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErroInstalacao {
    /// O grupo não existe, aparece mais de uma vez ou a linha está fora do formato.
    GrupoAusente(&'static str),
    /// O grupo tem membros (eles assumiriam o grupo por `newgrp`/`sg`).
    GrupoComMembros(&'static str),
    /// gid 0, 65534 (sem mapa), igual nos dois grupos, ou um gid que este processo já
    /// tem (real, efetivo ou suplementar).
    GidInvalido(&'static str),
    /// O gid cai numa faixa do `/etc/subgid` (um namespace de usuário o mapearia).
    GidEmSubgid(&'static str),
    /// `/etc/group`, `/etc/subgid` ou `/proc/self/status` ilegível ou grande demais.
    Leitura,
}

impl fmt::Display for ErroInstalacao {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErroInstalacao::GrupoAusente(g) => write!(f, "grupo {g} ausente ou repetido"),
            ErroInstalacao::GrupoComMembros(g) => {
                write!(f, "o grupo {g} tem membros; ele tem de ficar vazio")
            }
            ErroInstalacao::GidInvalido(g) => write!(
                f,
                "gid do grupo {g} inválido (0, 65534, repetido ou já deste processo)"
            ),
            ErroInstalacao::GidEmSubgid(g) => {
                write!(f, "gid do grupo {g} dentro de uma faixa do /etc/subgid")
            }
            ErroInstalacao::Leitura => f.write_str("não deu para ler a lista de grupos"),
        }
    }
}

impl std::error::Error for ErroInstalacao {}

impl Instalacao {
    /// Confere os gids contra este processo (real, efetivo e suplementares) e contra o
    /// `/etc/subgid`. O daemon roda como o usuário: se ele já tem o gid, qualquer
    /// processo do usuário também tem e viraria app.
    pub fn nova(
        gid_app: u32,
        gid_extensao: u32,
        chrome: BinarioPermitido,
    ) -> Result<Instalacao, ErroInstalacao> {
        let meus = gids_deste_processo()?;
        let subgid = ler_limitado(Path::new("/etc/subgid"))?;
        for (gid, nome) in [(gid_app, GRUPO_APP), (gid_extensao, GRUPO_EXTENSAO)] {
            if gid == 0 || gid == GID_SEM_MAPA || meus.contains(&gid) {
                return Err(ErroInstalacao::GidInvalido(nome));
            }
            if gid_em_subgid(subgid.as_deref().unwrap_or_default(), gid) {
                return Err(ErroInstalacao::GidEmSubgid(nome));
            }
        }
        if gid_app == gid_extensao {
            return Err(ErroInstalacao::GidInvalido(GRUPO_EXTENSAO));
        }
        Ok(Instalacao {
            gid_app,
            gid_extensao,
            chrome,
        })
    }

    /// Lê `ward-app` e `ward-nm` do `/etc/group` (uma linha cada, sem membros) e passa
    /// por [`nova`](Self::nova).
    pub fn do_sistema(chrome: BinarioPermitido) -> Result<Instalacao, ErroInstalacao> {
        let grupos = ler_limitado(Path::new("/etc/group"))?.ok_or(ErroInstalacao::Leitura)?;
        let gid_app = gid_do_grupo(&grupos, GRUPO_APP)?;
        let gid_extensao = gid_do_grupo(&grupos, GRUPO_EXTENSAO)?;
        Instalacao::nova(gid_app, gid_extensao, chrome)
    }

    /// Só no `modo-teste`: sem as conferências contra este processo, para testes que
    /// usam um grupo suplementar do próprio usuário (com `sg`).
    #[cfg(feature = "modo-teste")]
    pub fn de_teste(gid_app: u32, gid_extensao: u32, chrome: BinarioPermitido) -> Instalacao {
        Instalacao {
            gid_app,
            gid_extensao,
            chrome,
        }
    }

    pub fn gid_app(&self) -> u32 {
        self.gid_app
    }

    pub fn gid_extensao(&self) -> u32 {
        self.gid_extensao
    }

    pub fn chrome(&self) -> &BinarioPermitido {
        &self.chrome
    }
}

/// Lê um arquivo de configuração do sistema com limite. `Ok(None)` se não existe.
fn ler_limitado(caminho: &Path) -> Result<Option<Vec<u8>>, ErroInstalacao> {
    use std::io::Read;
    let arquivo = match std::fs::File::open(caminho) {
        Ok(a) => a,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(ErroInstalacao::Leitura),
    };
    let mut buf = Vec::new();
    arquivo
        .take(MAX_ARQUIVO_GRUPOS + 1)
        .read_to_end(&mut buf)
        .map_err(|_| ErroInstalacao::Leitura)?;
    if buf.len() as u64 > MAX_ARQUIVO_GRUPOS {
        return Err(ErroInstalacao::Leitura);
    }
    Ok(Some(buf))
}

/// gid real, efetivo e suplementares deste processo (`Groups:` do status).
fn gids_deste_processo() -> Result<Vec<u32>, ErroInstalacao> {
    let status = std::fs::read("/proc/self/status").map_err(|_| ErroInstalacao::Leitura)?;
    let mut gids = vec![sys::getgid(), sys::getegid()];
    let linha = status
        .split(|&b| b == b'\n')
        .find_map(|l| l.strip_prefix(b"Groups:"))
        .ok_or(ErroInstalacao::Leitura)?;
    let texto = std::str::from_utf8(linha).map_err(|_| ErroInstalacao::Leitura)?;
    for g in texto.split_whitespace() {
        gids.push(g.parse().map_err(|_| ErroInstalacao::Leitura)?);
    }
    // `Gid:` traz real, efetivo, salvo e de sistema de arquivos.
    if let Some(l) = status
        .split(|&b| b == b'\n')
        .find_map(|l| l.strip_prefix(b"Gid:"))
    {
        let texto = std::str::from_utf8(l).map_err(|_| ErroInstalacao::Leitura)?;
        for g in texto.split_whitespace() {
            gids.push(g.parse().map_err(|_| ErroInstalacao::Leitura)?);
        }
    }
    Ok(gids)
}

/// gid do grupo `nome` no formato do `/etc/group` (`nome:senha:gid:membros`). Exige
/// uma linha só e a lista de membros vazia.
pub(crate) fn gid_do_grupo(grupos: &[u8], nome: &'static str) -> Result<u32, ErroInstalacao> {
    let mut achado = None;
    for linha in grupos.split(|&b| b == b'\n') {
        let mut campos = linha.split(|&b| b == b':');
        if campos.next() != Some(nome.as_bytes()) {
            continue;
        }
        if achado.is_some() {
            return Err(ErroInstalacao::GrupoAusente(nome));
        }
        let _senha = campos.next().ok_or(ErroInstalacao::GrupoAusente(nome))?;
        let gid = campos
            .next()
            .and_then(|g| std::str::from_utf8(g).ok())
            .filter(|g| !g.is_empty() && g.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|g| g.parse::<u32>().ok())
            .ok_or(ErroInstalacao::GrupoAusente(nome))?;
        let membros = campos.next().ok_or(ErroInstalacao::GrupoAusente(nome))?;
        if campos.next().is_some() {
            return Err(ErroInstalacao::GrupoAusente(nome));
        }
        if !membros.is_empty() {
            return Err(ErroInstalacao::GrupoComMembros(nome));
        }
        achado = Some(gid);
    }
    achado.ok_or(ErroInstalacao::GrupoAusente(nome))
}

/// `gid` dentro de alguma faixa `nome:início:quantidade` do `/etc/subgid`, de qualquer
/// usuário. Linha fora do formato conta como "dentro" (falha fechada).
pub(crate) fn gid_em_subgid(subgid: &[u8], gid: u32) -> bool {
    for linha in subgid.split(|&b| b == b'\n') {
        if linha.is_empty() || linha.starts_with(b"#") {
            continue;
        }
        let campos: Vec<&[u8]> = linha.split(|&b| b == b':').collect();
        let numero = |c: &[u8]| std::str::from_utf8(c).ok()?.parse::<u64>().ok();
        let (Some(inicio), Some(qtd)) = (
            campos.get(1).and_then(|c| numero(c)),
            campos.get(2).and_then(|c| numero(c)),
        ) else {
            return true;
        };
        if campos.len() != 3 {
            return true;
        }
        let g = u64::from(gid);
        if g >= inicio && g < inicio.saturating_add(qtd) {
            return true;
        }
    }
    false
}

/// `kernel.yama.ptrace_scope` e `fs.suid_dumpable`. Com `ptrace_scope` 0, qualquer
/// processo do usuário se prende a outro; com `suid_dumpable` 1, o processo setgid
/// fica dumpable. Em qualquer dos dois, o daemon não dá papel de app nem de extensão.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoliticaKernel {
    ptrace_scope: Option<u32>,
    suid_dumpable: Option<u32>,
}

impl PoliticaKernel {
    /// Lê os dois do `/proc/sys`. Ilegível (Yama ausente, por exemplo) fica `None`, que
    /// recusa.
    pub fn ler() -> PoliticaKernel {
        let ler = |p: &str| {
            std::fs::read_to_string(p)
                .ok()
                .and_then(|t| t.trim().parse::<u32>().ok())
        };
        PoliticaKernel {
            ptrace_scope: ler("/proc/sys/kernel/yama/ptrace_scope"),
            suid_dumpable: ler("/proc/sys/fs/suid_dumpable"),
        }
    }

    /// Valores dados, para simular nos testes um kernel com outra política.
    #[cfg(any(test, feature = "modo-teste"))]
    pub fn simulada(ptrace_scope: Option<u32>, suid_dumpable: Option<u32>) -> PoliticaKernel {
        PoliticaKernel {
            ptrace_scope,
            suid_dumpable,
        }
    }

    /// `ptrace_scope` ≥ 1 e `suid_dumpable` 0 ou 2.
    pub fn conferir(&self) -> Result<(), Recusa> {
        let ptrace_ok = matches!(self.ptrace_scope, Some(n) if n >= 1);
        let dumpable_ok = matches!(self.suid_dumpable, Some(0 | 2));
        if ptrace_ok && dumpable_ok {
            Ok(())
        } else {
            Err(Recusa::PoliticaKernel)
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn grupo_do_etc_group() {
        let g = b"root:x:0:\nward-app:x:991:\nward-nm:x:992:\ndocker:x:984:eu\n";
        assert_eq!(gid_do_grupo(g, GRUPO_APP), Ok(991));
        assert_eq!(gid_do_grupo(g, GRUPO_EXTENSAO), Ok(992));
    }

    #[test]
    fn ataque_grupo_com_membro_repetido_ou_estragado() {
        assert_eq!(
            gid_do_grupo(b"ward-app:x:991:eu\n", GRUPO_APP),
            Err(ErroInstalacao::GrupoComMembros(GRUPO_APP))
        );
        assert_eq!(
            gid_do_grupo(b"ward-app:x:991:\nward-app:x:5:\n", GRUPO_APP),
            Err(ErroInstalacao::GrupoAusente(GRUPO_APP))
        );
        for ruim in [
            &b"ward-app:x::\n"[..],
            b"ward-app:x:+1:\n",
            b"ward-app:x:991\n",
            b"ward-app:x:991::\n",
            b"ward-appx:x:991:\n",
            b"",
        ] {
            assert_eq!(
                gid_do_grupo(ruim, GRUPO_APP),
                Err(ErroInstalacao::GrupoAusente(GRUPO_APP)),
                "{:?}",
                String::from_utf8_lossy(ruim)
            );
        }
    }

    #[test]
    fn ataque_gid_dentro_de_faixa_do_subgid() {
        let s = b"eu:100000:65536\noutro:165536:65536\n";
        assert!(!gid_em_subgid(s, 991));
        assert!(gid_em_subgid(s, 100000));
        assert!(gid_em_subgid(s, 165535));
        assert!(gid_em_subgid(s, 231071));
        assert!(!gid_em_subgid(s, 231072));
        // Linha estragada falha fechada.
        assert!(gid_em_subgid(b"eu:abc:1\n", 991));
        assert!(!gid_em_subgid(b"", 991));
    }

    #[test]
    fn ataque_gid_que_este_processo_ja_tem() {
        let chrome = BinarioPermitido::novo("/opt/google/chrome/chrome").unwrap();
        let meu = sys::getegid();
        assert_eq!(
            Instalacao::nova(meu, 991_999, chrome.clone()),
            Err(ErroInstalacao::GidInvalido(GRUPO_APP))
        );
        for ruim in [0, GID_SEM_MAPA] {
            assert_eq!(
                Instalacao::nova(991_998, ruim, chrome.clone()),
                Err(ErroInstalacao::GidInvalido(GRUPO_EXTENSAO))
            );
        }
        assert_eq!(
            Instalacao::nova(991_998, 991_998, chrome.clone()),
            Err(ErroInstalacao::GidInvalido(GRUPO_EXTENSAO))
        );
        // Cada grupo suplementar deste processo também recusa.
        for g in gids_deste_processo().unwrap() {
            assert!(Instalacao::nova(g, 991_999, chrome.clone()).is_err(), "{g}");
        }
    }

    #[test]
    fn politica_do_kernel() {
        assert_eq!(
            PoliticaKernel::simulada(Some(1), Some(0)).conferir(),
            Ok(())
        );
        assert_eq!(
            PoliticaKernel::simulada(Some(2), Some(2)).conferir(),
            Ok(())
        );
        assert_eq!(
            PoliticaKernel::simulada(Some(3), Some(0)).conferir(),
            Ok(())
        );
        for (p, d) in [
            (Some(0), Some(0)),
            (Some(1), Some(1)),
            (None, Some(0)),
            (Some(1), None),
            (Some(1), Some(3)),
        ] {
            assert_eq!(
                PoliticaKernel::simulada(p, d).conferir(),
                Err(Recusa::PoliticaKernel),
                "{p:?} {d:?}"
            );
        }
    }
}
