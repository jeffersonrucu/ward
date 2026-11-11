//! Protocol tests (Phase 20): operation × role table (V3.15), no credential for the
//! agent (V3.3, S1), refused frames (V3.4), strict JSON, D29 attachments from the app,
//! D39 errors without generation, and fuzzing of the decoder. Only fake values (D15).

use std::io::{self, Read};

use base64ct::{Base64, Encoding};

use super::frame::{self, FrameError, frame_limit, read_frame};
use super::*;
use crate as wc;
use crate::atomic_write::WriteError;
use crate::body::OpenError;
use crate::entry::{AttachmentDraft, DataDraft, Draft, EntryType, Environment, ExtraDraft, Secret};
use crate::limits::{
    KIB, MAX_APP_FRAME, MAX_ATTACHMENT_NAME, MAX_ATTACHMENTS, MAX_ATTACHMENTS_TOTAL,
    MAX_COMMON_FRAME, MAX_EXTRA_VALUE, MAX_EXTRAS, MAX_NOTE, MIB,
};

// The same test vault as the body tests (their module is private to `body`).
#[allow(clippy::duplicate_mod)]
#[path = "../body/test_vault.rs"]
mod test_vault;
use test_vault::test_vault_entries;

// ---- Helpers ----

fn b64(b: &[u8]) -> String {
    Base64::encode_string(b)
}

fn req(role: Role, json: &str) -> Result<Request, ResponseError> {
    decode_request(role, json.as_bytes())
}

fn mk_secret(s: &str) -> Secret {
    Secret::from_text(s.to_owned())
}

