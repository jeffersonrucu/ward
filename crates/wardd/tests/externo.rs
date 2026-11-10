//! Backup externo em pendrive (Fase 46; V7.4, V7.5, V7.6, V7.8).
//!
//! Nenhum dispositivo é montado: o "pendrive" é um diretório em `target/` e a fonte dos
//! pendrives é uma lista (ou um udisks2 falso num `dbus-daemon` privado deste teste,
//! como o logind falso da Fase 24). O `st_dev` dado pela fonte é o do diretório, que é
//! o que o udisks2 daria para um sistema de arquivos montado ali.
//!
//! Ataques (todos falham do lado do atacante):
//! - UUID errado, pendrive B no lugar do A, dois com o mesmo UUID, dispositivo que não
//!   bate com a montagem e udisks2 que muda de resposta entre as consultas: nada é
//!   escrito;
//! - montagem que é link simbólico, `ward-backup` que é link ou arquivo: nada é escrito
//!   no alvo;
//! - versão que já está no pendrive (sem `link(2)`, como no vfat) nunca é sobrescrita;
//! - desligado por padrão, travado ou desligado depois: nada é consultado nem gravado;
//! - agente e extensão não leem, não ligam e não listam pendrives (V7.8).

mod comum;

use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use comum::*;
use ward_core::backup::VERSOES;
use ward_core::protocolo::{ErroResposta, Papel, Resposta};
use wardd::externo::{ErroExterno, FontePendrives, Visto};
use wardd::nucleo::Daemon;
use zbus::connection::Builder;
use zbus::{Connection, OwnedObjectPath, OwnedVariant, Variant};

const UUID_A: &str = "ABCD-1234";
const UUID_B: &str = "0F3C-9A51";

// ---------------------------------------------------------------- fonte falsa

/// Fonte de pendrives: a lista que o teste põe, contando as consultas.
#[derive(Clone, Default)]
struct Fonte {
    vistos: Arc<Mutex<Vec<Visto>>>,
    /// Respostas seguintes, uma por consulta (para mudar de ideia no meio).
    proximas: Arc<Mutex<Vec<Vec<Visto>>>>,
    consultas: Arc<AtomicUsize>,
}

impl Fonte {
    fn com(vistos: Vec<Visto>) -> Fonte {
        let f = Fonte::default();
        *f.vistos.lock().unwrap() = vistos;
        f
    }
    fn por(&self, vistos: Vec<Visto>) {
        *self.vistos.lock().unwrap() = vistos;
    }
    fn consultas(&self) -> usize {
        self.consultas.load(Ordering::SeqCst)
    }
}

impl FontePendrives for Fonte {
    fn pendrives(&self) -> Option<Vec<Visto>> {
        self.consultas.fetch_add(1, Ordering::SeqCst);
        let mut p = self.proximas.lock().unwrap();
        if !p.is_empty() {
            return Some(p.remove(0));
        }
        Some(self.vistos.lock().unwrap().clone())
    }
}

/// Um "pendrive" montado em `dir` (o dispositivo é o `st_dev` do diretório).
fn pendrive(uuid: &str, dir: &Path) -> Visto {
    Visto {
        uuid: uuid.into(),
        rotulo: "PENDRIVE".into(),
        montagem: Some(dir.to_path_buf()),
        dispositivo: fs::metadata(dir).unwrap().dev(),
    }
}

fn montagem(dir: &Dir, nome: &str) -> PathBuf {
    let p = dir.0.join(nome);
    fs::create_dir_all(&p).unwrap();
    p
}

fn ligar(d: &Daemon, papel: Papel, uuid: &str) -> Resposta {
    pedir(
        d,
        papel,
        "definir_backup_externo",
        &format!(r#"{{"ligado":true,"uuid":"{uuid}"}}"#),
    )
}

/// Versões no backup local, da mais nova para a mais antiga.
fn locais(dir: &Dir) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir.0.join("backups"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| ward_core::backup::nome_valido(n))
        .collect();
    v.sort_by_key(|n| std::cmp::Reverse(ward_core::backup::Versao::de_nome(n).unwrap()));
    v
}

/// Tudo que está no diretório, recursivo, com o conteúdo (links pelo alvo, sem seguir).
fn arvore(p: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut v = Vec::new();
    let Ok(itens) = fs::read_dir(p) else {
        return v;
    };
    for e in itens {
        let e = e.unwrap();
        let t = e.file_type().unwrap();
        if t.is_dir() {
            v.extend(arvore(&e.path()));
        } else if t.is_symlink() {
            v.push((
                e.path(),
                fs::read_link(e.path()).unwrap().into_os_string().into_vec(),
            ));
        } else {
            v.push((e.path(), fs::read(e.path()).unwrap()));
        }
    }
    v.sort();
    v
}

use std::os::unix::ffi::OsStringExt;

fn pasta_externa(m: &Path) -> PathBuf {
    m.join("ward-backup")
}

