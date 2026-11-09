//! Daemon falso, só nos testes e no `modo-teste` (D7): responde pelo protocolo da Fase
//! 20 com as entradas falsas do cofre de teste (D15 + `acme-totp-dev`, as mesmas de
//! `dev/fixtures/entradas.json`, montadas por `crates/ward-core/src/corpo/cofre_teste.rs`).
//!
//! Serve para a interface ser conferida no Chrome pela ponte de teste antes do `wardd`
//! da Fase 22. Recebe o corpo do quadro, lê com `decodificar_pedido` no papel app, monta
//! a resposta com `ItemLista::de_entrada` / `DetalheEntrada::de_entrada` (sem segredo
//! por construção) e devolve o corpo de `codificar_resposta`. Atende `estado`, `lista`,
//! `detalhe`, `copiar`, `revelar`, `totp`, `emitir_token` (Fase 34) e `salvar_entrada` (Fase 33, com a mesma
//! regra do `wardd`: `Rascunho::validar`, `conferir_no_cofre`, nível "com aprovação" na
//! entrada nova e mantido na editada) e, no [`DaemonFalso::sem_cofre`], `criar_cofre`
//! (Fase 30, guarda só o hash do PIN, D30); o resto responde `interno`. As mudanças ficam
//! só na memória do processo.
//!
//! Sem a feature `modo-teste` este módulo nem existe no binário (só nos testes).

use zeroize::Zeroizing;

use ward_core::configuracao::HashPin;
use ward_core::entrada::{
    Entrada, NivelAcesso, OrigemSegredo, Rascunho, Segredo as SegredoCore, TipoEntrada,
};
use ward_core::protocolo::{
    self, Alvo, DetalheEntrada, ErroResposta, ItemLista, Papel, Pedido, Resposta,
};
use ward_core::totp::Totp;

use crate::ErroApp;
use crate::daemon::Daemon;

// O arquivo do cofre de teste usa a API pública do `ward-core` pelo apelido `wc`.
use ward_core as wc;
#[path = "../../../crates/ward-core/src/corpo/cofre_teste.rs"]
mod cofre_teste;

/// Os valores secretos do cofre de teste (as agulhas dos testes de ataque).
pub use cofre_teste::SEGREDOS;

/// Daemon falso com as entradas em memória.
pub struct DaemonFalso {
    entradas: Vec<Entrada>,
    /// `false` só no [`DaemonFalso::sem_cofre`] até o `criar_cofre` (Fase 30).
    existe: bool,
    /// Hash do PIN do cofre criado pelo `criar_cofre`, como o `wardd` o guarda (D30).
    hash_pin: Option<HashPin>,
}

impl DaemonFalso {
    /// As 21 entradas do cofre de teste.
    pub fn cheio() -> DaemonFalso {
        DaemonFalso {
            entradas: cofre_teste::cofre_de_teste(),
            existe: true,
            hash_pin: None,
        }
    }

    /// Cofre destravado sem nenhuma entrada (`Credenciais / Vazia`).
    pub fn vazio() -> DaemonFalso {
        DaemonFalso {
            entradas: Vec::new(),
            existe: true,
            hash_pin: None,
        }
    }

    /// Sem cofre ainda (`Primeiro uso / *`, Fase 30): `estado` responde
    /// `cofre_existe: false`, o resto responde "ainda não existe cofre" até o
    /// `criar_cofre`, que deixa o cofre vazio e destravado, como o `wardd`.
    pub fn sem_cofre() -> DaemonFalso {
        DaemonFalso {
            entradas: Vec::new(),
            existe: false,
            hash_pin: None,
        }
    }

    /// O PIN confere com o hash guardado pelo `criar_cofre` (só para os testes).
    pub fn pin_confere(&self, pin: &[u8]) -> bool {
        self.hash_pin.as_ref().is_some_and(|h| h.conferir(pin))
    }

    fn entrada(&self, nome: &str) -> Option<&Entrada> {
        self.entradas.iter().find(|e| e.nome() == nome)
    }

