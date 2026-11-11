//! What the Credentials screen (Phase 32) receives: the list and the detail of an entry,
//! built from the list and detail responses of the protocol. The 2nd line and the
//! search username come from the summary of each item of the app list (D65 c), in a
//! single exchange, without one detail request per entry.
//!
//! S11/D4: nothing here has a secret. Of the secrets, only the identifier and the field
//! label go (for the secret field component to draw the mask); of the secret extras, only the
//! name; of the attachments, name and size. The types implement `Serialize` for Tauri and
//! for the test bridge; the app `Secret` does not, so there is no way for a secret value
//! to get in here by mistake.

use serde::Serialize;
use ward_core::entry::EntryType;
use ward_core::protocolo::{DetalheEntrada, ItemLista, ResumoItem};

/// A row of the list (compact list item).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CredentialItem {
    #[serde(rename = "nome")]
    pub name: String,
    /// Type identifier (the wire name of the entry type, e.g. `ftp-sftp`).
    #[serde(rename = "tipo")]
    pub kind: &'static str,
    #[serde(rename = "grupo")]
    pub group: String,
    #[serde(rename = "ambiente")]
    pub environment: &'static str,
    /// Wire name of the access level (blocked, with approval or free).
    #[serde(rename = "nivel")]
    pub level: &'static str,
    /// Second line of the item (`postgres · app@127.0.0.1:55432`).
    #[serde(rename = "resumo")]
    pub summary: String,
    /// Host or URL, for the search.
    #[serde(rename = "endereco")]
    pub address: Option<String>,
    /// Username, for the search.
    #[serde(rename = "usuario")]
    pub username: Option<String>,
}

/// Text field of the detail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TextField {
    /// Protocol identifier (`host`, `url`…), the one "copy" uses.
    pub id: String,
    #[serde(rename = "rotulo")]
    pub label: String,
    #[serde(rename = "valor")]
    pub value: String,
}

/// Secret field of the detail: only the identifier and the label, never the value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SecretField {
    pub id: String,
    #[serde(rename = "rotulo")]
    pub label: String,
}

/// Extra field: the value only when it is not a secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Extra {
    #[serde(rename = "nome")]
    pub name: String,
    #[serde(rename = "segredo")]
    pub secret: bool,
    #[serde(rename = "valor")]
    pub value: Option<String>,
}

/// Attachment: name and size, without the content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Attachment {
    #[serde(rename = "nome")]
    pub name: String,
    #[serde(rename = "tamanho")]
    pub size: u64,
}

/// The detail panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CredentialDetail {
    #[serde(rename = "nome")]
    pub name: String,
    #[serde(rename = "tipo")]
    pub kind: &'static str,
    #[serde(rename = "grupo")]
    pub group: String,
    #[serde(rename = "ambiente")]
    pub environment: &'static str,
    #[serde(rename = "nivel")]
    pub level: &'static str,
    /// Project and tags, for the edit form (Phase 33).
    #[serde(rename = "projeto")]
    pub project: Option<String>,
    pub tags: Vec<String>,
    /// Line next to the badges (group, engine or protocol, and project).
    #[serde(rename = "subtitulo")]
    pub subtitle: String,
    #[serde(rename = "campos")]
    pub fields: Vec<TextField>,
    #[serde(rename = "segredos")]
    pub secrets: Vec<SecretField>,
    pub extras: Vec<Extra>,
    #[serde(rename = "anexos")]
    pub attachments: Vec<Attachment>,
    /// Daemon dates (D65 d), in UTC seconds; the screen shows them in the local time zone.
    /// `None`: the entry does not have the date (those of `make dev-vault`), and the
    /// footer does not show it.
    #[serde(rename = "criada_em")]
    pub created_at: Option<u64>,
    #[serde(rename = "alterada_em")]
    pub modified_at: Option<u64>,
}

/// Interface label of a protocol field (the identifier, if it is new).
pub fn field_label(id: &str) -> String {
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
        other => return other.to_owned(),
    }
    .to_owned()
}

fn field<'a>(fields: &'a [(String, String)], id: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(k, _)| k == id)
        .map(|(_, v)| v.as_str())
}

/// `user@host:port`, without the missing parts.
fn connection(username: Option<&str>, host: Option<&str>, port: Option<&str>) -> String {
    let mut s = String::new();
    if let Some(u) = username {
        s.push_str(u);
        s.push('@');
    }
    s.push_str(host.unwrap_or_default());
    if let Some(p) = port {
        s.push(':');
        s.push_str(p);
    }
    s
}

