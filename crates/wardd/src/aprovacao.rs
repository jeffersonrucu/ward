//! Motor de aprovação (Fase 23; S9, S10; D2, D8, D16, D19, D21, D57).
//!
//! - Um uso "com aprovação" vira um pedido pendente com id. O pedido vai ao app que
//!   assinou ([`Motor::assinar`]: a conexão do papel app, D65 a); sem app
//!   assinado, o motor abre o app pelo lançador ([`AbridorApp`]). Sem como abrir
//!   (sessão Wayland fora de D57, binário não confiável, sem instalação) o pedido é
//!   negado na hora com "aprovação indisponível" (D21).
//! - O pedido espera a resposta do app até [`PRAZO_APROVACAO`] (2 min, S10) pelo
//!   relógio injetado; passou, "expirado". A conexão do app que caiu com o pedido
//!   entregue nega ("fechar nega"); o app que saiu sem ter recebido o pedido dá
//!   "aprovação indisponível". O cliente (CLI) que caiu enquanto espera cancela.
//! - A resposta vem só pelo papel app (a tabela do protocolo, V3.15). O PIN, o efeito de
//!   cada botão e a liberação de 1 hora ficam no [`crate::nucleo`]; aqui só se decide o
//!   pedido ([`Motor::decidir`]).
//! - Cofre travado num uso do agente: o motor pede ao app a tela de destravar e espera
//!   até [`PRAZO_DESTRAVE`] (D16), contado em separado da aprovação. O aviso leva a
//!   entrada, a operação e o comando já com escape visível (D51) e os segundos que
//!   faltam pelo relógio do motor (Fase 39a). Com mais de um uso esperando, o app recebe
//!   o mais novo (um aviso por espera nova; ao assinar, só o mais novo).
//!
//! Não há aprovador no binário: os testes (D8) assinam um [`Assinante`] em processo e
//! respondem por [`crate::nucleo::Daemon::atender`] com o papel app, o mesmo caminho do
//! socket. Nenhuma feature, variável ou argumento muda isso.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use ward_core::protocol::{
    MAX_RESTANTE_DESTRAVE, PedidoDeAprovacao, PedidoDeDestrave, ResponseError, Resposta,
};

use crate::{debug, warning};

/// Prazo da resposta da janela de aprovação (S10).
pub const PRAZO_APROVACAO: Duration = Duration::from_secs(120);
/// Prazo da espera pelo destrave (D16).
pub const PRAZO_DESTRAVE: Duration = Duration::from_secs(120);
/// "Permitir por 1 hora" (D2).
pub const UMA_HORA: Duration = Duration::from_secs(3600);
/// De quanto em quanto tempo (real) a espera olha o relógio, o cliente e o cofre.
const PASSO: Duration = Duration::from_millis(25);

/// Relógio do motor (monotônico; injetável nos testes).
pub trait RelogioMotor: Send + Sync {
    /// Tempo desde um ponto fixo qualquer.
    fn agora(&self) -> Duration;
}

/// Relógio monotônico do sistema.
pub struct RelogioMonotonico(Instant);

impl RelogioMonotonico {
    pub fn novo() -> Self {
        Self(Instant::now())
    }
}

impl Default for RelogioMonotonico {
    fn default() -> Self {
        Self::novo()
    }
}

impl RelogioMotor for RelogioMonotonico {
    fn agora(&self) -> Duration {
        self.0.elapsed()
    }
}

/// Por que o app não foi aberto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroAbrirApp {
    /// Já há uma instância (uma por vez, D57); ela vai assinar.
    JaAberto,
    /// Não há como abrir (D21).
    Indisponivel,
}

/// Como o motor abre o app (o lançador de D57 no daemon; um falso nos testes).
pub trait AbridorApp: Send + Sync {
    /// Abre o app. `ao_sair` é chamado uma vez, quando o app aberto por esta chamada
    /// sai.
    fn abrir(&self, ao_sair: Box<dyn FnOnce() + Send>) -> Result<(), ErroAbrirApp>;
}

