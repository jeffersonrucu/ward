//! Respostas e avisos do daemon ao cliente: os campos de cada tipo, a escrita (lado do
//! daemon) e a leitura estrita (lado do cliente).
//!
//! Os tipos que levam credencial ([`super::TipoResposta::carrega_credencial`]) são só de
//! app ou de extensão; [`super::codificar_resposta`] confere o papel antes de escrever.
//! [`ItemLista`] e [`DetalheEntrada`] são montados da [`Entrada`] e por construção não
//! têm segredo: o extra secreto vai sem valor, e dos segredos do tipo vai só o nome do
//! campo preenchido.

use zeroize::Zeroizing;

use super::campos::{Campos, R, segredo_de, texto_de, textos_de};
use super::erros::{ErroResposta, Problema};
use super::json::{Escritor, JsonInvalido};
use super::pedido::CAMPOS_SECRETOS;
use super::{Operacao, Papel, TipoResposta};
use crate::entrada::{
    Ambiente, Autenticacao, Dados, Datas, Entrada, NivelAcesso, Segredo, TipoEntrada, ValorExtra,
};
use crate::limites::{
    MAX_ANEXOS, MAX_ANEXOS_SOMADOS, MAX_ENTRADAS, MAX_EXTRAS, MAX_LINHA, MAX_MINUTOS_TRAVA,
    MAX_NOME, MAX_NOME_ANEXO, MAX_NOME_EXTRA, MAX_SEGREDO, MAX_TAG, MAX_TAGS, MAX_TEXTO,
    MAX_TEXTO_EXIBIDO, MAX_URL, MAX_VALOR_EXTRA, MIN_MINUTOS_TRAVA,
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
    pub tipo: TipoEntrada,
    pub grupo: String,
    pub ambiente: Ambiente,
    pub projeto: Option<String>,
    pub tags: Vec<String>,
    /// Nível de acesso: `Some` só na lista do app. A do agente vai sem ele (D50):
    /// [`super::codificar_resposta`] recusa mandar ao agente um item com nível, e a
    /// leitura do agente recusa um item que o traga.
    pub nivel: Option<NivelAcesso>,
    /// URL (`login`, `api`) ou host (`banco`, `servidor`, `smtp`, `ftp/sftp`).
    pub endereco: Option<String>,
    /// Resumo para a 2ª linha e a busca do app (D65 c): `Some` só na lista do app, no
    /// mesmo padrão do nível (D50). [`super::codificar_resposta`] recusa mandar ao
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
    pub fn de_entrada(e: &Entrada) -> Self {
        let (campos, segredos) = campos_e_segredos(e);
        Self {
            campos,
            segredos,
            extras: e.extras().len() as u64,
            anexos: e.anexos().len() as u64,
        }
    }
}

/// Valores no JSON de um resumo, no máximo: o objeto, `campos`, `segredos`, `extras`,
/// `anexos` e os itens de `campos` e de `segredos` (para o limite de nós da resposta).
pub(crate) const MAX_NOS_RESUMO: usize = 5 + CAMPOS_TEXTO.len() + CAMPOS_SECRETOS.len();

