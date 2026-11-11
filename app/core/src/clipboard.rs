//! "Copy" on the Rust side of the app (D4, S2, S11, S12).
//!
//! The value comes from the daemon as a [`Secret`], goes straight to the clipboard and
//! never passes through the webview. After [`CLEAR_DELAY`], the cleanup only erases if
//! the clipboard still holds exactly the copied value (constant-time comparison): if the
//! user copied something else in the meantime, it stays there (V5.5).
//!
//! Phase 39f (S2): what implements [`Clipboard`] in the app is the GTK clipboard
//! (`app/src-tauri/src/gtk_clipboard.rs`), which serves the value from the process
//! memory. No external program (`wl-copy`, which stored the value in `/tmp`) enters the
//! app's path. The test bridge (fake values only) has its own, in `ward-test-bridge`.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use ward_core::limits;
/// The type [`Clipboard::read`] returns, for whoever implements the clipboard outside
/// here without depending on `zeroize` (the Tauri app).
pub use zeroize::Zeroizing;

use crate::AppError;
use crate::secret::Secret;

/// S12: the clipboard is cleared 30 s after the "copy".
pub const CLEAR_DELAY: Duration = Duration::from_secs(30);

/// Largest value the "copy" accepts (the limit of a `ward-core` text secret). Whoever
/// reads the clipboard needs no more than `MAX_VALUE + 1` bytes to compare.
pub const MAX_VALUE: usize = limits::MAX_SECRET;

/// Where the copied value stays. The tests use a fake clipboard.
pub trait Clipboard: Send + Sync + 'static {
    fn write(&self, value: &[u8]) -> Result<(), AppError>;
    /// Current text content (`None` if empty or without text), limited to
    /// `MAX_SECRET + 1` bytes: more than that cannot be the copied value.
    fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>, AppError>;
    fn clear(&self) -> Result<(), AppError>;
}

struct Pending {
    generation: u64,
    value: Option<Zeroizing<Vec<u8>>>,
}

/// Copies and schedules the conditional cleanup.
pub struct Copier<A: Clipboard> {
    clipboard: Arc<A>,
    pending: Arc<Mutex<Pending>>,
    delay: Duration,
}

impl<A: Clipboard> Copier<A> {
    pub fn new(clipboard: A) -> Self {
        Self::with_delay(clipboard, CLEAR_DELAY)
    }

    /// A different delay, for the tests only.
    pub fn with_delay(clipboard: A, delay: Duration) -> Self {
        Copier {
            clipboard: Arc::new(clipboard),
            pending: Arc::new(Mutex::new(Pending {
                generation: 0,
                value: None,
            })),
            delay,
        }
    }

    /// Writes the value and schedules the cleanup. A new copy replaces the previous one:
    /// the previous delay stops counting and only the new one clears.
    pub fn copy(&self, secret: Secret) -> Result<(), AppError> {
        if secret.is_empty() || secret.len() > limits::MAX_SECRET {
            return Err(AppError::InvalidRequest);
        }
        let generation = {
            let mut p = self.pending.lock().map_err(|_| AppError::Clipboard)?;
            self.clipboard.write(secret.expose())?;
            p.generation = p.generation.wrapping_add(1);
            p.value = Some(Zeroizing::new(secret.expose().to_vec()));
            p.generation
        };
        drop(secret);
        let clipboard = Arc::clone(&self.clipboard);
        let pending = Arc::clone(&self.pending);
        let delay = self.delay;
        thread::Builder::new()
            .name("ward-limpeza".into())
            .spawn(move || {
                thread::sleep(delay);
                clear_if_still(&*clipboard, &pending, Some(generation));
            })
            .map_err(|_| AppError::Clipboard)?;
        Ok(())
    }

    /// Clears now (on locking or closing the app), with the same condition as the delay.
    pub fn clear_now(&self) {
        clear_if_still(&*self.clipboard, &self.pending, None);
    }
}

fn clear_if_still<A: Clipboard>(clipboard: &A, pending: &Mutex<Pending>, generation: Option<u64>) {
    let Ok(mut p) = pending.lock() else { return };
    if generation.is_some_and(|g| g != p.generation) {
        return; // a newer copy has its own delay
    }
    let Some(value) = p.value.take() else { return };
    if let Ok(Some(current)) = clipboard.read()
        && equal(&current, &value)
    {
        let _ = clipboard.clear();
    }
}

