//! Modelo de entrada do cofre (R5–R9, D10, D24): os 9 tipos, campos secretos, campos
//! extras, anexos, `grupo`/`ambiente`/`projeto`/tags e nível de acesso.
//!
//! - Uma [`Entrada`] só nasce validada: de um [`Rascunho`] por [`Rascunho::validar`].
//!   Os campos ficam privados, então nenhum código monta uma entrada inválida.
//! - Todo valor secreto está num [`Segredo`], que zera ao sair, não tem `Clone` e
//!   imprime `••••` (S8, S13). Nenhum tipo daqui tem `Clone`.
//! - Os erros de validação ([`ErroValidacao`]) só carregam textos fixos em português,
//!   nunca um valor digitado: um erro não vaza segredo nem por engano.
//! - Entrada nova sempre começa "com aprovação" (S9). [`Entrada::definir_nivel`] existe
//!   para o daemon, que exige o PIN antes de chamar.
//!
//! O que a validação barra de propósito (entrada do app e da extensão não é confiável):
//! tamanhos, caracteres de controle e de formatação Unicode (que forjariam o texto da
//! janela de aprovação), host ou usuário começando com `-` (viraria opção do `ssh`),
//! nomes de campo extra e anexo que viram a mesma variável (D10) ou batem com uma
//! variável do tipo, e `LD_*`/`WARD_*` como variável do `api`.

use core::fmt;

use zeroize::ZeroizeOnDrop;

mod segredo;
mod validacao;

#[cfg(test)]
mod testes;

pub use segredo::Segredo;
pub use validacao::{
    ErroValidacao, ErrosValidacao, GRANDE_PARA_MASCARA, Rascunho, RascunhoAnexo, RascunhoDados,
    RascunhoExtra, nome_variavel, origem_da_url,
};
// Nomes reservados que a entrega (D10) monta; o teste da entrega confere que batem.
#[cfg(test)]
pub(crate) use validacao::{VARIAVEIS_COMUNS, anexos_do_tipo, variaveis_do_tipo};

/// Ambiente da entrada (obrigatório). Organiza e filtra; não muda regra nenhuma (R16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ambiente {
    Dev,
    Stage,
    Prod,
}

impl Ambiente {
    /// Todos, na ordem da interface.
    pub const TODOS: [Ambiente; 3] = [Ambiente::Dev, Ambiente::Stage, Ambiente::Prod];

    /// Nome usado na interface, na CLI e em `WARD_AMBIENTE`.
    pub fn nome(self) -> &'static str {
        match self {
            Ambiente::Dev => "dev",
            Ambiente::Stage => "stage",
            Ambiente::Prod => "prod",
        }
    }

    /// Lê o nome exato (`dev`, `stage` ou `prod`).
    pub fn de_nome(nome: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|a| a.nome() == nome)
    }
}

impl fmt::Display for Ambiente {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.nome())
    }
}

/// Nível de acesso da entrada para o agente (R15). O padrão é [`NivelAcesso::ComAprovacao`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum NivelAcesso {
    /// O agente nem vê a entrada.
    Bloqueado,
    /// Cada uso abre a janela de aprovação.
    #[default]
    ComAprovacao,
    /// Usa sem perguntar.
    Livre,
}

impl NivelAcesso {
    /// Todos, do mais restritivo ao menos restritivo.
    pub const TODOS: [NivelAcesso; 3] = [
        NivelAcesso::Bloqueado,
        NivelAcesso::ComAprovacao,
        NivelAcesso::Livre,
    ];

