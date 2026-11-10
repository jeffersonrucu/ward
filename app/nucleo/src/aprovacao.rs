//! Janela de aprovação (Fase 36; S9, S10, D51, D65 a).
//!
//! - [`Aprovacoes`]: os `pedido_de_aprovacao` que o daemon mandou e ainda esperam
//!   resposta, na ordem de chegada, cada um com o relógio de [`PRAZO_APROVACAO`] contado
//!   da chegada (o aviso não traz o prazo; o daemon conta o dele, S10, e nega sozinho).
//!   No máximo [`MAX_PENDENTES`]: o excedente é descartado e expira no daemon (falha
//!   fechada).
//! - [`vigiar`]: a thread que tira os `pedido_de_aprovacao` da fila de avisos da conexão
//!   (D65 a) e avisa quem abre a janela. Os `pedido_de_destrave` ficam na fila para a
//!   Fase 31.
//! - [`exibir`]: o que a janela recebe. O texto vem do agente (comando, cadeia de
//!   processos) e é **não confiável**: passa por `ward_core::exibicao::neutralizar`
//!   (D51) aqui, no lado Rust, antes de ir à webview; a tela o põe como texto (nunca
//!   como HTML). Controle, bidi, largura zero e tags viram escape visível; nada é
//!   tirado nem reordenado. A seta `→` que separa a cadeia na tela é reservada: a que
//!   vier dentro do nome de um processo vira `\u{2192}`, para um processo não se passar
//!   por dois.
//! - [`ResultadoAprovacao`]: o que a resposta virou. O PIN não volta em nada daqui.
//!
//! Só a janela `aprovacao` responde (capability própria no Tauri; o comando confere o
//! rótulo da janela de novo). O papel agente nunca responde: o daemon só aceita
//! `responder_aprovacao` do papel app (V3.15, V5.20).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::Serialize;
use ward_core::exibicao::neutralizar;
use ward_core::protocolo::{Decisao, PedidoDeAprovacao};

use crate::ErroApp;
use crate::daemon::{FilaAvisos, MAX_AVISOS};

/// Quanto o pedido espera resposta (S10; o mesmo prazo do `wardd`).
pub const PRAZO_APROVACAO: Duration = Duration::from_secs(120);

/// A partir de quantos segundos restantes a janela passa a `Aprovação / Expirando`.
pub const SEGUNDOS_URGENTE: u64 = 20;

/// Pedidos guardados ao mesmo tempo (o mesmo limite da fila de avisos, D65 a).
pub const MAX_PENDENTES: usize = MAX_AVISOS;

/// Separador da cadeia na tela; dentro de um nome de processo ele vira escape.
pub const SEPARADOR_CADEIA: char = '\u{2192}';

/// A decisão pelo nome que vem da interface (não confiável).
pub fn decisao_de_nome(nome: &str) -> Result<Decisao, ErroApp> {
    match nome {
        "negar" => Ok(Decisao::Negar),
        "uma_vez" => Ok(Decisao::PermitirUmaVez),
        "uma_hora" => Ok(Decisao::PermitirUmaHora),
        "sempre" => Ok(Decisao::PermitirSempre),
        // Fase 44c: só a confirmação da origem `http` (sem PIN).
        "confirmar" => Ok(Decisao::Confirmar),
        _ => Err(ErroApp::PedidoInvalido),
    }
}

/// Uma entrada do pedido, para a tela. `tipo` é o do detalhe (só o identificador); sem
/// ele a tela mostra o ícone genérico.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EntradaExibida {
    pub nome: String,
    pub tipo: Option<&'static str>,
}

