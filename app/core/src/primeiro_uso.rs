//! Primeiro uso (Fase 30): senha mestra e PIN do cofre novo.
//!
//! A interface manda a senha mestra, a confirmação, o PIN e a confirmação do PIN. Tudo
//! chega como [`Digitado`] (zera ao sair, `Debug` sem valor, `Deserialize` que toma
//! posse do texto lido) e é conferido aqui, no lado Rust, antes de ir ao daemon:
//!
//! - senha mestra com [`MIN_CARACTERES_SENHA`] caracteres ou mais (design
//!   `Primeiro uso / Senha mestra`) e até `MAX_SECRET` bytes;
//! - PIN só com dígitos ASCII, de `MIN_PIN_DIGITS` a `MAX_PIN_DIGITS` (R24);
//! - confirmações iguais, comparadas em tempo constante.
//!
//! O daemon recebe só `criar_cofre { senha, pin }` e grava o hash Argon2id do PIN na
//! seção de configuração do corpo (D30); o PIN nunca é gravado em claro. Os erros voltam
//! por campo, com texto fixo, sem valor nenhum.

use std::fmt;

use ctutils::CtEq;
use serde::Deserialize;
use serde::de::{self, Deserializer, Visitor};
use ward_core::limits::{MAX_PIN_DIGITS, MAX_SECRET, MIN_PIN_DIGITS};
use zeroize::Zeroizing;

use crate::formulario::ErroCampo;

/// Mínimo de caracteres da senha mestra (texto de ajuda do design), o mesmo do daemon
/// (Fase 39c): contado em pontos de código depois do NFC.
pub const MIN_CARACTERES_SENHA: usize = ward_core::limits::MIN_MASTER_PASSWORD_CHARS;

pub const SENHA_CURTA: &str = "a senha mestra precisa de 12 caracteres ou mais";
pub const SENHA_LONGA: &str = "a senha mestra passou do tamanho máximo";
pub const SENHA_DIFERENTE: &str = "as duas senhas não são iguais";
pub const PIN_INVALIDO: &str = "o PIN tem de ter de 4 a 6 dígitos, só números";
pub const PIN_DIFERENTE: &str = "os dois PINs não são iguais";

/// Texto digitado na interface (senha mestra ou PIN).
pub struct Digitado(Zeroizing<String>);

impl Digitado {
    pub fn novo(texto: String) -> Self {
        Digitado(Zeroizing::new(texto))
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// A regra do daemon para senha mestra nova (12 caracteres depois do NFC), numa
    /// cópia que zera ao sair. O daemon confere de novo; aqui é para o erro no campo.
    pub(crate) fn serve_de_nova(&self) -> bool {
        ward_core::formato::SenhaMestra::digitada(Zeroizing::new(self.bytes().to_vec()))
            .serve_de_nova()
    }

    pub(crate) fn igual(&self, outro: &Digitado) -> bool {
        // O tamanho não é segredo aqui (as duas vieram da mesma pessoa, na mesma tela).
        self.bytes().len() == outro.bytes().len() && self.bytes().ct_eq(outro.bytes()).to_bool()
    }
}

impl fmt::Debug for Digitado {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Digitado(••••)")
    }
}

impl<'de> Deserialize<'de> for Digitado {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = Digitado;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("texto")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Digitado, E> {
                Ok(Digitado::novo(v.to_owned()))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Digitado, E> {
                // Toma posse do texto já lido: sem cópia que fique sem zerar.
                Ok(Digitado::novo(v))
            }
        }
        d.deserialize_string(V)
    }
}

/// O que a tela `Primeiro uso` manda. Campo a mais é recusado.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NovoCofre {
    pub senha: Digitado,
    pub confirmacao: Digitado,
    pub pin: Digitado,
    pub confirmacao_pin: Digitado,
}

fn erro(campo: &'static str, motivo: &str) -> ErroCampo {
    ErroCampo {
        campo,
        posicao: None,
        motivo: motivo.to_owned(),
    }
}

/// Erros da senha mestra (passo 2), sem olhar o PIN.
pub fn erros_da_senha(senha: &Digitado, confirmacao: &Digitado) -> Vec<ErroCampo> {
    let mut erros = Vec::new();
    if senha.bytes().len() > MAX_SECRET {
        erros.push(erro("senha", SENHA_LONGA));
    } else if !senha.serve_de_nova() {
        erros.push(erro("senha", SENHA_CURTA));
    } else if !senha.igual(confirmacao) {
        erros.push(erro("confirmacao", SENHA_DIFERENTE));
    }
    erros
}

