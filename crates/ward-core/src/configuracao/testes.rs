//! Testes da configuração do cofre (D30): hash do PIN (criar, conferir, formato PHC e
//! ataques ao hash), tempo de trava e backup externo. Só valores falsos (PIN de D15).

use std::sync::OnceLock;

use super::*;
use crate::formato::{CofreAberto, SenhaMestra};

const PIN: &[u8] = b"135790";

/// Um hash do PIN de teste, calculado uma vez (o Argon2id do PIN custa 64 MiB).
fn hash() -> &'static HashPin {
    static H: OnceLock<HashPin> = OnceLock::new();
    H.get_or_init(|| HashPin::criar(PIN).unwrap())
}

fn texto(h: &HashPin) -> &str {
    core::str::from_utf8(h.bytes()).unwrap()
}

#[test]
fn hash_do_pin_confere_o_pin_certo_e_recusa_os_outros() {
    let h = hash();
    assert!(h.conferir(PIN));
    for errado in [
        &b"135791"[..],
        b"035790",
        b"13579",
        b"1357900",
        b"",
        b"abcdef",
        b"13579 ",
        b" 135790",
        b"135790\0",
        "١٣٥٧٩٠".as_bytes(),
    ] {
        assert!(!h.conferir(errado), "aceitou um PIN errado");
    }
}

#[test]
fn pin_de_4_a_6_digitos() {
    for invalido in [
        &b""[..],
        b"1",
        b"123",
        b"1234567",
        b"12a4",
        b"12 34",
        b"-1234",
        b"+1234",
    ] {
        assert_eq!(
            HashPin::criar(invalido).map(|_| ()),
            Err(ErroConfiguracao::PinInvalido)
        );
    }
    let quatro = HashPin::criar(b"0042").unwrap();
    assert!(quatro.conferir(b"0042"));
    assert!(!quatro.conferir(b"042"));
    assert!(!quatro.conferir(b"00420"));
}

/// O hash é PHC Argon2id v=19 com os parâmetros de [`KDF_PIN`], sal de 16 bytes e
/// saída de 32; cabe no limite; e a biblioteca (`argon2::PasswordVerifier`, que faz a
/// leitura do PHC por conta própria) concorda com ele.
#[test]
fn hash_e_phc_argon2id_padrao() {
    use argon2::password_hash::PasswordVerifier;

    let t = texto(hash());
    assert!(t.starts_with("$argon2id$v=19$m=65536,t=3,p=1$"));
    let campos: Vec<&str> = t.split('$').collect();
    assert_eq!(campos.len(), 6);
    assert_eq!(campos[4].len(), 22);
    assert_eq!(campos[5].len(), 43);
    assert!(t.len() <= MAX_HASH_PIN);
    assert_eq!(hash().parametros(), KDF_PIN);
    let biblioteca = argon2::Argon2::default();
    assert!(biblioteca.verify_password(PIN, t).is_ok());
    assert!(biblioteca.verify_password(b"135791", t).is_err());
}

#[test]
fn cada_hash_tem_sal_proprio() {
    let outro = HashPin::criar(PIN).unwrap();
    assert_ne!(texto(&outro), texto(hash()));
    assert!(outro.conferir(PIN));
}

/// Ataque: qualquer caractere trocado no hash (sal ou saída) faz o PIN certo não
/// conferir (ou o hash nem ser aceito).
#[test]
fn hash_adulterado_nao_confere() {
    let t = texto(hash()).to_owned();
    let inicio_sal = t.rfind('$').unwrap() - 22;
    for pos in [inicio_sal, inicio_sal + 10, t.len() - 2, t.len() - 10] {
        let mut b = t.clone().into_bytes();
        b[pos] = if b[pos] == b'A' { b'B' } else { b'A' };
        if let Some(h) = HashPin::de_phc(&b) {
            assert!(!h.conferir(PIN), "hash adulterado conferiu");
        }
    }
    // Sem adulteração, o texto relido confere.
    assert!(HashPin::de_phc(t.as_bytes()).unwrap().conferir(PIN));
}

/// Ataque: um hash com custo inflado (4 GiB de memória) é recusado sem rodar o Argon2.
#[test]
fn hash_inflado_recusado_rapido() {
    let t = texto(hash()).replace("m=65536", "m=4194304");
    let inicio = std::time::Instant::now();
    assert!(HashPin::de_phc(t.as_bytes()).is_none());
    assert!(!conferir_texto(t.as_bytes()));
    assert!(inicio.elapsed() < std::time::Duration::from_millis(100));
}

