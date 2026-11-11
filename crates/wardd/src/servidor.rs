//! Laço do socket: aceita, confere o uid do par (`SO_PEERCRED`, S5), dá o papel pelo
//! grupo (D47), lê quadros com o limite do papel e responde.
//!
//! - Uid diferente, ou sem identidade do processo: fecha sem mandar nenhum byte e
//!   registra `UidRecusado` com uid e pid (V3.2).
//! - No máximo [`MAX_CONEXOES`] conexões de agente e extensão ao mesmo tempo, uma
//!   thread cada. Cheio: a conexão ociosa (esperando quadro) mais antiga entre elas é
//!   derrubada para dar lugar à nova; sem nenhuma ociosa, a nova recebe `ocupado` e
//!   fecha. Assim 10 000 conexões caladas não impedem um cliente normal de ser atendido
//!   (V3.4).
//! - O papel app tem vagas à parte ([`MAX_CONEXOES_APP`], scrutiny do M3): a conexão
//!   dele, que espera avisos calada (assinante), nunca é despejada por conexões de
//!   outro papel, e um agente com as 64 vagas ocupadas não tira a vaga do app. Só o
//!   processo que o lançador abriu é app (D63), então ninguém mais enche essas vagas.
//! - Cada leitura tem prazo ([`PRAZO_LEITURA`]); quadro recusado pela leitura recebe
//!   `quadro_invalido` e a conexão fecha.
//! - Antes de cada pedido de app ou extensão, o daemon reconfere que o cliente segue
//!   vivo e sem depurador; se não, fecha.
//! - Papel app (D63): além do grupo e do pai (D47), o par tem de ser o processo que o
//!   lançador abriu, vivo ([`papel_da_classe`], o único ponto da classificação).
//! - Conexão do papel app (D65 a): recebe os avisos do motor (`pedido_de_aprovacao`,
//!   `pedido_de_destrave`), cada um num quadro inteiro, nunca no meio de uma resposta
//!   (uma trava de escrita por conexão); as respostas seguem a ordem dos pedidos. Fica
//!   sem prazo de leitura; ao fechar, o que ela recebeu e não respondeu é negado. Só a
//!   conexão do papel app recebe avisos.

use std::io;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ward_core::protocol::{
    self, Request, ResponseError, Resposta, Role,
    frame::{self, FrameError},
};
use ward_platform::{AcceptError, Install, KernelPolicy, Listener, PeerClass, PeerProcess};

use crate::aprovacao::{Assinante, Cliente};
use crate::lancamento::{AppDeTeste, AppLancado, ExtensaoDeTeste};
use crate::nucleo::Daemon;
use crate::{aviso, depurar};

/// Conexões de agente e extensão atendidas ao mesmo tempo.
pub const MAX_CONEXOES: usize = 64;
/// Conexões do papel app ao mesmo tempo, à parte das [`MAX_CONEXOES`].
pub const MAX_CONEXOES_APP: usize = 8;
/// Prazo de cada leitura do socket.
pub const PRAZO_LEITURA: Duration = Duration::from_secs(60);
/// Prazo de cada escrita.
pub const PRAZO_ESCRITA: Duration = Duration::from_secs(10);

struct Vaga {
    id: u64,
    /// Vaga do papel app: fora da conta e do despejo das outras.
    app: bool,
    stream: UnixStream,
    ociosa: Arc<AtomicBool>,
    desde: Instant,
}

#[derive(Default)]
struct Vagas {
    lista: Mutex<Vec<Vaga>>,
    proximo: AtomicU64,
}

