use super::*;

fn opcoes(tamanho: usize, mascara: u8) -> OpcoesGerador {
    OpcoesGerador {
        tamanho,
        minusculas: mascara & 1 != 0,
        maiusculas: mascara & 2 != 0,
        digitos: mascara & 4 != 0,
        simbolos: mascara & 8 != 0,
    }
}

fn conjuntos_de(mascara: u8) -> Vec<&'static [u8]> {
    [MINUSCULAS, MAIUSCULAS, DIGITOS, SIMBOLOS]
        .into_iter()
        .enumerate()
        .filter(|(i, _)| mascara & (1 << i) != 0)
        .map(|(_, c)| c)
        .collect()
}

#[test]
fn conjuntos_sao_disjuntos_e_ascii_visivel() {
    let todos = [MINUSCULAS, MAIUSCULAS, DIGITOS, SIMBOLOS].concat();
    let mut ordenado = todos.clone();
    ordenado.sort_unstable();
    ordenado.dedup();
    assert_eq!(
        ordenado.len(),
        todos.len(),
        "conjuntos com caractere repetido"
    );
    assert!(todos.iter().all(|c| c.is_ascii_graphic()));
    for proibido in *b" \"'`\\" {
        assert!(!todos.contains(&proibido));
    }
    assert!(SIMBOLOS.iter().all(|c| c.is_ascii_punctuation()));
}

/// V2.26: para cada uma das 15 combinações de conjuntos, 10 000 senhas com os tamanhos
/// de 4 a 128 em rodízio (todos os tamanhos aparecem em toda combinação). Nenhuma tem
/// caractere fora dos conjuntos marcados e todas têm pelo menos um de cada.
#[test]
fn v2_26_dez_mil_senhas_por_combinacao() {
    for mascara in 1u8..16 {
        let conjuntos = conjuntos_de(mascara);
        let permitido = conjuntos.concat();
        for i in 0..10_000usize {
            let tamanho = TAMANHO_MIN + i % (TAMANHO_MAX - TAMANHO_MIN + 1);
            let senha = gerar(&opcoes(tamanho, mascara)).unwrap();
            let b = senha.expor();
            assert_eq!(b.len(), tamanho);
            assert!(
                b.iter().all(|c| permitido.contains(c)),
                "caractere fora dos conjuntos (máscara {mascara:04b})"
            );
            for c in &conjuntos {
                assert!(
                    b.iter().any(|x| c.contains(x)),
                    "faltou um conjunto marcado (máscara {mascara:04b}, tamanho {tamanho})"
                );
            }
            if mascara & 8 == 0 {
                assert!(
                    !b.iter().any(|c| SIMBOLOS.contains(c)),
                    "símbolo sem símbolos"
                );
                assert!(b.iter().all(|c| c.is_ascii_alphanumeric()));
            }
        }
    }
}

#[test]
fn v2_26_tamanho_e_conjuntos_invalidos() {
    for tamanho in [0, 1, 3, 129, 1_000, usize::MAX] {
        assert_eq!(
            gerar(&opcoes(tamanho, 15)).unwrap_err(),
            ErroGerador::TamanhoInvalido
        );
    }
    assert_eq!(
        gerar(&opcoes(24, 0)).unwrap_err(),
        ErroGerador::NenhumConjunto
    );
    let padrao = gerar(&OpcoesGerador::default()).unwrap();
    assert_eq!(padrao.len(), TAMANHO_PADRAO);
}

/// V2.26 ("a fonte é o RNG do sistema"): toda a aleatoriedade vem da fonte. Com a mesma
/// fonte determinística, a mesma senha; e a função pública usa `cripto::aleatorio`
/// (`getrandom`), conferido pelo próprio código de `gerar`.
#[test]
fn v2_26_toda_aleatoriedade_vem_da_fonte() {
    fn contador() -> impl FnMut(&mut [u8]) -> Result<(), FalhaCripto> {
        let mut n: u8 = 0;
        move |buf: &mut [u8]| {
            for b in buf {
                *b = n;
                n = n.wrapping_mul(13).wrapping_add(7);
            }
            Ok(())
        }
    }
    let o = opcoes(32, 15);
    let a = gerar_com(&o, &mut contador()).unwrap();
    let b = gerar_com(&o, &mut contador()).unwrap();
    assert_eq!(a.expor(), b.expor());
    // A fonte do sistema não repete.
    let x = gerar(&o).unwrap();
    let y = gerar(&o).unwrap();
    assert_ne!(x.expor(), y.expor());
    let fonte = include_str!("../gerador.rs");
    assert!(fonte.contains("gerar_com(opcoes, &mut cripto::aleatorio)"));
}

