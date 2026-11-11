//! Fake daemon, only in the tests and in `test-mode` (D7): answers over the Phase 20
//! protocol with the fake entries of the test vault (D15 + `acme-totp-dev`, the same as
//! `dev/fixtures/entries.json`, built by `crates/ward-core/src/body/test_vault.rs`).
//!
//! It lets the interface be checked in Chrome through the test bridge before the Phase
//! 22 `wardd`. It receives the frame body, reads it with the request decoder in the app
//! role, builds the response with the protocol's list item (with the app role summary,
//! D65 c) / entry detail (no secret by construction) builders and returns the encoded
//! response body. The secret of reveal/copy/totp comes out through the protocol's
//! target lookup, the same as `wardd`'s (D65 b). It handles status, list, detail,
//! copy, reveal, totp, issue token (Phase 34) and save entry (Phase 33, with the same
//! rule as `wardd`: draft validation, the in-vault check, level "with approval" on a new
//! entry and kept on an edited one) and set level (Phase 35, the same rule as `wardd`:
//! PIN checked against the Argon2id hash of the test vault config, in constant time by
//! the library; the 5th error in a row locks, and while locked everything but status
//! answers "locked"), unlock (Phase 31: test master password checked in constant time,
//! the D20 wait before looking at the password) and answer approval (Phase 36, only the
//! requests put by [`FakeDaemon::with_approval`], with the same rule as `wardd`); the
//! rest answers "internal". The changes stay only in the process memory (locked, only by
//! restarting the bridge). In [`FakeDaemon::no_vault`], it handles create vault (Phase
//! 30): it keeps only the PIN hash (D30), which set level then checks instead of the
//! test PIN. On the Settings screen (Phase 38): config, lock timeout (1 to 1440, D45),
//! change PIN (current PIN checked as in set level, with the same error count) and change
//! password (current password checked in constant time; the new one then unlocks).
//! External backup (Phase 48): external backup, set external backup (the same check as
//! `wardd`: `ExternalBackup::new`) and USB drives, with the fake drives of
//! [`test_usb_drives`] (no real device; labels with HTML, text direction and zero width
//! for the display QA).
//! Restore (Phase 47): backups and restore backup with the same check order as `wardd`
//! (its core, restore backup): name in format, current PIN (counts toward the 5 that
//! lock), version in the folder, the version's master password (in constant time); the
//! current one becomes the newest version and the vault takes the version's entries,
//! password and PIN. The fake versions ([`test_backups`]) are copies of the test vault
//! with fewer entries, all with the test master password (D15).
//! The D65 d dates (Phase 22d) follow the `wardd` rule: a new entry gets both, an edit
//! and set level only the modification, from the process clock; the test vault ones are
//! born without dates.
//!
//! Without the `test-mode` feature this module does not even exist in the binary (tests
//! only).

use zeroize::Zeroizing;

use ward_core::config::PinHash;
use ward_core::entry::{AccessLevel, Dates, Draft, Entry, EntryType, Secret as CoreSecret};
use ward_core::protocol::{
    self, ApprovalRequest, Decision, EntryDetail, ListItem, Request, Response, ResponseError, Role,
    Target, UsbDrive,
};
use ward_core::totp::Totp;

use crate::AppError;
use crate::daemon::Daemon;

// The test vault file uses the public `ward-core` API through the `wc` alias.
use ward_core as wc;
#[path = "../../../crates/ward-core/src/body/test_vault.rs"]
mod test_vault;

/// The secret values of the test vault (the needles of the attack tests).
pub use test_vault::SECRETS;

/// Fake daemon with the entries in memory.
pub struct FakeDaemon {
    entries: Vec<Entry>,
    /// `false` only in [`FakeDaemon::no_vault`] until create vault (Phase 30).
    exists: bool,
    /// PIN hash of the vault made by create vault, as `wardd` stores it (D30).
    pin_hash: Option<PinHash>,
    /// PIN errors in a row (S9). Reset on a hit.
    pin_errors: u32,
    locked: bool,
    /// Wrong master passwords in a row from the app role (D20, D62). Reset on a hit.
    unlock_failures: u32,
    /// Sleeps the D20 wait before looking at the password, like `wardd` (the unit tests
    /// turn it off so as not to wait).
    d20_wait: bool,
    /// Lock timeout of the config (D30; default 240).
    lock_minutes: u32,
    /// Master password changed by change password (Phase 38); without it the test one
    /// holds.
    password: Option<Zeroizing<Vec<u8>>>,
    /// Pending approval requests (Phase 36): id, entries and whether the stdin is hidden
    /// (Phase 54b).
    approvals: Vec<(u64, Vec<String>, bool)>,
    /// External backup of the config (Phase 48; D30: off and no UUID by default).
    backup: wc::config::ExternalBackup,
    /// USB drives that "udisks2" sees; `None` is udisks2 with no answer.
    usb_drives: Option<Vec<UsbDrive>>,
    /// Local backup versions (Phase 47), from the newest to the oldest; `None` is the
    /// backup folder unavailable.
    backups: Option<Vec<FakeBackup>>,
    /// Generation of the current vault (Phase 49a): the one the current takes when it
    /// becomes a backup.
    generation: u64,
    /// The local backup folder is safe (Phase 49a); `false` in the insecure local backup
    /// mode.
    local_backup: bool,
}

