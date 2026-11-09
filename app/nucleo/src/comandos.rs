//! Comandos próprios do app: a única lista do que a interface pode pedir (S11).
//!
//! O `build.rs` do Tauri registra exatamente estes nomes no manifesto do app (sem ele,
//! todo comando registrado ficaria liberado), a capability `principal` libera só as
//! permissões `allow-<comando>` deles, e a ponte de teste recusa qualquer outro nome.
//! Fases seguintes acrescentam comandos aqui, um por tela que precisa.

use ward_core::limites;

use crate::ErroApp;

/// `estado_do_cofre`: travado, destravado ou sem cofre. Sem argumento.
pub const ESTADO_DO_COFRE: &str = "estado_do_cofre";
/// `copiar_campo { entrada, campo }`: o valor vai do daemon para a área de
/// transferência pelo lado Rust (D4); a resposta não traz o valor.
pub const COPIAR_CAMPO: &str = "copiar_campo";

/// Todos os comandos, na ordem em que aparecem nas capabilities.
pub const COMANDOS: &[&str] = &[ESTADO_DO_COFRE, COPIAR_CAMPO];

/// Nome da permissão gerada pelo Tauri para o comando (`allow-estado-do-cofre`).
pub fn permissao(comando: &str) -> String {
    format!("allow-{}", comando.replace('_', "-"))
}

/// Comando conhecido, a partir do nome que veio da interface (não confiável).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comando {
    EstadoDoCofre,
    CopiarCampo,
}

impl Comando {
    pub fn de_nome(nome: &str) -> Option<Comando> {
        match nome {
            ESTADO_DO_COFRE => Some(Comando::EstadoDoCofre),
            COPIAR_CAMPO => Some(Comando::CopiarCampo),
            _ => None,
        }
    }
}

/// Qual campo de qual entrada. Validado antes de chegar ao daemon: nome da entrada até
/// `MAX_NOME`, nome do campo até `MAX_NOME_EXTRA`, sem controle e sem espaço nas
/// pontas. O daemon valida de novo (a entrada também não é confiável lá).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Referencia {
    entrada: String,
    campo: String,
}

impl Referencia {
    pub fn nova(entrada: &str, campo: &str) -> Result<Referencia, ErroApp> {
        if !nome_valido(entrada, limites::MAX_NOME) || !nome_valido(campo, limites::MAX_NOME_EXTRA)
        {
            return Err(ErroApp::PedidoInvalido);
        }
        Ok(Referencia {
            entrada: entrada.to_owned(),
            campo: campo.to_owned(),
        })
    }

    pub fn entrada(&self) -> &str {
        &self.entrada
    }

    pub fn campo(&self) -> &str {
        &self.campo
    }
}

fn nome_valido(nome: &str, max: usize) -> bool {
    !nome.is_empty()
        && nome.chars().count() <= max
        && nome.trim() == nome
        && !nome.chars().any(|c| c.is_control() || matches!(c, '\u{200B}'..='\u{200F}' | '\u{2028}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}'))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn lista_e_permissoes() {
        assert_eq!(COMANDOS, &["estado_do_cofre", "copiar_campo"]);
        assert_eq!(permissao(COPIAR_CAMPO), "allow-copiar-campo");
        for c in COMANDOS {
            assert!(Comando::de_nome(c).is_some());
        }
    }

    #[test]
    fn ataque_comando_inventado_ou_de_plugin_nao_existe() {
        for nome in [
            "",
            "plugin:fs|read_text_file",
            "plugin:shell|execute",
            "plugin:http|fetch",
            "revelar_tudo",
            "COPIAR_CAMPO",
            "copiar_campo ",
            "copiar-campo",
        ] {
            assert_eq!(Comando::de_nome(nome), None, "{nome}");
        }
    }

    #[test]
    fn ataque_referencia_fora_do_formato() {
        let longo = "a".repeat(limites::MAX_NOME + 1);
        let longo_campo = "a".repeat(limites::MAX_NOME_EXTRA + 1);
        for (e, c) in [
            ("", "senha"),
            ("acme-pg-dev", ""),
            (longo.as_str(), "senha"),
            ("acme-pg-dev", longo_campo.as_str()),
            (" acme-pg-dev", "senha"),
            ("acme\npg", "senha"),
            ("acme-pg-dev", "se\u{0}nha"),
            ("acme\u{202E}gp", "senha"),
            ("acme-pg-dev", "senha\u{200B}"),
        ] {
            assert_eq!(
                Referencia::nova(e, c),
                Err(ErroApp::PedidoInvalido),
                "{e:?} {c:?}"
            );
        }
        let r = Referencia::nova("acme-pg-dev", "senha").unwrap();
        assert_eq!((r.entrada(), r.campo()), ("acme-pg-dev", "senha"));
        assert!(Referencia::nova(&"á".repeat(limites::MAX_NOME), "senha").is_ok());
    }
}
