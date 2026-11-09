//! Cliente do socket do `wardd` (lado Rust do app; a webview nunca fala com o socket).
//!
//! Antes de mandar qualquer byte, o app confere o servidor pela mesma regra da CLI
//! (`ward_plataforma::conectar_servidor`, S5, D46): o socket é
//! `/run/ward/<uid>/wardd.sock`, toda pasta do caminho é do root, sem escrita para grupo
//! e outros e sem link, o socket é do próprio uid e 0600, e o `SO_PEERCRED` do par tem
//! uid 0 (quem fez o `listen` foi o systemd do sistema). Só no build de teste
//! (`modo-teste`, D7) o socket é o do daemon de teste, em `/run/user/<uid>/ward-teste/`
//! (pasta 0700 do usuário e par do mesmo uid); sem a feature, esse caminho e essa
//! exceção nem existem no binário, e nenhuma variável de ambiente escolhe o caminho.
//!
//! O protocolo (Fase 20) fica em [`crate::cliente`]: lá o pedido vira o corpo do
//! quadro e a resposta é lida no papel app. Este módulo é só o transporte.
//!
//! Transporte (D65 a, Fase 23a): `escrever_quadro`/`ler_quadro` na conexão única.
//! - Uma thread leitora é a única que lê o socket. O que chega é lido no papel app
//!   (`decodificar_resposta(Papel::App, …)`) e separado:
//!   - `pedido_de_aprovacao` e `pedido_de_destrave` são **avisos** e vão para a
//!     [`FilaAvisos`] (no máximo [`MAX_AVISOS`]), nunca no lugar de uma resposta. Com a
//!     fila cheia, o aviso novo é descartado com uma linha de log sem conteúdo (o
//!     pedido do agente expira e nega no daemon, S10);
//!   - qualquer outro tipo é a resposta de quem pediu;
//!   - resposta sem pedido em curso, quadro acima de `LIMITE_RESPOSTA` ou quadro que
//!     não decodifica no papel app (inclusive um tipo do papel agente) **fecham a
//!     conexão**: o app não tenta adivinhar o que o daemon quis dizer.
//! - Um pedido por vez, sob mutex: o protocolo não tem id de pedido e o daemon
//!   responde na ordem. A resposta tem [`PRAZO_RESPOSTA`] (destravar com a espera de
//!   D20 mais o Argon2, D62); sem ela, a conexão fecha (uma resposta atrasada seria
//!   "resposta sem pedido").
//! - Os quadros ficam em `Zeroizing` (o `copiar` traz segredo) e o limite é o de
//!   `ward_core::limites`.
//!
//! Conexão única (D47, Fase 29a): o app setgid conecta **uma vez**, antes do GTK, com o
//! egid do grupo `ward-app` (o papel vem dele), e depois larga o grupo
//! ([`crate::partida`]). Uma conexão nova depois disso teria o gid real e cairia no
//! papel agente; por isso o [`DaemonSocket`] da partida (e o da ponte de teste) nunca
//! reconecta: sem a conexão, os pedidos respondem [`ErroApp::ConexaoPerdida`] e o app
//! se executa de novo. O [`DaemonSocket::padrao`] sem [`DaemonSocket::conectar_agora`]
//! (só os testes) conecta no primeiro uso e, se a conexão cair, no uso seguinte.

use std::collections::VecDeque;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use ward_core::protocolo::quadro::{self, ErroQuadro};
use ward_core::protocolo::{self, Papel, PedidoDeAprovacao, Resposta};
use ward_plataforma::{RecusaServidor, ServidorEsperado, conectar_servidor};

use zeroize::Zeroizing;

use crate::ErroApp;

/// Nome do socket (o mesmo do `wardd`).
pub const SOCKET: &str = "wardd.sock";

/// Prazo de cada escrita no socket.
const PRAZO_ESCRITA: Duration = Duration::from_secs(10);

/// Prazo da resposta de um pedido (D65 a): o destravar espera até 2 min pela trava de
/// D20 e ainda roda o Argon2 (D62).
pub const PRAZO_RESPOSTA: Duration = Duration::from_secs(130);

/// Maior número de avisos esperando na fila (D65 a).
pub const MAX_AVISOS: usize = 16;

/// Estado do cofre como a interface vê.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EstadoCofre {
    SemCofre,
    Travado,
    Destravado,
}

impl EstadoCofre {
    pub fn codigo(self) -> &'static str {
        match self {
            EstadoCofre::SemCofre => "sem-cofre",
            EstadoCofre::Travado => "travado",
            EstadoCofre::Destravado => "destravado",
        }
    }
}

