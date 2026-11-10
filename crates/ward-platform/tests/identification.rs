//! Identification of the process on the other side of the socket (D5, D47; V3.5, V3.24;
//! S1, S5).
//!
//! The "installed binary" of the tests is the system python (`/usr/bin/python3.12`,
//! root's, with no write for group and others): Ward's test binary stays in `target/`
//! and is the user's, so it never passes — which is exactly the attack case.
//!
//! Role by the group (D47) on the host, without root: the "app group" of the tests is
//! the user's own egid (`Install::for_test`, only in `test-mode`) and the "daemon" is
//! this test process, the client's parent. The real setgid is left to the container
//! (`make test-docker-platform`).
//!
//! Attacks: binary copied out of the install, `LD_PRELOAD`/`LD_AUDIT`/
//! `LD_LIBRARY_PATH`, `strace` (TracerPid ≠ 0), user library loaded without a variable,
//! process that exits before the check, parent that is not Chrome, app parent that is
//! not the daemon, common group, user namespace, simulated `ptrace_scope` 0.

#![cfg(feature = "test-mode")]

mod common;

use std::path::{Path, PathBuf};

use common::{TestChild, TestDir, allowed, clean_command, python_client, real_python, start};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use ward_platform::socket::{Listener, prepare_dir};
use ward_platform::{AllowedBinary, Install, KernelPolicy, PeerClass, PeerProcess, Refusal};
use ward_platform::{ExpectedServer, ServerRefusal, connect_server, verify_server};

fn listen() -> (TestDir, Listener) {
    let d = TestDir::new();
    let listener = prepare_dir(&d.path).unwrap().listen("wardd.sock").unwrap();
    (d, listener)
}

/// Starts the client, waits for it to connect and returns the accepted connection.
fn connect(listener: &Listener, cmd: std::process::Command) -> (TestChild, PeerProcess) {
    let mut child = start(cmd);
    child.wait_for("conectado");
    let connection = listener.accept().unwrap();
    assert_eq!(connection.peer.credentials().pid, child.pid());
    (child, connection.peer)
}

fn missing() -> AllowedBinary {
    AllowedBinary::new("/usr/local/bin/ward-falso-nao-instalado").unwrap()
}

/// gid that no process of the test has.
const NO_GID: u32 = 999_123;

fn policy_ok() -> KernelPolicy {
    KernelPolicy::simulated(Some(1), Some(2))
}

/// "App group" = `gid`; the host's, a gid nobody has.
fn app_install(gid: u32) -> Install {
    Install::for_test(gid, NO_GID, missing())
}

/// "Host group" = `gid`, with the "Chrome" `chrome`.
fn host_install(gid: u32, chrome: AllowedBinary) -> Install {
    Install::for_test(NO_GID, gid, chrome)
}

/// Python client with bash as the parent (the `; true` prevents the direct exec of
/// python).
fn client_with_bash_parent(socket: &Path) -> std::process::Command {
    let script = format!(
        "{} -I -c \"$CLIENTE\" \"$SOCKET\"; true",
        real_python().display()
    );
    let mut cmd = clean_command(Path::new("/usr/bin/bash"));
    cmd.arg("-c")
        .arg(script)
        .env("CLIENTE", common::CLIENT_PY)
        .env("SOCKET", socket);
    cmd
}

#[test]
fn installed_binary_is_identified() {
    let (_d, listener) = listen();
    let py = real_python();
    let (child, peer) = connect(&listener, python_client(&py, listener.path(), None));
    assert_eq!(peer.identify(&allowed(&py)), Ok(()));
    assert_eq!(peer.reverify_no_debugger(), Ok(()));
    // D47: the client's egid is the "app group" and the parent is this process (the
    // "daemon").
    let egid = peer.credentials().gid;
    assert_eq!(
        peer.classify(&app_install(egid), &policy_ok()),
        PeerClass::App
    );
    // Another gid in the app group: common (agent), even with the installed exe.
    assert_eq!(
        peer.classify(&app_install(egid.wrapping_add(7)), &policy_ok()),
        PeerClass::Common {
            reason: Refusal::CommonGroup
        }
    );
    // Chain for display: ends at the client, and the test is before it.
    let chain = peer.chain();
    assert_eq!(chain.last().unwrap().pid, child.pid());
    assert!(chain.iter().any(|e| e.pid == std::process::id()));
    child.finish();
}

#[test]
fn attack_binary_copied_out_of_the_install() {
    // V3.5: the same binary, copied to a directory of the user.
    let copy = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ward-app-copiado");
    common::copy_binary(&real_python(), &copy);
    let (_d, listener) = listen();
    let (child, peer) = connect(&listener, python_client(&copy, listener.path(), None));
    assert_eq!(
        peer.identify(&allowed(&real_python())),
        Err(Refusal::ExeNotAllowed)
    );
    // Not even putting the copy as "allowed": it is not root's.
    assert_eq!(
        peer.identify(&allowed(&copy)),
        Err(Refusal::UntrustedInstall)
    );
    child.finish();
    let _ = std::fs::remove_file(&copy);
}

