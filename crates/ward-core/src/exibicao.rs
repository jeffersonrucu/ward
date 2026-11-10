//! Texto do agente para exibir na janela de aprovação (D51, Fase 23).
//!
//! O comando e a cadeia de processos vêm do agente: podem ter caractere de controle
//! (`ESC` que apaga a linha num terminal), de direção de texto (U+202E inverte o que
//! vem depois), de largura zero ou tags Unicode, que fazem a janela mostrar outra
//! coisa que não o comando executado. Aqui nada é escondido nem tirado: o que não é
//! visível vira um **escape visível** e o resto passa igual.
//!
//! - [`exibir_argumentos`]: cada argumento entre aspas duplas, separados por espaço;
//!   `"` vira `\"` e `\` vira `\\`, então a forma é sem ambiguidade (dá para voltar aos
//!   bytes exatos). Byte que não é UTF-8 válido vira `\xNN`.
//! - [`exibir_texto`]: o mesmo sem as aspas (nome de processo, um texto só).
//! - [`neutralizar`]: só troca o invisível por escape (não mexe em `\` nem em aspas).
//!   Para quem recebe um texto já montado e o trata como não confiável (a janela, Fase
//!   36): sobre a saída das duas acima não muda nada.
//!
//! Escapes: `\n`, `\r`, `\t`; outro controle C0, `DEL` e C1 como `\xNN`; formatação e
//! separadores Unicode como `\u{XXXX}`. Texto acima do limite é cortado num ponto que
//! não parte escape nem caractere, com o aviso [`AVISO_CORTE`] e o número de bytes que
//! ficaram de fora; nunca em silêncio.

use core::fmt::Write as _;

/// Começo do aviso de corte (o número de bytes vem depois).
pub const AVISO_CORTE: &str = " … [cortado: mais ";

/// Espaço reservado no limite para o aviso de corte.
const RESERVA_AVISO: usize = 64;

/// O caractere não é visível ou muda a ordem/forma do texto ao redor: controle (C0, C1,
/// `DEL`), formatação Unicode (bidi, largura zero, BOM, tags, juntores), separadores
/// de linha/parágrafo, seletores de variação e preenchedores do Hangul.
pub fn invisivel(c: char) -> bool {
    let u = c as u32;
    c.is_control()
        || matches!(
            u,
            0x00AD                  // hífen suave
            | 0x034F                // juntor de grafema
            | 0x0600..=0x0605       // marcas árabes de número
            | 0x061C                // marca de letra árabe (bidi)
            | 0x06DD
            | 0x070F
            | 0x08E2
            | 0x115F | 0x1160      // preenchedores do Hangul
            | 0x17B4 | 0x17B5
            | 0x180B..=0x180F       // seletores mongóis e U+180E
            | 0x200B..=0x200F       // largura zero, ZWJ/ZWNJ, LRM/RLM
            | 0x2028 | 0x2029      // separadores de linha e parágrafo
            | 0x202A..=0x202E       // bidi: embed/override
            | 0x2060..=0x206F       // word joiner, invisíveis, isolates bidi
            | 0x3164                // preenchedor do Hangul
            | 0xFE00..=0xFE0F       // seletores de variação
            | 0xFEFD | 0xFEFE      // não atribuídos; a ICU os dá como AL (Fase 39g)
            | 0xFEFF                // BOM / largura zero sem quebra
            | 0xFFA0
            | 0xFFF9..=0xFFFB       // anotação interlinear
            | 0x110BD | 0x110CD
            | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0000..=0xE007F     // tags
            | 0xE0100..=0xE01EF // seletores de variação suplementares
        )
}

/// Escreve `c` em `saida`, com escape se for invisível.
fn escrever_char(saida: &mut String, c: char) {
    match c {
        '\n' => saida.push_str("\\n"),
        '\r' => saida.push_str("\\r"),
        '\t' => saida.push_str("\\t"),
        c if (c as u32) < 0x100 && invisivel(c) => {
            let _ = write!(saida, "\\x{:02x}", c as u32);
        }
        c if invisivel(c) => {
            let _ = write!(saida, "\\u{{{:04X}}}", c as u32);
        }
        c => saida.push(c),
    }
}

/// Os pedaços (já com escape) de `bytes`: um por caractere ou byte inválido.
fn pedacos(bytes: &[u8], aspas: bool, mut cada: impl FnMut(&str) -> bool) {
    let mut buf = String::new();
    for pedaco in bytes.utf8_chunks() {
        for c in pedaco.valid().chars() {
            buf.clear();
            match c {
                '\\' => buf.push_str("\\\\"),
                '"' if aspas => buf.push_str("\\\""),
                c => escrever_char(&mut buf, c),
            }
            if !cada(&buf) {
                return;
            }
        }
        for b in pedaco.invalid() {
            buf.clear();
            let _ = write!(buf, "\\x{b:02x}");
            if !cada(&buf) {
                return;
            }
        }
    }
}