impl Vagas {
    /// Reserva uma vaga no grupo do papel (app ou o resto). Cheio: no grupo do resto,
    /// derruba a ociosa mais antiga dele; no do app, recusa. Uma conexão nunca derruba
    /// a de outro grupo.
    fn reservar(&self, stream: &UnixStream, papel: Role) -> Option<(u64, Arc<AtomicBool>)> {
        let app = papel == Role::App;
        let mut lista = self.lista.lock().unwrap_or_else(|e| e.into_inner());
        let limite = if app { MAX_CONEXOES_APP } else { MAX_CONEXOES };
        if lista.iter().filter(|v| v.app == app).count() >= limite {
            if app {
                return None;
            }
            let velha = lista
                .iter()
                .enumerate()
                .filter(|(_, v)| !v.app && v.ociosa.load(Ordering::Acquire))
                .min_by_key(|(_, v)| v.desde)
                .map(|(i, _)| i)?;
            let v = lista.swap_remove(velha);
            let _ = v.stream.shutdown(Shutdown::Both);
        }
        let copia = stream.try_clone().ok()?;
        let id = self.proximo.fetch_add(1, Ordering::Relaxed);
        let ociosa = Arc::new(AtomicBool::new(true));
        lista.push(Vaga {
            id,
            app,
            stream: copia,
            ociosa: ociosa.clone(),
            desde: Instant::now(),
        });
        Some((id, ociosa))
    }

    fn marcar(&self, id: u64, ociosa: bool) {
        let mut lista = self.lista.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(v) = lista.iter_mut().find(|v| v.id == id) {
            v.ociosa.store(ociosa, Ordering::Release);
            v.desde = Instant::now();
        }
    }

    fn soltar(&self, id: u64) {
        let mut lista = self.lista.lock().unwrap_or_else(|e| e.into_inner());
        lista.retain(|v| v.id != id);
    }
}

struct Soltar(Arc<Vagas>, u64);

impl Drop for Soltar {
    fn drop(&mut self) {
        self.0.soltar(self.1);
    }
}

/// Como o daemon decide o papel de cada conexão.
pub struct Papeis {
    /// `None`: grupos da instalação ausentes ou recusados; todo cliente é agente.
    pub instalacao: Option<Install>,
    /// O app que o lançador abriu (D63). `None`: ninguém é app.
    pub app: Option<AppLancado>,
    /// Só no daemon de teste (D63): o app de teste, app sem o egid. No release é sempre
    /// `None` (o tipo não se constrói sem a feature).
    pub app_de_teste: Option<AppDeTeste>,
    /// Só no daemon de teste com `WARD_DEV_EXTENSAO=chrome` (Fase 41): o host de teste
    /// da extensão, extensão sem o egid. No release é sempre `None` (o tipo não se
    /// constrói sem a feature).
    pub extensao_de_teste: Option<ExtensaoDeTeste>,
}

/// O papel de um par já classificado pelo grupo e pelo pai (D47). Papel app só para o
/// processo `pid` que o lançador abriu e que continua vivo (D63): outro filho do daemon
/// com o grupo do app (um `ward run` do `ward-app`, Fase 26) fica agente.
pub fn papel_da_classe(classe: PeerClass, pid: u32, e_o_app: impl Fn(u32) -> bool) -> Role {
    match classe {
        PeerClass::App if e_o_app(pid) => Role::App,
        PeerClass::App => {
            aviso!("papel app recusado (pid {pid}): não é o app aberto pelo lançador (D63)");
            Role::Agent
        }
        PeerClass::ExtensionHost => Role::Extension,
        PeerClass::Common { .. } => Role::Agent,
    }
}