    /// Identificador estável (`bloqueado`, `com-aprovacao`, `livre`).
    pub fn nome(self) -> &'static str {
        match self {
            NivelAcesso::Bloqueado => "bloqueado",
            NivelAcesso::ComAprovacao => "com-aprovacao",
            NivelAcesso::Livre => "livre",
        }
    }

    /// Texto da interface.
    pub fn rotulo(self) -> &'static str {
        match self {
            NivelAcesso::Bloqueado => "bloqueado",
            NivelAcesso::ComAprovacao => "com aprovação",
            NivelAcesso::Livre => "livre",
        }
    }

    /// Lê o identificador exato.
    pub fn de_nome(nome: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|n| n.nome() == nome)
    }

    /// O mais restritivo dos dois (nível combinado, D19).
    pub fn mais_restritivo(self, outro: Self) -> Self {
        fn ordem(n: NivelAcesso) -> u8 {
            match n {
                NivelAcesso::Bloqueado => 0,
                NivelAcesso::ComAprovacao => 1,
                NivelAcesso::Livre => 2,
            }
        }
        if ordem(outro) < ordem(self) {
            outro
        } else {
            self
        }
    }
}

impl fmt::Display for NivelAcesso {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.rotulo())
    }
}

/// Os 9 tipos da v1 (R5). Tipos customizados ficam para a v2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TipoEntrada {
    Login,
    Banco,
    Servidor,
    Api,
    Aws,
    Smtp,
    FtpSftp,
    NotaSegura,
    Generico,
}

impl TipoEntrada {
    /// Todos, na ordem do escopo.
    pub const TODOS: [TipoEntrada; 9] = [
        TipoEntrada::Login,
        TipoEntrada::Banco,
        TipoEntrada::Servidor,
        TipoEntrada::Api,
        TipoEntrada::Aws,
        TipoEntrada::Smtp,
        TipoEntrada::FtpSftp,
        TipoEntrada::NotaSegura,
        TipoEntrada::Generico,
    ];

    /// Identificador estável, sem espaço nem acento (o de `dev/fixtures/entradas.json`).
    pub fn nome(self) -> &'static str {
        match self {
            TipoEntrada::Login => "login",
            TipoEntrada::Banco => "banco",
            TipoEntrada::Servidor => "servidor",
            TipoEntrada::Api => "api",
            TipoEntrada::Aws => "aws",
            TipoEntrada::Smtp => "smtp",
            TipoEntrada::FtpSftp => "ftp-sftp",
            TipoEntrada::NotaSegura => "nota-segura",
            TipoEntrada::Generico => "generico",
        }
    }

    /// Texto da interface.
    pub fn rotulo(self) -> &'static str {
        match self {
            TipoEntrada::FtpSftp => "ftp/sftp",
            TipoEntrada::NotaSegura => "nota segura",
            TipoEntrada::Generico => "genérico",
            outro => outro.nome(),
        }
    }

    /// Lê o identificador exato.
    pub fn de_nome(nome: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|t| t.nome() == nome)
    }
}

impl fmt::Display for TipoEntrada {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.rotulo())
    }
}

/// Motor do tipo `banco`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Motor {
    Postgres,
    Mysql,
    Mongo,
    Redis,
    Sqlserver,
}

impl Motor {
    /// Todos, na ordem do escopo.
    pub const TODOS: [Motor; 5] = [
        Motor::Postgres,
        Motor::Mysql,
        Motor::Mongo,
        Motor::Redis,
        Motor::Sqlserver,
    ];

    /// Identificador.
    pub fn nome(self) -> &'static str {
        match self {
            Motor::Postgres => "postgres",
            Motor::Mysql => "mysql",
            Motor::Mongo => "mongo",
            Motor::Redis => "redis",
            Motor::Sqlserver => "sqlserver",
        }
    }

    /// Lê o identificador exato.
    pub fn de_nome(nome: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|m| m.nome() == nome)
    }

    /// Porta padrão, usada quando o formulário não traz porta.
    pub fn porta_padrao(self) -> u16 {
        match self {
            Motor::Postgres => 5432,
            Motor::Mysql => 3306,
            Motor::Mongo => 27017,
            Motor::Redis => 6379,
            Motor::Sqlserver => 1433,
        }
    }
}

/// Segurança da conexão do `smtp` (`SMTP_SECURITY`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SegurancaSmtp {
    Nenhuma,
    Starttls,
    Tls,
}

