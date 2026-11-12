//! Wire tests of the protocol v2 (m2:DT8, m2:V4.13–V4.15; Phase 50 of mission 2).
//!
//! `dev/fixtures/protocol-v2.jsonl` is `dev/fixtures/legacy/protocol-v1.jsonl` (the
//! 0.1.4 frames) with every name translated by the tables of m2:DT8 (glossary, section
//! 10) by a script independent of this code, and the version set to 2. It has the same
//! lines, in the same order, with the same values: only the names, the version and the
//! message of `unknown_version` (m2:DT8) changed. Each frame is read by the current
//! code with the role of the line and written again, and the bytes must be the same: a
//! name that differs from the table breaks the reading (strict) or the writing.
//!
//! Only fake values (D15). Nothing here prints a value: the comparisons are on bytes.

use std::collections::BTreeSet;

use crate::protocol::{self, Operation, Response, ResponseError, ResponseType, Role, VERSION};

const PROTOCOL_V2: &[u8] = include_bytes!("../../../dev/fixtures/protocol-v2.jsonl");
const PROTOCOL_V1: &[u8] = include_bytes!("../../../dev/fixtures/legacy/protocol-v1.jsonl");

/// One line of `protocol-v2.jsonl`.
struct Line<'a> {
    kind: &'a str,
    role: Role,
    frame: &'a [u8],
}

/// Splits the fixed shape of a line (the same shape as the v1 fixture).
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
        role: Role::from_name(role).expect("fixture role unknown"),
        frame: frame.as_bytes(),
    }
}

fn split(fixture: &[u8]) -> Vec<&[u8]> {
    assert_eq!(
        fixture.last(),
        Some(&b'\n'),
        "fixture must end with a newline"
    );
    fixture[..fixture.len() - 1]
        .split(|&b| b == b'\n')
        .collect()
}

fn lines() -> Vec<Line<'static>> {
    split(PROTOCOL_V2).into_iter().map(parse_line).collect()
}

