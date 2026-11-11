//! Tests of the entry model: V2.12, V2.13, V2.16 (validation part), V2.27 and V2.28,
//! plus the attacks on validation (option injection into `ssh`, invisible text,
//! dangerous variable, sizes, level on edit). Fake values of the test vault only (D15).

use super::*;

// ---- Helpers ----

fn s(v: &str) -> Option<Secret> {
    Some(Secret::from_text(v.to_owned()))
}

fn t(v: &str) -> String {
    v.to_owned()
}

/// Fake private key, built at run time (the whole text is not in the repository, so it
/// does not look like a real key to a scanner).
fn fake_key() -> String {
    let label = ["OPENSSH", "PRIVATE KEY"].join(" ");
    format!("-----BEGIN {label}-----\nfalso-chave-ed25519-teste\n-----END {label}-----\n")
}

/// Function that builds the data of a type.
type MakeData = fn() -> DataDraft;

fn draft(name: &str, group: &str, environment: &str, data: DataDraft) -> Draft {
    Draft {
        name: t(name),
        group: t(group),
        environment: t(environment),
        project: String::new(),
        tags: Vec::new(),
        data,
        extras: Vec::new(),
        attachments: Vec::new(),
    }
}

fn login() -> DataDraft {
    DataDraft::Login {
        url: t("http://localhost:8081/wp-login.php"),
        username: t("wp_admin"),
        password: s("falso-WP-senha-01"),
        totp_seed: s("JBSWY3DPEHPK3PXP"),
    }
}

fn database() -> DataDraft {
    DataDraft::Database {
        engine: t("postgres"),
        host: t("127.0.0.1"),
        port: t("55432"),
        db_name: t("app_teste"),
        username: t("app"),
        password: s("falso-PG:s3nha/01@#"),
        tunnel: String::new(),
    }
}

fn server_password() -> DataDraft {
    DataDraft::Server {
        host: t("127.0.0.1"),
        port: t("2222"),
        username: t("deploy"),
        password: s("falso-SSH-senha-01"),
        private_key: None,
        passphrase: None,
        jump: String::new(),
    }
}

fn server_key() -> DataDraft {
    DataDraft::Server {
        host: t("127.0.0.1"),
        port: t("2223"),
        username: t("deploy"),
        password: None,
        private_key: Some(Secret::from_text(fake_key())),
        passphrase: s("falso-passphrase-01"),
        jump: String::new(),
    }
}

fn api() -> DataDraft {
    DataDraft::Api {
        url: t("https://api.github.com"),
        token: s("falso_token_github_01"),
        variable: t("GITHUB_TOKEN"),
    }
}

fn aws() -> DataDraft {
    DataDraft::Aws {
        access_key_id: t("FALSOACCESSKEY01"),
        secret_access_key: s("falso-aws-secret-01"),
        region: t("sa-east-1"),
        role_arn: t("arn:aws:iam::000000000000:role/teste"),
        session_token: s("falso-aws-session-01"),
    }
}

fn smtp() -> DataDraft {
    DataDraft::Smtp {
        host: t("smtp.teste.invalid"),
        port: t("587"),
        username: t("mailer"),
        password: s("falso-SMTP-01"),
        security: t("starttls"),
    }
}

fn sftp() -> DataDraft {
    DataDraft::FtpSftp {
        protocol: t("sftp"),
        host: t("127.0.0.1"),
        port: t("2222"),
        username: t("sftpuser"),
        password: s("falso-SFTP-01"),
        private_key: None,
        passphrase: None,
    }
}

fn note() -> DataDraft {
    DataDraft::SecureNote {
        text: s("falso-nota-01"),
    }
}

fn extra(name: &str, value: &str, secret: bool) -> ExtraDraft {
    ExtraDraft {
        name: t(name),
        value: Secret::from_text(t(value)),
        secret,
    }
}

fn attachment(name: &str, content: &[u8]) -> AttachmentDraft {
    AttachmentDraft {
        name: t(name),
        content: Secret::from_bytes(content.to_vec()),
    }
}

fn generic() -> Draft {
    let mut r = draft("gama-generico", "gama", "stage", DataDraft::Generic);
    r.extras = vec![
        extra("REGIAO", "r-01", false),
        extra("CHAVE_LICENCA", "falso-licenca-01", true),
    ];
    r.attachments = vec![attachment("app.env", b"FALSO_ENV=falso-anexo-01\n")];
    r
}

/// One valid draft of each type (server with both kinds of auth).
fn one_of_each() -> Vec<Draft> {
    vec![
        draft("acme-wp-dev", "acme", "dev", login()),
        draft("acme-pg-dev", "acme", "dev", database()),
        draft("acme-ssh-senha-stage", "acme", "stage", server_password()),
        draft("acme-ssh-chave-prod", "acme", "prod", server_key()),
        draft("beta-github-api", "beta", "dev", api()),
        draft("beta-aws-dev", "beta", "dev", aws()),
        draft("beta-smtp-stage", "beta", "stage", smtp()),
        draft("beta-sftp-dev", "beta", "dev", sftp()),
        draft("gama-nota", "gama", "dev", note()),
        generic(),
    ]
}

/// All the secret values used in [`one_of_each`].
fn test_secrets() -> Vec<String> {
    vec![
        t("falso-WP-senha-01"),
        t("JBSWY3DPEHPK3PXP"),
        t("falso-PG:s3nha/01@#"),
        t("falso-SSH-senha-01"),
        t("falso-chave-ed25519-teste"),
        t("falso-passphrase-01"),
        t("falso_token_github_01"),
        t("falso-aws-secret-01"),
        t("falso-aws-session-01"),
        t("falso-SMTP-01"),
        t("falso-SFTP-01"),
        t("falso-nota-01"),
        t("falso-licenca-01"),
        t("falso-anexo-01"),
    ]
}

fn valid(r: Draft) -> Entry {
    match r.validate() {
        Ok(e) => e,
        Err(errors) => panic!("devia validar: {errors}"),
    }
}

fn errors(r: Draft) -> ValidationErrors {
    match r.validate() {
        Ok(e) => panic!("devia recusar: {e}"),
        Err(errors) => errors,
    }
}

fn no_secret(text: &str, place: &str) {
    for secret in test_secrets() {
        assert!(!text.contains(&secret), "{place} contém segredo");
    }
}

// ---- Happy path ----

#[test]
fn one_of_each_type_validates() {
    let entries: Vec<Entry> = one_of_each().into_iter().map(valid).collect();
    let kinds: Vec<EntryType> = entries.iter().map(Entry::kind).collect();
    for kind in EntryType::ALL {
        assert!(kinds.contains(&kind), "falta {kind}");
    }
    for e in &entries {
        assert_eq!(
            e.level(),
            AccessLevel::Approval,
            "{e} não começou com aprovação"
        );
    }
}

#[test]
fn contract_test_vault_validates_and_verifies() {
    // The 20 entries of the contract's test vault table (D15), with the tunnel and jump
    // references.
    let mut rs = one_of_each();
    let mut wp_prod = draft("acme-wp-prod", "acme", "prod", login());
    if let DataDraft::Login {
        username, password, ..
    } = &mut wp_prod.data
    {
        *username = t("wp_editor");
        *password = s("falso-WP-senha-02");
    }
    rs.push(wp_prod);
    rs.push(draft("acme-pg-aprovacao", "acme", "stage", database()));
    rs.push(draft("acme-pg-prod", "acme", "prod", database()));
    let mut tunnel = draft("acme-pg-tunel", "acme", "stage", database());
    if let DataDraft::Database {
        host,
        port,
        tunnel: t_,
        ..
    } = &mut tunnel.data
    {
        *host = t("postgres");
        *port = t("5432");
        *t_ = t("acme-ssh-senha-stage");
    }
    rs.push(tunnel);
    for name in ["acme-ssh-salto", "acme-ssh-salto-livre"] {
        let mut r = draft(name, "acme", "prod", server_password());
        if let DataDraft::Server {
            port,
            username,
            password,
            ..
        } = &mut r.data
        {
            *port = t("2224");
            *username = t("salto");
            *password = s("falso-SALTO-01");
        }
        rs.push(r);
    }
    for (name, jump_from) in [
        ("acme-ssh-interno", "acme-ssh-salto-livre"),
        ("acme-ssh-interno-aprov", "acme-ssh-salto"),
    ] {
        let mut r = draft(name, "acme", "prod", server_key());
        if let DataDraft::Server {
            host, port, jump, ..
        } = &mut r.data
        {
            *host = t("ssh-interno");
            *port = t("22");
            *jump = t(jump_from);
        }
        rs.push(r);
    }
    let mut blocked = draft("gama-bloqueada", "gama", "prod", login());
    if let DataDraft::Login {
        url,
        username,
        password,
        totp_seed,
    } = &mut blocked.data
    {
        *url = t("https://bloqueada.invalid");
        *username = t("x");
        *password = s("falso-BLOQ-01");
        *totp_seed = None;
    }
    rs.push(blocked);
    let mut redir = draft("delta-redir", "delta", "dev", login());
    if let DataDraft::Login {
        url,
        username,
        password,
        totp_seed,
    } = &mut redir.data
    {
        *url = t("http://localhost:8084/login");
        *username = t("u");
        *password = s("falso-REDIR-01");
        *totp_seed = None;
    }
    rs.push(redir);

    let entries: Vec<Entry> = rs.into_iter().map(valid).collect();
    assert_eq!(entries.len(), 20);
    for (i, e) in entries.iter().enumerate() {
        let others = entries
            .iter()
            .enumerate()
            .filter(move |(j, _)| *j != i)
            .map(|(_, o)| o);
        e.verify_in_vault(others)
            .unwrap_or_else(|errors| panic!("{e}: {errors}"));
    }
}

