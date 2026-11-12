//! Tests of the host with an in-memory fake daemon (fake values only).

use super::*;
use ward_core::entry::Secret;
use ward_core::protocol::Suggestion;

const FAKE_PASSWORD: &str = "falso-WP-senha-01";

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                s.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

fn contains(p: &[u8], a: &[u8]) -> bool {
    p.windows(a.len()).any(|w| w == a)
}

/// Fake daemon: keeps the requests and answers what the test tells it, in the given
/// role (`raw`: raw body).
struct FakeDaemon {
    requests: Vec<Vec<u8>>,
    reply: Box<dyn FnMut() -> Result<Vec<u8>, DaemonFailure>>,
}

impl FakeDaemon {
    fn replying(r: Response) -> FakeDaemon {
        let body = protocol::encode_response(Role::App, &r)
            .or_else(|_| protocol::encode_response(Role::Extension, &r))
            .or_else(|_| protocol::encode_response(Role::Agent, &r))
            .unwrap()
            .to_vec();
        FakeDaemon::raw(body)
    }

    fn raw(body: Vec<u8>) -> FakeDaemon {
        FakeDaemon {
            requests: Vec::new(),
            reply: Box::new(move || Ok(body.clone())),
        }
    }
}

impl Daemon for FakeDaemon {
    fn exchange(&mut self, request: &[u8]) -> Result<Zeroizing<Vec<u8>>, DaemonFailure> {
        self.requests.push(request.to_vec());
        (self.reply)().map(Zeroizing::new)
    }
}

fn handle_with(m: &str, d: &mut FakeDaemon) -> String {
    let mut logs = Vec::new();
    let r = handle(m.as_bytes(), d, &mut |l| logs.push(l.to_owned()));
    for l in &logs {
        assert!(!l.contains("falso-"), "log com conteúdo: {l}");
    }
    String::from_utf8(r.to_vec()).unwrap()
}

fn fill_response() -> Response {
    Response::Fill {
        username: "wp_admin".into(),
        password: Secret::from_bytes(FAKE_PASSWORD.as_bytes().to_vec()),
        totp: None,
    }
}

const FILL: &str =
    r#"{"v":2,"op":"fill","data":{"entry":"acme-wp-dev","origin":"http://localhost:8081"}}"#;

#[test]
fn operations_are_the_extensions() {
    for op in Operation::ALL {
        assert_eq!(
            OPERATIONS.contains(op),
            op.allowed(Role::Extension),
            "{}",
            op.name()
        );
    }
}

#[test]
fn extension_origin() {
    assert!(valid_extension_origin(
        b"chrome-extension://nhkehjnmlgkebhmelajfpmpblepaichf/"
    ));
    for bad in [
        &b"chrome-extension://nhkehjnmlgkebhmelajfpmpblepaichf"[..],
        b"chrome-extension://nhkehjnmlgkebhmelajfpmpblepaichfq/",
        b"chrome-extension://dkacdineofgfabdognjgeoakekaenaez/",
        b"chrome-extension://NHKEHJNMLGKEBHMELAJFPMPBLEPAICHF/",
        b"chrome-extension://nhkehjnmlgkebhmelajfpmpblepaichf/x",
        // Another extension with a well-formed id (the one a manifest in the user's
        // profile would point at `ward-nm`) does not pass: only Ward's id.
        b"chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/",
        b"chrome-extension://dkacdineofgfabdognjgeoakekaenaei/",
        b"moz-extension://nhkehjnmlgkebhmelajfpmpblepaichf/",
        b"https://localhost:8081/",
        b"",
        b"--parent-window=0",
    ] {
        assert!(!valid_extension_origin(bad), "{bad:?}");
    }
}

