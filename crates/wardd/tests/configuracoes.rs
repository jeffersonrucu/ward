//! Tela Configurações no daemon (Fase 38, V5.22; D30, D39, S9): a operação
//! `configuracao` (tempo de trava e backup externo ligado ou não) e as trocas de PIN,
//! senha mestra e tempo de trava pelo papel app.
//!
//! Ataques: o agente e a extensão não leem nem mudam a configuração; travado, nada sai;
//! a resposta não leva o hash do PIN; trocar o PIN sem o PIN atual e a senha sem a senha
//! atual não muda nada; cada mudança grava geração nova (D39).
//!
//! Cofre de teste próprio, só com valores falsos (nunca o cofre real nem o de `.dev/`).

mod comum;

use comum::*;
use ward_core::limits::{DEFAULT_LOCK_MINUTES, MAX_LOCK_MINUTES, MIN_LOCK_MINUTES};
use ward_core::protocolo::{ErroResposta, Papel, Resposta};

fn configuracao(d: &wardd::nucleo::Daemon) -> (u32, bool) {
    match pedir(d, Papel::App, "configuracao", "{}") {
        Resposta::Configuracao {
            minutos_trava,
            backup_externo,
            backup_local,
        } => {
            // O cofre de teste tem a pasta de backups 0700 (Fase 49a).
            assert!(backup_local);
            (minutos_trava, backup_externo)
        }
        outra => panic!("{outra:?}"),
    }
}

