//! Fase 43 (`ward login`; R14, D9; V6.3, V6.10, V6.11, V6.15; S1, S13), no núcleo do
//! daemon: o agente pede, a extensão (aqui em processo, pelo mesmo `atender` do socket
//! com o papel extensão) pega o pedido, manda a origem da aba, recebe o preenchimento só
//! se a origem bater e diz o desfecho.
//!
//! Ataques: origem do phishing e do redirecionamento (a credencial não sai, e o mesmo id
//! não preenche de novo), id forjado, preencher duas vezes, desfecho antes do
//! preenchimento, agente e app nas operações da extensão, bloqueada, tipo errado, cofre
//! travado e CLI que cai no meio, aprovação negada (nada chega à extensão), nenhuma
//! extensão no ar. Só valores falsos, num cofre de teste próprio.

#![cfg(feature = "ganchos-teste")]

mod comum;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use comum::*;
use ward_core::protocolo::{
    ErroResposta, Operacao, Papel, Pedido, PedidoDeAprovacao, Resposta, ResultadoLogin,
};
use wardd::aprovacao::{
    AbridorApp, Assinante, Cliente, ErroAbrirApp, Motor, RelogioMotor, SemCliente,
};
use wardd::login::PrazosLogin;
use wardd::nucleo::Daemon;

/// A URL das entradas `login` do cofre de teste comum (`t-livre`, `t-aprov`, `t-bloq`).
const ORIGEM: &str = "https://clientex.invalid";

fn prazos(retirada_ms: u64) -> PrazosLogin {
    PrazosLogin {
        aguardar: Duration::from_millis(300),
        retirada: Duration::from_millis(retirada_ms),
        total: Duration::from_secs(5),
    }
}

fn cofre(rotulo: &str, retirada_ms: u64) -> (Daemon, Dir) {
    let dir = Dir::novo(rotulo);
    let (d, _) = cofre_de_teste(&dir);
    (d.com_prazos_login(prazos(retirada_ms)), dir)
}