/// PIN de 4 a 6 dígitos ASCII (a mesma regra do protocolo, R24).
pub fn pin_valido(pin: &Digitado) -> bool {
    let b = pin.bytes();
    (MIN_PIN_DIGITS..=MAX_PIN_DIGITS).contains(&b.len()) && b.iter().all(u8::is_ascii_digit)
}

impl NovoCofre {
    /// Todos os erros por campo; vazio se pode ir ao daemon.
    pub fn erros(&self) -> Vec<ErroCampo> {
        let mut erros = erros_da_senha(&self.senha, &self.confirmacao);
        if !pin_valido(&self.pin) {
            erros.push(erro("pin", PIN_INVALIDO));
        } else if !self.pin.igual(&self.confirmacao_pin) {
            erros.push(erro("confirmacao_pin", PIN_DIFERENTE));
        }
        erros
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn d(s: &str) -> Digitado {
        Digitado::novo(s.to_owned())
    }

    fn novo(senha: &str, conf: &str, pin: &str, conf_pin: &str) -> NovoCofre {
        NovoCofre {
            senha: d(senha),
            confirmacao: d(conf),
            pin: d(pin),
            confirmacao_pin: d(conf_pin),
        }
    }

    fn campos(n: &NovoCofre) -> Vec<&'static str> {
        n.erros().iter().map(|e| e.campo).collect()
    }

    const SENHA: &str = "falso-mestra-ward-01";

    #[test]
    fn senha_e_pin_validos_passam() {
        assert!(campos(&novo(SENHA, SENHA, "135790", "135790")).is_empty());
        assert!(campos(&novo(SENHA, SENHA, "1357", "1357")).is_empty());
        // 12 caracteres com acento contam como caracteres, não bytes.
        assert!(campos(&novo("ááááááááááá1", "ááááááááááá1", "0000", "0000")).is_empty());
    }

    #[test]
    fn ataque_pin_com_3_ou_7_digitos_ou_letras_e_recusado() {
        for pin in [
            "",
            "135",
            "1357901",
            "13579a",
            "abcd",
            "１３５７",
            "135 79",
            "-1357",
            "1357\n",
            "13\u{0}57",
            "+1357",
        ] {
            assert_eq!(
                campos(&novo(SENHA, SENHA, pin, pin)),
                vec!["pin"],
                "{pin:?}"
            );
        }
    }

    #[test]
    fn ataque_confirmacao_diferente_e_recusada() {
        assert_eq!(
            campos(&novo(SENHA, "falso-mestra-ward-02", "135790", "135790")),
            vec!["confirmacao"]
        );
        assert_eq!(
            campos(&novo(SENHA, SENHA, "135790", "135791")),
            vec!["confirmacao_pin"]
        );
        // Prefixo da outra não é igual.
        assert_eq!(
            campos(&novo(SENHA, SENHA, "135790", "1357")),
            vec!["confirmacao_pin"]
        );
    }

    #[test]
    fn ataque_senha_curta_ou_grande_demais_e_recusada() {
        assert_eq!(
            campos(&novo("curta-11chr", "curta-11chr", "1357", "1357")),
            vec!["senha"]
        );
        assert_eq!(campos(&novo("", "", "1357", "1357")), vec!["senha"]);
        let grande = "a".repeat(MAX_SECRET + 1);
        let e = novo(&grande, &grande, "1357", "1357").erros();
        assert_eq!(e[0].motivo, SENHA_LONGA);
    }

    #[test]
    fn ataque_erros_e_debug_nao_mostram_valor() {
        let n = novo(SENHA, "falso-mestra-ward-02", "13579a", "13579a");
        let texto = format!("{n:?} {:?}", n.erros());
        for agulha in ["falso", "mestra", "13579"] {
            assert!(!texto.contains(agulha), "{texto}");
        }
    }

    #[test]
    fn ataque_campo_a_mais_ou_tipo_errado_e_recusado() {
        let ok = r#"{"senha":"x","confirmacao":"x","pin":"1","confirmacao_pin":"1"}"#;
        assert!(serde_json::from_str::<NovoCofre>(ok).is_ok());
        for mau in [
            r#"{"senha":"x","confirmacao":"x","pin":"1","confirmacao_pin":"1","nivel":"livre"}"#,
            r#"{"senha":"x","confirmacao":"x","pin":1357,"confirmacao_pin":"1"}"#,
            r#"{"senha":"x","pin":"1","confirmacao_pin":"1"}"#,
            r#"{"senha":["x"],"confirmacao":"x","pin":"1","confirmacao_pin":"1"}"#,
        ] {
            assert!(serde_json::from_str::<NovoCofre>(mau).is_err(), "{mau}");
        }
    }

    mod pelo_servico {
        use std::sync::{Arc, Mutex};

