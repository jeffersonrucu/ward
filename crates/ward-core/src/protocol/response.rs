//! Respostas e avisos do daemon ao cliente: os campos de cada tipo, a escrita (lado do
//! daemon) e a leitura estrita (lado do cliente).
//!
//! Os tipos que levam credencial ([`super::ResponseType::carries_credential`]) são só de
//! app ou de extensão; [`super::encode_response`] confere o papel antes de escrever.
//! [`ItemLista`] e [`DetalheEntrada`] são montados da [`Entry`] e por construção não
//! têm segredo: o extra secreto vai sem valor, e dos segredos do tipo vai só o nome do
//! campo preenchido.

use zeroize::Zeroizing;

use super::errors::{Problem, ResponseError};
use super::fields::{Fields, R, secret_of, text_of, texts_of};
use super::json::{InvalidJson, Writer};
use super::request::SECRET_FIELDS;
use super::{Operation, ResponseType, Role};
use crate::entry::{
    AccessLevel, Auth, Data, Dates, Entry, EntryType, Environment, ExtraValue, Secret,
};
use crate::limits::{
    MAX_ATTACHMENT_NAME, MAX_ATTACHMENTS, MAX_ATTACHMENTS_TOTAL, MAX_DISPLAYED_TEXT, MAX_ENTRIES,
    MAX_EXTRA_NAME, MAX_EXTRA_VALUE, MAX_EXTRAS, MAX_LINE, MAX_LOCK_MINUTES, MAX_NAME, MAX_SECRET,
    MAX_TAG, MAX_TAGS, MAX_TEXT, MAX_URL, MIN_LOCK_MINUTES,
};

/// Fluxo de um pedaço de saída.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fluxo {
    Saida,
    Erro,
}

impl Fluxo {
    fn nome(self) -> &'static str {
        match self {
            Fluxo::Saida => "saida",
            Fluxo::Erro => "erro",
        }
    }
}

/// Um item da `lista`: o que o agente pode ver ("Comandos": nome, tipo, grupo,
/// ambiente, projeto, URL/host), mais tags; o nível só vai ao app (D50).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemLista {
    pub nome: String,
    pub tipo: EntryType,
    pub grupo: String,
    pub ambiente: Environment,
    pub projeto: Option<String>,
    pub tags: Vec<String>,
    /// Nível de acesso: `Some` só na lista do app. A do agente vai sem ele (D50):
    /// [`super::encode_response`] recusa mandar ao agente um item com nível, e a
    /// leitura do agente recusa um item que o traga.
    pub nivel: Option<AccessLevel>,
    /// URL (`login`, `api`) ou host (`banco`, `servidor`, `smtp`, `ftp/sftp`).
    pub endereco: Option<String>,
    /// Resumo para a 2ª linha e a busca do app (D65 c): `Some` só na lista do app, no
    /// mesmo padrão do nível (D50). [`super::encode_response`] recusa mandar ao
    /// agente um item com resumo, e a leitura do agente recusa um item que o traga.
    pub resumo: Option<ResumoItem>,
}

/// O que a lista do app leva além do cabeçalho (D65 c), para a tela montar a 2ª linha
/// e a busca por usuário sem um `detalhe` por entrada. Sem segredo por construção: os
/// mesmos campos de texto do [`DetalheEntrada`], só o identificador dos segredos
/// preenchidos e a contagem de extras e anexos (nem o nome deles).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumoItem {
    /// Campos de texto do tipo preenchidos (identificador do protocolo, valor).
    pub campos: Vec<(String, String)>,
    /// Campos secretos do tipo preenchidos (só o identificador).
    pub segredos: Vec<String>,
    /// Número de campos extras.
    pub extras: u64,
    /// Número de anexos.
    pub anexos: u64,
}

impl ResumoItem {
    /// O resumo de uma entrada.
    pub fn de_entrada(e: &Entry) -> Self {
        let (campos, segredos) = campos_e_segredos(e);
        Self {
            campos,
            segredos,
            extras: e.extras().len() as u64,
            anexos: e.attachments().len() as u64,
        }
    }
}

/// Valores no JSON de um resumo, no máximo: o objeto, `campos`, `segredos`, `extras`,
/// `anexos` e os itens de `campos` e de `segredos` (para o limite de nós da resposta).
pub(crate) const MAX_NOS_RESUMO: usize = 5 + CAMPOS_TEXTO.len() + SECRET_FIELDS.len();

impl ItemLista {
    /// O item de uma entrada para o papel: com o nível só para o app (D50). Quem filtra
    /// as bloqueadas para o agente é o daemon.
    pub fn de_entrada(e: &Entry, papel: Role) -> Self {
        let endereco = match e.data() {
            Data::Login(l) => Some(l.url.clone()),
            Data::Api(a) => a.url.clone(),
            Data::Database(b) => Some(b.host.clone()),
            Data::Server(s) => Some(s.host.clone()),
            Data::Smtp(s) => Some(s.host.clone()),
            Data::FtpSftp(f) => Some(f.host.clone()),
            Data::Aws(_) | Data::SecureNote(_) | Data::Generic => None,
        };
        Self {
            nome: e.name().to_owned(),
            tipo: e.kind(),
            grupo: e.group().to_owned(),
            ambiente: e.environment(),
            projeto: e.project().map(str::to_owned),
            tags: e.tags().to_vec(),
            nivel: (papel == Role::App).then(|| e.level()),
            endereco,
            resumo: (papel == Role::App).then(|| ResumoItem::de_entrada(e)),
        }
    }
}

/// Extra no detalhe: o valor só quando não é segredo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraDetalhe {
    pub nome: String,
    /// `None` quando o extra é secreto (o valor sai só pelo revelar).
    pub valor: Option<String>,
}

/// Anexo no detalhe: nome e tamanho, sem o conteúdo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnexoDetalhe {
    pub nome: String,
    pub tamanho: u64,
}

