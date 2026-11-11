//! Backup local e restaurar no daemon (Fase 45; D17, D27, D39): cada gravação vai para
//! o backup (V7.1), restaurar só pelo app (V7.8), versão restaurada selada com geração
//! nova e a cópia à mão em retrocesso (V7.11), pasta trocada por link, PIN e senha da
//! versão, cofre trocado fora do Ward, estado que não grava (S7, S9) e permissões.
//!
//! Cofre de teste próprio, só com valores falsos (nunca o cofre real nem o de `.dev/`).

mod common;

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use common::*;
use ward_core::backup::Version;
use ward_core::entry::Secret;
use ward_core::protocolo::{ErroResposta, MSG_RETROCESSO, Papel, Pedido, Resposta, VersaoBackup};
use wardd::arquivos::Geracoes;
use wardd::nucleo::Daemon;

fn pasta(dir: &Dir) -> PathBuf {
    dir.0.join("backups")
}

fn modo(p: &Path) -> u32 {
    fs::symlink_metadata(p).unwrap().permissions().mode() & 0o7777
}

/// Os nomes das versões na pasta, da mais nova para a mais antiga (pelo app).
fn backups(d: &Daemon) -> Vec<String> {
    versoes(d).into_iter().map(|v| v.nome).collect()
}

/// As versões com a geração selada (Fase 49a).
fn versoes(d: &Daemon) -> Vec<VersaoBackup> {
    match request(d, Papel::App, "backups", "{}") {
        Resposta::Backups(l) => l,
        outra => panic!("{outra:?}"),
    }
}

fn restaurar(d: &Daemon, versao: &str, senha: &str, pin: &str) -> Resposta {
    request(
        d,
        Papel::App,
        "restaurar_backup",
        &format!(
            r#"{{"versao":"{versao}","senha":"{}","pin":"{}"}}"#,
            b64(senha.as_bytes()),
            b64(pin.as_bytes())
        ),
    )
}

fn nomes_da_lista(d: &Daemon) -> Vec<String> {
    match request(d, Papel::App, "lista", "{}") {
        Resposta::Lista(l) => l.into_iter().map(|i| i.nome).collect(),
        outra => panic!("{outra:?}"),
    }
}

