//! Validação do que o app (ou a extensão, no "salvar login novo") manda salvar (V2.12).
//!
//! Um [`Rascunho`] é o formulário como chegou: textos crus e segredos já em
//! [`Segredo`]. [`Rascunho::validar`] confere tudo, junta **todos** os erros e só então
//! devolve a [`Entrada`]. Se falhar, o rascunho é descartado e os segredos zeram.
//!
//! Os erros só têm textos fixos (`&'static str`): nenhum valor digitado entra numa
//! mensagem, nem os que não são segredo.

use core::fmt;

use super::{
    Ambiente, Anexo, Api, Autenticacao, Aws, Banco, CampoExtra, Dados, Datas, Entrada, FtpSftp,
    Login, Motor, NivelAcesso, NotaSegura, Protocolo, Segredo, SegurancaSmtp, Servidor, Smtp,
    TipoEntrada, ValorExtra,
};

// Limites (D25, D29): numa fonte só, `crate::limites` (D32). Tudo o que vem do socket
// é não confiável (tamanho, formato).
use crate::limites::{
    MAX_ACCESS_KEY, MAX_ANEXOS, MAX_ANEXOS_SOMADOS, MAX_BASE, MAX_BASE_REDIS, MAX_EXTRAS, MAX_HOST,
    MAX_LINHA, MAX_NOME, MAX_NOME_ANEXO, MAX_NOME_EXTRA, MAX_NOTA, MAX_REGIAO, MAX_SEGREDO,
    MAX_TAG, MAX_TAGS, MAX_TOTP, MAX_URL, MAX_USUARIO, MAX_VALOR_EXTRA, MIN_ACCESS_KEY, MIN_TOTP,
};
use crate::mascara::{self, ErroMascara};

// Mensagens (textos fixos, sem valor do usuário).
const OBRIGATORIO: &str = "obrigatório";
const LONGO: &str = "longo demais";
/// Texto fora da lista branca do campo (D38).
const CARACTERE: &str = "caractere não permitido";
const ESPACO_NAS_PONTAS: &str = "não pode começar nem terminar com espaço";
const NOME_FORMATO: &str =
    "use letras minúsculas, números, '.', '_' ou '-', começando por letra ou número";
const AMBIENTE: &str = "deve ser dev, stage ou prod";
const MOTOR: &str = "fora da lista: postgres, mysql, mongo, redis ou sqlserver";
const PORTA: &str = "deve ser um número de 1 a 65535";
const HOST: &str = "use letras, números, '.', '-', '_', ':' ou colchetes, sem começar por '-'";
const USUARIO: &str = "use só ASCII visível, sem espaço e sem começar por '-'";
const USUARIO_SSH: &str =
    "use letras sem acento, números, '.', '_', '@' ou '-', sem começar por '-'";
const URL: &str = "use https:// (http:// só para localhost ou 127.0.0.1), com host válido em \
     minúsculas, sem usuário na URL, porta de 1 a 65535 e só ASCII visível";
const SENHA_E_CHAVE: &str = "use senha ou chave, não as duas";
const SEM_AUTENTICACAO: &str = "informe a senha ou a chave";
const PASSPHRASE_SEM_CHAVE: &str = "passphrase só vale com chave";
const FTP_COM_CHAVE: &str = "ftp não aceita chave; use sftp ou senha";
const CHAVE_FORMATO: &str =
    "chave privada em formato PEM ou OpenSSH (-----BEGIN … PRIVATE KEY-----)";
const TOTP: &str = "semente TOTP em base32 (A–Z e 2–7), com 16 a 128 caracteres";
const NULO: &str = "não pode ter o caractere nulo";
const NAO_TEXTO: &str = "precisa ser texto (UTF-8)";
const SEGURANCA: &str = "deve ser nenhuma, starttls ou tls";
const PROTOCOLO: &str = "deve ser ftp ou sftp";
const VARIAVEL: &str = "use maiúsculas, números e '_', começando por letra";
const VARIAVEL_SUFIXO: &str =
    "a variável precisa terminar em _TOKEN, _KEY, _SECRET, _PASSWORD, _PAT ou _AUTH";
const VARIAVEL_PROIBIDA: &str =
    "variável reservada (LD_*, WARD_* ou variável que muda o comportamento do sistema)";
const ACCESS_KEY: &str = "use só letras maiúsculas e números (16 a 128)";
const REGIAO: &str = "use letras minúsculas, números e '-' (ex.: sa-east-1)";
const ARN: &str = "deve começar com arn: e ter só ASCII visível, sem espaço";
const TAG: &str = "tag sem espaço nem vírgula, até 32 caracteres";
const TAG_REPETIDA: &str = "tag repetida";
const ITENS_DEMAIS: &str = "itens demais";
const GENERICO_SEM_EXTRAS: &str = "o tipo genérico precisa de pelo menos um campo extra";
const SEM_LETRA: &str = "precisa ter letra ou número";
const EXTRA_REPETIDO: &str = "vira a mesma variável de outro campo extra";
const EXTRA_RESERVADO: &str = "vira uma variável que o tipo já entrega";
const ANEXO_REPETIDO: &str = "vira a mesma variável de outro anexo";
const ANEXO_RESERVADO: &str = "vira uma variável que o tipo já entrega";
const ANEXO_NOME: &str = "nome de arquivo sem '/', sem começar por '.' ou '-'";
const ANEXO_VAZIO: &str = "anexo vazio";
const ANEXOS_SOMADOS: &str = "os anexos somam mais de 256 KiB";
const BASE: &str = "use letras, números, '_' ou '-', com até 63 caracteres";
const BASE_REDIS: &str = "no redis, a base é um número de 0 a 99999";
/// Mensagem de D29 (V2.36).
pub const GRANDE_PARA_MASCARA: &str = "entrada grande demais para a máscara do `ward run`";
const SEM_ENTREGA: &str = "a entrega desta entrada não pôde ser montada";
const TIPO_MUDOU: &str = "o tipo não muda depois de criado";
const NOME_EM_USO: &str = "já existe outra entrada com este nome";
const AUTORREFERENCIA: &str = "aponta para a própria entrada";
const NAO_E_SERVIDOR: &str = "precisa ser uma entrada servidor existente";
const SALTO_COM_SALTO: &str = "o servidor de salto não pode ter salto próprio";
const TUNEL_COM_SALTO: &str = "o servidor do túnel não pode ter salto";
const USADO_COMO_SALTO: &str =
    "outra entrada usa este servidor como salto ou túnel; ele não pode ter salto";