/// Monta o texto até `limite` bytes; se não couber, corta e põe o aviso.
struct Montagem {
    saida: String,
    limite: usize,
    total: usize,
    cortado: bool,
}

impl Montagem {
    fn nova(limite: usize) -> Montagem {
        Montagem {
            saida: String::new(),
            // Limite pequeno: metade fica para o aviso (o aviso inteiro só cabe a partir
            // de ~64 bytes).
            limite: limite.saturating_sub(RESERVA_AVISO.min(limite / 2)),
            total: 0,
            cortado: false,
        }
    }

    /// Acrescenta `p`; `false` quando passou do limite (o resto só conta bytes).
    fn por(&mut self, p: &str) -> bool {
        self.total += p.len();
        if !self.cortado && self.saida.len() + p.len() <= self.limite {
            self.saida.push_str(p);
        } else {
            self.cortado = true;
        }
        true
    }

    fn fim(mut self) -> String {
        if self.cortado {
            let fora = self.total - self.saida.len();
            let _ = write!(self.saida, "{AVISO_CORTE}{fora} bytes não mostrados]");
        }
        self.saida
    }
}

/// O comando (`argv`) para exibir: cada argumento entre aspas, separados por espaço,
/// com escape visível, em até `limite` bytes (com o aviso de corte se passar).
pub fn exibir_argumentos<A: AsRef<[u8]>>(argv: &[A], limite: usize) -> String {
    let mut m = Montagem::nova(limite);
    for (i, a) in argv.iter().enumerate() {
        if i > 0 {
            m.por(" ");
        }
        m.por("\"");
        pedacos(a.as_ref(), true, |p| m.por(p));
        m.por("\"");
    }
    m.fim()
}

/// Um texto para exibir (sem aspas), com escape visível e `\` como `\\`, em até
/// `limite` bytes (com o aviso de corte se passar).
pub fn exibir_texto(texto: &[u8], limite: usize) -> String {
    let mut m = Montagem::nova(limite);
    pedacos(texto, false, |p| m.por(p));
    m.fim()
}

/// Só troca o caractere invisível por escape visível; o resto fica igual. Sobre a
/// saída de [`exibir_argumentos`] e [`exibir_texto`] não muda nada.
pub fn neutralizar(texto: &str) -> String {
    let mut s = String::with_capacity(texto.len());
    for c in texto.chars() {
        escrever_char(&mut s, c);
    }
    s
}

#[cfg(test)]
mod testes {
    use super::*;

    const L: usize = 64 * 1024;

    /// Nenhum caractere invisível na saída.
    fn so_visivel(s: &str) -> bool {
        s.chars().all(|c| !invisivel(c))
    }

