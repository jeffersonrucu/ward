//! Delivery tests (D10): V2.14, V2.15, V2.16 and the attacks on building the environment
//! (URL that changes destination, extra that becomes a system variable, resource that
//! returns junk, secret in `Debug`/error). Only fake values of the test vault (D15).

use super::*;
use crate::entry::{
    AttachmentDraft, COMMON_VARIABLES, DataDraft, Draft, EntryType, ExtraDraft, type_attachments,
    type_variables,
};

// ---- Helpers ----

fn s(v: &str) -> Option<Secret> {
    Some(Secret::from_text(v.to_owned()))
}

fn t(v: &str) -> String {
    v.to_owned()
}

/// Fake private key, built at run time (the whole text does not stay in the repository,
/// so it does not look like a real key to a scan).
fn fake_key() -> String {
    let label = ["OPENSSH", "PRIVATE KEY"].join(" ");
    format!("-----BEGIN {label}-----\nfalso-chave-ed25519-teste\n-----END {label}-----\n")
}

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

fn database(
    engine: &str,
    host: &str,
    port: &str,
    db_name: &str,
    username: &str,
    password: &str,
) -> DataDraft {
    DataDraft::Database {
        engine: t(engine),
        host: t(host),
        port: t(port),
        db_name: t(db_name),
        username: t(username),
        password: s(password),
        tunnel: String::new(),
    }
}

fn valid(r: Draft) -> Entry {
    match r.validate() {
        Ok(e) => e,
        Err(errors) => panic!("devia validar: {errors}"),
    }
}

const FAKE_CODE: &str = "123456";
const TUNNEL_PORT: u16 = 40123;

/// Fake resources: record what was asked, without touching disk or network.
#[derive(Default)]
struct Fakes {
    totp: Vec<Vec<u8>>,
    files: Vec<(String, Vec<u8>)>,
    tunnels: Vec<(String, String, u16)>,
    /// Forced answers (for the attacks).
    code: Option<&'static str>,
    path: Option<&'static str>,
    port: Option<u16>,
    fail: bool,
}

impl Fakes {
    fn calls(&self) -> usize {
        self.totp.len() + self.files.len() + self.tunnels.len()
    }
}

impl Resources for Fakes {
    fn totp_code(&mut self, seed: &Secret) -> Result<Zeroizing<String>, ResourceError> {
        self.totp.push(seed.expose().to_vec());
        if self.fail {
            return Err(ResourceError("relógio indisponível"));
        }
        Ok(Zeroizing::new(t(self.code.unwrap_or(FAKE_CODE))))
    }

    fn file(&mut self, name: &str, content: &Secret) -> Result<String, ResourceError> {
        self.files.push((t(name), content.expose().to_vec()));
        if self.fail {
            return Err(ResourceError("sem espaço em /run/user"));
        }
        Ok(match self.path {
            Some(c) => t(c),
            None => format!("/run/user/1000/ward/tmp/{}-{name}", self.files.len()),
        })
    }

    fn tunnel(&mut self, server: &str, host: &str, port: u16) -> Result<u16, ResourceError> {
        self.tunnels.push((t(server), t(host), port));
        if self.fail {
            return Err(ResourceError("túnel recusado"));
        }
        Ok(self.port.unwrap_or(TUNNEL_PORT))
    }
}

fn build_ok(e: &Entry, r: &mut Fakes) -> Delivery {
    build(e, r).unwrap_or_else(|error| panic!("{e}: {error}"))
}

/// (name, value) pairs of the delivery, in order.
fn pairs(e: &Delivery) -> Vec<(String, String)> {
    e.variables()
        .iter()
        .map(|v| (t(v.name()), t(v.expose())))
        .collect()
}

/// Checks names and values exactly (order does not matter; nothing more, nothing less).
fn check(e: &Delivery, expected: &[(&str, &str)], context: &str) {
    let mut got = pairs(e);
    got.sort();
    let mut expected: Vec<(String, String)> = expected.iter().map(|(n, v)| (t(n), t(v))).collect();
    expected.sort();
    assert_eq!(got, expected, "{context}");
}

/// Secrets used in the tests, for the leak tests.
const SECRETS: &[&str] = &[
    "falso-WP-senha-02",
    "JBSWY3DPEHPK3PXP",
    "falso-PG:s3nha/01@#",
    "falso-PG%3As3nha%2F01%40%23",
    "falso-SSH-senha-01",
    "falso-chave-ed25519-teste",
    "falso-passphrase-01",
    "falso_token_github_01",
    "falso-aws-secret-01",
    "falso-aws-session-01",
    "falso-SMTP-01",
    "falso-SFTP-01",
    "falso-nota-01",
    "falso-licenca-01",
    "falso-anexo-01",
    "falso:DB/01",
    "falso%3ADB%2F01",
];

fn no_secret(text: &str, context: &str) {
    for secret in SECRETS {
        assert!(!text.contains(secret), "{context} contém segredo");
    }
}

// ---- The entries of the "expected delivery" table ----

