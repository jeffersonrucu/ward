//! Testes dos limites numa fonte só (D32, V2.35) e do orçamento da máscara (D29, V2.36).
//!
//! Todo número aqui sai de `crate::limites`: se uma constante mudar, validação, leitura
//! do corpo e orçamento mudam juntos, e estes testes continuam valendo no novo limite.
//! Só valores falsos.

use zeroize::Zeroizing;

use super::*;
use crate::configuracao::{Configuracao, HashPin};
use crate::corpo::{ErroCorpo, desserializar, serializar};
use crate::entrada::{
    Entrada, ErrosValidacao, GRANDE_PARA_MASCARA, Rascunho, RascunhoAnexo, RascunhoDados,
    RascunhoExtra, Segredo, TipoEntrada,
};
use crate::entrega::{self, Entrega, ErroRecurso, Recursos};
use crate::mascara::{self, ConstrutorMascara, ErroMascara};

// ---- Ajudantes ----

/// Configuração do corpo v2 (hash do PIN falso de D15), calculada uma vez.
fn cfg() -> &'static Configuracao {
    static C: std::sync::OnceLock<Configuracao> = std::sync::OnceLock::new();
    C.get_or_init(|| Configuracao::nova(HashPin::criar(b"135790").unwrap()))
}

fn t(v: &str) -> String {
    v.to_owned()
}

fn seg(v: Vec<u8>) -> Option<Segredo> {
    Some(Segredo::de_bytes(v))
}

/// Texto de `n` bytes que exercita as variações da máscara: escape JSON (`"`, `\`, `/`)
/// e percent-encoding na URL (`@`, `:`, `%`), sem nulo nem controle.
fn texto_dificil(n: usize) -> Vec<u8> {
    const PADRAO: &[u8] = b"falso\"\\/@:%-k";
    (0..n).map(|i| PADRAO[i % PADRAO.len()]).collect()
}

/// Bytes de 0 a 255 repetidos (binário, com quebras de linha).
fn binario(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i % 256) as u8).collect()
}

fn chave_falsa(n: usize) -> Vec<u8> {
    let rotulo = ["OPENSSH", "PRIVATE KEY"].join(" ");
    let mut v = format!("-----BEGIN {rotulo}-----\n").into_bytes();
    let fim = format!("\n-----END {rotulo}-----\n").into_bytes();
    let miolo = n.saturating_sub(v.len() + fim.len());
    v.extend((0..miolo).map(|i| if i % 70 == 69 { b'\n' } else { b'f' }));
    v.extend(fim);
    v
}

fn rascunho(nome: &str, dados: RascunhoDados) -> Rascunho {
    Rascunho {
        nome: t(nome),
        grupo: t("zeta"),
        ambiente: t("stage"),
        projeto: t("projeto"),
        tags: (1..=MAX_TAGS).map(|n| format!("tag-{n:02}")).collect(),
        dados,
        extras: Vec::new(),
        anexos: Vec::new(),
    }
}

/// Os campos do tipo grandes (segredos de 4 KiB, que já geram URL, Basic auth e
/// escapes; a chave privada do `servidor` e do `ftp/sftp` no limite de 64 KiB, em
/// várias linhas, para a passada por linha de D36 pesar, V2.43), referência de túnel
/// quando `tunel`.
fn dados_grandes(tipo: TipoEntrada, tunel: Option<&str>) -> RascunhoDados {
    const S: usize = 4 * 1024;
    match tipo {
        TipoEntrada::Login => RascunhoDados::Login {
            url: format!("https://cheio.invalid/{}", "a".repeat(MAX_URL - 22)),
            usuario: "u".repeat(MAX_LINHA),
            senha: seg(texto_dificil(S)),
            totp_semente: seg(vec![b'A'; MAX_TOTP]),
        },
        TipoEntrada::Banco => RascunhoDados::Banco {
            motor: t("redis"),
            host: "h".repeat(MAX_HOST),
            porta: t("1"),
            base: t("99999"),
            usuario: "u".repeat(MAX_LINHA),
            senha: seg(texto_dificil(S)),
            tunel: tunel.map(t).unwrap_or_default(),
        },
        TipoEntrada::Servidor => RascunhoDados::Servidor {
            host: "h".repeat(MAX_HOST),
            porta: t("65535"),
            usuario: "u".repeat(MAX_LINHA),
            senha: None,
            chave_privada: seg(chave_falsa(MAX_SEGREDO)),
            passphrase: seg(texto_dificil(S)),
            salto: String::new(),
        },
        TipoEntrada::Api => RascunhoDados::Api {
            url: format!("https://cheio.invalid/{}", "a".repeat(MAX_URL - 22)),
            token: seg(texto_dificil(S)),
            variavel: t("FALSO_TOKEN"),
        },
        TipoEntrada::Aws => RascunhoDados::Aws {
            access_key_id: "A".repeat(MAX_ACCESS_KEY),
            secret_access_key: seg(texto_dificil(S)),
            regiao: "r".repeat(MAX_REGIAO),
            role_arn: format!("arn:{}", "a".repeat(MAX_URL - 4)),
            session_token: seg(texto_dificil(S)),
        },
        TipoEntrada::Smtp => RascunhoDados::Smtp {
            host: "h".repeat(MAX_HOST),
            porta: t("1"),
            usuario: "u".repeat(MAX_LINHA),
            senha: seg(texto_dificil(S)),
            seguranca: t("tls"),
        },
        TipoEntrada::FtpSftp => RascunhoDados::FtpSftp {
            protocolo: t("sftp"),
            host: "h".repeat(MAX_HOST),
            porta: t("1"),
            usuario: "u".repeat(MAX_LINHA),
            senha: None,
            chave_privada: seg(chave_falsa(MAX_SEGREDO)),
            passphrase: seg(texto_dificil(S)),
        },
        TipoEntrada::NotaSegura => RascunhoDados::NotaSegura {
            texto: seg(vec![b'n'; MAX_NOTA]),
        },
        TipoEntrada::Generico => RascunhoDados::Generico,
    }
}