/// O pedido como a janela o mostra. Todo texto que veio do agente já está neutralizado
/// (D51). Sem segredo: o pedido do protocolo não tem nenhum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PedidoExibido {
    pub id: u64,
    /// A entrada e, no uso combinado (D19), a do salto ou do túnel.
    pub entradas: Vec<EntradaExibida>,
    pub ambiente: &'static str,
    pub operacao: &'static str,
    /// O comando exato, cada argumento entre aspas (D51).
    pub comando: String,
    /// Onde o comando roda (`executar`, Fase 26a), entre aspas (D51); vazio nas outras
    /// operações.
    pub diretorio: String,
    /// Quem pediu, do mais distante ao mais próximo.
    pub cadeia: Vec<String>,
    /// Fase 44c (D9): a origem `http` para onde vão usuário e senha, com escape visível
    /// (D51); vazia quando não há.
    pub origem: String,
    /// Fase 44c: só a confirmação da origem ("Negar" e "Confirmar", sem PIN).
    pub confirmacao: bool,
    /// Fase 54b (D68): o comando recebe pela entrada padrão (pipe, arquivo…) dados que
    /// a janela não mostra: aviso em `Destaque` e os "Permitir" só com a caixa marcada.
    pub entrada_oculta: bool,
    /// Segundos até expirar (0 quando `estado` é `bloqueado`).
    pub restante: u64,
    /// `pendente` ou `bloqueado` (o 5º PIN errado travou o cofre e o daemon negou; a
    /// janela só mostra o que aconteceu até o usuário fechar).
    pub estado: &'static str,
}

/// O que a resposta da janela virou.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ResultadoAprovacao {
    /// `respondido`, `pin-incorreto` (o pedido continua pendente), `bloqueado` (o 5º
    /// erro travou o cofre e o pedido foi negado) ou `expirado` (o daemon já não o tem).
    pub resultado: &'static str,
    /// Ainda há pedido esperando (a janela continua aberta).
    pub ha_outro: bool,
}

struct Item {
    pedido: PedidoDeAprovacao,
    chegada: Instant,
    bloqueado: bool,
    /// Tipos das entradas, buscados uma vez no detalhe.
    tipos: Option<Vec<Option<&'static str>>>,
}

/// O pedido da vez, já tirado do lock.
pub(crate) struct Atual {
    pub pedido: PedidoDeAprovacao,
    pub restante: u64,
    pub bloqueado: bool,
    pub tipos: Option<Vec<Option<&'static str>>>,
}

/// Os pedidos que esperam a janela.
#[derive(Default)]
pub struct Aprovacoes {
    itens: Mutex<VecDeque<Item>>,
}

impl Aprovacoes {
    fn travar(&self) -> MutexGuard<'_, VecDeque<Item>> {
        self.itens.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Chegou um `pedido_de_aprovacao` agora.
    pub fn chegou(&self, p: PedidoDeAprovacao) {
        self.chegou_em(p, Instant::now());
    }

    /// Só nos testes e na ponte de teste: um pedido que chegou há `decorrido` (para a
    /// `Aprovação / Expirando` sem esperar 2 min).
    #[cfg(any(test, feature = "modo-teste"))]
    pub fn chegou_ha(&self, p: PedidoDeAprovacao, decorrido: Duration) {
        let agora = Instant::now();
        self.chegou_em(p, agora.checked_sub(decorrido).unwrap_or(agora));
    }

    fn chegou_em(&self, p: PedidoDeAprovacao, chegada: Instant) {
        let mut f = self.travar();
        if f.iter().any(|i| i.pedido.id == p.id) {
            return;
        }
        if f.len() >= MAX_PENDENTES {
            drop(f);
            eprintln!("ward-app: pedido de aprovação descartado (muitos esperando)");
            return;
        }
        f.push_back(Item {
            pedido: p,
            chegada,
            bloqueado: false,
            tipos: None,
        });
    }

    /// Tira os pendentes que passaram do prazo (o daemon já os negou).
    fn limpar_expirados(f: &mut VecDeque<Item>, agora: Instant) {
        f.retain(|i| i.bloqueado || restante(i.chegada, agora) > 0);
    }

    /// O pedido da vez (o mais antigo ainda no prazo, ou o bloqueado que a janela mostra).
    pub(crate) fn atual(&self) -> Option<Atual> {
        self.atual_em(Instant::now())
    }

    fn atual_em(&self, agora: Instant) -> Option<Atual> {
        let mut f = self.travar();
        Self::limpar_expirados(&mut f, agora);
        f.front().map(|i| Atual {
            pedido: i.pedido.clone(),
            restante: if i.bloqueado {
                0
            } else {
                restante(i.chegada, agora)
            },
            bloqueado: i.bloqueado,
            tipos: i.tipos.clone(),
        })
    }