impl SegurancaSmtp {
    /// Todas.
    pub const TODAS: [SegurancaSmtp; 3] = [
        SegurancaSmtp::Nenhuma,
        SegurancaSmtp::Starttls,
        SegurancaSmtp::Tls,
    ];

    /// Identificador (o valor de `SMTP_SECURITY`).
    pub fn nome(self) -> &'static str {
        match self {
            SegurancaSmtp::Nenhuma => "nenhuma",
            SegurancaSmtp::Starttls => "starttls",
            SegurancaSmtp::Tls => "tls",
        }
    }

    /// Lê o identificador exato.
    pub fn de_nome(nome: &str) -> Option<Self> {
        Self::TODAS.into_iter().find(|s| s.nome() == nome)
    }

    /// Porta padrão de cada modo.
    pub fn porta_padrao(self) -> u16 {
        match self {
            SegurancaSmtp::Nenhuma => 25,
            SegurancaSmtp::Starttls => 587,
            SegurancaSmtp::Tls => 465,
        }
    }
}

/// Protocolo do tipo `ftp/sftp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocolo {
    Ftp,
    Sftp,
}

impl Protocolo {
    /// Identificador (o valor de `WARD_PROTOCOLO`).
    pub fn nome(self) -> &'static str {
        match self {
            Protocolo::Ftp => "ftp",
            Protocolo::Sftp => "sftp",
        }
    }

    /// Lê o identificador exato.
    pub fn de_nome(nome: &str) -> Option<Self> {
        [Protocolo::Ftp, Protocolo::Sftp]
            .into_iter()
            .find(|p| p.nome() == nome)
    }

    /// Porta padrão.
    pub fn porta_padrao(self) -> u16 {
        match self {
            Protocolo::Ftp => 21,
            Protocolo::Sftp => 22,
        }
    }
}

/// Senha **ou** chave (com passphrase opcional), nunca as duas (V2.12).
#[derive(Debug)]
pub enum Autenticacao {
    Senha(Segredo),
    Chave {
        chave_privada: Segredo,
        passphrase: Option<Segredo>,
    },
}

/// `login`: URL, usuário, senha e semente TOTP opcional.
#[derive(Debug)]
pub struct Login {
    pub url: String,
    pub usuario: String,
    pub senha: Segredo,
    pub totp_semente: Option<Segredo>,
}

/// `banco`: motor, host, porta, base, usuário, senha e túnel SSH opcional (nome de uma
/// entrada `servidor`).
#[derive(Debug)]
pub struct Banco {
    pub motor: Motor,
    pub host: String,
    pub porta: u16,
    pub base: Option<String>,
    pub usuario: String,
    pub senha: Segredo,
    pub tunel: Option<String>,
}

/// `servidor`: host, porta, usuário, senha ou chave e salto opcional (nome de outra
/// entrada `servidor`).
#[derive(Debug)]
pub struct Servidor {
    pub host: String,
    pub porta: u16,
    pub usuario: String,
    pub autenticacao: Autenticacao,
    pub salto: Option<String>,
}

/// `api`: URL base, token e a variável extra que recebe o token (ex.: `GITHUB_TOKEN`).
#[derive(Debug)]
pub struct Api {
    pub url: Option<String>,
    pub token: Segredo,
    pub variavel: Option<String>,
}

/// `aws`: o access key ID é identificador, não segredo (decisão do M0).
#[derive(Debug)]
pub struct Aws {
    pub access_key_id: String,
    pub secret_access_key: Segredo,
    pub regiao: String,
    pub role_arn: Option<String>,
    pub session_token: Option<Segredo>,
}

/// `smtp`.
#[derive(Debug)]
pub struct Smtp {
    pub host: String,
    pub porta: u16,
    pub usuario: String,
    pub senha: Segredo,
    pub seguranca: SegurancaSmtp,
}

