//! Testes da entrega (D10): V2.14, V2.15, V2.16 e os ataques à montagem do ambiente
//! (URL que muda de destino, extra que vira variável do sistema, recurso que devolve
//! lixo, segredo em `Debug`/erro). Só valores falsos do cofre de teste (D15).

use super::*;
use crate::entry::{
    AttachmentDraft, COMMON_VARIABLES, DataDraft, Draft, EntryType, ExtraDraft, type_attachments,
    type_variables,
};

// ---- Ajudantes ----

fn s(v: &str) -> Option<Secret> {
    Some(Secret::from_text(v.to_owned()))
}

fn t(v: &str) -> String {
    v.to_owned()
}

/// Chave privada falsa, montada em tempo de execução (o texto inteiro não fica no
/// repositório, para não parecer chave real a uma varredura).
fn chave_falsa() -> String {
    let rotulo = ["OPENSSH", "PRIVATE KEY"].join(" ");
    format!("-----BEGIN {rotulo}-----\nfalso-chave-ed25519-teste\n-----END {rotulo}-----\n")
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

fn extra(nome: &str, valor: &str, segredo: bool) -> ExtraDraft {
    ExtraDraft {
        name: t(nome),
        value: Secret::from_text(t(valor)),
        secret: segredo,
    }
}

fn anexo(nome: &str, conteudo: &[u8]) -> AttachmentDraft {
    AttachmentDraft {
        name: t(nome),
        content: Secret::from_bytes(conteudo.to_vec()),
    }
}

fn banco(
    motor: &str,
    host: &str,
    porta: &str,
    base: &str,
    usuario: &str,
    senha: &str,
) -> DataDraft {
    DataDraft::Database {
        engine: t(motor),
        host: t(host),
        port: t(porta),
        db_name: t(base),
        username: t(usuario),
        password: s(senha),
        tunnel: String::new(),
    }
}

fn valida(r: Draft) -> Entry {
    match r.validate() {
        Ok(e) => e,
        Err(erros) => panic!("devia validar: {erros}"),
    }
}

const CODIGO_FALSO: &str = "123456";
const PORTA_TUNEL: u16 = 40123;

/// Recursos falsos: guardam o que foi pedido, sem tocar em disco nem rede.
#[derive(Default)]
struct Falsos {
    totp: Vec<Vec<u8>>,
    arquivos: Vec<(String, Vec<u8>)>,
    tuneis: Vec<(String, String, u16)>,
    /// Respostas forçadas (para os ataques).
    codigo: Option<&'static str>,
    caminho: Option<&'static str>,
    porta: Option<u16>,
    falhar: bool,
}

impl Falsos {
    fn chamadas(&self) -> usize {
        self.totp.len() + self.arquivos.len() + self.tuneis.len()
    }
}

impl Recursos for Falsos {
    fn codigo_totp(&mut self, semente: &Secret) -> Result<Zeroizing<String>, ErroRecurso> {
        self.totp.push(semente.expose().to_vec());
        if self.falhar {
            return Err(ErroRecurso("relógio indisponível"));
        }
        Ok(Zeroizing::new(t(self.codigo.unwrap_or(CODIGO_FALSO))))
    }

    fn arquivo(&mut self, nome: &str, conteudo: &Secret) -> Result<String, ErroRecurso> {
        self.arquivos.push((t(nome), conteudo.expose().to_vec()));
        if self.falhar {
            return Err(ErroRecurso("sem espaço em /run/user"));
        }
        Ok(match self.caminho {
            Some(c) => t(c),
            None => format!("/run/user/1000/ward/tmp/{}-{nome}", self.arquivos.len()),
        })
    }

    fn tunel(&mut self, servidor: &str, host: &str, porta: u16) -> Result<u16, ErroRecurso> {
        self.tuneis.push((t(servidor), t(host), porta));
        if self.falhar {
            return Err(ErroRecurso("túnel recusado"));
        }
        Ok(self.porta.unwrap_or(PORTA_TUNEL))
    }
}

fn montar_ok(e: &Entry, r: &mut Falsos) -> Entrega {
    montar(e, r).unwrap_or_else(|erro| panic!("{e}: {erro}"))
}

/// Pares (nome, valor) da entrega, na ordem.
fn pares(e: &Entrega) -> Vec<(String, String)> {
    e.variaveis()
        .iter()
        .map(|v| (t(v.nome()), t(v.expor())))
        .collect()
}

/// Confere nomes e valores exatamente (ordem não importa; nada a mais, nada a menos).
fn confere(e: &Entrega, esperado: &[(&str, &str)], onde: &str) {
    let mut obtido = pares(e);
    obtido.sort();
    let mut esperado: Vec<(String, String)> = esperado.iter().map(|(n, v)| (t(n), t(v))).collect();
    esperado.sort();
    assert_eq!(obtido, esperado, "{onde}");
}

/// Segredos usados nos testes, para os testes de vazamento.
const SEGREDOS: &[&str] = &[
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

fn sem_segredo(texto: &str, onde: &str) {
    for segredo in SEGREDOS {
        assert!(!texto.contains(segredo), "{onde} contém segredo");
    }
}

// ---- As entradas da tabela "Entrega esperada" ----

fn wp_prod() -> Entry {
    valida(rascunho(
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
    valida(rascunho(
        "acme-pg-dev",
        "acme",
        "dev",
        banco(
            "postgres",
            "127.0.0.1",
            "55432",
            "app_teste",
            "app",
            "falso-PG:s3nha/01@#",
        ),
    ))
}

fn pg_tunel() -> Entry {
    let mut dados = banco(
        "postgres",
        "postgres",
        "5432",
        "app_teste",
        "app",
        "falso-PG:s3nha/01@#",
    );
    if let DataDraft::Database { tunnel: tunel, .. } = &mut dados {
        *tunel = t("acme-ssh-senha-stage");
    }
    valida(rascunho("acme-pg-tunel", "acme", "stage", dados))
}

fn ssh_senha() -> Entry {
    valida(rascunho(
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

fn ssh_chave() -> Entry {
    valida(rascunho(
        "acme-ssh-chave-prod",
        "acme",
        "prod",
        DataDraft::Server {
            host: t("127.0.0.1"),
            port: t("2223"),
            username: t("deploy"),
            password: None,
            private_key: Some(Secret::from_text(chave_falsa())),
            passphrase: s("falso-passphrase-01"),
            jump: String::new(),
        },
    ))
}

fn github() -> Entry {
    valida(rascunho(
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
    valida(rascunho(
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
    valida(rascunho(
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
    valida(rascunho(
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

fn nota() -> Entry {
    valida(rascunho(
        "gama-nota",
        "gama",
        "dev",
        DataDraft::SecureNote {
            text: s("falso-nota-01"),
        },
    ))
}

fn generico() -> Entry {
    let mut r = rascunho("gama-generico", "gama", "stage", DataDraft::Generic);
    r.extras = vec![
        extra("REGIAO", "r-01", false),
        extra("CHAVE_LICENCA", "falso-licenca-01", true),
    ];
    r.attachments = vec![anexo("app.env", b"FALSO_ENV=falso-anexo-01\n")];
    valida(r)
}

/// Uma entrada de cada tipo com entrega (nota fora).
fn todas_com_entrega() -> Vec<Entry> {
    vec![
        wp_prod(),
        pg_dev(),
        pg_tunel(),
        ssh_senha(),
        ssh_chave(),
        github(),
        aws(),
        smtp(),
        sftp(),
        generico(),
    ]
}

// ---- V2.14 ----

#[test]
fn v2_14_entrega_esperada_do_contrato() {
    let mut r = Falsos::default();
    let e = montar_ok(&wp_prod(), &mut r);
    confere(
        &e,
        &[
            ("WARD_ENTRADA", "acme-wp-prod"),
            ("WARD_GRUPO", "acme"),
            ("WARD_AMBIENTE", "prod"),
            ("WARD_URL", "http://localhost:8081/wp-login.php"),
            ("WARD_USER", "wp_editor"),
            ("WARD_PASS", "falso-WP-senha-02"),
            ("WARD_TOTP", CODIGO_FALSO),
        ],
        "acme-wp-prod",
    );
    // O código vem da semente certa; a semente nunca vira variável.
    assert_eq!(r.totp, vec![b"JBSWY3DPEHPK3PXP".to_vec()]);
    assert!(
        !pares(&e)
            .iter()
            .any(|(_, v)| v.contains("JBSWY3DPEHPK3PXP"))
    );

    let mut r = Falsos::default();
    let e = montar_ok(&pg_dev(), &mut r);
    confere(
        &e,
        &[
            ("WARD_ENTRADA", "acme-pg-dev"),
            ("WARD_GRUPO", "acme"),
            ("WARD_AMBIENTE", "dev"),
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
    assert_eq!(r.chamadas(), 0, "banco sem túnel não pede recurso");

    let mut r = Falsos::default();
    let e = montar_ok(&pg_tunel(), &mut r);
    let porta = PORTA_TUNEL.to_string();
    let url = format!("postgresql://app:falso-PG%3As3nha%2F01%40%23@127.0.0.1:{porta}/app_teste");
    confere(
        &e,
        &[
            ("WARD_ENTRADA", "acme-pg-tunel"),
            ("WARD_GRUPO", "acme"),
            ("WARD_AMBIENTE", "stage"),
            ("PGHOST", "127.0.0.1"),
            ("PGPORT", &porta),
            ("PGDATABASE", "app_teste"),
            ("PGUSER", "app"),
            ("PGPASSWORD", "falso-PG:s3nha/01@#"),
            ("DATABASE_URL", &url),
        ],
        "acme-pg-tunel",
    );
    assert_ne!(PORTA_TUNEL, 5432);
    assert_eq!(
        r.tuneis,
        vec![(t("acme-ssh-senha-stage"), t("postgres"), 5432)],
        "o túnel vai pelo servidor referenciado até host:porta do banco"
    );

    let e = montar_ok(&ssh_senha(), &mut Falsos::default());
    confere(
        &e,
        &[
            ("WARD_ENTRADA", "acme-ssh-senha-stage"),
            ("WARD_GRUPO", "acme"),
            ("WARD_AMBIENTE", "stage"),
            ("WARD_HOST", "127.0.0.1"),
            ("WARD_PORT", "2222"),
            ("WARD_USER", "deploy"),
            ("WARD_PASS", "falso-SSH-senha-01"),
        ],
        "acme-ssh-senha-stage",
    );

    let mut r = Falsos::default();
    let e = montar_ok(&ssh_chave(), &mut r);
    confere(
        &e,
        &[
            ("WARD_ENTRADA", "acme-ssh-chave-prod"),
            ("WARD_GRUPO", "acme"),
            ("WARD_AMBIENTE", "prod"),
            ("WARD_HOST", "127.0.0.1"),
            ("WARD_PORT", "2223"),
            ("WARD_USER", "deploy"),
            ("WARD_ARQUIVO_CHAVE", "/run/user/1000/ward/tmp/1-chave"),
            ("WARD_PASSPHRASE", "falso-passphrase-01"),
        ],
        "acme-ssh-chave-prod",
    );
    assert_eq!(r.arquivos, vec![(t("chave"), chave_falsa().into_bytes())]);
    assert!(
        !pares(&e)
            .iter()
            .any(|(_, v)| v.contains("falso-chave-ed25519-teste")),
        "a chave vai em arquivo, nunca no ambiente"
    );

    let e = montar_ok(&github(), &mut Falsos::default());
    confere(
        &e,
        &[
            ("WARD_ENTRADA", "beta-github-api"),
            ("WARD_GRUPO", "beta"),
            ("WARD_AMBIENTE", "dev"),
            ("WARD_URL", "https://api.github.com"),
            ("WARD_TOKEN", "falso_token_github_01"),
            ("GITHUB_TOKEN", "falso_token_github_01"),
        ],
        "beta-github-api",
    );

    let e = montar_ok(&aws(), &mut Falsos::default());
    confere(
        &e,
        &[
            ("WARD_ENTRADA", "beta-aws-dev"),
            ("WARD_GRUPO", "beta"),
            ("WARD_AMBIENTE", "dev"),
            ("AWS_ACCESS_KEY_ID", "FALSOACCESSKEY01"),
            ("AWS_SECRET_ACCESS_KEY", "falso-aws-secret-01"),
            ("AWS_SESSION_TOKEN", "falso-aws-session-01"),
            ("AWS_REGION", "sa-east-1"),
            ("AWS_DEFAULT_REGION", "sa-east-1"),
            ("AWS_ROLE_ARN", "arn:aws:iam::000000000000:role/teste"),
        ],
        "beta-aws-dev",
    );

    let e = montar_ok(&smtp(), &mut Falsos::default());
    confere(
        &e,
        &[
            ("WARD_ENTRADA", "beta-smtp-stage"),
            ("WARD_GRUPO", "beta"),
            ("WARD_AMBIENTE", "stage"),
            ("SMTP_HOST", "smtp.teste.invalid"),
            ("SMTP_PORT", "587"),
            ("SMTP_USER", "mailer"),
            ("SMTP_PASS", "falso-SMTP-01"),
            ("SMTP_SECURITY", "starttls"),
        ],
        "beta-smtp-stage",
    );

    let e = montar_ok(&sftp(), &mut Falsos::default());
    confere(
        &e,
        &[
            ("WARD_ENTRADA", "beta-sftp-dev"),
            ("WARD_GRUPO", "beta"),
            ("WARD_AMBIENTE", "dev"),
            ("WARD_PROTOCOLO", "sftp"),
            ("WARD_HOST", "127.0.0.1"),
            ("WARD_PORT", "2222"),
            ("WARD_USER", "sftpuser"),
            ("WARD_PASS", "falso-SFTP-01"),
        ],
        "beta-sftp-dev",
    );

    let mut r = Falsos::default();
    let e = montar_ok(&generico(), &mut r);
    confere(
        &e,
        &[
            ("WARD_ENTRADA", "gama-generico"),
            ("WARD_GRUPO", "gama"),
            ("WARD_AMBIENTE", "stage"),
            ("WARD_REGIAO", "r-01"),
            ("WARD_CHAVE_LICENCA", "falso-licenca-01"),
            ("WARD_ARQUIVO_APP_ENV", "/run/user/1000/ward/tmp/1-app.env"),
        ],
        "gama-generico",
    );
    assert_eq!(
        r.arquivos,
        vec![(t("app.env"), b"FALSO_ENV=falso-anexo-01\n".to_vec())]
    );

    let mut r = Falsos::default();
    let erro = montar(&nota(), &mut r).expect_err("nota não tem entrega");
    assert_eq!(erro, ErroEntrega::TipoSemEntrega);
    assert_eq!(erro.to_string(), "tipo sem entrega");
    assert_eq!(r.chamadas(), 0, "a recusa vem antes de qualquer recurso");
}

#[test]
fn v2_14_comuns_vem_primeiro_e_na_ordem() {
    for e in todas_com_entrega() {
        let entrega = montar_ok(&e, &mut Falsos::default());
        let nomes: Vec<&str> = entrega.nomes().take(3).collect();
        assert_eq!(
            nomes,
            ["WARD_ENTRADA", "WARD_GRUPO", "WARD_AMBIENTE"],
            "{e}"
        );
        assert_eq!(entrega.valor("WARD_ENTRADA"), Some(e.name()));
        assert_eq!(entrega.valor("WARD_GRUPO"), Some(e.group()));
        assert_eq!(entrega.valor("WARD_AMBIENTE"), Some(e.environment().name()));
    }
}

#[test]
fn opcionais_ausentes_ficam_de_fora() {
    // login sem semente: sem WARD_TOTP e sem chamar o TOTP.
    let r = rascunho(
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
    let mut f = Falsos::default();
    let e = montar_ok(&valida(r), &mut f);
    assert!(e.valor("WARD_TOTP").is_none());
    assert_eq!(f.chamadas(), 0);

    // aws sem session token e sem role.
    let r = rascunho(
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
    let e = montar_ok(&valida(r), &mut Falsos::default());
    assert!(e.valor("AWS_SESSION_TOKEN").is_none());
    assert!(e.valor("AWS_ROLE_ARN").is_none());

    // api sem URL e sem variável: só WARD_TOKEN.
    let r = rascunho(
        "z",
        "beta",
        "dev",
        DataDraft::Api {
            url: String::new(),
            token: s("falso_token_github_01"),
            variable: String::new(),
        },
    );
    let e = montar_ok(&valida(r), &mut Falsos::default());
    let nomes: Vec<&str> = e.nomes().skip(3).collect();
    assert_eq!(nomes, ["WARD_TOKEN"]);

    // banco postgres sem base: sem PGDATABASE e URL sem caminho.
    let e = montar_ok(
        &valida(rascunho(
            "w",
            "acme",
            "dev",
            banco(
                "postgres",
                "127.0.0.1",
                "55432",
                "",
                "app",
                "falso-PG:s3nha/01@#",
            ),
        )),
        &mut Falsos::default(),
    );
    assert!(e.valor("PGDATABASE").is_none());
    assert_eq!(
        e.valor("DATABASE_URL"),
        Some("postgresql://app:falso-PG%3As3nha%2F01%40%23@127.0.0.1:55432")
    );
}

// ---- V2.15 ----

fn banco_v2_15(motor: &str) -> Entry {
    // No redis a base é só número (D33); aqui fica sem base.
    let base = if motor == "redis" { "" } else { "b" };
    valida(rascunho(
        "motor-teste",
        "acme",
        "dev",
        banco(motor, "db.teste.invalid", "", base, "u", "falso:DB/01"),
    ))
}

#[test]
fn v2_15_todos_os_motores() {
    let comuns = [
        ("WARD_ENTRADA", "motor-teste"),
        ("WARD_GRUPO", "acme"),
        ("WARD_AMBIENTE", "dev"),
    ];
    let casos: [(&str, Vec<(&str, &str)>); 5] = [
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
    for (motor, especificas) in casos {
        let e = montar_ok(&banco_v2_15(motor), &mut Falsos::default());
        let mut esperado = comuns.to_vec();
        esperado.extend(especificas);
        confere(&e, &esperado, motor);
    }
}

#[test]
fn redis_com_base_numerica_vai_no_url() {
    let e = valida(rascunho(
        "r",
        "acme",
        "dev",
        banco("redis", "db.teste.invalid", "", "3", "u", "falso:DB/01"),
    ));
    let e = montar_ok(&e, &mut Falsos::default());
    assert_eq!(
        e.valor("REDIS_URL"),
        Some("redis://u:falso%3ADB%2F01@db.teste.invalid:6379/3")
    );
}

#[test]
fn host_ipv6_vai_entre_colchetes_no_url() {
    let e = valida(rascunho(
        "v6",
        "acme",
        "dev",
        banco("postgres", "::1", "", "b", "u", "falso:DB/01"),
    ));
    let e = montar_ok(&e, &mut Falsos::default());
    assert_eq!(e.valor("PGHOST"), Some("::1"));
    assert_eq!(
        e.valor("DATABASE_URL"),
        Some("postgresql://u:falso%3ADB%2F01@[::1]:5432/b")
    );
}

#[test]
fn percent_encoding_de_todos_os_bytes() {
    let mut s = String::new();
    codificar_em(&mut s, "aZ09-._~ :/?#[]@!$&'()*+,;=%\u{e9}");
    assert_eq!(
        s,
        "aZ09-._~%20%3A%2F%3F%23%5B%5D%40%21%24%26%27%28%29%2A%2B%2C%3B%3D%25%C3%A9"
    );
}

// ---- V2.16 ----

#[test]
fn v2_16_extra_e_anexo_normalizados() {
    let mut r = rascunho("g", "gama", "dev", DataDraft::Generic);
    r.extras = vec![extra("api-key", "falso-licenca-01", true)];
    r.attachments = vec![anexo("chave acesso.pem", b"falso-anexo-01\n")];
    let mut f = Falsos::default();
    let e = montar_ok(&valida(r), &mut f);
    assert_eq!(e.valor("WARD_API_KEY"), Some("falso-licenca-01"));
    assert_eq!(
        e.valor("WARD_ARQUIVO_CHAVE_ACESSO_PEM"),
        Some("/run/user/1000/ward/tmp/1-chave acesso.pem")
    );
    assert_eq!(
        f.arquivos,
        vec![(t("chave acesso.pem"), b"falso-anexo-01\n".to_vec())]
    );

    // Dois extras que normalizam para o mesmo nome: recusados ao salvar.
    let mut r = rascunho("g", "gama", "dev", DataDraft::Generic);
    r.extras = vec![extra("api-key", "a", false), extra("API KEY", "b", false)];
    assert!(
        r.validate().is_err(),
        "api-key e API KEY viram WARD_API_KEY"
    );
    let mut r = rascunho("g", "gama", "dev", DataDraft::Generic);
    r.extras = vec![extra("x", "a", false)];
    r.attachments = vec![anexo("a.pem", b"1"), anexo("a-pem", b"2")];
    assert!(
        r.validate().is_err(),
        "a.pem e a-pem viram WARD_ARQUIVO_A_PEM"
    );
}

#[test]
fn extras_entram_em_todo_tipo_que_tem_entrega() {
    let mut r = rascunho(
        "pg",
        "acme",
        "dev",
        banco("postgres", "127.0.0.1", "", "b", "u", "falso:DB/01"),
    );
    r.extras = vec![extra("sslmode", "require", false)];
    r.attachments = vec![anexo("ca.pem", b"falso-anexo-01")];
    let e = montar_ok(&valida(r), &mut Falsos::default());
    let nomes: Vec<&str> = e.nomes().collect();
    // Comuns, depois o tipo, depois extras e anexos.
    assert_eq!(nomes.last(), Some(&"WARD_ARQUIVO_CA_PEM"));
    assert_eq!(nomes[nomes.len() - 2], "WARD_SSLMODE");
}

// ---- Coerência com a validação (Fase 15) ----

/// As variáveis `WARD_*` que cada tipo monta são exatamente as que a validação reserva
/// (e por isso um extra ou anexo nunca as sobrescreve).
#[test]
fn variaveis_do_tipo_batem_com_as_reservadas() {
    let mut vistos = Vec::new();
    for e in todas_com_entrega() {
        let tipo = e.kind();
        vistos.push(tipo);
        let entrega = montar_ok(&e, &mut Falsos::default());
        let extras: Vec<String> = e
            .extras()
            .iter()
            .map(|x| format!("WARD_{}", variable_name(x.name())))
            .chain(
                e.attachments()
                    .iter()
                    .map(|a| format!("WARD_ARQUIVO_{}", variable_name(a.name()))),
            )
            .collect();
        for nome in entrega.nomes() {
            if extras.iter().any(|x| x == nome) {
                continue;
            }
            let Some(sufixo) = nome.strip_prefix("WARD_") else {
                continue;
            };
            if let Some(arquivo) = sufixo.strip_prefix("ARQUIVO_") {
                assert!(
                    type_attachments(tipo).contains(&arquivo),
                    "{nome} de {tipo} não está em anexos_do_tipo"
                );
            } else {
                assert!(
                    COMMON_VARIABLES.contains(&sufixo) || type_variables(tipo).contains(&sufixo),
                    "{nome} de {tipo} não está em variaveis_do_tipo"
                );
            }
        }
    }
    // Todos os tipos com entrega passaram por aqui (a nota é recusada à parte).
    for tipo in EntryType::ALL {
        if tipo != EntryType::SecureNote {
            assert!(vistos.contains(&tipo), "falta {tipo}");
        }
    }
}

#[test]
fn marca_de_segredo_bate_com_o_valor() {
    for e in todas_com_entrega() {
        let entrega = montar_ok(&e, &mut Falsos::default());
        for v in entrega.variaveis() {
            let tem_segredo = SEGREDOS.iter().any(|s| v.expor().contains(s));
            assert_eq!(
                v.e_segredo(),
                tem_segredo,
                "{} de {e}: marca de segredo errada",
                v.nome()
            );
        }
    }
}

// ---- Ataques ----

/// Senha e usuário com caracteres de URL não mudam host, porta nem opções do cliente
/// (ex.: `?host=` do libpq ou `authSource` do mongo). A base com esses caracteres nem
/// chega à entrega: a validação recusa (D33); o percent-encoding dela no URL fica como
/// defesa extra.
#[test]
fn ataque_senha_usuario_ou_base_mudam_o_destino_do_url() {
    let senha = "x@evil.invalid:1/?#&host=evil";
    let usuario = "u@evil.invalid:2";
    let hostil = "app?host=evil.invalid&sslmode=disable#f/../x";
    for motor in ["postgres", "mysql", "redis", "mongo", "sqlserver"] {
        let r = rascunho(
            "ataque",
            "acme",
            "dev",
            banco(motor, "db.teste.invalid", "", hostil, usuario, senha),
        );
        assert!(
            r.validate().is_err_and(|e| e.has("base")),
            "{motor}: base hostil aceita"
        );
        let base = if motor == "redis" { "7" } else { "app_teste-1" };
        let e = valida(rascunho(
            "ataque",
            "acme",
            "dev",
            banco(motor, "db.teste.invalid", "", base, usuario, senha),
        ));
        let entrega = montar_ok(&e, &mut Falsos::default());
        for v in entrega.variaveis() {
            if !v.nome().ends_with("_URL") && v.nome() != "MONGODB_URI" {
                continue;
            }
            let url = v.expor();
            let (_, resto) = url.split_once("://").expect("esquema");
            // Um único '@' (o separador) e, depois dele, o host e a porta de verdade.
            assert_eq!(resto.matches('@').count(), 1, "{motor} {}", v.nome());
            let (credenciais, destino) = resto.split_once('@').expect("@");
            assert_eq!(credenciais.matches(':').count(), 1, "{motor}");
            let porta = Engine::from_name(motor).expect("motor").default_port();
            assert!(
                destino.starts_with(&format!("db.teste.invalid:{porta}")),
                "{motor}: destino mudou"
            );
            let depois = &destino[format!("db.teste.invalid:{porta}").len()..];
            // Só o '?database=' do sqlserver; nenhuma outra opção, fragmento ou '/'.
            let depois = depois.strip_prefix("?database=").unwrap_or(depois);
            let depois = depois.strip_prefix('/').unwrap_or(depois);
            assert!(
                !depois.contains(['?', '#', '&', '/', '=']),
                "{motor}: base virou opção ({depois})"
            );
        }
    }
}

#[test]
fn ataque_extra_vira_variavel_do_sistema() {
    let mut r = rascunho(
        "pg",
        "acme",
        "dev",
        banco("postgres", "127.0.0.1", "", "b", "u", "falso:DB/01"),
    );
    r.extras = vec![
        extra("LD_PRELOAD", "/tmp/evil.so", false),
        extra("PATH", "/tmp/evil", false),
        extra("PGPASSWORD", "outra", false),
        extra("DATABASE_URL", "postgresql://evil.invalid", false),
    ];
    r.attachments = vec![anexo("LD_AUDIT", b"x")];
    let e = montar_ok(&valida(r), &mut Falsos::default());
    for nome in e.nomes() {
        assert!(!nome.starts_with("LD_"), "{nome}");
        assert_ne!(nome, "PATH");
    }
    assert_eq!(e.valor("PGPASSWORD"), Some("falso:DB/01"));
    assert_eq!(
        e.valor("DATABASE_URL"),
        Some("postgresql://u:falso%3ADB%2F01@127.0.0.1:5432/b")
    );
    assert_eq!(e.valor("WARD_LD_PRELOAD"), Some("/tmp/evil.so"));
    assert!(e.valor("WARD_ARQUIVO_LD_AUDIT").is_some());
}

#[test]
fn ataque_recurso_devolve_lixo() {
    // Código TOTP fora do formato (quebra de linha, letras, longo, vazio, nulo).
    for codigo in [
        "12345\n",
        "abcdef",
        "123456789",
        "",
        "12345",
        "1234\u{0}56",
        "12 345",
    ] {
        let mut f = Falsos {
            codigo: Some(codigo),
            ..Falsos::default()
        };
        let erro = montar(&wp_prod(), &mut f).expect_err("código inválido");
        assert!(
            matches!(erro, ErroEntrega::RecursoInvalido(_)),
            "{codigo:?}"
        );
    }
    // Caminho vazio ou com nulo.
    for caminho in ["", "/run/user/1000/ward/tmp/x\u{0}y"] {
        let mut f = Falsos {
            caminho: Some(caminho),
            ..Falsos::default()
        };
        assert!(montar(&generico(), &mut f).is_err(), "{caminho:?}");
        assert!(montar(&ssh_chave(), &mut f).is_err(), "{caminho:?}");
    }
    // Porta 0 do túnel.
    let mut f = Falsos {
        porta: Some(0),
        ..Falsos::default()
    };
    assert!(matches!(
        montar(&pg_tunel(), &mut f),
        Err(ErroEntrega::RecursoInvalido(_))
    ));
    // Recurso que falha: o erro sobe, sem segredo.
    let mut f = Falsos {
        falhar: true,
        ..Falsos::default()
    };
    for e in [wp_prod(), pg_tunel(), ssh_chave(), generico()] {
        let erro = montar(&e, &mut f).expect_err("recurso falhou");
        assert!(matches!(erro, ErroEntrega::Recurso(_)), "{e}");
        sem_segredo(&format!("{erro} {erro:?}"), "erro de recurso");
    }
}

#[test]
fn ataque_debug_e_erros_sem_segredo() {
    for e in todas_com_entrega() {
        let entrega = montar_ok(&e, &mut Falsos::default());
        for texto in [format!("{entrega:?}"), format!("{entrega:#?}")] {
            sem_segredo(&texto, "Debug da entrega");
            // Nem valores não secretos aparecem.
            assert!(!texto.contains("127.0.0.1"), "Debug com valor");
        }
        for v in entrega.variaveis() {
            sem_segredo(&format!("{v:?}"), "Debug da variável");
        }
    }
    let erros = [
        ErroEntrega::TipoSemEntrega,
        ErroEntrega::Recurso(ErroRecurso("x")),
        ErroEntrega::RecursoInvalido("y"),
        ErroEntrega::Invalida("z"),
    ];
    for erro in erros {
        sem_segredo(&format!("{erro} {erro:?}"), "erro de entrega");
    }
}

#[test]
fn defesa_conferir_recusa_lista_invalida() {
    let mut m = Montagem::default();
    m.texto("WARD_X", "a");
    m.texto("WARD_X", "b");
    assert_eq!(
        m.conferir(),
        Err(ErroEntrega::Invalida("variável repetida"))
    );

    for nome in [
        "",
        "1A",
        "a",
        "WARD-X",
        "WARD X",
        "LD_PRELOAD",
        "WARD_\u{e9}",
    ] {
        let mut m = Montagem::default();
        m.texto(nome, "v");
        assert!(m.conferir().is_err(), "{nome:?}");
    }
    let mut m = Montagem::default();
    m.texto("WARD_X", "a\u{0}b");
    assert_eq!(
        m.conferir(),
        Err(ErroEntrega::Invalida("valor com byte nulo"))
    );
}

// ---- S8 / V2.28: zera ao sair, sem Clone ----

/// Falha a compilação se `T` implementar `Clone` (dois impls viram ambíguos).
trait NaoClone<A> {
    fn conferir() {}
}
impl<T> NaoClone<()> for T {}
#[allow(dead_code)]
struct ImplementaClone;
impl<T: Clone> NaoClone<ImplementaClone> for T {}

fn zera_ao_sair<T: ZeroizeOnDrop>() {}

#[test]
fn entrega_zera_ao_sair_e_nao_clona() {
    <Entrega as NaoClone<_>>::conferir();
    <Variavel as NaoClone<_>>::conferir();
    zera_ao_sair::<Entrega>();
    zera_ao_sair::<Variavel>();
    zera_ao_sair::<Zeroizing<String>>();
}

#[test]
fn url_alocada_uma_vez_no_tamanho_exato() {
    // Sem realocação, não sobra cópia parcial da senha em memória liberada.
    let partes = [
        ("postgresql", false),
        ("://", false),
        ("app", true),
        (":", false),
        ("falso-PG:s3nha/01@#", true),
        ("@", false),
        ("127.0.0.1:55432", false),
    ];
    let url = juntar(&partes);
    assert_eq!(url.len(), url.capacity());
}

// ---- V2.37: base do banco sai igual (D33) ----

#[test]
fn v2_37_base_valida_sai_igual() {
    let pg = valida(rascunho(
        "pg",
        "acme",
        "dev",
        banco(
            "postgres",
            "db.teste.invalid",
            "",
            "acme_db-1",
            "u",
            "falso:DB/01",
        ),
    ));
    let g = montar_ok(&pg, &mut Falsos::default());
    assert_eq!(g.valor("PGDATABASE"), Some("acme_db-1"));
    assert_eq!(
        g.valor("DATABASE_URL"),
        Some("postgresql://u:falso%3ADB%2F01@db.teste.invalid:5432/acme_db-1")
    );
    let redis = valida(rascunho(
        "r",
        "acme",
        "dev",
        banco("redis", "db.teste.invalid", "", "3", "u", "falso:DB/01"),
    ));
    let g = montar_ok(&redis, &mut Falsos::default());
    for nome in ["REDIS_URL", "DATABASE_URL"] {
        assert!(
            g.valor(nome).is_some_and(|u| u.ends_with(":6379/3")),
            "{nome}"
        );
    }
}

/// V2.48 (D42): as variáveis aceitas no `api` saem no `ward run` com o token, além de
/// `WARD_TOKEN`; e o resto do ambiente não muda.
#[test]
fn v2_48_variavel_aceita_sai_com_o_token() {
    for var in [
        "GITHUB_TOKEN",
        "GH_TOKEN",
        "NPM_TOKEN",
        "OPENAI_API_KEY",
        "STRIPE_SECRET_KEY",
        "DOCKER_PASSWORD",
        "NPM_CONFIG__AUTH",
    ] {
        let e = valida(rascunho(
            "beta-github-api",
            "beta",
            "dev",
            DataDraft::Api {
                url: t("https://api.github.com"),
                token: s("falso_token_github_01"),
                variable: t(var),
            },
        ));
        let entrega = montar_ok(&e, &mut Falsos::default());
        assert_eq!(entrega.valor(var), Some("falso_token_github_01"), "{var}");
        assert_eq!(entrega.valor("WARD_TOKEN"), Some("falso_token_github_01"));
        let segredo = entrega
            .variaveis()
            .iter()
            .find(|v| v.nome() == var)
            .map(|v| v.e_segredo());
        assert_eq!(segredo, Some(true), "{var}: vai para a máscara");
        sem_segredo(&format!("{entrega:?}"), var);
    }
}
