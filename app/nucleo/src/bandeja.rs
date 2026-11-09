//! Ícone e menu da bandeja (Fase 39, V5.23), sem Tauri: o que cada estado do cofre
//! mostra, o que cada item faz e onde o ícone pode ser gravado. O `app/src-tauri` só
//! monta o menu nativo a partir daqui.
//!
//! Segurança:
//! - O menu é exportado pelo barramento da sessão (AppIndicator/`dbusmenu`): qualquer
//!   processo do usuário lê os rótulos e pode "clicar" num item. Por isso o menu só tem
//!   o estado do cofre (que o papel agente já obtém com `estado`) e quatro ações que não
//!   revelam, não copiam e não aprovam nada: abrir a janela, travar, abrir a tela de
//!   destravar (a senha é digitada na janela do Ward) e sair. Nenhum item leva nome de
//!   entrada nem valor (S1, S10, S13). Um id desconhecido ou fora do estado não faz nada.
//! - A consulta periódica usa só `estado`, que o `wardd` não conta como uso
//!   (`wardd::trava::conta_como_uso`): a bandeja não segura o cofre aberto. O tempo
//!   de trava (`configuracao`, que **conta** como uso) só é lido depois de uma ação do
//!   usuário: a passagem de travado (ou sem cofre) para destravado, que exige a senha
//!   mestra, e a tela Configurações ([`ler_configuracao_na_passagem`]). Nunca na partida
//!   nem na consulta periódica: um agente que derruba o app e pede `abrir_app` de novo
//!   não renova o prazo de inatividade.
//! - O ícone (fixo, embutido no binário) é gravado pelo AppIndicator numa pasta própria
//!   em `XDG_RUNTIME_DIR`, 0700 e do uid, conferida sem seguir link ([`pasta_dos_icones`]).
//! - A biblioteca do AppIndicator é carregada na hora pelo `tray-icon`, que entra em pânico
//!   se ela faltar (e o release aborta no pânico): antes, [`biblioteca_disponivel`]
//!   procura o arquivo nas pastas do sistema; sem ele, o app abre sem bandeja.

use std::fs::DirBuilder;
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::ErroApp;
use crate::daemon::EstadoCofre;

/// Ids dos itens do menu. Os de texto só informam (desabilitados).
pub const ID_TITULO: &str = "bandeja-titulo";
pub const ID_SUBTITULO: &str = "bandeja-subtitulo";
pub const ID_ABRIR: &str = "bandeja-abrir";
pub const ID_ACAO: &str = "bandeja-acao";
pub const ID_SAIR: &str = "bandeja-sair";

/// Textos fixos dos itens que não mudam com o estado (frames `17269`/`17300`).
pub const TEXTO_ABRIR: &str = "Abrir o Ward";
pub const TEXTO_SAIR: &str = "Sair";

/// Nome do arquivo que o `tray-icon` carrega (o AppIndicator do Ayatana ou o antigo),
/// na ordem dele (`libappindicator-sys`).
pub const BIBLIOTECAS: &[&str] = &[
    "libayatana-appindicator3.so.1",
    "libappindicator3.so.1",
    "libayatana-appindicator3.so",
    "libappindicator3.so",
];

/// Pastas do sistema onde o carregador acha as bibliotecas (sem `LD_LIBRARY_PATH`, que a
/// partida do app não deixa passar, D57).
pub const PASTAS_DO_SISTEMA: &[&str] = &[
    "/usr/lib/x86_64-linux-gnu",
    "/lib/x86_64-linux-gnu",
    "/usr/lib/aarch64-linux-gnu",
    "/lib/aarch64-linux-gnu",
    "/usr/lib64",
    "/lib64",
    "/usr/lib",
    "/lib",
];

/// Nome da pasta dos ícones dentro de `XDG_RUNTIME_DIR`.
pub const PASTA_DOS_ICONES: &str = "ward-bandeja";

/// Estado mostrado na bandeja.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EstadoBandeja {
    Destravado,
    Travado,
    SemCofre,
    /// Sem resposta do daemon: falha fechada (ícone de travado, sem "Travar agora").
    SemDaemon,
}

