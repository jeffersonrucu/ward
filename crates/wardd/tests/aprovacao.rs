//! Motor de aprovação (Fase 23): V3.14–V3.16, V3.18, V3.36 (classificador), D19, D21,
//! D51, D57 (`abrir_app`).
//!
//! O aprovador é em processo (D8): um [`Assinante`] falso recebe o que o motor manda ao
//! app e a resposta volta por `Daemon::atender` com o papel app, o mesmo caminho do
//! socket (o pedido passa pela leitura do protocolo). O relógio do motor é manual.

mod comum;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use comum::*;
use ward_core::entrada::{Ambiente, NivelAcesso};
use ward_core::protocolo::{ErroResposta, Operacao, Papel, PedidoDeAprovacao, Resposta};
use ward_plataforma::{ClassePar, Recusa};
use wardd::aprovacao::{
    AbridorApp, Assinante, Cliente, ErroAbrirApp, Motor, RelogioMotor, SemCliente,
};
use wardd::nucleo::Daemon;
use wardd::servidor::papel_da_classe;
use wardd::uso::Autorizacao;

const PRAZO: Duration = Duration::from_secs(5);

#[derive(Clone, Default)]
struct RelogioManual(Arc<Mutex<Duration>>);

impl RelogioManual {
    fn ir(&self, t: Duration) {
        *self.0.lock().unwrap() = t;
    }
    fn avancar(&self, d: Duration) {
        *self.0.lock().unwrap() += d;
    }
}

impl RelogioMotor for RelogioManual {
    fn agora(&self) -> Duration {
        *self.0.lock().unwrap()
    }
}

/// O que o app falso recebeu.
#[derive(Debug)]
enum Recebido {
    Aprovacao(PedidoDeAprovacao),
    Destrave,
}

struct AppFalso(Sender<Recebido>);

impl Assinante for AppFalso {
    fn entregar(&mut self, r: &Resposta) -> bool {
        let x = match r {
            Resposta::PedidoDeAprovacao(p) => Recebido::Aprovacao(p.clone()),
            Resposta::PedidoDeDestrave => Recebido::Destrave,
            outra => panic!("o motor mandou ao app {outra:?}"),
        };
        self.0.send(x).is_ok()
    }
}

type AoSair = Box<dyn FnOnce() + Send>;

/// Abridor falso: conta as aberturas e guarda o `ao_sair`.
#[derive(Clone)]
struct AbridorFalso {
    resultado: Result<(), ErroAbrirApp>,
    aberturas: Arc<AtomicU32>,
    ao_sair: Arc<Mutex<Option<AoSair>>>,
}

impl AbridorFalso {
    fn novo(resultado: Result<(), ErroAbrirApp>) -> Self {
        Self {
            resultado,
            aberturas: Arc::default(),
            ao_sair: Arc::default(),
        }
    }
    fn sair(&self) {
        if let Some(f) = self.ao_sair.lock().unwrap().take() {
            f();
        }
    }
}

impl AbridorApp for AbridorFalso {
    fn abrir(&self, ao_sair: Box<dyn FnOnce() + Send>) -> Result<(), ErroAbrirApp> {
        self.aberturas.fetch_add(1, Ordering::SeqCst);
        if self.resultado.is_ok() {
            *self.ao_sair.lock().unwrap() = Some(ao_sair);
        }
        self.resultado
    }
}

struct Cena {
    d: Daemon,
    _dir: Dir,
    relogio: RelogioManual,
    abridor: AbridorFalso,
}

fn cena_com(rotulo: &str, abrir: Result<(), ErroAbrirApp>) -> Cena {
    let dir = Dir::novo(rotulo);
    let (d, _) = cofre_de_teste(&dir);
    let relogio = RelogioManual::default();
    let abridor = AbridorFalso::novo(abrir);
    let d = d.com_motor(Motor::novo(
        Arc::new(relogio.clone()),
        Box::new(abridor.clone()),
    ));
    Cena {
        d,
        _dir: dir,
        relogio,
        abridor,
    }
}

fn cena(rotulo: &str) -> Cena {
    cena_com(rotulo, Ok(()))
}

/// O app falso assina (como a conexão do papel app no servidor).
fn assinar(d: &Daemon, id: u64) -> Receiver<Recebido> {
    let (tx, rx) = mpsc::channel();
    d.motor().assinar(id, Box::new(AppFalso(tx)));
    rx
}

fn executar_json(nome: &str, argv: &[&[u8]]) -> String {
    executar_json_em(nome, argv, b"/")
}

fn executar_json_em(nome: &str, argv: &[&[u8]], cwd: &[u8]) -> String {
    let argv: Vec<String> = argv.iter().map(|a| format!("\"{}\"", b64(a))).collect();
    format!(
        r#"{{"entrada":"{nome}","argv":[{}],"cwd":"{}","ambiente":[]}}"#,
        argv.join(","),
        b64(cwd)
    )
}