/// Sem app: tudo que precisa da janela é "aprovação indisponível" (D21).
pub struct SemApp;

impl AbridorApp for SemApp {
    fn abrir(&self, _ao_sair: Box<dyn FnOnce() + Send>) -> Result<(), ErroAbrirApp> {
        Err(ErroAbrirApp::Indisponivel)
    }
}

/// Quem recebe os avisos do motor: a conexão do papel app (D65 a).
pub trait Assinante: Send {
    /// Entrega `pedido_de_aprovacao` ou `pedido_de_destrave`. `false`: não deu (a
    /// conexão caiu); o motor trata como app fechado.
    fn entregar(&mut self, r: &Resposta) -> bool;
}

/// O cliente que fez o pedido do agente (a conexão da CLI).
pub trait Cliente {
    /// Cadeia de processos para exibir (`claude → bash → ward`), do mais antigo ao mais
    /// novo.
    fn cadeia(&self) -> Vec<String>;
    /// A conexão caiu? (a espera é cancelada.)
    fn saiu(&self) -> bool;
}

/// Cliente sem conexão (chamada em processo): sem cadeia, nunca sai.
pub struct SemCliente;

impl Cliente for SemCliente {
    fn cadeia(&self) -> Vec<String> {
        Vec::new()
    }
    fn saiu(&self) -> bool {
        false
    }
}

struct Pendente {
    pedido: PedidoDeAprovacao,
    /// Id do assinante que recebeu.
    entregue_a: Option<u64>,
    resultado: Option<Result<(), ResponseError>>,
}

/// Um uso do agente esperando o destrave: o aviso (sem `restante`, que é calculado na
/// entrega) e o prazo pelo relógio do motor.
struct EsperaDestrave {
    id: u64,
    prazo: Duration,
    aviso: PedidoDeDestrave,
}

impl EsperaDestrave {
    /// O aviso com os segundos que faltam em `agora`, arredondados para cima, em
    /// 1..=[`MAX_RESTANTE_DESTRAVE`] (o formato do protocolo).
    fn aviso_em(&self, agora: Duration) -> Resposta {
        let falta = self.prazo.saturating_sub(agora);
        let s = falta.as_secs() + u64::from(falta.subsec_nanos() > 0);
        let mut a = self.aviso.clone();
        a.restante = s.clamp(1, MAX_RESTANTE_DESTRAVE);
        Resposta::PedidoDeDestrave(a)
    }
}

struct Estado {
    proximo_id: u64,
    pendentes: Vec<Pendente>,
    assinante: Option<(u64, Box<dyn Assinante>)>,
    /// Pedidos do agente esperando o destrave agora, do mais antigo ao mais novo.
    esperando_destrave: Vec<EsperaDestrave>,
}

impl Estado {
    fn pendente(&mut self, id: u64) -> Option<&mut Pendente> {
        self.pendentes.iter_mut().find(|p| p.pedido.id == id)
    }

    fn tirar(&mut self, id: u64) -> Option<Pendente> {
        let i = self.pendentes.iter().position(|p| p.pedido.id == id)?;
        Some(self.pendentes.remove(i))
    }

    /// Entrega ao assinante tudo que ainda não foi entregue (e o destrave mais novo, com
    /// os segundos que faltam em `agora`, se `destrave`). Falha na entrega: o assinante
    /// sai.
    fn entregar_pendentes(&mut self, destrave: Option<Duration>) {
        let Some((aid, a)) = self.assinante.as_mut() else {
            return;
        };
        let aid = *aid;
        let mut ok = true;
        if let (Some(agora), Some(d)) = (destrave, self.esperando_destrave.last()) {
            ok = a.entregar(&d.aviso_em(agora));
        }
        for p in self.pendentes.iter_mut() {
            if !ok {
                break;
            }
            if p.entregue_a.is_none() && p.resultado.is_none() {
                ok = a.entregar(&Resposta::PedidoDeAprovacao(p.pedido.clone()));
                if ok {
                    p.entregue_a = Some(aid);
                }
            }
        }
        if !ok {
            warning!("entrega ao app falhou: a janela conta como fechada");
            self.sair_assinante(aid);
        }
    }

