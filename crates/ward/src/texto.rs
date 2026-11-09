//! Saída da `lista` em texto e em JSON. Os textos vêm do cofre e são exibidos no
//! terminal e lidos pelo agente: caractere de controle (C0, C1, `DEL`) e de formatação
//! Unicode (bidi, largura zero, tags, BOM) nunca saem crus, para não mexer no terminal
//! nem esconder ou inverter texto (mesma regra de D51).

use std::fmt::Write as _;

use ward_core::protocolo::ItemLista;

/// Caractere que não pode sair cru: controle ou formatação invisível.
pub fn invisivel(c: char) -> bool {
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

/// Texto para o terminal: invisíveis como escape visível (`\x1b`, `\u{202e}`), `\` como
/// `\\` (assim o escape não é forjado por texto comum).
pub fn neutralizar(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if invisivel(c) && u32::from(c) < 0x80 => {
                let _ = write!(out, "\\x{:02x}", u32::from(c));
            }
            c if invisivel(c) => {
                let _ = write!(out, "\\u{{{:04x}}}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out
}

/// Texto JSON entre aspas. Aspas, `\`, controles e invisíveis vão como escape
/// (`\uXXXX`, com par substituto acima de U+FFFF): o JSON fica só com texto visível.
pub fn json_texto(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if invisivel(c) => {
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

fn json_opcional(out: &mut String, s: Option<&str>) {
    match s {
        Some(s) => json_texto(out, s),
        None => out.push_str("null"),
    }
}

/// `ward lista --json`: um array com só nome, tipo, grupo, ambiente, projeto e
/// endereço (URL ou host) de cada entrada (V4.1). Sem nível (D50), sem tags e sem
/// nenhum campo que a lista não tenha.
pub fn lista_json(itens: &[ItemLista]) -> String {
    let mut out = String::from("[");
    for (i, item) in itens.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"nome\":");
        json_texto(&mut out, &item.nome);
        out.push_str(",\"tipo\":");
        json_texto(&mut out, item.tipo.nome());
        out.push_str(",\"grupo\":");
        json_texto(&mut out, &item.grupo);
        out.push_str(",\"ambiente\":");
        json_texto(&mut out, item.ambiente.nome());
        out.push_str(",\"projeto\":");
        json_opcional(&mut out, item.projeto.as_deref());
        out.push_str(",\"endereco\":");
        json_opcional(&mut out, item.endereco.as_deref());
        out.push('}');
    }
    out.push_str("]\n");
    out
}

/// `ward lista`: tabela alinhada, com `-` no campo vazio.
pub fn lista_tabela(itens: &[ItemLista]) -> String {
    if itens.is_empty() {
        return "nenhuma entrada visível\n".to_owned();
    }
    const CABECALHO: [&str; 6] = ["NOME", "TIPO", "GRUPO", "AMBIENTE", "PROJETO", "URL/HOST"];
    let linhas: Vec<[String; 6]> = itens
        .iter()
        .map(|i| {
            [
                neutralizar(&i.nome),
                i.tipo.nome().to_owned(),
                neutralizar(&i.grupo),
                i.ambiente.nome().to_owned(),
                i.projeto
                    .as_deref()
                    .map_or_else(|| "-".to_owned(), neutralizar),
                i.endereco
                    .as_deref()
                    .map_or_else(|| "-".to_owned(), neutralizar),
            ]
        })
        .collect();
    let mut largura = CABECALHO.map(|c| c.chars().count());
    for l in &linhas {
        for (w, campo) in largura.iter_mut().zip(l) {
            *w = (*w).max(campo.chars().count());
        }
    }
    let mut out = String::new();
    let mut linha = |campos: [&str; 6]| {
        let mut s = String::new();
        for (k, (campo, w)) in campos.iter().zip(largura).enumerate() {
            if k + 1 == campos.len() {
                s.push_str(campo);
            } else {
                s.push_str(campo);
                let falta = w - campo.chars().count() + 2;
                s.extend(std::iter::repeat_n(' ', falta));
            }
        }
        out.push_str(s.trim_end());
        out.push('\n');
    };
    linha(CABECALHO);
    for l in &linhas {
        linha([&l[0], &l[1], &l[2], &l[3], &l[4], &l[5]]);
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;
    use ward_core::entrada::{Ambiente, TipoEntrada};

    fn item(nome: &str, endereco: Option<&str>) -> ItemLista {
        ItemLista {
            nome: nome.to_owned(),
            tipo: TipoEntrada::Banco,
            grupo: "clientex".to_owned(),
            ambiente: Ambiente::Stage,
            projeto: None,
            tags: vec!["etiqueta-x".to_owned()],
            nivel: None,
            endereco: endereco.map(str::to_owned),
            resumo: None,
        }
    }

    #[test]
    fn json_so_com_os_campos_da_lista() {
        let j = lista_json(&[item("clientex-banco", Some("127.0.0.1"))]);
        assert_eq!(
            j,
            "[{\"nome\":\"clientex-banco\",\"tipo\":\"banco\",\"grupo\":\"clientex\",\
             \"ambiente\":\"stage\",\"projeto\":null,\"endereco\":\"127.0.0.1\"}]\n"
        );
        assert!(!j.contains("etiqueta-x"));
        assert!(!j.contains("nivel"));
        assert_eq!(lista_json(&[]), "[]\n");
    }

    /// Ataque: um nome com escape de terminal, bidi ou aspas não sai cru nem quebra o
    /// JSON.
    #[test]
    fn ataque_texto_com_controle_bidi_e_aspas() {
        let mal = "a\u{1b}]0;x\u{7}\u{202e}b\"c\\d\u{200b}\u{e0041}\u{85}";
        let t = neutralizar(mal);
        assert!(!t.chars().any(invisivel), "{t:?}");
        assert!(t.contains("\\x1b") && t.contains("\\u{202e}") && t.contains("\\u{0085}"));
        let mut j = String::new();
        json_texto(&mut j, mal);
        assert!(!j.chars().any(invisivel), "{j:?}");
        assert_eq!(
            j,
            "\"a\\u001b]0;x\\u0007\\u202eb\\\"c\\\\d\\u200b\\udb40\\udc41\\u0085\""
        );
        let tabela = lista_tabela(&[item(mal, Some(mal))]);
        assert!(!tabela.chars().any(|c| invisivel(c) && c != '\n'));
        assert_eq!(neutralizar("a\\x1b"), "a\\\\x1b");
    }

    #[test]
    fn tabela() {
        let t = lista_tabela(&[item("acme-pg-dev", Some("127.0.0.1")), item("b", None)]);
        let linhas: Vec<&str> = t.lines().collect();
        assert_eq!(
            linhas[0],
            "NOME         TIPO   GRUPO     AMBIENTE  PROJETO  URL/HOST"
        );
        assert_eq!(
            linhas[1],
            "acme-pg-dev  banco  clientex  stage     -        127.0.0.1"
        );
        assert_eq!(
            linhas[2],
            "b            banco  clientex  stage     -        -"
        );
        assert_eq!(lista_tabela(&[]), "nenhuma entrada visível\n");
    }
}
