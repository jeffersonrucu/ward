//! Formulário de entrada (`Formulário / <tipo> / Novo` e `/ Edição`) dos 9 tipos:
//! `login`, `api`, `aws`, `smtp`, `nota segura` e `genérico` (Fase 33); `banco`,
//! `servidor` e `ftp/sftp` (Fase 34). Os anexos não vêm da interface: o formulário só
//! diz quais dos anexos escolhidos no lado Rust (`escolher_anexo`) vão no rascunho.
//!
//! O que a interface manda ([`Formulario`]) é não confiável: chave de campo fora da
//! lista do tipo, lista grande demais ou texto longo demais são recusados aqui
//! (`PedidoInvalido`), antes de qualquer pedido ao daemon. O formato de cada campo é
//! conferido pelo daemon (`Rascunho::validar`), que devolve os problemas por campo
//! ([`ErroCampo`]).
//!
//! Segredos (S11, D4):
//! - `"vazio"`: sem valor (campo opcional ou não informado);
//! - `{"novo": "…"}`: o que o usuário digitou agora, num [`TextoSecreto`] que zera ao
//!   sair e não tem `Debug` do valor;
//! - `"manter"`: na edição, o valor que já está no cofre. Ele não passa pela interface:
//!   o lado Rust pede ao daemon (`revelar`) e o põe direto no rascunho;
//! - `"gerada"`: a senha que o gerador do formulário guardou no lado Rust ("Usar esta
//!   senha"). Também não passa pela interface.
//!
//! TOTP (D35): a semente digitada é normalizada aqui (sem espaço, em maiúsculas); um
//! `otpauth://` colado é lido e recusado com "parâmetros de TOTP não suportados" se
//! tiver `algorithm`, `digits` ou `period` fora de SHA1, 6 e 30.

use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize};
use ward_core::entrada::{
    GRANDE_PARA_MASCARA, Rascunho, RascunhoAnexo, RascunhoDados, RascunhoExtra,
    Segredo as SegredoCore, TipoEntrada,
};
use ward_core::limites;
use ward_core::protocolo::{ErroResposta, Problema};
use zeroize::Zeroizing;

use crate::ErroApp;

/// Mensagem de D35 para um `otpauth://` com algoritmo, dígitos ou período fora de
/// SHA1, 6 e 30.
pub const TOTP_NAO_SUPORTADO: &str = "parâmetros de TOTP não suportados";
/// `otpauth://` sem `secret` ou mal formado.
pub const TOTP_URI_INVALIDA: &str = "endereço otpauth:// sem semente válida";

/// Texto secreto digitado na interface. Zera ao sair; o `Debug` não mostra o valor.
pub struct TextoSecreto(Zeroizing<String>);

impl fmt::Debug for TextoSecreto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TextoSecreto(••••)")
    }
}

impl<'de> Deserialize<'de> for TextoSecreto {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = TextoSecreto;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("texto")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<TextoSecreto, E> {
                Ok(TextoSecreto(Zeroizing::new(v.to_owned())))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<TextoSecreto, E> {
                // Toma posse do texto já lido: sem cópia que fique sem zerar.
                Ok(TextoSecreto(Zeroizing::new(v)))
            }
        }
        d.deserialize_string(V)
    }
}

/// Valor de um campo secreto do tipo. Enum com tag externa: o serde lê direto, sem
/// guardar cópia intermediária do texto.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ValorSecreto {
    Vazio,
    Manter,
    Gerada,
    Novo(TextoSecreto),
}

/// Valor de um campo extra: digitado agora, ou (na edição, só para extra secreto) o que
/// já está no cofre, pelo nome que o extra tinha.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ValorExtra {
    Novo(TextoSecreto),
    Manter(String),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtraFormulario {
    pub nome: String,
    pub segredo: bool,
    pub valor: ValorExtra,
}

/// O formulário como veio da interface.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Formulario {
    pub tipo: String,
    pub nome: String,
    pub grupo: String,
    pub ambiente: String,
    pub projeto: String,
    pub tags: Vec<String>,
    pub campos: BTreeMap<String, String>,
    pub segredos: BTreeMap<String, ValorSecreto>,
    pub extras: Vec<ExtraFormulario>,
    /// Identificadores dos anexos escolhidos no lado Rust que vão no rascunho, na ordem
    /// da tela. O conteúdo nunca passa pela interface (S11, V5.12).
    #[serde(default)]
    pub anexos: Vec<u64>,
}