    fn atender(&mut self, pedido: Pedido) -> Resposta {
        if !self.existe {
            return match pedido {
                Pedido::Estado => Resposta::Estado {
                    travado: true,
                    cofre_existe: false,
                    papel: Papel::App,
                },
                // Como o `wardd` (`nucleo.rs` › `criar_cofre`): só o hash do PIN fica.
                Pedido::CriarCofre { senha: _, pin } => match HashPin::criar(pin.expor()) {
                    Ok(h) => {
                        self.hash_pin = Some(h);
                        self.existe = true;
                        Resposta::Ok
                    }
                    Err(_) => Resposta::Erro(ErroResposta::PedidoInvalido),
                },
                _ => Resposta::Erro(ErroResposta::CofreInexistente),
            };
        }
        match pedido {
            Pedido::Estado => Resposta::Estado {
                travado: false,
                cofre_existe: true,
                papel: Papel::App,
            },
            Pedido::CriarCofre { .. } => Resposta::Erro(ErroResposta::CofreJaExiste),
            Pedido::Lista => Resposta::Lista(
                self.entradas
                    .iter()
                    .map(|e| ItemLista::de_entrada(e, Papel::App))
                    .collect(),
            ),
            Pedido::Detalhe { entrada } => match self.entrada(&entrada) {
                Some(e) => Resposta::Detalhe(DetalheEntrada::de_entrada(e)),
                None => Resposta::Erro(ErroResposta::EntradaInexistente),
            },
            Pedido::Copiar { entrada, alvo } | Pedido::Revelar { entrada, alvo } => {
                match self.entrada(&entrada) {
                    Some(e) => match valor(e, &alvo) {
                        Some(v) => Resposta::Segredo(SegredoCore::de_bytes(v.to_vec())),
                        None => Resposta::Erro(ErroResposta::CampoInexistente),
                    },
                    None => Resposta::Erro(ErroResposta::EntradaInexistente),
                }
            }
            Pedido::Totp { entrada } => match self.entrada(&entrada) {
                Some(e) => match valor(e, &Alvo::Campo("totp_semente".to_owned())) {
                    Some(v) => {
                        let semente = SegredoCore::de_bytes(v.to_vec());
                        match Totp::de_semente(&semente).and_then(|t| t.codigo_agora()) {
                            Ok(c) => Resposta::CodigoTotp {
                                codigo: Zeroizing::new(c.codigo().to_owned()),
                                restante: c.restante(),
                            },
                            Err(_) => Resposta::Erro(ErroResposta::Interno),
                        }
                    }
                    None => Resposta::Erro(ErroResposta::CampoInexistente),
                },
                None => Resposta::Erro(ErroResposta::EntradaInexistente),
            },
            Pedido::SalvarEntrada { anterior, rascunho } => {
                match self.salvar(anterior.as_deref(), rascunho) {
                    Ok(()) => Resposta::Ok,
                    Err(e) => Resposta::Erro(e),
                }
            }
            // Fase 34: o token do "Conectar" (D3), só para `servidor`. O falso não abre
            // sessão nenhuma; o token é aleatório e não é guardado.
            Pedido::EmitirToken { entrada } => match self.entrada(&entrada) {
                Some(e) if e.tipo() == TipoEntrada::Servidor => {
                    match ward_core::gerador::gerar(&ward_core::gerador::OpcoesGerador {
                        tamanho: 32,
                        minusculas: true,
                        maiusculas: true,
                        digitos: true,
                        simbolos: false,
                    }) {
                        Ok(t) => Resposta::Token(t),
                        Err(_) => Resposta::Erro(ErroResposta::Interno),
                    }
                }
                Some(_) => Resposta::Erro(ErroResposta::PedidoInvalido),
                None => Resposta::Erro(ErroResposta::EntradaInexistente),
            },
            _ => Resposta::Erro(ErroResposta::Interno),
        }
    }

