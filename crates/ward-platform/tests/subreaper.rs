//! Phase 55 (S8; medium finding of the final reaudit): a descendant that leaves the
//! group of `ward run` survives neither the end of the command nor the lock.
//!
//! Test binary of this file only (a process of its own): the process becomes a
//! subreaper, like `wardd`. A single `#[test]`, with the scenarios in sequence, so that
//! no other test in parallel has children in this process.
//!
//! Attacks (the two steps of the reaudit), first without the subreaper (control: the
//! runaway survives) and then with it (it dies):
//! 1. `setsid -f`: the command creates a grandchild with its own session, the middle
//!    parent exits and the command ends; the grandchild keeps the environment;
//! 2. `setpgid(0, 0)`: the child leaves the group (stays in the session) and the
//!    command ends.
//!
//! And what must not die: the tree of another session in progress and the orphan of a
//! plain child of this process (the app of `wardd`, which does no `setsid`).

use std::io::Read;
use std::process::Stdio;
use std::time::{Duration, Instant};

use ward_platform::child::{Command, End, Group, Pipes};
use ward_platform::descendants;

/// `python3 -I -c` with a mode:
/// - `setsid`: grandchild with `setsid` through two `fork`s (the middle one exits),
///   prints its pid and exits (or, with an extra argument, sleeps);
/// - `setpgid`: child with `setpgid(0, 0)`, prints its pid and exits;
/// - otherwise (the tree mode): child in the same group, prints the pid and sleeps.
const PY: &str = r#"
import os, sys, time
modo = sys.argv[1]
fica = len(sys.argv) > 2
r, w = os.pipe()
if modo == "setsid":
    a = os.fork()
    if a == 0:
        b = os.fork()
        if b == 0:
            os.setsid()
            os.write(w, (str(os.getpid()) + "\n").encode())
            time.sleep(60)
            os._exit(0)
        os._exit(0)
    os.waitpid(a, 0)
elif modo == "setpgid":
    a = os.fork()
    if a == 0:
        os.setpgid(0, 0)
        os.write(w, (str(os.getpid()) + "\n").encode())
        time.sleep(60)
        os._exit(0)
else:
    a = os.fork()
    if a == 0:
        os.write(w, (str(os.getpid()) + "\n").encode())
        time.sleep(60)
        os._exit(0)
print(os.read(r, 32).decode().strip(), flush=True)
if fica:
    time.sleep(60)
"#;

fn launch(mode: &str, stay: bool) -> (Group, Pipes, i32) {
    let mut c = Command::new(b"/usr/bin/python3", b"/").unwrap();
    for a in [&b"-I"[..], b"-c", PY.as_bytes(), mode.as_bytes()] {
        c.argument(a).unwrap();
    }
    if stay {
        c.argument(b"fica").unwrap();
    }
    c.variable(b"WARD_PASS", b"falso-senha-f55").unwrap();
    let (g, mut p) = c.launch().unwrap();
    let mut line = [0u8; 32];
    let n = p.stdout.as_mut().unwrap().read(&mut line).unwrap();
    let pid = std::str::from_utf8(&line[..n])
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    (g, p, pid)
}

/// Normal end of the command, like `wardd`: waits, kills the session, collects.
fn command_end(g: &Group) -> descendants::Report {
    g.wait_for_end().unwrap();
    let r = g.kill_with_child_sessions();
    let _ = g.collect().unwrap();
    r
}

fn stat(pid: i32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()
}

fn alive(pid: i32) -> bool {
    stat(pid).is_some_and(|s| !s.contains(") Z "))
}