/// `ftp/sftp`. Chave só com `sftp`.
#[derive(Debug)]
pub struct FtpSftp {
    pub protocolo: Protocolo,
    pub host: String,
    pub porta: u16,
    pub usuario: String,
    pub autenticacao: Autenticacao,
}

/// `nota segura`: só o texto, sem entrega.
#[derive(Debug)]
pub struct NotaSegura {
    pub texto: Segredo,
}

/// Os campos próprios de cada tipo.
#[derive(Debug)]
pub enum Dados {
    Login(Login),
    Banco(Banco),
    Servidor(Servidor),
    Api(Api),
    Aws(Aws),
    Smtp(Smtp),
    FtpSftp(FtpSftp),
    NotaSegura(NotaSegura),
    /// Só campos extras.
    Generico,
}

impl Dados {
    /// O tipo destes dados.
    pub fn tipo(&self) -> TipoEntrada {
        match self {
            Dados::Login(_) => TipoEntrada::Login,
            Dados::Banco(_) => TipoEntrada::Banco,
            Dados::Servidor(_) => TipoEntrada::Servidor,
            Dados::Api(_) => TipoEntrada::Api,
            Dados::Aws(_) => TipoEntrada::Aws,
            Dados::Smtp(_) => TipoEntrada::Smtp,
            Dados::FtpSftp(_) => TipoEntrada::FtpSftp,
            Dados::NotaSegura(_) => TipoEntrada::NotaSegura,
            Dados::Generico => TipoEntrada::Generico,
        }
    }
}

/// Valor de um campo extra: texto comum ou segredo.
#[derive(Debug)]
pub enum ValorExtra {
    Texto(String),
    Segredo(Segredo),
}

/// Campo extra (nome, valor, segredo sim/não). Vira `WARD_<NOME>` (D10).
#[derive(Debug)]
pub struct CampoExtra {
    nome: String,
    valor: ValorExtra,
}

impl CampoExtra {
    /// Nome como o usuário digitou.
    pub fn nome(&self) -> &str {
        &self.nome
    }

    /// Valor.
    pub fn valor(&self) -> &ValorExtra {
        &self.valor
    }

    /// É segredo?
    pub fn e_segredo(&self) -> bool {
        matches!(self.valor, ValorExtra::Segredo(_))
    }
}

/// Anexo cifrado no cofre. O conteúdo inteiro é segredo (V2.13). Vira
/// `WARD_ARQUIVO_<NOME>` (D10).
#[derive(Debug)]
pub struct Anexo {
    nome: String,
    conteudo: Segredo,
}

impl Anexo {
    /// Nome do arquivo.
    pub fn nome(&self) -> &str {
        &self.nome
    }

    /// Conteúdo (segredo).
    pub fn conteudo(&self) -> &Segredo {
        &self.conteudo
    }
}

/// De onde vem um valor secreto da entrada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrigemSegredo<'a> {
    /// Campo do tipo (ex.: `"senha"`, `"semente TOTP"`).
    Campo(&'static str),
    /// Campo extra marcado como segredo, pelo nome.
    Extra(&'a str),
    /// Anexo, pelo nome.
    Anexo(&'a str),
}

/// Um valor secreto da entrada e de onde ele vem.
#[derive(Debug)]
pub struct SegredoDaEntrada<'a> {
    pub origem: OrigemSegredo<'a>,
    pub valor: &'a Segredo,
}

/// Datas de criação e de alteração da entrada (D65 d, provisória), em segundos UTC
/// desde 1970.
///
/// - Só o daemon carimba, pelo relógio dele: nem o [`Rascunho`] nem o protocolo do
///   cliente têm data. Nenhuma decisão de segurança usa estas datas (o rollback é D39,
///   pela geração).
/// - Regra (conferida na construção e na leitura do corpo): cada data vai de 1 a
///   [`crate::limites::MAX_DATA`]; com `criada_em`, `alterada_em` existe e não é menor.
///   Uma entrada sem datas (a do `make dev-cofre`, por exemplo) pode ganhar só a de
///   alteração quando é editada: a criação continua desconhecida.
/// - Fora do crate só se lê: o valor nasce de uma [`Entrada`] ou de [`Datas::carimbo`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Datas {
    criada_em: Option<u64>,
    alterada_em: Option<u64>,
}

