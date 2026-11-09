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
use ward_core::limites::{MAX_MINUTOS_TRAVA, MIN_MINUTOS_TRAVA, MINUTOS_TRAVA_PADRAO};
use ward_core::protocolo::{ErroResposta, Papel, Resposta};

fn configuracao(d: &wardd::nucleo::Daemon) -> (u32, bool) {
    match pedir(d, Papel::App, "configuracao", "{}") {
        Resposta::Configuracao {
            minutos_trava,
            backup_externo,
        } => (minutos_trava, backup_externo),
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
    assert_eq!(configuracao(&d), (MINUTOS_TRAVA_PADRAO, false));
    assert_eq!(MINUTOS_TRAVA_PADRAO, 240);
    let antes = geracao_no_disco(&dir.caminhos().cofre, SENHA);
    assert!(ok(&tempo_trava(&d, Papel::App, 30)));
    assert_eq!(configuracao(&d), (30, false));
    assert_eq!(geracao_no_disco(&dir.caminhos().cofre, SENHA), antes + 1);
    d.travar();
    assert!(ok(&destravar(&d, SENHA)));
    assert_eq!(configuracao(&d), (30, false));
    // Os extremos de D45 passam; fora deles nem chega ao daemon.
    assert!(ok(&tempo_trava(&d, Papel::App, MIN_MINUTOS_TRAVA)));
    assert!(ok(&tempo_trava(&d, Papel::App, MAX_MINUTOS_TRAVA)));
    for fora in [0, MAX_MINUTOS_TRAVA + 1, u32::MAX] {
        assert_eq!(
            erro(&tempo_trava(&d, Papel::App, fora)),
            Some(ErroResposta::PedidoInvalido),
            "{fora}"
        );
    }
    assert_eq!(configuracao(&d), (MAX_MINUTOS_TRAVA, false));
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
    assert_eq!(configuracao(&d), (MINUTOS_TRAVA_PADRAO, false));
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
/// a senha; só os dois campos.
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
        texto.ends_with(r#""dados":{"minutos_trava":240,"backup_externo":false}}"#),
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
