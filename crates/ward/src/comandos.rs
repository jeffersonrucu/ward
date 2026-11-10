//! Os comandos da CLI (Fase 25): `lista [--json]`, `travar` e `destravar`; e o
//! `run` (Fase 26).
//!
//! Ordem do `destravar` (S5, D46, V4.2):
//! 1. conecta e confere o servidor; recusado, sai sem ler a senha e sem mandar nada;
//! 2. pergunta o `estado` (sem segredo); já destravado ou sem cofre, sai sem ler;
//! 3. endurece o processo (não-dumpable, sem core) e só então lê a senha do terminal;
//! 4. confere o servidor de novo no mesmo socket e só então manda a senha.

use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ward_core::entrada::Segredo;
use ward_core::limits::{KIB, MAX_ARGUMENT, MAX_ARGUMENTS, MAX_ENV_VARIABLES, MAX_SECRET};
use ward_core::protocolo::{
    ErroCodificar, ErroResposta, Fluxo, Janela, Papel, Pedido, PedidoExecutar, Resposta,
    TipoEntradaPadrao, Variavel,
};
use ward_platform::connect::ChannelError;
use ward_platform::{ExpectedServer, TerminalError};
use zeroize::Zeroizing;

use crate::args::{AJUDA, Comando};
use crate::cliente::{Conexao, ErroConexao, EscritaSessao, Sessao};
use crate::texto;

/// Código de saída: sucesso.
pub const SAIDA_OK: u8 = 0;
/// Código de saída: erro (daemon, servidor, travado, senha).
pub const SAIDA_ERRO: u8 = 1;
/// Código de saída: uso errado.
pub const SAIDA_USO: u8 = 2;
/// Código de saída do `ward run` quando o erro é do Ward, não do comando (como o `env`
/// e o `docker run`): daemon, servidor, travado, negado, tipo sem entrega.
pub const SAIDA_RUN_ERRO: u8 = 125;

/// D52: argv + ambiente do `ward run` cabem no quadro de 1 MiB depois do base64.
pub const MAX_RUN: usize = 750 * KIB;
/// D52: a mensagem do limite, sem valor nenhum.
pub const MSG_RUN_GRANDE: &str =
    "o comando e o ambiente passam de 750 KiB; reduza o ambiente (ex.: `env -i`)";
/// Mensagem de um argumento ou variável acima do limite de um campo.
pub const MSG_RUN_CAMPO: &str = "um argumento ou uma variável do ambiente passa de 128 KiB, \
     ou há argumentos ou variáveis demais; reduza o ambiente (ex.: `env -i`)";
/// Aviso de quando o cofre está travado: o pedido espera o destrave (D16).
pub const MSG_RUN_TRAVADO: &str = "cofre travado: o comando espera o destrave por até 2 min \
     (`/usr/local/bin/ward destravar` em outro terminal, ou pelo app)";
/// Pedaço da entrada padrão por quadro.
const PEDACO_ENTRADA: usize = 64 * KIB;

