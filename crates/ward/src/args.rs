//! Linha de comando da CLI. Nenhum argumento é repetido nas mensagens de erro: quem
//! digita `ward destravar <senha>` por engano não vê a senha voltar na tela, no log do
//! terminal nem na saída que o agente lê (S1, S3).

use std::ffi::OsString;

/// O que a CLI vai fazer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Comando {
    Lista {
        json: bool,
    },
    Travar,
    Destravar,
    /// `ward run <entrada> -- <comando…>` (Fase 26): o argv vai em bytes, sem shell.
    Run {
        entrada: String,
        argv: Vec<OsString>,
    },
    /// `ward ssh [--token-fd <n>] <entrada> [--] [comando…]` (Fases 27 e 27a): o
    /// comando remoto em bytes; `token_fd` é o número do fd herdado do app por onde
    /// chega o token do "Conectar" (D3). O token nunca vem no argv.
    Ssh {
        entrada: String,
        token_fd: Option<u32>,
        comando: Vec<OsString>,
    },
    /// `ward playwright <entrada>` (Fase 44, D6): o daemon faz o login no Chrome
    /// headless e a CLI imprime só o caminho do storage state.
    Playwright {
        entrada: String,
    },
    /// `ward login <entrada>` (Fase 43): a extensão do Chrome confere a origem da aba
    /// ativa, preenche, envia e limpa os campos. Nenhum valor volta.
    Login {
        entrada: String,
    },
    Ajuda,
    Versao,
}

/// Erro de uso (sai com código 2). Só textos fixos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroUso {
    SemComando,
    ComandoDesconhecido,
    ArgumentoInesperado(&'static str),
    NaoUtf8,
    /// `ward run` sem entrada, sem `--` ou sem comando.
    UsoRun,
    /// `ward ssh` sem entrada, ou com `--token-fd` fora do formato.
    UsoSsh,
    /// `ward ssh --token <t>` (Fase 27a): o token no argv fica legível em
    /// `/proc/<pid>/cmdline` por qualquer processo do mesmo usuário. Recusado.
    TokenEmArgv,
    /// `ward playwright` sem entrada ou com argumento a mais.
    UsoPlaywright,
    /// `ward login` sem entrada ou com argumento a mais.
    UsoLogin,
}

impl ErroUso {
    pub fn mensagem(self) -> String {
        match self {
            ErroUso::SemComando => "falta o comando".to_owned(),
            ErroUso::ComandoDesconhecido => "comando desconhecido".to_owned(),
            ErroUso::ArgumentoInesperado(cmd) => format!("argumento inesperado para `{cmd}`"),
            ErroUso::NaoUtf8 => "argumento que não é UTF-8".to_owned(),
            ErroUso::UsoRun => "uso: ward run <entrada> -- <comando> [argumentos…]".to_owned(),
            ErroUso::UsoSsh => "uso: ward ssh <entrada> [--] [comando remoto…]".to_owned(),
            ErroUso::TokenEmArgv => "o token do Conectar não vai na linha de comando; \
                 use o botão Conectar do app"
                .to_owned(),
            ErroUso::UsoPlaywright => "uso: ward playwright <entrada>".to_owned(),
            ErroUso::UsoLogin => "uso: ward login <entrada>".to_owned(),
        }
    }
}

pub const AJUDA: &str = "\
ward: cofre de credenciais do trabalho (cliente do wardd)

