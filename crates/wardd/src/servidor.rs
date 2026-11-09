//! Laço do socket: aceita, confere o uid do par (`SO_PEERCRED`, S5), dá o papel pelo
//! grupo (D47), lê quadros com o limite do papel e responde.
//!
//! - Uid diferente, ou sem identidade do processo: fecha sem mandar nenhum byte e
//!   registra `UidRecusado` com uid e pid (V3.2).
//! - No máximo [`MAX_CONEXOES`] conexões ao mesmo tempo, uma thread cada. Cheio: a
//!   conexão ociosa (esperando quadro) mais antiga é derrubada para dar lugar à nova;
//!   sem nenhuma ociosa, a nova recebe `ocupado` e fecha. Assim 10 000 conexões caladas
//!   não impedem um cliente normal de ser atendido (V3.4).
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

use ward_core::protocolo::{
    self, ErroResposta, Papel, Resposta,
    quadro::{self, ErroQuadro},
};
use ward_plataforma::{ClassePar, ErroAceitar, Escuta, Instalacao, PoliticaKernel, ProcessoPar};

use crate::aprovacao::{Assinante, Cliente};
use crate::lancamento::AppLancado;
use crate::nucleo::Daemon;
use crate::{aviso, depurar};

/// Conexões atendidas ao mesmo tempo.
pub const MAX_CONEXOES: usize = 64;
/// Prazo de cada leitura do socket.
pub const PRAZO_LEITURA: Duration = Duration::from_secs(60);
/// Prazo de cada escrita.
pub const PRAZO_ESCRITA: Duration = Duration::from_secs(10);

struct Vaga {
    id: u64,
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
    /// Reserva uma vaga, derrubando a ociosa mais antiga se estiver cheio.
    fn reservar(&self, stream: &UnixStream) -> Option<(u64, Arc<AtomicBool>)> {
        let mut lista = self.lista.lock().unwrap_or_else(|e| e.into_inner());
        if lista.len() >= MAX_CONEXOES {
            let velha = lista
                .iter()
                .enumerate()
                .filter(|(_, v)| v.ociosa.load(Ordering::Acquire))
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
    pub instalacao: Option<Instalacao>,
    /// O app que o lançador abriu (D63). `None`: ninguém é app.
    pub app: Option<AppLancado>,
}

/// O papel de um par já classificado pelo grupo e pelo pai (D47). Papel app só para o
/// processo `pid` que o lançador abriu e que continua vivo (D63): outro filho do daemon
/// com o grupo do app (um `ward run` do `ward-app`, Fase 26) fica agente.
pub fn papel_da_classe(classe: ClassePar, pid: u32, e_o_app: impl Fn(u32) -> bool) -> Papel {
    match classe {
        ClassePar::App if e_o_app(pid) => Papel::App,
        ClassePar::App => {
            aviso!("papel app recusado (pid {pid}): não é o app aberto pelo lançador (D63)");
            Papel::Agente
        }
        ClassePar::HostExtensao => Papel::Extensao,
        ClassePar::Comum { .. } => Papel::Agente,
    }
}

impl Papeis {
    fn papel(&self, par: &ProcessoPar) -> Papel {
        let Some(inst) = &self.instalacao else {
            return Papel::Agente;
        };
        let pid = par.credenciais().pid;
        match par.classificar(inst, &PoliticaKernel::ler()) {
            c @ (ClassePar::App | ClassePar::HostExtensao) => {
                papel_da_classe(c, pid, |p| self.app.as_ref().is_some_and(|a| a.e_o_app(p)))
            }
            ClassePar::Comum { motivo } => {
                // Só registra quando o cliente tentou um grupo especial.
                let cred = par.credenciais();
                if cred.gid == inst.gid_app() || cred.gid == inst.gid_extensao() {
                    aviso!("papel especial recusado (pid {}): {motivo}", cred.pid);
                }
                Papel::Agente
            }
        }
    }
}

/// Escreve uma resposta para o papel. Uma resposta que o papel não pode receber vira
/// erro interno (nunca sai credencial para o agente, S1).
fn responder(stream: &mut UnixStream, papel: Papel, r: &Resposta) -> Result<(), ErroQuadro> {
    let q = match protocolo::codificar_resposta(papel, r) {
        Ok(q) => q,
        Err(_) => {
            aviso!("resposta recusada para o papel {}", papel.nome());
            protocolo::codificar_resposta(papel, &Resposta::Erro(ErroResposta::Interno))
                .map_err(|_| ErroQuadro::Io(io::ErrorKind::InvalidData))?
        }
    };
    quadro::escrever_quadro(stream, &q, quadro::LIMITE_RESPOSTA)
}

/// A conexão do app que assinou: o motor escreve nela com a mesma trava das respostas.
struct AssinanteSocket {
    stream: UnixStream,
    escrita: Arc<Mutex<()>>,
}

impl Assinante for AssinanteSocket {
    fn entregar(&mut self, r: &Resposta) -> bool {
        let _g = self.escrita.lock().unwrap_or_else(|e| e.into_inner());
        let ok = responder(&mut self.stream, Papel::App, r).is_ok();
        if !ok {
            let _ = self.stream.shutdown(Shutdown::Both);
        }
        ok
    }
}

/// O cliente da conexão, para o motor: cadeia de processos e se ele caiu.
struct ClienteSocket<'a> {
    stream: &'a UnixStream,
    par: &'a ProcessoPar,
}

impl Cliente for ClienteSocket<'_> {
    fn cadeia(&self) -> Vec<String> {
        self.par.cadeia().into_iter().map(|e| e.nome).collect()
    }

