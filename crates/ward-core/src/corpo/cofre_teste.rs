//! Cofre de teste para os testes do corpo (D15 + `acme-totp-dev` de D28 +
//! `delta-api-simbolos` de D59): as 22 entradas
//! da tabela do contrato, com os níveis dela, e as entradas "cheias" do V2.30. Só valores
//! falsos.
//!
//! Usado pelos testes de unidade (`corpo/testes.rs`) e pelo teste de integração do
//! alocador (`tests/corpo_memoria.rs`), por isso só usa a API pública, pelo apelido
//! `wc` que quem inclui define. Os segredos nascem sem realocação (o teste do alocador
//! procura as marcas em todo bloco liberado).

#![allow(dead_code)]

use super::wc::config::{PinHash, VaultConfig};
use super::wc::entry::{AccessLevel, AttachmentDraft, DataDraft, Draft, Entry, ExtraDraft, Secret};
use super::wc::limits::MAX_ATTACHMENTS_TOTAL;

/// Senha mestra do cofre de teste.
pub const SENHA_MESTRA: &str = "falso-mestra-ward-01";
/// PIN do cofre de teste (D15). Nunca é gravado em claro: só o hash dele.
pub const PIN_TESTE: &str = "135790";

/// Configuração do cofre de teste (D30): hash do PIN de teste, trava em 240 minutos e
/// backup desligado. Calculada uma vez (o Argon2id do PIN custa 64 MiB) e compartilhada
/// pelos testes.
pub fn configuracao_de_teste() -> &'static VaultConfig {
    static CFG: std::sync::OnceLock<VaultConfig> = std::sync::OnceLock::new();
    CFG.get_or_init(|| VaultConfig::new(PinHash::create(PIN_TESTE.as_bytes()).unwrap()))
}

/// Uma configuração nova, com hash próprio (sal novo), para quem precisa de posse.
pub fn configuracao_nova() -> VaultConfig {
    VaultConfig::new(PinHash::create(PIN_TESTE.as_bytes()).unwrap())
}

