//! O que a tela Credenciais (Fase 32) recebe: a lista e o detalhe de uma entrada,
//! montados das respostas `lista` e `detalhe` do protocolo. A 2ª linha e o usuário da
//! busca saem do `resumo` de cada item da lista do app (D65 c), numa troca só, sem um
//! `detalhe` por entrada.
//!
//! S11/D4: nada daqui tem segredo. Dos segredos vai só o identificador e o rótulo do
//! campo (para o `Campo secreto` desenhar a máscara); dos extras secretos, só o nome;
//! dos anexos, nome e tamanho. Os tipos implementam `Serialize` para o Tauri e para a
//! ponte de teste; o `Segredo` do app não implementa, então não há como um valor
//! secreto entrar aqui por engano.

use serde::Serialize;
use ward_core::entrada::TipoEntrada;
use ward_core::protocolo::{DetalheEntrada, ItemLista, ResumoItem};

/// Uma linha da lista (`Item de lista` compacto).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemCredencial {
    pub nome: String,
    /// Identificador do tipo (`banco`, `ftp-sftp`, `nota-segura`…).
    pub tipo: &'static str,
    pub grupo: String,
    pub ambiente: &'static str,
    /// `bloqueado`, `com-aprovacao` ou `livre`.
    pub nivel: &'static str,
    /// Segunda linha do item (`postgres · app@127.0.0.1:55432`).
    pub resumo: String,
    /// Host ou URL, para a busca.
    pub endereco: Option<String>,
    /// Usuário, para a busca.
    pub usuario: Option<String>,
}

/// Campo de texto do detalhe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CampoTexto {
    /// Identificador do protocolo (`host`, `usuario`…), o que o "copiar" usa.
    pub id: String,
    pub rotulo: String,
    pub valor: String,
}

/// Campo secreto do detalhe: só o identificador e o rótulo, nunca o valor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CampoSecreto {
    pub id: String,
    pub rotulo: String,
}

/// Campo extra: o valor só quando não é segredo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Extra {
    pub nome: String,
    pub segredo: bool,
    pub valor: Option<String>,
}

/// Anexo: nome e tamanho, sem o conteúdo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Anexo {
    pub nome: String,
    pub tamanho: u64,
}

/// O painel de detalhe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DetalheCredencial {
    pub nome: String,
    pub tipo: &'static str,
    pub grupo: String,
    pub ambiente: &'static str,
    pub nivel: &'static str,
    /// Projeto e tags, para o formulário de edição (Fase 33).
    pub projeto: Option<String>,
    pub tags: Vec<String>,
    /// Linha ao lado dos selos (`grupo acme · postgres`).
    pub subtitulo: String,
    pub campos: Vec<CampoTexto>,
    pub segredos: Vec<CampoSecreto>,
    pub extras: Vec<Extra>,
    pub anexos: Vec<Anexo>,
    /// Datas do daemon (D65 d), em segundos UTC; a tela mostra no fuso local. `None`:
    /// a entrada não tem a data (as do `make dev-cofre`), e o rodapé não a mostra.
    pub criada_em: Option<u64>,
    pub alterada_em: Option<u64>,
}

/// Rótulo da interface de um campo do protocolo (o identificador, se for novo).
pub fn rotulo_do_campo(id: &str) -> String {
    match id {
        "url" => "URL",
        "usuario" => "Usuário",
        "motor" => "Motor",
        "host" => "Host",
        "porta" => "Porta",
        "base" => "Base",
        "tunel" => "Túnel SSH",
        "salto" => "Salto",
        "variavel" => "Variável",
        "access_key_id" => "Access key ID",
        "regiao" => "Região",
        "role_arn" => "Role ARN",
        "seguranca" => "Segurança",
        "protocolo" => "Protocolo",
        "senha" => "Senha",
        "totp_semente" => "Semente TOTP",
        "chave_privada" => "Chave privada",
        "passphrase" => "Passphrase",
        "token" => "Token",
        "secret_access_key" => "Secret access key",
        "session_token" => "Session token",
        "texto" => "Texto",
        outro => return outro.to_owned(),
    }
    .to_owned()
}

fn campo<'a>(campos: &'a [(String, String)], id: &str) -> Option<&'a str> {
    campos
        .iter()
        .find(|(k, _)| k == id)
        .map(|(_, v)| v.as_str())
}