/// Uma entrada para o formulário do app, sem segredo nenhum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetalheEntrada {
    pub nome: String,
    pub tipo: EntryType,
    pub grupo: String,
    pub ambiente: Environment,
    pub projeto: Option<String>,
    pub tags: Vec<String>,
    pub nivel: AccessLevel,
    /// Campos de texto do tipo preenchidos (identificador do protocolo, valor).
    pub campos: Vec<(String, String)>,
    /// Campos secretos do tipo preenchidos (só o identificador).
    pub segredos: Vec<String>,
    pub extras: Vec<ExtraDetalhe>,
    pub anexos: Vec<AnexoDetalhe>,
    /// Datas de criação e alteração (D65 d), carimbadas pelo daemon. O `detalhe` é só
    /// do papel app; o agente nunca recebe datas. Sem data, a chave não vai.
    pub datas: Dates,
}

impl DetalheEntrada {
    /// O detalhe de uma entrada.
    pub fn de_entrada(e: &Entry) -> Self {
        let (campos, segredos) = campos_e_segredos(e);
        Self {
            nome: e.name().to_owned(),
            tipo: e.kind(),
            grupo: e.group().to_owned(),
            ambiente: e.environment(),
            projeto: e.project().map(str::to_owned),
            tags: e.tags().to_vec(),
            nivel: e.level(),
            campos,
            segredos,
            extras: e
                .extras()
                .iter()
                .map(|x| ExtraDetalhe {
                    nome: x.name().to_owned(),
                    valor: match x.value() {
                        ExtraValue::Text(v) => Some(v.clone()),
                        ExtraValue::Secret(_) => None,
                    },
                })
                .collect(),
            anexos: e
                .attachments()
                .iter()
                .map(|a| AnexoDetalhe {
                    nome: a.name().to_owned(),
                    tamanho: a.content().len() as u64,
                })
                .collect(),
            datas: e.dates(),
        }
    }
}

/// Campos de texto (identificador, valor) e identificadores dos segredos preenchidos.
type CamposESegredos = (Vec<(String, String)>, Vec<String>);

/// Os campos de texto do tipo preenchidos (identificador, valor) e os identificadores
/// dos campos secretos preenchidos, sem o valor (do `detalhe` e do resumo da lista).
fn campos_e_segredos(e: &Entry) -> CamposESegredos {
    let mut campos: Vec<(String, String)> = Vec::new();
    let mut segredos: Vec<String> = Vec::new();
    let mut t = |k: &str, v: &str| campos.push((k.to_owned(), v.to_owned()));
    let mut s = |k: &str| segredos.push(k.to_owned());
    let auth = |a: &Auth, s: &mut dyn FnMut(&str)| match a {
        Auth::Password(_) => s("senha"),
        Auth::Key { passphrase, .. } => {
            s("chave_privada");
            if passphrase.is_some() {
                s("passphrase");
            }
        }
    };
    match e.data() {
        Data::Login(l) => {
            t("url", &l.url);
            t("usuario", &l.username);
            s("senha");
            if l.totp_seed.is_some() {
                s("totp_semente");
            }
        }
        Data::Database(b) => {
            t("motor", b.engine.name());
            t("host", &b.host);
            t("porta", &b.port.to_string());
            if let Some(x) = &b.db_name {
                t("base", x);
            }
            t("usuario", &b.username);
            if let Some(x) = &b.tunnel {
                t("tunel", x);
            }
            s("senha");
        }
        Data::Server(v) => {
            t("host", &v.host);
            t("porta", &v.port.to_string());
            t("usuario", &v.username);
            if let Some(x) = &v.jump {
                t("salto", x);
            }
            auth(&v.auth, &mut s);
        }
        Data::Api(a) => {
            if let Some(x) = &a.url {
                t("url", x);
            }
            if let Some(x) = &a.variable {
                t("variavel", x);
            }
            s("token");
        }
        Data::Aws(a) => {
            t("access_key_id", &a.access_key_id);
            t("regiao", &a.region);
            if let Some(x) = &a.role_arn {
                t("role_arn", x);
            }
            s("secret_access_key");
            if a.session_token.is_some() {
                s("session_token");
            }
        }
        Data::Smtp(m) => {
            t("host", &m.host);
            t("porta", &m.port.to_string());
            t("usuario", &m.username);
            t("seguranca", m.security.name());
            s("senha");
        }
        Data::FtpSftp(f) => {
            t("protocolo", f.protocol.name());
            t("host", &f.host);
            t("porta", &f.port.to_string());
            t("usuario", &f.username);
            auth(&f.auth, &mut s);
        }
        Data::SecureNote(_) => s("texto"),
        Data::Generic => {}
    }
    (campos, segredos)
}

/// Identificadores dos campos de texto de todos os tipos.
const CAMPOS_TEXTO: &[&str] = &[
    "url",
    "usuario",
    "motor",
    "host",
    "porta",
    "base",
    "tunel",
    "salto",
    "variavel",
    "access_key_id",
    "regiao",
    "role_arn",
    "seguranca",
    "protocolo",
];

/// O daemon pede ao app a janela de aprovação (o texto é do daemon, já montado para
/// exibir; o app ainda trata como não confiável).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PedidoDeAprovacao {
    pub id: u64,
    /// A entrada e, no uso combinado (D19), a de salto ou túnel.
    pub entradas: Vec<String>,
    pub ambiente: Environment,
    /// `executar`, `ssh`, `login` ou `playwright`.
    pub operacao: Operation,
    /// O comando exato.
    pub comando: String,
    /// O diretório de trabalho do comando no `executar` (Fase 26a: o da CLI, onde o
    /// comando roda), já formatado para exibir (D51); vazio nas outras operações.
    pub diretorio: String,
    /// Quem pediu (ex.: `claude → bash → ward run`).
    pub cadeia: Vec<String>,
    /// Fase 44c (D9): a origem para onde vão usuário e senha, quando ela é `http`
    /// (`localhost`/`127.0.0.1`: o Ward não sabe que processo responde ali); vazia nos
    /// outros casos. Montada pelo daemon a partir da URL da entrada.
    pub origem: String,
    /// Fase 44c: o pedido é só uma confirmação da origem (a entrada não pede PIN: nível
    /// livre, liberação de 1 hora ou o popup da extensão). A janela mostra "Negar" e
    /// "Confirmar", sem PIN; o daemon recusa os três "Permitir" num pedido assim e
    /// recusa "Confirmar" num pedido que não é.
    pub confirmacao: bool,
    /// Fase 54b (D68): a entrada padrão do `executar`/`ssh` não é terminal nem nula
    /// (pipe, arquivo…): o comando recebe dados que a janela não mostra. A janela
    /// avisa e só permite com a confirmação a mais; o daemon recusa um "Permitir" sem
    /// `entrada_confirmada`.
    pub entrada_oculta: bool,
}

