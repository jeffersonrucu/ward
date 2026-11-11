//! Fase 40 (D5, D9; V6.1, V6.2), no núcleo do daemon: `sugerir` e `preencher` do papel
//! extensão pela origem da aba. Ataques: origem parecida (outro host, porta, esquema,
//! subdomínio, userinfo), entrada de outro tipo, agente e app pedindo as operações da
//! extensão, e a extensão pedindo as do app e do agente. Só valores falsos, num cofre de
//! teste próprio.

mod comum;

use std::time::Duration;

use comum::{Dir, PIN, SEGREDOS, b64, erro, ok, ped, pedir};
use ward_core::protocolo::{self, ErroResposta, Operacao, Papel, Resposta};
use ward_core::totp::Totp;
use wardd::nucleo::{Daemon, Relogio};

/// Semente TOTP falsa (a de `acme-totp-dev`, D28).
const SEMENTE: &str = "JBSWY3DPEHPK3PXP";
const AGORA: u64 = 1_700_000_000;

struct RelogioFixo;

impl Relogio for RelogioFixo {
    fn dormir(&self, _d: Duration) {}

    fn agora(&self) -> Option<Duration> {
        Some(Duration::from_secs(1000))
    }

    fn segundos_unix(&self) -> Option<u64> {
        Some(AGORA)
    }
}

