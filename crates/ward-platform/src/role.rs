//! What the daemon needs to know to give a role by the group (D47): the gids of
//! `ward-app` and `ward-nm`, the installed Chrome ([`Install`]) and the kernel policy
//! that makes the setgid safe ([`KernelPolicy`]).

use std::fmt;
use std::path::Path;

use crate::process::{AllowedBinary, Refusal};
use crate::sys;

/// Name of the app's system group.
pub const APP_GROUP: &str = "ward-app";
/// Name of the extension host's system group.
pub const EXTENSION_GROUP: &str = "ward-nm";
/// gid the kernel shows for a gid with no mapping in a user namespace.
const UNMAPPED_GID: u32 = 65534;
/// Largest `/etc/group` or `/etc/subgid` file read.
const MAX_GROUPS_FILE: u64 = 4 * 1024 * 1024;

/// Groups and Chrome of the install. Built only by [`Install::new`] (or
/// [`Install::from_system`]), which refuses gids the user can take on by themselves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Install {
    app_gid: u32,
    extension_gid: u32,
    chrome: AllowedBinary,
}

/// Install refused. No user data: it can go to the log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallError {
    /// The group does not exist, shows up more than once or the line is malformed.
    MissingGroup(&'static str),
    /// The group has members (they would take on the group with `newgrp`/`sg`).
    GroupWithMembers(&'static str),
    /// gid 0, 65534 (unmapped), the same in both groups, or a gid this process already
    /// has (real, effective or supplementary).
    InvalidGid(&'static str),
    /// The gid falls in a range of `/etc/subgid` (a user namespace would map it).
    GidInSubgid(&'static str),
    /// `/etc/group`, `/etc/subgid` or `/proc/self/status` unreadable or too large.
    Read,
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstallError::MissingGroup(g) => write!(f, "grupo {g} ausente ou repetido"),
            InstallError::GroupWithMembers(g) => {
                write!(f, "o grupo {g} tem membros; ele tem de ficar vazio")
            }
            InstallError::InvalidGid(g) => write!(
                f,
                "gid do grupo {g} inválido (0, 65534, repetido ou já deste processo)"
            ),
            InstallError::GidInSubgid(g) => {
                write!(f, "gid do grupo {g} dentro de uma faixa do /etc/subgid")
            }
            InstallError::Read => f.write_str("não deu para ler a lista de grupos"),
        }
    }
}

impl std::error::Error for InstallError {}

impl Install {
    /// Verifies the gids against this process (real, effective and supplementary) and
    /// against `/etc/subgid`. The daemon runs as the user: if it already has the gid,
    /// every process of the user has it too and would become the app.
    pub fn new(
        app_gid: u32,
        extension_gid: u32,
        chrome: AllowedBinary,
    ) -> Result<Install, InstallError> {
        let mine = own_gids()?;
        let subgid = read_limited(Path::new("/etc/subgid"))?;
        for (gid, name) in [(app_gid, APP_GROUP), (extension_gid, EXTENSION_GROUP)] {
            if gid == 0 || gid == UNMAPPED_GID || mine.contains(&gid) {
                return Err(InstallError::InvalidGid(name));
            }
            if gid_in_subgid(subgid.as_deref().unwrap_or_default(), gid) {
                return Err(InstallError::GidInSubgid(name));
            }
        }
        if app_gid == extension_gid {
            return Err(InstallError::InvalidGid(EXTENSION_GROUP));
        }
        Ok(Install {
            app_gid,
            extension_gid,
            chrome,
        })
    }

    /// Reads `ward-app` and `ward-nm` from `/etc/group` (one line each, no members) and
    /// goes through [`new`](Self::new).
    pub fn from_system(chrome: AllowedBinary) -> Result<Install, InstallError> {
        let groups = read_limited(Path::new("/etc/group"))?.ok_or(InstallError::Read)?;
        let app_gid = group_gid(&groups, APP_GROUP)?;
        let extension_gid = group_gid(&groups, EXTENSION_GROUP)?;
        Install::new(app_gid, extension_gid, chrome)
    }

    /// Only in `test-mode`: without the checks against this process, for tests that use
    /// a supplementary group of the user (with `sg`).
    #[cfg(feature = "test-mode")]
    pub fn for_test(app_gid: u32, extension_gid: u32, chrome: AllowedBinary) -> Install {
        Install {
            app_gid,
            extension_gid,
            chrome,
        }
    }

    pub fn app_gid(&self) -> u32 {
        self.app_gid
    }

    pub fn extension_gid(&self) -> u32 {
        self.extension_gid
    }

    pub fn chrome(&self) -> &AllowedBinary {
        &self.chrome
    }
}

/// Reads a system configuration file with a limit. `Ok(None)` if it does not exist.
fn read_limited(path: &Path) -> Result<Option<Vec<u8>>, InstallError> {
    use std::io::Read;
    let file = match std::fs::File::open(path) {
        Ok(a) => a,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(InstallError::Read),
    };
    let mut buf = Vec::new();
    file.take(MAX_GROUPS_FILE + 1)
        .read_to_end(&mut buf)
        .map_err(|_| InstallError::Read)?;
    if buf.len() as u64 > MAX_GROUPS_FILE {
        return Err(InstallError::Read);
    }
    Ok(Some(buf))
}

/// Real, effective and supplementary gids of this process (`Groups:` of the status).
fn own_gids() -> Result<Vec<u32>, InstallError> {
    let status = std::fs::read("/proc/self/status").map_err(|_| InstallError::Read)?;
    let mut gids = vec![sys::getgid(), sys::getegid()];
    let line = status
        .split(|&b| b == b'\n')
        .find_map(|l| l.strip_prefix(b"Groups:"))
        .ok_or(InstallError::Read)?;
    let text = std::str::from_utf8(line).map_err(|_| InstallError::Read)?;
    for g in text.split_whitespace() {
        gids.push(g.parse().map_err(|_| InstallError::Read)?);
    }
    // `Gid:` has the real, effective, saved and file system ones.
    if let Some(l) = status
        .split(|&b| b == b'\n')
        .find_map(|l| l.strip_prefix(b"Gid:"))
    {
        let text = std::str::from_utf8(l).map_err(|_| InstallError::Read)?;
        for g in text.split_whitespace() {
            gids.push(g.parse().map_err(|_| InstallError::Read)?);
        }
    }
    Ok(gids)
}

/// gid of the group `name` in the `/etc/group` format (`name:password:gid:members`).
/// Requires a single line and an empty member list.
pub(crate) fn group_gid(groups: &[u8], name: &'static str) -> Result<u32, InstallError> {
    let mut found = None;
    for line in groups.split(|&b| b == b'\n') {
        let mut fields = line.split(|&b| b == b':');
        if fields.next() != Some(name.as_bytes()) {
            continue;
        }
        if found.is_some() {
            return Err(InstallError::MissingGroup(name));
        }
        let _password = fields.next().ok_or(InstallError::MissingGroup(name))?;
        let gid = fields
            .next()
            .and_then(|g| std::str::from_utf8(g).ok())
            .filter(|g| !g.is_empty() && g.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|g| g.parse::<u32>().ok())
            .ok_or(InstallError::MissingGroup(name))?;
        let members = fields.next().ok_or(InstallError::MissingGroup(name))?;
        if fields.next().is_some() {
            return Err(InstallError::MissingGroup(name));
        }
        if !members.is_empty() {
            return Err(InstallError::GroupWithMembers(name));
        }
        found = Some(gid);
    }
    found.ok_or(InstallError::MissingGroup(name))
}

/// `gid` inside some `name:start:count` range of `/etc/subgid`, of any user. A
/// malformed line counts as "inside" (fails closed).
pub(crate) fn gid_in_subgid(subgid: &[u8], gid: u32) -> bool {
    for line in subgid.split(|&b| b == b'\n') {
        if line.is_empty() || line.starts_with(b"#") {
            continue;
        }
        let fields: Vec<&[u8]> = line.split(|&b| b == b':').collect();
        let number = |c: &[u8]| std::str::from_utf8(c).ok()?.parse::<u64>().ok();
        let (Some(start), Some(count)) = (
            fields.get(1).and_then(|c| number(c)),
            fields.get(2).and_then(|c| number(c)),
        ) else {
            return true;
        };
        if fields.len() != 3 {
            return true;
        }
        let g = u64::from(gid);
        if g >= start && g < start.saturating_add(count) {
            return true;
        }
    }
    false
}

/// `kernel.yama.ptrace_scope` and `fs.suid_dumpable`. With `ptrace_scope` 0, any
/// process of the user attaches to another; with `suid_dumpable` 1, the setgid process
/// stays dumpable. In either case, the daemon gives neither the app nor the extension
/// role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KernelPolicy {
    ptrace_scope: Option<u32>,
    suid_dumpable: Option<u32>,
}

