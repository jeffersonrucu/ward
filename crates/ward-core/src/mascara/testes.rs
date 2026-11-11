//! Testes da máscara (D11, S4): V2.17–V2.24 e os ataques (pedaços quebrados,
//! codificações que a lista de D11 não cita, saída patológica, segredo em `Debug`).
//! Só valores falsos do cofre de teste (D15).

use super::*;
use crate::entrada::{Rascunho, RascunhoAnexo, RascunhoDados, RascunhoExtra, Segredo};
use crate::entrega::{ErroRecurso, Recursos, montar};

// ---- Ajudantes ----

const SENHA_PG: &str = "falso-PG:s3nha/01@#";
const URL_PG: &str = "postgresql://app:falso-PG%3As3nha%2F01%40%23@127.0.0.1:55432/app_teste";
const M: &str = MARCA;

fn t(v: &str) -> String {
    v.to_owned()
}

fn s(v: &str) -> Option<Segredo> {
    Some(Segredo::de_texto(t(v)))
}

fn rascunho(nome: &str, dados: RascunhoDados) -> Rascunho {
    Rascunho {
        nome: t(nome),
        grupo: t("acme"),
        ambiente: t("dev"),
        projeto: String::new(),
        tags: Vec::new(),
        dados,
        extras: Vec::new(),
        anexos: Vec::new(),
    }
}

fn extra(nome: &str, valor: &str, segredo: bool) -> RascunhoExtra {
    RascunhoExtra {
        nome: t(nome),
        valor: Segredo::de_texto(t(valor)),
        segredo,
    }
}

fn anexo(nome: &str, conteudo: &[u8]) -> RascunhoAnexo {
    RascunhoAnexo {
        nome: t(nome),
        conteudo: Segredo::de_bytes(conteudo.to_vec()),
    }
}

fn valida(r: Rascunho) -> Entrada {
    r.validar().unwrap_or_else(|e| panic!("devia validar: {e}"))
}

fn pg_dev() -> Entrada {
    valida(rascunho(
        "acme-pg-dev",
        RascunhoDados::Banco {
            motor: t("postgres"),
            host: t("127.0.0.1"),
            porta: t("55432"),
            base: t("app_teste"),
            usuario: t("app"),
            senha: s(SENHA_PG),
            tunel: String::new(),
        },
    ))
}

fn gama_generico() -> Entrada {
    let mut r = rascunho("gama-generico", RascunhoDados::Generico);
    r.grupo = t("gama");
    r.ambiente = t("stage");
    r.extras = vec![
        extra("REGIAO", "r-01", false),
        extra("CHAVE_LICENCA", "falso-licenca-01", true),
    ];
    r.anexos = vec![anexo("app.env", b"FALSO_ENV=falso-anexo-01\n")];
    valida(r)
}

/// Genérico só com extras secretos (para os testes de vários segredos).
fn generico(segredos: &[&str]) -> Entrada {
    let mut r = rascunho("teste-generico", RascunhoDados::Generico);
    r.extras = segredos
        .iter()
        .enumerate()
        .map(|(i, v)| extra(&format!("S{i}"), v, true))
        .collect();
    valida(r)
}

const CODIGO_FALSO: &str = "123456";

struct Falsos;

impl Recursos for Falsos {
    fn codigo_totp(&mut self, _: &Segredo) -> Result<Zeroizing<String>, ErroRecurso> {
        Ok(Zeroizing::new(t(CODIGO_FALSO)))
    }
    fn arquivo(&mut self, nome: &str, _: &Segredo) -> Result<String, ErroRecurso> {
        Ok(format!("/run/user/1000/ward/tmp/1-{nome}"))
    }
    fn tunel(&mut self, _: &str, _: &str, _: u16) -> Result<u16, ErroRecurso> {
        Ok(40123)
    }
}

fn entrega(e: &Entrada) -> Entrega {
    montar(e, &mut Falsos).unwrap_or_else(|erro| panic!("{erro}"))
}

/// Máscara da entrada com a entrega dela (como o `ward run` monta).
fn mascara(e: &Entrada) -> Mascara {
    let g = entrega(e);
    Mascara::da_entrada(e, Some(&g)).expect("máscara")
}

/// Passa a saída em pedaços (cortes em posições crescentes) e junta o resultado.
/// Confere, a cada pedaço, que nada ficou segurado (D34 revisto): o fluxo devolveu
/// tudo o que leu.
fn em_pedacos(m: &Mascara, saida: &[u8], cortes: &[usize]) -> Vec<u8> {
    let mut f = m.fluxo();
    let mut out = Vec::new();
    let mut ini = 0;
    for &c in cortes.iter().chain(core::iter::once(&saida.len())) {
        f.mascarar(&saida[ini..c], &mut out);
        nada_segurado(&f);
        ini = c;
    }
    f.terminar(&mut out);
    out
}

/// O fluxo não guarda nada para depois (D34 revisto: nunca segura nem atrasa).
fn nada_segurado(f: &Fluxo<'_>) {
    assert_eq!(f.emitido, f.lido, "o fluxo segurou saída");
    assert!(f.pendente.is_empty(), "o fluxo segurou saída");
}

/// A saída num pedaço só (o caso que S4 garante: o segredo inteiro no mesmo bloco).
/// Também passa a mesma saída byte a byte e confere que nenhum byte ficou segurado e
/// que cada byte saiu na hora, igual, até o último byte de um padrão (o casamento pela
/// metade não muda nada: sem oráculo de prefixo).
fn um_bloco(m: &Mascara, saida: &[u8]) -> Vec<u8> {
    let mut f = m.fluxo();
    let mut out = Vec::new();
    for b in saida {
        let antes = out.len();
        let marca_no_fim = out.ends_with(M.as_bytes());
        f.mascarar(core::slice::from_ref(b), &mut out);
        nada_segurado(&f);
        let novo = &out[antes..];
        // Cada byte sai na hora: ele mesmo, a marca (um padrão terminou aqui) ou nada
        // (o padrão continua colado na marca que acabou de sair).
        assert!(
            novo == core::slice::from_ref(b)
                || novo == M.as_bytes()
                || (novo.is_empty() && marca_no_fim),
            "byte segurado ou trocado"
        );
    }
    m.mascarar_tudo(saida)
}

