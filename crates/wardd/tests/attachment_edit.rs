//! Phase 39b (V5.12; S1, S11, D29): attachments on edit through the save-entry
//! operation with the keep-attachments list. The daemon copies the content of the kept attachments from the
//! edited entry; the content never goes over or comes back through the socket.
//! In-process daemon, test vault of its own in `target/tmp`, fake values.

mod common;

use common::*;
use ward_core::entry::Entry;
use ward_core::protocol::{ResponseError, Role, Resposta};

const ATTACHMENT_A: &[u8] = b"FALSO_A=falso-anexo-39b-a\n";
const ATTACHMENT_B: &[u8] = b"FALSO_B=falso-anexo-39b-b\n";

fn generic(name: &str, attachments: &[(&str, &[u8])]) -> String {
    let attachments: Vec<String> = attachments
        .iter()
        .map(|(n, c)| format!(r#"{{"nome":"{n}","conteudo":"{}"}}"#, b64(c)))
        .collect();
    format!(
        r#"{{"entrada":{{"nome":"{name}","grupo":"clientex","ambiente":"stage","tipo":"generico","extras":[{{"nome":"REGIAO","valor":"{}","segredo":false}}],"anexos":[{}]}}}}"#,
        b64(b"r-01"),
        attachments.join(",")
    )
}

/// Edits `previous` keeping `keep` and sending `new`.
fn edit(previous: &str, keep: &[&str], new: &[(&str, &[u8])]) -> String {
    let keep: Vec<String> = keep.iter().map(|n| format!(r#""{n}""#)).collect();
    generic(previous, new).replacen(
        '{',
        &format!(
            r#"{{"anterior":"{previous}","manter_anexos":[{}],"#,
            keep.join(",")
        ),
        1,
    )
}

/// The entries as they are on disk (opens with the test password).
fn on_disk(dir: &Dir) -> Vec<Entry> {
    let bytes = std::fs::read(dir.paths().cofre).unwrap();
    let s = ward_core::format::MasterPassword::new(PASSWORD.to_owned());
    ward_core::body::open_entries_with_generation(&bytes, &s, 0, 0)
        .unwrap()
        .1
}

fn attachments_on_disk(dir: &Dir, name: &str) -> Vec<(String, Vec<u8>)> {
    on_disk(dir)
        .iter()
        .find(|e| e.name() == name)
        .unwrap()
        .attachments()
        .iter()
        .map(|a| (a.name().to_owned(), a.content().expose().to_vec()))
        .collect()
}

/// V5.12: keep, add and remove an attachment of an entry already saved. The kept
/// content is the same byte for byte on disk, and no daemon answer carries it.
#[test]
fn v5_12_edit_keeps_adds_and_removes_attachment() {
    let dir = Dir::new("anexos-edicao");
    let (d, _) = test_vault(&dir);
    let r = request(
        &d,
        Role::App,
        "salvar_entrada",
        &generic("g1", &[("a.env", ATTACHMENT_A)]),
    );
    assert!(ok(&r), "{r:?}");

    // Keep `a.env` and add `b.env`.
    let r = request(
        &d,
        Role::App,
        "salvar_entrada",
        &edit("g1", &["a.env"], &[("b.env", ATTACHMENT_B)]),
    );
    assert!(ok(&r), "{r:?}");
    assert_eq!(
        attachments_on_disk(&dir, "g1"),
        [
            ("a.env".to_owned(), ATTACHMENT_A.to_vec()),
            ("b.env".to_owned(), ATTACHMENT_B.to_vec())
        ]
    );
    let det = request(&d, Role::App, "detalhe", r#"{"entrada":"g1"}"#);
    let Resposta::Detalhe(x) = &det else {
        panic!("{det:?}")
    };
    assert_eq!(x.anexos.len(), 2);
    let bytes = encoded(Role::App, &det);
    for c in [ATTACHMENT_A, ATTACHMENT_B] {
        assert!(!contains(&bytes, c) && !contains(&bytes, b64(c).as_bytes()));
    }

    // Rename keeping both: the content goes along.
    let rename =
        edit("g1", &["b.env", "a.env"], &[]).replacen(r#""nome":"g1""#, r#""nome":"g2""#, 1);
    let r = request(&d, Role::App, "salvar_entrada", &rename);
    assert!(ok(&r), "{r:?}");
    assert_eq!(
        attachments_on_disk(&dir, "g2"),
        [
            ("b.env".to_owned(), ATTACHMENT_B.to_vec()),
            ("a.env".to_owned(), ATTACHMENT_A.to_vec())
        ]
    );

    // Remove: what is not in the keep list goes away.
    let r = request(
        &d,
        Role::App,
        "salvar_entrada",
        &edit("g2", &["a.env"], &[]),
    );
    assert!(ok(&r), "{r:?}");
    assert_eq!(
        attachments_on_disk(&dir, "g2"),
        [("a.env".to_owned(), ATTACHMENT_A.to_vec())]
    );
    // Without the keep list (the request from before Phase 39b): all go away.
    let r = request(&d, Role::App, "salvar_entrada", &edit("g2", &[], &[]));
    assert!(ok(&r), "{r:?}");
    assert!(attachments_on_disk(&dir, "g2").is_empty());
}

/// Attacks: keeping an attachment of another entry, a nonexistent name, on a new entry,
/// through the agent or the extension, and kept + new above 256 KiB (D29). Nothing
/// changes on disk.
#[test]
fn v5_12_attack_forged_keep_attachments() {
    let dir = Dir::new("anexos-ataque");
    let (d, _) = test_vault(&dir);
    for q in [
        generic("g1", &[("a.env", ATTACHMENT_A)]),
        generic("g2", &[("b.env", ATTACHMENT_B)]),
    ] {
        assert!(ok(&request(&d, Role::App, "salvar_entrada", &q)));
    }
    let large = vec![b'x'; 200 * 1024];
    assert!(ok(&request(
        &d,
        Role::App,
        "salvar_entrada",
        &generic("g3", &[("grande.bin", &large)])
    )));
    let before = on_disk(&dir).len();

    // The attachment of `g2` cannot be reached by editing `g1`.
    let r = request(
        &d,
        Role::App,
        "salvar_entrada",
        &edit("g1", &["b.env"], &[]),
    );
    assert_eq!(error(&r), Some(ResponseError::InvalidRequest), "{r:?}");
    // Not even with a path.
    let r = request(
        &d,
        Role::App,
        "salvar_entrada",
        &edit("g1", &["../g2/b.env"], &[]),
    );
    assert_eq!(error(&r), Some(ResponseError::InvalidRequest), "{r:?}");
    // Edited entry that does not exist.
    let r = request(
        &d,
        Role::App,
        "salvar_entrada",
        &edit("nao-existe", &["a.env"], &[]),
    );
    assert_eq!(error(&r), Some(ResponseError::EntryNotFound), "{r:?}");
    // New entry "copying" the attachment of `g1`: reading the request refuses it.
    let new = generic("g9", &[]).replacen('{', r#"{"manter_anexos":["a.env"],"#, 1);
    let r = request(&d, Role::App, "salvar_entrada", &new);
    assert_eq!(error(&r), Some(ResponseError::InvalidRequest), "{r:?}");
    // Agent and extension do not have the operation.
    for role in [Role::Agent, Role::Extension] {
        let r = request(&d, role, "salvar_entrada", &edit("g1", &["a.env"], &[]));
        assert_eq!(error(&r), Some(ResponseError::Unauthorized), "{role:?}");
    }
    // 200 KiB kept + 60 KiB new go over the sum: validation error.
    let new60 = vec![b'y'; 60 * 1024];
    let r = request(
        &d,
        Role::App,
        "salvar_entrada",
        &edit("g3", &["grande.bin"], &[("mais.bin", &new60)]),
    );
    assert!(
        matches!(error(&r), Some(ResponseError::Validation(_))),
        "{r:?}"
    );
    // Kept name equal to that of a new attachment: validation error (repeated name).
    let r = request(
        &d,
        Role::App,
        "salvar_entrada",
        &edit("g1", &["a.env"], &[("a.env", ATTACHMENT_B)]),
    );
    assert!(
        matches!(error(&r), Some(ResponseError::Validation(_))),
        "{r:?}"
    );

    // Nothing changed on disk.
    assert_eq!(on_disk(&dir).len(), before);
    assert_eq!(
        attachments_on_disk(&dir, "g1"),
        [("a.env".to_owned(), ATTACHMENT_A.to_vec())]
    );
    assert_eq!(
        attachments_on_disk(&dir, "g2"),
        [("b.env".to_owned(), ATTACHMENT_B.to_vec())]
    );
    assert_eq!(attachments_on_disk(&dir, "g3").len(), 1);
}
