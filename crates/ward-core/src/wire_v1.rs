//! Wire tests against the 0.1.4 baseline (m2:DT18, m2:V2.2; Phase 3 of mission 2).
//!
//! The fixtures in `dev/fixtures/legacy/` were produced by the code of the tag `v0.1.4`
//! (handoff of Phase 3) and are never regenerated. Until the format and the protocol
//! change on purpose (Phases 48 and 50), every rename must keep these tests green
//! without touching their expectations:
//!
//! - `protocol-v1.jsonl`: one line per request (every operation), per response sample
//!   (every response type) and per simple error code, each one
//!   `{"kind":"<request|response|error>","role":"<role>","frame":<frame>}` with the
//!   frame exactly as the 0.1.4 code wrote it. Each frame is read by the current code
//!   with that role and written again, and the bytes must be the same: a renamed
//!   operation, field, error code, role or value breaks the reading (strict) or the
//!   writing.
//! - `vault-0.1.4.ward`: the test vault (D15 entries, fake values only), body v4,
//!   generation 3. It opens with the test password; the body read and written again
//!   gives the same bytes, and the names, types and levels are the ones 0.1.4 wrote.
//! - `backup-0.1.4/`: the two backup versions of the same vault (generations 2 and 3);
//!   the newest is the vault itself.
//!
//! Only fake values (D15). Nothing here prints a value: the comparisons are on bytes.

use std::collections::BTreeSet;

use crate::corpo;
use crate::format::{self, MasterPassword};
use crate::protocolo::{self, ErroResposta, Operacao, Papel, Resposta, TipoResposta, VERSAO};

const PROTOCOL_V1: &[u8] = include_bytes!("../../../dev/fixtures/legacy/protocol-v1.jsonl");
const VAULT: &[u8] = include_bytes!("../../../dev/fixtures/legacy/vault-0.1.4.ward");
const BACKUP_GEN_2: &[u8] =
    include_bytes!("../../../dev/fixtures/legacy/backup-0.1.4/ward-20261110-213053.ward");
const BACKUP_GEN_3: &[u8] =
    include_bytes!("../../../dev/fixtures/legacy/backup-0.1.4/ward-20261110-213054.ward");

/// Test master password (D15).
const TEST_PASSWORD: &str = "falso-mestra-ward-01";

/// Entries of the test vault as 0.1.4 wrote them: name, stored type name and stored
/// level name (body v4).
const ENTRIES_V4: [(&str, &str, &str); 22] = [
    ("acme-pg-aprovacao", "banco", "com-aprovacao"),
    ("acme-pg-dev", "banco", "livre"),
    ("acme-pg-prod", "banco", "com-aprovacao"),
    ("acme-pg-tunel", "banco", "livre"),
    ("acme-ssh-chave-prod", "servidor", "livre"),
    ("acme-ssh-interno", "servidor", "livre"),
    ("acme-ssh-interno-aprov", "servidor", "livre"),
    ("acme-ssh-salto", "servidor", "com-aprovacao"),
    ("acme-ssh-salto-livre", "servidor", "livre"),
    ("acme-ssh-senha-stage", "servidor", "livre"),
    ("acme-totp-dev", "login", "livre"),
    ("acme-wp-dev", "login", "com-aprovacao"),
    ("acme-wp-prod", "login", "livre"),
    ("beta-aws-dev", "aws", "livre"),
    ("beta-github-api", "api", "livre"),
    ("beta-sftp-dev", "ftp-sftp", "livre"),
    ("beta-smtp-stage", "smtp", "livre"),
    ("delta-api-simbolos", "api", "livre"),
    ("delta-redir", "login", "livre"),
    ("gama-bloqueada", "login", "bloqueado"),
    ("gama-generico", "generico", "livre"),
    ("gama-nota", "nota-segura", "livre"),
];

/// One line of `protocol-v1.jsonl`.
struct Line<'a> {
    kind: &'a str,
    role: Papel,
    frame: &'a [u8],
}