/// Transporte até o daemon. Recebe o corpo de um pedido já codificado no papel app e
/// devolve o corpo da resposta, ainda não lido (quem lê é [`crate::cliente`], com a
/// leitura estrita do protocolo). Os testes e a ponte de teste trocam o socket pelo
/// [`crate::daemon_falso`].
pub trait Daemon: Send {
    fn trocar(&mut self, pedido: &[u8]) -> Result<Zeroizing<Vec<u8>>, ErroApp>;
}

/// O que o daemon manda sem pedido, só na conexão do papel app (D65 a). Nenhum dos dois
/// tem segredo: o texto do pedido (`comando`, `cadeia`) é exibido pela Fase 36 com D51.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Aviso {
    /// Um uso do agente espera aprovação (Fase 36).
    Aprovacao(PedidoDeAprovacao),
    /// Um uso do agente espera o destrave (Fase 31).
    Destrave,
}

#[derive(Default)]
struct Fila {
    itens: VecDeque<Aviso>,
    descartados: u64,
}

/// Fila limitada dos avisos (D65 a). A thread leitora põe; as telas de destrave (Fase
/// 31) e de aprovação (Fase 36) tiram.
#[derive(Default)]
pub struct FilaAvisos {
    fila: Mutex<Fila>,
    chegou: Condvar,
}

impl FilaAvisos {
    fn travar(&self) -> MutexGuard<'_, Fila> {
        self.fila.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Põe no fim. Cheia: descarta o aviso novo, com log sem conteúdo.
    fn por(&self, aviso: Aviso) {
        let mut f = self.travar();
        if f.itens.len() >= MAX_AVISOS {
            f.descartados = f.descartados.saturating_add(1);
            drop(f);
            drop(aviso);
            eprintln!("ward-app: aviso do daemon descartado (fila cheia)");
            return;
        }
        f.itens.push_back(aviso);
        drop(f);
        self.chegou.notify_all();
    }

    /// O aviso mais antigo, sem esperar.
    pub fn tomar(&self) -> Option<Aviso> {
        self.travar().itens.pop_front()
    }

    /// O aviso mais antigo, esperando até `prazo`.
    pub fn esperar(&self, prazo: Duration) -> Option<Aviso> {
        let fim = Instant::now() + prazo;
        let mut f = self.travar();
        loop {
            if let Some(a) = f.itens.pop_front() {
                return Some(a);
            }
            let falta = fim.checked_duration_since(Instant::now())?;
            f = self
                .chegou
                .wait_timeout(f, falta)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }

    /// Avisos esperando.
    pub fn tamanho(&self) -> usize {
        self.travar().itens.len()
    }

    /// Avisos descartados com a fila cheia, desde o começo.
    pub fn descartados(&self) -> u64 {
        self.travar().descartados
    }
}

/// Lado de quem pede: a escrita e o canal por onde a thread leitora entrega a resposta.
struct Pedinte {
    escrita: UnixStream,
    respostas: Receiver<Zeroizing<Vec<u8>>>,
}

/// Uma conexão aberta com a sua thread leitora.
struct Transporte {
    /// A conexão (a partida confere o `CLOEXEC` e vigia o fechamento nela).
    stream: UnixStream,
    /// Um pedido por vez.
    pedinte: Mutex<Pedinte>,
    /// Há um pedido esperando resposta (a leitora só entrega se houver).
    esperando: Arc<AtomicBool>,
    /// `false` depois que a conexão fechou (pelo daemon ou por um quadro recusado).
    aberta: Arc<AtomicBool>,
}

impl Transporte {
    /// Liga a thread leitora à conexão já conferida.
    fn novo(stream: UnixStream, avisos: Arc<FilaAvisos>) -> Result<Transporte, ErroApp> {
        let perdida = |_| ErroApp::ConexaoPerdida;
        // A leitora espera sem prazo (os avisos chegam a qualquer hora); o prazo da
        // resposta é o do canal.
        stream.set_read_timeout(None).map_err(perdida)?;
        stream
            .set_write_timeout(Some(PRAZO_ESCRITA))
            .map_err(perdida)?;
        let leitura = stream.try_clone().map_err(perdida)?;
        let escrita = stream.try_clone().map_err(perdida)?;
        let (tx, rx) = mpsc::sync_channel(1);
        let esperando = Arc::new(AtomicBool::new(false));
        let aberta = Arc::new(AtomicBool::new(true));
        let (e, a) = (esperando.clone(), aberta.clone());
        std::thread::Builder::new()
            .name("ward-app-leitora".into())
            .spawn(move || ler(leitura, &tx, &e, &a, &avisos))
            .map_err(perdida)?;
        Ok(Transporte {
            stream,
            pedinte: Mutex::new(Pedinte {
                escrita,
                respostas: rx,
            }),
            esperando,
            aberta,
        })
    }