    #[test]
    fn argumentos_entre_aspas_e_separados() {
        assert_eq!(
            exibir_argumentos(&[b"psql".as_slice(), b"-c", b"select 1"], L),
            r#""psql" "-c" "select 1""#
        );
        assert_eq!(exibir_argumentos::<&[u8]>(&[], L), "");
        assert_eq!(exibir_argumentos(&[b""], L), r#""""#);
        // Aspas e barra no argumento não fecham o argumento nem somem.
        assert_eq!(
            exibir_argumentos(&[br#"a" "b\c"#.as_slice()], L),
            r#""a\" \"b\\c""#
        );
    }

    /// Ataque (D51): o agente põe ESC, CR, bidi, largura zero e tags no comando para a
    /// janela mostrar outro texto. Tudo aparece como escape visível, nada some.
    #[test]
    fn ataque_controle_e_bidi_viram_escape_visivel() {
        let casos: &[(&str, &str)] = &[
            ("rm -rf /\x1b[2K\rls", r#""rm -rf /\x1b[2K\rls""#),
            ("a\nb", r#""a\nb""#),
            ("a\tb", r#""a\tb""#),
            ("\u{7f}", r#""\x7f""#),
            ("\u{85}", r#""\x85""#),
            ("\u{9b}31m", r#""\x9b31m""#),
            ("ls \u{202E}txt.hs", r#""ls \u{202E}txt.hs""#),
            ("\u{2066}x\u{2069}", r#""\u{2066}x\u{2069}""#),
            ("a\u{200B}b\u{200D}c", r#""a\u{200B}b\u{200D}c""#),
            ("\u{FEFF}bom", r#""\u{FEFF}bom""#),
            // Fase 39g: não atribuídos que a ICU trata como AL (letra árabe).
            ("ls \u{FEFD}1\u{FEFE}", r#""ls \u{FEFD}1\u{FEFE}""#),
            ("x\u{E0041}\u{E007F}", r#""x\u{E0041}\u{E007F}""#),
            ("l\u{2028}p\u{2029}", r#""l\u{2028}p\u{2029}""#),
            (
                "\u{00AD}\u{061C}\u{200E}\u{200F}",
                r#""\xad\u{061C}\u{200E}\u{200F}""#,
            ),
        ];
        for (entrada, esperado) in casos {
            let s = exibir_argumentos(&[entrada.as_bytes()], L);
            assert_eq!(&s, esperado, "{entrada:?}");
            assert!(so_visivel(&s), "{s:?}");
        }
        // Acento e emoji (visíveis) passam iguais.
        assert_eq!(
            exibir_argumentos(&["ação ✓ 日本".as_bytes()], L),
            "\"ação ✓ 日本\""
        );
    }

    #[test]
    fn byte_invalido_vira_x() {
        assert_eq!(exibir_argumentos(&[b"a\xffb\xc3"], L), r#""a\xffb\xc3""#);
        assert_eq!(exibir_texto(b"\x00", L), r"\x00");
    }

    /// A forma é sem ambiguidade: argumentos diferentes dão textos diferentes, mesmo
    /// quando um tenta imitar o escape do outro.
    #[test]
    fn ataque_imitar_escape_ou_separador_nao_engana() {
        type Argv<'a> = &'a [&'a [u8]];
        let pares: &[(Argv, Argv)] = &[
            (&[b"\x1b"], &[b"\\x1b"]),
            (&[b"a b"], &[b"a", b"b"]),
            (&[br#"a" "b"#], &[b"a", b"b"]),
            (&["\u{202E}".as_bytes()], &[b"\\u{202E}"]),
            (&[b"\n"], &[b"\\n"]),
            (&[b"\xff"], &[b"\\xff"]),
        ];
        for (a, b) in pares {
            assert_ne!(exibir_argumentos(a, L), exibir_argumentos(b, L));
        }
    }

    /// Texto longo é cortado com aviso, no limite, sem partir caractere nem escape.
    #[test]
    fn corte_com_aviso_nunca_em_silencio() {
        let longo = "ç".repeat(10_000);
        let s = exibir_argumentos(&[longo.as_bytes()], 1000);
        assert!(s.len() <= 1000, "{}", s.len());
        assert!(s.contains(AVISO_CORTE), "{s}");
        assert!(s.ends_with(" bytes não mostrados]"));
        // O que foi mostrado é começo exato do texto inteiro.
        let inteiro = exibir_argumentos(&[longo.as_bytes()], usize::MAX);
        let mostrado = &s[..s.find(AVISO_CORTE).unwrap()];
        assert!(inteiro.starts_with(mostrado));
        let fora: usize = s[s.find(AVISO_CORTE).unwrap() + AVISO_CORTE.len()..]
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(mostrado.len() + fora, inteiro.len());
        // Escape não é partido.
        let esc = "\u{1b}".repeat(1000);
        let s = exibir_texto(esc.as_bytes(), 200);
        let mostrado = &s[..s.find(AVISO_CORTE).unwrap()];
        assert_eq!(mostrado.len() % 4, 0, "{mostrado}");
        assert!(s.len() <= 200);
        // Sem corte, sem aviso.
        assert!(!exibir_texto(b"curto", 200).contains(AVISO_CORTE));
    }

    #[test]
    fn neutralizar_e_idempotente_sobre_a_saida() {
        for t in [
            "rm\u{1b}[2K\u{202E}x\u{200B}\n\\\"".as_bytes(),
            b"\xff\x00plain",
        ] {
            let a = exibir_argumentos(&[t], L);
            assert_eq!(neutralizar(&a), a);
            let b = exibir_texto(t, L);
            assert_eq!(neutralizar(&b), b);
        }
        assert_eq!(neutralizar("a\u{202E}b\u{1b}"), r"a\u{202E}b\x1b");
        assert_eq!(neutralizar(r#"a\b"c"#), r#"a\b"c"#);
    }

    #[test]
    fn texto_sem_aspas() {
        assert_eq!(exibir_texto(br#"claude "x" \"#, L), r#"claude "x" \\"#);
        assert_eq!(exibir_texto("ba\u{202E}sh".as_bytes(), L), r"ba\u{202E}sh");
    }
}
