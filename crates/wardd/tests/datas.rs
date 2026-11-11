//! D65 d (Fase 22d, provisória): datas de criação e alteração carimbadas pelo relógio
//! do daemon (injetado aqui). Entrada nova ganha as duas; edição e `mudar_nivel` trocam
//! só `alterada_em`; reabrir, renomear e restaurar mantêm as datas; elas só saem no
//! `detalhe` do papel app (V3.41). Cofre de teste próprio, só valores falsos.

mod comum;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use comum::{Dir, PIN, SEGREDOS, b64, erro, login, mudar_nivel, ok, pedir};
use ward_core::protocol::{self, DetalheEntrada, ResponseError, Resposta, Role, decode_response};
use wardd::nucleo::{Daemon, Relogio};

/// 2026-11-02 00:00 UTC e seguintes (o rodapé do frame aprovado no M0).
const T0: u64 = 1_762_041_600;
const DIA: u64 = 86_400;

/// Relógio de parede ajustável pelo teste; 0 = ilegível.
#[derive(Clone)]
struct RelogioDeParede(Arc<AtomicU64>);

impl RelogioDeParede {
    fn em(&self, s: u64) {
        self.0.store(s, Ordering::SeqCst);
    }
}

impl Relogio for RelogioDeParede {
    fn dormir(&self, _d: Duration) {}

    fn agora(&self) -> Option<Duration> {
        Some(Duration::from_secs(1000))
    }

    fn segundos_unix(&self) -> Option<u64> {
        Some(self.0.load(Ordering::SeqCst)).filter(|s| *s != 0)
    }
}

fn daemon(dir: &Dir, relogio: &RelogioDeParede) -> Daemon {
    Daemon::novo(dir.caminhos(), Box::new(relogio.clone()))
}

fn criar(dir: &Dir, relogio: &RelogioDeParede) -> Daemon {
    let d = daemon(dir, relogio);
    let r = pedir(
        &d,
        Role::App,
        "criar_cofre",
        &format!(
            r#"{{"senha":"{}","pin":"{}"}}"#,
            b64(comum::SENHA.as_bytes()),
            b64(PIN.as_bytes())
        ),
    );
    assert!(ok(&r), "{r:?}");
    d
}