/// Um problema mostrado no campo: o identificador do campo no formulário (`nome`,
/// `senha`, `totp_semente`, `extras`…, ou `formulario` quando não é de um campo), a
/// posição na lista (extras e tags, a partir de 1) e o motivo, um texto fixo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErroCampo {
    pub campo: &'static str,
    pub posicao: Option<u64>,
    pub motivo: String,
}

/// Tipo do formulário: os 9 tipos (Fases 33 e 34).
pub fn tipo_do_formulario(nome: &str) -> Option<TipoEntrada> {
    TipoEntrada::de_nome(nome)
}

/// Campos de texto e segredos de cada tipo, com os identificadores do protocolo.
pub fn campos_do_tipo(tipo: TipoEntrada) -> (&'static [&'static str], &'static [&'static str]) {
    match tipo {
        TipoEntrada::Login => (&["url", "usuario"], &["senha", "totp_semente"]),
        TipoEntrada::Api => (&["url", "variavel"], &["token"]),
        TipoEntrada::Aws => (
            &["access_key_id", "regiao", "role_arn"],
            &["secret_access_key", "session_token"],
        ),
        TipoEntrada::Smtp => (&["host", "porta", "usuario", "seguranca"], &["senha"]),
        TipoEntrada::NotaSegura => (&[], &["texto"]),
        TipoEntrada::Banco => (
            &["motor", "host", "porta", "base", "usuario", "tunel"],
            &["senha"],
        ),
        TipoEntrada::Servidor => (
            &["host", "porta", "usuario", "salto"],
            &["senha", "chave_privada", "passphrase"],
        ),
        TipoEntrada::FtpSftp => (
            &["protocolo", "host", "porta", "usuario"],
            &["senha", "chave_privada", "passphrase"],
        ),
        TipoEntrada::Generico => (&[], &[]),
    }
}

/// Campo do formulário a partir do nome que o `ward-core` usa no problema de
/// validação. O que não é de um campo vai para `formulario`.
fn campo_do_problema(nome: &str) -> &'static str {
    match nome {
        "nome" => "nome",
        "grupo" => "grupo",
        "ambiente" => "ambiente",
        "projeto" => "projeto",
        "tags" => "tags",
        "url" => "url",
        "usuário" => "usuario",
        "senha" => "senha",
        "semente TOTP" => "totp_semente",
        "token" => "token",
        "variável" => "variavel",
        "access key ID" => "access_key_id",
        "secret access key" => "secret_access_key",
        "região" => "regiao",
        "role ARN" => "role_arn",
        "session token" => "session_token",
        "host" => "host",
        "porta" => "porta",
        "segurança" => "seguranca",
        "texto" => "texto",
        "campos extras" => "extras",
        "motor" => "motor",
        "base" => "base",
        "túnel SSH" => "tunel",
        "servidor de salto" => "salto",
        "protocolo" => "protocolo",
        "chave privada" => "chave_privada",
        "passphrase" => "passphrase",
        "autenticação" => "autenticacao",
        "anexos" => "anexos",
        _ => "formulario",
    }
}

/// Maior motivo aceito da resposta do daemon.
const MAX_MOTIVO: usize = 200;

/// Problemas do daemon como erros de campo. A resposta do daemon não é confiável
/// (S1): o campo sai de uma tabela fixa e o motivo só passa se for texto curto, sem
/// controle nem formatação Unicode; senão vira "inválido".
pub fn erros_do_daemon(problemas: &[Problema]) -> Vec<ErroCampo> {
    problemas
        .iter()
        .take(limites::MAX_EXTRAS + 32)
        .map(|p| {
            let limpo = p.motivo.chars().count() <= MAX_MOTIVO
                && !p.motivo.is_empty()
                && !p.motivo.chars().any(|c| c.is_control() || formatacao(c));
            // D29: a recusa do orçamento da máscara vem da entrada inteira; a tela a
            // mostra no campo dos anexos, o que mais pesa no orçamento.
            let campo = if p.campo == "entrada" && p.motivo == GRANDE_PARA_MASCARA {
                "anexos"
            } else {
                campo_do_problema(&p.campo)
            };
            ErroCampo {
                campo,
                posicao: p.posicao.filter(|n| (1..=1024).contains(n)),
                motivo: if limpo {
                    p.motivo.clone()
                } else {
                    "inválido".to_owned()
                },
            }
        })
        .collect()
}