#[test]
fn v2_fixture_is_version_2() {
    assert_eq!(VERSION, 2);
    for l in lines() {
        assert!(l.frame.starts_with(br#"{"v":2,"#));
    }
}

/// The v2 fixture is the v1 one line by line: same count, same kinds, same order.
#[test]
fn v2_fixture_has_the_lines_of_v1() {
    let v1 = split(PROTOCOL_V1);
    let v2 = lines();
    assert_eq!(v1.len(), v2.len());
    for (a, b) in v1.iter().zip(&v2) {
        let kind = |l: &[u8]| {
            let t = std::str::from_utf8(l).unwrap();
            t[9..t.find("\",").unwrap()].to_owned()
        };
        assert_eq!(kind(a), b.kind);
    }
}

#[test]
fn v2_requests_read_and_written_byte_for_byte() {
    let mut seen = Vec::new();
    for l in lines().into_iter().filter(|l| l.kind == "request") {
        let request = protocol::decode_request(l.role, l.frame).unwrap_or_else(|e| {
            panic!(
                "v2 request refused ({e:?}): {}",
                String::from_utf8_lossy(l.frame)
            )
        });
        let again = protocol::encode_request(l.role, &request).expect("request not written");
        assert_eq!(
            &again[..],
            l.frame,
            "request written differently from the fixture: {}",
            String::from_utf8_lossy(l.frame)
        );
        seen.push(request.operation());
    }
    // One line per operation, in the order of the table.
    assert_eq!(seen, Operation::ALL);
}

#[test]
fn v2_responses_read_and_written_byte_for_byte() {
    let mut types = BTreeSet::new();
    let mut count = 0;
    for l in lines().into_iter().filter(|l| l.kind == "response") {
        let response = protocol::decode_response(l.role, l.frame).unwrap_or_else(|_| {
            panic!(
                "v2 response refused: {}",
                String::from_utf8_lossy(&l.frame[..40.min(l.frame.len())])
            )
        });
        let again = protocol::encode_response(l.role, &response).expect("response not written");
        assert_eq!(
            &again[..],
            l.frame,
            "response written differently from the fixture"
        );
        types.insert(response.kind().name());
        count += 1;
    }
    // `show_window` came after 0.1.4 (port of 0.1.5, Phase 66a of mission 2): the
    // fixture is the 0.1.4 one, line by line; its frame is checked byte for byte in
    // `protocol::tests::attack_show_window_app_only_and_without_data`.
    let all: BTreeSet<_> = ResponseType::ALL
        .iter()
        .map(|t| t.name())
        .filter(|n| *n != "show_window")
        .collect();
    assert_eq!(types, all, "every response type has a line");
    assert_eq!(count, 23);
}

#[test]
fn v2_error_codes_read_and_written_byte_for_byte() {
    let mut codes = Vec::new();
    for l in lines().into_iter().filter(|l| l.kind == "error") {
        let response = protocol::decode_response(l.role, l.frame)
            .unwrap_or_else(|_| panic!("v2 error refused: {}", String::from_utf8_lossy(l.frame)));
        let again = protocol::encode_response(l.role, &response).expect("error not written");
        assert_eq!(
            &again[..],
            l.frame,
            "error written differently from the fixture"
        );
        match response {
            Response::Error(e) => codes.push(e),
            _ => panic!("error line is not an error"),
        }
    }
    assert_eq!(codes, ResponseError::SIMPLE);
}

/// m2:V4.15 (S1, S9): the request of each line comes from a role that may ask for it,
/// and the operations that carry or change a secret are not the agent's.
#[test]
fn v2_fixture_roles_follow_the_table() {
    for l in lines().into_iter().filter(|l| l.kind == "request") {
        let request = protocol::decode_request(l.role, l.frame).unwrap();
        assert!(request.operation().allowed(l.role));
    }
    for name in [
        "reveal",
        "copy",
        "detail",
        "totp",
        "fill",
        "fill_login",
        "suggest",
        "set_level",
        "answer_approval",
        "issue_token",
        "save_entry",
        "remove_entry",
        "restore_backup",
        "backups",
        "change_pin",
        "change_password",
        "config",
        "external_backup",
        "set_external_backup",
        "usb_drives",
        "create_vault",
    ] {
        let op = Operation::from_name(name).expect("v2 operation");
        assert!(!op.allowed(Role::Agent), "{name} from the agent");
    }
}

// ---- Attacks (m2:RM1, m2:V4.14) ----

/// RM1: a renamed name (operation or field) is not silently accepted by the v2 reader,
/// and the v1 name of the field is not read as the v2 one.
#[test]
fn attack_renamed_wire_name_is_refused() {
    let mut fields = 0;
    for l in lines().into_iter().filter(|l| l.kind == "request") {
        let frame = std::str::from_utf8(l.frame).unwrap();
        let (head, tail) = frame.split_once(r#""op":""#).unwrap();
        let renamed = format!(r#"{head}"op":"x_{tail}"#);
        assert_eq!(
            protocol::decode_request(l.role, renamed.as_bytes()).unwrap_err(),
            ResponseError::UnknownOperation
        );
        if frame.contains(r#""entry":"#) {
            let renamed = frame.replacen(r#""entry":"#, r#""entrada":"#, 1);
            assert_eq!(
                protocol::decode_request(l.role, renamed.as_bytes()).unwrap_err(),
                ResponseError::InvalidRequest
            );
            fields += 1;
        }
        let renamed = frame.replacen(r#""data":"#, r#""dados":"#, 1);
        assert_eq!(
            protocol::decode_request(l.role, renamed.as_bytes()).unwrap_err(),
            ResponseError::InvalidRequest
        );
    }
    assert!(fields >= 10);
}

/// m2:DT8: the same frames as version 1 answer the version error, never the request.
#[test]
fn attack_v2_frame_labelled_1_is_unknown_version() {
    for l in lines().into_iter().filter(|l| l.kind == "request") {
        let frame = std::str::from_utf8(l.frame).unwrap();
        let old = frame.replacen(r#"{"v":2,"#, r#"{"v":1,"#, 1);
        for role in Role::ALL {
            assert_eq!(
                protocol::decode_request(role, old.as_bytes()).unwrap_err(),
                ResponseError::UnknownVersion
            );
        }
    }
    for l in lines().into_iter().filter(|l| l.kind != "request") {
        let frame = std::str::from_utf8(l.frame).unwrap();
        let old = frame.replacen(r#"{"v":2,"#, r#"{"v":1,"#, 1);
        for role in Role::ALL {
            assert!(protocol::decode_response(role, old.as_bytes()).is_err());
        }
        assert_eq!(protocol::frame_version(old.as_bytes()), Some(1));
        assert_eq!(protocol::frame_version(l.frame), Some(2));
    }
}
