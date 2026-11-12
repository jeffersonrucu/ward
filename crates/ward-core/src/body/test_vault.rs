//! Test vault for the body tests (D15 + `acme-totp-dev` from D28 + the special
//! characters API entry from D59): the 22 entries of the contract table, with its
//! levels, and the "full" entries of V2.30. Only fake values.
//!
//! Used by the unit tests (`body/tests.rs`) and by the allocator integration test in
//! `tests/` (V2.32, V2.33), so it only uses the public API, through the alias `wc`
//! that the includer defines. The secrets are born without reallocation (the allocator
//! test looks for the markers in every freed block).

#![allow(dead_code)]

use super::wc::config::{PinHash, VaultConfig};
use super::wc::entry::{AccessLevel, AttachmentDraft, DataDraft, Draft, Entry, ExtraDraft, Secret};
use super::wc::limits::MAX_ATTACHMENTS_TOTAL;

/// Master password of the test vault.
pub const MASTER_PASSWORD: &str = "falso-mestra-ward-01";
/// PIN of the test vault (D15). Never written in cleartext: only its hash.
pub const TEST_PIN: &str = "135790";

/// Configuration of the test vault (D30): hash of the test PIN, lock at 240 minutes and
/// backup disabled. Computed once (the PIN Argon2id costs 64 MiB) and shared by the
/// tests.
pub fn test_config() -> &'static VaultConfig {
    static CFG: std::sync::OnceLock<VaultConfig> = std::sync::OnceLock::new();
    CFG.get_or_init(|| VaultConfig::new(PinHash::create(TEST_PIN.as_bytes()).unwrap()))
}

/// A new configuration, with its own hash (new salt), for whoever needs ownership.
pub fn new_config() -> VaultConfig {
    VaultConfig::new(PinHash::create(TEST_PIN.as_bytes()).unwrap())
}

/// All the secret values of the test vault (the markers the tests look for).
pub const SECRETS: &[&str] = &[
    MASTER_PASSWORD,
    "falso-WP-senha-01",
    "falso-WP-senha-02",
    "JBSWY3DPEHPK3PXP",
    "falso-PG:s3nha/01@#",
    "falso-SSH-senha-01",
    "falso-chave-ed25519-teste",
    "falso-passphrase-01",
    "falso-SALTO-01",
    "falso_token_github_01",
    "falso-aws-secret-01",
    "falso-aws-session-01",
    "falso-SMTP-01",
    "falso-SFTP-01",
    "falso-nota-01",
    "falso-licenca-01",
    "falso-anexo-01",
    "falso-BLOQ-01",
    "falso-REDIR-01",
    "falso-Ab3$k&<9>Zq!'/ç-01",
    "falso-Ab3$k&<9>Zq!'",
];

fn t(v: &str) -> String {
    v.to_owned()
}

fn s(v: &str) -> Option<Secret> {
    Some(Secret::from_text(v.to_owned()))
}