fn wp_prod() -> Entry {
    valid(draft(
        "acme-wp-prod",
        "acme",
        "prod",
        DataDraft::Login {
            url: t("http://localhost:8081/wp-login.php"),
            username: t("wp_editor"),
            password: s("falso-WP-senha-02"),
            totp_seed: s("JBSWY3DPEHPK3PXP"),
        },
    ))
}

fn pg_dev() -> Entry {
    valid(draft(
        "acme-pg-dev",
        "acme",
        "dev",
        database(
            "postgres",
            "127.0.0.1",
            "55432",
            "app_teste",
            "app",
            "falso-PG:s3nha/01@#",
        ),
    ))
}

fn pg_tunnel() -> Entry {
    let mut data = database(
        "postgres",
        "postgres",
        "5432",
        "app_teste",
        "app",
        "falso-PG:s3nha/01@#",
    );
    if let DataDraft::Database { tunnel, .. } = &mut data {
        *tunnel = t("acme-ssh-senha-stage");
    }
    valid(draft("acme-pg-tunel", "acme", "stage", data))
}

fn ssh_password() -> Entry {
    valid(draft(
        "acme-ssh-senha-stage",
        "acme",
        "stage",
        DataDraft::Server {
            host: t("127.0.0.1"),
            port: t("2222"),
            username: t("deploy"),
            password: s("falso-SSH-senha-01"),
            private_key: None,
            passphrase: None,
            jump: String::new(),
        },
    ))
}

fn ssh_key() -> Entry {
    valid(draft(
        "acme-ssh-chave-prod",
        "acme",
        "prod",
        DataDraft::Server {
            host: t("127.0.0.1"),
            port: t("2223"),
            username: t("deploy"),
            password: None,
            private_key: Some(Secret::from_text(fake_key())),
            passphrase: s("falso-passphrase-01"),
            jump: String::new(),
        },
    ))
}

fn github() -> Entry {
    valid(draft(
        "beta-github-api",
        "beta",
        "dev",
        DataDraft::Api {
            url: t("https://api.github.com"),
            token: s("falso_token_github_01"),
            variable: t("GITHUB_TOKEN"),
        },
    ))
}

fn aws() -> Entry {
    valid(draft(
        "beta-aws-dev",
        "beta",
        "dev",
        DataDraft::Aws {
            access_key_id: t("FALSOACCESSKEY01"),
            secret_access_key: s("falso-aws-secret-01"),
            region: t("sa-east-1"),
            role_arn: t("arn:aws:iam::000000000000:role/teste"),
            session_token: s("falso-aws-session-01"),
        },
    ))
}

fn smtp() -> Entry {
    valid(draft(
        "beta-smtp-stage",
        "beta",
        "stage",
        DataDraft::Smtp {
            host: t("smtp.teste.invalid"),
            port: t("587"),
            username: t("mailer"),
            password: s("falso-SMTP-01"),
            security: t("starttls"),
        },
    ))
}

fn sftp() -> Entry {
    valid(draft(
        "beta-sftp-dev",
        "beta",
        "dev",
        DataDraft::FtpSftp {
            protocol: t("sftp"),
            host: t("127.0.0.1"),
            port: t("2222"),
            username: t("sftpuser"),
            password: s("falso-SFTP-01"),
            private_key: None,
            passphrase: None,
        },
    ))
}

fn note() -> Entry {
    valid(draft(
        "gama-nota",
        "gama",
        "dev",
        DataDraft::SecureNote {
            text: s("falso-nota-01"),
        },
    ))
}

fn generic() -> Entry {
    let mut r = draft("gama-generico", "gama", "stage", DataDraft::Generic);
    r.extras = vec![
        extra("REGIAO", "r-01", false),
        extra("CHAVE_LICENCA", "falso-licenca-01", true),
    ];
    r.attachments = vec![attachment("app.env", b"FALSO_ENV=falso-anexo-01\n")];
    valid(r)
}

/// One entry of each type with a delivery (the note left out).
fn all_with_delivery() -> Vec<Entry> {
    vec![
        wp_prod(),
        pg_dev(),
        pg_tunnel(),
        ssh_password(),
        ssh_key(),
        github(),
        aws(),
        smtp(),
        sftp(),
        generic(),
    ]
}

// ---- V2.14 ----

