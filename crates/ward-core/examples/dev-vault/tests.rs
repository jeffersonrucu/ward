//! Tests of `make dev-vault` (V2.29, S15): fixtures equal to the contract table, output
//! path only inside `.dev/`, input files only from the allowed folders, strict JSON and
//! errors without secrets. They run in `cargo test` (`test = true` in `Cargo.toml`); the
//! temporary directories stay in the system temporary directory, only with fake values,
//! and are deleted at the end.

use std::os::unix::fs::symlink;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::test_vault::{
    PIN_TESTE, SEGREDOS, chave_falsa, cofre_de_teste, configuracao_de_teste, configuracao_nova,
};
use super::*;

// --- Support ------------------------------------------------------------------------

/// Temporary directory of the test itself, deleted on exit.
struct Temp(PathBuf);

impl Temp {
    fn new(label: &str) -> Self {
        static N: AtomicUsize = AtomicUsize::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let p = env::temp_dir().join(format!(
            "ward-test-dev-vault-{}-{label}-{n}",
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

/// Fake repository root with `.dev/`.
fn fake_root(label: &str) -> Temp {
    let t = Temp::new(label);
    DirBuilder::new().mode(0o700).create(t.p(".dev")).unwrap();
    t
}

fn refuses_output(root: &Path, requested: &str) {
    match validate_output(root, OsStr::new(requested)) {
        Err(Error::Output(_)) => {}
        other => panic!("saída {requested:?} devia ser recusada: {other:?}"),
    }
}

fn error_text(e: &Error) -> String {
    format!("{e} {e:?}")
}

/// Reading of the fixture files for the tests: the fake key of the body test in place of
/// the key generated in `.dev/` (the test does not depend on `make dev-services`) and the
/// real attachment, through the same path as the program.
fn test_read(root: &Path) -> impl FnMut(Source, &str) -> Result<Segredo, Error> + '_ {
    move |source, path| match source {
        Source::Key => {
            assert_eq!(path, ".dev/services/keys/acme-deploy_ed25519");
            Ok(Segredo::de_texto(chave_falsa()))
        }
        Source::Attachment => read_from_repo(root, source, path),
    }
}

fn real_fixtures() -> (PathBuf, Zeroizing<Vec<u8>>) {
    let r = root().expect("raiz do repositório");
    let f = read_fixtures(&r).expect("fixtures");
    (r, f)
}

/// Minimal fixture of one entry, with the JSON of the entry given.
fn fixture_with(entry: &str) -> String {
    format!(
        r#"{{"versao": 1, "senha_mestra": "falso-mestra-ward-01", "pin": "135790",
            "entradas": [{entry}]}}"#
    )
}

fn build_text(json_text: &str) -> Result<Built, Error> {
    let mut read =
        |_: Source, _: &str| -> Result<Segredo, Error> { Ok(Segredo::de_texto(chave_falsa())) };
    build(json_text.as_bytes(), &mut read)
}

// --- V2.29: the fixtures are the contract table ------------------------------------

/// The 22 entries of the fixtures, built through the path of `make dev-vault`, give the
/// same canonical body as the test vault of Phase 15a (written from the contract table,
/// with the TOTP entry from D28 and the symbols entry from D59): same names, kinds,
/// fields, secrets and levels.
#[test]
fn v2_29_fixtures_equal_the_contract_table() {
    let (r, f) = real_fixtures();
    let mut read = test_read(&r);
    let (_password, pin, built) = build(&f, &mut read).expect("fixtures montam");
    assert_eq!(pin.as_str(), PIN_TESTE, "PIN de D15");
    assert_eq!(built.len(), 22);
    let totp = built
        .iter()
        .find(|e| e.nome() == "acme-totp-dev")
        .expect("acme-totp-dev (D28) nas fixtures");
    assert_eq!(totp.nivel(), NivelAcesso::Livre);
    assert_eq!(totp.tipo(), TipoEntrada::Login);
    let a = serializar(configuracao_de_teste(), 1, &built).unwrap();
    let b = serializar(configuracao_de_teste(), 1, &cofre_de_teste()).unwrap();
    assert!(*a == *b, "fixtures diferem da tabela do contrato");
}

/// The three levels of D15 come from the fixtures (S9: the assembly is not born all
/// "free").
#[test]
fn v2_29_levels_of_the_table() {
    let (r, f) = real_fixtures();
    let mut read = test_read(&r);
    let (_s, _pin, built) = build(&f, &mut read).unwrap();
    let level = |n: &str| built.iter().find(|e| e.nome() == n).unwrap().nivel();
    assert_eq!(level("gama-bloqueada"), NivelAcesso::Bloqueado);
    assert_eq!(level("acme-wp-dev"), NivelAcesso::ComAprovacao);
    assert_eq!(level("acme-pg-aprovacao"), NivelAcesso::ComAprovacao);
    assert_eq!(level("acme-pg-prod"), NivelAcesso::ComAprovacao);
    assert_eq!(level("acme-ssh-salto"), NivelAcesso::ComAprovacao);
    assert_eq!(level("acme-wp-prod"), NivelAcesso::Livre);
}

/// Round trip through the disk in a fake root: writes into `.dev/`, 0600, reopens and
/// verifies; a wrong password and a vault with other entries are refused; the file has
/// no secret in clear. Uses the real Argon2 parameters (a few seconds).
#[test]
fn v2_29_writes_reopens_and_verifies() {
    let (r, f) = real_fixtures();
    let mut read = test_read(&r);
    let (password, pin, built) = build(&f, &mut read).unwrap();
    let t = fake_root("ida-volta");
    let output = validate_output(&t.0, OsStr::new(DEFAULT_OUTPUT)).unwrap();
    let mut vault = OpenVault::new(&password, configuracao_nova()).unwrap();
    write_entries(&output, &mut vault, &built).unwrap();
    // Second write (V2.29 "runs twice"): replaces the file through the same path.
    write_entries(&output, &mut vault, &built).unwrap();
    // The second write of the same open vault raises the generation (D39).
    assert_eq!(verify(&output, &password, &pin, &built).unwrap(), (22, 2));
    let mode = fs::metadata(&output).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);

