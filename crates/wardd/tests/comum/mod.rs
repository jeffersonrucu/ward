//! Apoio dos testes do `wardd`: diretórios de teste, pedidos em JSON, relógio falso e
//! um cofre de teste próprio (só valores falsos; nunca o cofre real nem o de `.dev/`).

#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ward_core::protocolo::{self, ErroResposta, Papel, Pedido, Resposta};
use wardd::nucleo::{Caminhos, Daemon, Relogio};

pub const SENHA: &str = "falso-mestra-ward-01";
pub const PIN: &str = "135790";
pub const PIN_ERRADO: &str = "246801";
/// Segredos das entradas de teste (falsos).
pub const SEGREDOS: [&str; 4] = [
    "falso-clientex-01",
    "falso-servidor-ward-07",
    "falso-banco-ward-08",
    "falso-livre-ward-09",
];

/// Base64 padrão (só para montar os quadros de teste).
pub fn b64(dados: &[u8]) -> String {
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

static N: AtomicU32 = AtomicU32::new(0);

/// Diretório 0700 só deste teste, em `target/tmp` (apagado no `Drop`).
pub struct Dir(pub PathBuf);

impl Dir {
    pub fn novo(rotulo: &str) -> Dir {
        let p = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "wardd-{}-{rotulo}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        Dir(p)
    }

    pub fn caminhos(&self) -> Caminhos {
        wardd::config::caminhos_de_teste(&self.0)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700));
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Relógio que só anota as esperas.
#[derive(Clone, Default)]
pub struct RelogioFalso(pub Arc<Mutex<Vec<Duration>>>);

impl Relogio for RelogioFalso {
    fn dormir(&self, d: Duration) {
        self.0.lock().unwrap().push(d);
    }
}

pub fn daemon_em(c: Caminhos) -> (Daemon, RelogioFalso) {
    let r = RelogioFalso::default();
    (Daemon::novo(c, Box::new(r.clone())), r)
}

/// Lê o pedido como o socket leria (o papel confere antes).
pub fn ped(papel: Papel, op: &str, dados: &str) -> Result<Pedido, ErroResposta> {
    protocolo::decodificar_pedido(
        papel,
        format!(r#"{{"v":1,"op":"{op}","dados":{dados}}}"#).as_bytes(),
    )
}

/// Pede e devolve a resposta (o pedido tem de passar pela leitura).
pub fn pedir(d: &Daemon, papel: Papel, op: &str, dados: &str) -> Resposta {
    match ped(papel, op, dados) {
        Ok(p) => d.atender(papel, p),
        Err(e) => Resposta::Erro(e),
    }
}

pub fn erro(r: &Resposta) -> Option<ErroResposta> {
    match r {
        Resposta::Erro(e) => Some(e.clone()),
        _ => None,
    }
}

pub fn ok(r: &Resposta) -> bool {
    matches!(r, Resposta::Ok)
}

pub fn destravar(d: &Daemon, senha: &str) -> Resposta {
    pedir(
        d,
        Papel::Agente,
        "destravar",
        &format!(r#"{{"senha":"{}"}}"#, b64(senha.as_bytes())),
    )
}

pub fn login(nome: &str, senha: &str) -> String {
    format!(
        r#"{{"entrada":{{"nome":"{nome}","grupo":"clientex","ambiente":"dev","tipo":"login","campos":{{"url":"https://clientex.invalid/login","usuario":"admin","senha":"{}"}}}}}}"#,
        b64(senha.as_bytes())
    )
}

pub fn servidor(nome: &str) -> String {
    format!(
        r#"{{"entrada":{{"nome":"{nome}","grupo":"clientex","ambiente":"stage","tipo":"servidor","campos":{{"host":"ssh.clientex.invalid","porta":"22","usuario":"deploy","senha":"{}"}}}}}}"#,
        b64(SEGREDOS[1].as_bytes())
    )
}

pub fn banco(nome: &str, tunel: &str) -> String {
    format!(
        r#"{{"entrada":{{"nome":"{nome}","grupo":"clientex","ambiente":"prod","tipo":"banco","campos":{{"motor":"postgres","host":"127.0.0.1","porta":"5432","base":"app","usuario":"app","senha":"{}","tunel":"{tunel}"}}}}}}"#,
        b64(SEGREDOS[2].as_bytes())
    )
}

pub fn mudar_nivel(d: &Daemon, papel: Papel, nome: &str, nivel: &str, pin: &str) -> Resposta {
    pedir(
        d,
        papel,
        "mudar_nivel",
        &format!(
            r#"{{"entrada":"{nome}","nivel":"{nivel}","pin":"{}"}}"#,
            b64(pin.as_bytes())
        ),
    )
}

/// Cofre de teste novo, destravado, com: `t-aprov` (com aprovação), `t-livre` (livre),
/// `t-bloq` (bloqueado), o servidor `s1` e o banco `b1` com túnel por `s1`.
pub fn cofre_de_teste(dir: &Dir) -> (Daemon, RelogioFalso) {
    let (d, r) = daemon_em(dir.caminhos());
    let criar = pedir(
        &d,
        Papel::App,
        "criar_cofre",
        &format!(
            r#"{{"senha":"{}","pin":"{}"}}"#,
            b64(SENHA.as_bytes()),
            b64(PIN.as_bytes())
        ),
    );
    assert!(ok(&criar), "{criar:?}");
    for q in [
        login("t-aprov", SEGREDOS[0]),
        login("t-livre", SEGREDOS[3]),
        login("t-bloq", SEGREDOS[0]),
        servidor("s1"),
        banco("b1", "s1"),
    ] {
        let r = pedir(&d, Papel::App, "salvar_entrada", &q);
        assert!(ok(&r), "{r:?}");
    }
    assert!(ok(&mudar_nivel(&d, Papel::App, "t-livre", "livre", PIN)));
    assert!(ok(&mudar_nivel(&d, Papel::App, "t-bloq", "bloqueado", PIN)));
    (d, r)
}

pub fn codificado(papel: Papel, r: &Resposta) -> Vec<u8> {
    protocolo::codificar_resposta(papel, r).unwrap().to_vec()
}

pub fn contem(palheiro: &[u8], agulha: &[u8]) -> bool {
    !agulha.is_empty()
        && agulha.len() <= palheiro.len()
        && palheiro.windows(agulha.len()).any(|w| w == agulha)
}

pub fn geracoes(dir: &Dir) -> wardd::arquivos::Geracoes {
    wardd::arquivos::ler_estado(&dir.caminhos().estado).unwrap()
}

/// Geração lida do disco (abre com a senha de teste, sem piso).
pub fn geracao_no_disco(caminho: &Path, senha: &str) -> u64 {
    let bytes = fs::read(caminho).unwrap();
    let s = ward_core::formato::SenhaMestra::nova(senha.to_owned());
    ward_core::corpo::abrir_entradas_lida(&bytes, &s, 0, 0)
        .unwrap()
        .2
}

/// Fase 44c (D9): janela de aprovação falsa, em processo (D8: o mesmo caminho do
/// socket, `responder_aprovacao` com o papel app). Durante `f`, cada
/// `pedido_de_aprovacao` que o motor entrega é guardado e respondido com `decisao`
/// (`confirmar`, `negar`…; com `pin` quando a decisão pede). Devolve o resultado de `f`
/// e os pedidos vistos. O motor do daemon não abre app nenhum: com a janela assinada,
/// ele entrega direto.
pub fn com_janela<R>(
    d: &Daemon,
    decisao: &str,
    pin: Option<&str>,
    f: impl FnOnce() -> R,
) -> (R, Vec<protocolo::PedidoDeAprovacao>) {
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;
    use wardd::aprovacao::Assinante;

    struct JanelaFalsa(mpsc::Sender<protocolo::PedidoDeAprovacao>);
    impl Assinante for JanelaFalsa {
        fn entregar(&mut self, r: &Resposta) -> bool {
            if let Resposta::PedidoDeAprovacao(p) = r {
                return self.0.send(p.clone()).is_ok();
            }
            true
        }
    }

    const ID_JANELA: u64 = 9_999;
    let (tx, rx) = mpsc::channel();
    d.motor().assinar(ID_JANELA, Box::new(JanelaFalsa(tx)));
    let parar = AtomicBool::new(false);
    let vistos = Mutex::new(Vec::new());
    let r = std::thread::scope(|s| {
        let (parar, vistos) = (&parar, &vistos);
        s.spawn(move || {
            while !parar.load(Ordering::Acquire) {
                let Ok(p) = rx.recv_timeout(Duration::from_millis(20)) else {
                    continue;
                };
                let pin = pin
                    .map(|p| format!(r#","pin":"{}""#, b64(p.as_bytes())))
                    .unwrap_or_default();
                let _ = pedir(
                    d,
                    Papel::App,
                    "responder_aprovacao",
                    &format!(r#"{{"pedido":{},"decisao":"{decisao}"{pin}}}"#, p.id),
                );
                vistos.lock().unwrap().push(p);
            }
        });
        let r = f();
        parar.store(true, Ordering::Release);
        r
    });
    d.motor().desassinar(ID_JANELA);
    (r, vistos.into_inner().unwrap())
}
