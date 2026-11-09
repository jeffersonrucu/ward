//! Servidor HTTP mínimo da ponte de teste (só a biblioteca padrão). Ver o topo do crate.

use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use ward_app_nucleo::ErroApp;
use ward_app_nucleo::area_transferencia::AreaTransferencia;
use ward_app_nucleo::comandos::Comando;
use ward_app_nucleo::daemon::Daemon;
use ward_app_nucleo::servico::Servico;
use zeroize::Zeroizing;

/// Cabeçalho da requisição (linha + cabeçalhos).
pub const MAX_CABECALHO: usize = 8 * 1024;
/// Corpo de um `POST /ponte/<comando>`.
pub const MAX_CORPO: usize = 16 * 1024;
/// Conexões abertas ao mesmo tempo.
pub const MAX_CONEXOES: usize = 16;
const PRAZO: Duration = Duration::from_secs(5);

/// Mesma CSP do app (`tauri.conf.json`), com `connect-src 'self'` no lugar da IPC.
pub const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; \
font-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; \
frame-src 'none'; worker-src 'none'; object-src 'none'; media-src 'none'; manifest-src 'none'";

/// Ponte ligada a um serviço do app e a um diretório `dist-teste/`.
pub struct Ponte<D: Daemon, A: AreaTransferencia> {
    servico: Servico<D, A>,
    dist: PathBuf,
    token: Zeroizing<String>,
    host: String,
    origem: String,
    abertas: AtomicUsize,
}

