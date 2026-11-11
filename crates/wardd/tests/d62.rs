//! D62 no núcleo do daemon (Fase 22b): espera do D20 por papel e guardada no arquivo
//! de estado (V3.33, S9), `criar_cofre` com estado velho (V3.35, S7) e identidade do
//! cofre gravado lida do fd do temporário (V3.35, S7, S13).
//!
//! Cofre de teste próprio, só com valores falsos (nunca o cofre real nem o de `.dev/`).

mod comum;

use std::fs;
#[cfg(feature = "test-hooks")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use comum::*;
use ward_core::config::{PinHash, VaultConfig};
use ward_core::format::{MasterPassword, OpenVault};
use ward_core::protocol::{MSG_ROLLBACK, ResponseError, Resposta, Role};
use wardd::arquivos::{self, Geracoes};
use wardd::nucleo::{Caminhos, Daemon, ESPERA_MAXIMA, Relogio, espera_destrave};

/// Relógio com hora controlada pelo teste: anota as esperas e não dorme.
#[derive(Clone, Default)]
struct RelogioMovel {
    agora: Arc<Mutex<Duration>>,
    esperas: Arc<Mutex<Vec<Duration>>>,
}

impl RelogioMovel {
    fn andar(&self, d: Duration) {
        *self.agora.lock().unwrap() += d;
    }

    fn esperas(&self) -> Vec<Duration> {
        std::mem::take(&mut *self.esperas.lock().unwrap())
    }
}

impl Relogio for RelogioMovel {
    fn dormir(&self, d: Duration) {
        self.esperas.lock().unwrap().push(d);
    }

    fn agora(&self) -> Option<Duration> {
        Some(*self.agora.lock().unwrap())
    }
}

fn daemon_movel(c: Caminhos, r: &RelogioMovel) -> Daemon {
    Daemon::novo(c, Box::new(r.clone()))
}