/// Variáveis que o núcleo entrega para cada tipo (sufixo depois de `WARD_`, D10). Um
/// campo extra que normalize para um destes nomes é recusado. A Fase 16 (entrega) deve
/// manter esta lista igual às variáveis `WARD_*` que monta.
pub(crate) fn variaveis_do_tipo(tipo: TipoEntrada) -> &'static [&'static str] {
    match tipo {
        TipoEntrada::Login => &["URL", "USER", "PASS", "TOTP"],
        TipoEntrada::Servidor => &["HOST", "PORT", "USER", "PASS", "PASSPHRASE"],
        TipoEntrada::Api => &["URL", "TOKEN"],
        TipoEntrada::FtpSftp => &["PROTOCOLO", "HOST", "PORT", "USER", "PASS", "PASSPHRASE"],
        TipoEntrada::Banco
        | TipoEntrada::Aws
        | TipoEntrada::Smtp
        | TipoEntrada::NotaSegura
        | TipoEntrada::Generico => &[],
    }
}

/// Variáveis de todo tipo (`WARD_ENTRADA`, `WARD_GRUPO`, `WARD_AMBIENTE`).
pub(crate) const VARIAVEIS_COMUNS: [&str; 3] = ["ENTRADA", "GRUPO", "AMBIENTE"];

/// Anexos que o tipo já entrega (sufixo depois de `WARD_ARQUIVO_`): a chave do
/// `servidor` e do `ftp/sftp` vira `WARD_ARQUIVO_CHAVE`.
pub(crate) fn anexos_do_tipo(tipo: TipoEntrada) -> &'static [&'static str] {
    match tipo {
        TipoEntrada::Servidor | TipoEntrada::FtpSftp => &["CHAVE"],
        _ => &[],
    }
}

// Variáveis que mudam o comportamento do sistema, do shell ou de ferramentas que o
// `ward run`/`ward ssh` chamam. O `api` não pode pôr o token numa delas.
const VARIAVEIS_DO_SISTEMA: &[&str] = &[
    "PATH",
    "HOME",
    "SHELL",
    "USER",
    "LOGNAME",
    "PWD",
    "OLDPWD",
    "IFS",
    "ENV",
    "BASH_ENV",
    "BASHOPTS",
    "SHELLOPTS",
    "PS1",
    "PS2",
    "PS4",
    "PROMPT_COMMAND",
    "CDPATH",
    "GLOBIGNORE",
    "TMPDIR",
    "TERM",
    "LANG",
    "LANGUAGE",
    "SSHPASS",
    "SSH_AUTH_SOCK",
    "SSH_ASKPASS",
    "SSH_ASKPASS_REQUIRE",
    "GIT_SSH",
    "GIT_SSH_COMMAND",
    "GIT_ASKPASS",
    "GIT_EXEC_PATH",
    "GIT_CONFIG",
    "GIT_CONFIG_GLOBAL",
    "PYTHONPATH",
    "PYTHONSTARTUP",
    "PYTHONHOME",
    "NODE_OPTIONS",
    "NODE_PATH",
    "PERL5OPT",
    "PERL5LIB",
    "RUBYOPT",
    "RUBYLIB",
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
];
const PREFIXOS_PROIBIDOS: &[&str] = &["LD_", "WARD_", "BASH_FUNC_", "GCONV_", "MALLOC_"];

/// Nome de variável a partir do nome de um campo extra ou anexo (D10): letras e
/// números ASCII em maiúsculas; o resto vira `_`. Ex.: `api-key` → `API_KEY`,
/// `chave acesso.pem` → `CHAVE_ACESSO_PEM`.
pub fn nome_variavel(nome: &str) -> String {
    nome.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

/// Um problema de validação: campo, posição na lista (campos extras, anexos, tags) e
/// motivo. Só textos fixos; nenhum valor do usuário.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErroValidacao {
    pub campo: &'static str,
    /// Posição (a partir de 1) quando o campo é uma lista.
    pub posicao: Option<usize>,
    pub motivo: &'static str,
}

impl fmt::Display for ErroValidacao {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.posicao {
            Some(n) => write!(f, "{} {}: {}", self.campo, n, self.motivo),
            None => write!(f, "{}: {}", self.campo, self.motivo),
        }
    }
}

/// Todos os problemas encontrados. Nunca vazio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrosValidacao(Vec<ErroValidacao>);

impl ErrosValidacao {
    /// Os problemas, na ordem do formulário.
    pub fn lista(&self) -> &[ErroValidacao] {
        &self.0
    }

    /// Algum problema neste campo?
    pub fn tem(&self, campo: &str) -> bool {
        self.0.iter().any(|e| e.campo == campo)
    }

    /// Algum problema neste campo com este motivo?
    pub fn tem_motivo(&self, campo: &str, motivo: &str) -> bool {
        self.0
            .iter()
            .any(|e| e.campo == campo && e.motivo == motivo)
    }
}