impl<D: Daemon + 'static, A: AreaTransferencia> Ponte<D, A> {
    /// Abre `127.0.0.1:0` e sorteia o token.
    pub fn abrir(servico: Servico<D, A>, dist: &Path) -> io::Result<(Arc<Self>, TcpListener)> {
        let dist = fs::canonicalize(dist)?;
        if !dist.join("index.html").is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "dist sem index.html",
            ));
        }
        let ouvinte = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
        let porta = ouvinte.local_addr()?.port();
        let mut bruto = Zeroizing::new([0u8; 32]);
        getrandom::fill(&mut *bruto).map_err(|_| io::Error::other("sem aleatório do sistema"))?;
        let mut token = Zeroizing::new(String::with_capacity(64));
        for b in bruto.iter() {
            token.push(char::from(b"0123456789abcdef"[usize::from(b >> 4)]));
            token.push(char::from(b"0123456789abcdef"[usize::from(b & 0xf)]));
        }
        let host = format!("127.0.0.1:{porta}");
        let origem = format!("http://{host}");
        let ponte = Arc::new(Ponte {
            servico,
            dist,
            token,
            host,
            origem,
            abertas: AtomicUsize::new(0),
        });
        Ok((ponte, ouvinte))
    }

    /// URL que o Chrome abre (com o token no fragmento).
    pub fn url_de_abertura(&self) -> String {
        format!("{}/#token={}", self.origem, *self.token)
    }

    pub fn origem(&self) -> &str {
        &self.origem
    }

    /// Só para os testes da ponte.
    #[doc(hidden)]
    pub fn token_para_teste(&self) -> &str {
        &self.token
    }

    /// Atende até o processo terminar.
    pub fn servir(self: &Arc<Self>, ouvinte: TcpListener) {
        for conexao in ouvinte.incoming() {
            let Ok(conexao) = conexao else { continue };
            if self.abertas.fetch_add(1, Ordering::SeqCst) >= MAX_CONEXOES {
                self.abertas.fetch_sub(1, Ordering::SeqCst);
                continue; // cai a conexão
            }
            let ponte = Arc::clone(self);
            let r = thread::Builder::new()
                .name("ward-ponte".into())
                .spawn(move || {
                    let _ = ponte.atender(conexao);
                    ponte.abertas.fetch_sub(1, Ordering::SeqCst);
                });
            if r.is_err() {
                self.abertas.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }

    fn atender(&self, conexao: TcpStream) -> io::Result<()> {
        conexao.set_read_timeout(Some(PRAZO))?;
        conexao.set_write_timeout(Some(PRAZO))?;
        let mut saida = conexao.try_clone()?;
        let resposta = match ler_pedido(conexao) {
            Ok(p) => self.responder(&p),
            Err(status) => Resposta::texto(status),
        };
        resposta.escrever(&mut saida)
    }

    /// Decide a resposta de um pedido já lido.
    pub fn responder(&self, p: &Pedido) -> Resposta {
        if p.cabecalho("host") != Some(self.host.as_str()) {
            return Resposta::texto(421);
        }
        match (p.metodo.as_str(), p.caminho.as_str()) {
            ("GET", caminho) => self.estatico(caminho),
            ("POST", caminho) if caminho.starts_with("/ponte/") => {
                self.comando(p, &caminho["/ponte/".len()..])
            }
            _ => Resposta::texto(405),
        }
    }

    fn estatico(&self, caminho: &str) -> Resposta {
        let arquivo = match caminho {
            "/" | "/index.html" => self.dist.join("index.html"),
            c => match c.strip_prefix("/assets/") {
                Some(nome) if nome_de_asset_valido(nome) => self.dist.join("assets").join(nome),
                _ => return Resposta::texto(404),
            },
        };
        // Sem seguir link para fora do dist.
        let Ok(real) = fs::canonicalize(&arquivo) else {
            return Resposta::texto(404);
        };
        if !real.starts_with(&self.dist) || !real.is_file() {
            return Resposta::texto(404);
        }
        match fs::read(&real) {
            Ok(corpo) => Resposta {
                status: 200,
                tipo: tipo_de(&real),
                corpo,
            },
            Err(_) => Resposta::texto(404),
        }
    }

    fn comando(&self, p: &Pedido, nome: &str) -> Resposta {
        if p.cabecalho("origin") != Some(self.origem.as_str()) {
            return Resposta::texto(403);
        }
        let autorizado = p
            .cabecalho("authorization")
            .and_then(|v| v.strip_prefix("Bearer "))
            .is_some_and(|t| iguais(t.as_bytes(), self.token.as_bytes()));
        if !autorizado {
            return Resposta::texto(401);
        }
        if !p
            .cabecalho("content-type")
            .is_some_and(|t| t == "application/json" || t.starts_with("application/json;"))
        {
            return Resposta::texto(415);
        }
        let Some(comando) = Comando::de_nome(nome) else {
            return Resposta::erro(404, ErroApp::PedidoInvalido);
        };
        let Ok(Value::Object(args)) = serde_json::from_slice::<Value>(&p.corpo) else {
            return Resposta::erro(400, ErroApp::PedidoInvalido);
        };
        let texto = |chave: &str| args.get(chave).and_then(Value::as_str);
        let resultado = match comando {
            Comando::EstadoDoCofre => self
                .servico
                .estado_do_cofre()
                .map(|e| json!({ "estado": e.codigo() })),
            Comando::CopiarCampo => match (texto("entrada"), texto("campo")) {
                (Some(e), Some(c)) => self.servico.copiar_campo(e, c).map(|()| Value::Null),
                _ => Err(ErroApp::PedidoInvalido),
            },
        };
        match resultado {
            Ok(valor) => Resposta::json(200, &json!({ "ok": true, "valor": valor })),
            Err(e) => Resposta::erro(200, e),
        }
    }
}

fn iguais(a: &[u8], b: &[u8]) -> bool {
    use ctutils::CtEq;
    a.ct_eq(b).to_bool()
}

fn nome_de_asset_valido(nome: &str) -> bool {
    !nome.is_empty()
        && nome.len() <= 128
        && !nome.starts_with('.')
        && nome
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

fn tipo_de(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    }
}

/// Pedido HTTP já lido e dentro dos limites.
pub struct Pedido {
    pub metodo: String,
    pub caminho: String,
    cabecalhos: Vec<(String, String)>,
    pub corpo: Vec<u8>,
}

impl Pedido {
    /// Monta um pedido (testes).
    pub fn novo(metodo: &str, caminho: &str, cabecalhos: &[(&str, &str)], corpo: &[u8]) -> Pedido {
        Pedido {
            metodo: metodo.into(),
            caminho: caminho.into(),
            cabecalhos: cabecalhos
                .iter()
                .map(|(n, v)| (n.to_ascii_lowercase(), (*v).into()))
                .collect(),
            corpo: corpo.to_vec(),
        }
    }

    /// Valor do cabeçalho; repetido conta como ausente (não há como saber qual vale).
    pub fn cabecalho(&self, nome: &str) -> Option<&str> {
        let mut achados = self.cabecalhos.iter().filter(|(n, _)| n == nome);
        let primeiro = achados.next()?;
        if achados.next().is_some() {
            None
        } else {
            Some(primeiro.1.as_str())
        }
    }
}

/// Lê linha, cabeçalhos e corpo com os limites; erro é o status HTTP.
pub fn ler_pedido<R: Read>(entrada: R) -> Result<Pedido, u16> {
    let mut leitor = BufReader::new(entrada);
    let mut usados = 0usize;
    // Uma linha, sem passar do que resta do limite do cabeçalho.
    let mut ler_linha = |leitor: &mut BufReader<R>| -> Result<String, u16> {
        let mut bruto = Vec::new();
        let restante = (MAX_CABECALHO - usados) as u64;
        let n = (&mut *leitor)
            .take(restante)
            .read_until(b'\n', &mut bruto)
            .map_err(|_| 400u16)?;
        usados += n;
        if n == 0 || !bruto.ends_with(b"\n") {
            return Err(431); // limite do cabeçalho ou conexão fechada no meio
        }
        let texto = String::from_utf8(bruto).map_err(|_| 400u16)?;
        Ok(texto.trim_end_matches(['\r', '\n']).to_owned())
    };
    let linha = ler_linha(&mut leitor)?;
    let mut partes = linha.split(' ');
    let (Some(metodo), Some(caminho), Some("HTTP/1.1"), None) =
        (partes.next(), partes.next(), partes.next(), partes.next())
    else {
        return Err(400);
    };
    if !caminho.starts_with('/')
        || caminho.contains(['?', '#', '%', '\\'])
        || caminho.contains("..")
    {
        return Err(400);
    }
    let mut cabecalhos = Vec::new();
    loop {
        let l = ler_linha(&mut leitor)?;
        if l.is_empty() {
            break;
        }
        let (n, v) = l.split_once(':').ok_or(400u16)?;
        if n.is_empty() || !n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return Err(400);
        }
        cabecalhos.push((n.to_ascii_lowercase(), v.trim().to_owned()));
    }
    let mut pedido = Pedido {
        metodo: metodo.into(),
        caminho: caminho.into(),
        cabecalhos,
        corpo: Vec::new(),
    };
    if pedido
        .cabecalhos
        .iter()
        .any(|(n, _)| n == "transfer-encoding")
    {
        return Err(501);
    }
    let tamanho = match pedido.cabecalho("content-length") {
        None if pedido.cabecalhos.iter().any(|(n, _)| n == "content-length") => return Err(400),
        None => 0,
        Some(v) if !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()) => {
            v.parse::<usize>().map_err(|_| 413u16)?
        }
        Some(_) => return Err(400),
    };
    if tamanho > MAX_CORPO {
        return Err(413);
    }
    let mut corpo = Vec::with_capacity(tamanho);
    leitor
        .take(tamanho as u64)
        .read_to_end(&mut corpo)
        .map_err(|_| 400u16)?;
    if corpo.len() != tamanho {
        return Err(400);
    }
    pedido.corpo = corpo;
    Ok(pedido)
}

