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
//!
//! Fase 44c (D9): no `login` e no `playwright` de uma entrada cuja URL é `http`
//! (`localhost`/`127.0.0.1`), e desde a Fase 44e de toda entrada cujo host é de
//! loopback em qualquer esquema (`https://localhost`, `https://[::1]`…), o pedido sempre vai à janela com a origem à vista, mesmo
//! no nível livre ou coberto pela liberação de 1 hora: o Ward não sabe que processo
//! responde nessa origem (outro processo do uid pode escutar em `[::1]` ou na porta de
//! um serviço parado). Nível livre: só uma confirmação, sem PIN ("Negar" ou
//! "Confirmar"). "Com aprovação": a janela de sempre, com o PIN e a origem. Sem app,
//! "aprovação indisponível". Em `https` de um host de fora nada muda.
//!
//! Fase 54b (D68): o `executar` e o `ssh` trazem o tipo da entrada padrão da CLI. Pipe,
//! arquivo ou outro (nem terminal, nem `/dev/null`): o comando recebe dados que a janela
//! não mostra. Numa entrada "com aprovação" o pedido sempre vai à janela, mesmo coberto
//! pela liberação de 1 hora, com o aviso (`entrada_oculta`), e os três "Permitir" só
//! valem com `entrada_confirmada` (a caixa marcada na janela); sem ela, recusado antes do
//! PIN. Nível livre: nada muda. O tipo é dito pelo cliente: `terminal` não se confere
//! (D68, RA11).

use ward_core::display;
use ward_core::entry::{AccessLevel, Data, Entry, Environment, Secret};
use ward_core::limits::{MAX_DISPLAYED_TEXT, MAX_TEXT};
use ward_core::protocol::{
    Decision, Operation, PedidoDeAprovacao, PedidoDeDestrave, Request, ResponseError, Resposta,
    StdinKind,
};

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
pub fn mais_restritivo(a: AccessLevel, b: AccessLevel) -> AccessLevel {
    let grau = |n| match n {
        AccessLevel::Blocked => 2,
        AccessLevel::Approval => 1,
        AccessLevel::Free => 0,
    };
    if grau(a) >= grau(b) { a } else { b }
}

/// O ambiente mais sensível de dois, para o destaque da janela.
fn mais_sensivel(a: Environment, b: Environment) -> Environment {
    let grau = |x| match x {
        Environment::Prod => 2,
        Environment::Stage => 1,
        Environment::Dev => 0,
    };
    if grau(a) >= grau(b) { a } else { b }
}

/// A outra entrada de um uso combinado (D19): túnel do `banco`, salto do `servidor`.
fn segunda_entrada(e: &Entry, op: Operation) -> Option<&str> {
    match (e.data(), op) {
        (Data::Database(b), Operation::Run) => b.tunnel.as_deref(),
        (Data::Server(s), Operation::Run | Operation::Ssh) => s.jump.as_deref(),
        _ => None,
    }
}

/// Fase 44c (D9): a origem local para onde o uso leva usuário e senha (só no `login`, no
/// `playwright` e no `preencher` do popup de uma entrada `login`), ou `None` (host de
/// fora, outro tipo, outra operação). Fase 44e (2ª auditoria do M6): todo host de
/// loopback conta, em qualquer esquema (`https://localhost`, `https://127.0.0.1`,
/// `https://[::1]`…): um certificado de uma CA local (mkcert, `~/.pki`) não prova quem
/// responde ([`ward_core::entry::local_url_origin`]).
pub(crate) fn origem_a_confirmar(e: &Entry, op: Operation) -> Option<String> {
    match (e.data(), op) {
        (Data::Login(l), Operation::Login | Operation::Playwright | Operation::Fill) => {
            ward_core::entry::local_url_origin(&l.url)
        }
        _ => None,
    }
}

/// O nível de uma entrada para este uso, com a liberação de 1 hora (D2). Com
/// `sem_liberacao` (Fase 54b: entrada padrão oculta), a liberação não conta.
fn nivel_efetivo(
    a: &Aberto,
    e: &Entry,
    agora: std::time::Duration,
    sem_liberacao: bool,
) -> AccessLevel {
    match e.level() {
        AccessLevel::Approval
            if !sem_liberacao && a.liberacoes.get(e.name()).is_some_and(|ate| agora < *ate) =>
        {
            AccessLevel::Free
        }
        n => n,
    }
}

