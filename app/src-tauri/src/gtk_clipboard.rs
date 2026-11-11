//! "Copy" through the GTK clipboard (Phase 39f; S2, S12, D4).
//!
//! The value stays only in the app's memory: GTK announces it to the compositor and,
//! when someone pastes, writes to the pipe the compositor hands over
//! (`wl_data_source.send`). No external program or file enters the path. The `wl-copy`
//! the app used before kept the value in `/tmp/wl-copy-buffer-*/stdin`, on disk (M5
//! audit).
//!
//! - Offered types: UTF-8 text (`text/plain;charset=utf-8`, `UTF8_STRING`,
//!   `text/plain`) and `x-kde-passwordManagerHint` = `secret`, which asks clipboard
//!   managers not to keep the value in the history.
//! - "Still Ward's" means being the owner of the selection: GTK calls the content
//!   cleanup (`clear_func`, here the `Drop` of the [`Guard`]) when another program copies
//!   something or when the app clears. While Ward is the owner, [`GtkClipboard::read`]
//!   returns the value it published; afterwards, `None`. [`GtkClipboard::clear`] only
//!   removes the content if the owner is still the same copy: the text the user copied
//!   later is never erased, without the race between reading and clearing that
//!   `wl-paste`/`wl-copy --clear` had.
//! - GTK only on the main thread. `write` comes from the command (synchronous in Tauri,
//!   on the main thread); outside it, it is refused. `clear` may come from another thread
//!   (30 s delay, lock seen by the tray, connection watcher): the request goes to the
//!   main thread through GLib's main context, without waiting, and there it checks again
//!   whether the copy is still the same.
//! - The value stays in a `Zeroizing` shared between the clipboard and the GTK content,
//!   zeroed when both drop it. GTK copies the value into its own buffers on each paste
//!   (out of our control; see the Phase 39f handoff).

use std::sync::{Arc, Mutex, MutexGuard};

use gtk::{gdk, glib};
use ward_app_core::AppError;
use ward_app_core::clipboard::{Clipboard, Zeroizing};

/// Offered text types (`info` 0).
const TEXT_TYPES: [&str; 3] = ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain"];
/// Notice for clipboard managers (KDE Klipper and others): do not keep.
const HINT_TYPE: &str = "x-kde-passwordManagerHint";
const HINT: &[u8] = b"secret";
const TEXT_INFO: u32 = 0;
const HINT_INFO: u32 = 1;

type Value = Arc<Zeroizing<Vec<u8>>>;

#[derive(Default)]
struct Owner {
    /// Copy counter (each `write` gets a number).
    last: u64,
    /// The copy GTK is serving now.
    current: Option<(u64, Value)>,
}

/// The GTK clipboard. Keeps only Ward state; GTK is touched on the main thread.
#[derive(Default)]
pub struct GtkClipboard {
    owner: Arc<Mutex<Owner>>,
}

impl GtkClipboard {
    pub fn new() -> GtkClipboard {
        GtkClipboard::default()
    }
}

fn lock_owner(owner: &Mutex<Owner>) -> MutexGuard<'_, Owner> {
    owner.lock().unwrap_or_else(|e| e.into_inner())
}

/// Goes with the content handed to GTK. When GTK drops the content (another program
/// copied, the app cleared or replaced the copy), the copy stops being the current one.
struct Guard {
    id: u64,
    owner: Arc<Mutex<Owner>>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        release(&self.owner, self.id);
    }
}

fn release(owner: &Mutex<Owner>, id: u64) {
    let mut d = lock_owner(owner);
    if d.current.as_ref().is_some_and(|(current, _)| *current == id) {
        d.current = None;
    }
}

fn on_main_thread() -> bool {
    gtk::is_initialized_main_thread()
}