fn detalhe(d: &Daemon, nome: &str) -> DetalheEntrada {
    match pedir(
        d,
        Role::App,
        "detalhe",
        &format!(r#"{{"entrada":"{nome}"}}"#),
    ) {
        Resposta::Detalhe(x) => x,
        outra => panic!("{outra:?}"),
    }
}

fn datas(d: &Daemon, nome: &str) -> (Option<u64>, Option<u64>) {
    let x = detalhe(d, nome).datas;
    (x.created_at(), x.modified_at())
}

fn salvar(d: &Daemon, dados: &str) {
    let r = pedir(d, Role::App, "salvar_entrada", dados);
    assert!(ok(&r), "{r:?}");
}

/// `login(nome)` editando `anterior`.
fn editar(anterior: &str, nome: &str, senha: &str) -> String {
    login(nome, senha).replacen('{', &format!(r#"{{"anterior":"{anterior}","#), 1)
}

/// V3.41: nova → as duas; edição e `mudar_nivel` → só a alteração; relógio que volta
/// não faz a data descer; reabrir num daemon novo devolve as mesmas datas.
#[test]
fn v3_41_datas_pelo_relogio_do_daemon() {
    let dir = Dir::novo("datas-v3-41");
    let relogio = RelogioDeParede(Arc::new(AtomicU64::new(T0)));
    let d = criar(&dir, &relogio);

    salvar(&d, &login("t-datas", SEGREDOS[0]));
    assert_eq!(datas(&d, "t-datas"), (Some(T0), Some(T0)));

    relogio.em(T0 + DIA);
    salvar(&d, &editar("t-datas", "t-datas", SEGREDOS[1]));
    assert_eq!(datas(&d, "t-datas"), (Some(T0), Some(T0 + DIA)));

    relogio.em(T0 + 2 * DIA);
    assert!(ok(&mudar_nivel(&d, Role::App, "t-datas", "livre", PIN)));
    assert_eq!(datas(&d, "t-datas"), (Some(T0), Some(T0 + 2 * DIA)));

    // PIN errado não muda nada, nem a data.
    relogio.em(T0 + 3 * DIA);
    let r = mudar_nivel(&d, Role::App, "t-datas", "bloqueado", comum::PIN_ERRADO);
    assert_eq!(erro(&r), Some(ResponseError::WrongPin));
    assert_eq!(datas(&d, "t-datas"), (Some(T0), Some(T0 + 2 * DIA)));

    // Relógio que volta: a alteração não desce, a criação nunca muda.
    relogio.em(T0 - DIA);
    salvar(&d, &editar("t-datas", "t-datas", SEGREDOS[2]));
    assert_eq!(datas(&d, "t-datas"), (Some(T0), Some(T0 + 2 * DIA)));

    // Renomear é editar: a criação vai junto.
    relogio.em(T0 + 4 * DIA);
    salvar(&d, &editar("t-datas", "t-renomeada", SEGREDOS[2]));
    assert_eq!(datas(&d, "t-renomeada"), (Some(T0), Some(T0 + 4 * DIA)));

    // Outra entrada nova não mexe nas datas da primeira.
    relogio.em(T0 + 5 * DIA);
    salvar(&d, &login("t-outra", SEGREDOS[3]));
    assert_eq!(
        datas(&d, "t-outra"),
        (Some(T0 + 5 * DIA), Some(T0 + 5 * DIA))
    );
    assert_eq!(datas(&d, "t-renomeada"), (Some(T0), Some(T0 + 4 * DIA)));

    // O daemon reabre o cofre (outro processo): as mesmas datas, lidas do corpo v4.
    drop(d);
    relogio.em(T0 + 9 * DIA);
    let d2 = daemon(&dir, &relogio);
    assert!(ok(&comum::destravar(&d2, comum::SENHA)));
    assert_eq!(datas(&d2, "t-renomeada"), (Some(T0), Some(T0 + 4 * DIA)));
    assert_eq!(
        datas(&d2, "t-outra"),
        (Some(T0 + 5 * DIA), Some(T0 + 5 * DIA))
    );
}

/// V3.41: as datas só chegam no `detalhe` do papel app. O agente e a extensão não pedem
/// `detalhe`, e a lista (do app e do agente) não leva data nenhuma.
#[test]
fn ataque_v3_41_datas_nao_vao_ao_agente() {
    let dir = Dir::novo("datas-agente");
    let relogio = RelogioDeParede(Arc::new(AtomicU64::new(T0)));
    let d = criar(&dir, &relogio);
    salvar(&d, &login("t-datas", SEGREDOS[0]));
    for papel in [Role::Agent, Role::Extension] {
        let r = pedir(&d, papel, "detalhe", r#"{"entrada":"t-datas"}"#);
        assert_eq!(erro(&r), Some(ResponseError::Unauthorized), "{papel:?}");
    }
    for papel in [Role::App, Role::Agent] {
        let r = pedir(&d, papel, "lista", "{}");
        let q = protocol::encode_response(papel, &r).unwrap();
        let s = String::from_utf8_lossy(&q);
        assert!(
            !s.contains("criada_em") && !s.contains("alterada_em"),
            "{s}"
        );
        assert!(!s.contains(&T0.to_string()), "{s}");
    }
    // O quadro do detalhe do app traz as datas e não passa pela leitura do agente.
    let r = pedir(&d, Role::App, "detalhe", r#"{"entrada":"t-datas"}"#);
    let q = protocol::encode_response(Role::App, &r).unwrap();
    assert!(String::from_utf8_lossy(&q).contains(&format!(r#""criada_em":{T0}"#)));
    assert!(decode_response(Role::Agent, &q).is_err());
    // Nenhum segredo da entrada no quadro com as datas.
    for s in SEGREDOS {
        assert!(!q.windows(s.len()).any(|w| w == s.as_bytes()));
        let b = b64(s.as_bytes());
        assert!(!q.windows(b.len()).any(|w| w == b.as_bytes()));
    }
}

/// V3.41: data no rascunho do `salvar_entrada` é recusada antes de chegar ao núcleo, e
/// o cofre fica como estava (mesmos bytes no disco, mesmas datas).
#[test]
fn ataque_v3_41_data_vinda_do_cliente_recusada() {
    let dir = Dir::novo("datas-cliente");
    let relogio = RelogioDeParede(Arc::new(AtomicU64::new(T0)));
    let d = criar(&dir, &relogio);
    salvar(&d, &login("t-datas", SEGREDOS[0]));
    let antes = std::fs::read(&dir.caminhos().cofre).unwrap();
    relogio.em(T0 + DIA);
    for forjado in [
        editar("t-datas", "t-datas", SEGREDOS[1]).replacen(
            r#""nome":"t-datas","grupo""#,
            r#""nome":"t-datas","criada_em":1,"grupo""#,
            1,
        ),
        editar("t-datas", "t-datas", SEGREDOS[1]).replacen(
            r#""nome":"t-datas","grupo""#,
            r#""nome":"t-datas","alterada_em":1,"grupo""#,
            1,
        ),
        login("t-nova", SEGREDOS[1]).replacen('{', r#"{"criada_em":1,"#, 1),
        login("t-nova", SEGREDOS[1]).replacen(r#""url":"#, r#""alterada_em":1,"url":"#, 1),
    ] {
        assert!(forjado.contains("_em\":1"), "{forjado}");
        let r = pedir(&d, Role::App, "salvar_entrada", &forjado);
        assert_eq!(erro(&r), Some(ResponseError::InvalidRequest), "{forjado}");
    }
    assert_eq!(datas(&d, "t-datas"), (Some(T0), Some(T0)));
    assert!(std::fs::read(&dir.caminhos().cofre).unwrap() == antes);
    assert!(erro(&pedir(&d, Role::App, "detalhe", r#"{"entrada":"t-nova"}"#)).is_some());
}

/// Relógio ilegível: a gravação segue sem carimbo novo (entrada nova sem datas, edição
/// mantém as anteriores); com o relógio de volta, a entrada sem criação ganha só a
/// alteração.
#[test]
fn relogio_ilegivel_grava_sem_data_nova() {
    let dir = Dir::novo("datas-sem-relogio");
    let relogio = RelogioDeParede(Arc::new(AtomicU64::new(T0)));
    let d = criar(&dir, &relogio);
    salvar(&d, &login("t-com", SEGREDOS[0]));
    relogio.em(0);
    salvar(&d, &login("t-sem", SEGREDOS[1]));
    assert_eq!(datas(&d, "t-sem"), (None, None));
    salvar(&d, &editar("t-com", "t-com", SEGREDOS[2]));
    assert_eq!(datas(&d, "t-com"), (Some(T0), Some(T0)));
    assert!(ok(&mudar_nivel(&d, Role::App, "t-com", "livre", PIN)));
    assert_eq!(datas(&d, "t-com"), (Some(T0), Some(T0)));
    relogio.em(T0 + DIA);
    salvar(&d, &editar("t-sem", "t-sem", SEGREDOS[3]));
    assert_eq!(datas(&d, "t-sem"), (None, Some(T0 + DIA)));
    // Relógio além de 9999-12-31: como ilegível.
    relogio.em(u64::MAX);
    salvar(&d, &editar("t-sem", "t-sem", SEGREDOS[1]));
    assert_eq!(datas(&d, "t-sem"), (None, Some(T0 + DIA)));
}

/// D65 d: restaurar um backup mantém as datas da versão restaurada.
#[test]
fn restaurar_mantem_as_datas() {
    let dir = Dir::novo("datas-restaurar");
    let relogio = RelogioDeParede(Arc::new(AtomicU64::new(T0)));
    let d = criar(&dir, &relogio);
    salvar(&d, &login("t-datas", SEGREDOS[0]));
    let versao = match pedir(&d, Role::App, "backups", "{}") {
        Resposta::Backups(l) => l[0].nome.clone(),
        outra => panic!("{outra:?}"),
    };
    relogio.em(T0 + 3 * DIA);
    salvar(&d, &editar("t-datas", "t-datas", SEGREDOS[1]));
    assert_eq!(datas(&d, "t-datas"), (Some(T0), Some(T0 + 3 * DIA)));
    relogio.em(T0 + 7 * DIA);
    let r = pedir(
        &d,
        Role::App,
        "restaurar_backup",
        &format!(
            r#"{{"versao":"{versao}","senha":"{}","pin":"{}"}}"#,
            b64(comum::SENHA.as_bytes()),
            b64(PIN.as_bytes())
        ),
    );
    assert!(ok(&r), "{r:?}");
    assert_eq!(datas(&d, "t-datas"), (Some(T0), Some(T0)));
}