fn tempo_trava(d: &Daemon, minutos: u32) {
    let r = request(
        d,
        Papel::App,
        "tempo_trava",
        &format!(r#"{{"minutos":{minutos}}}"#),
    );
    assert!(ok(&r), "{r:?}");
}

/// V7.1 (D17): cada gravação do daemon (criar, salvar, mudar nível) deixa uma versão na
/// pasta `backups/` ao lado do cofre, 0700, com arquivos 0600 iguais ao cofre gravado e
/// sem segredo em claro.
#[test]
fn v7_1_cada_gravacao_do_daemon_vai_para_o_backup() {
    let dir = Dir::new("bk-cada");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    // criar_cofre + 5 entradas + 2 mudanças de nível.
    let lista = backups(&d);
    assert_eq!(lista.len(), 8, "{lista:?}");
    assert_eq!(modo(&pasta(&dir)), 0o700);
    for n in &lista {
        assert!(Version::from_name(n).is_some(), "{n}");
        let p = pasta(&dir).join(n);
        assert_eq!(modo(&p), 0o600, "{n}");
        let b = fs::read(&p).unwrap();
        for s in SECRETS.iter().chain([PASSWORD, PIN].iter()) {
            assert!(!contains(&b, s.as_bytes()), "segredo em claro em {n}");
        }
    }
    assert_eq!(
        fs::read(pasta(&dir).join(&lista[0])).unwrap(),
        fs::read(&c.cofre).unwrap(),
        "a mais nova é o cofre atual"
    );
    // A geração de cada versão: 1 (criar) até 8, da mais antiga para a mais nova.
    let mut g: Vec<u64> = lista
        .iter()
        .map(|n| generation_on_disk(&pasta(&dir).join(n), PASSWORD))
        .collect();
    g.reverse();
    assert_eq!(g, (1..=8).collect::<Vec<_>>());
    // A resposta ao app tem só nomes e as marcas (Fase 49a): todas conferidas e em
    // ordem; sem caminho nem geração (D39).
    let v = versoes(&d);
    assert!(v.iter().all(|x| x.conferida && !x.fora_de_ordem), "{v:?}");
    let bytes = encoded(Papel::App, &Resposta::Backups(v));
    assert!(!contains(&bytes, dir.0.to_string_lossy().as_bytes()));
    assert!(!contains(&bytes, b"geracao"));
}

/// V7.8 (D17): o agente e a extensão pedindo restaurar ou listar backups recebem "não
/// autorizado" no protocolo e no daemon (segunda camada), com o cofre destravado e o
/// PIN certo; nada muda no disco.
#[test]
fn v7_8_agente_e_extensao_nao_restauram() {
    let dir = Dir::new("bk-papel");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    let alvo = backups(&d).pop().unwrap();
    let antes = fs::read(&c.cofre).unwrap();
    let estado_antes = fs::read(&c.estado).unwrap();
    for papel in [Papel::Agente, Papel::Extensao] {
        let r = request(
            &d,
            papel,
            "restaurar_backup",
            &format!(
                r#"{{"versao":"{alvo}","senha":"{}","pin":"{}"}}"#,
                b64(PASSWORD.as_bytes()),
                b64(PIN.as_bytes())
            ),
        );
        assert_eq!(
            error(&r),
            Some(ErroResposta::NaoAutorizado),
            "{}",
            papel.nome()
        );
        let r = request(&d, papel, "backups", "{}");
        assert_eq!(
            error(&r),
            Some(ErroResposta::NaoAutorizado),
            "{}",
            papel.nome()
        );
        // Segunda camada: o pedido montado em memória, sem passar pelo protocolo.
        let r = d.atender(
            papel,
            Pedido::RestaurarBackup {
                versao: alvo.clone(),
                senha: Secret::from_text(PASSWORD.to_owned()),
                pin: Secret::from_text(PIN.to_owned()),
            },
        );
        assert_eq!(
            error(&r),
            Some(ErroResposta::NaoAutorizado),
            "{}",
            papel.nome()
        );
        assert_eq!(
            error(&d.atender(papel, Pedido::Backups)),
            Some(ErroResposta::NaoAutorizado)
        );
    }
    assert_eq!(fs::read(&c.cofre).unwrap(), antes);
    assert_eq!(fs::read(&c.estado).unwrap(), estado_antes);
    assert!(!d.travado());
}

/// V7.11 (D39, D17; S7, S9): cofre na geração n+2 e o backup da geração n. Restaurar
/// pelo app sela a versão de novo na n+3 (cofre e estado), o atual (n+2) vira backup, o
/// daemon destrava com ela e as entradas são as da versão n. O mesmo backup copiado à
/// mão por cima do cofre cai no retrocesso (V3.22), também num daemon novo.
#[test]
fn v7_11_restaurar_sela_geracao_nova_e_copia_a_mao_e_retrocesso() {
    let dir = Dir::new("bk-v7-11");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    let n = generation_on_disk(&c.cofre, PASSWORD);
    let versao_n = backups(&d)[0].clone();
    assert_eq!(
        generation_on_disk(&pasta(&dir).join(&versao_n), PASSWORD),
        n
    );
    let copia_n = fs::read(pasta(&dir).join(&versao_n)).unwrap();
    let entradas_n = nomes_da_lista(&d);

    // Duas gravações: uma entrada nova e o tempo de trava.
    let r = request(
        &d,
        Papel::App,
        "salvar_entrada",
        &login("t-nova", SECRETS[0]),
    );
    assert!(ok(&r), "{r:?}");
    tempo_trava(&d, 30);
    assert_eq!(generation_on_disk(&c.cofre, PASSWORD), n + 2);
    let atual_n2 = fs::read(&c.cofre).unwrap();
    assert_ne!(nomes_da_lista(&d), entradas_n);

    let r = restaurar(&d, &versao_n, PASSWORD, PIN);
    assert!(ok(&r), "{r:?}");
    assert_eq!(generation_on_disk(&c.cofre, PASSWORD), n + 3);
    assert_eq!(
        generations(&dir),
        Geracoes {
            vista: n + 3,
            queimada: n + 3
        }
    );
    assert_eq!(nomes_da_lista(&d), entradas_n);
    // O atual de antes (n+2) ficou na pasta, igual byte a byte, e a versão nova também.
    let lista = backups(&d);
    assert_eq!(
        fs::read(pasta(&dir).join(&lista[0])).unwrap(),
        fs::read(&c.cofre).unwrap()
    );
    assert!(
        lista
            .iter()
            .any(|v| fs::read(pasta(&dir).join(v)).unwrap() == atual_n2),
        "o cofre de antes não virou backup"
    );
    // A versão n restaurada continua na pasta, sem mudar.
    assert_eq!(fs::read(pasta(&dir).join(&versao_n)).unwrap(), copia_n);

    // Trava e destrava com a senha da versão: as entradas da versão n.
    d.travar();
    let r = unlock(&d, PASSWORD);
    assert!(ok(&r), "{r:?}");
    assert_eq!(nomes_da_lista(&d), entradas_n);
    d.travar();

    // Cópia à mão do mesmo backup: a geração n é menor que a vista (n+3).
    fs::write(&c.cofre, &copia_n).unwrap();
    let r = unlock(&d, PASSWORD);
    assert_eq!(error(&r), Some(ErroResposta::Retrocesso));
    assert!(contains(
        &encoded(Papel::Agente, &r),
        MSG_RETROCESSO.as_bytes()
    ));
    assert!(d.travado());
    // Num daemon novo, direto o retrocesso; o arquivo copiado não é regravado.
    drop(d);
    let (d2, _) = daemon_at(dir.paths());
    assert_eq!(
        error(&unlock(&d2, PASSWORD)),
        Some(ErroResposta::Retrocesso)
    );
    assert_eq!(fs::read(&c.cofre).unwrap(), copia_n);
}

/// S9: restaurar exige o cofre destravado e o PIN atual (o erro conta para os 5 que
/// travam) e a senha da versão; nome que não existe, senha errada ou PIN errado não
/// mexem em nada.
#[test]
fn ataque_restaurar_sem_pin_senha_errada_e_travado() {
    let dir = Dir::new("bk-pin");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    let alvo = backups(&d).pop().unwrap();
    let antes = fs::read(&c.cofre).unwrap();
    let estado = fs::read(&c.estado).unwrap();
    let n_backups = backups(&d).len();

    let r = restaurar(&d, &alvo, PASSWORD, WRONG_PIN);
    assert_eq!(error(&r), Some(ErroResposta::PinIncorreto));
    let r = restaurar(&d, &alvo, "falso-mestra-ward-02", PIN);
    assert_eq!(error(&r), Some(ErroResposta::SenhaIncorreta));
    let r = restaurar(&d, "ward-20000101-000000.ward", PASSWORD, PIN);
    assert_eq!(error(&r), Some(ErroResposta::BackupInexistente));
    let r = restaurar(&d, "../test-vault.ward", PASSWORD, PIN);
    assert_eq!(error(&r), Some(ErroResposta::PedidoInvalido));
    assert_eq!(fs::read(&c.cofre).unwrap(), antes);
    assert_eq!(fs::read(&c.estado).unwrap(), estado);
    assert_eq!(backups(&d).len(), n_backups);

    // 5 PINs errados seguidos pelo restaurar travam o cofre.
    for i in 0..5 {
        let r = restaurar(&d, &alvo, PASSWORD, WRONG_PIN);
        assert_eq!(error(&r), Some(ErroResposta::PinIncorreto), "tentativa {i}");
    }
    assert!(d.travado());
    // Travado: nem lista nem restaura, com o PIN certo.
    assert_eq!(
        error(&request(&d, Papel::App, "backups", "{}")),
        Some(ErroResposta::Travado)
    );
    assert_eq!(
        error(&restaurar(&d, &alvo, PASSWORD, PIN)),
        Some(ErroResposta::Travado)
    );
    assert_eq!(fs::read(&c.cofre).unwrap(), antes);
}

/// A pasta de backups trocada por um link simbólico para outra pasta (com uma cópia
/// autêntica de uma versão antiga): listar e restaurar respondem "indisponível", e a
/// gravação do cofre segue sem escrever nada no alvo do link. O mesmo com a pasta 0755.
#[test]
fn ataque_pasta_de_backups_trocada_por_link() {
    let dir = Dir::new("bk-link");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    let alvo_versao = backups(&d).pop().unwrap();
    let antigo = fs::read(pasta(&dir).join(&alvo_versao)).unwrap();

    let outra = dir.0.join("outra");
    fs::create_dir(&outra).unwrap();
    fs::set_permissions(&outra, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(outra.join(&alvo_versao), &antigo).unwrap();
    fs::set_permissions(outra.join(&alvo_versao), fs::Permissions::from_mode(0o600)).unwrap();
    fs::rename(pasta(&dir), dir.0.join("backups-verdadeira")).unwrap();
    symlink(&outra, pasta(&dir)).unwrap();

    assert_eq!(
        error(&request(&d, Papel::App, "backups", "{}")),
        Some(ErroResposta::BackupIndisponivel)
    );
    let antes = fs::read(&c.cofre).unwrap();
    assert_eq!(
        error(&restaurar(&d, &alvo_versao, PASSWORD, PIN)),
        Some(ErroResposta::BackupIndisponivel)
    );
    assert_eq!(fs::read(&c.cofre).unwrap(), antes);
    // A gravação segue e não escreve no alvo do link.
    tempo_trava(&d, 45);
    assert_ne!(fs::read(&c.cofre).unwrap(), antes);
    let no_alvo: Vec<_> = fs::read_dir(&outra).unwrap().collect();
    assert_eq!(no_alvo.len(), 1, "o backup foi gravado através do link");

    // Pasta de verdade de volta, mas 0755: também recusada.
    fs::remove_file(pasta(&dir)).unwrap();
    fs::rename(dir.0.join("backups-verdadeira"), pasta(&dir)).unwrap();
    fs::set_permissions(pasta(&dir), fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        error(&request(&d, Papel::App, "backups", "{}")),
        Some(ErroResposta::BackupIndisponivel)
    );
    fs::set_permissions(pasta(&dir), fs::Permissions::from_mode(0o700)).unwrap();
    assert!(!backups(&d).is_empty());
}

/// Uma versão na pasta trocada por um link simbólico para uma cópia autêntica fora da
/// pasta: o daemon não segue (`O_NOFOLLOW`) e não restaura; a lista a ignora.
#[test]
fn ataque_versao_trocada_por_link() {
    let dir = Dir::new("bk-versao-link");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    let v = backups(&d).pop().unwrap();
    let fora = dir.0.join("copia-fora.ward");
    fs::rename(pasta(&dir).join(&v), &fora).unwrap();
    symlink(&fora, pasta(&dir).join(&v)).unwrap();
    assert!(!backups(&d).contains(&v));
    let antes = fs::read(&c.cofre).unwrap();
    // Fase 49a: o restaurar só aceita versão da lista (que ignora links); a leitura com
    // `O_NOFOLLOW` continua para a troca entre a lista e a abertura.
    assert_eq!(
        error(&restaurar(&d, &v, PASSWORD, PIN)),
        Some(ErroResposta::BackupInexistente)
    );
    assert_eq!(fs::read(&c.cofre).unwrap(), antes);
    assert!(!d.travado());
}

/// D39: o arquivo do cofre trocado fora do Ward antes do restaurar (outro inode, mesmos
/// bytes): nada é restaurado, o cofre trava e o arquivo trocado fica como está.
#[test]
fn ataque_restaurar_com_cofre_trocado_fora_do_ward() {
    let dir = Dir::new("bk-mudou");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    let v = backups(&d).pop().unwrap();
    let bytes = fs::read(&c.cofre).unwrap();
    let tmp = dir.0.join("troca");
    fs::write(&tmp, &bytes).unwrap();
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600)).unwrap();
    fs::rename(&tmp, &c.cofre).unwrap();
    let n_backups = backups(&d).len();
    assert_eq!(
        error(&restaurar(&d, &v, PASSWORD, PIN)),
        Some(ErroResposta::CofreMudou)
    );
    assert!(d.travado());
    assert_eq!(fs::read(&c.cofre).unwrap(), bytes);
    let (d2, _) = daemon_at(dir.paths());
    assert!(ok(&unlock(&d2, PASSWORD)));
    assert_eq!(backups(&d2).len(), n_backups, "nada novo na pasta");
}

