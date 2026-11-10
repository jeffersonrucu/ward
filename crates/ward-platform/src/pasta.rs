//! Renomear sem sobrescrever dentro de uma pasta aberta (Fase 46) e apagar o que há
//! numa pasta aberta pelo fd, sem seguir link (Fase 44e).
//!
//! O backup externo vai para pendrive, quase sempre vfat, que não tem `link(2)`: o
//! "temporário → `link` para o nome final" do backup local (Fase 45) não serve lá.
//! [`renomear_sem_sobrescrever`] usa `renameat2(RENAME_NOREPLACE)` relativo ao fd da
//! pasta: o nome final só aparece inteiro, e uma versão que já existe nunca é trocada
//! (`AlreadyExists`).

use std::ffi::{CString, OsStr};
use std::io;
use std::os::fd::BorrowedFd;
use std::os::unix::ffi::OsStrExt;

use crate::sys;

/// Nome simples (sem `/`, sem NUL, nem `.`/`..`) para uma chamada `*at`.
fn nome_c(nome: &OsStr) -> io::Result<CString> {
    let b = nome.as_bytes();
    if b.is_empty() || b == b"." || b == b".." || b.contains(&b'/') {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    CString::new(b).map_err(|_| io::ErrorKind::InvalidInput.into())
}

/// Renomeia `de` para `para`, os dois dentro da pasta `dir`, sem sobrescrever: se
/// `para` existe (inclusive um link simbólico, que não é seguido), `AlreadyExists` e
/// nada muda. Nomes com `/`, NUL, `.` ou `..` são recusados (`InvalidInput`): a
/// operação nunca sai da pasta.
pub fn renomear_sem_sobrescrever(dir: BorrowedFd<'_>, de: &OsStr, para: &OsStr) -> io::Result<()> {
    let (de, para) = (nome_c(de)?, nome_c(para)?);
    sys::renameat_sem_sobrescrever(dir, &de, &para)
}

/// Mais fundo que isto, [`apagar_conteudo`] para (as pastas do Ward têm um nível).
const MAX_PROFUNDIDADE: usize = 16;

/// Apaga tudo o que há dentro da pasta aberta em `dir`, pelo fd (`readdir` no fd,
/// `unlinkat` relativo a ele): nada é resolvido pelo caminho de novo, então trocar a
/// pasta por um link depois de aberta não leva a apagar outra. Um link dentro dela é
/// apagado como ele é (o alvo fica); uma subpasta é aberta com
/// `O_DIRECTORY | O_NOFOLLOW` relativo ao fd e esvaziada do mesmo jeito. Devolve
/// quantos itens do primeiro nível sumiram; o que falhar fica (e não para os outros).
pub fn apagar_conteudo(dir: BorrowedFd<'_>) -> usize {
    apagar_conteudo_em(dir, 0)
}

fn apagar_conteudo_em(dir: BorrowedFd<'_>, profundidade: usize) -> usize {
    let Ok(nomes) = sys::listar_pasta(dir) else {
        return 0;
    };
    nomes
        .iter()
        .filter(|nome| apagar_item(dir, nome, profundidade).is_ok())
        .count()
}

fn apagar_item(dir: BorrowedFd<'_>, nome: &std::ffi::CStr, profundidade: usize) -> io::Result<()> {
    match sys::unlinkat(dir, nome, 0) {
        Err(e) if e.raw_os_error() == Some(libc::EISDIR) => {}
        r => return r,
    }
    if profundidade >= MAX_PROFUNDIDADE {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let sub = sys::openat(
        dir,
        nome,
        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW,
    )?;
    use std::os::fd::AsFd;
    apagar_conteudo_em(sub.as_fd(), profundidade + 1);
    sys::unlinkat(dir, nome, libc::AT_REMOVEDIR)
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::fs;
    use std::os::fd::AsFd;
    use std::path::PathBuf;

    fn dir(rotulo: &str) -> PathBuf {
        let d = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/teste-plataforma")
            .join(format!("pasta-{rotulo}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn renomeia_e_nunca_sobrescreve() {
        let d = dir("renomear");
        fs::write(d.join("tmp"), b"novo").unwrap();
        fs::write(d.join("versao"), b"antiga").unwrap();
        let f = fs::File::open(&d).unwrap();
        let e =
            renomear_sem_sobrescrever(f.as_fd(), "tmp".as_ref(), "versao".as_ref()).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(d.join("versao")).unwrap(), b"antiga");
        assert_eq!(fs::read(d.join("tmp")).unwrap(), b"novo");
        renomear_sem_sobrescrever(f.as_fd(), "tmp".as_ref(), "outra".as_ref()).unwrap();
        assert_eq!(fs::read(d.join("outra")).unwrap(), b"novo");
        assert!(!d.join("tmp").exists());
        // Link simbólico no nome final também "existe" e não é seguido.
        std::os::unix::fs::symlink("/nao/existe", d.join("link")).unwrap();
        fs::write(d.join("tmp2"), b"x").unwrap();
        let e = renomear_sem_sobrescrever(f.as_fd(), "tmp2".as_ref(), "link".as_ref()).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::AlreadyExists);
        assert!(fs::symlink_metadata(d.join("link")).unwrap().is_symlink());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn ataque_nome_que_sai_da_pasta_recusado() {
        let d = dir("fora");
        fs::write(d.join("tmp"), b"x").unwrap();
        let f = fs::File::open(&d).unwrap();
        for ruim in ["../fora", "a/b", "..", ".", "", "a\0b"] {
            let e =
                renomear_sem_sobrescrever(f.as_fd(), "tmp".as_ref(), ruim.as_ref()).unwrap_err();
            assert_eq!(e.kind(), io::ErrorKind::InvalidInput, "{ruim:?}");
            let e = renomear_sem_sobrescrever(f.as_fd(), ruim.as_ref(), "x".as_ref()).unwrap_err();
            assert_eq!(e.kind(), io::ErrorKind::InvalidInput, "{ruim:?}");
        }
        assert!(d.join("tmp").exists());
        assert!(!d.join("../fora").exists());
        fs::remove_dir_all(&d).unwrap();
    }

    /// Fase 44e: o conteúdo some pelo fd; link dentro da pasta é apagado sem seguir, e a
    /// subpasta também.
    #[test]
    fn apaga_o_conteudo_pelo_fd_sem_seguir_link() {
        let d = dir("apagar");
        let alvo = d.join("alvo");
        fs::create_dir_all(alvo.join("sub")).unwrap();
        fs::write(alvo.join("precioso.txt"), b"x").unwrap();
        let p = d.join("pasta");
        fs::create_dir_all(p.join("sub/mais")).unwrap();
        fs::write(p.join("a.json"), b"{}").unwrap();
        fs::write(p.join("sub/b.json"), b"{}").unwrap();
        fs::write(p.join("sub/mais/c.json"), b"{}").unwrap();
        std::os::unix::fs::symlink(&alvo, p.join("ligacao")).unwrap();
        std::os::unix::fs::symlink(&alvo, p.join("sub/ligacao")).unwrap();
        let f = fs::File::open(&p).unwrap();
        assert_eq!(apagar_conteudo(f.as_fd()), 3);
        assert_eq!(fs::read_dir(&p).unwrap().count(), 0);
        assert!(alvo.join("precioso.txt").exists());
        assert!(alvo.join("sub").is_dir());
        // De novo, vazia: nada a apagar.
        assert_eq!(apagar_conteudo(f.as_fd()), 0);
        fs::remove_dir_all(&d).unwrap();
    }

    /// Ataque (Fase 44e): a pasta é aberta e depois trocada por um link para outra; o
    /// fd continua na original (já fora do lugar), e a outra fica intacta.
    #[test]
    fn ataque_pasta_trocada_por_link_depois_de_aberta() {
        let d = dir("trocada");
        let alvo = d.join("alvo");
        fs::create_dir_all(&alvo).unwrap();
        fs::write(alvo.join("precioso.txt"), b"x").unwrap();
        let p = d.join("pasta");
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("a.json"), b"{}").unwrap();
        let f = fs::File::open(&p).unwrap();
        fs::rename(&p, d.join("movida")).unwrap();
        std::os::unix::fs::symlink(&alvo, &p).unwrap();
        assert_eq!(apagar_conteudo(f.as_fd()), 1);
        assert!(alvo.join("precioso.txt").exists());
        assert!(!d.join("movida/a.json").exists());
        fs::remove_dir_all(&d).unwrap();
    }
}