/// Nomes de versão no pendrive.
fn no_pendrive(m: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(pasta_externa(m))
        .map(|l| {
            l.map(|e| e.unwrap().file_name().into_string().unwrap())
                .filter(|n| ward_core::backup::nome_valido(n))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

// ---------------------------------------------------------------- V7.4

/// V7.4: cofre novo, pendrive conectado e montado: nada é copiado nem consultado
/// (desligado por padrão), e a configuração diz desligado, sem UUID.
#[test]
fn v7_4_desligado_por_padrao_nao_grava_nada() {
    let dir = Dir::novo("ext-padrao");
    let (d, _) = cofre_de_teste(&dir);
    let a = montagem(&dir, "pendrive-a");
    let fonte = Fonte::com(vec![pendrive(UUID_A, &a)]);
    let d = d.com_pendrives(Arc::new(fonte.clone()));
    match pedir(&d, Papel::App, "backup_externo", "{}") {
        Resposta::BackupExterno { ligado, uuid } => {
            assert!(!ligado);
            assert_eq!(uuid, None);
        }
        r => panic!("{r:?}"),
    }
    assert_eq!(d.sincronizar_externo(&fonte), Ok(None));
    assert_eq!(fonte.consultas(), 0, "desligado nem consulta o udisks2");
    assert!(arvore(&a).is_empty(), "nada no pendrive");
    // A lista de pendrives (para o app escolher) funciona e não grava nada.
    match pedir(&d, Papel::App, "pendrives", "{}") {
        Resposta::Pendrives(l) => {
            assert_eq!(l.len(), 1);
            assert_eq!(l[0].uuid, UUID_A);
            assert!(l[0].montado);
        }
        r => panic!("{r:?}"),
    }
    assert!(arvore(&a).is_empty());
}

// ---------------------------------------------------------------- V7.5, V7.6

/// V7.5: ligado com o UUID do A, as versões vão para `<montagem A>/ward-backup/`, iguais
/// às do backup local; o B conectado (sozinho ou junto) não recebe nada. V7.6: nenhum
/// segredo, senha ou PIN em claro ou em base64 no pendrive, e cada cópia abre com a
/// senha de teste.
#[test]
fn v7_5_v7_6_copia_so_para_o_pendrive_escolhido_e_sem_segredo() {
    let dir = Dir::novo("ext-a-b");
    let (d, _) = cofre_de_teste(&dir);
    let a = montagem(&dir, "pendrive-a");
    let b = montagem(&dir, "pendrive-b");
    let fonte = Fonte::com(vec![pendrive(UUID_B, &b)]);
    assert!(ok(&ligar(&d, Papel::App, UUID_A)));
    match pedir(&d, Papel::App, "backup_externo", "{}") {
        Resposta::BackupExterno { ligado, uuid } => {
            assert!(ligado);
            assert_eq!(uuid.as_deref(), Some(UUID_A));
        }
        r => panic!("{r:?}"),
    }
    // Só o B conectado: nada.
    assert_eq!(
        d.sincronizar_externo(&fonte),
        Err(ErroExterno::NaoConectado)
    );
    assert!(arvore(&b).is_empty());
    // A e B conectados: só o A recebe.
    fonte.por(vec![pendrive(UUID_A, &a), pendrive(UUID_B, &b)]);
    let c = d.sincronizar_externo(&fonte).unwrap().unwrap();
    let locais = locais(&dir);
    assert_eq!(c.copiadas.len(), locais.len());
    assert!(arvore(&b).is_empty(), "o pendrive B não recebe nada");
    let mut esperado = locais.clone();
    esperado.sort();
    assert_eq!(no_pendrive(&a), esperado);
    for n in &locais {
        assert_eq!(
            fs::read(pasta_externa(&a).join(n)).unwrap(),
            fs::read(dir.0.join("backups").join(n)).unwrap(),
            "{n}"
        );
    }
    // Nada além das versões (nem temporário).
    assert_eq!(arvore(&a).len(), locais.len());
    // De novo: nada a copiar, nada muda.
    let antes = arvore(&a);
    assert!(
        d.sincronizar_externo(&fonte)
            .unwrap()
            .unwrap()
            .copiadas
            .is_empty()
    );
    assert_eq!(arvore(&a), antes);
    // Uma gravação nova: só ela vai.
    let r = pedir(
        &d,
        Papel::App,
        "salvar_entrada",
        &login("t-nova", SEGREDOS[3]),
    );
    assert!(ok(&r), "{r:?}");
    let c = d.sincronizar_externo(&fonte).unwrap().unwrap();
    assert_eq!(c.copiadas.len(), 1);
    assert_eq!(no_pendrive(&a).len(), locais.len() + 1);
    // UUID em minúsculas no udisks2 (o vfat vem em maiúsculas; tanto faz).
    fonte.por(vec![pendrive(&UUID_A.to_lowercase(), &a)]);
    assert!(d.sincronizar_externo(&fonte).unwrap().is_some());

    // V7.6: o auditor procura segredos nos arquivos do pendrive.
    let mut agulhas: Vec<Vec<u8>> = Vec::new();
    for s in SEGREDOS.iter().chain([&SENHA, &PIN]) {
        agulhas.push(s.as_bytes().to_vec());
        agulhas.push(b64(s.as_bytes()).trim_end_matches('=').as_bytes().to_vec());
    }
    for (caminho, bytes) in arvore(&a) {
        for ag in &agulhas {
            assert!(
                !contem(&bytes, ag),
                "{} tem um valor de teste em claro",
                caminho.display()
            );
        }
        assert!(geracao_no_disco(&caminho, SENHA) >= 1);
    }
}

// ---------------------------------------------------------------- ataques

/// Pendrive trocado: a fonte diz que o A está montado num diretório de outro
/// dispositivo (o B no lugar, ou outra coisa montada no caminho), ou muda a resposta
/// entre a primeira consulta e a de depois de abrir. Dois sistemas de arquivos com o
/// mesmo UUID também não recebem nada.
#[test]
fn ataque_pendrive_trocado_ou_uuid_repetido_nao_recebe_nada() {
    let dir = Dir::novo("ext-trocado");
    let (d, _) = cofre_de_teste(&dir);
    let a = montagem(&dir, "pendrive-a");
    let b = montagem(&dir, "pendrive-b");
    assert!(ok(&ligar(&d, Papel::App, UUID_A)));
    // Dispositivo que não é o do diretório montado.
    let mut falso = pendrive(UUID_A, &a);
    falso.dispositivo ^= 0x55;
    let fonte = Fonte::com(vec![falso]);
    assert_eq!(d.sincronizar_externo(&fonte), Err(ErroExterno::Trocado));
    // A resposta muda depois de abrir: outro dispositivo, outra montagem.
    for depois in [
        {
            let mut v = pendrive(UUID_A, &a);
            v.dispositivo ^= 1;
            v
        },
        pendrive(UUID_A, &b),
    ] {
        let fonte = Fonte::com(vec![pendrive(UUID_A, &a)]);
        fonte
            .proximas
            .lock()
            .unwrap()
            .extend([vec![pendrive(UUID_A, &a)], vec![depois]]);
        assert_eq!(d.sincronizar_externo(&fonte), Err(ErroExterno::Trocado));
    }
    // Pendrive desconectado entre as consultas.
    let fonte = Fonte::com(vec![pendrive(UUID_A, &a)]);
    fonte
        .proximas
        .lock()
        .unwrap()
        .extend([vec![pendrive(UUID_A, &a)], vec![]]);
    assert_eq!(
        d.sincronizar_externo(&fonte),
        Err(ErroExterno::NaoConectado)
    );
    // Dois com o mesmo UUID (pendrive clonado).
    let fonte = Fonte::com(vec![pendrive(UUID_A, &a), pendrive(UUID_A, &b)]);
    assert_eq!(d.sincronizar_externo(&fonte), Err(ErroExterno::Ambiguo));
    // Conectado sem montar.
    let mut sem = pendrive(UUID_A, &a);
    sem.montagem = None;
    assert_eq!(
        d.sincronizar_externo(&Fonte::com(vec![sem])),
        Err(ErroExterno::NaoMontado)
    );
    assert!(arvore(&a).is_empty() && arvore(&b).is_empty());
}

/// A montagem que é um link simbólico (para um diretório do mesmo dispositivo) e a
/// `ward-backup` que é link ou arquivo: nada é escrito no alvo.
#[test]
fn ataque_montagem_ou_pasta_que_e_link() {
    let dir = Dir::novo("ext-link");
    let (d, _) = cofre_de_teste(&dir);
    assert!(ok(&ligar(&d, Papel::App, UUID_A)));
    let alvo = montagem(&dir, "alvo");
    let link = dir.0.join("montagem-link");
    std::os::unix::fs::symlink(&alvo, &link).unwrap();
    let mut v = pendrive(UUID_A, &alvo);
    v.montagem = Some(link.clone());
    assert_eq!(
        d.sincronizar_externo(&Fonte::com(vec![v])),
        Err(ErroExterno::Trocado)
    );
    assert!(arvore(&alvo).is_empty());

    // `ward-backup` é um link para outra pasta.
    let a = montagem(&dir, "pendrive-a");
    let outra = montagem(&dir, "outra");
    std::os::unix::fs::symlink(&outra, pasta_externa(&a)).unwrap();
    let fonte = Fonte::com(vec![pendrive(UUID_A, &a)]);
    assert_eq!(
        d.sincronizar_externo(&fonte),
        Err(ErroExterno::PastaInsegura)
    );
    assert!(arvore(&outra).is_empty());
    assert!(
        fs::symlink_metadata(pasta_externa(&a))
            .unwrap()
            .is_symlink()
    );
    // `ward-backup` é um arquivo.
    fs::remove_file(pasta_externa(&a)).unwrap();
    fs::write(pasta_externa(&a), b"nao e pasta").unwrap();
    assert_eq!(
        d.sincronizar_externo(&fonte),
        Err(ErroExterno::PastaInsegura)
    );
    assert_eq!(fs::read(pasta_externa(&a)).unwrap(), b"nao e pasta");
}

/// Sem `link(2)` (vfat): uma versão que já está no pendrive nunca é sobrescrita, um
/// link com nome de versão não é seguido, o temporário de uma cópia interrompida some
/// e o que não tem nome de versão fica.
#[test]
fn ataque_versao_existente_no_pendrive_nao_e_sobrescrita() {
    let dir = Dir::novo("ext-existe");
    let (d, _) = cofre_de_teste(&dir);
    assert!(ok(&ligar(&d, Papel::App, UUID_A)));
    let a = montagem(&dir, "pendrive-a");
    let destino = pasta_externa(&a);
    fs::create_dir(&destino).unwrap();
    let locais = locais(&dir);
    let mais_nova = &locais[0];
    let segunda = &locais[1];
    fs::write(destino.join(mais_nova), b"versao que ja estava").unwrap();
    let fora = montagem(&dir, "fora");
    std::os::unix::fs::symlink(fora.join("x"), destino.join(segunda)).unwrap();
    fs::write(destino.join(".ward-tmp-1-2-00000000"), b"pedaco").unwrap();
    fs::write(destino.join("LEIA-ME.txt"), b"do usuario").unwrap();
    let c = d
        .sincronizar_externo(&Fonte::com(vec![pendrive(UUID_A, &a)]))
        .unwrap()
        .unwrap();
    assert_eq!(
        fs::read(destino.join(mais_nova)).unwrap(),
        b"versao que ja estava"
    );
    assert!(
        fs::symlink_metadata(destino.join(segunda))
            .unwrap()
            .is_symlink()
    );
    assert!(!fora.join("x").exists(), "o link não foi seguido");
    assert!(!destino.join(".ward-tmp-1-2-00000000").exists());
    assert_eq!(
        fs::read(destino.join("LEIA-ME.txt")).unwrap(),
        b"do usuario"
    );
    // O link com nome de versão não conta como cópia: ele fica, e a versão não vai
    // para um nome que existe.
    assert_eq!(c.copiadas.len(), locais.len() - 2);
}

/// Rotação no pendrive: com mais de 30 gerações reais, ficam as 30 mais novas (as
/// mesmas do backup local), sem apagar o que acabou de copiar nem o que não é versão.
/// Desde a Fase 54c, arquivos com nome de versão que não conferem pela chave do cofre
/// (aqui, `antiga`) não contam e não saem.
#[test]
fn pendrive_fica_com_30_versoes() {
    let dir = Dir::novo("ext-rodar");
    let (d, _) = cofre_de_teste(&dir);
    assert!(ok(&ligar(&d, Papel::App, UUID_A)));
    let a = montagem(&dir, "pendrive-a");
    let destino = pasta_externa(&a);
    fs::create_dir(&destino).unwrap();
    let mut lixo = Vec::new();
    for i in 0..5u32 {
        let n = format!("ward-20200101-{:06}.ward", i * 100);
        fs::write(destino.join(&n), b"antiga").unwrap();
        lixo.push(n);
    }
    fs::write(destino.join("notas.txt"), b"x").unwrap();
    let fonte = Fonte::com(vec![pendrive(UUID_A, &a)]);
    let mut apagadas = 0;
    for i in 0..VERSOES + 5 {
        let r = pedir(
            &d,
            Papel::App,
            "salvar_entrada",
            &login(&format!("t-rodar-{i}"), SEGREDOS[3]),
        );
        assert!(ok(&r), "{r:?}");
        let c = d.sincronizar_externo(&fonte).unwrap().unwrap();
        let todas = no_pendrive(&a);
        for v in &c.copiadas {
            assert!(todas.contains(&v.nome()), "{v}");
        }
        apagadas += c.apagadas;
    }
    let mut reais: Vec<String> = no_pendrive(&a)
        .into_iter()
        .filter(|n| !lixo.contains(n))
        .collect();
    reais.sort();
    let mut esperadas = locais(&dir);
    esperadas.sort();
    assert_eq!(esperadas.len(), VERSOES);
    assert_eq!(reais, esperadas, "o pendrive não tem as 30 mais novas");
    assert!(apagadas > 0);
    for n in &lixo {
        assert!(
            destino.join(n).exists(),
            "{n}: a rotação apagou o que não confere"
        );
    }
    assert!(destino.join("notas.txt").exists());
}

/// Fase 54c (achado da Fase 54), ataque: quem escreve no pendrive (vfat 0777, outro
/// usuário da máquina) planta nomes de versão com data passada recente (depois das
/// cópias reais, antes de agora): lixo, versão de outro cofre e cópias de uma versão
/// real com nome novo. Antes da correção, a cópia seguinte girava o pendrive pela data
/// e apagava as cópias reais. Agora nada real sai, e o que não confere fica.
#[test]
fn f54c_ataque_nomes_plantados_no_pendrive_nao_apagam_copias_reais() {
    let dir = Dir::novo("ext-plantados");
    let (d, _) = cofre_de_teste(&dir);
    assert!(ok(&ligar(&d, Papel::App, UUID_A)));
    let a = montagem(&dir, "pendrive-a");
    let fonte = Fonte::com(vec![pendrive(UUID_A, &a)]);
    let c = d.sincronizar_externo(&fonte).unwrap().unwrap();
    let reais = no_pendrive(&a);
    assert_eq!(reais.len(), c.copiadas.len());
    assert!(!reais.is_empty());
    let destino = pasta_externa(&a);
    let real = fs::read(destino.join(&reais[0])).unwrap();
    let outro = Dir::novo("ext-plantados-outro");
    let _ = cofre_de_teste(&outro);
    let de_outro = fs::read(outro.0.join("backups").join(&locais(&outro)[0])).unwrap();
    std::thread::sleep(Duration::from_millis(1100));
    let base = ward_core::backup::Versao::do_instante(std::time::SystemTime::now())
        .unwrap()
        .nome();
    let mut plantados = Vec::new();
    for i in 1..=VERSOES + 5 {
        let n = base.replace(".ward", &format!("-{i}.ward"));
        let conteudo = match i % 3 {
            0 => b"lixo com nome de versao".to_vec(),
            1 => de_outro.clone(),
            _ => real.clone(),
        };
        fs::write(destino.join(&n), &conteudo).unwrap();
        plantados.push(n);
    }
    // Uma gravação real e a cópia: só ela vai; nada sai.
    std::thread::sleep(Duration::from_millis(1100));
    let r = pedir(
        &d,
        Papel::App,
        "salvar_entrada",
        &login("t-plantados", SEGREDOS[3]),
    );
    assert!(ok(&r), "{r:?}");
    let c = d.sincronizar_externo(&fonte).unwrap().unwrap();
    assert_eq!(c.copiadas.len(), 1, "{:?}", c.copiadas);
    assert_eq!(c.apagadas, 0, "os plantados giraram o pendrive");
    let agora = no_pendrive(&a);
    for n in reais.iter().chain(&plantados) {
        assert!(agora.contains(n), "{n} sumiu do pendrive");
    }
}

/// Fase 49a (achado médio do M7): no pendrive (vfat 0777: outro usuário da máquina pode
/// escrever) e no backup local, arquivos com nome de data futura não fazem a rotação
/// apagar as cópias reais; as futuras não contam nem são apagadas; do backup local, as
/// de data depois da margem não vão para o pendrive.
#[test]
fn ataque_versoes_com_data_futura_no_pendrive_nao_apagam_as_reais() {
    let dir = Dir::novo("ext-futuro");
    let (d, _) = cofre_de_teste(&dir);
    assert!(ok(&ligar(&d, Papel::App, UUID_A)));
    let a = montagem(&dir, "pendrive-a");
    let fonte = Fonte::com(vec![pendrive(UUID_A, &a)]);
    let c = d.sincronizar_externo(&fonte).unwrap().unwrap();
    let reais = no_pendrive(&a);
    assert_eq!(reais.len(), c.copiadas.len());
    let destino = pasta_externa(&a);
    // 30 de 2099 no pendrive e uma no backup local, 10 anos à frente.
    for i in 0..30u32 {
        fs::write(
            destino.join(format!("ward-20990101-{:06}.ward", i * 100)),
            b"plantada",
        )
        .unwrap();
    }
    let longe = ward_core::backup::Versao::do_instante(
        std::time::SystemTime::now() + Duration::from_secs(10 * 365 * 86_400),
    )
    .unwrap()
    .nome();
    let local = dir.0.join("backups").join(&longe);
    fs::copy(dir.0.join("backups").join(&locais(&dir)[0]), &local).unwrap();
    fs::set_permissions(&local, fs::Permissions::from_mode(0o600)).unwrap();
    // Uma gravação nova e a cópia: nenhuma real sai, a plantada local não vai.
    let r = pedir(
        &d,
        Papel::App,
        "salvar_entrada",
        &login("t-futuro", SEGREDOS[3]),
    );
    assert!(ok(&r), "{r:?}");
    let c = d.sincronizar_externo(&fonte).unwrap().unwrap();
    assert_eq!(c.apagadas, 0);
    assert!(c.copiadas.iter().all(|v| v.nome() != longe));
    let agora = no_pendrive(&a);
    for n in &reais {
        assert!(agora.contains(n), "{n} sumiu do pendrive");
    }
    assert!(!agora.contains(&longe));
    assert_eq!(
        agora.iter().filter(|n| n.starts_with("ward-2099")).count(),
        30
    );
}

/// Fase 54 (baixo da auditoria final): lixo plantado no backup local (cabeçalho válido
/// com o corpo trocado, versão de outro cofre e a cópia de uma versão antiga real com
/// nome novo), com datas mais novas que as reais, não vai para o pendrive e não faz a
/// rotação de lá apagar nada. Uma gravação real depois vai, sozinha.
#[test]
fn ataque_lixo_no_backup_local_nao_gira_o_pendrive() {
    let dir = Dir::novo("ext-lixo");
    let (d, _) = cofre_de_teste(&dir);
    assert!(ok(&ligar(&d, Papel::App, UUID_A)));
    let a = montagem(&dir, "pendrive-a");
    let destino = pasta_externa(&a);
    fs::create_dir(&destino).unwrap();
    // O pendrive já cheio: as reais de agora mais antigas de 2020 até `VERSOES`.
    let fonte = Fonte::com(vec![pendrive(UUID_A, &a)]);
    let reais = locais(&dir);
    for i in 0..(VERSOES - reais.len()) as u32 {
        fs::write(
            destino.join(format!("ward-20200101-{:06}.ward", i * 100)),
            b"antiga",
        )
        .unwrap();
    }
    let c = d.sincronizar_externo(&fonte).unwrap().unwrap();
    assert_eq!(c.copiadas.len(), reais.len());
    assert_eq!(c.apagadas, 0);
    let antes = arvore(&a);
    assert_eq!(no_pendrive(&a).len(), VERSOES);

    // O atacante (mesmo uid) planta na pasta local, com data mais nova que as reais.
    std::thread::sleep(Duration::from_millis(1100));
    let base = ward_core::backup::Versao::do_instante(std::time::SystemTime::now())
        .unwrap()
        .nome();
    let nome = |i: usize| base.replace(".ward", &format!("-{i}.ward"));
    let backups = dir.0.join("backups");
    let real = fs::read(backups.join(&reais[0])).unwrap();
    let outro = Dir::novo("ext-lixo-outro");
    let _ = cofre_de_teste(&outro);
    let de_outro = fs::read(outro.0.join("backups").join(&locais(&outro)[0])).unwrap();
    let antiga = fs::read(backups.join(reais.last().unwrap())).unwrap();
    let mut plantados = Vec::new();
    for i in 1..=VERSOES + 5 {
        let conteudo = match i % 3 {
            0 => {
                // Cabeçalho válido, corpo trocado.
                let mut b = real.clone();
                let n = b.len();
                b[n - 1 - i] ^= 0x5a;
                b
            }
            1 => de_outro.clone(),
            _ => antiga.clone(),
        };
        assert!(ward_core::formato::ler_cabecalho(&conteudo).is_ok());
        let p = backups.join(nome(i));
        fs::write(&p, &conteudo).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
        plantados.push(nome(i));
    }
    assert!(ward_core::backup::nome_valido(&plantados[0]));
    let c = d.sincronizar_externo(&fonte).unwrap().unwrap();
    assert!(c.copiadas.is_empty(), "{:?}", c.copiadas);
    assert_eq!(c.apagadas, 0, "o lixo girou o pendrive");
    assert_eq!(arvore(&a), antes, "o pendrive mudou");

    // Uma gravação real: só ela vai, mesmo com o lixo mais novo em volta.
    std::thread::sleep(Duration::from_millis(1100));
    let r = pedir(
        &d,
        Papel::App,
        "salvar_entrada",
        &login("t-lixo", SEGREDOS[3]),
    );
    assert!(ok(&r), "{r:?}");
    let c = d.sincronizar_externo(&fonte).unwrap().unwrap();
    assert_eq!(c.copiadas.len(), 1, "{:?}", c.copiadas);
    assert!(!plantados.contains(&c.copiadas[0].nome()));
    // Fase 54c: o enchimento `antiga` do pendrive não confere: não conta nem sai.
    assert_eq!(c.apagadas, 0);
    for n in no_pendrive(&a) {
        assert!(!plantados.contains(&n), "{n} plantado foi para o pendrive");
    }
    for n in &reais {
        assert!(no_pendrive(&a).contains(n), "{n} real sumiu do pendrive");
    }
}

/// Travado ou desligado depois de ligado: nada é consultado nem gravado. Desligar não
/// apaga o que já está no pendrive.
#[test]
fn travado_ou_desligado_nao_copia() {
    let dir = Dir::novo("ext-travado");
    let (d, _) = cofre_de_teste(&dir);
    let a = montagem(&dir, "pendrive-a");
    let fonte = Fonte::com(vec![pendrive(UUID_A, &a)]);
    assert!(ok(&ligar(&d, Papel::App, UUID_A)));
    d.sincronizar_externo(&fonte).unwrap().unwrap();
    let antes = arvore(&a);
    assert!(!antes.is_empty());
    let n = fonte.consultas();
    d.travar();
    assert_eq!(d.sincronizar_externo(&fonte), Ok(None));
    assert_eq!(fonte.consultas(), n);
    assert_eq!(
        erro(&pedir(&d, Papel::App, "backup_externo", "{}")),
        Some(ErroResposta::Travado)
    );
    assert_eq!(
        erro(&pedir(&d, Papel::App, "pendrives", "{}")),
        Some(ErroResposta::Travado)
    );
    assert!(ok(&destravar(&d, SENHA)));
    // A configuração ligada voltou com o cofre (está na parte cifrada).
    assert!(d.sincronizar_externo(&fonte).unwrap().is_some());
    let r = pedir(
        &d,
        Papel::App,
        "definir_backup_externo",
        &format!(r#"{{"ligado":false,"uuid":"{UUID_A}"}}"#),
    );
    assert!(ok(&r), "{r:?}");
    let r = pedir(
        &d,
        Papel::App,
        "salvar_entrada",
        &login("t-depois", SEGREDOS[3]),
    );
    assert!(ok(&r));
    let n = fonte.consultas();
    assert_eq!(d.sincronizar_externo(&fonte), Ok(None));
    assert_eq!(fonte.consultas(), n);
    assert_eq!(arvore(&a), antes, "nada novo, nada apagado");
    match pedir(&d, Papel::App, "backup_externo", "{}") {
        Resposta::BackupExterno { ligado, uuid } => {
            assert!(!ligado);
            assert_eq!(uuid.as_deref(), Some(UUID_A));
        }
        r => panic!("{r:?}"),
    }
}

/// V7.8: o agente e a extensão pedindo para ler, ligar ou listar pendrives recebem
/// "não autorizado", direto no daemon (a segunda camada) e pela leitura do protocolo;
/// o cofre fica igual byte a byte e o externo, desligado.
#[test]
fn v7_8_agente_e_extensao_nao_ligam_nem_leem() {
    let dir = Dir::novo("ext-papel");
    let (d, _) = cofre_de_teste(&dir);
    let a = montagem(&dir, "pendrive-a");
    let fonte = Fonte::com(vec![pendrive(UUID_A, &a)]);
    let d = d.com_pendrives(Arc::new(fonte.clone()));
    let cofre = fs::read(&dir.caminhos().cofre).unwrap();
    for papel in [Papel::Agente, Papel::Extensao] {
        for (op, dados) in [
            ("backup_externo", "{}".to_owned()),
            ("pendrives", "{}".to_owned()),
            (
                "definir_backup_externo",
                format!(r#"{{"ligado":true,"uuid":"{UUID_A}"}}"#),
            ),
        ] {
            // Pela leitura do protocolo.
            assert_eq!(
                erro(&pedir(&d, papel, op, &dados)),
                Some(ErroResposta::NaoAutorizado),
                "{op} pelo {}",
                papel.nome()
            );
            // Direto no daemon, com o pedido lido como app.
            let p = ped(Papel::App, op, &dados).unwrap();
            assert_eq!(
                erro(&d.atender(papel, p)),
                Some(ErroResposta::NaoAutorizado)
            );
        }
    }
    assert_eq!(fs::read(&dir.caminhos().cofre).unwrap(), cofre);
    assert_eq!(fonte.consultas(), 0);
    assert_eq!(d.sincronizar_externo(&fonte), Ok(None));
    assert!(arvore(&a).is_empty());
}

/// Sem udisks2 (daemon de teste sem barramento), `pendrives` responde
/// `pendrives_indisponiveis`; UUID fora do formato é recusado sem gravar.
#[test]
fn pendrives_sem_udisks_e_uuid_invalido() {
    let dir = Dir::novo("ext-sem");
    let (d, _) = cofre_de_teste(&dir);
    assert_eq!(
        erro(&pedir(&d, Papel::App, "pendrives", "{}")),
        Some(ErroResposta::PendrivesIndisponiveis)
    );
    let cofre = fs::read(&dir.caminhos().cofre).unwrap();
    for uuid in ["../x", "ABCD 1234", "-AB", ""] {
        assert_eq!(
            erro(&ligar(&d, Papel::App, uuid)),
            Some(ErroResposta::PedidoInvalido),
            "{uuid}"
        );
    }
    assert_eq!(
        erro(&pedir(
            &d,
            Papel::App,
            "definir_backup_externo",
            r#"{"ligado":true}"#
        )),
        Some(ErroResposta::PedidoInvalido)
    );
    assert_eq!(fs::read(&dir.caminhos().cofre).unwrap(), cofre);
}

// ---------------------------------------------------------------- udisks2 falso

static N: AtomicU32 = AtomicU32::new(0);

/// `dbus-daemon` privado, só deste teste (morto no `Drop`).
struct BarramentoFalso {
    dir: PathBuf,
    socket: PathBuf,
    filho: Option<Child>,
}

impl BarramentoFalso {
    fn subir() -> BarramentoFalso {
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| {
                PathBuf::from(format!("/run/user/{}", ward_platform::uid_efetivo()))
            });
        let dir = base.join(format!(
            "wardd-udisks-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        let socket = dir.join("bus");
        let config = dir.join("bus.conf");
        fs::write(
            &config,
            format!(
                r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <listen>unix:path={}</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#,
                socket.display()
            ),
        )
        .unwrap();
        let filho = Command::new("/usr/bin/dbus-daemon")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--nopidfile", "--nosyslog"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("dbus-daemon");
        let fim = Instant::now() + Duration::from_secs(10);
        while UnixStream::connect(&socket).is_err() {
            assert!(Instant::now() < fim, "o dbus-daemon falso não subiu");
            std::thread::sleep(Duration::from_millis(20));
        }
        BarramentoFalso {
            dir,
            socket,
            filho: Some(filho),
        }
    }

    fn endereco(&self) -> String {
        format!("unix:path={}", self.socket.display())
    }
}

impl Drop for BarramentoFalso {
    fn drop(&mut self) {
        if let Some(mut f) = self.filho.take() {
            let _ = f.kill();
            let _ = f.wait();
        }
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Um objeto do udisks2 falso.
#[derive(Clone)]
struct Objeto {
    caminho: &'static str,
    uuid: String,
    rotulo: String,
    dispositivo: u64,
    montagem: Option<Vec<u8>>,
    sistema: bool,
    de_loop: bool,
}

type Objetos = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedVariant>>>;

fn ov<'a>(v: impl Into<Variant<'a>>) -> OwnedVariant {
    OwnedVariant::try_from(v.into()).unwrap()
}

fn objetos(lista: &[Objeto]) -> Objetos {
    let mut m = Objetos::new();
    for o in lista {
        let mut bloco = HashMap::new();
        bloco.insert("IdUsage".into(), ov("filesystem"));
        bloco.insert("IdUUID".into(), ov(o.uuid.as_str()));
        bloco.insert("IdLabel".into(), ov(o.rotulo.as_str()));
        bloco.insert("DeviceNumber".into(), ov(o.dispositivo));
        bloco.insert("HintIgnore".into(), ov(false));
        bloco.insert("HintSystem".into(), ov(o.sistema));
        let mut fs_ = HashMap::new();
        let pontos: Vec<Vec<u8>> = o.montagem.iter().cloned().collect();
        fs_.insert("MountPoints".into(), ov(pontos));
        let mut ifaces = HashMap::new();
        ifaces.insert("org.freedesktop.UDisks2.Block".to_owned(), bloco);
        ifaces.insert("org.freedesktop.UDisks2.Filesystem".to_owned(), fs_);
        if o.de_loop {
            ifaces.insert("org.freedesktop.UDisks2.Loop".to_owned(), HashMap::new());
        }
        m.insert(OwnedObjectPath::try_from(o.caminho).unwrap(), ifaces);
    }
    m
}

struct Gerente {
    lista: Arc<Mutex<Vec<Objeto>>>,
}

#[zbus::interface(name = "org.freedesktop.DBus.ObjectManager")]
impl Gerente {
    fn get_managed_objects(&self) -> Objetos {
        objetos(&self.lista.lock().unwrap())
    }
}

/// O udisks2 falso: dono de `org.freedesktop.UDisks2` no barramento falso.
struct UdisksFalso {
    conn: Connection,
    lista: Arc<Mutex<Vec<Objeto>>>,
}

impl UdisksFalso {
    fn novo(b: &BarramentoFalso, lista: Vec<Objeto>) -> UdisksFalso {
        let lista = Arc::new(Mutex::new(lista));
        let conn = zbus::block_on(
            Builder::address(b.endereco().as_str())
                .name("org.freedesktop.UDisks2")
                .serve_at(
                    "/org/freedesktop/UDisks2",
                    Gerente {
                        lista: lista.clone(),
                    },
                )
                .build(),
        )
        .unwrap();
        UdisksFalso { conn, lista }
    }

    /// `PropertiesChanged` do `MountPoints` (o que o udisks2 manda ao montar).
    fn montou(&self, caminho: &str) {
        let mudou: HashMap<&str, Variant<'_>> = HashMap::new();
        zbus::block_on(self.conn.emit_signal(
            None::<&str>,
            caminho,
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
            &(
                "org.freedesktop.UDisks2.Filesystem",
                mudou,
                vec!["MountPoints"],
            ),
        ))
        .unwrap();
    }
}

fn objeto(caminho: &'static str, uuid: &str, dir: Option<&Path>) -> Objeto {
    Objeto {
        caminho,
        uuid: uuid.into(),
        rotulo: "PEN".into(),
        dispositivo: dir.map_or(0, |d| fs::metadata(d).unwrap().dev()),
        montagem: dir.map(|d| {
            let mut b = d.as_os_str().as_encoded_bytes().to_vec();
            b.push(0);
            b
        }),
        sistema: false,
        de_loop: false,
    }
}

/// O que sai do `GetManagedObjects`: só pendrive (não sistema) ou loop, UUID no
/// formato, rótulo sem controle, montagem conferida.
#[test]
fn udisks_falso_e_lido_com_as_conferencias() {
    let dir = Dir::novo("ext-udisks-ler");
    let a = montagem(&dir, "pendrive-a");
    let b = BarramentoFalso::subir();
    let mut sistema = objeto(
        "/org/freedesktop/UDisks2/block_devices/nvme0n1p2",
        "1111-2222",
        Some(&a),
    );
    sistema.sistema = true;
    let mut de_loop = objeto("/org/freedesktop/UDisks2/block_devices/loop7", UUID_B, None);
    de_loop.sistema = true;
    de_loop.de_loop = true;
    let mut sujo = objeto(
        "/org/freedesktop/UDisks2/block_devices/sdc1",
        UUID_A,
        Some(&a),
    );
    sujo.rotulo = "PEN\u{1b}[2J".into();
    let ruim = objeto(
        "/org/freedesktop/UDisks2/block_devices/sdd1",
        "../etc",
        Some(&a),
    );
    let mut estranho = objeto(
        "/org/freedesktop/UDisks2/block_devices/sde1",
        "AAAA-BBBB",
        None,
    );
    estranho.montagem = Some(b"/media/u/../../etc\0".to_vec());
    let _u = UdisksFalso::novo(&b, vec![sistema, de_loop, sujo, ruim, estranho]);
    let fonte = wardd::udisks::Udisks::novo(b.socket.clone());
    let vistos = fonte.pendrives().expect("udisks2 falso respondeu");
    let uuids: Vec<&str> = vistos.iter().map(|v| v.uuid.as_str()).collect();
    assert_eq!(uuids, ["0F3C-9A51", "AAAA-BBBB", "ABCD-1234"]);
    let pa = vistos.iter().find(|v| v.uuid == UUID_A).unwrap();
    assert_eq!(pa.rotulo, "PEN[2J");
    assert_eq!(pa.montagem.as_deref(), Some(a.as_path()));
    assert_eq!(pa.dispositivo, fs::metadata(&a).unwrap().dev());
    assert!(
        vistos
            .iter()
            .all(|v| v.uuid != "AAAA-BBBB" || v.montagem.is_none())
    );
    // Barramento sem udisks2: `None`.
    let b2 = BarramentoFalso::subir();
    assert_eq!(
        wardd::udisks::Udisks::novo(b2.socket.clone()).pendrives(),
        None
    );
}

/// Ponta a ponta com as threads do daemon: ligado com o UUID do A, nada enquanto ele
/// não está montado; o sinal de montagem do udisks2 faz a cópia chegar ao A, e o B
/// montado ao mesmo tempo não recebe nada.
#[test]
fn v7_5_sinal_do_udisks_copia_para_o_pendrive_montado() {
    let dir = Dir::novo("ext-udisks-sinal");
    let (d, _) = cofre_de_teste(&dir);
    let a = montagem(&dir, "pendrive-a");
    let bdir = montagem(&dir, "pendrive-b");
    let b = BarramentoFalso::subir();
    let u = UdisksFalso::novo(
        &b,
        vec![
            objeto("/org/freedesktop/UDisks2/block_devices/sdb1", UUID_A, None),
            objeto(
                "/org/freedesktop/UDisks2/block_devices/sdc1",
                UUID_B,
                Some(&bdir),
            ),
        ],
    );
    let fonte = Arc::new(wardd::udisks::Udisks::novo(b.socket.clone()));
    let d = Arc::new(d.com_pendrives(fonte.clone()));
    assert!(ok(&ligar(&d, Papel::App, UUID_A)));
    let cutucoes = d.ligar_copia_externa().unwrap();
    assert!(d.ligar_copia_externa().is_none(), "só uma thread de cópia");
    wardd::udisks::vigiar_copia(d.clone(), fonte, cutucoes).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    wardd::udisks::vigiar_udisks(
        d.clone(),
        b.socket.clone(),
        Box::new(move || {
            let _ = tx.send(());
        }),
    )
    .unwrap();
    rx.recv_timeout(Duration::from_secs(10))
        .expect("vigia conectou");
    match pedir(&d, Papel::App, "pendrives", "{}") {
        Resposta::Pendrives(l) => {
            assert_eq!(l.len(), 2);
            assert!(!l.iter().find(|p| p.uuid == UUID_A).unwrap().montado);
        }
        r => panic!("{r:?}"),
    }
    std::thread::sleep(wardd::udisks::ESPERA_RAJADA * 3);
    assert!(arvore(&a).is_empty() && arvore(&bdir).is_empty());
    // O A é montado.
    u.lista.lock().unwrap()[0] = objeto(
        "/org/freedesktop/UDisks2/block_devices/sdb1",
        UUID_A,
        Some(&a),
    );
    u.montou("/org/freedesktop/UDisks2/block_devices/sdb1");
    let fim = Instant::now() + Duration::from_secs(15);
    let total = locais(&dir).len();
    while no_pendrive(&a).len() < total {
        assert!(Instant::now() < fim, "a cópia não chegou ao pendrive A");
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(arvore(&bdir).is_empty(), "o B não recebe nada");
    // Uma gravação cutuca a cópia de novo.
    let r = pedir(
        &d,
        Papel::App,
        "salvar_entrada",
        &login("t-sinal", SEGREDOS[3]),
    );
    assert!(ok(&r));
    let fim = Instant::now() + Duration::from_secs(15);
    while no_pendrive(&a).len() < total + 1 {
        assert!(
            Instant::now() < fim,
            "a gravação nova não chegou ao pendrive"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Sem `link(2)` no caminho do pendrive (o vfat não tem): a cópia publica só por
/// `renameat2(RENAME_NOREPLACE)`.
#[test]
fn externo_nao_usa_link() {
    let fonte = include_str!("../src/externo.rs");
    for proibido in ["hard_link", "linkat", "libc::link", "fs::symlink("] {
        assert!(!fonte.contains(proibido), "{proibido} em externo.rs");
    }
    assert!(fonte.contains("renomear_sem_sobrescrever(destino.as_fd()"));
}