impl fmt::Display for ErrosValidacao {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("entrada inválida: ")?;
        for (i, e) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str("; ")?;
            }
            write!(f, "{e}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ErrosValidacao {}

/// Campo extra como veio do formulário. O valor já chega num [`Segredo`] (mesmo quando
/// não é segredo), para não deixar cópia sem zerar se a validação falhar.
pub struct RascunhoExtra {
    pub nome: String,
    pub valor: Segredo,
    pub segredo: bool,
}

/// Anexo como veio do formulário.
pub struct RascunhoAnexo {
    pub nome: String,
    pub conteudo: Segredo,
}

/// Campos do tipo como vieram do formulário. Texto vazio e segredo vazio contam como
/// "não informado". Porta vazia usa a padrão do motor, protocolo ou segurança.
pub enum RascunhoDados {
    Login {
        url: String,
        usuario: String,
        senha: Option<Segredo>,
        totp_semente: Option<Segredo>,
    },
    Banco {
        motor: String,
        host: String,
        porta: String,
        base: String,
        usuario: String,
        senha: Option<Segredo>,
        tunel: String,
    },
    Servidor {
        host: String,
        porta: String,
        usuario: String,
        senha: Option<Segredo>,
        chave_privada: Option<Segredo>,
        passphrase: Option<Segredo>,
        salto: String,
    },
    Api {
        url: String,
        token: Option<Segredo>,
        variavel: String,
    },
    Aws {
        access_key_id: String,
        secret_access_key: Option<Segredo>,
        regiao: String,
        role_arn: String,
        session_token: Option<Segredo>,
    },
    Smtp {
        host: String,
        porta: String,
        usuario: String,
        senha: Option<Segredo>,
        seguranca: String,
    },
    FtpSftp {
        protocolo: String,
        host: String,
        porta: String,
        usuario: String,
        senha: Option<Segredo>,
        chave_privada: Option<Segredo>,
        passphrase: Option<Segredo>,
    },
    NotaSegura {
        texto: Option<Segredo>,
    },
    Generico,
}

impl RascunhoDados {
    /// O tipo que este rascunho cria.
    pub fn tipo(&self) -> TipoEntrada {
        match self {
            RascunhoDados::Login { .. } => TipoEntrada::Login,
            RascunhoDados::Banco { .. } => TipoEntrada::Banco,
            RascunhoDados::Servidor { .. } => TipoEntrada::Servidor,
            RascunhoDados::Api { .. } => TipoEntrada::Api,
            RascunhoDados::Aws { .. } => TipoEntrada::Aws,
            RascunhoDados::Smtp { .. } => TipoEntrada::Smtp,
            RascunhoDados::FtpSftp { .. } => TipoEntrada::FtpSftp,
            RascunhoDados::NotaSegura { .. } => TipoEntrada::NotaSegura,
            RascunhoDados::Generico => TipoEntrada::Generico,
        }
    }
}

/// O formulário inteiro como chegou. Sem nível: entrada nova sempre começa "com
/// aprovação" (S9), e mudar o nível é outra operação, que pede PIN.
pub struct Rascunho {
    pub nome: String,
    pub grupo: String,
    pub ambiente: String,
    pub projeto: String,
    pub tags: Vec<String>,
    pub dados: RascunhoDados,
    pub extras: Vec<RascunhoExtra>,
    pub anexos: Vec<RascunhoAnexo>,
}

impl fmt::Debug for RascunhoExtra {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RascunhoExtra")
            .field("nome", &self.nome)
            .field("valor", &self.valor)
            .field("segredo", &self.segredo)
            .finish()
    }
}

impl fmt::Debug for RascunhoAnexo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RascunhoAnexo")
            .field("nome", &self.nome)
            .field("conteudo", &self.conteudo)
            .finish()
    }
}

impl fmt::Debug for RascunhoDados {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Só o tipo: os textos crus podem ser qualquer coisa e os segredos já imprimem
        // `••••`, mas não há motivo para logar um formulário.
        write!(f, "RascunhoDados({})", self.tipo().nome())
    }
}

impl fmt::Debug for Rascunho {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rascunho")
            .field("tipo", &self.dados.tipo().nome())
            .finish_non_exhaustive()
    }
}

/// Junta os erros na ordem do formulário.
struct Coletor(Vec<ErroValidacao>);

impl Coletor {
    fn erro(&mut self, campo: &'static str, motivo: &'static str) {
        self.0.push(ErroValidacao {
            campo,
            posicao: None,
            motivo,
        });
    }

    fn erro_em(&mut self, campo: &'static str, indice: usize, motivo: &'static str) {
        self.0.push(ErroValidacao {
            campo,
            posicao: Some(indice + 1),
            motivo,
        });
    }

    /// Texto obrigatório de uma linha.
    fn linha(&mut self, campo: &'static str, valor: String, max: usize) -> Option<String> {
        match motivo_linha(&valor, max) {
            Some(m) => {
                self.erro(campo, m);
                None
            }
            None => Some(valor),
        }
    }

    /// Texto opcional de uma linha (vazio = ausente).
    fn linha_opcional(
        &mut self,
        campo: &'static str,
        valor: String,
        max: usize,
    ) -> Result<Option<String>, ()> {
        if valor.is_empty() {
            return Ok(None);
        }
        self.linha(campo, valor, max).map(Some).ok_or(())
    }

    /// Texto obrigatório que passa por uma regra de formato.
    fn formato(
        &mut self,
        campo: &'static str,
        valor: String,
        max: usize,
        regra: fn(&str) -> bool,
        motivo: &'static str,
    ) -> Option<String> {
        let valor = self.linha(campo, valor, max)?;
        if regra(&valor) {
            Some(valor)
        } else {
            self.erro(campo, motivo);
            None
        }
    }

    /// Opcional com regra de formato.
    fn formato_opcional(
        &mut self,
        campo: &'static str,
        valor: String,
        max: usize,
        regra: fn(&str) -> bool,
        motivo: &'static str,
    ) -> Result<Option<String>, ()> {
        if valor.is_empty() {
            return Ok(None);
        }
        self.formato(campo, valor, max, regra, motivo)
            .map(Some)
            .ok_or(())
    }

    fn host(&mut self, valor: String) -> Option<String> {
        self.formato("host", valor, MAX_HOST, host_valido, HOST)
    }

    /// Usuário de `banco` e `smtp`: campo técnico, só ASCII visível (D38).
    fn usuario_estrito(&mut self, valor: String) -> Option<String> {
        self.formato("usuário", valor, MAX_USUARIO, usuario_valido, USUARIO)
    }

    /// Usuário de `servidor` e `ftp/sftp` (D43): `[A-Za-z0-9._@-]`, sem `-` no começo.
    /// Ele vai para o `ssh`/`sftp`; nada de `%`, `;`, aspas ou espaço.
    fn usuario_ssh(&mut self, valor: String) -> Option<String> {
        self.formato(
            "usuário",
            valor,
            MAX_USUARIO,
            usuario_ssh_valido,
            USUARIO_SSH,
        )
    }

    fn url(&mut self, valor: String) -> Option<String> {
        self.formato("url", valor, MAX_URL, url_valida, URL)
    }

    fn porta(&mut self, valor: &str, padrao: Option<u16>) -> Option<u16> {
        if valor.is_empty() {
            if padrao.is_none() {
                self.erro("porta", OBRIGATORIO);
            }
            return padrao;
        }
        let ok = valor.len() <= 5 && valor.bytes().all(|b| b.is_ascii_digit());
        match valor.parse::<u16>() {
            Ok(p) if ok && p != 0 => Some(p),
            _ => {
                self.erro("porta", PORTA);
                None
            }
        }
    }

    /// Segredo de texto obrigatório (vazio = ausente).
    fn segredo(
        &mut self,
        campo: &'static str,
        valor: Option<Segredo>,
        max: usize,
    ) -> Option<Segredo> {
        match self.segredo_opcional(campo, valor, max) {
            Ok(Some(s)) => Some(s),
            Ok(None) => {
                self.erro(campo, OBRIGATORIO);
                None
            }
            Err(()) => None,
        }
    }

