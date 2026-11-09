//! `ward login` (Fase 43; R14, D9; S1, S13): o daemon leva o pedido à extensão do Chrome,
//! que confere a origem da aba ativa, preenche, envia e limpa os campos.
//!
//! O daemon não fala com a extensão por conta própria: o service worker dela mantém o
//! `ward-nm` aberto e pergunta `aguardar_login` (espera longa, até
//! [`PrazosLogin::aguardar`]). O caminho de um pedido:
//!
//! 1. o agente pede `login <entrada>`: tipo `login` conferido antes da aprovação, depois
//!    o nível e a aprovação ([`Daemon::autorizar_uso`]: bloqueada = inexistente, V3.17);
//! 2. o daemon cria um pedido com id aleatório (32 letras e dígitos, do RNG do sistema)
//!    e o entrega à primeira extensão que perguntar: `pedido_de_login` leva só o id e o
//!    nome da entrada, nenhum valor;
//! 3. a extensão manda `preencher_login` com o id e a origem do documento do quadro
//!    principal da aba ativa. O daemon compara a origem com a URL da entrada pelo mesmo
//!    `preencher` da Fase 40 (D9): diferente, o pedido acaba em `origem_diferente` e
//!    nenhum valor sai; igual, sai o `preenchimento` (só para o papel extensão). Um id
//!    só preenche uma vez;
//! 4. a extensão diz o desfecho (`resultado_login`) e o agente recebe `ok` ou o erro.
//!
//! Um pedido por vez (o segundo agente recebe `ocupado`). Ninguém pegou o pedido em
//! [`PrazosLogin::retirada`], ou a extensão não terminou em [`PrazosLogin::total`]: o
//! agente recebe `navegador_falhou` e o id morre. Cofre travado no meio, ou a CLI que
//! caiu: o pedido morre e nada mais é preenchido. O log leva só o nome da entrada e o
//! desfecho, nunca a origem nem valores (S13).

use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use ward_core::entrada::{Dados, NivelAcesso};
use ward_core::gerador;
use ward_core::protocolo::{ErroResposta, Papel, Pedido, Resposta, ResultadoLogin};

use crate::aprovacao::Cliente;
use crate::aviso;
use crate::nucleo::Daemon;

/// Prazos do `ward login`.
#[derive(Debug, Clone, Copy)]
pub struct PrazosLogin {
    /// Quanto o `aguardar_login` da extensão espera um pedido antes de responder `ok`
    /// (vazio) e perguntar de novo.
    pub aguardar: Duration,
    /// Quanto o pedido espera uma extensão pegá-lo.
    pub retirada: Duration,
    /// Quanto o pedido espera o desfecho, contado de quando a extensão o pegou (o envio
    /// e a página seguinte carregar).
    pub total: Duration,
}

impl Default for PrazosLogin {
    fn default() -> Self {
        Self {
            aguardar: Duration::from_secs(25),
            retirada: Duration::from_secs(10),
            total: Duration::from_secs(60),
        }
    }
}

/// De quanto em quanto o agente que espera olha se a CLI caiu ou o cofre travou.
const PASSO: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fase {
    /// Esperando uma extensão pegar.
    Esperando,
    /// Uma extensão pegou; ainda não preencheu.
    Entregue,
    /// O preenchimento saiu; falta o desfecho.
    Preenchido,
}

struct Trabalho {
    /// Id aleatório do pedido (comparado em tempo constante).
    id: String,
    entrada: String,
    fase: Fase,
    resultado: Option<Result<(), ErroResposta>>,
}

/// Fila de um lugar dos pedidos de `ward login`.
pub struct Logins {
    atual: Mutex<Option<Trabalho>>,
    sinal: Condvar,
    prazos: PrazosLogin,
}

impl Default for Logins {
    fn default() -> Self {
        Self::novo(PrazosLogin::default())
    }
}

impl Logins {
    pub fn novo(prazos: PrazosLogin) -> Self {
        Self {
            atual: Mutex::new(None),
            sinal: Condvar::new(),
            prazos,
        }
    }

    fn trava(&self) -> MutexGuard<'_, Option<Trabalho>> {
        self.atual.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// O pedido atual, se o id é o dele.
    fn do_id<'a>(atual: &'a mut Option<Trabalho>, id: &str) -> Option<&'a mut Trabalho> {
        atual
            .as_mut()
            .filter(|t| gerador::tokens_iguais(t.id.as_bytes(), id.as_bytes()))
    }

    /// Termina o pedido `id` (se ainda é o atual e não terminou) e acorda quem espera.
    fn terminar(&self, id: &str, r: Result<(), ErroResposta>) -> bool {
        let mut g = self.trava();
        let Some(t) = Self::do_id(&mut g, id) else {
            return false;
        };
        if t.resultado.is_some() {
            return false;
        }
        t.resultado = Some(r);
        self.sinal.notify_all();
        true
    }
}