impl Papeis {
    fn papel(&self, par: &PeerProcess) -> Role {
        let pid = par.credentials().pid;
        // D63, daemon de teste: o pid do lançador e o pai = daemon, sem o egid. O papel
        // sai do mesmo `papel_da_classe`.
        if let Some(t) = &self.app_de_teste
            && t.e_o_app(pid)
        {
            match t.classificar(par) {
                c @ PeerClass::App => return papel_da_classe(c, pid, |p| t.e_o_app(p)),
                PeerClass::Common { reason: motivo } => {
                    aviso!("papel app de teste recusado (pid {pid}): {motivo}");
                }
                PeerClass::ExtensionHost => {}
            }
        }
        // Fase 41, daemon de teste: pai = Chrome instalado pelo `/proc`, sem o egid. Quem
        // não passa segue o caminho do release (sem os grupos, agente).
        if let Some(x) = &self.extensao_de_teste
            && let c @ PeerClass::ExtensionHost = x.classificar(par)
        {
            aviso!("papel extensão de teste (pid {pid}): pai é o Chrome instalado");
            return papel_da_classe(c, pid, |_| false);
        }
        let Some(inst) = &self.instalacao else {
            return Role::Agent;
        };
        match par.classify(inst, &KernelPolicy::read()) {
            c @ (PeerClass::App | PeerClass::ExtensionHost) => {
                papel_da_classe(c, pid, |p| self.app.as_ref().is_some_and(|a| a.e_o_app(p)))
            }
            PeerClass::Common { reason: motivo } => {
                // Só registra quando o cliente tentou um grupo especial.
                let cred = par.credentials();
                if cred.gid == inst.app_gid() || cred.gid == inst.extension_gid() {
                    aviso!("papel especial recusado (pid {}): {motivo}", cred.pid);
                }
                Role::Agent
            }
        }
    }
}

/// Escreve uma resposta para o papel. Uma resposta que o papel não pode receber vira
/// erro interno (nunca sai credencial para o agente, S1).
fn responder(stream: &mut UnixStream, papel: Role, r: &Resposta) -> Result<(), FrameError> {
    let q = match protocol::encode_response(papel, r) {
        Ok(q) => q,
        Err(_) => {
            aviso!("resposta recusada para o papel {}", papel.name());
            protocol::encode_response(papel, &Resposta::Erro(ResponseError::Internal))
                .map_err(|_| FrameError::Io(io::ErrorKind::InvalidData))?
        }
    };
    frame::write_frame(stream, &q, frame::RESPONSE_LIMIT)
}

/// A conexão do app que assinou: o motor escreve nela com a mesma trava das respostas.
struct AssinanteSocket {
    stream: UnixStream,
    escrita: Arc<Mutex<()>>,
}

impl Assinante for AssinanteSocket {
    fn entregar(&mut self, r: &Resposta) -> bool {
        let _g = self.escrita.lock().unwrap_or_else(|e| e.into_inner());
        let ok = responder(&mut self.stream, Role::App, r).is_ok();
        if !ok {
            let _ = self.stream.shutdown(Shutdown::Both);
        }
        ok
    }
}

/// O cliente da conexão, para o motor: cadeia de processos e se ele caiu.
struct ClienteSocket<'a> {
    stream: &'a UnixStream,
    par: &'a PeerProcess,
}

impl Cliente for ClienteSocket<'_> {
    fn cadeia(&self) -> Vec<String> {
        self.par.chain().into_iter().map(|e| e.name).collect()
    }

    /// Olha sem ler nem esperar (`poll`): fim de arquivo ou erro é cliente que caiu.
    fn saiu(&self) -> bool {
        ward_platform::peer_closed(self.stream)
    }
}