/// S7/D53: com o arquivo de estado sem como gravar, o restaurar não troca o cofre e o
/// daemon segue com as entradas de antes; nada da versão vai para o disco do cofre.
#[test]
fn ataque_restaurar_sem_gravar_o_estado_nao_troca_o_cofre() {
    let dir = Dir::new("bk-estado");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    let v = backups(&d).pop().unwrap();
    let antes = fs::read(&c.cofre).unwrap();
    let lista_antes = nomes_da_lista(&d);
    // O diretório do cofre sem escrita: o temporário do estado não nasce.
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o500)).unwrap();
    let r = restaurar(&d, &v, PASSWORD, PIN);
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(error(&r), Some(ErroResposta::FalhaAoGravar));
    assert_eq!(fs::read(&c.cofre).unwrap(), antes);
    assert!(!d.travado());
    assert_eq!(nomes_da_lista(&d), lista_antes);
    // A próxima gravação comum funciona e a geração não repete.
    let g = generation_on_disk(&c.cofre, PASSWORD);
    tempo_trava(&d, 50);
    assert!(generation_on_disk(&c.cofre, PASSWORD) > g);
}

/// A troca de senha vai para o backup com a senha nova, e restaurar a versão anterior
/// pede a senha antiga (D17: a senha daquela versão).
#[test]
fn restaurar_pede_a_senha_da_versao() {
    let dir = Dir::new("bk-senha");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    let antiga = backups(&d)[0].clone();
    let nova = "falso-mestra-ward-05";
    let r = request(
        &d,
        Papel::App,
        "trocar_senha",
        &format!(
            r#"{{"atual":"{}","nova":"{}"}}"#,
            b64(PASSWORD.as_bytes()),
            b64(nova.as_bytes())
        ),
    );
    assert!(ok(&r), "{r:?}");
    let depois = backups(&d)[0].clone();
    assert_ne!(depois, antiga);
    assert_eq!(
        fs::read(pasta(&dir).join(&depois)).unwrap(),
        fs::read(&c.cofre).unwrap()
    );
    // A versão antiga só abre com a senha antiga.
    assert_eq!(
        error(&restaurar(&d, &antiga, nova, PIN)),
        Some(ErroResposta::SenhaIncorreta)
    );
    assert!(ok(&restaurar(&d, &antiga, PASSWORD, PIN)));
    d.travar();
    assert_eq!(error(&unlock(&d, nova)), Some(ErroResposta::SenhaIncorreta));
    let (d2, _) = daemon_at(dir.paths());
    assert!(ok(&unlock(&d2, PASSWORD)));
}