    fn aberta(&self) -> bool {
        self.aberta.load(Ordering::Acquire)
    }

    fn fechar(&self) {
        self.aberta.store(false, Ordering::Release);
        let _ = self.stream.shutdown(Shutdown::Both);
    }

    /// Um pedido e a sua resposta, com `prazo`.
    fn trocar(&self, pedido: &[u8], prazo: Duration) -> Result<Zeroizing<Vec<u8>>, ErroApp> {
        let mut p = self.pedinte.lock().map_err(|_| ErroApp::ConexaoPerdida)?;
        if !self.aberta() {
            return Err(ErroApp::ConexaoPerdida);
        }
        // Uma resposta que sobrou não pode virar a deste pedido (a leitora só entrega
        // com `esperando`, então isto não deveria achar nada).
        while p.respostas.try_recv().is_ok() {}
        self.esperando.store(true, Ordering::Release);
        let limite = quadro::limite_do_quadro(Papel::App);
        match quadro::escrever_quadro(&mut p.escrita, pedido, limite) {
            Ok(()) => {}
            Err(ErroQuadro::Vazio | ErroQuadro::GrandeDemais) => {
                // Nada foi escrito: a conexão continua em ordem.
                self.esperando.store(false, Ordering::Release);
                return Err(ErroApp::PedidoInvalido);
            }
            Err(_) => {
                self.fechar();
                return Err(ErroApp::ConexaoPerdida);
            }
        }
        match p.respostas.recv_timeout(prazo) {
            Ok(r) => Ok(r),
            Err(_) => {
                // Sem resposta no prazo (ou a leitora fechou): uma resposta atrasada
                // seria lida como a de outro pedido.
                self.esperando.store(false, Ordering::Release);
                self.fechar();
                Err(ErroApp::ConexaoPerdida)
            }
        }
    }
}

impl Drop for Transporte {
    /// A leitora segura uma cópia do fd: sem o `shutdown`, a conexão ficaria aberta.
    fn drop(&mut self) {
        self.fechar();
    }
}

/// A thread leitora: a única que lê o socket. Sai (e fecha a conexão) no fim da
/// conexão, num quadro recusado ou numa resposta sem pedido.
fn ler(
    mut s: UnixStream,
    tx: &SyncSender<Zeroizing<Vec<u8>>>,
    esperando: &AtomicBool,
    aberta: &AtomicBool,
    avisos: &FilaAvisos,
) {
    // Fechada, truncada, acima do limite ou erro de E/S: sai.
    while let Ok(q) = quadro::ler_quadro(&mut s, quadro::LIMITE_RESPOSTA) {
        match protocolo::decodificar_resposta(Papel::App, &q) {
            Ok(Resposta::PedidoDeAprovacao(p)) => avisos.por(Aviso::Aprovacao(p)),
            Ok(Resposta::PedidoDeDestrave) => avisos.por(Aviso::Destrave),
            Ok(_) => {
                // Resposta sem pedido: sai.
                if !esperando.swap(false, Ordering::AcqRel) || tx.send(q).is_err() {
                    break;
                }
            }
            // Tipo de outro papel ou quadro forjado.
            Err(_) => break,
        }
    }
    aberta.store(false, Ordering::Release);
    let _ = s.shutdown(Shutdown::Both);
}

/// Daemon real, pelo socket local.
pub struct DaemonSocket {
    esperado: ServidorEsperado,
    transporte: Option<Transporte>,
    /// `false` depois de [`conectar_agora`](Self::conectar_agora): a conexão não se
    /// refaz neste processo.
    pode_conectar: bool,
    avisos: Arc<FilaAvisos>,
    prazo_resposta: Duration,
}

impl DaemonSocket {
    /// O daemon instalado: `/run/ward/<uid>/wardd.sock`, conferido como socket do
    /// systemd do sistema. No build de teste, o daemon de teste em
    /// `/run/user/<uid>/ward-teste/wardd.sock`. O uid é o efetivo do processo, não
    /// `XDG_RUNTIME_DIR` nem outra variável.
    pub fn padrao() -> Result<DaemonSocket, ErroApp> {
        #[cfg(not(feature = "modo-teste"))]
        let esperado = ServidorEsperado::sistema();
        #[cfg(feature = "modo-teste")]
        let esperado = ServidorEsperado::teste();
        Ok(DaemonSocket::sem_conexao(esperado))
    }

