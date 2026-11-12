//! "Chrome" of the container of `make test-docker-platform` (Phase 40): installed as
//! `/opt/google/chrome/chrome` (root, 0755) only inside the container, it does what
//! Chrome does with a native messaging host: executes the host as a direct child, with
//! the origin of the extension as argument, sends the messages and reads the replies.
//!
//! Usage: `fake-chrome <host> <file>`: each line of the file is one JSON message; the
//! output is one line per reply, and at the end the exit-code line. No secret of its own: the
//! replies are what the test daemon sent.

use std::io::{BufRead, Read, Write};
use std::process::{Command, ExitCode, Stdio};

const ORIGIN: &str = "chrome-extension://nhkehjnmlgkebhmelajfpmpblepaichf/";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let (Some(host), Some(file)) = (args.get(1), args.get(2)) else {
        eprintln!("uso: fake-chrome <host> <arquivo>");
        return ExitCode::from(2);
    };
    let Ok(f) = std::fs::File::open(file) else {
        return ExitCode::from(2);
    };
    let mut input = Vec::new();
    for line in std::io::BufReader::new(f).lines().map_while(Result::ok) {
        if line.is_empty() {
            continue;
        }
        input.extend_from_slice(&(line.len() as u32).to_ne_bytes());
        input.extend_from_slice(line.as_bytes());
    }
    let Ok(mut child) = Command::new(host)
        .arg(ORIGIN)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    else {
        println!("spawn falhou");
        return ExitCode::FAILURE;
    };
    if let Some(mut i) = child.stdin.take() {
        let _ = i.write_all(&input);
    }
    let mut output = Vec::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_end(&mut output);
    }
    let status = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
    let mut rest = &output[..];
    while rest.len() >= 4 {
        let n = u32::from_ne_bytes([rest[0], rest[1], rest[2], rest[3]]) as usize;
        let end = (4 + n).min(rest.len());
        println!("{}", String::from_utf8_lossy(&rest[4..end]));
        rest = &rest[end..];
    }
    println!("saida={status}");
    ExitCode::SUCCESS
}
