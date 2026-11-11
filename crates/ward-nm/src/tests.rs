//! Tests of the host with an in-memory fake daemon (fake values only).

use super::*;
use ward_core::entry::Secret;
use ward_core::protocol::Sugestao;

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
    fn replying(r: Resposta) -> FakeDaemon {
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

fn fill_response() -> Resposta {
    Resposta::Preenchimento {
        usuario: "wp_admin".into(),
        senha: Secret::from_bytes(FAKE_PASSWORD.as_bytes().to_vec()),
        totp: None,
    }
}

const FILL: &str = r#"{"v":1,"op":"preencher","dados":{"entrada":"acme-wp-dev","origem":"http://localhost:8081"}}"#;

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
    assert!(r.contains(r#""tipo":"preenchimento""#), "{r}");
    assert!(r.contains(&b64(FAKE_PASSWORD.as_bytes())));
    // The request to the daemon is the canonical form of the protocol, in the extension
    // role.
    assert_eq!(d.requests.len(), 1);
    let p = protocol::decode_request(Role::Extension, &d.requests[0]).unwrap();
    assert_eq!(p.operation(), Operation::Fill);
}

#[test]
fn attack_app_and_agent_operations_do_not_reach_the_daemon() {
    for m in [
        r#"{"v":1,"op":"revelar","dados":{"entrada":"acme-wp-dev","campo":"senha"}}"#,
        r#"{"v":1,"op":"copiar","dados":{"entrada":"acme-wp-dev","campo":"senha"}}"#,
        r#"{"v":1,"op":"totp","dados":{"entrada":"acme-totp-dev"}}"#,
        r#"{"v":1,"op":"lista"}"#,
        r#"{"v":1,"op":"detalhe","dados":{"entrada":"acme-wp-dev"}}"#,
        r#"{"v":1,"op":"mudar_nivel","dados":{"entrada":"acme-wp-dev","nivel":"livre","pin":"MTM1Nzkw"}}"#,
        r#"{"v":1,"op":"responder_aprovacao","dados":{}}"#,
        r#"{"v":1,"op":"destravar","dados":{"senha":"eA=="}}"#,
        r#"{"v":1,"op":"travar"}"#,
        r#"{"v":1,"op":"executar","dados":{}}"#,
        r#"{"v":1,"op":"login","dados":{"entrada":"acme-wp-dev"}}"#,
        r#"{"v":1,"op":"emitir_token","dados":{"entrada":"acme-wp-dev"}}"#,
        r#"{"v":1,"op":"restaurar_backup","dados":{}}"#,
    ] {
        let mut d = FakeDaemon::replying(fill_response());
        let r = handle_with(m, &mut d);
        assert!(r.contains(r#""codigo":"nao_autorizado""#), "{m} -> {r}");
        assert!(d.requests.is_empty(), "{m} chegou ao daemon");
    }
}

#[test]
fn attack_malformed_json_and_extra_fields() {
    for (m, code) in [
        ("", "pedido_invalido"),
        ("{", "pedido_invalido"),
        ("null", "pedido_invalido"),
        (r#"[1,2]"#, "pedido_invalido"),
        (r#"{"v":1}"#, "pedido_invalido"),
        (r#"{"v":2,"op":"estado"}"#, "versao_desconhecida"),
        (r#"{"v":1,"op":"inventada"}"#, "operacao_desconhecida"),
        (r#"{"v":1,"op":"estado","op":"revelar"}"#, "pedido_invalido"),
        (
            r#"{"v":1,"op":"preencher","dados":{"entrada":"acme-wp-dev","origem":"http://localhost:8081","campo":"senha"}}"#,
            "pedido_invalido",
        ),
        (
            r#"{"v":1,"op":"preencher","revelar":true,"dados":{"entrada":"acme-wp-dev","origem":"http://localhost:8081"}}"#,
            "pedido_invalido",
        ),
    ] {
        let mut d = FakeDaemon::replying(fill_response());
        let r = handle_with(m, &mut d);
        assert!(r.contains(&format!(r#""codigo":"{code}""#)), "{m} -> {r}");
        assert!(d.requests.is_empty(), "{m} chegou ao daemon");
    }
    // Origin above the URL limit.
    let m = format!(
        r#"{{"v":1,"op":"sugerir","dados":{{"origem":"http://localhost:8081/{}"}}}}"#,
        "a".repeat(4096)
    );
    let mut d = FakeDaemon::replying(fill_response());
    assert!(handle_with(&m, &mut d).contains("pedido_invalido"));
    assert!(d.requests.is_empty());
    // Bytes that are not UTF-8.
    let mut d = FakeDaemon::replying(fill_response());
    let r = handle(b"{\"v\":1,\"op\":\"estado\xff\"}", &mut d, &mut |_| {});
    assert!(contains(&r, b"pedido_invalido") || contains(&r, b"operacao_desconhecida"));
    assert!(d.requests.is_empty());
}

#[test]
fn attack_daemon_response_outside_the_role_or_the_operation_does_not_go_out() {
    // A daemon (or a corrupted frame) that sent an app secret, a token or a fill for
    // another operation: none of it reaches the extension.
    let cases: Vec<(&str, Resposta)> = vec![
        (
            FILL,
            Resposta::Segredo(Secret::from_bytes(FAKE_PASSWORD.as_bytes().to_vec())),
        ),
        (
            FILL,
            Resposta::Token(Secret::from_bytes(FAKE_PASSWORD.as_bytes().to_vec())),
        ),
        (
            r#"{"v":1,"op":"sugerir","dados":{"origem":"http://localhost:8081"}}"#,
            fill_response(),
        ),
        (r#"{"v":1,"op":"estado"}"#, fill_response()),
        (r#"{"v":1,"op":"abrir_app"}"#, fill_response()),
        (FILL, Resposta::Lista(Vec::new())),
        // Phase 43: the login wait and the login result never carry a value; only the
        // login fill gets the fill.
        (r#"{"v":1,"op":"aguardar_login"}"#, fill_response()),
        (
            r#"{"v":1,"op":"resultado_login","dados":{"id":"AbCdEfGhIjKlMnOpQrStUvWxYz012345","resultado":"enviado"}}"#,
            fill_response(),
        ),
        (
            r#"{"v":1,"op":"preencher_login","dados":{"id":"AbCdEfGhIjKlMnOpQrStUvWxYz012345","origem":"http://localhost:8081"}}"#,
            Resposta::Segredo(Secret::from_bytes(FAKE_PASSWORD.as_bytes().to_vec())),
        ),
        (
            FILL,
            Resposta::Sugestoes(vec![Sugestao {
                nome: "x".into(),
                usuario: "y".into(),
                ambiente: ward_core::entry::Environment::Dev,
                totp: false,
            }]),
        ),
    ];
    for (m, r) in cases {
        let mut d = FakeDaemon::replying(r);
        let s = handle_with(m, &mut d);
        assert!(s.contains(r#""codigo":"interno""#), "{m} -> {s}");
        assert!(!s.contains(&b64(FAKE_PASSWORD.as_bytes())), "{s}");
        assert!(!s.contains(FAKE_PASSWORD), "{s}");
    }
    // Raw body that is not a response.
    for raw in [
        &b"lixo"[..],
        b"{}",
        br#"{"v":1,"tipo":"segredo","dados":{"valor":"eA=="}}"#,
    ] {
        let mut d = FakeDaemon::raw(raw.to_vec());
        assert!(handle_with(FILL, &mut d).contains("interno"));
    }
    // Daemon down.
    let mut d = FakeDaemon {
        requests: Vec::new(),
        reply: Box::new(|| Err(DaemonFailure::Unavailable)),
    };
    assert!(handle_with(FILL, &mut d).contains(r#""codigo":"interno""#));
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
            format!(r#"{{"v":1,"tipo":"estado","dados":{{"travado":false,"cofre_existe":true,"papel":"{}"}}}}"#, role.name())
                .into_bytes(),
        );
        let r = handle_with(r#"{"v":1,"op":"estado"}"#, &mut d);
        assert!(r.contains(r#""codigo":"nao_autorizado""#), "{r}");
    }
}

#[test]
fn loop_many_messages_and_normal_end() {
    let mut input = frame(br#"{"v":1,"op":"estado"}"#);
    input.extend(frame(FILL.as_bytes()));
    let mut d = FakeDaemon::replying(Resposta::Estado {
        travado: false,
        cofre_existe: true,
        papel: Role::Extension,
    });
    let mut output = Vec::new();
    serve(&mut &input[..], &mut output, &mut d, &mut |_| {}).unwrap();
    let m = messages(&output);
    assert_eq!(m.len(), 2);
    assert!(m[0].contains(r#""tipo":"estado""#));
    // A state response does not fit as the response to a fill.
    assert!(m[1].contains(r#""codigo":"interno""#));
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
    assert!(messages(&output)[0].contains("quadro_invalido"));
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
    assert!(messages(&output)[0].contains("pedido_invalido"));
}

#[test]
fn write_refuses_over_the_chrome_limit() {
    let mut output = Vec::new();
    assert!(write_message(&mut output, &vec![b'x'; MAX_MESSAGE_OUT + 1]).is_err());
    assert!(write_message(&mut output, b"").is_err());
    assert!(output.is_empty());
}