fn join(parts: &[&str]) -> String {
    parts
        .iter()
        .filter(|p| !p.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The second line of the item, by type (as in the Credentials screen frames), from the
/// app list summary.
fn summary(kind: EntryType, r: &ResumoItem) -> String {
    let c = |id: &str| field(&r.campos, id);
    let has = |id: &str| r.segredos.iter().any(|s| s == id);
    match kind {
        EntryType::Database => {
            let engine = c("motor").unwrap_or_default();
            if c("tunel").is_some() {
                join(&[
                    engine,
                    &connection(None, c("host"), c("porta")),
                    "túnel SSH",
                ])
            } else {
                join(&[engine, &connection(c("usuario"), c("host"), c("porta"))])
            }
        }
        EntryType::Server | EntryType::FtpSftp => {
            let access = if c("salto").is_some() {
                "salto"
            } else if has("chave_privada") {
                "chave"
            } else {
                "senha"
            };
            let protocol = c("protocolo").unwrap_or_default();
            join(&[
                protocol,
                &connection(c("usuario"), c("host"), c("porta")),
                access,
            ])
        }
        EntryType::Smtp => join(&[
            &connection(c("usuario"), c("host"), c("porta")),
            c("seguranca").unwrap_or_default(),
        ]),
        EntryType::Login => join(&[
            c("usuario").unwrap_or_default(),
            c("url").unwrap_or_default(),
            if has("totp_semente") { "TOTP" } else { "" },
        ]),
        EntryType::Api => join(&[
            c("variavel").unwrap_or_default(),
            c("url").unwrap_or_default(),
        ]),
        EntryType::Aws => join(&[
            c("access_key_id").unwrap_or_default(),
            c("regiao").unwrap_or_default(),
        ]),
        EntryType::SecureNote => "texto protegido".to_owned(),
        EntryType::Generic => match (r.extras, r.anexos) {
            (0, 0) => "sem campos".to_owned(),
            (e, a) => join(&[
                &plural(e, "campo extra", "campos extras"),
                &plural(a, "anexo", "anexos"),
            ]),
        },
    }
}

fn plural(n: u64, one: &str, many: &str) -> String {
    match n {
        0 => String::new(),
        1 => format!("1 {one}"),
        n => format!("{n} {many}"),
    }
}

impl CredentialItem {
    /// The app list item, with the 2nd line and the username taken from the summary
    /// (D65 c). `None` when the item has no level or summary: it is not an app role list
    /// (D50), and the caller refuses the whole response.
    pub fn new(item: &ItemLista) -> Option<CredentialItem> {
        let r = item.resumo.as_ref()?;
        Some(CredentialItem {
            name: item.nome.clone(),
            kind: item.tipo.name(),
            group: item.grupo.clone(),
            environment: item.ambiente.name(),
            level: item.nivel?.name(),
            summary: summary(item.tipo, r),
            address: item.endereco.clone(),
            username: field(&r.campos, "usuario").map(str::to_owned),
        })
    }
}

impl CredentialDetail {
    pub fn new(d: &DetalheEntrada) -> CredentialDetail {
        let engine = field(&d.campos, "motor");
        let protocol = field(&d.campos, "protocolo");
        let group = format!("grupo {}", d.grupo);
        let subtitle = join(&[
            &group,
            engine.or(protocol).unwrap_or_default(),
            d.projeto.as_deref().unwrap_or_default(),
        ]);
        CredentialDetail {
            name: d.nome.clone(),
            kind: d.tipo.name(),
            group: d.grupo.clone(),
            environment: d.ambiente.name(),
            level: d.nivel.name(),
            project: d.projeto.clone(),
            tags: d.tags.clone(),
            subtitle,
            // The engine and the protocol are already in the subtitle.
            fields: d
                .campos
                .iter()
                .filter(|(k, _)| k != "motor" && k != "protocolo")
                .map(|(k, v)| TextField {
                    id: k.clone(),
                    label: field_label(k),
                    value: v.clone(),
                })
                .collect(),
            secrets: d
                .segredos
                .iter()
                .map(|k| SecretField {
                    id: k.clone(),
                    label: field_label(k),
                })
                .collect(),
            extras: d
                .extras
                .iter()
                .map(|x| Extra {
                    name: x.nome.clone(),
                    secret: x.valor.is_none(),
                    value: x.valor.clone(),
                })
                .collect(),
            attachments: d
                .anexos
                .iter()
                .map(|a| Attachment {
                    name: a.nome.clone(),
                    size: a.tamanho,
                })
                .collect(),
            created_at: d.datas.created_at(),
            modified_at: d.datas.modified_at(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::Daemon;
    use crate::fake_daemon::{FakeDaemon, SECRETS};
    use ward_core::entry::{AccessLevel, DataDraft, Draft, Secret};
    use ward_core::protocolo::{self, Papel, Pedido, Resposta};

    fn exchange(d: &mut FakeDaemon, request: &Pedido) -> Resposta {
        let q = protocolo::codificar_pedido(Papel::App, request).unwrap();
        let r = d.exchange(&q).unwrap();
        protocolo::decodificar_resposta(Papel::App, &r).unwrap()
    }

    fn detail(d: &mut FakeDaemon, name: &str) -> CredentialDetail {
        let p = Pedido::Detalhe {
            entrada: name.to_owned(),
        };
        match exchange(d, &p) {
            Resposta::Detalhe(x) => CredentialDetail::new(&x),
            other => panic!("{:?}", other.tipo()),
        }
    }

    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// V5.34 on the Rust side (D65 d): the entry created by the app reaches the screen
    /// with both daemon dates; the test vault one, without dates (`null`, and the screen
    /// does not show the footer). The JSON that goes to the interface has no secret.
    #[test]
    fn d65_dates_in_the_screen_detail() {
        let mut d = FakeDaemon::full();
        let without = detail(&mut d, "acme-pg-dev");
        assert_eq!((without.created_at, without.modified_at), (None, None));
        let json = serde_json::to_string(&without).unwrap();
        assert!(
            json.contains(r#""criada_em":null,"alterada_em":null"#),
            "{json}"
        );

        let before = now();
        let new = Pedido::SalvarEntrada {
            anterior: None,
            manter_anexos: Vec::new(),
            rascunho: Draft {
                name: "clientex-stage-wp".to_owned(),
                group: "clientex".to_owned(),
                environment: "stage".to_owned(),
                project: String::new(),
                tags: Vec::new(),
                data: DataDraft::Login {
                    url: "https://clientex.invalid/login".to_owned(),
                    username: "admin".to_owned(),
                    password: Some(Secret::from_text(SECRETS[0].to_owned())),
                    totp_seed: None,
                },
                extras: Vec::new(),
                attachments: Vec::new(),
            },
        };
        assert!(matches!(exchange(&mut d, &new), Resposta::Ok));
        let x = detail(&mut d, "clientex-stage-wp");
        let created = x.created_at.unwrap();
        assert!(created >= before && created <= now());
        assert_eq!(x.modified_at, Some(created));
        let json = serde_json::to_string(&x).unwrap();
        assert!(
            json.contains(&format!(r#""criada_em":{created}"#)),
            "{json}"
        );
        for s in SECRETS {
            assert!(!json.contains(s));
        }
        // Changing the level (with the test PIN) only touches the modification.
        let set = Pedido::MudarNivel {
            entrada: "clientex-stage-wp".to_owned(),
            nivel: AccessLevel::Free,
            pin: Secret::from_text("135790".to_owned()),
        };
        assert!(matches!(exchange(&mut d, &set), Resposta::Ok));
        let y = detail(&mut d, "clientex-stage-wp");
        assert_eq!(y.created_at, Some(created));
        assert!(y.modified_at.unwrap() >= created);
    }

    /// m2:DT3: the new Rust names are not wire names. The webview keeps receiving the
    /// old fields until Phase 54.
    #[test]
    fn attack_new_rust_names_are_not_wire_names() {
        let mut d = FakeDaemon::full();
        let json = serde_json::to_string(&detail(&mut d, "acme-pg-dev")).unwrap();
        for old in [
            "\"nome\":",
            "\"tipo\":",
            "\"grupo\":",
            "\"ambiente\":",
            "\"nivel\":",
            "\"projeto\":",
            "\"subtitulo\":",
            "\"campos\":",
            "\"segredos\":",
            "\"anexos\":",
            "\"rotulo\":",
            "\"valor\":",
        ] {
            assert!(json.contains(old), "{old} em {json}");
        }
        for new in [
            "\"name\":",
            "\"kind\":",
            "\"group\":",
            "\"environment\":",
            "\"level\":",
            "\"project\":",
            "\"subtitle\":",
            "\"fields\":",
            "\"secrets\":",
            "\"attachments\":",
            "\"label\":",
            "\"value\":",
            "\"created_at\":",
            "\"modified_at\":",
        ] {
            assert!(!json.contains(new), "{new} em {json}");
        }
    }
}