    /// Segredo de texto opcional: texto UTF-8, sem nulo (vai para o ambiente), até `max`.
    fn segredo_opcional(
        &mut self,
        campo: &'static str,
        valor: Option<Segredo>,
        max: usize,
    ) -> Result<Option<Segredo>, ()> {
        let Some(valor) = valor.filter(|s| !s.is_empty()) else {
            return Ok(None);
        };
        match motivo_segredo(&valor, max) {
            Some(m) => {
                self.erro(campo, m);
                Err(())
            }
            None => Ok(Some(valor)),
        }
    }

    /// Senha **ou** chave (+ passphrase), V2.12.
    fn autenticacao(
        &mut self,
        senha: Option<Segredo>,
        chave: Option<Segredo>,
        passphrase: Option<Segredo>,
        aceita_chave: bool,
    ) -> Option<Autenticacao> {
        let senha = self.segredo_opcional("senha", senha, MAX_SEGREDO);
        let chave = self.segredo_opcional("chave privada", chave, MAX_SEGREDO);
        let passphrase = self.segredo_opcional("passphrase", passphrase, MAX_SEGREDO);
        let (Ok(senha), Ok(chave), Ok(passphrase)) = (senha, chave, passphrase) else {
            return None;
        };
        match (senha, chave) {
            (Some(_), Some(_)) => {
                self.erro("autenticação", SENHA_E_CHAVE);
                None
            }
            (None, None) => {
                self.erro("autenticação", SEM_AUTENTICACAO);
                None
            }
            (Some(senha), None) => {
                if passphrase.is_some() {
                    self.erro("passphrase", PASSPHRASE_SEM_CHAVE);
                    return None;
                }
                Some(Autenticacao::Senha(senha))
            }
            (None, Some(chave)) => {
                if !aceita_chave {
                    self.erro("autenticação", FTP_COM_CHAVE);
                    return None;
                }
                if !chave_parece_privada(&chave) {
                    self.erro("chave privada", CHAVE_FORMATO);
                    return None;
                }
                Some(Autenticacao::Chave {
                    chave_privada: chave,
                    passphrase,
                })
            }
        }
    }
}

impl Rascunho {
    /// Confere o formulário inteiro. Devolve a entrada com nível "com aprovação" ou
    /// todos os problemas encontrados.
    pub fn validar(self) -> Result<Entrada, ErrosValidacao> {
        let mut c = Coletor(Vec::new());
        let tipo = self.dados.tipo();

        let nome = c.formato(
            "nome",
            self.nome,
            MAX_NOME,
            nome_entrada_valido,
            NOME_FORMATO,
        );
        let grupo = c.linha("grupo", self.grupo, MAX_LINHA);
        let ambiente = if self.ambiente.is_empty() {
            c.erro("ambiente", OBRIGATORIO);
            None
        } else {
            let a = Ambiente::de_nome(&self.ambiente);
            if a.is_none() {
                c.erro("ambiente", AMBIENTE);
            }
            a
        };
        let projeto = c.linha_opcional("projeto", self.projeto, MAX_LINHA);
        let tags = validar_tags(&mut c, self.tags);
        let dados = validar_dados(&mut c, self.dados);
        let extras = validar_extras(&mut c, tipo, self.extras);
        let anexos = validar_anexos(&mut c, tipo, self.anexos);
        if let Some(extras) = &extras
            && tipo == TipoEntrada::Generico
            && extras.is_empty()
        {
            c.erro("campos extras", GENERICO_SEM_EXTRAS);
        }

        match (nome, grupo, ambiente, projeto, tags, dados, extras, anexos) {
            (
                Some(nome),
                Some(grupo),
                Some(ambiente),
                Ok(projeto),
                Some(tags),
                Some(dados),
                Some(extras),
                Some(anexos),
            ) if c.0.is_empty() => {
                let entrada = Entrada {
                    nome,
                    grupo,
                    ambiente,
                    projeto,
                    tags,
                    nivel: NivelAcesso::ComAprovacao,
                    // D65 d: só o daemon carimba (relógio dele); o rascunho não tem data.
                    datas: Datas::NENHUMA,
                    dados,
                    extras,
                    anexos,
                };
                // Orçamento da máscara (D29), pela mesma função que monta o autômato:
                // nenhuma entrada aceita fica sem `ward run`. Se recusar, a entrada é
                // descartada aqui e os segredos zeram.
                match mascara::custo_da_entrada(&entrada) {
                    Ok(_) => Ok(entrada),
                    Err(e) => {
                        let motivo = match e {
                            ErroMascara::GrandeDemais => GRANDE_PARA_MASCARA,
                            ErroMascara::SemEntrega => SEM_ENTREGA,
                        };
                        c.erro("entrada", motivo);
                        Err(ErrosValidacao(c.0))
                    }
                }
            }
            _ => {
                debug_assert!(!c.0.is_empty());
                Err(ErrosValidacao(c.0))
            }
        }
    }
}

fn validar_tags(c: &mut Coletor, tags: Vec<String>) -> Option<Vec<String>> {
    if tags.len() > MAX_TAGS {
        c.erro("tags", ITENS_DEMAIS);
        return None;
    }
    let antes = c.0.len();
    let mut saida: Vec<String> = Vec::with_capacity(tags.len());
    for (i, tag) in tags.into_iter().enumerate() {
        if let Some(m) = motivo_linha(&tag, MAX_TAG) {
            c.erro_em("tags", i, m);
        } else if tag.chars().any(|ch| ch.is_whitespace() || ch == ',') {
            c.erro_em("tags", i, TAG);
        } else if saida.contains(&tag) {
            c.erro_em("tags", i, TAG_REPETIDA);
        } else {
            saida.push(tag);
        }
    }
    (c.0.len() == antes).then_some(saida)
}

