//! Os pedidos do app ao daemon pelo protocolo da Fase 20 (`ward_core::protocolo`), com
//! o papel app (D5): `estado`, `lista`, `detalhe`, `copiar`, `revelar`, `totp` e
//! `salvar_entrada`.
//!
//! Aqui o pedido vira o corpo do quadro (`codificar_pedido`) e a resposta é lida com a
//! leitura estrita do cliente (`decodificar_resposta`), que recusa tipo de outro papel e
//! detalhe forjado (extra secreto com valor, campo secreto em `campos`). O transporte
//! é o [`Daemon`]: o socket do `wardd` ([`crate::daemon::DaemonSocket`]) ou, nos testes
//! e na ponte de teste, o daemon falso ([`crate::daemon_falso`]).
//!
//! Nenhuma função daqui devolve segredo à interface: [`copiar`] e [`revelar`] leem um
//! valor em [`Segredo`] (que zera ao sair); quem decide o destino é o
//! [`crate::servico`]: a área de transferência (D4), o "revelar" no clique (S11) ou o
//! rascunho que volta ao daemon ao editar (Fase 33).

use ward_core::entrada::Rascunho;
use ward_core::protocolo::{
    self, Alvo, DetalheEntrada, ErroResposta, ItemLista, Papel, Pedido, Problema, Resposta,
};
use zeroize::Zeroizing;

use crate::ErroApp;
use crate::daemon::{Daemon, EstadoCofre};
use crate::segredo::Segredo;

/// Um pedido e a resposta já lida. Um `erro` do daemon vira [`ErroApp`] com mensagem
/// fixa (o código do protocolo não vai à interface).
fn pedir<D: Daemon + ?Sized>(daemon: &mut D, pedido: &Pedido) -> Result<Resposta, ErroApp> {
    let quadro =
        protocolo::codificar_pedido(Papel::App, pedido).map_err(|_| ErroApp::PedidoInvalido)?;
    let corpo = daemon.trocar(&quadro)?;
    match protocolo::decodificar_resposta(Papel::App, &corpo) {
        Ok(Resposta::Erro(e)) => Err(erro_do_daemon(&e)),
        Ok(r) => Ok(r),
        Err(_) => Err(ErroApp::RespostaInvalida),
    }
}

/// O erro do protocolo que a tela distingue; o resto é "recusado".
fn erro_do_daemon(e: &ErroResposta) -> ErroApp {
    match e {
        ErroResposta::Travado | ErroResposta::TravadoTempoEsgotado => ErroApp::Travado,
        ErroResposta::EntradaInexistente => ErroApp::EntradaInexistente,
        ErroResposta::CampoInexistente => ErroApp::CampoInexistente,
        ErroResposta::NaoAutorizado => ErroApp::NaoAutorizado,
        _ => ErroApp::Recusado,
    }
}

/// `estado`.
pub fn estado<D: Daemon + ?Sized>(daemon: &mut D) -> Result<EstadoCofre, ErroApp> {
    match pedir(daemon, &Pedido::Estado)? {
        Resposta::Estado {
            cofre_existe: false,
            ..
        } => Ok(EstadoCofre::SemCofre),
        Resposta::Estado { travado: true, .. } => Ok(EstadoCofre::Travado),
        Resposta::Estado { travado: false, .. } => Ok(EstadoCofre::Destravado),
        _ => Err(ErroApp::RespostaInvalida),
    }
}

/// `lista`: todas as entradas (o app vê também as bloqueadas), sem segredo.
pub fn lista<D: Daemon + ?Sized>(daemon: &mut D) -> Result<Vec<ItemLista>, ErroApp> {
    match pedir(daemon, &Pedido::Lista)? {
        Resposta::Lista(itens) => Ok(itens),
        _ => Err(ErroApp::RespostaInvalida),
    }
}