/// O daemon pede ao app a tela de destravar (D16; Fase 39a): um uso do agente espera
/// com o cofre travado. Com o cofre travado o daemon não sabe tipo nem ambiente (o
/// índice é cifrado): o aviso leva só o que o agente pediu, já formatado para exibir
/// por [`crate::display`] (D51), e o app ainda trata como não confiável
/// (`neutralizar`). Nenhum segredo: a entrada é o nome que o agente mandou (nem se sabe
/// se existe) e o comando é o mesmo que a janela de aprovação mostraria.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PedidoDeDestrave {
    /// O nome que o agente pediu, com escape visível.
    pub entrada: String,
    /// `executar`, `ssh`, `login` ou `playwright`.
    pub operacao: Operation,
    /// O comando exato com escape visível (D51); vazio no `login` e no `playwright`.
    pub comando: String,
    /// Segundos que faltam do prazo do daemon (D16), de 1 a [`MAX_RESTANTE_DESTRAVE`].
    pub restante: u64,
}

/// Maior `restante` de um [`PedidoDeDestrave`]: os 2 min de D16.
pub const MAX_RESTANTE_DESTRAVE: u64 = 120;

/// Entrada sugerida pela extensão. Sem segredo: nome, usuário, ambiente e se a
/// entrada tem semente TOTP (só o fato, para o popup mostrar "com TOTP"; Fase 41).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sugestao {
    pub nome: String,
    pub usuario: String,
    pub ambiente: Environment,
    pub totp: bool,
}

/// Um pendrive visto pelo udisks2 (Fase 46): o UUID do sistema de arquivos, o rótulo
/// (sem caractere de controle) e se está montado. Nenhum caminho de montagem vai ao
/// cliente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pendrive {
    pub uuid: String,
    pub rotulo: String,
    pub montado: bool,
}

/// Uma versão do backup local (Fase 45, Fase 49a): o nome do arquivo e o que o daemon
/// conferiu nela. A data do nome é só do nome (qualquer processo do usuário pode
/// escolhê-la); a geração selada no corpo não dá para forjar, mas fica no daemon (D39:
/// nenhuma resposta leva a geração). Vão só as marcas:
/// - `conferida`: a versão abre com a chave do cofre aberto (AEAD); `false` para versões
///   de outra senha mestra;
/// - `fora_de_ordem`: a geração dela é menor que a de uma versão com data anterior
///   ([`crate::backup::out_of_order`]); só com `conferida`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersaoBackup {
    pub nome: String,
    pub conferida: bool,
    pub fora_de_ordem: bool,
}

/// Resposta do daemon.
#[derive(Debug)]
pub enum Resposta {
    Ok,
    Estado {
        travado: bool,
        cofre_existe: bool,
        /// O papel que o daemon deu a esta conexão (D63), sem nenhum outro dado. O app e
        /// a ponte conferem na primeira resposta e, sem `app`, não usam a conexão.
        papel: Role,
    },
    Erro(ResponseError),
    Lista(Vec<ItemLista>),
    Saida {
        fluxo: Fluxo,
        /// Saída já mascarada (D11).
        dados: Zeroizing<Vec<u8>>,
    },
    Fim {
        codigo: u8,
    },
    SessaoPlaywright {
        caminho: String,
    },
    Detalhe(DetalheEntrada),
    Segredo(Secret),
    Token(Secret),
    PedidoDeAprovacao(PedidoDeAprovacao),
    PedidoDeDestrave(PedidoDeDestrave),
    CodigoTotp {
        codigo: Zeroizing<String>,
        restante: u64,
    },
    Sugestoes(Vec<Sugestao>),
    Preenchimento {
        usuario: String,
        senha: Secret,
        totp: Option<Zeroizing<String>>,
    },
    /// `ward login` (Fase 43): o pedido que a extensão vai atender. Só o id e o nome da
    /// entrada, nenhum valor: a credencial só sai no `preencher_login`, com a origem
    /// conferida (D9).
    PedidoDeLogin {
        id: String,
        entrada: String,
    },
    /// Versões do backup local, da mais nova para a mais antiga pela data do nome
    /// (Fase 45), sem as do futuro (Fase 49a). Só nomes no formato exato e a geração
    /// selada de cada uma; nenhum caminho nem conteúdo.
    Backups(Vec<VersaoBackup>),
    /// A parte da configuração do cofre que a tela Configurações mostra (Fase 38): o
    /// tempo de trava, se o backup externo está ligado e se a pasta do backup local é
    /// segura (Fase 49a: `false` quando ela não abre ou não é um diretório 0700 do
    /// usuário, e então o cofre é gravado sem backup). Nunca o hash do PIN, o UUID do
    /// pendrive nem caminho.
    Configuracao {
        minutos_trava: u32,
        backup_externo: bool,
        backup_local: bool,
    },
    /// Configuração do backup externo (Fase 46): ligado ou não e o UUID do pendrive.
    BackupExterno {
        ligado: bool,
        uuid: Option<String>,
    },
    /// Pendrives vistos pelo udisks2 (Fase 46), até [`MAX_PENDRIVES`].
    Pendrives(Vec<Pendrive>),
}

const OPERACOES_APROVADAS: [Operation; 4] = [
    Operation::Run,
    Operation::Ssh,
    Operation::Login,
    Operation::Playwright,
];

fn digitos_totp(c: &str) -> bool {
    (6..=8).contains(&c.len()) && c.bytes().all(|b| b.is_ascii_digit())
}