/// Extra secreto de várias linhas (D36): um JSON de conta de serviço com linhas
/// `"chave": "valor"`, `chave: valor` e `CHAVE=valor`, de `n` bytes. Cada linha e cada
/// valor viram padrões, com as formas escapadas de D37 (`$`, `&`, `\`, não ASCII) e as
/// formas de texto por linha de D54 (aspas simples, `printf %q`, PHP, campos de tab,
/// `chave : valor`) e os candidatos de D61 (pares JSON em qualquer posição da linha,
/// comentário no fim do YAML e do dotenv, chave YAML composta, `''`, Go com
/// `ensure_ascii`).
fn json_de_varias_linhas(n: usize) -> Vec<u8> {
    let mut v = b"{\n".to_vec();
    let mut i = 0usize;
    while v.len() + 80 < n {
        let linha = match i % 9 {
            0 => format!("  \"campo_{i:05}\": \"falso-valor-$&ç-{i:05}-0123456789\",\n"),
            1 => format!("  chave_{i:05}: falso-yaml-\\-{i:05}-0123456789\n"),
            2 => format!("  CHAVE_{i:05}=falso-env-`<>`-{i:05}-0123456789\n"),
            3 => format!("  \"x/{i:05}\":\"falso-'php'/ação-{i:05}\tcampo-{i:05}\",\n"),
            4 => format!("  chave_{i:05} : falso esp {i:05}\tcampo-de-tab-{i:05}\n"),
            5 => format!(
                "  \"a\": \"x\", \"p_{i:05}\": \"falso-par-\\u00e7-{i:05}\", \"b\": {{\"c\": \"falso-ani-&ç-{i:05}\"}},\n"
            ),
            6 => format!("  senha_{i:05}: falso-yaml-{i:05}-0123 # comentario {i:05}\n"),
            7 => format!("  private key {i:05}: 'falso-it''s-{i:05}-0123' # c\n"),
            _ => format!("  SENHA_{i:05}=\"falso-env-{i:05}-0123456789\" # dotenv\n"),
        };
        v.extend_from_slice(linha.as_bytes());
        i += 1;
    }
    v.extend_from_slice(b"}\n");
    v
}