/// O que o pedido do agente quer, para exibir.
struct Descricao<'a> {
    operacao: Operation,
    entrada: &'a str,
    /// O comando exato (D51).
    comando: String,
    /// Onde o comando roda: no `executar`, o diretório da CLI (Fase 26a), entre aspas
    /// e com escape visível; vazio nas outras operações (o `ssh` roda na pasta pessoal
    /// do passwd, fixa).
    diretorio: String,
    /// Fase 54b (D68): o comando recebe pela entrada padrão dados que a janela não
    /// mostra (só `executar` e `ssh`).
    entrada_oculta: bool,
}

/// O que o pedido do agente quer: operação, entrada, o comando exato e o diretório.
fn descrever(pedido: &Request) -> Option<Descricao<'_>> {
    let oculta = |t: &StdinKind| t.hidden();
    let (operacao, entrada, comando, diretorio, entrada_oculta) = match pedido {
        Request::Run(x) => (
            Operation::Run,
            x.entry.as_str(),
            display::display_args(&x.argv, MAX_DISPLAYED_TEXT),
            display::display_args(&[x.cwd.as_slice()], MAX_DISPLAYED_TEXT),
            oculta(&x.stdin_kind),
        ),
        Request::Ssh {
            entry: entrada,
            command: comando,
            stdin_kind: tipo_entrada,
            ..
        } => (
            Operation::Ssh,
            entrada.as_str(),
            display::display_args(comando, MAX_DISPLAYED_TEXT),
            String::new(),
            oculta(tipo_entrada),
        ),
        Request::Login { entry: entrada } => (
            Operation::Login,
            entrada.as_str(),
            String::new(),
            String::new(),
            false,
        ),
        Request::Playwright { entry: entrada } => (
            Operation::Playwright,
            entrada.as_str(),
            String::new(),
            String::new(),
            false,
        ),
        _ => return None,
    };
    Some(Descricao {
        operacao,
        entrada,
        comando,
        diretorio,
        entrada_oculta,
    })
}

/// Cadeia para exibir (D51): nomes com escape visível, no limite do protocolo.
pub(crate) fn cadeia_para_exibir(cliente: &dyn Cliente) -> Vec<String> {
    let mut c = cliente.cadeia();
    if c.len() > MAX_CADEIA {
        c.drain(..c.len() - MAX_CADEIA);
    }
    c.iter()
        .map(|n| display::display_text(n.as_bytes(), MAX_TEXT))
        .collect()
}