fn validar_dados(c: &mut Coletor, dados: RascunhoDados) -> Option<Dados> {
    match dados {
        RascunhoDados::Login {
            url,
            usuario,
            senha,
            totp_semente,
        } => {
            let url = c.url(url);
            let usuario = c.linha("usuário", usuario, MAX_USUARIO);
            let senha = c.segredo("senha", senha, MAX_SEGREDO);
            let totp = c.segredo_opcional("semente TOTP", totp_semente, MAX_TOTP);
            let totp = match totp {
                Ok(Some(s)) if !semente_totp_valida(&s) => {
                    c.erro("semente TOTP", TOTP);
                    Err(())
                }
                outro => outro,
            };
            Some(Dados::Login(Login {
                url: url?,
                usuario: usuario?,
                senha: senha?,
                totp_semente: totp.ok()?,
            }))
        }
        RascunhoDados::Banco {
            motor,
            host,
            porta,
            base,
            usuario,
            senha,
            tunel,
        } => {
            let motor = if motor.is_empty() {
                c.erro("motor", OBRIGATORIO);
                None
            } else {
                let m = Motor::de_nome(&motor);
                if m.is_none() {
                    c.erro("motor", MOTOR);
                }
                m
            };
            let host = c.host(host);
            let porta = c.porta(&porta, motor.map(Motor::porta_padrao).or(Some(1)));
            let base = if motor == Some(Motor::Redis) {
                c.formato_opcional("base", base, MAX_BASE, base_redis_valida, BASE_REDIS)
            } else {
                c.formato_opcional("base", base, MAX_BASE, base_valida, BASE)
            };
            let usuario = c.usuario_estrito(usuario);
            let senha = c.segredo("senha", senha, MAX_SEGREDO);
            let tunel = c.formato_opcional(
                "túnel SSH",
                tunel,
                MAX_NOME,
                nome_entrada_valido,
                NOME_FORMATO,
            );
            Some(Dados::Banco(Banco {
                motor: motor?,
                host: host?,
                porta: porta?,
                base: base.ok()?,
                usuario: usuario?,
                senha: senha?,
                tunel: tunel.ok()?,
            }))
        }
        RascunhoDados::Servidor {
            host,
            porta,
            usuario,
            senha,
            chave_privada,
            passphrase,
            salto,
        } => {
            let host = c.host(host);
            let porta = c.porta(&porta, Some(22));
            let usuario = c.usuario_ssh(usuario);
            let autenticacao = c.autenticacao(senha, chave_privada, passphrase, true);
            let salto = c.formato_opcional(
                "servidor de salto",
                salto,
                MAX_NOME,
                nome_entrada_valido,
                NOME_FORMATO,
            );
            Some(Dados::Servidor(Servidor {
                host: host?,
                porta: porta?,
                usuario: usuario?,
                autenticacao: autenticacao?,
                salto: salto.ok()?,
            }))
        }
        RascunhoDados::Api {
            url,
            token,
            variavel,
        } => {
            let url = if url.is_empty() {
                Ok(None)
            } else {
                c.url(url).map(Some).ok_or(())
            };
            let token = c.segredo("token", token, MAX_SEGREDO);
            let variavel =
                c.formato_opcional("variável", variavel, MAX_NOME, variavel_valida, VARIAVEL);
            // D42: lista branca de sufixo primeiro; a lista de recusa de D25 fica como
            // segunda camada.
            let variavel = match variavel {
                Ok(Some(v)) if !variavel_com_sufixo(&v) => {
                    c.erro("variável", VARIAVEL_SUFIXO);
                    Err(())
                }
                Ok(Some(v)) if variavel_proibida(&v) => {
                    c.erro("variável", VARIAVEL_PROIBIDA);
                    Err(())
                }
                outro => outro,
            };
            Some(Dados::Api(Api {
                url: url.ok()?,
                token: token?,
                variavel: variavel.ok()?,
            }))
        }
        RascunhoDados::Aws {
            access_key_id,
            secret_access_key,
            regiao,
            role_arn,
            session_token,
        } => {
            let access_key_id = c.formato(
                "access key ID",
                access_key_id,
                MAX_ACCESS_KEY,
                access_key_valida,
                ACCESS_KEY,
            );
            let secret = c.segredo("secret access key", secret_access_key, MAX_SEGREDO);
            let regiao = c.formato("região", regiao, MAX_REGIAO, regiao_valida, REGIAO);
            let role_arn = c.formato_opcional("role ARN", role_arn, MAX_URL, arn_valido, ARN);
            let session = c.segredo_opcional("session token", session_token, MAX_SEGREDO);
            Some(Dados::Aws(Aws {
                access_key_id: access_key_id?,
                secret_access_key: secret?,
                regiao: regiao?,
                role_arn: role_arn.ok()?,
                session_token: session.ok()?,
            }))
        }
        RascunhoDados::Smtp {
            host,
            porta,
            usuario,
            senha,
            seguranca,
        } => {
            let host = c.host(host);
            let seguranca = if seguranca.is_empty() {
                c.erro("segurança", OBRIGATORIO);
                None
            } else {
                let s = SegurancaSmtp::de_nome(&seguranca);
                if s.is_none() {
                    c.erro("segurança", SEGURANCA);
                }
                s
            };
            let porta = c.porta(
                &porta,
                seguranca.map(SegurancaSmtp::porta_padrao).or(Some(1)),
            );
            let usuario = c.usuario_estrito(usuario);
            let senha = c.segredo("senha", senha, MAX_SEGREDO);
            Some(Dados::Smtp(Smtp {
                host: host?,
                porta: porta?,
                usuario: usuario?,
                senha: senha?,
                seguranca: seguranca?,
            }))
        }
        RascunhoDados::FtpSftp {
            protocolo,
            host,
            porta,
            usuario,
            senha,
            chave_privada,
            passphrase,
        } => {
            let protocolo = if protocolo.is_empty() {
                c.erro("protocolo", OBRIGATORIO);
                None
            } else {
                let p = Protocolo::de_nome(&protocolo);
                if p.is_none() {
                    c.erro("protocolo", PROTOCOLO);
                }
                p
            };
            let host = c.host(host);
            let porta = c.porta(&porta, protocolo.map(Protocolo::porta_padrao).or(Some(1)));
            let usuario = c.usuario_ssh(usuario);
            // Sem protocolo válido, não dá para saber se a chave vale: aceita aqui e o
            // erro do protocolo já recusa a entrada.
            let aceita_chave = protocolo != Some(Protocolo::Ftp);
            let autenticacao = c.autenticacao(senha, chave_privada, passphrase, aceita_chave);
            Some(Dados::FtpSftp(FtpSftp {
                protocolo: protocolo?,
                host: host?,
                porta: porta?,
                usuario: usuario?,
                autenticacao: autenticacao?,
            }))
        }
        RascunhoDados::NotaSegura { texto } => {
            let texto = c.segredo("texto", texto, MAX_NOTA);
            Some(Dados::NotaSegura(NotaSegura { texto: texto? }))
        }
        RascunhoDados::Generico => Some(Dados::Generico),
    }
}

