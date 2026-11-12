//! Command line of the CLI. No argument is repeated in the error messages: whoever
//! types the password after the unlock subcommand by mistake does not see it come back
//! on the screen, in the terminal log nor in the output the agent reads (S1, S3).

use std::ffi::OsString;

/// What the CLI is going to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    List {
        json: bool,
    },
    Lock,
    Unlock,
    /// `ward run <entry> -- <command…>` (Phase 26): the argv goes as bytes, without a
    /// shell.
    Run {
        entry: String,
        argv: Vec<OsString>,
    },
    /// `ward ssh [--token-fd <n>] <entry> [--] [command…]` (Phases 27 and 27a): the
    /// remote command as bytes; `token_fd` is the number of the fd inherited from the app
    /// through which the token of the Connect button arrives (D3). The token never comes in argv.
    Ssh {
        entry: String,
        token_fd: Option<u32>,
        command: Vec<OsString>,
    },
    /// `ward playwright <entry>` (Phase 44, D6): the daemon logs in on headless Chrome
    /// and the CLI prints only the path of the storage state.
    Playwright {
        entry: String,
    },
    /// `ward login <entry>` (Phase 43): the Chrome extension checks the origin of the
    /// active tab, fills, submits and clears the fields. No value comes back.
    Login {
        entry: String,
    },
    Help,
    Version,
}

/// Usage error (exits with code 2). Only fixed texts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageError {
    NoCommand,
    UnknownCommand,
    UnexpectedArgument(&'static str),
    NotUtf8,
    /// `ward run` without entry, without `--` or without command.
    RunUsage,
    /// `ward ssh` without entry, or with `--token-fd` out of format.
    SshUsage,
    /// `ward ssh --token <t>` (Phase 27a): the token in argv is readable in
    /// `/proc/<pid>/cmdline` by any process of the same user. Refused.
    TokenInArgv,
    /// `ward playwright` without entry or with an extra argument.
    PlaywrightUsage,
    /// `ward login` without entry or with an extra argument.
    LoginUsage,
}

impl UsageError {
    pub fn message(self) -> String {
        match self {
            UsageError::NoCommand => "falta o comando".to_owned(),
            UsageError::UnknownCommand => "comando desconhecido".to_owned(),
            UsageError::UnexpectedArgument(cmd) => format!("argumento inesperado para `{cmd}`"),
            UsageError::NotUtf8 => "argumento que não é UTF-8".to_owned(),
            UsageError::RunUsage => "uso: ward run <entrada> -- <comando> [argumentos…]".to_owned(),
            UsageError::SshUsage => "uso: ward ssh <entrada> [--] [comando remoto…]".to_owned(),
            UsageError::TokenInArgv => "o token do Conectar não vai na linha de comando; \
                 use o botão Conectar do app"
                .to_owned(),
            UsageError::PlaywrightUsage => "uso: ward playwright <entrada>".to_owned(),
            UsageError::LoginUsage => "uso: ward login <entrada>".to_owned(),
        }
    }
}

pub const HELP: &str = "\
ward: cofre de credenciais do trabalho (cliente do wardd)

Uso:
  ward list [--json]    entradas que o agente pode ver: nome, tipo, grupo,
                        ambiente, projeto e URL/host (nunca segredo)
  ward unlock           destrava o cofre com a senha mestra, lida do terminal
  ward lock             trava o cofre agora
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

/// `run <entry> -- <command…>`: the entry in UTF-8; the command as bytes, as it came.
fn parse_run(rest: Vec<OsString>) -> Result<Command, UsageError> {
    let mut it = rest.into_iter();
    let entry = it
        .next()
        .ok_or(UsageError::RunUsage)?
        .into_string()
        .map_err(|_| UsageError::NotUtf8)?;
    if entry.is_empty() || entry.starts_with('-') {
        return Err(UsageError::RunUsage);
    }
    if it.next().is_none_or(|s| s != "--") {
        return Err(UsageError::RunUsage);
    }
    let argv: Vec<OsString> = it.collect();
    if argv.first().is_none_or(|a| a.is_empty()) {
        return Err(UsageError::RunUsage);
    }
    Ok(Command::Run { entry, argv })
}

/// Token of the Connect button (D3): the format the daemon issues and the app accepts.
pub fn token_ok(t: &[u8]) -> bool {
    (16..=64).contains(&t.len())
        && t[0] != b'-'
        && t.iter()
            .all(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'-')
}