/// O que os comandos usam do mundo de fora (os testes trocam).
pub trait Entorno {
    /// Conecta e confere o servidor.
    fn conectar(&mut self) -> Result<Conexao, ErroConexao>;
    /// Desliga o dump do processo antes de a senha existir na memória.
    fn endurecer(&mut self) -> io::Result<()>;
    /// Lê a senha mestra do terminal.
    fn ler_senha(&mut self) -> Result<Zeroizing<Vec<u8>>, TerminalError>;
    fn saida(&mut self) -> &mut dyn Write;
    fn erro(&mut self) -> &mut dyn Write;
    /// Ambiente da CLI, para o `ward run` (D13).
    fn ambiente(&self) -> Vec<(OsString, OsString)> {
        std::env::vars_os().collect()
    }
    /// Diretório atual, para o `ward run`.
    fn diretorio(&self) -> io::Result<PathBuf> {
        std::env::current_dir()
    }
    /// Entrada padrão, repassada ao comando do `ward run`.
    fn entrada_padrao(&mut self) -> Box<dyn Read + Send> {
        Box::new(io::stdin())
    }
    /// Fase 54b (D68): o que é a entrada padrão (`fstat` do fd 0), para o daemon avisar
    /// na janela quando o comando recebe dados que ela não mostra.
    fn tipo_entrada(&self) -> TipoEntradaPadrao {
        tipo_da_entrada_padrao()
    }
    /// `ward ssh` interativo (Fase 27): o tamanho do terminal, se a entrada e a saída
    /// padrão são terminais; `None` sem terminal (pipe, arquivo).
    fn terminal(&self) -> Option<Janela> {
        if !ward_platform::terminal::is_terminal(io::stdin())
            || !ward_platform::terminal::is_terminal(io::stdout())
        {
            return None;
        }
        Some(janela_de(ward_platform::terminal::size(io::stdin())))
    }
    /// O tamanho do terminal agora (a thread que acompanha a janela pergunta de novo).
    fn tamanho_terminal(&self) -> Box<dyn Fn() -> Option<(u16, u16)> + Send> {
        Box::new(|| ward_platform::terminal::size(io::stdin()))
    }
    /// Põe o terminal de entrada em modo cru; volta quando o valor sai.
    fn modo_cru(&mut self) -> Option<ward_platform::RawMode> {
        ward_platform::RawMode::enable(io::stdin()).ok()
    }
    /// `ward ssh --token-fd <n>` (Fase 27a): lê o token do "Conectar" do fd herdado do
    /// app e fecha o fd.
    fn ler_token(&mut self, fd: u32) -> Result<Zeroizing<Vec<u8>>, ChannelError> {
        ward_platform::connect::read_inherited_token(fd)
    }
}

/// `/dev/null` (`makedev(1, 3)` no Linux).
const DEV_NULL: u64 = (1 << 8) | 3;

/// Fase 54b (D68): o tipo da entrada padrão pelo `fstat` do fd 0 (uma cópia do fd, sem
/// `unsafe`). Fechada (sem fd 0) conta como nula.
pub fn tipo_da_entrada_padrao() -> TipoEntradaPadrao {
    use std::io::IsTerminal;
    use std::os::fd::AsFd;
    let meta = io::stdin()
        .as_fd()
        .try_clone_to_owned()
        .map(std::fs::File::from)
        .and_then(|f| f.metadata());
    classificar_entrada(meta.as_ref().ok(), io::stdin().is_terminal())
}

/// Fase 54b: a classificação, separada para os testes. `None`: o `fstat` falhou (fd 0
/// fechado).
pub fn classificar_entrada(meta: Option<&std::fs::Metadata>, terminal: bool) -> TipoEntradaPadrao {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    let Some(m) = meta else {
        return TipoEntradaPadrao::Nula;
    };
    let t = m.file_type();
    if t.is_char_device() && m.rdev() == DEV_NULL {
        TipoEntradaPadrao::Nula
    } else if terminal && t.is_char_device() {
        TipoEntradaPadrao::Terminal
    } else if t.is_fifo() || t.is_socket() {
        TipoEntradaPadrao::Pipe
    } else if t.is_file() {
        TipoEntradaPadrao::Arquivo
    } else {
        TipoEntradaPadrao::Outra
    }
}

/// Tamanho do terminal para o protocolo (24×80 quando o terminal não diz).
fn janela_de(t: Option<(u16, u16)>) -> Janela {
    let (linhas, colunas) = t.unwrap_or((24, 80));
    Janela {
        linhas: linhas.clamp(1, 10_000),
        colunas: colunas.clamp(1, 10_000),
    }
}

/// O entorno de verdade: socket do [`ExpectedServer`], `/dev/tty`, stdout e stderr.
pub struct Sistema {
    pub esperado: ExpectedServer,
    pub stdout: io::Stdout,
    pub stderr: io::Stderr,
}