#[test]
fn ataque_rng_falha_nao_gera_senha_fraca() {
    // Sem RNG não existe senha "de reserva": o gerador falha.
    let mut quebrada = |_: &mut [u8]| Err(FalhaCripto);
    assert_eq!(
        gerar_com(&opcoes(24, 15), &mut quebrada).unwrap_err(),
        ErroGerador::AleatorioIndisponivel
    );
    // Falha no meio (depois de entregar bytes): também falha, sem senha parcial.
    let mut chamadas = 0;
    let mut cansa = |buf: &mut [u8]| {
        chamadas += 1;
        if chamadas > 1 {
            return Err(FalhaCripto);
        }
        buf.fill(1);
        Ok(())
    };
    assert_eq!(
        gerar_com(&opcoes(128, 15), &mut cansa).unwrap_err(),
        ErroGerador::AleatorioIndisponivel
    );
}

#[test]
fn ataque_rng_preso_nao_trava() {
    // Sempre o mesmo byte alto: toda amostra é rejeitada; o gerador para com erro.
    let mut alto = |buf: &mut [u8]| {
        buf.fill(0xff);
        Ok(())
    };
    assert_eq!(
        gerar_com(&opcoes(24, 15), &mut alto).unwrap_err(),
        ErroGerador::AleatorioIndisponivel
    );
    // Sempre zero: sai só "a", que nunca cumpre os 4 conjuntos; para com erro.
    let mut zero = |buf: &mut [u8]| {
        buf.fill(0);
        Ok(())
    };
    assert_eq!(
        gerar_com(&opcoes(24, 15), &mut zero).unwrap_err(),
        ErroGerador::AleatorioIndisponivel
    );
}

/// Sem viés de módulo: com o alfabeto de 90 caracteres (256 % 90 = 76), cada caractere
/// fica perto da média.
#[test]
fn ataque_vies_de_modulo() {
    let alfabeto = [MINUSCULAS, MAIUSCULAS, DIGITOS, SIMBOLOS].concat();
    let mut contagem = [0u64; 256];
    let mut total = 0u64;
    for _ in 0..4_000 {
        let s = gerar(&opcoes(128, 15)).unwrap();
        for &c in s.expor() {
            contagem[usize::from(c)] += 1;
            total += 1;
        }
    }
    let media = total as f64 / alfabeto.len() as f64;
    for &c in &alfabeto {
        let n = contagem[usize::from(c)] as f64;
        assert!(
            (n - media).abs() < media * 0.08,
            "caractere {:?}: {n} contra média {media}",
            c as char
        );
    }
    // Com `b % 90` ingênuo, os 14 últimos caracteres sairiam ~30 % abaixo da média: a
    // margem de 8 % pega. O desvio-padrão por caractere aqui é ~1,3 % da média.
}

#[test]
fn ataque_debug_da_senha_e_erros_sem_valor() {
    let s = gerar(&opcoes(64, 15)).unwrap();
    let texto = s.expor_texto().unwrap().to_owned();
    for saida in [format!("{s:?}"), format!("{s}"), format!("{s:#?}")] {
        assert!(!saida.contains(&texto));
        assert!(saida.contains("••••"));
    }
    for e in [
        ErroGerador::TamanhoInvalido,
        ErroGerador::NenhumConjunto,
        ErroGerador::AleatorioIndisponivel,
    ] {
        assert!(!e.to_string().is_empty());
    }
}

#[test]
fn v2_28_senha_gerada_zera() {
    fn zera<T: zeroize::ZeroizeOnDrop>(_: &T) {}
    let s = gerar(&OpcoesGerador::default()).unwrap();
    zera(&s);
}
