//! Form attachments (Phase 34, V5.12; D29, D32).
//!
//! S11: the content of an attachment never passes through the webview. The interface
//! only asks to choose an attachment; who chooses the file is the [`Picker`] on the Rust
//! side (in the app, the GTK file dialog; in the test bridge, a fixed folder given on the
//! command line). The file is read here, kept in a [`Secret`] (zeroed on drop) and the
//! response carries only the id, the name and the size.
//!
//! The limits come from `ward_core::limits` (D32): a file above `MAX_ATTACHMENTS_TOTAL`,
//! or that would take the total past it, produces a notice and is not kept (nor sent to
//! the daemon). An empty file is refused (D29).
//!
//! Phase 39b (V5.12): when editing, the total and the count start from the stored
//! attachments that stay in the entry ([`Base`]); their content stays in the daemon. A
//! file name starting with `.` (e.g. `.env`) is attached without the leading dots
//! (`env`): `ward-core` does not accept an attachment name starting with `.` (the name
//! becomes a file in `/run/user/<uid>/ward/tmp/` and the delivered file variable), and the
//! screen shows the stored name.

use std::fs::OpenOptions;
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ward_core::limits;

use crate::AppError;
use crate::secret::Secret;

/// Who chooses the file. `Ok(None)`: the user canceled.
pub trait Picker: Send + Sync {
    fn choose(&self) -> Result<Option<PathBuf>, AppError>;

    /// The private key file (the "load from file" button, Phase 53a). The default is the same
    /// picker; the GTK one only changes the title and the button of the dialog.
    fn choose_key(&self) -> Result<Option<PathBuf>, AppError> {
        self.choose()
    }
}

/// No picker (the service default until someone sets one).
pub struct NoPicker;

impl Picker for NoPicker {
    fn choose(&self) -> Result<Option<PathBuf>, AppError> {
        Err(AppError::PickerUnavailable)
    }
}

/// An attachment chosen and not saved yet.
pub struct Pending {
    pub id: u64,
    pub name: String,
    pub content: Secret,
}

/// What the interface receives from choosing an attachment. Never the content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "result")]
pub enum Choice {
    #[serde(rename = "attached")]
    Attached { id: u64, name: String, size: u64 },
    #[serde(rename = "canceled")]
    Canceled,
    /// The file was not kept; the reason is a fixed text.
    #[serde(rename = "notice")]
    Notice { reason: String },
}

/// Texts of the notices (fixed; no path nor system error, S13).
pub fn too_large_notice() -> String {
    format!(
        "o arquivo passa de {} KiB e não foi anexado",
        limits::MAX_ATTACHMENTS_TOTAL / limits::KIB
    )
}

pub fn total_notice() -> String {
    format!(
        "com este arquivo os anexos somariam mais de {} KiB; ele não foi anexado",
        limits::MAX_ATTACHMENTS_TOTAL / limits::KIB
    )
}

pub const EMPTY_NOTICE: &str = "arquivo vazio: anexo vazio não é aceito";
pub const TOO_MANY_NOTICE: &str = "anexos demais nesta entrada";
pub const READ_NOTICE: &str = "não foi possível ler o arquivo";
pub const NAME_NOTICE: &str = "o nome do arquivo não é texto válido";

/// What already counts for the limits before the ones chosen now: when editing, the
/// stored attachments that stay in the entry (summed size and count). For a new entry,
/// zero.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Base {
    pub total: usize,
    pub count: usize,
}

/// Argument of choosing an attachment when editing (Phase 39b): the edited entry and the
/// names of the stored attachments the screen still keeps. Untrusted: the service checks
/// the names in the entry detail and takes the [`Base`] from them.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachmentEdit {
    pub entry: String,
    pub kept: Vec<String>,
}

/// Name the file is attached with: the file name without the leading `.` (Phase 39b;
/// `ward-core` refuses an attachment name starting with `.`). `None` if nothing is left.
pub fn attachment_name(file: &str) -> Option<&str> {
    let name = file.trim_start_matches('.');
    (!name.is_empty()).then_some(name)
}

/// The attachments chosen in the open form.
#[derive(Default)]
pub struct Attachments {
    pending: Vec<Pending>,
    next: u64,
}

impl Attachments {
    /// Sum of the kept sizes.
    pub fn total(&self) -> usize {
        self.pending.iter().map(|p| p.content.len()).sum()
    }

    /// Reads the chosen file and keeps it, or returns the notice (new entry).
    pub fn attach(&mut self, path: &Path) -> Choice {
        self.attach_over(path, Base::default())
    }