impl Resposta {
    /// O tipo desta resposta.
    pub fn tipo(&self) -> ResponseType {
        match self {
            Resposta::Ok => ResponseType::Ok,
            Resposta::Estado { .. } => ResponseType::Status,
            Resposta::Erro(_) => ResponseType::Error,
            Resposta::Lista(_) => ResponseType::List,
            Resposta::Saida { .. } => ResponseType::Output,
            Resposta::Fim { .. } => ResponseType::Exit,
            Resposta::SessaoPlaywright { .. } => ResponseType::PlaywrightSession,
            Resposta::Detalhe(_) => ResponseType::Detail,
            Resposta::Segredo(_) => ResponseType::Secret,
            Resposta::Token(_) => ResponseType::Token,
            Resposta::PedidoDeAprovacao(_) => ResponseType::ApprovalRequest,
            Resposta::PedidoDeDestrave(_) => ResponseType::UnlockRequest,
            Resposta::CodigoTotp { .. } => ResponseType::TotpCode,
            Resposta::Sugestoes(_) => ResponseType::Suggestions,
            Resposta::Preenchimento { .. } => ResponseType::Fill,
            Resposta::PedidoDeLogin { .. } => ResponseType::LoginRequest,
            Resposta::Backups(_) => ResponseType::Backups,
            Resposta::Configuracao { .. } => ResponseType::Config,
            Resposta::BackupExterno { .. } => ResponseType::ExternalBackup,
            Resposta::Pendrives(_) => ResponseType::UsbDrives,
        }
    }

    /// Confere os limites que a leitura confere, e o nível e o resumo da lista pelo
    /// papel (D50, D65 c: o app sempre recebe, o agente nunca).
    pub(crate) fn conferir(&self, papel: Role) -> R<()> {
        let ok = |c: bool| if c { Ok(()) } else { Err(InvalidJson) };
        let item_ok = |i: &ItemLista| {
            i.nome.len() <= MAX_NAME
                && i.grupo.len() <= MAX_TEXT
                && i.projeto.as_ref().is_none_or(|p| p.len() <= MAX_TEXT)
                && i.tags.len() <= MAX_TAGS
                && i.tags.iter().all(|t| t.len() <= MAX_TAG)
                && i.endereco.as_ref().is_none_or(|p| p.len() <= MAX_URL)
                && i.resumo.as_ref().is_none_or(|r| {
                    campos_ok(&r.campos, &r.segredos)
                        && r.extras <= MAX_EXTRAS as u64
                        && r.anexos <= MAX_ATTACHMENTS as u64
                })
        };
        match self {
            // O papel do `estado` só é conferido na escrita (`codificar_resposta`): o
            // cliente lê o que veio e decide (D63).
            Resposta::Ok | Resposta::Estado { .. } => Ok(()),
            Resposta::PedidoDeDestrave(p) => ok(!p.entrada.is_empty()
                && p.entrada.len() <= MAX_TEXT
                && OPERACOES_APROVADAS.contains(&p.operacao)
                && p.comando.len() <= MAX_DISPLAYED_TEXT
                && (1..=MAX_RESTANTE_DESTRAVE).contains(&p.restante)),
            Resposta::Fim { .. } => Ok(()),
            Resposta::Erro(e) => ok(match e {
                ResponseError::Validation(p) => {
                    p.len() <= MAX_PROBLEMAS
                        && p.iter()
                            .all(|p| p.field.len() <= MAX_LINE && p.reason.len() <= MAX_LINE)
                }
                _ => true,
            }),
            Resposta::Lista(l) => ok(l.len() <= MAX_ENTRIES
                && l.iter().all(item_ok)
                && l.iter().all(|i| i.nivel.is_some() == (papel == Role::App))
                && l.iter().all(|i| i.resumo.is_some() == (papel == Role::App))),
            Resposta::Saida { dados, .. } => ok(dados.len() <= MAX_DISPLAYED_TEXT),
            Resposta::SessaoPlaywright { caminho } => {
                ok(!caminho.is_empty() && caminho.len() <= MAX_TEXT)
            }
            Resposta::Detalhe(d) => ok(d.nome.len() <= MAX_NAME
                && d.grupo.len() <= MAX_TEXT
                && d.projeto.as_ref().is_none_or(|p| p.len() <= MAX_TEXT)
                && d.tags.len() <= MAX_TAGS
                && d.tags.iter().all(|t| t.len() <= MAX_TAG)
                && campos_ok(&d.campos, &d.segredos)
                && d.extras.len() <= MAX_EXTRAS
                && d.extras.iter().all(|x| {
                    x.nome.len() <= MAX_EXTRA_NAME
                        && x.valor.as_ref().is_none_or(|v| v.len() <= MAX_EXTRA_VALUE)
                })
                && d.anexos.len() <= MAX_ATTACHMENTS
                && d.anexos.iter().all(|a| {
                    a.nome.len() <= MAX_ATTACHMENT_NAME && a.tamanho <= MAX_ATTACHMENTS_TOTAL as u64
                })
                && Dates::from_parts(d.datas.created_at(), d.datas.modified_at()).is_some()),
            Resposta::Segredo(s) => ok(s.len() <= crate::limits::MAX_NOTE),
            Resposta::Token(t) => ok(!t.is_empty() && t.len() <= MAX_NAME),
            Resposta::PedidoDeAprovacao(p) => ok((1..=2).contains(&p.entradas.len())
                && p.entradas.iter().all(|n| n.len() <= MAX_NAME)
                && (OPERACOES_APROVADAS.contains(&p.operacao) || p.operacao == Operation::Fill)
                && p.origem.len() <= MAX_TEXT
                // Fase 44c: a origem só existe no preencher, no `login` e no
                // `playwright`; a confirmação sempre tem origem; o popup só confirma.
                && (p.origem.is_empty()
                    || matches!(
                        p.operacao,
                        Operation::Login | Operation::Playwright | Operation::Fill
                    ))
                && (!p.confirmacao || !p.origem.is_empty())
                && (p.operacao != Operation::Fill || p.confirmacao)
                // Fase 54b: só o `executar` e o `ssh` têm entrada padrão, e nunca numa
                // confirmação de origem.
                && (!p.entrada_oculta
                    || (matches!(p.operacao, Operation::Run | Operation::Ssh)
                        && !p.confirmacao))
                && p.comando.len() <= MAX_DISPLAYED_TEXT
                && p.diretorio.len() <= MAX_DISPLAYED_TEXT
                && p.cadeia.len() <= MAX_CADEIA
                && p.cadeia.iter().all(|c| c.len() <= MAX_TEXT)),
            Resposta::CodigoTotp { codigo, restante } => {
                ok(digitos_totp(codigo) && (1..=30).contains(restante))
            }
            Resposta::Backups(l) => ok(l.len() <= MAX_BACKUPS
                && l.iter()
                    .all(|v| backup_ok(&v.nome) && (v.conferida || !v.fora_de_ordem))),
            Resposta::Configuracao { minutos_trava, .. } => {
                ok((MIN_LOCK_MINUTES..=MAX_LOCK_MINUTES).contains(minutos_trava))
            }
            Resposta::BackupExterno { ligado, uuid } => {
                ok(crate::config::ExternalBackup::new(*ligado, uuid.as_deref()).is_ok())
            }
            Resposta::Pendrives(l) => ok(l.len() <= MAX_PENDRIVES
                && l.iter()
                    .all(|p| crate::config::valid_uuid(p.uuid.as_bytes()) && rotulo_ok(&p.rotulo))),
            Resposta::Sugestoes(l) => ok(l.len() <= MAX_ENTRIES
                && l.iter()
                    .all(|s| s.nome.len() <= MAX_NAME && s.usuario.len() <= MAX_TEXT)),
            Resposta::Preenchimento {
                usuario,
                senha,
                totp,
            } => ok(usuario.len() <= MAX_TEXT
                && senha.len() <= MAX_SECRET
                && totp.as_ref().is_none_or(|c| digitos_totp(c))),
            Resposta::PedidoDeLogin { id, entrada } => {
                ok(super::login_id_ok(id) && !entrada.is_empty() && entrada.len() <= MAX_NAME)
            }
        }
    }