        use zeroize::Zeroizing;

        use super::*;
        use crate::ErroApp;
        use crate::area_transferencia::{AreaTransferencia, Copiador};
        use crate::cliente;
        use crate::daemon::{Daemon, EstadoCofre};
        use crate::daemon_falso::DaemonFalso;
        use crate::servico::Servico;

        type Quadros = Arc<Mutex<Vec<Vec<u8>>>>;

        struct SemArea;
        impl AreaTransferencia for SemArea {
            fn escrever(&self, _: &[u8]) -> Result<(), ErroApp> {
                Ok(())
            }
            fn ler(&self) -> Result<Option<Zeroizing<Vec<u8>>>, ErroApp> {
                Ok(None)
            }
            fn limpar(&self) -> Result<(), ErroApp> {
                Ok(())
            }
        }

        /// Daemon falso que guarda cada quadro recebido (o "daemon" é quem olha).
        struct Espiao(DaemonFalso, Quadros);
        impl Daemon for Espiao {
            fn trocar(&mut self, pedido: &[u8]) -> Result<Zeroizing<Vec<u8>>, ErroApp> {
                self.1.lock().unwrap().push(pedido.to_vec());
                self.0.trocar(pedido)
            }
        }

        fn servico() -> (Servico<Espiao, SemArea>, Quadros) {
            let quadros = Arc::new(Mutex::new(Vec::new()));
            let d = Espiao(DaemonFalso::sem_cofre(), Arc::clone(&quadros));
            (Servico::novo(d, Copiador::novo(SemArea)), quadros)
        }

        #[test]
        fn cria_o_cofre_e_fica_destravado() {
            let (s, quadros) = servico();
            assert_eq!(s.estado_do_cofre(), Ok(EstadoCofre::SemCofre));
            let r = s
                .criar_cofre(novo(SENHA, SENHA, "135790", "135790"))
                .unwrap();
            assert!(r.criado && r.erros.is_empty());
            assert_eq!(s.estado_do_cofre(), Ok(EstadoCofre::Destravado));
            assert!(s.listar_credenciais().unwrap().is_empty());
            // Só senha e PIN vão ao daemon; as confirmações ficam no app.
            let q = quadros.lock().unwrap();
            let criar: Vec<_> = q
                .iter()
                .filter(|q| String::from_utf8_lossy(q).contains("\"criar_cofre\""))
                .collect();
            assert_eq!(criar.len(), 1);
            let t = String::from_utf8_lossy(criar[0]);
            assert!(!t.contains("confirmacao"), "{t}");
        }

        #[test]
        fn daemon_guarda_so_o_hash_do_pin() {
            let mut d = DaemonFalso::sem_cofre();
            cliente::criar_cofre(&mut d, SENHA.as_bytes(), b"135790").unwrap();
            assert!(d.pin_confere(b"135790"));
            assert!(!d.pin_confere(b"135791"));
        }

        #[test]
        fn ataque_pin_ou_confirmacao_invalidos_nao_chegam_ao_daemon() {
            let (s, quadros) = servico();
            for n in [
                novo(SENHA, SENHA, "135", "135"),
                novo(SENHA, SENHA, "1357901", "1357901"),
                novo(SENHA, SENHA, "13579a", "13579a"),
                novo(SENHA, SENHA, "135790", "135791"),
                novo(SENHA, "outra-senha-qualquer", "135790", "135790"),
                novo("curta", "curta", "135790", "135790"),
            ] {
                let r = s.criar_cofre(n).unwrap();
                assert!(!r.criado && !r.erros.is_empty());
            }
            assert!(quadros.lock().unwrap().is_empty());
            assert_eq!(s.estado_do_cofre(), Ok(EstadoCofre::SemCofre));
        }

        #[test]
        fn ataque_criar_por_cima_de_cofre_existente_e_recusado() {
            let (s, _) = servico();
            assert!(
                s.criar_cofre(novo(SENHA, SENHA, "135790", "135790"))
                    .unwrap()
                    .criado
            );
            assert_eq!(
                s.criar_cofre(novo(SENHA, SENHA, "2468", "2468")),
                Err(ErroApp::CofreJaExiste)
            );
            let cheio = Servico::novo(DaemonFalso::cheio(), Copiador::novo(SemArea));
            assert_eq!(
                cheio.criar_cofre(novo(SENHA, SENHA, "2468", "2468")),
                Err(ErroApp::CofreJaExiste)
            );
        }

        #[test]
        fn sem_cofre_o_resto_e_recusado() {
            let (s, _) = servico();
            assert!(s.listar_credenciais().is_err());
            assert!(s.detalhe_credencial("acme-pg-dev").is_err());
        }
    }
}