/// `usuário@host:porta`, sem as partes que faltam.
fn conexao(usuario: Option<&str>, host: Option<&str>, porta: Option<&str>) -> String {
    let mut s = String::new();
    if let Some(u) = usuario {
        s.push_str(u);
        s.push('@');
    }
    s.push_str(host.unwrap_or_default());
    if let Some(p) = porta {
        s.push(':');
        s.push_str(p);
    }
    s
}

fn juntar(partes: &[&str]) -> String {
    partes
        .iter()
        .filter(|p| !p.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" · ")
}

/// A segunda linha do item, pelo tipo (como nos frames `Credenciais / *`), do resumo
/// da lista do app.
fn resumo(tipo: TipoEntrada, r: &ResumoItem) -> String {
    let c = |id: &str| campo(&r.campos, id);
    let tem = |id: &str| r.segredos.iter().any(|s| s == id);
    match tipo {
        TipoEntrada::Banco => {
            let motor = c("motor").unwrap_or_default();
            if c("tunel").is_some() {
                juntar(&[motor, &conexao(None, c("host"), c("porta")), "túnel SSH"])
            } else {
                juntar(&[motor, &conexao(c("usuario"), c("host"), c("porta"))])
            }
        }
        TipoEntrada::Servidor | TipoEntrada::FtpSftp => {
            let acesso = if c("salto").is_some() {
                "salto"
            } else if tem("chave_privada") {
                "chave"
            } else {
                "senha"
            };
            let protocolo = c("protocolo").unwrap_or_default();
            juntar(&[
                protocolo,
                &conexao(c("usuario"), c("host"), c("porta")),
                acesso,
            ])
        }
        TipoEntrada::Smtp => juntar(&[
            &conexao(c("usuario"), c("host"), c("porta")),
            c("seguranca").unwrap_or_default(),
        ]),
        TipoEntrada::Login => juntar(&[
            c("usuario").unwrap_or_default(),
            c("url").unwrap_or_default(),
            if tem("totp_semente") { "TOTP" } else { "" },
        ]),
        TipoEntrada::Api => juntar(&[
            c("variavel").unwrap_or_default(),
            c("url").unwrap_or_default(),
        ]),
        TipoEntrada::Aws => juntar(&[
            c("access_key_id").unwrap_or_default(),
            c("regiao").unwrap_or_default(),
        ]),
        TipoEntrada::NotaSegura => "texto protegido".to_owned(),
        TipoEntrada::Generico => match (r.extras, r.anexos) {
            (0, 0) => "sem campos".to_owned(),
            (e, a) => juntar(&[
                &plural(e, "campo extra", "campos extras"),
                &plural(a, "anexo", "anexos"),
            ]),
        },
    }
}

fn plural(n: u64, um: &str, varios: &str) -> String {
    match n {
        0 => String::new(),
        1 => format!("1 {um}"),
        n => format!("{n} {varios}"),
    }
}

impl ItemCredencial {
    /// O item da lista do app, com a 2ª linha e o usuário tirados do resumo (D65 c).
    /// `None` quando o item não tem nível ou resumo: não é lista do papel app (D50), e
    /// quem chama recusa a resposta inteira.
    pub fn novo(item: &ItemLista) -> Option<ItemCredencial> {
        let r = item.resumo.as_ref()?;
        Some(ItemCredencial {
            nome: item.nome.clone(),
            tipo: item.tipo.nome(),
            grupo: item.grupo.clone(),
            ambiente: item.ambiente.nome(),
            nivel: item.nivel?.nome(),
            resumo: resumo(item.tipo, r),
            endereco: item.endereco.clone(),
            usuario: campo(&r.campos, "usuario").map(str::to_owned),
        })
    }
}