    /// Escreve os `dados` da resposta.
    pub(crate) fn escrever(&self, e: &mut Writer) {
        match self {
            Resposta::Ok => {}
            Resposta::PedidoDeDestrave(p) => {
                e.key("entrada");
                e.text(&p.entrada);
                e.key("operacao");
                e.text(p.operacao.name());
                e.key("comando");
                e.text(&p.comando);
                e.key("restante");
                e.integer(p.restante);
            }
            Resposta::Estado {
                travado,
                cofre_existe,
                papel,
            } => {
                e.key("travado");
                e.boolean(*travado);
                e.key("cofre_existe");
                e.boolean(*cofre_existe);
                e.key("papel");
                e.text(papel.name());
            }
            Resposta::Erro(x) => {
                e.key("codigo");
                e.text(x.code());
                e.key("mensagem");
                e.text(x.message());
                if let ResponseError::Validation(p) = x {
                    e.key("problemas");
                    e.list(|e| {
                        for p in p {
                            e.item();
                            e.object(|e| {
                                e.key("campo");
                                e.text(&p.field);
                                if let Some(n) = p.position {
                                    e.key("posicao");
                                    e.integer(n);
                                }
                                e.key("motivo");
                                e.text(&p.reason);
                            });
                        }
                    });
                }
            }
            Resposta::Lista(l) => {
                e.key("itens");
                e.list(|e| {
                    for i in l {
                        e.item();
                        e.object(|e| {
                            cabecalho(
                                e, &i.nome, i.tipo, &i.grupo, i.ambiente, &i.projeto, &i.tags,
                                i.nivel,
                            );
                            if let Some(x) = &i.endereco {
                                e.key("endereco");
                                e.text(x);
                            }
                            if let Some(r) = &i.resumo {
                                e.key("resumo");
                                e.object(|e| {
                                    escrever_campos(e, &r.campos, &r.segredos);
                                    e.key("extras");
                                    e.integer(r.extras);
                                    e.key("anexos");
                                    e.integer(r.anexos);
                                });
                            }
                        });
                    }
                });
            }
            Resposta::Saida { fluxo, dados } => {
                e.key("fluxo");
                e.text(fluxo.nome());
                e.key("dados");
                e.base64(dados);
            }
            Resposta::Fim { codigo } => {
                e.key("codigo");
                e.integer(u64::from(*codigo));
            }
            Resposta::SessaoPlaywright { caminho } => {
                e.key("caminho");
                e.text(caminho);
            }
            Resposta::Detalhe(d) => {
                cabecalho(
                    e,
                    &d.nome,
                    d.tipo,
                    &d.grupo,
                    d.ambiente,
                    &d.projeto,
                    &d.tags,
                    Some(d.nivel),
                );
                escrever_campos(e, &d.campos, &d.segredos);
                e.key("extras");
                e.list(|e| {
                    for x in &d.extras {
                        e.item();
                        e.object(|e| {
                            e.key("nome");
                            e.text(&x.nome);
                            e.key("segredo");
                            e.boolean(x.valor.is_none());
                            if let Some(v) = &x.valor {
                                e.key("valor");
                                e.text(v);
                            }
                        });
                    }
                });
                e.key("anexos");
                e.list(|e| {
                    for a in &d.anexos {
                        e.item();
                        e.object(|e| {
                            e.key("nome");
                            e.text(&a.nome);
                            e.key("tamanho");
                            e.integer(a.tamanho);
                        });
                    }
                });
                // D65 d: só as datas que existem.
                if let Some(c) = d.datas.created_at() {
                    e.key("criada_em");
                    e.integer(c);
                }
                if let Some(a) = d.datas.modified_at() {
                    e.key("alterada_em");
                    e.integer(a);
                }
            }
            Resposta::Segredo(s) => {
                e.key("valor");
                e.base64(s.expose());
            }
            Resposta::Token(t) => {
                e.key("token");
                e.base64(t.expose());
            }
            Resposta::PedidoDeAprovacao(p) => {
                e.key("id");
                e.integer(p.id);
                e.key("entradas");
                escrever_textos(e, &p.entradas);
                e.key("ambiente");
                e.text(p.ambiente.name());
                e.key("operacao");
                e.text(p.operacao.name());
                e.key("comando");
                e.text(&p.comando);
                e.key("diretorio");
                e.text(&p.diretorio);
                e.key("cadeia");
                escrever_textos(e, &p.cadeia);
                e.key("origem");
                e.text(&p.origem);
                e.key("confirmacao");
                e.boolean(p.confirmacao);
                e.key("entrada_oculta");
                e.boolean(p.entrada_oculta);
            }
            Resposta::CodigoTotp { codigo, restante } => {
                e.key("codigo");
                e.text(codigo);
                e.key("restante");
                e.integer(*restante);
            }
            Resposta::Backups(l) => {
                e.key("versoes");
                e.list(|e| {
                    for v in l {
                        e.item();
                        e.object(|e| {
                            e.key("nome");
                            e.text(&v.nome);
                            e.key("conferida");
                            e.boolean(v.conferida);
                            e.key("fora_de_ordem");
                            e.boolean(v.fora_de_ordem);
                        });
                    }
                });
            }
            Resposta::Configuracao {
                minutos_trava,
                backup_externo,
                backup_local,
            } => {
                e.key("minutos_trava");
                e.integer(u64::from(*minutos_trava));
                e.key("backup_externo");
                e.boolean(*backup_externo);
                e.key("backup_local");
                e.boolean(*backup_local);
            }
            Resposta::BackupExterno { ligado, uuid } => {
                e.key("ligado");
                e.boolean(*ligado);
                if let Some(u) = uuid {
                    e.key("uuid");
                    e.text(u);
                }
            }
            Resposta::Pendrives(l) => {
                e.key("itens");
                e.list(|e| {
                    for p in l {
                        e.item();
                        e.object(|e| {
                            e.key("uuid");
                            e.text(&p.uuid);
                            e.key("rotulo");
                            e.text(&p.rotulo);
                            e.key("montado");
                            e.boolean(p.montado);
                        });
                    }
                });
            }
            Resposta::Sugestoes(l) => {
                e.key("itens");
                e.list(|e| {
                    for s in l {
                        e.item();
                        e.object(|e| {
                            e.key("nome");
                            e.text(&s.nome);
                            e.key("usuario");
                            e.text(&s.usuario);
                            e.key("ambiente");
                            e.text(s.ambiente.name());
                            e.key("totp");
                            e.boolean(s.totp);
                        });
                    }
                });
            }
            Resposta::Preenchimento {
                usuario,
                senha,
                totp,
            } => {
                e.key("usuario");
                e.text(usuario);
                e.key("senha");
                e.base64(senha.expose());
                if let Some(c) = totp {
                    e.key("totp");
                    e.text(c);
                }
            }
            Resposta::PedidoDeLogin { id, entrada } => {
                e.key("id");
                e.text(id);
                e.key("entrada");
                e.text(entrada);
            }
        }
    }
}