#[test]
fn empty_port_uses_the_default() {
    let mut r = draft("x", "acme", "dev", database());
    if let DataDraft::Database {
        engine,
        port,
        db_name,
        ..
    } = &mut r.data
    {
        *engine = t("redis");
        *port = String::new();
        // In redis the database name is a number (D33).
        *db_name = t("0");
    }
    let Data::Database(b) = valid(r).data else {
        unreachable!()
    };
    assert_eq!(b.port, 6379);
    for (engine, port) in [
        ("postgres", 5432),
        ("mysql", 3306),
        ("mongo", 27017),
        ("sqlserver", 1433),
    ] {
        assert_eq!(
            Engine::from_name(engine).map(Engine::default_port),
            Some(port)
        );
    }
}

// ---- V2.12: validation refuses with a message in Portuguese ----

/// Removes a field from the draft (leaves it empty).
fn without(data: DataDraft, field: &str) -> DataDraft {
    let mut d = data;
    macro_rules! clear {
        ($($f:ident),*) => {{
            $( if field == stringify!($f) { *$f = Default::default(); } )*
        }};
    }
    match &mut d {
        DataDraft::Login {
            url,
            username,
            password,
            totp_seed,
        } => clear!(url, username, password, totp_seed),
        DataDraft::Database {
            engine,
            host,
            port,
            db_name,
            username,
            password,
            tunnel,
        } => clear!(engine, host, port, db_name, username, password, tunnel),
        DataDraft::Server {
            host,
            port,
            username,
            password,
            private_key,
            passphrase,
            jump,
        } => clear!(
            host,
            port,
            username,
            password,
            private_key,
            passphrase,
            jump
        ),
        DataDraft::Api {
            url,
            token,
            variable,
        } => clear!(url, token, variable),
        DataDraft::Aws {
            access_key_id,
            secret_access_key,
            region,
            role_arn,
            session_token,
        } => clear!(
            access_key_id,
            secret_access_key,
            region,
            role_arn,
            session_token
        ),
        DataDraft::Smtp {
            host,
            port,
            username,
            password,
            security,
        } => clear!(host, port, username, password, security),
        DataDraft::FtpSftp {
            protocol,
            host,
            port,
            username,
            password,
            private_key,
            passphrase,
        } => clear!(
            protocol,
            host,
            port,
            username,
            password,
            private_key,
            passphrase
        ),
        DataDraft::SecureNote { text } => clear!(text),
        DataDraft::Generic => {}
    }
    d
}

#[test]
fn v2_12_required_field_of_each_type() {
    // (data, draft field, field in the error)
    let cases: Vec<(MakeData, &str, &str)> = vec![
        (login, "url", "url"),
        (login, "username", "usuário"),
        (login, "password", "senha"),
        (database, "engine", "motor"),
        (database, "host", "host"),
        (database, "username", "usuário"),
        (database, "password", "senha"),
        (server_password, "host", "host"),
        (server_password, "username", "usuário"),
        (server_password, "password", "autenticação"),
        (server_key, "private_key", "autenticação"),
        (api, "token", "token"),
        (aws, "access_key_id", "access key ID"),
        (aws, "secret_access_key", "secret access key"),
        (aws, "region", "região"),
        (smtp, "host", "host"),
        (smtp, "username", "usuário"),
        (smtp, "password", "senha"),
        (smtp, "security", "segurança"),
        (sftp, "protocol", "protocolo"),
        (sftp, "host", "host"),
        (sftp, "username", "usuário"),
        (sftp, "password", "autenticação"),
        (note, "text", "texto"),
    ];
    for (data, remove, field) in cases {
        let r = draft("x", "acme", "dev", without(data(), remove));
        let e = errors(r);
        assert!(
            e.has(field),
            "sem {remove}: esperava erro em {field}, veio {e}"
        );
        assert!(e.to_string().starts_with("entrada inválida: "), "{e}");
    }
    // Without the key and without a password: no auth (the passphrase alone does not
    // count).
    let r = draft(
        "x",
        "acme",
        "dev",
        without(without(server_key(), "private_key"), "passphrase"),
    );
    assert!(errors(r).has_reason("autenticação", "informe a senha ou a chave"));
    // Generic without extras.
    let mut g = generic();
    g.extras.clear();
    assert!(errors(g).has_reason(
        "campos extras",
        "o tipo genérico precisa de pelo menos um campo extra"
    ));
    // Optional ones may be missing.
    for (data, remove) in [
        (login as MakeData, "totp_seed"),
        (database, "db_name"),
        (database, "port"),
        (database, "tunnel"),
        (server_key, "passphrase"),
        (server_password, "port"),
        (api, "url"),
        (api, "variable"),
        (aws, "role_arn"),
        (aws, "session_token"),
        (smtp, "port"),
        (sftp, "port"),
    ] {
        valid(draft("x", "acme", "dev", without(data(), remove)));
    }
}

#[test]
fn v2_12_no_group_no_environment_and_environment_off_the_list() {
    for r in one_of_each() {
        let kind = r.data.kind();
        let mut no_group = r;
        no_group.group.clear();
        let e = errors(no_group);
        assert!(e.has_reason("grupo", "obrigatório"), "{kind}: {e}");
    }
    for r in one_of_each() {
        let mut no_environment = r;
        no_environment.environment.clear();
        assert!(errors(no_environment).has_reason("ambiente", "obrigatório"));
    }
    for outside in [
        "producao", "PROD", "Dev", " dev", "dev ", "staging", "qa", "prod\n",
    ] {
        let r = draft("x", "acme", outside, note());
        assert!(
            errors(r).has_reason("ambiente", "deve ser dev, stage ou prod"),
            "ambiente {outside:?} passou"
        );
    }
    for ok in ["dev", "stage", "prod"] {
        assert_eq!(
            valid(draft("x", "acme", ok, note())).environment().name(),
            ok
        );
    }
}

#[test]
fn v2_12_server_with_password_and_key_or_neither() {
    let mut d = server_key();
    if let DataDraft::Server { password, .. } = &mut d {
        *password = s("falso-SSH-senha-01");
    }
    let e = errors(draft("x", "acme", "dev", d));
    assert!(
        e.has_reason("autenticação", "use senha ou chave, não as duas"),
        "{e}"
    );

    let d = without(without(server_password(), "password"), "private_key");
    let e = errors(draft("x", "acme", "dev", d));
    assert!(
        e.has_reason("autenticação", "informe a senha ou a chave"),
        "{e}"
    );

    // The same goes for ftp/sftp.
    let mut d = sftp();
    if let DataDraft::FtpSftp { private_key, .. } = &mut d {
        *private_key = Some(Secret::from_text(fake_key()));
    }
    assert!(errors(draft("x", "acme", "dev", d)).has("autenticação"));

    // A passphrase with a password does not count.
    let mut d = server_password();
    if let DataDraft::Server { passphrase, .. } = &mut d {
        *passphrase = s("falso-passphrase-01");
    }
    assert!(
        errors(draft("x", "acme", "dev", d))
            .has_reason("passphrase", "passphrase só vale com chave")
    );

    // ftp (without the s) does not accept a key.
    let mut d = without(sftp(), "password");
    if let DataDraft::FtpSftp {
        protocol,
        private_key,
        ..
    } = &mut d
    {
        *protocol = t("ftp");
        *private_key = Some(Secret::from_text(fake_key()));
    }
    assert!(
        errors(draft("x", "acme", "dev", d))
            .has_reason("autenticação", "ftp não aceita chave; use sftp ou senha")
    );

    // Any text in place of the key.
    let mut d = without(server_key(), "private_key");
    if let DataDraft::Server { private_key, .. } = &mut d {
        *private_key = s("falso-SSH-senha-01");
    }
    assert!(errors(draft("x", "acme", "dev", d)).has("chave privada"));
}

