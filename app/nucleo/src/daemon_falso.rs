//! Daemon falso, só nos testes e no `modo-teste` (D7): responde pelo protocolo da Fase
//! 20 com as entradas falsas do cofre de teste (D15 + `acme-totp-dev`, as mesmas de
//! `dev/fixtures/entradas.json`, montadas por `crates/ward-core/src/corpo/cofre_teste.rs`).
//!
//! Serve para a interface ser conferida no Chrome pela ponte de teste antes do `wardd`
//! da Fase 22. Recebe o corpo do quadro, lê com `decodificar_pedido` no papel app, monta
//! a resposta com `ItemLista::de_entrada` / `DetalheEntrada::de_entrada` (sem segredo
//! por construção) e devolve o corpo de `codificar_resposta`. Só atende `estado`,
//! `lista`, `detalhe` e `copiar`; o resto responde `interno`.
//!
//! Sem a feature `modo-teste` este módulo nem existe no binário (só nos testes).

use zeroize::Zeroizing;

use ward_core::entrada::{Entrada, OrigemSegredo, Segredo as SegredoCore};
use ward_core::protocolo::{
    self, Alvo, DetalheEntrada, ErroResposta, ItemLista, Papel, Pedido, Resposta,
};

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
}

impl DaemonFalso {
    /// As 21 entradas do cofre de teste.
    pub fn cheio() -> DaemonFalso {
        DaemonFalso {
            entradas: cofre_teste::cofre_de_teste(),
        }
    }

    /// Cofre destravado sem nenhuma entrada (`Credenciais / Vazia`).
    pub fn vazio() -> DaemonFalso {
        DaemonFalso {
            entradas: Vec::new(),
        }
    }

    fn entrada(&self, nome: &str) -> Option<&Entrada> {
        self.entradas.iter().find(|e| e.nome() == nome)
    }

    fn atender(&self, pedido: Pedido) -> Resposta {
        match pedido {
            Pedido::Estado => Resposta::Estado {
                travado: false,
                cofre_existe: true,
            },
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
            Pedido::Copiar { entrada, alvo } => match self.entrada(&entrada) {
                Some(e) => match valor(e, &alvo) {
                    Some(v) => Resposta::Segredo(SegredoCore::de_bytes(v.to_vec())),
                    None => Resposta::Erro(ErroResposta::CampoInexistente),
                },
                None => Resposta::Erro(ErroResposta::EntradaInexistente),
            },
            _ => Resposta::Erro(ErroResposta::Interno),
        }
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
