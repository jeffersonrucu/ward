//! Testes de ataque do estado do daemon (Fase 22): travar, destravar com espera
//! crescente, PIN, nível, lista do agente, gerações e identidade do arquivo.
//!
//! O daemon roda em processo, com relógio falso e um cofre de teste próprio em
//! `target/tmp` (valores falsos). O papel é o que o socket daria: o pedido passa pela
//! mesma leitura do protocolo.

mod comum;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use comum::*;
use ward_core::entrada::NivelAcesso;
use ward_core::protocolo::{ErroResposta, Papel, Resposta};
use wardd::arquivos::{self, Geracoes};
use wardd::nucleo::{ESPERA_MAXIMA, espera_destrave};

fn lista(d: &wardd::nucleo::Daemon, papel: Papel) -> Vec<ward_core::protocolo::ItemLista> {
    match pedir(d, papel, "lista", "{}") {
        Resposta::Lista(l) => l,
        r => panic!("{r:?}"),
    }
}

fn nivel_de(d: &wardd::nucleo::Daemon, nome: &str) -> Option<NivelAcesso> {
    lista(d, Papel::App)
        .into_iter()
        .find(|i| i.nome == nome)
        .and_then(|i| i.nivel)
}

fn sou_root() -> bool {
    ward_plataforma::uid_efetivo() == 0
}

