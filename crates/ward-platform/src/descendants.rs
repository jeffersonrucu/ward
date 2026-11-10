//! End of session with nothing left behind (Phase 55, S8; medium finding of the final
//! re-audit).
//!
//! `ward run`, `ward ssh`, the bridge and the Chrome of `ward playwright` run in their own
//! process group ([`crate::child::Group`]). Killing only the group left behind the
//! descendant that moved out of it: `setpgid(0, 0)` (another group in the same session)
//! or `setsid -f` (a new session, and the intermediate parent exits). That process kept
//! the secret in its environment after the lock and after the command ended. Here:
//!
//! - **subreaper** ([`become_subreaper`]): `wardd` turns on `PR_SET_CHILD_SUBREAPER`;
//!   every orphan of a descendant becomes a child of `wardd`, instead of vanishing to
//!   `init` or `systemd --user`;
//! - **who belongs to the session**: the descendants of the leader (by the `ppid` chain
//!   in `/proc`) and, among the children of `wardd` that are not recorded leaders
//!   ([`record`]), the orphans of the leader's session (`sid` = leader) and the
//!   **ownerless** orphans (a session that belongs to no leader and is not the one of
//!   `wardd`: whoever ran `setsid`), plus their descendants. The kernel does not reuse a
//!   pid while it is the session id of someone alive, so the `sid` cannot be forged:
//!   nobody from outside enters the leader's session or the one of `wardd`. The ownerless
//!   orphan does not tell which session it came from; it dies at the end of **any**
//!   session and on lock (security before convenience);
//! - **no race with pid reuse**: each process found gets `SIGSTOP` (stopped, it neither
//!   collects its children nor creates others) until no new one shows up; then `SIGKILL`
//!   for all. Every signal goes through a `pidfd`, opened and verified by the start time
//!   in `stat`: the signal never hits another process that inherited the pid;
//! - **what remained** is counted ([`Report::remaining`]): the CLI only hears "ended"
//!   when nothing of the session is still alive.
//!
//! The direct child of `wardd` in its own session (the app, opened without `setsid`) and
//! whatever it leaves orphaned belong to no session and get no signal.
//!
//! Without the subreaper (the unit tests, in a process that is not `wardd`), only the
//! leader's descendants count: no child of the test process is touched.

use std::collections::{HashMap, HashSet};
use std::io;
use std::os::fd::AsFd;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::sys;

/// Pids of the live leaders (launched and not yet collected). The same mutex orders the
/// launch (`fork` + record) with the sweep: a newborn leader is never taken for an
/// ownerless orphan.
static LEADERS: Mutex<Vec<libc::pid_t>> = Mutex::new(Vec::new());
/// Is this process a subreaper (`wardd`)?
static SUBREAPER: AtomicBool = AtomicBool::new(false);

/// Rounds of `SIGSTOP` before giving up waiting for the `fork`s to end.
const MAX_ROUNDS: usize = 256;
/// How long to wait for the killed processes to exit before counting what remained.
const KILL_DEADLINE: Duration = Duration::from_secs(2);

/// Turns on `PR_SET_CHILD_SUBREAPER` in this process (`wardd`, at startup). Without it,
/// the daemon does not start (fails closed).
pub fn become_subreaper() -> io::Result<()> {
    sys::become_subreaper()?;
    SUBREAPER.store(true, Ordering::SeqCst);
    Ok(())
}

/// Did this process become a subreaper through [`become_subreaper`]?
pub fn is_subreaper() -> bool {
    SUBREAPER.load(Ordering::SeqCst)
}

/// The record of the live leaders, locked.
pub(crate) fn record() -> MutexGuard<'static, Vec<libc::pid_t>> {
    LEADERS.lock().unwrap_or_else(|e| e.into_inner())
}

/// What the end of a session did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Report {
    /// Processes that got `SIGKILL` (the live leader included).
    pub killed: usize,
    /// Of those, how many were still alive at the end of the deadline.
    pub remaining: usize,
}

#[derive(Debug, Clone, Copy)]
struct Proc {
    ppid: libc::pid_t,
    sid: libc::pid_t,
    start: u64,
    zombie: bool,
}

/// `(state, ppid, sid, start)` of a `/proc/<pid>/stat`. Read after the last `)` (the
/// program name may have spaces and parentheses).
fn parse_stat(stat: &str) -> Option<(char, libc::pid_t, libc::pid_t, u64)> {
    let rest = &stat[stat.rfind(')')? + 1..];
    let c: Vec<&str> = rest.split_whitespace().collect();
    let state = c.first()?.chars().next()?;
    let ppid = c.get(1)?.parse().ok()?;
    let sid = c.get(3)?.parse().ok()?;
    // `starttime` is the 22nd field of `stat`; here, the 20th after the name.
    let start = c.get(19)?.parse().ok()?;
    Some((state, ppid, sid, start))
}