impl Entorno for Sistema {
    fn conectar(&mut self) -> Result<Conexao, ErroConexao> {
        Conexao::conectar(&self.esperado)
    }
    fn endurecer(&mut self) -> io::Result<()> {
        ward_platform::harden_process()
    }
    fn ler_senha(&mut self) -> Result<Zeroizing<Vec<u8>>, TerminalError> {
        ward_platform::read_password("Senha mestra do Ward: ", MAX_SECRET)
    }
    fn saida(&mut self) -> &mut dyn Write {
        &mut self.stdout
    }
    fn erro(&mut self) -> &mut dyn Write {
        &mut self.stderr
    }
}

fn falhar(e: &mut dyn Entorno, msg: &str) -> u8 {
    let _ = writeln!(e.erro(), "ward: {msg}");
    SAIDA_ERRO
}

/// Mensagem de um erro do daemon, com a dica do que fazer quando há uma.
pub fn mensagem_do_daemon(erro: &ErroResposta) -> String {
    match erro {
        // Fase 54a (RA8): pelo caminho completo, não pelo `ward` do `PATH`.
        ErroResposta::Travado => {
            "travado: destrave com `/usr/local/bin/ward destravar` ou pelo app".to_owned()
        }
        ErroResposta::CofreInexistente => "ainda não existe cofre; crie pelo app".to_owned(),
        e => e.mensagem().to_owned(),
    }
}

/// Roda um comando e devolve o código de saída.
pub fn executar(cmd: Comando, e: &mut dyn Entorno) -> u8 {
    match cmd {
        Comando::Ajuda => {
            let _ = e.saida().write_all(AJUDA.as_bytes());
            SAIDA_OK
        }
        Comando::Versao => {
            let _ = writeln!(e.saida(), "ward {}", env!("CARGO_PKG_VERSION"));
            SAIDA_OK
        }
        Comando::Lista { json } => lista(e, json),
        Comando::Travar => travar(e),
        Comando::Destravar => destravar(e),
        Comando::Run { entrada, argv } => run(e, entrada, argv),
        Comando::Ssh {
            entrada,
            token_fd,
            comando,
        } => ssh(e, entrada, token_fd, comando),
        Comando::Playwright { entrada } => playwright(e, entrada),
        Comando::Login { entrada } => login(e, entrada),
    }
}

fn pedir(e: &mut dyn Entorno, c: &mut Conexao, p: &Pedido) -> Result<Resposta, u8> {
    match c.pedir(p) {
        Ok(Resposta::Erro(erro)) => Err(falhar(e, &mensagem_do_daemon(&erro))),
        Ok(r) => Ok(r),
        Err(erro) => Err(falhar(e, &erro.mensagem())),
    }
}

fn conectar(e: &mut dyn Entorno) -> Result<Conexao, u8> {
    e.conectar().map_err(|erro| falhar(e, &erro.mensagem()))
}

fn lista(e: &mut dyn Entorno, json: bool) -> u8 {
    let mut c = match conectar(e) {
        Ok(c) => c,
        Err(codigo) => return codigo,
    };
    match pedir(e, &mut c, &Pedido::Lista) {
        Ok(Resposta::Lista(itens)) => {
            let texto = if json {
                texto::lista_json(&itens)
            } else {
                texto::lista_tabela(&itens)
            };
            if e.saida().write_all(texto.as_bytes()).is_err() {
                return SAIDA_ERRO;
            }
            SAIDA_OK
        }
        Ok(_) => falhar(e, "resposta inesperada do daemon"),
        Err(codigo) => codigo,
    }
}

fn travar(e: &mut dyn Entorno) -> u8 {
    let mut c = match conectar(e) {
        Ok(c) => c,
        Err(codigo) => return codigo,
    };
    match pedir(e, &mut c, &Pedido::Travar) {
        Ok(Resposta::Ok) => {
            let _ = writeln!(e.saida(), "cofre travado");
            SAIDA_OK
        }
        Ok(_) => falhar(e, "resposta inesperada do daemon"),
        Err(codigo) => codigo,
    }
}