impl Daemon {
    /// O agente pode usar a entrada agora? Espera o destrave e a janela quando precisa.
    /// Bloqueada responde igual a inexistente (V3.17); no uso combinado, a outra
    /// entrada bloqueada (ou sumida) nega sem perguntar.
    pub fn autorizar_uso(
        &self,
        cliente: &dyn Cliente,
        pedido: &Request,
    ) -> Result<Autorizacao, ResponseError> {
        let Descricao {
            operacao: op,
            entrada: nome,
            comando,
            diretorio,
            entrada_oculta,
        } = descrever(pedido).ok_or(ResponseError::InvalidRequest)?;
        // Nome vazio nunca é de uma entrada (o `ward-core` exige nome não vazio): responde
        // como inexistente antes de qualquer espera. Fase 39a: assim o aviso de destrave
        // nunca sai fora do formato (entrada vazia), o que fecharia a conexão do app e
        // negaria o que ele tinha na tela.
        if nome.is_empty() {
            return Err(ResponseError::EntryNotFound);
        }
        if self.travado() {
            // Fase 39a (D51): a tela de destravar mostra o que o agente pediu, com escape
            // visível. Com o cofre travado não se sabe se a entrada existe, nem o tipo e o
            // ambiente; o nome vai como o agente mandou.
            let aviso = PedidoDeDestrave {
                entrada: display::display_text(nome.as_bytes(), MAX_TEXT),
                operacao: op,
                comando: comando.clone(),
                restante: 0,
            };
            self.aprovacao
                .esperar_destrave(aviso, &|| self.travado(), cliente)?;
        }
        let agora = self.aprovacao.agora();
        let (nivel, entradas, ambiente, origem) = {
            let est = self.estado();
            let a = est.aberto.as_ref().ok_or(ResponseError::Locked)?;
            let e = a
                .entradas
                .iter()
                .find(|e| e.name() == nome && e.level() != AccessLevel::Blocked)
                .ok_or(ResponseError::EntryNotFound)?;
            // Fase 54b: com a entrada padrão oculta, a liberação de 1 hora não cobre.
            let mut nivel = nivel_efetivo(a, e, agora, entrada_oculta);
            let origem = origem_a_confirmar(e, op);
            let mut entradas = vec![e.name().to_owned()];
            let mut ambiente = e.environment();
            if let Some(outra) = segunda_entrada(e, op) {
                match a.entradas.iter().find(|x| x.name() == outra) {
                    Some(x) => {
                        nivel = mais_restritivo(nivel, nivel_efetivo(a, x, agora, entrada_oculta));
                        ambiente = mais_sensivel(ambiente, x.environment());
                        entradas.push(x.name().to_owned());
                    }
                    None => nivel = AccessLevel::Blocked,
                }
            }
            (nivel, entradas, ambiente, origem)
        };
        let pedido_da_janela = |confirmacao: bool, origem: Option<String>| PedidoDeAprovacao {
            id: 0,
            entradas: entradas.clone(),
            ambiente,
            operacao: op,
            comando: comando.clone(),
            diretorio: diretorio.clone(),
            cadeia: cadeia_para_exibir(cliente),
            origem: origem.unwrap_or_default(),
            confirmacao,
            // Fase 54b: só no pedido com PIN (a confirmação de origem não tem entrada
            // padrão).
            entrada_oculta: entrada_oculta && !confirmacao,
        };
        match nivel {
            AccessLevel::Free => match origem {
                None => Ok(Autorizacao::SemPerguntar),
                // Fase 44c/44e: origem local, só a confirmação (sem PIN).
                Some(o) => {
                    aviso!("uso numa origem local: confirmação na janela (D9)");
                    self.aprovacao
                        .pedir_aprovacao(pedido_da_janela(true, Some(o)), cliente)?;
                    Ok(Autorizacao::Aprovado)
                }
            },
            AccessLevel::Blocked => {
                aviso!("uso combinado com entrada bloqueada: negado sem perguntar (D19)");
                Err(ResponseError::Denied)
            }
            AccessLevel::Approval => {
                if entrada_oculta {
                    aviso!(
                        "uso com a entrada padrão que a janela não mostra: aviso e confirmação na janela (D68)"
                    );
                }
                self.aprovacao
                    .pedir_aprovacao(pedido_da_janela(false, origem), cliente)?;
                Ok(Autorizacao::Aprovado)
            }
        }
    }

