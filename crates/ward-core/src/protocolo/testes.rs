//! Testes do protocolo (Fase 20): tabela operação × papel (V3.15), nenhuma credencial
//! para o agente (V3.3, S1), quadros recusados (V3.4), JSON estrito, anexos de D29 pelo
//! app, erros de D39 sem geração, e fuzz do decodificador. Só valores falsos (D15).

use std::io::{self, Read};

use base64ct::{Base64, Encoding};

use super::quadro::{self, ErroQuadro, ler_quadro, limite_do_quadro};
use super::*;
use crate as wc;
use crate::corpo::ErroAbrir;
use crate::entrada::{
    Ambiente, Rascunho, RascunhoAnexo, RascunhoDados, RascunhoExtra, Segredo, TipoEntrada,
};
use crate::gravacao::ErroGravacao;
use crate::limites::{
    KIB, MAX_ANEXOS, MAX_ANEXOS_SOMADOS, MAX_EXTRAS, MAX_NOME_ANEXO, MAX_NOTA, MAX_QUADRO_APP,
    MAX_QUADRO_COMUM, MAX_VALOR_EXTRA, MIB,
};

// O mesmo cofre de teste dos testes do corpo (o módulo deles é privado de `corpo`).
#[allow(clippy::duplicate_mod)]
#[path = "../corpo/cofre_teste.rs"]
mod cofre_teste;
use cofre_teste::cofre_de_teste;

// ---- Ajudantes ----

fn b64(b: &[u8]) -> String {
    Base64::encode_string(b)
}

fn ped(papel: Papel, json: &str) -> Result<Pedido, ErroResposta> {
    decodificar_pedido(papel, json.as_bytes())
}

fn seg(s: &str) -> Segredo {
    Segredo::de_texto(s.to_owned())
}

