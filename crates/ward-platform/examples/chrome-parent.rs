//! Verification of the Chrome parent with the real Chrome (Phase 40; D5, D47;
//! `make test-nm`). No secret at all.
//!
//! - `chrome-parent serve <socket> <chrome>`: listens on the socket (test folder),
//!   accepts one connection and prints one line with the verdict of
//!   [`PeerProcess::verify_parent`] against the given binary (`Ok` or the reason), the
//!   parent pid, whether the parent is dumpable and the chain of process names.
//! - As a native messaging host (Chrome passes `chrome-extension://…/`): connects to the
//!   socket named by the parent-socket variable (renamed in m2:Phase 56), waits for the server to close, answers one message `{"ok":true}` to
//!   Chrome and exits. The same holds when run by hand (control).
//!
//! The printed markers are what `dev/tests/nm.py` looks for and stay as they are.

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::ExitCode;

use ward_platform::{AllowedBinary, PeerProcess};

fn serve(sock: &str, chrome: &str) -> ExitCode {
    let _ = std::fs::remove_file(sock);
    let l = match UnixListener::bind(sock) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bind: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("pronto");
    let _ = std::io::stdout().flush();
    let Ok((s, _)) = l.accept() else {
        return ExitCode::FAILURE;
    };
    let peer = match PeerProcess::from_socket(&s) {
        Ok(p) => p,
        Err(e) => {
            println!("pai=NoIdentity({})", e.kind());
            return ExitCode::SUCCESS;
        }
    };
    let chrome = AllowedBinary::new(chrome).expect("caminho do chrome");
    let r = peer.verify_parent(&chrome);
    let ppid = peer.parent_pid().unwrap_or(0);
    let dump = if std::fs::read_link(format!("/proc/{ppid}/exe")).is_ok() {
        "sim"
    } else {
        "nao"
    };
    let chain: Vec<String> = peer.chain().into_iter().map(|e| e.name).collect();
    println!(
        "pai={} ppid={ppid} pai_dumpable={dump} cadeia={}",
        match r {
            Ok(()) => "Ok".to_owned(),
            Err(m) => format!("{m:?}"),
        },
        chain.join(",")
    );
    drop(s);
    ExitCode::SUCCESS
}

fn probe() -> ExitCode {
    let Some(sock) = std::env::var_os("WARD_CHROME_PARENT_SOCK") else {
        return ExitCode::FAILURE;
    };
    if let Ok(mut s) = UnixStream::connect(sock) {
        let mut b = [0u8; 1];
        let _ = s.read(&mut b);
    }
    let body = br#"{"ok":true}"#;
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(&(body.len() as u32).to_ne_bytes());
    let _ = out.write_all(body);
    let _ = out.flush();
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("serve") => match (args.get(2), args.get(3)) {
            (Some(s), Some(c)) => serve(s, c),
            _ => ExitCode::from(2),
        },
        _ => probe(),
    }
}