    fn sem_conexao(esperado: ServidorEsperado) -> DaemonSocket {
        DaemonSocket {
            esperado,
            transporte: None,
            pode_conectar: true,
            avisos: Arc::new(FilaAvisos::default()),
            prazo_resposta: PRAZO_RESPOSTA,
        }
    }

    /// Conecta agora (com toda a conferência do servidor) e trava a conexão: daqui em
    /// diante este `DaemonSocket` nunca conecta de novo. É o que a partida do app chama
    /// antes de largar o grupo (D47), e o que a ponte de teste chama ao subir.
    pub fn conectar_agora(&mut self) -> Result<&UnixStream, ErroApp> {
        self.pode_conectar = false;
        self.transporte = None;
        let t = self.conectar()?;
        Ok(&self.transporte.insert(t).stream)
    }

    /// Numa thread: espera a conexão aberta fechar (pelo daemon, ou pelo app depois de
    /// um quadro recusado), sem ler nada, e chama `ao_fechar`. A ponte de teste sai por
    /// aqui quando o daemon de teste para.
    pub fn vigiar_fechamento(
        &self,
        ao_fechar: impl FnOnce() + Send + 'static,
    ) -> Result<(), ErroApp> {
        let t = self.transporte.as_ref().ok_or(ErroApp::ConexaoPerdida)?;
        let s = t.stream.try_clone().map_err(|_| ErroApp::ConexaoPerdida)?;
        std::thread::Builder::new()
            .name("ward-app-vigia".into())
            .spawn(move || {
                let _ = ward_plataforma::app::esperar_fechamento(&s);
                ao_fechar();
            })
            .map(|_| ())
            .map_err(|_| ErroApp::ConexaoPerdida)
    }

    /// Os avisos que o daemon manda sem pedido (D65 a). A mesma fila vale para toda a
    /// vida deste `DaemonSocket`.
    pub fn avisos(&self) -> Arc<FilaAvisos> {
        self.avisos.clone()
    }

    /// A conexão aberta. Sem ela: conecta no primeiro uso, menos depois de
    /// [`conectar_agora`](Self::conectar_agora), quando responde
    /// [`ErroApp::ConexaoPerdida`].
    fn transporte(&mut self) -> Result<&Transporte, ErroApp> {
        if self.transporte.as_ref().is_some_and(|t| !t.aberta()) {
            self.transporte = None;
        }
        if self.transporte.is_none() {
            if !self.pode_conectar {
                return Err(ErroApp::ConexaoPerdida);
            }
            let t = self.conectar()?;
            self.transporte = Some(t);
        }
        self.transporte.as_ref().ok_or(ErroApp::ConexaoPerdida)
    }

    /// Só nos testes: a conexão já aberta `s` (um par com o daemon falso), como a da
    /// partida (sem reconectar).
    #[cfg(test)]
    pub(crate) fn de_par(s: UnixStream, prazo: Duration) -> DaemonSocket {
        let mut d = DaemonSocket::sistema_em(PathBuf::from("/nao-usado/wardd.sock"));
        d.pode_conectar = false;
        d.prazo_resposta = prazo;
        d.transporte = Some(Transporte::novo(s, d.avisos.clone()).expect("transporte"));
        d
    }

    /// Outro caminho, com as regras do daemon instalado (pasta do root, par uid 0).
    pub fn sistema_em(caminho: PathBuf) -> DaemonSocket {
        DaemonSocket::sem_conexao(ServidorEsperado::sistema_em(caminho))
    }

    /// Outro caminho, com as regras do daemon de teste. Só no `modo-teste`.
    #[cfg(feature = "modo-teste")]
    pub fn teste_em(caminho: PathBuf) -> DaemonSocket {
        DaemonSocket::sem_conexao(ServidorEsperado::teste_em(caminho))
    }

    pub fn caminho(&self) -> &Path {
        self.esperado.caminho()
    }

    /// Confere o caminho, conecta e confere o par antes de qualquer byte, e liga a
    /// thread leitora. Nada é escrito no socket aqui.
    fn conectar(&self) -> Result<Transporte, ErroApp> {
        let s = conectar_servidor(&self.esperado).map_err(|r| match r {
            RecusaServidor::Ausente | RecusaServidor::Io(_) => ErroApp::DaemonIndisponivel,
            _ => ErroApp::DaemonNaoConfiavel,
        })?;
        Transporte::novo(s, self.avisos.clone())
    }
}

impl Daemon for DaemonSocket {
    fn trocar(&mut self, pedido: &[u8]) -> Result<Zeroizing<Vec<u8>>, ErroApp> {
        let prazo = self.prazo_resposta;
        self.transporte()?.trocar(pedido, prazo)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::fs;
    use std::io::{Read, Write};
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt, symlink};
    use std::os::unix::net::UnixListener;

