//! Entrega por variáveis de ambiente (D10): de uma [`Entrada`] validada, monta a lista
//! exata de variáveis que o `ward run` põe no ambiente do filho.
//!
//! O núcleo não faz E/S nem depende de Linux. O que precisa do sistema fica com quem
//! chama, pelo trait [`Recursos`]:
//! - o código TOTP atual (a semente nunca vira variável);
//! - o arquivo temporário de cada anexo e da chave do `servidor`/`ftp/sftp`
//!   (`WARD_ARQUIVO_<NOME>` recebe só o caminho);
//! - a porta local do túnel SSH do `banco`.
//!
//! Regras de segurança desta camada:
//! - Todo valor fica num `Zeroizing<String>` alocado uma vez só, no tamanho exato
//!   (inclusive as URLs), para não sobrar cópia do segredo em memória liberada (S8).
//! - Sem `Clone`; `Debug` mostra só os nomes, nunca um valor (S13). Os erros só têm
//!   textos fixos.
//! - Usuário, senha e base dentro das URLs vão com percent-encoding: uma senha com `@`,
//!   `/` ou `#`, ou uma base com `?host=…`, não muda para onde o cliente conecta.
//! - Antes de devolver, a lista é conferida de novo: nome de variável em
//!   `[A-Z_][A-Z0-9_]*`, sem `LD_*`, sem nome repetido e sem byte nulo em valor. O que
//!   vem de [`Recursos`] também é conferido (código TOTP só com dígitos, caminho sem
//!   nulo, porta diferente de 0).
//! - `nota segura` não tem entrega: a recusa vem antes de qualquer chamada a
//!   [`Recursos`], então nenhum arquivo é criado.
//!
//! Se a montagem falhar depois de [`Recursos::arquivo`] ter criado arquivos, quem chama
//! apaga o que criou (o `ward run` já limpa em toda saída, S2).

use core::fmt;

use zeroize::{ZeroizeOnDrop, Zeroizing};

use crate::entrada::{
    Autenticacao, Banco, Dados, Entrada, Motor, Segredo, ValorExtra, nome_variavel,
};

#[cfg(test)]
mod testes;

/// Falha de um recurso do sistema (TOTP, arquivo temporário, túnel). Só texto fixo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErroRecurso(pub &'static str);

/// O que a entrega precisa do sistema. O `ward run` (daemon) implementa; os testes usam
/// um falso.
pub trait Recursos {
    /// Código TOTP atual da semente (RFC 6238). Só dígitos, de 6 a 8.
    fn codigo_totp(&mut self, semente: &Segredo) -> Result<Zeroizing<String>, ErroRecurso>;

    /// Grava o conteúdo num arquivo temporário 0600 em `/run/user/<uid>/ward/` e devolve
    /// o caminho. `nome` é o nome do anexo (já validado: sem `/`, sem começar por `.` ou
    /// `-`) ou `chave` para a chave privada do tipo.
    fn arquivo(&mut self, nome: &str, conteudo: &Segredo) -> Result<String, ErroRecurso>;

    /// Abre o túnel SSH pelo `servidor` de nome `servidor` até `host:porta` e devolve a
    /// porta local em `127.0.0.1`.
    fn tunel(&mut self, servidor: &str, host: &str, porta: u16) -> Result<u16, ErroRecurso>;
}

/// Por que a entrega não saiu. Só textos fixos; nenhum valor da entrada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroEntrega {
    /// `nota segura`: o `ward run` recusa sem executar o comando.
    TipoSemEntrega,
    /// Um recurso do sistema falhou.
    Recurso(ErroRecurso),
    /// O recurso devolveu algo fora do formato (código TOTP, caminho ou porta).
    RecursoInvalido(&'static str),
    /// Defesa em profundidade: um valor ou nome que a validação deveria ter barrado.
    Invalida(&'static str),
}

impl fmt::Display for ErroEntrega {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErroEntrega::TipoSemEntrega => f.write_str("tipo sem entrega"),
            ErroEntrega::Recurso(ErroRecurso(m)) => write!(f, "entrega falhou: {m}"),
            ErroEntrega::RecursoInvalido(m) => write!(f, "entrega falhou: {m}"),
            ErroEntrega::Invalida(m) => write!(f, "entrada sem entrega válida: {m}"),
        }
    }
}

impl std::error::Error for ErroEntrega {}

impl From<ErroRecurso> for ErroEntrega {
    fn from(e: ErroRecurso) -> Self {
        ErroEntrega::Recurso(e)
    }
}

/// Uma variável de ambiente da entrega.
pub struct Variavel {
    nome: String,
    valor: Zeroizing<String>,
    segredo: bool,
}

impl ZeroizeOnDrop for Variavel {}