fn tempo_trava(d: &wardd::nucleo::Daemon, papel: Papel, minutos: u32) -> Resposta {
    pedir(
        d,
        papel,
        "tempo_trava",
        &format!(r#"{{"minutos":{minutos}}}"#),
    )
}

fn trocar_pin(d: &wardd::nucleo::Daemon, papel: Papel, atual: &str, novo: &str) -> Resposta {
    pedir(
        d,
        papel,
        "trocar_pin",
        &format!(
            r#"{{"atual":"{}","novo":"{}"}}"#,
            b64(atual.as_bytes()),
            b64(novo.as_bytes())
        ),
    )
}

fn trocar_senha(d: &wardd::nucleo::Daemon, papel: Papel, atual: &str, nova: &str) -> Resposta {
    pedir(
        d,
        papel,
        "trocar_senha",
        &format!(
            r#"{{"atual":"{}","nova":"{}"}}"#,
            b64(atual.as_bytes()),
            b64(nova.as_bytes())
        ),
    )
}

/// V5.22: padrão 4 h e backup externo desligado (D30); o tempo novo vale depois de
/// travar e destravar (está no corpo cifrado, não só na memória) e grava geração nova.
#[test]
fn v5_22_padrao_4h_desligado_e_tempo_alteravel_gravado_no_cofre() {
    let dir = Dir::novo("cfg-padrao");
    let (d, _) = cofre_de_teste(&dir);
    assert_eq!(configuracao(&d), (DEFAULT_LOCK_MINUTES, false));
    assert_eq!(DEFAULT_LOCK_MINUTES, 240);
    let antes = geracao_no_disco(&dir.caminhos().cofre, SENHA);
    assert!(ok(&tempo_trava(&d, Papel::App, 30)));
    assert_eq!(configuracao(&d), (30, false));
    assert_eq!(geracao_no_disco(&dir.caminhos().cofre, SENHA), antes + 1);
    d.travar();
    assert!(ok(&destravar(&d, SENHA)));
    assert_eq!(configuracao(&d), (30, false));
    // Os extremos de D45 passam; fora deles nem chega ao daemon.
    assert!(ok(&tempo_trava(&d, Papel::App, MIN_LOCK_MINUTES)));
    assert!(ok(&tempo_trava(&d, Papel::App, MAX_LOCK_MINUTES)));
    for fora in [0, MAX_LOCK_MINUTES + 1, u32::MAX] {
        assert_eq!(
            erro(&tempo_trava(&d, Papel::App, fora)),
            Some(ErroResposta::PedidoInvalido),
            "{fora}"
        );
    }
    assert_eq!(configuracao(&d), (MAX_LOCK_MINUTES, false));
}

/// Ataque (S9, D5): o agente e a extensão não leem a configuração nem mudam PIN, senha
/// ou trava. Nada muda.
#[test]
fn ataque_agente_e_extensao_nao_leem_nem_mudam_a_configuracao() {
    let dir = Dir::novo("cfg-papel");
    let (d, _) = cofre_de_teste(&dir);
    let geracao = geracao_no_disco(&dir.caminhos().cofre, SENHA);
    for papel in [Papel::Agente, Papel::Extensao] {
        let r = pedir(&d, papel, "configuracao", "{}");
        assert_eq!(erro(&r), Some(ErroResposta::NaoAutorizado), "{papel:?}");
        let r = tempo_trava(&d, papel, 1);
        assert_eq!(erro(&r), Some(ErroResposta::NaoAutorizado), "{papel:?}");
        let r = trocar_pin(&d, papel, PIN, "2468");
        assert_eq!(erro(&r), Some(ErroResposta::NaoAutorizado), "{papel:?}");
        let r = trocar_senha(&d, papel, SENHA, "falso-nova-mestra-02");
        assert_eq!(erro(&r), Some(ErroResposta::NaoAutorizado), "{papel:?}");
    }
    assert_eq!(configuracao(&d), (DEFAULT_LOCK_MINUTES, false));
    assert_eq!(geracao_no_disco(&dir.caminhos().cofre, SENHA), geracao);
    // O PIN e a senha continuam os mesmos.
    assert!(ok(&mudar_nivel(&d, Papel::App, "t-aprov", "livre", PIN)));
    d.travar();
    assert!(ok(&destravar(&d, SENHA)));
}

/// Ataque: travado, a configuração não sai (nem para o app).
#[test]
fn ataque_configuracao_com_o_cofre_travado() {
    let dir = Dir::novo("cfg-travado");
    let (d, _) = cofre_de_teste(&dir);
    d.travar();
    for (op, dados) in [
        ("configuracao", "{}".to_owned()),
        ("tempo_trava", r#"{"minutos":30}"#.to_owned()),
    ] {
        let r = pedir(&d, Papel::App, op, &dados);
        assert_eq!(erro(&r), Some(ErroResposta::Travado), "{op}");
    }
    assert_eq!(
        erro(&trocar_pin(&d, Papel::App, PIN, "2468")),
        Some(ErroResposta::Travado)
    );
}

/// Ataque (D30, S1): a resposta `configuracao` não leva o hash do PIN, o sal, o PIN nem
/// a senha; só os três campos (Fase 49a: `backup_local`).
#[test]
fn ataque_resposta_configuracao_sem_hash_nem_pin() {
    let dir = Dir::novo("cfg-hash");
    let (d, _) = cofre_de_teste(&dir);
    let r = pedir(&d, Papel::App, "configuracao", "{}");
    let bytes = codificado(Papel::App, &r);
    let texto = String::from_utf8(bytes.clone()).unwrap();
    for agulha in [
        "argon2",
        "$m=",
        "hash",
        "uuid",
        PIN,
        SENHA,
        &b64(PIN.as_bytes()),
    ] {
        assert!(!contem(&bytes, agulha.as_bytes()), "{agulha}: {texto}");
    }
    assert!(
        texto.ends_with(
            r#""dados":{"minutos_trava":240,"backup_externo":false,"backup_local":true}}"#
        ),
        "{texto}"
    );
}

/// V5.22 (S9): trocar o PIN pede o PIN atual. Errado não muda nada e conta para a trava
/// do 5.º erro; certo grava geração nova, e daí só o PIN novo aprova.
#[test]
fn v5_22_trocar_pin_pede_o_pin_atual() {
    let dir = Dir::novo("cfg-pin");
    let (d, _) = cofre_de_teste(&dir);
    let geracao = geracao_no_disco(&dir.caminhos().cofre, SENHA);
    assert_eq!(
        erro(&trocar_pin(&d, Papel::App, PIN_ERRADO, "2468")),
        Some(ErroResposta::PinIncorreto)
    );
    assert_eq!(geracao_no_disco(&dir.caminhos().cofre, SENHA), geracao);
    assert!(ok(&mudar_nivel(&d, Papel::App, "t-aprov", "livre", PIN)));
    assert!(ok(&trocar_pin(&d, Papel::App, PIN, "2468")));
    assert_eq!(
        geracao_no_disco(&dir.caminhos().cofre, SENHA),
        geracao + 2,
        "mudar_nivel e trocar_pin gravam"
    );
    assert_eq!(
        erro(&mudar_nivel(
            &d,
            Papel::App,
            "t-aprov",
            "com-aprovacao",
            PIN
        )),
        Some(ErroResposta::PinIncorreto)
    );
    assert!(ok(&mudar_nivel(
        &d,
        Papel::App,
        "t-aprov",
        "com-aprovacao",
        "2468"
    )));
    // O PIN novo está no disco: vale depois de travar e destravar.
    d.travar();
    assert!(ok(&destravar(&d, SENHA)));
    assert!(ok(&mudar_nivel(&d, Papel::App, "t-aprov", "livre", "2468")));
    // PIN novo fora do formato nem chega ao daemon.
    for novo in ["135", "1357901", "13579a", ""] {
        assert_eq!(
            erro(&trocar_pin(&d, Papel::App, "2468", novo)),
            Some(ErroResposta::PedidoInvalido),
            "{novo:?}"
        );
    }
}

/// Ataque (S9): adivinhar o PIN atual pela troca trava no 5.º erro, como o
/// `mudar_nivel`.
#[test]
fn ataque_adivinhar_pin_pela_troca_trava_no_quinto_erro() {
    let dir = Dir::novo("cfg-pin5");
    let (d, _) = cofre_de_teste(&dir);
    for i in 0..5 {
        let r = trocar_pin(&d, Papel::App, &format!("00{i:02}"), "2468");
        assert_eq!(erro(&r), Some(ErroResposta::PinIncorreto));
    }
    assert!(d.travado());
    assert!(ok(&destravar(&d, SENHA)));
    // Nada mudou: o PIN é o antigo.
    assert!(ok(&mudar_nivel(&d, Papel::App, "t-aprov", "livre", PIN)));
}

/// V5.22: trocar a senha mestra pede a atual. Errada não muda nada; certa grava geração
/// nova e o cofre abre só com a nova; o tempo de trava e o PIN continuam.
#[test]
fn v5_22_trocar_senha_pede_a_atual_e_mantem_a_configuracao() {
    let dir = Dir::novo("cfg-senha");
    let (d, _) = cofre_de_teste(&dir);
    assert!(ok(&tempo_trava(&d, Papel::App, 60)));
    let geracao = geracao_no_disco(&dir.caminhos().cofre, SENHA);
    assert_eq!(
        erro(&trocar_senha(
            &d,
            Papel::App,
            "falso-errada-mestra-01",
            "falso-nova-mestra-02"
        )),
        Some(ErroResposta::SenhaIncorreta)
    );
    assert!(!d.travado());
    assert_eq!(geracao_no_disco(&dir.caminhos().cofre, SENHA), geracao);
    assert!(ok(&trocar_senha(
        &d,
        Papel::App,
        SENHA,
        "falso-nova-mestra-02"
    )));
    assert!(geracao_no_disco(&dir.caminhos().cofre, "falso-nova-mestra-02") > geracao);
    d.travar();
    assert_eq!(
        erro(&destravar(&d, SENHA)),
        Some(ErroResposta::SenhaIncorreta)
    );
    assert!(ok(&destravar(&d, "falso-nova-mestra-02")));
    assert_eq!(configuracao(&d), (60, false));
    assert!(ok(&mudar_nivel(&d, Papel::App, "t-aprov", "livre", PIN)));
}

/// Fase 39c: o mínimo de 12 caracteres da senha mestra vale também no daemon, contado
/// depois do NFC. Ataque: um cliente do papel app que pule a tela manda senha curta, ou
/// 11 letras com acento decompostas (22 pontos de código). Nada é gravado, nenhuma
/// geração é queimada e a senha atual continua valendo.
#[test]
fn ataque_senha_mestra_nova_curta_recusada_pelo_daemon() {
    let campo = |r: &Resposta| match r {
        Resposta::Erro(ErroResposta::Validacao(p)) => {
            assert!(p.iter().all(|p| p.motivo.contains("12 caracteres")));
            p.iter().map(|p| p.campo.clone()).collect::<Vec<_>>()
        }
        outra => panic!("{outra:?}"),
    };
    let disfarcada = "e\u{301}".repeat(11);

    // Criar o cofre.
    let dir = Dir::novo("cfg-senha-curta");
    let c = dir.caminhos();
    let (d, _) = daemon_em(c.clone());
    // (Vazia nem passa do protocolo: `pedido_invalido`.)
    for curta in ["falso-mest1", disfarcada.as_str()] {
        let r = pedir(
            &d,
            Papel::App,
            "criar_cofre",
            &format!(
                r#"{{"senha":"{}","pin":"{}"}}"#,
                b64(curta.as_bytes()),
                b64(PIN.as_bytes())
            ),
        );
        assert_eq!(campo(&r), ["senha"], "{curta:?}");
        assert!(!c.cofre.exists());
        assert!(!c.estado.exists());
    }

    // Trocar a senha.
    let dir = Dir::novo("cfg-senha-curta-troca");
    let (d, _) = cofre_de_teste(&dir);
    let antes = geracoes(&dir);
    for curta in ["falso-mest1", disfarcada.as_str()] {
        assert_eq!(campo(&trocar_senha(&d, Papel::App, SENHA, curta)), ["nova"]);
    }
    assert_eq!(geracoes(&dir), antes);
    d.travar();
    assert!(ok(&destravar(&d, SENHA)));
}

/// Fase 39c: a senha mestra criada composta abre decomposta (e o contrário), pelo
/// mesmo socket que o app e a CLI usam.
#[test]
fn senha_mestra_composta_abre_decomposta_pelo_daemon() {
    let composta = "falso-mestra-\u{e9}\u{e7}\u{e3}o-01";
    let decomposta = "falso-mestra-e\u{301}c\u{327}a\u{303}o-01";
    let dir = Dir::novo("cfg-senha-nfc");
    let (d, _) = cofre_de_teste(&dir);
    assert!(ok(&trocar_senha(&d, Papel::App, SENHA, decomposta)));
    d.travar();
    assert!(ok(&destravar(&d, composta)));
    d.travar();
    assert!(ok(&destravar(&d, decomposta)));
    assert!(ok(&trocar_senha(&d, Papel::App, composta, SENHA)));
}

/// Fase 55 (reauditoria final, baixo): a troca de senha lia o cofre de novo pela
/// biblioteca padrão (`File::open`, segue link e espera o outro lado de um FIFO),
/// depois de conferir a identidade por outro `open`. Um FIFO trocado no lugar do
/// arquivo entre as duas aberturas prendia a thread com o estado do daemon travado.
/// Agora a troca usa os bytes do mesmo fd da conferência (`O_NOFOLLOW | O_NONBLOCK`).
/// Ataque: um FIFO entra e sai do lugar do cofre sem parar enquanto o app pede a troca
/// (com a senha atual errada, para nada ser gravado); cada pedido volta logo.
#[test]
fn f55_ataque_fifo_no_lugar_do_cofre_nao_prende_a_troca_de_senha() {
    use std::fs;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    let dir = Dir::novo("troca-fifo");
    let (d, _) = cofre_de_teste(&dir);
    let d = Arc::new(d);
    let cofre = dir.caminhos().cofre;
    let real = dir.0.join("x-real");
    fs::hard_link(&cofre, &real).unwrap();

    // FIFO parado no lugar: recusa na hora e trava (o arquivo mudou fora do Ward).
    let fifo = dir.0.join("x-fifo");
    let st = std::process::Command::new("/usr/bin/mkfifo")
        .args(["-m", "600"])
        .arg(&fifo)
        .status()
        .unwrap();
    assert!(st.success());
    fs::rename(&fifo, &cofre).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    {
        let d = d.clone();
        std::thread::spawn(move || {
            let _ = tx.send(trocar_senha(&d, Papel::App, SENHA, "falso-nova-mestra-55"));
        });
    }
    let r = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("a troca de senha ficou presa no FIFO");
    assert_eq!(erro(&r), Some(ErroResposta::CofreMudou), "{r:?}");
    assert!(d.travado());
    let volta = dir.0.join("x-volta");
    fs::hard_link(&real, &volta).unwrap();
    fs::rename(&volta, &cofre).unwrap();
    assert!(ok(&destravar(&d, SENHA)));

    // FIFO entrando e saindo do lugar durante os pedidos.
    let parar = Arc::new(AtomicBool::new(false));
    let trocador = {
        let (dir0, cofre, real, parar) =
            (dir.0.clone(), cofre.clone(), real.clone(), parar.clone());
        std::thread::spawn(move || {
            let mut n = 0u64;
            while !parar.load(Ordering::Acquire) {
                let fifo = dir0.join(format!("x-fifo-{n}"));
                let st = std::process::Command::new("/usr/bin/mkfifo")
                    .args(["-m", "600"])
                    .arg(&fifo)
                    .status()
                    .unwrap();
                assert!(st.success());
                fs::rename(&fifo, &cofre).unwrap();
                std::thread::sleep(Duration::from_millis(3));
                let volta = dir0.join(format!("x-volta-{n}"));
                fs::hard_link(&real, &volta).unwrap();
                fs::rename(&volta, &cofre).unwrap();
                std::thread::sleep(Duration::from_millis(3));
                n += 1;
            }
            n
        })
    };
    let fim = Instant::now() + Duration::from_secs(6);
    let mut pedidos = 0;
    let mut lidos = 0;
    while Instant::now() < fim {
        let (tx, rx) = std::sync::mpsc::channel();
        let d2 = d.clone();
        std::thread::spawn(move || {
            let r = trocar_senha(
                &d2,
                Papel::App,
                "falso-senha-errada-55",
                "falso-nova-mestra-55",
            );
            if erro(&r) != Some(ErroResposta::SenhaIncorreta) {
                // Travou (o arquivo mudou ou erros demais): destrava de novo (o
                // destravar também lê sem seguir FIFO; tenta até pegar o real).
                for _ in 0..50 {
                    if ok(&destravar(&d2, SENHA)) {
                        break;
                    }
                }
            }
            let _ = tx.send(r);
        });
        let r = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("a troca de senha ficou presa num FIFO (o daemon parou)");
        assert!(
            matches!(
                erro(&r),
                Some(
                    ErroResposta::SenhaIncorreta | ErroResposta::CofreMudou | ErroResposta::Travado
                )
            ),
            "{r:?}"
        );
        if erro(&r) == Some(ErroResposta::SenhaIncorreta) {
            lidos += 1;
        }
        pedidos += 1;
    }
    parar.store(true, Ordering::Release);
    let trocas = trocador.join().unwrap();
    assert!(
        trocas > 50 && pedidos > 5 && lidos >= 3,
        "{trocas} trocas, {pedidos} pedidos, {lidos} lidos"
    );
    // O arquivo real no lugar: a troca com a senha certa funciona.
    if d.travado() {
        assert!(ok(&destravar(&d, SENHA)));
    }
    let r = trocar_senha(&d, Papel::App, SENHA, "falso-nova-mestra-55");
    assert!(ok(&r), "{r:?}");
}
