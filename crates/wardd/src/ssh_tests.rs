//! Argv of `ward ssh` (S3, D13, D43, D56): what goes to `execve`, without a shell.

use super::*;

fn progs() -> SshPrograms {
    SshPrograms::from_system()
}

fn text(a: &[Vec<u8>]) -> Vec<String> {
    a.iter()
        .map(|x| String::from_utf8(x.clone()).unwrap())
        .collect()
}

fn destination<'a>(username: &'a str, host: &'a str) -> Destination<'a> {
    Destination {
        host,
        port: 2222,
        username,
    }
}

fn kh() -> PathBuf {
    PathBuf::from("/x/ward/known_hosts")
}

/// D43 (V4.35, attack): the user with `@` goes whole as the argument of `-l`; the host
/// after `--`; the remote command after the host, each item one argument; never
/// `user@host` nor `-o User=`.
#[test]
fn d43_user_with_at_sign_only_after_l() {
    let cmd = [
        Zeroizing::new(b"echo".to_vec()),
        Zeroizing::new(b"-oProxyCommand=x".to_vec()),
    ];
    let a = build_argv(
        &progs(),
        &kh(),
        destination("deploy@cliente.com.br", "srv.clientex.com.br"),
        SshAuth::Password,
        Program::Ssh { command: &cmd },
    )
    .unwrap();
    let a = text(&a);
    assert_eq!(
        &a[..5],
        ["/usr/bin/sshpass", "-e", "/usr/bin/ssh", "-F", "/dev/null"]
    );
    assert_eq!(
        &a[a.len() - 8..],
        [
            "-p",
            "2222",
            "-l",
            "deploy@cliente.com.br",
            "--",
            "srv.clientex.com.br",
            "echo",
            "-oProxyCommand=x"
        ]
    );
    assert_eq!(a.iter().filter(|x| *x == "--").count(), 1);
    assert!(!a.iter().any(|x| x.starts_with("User=")));
    assert!(!a.iter().any(|x| x.ends_with("@srv.clientex.com.br")));
    // Before `--`, only Ward's options (the request's command never becomes an option).
    let before = &a[..a.iter().position(|x| x == "--").unwrap()];
    assert!(!before.iter().any(|x| x.contains("ProxyCommand=x")));
    assert!(before.contains(&"ProxyCommand=none".to_owned()));
}

/// D56 (V4.37): `sftp` with `-o User=`, `-P` and the host after `--`; no `-l`.
#[test]
fn d56_sftp() {
    let a = build_argv(
        &progs(),
        &kh(),
        destination("sftpuser", "127.0.0.1"),
        SshAuth::Password,
        Program::Sftp,
    )
    .unwrap();
    let a = text(&a);
    assert_eq!(
        &a[..5],
        ["/usr/bin/sshpass", "-e", "/usr/bin/sftp", "-F", "/dev/null"]
    );
    assert_eq!(
        &a[a.len() - 6..],
        ["-o", "User=sftpuser", "-P", "2222", "--", "127.0.0.1"]
    );
    assert!(!a.iter().any(|x| x == "-l" || x == "-p"));
}

/// Key (D12): no `sshpass`, Ward's agent and public key, password off.
#[test]
fn key_uses_the_session_agent() {
    let sock = PathBuf::from("/run/user/1000/ward/ssh-abc/agent.sock");
    let publ = PathBuf::from("/run/user/1000/ward/ssh-abc/id.pub");
    let a = build_argv(
        &progs(),
        &kh(),
        destination("deploy", "127.0.0.1"),
        SshAuth::Key {
            agent: &sock,
            public_key: &publ,
        },
        Program::Ssh { command: &[] },
    )
    .unwrap();
    let a = text(&a);
    assert_eq!(a[0], "/usr/bin/ssh");
    for o in [
        "IdentityAgent=/run/user/1000/ward/ssh-abc/agent.sock",
        "IdentityFile=/run/user/1000/ward/ssh-abc/id.pub",
        "IdentitiesOnly=yes",
        "PasswordAuthentication=no",
        "BatchMode=yes",
        "UserKnownHostsFile=/x/ward/known_hosts",
        "GlobalKnownHostsFile=/dev/null",
    ] {
        assert!(a.iter().any(|x| x == o), "falta {o}");
    }
    assert_eq!(a.last().unwrap(), "127.0.0.1");
}

