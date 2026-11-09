//! Testes do formato (V2.1–V2.8). Todos com os parâmetros reais do Argon2id, a não ser
//! onde o teste diz o contrário. Senhas de teste falsas (D15).

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Condvar, Mutex};

use super::*;

const SENHA: &str = "falso-mestra-ward-01";
const SENHA_ERRADA: &str = "falso-mestra-ward-02";
const SENHA_NOVA: &str = "falso-mestra-ward-03";
const CONTEUDO: &[u8] = br#"{"entradas":[{"nome":"acme-pg-dev","senha":"falso-PG:s3nha/01@#"}]}"#;

/// Cada Argon2id real usa 256 MiB: no máximo 4 ao mesmo tempo entre todos os testes
/// (o `cargo test` roda os testes em paralelo).
static VAGAS: Mutex<usize> = Mutex::new(4);
static AVISO: Condvar = Condvar::new();

fn com_vaga<T>(f: impl FnOnce() -> T) -> T {
    {
        let mut v = AVISO
            .wait_while(VAGAS.lock().unwrap(), |v| *v == 0)
            .unwrap();
        *v -= 1;
    }
    struct Devolve;
    impl Drop for Devolve {
        fn drop(&mut self) {
            *VAGAS.lock().unwrap_or_else(|e| e.into_inner()) += 1;
            AVISO.notify_one();
        }
    }
    let _d = Devolve;
    f()
}

fn senha(s: &str) -> SenhaMestra {
    SenhaMestra::nova(s.to_owned())
}

fn criar(conteudo: &[u8]) -> Vec<u8> {
    com_vaga(|| {
        ChavesCofre::novo_com(&senha(SENHA), ParametrosKdf::PADRAO)
            .unwrap()
            .selar(conteudo)
            .unwrap()
    })
}

fn abrir_t(arquivo: &[u8], s: &str) -> Result<Zeroizing<Vec<u8>>, ErroFormato> {
    com_vaga(|| abrir(arquivo, &senha(s)).map(|(_, c)| c))
}

/// Abre cada arquivo em até 4 threads; devolve, por arquivo, se abriu e se houve pânico.
fn abrir_varios(arquivos: Vec<Vec<u8>>) -> Vec<(bool, bool)> {
    let fila = Mutex::new(arquivos.into_iter().enumerate().collect::<Vec<_>>());
    let saida = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                loop {
                    let Some((i, arq)) = fila.lock().unwrap().pop() else {
                        break;
                    };
                    let r = catch_unwind(AssertUnwindSafe(|| abrir_t(&arq, SENHA).is_ok()));
                    let item = match r {
                        Ok(abriu) => (i, abriu, false),
                        Err(_) => (i, false, true),
                    };
                    saida.lock().unwrap().push(item);
                }
            });
        }
    });
    let mut v = saida.into_inner().unwrap();
    v.sort_by_key(|x| x.0);
    v.into_iter().map(|(_, a, p)| (a, p)).collect()
}

#[test]
fn ida_e_volta() {
    let arq = criar(CONTEUDO);
    assert_eq!(arq.len(), TAM_CABECALHO + CONTEUDO.len() + TAM_TAG);
    assert_eq!(&abrir_t(&arq, SENHA).unwrap()[..], CONTEUDO);
}

#[test]
fn conteudo_vazio_abre() {
    let arq = criar(b"");
    assert_eq!(abrir_t(&arq, SENHA).unwrap().len(), 0);
}

/// Prova de S2 no formato: nenhum trecho do conteúdo (8 bytes ou mais) aparece no
/// arquivo cifrado.
#[test]
fn nada_em_claro_no_arquivo() {
    let arq = criar(CONTEUDO);
    for janela in CONTEUDO.windows(8) {
        assert!(!arq.windows(8).any(|w| w == janela));
    }
    assert!(!arq.windows(SENHA.len()).any(|w| w == SENHA.as_bytes()));
}

/// V2.1: senha errada dá o mesmo erro de arquivo adulterado, e nada é decifrado.
#[test]
fn v2_1_senha_errada_mesmo_erro_de_adulterado() {
    let arq = criar(CONTEUDO);
    let errada = abrir_t(&arq, SENHA_ERRADA);
    assert_eq!(errada, Err(ErroFormato::SenhaOuArquivo));

    let mut adulterado = arq.clone();
    let ultimo = adulterado.len() - 1;
    adulterado[ultimo] ^= 0x01;
    let adult = abrir_t(&adulterado, SENHA);
    assert_eq!(errada, adult);
    assert_eq!(
        errada.unwrap_err().to_string(),
        "senha mestra errada ou arquivo do cofre adulterado"
    );
}