/// O `Debug` não mostra o hash nem o PIN, nem na configuração nem no cofre aberto, e
/// os erros não têm valor (S13, V2.27).
#[test]
fn debug_e_erros_sem_segredo() {
    let t = texto(hash()).to_owned();
    let saida = t.rsplit('$').next().unwrap().to_owned();
    let d = format!("{:?} {:#?}", hash(), hash());
    assert!(d.contains("••••"));
    let cfg = Configuracao::nova(HashPin::de_phc(t.as_bytes()).unwrap());
    let dc = format!("{cfg:?} {cfg:#?}");
    let senha = SenhaMestra::nova("falso-mestra-ward-01".to_owned());
    let cofre = CofreAberto::novo(&senha, cfg).unwrap();
    let dcofre = format!("{cofre:?} {cofre:#?}");
    for texto in [&d, &dc, &dcofre] {
        assert!(!texto.contains(&saida), "Debug com o hash");
        assert!(!texto.contains("135790"), "Debug com o PIN");
        assert!(!texto.contains("argon2id$"), "Debug com o texto PHC");
        assert!(!texto.contains("falso-mestra"), "Debug com a senha");
    }
    assert!(dcofre.contains("minutos_trava: 240"));
    for e in [
        ErroConfiguracao::PinInvalido,
        ErroConfiguracao::Hash,
        ErroConfiguracao::TempoDeTrava,
        ErroConfiguracao::UuidInvalido,
        ErroConfiguracao::BackupSemUuid,
    ] {
        let s = format!("{e} {e:?}");
        assert!(!s.contains("135790") && !format!("{e}").is_empty());
    }
}

#[test]
fn configuracao_nova_tem_os_padroes() {
    let cfg = Configuracao::nova(HashPin::de_phc(hash().bytes()).unwrap());
    assert_eq!(cfg.minutos_trava(), 240);
    assert_eq!(cfg.minutos_trava(), crate::limites::MINUTOS_TRAVA_PADRAO);
    assert!(!cfg.backup().ligado());
    assert_eq!(cfg.backup().uuid(), None);
    assert!(cfg.conferir_pin(PIN));
}

#[test]
fn tempo_de_trava_na_faixa() {
    let mut cfg = Configuracao::nova(HashPin::de_phc(hash().bytes()).unwrap());
    for invalido in [0, MAX_MINUTOS_TRAVA + 1, u32::MAX] {
        assert_eq!(
            cfg.definir_minutos_trava(invalido),
            Err(ErroConfiguracao::TempoDeTrava)
        );
        assert_eq!(cfg.minutos_trava(), 240, "valor recusado não muda nada");
    }
    for valido in [MIN_MINUTOS_TRAVA, 30, MAX_MINUTOS_TRAVA] {
        cfg.definir_minutos_trava(valido).unwrap();
        assert_eq!(cfg.minutos_trava(), valido);
    }
}

#[test]
fn backup_externo() {
    assert_eq!(
        BackupExterno::novo(true, None),
        Err(ErroConfiguracao::BackupSemUuid)
    );
    for invalido in [
        "",
        "-1234",
        "1234-",
        "12 34",
        "../x",
        "1234/ABCD",
        "ç",
        &"a".repeat(37),
    ] {
        assert_eq!(
            BackupExterno::novo(false, Some(invalido)),
            Err(ErroConfiguracao::UuidInvalido),
            "{invalido}"
        );
    }
    for valido in [
        "1234-ABCD",
        "0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0",
        "A1B2C3D4E5F60718",
    ] {
        let b = BackupExterno::novo(true, Some(valido)).unwrap();
        assert!(b.ligado());
        assert_eq!(b.uuid(), Some(valido));
    }
    let b = BackupExterno::novo(false, Some("1234-ABCD")).unwrap();
    assert!(!b.ligado());
    assert_eq!(
        BackupExterno::desligado(),
        BackupExterno::novo(false, None).unwrap()
    );
}

#[test]
fn trocar_o_pin() {
    let mut cfg = Configuracao::nova(HashPin::de_phc(hash().bytes()).unwrap());
    cfg.trocar_hash_pin(HashPin::criar(b"2468").unwrap());
    assert!(cfg.conferir_pin(b"2468"));
    assert!(!cfg.conferir_pin(PIN));
}