#[test]
fn v2_14_expected_delivery_of_the_contract() {
    let mut r = Fakes::default();
    let e = build_ok(&wp_prod(), &mut r);
    check(
        &e,
        &[
            ("WARD_ENTRY", "acme-wp-prod"),
            ("WARD_GROUP", "acme"),
            ("WARD_ENVIRONMENT", "prod"),
            ("WARD_URL", "http://localhost:8081/wp-login.php"),
            ("WARD_USER", "wp_editor"),
            ("WARD_PASS", "falso-WP-senha-02"),
            ("WARD_TOTP", FAKE_CODE),
        ],
        "acme-wp-prod",
    );
    // The code comes from the right seed; the seed never becomes a variable.
    assert_eq!(r.totp, vec![b"JBSWY3DPEHPK3PXP".to_vec()]);
    assert!(
        !pairs(&e)
            .iter()
            .any(|(_, v)| v.contains("JBSWY3DPEHPK3PXP"))
    );

    let mut r = Fakes::default();
    let e = build_ok(&pg_dev(), &mut r);
    check(
        &e,
        &[
            ("WARD_ENTRY", "acme-pg-dev"),
            ("WARD_GROUP", "acme"),
            ("WARD_ENVIRONMENT", "dev"),
            ("PGHOST", "127.0.0.1"),
            ("PGPORT", "55432"),
            ("PGDATABASE", "app_teste"),
            ("PGUSER", "app"),
            ("PGPASSWORD", "falso-PG:s3nha/01@#"),
            (
                "DATABASE_URL",
                "postgresql://app:falso-PG%3As3nha%2F01%40%23@127.0.0.1:55432/app_teste",
            ),
        ],
        "acme-pg-dev",
    );
    assert_eq!(r.calls(), 0, "banco sem túnel não pede recurso");

    let mut r = Fakes::default();
    let e = build_ok(&pg_tunnel(), &mut r);
    let port = TUNNEL_PORT.to_string();
    let url = format!("postgresql://app:falso-PG%3As3nha%2F01%40%23@127.0.0.1:{port}/app_teste");
    check(
        &e,
        &[
            ("WARD_ENTRY", "acme-pg-tunel"),
            ("WARD_GROUP", "acme"),
            ("WARD_ENVIRONMENT", "stage"),
            ("PGHOST", "127.0.0.1"),
            ("PGPORT", &port),
            ("PGDATABASE", "app_teste"),
            ("PGUSER", "app"),
            ("PGPASSWORD", "falso-PG:s3nha/01@#"),
            ("DATABASE_URL", &url),
        ],
        "acme-pg-tunel",
    );
    assert_ne!(TUNNEL_PORT, 5432);
    assert_eq!(
        r.tunnels,
        vec![(t("acme-ssh-senha-stage"), t("postgres"), 5432)],
        "o túnel vai pelo servidor referenciado até host:porta do banco"
    );

    let e = build_ok(&ssh_password(), &mut Fakes::default());
    check(
        &e,
        &[
            ("WARD_ENTRY", "acme-ssh-senha-stage"),
            ("WARD_GROUP", "acme"),
            ("WARD_ENVIRONMENT", "stage"),
            ("WARD_HOST", "127.0.0.1"),
            ("WARD_PORT", "2222"),
            ("WARD_USER", "deploy"),
            ("WARD_PASS", "falso-SSH-senha-01"),
        ],
        "acme-ssh-senha-stage",
    );

    let mut r = Fakes::default();
    let e = build_ok(&ssh_key(), &mut r);
    check(
        &e,
        &[
            ("WARD_ENTRY", "acme-ssh-chave-prod"),
            ("WARD_GROUP", "acme"),
            ("WARD_ENVIRONMENT", "prod"),
            ("WARD_HOST", "127.0.0.1"),
            ("WARD_PORT", "2223"),
            ("WARD_USER", "deploy"),
            ("WARD_FILE_KEY", "/run/user/1000/ward/tmp/1-chave"),
            ("WARD_PASSPHRASE", "falso-passphrase-01"),
        ],
        "acme-ssh-chave-prod",
    );
    assert_eq!(r.files, vec![(t("chave"), fake_key().into_bytes())]);
    assert!(
        !pairs(&e)
            .iter()
            .any(|(_, v)| v.contains("falso-chave-ed25519-teste")),
        "a chave vai em arquivo, nunca no ambiente"
    );

    let e = build_ok(&github(), &mut Fakes::default());
    check(
        &e,
        &[
            ("WARD_ENTRY", "beta-github-api"),
            ("WARD_GROUP", "beta"),
            ("WARD_ENVIRONMENT", "dev"),
            ("WARD_URL", "https://api.github.com"),
            ("WARD_TOKEN", "falso_token_github_01"),
            ("GITHUB_TOKEN", "falso_token_github_01"),
        ],
        "beta-github-api",
    );

    let e = build_ok(&aws(), &mut Fakes::default());
    check(
        &e,
        &[
            ("WARD_ENTRY", "beta-aws-dev"),
            ("WARD_GROUP", "beta"),
            ("WARD_ENVIRONMENT", "dev"),
            ("AWS_ACCESS_KEY_ID", "FALSOACCESSKEY01"),
            ("AWS_SECRET_ACCESS_KEY", "falso-aws-secret-01"),
            ("AWS_SESSION_TOKEN", "falso-aws-session-01"),
            ("AWS_REGION", "sa-east-1"),
            ("AWS_DEFAULT_REGION", "sa-east-1"),
            ("AWS_ROLE_ARN", "arn:aws:iam::000000000000:role/teste"),
        ],
        "beta-aws-dev",
    );

    let e = build_ok(&smtp(), &mut Fakes::default());
    check(
        &e,
        &[
            ("WARD_ENTRY", "beta-smtp-stage"),
            ("WARD_GROUP", "beta"),
            ("WARD_ENVIRONMENT", "stage"),
            ("SMTP_HOST", "smtp.teste.invalid"),
            ("SMTP_PORT", "587"),
            ("SMTP_USER", "mailer"),
            ("SMTP_PASS", "falso-SMTP-01"),
            ("SMTP_SECURITY", "starttls"),
        ],
        "beta-smtp-stage",
    );

    let e = build_ok(&sftp(), &mut Fakes::default());
    check(
        &e,
        &[
            ("WARD_ENTRY", "beta-sftp-dev"),
            ("WARD_GROUP", "beta"),
            ("WARD_ENVIRONMENT", "dev"),
            ("WARD_PROTOCOL", "sftp"),
            ("WARD_HOST", "127.0.0.1"),
            ("WARD_PORT", "2222"),
            ("WARD_USER", "sftpuser"),
            ("WARD_PASS", "falso-SFTP-01"),
        ],
        "beta-sftp-dev",
    );

    let mut r = Fakes::default();
    let e = build_ok(&generic(), &mut r);
    check(
        &e,
        &[
            ("WARD_ENTRY", "gama-generico"),
            ("WARD_GROUP", "gama"),
            ("WARD_ENVIRONMENT", "stage"),
            ("WARD_REGIAO", "r-01"),
            ("WARD_CHAVE_LICENCA", "falso-licenca-01"),
            ("WARD_FILE_APP_ENV", "/run/user/1000/ward/tmp/1-app.env"),
        ],
        "gama-generico",
    );
    assert_eq!(
        r.files,
        vec![(t("app.env"), b"FALSO_ENV=falso-anexo-01\n".to_vec())]
    );

    let mut r = Fakes::default();
    let error = build(&note(), &mut r).expect_err("nota não tem entrega");
    assert_eq!(error, DeliveryError::TypeWithoutDelivery);
    assert_eq!(error.to_string(), "tipo sem entrega");
    assert_eq!(r.calls(), 0, "a recusa vem antes de qualquer recurso");
}