/// Resposta pronta para escrever.
pub struct Resposta {
    pub status: u16,
    tipo: &'static str,
    pub corpo: Vec<u8>,
}

impl Resposta {
    fn texto(status: u16) -> Resposta {
        Resposta {
            status,
            tipo: "text/plain; charset=utf-8",
            corpo: motivo(status).as_bytes().to_vec(),
        }
    }

    fn json(status: u16, v: &Value) -> Resposta {
        Resposta {
            status,
            tipo: "application/json",
            corpo: v.to_string().into_bytes(),
        }
    }

    fn erro(status: u16, e: ErroApp) -> Resposta {
        Resposta::json(
            status,
            &json!({ "ok": false, "codigo": e.codigo(), "mensagem": e.to_string() }),
        )
    }

    pub fn corpo_json(&self) -> Option<Value> {
        serde_json::from_slice(&self.corpo).ok()
    }

    fn escrever<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let cabecalho = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nContent-Security-Policy: {CSP}\r\n\
X-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nCache-Control: no-store\r\n\
Cross-Origin-Opener-Policy: same-origin\r\nCross-Origin-Resource-Policy: same-origin\r\n\
X-Frame-Options: DENY\r\nConnection: close\r\n\r\n",
            self.status,
            motivo(self.status),
            self.tipo,
            self.corpo.len()
        );
        w.write_all(cabecalho.as_bytes())?;
        w.write_all(&self.corpo)?;
        w.flush()
    }
}

fn motivo(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        421 => "Misdirected Request",
        431 => "Request Header Fields Too Large",
        501 => "Not Implemented",
        _ => "Internal Server Error",
    }
}