    let bytes = fs::read(&output).unwrap();
    for s in SEGREDOS.iter().chain([&PIN_TESTE, &"$argon2id$"]) {
        assert!(
            !bytes.windows(s.len()).any(|w| w == s.as_bytes()),
            "segredo em claro no arquivo"
        );
    }

    let wrong = MasterPassword::new("falso-mestra-errada".to_owned());
    let e = verify(&output, &wrong, &pin, &built).unwrap_err();
    assert!(error_text(&e).contains("não abriu"));

    // Fixture PIN different from the written hash: the verification reports it, without
    // the PIN in the error.
    let e = verify(&output, &password, "246802", &built).unwrap_err();
    let msg = error_text(&e);
    assert!(msg.contains("hash do PIN"));
    assert!(!msg.contains("246802") && !msg.contains(PIN_TESTE));

    // Configuration other than the default one (lock of 30 minutes) too.
    let mut other = OpenVault::new(&password, configuracao_nova()).unwrap();
    other.config_mut().definir_minutos_trava(30).unwrap();
    write_entries(&output, &mut other, &built).unwrap();
    let e = verify(&output, &password, &pin, &built).unwrap_err();
    assert!(error_text(&e).contains("não é a padrão"));
    write_entries(&output, &mut vault, &built).unwrap();

    // A vault with one entry less does not pass the verification.
    let fewer: Vec<Entrada> = cofre_de_teste()
        .into_iter()
        .filter(|e| e.nome() != "acme-totp-dev")
        .collect();
    write_entries(&output, &mut vault, &fewer).unwrap();
    let e = verify(&output, &password, &pin, &built).unwrap_err();
    assert!(error_text(&e).contains("diferem"));