    /// A mesma regra do `wardd` (`crates/wardd/src/nucleo.rs` › `salvar_entrada`).
    fn salvar(&mut self, anterior: Option<&str>, rascunho: Rascunho) -> Result<(), ErroResposta> {
        let mut nova = rascunho
            .validar()
            .map_err(|e| ErroResposta::de_validacao(&e))?;
        match anterior {
            None => {
                nova.definir_nivel(NivelAcesso::ComAprovacao);
                nova.conferir_no_cofre(self.entradas.iter())
                    .map_err(|e| ErroResposta::de_validacao(&e))?;
            }
            Some(nome) => {
                let i = self
                    .entradas
                    .iter()
                    .position(|e| e.nome() == nome)
                    .ok_or(ErroResposta::EntradaInexistente)?;
                nova.definir_nivel(self.entradas[i].nivel());
                nova.conferir_no_cofre(self.entradas.iter().filter(|e| e.nome() != nome))
                    .map_err(|e| ErroResposta::de_validacao(&e))?;
                drop(self.entradas.remove(i));
            }
        }
        self.entradas.push(nova);
        self.entradas.sort_by(|a, b| a.nome().cmp(b.nome()));
        Ok(())
    }
}

/// Nome do segredo do tipo em `Entrada::segredos` a partir do identificador do
/// protocolo.
fn origem_do_campo(id: &str) -> Option<&'static str> {
    Some(match id {
        "senha" => "senha",
        "totp_semente" => "semente TOTP",
        "chave_privada" => "chave privada",
        "passphrase" => "passphrase",
        "token" => "token",
        "secret_access_key" => "secret access key",
        "session_token" => "session token",
        "texto" => "texto",
        _ => return None,
    })
}

fn valor<'a>(e: &'a Entrada, alvo: &Alvo) -> Option<&'a [u8]> {
    e.segredos().into_iter().find_map(|s| {
        let bate = match (alvo, s.origem) {
            (Alvo::Campo(id), OrigemSegredo::Campo(nome)) => origem_do_campo(id) == Some(nome),
            (Alvo::Extra(x), OrigemSegredo::Extra(nome)) => x == nome,
            _ => false,
        };
        bate.then(|| s.valor.expor())
    })
}

impl Daemon for DaemonFalso {
    fn trocar(&mut self, pedido: &[u8]) -> Result<Zeroizing<Vec<u8>>, ErroApp> {
        let resposta = match protocolo::decodificar_pedido(Papel::App, pedido) {
            Ok(p) => self.atender(p),
            Err(e) => Resposta::Erro(e),
        };
        protocolo::codificar_resposta(Papel::App, &resposta).map_err(|_| ErroApp::RespostaInvalida)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn mesmas_entradas_das_fixtures() {
        // `dev/fixtures/entradas.json` é a fonte das entradas do cofre de teste; o
        // daemon falso tem exatamente os mesmos nomes, tipos, grupos, ambientes e níveis.
        let caminho = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dev/fixtures/entradas.json"
        );
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(caminho).unwrap()).unwrap();
        let mut esperado: Vec<(String, String, String, String, String)> = json["entradas"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                let t = |k: &str| e[k].as_str().unwrap().to_owned();
                (t("nome"), t("tipo"), t("grupo"), t("ambiente"), t("nivel"))
            })
            .collect();
        let mut falso: Vec<_> = DaemonFalso::cheio()
            .entradas
            .iter()
            .map(|e| {
                (
                    e.nome().to_owned(),
                    e.tipo().nome().to_owned(),
                    e.grupo().to_owned(),
                    e.ambiente().nome().to_owned(),
                    e.nivel().nome().to_owned(),
                )
            })
            .collect();
        esperado.sort();
        falso.sort();
        assert_eq!(falso, esperado);
    }

    #[test]
    fn so_o_papel_app_e_atendido() {
        // O falso lê o pedido como o daemon real leria do app; um quadro de agente com
        // operação só do app vira "não autorizado" do lado do agente, e um corpo
        // inválido vira erro, nunca pânico.
        let mut d = DaemonFalso::cheio();
        for lixo in [&b""[..], b"{}", b"{\"v\":1,\"op\":\"revelar\"}", b"\xff"] {
            let r = d.trocar(lixo).unwrap();
            let r = protocolo::decodificar_resposta(Papel::App, &r).unwrap();
            assert!(matches!(r, Resposta::Erro(_)));
        }
    }
}