// ---------------------------------------------------------------- Fase 49a

fn versao_em(t: SystemTime) -> String {
    Version::from_instant(t).unwrap().name()
}

fn plantar(dir: &Dir, nome: &str, bytes: &[u8]) {
    let p = pasta(dir).join(nome);
    fs::write(&p, bytes).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
}

/// Fase 49a (achado médio do M7): 30 cópias autênticas antigas plantadas com nome de
/// data futura (qualquer processo do usuário pode escrever na pasta) não fazem o daemon
/// apagar as versões reais na próxima gravação; as de depois da margem não aparecem na
/// lista nem restauram; as dentro da margem aparecem com a geração antiga, que as
/// denuncia (a data do nome não é confiável, a geração é autenticada).
#[test]
fn ataque_versoes_com_data_futura_nao_apagam_as_reais_nem_restauram() {
    let dir = Dir::new("bk-futuro");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    let reais = backups(&d);
    assert_eq!(reais.len(), 8);
    let antiga = fs::read(pasta(&dir).join(reais.last().unwrap())).unwrap();
    let agora = SystemTime::now();
    let mut longe = Vec::new();
    let mut perto = Vec::new();
    for i in 0..30u64 {
        let n = versao_em(agora + Duration::from_secs(10 * 365 * 86_400 + i * 3600));
        plantar(&dir, &n, &antiga);
        longe.push(n);
        let n = versao_em(agora + Duration::from_secs(60 + i));
        plantar(&dir, &n, &antiga);
        perto.push(n);
    }
    // Uma gravação de verdade: a rotação não apaga nenhuma real nem plantada.
    tempo_trava(&d, 45);
    for n in reais.iter().chain(&longe).chain(&perto) {
        assert!(pasta(&dir).join(n).exists(), "{n} sumiu");
    }
    let v = versoes(&d);
    // As de depois da margem não aparecem; as de dentro saem marcadas (geração 1, a
    // antiga, menor que a das reais de data anterior); as reais, não.
    assert!(v.iter().all(|x| !longe.contains(&x.nome)));
    assert_eq!(v.len(), reais.len() + 1 + perto.len());
    for x in &v {
        assert!(x.conferida, "{}", x.nome);
        assert_eq!(x.fora_de_ordem, perto.contains(&x.nome), "{}", x.nome);
    }
    // Restaurar uma de depois da margem: inexistente, nada muda.
    let antes = fs::read(&c.cofre).unwrap();
    let estado = fs::read(&c.estado).unwrap();
    assert_eq!(
        error(&restaurar(&d, &longe[0], PASSWORD, PIN)),
        Some(ErroResposta::BackupInexistente)
    );
    assert_eq!(fs::read(&c.cofre).unwrap(), antes);
    assert_eq!(fs::read(&c.estado).unwrap(), estado);
    // 40 gravações a mais: ficam 30 com data até agora; as de 2036 continuam lá.
    for i in 0..40u32 {
        tempo_trava(&d, 30 + (i % 2));
    }
    let ate_agora = fs::read_dir(pasta(&dir))
        .unwrap()
        .filter_map(|e| Version::from_name(&e.unwrap().file_name().into_string().unwrap()))
        .filter(|v| {
            *v <= Version::from_instant(SystemTime::now())
                .unwrap()
                .end_of_second()
        })
        .count();
    assert_eq!(ate_agora, ward_core::backup::VERSIONS);
    for n in &longe {
        assert!(pasta(&dir).join(n).exists(), "{n} sumiu");
    }
}