#[test]
fn v2_12_database_with_engine_off_the_list() {
    for engine in ["oracle", "Postgres", "postgresql", "sqlite", "postgres "] {
        let mut d = database();
        if let DataDraft::Database { engine: m, .. } = &mut d {
            *m = t(engine);
        }
        let e = errors(draft("x", "acme", "dev", d));
        assert!(
            e.has_reason(
                "motor",
                "fora da lista: postgres, mysql, mongo, redis ou sqlserver"
            ),
            "motor {engine:?}: {e}"
        );
    }
    for engine in Engine::ALL {
        let mut d = database();
        if let DataDraft::Database {
            engine: m, db_name, ..
        } = &mut d
        {
            *m = t(engine.name());
            if engine == Engine::Redis {
                // In redis the database name is a number (D33).
                *db_name = t("1");
            }
        }
        valid(draft("x", "acme", "dev", d));
    }
}

#[test]
fn v2_12_other_formats() {
    let cases: Vec<(DataDraft, &str)> = vec![
        (
            {
                let mut d = database();
                if let DataDraft::Database { port, .. } = &mut d {
                    *port = t("0");
                }
                d
            },
            "porta",
        ),
        (
            {
                let mut d = database();
                if let DataDraft::Database { port, .. } = &mut d {
                    *port = t("65536");
                }
                d
            },
            "porta",
        ),
        (
            {
                let mut d = database();
                if let DataDraft::Database { port, .. } = &mut d {
                    *port = t("+22");
                }
                d
            },
            "porta",
        ),
        (
            {
                let mut d = login();
                if let DataDraft::Login { url, .. } = &mut d {
                    *url = t("javascript:alert(1)");
                }
                d
            },
            "url",
        ),
        (
            {
                let mut d = login();
                if let DataDraft::Login { url, .. } = &mut d {
                    *url = t("https://");
                }
                d
            },
            "url",
        ),
        (
            {
                let mut d = login();
                if let DataDraft::Login { totp_seed, .. } = &mut d {
                    *totp_seed = s("jbswy3dpehpk3pxp");
                }
                d
            },
            "semente TOTP",
        ),
        (
            {
                let mut d = login();
                if let DataDraft::Login { totp_seed, .. } = &mut d {
                    *totp_seed = s("JBSWY3DP");
                }
                d
            },
            "semente TOTP",
        ),
        (
            {
                let mut d = smtp();
                if let DataDraft::Smtp { security, .. } = &mut d {
                    *security = t("ssl");
                }
                d
            },
            "segurança",
        ),
        (
            {
                let mut d = sftp();
                if let DataDraft::FtpSftp { protocol, .. } = &mut d {
                    *protocol = t("ftps");
                }
                d
            },
            "protocolo",
        ),
        (
            {
                let mut d = aws();
                if let DataDraft::Aws { region, .. } = &mut d {
                    *region = t("Sao Paulo");
                }
                d
            },
            "região",
        ),
        (
            {
                let mut d = aws();
                if let DataDraft::Aws { role_arn, .. } = &mut d {
                    *role_arn = t("role/teste");
                }
                d
            },
            "role ARN",
        ),
        (
            {
                let mut d = api();
                if let DataDraft::Api { variable, .. } = &mut d {
                    *variable = t("github-token");
                }
                d
            },
            "variável",
        ),
    ];
    for (data, field) in cases {
        let e = errors(draft("x", "acme", "dev", data));
        assert!(e.has(field), "esperava erro em {field}: {e}");
    }
    // Entry name.
    for name in ["", "Acme", "-x", ".x", "a b", "acme/dev", &"a".repeat(65)] {
        assert!(
            errors(draft(name, "acme", "dev", note())).has("nome"),
            "{name:?}"
        );
    }
}

#[test]
fn v2_12_collects_all_errors() {
    let r = Draft {
        name: String::new(),
        group: String::new(),
        environment: t("qa"),
        project: String::new(),
        tags: vec![t("a b")],
        data: without(without(database(), "password"), "host"),
        extras: vec![extra("", "x", false)],
        attachments: vec![attachment("../x", b"y")],
    };
    let e = errors(r);
    for field in [
        "nome",
        "grupo",
        "ambiente",
        "tags",
        "host",
        "senha",
        "campos extras",
        "anexos",
    ] {
        assert!(e.has(field), "falta {field}: {e}");
    }
    let tag = e.list().iter().find(|x| x.field == "tags").copied();
    assert_eq!(tag.and_then(|x| x.position), Some(1));
}

// ---- V2.13: secret fields of each type ----

#[test]
fn v2_13_secret_fields_of_each_type() {
    use SecretOrigin::{Attachment as A, Extra as X, Field as C};
    let expected: Vec<(&str, Vec<SecretOrigin<'static>>)> = vec![
        ("acme-wp-dev", vec![C("senha"), C("semente TOTP")]),
        ("acme-pg-dev", vec![C("senha")]),
        ("acme-ssh-senha-stage", vec![C("senha")]),
        (
            "acme-ssh-chave-prod",
            vec![C("chave privada"), C("passphrase")],
        ),
        ("beta-github-api", vec![C("token")]),
        (
            "beta-aws-dev",
            vec![C("secret access key"), C("session token")],
        ),
        ("beta-smtp-stage", vec![C("senha")]),
        ("beta-sftp-dev", vec![C("senha")]),
        ("gama-nota", vec![C("texto")]),
        ("gama-generico", vec![X("CHAVE_LICENCA"), A("app.env")]),
    ];
    let entries: Vec<Entry> = one_of_each().into_iter().map(valid).collect();
    for (e, (name, origins)) in entries.iter().zip(&expected) {
        assert_eq!(e.name(), *name);
        let found: Vec<SecretOrigin<'_>> = e.secrets().iter().map(|s| s.origin).collect();
        assert_eq!(&found, origins, "{name}");
        // The non-secret ones are not in the list.
        for sd in e.secrets() {
            assert!(!sd.value.is_empty());
        }
    }
    // The listed values are exactly the test secrets.
    let mut values: Vec<Vec<u8>> = entries
        .iter()
        .flat_map(|e| {
            e.secrets()
                .into_iter()
                .map(|s| s.value.expose().to_vec())
                .collect::<Vec<_>>()
        })
        .collect();
    values.sort();
    for secret in test_secrets() {
        assert!(
            values
                .iter()
                .any(|v| v.windows(secret.len()).any(|w| w == secret.as_bytes())),
            "segredo de teste fora da lista"
        );
    }
    // Ftp/sftp with a key: key and passphrase.
    let mut d = without(sftp(), "password");
    if let DataDraft::FtpSftp {
        private_key,
        passphrase,
        ..
    } = &mut d
    {
        *private_key = Some(Secret::from_text(fake_key()));
        *passphrase = s("falso-passphrase-01");
    }
    let e = valid(draft("x", "beta", "dev", d));
    let origins: Vec<_> = e.secrets().iter().map(|s| s.origin).collect();
    assert_eq!(origins, vec![C("chave privada"), C("passphrase")]);
    // A non-secret extra does not go in; every attachment does, even in a type that is
    // not generic.
    let mut r = draft("y", "acme", "dev", login());
    r.extras = vec![extra("SITE_ID", "7", false)];
    r.attachments = vec![attachment("ca-teste.pem", b"conteudo falso")];
    let e = valid(r);
    let origins: Vec<_> = e.secrets().iter().map(|s| s.origin).collect();
    assert_eq!(
        origins,
        vec![C("senha"), C("semente TOTP"), A("ca-teste.pem")]
    );
}

// ---- V2.16: names of extras and attachments (validation part) ----

#[test]
fn v2_16_normalization_and_duplicates() {
    assert_eq!(variable_name("api-key"), "API_KEY");
    assert_eq!(variable_name("chave acesso.pem"), "CHAVE_ACESSO_PEM");
    assert_eq!(variable_name("região"), "REGI_O");
    assert_eq!(variable_name("app.env"), "APP_ENV");

    let mut r = generic();
    r.extras = vec![extra("api-key", "1", false), extra("API_KEY", "2", true)];
    let e = errors(r);
    assert!(
        e.has_reason(
            "campos extras",
            "vira a mesma variável de outro campo extra"
        ),
        "{e}"
    );
    assert_eq!(e.list()[0].position, Some(2));

    let mut r = generic();
    r.extras.push(extra("api key", "1", false));
    r.extras.push(extra("Api.Key", "2", false));
    assert!(errors(r).has("campos extras"));

    let mut r = generic();
    r.attachments = vec![
        attachment("chave acesso.pem", b"a"),
        attachment("chave-acesso.pem", b"b"),
    ];
    assert!(errors(r).has_reason("anexos", "vira a mesma variável de outro anexo"));

    let mut r = generic();
    r.extras = vec![extra("api-key", "1", false)];
    r.attachments = vec![attachment("chave acesso.pem", b"a")];
    let e = valid(r);
    assert_eq!(e.extras()[0].name(), "api-key");
    assert_eq!(e.attachments()[0].name(), "chave acesso.pem");
}

// ---- V2.27 (SEG): no formatting shows a secret ----

