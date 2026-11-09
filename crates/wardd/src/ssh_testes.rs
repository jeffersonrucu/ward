//! Argv do `ward ssh` (S3, D13, D43, D56): o que vai ao `execve`, sem shell.

use super::*;

fn progs() -> ProgramasSsh {
    ProgramasSsh::do_sistema()
}

fn texto(a: &[Vec<u8>]) -> Vec<String> {
    a.iter()
        .map(|x| String::from_utf8(x.clone()).unwrap())
        .collect()
}

fn destino<'a>(usuario: &'a str, host: &'a str) -> Destino<'a> {
    Destino {
        host,
        porta: 2222,
        usuario,
    }
}

fn kh() -> PathBuf {
    PathBuf::from("/x/ward/known_hosts")
}

/// D43 (V4.35, ataque): o usuário com `@` vai inteiro como argumento de `-l`; o host
/// depois de `--`; o comando remoto depois do host, cada item um argumento; nunca
/// `usuário@host` nem `-o User=`.
#[test]
fn d43_usuario_com_arroba_so_depois_de_l() {
    let cmd = [
        Zeroizing::new(b"echo".to_vec()),
        Zeroizing::new(b"-oProxyCommand=x".to_vec()),
    ];
    let a = montar_argv(
        &progs(),
        &kh(),
        destino("deploy@cliente.com.br", "srv.clientex.com.br"),
        Autent::Senha,
        Programa::Ssh { comando: &cmd },
    )
    .unwrap();
    let a = texto(&a);
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
    // Antes do `--`, só opções do Ward (o comando do pedido nunca vira opção).
    let antes = &a[..a.iter().position(|x| x == "--").unwrap()];
    assert!(!antes.iter().any(|x| x.contains("ProxyCommand=x")));
    assert!(antes.contains(&"ProxyCommand=none".to_owned()));
}

/// D56 (V4.37): `sftp` com `-o User=`, `-P` e o host depois de `--`; sem `-l`.
#[test]
fn d56_sftp() {
    let a = montar_argv(
        &progs(),
        &kh(),
        destino("sftpuser", "127.0.0.1"),
        Autent::Senha,
        Programa::Sftp,
    )
    .unwrap();
    let a = texto(&a);
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

/// Chave (D12): sem `sshpass`, o agente e a pública do Ward, senha desligada.
#[test]
fn chave_usa_o_agente_da_sessao() {
    let sock = PathBuf::from("/run/user/1000/ward/ssh-abc/agent.sock");
    let publ = PathBuf::from("/run/user/1000/ward/ssh-abc/id.pub");
    let a = montar_argv(
        &progs(),
        &kh(),
        destino("deploy", "127.0.0.1"),
        Autent::Chave {
            agente: &sock,
            publica: &publ,
        },
        Programa::Ssh { comando: &[] },
    )
    .unwrap();
    let a = texto(&a);
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

/// Ataque: usuário, host ou caminho fora do formato não monta argv (nada executa).
#[test]
fn ataque_valores_fora_do_formato_recusados() {
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
        for prog in [Programa::Ssh { comando: &[] }, Programa::Sftp] {
            assert!(
                montar_argv(&progs(), &kh(), destino(u, h), Autent::Senha, prog).is_none(),
                "{u:?} {h:?}"
            );
        }
    }
    let ruim = PathBuf::from("/x/a b/known_hosts");
    assert!(
        montar_argv(
            &progs(),
            &ruim,
            destino("deploy", "127.0.0.1"),
            Autent::Senha,
            Programa::Sftp
        )
        .is_none()
    );
    let porta0 = Destino {
        host: "127.0.0.1",
        porta: 0,
        usuario: "deploy",
    };
    assert!(montar_argv(&progs(), &kh(), porta0, Autent::Senha, Programa::Sftp).is_none());
}

/// Fase 28 (D13, D43; V4.35): o `ssh -W` da ponte do salto/túnel. Sem `ProxyJump`
/// (que o OpenSSH roda por `sh -c`), usuário do salto com `@` inteiro depois de `-l`,
/// alvo em `-W` (IPv6 entre colchetes), host do salto depois de `--`.
#[test]
fn fase28_argv_da_ponte_sem_proxyjump() {
    let a = texto(
        &montar_argv(
            &progs(),
            &kh(),
            destino("salto@cliente.com.br", "127.0.0.1"),
            Autent::Senha,
            Programa::Encaminhar {
                host: "ssh-interno",
                porta: 22,
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
    let a = texto(
        &montar_argv(
            &progs(),
            &kh(),
            destino("deploy", "127.0.0.1"),
            Autent::Senha,
            Programa::Encaminhar {
                host: "fd00::5",
                porta: 5432,
            },
        )
        .unwrap(),
    );
    assert!(a.contains(&"[fd00::5]:5432".to_owned()), "{a:?}");
}

/// Fase 28 (ataque): alvo da ponte ou alias fora do formato não monta argv.
#[test]
fn fase28_ataque_alvo_e_alias_fora_do_formato() {
    for (h, p) in [
        ("-oProxyCommand=x", 22),
        ("a b", 22),
        ("[::1]", 22),
        ("x%h", 22),
        ("", 22),
        ("db", 0),
    ] {
        assert!(
            montar_argv(
                &progs(),
                &kh(),
                destino("deploy", "127.0.0.1"),
                Autent::Senha,
                Programa::Encaminhar { host: h, porta: p },
            )
            .is_none(),
            "{h:?}:{p}"
        );
        assert!(alias_do_destino(h, p).is_none(), "{h:?}:{p}");
    }
    assert_eq!(alias_do_destino("ssh-interno", 22).unwrap(), "ssh-interno");
    assert_eq!(
        alias_do_destino("127.0.0.1", 2222).unwrap(),
        "[127.0.0.1]:2222"
    );
    for al in [
        "a b", "[x]:0", "[x]:022", "[x]", "-x", "[x]:22 y", "[[x]]:22",
    ] {
        assert!(
            montar_argv_com_alias(
                &progs(),
                &kh(),
                destino("deploy", "127.0.0.1"),
                Autent::Senha,
                Programa::Ssh { comando: &[] },
                Some(al),
            )
            .is_none(),
            "{al:?}"
        );
    }
    let a = texto(
        &montar_argv_com_alias(
            &progs(),
            &kh(),
            destino("deploy", "127.0.0.1"),
            Autent::Senha,
            Programa::Ssh { comando: &[] },
            Some("[127.0.0.1]:2222"),
        )
        .unwrap(),
    );
    assert!(a.contains(&"HostKeyAlias=[127.0.0.1]:2222".to_owned()));
    assert!(a.contains(&"CheckHostIP=no".to_owned()));
}