fn read_proc(proc: &Path, pid: libc::pid_t) -> Option<Proc> {
    let s = std::fs::read_to_string(proc.join(pid.to_string()).join("stat")).ok()?;
    let (state, ppid, sid, start) = parse_stat(&s)?;
    Some(Proc {
        ppid,
        sid,
        start,
        zombie: state == 'Z' || state == 'X',
    })
}

/// Every process of `proc` right now.
fn table(proc: &Path) -> HashMap<libc::pid_t, Proc> {
    let mut t = HashMap::new();
    let Ok(dir) = std::fs::read_dir(proc) else {
        return t;
    };
    for e in dir.flatten() {
        let Some(pid) = e
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<libc::pid_t>().ok())
        else {
            continue;
        };
        if let Some(p) = read_proc(proc, pid) {
            t.insert(pid, p);
        }
    }
    t
}

/// Who belongs to the session of `leader` (or, with `None`, only the ownerless orphans),
/// alive, in table `t`. `me`/`my_sid`: the sweeping process; `orphans`: it is a
/// subreaper.
fn targets(
    t: &HashMap<libc::pid_t, Proc>,
    leader: Option<libc::pid_t>,
    me: libc::pid_t,
    my_sid: libc::pid_t,
    leaders: &[libc::pid_t],
    orphans: bool,
) -> Vec<libc::pid_t> {
    let mut children: HashMap<libc::pid_t, Vec<libc::pid_t>> = HashMap::new();
    for (pid, p) in t {
        children.entry(p.ppid).or_default().push(*pid);
    }
    let mut queue: Vec<libc::pid_t> = Vec::new();
    if let Some(l) = leader
        && t.contains_key(&l)
    {
        queue.push(l);
    }
    if orphans {
        for c in children.get(&me).map(Vec::as_slice).unwrap_or(&[]) {
            let p = &t[c];
            if leaders.contains(c) || p.sid == my_sid {
                continue;
            }
            let in_session = leader == Some(p.sid);
            let ownerless = !leaders.contains(&p.sid);
            if in_session || ownerless {
                queue.push(*c);
            }
        }
    }
    let mut seen: HashSet<libc::pid_t> = HashSet::new();
    while let Some(p) = queue.pop() {
        if p <= 1 || p == me || !seen.insert(p) {
            continue;
        }
        for c in children.get(&p).map(Vec::as_slice).unwrap_or(&[]) {
            if !leaders.contains(c) || Some(*c) == leader {
                queue.push(*c);
            }
        }
    }
    seen.into_iter()
        .filter(|p| t.get(p).is_some_and(|x| !x.zombie))
        .collect()
}

/// `signal` to `pid` only if it is still the process that started at `start` (pidfd
/// opened before the check: the signal does not hit whoever inherited the pid).
fn verified_signal(pid: libc::pid_t, start: u64, signal: libc::c_int) -> bool {
    let Ok(fd) = sys::pidfd_open(pid) else {
        return false;
    };
    if read_proc(Path::new("/proc"), pid).is_none_or(|p| p.start != start) {
        return false;
    }
    sys::pidfd_signal(fd.as_fd(), signal).unwrap_or(false)
}

/// Collects the zombie orphans (children of this process, outside the record and its
/// session).
fn collect_orphans(
    t: &HashMap<libc::pid_t, Proc>,
    me: libc::pid_t,
    my_sid: libc::pid_t,
    leaders: &[libc::pid_t],
) {
    for (pid, p) in t {
        if p.ppid == me && p.zombie && p.sid != my_sid && !leaders.contains(pid) {
            sys::collect_if_exited(*pid);
        }
    }
}

/// Kills the whole session of `leader` (see the module) and, being a subreaper, the
/// ownerless orphans. With `None`, only the ownerless orphans (the lock with no session
/// running).
pub(crate) fn kill_session(leader: Option<libc::pid_t>) -> Report {
    kill_session_in(Path::new("/proc"), leader)
}