impl Variavel {
    /// Nome (não é segredo).
    pub fn nome(&self) -> &str {
        &self.nome
    }

    /// O valor. Pode ser segredo: quem chama passa direto ao ambiente do filho, sem
    /// copiar para fora de um tipo que zera e sem logar.
    pub fn expor(&self) -> &str {
        &self.valor
    }

    /// O valor vem de um segredo (senha, token, URL com senha…)? O código TOTP e o
    /// caminho de arquivo não são (D11).
    pub fn e_segredo(&self) -> bool {
        self.segredo
    }
}

impl fmt::Debug for Variavel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}=••••", self.nome)
    }
}

/// O ambiente montado, na ordem: variáveis comuns, do tipo, extras e anexos.
pub struct Entrega {
    variaveis: Vec<Variavel>,
}

impl ZeroizeOnDrop for Entrega {}

impl Entrega {
    /// As variáveis, na ordem.
    pub fn variaveis(&self) -> &[Variavel] {
        &self.variaveis
    }

    /// Valor de uma variável pelo nome.
    pub fn valor(&self, nome: &str) -> Option<&str> {
        self.variaveis
            .iter()
            .find(|v| v.nome == nome)
            .map(Variavel::expor)
    }

    /// Os nomes, na ordem.
    pub fn nomes(&self) -> impl Iterator<Item = &str> {
        self.variaveis.iter().map(Variavel::nome)
    }
}

impl fmt::Debug for Entrega {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.variaveis.iter()).finish()
    }
}

/// Monta o ambiente da entrada (D10).
pub fn montar(entrada: &Entrada, recursos: &mut dyn Recursos) -> Result<Entrega, ErroEntrega> {
    if matches!(entrada.dados(), Dados::NotaSegura(_)) {
        return Err(ErroEntrega::TipoSemEntrega);
    }
    let mut m = Montagem::default();
    m.texto("WARD_ENTRADA", entrada.nome());
    m.texto("WARD_GRUPO", entrada.grupo());
    m.texto("WARD_AMBIENTE", entrada.ambiente().nome());

    match entrada.dados() {
        Dados::Login(l) => {
            m.texto("WARD_URL", &l.url);
            m.texto("WARD_USER", &l.usuario);
            m.segredo("WARD_PASS", &l.senha)?;
            if let Some(semente) = &l.totp_semente {
                let codigo = recursos.codigo_totp(semente)?;
                if !(6..=8).contains(&codigo.len()) || !codigo.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(ErroEntrega::RecursoInvalido("código TOTP fora do formato"));
                }
                m.push("WARD_TOTP", codigo, false);
            }
        }
        Dados::Banco(b) => banco(&mut m, b, recursos)?,
        Dados::Servidor(s) => {
            m.texto("WARD_HOST", &s.host);
            m.texto("WARD_PORT", &s.porta.to_string());
            m.texto("WARD_USER", &s.usuario);
            autenticacao(&mut m, &s.autenticacao, recursos)?;
        }
        Dados::Api(a) => {
            if let Some(url) = &a.url {
                m.texto("WARD_URL", url);
            }
            m.segredo("WARD_TOKEN", &a.token)?;
            if let Some(var) = &a.variavel {
                m.segredo(var, &a.token)?;
            }
        }
        Dados::Aws(a) => {
            m.texto("AWS_ACCESS_KEY_ID", &a.access_key_id);
            m.segredo("AWS_SECRET_ACCESS_KEY", &a.secret_access_key)?;
            if let Some(t) = &a.session_token {
                m.segredo("AWS_SESSION_TOKEN", t)?;
            }
            m.texto("AWS_REGION", &a.regiao);
            m.texto("AWS_DEFAULT_REGION", &a.regiao);
            if let Some(arn) = &a.role_arn {
                m.texto("AWS_ROLE_ARN", arn);
            }
        }
        Dados::Smtp(s) => {
            m.texto("SMTP_HOST", &s.host);
            m.texto("SMTP_PORT", &s.porta.to_string());
            m.texto("SMTP_USER", &s.usuario);
            m.segredo("SMTP_PASS", &s.senha)?;
            m.texto("SMTP_SECURITY", s.seguranca.nome());
        }
        Dados::FtpSftp(f) => {
            m.texto("WARD_PROTOCOLO", f.protocolo.nome());
            m.texto("WARD_HOST", &f.host);
            m.texto("WARD_PORT", &f.porta.to_string());
            m.texto("WARD_USER", &f.usuario);
            autenticacao(&mut m, &f.autenticacao, recursos)?;
        }
        Dados::NotaSegura(_) => return Err(ErroEntrega::TipoSemEntrega),
        Dados::Generico => {}
    }

    for extra in entrada.extras() {
        let nome = prefixado("WARD_", extra.nome());
        match extra.valor() {
            ValorExtra::Texto(v) => m.texto(&nome, v),
            ValorExtra::Segredo(v) => m.segredo(&nome, v)?,
        }
    }
    for anexo in entrada.anexos() {
        let nome = prefixado("WARD_ARQUIVO_", anexo.nome());
        m.arquivo(&nome, anexo.nome(), anexo.conteudo(), recursos)?;
    }

    m.conferir()?;
    Ok(Entrega {
        variaveis: m.variaveis,
    })
}