/// Splits the fixed shape of a line. The fixture is ours (written by the generator of
/// Phase 3), so any other shape is a broken fixture.
fn parse_line(line: &[u8]) -> Line<'_> {
    let text = std::str::from_utf8(line).expect("fixture line is not UTF-8");
    let rest = text
        .strip_prefix(r#"{"kind":""#)
        .expect("fixture line without kind");
    let (kind, rest) = rest.split_once('"').expect("fixture kind not closed");
    let rest = rest
        .strip_prefix(r#","role":""#)
        .expect("fixture line without role");
    let (role, rest) = rest.split_once('"').expect("fixture role not closed");
    let frame = rest
        .strip_prefix(r#","frame":"#)
        .and_then(|f| f.strip_suffix('}'))
        .expect("fixture line without frame");
    Line {
        kind,
        role: Papel::de_nome(role).expect("fixture role unknown"),
        frame: frame.as_bytes(),
    }
}

fn lines() -> Vec<Line<'static>> {
    assert_eq!(
        PROTOCOL_V1.last(),
        Some(&b'\n'),
        "fixture must end with a newline"
    );
    PROTOCOL_V1[..PROTOCOL_V1.len() - 1]
        .split(|&b| b == b'\n')
        .map(parse_line)
        .collect()
}

#[test]
fn v1_fixture_is_version_1() {
    // Phase 50 moves the protocol to v2 on purpose; until then the version is 1.
    assert_eq!(VERSAO, 1);
    for l in lines() {
        assert!(l.frame.starts_with(br#"{"v":1,"#));
    }
}

#[test]
fn v1_requests_read_and_written_byte_for_byte() {
    let mut seen = Vec::new();
    for l in lines().into_iter().filter(|l| l.kind == "request") {
        let request = protocolo::decodificar_pedido(l.role, l.frame)
            .unwrap_or_else(|_| panic!("v1 request refused: {}", String::from_utf8_lossy(l.frame)));
        let again = protocolo::codificar_pedido(l.role, &request).expect("request not written");
        assert_eq!(
            &again[..],
            l.frame,
            "request written differently from 0.1.4"
        );
        seen.push(request.operacao());
    }
    // One line per operation, in the order of the table.
    assert_eq!(seen, Operacao::TODAS);
}

#[test]
fn v1_responses_read_and_written_byte_for_byte() {
    let mut types = BTreeSet::new();
    let mut count = 0;
    for l in lines().into_iter().filter(|l| l.kind == "response") {
        let response = protocolo::decodificar_resposta(l.role, l.frame)
            .unwrap_or_else(|_| panic!("v1 response refused (kind {})", l.kind));
        let again = protocolo::codificar_resposta(l.role, &response).expect("response not written");
        assert_eq!(
            &again[..],
            l.frame,
            "response written differently from 0.1.4"
        );
        types.insert(response.tipo().nome());
        count += 1;
    }
    let all: BTreeSet<_> = TipoResposta::TODAS.iter().map(|t| t.nome()).collect();
    assert_eq!(types, all, "every response type has a line");
    assert_eq!(count, 23);
}

#[test]
fn v1_error_codes_read_and_written_byte_for_byte() {
    let mut codes = Vec::new();
    for l in lines().into_iter().filter(|l| l.kind == "error") {
        let response = protocolo::decodificar_resposta(l.role, l.frame)
            .unwrap_or_else(|_| panic!("v1 error refused: {}", String::from_utf8_lossy(l.frame)));
        let again = protocolo::codificar_resposta(l.role, &response).expect("error not written");
        assert_eq!(&again[..], l.frame, "error written differently from 0.1.4");
        match response {
            Resposta::Erro(e) => codes.push(e),
            _ => panic!("error line is not an error"),
        }
    }
    assert_eq!(codes, ErroResposta::SIMPLES);
}

#[test]
fn v1_fixture_has_only_known_kinds() {
    let all = lines();
    assert_eq!(
        all.len(),
        Operacao::TODAS.len() + 23 + ErroResposta::SIMPLES.len()
    );
    assert!(
        all.iter()
            .all(|l| matches!(l.kind, "request" | "response" | "error"))
    );
}

/// Opens a 0.1.4 file and returns the decrypted body (zeroed on drop).
fn body_of(file: &[u8]) -> zeroize::Zeroizing<Vec<u8>> {
    let password = MasterPassword::new(TEST_PASSWORD.to_owned());
    let (_keys, body) = format::open(file, &password).expect("0.1.4 file does not open");
    body
}

#[test]
fn vault_014_body_v4_read_and_written_byte_for_byte() {
    let body = body_of(VAULT);
    // Body v4 (u16 LE at the start) until Phase 48 moves it to v5 on purpose.
    assert_eq!(corpo::VERSAO_CORPO, 4);
    assert_eq!(u16::from_le_bytes([body[0], body[1]]), 4);
    let (config, generation, entries) = corpo::desserializar(&body).expect("body v4 refused");
    assert_eq!(generation, 3);
    let again = corpo::serializar(&config, generation, &entries).expect("body not written");
    assert!(*again == *body, "body written differently from 0.1.4");
    let got: Vec<(&str, &str, &str)> = entries
        .iter()
        .map(|e| (e.name(), e.kind().name(), e.level().name()))
        .collect();
    assert_eq!(got, ENTRIES_V4);
    assert_eq!(config.lock_minutes(), 240);
}

#[test]
fn backup_014_two_consecutive_generations() {
    assert_eq!(BACKUP_GEN_3, VAULT, "the newest backup is the vault itself");
    let older = body_of(BACKUP_GEN_2);
    let (config, generation, entries) = corpo::desserializar(&older).expect("backup refused");
    assert_eq!(generation, 2);
    // Same configuration and entries, one generation before: written at generation 3
    // they give the body of the vault.
    let again = corpo::serializar(&config, 3, &entries).expect("body not written");
    assert!(*again == *body_of(VAULT));
}

// ---- Attacks (m2:RM1, S6, S2) ----

/// RM1: a rename that reaches the wire (operation or field) is not silently accepted
/// by the v1 reader; the wire tests above depend on this strictness.
#[test]
fn attack_renamed_wire_name_is_refused() {
    for l in lines().into_iter().filter(|l| l.kind == "request") {
        let frame = std::str::from_utf8(l.frame).unwrap();
        let (head, tail) = frame.split_once(r#""op":""#).unwrap();
        let renamed = format!(r#"{head}"op":"x_{tail}"#);
        assert_eq!(
            protocolo::decodificar_pedido(l.role, renamed.as_bytes()).unwrap_err(),
            ErroResposta::OperacaoDesconhecida
        );
        if frame.contains(r#""entrada":"#) {
            let renamed = frame.replacen(r#""entrada":"#, r#""entry":"#, 1);
            assert_eq!(
                protocolo::decodificar_pedido(l.role, renamed.as_bytes()).unwrap_err(),
                ErroResposta::PedidoInvalido
            );
        }
    }
}

/// S6: one byte changed anywhere in the 0.1.4 file and it does not open.
#[test]
fn attack_vault_014_byte_flipped_does_not_open() {
    let password = MasterPassword::new(TEST_PASSWORD.to_owned());
    for pos in [0, 12, 100, VAULT.len() / 2, VAULT.len() - 1] {
        let mut file = VAULT.to_vec();
        file[pos] ^= 0x01;
        assert!(
            format::open(&file, &password).is_err(),
            "byte {pos} flipped opened"
        );
    }
    let wrong = MasterPassword::new("falso-mestra-ward-02".to_owned());
    assert!(format::open(VAULT, &wrong).is_err());
}

/// S2, m2:V1.7: the fixtures in git are sealed; no fake value of the test vault is in
/// them in clear.
#[test]
fn vault_and_backups_014_have_no_value_in_clear() {
    const VALUES: [&str; 12] = [
        TEST_PASSWORD,
        "falso-WP-senha-01",
        "falso-PG:s3nha/01@#",
        "falso-SSH-senha-01",
        "falso-passphrase-01",
        "falso_token_github_01",
        "falso-aws-secret-01",
        "falso-SMTP-01",
        "falso-nota-01",
        "falso-licenca-01",
        "falso-anexo-01",
        "JBSWY3DPEHPK3PXP",
    ];
    for file in [VAULT, BACKUP_GEN_2, BACKUP_GEN_3] {
        for v in VALUES {
            assert!(
                !file.windows(v.len()).any(|w| w == v.as_bytes()),
                "a fake value is in clear in a 0.1.4 file"
            );
        }
        assert!(!file.windows(5).any(|w| w == b"falso"));
        assert!(!file.windows(7).any(|w| w == b"OPENSSH"));
    }
}