    /// Guarda os tipos das entradas do pedido `id`.
    pub(crate) fn guardar_tipos(&self, id: u64, tipos: Vec<Option<&'static str>>) {
        if let Some(i) = self.travar().iter_mut().find(|i| i.pedido.id == id) {
            i.tipos = Some(tipos);
        }
    }

    /// O pedido `id` é só a confirmação da origem (Fase 44c)? `None` se não está aqui.
    pub(crate) fn confirmacao(&self, id: u64) -> Option<bool> {
        self.travar()
            .iter()
            .find(|i| i.pedido.id == id)
            .map(|i| i.pedido.confirmacao)
    }

    /// O pedido `id` tem a entrada padrão oculta (Fase 54b)? `None` se não está aqui.
    pub(crate) fn entrada_oculta(&self, id: u64) -> Option<bool> {
        self.travar()
            .iter()
            .find(|i| i.pedido.id == id)
            .map(|i| i.pedido.entrada_oculta)
    }

    /// Estado do pedido `id` aqui: `Some(false)` pendente, `Some(true)` bloqueado,
    /// `None` desconhecido ou expirado.
    pub(crate) fn situacao(&self, id: u64) -> Option<bool> {
        let mut f = self.travar();
        Self::limpar_expirados(&mut f, Instant::now());
        f.iter().find(|i| i.pedido.id == id).map(|i| i.bloqueado)
    }

    pub(crate) fn remover(&self, id: u64) {
        self.travar().retain(|i| i.pedido.id != id);
    }

    pub(crate) fn marcar_bloqueado(&self, id: u64) {
        if let Some(i) = self.travar().iter_mut().find(|i| i.pedido.id == id) {
            i.bloqueado = true;
        }
    }

    /// Ids dos pendentes no prazo (não os bloqueados).
    pub(crate) fn pendentes(&self) -> Vec<u64> {
        let mut f = self.travar();
        Self::limpar_expirados(&mut f, Instant::now());
        f.iter()
            .filter(|i| !i.bloqueado)
            .map(|i| i.pedido.id)
            .collect()
    }

    /// Há algo para a janela mostrar.
    pub fn ha_pedido(&self) -> bool {
        let mut f = self.travar();
        Self::limpar_expirados(&mut f, Instant::now());
        !f.is_empty()
    }

    /// Tudo fora (o app vai sair).
    pub fn esvaziar(&self) {
        self.travar().clear();
    }
}

/// Segundos que faltam, arredondando para cima (com 0,4 s ainda há pedido).
fn restante(chegada: Instant, agora: Instant) -> u64 {
    match (chegada + PRAZO_APROVACAO).checked_duration_since(agora) {
        Some(f) if !f.is_zero() => f.as_secs() + u64::from(f.subsec_nanos() > 0),
        _ => 0,
    }
}

/// Um nome de processo da cadeia: escape visível (D51) e a seta reservada como escape.
fn exibir_processo(nome: &str) -> String {
    neutralizar(nome).replace(SEPARADOR_CADEIA, "\\u{2192}")
}

/// O pedido como a janela o mostra (D51 em todo texto que veio do agente ou do daemon).
pub(crate) fn exibir(a: &Atual) -> PedidoExibido {
    let p = &a.pedido;
    let entradas = p
        .entradas
        .iter()
        .enumerate()
        .map(|(i, n)| EntradaExibida {
            nome: neutralizar(n),
            tipo: a.tipos.as_ref().and_then(|t| t.get(i).copied().flatten()),
        })
        .collect();
    PedidoExibido {
        id: p.id,
        entradas,
        ambiente: p.ambiente.nome(),
        operacao: p.operacao.nome(),
        comando: neutralizar(&p.comando),
        diretorio: neutralizar(&p.diretorio),
        cadeia: p.cadeia.iter().map(|c| exibir_processo(c)).collect(),
        origem: neutralizar(&p.origem),
        confirmacao: p.confirmacao,
        entrada_oculta: p.entrada_oculta,
        restante: a.restante,
        estado: if a.bloqueado { "bloqueado" } else { "pendente" },
    }
}