/// Fase 49a (achado baixo): restaurar uma versão antiga mantém o tempo de trava e o
/// backup externo de agora (a versão traz os dela; restaurar não desliga a cópia para
/// o pendrive sem aviso). Entradas, senha e PIN são os da versão.
#[test]
fn restaurar_mantem_tempo_de_trava_e_backup_externo_atuais() {
    let dir = Dir::new("bk-preserva");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    let versao = backups(&d)[0].clone();
    let entradas = nomes_da_lista(&d);
    tempo_trava(&d, 30);
    let r = request(
        &d,
        Papel::App,
        "definir_backup_externo",
        r#"{"ligado":true,"uuid":"ABCD-1234"}"#,
    );
    assert!(ok(&r), "{r:?}");
    assert!(ok(&restaurar(&d, &versao, PASSWORD, PIN)));
    assert_eq!(nomes_da_lista(&d), entradas);
    match request(&d, Papel::App, "configuracao", "{}") {
        Resposta::Configuracao {
            minutos_trava,
            backup_externo,
            backup_local,
        } => {
            assert_eq!(minutos_trava, 30);
            assert!(backup_externo);
            assert!(backup_local);
        }
        r => panic!("{r:?}"),
    }
    match request(&d, Papel::App, "backup_externo", "{}") {
        Resposta::BackupExterno { ligado, uuid } => {
            assert!(ligado);
            assert_eq!(uuid.as_deref(), Some("ABCD-1234"));
        }
        r => panic!("{r:?}"),
    }
    // No disco também (o destravar lê a configuração do arquivo).
    let bytes = fs::read(&c.cofre).unwrap();
    let s = ward_core::format::MasterPassword::new(PASSWORD.to_owned());
    let (aberto, _, _) = ward_core::body::open_entries_with_generation(&bytes, &s, 0, 0).unwrap();
    assert_eq!(aberto.config().lock_minutes(), 30);
    assert!(aberto.config().backup().enabled());
}

