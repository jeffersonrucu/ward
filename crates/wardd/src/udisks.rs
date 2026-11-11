//! udisks2 on the system bus (Phase 46): which USB drives exist and when one is
//! connected or mounted.
//!
//! - **Query** ([`Udisks`]): `GetManagedObjects` of `org.freedesktop.UDisks2`, on a new
//!   connection for each query, with the call deadline of Phase 24. From each object
//!   with `Block` and `Filesystem` come `IdUUID`, `IdLabel`, `DeviceNumber` and the first
//!   of `MountPoints` ([`read_objects`]). Only file systems that the desktop shows get
//!   in: `HintIgnore` false and `HintSystem` false (USB drive, card), or a loop device
//!   (the V7.4 test image mounted by `udisksctl`).
//! - **Everything that comes from the bus is checked**: UUID in the D45 format, label
//!   without control characters and cut at [`MAX_LABEL`], absolute mount point, without
//!   NUL, without `.`/`..` and with up to 4096 bytes (otherwise, it counts as not
//!   mounted), and at most [`MAX_OBJECTS`] objects read. What guarantees that the mount
//!   point is the UUID's is [`crate::external`], through the `st_dev` of the open fd.
//! - **Watcher** ([`watch_udisks`]): a thread subscribes to the signals of
//!   `org.freedesktop.UDisks2` (`InterfacesAdded`, `PropertiesChanged` of
//!   `MountPoints`…) and, on each one, pokes the copy thread, which waits a bit (the
//!   signals come in bursts), asks the daemon for the copy
//!   ([`Daemon::sincronizar_externo`]) and records the result with a fixed text. The
//!   copy is also poked by the daemon itself (unlocking, writing, turning the external
//!   backup on) and runs every [`COPY_INTERVAL`], so that a lost signal does not leave
//!   the USB drive without a copy.
//! - No signal or response from the bus unlocks, writes the vault or changes the
//!   config: the worst a fake udisks2 can do is point to a directory of the very device
//!   it names, and what goes there is the encrypted backup.
//! - The address is the same as Phase 24's ([`crate::trava::barramento_do_daemon`]):
//!   fixed in the release; in the test daemon, only `WARD_DEV_DBUS_SISTEMA`, and without
//!   it neither logind nor udisks2 are turned on.

use std::collections::HashMap;
use std::io;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::Duration;

use std::future::poll_fn;
use std::num::NonZeroUsize;
use std::pin::Pin;

use futures_core::Stream;
use zbus::message::Type as MessageType;
use zbus::{Connection, MatchRule, MessageStream, OwnedObjectPath, OwnedVariant, Variant};

use ward_core::config::ExternalBackup;
use ward_core::protocol::MAX_LABEL;

use crate::external::{ExternalError, Seen, UsbDriveSource};
use crate::nucleo::Daemon;
use crate::trava::{ESPERA_MAXIMA_BARRAMENTO, conectar};
use crate::{aviso, depurar};

const UDISKS: &str = "org.freedesktop.UDisks2";
const UDISKS_PATH: &str = "/org/freedesktop/UDisks2";
const IFACE_OBJECTS: &str = "org.freedesktop.DBus.ObjectManager";
const IFACE_BLOCK: &str = "org.freedesktop.UDisks2.Block";
const IFACE_FS: &str = "org.freedesktop.UDisks2.Filesystem";
const IFACE_LOOP: &str = "org.freedesktop.UDisks2.Loop";
/// Most objects read from a udisks2 response.
pub const MAX_OBJECTS: usize = 4096;
/// Longest mount path accepted.
const MAX_MOUNT: usize = 4096;
/// Wait after a signal, to gather the burst (`MountPoints` arrives after
/// `InterfacesAdded`).
pub const BURST_WAIT: Duration = Duration::from_millis(500);
/// How often the copy runs even without a signal.
pub const COPY_INTERVAL: Duration = Duration::from_secs(300);
const QUEUE: NonZeroUsize = NonZeroUsize::new(256).unwrap();

/// Objects of `GetManagedObjects`: path → interface → property → value.
pub type Objects = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedVariant>>>;

fn text(p: &HashMap<String, OwnedVariant>, k: &str) -> Option<String> {
    match p.get(k).map(|v| &**v) {
        Some(Variant::Str(s)) => Some(s.as_str().to_owned()),
        _ => None,
    }
}