/// Liga a thread que tira os `pedido_de_aprovacao` da `fila` (D65 a): cada um vai para
/// `aprovacoes` e chama `ao_chegar` (que abre ou traz a janela para a frente).
pub fn vigiar(
    fila: Arc<FilaAvisos>,
    aprovacoes: Arc<Aprovacoes>,
    ao_chegar: impl Fn() + Send + 'static,
) -> Result<(), ErroApp> {
    std::thread::Builder::new()
        .name("ward-app-aprovacao".into())
        .spawn(move || {
            loop {
                if let Some(p) = fila.esperar_aprovacao(Duration::from_secs(3600)) {
                    aprovacoes.chegou(p);
                    ao_chegar();
                }
            }
        })
        .map(|_| ())
        .map_err(|_| ErroApp::ConexaoPerdida)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::daemon::Aviso;
    use ward_core::entrada::Ambiente;
    use ward_core::protocolo::Operacao;

    pub(crate) fn pedido(id: u64, comando: &str, cadeia: &[&str]) -> PedidoDeAprovacao {
        PedidoDeAprovacao {
            id,
            entradas: vec!["acme-pg-aprovacao".into()],
            ambiente: Ambiente::Stage,
            operacao: Operacao::Executar,
            comando: comando.into(),
            diretorio: "\"/home/dev/projetos/acme\"".into(),
            cadeia: cadeia.iter().map(|c| (*c).to_owned()).collect(),
            origem: String::new(),
            confirmacao: false,
            entrada_oculta: false,
        }
    }

    #[test]
    fn decisao_so_pelos_cinco_nomes() {
        assert_eq!(decisao_de_nome("negar"), Ok(Decisao::Negar));
        assert_eq!(decisao_de_nome("uma_vez"), Ok(Decisao::PermitirUmaVez));
        assert_eq!(decisao_de_nome("uma_hora"), Ok(Decisao::PermitirUmaHora));
        assert_eq!(decisao_de_nome("sempre"), Ok(Decisao::PermitirSempre));
        assert_eq!(decisao_de_nome("confirmar"), Ok(Decisao::Confirmar));
        for ruim in ["", "Negar", "permitir", "uma-vez", "sempre ", "livre"] {
            assert_eq!(
                decisao_de_nome(ruim),
                Err(ErroApp::PedidoInvalido),
                "{ruim}"
            );
        }
    }

    /// Fase 26a: o diretório de trabalho vai à janela e também passa por D51 (o daemon
    /// já formata, mas a janela não confia).
    #[test]
    fn ataque_diretorio_com_bidi_vira_escape_visivel() {
        let mut p = pedido(1, "\"make\"", &["claude"]);
        p.diretorio = "\"/home/dev/\u{202E}emca\u{1b}[2J\"".into();
        let e = exibir(&Atual {
            pedido: p,
            restante: 90,
            bloqueado: false,
            tipos: None,
        });
        assert!(
            e.diretorio
                .chars()
                .all(|c| !ward_core::exibicao::invisivel(c))
        );
        assert_eq!(e.diretorio, "\"/home/dev/\\u{202E}emca\\x1b[2J\"");
    }

    /// V5.29 (D51): HTML, controle, bidi, largura zero, tags e `\n` falso aparecem como
    /// escape visível; nada some nem muda de ordem; a seta da cadeia não se forja.
    #[test]
    fn ataque_texto_do_agente_vira_escape_visivel() {
        let comando = "\"sh\" \"-c\" \"echo <img src=x onerror=alert(1)>\u{202E}fdp.exe\u{2066}x\u{200B}y\u{1b}[2J\nward run acme-pg-dev -- ls\u{E0041}\"";
        let p = pedido(1, comando, &["claude\u{202E}", "ba\u{2192}sh", "ward run"]);
        let a = Atual {
            pedido: p,
            restante: 90,
            bloqueado: false,
            tipos: Some(vec![Some("banco")]),
        };
        let e = exibir(&a);
        for c in e
            .comando
            .chars()
            .chain(e.cadeia.iter().flat_map(|c| c.chars()))
        {
            assert!(
                !ward_core::exibicao::invisivel(c),
                "caractere invisível {:?} passou",
                c
            );
        }
        assert!(e.comando.contains("\\u{202E}fdp.exe"), "{}", e.comando);
        assert!(e.comando.contains("\\u{2066}x\\u{200B}y"), "{}", e.comando);
        assert!(e.comando.contains("\\x1b[2J\\nward run"), "{}", e.comando);
        assert!(e.comando.contains("\\u{E0041}"), "{}", e.comando);
        // O HTML passa como texto (a tela nunca usa `{@html}`): nada some.
        assert!(e.comando.contains("<img src=x onerror=alert(1)>"));
        // Ordem e conteúdo: tirando os escapes, sobra o comando visível original.
        assert!(e.comando.starts_with("\"sh\" \"-c\" \"echo <img"));
        assert_eq!(e.cadeia, ["claude\\u{202E}", "ba\\u{2192}sh", "ward run"]);
        assert_eq!(e.entradas[0].tipo, Some("banco"));
        assert_eq!(e.estado, "pendente");
        // O nome da entrada também passa por D51 (o daemon já recusa, mas a janela não
        // confia).
        let mut p = pedido(2, "\"ls\"", &[]);
        p.entradas = vec!["acme\u{202E}gp".into()];
        let a = Atual {
            pedido: p,
            restante: 1,
            bloqueado: true,
            tipos: None,
        };
        let e = exibir(&a);
        assert_eq!(e.entradas[0].nome, "acme\\u{202E}gp");
        assert_eq!(e.estado, "bloqueado");
    }

    #[test]
    fn prazo_de_120_s_e_expirado_sai() {
        let a = Aprovacoes::default();
        let t0 = Instant::now();
        a.chegou_em(pedido(1, "\"ls\"", &[]), t0);
        assert_eq!(a.atual_em(t0).unwrap().restante, 120);
        assert_eq!(
            a.atual_em(t0 + Duration::from_millis(100_400))
                .unwrap()
                .restante,
            20
        );
        assert!(a.atual_em(t0 + PRAZO_APROVACAO).is_none());
        // Expirado não volta.
        assert!(a.atual_em(t0).is_none());
    }

    #[test]
    fn ordem_de_chegada_sem_repetir_e_com_limite() {
        let a = Aprovacoes::default();
        a.chegou(pedido(1, "\"a\"", &[]));
        a.chegou(pedido(2, "\"b\"", &[]));
        a.chegou(pedido(1, "\"outro\"", &[]));
        assert_eq!(a.pendentes(), [1, 2]);
        assert_eq!(a.atual().unwrap().pedido.comando, "\"a\"");
        a.marcar_bloqueado(1);
        assert_eq!(a.pendentes(), [2]);
        assert_eq!(a.situacao(1), Some(true));
        assert_eq!(a.situacao(2), Some(false));
        assert_eq!(a.situacao(3), None);
        a.remover(1);
        assert_eq!(a.atual().unwrap().pedido.id, 2);
        for i in 10..100 {
            a.chegou(pedido(i, "\"x\"", &[]));
        }
        assert_eq!(a.pendentes().len(), MAX_PENDENTES);
        a.esvaziar();
        assert!(!a.ha_pedido());
    }

    #[test]
    fn vigia_tira_a_aprovacao_e_deixa_o_destrave() {
        let fila = Arc::new(FilaAvisos::default());
        let aprovacoes = Arc::new(Aprovacoes::default());
        let (tx, rx) = std::sync::mpsc::channel();
        vigiar(Arc::clone(&fila), Arc::clone(&aprovacoes), move || {
            let _ = tx.send(());
        })
        .unwrap();
        fila.simular(Aviso::destrave_de_teste("acme-pg-aprovacao"));
        fila.simular(Aviso::Aprovacao(pedido(9, "\"ls\"", &["claude"])));
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(aprovacoes.pendentes(), [9]);
        assert_eq!(
            fila.tomar(),
            Some(Aviso::destrave_de_teste("acme-pg-aprovacao"))
        );
    }
}

