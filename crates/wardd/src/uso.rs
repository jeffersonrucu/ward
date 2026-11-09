//! Uso do agente (`executar`, `ssh`, `login`, `playwright`) e a resposta da janela de
//! aprovação (Fase 23; S1, S9, S10; D2, D16, D19, D21, D51).
//!
//! Ordem de um uso:
//! 1. cofre travado: o motor pede o destrave ao app e espera até 2 min (D16);
//! 2. a entrada (bloqueada = inexistente, V3.17) e, no uso combinado, a de túnel do
//!    `banco` ou a de salto do `servidor` (D19);
//! 3. o nível que vale é o mais restritivo das duas; "com aprovação" coberto por uma
//!    liberação de 1 hora ainda dentro do prazo conta como livre (D2);
//! 4. com aprovação: o pedido vai ao app com o comando e a cadeia formatados pela função
//!    de D51 (sem ambiguidade, com escape visível) e espera até 2 min (S10).
//!
//! Os três "Permitir" exigem o PIN, com a contagem de erros do núcleo (S9): o 5.º erro
//! trava o cofre e nega o pedido; antes disso o pedido continua pendente.

use ward_core::entrada::{Ambiente, Dados, Entrada, NivelAcesso, Segredo};
use ward_core::exibicao;
use ward_core::limites::{MAX_TEXTO, MAX_TEXTO_EXIBIDO};
use ward_core::protocolo::{Decisao, ErroResposta, Operacao, Pedido, PedidoDeAprovacao, Resposta};

use crate::aprovacao::{Cliente, UMA_HORA};
use crate::aviso;
use crate::nucleo::{Aberto, Daemon};

/// Maior número de processos da cadeia que vai ao app.
const MAX_CADEIA: usize = 64;

/// O uso foi autorizado: por nível livre (ou liberação de 1 hora) ou pela janela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Autorizacao {
    /// Livre, ou coberto pela liberação de 1 hora.
    SemPerguntar,
    /// Permitido na janela de aprovação.
    Aprovado,
}

/// O mais restritivo de dois níveis (D19).
pub fn mais_restritivo(a: NivelAcesso, b: NivelAcesso) -> NivelAcesso {
    let grau = |n| match n {
        NivelAcesso::Bloqueado => 2,
        NivelAcesso::ComAprovacao => 1,
        NivelAcesso::Livre => 0,
    };
    if grau(a) >= grau(b) { a } else { b }
}

/// O ambiente mais sensível de dois, para o destaque da janela.
fn mais_sensivel(a: Ambiente, b: Ambiente) -> Ambiente {
    let grau = |x| match x {
        Ambiente::Prod => 2,
        Ambiente::Stage => 1,
        Ambiente::Dev => 0,
    };
    if grau(a) >= grau(b) { a } else { b }
}

/// A outra entrada de um uso combinado (D19): túnel do `banco`, salto do `servidor`.
fn segunda_entrada(e: &Entrada, op: Operacao) -> Option<&str> {
    match (e.dados(), op) {
        (Dados::Banco(b), Operacao::Executar) => b.tunel.as_deref(),
        (Dados::Servidor(s), Operacao::Executar | Operacao::Ssh) => s.salto.as_deref(),
        _ => None,
    }
}

/// O nível de uma entrada para este uso, com a liberação de 1 hora (D2).
fn nivel_efetivo(a: &Aberto, e: &Entrada, agora: std::time::Duration) -> NivelAcesso {
    match e.nivel() {
        NivelAcesso::ComAprovacao if a.liberacoes.get(e.nome()).is_some_and(|ate| agora < *ate) => {
            NivelAcesso::Livre
        }
        n => n,
    }
}

/// O que o pedido do agente quer: operação, entrada e o comando exato para exibir.
fn descrever(pedido: &Pedido) -> Option<(Operacao, &str, String)> {
    Some(match pedido {
        Pedido::Executar(x) => (
            Operacao::Executar,
            x.entrada.as_str(),
            exibicao::exibir_argumentos(&x.argv, MAX_TEXTO_EXIBIDO),
        ),
        Pedido::Ssh {
            entrada, comando, ..
        } => (
            Operacao::Ssh,
            entrada.as_str(),
            exibicao::exibir_argumentos(comando, MAX_TEXTO_EXIBIDO),
        ),
        Pedido::Login { entrada } => (Operacao::Login, entrada.as_str(), String::new()),
        Pedido::Playwright { entrada } => (Operacao::Playwright, entrada.as_str(), String::new()),
        _ => return None,
    })
}

/// Cadeia para exibir (D51): nomes com escape visível, no limite do protocolo.
fn cadeia_para_exibir(cliente: &dyn Cliente) -> Vec<String> {
    let mut c = cliente.cadeia();
    if c.len() > MAX_CADEIA {
        c.drain(..c.len() - MAX_CADEIA);
    }
    c.iter()
        .map(|n| exibicao::exibir_texto(n.as_bytes(), MAX_TEXTO))
        .collect()
}