fn usar(d: &Daemon, cliente: &dyn Cliente, nome: &str) -> Result<Autorizacao, ErroResposta> {
    let p = ped(Papel::Agente, "executar", &executar_json(nome, &[b"true"])).unwrap();
    d.autorizar_uso(cliente, &p)
}

fn pedido(rx: &Receiver<Recebido>) -> PedidoDeAprovacao {
    match rx.recv_timeout(PRAZO).expect("o app não recebeu o pedido") {
        Recebido::Aprovacao(p) => p,
        outro => panic!("{outro:?}"),
    }
}

fn responder(d: &Daemon, papel: Papel, id: u64, decisao: &str, pin: Option<&str>) -> Resposta {
    let pin = pin
        .map(|p| format!(r#","pin":"{}""#, b64(p.as_bytes())))
        .unwrap_or_default();
    pedir(
        d,
        papel,
        "responder_aprovacao",
        &format!(r#"{{"pedido":{id},"decisao":"{decisao}"{pin}}}"#),
    )
}

fn nivel(d: &Daemon, nome: &str) -> Option<NivelAcesso> {
    match pedir(d, Papel::App, "lista", "{}") {
        Resposta::Lista(l) => l.into_iter().find(|i| i.nome == nome).and_then(|i| i.nivel),
        r => panic!("{r:?}"),
    }
}

/// Se o teste cai com um uso esperando, o relógio pula o prazo e a espera acaba (sem
/// isso o `scope` esperaria a thread para sempre).
struct ExpirarSeCair<'a>(&'a RelogioManual);

impl Drop for ExpirarSeCair<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.avancar(Duration::from_secs(100_000));
        }
    }
}

/// Um uso que pergunta: devolve o resultado depois de `responder` com o pedido.
fn com_janela(
    c: &Cena,
    rx: &Receiver<Recebido>,
    nome: &str,
    responder: impl FnOnce(&PedidoDeAprovacao),
) -> Result<Autorizacao, ErroResposta> {
    std::thread::scope(|s| {
        let t = s.spawn(|| usar(&c.d, &SemCliente, nome));
        let _g = ExpirarSeCair(&c.relogio);
        let p = pedido(rx);
        responder(&p);
        t.join().unwrap()
    })
}

/// V3.14 (S10): sem resposta, expira em 2 min pelo relógio injetado (antes não).
#[test]
fn v3_14_sem_resposta_expira_em_2_min() {
    let c = cena("expira");
    let rx = assinar(&c.d, 1);
    let r = com_janela(&c, &rx, "t-aprov", |p| {
        c.relogio.ir(Duration::from_secs(119));
        std::thread::sleep(Duration::from_millis(150));
        assert!(
            c.d.motor().pendente(p.id).is_some(),
            "expirou antes de 2 min"
        );
        c.relogio.ir(Duration::from_secs(120));
    });
    assert_eq!(r, Err(ErroResposta::Expirado));
    // Responder depois de expirado não permite nada.
    c.relogio.ir(Duration::ZERO);
    assert_eq!(
        erro(&responder(&c.d, Papel::App, 1, "uma_vez", Some(PIN))),
        Some(ErroResposta::Expirado)
    );
}

/// V3.14 (S10): fechar a janela nega (o botão "Negar", o fechar da janela que manda
/// "negar", e a conexão do app que cai com o pedido entregue).
#[test]
fn v3_14_negar_e_fechar_negam() {
    let c = cena("fechar");
    let rx = assinar(&c.d, 1);
    let r = com_janela(&c, &rx, "t-aprov", |p| {
        assert!(ok(&responder(&c.d, Papel::App, p.id, "negar", None)));
    });
    assert_eq!(r, Err(ErroResposta::Negado));
    let r = com_janela(&c, &rx, "t-aprov", |_| c.d.motor().desassinar(1));
    assert_eq!(r, Err(ErroResposta::Negado));
    assert!(!c.d.motor().tem_assinante());
}

/// V3.14: "Permitir uma vez" vale só para aquele pedido; o seguinte pergunta de novo.
#[test]
fn v3_14_uma_vez_so_aquele_pedido() {
    let c = cena("uma-vez");
    let rx = assinar(&c.d, 1);
    let r = com_janela(&c, &rx, "t-aprov", |p| {
        assert!(ok(&responder(&c.d, Papel::App, p.id, "uma_vez", Some(PIN))));
    });
    assert_eq!(r, Ok(Autorizacao::Aprovado));
    let r = com_janela(&c, &rx, "t-aprov", |p| {
        assert!(ok(&responder(&c.d, Papel::App, p.id, "negar", None)));
    });
    assert_eq!(r, Err(ErroResposta::Negado));
}