/// O serviço inteiro com o daemon falso (a mesma regra do `wardd`): S9, S10, D51.
#[cfg(test)]
mod testes_servico {
    use super::testes::pedido;
    use super::*;
    use crate::area_transferencia::{AreaTransferencia, Copiador};
    use crate::daemon_falso::{DaemonFalso, SEGREDOS};
    use crate::nivel::Pin;
    use crate::servico::Servico;
    use std::sync::Mutex as M;
    use ward_core::entrada::{Ambiente, NivelAcesso};
    use ward_core::protocolo;
    use zeroize::Zeroizing;

    struct SemArea;
    impl AreaTransferencia for SemArea {
        fn escrever(&self, _: &[u8]) -> Result<(), ErroApp> {
            Err(ErroApp::AreaTransferencia)
        }
        fn ler(&self) -> Result<Option<Zeroizing<Vec<u8>>>, ErroApp> {
            Ok(None)
        }
        fn limpar(&self) -> Result<(), ErroApp> {
            Ok(())
        }
    }

    /// O daemon falso atrás de um `Arc<Mutex>`, para o teste olhar o estado dele.
    #[derive(Clone)]
    struct Falso(Arc<M<DaemonFalso>>);
    impl crate::daemon::Daemon for Falso {
        fn trocar(&mut self, p: &[u8]) -> Result<Zeroizing<Vec<u8>>, ErroApp> {
            self.0.lock().unwrap().trocar(p)
        }
    }

