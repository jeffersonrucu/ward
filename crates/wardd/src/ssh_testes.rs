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