/// A fake local backup version (Phase 47): the name, the master password that held in
/// it and the entries. Memory only.
pub struct FakeBackup {
    name: String,
    /// The sealed generation (Phase 49a); `None` like one of another master password.
    generation: Option<u64>,
    password: Zeroizing<Vec<u8>>,
    entries: BackupEntries,
}

/// The entries of a fake version. `Entry` has no `Clone` (it has secrets): the test
/// versions are built again on each restore; the vault kept on restore goes whole (and
/// leaves the list when it is restored, the only difference from `wardd`, which leaves
/// the file in the folder).
enum BackupEntries {
    /// The test vault minus its last `minus` entries.
    Test {
        minus: usize,
    },
    Stored(Vec<Entry>),
}

fn test_vault_minus(minus: usize) -> Vec<Entry> {
    let mut e = test_vault::test_vault_entries();
    e.truncate(e.len().saturating_sub(minus));
    e
}

/// How many versions stay (D17), like `wardd`.
const FAKE_VERSIONS: usize = 30;

/// The 30 fake versions of the test bridge (Phase 47), counted backwards from `now`:
/// 2 min ago, 5 h ago, yesterday, 3 and 6 days ago and then one every 9 h. Version `i`
/// has the test vault entries minus the last `min(i, 6)`, so the change shows in the
/// list. All with the test master password (D15); no file.
pub fn test_backups(now: std::time::SystemTime) -> Vec<FakeBackup> {
    use std::time::Duration;
    let min = |m: u64| Duration::from_secs(m * 60);
    let mut ages = vec![
        min(2),
        min(5 * 60 + 17),
        min(25 * 60 + 45),
        min(3 * 24 * 60 + 3 * 60 + 30),
        min(6 * 24 * 60 + 22 * 60 + 12),
    ];
    while ages.len() < FAKE_VERSIONS {
        let last = ages[ages.len() - 1];
        ages.push(last + min(9 * 60));
    }
    ages.iter()
        .enumerate()
        .filter_map(|(i, age)| {
            let name = wc::backup::Version::from_instant(now.checked_sub(*age)?)?.name();
            Some(FakeBackup {
                name,
                generation: u64::try_from(FAKE_VERSIONS - i).ok(),
                password: Zeroizing::new(test_vault::MASTER_PASSWORD.as_bytes().to_vec()),
                entries: BackupEntries::Test { minus: i.min(6) },
            })
        })
        .collect()
}

/// The fake USB drives of the test bridge (Phase 48). Data only: no device, no path. The
/// 3rd has a label with HTML, U+202E and zero width (udisks2 accepts any label); the 4th
/// has no label.
pub fn test_usb_drives() -> Vec<UsbDrive> {
    let p = |uuid: &str, label: &str, mounted| UsbDrive {
        uuid: uuid.to_owned(),
        label: label.to_owned(),
        mounted,
    };
    vec![
        p("7C1E-42A9", "WARD-BACKUP", true),
        p("3F2A-9B10", "FOTOS 2024", false),
        p(
            "a1b2c3d4-0000-4e5f-9a8b-1234567890ab",
            "<img src=x onerror=alert(1)>\u{202E}gpj.exe\u{200B}",
            true,
        ),
        p("0E9D-55C1", "", true),
    ]
}

/// Simulated approval request and how long ago it "arrived" (Phase 36, test bridge).
pub type SimulatedApproval = (ApprovalRequest, std::time::Duration);