/// V3.14 (D2): "Permitir por 1 hora" vale para a entrada até 59:59; em 60:01 pergunta;
/// travar acaba com a liberação; mudar o nível também.
#[test]
fn v3_14_uma_hora_ate_59_59_e_travar_encerra() {
    let c = cena("uma-hora");
    let rx = assinar(&c.d, 1);
    c.relogio.ir(Duration::from_secs(1000));
    let r = com_janela(&c, &rx, "t-aprov", |p| {
        assert!(ok(&responder(
            &c.d,
            Papel::App,
            p.id,
            "uma_hora",
            Some(PIN)
        )));
    });
    assert_eq!(r, Ok(Autorizacao::Aprovado));
    for t in [1001, 2000, 1000 + 3599] {
        c.relogio.ir(Duration::from_secs(t));
        assert_eq!(
            usar(&c.d, &SemCliente, "t-aprov"),
            Ok(Autorizacao::SemPerguntar)
        );
    }
    assert!(rx.try_recv().is_err(), "perguntou dentro da hora");
    // Só aquela entrada.
    let r = com_janela(&c, &rx, "s1", |p| {
        assert!(ok(&responder(&c.d, Papel::App, p.id, "negar", None)));
    });
    assert_eq!(r, Err(ErroResposta::Negado));
    c.relogio.ir(Duration::from_secs(1000 + 3601));
    let r = com_janela(&c, &rx, "t-aprov", |p| {
        assert!(ok(&responder(
            &c.d,
            Papel::App,
            p.id,
            "uma_hora",
            Some(PIN)
        )));
    });
    assert_eq!(r, Ok(Autorizacao::Aprovado));
    assert_eq!(
        usar(&c.d, &SemCliente, "t-aprov"),
        Ok(Autorizacao::SemPerguntar)
    );
    // Travar e destravar: pergunta de novo.
    assert!(ok(&pedir(&c.d, Papel::Agente, "travar", "{}")));
    assert!(ok(&destravar(&c.d, SENHA)));
    let r = com_janela(&c, &rx, "t-aprov", |p| {
        assert!(ok(&responder(
            &c.d,
            Papel::App,
            p.id,
            "uma_hora",
            Some(PIN)
        )));
    });
    assert_eq!(r, Ok(Autorizacao::Aprovado));
    // Mudar o nível (voltar atrás) também acaba com ela.
    assert!(ok(&mudar_nivel(&c.d, Papel::App, "t-aprov", "livre", PIN)));
    assert!(ok(&mudar_nivel(
        &c.d,
        Papel::App,
        "t-aprov",
        "com-aprovacao",
        PIN
    )));
    let r = com_janela(&c, &rx, "t-aprov", |p| {
        assert!(ok(&responder(&c.d, Papel::App, p.id, "negar", None)));
    });
    assert_eq!(r, Err(ErroResposta::Negado));
}

/// V3.14: "Permitir sempre" deixa a entrada livre no arquivo (vale depois de travar).
#[test]
fn v3_14_sempre_deixa_livre_no_arquivo() {
    let c = cena("sempre");
    let rx = assinar(&c.d, 1);
    let r = com_janela(&c, &rx, "t-aprov", |p| {
        assert!(ok(&responder(&c.d, Papel::App, p.id, "sempre", Some(PIN))));
    });
    assert_eq!(r, Ok(Autorizacao::Aprovado));
    assert!(ok(&pedir(&c.d, Papel::Agente, "travar", "{}")));
    assert!(ok(&destravar(&c.d, SENHA)));
    assert_eq!(nivel(&c.d, "t-aprov"), Some(NivelAcesso::Livre));
    assert_eq!(
        usar(&c.d, &SemCliente, "t-aprov"),
        Ok(Autorizacao::SemPerguntar)
    );
}

/// Ataque (V3.14, S9/S10): os três "Permitir" sem PIN válido não permitem; PIN errado
/// deixa o pedido pendente; o 5.º erro trava o cofre e nega o pedido.
#[test]
fn ataque_permitir_sem_pin_valido() {
    let c = cena("sem-pin");
    let rx = assinar(&c.d, 1);
    let r = com_janela(&c, &rx, "t-aprov", |p| {
        for decisao in ["uma_vez", "uma_hora", "sempre"] {
            // Sem PIN: o protocolo recusa o pedido inteiro.
            assert_eq!(
                erro(&responder(&c.d, Papel::App, p.id, decisao, None)),
                Some(ErroResposta::PedidoInvalido)
            );
            // PIN fora do formato.
            assert_eq!(
                erro(&responder(&c.d, Papel::App, p.id, decisao, Some("12"))),
                Some(ErroResposta::PedidoInvalido)
            );
        }
        for (i, decisao) in ["uma_vez", "uma_hora", "sempre", "uma_vez"]
            .iter()
            .enumerate()
        {
            assert_eq!(
                erro(&responder(
                    &c.d,
                    Papel::App,
                    p.id,
                    decisao,
                    Some(PIN_ERRADO)
                )),
                Some(ErroResposta::PinIncorreto),
                "{i}"
            );
            assert!(
                c.d.motor().pendente(p.id).is_some(),
                "o pedido saiu no erro {i}"
            );
        }
        assert_eq!(
            erro(&responder(
                &c.d,
                Papel::App,
                p.id,
                "sempre",
                Some(PIN_ERRADO)
            )),
            Some(ErroResposta::PinIncorreto)
        );
    });
    assert_eq!(r, Err(ErroResposta::Negado));
    assert!(c.d.travado());
    assert!(ok(&destravar(&c.d, SENHA)));
    assert_eq!(nivel(&c.d, "t-aprov"), Some(NivelAcesso::ComAprovacao));
}