#[test]
fn attack_ld_variables_in_the_environment() {
    let py = real_python();
    // Empty or missing values: the loader ignores them, but the name already refuses.
    for (name, value) in [
        ("LD_PRELOAD", ""),
        ("LD_AUDIT", ""),
        ("LD_LIBRARY_PATH", "/nao-existe-ward"),
    ] {
        let (_d, listener) = listen();
        let mut cmd = python_client(&py, listener.path(), None);
        cmd.env(name, value);
        let (child, peer) = connect(&listener, cmd);
        assert_eq!(
            peer.identify(&allowed(&py)),
            Err(Refusal::LdVariable(match name {
                "LD_PRELOAD" => "LD_PRELOAD",
                "LD_AUDIT" => "LD_AUDIT",
                _ => "LD_LIBRARY_PATH",
            })),
            "{name}"
        );
        child.finish();
    }
}

#[test]
fn attack_user_library_without_variable() {
    // Like an `LD_PRELOAD` that deleted its own variable: the library shows up in
    // `maps`, outside root's.
    let lib = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("libfalsa-ward.so");
    std::fs::copy("/usr/lib/x86_64-linux-gnu/libz.so.1", &lib).unwrap();
    let (_d, listener) = listen();
    let py = real_python();
    let (child, peer) = connect(&listener, python_client(&py, listener.path(), Some(&lib)));
    assert_eq!(peer.identify(&allowed(&py)), Err(Refusal::UntrustedLibrary));
    child.finish();
    let _ = std::fs::remove_file(&lib);
}

#[test]
fn attack_client_under_strace() {
    let (_d, listener) = listen();
    let py = real_python();
    let mut cmd = clean_command(Path::new("/usr/bin/strace"));
    cmd.args(["-f", "-o", "/dev/null"])
        .arg(&py)
        .arg("-I")
        .arg("-c")
        .arg(common::CLIENT_PY)
        .arg(listener.path());
    let mut child = start(cmd);
    child.wait_for("conectado");
    let peer = listener.accept().unwrap().peer;
    assert_eq!(peer.identify(&allowed(&py)), Err(Refusal::Debugger));
    assert_eq!(peer.reverify_no_debugger(), Err(Refusal::Debugger));
    // D47: with the app group, but under a debugger, it stays an agent.
    assert_eq!(
        peer.classify(&app_install(peer.credentials().gid), &policy_ok()),
        PeerClass::Common {
            reason: Refusal::Debugger
        }
    );
    child.finish();
}

#[test]
fn attack_process_that_exits_before_the_check() {
    let (_d, listener) = listen();
    let py = real_python();
    let (mut child, peer) = connect(&listener, python_client(&py, listener.path(), None));
    drop(child.child.stdin.take());
    child.child.wait().unwrap();
    // The pid may even be reused; the pidfd may not.
    assert_eq!(peer.identify(&allowed(&py)), Err(Refusal::ProcessVanished));
    assert_eq!(peer.reverify_no_debugger(), Err(Refusal::ProcessVanished));
}

#[test]
fn extension_host_requires_the_allowed_parent() {
    let (_d, listener) = listen();
    let mut child = start(client_with_bash_parent(listener.path()));
    child.wait_for("conectado");
    let peer = listener.accept().unwrap().peer;
    let bash = allowed(Path::new("/usr/bin/bash"));
    let egid = peer.credentials().gid;
    assert_eq!(peer.verify_parent(&bash), Ok(()));
    assert_eq!(
        peer.classify(&host_install(egid, bash.clone()), &policy_ok()),
        PeerClass::ExtensionHost
    );
    // A parent that is not the allowed "Chrome".
    let sleep = allowed(Path::new("/usr/bin/sleep"));
    assert_eq!(peer.verify_parent(&sleep), Err(Refusal::ParentNotAllowed));
    assert_eq!(
        peer.classify(&host_install(egid, sleep), &policy_ok()),
        PeerClass::Common {
            reason: Refusal::ParentNotAllowed
        }
    );
    // With the app group and bash as the parent: only the daemon is an accepted parent
    // (D57).
    assert_eq!(
        peer.classify(&app_install(egid), &policy_ok()),
        PeerClass::Common {
            reason: Refusal::AppParentNotAllowed
        }
    );
    child.finish();
}

