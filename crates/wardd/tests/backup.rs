//! Backup local e restaurar no daemon (Fase 45; D17, D27, D39): cada gravação vai para
//! o backup (V7.1), restaurar só pelo app (V7.8), versão restaurada selada com geração
//! nova e a cópia à mão em retrocesso (V7.11), pasta trocada por link, PIN e senha da
//! versão, cofre trocado fora do Ward, estado que não grava (S7, S9) e permissões.
//!
//! Cofre de teste próprio, só com valores falsos (nunca o cofre real nem o de `.dev/`).

mod comum;

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use comum::*;
use ward_core::backup::Versao;
use ward_core::entrada::Segredo;
use ward_core::protocolo::{ErroResposta, MSG_RETROCESSO, Papel, Pedido, Resposta};
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
    match pedir(d, Papel::App, "backups", "{}") {
        Resposta::Backups(l) => l,
        outra => panic!("{outra:?}"),
    }
}

fn restaurar(d: &Daemon, versao: &str, senha: &str, pin: &str) -> Resposta {
    pedir(
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
    match pedir(d, Papel::App, "lista", "{}") {
        Resposta::Lista(l) => l.into_iter().map(|i| i.nome).collect(),
        outra => panic!("{outra:?}"),
    }
}

fn tempo_trava(d: &Daemon, minutos: u32) {
    let r = pedir(
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
    let dir = Dir::novo("bk-cada");
    let c = dir.caminhos();
    let (d, _) = cofre_de_teste(&dir);
    // criar_cofre + 5 entradas + 2 mudanças de nível.
    let lista = backups(&d);
    assert_eq!(lista.len(), 8, "{lista:?}");
    assert_eq!(modo(&pasta(&dir)), 0o700);
    for n in &lista {
        assert!(Versao::de_nome(n).is_some(), "{n}");
        let p = pasta(&dir).join(n);
        assert_eq!(modo(&p), 0o600, "{n}");
        let b = fs::read(&p).unwrap();
        for s in SEGREDOS.iter().chain([SENHA, PIN].iter()) {
            assert!(!contem(&b, s.as_bytes()), "segredo em claro em {n}");
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
        .map(|n| geracao_no_disco(&pasta(&dir).join(n), SENHA))
        .collect();
    g.reverse();
    assert_eq!(g, (1..=8).collect::<Vec<_>>());
    // A resposta ao app tem só nomes.
    let bytes = codificado(Papel::App, &Resposta::Backups(lista.clone()));
    assert!(!contem(&bytes, dir.0.to_string_lossy().as_bytes()));
}

/// V7.8 (D17): o agente e a extensão pedindo restaurar ou listar backups recebem "não
/// autorizado" no protocolo e no daemon (segunda camada), com o cofre destravado e o
/// PIN certo; nada muda no disco.
#[test]
fn v7_8_agente_e_extensao_nao_restauram() {
    let dir = Dir::novo("bk-papel");
    let c = dir.caminhos();
    let (d, _) = cofre_de_teste(&dir);
    let alvo = backups(&d).pop().unwrap();
    let antes = fs::read(&c.cofre).unwrap();
    let estado_antes = fs::read(&c.estado).unwrap();
    for papel in [Papel::Agente, Papel::Extensao] {
        let r = pedir(
            &d,
            papel,
            "restaurar_backup",
            &format!(
                r#"{{"versao":"{alvo}","senha":"{}","pin":"{}"}}"#,
                b64(SENHA.as_bytes()),
                b64(PIN.as_bytes())
            ),
        );
        assert_eq!(
            erro(&r),
            Some(ErroResposta::NaoAutorizado),
            "{}",
            papel.nome()
        );
        let r = pedir(&d, papel, "backups", "{}");
        assert_eq!(
            erro(&r),
            Some(ErroResposta::NaoAutorizado),
            "{}",
            papel.nome()
        );
        // Segunda camada: o pedido montado em memória, sem passar pelo protocolo.
        let r = d.atender(
            papel,
            Pedido::RestaurarBackup {
                versao: alvo.clone(),
                senha: Segredo::de_texto(SENHA.to_owned()),
                pin: Segredo::de_texto(PIN.to_owned()),
            },
        );
        assert_eq!(
            erro(&r),
            Some(ErroResposta::NaoAutorizado),
            "{}",
            papel.nome()
        );
        assert_eq!(
            erro(&d.atender(papel, Pedido::Backups)),
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
    let dir = Dir::novo("bk-v7-11");
    let c = dir.caminhos();
    let (d, _) = cofre_de_teste(&dir);
    let n = geracao_no_disco(&c.cofre, SENHA);
    let versao_n = backups(&d)[0].clone();
    assert_eq!(geracao_no_disco(&pasta(&dir).join(&versao_n), SENHA), n);
    let copia_n = fs::read(pasta(&dir).join(&versao_n)).unwrap();
    let entradas_n = nomes_da_lista(&d);

    // Duas gravações: uma entrada nova e o tempo de trava.
    let r = pedir(
        &d,
        Papel::App,
        "salvar_entrada",
        &login("t-nova", SEGREDOS[0]),
    );
    assert!(ok(&r), "{r:?}");
    tempo_trava(&d, 30);
    assert_eq!(geracao_no_disco(&c.cofre, SENHA), n + 2);
    let atual_n2 = fs::read(&c.cofre).unwrap();
    assert_ne!(nomes_da_lista(&d), entradas_n);

    let r = restaurar(&d, &versao_n, SENHA, PIN);
    assert!(ok(&r), "{r:?}");
    assert_eq!(geracao_no_disco(&c.cofre, SENHA), n + 3);
    assert_eq!(
        geracoes(&dir),
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
    let r = destravar(&d, SENHA);
    assert!(ok(&r), "{r:?}");
    assert_eq!(nomes_da_lista(&d), entradas_n);
    d.travar();

    // Cópia à mão do mesmo backup: a geração n é menor que a vista (n+3).
    fs::write(&c.cofre, &copia_n).unwrap();
    let r = destravar(&d, SENHA);
    assert_eq!(erro(&r), Some(ErroResposta::Retrocesso));
    assert!(contem(
        &codificado(Papel::Agente, &r),
        MSG_RETROCESSO.as_bytes()
    ));
    assert!(d.travado());
    // Num daemon novo, direto o retrocesso; o arquivo copiado não é regravado.
    drop(d);
    let (d2, _) = daemon_em(dir.caminhos());
    assert_eq!(erro(&destravar(&d2, SENHA)), Some(ErroResposta::Retrocesso));
    assert_eq!(fs::read(&c.cofre).unwrap(), copia_n);
}

/// S9: restaurar exige o cofre destravado e o PIN atual (o erro conta para os 5 que
/// travam) e a senha da versão; nome que não existe, senha errada ou PIN errado não
/// mexem em nada.
#[test]
fn ataque_restaurar_sem_pin_senha_errada_e_travado() {
    let dir = Dir::novo("bk-pin");
    let c = dir.caminhos();
    let (d, _) = cofre_de_teste(&dir);
    let alvo = backups(&d).pop().unwrap();
    let antes = fs::read(&c.cofre).unwrap();
    let estado = fs::read(&c.estado).unwrap();
    let n_backups = backups(&d).len();

    let r = restaurar(&d, &alvo, SENHA, PIN_ERRADO);
    assert_eq!(erro(&r), Some(ErroResposta::PinIncorreto));
    let r = restaurar(&d, &alvo, "falso-mestra-ward-02", PIN);
    assert_eq!(erro(&r), Some(ErroResposta::SenhaIncorreta));
    let r = restaurar(&d, "ward-20000101-000000.ward", SENHA, PIN);
    assert_eq!(erro(&r), Some(ErroResposta::BackupInexistente));
    let r = restaurar(&d, "../cofre-teste.ward", SENHA, PIN);
    assert_eq!(erro(&r), Some(ErroResposta::PedidoInvalido));
    assert_eq!(fs::read(&c.cofre).unwrap(), antes);
    assert_eq!(fs::read(&c.estado).unwrap(), estado);
    assert_eq!(backups(&d).len(), n_backups);

    // 5 PINs errados seguidos pelo restaurar travam o cofre.
    for i in 0..5 {
        let r = restaurar(&d, &alvo, SENHA, PIN_ERRADO);
        assert_eq!(erro(&r), Some(ErroResposta::PinIncorreto), "tentativa {i}");
    }
    assert!(d.travado());
    // Travado: nem lista nem restaura, com o PIN certo.
    assert_eq!(
        erro(&pedir(&d, Papel::App, "backups", "{}")),
        Some(ErroResposta::Travado)
    );
    assert_eq!(
        erro(&restaurar(&d, &alvo, SENHA, PIN)),
        Some(ErroResposta::Travado)
    );
    assert_eq!(fs::read(&c.cofre).unwrap(), antes);
}

/// A pasta de backups trocada por um link simbólico para outra pasta (com uma cópia
/// autêntica de uma versão antiga): listar e restaurar respondem "indisponível", e a
/// gravação do cofre segue sem escrever nada no alvo do link. O mesmo com a pasta 0755.
#[test]
fn ataque_pasta_de_backups_trocada_por_link() {
    let dir = Dir::novo("bk-link");
    let c = dir.caminhos();
    let (d, _) = cofre_de_teste(&dir);
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
        erro(&pedir(&d, Papel::App, "backups", "{}")),
        Some(ErroResposta::BackupIndisponivel)
    );
    let antes = fs::read(&c.cofre).unwrap();
    assert_eq!(
        erro(&restaurar(&d, &alvo_versao, SENHA, PIN)),
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
        erro(&pedir(&d, Papel::App, "backups", "{}")),
        Some(ErroResposta::BackupIndisponivel)
    );
    fs::set_permissions(pasta(&dir), fs::Permissions::from_mode(0o700)).unwrap();
    assert!(!backups(&d).is_empty());
}

/// Uma versão na pasta trocada por um link simbólico para uma cópia autêntica fora da
/// pasta: o daemon não segue (`O_NOFOLLOW`) e não restaura; a lista a ignora.
#[test]
fn ataque_versao_trocada_por_link() {
    let dir = Dir::novo("bk-versao-link");
    let c = dir.caminhos();
    let (d, _) = cofre_de_teste(&dir);
    let v = backups(&d).pop().unwrap();
    let fora = dir.0.join("copia-fora.ward");
    fs::rename(pasta(&dir).join(&v), &fora).unwrap();
    symlink(&fora, pasta(&dir).join(&v)).unwrap();
    assert!(!backups(&d).contains(&v));
    let antes = fs::read(&c.cofre).unwrap();
    assert_eq!(
        erro(&restaurar(&d, &v, SENHA, PIN)),
        Some(ErroResposta::BackupIndisponivel)
    );
    assert_eq!(fs::read(&c.cofre).unwrap(), antes);
    assert!(!d.travado());
}

/// D39: o arquivo do cofre trocado fora do Ward antes do restaurar (outro inode, mesmos
/// bytes): nada é restaurado, o cofre trava e o arquivo trocado fica como está.
#[test]
fn ataque_restaurar_com_cofre_trocado_fora_do_ward() {
    let dir = Dir::novo("bk-mudou");
    let c = dir.caminhos();
    let (d, _) = cofre_de_teste(&dir);
    let v = backups(&d).pop().unwrap();
    let bytes = fs::read(&c.cofre).unwrap();
    let tmp = dir.0.join("troca");
    fs::write(&tmp, &bytes).unwrap();
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600)).unwrap();
    fs::rename(&tmp, &c.cofre).unwrap();
    let n_backups = backups(&d).len();
    assert_eq!(
        erro(&restaurar(&d, &v, SENHA, PIN)),
        Some(ErroResposta::CofreMudou)
    );
    assert!(d.travado());
    assert_eq!(fs::read(&c.cofre).unwrap(), bytes);
    let (d2, _) = daemon_em(dir.caminhos());
    assert!(ok(&destravar(&d2, SENHA)));
    assert_eq!(backups(&d2).len(), n_backups, "nada novo na pasta");
}

/// S7/D53: com o arquivo de estado sem como gravar, o restaurar não troca o cofre e o
/// daemon segue com as entradas de antes; nada da versão vai para o disco do cofre.
#[test]
fn ataque_restaurar_sem_gravar_o_estado_nao_troca_o_cofre() {
    let dir = Dir::novo("bk-estado");
    let c = dir.caminhos();
    let (d, _) = cofre_de_teste(&dir);
    let v = backups(&d).pop().unwrap();
    let antes = fs::read(&c.cofre).unwrap();
    let lista_antes = nomes_da_lista(&d);
    // O diretório do cofre sem escrita: o temporário do estado não nasce.
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o500)).unwrap();
    let r = restaurar(&d, &v, SENHA, PIN);
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(erro(&r), Some(ErroResposta::FalhaAoGravar));
    assert_eq!(fs::read(&c.cofre).unwrap(), antes);
    assert!(!d.travado());
    assert_eq!(nomes_da_lista(&d), lista_antes);
    // A próxima gravação comum funciona e a geração não repete.
    let g = geracao_no_disco(&c.cofre, SENHA);
    tempo_trava(&d, 50);
    assert!(geracao_no_disco(&c.cofre, SENHA) > g);
}

/// A troca de senha vai para o backup com a senha nova, e restaurar a versão anterior
/// pede a senha antiga (D17: a senha daquela versão).
#[test]
fn restaurar_pede_a_senha_da_versao() {
    let dir = Dir::novo("bk-senha");
    let c = dir.caminhos();
    let (d, _) = cofre_de_teste(&dir);
    let antiga = backups(&d)[0].clone();
    let nova = "falso-mestra-ward-05";
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
    assert!(ok(&r), "{r:?}");
    let depois = backups(&d)[0].clone();
    assert_ne!(depois, antiga);
    assert_eq!(
        fs::read(pasta(&dir).join(&depois)).unwrap(),
        fs::read(&c.cofre).unwrap()
    );
    // A versão antiga só abre com a senha antiga.
    assert_eq!(
        erro(&restaurar(&d, &antiga, nova, PIN)),
        Some(ErroResposta::SenhaIncorreta)
    );
    assert!(ok(&restaurar(&d, &antiga, SENHA, PIN)));
    d.travar();
    assert_eq!(
        erro(&destravar(&d, nova)),
        Some(ErroResposta::SenhaIncorreta)
    );
    let (d2, _) = daemon_em(dir.caminhos());
    assert!(ok(&destravar(&d2, SENHA)));
}