/// Apaga o pedido ao sair do `login` (inclusive por erro ou pânico).
struct Apagar<'a>(&'a Logins);

impl Drop for Apagar<'_> {
    fn drop(&mut self) {
        *self.0.trava() = None;
        self.0.sinal.notify_all();
    }
}

impl Daemon {
    /// `login <entrada>` do agente (Fase 43). Só `ok` ou erro voltam; nenhum valor.
    pub(crate) fn login(
        &self,
        cliente: &dyn Cliente,
        pedido: &Pedido,
    ) -> Result<Resposta, ErroResposta> {
        let Pedido::Login { entrada } = pedido else {
            return Err(ErroResposta::PedidoInvalido);
        };
        // Tipo sem login: recusa antes de pedir aprovação (como o `playwright`).
        if let Some(a) = self.estado().aberto.as_ref()
            && a.entradas
                .iter()
                .find(|e| e.nome() == entrada && e.nivel() != NivelAcesso::Bloqueado)
                .is_some_and(|e| !matches!(e.dados(), Dados::Login(_)))
        {
            return Err(ErroResposta::TipoSemEntrega);
        }
        self.autorizar_uso(cliente, pedido)?;
        self.registrar_uso();
        // A entrada pode ter mudado durante a aprovação: confere de novo.
        let nome = {
            let est = self.estado();
            let a = est.aberto.as_ref().ok_or(ErroResposta::Travado)?;
            let e = a
                .entradas
                .iter()
                .find(|e| e.nome() == entrada && e.nivel() != NivelAcesso::Bloqueado)
                .ok_or(ErroResposta::EntradaInexistente)?;
            if !matches!(e.dados(), Dados::Login(_)) {
                return Err(ErroResposta::TipoSemEntrega);
            }
            e.nome().to_owned()
        };
        let id = {
            let t = gerador::token_conectar().map_err(|_| ErroResposta::Interno)?;
            String::from_utf8(t.expor().to_vec()).map_err(|_| ErroResposta::Interno)?
        };
        let logins = self.logins();
        {
            let mut g = logins.trava();
            if g.is_some() {
                return Err(ErroResposta::Ocupado);
            }
            *g = Some(Trabalho {
                id: id.clone(),
                entrada: nome.clone(),
                fase: Fase::Esperando,
                resultado: None,
            });
            logins.sinal.notify_all();
        }
        let _apagar = Apagar(logins);
        aviso!("login {nome}: pedido à extensão");
        let r = self.esperar_desfecho(logins, &id, cliente);
        match &r {
            Ok(()) => aviso!("login {nome}: formulário enviado"),
            Err(e) => aviso!("login {nome}: {}", e.codigo()),
        }
        r.map(|()| Resposta::Ok)
    }

    fn esperar_desfecho(
        &self,
        logins: &Logins,
        id: &str,
        cliente: &dyn Cliente,
    ) -> Result<(), ErroResposta> {
        let inicio = Instant::now();
        let mut pegou: Option<Instant> = None;
        let mut g = logins.trava();
        loop {
            let Some(t) = Logins::do_id(&mut g, id) else {
                return Err(ErroResposta::Interno);
            };
            if let Some(r) = t.resultado.clone() {
                return r;
            }
            if t.fase != Fase::Esperando && pegou.is_none() {
                pegou = Some(Instant::now());
            }
            let vencido = match pegou {
                None => inicio.elapsed() >= logins.prazos.retirada,
                Some(p) => p.elapsed() >= logins.prazos.total,
            };
            if vencido {
                // Ninguém mais preenche com este id.
                t.resultado = Some(Err(ErroResposta::NavegadorFalhou));
                return Err(ErroResposta::NavegadorFalhou);
            }
            drop(g);
            // Fora da trava dos pedidos: o estado do cofre tem a trava dele.
            if cliente.saiu() {
                logins.terminar(id, Err(ErroResposta::Negado));
                return Err(ErroResposta::Negado);
            }
            if self.travado() {
                logins.terminar(id, Err(ErroResposta::Travado));
                return Err(ErroResposta::Travado);
            }
            g = logins.trava();
            if Logins::do_id(&mut g, id).is_some_and(|t| t.resultado.is_none()) {
                g = logins
                    .sinal
                    .wait_timeout(g, PASSO)
                    .unwrap_or_else(|e| e.into_inner())
                    .0;
            }
        }
    }

