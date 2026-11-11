//! Testes do modelo de entrada: V2.12, V2.13, V2.16 (parte de validação), V2.27 e V2.28,
//! mais os ataques à validação (injeção de opção no `ssh`, texto invisível, variável
//! perigosa, tamanhos, nível na edição). Só valores falsos do cofre de teste (D15).

use super::*;

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

/// Função que monta os dados de um tipo.
type FazDados = fn() -> DataDraft;

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

fn login() -> DataDraft {
    DataDraft::Login {
        url: t("http://localhost:8081/wp-login.php"),
        username: t("wp_admin"),
        password: s("falso-WP-senha-01"),
        totp_seed: s("JBSWY3DPEHPK3PXP"),
    }
}

fn banco() -> DataDraft {
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

fn servidor_senha() -> DataDraft {
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

fn servidor_chave() -> DataDraft {
    DataDraft::Server {
        host: t("127.0.0.1"),
        port: t("2223"),
        username: t("deploy"),
        password: None,
        private_key: Some(Secret::from_text(chave_falsa())),
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

fn nota() -> DataDraft {
    DataDraft::SecureNote {
        text: s("falso-nota-01"),
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

fn generico() -> Draft {
    let mut r = rascunho("gama-generico", "gama", "stage", DataDraft::Generic);
    r.extras = vec![
        extra("REGIAO", "r-01", false),
        extra("CHAVE_LICENCA", "falso-licenca-01", true),
    ];
    r.attachments = vec![anexo("app.env", b"FALSO_ENV=falso-anexo-01\n")];
    r
}

/// Um rascunho válido de cada tipo (servidor nas duas autenticações).
fn um_de_cada() -> Vec<Draft> {
    vec![
        rascunho("acme-wp-dev", "acme", "dev", login()),
        rascunho("acme-pg-dev", "acme", "dev", banco()),
        rascunho("acme-ssh-senha-stage", "acme", "stage", servidor_senha()),
        rascunho("acme-ssh-chave-prod", "acme", "prod", servidor_chave()),
        rascunho("beta-github-api", "beta", "dev", api()),
        rascunho("beta-aws-dev", "beta", "dev", aws()),
        rascunho("beta-smtp-stage", "beta", "stage", smtp()),
        rascunho("beta-sftp-dev", "beta", "dev", sftp()),
        rascunho("gama-nota", "gama", "dev", nota()),
        generico(),
    ]
}

/// Todos os valores secretos usados em [`um_de_cada`].
fn segredos_de_teste() -> Vec<String> {
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

fn valida(r: Draft) -> Entry {
    match r.validate() {
        Ok(e) => e,
        Err(erros) => panic!("devia validar: {erros}"),
    }
}

fn erros(r: Draft) -> ValidationErrors {
    match r.validate() {
        Ok(e) => panic!("devia recusar: {e}"),
        Err(erros) => erros,
    }
}

fn sem_segredo(texto: &str, onde: &str) {
    for segredo in segredos_de_teste() {
        assert!(!texto.contains(&segredo), "{onde} contém segredo");
    }
}

// ---- Caminho feliz ----

#[test]
fn um_de_cada_tipo_valida() {
    let entradas: Vec<Entry> = um_de_cada().into_iter().map(valida).collect();
    let tipos: Vec<EntryType> = entradas.iter().map(Entry::kind).collect();
    for tipo in EntryType::ALL {
        assert!(tipos.contains(&tipo), "falta {tipo}");
    }
    for e in &entradas {
        assert_eq!(
            e.level(),
            AccessLevel::Approval,
            "{e} não começou com aprovação"
        );
    }
}

#[test]
fn cofre_de_teste_do_contrato_valida_e_confere() {
    // As 20 entradas da tabela "Cofre de teste" (D15), com as referências de túnel e salto.
    let mut rs = um_de_cada();
    let mut wp_prod = rascunho("acme-wp-prod", "acme", "prod", login());
    if let DataDraft::Login {
        username: usuario,
        password: senha,
        ..
    } = &mut wp_prod.data
    {
        *usuario = t("wp_editor");
        *senha = s("falso-WP-senha-02");
    }
    rs.push(wp_prod);
    rs.push(rascunho("acme-pg-aprovacao", "acme", "stage", banco()));
    rs.push(rascunho("acme-pg-prod", "acme", "prod", banco()));
    let mut tunel = rascunho("acme-pg-tunel", "acme", "stage", banco());
    if let DataDraft::Database {
        host,
        port: porta,
        tunnel: t_,
        ..
    } = &mut tunel.data
    {
        *host = t("postgres");
        *porta = t("5432");
        *t_ = t("acme-ssh-senha-stage");
    }
    rs.push(tunel);
    for nome in ["acme-ssh-salto", "acme-ssh-salto-livre"] {
        let mut r = rascunho(nome, "acme", "prod", servidor_senha());
        if let DataDraft::Server {
            port: porta,
            username: usuario,
            password: senha,
            ..
        } = &mut r.data
        {
            *porta = t("2224");
            *usuario = t("salto");
            *senha = s("falso-SALTO-01");
        }
        rs.push(r);
    }
    for (nome, salto_de) in [
        ("acme-ssh-interno", "acme-ssh-salto-livre"),
        ("acme-ssh-interno-aprov", "acme-ssh-salto"),
    ] {
        let mut r = rascunho(nome, "acme", "prod", servidor_chave());
        if let DataDraft::Server {
            host,
            port: porta,
            jump: salto,
            ..
        } = &mut r.data
        {
            *host = t("ssh-interno");
            *porta = t("22");
            *salto = t(salto_de);
        }
        rs.push(r);
    }
    let mut bloq = rascunho("gama-bloqueada", "gama", "prod", login());
    if let DataDraft::Login {
        url,
        username: usuario,
        password: senha,
        totp_seed: totp_semente,
    } = &mut bloq.data
    {
        *url = t("https://bloqueada.invalid");
        *usuario = t("x");
        *senha = s("falso-BLOQ-01");
        *totp_semente = None;
    }
    rs.push(bloq);
    let mut redir = rascunho("delta-redir", "delta", "dev", login());
    if let DataDraft::Login {
        url,
        username: usuario,
        password: senha,
        totp_seed: totp_semente,
    } = &mut redir.data
    {
        *url = t("http://localhost:8084/login");
        *usuario = t("u");
        *senha = s("falso-REDIR-01");
        *totp_semente = None;
    }
    rs.push(redir);

    let entradas: Vec<Entry> = rs.into_iter().map(valida).collect();
    assert_eq!(entradas.len(), 20);
    for (i, e) in entradas.iter().enumerate() {
        let outras = entradas
            .iter()
            .enumerate()
            .filter(move |(j, _)| *j != i)
            .map(|(_, o)| o);
        e.verify_in_vault(outras)
            .unwrap_or_else(|erros| panic!("{e}: {erros}"));
    }
}

#[test]
fn porta_vazia_usa_a_padrao() {
    let mut r = rascunho("x", "acme", "dev", banco());
    if let DataDraft::Database {
        engine: motor,
        port: porta,
        db_name: base,
        ..
    } = &mut r.data
    {
        *motor = t("redis");
        *porta = String::new();
        // No redis a base é número (D33).
        *base = t("0");
    }
    let Data::Database(b) = valida(r).data else {
        unreachable!()
    };
    assert_eq!(b.port, 6379);
    for (motor, porta) in [
        ("postgres", 5432),
        ("mysql", 3306),
        ("mongo", 27017),
        ("sqlserver", 1433),
    ] {
        assert_eq!(
            Engine::from_name(motor).map(Engine::default_port),
            Some(porta)
        );
    }
}

// ---- V2.12: validação recusa com mensagem em português ----

/// Tira um campo do rascunho (o deixa vazio).
fn sem(dados: DataDraft, campo: &str) -> DataDraft {
    let mut d = dados;
    macro_rules! vazio {
        ($($f:ident),*) => {{
            $( if campo == stringify!($f) { *$f = Default::default(); } )*
        }};
    }
    match &mut d {
        DataDraft::Login {
            url,
            username: usuario,
            password: senha,
            totp_seed: totp_semente,
        } => vazio!(url, usuario, senha, totp_semente),
        DataDraft::Database {
            engine: motor,
            host,
            port: porta,
            db_name: base,
            username: usuario,
            password: senha,
            tunnel: tunel,
        } => vazio!(motor, host, porta, base, usuario, senha, tunel),
        DataDraft::Server {
            host,
            port: porta,
            username: usuario,
            password: senha,
            private_key: chave_privada,
            passphrase,
            jump: salto,
        } => vazio!(
            host,
            porta,
            usuario,
            senha,
            chave_privada,
            passphrase,
            salto
        ),
        DataDraft::Api {
            url,
            token,
            variable: variavel,
        } => vazio!(url, token, variavel),
        DataDraft::Aws {
            access_key_id,
            secret_access_key,
            region: regiao,
            role_arn,
            session_token,
        } => vazio!(
            access_key_id,
            secret_access_key,
            regiao,
            role_arn,
            session_token
        ),
        DataDraft::Smtp {
            host,
            port: porta,
            username: usuario,
            password: senha,
            security: seguranca,
        } => vazio!(host, porta, usuario, senha, seguranca),
        DataDraft::FtpSftp {
            protocol: protocolo,
            host,
            port: porta,
            username: usuario,
            password: senha,
            private_key: chave_privada,
            passphrase,
        } => vazio!(
            protocolo,
            host,
            porta,
            usuario,
            senha,
            chave_privada,
            passphrase
        ),
        DataDraft::SecureNote { text: texto } => vazio!(texto),
        DataDraft::Generic => {}
    }
    d
}

#[test]
fn v2_12_campo_obrigatorio_de_cada_tipo() {
    // (dados, campo do rascunho, campo no erro)
    let casos: Vec<(FazDados, &str, &str)> = vec![
        (login, "url", "url"),
        (login, "usuario", "usuário"),
        (login, "senha", "senha"),
        (banco, "motor", "motor"),
        (banco, "host", "host"),
        (banco, "usuario", "usuário"),
        (banco, "senha", "senha"),
        (servidor_senha, "host", "host"),
        (servidor_senha, "usuario", "usuário"),
        (servidor_senha, "senha", "autenticação"),
        (servidor_chave, "chave_privada", "autenticação"),
        (api, "token", "token"),
        (aws, "access_key_id", "access key ID"),
        (aws, "secret_access_key", "secret access key"),
        (aws, "regiao", "região"),
        (smtp, "host", "host"),
        (smtp, "usuario", "usuário"),
        (smtp, "senha", "senha"),
        (smtp, "seguranca", "segurança"),
        (sftp, "protocolo", "protocolo"),
        (sftp, "host", "host"),
        (sftp, "usuario", "usuário"),
        (sftp, "senha", "autenticação"),
        (nota, "texto", "texto"),
    ];
    for (dados, tirar, campo) in casos {
        let r = rascunho("x", "acme", "dev", sem(dados(), tirar));
        let e = erros(r);
        assert!(
            e.has(campo),
            "sem {tirar}: esperava erro em {campo}, veio {e}"
        );
        assert!(e.to_string().starts_with("entrada inválida: "), "{e}");
    }
    // Sem a chave e sem senha: nenhuma autenticação (a passphrase sozinha não vale).
    let r = rascunho(
        "x",
        "acme",
        "dev",
        sem(sem(servidor_chave(), "chave_privada"), "passphrase"),
    );
    assert!(erros(r).has_reason("autenticação", "informe a senha ou a chave"));
    // Genérico sem extras.
    let mut g = generico();
    g.extras.clear();
    assert!(erros(g).has_reason(
        "campos extras",
        "o tipo genérico precisa de pelo menos um campo extra"
    ));
    // Opcionais podem faltar.
    for (dados, tirar) in [
        (login as FazDados, "totp_semente"),
        (banco, "base"),
        (banco, "porta"),
        (banco, "tunel"),
        (servidor_chave, "passphrase"),
        (servidor_senha, "porta"),
        (api, "url"),
        (api, "variavel"),
        (aws, "role_arn"),
        (aws, "session_token"),
        (smtp, "porta"),
        (sftp, "porta"),
    ] {
        valida(rascunho("x", "acme", "dev", sem(dados(), tirar)));
    }
}

#[test]
fn v2_12_sem_grupo_sem_ambiente_e_ambiente_fora_da_lista() {
    for r in um_de_cada() {
        let tipo = r.data.kind();
        let mut sem_grupo = r;
        sem_grupo.group.clear();
        let e = erros(sem_grupo);
        assert!(e.has_reason("grupo", "obrigatório"), "{tipo}: {e}");
    }
    for r in um_de_cada() {
        let mut sem_ambiente = r;
        sem_ambiente.environment.clear();
        assert!(erros(sem_ambiente).has_reason("ambiente", "obrigatório"));
    }
    for fora in [
        "producao", "PROD", "Dev", " dev", "dev ", "staging", "qa", "prod\n",
    ] {
        let r = rascunho("x", "acme", fora, nota());
        assert!(
            erros(r).has_reason("ambiente", "deve ser dev, stage ou prod"),
            "ambiente {fora:?} passou"
        );
    }
    for ok in ["dev", "stage", "prod"] {
        assert_eq!(
            valida(rascunho("x", "acme", ok, nota()))
                .environment()
                .name(),
            ok
        );
    }
}

#[test]
fn v2_12_servidor_com_senha_e_chave_ou_sem_nenhuma() {
    let mut d = servidor_chave();
    if let DataDraft::Server {
        password: senha, ..
    } = &mut d
    {
        *senha = s("falso-SSH-senha-01");
    }
    let e = erros(rascunho("x", "acme", "dev", d));
    assert!(
        e.has_reason("autenticação", "use senha ou chave, não as duas"),
        "{e}"
    );

    let d = sem(sem(servidor_senha(), "senha"), "chave_privada");
    let e = erros(rascunho("x", "acme", "dev", d));
    assert!(
        e.has_reason("autenticação", "informe a senha ou a chave"),
        "{e}"
    );

    // Mesmo vale para ftp/sftp.
    let mut d = sftp();
    if let DataDraft::FtpSftp {
        private_key: chave_privada,
        ..
    } = &mut d
    {
        *chave_privada = Some(Secret::from_text(chave_falsa()));
    }
    assert!(erros(rascunho("x", "acme", "dev", d)).has("autenticação"));

    // Passphrase com senha não vale.
    let mut d = servidor_senha();
    if let DataDraft::Server { passphrase, .. } = &mut d {
        *passphrase = s("falso-passphrase-01");
    }
    assert!(
        erros(rascunho("x", "acme", "dev", d))
            .has_reason("passphrase", "passphrase só vale com chave")
    );

    // ftp (sem s) não aceita chave.
    let mut d = sem(sftp(), "senha");
    if let DataDraft::FtpSftp {
        protocol: protocolo,
        private_key: chave_privada,
        ..
    } = &mut d
    {
        *protocolo = t("ftp");
        *chave_privada = Some(Secret::from_text(chave_falsa()));
    }
    assert!(
        erros(rascunho("x", "acme", "dev", d))
            .has_reason("autenticação", "ftp não aceita chave; use sftp ou senha")
    );

    // Texto qualquer no lugar da chave.
    let mut d = sem(servidor_chave(), "chave_privada");
    if let DataDraft::Server {
        private_key: chave_privada,
        ..
    } = &mut d
    {
        *chave_privada = s("falso-SSH-senha-01");
    }
    assert!(erros(rascunho("x", "acme", "dev", d)).has("chave privada"));
}

#[test]
fn v2_12_banco_com_motor_fora_da_lista() {
    for motor in ["oracle", "Postgres", "postgresql", "sqlite", "postgres "] {
        let mut d = banco();
        if let DataDraft::Database { engine: m, .. } = &mut d {
            *m = t(motor);
        }
        let e = erros(rascunho("x", "acme", "dev", d));
        assert!(
            e.has_reason(
                "motor",
                "fora da lista: postgres, mysql, mongo, redis ou sqlserver"
            ),
            "motor {motor:?}: {e}"
        );
    }
    for motor in Engine::ALL {
        let mut d = banco();
        if let DataDraft::Database {
            engine: m,
            db_name: base,
            ..
        } = &mut d
        {
            *m = t(motor.name());
            if motor == Engine::Redis {
                // No redis a base é número (D33).
                *base = t("1");
            }
        }
        valida(rascunho("x", "acme", "dev", d));
    }
}

#[test]
fn v2_12_outros_formatos() {
    let casos: Vec<(DataDraft, &str)> = vec![
        (
            {
                let mut d = banco();
                if let DataDraft::Database { port: porta, .. } = &mut d {
                    *porta = t("0");
                }
                d
            },
            "porta",
        ),
        (
            {
                let mut d = banco();
                if let DataDraft::Database { port: porta, .. } = &mut d {
                    *porta = t("65536");
                }
                d
            },
            "porta",
        ),
        (
            {
                let mut d = banco();
                if let DataDraft::Database { port: porta, .. } = &mut d {
                    *porta = t("+22");
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
                if let DataDraft::Login {
                    totp_seed: totp_semente,
                    ..
                } = &mut d
                {
                    *totp_semente = s("jbswy3dpehpk3pxp");
                }
                d
            },
            "semente TOTP",
        ),
        (
            {
                let mut d = login();
                if let DataDraft::Login {
                    totp_seed: totp_semente,
                    ..
                } = &mut d
                {
                    *totp_semente = s("JBSWY3DP");
                }
                d
            },
            "semente TOTP",
        ),
        (
            {
                let mut d = smtp();
                if let DataDraft::Smtp {
                    security: seguranca,
                    ..
                } = &mut d
                {
                    *seguranca = t("ssl");
                }
                d
            },
            "segurança",
        ),
        (
            {
                let mut d = sftp();
                if let DataDraft::FtpSftp {
                    protocol: protocolo,
                    ..
                } = &mut d
                {
                    *protocolo = t("ftps");
                }
                d
            },
            "protocolo",
        ),
        (
            {
                let mut d = aws();
                if let DataDraft::Aws { region: regiao, .. } = &mut d {
                    *regiao = t("Sao Paulo");
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
                if let DataDraft::Api {
                    variable: variavel, ..
                } = &mut d
                {
                    *variavel = t("github-token");
                }
                d
            },
            "variável",
        ),
    ];
    for (dados, campo) in casos {
        let e = erros(rascunho("x", "acme", "dev", dados));
        assert!(e.has(campo), "esperava erro em {campo}: {e}");
    }
    // Nome da entrada.
    for nome in ["", "Acme", "-x", ".x", "a b", "acme/dev", &"a".repeat(65)] {
        assert!(
            erros(rascunho(nome, "acme", "dev", nota())).has("nome"),
            "{nome:?}"
        );
    }
}

#[test]
fn v2_12_junta_todos_os_erros() {
    let r = Draft {
        name: String::new(),
        group: String::new(),
        environment: t("qa"),
        project: String::new(),
        tags: vec![t("a b")],
        data: sem(sem(banco(), "senha"), "host"),
        extras: vec![extra("", "x", false)],
        attachments: vec![anexo("../x", b"y")],
    };
    let e = erros(r);
    for campo in [
        "nome",
        "grupo",
        "ambiente",
        "tags",
        "host",
        "senha",
        "campos extras",
        "anexos",
    ] {
        assert!(e.has(campo), "falta {campo}: {e}");
    }
    let tag = e.list().iter().find(|x| x.field == "tags").copied();
    assert_eq!(tag.and_then(|x| x.position), Some(1));
}

// ---- V2.13: campos secretos de cada tipo ----

#[test]
fn v2_13_campos_secretos_de_cada_tipo() {
    use SecretOrigin::{Attachment as A, Extra as X, Field as C};
    let esperado: Vec<(&str, Vec<SecretOrigin<'static>>)> = vec![
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
    let entradas: Vec<Entry> = um_de_cada().into_iter().map(valida).collect();
    for (e, (nome, origens)) in entradas.iter().zip(&esperado) {
        assert_eq!(e.name(), *nome);
        let achadas: Vec<SecretOrigin<'_>> = e.secrets().iter().map(|s| s.origin).collect();
        assert_eq!(&achadas, origens, "{nome}");
        // Os não secretos não estão na lista.
        for sd in e.secrets() {
            assert!(!sd.value.is_empty());
        }
    }
    // Os valores listados são exatamente os segredos de teste.
    let mut valores: Vec<Vec<u8>> = entradas
        .iter()
        .flat_map(|e| {
            e.secrets()
                .into_iter()
                .map(|s| s.value.expose().to_vec())
                .collect::<Vec<_>>()
        })
        .collect();
    valores.sort();
    for segredo in segredos_de_teste() {
        assert!(
            valores
                .iter()
                .any(|v| v.windows(segredo.len()).any(|w| w == segredo.as_bytes())),
            "segredo de teste fora da lista"
        );
    }
    // Ftp/sftp com chave: chave e passphrase.
    let mut d = sem(sftp(), "senha");
    if let DataDraft::FtpSftp {
        private_key: chave_privada,
        passphrase,
        ..
    } = &mut d
    {
        *chave_privada = Some(Secret::from_text(chave_falsa()));
        *passphrase = s("falso-passphrase-01");
    }
    let e = valida(rascunho("x", "beta", "dev", d));
    let origens: Vec<_> = e.secrets().iter().map(|s| s.origin).collect();
    assert_eq!(origens, vec![C("chave privada"), C("passphrase")]);
    // Extra não secreto não entra; todo anexo entra, mesmo num tipo que não é genérico.
    let mut r = rascunho("y", "acme", "dev", login());
    r.extras = vec![extra("SITE_ID", "7", false)];
    r.attachments = vec![anexo("ca-teste.pem", b"conteudo falso")];
    let e = valida(r);
    let origens: Vec<_> = e.secrets().iter().map(|s| s.origin).collect();
    assert_eq!(
        origens,
        vec![C("senha"), C("semente TOTP"), A("ca-teste.pem")]
    );
}

// ---- V2.16: nomes de extras e anexos (parte da validação) ----

#[test]
fn v2_16_normalizacao_e_repetidos() {
    assert_eq!(variable_name("api-key"), "API_KEY");
    assert_eq!(variable_name("chave acesso.pem"), "CHAVE_ACESSO_PEM");
    assert_eq!(variable_name("região"), "REGI_O");
    assert_eq!(variable_name("app.env"), "APP_ENV");

    let mut r = generico();
    r.extras = vec![extra("api-key", "1", false), extra("API_KEY", "2", true)];
    let e = erros(r);
    assert!(
        e.has_reason(
            "campos extras",
            "vira a mesma variável de outro campo extra"
        ),
        "{e}"
    );
    assert_eq!(e.list()[0].position, Some(2));

    let mut r = generico();
    r.extras.push(extra("api key", "1", false));
    r.extras.push(extra("Api.Key", "2", false));
    assert!(erros(r).has("campos extras"));

    let mut r = generico();
    r.attachments = vec![
        anexo("chave acesso.pem", b"a"),
        anexo("chave-acesso.pem", b"b"),
    ];
    assert!(erros(r).has_reason("anexos", "vira a mesma variável de outro anexo"));

    let mut r = generico();
    r.extras = vec![extra("api-key", "1", false)];
    r.attachments = vec![anexo("chave acesso.pem", b"a")];
    let e = valida(r);
    assert_eq!(e.extras()[0].name(), "api-key");
    assert_eq!(e.attachments()[0].name(), "chave acesso.pem");
}

// ---- V2.27 (SEG): nenhuma formatação mostra segredo ----

#[test]
fn v2_27_debug_e_display_sem_segredo() {
    for r in um_de_cada() {
        sem_segredo(&format!("{r:?}"), "Debug do rascunho");
        sem_segredo(&format!("{r:#?}"), "Debug alternativo do rascunho");
        let e = valida(r);
        let saidas = [
            format!("{e:?}"),
            format!("{e:#?}"),
            format!("{e}"),
            format!("{:?}", e.data()),
            format!("{:?}", e.extras()),
            format!("{:?}", e.attachments()),
            format!("{:?}", e.secrets()),
        ];
        for saida in &saidas {
            sem_segredo(saida, "formatação da entrada");
        }
        assert!(saidas[0].contains("••••") || e.secrets().is_empty());
        for sd in e.secrets() {
            assert_eq!(format!("{:?}", sd.value), "Secret(••••)");
            assert_eq!(format!("{}", sd.value), "••••");
            assert_eq!(format!("{:>20}", sd.value), "••••");
        }
    }
}

#[test]
fn v2_27_erros_nao_carregam_valor_do_usuario() {
    // Ataque: o segredo vai parar em campos errados (nome, host, usuário, URL, extra,
    // tag, motor, ambiente…) e em campos secretos inválidos. Nenhum erro o repete.
    let alvo = "falso-PG:s3nha/01@#";
    let invisivel = format!("{alvo}\u{202E}");
    let mut d = banco();
    if let DataDraft::Database {
        engine: motor,
        host,
        port: porta,
        db_name: base,
        username: usuario,
        password: senha,
        tunnel: tunel,
    } = &mut d
    {
        *motor = t(alvo);
        *host = t(alvo);
        *porta = t(alvo);
        *base = invisivel.clone();
        *usuario = format!("-{alvo}");
        *senha = Some(Secret::from_text(format!("{alvo}\0")));
        *tunel = t(alvo);
    }
    let r = Draft {
        name: t(alvo),
        group: invisivel.clone(),
        environment: t(alvo),
        project: invisivel.clone(),
        tags: vec![t(alvo), format!("{alvo} x")],
        data: d,
        extras: vec![extra(alvo, alvo, false), extra(&invisivel, alvo, true)],
        attachments: vec![anexo(&format!("../{alvo}"), alvo.as_bytes())],
    };
    let e = erros(r);
    assert!(e.list().len() >= 10, "{e}");
    for texto in [format!("{e}"), format!("{e:?}"), format!("{e:#?}")] {
        sem_segredo(&texto, "erro de validação");
        assert!(!texto.contains("s3nha"));
    }
    for x in e.list() {
        sem_segredo(&x.to_string(), "erro de validação");
    }
    // Mesmo nos erros de conferência no cofre.
    let a = valida(rascunho("a", "acme", "dev", banco()));
    let outra = [valida(rascunho("a", "acme", "dev", nota()))];
    let erro = a.verify_in_vault(&outra).expect_err("nome repetido");
    sem_segredo(&format!("{erro} {erro:?}"), "erro de conferência");
}

// ---- V2.28 (SEG): zera ao sair, sem Clone ----

/// Falha a compilação se `T` implementar `Clone` (dois impls viram ambíguos).
trait NaoClone<A> {
    fn conferir() {}
}
impl<T> NaoClone<()> for T {}
#[allow(dead_code)]
struct ImplementaClone;
impl<T: Clone> NaoClone<ImplementaClone> for T {}

fn zera_ao_sair<T: zeroize::ZeroizeOnDrop>() {}

#[test]
fn v2_28_tipos_de_segredo_zeram_e_nao_clonam() {
    // Em tempo de compilação: se um destes ganhar `Clone`, o teste não compila.
    <Secret as NaoClone<_>>::conferir();
    <Entry as NaoClone<_>>::conferir();
    <Data as NaoClone<_>>::conferir();
    <Login as NaoClone<_>>::conferir();
    <Database as NaoClone<_>>::conferir();
    <Server as NaoClone<_>>::conferir();
    <Auth as NaoClone<_>>::conferir();
    <Api as NaoClone<_>>::conferir();
    <Aws as NaoClone<_>>::conferir();
    <Smtp as NaoClone<_>>::conferir();
    <FtpSftp as NaoClone<_>>::conferir();
    <SecureNote as NaoClone<_>>::conferir();
    <ExtraField as NaoClone<_>>::conferir();
    <ExtraValue as NaoClone<_>>::conferir();
    <Attachment as NaoClone<_>>::conferir();
    <Draft as NaoClone<_>>::conferir();
    <DataDraft as NaoClone<_>>::conferir();
    <ExtraDraft as NaoClone<_>>::conferir();
    <AttachmentDraft as NaoClone<_>>::conferir();
    <crate::format::MasterPassword as NaoClone<_>>::conferir();
    <crate::format::OpenVault as NaoClone<_>>::conferir();
    <crate::format::VaultKeys as NaoClone<_>>::conferir();
    // Configuração do corpo v2 (Fase 19b, D30).
    <crate::configuracao::HashPin as NaoClone<_>>::conferir();
    <crate::configuracao::Configuracao as NaoClone<_>>::conferir();

    zera_ao_sair::<Secret>();
    zera_ao_sair::<Entry>();
    zera_ao_sair::<Data>();
    zera_ao_sair::<Auth>();
    zera_ao_sair::<ExtraField>();
    zera_ao_sair::<Attachment>();
    // A chave de dados do formato (Fase 13) é `Zeroizing<[u8; 32]>`.
    zera_ao_sair::<crate::crypto::Key>();
    // A senha mestra e o cofre aberto (que guarda a chave de dados), Fase 15a.
    zera_ao_sair::<crate::format::MasterPassword>();
    zera_ao_sair::<crate::format::OpenVault>();
    zera_ao_sair::<crate::format::VaultKeys>();
    // O hash do PIN e a configuração que o guarda (Fase 19b, D30).
    zera_ao_sair::<crate::configuracao::HashPin>();
    zera_ao_sair::<crate::configuracao::Configuracao>();

    assert!(core::mem::needs_drop::<Secret>());
}

// ---- Ataques à validação ----

#[test]
fn ataque_host_e_usuario_viram_opcao_do_ssh() {
    // `ward ssh` põe host e usuário no argv do `ssh`: começar com '-' viraria opção
    // (ex.: -oProxyCommand=…, execução de comando).
    for host in [
        "-oProxyCommand=touch /tmp/x",
        "-p",
        "host com espaço",
        "a;b",
        "a\nb",
    ] {
        let mut d = servidor_senha();
        if let DataDraft::Server { host: h, .. } = &mut d {
            *h = t(host);
        }
        assert!(
            erros(rascunho("x", "acme", "dev", d)).has("host"),
            "{host:?}"
        );
    }
    for usuario in ["-oProxyCommand=id", "-l", "de ploy"] {
        for dados in [servidor_senha(), sftp(), banco(), smtp()] {
            let mut d = dados;
            match &mut d {
                DataDraft::Server { username: u, .. }
                | DataDraft::FtpSftp { username: u, .. }
                | DataDraft::Database { username: u, .. }
                | DataDraft::Smtp { username: u, .. } => *u = t(usuario),
                _ => unreachable!(),
            }
            assert!(
                erros(rascunho("x", "acme", "dev", d)).has("usuário"),
                "{usuario:?}"
            );
        }
    }
    // Salto e túnel com nome que viraria opção.
    let mut d = servidor_senha();
    if let DataDraft::Server { jump: salto, .. } = &mut d {
        *salto = t("-J");
    }
    assert!(erros(rascunho("x", "acme", "dev", d)).has("servidor de salto"));
}

#[test]
fn ataque_texto_invisivel_forja_a_janela_de_aprovacao() {
    // Bidi override, largura zero, separador de linha e controle: o nome exibido na
    // janela de aprovação e na lista não pode mentir.
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
        let falso = format!("acme{c}prod");
        let mut r = rascunho("x", &falso, "dev", nota());
        r.project = falso.clone();
        r.extras = vec![extra(&falso, "v", false), extra("OK", &falso, false)];
        r.attachments = vec![anexo(&falso, b"y")];
        let e = erros(r);
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
    // Nome com espaço nas pontas também.
    assert!(erros(rascunho("x", " acme", "dev", nota())).has("grupo"));
    // Extra secreto pode ter quebra de linha (vai para o ambiente), mas não nulo.
    let mut r = generico();
    r.extras.push(extra("MULTI", "linha1\nlinha2", true));
    valida(r);
    let mut r = generico();
    r.extras.push(extra("NULO", "a\0b", true));
    assert!(erros(r).has_reason("campos extras", "não pode ter o caractere nulo"));
}

#[test]
fn ataque_segredo_com_nulo_ou_binario() {
    let mut d = login();
    if let DataDraft::Login {
        password: senha, ..
    } = &mut d
    {
        *senha = s("falso\0WP");
    }
    assert!(
        erros(rascunho("x", "acme", "dev", d)).has_reason("senha", "não pode ter o caractere nulo")
    );
    let mut d = api();
    if let DataDraft::Api { token, .. } = &mut d {
        *token = Some(Secret::from_bytes(vec![0xff, 0xfe, b'a']));
    }
    assert!(
        erros(rascunho("x", "acme", "dev", d)).has_reason("token", "precisa ser texto (UTF-8)")
    );
    // Anexo pode ser binário.
    let mut r = generico();
    r.attachments = vec![anexo("cert.p12", &[0, 1, 2, 0xff])];
    valida(r);
}

#[test]
fn ataque_variavel_perigosa_no_api() {
    for var in [
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "PATH",
        "BASH_ENV",
        "SSHPASS",
        "GIT_SSH_COMMAND",
        "NODE_OPTIONS",
        "WARD_PASS",
        "WARD_ENTRADA",
        "BASH_FUNC_x%%",
        "IFS",
    ] {
        let mut d = api();
        if let DataDraft::Api {
            variable: variavel, ..
        } = &mut d
        {
            *variavel = t(var);
        }
        assert!(
            erros(rascunho("x", "beta", "dev", d)).has("variável"),
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
        if let DataDraft::Api {
            variable: variavel, ..
        } = &mut d
        {
            *variavel = t(var);
        }
        valida(rascunho("x", "beta", "dev", d));
    }
}

#[test]
fn ataque_extra_ou_anexo_sobrescreve_variavel_do_tipo() {
    // Um extra "PASS" (não secreto) num login viraria WARD_PASS, por cima da senha.
    let reservados: Vec<(FazDados, &str)> = vec![
        (login, "pass"),
        (login, "TOTP"),
        (login, "url"),
        (servidor_senha, "host"),
        (servidor_senha, "passphrase"),
        (sftp, "protocolo"),
        (api, "token"),
        (nota, "entrada"),
        (aws, "GRUPO"),
        (smtp, "ambiente"),
        (banco, "arquivo_app_env"),
        (banco, "arquivo"),
    ];
    for (dados, nome) in reservados {
        let mut r = rascunho("x", "acme", "dev", dados());
        r.extras = vec![extra(nome, "v", false)];
        assert!(
            erros(r).has_reason("campos extras", "vira uma variável que o tipo já entrega"),
            "{nome}"
        );
    }
    // Anexo "chave" no servidor viraria WARD_ARQUIVO_CHAVE, a chave da entrada.
    for dados in [servidor_senha(), sftp()] {
        let mut r = rascunho("x", "acme", "dev", dados);
        r.attachments = vec![anexo("chave", b"x")];
        assert!(erros(r).has("anexos"));
    }
    // Em outros tipos, "PASS" e "chave" são nomes comuns.
    let mut r = generico();
    r.extras.push(extra("PASS", "v", true));
    r.attachments.push(anexo("chave", b"x"));
    valida(r);
}

#[test]
fn ataque_nome_de_anexo_sai_do_diretorio() {
    for nome in [
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
        let mut r = generico();
        r.attachments = vec![anexo(nome, b"x")];
        assert!(erros(r).has("anexos"), "{nome:?}");
    }
    let mut r = generico();
    r.attachments = vec![anexo("vazio.txt", b"")];
    assert!(erros(r).has_reason("anexos", "anexo vazio"));
}

#[test]
fn ataque_tamanhos_excessivos() {
    let grande = |n: usize| Some(Secret::from_bytes(vec![b'a'; n]));
    let mut d = login();
    if let DataDraft::Login {
        password: senha, ..
    } = &mut d
    {
        *senha = grande(64 * 1024 + 1);
    }
    assert!(erros(rascunho("x", "acme", "dev", d)).has_reason("senha", "longo demais"));
    let d = DataDraft::SecureNote {
        text: grande(1024 * 1024 + 1),
    };
    assert!(erros(rascunho("x", "acme", "dev", d)).has_reason("texto", "longo demais"));
    let d = DataDraft::SecureNote {
        text: grande(1024 * 1024),
    };
    valida(rascunho("x", "acme", "dev", d));
    let mut d = login();
    if let DataDraft::Login {
        totp_seed: totp_semente,
        ..
    } = &mut d
    {
        *totp_semente = Some(Secret::from_bytes(vec![b'A'; 129]));
    }
    assert!(erros(rascunho("x", "acme", "dev", d)).has("semente TOTP"));

    let mut r = generico();
    r.attachments = vec![AttachmentDraft {
        name: t("grande.bin"),
        content: Secret::from_bytes(vec![0; crate::limits::MAX_ATTACHMENTS_TOTAL + 1]),
    }];
    assert!(erros(r).has_reason("anexos", "longo demais"));

    let mut r = generico();
    r.extras = (0..65)
        .map(|i| extra(&format!("E{i}"), "v", false))
        .collect();
    assert!(erros(r).has_reason("campos extras", "itens demais"));
    let mut r = generico();
    r.attachments = (0..33).map(|i| anexo(&format!("a{i}"), b"v")).collect();
    assert!(erros(r).has_reason("anexos", "itens demais"));
    let mut r = generico();
    r.tags = (0..17).map(|i| format!("t{i}")).collect();
    assert!(erros(r).has_reason("tags", "itens demais"));
    let mut r = generico();
    r.tags = vec![t("a"), t("a")];
    assert!(erros(r).has_reason("tags", "tag repetida"));
    let mut r = generico();
    r.group = "g".repeat(257);
    assert!(erros(r).has_reason("grupo", "longo demais"));
    let mut d = login();
    if let DataDraft::Login { url, .. } = &mut d {
        *url = format!("https://a/{}", "b".repeat(2048));
    }
    assert!(erros(rascunho("x", "acme", "dev", d)).has_reason("url", "longo demais"));
}

#[test]
fn ataque_nivel_nao_muda_ao_salvar_nem_ao_editar() {
    // S9: entrada nova sempre "com aprovação" (o rascunho não tem nível).
    let mut atual = valida(rascunho("acme-pg-prod", "acme", "prod", banco()));
    assert_eq!(atual.level(), AccessLevel::Approval);
    // O daemon (com PIN) bloqueia; uma edição sem PIN não pode desbloquear.
    atual.set_level(AccessLevel::Blocked);
    let editada = valida(rascunho("acme-pg-prod", "acme", "prod", banco()));
    assert_eq!(editada.level(), AccessLevel::Approval);
    atual.replace_with(editada).expect("mesmo tipo");
    assert_eq!(atual.level(), AccessLevel::Blocked);
    // O tipo não muda na edição.
    let outra = valida(rascunho("acme-pg-prod", "acme", "prod", nota()));
    let e = atual.replace_with(outra).expect_err("tipo mudou");
    assert!(e.has_reason("tipo", "o tipo não muda depois de criado"));
    assert_eq!(atual.kind(), EntryType::Database);
}

#[test]
fn nivel_combinado_e_nomes() {
    use AccessLevel::*;
    assert_eq!(Free.most_restrictive(Approval), Approval);
    assert_eq!(Approval.most_restrictive(Free), Approval);
    assert_eq!(Free.most_restrictive(Blocked), Blocked);
    assert_eq!(Free.most_restrictive(Free), Free);
    assert_eq!(AccessLevel::default(), Approval);
    for n in AccessLevel::ALL {
        assert_eq!(AccessLevel::from_name(n.name()), Some(n));
    }
    for tipo in EntryType::ALL {
        assert_eq!(EntryType::from_name(tipo.name()), Some(tipo));
    }
    assert_eq!(EntryType::FtpSftp.label(), "ftp/sftp");
    assert_eq!(EntryType::SecureNote.label(), "nota segura");
    assert_eq!(EntryType::Generic.label(), "genérico");
    assert_eq!(Approval.label(), "com aprovação");
}

// ---- Conferência no cofre (nome único, túnel e salto) ----

#[test]
fn conferir_no_cofre_referencias() {
    let servidor = |nome: &str, salto: &str| {
        let mut d = servidor_senha();
        if let DataDraft::Server { jump: s_, .. } = &mut d {
            *s_ = t(salto);
        }
        valida(rascunho(nome, "acme", "dev", d))
    };
    let banco_por = |nome: &str, tunel: &str| {
        let mut d = banco();
        if let DataDraft::Database { tunnel: t_, .. } = &mut d {
            *t_ = t(tunel);
        }
        valida(rascunho(nome, "acme", "dev", d))
    };
    let cofre = vec![
        servidor("salto", ""),
        servidor("interno", "salto"),
        valida(rascunho("site", "acme", "dev", login())),
    ];

    // Ok: túnel e salto para um servidor sem salto.
    banco_por("pg", "salto")
        .verify_in_vault(&cofre)
        .expect("ok");
    servidor("novo", "salto")
        .verify_in_vault(&cofre)
        .expect("ok");

    let casos = [
        (
            servidor("salto", ""),
            "nome",
            "já existe outra entrada com este nome",
        ),
        (
            banco_por("pg", "nao-existe"),
            "túnel SSH",
            "precisa ser uma entrada servidor existente",
        ),
        (
            banco_por("pg", "site"),
            "túnel SSH",
            "precisa ser uma entrada servidor existente",
        ),
        (
            banco_por("pg", "interno"),
            "túnel SSH",
            "o servidor do túnel não pode ter salto",
        ),
        (
            servidor("eu", "eu"),
            "servidor de salto",
            "aponta para a própria entrada",
        ),
        (
            servidor("x", "interno"),
            "servidor de salto",
            "o servidor de salto não pode ter salto próprio",
        ),
        (
            servidor("x", "site"),
            "servidor de salto",
            "precisa ser uma entrada servidor existente",
        ),
    ];
    for (e, campo, motivo) in casos {
        let erro = e.verify_in_vault(&cofre).expect_err(motivo);
        assert!(erro.has_reason(campo, motivo), "{erro}");
    }

    // Um servidor usado como salto não pode ganhar salto próprio (viraria cadeia).
    let salto_editado = servidor("salto", "outro");
    let resto: Vec<Entry> = vec![servidor("interno", "salto"), servidor("outro", "")];
    let erro = salto_editado.verify_in_vault(&resto).expect_err("cadeia");
    assert!(erro.has_reason(
        "servidor de salto",
        "outra entrada usa este servidor como salto ou túnel; ele não pode ter salto"
    ));
}

// ---- D29, D33 (Fase 19a) ----

/// Anexos de `tamanhos` bytes, com conteúdo que não vira padrão por linha (sem `\n`).
fn anexos_de(tamanhos: &[usize]) -> Vec<AttachmentDraft> {
    tamanhos
        .iter()
        .enumerate()
        .map(|(i, &n)| anexo(&format!("a{i}.bin"), &vec![0x5a; n]))
        .collect()
}

#[test]
fn v2_36_anexos_somados_e_orcamento_ao_salvar() {
    use crate::limits::{MAX_ATTACHMENTS_TOTAL, MAX_EXTRA_VALUE, MAX_EXTRAS};
    const KIB: usize = 1024;

    // Um anexo de 256 KiB + 1 byte.
    let mut r = generico();
    r.attachments = anexos_de(&[MAX_ATTACHMENTS_TOTAL + 1]);
    assert!(erros(r).has_reason("anexos", "longo demais"));
    // Dois anexos de 200 KiB: cada um cabe, a soma não.
    let mut r = generico();
    r.attachments = anexos_de(&[200 * KIB, 200 * KIB]);
    let e = erros(r);
    assert!(
        e.has_reason("anexos", "os anexos somam mais de 256 KiB"),
        "{e}"
    );
    // A soma exata no limite passa (com um anexo e com vários).
    let mut r = generico();
    r.attachments = anexos_de(&[MAX_ATTACHMENTS_TOTAL]);
    valida(r);
    let mut r = generico();
    r.attachments = anexos_de(&[MAX_ATTACHMENTS_TOTAL / 2, MAX_ATTACHMENTS_TOTAL / 2]);
    valida(r);

    // `api` com 64 extras secretos de 64 KiB: cada campo cabe, a máscara não.
    let mut r = rascunho("beta-github-api", "beta", "dev", api());
    r.extras = (0..MAX_EXTRAS)
        .map(|i| ExtraDraft {
            name: format!("e{i:02}"),
            value: Secret::from_bytes(vec![b'k'; MAX_EXTRA_VALUE]),
            secret: true,
        })
        .collect();
    let e = erros(r);
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

    // A `nota segura` fica fora do orçamento (não tem entrega, D10).
    let mut r = rascunho("gama-nota", "gama", "dev", nota());
    r.extras = (0..MAX_EXTRAS)
        .map(|i| ExtraDraft {
            name: format!("e{i:02}"),
            value: Secret::from_bytes(vec![b'k'; MAX_EXTRA_VALUE]),
            secret: true,
        })
        .collect();
    valida(r);
}

#[test]
fn v2_37_base_do_banco() {
    let com_base = |motor: &str, base: &str| {
        let mut d = banco();
        if let DataDraft::Database {
            engine: m,
            db_name: b,
            ..
        } = &mut d
        {
            *m = t(motor);
            *b = t(base);
        }
        rascunho("acme-pg-dev", "acme", "dev", d)
    };
    let longa = "b".repeat(crate::limits::MAX_DB_NAME + 1);
    for motor in ["postgres", "mysql", "mongo", "sqlserver"] {
        for base in [
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
            longa.as_str(),
        ] {
            let e = erros(com_base(motor, base));
            assert!(e.has("base"), "{motor} {base:?}");
            // Mensagem em português, sem o valor.
            assert!(!e.to_string().contains(base), "{motor} {base:?}");
        }
        let e = erros(com_base(motor, "b?x=1"));
        assert!(e.has_reason(
            "base",
            "use letras, números, '_' ou '-', com até 63 caracteres"
        ));
        let e = erros(com_base(motor, &longa));
        assert!(e.has_reason("base", "longo demais"));
        for base in [
            "acme_db-1",
            "A",
            "app_teste",
            &"b".repeat(crate::limits::MAX_DB_NAME),
        ] {
            let entrada = valida(com_base(motor, base));
            let Data::Database(b) = entrada.data() else {
                panic!("banco")
            };
            assert_eq!(b.db_name.as_deref(), Some(base));
        }
        // Base vazia segue opcional.
        valida(com_base(motor, ""));
    }
    for base in [
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
        let e = erros(com_base("redis", base));
        assert!(e.has("base"), "redis {base:?}");
    }
    let e = erros(com_base("redis", "abc"));
    assert!(e.has_reason("base", "no redis, a base é um número de 0 a 99999"));
    for base in ["0", "3", "15", "99999"] {
        valida(com_base("redis", base));
    }
    valida(com_base("redis", ""));
}

#[test]
fn mensagens_citam_os_limites_atuais() {
    // Se um limite mudar em `crate::limits`, a mensagem que o cita tem de mudar junto.
    use crate::limits::{
        MAX_ATTACHMENTS_TOTAL, MAX_DB_NAME, MAX_REDIS_DB_NAME, MAX_TOTP, MIN_TOTP,
    };
    let somados = format!("{} KiB", MAX_ATTACHMENTS_TOTAL / 1024);
    let base = format!("até {MAX_DB_NAME} caracteres");
    let redis = format!("de 0 a {MAX_REDIS_DB_NAME}");
    let totp = format!("com {MIN_TOTP} a {MAX_TOTP} caracteres");
    let mut r = generico();
    r.attachments = anexos_de(&[MAX_ATTACHMENTS_TOTAL, 1]);
    assert!(erros(r).to_string().contains(&somados));
    let mut d = banco();
    if let DataDraft::Database { db_name: b, .. } = &mut d {
        *b = t("b.c");
    }
    assert!(
        erros(rascunho("x", "acme", "dev", d))
            .to_string()
            .contains(&base)
    );
    let mut d = banco();
    if let DataDraft::Database {
        engine: motor,
        db_name: b,
        ..
    } = &mut d
    {
        *motor = t("redis");
        *b = t("x");
    }
    assert!(
        erros(rascunho("x", "acme", "dev", d))
            .to_string()
            .contains(&redis)
    );
    let mut d = login();
    if let DataDraft::Login {
        totp_seed: totp_semente,
        ..
    } = &mut d
    {
        *totp_semente = s("abc1abc1abc1abc1");
    }
    assert!(
        erros(rascunho("x", "acme", "dev", d))
            .to_string()
            .contains(&totp)
    );
}

// ---- Fase 19c: D38, D41, D42, D43 ----

/// Caracteres que o repro `validacao.rs` do auditor mostrou passando (achado médio 3),
/// mais os de V2.45: formato (Cf), preenchimento Hangul, braille em branco, seletor de
/// variação, espaços fora do ASCII, combinante e emoji.
const FORA_DA_LISTA: &[&str] = &[
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

/// O erro não traz o texto recusado (nem pelo `Display`, nem pelo `Debug`).
fn sem_o_valor(e: &ValidationErrors, valor: &str) {
    for texto in [format!("{e}"), format!("{e:?}")] {
        assert!(!texto.contains(valor), "erro com o valor: {valor:?}");
    }
}

/// V2.45 (D38): o texto de exibição é lista branca (ASCII visível e o Latin-1/Latin
/// Extended-A dos acentos); tudo fora dela é recusado em todo campo que a janela de
/// aprovação e a lista mostram.
#[test]
fn v2_45_texto_de_exibicao_e_lista_branca() {
    for c in FORA_DA_LISTA {
        let falso = format!("acme{c}prod");
        let mut d = login();
        if let DataDraft::Login {
            username: usuario, ..
        } = &mut d
        {
            *usuario = falso.clone();
        }
        let mut r = rascunho("x", &falso, "dev", d);
        r.project = falso.clone();
        r.tags = vec![falso.clone()];
        r.extras = vec![extra(&falso, "v", false)];
        r.attachments = vec![anexo(&falso, b"y")];
        let e = erros(r);
        for campo in ["grupo", "projeto", "usuário"] {
            assert!(
                e.has_reason(campo, "caractere não permitido"),
                "{campo} {c:?}"
            );
        }
        assert!(e.has("tags"), "{c:?}");
        assert!(e.has("campos extras"), "{c:?}");
        assert!(e.has("anexos"), "{c:?}");
        sem_o_valor(&e, c);
        // Nome da entrada também.
        assert!(
            erros(rascunho(&falso, "acme", "dev", nota())).has("nome"),
            "{c:?}"
        );
    }
    for bom in [
        "São Paulo",
        "ação",
        "Müller",
        "Ñandú",
        "Łódź",
        "acme-prod_01 (v2)",
    ] {
        let mut r = rascunho("x", bom, "dev", nota());
        r.project = t(bom);
        valida(r);
    }
    // Segredo não é texto de exibição: senha com U+200B e U+3164 passa.
    let mut d = login();
    if let DataDraft::Login {
        password: senha, ..
    } = &mut d
    {
        *senha = s("falso\u{200B}senha\u{3164}01");
    }
    valida(rascunho("x", "acme", "dev", d));
}

/// V2.47 (D41), achado médio 4: a URL do `login` e do `api` passa por um parser estrito.
#[test]
fn v2_47_url_estrita() {
    let recusadas = [
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
    for url in recusadas {
        let mut d = login();
        if let DataDraft::Login { url: u, .. } = &mut d {
            *u = t(url);
        }
        let e = erros(rascunho("x", "acme", "dev", d));
        assert!(e.has("url"), "login {url:?}");
        let msg = format!("{e}");
        assert!(
            msg.contains("use https://") || msg.contains("caractere não permitido"),
            "{url:?}: {msg}"
        );
        // "https://" sozinho é também o começo da mensagem, que o explica.
        if url != "https://" {
            sem_o_valor(&e, url);
        }
        let mut d = api();
        if let DataDraft::Api { url: u, .. } = &mut d {
            *u = t(url);
        }
        assert!(
            erros(rascunho("x", "beta", "dev", d)).has("url"),
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
        valida(rascunho("x", "acme", "dev", d));
        let mut d = api();
        if let DataDraft::Api { url: u, .. } = &mut d {
            *u = t(url);
        }
        valida(rascunho("x", "beta", "dev", d));
    }
}

/// Variáveis que mudam o que um processo filho executa ou para onde ele manda os dados
/// (o repro do auditor, achado médio 6) e as de V2.48.
const VARIAVEIS_PERIGOSAS: &[&str] = &[
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

/// As que passam (V2.48): terminam num sufixo de credencial.
const VARIAVEIS_BOAS: &[&str] = &[
    "GITHUB_TOKEN",
    "GH_TOKEN",
    "NPM_TOKEN",
    "OPENAI_API_KEY",
    "STRIPE_SECRET_KEY",
    "DOCKER_PASSWORD",
    "NPM_CONFIG__AUTH",
    "SONAR_PAT",
];

/// V2.48 (D42): a variável do `api` precisa do sufixo de credencial; a lista negra
/// segue valendo por cima.
#[test]
fn v2_48_variavel_do_api_com_sufixo() {
    for var in VARIAVEIS_PERIGOSAS {
        let mut d = api();
        if let DataDraft::Api {
            variable: variavel, ..
        } = &mut d
        {
            *variavel = t(var);
        }
        let e = erros(rascunho("x", "beta", "dev", d));
        assert!(e.has("variável"), "{var}");
        sem_o_valor(&e, var);
    }
    for var in VARIAVEIS_BOAS {
        let mut d = api();
        if let DataDraft::Api {
            variable: variavel, ..
        } = &mut d
        {
            *variavel = t(var);
        }
        valida(rascunho("x", "beta", "dev", d));
    }
}

/// V2.49 (D43), achado baixo 8: o usuário do `servidor` e do `ftp/sftp` vai para o
/// `ssh`/`sftp`. `@` passa (conta de domínio, decisão do usuário); `;`, `$(…)`, `%h`,
/// espaço, barra invertida, acento e `-` no começo, não.
#[test]
fn v2_49_usuario_do_ssh() {
    let ruins = [
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
    for usuario in ruins {
        for dados in [servidor_senha(), sftp()] {
            let mut d = dados;
            match &mut d {
                DataDraft::Server { username: u, .. } | DataDraft::FtpSftp { username: u, .. } => {
                    *u = t(usuario)
                }
                _ => unreachable!(),
            }
            let e = erros(rascunho("x", "acme", "dev", d));
            assert!(e.has("usuário"), "{usuario:?}");
            // "usuário" é também o nome do campo, que o erro traz.
            if usuario != "usuário" {
                sem_o_valor(&e, usuario);
            }
        }
    }
    for usuario in [
        "deploy",
        "app_user",
        "j.silva",
        "www-data",
        "deploy@cliente.com.br",
        "a",
    ] {
        for dados in [servidor_senha(), sftp()] {
            let mut d = dados;
            match &mut d {
                DataDraft::Server { username: u, .. } | DataDraft::FtpSftp { username: u, .. } => {
                    *u = t(usuario)
                }
                _ => unreachable!(),
            }
            valida(rascunho("x", "acme", "dev", d));
        }
    }
    // Até o limite de usuário de `limites`.
    let mut d = servidor_senha();
    if let DataDraft::Server {
        username: usuario, ..
    } = &mut d
    {
        *usuario = "u".repeat(crate::limits::MAX_USERNAME + 1);
    }
    assert!(erros(rascunho("x", "acme", "dev", d)).has("usuário"));
}

/// V2.58 (D55, repro `url.rs` do auditor): o último rótulo de domínio que começa por
/// dígito é recusado, porque o Chrome e o `curl` leem `0x7f000001`, `0x7f.0.0.1` e
/// `1.2.3.0x4` como IPv4 (formas hex e numéricas do WHATWG) e `x.123` não é domínio.
/// A mensagem é a de URL inválida, sem o valor. Os outros casos do repro que já eram
/// recusados seguem recusados; os aceitos da V2.47 e os da V2.58 passam.
#[test]
fn v2_58_ultimo_rotulo_com_digito() {
    let recusadas = [
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
        // Do repro, já recusadas antes de D55.
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
    for url in recusadas {
        let mut d = login();
        if let DataDraft::Login { url: u, .. } = &mut d {
            *u = t(url);
        }
        let e = erros(rascunho("x", "acme", "dev", d));
        assert!(e.has("url"), "login {url:?}");
        let msg = format!("{e}");
        assert!(
            msg.contains("use https://") || msg.contains("caractere não permitido"),
            "{url:?}: {msg}"
        );
        sem_o_valor(&e, url);
        let mut d = api();
        if let DataDraft::Api { url: u, .. } = &mut d {
            *u = t(url);
        }
        assert!(
            erros(rascunho("x", "beta", "dev", d)).has("url"),
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
        valida(rascunho("x", "acme", "dev", d));
        let mut d = api();
        if let DataDraft::Api { url: u, .. } = &mut d {
            *u = t(url);
        }
        valida(rascunho("x", "beta", "dev", d));
    }
}

/// Fase 44 (D9): a origem inteira de uma URL, para o `ward playwright` comparar com a da
/// página. Ataques: subdomínio, porta, esquema, userinfo disfarçado e `http` fora do
/// `localhost` dão outra origem ou nenhuma.
#[test]
fn d9_origem_da_url() {
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
    for outra in [
        "https://www.acme.com.br/login",
        "https://acme.com.br.evil.io/login",
        "https://acme.com.br:444/login",
        "http://localhost:8082/wp-login.php",
    ] {
        assert_ne!(o(outra), base, "{outra}");
    }
    for ruim in [
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
        assert_eq!(o(ruim), None, "{ruim}");
    }
}

/// Fase 44e (D9, 2ª auditoria do M6): todo host de loopback é origem local, em
/// qualquer esquema. Ataque: `https://localhost`, `https://127.0.0.1` ou `https://[::1]`
/// com um certificado de CA local (mkcert, `~/.pki`) iria sem janela; as formas
/// disfarçadas do loopback também contam.
#[test]
fn ataque_origem_local_em_qualquer_esquema() {
    let o = local_url_origin;
    for (url, origem) in [
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
        assert_eq!(o(url).as_deref(), Some(origem), "{url}");
    }
    for fora in [
        "https://acme.com.br/login",
        "https://localhost.acme.com.br/",
        "https://localhostx/",
        "https://xlocalhost/",
        "https://128.0.0.1/",
        "https://10.0.0.1/",
        "https://[::2]/",
        "https://[::ffff:8.8.8.8]/",
        "https://[2001:db8::1]/",
        // Recusadas pelo parser: não têm origem nenhuma (e nada é usado).
        "https://LOCALHOST/",
        "https://0x7f000001/",
        "http://evil.io/",
        "",
    ] {
        assert_eq!(o(fora), None, "{fora}");
    }
}