/// `detalhe` de uma entrada: campos de texto, nomes dos segredos, extras sem o valor
/// secreto e anexos sem o conteúdo. A resposta tem de ser da entrada pedida.
pub fn detalhe<D: Daemon + ?Sized>(
    daemon: &mut D,
    entrada: &str,
) -> Result<DetalheEntrada, ErroApp> {
    let pedido = Pedido::Detalhe {
        entrada: entrada.to_owned(),
    };
    match pedir(daemon, &pedido)? {
        Resposta::Detalhe(d) if d.nome == entrada => Ok(d),
        _ => Err(ErroApp::RespostaInvalida),
    }
}

/// `copiar`: o valor de um segredo, só para a área de transferência (D4).
pub fn copiar<D: Daemon + ?Sized>(
    daemon: &mut D,
    entrada: &str,
    alvo: Alvo,
) -> Result<Segredo, ErroApp> {
    let pedido = Pedido::Copiar {
        entrada: entrada.to_owned(),
        alvo,
    };
    match pedir(daemon, &pedido)? {
        Resposta::Segredo(s) => Ok(Segredo::novo(s.expor().to_vec())),
        _ => Err(ErroApp::RespostaInvalida),
    }
}

/// `revelar`: o valor de um segredo para mostrar no clique em "revelar" (S11) ou para
/// manter o valor ao editar a entrada (o rascunho leva todos os segredos).
pub fn revelar<D: Daemon + ?Sized>(
    daemon: &mut D,
    entrada: &str,
    alvo: Alvo,
) -> Result<Segredo, ErroApp> {
    let pedido = Pedido::Revelar {
        entrada: entrada.to_owned(),
        alvo,
    };
    match pedir(daemon, &pedido)? {
        Resposta::Segredo(s) => Ok(Segredo::novo(s.expor().to_vec())),
        _ => Err(ErroApp::RespostaInvalida),
    }
}

/// Código TOTP atual de uma entrada e os segundos até o próximo.
pub struct CodigoTotp {
    pub codigo: Zeroizing<String>,
    pub restante: u64,
}

/// `totp`: o código atual (6 dígitos, D35) e quanto falta para trocar (1 a 30 s).
pub fn totp<D: Daemon + ?Sized>(daemon: &mut D, entrada: &str) -> Result<CodigoTotp, ErroApp> {
    let pedido = Pedido::Totp {
        entrada: entrada.to_owned(),
    };
    match pedir(daemon, &pedido)? {
        Resposta::CodigoTotp { codigo, restante }
            if codigo.len() == 6 && (1..=30).contains(&restante) =>
        {
            Ok(CodigoTotp { codigo, restante })
        }
        _ => Err(ErroApp::RespostaInvalida),
    }
}

/// Resultado do `salvar_entrada`: salvo, ou os problemas de validação por campo (só
/// textos fixos do `ward-core`; nenhum valor do usuário).
pub enum Salvamento {
    Salvo,
    Problemas(Vec<Problema>),
}