/// Problemas de validação numa resposta.
const MAX_PROBLEMAS: usize = 512;
/// Mais versões numa resposta `backups`: as 30 da rotação e folga para arquivos com
/// nome válido postos à mão na pasta (a lista sai cortada nas mais novas).
pub const MAX_BACKUPS: usize = 64;

/// Mais pendrives numa resposta `pendrives` (Fase 46).
pub const MAX_PENDRIVES: usize = 32;
/// Maior rótulo de pendrive, em bytes (o do ext4 tem 16, o do NTFS até 128 em UTF-16).
pub const MAX_ROTULO: usize = 128;

/// Rótulo de pendrive: até [`MAX_ROTULO`] bytes, sem caractere de controle.
fn rotulo_ok(r: &str) -> bool {
    r.len() <= MAX_ROTULO && !r.chars().any(char::is_control)
}

/// Nome de versão de backup no formato exato (Fase 45).
fn backup_ok(n: &str) -> bool {
    #[cfg(unix)]
    {
        crate::backup::valid_name(n)
    }
    #[cfg(not(unix))]
    {
        let _ = n;
        false
    }
}

/// Processos na cadeia mostrada na aprovação.
const MAX_CADEIA: usize = 64;

/// Os campos de texto e os identificadores dos segredos, sem valor: identificador de
/// texto conhecido, valor no limite e só identificador de segredo conhecido (o mesmo
/// do `detalhe` e do resumo da lista).
fn campos_ok(campos: &[(String, String)], segredos: &[String]) -> bool {
    campos.len() <= CAMPOS_TEXTO.len()
        && campos
            .iter()
            .all(|(k, v)| CAMPOS_TEXTO.contains(&k.as_str()) && v.len() <= MAX_TEXT)
        && segredos.len() <= SECRET_FIELDS.len()
        && segredos.iter().all(|k| SECRET_FIELDS.contains(&k.as_str()))
}

fn escrever_campos(e: &mut Writer, campos: &[(String, String)], segredos: &[String]) {
    e.key("campos");
    e.object(|e| {
        for (k, v) in campos {
            e.key(k);
            e.text(v);
        }
    });
    e.key("segredos");
    escrever_textos(e, segredos);
}

/// Lê `campos` e `segredos` (do `detalhe` e do resumo): na ordem do quadro, só os
/// campos de texto conhecidos (a chave repetida o JSON já recusou); a conferência no
/// fim limita a quantidade e os identificadores dos segredos.
fn ler_campos(c: &mut Fields) -> R<CamposESegredos> {
    let mut campos = Vec::new();
    for (k, v) in c.object("campos")?.in_order() {
        if !CAMPOS_TEXTO.contains(&k.as_str()) {
            return Err(InvalidJson);
        }
        campos.push((k, text_of(v, MAX_TEXT)?));
    }
    let segredos = texts_of(c.list("segredos", SECRET_FIELDS.len())?, MAX_NAME)?;
    Ok((campos, segredos))
}

