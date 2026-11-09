//! Fase 39b (V5.12; S1, S11, D29): anexos na edição pelo `salvar_entrada` com
//! `manter_anexos`. O daemon copia o conteúdo dos anexos mantidos da entrada editada; o
//! conteúdo nunca vai nem volta pelo socket. Daemon em processo, cofre de teste próprio
//! em `target/tmp`, valores falsos.

mod comum;

use comum::*;
use ward_core::entrada::Entrada;
use ward_core::protocolo::{ErroResposta, Papel, Resposta};

const ANEXO_A: &[u8] = b"FALSO_A=falso-anexo-39b-a\n";
const ANEXO_B: &[u8] = b"FALSO_B=falso-anexo-39b-b\n";

fn generico(nome: &str, anexos: &[(&str, &[u8])]) -> String {
    let anexos: Vec<String> = anexos
        .iter()
        .map(|(n, c)| format!(r#"{{"nome":"{n}","conteudo":"{}"}}"#, b64(c)))
        .collect();
    format!(
        r#"{{"entrada":{{"nome":"{nome}","grupo":"clientex","ambiente":"stage","tipo":"generico","extras":[{{"nome":"REGIAO","valor":"{}","segredo":false}}],"anexos":[{}]}}}}"#,
        b64(b"r-01"),
        anexos.join(",")
    )
}

/// Edita `anterior` mantendo `manter` e mandando `novos`.
fn editar(anterior: &str, manter: &[&str], novos: &[(&str, &[u8])]) -> String {
    let manter: Vec<String> = manter.iter().map(|n| format!(r#""{n}""#)).collect();
    generico(anterior, novos).replacen(
        '{',
        &format!(
            r#"{{"anterior":"{anterior}","manter_anexos":[{}],"#,
            manter.join(",")
        ),
        1,
    )
}

/// As entradas como estão no disco (abre com a senha de teste).
fn no_disco(dir: &Dir) -> Vec<Entrada> {
    let bytes = std::fs::read(dir.caminhos().cofre).unwrap();
    let s = ward_core::formato::SenhaMestra::nova(SENHA.to_owned());
    ward_core::corpo::abrir_entradas_lida(&bytes, &s, 0, 0)
        .unwrap()
        .1
}

fn anexos_no_disco(dir: &Dir, nome: &str) -> Vec<(String, Vec<u8>)> {
    no_disco(dir)
        .iter()
        .find(|e| e.nome() == nome)
        .unwrap()
        .anexos()
        .iter()
        .map(|a| (a.nome().to_owned(), a.conteudo().expor().to_vec()))
        .collect()
}

/// V5.12: manter, adicionar e remover anexo de uma entrada já salva. O conteúdo
/// mantido é o mesmo byte a byte no disco, e nenhuma resposta do daemon o traz.
#[test]
fn v5_12_editar_mantem_adiciona_e_remove_anexo() {
    let dir = Dir::novo("anexos-edicao");
    let (d, _) = cofre_de_teste(&dir);
    let r = pedir(
        &d,
        Papel::App,
        "salvar_entrada",
        &generico("g1", &[("a.env", ANEXO_A)]),
    );
    assert!(ok(&r), "{r:?}");

    // Manter `a.env` e adicionar `b.env`.
    let r = pedir(
        &d,
        Papel::App,
        "salvar_entrada",
        &editar("g1", &["a.env"], &[("b.env", ANEXO_B)]),
    );
    assert!(ok(&r), "{r:?}");
    assert_eq!(
        anexos_no_disco(&dir, "g1"),
        [
            ("a.env".to_owned(), ANEXO_A.to_vec()),
            ("b.env".to_owned(), ANEXO_B.to_vec())
        ]
    );
    let det = pedir(&d, Papel::App, "detalhe", r#"{"entrada":"g1"}"#);
    let Resposta::Detalhe(x) = &det else {
        panic!("{det:?}")
    };
    assert_eq!(x.anexos.len(), 2);
    let bytes = codificado(Papel::App, &det);
    for c in [ANEXO_A, ANEXO_B] {
        assert!(!contem(&bytes, c) && !contem(&bytes, b64(c).as_bytes()));
    }

    // Renomear mantendo os dois: o conteúdo vai junto.
    let renomear =
        editar("g1", &["b.env", "a.env"], &[]).replacen(r#""nome":"g1""#, r#""nome":"g2""#, 1);
    let r = pedir(&d, Papel::App, "salvar_entrada", &renomear);
    assert!(ok(&r), "{r:?}");
    assert_eq!(
        anexos_no_disco(&dir, "g2"),
        [
            ("b.env".to_owned(), ANEXO_B.to_vec()),
            ("a.env".to_owned(), ANEXO_A.to_vec())
        ]
    );

    // Remover: o que não está em `manter_anexos` sai.
    let r = pedir(
        &d,
        Papel::App,
        "salvar_entrada",
        &editar("g2", &["a.env"], &[]),
    );
    assert!(ok(&r), "{r:?}");
    assert_eq!(
        anexos_no_disco(&dir, "g2"),
        [("a.env".to_owned(), ANEXO_A.to_vec())]
    );
    // Sem `manter_anexos` (o pedido de antes da Fase 39b): todos saem.
    let r = pedir(&d, Papel::App, "salvar_entrada", &editar("g2", &[], &[]));
    assert!(ok(&r), "{r:?}");
    assert!(anexos_no_disco(&dir, "g2").is_empty());
}

/// Ataques: manter anexo de outra entrada, nome inexistente, numa entrada nova, pelo
/// agente ou pela extensão, e mantido + novo acima de 256 KiB (D29). Nada muda no
/// disco.
#[test]
fn v5_12_ataque_manter_anexos_forjado() {
    let dir = Dir::novo("anexos-ataque");
    let (d, _) = cofre_de_teste(&dir);
    for q in [
        generico("g1", &[("a.env", ANEXO_A)]),
        generico("g2", &[("b.env", ANEXO_B)]),
    ] {
        assert!(ok(&pedir(&d, Papel::App, "salvar_entrada", &q)));
    }
    let grande = vec![b'x'; 200 * 1024];
    assert!(ok(&pedir(
        &d,
        Papel::App,
        "salvar_entrada",
        &generico("g3", &[("grande.bin", &grande)])
    )));
    let antes = no_disco(&dir).len();

    // O anexo de `g2` não é alcançável editando `g1`.
    let r = pedir(
        &d,
        Papel::App,
        "salvar_entrada",
        &editar("g1", &["b.env"], &[]),
    );
    assert_eq!(erro(&r), Some(ErroResposta::PedidoInvalido), "{r:?}");
    // Nem com caminho.
    let r = pedir(
        &d,
        Papel::App,
        "salvar_entrada",
        &editar("g1", &["../g2/b.env"], &[]),
    );
    assert_eq!(erro(&r), Some(ErroResposta::PedidoInvalido), "{r:?}");
    // Entrada editada que não existe.
    let r = pedir(
        &d,
        Papel::App,
        "salvar_entrada",
        &editar("nao-existe", &["a.env"], &[]),
    );
    assert_eq!(erro(&r), Some(ErroResposta::EntradaInexistente), "{r:?}");
    // Entrada nova "copiando" o anexo de `g1`: a leitura recusa.
    let novo = generico("g9", &[]).replacen('{', r#"{"manter_anexos":["a.env"],"#, 1);
    let r = pedir(&d, Papel::App, "salvar_entrada", &novo);
    assert_eq!(erro(&r), Some(ErroResposta::PedidoInvalido), "{r:?}");
    // Agente e extensão não têm a operação.
    for papel in [Papel::Agente, Papel::Extensao] {
        let r = pedir(&d, papel, "salvar_entrada", &editar("g1", &["a.env"], &[]));
        assert_eq!(erro(&r), Some(ErroResposta::NaoAutorizado), "{papel:?}");
    }
    // 200 KiB mantidos + 60 KiB novos passam da soma: erro de validação.
    let novo60 = vec![b'y'; 60 * 1024];
    let r = pedir(
        &d,
        Papel::App,
        "salvar_entrada",
        &editar("g3", &["grande.bin"], &[("mais.bin", &novo60)]),
    );
    assert!(
        matches!(erro(&r), Some(ErroResposta::Validacao(_))),
        "{r:?}"
    );
    // Nome mantido igual ao de um anexo novo: erro de validação (nome repetido).
    let r = pedir(
        &d,
        Papel::App,
        "salvar_entrada",
        &editar("g1", &["a.env"], &[("a.env", ANEXO_B)]),
    );
    assert!(
        matches!(erro(&r), Some(ErroResposta::Validacao(_))),
        "{r:?}"
    );

    // Nada mudou no disco.
    assert_eq!(no_disco(&dir).len(), antes);
    assert_eq!(
        anexos_no_disco(&dir, "g1"),
        [("a.env".to_owned(), ANEXO_A.to_vec())]
    );
    assert_eq!(
        anexos_no_disco(&dir, "g2"),
        [("b.env".to_owned(), ANEXO_B.to_vec())]
    );
    assert_eq!(anexos_no_disco(&dir, "g3").len(), 1);
}