fn validar_extras(
    c: &mut Coletor,
    tipo: TipoEntrada,
    extras: Vec<RascunhoExtra>,
) -> Option<Vec<CampoExtra>> {
    const CAMPO: &str = "campos extras";
    if extras.len() > MAX_EXTRAS {
        c.erro(CAMPO, ITENS_DEMAIS);
        return None;
    }
    let antes = c.0.len();
    let mut vistos: Vec<String> = Vec::with_capacity(extras.len());
    let mut saida = Vec::with_capacity(extras.len());
    for (i, extra) in extras.into_iter().enumerate() {
        let RascunhoExtra {
            nome,
            valor,
            segredo,
        } = extra;
        if let Some(m) = motivo_linha(&nome, MAX_NOME_EXTRA) {
            c.erro_em(CAMPO, i, m);
            continue;
        }
        let var = nome_variavel(&nome);
        if var.bytes().all(|b| b == b'_') {
            c.erro_em(CAMPO, i, SEM_LETRA);
            continue;
        }
        if VARIAVEIS_COMUNS.contains(&var.as_str())
            || variaveis_do_tipo(tipo).contains(&var.as_str())
            || var == "ARQUIVO"
            || var.starts_with("ARQUIVO_")
        {
            c.erro_em(CAMPO, i, EXTRA_RESERVADO);
            continue;
        }
        if vistos.contains(&var) {
            c.erro_em(CAMPO, i, EXTRA_REPETIDO);
            continue;
        }
        vistos.push(var);
        if valor.is_empty() {
            c.erro_em(CAMPO, i, OBRIGATORIO);
            continue;
        }
        if let Some(m) = motivo_segredo(&valor, MAX_VALOR_EXTRA) {
            c.erro_em(CAMPO, i, m);
            continue;
        }
        let valor = if segredo {
            ValorExtra::Segredo(valor)
        } else {
            // Não é segredo: vira texto comum, de uma linha (aparece na lista e no app).
            let texto = valor.expor_texto().unwrap_or_default();
            if let Some(m) = motivo_linha(texto, MAX_VALOR_EXTRA) {
                c.erro_em(CAMPO, i, m);
                continue;
            }
            ValorExtra::Texto(texto.to_owned())
        };
        saida.push(CampoExtra { nome, valor });
    }
    (c.0.len() == antes).then_some(saida)
}

fn validar_anexos(
    c: &mut Coletor,
    tipo: TipoEntrada,
    anexos: Vec<RascunhoAnexo>,
) -> Option<Vec<Anexo>> {
    const CAMPO: &str = "anexos";
    if anexos.len() > MAX_ANEXOS {
        c.erro(CAMPO, ITENS_DEMAIS);
        return None;
    }
    let antes = c.0.len();
    let mut vistos: Vec<String> = Vec::with_capacity(anexos.len());
    let mut saida = Vec::with_capacity(anexos.len());
    // Soma dos anexos (D29). Cada anexo também fica até o limite da soma.
    let mut soma: usize = 0;
    for (i, anexo) in anexos.into_iter().enumerate() {
        let RascunhoAnexo { nome, conteudo } = anexo;
        if let Some(m) = motivo_linha(&nome, MAX_NOME_ANEXO) {
            c.erro_em(CAMPO, i, m);
            continue;
        }
        if nome.contains(['/', '\\']) || nome.starts_with(['.', '-']) {
            c.erro_em(CAMPO, i, ANEXO_NOME);
            continue;
        }
        let var = nome_variavel(&nome);
        if var.bytes().all(|b| b == b'_') {
            c.erro_em(CAMPO, i, SEM_LETRA);
            continue;
        }
        if anexos_do_tipo(tipo).contains(&var.as_str()) {
            c.erro_em(CAMPO, i, ANEXO_RESERVADO);
            continue;
        }
        if vistos.contains(&var) {
            c.erro_em(CAMPO, i, ANEXO_REPETIDO);
            continue;
        }
        vistos.push(var);
        if conteudo.is_empty() {
            c.erro_em(CAMPO, i, ANEXO_VAZIO);
            continue;
        }
        if conteudo.len() > MAX_ANEXOS_SOMADOS {
            c.erro_em(CAMPO, i, LONGO);
            continue;
        }
        soma = soma.saturating_add(conteudo.len());
        saida.push(Anexo { nome, conteudo });
    }
    if soma > MAX_ANEXOS_SOMADOS {
        c.erro(CAMPO, ANEXOS_SOMADOS);
    }
    (c.0.len() == antes).then_some(saida)
}

impl Entrada {
    /// Troca o conteúdo pela versão editada (já validada), mantendo o nível: editar não
    /// muda o nível (S9). O tipo não muda depois de criado.
    pub fn substituir_por(&mut self, editada: Entrada) -> Result<(), ErrosValidacao> {
        if editada.tipo() != self.tipo() {
            return Err(ErrosValidacao(vec![ErroValidacao {
                campo: "tipo",
                posicao: None,
                motivo: TIPO_MUDOU,
            }]));
        }
        let nivel = self.nivel;
        *self = editada;
        self.nivel = nivel;
        Ok(())
    }

    /// Nome da outra entrada que esta usa: o salto do `servidor` ou o túnel do `banco`.
    pub fn referencia(&self) -> Option<&str> {
        match &self.dados {
            Dados::Servidor(s) => s.salto.as_deref(),
            Dados::Banco(b) => b.tunel.as_deref(),
            _ => None,
        }
    }

    fn salto(&self) -> Option<&str> {
        match &self.dados {
            Dados::Servidor(s) => s.salto.as_deref(),
            _ => None,
        }
    }

    /// Confere a entrada contra as outras do cofre (todas menos ela mesma): nome único,
    /// túnel e salto apontando para um `servidor` que existe e sem cadeia (um uso
    /// envolve no máximo duas entradas, D19).
    ///
    /// `outras` é percorrido mais de uma vez (por isso `Clone`), por exemplo
    /// `entradas.iter().filter(|o| o.nome() != nome_antigo)`.
    pub fn conferir_no_cofre<'a, I>(&self, outras: I) -> Result<(), ErrosValidacao>
    where
        I: IntoIterator<Item = &'a Entrada>,
        I::IntoIter: Clone,
    {
        let outras = outras.into_iter();
        let mut c = Coletor(Vec::new());
        if outras.clone().any(|o| o.nome == self.nome) {
            c.erro("nome", NOME_EM_USO);
        }
        if let Some(alvo) = self.referencia() {
            let campo = if self.tipo() == TipoEntrada::Banco {
                "túnel SSH"
            } else {
                "servidor de salto"
            };
            if alvo == self.nome {
                c.erro(campo, AUTORREFERENCIA);
            } else {
                match outras.clone().find(|o| o.nome == alvo) {
                    Some(o) if o.tipo() == TipoEntrada::Servidor => {
                        if o.salto().is_some() {
                            let m = if self.tipo() == TipoEntrada::Banco {
                                TUNEL_COM_SALTO
                            } else {
                                SALTO_COM_SALTO
                            };
                            c.erro(campo, m);
                        }
                    }
                    _ => c.erro(campo, NAO_E_SERVIDOR),
                }
            }
        }
        if self.salto().is_some() && outras.clone().any(|o| o.referencia() == Some(&self.nome)) {
            c.erro("servidor de salto", USADO_COMO_SALTO);
        }
        if c.0.is_empty() {
            Ok(())
        } else {
            Err(ErrosValidacao(c.0))
        }
    }
}