fn destravar(e: &mut dyn Entorno) -> u8 {
    // 1. Servidor conferido antes de qualquer coisa.
    let mut c = match conectar(e) {
        Ok(c) => c,
        Err(codigo) => return codigo,
    };
    // 2. Estado, sem segredo.
    match pedir(e, &mut c, &Pedido::Estado) {
        Ok(Resposta::Estado {
            cofre_existe: false,
            ..
        }) => return falhar(e, &mensagem_do_daemon(&ErroResposta::CofreInexistente)),
        Ok(Resposta::Estado { travado: false, .. }) => {
            let _ = writeln!(e.saida(), "o cofre já está destravado");
            return SAIDA_OK;
        }
        Ok(Resposta::Estado { .. }) => {}
        Ok(_) => return falhar(e, "resposta inesperada do daemon"),
        Err(codigo) => return codigo,
    }
    // 3. Processo endurecido antes de a senha existir.
    if e.endurecer().is_err() {
        return falhar(
            e,
            "não deu para proteger a memória do processo; nada foi lido",
        );
    }
    let mut lida = match e.ler_senha() {
        Ok(s) => s,
        Err(TerminalError::NoTerminal) => {
            return falhar(
                e,
                "`ward destravar` precisa de um terminal para ler a senha",
            );
        }
        Err(TerminalError::Canceled) => return falhar(e, "cancelado; nada foi enviado"),
        Err(TerminalError::TooLong) => return falhar(e, "senha longa demais; nada foi enviado"),
        Err(TerminalError::Io(_)) => return falhar(e, "erro no terminal; nada foi enviado"),
    };
    if lida.is_empty() {
        return falhar(e, "senha vazia; nada foi enviado");
    }
    // 4. Servidor conferido de novo, no mesmo socket, logo antes de mandar.
    if let Err(erro) = c.reconferir() {
        return falhar(e, &erro.mensagem());
    }
    // A senha passa do `Zeroizing` para o `Segredo` sem cópia.
    let senha = Segredo::de_bytes(std::mem::take(&mut *lida));
    drop(lida);
    let r = pedir(e, &mut c, &Pedido::Destravar { senha });
    match r {
        Ok(Resposta::Ok) => {
            let _ = writeln!(e.saida(), "cofre destravado");
            SAIDA_OK
        }
        Ok(_) => falhar(e, "resposta inesperada do daemon"),
        Err(codigo) => codigo,
    }
}

/// D13: a CLI não manda `LD_*` (o daemon tira de novo).
fn variavel_mandada(nome: &[u8]) -> bool {
    !nome.is_empty() && !nome.starts_with(b"LD_") && !nome.contains(&b'=') && !nome.contains(&0)
}

/// O pedido do `executar`, ou a mensagem de por que ele não cabe (D52). Só tamanhos são
/// medidos; nenhum valor vai para a mensagem.
pub fn pedido_run(
    entrada: String,
    argv: Vec<OsString>,
    cwd: PathBuf,
    ambiente: Vec<(OsString, OsString)>,
    tipo_entrada: TipoEntradaPadrao,
) -> Result<Pedido, &'static str> {
    use std::os::unix::ffi::OsStringExt;
    let argv: Vec<Vec<u8>> = argv.into_iter().map(OsStringExt::into_vec).collect();
    let cwd = cwd.into_os_string().into_vec();
    let ambiente: Vec<(Vec<u8>, Vec<u8>)> = ambiente
        .into_iter()
        .map(|(k, v)| (k.into_vec(), v.into_vec()))
        .filter(|(k, _)| variavel_mandada(k))
        .collect();
    let total: usize = argv.iter().map(Vec::len).sum::<usize>()
        + cwd.len()
        + ambiente
            .iter()
            .map(|(k, v)| k.len() + v.len() + 1)
            .sum::<usize>();
    if total > MAX_RUN {
        return Err(MSG_RUN_GRANDE);
    }
    let campo_grande = argv
        .iter()
        .chain(std::iter::once(&cwd))
        .any(|a| a.len() > MAX_ARGUMENT)
        || ambiente
            .iter()
            .any(|(k, v)| k.len() > MAX_ARGUMENT || v.len() > MAX_ARGUMENT);
    if campo_grande || argv.len() > MAX_ARGUMENTS || ambiente.len() > MAX_ENV_VARIABLES {
        return Err(MSG_RUN_CAMPO);
    }
    let pedido = Pedido::Executar(PedidoExecutar {
        entrada,
        argv: argv.into_iter().map(Zeroizing::new).collect(),
        cwd: Zeroizing::new(cwd),
        ambiente: ambiente
            .into_iter()
            .map(|(k, v)| Variavel {
                nome: Zeroizing::new(k),
                valor: Segredo::de_bytes(v),
            })
            .collect(),
        tipo_entrada,
    });
    // O quadro inteiro (com os nomes do JSON) também cabe no limite.
    match ward_core::protocolo::codificar_pedido(Papel::Agente, &pedido) {
        Ok(_) => Ok(pedido),
        Err(ErroCodificar::GrandeDemais) => Err(MSG_RUN_GRANDE),
        Err(_) => Err(MSG_RUN_CAMPO),
    }
}