    // A vault open to the group is refused by the verification.
    fs::set_permissions(&output, fs::Permissions::from_mode(0o640)).unwrap();
    let e = verify(&output, &password, &pin, &fewer).unwrap_err();
    assert!(error_text(&e).contains("grupo"));
}

// --- V2.29 (S15): output only inside .dev/ -------------------------------------------

#[test]
fn v2_29_output_inside_dev_accepted() {
    let t = fake_root("aceita");
    let expected = t.p(".dev/test-vault.ward");
    for requested in [
        ".dev/test-vault.ward",
        "./.dev/test-vault.ward",
        ".dev/./test-vault.ward",
        ".dev//test-vault.ward",
    ] {
        assert_eq!(
            validate_output(&t.0, OsStr::new(requested)).unwrap(),
            expected,
            "{requested}"
        );
    }
    let abs = expected.to_str().unwrap().to_owned();
    assert_eq!(validate_output(&t.0, OsStr::new(&abs)).unwrap(), expected);
    DirBuilder::new()
        .mode(0o700)
        .create(t.p(".dev/sub"))
        .unwrap();
    assert_eq!(
        validate_output(&t.0, OsStr::new(".dev/sub/outro.ward")).unwrap(),
        t.p(".dev/sub/outro.ward")
    );
    // A regular file that already exists (second run) is accepted.
    fs::write(&expected, b"x").unwrap();
    assert!(validate_output(&t.0, OsStr::new(".dev/test-vault.ward")).is_ok());
}

#[test]
fn attack_output_outside_dev_refused() {
    let t = fake_root("fora");
    let outside = Temp::new("fora-alvo");
    let target_outside = outside.p("roubado.ward");
    for requested in [
        target_outside.to_str().unwrap(),
        "/tmp/ward-test-dev-vault-outside.ward",
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
        refuses_output(&t.0, requested);
    }
    // An absolute path that only resembles the repository (another root with `.dev`).
    let other = fake_root("outra");
    refuses_output(&t.0, other.p(".dev/test-vault.ward").to_str().unwrap());
    assert!(!target_outside.exists());
    assert!(!other.p(".dev/test-vault.ward").exists());
}

#[test]
fn attack_symbolic_link_in_the_path_refused() {
    let t = fake_root("link");
    let outside = Temp::new("link-alvo");
    // A directory of .dev that points outside.
    symlink(&outside.0, t.p(".dev/escape")).unwrap();
    refuses_output(&t.0, ".dev/escape/cofre.ward");
    // A target that is a link to a file outside: the file outside does not change.
    fs::write(outside.p("vitima.ward"), b"original").unwrap();
    symlink(outside.p("vitima.ward"), t.p(".dev/test-vault.ward")).unwrap();
    refuses_output(&t.0, ".dev/test-vault.ward");
    // A broken link too (it would be created outside by whoever followed the link).
    symlink(outside.p("nao-existe.ward"), t.p(".dev/quebrado.ward")).unwrap();
    refuses_output(&t.0, ".dev/quebrado.ward");
    // A target that is a directory.
    DirBuilder::new()
        .mode(0o700)
        .create(t.p(".dev/dir.ward"))
        .unwrap();
    refuses_output(&t.0, ".dev/dir.ward");
    assert_eq!(fs::read(outside.p("vitima.ward")).unwrap(), b"original");
    assert!(!outside.p("nao-existe.ward").exists());
    assert!(!outside.p("cofre.ward").exists());
}

#[test]
fn attack_dev_that_is_a_link_refused() {
    let t = Temp::new("dev-link");
    let outside = Temp::new("dev-link-alvo");
    symlink(&outside.0, t.p(".dev")).unwrap();
    refuses_output(&t.0, ".dev/test-vault.ward");
    assert!(fs::read_dir(&outside.0).unwrap().next().is_none());
    // `.dev` that is a file.
    let t2 = Temp::new("dev-arquivo");
    fs::write(t2.p(".dev"), b"").unwrap();
    refuses_output(&t2.0, ".dev/test-vault.ward");
}