#[test]
fn extension_host_without_allowed_parent_when_the_parent_is_the_test() {
    // The client's parent is the test binary (in target/, the user's).
    let (_d, listener) = listen();
    let py = real_python();
    let (child, peer) = connect(&listener, python_client(&py, listener.path(), None));
    let me = allowed(&std::env::current_exe().unwrap());
    assert_eq!(peer.verify_parent(&me), Err(Refusal::ParentNotAllowed));
    assert_eq!(
        peer.classify(&host_install(peer.credentials().gid, me), &policy_ok()),
        PeerClass::Common {
            reason: Refusal::ParentNotAllowed
        }
    );
    child.finish();
}

#[test]
fn attack_ptrace_scope_0_or_suid_dumpable_1_turns_the_roles_off() {
    // V3.24: with a weak kernel policy, not even the right group gives a role.
    let (_d, listener) = listen();
    let py = real_python();
    let (child, peer) = connect(&listener, python_client(&py, listener.path(), None));
    let egid = peer.credentials().gid;
    for policy in [
        KernelPolicy::simulated(Some(0), Some(0)),
        KernelPolicy::simulated(Some(1), Some(1)),
        KernelPolicy::simulated(None, Some(0)),
    ] {
        for install in [
            app_install(egid),
            host_install(egid, allowed(Path::new("/usr/bin/bash"))),
        ] {
            assert_eq!(
                peer.classify(&install, &policy),
                PeerClass::Common {
                    reason: Refusal::KernelPolicy
                },
                "{policy:?}"
            );
        }
    }
    // The policy of this kernel (Pop!_OS: ptrace_scope 1, suid_dumpable 2) passes.
    assert_eq!(KernelPolicy::read().verify(), Ok(()));
    child.finish();
}

#[test]
fn attack_user_namespace_does_not_forge_the_group() {
    // V3.24: in a user namespace the client is "root" with gid 0, but the daemon sees
    // the real gid (or 65534), never a system gid it does not have.
    let (_d, listener) = listen();
    let py = real_python();
    let mut cmd = clean_command(Path::new("/usr/bin/unshare"));
    cmd.args(["--user", "--map-root-user", "--"])
        .arg(&py)
        .arg("-I")
        .arg("-c")
        .arg(common::CLIENT_PY)
        .arg(listener.path());
    let (child, peer) = connect(&listener, cmd);
    let my_gid = std::fs::metadata("/proc/self").unwrap().gid();
    assert_eq!(peer.credentials().gid, my_gid);
    assert_eq!(peer.credentials().uid, ward_platform::effective_uid());
    assert_eq!(
        peer.classify(&app_install(NO_GID - 1), &policy_ok()),
        PeerClass::Common {
            reason: Refusal::CommonGroup
        }
    );
    assert!(matches!(
        peer.classify(&host_install(0, missing()), &policy_ok()),
        PeerClass::Common { .. }
    ));
    child.finish();
}

/// Python server that listens on `socket`; with `not_dumpable`, does
/// `prctl(PR_SET_DUMPABLE, 0)` first, like the hardened `wardd`.
const SERVER_PY: &str = r#"
import socket, sys, ctypes
s = socket.socket(socket.AF_UNIX)
s.bind(sys.argv[1]); s.listen(1)
if sys.argv[2] == "1":
    ctypes.CDLL(None).prctl(4, 0, 0, 0, 0)
sys.stdout.write("conectado\n"); sys.stdout.flush()
c, _ = s.accept()
sys.stdin.read()
"#;

fn server(socket: &Path, not_dumpable: bool) -> TestChild {
    let mut cmd = clean_command(&real_python());
    cmd.arg("-I")
        .arg("-c")
        .arg(SERVER_PY)
        .arg(socket)
        .arg(if not_dumpable { "1" } else { "0" });
    let mut f = start(cmd);
    f.wait_for("conectado");
    f
}

#[test]
fn attack_fake_server_of_the_same_uid_is_refused_before_any_byte() {
    // V3.23: server of the same uid, dumpable or not, in the user's directory. By the
    // system rule (D46), the directory is not root's and the peer is not uid 0: refuses
    // without writing.
    for not_dumpable in [false, true] {
        let d = TestDir::new();
        let dir = prepare_dir(&d.path).unwrap();
        let socket = dir.path().join("wardd.sock");
        let srv = server(&socket, not_dumpable);
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).unwrap();
        let expected = ExpectedServer::system_at(socket.clone());
        assert_eq!(
            connect_server(&expected).err(),
            Some(ServerRefusal::UntrustedDir)
        );
        // Even connecting from outside, the check of the connected socket refuses.
        let s = std::os::unix::net::UnixStream::connect(&socket).unwrap();
        assert_eq!(
            verify_server(&s, &expected),
            Err(ServerRefusal::UntrustedDir)
        );
        // The test form (D7) accepts the same server: it is the test daemon.
        assert_eq!(
            verify_server(&s, &ExpectedServer::test_at(socket.clone())),
            Ok(())
        );
        drop(s);
        srv.finish();
    }
}