// ---- Regras de formato (sem valor do usuário nas mensagens) ----

/// Lista branca do texto de exibição (D38): `grupo`, `projeto`, tags, nome de extra e de
/// anexo, `usuário` do `login` e valor de extra que não é segredo. Só ASCII visível,
/// espaço (as pontas são conferidas à parte) e as letras latinas pré-compostas de
/// U+00C0 a U+017F, menos `×` e `÷`. Fica de fora todo o resto: controle, formatação
/// (bidi, largura zero, Cf), preenchedores, seletores de variante, marca combinante,
/// espaço fora do ASCII, emoji, Braille. Esses caracteres forjariam o texto exibido na
/// janela de aprovação e na lista. Os campos técnicos (host, URL, variável…) passam por
/// aqui e depois pelo formato próprio, que só aceita ASCII.
fn caractere_de_exibicao(c: char) -> bool {
    matches!(c, ' '..='~' | '\u{C0}'..='\u{17F}') && c != '\u{D7}' && c != '\u{F7}'
}

/// Motivo de recusa de um texto de uma linha obrigatório, ou `None` se está bom.
fn motivo_linha(valor: &str, max: usize) -> Option<&'static str> {
    if valor.is_empty() {
        Some(OBRIGATORIO)
    } else if valor.len() > max {
        Some(LONGO)
    } else if !valor.chars().all(caractere_de_exibicao) {
        Some(CARACTERE)
    } else if valor.trim() != valor {
        Some(ESPACO_NAS_PONTAS)
    } else {
        None
    }
}

/// Motivo de recusa de um segredo de texto (já não vazio).
fn motivo_segredo(valor: &Segredo, max: usize) -> Option<&'static str> {
    if valor.len() > max {
        Some(LONGO)
    } else if valor.expor_texto().is_none() {
        Some(NAO_TEXTO)
    } else if valor.expor().contains(&0) {
        Some(NULO)
    } else {
        None
    }
}

fn nome_entrada_valido(v: &str) -> bool {
    let mut b = v.bytes();
    b.next()
        .is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit())
        && b.all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
        })
}

/// Base do `banco` (D33): `[A-Za-z0-9_-]`, de 1 a [`MAX_BASE`] (o comprimento já foi
/// conferido). Sem `?`, `#`, `/`, `.`, espaço nem `%`: `PGDATABASE` e as demais recebem
/// sempre um nome limpo.
fn base_valida(v: &str) -> bool {
    !v.is_empty()
        && v.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
}

/// Base do `redis` (D33): número de 0 a [`MAX_BASE_REDIS`], só dígitos ASCII e sem zero
/// à esquerda (uma forma só para cada número).
fn base_redis_valida(v: &str) -> bool {
    !v.is_empty()
        && v.bytes().all(|c| c.is_ascii_digit())
        && (v == "0" || !v.starts_with('0'))
        && v.parse::<u32>().is_ok_and(|n| n <= MAX_BASE_REDIS)
}

fn host_valido(v: &str) -> bool {
    !v.starts_with('-')
        && v.bytes().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_' | b':' | b'[' | b']')
        })
}

/// ASCII visível (U+0021 a U+007E), sem espaço.
fn ascii_visivel(v: &str) -> bool {
    v.bytes().all(|c| c.is_ascii_graphic())
}

/// Usuário de `banco` e `smtp` (D38): só ASCII visível (o do smtp costuma ser um
/// e-mail), sem `-` no começo.
fn usuario_valido(v: &str) -> bool {
    !v.starts_with('-') && ascii_visivel(v)
}

/// Usuário de `servidor` e `ftp/sftp` (D43): `[A-Za-z0-9._@-]`, sem `-` no começo. O
/// `@` vale para conta de domínio (`deploy@cliente.com.br`): a entrega (Fase 27) passa
/// o usuário só por `-l`, com o host depois de `--`, nunca como `usuário@host`. Vale
/// também para o do salto, que é outro `servidor`.
fn usuario_ssh_valido(v: &str) -> bool {
    !v.is_empty()
        && !v.starts_with('-')
        && v.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'@' | b'-'))
}

/// URL do `login` e do `api` por parser estrito (D41, D9), sem dependência nova:
/// - `https://`, ou `http://` só com host `localhost` ou `127.0.0.1` (o esquema em
///   minúsculas: a URL chega normalizada pelo app);
/// - só ASCII visível;
/// - autoridade sem `@`, `\` nem `%` (nada de userinfo, nem disfarçado);
/// - host: domínio em minúsculas (rótulos `[a-z0-9-]` de 1 a 63, sem `-` nas pontas,
///   até 253 no total; nome internacional só em punycode `xn--`), IPv4 de 4 octetos sem
///   zero à esquerda, ou IPv6 entre colchetes (os dois lidos por `std::net`). Host só de
///   dígitos e pontos é sempre lido como IPv4; domínio cujo último rótulo começa por
///   dígito (`0x7f000001`, `1.2.3.0x4`, `x.123`) é recusado (D55);
/// - porta opcional de 1 a 65535, sem zero à esquerda;
/// - caminho, query e fragmento sem `\`, e todo `%` seguido de dois dígitos hex.
fn url_valida(v: &str) -> bool {
    if !ascii_visivel(v) {
        return false;
    }
    let (https, resto) = if let Some(r) = v.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = v.strip_prefix("http://") {
        (false, r)
    } else {
        return false;
    };
    let fim = resto.find(['/', '?', '#']).unwrap_or(resto.len());
    let (autoridade, depois) = resto.split_at(fim);
    if autoridade.contains(['@', '\\', '%']) {
        return false;
    }
    let Some((host, porta)) = separar_host_porta(autoridade) else {
        return false;
    };
    if !host_de_url_valido(host) || !porta.is_none_or(porta_de_url_valida) {
        return false;
    }
    if !https && host != "localhost" && host != "127.0.0.1" {
        return false;
    }
    caminho_de_url_valido(depois)
}