/// V3.10 e V3.8 (estado): depois de `travar`, toda operação responde "travado" até
/// destravar de novo; o daemon não guarda mais as entradas.
#[test]
fn v3_10_travar_e_tudo_responde_travado() {
    let dir = Dir::novo("travar");
    let (d, _) = cofre_de_teste(&dir);
    assert!(!d.travado());
    assert!(ok(&pedir(&d, Papel::Agente, "travar", "{}")));
    assert!(d.travado());
    let pin = b64(PIN.as_bytes());
    for (papel, op, dados) in [
        (Papel::Agente, "lista", "{}".to_owned()),
        (Papel::App, "lista", "{}".to_owned()),
        (Papel::App, "detalhe", r#"{"entrada":"t-aprov"}"#.to_owned()),
        (
            Papel::App,
            "remover_entrada",
            r#"{"entrada":"t-aprov"}"#.to_owned(),
        ),
        (Papel::App, "tempo_trava", r#"{"minutos":30}"#.to_owned()),
        (
            Papel::App,
            "mudar_nivel",
            format!(r#"{{"entrada":"t-aprov","nivel":"livre","pin":"{pin}"}}"#),
        ),
        (Papel::App, "salvar_entrada", login("t-novo", SEGREDOS[0])),
        (
            Papel::Agente,
            "executar",
            format!(
                r#"{{"entrada":"t-livre","argv":["{}"],"cwd":"{}","ambiente":[]}}"#,
                b64(b"true"),
                b64(b"/")
            ),
        ),
    ] {
        assert_eq!(
            erro(&pedir(&d, papel, op, &dados)),
            Some(ErroResposta::Travado),
            "{op}"
        );
    }
    match pedir(&d, Papel::Agente, "estado", "{}") {
        Resposta::Estado {
            travado,
            cofre_existe,
            papel,
        } => assert!(travado && cofre_existe && papel == Papel::Agente),
        r => panic!("{r:?}"),
    }
    assert!(ok(&destravar(&d, SENHA)));
    assert_eq!(lista(&d, Papel::App).len(), 5);
}

/// V3.9 (D20): 10 senhas erradas seguidas; cada tentativa espera mais que a anterior,
/// até 30 s, e todas as respostas são iguais byte a byte (nenhuma diz quantas faltam).
/// A certa zera a espera.
#[test]
fn v3_9_senha_errada_espera_crescente_sem_contagem() {
    let esperadas: Vec<u64> = vec![0, 1, 2, 4, 8, 16, 30, 30, 30, 30];
    for (i, s) in esperadas.iter().enumerate() {
        assert_eq!(espera_destrave(i as u32), Duration::from_secs(*s));
    }
    assert_eq!(espera_destrave(u32::MAX), ESPERA_MAXIMA);

    let dir = Dir::novo("espera");
    let (d, relogio) = cofre_de_teste(&dir);
    d.travar();
    let mut quadros = Vec::new();
    for i in 0..10 {
        let r = destravar(&d, &format!("falso-errada-{i:02}"));
        assert_eq!(erro(&r), Some(ErroResposta::SenhaIncorreta));
        quadros.push(codificado(Papel::Agente, &r));
    }
    assert!(quadros.windows(2).all(|w| w[0] == w[1]));
    let s = String::from_utf8_lossy(&quadros[0]);
    assert!(!s.chars().any(|c| c.is_ascii_digit() && c != '1'), "{s}");
    let esperas: Vec<u64> = relogio
        .0
        .lock()
        .unwrap()
        .iter()
        .map(|d| d.as_secs())
        .collect();
    assert_eq!(esperas, esperadas);
    assert!(esperas.windows(2).all(|w| w[1] >= w[0]) && esperas[6] == 30);
    // A certa espera os 30 s e destrava; depois, a espera volta a 0.
    assert!(ok(&destravar(&d, SENHA)));
    d.travar();
    let _ = destravar(&d, "falso-errada-xx");
    let esperas = relogio.0.lock().unwrap().clone();
    assert_eq!(esperas[10], ESPERA_MAXIMA);
    assert_eq!(esperas[11], Duration::ZERO);
}

/// V3.11 (S9): `mudar_nivel` sem PIN ou com PIN errado é recusado e o arquivo não
/// muda; o agente com o PIN certo recebe "não autorizado".
#[test]
fn v3_11_mudar_nivel_exige_pin_e_papel_app() {
    let dir = Dir::novo("nivel");
    let (d, _) = cofre_de_teste(&dir);
    let cofre = dir.caminhos().cofre;
    let antes = fs::read(&cofre).unwrap();

    // Sem PIN: a leitura do pedido recusa.
    let sem_pin = pedir(
        &d,
        Papel::App,
        "mudar_nivel",
        r#"{"entrada":"t-aprov","nivel":"bloqueado"}"#,
    );
    assert_eq!(erro(&sem_pin), Some(ErroResposta::PedidoInvalido));
    // PIN errado.
    let r = mudar_nivel(&d, Papel::App, "t-aprov", "bloqueado", PIN_ERRADO);
    assert_eq!(erro(&r), Some(ErroResposta::PinIncorreto));
    // Agente, PIN certo: não autorizado, pela leitura e pela segunda camada.
    let r = mudar_nivel(&d, Papel::Agente, "t-aprov", "bloqueado", PIN);
    assert_eq!(erro(&r), Some(ErroResposta::NaoAutorizado));
    let p = ped(
        Papel::App,
        "mudar_nivel",
        &format!(
            r#"{{"entrada":"t-aprov","nivel":"bloqueado","pin":"{}"}}"#,
            b64(PIN.as_bytes())
        ),
    )
    .unwrap();
    assert_eq!(
        erro(&d.atender(Papel::Agente, p)),
        Some(ErroResposta::NaoAutorizado)
    );
    assert_eq!(fs::read(&cofre).unwrap(), antes, "o arquivo mudou");
    assert_eq!(nivel_de(&d, "t-aprov"), Some(NivelAcesso::ComAprovacao));

    // App com o PIN certo: muda e fica no disco.
    assert!(ok(&mudar_nivel(
        &d,
        Papel::App,
        "t-aprov",
        "bloqueado",
        PIN
    )));
    assert_ne!(fs::read(&cofre).unwrap(), antes);
    d.travar();
    assert!(ok(&destravar(&d, SENHA)));
    assert_eq!(nivel_de(&d, "t-aprov"), Some(NivelAcesso::Bloqueado));
}

/// V3.12 (S9): 5 erros de PIN seguidos travam, contando entre conexões (o contador é
/// do daemon); 4 erros e 1 acerto zeram a contagem.
#[test]
fn v3_12_cinco_erros_de_pin_travam() {
    let dir = Dir::novo("pin");
    let (d, _) = cofre_de_teste(&dir);
    for _ in 0..4 {
        let r = mudar_nivel(&d, Papel::App, "t-aprov", "livre", PIN_ERRADO);
        assert_eq!(erro(&r), Some(ErroResposta::PinIncorreto));
    }
    assert!(!d.travado());
    assert!(ok(&mudar_nivel(
        &d,
        Papel::App,
        "t-aprov",
        "com-aprovacao",
        PIN
    )));
    // A contagem zerou: mais 4 erros não travam (trocar_pin também conta).
    for _ in 0..4 {
        let r = pedir(
            &d,
            Papel::App,
            "trocar_pin",
            &format!(
                r#"{{"atual":"{}","novo":"{}"}}"#,
                b64(PIN_ERRADO.as_bytes()),
                b64(b"2468")
            ),
        );
        assert_eq!(erro(&r), Some(ErroResposta::PinIncorreto));
    }
    assert!(!d.travado());
    let r = mudar_nivel(&d, Papel::App, "t-aprov", "livre", PIN_ERRADO);
    assert_eq!(erro(&r), Some(ErroResposta::PinIncorreto));
    assert!(d.travado(), "o 5.º erro seguido tem de travar");
    assert_eq!(
        erro(&pedir(&d, Papel::App, "lista", "{}")),
        Some(ErroResposta::Travado)
    );
    // Destravar zera a contagem.
    assert!(ok(&destravar(&d, SENHA)));
    let r = mudar_nivel(&d, Papel::App, "t-aprov", "livre", PIN_ERRADO);
    assert_eq!(erro(&r), Some(ErroResposta::PinIncorreto));
    assert!(!d.travado());
}

/// V3.17, V3.26 (S1, D50): a lista do agente não tem a bloqueada nem o nível; a do app
/// tem as duas coisas; `run` com a bloqueada responde byte a byte igual ao de uma
/// entrada que não existe.
#[test]
fn v3_17_v3_26_lista_do_agente_e_bloqueada_igual_inexistente() {
    let dir = Dir::novo("lista");
    let (d, _) = cofre_de_teste(&dir);
    let agente = lista(&d, Papel::Agente);
    assert_eq!(agente.len(), 4);
    assert!(
        agente
            .iter()
            .all(|i| i.nome != "t-bloq" && i.nivel.is_none())
    );
    let q = codificado(Papel::Agente, &Resposta::Lista(agente));
    assert!(!contem(&q, b"\"nivel\""));
    for s in SEGREDOS {
        assert!(!contem(&q, s.as_bytes()) && !contem(&q, b64(s.as_bytes()).as_bytes()));
    }
    let app = lista(&d, Papel::App);
    assert_eq!(app.len(), 5);
    assert!(app.iter().all(|i| i.nivel.is_some()));
    assert_eq!(nivel_de(&d, "t-bloq"), Some(NivelAcesso::Bloqueado));

    let executar = |nome: &str| {
        codificado(
            Papel::Agente,
            &pedir(
                &d,
                Papel::Agente,
                "executar",
                &format!(
                    r#"{{"entrada":"{nome}","argv":["{}"],"cwd":"{}","ambiente":[]}}"#,
                    b64(b"env"),
                    b64(b"/")
                ),
            ),
        )
    };
    let bloqueada = executar("t-bloq");
    assert_eq!(bloqueada, executar("nao-existe"));
    assert!(contem(&bloqueada, b"entrada_inexistente"));
    for op in ["ssh", "login", "playwright"] {
        let a = codificado(
            Papel::Agente,
            &pedir(&d, Papel::Agente, op, r#"{"entrada":"t-bloq"}"#),
        );
        let b = codificado(
            Papel::Agente,
            &pedir(&d, Papel::Agente, op, r#"{"entrada":"nao-existe"}"#),
        );
        assert_eq!(a, b, "{op}");
    }
}

/// V3.27 (D50, S9): entrada criada nasce "com aprovação"; criar já "livre" é recusado;
/// remover e recriar volta "com aprovação"; editar mantém o nível.
#[test]
fn v3_27_entrada_nova_nasce_com_aprovacao() {
    let dir = Dir::novo("nasce");
    let (d, _) = cofre_de_teste(&dir);
    assert!(ok(&pedir(
        &d,
        Papel::App,
        "salvar_entrada",
        &login("t-nova", SEGREDOS[0])
    )));
    assert_eq!(nivel_de(&d, "t-nova"), Some(NivelAcesso::ComAprovacao));
    // Com nível no rascunho, em qualquer lugar: recusado pela leitura.
    let com_nivel =
        login("t-nova2", SEGREDOS[0]).replacen(r#""nome":"#, r#""nivel":"livre","nome":"#, 1);
    assert_eq!(
        erro(&pedir(&d, Papel::App, "salvar_entrada", &com_nivel)),
        Some(ErroResposta::PedidoInvalido)
    );
    assert!(lista(&d, Papel::App).iter().all(|i| i.nome != "t-nova2"));
    // Editar a livre mantém livre.
    let editar = login("t-livre", SEGREDOS[3]).replacen('{', r#"{"anterior":"t-livre","#, 1);
    assert!(ok(&pedir(&d, Papel::App, "salvar_entrada", &editar)));
    assert_eq!(nivel_de(&d, "t-livre"), Some(NivelAcesso::Livre));
    // Remover e recriar com o mesmo nome: volta "com aprovação".
    assert!(ok(&pedir(
        &d,
        Papel::App,
        "remover_entrada",
        r#"{"entrada":"t-livre"}"#
    )));
    assert!(ok(&pedir(
        &d,
        Papel::App,
        "salvar_entrada",
        &login("t-livre", SEGREDOS[3])
    )));
    assert_eq!(nivel_de(&d, "t-livre"), Some(NivelAcesso::ComAprovacao));
    // Nome repetido é erro de validação, e nada muda.
    let r = pedir(
        &d,
        Papel::App,
        "salvar_entrada",
        &login("t-livre", SEGREDOS[3]),
    );
    assert!(
        matches!(erro(&r), Some(ErroResposta::Validacao(_))),
        "{r:?}"
    );
    // Remover um servidor usado como túnel deixaria o cofre sem reabrir: recusado.
    let r = pedir(&d, Papel::App, "remover_entrada", r#"{"entrada":"s1"}"#);
    assert!(
        matches!(erro(&r), Some(ErroResposta::Validacao(_))),
        "{r:?}"
    );
    let renomear = servidor("s2").replacen('{', r#"{"anterior":"s1","#, 1);
    let r = pedir(&d, Papel::App, "salvar_entrada", &renomear);
    assert!(
        matches!(erro(&r), Some(ErroResposta::Validacao(_))),
        "{r:?}"
    );
    assert!(lista(&d, Papel::App).iter().any(|i| i.nome == "s1"));
    // O cofre continua abrindo com tudo isso.
    d.travar();
    assert!(ok(&destravar(&d, SENHA)));
    assert_eq!(lista(&d, Papel::App).len(), 6);
}

/// V3.22 (D39; S7, S9, S13): cópia antiga por cima com o daemon travado, estado
/// apagado, troca por `mv` e reescrita no lugar com o daemon destravado.
#[test]
fn v3_22_rollback_e_arquivo_trocado() {
    let dir = Dir::novo("rollback");
    let c = dir.caminhos();
    let (d, _) = cofre_de_teste(&dir);
    let copia_n = fs::read(&c.cofre).unwrap();
    let n = geracao_no_disco(&c.cofre, SENHA);
    assert!(ok(&pedir(
        &d,
        Papel::App,
        "tempo_trava",
        r#"{"minutos":30}"#
    )));
    assert!(ok(&pedir(
        &d,
        Papel::App,
        "tempo_trava",
        r#"{"minutos":60}"#
    )));
    assert_eq!(geracao_no_disco(&c.cofre, SENHA), n + 2);
    assert_eq!(
        geracoes(&dir),
        Geracoes {
            vista: n + 2,
            queimada: n + 2
        }
    );
    d.travar();

    // `cp` da cópia autêntica de n por cima e destravar: retrocesso, segue travado e o
    // arquivo não é regravado.
    fs::write(&c.cofre, &copia_n).unwrap();
    let r = destravar(&d, SENHA);
    assert_eq!(erro(&r), Some(ErroResposta::Retrocesso));
    assert!(contem(
        &codificado(Papel::Agente, &r),
        "o cofre no disco é mais antigo que o último aberto".as_bytes()
    ));
    assert!(d.travado());
    assert_eq!(fs::read(&c.cofre).unwrap(), copia_n);

    // Apagar o estado: recusa igual, mesmo com o cofre certo de volta.
    fs::remove_file(&c.estado).unwrap();
    let r = destravar(&d, SENHA);
    assert_eq!(erro(&r), Some(ErroResposta::Retrocesso));
    assert!(d.travado());

    // Estado de volta (n), destrava a cópia n: a geração sobe com as gravações.
    // O arquivo é outro desde o último travar: o primeiro destravar avisa que ele mudou
    // fora do Ward; o seguinte decide só pela geração.
    arquivos::gravar_estado(
        &c.estado,
        Geracoes {
            vista: n,
            queimada: n + 2,
        },
    )
    .unwrap();
    assert_eq!(erro(&destravar(&d, SENHA)), Some(ErroResposta::CofreMudou));
    assert!(ok(&destravar(&d, SENHA)));
    // Com o daemon destravado, o arquivo trocado por `mv` (inode novo): não grava por
    // cima, trava e avisa.
    let outro = dir.0.join("outro.ward");
    fs::copy(&c.cofre, &outro).unwrap();
    fs::rename(&outro, &c.cofre).unwrap();
    let antes = fs::read(&c.cofre).unwrap();
    let r = pedir(&d, Papel::App, "tempo_trava", r#"{"minutos":90}"#);
    assert_eq!(erro(&r), Some(ErroResposta::CofreMudou));
    assert!(contem(
        &codificado(Papel::App, &r),
        "o arquivo do cofre mudou fora do Ward".as_bytes()
    ));
    assert!(d.travado());
    assert_eq!(fs::read(&c.cofre).unwrap(), antes);

    // Reescrito no lugar (mesmo inode, `mtime` novo): igual.
    assert!(ok(&destravar(&d, SENHA)));
    std::thread::sleep(Duration::from_millis(20));
    fs::write(&c.cofre, &antes).unwrap();
    let r = pedir(&d, Papel::App, "tempo_trava", r#"{"minutos":90}"#);
    assert_eq!(erro(&r), Some(ErroResposta::CofreMudou));
    assert!(d.travado());
    // Trocado com o daemon travado (sem mudar a geração): o destravar avisa uma vez.
    assert!(ok(&destravar(&d, SENHA)));
    d.travar();
    std::thread::sleep(Duration::from_millis(20));
    fs::write(&c.cofre, &antes).unwrap();
    assert_eq!(erro(&destravar(&d, SENHA)), Some(ErroResposta::CofreMudou));
    assert!(ok(&destravar(&d, SENHA)));

    // O estado é 0600, sem segredo.
    let m = fs::metadata(&c.estado).unwrap();
    assert_eq!(m.permissions().mode() & 0o777, 0o600);
    let texto = fs::read(&c.estado).unwrap();
    for s in SEGREDOS.iter().chain([&SENHA, &PIN]) {
        assert!(!contem(&texto, s.as_bytes()));
    }
}

/// V3.28 (D53; S7, S9): gravação que falha depois do rename (fsync do diretório, pelo
/// diretório 0300), reinício entre A e B, estado que não grava e troca de senha que
/// falha depois de selar.
#[test]
fn v3_28_geracao_queimada_no_estado() {
    if sou_root() {
        eprintln!("v3_28: o root lê diretório 0300; o caso não se reproduz como root");
        return;
    }
    let dir = Dir::novo("queimada");
    let dir_estado = Dir::novo("queimada-estado");
    let mut c = dir.caminhos();
    c.estado = dir_estado.0.join("estado");
    // O cofre de teste nasce no diretório de estado padrão; muda para o separado.
    let (d0, _) = cofre_de_teste(&dir);
    drop(d0);
    fs::copy(dir.caminhos().estado, &c.estado).unwrap();
    let (d, _) = daemon_em(c.clone());
    assert!(ok(&destravar(&d, SENHA)));
    let n = geracao_no_disco(&c.cofre, SENHA);
    assert_eq!(
        geracoes_em(&c.estado),
        Geracoes {
            vista: n,
            queimada: n
        }
    );

    // A: o rename acontece, o fsync do diretório falha.
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o300)).unwrap();
    let r = pedir(&d, Papel::App, "tempo_trava", r#"{"minutos":30}"#);
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(erro(&r), Some(ErroResposta::FalhaAoGravar));
    assert_eq!(
        geracoes_em(&c.estado),
        Geracoes {
            vista: n,
            queimada: n + 1
        }
    );
    assert_eq!(geracao_no_disco(&c.cofre, SENHA), n + 1);
    let copia_a = fs::read(&c.cofre).unwrap();

    // Reinício entre A e B: o daemon novo abre com (n, n+1) e B grava n+2.
    drop(d);
    let (d, _) = daemon_em(c.clone());
    assert!(ok(&destravar(&d, SENHA)));
    assert_eq!(
        geracoes_em(&c.estado),
        Geracoes {
            vista: n + 1,
            queimada: n + 1
        }
    );
    assert!(ok(&pedir(
        &d,
        Papel::App,
        "tempo_trava",
        r#"{"minutos":45}"#
    )));
    assert_eq!(geracao_no_disco(&c.cofre, SENHA), n + 2);
    assert_eq!(
        geracoes_em(&c.estado),
        Geracoes {
            vista: n + 2,
            queimada: n + 2
        }
    );

    // A cópia de A no lugar: retrocesso.
    d.travar();
    fs::write(&c.cofre, &copia_a).unwrap();
    assert_eq!(erro(&destravar(&d, SENHA)), Some(ErroResposta::Retrocesso));
    assert!(d.travado());

    // Sem reinício: A falha depois do rename, B grava n+4 (n+3 foi queimada por A).
    let (d, _) = cofre_n2(&c, &dir);
    let g = geracao_no_disco(&c.cofre, SENHA);
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o300)).unwrap();
    let r = pedir(&d, Papel::App, "tempo_trava", r#"{"minutos":31}"#);
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(erro(&r), Some(ErroResposta::FalhaAoGravar));
    assert_eq!(
        geracoes_em(&c.estado),
        Geracoes {
            vista: g,
            queimada: g + 1
        }
    );
    assert!(ok(&pedir(
        &d,
        Papel::App,
        "tempo_trava",
        r#"{"minutos":32}"#
    )));
    assert_eq!(geracao_no_disco(&c.cofre, SENHA), g + 2);
    assert_eq!(
        geracoes_em(&c.estado),
        Geracoes {
            vista: g + 2,
            queimada: g + 2
        }
    );

    // O estado não grava (diretório só leitura): o cofre não é gravado.
    let antes = fs::read(&c.cofre).unwrap();
    fs::set_permissions(&dir_estado.0, fs::Permissions::from_mode(0o500)).unwrap();
    let r = pedir(&d, Papel::App, "tempo_trava", r#"{"minutos":33}"#);
    fs::set_permissions(&dir_estado.0, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(erro(&r), Some(ErroResposta::FalhaAoGravar));
    assert_eq!(fs::read(&c.cofre).unwrap(), antes);
    assert!(!d.travado());

    // `trocar_senha` que falha depois de selar: trava, e o destravar seguinte abre
    // (com a senha nova, que chegou ao disco) acima da queimada.
    let nova = "falso-nova-02";
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o300)).unwrap();
    let r = pedir(
        &d,
        Papel::App,
        "trocar_senha",
        &format!(
            r#"{{"atual":"{}","nova":"{}"}}"#,
            b64(SENHA.as_bytes()),
            b64(nova.as_bytes())
        ),
    );
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(erro(&r), Some(ErroResposta::FalhaAoGravar));
    assert!(d.travado());
    let q = geracoes_em(&c.estado).queimada;
    assert!(ok(&destravar(&d, nova)));
    assert!(ok(&pedir(
        &d,
        Papel::App,
        "tempo_trava",
        r#"{"minutos":34}"#
    )));
    assert!(geracao_no_disco(&c.cofre, nova) > q);
}

/// Daemon novo destravado no estado atual (depois de um retrocesso de teste: põe de volta
/// o cofre mais novo pelo próprio estado).
fn cofre_n2(c: &wardd::nucleo::Caminhos, dir: &Dir) -> (wardd::nucleo::Daemon, RelogioFalso) {
    // Recria o cofre do zero neste diretório, com o estado separado.
    let _ = fs::remove_file(&c.cofre);
    let _ = fs::remove_file(&c.estado);
    let (d0, _) = cofre_de_teste(dir);
    drop(d0);
    fs::copy(dir.caminhos().estado, &c.estado).unwrap();
    let (d, r) = daemon_em(c.clone());
    assert!(ok(&destravar(&d, SENHA)));
    (d, r)
}

fn geracoes_em(p: &std::path::Path) -> Geracoes {
    arquivos::ler_estado(p).unwrap()
}

/// V3.28: a troca de senha com a senha atual errada responde `senha_incorreta` (D52) e
/// não trava; a certa troca e o cofre abre só com a nova.
#[test]
fn d52_trocar_senha_com_senha_errada() {
    let dir = Dir::novo("troca");
    let (d, _) = cofre_de_teste(&dir);
    let troca = |atual: &str, nova: &str| {
        pedir(
            &d,
            Papel::App,
            "trocar_senha",
            &format!(
                r#"{{"atual":"{}","nova":"{}"}}"#,
                b64(atual.as_bytes()),
                b64(nova.as_bytes())
            ),
        )
    };
    assert_eq!(
        erro(&troca("falso-errada-01", "falso-nova-02")),
        Some(ErroResposta::SenhaIncorreta)
    );
    assert!(!d.travado());
    assert!(ok(&troca(SENHA, "falso-nova-02")));
    assert!(ok(&pedir(
        &d,
        Papel::App,
        "tempo_trava",
        r#"{"minutos":20}"#
    )));
    d.travar();
    assert_eq!(
        erro(&destravar(&d, SENHA)),
        Some(ErroResposta::SenhaIncorreta)
    );
    assert!(ok(&destravar(&d, "falso-nova-02")));
}

/// Primeiro uso: sem cofre, `estado` diz que não existe e destravar responde
/// `cofre_inexistente`; criar de novo é recusado; o PIN não aparece no estado nem no
/// arquivo (V3.13: só o hash, dentro da parte cifrada).
#[test]
fn primeiro_uso_e_pin_so_como_hash() {
    let dir = Dir::novo("primeiro");
    let (d, _) = daemon_em(dir.caminhos());
    match pedir(&d, Papel::Agente, "estado", "{}") {
        Resposta::Estado {
            travado,
            cofre_existe,
            ..
        } => assert!(travado && !cofre_existe),
        r => panic!("{r:?}"),
    }
    assert_eq!(
        erro(&destravar(&d, SENHA)),
        Some(ErroResposta::CofreInexistente)
    );
    // O agente não cria cofre.
    let criar = format!(
        r#"{{"senha":"{}","pin":"{}"}}"#,
        b64(SENHA.as_bytes()),
        b64(PIN.as_bytes())
    );
    assert_eq!(
        erro(&pedir(&d, Papel::Agente, "criar_cofre", &criar)),
        Some(ErroResposta::NaoAutorizado)
    );
    drop(d);
    let (d, _) = cofre_de_teste(&dir);
    assert_eq!(
        erro(&pedir(&d, Papel::App, "criar_cofre", &criar)),
        Some(ErroResposta::CofreJaExiste)
    );
    d.travar();
    assert_eq!(
        erro(&pedir(&d, Papel::App, "criar_cofre", &criar)),
        Some(ErroResposta::CofreJaExiste)
    );
    let c = dir.caminhos();
    for arq in [&c.cofre, &c.estado] {
        let b = fs::read(arq).unwrap();
        assert!(!contem(&b, PIN.as_bytes()) && !contem(&b, SENHA.as_bytes()));
        for s in SEGREDOS {
            assert!(!contem(&b, s.as_bytes()));
        }
        assert_eq!(
            fs::metadata(arq).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    // Trocar o PIN: o antigo deixa de valer, o novo vale, e fica no disco.
    assert!(ok(&destravar(&d, SENHA)));
    let r = pedir(
        &d,
        Papel::App,
        "trocar_pin",
        &format!(
            r#"{{"atual":"{}","novo":"{}"}}"#,
            b64(PIN.as_bytes()),
            b64(b"2468")
        ),
    );
    assert!(ok(&r), "{r:?}");
    d.travar();
    assert!(ok(&destravar(&d, SENHA)));
    assert_eq!(
        erro(&mudar_nivel(&d, Papel::App, "t-aprov", "livre", PIN)),
        Some(ErroResposta::PinIncorreto)
    );
    assert!(ok(&mudar_nivel(&d, Papel::App, "t-aprov", "livre", "2468")));
}