#[test]
fn v2_14_common_ones_come_first_and_in_order() {
    for e in all_with_delivery() {
        let delivery = build_ok(&e, &mut Fakes::default());
        let names: Vec<&str> = delivery.names().take(3).collect();
        assert_eq!(
            names,
            ["WARD_ENTRY", "WARD_GROUP", "WARD_ENVIRONMENT"],
            "{e}"
        );
        assert_eq!(delivery.value("WARD_ENTRY"), Some(e.name()));
        assert_eq!(delivery.value("WARD_GROUP"), Some(e.group()));
        assert_eq!(
            delivery.value("WARD_ENVIRONMENT"),
            Some(e.environment().name())
        );
    }
}

/// m2:V4.19 (m2:DT11): no delivery of the test vault carries a 0.1.x variable name (the
/// last row of the contract's expected delivery table).
#[test]
fn m2_v4_19_no_old_variable_name() {
    let old = [
        "WARD_ENTRADA",
        "WARD_GRUPO",
        "WARD_AMBIENTE",
        "WARD_PROTOCOLO",
    ];
    for e in all_with_delivery() {
        let delivery = build_ok(&e, &mut Fakes::default());
        for name in delivery.names() {
            assert!(!old.contains(&name), "{name} em {e}");
            assert!(!name.starts_with("WARD_ARQUIVO_"), "{name} em {e}");
        }
    }
    // The key and the attachments go in `WARD_FILE_*`.
    let delivery = build_ok(&ssh_key(), &mut Fakes::default());
    assert!(delivery.value("WARD_FILE_KEY").is_some());
    let delivery = build_ok(&sftp(), &mut Fakes::default());
    assert_eq!(delivery.value("WARD_PROTOCOL"), Some("sftp"));
}

/// m2:DT4/DT11 (finding of Phase 48): `SMTP_SECURITY` carries the v5 name of the
/// security (`none`, `starttls`, `tls`); 0.1.x delivered the pt-BR name of `none`.
#[test]
fn m2_smtp_security_with_the_v5_name() {
    for (security, expected) in [("none", "none"), ("starttls", "starttls"), ("tls", "tls")] {
        let e = valid(draft(
            "beta-smtp-stage",
            "beta",
            "stage",
            DataDraft::Smtp {
                host: t("smtp.teste.invalid"),
                port: t("587"),
                username: t("mailer"),
                password: s("falso-SMTP-01"),
                security: t(security),
            },
        ));
        let delivery = build_ok(&e, &mut Fakes::default());
        assert_eq!(
            delivery.value("SMTP_SECURITY"),
            Some(expected),
            "{security}"
        );
    }
}