/// The approval requests of the test bridge (approval modes of the fake daemon option),
/// for the QA of the M0 states in Chrome. All fake; no secret (the request has no field
/// for that).
/// - the plain approval mode: `acme-pg-aprovacao` (stage), like the normal approval frame of the design;
/// - the prod mode: `acme-pg-prod`, for the prod approval frame;
/// - the expiring mode: the plain one arrived 104 s ago (16 s left);
/// - the stdin mode (Phase 54b, D68): `printf <script> | ward run acme-pg-aprovacao --
///   python3` from the final audit: the stdin is a pipe, the window warns and the
///   "Allow" buttons wait for the box;
/// - the attack mode (V5.29): `sh -c` with U+202E, U+2066, zero width, `ESC[2J`, `\n`
///   imitating another command and HTML, raw (as if the daemon had not neutralized
///   them), and a `claude\u{202E}` process in the chain.
pub fn simulated_approvals(mode: &str) -> Option<Vec<SimulatedApproval>> {
    use std::time::Duration;
    use ward_core::entry::Environment;
    use ward_core::protocol::Operation;
    let request = |id, entry: &str, environment, command: &str, chain: &[&str]| ApprovalRequest {
        id,
        entries: vec![entry.to_owned()],
        environment,
        operation: Operation::Run,
        command: command.to_owned(),
        directory: "\"/home/dev/projetos/acme\"".to_owned(),
        chain: chain.iter().map(|c| (*c).to_owned()).collect(),
        origin: String::new(),
        confirmation: false,
        hidden_entry: false,
    };
    // Phase 44c (D9): fill on an `http` origin.
    // Phase 44e: environment and origin of the real test vault entry (`acme-wp-dev` is
    // dev, `acme-wp-prod` is prod), as `wardd` would build them.
    let at_origin = |id, entry: &str, operation, confirmation, chain: &[&str]| {
        let entries = test_vault::test_vault_entries();
        let e = entries.iter().find(|e| e.name() == entry)?;
        let ward_core::entry::Data::Login(l) = e.data() else {
            return None;
        };
        Some(ApprovalRequest {
            id,
            entries: vec![entry.to_owned()],
            environment: e.environment(),
            operation,
            command: String::new(),
            directory: String::new(),
            chain: chain.iter().map(|c| (*c).to_owned()).collect(),
            origin: ward_core::entry::url_origin(&l.url)?,
            confirmation,
            hidden_entry: false,
        })
    };
    let psql = |entry: &str| {
        ward_core::display::display_args(
            &[
                "ward",
                "run",
                entry,
                "--",
                "psql",
                "-c",
                "select count(*) from pedidos",
            ],
            64 * 1024,
        )
    };
    let chain = ["claude", "bash", "ward run"];
    let normal = request(
        1,
        "acme-pg-aprovacao",
        Environment::Stage,
        &psql("acme-pg-aprovacao"),
        &chain,
    );
    Some(match mode {
        "aprovacao" => vec![(normal, Duration::ZERO)],
        "aprovacao-prod" => vec![(
            request(
                2,
                "acme-pg-prod",
                Environment::Prod,
                &psql("acme-pg-prod"),
                &chain,
            ),
            Duration::ZERO,
        )],
        "aprovacao-expirando" => vec![(normal, Duration::from_secs(104))],
        "aprovacao-entrada" => vec![(
            ApprovalRequest {
                hidden_entry: true,
                ..request(
                    7,
                    "acme-pg-aprovacao",
                    Environment::Stage,
                    &ward_core::display::display_args(&["python3"], 64 * 1024),
                    &chain,
                )
            },
            Duration::ZERO,
        )],
        // Phase 44c: `ward playwright acme-wp-prod` (free) on an `http` origin: only the
        // confirmation, no PIN.
        "aprovacao-origem" => vec![(
            at_origin(
                4,
                "acme-wp-prod",
                Operation::Playwright,
                true,
                &["claude", "bash", "ward playwright"],
            )?,
            Duration::ZERO,
        )],
        // Phase 44c: the extension popup on an `http` origin.
        "aprovacao-origem-popup" => vec![(
            at_origin(
                5,
                "acme-wp-prod",
                Operation::Fill,
                true,
                &["chrome", "ward-nm"],
            )?,
            Duration::ZERO,
        )],
        // Phase 44c: `ward login acme-wp-dev` (with approval) on an `http` origin: PIN
        // and origin.
        "aprovacao-origem-pin" => vec![(
            at_origin(
                6,
                "acme-wp-dev",
                Operation::Login,
                false,
                &["claude", "bash", "ward login"],
            )?,
            Duration::ZERO,
        )],
        "aprovacao-ataque" => vec![(
            request(
                3,
                "acme-pg-aprovacao",
                Environment::Stage,
                "\"ward\" \"run\" \"acme-pg-aprovacao\" \"--\" \"sh\" \"-c\" \"echo <b>ok</b>\u{202E}txt.exe\u{2066}x\u{200B}y\u{1b}[2J\nward run acme-pg-dev -- ls\"",
                &["claude\u{202E}", "bash", "ward run"],
            ),
            Duration::ZERO,
        )],
        _ => return None,
    })
}

/// PIN errors in a row that lock the vault (the same limit as `wardd`).
pub const MAX_PIN_ERRORS: u32 = 5;

impl FakeDaemon {
    /// The 21 entries of the test vault.
    pub fn full() -> FakeDaemon {
        FakeDaemon {
            entries: test_vault::test_vault_entries(),
            backups: Some(test_backups(std::time::SystemTime::now())),
            generation: FAKE_VERSIONS as u64 + 1,
            ..FakeDaemon::empty()
        }
    }