/// Todos os valores secretos do cofre de teste (as marcas que os testes procuram).
pub const SEGREDOS: &[&str] = &[
    SENHA_MESTRA,
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

/// Chave privada falsa, montada em tempo de execução (o texto inteiro não fica no
/// repositório) e numa alocação só, do tamanho exato.
pub fn chave_falsa() -> String {
    let partes = [
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
    let mut c = String::with_capacity(partes.iter().map(|p| p.len()).sum());
    for p in partes {
        c.push_str(p);
    }
    c
}

fn rascunho(nome: &str, grupo: &str, ambiente: &str, dados: DataDraft) -> Draft {
    Draft {
        name: t(nome),
        group: t(grupo),
        environment: t(ambiente),
        project: String::new(),
        tags: Vec::new(),
        data: dados,
        extras: Vec::new(),
        attachments: Vec::new(),
    }
}

/// Valida e aplica o nível (como o daemon faria depois do PIN).
pub fn validada(r: Draft, nivel: AccessLevel) -> Entry {
    let mut e = match r.validate() {
        Ok(e) => e,
        Err(erros) => panic!("entrada de teste inválida: {erros}"),
    };
    e.set_level(nivel);
    e
}

fn login(url: &str, usuario: &str, senha: &str, totp: Option<&str>) -> DataDraft {
    DataDraft::Login {
        url: t(url),
        username: t(usuario),
        password: s(senha),
        totp_seed: totp.and_then(s),
    }
}

fn banco(host: &str, porta: &str, tunel: &str) -> DataDraft {
    DataDraft::Database {
        engine: t("postgres"),
        host: t(host),
        port: t(porta),
        db_name: t("app_teste"),
        username: t("app"),
        password: s("falso-PG:s3nha/01@#"),
        tunnel: t(tunel),
    }
}

fn servidor_senha(porta: &str, usuario: &str, senha: &str) -> DataDraft {
    DataDraft::Server {
        host: t("127.0.0.1"),
        port: t(porta),
        username: t(usuario),
        password: s(senha),
        private_key: None,
        passphrase: None,
        jump: String::new(),
    }
}

fn servidor_chave(host: &str, porta: &str, salto: &str) -> DataDraft {
    DataDraft::Server {
        host: t(host),
        port: t(porta),
        username: t("deploy"),
        password: None,
        private_key: Some(Secret::from_text(chave_falsa())),
        passphrase: s("falso-passphrase-01"),
        jump: t(salto),
    }
}

/// As 22 entradas do cofre de teste, com os níveis da tabela (fora de ordem de nome).
pub fn cofre_de_teste() -> Vec<Entry> {
    use AccessLevel::{Approval, Blocked, Free};
    const WP: &str = "http://localhost:8081/wp-login.php";
    let mut generico = rascunho("gama-generico", "gama", "stage", DataDraft::Generic);
    generico.extras = vec![
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
    generico.attachments = vec![AttachmentDraft {
        name: t("app.env"),
        content: Secret::from_bytes(b"FALSO_ENV=falso-anexo-01\n".to_vec()),
    }];
    // D59: caracteres especiais (aspas simples, `$&<>`, `/` e fora do ASCII).
    let mut simbolos = rascunho(
        "delta-api-simbolos",
        "delta",
        "dev",
        DataDraft::Api {
            url: t("https://api.teste.invalid"),
            token: s("falso-Ab3$k&<9>Zq!'/ç-01"),
            variable: t("SIMBOLOS_TOKEN"),
        },
    );
    simbolos.extras = vec![ExtraDraft {
        name: t("SEGREDO_ASCII"),
        value: Secret::from_text(t("falso-Ab3$k&<9>Zq!'")),
        secret: true,
    }];
    let lista = vec![
        (
            rascunho(
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
            rascunho(
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
            rascunho(
                "acme-pg-dev",
                "acme",
                "dev",
                banco("127.0.0.1", "55432", ""),
            ),
            Free,
        ),
        (
            rascunho(
                "acme-pg-aprovacao",
                "acme",
                "stage",
                banco("127.0.0.1", "55432", ""),
            ),
            Approval,
        ),
        (
            rascunho(
                "acme-pg-prod",
                "acme",
                "prod",
                banco("127.0.0.1", "55432", ""),
            ),
            Approval,
        ),
        (
            rascunho(
                "acme-pg-tunel",
                "acme",
                "stage",
                banco("postgres", "5432", "acme-ssh-senha-stage"),
            ),
            Free,
        ),
        (
            rascunho(
                "acme-ssh-senha-stage",
                "acme",
                "stage",
                servidor_senha("2222", "deploy", "falso-SSH-senha-01"),
            ),
            Free,
        ),
        (
            rascunho(
                "acme-ssh-chave-prod",
                "acme",
                "prod",
                servidor_chave("127.0.0.1", "2223", ""),
            ),
            Free,
        ),
        (
            rascunho(
                "acme-ssh-salto",
                "acme",
                "prod",
                servidor_senha("2224", "salto", "falso-SALTO-01"),
            ),
            Approval,
        ),
        (
            rascunho(
                "acme-ssh-salto-livre",
                "acme",
                "prod",
                servidor_senha("2224", "salto", "falso-SALTO-01"),
            ),
            Free,
        ),
        (
            rascunho(
                "acme-ssh-interno",
                "acme",
                "prod",
                servidor_chave("ssh-interno", "22", "acme-ssh-salto-livre"),
            ),
            Free,
        ),
        (
            rascunho(
                "acme-ssh-interno-aprov",
                "acme",
                "prod",
                servidor_chave("ssh-interno", "22", "acme-ssh-salto"),
            ),
            Free,
        ),
        (
            rascunho(
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
            rascunho(
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
            rascunho(
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
            rascunho(
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
            rascunho(
                "gama-nota",
                "gama",
                "dev",
                DataDraft::SecureNote {
                    text: s("falso-nota-01"),
                },
            ),
            Free,
        ),
        (generico, Free),
        (
            rascunho(
                "gama-bloqueada",
                "gama",
                "prod",
                login("https://bloqueada.invalid", "x", "falso-BLOQ-01", None),
            ),
            Blocked,
        ),
        (
            rascunho(
                "delta-redir",
                "delta",
                "dev",
                login("http://localhost:8084/login", "u", "falso-REDIR-01", None),
            ),
            Free,
        ),
        (
            rascunho(
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
        (simbolos, Free),
    ];
    lista
        .into_iter()
        .map(|(r, nivel)| validada(r, nivel))
        .collect()
}

/// Bytes de 0 a 255 repetidos até `tam`.
pub fn bytes_0_a_255(tam: usize) -> Vec<u8> {
    (0..tam).map(|i| (i % 256) as u8).collect()
}

/// Uma entrada de cada tipo no máximo do modelo (V2.30): projeto, 16 tags, 64 extras
/// (metade segredo) e dois anexos com bytes de 0 a 255 que somam exatamente o limite de
/// D29 (256 KiB), dentro do orçamento da máscara; mais uma `api` com o token de 64 KiB e
/// só o anexo pequeno (as duas coisas juntas passam do orçamento). Os campos no limite de D25 (URL de
/// 2048, token de 64 KiB, nota de 1 MiB) prendem os limites da leitura aos da
/// validação.
pub fn um_de_cada_cheio() -> Vec<Entry> {
    use AccessLevel::{Approval, Blocked, Free};
    let url_longa = {
        let base = "https://cheio.invalid/";
        let mut u = String::with_capacity(2048);
        u.push_str(base);
        while u.len() < 2048 {
            u.push('a');
        }
        u
    };
    let dados: Vec<(&str, DataDraft, AccessLevel)> = vec![
        (
            "zz-cheio-login",
            DataDraft::Login {
                url: url_longa,
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
                private_key: Some(Secret::from_text(chave_falsa())),
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
        // O token no limite de D25 (64 KiB) passa do orçamento da máscara junto com
        // 256 KiB de anexos (entrada e `WARD_TOKEN`, ~1 MiB de padrões cada, mais
        // ~2,4 MiB do anexo); fica numa segunda `api`, com o anexo pequeno.
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
    dados
        .into_iter()
        .map(|(nome, dados, nivel)| {
            let mut r = rascunho(nome, "zeta", "stage", dados);
            r.project = t("projeto cheio");
            r.tags = (1..=16).map(|n| format!("tag-{n:02}")).collect();
            r.extras = (1..=64)
                .map(|n| {
                    let segredo = n % 2 == 0;
                    let valor = if segredo {
                        format!("falso-extra-{n:02}")
                    } else {
                        format!("valor-{n:02}")
                    };
                    ExtraDraft {
                        name: format!("extra_{n:02}"),
                        value: Secret::from_text(valor),
                        secret: segredo,
                    }
                })
                .collect();
            r.attachments = vec![AttachmentDraft {
                name: t("binario.bin"),
                content: Secret::from_bytes(bytes_0_a_255(4096)),
            }];
            if nome != "zz-cheio-api-token" {
                r.attachments.push(AttachmentDraft {
                    name: t("grande.bin"),
                    content: Secret::from_bytes(bytes_0_a_255(MAX_ATTACHMENTS_TOTAL - 4096)),
                });
            }
            validada(r, nivel)
        })
        .collect()
}
