//! Conexão com o `wardd` (S1, S5, D46). Toda conexão passa pela conferência do servidor
//! de `ward_platform::server` antes do primeiro byte: no build de release, pastas
//! do root, socket 0600 do próprio uid, endereço do par igual ao caminho e par com
//! uid 0. A CLI fala sempre como o papel agente e só aceita resposta desse papel: uma
//! resposta com credencial é recusada na leitura (S1).

use std::io;
use std::os::unix::net::UnixStream;
use std::time::Duration;

use ward_core::limits::MAX_COMMON_FRAME;
use ward_core::protocolo::quadro::{self, ErroQuadro, LIMITE_RESPOSTA};
use ward_core::protocolo::{self, Papel, Pedido, Resposta};
use ward_platform::{ExpectedServer, ServerRefusal, connect_server, verify_server};

/// Prazo para o daemon responder. O destravar pode levar a espera crescente de D20
/// (até 30 s) mais o Argon2id.
const PRAZO_LEITURA: Duration = Duration::from_secs(130);
const PRAZO_ESCRITA: Duration = Duration::from_secs(10);

/// O servidor que esta CLI aceita.
///
/// - Release: só o `wardd` instalado, `/run/ward/<uid>/wardd.sock` (D46). Nenhuma
///   variável de ambiente nem argumento troca o caminho ou as regras.
/// - `test-mode`: o daemon de teste (D7) em `WARD_DEV_SOCKET_DIR` ou
///   `/run/user/<uid>/ward-teste/`.
pub fn servidor_esperado() -> ExpectedServer {
    #[cfg(feature = "test-mode")]
    {
        if let Some(d) = std::env::var_os("WARD_DEV_SOCKET_DIR") {
            let d = std::path::PathBuf::from(d);
            return ExpectedServer::test_at(d.join(ward_platform::socket::SOCKET_NAME));
        }
        ExpectedServer::test()
    }
    #[cfg(not(feature = "test-mode"))]
    {
        ExpectedServer::system()
    }
}

/// Por que a conversa com o daemon falhou. Sem conteúdo do quadro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroConexao {
    /// Não há daemon escutando.
    Ausente,
    /// O servidor não passou na conferência: nada foi enviado.
    NaoConfiavel(String),
    /// O daemon não respondeu no prazo.
    SemResposta,
    /// O daemon fechou a conexão.
    Fechada,
    /// Outro erro de E/S.
    Io(io::ErrorKind),
    /// Resposta fora do formato, de outro papel ou grande demais.
    RespostaInvalida,
    /// Pedido que o protocolo recusa (não deveria acontecer).
    PedidoInvalido,
}

impl ErroConexao {
    pub fn mensagem(&self) -> String {
        match self {
            ErroConexao::Ausente => "o daemon não está rodando".to_owned(),
            ErroConexao::NaoConfiavel(m) => {
                format!("servidor não é o wardd ({m}); nada foi enviado")
            }
            ErroConexao::SemResposta => "o daemon não respondeu a tempo".to_owned(),
            ErroConexao::Fechada => "o daemon fechou a conexão".to_owned(),
            ErroConexao::Io(k) => format!("erro ao falar com o daemon ({k})"),
            ErroConexao::RespostaInvalida => "resposta inválida do daemon".to_owned(),
            ErroConexao::PedidoInvalido => "pedido inválido".to_owned(),
        }
    }
}

impl From<ServerRefusal> for ErroConexao {
    fn from(r: ServerRefusal) -> Self {
        match r {
            ServerRefusal::Missing => ErroConexao::Ausente,
            ServerRefusal::Io(k) => ErroConexao::Io(k),
            outra => ErroConexao::NaoConfiavel(outra.to_string()),
        }
    }
}

fn erro_de_io(e: &io::Error) -> ErroConexao {
    match e.kind() {
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => ErroConexao::SemResposta,
        io::ErrorKind::BrokenPipe
        | io::ErrorKind::ConnectionReset
        | io::ErrorKind::UnexpectedEof => ErroConexao::Fechada,
        k => ErroConexao::Io(k),
    }
}

