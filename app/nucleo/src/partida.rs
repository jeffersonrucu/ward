//! Partida do app setgid (D47, D57; S1, S8). O `main` do Tauri chama [`iniciar`] antes
//! de tudo, antes do GTK. A ordem:
//! 1. `PR_SET_DUMPABLE=0` e core 0 (o `exec` setgid já deixou o processo não-dumpable
//!    para o usuário; com `fs.suid_dumpable=2` o valor é 2, e aqui vira 0);
//! 2. sem depurador (`TracerPid` 0);
//! 3. o ambiente recebido é exatamente o de D57 (o que o lançador do daemon monta),
//!    senão o app sai sem conectar ([`ward_plataforma::conferir_ambiente_do_app`]);
//! 4. guarda o pai (pidfd), para saber depois se ele continua sendo o pai;
//! 5. conecta ao socket, com a conferência do servidor (D46), ainda com o egid do grupo
//!    `ward-app`: o daemon dá o papel por ele; o socket tem `CLOEXEC`;
//! 6. larga o grupo (`setresgid` para o gid real; o GTK 3 se recusa a iniciar com
//!    setgid) e chama `PR_SET_DUMPABLE=0` de novo (trocar de gid volta o dumpable ao
//!    `fs.suid_dumpable`);
//! 7. confere de novo gids, dumpable e depurador, e que o pai parece o daemon (`wardd`
//!    do mesmo uid, não-dumpable). Com outro pai, o app sai com mensagem: quem decide o
//!    papel é o daemon, e com outro pai ele já deu o papel agente (o pedido
//!    `abrir_app` ao daemon entra na Fase 23).
//!
//! Depois, [`Partida::vigiar`] espera a conexão cair (sem ler nada). Se ela cair e o
//! pai ainda for o mesmo daemon, o app se executa de novo (`/proc/self/exe`, para o
//! kernel aplicar o setgid outra vez e o daemon dar o papel de novo); com outro pai,
//! sai com mensagem.

use std::ffi::OsString;
use std::fmt;
use std::os::unix::net::UnixStream;

use ward_plataforma::app::{self, Pai, PaiDiferente};
use ward_plataforma::{AmbienteApp, ErroSessao, conferir_ambiente_do_app, endurecer_processo};

use crate::ErroApp;
use crate::daemon::DaemonSocket;

/// Por que o app não abriu. Sem valor de variável nem caminho.
#[derive(Debug)]
pub enum ErroPartida {
    /// `PR_SET_DUMPABLE`/`RLIMIT_CORE` não ficaram em 0.
    Endurecer,
    /// Sob depurador.
    Depurador,
    /// Ambiente fora de D57.
    Ambiente(ErroSessao),
    /// O pai não é (ou não parece) o daemon.
    Pai(PaiDiferente),
    /// O daemon não aceitou a conexão ou não é confiável (D46).
    Daemon(ErroApp),
    /// O socket não ficou com `CLOEXEC`.
    SemCloexec,
    /// `setresgid` não deixou os três gids no real.
    Grupo,
}

impl fmt::Display for ErroPartida {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("o app do Ward não abriu: ")?;
        match self {
            ErroPartida::Endurecer => f.write_str("não deu para endurecer o processo"),
            ErroPartida::Depurador => f.write_str("processo sob depurador"),
            ErroPartida::Ambiente(e) => write!(
                f,
                "{e}; abra o Ward pelo daemon (o ambiente do app é montado por ele)"
            ),
            ErroPartida::Pai(e) => write!(f, "{e}; abra o Ward pelo menu ou pela bandeja"),
            ErroPartida::Daemon(e) => write!(f, "{e}"),
            ErroPartida::SemCloexec => f.write_str("o socket do daemon ficou herdável"),
            ErroPartida::Grupo => f.write_str("não deu para largar o grupo do app"),
        }
    }
}

impl std::error::Error for ErroPartida {}

/// O app depois da partida: a conexão (dentro do [`DaemonSocket`]), o ambiente
/// conferido e o pai do começo.
pub struct Partida {
    ambiente: AmbienteApp,
    pai: Pai,
    daemon: Option<DaemonSocket>,
    vigia: UnixStream,
    argumentos: Vec<OsString>,
}

/// A partida inteira com o ambiente e o daemon padrão deste processo.
pub fn iniciar() -> Result<Partida, ErroPartida> {
    iniciar_com(std::env::vars_os(), DaemonSocket::padrao)
}

fn tracer_zero() -> bool {
    std::fs::read("/proc/self/status").ok().and_then(|s| {
        s.split(|&b| b == b'\n')
            .find_map(|l| l.strip_prefix(b"TracerPid:"))
            .and_then(|v| std::str::from_utf8(v).ok())
            .and_then(|v| v.trim().parse::<u64>().ok())
    }) == Some(0)
}