fn escrever_textos(e: &mut Writer, l: &[String]) {
    e.list(|e| {
        for t in l {
            e.item();
            e.text(t);
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn cabecalho(
    e: &mut Writer,
    nome: &str,
    tipo: EntryType,
    grupo: &str,
    ambiente: Environment,
    projeto: &Option<String>,
    tags: &[String],
    nivel: Option<AccessLevel>,
) {
    e.key("nome");
    e.text(nome);
    e.key("tipo");
    e.text(tipo.name());
    e.key("grupo");
    e.text(grupo);
    e.key("ambiente");
    e.text(ambiente.name());
    if let Some(p) = projeto {
        e.key("projeto");
        e.text(p);
    }
    e.key("tags");
    escrever_textos(e, tags);
    if let Some(nivel) = nivel {
        e.key("nivel");
        e.text(nivel.name());
    }
}

struct Cabecalho {
    nome: String,
    tipo: EntryType,
    grupo: String,
    ambiente: Environment,
    projeto: Option<String>,
    tags: Vec<String>,
    nivel: Option<AccessLevel>,
}

/// Lê o cabeçalho de um item ou detalhe. `com_nivel`: o nível é obrigatório (app) ou
/// proibido (lista do agente, D50).
fn ler_cabecalho(c: &mut Fields, com_nivel: bool) -> R<Cabecalho> {
    Ok(Cabecalho {
        nome: c.text("nome", MAX_NAME)?,
        tipo: EntryType::from_name(&c.text("tipo", MAX_NAME)?).ok_or(InvalidJson)?,
        grupo: c.text("grupo", MAX_TEXT)?,
        ambiente: Environment::from_name(&c.text("ambiente", MAX_NAME)?).ok_or(InvalidJson)?,
        projeto: c.text_opt("projeto", MAX_TEXT)?,
        tags: texts_of(c.list("tags", MAX_TAGS)?, MAX_TAG)?,
        nivel: match c.text_opt("nivel", MAX_NAME)? {
            Some(n) if com_nivel => Some(AccessLevel::from_name(&n).ok_or(InvalidJson)?),
            None if !com_nivel => None,
            _ => return Err(InvalidJson),
        },
    })
}

/// Lê os `dados` de uma resposta do tipo (o papel já foi conferido).
pub(crate) fn ler(papel: Role, tipo: ResponseType, mut c: Fields) -> R<Resposta> {
    let r = match tipo {
        ResponseType::Ok => Resposta::Ok,
        ResponseType::UnlockRequest => Resposta::PedidoDeDestrave(PedidoDeDestrave {
            entrada: c.text("entrada", MAX_TEXT)?,
            operacao: Operation::from_name(&c.text("operacao", MAX_NAME)?).ok_or(InvalidJson)?,
            comando: c.text("comando", MAX_DISPLAYED_TEXT)?,
            restante: c.integer("restante")?,
        }),
        ResponseType::Status => Resposta::Estado {
            travado: c.boolean("travado")?,
            cofre_existe: c.boolean("cofre_existe")?,
            // Lido como veio: quem recebe confere se é o papel que esperava.
            papel: Role::from_name(&c.text("papel", MAX_NAME)?).ok_or(InvalidJson)?,
        },
        ResponseType::Error => {
            let codigo = c.text("codigo", MAX_NAME)?;
            // A mensagem do quadro é ignorada: vale a fixa do código.
            let _ = c.text("mensagem", MAX_LINE)?;
            let mut erro = ResponseError::from_code(&codigo).ok_or(InvalidJson)?;
            if let ResponseError::Validation(lista) = &mut erro {
                for v in c.list("problemas", MAX_PROBLEMAS)? {
                    let mut o = Fields::from_value(v)?;
                    let campo = o.text("campo", MAX_LINE)?;
                    let posicao = match o.take("posicao") {
                        Some(super::json::Value::Integer(n)) => Some(n),
                        Some(_) => return Err(InvalidJson),
                        None => None,
                    };
                    let motivo = o.text("motivo", MAX_LINE)?;
                    o.finish()?;
                    lista.push(Problem {
                        field: campo,
                        position: posicao,
                        reason: motivo,
                    });
                }
            }
            Resposta::Erro(erro)
        }
        ResponseType::List => {
            let mut itens = Vec::new();
            for v in c.list("itens", MAX_ENTRIES)? {
                let mut o = Fields::from_value(v)?;
                let k = ler_cabecalho(&mut o, papel == Role::App)?;
                let endereco = o.text_opt("endereco", MAX_URL)?;
                // D65 c: o resumo é obrigatório na lista do app e proibido na do agente.
                let resumo = match o.take("resumo") {
                    Some(v) if papel == Role::App => {
                        let mut r = Fields::from_value(v)?;
                        let (campos, segredos) = ler_campos(&mut r)?;
                        let extras = r.integer("extras")?;
                        let anexos = r.integer("anexos")?;
                        r.finish()?;
                        Some(ResumoItem {
                            campos,
                            segredos,
                            extras,
                            anexos,
                        })
                    }
                    None if papel != Role::App => None,
                    _ => return Err(InvalidJson),
                };
                o.finish()?;
                itens.push(ItemLista {
                    nome: k.nome,
                    tipo: k.tipo,
                    grupo: k.grupo,
                    ambiente: k.ambiente,
                    projeto: k.projeto,
                    tags: k.tags,
                    nivel: k.nivel,
                    endereco,
                    resumo,
                });
            }
            Resposta::Lista(itens)
        }
        ResponseType::Output => {
            let fluxo = match c.text("fluxo", MAX_NAME)?.as_str() {
                "saida" => Fluxo::Saida,
                "erro" => Fluxo::Erro,
                _ => return Err(InvalidJson),
            };
            Resposta::Saida {
                fluxo,
                dados: c.bytes("dados", MAX_DISPLAYED_TEXT)?,
            }
        }
        ResponseType::Exit => Resposta::Fim {
            codigo: u8::try_from(c.integer("codigo")?).map_err(|_| InvalidJson)?,
        },
        ResponseType::PlaywrightSession => Resposta::SessaoPlaywright {
            caminho: c.text("caminho", MAX_TEXT)?,
        },
        ResponseType::Detail => {
            let k = ler_cabecalho(&mut c, true)?;
            let (campos, segredos) = ler_campos(&mut c)?;
            let mut extras = Vec::new();
            for v in c.list("extras", MAX_EXTRAS)? {
                let mut o = Fields::from_value(v)?;
                let nome = o.text("nome", MAX_EXTRA_NAME)?;
                let segredo = o.boolean("segredo")?;
                let valor = o.text_opt("valor", MAX_EXTRA_VALUE)?;
                // Extra secreto nunca leva valor; o comum sempre leva.
                if segredo == valor.is_some() {
                    return Err(InvalidJson);
                }
                o.finish()?;
                extras.push(ExtraDetalhe { nome, valor });
            }
            let mut anexos = Vec::new();
            for v in c.list("anexos", MAX_ATTACHMENTS)? {
                let mut o = Fields::from_value(v)?;
                let nome = o.text("nome", MAX_ATTACHMENT_NAME)?;
                let tamanho = o.integer("tamanho")?;
                o.finish()?;
                anexos.push(AnexoDetalhe { nome, tamanho });
            }
            // D65 d: datas opcionais, pela regra de `Datas` (faixa, criação só com
            // alteração e não depois dela).
            let mut data = |k: &str| match c.take(k) {
                None => Ok(None),
                Some(super::json::Value::Integer(n)) => Ok(Some(n)),
                Some(_) => Err(InvalidJson),
            };
            let criada = data("criada_em")?;
            let alterada = data("alterada_em")?;
            let datas = Dates::from_parts(criada, alterada).ok_or(InvalidJson)?;
            Resposta::Detalhe(DetalheEntrada {
                nome: k.nome,
                tipo: k.tipo,
                grupo: k.grupo,
                ambiente: k.ambiente,
                projeto: k.projeto,
                tags: k.tags,
                nivel: k.nivel.ok_or(InvalidJson)?,
                campos,
                segredos,
                extras,
                anexos,
                datas,
            })
        }
        ResponseType::Secret => Resposta::Segredo(c.secret("valor", crate::limits::MAX_NOTE)?),
        ResponseType::Token => Resposta::Token(c.secret("token", MAX_NAME)?),
        ResponseType::ApprovalRequest => {
            let id = c.integer("id")?;
            let entradas = texts_of(c.list("entradas", 2)?, MAX_NAME)?;
            let ambiente =
                Environment::from_name(&c.text("ambiente", MAX_NAME)?).ok_or(InvalidJson)?;
            let operacao =
                Operation::from_name(&c.text("operacao", MAX_NAME)?).ok_or(InvalidJson)?;
            let comando = c.text("comando", MAX_DISPLAYED_TEXT)?;
            let diretorio = c.text("diretorio", MAX_DISPLAYED_TEXT)?;
            let cadeia = texts_of(c.list("cadeia", MAX_CADEIA)?, MAX_TEXT)?;
            let origem = c.text("origem", MAX_TEXT)?;
            let confirmacao = c.boolean("confirmacao")?;
            let entrada_oculta = c.boolean("entrada_oculta")?;
            Resposta::PedidoDeAprovacao(PedidoDeAprovacao {
                id,
                entradas,
                ambiente,
                operacao,
                comando,
                diretorio,
                cadeia,
                origem,
                confirmacao,
                entrada_oculta,
            })
        }
        ResponseType::TotpCode => {
            let codigo = c.text("codigo", 8)?;
            Resposta::CodigoTotp {
                codigo: Zeroizing::new(codigo),
                restante: c.integer("restante")?,
            }
        }
        ResponseType::Suggestions => {
            let mut itens = Vec::new();
            for v in c.list("itens", MAX_ENTRIES)? {
                let mut o = Fields::from_value(v)?;
                let nome = o.text("nome", MAX_NAME)?;
                let usuario = o.text("usuario", MAX_TEXT)?;
                let ambiente =
                    Environment::from_name(&o.text("ambiente", MAX_NAME)?).ok_or(InvalidJson)?;
                let totp = o.boolean("totp")?;
                o.finish()?;
                itens.push(Sugestao {
                    nome,
                    usuario,
                    ambiente,
                    totp,
                });
            }
            Resposta::Sugestoes(itens)
        }
        ResponseType::Backups => {
            let mut versoes = Vec::new();
            for v in c.list("versoes", MAX_BACKUPS)? {
                let mut o = Fields::from_value(v)?;
                let nome = o.text("nome", crate::limits::MAX_NAME)?;
                let conferida = o.boolean("conferida")?;
                let fora_de_ordem = o.boolean("fora_de_ordem")?;
                o.finish()?;
                versoes.push(VersaoBackup {
                    nome,
                    conferida,
                    fora_de_ordem,
                });
            }
            Resposta::Backups(versoes)
        }
        ResponseType::Config => Resposta::Configuracao {
            minutos_trava: u32::try_from(c.integer("minutos_trava")?).map_err(|_| InvalidJson)?,
            backup_externo: c.boolean("backup_externo")?,
            backup_local: c.boolean("backup_local")?,
        },
        ResponseType::ExternalBackup => Resposta::BackupExterno {
            ligado: c.boolean("ligado")?,
            uuid: c.text_opt("uuid", crate::limits::MAX_BACKUP_UUID)?,
        },
        ResponseType::UsbDrives => {
            let mut itens = Vec::new();
            for v in c.list("itens", MAX_PENDRIVES)? {
                let mut o = Fields::from_value(v)?;
                let uuid = o.text("uuid", crate::limits::MAX_BACKUP_UUID)?;
                let rotulo = o.text("rotulo", MAX_ROTULO)?;
                let montado = o.boolean("montado")?;
                o.finish()?;
                itens.push(Pendrive {
                    uuid,
                    rotulo,
                    montado,
                });
            }
            Resposta::Pendrives(itens)
        }
        ResponseType::Fill => Resposta::Preenchimento {
            usuario: c.text("usuario", MAX_TEXT)?,
            senha: secret_of(c.take("senha").ok_or(InvalidJson)?, MAX_SECRET)?,
            totp: c.text_opt("totp", 8)?.map(Zeroizing::new),
        },
        ResponseType::LoginRequest => Resposta::PedidoDeLogin {
            id: c.text("id", MAX_NAME)?,
            entrada: c.text("entrada", MAX_NAME)?,
        },
    };
    c.finish()?;
    r.conferir(papel)?;
    Ok(r)
}