/// `salvar_entrada`: cria (`anterior` vazio) ou edita. O nível não vai no pedido (S9).
pub fn salvar<D: Daemon + ?Sized>(
    daemon: &mut D,
    anterior: Option<String>,
    rascunho: Rascunho,
) -> Result<Salvamento, ErroApp> {
    let pedido = Pedido::SalvarEntrada { anterior, rascunho };
    let quadro =
        protocolo::codificar_pedido(Papel::App, &pedido).map_err(|_| ErroApp::PedidoInvalido)?;
    drop(pedido);
    let corpo = daemon.trocar(&quadro)?;
    drop(quadro);
    match protocolo::decodificar_resposta(Papel::App, &corpo) {
        Ok(Resposta::Ok) => Ok(Salvamento::Salvo),
        Ok(Resposta::Erro(ErroResposta::Validacao(p))) => Ok(Salvamento::Problemas(p)),
        Ok(Resposta::Erro(e)) => Err(erro_do_daemon(&e)),
        _ => Err(ErroApp::RespostaInvalida),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::daemon_falso::DaemonFalso;

    /// Daemon que devolve sempre o mesmo corpo (o "servidor" é o atacante).
    struct Resposta1(Vec<u8>);

    impl Daemon for Resposta1 {
        fn trocar(&mut self, _pedido: &[u8]) -> Result<Zeroizing<Vec<u8>>, ErroApp> {
            Ok(Zeroizing::new(self.0.clone()))
        }
    }

    #[test]
    fn estado_lista_e_detalhe_pelo_protocolo() {
        let mut d = DaemonFalso::cheio();
        assert_eq!(estado(&mut d), Ok(EstadoCofre::Destravado));
        let itens = lista(&mut d).unwrap();
        assert_eq!(
            itens.len(),
            serde_json::from_str::<serde_json::Value>(include_str!(
                "../../../dev/fixtures/entradas.json"
            ))
            .unwrap()["entradas"]
                .as_array()
                .unwrap()
                .len()
        );
        let det = detalhe(&mut d, "acme-pg-dev").unwrap();
        assert_eq!(det.segredos, vec!["senha".to_owned()]);
        assert_eq!(
            detalhe(&mut d, "nao-existe").unwrap_err(),
            ErroApp::EntradaInexistente
        );
        assert_eq!(
            estado(&mut DaemonFalso::vazio()),
            Ok(EstadoCofre::Destravado)
        );
        assert!(lista(&mut DaemonFalso::vazio()).unwrap().is_empty());
    }

    #[test]
    fn ataque_resposta_de_outro_tipo_ou_forjada_e_recusada() {
        // Um "daemon" que responde credencial ao pedido de lista, um detalhe de outra
        // entrada, um detalhe com o valor do extra secreto e uma resposta de outra
        // versão: tudo vira `RespostaInvalida`, nada chega à interface.
        let segredo_no_lugar_da_lista =
            br#"{"v":1,"tipo":"segredo","dados":{"valor":"ZmFsc28tUEc="}}"#.to_vec();
        assert_eq!(
            lista(&mut Resposta1(segredo_no_lugar_da_lista)).unwrap_err(),
            ErroApp::RespostaInvalida
        );
        let mut falso = DaemonFalso::cheio();
        let quadro = protocolo::codificar_pedido(
            Papel::App,
            &Pedido::Detalhe {
                entrada: "acme-pg-dev".into(),
            },
        )
        .unwrap();
        let outro = falso.trocar(&quadro).unwrap().to_vec();
        assert_eq!(
            detalhe(&mut Resposta1(outro), "acme-pg-prod").unwrap_err(),
            ErroApp::RespostaInvalida
        );
        // O detalhe verdadeiro de `gama-generico`, com o valor do extra secreto
        // enfiado pelo "daemon".
        let quadro = protocolo::codificar_pedido(
            Papel::App,
            &Pedido::Detalhe {
                entrada: "gama-generico".into(),
            },
        )
        .unwrap();
        let verdadeiro = String::from_utf8(falso.trocar(&quadro).unwrap().to_vec()).unwrap();
        let alvo = r#""segredo":true"#;
        assert!(verdadeiro.contains(alvo), "{verdadeiro}");
        let forjado = verdadeiro
            .replacen(alvo, r#""segredo":true,"valor":"falso-licenca-01""#, 1)
            .into_bytes();
        assert_eq!(
            detalhe(&mut Resposta1(forjado), "gama-generico").unwrap_err(),
            ErroApp::RespostaInvalida
        );
        let versao = br#"{"v":2,"tipo":"lista","dados":{"itens":[]}}"#.to_vec();
        assert_eq!(
            lista(&mut Resposta1(versao)).unwrap_err(),
            ErroApp::RespostaInvalida
        );
    }

    #[test]
    fn erros_do_daemon_tem_mensagem_fixa() {
        let nao = br#"{"v":1,"tipo":"erro","dados":{"codigo":"nao_autorizado","mensagem":"<img src=x> falso-PG:s3nha/01@#"}}"#.to_vec();
        let e = lista(&mut Resposta1(nao)).unwrap_err();
        assert_eq!(e, ErroApp::NaoAutorizado);
        assert!(!e.to_string().contains("img"));
        assert!(!e.to_string().contains("falso"));
        let travado =
            br#"{"v":1,"tipo":"erro","dados":{"codigo":"travado","mensagem":"x"}}"#.to_vec();
        assert_eq!(
            estado(&mut Resposta1(travado)).unwrap_err(),
            ErroApp::Travado
        );
    }
}