/// V2.2: um bit trocado em **cada** byte do arquivo (um arquivo por posição): toda
/// abertura falha, sem pânico.
#[test]
fn v2_2_bit_trocado_em_cada_byte() {
    let arq = criar(b"falso-nota-01");
    let arquivos: Vec<Vec<u8>> = (0..arq.len())
        .map(|i| {
            let mut a = arq.clone();
            a[i] ^= 1 << (i % 8);
            a
        })
        .collect();
    let resultado = abrir_varios(arquivos);
    assert_eq!(resultado.len(), arq.len());
    for (i, (abriu, panico)) in resultado.into_iter().enumerate() {
        assert!(!panico, "pânico com o byte {i} trocado");
        assert!(!abriu, "abriu com o byte {i} trocado");
    }
    // Controle: o original abre.
    assert!(abrir_t(&arq, SENHA).is_ok());
}

/// V2.3: truncado em cada tamanho de 0 a n−1 e acrescido de 1 byte: toda abertura falha.
#[test]
fn v2_3_truncado_e_acrescido() {
    let arq = criar(b"falso-nota-01");
    for tam in 0..arq.len() {
        let r = catch_unwind(|| abrir_t(&arq[..tam], SENHA));
        assert!(matches!(r, Ok(Err(_))), "truncado em {tam} não falhou");
    }
    for extra in [0x00u8, 0x01, 0xff] {
        let mut a = arq.clone();
        a.push(extra);
        assert_eq!(abrir_t(&a, SENHA), Err(ErroFormato::SenhaOuArquivo));
    }
    // Acrescido e com o campo de tamanho "consertado": a tag do corpo pega (o tamanho
    // está no AAD e o corpo mudou).
    let mut a = arq.clone();
    a.push(0x00);
    let novo = (a.len() - TAM_CABECALHO) as u64;
    a[P_TAMANHO..TAM_CABECALHO].copy_from_slice(&novo.to_le_bytes());
    assert_eq!(abrir_t(&a, SENHA), Err(ErroFormato::SenhaOuArquivo));
    assert!(abrir_t(&arq, SENHA).is_ok());
}

/// V2.4: o cabeçalho registra Argon2id com memória ≥ 262144 KiB, passes ≥ 3 e o
/// paralelismo.
#[test]
fn v2_4_cabecalho_registra_parametros() {
    let arq = criar(CONTEUDO);
    let cab = ler_cabecalho(&arq).unwrap();
    assert_eq!(&arq[..8], b"WARDCOFR");
    assert_eq!(arq[P_KDF], KDF_ARGON2ID_13);
    assert_eq!(cab.versao, 1);
    assert!(cab.kdf.memoria_kib >= 262_144);
    assert!(cab.kdf.passes >= 3);
    assert!(cab.kdf.paralelismo >= 1);
    assert_eq!(cab.kdf, ParametrosKdf::PADRAO);
}

fn com_parametros(arq: &[u8], kdf: ParametrosKdf) -> Vec<u8> {
    let mut a = arq.to_vec();
    a[P_MEMORIA..P_PASSES].copy_from_slice(&kdf.memoria_kib.to_le_bytes());
    a[P_PASSES..P_PARALELISMO].copy_from_slice(&kdf.passes.to_le_bytes());
    a[P_PARALELISMO..P_SAL].copy_from_slice(&kdf.paralelismo.to_le_bytes());
    a
}

const OITO_MIB: ParametrosKdf = ParametrosKdf {
    memoria_kib: 8 * 1024,
    passes: 3,
    paralelismo: 4,
};

/// V2.4, ataque de rebaixamento: o atacante reescreve o cabeçalho com 8 MiB. A abertura
/// recusa (abaixo do mínimo) e, mesmo que o mínimo aceitasse 8 MiB, a autenticação
/// pega (o cabeçalho é dado autenticado e entra na derivação).
#[test]
fn v2_4_rebaixamento_para_8_mib_falha() {
    let arq = criar(CONTEUDO);
    let rebaixado = com_parametros(&arq, OITO_MIB);
    assert_eq!(
        abrir_t(&rebaixado, SENHA),
        Err(ErroFormato::ParametrosAbaixoDoMinimo)
    );
    let r = abrir_com_minimo(&rebaixado, &senha(SENHA), &OITO_MIB).map(|_| ());
    assert_eq!(r, Err(ErroFormato::SenhaOuArquivo));

    // Rebaixar só os passes também.
    let p = ParametrosKdf {
        passes: 1,
        ..ParametrosKdf::PADRAO
    };
    assert_eq!(
        abrir_t(&com_parametros(&arq, p), SENHA),
        Err(ErroFormato::ParametrosAbaixoDoMinimo)
    );
}