fn boolean(p: &HashMap<String, OwnedVariant>, k: &str) -> Option<bool> {
    match p.get(k).map(|v| &**v) {
        Some(Variant::Bool(b)) => Some(*b),
        _ => None,
    }
}

fn number(p: &HashMap<String, OwnedVariant>, k: &str) -> Option<u64> {
    match p.get(k).map(|v| &**v) {
        Some(Variant::U64(n)) => Some(*n),
        _ => None,
    }
}

/// `aay` → bytes of each item.
fn bytes_from_aay(v: &Variant<'_>) -> Option<Vec<Vec<u8>>> {
    let Variant::Array(list) = v else {
        return None;
    };
    let mut out = Vec::new();
    for item in list.inner() {
        let Variant::Array(b) = item else {
            return None;
        };
        let mut bytes = Vec::with_capacity(b.len());
        for x in b.inner() {
            let Variant::U8(c) = x else {
                return None;
            };
            bytes.push(*c);
        }
        out.push(bytes);
    }
    Some(out)
}

/// Accepted mount path: the `ay` without the trailing NUL, absolute, without NUL in the
/// middle, without `.`/`..` and up to [`MAX_MOUNT`] bytes.
pub fn valid_mount(mut b: Vec<u8>) -> Option<PathBuf> {
    if b.last() == Some(&0) {
        b.pop();
    }
    if b.is_empty() || b.len() > MAX_MOUNT || b[0] != b'/' || b.contains(&0) {
        return None;
    }
    // Each part after the root: not empty (no `//` nor trailing `/`), not `.`, not `..`.
    let simple = b == b"/"
        || b[1..]
            .split(|c| *c == b'/')
            .all(|part| !part.is_empty() && part != b"." && part != b"..");
    simple.then(|| PathBuf::from(std::ffi::OsString::from_vec(b)))
}

/// Label to show: without control characters, cut at [`MAX_LABEL`] bytes.
pub fn clean_label(r: &str) -> String {
    let mut s = String::new();
    for c in r.chars().filter(|c| !c.is_control()) {
        if s.len() + c.len_utf8() > MAX_LABEL {
            break;
        }
        s.push(c);
    }
    s
}

/// The USB drives of the udisks2 objects, sorted by UUID. An object that does not pass
/// the checks is left out.
pub fn read_objects(objects: &Objects) -> Vec<Seen> {
    let mut seen = Vec::new();
    for ifaces in objects.values().take(MAX_OBJECTS) {
        let (Some(block), Some(fs)) = (ifaces.get(IFACE_BLOCK), ifaces.get(IFACE_FS)) else {
            continue;
        };
        if text(block, "IdUsage").as_deref() != Some("filesystem")
            || boolean(block, "HintIgnore") != Some(false)
        {
            continue;
        }
        let is_loop = ifaces.contains_key(IFACE_LOOP);
        if boolean(block, "HintSystem") != Some(false) && !is_loop {
            continue;
        }
        let Some(uuid) = text(block, "IdUUID") else {
            continue;
        };
        if ExternalBackup::new(false, Some(&uuid)).is_err() {
            continue;
        }
        let Some(device) = number(block, "DeviceNumber") else {
            continue;
        };
        let label = clean_label(&text(block, "IdLabel").unwrap_or_default());
        let mount = fs
            .get("MountPoints")
            .and_then(|v| bytes_from_aay(v))
            .and_then(|l| l.into_iter().next())
            .and_then(valid_mount);
        seen.push(Seen {
            uuid,
            label,
            mount,
            device,
        });
    }
    seen.sort_by(|a, b| a.uuid.cmp(&b.uuid).then(a.device.cmp(&b.device)));
    seen
}

async fn query(conn: &Connection) -> Option<Vec<Seen>> {
    let r = conn
        .call_method(
            Some(UDISKS),
            UDISKS_PATH,
            Some(IFACE_OBJECTS),
            "GetManagedObjects",
            &(),
        )
        .await
        .ok()?;
    let objects: Objects = r.body().deserialize().ok()?;
    Some(read_objects(&objects))
}

/// udisks2 on the bus at `socket` (a new connection for each query).
pub struct Udisks {
    socket: PathBuf,
}

impl Udisks {
    pub fn new(socket: PathBuf) -> Self {
        Self { socket }
    }
}

