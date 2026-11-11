//! Socket handed over by systemd (`LISTEN_FDS`, D46; V3.23; S5), on the host and without
//! root.
//!
//! Without root there is no root directory for the socket, so only the attacks fit
//! here: a "systemd" of the user itself that passes a socket from its own directory,
//! `LISTEN_PID` of another process, `LISTEN_FDS` other than 1, a different expected path
//! and an environment with no activation. The happy path (root:root 0755 directory in
//! `/run/ward/1000/`) runs in the container (`make test-docker-platform`).
//!
//! Both sides are this test binary, called again with `--exact helper_child` and
//! the [`MODE`] variable saying what to do.

#![cfg(feature = "test-mode")]

mod common;

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::process::{Command, Stdio};

use common::TestDir;
use ward_platform::socket::prepare_dir;

const MODE: &str = "WARD_ACTIVATION";
const MARK: &str = "RESULTADO:";

/// Body of the child processes. Without the variable, does nothing.
#[test]
fn helper_child() {
    let Ok(mode) = std::env::var(MODE) else {
        return;
    };
    let path = std::env::var_os("WARD_PATH").expect("WARD_PATH");
    match mode.as_str() {
        // Plays systemd: creates the socket and runs itself again as the receiver.
        "ativar" => {
            let listener = UnixListener::bind(&path).unwrap();
            let mut cmd = child("receber");
            // On `exec`, the same stdout as this process (the test's pipe).
            cmd.env("WARD_PATH", &path)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());
            if let Some(expected) = std::env::var_os("WARD_EXPECTED") {
                cmd.env("WARD_EXPECTED", expected);
            }
            ward_platform::testing::run_with_socket(&listener, cmd);
        }
        "receber" => {
            let expected = std::env::var_os("WARD_EXPECTED").unwrap_or(path);
            match ward_platform::listen_from_listen_fds(Path::new(&expected)) {
                Ok(_) => println!("{MARK}ok"),
                Err(e) => println!("{MARK}{e:?}"),
            }
        }
        _ => panic!("modo desconhecido"),
    }
}

fn child(mode: &str) -> Command {
    let mut c = Command::new(std::env::current_exe().unwrap());
    c.args(["--exact", "helper_child", "--nocapture", "--test-threads=1"])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env(MODE, mode)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    c
}

fn result(mut c: Command) -> String {
    let mut f = c.spawn().unwrap();
    let output = BufReader::new(f.stdout.take().unwrap());
    let mut r = String::new();
    for line in output.lines() {
        let line = line.unwrap();
        // The harness writes "test helper_child ... " before, on the same line.
        if let Some(i) = line.find(MARK) {
            r = line[i + MARK.len()..].to_owned();
        }
    }
    f.wait().unwrap();
    r
}

#[test]
fn attack_fake_user_systemd_with_socket_in_its_dir() {
    let d = TestDir::new();
    let dir = prepare_dir(&d.path).unwrap();
    let path = dir.path().join("wardd.sock");
    let mut c = child("ativar");
    c.env("WARD_PATH", &path);
    let r = result(c);
    // The socket is real and listens on the right path; the directory is what is not
    // root's.
    assert!(r.starts_with("UntrustedDir"), "{r}");
}

#[test]
fn attack_socket_at_another_path() {
    let d = TestDir::new();
    let dir = prepare_dir(&d.path).unwrap();
    let path = dir.path().join("wardd.sock");
    let mut c = child("ativar");
    c.env("WARD_PATH", &path)
        .env("WARD_EXPECTED", "/run/ward/1000/wardd.sock");
    assert_eq!(result(c), "PathMismatch");
}

#[test]
fn attack_wrong_listen_pid_and_listen_fds() {
    let path = "/run/ward/1000/wardd.sock";
    // Without the variables.
    let mut c = child("receber");
    c.env("WARD_PATH", path);
    assert_eq!(result(c), "NotActivated");
    // LISTEN_PID of another process (fd 3 would be another's), or malformed.
    for pid in ["1", "+1", "", "abc"] {
        let mut c = child("receber");
        c.env("WARD_PATH", path)
            .env("LISTEN_PID", pid)
            .env("LISTEN_FDS", "1");
        assert_eq!(result(c), "PidMismatch", "{pid:?}");
    }
}

#[test]
fn attack_more_than_one_fd() {
    // `sh` puts its own pid in LISTEN_PID and runs itself as the receiver (same pid).
    let path = "/run/ward/1000/wardd.sock";
    let me = std::env::current_exe().unwrap();
    let mut c = Command::new("/usr/bin/sh");
    c.arg("-c")
        .arg("LISTEN_PID=$$ LISTEN_FDS=2 exec \"$0\" --exact helper_child --nocapture")
        .arg(&me)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env(MODE, "receber")
        .env("WARD_PATH", path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    assert_eq!(result(c), "FdCount");
}