/// V2.4: um arquivo bem formado (autêntico) que declara parâmetros abaixo do mínimo é
/// recusado. Controle: com um mínimo de teste ele abre, então é bem formado.
#[test]
fn v2_4_bem_formado_abaixo_do_minimo_recusado() {
    let s = senha(SENHA);
    let fraco = ChavesCofre::novo_interno(&s, OITO_MIB, &OITO_MIB)
        .unwrap()
        .selar(CONTEUDO)
        .unwrap();
    assert_eq!(
        abrir(&fraco, &s).map(|_| ()),
        Err(ErroFormato::ParametrosAbaixoDoMinimo)
    );
    let (_, c) = abrir_com_minimo(&fraco, &s, &OITO_MIB).unwrap();
    assert_eq!(&c[..], CONTEUDO);
    // A API pública não cria cofre fraco.
    assert_eq!(
        ChavesCofre::novo_com(&s, OITO_MIB).map(|_| ()),
        Err(ErroFormato::ParametrosAbaixoDoMinimo)
    );
}

/// Ataque de negação de serviço: cabeçalho que pede memória ou passes enormes é recusado
/// antes de derivar (não aloca nada).
#[test]
fn parametros_acima_do_maximo_recusados_sem_derivar() {
    let arq = criar(b"falso-nota-01");
    for kdf in [
        ParametrosKdf {
            memoria_kib: u32::MAX,
            ..ParametrosKdf::PADRAO
        },
        ParametrosKdf {
            passes: u32::MAX,
            ..ParametrosKdf::PADRAO
        },
        ParametrosKdf {
            paralelismo: 0xFF_FFFF,
            ..ParametrosKdf::PADRAO
        },
    ] {
        let inicio = std::time::Instant::now();
        let r = abrir(&com_parametros(&arq, kdf), &senha(SENHA)).map(|_| ());
        assert_eq!(r, Err(ErroFormato::ParametrosAcimaDoMaximo));
        assert!(inicio.elapsed() < std::time::Duration::from_millis(100));
    }
    let zero = ParametrosKdf {
        paralelismo: 0,
        ..ParametrosKdf::PADRAO
    };
    assert_eq!(
        abrir(&com_parametros(&arq, zero), &senha(SENHA)).map(|_| ()),
        Err(ErroFormato::ParametrosAbaixoDoMinimo)
    );
    assert_eq!(
        ChavesCofre::novo_com(
            &senha(SENHA),
            ParametrosKdf {
                memoria_kib: u32::MAX,
                ..ParametrosKdf::PADRAO
            }
        )
        .map(|_| ()),
        Err(ErroFormato::ParametrosAcimaDoMaximo)
    );
}

/// O corpo está preso ao cabeçalho inteiro pelo AAD: com a chave de dados certa, o
/// corpo não abre se qualquer byte do cabeçalho mudar.
#[test]
fn cabecalho_e_aad_do_corpo() {
    let s = senha(SENHA);
    let original = criar(CONTEUDO);
    let (cofre, _) = com_vaga(|| abrir(&original, &s).unwrap());
    let arq = cofre.selar(CONTEUDO).unwrap();
    let mut nonce = [0u8; TAM_NONCE];
    nonce.copy_from_slice(&arq[P_NONCE_DADOS..P_TAMANHO]);
    let corpo = &arq[TAM_CABECALHO..];
    assert!(cripto::abrir(&cofre.chave_dados, &nonce, &arq[..TAM_CABECALHO], corpo).is_ok());
    for i in 0..TAM_CABECALHO {
        let mut cab = arq[..TAM_CABECALHO].to_vec();
        cab[i] ^= 0x01;
        assert!(
            cripto::abrir(&cofre.chave_dados, &nonce, &cab, corpo).is_err(),
            "byte {i} do cabeçalho fora do AAD"
        );
    }
}