fn atender_conexao(
    mut stream: UnixStream,
    par: PeerProcess,
    papel: Role,
    daemon: &Daemon,
    vagas: &Vagas,
    id: u64,
) {
    let _ = stream.set_read_timeout(Some(PRAZO_LEITURA));
    let _ = stream.set_write_timeout(Some(PRAZO_ESCRITA));
    let limite = frame::frame_limit(papel);
    let escrita = Arc::new(Mutex::new(()));
    // D65 (a): só a conexão do papel app recebe os avisos do motor, e fica sem prazo de
    // leitura (espera aberta).
    let mut assinou = false;
    if papel == Role::App
        && let Ok(copia) = stream.try_clone()
    {
        let _ = stream.set_read_timeout(None);
        daemon.motor().assinar(
            id,
            Box::new(AssinanteSocket {
                stream: copia,
                escrita: escrita.clone(),
            }),
        );
        assinou = true;
    }
    loop {
        vagas.marcar(id, true);
        let bytes = match frame::read_frame(&mut stream, limite) {
            Ok(b) => b,
            Err(FrameError::Closed) | Err(FrameError::Io(_)) | Err(FrameError::Truncated) => {
                break;
            }
            Err(e) => {
                depurar!("quadro recusado ({}): {e}", papel.name());
                let _ = responder(
                    &mut stream,
                    papel,
                    &Resposta::Erro(protocol::frame_error(e)),
                );
                break;
            }
        };
        vagas.marcar(id, false);
        if papel != Role::Agent && par.reverify_no_debugger().is_err() {
            aviso!(
                "cliente {} sob depurador ou encerrado: conexão fechada",
                papel.name()
            );
            break;
        }
        let resposta = match protocol::decode_request(papel, &bytes) {
            Ok(p) => {
                drop(bytes);
                let cliente = ClienteSocket {
                    stream: &stream,
                    par: &par,
                };
                // `ward run` (Fase 26): a sessão escreve as respostas; depois de um
                // comando lançado, a conexão acaba. Recusado antes, ela segue.
                if papel == Role::Agent && matches!(p, Request::Run(_)) {
                    if crate::execution::session(daemon, &cliente, &stream, p) {
                        continue;
                    }
                    break;
                }
                // `ward ssh` (Fase 27): a mesma forma de sessão.
                if papel == Role::Agent && matches!(p, Request::Ssh { .. }) {
                    if crate::ssh::session(daemon, &cliente, &stream, p) {
                        continue;
                    }
                    break;
                }
                daemon.atender_de(&cliente, papel, p)
            }
            Err(e) => {
                depurar!("pedido recusado ({}): {}", papel.name(), e.code());
                Resposta::Erro(e)
            }
        };
        let _g = escrita.lock().unwrap_or_else(|e| e.into_inner());
        if responder(&mut stream, papel, &resposta).is_err() {
            break;
        }
    }
    if assinou {
        // Fechar a janela (ou o app) nega o que ela recebeu (S10).
        daemon.motor().desassinar(id);
    }
}