    /// Like [`Attachments::attach`], with the limits counted from `base` (the stored
    /// attachments that stay in the edited entry).
    pub fn attach_over(&mut self, path: &Path, base: Base) -> Choice {
        let notice = |m: String| Choice::Notice { reason: m };
        if self.pending.len().saturating_add(base.count) >= limits::MAX_ATTACHMENTS {
            return notice(TOO_MANY_NOTICE.to_owned());
        }
        let Some(name) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(attachment_name)
        else {
            return notice(NAME_NOTICE.to_owned());
        };
        if name.is_empty() || name.len() > limits::MAX_ATTACHMENT_NAME * 4 {
            return notice(NAME_NOTICE.to_owned());
        }
        let name = name.to_owned();
        let free = limits::MAX_ATTACHMENTS_TOTAL.saturating_sub(base.total);
        let content = match read_limited(path, limits::MAX_ATTACHMENTS_TOTAL) {
            Ok(ReadOutcome::Content(c)) => c,
            Ok(ReadOutcome::TooLarge) => return notice(too_large_notice()),
            Err(()) => return notice(READ_NOTICE.to_owned()),
        };
        if content.is_empty() {
            return notice(EMPTY_NOTICE.to_owned());
        }
        if self.total().saturating_add(content.len()) > free {
            return notice(total_notice());
        }
        let id = self.next;
        self.next += 1;
        let size = content.len() as u64;
        self.pending.push(Pending {
            id,
            name: name.clone(),
            content,
        });
        Choice::Attached { id, name, size }
    }

    /// Removes a chosen attachment. An unknown id is an invalid request.
    pub fn remove(&mut self, id: u64) -> Result<(), AppError> {
        let i = self
            .pending
            .iter()
            .position(|p| p.id == id)
            .ok_or(AppError::InvalidRequest)?;
        drop(self.pending.remove(i));
        Ok(())
    }

    /// The attachments asked for by the form, in its order. Each id only once and only
    /// the ones that exist (the interface is untrusted).
    pub fn chosen(&self, ids: &[u64]) -> Result<Vec<&Pending>, AppError> {
        let mut out: Vec<&Pending> = Vec::with_capacity(ids.len());
        for id in ids {
            if out.iter().any(|p| p.id == *id) {
                return Err(AppError::InvalidRequest);
            }
            out.push(
                self.pending
                    .iter()
                    .find(|p| p.id == *id)
                    .ok_or(AppError::InvalidRequest)?,
            );
        }
        Ok(out)
    }

    /// Zeroes everything (cancel, save, quit).
    pub fn clear(&mut self) {
        self.pending.clear();
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

enum ReadOutcome {
    Content(Secret),
    TooLarge,
}

/// Reads at most `max` bytes of a regular file. The size is checked by the `fstat` of the
/// open file and again by what was read (the file may grow between the two).
///
/// Phase 39f (M5 audit), like the private key loading: `O_NOFOLLOW` refuses a symbolic
/// link in the last component (the user chooses the file, not a shortcut to somewhere
/// else) and `O_NONBLOCK` keeps a FIFO from holding the app on open (it is refused by the
/// `fstat` right after); `O_NOCTTY`, so a chosen terminal does not become the app's
/// terminal.
fn read_limited(path: &Path, max: usize) -> Result<ReadOutcome, ()> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_NOCTTY)
        .open(path)
        .map_err(|_| ())?;
    let meta = file.metadata().map_err(|_| ())?;
    if !meta.is_file() {
        return Err(());
    }
    if meta.len() > max as u64 {
        return Ok(ReadOutcome::TooLarge);
    }
    match read_up_to(file, max)? {
        Some(mut buf) => Ok(ReadOutcome::Content(Secret::new(core::mem::take(
            &mut *buf,
        )))),
        None => Ok(ReadOutcome::TooLarge),
    }
}

