use super::*;

fn semente(s: &str) -> Segredo {
    Segredo::de_texto(s.to_owned())
}

/// Semente do RFC 6238 (apêndice B) para SHA1: ASCII "12345678901234567890" em base32.
const RFC_SHA1: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
/// Semente falsa do contrato (D15, `acme-wp-dev`/`acme-wp-prod`).
const SEMENTE_D15: &str = "JBSWY3DPEHPK3PXP";

#[test]
fn v2_25_vetores_rfc6238_sha1() {
    // RFC 6238, apêndice B, coluna SHA1 (8 dígitos).
    let t = Totp::com_digitos(&semente(RFC_SHA1), 8).unwrap();
    for (tempo, esperado) in [
        (59, "94287082"),
        (1_111_111_109, "07081804"),
        (1_111_111_111, "14050471"),
        (1_234_567_890, "89005924"),
        (2_000_000_000, "69279037"),
        (20_000_000_000, "65353130"),
    ] {
        assert_eq!(t.codigo_em(tempo).codigo(), esperado, "T = {tempo}");
    }
}

#[test]
fn v2_25_vetores_rfc4226_hotp() {
    // RFC 4226, apêndice D: HOTP com a mesma chave, contadores 0–9, 6 dígitos.
    let chave = b"12345678901234567890";
    let esperados = [
        755_224, 287_082, 359_152, 969_429, 338_314, 254_676, 287_922, 162_583, 399_871, 520_489,
    ];
    for (contador, esperado) in esperados.iter().enumerate() {
        assert_eq!(hotp(chave, contador as u64) % 1_000_000, *esperado);
    }
}

/// Códigos de `JBSWY3DPEHPK3PXP` conferidos fora do Ward (HMAC-SHA1 da biblioteca padrão
/// do Python, mesmo cálculo do `oathtool --totp -b`).
const D15_CONHECIDOS: [(u64, &str); 10] = [
    (0, "282760"),
    (29, "282760"),
    (30, "996554"),
    (59, "996554"),
    (1_111_111_109, "071271"),
    (1_234_567_890, "742275"),
    (1_762_560_000, "975040"),
    (2_000_000_000, "890699"),
    (4_102_444_800, "573258"),
    (20_000_000_000, "752434"),
];

#[test]
fn v2_25_semente_do_contrato_codigos_conhecidos() {
    let t = Totp::de_semente(&semente(SEMENTE_D15)).unwrap();
    for (tempo, esperado) in D15_CONHECIDOS {
        assert_eq!(t.codigo_em(tempo).codigo(), esperado, "T = {tempo}");
    }
}

/// Compara com o `oathtool` instalado, no mesmo período. Sem `oathtool` no PATH, o teste
/// avisa e passa, a não ser com `WARD_EXIGIR_OATHTOOL=1` (o validador liga). A semente é
/// a falsa de D15, então pode ir em argv só neste teste.
#[test]
fn v2_25_bate_com_oathtool() {
    let exigir = std::env::var_os("WARD_EXIGIR_OATHTOOL").is_some_and(|v| v == "1");
    let t = Totp::de_semente(&semente(SEMENTE_D15)).unwrap();
    let mut tempos: Vec<u64> = D15_CONHECIDOS.iter().map(|(t, _)| *t).collect();
    tempos.push(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    );
    for tempo in tempos {
        let saida = std::process::Command::new("oathtool")
            .args(["--totp", "-b", &format!("--now=@{tempo}"), SEMENTE_D15])
            .env("TZ", "UTC")
            .output();
        let saida = match saida {
            Ok(s) => s,
            Err(_) if !exigir => {
                eprintln!("oathtool ausente: comparação pulada (WARD_EXIGIR_OATHTOOL=1 exige)");
                return;
            }
            Err(e) => panic!("oathtool exigido e ausente: {e}"),
        };
        assert!(saida.status.success(), "oathtool falhou em T = {tempo}");
        let do_oathtool = String::from_utf8(saida.stdout).unwrap();
        assert_eq!(
            t.codigo_em(tempo).codigo(),
            do_oathtool.trim(),
            "T = {tempo}"
        );
    }
}

