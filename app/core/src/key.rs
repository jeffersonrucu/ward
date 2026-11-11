//! "Load from file" of the "Private key" field (Phase 53a; S2, S11, S12).
//!
//! The private key goes from the chosen file to the daemon without passing through the
//! clipboard (`wl-copy` writes the input to `/tmp`, S2) nor through the webview (S11). The
//! interface only sends the load key command with the field name; the one who chooses the file is the
//! same [`Picker`](crate::attachments::Picker) as the attachments (in the app, the GTK
//! dialog; in the test bridge, the fixed folder from the command line). The file is read
//! here, checked and kept in a [`Secret`] (zeroes on drop) until saving, cancelling or
//! leaving the form; saving puts it in the draft (as a loaded value). The response carries
//! only the size, or a fixed notice.
//!
//! Refusals (none keeps anything nor tells the path or the system error, S13):
//! - symbolic link in the last component (`O_NOFOLLOW`): the user chooses the key file,
//!   not a shortcut that may point somewhere else;
//! - what is not a regular file (folder, FIFO, device, socket); `O_NONBLOCK` keeps a FIFO
//!   from holding the app on open;
//! - above `MAX_SECRET` (64 KiB, the `ward-core` limit for a text secret), by the `fstat`
//!   and again by what was read;
//! - empty;
//! - what is not an OpenSSH or PEM private key: only printable ASCII, a
//!   `-----BEGIN <label>-----` line with a private key label, the base64 body (and the
//!   `Name: value` headers of the old encrypted PEM) and the `-----END` line of the same
//!   label. Public key, PuTTY `.ppk`, certificate and any text are refused.

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use serde::Serialize;
use ward_core::limits;

use crate::secret::Secret;

/// Form fields that accept "Load from file".
pub const LOADABLE_FIELDS: &[&str] = &["private_key"];

/// Largest file accepted: the `ward-core` limit of a text secret.
pub const MAX_KEY: usize = limits::MAX_SECRET;

/// PEM/OpenSSH private key labels.
const LABELS: &[&str] = &[
    "OPENSSH PRIVATE KEY",
    "RSA PRIVATE KEY",
    "EC PRIVATE KEY",
    "DSA PRIVATE KEY",
    "PRIVATE KEY",
    "ENCRYPTED PRIVATE KEY",
];

pub const LINK_NOTICE: &str = "o arquivo é um link simbólico; escolha o próprio arquivo da chave";
pub const NOT_REGULAR_NOTICE: &str = "o item escolhido não é um arquivo comum";
pub const EMPTY_NOTICE: &str = "arquivo vazio: não há chave para carregar";
pub const READ_NOTICE: &str = "não foi possível ler o arquivo";
pub const NOT_A_KEY_NOTICE: &str = "o arquivo não é uma chave privada OpenSSH ou PEM";

pub fn too_big_notice() -> String {
    format!(
        "o arquivo passa de {} KiB e não é uma chave privada",
        MAX_KEY / limits::KIB
    )
}

/// What the interface receives from the load key command. Never the content nor the path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "result")]
pub enum Loading {
    #[serde(rename = "loaded")]
    Loaded { size: u64 },
    #[serde(rename = "canceled")]
    Canceled,
    /// Nothing was kept; the reason is a fixed text.
    #[serde(rename = "notice")]
    Notice { reason: String },
}

/// Reads and checks the key file. `Err` carries the fixed notice.
pub fn read_key(path: &Path) -> Result<Secret, String> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_NOCTTY)
        .open(path)
        .map_err(|e| match e.raw_os_error() {
            Some(libc::ELOOP) => LINK_NOTICE.to_owned(),
            _ => READ_NOTICE.to_owned(),
        })?;
    let meta = file.metadata().map_err(|_| READ_NOTICE.to_owned())?;
    if !meta.is_file() {
        return Err(NOT_REGULAR_NOTICE.to_owned());
    }
    if meta.len() > MAX_KEY as u64 {
        return Err(too_big_notice());
    }
    let mut content = read_up_to(file)?;
    if content.is_empty() {
        return Err(EMPTY_NOTICE.to_owned());
    }
    if !is_private_key(&content) {
        return Err(NOT_A_KEY_NOTICE.to_owned());
    }
    Ok(Secret::new(core::mem::take(&mut *content)))
}