fn erro_do_quadro(e: ErroQuadro) -> ErroConexao {
    match e {
        ErroQuadro::Fechado | ErroQuadro::Truncado => ErroConexao::Fechada,
        ErroQuadro::Vazio | ErroQuadro::GrandeDemais => ErroConexao::RespostaInvalida,
        ErroQuadro::Io(k) => erro_de_io(&k.into()),
    }
}

/// Conexão conferida com o daemon.
#[derive(Debug)]
pub struct Conexao {
    stream: UnixStream,
    esperado: ExpectedServer,
    /// `test-mode`: exigir que o processo do socket seja não-dumpable (ver
    /// [`Conexao::conferir_extra`]). Só os testes da própria CLI desligam.
    #[cfg(feature = "test-mode")]
    exigir_nao_dumpable: bool,
}

impl Conexao {
    /// Confere o servidor, conecta e confere de novo; nenhum byte é escrito antes.
    pub fn conectar(esperado: &ExpectedServer) -> Result<Conexao, ErroConexao> {
        Conexao::conectar_com(esperado, true)
    }

    fn conectar_com(
        esperado: &ExpectedServer,
        _exigir_nao_dumpable: bool,
    ) -> Result<Conexao, ErroConexao> {
        let stream = connect_server(esperado)?;
        stream
            .set_read_timeout(Some(PRAZO_LEITURA))
            .and_then(|()| stream.set_write_timeout(Some(PRAZO_ESCRITA)))
            .map_err(|e| erro_de_io(&e))?;
        let c = Conexao {
            stream,
            esperado: esperado.clone(),
            #[cfg(feature = "test-mode")]
            exigir_nao_dumpable: _exigir_nao_dumpable,
        };
        c.conferir_extra()?;
        Ok(c)
    }

    /// Só nos testes da CLI: conecta sem a conferência extra do `test-mode` (o
    /// servidor falso honesto do teste roda no próprio processo, que é dumpable).
    #[cfg(all(test, feature = "test-mode"))]
    pub(crate) fn conectar_sem_extra(esperado: &ExpectedServer) -> Result<Conexao, ErroConexao> {
        Conexao::conectar_com(esperado, false)
    }

    /// Confere o servidor de novo no socket já aberto, logo antes de mandar a senha.
    pub fn reconferir(&self) -> Result<(), ErroConexao> {
        verify_server(&self.stream, &self.esperado)?;
        self.conferir_extra()
    }

    /// `test-mode`: sem a conferência do uid 0 (o daemon de teste é do próprio
    /// usuário, D46), a CLI exige pelo menos que o processo do outro lado seja
    /// não-dumpable, como o `wardd` (S8): `/proc/<pid>/exe` dele dá `EACCES`. Um
    /// servidor falso comum (um script Python, um `socat`) é dumpable e é recusado. Não
    /// é garantia: um falso feito para isso pode se marcar não-dumpable. A garantia é a
    /// do release (uid 0).
    fn conferir_extra(&self) -> Result<(), ErroConexao> {
        #[cfg(feature = "test-mode")]
        if self.exigir_nao_dumpable {
            let par = ward_platform::PeerProcess::from_socket(&self.stream)
                .map_err(|e| erro_de_io(&e))?;
            let pid = par.credentials().pid;
            match std::fs::read_link(format!("/proc/{pid}/exe")) {
                Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {}
                _ => {
                    return Err(ErroConexao::NaoConfiavel(
                        "o processo do socket pode ser depurado; o wardd não pode".to_owned(),
                    ));
                }
            }
            // O mesmo processo, vivo e sem depurador.
            par.reverify_no_debugger().map_err(|_| {
                ErroConexao::NaoConfiavel(
                    "o processo do socket mudou ou está sob depurador".to_owned(),
                )
            })?;
        }
        Ok(())
    }

    /// Como [`Conexao::pedir`], esperando a resposta até `prazo` (o `ward playwright`
    /// espera o destrave, a aprovação e o navegador). O prazo volta ao normal depois.
    pub fn pedir_com_prazo(
        &mut self,
        pedido: &Pedido,
        prazo: Duration,
    ) -> Result<Resposta, ErroConexao> {
        self.stream
            .set_read_timeout(Some(prazo))
            .map_err(|e| erro_de_io(&e))?;
        let r = self.pedir(pedido);
        let _ = self.stream.set_read_timeout(Some(PRAZO_LEITURA));
        r
    }