impl Datas {
    /// Sem nenhuma data.
    pub const NENHUMA: Datas = Datas {
        criada_em: None,
        alterada_em: None,
    };

    /// Um instante válido para carimbar (de 1 a `MAX_DATA`), ou `None`.
    pub fn instante(segundos: u64) -> Option<u64> {
        (1..=crate::limites::MAX_DATA)
            .contains(&segundos)
            .then_some(segundos)
    }

    /// Monta e confere a regra. `None` quando não vale.
    pub(crate) fn de_partes(criada_em: Option<u64>, alterada_em: Option<u64>) -> Option<Datas> {
        let valida = |d: Option<u64>| d.is_none_or(|s| Self::instante(s).is_some());
        let ordem = match (criada_em, alterada_em) {
            (Some(c), Some(a)) => a >= c,
            (Some(_), None) => false,
            (None, _) => true,
        };
        (valida(criada_em) && valida(alterada_em) && ordem).then_some(Datas {
            criada_em,
            alterada_em,
        })
    }

    /// Criação, se conhecida.
    pub fn criada_em(self) -> Option<u64> {
        self.criada_em
    }

    /// Última alteração, se conhecida.
    pub fn alterada_em(self) -> Option<u64> {
        self.alterada_em
    }

    /// As datas de uma entrada nova: as duas em `agora`. `None` com `agora` fora da faixa.
    pub fn carimbo_novo(agora: u64) -> Option<Datas> {
        Self::de_partes(Some(agora), Some(agora))
    }

    /// As datas depois de uma alteração: a criação fica, e `alterada_em` passa a
    /// `agora`, sem nunca descer (um relógio que volta não faz a data voltar). `None`
    /// com `agora` fora da faixa.
    pub fn alterado(self, agora: u64) -> Option<Datas> {
        let agora = Self::instante(agora)?;
        let alterada = [Some(agora), self.alterada_em, self.criada_em]
            .into_iter()
            .flatten()
            .max();
        Self::de_partes(self.criada_em, alterada)
    }
}

/// Entrada validada do cofre. Sem `Clone`; o `Debug` mostra os segredos como `••••`.
#[derive(Debug)]
pub struct Entrada {
    nome: String,
    grupo: String,
    ambiente: Ambiente,
    projeto: Option<String>,
    tags: Vec<String>,
    nivel: NivelAcesso,
    datas: Datas,
    dados: Dados,
    extras: Vec<CampoExtra>,
    anexos: Vec<Anexo>,
}

// Todo valor secreto da entrada está num `Segredo`, que zera ao sair; os demais campos
// não são segredo. A marca vale para a entrada inteira (V2.28).
impl ZeroizeOnDrop for Entrada {}
impl ZeroizeOnDrop for Dados {}
impl ZeroizeOnDrop for CampoExtra {}
impl ZeroizeOnDrop for Anexo {}
impl ZeroizeOnDrop for Autenticacao {}

impl Entrada {
    /// Nome único da entrada (usado em `ward run <entrada>`).
    pub fn nome(&self) -> &str {
        &self.nome
    }

    /// Grupo (pasta), obrigatório.
    pub fn grupo(&self) -> &str {
        &self.grupo
    }

    /// Ambiente, obrigatório.
    pub fn ambiente(&self) -> Ambiente {
        self.ambiente
    }

    /// Projeto, opcional.
    pub fn projeto(&self) -> Option<&str> {
        self.projeto.as_deref()
    }

    /// Tags, opcionais.
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Nível de acesso para o agente.
    pub fn nivel(&self) -> NivelAcesso {
        self.nivel
    }