    /// O assinante `aid` saiu: o que ele recebeu e não respondeu é negado (fechar nega).
    fn sair_assinante(&mut self, aid: u64) {
        if self.assinante.as_ref().is_some_and(|(id, _)| *id == aid) {
            self.assinante = None;
        }
        for p in self.pendentes.iter_mut() {
            if p.entregue_a == Some(aid) && p.resultado.is_none() {
                p.resultado = Some(Err(ResponseError::Denied));
            }
        }
    }
}

struct Interno {
    estado: Mutex<Estado>,
    mudou: Condvar,
}

impl Interno {
    fn estado(&self) -> MutexGuard<'_, Estado> {
        self.estado.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// O app aberto saiu: pedido que ninguém recebeu fica sem janela (D21).
    fn app_saiu(&self) {
        let mut g = self.estado();
        if g.assinante.is_none() {
            for p in g.pendentes.iter_mut() {
                if p.entregue_a.is_none() && p.resultado.is_none() {
                    p.resultado = Some(Err(ResponseError::ApprovalUnavailable));
                }
            }
        }
        drop(g);
        self.mudou.notify_all();
    }
}

/// O motor de aprovação.
pub struct Motor {
    relogio: Arc<dyn RelogioMotor>,
    abridor: Box<dyn AbridorApp>,
    interno: Arc<Interno>,
    /// Cofre travado e sem app para mostrar a tela: espera os 2 min mesmo assim (o
    /// usuário pode destravar pela CLI, V4.21). Desligado, responde `travado` na hora.
    esperar_sem_app: bool,
}

impl Motor {
    pub fn novo(relogio: Arc<dyn RelogioMotor>, abridor: Box<dyn AbridorApp>) -> Self {
        Self {
            relogio,
            abridor,
            interno: Arc::new(Interno {
                estado: Mutex::new(Estado {
                    proximo_id: 1,
                    pendentes: Vec::new(),
                    assinante: None,
                    esperando_destrave: Vec::new(),
                }),
                mudou: Condvar::new(),
            }),
            esperar_sem_app: false,
        }
    }

    /// Com o cofre travado e sem app, o pedido do agente espera o destrave pela CLI
    /// (`ward destravar` em outro terminal) até [`PRAZO_DESTRAVE`] (D16, V4.21), em vez
    /// de responder `travado` na hora. O `wardd` liga; os testes do motor, não.
    pub fn esperando_sem_app(mut self) -> Self {
        self.esperar_sem_app = true;
        self
    }

    /// Motor sem app (D21), com o relógio do sistema.
    pub fn sem_app() -> Self {
        Self::novo(Arc::new(RelogioMonotonico::novo()), Box::new(SemApp))
    }

    /// Agora, pelo relógio do motor (liberação de 1 hora).
    pub fn agora(&self) -> Duration {
        self.relogio.agora()
    }