fn prefixado(prefixo: &str, nome: &str) -> String {
    let mut s = String::from(prefixo);
    s.push_str(&nome_variavel(nome));
    s
}

fn autenticacao(
    m: &mut Montagem,
    a: &Autenticacao,
    recursos: &mut dyn Recursos,
) -> Result<(), ErroEntrega> {
    match a {
        Autenticacao::Senha(senha) => m.segredo("WARD_PASS", senha),
        Autenticacao::Chave {
            chave_privada,
            passphrase,
        } => {
            m.arquivo("WARD_ARQUIVO_CHAVE", "chave", chave_privada, recursos)?;
            if let Some(p) = passphrase {
                m.segredo("WARD_PASSPHRASE", p)?;
            }
            Ok(())
        }
    }
}

fn banco(m: &mut Montagem, b: &Banco, recursos: &mut dyn Recursos) -> Result<(), ErroEntrega> {
    // Com túnel, o cliente conecta na ponta local (D10).
    let (host, porta) = match &b.tunel {
        Some(servidor) => {
            let local = recursos.tunel(servidor, &b.host, b.porta)?;
            if local == 0 {
                return Err(ErroEntrega::RecursoInvalido("porta do túnel inválida"));
            }
            ("127.0.0.1", local)
        }
        None => (b.host.as_str(), b.porta),
    };
    let porta = porta.to_string();
    let senha = texto_do_segredo(&b.senha)?;
    let base = b.base.as_deref();
    let url = |esquema: &str, base_no_caminho: Option<&str>, consulta_base: Option<&str>| {
        url_do_banco(
            esquema,
            &b.usuario,
            senha,
            host,
            &porta,
            base_no_caminho,
            consulta_base,
        )
    };

    match b.motor {
        Motor::Postgres => {
            m.texto("PGHOST", host);
            m.texto("PGPORT", &porta);
            if let Some(base) = base {
                m.texto("PGDATABASE", base);
            }
            m.texto("PGUSER", &b.usuario);
            m.segredo("PGPASSWORD", &b.senha)?;
            m.push("DATABASE_URL", url("postgresql", base, None), true);
        }
        Motor::Mysql => {
            m.texto("MYSQL_HOST", host);
            m.texto("MYSQL_TCP_PORT", &porta);
            m.texto("MYSQL_USER", &b.usuario);
            m.segredo("MYSQL_PWD", &b.senha)?;
            if let Some(base) = base {
                m.texto("MYSQL_DATABASE", base);
            }
            m.push("DATABASE_URL", url("mysql", base, None), true);
        }
        Motor::Redis => {
            // No redis o caminho do URL é o número do banco; base que não é número fica
            // de fora do URL (o redis-cli recusaria).
            let numero = base.filter(|b| !b.is_empty() && b.bytes().all(|c| c.is_ascii_digit()));
            m.texto("REDIS_HOST", host);
            m.texto("REDIS_PORT", &porta);
            m.segredo("REDISCLI_AUTH", &b.senha)?;
            m.push("REDIS_URL", url("redis", numero, None), true);
            m.push("DATABASE_URL", url("redis", numero, None), true);
        }
        Motor::Mongo => {
            m.push("MONGODB_URI", url("mongodb", base, None), true);
            m.push("DATABASE_URL", url("mongodb", base, None), true);
        }
        Motor::Sqlserver => {
            let mut servidor = String::with_capacity(host.len() + 1 + porta.len());
            servidor.push_str(host);
            servidor.push(',');
            servidor.push_str(&porta);
            m.texto("SQLCMDSERVER", &servidor);
            m.texto("SQLCMDUSER", &b.usuario);
            m.segredo("SQLCMDPASSWORD", &b.senha)?;
            if let Some(base) = base {
                m.texto("SQLCMDDBNAME", base);
            }
            m.push("DATABASE_URL", url("sqlserver", None, base), true);
        }
    }
    Ok(())
}