/// Attack: a user, host or path out of format does not build an argv (nothing runs).
#[test]
fn attack_values_out_of_format_refused() {
    for (u, h) in [
        ("-oProxyCommand=x", "127.0.0.1"),
        ("deploy", "-oProxyCommand=x"),
        ("deploy", "deploy@127.0.0.1"),
        ("deploy x", "127.0.0.1"),
        ("deploy", "127.0.0.1 x"),
        ("de%ploy", "127.0.0.1"),
        ("deploy=1", "127.0.0.1"),
        ("", "127.0.0.1"),
        ("deploy", ""),
        ("deploy\n", "127.0.0.1"),
    ] {
        for prog in [Program::Ssh { command: &[] }, Program::Sftp] {
            assert!(
                build_argv(&progs(), &kh(), destination(u, h), SshAuth::Password, prog).is_none(),
                "{u:?} {h:?}"
            );
        }
    }
    let bad = PathBuf::from("/x/a b/known_hosts");
    assert!(
        build_argv(
            &progs(),
            &bad,
            destination("deploy", "127.0.0.1"),
            SshAuth::Password,
            Program::Sftp
        )
        .is_none()
    );
    let port0 = Destination {
        host: "127.0.0.1",
        port: 0,
        username: "deploy",
    };
    assert!(build_argv(&progs(), &kh(), port0, SshAuth::Password, Program::Sftp).is_none());
}

/// Phase 28 (D13, D43; V4.35): the `ssh -W` of the jump/tunnel bridge. No `ProxyJump`
/// (which OpenSSH runs through `sh -c`), the jump's user with `@` whole after `-l`,
/// target in `-W` (IPv6 between brackets), the jump's host after `--`.
#[test]
fn phase28_bridge_argv_without_proxyjump() {
    let a = text(
        &build_argv(
            &progs(),
            &kh(),
            destination("salto@cliente.com.br", "127.0.0.1"),
            SshAuth::Password,
            Program::Forward {
                host: "ssh-interno",
                port: 22,
            },
        )
        .unwrap(),
    );
    assert_eq!(&a[..4], ["/usr/bin/sshpass", "-e", "/usr/bin/ssh", "-F"]);
    assert_eq!(
        &a[a.len() - 8..],
        [
            "-W",
            "ssh-interno:22",
            "-p",
            "2222",
            "-l",
            "salto@cliente.com.br",
            "--",
            "127.0.0.1"
        ]
    );
    assert!(
        !a.iter()
            .any(|x| x.contains("ProxyJump") || x.starts_with("User="))
    );
    let a = text(
        &build_argv(
            &progs(),
            &kh(),
            destination("deploy", "127.0.0.1"),
            SshAuth::Password,
            Program::Forward {
                host: "fd00::5",
                port: 5432,
            },
        )
        .unwrap(),
    );
    assert!(a.contains(&"[fd00::5]:5432".to_owned()), "{a:?}");
}

/// Phase 28 (attack): a bridge target or alias out of format does not build an argv.
#[test]
fn phase28_attack_target_and_alias_out_of_format() {
    for (h, p) in [
        ("-oProxyCommand=x", 22),
        ("a b", 22),
        ("[::1]", 22),
        ("x%h", 22),
        ("", 22),
        ("db", 0),
    ] {
        assert!(
            build_argv(
                &progs(),
                &kh(),
                destination("deploy", "127.0.0.1"),
                SshAuth::Password,
                Program::Forward { host: h, port: p },
            )
            .is_none(),
            "{h:?}:{p}"
        );
        assert!(destination_alias(h, p).is_none(), "{h:?}:{p}");
    }
    assert_eq!(destination_alias("ssh-interno", 22).unwrap(), "ssh-interno");
    assert_eq!(
        destination_alias("127.0.0.1", 2222).unwrap(),
        "[127.0.0.1]:2222"
    );
    for al in [
        "a b", "[x]:0", "[x]:022", "[x]", "-x", "[x]:22 y", "[[x]]:22",
    ] {
        assert!(
            build_argv_with_alias(
                &progs(),
                &kh(),
                destination("deploy", "127.0.0.1"),
                SshAuth::Password,
                Program::Ssh { command: &[] },
                Some(al),
            )
            .is_none(),
            "{al:?}"
        );
    }
    let a = text(
        &build_argv_with_alias(
            &progs(),
            &kh(),
            destination("deploy", "127.0.0.1"),
            SshAuth::Password,
            Program::Ssh { command: &[] },
            Some("[127.0.0.1]:2222"),
        )
        .unwrap(),
    );
    assert!(a.contains(&"HostKeyAlias=[127.0.0.1]:2222".to_owned()));
    assert!(a.contains(&"CheckHostIP=no".to_owned()));
}