    /// Troca o nível. **Só o daemon chama, depois de conferir o PIN e o papel app (S9).**
    pub fn definir_nivel(&mut self, nivel: NivelAcesso) {
        self.nivel = nivel;
    }

    /// Datas de criação e alteração (D65 d).
    pub fn datas(&self) -> Datas {
        self.datas
    }

    /// Troca as datas. **Só o daemon chama**, com datas tiradas do relógio dele
    /// ([`Datas::carimbo_novo`], [`Datas::alterado`]) ou com as que a entrada já tinha
    /// (para desfazer uma gravação que falhou). Um [`Datas`] só existe dentro da regra.
    pub fn definir_datas(&mut self, datas: Datas) {
        self.datas = datas;
    }

    /// Tipo.
    pub fn tipo(&self) -> TipoEntrada {
        self.dados.tipo()
    }

    /// Campos próprios do tipo.
    pub fn dados(&self) -> &Dados {
        &self.dados
    }

    /// Campos extras.
    pub fn extras(&self) -> &[CampoExtra] {
        &self.extras
    }

    /// Anexos.
    pub fn anexos(&self) -> &[Anexo] {
        &self.anexos
    }

    /// Todos os valores secretos da entrada (V2.13): os campos secretos do tipo, os
    /// extras marcados como segredo e todos os anexos. É a lista que a máscara (D11)
    /// cobre.
    pub fn segredos(&self) -> Vec<SegredoDaEntrada<'_>> {
        let mut lista = Vec::new();
        segredos_do_tipo(&self.dados, &mut lista);
        for extra in &self.extras {
            if let ValorExtra::Segredo(valor) = &extra.valor {
                lista.push(SegredoDaEntrada {
                    origem: OrigemSegredo::Extra(&extra.nome),
                    valor,
                });
            }
        }
        for anexo in &self.anexos {
            lista.push(SegredoDaEntrada {
                origem: OrigemSegredo::Anexo(&anexo.nome),
                valor: &anexo.conteudo,
            });
        }
        lista
    }
}

fn segredos_do_tipo<'a>(dados: &'a Dados, lista: &mut Vec<SegredoDaEntrada<'a>>) {
    fn campo<'a>(lista: &mut Vec<SegredoDaEntrada<'a>>, nome: &'static str, valor: &'a Segredo) {
        lista.push(SegredoDaEntrada {
            origem: OrigemSegredo::Campo(nome),
            valor,
        });
    }
    fn autenticacao<'a>(lista: &mut Vec<SegredoDaEntrada<'a>>, a: &'a Autenticacao) {
        match a {
            Autenticacao::Senha(senha) => campo(lista, "senha", senha),
            Autenticacao::Chave {
                chave_privada,
                passphrase,
            } => {
                campo(lista, "chave privada", chave_privada);
                if let Some(p) = passphrase {
                    campo(lista, "passphrase", p);
                }
            }
        }
    }
    match dados {
        Dados::Login(l) => {
            campo(lista, "senha", &l.senha);
            if let Some(s) = &l.totp_semente {
                campo(lista, "semente TOTP", s);
            }
        }
        Dados::Banco(b) => campo(lista, "senha", &b.senha),
        Dados::Servidor(s) => autenticacao(lista, &s.autenticacao),
        Dados::Api(a) => campo(lista, "token", &a.token),
        Dados::Aws(a) => {
            campo(lista, "secret access key", &a.secret_access_key);
            if let Some(t) = &a.session_token {
                campo(lista, "session token", t);
            }
        }
        Dados::Smtp(s) => campo(lista, "senha", &s.senha),
        Dados::FtpSftp(f) => autenticacao(lista, &f.autenticacao),
        Dados::NotaSegura(n) => campo(lista, "texto", &n.texto),
        Dados::Generico => {}
    }
}

/// Resumo sem segredo: `nome (tipo, grupo, ambiente)`.
impl fmt::Display for Entrada {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}, {}, {})",
            self.nome,
            self.tipo(),
            self.grupo,
            self.ambiente
        )
    }
}
