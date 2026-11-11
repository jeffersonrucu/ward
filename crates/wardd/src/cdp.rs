//! Cliente mínimo do Chrome DevTools Protocol pelo pipe de depuração (`ward playwright`,
//! Fase 44; D6, D9; S1–S3, S8, S13).
//!
//! - **Só pipe** (S3, D6): o Chrome recebe `--remote-debugging-pipe` e lê os comandos
//!   no fd 3 e escreve as respostas no fd 4 ([`ward_platform::child::Command::launch_with_debugging`]).
//!   Nenhuma porta TCP nem socket: outro processo não acha o navegador para mandar
//!   comandos. As pontas do daemon são `CLOEXEC`.
//! - **Mensagens**: JSON terminado em `\0`. Cada mensagem lida fica num buffer
//!   `Zeroizing` com a capacidade reservada uma vez ([`MAX_MENSAGEM`]): nunca realoca, e
//!   o que passou por ele (o corpo do POST do login aparece no `Fetch.requestPaused`)
//!   zera no fim. Mensagem maior que o limite encerra a sessão.
//! - **Leitura**: só os campos usados são lidos (`serde` com structs próprias); o resto
//!   é pulado pelo `serde_json` sem cópia (`IgnoredAny`), então o corpo do POST nunca
//!   vira `String`.
//! - **Escrita**: o [`Escritor`] monta cada comando num `Zeroizing<Vec<u8>>` com a
//!   capacidade calculada antes; passar dela é erro (nunca realoca com a senha dentro).
//! - **Navegação do quadro principal** (D9): todo pedido de documento do quadro
//!   principal passa pelo `Fetch`; origem diferente da entrada é recusada com
//!   `Fetch.failRequest` antes de sair do navegador (o redirecionamento para o
//!   phishing nem chega lá, e o POST com a senha só vai à origem certa).
//! - **Prazo**: toda espera tem o prazo da sessão; vencido, a sessão acaba.
//! - Erros sem texto da página nem do navegador (S13).

use std::fs::File;
use std::io::{self, Read, Write};
use std::time::Instant;

use serde::Deserialize;
use serde::de::IgnoredAny;
use zeroize::Zeroizing;

/// Maior mensagem aceita do Chrome (a resposta dos cookies de um site comum é bem menor).
pub const MAX_MENSAGEM: usize = 4 * 1024 * 1024;
/// Quanto se lê do pipe por vez.
const PEDACO: usize = 64 * 1024;

/// Por que a conversa com o navegador acabou. Sem texto do navegador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroCdp {
    /// O prazo da sessão venceu.
    Prazo,
    /// O navegador fechou o pipe, caiu ou soltou a aba.
    Fechado,
    /// Mensagem fora do formato, grande demais, ou comando grande demais.
    Protocolo,
    /// O navegador respondeu erro a um comando.
    Comando,
    /// Escrita ou leitura falhou.
    Io,
}

/// Escritor de JSON dos comandos: só o necessário, sem realocar.
pub struct Escritor {
    b: Zeroizing<Vec<u8>>,
    /// Precisa de vírgula antes do próximo item.
    virgula: bool,
    estourou: bool,
}

impl Escritor {
    /// Escritor com capacidade fixa de `capacidade` bytes.
    pub fn novo(capacidade: usize) -> Self {
        Escritor {
            b: Zeroizing::new(Vec::with_capacity(capacidade)),
            virgula: false,
            estourou: false,
        }
    }

    fn por(&mut self, s: &[u8]) {
        if self.estourou || self.b.len() + s.len() > self.b.capacity() {
            self.estourou = true;
            return;
        }
        self.b.extend_from_slice(s);
    }

    fn separar(&mut self) {
        if self.virgula {
            self.por(b",");
        }
    }

    pub fn abre_objeto(&mut self) {
        self.separar();
        self.por(b"{");
        self.virgula = false;
    }

    pub fn fecha_objeto(&mut self) {
        self.por(b"}");
        self.virgula = true;
    }

    pub fn abre_lista(&mut self) {
        self.separar();
        self.por(b"[");
        self.virgula = false;
    }

    pub fn fecha_lista(&mut self) {
        self.por(b"]");
        self.virgula = true;
    }

    pub fn chave(&mut self, k: &str) {
        self.separar();
        self.texto_cru(k);
        self.por(b":");
        self.virgula = false;
    }

    /// Um texto com o escape do JSON (aspas, barra invertida e controles).
    pub fn texto(&mut self, s: &str) {
        self.separar();
        self.texto_cru(s);
        self.virgula = true;
    }