fn falhar_run(e: &mut dyn Entorno, msg: &str) -> u8 {
    let _ = writeln!(e.erro(), "ward: {msg}");
    SAIDA_RUN_ERRO
}

/// `ward run <entrada> -- <comando…>` (Fase 26). O daemon roda o comando com a
/// credencial e devolve a saída já mascarada e o código de saída; a CLI repassa a
/// entrada padrão. Nenhum segredo passa por aqui (S1).
fn run(e: &mut dyn Entorno, entrada: String, argv: Vec<OsString>) -> u8 {
    let cwd = match e.diretorio() {
        Ok(d) => d,
        Err(_) => return falhar_run(e, "não deu para ler o diretório atual"),
    };
    let ambiente = e.ambiente();
    let tipo = e.tipo_entrada();
    // D52: medido antes de conectar; nada é executado.
    let pedido = match pedido_run(entrada, argv, cwd, ambiente, tipo) {
        Ok(p) => p,
        Err(msg) => return falhar_run(e, msg),
    };
    let mut c = match e.conectar() {
        Ok(c) => c,
        Err(erro) => return falhar_run(e, &erro.mensagem()),
    };
    match c.pedir(&Pedido::Estado) {
        Ok(Resposta::Estado {
            cofre_existe: false,
            ..
        }) => return falhar_run(e, &mensagem_do_daemon(&ErroResposta::CofreInexistente)),
        Ok(Resposta::Estado { travado: true, .. }) => {
            let _ = writeln!(e.erro(), "ward: {MSG_RUN_TRAVADO}");
        }
        Ok(Resposta::Estado { .. }) => {}
        Ok(Resposta::Erro(erro)) => return falhar_run(e, &mensagem_do_daemon(&erro)),
        Ok(_) => return falhar_run(e, "resposta inesperada do daemon"),
        Err(erro) => return falhar_run(e, &erro.mensagem()),
    }
    let (sessao, escrita) = match c.abrir_sessao(&pedido) {
        Ok(s) => s,
        Err(erro) => return falhar_run(e, &erro.mensagem()),
    };
    drop(pedido);
    let escrita = Arc::new(Mutex::new(escrita));
    repassar_entrada(e, escrita, tipo);
    receber_sessao(e, sessao)
}

/// A entrada padrão vai ao comando numa thread; no fim dela, um quadro vazio. Fase 54b:
/// declarada nula, nada é lido (o fd 0 pode até ser outro arquivo, se estava fechado) e
/// só o quadro vazio vai.
fn repassar_entrada(
    e: &mut dyn Entorno,
    escrita: Arc<Mutex<EscritaSessao>>,
    tipo: TipoEntradaPadrao,
) {
    let mut fonte = if tipo == TipoEntradaPadrao::Nula {
        Box::new(io::empty())
    } else {
        e.entrada_padrao()
    };
    let _ = std::thread::Builder::new()
        .name("ward-entrada".into())
        .spawn(move || {
            let mut buf = Zeroizing::new(vec![0u8; PEDACO_ENTRADA]);
            loop {
                let n = match fonte.read(&mut buf) {
                    Ok(n) => n,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => 0,
                };
                let dados = Segredo::de_bytes(buf[..n].to_vec());
                let mut w = escrita.lock().unwrap_or_else(|e| e.into_inner());
                if w.entrada(dados).is_err() || n == 0 {
                    return;
                }
            }
        });
}