impl UsbDriveSource for Udisks {
    fn usb_drives(&self) -> Option<Vec<Seen>> {
        zbus::block_on(async {
            let conn = conectar(&self.socket).await?;
            query(&conn).await
        })
    }
}

async fn next_message(signals: &mut MessageStream) -> Option<zbus::Result<zbus::Message>> {
    poll_fn(|cx| Pin::new(&mut *signals).poll_next(cx)).await
}

/// A signal session: pokes the copy on each udisks2 signal. `false` if it did not
/// connect.
async fn session(daemon: &Daemon, socket: &Path, ready: &dyn Fn()) -> bool {
    let Some(conn) = conectar(socket).await else {
        return false;
    };
    let Ok(rule) = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .sender(UDISKS)
        .path_namespace(UDISKS_PATH)
        .build()
    else {
        return false;
    };
    let Ok(mut signals) = MessageStream::for_match_rule(rule, &conn, Some(QUEUE)).await else {
        return false;
    };
    depurar!("udisks2: vigiando pendrives");
    ready();
    // The USB drive may already be connected.
    daemon.cutucar_externo();
    while let Some(Ok(_)) = next_message(&mut signals).await {
        daemon.cutucar_externo();
    }
    true
}

/// Records the result of a copy, with a fixed text.
pub fn record(r: &Result<Option<crate::external::BackupCopy>, ExternalError>) {
    match r {
        Ok(None) => {}
        Ok(Some(c)) if c.copied.is_empty() => depurar!("backup externo: pendrive em dia"),
        Ok(Some(c)) => aviso!(
            "backup externo: {} versão(ões) copiada(s) para o pendrive",
            c.copied.len()
        ),
        // An absent USB drive is the normal case.
        Err(ExternalError::NotConnected | ExternalError::NotMounted) => {}
        Err(e) => aviso!("backup externo não copiado ({})", e.code()),
    }
}

/// Copy thread: waits for a poke (or [`COPY_INTERVAL`]), gathers the burst and copies.
pub fn watch_copy(
    daemon: Arc<Daemon>,
    source: Arc<dyn UsbDriveSource>,
    pokes: Receiver<()>,
) -> io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("wardd-externo".into())
        .spawn(move || {
            loop {
                match pokes.recv_timeout(COPY_INTERVAL) {
                    Ok(()) | Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
                std::thread::sleep(BURST_WAIT);
                while pokes.try_recv().is_ok() {}
                record(&daemon.sincronizar_externo(&*source));
            }
        })
}

/// Thread that watches the udisks2 signals at `socket` forever. `ready` is called on
/// each connection opened (the tests wait for it).
pub fn watch_udisks(
    daemon: Arc<Daemon>,
    socket: PathBuf,
    ready: Box<dyn Fn() + Send>,
) -> io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("wardd-udisks".into())
        .spawn(move || {
            let mut wait = Duration::from_secs(1);
            let mut warned = false;
            loop {
                if zbus::block_on(session(&daemon, &socket, &*ready)) {
                    wait = Duration::from_secs(1);
                    warned = false;
                } else if !warned {
                    aviso!("udisks2: barramento indisponível; backup externo parado até conectar");
                    warned = true;
                }
                std::thread::sleep(wait);
                wait = (wait * 2).min(ESPERA_MAXIMA_BARRAMENTO);
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attack_strange_mount_counts_as_not_mounted() {
        for bad in [
            &b""[..],
            b"\0",
            b"relativo/x",
            b"/media/u/../../etc",
            b"/media/u/./x",
            b"/media/u/a\0b",
            b"/media/u/x/.",
            b"/media//u",
            b"/media/u/",
        ] {
            assert_eq!(valid_mount(bad.to_vec()), None, "{bad:?}");
        }
        let long = [b"/".as_slice(), &[b'a'; MAX_MOUNT]].concat();
        assert_eq!(valid_mount(long), None);
        assert_eq!(
            valid_mount(b"/media/usuario/PEN\0".to_vec()),
            Some(PathBuf::from("/media/usuario/PEN"))
        );
    }

    #[test]
    fn label_without_control_and_cut() {
        assert_eq!(clean_label("PEN\u{1b}[2J\n"), "PEN[2J");
        let long = "é".repeat(MAX_LABEL);
        let r = clean_label(&long);
        assert!(r.len() <= MAX_LABEL && r.chars().all(|c| c == 'é'));
    }
}