/// Entrada do tipo com os campos grandes, anexos somando o limite de D29, um extra
/// secreto de várias linhas (D36) e `total` bytes de extras secretos (em extras de até
/// [`MAX_VALOR_EXTRA`], no máximo [`MAX_EXTRAS`]).
fn cheia(tipo: TipoEntrada, total: usize, tunel: Option<&str>) -> Rascunho {
    let mut r = rascunho(
        &format!("zz-{}", tipo.nome().replace('/', "-")),
        dados_grandes(tipo, tunel),
    );
    // Com a chave de 64 KiB (custo ~1,6 MiB na máscara), o `servidor` e o `ftp/sftp`
    // não cabem no orçamento com os anexos no limite de D29 (já era assim antes de D36:
    // a passada por linha da chave soma só ~64 KiB); a maior entrada aceita deles leva
    // metade dos anexos.
    let binario_max = if matches!(tipo, TipoEntrada::Servidor | TipoEntrada::FtpSftp) {
        MAX_ANEXOS_SOMADOS / 2
    } else {
        MAX_ANEXOS_SOMADOS - 4096
    };
    r.anexos = vec![
        RascunhoAnexo {
            nome: t("binario.bin"),
            conteudo: Segredo::de_bytes(binario(binario_max)),
        },
        RascunhoAnexo {
            nome: t("app.env"),
            conteudo: Segredo::de_bytes(
                b"FALSO_ENV=falso-anexo-01\nPORT=8080\n"
                    .iter()
                    .copied()
                    .cycle()
                    .take(4096)
                    .collect(),
            ),
        },
    ];
    r.extras.push(RascunhoExtra {
        nome: t("json"),
        valor: Segredo::de_bytes(json_de_varias_linhas(4 * 1024)),
        segredo: true,
    });
    let mut resto = total;
    let mut i = 1;
    while resto > 0 && i < MAX_EXTRAS {
        let n = resto.min(MAX_VALOR_EXTRA);
        r.extras.push(RascunhoExtra {
            nome: format!("e{i:02}"),
            valor: Segredo::de_bytes(texto_dificil(n)),
            segredo: true,
        });
        resto -= n;
        i += 1;
    }
    r
}

fn grande_demais(e: &ErrosValidacao) -> bool {
    e.tem_motivo("entrada", GRANDE_PARA_MASCARA)
}

/// O maior `total` de extras que a validação aceita (busca binária) e a entrada nele.
fn maior_aceita(tipo: TipoEntrada, tunel: Option<&str>) -> (usize, Entrada) {
    let teto = MAX_EXTRAS * MAX_VALOR_EXTRA;
    let mut lo = 0usize;
    let mut hi = teto;
    let aceita = |n: usize| match cheia(tipo, n, tunel).validar() {
        Ok(_) => true,
        Err(e) => {
            assert!(grande_demais(&e), "{tipo}: recusada por outro motivo: {e}");
            false
        }
    };
    assert!(aceita(0), "{tipo}: nem sem extras cabe");
    if aceita(hi) {
        lo = hi;
    } else {
        // Invariante: `lo` aceita, `hi` recusa.
        while hi - lo > 1 {
            let meio = lo + (hi - lo) / 2;
            if aceita(meio) {
                lo = meio;
            } else {
                hi = meio;
            }
        }
    }
    let e = cheia(tipo, lo, tunel).validar().unwrap();
    (lo, e)
}

/// Recursos falsos do `ward run` (os de verdade são do daemon).
struct Falsos;

impl Recursos for Falsos {
    fn codigo_totp(&mut self, _: &Segredo) -> Result<Zeroizing<String>, ErroRecurso> {
        Ok(Zeroizing::new(t("123456")))
    }
    fn arquivo(&mut self, nome: &str, _: &Segredo) -> Result<String, ErroRecurso> {
        Ok(format!("/run/user/1000/ward/tmp/1-{nome}"))
    }
    fn tunel(&mut self, _: &str, _: &str, _: u16) -> Result<u16, ErroRecurso> {
        Ok(40123)
    }
}

fn entrega_de(e: &Entrada) -> Entrega {
    entrega::montar(e, &mut Falsos).unwrap()
}

/// Soma real dos padrões da máscara do `ward run` (entradas e entregas).
fn custo_real(entradas: &[&Entrada], entregas: &[&Entrega]) -> usize {
    let mut c = ConstrutorMascara::new();
    for e in entradas {
        c = c.entrada(e);
    }
    for g in entregas {
        c = c.entrega(g);
    }
    mascara::somar(&c, usize::MAX).unwrap()
}

// ---- V2.35: uma fonte só ----

#[test]
fn v2_35_validacao_e_corpo_no_mesmo_limite_de_anexos() {
    let generico = |tamanhos: &[usize]| {
        let mut r = rascunho("anexos", RascunhoDados::Generico);
        r.extras.push(RascunhoExtra {
            nome: t("x"),
            valor: Segredo::de_texto(t("y")),
            segredo: false,
        });
        r.anexos = tamanhos
            .iter()
            .enumerate()
            .map(|(i, &n)| RascunhoAnexo {
                nome: format!("a{i}.bin"),
                conteudo: Segredo::de_bytes(vec![0x5a; n]),
            })
            .collect();
        r
    };
    // No limite: validação aceita, o corpo grava e lê.
    let no_limite = generico(&[MAX_ANEXOS_SOMADOS - 1, 1]).validar().unwrap();
    let corpo = serializar(cfg(), 1, core::slice::from_ref(&no_limite)).unwrap();
    assert_eq!(desserializar(&corpo).map(|(_, _, v)| v.len()), Ok(1));
    // Um byte acima: a validação recusa, e o corpo com esse byte a mais também.
    assert!(generico(&[MAX_ANEXOS_SOMADOS, 1]).validar().is_err());
    let mut forjado = corpo.to_vec();
    // O último campo do corpo é o conteúdo do 2º anexo (1 byte): cresce para 2.
    let n = forjado.len();
    forjado[n - 5..n - 1].copy_from_slice(&2u32.to_le_bytes());
    forjado.push(0x5a);
    assert_eq!(desserializar(&forjado).err(), Some(ErroCorpo::Invalido));
    // E o mesmo corpo sem o byte extra volta a abrir (o forjador está certo).
    forjado[n - 5..n - 1].copy_from_slice(&1u32.to_le_bytes());
    forjado.pop();
    assert_eq!(desserializar(&forjado).map(|(_, _, v)| v.len()), Ok(1));
}