Uso:
  ward lista [--json]   entradas que o agente pode ver: nome, tipo, grupo,
                        ambiente, projeto e URL/host (nunca segredo)
  ward destravar        destrava o cofre com a senha mestra, lida do terminal
  ward travar           trava o cofre agora
  ward run <entrada> -- <comando> [argumentos…]
                        roda o comando com a credencial no ambiente (D10); a
                        saída volta com os segredos trocados por ••••, e o
                        código de saída é o do comando (125: erro do Ward).
                        Roda no diretório atual, com PATH fixo
                        (/usr/local/bin:/usr/bin:/bin; comando fora dele vai
                        com caminho absoluto) e, do seu ambiente, só LANG,
                        LC_*, TERM, COLORTERM, TZ e NO_COLOR
  ward ssh <entrada> [comando remoto…]
                        conecta no servidor da entrada (senha ou chave, sem o
                        ~/.ssh); sem comando, abre o shell remoto no terminal;
                        numa entrada sftp, abre o sftp. A saída volta com os
                        segredos trocados por ••••
  ward ssh --token-fd <n> <entrada>
                        o mesmo, aberto pelo botão Conectar do app: o token de
                        uso único chega pelo fd <n> que o app passa ao terminal,
                        nunca pela linha de comando
  ward login <entrada>   faz o login da entrada na aba ativa do Chrome, pela
                        extensão do Ward: confere que a origem da aba é a da
                        entrada (senão, \"domínio não confere\"), preenche,
                        envia o formulário e limpa os campos
  ward playwright <entrada>
                        faz o login da entrada no Chrome (headless) e imprime o
                        caminho do storage state para o Playwright
                        (`storage_state=`); a senha nunca vai para o arquivo.
                        Travar o cofre apaga as sessões
  ward --help           esta ajuda
  ward --version        versão

Nenhum comando devolve segredo. Revelar e copiar existem só no app.
";

/// `run <entrada> -- <comando…>`: a entrada em UTF-8; o comando em bytes, como veio.
fn interpretar_run(resto: Vec<OsString>) -> Result<Comando, ErroUso> {
    let mut it = resto.into_iter();
    let entrada = it
        .next()
        .ok_or(ErroUso::UsoRun)?
        .into_string()
        .map_err(|_| ErroUso::NaoUtf8)?;
    if entrada.is_empty() || entrada.starts_with('-') {
        return Err(ErroUso::UsoRun);
    }
    if it.next().is_none_or(|s| s != "--") {
        return Err(ErroUso::UsoRun);
    }
    let argv: Vec<OsString> = it.collect();
    if argv.first().is_none_or(|a| a.is_empty()) {
        return Err(ErroUso::UsoRun);
    }
    Ok(Comando::Run { entrada, argv })
}

/// Token do "Conectar" (D3): o formato que o daemon emite e o app aceita.
pub fn token_ok(t: &[u8]) -> bool {
    (16..=64).contains(&t.len())
        && t[0] != b'-'
        && t.iter()
            .all(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'-')
}

/// Número de `--token-fd`: só dígitos decimais (sem sinal, sem espaço), de 3 a
/// `MAX_NUMERO_FD` (0–2 são a entrada e as saídas padrão).
fn numero_fd(a: &OsString) -> Option<u32> {
    let t = a.to_str()?;
    if t.is_empty() || t.len() > 7 || !t.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: u32 = t.parse().ok()?;
    (3..=ward_platform::conectar::MAX_NUMERO_FD)
        .contains(&n)
        .then_some(n)
}

/// `ssh [--token-fd <n>] <entrada> [--] [comando…]`. `--token` (com o valor em argv,
/// separado ou com `=`) é recusado (Fase 27a).
fn interpretar_ssh(resto: Vec<OsString>) -> Result<Comando, ErroUso> {
    use std::os::unix::ffi::OsStrExt;
    let mut it = resto.into_iter().peekable();
    if it.peek().is_some_and(|a| {
        let b = a.as_bytes();
        b == b"--token" || b.starts_with(b"--token=")
    }) {
        return Err(ErroUso::TokenEmArgv);
    }
    let mut token_fd = None;
    if it.peek().is_some_and(|a| a == "--token-fd") {
        it.next();
        let n = it.next().ok_or(ErroUso::UsoSsh)?;
        token_fd = Some(numero_fd(&n).ok_or(ErroUso::UsoSsh)?);
    }
    let entrada = it
        .next()
        .ok_or(ErroUso::UsoSsh)?
        .into_string()
        .map_err(|_| ErroUso::NaoUtf8)?;
    if entrada.is_empty() || entrada.starts_with('-') {
        return Err(ErroUso::UsoSsh);
    }
    if it.peek().is_some_and(|a| a == "--") {
        it.next();
    }
    let comando: Vec<OsString> = it.collect();
    if comando.first().is_some_and(|a| a.is_empty()) {
        return Err(ErroUso::UsoSsh);
    }
    Ok(Comando::Ssh {
        entrada,
        token_fd,
        comando,
    })
}

/// Lê os argumentos (sem o nome do programa).
pub fn interpretar(args: impl IntoIterator<Item = OsString>) -> Result<Comando, ErroUso> {
    let mut args: Vec<OsString> = args.into_iter().collect();
    if args.first().is_some_and(|a| a == "run") {
        return interpretar_run(args.split_off(1));
    }
    if args.first().is_some_and(|a| a == "ssh") {
        return interpretar_ssh(args.split_off(1));
    }
    let mut textos = Vec::new();
    for a in args {
        textos.push(a.into_string().map_err(|_| ErroUso::NaoUtf8)?);
    }
    let mut it = textos.iter().map(String::as_str);
    let Some(primeiro) = it.next() else {
        return Err(ErroUso::SemComando);
    };
    let resto: Vec<&str> = it.collect();
    let sem_resto = |c: Comando, nome: &'static str| {
        if resto.is_empty() {
            Ok(c)
        } else {
            Err(ErroUso::ArgumentoInesperado(nome))
        }
    };
    match primeiro {
        "-h" | "--help" | "ajuda" => sem_resto(Comando::Ajuda, "--help"),
        "-V" | "--version" => sem_resto(Comando::Versao, "--version"),
        "travar" => sem_resto(Comando::Travar, "travar"),
        "destravar" => sem_resto(Comando::Destravar, "destravar"),
        "playwright" => match resto.as_slice() {
            [entrada] if !entrada.is_empty() && !entrada.starts_with('-') => {
                Ok(Comando::Playwright {
                    entrada: (*entrada).to_owned(),
                })
            }
            _ => Err(ErroUso::UsoPlaywright),
        },
        "login" => match resto.as_slice() {
            [entrada] if !entrada.is_empty() && !entrada.starts_with('-') => Ok(Comando::Login {
                entrada: (*entrada).to_owned(),
            }),
            _ => Err(ErroUso::UsoLogin),
        },
        "lista" => match resto.as_slice() {
            [] => Ok(Comando::Lista { json: false }),
            ["--json"] => Ok(Comando::Lista { json: true }),
            _ => Err(ErroUso::ArgumentoInesperado("lista")),
        },
        _ => Err(ErroUso::ComandoDesconhecido),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn ler(a: &[&str]) -> Result<Comando, ErroUso> {
        interpretar(a.iter().map(OsString::from))
    }

    #[test]
    fn comandos() {
        assert_eq!(ler(&["lista"]), Ok(Comando::Lista { json: false }));
        assert_eq!(ler(&["lista", "--json"]), Ok(Comando::Lista { json: true }));
        assert_eq!(ler(&["travar"]), Ok(Comando::Travar));
        assert_eq!(ler(&["destravar"]), Ok(Comando::Destravar));
        assert_eq!(ler(&["--help"]), Ok(Comando::Ajuda));
        assert_eq!(ler(&["-h"]), Ok(Comando::Ajuda));
        assert_eq!(ler(&["--version"]), Ok(Comando::Versao));
        assert_eq!(ler(&[]), Err(ErroUso::SemComando));
        assert_eq!(ler(&["revelar"]), Err(ErroUso::ComandoDesconhecido));
        assert_eq!(
            ler(&["lista", "--json", "x"]),
            Err(ErroUso::ArgumentoInesperado("lista"))
        );
        assert_eq!(
            ler(&["lista", "--nivel"]),
            Err(ErroUso::ArgumentoInesperado("lista"))
        );
    }

    /// Ataque: a senha digitada por engano como argumento não volta na mensagem.
    #[test]
    fn ataque_argumento_nao_volta_na_mensagem() {
        for a in [
            &["destravar", "falso-mestra-ward-01"][..],
            &["falso-mestra-ward-01"][..],
            &["lista", "falso-mestra-ward-01"][..],
        ] {
            let m = ler(a).unwrap_err().mensagem();
            assert!(!m.contains("falso-mestra"), "{m}");
        }
    }

    #[test]
    fn run() {
        assert_eq!(
            ler(&["run", "acme-pg-dev", "--", "psql", "-tAc", "select 1"]),
            Ok(Comando::Run {
                entrada: "acme-pg-dev".into(),
                argv: vec!["psql".into(), "-tAc".into(), "select 1".into()],
            })
        );
        // O `--` do comando é dele.
        assert_eq!(
            ler(&["run", "e", "--", "sh", "--", "-c"]),
            Ok(Comando::Run {
                entrada: "e".into(),
                argv: vec!["sh".into(), "--".into(), "-c".into()],
            })
        );
        for ruim in [
            &["run"][..],
            &["run", "e"][..],
            &["run", "e", "--"][..],
            &["run", "e", "psql"][..],
            &["run", "--", "psql"][..],
            &["run", "-x", "--", "psql"][..],
            &["run", "e", "--", ""][..],
        ] {
            assert_eq!(ler(ruim), Err(ErroUso::UsoRun), "{ruim:?}");
        }
        // Argumento do comando que não é UTF-8 passa em bytes.
        use std::os::unix::ffi::OsStringExt;
        let r = interpretar([
            OsString::from("run"),
            OsString::from("e"),
            OsString::from("--"),
            OsString::from_vec(vec![b'x', 0xff]),
        ]);
        assert!(matches!(r, Ok(Comando::Run { .. })));
        // A mensagem de uso não repete argumento.
        let m = ler(&["run", "falso-PG:s3nha/01@#"]).unwrap_err().mensagem();
        assert!(!m.contains("falso"), "{m}");
    }

    #[test]
    fn ssh() {
        assert_eq!(
            ler(&["ssh", "acme-ssh-senha-stage"]),
            Ok(Comando::Ssh {
                entrada: "acme-ssh-senha-stage".into(),
                token_fd: None,
                comando: vec![],
            })
        );
        assert_eq!(
            ler(&["ssh", "e", "whoami"]),
            Ok(Comando::Ssh {
                entrada: "e".into(),
                token_fd: None,
                comando: vec!["whoami".into()],
            })
        );
        assert_eq!(
            ler(&["ssh", "e", "--", "ls", "-la"]),
            Ok(Comando::Ssh {
                entrada: "e".into(),
                token_fd: None,
                comando: vec!["ls".into(), "-la".into()],
            })
        );
        assert_eq!(
            ler(&["ssh", "--token-fd", "7", "e"]),
            Ok(Comando::Ssh {
                entrada: "e".into(),
                token_fd: Some(7),
                comando: vec![],
            })
        );
        for ruim in [
            &["ssh"][..],
            &["ssh", "--token-fd"],
            &["ssh", "--token-fd", "7"],
            &["ssh", "--token-fd", "0", "e"],
            &["ssh", "--token-fd", "2", "e"],
            &["ssh", "--token-fd", "-3", "e"],
            &["ssh", "--token-fd", "+3", "e"],
            &["ssh", "--token-fd", " 3", "e"],
            &["ssh", "--token-fd", "0x3", "e"],
            &["ssh", "--token-fd", "", "e"],
            &["ssh", "--token-fd", "99999999", "e"],
            &["ssh", "--token-fd", "1048577", "e"],
            &["ssh", "-x", "e"],
            &["ssh", "e", ""],
        ] {
            assert_eq!(ler(ruim), Err(ErroUso::UsoSsh), "{ruim:?}");
        }
    }

    /// Ataque (Fase 27a, achado alto do M3): o token em argv (`--token <t>`,
    /// `--token=<t>`, com ou sem entrada) é recusado antes de qualquer conexão, e a
    /// mensagem não repete o valor.
    #[test]
    fn ataque_token_em_argv_recusado_sem_eco() {
        let t = "AbCdEfGhIjKlMnOpQrStUvWxYz012345";
        let igual = format!("--token={t}");
        for ruim in [
            &["ssh", "--token", t, "e"][..],
            &["ssh", "--token", t],
            &["ssh", "--token"],
            &["ssh", &igual, "e"],
            &["ssh", "--token=", "e"],
        ] {
            let e = ler(ruim).unwrap_err();
            assert_eq!(e, ErroUso::TokenEmArgv, "{ruim:?}");
            assert!(!e.mensagem().contains("AbCd"), "{}", e.mensagem());
        }
        // Depois da entrada, `--token` é argumento do comando remoto (como qualquer
        // outro), não o token do Ward: o daemon nunca o vê como token.
        assert_eq!(
            ler(&["ssh", "e", "--token", t]),
            Ok(Comando::Ssh {
                entrada: "e".into(),
                token_fd: None,
                comando: vec!["--token".into(), t.into()],
            })
        );
        let m = ler(&["ssh", "--token", "falso-SSH-senha-01!"])
            .unwrap_err()
            .mensagem();
        assert!(!m.contains("falso"), "{m}");
        assert!(
            !AJUDA.contains("--token <"),
            "a ajuda ainda ensina o token em argv"
        );
    }

    #[test]
    fn formato_do_token() {
        assert!(token_ok(b"AbCdEfGhIjKlMnOpQrStUvWxYz012345"));
        for t in [
            &b""[..],
            b"curto",
            b"-AbCdEfGhIjKlMnOpQr",
            b"AbCdEfGhIjKlMnOp$rStUv",
            b"AbCdEfGhIjKlMnOp\nrStUv",
            b"AbCdEfGhIjKlMnOp\xffStUv",
            &[b'a'; 65],
        ] {
            assert!(!token_ok(t), "{t:?}");
        }
    }

    #[test]
    fn playwright() {
        assert_eq!(
            ler(&["playwright", "acme-wp-prod"]),
            Ok(Comando::Playwright {
                entrada: "acme-wp-prod".into()
            })
        );
        for ruim in [
            &["playwright"][..],
            &["playwright", ""],
            &["playwright", "-x"],
            &["playwright", "a", "b"],
            &["playwright", "--", "a"],
        ] {
            assert_eq!(ler(ruim), Err(ErroUso::UsoPlaywright), "{ruim:?}");
        }
        // A mensagem de uso não repete argumento.
        let m = ler(&["playwright", "e", "falso-WP-senha-02"])
            .unwrap_err()
            .mensagem();
        assert!(!m.contains("falso"), "{m}");
    }

    #[test]
    fn login() {
        assert_eq!(
            ler(&["login", "acme-wp-prod"]),
            Ok(Comando::Login {
                entrada: "acme-wp-prod".into()
            })
        );
        for ruim in [
            &["login"][..],
            &["login", ""],
            &["login", "-x"],
            &["login", "a", "b"],
            &["login", "--", "a"],
        ] {
            assert_eq!(ler(ruim), Err(ErroUso::UsoLogin), "{ruim:?}");
        }
        let m = ler(&["login", "e", "falso-WP-senha-02"])
            .unwrap_err()
            .mensagem();
        assert!(!m.contains("falso"), "{m}");
    }

    #[test]
    fn nao_utf8() {
        use std::os::unix::ffi::OsStringExt;
        let r = interpretar([OsString::from_vec(vec![0xff, 0xfe])]);
        assert_eq!(r, Err(ErroUso::NaoUtf8));
    }
}
