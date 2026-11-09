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

use ward_core::entrada::Segredo;
use ward_core::limites::{KIB, MAX_ARGUMENTO, MAX_ARGUMENTOS, MAX_SEGREDO, MAX_VARIAVEIS_AMBIENTE};
use ward_core::protocolo::{
    ErroCodificar, ErroResposta, Fluxo, Papel, Pedido, PedidoExecutar, Resposta, Variavel,
};
use ward_plataforma::{ErroTerminal, ServidorEsperado};
use zeroize::Zeroizing;

use crate::args::{AJUDA, Comando};
use crate::cliente::{Conexao, ErroConexao};
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
     (`ward destravar` em outro terminal, ou pelo app)";
/// Pedaço da entrada padrão por quadro.
const PEDACO_ENTRADA: usize = 64 * KIB;

/// O que os comandos usam do mundo de fora (os testes trocam).
pub trait Entorno {
    /// Conecta e confere o servidor.
    fn conectar(&mut self) -> Result<Conexao, ErroConexao>;
    /// Desliga o dump do processo antes de a senha existir na memória.
    fn endurecer(&mut self) -> io::Result<()>;
    /// Lê a senha mestra do terminal.
    fn ler_senha(&mut self) -> Result<Zeroizing<Vec<u8>>, ErroTerminal>;
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
}

/// O entorno de verdade: socket do [`ServidorEsperado`], `/dev/tty`, stdout e stderr.
pub struct Sistema {
    pub esperado: ServidorEsperado,
    pub stdout: io::Stdout,
    pub stderr: io::Stderr,
}

impl Entorno for Sistema {
    fn conectar(&mut self) -> Result<Conexao, ErroConexao> {
        Conexao::conectar(&self.esperado)
    }
    fn endurecer(&mut self) -> io::Result<()> {
        ward_plataforma::endurecer_processo()
    }
    fn ler_senha(&mut self) -> Result<Zeroizing<Vec<u8>>, ErroTerminal> {
        ward_plataforma::ler_senha("Senha mestra do Ward: ", MAX_SEGREDO)
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
        ErroResposta::Travado => "travado: destrave com `ward destravar` ou pelo app".to_owned(),
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
        Err(ErroTerminal::SemTerminal) => {
            return falhar(
                e,
                "`ward destravar` precisa de um terminal para ler a senha",
            );
        }
        Err(ErroTerminal::Cancelado) => return falhar(e, "cancelado; nada foi enviado"),
        Err(ErroTerminal::Longa) => return falhar(e, "senha longa demais; nada foi enviado"),
        Err(ErroTerminal::Io(_)) => return falhar(e, "erro no terminal; nada foi enviado"),
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
        .any(|a| a.len() > MAX_ARGUMENTO)
        || ambiente
            .iter()
            .any(|(k, v)| k.len() > MAX_ARGUMENTO || v.len() > MAX_ARGUMENTO);
    if campo_grande || argv.len() > MAX_ARGUMENTOS || ambiente.len() > MAX_VARIAVEIS_AMBIENTE {
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
    // D52: medido antes de conectar; nada é executado.
    let pedido = match pedido_run(entrada, argv, cwd, ambiente) {
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
    let (mut sessao, mut escrita) = match c.abrir_sessao(&pedido) {
        Ok(s) => s,
        Err(erro) => return falhar_run(e, &erro.mensagem()),
    };
    drop(pedido);
    // A entrada padrão vai ao comando numa thread; no fim dela, um quadro vazio.
    let mut fonte = e.entrada_padrao();
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
                if escrita.entrada(dados).is_err() || n == 0 {
                    return;
                }
            }
        });
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