fn texto(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// Base64 de teste, independente do da máscara (acumulador de bits).
fn b64(dados: &[u8], url: bool, igual: bool) -> String {
    let alf: Vec<char> = if url {
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
    } else {
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
    }
    .chars()
    .collect();
    let mut out = String::new();
    let (mut acc, mut bits) = (0u32, 0u32);
    for &b in dados {
        acc = (acc << 8) | u32::from(b);
        bits += 8;
        while bits >= 6 {
            bits -= 6;
            out.push(alf[((acc >> bits) & 63) as usize]);
        }
    }
    if bits > 0 {
        out.push(alf[((acc << (6 - bits)) & 63) as usize]);
    }
    while igual && !out.len().is_multiple_of(4) {
        out.push('=');
    }
    out
}

/// Percent-encoding de teste: `todos` codifica tudo fora de `A-Za-z0-9-._~`;
/// `minusculo` usa hex minúsculo; `mais` troca espaço por `+`.
fn pct(v: &str, minusculo: bool, mais: bool) -> String {
    let mut out = String::new();
    for &b in v.as_bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else if b == b' ' && mais {
            out.push('+');
        } else if minusculo {
            out.push_str(&format!("%{b:02x}"));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Confere que nenhum pedaço do segredo com 4 bytes ou mais sobrou na saída.
fn sem_pedaco(saida: &[u8], segredo: &[u8]) {
    for n in 4..=segredo.len() {
        for janela in segredo.windows(n) {
            assert!(
                !saida.windows(n).any(|w| w == janela),
                "sobrou pedaço do segredo na saída: {}",
                texto(saida)
            );
        }
    }
}

/// Gerador pseudoaleatório de teste (xorshift64*). Não é cripto: só sorteia cortes.
struct Sorteio(u64);

impl Sorteio {
    fn proximo(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn ate(&mut self, n: usize) -> usize {
        (self.proximo() % n as u64) as usize
    }
}

// ---- V2.17 ----

#[test]
fn v2_17_literal() {
    let m = mascara(&pg_dev());
    let saida = m.mascarar_tudo(format!("x {SENHA_PG} y").as_bytes());
    assert_eq!(texto(&saida), format!("x {M} y"));
}

// ---- V2.18 ----

#[test]
fn v2_18_cada_ponto_de_corte() {
    // D34 revisto: nada é segurado. O segredo inteiro num pedaço sai `••••`; quebrado
    // entre pedaços, o que já saiu fica como saiu e o pedaço que completa o segredo sai
    // marcado a partir do corte (o estado do casamento segue de um pedaço ao outro).
    let m = mascara(&pg_dev());
    let saida = format!("x {SENHA_PG} y");
    let b = saida.as_bytes();
    let esperado = format!("x {M} y");
    let ini = 2;
    let fim = ini + SENHA_PG.len();
    // O que sai com os cortes: até o último corte antes do fim do segredo em claro (se
    // ele cai dentro do segredo), daí a marca.
    let com_cortes = |cortes: &[usize]| {
        let c = cortes.iter().copied().filter(|&c| c > ini && c < fim).max();
        match c {
            None => esperado.clone(),
            Some(c) => format!("x {}{M} y", &SENHA_PG[..c - ini]),
        }
    };
    for k in 0..=b.len() {
        assert_eq!(
            texto(&em_pedacos(&m, b, &[k])),
            com_cortes(&[k]),
            "corte {k}"
        );
        for j in k..=b.len() {
            assert_eq!(
                texto(&em_pedacos(&m, b, &[k, j])),
                com_cortes(&[k, j]),
                "cortes {k},{j}"
            );
        }
    }
    assert_eq!(texto(&um_bloco(&m, b)), esperado);
}

#[test]
fn d34_ataque_prefixo_no_fim_do_pedaco_sai_na_hora_e_igual() {
    // Auditoria do M4 (oráculo de prefixo): o começo do segredo no fim de um pedaço
    // sai na hora, igual a qualquer outro texto; nada fica segurado nem vira marca.
    // Prefixo certo e prefixo errado do mesmo tamanho dão a mesma resposta (o texto
    // que entrou), então a saída não diz nada sobre o segredo.
    let m = mascara(&pg_dev());
    let saida = format!("x {SENHA_PG} y");
    let b = saida.as_bytes();
    for k in 3..2 + SENHA_PG.len() {
        let mut f = m.fluxo();
        let mut out = Vec::new();
        f.mascarar(&b[..k], &mut out);
        nada_segurado(&f);
        assert_eq!(
            &out[..],
            &b[..k],
            "corte {k}: o prefixo foi segurado ou trocado"
        );
        let mut errado = b[..k].to_vec();
        *errado.last_mut().unwrap() ^= 0x01;
        let mut g = m.fluxo();
        let mut out_errado = Vec::new();
        g.mascarar(&errado, &mut out_errado);
        nada_segurado(&g);
        assert_eq!(out_errado, errado);
    }
}

// ---- V2.19 ----

#[test]
fn v2_19_percent_encoding() {
    let m = mascara(&pg_dev());
    let formas = [
        pct(SENHA_PG, false, false),    // maiúsculo: falso-PG%3As3nha%2F01%40%23
        pct(SENHA_PG, true, false),     // minúsculo
        pct(SENHA_PG, false, true),     // formulário
        t("falso-PG%3As3nha/01%40%23"), // urllib.parse.quote (deixa a barra)
        t("falso-PG%3as3nha%2F01@%23"), // misturado
        t("%66%61%6C%73%6F-PG:s3nha/01@#"), // até as letras codificadas
    ];
    assert_eq!(formas[0], "falso-PG%3As3nha%2F01%40%23");
    for forma in &formas {
        let saida = format!("x {forma} y");
        let esperado = format!("x {M} y");
        assert_eq!(
            texto(&m.mascarar_tudo(saida.as_bytes())),
            esperado,
            "{forma}"
        );
        assert_eq!(texto(&um_bloco(&m, saida.as_bytes())), esperado, "{forma}");
    }
}

#[test]
fn v2_19_formulario_com_espaco() {
    let m = mascara(&generico(&["falso senha 01/x"]));
    for forma in [
        pct("falso senha 01/x", false, true),
        pct("falso senha 01/x", true, true),
        pct("falso senha 01/x", false, false),
        t("falso+senha+01%2fx"),
    ] {
        let saida = format!("a={forma}&b=1");
        assert_eq!(texto(&um_bloco(&m, saida.as_bytes())), format!("a={M}&b=1"));
    }
}

// ---- V2.20 ----

#[test]
fn v2_20_base64() {
    let m = mascara(&pg_dev());
    let basic = b64(format!("app:{SENHA_PG}").as_bytes(), false, true);
    assert_eq!(basic, "YXBwOmZhbHNvLVBHOnMzbmhhLzAxQCM=");
    let formas = [
        b64(SENHA_PG.as_bytes(), false, true),
        b64(SENHA_PG.as_bytes(), false, false),
        b64(SENHA_PG.as_bytes(), true, true),
        b64(SENHA_PG.as_bytes(), true, false),
        basic.clone(),
        basic.trim_end_matches('=').to_owned(),
    ];
    for forma in &formas {
        let saida = format!("Authorization: Basic {forma}\n");
        let esperado = format!("Authorization: Basic {M}\n");
        assert_eq!(
            texto(&m.mascarar_tudo(saida.as_bytes())),
            esperado,
            "{forma}"
        );
        assert_eq!(texto(&um_bloco(&m, saida.as_bytes())), esperado, "{forma}");
    }
}

#[test]
fn v2_20_base64_url_safe_difere_do_padrao() {
    // Segredo cujo base64 tem `+` e `/`: o URL-safe troca por `-` e `_`.
    let segredo = "falso-PG:s3nha?01>>~";
    let m = mascara(&generico(&[segredo]));
    let padrao = b64(segredo.as_bytes(), false, true);
    let url = b64(segredo.as_bytes(), true, true);
    assert_ne!(padrao, url);
    for forma in [
        &padrao,
        &url,
        &t(padrao.trim_end_matches('=')),
        &t(url.trim_end_matches('=')),
    ] {
        assert_eq!(texto(&um_bloco(&m, forma.as_bytes())), M, "{forma}");
    }
}

#[test]
fn ataque_segredo_dentro_de_base64_maior() {
    // `usuario:token` com qualquer usuário, em qualquer alinhamento: o miolo do base64
    // do token é mascarado e não sobra pedaço que decodifique o token.
    let token = "falso-token-gh-01";
    let m = mascara(&generico(&[token]));
    for prefixo in ["x-access-token:", "x-access-tok:", "ab:", "a:", ""] {
        let codificado = b64(format!("{prefixo}{token}").as_bytes(), false, true);
        let saida = m.mascarar_tudo(codificado.as_bytes());
        assert!(texto(&saida).contains(M), "{prefixo}: {}", texto(&saida));
        // O que sobrou do base64 não tem mais que 2 caracteres colados ao segredo.
        let sobra: String = texto(&saida).replace(M, "");
        let limite = b64(prefixo.as_bytes(), false, false).len() + 2 + 4;
        assert!(sobra.len() <= limite, "{prefixo}: sobrou {sobra}");
    }
}

// ---- V2.21 ----

#[test]
fn v2_21_database_url_inteiro() {
    let e = pg_dev();
    let g = entrega(&e);
    assert_eq!(g.valor("DATABASE_URL"), Some(URL_PG));
    let m = Mascara::da_entrada(&e, Some(&g)).unwrap();
    for saida in [
        format!("DATABASE_URL={URL_PG}\n"),
        // O cliente pode imprimir o URL decodificado.
        format!("DATABASE_URL=postgresql://app:{SENHA_PG}@127.0.0.1:55432/app_teste\n"),
    ] {
        for out in [
            m.mascarar_tudo(saida.as_bytes()),
            um_bloco(&m, saida.as_bytes()),
        ] {
            sem_pedaco(&out, SENHA_PG.as_bytes());
            sem_pedaco(&out, pct(SENHA_PG, false, false).as_bytes());
            assert!(!texto(&out).contains("s3nha"), "{}", texto(&out));
        }
    }
    let out = m.mascarar_tudo(format!("DATABASE_URL={URL_PG}\n").as_bytes());
    assert_eq!(texto(&out), format!("DATABASE_URL={M}\n"));
}

#[test]
fn entrega_marca_o_que_e_segredo() {
    // Só o que a entrega marcou como segredo entra: host, porta, base e usuário não
    // viram `••••`.
    let e = pg_dev();
    let m = mascara(&e);
    let saida = "PGHOST=127.0.0.1 PGPORT=55432 PGDATABASE=app_teste PGUSER=app\n";
    assert_eq!(texto(&m.mascarar_tudo(saida.as_bytes())), saida);
}

#[test]
fn codigo_totp_nao_e_mascarado() {
    let e = valida(rascunho(
        "acme-wp-dev",
        RascunhoDados::Login {
            url: t("http://localhost:8081/wp-login.php"),
            usuario: t("wp_admin"),
            senha: s("falso-WP-senha-01"),
            totp_semente: s("JBSWY3DPEHPK3PXP"),
        },
    ));
    let g = entrega(&e);
    assert_eq!(g.valor("WARD_TOTP"), Some(CODIGO_FALSO));
    let m = Mascara::da_entrada(&e, Some(&g)).unwrap();
    let saida = format!("codigo {CODIGO_FALSO} semente JBSWY3DPEHPK3PXP senha falso-WP-senha-01");
    assert_eq!(
        texto(&m.mascarar_tudo(saida.as_bytes())),
        format!("codigo {CODIGO_FALSO} semente {M} senha {M}")
    );
}

// ---- V2.22 ----

#[test]
fn v2_22_segredo_que_contem_outro() {
    let maior = "falso-abc-123-xyz";
    let menor = "abc-123";
    let m = mascara(&generico(&[maior, menor]));
    for colado in [
        format!("{maior}{menor}"),
        format!("{menor}{maior}"),
        format!("{menor}{menor}"),
        format!("{maior}{maior}"),
        format!("{menor}{maior}{menor}"),
    ] {
        let saida = format!("<{colado}>");
        for out in [
            m.mascarar_tudo(saida.as_bytes()),
            um_bloco(&m, saida.as_bytes()),
        ] {
            assert_eq!(texto(&out), format!("<{M}>"), "{colado}");
        }
    }
    // O menor dentro do começo do maior, que depois diverge: o autômato está no meio do
    // maior quando o menor termina e precisa achá-lo pelo sufixo.
    let saida = format!("<falso-{menor}-!>");
    for out in [
        m.mascarar_tudo(saida.as_bytes()),
        um_bloco(&m, saida.as_bytes()),
    ] {
        assert_eq!(texto(&out), format!("<falso-{M}-!>"));
    }
}

#[test]
fn ataque_segredos_sobrepostos() {
    // O fim de um é o começo do outro: casar só o primeiro deixaria "-fim" do segundo.
    let a = "falso-sobre-01";
    let b = "sobre-01-fim";
    let m = mascara(&generico(&[a, b]));
    for colado in ["falso-sobre-01-fim", "falso-sobre-01-fimfalso-sobre-01"] {
        let saida = format!("[{colado}]");
        for out in [
            m.mascarar_tudo(saida.as_bytes()),
            um_bloco(&m, saida.as_bytes()),
        ] {
            assert_eq!(texto(&out), format!("[{M}]"), "{colado}");
            sem_pedaco(&out, a.as_bytes());
            sem_pedaco(&out, b.as_bytes());
        }
    }
}

// ---- V2.23 ----

#[test]
fn v2_23_anexo_inteiro_linha_e_valor() {
    let e = gama_generico();
    let m = mascara(&e);
    let casos = [
        ("FALSO_ENV=falso-anexo-01\n", M.to_owned()),
        ("FALSO_ENV=falso-anexo-01", M.to_owned()),
        // O conteúdo inteiro inclui a quebra de linha final.
        (
            "antes\nFALSO_ENV=falso-anexo-01\ndepois",
            format!("antes\n{M}depois"),
        ),
        (
            "antes FALSO_ENV=falso-anexo-01 depois",
            format!("antes {M} depois"),
        ),
        ("valor: falso-anexo-01.", format!("valor: {M}.")),
        ("licença falso-licenca-01", format!("licença {M}")),
        ("REGIAO=r-01", t("REGIAO=r-01")),
    ];
    for (saida, esperado) in casos {
        assert_eq!(
            texto(&m.mascarar_tudo(saida.as_bytes())),
            esperado,
            "{saida}"
        );
        assert_eq!(texto(&um_bloco(&m, saida.as_bytes())), esperado, "{saida}");
    }
    let b = b64(b"FALSO_ENV=falso-anexo-01\n", false, true);
    assert_eq!(texto(&m.mascarar_tudo(b.as_bytes())), M);
}

#[test]
fn anexo_com_varias_linhas() {
    let conteudo = "# comentário longo\n  export TOKEN=\"falso-tok-anexo\"\nA=1\nSEGREDO_X=falso-x-anexo-02\r\n";
    let mut r = rascunho("teste-anexo", RascunhoDados::Generico);
    r.extras = vec![extra("X", "y", false)];
    r.anexos = vec![anexo("app.env", conteudo.as_bytes())];
    let m = mascara(&valida(r));
    let casos = [
        (conteudo.to_owned(), M.to_owned()),
        // Valor com e sem aspas, linha aparada, linha com CRLF.
        (t("tok=falso-tok-anexo;"), format!("tok={M};")),
        (t("\"falso-tok-anexo\""), M.to_owned()),
        (t("export TOKEN=\"falso-tok-anexo\""), M.to_owned()),
        (t("SEGREDO_X=falso-x-anexo-02\n"), format!("{M}\n")),
        (t("v falso-x-anexo-02 v"), format!("v {M} v")),
        // Linha curta (< 6) não entra sozinha.
        (t("A=1"), t("A=1")),
    ];
    for (saida, esperado) in casos {
        assert_eq!(texto(&um_bloco(&m, saida.as_bytes())), esperado, "{saida}");
    }
}

// ---- V2.24 ----

#[test]
fn v2_24_saida_sem_segredo_sai_igual() {
    let m = mascara(&pg_dev());
    let mut sorteio = Sorteio(0x5eed_0001);
    let mut binario = vec![0u8; 1024 * 1024];
    for b in &mut binario {
        *b = sorteio.proximo() as u8;
    }
    // Escapes e prefixos do segredo espalhados, sem formar o segredo.
    binario[100..106].copy_from_slice(b"%3A%2F");
    binario[2000..2008].copy_from_slice(b"falso-PG");
    let saidas: Vec<Vec<u8>> = vec![
        b"texto comum, nada de segredo aqui\n".to_vec(),
        vec![0xff, 0xfe, b'f', 0xc3, 0x28, b'%', b'4', 0x80, b'+'],
        binario,
        // O fim que ficou segurado (começo do segredo, escape pela metade) sai igual.
        b"termina com falso-PG:s3".to_vec(),
        b"termina com %4".to_vec(),
        b"termina com %".to_vec(),
    ];
    for saida in &saidas {
        assert_eq!(&m.mascarar_tudo(saida), saida);
        let cortes: Vec<usize> = (1..saida.len()).step_by(4093).collect();
        assert_eq!(&em_pedacos(&m, saida, &cortes), saida);
        if saida.len() < 100 {
            assert_eq!(&um_bloco(&m, saida), saida);
        }
    }
}

#[test]
fn mascara_sem_segredo_nao_mexe_em_nada() {
    let mut r = rascunho("so-extras", RascunhoDados::Generico);
    r.extras = vec![extra("REGIAO", "r-01", false)];
    let e = valida(r);
    let m = mascara(&e);
    assert_eq!(m.mascarar_tudo(b"r-01 \xff"), b"r-01 \xff");
}

// ---- Propriedade: cortes aleatórios ----

#[test]
fn propriedade_cortes_aleatorios() {
    let e = pg_dev();
    let g = entrega(&e);
    let m = Mascara::da_entrada(&e, Some(&g)).unwrap();
    let variacoes: Vec<String> = vec![
        t(SENHA_PG),
        pct(SENHA_PG, false, false),
        pct(SENHA_PG, true, false),
        b64(SENHA_PG.as_bytes(), false, true),
        b64(SENHA_PG.as_bytes(), true, false),
        b64(format!("app:{SENHA_PG}").as_bytes(), false, true),
        t(URL_PG),
    ];
    let ruido: &[u8] = b"abc %2 % falso-PG: s3nha 01@# \n\xff\x00=+/";
    let mut sorteio = Sorteio(0x0dd_ba11);
    for rodada in 0..300 {
        let mut saida = Vec::new();
        let mut trechos = Vec::new();
        let mut inseridas = 0;
        for _ in 0..1 + sorteio.ate(8) {
            for _ in 0..sorteio.ate(12) {
                saida.push(ruido[sorteio.ate(ruido.len())]);
            }
            // Separador: o ruído não termina a variação de outro jeito.
            saida.push(b' ');
            let ini = saida.len();
            saida.extend_from_slice(variacoes[sorteio.ate(variacoes.len())].as_bytes());
            trechos.push((ini, saida.len()));
            saida.push(b' ');
            inseridas += 1;
        }
        let inteira = m.mascarar_tudo(&saida);
        // D34 revisto: S4 vale para o segredo inteiro dentro de um pedaço; os cortes
        // caem fora das variações (o ruído entre elas pode ser cortado em qualquer
        // lugar, inclusive no meio de um `%X` ou de um começo de segredo).
        let mut cortes: Vec<usize> = (0..sorteio.ate(10))
            .map(|_| sorteio.ate(saida.len()))
            .filter(|&c| !trechos.iter().any(|&(a, b)| a < c && c < b))
            .collect();
        cortes.sort_unstable();
        let pedacos = em_pedacos(&m, &saida, &cortes);
        assert_eq!(
            pedacos, inteira,
            "rodada {rodada}: cortes mudaram o resultado"
        );
        sem_pedaco(&pedacos, SENHA_PG.as_bytes());
        for v in &variacoes {
            assert!(
                !texto(&pedacos).contains(v.as_str()),
                "rodada {rodada}: sobrou {v}"
            );
        }
        let marcas = texto(&pedacos).matches(M).count();
        assert!(marcas >= 1 && marcas <= inseridas, "rodada {rodada}");
    }
}

// ---- Ataques ----

#[test]
fn ataque_json_escapado() {
    let m = mascara(&pg_dev());
    // PHP (`json_encode`) escapa a barra; JSON comum, não.
    for saida in [
        r#"{"senha":"falso-PG:s3nha\/01@#"}"#,
        r#"{"senha":"falso-PG:s3nha/01@#"}"#,
    ] {
        assert_eq!(
            texto(&m.mascarar_tudo(saida.as_bytes())),
            format!(r#"{{"senha":"{M}"}}"#)
        );
    }
    let m = mascara(&generico(&["falso\"aspas\\01"]));
    assert_eq!(
        texto(&m.mascarar_tudo(br#"{"v":"falso\"aspas\\01"}"#)),
        format!(r#"{{"v":"{M}"}}"#)
    );
}

#[test]
fn ataque_saida_patologica_continua_linear() {
    // Segredo de 64 KiB repetitivo contra 1 MiB do mesmo byte (cru e em %61): uma
    // busca ingênua faria 64 bilhões de comparações; o autômato é linear.
    let segredo = "a".repeat(64 * 1024);
    let m = mascara(&generico(&[&segredo]));
    let saida = vec![b'a'; 1024 * 1024];
    let mut f = m.fluxo();
    let mut out = Vec::new();
    for pedaco in saida.chunks(4096) {
        f.mascarar(pedaco, &mut out);
        nada_segurado(&f);
    }
    f.terminar(&mut out);
    // D34 revisto: os 15 pedaços antes do primeiro casamento saíram na hora; daí em
    // diante cada pedaço completa o segredo de novo e vira uma marca só.
    assert_eq!(texto(&out), format!("{}{M}", "a".repeat(15 * 4096)));
    assert_eq!(texto(&m.mascarar_tudo(&saida)), M);
    let codificada = "%61".repeat(128 * 1024);
    assert_eq!(texto(&m.mascarar_tudo(codificada.as_bytes())), M);
    // Saída um pouco menor que o segredo: não é segredo, sai igual.
    let quase = vec![b'a'; 64 * 1024 - 1];
    assert_eq!(m.mascarar_tudo(&quase), quase);
}

#[test]
fn stdout_e_stderr_em_fluxos_separados() {
    let m = mascara(&pg_dev());
    let (mut out, mut err) = (m.fluxo(), m.fluxo());
    let (mut so, mut se) = (Vec::new(), Vec::new());
    out.mascarar(b"o falso-PG:", &mut so);
    err.mascarar(b"e falso-PG:s3nha/01@# e", &mut se);
    out.mascarar(b"s3nha/01@# o", &mut so);
    out.terminar(&mut so);
    err.terminar(&mut se);
    // O stderr não completa o casamento do stdout; o começo que saiu no stdout fica
    // como saiu (D34 revisto) e o pedaço que completa o segredo vira marca.
    assert_eq!(texto(&so), format!("o falso-PG:{M} o"));
    assert_eq!(texto(&se), format!("e {M} e"));
}

#[test]
fn salto_e_tunel_juntam_as_duas_entradas() {
    let a = pg_dev();
    let b = generico(&["falso-ssh-senha-01"]);
    let m = ConstrutorMascara::new()
        .entrada(&a)
        .entrada(&b)
        .construir()
        .unwrap();
    assert_eq!(
        texto(&m.mascarar_tudo(b"falso-ssh-senha-01 falso-PG:s3nha/01@#")),
        format!("{M} {M}")
    );
}

#[test]
fn segredo_curto_e_mascarado_e_avisado() {
    let m = mascara(&generico(&["ab12"]));
    assert!(m.tem_segredo_curto());
    assert_eq!(texto(&m.mascarar_tudo(b"x ab12 y")), format!("x {M} y"));
    assert!(!mascara(&pg_dev()).tem_segredo_curto());
    // Anexo não conta como segredo curto (as linhas curtas é que ficam de fora).
    assert!(!mascara(&gama_generico()).tem_segredo_curto());
}

#[test]
fn ataque_anexo_grande_demais_falha_fechado() {
    // Cada entrada cabe no orçamento (D29), mas juntas passam do limite da máscara:
    // acima dele a máscara não é montada; o `ward run` não roda sem máscara.
    let grandes: Vec<Entrada> = (0..8)
        .map(|i| {
            let mut r = rascunho(&format!("anexo-grande-{i}"), RascunhoDados::Generico);
            r.extras = vec![extra("X", "y", false)];
            r.anexos = vec![anexo(
                "grande.bin",
                &vec![b'z'; crate::limits::MAX_ATTACHMENTS_TOTAL],
            )];
            valida(r)
        })
        .collect();
    let mut soma = 0;
    let mut usadas = 0;
    for e in &grandes {
        let custo = custo_da_entrada(e).expect("cabe no orçamento");
        assert!(custo <= MASK_BUDGET);
        soma += custo;
        usadas += 1;
        if soma > PATTERN_LIMIT {
            break;
        }
    }
    assert!(soma > PATTERN_LIMIT, "o teste precisa passar do limite");
    let mut c = ConstrutorMascara::new();
    for e in &grandes[..usadas] {
        c = c.entrada(e);
    }
    let erro = c.construir().unwrap_err();
    assert_eq!(erro, ErroMascara::GrandeDemais);
    assert!(!erro.to_string().contains("zzzz"));
}

#[test]
fn ataque_debug_sem_segredo() {
    let e = pg_dev();
    let g = entrega(&e);
    let m = Mascara::da_entrada(&e, Some(&g)).unwrap();
    let mut f = m.fluxo();
    let mut out = Vec::new();
    f.mascarar(b"falso-PG:s3n", &mut out);
    let c = ConstrutorMascara::new().entrada(&e).entrega(&g);
    for d in [
        format!("{m:?}"),
        format!("{f:?}"),
        format!("{c:?}"),
        format!("{m:#?}"),
    ] {
        assert!(!d.contains("falso"), "{d}");
        assert!(!d.contains("s3n"), "{d}");
    }
}

// ---- S8 / V2.28: zera ao sair, sem Clone ----

/// Falha a compilação se `T` implementar `Clone` (dois impls viram ambíguos).
trait NaoClone<A> {
    fn conferir() {}
}
impl<T> NaoClone<()> for T {}
#[allow(dead_code)]
struct ImplementaClone;
impl<T: Clone> NaoClone<ImplementaClone> for T {}

fn zera_ao_sair<T: ZeroizeOnDrop>() {}

#[test]
fn mascara_e_fluxo_zeram_e_nao_clonam() {
    <Mascara as NaoClone<_>>::conferir();
    <Fluxo<'static> as NaoClone<_>>::conferir();
    <ConstrutorMascara<'static> as NaoClone<_>>::conferir();
    zera_ao_sair::<Mascara>();
    zera_ao_sair::<Fluxo<'static>>();
}

#[test]
fn automato_alocado_uma_vez() {
    // A capacidade calculada antes cobre todos os nós: nenhuma realocação deixa cópia
    // dos padrões em memória liberada.
    let e = gama_generico();
    let g = entrega(&e);
    let c = ConstrutorMascara::new().entrada(&e).entrega(&g);
    let mut total = 0;
    visitar(&c, &mut |p| {
        total += p.len();
        Ok(())
    })
    .unwrap();
    let m = c.construir().unwrap();
    assert!(m.automato.nos() <= total + 1);
}

// ---- Codificações (não é cripto, mas tem de bater com a RFC 4648) ----

#[test]
fn base64_rfc4648() {
    let casos = [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ];
    for (entrada, esperado) in casos {
        let mut out = Zeroizing::new(Vec::new());
        base64(entrada.as_bytes(), B64_PADRAO, true, &mut out);
        assert_eq!(texto(&out), esperado);
        assert_eq!(b64(entrada.as_bytes(), false, true), esperado);
    }
    let mut out = Zeroizing::new(Vec::new());
    base64(&[0xfb, 0xff], B64_URL, false, &mut out);
    assert_eq!(texto(&out), "-_8");
}

// ---- V2.38: `CHAVE=valor` curto (D34) ----

#[test]
fn v2_38_valor_curto_de_chave_valor_nao_vira_padrao() {
    let mut r = rascunho("gama-generico", RascunhoDados::Generico);
    r.extras = vec![extra("REGIAO", "r-01", false)];
    r.anexos = vec![anexo(
        "app.env",
        b"PORT=8080\nDEBUG=1\nFALSO_ENV=falso-anexo-01\n",
    )];
    let m = mascara(&valida(r));
    let casos = [
        // Valor curto sozinho na saída: sai intacto (RA3, aceito).
        (t("porta 8080 debug 1"), t("porta 8080 debug 1")),
        (t("8080"), t("8080")),
        (t("1"), t("1")),
        // O valor longo sozinho e a linha inteira viram `••••`.
        (t("falso-anexo-01"), M.to_owned()),
        (t("PORT=8080"), M.to_owned()),
        (t("x PORT=8080 y"), format!("x {M} y")),
        (t("DEBUG=1"), M.to_owned()),
        (t("[FALSO_ENV=falso-anexo-01]"), format!("[{M}]")),
    ];
    for (saida, esperado) in casos {
        for out in [
            m.mascarar_tudo(saida.as_bytes()),
            um_bloco(&m, saida.as_bytes()),
        ] {
            assert_eq!(texto(&out), esperado, "{saida}");
        }
    }
    // Com aspas: `"abcd"` tem 6 caracteres e vira padrão; sem aspas (4), não.
    let mut r = rascunho("teste-aspas", RascunhoDados::Generico);
    r.extras = vec![extra("X", "y", false)];
    r.anexos = vec![anexo("a.env", b"CURTO=\"abcd\"\n")];
    let m = mascara(&valida(r));
    assert_eq!(texto(&m.mascarar_tudo(b"v \"abcd\" v")), format!("v {M} v"));
    assert_eq!(texto(&m.mascarar_tudo(b"v abcd v")), "v abcd v");

    // O segredo de campo segue mascarado em qualquer tamanho (D11).
    let curto = generico(&["ab12"]);
    let m = mascara(&curto);
    assert_eq!(texto(&m.mascarar_tudo(b"x ab12 y")), format!("x {M} y"));
    assert_eq!(texto(&um_bloco(&m, b"x ab12 y")), format!("x {M} y"));
}

#[test]
fn v2_38_mutacao_sem_minimo_mascararia_o_valor_curto() {
    // Controle do teste acima: o mesmo valor curto como segredo de campo é mascarado, então
    // a saída intacta de V2.38 vem da regra de D34, não de um defeito da busca.
    let m = mascara(&generico(&["8080"]));
    assert_eq!(texto(&m.mascarar_tudo(b"porta 8080")), format!("porta {M}"));
}

// ---- Fase 19c: D36 (por linha) e D37 (formas escapadas) ----

/// Maior trecho contíguo do segredo que sobra na saída (o repro `mascara.rs` do auditor).
fn maior_trecho(saida: &[u8], segredo: &[u8]) -> usize {
    let mut melhor = 0;
    for i in 0..segredo.len() {
        for j in (i + melhor + 1)..=segredo.len() {
            if saida.windows(j - i).any(|w| w == &segredo[i..j]) {
                melhor = j - i;
            } else {
                break;
            }
        }
    }
    melhor
}

/// A última linha de base64 da chave de teste: curta (menos de 6 caracteres), como a
/// sobra do fim de uma chave real. É o único trecho que pode sobrar (V2.41).
const FIM_CURTO: &str = "AQID";

/// Chave privada falsa no formato OpenSSH, em várias linhas de 70 caracteres de base64
/// pseudoaleatório (semente fixa) e uma última linha curta. Montada em tempo de
/// execução (o texto do cabeçalho não fica inteiro no repositório). Não é chave de
/// verdade: só bytes falsos com a forma de uma.
fn chave_de_varias_linhas() -> String {
    const ALFABETO: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let rotulo = ["OPENSSH", "PRIVATE", "KEY"].join(" ");
    let mut sorteio = Sorteio(0x0c4a_7e19);
    let mut c = format!("-----BEGIN {rotulo}-----\n");
    for _ in 0..6 {
        for _ in 0..70 {
            c.push(char::from(ALFABETO[sorteio.ate(64)]));
        }
        c.push('\n');
    }
    c.push_str(FIM_CURTO);
    c.push('\n');
    c.push_str(&format!("-----END {rotulo}-----\n"));
    c
}

fn servidor_com_chave(chave: &str) -> Entrada {
    valida(rascunho(
        "acme-ssh-chave-prod",
        RascunhoDados::Servidor {
            host: t("127.0.0.1"),
            porta: t("2223"),
            usuario: t("deploy"),
            senha: None,
            chave_privada: s(chave),
            passphrase: s("falso-passphrase-01"),
            salto: String::new(),
        },
    ))
}

/// Toda linha da chave com 6 caracteres ou mais sumiu (nenhum pedaço de 6 dela sobra),
/// e o maior trecho da chave que sobra é a linha curta do fim com as quebras ao lado.
fn sem_linha_da_chave(caso: &str, saida: &[u8], chave: &str) {
    for linha in chave
        .lines()
        .filter(|l| l.chars().count() >= TRUSTED_MINIMUM)
    {
        for janela in linha.as_bytes().windows(TRUSTED_MINIMUM) {
            assert!(
                !saida.windows(janela.len()).any(|w| w == janela),
                "{caso}: sobrou pedaço de uma linha da chave: {}",
                texto(saida)
            );
        }
    }
    let maior = maior_trecho(saida, chave.as_bytes());
    assert!(
        maior <= FIM_CURTO.len() + 2,
        "{caso}: sobraram {maior} bytes seguidos da chave: {}",
        texto(saida)
    );
}

/// V2.41 (D36), o achado médio 1 do auditor: a chave privada em campo (não anexo) era
/// mascarada só inteira. `grep -v -- -----`, `head -n 4`, o CRLF do pty e cada linha
/// sozinha deixavam passar quase tudo.
#[test]
fn v2_41_chave_em_campo_mascarada_por_linha() {
    let chave = chave_de_varias_linhas();
    let e = servidor_com_chave(&chave);
    let m = mascara(&e);
    let linhas: Vec<&str> = chave.lines().collect();

    let crlf = chave.replace('\n', "\r\n");
    let cabeca: String = linhas[..4].iter().map(|l| format!("{l}\n")).collect();
    let miolo: String = linhas[1..linhas.len() - 2]
        .iter()
        .map(|l| format!("{l}\n"))
        .collect();
    let mut casos: Vec<(String, String)> = vec![
        (t("cat"), chave.clone()),
        (t("CRLF do pty"), crlf.clone()),
        (t("head -n 4"), cabeca),
        (t("grep -v -- -----"), miolo.clone()),
        (t("grep -v com CRLF"), miolo.replace('\n', "\r\n")),
    ];
    for (i, l) in linhas.iter().enumerate() {
        casos.push((format!("linha {i} sozinha"), format!("x {l} y\n")));
        casos.push((format!("linha {i} com CR"), format!("{l}\r\n")));
        casos.push((format!("linha {i} aparada"), format!("   {l}   \n")));
    }
    for (caso, saida) in &casos {
        sem_linha_da_chave(caso, &m.mascarar_tudo(saida.as_bytes()), &chave);
        sem_linha_da_chave(caso, &um_bloco(&m, saida.as_bytes()), &chave);
    }
    // A chave inteira continua um `••••` só.
    assert_eq!(texto(&m.mascarar_tudo(chave.as_bytes())), M);
}

/// V2.41 (D36): extra secreto de várias linhas (JSON de conta de serviço) e o valor de
/// `"chave": "valor"`; e a linha `token: valor` de um anexo YAML.
#[test]
fn v2_41_json_de_varias_linhas_e_yaml() {
    const VALOR: &str = "falso-chave-de-conta-de-servico-0123456789";
    let json = format!("{{\n  \"type\": \"service_account\",\n  \"private_key\": \"{VALOR}\"\n}}");
    let mut r = rascunho("x-gen", RascunhoDados::Generico);
    r.extras = vec![extra("GCP", &json, true)];
    let m = mascara(&valida(r));
    let linha = format!("  \"private_key\": \"{VALOR}\"");
    for (caso, saida, esperado) in [
        ("JSON inteiro", json.clone(), M.to_owned()),
        ("a linha", format!("{linha}\n"), format!("{M}\n")),
        ("jq -r .private_key", format!("{VALOR}\n"), format!("{M}\n")),
        ("o valor com aspas", format!("\"{VALOR}\""), M.to_owned()),
        (
            "o valor no meio do texto",
            format!("chave={VALOR};"),
            format!("chave={M};"),
        ),
    ] {
        assert_eq!(
            texto(&m.mascarar_tudo(saida.as_bytes())),
            esperado,
            "{caso}"
        );
        assert_eq!(texto(&um_bloco(&m, saida.as_bytes())), esperado, "{caso}");
    }

    // Anexo YAML (kubeconfig): `token: valor`, com e sem aspas, e item de lista.
    let kube = "apiVersion: v1\nusers:\n- name: dev\n  user:\n    token: falso-token-kube-01\n    senha: \"falso-senha-kube-02\"\n- item-da-lista: falso-dados-kube-03\n";
    let mut r = rascunho("x-kube", RascunhoDados::Generico);
    r.extras = vec![extra("X", "y", false)];
    r.anexos = vec![anexo("kubeconfig", kube.as_bytes())];
    let m = mascara(&valida(r));
    for valor in [
        "falso-token-kube-01",
        "falso-senha-kube-02",
        "falso-dados-kube-03",
    ] {
        for saida in [
            format!("{valor}\n"),
            format!("v={valor}."),
            format!("'{valor}'"),
        ] {
            let fora = texto(&m.mascarar_tudo(saida.as_bytes()));
            assert!(!fora.contains(valor), "{saida} -> {fora}");
            assert!(fora.contains(M), "{saida}");
        }
    }
    // `host:porta` e URL não viram `chave: valor` (sem espaço depois dos dois pontos).
    let mut r = rascunho("x-url", RascunhoDados::Generico);
    r.extras = vec![extra("X", "y", false)];
    r.anexos = vec![anexo("hosts.txt", b"db:5432\n")];
    let m = mascara(&valida(r));
    assert_eq!(texto(&m.mascarar_tudo(b"porta 5432")), "porta 5432");
}

/// Controle de D36: sem a passada por linha nos segredos de campo, `grep -v` deixaria
/// a chave passar (o achado do auditor). A máscara montada só com o texto inteiro, como
/// era antes, deixa sobrar as linhas.
#[test]
fn v2_41_controle_sem_a_passada_por_linha_vazaria() {
    let chave = chave_de_varias_linhas();
    let e = servidor_com_chave(&chave);
    // Só o segredo inteiro e as variações dele, sem `linhas` (o código de antes).
    let mut g = Gerador::novo();
    let mut padroes: Vec<Vec<u8>> = Vec::new();
    for sg in e.segredos() {
        g.segredo(sg.valor.expor(), &mut |p| {
            padroes.push(p.to_vec());
            Ok(())
        })
        .unwrap();
    }
    let linhas: Vec<&str> = chave.lines().collect();
    let miolo: String = linhas[1..linhas.len() - 2]
        .iter()
        .map(|l| format!("{l}\n"))
        .collect();
    // Nenhum padrão antigo casa com o miolo: ele sairia em claro.
    assert!(
        !padroes
            .iter()
            .any(|p| miolo.as_bytes().windows(p.len()).any(|w| w == &p[..]))
    );
    // Com a máscara de verdade, não sobra.
    sem_linha_da_chave(
        "controle",
        &mascara(&e).mascarar_tudo(miolo.as_bytes()),
        &chave,
    );
}

fn banco_com_senha(senha: &str) -> Entrada {
    valida(rascunho(
        "x-pg",
        RascunhoDados::Banco {
            motor: t("postgres"),
            host: t("127.0.0.1"),
            porta: String::new(),
            base: t("app"),
            usuario: t("app"),
            senha: s(senha),
            tunel: String::new(),
        },
    ))
}

/// V2.42 (D37), o achado baixo 5: o JSON do Go (`docker inspect`), o `declare -p` do
/// bash e o `json.dumps` do Python escrevem a senha de outro jeito.
#[test]
fn v2_42_formas_escapadas() {
    let senha = "Ab3$k&<9>Zq!";
    let m = mascara(&banco_com_senha(senha));
    for (caso, saida, esperado) in [
        (
            "JSON do Go",
            r#"{"Env":["PGPASSWORD=Ab3$k\u0026\u003c9\u003eZq!"]}"#,
            format!(r#"{{"Env":["PGPASSWORD={M}"]}}"#),
        ),
        (
            "declare -p",
            r#"declare -x PGPASSWORD="Ab3\$k&<9>Zq!""#,
            format!(r#"declare -x PGPASSWORD="{M}""#),
        ),
        ("literal", senha, M.to_owned()),
        (
            "JSON padrão",
            r#"{"p":"Ab3$k&<9>Zq!"}"#,
            format!(r#"{{"p":"{M}"}}"#),
        ),
    ] {
        let fora = m.mascarar_tudo(saida.as_bytes());
        assert_eq!(texto(&fora), esperado, "{caso}");
        sem_pedaco(&fora, senha.as_bytes());
        assert_eq!(texto(&um_bloco(&m, saida.as_bytes())), esperado, "{caso}");
    }

    // `json.dumps` com `ensure_ascii` (o padrão do Python): fora do ASCII vira `\uXXXX`.
    let m = mascara(&banco_com_senha("senha-ação-01"));
    assert_eq!(
        texto(&m.mascarar_tudo(br#"{"p": "senha-a\u00e7\u00e3o-01"}"#)),
        format!(r#"{{"p": "{M}"}}"#)
    );
    assert_eq!(texto(&m.mascarar_tudo("senha-ação-01".as_bytes())), M);
    // Acima de U+FFFF, o par substituto.
    let m = mascara(&banco_com_senha("falso-😀-01"));
    assert_eq!(texto(&m.mascarar_tudo(br"falso-\ud83d\ude00-01")), M);
    assert_eq!(texto(&m.mascarar_tudo("falso-😀-01".as_bytes())), M);
    // Go: U+2028 e U+2029 também.
    let m = mascara(&banco_com_senha("falso\u{2028}linha\u{2029}01"));
    assert_eq!(texto(&m.mascarar_tudo(br"falso\u2028linha\u202901")), M);

    // Saída sem segredo com `&`, `<`, `$`, `\u0026` e aspas sai igual.
    let m = mascara(&banco_com_senha(senha));
    for limpa in [
        "a & b <c> $HOME \"x\" \\u0026 `y`",
        r#"{"Env":["HOME=/root","X=a\u0026b"]}"#,
        "declare -x PATH=\"/usr/bin\"",
    ] {
        assert_eq!(texto(&m.mascarar_tudo(limpa.as_bytes())), limpa);
    }
}

/// D37: cada forma só existe quando difere da literal (senão o padrão seria repetido e
/// pesaria no orçamento à toa).
#[test]
fn formas_escapadas_so_quando_mudam() {
    let contar = |valor: &str| {
        let mut g = Gerador::novo();
        let mut n = 0usize;
        g.segredo(valor.as_bytes(), &mut |_| {
            n += 1;
            Ok(())
        })
        .unwrap();
        n
    };
    let simples = contar("falso-simples-01");
    assert_eq!(contar("falso-simples-02"), simples);
    // `&`: a forma do Go e a do `printf %q` (D54). `$`: a das aspas duplas e a do
    // `printf %q`. `ç`: só a do `ensure_ascii` (o `printf %q` não vale fora do ASCII).
    assert_eq!(contar("falso-simples&01"), simples + 2);
    assert_eq!(contar("falso-simples$01"), simples + 2);
    assert_eq!(contar("falso-simplesç01"), simples + 1);
}

// ---- Fase 19e: D54 (reauditoria do M2, repro `desvio.rs`) ----

const BASH: &str = "/bin/bash";
const PYTHON: &str = "/usr/bin/python3";

/// Roda `prog` com o ambiente limpo (só `PATH`, locale UTF-8 e as variáveis dadas) e
/// devolve o stdout. `None` quando o programa não existe nesta máquina (o teste avisa e
/// segue com os casos montados à mão).
fn rodar(prog: &str, args: &[&str], env: &[(&str, &str)]) -> Option<Vec<u8>> {
    if !std::path::Path::new(prog).exists() {
        eprintln!("{prog} não existe aqui: caso do programa real pulado");
        return None;
    }
    let mut c = std::process::Command::new(prog);
    c.args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C.UTF-8");
    for (k, v) in env {
        c.env(k, v);
    }
    let saida = c.output().expect("rodar o programa de teste");
    assert!(saida.status.success(), "{prog} falhou");
    Some(saida.stdout)
}

fn bash(script: &str, env: &[(&str, &str)]) -> Option<Vec<u8>> {
    rodar(BASH, &["--norc", "--noprofile", "-c", script], env)
}

fn python(script: &str, env: &[(&str, &str)]) -> Option<Vec<u8>> {
    rodar(PYTHON, &["-I", "-c", script], env)
}

/// A saída como um leitor a veria depois de desfazer os escapes de JSON (`\uXXXX`, com
/// par substituto, `\"`, `\\`, `\/`, `\n`, `\t`) e a barra do shell (`\x` vira `x`).
/// Só para conferir que nenhum trecho do segredo sobra nem na forma decodificada.
fn decodificada(saida: &[u8]) -> Vec<u8> {
    let s = String::from_utf8_lossy(saida);
    let c: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let hex4 = |c: &[char], i: usize| -> Option<u32> {
        let h: String = c.get(i..i + 4)?.iter().collect();
        u32::from_str_radix(&h, 16).ok()
    };
    while i < c.len() {
        if c[i] != '\\' || i + 1 >= c.len() {
            out.push(c[i]);
            i += 1;
            continue;
        }
        match c[i + 1] {
            'u' => match hex4(&c, i + 2) {
                Some(alto @ 0xd800..=0xdbff)
                    if c.get(i + 6) == Some(&'\\') && c.get(i + 7) == Some(&'u') =>
                {
                    let baixo = hex4(&c, i + 8).unwrap_or(0);
                    let n = 0x10000 + ((alto - 0xd800) << 10) + (baixo.wrapping_sub(0xdc00));
                    out.push(char::from_u32(n).unwrap_or('?'));
                    i += 12;
                }
                Some(n) => {
                    out.push(char::from_u32(n).unwrap_or('?'));
                    i += 6;
                }
                None => {
                    out.push('u');
                    i += 2;
                }
            },
            'n' => {
                out.push('\n');
                i += 2;
            }
            't' => {
                out.push('\t');
                i += 2;
            }
            outro => {
                out.push(outro);
                i += 2;
            }
        }
    }
    // `'\''` (aspas simples do shell) volta a ser `'`.
    out.replace("'''", "'").into_bytes()
}

/// Nenhum trecho de 6 bytes ou mais do alvo sobra na saída mascarada, nem crua nem
/// decodificada. A mensagem diz o caso, nunca o valor.
fn sem_trecho_de_6(caso: &str, m: &Mascara, saida: &[u8], alvo: &str) {
    let fora = m.mascarar_tudo(saida);
    let alvo = alvo.as_bytes();
    assert!(
        maior_trecho(&fora, alvo) < 6,
        "{caso}: sobrou trecho do segredo"
    );
    assert!(
        maior_trecho(&decodificada(&fora), alvo) < 6,
        "{caso}: sobrou trecho do segredo na forma decodificada"
    );
}

fn api_de_varias_linhas(token: &str) -> Entrada {
    valida(rascunho(
        "beta-api-varias",
        RascunhoDados::Api {
            url: String::new(),
            token: s(token),
            variavel: t("GITHUB_TOKEN"),
        },
    ))
}

/// V2.54 (D54): o token de `api` de várias linhas passa por linha já pela entrega, sem
/// a entrada no construtor (`head -n1 <<<"$GITHUB_TOKEN"`, cada linha sozinha, com `\r`
/// e aparada). O mesmo com `Mascara::da_entrada`.
#[test]
fn v2_54_entrega_de_varias_linhas_por_linha() {
    let um = "falso-tok-linha-um-01";
    let dois = "falso-tok-linha-dois-02";
    let token = format!("{um}\n{dois}");
    let e = api_de_varias_linhas(&token);
    let g = entrega(&e);
    let so_entrega = ConstrutorMascara::new().entrega(&g).construir().unwrap();
    let completa = Mascara::da_entrada(&e, Some(&g)).unwrap();
    let mut saidas: Vec<(String, Vec<u8>)> = vec![
        ("linha 1".into(), format!("{um}\n").into_bytes()),
        ("linha 2".into(), format!("{dois}\n").into_bytes()),
        ("linha 1 com CR".into(), format!("{um}\r\n").into_bytes()),
        (
            "linha 2 aparada".into(),
            format!("  {dois}  \n").into_bytes(),
        ),
    ];
    if let Some(o) = bash("head -n1 <<<\"$GITHUB_TOKEN\"", &[("GITHUB_TOKEN", &token)]) {
        assert_eq!(o, format!("{um}\n").into_bytes());
        saidas.push(("head -n1 (bash)".into(), o));
    }
    if let Some(o) = bash("tail -n1 <<<\"$GITHUB_TOKEN\"", &[("GITHUB_TOKEN", &token)]) {
        saidas.push(("tail -n1 (bash)".into(), o));
    }
    for (nome, m) in [("só a entrega", &so_entrega), ("da_entrada", &completa)] {
        for (caso, saida) in &saidas {
            for alvo in [um, dois] {
                sem_trecho_de_6(&format!("{nome}: {caso}"), m, saida, alvo);
            }
        }
    }
    // Controle: sem a passada por linha da entrega (só o valor inteiro), a linha 1
    // sozinha sobrava inteira.
    let mut g_ = Gerador::novo();
    let mut padroes: Vec<Vec<u8>> = Vec::new();
    g_.segredo(token.as_bytes(), &mut |p| {
        padroes.push(p.to_vec());
        Ok(())
    })
    .unwrap();
    assert!(!padroes.iter().any(|p| p == um.as_bytes()));
}

/// V2.55 (D54): as formas de texto valem por linha. As linhas de um extra secreto de
/// várias linhas, escritas pelo `json.dumps` do Python (lista de linhas e uma linha num
/// objeto, `ensure_ascii`), pelo `declare -p` do bash de uma linha com `$` e pelo JSON
/// do Go, viram `••••`, sem trecho de 6+ nem na forma decodificada; e a saída sem
/// segredo com `\u00e7` segue intacta.
#[test]
fn v2_55_formas_de_texto_por_linha() {
    let linhas = [
        "linha-um-ação-01",
        "linha-dois-ação-02",
        "linha-tres-Ab3$k&<9>Zq!",
    ];
    let nota = linhas.join("\n");
    let e = generico(&[&nota]);
    let m = mascara(&e);
    let env = [("NOTA", nota.as_str())];
    let mut saidas: Vec<(&str, Vec<u8>)> = vec![
        // JSON do Go da linha 3, montado à mão (o `encoding/json`).
        (
            "Go linha 3",
            br#"{"l":"linha-tres-Ab3$k\u0026\u003c9\u003eZq!"}"#.to_vec(),
        ),
        // `ensure_ascii` à mão, se o python não existir.
        (
            "ensure_ascii linha 1",
            br#"["linha-um-a\u00e7\u00e3o-01"]"#.to_vec(),
        ),
        (
            "aspas duplas linha 3",
            br#"declare -- L="linha-tres-Ab3\$k&<9>Zq!""#.to_vec(),
        ),
    ];
    if let Some(o) = python(
        "import os,json;print(json.dumps(os.environ['NOTA'].splitlines()))",
        &env,
    ) {
        assert!(
            o.windows(6).any(|w| w == br"\u00e7"),
            "python sem ensure_ascii"
        );
        saidas.push(("json.dumps(splitlines())", o));
    }
    if let Some(o) = python(
        "import os,json;print(json.dumps({'l': os.environ['NOTA'].splitlines()[0]}))",
        &env,
    ) {
        saidas.push(("json.dumps({l: linha 1})", o));
    }
    if let Some(o) = bash("L=$(sed -n 3p <<<\"$NOTA\"); declare -p L", &env) {
        assert!(o.windows(2).any(|w| w == br"\$"), "declare -p sem a barra");
        saidas.push(("declare -p L (linha 3)", o));
    }
    for (caso, saida) in &saidas {
        for alvo in linhas {
            sem_trecho_de_6(caso, &m, saida, alvo);
        }
    }
    for limpa in [
        r#"{"cidade": "S\u00e3o Paulo", "a\u00e7\u00e3o": 1}"#,
        r#"["linha-um", "a\u00e7\u00e3o-02"]"#,
    ] {
        assert_eq!(texto(&m.mascarar_tudo(limpa.as_bytes())), limpa);
    }
}

/// V2.56 (D54): `printf %q` (bash real) da senha do gerador; `${X@Q}`, `set` e `set -x`
/// com `'`; o `json_encode` do PHP (`\/` + `\uXXXX`); e, num extra de várias linhas, as
/// linhas `chave : valor`, `chave:\tvalor`, `"k":"v"` colado e chave JSON com espaço e
/// com `/`, inteiras e só o valor, mais o `cut -f1` de uma linha com tab. Tudo vira
/// `••••` e nenhum trecho de 6+ sobra. Os casos de RA3 só são registrados.
#[test]
fn v2_56_formas_novas_e_parser_de_linha() {
    // `printf %q`.
    let senha = "Ab3$k&<9>Zq!";
    let m = mascara(&generico(&[senha]));
    let mut saidas: Vec<(&str, Vec<u8>)> =
        vec![("printf %q à mão", br"Ab3\$k\&\<9\>Zq\!".to_vec())];
    if let Some(o) = bash("printf '%q\\n' \"$NOTA\"", &[("NOTA", senha)]) {
        saidas.push(("printf %q (bash)", o));
    }
    for (caso, saida) in &saidas {
        sem_trecho_de_6(caso, &m, saida, senha);
        assert_eq!(texto(&m.mascarar_tudo(saida)).trim_end(), M, "{caso}");
    }

    // Aspas simples do shell.
    let aspa = "falso-it's-01x";
    let m = mascara(&generico(&[aspa]));
    let mut saidas: Vec<(&str, Vec<u8>)> = vec![("@Q à mão", br"'falso-it'\''s-01x'".to_vec())];
    let env = [("NOTA", aspa)];
    for (caso, script) in [
        ("${NOTA@Q}", "echo \"${NOTA@Q}\""),
        ("set", "set | grep ^NOTA="),
        ("set -x stderr", "exec 2>&1; set -x; : \"$NOTA\""),
    ] {
        if let Some(o) = bash(script, &env) {
            saidas.push((caso, o));
        }
    }
    for (caso, saida) in &saidas {
        sem_trecho_de_6(caso, &m, saida, aspa);
    }

    // `json_encode` do PHP.
    let php = "falso/ação/01";
    let m = mascara(&generico(&[php]));
    let mut saidas: Vec<(&str, Vec<u8>)> =
        vec![("PHP à mão", br#""falso\/a\u00e7\u00e3o\/01""#.to_vec())];
    if let Some(o) = python(
        "import os,json;print(json.dumps(os.environ['NOTA']).replace('/','\\\\/'))",
        &[("NOTA", php)],
    ) {
        saidas.push(("PHP simulado (python)", o));
    }
    for (caso, saida) in &saidas {
        assert_eq!(
            texto(&m.mascarar_tudo(saida)).trim_end(),
            format!("\"{M}\""),
            "{caso}"
        );
        sem_trecho_de_6(caso, &m, saida, "falso");
    }

    // Parser de linha e campos de tab.
    let linhas = [
        ("senha2 : falso-yaml-esp-02", "falso-yaml-esp-02"),
        ("senha:\tfalso-yaml-tab-01", "falso-yaml-tab-01"),
        ("\"k\":\"falso-json-colado-03\"", "falso-json-colado-03"),
        (
            "  \"private key\": \"falso-valor-com-espaco-na-chave-01\",",
            "falso-valor-com-espaco-na-chave-01",
        ),
        (
            "  \"x/y\": \"falso-valor-barra-chave-02\"",
            "falso-valor-barra-chave-02",
        ),
        ("falso-um-linha\tcom-tab", "falso-um-linha"),
    ];
    let nota: String = linhas
        .iter()
        .map(|(l, _)| *l)
        .collect::<Vec<_>>()
        .join("\n");
    let m = mascara(&generico(&[&nota]));
    for (linha, valor) in linhas {
        assert_eq!(
            texto(&m.mascarar_tudo(format!("{linha}\n").as_bytes())),
            format!("{M}\n")
        );
        assert_eq!(
            texto(&m.mascarar_tudo(format!("{valor}\n").as_bytes())),
            format!("{M}\n")
        );
        sem_trecho_de_6("linha inteira", &m, linha.as_bytes(), valor);
    }
    if let Some(o) = bash("sed -n 6p <<<\"$NOTA\" | cut -f1", &[("NOTA", &nota)]) {
        assert_eq!(o, b"falso-um-linha\n");
        assert_eq!(texto(&m.mascarar_tudo(&o)), format!("{M}\n"));
    }
    if let Some(o) = bash("sed -n 6p <<<\"$NOTA\" | cut -f2", &[("NOTA", &nota)]) {
        assert_eq!(texto(&m.mascarar_tudo(&o)), format!("{M}\n"));
    }

    // RA3: registrados, sem exigir máscara (nenhum valor no registro).
    let ctl = "falso-ctl\u{1b}-01x";
    let m = mascara(&generico(&[ctl]));
    if let Some(o) = bash("printf '%q\\n' \"$NOTA\"", &[("NOTA", ctl)]) {
        eprintln!(
            "RA3 printf %q com controle: mascarou = {}",
            m.mascarar_tudo(&o) != o
        );
    }
    let m = mascara(&generico(&[&nota]));
    if let Some(o) = bash("sed -n 6p <<<\"$NOTA\" | expand", &[("NOTA", &nota)]) {
        eprintln!(
            "RA3 expand da linha com tab: sobra {} bytes",
            maior_trecho(&m.mascarar_tudo(&o), b"falso-um-linha\tcom-tab")
        );
    }
    if let Some(o) = bash("sed -n 1p <<<\"$NOTA\" | base64 -w0", &[("NOTA", &nota)]) {
        eprintln!(
            "RA3 base64 de uma linha: mascarou = {}",
            m.mascarar_tudo(&o) != o
        );
    }
}

/// D54: o `printf %q` da máscara é igual ao do bash real para todo caractere ASCII
/// visível e o espaço, no começo, no meio, no fim e depois de `=` e `:`; e as aspas
/// simples, iguais ao miolo do `${X@Q}`.
#[test]
fn v2_56_printf_q_igual_ao_bash() {
    let mut valores = Vec::new();
    for c in (0x20u8..=0x7e).map(char::from) {
        for v in [
            format!("a{c}b"),
            format!("{c}b"),
            format!("a{c}"),
            format!("a={c}b"),
            format!("a:{c}b"),
        ] {
            valores.push(v);
        }
    }
    // Um processo só: o bash imprime um por linha (nenhum valor tem `\n`).
    let junto = valores.join("\n");
    let Some(o) = bash(
        "while IFS= read -r v; do printf '%q\\n' \"$v\"; done <<<\"$VALORES\"",
        &[("VALORES", &junto)],
    ) else {
        return;
    };
    let do_bash: Vec<&[u8]> = o
        .strip_suffix(b"\n")
        .unwrap()
        .split(|b| *b == b'\n')
        .collect();
    assert_eq!(do_bash.len(), valores.len());
    for (v, esperado) in valores.iter().zip(do_bash) {
        let mut nosso = Zeroizing::new(Vec::new());
        printf_q(v.as_bytes(), &mut nosso);
        assert_eq!(nosso.as_slice(), esperado, "printf %q de {v:?}");
        assert_eq!(
            printf_q_muda(v.as_bytes()),
            esperado != v.as_bytes(),
            "{v:?}"
        );
    }
    let Some(o) = bash(
        "while IFS= read -r v; do echo \"${v@Q}\"; done <<<\"$VALORES\"",
        &[("VALORES", "falso-it's-01x\n'a''b'\nsem-aspa")],
    ) else {
        return;
    };
    let do_bash: Vec<&[u8]> = o
        .strip_suffix(b"\n")
        .unwrap()
        .split(|b| *b == b'\n')
        .collect();
    for (v, esperado) in ["falso-it's-01x", "'a''b'", "sem-aspa"].iter().zip(do_bash) {
        let mut nosso = Zeroizing::new(Vec::new());
        aspas_simples(v.as_bytes(), &mut nosso);
        let mut com_pontas = b"'".to_vec();
        com_pontas.extend_from_slice(&nosso);
        com_pontas.push(b'\'');
        assert_eq!(com_pontas, esperado, "@Q de {v:?}");
    }
}

/// D54: cada forma nova só existe quando muda o valor, e a linha solta não ganha
/// base64 nem miolos (o base64 de linha solta fica em RA3).
#[test]
fn d54_formas_novas_so_quando_mudam_e_linha_sem_base64() {
    let contar = |valor: &str| {
        let mut g = Gerador::novo();
        let mut n = 0usize;
        g.formas_de_texto(valor.as_bytes(), &mut |_| {
            n += 1;
            Ok(())
        })
        .unwrap();
        n
    };
    assert_eq!(contar("falso-simples-01"), 0);
    assert_eq!(contar("falso-it's-01"), 2, "aspas simples e %q");
    assert_eq!(contar("falso/simples/01"), 1, "só o JSON com \\/");
    assert_eq!(contar("falso-ação-01"), 1, "só o ensure_ascii");
    assert_eq!(contar("falso/ação/01"), 3, "\\/, ensure_ascii e PHP");
    assert_eq!(contar("falso simples"), 1, "só o %q");
    assert_eq!(contar("falso~a"), 0, "~ no meio não muda o %q");
    assert_eq!(contar("~falso-a"), 1, "~ no começo muda o %q");

    let linha = "falso-linha-solta-0123456789";
    let mut b64 = Zeroizing::new(Vec::new());
    base64(linha.as_bytes(), B64_PADRAO, false, &mut b64);
    let mut g = Gerador::novo();
    let mut padroes: Vec<Vec<u8>> = Vec::new();
    g.linhas(
        format!("{linha}\noutra-linha-falsa\n").as_bytes(),
        &mut |p| {
            padroes.push(p.to_vec());
            Ok(())
        },
    )
    .unwrap();
    assert!(padroes.iter().any(|p| p == linha.as_bytes()));
    assert!(
        !padroes
            .iter()
            .any(|p| p.windows(8).any(|w| b64.windows(8).any(|x| x == w))),
        "a linha solta ganhou base64"
    );
}

/// V2.57 (D54), com valores que as fixtures não têm: o oráculo (`dev/variations.py`,
/// rodado pelo python, sem chamar nada da máscara) gera os padrões de valores com `$`,
/// `&`, `<`, `'`, `/`, acento, tab e linhas de chave e valor, e cada padrão, sozinho,
/// sai `••••` pela máscara da entrada dele. A tabela de D54 é a mesma nas duas pontas.
#[test]
fn v2_57_oraculo_e_mascara_com_valores_de_d54() {
    let fonte = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dev/variations.py");
    if !std::path::Path::new(PYTHON).exists() {
        eprintln!("{PYTHON} não existe aqui: teste cruzado pulado");
        return;
    }
    let extras = [
        "falso-Ab3$k&<9>Zq!",
        "falso-it's-01x",
        "falso/ação/01",
        "falso com espaço ~ #01",
        "linha-um-ação-01\nlinha-dois-ação-02\nlinha-tres-Ab3$k&<9>Zq!",
        "senha2 : falso-yaml-esp-02\nsenha:\tfalso-yaml-tab-01\n\"k\":\"falso-json-colado-03\"\n  \"private key\": \"falso-valor-com-espaco-na-chave-01\",\n  \"x/y\": \"falso-valor-barra-chave-02\"\nfalso-um-linha\tcom-tab\nexport FALSO_X='falso-it''s-02'\n",
    ];
    let anexo_ = "FALSO_ENV=falso-anexo-01\nPORT=8080\nnome\tfalso-tsv-valor-01\tfalso/ação\r\n";
    let senha_banco = "falso-PG$&<x>'q/ç-01";
    let mut r = rascunho("teste-generico", RascunhoDados::Generico);
    r.extras = extras
        .iter()
        .enumerate()
        .map(|(i, v)| extra(&format!("S{i}"), v, true))
        .collect();
    r.anexos = vec![anexo("app.env", anexo_.as_bytes())];
    let generico = valida(r);
    let banco = valida(rascunho(
        "teste-banco",
        RascunhoDados::Banco {
            motor: t("postgres"),
            host: t("127.0.0.1"),
            porta: t("55432"),
            base: t("app_teste"),
            usuario: t("app"),
            senha: s(senha_banco),
            tunel: String::new(),
        },
    ));
    // Pedido ao oráculo pelo stdin: grupo, usuário (ou vazio), anexo (0/1), valor em hex.
    let mut pedido = String::new();
    for v in extras {
        pedido.push_str(&format!("g\t\t0\t{}\n", hex(v.as_bytes())));
    }
    pedido.push_str(&format!("g\t\t1\t{}\n", hex(anexo_.as_bytes())));
    pedido.push_str(&format!("b\tapp\t0\t{}\n", hex(senha_banco.as_bytes())));
    let script = "import importlib.util,sys\n\
        spec=importlib.util.spec_from_file_location('variacoes',sys.argv[1])\n\
        v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)\n\
        for linha in sys.stdin.read().splitlines():\n\
        \x20   g,u,a,h=linha.split('\\t')\n\
        \x20   for p in sorted(v.variacoes(bytes.fromhex(h),u or None,anexo=a=='1')):\n\
        \x20       print(g+'\\t'+p.hex())\n";
    let mut filho = std::process::Command::new(PYTHON)
        .args(["-I", "-B", "-c", script, fonte])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write as _;
        filho
            .stdin
            .take()
            .unwrap()
            .write_all(pedido.as_bytes())
            .unwrap();
    }
    let saida = filho.wait_with_output().unwrap();
    assert!(saida.status.success(), "o oráculo falhou");
    let m_g = mascara(&generico);
    let m_b = mascara(&banco);
    let mut n = 0usize;
    for linha in saida
        .stdout
        .split(|b| *b == b'\n')
        .filter(|l| !l.is_empty())
    {
        let (grupo, h) = linha.split_at(1);
        let padrao = de_hex(&h[1..]);
        let m = if grupo == b"g" { &m_g } else { &m_b };
        assert_eq!(
            m.mascarar_tudo(&padrao),
            M.as_bytes(),
            "padrão nº {n} do oráculo não saiu {M}"
        );
        n += 1;
    }
    assert!(n > 200, "o oráculo gerou poucos padrões ({n})");
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn de_hex(h: &[u8]) -> Vec<u8> {
    h.chunks(2)
        .map(|p| u8::from_str_radix(core::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}

// ---- Fase 22a: D58 (Go com `ensure_ascii`), D59 e D61 (repro `contorno19e.rs`) ----

/// Roda o bash com variáveis de bytes quaisquer (o anexo Latin-1 não é `&str`). `None`
/// sem bash nesta máquina.
fn bash_bytes(script: &str, env: &[(&str, &[u8])]) -> Option<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt as _;
    if !std::path::Path::new(BASH).exists() {
        eprintln!("{BASH} não existe aqui: caso do programa real pulado");
        return None;
    }
    let mut c = std::process::Command::new(BASH);
    c.args(["--norc", "--noprofile", "-c", script])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C.UTF-8");
    for (k, v) in env {
        c.env(k, std::ffi::OsStr::from_bytes(v));
    }
    let saida = c.output().expect("rodar o bash de teste");
    assert!(saida.status.success(), "bash falhou");
    Some(saida.stdout)
}

/// O oráculo (`dev/variations.py`, pelo python, sem a máscara) gera os padrões de cada
/// pedido (máscara, usuário, anexo, valor) e cada padrão, sozinho, tem de sair `••••`
/// pela máscara do pedido. Devolve quantos padrões conferiu (0 sem python).
fn oraculo_cobre(pedidos: &[(&Mascara, Option<&str>, bool, &[u8])]) -> usize {
    let fonte = concat!(env!("CARGO_MANIFEST_DIR"), "/../../dev/variations.py");
    if !std::path::Path::new(PYTHON).exists() {
        eprintln!("{PYTHON} não existe aqui: teste cruzado pulado");
        return 0;
    }
    let mut pedido = String::new();
    for (i, (_, u, a, v)) in pedidos.iter().enumerate() {
        pedido.push_str(&format!(
            "{i}\t{}\t{}\t{}\n",
            u.unwrap_or(""),
            u8::from(*a),
            hex(v)
        ));
    }
    let script = "import importlib.util,sys\n\
        spec=importlib.util.spec_from_file_location('variacoes',sys.argv[1])\n\
        v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)\n\
        for linha in sys.stdin.read().splitlines():\n\
        \x20   g,u,a,h=linha.split('\\t')\n\
        \x20   for p in sorted(v.variacoes(bytes.fromhex(h),u or None,anexo=a=='1')):\n\
        \x20       print(g+'\\t'+p.hex())\n";
    let mut filho = std::process::Command::new(PYTHON)
        .args(["-I", "-B", "-c", script, fonte])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write as _;
        filho
            .stdin
            .take()
            .unwrap()
            .write_all(pedido.as_bytes())
            .unwrap();
    }
    let saida = filho.wait_with_output().unwrap();
    assert!(saida.status.success(), "o oráculo falhou");
    let mut n = 0usize;
    for linha in saida
        .stdout
        .split(|b| *b == b'\n')
        .filter(|l| !l.is_empty())
    {
        let tab = linha.iter().position(|b| *b == b'\t').unwrap();
        let i: usize = core::str::from_utf8(&linha[..tab])
            .unwrap()
            .parse()
            .unwrap();
        let padrao = de_hex(&linha[tab + 1..]);
        assert_eq!(
            pedidos[i].0.mascarar_tudo(&padrao),
            M.as_bytes(),
            "padrão nº {n} do oráculo (pedido {i}) não saiu {M}"
        );
        n += 1;
    }
    n
}

/// Os padrões que o `Gerador` monta para um segredo de várias linhas (ou anexo).
fn padroes_das_linhas(c: &[u8]) -> Vec<Vec<u8>> {
    let mut g = Gerador::novo();
    let mut padroes = Vec::new();
    g.linhas(c, &mut |p| {
        padroes.push(p.to_vec());
        Ok(())
    })
    .unwrap();
    padroes
}

const TOKEN_D59: &str = "falso-Ab3$k&<9>Zq!'/ç-01";
const ASCII_D59: &str = "falso-Ab3$k&<9>Zq!'";

/// `delta-api-simbolos` (D59), montada como na tabela do contrato.
fn delta_api_simbolos() -> Entrada {
    let mut r = rascunho(
        "delta-api-simbolos",
        RascunhoDados::Api {
            url: t("https://api.teste.invalid"),
            token: s(TOKEN_D59),
            variavel: t("SIMBOLOS_TOKEN"),
        },
    );
    r.grupo = t("delta");
    r.extras = vec![extra("SEGREDO_ASCII", ASCII_D59, true)];
    valida(r)
}

/// Python que imprime o valor de `V` como JSON "html-safe" com `ensure_ascii` (o
/// `json.dumps` padrão mais `&<>` em `\u`, como o `htmlsafe_json_dumps` do Jinja e o
/// `json_script` do Django). `{}` é a expressão do valor.
fn py_html_safe(expr: &str) -> String {
    format!(
        "import os,json;\
         h=lambda s:s.replace('&','\\\\u0026').replace('<','\\\\u003c').replace('>','\\\\u003e');\
         print(h(json.dumps({expr})))"
    )
}

/// V3.29 (D58, D59): o Go com `ensure_ascii` do token de `delta-api-simbolos` (a forma
/// do contrato e a do python real), as formas do shell e do PHP; cada padrão do oráculo
/// da entrada sai `••••`; o custo cabe no orçamento de D29.
#[test]
fn v3_29_delta_api_simbolos_e_go_com_ensure_ascii() {
    let e = delta_api_simbolos();
    let m = mascara(&e);
    let custo = custo_da_entrada(&e).unwrap();
    assert!(custo <= MASK_BUDGET, "D29: {custo}");
    let contrato = br"falso-Ab3$k\u0026\u003c9\u003eZq!'/\u00e7-01".to_vec();
    let mut saidas: Vec<(&str, Vec<u8>)> = vec![
        ("Go + ensure_ascii (contrato)", contrato.clone()),
        ("PHP à mão", br"falso-Ab3$k&<9>Zq!'\/\u00e7-01".to_vec()),
    ];
    for (caso, script) in [
        (
            "html-safe + ensure_ascii (python)",
            py_html_safe("os.environ['V']"),
        ),
        (
            "ensure_ascii (python)",
            t("import os,json;print(json.dumps(os.environ['V']))"),
        ),
    ] {
        if let Some(o) = python(&script, &[("V", TOKEN_D59)]) {
            saidas.push((caso, o));
        }
    }
    for (caso, script) in [
        ("${V@Q}", "echo \"${V@Q}\""),
        ("declare -p", "declare -p V"),
    ] {
        if let Some(o) = bash(script, &[("V", TOKEN_D59)]) {
            saidas.push((caso, o));
        }
    }
    for (caso, saida) in &saidas {
        sem_trecho_de_6(caso, &m, saida, TOKEN_D59);
    }
    if let Some(o) = bash("printf '%q\\n' \"$V\"", &[("V", ASCII_D59)]) {
        sem_trecho_de_6("printf %q do SEGREDO_ASCII", &m, &o, ASCII_D59);
    }
    // A forma do contrato inteira vira uma marca só.
    assert_eq!(texto(&m.mascarar_tudo(&contrato)), M);
    // O oráculo da entrada (token e extra), sozinho padrão por padrão.
    let n = oraculo_cobre(&[
        (&m, None, false, TOKEN_D59.as_bytes()),
        (&m, None, false, ASCII_D59.as_bytes()),
    ]);
    assert!(n == 0 || n > 20, "o oráculo gerou poucos padrões ({n})");
}

/// V3.29 (D58): o Go com `ensure_ascii` também por linha de um segredo de várias linhas
/// e no valor de linha; e uma linha binária de anexo continua mascarada pelo literal,
/// sem formas de texto (D54).
#[test]
fn v3_29_go_com_ensure_ascii_por_linha_e_linha_binaria() {
    let linhas = [
        "linha-um-ação-01",
        "linha-dois-Ab3$k&<9>Zq!-ação-02",
        "TOKEN=falso-tok&<ç>-03",
    ];
    let nota = linhas.join("\n");
    let e = generico(&[&nota]);
    let m = mascara(&e);
    let mut saidas: Vec<(&str, Vec<u8>)> = vec![(
        "linha 2 à mão",
        br#"["linha-dois-Ab3$k\u0026\u003c9\u003eZq!-a\u00e7\u00e3o-02"]"#.to_vec(),
    )];
    if let Some(o) = python(
        &py_html_safe("os.environ['V'].splitlines()"),
        &[("V", &nota)],
    ) {
        saidas.push(("html-safe + ensure_ascii das linhas (python)", o));
    }
    if let Some(o) = python(
        &py_html_safe("os.environ['V']"),
        &[("V", "falso-tok&<ç>-03")],
    ) {
        saidas.push(("valor de linha (python)", o));
    }
    for (caso, saida) in &saidas {
        for alvo in linhas.iter().chain(["falso-tok&<ç>-03"].iter()) {
            sem_trecho_de_6(caso, &m, saida, alvo);
        }
    }
    // Só quando muda: sem `&<>` ou sem caractere fora do ASCII, nenhuma forma a mais.
    let contar = |valor: &str| {
        let mut g = Gerador::novo();
        let mut n = 0usize;
        g.formas_de_texto(valor.as_bytes(), &mut |_| {
            n += 1;
            Ok(())
        })
        .unwrap();
        n
    };
    assert_eq!(contar("falso-ação-01"), 1, "só o ensure_ascii");
    assert_eq!(contar("falso-a&b-01"), 2, "Go e %q");
    assert_eq!(contar("falso-a&ç-01"), 3, "Go, ensure_ascii e D58");
    assert_eq!(
        contar("falso\u{2028}ç-01"),
        2,
        "U+2028 sem &<>: Go e ensure_ascii"
    );
    // Linha binária: o literal (com e sem `\r`, aparada) e nada de forma de texto.
    let binaria: &[u8] = b"falso-bin-\xff&<x>-linha-01 \r\nFALSO_ENV=falso-anexo-01\n";
    let padroes = padroes_das_linhas(binaria);
    assert!(padroes.iter().any(|p| p == b"falso-bin-\xff&<x>-linha-01 "));
    assert!(padroes.iter().any(|p| p == b"falso-bin-\xff&<x>-linha-01"));
    assert!(
        !padroes
            .iter()
            .any(|p| p.windows(6).any(|w| w == br"\u0026")),
        "linha binária ganhou forma de texto"
    );
    let mut r = rascunho("teste-binario", RascunhoDados::Generico);
    r.extras = vec![extra("OBS", "nada", false)];
    r.anexos = vec![anexo("cfg.bin", binaria)];
    let mb = mascara(&valida(r));
    assert_eq!(
        mb.mascarar_tudo(b"falso-bin-\xff&<x>-linha-01\n"),
        format!("{M}\n").as_bytes()
    );
}

/// V3.31 (D61): JSON numa linha. O anexo `client.json` minificado, o extra de uma linha
/// só `{"user":…,"token":…}`, o objeto aninhado na linha e duas chaves na linha de um
/// extra de várias linhas, e um valor com `\u00e7` e `\/` impresso decodificado, cada um
/// pela saída real do `json.loads` do python; nenhum trecho de 6+ sobra, e o oráculo
/// gera os mesmos padrões.
#[test]
fn v3_31_json_numa_linha() {
    let client: &[u8] =
        b"{\"installed\":{\"client_id\":\"x.apps\",\"client_secret\":\"falso-GOCSPX-minificado-01\"}}\n";
    let um = r#"{"user":"x","token":"falso-json-uma-linha-01"}"#;
    let varias = "{\n  \"db\": {\"senha\": \"falso-json-aninhado-02\"},\n  \"a\": \"x\", \"b\": \"falso-json-duas-chaves-03\"\n}";
    let escapado = r#"{"k": "falso-a\u00e7\/b-json-04"}"#;
    let mut r = rascunho("teste-json", RascunhoDados::Generico);
    r.extras = vec![
        extra("UM", um, true),
        extra("VARIAS", varias, true),
        extra("ESC", escapado, true),
    ];
    r.anexos = vec![anexo("client.json", client)];
    let e = valida(r);
    let m = mascara(&e);
    let custo = custo_da_entrada(&e).unwrap();
    assert!(custo <= MASK_BUDGET, "D29: {custo}");
    let cliente = core::str::from_utf8(client).unwrap();
    let casos: [(&str, &str, &str, &str); 5] = [
        (
            "anexo minificado, .installed.client_secret",
            cliente,
            "print(json.loads(os.environ['V'])['installed']['client_secret'])",
            "falso-GOCSPX-minificado-01",
        ),
        (
            "extra de uma linha, .token",
            um,
            "print(json.loads(os.environ['V'])['token'])",
            "falso-json-uma-linha-01",
        ),
        (
            "objeto aninhado na linha",
            varias,
            "print(json.loads(os.environ['V'])['db']['senha'])",
            "falso-json-aninhado-02",
        ),
        (
            "duas chaves na linha",
            varias,
            "print(json.loads(os.environ['V'])['b'])",
            "falso-json-duas-chaves-03",
        ),
        (
            "\\u00e7 e \\/ decodificados",
            escapado,
            "print(json.loads(os.environ['V'])['k'])",
            "falso-aç/b-json-04",
        ),
    ];
    for (caso, v, script, alvo) in casos {
        let mut saidas = vec![format!("{alvo}\n").into_bytes()];
        if let Some(o) = python(&format!("import os,json;{script}"), &[("V", v)]) {
            assert_eq!(
                o,
                format!("{alvo}\n").into_bytes(),
                "{caso}: o python leu outro valor"
            );
            saidas.push(o);
        }
        for saida in &saidas {
            sem_trecho_de_6(caso, &m, saida, alvo);
            assert_eq!(texto(&m.mascarar_tudo(saida)), format!("{M}\n"), "{caso}");
        }
    }
    // Escape inválido descarta o par; elemento de array sem chave fica em RA3 (só
    // registrado).
    let padroes =
        padroes_das_linhas(br#"  "k": "falso-invalido-\x-01", "s": "falso-sozinho-\ud800-02","#);
    assert!(!padroes.iter().any(|p| p == b"falso-invalido-x-01"));
    assert!(!padroes.iter().any(|p| p.starts_with(b"falso-sozinho-")));
    let arr = generico(&[r#"["falso-elemento-array-01"]"#]);
    let fora = mascara(&arr).mascarar_tudo(b"falso-elemento-array-01\n");
    eprintln!(
        "RA3 (array JSON sem chave): {}",
        if maior_trecho(&fora, b"falso-elemento-array-01") >= 6 {
            "não mascarado"
        } else {
            "mascarado"
        }
    );
    // O oráculo dá os mesmos padrões.
    let n = oraculo_cobre(&[
        (&m, None, true, client),
        (&m, None, false, um.as_bytes()),
        (&m, None, false, varias.as_bytes()),
        (&m, None, false, escapado.as_bytes()),
    ]);
    assert!(n == 0 || n > 40, "o oráculo gerou poucos padrões ({n})");
}

/// V3.32 (D61): YAML e dotenv (comentário no fim, chave com espaço e com `/`, `''` das
/// aspas simples) como o `yq` e o `source` do bash imprimem, e os campos de tab de uma
/// linha Latin-1 de anexo pelo `cut -f2`/`-f3` real; o custo das duas entradas cabe no
/// orçamento de D29, e o oráculo gera os mesmos padrões.
#[test]
fn v3_32_yaml_dotenv_e_tab_de_linha_binaria() {
    let nota = "url: https://x.example\nsenha: falso-yaml-coment-01 # comentario\nprivate key: falso-yaml-chave-espaco-02\naws/secret: falso-yaml-chave-barra-03\n  aninhado:\n    senha: falso-yaml-indent-04\nSENHA_DB=\"falso-env-coment-05\" # dotenv\ntok: 'falso-it''s-yaml-06'";
    let e = generico(&[nota]);
    let m = mascara(&e);
    assert!(custo_da_entrada(&e).unwrap() <= MASK_BUDGET);
    for alvo in [
        "falso-yaml-coment-01",
        "falso-yaml-chave-espaco-02",
        "falso-yaml-chave-barra-03",
        "falso-yaml-indent-04",
        "falso-it's-yaml-06",
    ] {
        // Como o `yq` imprime o valor (decodificado, sem aspas nem comentário).
        let saida = format!("{alvo}\n");
        sem_trecho_de_6(alvo, &m, saida.as_bytes(), alvo);
        assert_eq!(texto(&m.mascarar_tudo(saida.as_bytes())), format!("{M}\n"));
    }
    if let Some(o) = bash(
        "eval \"$(sed -n 7p <<<\"$NOTA\")\"; echo \"$SENHA_DB\"",
        &[("NOTA", nota)],
    ) {
        assert_eq!(o, b"falso-env-coment-05\n", "o source leu outro valor");
        sem_trecho_de_6("dotenv pelo source", &m, &o, "falso-env-coment-05");
    }
    // Anexo Latin-1 (não é UTF-8) com campos de tab.
    let mut an: Vec<u8> = b"# config\n".to_vec();
    an.extend_from_slice(b"SENHA=falso-latin\xe7-01\tTOKEN=falso-campo-tab-02\n");
    an.extend_from_slice(b"nome\xe7\tfalso-campo-tsv-03\tfalso-campo-tsv-04\n");
    let mut r = rascunho("teste-latin1", RascunhoDados::Generico);
    r.extras = vec![extra("OBS", "nada", false)];
    r.anexos = vec![anexo("cfg.ini", &an)];
    let ea = valida(r);
    let ma = mascara(&ea);
    assert!(custo_da_entrada(&ea).unwrap() <= MASK_BUDGET);
    for (caso, script, alvo) in [
        (
            "cut -f2 (KEY=v)",
            "printf '%s' \"$A\" | sed -n 2p | cut -f2",
            "TOKEN=falso-campo-tab-02",
        ),
        (
            "cut -f2 (TSV)",
            "printf '%s' \"$A\" | sed -n 3p | cut -f2",
            "falso-campo-tsv-03",
        ),
        (
            "cut -f3 (TSV)",
            "printf '%s' \"$A\" | sed -n 3p | cut -f3",
            "falso-campo-tsv-04",
        ),
    ] {
        let mut saidas = vec![format!("{alvo}\n").into_bytes()];
        if let Some(o) = bash_bytes(script, &[("A", &an)]) {
            assert_eq!(
                o,
                format!("{alvo}\n").into_bytes(),
                "{caso}: o cut deu outro campo"
            );
            saidas.push(o);
        }
        for saida in &saidas {
            assert_eq!(texto(&ma.mascarar_tudo(saida)), format!("{M}\n"), "{caso}");
        }
    }
    // Só pelo literal: o campo binário não ganha base64 nem forma de texto.
    let padroes = padroes_das_linhas(&an);
    let mut b64 = Zeroizing::new(Vec::new());
    base64(b"falso-campo-tsv-04", B64_PADRAO, true, &mut b64);
    assert!(
        !padroes.iter().any(|p| p[..] == b64[..]),
        "campo binário com base64"
    );
    // `iconv` e `repr()` ficam em RA3: só registrados.
    if let Some(o) = bash_bytes(
        "printf '%s' \"$A\" | sed -n 3p | iconv -f latin1 -t utf-8",
        &[("A", &an)],
    ) {
        let fora = ma.mascarar_tudo(&o);
        eprintln!(
            "RA3 (iconv da linha Latin-1): {}",
            if maior_trecho(&fora, b"falso-campo-tsv-03") >= 6 {
                "não mascarado"
            } else {
                "mascarado"
            }
        );
    }
    let n = oraculo_cobre(&[(&m, None, false, nota.as_bytes()), (&ma, None, true, &an)]);
    assert!(n == 0 || n > 40, "o oráculo gerou poucos padrões ({n})");
}

/// D61: cada candidato a mais só entra quando difere do que a linha já gera (o
/// orçamento de D29 não paga duas vezes o mesmo valor).
#[test]
fn d61_candidatos_so_quando_mudam() {
    let contar = |linha: &[u8], alvo: &[u8]| {
        padroes_das_linhas(linha)
            .iter()
            .filter(|p| p[..] == *alvo)
            .count()
    };
    assert_eq!(
        contar(b"senha: falso-yaml-simples-01", b"falso-yaml-simples-01"),
        1
    );
    assert_eq!(
        contar(
            br#""k": "falso-json-simples-01","#,
            b"falso-json-simples-01"
        ),
        1
    );
    assert_eq!(
        contar(b"SENHA=falso-env-simples-01", b"falso-env-simples-01"),
        1
    );
    // A chave composta que dá o mesmo valor do parser antigo não repete o valor.
    assert_eq!(
        contar(b"chave_x : falso-yaml-esp-01", b"falso-yaml-esp-01"),
        1
    );
    // URL, comentário e flow do YAML não viram chave composta.
    assert!(valor_de_chave_composta(b"https://host.invalid:8080/x").is_none());
    assert!(valor_de_chave_composta(b"# nota: x").is_none());
    assert!(valor_de_chave_composta(b"{k: falso-flow-01}").is_none());
    assert_eq!(
        valor_de_chave_composta(b"  - private key: falso-x-01,"),
        Some(&b"falso-x-01"[..])
    );
}

// ---- D34 revisto (auditoria do M4): sem oráculo de prefixo ----
//
// O ataque da auditoria (`oraculo_pty.py`): o comando imprime, em pedaços separados por
// pausa, `\nQ` + prefixo + candidato, e olha se aquele pedaço saiu diferente do que
// entrou (marca no lugar, ou segurado até a pausa/até mais saída). Com a máscara que
// segurava o prefixo, o candidato certo era o único diferente, e a busca em
// profundidade recuperava a senha. Aqui o mesmo ataque roda contra o fluxo, com a
// máscara de todas as famílias de padrão (literal, percent, base64 e miolos, Basic,
// URL, formas de texto, linhas e valor de linha de anexo, JSON e YAML na linha, campo
// de tab, chave privada por linha), e tem de não achar nada.

const SENHA_SSH: &str = "falso-SSH-senha-01";

/// A máscara com uma entrada de cada família de padrão.
fn mascara_de_todas_as_familias() -> Mascara {
    let pg = pg_dev();
    let g = entrega(&pg);
    let gama = gama_generico();
    let mut r = rascunho("anexo-familias", RascunhoDados::Generico);
    r.extras = vec![
        extra("S_ASPAS", "falso\"a&b<c>'d$e", true),
        extra("S_SSH", SENHA_SSH, true),
    ];
    r.anexos = vec![anexo(
        "conf",
        b"{\"api_token\": \"falso-json-tok-77\"}\nchave yaml: falso-yaml-val-88\ncol1\tfalso-tab-campo-99\nexport DB_PASS=falso-dotenv-55 # nota\n",
    )];
    let familias = valida(r);
    let chave = servidor_com_chave(&chave_de_varias_linhas());
    ConstrutorMascara::new()
        .entrada(&pg)
        .entrega(&g)
        .entrada(&gama)
        .entrada(&familias)
        .entrada(&chave)
        .construir()
        .unwrap()
}

/// O oráculo da auditoria: o pedaço `\nQ` + prefixo + candidato saiu diferente do que
/// entrou, ou algo ficou segurado (que, no daemon, viraria atraso ou marca depois da
/// pausa)?
fn oraculo_responde(f: &mut Fluxo<'_>, prefixo: &[u8], c: u8) -> bool {
    let mut bloco = b"\nQ".to_vec();
    bloco.extend_from_slice(prefixo);
    bloco.push(c);
    let mut out = Vec::new();
    f.mascarar(&bloco, &mut out);
    let segurou = f.emitido != f.lido || !f.pendente.is_empty();
    segurou || out != bloco
}

#[test]
fn d34_ataque_oraculo_de_prefixo_da_auditoria_nao_recupera_nada() {
    let m = mascara_de_todas_as_familias();
    let alfabeto: Vec<u8> = (0x20..0x7f).collect();
    // Busca em profundidade como a do `oraculo_pty.py`, no mesmo fluxo (a sessão
    // interativa é um fluxo só), partindo do vazio.
    let mut f = m.fluxo();
    let mut pilha: Vec<Vec<u8>> = vec![Vec::new()];
    let mut consultas = 0usize;
    let mut aceitos = Vec::new();
    while let Some(p) = pilha.pop() {
        // Teto: contra a máscara que segurava, a busca seguia por milhares de consultas.
        if p.len() >= 40 || consultas > 20_000 {
            break;
        }
        for &c in &alfabeto {
            consultas += 1;
            if oraculo_responde(&mut f, &p, c) {
                let mut q = p.clone();
                q.push(c);
                aceitos.push(q.clone());
                pilha.push(q);
            }
        }
    }
    assert!(consultas >= alfabeto.len());
    assert!(
        aceitos.is_empty(),
        "o oráculo distinguiu {} prefixo(s)",
        aceitos.len()
    );
}

#[test]
fn d34_ataque_prefixo_certo_e_errado_dao_a_mesma_resposta() {
    // O atacante que já sabe k caracteres (de qualquer variação) e testa o próximo: a
    // resposta é a mesma para o candidato certo e para todos os errados, até o último
    // caractere do padrão (aí ele já tinha o segredo inteiro, menos um caractere: é o
    // limite de qualquer máscara, D34). Vale para cada forma do segredo de uma linha.
    let m = mascara_de_todas_as_familias();
    let alfabeto: Vec<u8> = (0x20..0x7f).collect();
    let go = |c: u8| format!("\\u{:04x}", c);
    // Folga no fim: no base64, o miolo do segredo (padrão próprio) termina até um grupo
    // de 4 e o `=` antes do fim; nele o atacante já escreveu o segredo menos 1 ou 2
    // bytes, que é o mesmo limite.
    let base64 = [
        b64(SENHA_PG.as_bytes(), false, true),
        b64(SENHA_PG.as_bytes(), true, false),
        b64(format!("app:{SENHA_PG}").as_bytes(), false, true),
    ];
    let mut formas = vec![
        t(SENHA_PG),
        pct(SENHA_PG, false, false),
        pct(SENHA_PG, true, true),
        t(SENHA_SSH),
        // JSON do Go (`&`, `<`, `>` em `\u00XX`) e aspas duplas do shell.
        format!(r#"falso\"a{}b{}c{}'d$e"#, go(b'&'), go(b'<'), go(b'>')),
        t(r#"falso\"a&b<c>'d\$e"#),
        t("falso-json-tok-77"),
        t("falso-yaml-val-88"),
        t("falso-tab-campo-99"),
        t("falso-dotenv-55"),
        t("falso-licenca-01"),
    ];
    formas.dedup();
    let formas = formas
        .into_iter()
        .map(|f| (f, 1))
        .chain(base64.into_iter().map(|f| (f, 7)));
    for (forma, folga) in formas {
        let v = forma.as_bytes();
        for k in 0..v.len().saturating_sub(folga) {
            // O caractere certo (que não termina a forma) não muda nada.
            let mut f = m.fluxo();
            assert!(
                !oraculo_responde(&mut f, &v[..k], v[k]),
                "{forma}: o prefixo de {} bytes mudou a saída ou foi segurado",
                k + 1
            );
            // Um errado só muda a saída quando ele mesmo fecha o segredo inteiro por
            // outra forma (o `#` cru depois de `%40` no percent): perto do fim, com o
            // segredo todo já escrito.
            for &c in alfabeto.iter().filter(|&&c| c != v[k]) {
                let mut g = m.fluxo();
                assert!(
                    !oraculo_responde(&mut g, &v[..k], c) || k + 3 >= v.len(),
                    "{forma}: o candidato {:?} no caractere {k} mudou a saída",
                    char::from(c)
                );
            }
        }
    }
}

#[test]
fn d34_ataque_tempo_nada_segurado_ate_o_resto_chegar() {
    // `tempo.py` da auditoria (sem pty): `A`, pausa, candidato, pausa, `Z`. Com a
    // máscara que segurava, o candidato que era começo da senha só chegava junto com o
    // `Z`. Aqui cada pedaço sai inteiro na hora em que é lido.
    let m = mascara_de_todas_as_familias();
    for cand in [
        "qqq",
        "fal",
        "fax",
        "falso-S",
        "falso-X",
        "falso-PG:s3nha/01@",
    ] {
        let mut f = m.fluxo();
        for pedaco in [&b"A"[..], cand.as_bytes(), b"Z\n"] {
            let mut out = Vec::new();
            f.mascarar(pedaco, &mut out);
            nada_segurado(&f);
            assert_eq!(out, pedaco, "{cand}: o pedaço não saiu na hora");
        }
    }
}

#[test]
fn d34_pty_prefixo_parado_sai_em_claro_e_o_resto_vira_marca() {
    // O prompt que termina num começo de senha sai como veio (nada de marca no lugar
    // dele, que era o oráculo); se o resto chega depois, o pedaço que completa a senha
    // sai marcado; se diverge, sai normal.
    let m = mascara(&generico(&[SENHA_SSH]));
    let mut f = m.fluxo();
    let mut out = Vec::new();
    f.mascarar(b"senha? fals", &mut out);
    nada_segurado(&f);
    assert_eq!(texto(&out), "senha? fals");
    f.mascarar(b"o-SSH-senha-01 fim", &mut out);
    f.terminar(&mut out);
    assert_eq!(texto(&out), format!("senha? fals{M} fim"));

    let mut f = m.fluxo();
    let mut out = Vec::new();
    f.mascarar(b"x fals", &mut out);
    f.mascarar(b"e normal", &mut out);
    f.terminar(&mut out);
    assert_eq!(texto(&out), "x false normal");
}

#[test]
fn d34_escape_percent_pela_metade_no_fim_do_pedaco_sai_na_hora() {
    // O `%X` pela metade no fim de um pedaço era segurado até o próximo; agora sai.
    let m = mascara(&pg_dev());
    for fim in ["%", "%4", "falso-PG%3", "falso%2DP"] {
        let mut f = m.fluxo();
        let mut out = Vec::new();
        f.mascarar(fim.as_bytes(), &mut out);
        nada_segurado(&f);
        assert_eq!(texto(&out), fim);
    }
}