fn formatacao(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200F}' | '\u{2028}'..='\u{202E}' | '\u{2060}'..='\u{206F}' | '\u{FEFF}' | '\u{E0000}'..='\u{E007F}')
}

/// Segredo pronto para o rascunho, ou "sem valor".
pub type SegredoMontado = Option<SegredoCore>;

/// Limites do que a interface pode mandar, antes de qualquer validação de formato. A
/// validação fina (e a mensagem por campo) é do `ward-core`; isto só barra entrada
/// absurda (memória, quadro).
fn conferir_limites(f: &Formulario) -> Result<(), ErroApp> {
    let texto_ok = |s: &str| s.len() <= limites::MAX_TEXTO;
    let ok = texto_ok(&f.nome)
        && texto_ok(&f.grupo)
        && texto_ok(&f.ambiente)
        && texto_ok(&f.projeto)
        && f.tipo.len() <= 32
        && f.tags.len() <= 2 * limites::MAX_TAGS
        && f.tags.iter().all(|t| texto_ok(t))
        && f.campos.len() <= 8
        && f.campos.values().all(|v| texto_ok(v))
        && f.segredos.len() <= 8
        && f.segredos.values().all(|v| match v {
            ValorSecreto::Novo(t) => t.0.len() <= limites::MAX_NOTA,
            _ => true,
        })
        && f.anexos.len() <= limites::MAX_ANEXOS + 1
        && f.extras.len() <= limites::MAX_EXTRAS + 1
        && f.extras.iter().all(|x| {
            texto_ok(&x.nome)
                && match &x.valor {
                    ValorExtra::Novo(t) => t.0.len() <= limites::MAX_VALOR_EXTRA,
                    ValorExtra::Manter(n) => texto_ok(n),
                }
        });
    if ok {
        Ok(())
    } else {
        Err(ErroApp::PedidoInvalido)
    }
}

/// Lê um `otpauth://totp/…?secret=…` (D35). Devolve a semente normalizada, ou o motivo
/// da recusa. Parâmetros desconhecidos (`issuer`, `image`…) são ignorados; repetidos
/// são recusados.
fn semente_de_otpauth(uri: &str) -> Result<Zeroizing<String>, &'static str> {
    let resto = &uri["otpauth://".len()..];
    let (tipo, depois) = resto.split_once('/').ok_or(TOTP_URI_INVALIDA)?;
    if !tipo.eq_ignore_ascii_case("totp") {
        return Err(TOTP_NAO_SUPORTADO);
    }
    let consulta = depois.split_once('?').map(|(_, q)| q).unwrap_or("");
    let consulta = consulta.split_once('#').map(|(q, _)| q).unwrap_or(consulta);
    let mut semente: Option<Zeroizing<String>> = None;
    let mut vistos: Vec<String> = Vec::new();
    for par in consulta.split('&').filter(|p| !p.is_empty()) {
        let (chave, valor) = par.split_once('=').unwrap_or((par, ""));
        let chave = chave.to_ascii_lowercase();
        if vistos.contains(&chave) {
            return Err(TOTP_URI_INVALIDA);
        }
        let valor = decodificar_pct(valor).ok_or(TOTP_URI_INVALIDA)?;
        match chave.as_str() {
            "secret" => semente = Some(valor),
            "algorithm" if !valor.eq_ignore_ascii_case("SHA1") => return Err(TOTP_NAO_SUPORTADO),
            "digits" if valor.as_str() != "6" => return Err(TOTP_NAO_SUPORTADO),
            "period" if valor.as_str() != "30" => return Err(TOTP_NAO_SUPORTADO),
            _ => {}
        }
        vistos.push(chave);
    }
    let semente = semente.ok_or(TOTP_URI_INVALIDA)?;
    Ok(normalizar_base32(&semente))
}

/// `%XX` → byte; `+` fica como está (não é espaço numa semente). UTF-8 obrigatório.
fn decodificar_pct(v: &str) -> Option<Zeroizing<String>> {
    let b = v.as_bytes();
    let mut saida = Zeroizing::new(Vec::with_capacity(b.len()));
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = b.get(i + 1..i + 3)?;
            let alto = (hex[0] as char).to_digit(16)?;
            let baixo = (hex[1] as char).to_digit(16)?;
            saida.push(u8::try_from(alto * 16 + baixo).ok()?);
            i += 3;
        } else {
            saida.push(b[i]);
            i += 1;
        }
    }
    let texto = core::str::from_utf8(&saida).ok()?;
    Some(Zeroizing::new(texto.to_owned()))
}