    const PIN_CERTO: &str = "135790";

    fn pin(s: &str) -> Option<Pin> {
        Some(Pin::novo(s.to_owned()).unwrap())
    }

    fn montar(pedidos: &[PedidoDeAprovacao]) -> (Servico<Falso, SemArea>, Falso) {
        let mut d = DaemonFalso::cheio();
        for p in pedidos {
            d = d.com_aprovacao(p);
        }
        let f = Falso(Arc::new(M::new(d)));
        let s = Servico::novo(f.clone(), Copiador::novo(SemArea));
        for p in pedidos {
            s.aprovacoes().chegou(p.clone());
        }
        (s, f)
    }

    fn pendentes_no_daemon(f: &Falso) -> Vec<u64> {
        f.0.lock().unwrap().aprovacoes_pendentes()
    }

    #[test]
    fn negar_sem_pin_e_permitir_uma_vez_com_pin() {
        let (s, f) = montar(&[pedido(1, "\"ls\"", &["claude"]), pedido(2, "\"ls\"", &[])]);
        let e = s.pedido_de_aprovacao().unwrap().unwrap();
        assert_eq!((e.id, e.ambiente, e.operacao), (1, "stage", "executar"));
        assert_eq!(e.entradas[0].tipo, Some("banco"));
        let r = s.responder_aprovacao(1, "negar", None, false).unwrap();
        assert_eq!((r.resultado, r.ha_outro), ("respondido", true));
        assert_eq!(pendentes_no_daemon(&f), [2]);
        let r = s
            .responder_aprovacao(2, "uma_vez", pin(PIN_CERTO), false)
            .unwrap();
        assert_eq!((r.resultado, r.ha_outro), ("respondido", false));
        assert!(pendentes_no_daemon(&f).is_empty());
        assert_eq!(s.pedido_de_aprovacao().unwrap(), None);
    }

    /// V5.18 (S9): PIN errado deixa pendente; o 5º trava o cofre, nega e a janela mostra
    /// bloqueado; "Permitir" no bloqueado não vai ao daemon; "Negar" só fecha.
    #[test]
    fn ataque_pin_errado_cinco_vezes_trava_e_nega() {
        let (s, f) = montar(&[pedido(7, "\"ls\"", &[])]);
        for _ in 0..4 {
            let r = s
                .responder_aprovacao(7, "uma_vez", pin("000000"), false)
                .unwrap();
            assert_eq!(r.resultado, "pin-incorreto");
            assert_eq!(pendentes_no_daemon(&f), [7]);
            assert_eq!(s.pedido_de_aprovacao().unwrap().unwrap().estado, "pendente");
        }
        let r = s
            .responder_aprovacao(7, "sempre", pin("000000"), false)
            .unwrap();
        assert_eq!((r.resultado, r.ha_outro), ("bloqueado", true));
        assert!(pendentes_no_daemon(&f).is_empty(), "o daemon negou");
        let e = s.pedido_de_aprovacao().unwrap().unwrap();
        assert_eq!((e.estado, e.restante), ("bloqueado", 0));
        assert_eq!(
            s.responder_aprovacao(7, "uma_vez", pin(PIN_CERTO), false),
            Err(ErroApp::Travado)
        );
        let r = s.responder_aprovacao(7, "negar", None, false).unwrap();
        assert_eq!((r.resultado, r.ha_outro), ("respondido", false));
        // A entrada não virou livre.
        assert_eq!(
            f.0.lock().unwrap().nivel_de("acme-pg-aprovacao"),
            Some(NivelAcesso::ComAprovacao)
        );
    }