#[test]
fn fill_goes_through_the_daemon_and_comes_back_canonical() {
    let mut d = FakeDaemon::replying(fill_response());
    let r = handle_with(FILL, &mut d);
    assert!(r.contains(r#""type":"fill""#), "{r}");
    assert!(r.contains(&b64(FAKE_PASSWORD.as_bytes())));
    // The request to the daemon is the canonical form of the protocol, in the extension
    // role.
    assert_eq!(d.requests.len(), 1);
    let p = protocol::decode_request(Role::Extension, &d.requests[0]).unwrap();
    assert_eq!(p.operation(), Operation::Fill);
}

/// m2:Phase 66b (decision in the `protocol` module doc): a message without `data` or
/// with escapes in the names reaches the daemon only as the canonical bytes, and an app
/// or agent operation spelled with escapes is still refused here.
#[test]
fn attack_escaped_names_and_missing_data_reach_the_daemon_canonical() {
    for m in [
        r#"{"v":2,"op":"open_app"}"#,
        r#"{"v":2,"op":"open_app"}"#,
        r#"{"v":2,"op":"open_app","data":{}}"#,
    ] {
        let mut d = FakeDaemon::replying(Response::Ok);
        let r = handle_with(m, &mut d);
        assert!(r.contains(r#""type":"ok""#), "{m} -> {r}");
        assert_eq!(
            d.requests,
            vec![br#"{"v":2,"op":"open_app","data":{}}"#.to_vec()],
            "{m}"
        );
    }
    for m in [
        r#"{"v":2,"op":"reveal","data":{"entry":"acme-wp-dev","field":"password"}}"#,
        r#"{"v":2,"op":"list"}"#,
        r#"{"v":2,"op":"run","data":{}}"#,
    ] {
        let mut d = FakeDaemon::replying(fill_response());
        let r = handle_with(m, &mut d);
        assert!(r.contains(r#""code":"unauthorized""#), "{m} -> {r}");
        assert!(d.requests.is_empty(), "{m} chegou ao daemon");
    }
}

#[test]
fn attack_app_and_agent_operations_do_not_reach_the_daemon() {
    for m in [
        r#"{"v":2,"op":"reveal","data":{"entry":"acme-wp-dev","field":"password"}}"#,
        r#"{"v":2,"op":"copy","data":{"entry":"acme-wp-dev","field":"password"}}"#,
        r#"{"v":2,"op":"totp","data":{"entry":"acme-totp-dev"}}"#,
        r#"{"v":2,"op":"list"}"#,
        r#"{"v":2,"op":"detail","data":{"entry":"acme-wp-dev"}}"#,
        r#"{"v":2,"op":"set_level","data":{"entry":"acme-wp-dev","level":"free","pin":"MTM1Nzkw"}}"#,
        r#"{"v":2,"op":"answer_approval","data":{}}"#,
        r#"{"v":2,"op":"unlock","data":{"password":"eA=="}}"#,
        r#"{"v":2,"op":"lock"}"#,
        r#"{"v":2,"op":"run","data":{}}"#,
        r#"{"v":2,"op":"login","data":{"entry":"acme-wp-dev"}}"#,
        r#"{"v":2,"op":"issue_token","data":{"entry":"acme-wp-dev"}}"#,
        r#"{"v":2,"op":"restore_backup","data":{}}"#,
    ] {
        let mut d = FakeDaemon::replying(fill_response());
        let r = handle_with(m, &mut d);
        assert!(r.contains(r#""code":"unauthorized""#), "{m} -> {r}");
        assert!(d.requests.is_empty(), "{m} chegou ao daemon");
    }
}

#[test]
fn attack_malformed_json_and_extra_fields() {
    for (m, code) in [
        ("", "invalid_request"),
        ("{", "invalid_request"),
        ("null", "invalid_request"),
        (r#"[1,2]"#, "invalid_request"),
        (r#"{"v":2}"#, "invalid_request"),
        // m2:DT8: a v1 (0.1.4) or a v3 message gets the version error.
        (r#"{"v":1,"op":"estado"}"#, "unknown_version"),
        (
            r#"{"v":1,"op":"preencher","dados":{"entrada":"acme-wp-dev","origem":"http://localhost:8081"}}"#,
            "unknown_version",
        ),
        (r#"{"v":3,"op":"status"}"#, "unknown_version"),
        (r#"{"v":2,"op":"inventada"}"#, "unknown_operation"),
        (r#"{"v":2,"op":"status","op":"reveal"}"#, "invalid_request"),
        (
            r#"{"v":2,"op":"fill","data":{"entry":"acme-wp-dev","origin":"http://localhost:8081","field":"password"}}"#,
            "invalid_request",
        ),
        (
            r#"{"v":2,"op":"fill","revelar":true,"data":{"entry":"acme-wp-dev","origin":"http://localhost:8081"}}"#,
            "invalid_request",
        ),
    ] {
        let mut d = FakeDaemon::replying(fill_response());
        let r = handle_with(m, &mut d);
        assert!(r.contains(&format!(r#""code":"{code}""#)), "{m} -> {r}");
        assert!(d.requests.is_empty(), "{m} chegou ao daemon");
    }
    // Origin above the URL limit.
    let m = format!(
        r#"{{"v":2,"op":"suggest","data":{{"origin":"http://localhost:8081/{}"}}}}"#,
        "a".repeat(4096)
    );
    let mut d = FakeDaemon::replying(fill_response());
    assert!(handle_with(&m, &mut d).contains("invalid_request"));
    assert!(d.requests.is_empty());
    // Bytes that are not UTF-8.
    let mut d = FakeDaemon::replying(fill_response());
    let r = handle(b"{\"v\":2,\"op\":\"estado\xff\"}", &mut d, &mut |_| {});
    assert!(contains(&r, b"invalid_request") || contains(&r, b"unknown_operation"));
    assert!(d.requests.is_empty());
}

#[test]
fn attack_daemon_response_outside_the_role_or_the_operation_does_not_go_out() {
    // A daemon (or a corrupted frame) that sent an app secret, a token or a fill for
    // another operation: none of it reaches the extension.
    let cases: Vec<(&str, Response)> = vec![
        (
            FILL,
            Response::Secret(Secret::from_bytes(FAKE_PASSWORD.as_bytes().to_vec())),
        ),
        (
            FILL,
            Response::Token(Secret::from_bytes(FAKE_PASSWORD.as_bytes().to_vec())),
        ),
        (
            r#"{"v":2,"op":"suggest","data":{"origin":"http://localhost:8081"}}"#,
            fill_response(),
        ),
        (r#"{"v":2,"op":"status"}"#, fill_response()),
        (r#"{"v":2,"op":"open_app"}"#, fill_response()),
        (FILL, Response::List(Vec::new())),
        // Phase 43: the login wait and the login result never carry a value; only the
        // login fill gets the fill.
        (r#"{"v":2,"op":"await_login"}"#, fill_response()),
        (
            r#"{"v":2,"op":"login_result","data":{"id":"AbCdEfGhIjKlMnOpQrStUvWxYz012345","result":"submitted"}}"#,
            fill_response(),
        ),
        (
            r#"{"v":2,"op":"fill_login","data":{"id":"AbCdEfGhIjKlMnOpQrStUvWxYz012345","origin":"http://localhost:8081"}}"#,
            Response::Secret(Secret::from_bytes(FAKE_PASSWORD.as_bytes().to_vec())),
        ),
        (
            FILL,
            Response::Suggestions(vec![Suggestion {
                name: "x".into(),
                username: "y".into(),
                environment: ward_core::entry::Environment::Dev,
                totp: false,
            }]),
        ),
    ];
    for (m, r) in cases {
        let mut d = FakeDaemon::replying(r);
        let s = handle_with(m, &mut d);
        assert!(s.contains(r#""code":"internal""#), "{m} -> {s}");
        assert!(!s.contains(&b64(FAKE_PASSWORD.as_bytes())), "{s}");
        assert!(!s.contains(FAKE_PASSWORD), "{s}");
    }
    // Raw body that is not a response.
    for raw in [
        &b"lixo"[..],
        b"{}",
        br#"{"v":2,"type":"secret","data":{"value":"eA=="}}"#,
    ] {
        let mut d = FakeDaemon::raw(raw.to_vec());
        assert!(handle_with(FILL, &mut d).contains("internal"));
    }
    // Daemon down.
    let mut d = FakeDaemon {
        requests: Vec::new(),
        reply: Box::new(|| Err(DaemonFailure::Unavailable)),
    };
    assert!(handle_with(FILL, &mut d).contains(r#""code":"internal""#));
}

/// Attack m2:DT8: a daemon that answers in the 0.1.4 protocol (v1), even with a fill
/// with the password: the extension gets only the version error, never the frame.
#[test]
fn attack_m2_daemon_of_another_version_gives_only_the_version_error() {
    let password = b64(FAKE_PASSWORD.as_bytes());
    for raw in [
        format!(
            r#"{{"v":1,"tipo":"preenchimento","dados":{{"usuario":"admin","senha":"{password}"}}}}"#
        ),
        r#"{"v":1,"tipo":"erro","dados":{"codigo":"interno","mensagem":"erro interno"}}"#
            .to_owned(),
        format!(r#"{{"v":3,"type":"fill","data":{{"username":"admin","password":"{password}"}}}}"#),
    ] {
        let mut d = FakeDaemon::raw(raw.clone().into_bytes());
        let s = handle_with(FILL, &mut d);
        assert!(s.contains(r#""code":"unknown_version""#), "{raw} -> {s}");
        assert!(!s.contains(&password) && !s.contains(FAKE_PASSWORD), "{s}");
        assert!(!s.contains("admin"), "{s}");
        assert_eq!(d.requests.len(), 1);
    }
}

fn frame(body: &[u8]) -> Vec<u8> {
    let mut v = (body.len() as u32).to_ne_bytes().to_vec();
    v.extend_from_slice(body);
    v
}

fn messages(mut output: &[u8]) -> Vec<String> {
    let mut v = Vec::new();
    while !output.is_empty() {
        let n = u32::from_ne_bytes(output[..4].try_into().unwrap()) as usize;
        v.push(String::from_utf8(output[4..4 + n].to_vec()).unwrap());
        output = &output[4 + n..];
    }
    v
}

#[test]
fn state_without_the_extension_role_becomes_unauthorized() {
    for role in [Role::Agent, Role::App] {
        let mut d = FakeDaemon::raw(
            format!(r#"{{"v":2,"type":"status","data":{{"locked":false,"vault_exists":true,"role":"{}"}}}}"#, role.name())
                .into_bytes(),
        );
        let r = handle_with(r#"{"v":2,"op":"status"}"#, &mut d);
        assert!(r.contains(r#""code":"unauthorized""#), "{r}");
    }
}

#[test]
fn loop_many_messages_and_normal_end() {
    let mut input = frame(br#"{"v":2,"op":"status"}"#);
    input.extend(frame(FILL.as_bytes()));
    let mut d = FakeDaemon::replying(Response::Status {
        locked: false,
        vault_exists: true,
        role: Role::Extension,
    });
    let mut output = Vec::new();
    serve(&mut &input[..], &mut output, &mut d, &mut |_| {}).unwrap();
    let m = messages(&output);
    assert_eq!(m.len(), 2);
    assert!(m[0].contains(r#""type":"status""#));
    // A state response does not fit as the response to a fill.
    assert!(m[1].contains(r#""code":"internal""#));
}

#[test]
fn attack_message_over_the_limit_empty_or_truncated() {
    // Over the limit: the body is not even read; answers invalid frame and exits with an
    // error.
    let mut input = ((MAX_MESSAGE_IN + 1) as u32).to_ne_bytes().to_vec();
    input.extend(vec![b' '; 16]);
    let mut d = FakeDaemon::replying(fill_response());
    let mut output = Vec::new();
    let r = serve(&mut &input[..], &mut output, &mut d, &mut |_| {});
    assert_eq!(r, Err(MessageError::TooLarge));
    assert!(messages(&output)[0].contains("invalid_frame"));
    assert!(d.requests.is_empty());

    // 4 GiB declared.
    let input = u32::MAX.to_ne_bytes();
    let mut output = Vec::new();
    let r = serve(&mut &input[..], &mut output, &mut d, &mut |_| {});
    assert_eq!(r, Err(MessageError::TooLarge));

    // Empty.
    let input = 0u32.to_ne_bytes();
    let mut output = Vec::new();
    assert_eq!(
        serve(&mut &input[..], &mut output, &mut d, &mut |_| {}),
        Err(MessageError::Empty)
    );

    // Truncated in the size and in the body: nothing is answered.
    for input in [&[1u8, 0][..], &frame(FILL.as_bytes())[..20]] {
        let mut output = Vec::new();
        assert_eq!(
            serve(&mut &input[..], &mut output, &mut d, &mut |_| {}),
            Err(MessageError::Truncated)
        );
        assert!(output.is_empty());
    }
    assert!(d.requests.is_empty());

    // Exactly at the limit: read and refused by the protocol (it is not JSON).
    let input = frame(&vec![b' '; MAX_MESSAGE_IN]);
    let mut output = Vec::new();
    serve(&mut &input[..], &mut output, &mut d, &mut |_| {}).unwrap();
    assert!(messages(&output)[0].contains("invalid_request"));
}

#[test]
fn write_refuses_over_the_chrome_limit() {
    let mut output = Vec::new();
    assert!(write_message(&mut output, &vec![b'x'; MAX_MESSAGE_OUT + 1]).is_err());
    assert!(write_message(&mut output, b"").is_err());
    assert!(output.is_empty());
}