type ReadBuf = zeroize::Zeroizing<Vec<u8>>;

/// Reads at most `MAX_KEY` bytes (the file may grow between the `fstat` and the read)
/// into a fixed buffer of `MAX_KEY + 1`, allocated once and zeroed on drop (Phase 53d):
/// the read never reallocates, so no piece of the key stays in an old heap block without
/// being zeroed. The `truncate` only changes the length; the capacity (and the block) stay
/// the same and the `Secret` takes ownership of it without a copy.
fn read_up_to(mut file: File) -> Result<ReadBuf, String> {
    let mut buf = zeroize::Zeroizing::new(vec![0u8; MAX_KEY + 1]);
    let mut read = 0usize;
    while read < buf.len() {
        match file.read(&mut buf[read..]) {
            Ok(0) => break,
            Ok(n) => read += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err(READ_NOTICE.to_owned()),
        }
    }
    if read > MAX_KEY {
        return Err(too_big_notice());
    }
    buf.truncate(read);
    Ok(buf)
}

fn base64(line: &str) -> bool {
    !line.is_empty()
        && line
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'/' | b'='))
}

/// Header of the old encrypted PEM (`Proc-Type: 4,ENCRYPTED`, `DEK-Info: …`).
fn pem_header(line: &str) -> bool {
    line.split_once(": ").is_some_and(|(name, value)| {
        !name.is_empty()
            && name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
            && !value.is_empty()
    })
}