/// Repassa a saída da sessão até o `fim` e devolve o código.
fn receber_sessao(e: &mut dyn Entorno, mut sessao: Sessao) -> u8 {
    loop {
        match sessao.receber() {
            Ok(Resposta::Saida { fluxo, dados }) => {
                let destino = match fluxo {
                    Fluxo::Saida => e.saida(),
                    Fluxo::Erro => e.erro(),
                };
                // Saída fechada do lado da CLI (`| head`): o resto é descartado.
                let _ = destino.write_all(&dados).and_then(|()| destino.flush());
            }
            Ok(Resposta::Fim { codigo }) => {
                let _ = e.saida().flush();
                return codigo;
            }
            Ok(Resposta::Erro(erro)) => return falhar_run(e, &mensagem_do_daemon(&erro)),
            Ok(_) => return falhar_run(e, "resposta inesperada do daemon"),
            Err(erro) => return falhar_run(e, &erro.mensagem()),
        }
    }
}

/// De quanto em quanto tempo o `ward ssh` interativo olha o tamanho do terminal.
const PASSO_JANELA: Duration = Duration::from_millis(250);

/// O pedido do `ssh`, ou a mensagem de por que ele não cabe (os mesmos limites do
/// `ward run`, D52). Nenhum valor vai para a mensagem.
pub fn pedido_ssh(
    entrada: String,
    token: Option<Segredo>,
    comando: Vec<OsString>,
    janela: Option<Janela>,
    tipo_entrada: TipoEntradaPadrao,
) -> Result<Pedido, &'static str> {
    use std::os::unix::ffi::OsStringExt;
    let comando: Vec<Vec<u8>> = comando.into_iter().map(OsStringExt::into_vec).collect();
    if comando.iter().map(Vec::len).sum::<usize>() > MAX_RUN {
        return Err(MSG_RUN_GRANDE);
    }
    if comando.len() > MAX_ARGUMENTS || comando.iter().any(|a| a.len() > MAX_ARGUMENT) {
        return Err(MSG_RUN_CAMPO);
    }
    let pedido = Pedido::Ssh {
        entrada,
        comando: comando.into_iter().map(Zeroizing::new).collect(),
        token,
        janela,
        tipo_entrada,
    };
    match ward_core::protocolo::codificar_pedido(Papel::Agente, &pedido) {
        Ok(_) => Ok(pedido),
        Err(ErroCodificar::GrandeDemais) => Err(MSG_RUN_GRANDE),
        Err(_) => Err(MSG_RUN_CAMPO),
    }
}

/// Mensagem do token do canal fora do formato do daemon (sem o conteúdo).
pub const MSG_TOKEN_FORMATO: &str = "o token do Conectar veio fora do formato";

/// O token do "Conectar" pelo fd herdado (Fase 27a). Antes de ler, o processo fica
/// não-dumpable e sem core (o token passa a existir na memória dele). O fd fecha na
/// leitura, antes de qualquer conexão. O conteúdo nunca vai para mensagem.
fn token_do_canal(e: &mut dyn Entorno, fd: u32) -> Result<Segredo, &'static str> {
    e.endurecer()
        .map_err(|_| "não foi possível proteger o processo antes de ler o token")?;
    let mut bytes = e.ler_token(fd).map_err(ChannelError::message)?;
    if !crate::args::token_ok(&bytes) {
        return Err(MSG_TOKEN_FORMATO);
    }
    Ok(Segredo::de_bytes(std::mem::take(&mut *bytes)))
}

