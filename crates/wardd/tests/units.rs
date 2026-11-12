//! V3.21 (auditor's reading) and D57/D62: the system units that `make install` installs
//! (Phase 51), `install/systemd/wardd@.socket` and `wardd@.service`. The syntax is
//! checked by `systemd-analyze verify` in `make test-units` and `make test-install`;
//! the required and the forbidden directives are here.
//!
//! Attack covered: a directive that turns on `no_new_privs` (directly or implicitly) in
//! a service with `User=` would make the `execve` of `ward-app` lose the setgid, and the
//! app would never get the role (D57); `Environment=LD_*` would inject code into the
//! daemon.

use std::path::PathBuf;

fn unit(name: &str) -> Vec<(String, String, String)> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../install/systemd")
        .join(name);
    let text = std::fs::read_to_string(&p).unwrap();
    let mut section = String::new();
    let mut v = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with(';') {
            continue;
        }
        assert!(
            !l.ends_with('\\'),
            "{name}: continuação de linha não é aceita"
        );
        if l.starts_with('[') {
            section = l.to_owned();
            continue;
        }
        let (k, val) = l.split_once('=').expect("chave=valor");
        v.push((section.clone(), k.trim().to_owned(), val.trim().to_owned()));
    }
    v
}

fn values(u: &[(String, String, String)], section: &str, key: &str) -> Vec<String> {
    u.iter()
        .filter(|(s, k, _)| s == section && k == key)
        .map(|(_, _, v)| v.clone())
        .collect()
}

#[test]
fn v3_21_system_socket() {
    let u = unit("wardd@.socket");
    let s = "[Socket]";
    assert_eq!(values(&u, s, "ListenStream"), ["/run/ward/%i/wardd.sock"]);
    assert_eq!(values(&u, s, "SocketUser"), ["%i"]);
    assert_eq!(values(&u, s, "SocketMode"), ["0600"]);
    // D46: the directories created by systemd stay root:root 0755.
    assert_eq!(values(&u, s, "DirectoryMode"), ["0755"]);
    assert_eq!(values(&u, s, "Accept"), ["no"]);
    // A single socket (wardd requires LISTEN_FDS=1).
    let listens = u
        .iter()
        .filter(|(sec, k, _)| sec == s && k.starts_with("Listen"))
        .count();
    assert_eq!(listens, 1);
}

#[test]
fn v3_21_service_with_user_limitcore_and_no_ld() {
    let u = unit("wardd@.service");
    let s = "[Service]";
    assert_eq!(values(&u, s, "User"), ["%i"]);
    assert_eq!(values(&u, s, "LimitCORE"), ["0"]);
    assert_eq!(values(&u, s, "LimitMEMLOCK"), ["infinity"], "D62");
    assert_eq!(values(&u, s, "ExecStart"), ["/usr/local/bin/wardd"]);
    for (_, k, v) in &u {
        assert!(
            !(k == "Environment" || k == "EnvironmentFile" || k == "PassEnvironment"),
            "nenhum ambiente vindo da unit: {k}={v}"
        );
        assert!(!v.contains("LD_"), "{k}={v}");
    }
}

#[test]
fn d57_attack_no_directive_that_turns_on_no_new_privs() {
    // systemd.exec(5): NoNewPrivileges= and the options that imply it in a service with
    // User=.
    const FORBIDDEN: &[&str] = &[
        "NoNewPrivileges",
        "RestrictSUIDSGID",
        "SystemCallFilter",
        "SystemCallArchitectures",
        "SystemCallLog",
        "RestrictNamespaces",
        "RestrictAddressFamilies",
        "RestrictFileSystems",
        "RestrictRealtime",
        "LockPersonality",
        "MemoryDenyWriteExecute",
        "PrivateDevices",
        "ProtectKernelTunables",
        "ProtectKernelModules",
        "ProtectKernelLogs",
        "ProtectClock",
        "ProtectHostname",
        "DynamicUser",
        "SecureBits",
        "AmbientCapabilities",
        "CapabilityBoundingSet",
    ];
    for name in ["wardd@.service", "wardd@.socket"] {
        for (_, k, v) in unit(name) {
            assert!(!FORBIDDEN.contains(&k.as_str()), "{name}: {k}={v} (D57)");
        }
    }
}