    /// Olha sem ler nem esperar (`poll`): fim de arquivo ou erro é cliente que caiu.
    fn saiu(&self) -> bool {
        ward_plataforma::par_fechou(self.stream)
    }
}

fn atender_conexao(
    mut stream: UnixStream,
    par: ProcessoPar,
    papel: Papel,
    daemon: &Daemon,
    vagas: &Vagas,
    id: u64,
) {
    let _ = stream.set_read_timeout(Some(PRAZO_LEITURA));
    let _ = stream.set_write_timeout(Some(PRAZO_ESCRITA));
    let limite = quadro::limite_do_quadro(papel);
    let escrita = Arc::new(Mutex::new(()));
    // D65 (a): só a conexão do papel app recebe os avisos do motor, e fica sem prazo de
    // leitura (espera aberta).
    let mut assinou = false;
    if papel == Papel::App
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
        let bytes = match quadro::ler_quadro(&mut stream, limite) {
            Ok(b) => b,
            Err(ErroQuadro::Fechado) | Err(ErroQuadro::Io(_)) | Err(ErroQuadro::Truncado) => {
                break;
            }
            Err(e) => {
                depurar!("quadro recusado ({}): {e}", papel.nome());
                let _ = responder(
                    &mut stream,
                    papel,
                    &Resposta::Erro(protocolo::erro_do_quadro(e)),
                );
                break;
            }
        };
        vagas.marcar(id, false);
        if papel != Papel::Agente && par.reconferir_sem_depurador().is_err() {
            aviso!(
                "cliente {} sob depurador ou encerrado: conexão fechada",
                papel.nome()
            );
            break;
        }
        let resposta = match protocolo::decodificar_pedido(papel, &bytes) {
            Ok(p) => {
                drop(bytes);
                let cliente = ClienteSocket {
                    stream: &stream,
                    par: &par,
                };
                daemon.atender_de(&cliente, papel, p)
            }
            Err(e) => {
                depurar!("pedido recusado ({}): {}", papel.nome(), e.codigo());
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
pub fn servir(escuta: &Escuta, daemon: Arc<Daemon>, papeis: Arc<Papeis>) -> io::Result<()> {
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
        let par = match ward_plataforma::conferir_conexao(&stream) {
            Ok(p) => p,
            Err(ErroAceitar::UidRecusado { uid, pid }) => {
                // Fecha sem mandar nenhum byte (V3.2).
                aviso!("conexão recusada (UidRecusado): uid {uid}, pid {pid}");
                drop(stream);
                continue;
            }
            Err(ErroAceitar::SemIdentidade(e)) => {
                aviso!("conexão recusada (SemIdentidade): {}", e.kind());
                drop(stream);
                continue;
            }
            Err(ErroAceitar::Io(e)) => {
                aviso!("conexão recusada (Io): {}", e.kind());
                drop(stream);
                continue;
            }
        };
        let papel = papeis.papel(&par);
        let Some((id, _)) = vagas.reservar(&stream) else {
            let mut s = stream;
            let _ = s.set_write_timeout(Some(Duration::from_secs(1)));
            let _ = responder(&mut s, papel, &Resposta::Erro(ErroResposta::Ocupado));
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
    use ward_core::entrada::Ambiente;
    use ward_core::protocolo::{Operacao, PedidoDeAprovacao};

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
            ambiente: Ambiente::Prod,
            operacao: Operacao::Executar,
            comando: "\"true\"".into(),
            cadeia: vec!["claude".into()],
        };
        assert!(s.entregar(&Resposta::PedidoDeAprovacao(p.clone())));
        assert!(s.entregar(&Resposta::PedidoDeDestrave));
        let q = quadro::ler_quadro(&mut b, quadro::LIMITE_RESPOSTA).unwrap();
        match protocolo::decodificar_resposta(Papel::App, &q).unwrap() {
            Resposta::PedidoDeAprovacao(lido) => assert_eq!(lido, p),
            r => panic!("{r:?}"),
        }
        let q = quadro::ler_quadro(&mut b, quadro::LIMITE_RESPOSTA).unwrap();
        assert!(matches!(
            protocolo::decodificar_resposta(Papel::App, &q).unwrap(),
            Resposta::PedidoDeDestrave
        ));
        // O agente não lê esse quadro.
        assert!(protocolo::decodificar_resposta(Papel::Agente, &q).is_err());
        drop(b);
        // Outro teste em paralelo que faz `fork` segura uma cópia do fd até o `exec`
        // (CLOEXEC): a falha vem logo depois.
        let falhou = (0..200).any(|_| {
            let f = !s.entregar(&Resposta::PedidoDeDestrave);
            if !f {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            f
        });
        assert!(falhou);
    }
}