impl KernelPolicy {
    /// Reads both from `/proc/sys`. Unreadable (no Yama, for instance) stays `None`,
    /// which refuses.
    pub fn read() -> KernelPolicy {
        let read = |p: &str| {
            std::fs::read_to_string(p)
                .ok()
                .and_then(|t| t.trim().parse::<u32>().ok())
        };
        KernelPolicy {
            ptrace_scope: read("/proc/sys/kernel/yama/ptrace_scope"),
            suid_dumpable: read("/proc/sys/fs/suid_dumpable"),
        }
    }

    /// Given values, to simulate in the tests a kernel with another policy.
    #[cfg(any(test, feature = "test-mode"))]
    pub fn simulated(ptrace_scope: Option<u32>, suid_dumpable: Option<u32>) -> KernelPolicy {
        KernelPolicy {
            ptrace_scope,
            suid_dumpable,
        }
    }

    /// `ptrace_scope` ≥ 1 and `suid_dumpable` 0 or 2.
    pub fn verify(&self) -> Result<(), Refusal> {
        let ptrace_ok = matches!(self.ptrace_scope, Some(n) if n >= 1);
        let dumpable_ok = matches!(self.suid_dumpable, Some(0 | 2));
        if ptrace_ok && dumpable_ok {
            Ok(())
        } else {
            Err(Refusal::KernelPolicy)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_from_etc_group() {
        let g = b"root:x:0:\nward-app:x:991:\nward-nm:x:992:\ndocker:x:984:eu\n";
        assert_eq!(group_gid(g, APP_GROUP), Ok(991));
        assert_eq!(group_gid(g, EXTENSION_GROUP), Ok(992));
    }

    #[test]
    fn attack_group_with_member_repeated_or_broken() {
        assert_eq!(
            group_gid(b"ward-app:x:991:eu\n", APP_GROUP),
            Err(InstallError::GroupWithMembers(APP_GROUP))
        );
        assert_eq!(
            group_gid(b"ward-app:x:991:\nward-app:x:5:\n", APP_GROUP),
            Err(InstallError::MissingGroup(APP_GROUP))
        );
        for bad in [
            &b"ward-app:x::\n"[..],
            b"ward-app:x:+1:\n",
            b"ward-app:x:991\n",
            b"ward-app:x:991::\n",
            b"ward-appx:x:991:\n",
            b"",
        ] {
            assert_eq!(
                group_gid(bad, APP_GROUP),
                Err(InstallError::MissingGroup(APP_GROUP)),
                "{:?}",
                String::from_utf8_lossy(bad)
            );
        }
    }

    #[test]
    fn attack_gid_inside_a_subgid_range() {
        let s = b"eu:100000:65536\noutro:165536:65536\n";
        assert!(!gid_in_subgid(s, 991));
        assert!(gid_in_subgid(s, 100000));
        assert!(gid_in_subgid(s, 165535));
        assert!(gid_in_subgid(s, 231071));
        assert!(!gid_in_subgid(s, 231072));
        // A broken line fails closed.
        assert!(gid_in_subgid(b"eu:abc:1\n", 991));
        assert!(!gid_in_subgid(b"", 991));
    }

    #[test]
    fn attack_gid_this_process_already_has() {
        let chrome = AllowedBinary::new("/opt/google/chrome/chrome").unwrap();
        let mine = sys::getegid();
        assert_eq!(
            Install::new(mine, 991_999, chrome.clone()),
            Err(InstallError::InvalidGid(APP_GROUP))
        );
        for bad in [0, UNMAPPED_GID] {
            assert_eq!(
                Install::new(991_998, bad, chrome.clone()),
                Err(InstallError::InvalidGid(EXTENSION_GROUP))
            );
        }
        assert_eq!(
            Install::new(991_998, 991_998, chrome.clone()),
            Err(InstallError::InvalidGid(EXTENSION_GROUP))
        );
        // Each supplementary group of this process also refuses.
        for g in own_gids().unwrap() {
            assert!(Install::new(g, 991_999, chrome.clone()).is_err(), "{g}");
        }
    }

    #[test]
    fn kernel_policy() {
        assert_eq!(KernelPolicy::simulated(Some(1), Some(0)).verify(), Ok(()));
        assert_eq!(KernelPolicy::simulated(Some(2), Some(2)).verify(), Ok(()));
        assert_eq!(KernelPolicy::simulated(Some(3), Some(0)).verify(), Ok(()));
        for (p, d) in [
            (Some(0), Some(0)),
            (Some(1), Some(1)),
            (None, Some(0)),
            (Some(1), None),
            (Some(1), Some(3)),
        ] {
            assert_eq!(
                KernelPolicy::simulated(p, d).verify(),
                Err(Refusal::KernelPolicy),
                "{p:?} {d:?}"
            );
        }
    }
}