fn kill_session_in(proc: &Path, leader: Option<libc::pid_t>) -> Report {
    let leaders = record();
    let orphans = is_subreaper();
    let (me, my_sid) = sys::pid_and_session();
    if let Some(l) = leader {
        let _ = sys::stop_group(l);
    }
    // 1. Stop: each one found gets `SIGSTOP` until the sweep finds nobody new.
    let mut stopped: HashMap<libc::pid_t, u64> = HashMap::new();
    for _ in 0..MAX_ROUNDS {
        let t = table(proc);
        let new: Vec<libc::pid_t> = targets(&t, leader, me, my_sid, &leaders, orphans)
            .into_iter()
            .filter(|p| !stopped.contains_key(p))
            .collect();
        if new.is_empty() {
            break;
        }
        for p in new {
            let start = t[&p].start;
            verified_signal(p, start, libc::SIGSTOP);
            stopped.insert(p, start);
        }
    }
    // 2. Kill them all (and the leader's group, to be sure).
    for (p, start) in &stopped {
        verified_signal(*p, *start, libc::SIGKILL);
    }
    if let Some(l) = leader {
        let _ = sys::kill_group(l);
    }
    // 3. Wait for them to exit; whoever shows up now (a `SIGCONT` from outside before
    //    the `SIGKILL`) dies too. The leader stays a zombie for the group's `collect`.
    let end = Instant::now() + KILL_DEADLINE;
    let remaining = loop {
        let t = table(proc);
        if orphans {
            collect_orphans(&t, me, my_sid, &leaders);
        }
        for p in targets(&t, leader, me, my_sid, &leaders, orphans) {
            if let std::collections::hash_map::Entry::Vacant(v) = stopped.entry(p) {
                let start = t[&p].start;
                verified_signal(p, start, libc::SIGKILL);
                v.insert(start);
            }
        }
        let alive = stopped
            .iter()
            .filter(|(p, start)| {
                read_proc(proc, **p).is_some_and(|x| !x.zombie && x.start == **start)
            })
            .count();
        if alive == 0 || Instant::now() >= end {
            break alive;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    if orphans {
        collect_orphans(&table(proc), me, my_sid, &leaders);
    }
    Report {
        killed: stopped.len(),
        remaining,
    }
}

/// Only the ownerless orphans (the lock, Phase 55): whatever left a session with
/// `setsid` and became a child of `wardd`. Without the subreaper, does nothing.
pub fn kill_orphans() -> Report {
    if !is_subreaper() {
        return Report::default();
    }
    kill_session(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(ppid: libc::pid_t, sid: libc::pid_t) -> Proc {
        Proc {
            ppid,
            sid,
            start: 1,
            zombie: false,
        }
    }

    #[test]
    fn stat_with_parentheses_in_name() {
        let s = "42 (a) b) (c) S 7 42 40 0 -1 4194560 1 0 0 0 0 0 0 0 20 0 1 0 12345 0 0";
        assert_eq!(parse_stat(s), Some(('S', 7, 40, 12345)));
        assert_eq!(parse_stat("42 sem-parenteses"), None);
        assert_eq!(parse_stat("42 (x) S 7 1 1"), None);
    }

    /// The table of a `wardd` (100, session 100) with the app (101, the daemon's
    /// session), session A (200) and session B (300), each with a runaway.
    fn scenario() -> HashMap<libc::pid_t, Proc> {
        HashMap::from([
            (100, p(1, 100)),
            (101, p(100, 100)), // app
            (102, p(101, 100)), // child of the app
            (103, p(100, 100)), // orphan of the app (no setsid): belongs to no session
            (104, p(100, 104)), // orphan of the app that ran setsid: ownerless
            (200, p(100, 200)), // leader A
            (201, p(200, 200)),
            (202, p(201, 202)), // grandchild of A with its own session: descendant
            (203, p(100, 200)), // orphan of A (setpgid): sid = A
            (204, p(203, 204)),
            (300, p(100, 300)), // leader B
            (301, p(300, 300)),
            (302, p(100, 300)), // orphan of B
            (303, p(100, 303)), // ownerless orphan (setsid -f)
            (999, p(1, 999)),   // stranger
        ])
    }

    fn sorted(mut v: Vec<libc::pid_t>) -> Vec<libc::pid_t> {
        v.sort_unstable();
        v
    }

    #[test]
    fn session_targets_with_subreaper() {
        let t = scenario();
        let leaders = [200, 300, 101];
        assert_eq!(
            sorted(targets(&t, Some(200), 100, 100, &leaders, true)),
            [104, 200, 201, 202, 203, 204, 303]
        );
        assert_eq!(
            sorted(targets(&t, Some(300), 100, 100, &leaders, true)),
            [104, 300, 301, 302, 303]
        );
        assert_eq!(
            sorted(targets(&t, None, 100, 100, &leaders, true)),
            [104, 303]
        );
    }

    #[test]
    fn without_subreaper_only_the_leader_tree() {
        let t = scenario();
        assert_eq!(
            sorted(targets(&t, Some(200), 100, 100, &[200, 300], false)),
            [200, 201, 202]
        );
        assert!(targets(&t, None, 100, 100, &[200, 300], false).is_empty());
    }

    #[test]
    fn zombie_is_not_a_target() {
        let mut t = scenario();
        t.get_mut(&200).unwrap().zombie = true;
        t.get_mut(&303).unwrap().zombie = true;
        assert_eq!(
            sorted(targets(&t, Some(200), 100, 100, &[200, 300], true)),
            [104, 201, 202, 203, 204]
        );
    }
}