/// Ataque (V3.15, V5.20): o agente (e a extensão) mandam "responder aprovação" com o
/// PIN certo: "não autorizado", e o pedido continua pendente até o app decidir.
#[test]
fn ataque_agente_responde_a_aprovacao() {
    let c = cena("agente-responde");
    let rx = assinar(&c.d, 1);
    let r = com_janela(&c, &rx, "t-aprov", |p| {
        for papel in [Papel::Agente, Papel::Extensao] {
            for decisao in ["uma_vez", "uma_hora", "sempre"] {
                assert_eq!(
                    erro(&responder(&c.d, papel, p.id, decisao, Some(PIN))),
                    Some(ErroResposta::NaoAutorizado)
                );
            }
            // Nem pelo `atender` direto com o pedido montado à mão.
            let q = ward_core::protocolo::Pedido::ResponderAprovacao {
                pedido: p.id,
                decisao: ward_core::protocolo::Decisao::PermitirUmaVez,
                pin: Some(ward_core::entrada::Segredo::de_texto(PIN.to_owned())),
            };
            assert_eq!(
                erro(&c.d.atender(papel, q)),
                Some(ErroResposta::NaoAutorizado)
            );
        }
        assert!(c.d.motor().pendente(p.id).is_some());
        assert!(ok(&responder(&c.d, Papel::App, p.id, "negar", None)));
    });
    assert_eq!(r, Err(ErroResposta::Negado));
}

/// V3.15 (D8): o código do daemon não tem aprovador nem variável/argumento que mude a
/// aprovação. Só `config.rs`, `log.rs`, `main.rs` e `trava.rs` (do primeiro nível)
/// leem o ambiente, e nenhum arquivo do `src/` (subpastas inclusive, scrutiny do M3)
/// tem `cfg(feature` fora o `modo-teste` dos caminhos e o `ganchos-teste` dos dois
/// ganchos de teste do núcleo (`com_gancho_depois_de_gravar`, `com_programas_ssh`),
/// cada um logo antes do próprio `pub fn`.
#[test]
fn v3_15_sem_aprovador_no_binario() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let arquivos = fontes_rs(&src);
    assert!(arquivos.len() > 10, "poucos arquivos em src/: {arquivos:?}");
    let mut ganchos = Vec::new();
    for p in &arquivos {
        let rel = p.strip_prefix(&src).unwrap().to_string_lossy().into_owned();
        let texto = std::fs::read_to_string(p).unwrap();
        conferir_v3_15(&rel, &texto, &mut ganchos);
    }
    ganchos.sort();
    assert_eq!(
        ganchos, GANCHOS_DE_TESTE,
        "ganchos de teste fora do esperado"
    );
}

/// Todos os `.rs` de `dir`, descendo nas subpastas; link é recusado.
fn fontes_rs(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let e = e.unwrap();
        let tipo = e.file_type().unwrap();
        let p = e.path();
        assert!(!tipo.is_symlink(), "link em src/: {}", p.display());
        if tipo.is_dir() {
            v.extend(fontes_rs(&p));
        } else if p.extension().is_some_and(|x| x == "rs") {
            v.push(p);
        }
    }
    v
}

/// Em ordem alfabética (o teste compara com a lista achada, ordenada).
const GANCHOS_DE_TESTE: [&str; 3] = [
    "com_chrome",
    "com_gancho_depois_de_gravar",
    "com_programas_ssh",
];
const CFG_GANCHOS: &str = "#[cfg(feature = \"ganchos-teste\")]";

fn conferir_v3_15(rel: &str, texto: &str, ganchos: &mut Vec<&'static str>) {
    let nome = rel.rsplit('/').next().unwrap_or(rel);
    for proibido in ["env::args", "aprovador_teste", "WARD_APROV", "auto_aprov"] {
        assert!(!texto.contains(proibido), "{rel}: {proibido}");
    }
    // trava.rs (Fase 24): só o barramento do logind de teste, fora da aprovação.
    if rel.contains('/') || !["config.rs", "log.rs", "main.rs", "trava.rs"].contains(&nome) {
        assert!(!texto.contains("env::var"), "{rel} lê o ambiente");
    }
    for (i, _) in texto.match_indices("cfg(feature") {
        let resto = &texto[i..];
        if resto.starts_with("cfg(feature = \"modo-teste\")") {
            assert!(
                !["aprovacao.rs", "uso.rs", "servidor.rs", "nucleo.rs"].contains(&rel),
                "{rel}: modo-teste no caminho da aprovação"
            );
            continue;
        }
        assert!(
            resto.starts_with(&CFG_GANCHOS[2..]),
            "{rel}: feature desconhecida"
        );
        assert_eq!(rel, "nucleo.rs", "ganchos-teste fora do núcleo");
        // A primeira linha depois do atributo que não é atributo nem comentário tem de
        // ser o `pub fn` de um dos ganchos.
        let depois = resto.split_once('\n').map(|(_, d)| d).unwrap_or_default();
        let assinatura = depois
            .lines()
            .map(str::trim)
            .find(|l| !l.starts_with("#[") && !l.starts_with("//"))
            .unwrap_or_default();
        let gancho = GANCHOS_DE_TESTE
            .iter()
            .find(|g| assinatura.starts_with(&format!("pub fn {g}(")))
            .unwrap_or_else(|| panic!("{rel}: ganchos-teste em outro item: {assinatura}"));
        ganchos.push(gancho);
    }
    // Toda definição de gancho vem com a feature, numa das 3 linhas de cima.
    for g in GANCHOS_DE_TESTE {
        for (i, _) in texto.match_indices(&format!("fn {g}(")) {
            let com_feature = texto[..i]
                .lines()
                .rev()
                .take(3)
                .any(|l| l.trim() == CFG_GANCHOS);
            assert!(com_feature, "{rel}: {g} sem a feature ganchos-teste");
        }
    }
}