/// `ward ssh [--token-fd <n>] <entrada> [comando…]` (Fases 27 e 27a). O daemon conecta
/// com a credencial (senha por `sshpass`, chave pelo agente temporário de D12) e
/// devolve a saída já mascarada e o código. Sem comando e com terminal: shell remoto
/// num pty, com o terminal local em modo cru e o tamanho acompanhado. Nenhum segredo
/// passa por aqui (S1). O token do "Conectar" (D3) chega por um fd herdado do app,
/// nunca por argv ou ambiente, é lido e o fd fechado antes de conectar.
fn ssh(e: &mut dyn Entorno, entrada: String, token_fd: Option<u32>, comando: Vec<OsString>) -> u8 {
    let token = match token_fd.map(|fd| token_do_canal(e, fd)).transpose() {
        Ok(t) => t,
        Err(msg) => return falhar_run(e, msg),
    };
    let janela = if comando.is_empty() {
        e.terminal()
    } else {
        None
    };
    let tipo = e.tipo_entrada();
    let pedido = match pedido_ssh(entrada, token, comando, janela, tipo) {
        Ok(p) => p,
        Err(msg) => return falhar_run(e, msg),
    };
    let mut c = match e.conectar() {
        Ok(c) => c,
        Err(erro) => return falhar_run(e, &erro.mensagem()),
    };
    match c.pedir(&Pedido::Estado) {
        Ok(Resposta::Estado {
            cofre_existe: false,
            ..
        }) => return falhar_run(e, &mensagem_do_daemon(&ErroResposta::CofreInexistente)),
        Ok(Resposta::Estado { travado: true, .. }) => {
            let _ = writeln!(e.erro(), "ward: {MSG_RUN_TRAVADO}");
        }
        Ok(Resposta::Estado { .. }) => {}
        Ok(Resposta::Erro(erro)) => return falhar_run(e, &mensagem_do_daemon(&erro)),
        Ok(_) => return falhar_run(e, "resposta inesperada do daemon"),
        Err(erro) => return falhar_run(e, &erro.mensagem()),
    }
    let (sessao, escrita) = match c.abrir_sessao(&pedido) {
        Ok(s) => s,
        Err(erro) => return falhar_run(e, &erro.mensagem()),
    };
    drop(pedido);
    let escrita = Arc::new(Mutex::new(escrita));
    // Interativo: terminal cru enquanto a sessão durar (volta no `Drop`, mesmo em erro).
    let cru = janela.and_then(|_| e.modo_cru());
    let fim = Arc::new(AtomicBool::new(false));
    if let Some(inicial) = janela {
        acompanhar_janela(e, inicial, escrita.clone(), fim.clone());
    }
    repassar_entrada(e, escrita, tipo);
    let codigo = receber_sessao(e, sessao);
    fim.store(true, Ordering::Release);
    drop(cru);
    codigo
}

/// `ward playwright <entrada>` (Fase 44, D6): o daemon faz o login no Chrome e volta só o
/// caminho do storage state, que vai para a saída. Nenhum segredo passa por aqui (S1).
fn playwright(e: &mut dyn Entorno, entrada: String) -> u8 {
    let mut c = match conectar(e) {
        Ok(c) => c,
        Err(codigo) => return codigo,
    };
    match c.pedir(&Pedido::Estado) {
        Ok(Resposta::Estado {
            cofre_existe: false,
            ..
        }) => return falhar(e, &mensagem_do_daemon(&ErroResposta::CofreInexistente)),
        Ok(Resposta::Estado { travado: true, .. }) => {
            let _ = writeln!(e.erro(), "ward: {MSG_RUN_TRAVADO}");
        }
        Ok(Resposta::Estado { .. }) => {}
        Ok(Resposta::Erro(erro)) => return falhar(e, &mensagem_do_daemon(&erro)),
        Ok(_) => return falhar(e, "resposta inesperada do daemon"),
        Err(erro) => return falhar(e, &erro.mensagem()),
    }
    // Espera do destrave e da aprovação (2 min cada, D16, S10) mais o navegador.
    match c.pedir_com_prazo(&Pedido::Playwright { entrada }, PRAZO_PLAYWRIGHT) {
        Ok(Resposta::SessaoPlaywright { caminho })
            if caminho.starts_with('/') && !caminho.chars().any(char::is_control) =>
        {
            if writeln!(e.saida(), "{caminho}").is_err() {
                return SAIDA_ERRO;
            }
            SAIDA_OK
        }
        Ok(Resposta::Erro(erro)) => falhar(e, &mensagem_do_daemon(&erro)),
        Ok(_) => falhar(e, "resposta inesperada do daemon"),
        Err(erro) => falhar(e, &erro.mensagem()),
    }
}