fn iniciar_com<I, F>(ambiente: I, daemon: F) -> Result<Partida, ErroPartida>
where
    I: IntoIterator<Item = (OsString, OsString)>,
    F: FnOnce() -> Result<DaemonSocket, ErroApp>,
{
    endurecer_processo().map_err(|_| ErroPartida::Endurecer)?;
    if !tracer_zero() {
        return Err(ErroPartida::Depurador);
    }
    let ambiente = conferir_ambiente_do_app(ambiente).map_err(ErroPartida::Ambiente)?;
    let pai = Pai::atual().map_err(ErroPartida::Pai)?;

    let mut daemon = daemon().map_err(ErroPartida::Daemon)?;
    let conexao = daemon.conectar_agora().map_err(ErroPartida::Daemon)?;
    if !app::tem_cloexec(conexao).unwrap_or(false) {
        return Err(ErroPartida::SemCloexec);
    }
    let vigia = conexao
        .try_clone()
        .map_err(|_| ErroPartida::Daemon(ErroApp::DaemonIndisponivel))?;

    app::largar_grupo().map_err(|_| ErroPartida::Grupo)?;
    endurecer_processo().map_err(|_| ErroPartida::Endurecer)?;
    match app::gids() {
        Ok((r, e, s)) if r == e && e == s => {}
        _ => return Err(ErroPartida::Grupo),
    }
    if app::dumpable().unwrap_or(true) {
        return Err(ErroPartida::Endurecer);
    }
    if !tracer_zero() {
        return Err(ErroPartida::Depurador);
    }
    pai.conferir_parece_o_daemon().map_err(ErroPartida::Pai)?;
    if !app::tem_cloexec(&vigia).unwrap_or(false) {
        return Err(ErroPartida::SemCloexec);
    }
    Ok(Partida {
        ambiente,
        pai,
        daemon: Some(daemon),
        vigia,
        argumentos: std::env::args_os().skip(1).collect(),
    })
}

impl Partida {
    /// O cliente do daemon com a conexão da partida. Só uma vez.
    pub fn tomar_daemon(&mut self) -> Option<DaemonSocket> {
        self.daemon.take()
    }

    /// pid do pai (o daemon).
    pub fn pid_do_pai(&self) -> u32 {
        self.pai.pid()
    }

    /// Numa thread: espera a conexão cair, chama `ao_cair` (limpar a área de
    /// transferência) e depois [`reexecutar_ou_sair`](Self::reexecutar_ou_sair).
    pub fn vigiar(self, ao_cair: impl FnOnce() + Send + 'static) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            let _ = app::esperar_fechamento(&self.vigia);
            ao_cair();
            self.reexecutar_ou_sair()
        })
    }

    /// Com o pai ainda igual ao daemon do começo, executa o app de novo com o mesmo
    /// ambiente conferido; senão, sai com mensagem. Não volta.
    pub fn reexecutar_ou_sair(&self) -> ! {
        if self.pai.continua_pai() {
            let e = app::reexecutar(&self.argumentos, &self.ambiente);
            eprintln!("ward-app: a conexão com o daemon caiu e o app não se reabriu ({e})");
        } else {
            eprintln!("ward-app: a conexão com o daemon caiu e o pai não é mais o daemon");
        }
        std::process::exit(1)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;

    #[test]
    fn ataque_ambiente_de_fora_nao_chega_a_conectar() {
        // Um processo qualquer abre o app com o próprio ambiente (ou com um proxy em
        // `WAYLAND_DISPLAY`): o app sai antes de conectar, sem usar o papel.
        let uid = ward_plataforma::uid_efetivo();
        let dir = PathBuf::from(format!(
            "/run/user/{uid}/ward-partida-teste-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let caminho = dir.join("wardd.sock");
        let l = UnixListener::bind(&caminho).unwrap();
        l.set_nonblocking(true).unwrap();
        for ambiente in [
            vec![("WAYLAND_DISPLAY", "/tmp/proxy.sock")],
            vec![("GTK_MODULES", "/tmp/x.so"), ("PATH", "/usr/bin")],
            vec![],
        ] {
            let c = caminho.clone();
            let r = iniciar_com(
                ambiente
                    .into_iter()
                    .map(|(k, v)| (OsString::from(k), OsString::from(v))),
                move || Ok(DaemonSocket::sistema_em(c)),
            );
            assert!(matches!(r, Err(ErroPartida::Ambiente(_))), "{:?}", r.err());
        }
        assert!(l.accept().is_err(), "o app conectou com o ambiente errado");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mensagem_sem_valor() {
        let m = ErroPartida::Ambiente(ErroSessao::Diferente("WAYLAND_DISPLAY")).to_string();
        assert!(m.contains("WAYLAND_DISPLAY") && !m.contains("/tmp"));
        let m = ErroPartida::Pai(PaiDiferente::NaoEoDaemon).to_string();
        assert!(m.starts_with("o app do Ward não abriu"));
    }
}