/// Origem de uma URL (D9): `esquema://host[:porta]`, sem a porta padrão do esquema
/// (80 no `http`, 443 no `https`), pelo mesmo parser estrito de D41. `None` para
/// qualquer URL que o parser recusa (inclusive `http` fora de `localhost`/`127.0.0.1`),
/// e quem compara origens trata `None` como origem diferente. O host já vem em
/// minúsculas e em punycode (o parser recusa o resto), então duas origens iguais têm o
/// mesmo texto. Subdomínio é outra origem.
pub fn origem_da_url(v: &str) -> Option<String> {
    if !url_valida(v) {
        return None;
    }
    let (esquema, resto) = v.split_once("://")?;
    let fim = resto.find(['/', '?', '#']).unwrap_or(resto.len());
    let (host, porta) = separar_host_porta(&resto[..fim])?;
    let padrao = if esquema == "https" { "443" } else { "80" };
    Some(match porta {
        Some(p) if p != padrao => format!("{esquema}://{host}:{p}"),
        _ => format!("{esquema}://{host}"),
    })
}

/// `host` e `porta` (sem os `:`) da autoridade; IPv6 vem com os colchetes.
fn separar_host_porta(autoridade: &str) -> Option<(&str, Option<&str>)> {
    if autoridade.starts_with('[') {
        let fecha = autoridade.find(']')?;
        let (host, resto) = autoridade.split_at(fecha + 1);
        match resto.strip_prefix(':') {
            Some(p) => Some((host, Some(p))),
            None if resto.is_empty() => Some((host, None)),
            None => None,
        }
    } else {
        match autoridade.split_once(':') {
            Some((h, p)) => Some((h, Some(p))),
            None => Some((autoridade, None)),
        }
    }
}

fn host_de_url_valido(host: &str) -> bool {
    if let Some(dentro) = host.strip_prefix('[') {
        // IPv6 entre colchetes, em minúsculas, lido por `std::net`.
        return dentro.strip_suffix(']').is_some_and(|ip| {
            !ip.bytes().any(|c| c.is_ascii_uppercase()) && ip.parse::<std::net::Ipv6Addr>().is_ok()
        });
    }
    if host.bytes().all(|c| c.is_ascii_digit() || c == b'.') {
        // Só dígitos e pontos: IPv4, nunca domínio. `std::net` exige 4 octetos e recusa
        // zero à esquerda; a conferência abaixo repete a regra por garantia.
        return host.parse::<std::net::Ipv4Addr>().is_ok()
            && host.split('.').all(|o| o.len() == 1 || !o.starts_with('0'));
    }
    // D55: o último rótulo de um domínio não começa por dígito. O Chrome e o `curl`
    // leem `0x7f000001`, `0x7f.0.0.1` e `1.2.3.0x4` como IPv4 (formas hex e numéricas
    // do WHATWG), e `x.123` não é domínio. Isso inclui todo rótulo final `0x…`.
    if host
        .rsplit('.')
        .next()
        .is_none_or(|ultimo| ultimo.starts_with(|c: char| c.is_ascii_digit()))
    {
        return false;
    }
    host.len() <= MAX_HOST
        && host.split('.').all(|rotulo| {
            (1..=63).contains(&rotulo.len())
                && !rotulo.starts_with('-')
                && !rotulo.ends_with('-')
                && rotulo
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        })
}

fn porta_de_url_valida(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 5
        && !p.starts_with('0')
        && p.bytes().all(|c| c.is_ascii_digit())
        && p.parse::<u16>().is_ok_and(|n| n != 0)
}

/// Caminho, query e fragmento (já em ASCII visível): sem `\`, e todo `%` com dois
/// dígitos hex.
fn caminho_de_url_valido(depois: &str) -> bool {
    let b = depois.as_bytes();
    !b.contains(&b'\\')
        && b.iter().enumerate().all(|(i, &c)| {
            c != b'%'
                || (b.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
                    && b.get(i + 2).is_some_and(u8::is_ascii_hexdigit))
        })
}

fn variavel_valida(v: &str) -> bool {
    let mut b = v.bytes();
    b.next().is_some_and(|p| p.is_ascii_uppercase())
        && b.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
}

/// Sufixos aceitos na variável do `api` (D42): o nome diz que guarda credencial.
const SUFIXOS_DE_CREDENCIAL: &[&str] = &["_TOKEN", "_KEY", "_SECRET", "_PASSWORD", "_PAT", "_AUTH"];

/// Lista branca de D42: termina num sufixo de credencial (e tem algo antes dele).
fn variavel_com_sufixo(v: &str) -> bool {
    SUFIXOS_DE_CREDENCIAL
        .iter()
        .any(|s| v.len() > s.len() && v.ends_with(s))
}

/// Segunda camada (D25): variáveis do sistema e prefixos reservados, e todo nome com
/// `ASKPASS` (o `ssh`, o `git` e o `sudo` executam o programa que ela aponta).
fn variavel_proibida(v: &str) -> bool {
    VARIAVEIS_DO_SISTEMA.contains(&v)
        || PREFIXOS_PROIBIDOS.iter().any(|p| v.starts_with(p))
        || v.contains("ASKPASS")
}

fn access_key_valida(v: &str) -> bool {
    (MIN_ACCESS_KEY..=MAX_ACCESS_KEY).contains(&v.len())
        && v.bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

fn regiao_valida(v: &str) -> bool {
    !v.starts_with('-')
        && v.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

fn arn_valido(v: &str) -> bool {
    v.starts_with("arn:") && ascii_visivel(v)
}

/// Base32 do RFC 4648 em maiúsculas, `=` só no fim, pelo menos 16 caracteres úteis.
fn semente_totp_valida(s: &Segredo) -> bool {
    let b = s.expor();
    let uteis = b
        .iter()
        .take_while(|c| c.is_ascii_uppercase() || (b'2'..=b'7').contains(c))
        .count();
    let resto = &b[uteis..];
    uteis >= MIN_TOTP && resto.iter().all(|&c| c == b'=')
}

/// Começa com `-----BEGIN ` e tem `PRIVATE KEY-----` (PEM ou OpenSSH). Não decodifica:
/// quem lê a chave é o `ssh`/agente.
fn chave_parece_privada(s: &Segredo) -> bool {
    s.expor_texto().is_some_and(|t| {
        let t = t.trim_start();
        t.starts_with("-----BEGIN ") && t.contains("PRIVATE KEY-----")
    })
}