/// The minimal valid data of each operation (fake values).
fn valid_data(op: Operation) -> String {
    let pin = b64(b"135790");
    let password = b64(b"falso-mestra-ward-01");
    match op {
        Operation::Status
        | Operation::Lock
        | Operation::List
        | Operation::OpenApp
        | Operation::Backups
        | Operation::Config
        | Operation::ExternalBackup
        | Operation::AwaitLogin
        | Operation::UsbDrives => "{}".into(),
        Operation::FillLogin => {
            r#"{"id":"AbCdEfGhIjKlMnOpQrStUvWxYz012345","origin":"http://localhost:8081"}"#.into()
        }
        Operation::LoginResult => {
            r#"{"id":"AbCdEfGhIjKlMnOpQrStUvWxYz012345","result":"submitted"}"#.into()
        }
        Operation::SetExternalBackup => r#"{"enabled":true,"uuid":"ABCD-1234"}"#.into(),
        Operation::RestoreBackup => {
            format!(
                r#"{{"version":"ward-20261109-153000.ward","password":"{password}","pin":"{pin}"}}"#
            )
        }
        Operation::Unlock => format!(r#"{{"password":"{password}"}}"#),
        Operation::Run => format!(
            r#"{{"entry":"acme-pg-dev","argv":["{}"],"cwd":"{}","stdin_kind":"null","environment":[{{"name":"{}","value":"{}"}}]}}"#,
            b64(b"psql"),
            b64(b"/tmp"),
            b64(b"LANG"),
            b64(b"C.UTF-8")
        ),
        Operation::Ssh => r#"{"entry":"acme-ssh-dev","stdin_kind":"terminal"}"#.into(),
        Operation::Login | Operation::Playwright | Operation::Detail | Operation::Totp => {
            r#"{"entry":"acme-wp-dev"}"#.into()
        }
        Operation::RemoveEntry | Operation::IssueToken => r#"{"entry":"acme-ssh-dev"}"#.into(),
        Operation::Stdin => format!(r#"{{"data":"{}"}}"#, b64(b"ls\n")),
        Operation::Resize => r#"{"rows":24,"cols":80}"#.into(),
        Operation::CreateVault => format!(r#"{{"password":"{password}","pin":"{pin}"}}"#),
        Operation::Reveal | Operation::Copy => {
            r#"{"entry":"acme-wp-dev","field":"password"}"#.into()
        }
        Operation::SaveEntry | Operation::SaveLogin => format!(
            r#"{{"entry":{{"name":"clientex-stage-wp","group":"clientex","environment":"stage","type":"login","fields":{{"url":"https://clientex.invalid/login","username":"admin","password":"{}"}}}}}}"#,
            b64(b"falso-clientex-01")
        ),
        Operation::SetLevel => {
            format!(r#"{{"entry":"acme-pg-dev","level":"blocked","pin":"{pin}"}}"#)
        }
        Operation::AnswerApproval => {
            format!(r#"{{"request":7,"decision":"once","pin":"{pin}"}}"#)
        }
        Operation::ChangePin => format!(r#"{{"current":"{pin}","new":"{}"}}"#, b64(b"2468")),
        Operation::ChangePassword => {
            format!(
                r#"{{"current":"{password}","new":"{}"}}"#,
                b64(b"falso-nova-02")
            )
        }
        Operation::LockTimeout => r#"{"minutes":240}"#.into(),
        Operation::Suggest => r#"{"origin":"http://localhost:8081"}"#.into(),
        Operation::Fill => r#"{"entry":"acme-wp-dev","origin":"http://localhost:8081"}"#.into(),
    }
}

/// Puts `field` (`"k":v`) at the start of the `data` object.
fn with_field(data: &str, field: &str) -> String {
    if data == "{}" {
        format!("{{{field}}}")
    } else {
        data.replacen('{', &format!("{{{field},"), 1)
    }
}

fn frame_of(op: Operation, data: &str) -> String {
    format!(r#"{{"v":2,"op":"{}","data":{data}}}"#, op.name())
}

/// A role that may ask for the operation.
fn role_of(op: Operation) -> Role {
    Role::ALL
        .into_iter()
        .find(|p| op.allowed(*p))
        .expect("operação sem papel")
}

/// One request of each operation, built in memory.
fn one_request_of_each() -> Vec<Request> {
    Operation::ALL
        .iter()
        .map(|&op| req(role_of(op), &frame_of(op, &valid_data(op))).unwrap())
        .collect()
}

fn item(name: &str) -> ListItem {
    ListItem {
        name: name.into(),
        kind: EntryType::Login,
        group: "clientex".into(),
        environment: Environment::Stage,
        project: Some("portal \"novo\" \\ ação".into()),
        tags: vec!["web".into()],
        level: None,
        address: Some("https://clientex.invalid/login".into()),
        summary: None,
    }
}

/// One response of each type, with fake credentials where the type carries them.
fn one_response_of_each() -> Vec<Response> {
    let entries = test_vault_entries();
    let server = entries
        .iter()
        .find(|e| e.kind() == EntryType::Server)
        .unwrap();
    vec![
        Response::Ok,
        Response::Status {
            locked: true,
            vault_exists: true,
            role: Role::Agent,
        },
        Response::Error(ResponseError::Rollback),
        Response::Error(ResponseError::Validation(vec![Problem {
            field: "senha".into(),
            position: Some(2),
            reason: "caractere não permitido".into(),
        }])),
        Response::List(vec![item("clientex-stage-wp"), item("clientex-stage-api")]),
        Response::Output {
            stream: Stream::Stderr,
            data: zeroize::Zeroizing::new(b"\x00\xffsaida \xe2\x80\xa2\xe2\x80\xa2".to_vec()),
        },
        Response::Exit { code: 130 },
        Response::PlaywrightSession {
            path: "/run/user/1000/ward-teste/playwright/acme-wp-dev.json".into(),
        },
        Response::Detail(EntryDetail::from_entry(server)),
        Response::Secret(mk_secret("falso-revelar-01")),
        Response::Token(mk_secret("falso-token-conectar-01")),
        Response::ApprovalRequest(ApprovalRequest {
            id: 42,
            entries: vec!["acme-pg-prod".into(), "acme-ssh-prod".into()],
            environment: Environment::Prod,
            operation: Operation::Run,
            command: "psql -c 'select 1'".into(),
            directory: "\"/home/usuario/projetos/acme\"".into(),
            chain: vec!["claude".into(), "bash".into(), "ward run".into()],
            origin: String::new(),
            confirmation: false,
            hidden_entry: true,
        }),
        Response::ApprovalRequest(ApprovalRequest {
            id: 43,
            entries: vec!["acme-wp-prod".into()],
            environment: Environment::Prod,
            operation: Operation::Playwright,
            command: String::new(),
            directory: String::new(),
            chain: vec!["claude".into(), "ward playwright".into()],
            origin: "http://localhost:8081".into(),
            confirmation: true,
            hidden_entry: false,
        }),
        Response::ApprovalRequest(ApprovalRequest {
            id: 44,
            entries: vec!["acme-wp-dev".into()],
            environment: Environment::Dev,
            operation: Operation::Fill,
            command: String::new(),
            directory: String::new(),
            chain: vec!["chrome".into(), "ward-nm".into()],
            origin: "http://localhost:8081".into(),
            confirmation: true,
            hidden_entry: false,
        }),
        Response::UnlockRequest(UnlockRequest {
            entry: "acme-pg-aprovacao".into(),
            operation: Operation::Run,
            command: r#""psql" "-c" "select 1""#.into(),
            remaining: 102,
        }),
        Response::ShowWindow,
        Response::TotpCode {
            code: zeroize::Zeroizing::new("123456".into()),
            remaining: 17,
        },
        Response::Suggestions(vec![Suggestion {
            name: "acme-wp-dev".into(),
            username: "wp_admin".into(),
            environment: crate::entry::Environment::Dev,
            totp: true,
        }]),
        Response::Fill {
            username: "wp_admin".into(),
            password: mk_secret("falso-WP-senha-01"),
            totp: Some(zeroize::Zeroizing::new("654321".into())),
        },
        Response::LoginRequest {
            id: "AbCdEfGhIjKlMnOpQrStUvWxYz012345".into(),
            entry: "acme-wp-prod".into(),
        },
        Response::Backups(vec![
            vb("ward-20261109-153000-1.ward", true, true),
            vb("ward-20261109-153000.ward", false, false),
        ]),
        Response::Config {
            lock_minutes: 240,
            external_backup: false,
            local_backup: true,
        },
        Response::ExternalBackup {
            enabled: true,
            uuid: Some("ABCD-1234".into()),
        },
        Response::UsbDrives(vec![UsbDrive {
            uuid: "ABCD-1234".into(),
            label: "PENDRIVE \"A\"".into(),
            mounted: true,
        }]),
    ]
}

/// The role that may receive the type.
fn role_of_response(t: ResponseType) -> Role {
    Role::ALL
        .into_iter()
        .find(|p| t.allowed(*p))
        .expect("tipo sem papel")
}

// ---- V3.15: the table ----

/// The whole table, as the auditor reads it: (operation, agent, app, extension). A new
/// operation, or a new role in an operation, has to change this test.
#[test]
fn v3_15_operation_role_table() {
    let expected: &[(&str, bool, bool, bool)] = &[
        ("status", true, true, true),
        ("unlock", true, true, false),
        ("lock", true, true, false),
        ("list", true, true, false),
        ("run", true, false, false),
        ("ssh", true, false, false),
        ("login", true, false, false),
        ("playwright", true, false, false),
        ("stdin", true, false, false),
        ("resize", true, false, false),
        ("create_vault", false, true, false),
        ("detail", false, true, false),
        ("reveal", false, true, false),
        ("copy", false, true, false),
        ("totp", false, true, false),
        ("save_entry", false, true, false),
        ("remove_entry", false, true, false),
        ("set_level", false, true, false),
        ("answer_approval", false, true, false),
        ("issue_token", false, true, false),
        ("change_pin", false, true, false),
        ("change_password", false, true, false),
        ("lock_timeout", false, true, false),
        ("backups", false, true, false),
        ("restore_backup", false, true, false),
        ("config", false, true, false),
        ("external_backup", false, true, false),
        ("set_external_backup", false, true, false),
        ("usb_drives", false, true, false),
        ("suggest", false, false, true),
        ("fill", false, false, true),
        ("save_login", false, false, true),
        ("await_login", false, false, true),
        ("fill_login", false, false, true),
        ("login_result", false, false, true),
        ("open_app", true, true, true),
    ];
    let real: Vec<_> = Operation::ALL
        .iter()
        .map(|o| {
            let p = o.roles();
            (o.name(), p.agent, p.app, p.extension)
        })
        .collect();
    assert_eq!(real, expected);
    // No operation to approve, answer an approval or change a level from the agent.
    for o in Operation::ALL {
        let n = o.name();
        if n.contains("approv") || n.contains("level") || n.contains("token") {
            assert!(!o.allowed(Role::Agent), "{n} aceita do agente");
        }
    }

    let responses: &[(&str, bool, bool, bool, bool)] = &[
        // (type, agent, app, extension, carries a credential)
        ("ok", true, true, true, false),
        ("status", true, true, true, false),
        ("error", true, true, true, false),
        ("list", true, true, false, false),
        ("output", true, false, false, false),
        ("exit", true, false, false, false),
        ("playwright_session", true, false, false, false),
        ("detail", false, true, false, false),
        ("secret", false, true, false, true),
        ("token", false, true, false, true),
        ("approval_request", false, true, false, false),
        ("unlock_request", false, true, false, false),
        ("show_window", false, true, false, false),
        ("totp_code", false, true, true, true),
        ("suggestions", false, false, true, false),
        ("fill", false, false, true, true),
        ("login_request", false, false, true, false),
        ("backups", false, true, false, false),
        ("config", false, true, false, false),
        ("external_backup", false, true, false, false),
        ("usb_drives", false, true, false, false),
    ];
    let real: Vec<_> = ResponseType::ALL
        .iter()
        .map(|t| {
            let p = t.roles();
            (
                t.name(),
                p.agent,
                p.app,
                p.extension,
                t.carries_credential(),
            )
        })
        .collect();
    assert_eq!(real, responses);
    for t in ResponseType::ALL {
        assert!(
            !(t.carries_credential() && t.allowed(Role::Agent)),
            "{} com credencial para o agente",
            t.name()
        );
    }
}

/// V3.15 (D8): the protocol has no feature, environment variable or argument that
/// changes the table or turns on an approver on the socket.
#[test]
fn v3_15_no_backdoor_in_code() {
    let sources = [
        include_str!("../protocol.rs"),
        include_str!("json.rs"),
        include_str!("frame.rs"),
        include_str!("fields.rs"),
        include_str!("request.rs"),
        include_str!("response.rs"),
        include_str!("errors.rs"),
    ];
    for f in sources {
        for forbidden in [
            "cfg(feature",
            "cfg(debug_assertions",
            "std::env",
            "env::var",
            "env!(",
            "option_env!",
            "test-mode",
            "test_mode",
            "modo-teste",
            "modo_teste",
        ] {
            assert!(!f.contains(forbidden), "achou {forbidden:?} no protocolo");
        }
    }
}

// ---- V3.3: no credential for the agent ----

/// Every app and extension operation asked by the agent answers "unauthorized", with
/// the right data, without data, with invented fields at the top and in the data, and
/// with malformed data.
#[test]
fn v3_3_app_and_extension_operations_from_agent_are_unauthorized() {
    let mut tested = 0;
    for &op in Operation::ALL {
        for role in Role::ALL {
            if op.allowed(role) {
                continue;
            }
            let valid = valid_data(op);
            let with_extra = with_field(&valid, r#""revelar":true,"field":"password""#);
            let frames = [
                frame_of(op, &valid),
                format!(r#"{{"v":2,"op":"{}"}}"#, op.name()),
                frame_of(op, &with_extra),
                format!(
                    r#"{{"v":2,"op":"{}","revelar":true,"field":"password","data":{valid}}}"#,
                    op.name()
                ),
                frame_of(op, "5"),
                frame_of(op, r#"{"geracao":1}"#),
            ];
            for q in &frames {
                assert_eq!(
                    req(role, q).unwrap_err(),
                    ResponseError::Unauthorized,
                    "{} pelo {}: {q}",
                    op.name(),
                    role.name()
                );
                tested += 1;
            }
        }
    }
    assert!(tested >= 6 * 16);
}

/// The agent operations with invented fields are refused whole (strict reading): a
/// reveal flag or a password field turns into nothing.
#[test]
fn v3_3_invented_fields_refuse_the_request() {
    for &op in Operation::ALL {
        let role = role_of(op);
        let valid = valid_data(op);
        assert!(req(role, &frame_of(op, &valid)).is_ok(), "{}", op.name());
        for extra in [
            r#""revelar":true"#,
            r#""reveal":true"#,
            r#""field":"password""#,
            r#""value":"x""#,
            r#""level":"free""#,
            r#""role":"app""#,
        ] {
            // Reveal and copy have a real field key: with it repeated, the duplicated
            // key refuses too.
            let d = with_field(&valid, extra);
            assert_eq!(
                req(role, &frame_of(op, &d)).unwrap_err(),
                ResponseError::InvalidRequest,
                "{} com {extra}",
                op.name()
            );
        }
        let top = format!(
            r#"{{"v":2,"op":"{}","revelar":true,"data":{valid}}}"#,
            op.name()
        );
        assert_eq!(req(role, &top).unwrap_err(), ResponseError::InvalidRequest);
    }
}

/// D39: no operation accepts a generation coming from the client, neither at the top
/// nor in the data, and no response carries the generation.
#[test]
fn d39_generation_never_comes_from_nor_goes_to_client() {
    for &op in Operation::ALL {
        let role = role_of(op);
        let valid = valid_data(op);
        for generation in [r#""geracao":1"#, r#""generation":1"#] {
            let in_data = with_field(&valid, generation);
            assert_eq!(
                req(role, &frame_of(op, &in_data)).unwrap_err(),
                ResponseError::InvalidRequest
            );
        }
        let at_top = format!(
            r#"{{"v":2,"op":"{}","geracao":99,"data":{valid}}}"#,
            op.name()
        );
        assert_eq!(
            req(role, &at_top).unwrap_err(),
            ResponseError::InvalidRequest
        );
    }
    let draft =
        valid_data(Operation::SaveEntry).replacen(r#""name":"#, r#""geracao":2,"name":"#, 1);
    assert_eq!(
        req(Role::App, &frame_of(Operation::SaveEntry, &draft)).unwrap_err(),
        ResponseError::InvalidRequest
    );
    for r in one_response_of_each() {
        let role = role_of_response(r.kind());
        let q = encode_response(role, &r).unwrap();
        let s = String::from_utf8_lossy(&q);
        assert!(
            !s.contains("\"geracao\"") && !s.contains("\"generation\"") && !s.contains("geração"),
            "{s}"
        );
    }
}

/// S1: every response that carries a credential is refused for the agent, on writing
/// (daemon) and on reading (CLI), and the app's frame does not pass the agent's reading.
#[test]
fn s1_response_with_credential_never_goes_to_agent() {
    let mut seen = 0;
    for r in one_response_of_each() {
        let t = r.kind();
        for role in Role::ALL {
            // The list of the example is the agent's (without level); the app's, with level, is in
            // `d50_agent_list_without_level_and_app_list_with_level`.
            if t == ResponseType::List && role == Role::App {
                continue;
            }
            // The status carries the role of the receiver (D63).
            let own;
            let r = match &r {
                Response::Status {
                    locked,
                    vault_exists,
                    ..
                } => {
                    own = Response::Status {
                        locked: *locked,
                        vault_exists: *vault_exists,
                        role,
                    };
                    &own
                }
                other => other,
            };
            let enc = encode_response(role, r);
            if t.allowed(role) {
                let q = enc.unwrap();
                assert!(decode_response(role, &q).is_ok());
                for other in Role::ALL {
                    if !t.allowed(other) {
                        assert_eq!(decode_response(other, &q).unwrap_err(), InvalidResponse);
                    }
                }
            } else {
                assert_eq!(enc.unwrap_err(), EncodeError::NotAllowedForRole);
            }
        }
        if t.carries_credential() {
            assert_eq!(
                encode_response(Role::Agent, &r).unwrap_err(),
                EncodeError::NotAllowedForRole
            );
            seen += 1;
        }
    }
    assert_eq!(seen, 4); // secret, token, TOTP code, fill
}

/// V3.38 (D63): the status carries the role of the connection and nothing else new; the
/// daemon cannot send a role the status of another, and the client reads the role that
/// came.
#[test]
fn v3_38_status_carries_the_role_and_only_it() {
    for role in Role::ALL {
        let r = Response::Status {
            locked: false,
            vault_exists: true,
            role,
        };
        let q = encode_response(role, &r).unwrap();
        assert_eq!(
            String::from_utf8(q.to_vec()).unwrap(),
            format!(
                r#"{{"v":2,"type":"status","data":{{"locked":false,"vault_exists":true,"role":"{}"}}}}"#,
                role.name()
            )
        );
        match decode_response(role, &q).unwrap() {
            Response::Status { role: got, .. } => assert_eq!(got, role),
            other => panic!("{other:?}"),
        }
        for other in Role::ALL {
            if other != role {
                assert_eq!(
                    encode_response(other, &r).unwrap_err(),
                    EncodeError::Invalid
                );
                // The app that gets the agent role reads it as it came (and quits, Phase 23a).
                let q = encode_response(
                    other,
                    &Response::Status {
                        locked: false,
                        vault_exists: true,
                        role: other,
                    },
                )
                .unwrap();
                assert!(matches!(
                    decode_response(role, &q),
                    Ok(Response::Status { role: p, .. }) if p == other
                ));
            }
        }
    }
    // Without the role, with another name or with an extra field, the client refuses.
    for data in [
        r#"{"locked":false,"vault_exists":true}"#,
        r#"{"locked":false,"vault_exists":true,"role":"root"}"#,
        r#"{"locked":false,"vault_exists":true,"role":"app","uid":0}"#,
    ] {
        let q = format!(r#"{{"v":2,"type":"status","data":{data}}}"#);
        assert!(decode_response(Role::App, q.as_bytes()).is_err(), "{data}");
    }
}

/// The values of a secret as they may show up in a frame: raw, in base64 and with the
/// JSON escaping.
fn forms(secret: &[u8]) -> Vec<Vec<u8>> {
    let mut v = vec![secret.to_vec(), b64(secret).into_bytes()];
    if let Ok(s) = core::str::from_utf8(secret) {
        let esc = s
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n");
        v.push(esc.into_bytes());
        // Each line of 6+ characters (private key).
        for l in s.lines().filter(|l| l.len() >= 6) {
            v.push(l.as_bytes().to_vec());
        }
    }
    v
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.len() <= haystack.len() && haystack.windows(needle.len()).any(|w| w == needle)
}

/// V3.3: the list of all the entries of the test vault (the 22, including the blocked
/// ones, which the daemon would still filter) for the agent and the detail of each one
/// for the app have no secret at all, in any form.
#[test]
fn v3_3_test_vault_list_and_detail_without_secret() {
    let entries = test_vault_entries();
    assert_eq!(entries.len(), 22);
    let list = Response::List(
        entries
            .iter()
            .map(|e| ListItem::from_entry(e, Role::Agent))
            .collect(),
    );
    let q_list = encode_response(Role::Agent, &list).unwrap();
    let details: Vec<_> = entries
        .iter()
        .map(|e| encode_response(Role::App, &Response::Detail(EntryDetail::from_entry(e))).unwrap())
        .collect();
    let mut needles = 0;
    for e in &entries {
        for s in e.secrets() {
            for f in forms(s.value.expose()) {
                if f.len() < 6 {
                    continue;
                }
                needles += 1;
                assert!(!contains(&q_list, &f), "segredo de {} na lista", e.name());
                for d in &details {
                    assert!(!contains(d, &f), "segredo de {} num detalhe", e.name());
                }
            }
        }
    }
    assert!(needles > 40);
    // And the list comes back whole through the CLI's reading.
    let Response::List(items) = decode_response(Role::Agent, &q_list).unwrap() else {
        panic!("não é lista")
    };
    assert_eq!(items.len(), 22);
    assert!(items.iter().any(|i| i.name == "gama-bloqueada"));
}

/// The `Debug` of requests and responses shows no secret.
#[test]
fn debug_without_secret() {
    let password = "falso-mestra-debug-01";
    let p = req(
        Role::Agent,
        &frame_of(
            Operation::Unlock,
            &format!(r#"{{"password":"{}"}}"#, b64(password.as_bytes())),
        ),
    )
    .unwrap();
    let d = format!("{p:?}");
    assert!(
        !d.contains(password) && !d.contains(&b64(password.as_bytes())),
        "{d}"
    );
    for r in one_response_of_each() {
        let d = format!("{r:?}");
        for s in [
            "falso-revelar-01",
            "falso-token-conectar-01",
            "falso-WP-senha-01",
        ] {
            assert!(!d.contains(s), "{d}");
        }
    }
}

// ---- Round trip ----

#[test]
fn round_trip_of_every_request_and_response() {
    for p in one_request_of_each() {
        let role = role_of(p.operation());
        let q = encode_request(role, &p).unwrap();
        let again = decode_request(role, &q).unwrap();
        assert_eq!(*encode_request(role, &again).unwrap(), *q);
        // The client does not encode a request of another role.
        for other in Role::ALL {
            if !p.operation().allowed(other) {
                assert_eq!(
                    encode_request(other, &p).unwrap_err(),
                    EncodeError::NotAllowedForRole
                );
            }
        }
    }
    for r in one_response_of_each() {
        let role = role_of_response(r.kind());
        let q = encode_response(role, &r).unwrap();
        let again = decode_response(role, &q).unwrap();
        assert_eq!(*encode_response(role, &again).unwrap(), *q);
    }
}

#[test]
fn known_frame_format() {
    let q = encode_request(Role::Agent, &Request::List).unwrap();
    assert_eq!(&q[..], br#"{"v":2,"op":"list","data":{}}"#);
    let q = encode_response(Role::Agent, &Response::Error(ResponseError::Unauthorized)).unwrap();
    assert_eq!(
        &q[..],
        r#"{"v":2,"type":"error","data":{"code":"unauthorized","message":"não autorizado"}}"#
            .as_bytes()
    );
    let mut out = Vec::new();
    frame::write_frame(&mut out, &q, MAX_COMMON_FRAME).unwrap();
    assert_eq!(&out[..4], &(q.len() as u32).to_be_bytes());
    assert_eq!(&out[4..], &q[..]);
    // Missing data counts as empty.
    assert!(matches!(
        req(Role::Agent, r#"{"op":"list","v":2}"#),
        Ok(Request::List)
    ));
}

// ---- Envelope errors ----

#[test]
fn envelope_refused() {
    let cases: &[(&str, ResponseError)] = &[
        ("", ResponseError::InvalidRequest),
        ("nada", ResponseError::InvalidRequest),
        ("[]", ResponseError::InvalidRequest),
        (r#"{"op":"list"}"#, ResponseError::InvalidRequest),
        (r#"{"v":"1","op":"list"}"#, ResponseError::InvalidRequest),
        // m2:DT8: version 1 (0.1.x) and any other one answer the version error.
        (r#"{"v":1,"op":"list"}"#, ResponseError::UnknownVersion),
        (r#"{"v":1,"op":"lista"}"#, ResponseError::UnknownVersion),
        (r#"{"v":3,"op":"list"}"#, ResponseError::UnknownVersion),
        (r#"{"v":0,"op":"list"}"#, ResponseError::UnknownVersion),
        (r#"{"v":2}"#, ResponseError::InvalidRequest),
        (
            r#"{"v":2,"op":"apagar_tudo"}"#,
            ResponseError::UnknownOperation,
        ),
        (r#"{"v":2,"op":"Lista"}"#, ResponseError::UnknownOperation),
        (
            r#"{"v":2,"op":"list","data":[]}"#,
            ResponseError::InvalidRequest,
        ),
        (
            r#"{"v":2,"op":"list","op":"list"}"#,
            ResponseError::InvalidRequest,
        ),
        (
            r#"{"v":2,"op":"list","op":"reveal"}"#,
            ResponseError::InvalidRequest,
        ),
        (
            r#"{"v":2,"v":2,"op":"list"}"#,
            ResponseError::InvalidRequest,
        ),
        (
            r#"{"v":2,"op":"list","data":{},"data":{}}"#,
            ResponseError::InvalidRequest,
        ),
        (r#"{"v":2,"op":"list"} {}"#, ResponseError::InvalidRequest),
        (
            r#"{"v":2,"op":"list","data":{"x":1}}"#,
            ResponseError::InvalidRequest,
        ),
    ];
    for (q, e) in cases {
        assert_eq!(req(Role::Agent, q).unwrap_err(), *e, "{q}");
    }
}

/// m2:Phase 66b (security-auditor of M6, low): documented decision, see the module
/// doc. A missing `data` and escapes in the names give exactly the canonical request
/// (same re-encoded bytes), so no spelling reaches a different operation, skips the
/// role check or carries an extra field; any other spelling is unknown.
#[test]
fn decision_missing_data_and_escaped_names_are_the_canonical_request() {
    let canonical = |role: Role, q: &str| -> Vec<u8> {
        let r = req(role, q).unwrap_or_else(|e| panic!("{q}: {e:?}"));
        encode_request(role, &r).unwrap().to_vec()
    };
    let list = canonical(Role::Agent, r#"{"v":2,"op":"list","data":{}}"#);
    assert_eq!(list, br#"{"v":2,"op":"list","data":{}}"#);
    for q in [
        r#"{"v":2,"op":"list"}"#,
        r#"{"v":2,"op":"list"}"#,
        r#"{"v":2,"op":"list","data":{}}"#,
        r#"{"v":2,"op":"list","data":{}}"#,
    ] {
        assert_eq!(canonical(Role::Agent, q), list, "{q}");
    }
    let status = canonical(Role::Extension, r#"{"v":2,"op":"status"}"#);
    assert_eq!(status, br#"{"v":2,"op":"status","data":{}}"#);
    // The escape does not dodge the role: an app operation spelled with escapes is
    // still the app's.
    for role in [Role::Agent, Role::Extension] {
        for q in [
            r#"{"v":2,"op":"reveal"}"#,
            r#"{"v":2,"op":"copy","data":{}}"#,
            r#"{"v":2,"op":"set_level"}"#,
        ] {
            assert_eq!(
                req(role, q).unwrap_err(),
                ResponseError::Unauthorized,
                "{q}"
            );
        }
    }
    // Only the exact name after decoding: another spelling is unknown.
    for q in [
        r#"{"v":2,"op":"list "}"#,
        r#"{"v":2,"op":"List"}"#,
        r#"{"v":2,"op":"liſt"}"#,
        r#"{"v":2,"op":"list\u0000"}"#,
    ] {
        let e = req(Role::Agent, q).unwrap_err();
        assert!(
            matches!(
                e,
                ResponseError::UnknownOperation | ResponseError::InvalidRequest
            ),
            "{q}: {e:?}"
        );
    }
    // A missing `data` is the empty object, not a pass: required fields still refuse.
    for (role, op) in [
        (Role::Agent, "run"),
        (Role::Agent, "unlock"),
        (Role::App, "reveal"),
        (Role::App, "answer_approval"),
        (Role::Extension, "fill"),
    ] {
        assert_eq!(
            req(role, &format!(r#"{{"v":2,"op":"{op}"}}"#)).unwrap_err(),
            ResponseError::InvalidRequest,
            "{op}"
        );
    }
    // An escaped key is the same key: a second `op` written with escapes is a repeated
    // key, and an escaped extra field is still an extra field.
    for q in [
        r#"{"v":2,"op":"list","op":"reveal"}"#,
        r#"{"v":2,"op":"list","data":{},"data":{}}"#,
        r#"{"v":2,"op":"list","data":{"reveal":true}}"#,
    ] {
        assert_eq!(
            req(Role::Agent, q).unwrap_err(),
            ResponseError::InvalidRequest,
            "{q}"
        );
    }
}

// ---- Format of each operation ----

#[test]
fn approval_pin_and_decision() {
    let q = |d: &str| req(Role::App, &frame_of(Operation::AnswerApproval, d));
    let pin = b64(b"135790");
    assert!(q(r#"{"request":1,"decision":"deny"}"#).is_ok());
    assert!(
        q(&format!(
            r#"{{"request":1,"decision":"deny","pin":"{pin}"}}"#
        ))
        .is_err()
    );
    for d in ["once", "one_hour", "always"] {
        assert!(
            q(&format!(
                r#"{{"request":1,"decision":"{d}","pin":"{pin}"}}"#
            ))
            .is_ok()
        );
        assert!(
            q(&format!(r#"{{"request":1,"decision":"{d}"}}"#)).is_err(),
            "{d} sem PIN"
        );
    }
    for bad in ["123", "1234567", "12a4", "١٢٣٤", "12 34", ""] {
        let d = format!(
            r#"{{"request":1,"decision":"once","pin":"{}"}}"#,
            b64(bad.as_bytes())
        );
        assert!(q(&d).is_err(), "PIN {bad:?}");
        let m = format!(
            r#"{{"entry":"acme-pg-dev","level":"free","pin":"{}"}}"#,
            b64(bad.as_bytes())
        );
        assert!(req(Role::App, &frame_of(Operation::SetLevel, &m)).is_err());
    }
    // Phase 44c: the confirm decision carries no PIN (checking that the request is a
    // confirmation is the daemon's job).
    assert!(q(r#"{"request":1,"decision":"confirm"}"#).is_ok());
    assert!(
        q(&format!(
            r#"{{"request":1,"decision":"confirm","pin":"{pin}"}}"#
        ))
        .is_err()
    );
    // A PIN as text, without base64, is refused.
    assert!(q(r#"{"request":1,"decision":"once","pin":"135790"}"#).is_err());
    assert!(q(r#"{"request":1,"decision":"admin","pin":"MTM1Nzkw"}"#).is_err());
    assert!(
        req(
            Role::App,
            &frame_of(
                Operation::SetLevel,
                &format!(r#"{{"entry":"acme-pg-dev","level":"admin","pin":"{pin}"}}"#)
            )
        )
        .is_err()
    );
}

#[test]
fn run_refuses_nul_equals_and_empty() {
    let q = |argv: &[&[u8]], cwd: &[u8], env: &[(&[u8], &[u8])]| {
        let argv: Vec<_> = argv.iter().map(|a| format!("\"{}\"", b64(a))).collect();
        let env: Vec<_> = env
            .iter()
            .map(|(n, v)| format!(r#"{{"name":"{}","value":"{}"}}"#, b64(n), b64(v)))
            .collect();
        req(
            Role::Agent,
            &frame_of(
                Operation::Run,
                &format!(
                    r#"{{"entry":"acme-pg-dev","argv":[{}],"cwd":"{}","stdin_kind":"pipe","environment":[{}]}}"#,
                    argv.join(","),
                    b64(cwd),
                    env.join(",")
                ),
            ),
        )
    };
    assert!(q(&[b"ls", b"\xff\xfe"], b"/tmp", &[(b"A", b"\xff")]).is_ok());
    assert!(q(&[], b"/tmp", &[]).is_err());
    assert!(q(&[b"l\0s"], b"/tmp", &[]).is_err());
    assert!(q(&[b"ls"], b"", &[]).is_err());
    assert!(q(&[b"ls"], b"/t\0mp", &[]).is_err());
    assert!(q(&[b"ls"], b"/tmp", &[(b"A=B", b"c")]).is_err());
    assert!(q(&[b"ls"], b"/tmp", &[(b"", b"c")]).is_err());
    assert!(q(&[b"ls"], b"/tmp", &[(b"A", b"c\0")]).is_err());
    let large = vec![b'a'; crate::limits::MAX_ARGUMENT + 1];
    assert!(q(&[&large], b"/tmp", &[]).is_err());
}

#[test]
fn reveal_requires_a_known_target() {
    let q = |d: &str| req(Role::App, &frame_of(Operation::Reveal, d));
    assert!(q(r#"{"entry":"acme-wp-dev","field":"password"}"#).is_ok());
    assert!(q(r#"{"entry":"acme-wp-dev","extra":"PIN_BANCO"}"#).is_ok());
    assert!(q(r#"{"entry":"acme-wp-dev"}"#).is_err());
    assert!(q(r#"{"entry":"acme-wp-dev","field":"password","extra":"X"}"#).is_err());
    assert!(q(r#"{"entry":"acme-wp-dev","field":"username"}"#).is_err());
    assert!(q(r#"{"entry":"","field":"password"}"#).is_err());
}

/// S9: the entry that comes from the app has no level, and a field of another type refuses.
#[test]
fn s9_draft_without_level_and_without_field_of_other_type() {
    let base = valid_data(Operation::SaveEntry);
    let q = |d: &str| req(Role::App, &frame_of(Operation::SaveEntry, d));
    assert!(q(&base).is_ok());
    let with_level = base.replacen(r#""name":"#, r#""level":"free","name":"#, 1);
    assert_eq!(q(&with_level).unwrap_err(), ResponseError::InvalidRequest);
    let level_in_fields = base.replacen(r#""url":"#, r#""level":"free","url":"#, 1);
    assert_eq!(
        q(&level_in_fields).unwrap_err(),
        ResponseError::InvalidRequest
    );
    let other_type = base.replacen(r#""url":"#, r#""host":"x","url":"#, 1);
    assert_eq!(q(&other_type).unwrap_err(), ResponseError::InvalidRequest);
    // A secret as text, without base64, refuses.
    let as_text = base.replace(&b64(b"falso-clientex-01"), "falso-clientex-01");
    assert_eq!(q(&as_text).unwrap_err(), ResponseError::InvalidRequest);
    // The extension only saves logins.
    let api = format!(
        r#"{{"entry":{{"name":"x-api","group":"x","environment":"dev","type":"api","fields":{{"token":"{}"}}}}}}"#,
        b64(b"falso-token-01")
    );
    assert!(req(Role::Extension, &frame_of(Operation::SaveLogin, &api)).is_err());
    assert!(req(Role::App, &frame_of(Operation::SaveEntry, &api)).is_ok());
}

// ---- D29: attachments from the app ----

fn draft_with_attachments(sizes: &[usize]) -> Draft {
    Draft {
        name: "clientex-stage-vpn".into(),
        group: "clientex".into(),
        environment: "stage".into(),
        project: String::new(),
        tags: Vec::new(),
        data: DataDraft::Generic,
        extras: vec![ExtraDraft {
            name: "usuario".into(),
            value: mk_secret("vpn-user"),
            secret: false,
        }],
        attachments: sizes
            .iter()
            .enumerate()
            .map(|(i, &n)| AttachmentDraft {
                name: format!("cliente-{i}.ovpn"),
                content: Secret::from_bytes(test_vault::bytes_0_to_255(n)),
            })
            .collect(),
    }
}

/// D29: the app sends attachments adding up to 256 KiB; one byte more refuses. The agent
/// does not have the operation.
#[test]
fn d29_attachments_up_to_256_kib_total_from_app() {
    let at_limit = Request::SaveEntry {
        previous: None,
        draft: draft_with_attachments(&[MAX_ATTACHMENTS_TOTAL - 10 * KIB, 10 * KIB]),
        keep_attachments: Vec::new(),
    };
    let q = encode_request(Role::App, &at_limit).unwrap();
    assert!(q.len() > MAX_ATTACHMENTS_TOTAL && q.len() < MAX_APP_FRAME);
    let Request::SaveEntry { draft, .. } = decode_request(Role::App, &q).unwrap() else {
        panic!()
    };
    assert_eq!(
        draft.attachments[0].content.expose(),
        &test_vault::bytes_0_to_255(MAX_ATTACHMENTS_TOTAL - 10 * KIB)[..]
    );
    draft.validate().expect("entrada com anexos no limite");
    assert_eq!(
        decode_request(Role::Agent, &q).unwrap_err(),
        ResponseError::Unauthorized
    );

    // One byte above the total: the client does not encode and the daemon does not accept.
    let above = Request::SaveEntry {
        previous: Some("clientex-stage-vpn".into()),
        draft: draft_with_attachments(&[MAX_ATTACHMENTS_TOTAL - 10 * KIB, 10 * KIB + 1]),
        keep_attachments: Vec::new(),
    };
    assert_eq!(
        encode_request(Role::App, &above).unwrap_err(),
        EncodeError::Invalid
    );
    let forged = String::from_utf8(q.to_vec()).unwrap().replace(
        &b64(&test_vault::bytes_0_to_255(10 * KIB)),
        &b64(&test_vault::bytes_0_to_255(10 * KIB + 1)),
    );
    assert_eq!(
        req(Role::App, &forged).unwrap_err(),
        ResponseError::InvalidRequest
    );
}

// ---- Phase 39b: attachments kept on edit (V5.12) ----

/// The kept attachments go and come back; missing when empty (today's frame does not change).
#[test]
fn phase39b_keep_attachments_round_trip() {
    let p = Request::SaveEntry {
        previous: Some("clientex-stage-vpn".into()),
        draft: draft_with_attachments(&[10]),
        keep_attachments: vec!["cliente-0.ovpn".into(), "app.env".into()],
    };
    let q = encode_request(Role::App, &p).unwrap();
    let Request::SaveEntry {
        keep_attachments, ..
    } = decode_request(Role::App, &q).unwrap()
    else {
        panic!()
    };
    assert_eq!(keep_attachments, ["cliente-0.ovpn", "app.env"]);
    let without = Request::SaveEntry {
        previous: Some("clientex-stage-vpn".into()),
        draft: draft_with_attachments(&[10]),
        keep_attachments: Vec::new(),
    };
    let q = encode_request(Role::App, &without).unwrap();
    assert!(!String::from_utf8_lossy(&q).contains("keep_attachments"));
    assert_eq!(
        decode_request(Role::Agent, &q).unwrap_err(),
        ResponseError::Unauthorized
    );
}

/// Attacks on the kept attachments: in a new entry, empty name, repeated, too long, out
/// of a list of texts, and kept plus new above `MAX_ATTACHMENTS`.
#[test]
fn phase39b_attack_forged_keep_attachments() {
    let base = valid_data(Operation::SaveEntry);
    let q = |d: &str| req(Role::App, &frame_of(Operation::SaveEntry, d));
    let edit = |field: &str| {
        base.replacen(
            '{',
            &format!(r#"{{"previous":"clientex-stage-wp",{field},"#),
            1,
        )
    };
    assert!(q(&edit(r#""keep_attachments":["app.env"]"#)).is_ok());
    assert!(q(&edit(r#""keep_attachments":[]"#)).is_ok());
    // Without the previous name: there is nothing to keep from.
    let fresh = base.replacen('{', r#"{"keep_attachments":["app.env"],"#, 1);
    assert_eq!(q(&fresh).unwrap_err(), ResponseError::InvalidRequest);
    let long = format!(
        r#""keep_attachments":["{}"]"#,
        "a".repeat(MAX_ATTACHMENT_NAME + 1)
    );
    let many = format!(
        r#""keep_attachments":[{}]"#,
        (0..=MAX_ATTACHMENTS)
            .map(|i| format!(r#""a{i}.env""#))
            .collect::<Vec<_>>()
            .join(",")
    );
    for forged in [
        r#""keep_attachments":[""]"#.to_owned(),
        r#""keep_attachments":["app.env","app.env"]"#.to_owned(),
        r#""keep_attachments":"app.env""#.to_owned(),
        r#""keep_attachments":[{"name":"app.env","content":"QQ=="}]"#.to_owned(),
        r#""keep_attachments":[1]"#.to_owned(),
        long,
        many,
    ] {
        assert_eq!(
            q(&edit(&forged)).unwrap_err(),
            ResponseError::InvalidRequest,
            "{forged}"
        );
    }
    // Kept plus new above `MAX_ATTACHMENTS`: the client does not encode.
    let p = Request::SaveEntry {
        previous: Some("clientex-stage-vpn".into()),
        draft: draft_with_attachments(&[1; MAX_ATTACHMENTS]),
        keep_attachments: vec!["app.env".into()],
    };
    assert_eq!(
        encode_request(Role::App, &p).unwrap_err(),
        EncodeError::Invalid
    );
}

/// `merge_kept_attachments`: copies the content of the edited entry in front of the new
/// ones; a name the entry does not have (including with `/` or `..`) or a repeated one is
/// an invalid request and the draft does not change; the D29 total still holds through
/// the whole validation.
#[test]
fn phase39b_attack_merge_kept_attachments() {
    let old = draft_with_attachments(&[200 * KIB, 10]).validate().unwrap();
    let mut r = draft_with_attachments(&[5]);
    r.attachments[0].name = "novo.env".into();
    for names in [
        vec!["nao-existe.ovpn".to_owned()],
        vec!["cliente-0.ovpn".to_owned(), "cliente-0.ovpn".to_owned()],
        vec!["../cliente-0.ovpn".to_owned()],
        vec!["CLIENTE-0.OVPN".to_owned()],
    ] {
        assert_eq!(
            merge_kept_attachments(&mut r, &old, &names).unwrap_err(),
            ResponseError::InvalidRequest,
            "{names:?}"
        );
        assert_eq!(r.attachments.len(), 1);
    }
    merge_kept_attachments(&mut r, &old, &["cliente-1.ovpn".to_owned()]).unwrap();
    let names: Vec<&str> = r.attachments.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["cliente-1.ovpn", "novo.env"]);
    assert_eq!(
        r.attachments[0].content.expose(),
        old.attachments()[1].content().expose()
    );
    let e = r.validate().unwrap();
    assert_eq!(e.attachments().len(), 2);

    // Kept of 200 KiB + new of 60 KiB goes over the total: the validation refuses.
    let mut r = draft_with_attachments(&[60 * KIB]);
    r.attachments[0].name = "novo.env".into();
    merge_kept_attachments(&mut r, &old, &["cliente-0.ovpn".to_owned()]).unwrap();
    assert!(r.validate().is_err());

    // Kept name equal to the one of a new attachment: the validation flags the repetition.
    let mut r = draft_with_attachments(&[5]);
    merge_kept_attachments(&mut r, &old, &["cliente-0.ovpn".to_owned()]).unwrap();
    assert!(r.validate().is_err());
}

/// The largest entry the validation accepts (1 MiB note, 64 extras of 64 KiB, 256 KiB
/// of attachments) fits in the app's frame, goes, comes back and validates; the frame
/// goes over the limit of the common client.
#[test]
fn largest_entry_fits_in_app_frame() {
    let r = Draft {
        name: "clientex-prod-nota".into(),
        group: "clientex".into(),
        environment: "prod".into(),
        project: "portal".into(),
        tags: (0..16).map(|i| format!("tag-{i:02}")).collect(),
        data: DataDraft::SecureNote {
            text: Some(Secret::from_bytes(vec![b'"'; MAX_NOTE])),
        },
        extras: (0..MAX_EXTRAS)
            .map(|i| ExtraDraft {
                name: format!("EXTRA_{i:02}"),
                value: Secret::from_bytes(vec![b'x'; MAX_EXTRA_VALUE]),
                secret: i % 2 == 0,
            })
            .collect(),
        attachments: vec![AttachmentDraft {
            name: "tudo.bin".into(),
            content: Secret::from_bytes(test_vault::bytes_0_to_255(MAX_ATTACHMENTS_TOTAL)),
        }],
    };
    let p = Request::SaveEntry {
        previous: None,
        draft: r,
        keep_attachments: Vec::new(),
    };
    let q = encode_request(Role::App, &p).unwrap();
    assert!(q.len() > MAX_COMMON_FRAME, "{}", q.len());
    assert!(q.len() <= MAX_APP_FRAME, "{}", q.len());
    let Request::SaveEntry { draft, .. } = decode_request(Role::App, &q).unwrap() else {
        panic!()
    };
    let e = draft.validate().expect("maior entrada válida");
    assert_eq!(e.extras().len(), MAX_EXTRAS);
    // The same frame, through the frame reader: it fits in the app's and not in the common one.
    let mut raw = Vec::new();
    frame::write_frame(&mut raw, &q, MAX_APP_FRAME).unwrap();
    assert_eq!(
        read_frame(&mut &raw[..], frame_limit(Role::Agent)).unwrap_err(),
        FrameError::TooLarge
    );
    assert_eq!(
        read_frame(&mut &raw[..], frame_limit(Role::Extension)).unwrap_err(),
        FrameError::TooLarge
    );
    assert_eq!(
        *read_frame(&mut &raw[..], frame_limit(Role::App)).unwrap(),
        *q
    );
}

// ---- V3.4: frames ----

/// Reader that counts how many bytes of the body were asked for.
struct Counter<'a> {
    data: &'a [u8],
    consumed: usize,
}

impl Read for Counter<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = buf.len().min(self.data.len() - self.consumed);
        buf[..n].copy_from_slice(&self.data[self.consumed..self.consumed + n]);
        self.consumed += n;
        Ok(n)
    }
}

#[test]
fn v3_4_frames_refused_before_reading_the_body() {
    let mut body = Vec::new();
    for (header, error) in [
        ((2 * MIB) as u32, FrameError::TooLarge),
        ((MAX_COMMON_FRAME + 1) as u32, FrameError::TooLarge),
        (u32::MAX, FrameError::TooLarge), // the 32-bit -1
        ((-2i32) as u32, FrameError::TooLarge),
        (0, FrameError::Empty),
    ] {
        body.clear();
        body.extend_from_slice(&header.to_be_bytes());
        body.extend_from_slice(&[b'{'; 64]);
        let mut r = Counter {
            data: &body,
            consumed: 0,
        };
        assert_eq!(read_frame(&mut r, MAX_COMMON_FRAME).unwrap_err(), error);
        assert_eq!(r.consumed, 4, "leu o corpo de um quadro recusado");
    }
    // 2 MiB also goes over the frame of the common client for both roles.
    assert!(2 * MIB > frame_limit(Role::Agent));
    assert!(2 * MIB > frame_limit(Role::Extension));
    // At the exact limit it passes.
    let mut q = (MAX_COMMON_FRAME as u32).to_be_bytes().to_vec();
    q.extend(std::iter::repeat_n(b' ', MAX_COMMON_FRAME));
    assert_eq!(
        read_frame(&mut &q[..], MAX_COMMON_FRAME).unwrap().len(),
        MAX_COMMON_FRAME
    );
}

#[test]
fn v3_4_truncated_and_closed_frame() {
    assert_eq!(
        read_frame(&mut &b""[..], 100).unwrap_err(),
        FrameError::Closed
    );
    assert_eq!(
        read_frame(&mut &b"\0\0"[..], 100).unwrap_err(),
        FrameError::Truncated
    );
    assert_eq!(
        read_frame(&mut &b"\0\0\0\x0a{}"[..], 100).unwrap_err(),
        FrameError::Truncated
    );
    // Declared 1 MiB and sent 10 bytes: truncated, without waiting for the rest.
    let mut q = (MAX_COMMON_FRAME as u32).to_be_bytes().to_vec();
    q.extend_from_slice(b"0123456789");
    assert_eq!(
        read_frame(&mut &q[..], MAX_COMMON_FRAME).unwrap_err(),
        FrameError::Truncated
    );
    // Writing also refuses empty and above the limit, without writing anything.
    let mut out = Vec::new();
    assert_eq!(
        frame::write_frame(&mut out, b"", 10).unwrap_err(),
        FrameError::Empty
    );
    assert_eq!(
        frame::write_frame(&mut out, &[b' '; 11], 10).unwrap_err(),
        FrameError::TooLarge
    );
    assert!(out.is_empty());
}

/// V3.4: invalid JSON and unknown operation turn into an error response, without panic;
/// the response is a valid frame for the common client.
#[test]
fn v3_4_invalid_json_and_unknown_operation_answer_error() {
    for q in [
        &b"\xff\xfe{}"[..],
        b"{\"v\":2,\"op\":\"list\"",
        b"{\"v\":2,\"op\":\"\xc3\x28\"}",
        b"{\"v\":-1,\"op\":\"list\"}",
        b"{\"v\":2,\"op\":\"desconhecida\"}",
        b"\xef\xbb\xbf{\"v\":2,\"op\":\"list\"}",
    ] {
        let e = decode_request(Role::Agent, q).unwrap_err();
        let r = encode_response(Role::Agent, &Response::Error(e)).unwrap();
        assert!(decode_response(Role::Agent, &r).is_ok());
    }
    assert_eq!(
        frame_error(FrameError::TooLarge),
        ResponseError::InvalidFrame
    );
}

// ---- Strict JSON ----

#[test]
fn strict_json() {
    let accepted = [
        r#"{}"#,
        r#" { "a" : [ 1 , 0 , 18446744073709551615 ] , "b" : { } } "#,
        r#"{"a":"\" \\ \/ \b \f \n \r \t \u00e7 \ud83d\ude00"}"#,
        r#"{"a":true,"b":false,"c":null,"d":[]}"#,
        "{\"a\":\"ação\"}",
    ];
    for a in accepted {
        assert!(json::read(a.as_bytes(), json::MAX_NODES).is_ok(), "{a}");
    }
    let refused: &[&[u8]] = &[
        b"",
        b"[]",
        b"\"x\"",
        b"1",
        b"{",
        b"{}}",
        b"{} x",
        b"{\"a\":1,}",
        b"{\"a\" 1}",
        b"{a:1}",
        b"{'a':1}",
        b"{\"a\":01}",
        b"{\"a\":-1}",
        b"{\"a\":1.0}",
        b"{\"a\":1e3}",
        b"{\"a\":18446744073709551616}",
        b"{\"a\":+1}",
        b"{\"a\":\"\x01\"}",
        b"{\"a\":\"\n\"}",
        b"{\"a\":\"\\x\"}",
        b"{\"a\":\"\\u12\"}",
        b"{\"a\":\"\\ud800\"}",
        b"{\"a\":\"\\udc00\"}",
        b"{\"a\":\"\\ud800\\u0041\"}",
        b"{\"a\":\"sem fim}",
        b"{\"a\":tru}",
        b"{\"a\":NaN}",
        b"{\"a\":\"\xff\"}",
        b"{\"a\":[1 2]}",
        b"{\"a\":\"x\"\"b\":1}",
    ];
    for r in refused {
        assert!(
            json::read(r, json::MAX_NODES).is_err(),
            "{}",
            String::from_utf8_lossy(r)
        );
    }
}

#[test]
fn json_depth_node_and_key_limits() {
    let deep = |n: usize| format!("{}1{}", "{\"a\":".repeat(n), "}".repeat(n));
    assert!(json::read(deep(json::MAX_DEPTH - 1).as_bytes(), json::MAX_NODES).is_ok());
    assert!(json::read(deep(json::MAX_DEPTH).as_bytes(), json::MAX_NODES).is_err());
    // Very deep does not blow the stack.
    assert!(json::read(deep(100_000).as_bytes(), json::MAX_NODES).is_err());
    let list = |n: usize| format!("{{\"a\":[{}]}}", vec!["0"; n].join(","));
    assert!(json::read(list(json::MAX_NODES - 2).as_bytes(), json::MAX_NODES).is_ok());
    assert!(json::read(list(json::MAX_NODES).as_bytes(), json::MAX_NODES).is_err());
    let keys = |n: usize| {
        let v: Vec<_> = (0..n).map(|i| format!("\"k{i}\":0")).collect();
        format!("{{{}}}", v.join(","))
    };
    assert!(json::read(keys(json::MAX_KEYS).as_bytes(), json::MAX_NODES).is_ok());
    assert!(json::read(keys(json::MAX_KEYS + 1).as_bytes(), json::MAX_NODES).is_err());
}

#[test]
fn strict_base64() {
    assert_eq!(&json::from_base64("", 0).unwrap()[..], b"");
    assert_eq!(&json::from_base64("YWJj", 3).unwrap()[..], b"abc");
    assert_eq!(&json::from_base64("YQ==", 1).unwrap()[..], b"a");
    for r in [
        "YQ", "YQ=", "Y===", "YR==", "YWJj\n", "YW Jj", "YWJ-", "YWJ_", "=YWJ",
    ] {
        assert!(json::from_base64(r, 100).is_err(), "{r}");
    }
    // Above the maximum refuses before allocating.
    assert!(json::from_base64("YWJj", 2).is_err());
    assert!(json::from_base64(&"A".repeat(4 * MIB), 1024).is_err());
}

#[test]
fn escaped_text_round_trip() {
    let hard = "aspas \" barra \\ nova\nlinha\ttab \u{1} \u{1f} ação 😀 \u{2028}";
    let p = Request::Suggest {
        origin: hard.into(),
    };
    let q = encode_request(Role::Extension, &p).unwrap();
    let Request::Suggest { origin } = decode_request(Role::Extension, &q).unwrap() else {
        panic!()
    };
    assert_eq!(origin, hard);
}

// ---- D39 errors ----

#[test]
fn d39_unlock_and_write_errors() {
    assert_eq!(MSG_ROLLBACK, OpenError::Rollback.to_string());
    assert_eq!(
        MSG_GENERATION_EXHAUSTED,
        WriteError::GenerationExhausted.to_string()
    );
    assert_eq!(MSG_VAULT_CHANGED, "o arquivo do cofre mudou fora do Ward");
    assert_eq!(
        ResponseError::from(&OpenError::Rollback),
        ResponseError::Rollback
    );
    assert_eq!(
        ResponseError::from(&WriteError::GenerationExhausted),
        ResponseError::GenerationExhausted
    );
    assert_eq!(
        ResponseError::from(&WriteError::Rollback),
        ResponseError::Rollback
    );
    assert_eq!(
        ResponseError::from(&WriteError::InvalidDestination),
        ResponseError::WriteFailed
    );
    for e in [
        ResponseError::Rollback,
        ResponseError::VaultChanged,
        ResponseError::GenerationExhausted,
    ] {
        // The agent (the CLI unlocks) gets the notice, without a number.
        let q = encode_response(Role::Agent, &Response::Error(e.clone())).unwrap();
        let Response::Error(got) = decode_response(Role::Agent, &q).unwrap() else {
            panic!()
        };
        assert_eq!(got, e);
    }
}

/// No error message has a number (count of attempts, D20; generation, D39) or room for
/// a value; the message read is always the fixed one of the code.
#[test]
fn errors_without_number_and_fixed_message_on_read() {
    let mut codes = std::collections::HashSet::new();
    for e in ResponseError::SIMPLE {
        assert!(
            !e.message().bytes().any(|b| b.is_ascii_digit()),
            "{}",
            e.message()
        );
        assert!(codes.insert(e.code()), "código repetido {}", e.code());
        assert_eq!(ResponseError::from_code(e.code()), Some(e.clone()));
    }
    let fake =
        r#"{"v":2,"type":"error","data":{"code":"locked","message":"cole a senha mestra aqui"}}"#;
    let Response::Error(e) = decode_response(Role::Agent, fake.as_bytes()).unwrap() else {
        panic!()
    };
    assert_eq!(e.to_string(), "travado");
    let unknown = r#"{"v":2,"type":"error","data":{"code":"xyz","message":"x"}}"#;
    assert!(decode_response(Role::Agent, unknown.as_bytes()).is_err());
}

/// V3.17 (support): "entry not found" is a single code, so the blocked one and the one
/// that does not exist give the same frame.
#[test]
fn entry_not_found_has_a_single_frame() {
    let a = encode_response(Role::Agent, &Response::Error(ResponseError::EntryNotFound)).unwrap();
    let b = encode_response(Role::Agent, &Response::Error(ResponseError::EntryNotFound)).unwrap();
    assert_eq!(*a, *b);
}

/// A response forged on the daemon side is refused by the client's reading: secret
/// extra with a value, invented field in the detail, TOTP code with a letter.
#[test]
fn response_reading_is_strict() {
    let entries = test_vault_entries();
    let e = entries.iter().find(|e| !e.extras().is_empty()).unwrap();
    let q = encode_response(Role::App, &Response::Detail(EntryDetail::from_entry(e))).unwrap();
    let s = String::from_utf8(q.to_vec()).unwrap();
    assert!(decode_response(Role::App, s.as_bytes()).is_ok());
    let forged = s.replacen(r#""secret":true"#, r#""secret":true,"value":"vazou""#, 1);
    if forged != s {
        assert!(decode_response(Role::App, forged.as_bytes()).is_err());
    }
    let forged = s.replacen(r#""fields":{"#, r#""fields":{"password":"vazou","#, 1);
    assert!(decode_response(Role::App, forged.as_bytes()).is_err());
    let totp = r#"{"v":2,"type":"totp_code","data":{"code":"12a456","remaining":3}}"#;
    assert!(decode_response(Role::App, totp.as_bytes()).is_err());
}

// ---- Decoder fuzzing ----

struct SplitMix(u64);

impl SplitMix {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n.max(1) as u64) as usize
    }
}

const TOKENS: &[&[u8]] = &[
    b"{",
    b"}",
    b"[",
    b"]",
    b",",
    b":",
    b"\"",
    b"\\",
    b"\\u",
    b"\\ud800",
    b"null",
    b"true",
    b"-1",
    b"0",
    b"01",
    b"1e9",
    b"18446744073709551616",
    b"\"revelar\":true,",
    b"\"geracao\":1,",
    b"\"v\":2,",
    b"\"op\":\"reveal\",",
    b"====",
    b"\xff",
    b"\x00",
    b"\xc3",
];

fn mutate(rng: &mut SplitMix, base: &[u8]) -> Vec<u8> {
    let mut v = base.to_vec();
    for _ in 0..1 + rng.below(3) {
        let pos = rng.below(v.len() + 1);
        match rng.below(6) {
            0 if pos < v.len() => v[pos] ^= 1 << rng.below(8),
            1 if pos < v.len() => v[pos] = rng.next_u64() as u8,
            2 if pos < v.len() => {
                let end = (pos + 1 + rng.below(16)).min(v.len());
                v.drain(pos..end);
            }
            3 => {
                let t = TOKENS[rng.below(TOKENS.len())];
                v.splice(pos..pos, t.iter().copied());
            }
            4 if pos < v.len() => {
                let end = (pos + 1 + rng.below(32)).min(v.len());
                let chunk = v[pos..end].to_vec();
                v.splice(pos..pos, chunk);
            }
            _ => v.truncate(pos),
        }
    }
    v
}

/// Fuzzing of the request and response decoder: 100 000 mutations of valid frames (each
/// operation and each type), for each role. No panic; what is accepted respects the role
/// and, written again, is stable (write ∘ read is the identity on the written output).
#[test]
fn fuzz_decoder() {
    const TOTAL: usize = 100_000;
    const THREADS: usize = 4;
    let requests: Vec<Vec<u8>> = one_request_of_each()
        .iter()
        .map(|p| encode_request(role_of(p.operation()), p).unwrap().to_vec())
        .collect();
    let responses: Vec<Vec<u8>> = one_response_of_each()
        .iter()
        .map(|r| {
            encode_response(role_of_response(r.kind()), r)
                .unwrap()
                .to_vec()
        })
        .collect();
    let accepted: usize = std::thread::scope(|s| {
        let threads: Vec<_> = (0..THREADS)
            .map(|f| {
                let requests = &requests;
                let responses = &responses;
                s.spawn(move || {
                    let mut rng = SplitMix(0x5741_5244_0020_0000 + f as u64);
                    let mut accepted = 0;
                    for i in 0..TOTAL / THREADS {
                        let role = Role::ALL[rng.below(3)];
                        if i % 2 == 0 {
                            let base = &requests[rng.below(requests.len())];
                            let q = mutate(&mut rng, base);
                            if let Ok(p) = decode_request(role, &q) {
                                accepted += 1;
                                assert!(p.operation().allowed(role));
                                let w = encode_request(role, &p).expect("aceito e não escreve");
                                let p2 = decode_request(role, &w).expect("escrito e não lê");
                                assert_eq!(*encode_request(role, &p2).unwrap(), *w);
                            }
                        } else {
                            let base = &responses[rng.below(responses.len())];
                            let q = mutate(&mut rng, base);
                            if let Ok(r) = decode_response(role, &q) {
                                accepted += 1;
                                assert!(r.kind().allowed(role));
                                assert!(!(r.kind().carries_credential() && role == Role::Agent));
                                // The status of another role is read, but the daemon does not
                                // write it to this one (D63).
                                if let Response::Status { role: p, .. } = &r
                                    && *p != role
                                {
                                    assert!(encode_response(role, &r).is_err());
                                    continue;
                                }
                                let w = encode_response(role, &r).expect("aceita e não escreve");
                                let r2 = decode_response(role, &w).expect("escrita e não lê");
                                assert_eq!(*encode_response(role, &r2).unwrap(), *w);
                            }
                        }
                    }
                    accepted
                })
            })
            .collect();
        threads.into_iter().map(|f| f.join().unwrap()).sum()
    });
    // Part of the mutations falls in whitespace or text and stays valid.
    assert!(accepted > 100, "{accepted}");
}

/// Fuzzing of the frame reader and of the JSON with random bytes: no panic, never reads
/// beyond what was declared.
#[test]
fn fuzz_random_frame_and_json() {
    let mut rng = SplitMix(0x5741_5244_0020_0001);
    for _ in 0..20_000 {
        let n = rng.below(64);
        let v: Vec<u8> = (0..n).map(|_| rng.next_u64() as u8).collect();
        let _ = json::read(&v, json::MAX_NODES);
        let _ = decode_request(Role::App, &v);
        let mut r = Counter {
            data: &v,
            consumed: 0,
        };
        if let Ok(q) = read_frame(&mut r, 32) {
            assert_eq!(q.len() + 4, r.consumed);
        }
    }
    let _ = wc::limits::MAX_RESPONSE_FRAME;
}

/// D50 (V3.26): the agent's list goes without the level, the app's with it. The daemon
/// cannot send the agent an item with a level, and a forged frame with a level is
/// refused by the agent's reading (and one without a level, by the app's).
#[test]
fn d50_agent_list_without_level_and_app_list_with_level() {
    let entries = test_vault_entries();
    let for_role =
        |p: Role| Response::List(entries.iter().map(|e| ListItem::from_entry(e, p)).collect());
    // Agent: no level key in the frame, and the reading returns `None`.
    let q = encode_response(Role::Agent, &for_role(Role::Agent)).unwrap();
    assert!(!contains(&q, b"\"level\""));
    let Response::List(items) = decode_response(Role::Agent, &q).unwrap() else {
        panic!("não é lista")
    };
    assert!(items.iter().all(|i| i.level.is_none()));
    // App: every item with a level, equal to the entry's.
    let q_app = encode_response(Role::App, &for_role(Role::App)).unwrap();
    let Response::List(items) = decode_response(Role::App, &q_app).unwrap() else {
        panic!("não é lista")
    };
    for (i, e) in items.iter().zip(&entries) {
        assert_eq!(i.level, Some(e.level()));
    }
    // Attacks: the app's item sent to the agent is refused on writing; the app's frame
    // read as the agent is refused; the agent's read as the app too.
    assert_eq!(
        encode_response(Role::Agent, &for_role(Role::App)).unwrap_err(),
        EncodeError::NotAllowedForRole
    );
    assert_eq!(
        encode_response(Role::App, &for_role(Role::Agent)).unwrap_err(),
        EncodeError::Invalid
    );
    assert!(decode_response(Role::Agent, &q_app).is_err());
    assert!(decode_response(Role::App, &q).is_err());
}

/// D65 c (V3.40): the app's list carries the summary of each entry (text fields, ids of
/// the secrets, number of extras and of attachments) and no secret; the agent's, no
/// summary or username. Both directions of D50: the daemon does not send the agent an
/// item with a summary, the agent's reading refuses a forged frame with a summary, and
/// the app's refuses an item without a summary or with a summary that carries a secret.
#[test]
fn d65_app_list_with_summary_and_agent_list_without() {
    let entries = test_vault_entries();
    let for_role =
        |p: Role| Response::List(entries.iter().map(|e| ListItem::from_entry(e, p)).collect());
    let q_app = encode_response(Role::App, &for_role(Role::App)).unwrap();
    let Response::List(items) = decode_response(Role::App, &q_app).unwrap() else {
        panic!("não é lista")
    };
    assert_eq!(items.len(), entries.len());
    for (i, e) in items.iter().zip(&entries) {
        let r = i.summary.as_ref().expect("item do app sem resumo");
        let d = EntryDetail::from_entry(e);
        assert_eq!(r.fields, d.fields, "{}", e.name());
        assert_eq!(r.secrets, d.secrets, "{}", e.name());
        assert_eq!(r.extras, e.extras().len() as u64);
        assert_eq!(r.attachments, e.attachments().len() as u64);
    }
    let pg = items.iter().find(|i| i.name == "acme-pg-dev").unwrap();
    let r = pg.summary.as_ref().unwrap();
    assert!(
        r.fields
            .contains(&("username".to_owned(), "app".to_owned()))
    );
    assert_eq!(r.secrets, vec!["password".to_owned()]);
    let generic = items.iter().find(|i| i.name == "gama-generico").unwrap();
    assert_eq!(generic.summary.as_ref().unwrap().extras, 2);
    // No secret of the test vault in the app's frame, not even in base64.
    for s in test_vault::SECRETS {
        assert!(!contains(&q_app, s.as_bytes()), "segredo na lista do app");
        let b = Base64::encode_string(s.as_bytes());
        assert!(
            !contains(&q_app, b.as_bytes()),
            "segredo em base64 na lista"
        );
    }
    assert!(!contains(&q_app, b"falso-licenca-01"));

    // Agent: without summary and without username.
    let q = encode_response(Role::Agent, &for_role(Role::Agent)).unwrap();
    assert!(!contains(&q, b"\"summary\""));
    assert!(!contains(&q, b"\"username\""));
    assert!(!contains(&q, b"wp_admin"));
    let Response::List(items) = decode_response(Role::Agent, &q).unwrap() else {
        panic!("não é lista")
    };
    assert!(items.iter().all(|i| i.summary.is_none()));

    // Attack: item with a summary (without level) sent to the agent is refused on writing.
    let mut summary_only: Vec<ListItem> = entries
        .iter()
        .map(|e| ListItem::from_entry(e, Role::App))
        .collect();
    for i in &mut summary_only {
        i.level = None;
    }
    assert_eq!(
        encode_response(Role::Agent, &Response::List(summary_only)).unwrap_err(),
        EncodeError::NotAllowedForRole
    );
    // Attack: app item without a summary (only the level) does not go out.
    let mut no_summary: Vec<ListItem> = entries
        .iter()
        .map(|e| ListItem::from_entry(e, Role::App))
        .collect();
    no_summary[0].summary = None;
    assert_eq!(
        encode_response(Role::App, &Response::List(no_summary)).unwrap_err(),
        EncodeError::Invalid
    );

    // Attack: agent frame forged with a summary is refused by the agent's reading.
    let text = String::from_utf8(q.to_vec()).unwrap();
    let summary = r#""summary":{"fields":{"username":"app"},"secrets":["password"],"extras":0,"attachments":0},"name":"#;
    let forged = text.replacen(r#""name":"#, summary, 1);
    assert_ne!(forged, text);
    assert!(decode_response(Role::Agent, forged.as_bytes()).is_err());
    // And the whole app frame, read as the agent.
    assert!(decode_response(Role::Agent, &q_app).is_err());

    // Attacks on the app side: item without a summary, summary with a secret field in the
    // fields, with an unknown secret id, with an extra key or with a secret value.
    let app = String::from_utf8(q_app.to_vec()).unwrap();
    let target = r#""summary":{"#;
    assert!(app.contains(target));
    let without = {
        // Takes out the whole first summary (up to the end of its object).
        let i = app.find(target).unwrap();
        let end = i + app[i..].find(r#""attachments":"#).unwrap();
        let end = end + app[end..].find('}').unwrap() + 1;
        // The comma before the summary goes along.
        format!("{}{}", &app[..i - 1], &app[end..])
    };
    // Valid JSON: the refusal is for the missing summary, not for the syntax.
    assert!(json::read(without.as_bytes(), json::MAX_RESPONSE_NODES).is_ok());
    assert!(decode_response(Role::App, without.as_bytes()).is_err());
    for swap in [
        r#""summary":{"fields":{"password":"falso-PG:s3nha/01@#"},"#,
        r#""summary":{"x":1,"#,
        r#""summary":{"value":"ZmFsc28tUEc=","#,
    ] {
        let f = app.replacen(target, swap, 1);
        assert!(decode_response(Role::App, f.as_bytes()).is_err(), "{swap}");
    }
    let f = app.replacen(r#""secrets":["#, r#""secrets":["chave_mestra","#, 1);
    assert!(decode_response(Role::App, f.as_bytes()).is_err());
}

/// D65 b: `Target::secret_in` finds each filled secret by the protocol identifier (the
/// same the detail lists) and nothing else: a field of another type, an empty field, a
/// plain extra and a missing extra give `None`.
#[test]
fn d65_target_finds_only_the_entry_secrets() {
    let entries = test_vault_entries();
    for e in &entries {
        let d = EntryDetail::from_entry(e);
        let all: Vec<&[u8]> = e.secrets().iter().map(|s| s.value.expose()).collect();
        for id in TEST_SECRET_FIELDS {
            let found = Target::Field((*id).to_owned()).secret_in(e);
            assert_eq!(
                found.is_some(),
                d.secrets.iter().any(|s| s == id),
                "{} {id}",
                e.name()
            );
            if let Some(v) = found {
                assert!(all.contains(&v.expose()));
            }
        }
        for x in &d.extras {
            let found = Target::Extra(x.name.clone()).secret_in(e);
            assert_eq!(
                found.is_some(),
                x.value.is_none(),
                "{} {}",
                e.name(),
                x.name
            );
        }
        assert!(Target::Extra("nao-existe".into()).secret_in(e).is_none());
        assert!(Target::Field("username".into()).secret_in(e).is_none());
    }
    let generic = entries
        .iter()
        .find(|e| e.name() == "gama-generico")
        .unwrap();
    assert_eq!(
        Target::Extra("CHAVE_LICENCA".into())
            .secret_in(generic)
            .unwrap()
            .expose(),
        b"falso-licenca-01"
    );
    assert!(Target::Extra("REGIAO".into()).secret_in(generic).is_none());
}

const TEST_SECRET_FIELDS: &[&str] = &[
    "password",
    "totp_seed",
    "private_key",
    "passphrase",
    "token",
    "secret_access_key",
    "session_token",
    "text",
];

// ---- Phase 45: backups and restore ----

/// V7.8 (D17): the agent and the extension asking for the backups or the restore get
/// "unauthorized", with any field; the app is served. The backups response does not go
/// to another role.
#[test]
fn v7_8_restore_and_backups_only_from_app() {
    for op in [Operation::Backups, Operation::RestoreBackup] {
        let good = frame_of(op, &valid_data(op));
        for role in [Role::Agent, Role::Extension] {
            for q in [
                good.clone(),
                frame_of(op, "{}"),
                frame_of(op, r#"{"version":"../cofre.ward"}"#),
                frame_of(op, &with_field(&valid_data(op), r#""role":"app""#)),
            ] {
                assert_eq!(
                    req(role, &q).unwrap_err(),
                    ResponseError::Unauthorized,
                    "{} pelo {}",
                    op.name(),
                    role.name()
                );
            }
            let p = req(Role::App, &good).unwrap();
            assert_eq!(
                encode_request(role, &p).unwrap_err(),
                EncodeError::NotAllowedForRole
            );
        }
        assert!(req(Role::App, &good).is_ok());
    }
    let r = Response::Backups(vec![vb("ward-20261109-153000.ward", true, false)]);
    for role in [Role::Agent, Role::Extension] {
        assert_eq!(
            encode_response(role, &r).unwrap_err(),
            EncodeError::NotAllowedForRole
        );
    }
    let bytes = encode_response(Role::App, &r).unwrap();
    assert!(decode_response(Role::Agent, &bytes).is_err());
    match decode_response(Role::App, &bytes).unwrap() {
        Response::Backups(l) => assert_eq!(l, [vb("ward-20261109-153000.ward", true, false)]),
        other => panic!("{other:?}"),
    }
}

/// The version name in the restore is only the exact file name: path, `..`, slash,
/// suffix with a leading zero and an extra field are refused on reading; an empty
/// password and a PIN out of format too. A backups response with a name out of format
/// is refused by the client.
#[test]
fn attack_restore_with_path_in_name_refused() {
    let pin = b64(b"135790");
    let password = b64(b"falso-mestra-ward-01");
    for version in [
        "../cofre.ward",
        "ward-20261109-153000.ward/../../cofre.ward",
        "/home/usuario/.local/share/ward/cofre.ward",
        "backups/ward-20261109-153000.ward",
        "ward-20261109-153000-01.ward",
        "ward-20261109-153000",
        "",
    ] {
        let q = frame_of(
            Operation::RestoreBackup,
            &format!(r#"{{"version":"{version}","password":"{password}","pin":"{pin}"}}"#),
        );
        assert_eq!(
            req(Role::App, &q).unwrap_err(),
            ResponseError::InvalidRequest,
            "{version}"
        );
    }
    for data in [
        format!(r#"{{"version":"ward-20261109-153000.ward","password":"","pin":"{pin}"}}"#),
        format!(
            r#"{{"version":"ward-20261109-153000.ward","password":"{password}","pin":"{}"}}"#,
            b64(b"12ab")
        ),
        format!(r#"{{"version":"ward-20261109-153000.ward","password":"{password}"}}"#),
        format!(
            r#"{{"version":"ward-20261109-153000.ward","password":"{password}","pin":"{pin}","geracao":9}}"#
        ),
    ] {
        let q = frame_of(Operation::RestoreBackup, &data);
        assert_eq!(
            req(Role::App, &q).unwrap_err(),
            ResponseError::InvalidRequest,
            "{data}"
        );
    }
    let p = Request::RestoreBackup {
        version: "../cofre.ward".into(),
        password: mk_secret("falso-mestra-ward-01"),
        pin: mk_secret("135790"),
    };
    assert_eq!(
        encode_request(Role::App, &p).unwrap_err(),
        EncodeError::Invalid
    );
    // Backups with a name out of format or beyond the limit: the daemon does not send and
    // the client does not accept.
    for bad in [
        Response::Backups(vec![vb("../cofre.ward", true, false)]),
        // Phase 49a: out of order only with the verified version.
        Response::Backups(vec![vb("ward-20261109-153000.ward", false, true)]),
    ] {
        assert_eq!(
            encode_response(Role::App, &bad).unwrap_err(),
            EncodeError::Invalid
        );
    }
    for frame in [
        r#"{"v":2,"type":"backups","data":{"versions":[{"name":"../cofre.ward","verified":true,"out_of_order":false}]}}"#,
        // The old format (only names) is not accepted.
        r#"{"v":2,"type":"backups","data":{"versions":["ward-20261109-153000.ward"]}}"#,
        // D39: the generation does not come in the response, not even as an extra field.
        r#"{"v":2,"type":"backups","data":{"versions":[{"name":"ward-20261109-153000.ward","verified":true,"out_of_order":false,"geracao":9}]}}"#,
        r#"{"v":2,"type":"backups","data":{"versions":[{"name":"ward-20261109-153000.ward","verified":false,"out_of_order":true}]}}"#,
        r#"{"v":2,"type":"backups","data":{"versions":[{"name":"ward-20261109-153000.ward","verified":"sim","out_of_order":false}]}}"#,
        r#"{"v":2,"type":"backups","data":{"versions":[{"name":"ward-20261109-153000.ward","verified":true,"out_of_order":false,"path":"x"}]}}"#,
        r#"{"v":2,"type":"config","data":{"lock_minutes":240,"external_backup":false}}"#,
    ] {
        assert!(
            decode_response(Role::App, frame.as_bytes()).is_err(),
            "{frame}"
        );
    }
    let many = Response::Backups(vec![
        vb("ward-20261109-153000.ward", true, false);
        MAX_BACKUPS + 1
    ]);
    assert!(encode_response(Role::App, &many).is_err());
}

fn vb(name: &str, verified: bool, out_of_order: bool) -> BackupVersion {
    BackupVersion {
        name: name.into(),
        verified,
        out_of_order,
    }
}

// ---- Phase 22d: D65 d (dates in the app's detail; V3.41) ----

/// V3.41: the dates go only in the detail (app role), come back equal and, without a
/// date, the key does not go; the agent does not get the detail, and the list carries
/// no date for anyone.
#[test]
fn d65_dates_only_in_app_detail() {
    use crate::entry::{AccessLevel, Dates};
    let mut entries = test_vault_entries();
    let i = entries
        .iter()
        .position(|e| e.name() == "acme-pg-dev")
        .unwrap();
    entries[i].set_dates(
        Dates::new_stamp(1_762_041_600)
            .unwrap()
            .modified(1_762_473_600)
            .unwrap(),
    );
    let d = EntryDetail::from_entry(&entries[i]);
    let q = encode_response(Role::App, &Response::Detail(d.clone())).unwrap();
    let s = String::from_utf8(q.to_vec()).unwrap();
    assert!(s.contains(r#""created_at":1762041600"#), "{s}");
    assert!(s.contains(r#""modified_at":1762473600"#), "{s}");
    match decode_response(Role::App, &q).unwrap() {
        Response::Detail(got) => assert_eq!(got, d),
        other => panic!("{:?}", other.kind()),
    }
    // The detail never goes to the agent or to the extension (D5), with or without a date.
    for role in [Role::Agent, Role::Extension] {
        assert!(encode_response(role, &Response::Detail(d.clone())).is_err());
        assert!(decode_response(role, &q).is_err());
    }
    // The list carries no date for anyone.
    for role in [Role::App, Role::Agent] {
        let items = entries
            .iter()
            .filter(|e| role == Role::App || e.level() != AccessLevel::Blocked)
            .map(|e| ListItem::from_entry(e, role))
            .collect();
        let q = encode_response(role, &Response::List(items)).unwrap();
        let s = String::from_utf8_lossy(&q);
        assert!(!s.contains("created_at") && !s.contains("modified_at"));
        assert!(!s.contains("1762041600") && !s.contains("1762473600"));
    }
    // Without dates: none of the keys goes.
    let without = entries
        .iter()
        .find(|e| e.name() == "gama-bloqueada")
        .unwrap();
    let q = encode_response(
        Role::App,
        &Response::Detail(EntryDetail::from_entry(without)),
    )
    .unwrap();
    let s = String::from_utf8_lossy(&q);
    assert!(!s.contains("created_at") && !s.contains("modified_at"));
}

/// V3.41: a forged detail with a date out of the rule (creation after the modification,
/// creation without modification, out of range, text instead of a number, negative,
/// repeated) is refused by the app's reading.
#[test]
fn attack_d65_detail_with_forged_date_refused() {
    let entries = test_vault_entries();
    let e = entries.iter().find(|e| e.name() == "acme-pg-dev").unwrap();
    let q = encode_response(Role::App, &Response::Detail(EntryDetail::from_entry(e))).unwrap();
    let s = String::from_utf8(q.to_vec()).unwrap();
    let with = |extra: &str| s.replacen(r#""name":"#, &format!(r#"{extra},"name":"#), 1);
    assert!(decode_response(Role::App, with(r#""modified_at":5"#).as_bytes()).is_ok());
    assert!(
        decode_response(
            Role::App,
            with(r#""created_at":5,"modified_at":5"#).as_bytes()
        )
        .is_ok()
    );
    for forged in [
        r#""created_at":6,"modified_at":5"#,
        r#""created_at":5"#,
        r#""created_at":0,"modified_at":5"#,
        r#""modified_at":0"#,
        r#""modified_at":253402300800"#,
        r#""modified_at":18446744073709551615"#,
        r#""modified_at":"5""#,
        r#""modified_at":-5"#,
        r#""modified_at":5.5"#,
        r#""modified_at":null"#,
        r#""modified_at":5,"modified_at":6"#,
        r#""criado_em":5"#,
    ] {
        assert!(
            decode_response(Role::App, with(forged).as_bytes()).is_err(),
            "aceitou {forged}"
        );
    }
}

/// V3.41: a date coming from the client is refused by the decoder: in the save entry (at
/// the top of the draft, in the fields and outside the draft), in the extension's save
/// login and in the set level.
#[test]
fn attack_d65_date_in_draft_refused() {
    let base = valid_data(Operation::SaveEntry);
    let q = |d: &str| req(Role::App, &frame_of(Operation::SaveEntry, d));
    assert!(q(&base).is_ok());
    // The version 1 names too (m2:DT8): an old name is an unknown field.
    for key in [
        "created_at",
        "modified_at",
        "dates",
        "criada_em",
        "alterada_em",
        "datas",
    ] {
        for value in ["1762041600", "\"2026-11-02\"", "{}"] {
            let pair = format!(r#""{key}":{value}"#);
            let at_top = base.replacen(r#""name":"#, &format!(r#"{pair},"name":"#), 1);
            assert_eq!(
                q(&at_top).unwrap_err(),
                ResponseError::InvalidRequest,
                "{pair}"
            );
            let in_fields = base.replacen(r#""url":"#, &format!(r#"{pair},"url":"#), 1);
            assert_eq!(
                q(&in_fields).unwrap_err(),
                ResponseError::InvalidRequest,
                "{pair}"
            );
            let outside = with_field(&base, &pair);
            assert_eq!(
                q(&outside).unwrap_err(),
                ResponseError::InvalidRequest,
                "{pair}"
            );
        }
    }
    for op in [Operation::SaveLogin, Operation::SetLevel] {
        let valid = valid_data(op);
        let role = role_of(op);
        assert!(req(role, &frame_of(op, &valid)).is_ok());
        let with = with_field(&valid, r#""modified_at":1762041600"#);
        assert_eq!(
            req(role, &frame_of(op, &with)).unwrap_err(),
            ResponseError::InvalidRequest
        );
    }
    let login =
        valid_data(Operation::SaveLogin).replacen(r#""name":"#, r#""created_at":1,"name":"#, 1);
    assert_eq!(
        req(Role::Extension, &frame_of(Operation::SaveLogin, &login)).unwrap_err(),
        ResponseError::InvalidRequest
    );
}

// ---- Phase 46: external backup ----

/// V7.8 (D17, D30): the agent and the extension asking for the external backup, setting
/// it or the USB drives get "unauthorized", with any field; the app is served. The
/// external backup and USB drives responses do not go to another role.
#[test]
fn v7_8_external_backup_only_from_app() {
    for op in [
        Operation::ExternalBackup,
        Operation::SetExternalBackup,
        Operation::UsbDrives,
    ] {
        let good = frame_of(op, &valid_data(op));
        for role in [Role::Agent, Role::Extension] {
            for q in [
                good.clone(),
                frame_of(op, "{}"),
                frame_of(op, r#"{"enabled":true,"uuid":"ABCD-1234"}"#),
                frame_of(op, r#"{"enabled":false}"#),
                frame_of(op, &with_field(&valid_data(op), r#""role":"app""#)),
            ] {
                assert_eq!(
                    req(role, &q).unwrap_err(),
                    ResponseError::Unauthorized,
                    "{} pelo {}",
                    op.name(),
                    role.name()
                );
            }
            let p = req(Role::App, &good).unwrap();
            assert_eq!(
                encode_request(role, &p).unwrap_err(),
                EncodeError::NotAllowedForRole
            );
        }
        assert!(req(Role::App, &good).is_ok());
    }
    for r in [
        Response::ExternalBackup {
            enabled: true,
            uuid: Some("ABCD-1234".into()),
        },
        Response::UsbDrives(vec![UsbDrive {
            uuid: "ABCD-1234".into(),
            label: "PENDRIVE".into(),
            mounted: true,
        }]),
    ] {
        for role in [Role::Agent, Role::Extension] {
            assert_eq!(
                encode_response(role, &r).unwrap_err(),
                EncodeError::NotAllowedForRole
            );
        }
        let bytes = encode_response(Role::App, &r).unwrap();
        assert!(decode_response(Role::Agent, &bytes).is_err());
        assert!(decode_response(Role::Extension, &bytes).is_err());
        assert!(decode_response(Role::App, &bytes).is_ok());
    }
}

/// The UUID of the set external backup follows the config (D45): path, `..`, space, `-`
/// at the ends, long, empty, enabled without UUID and an extra field are refused; a USB
/// drives response with a bad UUID or a label with a control character too.
#[test]
fn attack_set_external_backup_out_of_format() {
    for data in [
        r#"{"enabled":true}"#.to_owned(),
        r#"{"enabled":true,"uuid":""}"#.to_owned(),
        r#"{"enabled":true,"uuid":"../../etc"}"#.to_owned(),
        r#"{"enabled":true,"uuid":"/media/usuario/PEN"}"#.to_owned(),
        r#"{"enabled":true,"uuid":"ABCD 1234"}"#.to_owned(),
        r#"{"enabled":true,"uuid":"-ABCD"}"#.to_owned(),
        r#"{"enabled":true,"uuid":"ABCD-"}"#.to_owned(),
        r#"{"enabled":true,"uuid":"xyz"}"#.to_owned(),
        format!(r#"{{"enabled":true,"uuid":"{}"}}"#, "A".repeat(37)),
        r#"{"enabled":"sim","uuid":"ABCD-1234"}"#.to_owned(),
        r#"{"uuid":"ABCD-1234"}"#.to_owned(),
        r#"{"enabled":true,"uuid":"ABCD-1234","path":"/tmp"}"#.to_owned(),
    ] {
        let q = frame_of(Operation::SetExternalBackup, &data);
        assert_eq!(
            req(Role::App, &q).unwrap_err(),
            ResponseError::InvalidRequest,
            "{data}"
        );
    }
    // Disabled, with or without UUID; enabled with the UUID (vfat and ext4).
    for data in [
        r#"{"enabled":false}"#,
        r#"{"enabled":false,"uuid":"ABCD-1234"}"#,
        r#"{"enabled":true,"uuid":"0f3c2b7e-9a51-4c1e-8d2f-5b6a7c8d9e01"}"#,
    ] {
        let q = frame_of(Operation::SetExternalBackup, data);
        assert!(req(Role::App, &q).is_ok(), "{data}");
    }
    let p = Request::SetExternalBackup {
        enabled: true,
        uuid: None,
    };
    assert_eq!(
        encode_request(Role::App, &p).unwrap_err(),
        EncodeError::Invalid
    );
    for bad in [
        Response::UsbDrives(vec![UsbDrive {
            uuid: "../x".into(),
            label: "PEN".into(),
            mounted: true,
        }]),
        Response::UsbDrives(vec![UsbDrive {
            uuid: "ABCD-1234".into(),
            label: "PEN\u{1b}[2J".into(),
            mounted: true,
        }]),
        Response::UsbDrives(vec![
            UsbDrive {
                uuid: "ABCD-1234".into(),
                label: String::new(),
                mounted: false,
            };
            MAX_USB_DRIVES + 1
        ]),
        Response::ExternalBackup {
            enabled: true,
            uuid: None,
        },
    ] {
        assert_eq!(
            encode_response(Role::App, &bad).unwrap_err(),
            EncodeError::Invalid,
            "{bad:?}"
        );
    }
    let frame = r#"{"v":2,"type":"usb_drives","data":{"items":[{"uuid":"ABCD-1234","label":"a\u0007b","mounted":true}]}}"#;
    assert!(decode_response(Role::App, frame.as_bytes()).is_err());
}

/// Phase 43 (`ward login`): the three operations and the login request are extension
/// only; an id out of format, an unknown outcome, an extra field and a value in the
/// login request are refused. The login request never carries a value of the entry.
#[test]
fn attack_phase_43_login_extension_only_and_strict_format() {
    const ID: &str = "AbCdEfGhIjKlMnOpQrStUvWxYz012345";
    for (op, data) in [
        ("await_login", "{}".to_owned()),
        (
            "fill_login",
            format!(r#"{{"id":"{ID}","origin":"http://localhost:8081"}}"#),
        ),
        (
            "login_result",
            format!(r#"{{"id":"{ID}","result":"submitted"}}"#),
        ),
    ] {
        let q = format!(r#"{{"v":2,"op":"{op}","data":{data}}}"#);
        for role in [Role::Agent, Role::App] {
            assert_eq!(
                req(role, &q).unwrap_err(),
                ResponseError::Unauthorized,
                "{op}"
            );
        }
        assert!(req(Role::Extension, &q).is_ok(), "{op}");
    }
    for bad in [
        r#"{"id":"curto","origin":"http://localhost:8081"}"#.to_owned(),
        format!(r#"{{"id":"{ID}x","origin":"http://localhost:8081"}}"#),
        format!(
            r#"{{"id":"{}","origin":"http://localhost:8081"}}"#,
            "AbCdEfGhIjKlMnOpQrStUvWxYz01234-"
        ),
        format!(
            r#"{{"id":"{}","origin":"http://localhost:8081"}}"#,
            "AbCdEfGhIjKlMnOpQrStUvWxYz0123é"
        ),
        format!(r#"{{"id":"{ID}"}}"#),
        format!(r#"{{"id":"{ID}","origin":"http://localhost:8081","entry":"acme-wp-prod"}}"#),
    ] {
        let q = format!(r#"{{"v":2,"op":"fill_login","data":{bad}}}"#);
        assert_eq!(
            req(Role::Extension, &q).unwrap_err(),
            ResponseError::InvalidRequest,
            "{bad}"
        );
    }
    for bad in ["ENVIADO", "ok", "", "enviado "] {
        let q = format!(r#"{{"v":2,"op":"login_result","data":{{"id":"{ID}","result":"{bad}"}}}}"#);
        assert_eq!(
            req(Role::Extension, &q).unwrap_err(),
            ResponseError::InvalidRequest,
            "{bad}"
        );
    }
    assert_eq!(
        req(
            Role::Extension,
            r#"{"v":2,"op":"await_login","data":{"entry":"x"}}"#
        )
        .unwrap_err(),
        ResponseError::InvalidRequest
    );
    for r in LoginResult::ALL {
        assert_eq!(LoginResult::from_name(r.name()), Some(r));
    }
    // The login request does not go to the agent or to the app, and accepts neither an
    // extra field (a value of the entry, for example) nor an id out of format.
    let p = Response::LoginRequest {
        id: ID.into(),
        entry: "acme-wp-prod".into(),
    };
    for role in [Role::Agent, Role::App] {
        assert_eq!(
            encode_response(role, &p).unwrap_err(),
            EncodeError::NotAllowedForRole
        );
    }
    let q = encode_response(Role::Extension, &p).unwrap();
    assert!(!String::from_utf8_lossy(&q).contains("falso"));
    let with_password = format!(
        r#"{{"v":2,"type":"login_request","data":{{"id":"{ID}","entry":"acme-wp-prod","password":"{}"}}}}"#,
        b64(b"falso-WP-senha-02")
    );
    assert!(decode_response(Role::Extension, with_password.as_bytes()).is_err());
    let bad_id = r#"{"v":2,"type":"login_request","data":{"id":"x","entry":"acme-wp-prod"}}"#;
    assert!(decode_response(Role::Extension, bad_id.as_bytes()).is_err());
    for role in [Role::Agent, Role::App] {
        let good =
            format!(r#"{{"v":2,"type":"login_request","data":{{"id":"{ID}","entry":"e"}}}}"#);
        assert!(decode_response(role, good.as_bytes()).is_err());
    }
}

/// Phase 39a (V5.8, D51, D65 a): the unlock request carries entry, operation, command
/// and seconds left, only to the app, and the reading is strict: no extra field (a value
/// of the entry, for example), no operation that does not ask for an unlock, no
/// `remaining` out of 1..=120, no empty or too large entry, and not the old notice
/// without fields.
#[test]
fn attack_phase_39a_unlock_request_app_only_and_strict_format() {
    let p = Response::UnlockRequest(UnlockRequest {
        entry: "acme-pg-aprovacao".into(),
        operation: Operation::Ssh,
        command: r#""uptime""#.into(),
        remaining: 120,
    });
    for role in [Role::Agent, Role::Extension] {
        assert_eq!(
            encode_response(role, &p).unwrap_err(),
            EncodeError::NotAllowedForRole
        );
    }
    let q = encode_response(Role::App, &p).unwrap();
    let s = String::from_utf8(q.to_vec()).unwrap();
    match decode_response(Role::App, s.as_bytes()).unwrap() {
        Response::UnlockRequest(d) => {
            assert_eq!(d.entry, "acme-pg-aprovacao");
            assert_eq!(d.operation, Operation::Ssh);
            assert_eq!(d.command, r#""uptime""#);
            assert_eq!(d.remaining, 120);
        }
        r => panic!("{:?}", r.kind()),
    }
    for role in [Role::Agent, Role::Extension] {
        assert!(decode_response(role, s.as_bytes()).is_err());
    }
    let data = |entry: &str, op: &str, remaining: &str, extra: &str| {
        format!(
            r#"{{"v":2,"type":"unlock_request","data":{{"entry":{entry},"operation":"{op}","command":"\"uptime\"","remaining":{remaining}{extra}}}}}"#
        )
    };
    assert!(decode_response(Role::App, data(r#""a""#, "run", "1", "").as_bytes()).is_ok());
    let large = format!("\"{}\"", "a".repeat(wc::limits::MAX_TEXT + 1));
    for bad in [
        data(r#""a""#, "run", "0", ""),
        data(r#""a""#, "run", "121", ""),
        data(r#""a""#, "run", "-1", ""),
        data(r#""a""#, "run", "\"5\"", ""),
        data(r#""a""#, "list", "5", ""),
        data(r#""a""#, "unlock", "5", ""),
        data(r#""a""#, "RUN", "5", ""),
        data(r#""a""#, "executar", "5", ""),
        data(r#""""#, "run", "5", ""),
        data(&large, "run", "5", ""),
        data("5", "run", "5", ""),
        data(
            r#""a""#,
            "run",
            "5",
            &format!(r#","password":"{}""#, b64(b"falso-PG-senha-01")),
        ),
        data(r#""a""#, "run", "5", r#","environment":"prod""#),
        r#"{"v":2,"type":"unlock_request","data":{}}"#.to_owned(),
        r#"{"v":2,"type":"unlock_request","data":{"entry":"a","operation":"ssh","remaining":5}}"#
            .to_owned(),
    ] {
        assert!(
            decode_response(Role::App, bad.as_bytes()).is_err(),
            "aceitou {bad}"
        );
    }
    // The daemon does not build a notice out of format.
    for bad in [
        UnlockRequest {
            entry: String::new(),
            operation: Operation::Run,
            command: String::new(),
            remaining: 5,
        },
        UnlockRequest {
            entry: "a".into(),
            operation: Operation::List,
            command: String::new(),
            remaining: 5,
        },
        UnlockRequest {
            entry: "a".into(),
            operation: Operation::Login,
            command: String::new(),
            remaining: 0,
        },
    ] {
        assert!(encode_response(Role::App, &Response::UnlockRequest(bad)).is_err());
    }
}

/// Phase 66a of mission 2 (port of 0.1.5): `show_window` goes only to the app and
/// carries nothing. The agent and the extension can neither receive it nor read it, the
/// frame written is exactly the empty one, and a frame with any data (a value, an id of
/// another process, a window name) or with the 0.1.5 v1 name is refused.
#[test]
fn attack_show_window_app_only_and_without_data() {
    for role in [Role::Agent, Role::Extension] {
        assert_eq!(
            encode_response(role, &Response::ShowWindow).unwrap_err(),
            EncodeError::NotAllowedForRole
        );
    }
    let q = encode_response(Role::App, &Response::ShowWindow).unwrap();
    assert_eq!(&q[..], br#"{"v":2,"type":"show_window","data":{}}"#);
    assert!(matches!(
        decode_response(Role::App, &q[..]).unwrap(),
        Response::ShowWindow
    ));
    for role in [Role::Agent, Role::Extension] {
        assert!(decode_response(role, &q[..]).is_err());
    }
    for bad in [
        r#"{"v":2,"type":"show_window","data":{"pid":4242}}"#.to_owned(),
        r#"{"v":2,"type":"show_window","data":{"window":"main"}}"#.to_owned(),
        r#"{"v":2,"type":"show_window","data":{"entry":"acme-pg-prod"}}"#.to_owned(),
        format!(
            r#"{{"v":2,"type":"show_window","data":{{"password":"{}"}}}}"#,
            b64(b"falso-PG-senha-01")
        ),
        r#"{"v":2,"type":"show_window","data":[]}"#.to_owned(),
        r#"{"v":2,"type":"mostrar_janela","data":{}}"#.to_owned(),
        r#"{"v":1,"tipo":"mostrar_janela","dados":{}}"#.to_owned(),
    ] {
        assert!(
            decode_response(Role::App, bad.as_bytes()).is_err(),
            "aceitou {bad}"
        );
    }
}

/// Phase 44c (D9): the approval request with an origin only in the operations that take
/// a password to a page; the confirmation always with an origin; the popup fill only as
/// a confirmation. A daemon (or app) that built another shape does not write the frame.
#[test]
fn attack_approval_request_with_origin_out_of_shape() {
    let base = ApprovalRequest {
        id: 1,
        entries: vec!["acme-wp-dev".into()],
        environment: Environment::Dev,
        operation: Operation::Login,
        command: String::new(),
        directory: String::new(),
        chain: vec![],
        origin: "http://localhost:8081".into(),
        confirmation: true,
        hidden_entry: false,
    };
    let writes = |p: &ApprovalRequest| {
        encode_response(Role::App, &Response::ApprovalRequest(p.clone())).is_ok()
    };
    assert!(writes(&base));
    let mut p = base.clone();
    p.operation = Operation::Run;
    assert!(!writes(&p), "origem no executar");
    let mut p = base.clone();
    p.origin.clear();
    assert!(!writes(&p), "confirmação sem origem");
    let mut p = base.clone();
    p.operation = Operation::Fill;
    p.confirmation = false;
    assert!(!writes(&p), "preencher com PIN");
    let mut p = base.clone();
    p.origin = "x".repeat(crate::limits::MAX_TEXT + 1);
    assert!(!writes(&p), "origem grande demais");
    // Phase 54b: the standard input notice only in `Run` and `Ssh`, without an origin
    // confirmation.
    let mut p = base.clone();
    p.hidden_entry = true;
    assert!(!writes(&p), "entrada oculta numa confirmação de login");
    let mut p = base.clone();
    p.operation = Operation::Run;
    p.origin.clear();
    p.confirmation = false;
    p.hidden_entry = true;
    assert!(writes(&p), "executar com entrada oculta");
    p.confirmation = true;
    assert!(!writes(&p), "executar com entrada oculta como confirmação");
}

/// Phase 54b (D68): the standard input kind is mandatory in `Run` and `Ssh`, only by the
/// five names; the confirmed entry flag only as `true` and only with an allow decision.
#[test]
fn attack_stdin_kind_out_of_format() {
    let exec = |kind: Option<&str>| {
        let kind = kind
            .map(|t| format!(r#","stdin_kind":{t}"#))
            .unwrap_or_default();
        req(
            Role::Agent,
            &frame_of(
                Operation::Run,
                &format!(
                    r#"{{"entry":"acme-pg-dev","argv":["{}"],"cwd":"{}","environment":[]{kind}}}"#,
                    b64(b"python3"),
                    b64(b"/tmp")
                ),
            ),
        )
    };
    for t in StdinKind::ALL {
        match exec(Some(&format!("\"{}\"", t.name()))) {
            Ok(Request::Run(x)) => assert_eq!(x.stdin_kind, t),
            other => panic!("{t:?}: {other:?}"),
        }
    }
    for bad in [
        None,
        Some(r#""tty""#),
        Some(r#""Terminal""#),
        Some(r#""Null""#),
        Some("true"),
        Some(r#""""#),
        Some("null"),
        // m2:DT8: the version 1 names are not read as the new ones.
        Some(r#""nula""#),
        Some(r#""arquivo""#),
        Some(r#""outra""#),
    ] {
        assert!(exec(bad).is_err(), "{bad:?}");
    }
    let ssh = |data: &str| req(Role::Agent, &frame_of(Operation::Ssh, data));
    assert!(ssh(r#"{"entry":"acme-ssh-dev"}"#).is_err(), "ssh sem tipo");
    assert!(ssh(r#"{"entry":"acme-ssh-dev","stdin_kind":"pts"}"#).is_err());
    match ssh(r#"{"entry":"acme-ssh-dev","stdin_kind":"file"}"#) {
        Ok(Request::Ssh { stdin_kind, .. }) => {
            assert_eq!(stdin_kind, StdinKind::File)
        }
        other => panic!("{other:?}"),
    }
    let pin = b64(b"135790");
    let answer = |extra: &str, decision: &str, with_pin: bool| {
        let pin = if with_pin {
            format!(r#","pin":"{pin}""#)
        } else {
            String::new()
        };
        req(
            Role::App,
            &frame_of(
                Operation::AnswerApproval,
                &format!(r#"{{"request":7,"decision":"{decision}"{pin}{extra}}}"#),
            ),
        )
    };
    match answer(r#","confirmed_entry":true"#, "once", true) {
        Ok(Request::AnswerApproval {
            confirmed_entry, ..
        }) => assert!(confirmed_entry),
        other => panic!("{other:?}"),
    }
    match answer("", "one_hour", true) {
        Ok(Request::AnswerApproval {
            confirmed_entry, ..
        }) => assert!(!confirmed_entry),
        other => panic!("{other:?}"),
    }
    assert!(answer(r#","confirmed_entry":false"#, "once", true).is_err());
    assert!(answer(r#","confirmed_entry":1"#, "once", true).is_err());
    assert!(answer(r#","confirmed_entry":"sim""#, "once", true).is_err());
    assert!(answer(r#","confirmed_entry":true"#, "deny", false).is_err());
    assert!(answer(r#","confirmed_entry":true"#, "confirm", false).is_err());
}

// ---- m2:DT8: protocol v2 (Phase 50 of mission 2; m2:V4.14, m2:V4.15) ----

/// The v2 field names that changed and the v1 name of each (m2:DT8, glossary section
/// 10), written here by hand. A v2 frame with the v1 name is an unknown field.
const V1_FIELD_NAMES: &[(&str, &str)] = &[
    ("data", "dados"),
    ("type", "tipo"),
    ("code", "codigo"),
    ("message", "mensagem"),
    ("problems", "problemas"),
    ("field", "campo"),
    ("position", "posicao"),
    ("reason", "motivo"),
    ("modified_at", "alterada_em"),
    ("environment", "ambiente"),
    ("attachments", "anexos"),
    ("previous", "anterior"),
    ("current", "atual"),
    ("external_backup", "backup_externo"),
    ("local_backup", "backup_local"),
    ("db_name", "base"),
    ("chain", "cadeia"),
    ("path", "caminho"),
    ("fields", "campos"),
    ("private_key", "chave_privada"),
    ("vault_exists", "cofre_existe"),
    ("cols", "colunas"),
    ("command", "comando"),
    ("verified", "conferida"),
    ("confirmation", "confirmacao"),
    ("content", "conteudo"),
    ("created_at", "criada_em"),
    ("decision", "decisao"),
    ("directory", "diretorio"),
    ("address", "endereco"),
    ("entry", "entrada"),
    ("confirmed_entry", "entrada_confirmada"),
    ("hidden_entry", "entrada_oculta"),
    ("entries", "entradas"),
    ("stream", "fluxo"),
    ("out_of_order", "fora_de_ordem"),
    ("group", "grupo"),
    ("items", "itens"),
    ("window_size", "janela"),
    ("enabled", "ligado"),
    ("rows", "linhas"),
    ("keep_attachments", "manter_anexos"),
    ("minutes", "minutos"),
    ("lock_minutes", "minutos_trava"),
    ("mounted", "montado"),
    ("engine", "motor"),
    ("level", "nivel"),
    ("name", "nome"),
    ("new", "novo"),
    ("new", "nova"),
    ("operation", "operacao"),
    ("origin", "origem"),
    ("role", "papel"),
    ("request", "pedido"),
    ("port", "porta"),
    ("project", "projeto"),
    ("protocol", "protocolo"),
    ("region", "regiao"),
    ("remaining", "restante"),
    ("result", "resultado"),
    ("summary", "resumo"),
    ("label", "rotulo"),
    ("jump", "salto"),
    ("secret", "segredo"),
    ("secrets", "segredos"),
    ("security", "seguranca"),
    ("password", "senha"),
    ("size", "tamanho"),
    ("text", "texto"),
    ("stdin_kind", "tipo_entrada"),
    ("totp_seed", "totp_semente"),
    ("locked", "travado"),
    ("tunnel", "tunel"),
    ("username", "usuario"),
    ("value", "valor"),
    ("variable", "variavel"),
    ("version", "versao"),
    ("versions", "versoes"),
];

/// The v1 operation names (m2:DT8), each with the v2 name it became.
const V1_OPERATIONS: &[(&str, &str)] = &[
    ("estado", "status"),
    ("destravar", "unlock"),
    ("travar", "lock"),
    ("lista", "list"),
    ("executar", "run"),
    ("entrada_padrao", "stdin"),
    ("janela", "resize"),
    ("criar_cofre", "create_vault"),
    ("detalhe", "detail"),
    ("revelar", "reveal"),
    ("copiar", "copy"),
    ("salvar_entrada", "save_entry"),
    ("remover_entrada", "remove_entry"),
    ("mudar_nivel", "set_level"),
    ("responder_aprovacao", "answer_approval"),
    ("emitir_token", "issue_token"),
    ("trocar_pin", "change_pin"),
    ("trocar_senha", "change_password"),
    ("tempo_trava", "lock_timeout"),
    ("restaurar_backup", "restore_backup"),
    ("configuracao", "config"),
    ("backup_externo", "external_backup"),
    ("definir_backup_externo", "set_external_backup"),
    ("pendrives", "usb_drives"),
    ("sugerir", "suggest"),
    ("preencher", "fill"),
    ("salvar_login", "save_login"),
    ("aguardar_login", "await_login"),
    ("preencher_login", "fill_login"),
    ("resultado_login", "login_result"),
    ("abrir_app", "open_app"),
];

/// The v1 response types that changed.
const V1_RESPONSE_TYPES: &[&str] = &[
    "estado",
    "erro",
    "lista",
    "saida",
    "fim",
    "sessao_playwright",
    "detalhe",
    "segredo",
    "pedido_de_aprovacao",
    "pedido_de_destrave",
    "codigo_totp",
    "sugestoes",
    "preenchimento",
    "pedido_de_login",
    "configuracao",
    "backup_externo",
    "pendrives",
];

/// m2:V4.14 (RM1, RA11): in a v2 frame, the v1 name of each field of each operation
/// (instead of the v2 one, or next to it) refuses the whole request with
/// `invalid_request`; nothing is read with a default.
#[test]
fn dt8_attack_v1_field_name_in_v2_request_refused() {
    let mut tested = 0;
    for &op in Operation::ALL {
        let role = role_of(op);
        let valid = valid_data(op);
        assert!(req(role, &frame_of(op, &valid)).is_ok(), "{}", op.name());
        for (new, old) in V1_FIELD_NAMES {
            let key = format!(r#""{new}":"#);
            if !valid.contains(&key) {
                continue;
            }
            let renamed = valid.replacen(&key, &format!(r#""{old}":"#), 1);
            assert_eq!(
                req(role, &frame_of(op, &renamed)).unwrap_err(),
                ResponseError::InvalidRequest,
                "{} with {old}",
                op.name()
            );
            let both = with_field(&valid, &format!(r#""{old}":1"#));
            assert_eq!(
                req(role, &frame_of(op, &both)).unwrap_err(),
                ResponseError::InvalidRequest,
                "{} with {old} next to {new}",
                op.name()
            );
            tested += 1;
        }
        // The v1 envelope keys in a v2 frame.
        for top in [
            format!(r#"{{"v":2,"op":"{}","dados":{valid}}}"#, op.name()),
            format!(
                r#"{{"v":2,"op":"{}","data":{valid},"dados":{{}}}}"#,
                op.name()
            ),
            format!(
                r#"{{"v":2,"op":"{}","tipo":"x","data":{valid}}}"#,
                op.name()
            ),
        ] {
            assert_eq!(
                req(role, &top).unwrap_err(),
                ResponseError::InvalidRequest,
                "{top}"
            );
        }
    }
    assert!(tested >= 40, "{tested}");
    // The terminal size of `ssh` by the v1 name.
    let ssh = |d: &str| req(Role::Agent, &frame_of(Operation::Ssh, d));
    assert!(
        ssh(r#"{"entry":"acme-ssh-dev","stdin_kind":"terminal","window_size":{"rows":24,"cols":80}}"#)
            .is_ok()
    );
    for bad in [
        r#"{"entry":"acme-ssh-dev","stdin_kind":"terminal","janela":{"rows":24,"cols":80}}"#,
        r#"{"entry":"acme-ssh-dev","stdin_kind":"terminal","window_size":{"linhas":24,"colunas":80}}"#,
    ] {
        assert_eq!(
            ssh(bad).unwrap_err(),
            ResponseError::InvalidRequest,
            "{bad}"
        );
    }
}

/// m2:V4.14: the v1 operation names are unknown operations for every role, also when
/// the role could ask for the v2 one, and with a v1 frame they answer the version
/// error first.
#[test]
fn dt8_attack_v1_operation_names_refused() {
    for (old, new) in V1_OPERATIONS {
        assert!(Operation::from_name(old).is_none(), "{old}");
        let op = Operation::from_name(new).expect("v2 name");
        let data = valid_data(op);
        for role in Role::ALL {
            let q = format!(r#"{{"v":2,"op":"{old}","data":{data}}}"#);
            assert_eq!(
                req(role, &q).unwrap_err(),
                ResponseError::UnknownOperation,
                "{q}"
            );
            let q = format!(r#"{{"v":1,"op":"{old}","dados":{{}}}}"#);
            assert_eq!(req(role, &q).unwrap_err(), ResponseError::UnknownVersion);
        }
    }
    for old in V1_RESPONSE_TYPES {
        assert!(ResponseType::from_name(old).is_none(), "{old}");
    }
}

/// m2:V4.14 (RA11): the v1 values are not read as the v2 ones: standard input kind
/// (the v1 name of `null` is not read as it, and a missing kind is never a terminal), decision, login
/// result, level, entry type and the reveal target field.
#[test]
fn dt8_attack_v1_values_refused() {
    let pin = b64(b"135790");
    let run = |kind: &str| {
        req(
            Role::Agent,
            &frame_of(
                Operation::Run,
                &format!(
                    r#"{{"entry":"acme-pg-dev","argv":["{}"],"cwd":"{}","environment":[]{kind}}}"#,
                    b64(b"psql"),
                    b64(b"/tmp")
                ),
            ),
        )
    };
    match run(r#","stdin_kind":"null""#) {
        Ok(Request::Run(r)) => assert_eq!(r.stdin_kind, StdinKind::Null),
        other => panic!("{other:?}"),
    }
    for kind in [
        "",
        r#","stdin_kind":"nula""#,
        r#","stdin_kind":"arquivo""#,
        r#","stdin_kind":"outra""#,
        r#","tipo_entrada":"terminal""#,
        r#","tipo_entrada":"null""#,
    ] {
        assert_eq!(
            run(kind).unwrap_err(),
            ResponseError::InvalidRequest,
            "{kind}"
        );
    }
    let mut cases: Vec<(Operation, Role, String)> = Vec::new();
    for d in ["negar", "uma_vez", "uma_hora", "sempre", "confirmar"] {
        cases.push((
            Operation::AnswerApproval,
            Role::App,
            format!(r#"{{"request":7,"decision":"{d}","pin":"{pin}"}}"#),
        ));
    }
    for r in [
        "enviado",
        "recusado",
        "sem_formulario",
        "aba_mudou",
        "falha",
    ] {
        cases.push((
            Operation::LoginResult,
            Role::Extension,
            format!(r#"{{"id":"AbCdEfGhIjKlMnOpQrStUvWxYz012345","result":"{r}"}}"#),
        ));
    }
    for l in ["bloqueado", "com-aprovacao", "livre"] {
        cases.push((
            Operation::SetLevel,
            Role::App,
            format!(r#"{{"entry":"acme-pg-dev","level":"{l}","pin":"{pin}"}}"#),
        ));
    }
    for f in ["senha", "totp_semente", "chave_privada", "texto"] {
        cases.push((
            Operation::Reveal,
            Role::App,
            format!(r#"{{"entry":"acme-wp-dev","field":"{f}"}}"#),
        ));
    }
    for t in ["banco", "servidor", "nota-segura", "generico"] {
        cases.push((
            Operation::SaveEntry,
            Role::App,
            format!(r#"{{"entry":{{"name":"x","group":"x","environment":"dev","type":"{t}"}}}}"#),
        ));
    }
    for (op, role, data) in &cases {
        assert_eq!(
            req(*role, &frame_of(*op, data)).unwrap_err(),
            ResponseError::InvalidRequest,
            "{data}"
        );
    }
}

/// m2:V4.14, client side: a daemon frame with a v1 name (envelope key, field key,
/// response type, error code, stream or role) is refused by the v2 reader of every
/// role; nothing is read with a default.
#[test]
fn dt8_attack_v1_names_in_v2_response_refused() {
    let mut responses = one_response_of_each();
    responses.push(Response::Output {
        stream: Stream::Stdout,
        data: zeroize::Zeroizing::new(b"ok".to_vec()),
    });
    let mut tested = 0;
    for r in &responses {
        let role = role_of_response(r.kind());
        let q = encode_response(role, r).unwrap();
        let s = String::from_utf8(q.to_vec()).unwrap();
        assert!(decode_response(role, s.as_bytes()).is_ok());
        for (new, old) in V1_FIELD_NAMES {
            let key = format!(r#""{new}":"#);
            if !s.contains(&key) {
                continue;
            }
            let renamed = s.replacen(&key, &format!(r#""{old}":"#), 1);
            for p in Role::ALL {
                assert!(
                    decode_response(p, renamed.as_bytes()).is_err(),
                    "{} with {old}",
                    r.kind().name()
                );
            }
            tested += 1;
        }
    }
    assert!(tested >= 60, "{tested}");
    let frames = [
        r#"{"v":2,"type":"estado","data":{"locked":true,"vault_exists":true,"role":"agent"}}"#,
        r#"{"v":2,"type":"status","data":{"locked":true,"vault_exists":true,"role":"agente"}}"#,
        r#"{"v":2,"type":"erro","data":{"code":"locked","message":"travado"}}"#,
        r#"{"v":2,"type":"error","data":{"code":"travado","message":"travado"}}"#,
        r#"{"v":2,"type":"error","data":{"code":"versao_desconhecida","message":"x"}}"#,
        r#"{"v":2,"type":"output","data":{"stream":"saida","data":"b2s="}}"#,
        r#"{"v":2,"type":"output","data":{"stream":"erro","data":"b2s="}}"#,
        r#"{"v":2,"type":"fim","data":{"code":0}}"#,
        r#"{"v":2,"tipo":"ok","data":{}}"#,
        r#"{"v":2,"type":"ok","dados":{}}"#,
    ];
    for f in frames {
        for p in Role::ALL {
            assert!(decode_response(p, f.as_bytes()).is_err(), "{f}");
        }
    }
    // Control: the same frames with the v2 names are read.
    for f in [
        r#"{"v":2,"type":"status","data":{"locked":true,"vault_exists":true,"role":"agent"}}"#,
        r#"{"v":2,"type":"error","data":{"code":"locked","message":"x"}}"#,
        r#"{"v":2,"type":"output","data":{"stream":"stdout","data":"b2s="}}"#,
        r#"{"v":2,"type":"exit","data":{"code":0}}"#,
    ] {
        assert!(decode_response(Role::Agent, f.as_bytes()).is_ok(), "{f}");
    }
}

/// The Portuguese words of the checker (`dev/english-check/pt-words.txt`, m2:DT17):
/// the stems, the exact words and the function words.
struct PtWords {
    stems: Vec<String>,
    words: std::collections::BTreeSet<String>,
}

fn pt_words() -> PtWords {
    let text = include_str!("../../../../dev/english-check/pt-words.txt");
    let mut w = PtWords {
        stems: Vec::new(),
        words: Default::default(),
    };
    let mut section = "";
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = match line {
                "[stems]" => "stems",
                "[words]" | "[function]" => "words",
                _ => "",
            };
            continue;
        }
        match section {
            "stems" => w.stems.push(line.to_lowercase()),
            "words" => {
                w.words.insert(line.to_lowercase());
            }
            _ => {}
        }
    }
    assert!(
        w.stems.len() > 100 && w.words.len() > 1000,
        "word list not read"
    );
    w
}

/// Does a wire name have a Portuguese word, by the rules of the checker for an
/// identifier (split at `_` and `-`; a stem, an exact word or a function word)?
fn pt_hit(w: &PtWords, name: &str) -> bool {
    if !name.is_ascii() {
        return true;
    }
    name.split(['_', '-'])
        .filter(|p| !p.is_empty())
        .map(|p| p.to_ascii_lowercase())
        .any(|p| w.words.contains(&p) || w.stems.iter().any(|s| p.starts_with(s.as_str())))
}

/// Collects the keys of a JSON value and the values that are names (operation, type,
/// code, role, kind, decision, result, stream, level, reveal target, secret fields).
/// The validation problems carry Portuguese texts on purpose (field label and reason,
/// shown to the user): only their keys are names.
fn wire_names(v: &super::json::Value, parent: &str, out: &mut BTreeSet<String>) {
    use super::json::Value;
    const NAME_VALUES: &[&str] = &[
        "op",
        "type",
        "code",
        "role",
        "stdin_kind",
        "decision",
        "result",
        "stream",
        "operation",
        "level",
        "field",
        "security",
        "engine",
        "protocol",
        "environment",
    ];
    match v {
        Value::Object(pairs) => {
            for (k, x) in pairs {
                out.insert(k.clone());
                if parent == "problems" {
                    continue;
                }
                match x {
                    Value::Text(t) if NAME_VALUES.contains(&k.as_str()) => {
                        // The TOTP code and the exit code are numbers, not names.
                        if !t.bytes().all(|b| b.is_ascii_digit()) {
                            out.insert(t.to_string());
                        }
                    }
                    Value::List(l) if k == "secrets" => {
                        for i in l {
                            if let Value::Text(t) = i {
                                out.insert(t.to_string());
                            }
                        }
                    }
                    _ => wire_names(x, k, out),
                }
            }
        }
        Value::List(l) => {
            for i in l {
                wire_names(i, parent, out);
            }
        }
        _ => {}
    }
}

use std::collections::BTreeSet;

/// m2:V4.15 (S1, S9): every name of the protocol v2 (operations, response types, error
/// codes, roles, value names, field identifiers and every key of a frame of each
/// request and response) is English: none has a word of the checker's Portuguese list.
/// The v1 names are the control: the same rule flags them.
#[test]
fn dt8_no_portuguese_name_in_protocol_v2() {
    let w = pt_words();
    let mut names: BTreeSet<String> = BTreeSet::new();
    names.extend(Operation::ALL.iter().map(|o| o.name().to_owned()));
    names.extend(ResponseType::ALL.iter().map(|t| t.name().to_owned()));
    names.extend(ResponseError::SIMPLE.iter().map(|e| e.code().to_owned()));
    names.insert(ResponseError::Validation(Vec::new()).code().to_owned());
    names.extend(Role::ALL.iter().map(|r| r.name().to_owned()));
    names.extend(StdinKind::ALL.iter().map(|k| k.name().to_owned()));
    names.extend(LoginResult::ALL.iter().map(|r| r.name().to_owned()));
    names.extend(
        [
            Decision::Deny,
            Decision::AllowOnce,
            Decision::AllowOneHour,
            Decision::AllowAlways,
            Decision::Confirm,
        ]
        .iter()
        .map(|d| d.name().to_owned()),
    );
    names.extend(
        super::request::SECRET_FIELDS
            .iter()
            .map(|f| (*f).to_owned()),
    );
    names.extend(super::response::TEXT_FIELDS.iter().map(|f| (*f).to_owned()));
    let mut frames: Vec<Vec<u8>> = Vec::new();
    for (op, r) in Operation::ALL.iter().zip(one_request_of_each()) {
        frames.push(encode_request(role_of(*op), &r).unwrap().to_vec());
    }
    let mut responses = one_response_of_each();
    responses.push(Response::Output {
        stream: Stream::Stdout,
        data: zeroize::Zeroizing::new(b"ok".to_vec()),
    });
    responses.push(Response::Error(ResponseError::UnknownVersion));
    for r in &responses {
        frames.push(
            encode_response(role_of_response(r.kind()), r)
                .unwrap()
                .to_vec(),
        );
    }
    for f in &frames {
        let v = super::json::read(f, super::json::MAX_RESPONSE_NODES).unwrap();
        wire_names(&v, "", &mut names);
    }
    assert!(names.len() > 150, "{}", names.len());
    let hits: Vec<_> = names.iter().filter(|n| pt_hit(&w, n)).collect();
    assert!(
        hits.is_empty(),
        "Portuguese names in the protocol: {hits:?}"
    );

    // Control: the rule flags the v1 names (operations, keys, codes and roles).
    for (old, _) in V1_OPERATIONS {
        assert!(pt_hit(&w, old), "{old}");
    }
    for old in [
        "entrada",
        "senha",
        "tipo_entrada",
        "dados",
        "codigo",
        "mensagem",
        "nao_autorizado",
        "versao_desconhecida",
        "agente",
        "extensao",
        "pedido_de_aprovacao",
        "preenchimento",
    ] {
        assert!(pt_hit(&w, old), "{old}");
    }
}

/// m2:V4.15 (S1, S9): the operation × role matrix of v2 is the one of v1, name by name
/// through the table of m2:DT8. The v1 matrix is written here by hand as 0.1.4 had it.
#[test]
fn dt8_role_matrix_same_as_v1() {
    // (v1 name, agent, app, extension), as `protocol.rs` of 0.1.4.
    let v1: &[(&str, bool, bool, bool)] = &[
        ("estado", true, true, true),
        ("destravar", true, true, false),
        ("travar", true, true, false),
        ("lista", true, true, false),
        ("executar", true, false, false),
        ("ssh", true, false, false),
        ("login", true, false, false),
        ("playwright", true, false, false),
        ("entrada_padrao", true, false, false),
        ("janela", true, false, false),
        ("criar_cofre", false, true, false),
        ("detalhe", false, true, false),
        ("revelar", false, true, false),
        ("copiar", false, true, false),
        ("totp", false, true, false),
        ("salvar_entrada", false, true, false),
        ("remover_entrada", false, true, false),
        ("mudar_nivel", false, true, false),
        ("responder_aprovacao", false, true, false),
        ("emitir_token", false, true, false),
        ("trocar_pin", false, true, false),
        ("trocar_senha", false, true, false),
        ("tempo_trava", false, true, false),
        ("backups", false, true, false),
        ("restaurar_backup", false, true, false),
        ("configuracao", false, true, false),
        ("backup_externo", false, true, false),
        ("definir_backup_externo", false, true, false),
        ("pendrives", false, true, false),
        ("sugerir", false, false, true),
        ("preencher", false, false, true),
        ("salvar_login", false, false, true),
        ("aguardar_login", false, false, true),
        ("preencher_login", false, false, true),
        ("resultado_login", false, false, true),
        ("abrir_app", true, true, true),
    ];
    assert_eq!(v1.len(), Operation::ALL.len());
    for (old, agent, app, extension) in v1 {
        let new = V1_OPERATIONS
            .iter()
            .find(|(o, _)| o == old)
            .map_or(*old, |(_, n)| *n);
        let op = Operation::from_name(new).unwrap_or_else(|| panic!("{old} -> {new}"));
        let r = op.roles();
        assert_eq!(
            (r.agent, r.app, r.extension),
            (*agent, *app, *extension),
            "{old} -> {new}"
        );
    }
    // The agent never asks for these (the examples of m2:V4.15).
    for name in [
        "reveal",
        "copy",
        "detail",
        "fill",
        "suggest",
        "set_level",
        "restore_backup",
        "answer_approval",
        "issue_token",
        "totp",
    ] {
        assert!(
            !Operation::from_name(name).unwrap().allowed(Role::Agent),
            "{name}"
        );
    }
}