/// Aceita conexões para sempre.
pub fn servir(escuta: &Listener, daemon: Arc<Daemon>, papeis: Arc<Papeis>) -> io::Result<()> {
    let vagas = Arc::new(Vagas::default());
    loop {
        let stream = match escuta.listener().accept() {
            Ok((s, _)) => s,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                aviso!("erro ao aceitar conexão ({})", e.kind());
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
        };
        let par = match ward_platform::verify_connection(&stream) {
            Ok(p) => p,
            Err(AcceptError::UidRefused { uid, pid }) => {
                // Fecha sem mandar nenhum byte (V3.2).
                aviso!("conexão recusada (UidRecusado): uid {uid}, pid {pid}");
                drop(stream);
                continue;
            }
            Err(AcceptError::NoIdentity(e)) => {
                aviso!("conexão recusada (SemIdentidade): {}", e.kind());
                drop(stream);
                continue;
            }
            Err(AcceptError::Io(e)) => {
                aviso!("conexão recusada (Io): {}", e.kind());
                drop(stream);
                continue;
            }
        };
        let papel = papeis.papel(&par);
        let Some((id, _)) = vagas.reservar(&stream, papel) else {
            let mut s = stream;
            let _ = s.set_write_timeout(Some(Duration::from_secs(1)));
            let _ = responder(&mut s, papel, &Resposta::Erro(ResponseError::Busy));
            continue;
        };
        let daemon = daemon.clone();
        let vagas_t = vagas.clone();
        let r = std::thread::Builder::new()
            .name("wardd-conexao".into())
            .spawn(move || {
                // Solta a vaga mesmo se a thread cair.
                let _soltar = Soltar(vagas_t.clone(), id);
                atender_conexao(stream, par, papel, &daemon, &vagas_t, id);
            });
        if r.is_err() {
            aviso!("não deu para criar a thread da conexão");
            vagas.soltar(id);
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use ward_core::entry::Environment;
    use ward_core::protocol::{Operation, PedidoDeAprovacao, PedidoDeDestrave};

    /// D63: sem o grupo, o pid do lançador não basta; com o grupo, só o pid do lançador.
    /// Vale nos dois builds (o `make test-no-test-mode` roda sem a feature).
    #[test]
    fn d63_sem_o_grupo_o_pid_do_lancador_nao_da_papel_app() {
        use ward_platform::Refusal;
        for motivo in [
            Refusal::CommonGroup,
            Refusal::AppParentNotAllowed,
            Refusal::Debugger,
        ] {
            assert_eq!(
                papel_da_classe(PeerClass::Common { reason: motivo }, 42, |_| true),
                Role::Agent
            );
        }
        assert_eq!(papel_da_classe(PeerClass::App, 42, |_| false), Role::Agent);
        assert_eq!(papel_da_classe(PeerClass::App, 42, |p| p == 42), Role::App);
    }

    /// D63, pelo socket: um filho deste processo (como o lançador o abriria), sem o
    /// egid do grupo `ward-app` e sem instalação, fica agente. Sem a feature
    /// `test-mode`, o `app_de_teste` nem tem valor possível; com ela, sem `WARD_DEV_APP`
    /// é `None` e dá o mesmo.
    #[test]
    fn d63_par_filho_sem_egid_fica_agente() {
        use std::os::unix::fs::DirBuilderExt;
        let dir = std::path::PathBuf::from(format!(
            "/run/user/{}/wardd-servidor-d63-{}",
            ward_platform::effective_uid(),
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let caminho = dir.join("s.sock");
        let l = std::os::unix::net::UnixListener::bind(&caminho).unwrap();
        let mut filho = std::process::Command::new("/usr/bin/python3")
            .args([
                "-I",
                "-c",
                "import socket,sys,time\ns=socket.socket(socket.AF_UNIX)\ns.connect(sys.argv[1])\ntime.sleep(10)",
            ])
            .arg(&caminho)
            .env_clear()
            .spawn()
            .unwrap();
        let (s, _) = l.accept().unwrap();
        let par = ward_platform::verify_connection(&s).unwrap();
        assert_eq!(par.credentials().pid, filho.id());
        let papeis = Papeis {
            instalacao: None,
            app: None,
            app_de_teste: None,
            extensao_de_teste: None,
        };
        assert_eq!(papeis.papel(&par), Role::Agent);
        // Fase 41, só no daemon de teste: com a exceção da extensão ligada e o Chrome
        // instalado de pai aceito, este par (filho do teste, não do Chrome) continua
        // agente. Sem a feature, a exceção nem se constrói.
        let extensao = ExtensaoDeTeste::do_chrome("/opt/google/chrome/chrome");
        if !cfg!(feature = "test-mode") {
            assert!(extensao.is_none());
        }
        let papeis = Papeis {
            instalacao: None,
            app: None,
            app_de_teste: None,
            extensao_de_teste: extensao,
        };
        assert_eq!(papeis.papel(&par), Role::Agent);
        let _ = filho.kill();
        let _ = filho.wait();
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn vivo(s: &UnixStream) -> bool {
        !ward_platform::peer_closed(s)
    }

    /// Scrutiny do M3: a conexão do app (assinante, ociosa) não é despejada por 10 000
    /// conexões caladas de agente; o agente cheio não tira a vaga do app; e o app cheio
    /// não despeja agente.
    #[test]
    fn ataque_conexoes_de_agente_nao_despejam_o_app() {
        let vagas = Vagas::default();
        // O app conecta primeiro e fica calado (esperando avisos).
        let (app_d, app_c) = UnixStream::pair().unwrap();
        let (id_app, _) = vagas.reservar(&app_d, Role::App).unwrap();
        // 10 000 agentes calados: os velhos são despejados entre eles.
        let mut agentes = Vec::new();
        for _ in 0..10_000 {
            let (d, c) = UnixStream::pair().unwrap();
            assert!(vagas.reservar(&d, Role::Agent).is_some());
            agentes.push(c);
            if agentes.len() > MAX_CONEXOES * 2 {
                agentes.remove(0);
            }
        }
        assert!(vivo(&app_c), "a conexão do app foi derrubada");
        {
            let lista = vagas.lista.lock().unwrap();
            assert!(lista.iter().any(|v| v.id == id_app && v.app));
            assert_eq!(lista.iter().filter(|v| !v.app).count(), MAX_CONEXOES);
        }
        // 64 agentes ocupados (nenhum ocioso): agente novo recebe `ocupado`, o app
        // ainda conecta.
        let ids: Vec<u64> = vagas
            .lista
            .lock()
            .unwrap()
            .iter()
            .filter(|v| !v.app)
            .map(|v| v.id)
            .collect();
        for id in &ids {
            vagas.marcar(*id, false);
        }
        let (d, _c) = UnixStream::pair().unwrap();
        assert!(vagas.reservar(&d, Role::Agent).is_none());
        let (d, _c2) = UnixStream::pair().unwrap();
        assert!(vagas.reservar(&d, Role::Extension).is_none());
        let mut apps = vec![(app_d, app_c)];
        for _ in 1..MAX_CONEXOES_APP {
            let (d, c) = UnixStream::pair().unwrap();
            assert!(vagas.reservar(&d, Role::App).is_some());
            apps.push((d, c));
        }
        // App cheio: recusa e não despeja ninguém (nem app, nem agente).
        let (d, _c3) = UnixStream::pair().unwrap();
        assert!(vagas.reservar(&d, Role::App).is_none());
        for (_, c) in &apps {
            assert!(vivo(c));
        }
        assert_eq!(
            vagas
                .lista
                .lock()
                .unwrap()
                .iter()
                .filter(|v| !v.app)
                .count(),
            MAX_CONEXOES
        );
        // Soltar uma vaga de app abre lugar para outra.
        vagas.soltar(id_app);
        let (d, _c4) = UnixStream::pair().unwrap();
        assert!(vagas.reservar(&d, Role::App).is_some());
    }

    /// O que o motor entrega ao app sai como um quadro de resposta do papel app, inteiro;
    /// com a conexão fechada, a entrega falha (o motor trata como janela fechada).
    #[test]
    fn assinante_escreve_quadro_inteiro_e_falha_fechado() {
        let (a, mut b) = UnixStream::pair().unwrap();
        let mut s = AssinanteSocket {
            stream: a,
            escrita: Arc::new(Mutex::new(())),
        };
        let p = PedidoDeAprovacao {
            id: 3,
            entradas: vec!["t-aprov".into()],
            ambiente: Environment::Prod,
            operacao: Operation::Run,
            comando: "\"true\"".into(),
            diretorio: "\"/\"".into(),
            cadeia: vec!["claude".into()],
            origem: String::new(),
            confirmacao: false,
            entrada_oculta: true,
        };
        assert!(s.entregar(&Resposta::PedidoDeAprovacao(p.clone())));
        let d = PedidoDeDestrave {
            entrada: "t-aprov".into(),
            operacao: Operation::Run,
            comando: "\"true\"".into(),
            restante: 120,
        };
        assert!(s.entregar(&Resposta::PedidoDeDestrave(d.clone())));
        let q = frame::read_frame(&mut b, frame::RESPONSE_LIMIT).unwrap();
        match protocol::decode_response(Role::App, &q).unwrap() {
            Resposta::PedidoDeAprovacao(lido) => assert_eq!(lido, p),
            r => panic!("{r:?}"),
        }
        let q = frame::read_frame(&mut b, frame::RESPONSE_LIMIT).unwrap();
        match protocol::decode_response(Role::App, &q).unwrap() {
            Resposta::PedidoDeDestrave(lido) => assert_eq!(lido, d),
            r => panic!("{r:?}"),
        }
        // O agente não lê esse quadro.
        assert!(protocol::decode_response(Role::Agent, &q).is_err());
        drop(b);
        // Outro teste em paralelo que faz `fork` segura uma cópia do fd até o `exec`
        // (CLOEXEC): a falha vem logo depois.
        let falhou = (0..200).any(|_| {
            let f = !s.entregar(&Resposta::PedidoDeDestrave(d.clone()));
            if !f {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            f
        });
        assert!(falhou);
    }
}