    fn texto_cru(&mut self, s: &str) {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        self.por(b"\"");
        for &c in s.as_bytes() {
            match c {
                b'"' => self.por(b"\\\""),
                b'\\' => self.por(b"\\\\"),
                c if c < 0x20 => {
                    self.por(&[
                        b'\\',
                        b'u',
                        b'0',
                        b'0',
                        HEX[usize::from(c >> 4)],
                        HEX[usize::from(c & 0xf)],
                    ]);
                }
                c => self.por(&[c]),
            }
        }
        self.por(b"\"");
    }

    pub fn inteiro(&mut self, n: u64) {
        self.separar();
        let mut buf = [0u8; 20];
        let mut i = buf.len();
        let mut n = n;
        loop {
            i -= 1;
            buf[i] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        self.por(&buf[i..]);
        self.virgula = true;
    }

    pub fn logico(&mut self, v: bool) {
        self.separar();
        self.por(if v { b"true" } else { b"false" });
        self.virgula = true;
    }

    pub fn nulo(&mut self) {
        self.separar();
        self.por(b"null");
        self.virgula = true;
    }

    /// Os bytes, ou `None` se passou da capacidade.
    pub fn fim(self) -> Option<Zeroizing<Vec<u8>>> {
        (!self.estourou).then_some(self.b)
    }
}

/// Tamanho do texto `s` depois do escape, no pior caso (para reservar a capacidade).
pub fn pior_caso(s: &str) -> usize {
    s.len() * 6 + 2
}

// --- Leitura ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct Envelope {
    id: Option<u64>,
    method: Option<String>,
    #[serde(rename = "sessionId")]
    sessao: Option<String>,
    error: Option<IgnoredAny>,
}

#[derive(Deserialize)]
struct ComResultado<T> {
    result: T,
}

#[derive(Deserialize)]
struct ComParametros<T> {
    params: T,
}

#[derive(Deserialize)]
struct Pausa {
    #[serde(rename = "requestId")]
    pedido: String,
    request: PedidoPausado,
    #[serde(rename = "resourceType")]
    tipo: String,
    #[serde(rename = "frameId")]
    quadro: String,
}

#[derive(Deserialize)]
struct PedidoPausado {
    url: String,
}

/// Regra da navegação do quadro principal (D9).
#[derive(Debug, Clone)]
pub struct Regra {
    /// A origem da entrada (`esquema://host[:porta]`).
    pub origem: String,
    /// O id do quadro principal da aba.
    pub quadro: String,
}

/// A conversa com um Chrome pelo pipe.
pub struct Cdp {
    escrita: File,
    leitura: File,
    buf: Zeroizing<Vec<u8>>,
    proximo: u64,
    prazo: Instant,
    /// Sessão da aba (`Target.attachToTarget` com `flatten`).
    pub sessao: Option<String>,
    /// Regra dos documentos do quadro principal; sem ela, todo pedido segue.
    pub regra: Option<Regra>,
    /// Algum documento do quadro principal foi recusado por origem.
    pub bloqueou: bool,
    /// Quantos `Page.loadEventFired` da aba chegaram.
    pub cargas: u64,
}

impl std::fmt::Debug for Cdp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cdp({} cargas, ••••)", self.cargas)
    }
}

impl Cdp {
    /// `escrita` vai ao fd 3 do Chrome; `leitura` vem do fd 4.
    pub fn novo(escrita: File, leitura: File, prazo: Instant) -> Self {
        Cdp {
            escrita,
            leitura,
            buf: Zeroizing::new(Vec::with_capacity(MAX_MENSAGEM + PEDACO)),
            proximo: 1,
            prazo,
            sessao: None,
            regra: None,
            bloqueou: false,
            cargas: 0,
        }
    }

    /// Manda um comando com os parâmetros já em JSON (objeto) e devolve o id.
    /// `da_aba`: com o `sessionId` da aba.
    pub fn enviar(&mut self, metodo: &str, params: &[u8], da_aba: bool) -> Result<u64, ErroCdp> {
        let id = self.proximo;
        self.proximo += 1;
        let sessao = if da_aba {
            Some(self.sessao.clone().ok_or(ErroCdp::Protocolo)?)
        } else {
            None
        };
        let cap = 96 + metodo.len() + params.len() + sessao.as_ref().map_or(0, |s| pior_caso(s));
        let mut e = Escritor::novo(cap);
        e.abre_objeto();
        e.chave("id");
        e.inteiro(id);
        e.chave("method");
        e.texto(metodo);
        if let Some(s) = &sessao {
            e.chave("sessionId");
            e.texto(s);
        }
        e.chave("params");
        e.virgula = false;
        e.por(params);
        e.virgula = true;
        e.fecha_objeto();
        e.por(b"\0");
        let msg = e.fim().ok_or(ErroCdp::Protocolo)?;
        self.escrita.write_all(&msg).map_err(|_| ErroCdp::Io)?;
        Ok(id)
    }

