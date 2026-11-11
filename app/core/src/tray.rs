//! Tray icon and menu (Phase 39, V5.23), without Tauri: what each vault status shows,
//! what each item does and where the icon may be written. `app/src-tauri` only builds the
//! native menu from here.
//!
//! Security:
//! - The menu is exported over the session bus (AppIndicator/`dbusmenu`): any process of
//!   the user reads the labels and can "click" an item. That is why the menu only has the
//!   vault status (which the agent role already gets with the status request) and four
//!   actions that do not reveal, copy nor approve anything: open the window, lock, open
//!   the unlock screen (the password is typed in the Ward window) and quit. No item
//!   carries an entry name nor a value (S1, S10, S13). An unknown id or one outside the
//!   status does nothing.
//! - The periodic poll uses only the status request, which `wardd` does not count as use
//!   (the daemon's "counts as use" rule): the tray does not hold the vault open. The lock
//!   timeout (the settings request, which **counts** as use) is only read after a user
//!   action: the transition from locked (or no vault) to unlocked, which requires the
//!   master password, and the Settings screen ([`read_settings_on_transition`]). Never at
//!   startup nor in the periodic poll: an agent that brings the app down and asks the
//!   daemon to open the app again does not renew the inactivity deadline.
//! - The icon (fixed, embedded in the binary) is written by the AppIndicator in its own
//!   folder in `XDG_RUNTIME_DIR`, 0700 and of the uid, checked without following links
//!   ([`icons_dir`]).
//! - The AppIndicator library is loaded at run time by `tray-icon`, which panics if it is
//!   missing (and the release aborts on panic): before that, [`library_available`] looks
//!   for the file in the system folders; without it, the app opens without a tray.

use std::fs::DirBuilder;
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::AppError;
use crate::daemon::VaultStatus;

/// Ids of the menu items. The text ones only inform (disabled).
pub const ID_TITLE: &str = "bandeja-titulo";
pub const ID_SUBTITLE: &str = "bandeja-subtitulo";
pub const ID_OPEN: &str = "bandeja-abrir";
pub const ID_ACTION: &str = "bandeja-acao";
pub const ID_QUIT: &str = "bandeja-sair";

/// Fixed texts of the items that do not change with the status (frames `17269`/`17300`).
pub const OPEN_TEXT: &str = "Abrir o Ward";
pub const QUIT_TEXT: &str = "Sair";

/// Name of the file `tray-icon` loads (the Ayatana AppIndicator or the old one), in its
/// order (`libappindicator-sys`).
pub const LIBRARIES: &[&str] = &[
    "libayatana-appindicator3.so.1",
    "libappindicator3.so.1",
    "libayatana-appindicator3.so",
    "libappindicator3.so",
];

/// System folders where the loader finds the libraries (without `LD_LIBRARY_PATH`, which
/// the app startup does not let through, D57).
pub const SYSTEM_DIRS: &[&str] = &[
    "/usr/lib/x86_64-linux-gnu",
    "/lib/x86_64-linux-gnu",
    "/usr/lib/aarch64-linux-gnu",
    "/lib/aarch64-linux-gnu",
    "/usr/lib64",
    "/lib64",
    "/usr/lib",
    "/lib",
];

/// Name of the icons folder inside `XDG_RUNTIME_DIR`.
pub const ICONS_DIR: &str = "ward-tray";

/// Status shown in the tray.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayState {
    Unlocked,
    Locked,
    NoVault,
    /// No answer from the daemon: fail closed (locked icon, no "lock now").
    NoDaemon,
}

impl TrayState {
    pub fn from_status(status: Result<VaultStatus, AppError>) -> TrayState {
        match status {
            Ok(VaultStatus::Unlocked) => TrayState::Unlocked,
            Ok(VaultStatus::Locked) => TrayState::Locked,
            Ok(VaultStatus::NoVault) => TrayState::NoVault,
            Err(_) => TrayState::NoDaemon,
        }
    }
}

/// Which of the two icons (tray icon component, `17138`/`17142`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Unlocked,
    Locked,
}

/// What the menu shows in a status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Appearance {
    pub title: &'static str,
    pub subtitle: &'static str,
    pub action: &'static str,
    pub action_enabled: bool,
    pub icon: Icon,
}

/// Subtitle of the unlocked status with the lock timeout read from the settings ("locks
/// by itself after 4 h without use", frame `17269`); without the reading, the generic text of
/// [`appearance`].
pub fn unlocked_subtitle(minutes: Option<u32>) -> String {
    let Some(m) = minutes.filter(|m| *m > 0) else {
        return appearance(TrayState::Unlocked).subtitle.to_owned();
    };
    let time = match (m / 60, m % 60) {
        (0, min) => format!("{min} min"),
        (h, 0) => format!("{h} h"),
        (h, min) => format!("{h} h {min} min"),
    };
    format!("Trava sozinho após {time} sem uso")
}