    use crate::cliente;
    use crate::comandos::Referencia;
    use ward_core::protocolo::Alvo;

    impl DaemonSocket {
        fn estado(&mut self) -> Result<EstadoCofre, ErroApp> {
            cliente::estado(self)
        }
        fn valor_para_copiar(&mut self, r: &Referencia) -> Result<(), ErroApp> {
            cliente::copiar(self, r.entrada(), Alvo::Campo(r.campo().to_owned())).map(|_| ())
        }
    }

    fn meu_uid() -> u32 {
        fs::metadata("/proc/self").unwrap().uid()
    }

    /// Pasta só deste teste em `/run/user/<uid>/` (tmpfs, 0700 do usuário; o `/tmp` é
    /// 1777 e não passaria em conferência nenhuma). Apagada ao sair.
    struct Base(PathBuf);

    impl Base {
        fn nova(nome: &str) -> Base {
            let p = PathBuf::from(format!(
                "/run/user/{}/ward-app-nucleo-{}-{nome}",
                meu_uid(),
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&p);
            fs::DirBuilder::new().mode(0o700).create(&p).unwrap();
            Base(p)
        }
    }

    impl Drop for Base {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Servidor falso do mesmo uid: socket 0600 em `dir`.
    fn servidor_falso(dir: &Path) -> UnixListener {
        let l = UnixListener::bind(dir.join(SOCKET)).unwrap();
        fs::set_permissions(dir.join(SOCKET), fs::Permissions::from_mode(0o600)).unwrap();
        l.set_nonblocking(true).unwrap();
        l
    }

    /// Nenhum byte chegou ao servidor falso (aceita o que houver e lê sem bloquear).
    fn nada_recebido(l: &UnixListener) {
        while let Ok((mut c, _)) = l.accept() {
            c.set_nonblocking(true).unwrap();
            let mut buf = [0u8; 16];
            assert!(
                !matches!(c.read(&mut buf), Ok(n) if n > 0),
                "o app mandou bytes a um servidor recusado"
            );
        }
    }

    #[test]
    fn caminho_padrao_sem_variavel_de_ambiente() {
        let d = DaemonSocket::padrao().unwrap();
        #[cfg(not(feature = "modo-teste"))]
        let esperado = format!("/run/ward/{}/wardd.sock", meu_uid());
        #[cfg(feature = "modo-teste")]
        let esperado = format!("/run/user/{}/ward-teste/wardd.sock", meu_uid());
        assert_eq!(d.caminho(), Path::new(&esperado));
        #[cfg(not(feature = "modo-teste"))]
        assert!(!d.caminho().to_string_lossy().contains("ward-teste"));
    }

    #[test]
    fn socket_ausente_e_indisponivel() {
        let mut d =
            DaemonSocket::sistema_em(PathBuf::from("/run/ward-nao-existe-falso/1/wardd.sock"));
        assert_eq!(d.estado(), Err(ErroApp::DaemonIndisponivel));
        #[cfg(feature = "modo-teste")]
        {
            let base = Base::nova("ausente");
            let dir = base.0.join("ward-teste");
            fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
            let mut t = DaemonSocket::teste_em(dir.join(SOCKET));
            assert_eq!(t.estado(), Err(ErroApp::DaemonIndisponivel));
        }
    }

    #[test]
    fn ataque_servidor_falso_do_mesmo_uid_e_recusado_sem_receber_nada() {
        // V3.23: servidor do mesmo uid, socket 0600, pasta 0700 do usuário. Pela regra
        // do daemon instalado, a pasta não é do root e o par não é uid 0.
        let base = Base::nova("falso");
        let dir = base.0.join("ward");
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let l = servidor_falso(&dir);
        let mut d = DaemonSocket::sistema_em(dir.join(SOCKET));
        assert_eq!(d.estado(), Err(ErroApp::DaemonNaoConfiavel));
        let r = Referencia::nova("acme-pg-dev", "senha").unwrap();
        assert!(matches!(
            d.valor_para_copiar(&r),
            Err(ErroApp::DaemonNaoConfiavel)
        ));
        nada_recebido(&l);
    }