fn equal(a: &[u8], b: &[u8]) -> bool {
    use ctutils::CtEq;
    a.ct_eq(b).to_bool()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fake clipboard: keeps the content and counts the cleanups.
    #[derive(Default)]
    pub(crate) struct FakeClipboard {
        pub content: Mutex<Option<Vec<u8>>>,
        pub clears: Mutex<u32>,
    }

    impl Clipboard for Arc<FakeClipboard> {
        fn write(&self, value: &[u8]) -> Result<(), AppError> {
            *self.content.lock().unwrap() = Some(value.to_vec());
            Ok(())
        }
        fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>, AppError> {
            Ok(self.content.lock().unwrap().clone().map(Zeroizing::new))
        }
        fn clear(&self) -> Result<(), AppError> {
            *self.content.lock().unwrap() = None;
            *self.clears.lock().unwrap() += 1;
            Ok(())
        }
    }

    const SHORT: Duration = Duration::from_millis(40);
    const WAIT: Duration = Duration::from_millis(600);

    #[test]
    fn delay_is_30_seconds() {
        assert_eq!(CLEAR_DELAY, Duration::from_secs(30));
    }

    #[test]
    fn copies_and_clears_after_the_delay() {
        let clipboard = Arc::new(FakeClipboard::default());
        let c = Copier::with_delay(Arc::clone(&clipboard), SHORT);
        c.copy(Secret::new(b"falso-valor-1".to_vec())).unwrap();
        assert_eq!(
            clipboard.content.lock().unwrap().as_deref(),
            Some(&b"falso-valor-1"[..])
        );
        thread::sleep(WAIT);
        assert_eq!(*clipboard.content.lock().unwrap(), None);
        assert_eq!(*clipboard.clears.lock().unwrap(), 1);
    }

    #[test]
    fn attack_user_text_copied_later_is_not_erased() {
        let clipboard = Arc::new(FakeClipboard::default());
        let c = Copier::with_delay(Arc::clone(&clipboard), SHORT);
        c.copy(Secret::new(b"falso-valor-2".to_vec())).unwrap();
        *clipboard.content.lock().unwrap() = Some(b"texto do usuario".to_vec());
        thread::sleep(WAIT);
        assert_eq!(
            clipboard.content.lock().unwrap().as_deref(),
            Some(&b"texto do usuario"[..])
        );
        assert_eq!(*clipboard.clears.lock().unwrap(), 0);
    }

    #[test]
    fn attack_value_prefix_does_not_count_as_equal() {
        let clipboard = Arc::new(FakeClipboard::default());
        let c = Copier::with_delay(Arc::clone(&clipboard), SHORT);
        c.copy(Secret::new(b"falso-valor-3".to_vec())).unwrap();
        *clipboard.content.lock().unwrap() = Some(b"falso-valor".to_vec());
        thread::sleep(WAIT);
        assert_eq!(*clipboard.clears.lock().unwrap(), 0);
    }

    #[test]
    fn new_copy_replaces_the_previous_delay() {
        let clipboard = Arc::new(FakeClipboard::default());
        let c = Copier::with_delay(Arc::clone(&clipboard), Duration::from_millis(400));
        c.copy(Secret::new(b"falso-a".to_vec())).unwrap();
        thread::sleep(Duration::from_millis(200));
        c.copy(Secret::new(b"falso-b".to_vec())).unwrap();
        // The first delay expires here and must not erase the second one early.
        thread::sleep(Duration::from_millis(300));
        assert_eq!(
            clipboard.content.lock().unwrap().as_deref(),
            Some(&b"falso-b"[..])
        );
        thread::sleep(Duration::from_millis(600));
        assert_eq!(*clipboard.content.lock().unwrap(), None);
        assert_eq!(*clipboard.clears.lock().unwrap(), 1);
    }

    #[test]
    fn clear_now_respects_the_same_condition() {
        let clipboard = Arc::new(FakeClipboard::default());
        let c = Copier::with_delay(Arc::clone(&clipboard), Duration::from_secs(60));
        c.copy(Secret::new(b"falso-c".to_vec())).unwrap();
        c.clear_now();
        assert_eq!(*clipboard.content.lock().unwrap(), None);
        c.copy(Secret::new(b"falso-d".to_vec())).unwrap();
        *clipboard.content.lock().unwrap() = Some(b"outro".to_vec());
        c.clear_now();
        assert_eq!(
            clipboard.content.lock().unwrap().as_deref(),
            Some(&b"outro"[..])
        );
    }

    #[test]
    fn attack_empty_or_too_large_value_is_refused() {
        let clipboard = Arc::new(FakeClipboard::default());
        let c = Copier::with_delay(Arc::clone(&clipboard), SHORT);
        assert_eq!(
            c.copy(Secret::new(Vec::new())),
            Err(AppError::InvalidRequest)
        );
        assert_eq!(
            c.copy(Secret::new(vec![b'a'; limits::MAX_SECRET + 1])),
            Err(AppError::InvalidRequest)
        );
        assert_eq!(*clipboard.content.lock().unwrap(), None);
    }
}
