//! D62 in the daemon core (Phase 22b): the D20 wait per role and kept in the state
//! file (V3.33, S9), vault creation with an old state (V3.35, S7) and the identity of the
//! written vault read from the temp file's fd (V3.35, S7, S13).
//!
//! Own test vault, only with fake values (never the real vault nor the one in `.dev/`).

mod common;

use std::fs;
#[cfg(feature = "test-hooks")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::*;
use ward_core::config::{PinHash, VaultConfig};
use ward_core::format::{MasterPassword, OpenVault};
use ward_core::protocol::{ResponseError, MSG_ROLLBACK, Role, Resposta};
use wardd::arquivos::{self, Geracoes};
use wardd::nucleo::{Caminhos, Daemon, ESPERA_MAXIMA, Relogio, espera_destrave};

/// Clock with the time controlled by the test: records the waits and does not sleep.
#[derive(Clone, Default)]
struct MovableClock {
    now: Arc<Mutex<Duration>>,
    waits: Arc<Mutex<Vec<Duration>>>,
}

impl MovableClock {
    fn advance(&self, d: Duration) {
        *self.now.lock().unwrap() += d;
    }

    fn waits(&self) -> Vec<Duration> {
        std::mem::take(&mut *self.waits.lock().unwrap())
    }
}

impl Relogio for MovableClock {
    fn dormir(&self, d: Duration) {
        self.waits.lock().unwrap().push(d);
    }

    fn agora(&self) -> Option<Duration> {
        Some(*self.now.lock().unwrap())
    }
}

fn movable_daemon(c: Caminhos, r: &MovableClock) -> Daemon {
    Daemon::novo(c, Box::new(r.clone()))
}