    /// `aguardar_login` (só extensão): o próximo pedido ainda não entregue, ou `ok`
    /// depois de [`PrazosLogin::aguardar`].
    pub(crate) fn aguardar_login(&self, papel: Papel) -> Result<Resposta, ErroResposta> {
        if papel != Papel::Extensao {
            return Err(ErroResposta::NaoAutorizado);
        }
        let logins = self.logins();
        let fim = Instant::now() + logins.prazos.aguardar;
        let mut g = logins.trava();
        loop {
            if let Some(t) = g.as_mut()
                && t.fase == Fase::Esperando
                && t.resultado.is_none()
            {
                t.fase = Fase::Entregue;
                logins.sinal.notify_all();
                return Ok(Resposta::PedidoDeLogin {
                    id: t.id.clone(),
                    entrada: t.entrada.clone(),
                });
            }
            let agora = Instant::now();
            if agora >= fim {
                return Ok(Resposta::Ok);
            }
            g = logins
                .sinal
                .wait_timeout(g, fim - agora)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }

    /// `preencher_login` (só extensão): os valores do pedido `id`, uma vez, se a origem
    /// da aba for a da entrada (D9, pelo `preencher` da Fase 40).
    pub(crate) fn preencher_login(
        &self,
        papel: Papel,
        id: &str,
        origem: &str,
    ) -> Result<Resposta, ErroResposta> {
        if papel != Papel::Extensao {
            return Err(ErroResposta::NaoAutorizado);
        }
        let logins = self.logins();
        let entrada = {
            let mut g = logins.trava();
            let t = Logins::do_id(&mut g, id).ok_or(ErroResposta::Expirado)?;
            if t.fase != Fase::Entregue || t.resultado.is_some() {
                return Err(ErroResposta::Expirado);
            }
            // Um id preenche uma vez só, dê certo ou não.
            t.fase = Fase::Preenchido;
            t.entrada.clone()
        };
        match self.preencher(papel, &entrada, origem) {
            Ok(r) => {
                // O pedido pode ter morrido (prazo, trava, CLI) enquanto o valor saía do
                // cofre: aí nada sai.
                let mut g = logins.trava();
                match Logins::do_id(&mut g, id) {
                    Some(t) if t.resultado.is_none() => Ok(r),
                    _ => Err(ErroResposta::Expirado),
                }
            }
            Err(e) => {
                logins.terminar(id, Err(e.clone()));
                Err(e)
            }
        }
    }

    /// `resultado_login` (só extensão): o desfecho do pedido `id`.
    pub(crate) fn resultado_login(
        &self,
        papel: Papel,
        id: &str,
        resultado: ResultadoLogin,
    ) -> Result<Resposta, ErroResposta> {
        if papel != Papel::Extensao {
            return Err(ErroResposta::NaoAutorizado);
        }
        let logins = self.logins();
        {
            let mut g = logins.trava();
            let t = Logins::do_id(&mut g, id).ok_or(ErroResposta::Expirado)?;
            if t.resultado.is_some() || t.fase == Fase::Esperando {
                return Err(ErroResposta::Expirado);
            }
            // "Enviado" e "recusado" só depois do preenchimento.
            if matches!(
                resultado,
                ResultadoLogin::Enviado | ResultadoLogin::Recusado
            ) && t.fase != Fase::Preenchido
            {
                return Err(ErroResposta::PedidoInvalido);
            }
        }
        let r = match resultado {
            ResultadoLogin::Enviado => Ok(()),
            ResultadoLogin::Recusado | ResultadoLogin::SemFormulario => {
                Err(ErroResposta::LoginRecusado)
            }
            ResultadoLogin::AbaMudou => Err(ErroResposta::OrigemDiferente),
            ResultadoLogin::Falha => Err(ErroResposta::NavegadorFalhou),
        };
        if logins.terminar(id, r) {
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
    fn id_de_outro_pedido_nao_casa() {
        let l = Logins::default();
        *l.trava() = Some(Trabalho {
            id: "a".repeat(32),
            entrada: "acme-wp-dev".into(),
            fase: Fase::Entregue,
            resultado: None,
        });
        let mut g = l.trava();
        assert!(Logins::do_id(&mut g, &"a".repeat(32)).is_some());
        assert!(Logins::do_id(&mut g, &"b".repeat(32)).is_none());
        assert!(Logins::do_id(&mut g, &"a".repeat(31)).is_none());
        assert!(Logins::do_id(&mut g, "").is_none());
    }
}
