//! D65 d (Phase 22d, provisional): creation and modification dates stamped by the
//! daemon's clock (injected here). A new entry gets both; editing and the level change
//! change only the modification date; reopening, renaming and restoring keep the dates; they
//! only come out in the entry detail of the app role (V3.41). Own test vault, fake values
//! only.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use common::{Dir, PIN, SECRETS, b64, error, login, ok, request, set_level};
use ward_core::protocol::{self, EntryDetail, Response, ResponseError, Role, decode_response};
use wardd::core::{Clock, Daemon};

/// 2026-11-02 00:00 UTC and following (the footer of the frame approved in M0).
const T0: u64 = 1_762_041_600;
const DAY: u64 = 86_400;

/// Wall clock adjustable by the test; 0 = unreadable.
#[derive(Clone)]
struct WallClock(Arc<AtomicU64>);

impl WallClock {
    fn set(&self, s: u64) {
        self.0.store(s, Ordering::SeqCst);
    }
}

impl Clock for WallClock {
    fn sleep(&self, _d: Duration) {}

    fn now(&self) -> Option<Duration> {
        Some(Duration::from_secs(1000))
    }

    fn unix_seconds(&self) -> Option<u64> {
        Some(self.0.load(Ordering::SeqCst)).filter(|s| *s != 0)
    }
}

fn daemon(dir: &Dir, clock: &WallClock) -> Daemon {
    Daemon::new(dir.paths(), Box::new(clock.clone()))
}

fn create(dir: &Dir, clock: &WallClock) -> Daemon {
    let d = daemon(dir, clock);
    let r = request(
        &d,
        Role::App,
        "create_vault",
        &format!(
            r#"{{"password":"{}","pin":"{}"}}"#,
            b64(common::PASSWORD.as_bytes()),
            b64(PIN.as_bytes())
        ),
    );
    assert!(ok(&r), "{r:?}");
    d
}

