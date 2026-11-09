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
use super::{Operacao, TipoResposta};
use crate::entrada::{
    Ambiente, Autenticacao, Dados, Entrada, NivelAcesso, Segredo, TipoEntrada, ValorExtra,
};
use crate::limites::{
    MAX_ANEXOS, MAX_ANEXOS_SOMADOS, MAX_ENTRADAS, MAX_EXTRAS, MAX_LINHA, MAX_NOME, MAX_NOME_ANEXO,
    MAX_NOME_EXTRA, MAX_SEGREDO, MAX_TAG, MAX_TAGS, MAX_TEXTO, MAX_TEXTO_EXIBIDO, MAX_URL,
    MAX_VALOR_EXTRA,
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
/// ambiente, projeto, URL/host), mais tags e nível.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemLista {
    pub nome: String,
    pub tipo: TipoEntrada,
    pub grupo: String,
    pub ambiente: Ambiente,
    pub projeto: Option<String>,
    pub tags: Vec<String>,
    pub nivel: NivelAcesso,
    /// URL (`login`, `api`) ou host (`banco`, `servidor`, `smtp`, `ftp/sftp`).
    pub endereco: Option<String>,
}

impl ItemLista {
    /// O item de uma entrada. Quem filtra as bloqueadas para o agente é o daemon.
    pub fn de_entrada(e: &Entrada) -> Self {
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
            nivel: e.nivel(),
            endereco,
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
}

impl DetalheEntrada {
    /// O detalhe de uma entrada.
    pub fn de_entrada(e: &Entrada) -> Self {
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
        }
    }
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
    /// Quem pediu (ex.: `claude → bash → ward run`).
    pub cadeia: Vec<String>,
}

/// Entrada sugerida pela extensão.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sugestao {
    pub nome: String,
    pub usuario: String,
}

/// Resposta do daemon.
#[derive(Debug)]
pub enum Resposta {
    Ok,
    Estado {
        travado: bool,
        cofre_existe: bool,
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
    PedidoDeDestrave,
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
            Resposta::PedidoDeDestrave => TipoResposta::PedidoDeDestrave,
            Resposta::CodigoTotp { .. } => TipoResposta::CodigoTotp,
            Resposta::Sugestoes(_) => TipoResposta::Sugestoes,
            Resposta::Preenchimento { .. } => TipoResposta::Preenchimento,
        }
    }

    /// Confere os limites que a leitura confere.
    pub(crate) fn conferir(&self) -> R<()> {
        let ok = |c: bool| if c { Ok(()) } else { Err(JsonInvalido) };
        let item_ok = |i: &ItemLista| {
            i.nome.len() <= MAX_NOME
                && i.grupo.len() <= MAX_TEXTO
                && i.projeto.as_ref().is_none_or(|p| p.len() <= MAX_TEXTO)
                && i.tags.len() <= MAX_TAGS
                && i.tags.iter().all(|t| t.len() <= MAX_TAG)
                && i.endereco.as_ref().is_none_or(|p| p.len() <= MAX_URL)
        };
        match self {
            Resposta::Ok | Resposta::Estado { .. } | Resposta::PedidoDeDestrave => Ok(()),
            Resposta::Fim { .. } => Ok(()),
            Resposta::Erro(e) => ok(match e {
                ErroResposta::Validacao(p) => {
                    p.len() <= MAX_PROBLEMAS
                        && p.iter()
                            .all(|p| p.campo.len() <= MAX_LINHA && p.motivo.len() <= MAX_LINHA)
                }
                _ => true,
            }),
            Resposta::Lista(l) => ok(l.len() <= MAX_ENTRADAS && l.iter().all(item_ok)),
            Resposta::Saida { dados, .. } => ok(dados.len() <= MAX_TEXTO_EXIBIDO),
            Resposta::SessaoPlaywright { caminho } => {
                ok(!caminho.is_empty() && caminho.len() <= MAX_TEXTO)
            }
            Resposta::Detalhe(d) => ok(d.nome.len() <= MAX_NOME
                && d.grupo.len() <= MAX_TEXTO
                && d.projeto.as_ref().is_none_or(|p| p.len() <= MAX_TEXTO)
                && d.tags.len() <= MAX_TAGS
                && d.tags.iter().all(|t| t.len() <= MAX_TAG)
                && d.campos.len() <= CAMPOS_TEXTO.len()
                && d.campos
                    .iter()
                    .all(|(k, v)| CAMPOS_TEXTO.contains(&k.as_str()) && v.len() <= MAX_TEXTO)
                && d.segredos.len() <= CAMPOS_SECRETOS.len()
                && d.segredos
                    .iter()
                    .all(|k| CAMPOS_SECRETOS.contains(&k.as_str()))
                && d.extras.len() <= MAX_EXTRAS
                && d.extras.iter().all(|x| {
                    x.nome.len() <= MAX_NOME_EXTRA
                        && x.valor.as_ref().is_none_or(|v| v.len() <= MAX_VALOR_EXTRA)
                })
                && d.anexos.len() <= MAX_ANEXOS
                && d.anexos.iter().all(|a| {
                    a.nome.len() <= MAX_NOME_ANEXO && a.tamanho <= MAX_ANEXOS_SOMADOS as u64
                })),
            Resposta::Segredo(s) => ok(s.len() <= crate::limites::MAX_NOTA),
            Resposta::Token(t) => ok(!t.is_empty() && t.len() <= MAX_NOME),
            Resposta::PedidoDeAprovacao(p) => ok((1..=2).contains(&p.entradas.len())
                && p.entradas.iter().all(|n| n.len() <= MAX_NOME)
                && OPERACOES_APROVADAS.contains(&p.operacao)
                && p.comando.len() <= MAX_TEXTO_EXIBIDO
                && p.cadeia.len() <= MAX_CADEIA
                && p.cadeia.iter().all(|c| c.len() <= MAX_TEXTO)),
            Resposta::CodigoTotp { codigo, restante } => {
                ok(digitos_totp(codigo) && (1..=30).contains(restante))
            }
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
        }
    }

    /// Escreve os `dados` da resposta.
    pub(crate) fn escrever(&self, e: &mut Escritor) {
        match self {
            Resposta::Ok | Resposta::PedidoDeDestrave => {}
            Resposta::Estado {
                travado,
                cofre_existe,
            } => {
                e.chave("travado");
                e.logico(*travado);
                e.chave("cofre_existe");
                e.logico(*cofre_existe);
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
                    e, &d.nome, d.tipo, &d.grupo, d.ambiente, &d.projeto, &d.tags, d.nivel,
                );
                e.chave("campos");
                e.objeto(|e| {
                    for (k, v) in &d.campos {
                        e.chave(k);
                        e.texto(v);
                    }
                });
                e.chave("segredos");
                e.lista(|e| {
                    for k in &d.segredos {
                        e.item();
                        e.texto(k);
                    }
                });
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
                e.chave("cadeia");
                escrever_textos(e, &p.cadeia);
            }
            Resposta::CodigoTotp { codigo, restante } => {
                e.chave("codigo");
                e.texto(codigo);
                e.chave("restante");
                e.inteiro(*restante);
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
        }
    }
}