/// Number of `--token-fd`: decimal digits only (no sign, no space), from 3 to
/// `MAX_FD_NUMBER` (0–2 are standard input and the standard outputs).
fn fd_number(a: &OsString) -> Option<u32> {
    let t = a.to_str()?;
    if t.is_empty() || t.len() > 7 || !t.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: u32 = t.parse().ok()?;
    (3..=ward_platform::connect::MAX_FD_NUMBER)
        .contains(&n)
        .then_some(n)
}

/// `ssh [--token-fd <n>] <entry> [--] [command…]`. `--token` (with the value in argv,
/// separate or with `=`) is refused (Phase 27a).
fn parse_ssh(rest: Vec<OsString>) -> Result<Command, UsageError> {
    use std::os::unix::ffi::OsStrExt;
    let mut it = rest.into_iter().peekable();
    if it.peek().is_some_and(|a| {
        let b = a.as_bytes();
        b == b"--token" || b.starts_with(b"--token=")
    }) {
        return Err(UsageError::TokenInArgv);
    }
    let mut token_fd = None;
    if it.peek().is_some_and(|a| a == "--token-fd") {
        it.next();
        let n = it.next().ok_or(UsageError::SshUsage)?;
        token_fd = Some(fd_number(&n).ok_or(UsageError::SshUsage)?);
    }
    let entry = it
        .next()
        .ok_or(UsageError::SshUsage)?
        .into_string()
        .map_err(|_| UsageError::NotUtf8)?;
    if entry.is_empty() || entry.starts_with('-') {
        return Err(UsageError::SshUsage);
    }
    if it.peek().is_some_and(|a| a == "--") {
        it.next();
    }
    let command: Vec<OsString> = it.collect();
    if command.first().is_some_and(|a| a.is_empty()) {
        return Err(UsageError::SshUsage);
    }
    Ok(Command::Ssh {
        entry,
        token_fd,
        command,
    })
}