/// Fake private key, built at run time (the whole text is not in the repository) and in
/// a single allocation, of the exact size.
pub fn fake_key() -> String {
    let parts = [
        "-----BEGIN ",
        "OPENSSH",
        " PRIVATE",
        " KEY-----\n",
        "falso-chave-ed25519-teste\n",
        "-----END ",
        "OPENSSH",
        " PRIVATE",
        " KEY-----\n",
    ];
    let mut c = String::with_capacity(parts.iter().map(|p| p.len()).sum());
    for p in parts {
        c.push_str(p);
    }
    c
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

/// Validates and applies the level (as the daemon would after the PIN).
pub fn validated(r: Draft, level: AccessLevel) -> Entry {
    let mut e = match r.validate() {
        Ok(e) => e,
        Err(errors) => panic!("entrada de teste inválida: {errors}"),
    };
    e.set_level(level);
    e
}

fn login(url: &str, username: &str, password: &str, totp: Option<&str>) -> DataDraft {
    DataDraft::Login {
        url: t(url),
        username: t(username),
        password: s(password),
        totp_seed: totp.and_then(s),
    }
}

fn database(host: &str, port: &str, tunnel: &str) -> DataDraft {
    DataDraft::Database {
        engine: t("postgres"),
        host: t(host),
        port: t(port),
        db_name: t("app_teste"),
        username: t("app"),
        password: s("falso-PG:s3nha/01@#"),
        tunnel: t(tunnel),
    }
}

fn server_password(port: &str, username: &str, password: &str) -> DataDraft {
    DataDraft::Server {
        host: t("127.0.0.1"),
        port: t(port),
        username: t(username),
        password: s(password),
        private_key: None,
        passphrase: None,
        jump: String::new(),
    }
}

fn server_key(host: &str, port: &str, jump: &str) -> DataDraft {
    DataDraft::Server {
        host: t(host),
        port: t(port),
        username: t("deploy"),
        password: None,
        private_key: Some(Secret::from_text(fake_key())),
        passphrase: s("falso-passphrase-01"),
        jump: t(jump),
    }
}

/// The 22 entries of the test vault, with the levels of the table (out of name order).
pub fn test_vault_entries() -> Vec<Entry> {
    use AccessLevel::{Approval, Blocked, Free};
    const WP: &str = "http://localhost:8081/wp-login.php";
    let mut generic = draft("gama-generico", "gama", "stage", DataDraft::Generic);
    generic.extras = vec![
        ExtraDraft {
            name: t("REGIAO"),
            value: Secret::from_text(t("r-01")),
            secret: false,
        },
        ExtraDraft {
            name: t("CHAVE_LICENCA"),
            value: Secret::from_text(t("falso-licenca-01")),
            secret: true,
        },
    ];
    generic.attachments = vec![AttachmentDraft {
        name: t("app.env"),
        content: Secret::from_bytes(b"FALSO_ENV=falso-anexo-01\n".to_vec()),
    }];
    // D59: special characters (single quotes, `$&<>`, `/` and outside ASCII).
    let mut symbols = draft(
        "delta-api-simbolos",
        "delta",
        "dev",
        DataDraft::Api {
            url: t("https://api.teste.invalid"),
            token: s("falso-Ab3$k&<9>Zq!'/ç-01"),
            variable: t("SIMBOLOS_TOKEN"),
        },
    );
    symbols.extras = vec![ExtraDraft {
        name: t("SEGREDO_ASCII"),
        value: Secret::from_text(t("falso-Ab3$k&<9>Zq!'")),
        secret: true,
    }];
    let list = vec![
        (
            draft(
                "acme-wp-dev",
                "acme",
                "dev",
                login(
                    WP,
                    "wp_admin",
                    "falso-WP-senha-01",
                    Some("JBSWY3DPEHPK3PXP"),
                ),
            ),
            Approval,
        ),
        (
            draft(
                "acme-wp-prod",
                "acme",
                "prod",
                login(
                    WP,
                    "wp_editor",
                    "falso-WP-senha-02",
                    Some("JBSWY3DPEHPK3PXP"),
                ),
            ),
            Free,
        ),
        (
            draft(
                "acme-pg-dev",
                "acme",
                "dev",
                database("127.0.0.1", "55432", ""),
            ),
            Free,
        ),
        (
            draft(
                "acme-pg-aprovacao",
                "acme",
                "stage",
                database("127.0.0.1", "55432", ""),
            ),
            Approval,
        ),
        (
            draft(
                "acme-pg-prod",
                "acme",
                "prod",
                database("127.0.0.1", "55432", ""),
            ),
            Approval,
        ),
        (
            draft(
                "acme-pg-tunel",
                "acme",
                "stage",
                database("postgres", "5432", "acme-ssh-senha-stage"),
            ),
            Free,
        ),
        (
            draft(
                "acme-ssh-senha-stage",
                "acme",
                "stage",
                server_password("2222", "deploy", "falso-SSH-senha-01"),
            ),
            Free,
        ),
        (
            draft(
                "acme-ssh-chave-prod",
                "acme",
                "prod",
                server_key("127.0.0.1", "2223", ""),
            ),
            Free,
        ),
        (
            draft(
                "acme-ssh-salto",
                "acme",
                "prod",
                server_password("2224", "salto", "falso-SALTO-01"),
            ),
            Approval,
        ),
        (
            draft(
                "acme-ssh-salto-livre",
                "acme",
                "prod",
                server_password("2224", "salto", "falso-SALTO-01"),
            ),
            Free,
        ),
        (
            draft(
                "acme-ssh-interno",
                "acme",
                "prod",
                server_key("ssh-interno", "22", "acme-ssh-salto-livre"),
            ),
            Free,
        ),
        (
            draft(
                "acme-ssh-interno-aprov",
                "acme",
                "prod",
                server_key("ssh-interno", "22", "acme-ssh-salto"),
            ),
            Free,
        ),
        (
            draft(
                "beta-github-api",
                "beta",
                "dev",
                DataDraft::Api {
                    url: t("https://api.github.com"),
                    token: s("falso_token_github_01"),
                    variable: t("GITHUB_TOKEN"),
                },
            ),
            Free,
        ),
        (
            draft(
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
            ),
            Free,
        ),
        (
            draft(
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
            ),
            Free,
        ),
        (
            draft(
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
            ),
            Free,
        ),
        (
            draft(
                "gama-nota",
                "gama",
                "dev",
                DataDraft::SecureNote {
                    text: s("falso-nota-01"),
                },
            ),
            Free,
        ),
        (generic, Free),
        (
            draft(
                "gama-bloqueada",
                "gama",
                "prod",
                login("https://bloqueada.invalid", "x", "falso-BLOQ-01", None),
            ),
            Blocked,
        ),
        (
            draft(
                "delta-redir",
                "delta",
                "dev",
                login("http://localhost:8084/login", "u", "falso-REDIR-01", None),
            ),
            Free,
        ),
        (
            draft(
                "acme-totp-dev",
                "acme",
                "dev",
                login(
                    "http://localhost:8083/login",
                    "wp_admin",
                    "falso-WP-senha-01",
                    Some("JBSWY3DPEHPK3PXP"),
                ),
            ),
            Free,
        ),
        (symbols, Free),
    ];
    list.into_iter()
        .map(|(r, level)| validated(r, level))
        .collect()
}

/// Bytes from 0 to 255 repeated up to `size`.
pub fn bytes_0_to_255(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

/// One entry of each type at the model maximum (V2.30): project, 16 tags, 64 extras
/// (half secret) and two attachments with bytes from 0 to 255 that add up to exactly the
/// D29 limit (256 KiB), within the mask budget; plus an `api` with the 64 KiB token and
/// only the small attachment (both together go over the budget). The fields at the D25
/// limit (2048 URL, 64 KiB token, 1 MiB note) tie the read limits to the validation
/// ones.
pub fn one_of_each_full() -> Vec<Entry> {
    use AccessLevel::{Approval, Blocked, Free};
    let long_url = {
        let base = "https://cheio.invalid/";
        let mut u = String::with_capacity(2048);
        u.push_str(base);
        while u.len() < 2048 {
            u.push('a');
        }
        u
    };
    let entries: Vec<(&str, DataDraft, AccessLevel)> = vec![
        (
            "zz-cheio-login",
            DataDraft::Login {
                url: long_url,
                username: t("Usuário Com Espaço"),
                password: s("falso-cheio-senha"),
                totp_seed: s("JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP"),
            },
            Blocked,
        ),
        (
            "zz-cheio-banco",
            DataDraft::Database {
                engine: t("sqlserver"),
                host: t("[::1]"),
                port: t("1"),
                db_name: t("base_cheia"),
                username: t("sa"),
                password: s("falso-cheio-banco"),
                tunnel: t("zz-cheio-servidor"),
            },
            Approval,
        ),
        (
            "zz-cheio-servidor",
            DataDraft::Server {
                host: t("cheio.invalid"),
                port: t("65535"),
                username: t("root"),
                password: None,
                private_key: Some(Secret::from_text(fake_key())),
                passphrase: None,
                jump: String::new(),
            },
            Free,
        ),
        (
            "zz-cheio-api",
            DataDraft::Api {
                url: String::new(),
                token: Some(Secret::from_bytes(vec![b'k'; 4096])),
                variable: String::new(),
            },
            Free,
        ),
        // The token at the D25 limit (64 KiB) goes over the mask budget together with
        // 256 KiB of attachments (entry and `WARD_TOKEN`, ~1 MiB of patterns each, plus
        // ~2.4 MiB from the attachment); it stays in a second `api`, with the small
        // attachment.
        (
            "zz-cheio-api-token",
            DataDraft::Api {
                url: String::new(),
                token: Some(Secret::from_bytes(vec![b'k'; 64 * 1024])),
                variable: String::new(),
            },
            Free,
        ),
        (
            "zz-cheio-aws",
            DataDraft::Aws {
                access_key_id: t("FALSOACCESSKEY02"),
                secret_access_key: s("falso-cheio-aws"),
                region: t("us-east-1"),
                role_arn: String::new(),
                session_token: None,
            },
            Approval,
        ),
        (
            "zz-cheio-smtp",
            DataDraft::Smtp {
                host: t("smtp.cheio.invalid"),
                port: String::new(),
                username: t("mailer"),
                password: s("falso-cheio-smtp"),
                security: t("tls"),
            },
            Free,
        ),
        (
            "zz-cheio-ftp",
            DataDraft::FtpSftp {
                protocol: t("ftp"),
                host: t("ftp.cheio.invalid"),
                port: String::new(),
                username: t("ftpuser"),
                password: s("falso-cheio-ftp"),
                private_key: None,
                passphrase: None,
            },
            Free,
        ),
        (
            "zz-cheio-nota",
            DataDraft::SecureNote {
                text: Some(Secret::from_bytes(vec![b'n'; 1024 * 1024])),
            },
            Blocked,
        ),
        ("zz-cheio-generico", DataDraft::Generic, Free),
    ];
    entries
        .into_iter()
        .map(|(name, data, level)| {
            let mut r = draft(name, "zeta", "stage", data);
            r.project = t("projeto cheio");
            r.tags = (1..=16).map(|n| format!("tag-{n:02}")).collect();
            r.extras = (1..=64)
                .map(|n| {
                    let secret = n % 2 == 0;
                    let value = if secret {
                        format!("falso-extra-{n:02}")
                    } else {
                        format!("valor-{n:02}")
                    };
                    ExtraDraft {
                        name: format!("extra_{n:02}"),
                        value: Secret::from_text(value),
                        secret,
                    }
                })
                .collect();
            r.attachments = vec![AttachmentDraft {
                name: t("binario.bin"),
                content: Secret::from_bytes(bytes_0_to_255(4096)),
            }];
            if name != "zz-cheio-api-token" {
                r.attachments.push(AttachmentDraft {
                    name: t("grande.bin"),
                    content: Secret::from_bytes(bytes_0_to_255(MAX_ATTACHMENTS_TOTAL - 4096)),
                });
            }
            validated(r, level)
        })
        .collect()
}