/// Fase 49a (achado baixo): a pasta de backups insegura vira aviso na tela
/// (`configuracao` com `backup_local: false`), sem caminho; segura de novo, volta.
#[test]
fn pasta_de_backups_insegura_aparece_na_configuracao() {
    let dir = Dir::new("bk-aviso");
    let (d, _) = test_vault(&dir);
    let local = |d: &Daemon| match request(d, Papel::App, "configuracao", "{}") {
        Resposta::Configuracao { backup_local, .. } => backup_local,
        r => panic!("{r:?}"),
    };
    assert!(local(&d));
    for m in [0o755, 0o750, 0o707] {
        fs::set_permissions(pasta(&dir), fs::Permissions::from_mode(m)).unwrap();
        assert!(!local(&d), "{m:o}");
    }
    fs::set_permissions(pasta(&dir), fs::Permissions::from_mode(0o700)).unwrap();
    assert!(local(&d));
    // Trocada por link para outra pasta 0700: insegura.
    let outra = dir.0.join("outra");
    fs::create_dir(&outra).unwrap();
    fs::set_permissions(&outra, fs::Permissions::from_mode(0o700)).unwrap();
    fs::rename(pasta(&dir), dir.0.join("verdadeira")).unwrap();
    symlink(&outra, pasta(&dir)).unwrap();
    assert!(!local(&d));
    let bytes = encoded(Papel::App, &request(&d, Papel::App, "configuracao", "{}"));
    assert!(!contains(&bytes, dir.0.to_string_lossy().as_bytes()));
}

