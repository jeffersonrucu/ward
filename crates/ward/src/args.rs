//! Linha de comando da CLI. Nenhum argumento é repetido nas mensagens de erro: quem
//! digita `ward destravar <senha>` por engano não vê a senha voltar na tela, no log do
//! terminal nem na saída que o agente lê (S1, S3).

use std::ffi::OsString;

/// O que a CLI vai fazer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comando {
    Lista { json: bool },
    Travar,
    Destravar,
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
}

impl ErroUso {
    pub fn mensagem(self) -> String {
        match self {
            ErroUso::SemComando => "falta o comando".to_owned(),
            ErroUso::ComandoDesconhecido => "comando desconhecido".to_owned(),
            ErroUso::ArgumentoInesperado(cmd) => format!("argumento inesperado para `{cmd}`"),
            ErroUso::NaoUtf8 => "argumento que não é UTF-8".to_owned(),
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
  ward --help           esta ajuda
  ward --version        versão

Nenhum comando devolve segredo. Revelar e copiar existem só no app.
";

/// Lê os argumentos (sem o nome do programa).
pub fn interpretar(args: impl IntoIterator<Item = OsString>) -> Result<Comando, ErroUso> {
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
    fn nao_utf8() {
        use std::os::unix::ffi::OsStringExt;
        let r = interpretar([OsString::from_vec(vec![0xff, 0xfe])]);
        assert_eq!(r, Err(ErroUso::NaoUtf8));
    }
}