impl EstadoBandeja {
    pub fn de(estado: Result<EstadoCofre, ErroApp>) -> EstadoBandeja {
        match estado {
            Ok(EstadoCofre::Destravado) => EstadoBandeja::Destravado,
            Ok(EstadoCofre::Travado) => EstadoBandeja::Travado,
            Ok(EstadoCofre::SemCofre) => EstadoBandeja::SemCofre,
            Err(_) => EstadoBandeja::SemDaemon,
        }
    }
}

/// Qual dos dois ícones (`Ícone da bandeja`, `17138`/`17142`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icone {
    Destravado,
    Travado,
}

/// O que o menu mostra num estado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Aparencia {
    pub titulo: &'static str,
    pub subtitulo: &'static str,
    pub acao: &'static str,
    pub acao_habilitada: bool,
    pub icone: Icone,
}

/// Subtítulo do destravado com o tempo de trava lido da configuração ("Trava sozinho
/// após 4 h sem uso", frame `17269`); sem a leitura, o texto genérico de [`aparencia`].
pub fn subtitulo_destravado(minutos: Option<u32>) -> String {
    let Some(m) = minutos.filter(|m| *m > 0) else {
        return aparencia(EstadoBandeja::Destravado).subtitulo.to_owned();
    };
    let tempo = match (m / 60, m % 60) {
        (0, min) => format!("{min} min"),
        (h, 0) => format!("{h} h"),
        (h, min) => format!("{h} h {min} min"),
    };
    format!("Trava sozinho após {tempo} sem uso")
}

/// Ler o tempo de trava (`configuracao`, que o daemon conta como uso) nesta passagem de
/// estado? Só quando o cofre acabou de ser destravado (ou criado) pelo usuário: de
/// travado ou sem cofre para destravado. Nunca de "sem daemon" (partida do app,
/// reconexão) nem de destravado para destravado (consulta periódica).
pub fn ler_configuracao_na_passagem(antes: EstadoBandeja, depois: EstadoBandeja) -> bool {
    matches!(
        (antes, depois),
        (
            EstadoBandeja::Travado | EstadoBandeja::SemCofre,
            EstadoBandeja::Destravado
        )
    )
}

/// Textos do menu (frames `Bandeja / Destravado` `17269` e `/ Travado` `17300`). O
/// subtítulo do destravado aqui é o genérico; com o tempo lido, [`subtitulo_destravado`].
pub fn aparencia(estado: EstadoBandeja) -> Aparencia {
    match estado {
        EstadoBandeja::Destravado => Aparencia {
            titulo: "Cofre destravado",
            subtitulo: "Trava sozinho após o tempo sem uso",
            acao: "Travar agora",
            acao_habilitada: true,
            icone: Icone::Destravado,
        },
        EstadoBandeja::Travado => Aparencia {
            titulo: "Cofre travado",
            subtitulo: "Destrave com a senha mestra",
            acao: "Destravar…",
            acao_habilitada: true,
            icone: Icone::Travado,
        },
        EstadoBandeja::SemCofre => Aparencia {
            titulo: "Nenhum cofre criado",
            subtitulo: "Crie o cofre na janela do Ward",
            acao: "Travar agora",
            acao_habilitada: false,
            icone: Icone::Travado,
        },
        EstadoBandeja::SemDaemon => Aparencia {
            titulo: "Daemon indisponível",
            subtitulo: "Abra o Ward para tentar de novo",
            acao: "Travar agora",
            acao_habilitada: false,
            icone: Icone::Travado,
        },
    }
}

/// O que um clique no menu faz.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Acao {
    /// Mostra (ou recria) a janela principal.
    Abrir,
    /// Trava o cofre agora (limpa a área de transferência e o que a tela guardava).
    Travar,
    /// Mostra a janela principal, que está na tela `Destravar`.
    Destravar,
    /// Fecha o app.
    Sair,
}