#[test]
fn v2_35_orcamento_e_o_mesmo_na_validacao_e_na_mascara() {
    // A validação recusa exatamente quando o custo pela função do `Gerador` passa do
    // orçamento: perto da fronteira, nos dois lados.
    let (n, e) = maior_aceita(TipoEntrada::Api, None);
    let custo = mascara::custo_da_entrada(&e).unwrap();
    assert!(custo <= ORCAMENTO_MASCARA);
    let r = cheia(TipoEntrada::Api, n + 1, None);
    assert!(r.validar().is_err_and(|e| grande_demais(&e)));
    // O custo da validação é o da máscara montada com a entrega de verdade, ou mais (a
    // porta do túnel medida é a mais longa).
    let g = entrega_de(&e);
    assert!(custo_real(&[&e], &[&g]) <= custo);
}

// ---- V2.36: nenhuma entrada aceita fica sem `ward run` ----

/// V2.36 e V2.43 (D36, D37): com a passada por linha e as formas escapadas, a maior
/// entrada aceita de cada tipo com entrega (com a chave privada de 64 KiB em várias
/// linhas no `servidor` e no `ftp/sftp` e um extra secreto de várias linhas) ainda
/// monta a máscara, e o custo medido pela validação é ≥ o real.
#[test]
fn v2_36_maior_entrada_de_cada_tipo_monta_a_mascara() {
    for tipo in TipoEntrada::TODOS {
        if tipo == TipoEntrada::NotaSegura {
            continue;
        }
        let (_, e) = maior_aceita(tipo, None);
        // O extra de várias linhas está lá, e a chave no limite de D25 (V2.43).
        assert!(e.extras().iter().any(|x| x.nome() == "json"), "{tipo}");
        if matches!(tipo, TipoEntrada::Servidor | TipoEntrada::FtpSftp) {
            let chave = e
                .segredos()
                .into_iter()
                .map(|s| s.valor.expor().len())
                .max()
                .unwrap();
            assert!(chave >= MAX_SEGREDO - 64, "{tipo}: chave de {chave} bytes");
        }
        let g = entrega_de(&e);
        let custo = mascara::custo_da_entrada(&e).unwrap();
        assert!(custo <= ORCAMENTO_MASCARA, "{tipo}");
        let real = custo_real(&[&e], &[&g]);
        assert!(real <= custo, "{tipo}: {real} > {custo}");
        assert!(real <= LIMITE_PADROES, "{tipo}");
        mascara::Mascara::da_entrada(&e, Some(&g)).unwrap_or_else(|erro| panic!("{tipo}: {erro}"));
    }
}

#[test]
fn v2_36_par_banco_e_servidor_de_tunel_monta_a_mascara() {
    // As duas entradas de um uso combinado (D19), cada uma no maior tamanho aceito.
    let (_, servidor) = maior_aceita(TipoEntrada::Servidor, None);
    let (_, banco) = maior_aceita(TipoEntrada::Banco, Some(servidor.nome()));
    assert!(banco.conferir_no_cofre([&servidor]).is_ok());
    let g_banco = entrega_de(&banco);
    let g_servidor = entrega_de(&servidor);
    let real = custo_real(&[&banco, &servidor], &[&g_banco, &g_servidor]);
    assert!(real <= LIMITE_PADROES, "{real}");
    ConstrutorMascara::new()
        .entrada(&banco)
        .entrada(&servidor)
        .entrega(&g_banco)
        .entrega(&g_servidor)
        .construir()
        .expect("o par cabe na máscara");
    // E o mesmo vale para o salto (servidor + servidor de salto).
    let (_, salto) = maior_aceita(TipoEntrada::Servidor, None);
    let g_salto = entrega_de(&salto);
    assert!(custo_real(&[&servidor, &salto], &[&g_servidor, &g_salto]) <= LIMITE_PADROES);
}

