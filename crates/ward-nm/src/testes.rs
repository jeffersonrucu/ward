//! Testes do host com um daemon falso em memória (só valores falsos).

use super::*;
use ward_core::entry::Secret;
use ward_core::protocolo::Sugestao;

const SENHA_FALSA: &str = "falso-WP-senha-01";

fn b64(dados: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in dados.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                s.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

fn contem(p: &[u8], a: &[u8]) -> bool {
    p.windows(a.len()).any(|w| w == a)
}

/// Daemon falso: guarda os pedidos e responde o que o teste mandar, no papel dado
/// (`None`: corpo cru).
struct Falso {
    pedidos: Vec<Vec<u8>>,
    resposta: Box<dyn FnMut() -> Result<Vec<u8>, FalhaDaemon>>,
}

impl Falso {
    fn respondendo(r: Resposta) -> Falso {
        let corpo = protocolo::codificar_resposta(Papel::App, &r)
            .or_else(|_| protocolo::codificar_resposta(Papel::Extensao, &r))
            .or_else(|_| protocolo::codificar_resposta(Papel::Agente, &r))
            .unwrap()
            .to_vec();
        Falso::cru(corpo)
    }

    fn cru(corpo: Vec<u8>) -> Falso {
        Falso {
            pedidos: Vec::new(),
            resposta: Box::new(move || Ok(corpo.clone())),
        }
    }
}

impl Daemon for Falso {
    fn trocar(&mut self, pedido: &[u8]) -> Result<Zeroizing<Vec<u8>>, FalhaDaemon> {
        self.pedidos.push(pedido.to_vec());
        (self.resposta)().map(Zeroizing::new)
    }
}

fn atender_com(m: &str, d: &mut Falso) -> String {
    let mut logs = Vec::new();
    let r = atender(m.as_bytes(), d, &mut |l| logs.push(l.to_owned()));
    for l in &logs {
        assert!(!l.contains("falso-"), "log com conteúdo: {l}");
    }
    String::from_utf8(r.to_vec()).unwrap()
}

fn preenchimento() -> Resposta {
    Resposta::Preenchimento {
        usuario: "wp_admin".into(),
        senha: Secret::from_bytes(SENHA_FALSA.as_bytes().to_vec()),
        totp: None,
    }
}

const PREENCHER: &str = r#"{"v":1,"op":"preencher","dados":{"entrada":"acme-wp-dev","origem":"http://localhost:8081"}}"#;

#[test]
fn operacoes_sao_as_da_extensao() {
    for op in Operacao::TODAS {
        assert_eq!(
            OPERACOES.contains(op),
            op.permitida(Papel::Extensao),
            "{}",
            op.nome()
        );
    }
}

#[test]
fn origem_da_extensao() {
    assert!(origem_da_extensao_valida(
        b"chrome-extension://nhkehjnmlgkebhmelajfpmpblepaichf/"
    ));
    for ruim in [
        &b"chrome-extension://nhkehjnmlgkebhmelajfpmpblepaichf"[..],
        b"chrome-extension://nhkehjnmlgkebhmelajfpmpblepaichfq/",
        b"chrome-extension://dkacdineofgfabdognjgeoakekaenaez/",
        b"chrome-extension://NHKEHJNMLGKEBHMELAJFPMPBLEPAICHF/",
        b"chrome-extension://nhkehjnmlgkebhmelajfpmpblepaichf/x",
        // Outra extensão com o id bem formado (a que um manifesto do perfil do usuário
        // pusesse apontando para o `ward-nm`) não passa: só o id do Ward.
        b"chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/",
        b"chrome-extension://dkacdineofgfabdognjgeoakekaenaei/",
        b"moz-extension://nhkehjnmlgkebhmelajfpmpblepaichf/",
        b"https://localhost:8081/",
        b"",
        b"--parent-window=0",
    ] {
        assert!(!origem_da_extensao_valida(ruim), "{ruim:?}");
    }
}

#[test]
fn preencher_passa_pelo_daemon_e_volta_canonico() {
    let mut d = Falso::respondendo(preenchimento());
    let r = atender_com(PREENCHER, &mut d);
    assert!(r.contains(r#""tipo":"preenchimento""#), "{r}");
    assert!(r.contains(&b64(SENHA_FALSA.as_bytes())));
    // O pedido ao daemon é a forma canônica do protocolo, no papel extensão.
    assert_eq!(d.pedidos.len(), 1);
    let p = protocolo::decodificar_pedido(Papel::Extensao, &d.pedidos[0]).unwrap();
    assert_eq!(p.operacao(), Operacao::Preencher);
}

#[test]
fn ataque_operacoes_de_app_e_de_agente_nao_chegam_ao_daemon() {
    for m in [
        r#"{"v":1,"op":"revelar","dados":{"entrada":"acme-wp-dev","campo":"senha"}}"#,
        r#"{"v":1,"op":"copiar","dados":{"entrada":"acme-wp-dev","campo":"senha"}}"#,
        r#"{"v":1,"op":"totp","dados":{"entrada":"acme-totp-dev"}}"#,
        r#"{"v":1,"op":"lista"}"#,
        r#"{"v":1,"op":"detalhe","dados":{"entrada":"acme-wp-dev"}}"#,
        r#"{"v":1,"op":"mudar_nivel","dados":{"entrada":"acme-wp-dev","nivel":"livre","pin":"MTM1Nzkw"}}"#,
        r#"{"v":1,"op":"responder_aprovacao","dados":{}}"#,
        r#"{"v":1,"op":"destravar","dados":{"senha":"eA=="}}"#,
        r#"{"v":1,"op":"travar"}"#,
        r#"{"v":1,"op":"executar","dados":{}}"#,
        r#"{"v":1,"op":"login","dados":{"entrada":"acme-wp-dev"}}"#,
        r#"{"v":1,"op":"emitir_token","dados":{"entrada":"acme-wp-dev"}}"#,
        r#"{"v":1,"op":"restaurar_backup","dados":{}}"#,
    ] {
        let mut d = Falso::respondendo(preenchimento());
        let r = atender_com(m, &mut d);
        assert!(r.contains(r#""codigo":"nao_autorizado""#), "{m} -> {r}");
        assert!(d.pedidos.is_empty(), "{m} chegou ao daemon");
    }
}

#[test]
fn ataque_json_malformado_e_campos_a_mais() {
    for (m, codigo) in [
        ("", "pedido_invalido"),
        ("{", "pedido_invalido"),
        ("null", "pedido_invalido"),
        (r#"[1,2]"#, "pedido_invalido"),
        (r#"{"v":1}"#, "pedido_invalido"),
        (r#"{"v":2,"op":"estado"}"#, "versao_desconhecida"),
        (r#"{"v":1,"op":"inventada"}"#, "operacao_desconhecida"),
        (r#"{"v":1,"op":"estado","op":"revelar"}"#, "pedido_invalido"),
        (
            r#"{"v":1,"op":"preencher","dados":{"entrada":"acme-wp-dev","origem":"http://localhost:8081","campo":"senha"}}"#,
            "pedido_invalido",
        ),
        (
            r#"{"v":1,"op":"preencher","revelar":true,"dados":{"entrada":"acme-wp-dev","origem":"http://localhost:8081"}}"#,
            "pedido_invalido",
        ),
    ] {
        let mut d = Falso::respondendo(preenchimento());
        let r = atender_com(m, &mut d);
        assert!(r.contains(&format!(r#""codigo":"{codigo}""#)), "{m} -> {r}");
        assert!(d.pedidos.is_empty(), "{m} chegou ao daemon");
    }
    // Origem acima do limite de URL.
    let m = format!(
        r#"{{"v":1,"op":"sugerir","dados":{{"origem":"http://localhost:8081/{}"}}}}"#,
        "a".repeat(4096)
    );
    let mut d = Falso::respondendo(preenchimento());
    assert!(atender_com(&m, &mut d).contains("pedido_invalido"));
    assert!(d.pedidos.is_empty());
    // Bytes que não são UTF-8.
    let mut d = Falso::respondendo(preenchimento());
    let r = atender(b"{\"v\":1,\"op\":\"estado\xff\"}", &mut d, &mut |_| {});
    assert!(contem(&r, b"pedido_invalido") || contem(&r, b"operacao_desconhecida"));
    assert!(d.pedidos.is_empty());
}

#[test]
fn ataque_resposta_do_daemon_fora_do_papel_ou_da_operacao_nao_sai() {
    // Um daemon (ou um quadro corrompido) que mandasse segredo do app, token ou um
    // preenchimento para outra operação: nada disso chega à extensão.
    let casos: Vec<(&str, Resposta)> = vec![
        (
            PREENCHER,
            Resposta::Segredo(Secret::from_bytes(SENHA_FALSA.as_bytes().to_vec())),
        ),
        (
            PREENCHER,
            Resposta::Token(Secret::from_bytes(SENHA_FALSA.as_bytes().to_vec())),
        ),
        (
            r#"{"v":1,"op":"sugerir","dados":{"origem":"http://localhost:8081"}}"#,
            preenchimento(),
        ),
        (r#"{"v":1,"op":"estado"}"#, preenchimento()),
        (r#"{"v":1,"op":"abrir_app"}"#, preenchimento()),
        (PREENCHER, Resposta::Lista(Vec::new())),
        // Fase 43: o `aguardar_login` e o `resultado_login` nunca levam valor; só o
        // `preencher_login` recebe o preenchimento.
        (r#"{"v":1,"op":"aguardar_login"}"#, preenchimento()),
        (
            r#"{"v":1,"op":"resultado_login","dados":{"id":"AbCdEfGhIjKlMnOpQrStUvWxYz012345","resultado":"enviado"}}"#,
            preenchimento(),
        ),
        (
            r#"{"v":1,"op":"preencher_login","dados":{"id":"AbCdEfGhIjKlMnOpQrStUvWxYz012345","origem":"http://localhost:8081"}}"#,
            Resposta::Segredo(Secret::from_bytes(SENHA_FALSA.as_bytes().to_vec())),
        ),
        (
            PREENCHER,
            Resposta::Sugestoes(vec![Sugestao {
                nome: "x".into(),
                usuario: "y".into(),
                ambiente: ward_core::entry::Environment::Dev,
                totp: false,
            }]),
        ),
    ];
    for (m, r) in casos {
        let mut d = Falso::respondendo(r);
        let s = atender_com(m, &mut d);
        assert!(s.contains(r#""codigo":"interno""#), "{m} -> {s}");
        assert!(!s.contains(&b64(SENHA_FALSA.as_bytes())), "{s}");
        assert!(!s.contains(SENHA_FALSA), "{s}");
    }
    // Corpo cru que não é resposta.
    for cru in [
        &b"lixo"[..],
        b"{}",
        br#"{"v":1,"tipo":"segredo","dados":{"valor":"eA=="}}"#,
    ] {
        let mut d = Falso::cru(cru.to_vec());
        assert!(atender_com(PREENCHER, &mut d).contains("interno"));
    }
    // Daemon fora do ar.
    let mut d = Falso {
        pedidos: Vec::new(),
        resposta: Box::new(|| Err(FalhaDaemon::Indisponivel)),
    };
    assert!(atender_com(PREENCHER, &mut d).contains(r#""codigo":"interno""#));
}

fn quadro(corpo: &[u8]) -> Vec<u8> {
    let mut v = (corpo.len() as u32).to_ne_bytes().to_vec();
    v.extend_from_slice(corpo);
    v
}

fn mensagens(mut saida: &[u8]) -> Vec<String> {
    let mut v = Vec::new();
    while !saida.is_empty() {
        let n = u32::from_ne_bytes(saida[..4].try_into().unwrap()) as usize;
        v.push(String::from_utf8(saida[4..4 + n].to_vec()).unwrap());
        saida = &saida[4 + n..];
    }
    v
}

#[test]
fn estado_sem_o_papel_extensao_vira_nao_autorizado() {
    for papel in [Papel::Agente, Papel::App] {
        let mut d = Falso::cru(
            format!(r#"{{"v":1,"tipo":"estado","dados":{{"travado":false,"cofre_existe":true,"papel":"{}"}}}}"#, papel.nome())
                .into_bytes(),
        );
        let r = atender_com(r#"{"v":1,"op":"estado"}"#, &mut d);
        assert!(r.contains(r#""codigo":"nao_autorizado""#), "{r}");
    }
}

#[test]
fn laco_varias_mensagens_e_fim_normal() {
    let mut entrada = quadro(br#"{"v":1,"op":"estado"}"#);
    entrada.extend(quadro(PREENCHER.as_bytes()));
    let mut d = Falso::respondendo(Resposta::Estado {
        travado: false,
        cofre_existe: true,
        papel: Papel::Extensao,
    });
    let mut saida = Vec::new();
    servir(&mut &entrada[..], &mut saida, &mut d, &mut |_| {}).unwrap();
    let m = mensagens(&saida);
    assert_eq!(m.len(), 2);
    assert!(m[0].contains(r#""tipo":"estado""#));
    // `estado` não cabe como resposta de `preencher`.
    assert!(m[1].contains(r#""codigo":"interno""#));
}

#[test]
fn ataque_mensagem_acima_do_limite_vazia_ou_cortada() {
    // Acima do limite: o corpo nem é lido; responde quadro inválido e sai com erro.
    let mut entrada = ((MAX_MENSAGEM_ENTRADA + 1) as u32).to_ne_bytes().to_vec();
    entrada.extend(vec![b' '; 16]);
    let mut d = Falso::respondendo(preenchimento());
    let mut saida = Vec::new();
    let r = servir(&mut &entrada[..], &mut saida, &mut d, &mut |_| {});
    assert_eq!(r, Err(ErroMensagem::GrandeDemais));
    assert!(mensagens(&saida)[0].contains("quadro_invalido"));
    assert!(d.pedidos.is_empty());

    // 4 GiB declarados.
    let entrada = u32::MAX.to_ne_bytes();
    let mut saida = Vec::new();
    let r = servir(&mut &entrada[..], &mut saida, &mut d, &mut |_| {});
    assert_eq!(r, Err(ErroMensagem::GrandeDemais));

    // Vazia.
    let entrada = 0u32.to_ne_bytes();
    let mut saida = Vec::new();
    assert_eq!(
        servir(&mut &entrada[..], &mut saida, &mut d, &mut |_| {}),
        Err(ErroMensagem::Vazia)
    );

    // Cortada no tamanho e no corpo: nada é respondido.
    for entrada in [&[1u8, 0][..], &quadro(PREENCHER.as_bytes())[..20]] {
        let mut saida = Vec::new();
        assert_eq!(
            servir(&mut &entrada[..], &mut saida, &mut d, &mut |_| {}),
            Err(ErroMensagem::Cortada)
        );
        assert!(saida.is_empty());
    }
    assert!(d.pedidos.is_empty());

    // Exatamente no limite: lido e recusado pelo protocolo (não é JSON).
    let entrada = quadro(&vec![b' '; MAX_MENSAGEM_ENTRADA]);
    let mut saida = Vec::new();
    servir(&mut &entrada[..], &mut saida, &mut d, &mut |_| {}).unwrap();
    assert!(mensagens(&saida)[0].contains("pedido_invalido"));
}

#[test]
fn escrever_recusa_acima_do_limite_do_chrome() {
    let mut saida = Vec::new();
    assert!(escrever_mensagem(&mut saida, &vec![b'x'; MAX_MENSAGEM_SAIDA + 1]).is_err());
    assert!(escrever_mensagem(&mut saida, b"").is_err());
    assert!(saida.is_empty());
}