#[test]
fn v2_27_debug_and_display_without_secret() {
    for r in one_of_each() {
        no_secret(&format!("{r:?}"), "Debug do rascunho");
        no_secret(&format!("{r:#?}"), "Debug alternativo do rascunho");
        let e = valid(r);
        let outputs = [
            format!("{e:?}"),
            format!("{e:#?}"),
            format!("{e}"),
            format!("{:?}", e.data()),
            format!("{:?}", e.extras()),
            format!("{:?}", e.attachments()),
            format!("{:?}", e.secrets()),
        ];
        for output in &outputs {
            no_secret(output, "formatação da entrada");
        }
        assert!(outputs[0].contains("••••") || e.secrets().is_empty());
        for sd in e.secrets() {
            assert_eq!(format!("{:?}", sd.value), "Secret(••••)");
            assert_eq!(format!("{}", sd.value), "••••");
            assert_eq!(format!("{:>20}", sd.value), "••••");
        }
    }
}

#[test]
fn v2_27_errors_do_not_carry_user_value() {
    // Attack: the secret ends up in wrong fields (name, host, username, URL, extra,
    // tag, engine, environment…) and in invalid secret fields. No error repeats it.
    let target = "falso-PG:s3nha/01@#";
    let invisible = format!("{target}\u{202E}");
    let mut d = database();
    if let DataDraft::Database {
        engine,
        host,
        port,
        db_name,
        username,
        password,
        tunnel,
    } = &mut d
    {
        *engine = t(target);
        *host = t(target);
        *port = t(target);
        *db_name = invisible.clone();
        *username = format!("-{target}");
        *password = Some(Secret::from_text(format!("{target}\0")));
        *tunnel = t(target);
    }
    let r = Draft {
        name: t(target),
        group: invisible.clone(),
        environment: t(target),
        project: invisible.clone(),
        tags: vec![t(target), format!("{target} x")],
        data: d,
        extras: vec![
            extra(target, target, false),
            extra(&invisible, target, true),
        ],
        attachments: vec![attachment(&format!("../{target}"), target.as_bytes())],
    };
    let e = errors(r);
    assert!(e.list().len() >= 10, "{e}");
    for text in [format!("{e}"), format!("{e:?}"), format!("{e:#?}")] {
        no_secret(&text, "erro de validação");
        assert!(!text.contains("s3nha"));
    }
    for x in e.list() {
        no_secret(&x.to_string(), "erro de validação");
    }
    // Also in the errors of the vault verification.
    let a = valid(draft("a", "acme", "dev", database()));
    let other = [valid(draft("a", "acme", "dev", note()))];
    let error = a.verify_in_vault(&other).expect_err("nome repetido");
    no_secret(&format!("{error} {error:?}"), "erro de conferência");
}

// ---- V2.28 (SEG): zeroed on drop, no Clone ----

/// Fails to compile if `T` implements `Clone` (the two impls become ambiguous).
trait NotClone<A> {
    fn check() {}
}
impl<T> NotClone<()> for T {}
#[allow(dead_code)]
struct ImplementsClone;
impl<T: Clone> NotClone<ImplementsClone> for T {}

fn zeroes_on_drop<T: zeroize::ZeroizeOnDrop>() {}

#[test]
fn v2_28_secret_types_zeroize_and_do_not_clone() {
    // At compile time: if one of these gains `Clone`, the test does not compile.
    <Secret as NotClone<_>>::check();
    <Entry as NotClone<_>>::check();
    <Data as NotClone<_>>::check();
    <Login as NotClone<_>>::check();
    <Database as NotClone<_>>::check();
    <Server as NotClone<_>>::check();
    <Auth as NotClone<_>>::check();
    <Api as NotClone<_>>::check();
    <Aws as NotClone<_>>::check();
    <Smtp as NotClone<_>>::check();
    <FtpSftp as NotClone<_>>::check();
    <SecureNote as NotClone<_>>::check();
    <ExtraField as NotClone<_>>::check();
    <ExtraValue as NotClone<_>>::check();
    <Attachment as NotClone<_>>::check();
    <Draft as NotClone<_>>::check();
    <DataDraft as NotClone<_>>::check();
    <ExtraDraft as NotClone<_>>::check();
    <AttachmentDraft as NotClone<_>>::check();
    <crate::format::MasterPassword as NotClone<_>>::check();
    <crate::format::OpenVault as NotClone<_>>::check();
    <crate::format::VaultKeys as NotClone<_>>::check();
    // Configuration of the v2 body (Phase 19b, D30).
    <crate::config::PinHash as NotClone<_>>::check();
    <crate::config::VaultConfig as NotClone<_>>::check();

    zeroes_on_drop::<Secret>();
    zeroes_on_drop::<Entry>();
    zeroes_on_drop::<Data>();
    zeroes_on_drop::<Auth>();
    zeroes_on_drop::<ExtraField>();
    zeroes_on_drop::<Attachment>();
    // The format's data key (Phase 13) is `Zeroizing<[u8; 32]>`.
    zeroes_on_drop::<crate::crypto::Key>();
    // The master password and the open vault (which holds the data key), Phase 15a.
    zeroes_on_drop::<crate::format::MasterPassword>();
    zeroes_on_drop::<crate::format::OpenVault>();
    zeroes_on_drop::<crate::format::VaultKeys>();
    // The PIN hash and the configuration that holds it (Phase 19b, D30).
    zeroes_on_drop::<crate::config::PinHash>();
    zeroes_on_drop::<crate::config::VaultConfig>();

    assert!(core::mem::needs_drop::<Secret>());
}

// ---- Attacks on validation ----

#[test]
fn attack_host_and_username_become_ssh_option() {
    // `ward ssh` puts host and username in the `ssh` argv: starting with '-' would turn
    // into an option (e.g. -oProxyCommand=…, command execution).
    for host in [
        "-oProxyCommand=touch /tmp/x",
        "-p",
        "host com espaço",
        "a;b",
        "a\nb",
    ] {
        let mut d = server_password();
        if let DataDraft::Server { host: h, .. } = &mut d {
            *h = t(host);
        }
        assert!(errors(draft("x", "acme", "dev", d)).has("host"), "{host:?}");
    }
    for username in ["-oProxyCommand=id", "-l", "de ploy"] {
        for data in [server_password(), sftp(), database(), smtp()] {
            let mut d = data;
            match &mut d {
                DataDraft::Server { username: u, .. }
                | DataDraft::FtpSftp { username: u, .. }
                | DataDraft::Database { username: u, .. }
                | DataDraft::Smtp { username: u, .. } => *u = t(username),
                _ => unreachable!(),
            }
            assert!(
                errors(draft("x", "acme", "dev", d)).has("usuário"),
                "{username:?}"
            );
        }
    }
    // Jump and tunnel with a name that would turn into an option.
    let mut d = server_password();
    if let DataDraft::Server { jump, .. } = &mut d {
        *jump = t("-J");
    }
    assert!(errors(draft("x", "acme", "dev", d)).has("servidor de salto"));
}

#[test]
fn attack_invisible_text_forges_the_approval_window() {
    // Bidi override, zero width, line separator and control: the name shown in the
    // approval window and in the list cannot lie.
    for c in [
        '\u{202E}',
        '\u{200B}',
        '\u{2066}',
        '\u{2028}',
        '\u{FEFF}',
        '\u{1B}',
        '\r',
        '\n',
        '\u{E0041}',
        '\u{00AD}',
    ] {
        let fake = format!("acme{c}prod");
        let mut r = draft("x", &fake, "dev", note());
        r.project = fake.clone();
        r.extras = vec![extra(&fake, "v", false), extra("OK", &fake, false)];
        r.attachments = vec![attachment(&fake, b"y")];
        let e = errors(r);
        assert!(e.has_reason("grupo", "caractere não permitido"), "{c:?}");
        assert!(e.has("projeto"), "{c:?}");
        assert_eq!(
            e.list()
                .iter()
                .filter(|x| x.field == "campos extras")
                .count(),
            2,
            "{c:?}"
        );
        assert!(e.has("anexos"), "{c:?}");
    }
    // A name with a space at the ends too.
    assert!(errors(draft("x", " acme", "dev", note())).has("grupo"));
    // A secret extra may have a line break (it goes to the environment), but not NUL.
    let mut r = generic();
    r.extras.push(extra("MULTI", "linha1\nlinha2", true));
    valid(r);
    let mut r = generic();
    r.extras.push(extra("NULO", "a\0b", true));
    assert!(errors(r).has_reason("campos extras", "não pode ter o caractere nulo"));
}