    /// S10: resposta forjada pela interface (id que o daemon não mandou, decisão
    /// inventada, "Negar" com PIN, "Permitir" sem PIN) não chega ao daemon e o pedido
    /// continua pendente.
    #[test]
    fn ataque_resposta_forjada_nao_chega_ao_daemon() {
        let (s, f) = montar(&[pedido(3, "\"ls\"", &[])]);
        assert_eq!(
            s.responder_aprovacao(4, "uma_vez", pin(PIN_CERTO), false),
            Err(ErroApp::PedidoInvalido)
        );
        assert_eq!(
            s.responder_aprovacao(3, "livre", pin(PIN_CERTO), false),
            Err(ErroApp::PedidoInvalido)
        );
        assert_eq!(
            s.responder_aprovacao(3, "negar", pin(PIN_CERTO), false),
            Err(ErroApp::PedidoInvalido)
        );
        for d in ["uma_vez", "uma_hora", "sempre"] {
            assert_eq!(
                s.responder_aprovacao(3, d, None, false),
                Err(ErroApp::PedidoInvalido),
                "{d}"
            );
        }
        assert_eq!(pendentes_no_daemon(&f), [3]);
        assert_eq!(s.aprovacoes().pendentes(), [3]);
    }

    /// Fase 44c (D9): a confirmação da origem `http` vai à janela com a origem (escape
    /// visível) e só se decide por "Negar" ou "Confirmar" (sem PIN); um pedido com PIN
    /// nunca é decidido por "Confirmar". A interface forjada não chega ao daemon.
    #[test]
    fn ataque_confirmacao_so_por_confirmar_e_pedido_com_pin_nunca() {
        let mut c = pedido(11, "", &["claude", "ward playwright"]);
        c.operacao = ward_core::protocolo::Operacao::Playwright;
        c.diretorio.clear();
        c.origem = "http://localhost:8081\u{202E}".into();
        c.confirmacao = true;
        let n = pedido(12, "\"ls\"", &["claude"]);
        let (s, f) = montar(&[c, n]);
        let e = s.pedido_de_aprovacao().unwrap().unwrap();
        assert_eq!(e.id, 11);
        assert!(e.confirmacao);
        assert_eq!(e.origem, "http://localhost:8081\\u{202E}");
        for d in ["uma_vez", "uma_hora", "sempre"] {
            assert_eq!(
                s.responder_aprovacao(11, d, pin(PIN_CERTO), false),
                Err(ErroApp::PedidoInvalido),
                "{d} numa confirmação"
            );
        }
        assert_eq!(
            s.responder_aprovacao(11, "confirmar", pin(PIN_CERTO), false),
            Err(ErroApp::PedidoInvalido)
        );
        assert_eq!(
            s.responder_aprovacao(12, "confirmar", None, false),
            Err(ErroApp::PedidoInvalido),
            "confirmar num pedido com PIN"
        );
        assert_eq!(pendentes_no_daemon(&f), [11, 12]);
        let r = s.responder_aprovacao(11, "confirmar", None, false).unwrap();
        assert_eq!(r.resultado, "respondido");
        assert_eq!(pendentes_no_daemon(&f), [12]);
    }