/// Ataque ao V3.15: gancho sem a feature, a feature em outro item ou arquivo, feature
/// desconhecida, `env::var` numa subpasta e `modo-teste` no caminho da aprovação são
/// recusados.
#[test]
fn ataque_v3_15_gancho_sem_feature_ou_fora_do_lugar() {
    let certo =
        "#[cfg(feature = \"ganchos-teste\")]\n#[doc(hidden)]\npub fn com_programas_ssh(x) {}\n";
    let mut g = Vec::new();
    conferir_v3_15("nucleo.rs", certo, &mut g);
    assert_eq!(g, ["com_programas_ssh"]);
    let casos = [
        ("nucleo.rs", "pub fn com_gancho_depois_de_gravar(x) {}\n"),
        (
            "nucleo.rs",
            "#[cfg(feature = \"ganchos-teste\")]\npub fn aprovar_tudo() {}\n",
        ),
        ("servidor.rs", certo),
        ("nucleo.rs", "#[cfg(feature = \"aprovador\")]\nfn x() {}\n"),
        ("sub/config.rs", "let _ = std::env::var(\"X\");\n"),
        ("uso.rs", "#[cfg(feature = \"modo-teste\")]\nfn x() {}\n"),
    ];
    for (rel, texto) in casos {
        let r = std::panic::catch_unwind(|| conferir_v3_15(rel, texto, &mut Vec::new()));
        assert!(r.is_err(), "aceitou {rel}: {texto:?}");
    }
}