#[test]
fn restante_e_codigo_agora() {
    let t = Totp::de_semente(&semente(SEMENTE_D15)).unwrap();
    assert_eq!(t.codigo_em(0).restante(), 30);
    assert_eq!(t.codigo_em(29).restante(), 1);
    assert_eq!(t.codigo_em(30).restante(), 30);
    let agora = t.codigo_agora().unwrap();
    assert_eq!(agora.codigo().len(), 6);
    assert!(agora.codigo().bytes().all(|c| c.is_ascii_digit()));
    assert!((1..=30).contains(&agora.restante()));
    // Zeros à esquerda são mantidos (T = 1111111109 dá "071271").
    assert_eq!(t.codigo_em(1_111_111_109).to_string(), "071271");
}

#[test]
fn enchimento_no_fim_e_aceito() {
    let com = Totp::de_semente(&semente("JBSWY3DPEHPK3PXP======")).unwrap();
    let sem = Totp::de_semente(&semente(SEMENTE_D15)).unwrap();
    assert_eq!(com.codigo_em(59), sem.codigo_em(59));
}

#[test]
fn base32_vetores_rfc4648() {
    // RFC 4648 §10, sem o vazio (abaixo do mínimo de 16): "foobar" repetido para passar
    // do mínimo, decodificado e comparado.
    let texto = b"foobarfoobar";
    let b32 = "MZXW6YTBOJTG633CMFZA====";
    assert_eq!(&**decodificar_base32(b32.as_bytes()).unwrap(), texto);
    assert_eq!(
        &**decodificar_base32(RFC_SHA1.as_bytes()).unwrap(),
        b"12345678901234567890"
    );
}

#[test]
fn ataque_semente_malformada_e_recusada() {
    let ruins = [
        "",
        "JBSWY3DPEHPK3PX",                // 15 caracteres
        "jbswy3dpehpk3pxp",               // minúsculas (a entrada exige normalizada)
        "JBSWY3DPEHPK3PX1",               // 1 não é base32
        "JBSWY3DPEHPK3PX8",               // 8 não é base32
        "JBSWY3DP=EHPK3PXP",              // = no meio
        "JBSWY3DPEHPK3PXP=A",             // dado depois do =
        "JBSWY3DP EHPK3PXP",              // espaço
        "JBSWY3DPEHPK3PXP\n",             // quebra de linha
        "JBSWY3DPEHPK3PXP\0",             // nulo
        "JBSWY3DPEHPK3PXPÁ",              // não ASCII
        "================",               // só enchimento
        "otpauth://totp/x?secret=JBSWY3", // URI, não semente
    ];
    for r in ruins {
        assert_eq!(
            Totp::de_semente(&semente(r)).unwrap_err(),
            ErroTotp::SementeInvalida,
            "{r:?}"
        );
    }
    let longa = "A".repeat(MAX_SEMENTE + 1);
    assert!(Totp::de_semente(&semente(&longa)).is_err());
    let no_limite = "A".repeat(MAX_SEMENTE);
    assert!(Totp::de_semente(&semente(&no_limite)).is_ok());
    // Bytes inválidos em UTF-8 (a semente é `Segredo`, pode vir de qualquer lugar).
    assert!(Totp::de_semente(&Segredo::de_bytes(vec![0xff; 20])).is_err());
}

#[test]
fn ataque_digitos_fora_do_intervalo() {
    for d in [0, 1, 5, 9, 10, u32::MAX] {
        assert_eq!(
            Totp::com_digitos(&semente(SEMENTE_D15), d).unwrap_err(),
            ErroTotp::DigitosInvalidos
        );
    }
}

#[test]
fn ataque_tempo_extremo_nao_entra_em_panico() {
    let t = Totp::com_digitos(&semente(SEMENTE_D15), 8).unwrap();
    for tempo in [0, 1, u64::MAX, u64::MAX - 1, u64::MAX / PERIODO * PERIODO] {
        let c = t.codigo_em(tempo);
        assert_eq!(c.codigo().len(), 8);
        assert!((1..=PERIODO).contains(&c.restante()));
    }
}

#[test]
fn ataque_debug_e_erros_sem_semente() {
    // R8 / S13: nem o Debug do gerador nem os erros mostram a semente ou os bytes dela.
    let t = Totp::de_semente(&semente(SEMENTE_D15)).unwrap();
    let chave = decodificar_base32(SEMENTE_D15.as_bytes()).unwrap();
    let hex: String = chave.iter().map(|b| format!("{b:02x}")).collect();
    let bytes_debug = format!("{:?}", &chave[..]);
    for texto in [format!("{t:?}"), format!("{t:#?}")] {
        assert!(!texto.contains(SEMENTE_D15), "{texto}");
        assert!(!texto.contains(&hex), "{texto}");
        assert!(
            !texto.contains(&bytes_debug[1..bytes_debug.len() - 1]),
            "{texto}"
        );
        assert!(texto.contains("••••"));
    }
    // Semente quase válida (um caractere errado): o erro não a repete.
    let quase = "JBSWY3DPEHPK3PX1";
    let e = Totp::de_semente(&semente(quase)).unwrap_err();
    for texto in [e.to_string(), format!("{e:?}"), format!("{e:#?}")] {
        assert!(!texto.contains(quase));
        assert!(!texto.contains("JBSWY3"));
    }
}