/// Read the lock timeout (the settings request, which the daemon counts as use) on this
/// status transition? Only when the vault was just unlocked (or created) by the user:
/// from locked or no vault to unlocked. Never from "no daemon" (app startup, reconnection)
/// nor from unlocked to unlocked (periodic poll).
pub fn read_settings_on_transition(before: TrayState, after: TrayState) -> bool {
    matches!(
        (before, after),
        (TrayState::Locked | TrayState::NoVault, TrayState::Unlocked)
    )
}

/// Zero what the form keeps (used passwords and loaded keys) on this status transition?
/// Whenever the vault stops being unlocked, wherever the lock comes from (inactivity,
/// the CLI lock, locked screen, suspend, daemon down): without this, a key loaded before
/// the lock would stay in Rust until the user left the form (Phase 53d, 2nd M10 audit).
pub fn discard_on_transition(before: TrayState, after: TrayState) -> bool {
    before == TrayState::Unlocked && after != TrayState::Unlocked
}

/// Texts of the menu (unlocked tray frame `17269` and locked one `17300`). The
/// unlocked subtitle here is the generic one; with the timeout read,
/// [`unlocked_subtitle`].
pub fn appearance(state: TrayState) -> Appearance {
    match state {
        TrayState::Unlocked => Appearance {
            title: "Cofre destravado",
            subtitle: "Trava sozinho após o tempo sem uso",
            action: "Travar agora",
            action_enabled: true,
            icon: Icon::Unlocked,
        },
        TrayState::Locked => Appearance {
            title: "Cofre travado",
            subtitle: "Destrave com a senha mestra",
            action: "Destravar…",
            action_enabled: true,
            icon: Icon::Locked,
        },
        TrayState::NoVault => Appearance {
            title: "Nenhum cofre criado",
            subtitle: "Crie o cofre na janela do Ward",
            action: "Travar agora",
            action_enabled: false,
            icon: Icon::Locked,
        },
        TrayState::NoDaemon => Appearance {
            title: "Daemon indisponível",
            subtitle: "Abra o Ward para tentar de novo",
            action: "Travar agora",
            action_enabled: false,
            icon: Icon::Locked,
        },
    }
}

/// What a click on the menu does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Shows (or recreates) the main window.
    Open,
    /// Locks the vault now (clears the clipboard and what the screen kept).
    Lock,
    /// Shows the main window, which is on the unlock screen.
    Unlock,
    /// Closes the app.
    Quit,
}

/// The id the native menu returned, in the status the menu showed. Only the menu ids
/// count; "lock"/"unlock" only in the status where they show up enabled.
pub fn action(id: &str, state: TrayState) -> Option<Action> {
    match id {
        ID_OPEN => Some(Action::Open),
        ID_QUIT => Some(Action::Quit),
        ID_ACTION => match state {
            TrayState::Unlocked => Some(Action::Lock),
            TrayState::Locked => Some(Action::Unlock),
            TrayState::NoVault | TrayState::NoDaemon => None,
        },
        _ => None,
    }
}

/// Does the AppIndicator library exist in one of the `dirs`? Only a regular file (or a
/// link to one), without opening anything.
pub fn library_available(dirs: &[&Path]) -> bool {
    dirs.iter().any(|p| {
        LIBRARIES
            .iter()
            .any(|b| std::fs::metadata(p.join(b)).is_ok_and(|m| m.is_file()))
    })
}

/// The tray icons folder in `runtime` (the `XDG_RUNTIME_DIR`): `runtime` must be an
/// absolute path, a folder (not a link) of `uid` and without group and other access; the
/// icons folder is created 0700 or, if it already exists, checked the same way (not a
/// link, of `uid`, 0700). Otherwise, error: the app opens without a tray.
pub fn icons_dir(runtime: &Path, uid: u32) -> io::Result<PathBuf> {
    if !runtime.is_absolute() {
        return Err(io::Error::other("XDG_RUNTIME_DIR relativo"));
    }
    check_dir(runtime, uid)?;
    let dir = runtime.join(ICONS_DIR);
    match DirBuilder::new().mode(0o700).create(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    check_dir(&dir, uid)?;
    Ok(dir)
}

/// [`icons_dir`] in the process's `XDG_RUNTIME_DIR`, with the effective uid.
pub fn session_icons_dir() -> io::Result<PathBuf> {
    let rt = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("sem XDG_RUNTIME_DIR"))?;
    icons_dir(&rt, ward_platform::effective_uid())
}