/// OpenSSH or PEM private key, by its shape (the key content is not interpreted).
pub fn is_private_key(b: &[u8]) -> bool {
    if !b
        .iter()
        .all(|c| matches!(c, b'\n' | b'\r') || (0x20..0x7f).contains(c))
    {
        return false;
    }
    let Ok(text) = core::str::from_utf8(b) else {
        return false;
    };
    let mut lines = text.lines().map(str::trim_end).filter(|l| !l.is_empty());
    let Some(first) = lines.next() else {
        return false;
    };
    let Some(label) = first
        .strip_prefix("-----BEGIN ")
        .and_then(|r| r.strip_suffix("-----"))
        .filter(|r| LABELS.contains(r))
    else {
        return false;
    };
    let end = format!("-----END {label}-----");
    let mut body = 0usize;
    let mut headers = true;
    let mut closed = false;
    for line in lines {
        if closed {
            // Nothing after the END (another key, certificate, text).
            return false;
        }
        if line == end {
            closed = true;
        } else if base64(line) {
            body += 1;
            headers = false;
        } else if headers && body == 0 && pem_header(line) {
            // Only before the base64.
        } else {
            return false;
        }
    }
    closed && body > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;

    fn test_dir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ward-chave-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    /// Fake key in the OpenSSH shape, built in pieces (the whole text of a key does not
    /// stay in the repository).
    pub(crate) fn fake_key(label: &str, body: &str) -> String {
        let (a, b) = ("-----BEGIN ", "-----END ");
        format!("{a}{label}-----\n{body}\n{b}{label}-----\n")
    }

    fn write_file(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let p = dir.join(name);
        let mut f = File::create(&p).unwrap();
        f.write_all(content).unwrap();
        p
    }

    #[test]
    fn accepts_openssh_and_pem() {
        for r in LABELS {
            let k = fake_key(r, "QUFBQUFBQUFBQUFB\nZmFsc28tbWlvbG8=");
            assert!(is_private_key(k.as_bytes()), "{r}");
        }
        // CRLF and blank lines at the ends.
        let k = fake_key("OPENSSH PRIVATE KEY", "ZmFsc28=").replace('\n', "\r\n");
        assert!(is_private_key(format!("\n{k}\n\n").as_bytes()));
        // Old encrypted PEM: headers before the base64.
        let k = fake_key(
            "RSA PRIVATE KEY",
            "Proc-Type: 4,ENCRYPTED\nDEK-Info: AES-128-CBC,00FF\n\nZmFsc28=",
        );
        assert!(is_private_key(k.as_bytes()));
    }

    #[test]
    fn attack_what_is_not_a_private_key_is_refused() {
        let pub_ = [
            "ssh-ed25519 ",
            "AAAAC3NzaC1lZDI1NTE5AAAAIfalso falso@host\n",
        ]
        .concat();
        let cases: Vec<String> = vec![
            pub_,
            fake_key("PUBLIC KEY", "ZmFsc28="),
            fake_key("CERTIFICATE", "ZmFsc28="),
            fake_key("OPENSSH PRIVATE KEY", ""),
            "PuTTY-User-Key-File-3: ssh-ed25519\nEncryption: none\n".into(),
            "falso texto qualquer\n".into(),
            // END of another label.
            fake_key("OPENSSH PRIVATE KEY", "ZmFsc28=")
                .replace("END OPENSSH PRIVATE", "END RSA PRIVATE"),
            // No END.
            ["-----BEGIN ", "OPENSSH PRIVATE KEY-----\nZmFsc28=\n"].concat(),
            // Two keys, or text after the END.
            fake_key("OPENSSH PRIVATE KEY", "ZmFsc28=").repeat(2),
            fake_key("OPENSSH PRIVATE KEY", "ZmFsc28=") + "falso\n",
            // Text before the BEGIN.
            format!("falso\n{}", fake_key("OPENSSH PRIVATE KEY", "ZmFsc28=")),
            // Body with a character outside base64, control, non-ASCII, NUL.
            fake_key("OPENSSH PRIVATE KEY", "ZmFs c28="),
            fake_key("OPENSSH PRIVATE KEY", "ZmFs\u{1b}[2Jc28="),
            fake_key("OPENSSH PRIVATE KEY", "ZmFsc28\u{e9}"),
            fake_key("OPENSSH PRIVATE KEY", "ZmFs\0c28="),
            // Header after the base64.
            fake_key("RSA PRIVATE KEY", "ZmFsc28=\nProc-Type: 4,ENCRYPTED"),
        ];
        for c in cases {
            assert!(!is_private_key(c.as_bytes()), "{c:?}");
        }
        assert!(!is_private_key(b""));
        assert!(!is_private_key(&[0xff, 0xfe]));
    }

    #[test]
    fn reads_the_key_without_returning_the_content() {
        let d = test_dir("ok");
        let k = fake_key("OPENSSH PRIVATE KEY", "QUFBQUFBQUFBQUFB");
        let p = write_file(&d, "deploy_ed25519", k.as_bytes());
        let s = read_key(&p).unwrap();
        assert_eq!(s.len(), k.len());
        assert_eq!(s.expose(), k.as_bytes());
        assert!(!format!("{s:?}").contains("QUFB"));
        let response = serde_json::to_string(&Loading::Loaded {
            size: s.len() as u64,
        })
        .unwrap();
        assert_eq!(
            response,
            format!("{{\"result\":\"loaded\",\"size\":{}}}", k.len())
        );
        let _ = std::fs::remove_dir_all(d);
    }

    /// m2:V4.23 (m2:Phase 54): the webview receives the English tag values and fields.
    #[test]
    fn webview_tag_and_fields_are_the_english_ones() {
        assert_eq!(
            serde_json::to_string(&Loading::Canceled).unwrap(),
            r#"{"result":"canceled"}"#
        );
        assert_eq!(
            serde_json::to_string(&Loading::Notice {
                reason: EMPTY_NOTICE.to_owned()
            })
            .unwrap(),
            format!(r#"{{"result":"notice","reason":"{EMPTY_NOTICE}"}}"#)
        );
    }

    /// Phase 53d: the read uses a fixed buffer of `MAX_KEY + 1` and does not reallocate;
    /// the `Secret` keeps the same block (same capacity), small, at the limit or above.
    #[test]
    fn attack_read_does_not_reallocate_the_key_buffer() {
        let d = test_dir("buffer");
        let k = fake_key("OPENSSH PRIVATE KEY", "QUFBQUFBQUFBQUFB");
        let p = write_file(&d, "pequena", k.as_bytes());
        let read = read_up_to(File::open(&p).unwrap()).unwrap();
        assert_eq!(read.len(), k.len());
        assert_eq!(read.capacity(), MAX_KEY + 1);
        let ptr = read.as_ptr();
        let s = Secret::new({
            let mut l = read;
            core::mem::take(&mut *l)
        });
        assert_eq!(s.expose().as_ptr(), ptr, "o Segredo fica com o mesmo bloco");

        let at_limit = write_file(&d, "limite", &vec![b'Q'; MAX_KEY]);
        let read = read_up_to(File::open(&at_limit).unwrap()).unwrap();
        assert_eq!((read.len(), read.capacity()), (MAX_KEY, MAX_KEY + 1));

        // File larger than the limit (one that grows after the `fstat` lands here):
        // refused.
        let above = write_file(&d, "acima", &vec![b'Q'; MAX_KEY * 2]);
        assert_eq!(
            read_up_to(File::open(&above).unwrap()).err(),
            Some(too_big_notice())
        );
        let _ = std::fs::remove_dir_all(d);
    }

    /// S2/S11: link, folder, FIFO, large, empty, missing and what is not a key give a
    /// fixed notice, with no path nor content, and nothing is kept.
    #[test]
    fn attack_link_fifo_large_empty_and_text_are_refused() {
        let d = test_dir("ataques");
        let k = fake_key("OPENSSH PRIVATE KEY", "QUFBQUFBQUFBQUFB");
        let target = write_file(&d, "chave", k.as_bytes());
        let link = d.join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert_eq!(read_key(&link).err().as_deref(), Some(LINK_NOTICE));
        // Link pointing outside (the attacker points to another file of the user).
        let outside = d.join("fora");
        std::os::unix::fs::symlink("/etc/hostname", &outside).unwrap();
        assert_eq!(read_key(&outside).err().as_deref(), Some(LINK_NOTICE));

        assert_eq!(read_key(&d).err().as_deref(), Some(NOT_REGULAR_NOTICE));
        let fifo = d.join("fifo");
        let ok = std::process::Command::new("/usr/bin/mkfifo")
            .arg(&fifo)
            .status()
            .is_ok_and(|s| s.success());
        if ok {
            // No writer on the other side: with O_NONBLOCK, the open does not hang.
            assert_eq!(read_key(&fifo).err().as_deref(), Some(NOT_REGULAR_NOTICE));
        }
        assert_eq!(
            read_key(Path::new("/dev/zero")).err().as_deref(),
            Some(NOT_REGULAR_NOTICE)
        );

        let large = fake_key("OPENSSH PRIVATE KEY", &"QUFB\n".repeat(MAX_KEY / 5));
        assert!(large.len() > MAX_KEY);
        let g = write_file(&d, "grande", large.as_bytes());
        assert_eq!(read_key(&g).err(), Some(too_big_notice()));
        let v = write_file(&d, "vazio", b"");
        assert_eq!(read_key(&v).err().as_deref(), Some(EMPTY_NOTICE));
        let t = write_file(&d, "texto.txt", b"falso-senha-em-texto\n");
        assert_eq!(read_key(&t).err().as_deref(), Some(NOT_A_KEY_NOTICE));
        let p = write_file(
            &d,
            "id.pub",
            ["ssh-ed25519 ", "AAAAC3Nzfalso falso@host\n"]
                .concat()
                .as_bytes(),
        );
        assert_eq!(read_key(&p).err().as_deref(), Some(NOT_A_KEY_NOTICE));
        assert_eq!(
            read_key(&d.join("nao-existe")).err().as_deref(),
            Some(READ_NOTICE)
        );
        // No read permission.
        let closed = write_file(&d, "fechado", k.as_bytes());
        std::fs::set_permissions(&closed, std::os::unix::fs::PermissionsExt::from_mode(0o000))
            .unwrap();
        if File::open(&closed).is_err() {
            assert_eq!(read_key(&closed).err().as_deref(), Some(READ_NOTICE));
        }
        // Exactly at the limit passes (if it has the shape of a key).
        let max_body = MAX_KEY - fake_key("OPENSSH PRIVATE KEY", "").len();
        let at_limit = fake_key("OPENSSH PRIVATE KEY", &"Q".repeat(max_body));
        assert_eq!(at_limit.len(), MAX_KEY);
        let n = write_file(&d, "no-limite", at_limit.as_bytes());
        assert_eq!(read_key(&n).map(|s| s.len()).ok(), Some(MAX_KEY));
        // No notice mentions a path.
        for a in [
            LINK_NOTICE,
            NOT_REGULAR_NOTICE,
            EMPTY_NOTICE,
            READ_NOTICE,
            NOT_A_KEY_NOTICE,
        ] {
            assert!(!a.contains('/'));
        }
        let _ = std::fs::remove_dir_all(d);
    }
}