/// Reads the arguments (without the program name).
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, UsageError> {
    let mut args: Vec<OsString> = args.into_iter().collect();
    if args.first().is_some_and(|a| a == "run") {
        return parse_run(args.split_off(1));
    }
    if args.first().is_some_and(|a| a == "ssh") {
        return parse_ssh(args.split_off(1));
    }
    let mut texts = Vec::new();
    for a in args {
        texts.push(a.into_string().map_err(|_| UsageError::NotUtf8)?);
    }
    let mut it = texts.iter().map(String::as_str);
    let Some(first) = it.next() else {
        return Err(UsageError::NoCommand);
    };
    let rest: Vec<&str> = it.collect();
    let no_rest = |c: Command, name: &'static str| {
        if rest.is_empty() {
            Ok(c)
        } else {
            Err(UsageError::UnexpectedArgument(name))
        }
    };
    match first {
        // m2:DT10: English names only; the four 0.1.x names (the unit test
        // `old_names_are_unknown_commands` lists them) are unknown commands, with no alias.
        "-h" | "--help" | "help" => no_rest(Command::Help, "--help"),
        "-V" | "--version" => no_rest(Command::Version, "--version"),
        "lock" => no_rest(Command::Lock, "lock"),
        "unlock" => no_rest(Command::Unlock, "unlock"),
        "playwright" => match rest.as_slice() {
            [entry] if !entry.is_empty() && !entry.starts_with('-') => Ok(Command::Playwright {
                entry: (*entry).to_owned(),
            }),
            _ => Err(UsageError::PlaywrightUsage),
        },
        "login" => match rest.as_slice() {
            [entry] if !entry.is_empty() && !entry.starts_with('-') => Ok(Command::Login {
                entry: (*entry).to_owned(),
            }),
            _ => Err(UsageError::LoginUsage),
        },
        "list" => match rest.as_slice() {
            [] => Ok(Command::List { json: false }),
            ["--json"] => Ok(Command::List { json: true }),
            _ => Err(UsageError::UnexpectedArgument("list")),
        },
        _ => Err(UsageError::UnknownCommand),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(a: &[&str]) -> Result<Command, UsageError> {
        parse(a.iter().map(OsString::from))
    }

    #[test]
    fn commands() {
        assert_eq!(parse_args(&["list"]), Ok(Command::List { json: false }));
        assert_eq!(
            parse_args(&["list", "--json"]),
            Ok(Command::List { json: true })
        );
        assert_eq!(parse_args(&["lock"]), Ok(Command::Lock));
        assert_eq!(parse_args(&["unlock"]), Ok(Command::Unlock));
        assert_eq!(parse_args(&["--help"]), Ok(Command::Help));
        assert_eq!(parse_args(&["-h"]), Ok(Command::Help));
        assert_eq!(parse_args(&["help"]), Ok(Command::Help));
        assert_eq!(parse_args(&["--version"]), Ok(Command::Version));
        assert_eq!(parse_args(&[]), Err(UsageError::NoCommand));
        assert_eq!(parse_args(&["revelar"]), Err(UsageError::UnknownCommand));
        assert_eq!(
            parse_args(&["list", "--json", "x"]),
            Err(UsageError::UnexpectedArgument("list"))
        );
        assert_eq!(
            parse_args(&["list", "--nivel"]),
            Err(UsageError::UnexpectedArgument("list"))
        );
        assert_eq!(
            parse_args(&["unlock", "x"]),
            Err(UsageError::UnexpectedArgument("unlock"))
        );
        assert_eq!(
            parse_args(&["lock", "x"]),
            Err(UsageError::UnexpectedArgument("lock"))
        );
    }

    /// m2:DT10, m2:V5.1: the 0.1.x subcommands have no alias. Each one (alone, with the
    /// arguments it took, and in another case) is an unknown command (exit 2), never
    /// the English command it used to mean.
    #[test]
    fn old_names_are_unknown_commands() {
        for old in [
            &["lista"][..],
            &["lista", "--json"],
            &["travar"],
            &["destravar"],
            &["ajuda"],
            &["LIST"],
            &["Unlock"],
        ] {
            assert_eq!(parse_args(old), Err(UsageError::UnknownCommand), "{old:?}");
        }
        assert_eq!(
            parse_args(&["help", "x"]),
            Err(UsageError::UnexpectedArgument("--help"))
        );
        assert_eq!(UsageError::UnknownCommand.message(), "comando desconhecido");
        for old in ["lista", "travar", "destravar", "ajuda"] {
            assert!(
                !HELP.contains(&format!("ward {old}")),
                "a ajuda ainda cita `ward {old}`"
            );
        }
        for new in ["ward list [--json]", "ward unlock", "ward lock"] {
            assert!(HELP.contains(new), "a ajuda não cita `{new}`");
        }
    }

    /// Attack: the password typed by mistake as an argument does not come back in the
    /// message.
    #[test]
    fn attack_argument_not_echoed_in_message() {
        for a in [
            &["unlock", "falso-mestra-ward-01"][..],
            &["falso-mestra-ward-01"][..],
            &["list", "falso-mestra-ward-01"][..],
            &["destravar", "falso-mestra-ward-01"][..],
        ] {
            let m = parse_args(a).unwrap_err().message();
            assert!(!m.contains("falso-mestra"), "{m}");
        }
    }

    #[test]
    fn run() {
        assert_eq!(
            parse_args(&["run", "acme-pg-dev", "--", "psql", "-tAc", "select 1"]),
            Ok(Command::Run {
                entry: "acme-pg-dev".into(),
                argv: vec!["psql".into(), "-tAc".into(), "select 1".into()],
            })
        );
        // The command's `--` belongs to it.
        assert_eq!(
            parse_args(&["run", "e", "--", "sh", "--", "-c"]),
            Ok(Command::Run {
                entry: "e".into(),
                argv: vec!["sh".into(), "--".into(), "-c".into()],
            })
        );
        for bad in [
            &["run"][..],
            &["run", "e"][..],
            &["run", "e", "--"][..],
            &["run", "e", "psql"][..],
            &["run", "--", "psql"][..],
            &["run", "-x", "--", "psql"][..],
            &["run", "e", "--", ""][..],
        ] {
            assert_eq!(parse_args(bad), Err(UsageError::RunUsage), "{bad:?}");
        }
        // A command argument that is not UTF-8 goes as bytes.
        use std::os::unix::ffi::OsStringExt;
        let r = parse([
            OsString::from("run"),
            OsString::from("e"),
            OsString::from("--"),
            OsString::from_vec(vec![b'x', 0xff]),
        ]);
        assert!(matches!(r, Ok(Command::Run { .. })));
        // The usage message does not repeat an argument.
        let m = parse_args(&["run", "falso-PG:s3nha/01@#"])
            .unwrap_err()
            .message();
        assert!(!m.contains("falso"), "{m}");
    }

    #[test]
    fn ssh() {
        assert_eq!(
            parse_args(&["ssh", "acme-ssh-senha-stage"]),
            Ok(Command::Ssh {
                entry: "acme-ssh-senha-stage".into(),
                token_fd: None,
                command: vec![],
            })
        );
        assert_eq!(
            parse_args(&["ssh", "e", "whoami"]),
            Ok(Command::Ssh {
                entry: "e".into(),
                token_fd: None,
                command: vec!["whoami".into()],
            })
        );
        assert_eq!(
            parse_args(&["ssh", "e", "--", "ls", "-la"]),
            Ok(Command::Ssh {
                entry: "e".into(),
                token_fd: None,
                command: vec!["ls".into(), "-la".into()],
            })
        );
        assert_eq!(
            parse_args(&["ssh", "--token-fd", "7", "e"]),
            Ok(Command::Ssh {
                entry: "e".into(),
                token_fd: Some(7),
                command: vec![],
            })
        );
        for bad in [
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
            assert_eq!(parse_args(bad), Err(UsageError::SshUsage), "{bad:?}");
        }
    }

    /// Attack (Phase 27a, high finding of M3): the token in argv (`--token <t>`,
    /// `--token=<t>`, with or without an entry) is refused before any connection, and
    /// the message does not repeat the value.
    #[test]
    fn attack_token_in_argv_refused_without_echo() {
        let t = "AbCdEfGhIjKlMnOpQrStUvWxYz012345";
        let with_equals = format!("--token={t}");
        for bad in [
            &["ssh", "--token", t, "e"][..],
            &["ssh", "--token", t],
            &["ssh", "--token"],
            &["ssh", &with_equals, "e"],
            &["ssh", "--token=", "e"],
        ] {
            let e = parse_args(bad).unwrap_err();
            assert_eq!(e, UsageError::TokenInArgv, "{bad:?}");
            assert!(!e.message().contains("AbCd"), "{}", e.message());
        }
        // After the entry, `--token` is an argument of the remote command (like any
        // other), not Ward's token: the daemon never sees it as a token.
        assert_eq!(
            parse_args(&["ssh", "e", "--token", t]),
            Ok(Command::Ssh {
                entry: "e".into(),
                token_fd: None,
                command: vec!["--token".into(), t.into()],
            })
        );
        let m = parse_args(&["ssh", "--token", "falso-SSH-senha-01!"])
            .unwrap_err()
            .message();
        assert!(!m.contains("falso"), "{m}");
        assert!(
            !HELP.contains("--token <"),
            "a ajuda ainda ensina o token em argv"
        );
    }

    #[test]
    fn token_format() {
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
            parse_args(&["playwright", "acme-wp-prod"]),
            Ok(Command::Playwright {
                entry: "acme-wp-prod".into()
            })
        );
        for bad in [
            &["playwright"][..],
            &["playwright", ""],
            &["playwright", "-x"],
            &["playwright", "a", "b"],
            &["playwright", "--", "a"],
        ] {
            assert_eq!(parse_args(bad), Err(UsageError::PlaywrightUsage), "{bad:?}");
        }
        // The usage message does not repeat an argument.
        let m = parse_args(&["playwright", "e", "falso-WP-senha-02"])
            .unwrap_err()
            .message();
        assert!(!m.contains("falso"), "{m}");
    }

    #[test]
    fn login() {
        assert_eq!(
            parse_args(&["login", "acme-wp-prod"]),
            Ok(Command::Login {
                entry: "acme-wp-prod".into()
            })
        );
        for bad in [
            &["login"][..],
            &["login", ""],
            &["login", "-x"],
            &["login", "a", "b"],
            &["login", "--", "a"],
        ] {
            assert_eq!(parse_args(bad), Err(UsageError::LoginUsage), "{bad:?}");
        }
        let m = parse_args(&["login", "e", "falso-WP-senha-02"])
            .unwrap_err()
            .message();
        assert!(!m.contains("falso"), "{m}");
    }

    #[test]
    fn not_utf8() {
        use std::os::unix::ffi::OsStringExt;
        let r = parse([OsString::from_vec(vec![0xff, 0xfe])]);
        assert_eq!(r, Err(UsageError::NotUtf8));
    }
}