/// Ataque de recorte: cabeçalho de um cofre com o corpo de outro (mesma senha) não abre.
/// Nem o corpo de uma gravação anterior do mesmo cofre com o cabeçalho de outra.
#[test]
fn recorte_entre_cofres_e_gravacoes_falha() {
    let a = criar(b"falso-conteudo-a");
    let b = criar(b"falso-conteudo-b");
    let mut misto = a[..TAM_CABECALHO].to_vec();
    misto.extend_from_slice(&b[TAM_CABECALHO..]);
    assert_eq!(abrir_t(&misto, SENHA), Err(ErroFormato::SenhaOuArquivo));

    let (cofre, _) = com_vaga(|| abrir(&a, &senha(SENHA)).unwrap());
    let g1 = cofre.selar(b"falso-versao-1").unwrap();
    let g2 = cofre.selar(b"falso-versao-2").unwrap();
    let mut misto = g1[..TAM_CABECALHO].to_vec();
    misto.extend_from_slice(&g2[TAM_CABECALHO..]);
    assert_eq!(abrir_t(&misto, SENHA), Err(ErroFormato::SenhaOuArquivo));
    // E a chave embrulhada de um cofre no cabeçalho do outro.
    let mut misto = a.clone();
    misto[P_EMBRULHADA..P_NONCE_DADOS].copy_from_slice(&b[P_EMBRULHADA..P_NONCE_DADOS]);
    assert_eq!(abrir_t(&misto, SENHA), Err(ErroFormato::SenhaOuArquivo));
}

/// V2.5 pelo caminho do formato: o arquivo é exatamente Argon2id (sem extras) → embrulho
/// XChaCha20-Poly1305 com AAD = bytes 0..80 → corpo XChaCha20-Poly1305 com AAD =
/// cabeçalho. Refeito aqui só com as funções de `cripto` (as mesmas dos vetores).
#[test]
fn v2_5_composicao_do_formato() {
    let arq = criar(CONTEUDO);
    let cab = ler_cabecalho(&arq).unwrap();
    let kek = com_vaga(|| {
        cripto::derivar_kek(SENHA.as_bytes(), &arq[P_SAL..P_NONCE_CHAVE], &cab.kdf).unwrap()
    });
    let nonce_chave: [u8; TAM_NONCE] = arq[P_NONCE_CHAVE..P_EMBRULHADA].try_into().unwrap();
    let chave = cripto::abrir(
        &kek,
        &nonce_chave,
        &arq[..P_EMBRULHADA],
        &arq[P_EMBRULHADA..P_NONCE_DADOS],
    )
    .unwrap();
    let chave: [u8; TAM_CHAVE] = chave[..].try_into().unwrap();
    let nonce_dados: [u8; TAM_NONCE] = arq[P_NONCE_DADOS..P_TAMANHO].try_into().unwrap();
    let texto = cripto::abrir(
        &chave,
        &nonce_dados,
        &arq[..TAM_CABECALHO],
        &arq[TAM_CABECALHO..],
    )
    .unwrap();
    assert_eq!(&texto[..], CONTEUDO);
}

/// V2.6: dois cofres com a mesma senha e o mesmo conteúdo têm sal, nonces, chave
/// embrulhada e texto cifrado diferentes.
#[test]
fn v2_6_dois_cofres_iguais_sao_diferentes() {
    let a = criar(CONTEUDO);
    let b = criar(CONTEUDO);
    assert_eq!(a.len(), b.len());
    let campos = [
        ("sal", P_SAL..P_NONCE_CHAVE),
        ("nonce da chave", P_NONCE_CHAVE..P_EMBRULHADA),
        ("chave embrulhada", P_EMBRULHADA..P_NONCE_DADOS),
        ("nonce dos dados", P_NONCE_DADOS..P_TAMANHO),
        ("texto cifrado", TAM_CABECALHO..a.len()),
    ];
    for (nome, faixa) in campos {
        assert_ne!(a[faixa.clone()], b[faixa], "{nome} igual nos dois cofres");
    }
    // A chave de dados também é outra (não só o embrulho).
    let (ca, _) = com_vaga(|| abrir(&a, &senha(SENHA)).unwrap());
    let (cb, _) = com_vaga(|| abrir(&b, &senha(SENHA)).unwrap());
    assert_ne!(&ca.chave_dados[..], &cb.chave_dados[..]);
}

