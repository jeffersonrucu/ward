//! Host de native messaging da extensão do Chrome (Fase 40; D5, D9, D47; S1, S8, S13).
//!
//! O Chrome executa o `ward-nm` instalado (root:`ward-nm` 2755, D47) como filho direto
//! do processo do navegador, com um argumento só, a origem da extensão
//! (`chrome-extension://<id>/`), e fala com ele pelo stdin/stdout em mensagens de
//! native messaging: 4 bytes de tamanho na ordem nativa e o corpo JSON em UTF-8.
//!
//! Cada mensagem da extensão é um pedido do protocolo do daemon
//! (`{"v":1,"op":"…","dados":{…}}`), lido com a mesma leitura estrita do daemon no
//! papel extensão. Só passam as operações da extensão ([`OPERACOES`]); o pedido é
//! recodificado (forma canônica) e vai ao `wardd` numa conexão própria, conferida antes
//! do primeiro byte (D46). Quem decide o papel é o daemon, pelo egid e pelo pai Chrome
//! (D47): aberto por qualquer outro processo, o `ward-nm` vira agente e o daemon recusa
//! (V6.1). A origem da aba é comparada no daemon (D9), nunca aqui.
//!
//! A resposta do daemon é lida no papel extensão (uma resposta de segredo, token ou
//! lista do app não decodifica), conferida contra a operação pedida
//! ([`resposta_cabe`]) e recodificada para a extensão. Nada mais sai no stdout.
//!
//! Limites: mensagem da extensão até [`MAX_MENSAGEM_ENTRADA`]; maior que isso, vazia ou
//! cortada, o host responde o erro e sai (o fluxo perdeu o compasso). Resposta à
//! extensão até [`MAX_MENSAGEM_SAIDA`] (o limite do Chrome). Mensagens e quadros com
//! credencial ficam em `Zeroizing`. O stderr (que o Chrome junta ao log dele) só leva
//! textos fixos, nunca o conteúdo de uma mensagem (S13).

#![forbid(unsafe_code)]

use std::io::{self, Read, Write};

use zeroize::Zeroizing;

use ward_core::limites::KIB;
use ward_core::protocolo::{self, ErroResposta, Operacao, Papel, Resposta, TipoResposta};

/// Maior mensagem aceita da extensão. O maior pedido legítimo é um `salvar_login` com
/// a URL, o usuário e a senha (cada um até 2 KiB, D25) em base64.
pub const MAX_MENSAGEM_ENTRADA: usize = 64 * KIB;
/// Maior mensagem que o Chrome aceita do host (1 MiB).
pub const MAX_MENSAGEM_SAIDA: usize = 1024 * KIB;

/// Operações que o host leva ao daemon: as do papel extensão na tabela do protocolo.
/// Lista explícita: uma operação nova na tabela não passa sem mudar aqui (o teste
/// `operacoes_sao_as_da_extensao` prende as duas).
pub const OPERACOES: [Operacao; 5] = [
    Operacao::Estado,
    Operacao::Sugerir,
    Operacao::Preencher,
    Operacao::SalvarLogin,
    Operacao::AbrirApp,
];

/// A resposta do daemon cabe na operação pedida? Erro cabe em todas; fora isso, cada
/// operação tem um tipo só (um `preenchimento` nunca responde um `sugerir`).
pub fn resposta_cabe(op: Operacao, tipo: TipoResposta) -> bool {
    tipo == TipoResposta::Erro
        || matches!(
            (op, tipo),
            (Operacao::Estado, TipoResposta::Estado)
                | (Operacao::Sugerir, TipoResposta::Sugestoes)
                | (Operacao::Preencher, TipoResposta::Preenchimento)
                | (Operacao::SalvarLogin, TipoResposta::Ok)
                | (Operacao::AbrirApp, TipoResposta::Ok)
        )
}

/// Id da extensão do Ward (Fase 41): sai da chave pública do campo `key` do
/// `extensao/manifest.json` (sha-256 da chave em DER, 32 primeiros dígitos hex em
/// `a`–`p`). `dev/extensao/montar.py` recusa montar a extensão se o id do manifesto não
/// for este.
pub const ID_EXTENSAO: &str = "nhkehjnmlgkebhmelajfpmpblepaichf";