#[test]
fn attack_secret_with_nul_or_binary() {
    let mut d = login();
    if let DataDraft::Login { password, .. } = &mut d {
        *password = s("falso\0WP");
    }
    assert!(
        errors(draft("x", "acme", "dev", d)).has_reason("senha", "não pode ter o caractere nulo")
    );
    let mut d = api();
    if let DataDraft::Api { token, .. } = &mut d {
        *token = Some(Secret::from_bytes(vec![0xff, 0xfe, b'a']));
    }
    assert!(errors(draft("x", "acme", "dev", d)).has_reason("token", "precisa ser texto (UTF-8)"));
    // An attachment may be binary.
    let mut r = generic();
    r.attachments = vec![attachment("cert.p12", &[0, 1, 2, 0xff])];
    valid(r);
}

#[test]
fn attack_dangerous_variable_in_api() {
    for var in [
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "PATH",
        "BASH_ENV",
        "SSHPASS",
        "GIT_SSH_COMMAND",
        "NODE_OPTIONS",
        "WARD_PASS",
        "WARD_ENTRY",
        "WARD_ENTRADA",
        "BASH_FUNC_x%%",
        "IFS",
    ] {
        let mut d = api();
        if let DataDraft::Api { variable, .. } = &mut d {
            *variable = t(var);
        }
        assert!(
            errors(draft("x", "beta", "dev", d)).has("variável"),
            "{var}"
        );
    }
    for var in [
        "GITHUB_TOKEN",
        "GITLAB_TOKEN",
        "OPENAI_API_KEY",
        "NPM_TOKEN",
    ] {
        let mut d = api();
        if let DataDraft::Api { variable, .. } = &mut d {
            *variable = t(var);
        }
        valid(draft("x", "beta", "dev", d));
    }
}

#[test]
fn attack_extra_or_attachment_overrides_type_variable() {
    // An extra "PASS" (not secret) in a login would become WARD_PASS, over the password.
    let reserved: Vec<(MakeData, &str)> = vec![
        (login, "pass"),
        (login, "TOTP"),
        (login, "url"),
        (server_password, "host"),
        (server_password, "passphrase"),
        (sftp, "protocol"),
        (api, "token"),
        (note, "entry"),
        (aws, "GROUP"),
        (smtp, "environment"),
        (database, "file_app_env"),
        (database, "file"),
        (login, "File-Key"),
    ];
    for (data, name) in reserved {
        let mut r = draft("x", "acme", "dev", data());
        r.extras = vec![extra(name, "v", false)];
        assert!(
            errors(r).has_reason("campos extras", "vira uma variável que o tipo já entrega"),
            "{name}"
        );
    }
    // An attachment named like the key in the server would become the key file variable,
    // over the entry's own key.
    for data in [server_password(), sftp()] {
        let mut r = draft("x", "acme", "dev", data);
        r.attachments = vec![attachment("key", b"x")];
        assert!(errors(r).has("anexos"));
    }
    // In other types, those two are common names.
    let mut r = generic();
    r.extras.push(extra("PASS", "v", true));
    r.attachments.push(attachment("key", b"x"));
    valid(r);
}

/// m2:DT11: the 0.1.x names are no longer delivered by the type, so they stop being
/// reserved; they are user data like any other extra or attachment name.
#[test]
fn m2_dt11_old_names_are_not_reserved() {
    let mut r = draft("x", "acme", "dev", sftp());
    r.extras = vec![
        extra("entrada", "v", false),
        extra("GRUPO", "v", false),
        extra("ambiente", "v", false),
        extra("protocolo", "v", false),
        extra("arquivo_x", "v", false),
    ];
    r.attachments = vec![attachment("chave", b"x")];
    valid(r);
}

#[test]
fn attack_attachment_name_leaves_the_directory() {
    for name in [
        "../x",
        "a/b",
        "..",
        ".",
        ".env",
        "-rf",
        "a\\b",
        "/etc/passwd",
        "__",
        "...",
    ] {
        let mut r = generic();
        r.attachments = vec![attachment(name, b"x")];
        assert!(errors(r).has("anexos"), "{name:?}");
    }
    let mut r = generic();
    r.attachments = vec![attachment("vazio.txt", b"")];
    assert!(errors(r).has_reason("anexos", "anexo vazio"));
}

#[test]
fn attack_excessive_sizes() {
    let large = |n: usize| Some(Secret::from_bytes(vec![b'a'; n]));
    let mut d = login();
    if let DataDraft::Login { password, .. } = &mut d {
        *password = large(64 * 1024 + 1);
    }
    assert!(errors(draft("x", "acme", "dev", d)).has_reason("senha", "longo demais"));
    let d = DataDraft::SecureNote {
        text: large(1024 * 1024 + 1),
    };
    assert!(errors(draft("x", "acme", "dev", d)).has_reason("texto", "longo demais"));
    let d = DataDraft::SecureNote {
        text: large(1024 * 1024),
    };
    valid(draft("x", "acme", "dev", d));
    let mut d = login();
    if let DataDraft::Login { totp_seed, .. } = &mut d {
        *totp_seed = Some(Secret::from_bytes(vec![b'A'; 129]));
    }
    assert!(errors(draft("x", "acme", "dev", d)).has("semente TOTP"));

    let mut r = generic();
    r.attachments = vec![AttachmentDraft {
        name: t("grande.bin"),
        content: Secret::from_bytes(vec![0; crate::limits::MAX_ATTACHMENTS_TOTAL + 1]),
    }];
    assert!(errors(r).has_reason("anexos", "longo demais"));

    let mut r = generic();
    r.extras = (0..65)
        .map(|i| extra(&format!("E{i}"), "v", false))
        .collect();
    assert!(errors(r).has_reason("campos extras", "itens demais"));
    let mut r = generic();
    r.attachments = (0..33)
        .map(|i| attachment(&format!("a{i}"), b"v"))
        .collect();
    assert!(errors(r).has_reason("anexos", "itens demais"));
    let mut r = generic();
    r.tags = (0..17).map(|i| format!("t{i}")).collect();
    assert!(errors(r).has_reason("tags", "itens demais"));
    let mut r = generic();
    r.tags = vec![t("a"), t("a")];
    assert!(errors(r).has_reason("tags", "tag repetida"));
    let mut r = generic();
    r.group = "g".repeat(257);
    assert!(errors(r).has_reason("grupo", "longo demais"));
    let mut d = login();
    if let DataDraft::Login { url, .. } = &mut d {
        *url = format!("https://a/{}", "b".repeat(2048));
    }
    assert!(errors(draft("x", "acme", "dev", d)).has_reason("url", "longo demais"));
}

#[test]
fn attack_level_does_not_change_on_save_or_edit() {
    // S9: a new entry always starts "with approval" (the draft has no level).
    let mut current = valid(draft("acme-pg-prod", "acme", "prod", database()));
    assert_eq!(current.level(), AccessLevel::Approval);
    // The daemon (with the PIN) blocks; an edit without the PIN cannot unblock.
    current.set_level(AccessLevel::Blocked);
    let edited = valid(draft("acme-pg-prod", "acme", "prod", database()));
    assert_eq!(edited.level(), AccessLevel::Approval);
    current.replace_with(edited).expect("mesmo tipo");
    assert_eq!(current.level(), AccessLevel::Blocked);
    // The type does not change on edit.
    let other = valid(draft("acme-pg-prod", "acme", "prod", note()));
    let e = current.replace_with(other).expect_err("tipo mudou");
    assert!(e.has_reason("tipo", "o tipo não muda depois de criado"));
    assert_eq!(current.kind(), EntryType::Database);
}

#[test]
fn combined_level_and_names() {
    use AccessLevel::*;
    assert_eq!(Free.most_restrictive(Approval), Approval);
    assert_eq!(Approval.most_restrictive(Free), Approval);
    assert_eq!(Free.most_restrictive(Blocked), Blocked);
    assert_eq!(Free.most_restrictive(Free), Free);
    assert_eq!(AccessLevel::default(), Approval);
    for n in AccessLevel::ALL {
        assert_eq!(AccessLevel::from_name(n.name()), Some(n));
    }
    for kind in EntryType::ALL {
        assert_eq!(EntryType::from_name(kind.name()), Some(kind));
    }
    assert_eq!(EntryType::FtpSftp.label(), "ftp/sftp");
    assert_eq!(EntryType::SecureNote.label(), "nota segura");
    assert_eq!(EntryType::Generic.label(), "genérico");
    assert_eq!(Approval.label(), "com aprovação");
    // m2:DT4: the stored names are the v5 ones; the labels stay the pt-BR texts.
    let names: Vec<&str> = EntryType::ALL.iter().map(|k| k.name()).collect();
    assert_eq!(
        names,
        [
            "login",
            "database",
            "server",
            "api",
            "aws",
            "smtp",
            "ftp-sftp",
            "secure-note",
            "generic"
        ]
    );
    let labels: Vec<&str> = EntryType::ALL.iter().map(|k| k.label()).collect();
    assert_eq!(
        labels,
        [
            "login",
            "banco",
            "servidor",
            "api",
            "aws",
            "smtp",
            "ftp/sftp",
            "nota segura",
            "genérico"
        ]
    );
    let levels: Vec<(&str, &str)> = AccessLevel::ALL
        .iter()
        .map(|n| (n.name(), n.label()))
        .collect();
    assert_eq!(
        levels,
        [
            ("blocked", "bloqueado"),
            ("approval", "com aprovação"),
            ("free", "livre")
        ]
    );
    let securities: Vec<&str> = SmtpSecurity::ALL.iter().map(|s| s.name()).collect();
    assert_eq!(securities, ["none", "starttls", "tls"]);
    // The v4 names are not known outside the v4 reading of the body (m2:RM2).
    for v4 in [
        "banco",
        "servidor",
        "nota-segura",
        "generico",
        "bloqueado",
        "com-aprovacao",
        "livre",
    ] {
        assert_eq!(EntryType::from_name(v4), None, "{v4}");
        assert_eq!(AccessLevel::from_name(v4), None, "{v4}");
    }
    assert_eq!(SmtpSecurity::from_name("nenhuma"), None);
}

