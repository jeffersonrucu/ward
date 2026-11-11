//! Attack tests of the daemon state (Phase 22): lock, unlock with a growing wait, PIN,
//! level, agent list, generations and file identity.
//!
//! The daemon runs in process, with a fake clock and its own test vault in `target/tmp`
//! (fake values). The role is the one the socket would give: the request goes through
//! the same protocol parsing.

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use common::*;
use ward_core::entry::AccessLevel;
use ward_core::protocol::{ResponseError, Role, Resposta};
use wardd::arquivos::{self, Geracoes};
use wardd::nucleo::{ESPERA_MAXIMA, espera_destrave};

fn list(d: &wardd::nucleo::Daemon, role: Role) -> Vec<ward_core::protocol::ItemLista> {
    match request(d, role, "lista", "{}") {
        Resposta::Lista(l) => l,
        r => panic!("{r:?}"),
    }
}

fn level_of(d: &wardd::nucleo::Daemon, name: &str) -> Option<AccessLevel> {
    list(d, Role::App)
        .into_iter()
        .find(|i| i.nome == name)
        .and_then(|i| i.nivel)
}

fn is_root() -> bool {
    ward_platform::effective_uid() == 0
}

/// V3.10 and V3.8 (state): after locking, every operation answers "locked" until it is
/// unlocked again; the daemon no longer keeps the entries.
#[test]
fn v3_10_lock_and_everything_answers_locked() {
    let dir = Dir::new("travar");
    let (d, _) = test_vault(&dir);
    assert!(!d.travado());
    assert!(ok(&request(&d, Role::Agent, "travar", "{}")));
    assert!(d.travado());
    let pin = b64(PIN.as_bytes());
    for (role, op, data) in [
        (Role::Agent, "lista", "{}".to_owned()),
        (Role::App, "lista", "{}".to_owned()),
        (Role::App, "detalhe", r#"{"entrada":"t-aprov"}"#.to_owned()),
        (
            Role::App,
            "remover_entrada",
            r#"{"entrada":"t-aprov"}"#.to_owned(),
        ),
        (Role::App, "tempo_trava", r#"{"minutos":30}"#.to_owned()),
        (
            Role::App,
            "mudar_nivel",
            format!(r#"{{"entrada":"t-aprov","nivel":"livre","pin":"{pin}"}}"#),
        ),
        (Role::App, "salvar_entrada", login("t-novo", SECRETS[0])),
        (
            Role::Agent,
            "executar",
            format!(
                r#"{{"entrada":"t-livre","argv":["{}"],"cwd":"{}","tipo_entrada":"nula","ambiente":[]}}"#,
                b64(b"true"),
                b64(b"/")
            ),
        ),
    ] {
        assert_eq!(
            error(&request(&d, role, op, &data)),
            Some(ResponseError::Locked),
            "{op}"
        );
    }
    match request(&d, Role::Agent, "estado", "{}") {
        Resposta::Estado {
            travado,
            cofre_existe,
            papel: role,
        } => assert!(travado && cofre_existe && role == Role::Agent),
        r => panic!("{r:?}"),
    }
    assert!(ok(&unlock(&d, PASSWORD)));
    assert_eq!(list(&d, Role::App).len(), 5);
}

/// V3.9 (D20): 10 wrong passwords in a row; each attempt waits longer than the previous
/// one, up to 30 s, and all answers are byte-for-byte equal (none says how many are
/// left). The right one resets the wait.
#[test]
fn v3_9_wrong_password_growing_wait_without_count() {
    let expected: Vec<u64> = vec![0, 1, 2, 4, 8, 16, 30, 30, 30, 30];
    for (i, s) in expected.iter().enumerate() {
        assert_eq!(espera_destrave(i as u32), Duration::from_secs(*s));
    }
    assert_eq!(espera_destrave(u32::MAX), ESPERA_MAXIMA);

    let dir = Dir::new("espera");
    let (d, clock) = test_vault(&dir);
    d.travar();
    let mut frames = Vec::new();
    for i in 0..10 {
        let r = unlock(&d, &format!("falso-errada-{i:02}"));
        assert_eq!(error(&r), Some(ResponseError::WrongPassword));
        frames.push(encoded(Role::Agent, &r));
    }
    assert!(frames.windows(2).all(|w| w[0] == w[1]));
    let s = String::from_utf8_lossy(&frames[0]);
    assert!(!s.chars().any(|c| c.is_ascii_digit() && c != '1'), "{s}");
    let waits: Vec<u64> = clock
        .0
        .lock()
        .unwrap()
        .iter()
        .map(|d| d.as_secs())
        .collect();
    assert_eq!(waits, expected);
    assert!(waits.windows(2).all(|w| w[1] >= w[0]) && waits[6] == 30);
    // The right one waits the 30 s and unlocks; afterwards, the wait goes back to 0.
    assert!(ok(&unlock(&d, PASSWORD)));
    d.travar();
    let _ = unlock(&d, "falso-errada-xx");
    let waits = clock.0.lock().unwrap().clone();
    assert_eq!(waits[10], ESPERA_MAXIMA);
    assert_eq!(waits[11], Duration::ZERO);
}

/// V3.11 (S9): changing the level without a PIN or with a wrong PIN is refused and the
/// file does not change; the agent with the right PIN gets "not authorized".
#[test]
fn v3_11_change_level_requires_pin_and_app_role() {
    let dir = Dir::new("nivel");
    let (d, _) = test_vault(&dir);
    let vault = dir.paths().cofre;
    let before = fs::read(&vault).unwrap();

    // No PIN: the request parsing refuses.
    let no_pin = request(
        &d,
        Role::App,
        "mudar_nivel",
        r#"{"entrada":"t-aprov","nivel":"bloqueado"}"#,
    );
    assert_eq!(error(&no_pin), Some(ResponseError::InvalidRequest));
    // Wrong PIN.
    let r = set_level(&d, Role::App, "t-aprov", "bloqueado", WRONG_PIN);
    assert_eq!(error(&r), Some(ResponseError::WrongPin));
    // Agent, right PIN: not authorized, by the parsing and by the second layer.
    let r = set_level(&d, Role::Agent, "t-aprov", "bloqueado", PIN);
    assert_eq!(error(&r), Some(ResponseError::Unauthorized));
    let p = req(
        Role::App,
        "mudar_nivel",
        &format!(
            r#"{{"entrada":"t-aprov","nivel":"bloqueado","pin":"{}"}}"#,
            b64(PIN.as_bytes())
        ),
    )
    .unwrap();
    assert_eq!(
        error(&d.atender(Role::Agent, p)),
        Some(ResponseError::Unauthorized)
    );
    assert_eq!(fs::read(&vault).unwrap(), before, "o arquivo mudou");
    assert_eq!(level_of(&d, "t-aprov"), Some(AccessLevel::Approval));

    // App with the right PIN: changes and stays on disk.
    assert!(ok(&set_level(&d, Role::App, "t-aprov", "bloqueado", PIN)));
    assert_ne!(fs::read(&vault).unwrap(), before);
    d.travar();
    assert!(ok(&unlock(&d, PASSWORD)));
    assert_eq!(level_of(&d, "t-aprov"), Some(AccessLevel::Blocked));
}

/// V3.12 (S9): 5 PIN errors in a row lock, counting across connections (the counter
/// belongs to the daemon); 4 errors and 1 success reset the count.
#[test]
fn v3_12_five_pin_errors_lock() {
    let dir = Dir::new("pin");
    let (d, _) = test_vault(&dir);
    for _ in 0..4 {
        let r = set_level(&d, Role::App, "t-aprov", "livre", WRONG_PIN);
        assert_eq!(error(&r), Some(ResponseError::WrongPin));
    }
    assert!(!d.travado());
    assert!(ok(&set_level(
        &d,
        Role::App,
        "t-aprov",
        "com-aprovacao",
        PIN
    )));
    // The count was reset: 4 more errors do not lock (changing the PIN also counts).
    for _ in 0..4 {
        let r = request(
            &d,
            Role::App,
            "trocar_pin",
            &format!(
                r#"{{"atual":"{}","novo":"{}"}}"#,
                b64(WRONG_PIN.as_bytes()),
                b64(b"2468")
            ),
        );
        assert_eq!(error(&r), Some(ResponseError::WrongPin));
    }
    assert!(!d.travado());
    let r = set_level(&d, Role::App, "t-aprov", "livre", WRONG_PIN);
    assert_eq!(error(&r), Some(ResponseError::WrongPin));
    assert!(d.travado(), "o 5.º erro seguido tem de travar");
    assert_eq!(
        error(&request(&d, Role::App, "lista", "{}")),
        Some(ResponseError::Locked)
    );
    // Unlocking resets the count.
    assert!(ok(&unlock(&d, PASSWORD)));
    let r = set_level(&d, Role::App, "t-aprov", "livre", WRONG_PIN);
    assert_eq!(error(&r), Some(ResponseError::WrongPin));
    assert!(!d.travado());
}

/// V3.17, V3.26 (S1, D50): the agent list has neither the blocked entry nor the level;
/// the app list has both; `run` with the blocked entry answers byte for byte the same
/// as with an entry that does not exist.
#[test]
fn v3_17_v3_26_agent_list_and_blocked_equals_missing() {
    let dir = Dir::new("lista");
    let (d, _) = test_vault(&dir);
    let agent = list(&d, Role::Agent);
    assert_eq!(agent.len(), 4);
    assert!(
        agent
            .iter()
            .all(|i| i.nome != "t-bloq" && i.nivel.is_none())
    );
    let q = encoded(Role::Agent, &Resposta::Lista(agent));
    assert!(!contains(&q, b"\"nivel\""));
    for s in SECRETS {
        assert!(!contains(&q, s.as_bytes()) && !contains(&q, b64(s.as_bytes()).as_bytes()));
    }
    let app = list(&d, Role::App);
    assert_eq!(app.len(), 5);
    assert!(app.iter().all(|i| i.nivel.is_some()));
    assert_eq!(level_of(&d, "t-bloq"), Some(AccessLevel::Blocked));

    let run = |name: &str| {
        encoded(
            Role::Agent,
            &request(
                &d,
                Role::Agent,
                "executar",
                &format!(
                    r#"{{"entrada":"{name}","argv":["{}"],"cwd":"{}","tipo_entrada":"nula","ambiente":[]}}"#,
                    b64(b"env"),
                    b64(b"/")
                ),
            ),
        )
    };
    let blocked = run("t-bloq");
    assert_eq!(blocked, run("nao-existe"));
    assert!(contains(&blocked, b"entrada_inexistente"));
    for op in ["ssh", "login", "playwright"] {
        // Phase 54b: `ssh` carries the standard input kind.
        let extra = if op == "ssh" {
            r#","tipo_entrada":"nula""#
        } else {
            ""
        };
        let a = encoded(
            Role::Agent,
            &request(
                &d,
                Role::Agent,
                op,
                &format!(r#"{{"entrada":"t-bloq"{extra}}}"#),
            ),
        );
        let b = encoded(
            Role::Agent,
            &request(
                &d,
                Role::Agent,
                op,
                &format!(r#"{{"entrada":"nao-existe"{extra}}}"#),
            ),
        );
        assert!(contains(&a, b"entrada_inexistente"), "{op}");
        assert_eq!(a, b, "{op}");
    }
}

/// V3.27 (D50, S9): a created entry starts "with approval"; creating it already "free"
/// is refused; removing and recreating goes back to "with approval"; editing keeps the
/// level.
#[test]
fn v3_27_new_entry_starts_with_approval() {
    let dir = Dir::new("nasce");
    let (d, _) = test_vault(&dir);
    assert!(ok(&request(
        &d,
        Role::App,
        "salvar_entrada",
        &login("t-nova", SECRETS[0])
    )));
    assert_eq!(level_of(&d, "t-nova"), Some(AccessLevel::Approval));
    // With a level in the draft, anywhere: refused by the parsing.
    let with_level =
        login("t-nova2", SECRETS[0]).replacen(r#""nome":"#, r#""nivel":"livre","nome":"#, 1);
    assert_eq!(
        error(&request(&d, Role::App, "salvar_entrada", &with_level)),
        Some(ResponseError::InvalidRequest)
    );
    assert!(list(&d, Role::App).iter().all(|i| i.nome != "t-nova2"));
    // Editing the free one keeps it free.
    let edit = login("t-livre", SECRETS[3]).replacen('{', r#"{"anterior":"t-livre","#, 1);
    assert!(ok(&request(&d, Role::App, "salvar_entrada", &edit)));
    assert_eq!(level_of(&d, "t-livre"), Some(AccessLevel::Free));
    // Remove and recreate with the same name: back to "with approval".
    assert!(ok(&request(
        &d,
        Role::App,
        "remover_entrada",
        r#"{"entrada":"t-livre"}"#
    )));
    assert!(ok(&request(
        &d,
        Role::App,
        "salvar_entrada",
        &login("t-livre", SECRETS[3])
    )));
    assert_eq!(level_of(&d, "t-livre"), Some(AccessLevel::Approval));
    // A repeated name is a validation error, and nothing changes.
    let r = request(
        &d,
        Role::App,
        "salvar_entrada",
        &login("t-livre", SECRETS[3]),
    );
    assert!(
        matches!(error(&r), Some(ResponseError::Validation(_))),
        "{r:?}"
    );
    // Removing a server used as a tunnel would leave the vault unable to reopen: refused.
    let r = request(&d, Role::App, "remover_entrada", r#"{"entrada":"s1"}"#);
    assert!(
        matches!(error(&r), Some(ResponseError::Validation(_))),
        "{r:?}"
    );
    let rename = server("s2").replacen('{', r#"{"anterior":"s1","#, 1);
    let r = request(&d, Role::App, "salvar_entrada", &rename);
    assert!(
        matches!(error(&r), Some(ResponseError::Validation(_))),
        "{r:?}"
    );
    assert!(list(&d, Role::App).iter().any(|i| i.nome == "s1"));
    // The vault still opens after all this.
    d.travar();
    assert!(ok(&unlock(&d, PASSWORD)));
    assert_eq!(list(&d, Role::App).len(), 6);
}

/// V3.22 (D39; S7, S9, S13): old copy on top with the daemon locked, deleted state,
/// swap by `mv` and rewrite in place with the daemon unlocked.
#[test]
fn v3_22_rollback_and_swapped_file() {
    let dir = Dir::new("rollback");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    let copy_n = fs::read(&c.cofre).unwrap();
    let n = generation_on_disk(&c.cofre, PASSWORD);
    assert!(ok(&request(
        &d,
        Role::App,
        "tempo_trava",
        r#"{"minutos":30}"#
    )));
    assert!(ok(&request(
        &d,
        Role::App,
        "tempo_trava",
        r#"{"minutos":60}"#
    )));
    assert_eq!(generation_on_disk(&c.cofre, PASSWORD), n + 2);
    assert_eq!(
        generations(&dir),
        Geracoes {
            vista: n + 2,
            queimada: n + 2
        }
    );
    d.travar();

    // `cp` of the authentic copy of n on top and unlock: rollback, stays locked and the
    // file is not rewritten.
    fs::write(&c.cofre, &copy_n).unwrap();
    let r = unlock(&d, PASSWORD);
    assert_eq!(error(&r), Some(ResponseError::Rollback));
    assert!(contains(
        &encoded(Role::Agent, &r),
        "o cofre no disco é mais antigo que o último aberto".as_bytes()
    ));
    assert!(d.travado());
    assert_eq!(fs::read(&c.cofre).unwrap(), copy_n);

    // Deleting the state: same refusal, even with the right vault back.
    fs::remove_file(&c.estado).unwrap();
    let r = unlock(&d, PASSWORD);
    assert_eq!(error(&r), Some(ResponseError::Rollback));
    assert!(d.travado());

    // State back (n), unlocks copy n: the generation goes up with the writes.
    // The file is a different one since the last lock: the first unlock warns that it
    // changed outside Ward; the next one decides by the generation only.
    arquivos::gravar_estado(
        &c.estado,
        Geracoes {
            vista: n,
            queimada: n + 2,
        },
    )
    .unwrap();
    assert_eq!(error(&unlock(&d, PASSWORD)), Some(ResponseError::VaultChanged));
    assert!(ok(&unlock(&d, PASSWORD)));
    // With the daemon unlocked, the file swapped by `mv` (new inode): does not write over
    // it, locks and warns.
    let other = dir.0.join("outro.ward");
    fs::copy(&c.cofre, &other).unwrap();
    fs::rename(&other, &c.cofre).unwrap();
    let before = fs::read(&c.cofre).unwrap();
    let r = request(&d, Role::App, "tempo_trava", r#"{"minutos":90}"#);
    assert_eq!(error(&r), Some(ResponseError::VaultChanged));
    assert!(contains(
        &encoded(Role::App, &r),
        "o arquivo do cofre mudou fora do Ward".as_bytes()
    ));
    assert!(d.travado());
    assert_eq!(fs::read(&c.cofre).unwrap(), before);

    // Rewritten in place (same inode, new `mtime`): the same.
    assert!(ok(&unlock(&d, PASSWORD)));
    std::thread::sleep(Duration::from_millis(20));
    fs::write(&c.cofre, &before).unwrap();
    let r = request(&d, Role::App, "tempo_trava", r#"{"minutos":90}"#);
    assert_eq!(error(&r), Some(ResponseError::VaultChanged));
    assert!(d.travado());
    // Swapped with the daemon locked (without changing the generation): unlocking warns once.
    assert!(ok(&unlock(&d, PASSWORD)));
    d.travar();
    std::thread::sleep(Duration::from_millis(20));
    fs::write(&c.cofre, &before).unwrap();
    assert_eq!(error(&unlock(&d, PASSWORD)), Some(ResponseError::VaultChanged));
    assert!(ok(&unlock(&d, PASSWORD)));

    // The state is 0600, with no secret.
    let m = fs::metadata(&c.estado).unwrap();
    assert_eq!(m.permissions().mode() & 0o777, 0o600);
    let text = fs::read(&c.estado).unwrap();
    for s in SECRETS.iter().chain([&PASSWORD, &PIN]) {
        assert!(!contains(&text, s.as_bytes()));
    }
}

/// V3.28 (D53; S7, S9): a write that fails after the rename (directory fsync, through
/// the 0300 directory), restart between A and B, a state that does not write and a
/// password change that fails after sealing.
#[test]
fn v3_28_burned_generation_in_the_state() {
    if is_root() {
        eprintln!("v3_28: o root lê diretório 0300; o caso não se reproduz como root");
        return;
    }
    let dir = Dir::new("queimada");
    let state_dir = Dir::new("queimada-estado");
    let mut c = dir.paths();
    c.estado = state_dir.0.join("estado");
    // The test vault is born in the default state directory; moves to the separate one.
    let (d0, _) = test_vault(&dir);
    drop(d0);
    fs::copy(dir.paths().estado, &c.estado).unwrap();
    let (d, _) = daemon_at(c.clone());
    assert!(ok(&unlock(&d, PASSWORD)));
    let n = generation_on_disk(&c.cofre, PASSWORD);
    assert_eq!(
        generations_at(&c.estado),
        Geracoes {
            vista: n,
            queimada: n
        }
    );

    // A: the rename happens, the directory fsync fails.
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o300)).unwrap();
    let r = request(&d, Role::App, "tempo_trava", r#"{"minutos":30}"#);
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(error(&r), Some(ResponseError::WriteFailed));
    assert_eq!(
        generations_at(&c.estado),
        Geracoes {
            vista: n,
            queimada: n + 1
        }
    );
    assert_eq!(generation_on_disk(&c.cofre, PASSWORD), n + 1);
    let copy_a = fs::read(&c.cofre).unwrap();

    // Restart between A and B: the new daemon opens with (n, n+1) and B writes n+2.
    drop(d);
    let (d, _) = daemon_at(c.clone());
    assert!(ok(&unlock(&d, PASSWORD)));
    assert_eq!(
        generations_at(&c.estado),
        Geracoes {
            vista: n + 1,
            queimada: n + 1
        }
    );
    assert!(ok(&request(
        &d,
        Role::App,
        "tempo_trava",
        r#"{"minutos":45}"#
    )));
    assert_eq!(generation_on_disk(&c.cofre, PASSWORD), n + 2);
    assert_eq!(
        generations_at(&c.estado),
        Geracoes {
            vista: n + 2,
            queimada: n + 2
        }
    );

    // The copy of A in place: rollback.
    d.travar();
    fs::write(&c.cofre, &copy_a).unwrap();
    assert_eq!(error(&unlock(&d, PASSWORD)), Some(ResponseError::Rollback));
    assert!(d.travado());

    // No restart: A fails after the rename, B writes n+4 (n+3 was burned by A).
    let (d, _) = vault_n2(&c, &dir);
    let g = generation_on_disk(&c.cofre, PASSWORD);
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o300)).unwrap();
    let r = request(&d, Role::App, "tempo_trava", r#"{"minutos":31}"#);
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(error(&r), Some(ResponseError::WriteFailed));
    assert_eq!(
        generations_at(&c.estado),
        Geracoes {
            vista: g,
            queimada: g + 1
        }
    );
    assert!(ok(&request(
        &d,
        Role::App,
        "tempo_trava",
        r#"{"minutos":32}"#
    )));
    assert_eq!(generation_on_disk(&c.cofre, PASSWORD), g + 2);
    assert_eq!(
        generations_at(&c.estado),
        Geracoes {
            vista: g + 2,
            queimada: g + 2
        }
    );

    // The state does not write (read-only directory): the vault is not written.
    let before = fs::read(&c.cofre).unwrap();
    fs::set_permissions(&state_dir.0, fs::Permissions::from_mode(0o500)).unwrap();
    let r = request(&d, Role::App, "tempo_trava", r#"{"minutos":33}"#);
    fs::set_permissions(&state_dir.0, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(error(&r), Some(ResponseError::WriteFailed));
    assert_eq!(fs::read(&c.cofre).unwrap(), before);
    assert!(!d.travado());

    // A password change that fails after sealing: locks, and the next unlock opens (with
    // the new password, which reached the disk) above the burned one.
    let new_password = "falso-nova-02";
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o300)).unwrap();
    let r = request(
        &d,
        Role::App,
        "trocar_senha",
        &format!(
            r#"{{"atual":"{}","nova":"{}"}}"#,
            b64(PASSWORD.as_bytes()),
            b64(new_password.as_bytes())
        ),
    );
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(error(&r), Some(ResponseError::WriteFailed));
    assert!(d.travado());
    let q = generations_at(&c.estado).queimada;
    assert!(ok(&unlock(&d, new_password)));
    assert!(ok(&request(
        &d,
        Role::App,
        "tempo_trava",
        r#"{"minutos":34}"#
    )));
    assert!(generation_on_disk(&c.cofre, new_password) > q);
}

/// New daemon unlocked in the current state (after a test rollback: puts back the
/// newest vault through the state itself).
fn vault_n2(c: &wardd::nucleo::Caminhos, dir: &Dir) -> (wardd::nucleo::Daemon, FakeClock) {
    // Recreates the vault from scratch in this directory, with the separate state.
    let _ = fs::remove_file(&c.cofre);
    let _ = fs::remove_file(&c.estado);
    let (d0, _) = test_vault(dir);
    drop(d0);
    fs::copy(dir.paths().estado, &c.estado).unwrap();
    let (d, r) = daemon_at(c.clone());
    assert!(ok(&unlock(&d, PASSWORD)));
    (d, r)
}

fn generations_at(p: &std::path::Path) -> Geracoes {
    arquivos::ler_estado(p).unwrap()
}

/// V3.28: the password change with a wrong current password answers "wrong password"
/// (D52) and does not lock; the right one changes it and the vault opens only with the
/// new one.
#[test]
fn d52_change_password_with_wrong_password() {
    let dir = Dir::new("troca");
    let (d, _) = test_vault(&dir);
    let change = |current: &str, new_password: &str| {
        request(
            &d,
            Role::App,
            "trocar_senha",
            &format!(
                r#"{{"atual":"{}","nova":"{}"}}"#,
                b64(current.as_bytes()),
                b64(new_password.as_bytes())
            ),
        )
    };
    assert_eq!(
        error(&change("falso-errada-01", "falso-nova-02")),
        Some(ResponseError::WrongPassword)
    );
    assert!(!d.travado());
    assert!(ok(&change(PASSWORD, "falso-nova-02")));
    assert!(ok(&request(
        &d,
        Role::App,
        "tempo_trava",
        r#"{"minutos":20}"#
    )));
    d.travar();
    assert_eq!(
        error(&unlock(&d, PASSWORD)),
        Some(ResponseError::WrongPassword)
    );
    assert!(ok(&unlock(&d, "falso-nova-02")));
}

/// First use: without a vault, the status says it does not exist and unlocking answers
/// "missing vault"; creating it again is refused; the PIN appears neither in the state
/// nor in the file (V3.13: only the hash, inside the encrypted part).
#[test]
fn first_use_and_pin_only_as_hash() {
    let dir = Dir::new("primeiro");
    let (d, _) = daemon_at(dir.paths());
    match request(&d, Role::Agent, "estado", "{}") {
        Resposta::Estado {
            travado,
            cofre_existe,
            ..
        } => assert!(travado && !cofre_existe),
        r => panic!("{r:?}"),
    }
    assert_eq!(
        error(&unlock(&d, PASSWORD)),
        Some(ResponseError::VaultNotFound)
    );
    // The agent does not create a vault.
    let create = format!(
        r#"{{"senha":"{}","pin":"{}"}}"#,
        b64(PASSWORD.as_bytes()),
        b64(PIN.as_bytes())
    );
    assert_eq!(
        error(&request(&d, Role::Agent, "criar_cofre", &create)),
        Some(ResponseError::Unauthorized)
    );
    drop(d);
    let (d, _) = test_vault(&dir);
    assert_eq!(
        error(&request(&d, Role::App, "criar_cofre", &create)),
        Some(ResponseError::VaultAlreadyExists)
    );
    d.travar();
    assert_eq!(
        error(&request(&d, Role::App, "criar_cofre", &create)),
        Some(ResponseError::VaultAlreadyExists)
    );
    let c = dir.paths();
    for file in [&c.cofre, &c.estado] {
        let b = fs::read(file).unwrap();
        assert!(!contains(&b, PIN.as_bytes()) && !contains(&b, PASSWORD.as_bytes()));
        for s in SECRETS {
            assert!(!contains(&b, s.as_bytes()));
        }
        assert_eq!(
            fs::metadata(file).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    // Changing the PIN: the old one stops working, the new one works, and it stays on disk.
    assert!(ok(&unlock(&d, PASSWORD)));
    let r = request(
        &d,
        Role::App,
        "trocar_pin",
        &format!(
            r#"{{"atual":"{}","novo":"{}"}}"#,
            b64(PIN.as_bytes()),
            b64(b"2468")
        ),
    );
    assert!(ok(&r), "{r:?}");
    d.travar();
    assert!(ok(&unlock(&d, PASSWORD)));
    assert_eq!(
        error(&set_level(&d, Role::App, "t-aprov", "livre", PIN)),
        Some(ResponseError::WrongPin)
    );
    assert!(ok(&set_level(&d, Role::App, "t-aprov", "livre", "2468")));
}