/// [`library_available`] in the [`SYSTEM_DIRS`].
pub fn system_library() -> bool {
    let dirs: Vec<&Path> = SYSTEM_DIRS.iter().map(Path::new).collect();
    library_available(&dirs)
}

fn check_dir(p: &Path, uid: u32) -> io::Result<()> {
    let m = std::fs::symlink_metadata(p)?;
    if !m.file_type().is_dir() {
        return Err(io::Error::other("não é pasta"));
    }
    if m.uid() != uid {
        return Err(io::Error::other("pasta de outro usuário"));
    }
    if m.permissions().mode() & 0o077 != 0 {
        return Err(io::Error::other("pasta aberta a grupo ou outros"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [TrayState; 4] = [
        TrayState::Unlocked,
        TrayState::Locked,
        TrayState::NoVault,
        TrayState::NoDaemon,
    ];

    fn temp_dir(name: &str) -> PathBuf {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("bandeja-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&p)
            .unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700)).unwrap();
        p
    }

    #[test]
    fn design_texts() {
        let d = appearance(TrayState::Unlocked);
        assert_eq!(
            (d.title, d.action, d.icon),
            ("Cofre destravado", "Travar agora", Icon::Unlocked)
        );
        let t = appearance(TrayState::Locked);
        assert_eq!(
            (t.title, t.subtitle, t.action, t.icon),
            (
                "Cofre travado",
                "Destrave com a senha mestra",
                "Destravar…",
                Icon::Locked
            )
        );
        assert_eq!(OPEN_TEXT, "Abrir o Ward");
        assert_eq!(QUIT_TEXT, "Sair");
    }

    #[test]
    fn status_comes_from_the_response_and_error_stays_locked() {
        assert_eq!(
            TrayState::from_status(Ok(VaultStatus::Unlocked)),
            TrayState::Unlocked
        );
        assert_eq!(
            TrayState::from_status(Ok(VaultStatus::Locked)),
            TrayState::Locked
        );
        assert_eq!(
            TrayState::from_status(Ok(VaultStatus::NoVault)),
            TrayState::NoVault
        );
        let e = TrayState::from_status(Err(AppError::DaemonUnavailable));
        assert_eq!(e, TrayState::NoDaemon);
        // Fail closed: without a daemon the icon is the locked one and there is nothing
        // to lock.
        assert_eq!(appearance(e).icon, Icon::Locked);
        assert!(!appearance(e).action_enabled);
    }

    /// Attack (S10, S13): a process of the user sends any id over `dbusmenu`. Only the
    /// four menu ids do something, and no action approves, reveals or copies.
    #[test]
    fn attack_forged_id_in_the_menu_does_nothing() {
        for state in ALL {
            for id in [
                "",
                "inventado",
                ID_TITLE,
                ID_SUBTITLE,
                "bandeja-abrir ",
                "BANDEJA-ACAO",
                "responder_aprovacao",
                "revelar_campo",
                "copiar_campo",
                "destravar",
                "answer_approval",
                "reveal_field",
                "copy_field",
                "unlock",
                "bandeja-acao\0",
            ] {
                assert_eq!(action(id, state), None, "{id:?} em {state:?}");
            }
            assert_eq!(action(ID_OPEN, state), Some(Action::Open));
            assert_eq!(action(ID_QUIT, state), Some(Action::Quit));
        }
        assert_eq!(action(ID_ACTION, TrayState::Unlocked), Some(Action::Lock));
        assert_eq!(action(ID_ACTION, TrayState::Locked), Some(Action::Unlock));
        // The disabled item does not act even if the click arrives over the bus.
        assert_eq!(action(ID_ACTION, TrayState::NoVault), None);
        assert_eq!(action(ID_ACTION, TrayState::NoDaemon), None);
    }

    /// S13: the menu (readable by any process of the session) carries no entry name nor
    /// value: only fixed texts, and none from the table of fake secrets.
    #[test]
    fn attack_menu_without_vault_data() {
        for state in ALL {
            let a = appearance(state);
            for text in [a.title, a.subtitle, a.action, OPEN_TEXT, QUIT_TEXT] {
                for s in crate::fake_daemon::SECRETS {
                    assert!(!text.contains(s), "{text}");
                }
                assert!(!text.contains("acme-"), "{text}");
            }
        }
    }

    #[test]
    fn subtitle_with_the_lock_timeout() {
        assert_eq!(
            unlocked_subtitle(Some(240)),
            "Trava sozinho após 4 h sem uso"
        );
        assert_eq!(
            unlocked_subtitle(Some(30)),
            "Trava sozinho após 30 min sem uso"
        );
        assert_eq!(
            unlocked_subtitle(Some(90)),
            "Trava sozinho após 1 h 30 min sem uso"
        );
        assert_eq!(
            unlocked_subtitle(None),
            appearance(TrayState::Unlocked).subtitle
        );
        assert_eq!(
            unlocked_subtitle(Some(0)),
            appearance(TrayState::Unlocked).subtitle
        );
    }

    /// Attack (lock on inactivity): the settings request counts as use in `wardd`. The
    /// tray only reads it when the user just unlocked (or created) the vault; the app
    /// startup (from "no daemon"), the periodic poll and the lock do not read it. An
    /// agent that brings the app down and asks the daemon to open the app in a loop does
    /// not hold the vault open.
    #[test]
    fn attack_settings_only_after_unlocking() {
        use TrayState::*;
        assert!(read_settings_on_transition(Locked, Unlocked));
        assert!(read_settings_on_transition(NoVault, Unlocked));
        for before in ALL {
            for after in ALL {
                if !matches!((before, after), (Locked | NoVault, Unlocked)) {
                    assert!(
                        !read_settings_on_transition(before, after),
                        "{before:?} -> {after:?}"
                    );
                }
            }
        }
        assert!(!read_settings_on_transition(NoDaemon, Unlocked));
        assert!(!read_settings_on_transition(Unlocked, Unlocked));
    }

    /// Phase 53d: every exit from unlocked (lock from outside or daemon down) discards
    /// the form; no other transition discards it.
    #[test]
    fn attack_lock_from_outside_discards_the_form() {
        use TrayState::*;
        for after in [Locked, NoVault, NoDaemon] {
            assert!(discard_on_transition(Unlocked, after), "{after:?}");
        }
        for before in ALL {
            for after in ALL {
                if before != Unlocked || after == Unlocked {
                    assert!(
                        !discard_on_transition(before, after),
                        "{before:?} -> {after:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn library_only_if_the_file_exists() {
        let empty = temp_dir("lib-vazia");
        assert!(!library_available(&[empty.as_path()]));
        // A folder with the library name does not count.
        std::fs::create_dir(empty.join(LIBRARIES[0])).unwrap();
        assert!(!library_available(&[empty.as_path()]));
        let full = temp_dir("lib-cheia");
        std::fs::write(full.join(LIBRARIES[1]), b"").unwrap();
        assert!(library_available(&[empty.as_path(), full.as_path()]));
        let _ = std::fs::remove_dir_all(empty);
        let _ = std::fs::remove_dir_all(full);
    }

    #[test]
    fn icons_dir_created_0700() {
        let uid = ward_platform::effective_uid();
        let rt = temp_dir("rt-ok");
        let p = icons_dir(&rt, uid).unwrap();
        let m = std::fs::symlink_metadata(&p).unwrap();
        assert!(m.is_dir());
        assert_eq!(m.permissions().mode() & 0o777, 0o700);
        // Again: the folder already exists and is accepted.
        assert_eq!(icons_dir(&rt, uid).unwrap(), p);
        let _ = std::fs::remove_dir_all(rt);
    }

    /// Attack: the icons folder swapped for a link (for the AppIndicator to write the PNG
    /// somewhere else), open to others, of another uid, or `XDG_RUNTIME_DIR` relative or
    /// open: all refused, and the app goes on without a tray.
    #[test]
    fn attack_icons_dir_link_open_or_relative() {
        let uid = ward_platform::effective_uid();

        let rt = temp_dir("rt-link");
        let target = temp_dir("rt-alvo");
        std::os::unix::fs::symlink(&target, rt.join(ICONS_DIR)).unwrap();
        assert!(icons_dir(&rt, uid).is_err());

        let rt2 = temp_dir("rt-aberta");
        let icons = rt2.join(ICONS_DIR);
        std::fs::create_dir(&icons).unwrap();
        std::fs::set_permissions(&icons, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(icons_dir(&rt2, uid).is_err());

        let rt3 = temp_dir("rt-runtime-aberto");
        std::fs::set_permissions(&rt3, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(icons_dir(&rt3, uid).is_err());
        assert!(!rt3.join(ICONS_DIR).exists());

        let rt4 = temp_dir("rt-outro-uid");
        assert!(icons_dir(&rt4, uid.wrapping_add(1)).is_err());

        assert!(icons_dir(Path::new("run/user/1000"), uid).is_err());

        let rt5 = temp_dir("rt-arquivo");
        std::fs::write(rt5.join(ICONS_DIR), b"").unwrap();
        assert!(icons_dir(&rt5, uid).is_err());

        for p in [rt, target, rt2, rt3, rt4, rt5] {
            let _ = std::fs::remove_dir_all(p);
        }
    }
}