fn detail(d: &Daemon, name: &str) -> EntryDetail {
    match request(d, Role::App, "detail", &format!(r#"{{"entry":"{name}"}}"#)) {
        Response::Detail(x) => x,
        other => panic!("{other:?}"),
    }
}

fn dates(d: &Daemon, name: &str) -> (Option<u64>, Option<u64>) {
    let x = detail(d, name).dates;
    (x.created_at(), x.modified_at())
}

fn save(d: &Daemon, data: &str) {
    let r = request(d, Role::App, "save_entry", data);
    assert!(ok(&r), "{r:?}");
}

/// `login(name)` editing `previous`.
fn edit(previous: &str, name: &str, password: &str) -> String {
    login(name, password).replacen('{', &format!(r#"{{"previous":"{previous}","#), 1)
}

/// V3.41: new → both; editing and the level change → only the modification; a clock that
/// goes back does not make the date go down; reopening in a new daemon returns the same
/// dates.
#[test]
fn v3_41_dates_from_the_daemon_clock() {
    let dir = Dir::new("dates-v3-41");
    let clock = WallClock(Arc::new(AtomicU64::new(T0)));
    let d = create(&dir, &clock);

    save(&d, &login("t-datas", SECRETS[0]));
    assert_eq!(dates(&d, "t-datas"), (Some(T0), Some(T0)));

    clock.set(T0 + DAY);
    save(&d, &edit("t-datas", "t-datas", SECRETS[1]));
    assert_eq!(dates(&d, "t-datas"), (Some(T0), Some(T0 + DAY)));

    clock.set(T0 + 2 * DAY);
    assert!(ok(&set_level(&d, Role::App, "t-datas", "free", PIN)));
    assert_eq!(dates(&d, "t-datas"), (Some(T0), Some(T0 + 2 * DAY)));

    // A wrong PIN changes nothing, not even the date.
    clock.set(T0 + 3 * DAY);
    let r = set_level(&d, Role::App, "t-datas", "blocked", common::WRONG_PIN);
    assert_eq!(error(&r), Some(ResponseError::WrongPin));
    assert_eq!(dates(&d, "t-datas"), (Some(T0), Some(T0 + 2 * DAY)));

    // A clock that goes back: the modification does not go down, the creation never
    // changes.
    clock.set(T0 - DAY);
    save(&d, &edit("t-datas", "t-datas", SECRETS[2]));
    assert_eq!(dates(&d, "t-datas"), (Some(T0), Some(T0 + 2 * DAY)));

    // Renaming is editing: the creation goes along.
    clock.set(T0 + 4 * DAY);
    save(&d, &edit("t-datas", "t-renomeada", SECRETS[2]));
    assert_eq!(dates(&d, "t-renomeada"), (Some(T0), Some(T0 + 4 * DAY)));

    // Another new entry does not touch the dates of the first one.
    clock.set(T0 + 5 * DAY);
    save(&d, &login("t-outra", SECRETS[3]));
    assert_eq!(
        dates(&d, "t-outra"),
        (Some(T0 + 5 * DAY), Some(T0 + 5 * DAY))
    );
    assert_eq!(dates(&d, "t-renomeada"), (Some(T0), Some(T0 + 4 * DAY)));

    // The daemon reopens the vault (another process): the same dates, read from the v4
    // body.
    drop(d);
    clock.set(T0 + 9 * DAY);
    let d2 = daemon(&dir, &clock);
    assert!(ok(&common::unlock(&d2, common::PASSWORD)));
    assert_eq!(dates(&d2, "t-renomeada"), (Some(T0), Some(T0 + 4 * DAY)));
    assert_eq!(
        dates(&d2, "t-outra"),
        (Some(T0 + 5 * DAY), Some(T0 + 5 * DAY))
    );
}

/// V3.41: the dates only arrive in the entry detail of the app role. The agent and the
/// extension do not request the detail, and the list (of the app and of the agent)
/// carries no date at all.
#[test]
fn attack_v3_41_dates_do_not_go_to_the_agent() {
    let dir = Dir::new("dates-agent");
    let clock = WallClock(Arc::new(AtomicU64::new(T0)));
    let d = create(&dir, &clock);
    save(&d, &login("t-datas", SECRETS[0]));
    for role in [Role::Agent, Role::Extension] {
        let r = request(&d, role, "detail", r#"{"entry":"t-datas"}"#);
        assert_eq!(error(&r), Some(ResponseError::Unauthorized), "{role:?}");
    }
    for role in [Role::App, Role::Agent] {
        let r = request(&d, role, "list", "{}");
        let q = protocol::encode_response(role, &r).unwrap();
        let s = String::from_utf8_lossy(&q);
        assert!(
            !s.contains("created_at") && !s.contains("modified_at"),
            "{s}"
        );
        assert!(!s.contains(&T0.to_string()), "{s}");
    }
    // The app's detail frame carries the dates and does not pass the agent's reading.
    let r = request(&d, Role::App, "detail", r#"{"entry":"t-datas"}"#);
    let q = protocol::encode_response(Role::App, &r).unwrap();
    assert!(String::from_utf8_lossy(&q).contains(&format!(r#""created_at":{T0}"#)));
    assert!(decode_response(Role::Agent, &q).is_err());
    // No secret of the entry in the frame with the dates.
    for s in SECRETS {
        assert!(!q.windows(s.len()).any(|w| w == s.as_bytes()));
        let b = b64(s.as_bytes());
        assert!(!q.windows(b.len()).any(|w| w == b.as_bytes()));
    }
}

/// V3.41: a date in the draft of a save is refused before it reaches the core,
/// and the vault stays as it was (same bytes on disk, same dates).
#[test]
fn attack_v3_41_date_from_the_client_refused() {
    let dir = Dir::new("dates-client");
    let clock = WallClock(Arc::new(AtomicU64::new(T0)));
    let d = create(&dir, &clock);
    save(&d, &login("t-datas", SECRETS[0]));
    let before = std::fs::read(&dir.paths().vault).unwrap();
    clock.set(T0 + DAY);
    let originals = [
        edit("t-datas", "t-datas", SECRETS[1]),
        login("t-nova", SECRETS[1]),
    ];
    for forged in [
        edit("t-datas", "t-datas", SECRETS[1]).replacen(
            r#""name":"t-datas","group""#,
            r#""name":"t-datas","created_at":1,"group""#,
            1,
        ),
        edit("t-datas", "t-datas", SECRETS[1]).replacen(
            r#""name":"t-datas","group""#,
            r#""name":"t-datas","modified_at":1,"group""#,
            1,
        ),
        login("t-nova", SECRETS[1]).replacen('{', r#"{"created_at":1,"#, 1),
        login("t-nova", SECRETS[1]).replacen(r#""url":"#, r#""modified_at":1,"url":"#, 1),
    ] {
        // The guard: the forged date really went into the draft (a `replacen` that
        // finds nothing would send the original and prove nothing).
        assert!(
            forged.contains("\"created_at\":1") || forged.contains("\"modified_at\":1"),
            "{forged}"
        );
        assert!(!originals.contains(&forged), "{forged}");
        assert!(
            serde_json::from_str::<serde_json::Value>(&forged).is_ok(),
            "{forged}"
        );
        let r = request(&d, Role::App, "save_entry", &forged);
        assert_eq!(error(&r), Some(ResponseError::InvalidRequest), "{forged}");
    }
    assert_eq!(dates(&d, "t-datas"), (Some(T0), Some(T0)));
    assert!(std::fs::read(&dir.paths().vault).unwrap() == before);
    assert!(error(&request(&d, Role::App, "detail", r#"{"entry":"t-nova"}"#)).is_some());
}

/// Unreadable clock: the write goes on without a new stamp (new entry without dates,
/// editing keeps the previous ones); with the clock back, the entry without a creation
/// date gets only the modification.
#[test]
fn unreadable_clock_writes_without_a_new_date() {
    let dir = Dir::new("dates-no-clock");
    let clock = WallClock(Arc::new(AtomicU64::new(T0)));
    let d = create(&dir, &clock);
    save(&d, &login("t-com", SECRETS[0]));
    clock.set(0);
    save(&d, &login("t-sem", SECRETS[1]));
    assert_eq!(dates(&d, "t-sem"), (None, None));
    save(&d, &edit("t-com", "t-com", SECRETS[2]));
    assert_eq!(dates(&d, "t-com"), (Some(T0), Some(T0)));
    assert!(ok(&set_level(&d, Role::App, "t-com", "free", PIN)));
    assert_eq!(dates(&d, "t-com"), (Some(T0), Some(T0)));
    clock.set(T0 + DAY);
    save(&d, &edit("t-sem", "t-sem", SECRETS[3]));
    assert_eq!(dates(&d, "t-sem"), (None, Some(T0 + DAY)));
    // A clock beyond 9999-12-31: like unreadable.
    clock.set(u64::MAX);
    save(&d, &edit("t-sem", "t-sem", SECRETS[1]));
    assert_eq!(dates(&d, "t-sem"), (None, Some(T0 + DAY)));
}

/// D65 d: restoring a backup keeps the dates of the restored version.
#[test]
fn restore_keeps_the_dates() {
    let dir = Dir::new("dates-restore");
    let clock = WallClock(Arc::new(AtomicU64::new(T0)));
    let d = create(&dir, &clock);
    save(&d, &login("t-datas", SECRETS[0]));
    let version = match request(&d, Role::App, "backups", "{}") {
        Response::Backups(l) => l[0].name.clone(),
        other => panic!("{other:?}"),
    };
    clock.set(T0 + 3 * DAY);
    save(&d, &edit("t-datas", "t-datas", SECRETS[1]));
    assert_eq!(dates(&d, "t-datas"), (Some(T0), Some(T0 + 3 * DAY)));
    clock.set(T0 + 7 * DAY);
    let r = request(
        &d,
        Role::App,
        "restore_backup",
        &format!(
            r#"{{"version":"{version}","password":"{}","pin":"{}"}}"#,
            b64(common::PASSWORD.as_bytes()),
            b64(PIN.as_bytes())
        ),
    );
    assert!(ok(&r), "{r:?}");
    assert_eq!(dates(&d, "t-datas"), (Some(T0), Some(T0)));
}