/// `ward login <entrada>` (Fase 43): o daemon leva o pedido à extensão do Chrome, que
/// confere a origem da aba ativa (D9), preenche, envia e limpa os campos. Volta só `ok`
/// ou o erro; nenhum segredo passa por aqui (S1).
fn login(e: &mut dyn Entorno, entrada: String) -> u8 {
    let mut c = match conectar(e) {
        Ok(c) => c,
        Err(codigo) => return codigo,
    };
    match c.pedir(&Pedido::Estado) {
        Ok(Resposta::Estado {
            cofre_existe: false,
            ..
        }) => return falhar(e, &mensagem_do_daemon(&ErroResposta::CofreInexistente)),
        Ok(Resposta::Estado { travado: true, .. }) => {
            let _ = writeln!(e.erro(), "ward: {MSG_RUN_TRAVADO}");
        }
        Ok(Resposta::Estado { .. }) => {}
        Ok(Resposta::Erro(erro)) => return falhar(e, &mensagem_do_daemon(&erro)),
        Ok(_) => return falhar(e, "resposta inesperada do daemon"),
        Err(erro) => return falhar(e, &erro.mensagem()),
    }
    match c.pedir_com_prazo(
        &Pedido::Login {
            entrada: entrada.clone(),
        },
        PRAZO_PLAYWRIGHT,
    ) {
        Ok(Resposta::Ok) => {
            if writeln!(e.saida(), "login enviado: {entrada} (aba ativa do Chrome)").is_err() {
                return SAIDA_ERRO;
            }
            SAIDA_OK
        }
        Ok(Resposta::Erro(erro)) => falhar(e, &mensagem_do_login(&erro)),
        Ok(_) => falhar(e, "resposta inesperada do daemon"),
        Err(erro) => falhar(e, &erro.mensagem()),
    }
}

/// Mensagem de um erro do `ward login`: a origem e a extensão ganham texto próprio.
pub fn mensagem_do_login(erro: &ErroResposta) -> String {
    match erro {
        ErroResposta::OrigemDiferente => {
            "domínio não confere: a origem da aba ativa não é a da entrada; nada foi preenchido"
                .to_owned()
        }
        ErroResposta::NavegadorFalhou => {
            "a extensão do Ward no Chrome não respondeu (o Chrome está aberto?)".to_owned()
        }
        e => mensagem_do_daemon(e),
    }
}

/// Prazo da resposta do `ward playwright`: destrave (2 min) + aprovação (2 min) + o
/// navegador (1 min) e folga.
const PRAZO_PLAYWRIGHT: std::time::Duration = std::time::Duration::from_secs(330);

/// Manda o tamanho novo do terminal quando ele muda (sem tratador de sinal: olha a
/// cada [`PASSO_JANELA`]).
fn acompanhar_janela(
    e: &mut dyn Entorno,
    inicial: Janela,
    escrita: Arc<Mutex<EscritaSessao>>,
    fim: Arc<AtomicBool>,
) {
    let tamanho = e.tamanho_terminal();
    let _ = std::thread::Builder::new()
        .name("ward-janela".into())
        .spawn(move || {
            let mut atual = inicial;
            while !fim.load(Ordering::Acquire) {
                std::thread::sleep(PASSO_JANELA);
                let nova = janela_de(tamanho());
                if nova != atual {
                    atual = nova;
                    let mut w = escrita.lock().unwrap_or_else(|e| e.into_inner());
                    if w.janela(nova).is_err() {
                        return;
                    }
                }
            }
        });
}