    /// Abre o app pelo lançador. Já aberto também é sucesso.
    fn abrir(&self) -> Result<(), ErroAbrirApp> {
        let interno = Arc::downgrade(&self.interno);
        let r = self.abridor.abrir(Box::new(move || {
            if let Some(i) = interno.upgrade() {
                i.app_saiu();
            }
        }));
        match r {
            Ok(()) | Err(ErroAbrirApp::JaAberto) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Operação `abrir_app` (D57): de qualquer papel, sem campo, sem segredo.
    pub fn abrir_app(&self) -> Result<(), ResponseError> {
        self.abrir().map_err(|_| ResponseError::ApprovalUnavailable)
    }

    /// A conexão `id` do papel app passa a receber os avisos. Uma por
    /// vez: a nova toma o lugar da anterior, e o que esta tinha recebido sem responder
    /// vai de novo à nova.
    pub fn assinar(&self, id: u64, a: Box<dyn Assinante>) {
        let mut g = self.interno.estado();
        if let Some((velho, _)) = g.assinante.take() {
            for p in g.pendentes.iter_mut() {
                if p.entregue_a == Some(velho) {
                    p.entregue_a = None;
                }
            }
        }
        g.assinante = Some((id, a));
        g.entregar_pendentes(Some(self.relogio.agora()));
        drop(g);
        self.interno.mudou.notify_all();
    }

    /// A conexão `id` do app fechou.
    pub fn desassinar(&self, id: u64) {
        let mut g = self.interno.estado();
        g.sair_assinante(id);
        drop(g);
        self.interno.mudou.notify_all();
    }

    /// Há app assinado?
    pub fn tem_assinante(&self) -> bool {
        self.interno.estado().assinante.is_some()
    }

    /// Pede a aprovação e espera (S10). `Ok`: permitido (por um dos três botões, com o
    /// PIN conferido pelo núcleo). O `id` do pedido é do motor.
    pub fn pedir_aprovacao(
        &self,
        mut pedido: PedidoDeAprovacao,
        cliente: &dyn Cliente,
    ) -> Result<(), ResponseError> {
        let mut g = self.interno.estado();
        let id = g.proximo_id;
        g.proximo_id = g.proximo_id.saturating_add(1);
        pedido.id = id;
        let prazo = self.relogio.agora().saturating_add(PRAZO_APROVACAO);
        g.pendentes.push(Pendente {
            pedido,
            entregue_a: None,
            resultado: None,
        });
        if g.assinante.is_some() {
            g.entregar_pendentes(None);
        } else {
            drop(g);
            if self.abrir().is_err() {
                warning!("aprovação {id}: app indisponível; negado (D21)");
                self.interno.estado().tirar(id);
                return Err(ResponseError::ApprovalUnavailable);
            }
            g = self.interno.estado();
        }
        debug!("aprovação {id}: pendente");
        loop {
            if let Some(r) = g.pendente(id).and_then(|p| p.resultado.take()) {
                g.tirar(id);
                match &r {
                    Ok(()) => debug!("aprovação {id}: permitida"),
                    Err(e) => warning!("aprovação {id}: {}", e.code()),
                }
                return r;
            }
            if self.relogio.agora() >= prazo {
                g.tirar(id);
                warning!("aprovação {id}: expirou em 2 min (S10)");
                return Err(ResponseError::Expired);
            }
            if cliente.saiu() {
                g.tirar(id);
                warning!("aprovação {id}: o cliente saiu; cancelada");
                return Err(ResponseError::Denied);
            }
            g = self
                .interno
                .mudou
                .wait_timeout(g, PASSO)
                .map(|(g, _)| g)
                .unwrap_or_else(|e| e.into_inner().0);
        }
    }

    /// Nega todo pedido de aprovação pendente e ainda sem decisão (Fase 49a: o
    /// restaurar troca o cofre, e uma resposta atrasada valeria para a versão nova).
    /// Quem pediu recebe "negado"; a janela que ainda mostra o pedido recebe "expirado"
    /// ao responder. Devolve quantos foram negados.
    pub fn cancelar_pendentes(&self) -> usize {
        let mut g = self.interno.estado();
        let mut n = 0;
        for p in g.pendentes.iter_mut() {
            if p.resultado.is_none() {
                p.resultado = Some(Err(ResponseError::Denied));
                n += 1;
            }
        }
        drop(g);
        self.interno.mudou.notify_all();
        n
    }

    /// As entradas do pedido `id`, se ele está pendente e ainda sem decisão.
    pub fn pendente(&self, id: u64) -> Option<Vec<String>> {
        let mut g = self.interno.estado();
        g.pendente(id)
            .filter(|p| p.resultado.is_none())
            .map(|p| p.pedido.entradas.clone())
    }

    /// O pedido `id`, pendente e sem decisão, é só uma confirmação de origem (Fase 44c)?
    pub fn e_confirmacao(&self, id: u64) -> Option<bool> {
        let mut g = self.interno.estado();
        g.pendente(id)
            .filter(|p| p.resultado.is_none())
            .map(|p| p.pedido.confirmacao)
    }

    /// O pedido `id`, pendente e sem decisão, tem entrada padrão que a janela não mostra
    /// (Fase 54b, D68)?
    pub fn e_entrada_oculta(&self, id: u64) -> Option<bool> {
        let mut g = self.interno.estado();
        g.pendente(id)
            .filter(|p| p.resultado.is_none())
            .map(|p| p.pedido.entrada_oculta)
    }

    /// Decide o pedido `id`. `false`: não está mais pendente (expirou, foi cancelado ou
    /// já decidido).
    pub fn decidir(&self, id: u64, r: Result<(), ResponseError>) -> bool {
        let mut g = self.interno.estado();
        let Some(p) = g.pendente(id).filter(|p| p.resultado.is_none()) else {
            return false;
        };
        p.resultado = Some(r);
        drop(g);
        self.interno.mudou.notify_all();
        true
    }

    /// Cofre travado num uso do agente (D16): pede ao app a tela de destravar e espera
    /// até 2 min. `aviso` é o que a tela mostra (entrada, operação e comando já com
    /// escape visível, D51; o `restante` é do motor). `travado` é consultado sem nenhuma
    /// trava do motor.
    pub fn esperar_destrave(
        &self,
        aviso: PedidoDeDestrave,
        travado: &dyn Fn() -> bool,
        cliente: &dyn Cliente,
    ) -> Result<(), ResponseError> {
        let agora = self.relogio.agora();
        let prazo = agora.saturating_add(PRAZO_DESTRAVE);
        let espera;
        {
            let mut g = self.interno.estado();
            espera = g.proximo_id;
            g.proximo_id = g.proximo_id.saturating_add(1);
            let d = EsperaDestrave {
                id: espera,
                prazo,
                aviso,
            };
            let resposta = d.aviso_em(agora);
            g.esperando_destrave.push(d);
            if g.assinante.is_some() {
                // Só o pedido de destrave (os de aprovação já foram).
                let ok = g
                    .assinante
                    .as_mut()
                    .is_some_and(|(_, a)| a.entregar(&resposta));
                if !ok && let Some((aid, _)) = g.assinante.as_ref() {
                    let aid = *aid;
                    g.sair_assinante(aid);
                }
            } else {
                drop(g);
                if self.abrir().is_err() {
                    if !self.esperar_sem_app {
                        // Ninguém mostraria a tela; quem pediu fica sabendo já.
                        self.sair_destrave(espera);
                        warning!("pedido com o cofre travado: app indisponível");
                        return Err(ResponseError::Locked);
                    }
                    // V4.21: o usuário pode destravar pela CLI; a CLI avisa quem pediu.
                    warning!("pedido com o cofre travado: app indisponível; esperando a CLI");
                }
            }
        }
        warning!("pedido com o cofre travado: esperando o destrave (até 2 min, D16)");
        let r = loop {
            if !travado() {
                break Ok(());
            }
            if self.relogio.agora() >= prazo {
                warning!("pedido com o cofre travado: tempo esgotado");
                break Err(ResponseError::LockedTimedOut);
            }
            if cliente.saiu() {
                break Err(ResponseError::Locked);
            }
            std::thread::sleep(PASSO);
        };
        self.sair_destrave(espera);
        r
    }

    fn sair_destrave(&self, id: u64) {
        let mut g = self.interno.estado();
        g.esperando_destrave.retain(|d| d.id != id);
    }
}