/// Os `dados` mínimos válidos de cada operação (valores falsos).
fn dados_validos(op: Operacao) -> String {
    let pin = b64(b"135790");
    let senha = b64(b"falso-mestra-ward-01");
    match op {
        Operacao::Estado
        | Operacao::Travar
        | Operacao::Lista
        | Operacao::AbrirApp
        | Operacao::Backups
        | Operacao::Configuracao
        | Operacao::BackupExterno
        | Operacao::AguardarLogin
        | Operacao::Pendrives => "{}".into(),
        Operacao::PreencherLogin => {
            r#"{"id":"AbCdEfGhIjKlMnOpQrStUvWxYz012345","origem":"http://localhost:8081"}"#.into()
        }
        Operacao::ResultadoLogin => {
            r#"{"id":"AbCdEfGhIjKlMnOpQrStUvWxYz012345","resultado":"enviado"}"#.into()
        }
        Operacao::DefinirBackupExterno => r#"{"ligado":true,"uuid":"ABCD-1234"}"#.into(),
        Operacao::RestaurarBackup => {
            format!(r#"{{"versao":"ward-20261109-153000.ward","senha":"{senha}","pin":"{pin}"}}"#)
        }
        Operacao::Destravar => format!(r#"{{"senha":"{senha}"}}"#),
        Operacao::Executar => format!(
            r#"{{"entrada":"acme-pg-dev","argv":["{}"],"cwd":"{}","tipo_entrada":"nula","ambiente":[{{"nome":"{}","valor":"{}"}}]}}"#,
            b64(b"psql"),
            b64(b"/tmp"),
            b64(b"LANG"),
            b64(b"C.UTF-8")
        ),
        Operacao::Ssh => r#"{"entrada":"acme-ssh-dev","tipo_entrada":"terminal"}"#.into(),
        Operacao::Login | Operacao::Playwright | Operacao::Detalhe | Operacao::Totp => {
            r#"{"entrada":"acme-wp-dev"}"#.into()
        }
        Operacao::RemoverEntrada | Operacao::EmitirToken => r#"{"entrada":"acme-ssh-dev"}"#.into(),
        Operacao::EntradaPadrao => format!(r#"{{"dados":"{}"}}"#, b64(b"ls\n")),
        Operacao::Janela => r#"{"linhas":24,"colunas":80}"#.into(),
        Operacao::CriarCofre => format!(r#"{{"senha":"{senha}","pin":"{pin}"}}"#),
        Operacao::Revelar | Operacao::Copiar => {
            r#"{"entrada":"acme-wp-dev","campo":"senha"}"#.into()
        }
        Operacao::SalvarEntrada | Operacao::SalvarLogin => format!(
            r#"{{"entrada":{{"nome":"clientex-stage-wp","grupo":"clientex","ambiente":"stage","tipo":"login","campos":{{"url":"https://clientex.invalid/login","usuario":"admin","senha":"{}"}}}}}}"#,
            b64(b"falso-clientex-01")
        ),
        Operacao::MudarNivel => {
            format!(r#"{{"entrada":"acme-pg-dev","nivel":"bloqueado","pin":"{pin}"}}"#)
        }
        Operacao::ResponderAprovacao => {
            format!(r#"{{"pedido":7,"decisao":"uma_vez","pin":"{pin}"}}"#)
        }
        Operacao::TrocarPin => format!(r#"{{"atual":"{pin}","novo":"{}"}}"#, b64(b"2468")),
        Operacao::TrocarSenha => {
            format!(
                r#"{{"atual":"{senha}","nova":"{}"}}"#,
                b64(b"falso-nova-02")
            )
        }
        Operacao::TempoTrava => r#"{"minutos":240}"#.into(),
        Operacao::Sugerir => r#"{"origem":"http://localhost:8081"}"#.into(),
        Operacao::Preencher => {
            r#"{"entrada":"acme-wp-dev","origem":"http://localhost:8081"}"#.into()
        }
    }
}

/// Põe `campo` (`"k":v`) no começo do objeto `dados`.
fn com_campo(dados: &str, campo: &str) -> String {
    if dados == "{}" {
        format!("{{{campo}}}")
    } else {
        dados.replacen('{', &format!("{{{campo},"), 1)
    }
}

fn quadro_de(op: Operacao, dados: &str) -> String {
    format!(r#"{{"v":1,"op":"{}","dados":{dados}}}"#, op.nome())
}

/// Um papel que pode pedir a operação.
fn papel_de(op: Operacao) -> Papel {
    Papel::TODOS
        .into_iter()
        .find(|p| op.permitida(*p))
        .expect("operação sem papel")
}

/// Um pedido de cada operação, montado em memória.
fn um_pedido_de_cada() -> Vec<Pedido> {
    Operacao::TODAS
        .iter()
        .map(|&op| ped(papel_de(op), &quadro_de(op, &dados_validos(op))).unwrap())
        .collect()
}

fn item(nome: &str) -> ItemLista {
    ItemLista {
        nome: nome.into(),
        tipo: TipoEntrada::Login,
        grupo: "clientex".into(),
        ambiente: Ambiente::Stage,
        projeto: Some("portal \"novo\" \\ ação".into()),
        tags: vec!["web".into()],
        nivel: None,
        endereco: Some("https://clientex.invalid/login".into()),
        resumo: None,
    }
}

/// Uma resposta de cada tipo, com credenciais falsas onde o tipo leva.
fn uma_resposta_de_cada() -> Vec<Resposta> {
    let entradas = cofre_de_teste();
    let servidor = entradas
        .iter()
        .find(|e| e.tipo() == TipoEntrada::Servidor)
        .unwrap();
    vec![
        Resposta::Ok,
        Resposta::Estado {
            travado: true,
            cofre_existe: true,
            papel: Papel::Agente,
        },
        Resposta::Erro(ErroResposta::Retrocesso),
        Resposta::Erro(ErroResposta::Validacao(vec![Problema {
            campo: "senha".into(),
            posicao: Some(2),
            motivo: "caractere não permitido".into(),
        }])),
        Resposta::Lista(vec![item("clientex-stage-wp"), item("clientex-stage-api")]),
        Resposta::Saida {
            fluxo: Fluxo::Erro,
            dados: zeroize::Zeroizing::new(b"\x00\xffsaida \xe2\x80\xa2\xe2\x80\xa2".to_vec()),
        },
        Resposta::Fim { codigo: 130 },
        Resposta::SessaoPlaywright {
            caminho: "/run/user/1000/ward-teste/playwright/acme-wp-dev.json".into(),
        },
        Resposta::Detalhe(DetalheEntrada::de_entrada(servidor)),
        Resposta::Segredo(seg("falso-revelar-01")),
        Resposta::Token(seg("falso-token-conectar-01")),
        Resposta::PedidoDeAprovacao(PedidoDeAprovacao {
            id: 42,
            entradas: vec!["acme-pg-prod".into(), "acme-ssh-prod".into()],
            ambiente: Ambiente::Prod,
            operacao: Operacao::Executar,
            comando: "psql -c 'select 1'".into(),
            diretorio: "\"/home/usuario/projetos/acme\"".into(),
            cadeia: vec!["claude".into(), "bash".into(), "ward run".into()],
            origem: String::new(),
            confirmacao: false,
            entrada_oculta: true,
        }),
        Resposta::PedidoDeAprovacao(PedidoDeAprovacao {
            id: 43,
            entradas: vec!["acme-wp-prod".into()],
            ambiente: Ambiente::Prod,
            operacao: Operacao::Playwright,
            comando: String::new(),
            diretorio: String::new(),
            cadeia: vec!["claude".into(), "ward playwright".into()],
            origem: "http://localhost:8081".into(),
            confirmacao: true,
            entrada_oculta: false,
        }),
        Resposta::PedidoDeAprovacao(PedidoDeAprovacao {
            id: 44,
            entradas: vec!["acme-wp-dev".into()],
            ambiente: Ambiente::Dev,
            operacao: Operacao::Preencher,
            comando: String::new(),
            diretorio: String::new(),
            cadeia: vec!["chrome".into(), "ward-nm".into()],
            origem: "http://localhost:8081".into(),
            confirmacao: true,
            entrada_oculta: false,
        }),
        Resposta::PedidoDeDestrave(PedidoDeDestrave {
            entrada: "acme-pg-aprovacao".into(),
            operacao: Operacao::Executar,
            comando: r#""psql" "-c" "select 1""#.into(),
            restante: 102,
        }),
        Resposta::CodigoTotp {
            codigo: zeroize::Zeroizing::new("123456".into()),
            restante: 17,
        },
        Resposta::Sugestoes(vec![Sugestao {
            nome: "acme-wp-dev".into(),
            usuario: "wp_admin".into(),
            ambiente: crate::entrada::Ambiente::Dev,
            totp: true,
        }]),
        Resposta::Preenchimento {
            usuario: "wp_admin".into(),
            senha: seg("falso-WP-senha-01"),
            totp: Some(zeroize::Zeroizing::new("654321".into())),
        },
        Resposta::PedidoDeLogin {
            id: "AbCdEfGhIjKlMnOpQrStUvWxYz012345".into(),
            entrada: "acme-wp-prod".into(),
        },
        Resposta::Backups(vec![
            vb("ward-20261109-153000-1.ward", true, true),
            vb("ward-20261109-153000.ward", false, false),
        ]),
        Resposta::Configuracao {
            minutos_trava: 240,
            backup_externo: false,
            backup_local: true,
        },
        Resposta::BackupExterno {
            ligado: true,
            uuid: Some("ABCD-1234".into()),
        },
        Resposta::Pendrives(vec![Pendrive {
            uuid: "ABCD-1234".into(),
            rotulo: "PENDRIVE \"A\"".into(),
            montado: true,
        }]),
    ]
}

/// O papel que pode receber o tipo.
fn papel_da_resposta(t: TipoResposta) -> Papel {
    Papel::TODOS
        .into_iter()
        .find(|p| t.permitida(*p))
        .expect("tipo sem papel")
}

// ---- V3.15: a tabela ----

/// A tabela inteira, como o auditor a lê: (operação, agente, app, extensão). Uma
/// operação nova, ou um papel novo numa operação, tem de mudar este teste.
#[test]
fn v3_15_tabela_operacao_papel() {
    let esperada: &[(&str, bool, bool, bool)] = &[
        ("estado", true, true, true),
        ("destravar", true, true, false),
        ("travar", true, true, false),
        ("lista", true, true, false),
        ("executar", true, false, false),
        ("ssh", true, false, false),
        ("login", true, false, false),
        ("playwright", true, false, false),
        ("entrada_padrao", true, false, false),
        ("janela", true, false, false),
        ("criar_cofre", false, true, false),
        ("detalhe", false, true, false),
        ("revelar", false, true, false),
        ("copiar", false, true, false),
        ("totp", false, true, false),
        ("salvar_entrada", false, true, false),
        ("remover_entrada", false, true, false),
        ("mudar_nivel", false, true, false),
        ("responder_aprovacao", false, true, false),
        ("emitir_token", false, true, false),
        ("trocar_pin", false, true, false),
        ("trocar_senha", false, true, false),
        ("tempo_trava", false, true, false),
        ("backups", false, true, false),
        ("restaurar_backup", false, true, false),
        ("configuracao", false, true, false),
        ("backup_externo", false, true, false),
        ("definir_backup_externo", false, true, false),
        ("pendrives", false, true, false),
        ("sugerir", false, false, true),
        ("preencher", false, false, true),
        ("salvar_login", false, false, true),
        ("aguardar_login", false, false, true),
        ("preencher_login", false, false, true),
        ("resultado_login", false, false, true),
        ("abrir_app", true, true, true),
    ];
    let real: Vec<_> = Operacao::TODAS
        .iter()
        .map(|o| {
            let p = o.papeis();
            (o.nome(), p.agente, p.app, p.extensao)
        })
        .collect();
    assert_eq!(real, esperada);
    // Nenhuma operação de aprovar, responder aprovação ou mudar nível do agente.
    for o in Operacao::TODAS {
        let n = o.nome();
        if n.contains("aprov") || n.contains("nivel") || n.contains("token") {
            assert!(!o.permitida(Papel::Agente), "{n} aceita do agente");
        }
    }

    let respostas: &[(&str, bool, bool, bool, bool)] = &[
        // (tipo, agente, app, extensão, carrega credencial)
        ("ok", true, true, true, false),
        ("estado", true, true, true, false),
        ("erro", true, true, true, false),
        ("lista", true, true, false, false),
        ("saida", true, false, false, false),
        ("fim", true, false, false, false),
        ("sessao_playwright", true, false, false, false),
        ("detalhe", false, true, false, false),
        ("segredo", false, true, false, true),
        ("token", false, true, false, true),
        ("pedido_de_aprovacao", false, true, false, false),
        ("pedido_de_destrave", false, true, false, false),
        ("codigo_totp", false, true, true, true),
        ("sugestoes", false, false, true, false),
        ("preenchimento", false, false, true, true),
        ("pedido_de_login", false, false, true, false),
        ("backups", false, true, false, false),
        ("configuracao", false, true, false, false),
        ("backup_externo", false, true, false, false),
        ("pendrives", false, true, false, false),
    ];
    let real: Vec<_> = TipoResposta::TODAS
        .iter()
        .map(|t| {
            let p = t.papeis();
            (
                t.nome(),
                p.agente,
                p.app,
                p.extensao,
                t.carrega_credencial(),
            )
        })
        .collect();
    assert_eq!(real, respostas);
    for t in TipoResposta::TODAS {
        assert!(
            !(t.carrega_credencial() && t.permitida(Papel::Agente)),
            "{} com credencial para o agente",
            t.nome()
        );
    }
}

/// V3.15 (D8): o protocolo não tem feature, variável de ambiente nem argumento que
/// mude a tabela ou ligue um aprovador no socket.
#[test]
fn v3_15_sem_porta_dos_fundos_no_codigo() {
    let fontes = [
        include_str!("../protocolo.rs"),
        include_str!("json.rs"),
        include_str!("quadro.rs"),
        include_str!("campos.rs"),
        include_str!("pedido.rs"),
        include_str!("resposta.rs"),
        include_str!("erros.rs"),
    ];
    for f in fontes {
        for proibido in [
            "cfg(feature",
            "cfg(debug_assertions",
            "std::env",
            "env::var",
            "env!(",
            "option_env!",
            "modo-teste",
            "modo_teste",
        ] {
            assert!(!f.contains(proibido), "achou {proibido:?} no protocolo");
        }
    }
}

// ---- V3.3: nada de credencial para o agente ----

/// Toda operação de app e de extensão pedida pelo agente responde "não autorizado",
/// com os dados certos, sem dados, com campos inventados no topo e nos dados, e com
/// dados malformados.
#[test]
fn v3_3_operacoes_de_app_e_extensao_do_agente_sao_nao_autorizadas() {
    let mut testadas = 0;
    for &op in Operacao::TODAS {
        for papel in Papel::TODOS {
            if op.permitida(papel) {
                continue;
            }
            let validos = dados_validos(op);
            let comextra = com_campo(&validos, r#""revelar":true,"campo":"senha""#);
            let quadros = [
                quadro_de(op, &validos),
                format!(r#"{{"v":1,"op":"{}"}}"#, op.nome()),
                quadro_de(op, &comextra),
                format!(
                    r#"{{"v":1,"op":"{}","revelar":true,"campo":"senha","dados":{validos}}}"#,
                    op.nome()
                ),
                quadro_de(op, "5"),
                quadro_de(op, r#"{"geracao":1}"#),
            ];
            for q in &quadros {
                assert_eq!(
                    ped(papel, q).unwrap_err(),
                    ErroResposta::NaoAutorizado,
                    "{} pelo {}: {q}",
                    op.nome(),
                    papel.nome()
                );
                testadas += 1;
            }
        }
    }
    assert!(testadas >= 6 * 16);
}

/// As operações do agente com campos inventados são recusadas inteiras (leitura
/// estrita): `"revelar": true` ou `"campo": "senha"` não viram nada.
#[test]
fn v3_3_campos_inventados_recusam_o_pedido() {
    for &op in Operacao::TODAS {
        let papel = papel_de(op);
        let validos = dados_validos(op);
        assert!(
            ped(papel, &quadro_de(op, &validos)).is_ok(),
            "{}",
            op.nome()
        );
        for extra in [
            r#""revelar":true"#,
            r#""campo":"senha""#,
            r#""valor":"x""#,
            r#""nivel":"livre""#,
            r#""papel":"app""#,
        ] {
            // Revelar e copiar têm "campo" de verdade: com ele repetido, a chave
            // duplicada recusa também.
            let d = com_campo(&validos, extra);
            assert_eq!(
                ped(papel, &quadro_de(op, &d)).unwrap_err(),
                ErroResposta::PedidoInvalido,
                "{} com {extra}",
                op.nome()
            );
        }
        let topo = format!(
            r#"{{"v":1,"op":"{}","revelar":true,"dados":{validos}}}"#,
            op.nome()
        );
        assert_eq!(ped(papel, &topo).unwrap_err(), ErroResposta::PedidoInvalido);
    }
}

/// D39: nenhuma operação aceita geração vinda do cliente, nem no topo nem nos dados,
/// e nenhuma resposta leva a geração.
#[test]
fn d39_geracao_nunca_vem_do_cliente_nem_vai_para_ele() {
    for &op in Operacao::TODAS {
        let papel = papel_de(op);
        let validos = dados_validos(op);
        let nos_dados = com_campo(&validos, r#""geracao":1"#);
        assert_eq!(
            ped(papel, &quadro_de(op, &nos_dados)).unwrap_err(),
            ErroResposta::PedidoInvalido
        );
        let no_topo = format!(
            r#"{{"v":1,"op":"{}","geracao":99,"dados":{validos}}}"#,
            op.nome()
        );
        assert_eq!(
            ped(papel, &no_topo).unwrap_err(),
            ErroResposta::PedidoInvalido
        );
    }
    let rascunho =
        dados_validos(Operacao::SalvarEntrada).replacen(r#""nome":"#, r#""geracao":2,"nome":"#, 1);
    assert_eq!(
        ped(Papel::App, &quadro_de(Operacao::SalvarEntrada, &rascunho)).unwrap_err(),
        ErroResposta::PedidoInvalido
    );
    for r in uma_resposta_de_cada() {
        let papel = papel_da_resposta(r.tipo());
        let q = codificar_resposta(papel, &r).unwrap();
        let s = String::from_utf8_lossy(&q);
        assert!(!s.contains("\"geracao\"") && !s.contains("geração"), "{s}");
    }
}

/// S1: toda resposta que leva credencial é recusada para o agente, na escrita (daemon)
/// e na leitura (CLI), e o quadro do app não passa pela leitura do agente.
#[test]
fn s1_resposta_com_credencial_nunca_vai_ao_agente() {
    let mut vistas = 0;
    for r in uma_resposta_de_cada() {
        let t = r.tipo();
        for papel in Papel::TODOS {
            // A lista do exemplo é a do agente (sem nível); a do app, com nível, está no
            // `d50_lista_do_agente_sem_nivel_e_do_app_com_nivel`.
            if t == TipoResposta::Lista && papel == Papel::App {
                continue;
            }
            // O `estado` leva o papel de quem recebe (D63).
            let proprio;
            let r = match &r {
                Resposta::Estado {
                    travado,
                    cofre_existe,
                    ..
                } => {
                    proprio = Resposta::Estado {
                        travado: *travado,
                        cofre_existe: *cofre_existe,
                        papel,
                    };
                    &proprio
                }
                outra => outra,
            };
            let enc = codificar_resposta(papel, r);
            if t.permitida(papel) {
                let q = enc.unwrap();
                assert!(decodificar_resposta(papel, &q).is_ok());
                for outro in Papel::TODOS {
                    if !t.permitida(outro) {
                        assert_eq!(
                            decodificar_resposta(outro, &q).unwrap_err(),
                            RespostaInvalida
                        );
                    }
                }
            } else {
                assert_eq!(enc.unwrap_err(), ErroCodificar::NaoPermitidaAoPapel);
            }
        }
        if t.carrega_credencial() {
            assert_eq!(
                codificar_resposta(Papel::Agente, &r).unwrap_err(),
                ErroCodificar::NaoPermitidaAoPapel
            );
            vistas += 1;
        }
    }
    assert_eq!(vistas, 4); // segredo, token, código TOTP, preenchimento
}

/// V3.38 (D63): o `estado` traz o papel da conexão e nada mais de novo; o daemon não
/// consegue mandar a um papel o `estado` de outro, e o cliente lê o papel que veio.
#[test]
fn v3_38_estado_traz_o_papel_e_so_ele() {
    for papel in Papel::TODOS {
        let r = Resposta::Estado {
            travado: false,
            cofre_existe: true,
            papel,
        };
        let q = codificar_resposta(papel, &r).unwrap();
        assert_eq!(
            String::from_utf8(q.to_vec()).unwrap(),
            format!(
                r#"{{"v":1,"tipo":"estado","dados":{{"travado":false,"cofre_existe":true,"papel":"{}"}}}}"#,
                papel.nome()
            )
        );
        match decodificar_resposta(papel, &q).unwrap() {
            Resposta::Estado { papel: lido, .. } => assert_eq!(lido, papel),
            outra => panic!("{outra:?}"),
        }
        for outro in Papel::TODOS {
            if outro != papel {
                assert_eq!(
                    codificar_resposta(outro, &r).unwrap_err(),
                    ErroCodificar::Invalida
                );
                // O app que recebe `agente` lê como veio (e sai, Fase 23a).
                let q = codificar_resposta(
                    outro,
                    &Resposta::Estado {
                        travado: false,
                        cofre_existe: true,
                        papel: outro,
                    },
                )
                .unwrap();
                assert!(matches!(
                    decodificar_resposta(papel, &q),
                    Ok(Resposta::Estado { papel: p, .. }) if p == outro
                ));
            }
        }
    }
    // Sem o papel, com outro nome ou com campo a mais, o cliente recusa.
    for dados in [
        r#"{"travado":false,"cofre_existe":true}"#,
        r#"{"travado":false,"cofre_existe":true,"papel":"root"}"#,
        r#"{"travado":false,"cofre_existe":true,"papel":"app","uid":0}"#,
    ] {
        let q = format!(r#"{{"v":1,"tipo":"estado","dados":{dados}}}"#);
        assert!(
            decodificar_resposta(Papel::App, q.as_bytes()).is_err(),
            "{dados}"
        );
    }
}

/// Os valores de um segredo como podem aparecer num quadro: crus, em base64 e com o
/// escape do JSON.
fn formas(segredo: &[u8]) -> Vec<Vec<u8>> {
    let mut v = vec![segredo.to_vec(), b64(segredo).into_bytes()];
    if let Ok(s) = core::str::from_utf8(segredo) {
        let esc = s
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n");
        v.push(esc.into_bytes());
        // Cada linha de 6+ caracteres (chave privada).
        for l in s.lines().filter(|l| l.len() >= 6) {
            v.push(l.as_bytes().to_vec());
        }
    }
    v
}

fn contem(palheiro: &[u8], agulha: &[u8]) -> bool {
    agulha.len() <= palheiro.len() && palheiro.windows(agulha.len()).any(|w| w == agulha)
}

/// V3.3: a lista de todas as entradas do cofre de teste (as 22, inclusive as
/// bloqueadas, que o daemon ainda filtraria) para o agente e o detalhe de cada uma
/// para o app não têm segredo nenhum, em nenhuma forma.
#[test]
fn v3_3_lista_e_detalhe_do_cofre_de_teste_sem_segredo() {
    let entradas = cofre_de_teste();
    assert_eq!(entradas.len(), 22);
    let lista = Resposta::Lista(
        entradas
            .iter()
            .map(|e| ItemLista::de_entrada(e, Papel::Agente))
            .collect(),
    );
    let q_lista = codificar_resposta(Papel::Agente, &lista).unwrap();
    let detalhes: Vec<_> = entradas
        .iter()
        .map(|e| {
            codificar_resposta(
                Papel::App,
                &Resposta::Detalhe(DetalheEntrada::de_entrada(e)),
            )
            .unwrap()
        })
        .collect();
    let mut agulhas = 0;
    for e in &entradas {
        for s in e.segredos() {
            for f in formas(s.valor.expor()) {
                if f.len() < 6 {
                    continue;
                }
                agulhas += 1;
                assert!(!contem(&q_lista, &f), "segredo de {} na lista", e.nome());
                for d in &detalhes {
                    assert!(!contem(d, &f), "segredo de {} num detalhe", e.nome());
                }
            }
        }
    }
    assert!(agulhas > 40);
    // E a lista volta inteira pela leitura da CLI.
    let Resposta::Lista(itens) = decodificar_resposta(Papel::Agente, &q_lista).unwrap() else {
        panic!("não é lista")
    };
    assert_eq!(itens.len(), 22);
    assert!(itens.iter().any(|i| i.nome == "gama-bloqueada"));
}

/// O `Debug` de pedidos e respostas não mostra segredo.
#[test]
fn debug_sem_segredo() {
    let senha = "falso-mestra-debug-01";
    let p = ped(
        Papel::Agente,
        &quadro_de(
            Operacao::Destravar,
            &format!(r#"{{"senha":"{}"}}"#, b64(senha.as_bytes())),
        ),
    )
    .unwrap();
    let d = format!("{p:?}");
    assert!(
        !d.contains(senha) && !d.contains(&b64(senha.as_bytes())),
        "{d}"
    );
    for r in uma_resposta_de_cada() {
        let d = format!("{r:?}");
        for s in [
            "falso-revelar-01",
            "falso-token-conectar-01",
            "falso-WP-senha-01",
        ] {
            assert!(!d.contains(s), "{d}");
        }
    }
}

// ---- Ida e volta ----

#[test]
fn ida_e_volta_de_todo_pedido_e_resposta() {
    for p in um_pedido_de_cada() {
        let papel = papel_de(p.operacao());
        let q = codificar_pedido(papel, &p).unwrap();
        let de_novo = decodificar_pedido(papel, &q).unwrap();
        assert_eq!(*codificar_pedido(papel, &de_novo).unwrap(), *q);
        // O cliente não codifica pedido de outro papel.
        for outro in Papel::TODOS {
            if !p.operacao().permitida(outro) {
                assert_eq!(
                    codificar_pedido(outro, &p).unwrap_err(),
                    ErroCodificar::NaoPermitidaAoPapel
                );
            }
        }
    }
    for r in uma_resposta_de_cada() {
        let papel = papel_da_resposta(r.tipo());
        let q = codificar_resposta(papel, &r).unwrap();
        let de_novo = decodificar_resposta(papel, &q).unwrap();
        assert_eq!(*codificar_resposta(papel, &de_novo).unwrap(), *q);
    }
}

#[test]
fn formato_conhecido_do_quadro() {
    let q = codificar_pedido(Papel::Agente, &Pedido::Lista).unwrap();
    assert_eq!(&q[..], br#"{"v":1,"op":"lista","dados":{}}"#);
    let q =
        codificar_resposta(Papel::Agente, &Resposta::Erro(ErroResposta::NaoAutorizado)).unwrap();
    assert_eq!(
        &q[..],
        r#"{"v":1,"tipo":"erro","dados":{"codigo":"nao_autorizado","mensagem":"não autorizado"}}"#
            .as_bytes()
    );
    let mut saida = Vec::new();
    quadro::escrever_quadro(&mut saida, &q, MAX_QUADRO_COMUM).unwrap();
    assert_eq!(&saida[..4], &(q.len() as u32).to_be_bytes());
    assert_eq!(&saida[4..], &q[..]);
    // `dados` ausente vale vazio.
    assert!(matches!(
        ped(Papel::Agente, r#"{"op":"lista","v":1}"#),
        Ok(Pedido::Lista)
    ));
}

// ---- Erros do envelope ----

#[test]
fn envelope_recusado() {
    let casos: &[(&str, ErroResposta)] = &[
        ("", ErroResposta::PedidoInvalido),
        ("nada", ErroResposta::PedidoInvalido),
        ("[]", ErroResposta::PedidoInvalido),
        (r#"{"op":"lista"}"#, ErroResposta::PedidoInvalido),
        (r#"{"v":"1","op":"lista"}"#, ErroResposta::PedidoInvalido),
        (r#"{"v":2,"op":"lista"}"#, ErroResposta::VersaoDesconhecida),
        (r#"{"v":0,"op":"lista"}"#, ErroResposta::VersaoDesconhecida),
        (r#"{"v":1}"#, ErroResposta::PedidoInvalido),
        (
            r#"{"v":1,"op":"apagar_tudo"}"#,
            ErroResposta::OperacaoDesconhecida,
        ),
        (
            r#"{"v":1,"op":"Lista"}"#,
            ErroResposta::OperacaoDesconhecida,
        ),
        (
            r#"{"v":1,"op":"lista","dados":[]}"#,
            ErroResposta::PedidoInvalido,
        ),
        (
            r#"{"v":1,"op":"lista","op":"lista"}"#,
            ErroResposta::PedidoInvalido,
        ),
        (
            r#"{"v":1,"op":"lista","op":"revelar"}"#,
            ErroResposta::PedidoInvalido,
        ),
        (
            r#"{"v":1,"v":1,"op":"lista"}"#,
            ErroResposta::PedidoInvalido,
        ),
        (
            r#"{"v":1,"op":"lista","dados":{},"dados":{}}"#,
            ErroResposta::PedidoInvalido,
        ),
        (r#"{"v":1,"op":"lista"} {}"#, ErroResposta::PedidoInvalido),
        (
            r#"{"v":1,"op":"lista","dados":{"x":1}}"#,
            ErroResposta::PedidoInvalido,
        ),
    ];
    for (q, e) in casos {
        assert_eq!(ped(Papel::Agente, q).unwrap_err(), *e, "{q}");
    }
}

// ---- Formato de cada operação ----

#[test]
fn pin_e_decisao_da_aprovacao() {
    let q = |d: &str| ped(Papel::App, &quadro_de(Operacao::ResponderAprovacao, d));
    let pin = b64(b"135790");
    assert!(q(r#"{"pedido":1,"decisao":"negar"}"#).is_ok());
    assert!(
        q(&format!(
            r#"{{"pedido":1,"decisao":"negar","pin":"{pin}"}}"#
        ))
        .is_err()
    );
    for d in ["uma_vez", "uma_hora", "sempre"] {
        assert!(q(&format!(r#"{{"pedido":1,"decisao":"{d}","pin":"{pin}"}}"#)).is_ok());
        assert!(
            q(&format!(r#"{{"pedido":1,"decisao":"{d}"}}"#)).is_err(),
            "{d} sem PIN"
        );
    }
    for ruim in ["123", "1234567", "12a4", "١٢٣٤", "12 34", ""] {
        let d = format!(
            r#"{{"pedido":1,"decisao":"uma_vez","pin":"{}"}}"#,
            b64(ruim.as_bytes())
        );
        assert!(q(&d).is_err(), "PIN {ruim:?}");
        let m = format!(
            r#"{{"entrada":"acme-pg-dev","nivel":"livre","pin":"{}"}}"#,
            b64(ruim.as_bytes())
        );
        assert!(ped(Papel::App, &quadro_de(Operacao::MudarNivel, &m)).is_err());
    }
    // Fase 44c: "confirmar" não leva PIN (quem confere que o pedido é uma confirmação
    // é o daemon).
    assert!(q(r#"{"pedido":1,"decisao":"confirmar"}"#).is_ok());
    assert!(
        q(&format!(
            r#"{{"pedido":1,"decisao":"confirmar","pin":"{pin}"}}"#
        ))
        .is_err()
    );
    // PIN em texto, sem base64, é recusado.
    assert!(q(r#"{"pedido":1,"decisao":"uma_vez","pin":"135790"}"#).is_err());
    assert!(q(r#"{"pedido":1,"decisao":"admin","pin":"MTM1Nzkw"}"#).is_err());
    assert!(
        ped(
            Papel::App,
            &quadro_de(
                Operacao::MudarNivel,
                &format!(r#"{{"entrada":"acme-pg-dev","nivel":"admin","pin":"{pin}"}}"#)
            )
        )
        .is_err()
    );
}

#[test]
fn executar_recusa_nul_igual_e_vazio() {
    let q = |argv: &[&[u8]], cwd: &[u8], amb: &[(&[u8], &[u8])]| {
        let argv: Vec<_> = argv.iter().map(|a| format!("\"{}\"", b64(a))).collect();
        let amb: Vec<_> = amb
            .iter()
            .map(|(n, v)| format!(r#"{{"nome":"{}","valor":"{}"}}"#, b64(n), b64(v)))
            .collect();
        ped(
            Papel::Agente,
            &quadro_de(
                Operacao::Executar,
                &format!(
                    r#"{{"entrada":"acme-pg-dev","argv":[{}],"cwd":"{}","tipo_entrada":"pipe","ambiente":[{}]}}"#,
                    argv.join(","),
                    b64(cwd),
                    amb.join(",")
                ),
            ),
        )
    };
    assert!(q(&[b"ls", b"\xff\xfe"], b"/tmp", &[(b"A", b"\xff")]).is_ok());
    assert!(q(&[], b"/tmp", &[]).is_err());
    assert!(q(&[b"l\0s"], b"/tmp", &[]).is_err());
    assert!(q(&[b"ls"], b"", &[]).is_err());
    assert!(q(&[b"ls"], b"/t\0mp", &[]).is_err());
    assert!(q(&[b"ls"], b"/tmp", &[(b"A=B", b"c")]).is_err());
    assert!(q(&[b"ls"], b"/tmp", &[(b"", b"c")]).is_err());
    assert!(q(&[b"ls"], b"/tmp", &[(b"A", b"c\0")]).is_err());
    let grande = vec![b'a'; crate::limites::MAX_ARGUMENTO + 1];
    assert!(q(&[&grande], b"/tmp", &[]).is_err());
}

#[test]
fn revelar_exige_um_alvo_conhecido() {
    let q = |d: &str| ped(Papel::App, &quadro_de(Operacao::Revelar, d));
    assert!(q(r#"{"entrada":"acme-wp-dev","campo":"senha"}"#).is_ok());
    assert!(q(r#"{"entrada":"acme-wp-dev","extra":"PIN_BANCO"}"#).is_ok());
    assert!(q(r#"{"entrada":"acme-wp-dev"}"#).is_err());
    assert!(q(r#"{"entrada":"acme-wp-dev","campo":"senha","extra":"X"}"#).is_err());
    assert!(q(r#"{"entrada":"acme-wp-dev","campo":"usuario"}"#).is_err());
    assert!(q(r#"{"entrada":"","campo":"senha"}"#).is_err());
}

/// S9: a entrada que chega do app não tem nível, e campo de outro tipo recusa.
#[test]
fn s9_rascunho_sem_nivel_e_sem_campo_de_outro_tipo() {
    let base = dados_validos(Operacao::SalvarEntrada);
    let q = |d: &str| ped(Papel::App, &quadro_de(Operacao::SalvarEntrada, d));
    assert!(q(&base).is_ok());
    let com_nivel = base.replacen(r#""nome":"#, r#""nivel":"livre","nome":"#, 1);
    assert_eq!(q(&com_nivel).unwrap_err(), ErroResposta::PedidoInvalido);
    let nivel_nos_campos = base.replacen(r#""url":"#, r#""nivel":"livre","url":"#, 1);
    assert_eq!(
        q(&nivel_nos_campos).unwrap_err(),
        ErroResposta::PedidoInvalido
    );
    let de_outro_tipo = base.replacen(r#""url":"#, r#""host":"x","url":"#, 1);
    assert_eq!(q(&de_outro_tipo).unwrap_err(), ErroResposta::PedidoInvalido);
    // Segredo em texto, sem base64, recusa.
    let em_texto = base.replace(&b64(b"falso-clientex-01"), "falso-clientex-01");
    assert_eq!(q(&em_texto).unwrap_err(), ErroResposta::PedidoInvalido);
    // A extensão só salva `login`.
    let api = format!(
        r#"{{"entrada":{{"nome":"x-api","grupo":"x","ambiente":"dev","tipo":"api","campos":{{"token":"{}"}}}}}}"#,
        b64(b"falso-token-01")
    );
    assert!(ped(Papel::Extensao, &quadro_de(Operacao::SalvarLogin, &api)).is_err());
    assert!(ped(Papel::App, &quadro_de(Operacao::SalvarEntrada, &api)).is_ok());
}

// ---- D29: anexos pelo app ----

fn rascunho_com_anexos(tamanhos: &[usize]) -> Rascunho {
    Rascunho {
        nome: "clientex-stage-vpn".into(),
        grupo: "clientex".into(),
        ambiente: "stage".into(),
        projeto: String::new(),
        tags: Vec::new(),
        dados: RascunhoDados::Generico,
        extras: vec![RascunhoExtra {
            nome: "usuario".into(),
            valor: seg("vpn-user"),
            segredo: false,
        }],
        anexos: tamanhos
            .iter()
            .enumerate()
            .map(|(i, &n)| RascunhoAnexo {
                nome: format!("cliente-{i}.ovpn"),
                conteudo: Segredo::de_bytes(cofre_teste::bytes_0_a_255(n)),
            })
            .collect(),
    }
}

/// D29: o app manda anexos que somam até 256 KiB; um byte a mais recusa. O agente não
/// tem a operação.
#[test]
fn d29_anexos_ate_256_kib_somados_pelo_app() {
    let no_limite = Pedido::SalvarEntrada {
        anterior: None,
        rascunho: rascunho_com_anexos(&[MAX_ANEXOS_SOMADOS - 10 * KIB, 10 * KIB]),
        manter_anexos: Vec::new(),
    };
    let q = codificar_pedido(Papel::App, &no_limite).unwrap();
    assert!(q.len() > MAX_ANEXOS_SOMADOS && q.len() < MAX_QUADRO_APP);
    let Pedido::SalvarEntrada { rascunho, .. } = decodificar_pedido(Papel::App, &q).unwrap() else {
        panic!()
    };
    assert_eq!(
        rascunho.anexos[0].conteudo.expor(),
        &cofre_teste::bytes_0_a_255(MAX_ANEXOS_SOMADOS - 10 * KIB)[..]
    );
    rascunho.validar().expect("entrada com anexos no limite");
    assert_eq!(
        decodificar_pedido(Papel::Agente, &q).unwrap_err(),
        ErroResposta::NaoAutorizado
    );

    // Um byte acima da soma: o cliente não codifica e o daemon não aceita.
    let acima = Pedido::SalvarEntrada {
        anterior: Some("clientex-stage-vpn".into()),
        rascunho: rascunho_com_anexos(&[MAX_ANEXOS_SOMADOS - 10 * KIB, 10 * KIB + 1]),
        manter_anexos: Vec::new(),
    };
    assert_eq!(
        codificar_pedido(Papel::App, &acima).unwrap_err(),
        ErroCodificar::Invalida
    );
    let forjado = String::from_utf8(q.to_vec()).unwrap().replace(
        &b64(&cofre_teste::bytes_0_a_255(10 * KIB)),
        &b64(&cofre_teste::bytes_0_a_255(10 * KIB + 1)),
    );
    assert_eq!(
        ped(Papel::App, &forjado).unwrap_err(),
        ErroResposta::PedidoInvalido
    );
}

// ---- Fase 39b: anexos mantidos na edição (V5.12) ----

/// `manter_anexos` vai e volta; ausente quando vazio (o quadro de hoje não muda).
#[test]
fn fase39b_manter_anexos_vai_e_volta() {
    let p = Pedido::SalvarEntrada {
        anterior: Some("clientex-stage-vpn".into()),
        rascunho: rascunho_com_anexos(&[10]),
        manter_anexos: vec!["cliente-0.ovpn".into(), "app.env".into()],
    };
    let q = codificar_pedido(Papel::App, &p).unwrap();
    let Pedido::SalvarEntrada { manter_anexos, .. } = decodificar_pedido(Papel::App, &q).unwrap()
    else {
        panic!()
    };
    assert_eq!(manter_anexos, ["cliente-0.ovpn", "app.env"]);
    let sem = Pedido::SalvarEntrada {
        anterior: Some("clientex-stage-vpn".into()),
        rascunho: rascunho_com_anexos(&[10]),
        manter_anexos: Vec::new(),
    };
    let q = codificar_pedido(Papel::App, &sem).unwrap();
    assert!(!String::from_utf8_lossy(&q).contains("manter_anexos"));
    assert_eq!(
        decodificar_pedido(Papel::Agente, &q).unwrap_err(),
        ErroResposta::NaoAutorizado
    );
}

/// Ataques ao `manter_anexos`: numa entrada nova, nome vazio, repetido, longo demais,
/// fora de lista de textos, e mantidos mais novos acima de `MAX_ANEXOS`.
#[test]
fn fase39b_ataque_manter_anexos_forjado() {
    let base = dados_validos(Operacao::SalvarEntrada);
    let q = |d: &str| ped(Papel::App, &quadro_de(Operacao::SalvarEntrada, d));
    let editar = |campo: &str| {
        base.replacen(
            '{',
            &format!(r#"{{"anterior":"clientex-stage-wp",{campo},"#),
            1,
        )
    };
    assert!(q(&editar(r#""manter_anexos":["app.env"]"#)).is_ok());
    assert!(q(&editar(r#""manter_anexos":[]"#)).is_ok());
    // Sem `anterior`: não há de onde manter.
    let novo = base.replacen('{', r#"{"manter_anexos":["app.env"],"#, 1);
    assert_eq!(q(&novo).unwrap_err(), ErroResposta::PedidoInvalido);
    let longo = format!(r#""manter_anexos":["{}"]"#, "a".repeat(MAX_NOME_ANEXO + 1));
    let muitos = format!(
        r#""manter_anexos":[{}]"#,
        (0..=MAX_ANEXOS)
            .map(|i| format!(r#""a{i}.env""#))
            .collect::<Vec<_>>()
            .join(",")
    );
    for forjado in [
        r#""manter_anexos":[""]"#.to_owned(),
        r#""manter_anexos":["app.env","app.env"]"#.to_owned(),
        r#""manter_anexos":"app.env""#.to_owned(),
        r#""manter_anexos":[{"nome":"app.env","conteudo":"QQ=="}]"#.to_owned(),
        r#""manter_anexos":[1]"#.to_owned(),
        longo,
        muitos,
    ] {
        assert_eq!(
            q(&editar(&forjado)).unwrap_err(),
            ErroResposta::PedidoInvalido,
            "{forjado}"
        );
    }
    // Mantidos mais novos acima de `MAX_ANEXOS`: o cliente não codifica.
    let p = Pedido::SalvarEntrada {
        anterior: Some("clientex-stage-vpn".into()),
        rascunho: rascunho_com_anexos(&[1; MAX_ANEXOS]),
        manter_anexos: vec!["app.env".into()],
    };
    assert_eq!(
        codificar_pedido(Papel::App, &p).unwrap_err(),
        ErroCodificar::Invalida
    );
}

/// `juntar_anexos_mantidos`: copia o conteúdo da entrada editada à frente dos novos;
/// nome que a entrada não tem (inclusive com `/` ou `..`) ou repetido é pedido inválido
/// e o rascunho não muda; a soma de D29 continua valendo pela validação inteira.
#[test]
fn fase39b_ataque_juntar_anexos_mantidos() {
    let antiga = rascunho_com_anexos(&[200 * KIB, 10]).validar().unwrap();
    let mut r = rascunho_com_anexos(&[5]);
    r.anexos[0].nome = "novo.env".into();
    for nomes in [
        vec!["nao-existe.ovpn".to_owned()],
        vec!["cliente-0.ovpn".to_owned(), "cliente-0.ovpn".to_owned()],
        vec!["../cliente-0.ovpn".to_owned()],
        vec!["CLIENTE-0.OVPN".to_owned()],
    ] {
        assert_eq!(
            juntar_anexos_mantidos(&mut r, &antiga, &nomes).unwrap_err(),
            ErroResposta::PedidoInvalido,
            "{nomes:?}"
        );
        assert_eq!(r.anexos.len(), 1);
    }
    juntar_anexos_mantidos(&mut r, &antiga, &["cliente-1.ovpn".to_owned()]).unwrap();
    let nomes: Vec<&str> = r.anexos.iter().map(|a| a.nome.as_str()).collect();
    assert_eq!(nomes, ["cliente-1.ovpn", "novo.env"]);
    assert_eq!(
        r.anexos[0].conteudo.expor(),
        antiga.anexos()[1].conteudo().expor()
    );
    let e = r.validar().unwrap();
    assert_eq!(e.anexos().len(), 2);

    // Mantido de 200 KiB + novo de 60 KiB passa da soma: a validação recusa.
    let mut r = rascunho_com_anexos(&[60 * KIB]);
    r.anexos[0].nome = "novo.env".into();
    juntar_anexos_mantidos(&mut r, &antiga, &["cliente-0.ovpn".to_owned()]).unwrap();
    assert!(r.validar().is_err());

    // Nome mantido igual ao de um anexo novo: a validação acusa a repetição.
    let mut r = rascunho_com_anexos(&[5]);
    juntar_anexos_mantidos(&mut r, &antiga, &["cliente-0.ovpn".to_owned()]).unwrap();
    assert!(r.validar().is_err());
}

/// A maior entrada que a validação aceita (nota de 1 MiB, 64 extras de 64 KiB, anexos
/// de 256 KiB) cabe no quadro do app, vai e volta e valida; o quadro passa do limite do
/// cliente comum.
#[test]
fn maior_entrada_cabe_no_quadro_do_app() {
    let r = Rascunho {
        nome: "clientex-prod-nota".into(),
        grupo: "clientex".into(),
        ambiente: "prod".into(),
        projeto: "portal".into(),
        tags: (0..16).map(|i| format!("tag-{i:02}")).collect(),
        dados: RascunhoDados::NotaSegura {
            texto: Some(Segredo::de_bytes(vec![b'"'; MAX_NOTA])),
        },
        extras: (0..MAX_EXTRAS)
            .map(|i| RascunhoExtra {
                nome: format!("EXTRA_{i:02}"),
                valor: Segredo::de_bytes(vec![b'x'; MAX_VALOR_EXTRA]),
                segredo: i % 2 == 0,
            })
            .collect(),
        anexos: vec![RascunhoAnexo {
            nome: "tudo.bin".into(),
            conteudo: Segredo::de_bytes(cofre_teste::bytes_0_a_255(MAX_ANEXOS_SOMADOS)),
        }],
    };
    let p = Pedido::SalvarEntrada {
        anterior: None,
        rascunho: r,
        manter_anexos: Vec::new(),
    };
    let q = codificar_pedido(Papel::App, &p).unwrap();
    assert!(q.len() > MAX_QUADRO_COMUM, "{}", q.len());
    assert!(q.len() <= MAX_QUADRO_APP, "{}", q.len());
    let Pedido::SalvarEntrada { rascunho, .. } = decodificar_pedido(Papel::App, &q).unwrap() else {
        panic!()
    };
    let e = rascunho.validar().expect("maior entrada válida");
    assert_eq!(e.extras().len(), MAX_EXTRAS);
    // O mesmo quadro, pelo leitor de quadros: cabe no do app e não no comum.
    let mut bruto = Vec::new();
    quadro::escrever_quadro(&mut bruto, &q, MAX_QUADRO_APP).unwrap();
    assert_eq!(
        ler_quadro(&mut &bruto[..], limite_do_quadro(Papel::Agente)).unwrap_err(),
        ErroQuadro::GrandeDemais
    );
    assert_eq!(
        ler_quadro(&mut &bruto[..], limite_do_quadro(Papel::Extensao)).unwrap_err(),
        ErroQuadro::GrandeDemais
    );
    assert_eq!(
        *ler_quadro(&mut &bruto[..], limite_do_quadro(Papel::App)).unwrap(),
        *q
    );
}

// ---- V3.4: quadros ----

/// Leitor que conta quantos bytes do corpo foram pedidos.
struct Contador<'a> {
    dados: &'a [u8],
    lidos: usize,
}

impl Read for Contador<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = buf.len().min(self.dados.len() - self.lidos);
        buf[..n].copy_from_slice(&self.dados[self.lidos..self.lidos + n]);
        self.lidos += n;
        Ok(n)
    }
}

#[test]
fn v3_4_quadros_recusados_antes_de_ler_o_corpo() {
    let mut corpo = Vec::new();
    for (cab, erro) in [
        ((2 * MIB) as u32, ErroQuadro::GrandeDemais),
        ((MAX_QUADRO_COMUM + 1) as u32, ErroQuadro::GrandeDemais),
        (u32::MAX, ErroQuadro::GrandeDemais), // o -1 de 32 bits
        ((-2i32) as u32, ErroQuadro::GrandeDemais),
        (0, ErroQuadro::Vazio),
    ] {
        corpo.clear();
        corpo.extend_from_slice(&cab.to_be_bytes());
        corpo.extend_from_slice(&[b'{'; 64]);
        let mut r = Contador {
            dados: &corpo,
            lidos: 0,
        };
        assert_eq!(ler_quadro(&mut r, MAX_QUADRO_COMUM).unwrap_err(), erro);
        assert_eq!(r.lidos, 4, "leu o corpo de um quadro recusado");
    }
    // 2 MiB também passa do quadro do cliente comum pelos dois papéis.
    assert!(2 * MIB > limite_do_quadro(Papel::Agente));
    assert!(2 * MIB > limite_do_quadro(Papel::Extensao));
    // No limite exato passa.
    let mut q = (MAX_QUADRO_COMUM as u32).to_be_bytes().to_vec();
    q.extend(std::iter::repeat_n(b' ', MAX_QUADRO_COMUM));
    assert_eq!(
        ler_quadro(&mut &q[..], MAX_QUADRO_COMUM).unwrap().len(),
        MAX_QUADRO_COMUM
    );
}

#[test]
fn v3_4_quadro_truncado_e_fechado() {
    assert_eq!(
        ler_quadro(&mut &b""[..], 100).unwrap_err(),
        ErroQuadro::Fechado
    );
    assert_eq!(
        ler_quadro(&mut &b"\0\0"[..], 100).unwrap_err(),
        ErroQuadro::Truncado
    );
    assert_eq!(
        ler_quadro(&mut &b"\0\0\0\x0a{}"[..], 100).unwrap_err(),
        ErroQuadro::Truncado
    );
    // Declarou 1 MiB e mandou 10 bytes: truncado, sem esperar o resto.
    let mut q = (MAX_QUADRO_COMUM as u32).to_be_bytes().to_vec();
    q.extend_from_slice(b"0123456789");
    assert_eq!(
        ler_quadro(&mut &q[..], MAX_QUADRO_COMUM).unwrap_err(),
        ErroQuadro::Truncado
    );
    // Escrever também recusa vazio e acima do limite, sem escrever nada.
    let mut saida = Vec::new();
    assert_eq!(
        quadro::escrever_quadro(&mut saida, b"", 10).unwrap_err(),
        ErroQuadro::Vazio
    );
    assert_eq!(
        quadro::escrever_quadro(&mut saida, &[b' '; 11], 10).unwrap_err(),
        ErroQuadro::GrandeDemais
    );
    assert!(saida.is_empty());
}

/// V3.4: JSON inválido e operação desconhecida viram resposta de erro, sem pânico; a
/// resposta é um quadro válido para o cliente comum.
#[test]
fn v3_4_json_invalido_e_operacao_desconhecida_respondem_erro() {
    for q in [
        &b"\xff\xfe{}"[..],
        b"{\"v\":1,\"op\":\"lista\"",
        b"{\"v\":1,\"op\":\"\xc3\x28\"}",
        b"{\"v\":-1,\"op\":\"lista\"}",
        b"{\"v\":1,\"op\":\"desconhecida\"}",
        b"\xef\xbb\xbf{\"v\":1,\"op\":\"lista\"}",
    ] {
        let e = decodificar_pedido(Papel::Agente, q).unwrap_err();
        let r = codificar_resposta(Papel::Agente, &Resposta::Erro(e)).unwrap();
        assert!(decodificar_resposta(Papel::Agente, &r).is_ok());
    }
    assert_eq!(
        erro_do_quadro(ErroQuadro::GrandeDemais),
        ErroResposta::QuadroInvalido
    );
}

// ---- JSON estrito ----

#[test]
fn json_estrito() {
    let aceitos = [
        r#"{}"#,
        r#" { "a" : [ 1 , 0 , 18446744073709551615 ] , "b" : { } } "#,
        r#"{"a":"\" \\ \/ \b \f \n \r \t \u00e7 \ud83d\ude00"}"#,
        r#"{"a":true,"b":false,"c":null,"d":[]}"#,
        "{\"a\":\"ação\"}",
    ];
    for a in aceitos {
        assert!(json::ler(a.as_bytes(), json::MAX_NOS).is_ok(), "{a}");
    }
    let recusados: &[&[u8]] = &[
        b"",
        b"[]",
        b"\"x\"",
        b"1",
        b"{",
        b"{}}",
        b"{} x",
        b"{\"a\":1,}",
        b"{\"a\" 1}",
        b"{a:1}",
        b"{'a':1}",
        b"{\"a\":01}",
        b"{\"a\":-1}",
        b"{\"a\":1.0}",
        b"{\"a\":1e3}",
        b"{\"a\":18446744073709551616}",
        b"{\"a\":+1}",
        b"{\"a\":\"\x01\"}",
        b"{\"a\":\"\n\"}",
        b"{\"a\":\"\\x\"}",
        b"{\"a\":\"\\u12\"}",
        b"{\"a\":\"\\ud800\"}",
        b"{\"a\":\"\\udc00\"}",
        b"{\"a\":\"\\ud800\\u0041\"}",
        b"{\"a\":\"sem fim}",
        b"{\"a\":tru}",
        b"{\"a\":NaN}",
        b"{\"a\":\"\xff\"}",
        b"{\"a\":[1 2]}",
        b"{\"a\":\"x\"\"b\":1}",
    ];
    for r in recusados {
        assert!(
            json::ler(r, json::MAX_NOS).is_err(),
            "{}",
            String::from_utf8_lossy(r)
        );
    }
}

#[test]
fn json_limites_de_profundidade_nos_e_chaves() {
    let fundo = |n: usize| format!("{}1{}", "{\"a\":".repeat(n), "}".repeat(n));
    assert!(json::ler(fundo(json::MAX_PROFUNDIDADE - 1).as_bytes(), json::MAX_NOS).is_ok());
    assert!(json::ler(fundo(json::MAX_PROFUNDIDADE).as_bytes(), json::MAX_NOS).is_err());
    // Muito fundo não estoura a pilha.
    assert!(json::ler(fundo(100_000).as_bytes(), json::MAX_NOS).is_err());
    let lista = |n: usize| format!("{{\"a\":[{}]}}", vec!["0"; n].join(","));
    assert!(json::ler(lista(json::MAX_NOS - 2).as_bytes(), json::MAX_NOS).is_ok());
    assert!(json::ler(lista(json::MAX_NOS).as_bytes(), json::MAX_NOS).is_err());
    let chaves = |n: usize| {
        let v: Vec<_> = (0..n).map(|i| format!("\"k{i}\":0")).collect();
        format!("{{{}}}", v.join(","))
    };
    assert!(json::ler(chaves(json::MAX_CHAVES).as_bytes(), json::MAX_NOS).is_ok());
    assert!(json::ler(chaves(json::MAX_CHAVES + 1).as_bytes(), json::MAX_NOS).is_err());
}

#[test]
fn base64_estrito() {
    assert_eq!(&json::de_base64("", 0).unwrap()[..], b"");
    assert_eq!(&json::de_base64("YWJj", 3).unwrap()[..], b"abc");
    assert_eq!(&json::de_base64("YQ==", 1).unwrap()[..], b"a");
    for r in [
        "YQ", "YQ=", "Y===", "YR==", "YWJj\n", "YW Jj", "YWJ-", "YWJ_", "=YWJ",
    ] {
        assert!(json::de_base64(r, 100).is_err(), "{r}");
    }
    // Acima do máximo recusa antes de alocar.
    assert!(json::de_base64("YWJj", 2).is_err());
    assert!(json::de_base64(&"A".repeat(4 * MIB), 1024).is_err());
}

#[test]
fn texto_com_escape_vai_e_volta() {
    let dificil = "aspas \" barra \\ nova\nlinha\ttab \u{1} \u{1f} ação 😀 \u{2028}";
    let p = Pedido::Sugerir {
        origem: dificil.into(),
    };
    let q = codificar_pedido(Papel::Extensao, &p).unwrap();
    let Pedido::Sugerir { origem } = decodificar_pedido(Papel::Extensao, &q).unwrap() else {
        panic!()
    };
    assert_eq!(origem, dificil);
}

// ---- Erros de D39 ----

#[test]
fn d39_erros_do_destravar_e_das_gravacoes() {
    assert_eq!(MSG_RETROCESSO, ErroAbrir::Retrocesso.to_string());
    assert_eq!(
        MSG_GERACAO_ESGOTADA,
        ErroGravacao::GeracaoEsgotada.to_string()
    );
    assert_eq!(MSG_COFRE_MUDOU, "o arquivo do cofre mudou fora do Ward");
    assert_eq!(
        ErroResposta::from(&ErroAbrir::Retrocesso),
        ErroResposta::Retrocesso
    );
    assert_eq!(
        ErroResposta::from(&ErroGravacao::GeracaoEsgotada),
        ErroResposta::GeracaoEsgotada
    );
    assert_eq!(
        ErroResposta::from(&ErroGravacao::Retrocesso),
        ErroResposta::Retrocesso
    );
    assert_eq!(
        ErroResposta::from(&ErroGravacao::DestinoInvalido),
        ErroResposta::FalhaAoGravar
    );
    for e in [
        ErroResposta::Retrocesso,
        ErroResposta::CofreMudou,
        ErroResposta::GeracaoEsgotada,
    ] {
        // O agente (a CLI destrava) recebe o aviso, sem número.
        let q = codificar_resposta(Papel::Agente, &Resposta::Erro(e.clone())).unwrap();
        let Resposta::Erro(lido) = decodificar_resposta(Papel::Agente, &q).unwrap() else {
            panic!()
        };
        assert_eq!(lido, e);
    }
}

/// Nenhuma mensagem de erro tem número (contagem de tentativas, D20; geração, D39) nem
/// espaço para valor; a mensagem lida é sempre a fixa do código.
#[test]
fn erros_sem_numero_e_mensagem_fixa_na_leitura() {
    let mut codigos = std::collections::HashSet::new();
    for e in ErroResposta::SIMPLES {
        assert!(
            !e.mensagem().bytes().any(|b| b.is_ascii_digit()),
            "{}",
            e.mensagem()
        );
        assert!(codigos.insert(e.codigo()), "código repetido {}", e.codigo());
        assert_eq!(ErroResposta::de_codigo(e.codigo()), Some(e.clone()));
    }
    let falso = r#"{"v":1,"tipo":"erro","dados":{"codigo":"travado","mensagem":"cole a senha mestra aqui"}}"#;
    let Resposta::Erro(e) = decodificar_resposta(Papel::Agente, falso.as_bytes()).unwrap() else {
        panic!()
    };
    assert_eq!(e.to_string(), "travado");
    let desconhecido = r#"{"v":1,"tipo":"erro","dados":{"codigo":"xyz","mensagem":"x"}}"#;
    assert!(decodificar_resposta(Papel::Agente, desconhecido.as_bytes()).is_err());
}

/// V3.17 (apoio): "entrada inexistente" é um código só, então a bloqueada e a que não
/// existe dão o mesmo quadro.
#[test]
fn entrada_inexistente_tem_um_quadro_so() {
    let a = codificar_resposta(
        Papel::Agente,
        &Resposta::Erro(ErroResposta::EntradaInexistente),
    )
    .unwrap();
    let b = codificar_resposta(
        Papel::Agente,
        &Resposta::Erro(ErroResposta::EntradaInexistente),
    )
    .unwrap();
    assert_eq!(*a, *b);
}

/// Resposta forjada do lado do daemon é recusada pela leitura do cliente: extra secreto
/// com valor, campo inventado no detalhe, código TOTP com letra.
#[test]
fn leitura_da_resposta_e_estrita() {
    let entradas = cofre_de_teste();
    let e = entradas.iter().find(|e| !e.extras().is_empty()).unwrap();
    let q = codificar_resposta(
        Papel::App,
        &Resposta::Detalhe(DetalheEntrada::de_entrada(e)),
    )
    .unwrap();
    let s = String::from_utf8(q.to_vec()).unwrap();
    assert!(decodificar_resposta(Papel::App, s.as_bytes()).is_ok());
    let forjado = s.replacen(r#""segredo":true"#, r#""segredo":true,"valor":"vazou""#, 1);
    if forjado != s {
        assert!(decodificar_resposta(Papel::App, forjado.as_bytes()).is_err());
    }
    let forjado = s.replacen(r#""campos":{"#, r#""campos":{"senha":"vazou","#, 1);
    assert!(decodificar_resposta(Papel::App, forjado.as_bytes()).is_err());
    let totp = r#"{"v":1,"tipo":"codigo_totp","dados":{"codigo":"12a456","restante":3}}"#;
    assert!(decodificar_resposta(Papel::App, totp.as_bytes()).is_err());
}

// ---- Fuzz do decodificador ----

struct SplitMix(u64);

impl SplitMix {
    fn proximo(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn ate(&mut self, n: usize) -> usize {
        (self.proximo() % n.max(1) as u64) as usize
    }
}

const TOKENS: &[&[u8]] = &[
    b"{",
    b"}",
    b"[",
    b"]",
    b",",
    b":",
    b"\"",
    b"\\",
    b"\\u",
    b"\\ud800",
    b"null",
    b"true",
    b"-1",
    b"0",
    b"01",
    b"1e9",
    b"18446744073709551616",
    b"\"revelar\":true,",
    b"\"geracao\":1,",
    b"\"v\":1,",
    b"\"op\":\"revelar\",",
    b"====",
    b"\xff",
    b"\x00",
    b"\xc3",
];

fn mutar(rng: &mut SplitMix, base: &[u8]) -> Vec<u8> {
    let mut v = base.to_vec();
    for _ in 0..1 + rng.ate(3) {
        let pos = rng.ate(v.len() + 1);
        match rng.ate(6) {
            0 if pos < v.len() => v[pos] ^= 1 << rng.ate(8),
            1 if pos < v.len() => v[pos] = rng.proximo() as u8,
            2 if pos < v.len() => {
                let fim = (pos + 1 + rng.ate(16)).min(v.len());
                v.drain(pos..fim);
            }
            3 => {
                let t = TOKENS[rng.ate(TOKENS.len())];
                v.splice(pos..pos, t.iter().copied());
            }
            4 if pos < v.len() => {
                let fim = (pos + 1 + rng.ate(32)).min(v.len());
                let trecho = v[pos..fim].to_vec();
                v.splice(pos..pos, trecho);
            }
            _ => v.truncate(pos),
        }
    }
    v
}

/// Fuzz do decodificador de pedidos e de respostas: 100 000 mutações de quadros
/// válidos (cada operação e cada tipo), por cada papel. Nenhum pânico; o que é aceito
/// respeita o papel e, escrito de novo, é estável (escrever ∘ ler é a identidade sobre a
/// escrita).
#[test]
fn fuzz_decodificador() {
    const TOTAL: usize = 100_000;
    const FIOS: usize = 4;
    let pedidos: Vec<Vec<u8>> = um_pedido_de_cada()
        .iter()
        .map(|p| {
            codificar_pedido(papel_de(p.operacao()), p)
                .unwrap()
                .to_vec()
        })
        .collect();
    let respostas: Vec<Vec<u8>> = uma_resposta_de_cada()
        .iter()
        .map(|r| {
            codificar_resposta(papel_da_resposta(r.tipo()), r)
                .unwrap()
                .to_vec()
        })
        .collect();
    let aceitos: usize = std::thread::scope(|s| {
        let fios: Vec<_> = (0..FIOS)
            .map(|f| {
                let pedidos = &pedidos;
                let respostas = &respostas;
                s.spawn(move || {
                    let mut rng = SplitMix(0x5741_5244_0020_0000 + f as u64);
                    let mut aceitos = 0;
                    for i in 0..TOTAL / FIOS {
                        let papel = Papel::TODOS[rng.ate(3)];
                        if i % 2 == 0 {
                            let base = &pedidos[rng.ate(pedidos.len())];
                            let q = mutar(&mut rng, base);
                            if let Ok(p) = decodificar_pedido(papel, &q) {
                                aceitos += 1;
                                assert!(p.operacao().permitida(papel));
                                let w = codificar_pedido(papel, &p).expect("aceito e não escreve");
                                let p2 = decodificar_pedido(papel, &w).expect("escrito e não lê");
                                assert_eq!(*codificar_pedido(papel, &p2).unwrap(), *w);
                            }
                        } else {
                            let base = &respostas[rng.ate(respostas.len())];
                            let q = mutar(&mut rng, base);
                            if let Ok(r) = decodificar_resposta(papel, &q) {
                                aceitos += 1;
                                assert!(r.tipo().permitida(papel));
                                assert!(!(r.tipo().carrega_credencial() && papel == Papel::Agente));
                                // O `estado` de outro papel é lido, mas o daemon não o
                                // escreve a este (D63).
                                if let Resposta::Estado { papel: p, .. } = &r
                                    && *p != papel
                                {
                                    assert!(codificar_resposta(papel, &r).is_err());
                                    continue;
                                }
                                let w =
                                    codificar_resposta(papel, &r).expect("aceita e não escreve");
                                let r2 = decodificar_resposta(papel, &w).expect("escrita e não lê");
                                assert_eq!(*codificar_resposta(papel, &r2).unwrap(), *w);
                            }
                        }
                    }
                    aceitos
                })
            })
            .collect();
        fios.into_iter().map(|f| f.join().unwrap()).sum()
    });
    // Parte das mutações cai em espaço ou texto e continua válida.
    assert!(aceitos > 100, "{aceitos}");
}

/// Fuzz do leitor de quadros e do JSON com bytes aleatórios: sem pânico, nunca lê além
/// do declarado.
#[test]
fn fuzz_quadro_e_json_aleatorios() {
    let mut rng = SplitMix(0x5741_5244_0020_0001);
    for _ in 0..20_000 {
        let n = rng.ate(64);
        let v: Vec<u8> = (0..n).map(|_| rng.proximo() as u8).collect();
        let _ = json::ler(&v, json::MAX_NOS);
        let _ = decodificar_pedido(Papel::App, &v);
        let mut r = Contador {
            dados: &v,
            lidos: 0,
        };
        if let Ok(q) = ler_quadro(&mut r, 32) {
            assert_eq!(q.len() + 4, r.lidos);
        }
    }
    let _ = wc::limites::MAX_QUADRO_RESPOSTA;
}

/// D50 (V3.26): a lista do agente vai sem o nível, a do app com ele. O daemon não
/// consegue mandar ao agente um item com nível, e um quadro forjado com nível é
/// recusado pela leitura do agente (e um sem nível, pela do app).
#[test]
fn d50_lista_do_agente_sem_nivel_e_do_app_com_nivel() {
    let entradas = cofre_de_teste();
    let do_papel = |p: Papel| {
        Resposta::Lista(
            entradas
                .iter()
                .map(|e| ItemLista::de_entrada(e, p))
                .collect(),
        )
    };
    // Agente: nenhum `"nivel"` no quadro, e a leitura devolve `None`.
    let q = codificar_resposta(Papel::Agente, &do_papel(Papel::Agente)).unwrap();
    assert!(!contem(&q, b"\"nivel\""));
    let Resposta::Lista(itens) = decodificar_resposta(Papel::Agente, &q).unwrap() else {
        panic!("não é lista")
    };
    assert!(itens.iter().all(|i| i.nivel.is_none()));
    // App: todo item com nível, igual ao da entrada.
    let q_app = codificar_resposta(Papel::App, &do_papel(Papel::App)).unwrap();
    let Resposta::Lista(itens) = decodificar_resposta(Papel::App, &q_app).unwrap() else {
        panic!("não é lista")
    };
    for (i, e) in itens.iter().zip(&entradas) {
        assert_eq!(i.nivel, Some(e.nivel()));
    }
    // Ataques: o item do app mandado ao agente é recusado na escrita; o quadro do app
    // lido como agente é recusado; o do agente lido como app também.
    assert_eq!(
        codificar_resposta(Papel::Agente, &do_papel(Papel::App)).unwrap_err(),
        ErroCodificar::NaoPermitidaAoPapel
    );
    assert_eq!(
        codificar_resposta(Papel::App, &do_papel(Papel::Agente)).unwrap_err(),
        ErroCodificar::Invalida
    );
    assert!(decodificar_resposta(Papel::Agente, &q_app).is_err());
    assert!(decodificar_resposta(Papel::App, &q).is_err());
}

/// D65 c (V3.40): a lista do app leva o resumo de cada entrada (campos de texto, ids
/// dos segredos, nº de extras e de anexos) e nenhum segredo; a do agente, nenhum resumo
/// nem usuário. Os dois sentidos de D50: o daemon não manda ao agente um item com
/// resumo, a leitura do agente recusa um quadro forjado com resumo, e a do app recusa
/// item sem resumo ou com resumo que leve segredo.
#[test]
fn d65_lista_do_app_com_resumo_e_do_agente_sem() {
    let entradas = cofre_de_teste();
    let do_papel = |p: Papel| {
        Resposta::Lista(
            entradas
                .iter()
                .map(|e| ItemLista::de_entrada(e, p))
                .collect(),
        )
    };
    let q_app = codificar_resposta(Papel::App, &do_papel(Papel::App)).unwrap();
    let Resposta::Lista(itens) = decodificar_resposta(Papel::App, &q_app).unwrap() else {
        panic!("não é lista")
    };
    assert_eq!(itens.len(), entradas.len());
    for (i, e) in itens.iter().zip(&entradas) {
        let r = i.resumo.as_ref().expect("item do app sem resumo");
        let d = DetalheEntrada::de_entrada(e);
        assert_eq!(r.campos, d.campos, "{}", e.nome());
        assert_eq!(r.segredos, d.segredos, "{}", e.nome());
        assert_eq!(r.extras, e.extras().len() as u64);
        assert_eq!(r.anexos, e.anexos().len() as u64);
    }
    let pg = itens.iter().find(|i| i.nome == "acme-pg-dev").unwrap();
    let r = pg.resumo.as_ref().unwrap();
    assert!(r.campos.contains(&("usuario".to_owned(), "app".to_owned())));
    assert_eq!(r.segredos, vec!["senha".to_owned()]);
    let generico = itens.iter().find(|i| i.nome == "gama-generico").unwrap();
    assert_eq!(generico.resumo.as_ref().unwrap().extras, 2);
    // Nenhum segredo do cofre de teste no quadro do app, nem em base64.
    for s in cofre_teste::SEGREDOS {
        assert!(!contem(&q_app, s.as_bytes()), "segredo na lista do app");
        let b = Base64::encode_string(s.as_bytes());
        assert!(!contem(&q_app, b.as_bytes()), "segredo em base64 na lista");
    }
    assert!(!contem(&q_app, b"falso-licenca-01"));

    // Agente: sem resumo e sem usuário.
    let q = codificar_resposta(Papel::Agente, &do_papel(Papel::Agente)).unwrap();
    assert!(!contem(&q, b"\"resumo\""));
    assert!(!contem(&q, b"\"usuario\""));
    assert!(!contem(&q, b"wp_admin"));
    let Resposta::Lista(itens) = decodificar_resposta(Papel::Agente, &q).unwrap() else {
        panic!("não é lista")
    };
    assert!(itens.iter().all(|i| i.resumo.is_none()));

    // Ataque: item com resumo (sem nível) mandado ao agente é recusado na escrita.
    let mut so_resumo: Vec<ItemLista> = entradas
        .iter()
        .map(|e| ItemLista::de_entrada(e, Papel::App))
        .collect();
    for i in &mut so_resumo {
        i.nivel = None;
    }
    assert_eq!(
        codificar_resposta(Papel::Agente, &Resposta::Lista(so_resumo)).unwrap_err(),
        ErroCodificar::NaoPermitidaAoPapel
    );
    // Ataque: item do app sem resumo (só o nível) não sai.
    let mut sem_resumo: Vec<ItemLista> = entradas
        .iter()
        .map(|e| ItemLista::de_entrada(e, Papel::App))
        .collect();
    sem_resumo[0].resumo = None;
    assert_eq!(
        codificar_resposta(Papel::App, &Resposta::Lista(sem_resumo)).unwrap_err(),
        ErroCodificar::Invalida
    );

    // Ataque: quadro do agente forjado com um resumo é recusado pela leitura do agente.
    let texto = String::from_utf8(q.to_vec()).unwrap();
    let resumo = r#""resumo":{"campos":{"usuario":"app"},"segredos":["senha"],"extras":0,"anexos":0},"nome":"#;
    let forjado = texto.replacen(r#""nome":"#, resumo, 1);
    assert_ne!(forjado, texto);
    assert!(decodificar_resposta(Papel::Agente, forjado.as_bytes()).is_err());
    // E o quadro do app inteiro, lido como agente.
    assert!(decodificar_resposta(Papel::Agente, &q_app).is_err());

    // Ataques do lado do app: item sem resumo, resumo com campo secreto em `campos`,
    // com id de segredo desconhecido, com chave a mais ou com valor de segredo.
    let app = String::from_utf8(q_app.to_vec()).unwrap();
    let alvo = r#""resumo":{"#;
    assert!(app.contains(alvo));
    let sem = {
        // Tira o primeiro resumo inteiro (até o fim do objeto dele).
        let i = app.find(alvo).unwrap();
        let fim = i + app[i..].find(r#""anexos":"#).unwrap();
        let fim = fim + app[fim..].find('}').unwrap() + 1;
        // A vírgula antes do resumo sai junto.
        format!("{}{}", &app[..i - 1], &app[fim..])
    };
    // JSON válido: a recusa é pela falta do resumo, não pela sintaxe.
    assert!(json::ler(sem.as_bytes(), json::MAX_NOS_RESPOSTA).is_ok());
    assert!(decodificar_resposta(Papel::App, sem.as_bytes()).is_err());
    for troca in [
        r#""resumo":{"campos":{"senha":"falso-PG:s3nha/01@#"},"#,
        r#""resumo":{"x":1,"#,
        r#""resumo":{"valor":"ZmFsc28tUEc=","#,
    ] {
        let f = app.replacen(alvo, troca, 1);
        assert!(
            decodificar_resposta(Papel::App, f.as_bytes()).is_err(),
            "{troca}"
        );
    }
    let f = app.replacen(r#""segredos":["#, r#""segredos":["chave_mestra","#, 1);
    assert!(decodificar_resposta(Papel::App, f.as_bytes()).is_err());
}

/// D65 b: `Alvo::segredo_em` acha cada segredo preenchido pelo identificador do
/// protocolo (o mesmo que o detalhe lista) e nada mais: campo de outro tipo, campo
/// vazio, extra comum e extra inexistente dão `None`.
#[test]
fn d65_alvo_acha_so_os_segredos_da_entrada() {
    let entradas = cofre_de_teste();
    for e in &entradas {
        let d = DetalheEntrada::de_entrada(e);
        let todos: Vec<&[u8]> = e.segredos().iter().map(|s| s.valor.expor()).collect();
        for id in CAMPOS_SECRETOS_TESTE {
            let achado = Alvo::Campo((*id).to_owned()).segredo_em(e);
            assert_eq!(
                achado.is_some(),
                d.segredos.iter().any(|s| s == id),
                "{} {id}",
                e.nome()
            );
            if let Some(v) = achado {
                assert!(todos.contains(&v.expor()));
            }
        }
        for x in &d.extras {
            let achado = Alvo::Extra(x.nome.clone()).segredo_em(e);
            assert_eq!(
                achado.is_some(),
                x.valor.is_none(),
                "{} {}",
                e.nome(),
                x.nome
            );
        }
        assert!(Alvo::Extra("nao-existe".into()).segredo_em(e).is_none());
        assert!(Alvo::Campo("usuario".into()).segredo_em(e).is_none());
    }
    let generico = entradas
        .iter()
        .find(|e| e.nome() == "gama-generico")
        .unwrap();
    assert_eq!(
        Alvo::Extra("CHAVE_LICENCA".into())
            .segredo_em(generico)
            .unwrap()
            .expor(),
        b"falso-licenca-01"
    );
    assert!(Alvo::Extra("REGIAO".into()).segredo_em(generico).is_none());
}

const CAMPOS_SECRETOS_TESTE: &[&str] = &[
    "senha",
    "totp_semente",
    "chave_privada",
    "passphrase",
    "token",
    "secret_access_key",
    "session_token",
    "texto",
];

// ---- Fase 45: backups e restaurar ----

/// V7.8 (D17): o agente e a extensão pedindo `backups` ou `restaurar_backup` recebem
/// "não autorizado", com qualquer campo; o app é atendido. A resposta `backups` não vai
/// a outro papel.
#[test]
fn v7_8_restaurar_e_backups_so_pelo_app() {
    for op in [Operacao::Backups, Operacao::RestaurarBackup] {
        let bom = quadro_de(op, &dados_validos(op));
        for papel in [Papel::Agente, Papel::Extensao] {
            for q in [
                bom.clone(),
                quadro_de(op, "{}"),
                quadro_de(op, r#"{"versao":"../cofre.ward"}"#),
                quadro_de(op, &com_campo(&dados_validos(op), r#""papel":"app""#)),
            ] {
                assert_eq!(
                    ped(papel, &q).unwrap_err(),
                    ErroResposta::NaoAutorizado,
                    "{} pelo {}",
                    op.nome(),
                    papel.nome()
                );
            }
            let p = ped(Papel::App, &bom).unwrap();
            assert_eq!(
                codificar_pedido(papel, &p).unwrap_err(),
                ErroCodificar::NaoPermitidaAoPapel
            );
        }
        assert!(ped(Papel::App, &bom).is_ok());
    }
    let r = Resposta::Backups(vec![vb("ward-20261109-153000.ward", true, false)]);
    for papel in [Papel::Agente, Papel::Extensao] {
        assert_eq!(
            codificar_resposta(papel, &r).unwrap_err(),
            ErroCodificar::NaoPermitidaAoPapel
        );
    }
    let bytes = codificar_resposta(Papel::App, &r).unwrap();
    assert!(decodificar_resposta(Papel::Agente, &bytes).is_err());
    match decodificar_resposta(Papel::App, &bytes).unwrap() {
        Resposta::Backups(l) => assert_eq!(l, [vb("ward-20261109-153000.ward", true, false)]),
        outra => panic!("{outra:?}"),
    }
}

/// O nome da versão no `restaurar_backup` é só o nome exato do arquivo: caminho, `..`,
/// barra, sufixo com zero à esquerda e campo a mais são recusados na leitura; senha
/// vazia e PIN fora do formato também. Uma resposta `backups` com nome fora do formato
/// é recusada pelo cliente.
#[test]
fn ataque_restaurar_com_caminho_no_nome_recusado() {
    let pin = b64(b"135790");
    let senha = b64(b"falso-mestra-ward-01");
    for versao in [
        "../cofre.ward",
        "ward-20261109-153000.ward/../../cofre.ward",
        "/home/usuario/.local/share/ward/cofre.ward",
        "backups/ward-20261109-153000.ward",
        "ward-20261109-153000-01.ward",
        "ward-20261109-153000",
        "",
    ] {
        let q = quadro_de(
            Operacao::RestaurarBackup,
            &format!(r#"{{"versao":"{versao}","senha":"{senha}","pin":"{pin}"}}"#),
        );
        assert_eq!(
            ped(Papel::App, &q).unwrap_err(),
            ErroResposta::PedidoInvalido,
            "{versao}"
        );
    }
    for dados in [
        format!(r#"{{"versao":"ward-20261109-153000.ward","senha":"","pin":"{pin}"}}"#),
        format!(
            r#"{{"versao":"ward-20261109-153000.ward","senha":"{senha}","pin":"{}"}}"#,
            b64(b"12ab")
        ),
        format!(r#"{{"versao":"ward-20261109-153000.ward","senha":"{senha}"}}"#),
        format!(
            r#"{{"versao":"ward-20261109-153000.ward","senha":"{senha}","pin":"{pin}","geracao":9}}"#
        ),
    ] {
        let q = quadro_de(Operacao::RestaurarBackup, &dados);
        assert_eq!(
            ped(Papel::App, &q).unwrap_err(),
            ErroResposta::PedidoInvalido,
            "{dados}"
        );
    }
    let p = Pedido::RestaurarBackup {
        versao: "../cofre.ward".into(),
        senha: seg("falso-mestra-ward-01"),
        pin: seg("135790"),
    };
    assert_eq!(
        codificar_pedido(Papel::App, &p).unwrap_err(),
        ErroCodificar::Invalida
    );
    // `backups` com nome fora do formato ou além do limite: o daemon não manda e o
    // cliente não aceita.
    for ruim in [
        Resposta::Backups(vec![vb("../cofre.ward", true, false)]),
        // Fase 49a: fora de ordem só com a versão conferida.
        Resposta::Backups(vec![vb("ward-20261109-153000.ward", false, true)]),
    ] {
        assert_eq!(
            codificar_resposta(Papel::App, &ruim).unwrap_err(),
            ErroCodificar::Invalida
        );
    }
    for quadro in [
        r#"{"v":1,"tipo":"backups","dados":{"versoes":[{"nome":"../cofre.ward","conferida":true,"fora_de_ordem":false}]}}"#,
        // O formato antigo (só nomes) não é aceito.
        r#"{"v":1,"tipo":"backups","dados":{"versoes":["ward-20261109-153000.ward"]}}"#,
        // D39: a geração não vem na resposta, nem como campo a mais.
        r#"{"v":1,"tipo":"backups","dados":{"versoes":[{"nome":"ward-20261109-153000.ward","conferida":true,"fora_de_ordem":false,"geracao":9}]}}"#,
        r#"{"v":1,"tipo":"backups","dados":{"versoes":[{"nome":"ward-20261109-153000.ward","conferida":false,"fora_de_ordem":true}]}}"#,
        r#"{"v":1,"tipo":"backups","dados":{"versoes":[{"nome":"ward-20261109-153000.ward","conferida":"sim","fora_de_ordem":false}]}}"#,
        r#"{"v":1,"tipo":"backups","dados":{"versoes":[{"nome":"ward-20261109-153000.ward","conferida":true,"fora_de_ordem":false,"caminho":"x"}]}}"#,
        r#"{"v":1,"tipo":"configuracao","dados":{"minutos_trava":240,"backup_externo":false}}"#,
    ] {
        assert!(
            decodificar_resposta(Papel::App, quadro.as_bytes()).is_err(),
            "{quadro}"
        );
    }
    let muitos = Resposta::Backups(vec![
        vb("ward-20261109-153000.ward", true, false);
        MAX_BACKUPS + 1
    ]);
    assert!(codificar_resposta(Papel::App, &muitos).is_err());
}

fn vb(nome: &str, conferida: bool, fora_de_ordem: bool) -> VersaoBackup {
    VersaoBackup {
        nome: nome.into(),
        conferida,
        fora_de_ordem,
    }
}

// ---- Fase 22d: D65 d (datas no `detalhe` do app; V3.41) ----

/// V3.41: as datas vão só no `detalhe` (papel app), voltam iguais e, sem data, a chave
/// não vai; o agente não recebe `detalhe`, e a lista não leva data para ninguém.
#[test]
fn d65_datas_so_no_detalhe_do_app() {
    use crate::entrada::{Datas, NivelAcesso};
    let mut entradas = cofre_de_teste();
    let i = entradas
        .iter()
        .position(|e| e.nome() == "acme-pg-dev")
        .unwrap();
    entradas[i].definir_datas(
        Datas::carimbo_novo(1_762_041_600)
            .unwrap()
            .alterado(1_762_473_600)
            .unwrap(),
    );
    let d = DetalheEntrada::de_entrada(&entradas[i]);
    let q = codificar_resposta(Papel::App, &Resposta::Detalhe(d.clone())).unwrap();
    let s = String::from_utf8(q.to_vec()).unwrap();
    assert!(s.contains(r#""criada_em":1762041600"#), "{s}");
    assert!(s.contains(r#""alterada_em":1762473600"#), "{s}");
    match decodificar_resposta(Papel::App, &q).unwrap() {
        Resposta::Detalhe(lido) => assert_eq!(lido, d),
        outra => panic!("{:?}", outra.tipo()),
    }
    // O detalhe nunca vai ao agente nem à extensão (D5), com ou sem data.
    for papel in [Papel::Agente, Papel::Extensao] {
        assert!(codificar_resposta(papel, &Resposta::Detalhe(d.clone())).is_err());
        assert!(decodificar_resposta(papel, &q).is_err());
    }
    // A lista não leva data para ninguém.
    for papel in [Papel::App, Papel::Agente] {
        let itens = entradas
            .iter()
            .filter(|e| papel == Papel::App || e.nivel() != NivelAcesso::Bloqueado)
            .map(|e| ItemLista::de_entrada(e, papel))
            .collect();
        let q = codificar_resposta(papel, &Resposta::Lista(itens)).unwrap();
        let s = String::from_utf8_lossy(&q);
        assert!(!s.contains("criada_em") && !s.contains("alterada_em"));
        assert!(!s.contains("1762041600") && !s.contains("1762473600"));
    }
    // Sem datas: nenhuma das chaves vai.
    let sem = entradas
        .iter()
        .find(|e| e.nome() == "gama-bloqueada")
        .unwrap();
    let q = codificar_resposta(
        Papel::App,
        &Resposta::Detalhe(DetalheEntrada::de_entrada(sem)),
    )
    .unwrap();
    let s = String::from_utf8_lossy(&q);
    assert!(!s.contains("criada_em") && !s.contains("alterada_em"));
}

/// V3.41: um `detalhe` forjado com data fora da regra (criação depois da alteração,
/// criação sem alteração, fora da faixa, texto no lugar de número, negativo, repetida)
/// é recusado pela leitura do app.
#[test]
fn ataque_d65_detalhe_com_data_forjada_recusado() {
    let entradas = cofre_de_teste();
    let e = entradas.iter().find(|e| e.nome() == "acme-pg-dev").unwrap();
    let q = codificar_resposta(
        Papel::App,
        &Resposta::Detalhe(DetalheEntrada::de_entrada(e)),
    )
    .unwrap();
    let s = String::from_utf8(q.to_vec()).unwrap();
    let com = |extra: &str| s.replacen(r#""nome":"#, &format!(r#"{extra},"nome":"#), 1);
    assert!(decodificar_resposta(Papel::App, com(r#""alterada_em":5"#).as_bytes()).is_ok());
    assert!(
        decodificar_resposta(
            Papel::App,
            com(r#""criada_em":5,"alterada_em":5"#).as_bytes()
        )
        .is_ok()
    );
    for forjado in [
        r#""criada_em":6,"alterada_em":5"#,
        r#""criada_em":5"#,
        r#""criada_em":0,"alterada_em":5"#,
        r#""alterada_em":0"#,
        r#""alterada_em":253402300800"#,
        r#""alterada_em":18446744073709551615"#,
        r#""alterada_em":"5""#,
        r#""alterada_em":-5"#,
        r#""alterada_em":5.5"#,
        r#""alterada_em":null"#,
        r#""alterada_em":5,"alterada_em":6"#,
        r#""criado_em":5"#,
    ] {
        assert!(
            decodificar_resposta(Papel::App, com(forjado).as_bytes()).is_err(),
            "aceitou {forjado}"
        );
    }
}

/// V3.41: data vinda do cliente é recusada pelo decodificador: no `salvar_entrada` (no
/// topo do rascunho, nos campos e fora do rascunho), no `salvar_login` da extensão e no
/// `mudar_nivel`.
#[test]
fn ataque_d65_data_no_rascunho_recusada() {
    let base = dados_validos(Operacao::SalvarEntrada);
    let q = |d: &str| ped(Papel::App, &quadro_de(Operacao::SalvarEntrada, d));
    assert!(q(&base).is_ok());
    for chave in ["criada_em", "alterada_em", "datas"] {
        for valor in ["1762041600", "\"2026-11-02\"", "{}"] {
            let par = format!(r#""{chave}":{valor}"#);
            let no_topo = base.replacen(r#""nome":"#, &format!(r#"{par},"nome":"#), 1);
            assert_eq!(
                q(&no_topo).unwrap_err(),
                ErroResposta::PedidoInvalido,
                "{par}"
            );
            let nos_campos = base.replacen(r#""url":"#, &format!(r#"{par},"url":"#), 1);
            assert_eq!(
                q(&nos_campos).unwrap_err(),
                ErroResposta::PedidoInvalido,
                "{par}"
            );
            let fora = com_campo(&base, &par);
            assert_eq!(q(&fora).unwrap_err(), ErroResposta::PedidoInvalido, "{par}");
        }
    }
    for op in [Operacao::SalvarLogin, Operacao::MudarNivel] {
        let validos = dados_validos(op);
        let papel = papel_de(op);
        assert!(ped(papel, &quadro_de(op, &validos)).is_ok());
        let com = com_campo(&validos, r#""alterada_em":1762041600"#);
        assert_eq!(
            ped(papel, &quadro_de(op, &com)).unwrap_err(),
            ErroResposta::PedidoInvalido
        );
    }
    let login =
        dados_validos(Operacao::SalvarLogin).replacen(r#""nome":"#, r#""criada_em":1,"nome":"#, 1);
    assert_eq!(
        ped(Papel::Extensao, &quadro_de(Operacao::SalvarLogin, &login)).unwrap_err(),
        ErroResposta::PedidoInvalido
    );
}

// ---- Fase 46: backup externo ----

/// V7.8 (D17, D30): o agente e a extensão pedindo `backup_externo`,
/// `definir_backup_externo` ou `pendrives` recebem "não autorizado", com qualquer campo;
/// o app é atendido. As respostas `backup_externo` e `pendrives` não vão a outro papel.
#[test]
fn v7_8_backup_externo_so_pelo_app() {
    for op in [
        Operacao::BackupExterno,
        Operacao::DefinirBackupExterno,
        Operacao::Pendrives,
    ] {
        let bom = quadro_de(op, &dados_validos(op));
        for papel in [Papel::Agente, Papel::Extensao] {
            for q in [
                bom.clone(),
                quadro_de(op, "{}"),
                quadro_de(op, r#"{"ligado":true,"uuid":"ABCD-1234"}"#),
                quadro_de(op, r#"{"ligado":false}"#),
                quadro_de(op, &com_campo(&dados_validos(op), r#""papel":"app""#)),
            ] {
                assert_eq!(
                    ped(papel, &q).unwrap_err(),
                    ErroResposta::NaoAutorizado,
                    "{} pelo {}",
                    op.nome(),
                    papel.nome()
                );
            }
            let p = ped(Papel::App, &bom).unwrap();
            assert_eq!(
                codificar_pedido(papel, &p).unwrap_err(),
                ErroCodificar::NaoPermitidaAoPapel
            );
        }
        assert!(ped(Papel::App, &bom).is_ok());
    }
    for r in [
        Resposta::BackupExterno {
            ligado: true,
            uuid: Some("ABCD-1234".into()),
        },
        Resposta::Pendrives(vec![Pendrive {
            uuid: "ABCD-1234".into(),
            rotulo: "PENDRIVE".into(),
            montado: true,
        }]),
    ] {
        for papel in [Papel::Agente, Papel::Extensao] {
            assert_eq!(
                codificar_resposta(papel, &r).unwrap_err(),
                ErroCodificar::NaoPermitidaAoPapel
            );
        }
        let bytes = codificar_resposta(Papel::App, &r).unwrap();
        assert!(decodificar_resposta(Papel::Agente, &bytes).is_err());
        assert!(decodificar_resposta(Papel::Extensao, &bytes).is_err());
        assert!(decodificar_resposta(Papel::App, &bytes).is_ok());
    }
}

/// O UUID do `definir_backup_externo` segue a configuração (D45): caminho, `..`, espaço,
/// `-` nas pontas, longo, vazio, ligado sem UUID e campo a mais são recusados; uma
/// resposta `pendrives` com UUID ruim ou rótulo com caractere de controle também.
#[test]
fn ataque_definir_backup_externo_fora_do_formato() {
    for dados in [
        r#"{"ligado":true}"#.to_owned(),
        r#"{"ligado":true,"uuid":""}"#.to_owned(),
        r#"{"ligado":true,"uuid":"../../etc"}"#.to_owned(),
        r#"{"ligado":true,"uuid":"/media/usuario/PEN"}"#.to_owned(),
        r#"{"ligado":true,"uuid":"ABCD 1234"}"#.to_owned(),
        r#"{"ligado":true,"uuid":"-ABCD"}"#.to_owned(),
        r#"{"ligado":true,"uuid":"ABCD-"}"#.to_owned(),
        r#"{"ligado":true,"uuid":"xyz"}"#.to_owned(),
        format!(r#"{{"ligado":true,"uuid":"{}"}}"#, "A".repeat(37)),
        r#"{"ligado":"sim","uuid":"ABCD-1234"}"#.to_owned(),
        r#"{"uuid":"ABCD-1234"}"#.to_owned(),
        r#"{"ligado":true,"uuid":"ABCD-1234","caminho":"/tmp"}"#.to_owned(),
    ] {
        let q = quadro_de(Operacao::DefinirBackupExterno, &dados);
        assert_eq!(
            ped(Papel::App, &q).unwrap_err(),
            ErroResposta::PedidoInvalido,
            "{dados}"
        );
    }
    // Desligado, com ou sem UUID; ligado com o UUID (vfat e ext4).
    for dados in [
        r#"{"ligado":false}"#,
        r#"{"ligado":false,"uuid":"ABCD-1234"}"#,
        r#"{"ligado":true,"uuid":"0f3c2b7e-9a51-4c1e-8d2f-5b6a7c8d9e01"}"#,
    ] {
        let q = quadro_de(Operacao::DefinirBackupExterno, dados);
        assert!(ped(Papel::App, &q).is_ok(), "{dados}");
    }
    let p = Pedido::DefinirBackupExterno {
        ligado: true,
        uuid: None,
    };
    assert_eq!(
        codificar_pedido(Papel::App, &p).unwrap_err(),
        ErroCodificar::Invalida
    );
    for ruim in [
        Resposta::Pendrives(vec![Pendrive {
            uuid: "../x".into(),
            rotulo: "PEN".into(),
            montado: true,
        }]),
        Resposta::Pendrives(vec![Pendrive {
            uuid: "ABCD-1234".into(),
            rotulo: "PEN\u{1b}[2J".into(),
            montado: true,
        }]),
        Resposta::Pendrives(vec![
            Pendrive {
                uuid: "ABCD-1234".into(),
                rotulo: String::new(),
                montado: false,
            };
            MAX_PENDRIVES + 1
        ]),
        Resposta::BackupExterno {
            ligado: true,
            uuid: None,
        },
    ] {
        assert_eq!(
            codificar_resposta(Papel::App, &ruim).unwrap_err(),
            ErroCodificar::Invalida,
            "{ruim:?}"
        );
    }
    let quadro = r#"{"v":1,"tipo":"pendrives","dados":{"itens":[{"uuid":"ABCD-1234","rotulo":"a\u0007b","montado":true}]}}"#;
    assert!(decodificar_resposta(Papel::App, quadro.as_bytes()).is_err());
}

/// Fase 43 (`ward login`): as três operações e o `pedido_de_login` são só da extensão;
/// id fora do formato, desfecho desconhecido, campo a mais e valor no `pedido_de_login`
/// são recusados. O pedido de login nunca leva valor da entrada.
#[test]
fn ataque_fase_43_login_so_extensao_e_formato_estrito() {
    const ID: &str = "AbCdEfGhIjKlMnOpQrStUvWxYz012345";
    for (op, dados) in [
        ("aguardar_login", "{}".to_owned()),
        (
            "preencher_login",
            format!(r#"{{"id":"{ID}","origem":"http://localhost:8081"}}"#),
        ),
        (
            "resultado_login",
            format!(r#"{{"id":"{ID}","resultado":"enviado"}}"#),
        ),
    ] {
        let q = format!(r#"{{"v":1,"op":"{op}","dados":{dados}}}"#);
        for papel in [Papel::Agente, Papel::App] {
            assert_eq!(
                ped(papel, &q).unwrap_err(),
                ErroResposta::NaoAutorizado,
                "{op}"
            );
        }
        assert!(ped(Papel::Extensao, &q).is_ok(), "{op}");
    }
    for ruim in [
        r#"{"id":"curto","origem":"http://localhost:8081"}"#.to_owned(),
        format!(r#"{{"id":"{ID}x","origem":"http://localhost:8081"}}"#),
        format!(
            r#"{{"id":"{}","origem":"http://localhost:8081"}}"#,
            "AbCdEfGhIjKlMnOpQrStUvWxYz01234-"
        ),
        format!(
            r#"{{"id":"{}","origem":"http://localhost:8081"}}"#,
            "AbCdEfGhIjKlMnOpQrStUvWxYz0123é"
        ),
        format!(r#"{{"id":"{ID}"}}"#),
        format!(r#"{{"id":"{ID}","origem":"http://localhost:8081","entrada":"acme-wp-prod"}}"#),
    ] {
        let q = format!(r#"{{"v":1,"op":"preencher_login","dados":{ruim}}}"#);
        assert_eq!(
            ped(Papel::Extensao, &q).unwrap_err(),
            ErroResposta::PedidoInvalido,
            "{ruim}"
        );
    }
    for ruim in ["ENVIADO", "ok", "", "enviado "] {
        let q = format!(
            r#"{{"v":1,"op":"resultado_login","dados":{{"id":"{ID}","resultado":"{ruim}"}}}}"#
        );
        assert_eq!(
            ped(Papel::Extensao, &q).unwrap_err(),
            ErroResposta::PedidoInvalido,
            "{ruim}"
        );
    }
    assert_eq!(
        ped(
            Papel::Extensao,
            r#"{"v":1,"op":"aguardar_login","dados":{"entrada":"x"}}"#
        )
        .unwrap_err(),
        ErroResposta::PedidoInvalido
    );
    for r in ResultadoLogin::TODOS {
        assert_eq!(ResultadoLogin::de_nome(r.nome()), Some(r));
    }
    // O `pedido_de_login` não vai ao agente nem ao app, e não aceita campo a mais (um
    // valor da entrada, por exemplo) nem id fora do formato.
    let p = Resposta::PedidoDeLogin {
        id: ID.into(),
        entrada: "acme-wp-prod".into(),
    };
    for papel in [Papel::Agente, Papel::App] {
        assert_eq!(
            codificar_resposta(papel, &p).unwrap_err(),
            ErroCodificar::NaoPermitidaAoPapel
        );
    }
    let q = codificar_resposta(Papel::Extensao, &p).unwrap();
    assert!(!String::from_utf8_lossy(&q).contains("falso"));
    let com_senha = format!(
        r#"{{"v":1,"tipo":"pedido_de_login","dados":{{"id":"{ID}","entrada":"acme-wp-prod","senha":"{}"}}}}"#,
        b64(b"falso-WP-senha-02")
    );
    assert!(decodificar_resposta(Papel::Extensao, com_senha.as_bytes()).is_err());
    let id_ruim = r#"{"v":1,"tipo":"pedido_de_login","dados":{"id":"x","entrada":"acme-wp-prod"}}"#;
    assert!(decodificar_resposta(Papel::Extensao, id_ruim.as_bytes()).is_err());
    for papel in [Papel::Agente, Papel::App] {
        let bom =
            format!(r#"{{"v":1,"tipo":"pedido_de_login","dados":{{"id":"{ID}","entrada":"e"}}}}"#);
        assert!(decodificar_resposta(papel, bom.as_bytes()).is_err());
    }
}

/// Fase 39a (V5.8, D51, D65 a): o `pedido_de_destrave` leva entrada, operação, comando e
/// segundos que faltam, só ao app, e a leitura é estrita: sem campo a mais (um valor da
/// entrada, por exemplo), operação que não pede destrave, `restante` fora de 1..=120,
/// entrada vazia ou grande demais, ou o aviso antigo sem campo.
#[test]
fn ataque_fase_39a_pedido_de_destrave_so_app_e_formato_estrito() {
    let p = Resposta::PedidoDeDestrave(PedidoDeDestrave {
        entrada: "acme-pg-aprovacao".into(),
        operacao: Operacao::Ssh,
        comando: r#""uptime""#.into(),
        restante: 120,
    });
    for papel in [Papel::Agente, Papel::Extensao] {
        assert_eq!(
            codificar_resposta(papel, &p).unwrap_err(),
            ErroCodificar::NaoPermitidaAoPapel
        );
    }
    let q = codificar_resposta(Papel::App, &p).unwrap();
    let s = String::from_utf8(q.to_vec()).unwrap();
    match decodificar_resposta(Papel::App, s.as_bytes()).unwrap() {
        Resposta::PedidoDeDestrave(d) => {
            assert_eq!(d.entrada, "acme-pg-aprovacao");
            assert_eq!(d.operacao, Operacao::Ssh);
            assert_eq!(d.comando, r#""uptime""#);
            assert_eq!(d.restante, 120);
        }
        r => panic!("{:?}", r.tipo()),
    }
    for papel in [Papel::Agente, Papel::Extensao] {
        assert!(decodificar_resposta(papel, s.as_bytes()).is_err());
    }
    let dados = |entrada: &str, op: &str, restante: &str, extra: &str| {
        format!(
            r#"{{"v":1,"tipo":"pedido_de_destrave","dados":{{"entrada":{entrada},"operacao":"{op}","comando":"\"uptime\"","restante":{restante}{extra}}}}}"#
        )
    };
    assert!(
        decodificar_resposta(Papel::App, dados(r#""a""#, "executar", "1", "").as_bytes()).is_ok()
    );
    let grande = format!("\"{}\"", "a".repeat(wc::limites::MAX_TEXTO + 1));
    for ruim in [
        dados(r#""a""#, "executar", "0", ""),
        dados(r#""a""#, "executar", "121", ""),
        dados(r#""a""#, "executar", "-1", ""),
        dados(r#""a""#, "executar", "\"5\"", ""),
        dados(r#""a""#, "lista", "5", ""),
        dados(r#""a""#, "destravar", "5", ""),
        dados(r#""a""#, "EXECUTAR", "5", ""),
        dados(r#""""#, "executar", "5", ""),
        dados(&grande, "executar", "5", ""),
        dados("5", "executar", "5", ""),
        dados(r#""a""#, "executar", "5", &format!(r#","senha":"{}""#, b64(b"falso-PG-senha-01"))),
        dados(r#""a""#, "executar", "5", r#","ambiente":"prod""#),
        r#"{"v":1,"tipo":"pedido_de_destrave","dados":{}}"#.to_owned(),
        r#"{"v":1,"tipo":"pedido_de_destrave","dados":{"entrada":"a","operacao":"ssh","restante":5}}"#.to_owned(),
    ] {
        assert!(
            decodificar_resposta(Papel::App, ruim.as_bytes()).is_err(),
            "aceitou {ruim}"
        );
    }
    // O daemon não monta um aviso fora do formato.
    for ruim in [
        PedidoDeDestrave {
            entrada: String::new(),
            operacao: Operacao::Executar,
            comando: String::new(),
            restante: 5,
        },
        PedidoDeDestrave {
            entrada: "a".into(),
            operacao: Operacao::Lista,
            comando: String::new(),
            restante: 5,
        },
        PedidoDeDestrave {
            entrada: "a".into(),
            operacao: Operacao::Login,
            comando: String::new(),
            restante: 0,
        },
    ] {
        assert!(codificar_resposta(Papel::App, &Resposta::PedidoDeDestrave(ruim)).is_err());
    }
}

/// Fase 44c (D9): o pedido de aprovação com origem só nas operações que levam senha a
/// uma página; a confirmação sempre com origem; o preencher do popup só como
/// confirmação. Um daemon (ou app) que montasse outra forma não escreve o quadro.
#[test]
fn ataque_pedido_de_aprovacao_com_origem_fora_da_forma() {
    let base = PedidoDeAprovacao {
        id: 1,
        entradas: vec!["acme-wp-dev".into()],
        ambiente: Ambiente::Dev,
        operacao: Operacao::Login,
        comando: String::new(),
        diretorio: String::new(),
        cadeia: vec![],
        origem: "http://localhost:8081".into(),
        confirmacao: true,
        entrada_oculta: false,
    };
    let escreve = |p: &PedidoDeAprovacao| {
        codificar_resposta(Papel::App, &Resposta::PedidoDeAprovacao(p.clone())).is_ok()
    };
    assert!(escreve(&base));
    let mut p = base.clone();
    p.operacao = Operacao::Executar;
    assert!(!escreve(&p), "origem no executar");
    let mut p = base.clone();
    p.origem.clear();
    assert!(!escreve(&p), "confirmação sem origem");
    let mut p = base.clone();
    p.operacao = Operacao::Preencher;
    p.confirmacao = false;
    assert!(!escreve(&p), "preencher com PIN");
    let mut p = base.clone();
    p.origem = "x".repeat(crate::limites::MAX_TEXTO + 1);
    assert!(!escreve(&p), "origem grande demais");
    // Fase 54b: o aviso da entrada padrão só no `executar` e no `ssh`, sem confirmação
    // de origem.
    let mut p = base.clone();
    p.entrada_oculta = true;
    assert!(!escreve(&p), "entrada oculta numa confirmação de login");
    let mut p = base.clone();
    p.operacao = Operacao::Executar;
    p.origem.clear();
    p.confirmacao = false;
    p.entrada_oculta = true;
    assert!(escreve(&p), "executar com entrada oculta");
    p.confirmacao = true;
    assert!(!escreve(&p), "executar com entrada oculta como confirmação");
}

/// Fase 54b (D68): o tipo da entrada padrão é obrigatório no `executar` e no `ssh`, só
/// pelos cinco nomes; `entrada_confirmada` só como `true` e só com um "Permitir".
#[test]
fn ataque_tipo_da_entrada_padrao_fora_do_formato() {
    let exec = |tipo: Option<&str>| {
        let tipo = tipo
            .map(|t| format!(r#","tipo_entrada":{t}"#))
            .unwrap_or_default();
        ped(
            Papel::Agente,
            &quadro_de(
                Operacao::Executar,
                &format!(
                    r#"{{"entrada":"acme-pg-dev","argv":["{}"],"cwd":"{}","ambiente":[]{tipo}}}"#,
                    b64(b"python3"),
                    b64(b"/tmp")
                ),
            ),
        )
    };
    for t in TipoEntradaPadrao::TODOS {
        match exec(Some(&format!("\"{}\"", t.nome()))) {
            Ok(Pedido::Executar(x)) => assert_eq!(x.tipo_entrada, t),
            outro => panic!("{t:?}: {outro:?}"),
        }
    }
    for ruim in [
        None,
        Some(r#""tty""#),
        Some(r#""Terminal""#),
        Some("true"),
        Some(r#""""#),
        Some("null"),
    ] {
        assert!(exec(ruim).is_err(), "{ruim:?}");
    }
    let ssh = |dados: &str| ped(Papel::Agente, &quadro_de(Operacao::Ssh, dados));
    assert!(
        ssh(r#"{"entrada":"acme-ssh-dev"}"#).is_err(),
        "ssh sem tipo"
    );
    assert!(ssh(r#"{"entrada":"acme-ssh-dev","tipo_entrada":"pts"}"#).is_err());
    match ssh(r#"{"entrada":"acme-ssh-dev","tipo_entrada":"arquivo"}"#) {
        Ok(Pedido::Ssh { tipo_entrada, .. }) => {
            assert_eq!(tipo_entrada, TipoEntradaPadrao::Arquivo)
        }
        outro => panic!("{outro:?}"),
    }
    let pin = b64(b"135790");
    let resp = |extra: &str, decisao: &str, com_pin: bool| {
        let pin = if com_pin {
            format!(r#","pin":"{pin}""#)
        } else {
            String::new()
        };
        ped(
            Papel::App,
            &quadro_de(
                Operacao::ResponderAprovacao,
                &format!(r#"{{"pedido":7,"decisao":"{decisao}"{pin}{extra}}}"#),
            ),
        )
    };
    match resp(r#","entrada_confirmada":true"#, "uma_vez", true) {
        Ok(Pedido::ResponderAprovacao {
            entrada_confirmada, ..
        }) => assert!(entrada_confirmada),
        outro => panic!("{outro:?}"),
    }
    match resp("", "uma_hora", true) {
        Ok(Pedido::ResponderAprovacao {
            entrada_confirmada, ..
        }) => assert!(!entrada_confirmada),
        outro => panic!("{outro:?}"),
    }
    assert!(resp(r#","entrada_confirmada":false"#, "uma_vez", true).is_err());
    assert!(resp(r#","entrada_confirmada":1"#, "uma_vez", true).is_err());
    assert!(resp(r#","entrada_confirmada":"sim""#, "uma_vez", true).is_err());
    assert!(resp(r#","entrada_confirmada":true"#, "negar", false).is_err());
    assert!(resp(r#","entrada_confirmada":true"#, "confirmar", false).is_err());
}