#[test]
fn missing_optionals_stay_out() {
    // Login without a seed: no TOTP variable and no call to the TOTP.
    let r = draft(
        "x",
        "acme",
        "dev",
        DataDraft::Login {
            url: t("https://painel.teste.invalid"),
            username: t("u"),
            password: s("falso-WP-senha-02"),
            totp_seed: None,
        },
    );
    let mut f = Fakes::default();
    let e = build_ok(&valid(r), &mut f);
    assert!(e.value("WARD_TOTP").is_none());
    assert_eq!(f.calls(), 0);

    // AWS without a session token and without a role.
    let r = draft(
        "y",
        "beta",
        "dev",
        DataDraft::Aws {
            access_key_id: t("FALSOACCESSKEY01"),
            secret_access_key: s("falso-aws-secret-01"),
            region: t("sa-east-1"),
            role_arn: String::new(),
            session_token: None,
        },
    );
    let e = build_ok(&valid(r), &mut Fakes::default());
    assert!(e.value("AWS_SESSION_TOKEN").is_none());
    assert!(e.value("AWS_ROLE_ARN").is_none());

    // API without a URL and without a variable: only the token variable.
    let r = draft(
        "z",
        "beta",
        "dev",
        DataDraft::Api {
            url: String::new(),
            token: s("falso_token_github_01"),
            variable: String::new(),
        },
    );
    let e = build_ok(&valid(r), &mut Fakes::default());
    let names: Vec<&str> = e.names().skip(3).collect();
    assert_eq!(names, ["WARD_TOKEN"]);

    // Postgres database without a database name: no PGDATABASE and a URL with no path.
    let e = build_ok(
        &valid(draft(
            "w",
            "acme",
            "dev",
            database(
                "postgres",
                "127.0.0.1",
                "55432",
                "",
                "app",
                "falso-PG:s3nha/01@#",
            ),
        )),
        &mut Fakes::default(),
    );
    assert!(e.value("PGDATABASE").is_none());
    assert_eq!(
        e.value("DATABASE_URL"),
        Some("postgresql://app:falso-PG%3As3nha%2F01%40%23@127.0.0.1:55432")
    );
}

// ---- V2.15 ----

fn database_v2_15(engine: &str) -> Entry {
    // In redis the database name is a number only (D33); here it has none.
    let db_name = if engine == "redis" { "" } else { "b" };
    valid(draft(
        "motor-teste",
        "acme",
        "dev",
        database(engine, "db.teste.invalid", "", db_name, "u", "falso:DB/01"),
    ))
}

#[test]
fn v2_15_all_engines() {
    let common = [
        ("WARD_ENTRY", "motor-teste"),
        ("WARD_GROUP", "acme"),
        ("WARD_ENVIRONMENT", "dev"),
    ];
    let cases: [(&str, Vec<(&str, &str)>); 5] = [
        (
            "postgres",
            vec![
                ("PGHOST", "db.teste.invalid"),
                ("PGPORT", "5432"),
                ("PGDATABASE", "b"),
                ("PGUSER", "u"),
                ("PGPASSWORD", "falso:DB/01"),
                (
                    "DATABASE_URL",
                    "postgresql://u:falso%3ADB%2F01@db.teste.invalid:5432/b",
                ),
            ],
        ),
        (
            "mysql",
            vec![
                ("MYSQL_HOST", "db.teste.invalid"),
                ("MYSQL_TCP_PORT", "3306"),
                ("MYSQL_USER", "u"),
                ("MYSQL_PWD", "falso:DB/01"),
                ("MYSQL_DATABASE", "b"),
                (
                    "DATABASE_URL",
                    "mysql://u:falso%3ADB%2F01@db.teste.invalid:3306/b",
                ),
            ],
        ),
        (
            "redis",
            vec![
                ("REDIS_HOST", "db.teste.invalid"),
                ("REDIS_PORT", "6379"),
                ("REDISCLI_AUTH", "falso:DB/01"),
                (
                    "REDIS_URL",
                    "redis://u:falso%3ADB%2F01@db.teste.invalid:6379",
                ),
                (
                    "DATABASE_URL",
                    "redis://u:falso%3ADB%2F01@db.teste.invalid:6379",
                ),
            ],
        ),
        (
            "mongo",
            vec![
                (
                    "MONGODB_URI",
                    "mongodb://u:falso%3ADB%2F01@db.teste.invalid:27017/b",
                ),
                (
                    "DATABASE_URL",
                    "mongodb://u:falso%3ADB%2F01@db.teste.invalid:27017/b",
                ),
            ],
        ),
        (
            "sqlserver",
            vec![
                ("SQLCMDSERVER", "db.teste.invalid,1433"),
                ("SQLCMDUSER", "u"),
                ("SQLCMDPASSWORD", "falso:DB/01"),
                ("SQLCMDDBNAME", "b"),
                (
                    "DATABASE_URL",
                    "sqlserver://u:falso%3ADB%2F01@db.teste.invalid:1433?database=b",
                ),
            ],
        ),
    ];
    for (engine, specific) in cases {
        let e = build_ok(&database_v2_15(engine), &mut Fakes::default());
        let mut expected = common.to_vec();
        expected.extend(specific);
        check(&e, &expected, engine);
    }
}

#[test]
fn redis_with_numeric_db_name_goes_in_the_url() {
    let e = valid(draft(
        "r",
        "acme",
        "dev",
        database("redis", "db.teste.invalid", "", "3", "u", "falso:DB/01"),
    ));
    let e = build_ok(&e, &mut Fakes::default());
    assert_eq!(
        e.value("REDIS_URL"),
        Some("redis://u:falso%3ADB%2F01@db.teste.invalid:6379/3")
    );
}