/// O argumento que o Chrome passa: `chrome-extension://<id>/`. Só o id do Ward
/// ([`ID_EXTENSAO`]) passa: outra extensão que tenha um manifesto de native messaging
/// apontando para o `ward-nm` (no perfil do usuário, por exemplo) não chega ao daemon.
/// Qualquer outra coisa (outro esquema, caminho, outro id) recusa.
pub fn origem_da_extensao_valida(arg: &[u8]) -> bool {
    let Some(resto) = arg.strip_prefix(b"chrome-extension://") else {
        return false;
    };
    let Some(id) = resto.strip_suffix(b"/") else {
        return false;
    };
    id.len() == 32 && id.iter().all(|c| (b'a'..=b'p').contains(c)) && id == ID_EXTENSAO.as_bytes()
}

/// Por que a leitura de uma mensagem da extensão parou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroMensagem {
    /// Fim do stdin antes do primeiro byte: o Chrome fechou (fim normal).
    Fim,
    /// Cortada no meio do tamanho ou do corpo.
    Cortada,
    /// Tamanho 0.
    Vazia,
    /// Acima de [`MAX_MENSAGEM_ENTRADA`].
    GrandeDemais,
    /// Erro de E/S.
    Io(io::ErrorKind),
}

/// Lê uma mensagem de native messaging (tamanho na ordem nativa). O corpo só é lido
/// depois de conferir o tamanho; nada além do tamanho declarado é lido.
pub fn ler_mensagem(r: &mut impl Read) -> Result<Zeroizing<Vec<u8>>, ErroMensagem> {
    let mut cab = [0u8; 4];
    let mut lidos = 0;
    while lidos < cab.len() {
        match r.read(&mut cab[lidos..]) {
            Ok(0) if lidos == 0 => return Err(ErroMensagem::Fim),
            Ok(0) => return Err(ErroMensagem::Cortada),
            Ok(n) => lidos += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(ErroMensagem::Io(e.kind())),
        }
    }
    let n = usize::try_from(u32::from_ne_bytes(cab)).map_err(|_| ErroMensagem::GrandeDemais)?;
    if n == 0 {
        return Err(ErroMensagem::Vazia);
    }
    if n > MAX_MENSAGEM_ENTRADA {
        return Err(ErroMensagem::GrandeDemais);
    }
    let mut corpo = Zeroizing::new(vec![0u8; n]);
    r.read_exact(&mut corpo).map_err(|e| match e.kind() {
        io::ErrorKind::UnexpectedEof => ErroMensagem::Cortada,
        k => ErroMensagem::Io(k),
    })?;
    Ok(corpo)
}

/// Escreve uma mensagem de native messaging. Acima de [`MAX_MENSAGEM_SAIDA`] não sai
/// nada.
pub fn escrever_mensagem(w: &mut impl Write, corpo: &[u8]) -> io::Result<()> {
    if corpo.is_empty() || corpo.len() > MAX_MENSAGEM_SAIDA {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let n = u32::try_from(corpo.len()).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let mut tudo = Zeroizing::new(Vec::with_capacity(4 + corpo.len()));
    tudo.extend_from_slice(&n.to_ne_bytes());
    tudo.extend_from_slice(corpo);
    w.write_all(&tudo)?;
    w.flush()
}

/// Falha ao falar com o daemon (só para o log; a extensão recebe `interno`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FalhaDaemon {
    /// Não há daemon, ou o servidor não passou na conferência (nada foi enviado).
    Indisponivel,
    /// Conexão caiu, prazo, quadro ruim.
    Conversa,
}

impl FalhaDaemon {
    pub fn texto(self) -> &'static str {
        match self {
            FalhaDaemon::Indisponivel => "daemon indisponível ou não conferido",
            FalhaDaemon::Conversa => "a conversa com o daemon falhou",
        }
    }
}