/// Semente base32 como o cofre guarda (D25): sem espaço e em maiúsculas.
fn normalizar_base32(s: &str) -> Zeroizing<String> {
    let mut saida = Zeroizing::new(String::with_capacity(s.len()));
    for c in s.chars().filter(|c| !c.is_whitespace()) {
        saida.push(c.to_ascii_uppercase());
    }
    saida
}

/// A semente TOTP digitada ou colada, pronta para o rascunho (D25, D35).
pub fn normalizar_semente(digitada: &str) -> Result<Zeroizing<String>, &'static str> {
    let aparada = digitada.trim();
    let prefixo = aparada.get(..10).unwrap_or("");
    if prefixo.eq_ignore_ascii_case("otpauth://") {
        semente_de_otpauth(aparada)
    } else {
        Ok(normalizar_base32(aparada))
    }
}

/// De onde vêm os valores que não estão no formulário: o cofre (`revelar`, na edição) e
/// a senha que o gerador do formulário guardou.
pub trait Fontes {
    /// Valor guardado de um campo secreto (`Alvo::Campo`) ou de um extra secreto
    /// (`Alvo::Extra`) da entrada editada.
    fn guardado(&mut self, alvo: ward_core::protocolo::Alvo) -> Result<SegredoCore, ErroApp>;
    /// Senha gerada para este campo ("Usar esta senha").
    fn gerada(&mut self, campo: &str) -> Result<SegredoCore, ErroApp>;
}

/// O que falta para salvar: o rascunho dos dados e dos extras, ou o motivo.
pub enum Falta {
    App(ErroApp),
    Campos(Vec<ErroCampo>),
}

impl From<ErroApp> for Falta {
    fn from(e: ErroApp) -> Self {
        Falta::App(e)
    }
}

/// Partes do rascunho que dependem do tipo.
pub struct Montado {
    pub dados: RascunhoDados,
    pub extras: Vec<RascunhoExtra>,
}

