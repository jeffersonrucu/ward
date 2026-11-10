//! V3.21 (leitura do auditor) e D57/D62: as units do sistema que o `make install` instala
//! (Fase 51), `install/systemd/wardd@.socket` e `wardd@.service`. A sintaxe é
//! conferida pelo `systemd-analyze verify` em `make test-units` e `make test-install`;
//! aqui ficam as diretivas exigidas e as proibidas.
//!
//! Ataque coberto: uma diretiva que liga `no_new_privs` (direta ou implícita) num serviço
//! com `User=` faria o `execve` do `ward-app` perder o setgid, e o app nunca receberia o
//! papel (D57); `Environment=LD_*` injetaria código no daemon.

use std::path::PathBuf;

fn unit(nome: &str) -> Vec<(String, String, String)> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../install/systemd")
        .join(nome);
    let texto = std::fs::read_to_string(&p).unwrap();
    let mut secao = String::new();
    let mut v = Vec::new();
    for linha in texto.lines() {
        let l = linha.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with(';') {
            continue;
        }
        assert!(
            !l.ends_with('\\'),
            "{nome}: continuação de linha não é aceita"
        );
        if l.starts_with('[') {
            secao = l.to_owned();
            continue;
        }
        let (k, val) = l.split_once('=').expect("chave=valor");
        v.push((secao.clone(), k.trim().to_owned(), val.trim().to_owned()));
    }
    v
}

fn valores(u: &[(String, String, String)], secao: &str, chave: &str) -> Vec<String> {
    u.iter()
        .filter(|(s, k, _)| s == secao && k == chave)
        .map(|(_, _, v)| v.clone())
        .collect()
}

#[test]
fn v3_21_socket_do_sistema() {
    let u = unit("wardd@.socket");
    let s = "[Socket]";
    assert_eq!(valores(&u, s, "ListenStream"), ["/run/ward/%i/wardd.sock"]);
    assert_eq!(valores(&u, s, "SocketUser"), ["%i"]);
    assert_eq!(valores(&u, s, "SocketMode"), ["0600"]);
    // D46: as pastas criadas pelo systemd ficam root:root 0755.
    assert_eq!(valores(&u, s, "DirectoryMode"), ["0755"]);
    assert_eq!(valores(&u, s, "Accept"), ["no"]);
    // Um socket só (o wardd exige LISTEN_FDS=1).
    let escutas = u
        .iter()
        .filter(|(sec, k, _)| sec == s && k.starts_with("Listen"))
        .count();
    assert_eq!(escutas, 1);
}

#[test]
fn v3_21_servico_com_user_limitcore_e_sem_ld() {
    let u = unit("wardd@.service");
    let s = "[Service]";
    assert_eq!(valores(&u, s, "User"), ["%i"]);
    assert_eq!(valores(&u, s, "LimitCORE"), ["0"]);
    assert_eq!(valores(&u, s, "LimitMEMLOCK"), ["infinity"], "D62");
    assert_eq!(valores(&u, s, "ExecStart"), ["/usr/local/bin/wardd"]);
    for (_, k, v) in &u {
        assert!(
            !(k == "Environment" || k == "EnvironmentFile" || k == "PassEnvironment"),
            "nenhum ambiente vindo da unit: {k}={v}"
        );
        assert!(!v.contains("LD_"), "{k}={v}");
    }
}

#[test]
fn d57_ataque_nenhuma_diretiva_que_liga_no_new_privs() {
    // systemd.exec(5): NoNewPrivileges= e as opções que o implicam num serviço com User=.
    const PROIBIDAS: &[&str] = &[
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
    for nome in ["wardd@.service", "wardd@.socket"] {
        for (_, k, v) in unit(nome) {
            assert!(!PROIBIDAS.contains(&k.as_str()), "{nome}: {k}={v} (D57)");
        }
    }
}