/// O id que o menu nativo devolveu, no estado que o menu mostrava. Só os ids do menu
/// valem; "Travar"/"Destravar" só no estado em que aparecem habilitados.
pub fn acao(id: &str, estado: EstadoBandeja) -> Option<Acao> {
    match id {
        ID_ABRIR => Some(Acao::Abrir),
        ID_SAIR => Some(Acao::Sair),
        ID_ACAO => match estado {
            EstadoBandeja::Destravado => Some(Acao::Travar),
            EstadoBandeja::Travado => Some(Acao::Destravar),
            EstadoBandeja::SemCofre | EstadoBandeja::SemDaemon => None,
        },
        _ => None,
    }
}

/// A biblioteca do AppIndicator existe numa das `pastas`? Só arquivo comum (ou link para
/// um), sem abrir nada.
pub fn biblioteca_disponivel(pastas: &[&Path]) -> bool {
    pastas.iter().any(|p| {
        BIBLIOTECAS
            .iter()
            .any(|b| std::fs::metadata(p.join(b)).is_ok_and(|m| m.is_file()))
    })
}

/// A pasta dos ícones da bandeja em `runtime` (o `XDG_RUNTIME_DIR`): `runtime` tem de ser
/// caminho absoluto, pasta (sem link) do `uid` e sem acesso de grupo e outros; a pasta
/// dos ícones é criada 0700 ou, se já existe, conferida do mesmo jeito (sem link, do
/// `uid`, 0700). Fora disso, erro: o app abre sem bandeja.
pub fn pasta_dos_icones(runtime: &Path, uid: u32) -> io::Result<PathBuf> {
    if !runtime.is_absolute() {
        return Err(io::Error::other("XDG_RUNTIME_DIR relativo"));
    }
    conferir_pasta(runtime, uid)?;
    let pasta = runtime.join(PASTA_DOS_ICONES);
    match DirBuilder::new().mode(0o700).create(&pasta) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    conferir_pasta(&pasta, uid)?;
    Ok(pasta)
}

/// [`pasta_dos_icones`] no `XDG_RUNTIME_DIR` do processo, com o uid efetivo.
pub fn pasta_dos_icones_da_sessao() -> io::Result<PathBuf> {
    let rt = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("sem XDG_RUNTIME_DIR"))?;
    pasta_dos_icones(&rt, ward_plataforma::uid_efetivo())
}

/// [`biblioteca_disponivel`] nas [`PASTAS_DO_SISTEMA`].
pub fn biblioteca_do_sistema() -> bool {
    let pastas: Vec<&Path> = PASTAS_DO_SISTEMA.iter().map(Path::new).collect();
    biblioteca_disponivel(&pastas)
}