/// `esquema://usuario:senha@host:porta[/base][?database=base]`, com usuário, senha e
/// base em percent-encoding. Alocado uma vez, no tamanho exato.
fn url_do_banco(
    esquema: &str,
    usuario: &str,
    senha: &str,
    host: &str,
    porta: &str,
    base_no_caminho: Option<&str>,
    consulta_base: Option<&str>,
) -> Zeroizing<String> {
    // IPv6 sem colchetes ganha colchetes; o resto do host já é só `[A-Za-z0-9._-:[]]`.
    let ipv6 = host.contains(':') && !host.starts_with('[');
    let mut partes: Vec<(&str, bool)> = vec![
        (esquema, false),
        ("://", false),
        (usuario, true),
        (":", false),
        (senha, true),
        ("@", false),
    ];
    if ipv6 {
        partes.push(("[", false));
    }
    partes.push((host, false));
    if ipv6 {
        partes.push(("]", false));
    }
    partes.push((":", false));
    partes.push((porta, false));
    if let Some(base) = base_no_caminho {
        partes.push(("/", false));
        partes.push((base, true));
    }
    if let Some(base) = consulta_base {
        partes.push(("?database=", false));
        partes.push((base, true));
    }
    juntar(&partes)
}

/// Junta as partes num texto alocado uma vez só; as marcadas vão com percent-encoding.
fn juntar(partes: &[(&str, bool)]) -> Zeroizing<String> {
    let tamanho: usize = partes
        .iter()
        .map(|(p, codificar)| {
            if *codificar {
                p.bytes().map(|b| if livre(b) { 1 } else { 3 }).sum()
            } else {
                p.len()
            }
        })
        .sum();
    let mut s = Zeroizing::new(String::with_capacity(tamanho));
    for (p, codificar) in partes {
        if *codificar {
            codificar_em(&mut s, p);
        } else {
            s.push_str(p);
        }
    }
    debug_assert_eq!(s.len(), tamanho);
    s
}

/// Caracteres "unreserved" do RFC 3986, que não precisam de percent-encoding.
fn livre(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~')
}

/// Percent-encoding (RFC 3986) com hexadecimal maiúsculo, byte a byte do UTF-8.
fn codificar_em(destino: &mut String, texto: &str) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for b in texto.bytes() {
        if livre(b) {
            destino.push(char::from(b));
        } else {
            destino.push('%');
            destino.push(char::from(HEX[usize::from(b >> 4)]));
            destino.push(char::from(HEX[usize::from(b & 0x0F)]));
        }
    }
}

fn texto_do_segredo(s: &Segredo) -> Result<&str, ErroEntrega> {
    s.expor_texto()
        .ok_or(ErroEntrega::Invalida("segredo que não é texto"))
}

#[derive(Default)]
struct Montagem {
    variaveis: Vec<Variavel>,
}

impl Montagem {
    fn push(&mut self, nome: &str, valor: Zeroizing<String>, segredo: bool) {
        self.variaveis.push(Variavel {
            nome: nome.to_owned(),
            valor,
            segredo,
        });
    }

    fn texto(&mut self, nome: &str, valor: &str) {
        self.push(nome, Zeroizing::new(valor.to_owned()), false);
    }

    fn segredo(&mut self, nome: &str, valor: &Segredo) -> Result<(), ErroEntrega> {
        let v = texto_do_segredo(valor)?;
        self.push(nome, Zeroizing::new(v.to_owned()), true);
        Ok(())
    }

    fn arquivo(
        &mut self,
        variavel: &str,
        nome: &str,
        conteudo: &Segredo,
        recursos: &mut dyn Recursos,
    ) -> Result<(), ErroEntrega> {
        let caminho = recursos.arquivo(nome, conteudo)?;
        if caminho.is_empty() {
            return Err(ErroEntrega::RecursoInvalido("caminho de arquivo vazio"));
        }
        self.push(variavel, Zeroizing::new(caminho), false);
        Ok(())
    }

    /// Defesa em profundidade sobre a lista final.
    fn conferir(&self) -> Result<(), ErroEntrega> {
        for (i, v) in self.variaveis.iter().enumerate() {
            if !nome_de_variavel_valido(&v.nome) {
                return Err(ErroEntrega::Invalida("nome de variável fora do formato"));
            }
            if v.nome.starts_with("LD_") {
                return Err(ErroEntrega::Invalida("variável LD_* na entrega"));
            }
            if self.variaveis[..i].iter().any(|o| o.nome == v.nome) {
                return Err(ErroEntrega::Invalida("variável repetida"));
            }
            if v.valor.as_bytes().contains(&0) {
                return Err(ErroEntrega::Invalida("valor com byte nulo"));
            }
        }
        Ok(())
    }
}

fn nome_de_variavel_valido(nome: &str) -> bool {
    let mut bytes = nome.bytes();
    bytes
        .next()
        .is_some_and(|c| c.is_ascii_uppercase() || c == b'_')
        && bytes.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
}
