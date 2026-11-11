//! Conferências por leitura do código-fonte do `ward-core` (o "grep do auditor" de
//! V2.46 e V2.50, Fase 19c). Leem só os arquivos de `src/` do próprio crate; o código
//! de teste (`testes.rs` e os módulos `testes/`) fica de fora.

use std::fs;
use std::path::{Path, PathBuf};

/// Os arquivos `.rs` de produção do crate, com o conteúdo.
fn fontes_de_producao() -> Vec<(PathBuf, String)> {
    fn andar(dir: &Path, saida: &mut Vec<(PathBuf, String)>) {
        let mut itens: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        itens.sort();
        for p in itens {
            let nome = p.file_name().unwrap().to_string_lossy().into_owned();
            if p.is_dir() {
                if nome != "testes" && nome != "tests" {
                    andar(&p, saida);
                }
            } else if nome.ends_with(".rs")
                && nome != "testes.rs"
                && nome != "tests.rs"
                && nome != "test_vault.rs"
            {
                let texto = fs::read_to_string(&p).unwrap();
                saida.push((p, texto));
            }
        }
    }
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut v = Vec::new();
    andar(&src, &mut v);
    assert!(v.len() >= 10, "não achou as fontes");
    v
}

/// Linhas de código (sem comentário de linha nem doc).
fn codigo(texto: &str) -> impl Iterator<Item = (usize, &str)> {
    texto.lines().enumerate().filter_map(|(i, l)| {
        let l = l.trim_start();
        (!l.starts_with("//")).then_some((i + 1, l))
    })
}

/// V2.46 (D40): a chave de dados e a KEK só existem no heap, o tamanho da `Chave` é
/// preso por asserção de compilação, e nenhum código de produção chama as APIs do
/// `password-hash` que devolvem o hash fora de um `Zeroizing`.
#[test]
fn v2_46_chave_no_heap_e_sem_hash_password() {
    let fontes = fontes_de_producao();
    let cripto = &fontes
        .iter()
        .find(|(p, _)| p.ends_with("src/crypto.rs"))
        .expect("crypto.rs")
        .1;
    assert!(
        cripto.contains("assert!(core::mem::size_of::<Key>() == core::mem::size_of::<usize>())"),
        "falta a asserção de compilação do tamanho da Chave"
    );
    assert!(cripto.contains("struct Key(Box<Zeroizing<[u8; KEY_SIZE]>>)"));
    assert!(cripto.contains("let mut buf: Zeroizing<[u8; MAX_PIN_HASH]>"));
    assert!(
        cripto.contains(".ct_eq("),
        "conferir_pin compara pelo ctutils"
    );

    for (p, texto) in &fontes {
        for (n, l) in codigo(texto) {
            for proibida in [
                "hash_password(",
                "verify_password(",
                "PasswordHasher",
                "PasswordVerifier",
            ] {
                assert!(!l.contains(proibida), "{}:{n}: {proibida}", p.display());
            }
            // Chave de 32 bytes por valor: só dentro do `Box` da `Chave`.
            // Por referência (`&[u8; KEY_SIZE]`) pode; por valor, não.
            let sem_ref = l
                .replace("&[u8; KEY_SIZE]", "")
                .replace("&mut [u8; KEY_SIZE]", "")
                .replace("Box<Zeroizing<[u8; KEY_SIZE]>>", "")
                // O `Deref` da `Chave` devolve referência.
                .replace("type Target = [u8; KEY_SIZE];", "");
            for chave in ["[u8; KEY_SIZE]", "Zeroizing<[u8; 32]>"] {
                assert!(
                    !sem_ref.contains(chave),
                    "{}:{n}: chave por valor fora do Box: {l}",
                    p.display()
                );
            }
            // Comparação de MAC/hash à mão (`==` em fatia de hash) não existe no cripto.
            if p.ends_with("src/crypto.rs") {
                assert!(!l.contains("output[..] =="), "{}:{n}", p.display());
            }
        }
    }
}

/// V2.50 (D44, D27): os únicos `pub fn` que gravam o cofre são `write_entries` e
/// `atomic_write::change_password_from_bytes` (Fase 55: com os bytes que o daemon leu) e as
/// variantes com backup da Fase 45 (que gravam os mesmos
/// bytes já selados por elas na pasta de backups), mais `clean_temp_files`, que só
/// apaga temporários. Só `atomic_write.rs` escreve arquivo (o `backup.rs` só lista e apaga
/// versões velhas), e `seal`/`write_vault`/`write` são `pub(crate)`.
#[test]
fn v2_50_unicas_gravacoes_publicas() {
    let fontes = fontes_de_producao();
    let mut publicas = Vec::new();
    for (p, texto) in &fontes {
        let nome = p.file_name().unwrap().to_string_lossy().into_owned();
        for (n, l) in codigo(texto) {
            for escrita in [
                "fs::write",
                "File::create",
                "OpenOptions",
                "fs::rename",
                "renameat",
                "fs::hard_link",
                "linkat",
            ] {
                if l.contains(escrita) {
                    assert_eq!(nome, "atomic_write.rs", "{}:{n}: {l}", p.display());
                }
            }
            if nome == "atomic_write.rs" && l.starts_with("pub fn ") {
                let f = l["pub fn ".len()..].split('(').next().unwrap().to_owned();
                publicas.push(f);
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
    publicas.sort();
    assert_eq!(
        publicas,
        [
            "write_entries",
            "write_entries_with_backup",
            "clean_temp_files",
            "change_password_with_backup_from_bytes",
            "change_password_from_bytes"
        ],
        "pub fn de atomic_write.rs"
    );
    // Os dois serializam ou desserializam o corpo antes de selar.
    let gravacao = &fontes
        .iter()
        .find(|(p, _)| p.ends_with("src/atomic_write.rs"))
        .unwrap()
        .1;
    let corpo_de = |f: &str| {
        let i = gravacao.find(&format!("fn {f}(")).unwrap();
        let fim = gravacao[i..].find("\n}\n").unwrap();
        gravacao[i..i + fim].to_owned()
    };
    assert!(corpo_de("write_entries").contains("write_entries_with_points"));
    assert!(corpo_de("write_entries_with_points").contains("write_entries_sealed("));
    assert!(corpo_de("write_entries_with_backup").contains("write_entries_sealed("));
    assert!(
        corpo_de("write_entries_sealed")
            .contains("body::serialize(vault.config(), generation, entries)")
    );
    assert!(corpo_de("change_password_from_bytes").contains("change_password_sealed("));
    assert!(corpo_de("change_password_with_backup_from_bytes").contains("change_password_sealed("));
    let troca = corpo_de("change_password_sealed");
    assert!(troca.contains("body::deserialize(&content)"));
    assert!(troca.contains("write_entries_sealed(destination, &mut vault, &entries"));
}