fn conferir_pasta(p: &Path, uid: u32) -> io::Result<()> {
    let m = std::fs::symlink_metadata(p)?;
    if !m.file_type().is_dir() {
        return Err(io::Error::other("não é pasta"));
    }
    if m.uid() != uid {
        return Err(io::Error::other("pasta de outro usuário"));
    }
    if m.permissions().mode() & 0o077 != 0 {
        return Err(io::Error::other("pasta aberta a grupo ou outros"));
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    const TODOS: [EstadoBandeja; 4] = [
        EstadoBandeja::Destravado,
        EstadoBandeja::Travado,
        EstadoBandeja::SemCofre,
        EstadoBandeja::SemDaemon,
    ];

    fn pasta_temporaria(nome: &str) -> PathBuf {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("bandeja-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&p)
            .unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700)).unwrap();
        p
    }

    #[test]
    fn textos_do_design() {
        let d = aparencia(EstadoBandeja::Destravado);
        assert_eq!(
            (d.titulo, d.acao, d.icone),
            ("Cofre destravado", "Travar agora", Icone::Destravado)
        );
        let t = aparencia(EstadoBandeja::Travado);
        assert_eq!(
            (t.titulo, t.subtitulo, t.acao, t.icone),
            (
                "Cofre travado",
                "Destrave com a senha mestra",
                "Destravar…",
                Icone::Travado
            )
        );
        assert_eq!(TEXTO_ABRIR, "Abrir o Ward");
        assert_eq!(TEXTO_SAIR, "Sair");
    }

    #[test]
    fn estado_vem_da_resposta_e_erro_fica_travado() {
        assert_eq!(
            EstadoBandeja::de(Ok(EstadoCofre::Destravado)),
            EstadoBandeja::Destravado
        );
        assert_eq!(
            EstadoBandeja::de(Ok(EstadoCofre::Travado)),
            EstadoBandeja::Travado
        );
        assert_eq!(
            EstadoBandeja::de(Ok(EstadoCofre::SemCofre)),
            EstadoBandeja::SemCofre
        );
        let e = EstadoBandeja::de(Err(ErroApp::DaemonIndisponivel));
        assert_eq!(e, EstadoBandeja::SemDaemon);
        // Falha fechada: sem daemon o ícone é o de travado e não há o que travar.
        assert_eq!(aparencia(e).icone, Icone::Travado);
        assert!(!aparencia(e).acao_habilitada);
    }

    /// Ataque (S10, S13): um processo do usuário manda pelo `dbusmenu` um id qualquer.
    /// Só os quatro ids do menu fazem algo, e nenhuma ação aprova, revela ou copia.
    #[test]
    fn ataque_id_forjado_no_menu_nao_faz_nada() {
        for estado in TODOS {
            for id in [
                "",
                "inventado",
                ID_TITULO,
                ID_SUBTITULO,
                "bandeja-abrir ",
                "BANDEJA-ACAO",
                "responder_aprovacao",
                "revelar_campo",
                "copiar_campo",
                "destravar",
                "bandeja-acao\0",
            ] {
                assert_eq!(acao(id, estado), None, "{id:?} em {estado:?}");
            }
            assert_eq!(acao(ID_ABRIR, estado), Some(Acao::Abrir));
            assert_eq!(acao(ID_SAIR, estado), Some(Acao::Sair));
        }
        assert_eq!(acao(ID_ACAO, EstadoBandeja::Destravado), Some(Acao::Travar));
        assert_eq!(acao(ID_ACAO, EstadoBandeja::Travado), Some(Acao::Destravar));
        // O item desabilitado não age mesmo que o clique chegue pelo barramento.
        assert_eq!(acao(ID_ACAO, EstadoBandeja::SemCofre), None);
        assert_eq!(acao(ID_ACAO, EstadoBandeja::SemDaemon), None);
    }

    /// S13: o menu (legível por qualquer processo da sessão) não leva nome de entrada nem
    /// valor: só textos fixos, e nenhum da tabela de segredos falsos.
    #[test]
    fn ataque_menu_sem_dado_do_cofre() {
        for estado in TODOS {
            let a = aparencia(estado);
            for texto in [a.titulo, a.subtitulo, a.acao, TEXTO_ABRIR, TEXTO_SAIR] {
                for s in crate::daemon_falso::SEGREDOS {
                    assert!(!texto.contains(s), "{texto}");
                }
                assert!(!texto.contains("acme-"), "{texto}");
            }
        }
    }

    #[test]
    fn subtitulo_com_o_tempo_de_trava() {
        assert_eq!(
            subtitulo_destravado(Some(240)),
            "Trava sozinho após 4 h sem uso"
        );
        assert_eq!(
            subtitulo_destravado(Some(30)),
            "Trava sozinho após 30 min sem uso"
        );
        assert_eq!(
            subtitulo_destravado(Some(90)),
            "Trava sozinho após 1 h 30 min sem uso"
        );
        assert_eq!(
            subtitulo_destravado(None),
            aparencia(EstadoBandeja::Destravado).subtitulo
        );
        assert_eq!(
            subtitulo_destravado(Some(0)),
            aparencia(EstadoBandeja::Destravado).subtitulo
        );
    }

    /// Ataque (trava por inatividade): `configuracao` conta como uso no `wardd`. A
    /// bandeja só a lê quando o usuário acabou de destravar (ou criar) o cofre; a
    /// partida do app (de "sem daemon"), a consulta periódica e a trava não a leem. Um
    /// agente que derruba o app e pede `abrir_app` em laço não segura o cofre aberto.
    #[test]
    fn ataque_configuracao_so_depois_de_destravar() {
        use EstadoBandeja::*;
        assert!(ler_configuracao_na_passagem(Travado, Destravado));
        assert!(ler_configuracao_na_passagem(SemCofre, Destravado));
        for antes in TODOS {
            for depois in TODOS {
                if !matches!((antes, depois), (Travado | SemCofre, Destravado)) {
                    assert!(
                        !ler_configuracao_na_passagem(antes, depois),
                        "{antes:?} -> {depois:?}"
                    );
                }
            }
        }
        assert!(!ler_configuracao_na_passagem(SemDaemon, Destravado));
        assert!(!ler_configuracao_na_passagem(Destravado, Destravado));
    }

    #[test]
    fn biblioteca_so_se_o_arquivo_existe() {
        let vazia = pasta_temporaria("lib-vazia");
        assert!(!biblioteca_disponivel(&[vazia.as_path()]));
        // Uma pasta com o nome da biblioteca não conta.
        std::fs::create_dir(vazia.join(BIBLIOTECAS[0])).unwrap();
        assert!(!biblioteca_disponivel(&[vazia.as_path()]));
        let cheia = pasta_temporaria("lib-cheia");
        std::fs::write(cheia.join(BIBLIOTECAS[1]), b"").unwrap();
        assert!(biblioteca_disponivel(&[vazia.as_path(), cheia.as_path()]));
        let _ = std::fs::remove_dir_all(vazia);
        let _ = std::fs::remove_dir_all(cheia);
    }

    #[test]
    fn pasta_dos_icones_criada_0700() {
        let uid = ward_plataforma::uid_efetivo();
        let rt = pasta_temporaria("rt-ok");
        let p = pasta_dos_icones(&rt, uid).unwrap();
        let m = std::fs::symlink_metadata(&p).unwrap();
        assert!(m.is_dir());
        assert_eq!(m.permissions().mode() & 0o777, 0o700);
        // De novo: a pasta já existe e é aceita.
        assert_eq!(pasta_dos_icones(&rt, uid).unwrap(), p);
        let _ = std::fs::remove_dir_all(rt);
    }

    /// Ataque: a pasta dos ícones trocada por um link (para o AppIndicator gravar o PNG
    /// em outro lugar), aberta a outros, de outro uid, ou `XDG_RUNTIME_DIR` relativo ou
    /// aberto: tudo recusado, e o app segue sem bandeja.
    #[test]
    fn ataque_pasta_dos_icones_link_aberta_ou_relativa() {
        let uid = ward_plataforma::uid_efetivo();

        let rt = pasta_temporaria("rt-link");
        let alvo = pasta_temporaria("rt-alvo");
        std::os::unix::fs::symlink(&alvo, rt.join(PASTA_DOS_ICONES)).unwrap();
        assert!(pasta_dos_icones(&rt, uid).is_err());

        let rt2 = pasta_temporaria("rt-aberta");
        let icones = rt2.join(PASTA_DOS_ICONES);
        std::fs::create_dir(&icones).unwrap();
        std::fs::set_permissions(&icones, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(pasta_dos_icones(&rt2, uid).is_err());

        let rt3 = pasta_temporaria("rt-runtime-aberto");
        std::fs::set_permissions(&rt3, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(pasta_dos_icones(&rt3, uid).is_err());
        assert!(!rt3.join(PASTA_DOS_ICONES).exists());

        let rt4 = pasta_temporaria("rt-outro-uid");
        assert!(pasta_dos_icones(&rt4, uid.wrapping_add(1)).is_err());

        assert!(pasta_dos_icones(Path::new("run/user/1000"), uid).is_err());

        let rt5 = pasta_temporaria("rt-arquivo");
        std::fs::write(rt5.join(PASTA_DOS_ICONES), b"").unwrap();
        assert!(pasta_dos_icones(&rt5, uid).is_err());

        for p in [rt, alvo, rt2, rt3, rt4, rt5] {
            let _ = std::fs::remove_dir_all(p);
        }
    }
}