#[test]
fn attack_test_server_with_open_socket_or_swapped_address() {
    let d = TestDir::new();
    let dir = prepare_dir(&d.path).unwrap();
    let socket = dir.path().join("wardd.sock");
    let srv = server(&socket, false);
    let test = ExpectedServer::test_at(socket.clone());
    // Python creates the socket with the umask (0755 or 0775): it is not 0600.
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o666)).unwrap();
    assert_eq!(
        connect_server(&test).err(),
        Some(ServerRefusal::UntrustedSocket)
    );
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).unwrap();
    // Expected at another path, with a link to the real socket: the peer does not listen
    // on the expected path (and the link is not a socket).
    let link = dir.path().join("outro.sock");
    std::os::unix::fs::symlink(&socket, &link).unwrap();
    assert_eq!(
        connect_server(&ExpectedServer::test_at(link.clone())).err(),
        Some(ServerRefusal::UntrustedSocket)
    );
    let s = std::os::unix::net::UnixStream::connect(&link).unwrap();
    assert_eq!(
        verify_server(&s, &ExpectedServer::test_at(dir.path().join("x.sock"))),
        Err(ServerRefusal::Missing)
    );
    drop(s);
    let s = connect_server(&test).unwrap();
    drop(s);
    srv.finish();
}

/// D63 (test-mode): the test app gets the app class **without** the egid of the group,
/// but only with the parent = this process (the "daemon"); with bash as the parent, it
/// stays common. The pid of the launcher is verified by `wardd` (Phase 23a).
#[test]
fn test_app_without_egid_only_with_the_daemon_parent() {
    let (_d, listener) = listen();
    let py = real_python();
    let (child, peer) = connect(&listener, python_client(&py, listener.path(), None));
    assert_eq!(peer.classify_test_app(), PeerClass::App);
    // By the group (release), the same peer without the app egid is common.
    assert_eq!(
        peer.classify(&app_install(NO_GID), &policy_ok()),
        PeerClass::Common {
            reason: Refusal::CommonGroup
        }
    );
    drop(child);

    let mut child = start(client_with_bash_parent(listener.path()));
    child.wait_for("conectado");
    let peer = listener.accept().unwrap().peer;
    assert_eq!(
        peer.classify_test_app(),
        PeerClass::Common {
            reason: Refusal::AppParentNotAllowed
        }
    );
}

/// Phase 41 (test-mode): the test host of the extension gets the extension class
/// **without** the egid of the `ward-nm` group, but only with the parent equal to the
/// allowed "Chrome" verified through `/proc` and with the kernel policy; the same peer
/// with another parent (a fake Chrome, the test itself) or with a weak policy stays
/// common.
#[test]
fn attack_test_extension_host_only_with_the_chrome_parent() {
    let (_d, listener) = listen();
    let mut child = start(client_with_bash_parent(listener.path()));
    child.wait_for("conectado");
    let peer = listener.accept().unwrap().peer;
    // The parent is bash: with "Chrome" = bash, it passes; the egid does not count.
    let bash = allowed(Path::new("/usr/bin/bash"));
    assert_eq!(
        peer.classify_test_extension_host(&bash, &policy_ok()),
        PeerClass::ExtensionHost
    );
    // By the group (release), the same peer without the host egid is common.
    assert_eq!(
        peer.classify(&host_install(NO_GID, bash.clone()), &policy_ok()),
        PeerClass::Common {
            reason: Refusal::CommonGroup
        }
    );
    // Another "Chrome" (another exe): parent not allowed.
    for other in [allowed(Path::new("/usr/bin/sleep")), missing()] {
        assert_eq!(
            peer.classify_test_extension_host(&other, &policy_ok()),
            PeerClass::Common {
                reason: Refusal::ParentNotAllowed
            }
        );
    }
    // Weak kernel policy: not even with the right parent.
    for policy in [
        KernelPolicy::simulated(Some(0), Some(0)),
        KernelPolicy::simulated(Some(1), Some(1)),
    ] {
        assert_eq!(
            peer.classify_test_extension_host(&bash, &policy),
            PeerClass::Common {
                reason: Refusal::KernelPolicy
            }
        );
    }
    child.finish();

    // The parent is this test process (like a `ward-nm` opened by hand): refuses.
    let py = real_python();
    let (child, peer) = connect(&listener, python_client(&py, listener.path(), None));
    assert_eq!(
        peer.classify_test_extension_host(&bash, &policy_ok()),
        PeerClass::Common {
            reason: Refusal::ParentNotAllowed
        }
    );
    drop(child);
}