/// O caminho até o daemon: manda o quadro do pedido e devolve o corpo da resposta.
pub trait Daemon {
    fn trocar(&mut self, pedido: &[u8]) -> Result<Zeroizing<Vec<u8>>, FalhaDaemon>;
}

/// Corpo da resposta de erro para a extensão.
fn erro(e: ErroResposta) -> Zeroizing<Vec<u8>> {
    protocolo::codificar_resposta(Papel::Extensao, &Resposta::Erro(e)).unwrap_or_else(|_| {
        Zeroizing::new(
            br#"{"v":1,"tipo":"erro","dados":{"codigo":"interno","mensagem":"erro interno"}}"#
                .to_vec(),
        )
    })
}

/// Atende uma mensagem da extensão e devolve o corpo da resposta (sempre uma resposta
/// do papel extensão). O texto do log, quando há, vai em `log`, sem conteúdo.
pub fn atender(
    mensagem: &[u8],
    daemon: &mut dyn Daemon,
    log: &mut dyn FnMut(&str),
) -> Zeroizing<Vec<u8>> {
    let pedido = match protocolo::decodificar_pedido(Papel::Extensao, mensagem) {
        Ok(p) => p,
        Err(e) => {
            log("mensagem da extensão recusada pela leitura do protocolo");
            return erro(e);
        }
    };
    let op = pedido.operacao();
    if !OPERACOES.contains(&op) {
        log("operação fora das da extensão");
        return erro(ErroResposta::NaoAutorizado);
    }
    let Ok(quadro) = protocolo::codificar_pedido(Papel::Extensao, &pedido) else {
        return erro(ErroResposta::PedidoInvalido);
    };
    drop(pedido);
    let corpo = match daemon.trocar(&quadro) {
        Ok(c) => c,
        Err(f) => {
            log(f.texto());
            return erro(ErroResposta::Interno);
        }
    };
    drop(quadro);
    let resposta = match protocolo::decodificar_resposta(Papel::Extensao, &corpo) {
        Ok(r) if resposta_cabe(op, r.tipo()) => r,
        _ => {
            log("resposta do daemon fora do papel extensão ou da operação");
            return erro(ErroResposta::Interno);
        }
    };
    // D63: o `estado` traz o papel que o daemon deu a esta conexão. Sem o papel
    // extensão (o host não foi aberto pelo Chrome instalado, ou não é o binário setgid),
    // a extensão recebe só `nao_autorizado`.
    if let Resposta::Estado { papel, .. } = &resposta
        && *papel != Papel::Extensao
    {
        log("o daemon não deu o papel extensão a este host");
        return erro(ErroResposta::NaoAutorizado);
    }
    match protocolo::codificar_resposta(Papel::Extensao, &resposta) {
        Ok(c) if c.len() <= MAX_MENSAGEM_SAIDA => c,
        _ => {
            log("resposta grande demais para a extensão");
            erro(ErroResposta::Interno)
        }
    }
}

/// Laço do host: lê cada mensagem, atende e responde. Sai com `Ok` no fim do stdin;
/// com `Err` (depois de responder o erro, quando dá) se a mensagem veio vazia, grande
/// demais ou cortada, ou se o stdout falhou.
pub fn servir(
    entrada: &mut impl Read,
    saida: &mut impl Write,
    daemon: &mut dyn Daemon,
    log: &mut dyn FnMut(&str),
) -> Result<(), ErroMensagem> {
    loop {
        let mensagem = match ler_mensagem(entrada) {
            Ok(m) => m,
            Err(ErroMensagem::Fim) => return Ok(()),
            Err(e) => {
                log("mensagem da extensão vazia, grande demais ou cortada");
                if matches!(e, ErroMensagem::Vazia | ErroMensagem::GrandeDemais) {
                    let _ = escrever_mensagem(saida, &erro(ErroResposta::QuadroInvalido));
                }
                return Err(e);
            }
        };
        let resposta = atender(&mensagem, daemon, log);
        drop(mensagem);
        escrever_mensagem(saida, &resposta).map_err(|e| ErroMensagem::Io(e.kind()))?;
    }
}

#[cfg(test)]
mod testes;