#[test]
fn ipv6_host_goes_in_brackets_in_the_url() {
    let e = valid(draft(
        "v6",
        "acme",
        "dev",
        database("postgres", "::1", "", "b", "u", "falso:DB/01"),
    ));
    let e = build_ok(&e, &mut Fakes::default());
    assert_eq!(e.value("PGHOST"), Some("::1"));
    assert_eq!(
        e.value("DATABASE_URL"),
        Some("postgresql://u:falso%3ADB%2F01@[::1]:5432/b")
    );
}

#[test]
fn percent_encoding_of_all_bytes() {
    let mut s = String::new();
    encode_into(&mut s, "aZ09-._~ :/?#[]@!$&'()*+,;=%\u{e9}");
    assert_eq!(
        s,
        "aZ09-._~%20%3A%2F%3F%23%5B%5D%40%21%24%26%27%28%29%2A%2B%2C%3B%3D%25%C3%A9"
    );
}

// ---- V2.16 ----

#[test]
fn v2_16_extra_and_attachment_normalized() {
    let mut r = draft("g", "gama", "dev", DataDraft::Generic);
    r.extras = vec![extra("api-key", "falso-licenca-01", true)];
    r.attachments = vec![attachment("chave acesso.pem", b"falso-anexo-01\n")];
    let mut f = Fakes::default();
    let e = build_ok(&valid(r), &mut f);
    assert_eq!(e.value("WARD_API_KEY"), Some("falso-licenca-01"));
    assert_eq!(
        e.value("WARD_FILE_CHAVE_ACESSO_PEM"),
        Some("/run/user/1000/ward/tmp/1-chave acesso.pem")
    );
    assert_eq!(
        f.files,
        vec![(t("chave acesso.pem"), b"falso-anexo-01\n".to_vec())]
    );

    // Two extras that normalize to the same name: refused on save.
    let mut r = draft("g", "gama", "dev", DataDraft::Generic);
    r.extras = vec![extra("api-key", "a", false), extra("API KEY", "b", false)];
    assert!(
        r.validate().is_err(),
        "api-key e API KEY viram WARD_API_KEY"
    );
    let mut r = draft("g", "gama", "dev", DataDraft::Generic);
    r.extras = vec![extra("x", "a", false)];
    r.attachments = vec![attachment("a.pem", b"1"), attachment("a-pem", b"2")];
    assert!(r.validate().is_err(), "a.pem e a-pem viram WARD_FILE_A_PEM");
}

#[test]
fn extras_go_in_every_type_with_a_delivery() {
    let mut r = draft(
        "pg",
        "acme",
        "dev",
        database("postgres", "127.0.0.1", "", "b", "u", "falso:DB/01"),
    );
    r.extras = vec![extra("sslmode", "require", false)];
    r.attachments = vec![attachment("ca.pem", b"falso-anexo-01")];
    let e = build_ok(&valid(r), &mut Fakes::default());
    let names: Vec<&str> = e.names().collect();
    // Common ones, then the type, then extras and attachments.
    assert_eq!(names.last(), Some(&"WARD_FILE_CA_PEM"));
    assert_eq!(names[names.len() - 2], "WARD_SSLMODE");
}

// ---- Consistency with the validation (Phase 15) ----

/// The `WARD_*` variables that each type builds are exactly the ones the validation
/// reserves (and so an extra or attachment never overwrites them).
#[test]
fn type_variables_match_the_reserved_ones() {
    let mut seen = Vec::new();
    for e in all_with_delivery() {
        let kind = e.kind();
        seen.push(kind);
        let delivery = build_ok(&e, &mut Fakes::default());
        let extras: Vec<String> = e
            .extras()
            .iter()
            .map(|x| format!("WARD_{}", variable_name(x.name())))
            .chain(
                e.attachments()
                    .iter()
                    .map(|a| format!("WARD_FILE_{}", variable_name(a.name()))),
            )
            .collect();
        for name in delivery.names() {
            if extras.iter().any(|x| x == name) {
                continue;
            }
            let Some(suffix) = name.strip_prefix("WARD_") else {
                continue;
            };
            if let Some(file) = suffix.strip_prefix("FILE_") {
                assert!(
                    type_attachments(kind).contains(&file),
                    "{name} de {kind} não está em anexos_do_tipo"
                );
            } else {
                assert!(
                    COMMON_VARIABLES.contains(&suffix) || type_variables(kind).contains(&suffix),
                    "{name} de {kind} não está em variaveis_do_tipo"
                );
            }
        }
    }
    // Every type with a delivery went through here (the note is refused separately).
    for kind in EntryType::ALL {
        if kind != EntryType::SecureNote {
            assert!(seen.contains(&kind), "falta {kind}");
        }
    }
}

#[test]
fn secret_mark_matches_the_value() {
    for e in all_with_delivery() {
        let delivery = build_ok(&e, &mut Fakes::default());
        for v in delivery.variables() {
            let has_secret = SECRETS.iter().any(|s| v.expose().contains(s));
            assert_eq!(
                v.is_secret(),
                has_secret,
                "{} de {e}: marca de segredo errada",
                v.name()
            );
        }
    }
}

// ---- Attacks ----