#[test]
fn missing_dev_is_created_0700() {
    let t = Temp::new("dev-ausente");
    let s = validate_output(&t.0, OsStr::new(DEFAULT_OUTPUT)).unwrap();
    assert_eq!(s, t.p(".dev/test-vault.ward"));
    let m = fs::symlink_metadata(t.p(".dev")).unwrap();
    assert!(m.file_type().is_dir());
    assert_eq!(m.permissions().mode() & 0o777, 0o700);
}

// --- Files named in the fixtures ----------------------------------------------------

#[test]
fn attack_input_file_outside_the_folder_refused() {
    let t = fake_root("entrada");
    fs::create_dir_all(t.p(".dev/services/keys")).unwrap();
    fs::create_dir_all(t.p("dev/fixtures/attachments")).unwrap();
    fs::write(t.p(".dev/services/keys/ok"), b"conteudo").unwrap();
    fs::write(t.p("dev/fixtures/attachments/ok.env"), b"A=1\n").unwrap();
    fs::write(t.p(".dev/outro"), b"x").unwrap();
    let outside = Temp::new("entrada-alvo");
    fs::write(outside.p("alvo"), b"de fora").unwrap();
    symlink(outside.p("alvo"), t.p(".dev/services/keys/link")).unwrap();
    symlink(&outside.0, t.p("dev/fixtures/attachments/dir")).unwrap();

    assert_eq!(
        read_from_repo(&t.0, Source::Key, ".dev/services/keys/ok")
            .unwrap()
            .expor(),
        b"conteudo"
    );
    assert!(read_from_repo(&t.0, Source::Attachment, "dev/fixtures/attachments/ok.env").is_ok());

    let target = outside.p("alvo");
    for (source, path) in [
        (Source::Key, target.to_str().unwrap()),
        (Source::Key, ".dev/services/keys/../../outro"),
        (Source::Key, ".dev/outro"),
        (Source::Key, "dev/fixtures/attachments/ok.env"),
        (Source::Key, ".dev/services/keys/link"),
        (Source::Key, ".dev/services/keys"),
        (Source::Key, ".dev/services/keys/nao-existe"),
        (Source::Attachment, ".dev/services/keys/ok"),
        (Source::Attachment, "dev/fixtures/attachments/dir/alvo"),
        (Source::Attachment, "dev/fixtures/entries.json"),
        (Source::Attachment, "/etc/hostname"),
    ] {
        assert!(
            matches!(read_from_repo(&t.0, source, path), Err(Error::File { .. })),
            "{source:?} {path} devia ser recusado"
        );
    }
    // A missing key asks for `make dev-services`.
    let e = read_from_repo(&t.0, Source::Key, ".dev/services/keys/nao-existe").unwrap_err();
    assert!(e.to_string().contains("make dev-services"));
}

#[test]
fn attack_input_file_too_large_refused() {
    let t = fake_root("grande");
    fs::create_dir_all(t.p(".dev/services/keys")).unwrap();
    let f = File::create(t.p(".dev/services/keys/grande")).unwrap();
    f.set_len(MAX_KEY + 1).unwrap();
    assert!(matches!(
        read_from_repo(&t.0, Source::Key, ".dev/services/keys/grande"),
        Err(Error::File {
            reason: "grande demais",
            ..
        })
    ));
}

// --- Strict JSON --------------------------------------------------------------------

#[test]
fn json_reads_what_the_fixtures_use() {
    let v = json(
        br#" {"a": [1, -2, 0, true, false, null], "b": "x\"\\\/\n\u00e9\ud83d\ude00", "c": {}} "#,
    )
    .unwrap();
    let Value::Object(fields) = v else {
        panic!("objeto")
    };
    assert_eq!(fields.len(), 3);
    match &fields[1].1 {
        Value::Text(t) => assert_eq!(t.as_str(), "x\"\\/\né😀"),
        other => panic!("{other:?}"),
    }
    match &fields[0].1 {
        Value::List(l) => assert!(matches!(l[1], Value::Integer(-2))),
        other => panic!("{other:?}"),
    }
}

