//! Output of the list command as text and as JSON. The texts come from the vault and
//! are shown on the terminal and read by the agent: control characters (C0, C1, `DEL`)
//! and Unicode formatting characters (bidi, zero width, tags, BOM) never go out raw, so
//! they do not mess with the terminal nor hide or reverse text (same rule as D51).

use std::fmt::Write as _;

use ward_core::protocolo::ItemLista;

/// Character that cannot go out raw: control or invisible formatting.
pub fn invisible(c: char) -> bool {
    c.is_control()
        || matches!(
            u32::from(c),
            0x00AD
                | 0x034F
                | 0x061C
                | 0x115F..=0x1160
                | 0x17B4..=0x17B5
                | 0x180B..=0x180F
                | 0x200B..=0x200F
                | 0x2028..=0x202E
                | 0x2060..=0x206F
                | 0x3164
                | 0xFE00..=0xFE0F
                | 0xFEFF
                | 0xFFA0
                | 0xFFF0..=0xFFFB
                | 0x1D173..=0x1D17A
                | 0xE0000..=0xE0FFF
        )
}

/// Text for the terminal: invisible characters as a visible escape (`\x1b`,
/// `\u{202e}`), `\` as `\\` (so the escape is not forged by common text).
pub fn neutralize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if invisible(c) && u32::from(c) < 0x80 => {
                let _ = write!(out, "\\x{:02x}", u32::from(c));
            }
            c if invisible(c) => {
                let _ = write!(out, "\\u{{{:04x}}}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out
}

/// Quoted JSON text. Quotes, `\`, controls and invisible characters go as an escape
/// (`\uXXXX`, with a surrogate pair above U+FFFF): the JSON keeps only visible text.
pub fn json_text(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if invisible(c) => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    let _ = write!(out, "\\u{u:04x}");
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn json_optional(out: &mut String, s: Option<&str>) {
    match s {
        Some(s) => json_text(out, s),
        None => out.push_str("null"),
    }
}

/// List as JSON (`--json`): an array with only the name, type, group, environment,
/// project and address (URL or host) of each entry (V4.1). No level (D50), no tags and
/// no field the list does not have.
pub fn list_json(items: &[ItemLista]) -> String {
    let mut out = String::from("[");
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"nome\":");
        json_text(&mut out, &item.nome);
        out.push_str(",\"tipo\":");
        json_text(&mut out, item.tipo.name());
        out.push_str(",\"grupo\":");
        json_text(&mut out, &item.grupo);
        out.push_str(",\"ambiente\":");
        json_text(&mut out, item.ambiente.name());
        out.push_str(",\"projeto\":");
        json_optional(&mut out, item.projeto.as_deref());
        out.push_str(",\"endereco\":");
        json_optional(&mut out, item.endereco.as_deref());
        out.push('}');
    }
    out.push_str("]\n");
    out
}

/// List as text: aligned table, with `-` in the empty field.
pub fn list_table(items: &[ItemLista]) -> String {
    if items.is_empty() {
        return "nenhuma entrada visível\n".to_owned();
    }
    const HEADER: [&str; 6] = ["NOME", "TIPO", "GRUPO", "AMBIENTE", "PROJETO", "URL/HOST"];
    let rows: Vec<[String; 6]> = items
        .iter()
        .map(|i| {
            [
                neutralize(&i.nome),
                i.tipo.name().to_owned(),
                neutralize(&i.grupo),
                i.ambiente.name().to_owned(),
                i.projeto
                    .as_deref()
                    .map_or_else(|| "-".to_owned(), neutralize),
                i.endereco
                    .as_deref()
                    .map_or_else(|| "-".to_owned(), neutralize),
            ]
        })
        .collect();
    let mut widths = HEADER.map(|c| c.chars().count());
    for r in &rows {
        for (w, field) in widths.iter_mut().zip(r) {
            *w = (*w).max(field.chars().count());
        }
    }
    let mut out = String::new();
    let mut emit_row = |fields: [&str; 6]| {
        let mut s = String::new();
        for (k, (field, w)) in fields.iter().zip(widths).enumerate() {
            if k + 1 == fields.len() {
                s.push_str(field);
            } else {
                s.push_str(field);
                let pad = w - field.chars().count() + 2;
                s.extend(std::iter::repeat_n(' ', pad));
            }
        }
        out.push_str(s.trim_end());
        out.push('\n');
    };
    emit_row(HEADER);
    for r in &rows {
        emit_row([&r[0], &r[1], &r[2], &r[3], &r[4], &r[5]]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ward_core::entry::{EntryType, Environment};

    fn item(name: &str, address: Option<&str>) -> ItemLista {
        ItemLista {
            nome: name.to_owned(),
            tipo: EntryType::Database,
            grupo: "clientex".to_owned(),
            ambiente: Environment::Stage,
            projeto: None,
            tags: vec!["etiqueta-x".to_owned()],
            nivel: None,
            endereco: address.map(str::to_owned),
            resumo: None,
        }
    }

    #[test]
    fn json_only_with_the_list_fields() {
        let j = list_json(&[item("clientex-banco", Some("127.0.0.1"))]);
        assert_eq!(
            j,
            "[{\"nome\":\"clientex-banco\",\"tipo\":\"banco\",\"grupo\":\"clientex\",\
             \"ambiente\":\"stage\",\"projeto\":null,\"endereco\":\"127.0.0.1\"}]\n"
        );
        assert!(!j.contains("etiqueta-x"));
        assert!(!j.contains("nivel"));
        assert_eq!(list_json(&[]), "[]\n");
    }

    /// Attack: a name with a terminal escape, bidi or quotes neither goes out raw nor
    /// breaks the JSON.
    #[test]
    fn attack_text_with_control_bidi_and_quotes() {
        let evil = "a\u{1b}]0;x\u{7}\u{202e}b\"c\\d\u{200b}\u{e0041}\u{85}";
        let t = neutralize(evil);
        assert!(!t.chars().any(invisible), "{t:?}");
        assert!(t.contains("\\x1b") && t.contains("\\u{202e}") && t.contains("\\u{0085}"));
        let mut j = String::new();
        json_text(&mut j, evil);
        assert!(!j.chars().any(invisible), "{j:?}");
        assert_eq!(
            j,
            "\"a\\u001b]0;x\\u0007\\u202eb\\\"c\\\\d\\u200b\\udb40\\udc41\\u0085\""
        );
        let table = list_table(&[item(evil, Some(evil))]);
        assert!(!table.chars().any(|c| invisible(c) && c != '\n'));
        assert_eq!(neutralize("a\\x1b"), "a\\\\x1b");
    }

    #[test]
    fn table() {
        let t = list_table(&[item("acme-pg-dev", Some("127.0.0.1")), item("b", None)]);
        let lines: Vec<&str> = t.lines().collect();
        assert_eq!(
            lines[0],
            "NOME         TIPO   GRUPO     AMBIENTE  PROJETO  URL/HOST"
        );
        assert_eq!(
            lines[1],
            "acme-pg-dev  banco  clientex  stage     -        127.0.0.1"
        );
        assert_eq!(
            lines[2],
            "b            banco  clientex  stage     -        -"
        );
        assert_eq!(list_table(&[]), "nenhuma entrada visível\n");
    }
}