/// Password and user with URL characters do not change host, port nor client options
/// (e.g. libpq's `?host=` or mongo's `authSource`). A database name with those characters
/// does not even reach the delivery: the validation refuses it (D33); its percent-encoding
/// in the URL stays as an extra defense.
#[test]
fn attack_password_user_or_db_name_change_the_url_destination() {
    let password = "x@evil.invalid:1/?#&host=evil";
    let username = "u@evil.invalid:2";
    let hostile = "app?host=evil.invalid&sslmode=disable#f/../x";
    for engine in ["postgres", "mysql", "redis", "mongo", "sqlserver"] {
        let r = draft(
            "ataque",
            "acme",
            "dev",
            database(engine, "db.teste.invalid", "", hostile, username, password),
        );
        assert!(
            r.validate().is_err_and(|e| e.has("base")),
            "{engine}: base hostil aceita"
        );
        let db_name = if engine == "redis" {
            "7"
        } else {
            "app_teste-1"
        };
        let e = valid(draft(
            "ataque",
            "acme",
            "dev",
            database(engine, "db.teste.invalid", "", db_name, username, password),
        ));
        let delivery = build_ok(&e, &mut Fakes::default());
        for v in delivery.variables() {
            if !v.name().ends_with("_URL") && v.name() != "MONGODB_URI" {
                continue;
            }
            let url = v.expose();
            let (_, rest) = url.split_once("://").expect("esquema");
            // A single '@' (the separator) and, after it, the real host and port.
            assert_eq!(rest.matches('@').count(), 1, "{engine} {}", v.name());
            let (credentials, destination) = rest.split_once('@').expect("@");
            assert_eq!(credentials.matches(':').count(), 1, "{engine}");
            let port = Engine::from_name(engine).expect("motor").default_port();
            assert!(
                destination.starts_with(&format!("db.teste.invalid:{port}")),
                "{engine}: destino mudou"
            );
            let after = &destination[format!("db.teste.invalid:{port}").len()..];
            // Only the sqlserver '?database='; no other option, fragment or '/'.
            let after = after.strip_prefix("?database=").unwrap_or(after);
            let after = after.strip_prefix('/').unwrap_or(after);
            assert!(
                !after.contains(['?', '#', '&', '/', '=']),
                "{engine}: base virou opção ({after})"
            );
        }
    }
}

#[test]
fn attack_extra_becomes_system_variable() {
    let mut r = draft(
        "pg",
        "acme",
        "dev",
        database("postgres", "127.0.0.1", "", "b", "u", "falso:DB/01"),
    );
    r.extras = vec![
        extra("LD_PRELOAD", "/tmp/evil.so", false),
        extra("PATH", "/tmp/evil", false),
        extra("PGPASSWORD", "outra", false),
        extra("DATABASE_URL", "postgresql://evil.invalid", false),
    ];
    r.attachments = vec![attachment("LD_AUDIT", b"x")];
    let e = build_ok(&valid(r), &mut Fakes::default());
    for name in e.names() {
        assert!(!name.starts_with("LD_"), "{name}");
        assert_ne!(name, "PATH");
    }
    assert_eq!(e.value("PGPASSWORD"), Some("falso:DB/01"));
    assert_eq!(
        e.value("DATABASE_URL"),
        Some("postgresql://u:falso%3ADB%2F01@127.0.0.1:5432/b")
    );
    assert_eq!(e.value("WARD_LD_PRELOAD"), Some("/tmp/evil.so"));
    assert!(e.value("WARD_FILE_LD_AUDIT").is_some());
}

#[test]
fn attack_resource_returns_junk() {
    // TOTP code out of format (line break, letters, long, empty, null).
    for code in [
        "12345\n",
        "abcdef",
        "123456789",
        "",
        "12345",
        "1234\u{0}56",
        "12 345",
    ] {
        let mut f = Fakes {
            code: Some(code),
            ..Fakes::default()
        };
        let error = build(&wp_prod(), &mut f).expect_err("código inválido");
        assert!(
            matches!(error, DeliveryError::InvalidResource(_)),
            "{code:?}"
        );
    }
    // Empty path or path with a null.
    for path in ["", "/run/user/1000/ward/tmp/x\u{0}y"] {
        let mut f = Fakes {
            path: Some(path),
            ..Fakes::default()
        };
        assert!(build(&generic(), &mut f).is_err(), "{path:?}");
        assert!(build(&ssh_key(), &mut f).is_err(), "{path:?}");
    }
    // Tunnel port 0.
    let mut f = Fakes {
        port: Some(0),
        ..Fakes::default()
    };
    assert!(matches!(
        build(&pg_tunnel(), &mut f),
        Err(DeliveryError::InvalidResource(_))
    ));
    // Resource that fails: the error goes up, with no secret.
    let mut f = Fakes {
        fail: true,
        ..Fakes::default()
    };
    for e in [wp_prod(), pg_tunnel(), ssh_key(), generic()] {
        let error = build(&e, &mut f).expect_err("recurso falhou");
        assert!(matches!(error, DeliveryError::Resource(_)), "{e}");
        no_secret(&format!("{error} {error:?}"), "erro de recurso");
    }
}