/// Reads at most `max` bytes into a fixed buffer of `max + 1`, allocated once and zeroed
/// on drop, like the private key loading does (Phase 39g, M5 audit): the read never
/// reallocates, so no piece of the attachment is left in an old heap block without
/// being zeroed (the `read_to_end` into a `Vec` with the `fstat` capacity reallocated if
/// the file grew between the `fstat` and the read). `None` when it went past `max`. The
/// `truncate` only changes the length; the `Secret` takes ownership of the same block,
/// without a copy.
fn read_up_to(mut file: impl Read, max: usize) -> Result<Option<zeroize::Zeroizing<Vec<u8>>>, ()> {
    let mut buf = zeroize::Zeroizing::new(vec![0u8; max + 1]);
    let mut read = 0usize;
    while read < buf.len() {
        match file.read(&mut buf[read..]) {
            Ok(0) => break,
            Ok(n) => read += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err(()),
        }
    }
    if read > max {
        return Ok(None);
    }
    buf.truncate(read);
    Ok(Some(buf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    fn dir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ward-anexos-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn file(dir: &Path, name: &str, size: usize) -> PathBuf {
        let p = dir.join(name);
        let mut f = File::create(&p).unwrap();
        f.write_all(&vec![b'x'; size]).unwrap();
        p
    }

    #[test]
    fn attaches_lists_and_removes_without_content_in_the_response() {
        let d = dir("ok");
        let mut a = Attachments::default();
        let e = a.attach(&file(&d, "deploy.pem", 1200));
        assert_eq!(
            e,
            Choice::Attached {
                id: 0,
                name: "deploy.pem".into(),
                size: 1200
            }
        );
        let json = serde_json::to_string(&e).unwrap();
        assert!(!json.contains("xxxxxx"));
        assert_eq!(a.chosen(&[0]).unwrap().len(), 1);
        a.remove(0).unwrap();
        assert!(a.is_empty());
        assert_eq!(a.remove(0), Err(AppError::InvalidRequest));
        let _ = std::fs::remove_dir_all(d);
    }

    /// m2:V4.23 (m2:Phase 54): the interface receives the English tag and keys, and the edit
    /// argument refuses the 0.1.x (pt-BR) keys.
    #[test]
    fn attack_old_wire_names_are_refused() {
        let j = |c: &Choice| serde_json::to_string(c).unwrap();
        assert_eq!(
            j(&Choice::Attached {
                id: 3,
                name: "a.pem".into(),
                size: 9
            }),
            r#"{"result":"attached","id":3,"name":"a.pem","size":9}"#
        );
        assert_eq!(j(&Choice::Canceled), r#"{"result":"canceled"}"#);
        assert_eq!(
            j(&Choice::Notice { reason: "x".into() }),
            r#"{"result":"notice","reason":"x"}"#
        );
        let e: AttachmentEdit =
            serde_json::from_str(r#"{"entry":"acme-ssh","kept":["a"]}"#).unwrap();
        assert_eq!((e.entry.as_str(), e.kept.len()), ("acme-ssh", 1));
        assert!(
            serde_json::from_str::<AttachmentEdit>(r#"{"entrada":"x","mantidos":[]}"#).is_err()
        );
    }

    /// V5.12: 300 KiB gives a notice and is not kept; going past 256 KiB in total gives a
    /// notice and is not kept; an empty file is refused; a directory and a missing file
    /// give a fixed notice.
    /// Phase 39f (low finding of the M5 audit): a symbolic link chosen as an attachment
    /// is refused (it does not follow to the target), and a FIFO does not hold the app:
    /// the fixed notice comes back at once, with nothing kept.
    /// Attack (Phase 39g): the file grows between the `fstat` and the read. The read uses
    /// a single block, of `max + 1`, that never reallocates (nothing of the attachment is
    /// left in an old block without being zeroed), and what goes past `max` is refused.
    #[test]
    fn attack_growing_file_does_not_reallocate_the_buffer() {
        /// Reader that hands out 7 bytes at a time, with no known end (the file
        /// "growing").
        struct Growing(usize);
        impl Read for Growing {
            fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
                let n = b.len().min(7).min(self.0);
                b[..n].fill(b'k');
                self.0 -= n;
                Ok(n)
            }
        }
        // Went past `max`: refused. It fit: the block is the same one of `max + 1`.
        let buf = read_up_to(Growing(100), 64).unwrap();
        assert!(buf.is_none(), "passou de max e não foi recusado");
        let buf = read_up_to(Growing(50), 64).unwrap().unwrap();
        assert_eq!(buf.len(), 50);
        assert_eq!(buf.capacity(), 65);
        let p = buf.as_ptr();
        let s = Secret::new({
            let mut b = buf;
            core::mem::take(&mut *b)
        });
        assert_eq!(
            s.expose().as_ptr(),
            p,
            "o Segredo não tomou posse do mesmo bloco"
        );
        // A read error returns nothing.
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("x"))
            }
        }
        assert!(read_up_to(Broken, 64).is_err());
    }

    #[test]
    fn attack_link_and_fifo_are_refused_without_holding() {
        let d = dir("link-fifo");
        let target = file(&d, "alvo.txt", 64);
        let link = d.join("atalho.txt");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let mut a = Attachments::default();
        assert_eq!(
            a.attach(&link),
            Choice::Notice {
                reason: READ_NOTICE.into()
            }
        );
        let fifo = d.join("cano");
        let ok = std::process::Command::new("/usr/bin/mkfifo")
            .arg(&fifo)
            .status()
            .unwrap();
        assert!(ok.success());
        let (tx, rx) = std::sync::mpsc::channel();
        let f = fifo.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Attachments::default().attach(&f));
        });
        let r = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("a FIFO prendeu a leitura do anexo");
        assert_eq!(
            r,
            Choice::Notice {
                reason: READ_NOTICE.into()
            }
        );
        assert!(a.is_empty());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn attack_large_file_total_and_empty_are_not_kept() {
        let d = dir("limites");
        let mut a = Attachments::default();
        assert_eq!(
            a.attach(&file(&d, "grande.bin", 300 * limits::KIB)),
            Choice::Notice {
                reason: too_large_notice()
            }
        );
        assert!(a.is_empty());
        assert!(matches!(
            a.attach(&file(&d, "a.bin", 200 * limits::KIB)),
            Choice::Attached { .. }
        ));
        assert_eq!(
            a.attach(&file(&d, "b.bin", 100 * limits::KIB)),
            Choice::Notice {
                reason: total_notice()
            }
        );
        assert_eq!(
            a.attach(&file(&d, "vazio.txt", 0)),
            Choice::Notice {
                reason: EMPTY_NOTICE.into()
            }
        );
        assert_eq!(a.len(), 1);
        assert_eq!(a.total(), 200 * limits::KIB);
        assert_eq!(
            a.attach(&d),
            Choice::Notice {
                reason: READ_NOTICE.into()
            }
        );
        assert_eq!(
            a.attach(&d.join("nao-existe")),
            Choice::Notice {
                reason: READ_NOTICE.into()
            }
        );
        // Exactly at the total limit passes.
        assert!(matches!(
            a.attach(&file(&d, "c.bin", 56 * limits::KIB)),
            Choice::Attached { .. }
        ));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn attack_ids_forged_by_the_interface() {
        let d = dir("ids");
        let mut a = Attachments::default();
        a.attach(&file(&d, "x.pem", 10));
        assert_eq!(a.chosen(&[0, 0]).err(), Some(AppError::InvalidRequest));
        assert_eq!(a.chosen(&[7]).err(), Some(AppError::InvalidRequest));
        assert!(a.chosen(&[]).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(d);
    }

    /// Phase 39b (V5.12): when editing, the stored attachments that stay count in the
    /// total and in the count; a file that would only fit without them gives a notice and
    /// is not kept.
    #[test]
    fn attack_total_with_the_kept_attachments() {
        let d = dir("base");
        let mut a = Attachments::default();
        let base = Base {
            total: 200 * limits::KIB,
            count: 1,
        };
        assert_eq!(
            a.attach_over(&file(&d, "b.bin", 100 * limits::KIB), base),
            Choice::Notice {
                reason: total_notice()
            }
        );
        assert!(a.is_empty());
        assert!(matches!(
            a.attach_over(&file(&d, "c.bin", 56 * limits::KIB), base),
            Choice::Attached { .. }
        ));
        let full = Base {
            total: 0,
            count: limits::MAX_ATTACHMENTS,
        };
        assert_eq!(
            a.attach_over(&file(&d, "d.bin", 1), full),
            Choice::Notice {
                reason: TOO_MANY_NOTICE.into()
            }
        );
        let _ = std::fs::remove_dir_all(d);
    }

    /// Phase 39b: `.env` is attached as `env` (`ward-core` refuses a leading `.`); a name
    /// made only of dots gives a notice. The stored name never starts with `.`.
    #[test]
    fn attack_name_with_leading_dot() {
        let d = dir("ponto");
        let mut a = Attachments::default();
        let e = a.attach(&file(&d, ".env", 12));
        assert!(
            matches!(&e, Choice::Attached { name, .. } if name == "env"),
            "{e:?}"
        );
        let e = a.attach(&file(&d, "..kube.cfg", 3));
        assert!(
            matches!(&e, Choice::Attached { name, .. } if name == "kube.cfg"),
            "{e:?}"
        );
        assert_eq!(attachment_name("..."), None);
        assert_eq!(attachment_name("."), None);
        assert_eq!(attachment_name(".."), None);
        assert_eq!(attachment_name("a.pem"), Some("a.pem"));
        assert!(
            a.chosen(&[0, 1])
                .unwrap()
                .iter()
                .all(|p| !p.name.starts_with('.'))
        );
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn attack_too_many_attachments() {
        let d = dir("demais");
        let mut a = Attachments::default();
        let p = file(&d, "um.txt", 1);
        for _ in 0..limits::MAX_ATTACHMENTS {
            assert!(matches!(a.attach(&p), Choice::Attached { .. }));
        }
        assert_eq!(
            a.attach(&p),
            Choice::Notice {
                reason: TOO_MANY_NOTICE.into()
            }
        );
        let _ = std::fs::remove_dir_all(d);
    }
}
