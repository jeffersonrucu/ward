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
use ward_core::protocol::{EntryDetail, ItemSummary, ListItem};

/// A row of the list (compact list item).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CredentialItem {
    pub name: String,
    /// Type identifier (the wire name of the entry type, e.g. `ftp-sftp`).
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub group: String,
    pub environment: &'static str,
    /// Wire name of the access level (blocked, with approval or free).
    pub level: &'static str,
    /// Second line of the item (`postgres · app@127.0.0.1:55432`).
    pub summary: String,
    /// Host or URL, for the search.
    pub address: Option<String>,
    /// Username, for the search.
    pub username: Option<String>,
}

/// Text field of the detail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TextField {
    /// Protocol identifier (`host`, `url`…), the one "copy" uses.
    pub id: String,
    pub label: String,
    pub value: String,
}

/// Secret field of the detail: only the identifier and the label, never the value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SecretField {
    pub id: String,
    pub label: String,
}

/// Extra field: the value only when it is not a secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Extra {
    pub name: String,
    pub secret: bool,
    pub value: Option<String>,
}

/// Attachment: name and size, without the content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Attachment {
    pub name: String,
    pub size: u64,
}

/// The detail panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CredentialDetail {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub group: String,
    pub environment: &'static str,
    pub level: &'static str,
    /// Project and tags, for the edit form (Phase 33).
    pub project: Option<String>,
    pub tags: Vec<String>,
    /// Line next to the badges (group, engine or protocol, and project).
    pub subtitle: String,
    pub fields: Vec<TextField>,
    pub secrets: Vec<SecretField>,
    pub extras: Vec<Extra>,
    pub attachments: Vec<Attachment>,
    /// Daemon dates (D65 d), in UTC seconds; the screen shows them in the local time zone.
    /// `None`: the entry does not have the date (those of `make dev-vault`), and the
    /// footer does not show it.
    pub created_at: Option<u64>,
    pub modified_at: Option<u64>,
}

