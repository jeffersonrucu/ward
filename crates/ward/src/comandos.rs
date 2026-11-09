//! Os comandos da CLI (Fase 25): `lista [--json]`, `travar` e `destravar`.
//!
//! Ordem do `destravar` (S5, D46, V4.2):
//! 1. conecta e confere o servidor; recusado, sai sem ler a senha e sem mandar nada;
//! 2. pergunta o `estado` (sem segredo); já destravado ou sem cofre, sai sem ler;
//! 3. endurece o processo (não-dumpable, sem core) e só então lê a senha do terminal;
//! 4. confere o servidor de novo no mesmo socket e só então manda a senha.

use std::io::{self, Write};

use ward_core::entrada::Segredo;
use ward_core::limites::MAX_SEGREDO;
use ward_core::protocolo::{ErroResposta, Pedido, Resposta};
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
