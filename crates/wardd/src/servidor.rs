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
}

impl Papeis {
    fn papel(&self, par: &ProcessoPar) -> Papel {
        let Some(inst) = &self.instalacao else {
            return Papel::Agente;
        };
        match par.classificar(inst, &PoliticaKernel::ler()) {
            ClassePar::App => Papel::App,
            ClassePar::HostExtensao => Papel::Extensao,
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
                daemon.atender(papel, p)
            }
            Err(e) => {
                depurar!("pedido recusado ({}): {}", papel.nome(), e.codigo());
                Resposta::Erro(e)
            }
        };
        if responder(&mut stream, papel, &resposta).is_err() {
            break;
        }
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
