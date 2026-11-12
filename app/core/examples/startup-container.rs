//! Test "ward-app" of the container of Phase 29a (`make test-docker-platform`; D47,
//! D57; V5.31). Runs the same startup as the real app (the startup module of
//! `ward_app_core`), without GTK (the container image has no WebKitGTK), and writes a
//! report to the file in `REPORT`, because the daemon's launcher binds 0–2 to
//! `/dev/null`. It is never installed outside the container.
//!
//! Report lines (one per execution, with the pid; the markers are the interface with
//! the `inside.sh` scripts of `dev/container-*` and stay as they are):
//! - the start line (`pid=… gid=… dumpable=… tracer=…`): right at the start of `main`,
//!   before the startup (what the setgid `exec` left);
//! - the after line (`pid=… gid=… dumpable=… tracer=… socket_cloexec=… env=…`) or the
//!   error line (`pid=… <message>`).
//!
//! On the first execution of a pid, it waits for the connection to drop (the test server
//! closes after answering) and executes itself again through the startup's watcher; on the
//! second one, it exits.

use std::io::Write;

use ward_app_core::startup;

const REPORT: &str = "/tmp/ward-app-relatorio.txt";

fn status(field: &str) -> String {
    std::fs::read_to_string("/proc/self/status")
        .unwrap_or_default()
        .lines()
        .find_map(|l| {
            l.strip_prefix(field)
                .map(|v| v.split_whitespace().collect::<Vec<_>>().join(","))
        })
        .unwrap_or_default()
}

fn dumpable() -> String {
    ward_platform::app::dumpable_value()
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "?".to_owned())
}

/// Does every fd that is a socket have `FD_CLOEXEC`? (read from `fdinfo`, flag
/// `02000000`).
fn sockets_with_cloexec() -> String {
    let mut sockets = 0;
    let mut all = true;
    for fd in std::fs::read_dir("/proc/self/fd")
        .into_iter()
        .flatten()
        .flatten()
    {
        let target = std::fs::read_link(fd.path()).unwrap_or_default();
        if !target.to_string_lossy().starts_with("socket:") {
            continue;
        }
        sockets += 1;
        let info = std::fs::read_to_string(format!(
            "/proc/self/fdinfo/{}",
            fd.file_name().to_string_lossy()
        ))
        .unwrap_or_default();
        let flags = info
            .lines()
            .find_map(|l| l.strip_prefix("flags:"))
            .and_then(|f| u32::from_str_radix(f.trim(), 8).ok())
            .unwrap_or(0);
        all &= flags & 0o2_000_000 != 0;
    }
    format!("{all}/{sockets}")
}

/// Open fds (the last one is the one of `read_dir` itself) and where 0–2 point to.
fn fds() -> String {
    let mut v: Vec<u32> = std::fs::read_dir("/proc/self/fd")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_string_lossy().parse().ok())
        .collect();
    v.sort_unstable();
    let std: Vec<String> = (0..3)
        .map(|n| {
            std::fs::read_link(format!("/proc/self/fd/{n}"))
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "-".to_owned())
        })
        .collect();
    let list: Vec<String> = v.iter().map(u32::to_string).collect();
    format!("fds={} std={}", list.join(","), std.join(","))
}

fn report(line: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(REPORT)
    {
        let _ = writeln!(f, "{line}");
    }
}

fn runs_of_this_pid(pid: u32) -> usize {
    let prefix = format!("inicio pid={pid} ");
    std::fs::read_to_string(REPORT)
        .unwrap_or_default()
        .lines()
        .filter(|l| l.starts_with(&prefix))
        .count()
}

fn main() {
    let pid = std::process::id();
    let fds = fds();
    report(&format!(
        "inicio pid={pid} gid={} dumpable={} tracer={} {fds}",
        status("Gid:"),
        dumpable(),
        status("TracerPid:")
    ));
    let mut p = match startup::start() {
        Ok(p) => p,
        Err(e) => {
            report(&format!("erro pid={pid} {e}"));
            std::process::exit(1);
        }
    };
    let mut names: Vec<String> = std::env::vars_os()
        .map(|(k, _)| k.to_string_lossy().into_owned())
        .collect();
    names.sort();
    report(&format!(
        "depois pid={pid} gid={} dumpable={} tracer={} socket_cloexec={} vmlck={} pai={} env={}",
        status("Gid:"),
        dumpable(),
        status("TracerPid:"),
        sockets_with_cloexec(),
        status("VmLck:"),
        p.parent_pid(),
        names.join(",")
    ));
    let _daemon = p.take_daemon();
    if runs_of_this_pid(pid) < 2 {
        let _ = p.watch(move || report(&format!("queda pid={pid}"))).join();
    }
}