    /// Manda um pedido do papel agente e lê uma resposta do papel agente.
    pub fn pedir(&mut self, pedido: &Pedido) -> Result<Resposta, ErroConexao> {
        let corpo = protocolo::codificar_pedido(Papel::Agente, pedido)
            .map_err(|_| ErroConexao::PedidoInvalido)?;
        quadro::escrever_quadro(&mut self.stream, &corpo, MAX_COMMON_FRAME)
            .map_err(erro_do_quadro)?;
        drop(corpo);
        let resposta =
            quadro::ler_quadro(&mut self.stream, LIMITE_RESPOSTA).map_err(erro_do_quadro)?;
        protocolo::decodificar_resposta(Papel::Agente, &resposta)
            .map_err(|_| ErroConexao::RespostaInvalida)
    }
}

/// Sessão do `ward run` (Fase 26): depois do `executar`, o daemon manda `saida` e `fim`;
/// a CLI manda a entrada padrão em `entrada_padrao` (vazio = fim). Sem prazo de leitura
/// nem de escrita: o comando dura o que durar, e a pressão do pipe volta até o filho.
#[derive(Debug)]
pub struct Sessao {
    leitura: UnixStream,
}

/// A ponta de escrita da sessão (a thread da entrada padrão).
#[derive(Debug)]
pub struct EscritaSessao {
    escrita: UnixStream,
}

impl Conexao {
    /// Manda o pedido que abre a sessão (`executar`) e devolve as duas pontas.
    pub fn abrir_sessao(self, pedido: &Pedido) -> Result<(Sessao, EscritaSessao), ErroConexao> {
        let mut stream = self.stream;
        let corpo = protocolo::codificar_pedido(Papel::Agente, pedido)
            .map_err(|_| ErroConexao::PedidoInvalido)?;
        quadro::escrever_quadro(&mut stream, &corpo, MAX_COMMON_FRAME).map_err(erro_do_quadro)?;
        drop(corpo);
        stream
            .set_read_timeout(None)
            .and_then(|()| stream.set_write_timeout(None))
            .map_err(|e| erro_de_io(&e))?;
        let escrita = stream.try_clone().map_err(|e| erro_de_io(&e))?;
        Ok((Sessao { leitura: stream }, EscritaSessao { escrita }))
    }
}

impl Sessao {
    /// A próxima resposta do daemon (do papel agente: uma com credencial é recusada).
    pub fn receber(&mut self) -> Result<Resposta, ErroConexao> {
        let q = quadro::ler_quadro(&mut self.leitura, LIMITE_RESPOSTA).map_err(erro_do_quadro)?;
        protocolo::decodificar_resposta(Papel::Agente, &q)
            .map_err(|_| ErroConexao::RespostaInvalida)
    }
}

impl EscritaSessao {
    /// Manda um pedaço da entrada padrão (vazio = fim da entrada).
    pub fn entrada(&mut self, dados: ward_core::entrada::Segredo) -> Result<(), ErroConexao> {
        let corpo = protocolo::codificar_pedido(Papel::Agente, &Pedido::EntradaPadrao { dados })
            .map_err(|_| ErroConexao::PedidoInvalido)?;
        quadro::escrever_quadro(&mut self.escrita, &corpo, MAX_COMMON_FRAME).map_err(erro_do_quadro)
    }

    /// Manda o novo tamanho do terminal (`ward ssh` interativo, Fase 27).
    pub fn janela(&mut self, j: ward_core::protocolo::Janela) -> Result<(), ErroConexao> {
        let corpo = protocolo::codificar_pedido(Papel::Agente, &Pedido::Janela(j))
            .map_err(|_| ErroConexao::PedidoInvalido)?;
        quadro::escrever_quadro(&mut self.escrita, &corpo, MAX_COMMON_FRAME).map_err(erro_do_quadro)
    }
}
