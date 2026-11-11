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

use super::wc::configuracao::{Configuracao, HashPin};
use super::wc::entrada::{
    Entrada, NivelAcesso, Rascunho, RascunhoAnexo, RascunhoDados, RascunhoExtra, Segredo,
};
use super::wc::limits::MAX_ATTACHMENTS_TOTAL;

/// Senha mestra do cofre de teste.
pub const SENHA_MESTRA: &str = "falso-mestra-ward-01";
/// PIN do cofre de teste (D15). Nunca é gravado em claro: só o hash dele.
pub const PIN_TESTE: &str = "135790";

/// Configuração do cofre de teste (D30): hash do PIN de teste, trava em 240 minutos e
/// backup desligado. Calculada uma vez (o Argon2id do PIN custa 64 MiB) e compartilhada
/// pelos testes.
pub fn configuracao_de_teste() -> &'static Configuracao {
    static CFG: std::sync::OnceLock<Configuracao> = std::sync::OnceLock::new();
    CFG.get_or_init(|| Configuracao::nova(HashPin::criar(PIN_TESTE.as_bytes()).unwrap()))
}

/// Uma configuração nova, com hash próprio (sal novo), para quem precisa de posse.
pub fn configuracao_nova() -> Configuracao {
    Configuracao::nova(HashPin::criar(PIN_TESTE.as_bytes()).unwrap())
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

fn s(v: &str) -> Option<Segredo> {
    Some(Segredo::de_texto(v.to_owned()))
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

fn rascunho(nome: &str, grupo: &str, ambiente: &str, dados: RascunhoDados) -> Rascunho {
    Rascunho {
        nome: t(nome),
        grupo: t(grupo),
        ambiente: t(ambiente),
        projeto: String::new(),
        tags: Vec::new(),
        dados,
        extras: Vec::new(),
        anexos: Vec::new(),
    }
}

/// Valida e aplica o nível (como o daemon faria depois do PIN).
pub fn validada(r: Rascunho, nivel: NivelAcesso) -> Entrada {
    let mut e = match r.validar() {
        Ok(e) => e,
        Err(erros) => panic!("entrada de teste inválida: {erros}"),
    };
    e.definir_nivel(nivel);
    e
}

fn login(url: &str, usuario: &str, senha: &str, totp: Option<&str>) -> RascunhoDados {
    RascunhoDados::Login {
        url: t(url),
        usuario: t(usuario),
        senha: s(senha),
        totp_semente: totp.and_then(s),
    }
}

fn banco(host: &str, porta: &str, tunel: &str) -> RascunhoDados {
    RascunhoDados::Banco {
        motor: t("postgres"),
        host: t(host),
        porta: t(porta),
        base: t("app_teste"),
        usuario: t("app"),
        senha: s("falso-PG:s3nha/01@#"),
        tunel: t(tunel),
    }
}

fn servidor_senha(porta: &str, usuario: &str, senha: &str) -> RascunhoDados {
    RascunhoDados::Servidor {
        host: t("127.0.0.1"),
        porta: t(porta),
        usuario: t(usuario),
        senha: s(senha),
        chave_privada: None,
        passphrase: None,
        salto: String::new(),
    }
}

fn servidor_chave(host: &str, porta: &str, salto: &str) -> RascunhoDados {
    RascunhoDados::Servidor {
        host: t(host),
        porta: t(porta),
        usuario: t("deploy"),
        senha: None,
        chave_privada: Some(Segredo::de_texto(chave_falsa())),
        passphrase: s("falso-passphrase-01"),
        salto: t(salto),
    }
}

/// As 22 entradas do cofre de teste, com os níveis da tabela (fora de ordem de nome).
pub fn cofre_de_teste() -> Vec<Entrada> {
    use NivelAcesso::{Bloqueado, ComAprovacao, Livre};
    const WP: &str = "http://localhost:8081/wp-login.php";
    let mut generico = rascunho("gama-generico", "gama", "stage", RascunhoDados::Generico);
    generico.extras = vec![
        RascunhoExtra {
            nome: t("REGIAO"),
            valor: Segredo::de_texto(t("r-01")),
            segredo: false,
        },
        RascunhoExtra {
            nome: t("CHAVE_LICENCA"),
            valor: Segredo::de_texto(t("falso-licenca-01")),
            segredo: true,
        },
    ];
    generico.anexos = vec![RascunhoAnexo {
        nome: t("app.env"),
        conteudo: Segredo::de_bytes(b"FALSO_ENV=falso-anexo-01\n".to_vec()),
    }];
    // D59: caracteres especiais (aspas simples, `$&<>`, `/` e fora do ASCII).
    let mut simbolos = rascunho(
        "delta-api-simbolos",
        "delta",
        "dev",
        RascunhoDados::Api {
            url: t("https://api.teste.invalid"),
            token: s("falso-Ab3$k&<9>Zq!'/ç-01"),
            variavel: t("SIMBOLOS_TOKEN"),
        },
    );
    simbolos.extras = vec![RascunhoExtra {
        nome: t("SEGREDO_ASCII"),
        valor: Segredo::de_texto(t("falso-Ab3$k&<9>Zq!'")),
        segredo: true,
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
            ComAprovacao,
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
            Livre,
        ),
        (
            rascunho(
                "acme-pg-dev",
                "acme",
                "dev",
                banco("127.0.0.1", "55432", ""),
            ),
            Livre,
        ),
        (
            rascunho(
                "acme-pg-aprovacao",
                "acme",
                "stage",
                banco("127.0.0.1", "55432", ""),
            ),
            ComAprovacao,
        ),
        (
            rascunho(
                "acme-pg-prod",
                "acme",
                "prod",
                banco("127.0.0.1", "55432", ""),
            ),
            ComAprovacao,
        ),
        (
            rascunho(
                "acme-pg-tunel",
                "acme",
                "stage",
                banco("postgres", "5432", "acme-ssh-senha-stage"),
            ),
            Livre,
        ),
        (
            rascunho(
                "acme-ssh-senha-stage",
                "acme",
                "stage",
                servidor_senha("2222", "deploy", "falso-SSH-senha-01"),
            ),
            Livre,
        ),
        (
            rascunho(
                "acme-ssh-chave-prod",
                "acme",
                "prod",
                servidor_chave("127.0.0.1", "2223", ""),
            ),
            Livre,
        ),
        (
            rascunho(
                "acme-ssh-salto",
                "acme",
                "prod",
                servidor_senha("2224", "salto", "falso-SALTO-01"),
            ),
            ComAprovacao,
        ),
        (
            rascunho(
                "acme-ssh-salto-livre",
                "acme",
                "prod",
                servidor_senha("2224", "salto", "falso-SALTO-01"),
            ),
            Livre,
        ),
        (
            rascunho(
                "acme-ssh-interno",
                "acme",
                "prod",
                servidor_chave("ssh-interno", "22", "acme-ssh-salto-livre"),
            ),
            Livre,
        ),
        (
            rascunho(
                "acme-ssh-interno-aprov",
                "acme",
                "prod",
                servidor_chave("ssh-interno", "22", "acme-ssh-salto"),
            ),
            Livre,
        ),
        (
            rascunho(
                "beta-github-api",
                "beta",
                "dev",
                RascunhoDados::Api {
                    url: t("https://api.github.com"),
                    token: s("falso_token_github_01"),
                    variavel: t("GITHUB_TOKEN"),
                },
            ),
            Livre,
        ),
        (
            rascunho(
                "beta-aws-dev",
                "beta",
                "dev",
                RascunhoDados::Aws {
                    access_key_id: t("FALSOACCESSKEY01"),
                    secret_access_key: s("falso-aws-secret-01"),
                    regiao: t("sa-east-1"),
                    role_arn: t("arn:aws:iam::000000000000:role/teste"),
                    session_token: s("falso-aws-session-01"),
                },
            ),
            Livre,
        ),
        (
            rascunho(
                "beta-smtp-stage",
                "beta",
                "stage",
                RascunhoDados::Smtp {
                    host: t("smtp.teste.invalid"),
                    porta: t("587"),
                    usuario: t("mailer"),
                    senha: s("falso-SMTP-01"),
                    seguranca: t("starttls"),
                },
            ),
            Livre,
        ),
        (
            rascunho(
                "beta-sftp-dev",
                "beta",
                "dev",
                RascunhoDados::FtpSftp {
                    protocolo: t("sftp"),
                    host: t("127.0.0.1"),
                    porta: t("2222"),
                    usuario: t("sftpuser"),
                    senha: s("falso-SFTP-01"),
                    chave_privada: None,
                    passphrase: None,
                },
            ),
            Livre,
        ),
        (
            rascunho(
                "gama-nota",
                "gama",
                "dev",
                RascunhoDados::NotaSegura {
                    texto: s("falso-nota-01"),
                },
            ),
            Livre,
        ),
        (generico, Livre),
        (
            rascunho(
                "gama-bloqueada",
                "gama",
                "prod",
                login("https://bloqueada.invalid", "x", "falso-BLOQ-01", None),
            ),
            Bloqueado,
        ),
        (
            rascunho(
                "delta-redir",
                "delta",
                "dev",
                login("http://localhost:8084/login", "u", "falso-REDIR-01", None),
            ),
            Livre,
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
            Livre,
        ),
        (simbolos, Livre),
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
pub fn um_de_cada_cheio() -> Vec<Entrada> {
    use NivelAcesso::{Bloqueado, ComAprovacao, Livre};
    let url_longa = {
        let base = "https://cheio.invalid/";
        let mut u = String::with_capacity(2048);
        u.push_str(base);
        while u.len() < 2048 {
            u.push('a');
        }
        u
    };
    let dados: Vec<(&str, RascunhoDados, NivelAcesso)> = vec![
        (
            "zz-cheio-login",
            RascunhoDados::Login {
                url: url_longa,
                usuario: t("Usuário Com Espaço"),
                senha: s("falso-cheio-senha"),
                totp_semente: s("JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP"),
            },
            Bloqueado,
        ),
        (
            "zz-cheio-banco",
            RascunhoDados::Banco {
                motor: t("sqlserver"),
                host: t("[::1]"),
                porta: t("1"),
                base: t("base_cheia"),
                usuario: t("sa"),
                senha: s("falso-cheio-banco"),
                tunel: t("zz-cheio-servidor"),
            },
            ComAprovacao,
        ),
        (
            "zz-cheio-servidor",
            RascunhoDados::Servidor {
                host: t("cheio.invalid"),
                porta: t("65535"),
                usuario: t("root"),
                senha: None,
                chave_privada: Some(Segredo::de_texto(chave_falsa())),
                passphrase: None,
                salto: String::new(),
            },
            Livre,
        ),
        (
            "zz-cheio-api",
            RascunhoDados::Api {
                url: String::new(),
                token: Some(Segredo::de_bytes(vec![b'k'; 4096])),
                variavel: String::new(),
            },
            Livre,
        ),
        // O token no limite de D25 (64 KiB) passa do orçamento da máscara junto com
        // 256 KiB de anexos (entrada e `WARD_TOKEN`, ~1 MiB de padrões cada, mais
        // ~2,4 MiB do anexo); fica numa segunda `api`, com o anexo pequeno.
        (
            "zz-cheio-api-token",
            RascunhoDados::Api {
                url: String::new(),
                token: Some(Segredo::de_bytes(vec![b'k'; 64 * 1024])),
                variavel: String::new(),
            },
            Livre,
        ),
        (
            "zz-cheio-aws",
            RascunhoDados::Aws {
                access_key_id: t("FALSOACCESSKEY02"),
                secret_access_key: s("falso-cheio-aws"),
                regiao: t("us-east-1"),
                role_arn: String::new(),
                session_token: None,
            },
            ComAprovacao,
        ),
        (
            "zz-cheio-smtp",
            RascunhoDados::Smtp {
                host: t("smtp.cheio.invalid"),
                porta: String::new(),
                usuario: t("mailer"),
                senha: s("falso-cheio-smtp"),
                seguranca: t("tls"),
            },
            Livre,
        ),
        (
            "zz-cheio-ftp",
            RascunhoDados::FtpSftp {
                protocolo: t("ftp"),
                host: t("ftp.cheio.invalid"),
                porta: String::new(),
                usuario: t("ftpuser"),
                senha: s("falso-cheio-ftp"),
                chave_privada: None,
                passphrase: None,
            },
            Livre,
        ),
        (
            "zz-cheio-nota",
            RascunhoDados::NotaSegura {
                texto: Some(Segredo::de_bytes(vec![b'n'; 1024 * 1024])),
            },
            Bloqueado,
        ),
        ("zz-cheio-generico", RascunhoDados::Generico, Livre),
    ];
    dados
        .into_iter()
        .map(|(nome, dados, nivel)| {
            let mut r = rascunho(nome, "zeta", "stage", dados);
            r.projeto = t("projeto cheio");
            r.tags = (1..=16).map(|n| format!("tag-{n:02}")).collect();
            r.extras = (1..=64)
                .map(|n| {
                    let segredo = n % 2 == 0;
                    let valor = if segredo {
                        format!("falso-extra-{n:02}")
                    } else {
                        format!("valor-{n:02}")
                    };
                    RascunhoExtra {
                        nome: format!("extra_{n:02}"),
                        valor: Segredo::de_texto(valor),
                        segredo,
                    }
                })
                .collect();
            r.anexos = vec![RascunhoAnexo {
                nome: t("binario.bin"),
                conteudo: Segredo::de_bytes(bytes_0_a_255(4096)),
            }];
            if nome != "zz-cheio-api-token" {
                r.anexos.push(RascunhoAnexo {
                    nome: t("grande.bin"),
                    conteudo: Segredo::de_bytes(bytes_0_a_255(MAX_ATTACHMENTS_TOTAL - 4096)),
                });
            }
            validada(r, nivel)
        })
        .collect()
}