fn dies(pid: i32) -> bool {
    let end = Instant::now() + Duration::from_secs(5);
    while alive(pid) {
        if Instant::now() > end {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    true
}

fn kill(pid: i32) {
    let _ = std::process::Command::new("/bin/kill")
        .args(["-9", &pid.to_string()])
        .status();
    assert!(dies(pid));
}

#[test]
fn attack_f55_descendant_outside_the_group_does_not_survive() {
    // Control, without the subreaper: both runaways survive the end of the command.
    assert!(!descendants::is_subreaper());
    for mode in ["setsid", "setpgid"] {
        let (g, _p, runaway) = launch(mode, false);
        let r = command_end(&g);
        assert_eq!(r.remaining, 0, "{r:?}");
        std::thread::sleep(Duration::from_millis(300));
        assert!(alive(runaway), "controle: {mode} não fugiu");
        kill(runaway);
    }

    descendants::become_subreaper().unwrap();
    assert!(descendants::is_subreaper());

    // 1 and 2: with the subreaper, the runaway dies at the end of the command and is
    // collected.
    for mode in ["setsid", "setpgid"] {
        let (g, _p, runaway) = launch(mode, false);
        let r = command_end(&g);
        assert_eq!(r.remaining, 0, "{r:?}");
        assert!(r.killed >= 1, "{mode}: {r:?}");
        assert!(dies(runaway), "{mode}: o fugitivo sobreviveu ao fim");
        let end = Instant::now() + Duration::from_secs(5);
        while stat(runaway).is_some() {
            assert!(Instant::now() < end, "{mode}: o zumbi não foi colhido");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    // The lock with the command alive: the whole session dies (the runaway with
    // `setsid` too, through the group and through the sweep of the orphans).
    let (g, _p, runaway) = launch("setsid", true);
    let r = g.kill_with_child_sessions();
    assert_eq!(r.remaining, 0, "{r:?}");
    assert_eq!(g.remaining(), 0);
    assert_eq!(g.collect().unwrap(), End::Signal(libc::SIGKILL));
    assert!(dies(runaway), "trava: o fugitivo sobreviveu");

    // The lock without a session (only the orphans): the runaway of a live session
    // dies; the session itself (leader and the child in the group) goes on.
    let (g, _p, runaway) = launch("setsid", true);
    let (other, _q, other_child) = launch("arvore", true);
    let r = descendants::kill_orphans();
    assert_eq!(r.remaining, 0, "{r:?}");
    assert!(dies(runaway), "kill_orphans: o fugitivo sobreviveu");
    assert!(alive(g.pid() as i32), "kill_orphans matou o líder");
    assert!(
        alive(other_child),
        "kill_orphans matou a árvore de outra sessão"
    );

    // The end of a session does not take the tree of another one in progress.
    let r = g.kill_with_child_sessions();
    assert_eq!(r.remaining, 0, "{r:?}");
    let _ = g.collect();
    assert!(alive(other_child), "o fim de uma sessão matou outra");
    assert!(alive(other.pid() as i32));

    // The orphan of a plain child of this process (without `setsid`: the session of
    // here, like the app of `wardd`) belongs to no session and gets no signal.
    let mut plain = std::process::Command::new("/usr/bin/python3")
        .args(["-I", "-c", PY, "arvore"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    // A single line: the grandchild holds the output open.
    let mut line = String::new();
    std::io::BufRead::read_line(
        &mut std::io::BufReader::new(plain.stdout.take().unwrap()),
        &mut line,
    )
    .unwrap();
    let plain_grandchild: i32 = line.trim().parse().unwrap();
    let _ = plain.wait();
    let end = Instant::now() + Duration::from_secs(5);
    while !stat(plain_grandchild)
        .is_some_and(|s| s.contains(&format!(") S {} ", std::process::id())))
    {
        assert!(
            Instant::now() < end,
            "o neto comum não virou filho deste processo"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let r = other.kill_with_child_sessions();
    assert_eq!(r.remaining, 0, "{r:?}");
    let _ = other.collect();
    assert!(dies(other_child));
    assert!(
        alive(plain_grandchild),
        "o órfão de um filho comum levou o sinal"
    );
    let r = descendants::kill_orphans();
    assert_eq!(r.killed, 0, "{r:?}");
    assert!(alive(plain_grandchild));
    kill(plain_grandchild);
}