impl ItemLista {
    /// O item de uma entrada para o papel: com o nível só para o app (D50). Quem filtra
    /// as bloqueadas para o agente é o daemon.
    pub fn de_entrada(e: &Entrada, papel: Papel) -> Self {
        let endereco = match e.dados() {
            Dados::Login(l) => Some(l.url.clone()),
            Dados::Api(a) => a.url.clone(),
            Dados::Banco(b) => Some(b.host.clone()),
            Dados::Servidor(s) => Some(s.host.clone()),
            Dados::Smtp(s) => Some(s.host.clone()),
            Dados::FtpSftp(f) => Some(f.host.clone()),
            Dados::Aws(_) | Dados::NotaSegura(_) | Dados::Generico => None,
        };
        Self {
            nome: e.nome().to_owned(),
            tipo: e.tipo(),
            grupo: e.grupo().to_owned(),
            ambiente: e.ambiente(),
            projeto: e.projeto().map(str::to_owned),
            tags: e.tags().to_vec(),
            nivel: (papel == Papel::App).then(|| e.nivel()),
            endereco,
            resumo: (papel == Papel::App).then(|| ResumoItem::de_entrada(e)),
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
    pub tipo: TipoEntrada,
    pub grupo: String,
    pub ambiente: Ambiente,
    pub projeto: Option<String>,
    pub tags: Vec<String>,
    pub nivel: NivelAcesso,
    /// Campos de texto do tipo preenchidos (identificador do protocolo, valor).
    pub campos: Vec<(String, String)>,
    /// Campos secretos do tipo preenchidos (só o identificador).
    pub segredos: Vec<String>,
    pub extras: Vec<ExtraDetalhe>,
    pub anexos: Vec<AnexoDetalhe>,
    /// Datas de criação e alteração (D65 d), carimbadas pelo daemon. O `detalhe` é só
    /// do papel app; o agente nunca recebe datas. Sem data, a chave não vai.
    pub datas: Datas,
}

impl DetalheEntrada {
    /// O detalhe de uma entrada.
    pub fn de_entrada(e: &Entrada) -> Self {
        let (campos, segredos) = campos_e_segredos(e);
        Self {
            nome: e.nome().to_owned(),
            tipo: e.tipo(),
            grupo: e.grupo().to_owned(),
            ambiente: e.ambiente(),
            projeto: e.projeto().map(str::to_owned),
            tags: e.tags().to_vec(),
            nivel: e.nivel(),
            campos,
            segredos,
            extras: e
                .extras()
                .iter()
                .map(|x| ExtraDetalhe {
                    nome: x.nome().to_owned(),
                    valor: match x.valor() {
                        ValorExtra::Texto(v) => Some(v.clone()),
                        ValorExtra::Segredo(_) => None,
                    },
                })
                .collect(),
            anexos: e
                .anexos()
                .iter()
                .map(|a| AnexoDetalhe {
                    nome: a.nome().to_owned(),
                    tamanho: a.conteudo().len() as u64,
                })
                .collect(),
            datas: e.datas(),
        }
    }
}

/// Campos de texto (identificador, valor) e identificadores dos segredos preenchidos.
type CamposESegredos = (Vec<(String, String)>, Vec<String>);

/// Os campos de texto do tipo preenchidos (identificador, valor) e os identificadores
/// dos campos secretos preenchidos, sem o valor (do `detalhe` e do resumo da lista).
fn campos_e_segredos(e: &Entrada) -> CamposESegredos {
    let mut campos: Vec<(String, String)> = Vec::new();
    let mut segredos: Vec<String> = Vec::new();
    let mut t = |k: &str, v: &str| campos.push((k.to_owned(), v.to_owned()));
    let mut s = |k: &str| segredos.push(k.to_owned());
    let auth = |a: &Autenticacao, s: &mut dyn FnMut(&str)| match a {
        Autenticacao::Senha(_) => s("senha"),
        Autenticacao::Chave { passphrase, .. } => {
            s("chave_privada");
            if passphrase.is_some() {
                s("passphrase");
            }
        }
    };
    match e.dados() {
        Dados::Login(l) => {
            t("url", &l.url);
            t("usuario", &l.usuario);
            s("senha");
            if l.totp_semente.is_some() {
                s("totp_semente");
            }
        }
        Dados::Banco(b) => {
            t("motor", b.motor.nome());
            t("host", &b.host);
            t("porta", &b.porta.to_string());
            if let Some(x) = &b.base {
                t("base", x);
            }
            t("usuario", &b.usuario);
            if let Some(x) = &b.tunel {
                t("tunel", x);
            }
            s("senha");
        }
        Dados::Servidor(v) => {
            t("host", &v.host);
            t("porta", &v.porta.to_string());
            t("usuario", &v.usuario);
            if let Some(x) = &v.salto {
                t("salto", x);
            }
            auth(&v.autenticacao, &mut s);
        }
        Dados::Api(a) => {
            if let Some(x) = &a.url {
                t("url", x);
            }
            if let Some(x) = &a.variavel {
                t("variavel", x);
            }
            s("token");
        }
        Dados::Aws(a) => {
            t("access_key_id", &a.access_key_id);
            t("regiao", &a.regiao);
            if let Some(x) = &a.role_arn {
                t("role_arn", x);
            }
            s("secret_access_key");
            if a.session_token.is_some() {
                s("session_token");
            }
        }
        Dados::Smtp(m) => {
            t("host", &m.host);
            t("porta", &m.porta.to_string());
            t("usuario", &m.usuario);
            t("seguranca", m.seguranca.nome());
            s("senha");
        }
        Dados::FtpSftp(f) => {
            t("protocolo", f.protocolo.nome());
            t("host", &f.host);
            t("porta", &f.porta.to_string());
            t("usuario", &f.usuario);
            auth(&f.autenticacao, &mut s);
        }
        Dados::NotaSegura(_) => s("texto"),
        Dados::Generico => {}
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
    pub ambiente: Ambiente,
    /// `executar`, `ssh`, `login` ou `playwright`.
    pub operacao: Operacao,
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
/// por [`crate::exibicao`] (D51), e o app ainda trata como não confiável
/// (`neutralizar`). Nenhum segredo: a entrada é o nome que o agente mandou (nem se sabe
/// se existe) e o comando é o mesmo que a janela de aprovação mostraria.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PedidoDeDestrave {
    /// O nome que o agente pediu, com escape visível.
    pub entrada: String,
    /// `executar`, `ssh`, `login` ou `playwright`.
    pub operacao: Operacao,
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
    pub ambiente: Ambiente,
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
///   ([`crate::backup::fora_de_ordem`]); só com `conferida`.
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
        papel: Papel,
    },
    Erro(ErroResposta),
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
    Segredo(Segredo),
    Token(Segredo),
    PedidoDeAprovacao(PedidoDeAprovacao),
    PedidoDeDestrave(PedidoDeDestrave),
    CodigoTotp {
        codigo: Zeroizing<String>,
        restante: u64,
    },
    Sugestoes(Vec<Sugestao>),
    Preenchimento {
        usuario: String,
        senha: Segredo,
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

const OPERACOES_APROVADAS: [Operacao; 4] = [
    Operacao::Executar,
    Operacao::Ssh,
    Operacao::Login,
    Operacao::Playwright,
];

fn digitos_totp(c: &str) -> bool {
    (6..=8).contains(&c.len()) && c.bytes().all(|b| b.is_ascii_digit())
}

impl Resposta {
    /// O tipo desta resposta.
    pub fn tipo(&self) -> TipoResposta {
        match self {
            Resposta::Ok => TipoResposta::Ok,
            Resposta::Estado { .. } => TipoResposta::Estado,
            Resposta::Erro(_) => TipoResposta::Erro,
            Resposta::Lista(_) => TipoResposta::Lista,
            Resposta::Saida { .. } => TipoResposta::Saida,
            Resposta::Fim { .. } => TipoResposta::Fim,
            Resposta::SessaoPlaywright { .. } => TipoResposta::SessaoPlaywright,
            Resposta::Detalhe(_) => TipoResposta::Detalhe,
            Resposta::Segredo(_) => TipoResposta::Segredo,
            Resposta::Token(_) => TipoResposta::Token,
            Resposta::PedidoDeAprovacao(_) => TipoResposta::PedidoDeAprovacao,
            Resposta::PedidoDeDestrave(_) => TipoResposta::PedidoDeDestrave,
            Resposta::CodigoTotp { .. } => TipoResposta::CodigoTotp,
            Resposta::Sugestoes(_) => TipoResposta::Sugestoes,
            Resposta::Preenchimento { .. } => TipoResposta::Preenchimento,
            Resposta::PedidoDeLogin { .. } => TipoResposta::PedidoDeLogin,
            Resposta::Backups(_) => TipoResposta::Backups,
            Resposta::Configuracao { .. } => TipoResposta::Configuracao,
            Resposta::BackupExterno { .. } => TipoResposta::BackupExterno,
            Resposta::Pendrives(_) => TipoResposta::Pendrives,
        }
    }

    /// Confere os limites que a leitura confere, e o nível e o resumo da lista pelo
    /// papel (D50, D65 c: o app sempre recebe, o agente nunca).
    pub(crate) fn conferir(&self, papel: Papel) -> R<()> {
        let ok = |c: bool| if c { Ok(()) } else { Err(JsonInvalido) };
        let item_ok = |i: &ItemLista| {
            i.nome.len() <= MAX_NOME
                && i.grupo.len() <= MAX_TEXTO
                && i.projeto.as_ref().is_none_or(|p| p.len() <= MAX_TEXTO)
                && i.tags.len() <= MAX_TAGS
                && i.tags.iter().all(|t| t.len() <= MAX_TAG)
                && i.endereco.as_ref().is_none_or(|p| p.len() <= MAX_URL)
                && i.resumo.as_ref().is_none_or(|r| {
                    campos_ok(&r.campos, &r.segredos)
                        && r.extras <= MAX_EXTRAS as u64
                        && r.anexos <= MAX_ANEXOS as u64
                })
        };
        match self {
            // O papel do `estado` só é conferido na escrita (`codificar_resposta`): o
            // cliente lê o que veio e decide (D63).
            Resposta::Ok | Resposta::Estado { .. } => Ok(()),
            Resposta::PedidoDeDestrave(p) => ok(!p.entrada.is_empty()
                && p.entrada.len() <= MAX_TEXTO
                && OPERACOES_APROVADAS.contains(&p.operacao)
                && p.comando.len() <= MAX_TEXTO_EXIBIDO
                && (1..=MAX_RESTANTE_DESTRAVE).contains(&p.restante)),
            Resposta::Fim { .. } => Ok(()),
            Resposta::Erro(e) => ok(match e {
                ErroResposta::Validacao(p) => {
                    p.len() <= MAX_PROBLEMAS
                        && p.iter()
                            .all(|p| p.campo.len() <= MAX_LINHA && p.motivo.len() <= MAX_LINHA)
                }
                _ => true,
            }),
            Resposta::Lista(l) => ok(l.len() <= MAX_ENTRADAS
                && l.iter().all(item_ok)
                && l.iter().all(|i| i.nivel.is_some() == (papel == Papel::App))
                && l.iter()
                    .all(|i| i.resumo.is_some() == (papel == Papel::App))),
            Resposta::Saida { dados, .. } => ok(dados.len() <= MAX_TEXTO_EXIBIDO),
            Resposta::SessaoPlaywright { caminho } => {
                ok(!caminho.is_empty() && caminho.len() <= MAX_TEXTO)
            }
            Resposta::Detalhe(d) => ok(d.nome.len() <= MAX_NOME
                && d.grupo.len() <= MAX_TEXTO
                && d.projeto.as_ref().is_none_or(|p| p.len() <= MAX_TEXTO)
                && d.tags.len() <= MAX_TAGS
                && d.tags.iter().all(|t| t.len() <= MAX_TAG)
                && campos_ok(&d.campos, &d.segredos)
                && d.extras.len() <= MAX_EXTRAS
                && d.extras.iter().all(|x| {
                    x.nome.len() <= MAX_NOME_EXTRA
                        && x.valor.as_ref().is_none_or(|v| v.len() <= MAX_VALOR_EXTRA)
                })
                && d.anexos.len() <= MAX_ANEXOS
                && d.anexos.iter().all(|a| {
                    a.nome.len() <= MAX_NOME_ANEXO && a.tamanho <= MAX_ANEXOS_SOMADOS as u64
                })
                && Datas::de_partes(d.datas.criada_em(), d.datas.alterada_em()).is_some()),
            Resposta::Segredo(s) => ok(s.len() <= crate::limites::MAX_NOTA),
            Resposta::Token(t) => ok(!t.is_empty() && t.len() <= MAX_NOME),
            Resposta::PedidoDeAprovacao(p) => ok((1..=2).contains(&p.entradas.len())
                && p.entradas.iter().all(|n| n.len() <= MAX_NOME)
                && (OPERACOES_APROVADAS.contains(&p.operacao) || p.operacao == Operacao::Preencher)
                && p.origem.len() <= MAX_TEXTO
                // Fase 44c: a origem só existe no preencher, no `login` e no
                // `playwright`; a confirmação sempre tem origem; o popup só confirma.
                && (p.origem.is_empty()
                    || matches!(
                        p.operacao,
                        Operacao::Login | Operacao::Playwright | Operacao::Preencher
                    ))
                && (!p.confirmacao || !p.origem.is_empty())
                && (p.operacao != Operacao::Preencher || p.confirmacao)
                // Fase 54b: só o `executar` e o `ssh` têm entrada padrão, e nunca numa
                // confirmação de origem.
                && (!p.entrada_oculta
                    || (matches!(p.operacao, Operacao::Executar | Operacao::Ssh)
                        && !p.confirmacao))
                && p.comando.len() <= MAX_TEXTO_EXIBIDO
                && p.diretorio.len() <= MAX_TEXTO_EXIBIDO
                && p.cadeia.len() <= MAX_CADEIA
                && p.cadeia.iter().all(|c| c.len() <= MAX_TEXTO)),
            Resposta::CodigoTotp { codigo, restante } => {
                ok(digitos_totp(codigo) && (1..=30).contains(restante))
            }
            Resposta::Backups(l) => ok(l.len() <= MAX_BACKUPS
                && l.iter()
                    .all(|v| backup_ok(&v.nome) && (v.conferida || !v.fora_de_ordem))),
            Resposta::Configuracao { minutos_trava, .. } => {
                ok((MIN_MINUTOS_TRAVA..=MAX_MINUTOS_TRAVA).contains(minutos_trava))
            }
            Resposta::BackupExterno { ligado, uuid } => {
                ok(crate::configuracao::BackupExterno::novo(*ligado, uuid.as_deref()).is_ok())
            }
            Resposta::Pendrives(l) => ok(l.len() <= MAX_PENDRIVES
                && l.iter().all(|p| {
                    crate::configuracao::uuid_valido(p.uuid.as_bytes()) && rotulo_ok(&p.rotulo)
                })),
            Resposta::Sugestoes(l) => ok(l.len() <= MAX_ENTRADAS
                && l.iter()
                    .all(|s| s.nome.len() <= MAX_NOME && s.usuario.len() <= MAX_TEXTO)),
            Resposta::Preenchimento {
                usuario,
                senha,
                totp,
            } => ok(usuario.len() <= MAX_TEXTO
                && senha.len() <= MAX_SEGREDO
                && totp.as_ref().is_none_or(|c| digitos_totp(c))),
            Resposta::PedidoDeLogin { id, entrada } => {
                ok(super::id_login_ok(id) && !entrada.is_empty() && entrada.len() <= MAX_NOME)
            }
        }
    }

    /// Escreve os `dados` da resposta.
    pub(crate) fn escrever(&self, e: &mut Escritor) {
        match self {
            Resposta::Ok => {}
            Resposta::PedidoDeDestrave(p) => {
                e.chave("entrada");
                e.texto(&p.entrada);
                e.chave("operacao");
                e.texto(p.operacao.nome());
                e.chave("comando");
                e.texto(&p.comando);
                e.chave("restante");
                e.inteiro(p.restante);
            }
            Resposta::Estado {
                travado,
                cofre_existe,
                papel,
            } => {
                e.chave("travado");
                e.logico(*travado);
                e.chave("cofre_existe");
                e.logico(*cofre_existe);
                e.chave("papel");
                e.texto(papel.nome());
            }
            Resposta::Erro(x) => {
                e.chave("codigo");
                e.texto(x.codigo());
                e.chave("mensagem");
                e.texto(x.mensagem());
                if let ErroResposta::Validacao(p) = x {
                    e.chave("problemas");
                    e.lista(|e| {
                        for p in p {
                            e.item();
                            e.objeto(|e| {
                                e.chave("campo");
                                e.texto(&p.campo);
                                if let Some(n) = p.posicao {
                                    e.chave("posicao");
                                    e.inteiro(n);
                                }
                                e.chave("motivo");
                                e.texto(&p.motivo);
                            });
                        }
                    });
                }
            }
            Resposta::Lista(l) => {
                e.chave("itens");
                e.lista(|e| {
                    for i in l {
                        e.item();
                        e.objeto(|e| {
                            cabecalho(
                                e, &i.nome, i.tipo, &i.grupo, i.ambiente, &i.projeto, &i.tags,
                                i.nivel,
                            );
                            if let Some(x) = &i.endereco {
                                e.chave("endereco");
                                e.texto(x);
                            }
                            if let Some(r) = &i.resumo {
                                e.chave("resumo");
                                e.objeto(|e| {
                                    escrever_campos(e, &r.campos, &r.segredos);
                                    e.chave("extras");
                                    e.inteiro(r.extras);
                                    e.chave("anexos");
                                    e.inteiro(r.anexos);
                                });
                            }
                        });
                    }
                });
            }
            Resposta::Saida { fluxo, dados } => {
                e.chave("fluxo");
                e.texto(fluxo.nome());
                e.chave("dados");
                e.base64(dados);
            }
            Resposta::Fim { codigo } => {
                e.chave("codigo");
                e.inteiro(u64::from(*codigo));
            }
            Resposta::SessaoPlaywright { caminho } => {
                e.chave("caminho");
                e.texto(caminho);
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
                e.chave("extras");
                e.lista(|e| {
                    for x in &d.extras {
                        e.item();
                        e.objeto(|e| {
                            e.chave("nome");
                            e.texto(&x.nome);
                            e.chave("segredo");
                            e.logico(x.valor.is_none());
                            if let Some(v) = &x.valor {
                                e.chave("valor");
                                e.texto(v);
                            }
                        });
                    }
                });
                e.chave("anexos");
                e.lista(|e| {
                    for a in &d.anexos {
                        e.item();
                        e.objeto(|e| {
                            e.chave("nome");
                            e.texto(&a.nome);
                            e.chave("tamanho");
                            e.inteiro(a.tamanho);
                        });
                    }
                });
                // D65 d: só as datas que existem.
                if let Some(c) = d.datas.criada_em() {
                    e.chave("criada_em");
                    e.inteiro(c);
                }
                if let Some(a) = d.datas.alterada_em() {
                    e.chave("alterada_em");
                    e.inteiro(a);
                }
            }
            Resposta::Segredo(s) => {
                e.chave("valor");
                e.base64(s.expor());
            }
            Resposta::Token(t) => {
                e.chave("token");
                e.base64(t.expor());
            }
            Resposta::PedidoDeAprovacao(p) => {
                e.chave("id");
                e.inteiro(p.id);
                e.chave("entradas");
                escrever_textos(e, &p.entradas);
                e.chave("ambiente");
                e.texto(p.ambiente.nome());
                e.chave("operacao");
                e.texto(p.operacao.nome());
                e.chave("comando");
                e.texto(&p.comando);
                e.chave("diretorio");
                e.texto(&p.diretorio);
                e.chave("cadeia");
                escrever_textos(e, &p.cadeia);
                e.chave("origem");
                e.texto(&p.origem);
                e.chave("confirmacao");
                e.logico(p.confirmacao);
                e.chave("entrada_oculta");
                e.logico(p.entrada_oculta);
            }
            Resposta::CodigoTotp { codigo, restante } => {
                e.chave("codigo");
                e.texto(codigo);
                e.chave("restante");
                e.inteiro(*restante);
            }
            Resposta::Backups(l) => {
                e.chave("versoes");
                e.lista(|e| {
                    for v in l {
                        e.item();
                        e.objeto(|e| {
                            e.chave("nome");
                            e.texto(&v.nome);
                            e.chave("conferida");
                            e.logico(v.conferida);
                            e.chave("fora_de_ordem");
                            e.logico(v.fora_de_ordem);
                        });
                    }
                });
            }
            Resposta::Configuracao {
                minutos_trava,
                backup_externo,
                backup_local,
            } => {
                e.chave("minutos_trava");
                e.inteiro(u64::from(*minutos_trava));
                e.chave("backup_externo");
                e.logico(*backup_externo);
                e.chave("backup_local");
                e.logico(*backup_local);
            }
            Resposta::BackupExterno { ligado, uuid } => {
                e.chave("ligado");
                e.logico(*ligado);
                if let Some(u) = uuid {
                    e.chave("uuid");
                    e.texto(u);
                }
            }
            Resposta::Pendrives(l) => {
                e.chave("itens");
                e.lista(|e| {
                    for p in l {
                        e.item();
                        e.objeto(|e| {
                            e.chave("uuid");
                            e.texto(&p.uuid);
                            e.chave("rotulo");
                            e.texto(&p.rotulo);
                            e.chave("montado");
                            e.logico(p.montado);
                        });
                    }
                });
            }
            Resposta::Sugestoes(l) => {
                e.chave("itens");
                e.lista(|e| {
                    for s in l {
                        e.item();
                        e.objeto(|e| {
                            e.chave("nome");
                            e.texto(&s.nome);
                            e.chave("usuario");
                            e.texto(&s.usuario);
                            e.chave("ambiente");
                            e.texto(s.ambiente.nome());
                            e.chave("totp");
                            e.logico(s.totp);
                        });
                    }
                });
            }
            Resposta::Preenchimento {
                usuario,
                senha,
                totp,
            } => {
                e.chave("usuario");
                e.texto(usuario);
                e.chave("senha");
                e.base64(senha.expor());
                if let Some(c) = totp {
                    e.chave("totp");
                    e.texto(c);
                }
            }
            Resposta::PedidoDeLogin { id, entrada } => {
                e.chave("id");
                e.texto(id);
                e.chave("entrada");
                e.texto(entrada);
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
        crate::backup::nome_valido(n)
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
            .all(|(k, v)| CAMPOS_TEXTO.contains(&k.as_str()) && v.len() <= MAX_TEXTO)
        && segredos.len() <= CAMPOS_SECRETOS.len()
        && segredos
            .iter()
            .all(|k| CAMPOS_SECRETOS.contains(&k.as_str()))
}

fn escrever_campos(e: &mut Escritor, campos: &[(String, String)], segredos: &[String]) {
    e.chave("campos");
    e.objeto(|e| {
        for (k, v) in campos {
            e.chave(k);
            e.texto(v);
        }
    });
    e.chave("segredos");
    escrever_textos(e, segredos);
}

/// Lê `campos` e `segredos` (do `detalhe` e do resumo): na ordem do quadro, só os
/// campos de texto conhecidos (a chave repetida o JSON já recusou); a conferência no
/// fim limita a quantidade e os identificadores dos segredos.
fn ler_campos(c: &mut Campos) -> R<CamposESegredos> {
    let mut campos = Vec::new();
    for (k, v) in c.objeto("campos")?.em_ordem() {
        if !CAMPOS_TEXTO.contains(&k.as_str()) {
            return Err(JsonInvalido);
        }
        campos.push((k, texto_de(v, MAX_TEXTO)?));
    }
    let segredos = textos_de(c.lista("segredos", CAMPOS_SECRETOS.len())?, MAX_NOME)?;
    Ok((campos, segredos))
}

fn escrever_textos(e: &mut Escritor, l: &[String]) {
    e.lista(|e| {
        for t in l {
            e.item();
            e.texto(t);
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn cabecalho(
    e: &mut Escritor,
    nome: &str,
    tipo: TipoEntrada,
    grupo: &str,
    ambiente: Ambiente,
    projeto: &Option<String>,
    tags: &[String],
    nivel: Option<NivelAcesso>,
) {
    e.chave("nome");
    e.texto(nome);
    e.chave("tipo");
    e.texto(tipo.nome());
    e.chave("grupo");
    e.texto(grupo);
    e.chave("ambiente");
    e.texto(ambiente.nome());
    if let Some(p) = projeto {
        e.chave("projeto");
        e.texto(p);
    }
    e.chave("tags");
    escrever_textos(e, tags);
    if let Some(nivel) = nivel {
        e.chave("nivel");
        e.texto(nivel.nome());
    }
}

struct Cabecalho {
    nome: String,
    tipo: TipoEntrada,
    grupo: String,
    ambiente: Ambiente,
    projeto: Option<String>,
    tags: Vec<String>,
    nivel: Option<NivelAcesso>,
}

/// Lê o cabeçalho de um item ou detalhe. `com_nivel`: o nível é obrigatório (app) ou
/// proibido (lista do agente, D50).
fn ler_cabecalho(c: &mut Campos, com_nivel: bool) -> R<Cabecalho> {
    Ok(Cabecalho {
        nome: c.texto("nome", MAX_NOME)?,
        tipo: TipoEntrada::de_nome(&c.texto("tipo", MAX_NOME)?).ok_or(JsonInvalido)?,
        grupo: c.texto("grupo", MAX_TEXTO)?,
        ambiente: Ambiente::de_nome(&c.texto("ambiente", MAX_NOME)?).ok_or(JsonInvalido)?,
        projeto: c.texto_opc("projeto", MAX_TEXTO)?,
        tags: textos_de(c.lista("tags", MAX_TAGS)?, MAX_TAG)?,
        nivel: match c.texto_opc("nivel", MAX_NOME)? {
            Some(n) if com_nivel => Some(NivelAcesso::de_nome(&n).ok_or(JsonInvalido)?),
            None if !com_nivel => None,
            _ => return Err(JsonInvalido),
        },
    })
}

/// Lê os `dados` de uma resposta do tipo (o papel já foi conferido).
pub(crate) fn ler(papel: Papel, tipo: TipoResposta, mut c: Campos) -> R<Resposta> {
    let r = match tipo {
        TipoResposta::Ok => Resposta::Ok,
        TipoResposta::PedidoDeDestrave => Resposta::PedidoDeDestrave(PedidoDeDestrave {
            entrada: c.texto("entrada", MAX_TEXTO)?,
            operacao: Operacao::de_nome(&c.texto("operacao", MAX_NOME)?).ok_or(JsonInvalido)?,
            comando: c.texto("comando", MAX_TEXTO_EXIBIDO)?,
            restante: c.inteiro("restante")?,
        }),
        TipoResposta::Estado => Resposta::Estado {
            travado: c.logico("travado")?,
            cofre_existe: c.logico("cofre_existe")?,
            // Lido como veio: quem recebe confere se é o papel que esperava.
            papel: Papel::de_nome(&c.texto("papel", MAX_NOME)?).ok_or(JsonInvalido)?,
        },
        TipoResposta::Erro => {
            let codigo = c.texto("codigo", MAX_NOME)?;
            // A mensagem do quadro é ignorada: vale a fixa do código.
            let _ = c.texto("mensagem", MAX_LINHA)?;
            let mut erro = ErroResposta::de_codigo(&codigo).ok_or(JsonInvalido)?;
            if let ErroResposta::Validacao(lista) = &mut erro {
                for v in c.lista("problemas", MAX_PROBLEMAS)? {
                    let mut o = Campos::de(v)?;
                    let campo = o.texto("campo", MAX_LINHA)?;
                    let posicao = match o.tirar("posicao") {
                        Some(super::json::Valor::Inteiro(n)) => Some(n),
                        Some(_) => return Err(JsonInvalido),
                        None => None,
                    };
                    let motivo = o.texto("motivo", MAX_LINHA)?;
                    o.fim()?;
                    lista.push(Problema {
                        campo,
                        posicao,
                        motivo,
                    });
                }
            }
            Resposta::Erro(erro)
        }
        TipoResposta::Lista => {
            let mut itens = Vec::new();
            for v in c.lista("itens", MAX_ENTRADAS)? {
                let mut o = Campos::de(v)?;
                let k = ler_cabecalho(&mut o, papel == Papel::App)?;
                let endereco = o.texto_opc("endereco", MAX_URL)?;
                // D65 c: o resumo é obrigatório na lista do app e proibido na do agente.
                let resumo = match o.tirar("resumo") {
                    Some(v) if papel == Papel::App => {
                        let mut r = Campos::de(v)?;
                        let (campos, segredos) = ler_campos(&mut r)?;
                        let extras = r.inteiro("extras")?;
                        let anexos = r.inteiro("anexos")?;
                        r.fim()?;
                        Some(ResumoItem {
                            campos,
                            segredos,
                            extras,
                            anexos,
                        })
                    }
                    None if papel != Papel::App => None,
                    _ => return Err(JsonInvalido),
                };
                o.fim()?;
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
        TipoResposta::Saida => {
            let fluxo = match c.texto("fluxo", MAX_NOME)?.as_str() {
                "saida" => Fluxo::Saida,
                "erro" => Fluxo::Erro,
                _ => return Err(JsonInvalido),
            };
            Resposta::Saida {
                fluxo,
                dados: c.bytes("dados", MAX_TEXTO_EXIBIDO)?,
            }
        }
        TipoResposta::Fim => Resposta::Fim {
            codigo: u8::try_from(c.inteiro("codigo")?).map_err(|_| JsonInvalido)?,
        },
        TipoResposta::SessaoPlaywright => Resposta::SessaoPlaywright {
            caminho: c.texto("caminho", MAX_TEXTO)?,
        },
        TipoResposta::Detalhe => {
            let k = ler_cabecalho(&mut c, true)?;
            let (campos, segredos) = ler_campos(&mut c)?;
            let mut extras = Vec::new();
            for v in c.lista("extras", MAX_EXTRAS)? {
                let mut o = Campos::de(v)?;
                let nome = o.texto("nome", MAX_NOME_EXTRA)?;
                let segredo = o.logico("segredo")?;
                let valor = o.texto_opc("valor", MAX_VALOR_EXTRA)?;
                // Extra secreto nunca leva valor; o comum sempre leva.
                if segredo == valor.is_some() {
                    return Err(JsonInvalido);
                }
                o.fim()?;
                extras.push(ExtraDetalhe { nome, valor });
            }
            let mut anexos = Vec::new();
            for v in c.lista("anexos", MAX_ANEXOS)? {
                let mut o = Campos::de(v)?;
                let nome = o.texto("nome", MAX_NOME_ANEXO)?;
                let tamanho = o.inteiro("tamanho")?;
                o.fim()?;
                anexos.push(AnexoDetalhe { nome, tamanho });
            }
            // D65 d: datas opcionais, pela regra de `Datas` (faixa, criação só com
            // alteração e não depois dela).
            let mut data = |k: &str| match c.tirar(k) {
                None => Ok(None),
                Some(super::json::Valor::Inteiro(n)) => Ok(Some(n)),
                Some(_) => Err(JsonInvalido),
            };
            let criada = data("criada_em")?;
            let alterada = data("alterada_em")?;
            let datas = Datas::de_partes(criada, alterada).ok_or(JsonInvalido)?;
            Resposta::Detalhe(DetalheEntrada {
                nome: k.nome,
                tipo: k.tipo,
                grupo: k.grupo,
                ambiente: k.ambiente,
                projeto: k.projeto,
                tags: k.tags,
                nivel: k.nivel.ok_or(JsonInvalido)?,
                campos,
                segredos,
                extras,
                anexos,
                datas,
            })
        }
        TipoResposta::Segredo => Resposta::Segredo(c.segredo("valor", crate::limites::MAX_NOTA)?),
        TipoResposta::Token => Resposta::Token(c.segredo("token", MAX_NOME)?),
        TipoResposta::PedidoDeAprovacao => {
            let id = c.inteiro("id")?;
            let entradas = textos_de(c.lista("entradas", 2)?, MAX_NOME)?;
            let ambiente =
                Ambiente::de_nome(&c.texto("ambiente", MAX_NOME)?).ok_or(JsonInvalido)?;
            let operacao =
                Operacao::de_nome(&c.texto("operacao", MAX_NOME)?).ok_or(JsonInvalido)?;
            let comando = c.texto("comando", MAX_TEXTO_EXIBIDO)?;
            let diretorio = c.texto("diretorio", MAX_TEXTO_EXIBIDO)?;
            let cadeia = textos_de(c.lista("cadeia", MAX_CADEIA)?, MAX_TEXTO)?;
            let origem = c.texto("origem", MAX_TEXTO)?;
            let confirmacao = c.logico("confirmacao")?;
            let entrada_oculta = c.logico("entrada_oculta")?;
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
        TipoResposta::CodigoTotp => {
            let codigo = c.texto("codigo", 8)?;
            Resposta::CodigoTotp {
                codigo: Zeroizing::new(codigo),
                restante: c.inteiro("restante")?,
            }
        }
        TipoResposta::Sugestoes => {
            let mut itens = Vec::new();
            for v in c.lista("itens", MAX_ENTRADAS)? {
                let mut o = Campos::de(v)?;
                let nome = o.texto("nome", MAX_NOME)?;
                let usuario = o.texto("usuario", MAX_TEXTO)?;
                let ambiente =
                    Ambiente::de_nome(&o.texto("ambiente", MAX_NOME)?).ok_or(JsonInvalido)?;
                let totp = o.logico("totp")?;
                o.fim()?;
                itens.push(Sugestao {
                    nome,
                    usuario,
                    ambiente,
                    totp,
                });
            }
            Resposta::Sugestoes(itens)
        }
        TipoResposta::Backups => {
            let mut versoes = Vec::new();
            for v in c.lista("versoes", MAX_BACKUPS)? {
                let mut o = Campos::de(v)?;
                let nome = o.texto("nome", crate::limites::MAX_NOME)?;
                let conferida = o.logico("conferida")?;
                let fora_de_ordem = o.logico("fora_de_ordem")?;
                o.fim()?;
                versoes.push(VersaoBackup {
                    nome,
                    conferida,
                    fora_de_ordem,
                });
            }
            Resposta::Backups(versoes)
        }
        TipoResposta::Configuracao => Resposta::Configuracao {
            minutos_trava: u32::try_from(c.inteiro("minutos_trava")?).map_err(|_| JsonInvalido)?,
            backup_externo: c.logico("backup_externo")?,
            backup_local: c.logico("backup_local")?,
        },
        TipoResposta::BackupExterno => Resposta::BackupExterno {
            ligado: c.logico("ligado")?,
            uuid: c.texto_opc("uuid", crate::limites::MAX_UUID_BACKUP)?,
        },
        TipoResposta::Pendrives => {
            let mut itens = Vec::new();
            for v in c.lista("itens", MAX_PENDRIVES)? {
                let mut o = Campos::de(v)?;
                let uuid = o.texto("uuid", crate::limites::MAX_UUID_BACKUP)?;
                let rotulo = o.texto("rotulo", MAX_ROTULO)?;
                let montado = o.logico("montado")?;
                o.fim()?;
                itens.push(Pendrive {
                    uuid,
                    rotulo,
                    montado,
                });
            }
            Resposta::Pendrives(itens)
        }
        TipoResposta::Preenchimento => Resposta::Preenchimento {
            usuario: c.texto("usuario", MAX_TEXTO)?,
            senha: segredo_de(c.tirar("senha").ok_or(JsonInvalido)?, MAX_SEGREDO)?,
            totp: c.texto_opc("totp", 8)?.map(Zeroizing::new),
        },
        TipoResposta::PedidoDeLogin => Resposta::PedidoDeLogin {
            id: c.texto("id", MAX_NOME)?,
            entrada: c.texto("entrada", MAX_NOME)?,
        },
    };
    c.fim()?;
    r.conferir(papel)?;
    Ok(r)
}