// ---- Vault verification (unique name, tunnel and jump) ----

#[test]
fn verify_in_vault_references() {
    let server = |name: &str, jump: &str| {
        let mut d = server_password();
        if let DataDraft::Server { jump: j, .. } = &mut d {
            *j = t(jump);
        }
        valid(draft(name, "acme", "dev", d))
    };
    let database_via = |name: &str, tunnel: &str| {
        let mut d = database();
        if let DataDraft::Database { tunnel: t_, .. } = &mut d {
            *t_ = t(tunnel);
        }
        valid(draft(name, "acme", "dev", d))
    };
    let vault = vec![
        server("salto", ""),
        server("interno", "salto"),
        valid(draft("site", "acme", "dev", login())),
    ];

    // Ok: tunnel and jump to a server without a jump.
    database_via("pg", "salto")
        .verify_in_vault(&vault)
        .expect("ok");
    server("novo", "salto").verify_in_vault(&vault).expect("ok");

    let cases = [
        (
            server("salto", ""),
            "nome",
            "já existe outra entrada com este nome",
        ),
        (
            database_via("pg", "nao-existe"),
            "túnel SSH",
            "precisa ser uma entrada servidor existente",
        ),
        (
            database_via("pg", "site"),
            "túnel SSH",
            "precisa ser uma entrada servidor existente",
        ),
        (
            database_via("pg", "interno"),
            "túnel SSH",
            "o servidor do túnel não pode ter salto",
        ),
        (
            server("eu", "eu"),
            "servidor de salto",
            "aponta para a própria entrada",
        ),
        (
            server("x", "interno"),
            "servidor de salto",
            "o servidor de salto não pode ter salto próprio",
        ),
        (
            server("x", "site"),
            "servidor de salto",
            "precisa ser uma entrada servidor existente",
        ),
    ];
    for (e, field, reason) in cases {
        let error = e.verify_in_vault(&vault).expect_err(reason);
        assert!(error.has_reason(field, reason), "{error}");
    }

    // A server used as a jump cannot gain its own jump (it would become a chain).
    let edited_jump = server("salto", "outro");
    let rest: Vec<Entry> = vec![server("interno", "salto"), server("outro", "")];
    let error = edited_jump.verify_in_vault(&rest).expect_err("cadeia");
    assert!(error.has_reason(
        "servidor de salto",
        "outra entrada usa este servidor como salto ou túnel; ele não pode ter salto"
    ));
}

// ---- D29, D33 (Phase 19a) ----

/// Attachments of `sizes` bytes, with content that does not become a per-line pattern
/// (no `\n`).
fn attachments_of(sizes: &[usize]) -> Vec<AttachmentDraft> {
    sizes
        .iter()
        .enumerate()
        .map(|(i, &n)| attachment(&format!("a{i}.bin"), &vec![0x5a; n]))
        .collect()
}

#[test]
fn v2_36_attachments_total_and_budget_on_save() {
    use crate::limits::{MAX_ATTACHMENTS_TOTAL, MAX_EXTRA_VALUE, MAX_EXTRAS};
    const KIB: usize = 1024;

    // One attachment of 256 KiB + 1 byte.
    let mut r = generic();
    r.attachments = attachments_of(&[MAX_ATTACHMENTS_TOTAL + 1]);
    assert!(errors(r).has_reason("anexos", "longo demais"));
    // Two attachments of 200 KiB: each one fits, the sum does not.
    let mut r = generic();
    r.attachments = attachments_of(&[200 * KIB, 200 * KIB]);
    let e = errors(r);
    assert!(
        e.has_reason("anexos", "os anexos somam mais de 256 KiB"),
        "{e}"
    );
    // The exact sum at the limit passes (with one attachment and with several).
    let mut r = generic();
    r.attachments = attachments_of(&[MAX_ATTACHMENTS_TOTAL]);
    valid(r);
    let mut r = generic();
    r.attachments = attachments_of(&[MAX_ATTACHMENTS_TOTAL / 2, MAX_ATTACHMENTS_TOTAL / 2]);
    valid(r);

    // `api` with 64 secret extras of 64 KiB: each field fits, the mask does not.
    let mut r = draft("beta-github-api", "beta", "dev", api());
    r.extras = (0..MAX_EXTRAS)
        .map(|i| ExtraDraft {
            name: format!("e{i:02}"),
            value: Secret::from_bytes(vec![b'k'; MAX_EXTRA_VALUE]),
            secret: true,
        })
        .collect();
    let e = errors(r);
    assert!(
        e.has_reason(
            "entrada",
            "entrada grande demais para a máscara do `ward run`"
        ),
        "{e}"
    );
    assert!(
        e.to_string()
            .contains("entrada grande demais para a máscara do `ward run`")
    );
    assert!(!e.to_string().contains("kkkk"));
    assert_eq!(e.list().len(), 1);

    // The secure note stays out of the budget (it has no delivery, D10).
    let mut r = draft("gama-nota", "gama", "dev", note());
    r.extras = (0..MAX_EXTRAS)
        .map(|i| ExtraDraft {
            name: format!("e{i:02}"),
            value: Secret::from_bytes(vec![b'k'; MAX_EXTRA_VALUE]),
            secret: true,
        })
        .collect();
    valid(r);
}

#[test]
fn v2_37_database_name() {
    let with_db_name = |engine: &str, db_name: &str| {
        let mut d = database();
        if let DataDraft::Database {
            engine: m,
            db_name: b,
            ..
        } = &mut d
        {
            *m = t(engine);
            *b = t(db_name);
        }
        draft("acme-pg-dev", "acme", "dev", d)
    };
    let long = "b".repeat(crate::limits::MAX_DB_NAME + 1);
    for engine in ["postgres", "mysql", "mongo", "sqlserver"] {
        for db_name in [
            "b?x=1",
            "b#x",
            "../b",
            "b.c",
            "b c",
            "b%41",
            "b/c",
            "b&c",
            "b=c",
            "b\u{202e}c",
            "bá",
            long.as_str(),
        ] {
            let e = errors(with_db_name(engine, db_name));
            assert!(e.has("base"), "{engine} {db_name:?}");
            // Message in Portuguese, without the value.
            assert!(!e.to_string().contains(db_name), "{engine} {db_name:?}");
        }
        let e = errors(with_db_name(engine, "b?x=1"));
        assert!(e.has_reason(
            "base",
            "use letras, números, '_' ou '-', com até 63 caracteres"
        ));
        let e = errors(with_db_name(engine, &long));
        assert!(e.has_reason("base", "longo demais"));
        for db_name in [
            "acme_db-1",
            "A",
            "app_teste",
            &"b".repeat(crate::limits::MAX_DB_NAME),
        ] {
            let entry = valid(with_db_name(engine, db_name));
            let Data::Database(b) = entry.data() else {
                panic!("banco")
            };
            assert_eq!(b.db_name.as_deref(), Some(db_name));
        }
        // An empty database name is still optional.
        valid(with_db_name(engine, ""));
    }
    for db_name in [
        "abc",
        "b",
        "-1",
        "1.5",
        "100000",
        "00",
        "03",
        "4294967296",
        " 3",
    ] {
        let e = errors(with_db_name("redis", db_name));
        assert!(e.has("base"), "redis {db_name:?}");
    }
    let e = errors(with_db_name("redis", "abc"));
    assert!(e.has_reason("base", "no redis, a base é um número de 0 a 99999"));
    for db_name in ["0", "3", "15", "99999"] {
        valid(with_db_name("redis", db_name));
    }
    valid(with_db_name("redis", ""));
}