    /// Fase 54b (D68): o pedido com a entrada padrão oculta vai à janela com o aviso; um
    /// "Permitir" sem a caixa (`ciente`) não chega ao daemon, nem a caixa num pedido sem
    /// o aviso, nem com "Negar"; com a caixa, passa. O daemon falso confere de novo.
    #[test]
    fn ataque_entrada_oculta_so_com_a_caixa() {
        let mut o = pedido(21, "\"python3\"", &["claude", "bash", "ward run"]);
        o.entrada_oculta = true;
        let n = pedido(22, "\"ls\"", &["claude"]);
        let (s, f) = montar(&[o, n]);
        let e = s.pedido_de_aprovacao().unwrap().unwrap();
        assert_eq!(e.id, 21);
        assert!(e.entrada_oculta);
        for d in ["uma_vez", "uma_hora", "sempre"] {
            assert_eq!(
                s.responder_aprovacao(21, d, pin(PIN_CERTO), false),
                Err(ErroApp::PedidoInvalido),
                "{d} sem a caixa"
            );
        }
        assert_eq!(
            s.responder_aprovacao(21, "negar", None, true),
            Err(ErroApp::PedidoInvalido)
        );
        assert_eq!(
            s.responder_aprovacao(22, "uma_vez", pin(PIN_CERTO), true),
            Err(ErroApp::PedidoInvalido),
            "caixa num pedido sem o aviso"
        );
        assert_eq!(pendentes_no_daemon(&f), [21, 22]);
        // O daemon também recusa (a interface forjada que passasse daqui).
        let q = protocolo::codificar_pedido(
            protocolo::Papel::App,
            &protocolo::Pedido::ResponderAprovacao {
                pedido: 21,
                decisao: protocolo::Decisao::PermitirUmaVez,
                pin: Some(ward_core::entrada::Segredo::de_texto(PIN_CERTO.to_owned())),
                entrada_confirmada: false,
            },
        )
        .unwrap();
        let r = crate::daemon::Daemon::trocar(&mut *f.0.lock().unwrap(), &q).unwrap();
        assert!(matches!(
            protocolo::decodificar_resposta(protocolo::Papel::App, &r),
            Ok(protocolo::Resposta::Erro(
                protocolo::ErroResposta::PedidoInvalido
            ))
        ));
        let r = s
            .responder_aprovacao(21, "uma_vez", pin(PIN_CERTO), true)
            .unwrap();
        assert_eq!(r.resultado, "respondido");
        let r = s
            .responder_aprovacao(22, "uma_vez", pin(PIN_CERTO), false)
            .unwrap();
        assert_eq!(r.resultado, "respondido");
        assert!(pendentes_no_daemon(&f).is_empty());
    }

    /// V5.16: "Permitir sempre" deixa a entrada livre; com dois pedidos, "fechar" nega
    /// todos os pendentes.
    #[test]
    fn sempre_deixa_livre_e_fechar_nega_todos() {
        let (s, f) = montar(&[pedido(5, "\"ls\"", &[])]);
        s.responder_aprovacao(5, "sempre", pin(PIN_CERTO), false)
            .unwrap();
        assert_eq!(
            f.0.lock().unwrap().nivel_de("acme-pg-aprovacao"),
            Some(NivelAcesso::Livre)
        );
        let mut prod = pedido(6, "\"ls\"", &[]);
        prod.entradas = vec!["acme-pg-prod".into()];
        prod.ambiente = Ambiente::Prod;
        let (s, f) = montar(&[prod, pedido(8, "\"ls\"", &[])]);
        assert_eq!(s.pedido_de_aprovacao().unwrap().unwrap().ambiente, "prod");
        s.negar_aprovacoes_pendentes();
        assert!(pendentes_no_daemon(&f).is_empty());
        assert_eq!(s.pedido_de_aprovacao().unwrap(), None);
    }

    /// S1, S11: nada do que a janela recebe tem PIN ou segredo do cofre.
    #[test]
    fn ataque_nada_de_segredo_ou_pin_na_janela() {
        let (s, _f) = montar(&[pedido(9, "\"psql\" \"-c\" \"select 1\"", &["claude"])]);
        let mut textos = vec![serde_json::to_string(&s.pedido_de_aprovacao().unwrap()).unwrap()];
        for _ in 0..2 {
            let r = s
                .responder_aprovacao(9, "uma_hora", pin("000000"), false)
                .unwrap();
            textos.push(serde_json::to_string(&r).unwrap());
        }
        let r = s
            .responder_aprovacao(9, "uma_hora", pin(PIN_CERTO), false)
            .unwrap();
        textos.push(serde_json::to_string(&r).unwrap());
        for t in &textos {
            assert!(!t.contains(PIN_CERTO) && !t.contains("000000"), "{t}");
            for seg in SEGREDOS {
                assert!(!t.contains(seg), "segredo na janela: {t}");
            }
        }
    }
}