    /// Phase 49a: the full one with an old copy (generation 3) planted with the name of
    /// 1 min from now, within the clock margin: it shows on top, flagged by the
    /// generation.
    pub fn planted_backup() -> FakeDaemon {
        let mut d = FakeDaemon::full();
        let at = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
        if let (Some(list), Some(v)) = (d.backups.as_mut(), wc::backup::Version::from_instant(at)) {
            list.insert(
                0,
                FakeBackup {
                    name: v.name(),
                    generation: Some(3),
                    password: Zeroizing::new(test_vault::MASTER_PASSWORD.as_bytes().to_vec()),
                    entries: BackupEntries::Test { minus: 6 },
                },
            );
        }
        d
    }

    /// Phase 49a: the full one with the local backup folder insecure (the "Local backup"
    /// line of Settings warns).
    pub fn insecure_local_backup() -> FakeDaemon {
        FakeDaemon {
            local_backup: false,
            ..FakeDaemon::full()
        }
    }

    /// Unlocked vault with no entry (the empty credentials screen).
    pub fn empty() -> FakeDaemon {
        FakeDaemon {
            entries: Vec::new(),
            exists: true,
            pin_hash: None,
            pin_errors: 0,
            locked: false,
            unlock_failures: 0,
            d20_wait: true,
            lock_minutes: wc::limits::DEFAULT_LOCK_MINUTES,
            password: None,
            approvals: Vec::new(),
            backup: wc::config::ExternalBackup::disabled(),
            usb_drives: Some(test_usb_drives()),
            backups: Some(Vec::new()),
            generation: 1,
            local_backup: true,
        }
    }

    /// Phase 48: the test vault with the external backup on `WARD-BACKUP`
    /// (the settings screen with the external backup on).
    pub fn external_backup_enabled() -> FakeDaemon {
        let mut d = FakeDaemon::full();
        d.backup = wc::config::ExternalBackup::new(true, Some("7C1E-42A9"))
            .unwrap_or_else(|_| wc::config::ExternalBackup::disabled());
        d
    }

    /// Phase 48: the USB drives the fake one shows; `None` is udisks2 with no answer.
    pub fn with_usb_drives(mut self, usb_drives: Option<Vec<UsbDrive>>) -> FakeDaemon {
        self.usb_drives = usb_drives;
        self
    }

    /// Phase 47: the local backup versions; `None` is the folder unavailable.
    pub fn with_backups(mut self, backups: Option<Vec<FakeBackup>>) -> FakeDaemon {
        self.backups = backups;
        self
    }

    /// Names of the local backup versions (tests only).
    pub fn backup_names(&self) -> Vec<String> {
        self.backups
            .iter()
            .flatten()
            .map(|b| b.name.clone())
            .collect()
    }

    /// How many entries the vault has (tests only).
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// PIN errors in a row (tests only).
    pub fn pin_errors(&self) -> u32 {
        self.pin_errors
    }

    /// External backup of the config (tests only).
    pub fn backup(&self) -> (bool, Option<String>) {
        (self.backup.enabled(), self.backup.uuid().map(str::to_owned))
    }

    /// Phase 31: the test vault entries with the vault locked (the unlock screens). The
    /// master password that unlocks is the test one (D15); the check is in constant
    /// time.
    pub fn locked() -> FakeDaemon {
        FakeDaemon {
            locked: true,
            ..FakeDaemon::full()
        }
    }

    /// Without the D20 wait (unit tests only).
    #[cfg(test)]
    pub(crate) fn without_wait(mut self) -> FakeDaemon {
        self.d20_wait = false;
        self
    }

    /// The same rule as `wardd` (the `wardd` core, unlock): the D20 wait
    /// comes before looking at the password; a hit resets the failures (password and
    /// PIN).
    fn unlock(&mut self, password: &CoreSecret) -> Result<(), ResponseError> {
        if !self.locked {
            return Ok(());
        }
        if self.d20_wait && self.unlock_failures > 0 {
            let seconds = 1u64
                .checked_shl(self.unlock_failures - 1)
                .unwrap_or(u64::MAX)
                .min(30);
            std::thread::sleep(std::time::Duration::from_secs(seconds));
        }
        if !self.password_matches(password) {
            self.unlock_failures = self.unlock_failures.saturating_add(1);
            return Err(ResponseError::WrongPassword);
        }
        self.unlock_failures = 0;
        self.pin_errors = 0;
        self.locked = false;
        Ok(())
    }

    /// No vault yet (the first use screens, Phase 30): status answers "vault does not
    /// exist", the rest answers "no vault yet" until create vault, which leaves the vault
    /// empty and unlocked, like `wardd`.
    pub fn no_vault() -> FakeDaemon {
        FakeDaemon {
            exists: false,
            ..FakeDaemon::empty()
        }
    }

    /// The master password matches, in constant time: the one changed by change password
    /// or, with no change, the test one (D15).
    fn password_matches(&self, password: &CoreSecret) -> bool {
        use ctutils::CtEq;
        let right: &[u8] = match &self.password {
            Some(s) => s,
            None => test_vault::MASTER_PASSWORD.as_bytes(),
        };
        password.expose().len() == right.len() && password.expose().ct_eq(right).to_bool()
    }