fn destravar_como(d: &Daemon, papel: Role, senha: &str) -> Resposta {
    pedir(
        d,
        papel,
        "destravar",
        &format!(r#"{{"senha":"{}"}}"#, b64(senha.as_bytes())),
    )
}

fn texto_estado(dir: &Dir) -> String {
    fs::read_to_string(dir.caminhos().estado).unwrap()
}

fn falhas_no_disco(dir: &Dir) -> u32 {
    arquivos::ler_estado_completo(&dir.caminhos().estado)
        .unwrap()
        .falhas_agente
}

/// Cofre de teste criado e travado; o daemon que o criou é descartado.
fn cofre_travado(dir: &Dir) {
    let (d, _) = cofre_de_teste(dir);
    d.travar();
}

/// V3.33 (D62, S9): 10 senhas erradas do agente, o `wardd` morre e sobe de novo, e a
/// tentativa seguinte do agente ainda espera 30 s; o contador está no disco antes de a
/// resposta de erro sair.
#[test]
fn v3_33_espera_do_agente_sobrevive_ao_reinicio() {
    let dir = Dir::novo("d62-reinicio");
    cofre_travado(&dir);
    let r = RelogioMovel::default();
    let d = daemon_movel(dir.caminhos(), &r);
    for i in 0..10u32 {
        let resp = destravar_como(&d, Role::Agent, &format!("falso-errada-{i:02}"));
        assert_eq!(erro(&resp), Some(ResponseError::WrongPassword));
        // Já no disco quando a resposta sai (fsync antes da resposta).
        assert_eq!(falhas_no_disco(&dir), i + 1);
    }
    let esperas: Vec<u64> = r.esperas().iter().map(Duration::as_secs).collect();
    assert_eq!(esperas, [0, 1, 2, 4, 8, 16, 30, 30, 30, 30]);

    // "kill -9": o daemon some sem nada além do que já foi para o disco.
    drop(d);
    let r = RelogioMovel::default();
    let d = daemon_movel(dir.caminhos(), &r);
    assert_eq!(
        erro(&destravar_como(&d, Role::Agent, "falso-errada-xx")),
        Some(ResponseError::WrongPassword)
    );
    assert_eq!(r.esperas(), [ESPERA_MAXIMA]);
    assert_eq!(falhas_no_disco(&dir), 11);

    // Contada da partida: o daemon novo subiu há 20 s, a primeira espera é de 10 s.
    drop(d);
    let r = RelogioMovel::default();
    let d = daemon_movel(dir.caminhos(), &r);
    r.andar(Duration::from_secs(20));
    let _ = destravar_como(&d, Role::Agent, "falso-errada-yy");
    // A seguinte já espera a inteira.
    let _ = destravar_como(&d, Role::Agent, "falso-errada-zz");
    assert_eq!(
        r.esperas(),
        [Duration::from_secs(10), ESPERA_MAXIMA],
        "a 1.ª depois da partida conta da partida; as outras, inteiras"
    );
    // Subiu há mais de 30 s: a primeira não espera, mas o contador continua.
    drop(d);
    let r = RelogioMovel::default();
    let d = daemon_movel(dir.caminhos(), &r);
    r.andar(Duration::from_secs(45));
    let _ = destravar_como(&d, Role::Agent, "falso-errada-ww");
    let _ = destravar_como(&d, Role::Agent, "falso-errada-vv");
    assert_eq!(r.esperas(), [Duration::ZERO, ESPERA_MAXIMA]);

    // O destravar certo zera os dois, no disco também.
    assert!(ok(&destravar_como(&d, Role::Agent, SENHA)));
    assert_eq!(falhas_no_disco(&dir), 0);
    d.travar();
    let _ = destravar_como(&d, Role::Agent, "falso-errada-uu");
    let _ = destravar_como(&d, Role::App, "falso-errada-tt");
    assert_eq!(r.esperas()[1..], [Duration::ZERO, Duration::ZERO]);
}

/// V3.33 (D62): o agente no meio das esperas não atrasa o app, que destrava com a senha
/// certa sem esperar; depois do destravar certo do app, os dois contadores zeram.
#[test]
fn ataque_agente_no_meio_da_espera_nao_atrasa_o_app() {
    let dir = Dir::novo("d62-papel");
    cofre_travado(&dir);

    /// Relógio cujo `dormir` de 4 s ou mais (a espera do agente) fica preso até o teste
    /// soltar; as esperas menores voltam na hora.
    struct Preso {
        entrou: Mutex<mpsc::Sender<Duration>>,
        soltar: Mutex<mpsc::Receiver<()>>,
        esperas: Arc<Mutex<Vec<(Duration, bool)>>>,
    }
    impl Relogio for Preso {
        fn dormir(&self, d: Duration) {
            let preso = d >= Duration::from_secs(4);
            self.esperas.lock().unwrap().push((d, preso));
            if preso {
                let _ = self.entrou.lock().unwrap().send(d);
                let _ = self.soltar.lock().unwrap().recv();
            }
        }
    }
    let (tx_entrou, rx_entrou) = mpsc::channel();
    let (tx_soltar, rx_soltar) = mpsc::channel();
    let esperas = Arc::new(Mutex::new(Vec::new()));
    let d = Arc::new(Daemon::novo(
        dir.caminhos(),
        Box::new(Preso {
            entrou: Mutex::new(tx_entrou),
            soltar: Mutex::new(rx_soltar),
            esperas: esperas.clone(),
        }),
    ));
    // Três erradas do agente: a próxima dele espera 4 s.
    for i in 0..3 {
        let resp = destravar_como(&d, Role::Agent, &format!("falso-errada-{i}"));
        assert_eq!(erro(&resp), Some(ResponseError::WrongPassword));
    }
    let agente = {
        let d = d.clone();
        std::thread::spawn(move || destravar_como(&d, Role::Agent, "falso-errada-3"))
    };
    // O agente está preso na espera de 4 s.
    assert_eq!(
        rx_entrou.recv_timeout(Duration::from_secs(20)).unwrap(),
        Duration::from_secs(4)
    );
    // O app acerta sem esperar: o contador dele é outro, e a vez do agente não o segura.
    assert!(ok(&destravar_como(&d, Role::App, SENHA)));
    assert!(!d.travado());
    tx_soltar.send(()).unwrap();
    // O agente acorda com o cofre já aberto.
    assert!(ok(&agente.join().unwrap()));
    let s = Duration::from_secs;
    assert_eq!(
        *esperas.lock().unwrap(),
        [
            (s(0), false),
            (s(1), false),
            (s(2), false),
            (s(4), true),
            (s(0), false)
        ],
        "o app não esperou nada"
    );
    // Os dois zeraram (no disco também).
    assert_eq!(falhas_no_disco(&dir), 0);
    d.travar();
    esperas.lock().unwrap().clear();
    let _ = destravar_como(&d, Role::Agent, "falso-errada-4");
    let _ = destravar_como(&d, Role::App, "falso-errada-5");
    // Erro do app não atrasa o agente, e vice-versa.
    let _ = destravar_como(&d, Role::Agent, "falso-errada-6");
    assert_eq!(
        *esperas.lock().unwrap(),
        [(s(0), false), (s(0), false), (s(1), false)]
    );
}

/// V3.33 (D62): o estado no formato antigo (três linhas) é lido como contador 0, e a
/// primeira senha errada regrava no formato novo; o arquivo não tem senha, PIN nem
/// segredo, só números.
#[test]
fn v3_33_formato_antigo_e_estado_sem_segredo() {
    let dir = Dir::novo("d62-antigo");
    cofre_travado(&dir);
    let g = geracoes(&dir);
    fs::write(
        dir.caminhos().estado,
        format!(
            "ward-estado 1\nvista {}\nqueimada {}\n",
            g.vista, g.queimada
        ),
    )
    .unwrap();
    let r = RelogioMovel::default();
    let d = daemon_movel(dir.caminhos(), &r);
    let _ = destravar_como(&d, Role::Agent, "falso-errada-01");
    assert_eq!(r.esperas(), [Duration::ZERO]);
    assert_eq!(
        texto_estado(&dir),
        format!(
            "ward-estado 1\nvista {}\nqueimada {}\nfalhas_agente 1\n",
            g.vista, g.queimada
        )
    );
    // O app não vai para o disco.
    let _ = destravar_como(&d, Role::App, "falso-errada-02");
    assert_eq!(falhas_no_disco(&dir), 1);
    let texto = fs::read(dir.caminhos().estado).unwrap();
    for s in SEGREDOS
        .iter()
        .chain([&SENHA, &PIN, &"falso-errada-01", &"falso-errada-02"])
    {
        assert!(!contem(&texto, s.as_bytes()), "{s}");
        assert!(
            !contem(&texto, b64(s.as_bytes()).as_bytes()),
            "{s} em base64"
        );
    }
    assert!(
        String::from_utf8(texto).unwrap().lines().skip(1).all(|l| l
            .split_once(' ')
            .is_some_and(|(_, n)| n.parse::<u64>().is_ok())),
        "só números depois do cabeçalho"
    );
    // A espera continua sendo a mesma tabela de D20.
    assert_eq!(espera_destrave(1), Duration::from_secs(1));
}

/// V3.35 (D62, S7): estado com `vista` = `queimada` = 7 e nenhum cofre: o cofre novo
/// nasce na geração 8 e o estado vai a 8/8; a cópia de um cofre anterior na geração 5,
/// posta no lugar, é recusada como retrocesso.
#[test]
fn v3_35_criar_cofre_com_estado_velho_e_copia_antiga() {
    let dir = Dir::novo("d62-criar");
    let c = dir.caminhos();
    // Um cofre anterior, na geração 5, com a mesma senha de teste (o atacante guardou).
    let antigo = dir.0.join("antigo.ward");
    let senha = MasterPassword::new(SENHA.to_owned());
    let mut velho = OpenVault::new(
        &senha,
        VaultConfig::new(PinHash::create(PIN.as_bytes()).unwrap()),
    )
    .unwrap()
    .after_generation(4);
    ward_core::atomic_write::write_entries(&antigo, &mut velho, &[]).unwrap();
    assert_eq!(geracao_no_disco(&antigo, SENHA), 5);
    arquivos::gravar_estado(
        &c.estado,
        Geracoes {
            vista: 7,
            queimada: 7,
        },
    )
    .unwrap();
    assert!(!c.cofre.exists());

    let (d, _) = daemon_em(c.clone());
    let criar = pedir(
        &d,
        Role::App,
        "criar_cofre",
        &format!(
            r#"{{"senha":"{}","pin":"{}"}}"#,
            b64(SENHA.as_bytes()),
            b64(PIN.as_bytes())
        ),
    );
    assert!(ok(&criar), "{criar:?}");
    assert_eq!(geracao_no_disco(&c.cofre, SENHA), 8);
    assert_eq!(
        geracoes(&dir),
        Geracoes {
            vista: 8,
            queimada: 8
        }
    );

    // O atacante põe a cópia antiga no lugar e destrava.
    d.travar();
    fs::copy(&antigo, &c.cofre).unwrap();
    let r = destravar(&d, SENHA);
    assert_eq!(erro(&r), Some(ResponseError::Rollback));
    assert_eq!(ResponseError::Rollback.message(), MSG_ROLLBACK);
    assert!(d.travado());
    // Nem um daemon novo abre.
    let (d2, _) = daemon_em(c.clone());
    assert_eq!(erro(&destravar(&d2, SENHA)), Some(ResponseError::Rollback));
}

/// V3.35 (D62): sem estado, o cofre nasce na geração 1 (primeiro uso de sempre); com
/// estado ilegível e sem cofre, o `criar_cofre` recusa em vez de começar do zero.
#[test]
fn v3_35_criar_cofre_sem_estado_e_com_estado_invalido() {
    let dir = Dir::novo("d62-criar-sem");
    let c = dir.caminhos();
    let criar = format!(
        r#"{{"senha":"{}","pin":"{}"}}"#,
        b64(SENHA.as_bytes()),
        b64(PIN.as_bytes())
    );
    fs::write(&c.estado, "ward-estado 1\nvista 9\n").unwrap();
    let (d, _) = daemon_em(c.clone());
    assert_eq!(
        erro(&pedir(&d, Role::App, "criar_cofre", &criar)),
        Some(ResponseError::WriteFailed)
    );
    assert!(!c.cofre.exists());
    fs::remove_file(&c.estado).unwrap();
    assert!(ok(&pedir(&d, Role::App, "criar_cofre", &criar)));
    assert_eq!(geracao_no_disco(&c.cofre, SENHA), 1);
    assert_eq!(
        geracoes(&dir),
        Geracoes {
            vista: 1,
            queimada: 1
        }
    );
}

/// Troca o arquivo do cofre por uma cópia de bytes iguais em outro inode (o que um
/// processo do mesmo usuário faria logo depois do rename).
#[cfg(feature = "test-hooks")]
fn trocar_por_copia(c: &Caminhos) {
    let bytes = fs::read(&c.cofre).unwrap();
    let tmp = c.cofre.with_extension("troca");
    fs::write(&tmp, &bytes).unwrap();
    fs::rename(&tmp, &c.cofre).unwrap();
}

/// V3.35 (D62, S7, S13): um gancho troca o arquivo logo depois do rename do
/// `write_entries`. A identidade que o daemon guarda é a do fd do temporário, então
/// a gravação seguinte responde "o arquivo do cofre mudou fora do Ward", trava e não
/// grava. Com o `stat` depois do rename, a troca passaria como sendo do Ward.
#[cfg(feature = "test-hooks")]
#[test]
fn v3_35_troca_logo_depois_do_rename_e_recusada() {
    let dir = Dir::novo("d62-rename");
    cofre_travado(&dir);
    let c = dir.caminhos();
    let armado = Arc::new(AtomicBool::new(false));
    let trocou = Arc::new(AtomicBool::new(false));
    let d = {
        let (armado, trocou, c) = (armado.clone(), trocou.clone(), c.clone());
        Daemon::novo(c.clone(), Box::new(RelogioFalso::default())).com_gancho_depois_de_gravar(
            Box::new(move || {
                if armado.swap(false, Ordering::SeqCst) {
                    trocar_por_copia(&c);
                    trocou.store(true, Ordering::SeqCst);
                }
            }),
        )
    };
    assert!(ok(&destravar(&d, SENHA)));
    // Sem troca, gravar duas vezes funciona.
    assert!(ok(&pedir(
        &d,
        Role::App,
        "tempo_trava",
        r#"{"minutos":30}"#
    )));
    armado.store(true, Ordering::SeqCst);
    assert!(ok(&pedir(
        &d,
        Role::App,
        "tempo_trava",
        r#"{"minutos":31}"#
    )));
    assert!(trocou.load(Ordering::SeqCst), "o gancho não rodou");
    let trocado = fs::read(&c.cofre).unwrap();
    let r = pedir(&d, Role::App, "tempo_trava", r#"{"minutos":32}"#);
    assert_eq!(erro(&r), Some(ResponseError::VaultChanged));
    assert_eq!(
        ResponseError::VaultChanged.message(),
        ward_core::protocol::MSG_VAULT_CHANGED
    );
    assert!(d.travado());
    assert_eq!(
        fs::read(&c.cofre).unwrap(),
        trocado,
        "gravou por cima da troca"
    );

    // O mesmo na troca de senha: a identidade vem do fd da gravação dela.
    let dir2 = Dir::novo("d62-rename-senha");
    cofre_travado(&dir2);
    let c2 = dir2.caminhos();
    let armado2 = Arc::new(AtomicBool::new(false));
    let d2 = {
        let (armado2, c2) = (armado2.clone(), c2.clone());
        Daemon::novo(c2.clone(), Box::new(RelogioFalso::default())).com_gancho_depois_de_gravar(
            Box::new(move || {
                if armado2.swap(false, Ordering::SeqCst) {
                    trocar_por_copia(&c2);
                }
            }),
        )
    };
    assert!(ok(&destravar(&d2, SENHA)));
    armado2.store(true, Ordering::SeqCst);
    let nova = "falso-nova-ward-03";
    assert!(ok(&pedir(
        &d2,
        Role::App,
        "trocar_senha",
        &format!(
            r#"{{"atual":"{}","nova":"{}"}}"#,
            b64(SENHA.as_bytes()),
            b64(nova.as_bytes())
        ),
    )));
    let trocado = fs::read(&c2.cofre).unwrap();
    let r = pedir(&d2, Role::App, "tempo_trava", r#"{"minutos":33}"#);
    assert_eq!(erro(&r), Some(ResponseError::VaultChanged));
    assert!(d2.travado());
    assert_eq!(fs::read(&c2.cofre).unwrap(), trocado);
}