    #[test]
    fn ataque_sem_a_conexao_da_partida_nao_reconecta() {
        // D47: depois de largar o grupo, uma conexão nova teria o gid real (papel
        // agente). Falhou na partida: nenhum pedido conecta depois.
        let base = Base::nova("partida");
        let dir = base.0.join("ward");
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let mut d = DaemonSocket::sistema_em(dir.join(SOCKET));
        assert!(d.conectar_agora().is_err());
        let l = servidor_falso(&dir);
        assert_eq!(d.estado(), Err(ErroApp::ConexaoPerdida));
        let r = Referencia::nova("acme-pg-dev", "senha").unwrap();
        assert!(matches!(
            d.valor_para_copiar(&r),
            Err(ErroApp::ConexaoPerdida)
        ));
        assert!(l.accept().is_err(), "reconectou depois da partida");
        #[cfg(feature = "modo-teste")]
        {
            // Com a conexão da partida aberta, os pedidos usam a mesma, sem conectar
            // de novo.
            let mut t = DaemonSocket::teste_em(dir.join(SOCKET));
            let c = t.conectar_agora().unwrap();
            assert!(ward_plataforma::app::tem_cloexec(c).unwrap());
            let (mut c, _) = l.accept().unwrap();
            let servidor = std::thread::spawn(move || {
                for _ in 0..2 {
                    pedido_lido(&mut c);
                    mandar(&mut c, &estado_app());
                }
                c
            });
            assert_eq!(t.estado(), Ok(EstadoCofre::Destravado));
            assert_eq!(t.estado(), Ok(EstadoCofre::Destravado));
            let _c = servidor.join().unwrap();
            assert!(l.accept().is_err(), "conectou de novo");
        }
    }