    /// Phase 36: a pending approval request, as if `wardd` had sent it to the app (the
    /// notice goes to the service queue through whoever builds the bridge).
    pub fn with_approval(mut self, p: &ApprovalRequest) -> FakeDaemon {
        self.approvals
            .push((p.id, p.entries.clone(), p.hidden_entry));
        self
    }

    /// Ids of the approval requests still waiting (tests only).
    pub fn pending_approvals(&self) -> Vec<u64> {
        self.approvals.iter().map(|(id, _, _)| *id).collect()
    }

    /// Level of an entry (tests only).
    pub fn level_of(&self, name: &str) -> Option<AccessLevel> {
        self.entry(name).map(Entry::level)
    }

    /// The PIN matches the hash kept by create vault (tests only).
    pub fn pin_matches(&self, pin: &[u8]) -> bool {
        self.pin_hash.as_ref().is_some_and(|h| h.verify(pin))
    }

    fn entry(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.name() == name)
    }

    fn handle(&mut self, request: Request) -> Response {
        if !self.exists {
            return match request {
                Request::Status => Response::Status {
                    locked: true,
                    vault_exists: false,
                    role: Role::App,
                },
                // Like `wardd` (its core, create vault): only the PIN hash stays.
                Request::CreateVault { password: _, pin } => match PinHash::create(pin.expose()) {
                    Ok(h) => {
                        self.pin_hash = Some(h);
                        self.exists = true;
                        Response::Ok
                    }
                    Err(_) => Response::Error(ResponseError::InvalidRequest),
                },
                _ => Response::Error(ResponseError::VaultNotFound),
            };
        }
        if self.locked
            && !matches!(
                request,
                Request::Status | Request::Unlock { .. } | Request::Lock
            )
        {
            return Response::Error(ResponseError::Locked);
        }
        match request {
            Request::Status => Response::Status {
                locked: self.locked,
                vault_exists: true,
                role: Role::App,
            },
            Request::Unlock { password: senha } => match self.unlock(&senha) {
                Ok(()) => Response::Ok,
                Err(e) => Response::Error(e),
            },
            Request::Lock => {
                self.locked = true;
                Response::Ok
            }
            Request::CreateVault { .. } => Response::Error(ResponseError::VaultAlreadyExists),
            Request::List => Response::List(
                self.entries
                    .iter()
                    .map(|e| ListItem::from_entry(e, Role::App))
                    .collect(),
            ),
            Request::Detail { entry: entrada } => match self.entry(&entrada) {
                Some(e) => Response::Detail(EntryDetail::from_entry(e)),
                None => Response::Error(ResponseError::EntryNotFound),
            },
            Request::Copy {
                entry: entrada,
                target: alvo,
            }
            | Request::Reveal {
                entry: entrada,
                target: alvo,
            } => match self.entry(&entrada) {
                Some(e) => match alvo.secret_in(e) {
                    Some(v) => Response::Secret(CoreSecret::from_bytes(v.expose().to_vec())),
                    None => Response::Error(ResponseError::FieldNotFound),
                },
                None => Response::Error(ResponseError::EntryNotFound),
            },
            Request::Totp { entry: entrada } => match self.entry(&entrada) {
                Some(e) => match Target::Field("totp_semente".to_owned()).secret_in(e) {
                    Some(seed) => match Totp::from_seed(seed).and_then(|t| t.code_now()) {
                        Ok(c) => Response::TotpCode {
                            code: Zeroizing::new(c.code().to_owned()),
                            remaining: c.remaining(),
                        },
                        Err(_) => Response::Error(ResponseError::Internal),
                    },
                    None => Response::Error(ResponseError::FieldNotFound),
                },
                None => Response::Error(ResponseError::EntryNotFound),
            },
            Request::SaveEntry {
                previous: anterior,
                draft: rascunho,
                keep_attachments: manter_anexos,
            } => match self.save(anterior.as_deref(), rascunho, &manter_anexos) {
                Ok(()) => Response::Ok,
                Err(e) => Response::Error(e),
            },
            Request::SetLevel {
                entry: entrada,
                level: nivel,
                pin,
            } => match self.set_level(&entrada, nivel, &pin) {
                Ok(()) => Response::Ok,
                Err(e) => Response::Error(e),
            },
            // Phase 34: the "Connect" token (D3), only for a server. The fake one opens
            // no session; the token is random and not kept.
            // Phase 36: the same rule as `wardd` (its use module, answer approval).
            Request::AnswerApproval {
                request: pedido,
                decision: decisao,
                pin,
                confirmed_entry: entrada_confirmada,
            } => match self.answer_approval(pedido, decisao, pin.as_ref(), entrada_confirmada) {
                Ok(()) => Response::Ok,
                Err(e) => Response::Error(e),
            },
            Request::IssueToken { entry: entrada } => match self.entry(&entrada) {
                Some(e) if e.kind() == EntryType::Server => {
                    match ward_core::generator::generate(&ward_core::generator::GeneratorOptions {
                        size: 32,
                        lowercase: true,
                        uppercase: true,
                        digits: true,
                        symbols: false,
                    }) {
                        Ok(t) => Response::Token(t),
                        Err(_) => Response::Error(ResponseError::Internal),
                    }
                }
                Some(_) => Response::Error(ResponseError::InvalidRequest),
                None => Response::Error(ResponseError::EntryNotFound),
            },
            // Phase 38: the config and the changes (no hash nor UUID in the response).
            Request::Config => Response::Config {
                lock_minutes: self.lock_minutes,
                external_backup: self.backup.enabled(),
                local_backup: self.local_backup,
            },
            // Phase 48: the same rule as `wardd` (Phase 46): only the D45 format; "on"
            // requires a UUID. Nothing is copied (there is no real USB drive).
            Request::ExternalBackup => Response::ExternalBackup {
                enabled: self.backup.enabled(),
                uuid: self.backup.uuid().map(str::to_owned),
            },
            Request::SetExternalBackup {
                enabled: ligado,
                uuid,
            } => match wc::config::ExternalBackup::new(ligado, uuid.as_deref()) {
                Ok(b) => {
                    self.backup = b;
                    Response::Ok
                }
                Err(_) => Response::Error(ResponseError::InvalidRequest),
            },
            Request::UsbDrives => match &self.usb_drives {
                Some(l) => Response::UsbDrives(l.clone()),
                None => Response::Error(ResponseError::UsbDrivesUnavailable),
            },
            // Phase 47: the same rule as `wardd` (Phase 45).
            Request::Backups => match &self.backups {
                // The same rule as `wardd` (Phase 49a): the generation stays here; only
                // the flags go out.
                Some(l) => {
                    let with: Vec<_> = l
                        .iter()
                        .filter_map(|b| {
                            Some((wc::backup::Version::from_name(&b.name)?, b.generation))
                        })
                        .collect();
                    let flags = wc::backup::out_of_order(&with);
                    Response::Backups(
                        with.iter()
                            .zip(flags)
                            .map(|((v, g), out)| wc::protocol::BackupVersion {
                                name: v.name(),
                                verified: g.is_some(),
                                out_of_order: out,
                            })
                            .collect(),
                    )
                }
                None => Response::Error(ResponseError::BackupUnavailable),
            },
            Request::RestoreBackup {
                version: versao,
                password: senha,
                pin,
            } => match self.restore(&versao, &senha, &pin) {
                Ok(()) => Response::Ok,
                Err(e) => Response::Error(e),
            },
            Request::LockTimeout { minutes: minutos } => {
                if (wc::limits::MIN_LOCK_MINUTES..=wc::limits::MAX_LOCK_MINUTES).contains(&minutos)
                {
                    self.lock_minutes = minutos;
                    Response::Ok
                } else {
                    Response::Error(ResponseError::InvalidRequest)
                }
            }
            Request::ChangePin {
                current: atual,
                new: novo,
            } => match self.change_pin(&atual, &novo) {
                Ok(()) => Response::Ok,
                Err(e) => Response::Error(e),
            },
            Request::ChangePassword {
                current: atual,
                new: nova,
            } => match self.change_password(&atual, &nova) {
                Ok(()) => Response::Ok,
                Err(e) => Response::Error(e),
            },
            _ => Response::Error(ResponseError::Internal),
        }
    }

    /// The same rule as `wardd` (the `wardd` core, check PIN): the library's
    /// Argon2id hash (constant time); the 5th error in a row locks.
    fn check_pin(&mut self, pin: &CoreSecret) -> Result<(), ResponseError> {
        let matches = match &self.pin_hash {
            Some(h) => h.verify(pin.expose()),
            None => test_vault::test_config().verify_pin(pin.expose()),
        };
        if !matches {
            self.pin_errors = self.pin_errors.saturating_add(1);
            if self.pin_errors >= MAX_PIN_ERRORS {
                self.locked = true;
            }
            return Err(ResponseError::WrongPin);
        }
        self.pin_errors = 0;
        Ok(())
    }

    /// The same rule as `wardd` (its core, change PIN): the current PIN first; the new
    /// one becomes only the hash (D30).
    fn change_pin(&mut self, current: &CoreSecret, new: &CoreSecret) -> Result<(), ResponseError> {
        self.check_pin(current)?;
        self.pin_hash =
            Some(PinHash::create(new.expose()).map_err(|_| ResponseError::InvalidRequest)?);
        Ok(())
    }

    /// The same order as `wardd` (its core, restore backup): the name in format, the
    /// current PIN (counts toward the 5 that lock), the version in the folder and its
    /// password (constant time). The current one becomes the newest version (with the
    /// password of now); the vault takes the version's entries, password and PIN (the
    /// test one), with no pending releases.
    fn restore(
        &mut self,
        version: &str,
        password: &CoreSecret,
        pin: &CoreSecret,
    ) -> Result<(), ResponseError> {
        use ctutils::CtEq;
        if !wc::backup::valid_name(version) {
            return Err(ResponseError::InvalidRequest);
        }
        self.check_pin(pin)?;
        let Some(list) = self.backups.as_mut() else {
            return Err(ResponseError::BackupUnavailable);
        };
        let Some(i) = list.iter().position(|b| b.name == version) else {
            return Err(ResponseError::BackupNotFound);
        };
        let s = password.expose();
        let right = &list[i].password;
        if s.len() != right.len() || !s.ct_eq(&right[..]).to_bool() {
            return Err(ResponseError::WrongPassword);
        }
        let version_password = right.clone();
        let entries = match &list[i].entries {
            BackupEntries::Test { minus } => test_vault_minus(*minus),
            BackupEntries::Stored(_) => match list.remove(i).entries {
                BackupEntries::Stored(e) => e,
                BackupEntries::Test { minus } => test_vault_minus(minus),
            },
        };
        let now_name = wc::backup::Version::from_instant(std::time::SystemTime::now())
            .ok_or(ResponseError::Internal)?
            .name();
        let current_password = match &self.password {
            Some(s) => s.clone(),
            None => Zeroizing::new(test_vault::MASTER_PASSWORD.as_bytes().to_vec()),
        };
        let current = FakeBackup {
            name: now_name,
            generation: Some(self.generation),
            password: current_password,
            entries: BackupEntries::Stored(std::mem::replace(&mut self.entries, entries)),
        };
        let list = self.backups.get_or_insert_with(Vec::new);
        list.retain(|b| b.name != current.name);
        list.insert(0, current);
        list.truncate(FAKE_VERSIONS);
        self.generation = self.generation.saturating_add(1);
        self.password = Some(version_password);
        self.pin_hash = None;
        self.pin_errors = 0;
        self.approvals.clear();
        Ok(())
    }

    /// The same response as `wardd` (its core, change password, D52): a wrong current
    /// one is "wrong password" and does not lock.
    fn change_password(
        &mut self,
        current: &CoreSecret,
        new: &CoreSecret,
    ) -> Result<(), ResponseError> {
        if !self.password_matches(current) {
            return Err(ResponseError::WrongPassword);
        }
        self.password = Some(Zeroizing::new(new.expose().to_vec()));
        Ok(())
    }

    /// The same rule as `wardd` (its use module, answer approval): an unknown request is
    /// "expired"; "Deny" without a PIN; a wrong PIN leaves it pending, and the 5th locks
    /// and denies; "always" frees the request's entries that are "with approval". Phase
    /// 54b: an "Allow" of a request with the stdin hidden only with `stdin_confirmed`
    /// (before the PIN), and `stdin_confirmed` only on that request.
    fn answer_approval(
        &mut self,
        id: u64,
        decision: Decision,
        pin: Option<&CoreSecret>,
        stdin_confirmed: bool,
    ) -> Result<(), ResponseError> {
        let i = self
            .approvals
            .iter()
            .position(|(p, _, _)| *p == id)
            .ok_or(ResponseError::Expired)?;
        if decision == Decision::Deny {
            self.approvals.remove(i);
            return Ok(());
        }
        // Phase 44c: the origin confirmation has no PIN (the fake one does not know which
        // request it is; `wardd` and the app service check that).
        if decision == Decision::Confirm {
            self.approvals.remove(i);
            return Ok(());
        }
        if self.approvals[i].2 != stdin_confirmed {
            return Err(ResponseError::InvalidRequest);
        }
        let pin = pin.ok_or(ResponseError::InvalidRequest)?;
        if let Err(e) = self.check_pin(pin) {
            if self.locked {
                self.approvals.remove(i);
            }
            return Err(e);
        }
        let (_, entries, _) = self.approvals.remove(i);
        if decision == Decision::AllowAlways {
            for e in self.entries.iter_mut() {
                if entries.iter().any(|n| n == e.name()) && e.level() == AccessLevel::Approval {
                    e.set_level(AccessLevel::Free);
                }
            }
        }
        Ok(())
    }

    /// The same rule as `wardd` (the `wardd` core, check PIN and set
    /// level): the PIN first, and only then the entry.
    fn set_level(
        &mut self,
        name: &str,
        level: AccessLevel,
        pin: &CoreSecret,
    ) -> Result<(), ResponseError> {
        self.check_pin(pin)?;
        let e = self
            .entries
            .iter_mut()
            .find(|e| e.name() == name)
            .ok_or(ResponseError::EntryNotFound)?;
        e.set_level(level);
        e.set_dates(modified(e.dates()));
        Ok(())
    }

    /// The same rule as `wardd` (the `wardd` core, save entry).
    fn save(
        &mut self,
        previous: Option<&str>,
        mut draft: Draft,
        kept_attachments: &[String],
    ) -> Result<(), ResponseError> {
        // Phase 39b: the same rule as `wardd` (its core, save entry).
        if !kept_attachments.is_empty() {
            let name = previous.ok_or(ResponseError::InvalidRequest)?;
            let old = self.entry(name).ok_or(ResponseError::EntryNotFound)?;
            protocol::merge_kept_attachments(&mut draft, old, kept_attachments)?;
        }
        let mut new = draft
            .validate()
            .map_err(|e| ResponseError::from_validation(&e))?;
        match previous {
            None => {
                new.set_level(AccessLevel::Approval);
                new.set_dates(now_secs().and_then(Dates::new_stamp).unwrap_or(Dates::NONE));
                new.verify_in_vault(self.entries.iter())
                    .map_err(|e| ResponseError::from_validation(&e))?;
            }
            Some(name) => {
                let i = self
                    .entries
                    .iter()
                    .position(|e| e.name() == name)
                    .ok_or(ResponseError::EntryNotFound)?;
                new.set_level(self.entries[i].level());
                new.set_dates(modified(self.entries[i].dates()));
                new.verify_in_vault(self.entries.iter().filter(|e| e.name() != name))
                    .map_err(|e| ResponseError::from_validation(&e))?;
                drop(self.entries.remove(i));
            }
        }
        self.entries.push(new);
        self.entries.sort_by(|a, b| a.name().cmp(b.name()));
        Ok(())
    }
}

