//! Checks by reading the source code of `ward-core` (the "auditor's grep" of V2.46 and
//! V2.50, Phase 19c). They read only the files of the crate's own `src/`; the test code
//! (`tests.rs` and the `tests/` modules, also under their mission 1 names) stays out.

use std::fs;
use std::path::{Path, PathBuf};

/// The production `.rs` files of the crate, with their content.
fn production_sources() -> Vec<(PathBuf, String)> {
    fn walk(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
        let mut items: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        items.sort();
        for p in items {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            if p.is_dir() {
                if name != "testes" && name != "tests" {
                    walk(&p, out);
                }
            } else if name.ends_with(".rs")
                && name != "testes.rs"
                && name != "tests.rs"
                && name != "test_vault.rs"
            {
                let text = fs::read_to_string(&p).unwrap();
                out.push((p, text));
            }
        }
    }
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut v = Vec::new();
    walk(&src, &mut v);
    assert!(v.len() >= 10, "não achou as fontes");
    v
}

/// Code lines (without line comments or docs).
fn code_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines().enumerate().filter_map(|(i, l)| {
        let l = l.trim_start();
        (!l.starts_with("//")).then_some((i + 1, l))
    })
}

/// V2.46 (D40): the data key and the KEK only exist on the heap, the size of the `Key`
/// is pinned by a compile-time assertion, and no production code calls the
/// `password-hash` APIs that return the hash outside a `Zeroizing`.
#[test]
fn v2_46_key_on_heap_and_no_hash_password() {
    let sources = production_sources();
    let crypto = &sources
        .iter()
        .find(|(p, _)| p.ends_with("src/crypto.rs"))
        .expect("crypto.rs")
        .1;
    assert!(
        crypto.contains("assert!(core::mem::size_of::<Key>() == core::mem::size_of::<usize>())"),
        "falta a asserção de compilação do tamanho da Chave"
    );
    assert!(crypto.contains("struct Key(Box<Zeroizing<[u8; KEY_SIZE]>>)"));
    assert!(crypto.contains("let mut buf: Zeroizing<[u8; MAX_PIN_HASH]>"));
    assert!(
        crypto.contains(".ct_eq("),
        "conferir_pin compara pelo ctutils"
    );

    for (p, text) in &sources {
        for (n, l) in code_lines(text) {
            for forbidden in [
                "hash_password(",
                "verify_password(",
                "PasswordHasher",
                "PasswordVerifier",
            ] {
                assert!(!l.contains(forbidden), "{}:{n}: {forbidden}", p.display());
            }
            // 32-byte key by value: only inside the `Box` of the `Key`.
            // By reference (`&[u8; KEY_SIZE]`) it may; by value, no.
            let without_ref = l
                .replace("&[u8; KEY_SIZE]", "")
                .replace("&mut [u8; KEY_SIZE]", "")
                .replace("Box<Zeroizing<[u8; KEY_SIZE]>>", "")
                // The `Deref` of the `Key` returns a reference.
                .replace("type Target = [u8; KEY_SIZE];", "");
            for key in ["[u8; KEY_SIZE]", "Zeroizing<[u8; 32]>"] {
                assert!(
                    !without_ref.contains(key),
                    "{}:{n}: chave por valor fora do Box: {l}",
                    p.display()
                );
            }
            // Hand-made MAC/hash comparison (`==` on a hash slice) does not exist in the crypto.
            if p.ends_with("src/crypto.rs") {
                assert!(!l.contains("output[..] =="), "{}:{n}", p.display());
            }
        }
    }
}

/// V2.50 (D44, D27): the only `pub fn` that write the vault are `write_entries` and
/// `atomic_write::change_password_from_bytes` (Phase 55: with the bytes the daemon read)
/// and the variants with backup of Phase 45 (which write the same bytes, already sealed
/// by them, to the backups directory), plus `clean_temp_files`, which only deletes
/// temporary files. Only `atomic_write.rs` writes a file (`backup.rs` only lists and
/// deletes old versions), and `seal`/`write_vault`/`write` are `pub(crate)`.
#[test]
fn v2_50_only_public_writes() {
    let sources = production_sources();
    let mut public = Vec::new();
    for (p, text) in &sources {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        for (n, l) in code_lines(text) {
            for write_call in [
                "fs::write",
                "File::create",
                "OpenOptions",
                "fs::rename",
                "renameat",
                "fs::hard_link",
                "linkat",
            ] {
                if l.contains(write_call) {
                    assert_eq!(name, "atomic_write.rs", "{}:{n}: {l}", p.display());
                }
            }
            if name == "atomic_write.rs" && l.starts_with("pub fn ") {
                let f = l["pub fn ".len()..].split('(').next().unwrap().to_owned();
                public.push(f);
            }
            if l.contains("fn seal(") || l.contains("fn write_vault(") || l.contains("fn write(") {
                assert!(
                    l.starts_with("pub(crate) fn") || l.starts_with("fn"),
                    "{}:{n}: {l}",
                    p.display()
                );
            }
        }
    }
    public.sort();
    assert_eq!(
        public,
        [
            "change_password_from_bytes",
            "change_password_with_backup_from_bytes",
            "clean_temp_files",
            "write_entries",
            "write_entries_with_backup"
        ],
        "pub fn de atomic_write.rs"
    );
    // Both serialize or deserialize the body before sealing.
    let atomic_write_src = &sources
        .iter()
        .find(|(p, _)| p.ends_with("src/atomic_write.rs"))
        .unwrap()
        .1;
    let body_of = |f: &str| {
        let i = atomic_write_src.find(&format!("fn {f}(")).unwrap();
        let end = atomic_write_src[i..].find("\n}\n").unwrap();
        atomic_write_src[i..i + end].to_owned()
    };
    assert!(body_of("write_entries").contains("write_entries_with_points"));
    assert!(body_of("write_entries_with_points").contains("write_entries_sealed("));
    assert!(body_of("write_entries_with_backup").contains("write_entries_sealed("));
    assert!(
        body_of("write_entries_sealed")
            .contains("body::serialize(vault.config(), generation, entries)")
    );
    assert!(body_of("change_password_from_bytes").contains("change_password_sealed("));
    assert!(body_of("change_password_with_backup_from_bytes").contains("change_password_sealed("));
    let change = body_of("change_password_sealed");
    assert!(change.contains("body::deserialize(&content)"));
    assert!(change.contains("write_entries_sealed(destination, &mut vault, &entries"));
}