impl Daemon {
    /// O agente pode usar a entrada agora? Espera o destrave e a janela quando precisa.
    /// Bloqueada responde igual a inexistente (V3.17); no uso combinado, a outra
    /// entrada bloqueada (ou sumida) nega sem perguntar.
    pub fn autorizar_uso(
        &self,
        cliente: &dyn Cliente,
        pedido: &Pedido,
    ) -> Result<Autorizacao, ErroResposta> {
        let (op, nome, comando) = descrever(pedido).ok_or(ErroResposta::PedidoInvalido)?;
        if self.travado() {
            self.aprovacao
                .esperar_destrave(&|| self.travado(), cliente)?;
        }
        let agora = self.aprovacao.agora();
        let (nivel, entradas, ambiente) = {
            let est = self.estado();
            let a = est.aberto.as_ref().ok_or(ErroResposta::Travado)?;
            let e = a
                .entradas
                .iter()
                .find(|e| e.nome() == nome && e.nivel() != NivelAcesso::Bloqueado)
                .ok_or(ErroResposta::EntradaInexistente)?;
            let mut nivel = nivel_efetivo(a, e, agora);
            let mut entradas = vec![e.nome().to_owned()];
            let mut ambiente = e.ambiente();
            if let Some(outra) = segunda_entrada(e, op) {
                match a.entradas.iter().find(|x| x.nome() == outra) {
                    Some(x) => {
                        nivel = mais_restritivo(nivel, nivel_efetivo(a, x, agora));
                        ambiente = mais_sensivel(ambiente, x.ambiente());
                        entradas.push(x.nome().to_owned());
                    }
                    None => nivel = NivelAcesso::Bloqueado,
                }
            }
            (nivel, entradas, ambiente)
        };
        match nivel {
            NivelAcesso::Livre => Ok(Autorizacao::SemPerguntar),
            NivelAcesso::Bloqueado => {
                aviso!("uso combinado com entrada bloqueada: negado sem perguntar (D19)");
                Err(ErroResposta::Negado)
            }
            NivelAcesso::ComAprovacao => {
                let p = PedidoDeAprovacao {
                    id: 0,
                    entradas,
                    ambiente,
                    operacao: op,
                    comando,
                    cadeia: cadeia_para_exibir(cliente),
                };
                self.aprovacao.pedir_aprovacao(p, cliente)?;
                Ok(Autorizacao::Aprovado)
            }
        }
    }

    /// Resposta da janela (só do papel app, pela tabela do protocolo). "Negar" não pede
    /// PIN; os três "Permitir" pedem (S10). PIN errado deixa o pedido pendente; o 5.º
    /// erro trava o cofre e nega (S9).
    pub(crate) fn responder_aprovacao(
        &self,
        id: u64,
        decisao: Decisao,
        pin: Option<&Segredo>,
    ) -> Result<Resposta, ErroResposta> {
        let entradas = self.aprovacao.pendente(id).ok_or(ErroResposta::Expirado)?;
        if decisao == Decisao::Negar {
            return if self.aprovacao.decidir(id, Err(ErroResposta::Negado)) {
                Ok(Resposta::Ok)
            } else {
                Err(ErroResposta::Expirado)
            };
        }
        // O protocolo já exige o PIN em todo "Permitir"; aqui é a segunda camada.
        let pin = pin.ok_or(ErroResposta::PedidoInvalido)?;
        let mut est = self.estado();
        if let Err(e) = Self::conferir_pin(&mut est, pin) {
            if est.aberto.is_none() {
                // O 5.º erro travou (ou o cofre travou no meio): o pedido é negado.
                self.aprovacao.decidir(id, Err(ErroResposta::Negado));
            }
            return Err(e);
        }
        match decisao {
            Decisao::Negar | Decisao::PermitirUmaVez => {}
            Decisao::PermitirUmaHora => {
                if !self.aprovacao.decidir(id, Ok(())) {
                    return Err(ErroResposta::Expirado);
                }
                let ate = self.aprovacao.agora().saturating_add(UMA_HORA);
                let a = est.aberto.as_mut().ok_or(ErroResposta::Travado)?;
                for n in entradas {
                    a.liberacoes.insert(n, ate);
                }
                aviso!("aprovação {id}: permitida por 1 hora");
                return Ok(Resposta::Ok);
            }
            Decisao::PermitirSempre => {
                // As entradas do pedido que estão "com aprovação" viram "livre", no disco.
                let a = est.aberto.as_mut().ok_or(ErroResposta::Travado)?;
                let mut mudadas = Vec::new();
                for e in a.entradas.iter_mut() {
                    if entradas.iter().any(|n| n == e.nome())
                        && e.nivel() == NivelAcesso::ComAprovacao
                    {
                        e.definir_nivel(NivelAcesso::Livre);
                        mudadas.push(e.nome().to_owned());
                    }
                }
                for n in &mudadas {
                    a.liberacoes.remove(n);
                }
                // Falhou ao gravar: o nível volta e o pedido continua pendente.
                self.gravar_ou_desfazer(&mut est, move |a| {
                    for e in a.entradas.iter_mut() {
                        if mudadas.iter().any(|n| n == e.nome()) {
                            e.definir_nivel(NivelAcesso::ComAprovacao);
                        }
                    }
                })?;
                aviso!("aprovação {id}: permitida sempre (entrada agora livre)");
            }
        }
        if self.aprovacao.decidir(id, Ok(())) {
            Ok(Resposta::Ok)
        } else {
            Err(ErroResposta::Expirado)
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nivel_combinado_e_o_mais_restritivo() {
        use NivelAcesso::*;
        for (a, b, r) in [
            (Livre, Livre, Livre),
            (Livre, ComAprovacao, ComAprovacao),
            (ComAprovacao, Livre, ComAprovacao),
            (Livre, Bloqueado, Bloqueado),
            (Bloqueado, ComAprovacao, Bloqueado),
            (ComAprovacao, ComAprovacao, ComAprovacao),
        ] {
            assert_eq!(mais_restritivo(a, b), r);
        }
    }
}