    /// Manda e espera a resposta (a mensagem inteira, para ler o `result`).
    pub fn comando(
        &mut self,
        metodo: &str,
        params: &[u8],
        da_aba: bool,
    ) -> Result<Zeroizing<Vec<u8>>, ErroCdp> {
        let id = self.enviar(metodo, params, da_aba)?;
        loop {
            let msg = self.proxima()?;
            if let Some(r) = self.tratar(&msg, Some(id))? {
                return Ok(r);
            }
        }
    }

    /// Como [`Cdp::comando`], já lendo o `result` como `T`.
    pub fn comando_de<T: for<'de> Deserialize<'de>>(
        &mut self,
        metodo: &str,
        params: &[u8],
        da_aba: bool,
    ) -> Result<T, ErroCdp> {
        let msg = self.comando(metodo, params, da_aba)?;
        serde_json::from_slice::<ComResultado<T>>(&msg)
            .map(|r| r.result)
            .map_err(|_| ErroCdp::Protocolo)
    }

    /// Espera até chegar mais um `Page.loadEventFired` da aba depois de `antes`, ou o
    /// prazo `ate` (o menor dos dois prazos). `Ok(false)`: venceu `ate`.
    pub fn esperar_carga(&mut self, antes: u64, ate: Instant) -> Result<bool, ErroCdp> {
        while self.cargas <= antes {
            if Instant::now() >= ate {
                return Ok(false);
            }
            match self.proxima_ate(ate.min(self.prazo)) {
                Ok(msg) => {
                    self.tratar(&msg, None)?;
                }
                Err(ErroCdp::Prazo) if Instant::now() < self.prazo => return Ok(false),
                Err(e) => return Err(e),
            }
        }
        Ok(true)
    }

    /// Trata uma mensagem: a resposta esperada volta em `Some`; eventos são tratados.
    fn tratar(
        &mut self,
        msg: &Zeroizing<Vec<u8>>,
        esperado: Option<u64>,
    ) -> Result<Option<Zeroizing<Vec<u8>>>, ErroCdp> {
        let env: Envelope = serde_json::from_slice(msg).map_err(|_| ErroCdp::Protocolo)?;
        if let Some(id) = env.id {
            if Some(id) == esperado {
                if env.error.is_some() {
                    return Err(ErroCdp::Comando);
                }
                return Ok(Some(msg.clone()));
            }
            // Resposta de um `continueRequest`/`failRequest` já mandado: nada a fazer.
            return Ok(None);
        }
        let Some(metodo) = env.method.as_deref() else {
            return Err(ErroCdp::Protocolo);
        };
        let da_aba = env.sessao.is_some() && env.sessao == self.sessao;
        match metodo {
            "Fetch.requestPaused" if da_aba => self.decidir(msg)?,
            "Page.loadEventFired" if da_aba => self.cargas += 1,
            "Page.javascriptDialogOpening" if da_aba => {
                // Um `alert`/`confirm` pararia a página: fecha sem aceitar.
                self.enviar("Page.handleJavaScriptDialog", b"{\"accept\":false}", true)?;
            }
            "Target.detachedFromTarget" | "Inspector.detached" | "Target.targetCrashed" => {
                return Err(ErroCdp::Fechado);
            }
            _ => {}
        }
        Ok(None)
    }