impl Daemon for FakeDaemon {
    fn exchange(&mut self, request: &[u8]) -> Result<Zeroizing<Vec<u8>>, AppError> {
        let response = match protocol::decode_request(Role::App, request) {
            Ok(p) => self.handle(p),
            Err(e) => Response::Error(e),
        };
        protocol::encode_response(Role::App, &response).map_err(|_| AppError::InvalidResponse)
    }
}

/// UTC seconds of the process clock (the `wardd` real clock, D65 d).
fn now_secs() -> Option<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|d| Dates::instant(d.as_secs()))
}

/// The dates after a modification now (the same without a clock), as in `wardd`.
fn modified(dates: Dates) -> Dates {
    now_secs().and_then(|a| dates.modified(a)).unwrap_or(dates)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_entries_as_the_fixtures() {
        // `dev/fixtures/entries.json` is the source of the test vault entries; the fake
        // daemon has exactly the same names, kinds, groups, environments and levels.
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dev/fixtures/entries.json"
        );
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let mut expected: Vec<(String, String, String, String, String)> = json["entradas"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                let t = |k: &str| e[k].as_str().unwrap().to_owned();
                (t("nome"), t("tipo"), t("grupo"), t("ambiente"), t("nivel"))
            })
            .collect();
        let mut fake: Vec<_> = FakeDaemon::full()
            .entries
            .iter()
            .map(|e| {
                (
                    e.name().to_owned(),
                    e.kind().name().to_owned(),
                    e.group().to_owned(),
                    e.environment().name().to_owned(),
                    e.level().name().to_owned(),
                )
            })
            .collect();
        expected.sort();
        fake.sort();
        assert_eq!(fake, expected);
    }

    #[test]
    fn origin_requests_use_the_real_entry() {
        // Phase 44e: the environment and the origin of the origin approval modes come
        // from the test vault entry (`acme-wp-dev` is dev; `acme-wp-prod`, prod), as in
        // `wardd`.
        use ward_core::entry::Environment;
        for (mode, environment, confirmation) in [
            ("aprovacao-origem", Environment::Prod, true),
            ("aprovacao-origem-popup", Environment::Prod, true),
            ("aprovacao-origem-pin", Environment::Dev, false),
        ] {
            let v = simulated_approvals(mode).unwrap();
            let p = &v[0].0;
            assert_eq!(p.environment, environment, "{mode}");
            assert_eq!(p.confirmation, confirmation, "{mode}");
            assert_eq!(p.origin, "http://localhost:8081", "{mode}");
            assert!(p.command.is_empty() && p.directory.is_empty(), "{mode}");
        }
    }

    #[test]
    fn only_the_app_role_is_handled() {
        // The fake one reads the request as the real daemon would read it from the app;
        // an agent frame with an app-only operation becomes "not authorized" on the agent
        // side, and an invalid body becomes an error, never a panic.
        let mut d = FakeDaemon::full();
        for junk in [&b""[..], b"{}", b"{\"v\":1,\"op\":\"revelar\"}", b"\xff"] {
            let r = d.exchange(junk).unwrap();
            let r = protocol::decode_response(Role::App, &r).unwrap();
            assert!(matches!(r, Response::Error(_)));
        }
    }
}