/// Fase 49a (achado baixo): os temporários do arquivo de estado de uma gravação
/// interrompida somem na partida; o que não é exatamente um deles (link, pasta, nome
/// parecido, o de outro arquivo) fica.
#[test]
fn temporarios_do_estado_somem_na_partida() {
    let dir = Dir::new("bk-estado-tmp");
    let c = dir.paths();
    let (d, _) = test_vault(&dir);
    drop(d);
    let pai = c.estado.parent().unwrap().to_path_buf();
    let nome = c.estado.file_name().unwrap().to_string_lossy().into_owned();
    let velho = pai.join(format!(".{nome}.tmp-4194301-7"));
    let velho2 = pai.join(format!(".{nome}.tmp-12-0"));
    fs::write(&velho, b"vista 3\nqueimada 3\n").unwrap();
    fs::write(&velho2, b"x").unwrap();
    let alvo = dir.0.join("alvo-do-link");
    fs::write(&alvo, b"nao apagar").unwrap();
    let link = pai.join(format!(".{nome}.tmp-4194302-1"));
    symlink(&alvo, &link).unwrap();
    let pasta_igual = pai.join(format!(".{nome}.tmp-4194303-1"));
    fs::create_dir(&pasta_igual).unwrap();
    let parecidos = [
        pai.join(format!(".{nome}.tmp-abc-1")),
        pai.join(format!(".{nome}.tmp-1")),
        pai.join(format!(".{nome}.tmp-1-2-3")),
        pai.join(format!("{nome}.tmp-1-2")),
        pai.join(".outro.estado.tmp-1-2"),
    ];
    for p in &parecidos {
        fs::write(p, b"fica").unwrap();
    }
    let (d, _) = daemon_at(dir.paths());
    assert!(!velho.exists() && !velho2.exists());
    assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
    assert_eq!(fs::read(&alvo).unwrap(), b"nao apagar");
    assert!(pasta_igual.is_dir());
    for p in &parecidos {
        assert!(p.exists(), "{}", p.display());
    }
    // O estado de verdade fica e o daemon destrava com ele.
    assert!(ok(&unlock(&d, PASSWORD)));
}