#[test]
fn messages_cite_the_current_limits() {
    // If a limit changes in `crate::limits`, the message that cites it must change too.
    use crate::limits::{
        MAX_ATTACHMENTS_TOTAL, MAX_DB_NAME, MAX_REDIS_DB_NAME, MAX_TOTP, MIN_TOTP,
    };
    let total = format!("{} KiB", MAX_ATTACHMENTS_TOTAL / 1024);
    let db_name = format!("até {MAX_DB_NAME} caracteres");
    let redis = format!("de 0 a {MAX_REDIS_DB_NAME}");
    let totp = format!("com {MIN_TOTP} a {MAX_TOTP} caracteres");
    let mut r = generic();
    r.attachments = attachments_of(&[MAX_ATTACHMENTS_TOTAL, 1]);
    assert!(errors(r).to_string().contains(&total));
    let mut d = database();
    if let DataDraft::Database { db_name: b, .. } = &mut d {
        *b = t("b.c");
    }
    assert!(
        errors(draft("x", "acme", "dev", d))
            .to_string()
            .contains(&db_name)
    );
    let mut d = database();
    if let DataDraft::Database {
        engine, db_name: b, ..
    } = &mut d
    {
        *engine = t("redis");
        *b = t("x");
    }
    assert!(
        errors(draft("x", "acme", "dev", d))
            .to_string()
            .contains(&redis)
    );
    let mut d = login();
    if let DataDraft::Login { totp_seed, .. } = &mut d {
        *totp_seed = s("abc1abc1abc1abc1");
    }
    assert!(
        errors(draft("x", "acme", "dev", d))
            .to_string()
            .contains(&totp)
    );
}

// ---- Phase 19c: D38, D41, D42, D43 ----

/// Characters that the auditor's validation repro showed passing (medium finding
/// 3), plus those of V2.45: format (Cf), Hangul filler, blank braille, variation
/// selector, non-ASCII spaces, combining mark and emoji.
const OFF_THE_LIST: &[&str] = &[
    "\u{0600}",
    "\u{1D173}",
    "\u{13430}",
    "\u{1BCA0}",
    "\u{3164}",
    "\u{115F}",
    "\u{FFA0}",
    "\u{2800}",
    "\u{FE0F}",
    "\u{180B}",
    "\u{E0100}",
    "\u{034F}",
    "\u{200B}",
    "\u{202E}",
    "\u{00A0}",
    "\u{3000}",
    "a\u{0301}",
    "\u{1F600}",
    "\u{D7}",
];

/// The error does not carry the refused text (neither through `Display` nor `Debug`).
fn without_the_value(e: &ValidationErrors, value: &str) {
    for text in [format!("{e}"), format!("{e:?}")] {
        assert!(!text.contains(value), "erro com o valor: {value:?}");
    }
}

/// V2.45 (D38): display text is an allowlist (visible ASCII and the Latin-1/Latin
/// Extended-A of the accents); everything outside it is refused in every field that the
/// approval window and the list show.
#[test]
fn v2_45_display_text_is_allowlist() {
    for c in OFF_THE_LIST {
        let fake = format!("acme{c}prod");
        let mut d = login();
        if let DataDraft::Login { username, .. } = &mut d {
            *username = fake.clone();
        }
        let mut r = draft("x", &fake, "dev", d);
        r.project = fake.clone();
        r.tags = vec![fake.clone()];
        r.extras = vec![extra(&fake, "v", false)];
        r.attachments = vec![attachment(&fake, b"y")];
        let e = errors(r);
        for field in ["grupo", "projeto", "usuário"] {
            assert!(
                e.has_reason(field, "caractere não permitido"),
                "{field} {c:?}"
            );
        }
        assert!(e.has("tags"), "{c:?}");
        assert!(e.has("campos extras"), "{c:?}");
        assert!(e.has("anexos"), "{c:?}");
        without_the_value(&e, c);
        // The entry name too.
        assert!(
            errors(draft(&fake, "acme", "dev", note())).has("nome"),
            "{c:?}"
        );
    }
    for good in [
        "São Paulo",
        "ação",
        "Müller",
        "Ñandú",
        "Łódź",
        "acme-prod_01 (v2)",
    ] {
        let mut r = draft("x", good, "dev", note());
        r.project = t(good);
        valid(r);
    }
    // A secret is not display text: a password with U+200B and U+3164 passes.
    let mut d = login();
    if let DataDraft::Login { password, .. } = &mut d {
        *password = s("falso\u{200B}senha\u{3164}01");
    }
    valid(draft("x", "acme", "dev", d));
}

/// V2.47 (D41), medium finding 4: the URL of `login` and `api` goes through a strict
/// parser.
#[test]
fn v2_47_strict_url() {
    let refused = [
        "https://cliente.com.br@evil.example/",
        "https://evil.example\\@cliente.com.br/",
        "https://evil.example%2f@cliente.com.br",
        "http://evil.example",
        "https://x.example:99999",
        "https://x.example:0",
        "https://x.example:080",
        "HTTPS://x.example",
        "https://X.example",
        "https://\u{441}liente.com.br",
        "https://-x.example",
        "https://x-.example",
        "https://x.example/a\\b",
        "https://x.example/%zz",
        "https://x..example",
        "https://010.0.0.1",
        "https://x/../\u{0}",
        "https://x.example/a b",
        "https://",
        "https://:443",
        "https://[::1",
        "https://[evil]/",
        "javascript:alert(1)",
        "ftp://x.example",
        "https://x.example\u{202E}",
    ];
    for url in refused {
        let mut d = login();
        if let DataDraft::Login { url: u, .. } = &mut d {
            *u = t(url);
        }
        let e = errors(draft("x", "acme", "dev", d));
        assert!(e.has("url"), "login {url:?}");
        let msg = format!("{e}");
        assert!(
            msg.contains("use https://") || msg.contains("caractere não permitido"),
            "{url:?}: {msg}"
        );
        // "https://" alone is also the start of the message, which explains it.
        if url != "https://" {
            without_the_value(&e, url);
        }
        let mut d = api();
        if let DataDraft::Api { url: u, .. } = &mut d {
            *u = t(url);
        }
        assert!(
            errors(draft("x", "beta", "dev", d)).has("url"),
            "api {url:?}"
        );
    }
    for url in [
        "https://cliente.com.br/wp-login.php?x=1#y",
        "https://xn--clente-9ya.com.br",
        "https://[::1]:8443/",
        "http://localhost:8083/login",
        "http://127.0.0.1:8081/",
        "https://api.github.com",
        "https://10.0.0.1:65535/a/%20b",
    ] {
        let mut d = login();
        if let DataDraft::Login { url: u, .. } = &mut d {
            *u = t(url);
        }
        valid(draft("x", "acme", "dev", d));
        let mut d = api();
        if let DataDraft::Api { url: u, .. } = &mut d {
            *u = t(url);
        }
        valid(draft("x", "beta", "dev", d));
    }
}

/// Variables that change what a child process runs or where it sends the data (the
/// auditor's repro, medium finding 6) and those of V2.48.
const DANGEROUS_VARIABLES: &[&str] = &[
    "GIT_PROXY_COMMAND",
    "GIT_EXTERNAL_DIFF",
    "PAGER",
    "GIT_PAGER",
    "EDITOR",
    "LESSOPEN",
    "PS0",
    "PROMPT_COMMAND",
    "BROWSER",
    "JAVA_TOOL_OPTIONS",
    "OPENSSL_CONF",
    "SSLKEYLOGFILE",
    "HTTPS_PROXY",
    "LC_ALL",
    "LOCPATH",
    "PYTHONWARNINGS",
    "HOSTALIASES",
    "TZ",
    "SSHPASS",
    "LD_PRELOAD",
    "WARD_X_TOKEN",
    "GIT_ASKPASS",
    "SSH_ASKPASS_TOKEN",
    "LD_X_TOKEN",
    "github_token",
    "1_TOKEN",
    "_TOKEN",
];

/// The ones that pass (V2.48): they end in a credential suffix.
const GOOD_VARIABLES: &[&str] = &[
    "GITHUB_TOKEN",
    "GH_TOKEN",
    "NPM_TOKEN",
    "OPENAI_API_KEY",
    "STRIPE_SECRET_KEY",
    "DOCKER_PASSWORD",
    "NPM_CONFIG__AUTH",
    "SONAR_PAT",
];

/// V2.48 (D42): the `api` variable needs the credential suffix; the denylist still
/// applies on top.
#[test]
fn v2_48_api_variable_with_suffix() {
    for var in DANGEROUS_VARIABLES {
        let mut d = api();
        if let DataDraft::Api { variable, .. } = &mut d {
            *variable = t(var);
        }
        let e = errors(draft("x", "beta", "dev", d));
        assert!(e.has("variável"), "{var}");
        without_the_value(&e, var);
    }
    for var in GOOD_VARIABLES {
        let mut d = api();
        if let DataDraft::Api { variable, .. } = &mut d {
            *variable = t(var);
        }
        valid(draft("x", "beta", "dev", d));
    }
}

