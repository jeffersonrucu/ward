//! Renomear sem sobrescrever dentro de uma pasta aberta (Fase 46).
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
}