fn login(nome: &str, url: &str, usuario: &str, senha: &str, semente: Option<&str>) -> String {
    let totp = semente
        .map(|s| format!(r#","totp_semente":"{}""#, b64(s.as_bytes())))
        .unwrap_or_default();
    format!(
        r#"{{"entrada":{{"nome":"{nome}","grupo":"acme","ambiente":"dev","tipo":"login","campos":{{"url":"{url}","usuario":"{usuario}","senha":"{}"{totp}}}}}}}"#,
        b64(senha.as_bytes())
    )
}

/// Cofre de teste com logins em `localhost:8081` (um deles bloqueado), um com TOTP em
/// `localhost:8083`, um em `https`, um servidor e um banco.
fn cofre(dir: &Dir) -> Daemon {
    let d = Daemon::novo(dir.caminhos(), Box::new(RelogioFixo));
    let criar = pedir(
        &d,
        Papel::App,
        "criar_cofre",
        &format!(
            r#"{{"senha":"{}","pin":"{}"}}"#,
            b64(comum::SENHA.as_bytes()),
            b64(PIN.as_bytes())
        ),
    );
    assert!(ok(&criar), "{criar:?}");
    for q in [
        login(
            "acme-wp-dev",
            "http://localhost:8081/wp-login.php",
            "wp_admin",
            SEGREDOS[0],
            None,
        ),
        login(
            "acme-wp-prod",
            "http://localhost:8081/wp-login.php",
            "wp_editor",
            SEGREDOS[3],
            None,
        ),
        login(
            "acme-totp-dev",
            "http://localhost:8083/login",
            "wp_admin",
            SEGREDOS[0],
            Some(SEMENTE),
        ),
        login(
            "clientex-portal",
            "https://app.clientex.com.br/login",
            "admin",
            SEGREDOS[1],
            None,
        ),
        comum::servidor("s1"),
        comum::banco("b1", "s1"),
    ] {
        let r = pedir(&d, Papel::App, "salvar_entrada", &q);
        assert!(ok(&r), "{r:?}");
    }
    assert!(ok(&comum::mudar_nivel(
        &d,
        Papel::App,
        "acme-wp-prod",
        "bloqueado",
        PIN
    )));
    d
}

fn sugerir(d: &Daemon, papel: Papel, origem: &str) -> Resposta {
    pedir(d, papel, "sugerir", &format!(r#"{{"origem":"{origem}"}}"#))
}

/// `preencher` do popup sem janela nenhuma (o motor do daemon de teste não tem app).
fn preencher_cru(d: &Daemon, papel: Papel, entrada: &str, origem: &str) -> Resposta {
    pedir(
        d,
        papel,
        "preencher",
        &format!(r#"{{"entrada":"{entrada}","origem":"{origem}"}}"#),
    )
}

/// `preencher` do popup com o usuário confirmando a origem na janela (Fase 44c: as
/// entradas `http` só preenchem depois disso).
fn preencher(d: &Daemon, papel: Papel, entrada: &str, origem: &str) -> Resposta {
    comum::com_janela(d, "confirmar", None, || {
        preencher_cru(d, papel, entrada, origem)
    })
    .0
}

fn nomes(r: &Resposta) -> Vec<(String, String)> {
    match r {
        Resposta::Sugestoes(l) => l
            .iter()
            .map(|s| (s.nome.clone(), s.usuario.clone()))
            .collect(),
        outra => panic!("esperava sugestões: {:?}", outra.tipo()),
    }
}

/// Nenhum segredo do cofre de teste nos bytes (em claro ou em base64).
fn sem_segredo(bytes: &[u8]) {
    for s in SEGREDOS.iter().chain([&SEMENTE]) {
        assert!(!comum::contem(bytes, s.as_bytes()), "segredo em claro");
        assert!(
            !comum::contem(bytes, b64(s.as_bytes()).as_bytes()),
            "segredo em base64"
        );
    }
}

/// Fase 41: a sugestão leva o ambiente e se a entrada tem TOTP (só o fato), para o
/// popup mostrar o selo e "com TOTP"; nada de valor.
#[test]
fn sugestao_traz_ambiente_e_se_tem_totp() {
    use ward_core::entry::Environment;
    let dir = Dir::novo("ext-sugerir-totp");
    let d = cofre(&dir);
    let resumo = |r: &Resposta| match r {
        Resposta::Sugestoes(l) => l
            .iter()
            .map(|s| (s.nome.clone(), s.ambiente, s.totp))
            .collect::<Vec<_>>(),
        outra => panic!("esperava sugestões: {:?}", outra.tipo()),
    };
    assert_eq!(
        resumo(&sugerir(&d, Papel::Extensao, "http://localhost:8083/login")),
        [("acme-totp-dev".to_owned(), Environment::Dev, true)]
    );
    let r = sugerir(&d, Papel::Extensao, "http://localhost:8081/");
    assert_eq!(
        resumo(&r),
        [
            ("acme-wp-dev".to_owned(), Environment::Dev, false),
            ("acme-wp-prod".to_owned(), Environment::Dev, false),
        ]
    );
    let q = comum::codificado(Papel::Extensao, &r);
    assert!(comum::contem(&q, br#""ambiente":"dev","totp":false"#));
    sem_segredo(&q);
}

#[test]
fn v6_2_sugerir_so_a_origem_da_aba() {
    let dir = Dir::novo("ext-sugerir");
    let d = cofre(&dir);
    let r = sugerir(&d, Papel::Extensao, "http://localhost:8081/wp-login.php");
    assert_eq!(
        nomes(&r),
        [
            ("acme-wp-dev".to_owned(), "wp_admin".to_owned()),
            // Bloqueada também: o nível regula o agente, não o usuário na extensão.
            ("acme-wp-prod".to_owned(), "wp_editor".to_owned()),
        ]
    );
    // A resposta de sugerir não tem segredo nenhum.
    sem_segredo(&comum::codificado(Papel::Extensao, &r));
    assert_eq!(
        nomes(&sugerir(&d, Papel::Extensao, "http://localhost:8083/login")),
        [("acme-totp-dev".to_owned(), "wp_admin".to_owned())]
    );
    assert_eq!(
        nomes(&sugerir(&d, Papel::Extensao, "https://app.clientex.com.br")).len(),
        1
    );
    // V6.2 e origens parecidas: nada sugerido.
    for aba in [
        "http://127.0.0.1:8081/wp-login.php",
        "http://localhost:8082/wp-login.php",
        "http://localhost:8084/",
        "https://localhost:8081/",
        "http://localhost:8081@127.0.0.1/",
        "https://evil.app.clientex.com.br/login",
        "https://app.clientex.com.br.evil.invalid/",
        "http://app.clientex.com.br/login",
        "chrome://settings",
        "file:///etc/passwd",
        "",
    ] {
        assert!(
            nomes(&sugerir(&d, Papel::Extensao, aba)).is_empty(),
            "{aba:?}"
        );
    }
}

#[test]
fn v6_2_preencher_so_na_origem_certa() {
    let dir = Dir::novo("ext-preencher");
    let d = cofre(&dir);
    let r = preencher(
        &d,
        Papel::Extensao,
        "acme-wp-dev",
        "http://localhost:8081/wp-login.php",
    );
    match &r {
        Resposta::Preenchimento {
            usuario,
            senha,
            totp,
        } => {
            assert_eq!(usuario, "wp_admin");
            assert_eq!(senha.expose(), SEGREDOS[0].as_bytes());
            assert!(totp.is_none());
        }
        outra => panic!("esperava preenchimento: {:?}", outra.tipo()),
    }
    // TOTP: o código atual da semente, pelo relógio do daemon.
    let esperado = Totp::from_seed(&ward_core::entry::Secret::from_bytes(
        SEMENTE.as_bytes().to_vec(),
    ))
    .unwrap()
    .code_at(AGORA);
    match preencher(
        &d,
        Papel::Extensao,
        "acme-totp-dev",
        "http://localhost:8083/",
    ) {
        Resposta::Preenchimento { totp: Some(c), .. } => assert_eq!(&*c, esperado.code()),
        outra => panic!("esperava preenchimento com TOTP: {:?}", outra.tipo()),
    }

    // V6.2 (D9): a mesma entrada para outra origem é recusada, sem valor nenhum.
    for aba in [
        "http://127.0.0.1:8081/wp-login.php",
        "http://localhost:8082/wp-login.php",
        "http://localhost:8083/login",
        "https://localhost:8081/wp-login.php",
        "http://localhost:8081@evil.invalid/",
        "http://evil.invalid/",
        "chrome-extension://abcdefghijklmnopabcdefghijklmnop/",
        "",
    ] {
        let r = preencher(&d, Papel::Extensao, "acme-wp-dev", aba);
        assert_eq!(erro(&r), Some(ErroResposta::OrigemDiferente), "{aba:?}");
        sem_segredo(&comum::codificado(Papel::Extensao, &r));
    }
    // Entrada que não existe e entrada de outro tipo respondem igual.
    for nome in ["nao-existe", "s1", "b1"] {
        let r = preencher(&d, Papel::Extensao, nome, "http://localhost:8081/");
        assert_eq!(erro(&r), Some(ErroResposta::EntradaInexistente), "{nome}");
    }
    // Travado: nada.
    d.travar();
    let r = preencher(
        &d,
        Papel::Extensao,
        "acme-wp-dev",
        "http://localhost:8081/wp-login.php",
    );
    assert_eq!(erro(&r), Some(ErroResposta::Travado));
    let r = sugerir(&d, Papel::Extensao, "http://localhost:8081/wp-login.php");
    assert_eq!(erro(&r), Some(ErroResposta::Travado));
}

#[test]
fn v6_1_agente_e_app_nao_sugerem_nem_preenchem() {
    let dir = Dir::novo("ext-papeis");
    let d = cofre(&dir);
    for papel in [Papel::Agente, Papel::App] {
        // Primeira camada: a leitura do pedido.
        assert_eq!(
            erro(&preencher(
                &d,
                papel,
                "acme-wp-dev",
                "http://localhost:8081/wp-login.php"
            )),
            Some(ErroResposta::NaoAutorizado)
        );
        assert_eq!(
            erro(&sugerir(&d, papel, "http://localhost:8081/")),
            Some(ErroResposta::NaoAutorizado)
        );
        // Segunda e terceira camadas: o pedido já lido como extensão, atendido com
        // outro papel (o papel vem da conexão, nunca do quadro).
        let p = ped(
            Papel::Extensao,
            "preencher",
            r#"{"entrada":"acme-wp-dev","origem":"http://localhost:8081/"}"#,
        )
        .unwrap();
        let r = d.atender(papel, p);
        assert_eq!(erro(&r), Some(ErroResposta::NaoAutorizado));
        sem_segredo(&comum::codificado(papel, &r));
    }
}

#[test]
fn ataque_extensao_sem_operacoes_de_app_nem_de_agente() {
    let dir = Dir::novo("ext-tabela");
    let d = cofre(&dir);
    let permitidas = [
        Operacao::Estado,
        Operacao::Sugerir,
        Operacao::Preencher,
        Operacao::SalvarLogin,
        // Fase 43 (`ward login`): só a extensão.
        Operacao::AguardarLogin,
        Operacao::PreencherLogin,
        Operacao::ResultadoLogin,
        Operacao::AbrirApp,
    ];
    for op in Operacao::TODAS {
        assert_eq!(
            op.permitida(Papel::Extensao),
            permitidas.contains(op),
            "{}",
            op.nome()
        );
        if !permitidas.contains(op) {
            // Qualquer que seja o resto do quadro.
            for dados in ["{}", r#"{"entrada":"acme-wp-dev","campo":"senha"}"#] {
                assert_eq!(
                    ped(Papel::Extensao, op.nome(), dados).unwrap_err(),
                    ErroResposta::NaoAutorizado,
                    "{}",
                    op.nome()
                );
            }
        }
    }
    // Pedidos de app e de agente bem formados, atendidos com o papel extensão.
    for (papel, op, dados) in [
        (
            Papel::App,
            "revelar",
            r#"{"entrada":"acme-wp-dev","campo":"senha"}"#,
        ),
        (
            Papel::App,
            "copiar",
            r#"{"entrada":"acme-wp-dev","campo":"senha"}"#,
        ),
        (Papel::App, "totp", r#"{"entrada":"acme-totp-dev"}"#),
        (Papel::App, "detalhe", r#"{"entrada":"acme-wp-dev"}"#),
        (Papel::App, "lista", "{}"),
        (Papel::Agente, "lista", "{}"),
        (Papel::App, "emitir_token", r#"{"entrada":"s1"}"#),
        (Papel::Agente, "login", r#"{"entrada":"acme-wp-dev"}"#),
        (Papel::Agente, "playwright", r#"{"entrada":"acme-wp-dev"}"#),
    ] {
        let p = ped(papel, op, dados).unwrap_or_else(|e| panic!("{op}: {e:?}"));
        let r = d.atender(Papel::Extensao, p);
        assert_eq!(erro(&r), Some(ErroResposta::NaoAutorizado), "{op}");
    }
    let r = d.atender(
        Papel::Extensao,
        ped(
            Papel::App,
            "mudar_nivel",
            &format!(
                r#"{{"entrada":"acme-wp-dev","nivel":"livre","pin":"{}"}}"#,
                b64(PIN.as_bytes())
            ),
        )
        .unwrap(),
    );
    assert_eq!(erro(&r), Some(ErroResposta::NaoAutorizado));
    // O que a extensão recebe: nenhuma resposta de segredo, token ou lista do app.
    for t in [
        protocolo::TipoResposta::Segredo,
        protocolo::TipoResposta::Token,
        protocolo::TipoResposta::Lista,
        protocolo::TipoResposta::Detalhe,
        protocolo::TipoResposta::Saida,
    ] {
        assert!(!t.permitida(Papel::Extensao), "{}", t.nome());
    }
}

// --- Fase 42: `salvar_login` (V6.8) -------------------------------------------------------

/// Pedido `salvar_login` como o service worker da extensão monta: só nome, grupo,
/// ambiente, tipo e os campos URL, usuário e senha; `extra` entra cru no objeto
/// `entrada` (para os ataques).
fn salvar_login(d: &Daemon, papel: Papel, nome: &str, senha: &str, extra: &str) -> Resposta {
    pedir(
        d,
        papel,
        "salvar_login",
        &format!(
            r#"{{"entrada":{{"nome":"{nome}","grupo":"localhost","ambiente":"dev","tipo":"login","campos":{{"url":"http://localhost:8083/cadastro","usuario":"novo.usuario","senha":"{}"}}{extra}}}}}"#,
            b64(senha.as_bytes())
        ),
    )
}

/// Nível de uma entrada na lista do app (`None` se ela não existe).
fn nivel_na_lista(d: &Daemon, nome: &str) -> Option<ward_core::entry::AccessLevel> {
    match pedir(d, Papel::App, "lista", "{}") {
        Resposta::Lista(l) => l.into_iter().find(|i| i.nome == nome).and_then(|i| i.nivel),
        outra => panic!("esperava a lista: {:?}", outra.tipo()),
    }
}

/// Senha falsa nova, que não está em `SEGREDOS`.
const SENHA_NOVA: &str = "falso-cadastro-ward-42";

/// V6.8: o login novo da extensão aparece na lista com nível "com aprovação", é
/// sugerido para a origem e preenche com a senha enviada; a resposta não leva valor.
#[test]
fn v6_8_salvar_login_nasce_com_aprovacao() {
    use ward_core::entry::AccessLevel;
    let dir = Dir::novo("ext-salvar");
    let d = cofre(&dir);
    let r = salvar_login(&d, Papel::Extensao, "localhost-8083", SENHA_NOVA, "");
    assert!(ok(&r), "{r:?}");
    assert!(!comum::contem(
        &comum::codificado(Papel::Extensao, &r),
        SENHA_NOVA.as_bytes()
    ));
    assert_eq!(
        nivel_na_lista(&d, "localhost-8083"),
        Some(AccessLevel::Approval)
    );
    assert_eq!(
        nomes(&sugerir(&d, Papel::Extensao, "http://localhost:8083/")),
        [
            ("acme-totp-dev".to_owned(), "wp_admin".to_owned()),
            ("localhost-8083".to_owned(), "novo.usuario".to_owned()),
        ]
    );
    match preencher(
        &d,
        Papel::Extensao,
        "localhost-8083",
        "http://localhost:8083",
    ) {
        Resposta::Preenchimento { senha, totp, .. } => {
            assert_eq!(senha.expose(), SENHA_NOVA.as_bytes());
            assert!(totp.is_none());
        }
        outra => panic!("esperava preenchimento: {:?}", outra.tipo()),
    }
    // Gravado no disco: depois de travar e destravar, continua lá e com aprovação.
    assert!(ok(&pedir(&d, Papel::App, "travar", "{}")));
    assert!(ok(&comum::destravar(&d, comum::SENHA)));
    assert_eq!(
        nivel_na_lista(&d, "localhost-8083"),
        Some(AccessLevel::Approval)
    );
}

/// Ataque: a extensão tenta sobrescrever uma entrada que já existe (trocar a senha de
/// `acme-wp-prod`, que está bloqueada, ou de `acme-wp-dev`): o nome repetido é erro de
/// validação, e a entrada continua com a senha e o nível de antes.
#[test]
fn ataque_salvar_login_nao_sobrescreve_entrada() {
    use ward_core::entry::AccessLevel;
    let dir = Dir::novo("ext-salvar-sobrescrever");
    let d = cofre(&dir);
    for nome in ["acme-wp-dev", "acme-wp-prod"] {
        let r = salvar_login(&d, Papel::Extensao, nome, SENHA_NOVA, "");
        assert!(
            matches!(erro(&r), Some(ErroResposta::Validacao(_))),
            "{nome}: {r:?}"
        );
    }
    assert_eq!(
        nivel_na_lista(&d, "acme-wp-prod"),
        Some(AccessLevel::Blocked)
    );
    assert_eq!(
        nivel_na_lista(&d, "acme-wp-dev"),
        Some(AccessLevel::Approval)
    );
    match preencher(&d, Papel::Extensao, "acme-wp-dev", "http://localhost:8081") {
        Resposta::Preenchimento { senha, .. } => assert_eq!(senha.expose(), SEGREDOS[0].as_bytes()),
        outra => panic!("esperava preenchimento: {:?}", outra.tipo()),
    }
}

/// Ataque: o `salvar_login` só aceita o que o popup pede. Nível no pedido (o quadro
/// não tem esse campo), TOTP, extras, anexos, projeto, tags e outro tipo são recusados
/// e nada é gravado.
#[test]
fn ataque_salvar_login_so_os_campos_do_popup() {
    let dir = Dir::novo("ext-salvar-campos");
    let d = cofre(&dir);
    let semente = b64(SEMENTE.as_bytes());
    let valor = b64(b"x");
    let recusados = [
        (r#","nivel":"livre""#.to_owned(), false),
        (r#","projeto":"p1""#.to_owned(), true),
        (r#","tags":["t1"]"#.to_owned(), true),
        (
            format!(r#","extras":[{{"nome":"API_KEY","valor":"{valor}","segredo":true}}]"#),
            true,
        ),
        (
            format!(r#","anexos":[{{"nome":"a.txt","conteudo":"{valor}"}}]"#),
            true,
        ),
    ];
    for (extra, chega_ao_daemon) in &recusados {
        let r = salvar_login(&d, Papel::Extensao, "localhost-8083", SENHA_NOVA, extra);
        let e = erro(&r).unwrap_or_else(|| panic!("{extra}: aceito"));
        if *chega_ao_daemon {
            assert_eq!(e, ErroResposta::PedidoInvalido, "{extra}");
        }
        assert_eq!(nivel_na_lista(&d, "localhost-8083"), None, "{extra}");
    }
    // TOTP dentro dos campos do login.
    let r = pedir(
        &d,
        Papel::Extensao,
        "salvar_login",
        &format!(
            r#"{{"entrada":{{"nome":"localhost-8083","grupo":"g","ambiente":"dev","tipo":"login","campos":{{"url":"http://localhost:8083/","usuario":"u","senha":"{}","totp_semente":"{semente}"}}}}}}"#,
            b64(SENHA_NOVA.as_bytes())
        ),
    );
    assert_eq!(erro(&r), Some(ErroResposta::PedidoInvalido));
    // Outro tipo (banco) pelo `salvar_login`: a leitura do pedido recusa.
    assert!(ped(Papel::Extensao, "salvar_login", &comum::banco("b9", "s1")).is_err());
    assert_eq!(nivel_na_lista(&d, "localhost-8083"), None);
    assert_eq!(nivel_na_lista(&d, "b9"), None);
}

/// Ataque: agente e app não pedem `salvar_login`, nem com o pedido já lido como
/// extensão; com o cofre travado, nada é gravado.
#[test]
fn ataque_salvar_login_so_extensao_e_destravado() {
    let dir = Dir::novo("ext-salvar-papeis");
    let d = cofre(&dir);
    let p = |nome: &str| {
        format!(
            r#"{{"entrada":{{"nome":"{nome}","grupo":"g","ambiente":"dev","tipo":"login","campos":{{"url":"http://localhost:8083/","usuario":"u","senha":"{}"}}}}}}"#,
            b64(SENHA_NOVA.as_bytes())
        )
    };
    for papel in [Papel::Agente, Papel::App] {
        assert_eq!(
            erro(&pedir(&d, papel, "salvar_login", &p("x1"))),
            Some(ErroResposta::NaoAutorizado)
        );
        let lido = ped(Papel::Extensao, "salvar_login", &p("x2")).unwrap();
        assert_eq!(
            erro(&d.atender(papel, lido)),
            Some(ErroResposta::NaoAutorizado)
        );
    }
    assert_eq!(nivel_na_lista(&d, "x1"), None);
    assert_eq!(nivel_na_lista(&d, "x2"), None);
    assert!(ok(&pedir(&d, Papel::App, "travar", "{}")));
    assert_eq!(
        erro(&pedir(&d, Papel::Extensao, "salvar_login", &p("x3"))),
        Some(ErroResposta::Travado)
    );
    assert!(ok(&comum::destravar(&d, comum::SENHA)));
    assert_eq!(nivel_na_lista(&d, "x3"), None);
}

// --- Fase 44c (D9): origem `http` só com a confirmação na janela ---------------------------

/// Ataque (auditoria do M6, `[::1]`): outro processo do uid escuta em
/// `[::1]:8081` com o WordPress só em `127.0.0.1`, e o Chrome manda a aba
/// `http://localhost:8081` para ele. A origem confere, então o daemon não tem como saber;
/// por isso o popup só preenche uma entrada `http` depois de o usuário confirmar a origem
/// na janela. Sem app: "aprovação indisponível"; negado: nada; o pedido mostra a origem,
/// é do popup (`preencher`) e não pede PIN. `https` segue sem janela.
#[test]
fn ataque_preencher_http_sem_confirmar_nada_sai() {
    let dir = Dir::novo("ext-http-confirma");
    let d = cofre(&dir);
    let aba = "http://localhost:8081/wp-login.php";
    // Sem app nenhum.
    let r = preencher_cru(&d, Papel::Extensao, "acme-wp-dev", aba);
    assert_eq!(erro(&r), Some(ErroResposta::AprovacaoIndisponivel));
    sem_segredo(&comum::codificado(Papel::Extensao, &r));
    // Negado na janela.
    let (r, vistos) = comum::com_janela(&d, "negar", None, || {
        preencher_cru(&d, Papel::Extensao, "acme-wp-dev", aba)
    });
    assert_eq!(erro(&r), Some(ErroResposta::Negado));
    sem_segredo(&comum::codificado(Papel::Extensao, &r));
    assert_eq!(vistos.len(), 1);
    let p = &vistos[0];
    assert_eq!(p.operacao, Operacao::Preencher);
    assert_eq!(p.entradas, ["acme-wp-dev".to_owned()]);
    assert_eq!(p.origem, "http://localhost:8081");
    assert!(p.confirmacao);
    sem_segredo(&comum::codificado(
        Papel::App,
        &Resposta::PedidoDeAprovacao(p.clone()),
    ));
    // Os "Permitir" com PIN não decidem uma confirmação (a janela forjada não pula o
    // formato): o pedido segue pendente e expira; aqui o laço da janela só tenta.
    let (r, _) = comum::com_janela(&d, "uma_vez", Some(PIN), || {
        // Sem decisão aceita, o pedido ficaria pendente até expirar (2 min): o teste
        // encerra pelo cliente que sai.
        struct SaiLogo(std::sync::atomic::AtomicU32);
        impl wardd::aprovacao::Cliente for SaiLogo {
            fn cadeia(&self) -> Vec<String> {
                vec!["chrome".into(), "ward-nm".into()]
            }
            fn saiu(&self) -> bool {
                self.0.fetch_add(1, std::sync::atomic::Ordering::AcqRel) > 40
            }
        }
        let p = ped(
            Papel::Extensao,
            "preencher",
            &format!(r#"{{"entrada":"acme-wp-dev","origem":"{aba}"}}"#),
        )
        .unwrap();
        d.atender_de(&SaiLogo(Default::default()), Papel::Extensao, p)
    });
    assert_eq!(
        erro(&r),
        Some(ErroResposta::Negado),
        "uma_vez decidiu uma confirmação"
    );
    sem_segredo(&comum::codificado(Papel::Extensao, &r));
    // Confirmado: preenche.
    let (r, vistos) = comum::com_janela(&d, "confirmar", None, || {
        preencher_cru(&d, Papel::Extensao, "acme-wp-dev", aba)
    });
    assert!(
        matches!(r, Resposta::Preenchimento { .. }),
        "{:?}",
        r.tipo()
    );
    assert_eq!(vistos.len(), 1);
    // `https`: nenhuma janela.
    let (r, vistos) = comum::com_janela(&d, "negar", None, || {
        preencher_cru(
            &d,
            Papel::Extensao,
            "clientex-portal",
            "https://app.clientex.com.br/login",
        )
    });
    assert!(
        matches!(r, Resposta::Preenchimento { .. }),
        "{:?}",
        r.tipo()
    );
    assert!(vistos.is_empty(), "janela num preencher https");
    // Origem diferente: recusa antes de perguntar.
    let (r, vistos) = comum::com_janela(&d, "confirmar", None, || {
        preencher_cru(&d, Papel::Extensao, "acme-wp-dev", "http://127.0.0.1:8081/")
    });
    assert_eq!(erro(&r), Some(ErroResposta::OrigemDiferente));
    assert!(vistos.is_empty());
}

/// Fase 44e (Médio da 2ª auditoria do M6): o popup numa entrada `https` de loopback
/// (`https://localhost`, `https://[::1]`) também só preenche depois da confirmação. Com
/// uma CA local plantada (mkcert, `~/.pki`), o processo em `[::1]` passa no TLS. Sem
/// app: "aprovação indisponível"; negado: nenhum valor; o pedido mostra a origem.
#[test]
fn ataque_preencher_https_loopback_sem_confirmar_nada_sai() {
    let dir = Dir::novo("ext-https-loopback");
    let d = cofre(&dir);
    for (i, (url, origem)) in [
        ("https://localhost:8443/login", "https://localhost:8443"),
        ("https://[::1]:8443/login", "https://[::1]:8443"),
        ("https://127.0.0.1/login", "https://127.0.0.1"),
    ]
    .into_iter()
    .enumerate()
    {
        let nome = format!("l-{i}");
        let r = pedir(
            &d,
            Papel::App,
            "salvar_entrada",
            &login(&nome, url, "wp_admin", SEGREDOS[0], None),
        );
        assert!(ok(&r), "{r:?}");
        let r = preencher_cru(&d, Papel::Extensao, &nome, url);
        assert_eq!(erro(&r), Some(ErroResposta::AprovacaoIndisponivel), "{url}");
        sem_segredo(&comum::codificado(Papel::Extensao, &r));
        let (r, vistos) = comum::com_janela(&d, "negar", None, || {
            preencher_cru(&d, Papel::Extensao, &nome, url)
        });
        assert_eq!(erro(&r), Some(ErroResposta::Negado), "{url}");
        sem_segredo(&comum::codificado(Papel::Extensao, &r));
        assert_eq!(vistos.len(), 1, "{url}");
        assert_eq!(vistos[0].operacao, Operacao::Preencher);
        assert_eq!(vistos[0].origem, origem, "{url}");
        assert!(vistos[0].confirmacao, "{url}");
        let (r, _) = comum::com_janela(&d, "confirmar", None, || {
            preencher_cru(&d, Papel::Extensao, &nome, url)
        });
        assert!(matches!(r, Resposta::Preenchimento { .. }), "{url}");
    }
}