/// V2.49 (D43), low finding 8: the username of the server and `ftp/sftp` types goes to
/// `ssh`/`sftp`. `@` passes (domain account, the user's decision); `;`, `$(…)`, `%h`,
/// space, backslash, accent and a leading `-` do not.
#[test]
fn v2_49_ssh_username() {
    let bad = [
        "u;id",
        "u'$(id)'",
        "u%h",
        "-l",
        "-deploy@x",
        "u v",
        "u\\x",
        "usuário",
        "deploy@x;id",
        "deploy@'$(id)'",
        "deploy@%h",
        "u\"x",
        "u`id`",
        "u/x",
        "u:x",
    ];
    for username in bad {
        for data in [server_password(), sftp()] {
            let mut d = data;
            match &mut d {
                DataDraft::Server { username: u, .. } | DataDraft::FtpSftp { username: u, .. } => {
                    *u = t(username)
                }
                _ => unreachable!(),
            }
            let e = errors(draft("x", "acme", "dev", d));
            assert!(e.has("usuário"), "{username:?}");
            // This one is also the field label, which the error carries.
            if username != "usuário" {
                without_the_value(&e, username);
            }
        }
    }
    for username in [
        "deploy",
        "app_user",
        "j.silva",
        "www-data",
        "deploy@cliente.com.br",
        "a",
    ] {
        for data in [server_password(), sftp()] {
            let mut d = data;
            match &mut d {
                DataDraft::Server { username: u, .. } | DataDraft::FtpSftp { username: u, .. } => {
                    *u = t(username)
                }
                _ => unreachable!(),
            }
            valid(draft("x", "acme", "dev", d));
        }
    }
    // Up to the username limit of `limits`.
    let mut d = server_password();
    if let DataDraft::Server { username, .. } = &mut d {
        *username = "u".repeat(crate::limits::MAX_USERNAME + 1);
    }
    assert!(errors(draft("x", "acme", "dev", d)).has("usuário"));
}

/// V2.58 (D55, the auditor's `url.rs` repro): a last domain label that starts with a
/// digit is refused, because Chrome and `curl` read `0x7f000001`, `0x7f.0.0.1` and
/// `1.2.3.0x4` as IPv4 (WHATWG hex and numeric forms) and `x.123` is not a domain.
/// The message is the invalid URL one, without the value. The other repro cases that
/// were already refused stay refused; the accepted ones of V2.47 and V2.58 pass.
#[test]
fn v2_58_last_label_with_digit() {
    let refused = [
        // D55
        "https://0x7f000001/",
        "https://0x7f.0.0.1/",
        "https://1.2.3.0x4/",
        "https://x.123/",
        "https://x.0x10/",
        "https://x.example.9z/",
        "https://0X7F000001/",
        "https://017700000001/",
        "http://0x7f000001/",
        "https://x.example.0/",
        // From the repro, already refused before D55.
        "https://127.0.0.1./",
        "https://cliente.com.br./",
        "http://localhost./",
        "http://LOCALHOST/",
        "http://localhost:0/",
        "http://localhost:65536/",
        "https://x.example:/",
        "http://[::1]/",
        "https://[::1%25lo]/",
        "https://[v1.x]/",
        "https://[::1]x/",
        "https://[::1]:/",
        "https://a_b.example/",
        "https://1.1.1.1.1/",
        "https://999.1.1.1/",
        "https://xn--/",
        "https://x.example/\u{7f}",
        "https:x.example",
        "https:/x.example",
        "https:///x.example",
        "https://x.example;@evil/",
        "https://[::1]:443:1/",
        "https://x.example:44a/",
        "https://x.example\u{ff0e}com/",
    ];
    for url in refused {
        let mut d = login();
        if let DataDraft::Login { url: u, .. } = &mut d {
            *u = t(url);
        }
        let e = errors(draft("x", "acme", "dev", d));
        assert!(e.has("url"), "login {url:?}");
        let msg = format!("{e}");
        assert!(
            msg.contains("use https://") || msg.contains("caractere não permitido"),
            "{url:?}: {msg}"
        );
        without_the_value(&e, url);
        let mut d = api();
        if let DataDraft::Api { url: u, .. } = &mut d {
            *u = t(url);
        }
        assert!(
            errors(draft("x", "beta", "dev", d)).has("url"),
            "api {url:?}"
        );
    }
    for url in [
        "https://cliente.com.br/",
        "https://x1.example/",
        "https://1password.com/",
        "https://x.example2/",
        "https://9gag.com/",
        "https://a.b2c.example/",
        "https://cliente.com.br/wp-login.php?x=1#y",
        "https://xn--clente-9ya.com.br",
        "https://[::1]:8443/",
        "http://localhost:8083/login",
        "http://127.0.0.1:8081/",
        "https://10.0.0.1/",
    ] {
        let mut d = login();
        if let DataDraft::Login { url: u, .. } = &mut d {
            *u = t(url);
        }
        valid(draft("x", "acme", "dev", d));
        let mut d = api();
        if let DataDraft::Api { url: u, .. } = &mut d {
            *u = t(url);
        }
        valid(draft("x", "beta", "dev", d));
    }
}

/// Phase 44 (D9): the whole origin of a URL, for `ward playwright` to compare with the
/// page's. Attacks: subdomain, port, scheme, disguised userinfo and `http` outside
/// `localhost` give another origin or none.
#[test]
fn d9_url_origin() {
    let o = url_origin;
    assert_eq!(
        o("http://localhost:8081/wp-login.php").as_deref(),
        Some("http://localhost:8081")
    );
    assert_eq!(
        o("http://localhost:80/x").as_deref(),
        Some("http://localhost")
    );
    assert_eq!(o("http://localhost").as_deref(), Some("http://localhost"));
    assert_eq!(
        o("https://acme.com.br:443/a?b#c").as_deref(),
        Some("https://acme.com.br")
    );
    assert_eq!(
        o("https://acme.com.br:8443").as_deref(),
        Some("https://acme.com.br:8443")
    );
    assert_eq!(
        o("http://127.0.0.1:8084/login").as_deref(),
        Some("http://127.0.0.1:8084")
    );
    assert_eq!(o("https://[::1]:9/").as_deref(), Some("https://[::1]:9"));
    let base = o("https://acme.com.br/login");
    for other in [
        "https://www.acme.com.br/login",
        "https://acme.com.br.evil.io/login",
        "https://acme.com.br:444/login",
        "http://localhost:8082/wp-login.php",
    ] {
        assert_ne!(o(other), base, "{other}");
    }
    for bad in [
        "http://acme.com.br/login",
        "https://user@acme.com.br/",
        "https://acme.com.br%40evil.io/",
        "https://ACME.com.br/",
        "http://evil.io/",
        "javascript:alert(1)",
        "about:blank",
        "https://acme.com.br\\@evil.io/",
        "https://0x7f000001/",
        "",
    ] {
        assert_eq!(o(bad), None, "{bad}");
    }
}

/// Phase 44e (D9, 2nd audit of M6): every loopback host is a local origin, in any
/// scheme. Attack: `https://localhost`, `https://127.0.0.1` or `https://[::1]` with a
/// local CA certificate (mkcert, `~/.pki`) would go without a window; the disguised
/// forms of loopback count too.
#[test]
fn attack_local_origin_in_any_scheme() {
    let o = local_url_origin;
    for (url, origin) in [
        (
            "http://localhost:8081/wp-login.php",
            "http://localhost:8081",
        ),
        ("http://127.0.0.1:8084/login", "http://127.0.0.1:8084"),
        ("https://localhost/", "https://localhost"),
        ("https://localhost:8443/x", "https://localhost:8443"),
        ("https://127.0.0.1/", "https://127.0.0.1"),
        ("https://127.1.2.3:9/", "https://127.1.2.3:9"),
        ("https://[::1]:8443/", "https://[::1]:8443"),
        ("https://[0:0:0:0:0:0:0:1]/", "https://[0:0:0:0:0:0:0:1]"),
        ("https://[::]/", "https://[::]"),
        ("https://[::ffff:127.0.0.1]/", "https://[::ffff:127.0.0.1]"),
        ("https://[::ffff:7f00:1]/", "https://[::ffff:7f00:1]"),
        ("https://[::127.0.0.1]/", "https://[::127.0.0.1]"),
        ("https://0.0.0.0/", "https://0.0.0.0"),
        ("https://wp.localhost/", "https://wp.localhost"),
        ("https://a.b.localhost:8443/", "https://a.b.localhost:8443"),
    ] {
        assert_eq!(o(url).as_deref(), Some(origin), "{url}");
    }
    for outside in [
        "https://acme.com.br/login",
        "https://localhost.acme.com.br/",
        "https://localhostx/",
        "https://xlocalhost/",
        "https://128.0.0.1/",
        "https://10.0.0.1/",
        "https://[::2]/",
        "https://[::ffff:8.8.8.8]/",
        "https://[2001:db8::1]/",
        // Refused by the parser: they have no origin at all (and nothing is used).
        "https://LOCALHOST/",
        "https://0x7f000001/",
        "http://evil.io/",
        "",
    ] {
        assert_eq!(o(outside), None, "{outside}");
    }
}