/// Fase 54l (achado da 54c), ataque: o mesmo uid troca uma versão do backup por um FIFO
/// sem parar, entre o `readdir` da lista (que só aceita arquivo comum) e o `open` de
/// cada versão (lista do restaurar, `nucleo::backups`). Antes, o `open` sem
/// `O_NONBLOCK` caía no FIFO e ficava preso com o estado do daemon na mão (nenhum pedido
/// era mais atendido). Agora cada lista volta logo, a versão some ou aparece sem
/// conferir enquanto é FIFO, e o daemon segue atendendo.
#[test]
fn f54l_ataque_fifo_trocado_no_lugar_da_versao_nao_prende_o_daemon() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    let dir = Dir::new("bk-fifo");
    let (d, _) = test_vault(&dir);
    let d = Arc::new(d);
    let todas = backups(&d);
    let total = todas.len();
    let v = todas.last().unwrap().clone();
    let p = pasta(&dir);
    let real = p.join("x-real");
    fs::hard_link(p.join(&v), &real).unwrap();
    let parar = Arc::new(AtomicBool::new(false));
    let trocador = {
        let (p, v, real, parar) = (p.clone(), v.clone(), real.clone(), parar.clone());
        std::thread::spawn(move || {
            let mut n = 0u64;
            while !parar.load(Ordering::Acquire) {
                let fifo = p.join(format!("x-fifo-{n}"));
                let st = std::process::Command::new("/usr/bin/mkfifo")
                    .arg("-m")
                    .arg("600")
                    .arg(&fifo)
                    .status()
                    .unwrap();
                assert!(st.success());
                fs::rename(&fifo, p.join(&v)).unwrap();
                std::thread::sleep(Duration::from_micros(500));
                let volta = p.join(format!("x-volta-{n}"));
                fs::hard_link(&real, &volta).unwrap();
                fs::rename(&volta, p.join(&v)).unwrap();
                std::thread::sleep(Duration::from_micros(500));
                n += 1;
            }
            // No fim, a versão real no lugar.
            n
        })
    };
    let fim = std::time::Instant::now() + Duration::from_secs(3);
    let mut listas = 0;
    while std::time::Instant::now() < fim {
        let (tx, rx) = std::sync::mpsc::channel();
        let d2 = d.clone();
        std::thread::spawn(move || {
            let _ = tx.send(versoes(&d2).len());
        });
        let n = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("a lista dos backups ficou presa num FIFO (o daemon parou)");
        assert!(n <= total, "{n} > {total}");
        listas += 1;
    }
    parar.store(true, Ordering::Release);
    let trocas = trocador.join().unwrap();
    assert!(
        trocas > 100 && listas > 100,
        "{trocas} trocas, {listas} listas"
    );
    // A versão real volta a conferir; o daemon segue destravado.
    let l = versoes(&d);
    assert!(l.iter().any(|x| x.nome == v && x.conferida), "{l:?}");
    assert!(!d.travado());
}