#[test]
fn attack_malformed_json_refused() {
    let deep = format!("{}1{}", "[".repeat(40), "]".repeat(40));
    let cases: &[&[u8]] = &[
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
        deep.as_bytes(),
    ];
    for c in cases {
        assert!(
            matches!(json(c), Err(Error::Json { .. })),
            "devia recusar {:?}",
            String::from_utf8_lossy(c)
        );
    }
}

// --- Strict fixtures and errors without secrets ----------------------------------------

#[test]
fn attack_fixture_with_unknown_field_refused() {
    let base =
        r#""nome": "x-1", "tipo": "login", "grupo": "g", "ambiente": "dev", "nivel": "livre""#;
    for entry in [
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
        let e = build_text(&fixture_with(&entry)).expect_err("recusa");
        let t = error_text(&e);
        assert!(t.contains("campo desconhecido"), "{t}");
        assert!(!t.contains("falso-no-lugar-errado"), "valor no erro: {t}");
    }
    // Unknown field at the top.
    let e = build_text(
        r#"{"versao": 1, "senha_mestra": "falso-mestra-ward-01", "pin": "135790", "entradas": [], "extra": 1}"#,
    )
    .err()
    .unwrap();
    assert!(error_text(&e).contains("campo desconhecido"));
}

#[test]
fn attack_invalid_fixture_refused_without_value_in_the_error() {
    let secret = "falso-segredo-que-nao-aparece";
    let base = |kind: &str, level: &str, fields: &str, secrets: &str| {
        fixture_with(&format!(
            r#"{{"nome": "x-1", "tipo": "{kind}", "grupo": "g", "ambiente": "dev", "nivel": "{level}", "campos": {fields}, "segredos": {secrets}}}"#
        ))
    };
    let s = format!(r#"{{"senha": "{secret}"}}"#);
    for (j, reason) in [
        // Invalid URL: the validation of Phase 15 refuses it.
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
        let e = build_text(&j)
            .err()
            .unwrap_or_else(|| panic!("devia recusar: {reason}"));
        let t = error_text(&e);
        assert!(t.contains(reason), "{t}");
        assert!(!t.contains(secret), "segredo no erro: {t}");
    }
    // Version, password and PIN of the top.
    for top in [
        r#"{"versao": 2, "senha_mestra": "falso-mestra-ward-01", "pin": "135790", "entradas": []}"#,
        r#"{"versao": 1, "senha_mestra": "", "pin": "135790", "entradas": []}"#,
        r#"{"versao": 1, "senha_mestra": "falso-mestra-ward-01", "pin": "1357", "entradas": []}"#,
        r#"{"versao": 1, "senha_mestra": "falso-mestra-ward-01", "pin": "135790", "entradas": {}}"#,
    ] {
        let e = build_text(top).expect_err("recusa");
        let t = error_text(&e);
        assert!(
            !t.contains("falso-mestra-ward-01") && !t.contains("135790"),
            "{t}"
        );
    }
}

/// Two entries with the same name, or a tunnel to an entry that does not exist: the
/// assembly passes entry by entry, but the write refuses the vault (it would not reopen).
#[test]
fn attack_inconsistent_vault_is_not_written() {
    let login = r#"{"nome": "x-1", "tipo": "login", "grupo": "g", "ambiente": "dev", "nivel": "livre", "campos": {"url": "https://a.invalid", "usuario": "u"}, "segredos": {"senha": "falso-s-01"}}"#;
    let (_s, _p, repeated) = build_text(&fixture_with(&format!("{login}, {login}"))).unwrap();
    assert!(serializar(configuracao_de_teste(), 1, &repeated).is_err());
    let tunnel = r#"{"nome": "x-2", "tipo": "banco", "grupo": "g", "ambiente": "dev", "nivel": "livre", "campos": {"motor": "postgres", "host": "h", "porta": 1, "base": "b", "usuario": "u", "tunel": "nao-existe"}, "segredos": {"senha": "falso-s-01"}}"#;
    let (_s, _p, without_tunnel) = build_text(&fixture_with(tunnel)).unwrap();
    assert!(serializar(configuracao_de_teste(), 1, &without_tunnel).is_err());
}

#[test]
fn command_line_options() {
    let o = |a: &[&str]| read_options(a.iter().map(OsString::from));
    assert!(o(&[]).is_ok());
    assert!(o(&["--verify", "--password-stdin"]).is_ok());
    assert!(o(&["--output", ".dev/x.ward"]).unwrap().output.is_some());
    assert!(matches!(o(&["--password-stdin"]), Err(Error::Usage(_))));
    assert!(matches!(o(&["--output"]), Err(Error::Usage(_))));
    assert!(matches!(
        o(&["--output", "a.ward", "--output", "b.ward"]),
        Err(Error::Usage(_))
    ));
    // A password in argv does not exist as an option (old and new flag names).
    for flag in ["--password", "--senha"] {
        assert!(matches!(
            o(&[flag, "falso-mestra-ward-01"]),
            Err(Error::Usage(_))
        ));
    }
    // The old flags (until m2:Phase 43) are unknown, without alias (m2:PRD rule 12).
    for old in ["--conferir", "--senha-stdin", "--cruzado"] {
        assert!(matches!(o(&[old]), Err(Error::Usage(_))), "{old}");
    }
}

// --- V2.39 (D30): the configuration of body v2 in the generated vault -------------------

/// The vault generated through the path of `make dev-vault`, decrypted with the test
/// password, has the configuration section with a PHC hash `$argon2id$` (salt and
/// parameters), the lock at 240 minutes and the backup off; the test PIN does not appear
/// in clear neither in the file nor in the decrypted body (only the hash, which verifies
/// it).
#[test]
fn v2_39_generated_vault_has_the_configuration() {
    let (r, f) = real_fixtures();
    let mut read = test_read(&r);
    let (password, pin, built) = build(&f, &mut read).unwrap();
    let t = fake_root("configuracao");
    let output = validate_output(&t.0, OsStr::new(DEFAULT_OUTPUT)).unwrap();
    let hash = HashPin::criar(pin.as_bytes()).unwrap();
    let mut vault = OpenVault::new(&password, Configuracao::nova(hash)).unwrap();
    write_entries(&output, &mut vault, &built).unwrap();
    drop(vault);

    let bytes = fs::read(&output).unwrap();
    assert!(
        !bytes
            .windows(PIN_TESTE.len())
            .any(|w| w == PIN_TESTE.as_bytes())
    );
    let (vault, loaded) = abrir_entradas(&bytes, &password).unwrap();
    let cfg = vault.config();
    assert_eq!(loaded.len(), 22);
    // D39: a freshly created vault is at generation 1.
    assert_eq!(vault.generation(), 1);
    assert_eq!(cfg.minutos_trava(), 240);
    assert!(!cfg.backup().ligado() && cfg.backup().uuid().is_none());
    assert_eq!(cfg.hash_pin().parametros(), KDF_PIN);
    assert!(cfg.conferir_pin(PIN_TESTE.as_bytes()));
    assert!(!cfg.conferir_pin(b"135791"));

    // The decrypted body (through the public path: the canonical body of the entries read).
    let body = serializar(cfg, 1, &loaded).unwrap();
    assert!(
        !body
            .windows(PIN_TESTE.len())
            .any(|w| w == PIN_TESTE.as_bytes())
    );
    assert!(body[..2] == [4, 0], "corpo v4");
    assert!(body[2..10] == 1u64.to_le_bytes(), "geração 1");
    let prefix = b"$argon2id$v=19$m=65536,t=3,p=1$";
    assert!(
        body[14..14 + prefix.len()] == prefix[..],
        "hash PHC Argon2id"
    );
    assert!(verify(&output, &password, &pin, &built).is_ok());
}