fn login_do_agente(d: &Daemon, cliente: &dyn Cliente, nome: &str) -> Resposta {
    match ped(
        Papel::Agente,
        "login",
        &format!(r#"{{"entrada":"{nome}"}}"#),
    ) {
        Ok(p) => d.atender_de(cliente, Papel::Agente, p),
        Err(e) => Resposta::Erro(e),
    }
}

/// O `aguardar_login` da extensão: `Some((id, entrada))` ou `None` (espera vazia).
fn aguardar(d: &Daemon) -> Option<(String, String)> {
    match pedir(d, Papel::Extensao, "aguardar_login", "{}") {
        Resposta::PedidoDeLogin { id, entrada } => {
            // S1: o pedido levado à extensão não tem valor nenhum da entrada.
            let q = codificado(
                Papel::Extensao,
                &Resposta::PedidoDeLogin {
                    id: id.clone(),
                    entrada: entrada.clone(),
                },
            );
            for s in SEGREDOS {
                assert!(!contem(&q, s.as_bytes()));
                assert!(!contem(&q, b64(s.as_bytes()).as_bytes()));
            }
            Some((id, entrada))
        }
        Resposta::Ok => None,
        r => panic!("{r:?}"),
    }
}

/// Espera a extensão ver o pedido (o agente está em outra thread).
fn pegar(d: &Daemon) -> (String, String) {
    for _ in 0..40 {
        if let Some(p) = aguardar(d) {
            return p;
        }
    }
    panic!("o pedido não chegou à extensão");
}

fn preencher(d: &Daemon, id: &str, origem: &str) -> Resposta {
    pedir(
        d,
        Papel::Extensao,
        "preencher_login",
        &format!(r#"{{"id":"{id}","origem":"{origem}"}}"#),
    )
}

fn resultado(d: &Daemon, id: &str, r: &str) -> Resposta {
    pedir(
        d,
        Papel::Extensao,
        "resultado_login",
        &format!(r#"{{"id":"{id}","resultado":"{r}"}}"#),
    )
}

fn sem_valor(r: &Resposta) {
    let q = codificado(Papel::Extensao, r);
    for s in SEGREDOS {
        assert!(!contem(&q, s.as_bytes()), "{r:?}");
        assert!(!contem(&q, b64(s.as_bytes()).as_bytes()), "{r:?}");
    }
}

/// V6.10 (núcleo): livre, origem certa, preenchimento uma vez, enviado → `ok`. A
/// resposta ao agente não tem valor nenhum.
#[test]
fn v6_10_login_livre_preenche_na_origem_certa_e_volta_ok() {
    let (d, _dir) = cofre("login-ok", 2000);
    std::thread::scope(|s| {
        let t = s.spawn(|| login_do_agente(&d, &SemCliente, "t-livre"));
        let (id, entrada) = pegar(&d);
        assert_eq!(entrada, "t-livre");
        assert_eq!(id.len(), 32);
        match preencher(&d, &id, ORIGEM) {
            Resposta::Preenchimento { usuario, senha, .. } => {
                assert_eq!(usuario, "admin");
                assert_eq!(senha.expor(), SEGREDOS[3].as_bytes());
            }
            r => panic!("{r:?}"),
        }
        // Um id preenche uma vez só.
        let r = preencher(&d, &id, ORIGEM);
        assert_eq!(erro(&r), Some(ErroResposta::Expirado));
        sem_valor(&r);
        assert!(ok(&resultado(&d, &id, "enviado")));
        let r = t.join().unwrap();
        assert!(ok(&r), "{r:?}");
        // A resposta do agente não carrega credencial (S1).
        let q = codificado(Papel::Agente, &r);
        for sgr in SEGREDOS {
            assert!(!contem(&q, sgr.as_bytes()));
        }
    });
    // O pedido morreu: o id não serve mais e a fila está vazia.
    assert_eq!(aguardar(&d), None);
}

/// V6.3 e V6.15 (núcleo): a aba no phishing (`localhost:8082`) ou na página para onde o
/// 8084 redireciona: `origem_diferente` sem valor, o agente recebe o mesmo erro, e o id
/// não preenche depois nem com a origem certa.
#[test]
fn v6_3_v6_15_ataque_origem_diferente_nao_preenche() {
    let (d, _dir) = cofre("login-phish", 2000);
    for (i, aba) in [
        "http://localhost:8082",
        "http://localhost:8084",
        "https://clientex.invalid.phish.test",
        "https://sub.clientex.invalid",
        "http://clientex.invalid",
        "https://clientex.invalid:8443",
        "",
        "chrome://settings",
    ]
    .into_iter()
    .enumerate()
    {
        std::thread::scope(|s| {
            let t = s.spawn(|| login_do_agente(&d, &SemCliente, "t-livre"));
            let (id, _) = pegar(&d);
            let r = preencher(&d, &id, aba);
            assert_eq!(erro(&r), Some(ErroResposta::OrigemDiferente), "{i}: {aba}");
            sem_valor(&r);
            let r = preencher(&d, &id, ORIGEM);
            assert_eq!(erro(&r), Some(ErroResposta::Expirado), "{aba}");
            sem_valor(&r);
            assert_eq!(
                erro(&resultado(&d, &id, "enviado")),
                Some(ErroResposta::Expirado)
            );
            assert_eq!(
                erro(&t.join().unwrap()),
                Some(ErroResposta::OrigemDiferente),
                "{aba}"
            );
        });
    }
}

/// Id forjado (de outro pedido, com um caractere trocado, de outro tamanho): nada sai,
/// e o pedido verdadeiro segue.
#[test]
fn ataque_id_forjado_nao_preenche() {
    let (d, _dir) = cofre("login-id", 2000);
    std::thread::scope(|s| {
        let t = s.spawn(|| login_do_agente(&d, &SemCliente, "t-livre"));
        let (id, _) = pegar(&d);
        let mut trocado = id.clone().into_bytes();
        trocado[31] = if trocado[31] == b'a' { b'b' } else { b'a' };
        let trocado = String::from_utf8(trocado).unwrap();
        for falso in [trocado.as_str(), "AbCdEfGhIjKlMnOpQrStUvWxYz012345"] {
            let r = preencher(&d, falso, ORIGEM);
            assert_eq!(erro(&r), Some(ErroResposta::Expirado));
            sem_valor(&r);
            assert_eq!(
                erro(&resultado(&d, falso, "enviado")),
                Some(ErroResposta::Expirado)
            );
        }
        for ruim in ["", "x", &id[..31]] {
            let r = preencher(&d, ruim, ORIGEM);
            assert_eq!(erro(&r), Some(ErroResposta::PedidoInvalido));
        }
        // "Enviado" antes de preencher não vale.
        assert_eq!(
            erro(&resultado(&d, &id, "enviado")),
            Some(ErroResposta::PedidoInvalido)
        );
        assert!(matches!(
            preencher(&d, &id, ORIGEM),
            Resposta::Preenchimento { .. }
        ));
        assert!(ok(&resultado(&d, &id, "recusado")));
        assert_eq!(erro(&t.join().unwrap()), Some(ErroResposta::LoginRecusado));
    });
}

/// Os desfechos da extensão viram o erro certo para o agente.
#[test]
fn desfechos_da_extensao() {
    let (d, _dir) = cofre("login-desfechos", 2000);
    for (r, esperado) in [
        (ResultadoLogin::SemFormulario, ErroResposta::LoginRecusado),
        (ResultadoLogin::AbaMudou, ErroResposta::OrigemDiferente),
        (ResultadoLogin::Falha, ErroResposta::NavegadorFalhou),
    ] {
        std::thread::scope(|s| {
            let t = s.spawn(|| login_do_agente(&d, &SemCliente, "t-livre"));
            let (id, _) = pegar(&d);
            // Antes do preenchimento (a aba sem formulário, ou que mudou na sonda).
            assert!(ok(&resultado(&d, &id, r.nome())));
            // Desfecho de novo: não muda nada.
            assert_eq!(
                erro(&resultado(&d, &id, "enviado")),
                Some(ErroResposta::Expirado)
            );
            assert_eq!(erro(&t.join().unwrap()), Some(esperado));
        });
    }
}

/// Agente e app não pegam, não preenchem e não respondem pedido de login (as duas
/// camadas: a leitura do protocolo e o núcleo).
#[test]
fn ataque_agente_e_app_nas_operacoes_da_extensao() {
    let (d, _dir) = cofre("login-papeis", 2000);
    std::thread::scope(|s| {
        let t = s.spawn(|| login_do_agente(&d, &SemCliente, "t-livre"));
        let (id, _) = pegar(&d);
        for papel in [Papel::Agente, Papel::App] {
            for (op, dados) in [
                ("aguardar_login", "{}".to_owned()),
                (
                    "preencher_login",
                    format!(r#"{{"id":"{id}","origem":"{ORIGEM}"}}"#),
                ),
                (
                    "resultado_login",
                    format!(r#"{{"id":"{id}","resultado":"enviado"}}"#),
                ),
            ] {
                let r = pedir(&d, papel, op, &dados);
                assert_eq!(erro(&r), Some(ErroResposta::NaoAutorizado), "{op}");
            }
            for p in [
                Pedido::AguardarLogin,
                Pedido::PreencherLogin {
                    id: id.clone(),
                    origem: ORIGEM.into(),
                },
                Pedido::ResultadoLogin {
                    id: id.clone(),
                    resultado: ResultadoLogin::Enviado,
                },
            ] {
                let r = d.atender(papel, p);
                assert_eq!(erro(&r), Some(ErroResposta::NaoAutorizado));
            }
        }
        // A extensão não pede `login` (só o agente).
        let r = pedir(&d, Papel::Extensao, "login", r#"{"entrada":"t-livre"}"#);
        assert_eq!(erro(&r), Some(ErroResposta::NaoAutorizado));
        // O pedido verdadeiro segue intacto.
        assert!(matches!(
            preencher(&d, &id, ORIGEM),
            Resposta::Preenchimento { .. }
        ));
        assert!(ok(&resultado(&d, &id, "enviado")));
        assert!(ok(&t.join().unwrap()));
    });
}

/// Bloqueada = inexistente (V3.17), tipo errado e inexistente: recusados antes de
/// qualquer pedido à extensão.
#[test]
fn recusas_antes_da_extensao() {
    let (d, _dir) = cofre("login-recusas", 2000);
    for (nome, esperado) in [
        ("t-bloq", ErroResposta::EntradaInexistente),
        ("nao-existe", ErroResposta::EntradaInexistente),
        ("s1", ErroResposta::TipoSemEntrega),
    ] {
        let r = login_do_agente(&d, &SemCliente, nome);
        assert_eq!(erro(&r), Some(esperado), "{nome}");
        assert_eq!(aguardar(&d), None, "{nome}");
    }
}

/// Nenhuma extensão no ar: o agente recebe `navegador_falhou` no prazo e o pedido morre
/// (uma extensão que chegue depois não o vê).
#[test]
fn sem_extensao_o_pedido_morre() {
    let (d, _dir) = cofre("login-sem-ext", 300);
    let r = login_do_agente(&d, &SemCliente, "t-livre");
    assert_eq!(erro(&r), Some(ErroResposta::NavegadorFalhou));
    assert_eq!(aguardar(&d), None);
}

/// Um pedido por vez: o segundo agente recebe `ocupado`.
#[test]
fn um_pedido_por_vez() {
    let (d, _dir) = cofre("login-ocupado", 2000);
    std::thread::scope(|s| {
        let t = s.spawn(|| login_do_agente(&d, &SemCliente, "t-livre"));
        let (id, _) = pegar(&d);
        let r = login_do_agente(&d, &SemCliente, "t-livre");
        assert_eq!(erro(&r), Some(ErroResposta::Ocupado));
        assert!(ok(&resultado(&d, &id, "falha")));
        assert_eq!(
            erro(&t.join().unwrap()),
            Some(ErroResposta::NavegadorFalhou)
        );
    });
}

/// Cofre travado com o pedido entregue: o agente recebe `travado` e o id não preenche.
#[test]
fn travar_no_meio_mata_o_pedido() {
    let (d, _dir) = cofre("login-travar", 2000);
    std::thread::scope(|s| {
        let t = s.spawn(|| login_do_agente(&d, &SemCliente, "t-livre"));
        let (id, _) = pegar(&d);
        assert!(ok(&pedir(&d, Papel::App, "travar", "{}")));
        assert_eq!(erro(&t.join().unwrap()), Some(ErroResposta::Travado));
        let r = preencher(&d, &id, ORIGEM);
        assert!(erro(&r).is_some(), "{r:?}");
        sem_valor(&r);
    });
}

struct ClienteQueSai(AtomicBool);

impl Cliente for ClienteQueSai {
    fn cadeia(&self) -> Vec<String> {
        vec!["claude".into(), "ward login".into()]
    }
    fn saiu(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// A CLI que cai (Ctrl+C) com o pedido entregue: o pedido morre e nada é preenchido.
#[test]
fn cli_que_cai_mata_o_pedido() {
    let (d, _dir) = cofre("login-cli", 2000);
    let cliente = ClienteQueSai(AtomicBool::new(false));
    std::thread::scope(|s| {
        let t = s.spawn(|| login_do_agente(&d, &cliente, "t-livre"));
        let (id, _) = pegar(&d);
        cliente.0.store(true, Ordering::Release);
        assert_eq!(erro(&t.join().unwrap()), Some(ErroResposta::Negado));
        let r = preencher(&d, &id, ORIGEM);
        assert_eq!(erro(&r), Some(ErroResposta::Expirado));
        sem_valor(&r);
    });
}

/// O `aguardar_login` da extensão (a cada 25 s) não conta como uso: não segura a trava
/// por inatividade (Fase 24).
#[test]
fn aguardar_nao_conta_como_uso() {
    assert!(!wardd::trava::conta_como_uso(
        Papel::Extensao,
        Operacao::AguardarLogin,
        &Resposta::Ok
    ));
    assert!(wardd::trava::conta_como_uso(
        Papel::Extensao,
        Operacao::PreencherLogin,
        &Resposta::Ok
    ));
}

// ---- V6.11: com aprovação, negar → nada chega à extensão ----

#[derive(Clone, Default)]
struct RelogioParado;

impl RelogioMotor for RelogioParado {
    fn agora(&self) -> Duration {
        Duration::ZERO
    }
}

struct AppFalso(Sender<PedidoDeAprovacao>);

impl Assinante for AppFalso {
    fn entregar(&mut self, r: &Resposta) -> bool {
        match r {
            Resposta::PedidoDeAprovacao(p) => self.0.send(p.clone()).is_ok(),
            outra => panic!("o motor mandou ao app {outra:?}"),
        }
    }
}

struct SemAbrir;

impl AbridorApp for SemAbrir {
    fn abrir(&self, _ao_sair: Box<dyn FnOnce() + Send>) -> Result<(), ErroAbrirApp> {
        Ok(())
    }
}

fn responder(d: &Daemon, id: u64, decisao: &str, pin: Option<&str>) -> Resposta {
    let pin = pin
        .map(|p| format!(r#","pin":"{}""#, b64(p.as_bytes())))
        .unwrap_or_default();
    pedir(
        d,
        Papel::App,
        "responder_aprovacao",
        &format!(r#"{{"pedido":{id},"decisao":"{decisao}"{pin}}}"#),
    )
}

#[test]
fn v6_11_com_aprovacao_negar_nao_chega_a_extensao_e_permitir_chega() {
    let (d, _dir) = cofre("login-aprov", 2000);
    let d = d.com_motor(Motor::novo(Arc::new(RelogioParado), Box::new(SemAbrir)));
    let (tx, rx) = mpsc::channel();
    d.motor().assinar(1, Box::new(AppFalso(tx)));
    let visto = Mutex::new(Vec::new());
    // Negar: a extensão não vê pedido nenhum durante nem depois.
    std::thread::scope(|s| {
        let t = s.spawn(|| login_do_agente(&d, &SemCliente, "t-aprov"));
        let p = rx.recv_timeout(Duration::from_secs(5)).expect("sem janela");
        assert_eq!(p.operacao, Operacao::Login);
        assert_eq!(p.entradas, vec!["t-aprov".to_owned()]);
        assert_eq!(aguardar(&d), None);
        assert!(ok(&responder(&d, p.id, "negar", None)));
        assert_eq!(erro(&t.join().unwrap()), Some(ErroResposta::Negado));
        visto.lock().unwrap().push(aguardar(&d));
    });
    assert_eq!(visto.lock().unwrap().as_slice(), &[None]);
    // Permitir uma vez (com o PIN): o pedido chega e o login segue.
    std::thread::scope(|s| {
        let t = s.spawn(|| login_do_agente(&d, &SemCliente, "t-aprov"));
        let p = rx.recv_timeout(Duration::from_secs(5)).expect("sem janela");
        assert!(ok(&responder(&d, p.id, "uma_vez", Some(PIN))));
        let (id, entrada) = pegar(&d);
        assert_eq!(entrada, "t-aprov");
        assert!(matches!(
            preencher(&d, &id, ORIGEM),
            Resposta::Preenchimento { .. }
        ));
        assert!(ok(&resultado(&d, &id, "enviado")));
        assert!(ok(&t.join().unwrap()));
    });
}

// ---- Fase 44c (D9): origem `http` sempre pela janela ----

/// Entrada `login` em `http://localhost:8081` no nível pedido.
fn login_http(d: &Daemon, nome: &str, nivel: &str) {
    login_na_url(d, nome, nivel, "http://localhost:8081/wp-login.php");
}

/// Entrada `login` na URL e no nível pedidos.
fn login_na_url(d: &Daemon, nome: &str, nivel: &str, url: &str) {
    let q = format!(
        r#"{{"entrada":{{"nome":"{nome}","grupo":"acme","ambiente":"prod","tipo":"login","campos":{{"url":"{url}","usuario":"wp_editor","senha":"{}"}}}}}}"#,
        b64(SEGREDOS[3].as_bytes())
    );
    let r = pedir(d, Papel::App, "salvar_entrada", &q);
    assert!(ok(&r), "{r:?}");
    // Entrada nova já nasce "com aprovação" (D50).
    if nivel != "com-aprovacao" {
        let r = mudar_nivel(d, Papel::App, nome, nivel, PIN);
        assert!(ok(&r), "{r:?}");
    }
}

/// Cliente que cai depois de umas voltas da espera (para encerrar um pedido que ninguém
/// decide sem esperar os 2 min).
struct CaiDepois(std::sync::atomic::AtomicU32);

impl Cliente for CaiDepois {
    fn cadeia(&self) -> Vec<String> {
        vec!["claude".into(), "ward login".into()]
    }
    fn saiu(&self) -> bool {
        self.0.fetch_add(1, Ordering::AcqRel) > 40
    }
}

/// O agente pede `login`, a extensão pega, preenche na origem `http` e diz "enviado".
fn login_http_completo(d: &Daemon, nome: &str) -> Resposta {
    std::thread::scope(|s| {
        let t = s.spawn(|| login_do_agente(d, &SemCliente, nome));
        let (id, _) = pegar(d);
        assert!(matches!(
            preencher(d, &id, "http://localhost:8081"),
            Resposta::Preenchimento { .. }
        ));
        assert!(ok(&resultado(d, &id, "enviado")));
        t.join().unwrap()
    })
}

/// Ataque da auditoria do M6 (`[::1]:8081` com o WordPress só em `127.0.0.1`): a origem
/// da aba confere, então o daemon não distingue. `ward login` numa entrada `http`
/// **livre** não vai à extensão sem a confirmação na janela: sem app, "aprovação
/// indisponível"; negado, nada; o pedido leva a origem, é só confirmação (sem PIN), e
/// um "Permitir" com PIN não o decide. Confirmado, segue como antes.
#[test]
fn ataque_login_http_livre_so_com_confirmacao() {
    let (d, _dir) = cofre("login-http-livre", 2000);
    login_http(&d, "h-livre", "livre");
    // Sem app.
    let r = login_do_agente(&d, &SemCliente, "h-livre");
    assert_eq!(erro(&r), Some(ErroResposta::AprovacaoIndisponivel));
    assert_eq!(aguardar(&d), None);
    // Negado: a extensão não vê nada.
    let (r, vistos) = com_janela(&d, "negar", None, || {
        login_do_agente(&d, &SemCliente, "h-livre")
    });
    assert_eq!(erro(&r), Some(ErroResposta::Negado));
    assert_eq!(aguardar(&d), None);
    let p = &vistos[0];
    assert_eq!(p.operacao, Operacao::Login);
    assert_eq!(p.entradas, ["h-livre".to_owned()]);
    assert_eq!(p.origem, "http://localhost:8081");
    assert!(p.confirmacao);
    // "Permitir uma vez" com PIN numa confirmação: recusado, e o pedido nunca chega.
    let cliente = CaiDepois(std::sync::atomic::AtomicU32::new(0));
    let (r, _) = com_janela(&d, "uma_vez", Some(PIN), || {
        login_do_agente(&d, &cliente, "h-livre")
    });
    assert_eq!(erro(&r), Some(ErroResposta::Negado));
    assert_eq!(aguardar(&d), None);
    // Confirmado: o pedido vai à extensão e preenche na origem certa.
    let (r, vistos) = com_janela(&d, "confirmar", None, || login_http_completo(&d, "h-livre"));
    assert!(ok(&r), "{r:?}");
    assert_eq!(vistos.len(), 1);
}

/// "Com aprovação" numa origem `http`: a janela de sempre (com PIN), agora com a origem;
/// "Confirmar" (sem PIN) não a decide. A liberação de 1 hora não pula a confirmação da
/// origem: o próximo uso ainda abre a janela (só a confirmação).
#[test]
fn ataque_login_http_com_aprovacao_pede_pin_e_mostra_a_origem() {
    let (d, _dir) = cofre("login-http-aprov", 2000);
    login_http(&d, "h-aprov", "com-aprovacao");
    let cliente = CaiDepois(std::sync::atomic::AtomicU32::new(0));
    let (r, vistos) = com_janela(&d, "confirmar", None, || {
        login_do_agente(&d, &cliente, "h-aprov")
    });
    assert_eq!(
        erro(&r),
        Some(ErroResposta::Negado),
        "confirmar sem PIN decidiu"
    );
    assert_eq!(aguardar(&d), None);
    assert!(!vistos[0].confirmacao);
    assert_eq!(vistos[0].origem, "http://localhost:8081");
    // Permitir por 1 hora (com PIN): segue.
    let (r, vistos) = com_janela(&d, "uma_hora", Some(PIN), || {
        login_http_completo(&d, "h-aprov")
    });
    assert!(ok(&r), "{r:?}");
    assert!(!vistos[0].confirmacao);
    // Dentro da liberação: ainda a confirmação da origem (sem PIN).
    let (r, vistos) = com_janela(&d, "confirmar", None, || login_http_completo(&d, "h-aprov"));
    assert!(ok(&r), "{r:?}");
    assert_eq!(vistos.len(), 1);
    assert!(vistos[0].confirmacao);
    // Sem app, nada.
    let r = login_do_agente(&d, &SemCliente, "h-aprov");
    assert_eq!(erro(&r), Some(ErroResposta::AprovacaoIndisponivel));
}

/// Fase 44e (Médio da 2ª auditoria do M6): `https` num host de loopback também pede a
/// confirmação. Com uma CA local (mkcert, `~/.pki`), um processo do uid em `[::1]`
/// apresenta um certificado válido para `localhost`; o `https` não prova quem responde.
/// Livre: sem app, "aprovação indisponível"; negado, a extensão não vê nada; o pedido
/// mostra a origem `https` e é só confirmação.
#[test]
fn ataque_login_https_loopback_livre_so_com_confirmacao() {
    let (d, _dir) = cofre("login-https-loopback", 2000);
    for (i, (url, origem)) in [
        (
            "https://localhost:8443/wp-login.php",
            "https://localhost:8443",
        ),
        ("https://127.0.0.1/wp-login.php", "https://127.0.0.1"),
        ("https://[::1]:8443/wp-login.php", "https://[::1]:8443"),
        ("https://wp.localhost/wp-login.php", "https://wp.localhost"),
        ("https://[::ffff:127.0.0.1]/", "https://[::ffff:127.0.0.1]"),
    ]
    .into_iter()
    .enumerate()
    {
        let nome = format!("l-livre-{i}");
        login_na_url(&d, &nome, "livre", url);
        let r = login_do_agente(&d, &SemCliente, &nome);
        assert_eq!(erro(&r), Some(ErroResposta::AprovacaoIndisponivel), "{url}");
        assert_eq!(aguardar(&d), None, "{url}");
        let (r, vistos) = com_janela(&d, "negar", None, || {
            login_do_agente(&d, &SemCliente, &nome)
        });
        assert_eq!(erro(&r), Some(ErroResposta::Negado), "{url}");
        assert_eq!(aguardar(&d), None, "{url}");
        assert_eq!(vistos.len(), 1, "{url}");
        assert_eq!(vistos[0].origem, origem, "{url}");
        assert!(vistos[0].confirmacao, "{url}");
    }
}

/// `https` de fora não muda: livre não abre janela nenhuma.
#[test]
fn login_https_livre_sem_janela() {
    let (d, _dir) = cofre("login-https-livre", 2000);
    let (r, vistos) = com_janela(&d, "negar", None, || {
        std::thread::scope(|s| {
            let t = s.spawn(|| login_do_agente(&d, &SemCliente, "t-livre"));
            let (id, _) = pegar(&d);
            assert!(matches!(
                preencher(&d, &id, ORIGEM),
                Resposta::Preenchimento { .. }
            ));
            assert!(ok(&resultado(&d, &id, "enviado")));
            t.join().unwrap()
        })
    });
    assert!(ok(&r), "{r:?}");
    assert!(vistos.is_empty());
}