    #[test]
    fn ataque_diretorio_aberto_para_outros_e_recusado() {
        let base = Base::nova("aberto");
        let dir = base.0.join("ward");
        fs::DirBuilder::new().mode(0o755).create(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
        let l = servidor_falso(&dir);
        let mut d = DaemonSocket::sistema_em(dir.join(SOCKET));
        assert_eq!(d.estado(), Err(ErroApp::DaemonNaoConfiavel));
        #[cfg(feature = "modo-teste")]
        {
            // Nem o build de teste aceita pasta aberta; com 0700 é o daemon de teste.
            let mut t = DaemonSocket::teste_em(dir.join(SOCKET));
            assert_eq!(t.estado(), Err(ErroApp::DaemonNaoConfiavel));
            nada_recebido(&l);
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
            t.prazo_resposta = Duration::from_millis(100);
            assert_eq!(t.estado(), Err(ErroApp::ConexaoPerdida));
            let _ = l.accept();
        }
        nada_recebido(&l);
    }

    #[test]
    fn ataque_diretorio_por_link_simbolico_e_recusado() {
        let base = Base::nova("link");
        let real = base.0.join("real");
        fs::DirBuilder::new().mode(0o700).create(&real).unwrap();
        let l = servidor_falso(&real);
        symlink(&real, base.0.join("ward")).unwrap();
        let mut d = DaemonSocket::sistema_em(base.0.join("ward").join(SOCKET));
        assert_eq!(d.estado(), Err(ErroApp::DaemonNaoConfiavel));
        #[cfg(feature = "modo-teste")]
        {
            let mut t = DaemonSocket::teste_em(base.0.join("ward").join(SOCKET));
            assert_eq!(t.estado(), Err(ErroApp::DaemonNaoConfiavel));
        }
        nada_recebido(&l);
    }

    #[test]
    fn ataque_arquivo_comum_no_lugar_do_socket_e_recusado() {
        let base = Base::nova("arquivo");
        let dir = base.0.join("ward");
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        fs::write(dir.join(SOCKET), b"").unwrap();
        fs::set_permissions(dir.join(SOCKET), fs::Permissions::from_mode(0o600)).unwrap();
        let mut d = DaemonSocket::sistema_em(dir.join(SOCKET));
        assert_eq!(d.estado(), Err(ErroApp::DaemonNaoConfiavel));
        #[cfg(feature = "modo-teste")]
        {
            let mut t = DaemonSocket::teste_em(dir.join(SOCKET));
            assert_eq!(t.estado(), Err(ErroApp::DaemonNaoConfiavel));
        }
    }

    #[test]
    fn ataque_socket_aberto_para_outros_e_recusado() {
        let base = Base::nova("socket");
        let dir = base.0.join("ward");
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let l = servidor_falso(&dir);
        fs::set_permissions(dir.join(SOCKET), fs::Permissions::from_mode(0o666)).unwrap();
        let mut d = DaemonSocket::sistema_em(dir.join(SOCKET));
        assert_eq!(d.estado(), Err(ErroApp::DaemonNaoConfiavel));
        #[cfg(feature = "modo-teste")]
        {
            let mut t = DaemonSocket::teste_em(dir.join(SOCKET));
            assert_eq!(t.estado(), Err(ErroApp::DaemonNaoConfiavel));
        }
        nada_recebido(&l);
    }

    // --- Transporte (D65 a, V5.33) com um daemon falso do outro lado de um par ----------

    fn mandar(s: &mut UnixStream, r: &Resposta) {
        let q = protocolo::codificar_resposta(Papel::App, r).unwrap();
        quadro::escrever_quadro(s, &q, quadro::LIMITE_RESPOSTA).unwrap();
    }

    fn pedido_lido(s: &mut UnixStream) -> ward_core::protocolo::Pedido {
        let q = quadro::ler_quadro(s, quadro::limite_do_quadro(Papel::App)).unwrap();
        protocolo::decodificar_pedido(Papel::App, &q).unwrap()
    }

    fn estado_app() -> Resposta {
        Resposta::Estado {
            travado: false,
            cofre_existe: true,
            papel: Papel::App,
        }
    }

    fn aprovacao(id: u64) -> PedidoDeAprovacao {
        PedidoDeAprovacao {
            id,
            entradas: vec!["acme-pg-dev".into()],
            ambiente: ward_core::entrada::Ambiente::Dev,
            operacao: ward_core::protocolo::Operacao::Executar,
            comando: "\"psql\"".into(),
            cadeia: vec!["claude".into(), "ward run".into()],
        }
    }

    /// O daemon falso fechou do lado dele? (o app fecha a conexão sem mandar nada)
    fn fechou(s: &mut UnixStream) -> bool {
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut b = [0u8; 1];
        matches!(s.read(&mut b), Ok(0))
    }

    #[test]
    fn v5_33_avisos_no_meio_vao_para_a_fila_e_a_resposta_para_quem_pediu() {
        let (a, mut b) = UnixStream::pair().unwrap();
        let mut d = DaemonSocket::de_par(a, Duration::from_secs(10));
        let avisos = d.avisos();
        let servidor = std::thread::spawn(move || {
            for _ in 0..2 {
                assert!(matches!(pedido_lido(&mut b), ward_core::protocolo::Pedido::Estado));
                mandar(&mut b, &Resposta::PedidoDeAprovacao(aprovacao(7)));
                mandar(&mut b, &Resposta::PedidoDeDestrave);
                mandar(&mut b, &estado_app());
            }
            b
        });
        assert_eq!(cliente::estado(&mut d), Ok(EstadoCofre::Destravado));
        assert_eq!(cliente::papel(&mut d), Ok(Papel::App));
        let _b = servidor.join().unwrap();
        assert_eq!(avisos.tomar(), Some(Aviso::Aprovacao(aprovacao(7))));
        assert_eq!(avisos.tomar(), Some(Aviso::Destrave));
        assert_eq!(avisos.tomar(), Some(Aviso::Aprovacao(aprovacao(7))));
        assert_eq!(avisos.esperar(Duration::from_millis(50)), Some(Aviso::Destrave));
        assert_eq!(avisos.tomar(), None);
        assert_eq!(avisos.descartados(), 0);
    }

    #[test]
    fn v5_33_mil_avisos_a_fila_nao_passa_de_16_e_o_app_segue_respondendo() {
        let (a, mut b) = UnixStream::pair().unwrap();
        let mut d = DaemonSocket::de_par(a, Duration::from_secs(30));
        let avisos = d.avisos();
        let servidor = std::thread::spawn(move || {
            for i in 0..1000 {
                if i % 2 == 0 {
                    mandar(&mut b, &Resposta::PedidoDeAprovacao(aprovacao(i)));
                } else {
                    mandar(&mut b, &Resposta::PedidoDeDestrave);
                }
            }
            for _ in 0..2 {
                pedido_lido(&mut b);
                mandar(&mut b, &estado_app());
            }
            b
        });
        assert_eq!(cliente::estado(&mut d), Ok(EstadoCofre::Destravado));
        assert_eq!(cliente::estado(&mut d), Ok(EstadoCofre::Destravado));
        let _b = servidor.join().unwrap();
        assert_eq!(avisos.tamanho(), MAX_AVISOS);
        assert_eq!(avisos.descartados(), 1000 - 16);
        // Ficam os 16 primeiros, na ordem.
        assert_eq!(avisos.tomar(), Some(Aviso::Aprovacao(aprovacao(0))));
        assert_eq!(avisos.tomar(), Some(Aviso::Destrave));
    }

    #[test]
    fn v5_33_resposta_sem_pedido_fecha_a_conexao() {
        let (a, mut b) = UnixStream::pair().unwrap();
        let mut d = DaemonSocket::de_par(a, Duration::from_secs(10));
        mandar(&mut b, &estado_app());
        assert!(fechou(&mut b), "o app não fechou a conexão");
        assert_eq!(cliente::estado(&mut d), Err(ErroApp::ConexaoPerdida));
        assert_eq!(d.avisos().tamanho(), 0);
    }

    #[test]
    fn v5_33_quadro_acima_do_limite_fecha_a_conexao() {
        let (a, mut b) = UnixStream::pair().unwrap();
        let mut d = DaemonSocket::de_par(a, Duration::from_secs(10));
        let servidor = std::thread::spawn(move || {
            pedido_lido(&mut b);
            let n = u32::try_from(quadro::LIMITE_RESPOSTA + 1).unwrap();
            b.write_all(&n.to_be_bytes()).unwrap();
            let _ = b.write_all(&[b'{'; 64]);
            fechou(&mut b)
        });
        assert_eq!(cliente::estado(&mut d), Err(ErroApp::ConexaoPerdida));
        assert!(servidor.join().unwrap(), "o app não fechou a conexão");
        assert_eq!(cliente::estado(&mut d), Err(ErroApp::ConexaoPerdida));
    }

    #[test]
    fn v5_33_tipo_do_papel_agente_ou_forjado_fecha_sem_mostrar_nada() {
        use ward_core::entrada::{Ambiente, TipoEntrada};
        use ward_core::protocolo::ItemLista;
        // A lista do agente vem sem nível (D50): não é resposta que o app aceite.
        let item = ItemLista {
            nome: "acme-pg-dev".into(),
            tipo: TipoEntrada::Banco,
            grupo: "acme".into(),
            ambiente: Ambiente::Dev,
            projeto: None,
            tags: vec![],
            nivel: None,
            endereco: None,
        };
        let do_agente = protocolo::codificar_resposta(Papel::Agente, &Resposta::Lista(vec![item]))
            .unwrap()
            .to_vec();
        let forjado = br#"{"v":1,"tipo":"estado","dados":{"travado":false,"cofre_existe":true,"papel":"app","x":1}}"#.to_vec();
        for corpo in [do_agente, forjado] {
            let (a, mut b) = UnixStream::pair().unwrap();
            let mut d = DaemonSocket::de_par(a, Duration::from_secs(10));
            let servidor = std::thread::spawn(move || {
                pedido_lido(&mut b);
                quadro::escrever_quadro(&mut b, &corpo, quadro::LIMITE_RESPOSTA).unwrap();
                fechou(&mut b)
            });
            assert_eq!(cliente::lista(&mut d).err(), Some(ErroApp::ConexaoPerdida));
            assert!(servidor.join().unwrap(), "o app não fechou a conexão");
        }
    }

    #[test]
    fn sem_resposta_no_prazo_fecha_e_a_atrasada_nao_vira_outra() {
        let (a, mut b) = UnixStream::pair().unwrap();
        let mut d = DaemonSocket::de_par(a, Duration::from_millis(200));
        let servidor = std::thread::spawn(move || {
            pedido_lido(&mut b);
            std::thread::sleep(Duration::from_millis(400));
            // A resposta atrasada: a conexão já fechou do lado do app.
            let _ = quadro::escrever_quadro(
                &mut b,
                &protocolo::codificar_resposta(Papel::App, &estado_app()).unwrap(),
                quadro::LIMITE_RESPOSTA,
            );
            fechou(&mut b)
        });
        assert_eq!(cliente::estado(&mut d), Err(ErroApp::ConexaoPerdida));
        assert!(servidor.join().unwrap());
        assert_eq!(cliente::estado(&mut d), Err(ErroApp::ConexaoPerdida));
    }

    #[test]
    fn pedido_grande_demais_nao_sai_e_a_conexao_continua() {
        let (a, mut b) = UnixStream::pair().unwrap();
        let mut d = DaemonSocket::de_par(a, Duration::from_secs(10));
        let grande = vec![b'x'; quadro::limite_do_quadro(Papel::App) + 1];
        assert_eq!(d.trocar(&grande).err(), Some(ErroApp::PedidoInvalido));
        let servidor = std::thread::spawn(move || {
            pedido_lido(&mut b);
            mandar(&mut b, &estado_app());
            b
        });
        assert_eq!(cliente::estado(&mut d), Ok(EstadoCofre::Destravado));
        servidor.join().unwrap();
    }
}