/// V3.16 (R16): `prod` livre passa sem perguntar; `prod` com aprovação oferece os quatro
/// botões, inclusive "Permitir sempre", e o ambiente vai em destaque.
#[test]
fn v3_16_prod_tem_os_mesmos_niveis_e_botoes() {
    let c = cena("prod");
    for nome in ["p-livre", "p-aprov"] {
        let q = login(nome, SEGREDOS[0]).replace(r#""ambiente":"dev""#, r#""ambiente":"prod""#);
        assert!(ok(&pedir(&c.d, Papel::App, "salvar_entrada", &q)));
    }
    assert!(ok(&mudar_nivel(&c.d, Papel::App, "p-livre", "livre", PIN)));
    let rx = assinar(&c.d, 1);
    assert_eq!(
        usar(&c.d, &SemCliente, "p-livre"),
        Ok(Autorizacao::SemPerguntar)
    );
    assert!(rx.try_recv().is_err());
    for decisao in ["negar", "uma_vez", "uma_hora", "sempre"] {
        let r = com_janela(&c, &rx, "p-aprov", |p| {
            assert_eq!(p.ambiente, Ambiente::Prod);
            assert_eq!(p.entradas, ["p-aprov"]);
            let pin = (decisao != "negar").then_some(PIN);
            assert!(
                ok(&responder(&c.d, Papel::App, p.id, decisao, pin)),
                "{decisao}"
            );
        });
        if decisao == "negar" {
            assert_eq!(r, Err(ErroResposta::Negado));
        } else {
            assert_eq!(r, Ok(Autorizacao::Aprovado), "{decisao}");
        }
        if decisao == "uma_hora" {
            // Para o "sempre" ainda perguntar.
            assert!(ok(&pedir(&c.d, Papel::Agente, "travar", "{}")));
            assert!(ok(&destravar(&c.d, SENHA)));
        }
    }
    assert_eq!(nivel(&c.d, "p-aprov"), Some(NivelAcesso::Livre));
}

/// D19: banco com túnel vale o nível mais restritivo das duas; a janela mostra as duas
/// e o ambiente mais sensível; a outra bloqueada nega sem perguntar.
#[test]
fn d19_nivel_combinado() {
    let c = cena("combinado");
    let rx = assinar(&c.d, 1);
    // b1 (prod, com aprovação) + s1 (stage, com aprovação).
    let r = com_janela(&c, &rx, "b1", |p| {
        assert_eq!(p.entradas, ["b1", "s1"]);
        assert_eq!(p.ambiente, Ambiente::Prod);
        assert!(ok(&responder(
            &c.d,
            Papel::App,
            p.id,
            "uma_hora",
            Some(PIN)
        )));
    });
    assert_eq!(r, Ok(Autorizacao::Aprovado));
    // A liberação vale para as duas.
    assert_eq!(usar(&c.d, &SemCliente, "b1"), Ok(Autorizacao::SemPerguntar));
    assert!(ok(&pedir(&c.d, Papel::Agente, "travar", "{}")));
    assert!(ok(&destravar(&c.d, SENHA)));
    // b1 livre, s1 com aprovação: pergunta.
    assert!(ok(&mudar_nivel(&c.d, Papel::App, "b1", "livre", PIN)));
    let r = com_janela(&c, &rx, "b1", |p| {
        assert_eq!(p.entradas, ["b1", "s1"]);
        assert!(ok(&responder(&c.d, Papel::App, p.id, "negar", None)));
    });
    assert_eq!(r, Err(ErroResposta::Negado));
    // As duas livres: passa.
    assert!(ok(&mudar_nivel(&c.d, Papel::App, "s1", "livre", PIN)));
    assert_eq!(usar(&c.d, &SemCliente, "b1"), Ok(Autorizacao::SemPerguntar));
    // A outra bloqueada: nega sem abrir janela.
    assert!(ok(&mudar_nivel(&c.d, Papel::App, "s1", "bloqueado", PIN)));
    assert_eq!(usar(&c.d, &SemCliente, "b1"), Err(ErroResposta::Negado));
    assert!(rx.try_recv().is_err());
    // A própria bloqueada continua igual a inexistente (V3.17).
    assert_eq!(
        usar(&c.d, &SemCliente, "s1"),
        usar(&c.d, &SemCliente, "nao-existe")
    );
    // "Sempre" no combinado deixa livres as duas que pediam aprovação.
    assert!(ok(&mudar_nivel(
        &c.d,
        Papel::App,
        "s1",
        "com-aprovacao",
        PIN
    )));
    assert!(ok(&mudar_nivel(
        &c.d,
        Papel::App,
        "b1",
        "com-aprovacao",
        PIN
    )));
    let r = com_janela(&c, &rx, "b1", |p| {
        assert!(ok(&responder(&c.d, Papel::App, p.id, "sempre", Some(PIN))));
    });
    assert_eq!(r, Ok(Autorizacao::Aprovado));
    assert_eq!(nivel(&c.d, "b1"), Some(NivelAcesso::Livre));
    assert_eq!(nivel(&c.d, "s1"), Some(NivelAcesso::Livre));
}

/// D21: sem como abrir o app, o pedido com aprovação é negado na hora; o app que abre e
/// sai sem receber o pedido também dá "aprovação indisponível".
#[test]
fn d21_aprovacao_indisponivel() {
    let c = cena_com("indisponivel", Err(ErroAbrirApp::Indisponivel));
    assert_eq!(
        usar(&c.d, &SemCliente, "t-aprov"),
        Err(ErroResposta::AprovacaoIndisponivel)
    );
    assert_eq!(c.abridor.aberturas.load(Ordering::SeqCst), 1);
    // Livre não precisa do app.
    assert_eq!(
        usar(&c.d, &SemCliente, "t-livre"),
        Ok(Autorizacao::SemPerguntar)
    );
    // O daemon sem motor (Daemon::novo) também.
    let dir = Dir::novo("sem-motor");
    let (d, _) = cofre_de_teste(&dir);
    assert_eq!(
        usar(&d, &SemCliente, "t-aprov"),
        Err(ErroResposta::AprovacaoIndisponivel)
    );

    let c = cena("app-saiu");
    let r = std::thread::scope(|s| {
        let t = s.spawn(|| usar(&c.d, &SemCliente, "t-aprov"));
        while c.abridor.aberturas.load(Ordering::SeqCst) == 0 {
            std::thread::sleep(Duration::from_millis(5));
        }
        std::thread::sleep(Duration::from_millis(50));
        c.abridor.sair();
        t.join().unwrap()
    });
    assert_eq!(r, Err(ErroResposta::AprovacaoIndisponivel));
}

/// O app aberto pelo motor recebe na assinatura o pedido que já esperava.
#[test]
fn app_aberto_recebe_o_pedido_ao_assinar() {
    let c = cena("abre-e-assina");
    let r = std::thread::scope(|s| {
        let t = s.spawn(|| usar(&c.d, &SemCliente, "t-aprov"));
        while c.abridor.aberturas.load(Ordering::SeqCst) == 0 {
            std::thread::sleep(Duration::from_millis(5));
        }
        let rx = assinar(&c.d, 7);
        let p = pedido(&rx);
        assert!(ok(&responder(&c.d, Papel::App, p.id, "uma_vez", Some(PIN))));
        t.join().unwrap()
    });
    assert_eq!(r, Ok(Autorizacao::Aprovado));
}

/// D51: o comando e a cadeia vão ao app formatados sem ambiguidade, com escape visível
/// (o agente não consegue mostrar outro comando que não o executado).
#[test]
fn d51_comando_e_cadeia_com_escape_visivel() {
    struct ClienteForjado;
    impl Cliente for ClienteForjado {
        fn cadeia(&self) -> Vec<String> {
            vec!["claude".into(), "ba\u{202E}hs".into(), "ward\x1b[2K".into()]
        }
        fn saiu(&self) -> bool {
            false
        }
    }
    let c = cena("d51");
    let rx = assinar(&c.d, 1);
    let r = std::thread::scope(|s| {
        let t = s.spawn(|| {
            let q = executar_json(
                "t-aprov",
                &[b"rm", "-rf /\x1b[2K\rls \u{202E}".as_bytes(), b"\xff"],
            );
            let p = ped(Papel::Agente, "executar", &q).unwrap();
            c.d.autorizar_uso(&ClienteForjado, &p)
        });
        let _g = ExpirarSeCair(&c.relogio);
        let p = pedido(&rx);
        assert_eq!(p.operacao, Operacao::Executar);
        assert_eq!(p.comando, r#""rm" "-rf /\x1b[2K\rls \u{202E}" "\xff""#);
        assert_eq!(p.cadeia, ["claude", r"ba\u{202E}hs", r"ward\x1b[2K"]);
        assert!(ok(&responder(&c.d, Papel::App, p.id, "negar", None)));
        t.join().unwrap()
    });
    assert_eq!(r, Err(ErroResposta::Negado));
}

/// Fase 26a: o `executar` roda no diretório da CLI, e a janela mostra esse diretório
/// exato, entre aspas e com escape visível (D51): o agente que roda `./deploy.sh` numa
/// pasta forjada não esconde onde o comando roda. O `ssh` (pasta pessoal do passwd,
/// fixa) vai sem diretório.
#[test]
fn f26a_diretorio_de_trabalho_vai_a_janela_com_escape_visivel() {
    let c = cena("f26a");
    let rx = assinar(&c.d, 1);
    let r = std::thread::scope(|s| {
        let t = s.spawn(|| {
            let q = executar_json_em(
                "t-aprov",
                &[b"./deploy.sh"],
                "/tmp/atacante/\u{202E}hs.yolped\x1b[2K /home/dev".as_bytes(),
            );
            let p = ped(Papel::Agente, "executar", &q).unwrap();
            c.d.autorizar_uso(&SemCliente, &p)
        });
        let _g = ExpirarSeCair(&c.relogio);
        let p = pedido(&rx);
        assert_eq!(p.comando, r#""./deploy.sh""#);
        assert_eq!(
            p.diretorio,
            r#""/tmp/atacante/\u{202E}hs.yolped\x1b[2K /home/dev""#
        );
        assert!(ok(&responder(&c.d, Papel::App, p.id, "negar", None)));
        t.join().unwrap()
    });
    assert_eq!(r, Err(ErroResposta::Negado));
}

/// O cliente (CLI) que cai enquanto espera cancela o pedido.
#[test]
fn cliente_que_cai_cancela() {
    struct Caiu(std::sync::atomic::AtomicBool);
    impl Cliente for Caiu {
        fn cadeia(&self) -> Vec<String> {
            Vec::new()
        }
        fn saiu(&self) -> bool {
            self.0.load(Ordering::SeqCst)
        }
    }
    let c = cena("caiu");
    let rx = assinar(&c.d, 1);
    let cli = Caiu(false.into());
    let r = std::thread::scope(|s| {
        let t = s.spawn(|| usar(&c.d, &cli, "t-aprov"));
        let p = pedido(&rx);
        cli.0.store(true, Ordering::SeqCst);
        let r = t.join().unwrap();
        assert!(c.d.motor().pendente(p.id).is_none());
        r
    });
    assert_eq!(r, Err(ErroResposta::Negado));
}

/// V3.18 (D16): cofre travado: o app recebe o pedido de destrave e o uso espera;
/// destravado aos 60 s segue (e então pergunta a aprovação, que conta em separado);
/// passados 120 s, "travado: tempo esgotado". Sem app, "travado" na hora.
#[test]
fn v3_18_espera_o_destrave_ate_2_min() {
    let c = cena("destrave");
    let rx = assinar(&c.d, 1);
    assert!(ok(&pedir(&c.d, Papel::Agente, "travar", "{}")));
    let r = std::thread::scope(|s| {
        let t = s.spawn(|| usar(&c.d, &SemCliente, "t-livre"));
        assert!(matches!(
            rx.recv_timeout(PRAZO).unwrap(),
            Recebido::Destrave
        ));
        c.relogio.ir(Duration::from_secs(60));
        std::thread::sleep(Duration::from_millis(100));
        assert!(!t.is_finished());
        assert!(ok(&destravar(&c.d, SENHA)));
        t.join().unwrap()
    });
    assert_eq!(r, Ok(Autorizacao::SemPerguntar));

    // Destrave e depois aprovação: os dois prazos em separado.
    assert!(ok(&pedir(&c.d, Papel::Agente, "travar", "{}")));
    let r = std::thread::scope(|s| {
        let t = s.spawn(|| usar(&c.d, &SemCliente, "t-aprov"));
        assert!(matches!(
            rx.recv_timeout(PRAZO).unwrap(),
            Recebido::Destrave
        ));
        c.relogio.avancar(Duration::from_secs(110));
        assert!(ok(&destravar(&c.d, SENHA)));
        let p = pedido(&rx);
        c.relogio.avancar(Duration::from_secs(100));
        std::thread::sleep(Duration::from_millis(100));
        assert!(c.d.motor().pendente(p.id).is_some());
        assert!(ok(&responder(&c.d, Papel::App, p.id, "uma_vez", Some(PIN))));
        t.join().unwrap()
    });
    assert_eq!(r, Ok(Autorizacao::Aprovado));

    assert!(ok(&pedir(&c.d, Papel::Agente, "travar", "{}")));
    let r = std::thread::scope(|s| {
        let t = s.spawn(|| usar(&c.d, &SemCliente, "t-livre"));
        assert!(matches!(
            rx.recv_timeout(PRAZO).unwrap(),
            Recebido::Destrave
        ));
        c.relogio.avancar(Duration::from_secs(120));
        t.join().unwrap()
    });
    assert_eq!(r, Err(ErroResposta::TravadoTempoEsgotado));
    // Pelo protocolo, a mesma resposta.
    let s2 = cena_com("destrave-sem-app", Err(ErroAbrirApp::Indisponivel));
    assert!(ok(&pedir(&s2.d, Papel::Agente, "travar", "{}")));
    assert_eq!(
        erro(&pedir(
            &s2.d,
            Papel::Agente,
            "executar",
            &executar_json("t-livre", &[b"true"])
        )),
        Some(ErroResposta::Travado)
    );
}

/// D57: `abrir_app` de qualquer papel, sem campo; abre pelo lançador; sem sessão,
/// "aprovação indisponível". Campo no pedido (caminho, ambiente) recusa o pedido.
#[test]
fn d57_abrir_app() {
    let c = cena("abrir-app");
    for papel in Papel::TODOS {
        assert!(ok(&pedir(&c.d, papel, "abrir_app", "{}")), "{papel:?}");
    }
    assert_eq!(c.abridor.aberturas.load(Ordering::SeqCst), 3);
    for dados in [
        r#"{"binario":"/tmp/x"}"#,
        r#"{"ambiente":[{"nome":"V0FZTEFORF9ESVNQTEFZ","valor":"L3RtcC9wcm94eQ=="}]}"#,
        r#"{"argv":[]}"#,
    ] {
        assert_eq!(
            erro(&pedir(&c.d, Papel::Agente, "abrir_app", dados)),
            Some(ErroResposta::PedidoInvalido)
        );
    }
    assert_eq!(c.abridor.aberturas.load(Ordering::SeqCst), 3);
    let s = cena_com("abrir-app-sem", Err(ErroAbrirApp::Indisponivel));
    assert_eq!(
        erro(&pedir(&s.d, Papel::Agente, "abrir_app", "{}")),
        Some(ErroResposta::AprovacaoIndisponivel)
    );
    let s = cena_com("abrir-app-ja", Err(ErroAbrirApp::JaAberto));
    assert!(ok(&pedir(&s.d, Papel::Agente, "abrir_app", "{}")));
}

/// V3.36 (D63, a parte do classificador): com o grupo do app e o pai certo, só o pid que
/// o lançador abriu é app; outro filho do daemon (o `ward run` da Fase 26) fica agente.
#[test]
fn v3_36_so_o_pid_do_lancador_e_app() {
    let lancado = 4242;
    let e = |p: u32| p == lancado;
    assert_eq!(papel_da_classe(ClassePar::App, lancado, e), Papel::App);
    assert_eq!(papel_da_classe(ClassePar::App, 4243, e), Papel::Agente);
    assert_eq!(
        papel_da_classe(ClassePar::App, lancado, |_| false),
        Papel::Agente
    );
    assert_eq!(
        papel_da_classe(
            ClassePar::Comum {
                motivo: Recusa::GrupoComum
            },
            lancado,
            e
        ),
        Papel::Agente
    );
    assert_eq!(
        papel_da_classe(ClassePar::HostExtensao, 1, e),
        Papel::Extensao
    );
}

/// V3.38 (D63): o `estado` traz o papel da conexão.
#[test]
fn v3_38_estado_traz_o_papel() {
    let c = cena("estado-papel");
    for papel in Papel::TODOS {
        match pedir(&c.d, papel, "estado", "{}") {
            Resposta::Estado { papel: p, .. } => assert_eq!(p, papel),
            r => panic!("{r:?}"),
        }
    }
}
