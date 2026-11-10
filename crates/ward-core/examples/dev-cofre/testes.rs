//! Testes do `make dev-vault` (V2.29, S15): fixtures iguais à tabela do contrato,
//! caminho de saída só dentro de `.dev/`, arquivos de entrada só das pastas permitidas,
//! JSON estrito e erros sem segredo. Rodam no `cargo test` (`test = true` no
//! `Cargo.toml`); os diretórios temporários ficam no diretório temporário do sistema,
//! só com valores falsos, e são apagados no fim.

use std::os::unix::fs::symlink;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::cofre_teste::{
    PIN_TESTE, SEGREDOS, chave_falsa, cofre_de_teste, configuracao_de_teste, configuracao_nova,
};
use super::*;

// --- Apoio --------------------------------------------------------------------------

/// Diretório temporário próprio do teste, apagado ao sair.
struct Temp(PathBuf);

impl Temp {
    fn novo(rotulo: &str) -> Self {
        static N: AtomicUsize = AtomicUsize::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let p = env::temp_dir().join(format!(
            "ward-teste-dev-cofre-{}-{rotulo}-{n}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        DirBuilder::new().mode(0o700).create(&p).unwrap();
        Temp(p.canonicalize().unwrap())
    }

    fn p(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Raiz falsa de repositório com `.dev/`.
fn raiz_falsa(rotulo: &str) -> Temp {
    let t = Temp::novo(rotulo);
    DirBuilder::new().mode(0o700).create(t.p(".dev")).unwrap();
    t
}

fn recusa_saida(raiz: &Path, pedido: &str) {
    match validar_saida(raiz, OsStr::new(pedido)) {
        Err(Erro::Saida(_)) => {}
        outro => panic!("saída {pedido:?} devia ser recusada: {outro:?}"),
    }
}

fn texto_do_erro(e: &Erro) -> String {
    format!("{e} {e:?}")
}

/// Leitura dos arquivos das fixtures para os testes: a chave falsa do teste do corpo no
/// lugar da chave gerada em `.dev/` (o teste não depende de `make dev-services`) e o
/// anexo de verdade, pelo mesmo caminho do programa.
fn ler_de_teste(raiz: &Path) -> impl FnMut(Origem, &str) -> Result<Segredo, Erro> + '_ {
    move |origem, caminho| match origem {
        Origem::Chave => {
            assert_eq!(caminho, ".dev/services/keys/acme-deploy_ed25519");
            Ok(Segredo::de_texto(chave_falsa()))
        }
        Origem::Anexo => ler_do_repositorio(raiz, origem, caminho),
    }
}

fn fixtures_reais() -> (PathBuf, Zeroizing<Vec<u8>>) {
    let r = raiz().expect("raiz do repositório");
    let f = ler_fixtures(&r).expect("fixtures");
    (r, f)
}

/// Fixture mínima de uma entrada, com o JSON da entrada dado.
fn fixture_com(entrada: &str) -> String {
    format!(
        r#"{{"versao": 1, "senha_mestra": "falso-mestra-ward-01", "pin": "135790",
            "entradas": [{entrada}]}}"#
    )
}

fn montar_texto(json_txt: &str) -> Result<Montado, Erro> {
    let mut ler =
        |_: Origem, _: &str| -> Result<Segredo, Erro> { Ok(Segredo::de_texto(chave_falsa())) };
    montar(json_txt.as_bytes(), &mut ler)
}

// --- V2.29: as fixtures são a tabela do contrato -----------------------------------

/// As 22 entradas das fixtures, montadas pelo caminho do `make dev-vault`, dão o mesmo
/// corpo canônico que o cofre de teste da Fase 15a (escrito a partir da tabela do
/// contrato, com `acme-totp-dev` de D28 e `delta-api-simbolos` de D59): mesmos nomes, tipos, campos, segredos e níveis.
#[test]
fn v2_29_fixtures_iguais_a_tabela_do_contrato() {
    let (r, f) = fixtures_reais();
    let mut ler = ler_de_teste(&r);
    let (_senha, pin, montadas) = montar(&f, &mut ler).expect("fixtures montam");
    assert_eq!(pin.as_str(), PIN_TESTE, "PIN de D15");
    assert_eq!(montadas.len(), 22);
    let totp = montadas
        .iter()
        .find(|e| e.nome() == "acme-totp-dev")
        .expect("acme-totp-dev (D28) nas fixtures");
    assert_eq!(totp.nivel(), NivelAcesso::Livre);
    assert_eq!(totp.tipo(), TipoEntrada::Login);
    let a = serializar(configuracao_de_teste(), 1, &montadas).unwrap();
    let b = serializar(configuracao_de_teste(), 1, &cofre_de_teste()).unwrap();
    assert!(*a == *b, "fixtures diferem da tabela do contrato");
}

/// Os três níveis de D15 vêm das fixtures (S9: a montagem não nasce tudo "livre").
#[test]
fn v2_29_niveis_da_tabela() {
    let (r, f) = fixtures_reais();
    let mut ler = ler_de_teste(&r);
    let (_s, _pin, montadas) = montar(&f, &mut ler).unwrap();
    let nivel = |n: &str| montadas.iter().find(|e| e.nome() == n).unwrap().nivel();
    assert_eq!(nivel("gama-bloqueada"), NivelAcesso::Bloqueado);
    assert_eq!(nivel("acme-wp-dev"), NivelAcesso::ComAprovacao);
    assert_eq!(nivel("acme-pg-aprovacao"), NivelAcesso::ComAprovacao);
    assert_eq!(nivel("acme-pg-prod"), NivelAcesso::ComAprovacao);
    assert_eq!(nivel("acme-ssh-salto"), NivelAcesso::ComAprovacao);
    assert_eq!(nivel("acme-wp-prod"), NivelAcesso::Livre);
}

/// Ida e volta pelo disco numa raiz falsa: grava em `.dev/`, 0600, reabre e confere;
/// senha errada e cofre com outras entradas são recusados; o arquivo não tem nenhum
/// segredo em claro. Usa os parâmetros reais do Argon2 (alguns segundos).
#[test]
fn v2_29_grava_reabre_e_confere() {
    let (r, f) = fixtures_reais();
    let mut ler = ler_de_teste(&r);
    let (senha, pin, montadas) = montar(&f, &mut ler).unwrap();
    let t = raiz_falsa("ida-volta");
    let saida = validar_saida(&t.0, OsStr::new(SAIDA_PADRAO)).unwrap();
    let mut cofre = CofreAberto::novo(&senha, configuracao_nova()).unwrap();
    gravar_entradas(&saida, &mut cofre, &montadas).unwrap();
    // Segunda gravação (V2.29 "roda duas vezes"): troca o arquivo pelo mesmo caminho.
    gravar_entradas(&saida, &mut cofre, &montadas).unwrap();
    // A segunda gravação do mesmo cofre aberto sobe a geração (D39).
    assert_eq!(conferir(&saida, &senha, &pin, &montadas).unwrap(), (22, 2));
    let modo = fs::metadata(&saida).unwrap().permissions().mode() & 0o777;
    assert_eq!(modo, 0o600);

    let bytes = fs::read(&saida).unwrap();
    for s in SEGREDOS.iter().chain([&PIN_TESTE, &"$argon2id$"]) {
        assert!(
            !bytes.windows(s.len()).any(|w| w == s.as_bytes()),
            "segredo em claro no arquivo"
        );
    }

    let errada = SenhaMestra::nova("falso-mestra-errada".to_owned());
    let e = conferir(&saida, &errada, &pin, &montadas).unwrap_err();
    assert!(texto_do_erro(&e).contains("não abriu"));

    // PIN da fixture diferente do hash gravado: a conferência acusa, sem o PIN no erro.
    let e = conferir(&saida, &senha, "246802", &montadas).unwrap_err();
    let msg = texto_do_erro(&e);
    assert!(msg.contains("hash do PIN"));
    assert!(!msg.contains("246802") && !msg.contains(PIN_TESTE));

    // Configuração fora da padrão (trava de 30 minutos) também.
    let mut outro = CofreAberto::novo(&senha, configuracao_nova()).unwrap();
    outro.configuracao_mut().definir_minutos_trava(30).unwrap();
    gravar_entradas(&saida, &mut outro, &montadas).unwrap();
    let e = conferir(&saida, &senha, &pin, &montadas).unwrap_err();
    assert!(texto_do_erro(&e).contains("não é a padrão"));
    gravar_entradas(&saida, &mut cofre, &montadas).unwrap();

    // Cofre com uma entrada a menos não passa na conferência.
    let menos: Vec<Entrada> = cofre_de_teste()
        .into_iter()
        .filter(|e| e.nome() != "acme-totp-dev")
        .collect();
    gravar_entradas(&saida, &mut cofre, &menos).unwrap();
    let e = conferir(&saida, &senha, &pin, &montadas).unwrap_err();
    assert!(texto_do_erro(&e).contains("diferem"));

    // Cofre aberto para o grupo é recusado na conferência.
    fs::set_permissions(&saida, fs::Permissions::from_mode(0o640)).unwrap();
    let e = conferir(&saida, &senha, &pin, &menos).unwrap_err();
    assert!(texto_do_erro(&e).contains("grupo"));
}

// --- V2.29 (S15): saída só dentro de .dev/ -------------------------------------------

#[test]
fn v2_29_saida_dentro_de_dev_aceita() {
    let t = raiz_falsa("aceita");
    let esperado = t.p(".dev/test-vault.ward");
    for pedido in [
        ".dev/test-vault.ward",
        "./.dev/test-vault.ward",
        ".dev/./test-vault.ward",
        ".dev//test-vault.ward",
    ] {
        assert_eq!(
            validar_saida(&t.0, OsStr::new(pedido)).unwrap(),
            esperado,
            "{pedido}"
        );
    }
    let abs = esperado.to_str().unwrap().to_owned();
    assert_eq!(validar_saida(&t.0, OsStr::new(&abs)).unwrap(), esperado);
    DirBuilder::new()
        .mode(0o700)
        .create(t.p(".dev/sub"))
        .unwrap();
    assert_eq!(
        validar_saida(&t.0, OsStr::new(".dev/sub/outro.ward")).unwrap(),
        t.p(".dev/sub/outro.ward")
    );
    // Arquivo comum já existente (segunda rodada) é aceito.
    fs::write(&esperado, b"x").unwrap();
    assert!(validar_saida(&t.0, OsStr::new(".dev/test-vault.ward")).is_ok());
}

#[test]
fn ataque_saida_fora_de_dev_recusada() {
    let t = raiz_falsa("fora");
    let fora = Temp::novo("fora-alvo");
    let alvo_fora = fora.p("roubado.ward");
    for pedido in [
        alvo_fora.to_str().unwrap(),
        "/tmp/ward-teste-dev-cofre-fora.ward",
        "test-vault.ward",
        "dev/test-vault.ward",
        ".dev/../test-vault.ward",
        ".dev/sub/../../test-vault.ward",
        "../test-vault.ward",
        ".devx/test-vault.ward",
        ".dev-mau/test-vault.ward",
        ".dev",
        ".dev/",
        ".dev/.ward",
        ".dev/.oculto.ward",
        ".dev/cofre.txt",
        ".dev/cofre.ward.tmp",
        ".dev/nao-existe/cofre.ward",
    ] {
        recusa_saida(&t.0, pedido);
    }
    // Caminho absoluto que só lembra o repositório (outra raiz com `.dev`).
    let outra = raiz_falsa("outra");
    recusa_saida(&t.0, outra.p(".dev/test-vault.ward").to_str().unwrap());
    assert!(!alvo_fora.exists());
    assert!(!outra.p(".dev/test-vault.ward").exists());
}

#[test]
fn ataque_link_simbolico_no_caminho_recusado() {
    let t = raiz_falsa("link");
    let fora = Temp::novo("link-alvo");
    // Diretório de .dev que aponta para fora.
    symlink(&fora.0, t.p(".dev/escape")).unwrap();
    recusa_saida(&t.0, ".dev/escape/cofre.ward");
    // Destino que é link para um arquivo de fora: o arquivo de fora não muda.
    fs::write(fora.p("vitima.ward"), b"original").unwrap();
    symlink(fora.p("vitima.ward"), t.p(".dev/test-vault.ward")).unwrap();
    recusa_saida(&t.0, ".dev/test-vault.ward");
    // Link quebrado também (seria criado fora por quem seguisse o link).
    symlink(fora.p("nao-existe.ward"), t.p(".dev/quebrado.ward")).unwrap();
    recusa_saida(&t.0, ".dev/quebrado.ward");
    // Destino que é diretório.
    DirBuilder::new()
        .mode(0o700)
        .create(t.p(".dev/dir.ward"))
        .unwrap();
    recusa_saida(&t.0, ".dev/dir.ward");
    assert_eq!(fs::read(fora.p("vitima.ward")).unwrap(), b"original");
    assert!(!fora.p("nao-existe.ward").exists());
    assert!(!fora.p("cofre.ward").exists());
}

#[test]
fn ataque_dev_que_e_link_recusado() {
    let t = Temp::novo("dev-link");
    let fora = Temp::novo("dev-link-alvo");
    symlink(&fora.0, t.p(".dev")).unwrap();
    recusa_saida(&t.0, ".dev/test-vault.ward");
    assert!(fs::read_dir(&fora.0).unwrap().next().is_none());
    // `.dev` que é arquivo.
    let t2 = Temp::novo("dev-arquivo");
    fs::write(t2.p(".dev"), b"").unwrap();
    recusa_saida(&t2.0, ".dev/test-vault.ward");
}

#[test]
fn dev_ausente_e_criado_0700() {
    let t = Temp::novo("dev-ausente");
    let s = validar_saida(&t.0, OsStr::new(SAIDA_PADRAO)).unwrap();
    assert_eq!(s, t.p(".dev/test-vault.ward"));
    let m = fs::symlink_metadata(t.p(".dev")).unwrap();
    assert!(m.file_type().is_dir());
    assert_eq!(m.permissions().mode() & 0o777, 0o700);
}

// --- Arquivos citados nas fixtures --------------------------------------------------

#[test]
fn ataque_arquivo_de_entrada_fora_da_pasta_recusado() {
    let t = raiz_falsa("entrada");
    fs::create_dir_all(t.p(".dev/services/keys")).unwrap();
    fs::create_dir_all(t.p("dev/fixtures/attachments")).unwrap();
    fs::write(t.p(".dev/services/keys/ok"), b"conteudo").unwrap();
    fs::write(t.p("dev/fixtures/attachments/ok.env"), b"A=1\n").unwrap();
    fs::write(t.p(".dev/outro"), b"x").unwrap();
    let fora = Temp::novo("entrada-alvo");
    fs::write(fora.p("alvo"), b"de fora").unwrap();
    symlink(fora.p("alvo"), t.p(".dev/services/keys/link")).unwrap();
    symlink(&fora.0, t.p("dev/fixtures/attachments/dir")).unwrap();

    assert_eq!(
        ler_do_repositorio(&t.0, Origem::Chave, ".dev/services/keys/ok")
            .unwrap()
            .expor(),
        b"conteudo"
    );
    assert!(ler_do_repositorio(&t.0, Origem::Anexo, "dev/fixtures/attachments/ok.env").is_ok());

    let alvo = fora.p("alvo");
    for (origem, caminho) in [
        (Origem::Chave, alvo.to_str().unwrap()),
        (Origem::Chave, ".dev/services/keys/../../outro"),
        (Origem::Chave, ".dev/outro"),
        (Origem::Chave, "dev/fixtures/attachments/ok.env"),
        (Origem::Chave, ".dev/services/keys/link"),
        (Origem::Chave, ".dev/services/keys"),
        (Origem::Chave, ".dev/services/keys/nao-existe"),
        (Origem::Anexo, ".dev/services/keys/ok"),
        (Origem::Anexo, "dev/fixtures/attachments/dir/alvo"),
        (Origem::Anexo, "dev/fixtures/entries.json"),
        (Origem::Anexo, "/etc/hostname"),
    ] {
        assert!(
            matches!(
                ler_do_repositorio(&t.0, origem, caminho),
                Err(Erro::Arquivo { .. })
            ),
            "{origem:?} {caminho} devia ser recusado"
        );
    }
    // Chave que falta pede `make dev-services`.
    let e = ler_do_repositorio(&t.0, Origem::Chave, ".dev/services/keys/nao-existe").unwrap_err();
    assert!(e.to_string().contains("make dev-services"));
}

#[test]
fn ataque_arquivo_de_entrada_grande_demais_recusado() {
    let t = raiz_falsa("grande");
    fs::create_dir_all(t.p(".dev/services/keys")).unwrap();
    let f = File::create(t.p(".dev/services/keys/grande")).unwrap();
    f.set_len(MAX_CHAVE + 1).unwrap();
    assert!(matches!(
        ler_do_repositorio(&t.0, Origem::Chave, ".dev/services/keys/grande"),
        Err(Erro::Arquivo {
            motivo: "grande demais",
            ..
        })
    ));
}

// --- JSON estrito -------------------------------------------------------------------

#[test]
fn json_le_o_que_as_fixtures_usam() {
    let v = json(
        br#" {"a": [1, -2, 0, true, false, null], "b": "x\"\\\/\n\u00e9\ud83d\ude00", "c": {}} "#,
    )
    .unwrap();
    let Valor::Objeto(campos) = v else {
        panic!("objeto")
    };
    assert_eq!(campos.len(), 3);
    match &campos[1].1 {
        Valor::Texto(t) => assert_eq!(t.as_str(), "x\"\\/\né😀"),
        outro => panic!("{outro:?}"),
    }
    match &campos[0].1 {
        Valor::Lista(l) => assert!(matches!(l[1], Valor::Inteiro(-2))),
        outro => panic!("{outro:?}"),
    }
}

#[test]
fn ataque_json_malformado_recusado() {
    let fundo = format!("{}1{}", "[".repeat(40), "]".repeat(40));
    let casos: &[&[u8]] = &[
        b"",
        b"{",
        br#"{"a": 1, "a": 2}"#,
        br#"{"a": 1} x"#,
        br#"{"a": 1}{}"#,
        br#"{"a": 1,}"#,
        br#"[1,]"#,
        br#"{"a": 1.5}"#,
        br#"{"a": 1e3}"#,
        br#"{"a": 01}"#,
        br#"{"a": -}"#,
        br#"{"a": 99999999999999999999}"#,
        br#"{"a": "\x"}"#,
        br#"{"a": "\u12"}"#,
        br#"{"a": "\ud800"}"#,
        br#"{"a": "\ud800\u0041"}"#,
        br#"{"a": "\udc00"}"#,
        b"{\"a\": \"linha\ncrua\"}",
        b"{\"a\": \"nulo\x00\"}",
        b"{\"a\": \"\xff\"}",
        br#"{"a": "sem fim}"#,
        br#"{a: 1}"#,
        br#"{"a" 1}"#,
        br#"{"a": tru}"#,
        br#"{"a": nul}"#,
        fundo.as_bytes(),
    ];
    for c in casos {
        assert!(
            matches!(json(c), Err(Erro::Json { .. })),
            "devia recusar {:?}",
            String::from_utf8_lossy(c)
        );
    }
}

// --- Fixtures estritas e erros sem segredo -------------------------------------------

#[test]
fn ataque_fixture_com_campo_desconhecido_recusada() {
    let base =
        r#""nome": "x-1", "tipo": "login", "grupo": "g", "ambiente": "dev", "nivel": "livre""#;
    for entrada in [
        format!(
            r#"{{{base}, "campos": {{"url": "http://a.invalid", "usuario": "u", "senha": "falso-no-lugar-errado"}}, "segredos": {{"senha": "falso-s-01"}}}}"#
        ),
        format!(
            r#"{{{base}, "campos": {{"url": "http://a.invalid", "usuario": "u"}}, "segredos": {{"senha": "falso-s-01", "token": "falso-no-lugar-errado"}}}}"#
        ),
        format!(
            r#"{{{base}, "campos": {{"url": "http://a.invalid", "usuario": "u"}}, "segredos": {{"senha": "falso-s-01"}}, "nivel_extra": "falso-no-lugar-errado"}}"#
        ),
    ] {
        let e = montar_texto(&fixture_com(&entrada)).expect_err("recusa");
        let t = texto_do_erro(&e);
        assert!(t.contains("campo desconhecido"), "{t}");
        assert!(!t.contains("falso-no-lugar-errado"), "valor no erro: {t}");
    }
    // Campo desconhecido no topo.
    let e = montar_texto(
        r#"{"versao": 1, "senha_mestra": "falso-mestra-ward-01", "pin": "135790", "entradas": [], "extra": 1}"#,
    )
    .err()
    .unwrap();
    assert!(texto_do_erro(&e).contains("campo desconhecido"));
}

#[test]
fn ataque_fixture_invalida_recusada_sem_valor_no_erro() {
    let segredo = "falso-segredo-que-nao-aparece";
    let base = |tipo: &str, nivel: &str, campos: &str, segredos: &str| {
        fixture_com(&format!(
            r#"{{"nome": "x-1", "tipo": "{tipo}", "grupo": "g", "ambiente": "dev", "nivel": "{nivel}", "campos": {campos}, "segredos": {segredos}}}"#
        ))
    };
    let s = format!(r#"{{"senha": "{segredo}"}}"#);
    for (j, motivo) in [
        // URL inválida: a validação da Fase 15 recusa.
        (
            base(
                "login",
                "livre",
                r#"{"url": "javascript:x", "usuario": "u"}"#,
                &s,
            ),
            "inválida",
        ),
        (base("root", "livre", r#"{}"#, &s), "tipo desconhecido"),
        (
            base(
                "login",
                "admin",
                r#"{"url": "http://a.invalid", "usuario": "u"}"#,
                &s,
            ),
            "nível desconhecido",
        ),
        (
            base(
                "banco",
                "livre",
                r#"{"motor": "postgres", "host": "-oProxyCommand=x", "porta": 1, "base": "b", "usuario": "u"}"#,
                &s,
            ),
            "inválida",
        ),
        (
            base(
                "servidor",
                "livre",
                r#"{"host": "h", "porta": 70000, "usuario": "u"}"#,
                &s,
            ),
            "inválida",
        ),
    ] {
        let e = montar_texto(&j)
            .err()
            .unwrap_or_else(|| panic!("devia recusar: {motivo}"));
        let t = texto_do_erro(&e);
        assert!(t.contains(motivo), "{t}");
        assert!(!t.contains(segredo), "segredo no erro: {t}");
    }
    // Versão, senha e PIN do topo.
    for topo in [
        r#"{"versao": 2, "senha_mestra": "falso-mestra-ward-01", "pin": "135790", "entradas": []}"#,
        r#"{"versao": 1, "senha_mestra": "", "pin": "135790", "entradas": []}"#,
        r#"{"versao": 1, "senha_mestra": "falso-mestra-ward-01", "pin": "1357", "entradas": []}"#,
        r#"{"versao": 1, "senha_mestra": "falso-mestra-ward-01", "pin": "135790", "entradas": {}}"#,
    ] {
        let e = montar_texto(topo).expect_err("recusa");
        let t = texto_do_erro(&e);
        assert!(
            !t.contains("falso-mestra-ward-01") && !t.contains("135790"),
            "{t}"
        );
    }
}

/// Duas entradas com o mesmo nome, ou túnel para entrada que não existe: a montagem
/// passa entrada por entrada, mas a gravação recusa o cofre (não reabriria).
#[test]
fn ataque_cofre_inconsistente_nao_grava() {
    let login = r#"{"nome": "x-1", "tipo": "login", "grupo": "g", "ambiente": "dev", "nivel": "livre", "campos": {"url": "https://a.invalid", "usuario": "u"}, "segredos": {"senha": "falso-s-01"}}"#;
    let (_s, _p, repetidas) = montar_texto(&fixture_com(&format!("{login}, {login}"))).unwrap();
    assert!(serializar(configuracao_de_teste(), 1, &repetidas).is_err());
    let tunel = r#"{"nome": "x-2", "tipo": "banco", "grupo": "g", "ambiente": "dev", "nivel": "livre", "campos": {"motor": "postgres", "host": "h", "porta": 1, "base": "b", "usuario": "u", "tunel": "nao-existe"}, "segredos": {"senha": "falso-s-01"}}"#;
    let (_s, _p, sem_tunel) = montar_texto(&fixture_com(tunel)).unwrap();
    assert!(serializar(configuracao_de_teste(), 1, &sem_tunel).is_err());
}

#[test]
fn opcoes_da_linha_de_comando() {
    let o = |a: &[&str]| ler_opcoes(a.iter().map(OsString::from));
    assert!(o(&[]).is_ok());
    assert!(o(&["--conferir", "--senha-stdin"]).is_ok());
    assert!(o(&["--output", ".dev/x.ward"]).unwrap().saida.is_some());
    assert!(matches!(o(&["--senha-stdin"]), Err(Erro::Uso(_))));
    assert!(matches!(o(&["--output"]), Err(Erro::Uso(_))));
    assert!(matches!(
        o(&["--output", "a.ward", "--output", "b.ward"]),
        Err(Erro::Uso(_))
    ));
    // Senha em argv não existe como opção.
    assert!(matches!(
        o(&["--senha", "falso-mestra-ward-01"]),
        Err(Erro::Uso(_))
    ));
}

// --- V2.39 (D30): a configuração do corpo v2 no cofre gerado --------------------------

/// O cofre gerado pelo caminho do `make dev-vault`, decifrado com a senha de teste, tem
/// a seção de configuração com um hash PHC `$argon2id$` (sal e parâmetros), a trava em
/// 240 minutos e o backup desligado; o PIN de teste não aparece em claro nem no arquivo
/// nem no corpo decifrado (só o hash, que o confere).
#[test]
fn v2_39_cofre_gerado_tem_a_configuracao() {
    let (r, f) = fixtures_reais();
    let mut ler = ler_de_teste(&r);
    let (senha, pin, montadas) = montar(&f, &mut ler).unwrap();
    let t = raiz_falsa("configuracao");
    let saida = validar_saida(&t.0, OsStr::new(SAIDA_PADRAO)).unwrap();
    let hash = HashPin::criar(pin.as_bytes()).unwrap();
    let mut cofre = CofreAberto::novo(&senha, Configuracao::nova(hash)).unwrap();
    gravar_entradas(&saida, &mut cofre, &montadas).unwrap();
    drop(cofre);

    let bytes = fs::read(&saida).unwrap();
    assert!(
        !bytes
            .windows(PIN_TESTE.len())
            .any(|w| w == PIN_TESTE.as_bytes())
    );
    let (cofre, lidas) = abrir_entradas(&bytes, &senha).unwrap();
    let cfg = cofre.configuracao();
    assert_eq!(lidas.len(), 22);
    // D39: o cofre recém-criado está na geração 1.
    assert_eq!(cofre.geracao(), 1);
    assert_eq!(cfg.minutos_trava(), 240);
    assert!(!cfg.backup().ligado() && cfg.backup().uuid().is_none());
    assert_eq!(cfg.hash_pin().parametros(), KDF_PIN);
    assert!(cfg.conferir_pin(PIN_TESTE.as_bytes()));
    assert!(!cfg.conferir_pin(b"135791"));

    // O corpo decifrado (pelo caminho público: o corpo canônico das entradas lidas).
    let corpo = serializar(cfg, 1, &lidas).unwrap();
    assert!(
        !corpo
            .windows(PIN_TESTE.len())
            .any(|w| w == PIN_TESTE.as_bytes())
    );
    assert!(corpo[..2] == [4, 0], "corpo v4");
    assert!(corpo[2..10] == 1u64.to_le_bytes(), "geração 1");
    let prefixo = b"$argon2id$v=19$m=65536,t=3,p=1$";
    assert!(
        corpo[14..14 + prefixo.len()] == prefixo[..],
        "hash PHC Argon2id"
    );
    assert!(conferir(&saida, &senha, &pin, &montadas).is_ok());
}