/// Problemas de validação numa resposta.
const MAX_PROBLEMAS: usize = 512;
/// Processos na cadeia mostrada na aprovação.
const MAX_CADEIA: usize = 64;

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
    nivel: NivelAcesso,
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
    e.chave("nivel");
    e.texto(nivel.nome());
}

struct Cabecalho {
    nome: String,
    tipo: TipoEntrada,
    grupo: String,
    ambiente: Ambiente,
    projeto: Option<String>,
    tags: Vec<String>,
    nivel: NivelAcesso,
}

fn ler_cabecalho(c: &mut Campos) -> R<Cabecalho> {
    Ok(Cabecalho {
        nome: c.texto("nome", MAX_NOME)?,
        tipo: TipoEntrada::de_nome(&c.texto("tipo", MAX_NOME)?).ok_or(JsonInvalido)?,
        grupo: c.texto("grupo", MAX_TEXTO)?,
        ambiente: Ambiente::de_nome(&c.texto("ambiente", MAX_NOME)?).ok_or(JsonInvalido)?,
        projeto: c.texto_opc("projeto", MAX_TEXTO)?,
        tags: textos_de(c.lista("tags", MAX_TAGS)?, MAX_TAG)?,
        nivel: NivelAcesso::de_nome(&c.texto("nivel", MAX_NOME)?).ok_or(JsonInvalido)?,
    })
}

/// Lê os `dados` de uma resposta do tipo (o papel já foi conferido).
pub(crate) fn ler(tipo: TipoResposta, mut c: Campos) -> R<Resposta> {
    let r = match tipo {
        TipoResposta::Ok => Resposta::Ok,
        TipoResposta::PedidoDeDestrave => Resposta::PedidoDeDestrave,
        TipoResposta::Estado => Resposta::Estado {
            travado: c.logico("travado")?,
            cofre_existe: c.logico("cofre_existe")?,
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
                let k = ler_cabecalho(&mut o)?;
                let endereco = o.texto_opc("endereco", MAX_URL)?;
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
            let k = ler_cabecalho(&mut c)?;
            // Na ordem do quadro; só os campos de texto conhecidos (a chave repetida o
            // JSON já recusou), e a conferência no fim limita a quantidade.
            let mut campos = Vec::new();
            for (k, v) in c.objeto("campos")?.em_ordem() {
                if !CAMPOS_TEXTO.contains(&k.as_str()) {
                    return Err(JsonInvalido);
                }
                campos.push((k, texto_de(v, MAX_TEXTO)?));
            }
            let segredos = textos_de(c.lista("segredos", CAMPOS_SECRETOS.len())?, MAX_NOME)?;
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
            Resposta::Detalhe(DetalheEntrada {
                nome: k.nome,
                tipo: k.tipo,
                grupo: k.grupo,
                ambiente: k.ambiente,
                projeto: k.projeto,
                tags: k.tags,
                nivel: k.nivel,
                campos,
                segredos,
                extras,
                anexos,
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
            let cadeia = textos_de(c.lista("cadeia", MAX_CADEIA)?, MAX_TEXTO)?;
            Resposta::PedidoDeAprovacao(PedidoDeAprovacao {
                id,
                entradas,
                ambiente,
                operacao,
                comando,
                cadeia,
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
                o.fim()?;
                itens.push(Sugestao { nome, usuario });
            }
            Resposta::Sugestoes(itens)
        }
        TipoResposta::Preenchimento => Resposta::Preenchimento {
            usuario: c.texto("usuario", MAX_TEXTO)?,
            senha: segredo_de(c.tirar("senha").ok_or(JsonInvalido)?, MAX_SEGREDO)?,
            totp: c.texto_opc("totp", 8)?.map(Zeroizing::new),
        },
    };
    c.fim()?;
    r.conferir()?;
    Ok(r)
}