#[test]
fn attack_debug_and_errors_without_secret() {
    for e in all_with_delivery() {
        let delivery = build_ok(&e, &mut Fakes::default());
        for text in [format!("{delivery:?}"), format!("{delivery:#?}")] {
            no_secret(&text, "Debug da entrega");
            // Not even non-secret values show.
            assert!(!text.contains("127.0.0.1"), "Debug com valor");
        }
        for v in delivery.variables() {
            no_secret(&format!("{v:?}"), "Debug da variável");
        }
    }
    let errors = [
        DeliveryError::TypeWithoutDelivery,
        DeliveryError::Resource(ResourceError("x")),
        DeliveryError::InvalidResource("y"),
        DeliveryError::Invalid("z"),
    ];
    for error in errors {
        no_secret(&format!("{error} {error:?}"), "erro de entrega");
    }
}

#[test]
fn defense_check_refuses_invalid_list() {
    let mut m = Builder::default();
    m.text("WARD_X", "a");
    m.text("WARD_X", "b");
    assert_eq!(m.check(), Err(DeliveryError::Invalid("variável repetida")));

    for name in [
        "",
        "1A",
        "a",
        "WARD-X",
        "WARD X",
        "LD_PRELOAD",
        "WARD_\u{e9}",
    ] {
        let mut m = Builder::default();
        m.text(name, "v");
        assert!(m.check().is_err(), "{name:?}");
    }
    let mut m = Builder::default();
    m.text("WARD_X", "a\u{0}b");
    assert_eq!(
        m.check(),
        Err(DeliveryError::Invalid("valor com byte nulo"))
    );
}

// ---- S8 / V2.28: zeroes on drop, no Clone ----

/// Fails the build if `T` implements `Clone` (two impls become ambiguous).
trait NotClone<A> {
    fn check() {}
}
impl<T> NotClone<()> for T {}
#[allow(dead_code)]
struct ImplementsClone;
impl<T: Clone> NotClone<ImplementsClone> for T {}

fn zeroes_on_drop<T: ZeroizeOnDrop>() {}

#[test]
fn delivery_zeroes_on_drop_and_does_not_clone() {
    <Delivery as NotClone<_>>::check();
    <Variable as NotClone<_>>::check();
    zeroes_on_drop::<Delivery>();
    zeroes_on_drop::<Variable>();
    zeroes_on_drop::<Zeroizing<String>>();
}

#[test]
fn url_allocated_once_at_the_exact_size() {
    // With no reallocation, no partial copy of the password is left in freed memory.
    let parts = [
        ("postgresql", false),
        ("://", false),
        ("app", true),
        (":", false),
        ("falso-PG:s3nha/01@#", true),
        ("@", false),
        ("127.0.0.1:55432", false),
    ];
    let url = join_parts(&parts);
    assert_eq!(url.len(), url.capacity());
}

// ---- V2.37: the database name comes out the same (D33) ----

#[test]
fn v2_37_valid_db_name_comes_out_the_same() {
    let pg = valid(draft(
        "pg",
        "acme",
        "dev",
        database(
            "postgres",
            "db.teste.invalid",
            "",
            "acme_db-1",
            "u",
            "falso:DB/01",
        ),
    ));
    let g = build_ok(&pg, &mut Fakes::default());
    assert_eq!(g.value("PGDATABASE"), Some("acme_db-1"));
    assert_eq!(
        g.value("DATABASE_URL"),
        Some("postgresql://u:falso%3ADB%2F01@db.teste.invalid:5432/acme_db-1")
    );
    let redis = valid(draft(
        "r",
        "acme",
        "dev",
        database("redis", "db.teste.invalid", "", "3", "u", "falso:DB/01"),
    ));
    let g = build_ok(&redis, &mut Fakes::default());
    for name in ["REDIS_URL", "DATABASE_URL"] {
        assert!(
            g.value(name).is_some_and(|u| u.ends_with(":6379/3")),
            "{name}"
        );
    }
}

/// V2.48 (D42): the variables accepted in the API type come out in `ward run` with the
/// token, besides `WARD_TOKEN`; and the rest of the environment does not change.
#[test]
fn v2_48_accepted_variable_comes_out_with_the_token() {
    for var in [
        "GITHUB_TOKEN",
        "GH_TOKEN",
        "NPM_TOKEN",
        "OPENAI_API_KEY",
        "STRIPE_SECRET_KEY",
        "DOCKER_PASSWORD",
        "NPM_CONFIG__AUTH",
    ] {
        let e = valid(draft(
            "beta-github-api",
            "beta",
            "dev",
            DataDraft::Api {
                url: t("https://api.github.com"),
                token: s("falso_token_github_01"),
                variable: t(var),
            },
        ));
        let delivery = build_ok(&e, &mut Fakes::default());
        assert_eq!(delivery.value(var), Some("falso_token_github_01"), "{var}");
        assert_eq!(delivery.value("WARD_TOKEN"), Some("falso_token_github_01"));
        let secret = delivery
            .variables()
            .iter()
            .find(|v| v.name() == var)
            .map(|v| v.is_secret());
        assert_eq!(secret, Some(true), "{var}: vai para a máscara");
        no_secret(&format!("{delivery:?}"), var);
    }
}