    /// D9: documento do quadro principal só da origem da entrada; o resto segue.
    fn decidir(&mut self, msg: &[u8]) -> Result<(), ErroCdp> {
        let p: ComParametros<Pausa> =
            serde_json::from_slice(msg).map_err(|_| ErroCdp::Protocolo)?;
        let p = p.params;
        let url = Zeroizing::new(p.request.url);
        let permitido = match &self.regra {
            Some(r) if p.tipo == "Document" && p.quadro == r.quadro => {
                ward_core::entry::url_origin(&url).is_some_and(|o| o == r.origem)
            }
            _ => true,
        };
        let mut e = Escritor::novo(64 + pior_caso(&p.pedido));
        e.abre_objeto();
        e.chave("requestId");
        e.texto(&p.pedido);
        if !permitido {
            e.chave("errorReason");
            e.texto("BlockedByClient");
        }
        e.fecha_objeto();
        let params = e.fim().ok_or(ErroCdp::Protocolo)?;
        if permitido {
            self.enviar("Fetch.continueRequest", &params, true)?;
        } else {
            self.bloqueou = true;
            crate::aviso!("playwright: navegação para outra origem recusada (D9)");
            self.enviar("Fetch.failRequest", &params, true)?;
        }
        Ok(())
    }

    fn proxima(&mut self) -> Result<Zeroizing<Vec<u8>>, ErroCdp> {
        self.proxima_ate(self.prazo)
    }

    /// A próxima mensagem inteira (sem o `\0`), num buffer `Zeroizing` do tamanho exato.
    fn proxima_ate(&mut self, ate: Instant) -> Result<Zeroizing<Vec<u8>>, ErroCdp> {
        loop {
            if let Some(fim) = self.buf.iter().position(|&b| b == 0) {
                let mut msg = Zeroizing::new(Vec::with_capacity(fim));
                msg.extend_from_slice(&self.buf[..fim]);
                // `drain` move o resto para o começo do mesmo buffer (sem realocar); o
                // `Zeroizing` zera a capacidade inteira no fim.
                self.buf.drain(..=fim);
                return Ok(msg);
            }
            if self.buf.len() >= MAX_MENSAGEM {
                return Err(ErroCdp::Protocolo);
            }
            let agora = Instant::now();
            if agora >= ate {
                return Err(ErroCdp::Prazo);
            }
            let ms =
                i32::try_from(ate.duration_since(agora).as_millis().max(1)).unwrap_or(i32::MAX);
            match ward_platform::child::ready_to_read(&self.leitura, ms) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(ErroCdp::Io),
            }
            let antes = self.buf.len();
            // Dentro da capacidade reservada: o `resize` não realoca.
            self.buf.resize(antes + PEDACO, 0);
            match self.leitura.read(&mut self.buf[antes..]) {
                Ok(0) => {
                    self.buf.truncate(antes);
                    return Err(ErroCdp::Fechado);
                }
                Ok(n) => self.buf.truncate(antes + n),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => self.buf.truncate(antes),
                Err(_) => {
                    self.buf.truncate(antes);
                    return Err(ErroCdp::Io);
                }
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn escritor_escapa_e_nao_realoca() {
        let mut e = Escritor::novo(256);
        e.abre_objeto();
        e.chave("a");
        e.texto("x\"y\\z\n\u{1}é");
        e.chave("b");
        e.abre_lista();
        e.inteiro(0);
        e.inteiro(18_446_744_073_709_551_615);
        e.logico(true);
        e.nulo();
        e.fecha_lista();
        e.fecha_objeto();
        let b = e.fim().unwrap();
        let v: serde_json::Value = serde_json::from_slice(&b).unwrap();
        assert_eq!(v["a"], "x\"y\\z\n\u{1}é");
        assert_eq!(v["b"][1], 18_446_744_073_709_551_615u64);
        assert_eq!(v["b"][2], true);
        assert!(v["b"][3].is_null());
        // Ataque: um texto que passa da capacidade nunca realoca (nada pela metade).
        let mut e = Escritor::novo(8);
        e.texto("falso-WP-senha-02");
        assert!(e.fim().is_none());
    }

    /// O corpo do POST no `Fetch.requestPaused` é pulado sem virar `String`: a leitura
    /// só pega os campos usados.
    #[test]
    fn pausa_le_so_url_tipo_e_quadro() {
        let msg = br#"{"method":"Fetch.requestPaused","sessionId":"S","params":{"requestId":"r1","request":{"url":"http://localhost:8081/wp-login.php","method":"POST","postData":"log=wp_editor&pwd=falso-WP-senha-02","headers":{}},"frameId":"F","resourceType":"Document","extra":[1,2.5,-3e2]}}"#;
        let p: ComParametros<Pausa> = serde_json::from_slice(msg).unwrap();
        assert_eq!(p.params.pedido, "r1");
        assert_eq!(p.params.tipo, "Document");
        assert_eq!(p.params.quadro, "F");
        assert_eq!(p.params.request.url, "http://localhost:8081/wp-login.php");
    }
}
