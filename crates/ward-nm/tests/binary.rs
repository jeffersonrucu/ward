//! The `ward-nm` binary executed as any process of the user (Phase 40, V6.1): argument
//! out of form, messages above the limit, malformed and without a daemon. No secret
//! exists here; what is checked is that nothing goes out on stdout besides the error
//! responses of the protocol.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const ORIGIN: &str = "chrome-extension://nhkehjnmlgkebhmelajfpmpblepaichf/";

fn empty_dir() -> PathBuf {
    let p =
        PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("ward-nm-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&p);
    p
}

fn run_host(args: &[&str], input: &[u8]) -> (Option<i32>, Vec<u8>, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ward-nm"))
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("WARD_DEV_SOCKET_DIR", empty_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut i = c.stdin.take().unwrap();
    let _ = i.write_all(input);
    drop(i);
    let o = c.wait_with_output().unwrap();
    (
        o.status.code(),
        o.stdout,
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn frame(body: &[u8]) -> Vec<u8> {
    let mut v = (body.len() as u32).to_ne_bytes().to_vec();
    v.extend_from_slice(body);
    v
}

#[test]
fn attack_arguments_out_of_form_exit_with_nothing_on_stdout() {
    let m = frame(br#"{"v":2,"op":"status"}"#);
    for args in [
        &[][..],
        &["chrome-extension://nhkehjnmlgkebhmelajfpmpblepaichf"],
        &[ORIGIN, "--parent-window=0"],
        &["https://localhost:8081/"],
        &["--help"],
    ] {
        let (code, output, _) = run_host(args, &m);
        assert_eq!(code, Some(2), "{args:?}");
        assert!(output.is_empty(), "{args:?}");
    }
}

#[test]
fn attack_message_too_large_and_malformed() {
    // Above 64 KiB: answers invalid frame and exits ≠ 0 without reading the rest.
    let mut m = (1_000_000u32).to_ne_bytes().to_vec();
    m.extend(vec![b'{'; 1000]);
    let (code, output, err) = run_host(&[ORIGIN], &m);
    assert_eq!(code, Some(1));
    assert!(String::from_utf8_lossy(&output).contains("invalid_frame"));
    assert!(!err.contains('{'), "stderr com conteúdo: {err}");

    // Malformed JSON and an app operation: protocol error, and the loop goes on.
    let mut m = frame(b"{nao e json");
    m.extend(frame(
        br#"{"v":2,"op":"reveal","data":{"entry":"acme-wp-dev","field":"password"}}"#,
    ));
    let (code, output, err) = run_host(&[ORIGIN], &m);
    assert_eq!(code, Some(0));
    let s = String::from_utf8_lossy(&output);
    assert!(
        s.contains("invalid_request") && s.contains("unauthorized"),
        "{s}"
    );
    assert!(!err.contains("acme-wp-dev"), "stderr com conteúdo: {err}");
}

#[test]
fn no_daemon_answers_internal() {
    let m = frame(
        br#"{"v":2,"op":"fill","data":{"entry":"acme-wp-dev","origin":"http://localhost:8081"}}"#,
    );
    let (code, output, err) = run_host(&[ORIGIN], &m);
    assert_eq!(code, Some(0));
    let s = String::from_utf8_lossy(&output).into_owned();
    assert!(s.contains(r#""code":"internal""#), "{s}");
    assert!(err.contains("daemon indisponível"), "{err}");
}