fn unlock_as(d: &Daemon, role: Role, password: &str) -> Resposta {
    request(
        d,
        role,
        "destravar",
        &format!(r#"{{"senha":"{}"}}"#, b64(password.as_bytes())),
    )
}

fn state_text(dir: &Dir) -> String {
    fs::read_to_string(dir.paths().estado).unwrap()
}

fn failures_on_disk(dir: &Dir) -> u32 {
    arquivos::ler_estado_completo(&dir.paths().estado)
        .unwrap()
        .falhas_agente
}

/// Test vault created and locked; the daemon that created it is dropped.
fn locked_vault(dir: &Dir) {
    let (d, _) = test_vault(dir);
    d.travar();
}

/// V3.33 (D62, S9): 10 wrong passwords from the agent, `wardd` dies and starts again,
/// and the agent's next attempt still waits 30 s; the counter is on disk before the
/// error reply goes out.
#[test]
fn v3_33_agent_wait_survives_the_restart() {
    let dir = Dir::new("d62-reinicio");
    locked_vault(&dir);
    let r = MovableClock::default();
    let d = movable_daemon(dir.paths(), &r);
    for i in 0..10u32 {
        let resp = unlock_as(&d, Role::Agent, &format!("falso-errada-{i:02}"));
        assert_eq!(error(&resp), Some(ResponseError::WrongPassword));
        // Already on disk when the reply goes out (fsync before the reply).
        assert_eq!(failures_on_disk(&dir), i + 1);
    }
    let waits: Vec<u64> = r.waits().iter().map(Duration::as_secs).collect();
    assert_eq!(waits, [0, 1, 2, 4, 8, 16, 30, 30, 30, 30]);

    // "kill -9": the daemon goes away with nothing beyond what is already on disk.
    drop(d);
    let r = MovableClock::default();
    let d = movable_daemon(dir.paths(), &r);
    assert_eq!(
        error(&unlock_as(&d, Role::Agent, "falso-errada-xx")),
        Some(ResponseError::WrongPassword)
    );
    assert_eq!(r.waits(), [ESPERA_MAXIMA]);
    assert_eq!(failures_on_disk(&dir), 11);

    // Counted from the start: the new daemon started 20 s ago, the first wait is 10 s.
    drop(d);
    let r = MovableClock::default();
    let d = movable_daemon(dir.paths(), &r);
    r.advance(Duration::from_secs(20));
    let _ = unlock_as(&d, Role::Agent, "falso-errada-yy");
    // The next one already waits the whole time.
    let _ = unlock_as(&d, Role::Agent, "falso-errada-zz");
    assert_eq!(
        r.waits(),
        [Duration::from_secs(10), ESPERA_MAXIMA],
        "a 1.ª depois da partida conta da partida; as outras, inteiras"
    );
    // Started more than 30 s ago: the first one does not wait, but the counter goes on.
    drop(d);
    let r = MovableClock::default();
    let d = movable_daemon(dir.paths(), &r);
    r.advance(Duration::from_secs(45));
    let _ = unlock_as(&d, Role::Agent, "falso-errada-ww");
    let _ = unlock_as(&d, Role::Agent, "falso-errada-vv");
    assert_eq!(r.waits(), [Duration::ZERO, ESPERA_MAXIMA]);

    // The right unlock zeroes both, on disk too.
    assert!(ok(&unlock_as(&d, Role::Agent, PASSWORD)));
    assert_eq!(failures_on_disk(&dir), 0);
    d.travar();
    let _ = unlock_as(&d, Role::Agent, "falso-errada-uu");
    let _ = unlock_as(&d, Role::App, "falso-errada-tt");
    assert_eq!(r.waits()[1..], [Duration::ZERO, Duration::ZERO]);
}

/// V3.33 (D62): the agent in the middle of its waits does not delay the app, which
/// unlocks with the right password without waiting; after the app's right unlock, both
/// counters reset.
#[test]
fn attack_agent_in_the_middle_of_the_wait_does_not_delay_the_app() {
    let dir = Dir::new("d62-papel");
    locked_vault(&dir);

    /// Clock whose sleep of 4 s or more (the agent's wait) stays stuck until the
    /// test releases it; the shorter waits return at once.
    struct Stuck {
        entered: Mutex<mpsc::Sender<Duration>>,
        release: Mutex<mpsc::Receiver<()>>,
        waits: Arc<Mutex<Vec<(Duration, bool)>>>,
    }
    impl Relogio for Stuck {
        fn dormir(&self, d: Duration) {
            let stuck = d >= Duration::from_secs(4);
            self.waits.lock().unwrap().push((d, stuck));
            if stuck {
                let _ = self.entered.lock().unwrap().send(d);
                let _ = self.release.lock().unwrap().recv();
            }
        }
    }
    let (tx_entered, rx_entered) = mpsc::channel();
    let (tx_release, rx_release) = mpsc::channel();
    let waits = Arc::new(Mutex::new(Vec::new()));
    let d = Arc::new(Daemon::novo(
        dir.paths(),
        Box::new(Stuck {
            entered: Mutex::new(tx_entered),
            release: Mutex::new(rx_release),
            waits: waits.clone(),
        }),
    ));
    // Three wrong ones from the agent: its next one waits 4 s.
    for i in 0..3 {
        let resp = unlock_as(&d, Role::Agent, &format!("falso-errada-{i}"));
        assert_eq!(error(&resp), Some(ResponseError::WrongPassword));
    }
    let agent = {
        let d = d.clone();
        std::thread::spawn(move || unlock_as(&d, Role::Agent, "falso-errada-3"))
    };
    // The agent is stuck in the 4 s wait.
    assert_eq!(
        rx_entered.recv_timeout(Duration::from_secs(20)).unwrap(),
        Duration::from_secs(4)
    );
    // The app gets it right without waiting: its counter is another one, and the agent's
    // turn does not hold it.
    assert!(ok(&unlock_as(&d, Role::App, PASSWORD)));
    assert!(!d.travado());
    tx_release.send(()).unwrap();
    // The agent wakes up with the vault already open.
    assert!(ok(&agent.join().unwrap()));
    let s = Duration::from_secs;
    assert_eq!(
        *waits.lock().unwrap(),
        [
            (s(0), false),
            (s(1), false),
            (s(2), false),
            (s(4), true),
            (s(0), false)
        ],
        "o app não esperou nada"
    );
    // Both reset (on disk too).
    assert_eq!(failures_on_disk(&dir), 0);
    d.travar();
    waits.lock().unwrap().clear();
    let _ = unlock_as(&d, Role::Agent, "falso-errada-4");
    let _ = unlock_as(&d, Role::App, "falso-errada-5");
    // An app error does not delay the agent, and vice versa.
    let _ = unlock_as(&d, Role::Agent, "falso-errada-6");
    assert_eq!(
        *waits.lock().unwrap(),
        [(s(0), false), (s(0), false), (s(1), false)]
    );
}

/// V3.33 (D62): the state in the old format (three lines) is read as counter 0, and the
/// first wrong password rewrites it in the new format; the file has no password, PIN or
/// secret, only numbers.
#[test]
fn v3_33_old_format_and_state_without_secret() {
    let dir = Dir::new("d62-antigo");
    locked_vault(&dir);
    let g = generations(&dir);
    fs::write(
        dir.paths().estado,
        format!(
            "ward-estado 1\nvista {}\nqueimada {}\n",
            g.vista, g.queimada
        ),
    )
    .unwrap();
    let r = MovableClock::default();
    let d = movable_daemon(dir.paths(), &r);
    let _ = unlock_as(&d, Role::Agent, "falso-errada-01");
    assert_eq!(r.waits(), [Duration::ZERO]);
    assert_eq!(
        state_text(&dir),
        format!(
            "ward-estado 1\nvista {}\nqueimada {}\nfalhas_agente 1\n",
            g.vista, g.queimada
        )
    );
    // The app does not go to disk.
    let _ = unlock_as(&d, Role::App, "falso-errada-02");
    assert_eq!(failures_on_disk(&dir), 1);
    let text = fs::read(dir.paths().estado).unwrap();
    for s in SECRETS
        .iter()
        .chain([&PASSWORD, &PIN, &"falso-errada-01", &"falso-errada-02"])
    {
        assert!(!contains(&text, s.as_bytes()), "{s}");
        assert!(
            !contains(&text, b64(s.as_bytes()).as_bytes()),
            "{s} em base64"
        );
    }
    assert!(
        String::from_utf8(text).unwrap().lines().skip(1).all(|l| l
            .split_once(' ')
            .is_some_and(|(_, n)| n.parse::<u64>().is_ok())),
        "só números depois do cabeçalho"
    );
    // The wait is still the same D20 table.
    assert_eq!(espera_destrave(1), Duration::from_secs(1));
}

/// V3.35 (D62, S7): state with seen = burned = 7 and no vault: the new vault is
/// born at generation 8 and the state goes to 8/8; the copy of an earlier vault at
/// generation 5, put in place, is refused as a rollback.
#[test]
fn v3_35_create_vault_with_old_state_and_old_copy() {
    let dir = Dir::new("d62-criar");
    let c = dir.paths();
    // An earlier vault, at generation 5, with the same test password (the attacker kept
    // it).
    let old = dir.0.join("antigo.ward");
    let password = MasterPassword::new(PASSWORD.to_owned());
    let mut stale = OpenVault::new(
        &password,
        VaultConfig::new(PinHash::create(PIN.as_bytes()).unwrap()),
    )
    .unwrap()
    .after_generation(4);
    ward_core::atomic_write::write_entries(&old, &mut stale, &[]).unwrap();
    assert_eq!(generation_on_disk(&old, PASSWORD), 5);
    arquivos::gravar_estado(
        &c.estado,
        Geracoes {
            vista: 7,
            queimada: 7,
        },
    )
    .unwrap();
    assert!(!c.cofre.exists());

    let (d, _) = daemon_at(c.clone());
    let create = request(
        &d,
        Role::App,
        "criar_cofre",
        &format!(
            r#"{{"senha":"{}","pin":"{}"}}"#,
            b64(PASSWORD.as_bytes()),
            b64(PIN.as_bytes())
        ),
    );
    assert!(ok(&create), "{create:?}");
    assert_eq!(generation_on_disk(&c.cofre, PASSWORD), 8);
    assert_eq!(
        generations(&dir),
        Geracoes {
            vista: 8,
            queimada: 8
        }
    );

    // The attacker puts the old copy in place and unlocks.
    d.travar();
    fs::copy(&old, &c.cofre).unwrap();
    let r = unlock(&d, PASSWORD);
    assert_eq!(error(&r), Some(ResponseError::Rollback));
    assert_eq!(ResponseError::Rollback.mensagem(), MSG_ROLLBACK);
    assert!(d.travado());
    // Not even a new daemon opens it.
    let (d2, _) = daemon_at(c.clone());
    assert_eq!(
        error(&unlock(&d2, PASSWORD)),
        Some(ResponseError::Rollback)
    );
}

/// V3.35 (D62): without a state, the vault is born at generation 1 (the usual first
/// use); with an unreadable state and no vault, the vault creation refuses instead
/// of starting from zero.
#[test]
fn v3_35_create_vault_without_state_and_with_invalid_state() {
    let dir = Dir::new("d62-criar-sem");
    let c = dir.paths();
    let create = format!(
        r#"{{"senha":"{}","pin":"{}"}}"#,
        b64(PASSWORD.as_bytes()),
        b64(PIN.as_bytes())
    );
    fs::write(&c.estado, "ward-estado 1\nvista 9\n").unwrap();
    let (d, _) = daemon_at(c.clone());
    assert_eq!(
        error(&request(&d, Role::App, "criar_cofre", &create)),
        Some(ResponseError::WriteFailed)
    );
    assert!(!c.cofre.exists());
    fs::remove_file(&c.estado).unwrap();
    assert!(ok(&request(&d, Role::App, "criar_cofre", &create)));
    assert_eq!(generation_on_disk(&c.cofre, PASSWORD), 1);
    assert_eq!(
        generations(&dir),
        Geracoes {
            vista: 1,
            queimada: 1
        }
    );
}

/// Swaps the vault file for a copy with the same bytes in another inode (what a process
/// of the same user would do right after the rename).
#[cfg(feature = "test-hooks")]
fn swap_for_copy(c: &Caminhos) {
    let bytes = fs::read(&c.cofre).unwrap();
    let tmp = c.cofre.with_extension("troca");
    fs::write(&tmp, &bytes).unwrap();
    fs::rename(&tmp, &c.cofre).unwrap();
}

/// V3.35 (D62, S7, S13): a hook swaps the file right after the rename of
/// `write_entries`. The identity the daemon keeps is the one of the temp file's fd, so
/// the next write answers "the vault file changed outside Ward", locks and does not
/// write. With the `stat` after the rename, the swap would pass as Ward's own.
#[cfg(feature = "test-hooks")]
#[test]
fn v3_35_swap_right_after_the_rename_is_refused() {
    let dir = Dir::new("d62-rename");
    locked_vault(&dir);
    let c = dir.paths();
    let armed = Arc::new(AtomicBool::new(false));
    let swapped = Arc::new(AtomicBool::new(false));
    let d = {
        let (armed, swapped, c) = (armed.clone(), swapped.clone(), c.clone());
        Daemon::novo(c.clone(), Box::new(FakeClock::default())).com_gancho_depois_de_gravar(
            Box::new(move || {
                if armed.swap(false, Ordering::SeqCst) {
                    swap_for_copy(&c);
                    swapped.store(true, Ordering::SeqCst);
                }
            }),
        )
    };
    assert!(ok(&unlock(&d, PASSWORD)));
    // Without a swap, writing twice works.
    assert!(ok(&request(
        &d,
        Role::App,
        "tempo_trava",
        r#"{"minutos":30}"#
    )));
    armed.store(true, Ordering::SeqCst);
    assert!(ok(&request(
        &d,
        Role::App,
        "tempo_trava",
        r#"{"minutos":31}"#
    )));
    assert!(swapped.load(Ordering::SeqCst), "o gancho não rodou");
    let swapped_bytes = fs::read(&c.cofre).unwrap();
    let r = request(&d, Role::App, "tempo_trava", r#"{"minutos":32}"#);
    assert_eq!(error(&r), Some(ResponseError::VaultChanged));
    assert_eq!(
        ResponseError::VaultChanged.mensagem(),
        ward_core::protocol::MSG_VAULT_CHANGED
    );
    assert!(d.travado());
    assert_eq!(
        fs::read(&c.cofre).unwrap(),
        swapped_bytes,
        "gravou por cima da troca"
    );

    // The same in the password change: the identity comes from the fd of its write.
    let dir2 = Dir::new("d62-rename-senha");
    locked_vault(&dir2);
    let c2 = dir2.paths();
    let armed2 = Arc::new(AtomicBool::new(false));
    let d2 = {
        let (armed2, c2) = (armed2.clone(), c2.clone());
        Daemon::novo(c2.clone(), Box::new(FakeClock::default())).com_gancho_depois_de_gravar(
            Box::new(move || {
                if armed2.swap(false, Ordering::SeqCst) {
                    swap_for_copy(&c2);
                }
            }),
        )
    };
    assert!(ok(&unlock(&d2, PASSWORD)));
    armed2.store(true, Ordering::SeqCst);
    let new_password = "falso-nova-ward-03";
    assert!(ok(&request(
        &d2,
        Role::App,
        "trocar_senha",
        &format!(
            r#"{{"atual":"{}","nova":"{}"}}"#,
            b64(PASSWORD.as_bytes()),
            b64(new_password.as_bytes())
        ),
    )));
    let swapped_bytes = fs::read(&c2.cofre).unwrap();
    let r = request(&d2, Role::App, "tempo_trava", r#"{"minutos":33}"#);
    assert_eq!(error(&r), Some(ResponseError::VaultChanged));
    assert!(d2.travado());
    assert_eq!(fs::read(&c2.cofre).unwrap(), swapped_bytes);
}