impl DetalheCredencial {
    pub fn novo(d: &DetalheEntrada) -> DetalheCredencial {
        let motor = campo(&d.campos, "motor");
        let protocolo = campo(&d.campos, "protocolo");
        let grupo = format!("grupo {}", d.grupo);
        let subtitulo = juntar(&[
            &grupo,
            motor.or(protocolo).unwrap_or_default(),
            d.projeto.as_deref().unwrap_or_default(),
        ]);
        DetalheCredencial {
            nome: d.nome.clone(),
            tipo: d.tipo.nome(),
            grupo: d.grupo.clone(),
            ambiente: d.ambiente.nome(),
            nivel: d.nivel.nome(),
            projeto: d.projeto.clone(),
            tags: d.tags.clone(),
            subtitulo,
            // O motor e o protocolo já estão no subtítulo.
            campos: d
                .campos
                .iter()
                .filter(|(k, _)| k != "motor" && k != "protocolo")
                .map(|(k, v)| CampoTexto {
                    id: k.clone(),
                    rotulo: rotulo_do_campo(k),
                    valor: v.clone(),
                })
                .collect(),
            segredos: d
                .segredos
                .iter()
                .map(|k| CampoSecreto {
                    id: k.clone(),
                    rotulo: rotulo_do_campo(k),
                })
                .collect(),
            extras: d
                .extras
                .iter()
                .map(|x| Extra {
                    nome: x.nome.clone(),
                    segredo: x.valor.is_none(),
                    valor: x.valor.clone(),
                })
                .collect(),
            anexos: d
                .anexos
                .iter()
                .map(|a| Anexo {
                    nome: a.nome.clone(),
                    tamanho: a.tamanho,
                })
                .collect(),
            criada_em: d.datas.criada_em(),
            alterada_em: d.datas.alterada_em(),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::daemon::Daemon;
    use crate::daemon_falso::{DaemonFalso, SEGREDOS};
    use ward_core::entrada::{NivelAcesso, Rascunho, RascunhoDados, Segredo};
    use ward_core::protocolo::{self, Papel, Pedido, Resposta};

    fn trocar(d: &mut DaemonFalso, pedido: &Pedido) -> Resposta {
        let q = protocolo::codificar_pedido(Papel::App, pedido).unwrap();
        let r = d.trocar(&q).unwrap();
        protocolo::decodificar_resposta(Papel::App, &r).unwrap()
    }

    fn detalhe(d: &mut DaemonFalso, nome: &str) -> DetalheCredencial {
        let p = Pedido::Detalhe {
            entrada: nome.to_owned(),
        };
        match trocar(d, &p) {
            Resposta::Detalhe(x) => DetalheCredencial::novo(&x),
            outra => panic!("{:?}", outra.tipo()),
        }
    }

    fn agora() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// V5.34 do lado Rust (D65 d): a entrada criada pelo app chega à tela com as duas
    /// datas do daemon; a do cofre de teste, sem datas (`null`, e a tela não mostra o
    /// rodapé). O JSON que vai à interface não tem segredo.
    #[test]
    fn d65_datas_no_detalhe_da_tela() {
        let mut d = DaemonFalso::cheio();
        let sem = detalhe(&mut d, "acme-pg-dev");
        assert_eq!((sem.criada_em, sem.alterada_em), (None, None));
        let json = serde_json::to_string(&sem).unwrap();
        assert!(
            json.contains(r#""criada_em":null,"alterada_em":null"#),
            "{json}"
        );

        let antes = agora();
        let novo = Pedido::SalvarEntrada {
            anterior: None,
            manter_anexos: Vec::new(),
            rascunho: Rascunho {
                nome: "clientex-stage-wp".to_owned(),
                grupo: "clientex".to_owned(),
                ambiente: "stage".to_owned(),
                projeto: String::new(),
                tags: Vec::new(),
                dados: RascunhoDados::Login {
                    url: "https://clientex.invalid/login".to_owned(),
                    usuario: "admin".to_owned(),
                    senha: Some(Segredo::de_texto(SEGREDOS[0].to_owned())),
                    totp_semente: None,
                },
                extras: Vec::new(),
                anexos: Vec::new(),
            },
        };
        assert!(matches!(trocar(&mut d, &novo), Resposta::Ok));
        let x = detalhe(&mut d, "clientex-stage-wp");
        let criada = x.criada_em.unwrap();
        assert!(criada >= antes && criada <= agora());
        assert_eq!(x.alterada_em, Some(criada));
        let json = serde_json::to_string(&x).unwrap();
        assert!(json.contains(&format!(r#""criada_em":{criada}"#)), "{json}");
        for s in SEGREDOS {
            assert!(!json.contains(s));
        }
        // Mudar o nível (com o PIN de teste) só mexe na alteração.
        let mudar = Pedido::MudarNivel {
            entrada: "clientex-stage-wp".to_owned(),
            nivel: NivelAcesso::Livre,
            pin: Segredo::de_texto("135790".to_owned()),
        };
        assert!(matches!(trocar(&mut d, &mudar), Resposta::Ok));
        let y = detalhe(&mut d, "clientex-stage-wp");
        assert_eq!(y.criada_em, Some(criada));
        assert!(y.alterada_em.unwrap() >= criada);
    }
}