impl Clipboard for GtkClipboard {
    fn write(&self, value: &[u8]) -> Result<(), AppError> {
        if !on_main_thread() {
            return Err(AppError::Clipboard);
        }
        let value: Value = Arc::new(Zeroizing::new(value.to_vec()));
        let id = {
            let mut d = lock_owner(&self.owner);
            d.last = d.last.wrapping_add(1);
            d.last
        };
        let mut targets: Vec<gtk::TargetEntry> = TEXT_TYPES
            .iter()
            .map(|t| gtk::TargetEntry::new(t, gtk::TargetFlags::empty(), TEXT_INFO))
            .collect();
        targets.push(gtk::TargetEntry::new(
            HINT_TYPE,
            gtk::TargetFlags::empty(),
            HINT_INFO,
        ));
        let guard = Guard {
            id,
            owner: Arc::clone(&self.owner),
        };
        let served = Arc::clone(&value);
        let clipboard = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD);
        // GTK drops the previous copy in here (its guard takes the previous one out of
        // `current`); the `owner` lock is not held.
        let ok = clipboard.set_with_data(&targets, move |_, data, info| {
            let _ = &guard;
            let content: &[u8] = if info == HINT_INFO { HINT } else { &served };
            data.set(&data.target(), 8, content);
        });
        if !ok {
            return Err(AppError::Clipboard);
        }
        lock_owner(&self.owner).current = Some((id, value));
        Ok(())
    }

    fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>, AppError> {
        Ok(lock_owner(&self.owner)
            .current
            .as_ref()
            .map(|(_, v)| Zeroizing::new(v.to_vec())))
    }

    fn clear(&self) -> Result<(), AppError> {
        let Some(id) = lock_owner(&self.owner).current.as_ref().map(|(id, _)| *id) else {
            return Ok(());
        };
        let owner = Arc::clone(&self.owner);
        let clear = move || {
            // Outside the main thread (GLib runs it right here when the loop is not up,
            // e.g. on exit): does not touch GTK.
            if !on_main_thread() {
                return;
            }
            let still = lock_owner(&owner)
                .current
                .as_ref()
                .is_some_and(|(current, _)| *current == id);
            if still {
                // GTK calls the content cleanup (the guard) in here.
                gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD).clear();
            }
        };
        if on_main_thread() {
            clear();
        } else {
            glib::MainContext::default().invoke(clear);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_copy(clipboard: &GtkClipboard, id: u64, v: &[u8]) -> Guard {
        let mut d = lock_owner(&clipboard.owner);
        d.last = id;
        d.current = Some((id, Arc::new(Zeroizing::new(v.to_vec()))));
        Guard {
            id,
            owner: Arc::clone(&clipboard.owner),
        }
    }

    /// S12/V5.5: while Ward is the owner, `read` returns the copied value; when GTK drops
    /// the content (another program copied), `read` gives `None` and the copier does not
    /// clear.
    #[test]
    fn attack_text_copied_by_another_program_stops_being_wards() {
        let clipboard = GtkClipboard::new();
        let guard = with_copy(&clipboard, 1, b"falso-39f-a");
        assert_eq!(
            clipboard.read().unwrap().as_deref().map(Vec::as_slice),
            Some(&b"falso-39f-a"[..])
        );
        drop(guard);
        assert_eq!(clipboard.read().unwrap(), None);
        // Without a current copy, clear does nothing (does not even touch GTK).
        assert_eq!(clipboard.clear(), Ok(()));
    }

    /// The guard of an old copy, dropped after the new one, does not erase the new one.
    #[test]
    fn attack_old_guard_does_not_release_the_new_copy() {
        let clipboard = GtkClipboard::new();
        let old = with_copy(&clipboard, 1, b"falso-39f-b");
        let new = with_copy(&clipboard, 2, b"falso-39f-c");
        drop(old);
        assert_eq!(
            clipboard.read().unwrap().as_deref().map(Vec::as_slice),
            Some(&b"falso-39f-c"[..])
        );
        drop(new);
        assert_eq!(clipboard.read().unwrap(), None);
    }

    /// Outside the GTK main thread the "copy" is refused and nothing is kept.
    #[test]
    fn attack_copy_outside_the_main_thread_is_refused() {
        let clipboard = GtkClipboard::new();
        assert_eq!(clipboard.write(b"falso-39f-d"), Err(AppError::Clipboard));
        assert_eq!(clipboard.read().unwrap(), None);
    }

    /// `clear` outside the main thread with a current copy: the request goes to the GLib
    /// loop and does not touch GTK from here (GTK was not even started in this test).
    #[test]
    fn clear_outside_the_main_thread_does_not_touch_gtk() {
        let clipboard = GtkClipboard::new();
        let guard = with_copy(&clipboard, 7, b"falso-39f-e");
        assert_eq!(clipboard.clear(), Ok(()));
        // Nobody dropped the copy: it stays Ward's until GTK drops it.
        assert!(clipboard.read().unwrap().is_some());
        drop(guard);
    }

    /// Test in the real graphical session (Phase 39f): `make test-clipboard` runs this
    /// test under `strace -f`, with GTK started here, and drives it through stdin
    /// (`dev/tests/clipboard.py`). Fake values only. Lines:
    /// `prazo <ms>`, `copiar <valor>`, `copiar-fora <valor>`, `limpar-fora`, `sair`.
    /// Each answer goes out on a `ward39f: …` line.
    #[test]
    #[ignore = "precisa da sessão gráfica; rode por `make test-clipboard`"]
    fn real_session() {
        use std::io::BufRead;
        use std::sync::mpsc;
        use std::time::Duration;
        use ward_app_core::clipboard::Copier;
        use ward_app_core::secret::Secret;

        gtk::init().expect("GTK");
        let (tx, rx) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            for line in std::io::stdin().lock().lines() {
                let Ok(l) = line else { break };
                if tx.send(l).is_err() {
                    break;
                }
            }
        });
        let answer = |r: &str| println!("ward39f: {r}");
        let mut copier: Option<Arc<Copier<GtkClipboard>>> = None;
        answer("pronto");
        loop {
            while gtk::events_pending() {
                gtk::main_iteration_do(false);
            }
            let Ok(line) = rx.recv_timeout(Duration::from_millis(10)) else {
                continue;
            };
            let (cmd, arg) = line.split_once(' ').unwrap_or((line.as_str(), ""));
            match cmd {
                "prazo" => {
                    let ms: u64 = arg.parse().expect("prazo");
                    copier = Some(Arc::new(Copier::with_delay(
                        GtkClipboard::new(),
                        Duration::from_millis(ms),
                    )));
                    answer("ok");
                }
                "copiar" => {
                    let c = copier.as_ref().expect("prazo antes");
                    let r = c.copy(Secret::new(arg.as_bytes().to_vec()));
                    answer(if r.is_ok() { "ok" } else { "erro" });
                }
                "copiar-fora" => {
                    let c = Arc::clone(copier.as_ref().expect("prazo antes"));
                    let v = arg.as_bytes().to_vec();
                    let r = std::thread::spawn(move || c.copy(Secret::new(v)))
                        .join()
                        .unwrap();
                    answer(if r.is_ok() { "ok" } else { "erro" });
                }
                "limpar-fora" => {
                    let c = Arc::clone(copier.as_ref().expect("prazo antes"));
                    std::thread::spawn(move || c.clear_now()).join().unwrap();
                    answer("ok");
                }
                "sair" => break,
                _ => answer("desconhecido"),
            }
        }
        answer("fim");
    }
}