/// Monta os dados do tipo e os extras. `editando` diz se é edição (só assim vale
/// `"manter"`); `segredos_guardados` e `extras_guardados` são os que o detalhe da
/// entrada editada lista.
pub fn montar(
    f: &mut Formulario,
    tipo: TipoEntrada,
    editando: Option<(&[String], &[String])>,
    fontes: &mut dyn Fontes,
) -> Result<Montado, Falta> {
    conferir_limites(f)?;
    let (textos, segredos) = campos_do_tipo(tipo);
    if f.campos.keys().any(|k| !textos.contains(&k.as_str()))
        || f.segredos.keys().any(|k| !segredos.contains(&k.as_str()))
    {
        return Err(ErroApp::PedidoInvalido.into());
    }
    let mut erros = Vec::new();
    let mut tirar = |id: &str| f.campos.remove(id).unwrap_or_default();
    let (url, usuario, variavel, access, regiao, arn, host, porta, seguranca) = (
        tirar("url"),
        tirar("usuario"),
        tirar("variavel"),
        tirar("access_key_id"),
        tirar("regiao"),
        tirar("role_arn"),
        tirar("host"),
        tirar("porta"),
        tirar("seguranca"),
    );
    let (motor, base, tunel, salto, protocolo) = (
        tirar("motor"),
        tirar("base"),
        tirar("tunel"),
        tirar("salto"),
        tirar("protocolo"),
    );
    let mut segredo = |id: &'static str| -> Result<SegredoMontado, Falta> {
        let valor = f.segredos.remove(id).unwrap_or(ValorSecreto::Vazio);
        Ok(match valor {
            ValorSecreto::Vazio => None,
            ValorSecreto::Novo(t) if t.0.is_empty() => None,
            ValorSecreto::Novo(t) if id == "totp_semente" => match normalizar_semente(&t.0) {
                Ok(s) => Some(SegredoCore::de_bytes(s.as_bytes().to_vec())),
                Err(motivo) => {
                    erros.push(ErroCampo {
                        campo: "totp_semente",
                        posicao: None,
                        motivo: motivo.to_owned(),
                    });
                    None
                }
            },
            ValorSecreto::Novo(t) => Some(SegredoCore::de_bytes(t.0.as_bytes().to_vec())),
            ValorSecreto::Manter => match editando {
                Some((guardados, _)) if guardados.iter().any(|g| g == id) => {
                    Some(fontes.guardado(ward_core::protocolo::Alvo::Campo(id.to_owned()))?)
                }
                _ => return Err(ErroApp::PedidoInvalido.into()),
            },
            ValorSecreto::Gerada => Some(fontes.gerada(id)?),
        })
    };
    let dados = match tipo {
        TipoEntrada::Login => RascunhoDados::Login {
            url,
            usuario,
            senha: segredo("senha")?,
            totp_semente: segredo("totp_semente")?,
        },
        TipoEntrada::Api => RascunhoDados::Api {
            url,
            token: segredo("token")?,
            variavel,
        },
        TipoEntrada::Aws => RascunhoDados::Aws {
            access_key_id: access,
            secret_access_key: segredo("secret_access_key")?,
            regiao,
            role_arn: arn,
            session_token: segredo("session_token")?,
        },
        TipoEntrada::Smtp => RascunhoDados::Smtp {
            host,
            porta,
            usuario,
            senha: segredo("senha")?,
            seguranca,
        },
        TipoEntrada::NotaSegura => RascunhoDados::NotaSegura {
            texto: segredo("texto")?,
        },
        TipoEntrada::Banco => RascunhoDados::Banco {
            motor,
            host,
            porta,
            base,
            usuario,
            senha: segredo("senha")?,
            tunel,
        },
        TipoEntrada::Servidor => RascunhoDados::Servidor {
            host,
            porta,
            usuario,
            senha: segredo("senha")?,
            chave_privada: segredo("chave_privada")?,
            passphrase: segredo("passphrase")?,
            salto,
        },
        TipoEntrada::FtpSftp => RascunhoDados::FtpSftp {
            protocolo,
            host,
            porta,
            usuario,
            senha: segredo("senha")?,
            chave_privada: segredo("chave_privada")?,
            passphrase: segredo("passphrase")?,
        },
        TipoEntrada::Generico => RascunhoDados::Generico,
    };
    let mut extras = Vec::with_capacity(f.extras.len());
    for x in f.extras.drain(..) {
        let valor = match x.valor {
            ValorExtra::Novo(t) => SegredoCore::de_bytes(t.0.as_bytes().to_vec()),
            ValorExtra::Manter(nome) => match editando {
                Some((_, guardados)) if guardados.contains(&nome) => {
                    fontes.guardado(ward_core::protocolo::Alvo::Extra(nome))?
                }
                _ => return Err(ErroApp::PedidoInvalido.into()),
            },
        };
        extras.push(RascunhoExtra {
            nome: x.nome,
            valor,
            segredo: x.segredo,
        });
    }
    if erros.is_empty() {
        Ok(Montado { dados, extras })
    } else {
        Err(Falta::Campos(erros))
    }
}

fn copiar_segredo(s: &SegredoCore) -> SegredoCore {
    SegredoCore::de_bytes(s.expor().to_vec())
}

fn copiar_opcional(s: &Option<SegredoCore>) -> Option<SegredoCore> {
    s.as_ref().map(copiar_segredo)
}