#[test]
fn v2_36_custo_recusa_acima_do_orcamento_e_nota_fica_fora() {
    // Acima do orçamento: `GrandeDemais`, sem valor no erro.
    let mut r = cheia(TipoEntrada::Generico, 0, None);
    r.extras = (0..MAX_EXTRAS)
        .map(|i| RascunhoExtra {
            nome: format!("e{i:02}"),
            valor: Segredo::de_bytes(vec![b'k'; MAX_VALOR_EXTRA]),
            segredo: true,
        })
        .collect();
    let erros = r.validar().unwrap_err();
    assert!(grande_demais(&erros));
    assert!(!format!("{erros} {erros:?}").contains("kkkk"));
    // A nota segura não tem entrega: custo 0, mesmo cheia.
    let nota = cheia(TipoEntrada::NotaSegura, MAX_EXTRAS * MAX_VALOR_EXTRA, None)
        .validar()
        .unwrap();
    assert_eq!(mascara::custo_da_entrada(&nota), Ok(0));
    assert!(!format!("{}", ErroMascara::GrandeDemais).contains("falso"));
}

/// Texto de linhas `falso-linha-NNNNN-0123456789` separadas por `sep`, de `n` bytes.
fn linhas_com(sep: u8, n: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(n + 32);
    let mut i = 0usize;
    while v.len() < n {
        v.extend_from_slice(format!("falso-linha-{i:05}-0123456789").as_bytes());
        v.push(sep);
        i += 1;
    }
    v.truncate(n);
    v
}

/// O genérico cheio com um extra secreto de linhas separadas por `sep` e `total` bytes
/// de extras de preenchimento.
fn com_linhas(sep: u8, total: usize) -> Rascunho {
    let mut r = cheia(TipoEntrada::Generico, total, None);
    // Sem os anexos: o preenchimento (extras) é que leva a entrada ao orçamento.
    r.anexos.clear();
    r.extras.push(RascunhoExtra {
        nome: t("linhas"),
        valor: Segredo::de_bytes(linhas_com(sep, MAX_VALOR_EXTRA)),
        segredo: true,
    });
    r
}

/// V2.43 (D29 com D36): o orçamento conta a passada por linha. Tabulação e quebra de
/// linha custavam o mesmo antes de D36 (as duas viram 2 bytes no JSON, `%09`/`%0A` na
/// URL e o mesmo tamanho em base64); com D36 só a quebra gera os padrões por linha. No
/// maior preenchimento em que a versão com tabulação cabe, a versão com quebras de
/// linha passa do orçamento só por causa de D36 e é recusada com a mensagem de D29,
/// sem valor no erro.
#[test]
fn v2_43_entrada_que_so_passa_do_orcamento_por_d36_e_recusada() {
    let aceita = |n: usize| match com_linhas(b'\t', n).validar() {
        Ok(_) => true,
        Err(e) => {
            assert!(grande_demais(&e), "recusada por outro motivo: {e}");
            false
        }
    };
    let (mut lo, mut hi) = (0usize, MAX_VALOR_EXTRA * 40);
    assert!(aceita(lo) && !aceita(hi));
    while hi - lo > 1 {
        let meio = lo + (hi - lo) / 2;
        if aceita(meio) {
            lo = meio;
        } else {
            hi = meio;
        }
    }
    let tab = com_linhas(b'\t', lo).validar().unwrap();
    let custo_tab = mascara::custo_da_entrada(&tab).unwrap();
    assert!(custo_tab <= ORCAMENTO_MASCARA);
    let erros = com_linhas(b'\n', lo).validar().unwrap_err();
    assert!(grande_demais(&erros), "{erros}");
    let texto = format!("{erros} {erros:?}");
    assert!(!texto.contains("falso"), "erro com valor");
    assert!(texto.contains("entrada grande demais para a máscara do `ward run`"));
    // Com a mesma linha em pedaço menor, a versão com quebras cabe e o custo dela é ≥ o
    // real (o mesmo `Gerador`).
    let pequena = {
        let mut r = cheia(TipoEntrada::Generico, 0, None);
        r.extras.push(RascunhoExtra {
            nome: t("linhas"),
            valor: Segredo::de_bytes(linhas_com(b'\n', 16 * 1024)),
            segredo: true,
        });
        r.validar().unwrap()
    };
    let g = entrega_de(&pequena);
    let custo = mascara::custo_da_entrada(&pequena).unwrap();
    assert!(custo_real(&[&pequena], &[&g]) <= custo);
}