    /// Resposta da janela (só do papel app, pela tabela do protocolo). "Negar" não pede
    /// PIN; os três "Permitir" pedem (S10). PIN errado deixa o pedido pendente; o 5.º
    /// erro trava o cofre e nega (S9). Fase 54b (D68): num pedido com a entrada padrão
    /// oculta, um "Permitir" sem `entrada_confirmada` é recusado antes do PIN (o pedido
    /// segue pendente); `entrada_confirmada` num pedido sem o aviso também.
    pub(crate) fn responder_aprovacao(
        &self,
        id: u64,
        decisao: Decision,
        pin: Option<&Secret>,
        entrada_confirmada: bool,
    ) -> Result<Resposta, ResponseError> {
        let entradas = self.aprovacao.pendente(id).ok_or(ResponseError::Expired)?;
        let confirmacao = self
            .aprovacao
            .e_confirmacao(id)
            .ok_or(ResponseError::Expired)?;
        let entrada_oculta = self
            .aprovacao
            .e_entrada_oculta(id)
            .ok_or(ResponseError::Expired)?;
        if decisao == Decision::Deny {
            return if self.aprovacao.decidir(id, Err(ResponseError::Denied)) {
                Ok(Resposta::Ok)
            } else {
                Err(ResponseError::Expired)
            };
        }
        // Fase 44c: a confirmação de origem só aceita "Negar" e "Confirmar"; um pedido
        // que pede PIN nunca é decidido por "Confirmar" (sem PIN).
        if confirmacao != (decisao == Decision::Confirm) {
            aviso!("aprovação {id}: decisão que não cabe no pedido; recusada");
            return Err(ResponseError::InvalidRequest);
        }
        if decisao == Decision::Confirm {
            return if self.aprovacao.decidir(id, Ok(())) {
                aviso!("aprovação {id}: origem confirmada");
                Ok(Resposta::Ok)
            } else {
                Err(ResponseError::Expired)
            };
        }
        // Fase 54b (D68): a caixa da entrada padrão, antes do PIN (nada conta como erro
        // de PIN). O daemon confere; a janela não decide sozinha (S9).
        if entrada_oculta != entrada_confirmada {
            aviso!(
                "aprovação {id}: permitir sem a confirmação da entrada padrão (ou com ela fora do aviso); recusado (D68)"
            );
            return Err(ResponseError::InvalidRequest);
        }
        // O protocolo já exige o PIN em todo "Permitir"; aqui é a segunda camada.
        let pin = pin.ok_or(ResponseError::InvalidRequest)?;
        let mut est = self.estado();
        // Fase 49a: com o estado preso, o pedido ainda tem de estar pendente. Um
        // restaurar entre a leitura acima e aqui cancelou o pedido; sem esta conferência,
        // o "permitir sempre" mudaria a entrada de mesmo nome da versão restaurada.
        if self.aprovacao.pendente(id).as_ref() != Some(&entradas) {
            return Err(ResponseError::Expired);
        }
        if let Err(e) = Self::conferir_pin(&mut est, pin) {
            if est.aberto.is_none() {
                // O 5.º erro travou (ou o cofre travou no meio): o pedido é negado.
                self.aprovacao.decidir(id, Err(ResponseError::Denied));
            }
            return Err(e);
        }
        match decisao {
            Decision::Deny | Decision::AllowOnce | Decision::Confirm => {}
            Decision::AllowOneHour => {
                if !self.aprovacao.decidir(id, Ok(())) {
                    return Err(ResponseError::Expired);
                }
                let ate = self.aprovacao.agora().saturating_add(UMA_HORA);
                let a = est.aberto.as_mut().ok_or(ResponseError::Locked)?;
                for n in entradas {
                    a.liberacoes.insert(n, ate);
                }
                aviso!("aprovação {id}: permitida por 1 hora");
                return Ok(Resposta::Ok);
            }
            Decision::AllowAlways => {
                // As entradas do pedido que estão "com aprovação" viram "livre", no disco.
                let a = est.aberto.as_mut().ok_or(ResponseError::Locked)?;
                let mut mudadas = Vec::new();
                for e in a.entradas.iter_mut() {
                    if entradas.iter().any(|n| n == e.name()) && e.level() == AccessLevel::Approval
                    {
                        e.set_level(AccessLevel::Free);
                        mudadas.push(e.name().to_owned());
                    }
                }
                for n in &mudadas {
                    a.liberacoes.remove(n);
                }
                // Falhou ao gravar: o nível volta e o pedido continua pendente.
                self.gravar_ou_desfazer(&mut est, move |a| {
                    for e in a.entradas.iter_mut() {
                        if mudadas.iter().any(|n| n == e.name()) {
                            e.set_level(AccessLevel::Approval);
                        }
                    }
                })?;
                aviso!("aprovação {id}: permitida sempre (entrada agora livre)");
            }
        }
        if self.aprovacao.decidir(id, Ok(())) {
            Ok(Resposta::Ok)
        } else {
            Err(ResponseError::Expired)
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nivel_combinado_e_o_mais_restritivo() {
        use AccessLevel::*;
        for (a, b, r) in [
            (Free, Free, Free),
            (Free, Approval, Approval),
            (Approval, Free, Approval),
            (Free, Blocked, Blocked),
            (Blocked, Approval, Blocked),
            (Approval, Approval, Approval),
        ] {
            assert_eq!(mais_restritivo(a, b), r);
        }
    }
}