/// Cópia do rascunho (os segredos copiados também zeram ao sair) para a validação local
/// antes do pedido: os erros de todos os campos aparecem juntos e com as mensagens fixas
/// do `ward-core`, e um rascunho fora do formato do protocolo (nome vazio, tags demais)
/// nem vira pedido. O daemon valida de novo.
pub fn copiar_rascunho(r: &Rascunho) -> Rascunho {
    let dados = match &r.dados {
        RascunhoDados::Login {
            url,
            usuario,
            senha,
            totp_semente,
        } => RascunhoDados::Login {
            url: url.clone(),
            usuario: usuario.clone(),
            senha: copiar_opcional(senha),
            totp_semente: copiar_opcional(totp_semente),
        },
        RascunhoDados::Api {
            url,
            token,
            variavel,
        } => RascunhoDados::Api {
            url: url.clone(),
            token: copiar_opcional(token),
            variavel: variavel.clone(),
        },
        RascunhoDados::Aws {
            access_key_id,
            secret_access_key,
            regiao,
            role_arn,
            session_token,
        } => RascunhoDados::Aws {
            access_key_id: access_key_id.clone(),
            secret_access_key: copiar_opcional(secret_access_key),
            regiao: regiao.clone(),
            role_arn: role_arn.clone(),
            session_token: copiar_opcional(session_token),
        },
        RascunhoDados::Smtp {
            host,
            porta,
            usuario,
            senha,
            seguranca,
        } => RascunhoDados::Smtp {
            host: host.clone(),
            porta: porta.clone(),
            usuario: usuario.clone(),
            senha: copiar_opcional(senha),
            seguranca: seguranca.clone(),
        },
        RascunhoDados::NotaSegura { texto } => RascunhoDados::NotaSegura {
            texto: copiar_opcional(texto),
        },
        RascunhoDados::Banco {
            motor,
            host,
            porta,
            base,
            usuario,
            senha,
            tunel,
        } => RascunhoDados::Banco {
            motor: motor.clone(),
            host: host.clone(),
            porta: porta.clone(),
            base: base.clone(),
            usuario: usuario.clone(),
            senha: copiar_opcional(senha),
            tunel: tunel.clone(),
        },
        RascunhoDados::Servidor {
            host,
            porta,
            usuario,
            senha,
            chave_privada,
            passphrase,
            salto,
        } => RascunhoDados::Servidor {
            host: host.clone(),
            porta: porta.clone(),
            usuario: usuario.clone(),
            senha: copiar_opcional(senha),
            chave_privada: copiar_opcional(chave_privada),
            passphrase: copiar_opcional(passphrase),
            salto: salto.clone(),
        },
        RascunhoDados::FtpSftp {
            protocolo,
            host,
            porta,
            usuario,
            senha,
            chave_privada,
            passphrase,
        } => RascunhoDados::FtpSftp {
            protocolo: protocolo.clone(),
            host: host.clone(),
            porta: porta.clone(),
            usuario: usuario.clone(),
            senha: copiar_opcional(senha),
            chave_privada: copiar_opcional(chave_privada),
            passphrase: copiar_opcional(passphrase),
        },
        RascunhoDados::Generico => RascunhoDados::Generico,
    };
    Rascunho {
        nome: r.nome.clone(),
        grupo: r.grupo.clone(),
        ambiente: r.ambiente.clone(),
        projeto: r.projeto.clone(),
        tags: r.tags.clone(),
        dados,
        extras: r
            .extras
            .iter()
            .map(|x| RascunhoExtra {
                nome: x.nome.clone(),
                valor: copiar_segredo(&x.valor),
                segredo: x.segredo,
            })
            .collect(),
        anexos: r
            .anexos
            .iter()
            .map(|a| RascunhoAnexo {
                nome: a.nome.clone(),
                conteudo: copiar_segredo(&a.conteudo),
            })
            .collect(),
    }
}