/// Regravar pelo cofre aberto (caminho do daemon): nonce de dados novo a cada gravação,
/// e o arquivo abre com a senha.
#[test]
fn selar_de_novo_reabre_e_troca_o_nonce() {
    let arq = criar(CONTEUDO);
    let (cofre, _) = com_vaga(|| abrir(&arq, &senha(SENHA)).unwrap());
    let g1 = cofre.selar(b"falso-versao-1").unwrap();
    let g2 = cofre.selar(b"falso-versao-1").unwrap();
    assert_ne!(g1[P_NONCE_DADOS..P_TAMANHO], g2[P_NONCE_DADOS..P_TAMANHO]);
    assert_eq!(g1[..P_NONCE_DADOS], arq[..P_NONCE_DADOS]);
    assert_eq!(&abrir_t(&g2, SENHA).unwrap()[..], b"falso-versao-1");
}

/// V2.8: trocar a senha mestra: a antiga falha, a nova abre, o conteúdo é o mesmo; e o
/// arquivo novo não reaproveita sal nem chave de dados.
#[test]
fn v2_8_troca_de_senha() {
    let arq = criar(CONTEUDO);
    let (novo_cofre, conteudo_trocado) =
        com_vaga(|| trocar_senha(&arq, &senha(SENHA), &senha(SENHA_NOVA)).unwrap());
    let novo = novo_cofre.selar(&conteudo_trocado).unwrap();
    assert_eq!(abrir_t(&novo, SENHA), Err(ErroFormato::SenhaOuArquivo));
    assert_eq!(&abrir_t(&novo, SENHA_NOVA).unwrap()[..], CONTEUDO);
    assert_eq!(&conteudo_trocado[..], CONTEUDO);
    assert_ne!(arq[P_SAL..P_NONCE_CHAVE], novo[P_SAL..P_NONCE_CHAVE]);
    let (antigo, _) = com_vaga(|| abrir(&arq, &senha(SENHA)).unwrap());
    assert_ne!(&antigo.chave_dados[..], &novo_cofre.chave_dados[..]);
    assert_eq!(ler_cabecalho(&novo).unwrap().kdf, ParametrosKdf::PADRAO);
}

/// Ataque: trocar a senha sem saber a atual não funciona; senha nova vazia é recusada.
#[test]
fn troca_de_senha_exige_a_atual() {
    let arq = criar(CONTEUDO);
    assert_eq!(
        com_vaga(|| trocar_senha(&arq, &senha(SENHA_ERRADA), &senha(SENHA_NOVA)).map(|_| ())),
        Err(ErroFormato::SenhaOuArquivo)
    );
    assert_eq!(
        trocar_senha(&arq, &senha(SENHA), &senha("")).map(|_| ()),
        Err(ErroFormato::SenhaVazia)
    );
    assert_eq!(
        ChavesCofre::novo_com(&senha(""), ParametrosKdf::PADRAO).map(|_| ()),
        Err(ErroFormato::SenhaVazia)
    );
}

/// `Debug` não mostra senha nem chave (S13).
#[test]
fn debug_nao_mostra_segredo() {
    let s = senha(SENHA);
    let d = format!("{s:?}");
    assert!(!d.contains(SENHA));
    assert!(d.contains("••••"));

    let original = criar(CONTEUDO);
    let (cofre, conteudo) = com_vaga(|| abrir(&original, &s).unwrap());
    let d = format!("{cofre:?}");
    let chave_hex: String = cofre
        .chave_dados
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let chave_dec = format!("{:?}", &cofre.chave_dados[..]);
    assert!(!d.contains(&chave_hex));
    assert!(!d.contains(&chave_dec[1..chave_dec.len() - 1]));
    assert!(d.contains("••••"));
    drop(conteudo);
}

/// Lixo, arquivo vazio e arquivo de outro formato dão o erro genérico, sem pânico.
#[test]
fn lixo_nao_abre() {
    let s = senha(SENHA);
    for lixo in [
        Vec::new(),
        b"WARDCOFR".to_vec(),
        vec![0u8; TAM_CABECALHO + TAM_TAG],
        vec![0xffu8; 4096],
    ] {
        assert_eq!(
            abrir(&lixo, &s).map(|_| ()),
            Err(ErroFormato::SenhaOuArquivo)
        );
    }
    let mut outra_versao = criar(b"x");
    outra_versao[P_VERSAO] = 2;
    assert_eq!(
        abrir(&outra_versao, &s).map(|_| ()),
        Err(ErroFormato::VersaoNaoSuportada(2))
    );
}