/// Interface label of a protocol field (the identifier, if it is new).
pub fn field_label(id: &str) -> String {
    match id {
        "url" => "URL",
        "username" => "Usuário",
        "engine" => "Motor",
        "host" => "Host",
        "port" => "Porta",
        "db_name" => "Base",
        "tunnel" => "Túnel SSH",
        "jump" => "Salto",
        "variable" => "Variável",
        "access_key_id" => "Access key ID",
        "region" => "Região",
        "role_arn" => "Role ARN",
        "security" => "Segurança",
        "protocol" => "Protocolo",
        "password" => "Senha",
        "totp_seed" => "Semente TOTP",
        "private_key" => "Chave privada",
        "passphrase" => "Passphrase",
        "token" => "Token",
        "secret_access_key" => "Secret access key",
        "session_token" => "Session token",
        "text" => "Texto",
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
fn summary(kind: EntryType, r: &ItemSummary) -> String {
    let c = |id: &str| field(&r.fields, id);
    let has = |id: &str| r.secrets.iter().any(|s| s == id);
    match kind {
        EntryType::Database => {
            let engine = c("engine").unwrap_or_default();
            if c("tunnel").is_some() {
                join(&[engine, &connection(None, c("host"), c("port")), "túnel SSH"])
            } else {
                join(&[engine, &connection(c("username"), c("host"), c("port"))])
            }
        }
        EntryType::Server | EntryType::FtpSftp => {
            let access = if c("jump").is_some() {
                "salto"
            } else if has("private_key") {
                "chave"
            } else {
                "senha"
            };
            let protocol = c("protocol").unwrap_or_default();
            join(&[
                protocol,
                &connection(c("username"), c("host"), c("port")),
                access,
            ])
        }
        EntryType::Smtp => join(&[
            &connection(c("username"), c("host"), c("port")),
            c("security").unwrap_or_default(),
        ]),
        EntryType::Login => join(&[
            c("username").unwrap_or_default(),
            c("url").unwrap_or_default(),
            if has("totp_seed") { "TOTP" } else { "" },
        ]),
        EntryType::Api => join(&[
            c("variable").unwrap_or_default(),
            c("url").unwrap_or_default(),
        ]),
        EntryType::Aws => join(&[
            c("access_key_id").unwrap_or_default(),
            c("region").unwrap_or_default(),
        ]),
        EntryType::SecureNote => "texto protegido".to_owned(),
        EntryType::Generic => match (r.extras, r.attachments) {
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
    pub fn new(item: &ListItem) -> Option<CredentialItem> {
        let r = item.summary.as_ref()?;
        Some(CredentialItem {
            name: item.name.clone(),
            kind: item.kind.name(),
            group: item.group.clone(),
            environment: item.environment.name(),
            level: item.level?.name(),
            summary: summary(item.kind, r),
            address: item.address.clone(),
            username: field(&r.fields, "username").map(str::to_owned),
        })
    }
}

impl CredentialDetail {
    pub fn new(d: &EntryDetail) -> CredentialDetail {
        let engine = field(&d.fields, "engine");
        let protocol = field(&d.fields, "protocol");
        let group = format!("grupo {}", d.group);
        let subtitle = join(&[
            &group,
            engine.or(protocol).unwrap_or_default(),
            d.project.as_deref().unwrap_or_default(),
        ]);
        CredentialDetail {
            name: d.name.clone(),
            kind: d.kind.name(),
            group: d.group.clone(),
            environment: d.environment.name(),
            level: d.level.name(),
            project: d.project.clone(),
            tags: d.tags.clone(),
            subtitle,
            // The engine and the protocol are already in the subtitle.
            fields: d
                .fields
                .iter()
                .filter(|(k, _)| k != "engine" && k != "protocol")
                .map(|(k, v)| TextField {
                    id: k.clone(),
                    label: field_label(k),
                    value: v.clone(),
                })
                .collect(),
            secrets: d
                .secrets
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
                    name: x.name.clone(),
                    secret: x.value.is_none(),
                    value: x.value.clone(),
                })
                .collect(),
            attachments: d
                .attachments
                .iter()
                .map(|a| Attachment {
                    name: a.name.clone(),
                    size: a.size,
                })
                .collect(),
            created_at: d.dates.created_at(),
            modified_at: d.dates.modified_at(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::Daemon;
    use crate::fake_daemon::{FakeDaemon, SECRETS};
    use ward_core::entry::{AccessLevel, DataDraft, Draft, Secret};
    use ward_core::protocol::{self, Request, Response, Role};

    fn exchange(d: &mut FakeDaemon, request: &Request) -> Response {
        let q = protocol::encode_request(Role::App, request).unwrap();
        let r = d.exchange(&q).unwrap();
        protocol::decode_response(Role::App, &r).unwrap()
    }

    fn detail(d: &mut FakeDaemon, name: &str) -> CredentialDetail {
        let p = Request::Detail {
            entry: name.to_owned(),
        };
        match exchange(d, &p) {
            Response::Detail(x) => CredentialDetail::new(&x),
            other => panic!("{:?}", other.kind()),
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
            json.contains(r#""created_at":null,"modified_at":null"#),
            "{json}"
        );

        let before = now();
        let new = Request::SaveEntry {
            previous: None,
            keep_attachments: Vec::new(),
            draft: Draft {
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
        assert!(matches!(exchange(&mut d, &new), Response::Ok));
        let x = detail(&mut d, "clientex-stage-wp");
        let created = x.created_at.unwrap();
        assert!(created >= before && created <= now());
        assert_eq!(x.modified_at, Some(created));
        let json = serde_json::to_string(&x).unwrap();
        assert!(
            json.contains(&format!(r#""created_at":{created}"#)),
            "{json}"
        );
        for s in SECRETS {
            assert!(!json.contains(s));
        }
        // Changing the level (with the test PIN) only touches the modification.
        let set = Request::SetLevel {
            entry: "clientex-stage-wp".to_owned(),
            level: AccessLevel::Free,
            pin: Secret::from_text("135790".to_owned()),
        };
        assert!(matches!(exchange(&mut d, &set), Response::Ok));
        let y = detail(&mut d, "clientex-stage-wp");
        assert_eq!(y.created_at, Some(created));
        assert!(y.modified_at.unwrap() >= created);
    }

    /// m2:V4.23 (m2:Phase 54): the webview receives the English fields, never the 0.1.x
    /// (pt-BR) ones. The type goes as `type` (the Rust field is `kind`).
    #[test]
    fn attack_old_wire_names_are_gone() {
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
            "\"criada_em\":",
            "\"alterada_em\":",
        ] {
            assert!(!json.contains(old), "{old} em {json}");
        }
        for new in [
            "\"name\":",
            "\"type\":",
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
            assert!(json.contains(new), "{new} em {json}");
        }
    }
}