/// Valida o rascunho no lado do app (cópia); `Err` com os erros por campo.
pub fn validar_local(r: &Rascunho) -> Result<(), Vec<ErroCampo>> {
    match copiar_rascunho(r).validar() {
        Ok(entrada) => {
            drop(entrada);
            Ok(())
        }
        Err(e) => Err(erros_do_daemon(&match ErroResposta::de_validacao(&e) {
            ErroResposta::Validacao(p) => p,
            _ => Vec::new(),
        })),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn semente_digitada_e_normalizada() {
        assert_eq!(
            normalizar_semente(" jbsw y3dp ehpk 3pxp ")
                .unwrap()
                .as_str(),
            "JBSWY3DPEHPK3PXP"
        );
        assert_eq!(
            normalizar_semente("otpauth://totp/Acme:wp?secret=jbswy3dpehpk3pxp&issuer=Acme")
                .unwrap()
                .as_str(),
            "JBSWY3DPEHPK3PXP"
        );
        assert_eq!(
            normalizar_semente(
                "OTPAUTH://TOTP/x?SECRET=JBSWY3DPEHPK3PXP&algorithm=sha1&digits=6&period=30"
            )
            .unwrap()
            .as_str(),
            "JBSWY3DPEHPK3PXP"
        );
    }

    /// D35/V5.13: `otpauth://` fora de SHA1/6/30 é recusado com a mensagem fixa; HOTP
    /// também. URI sem semente, repetida ou com `%` quebrado é recusada.
    #[test]
    fn ataque_otpauth_fora_de_sha1_6_30_e_recusado() {
        for uri in [
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&algorithm=SHA256",
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&algorithm=SHA512",
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&digits=8",
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&digits=06",
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&period=60",
            "otpauth://totp/x?period=60&secret=JBSWY3DPEHPK3PXP",
            "otpauth://hotp/x?secret=JBSWY3DPEHPK3PXP&counter=1",
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&algorithm=%53HA256",
        ] {
            assert_eq!(normalizar_semente(uri), Err(TOTP_NAO_SUPORTADO), "{uri}");
        }
        for uri in [
            "otpauth://totp/x?issuer=Acme",
            "otpauth://totp",
            "otpauth://totp/x?secret=AAAA&secret=BBBB",
            "otpauth://totp/x?secret=JBSW%Z3",
            "otpauth://totp/x?secret=%ff%fe",
        ] {
            assert_eq!(normalizar_semente(uri), Err(TOTP_URI_INVALIDA), "{uri}");
        }
    }

    #[test]
    fn ataque_problema_forjado_pelo_daemon_nao_vira_texto_livre() {
        let p = |campo: &str, motivo: &str| Problema {
            campo: campo.to_owned(),
            posicao: None,
            motivo: motivo.to_owned(),
        };
        let e = erros_do_daemon(&[
            p("senha", "obrigatório"),
            p("<img src=x>", "obrigatório"),
            p("usuário", "a\u{202E}b"),
            p("nome", &"x".repeat(500)),
            p("url", "linha\nnova"),
        ]);
        assert_eq!(e[0].campo, "senha");
        assert_eq!(e[0].motivo, "obrigatório");
        assert_eq!(e[1].campo, "formulario");
        assert_eq!(e[2].motivo, "inválido");
        assert_eq!(e[3].motivo, "inválido");
        assert_eq!(e[4].motivo, "inválido");
    }

    /// D29/V5.12: a recusa do orçamento da máscara (campo "entrada") aparece no campo
    /// dos anexos; outro motivo em "entrada" não vira erro de anexo.
    #[test]
    fn recusa_do_orcamento_da_mascara_vai_para_os_anexos() {
        let p = |campo: &str, motivo: &str| Problema {
            campo: campo.to_owned(),
            posicao: None,
            motivo: motivo.to_owned(),
        };
        let e = erros_do_daemon(&[
            p("entrada", GRANDE_PARA_MASCARA),
            p("entrada", "outra coisa"),
            p("túnel SSH", "precisa ser uma entrada servidor existente"),
            p("autenticação", "informe a senha ou a chave"),
        ]);
        assert_eq!(e[0].campo, "anexos");
        assert_eq!(e[0].motivo, GRANDE_PARA_MASCARA);
        assert_eq!(e[1].campo, "formulario");
        assert_eq!(e[2].campo, "tunel");
        assert_eq!(e[3].campo, "autenticacao");
    }

    #[test]
    fn ataque_debug_do_texto_secreto_nao_mostra_o_valor() {
        let f: Formulario = serde_json::from_str(
            r#"{"tipo":"login","nome":"x","grupo":"g","ambiente":"dev","projeto":"","tags":[],
                "campos":{},"segredos":{"senha":{"novo":"falso-segredo-123"}},"extras":[
                {"nome":"A","segredo":true,"valor":{"novo":"falso-extra-456"}}]}"#,
        )
        .unwrap();
        let texto = format!("{f:?}");
        assert!(!texto.contains("falso"), "{texto}");
        assert!(texto.contains("••••"));
    }

    #[test]
    fn ataque_formulario_com_chave_desconhecida_e_recusado() {
        for json in [
            r#"{"tipo":"login","nome":"x","grupo":"g","ambiente":"dev","projeto":"","tags":[],"campos":{},"segredos":{},"extras":[],"nivel":"livre"}"#,
            r#"{"tipo":"login","nome":"x","grupo":"g","ambiente":"dev","projeto":"","tags":[],"campos":{},"segredos":{"senha":{"revelar":"x"}},"extras":[]}"#,
            r#"{"tipo":"login","nome":"x","grupo":"g","ambiente":"dev","projeto":"","tags":[],"campos":{},"segredos":{},"extras":[{"nome":"A","segredo":true,"valor":{"novo":"v"},"x":1}]}"#,
            r#"{"tipo":"login","nome":"x","grupo":"g","ambiente":"dev","projeto":"","tags":[],"campos":{},"segredos":{"senha":{"novo":5}},"extras":[]}"#,
        ] {
            assert!(serde_json::from_str::<Formulario>(json).is_err(), "{json}");
        }
    }
}