#[test]
fn v2_28_totp_sem_clone_e_semente_zera() {
    // Em tempo de compilação: se `Totp` ganhar `Clone`, a chamada fica ambígua (E0283).
    trait SemClone<A> {
        fn x() {}
    }
    impl<T: ?Sized> SemClone<()> for T {}
    struct ComClone;
    impl<T: Clone> SemClone<ComClone> for T {}
    <Totp as SemClone<_>>::x();

    // A chave fica num tipo que zera ao sair.
    fn zera<T: zeroize::ZeroizeOnDrop>(_: &T) {}
    let t = Totp::de_semente(&semente(SEMENTE_D15)).unwrap();
    zera(&t.chave);
}

/// Fase 16 + 18: o `WARD_TOTP` que o `entrega::montar` põe no ambiente é o código deste
/// módulo, com os vetores do RFC 6238 (8 dígitos) e a semente de D15 (6 dígitos), num
/// relógio fixo. Os recursos de arquivo e túnel não são usados por um `login`.
#[test]
fn v2_25_ward_totp_da_entrega_vem_deste_modulo() {
    use crate::entrada::{Rascunho, RascunhoDados};
    use crate::entrega::{ErroRecurso, Recursos, montar};

    struct RelogioFixo {
        tempo: u64,
        digitos: u32,
    }
    impl Recursos for RelogioFixo {
        fn codigo_totp(&mut self, semente: &Segredo) -> Result<Zeroizing<String>, ErroRecurso> {
            let t = Totp::com_digitos(semente, self.digitos)
                .map_err(|_| ErroRecurso("semente TOTP inválida"))?;
            Ok(Zeroizing::new(t.codigo_em(self.tempo).codigo().to_owned()))
        }
        fn arquivo(&mut self, _: &str, _: &Segredo) -> Result<String, ErroRecurso> {
            Err(ErroRecurso("sem arquivo no teste"))
        }
        fn tunel(&mut self, _: &str, _: &str, _: u16) -> Result<u16, ErroRecurso> {
            Err(ErroRecurso("sem túnel no teste"))
        }
    }

    fn login(semente_b32: &str) -> crate::entrada::Entrada {
        let r = Rascunho {
            nome: "acme-wp-prod".to_owned(),
            grupo: "acme".to_owned(),
            ambiente: "prod".to_owned(),
            projeto: String::new(),
            tags: Vec::new(),
            dados: RascunhoDados::Login {
                url: "http://localhost:8081/wp-login.php".to_owned(),
                usuario: "wp_editor".to_owned(),
                senha: Some(semente("falso-WP-senha-02")),
                totp_semente: Some(semente(semente_b32)),
            },
            extras: Vec::new(),
            anexos: Vec::new(),
        };
        match r.validar() {
            Ok(e) => e,
            Err(erros) => panic!("devia validar: {erros}"),
        }
    }

    let rfc = login(RFC_SHA1);
    for (tempo, esperado) in [
        (59, "94287082"),
        (1_111_111_109, "07081804"),
        (2_000_000_000, "69279037"),
    ] {
        let mut r = RelogioFixo { tempo, digitos: 8 };
        let e = montar(&rfc, &mut r).unwrap();
        assert_eq!(e.valor("WARD_TOTP"), Some(esperado), "T = {tempo}");
    }
    let d15 = login(SEMENTE_D15);
    for (tempo, esperado) in D15_CONHECIDOS {
        let mut r = RelogioFixo { tempo, digitos: 6 };
        let e = montar(&d15, &mut r).unwrap();
        assert_eq!(e.valor("WARD_TOTP"), Some(esperado), "T = {tempo}");
        // A semente não vai para o ambiente (R8): nenhuma variável a contém.
        for v in e.variaveis() {
            assert!(!v.expor().contains(SEMENTE_D15), "{}", v.nome());
        }
    }
}
