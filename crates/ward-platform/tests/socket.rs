//! Diretório e socket do daemon (V3.1, V3.2; S5). Os ataques: diretório 0755,
//! diretório de outro dono, link simbólico no lugar do diretório, arquivo no lugar do
//! socket, segundo daemon no mesmo socket.

mod comum;

use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::Path;

use comum::DirTeste;
use ward_platform::socket::{ErroDiretorio, ErroEscuta, preparar_diretorio};

fn modo(p: &Path) -> u32 {
    std::fs::symlink_metadata(p).unwrap().mode() & 0o7777
}

#[test]
fn diretorio_novo_fica_0700_e_socket_0600_do_usuario() {
    let d = DirTeste::novo();
    let dir = preparar_diretorio(&d.caminho).unwrap();
    assert_eq!(modo(&d.caminho), 0o700);
    let escuta = dir.escutar("wardd.sock").unwrap();
    let m = std::fs::symlink_metadata(escuta.caminho()).unwrap();
    assert!(m.file_type().is_socket());
    assert_eq!(m.mode() & 0o7777, 0o600);
    assert_eq!(m.uid(), ward_platform::effective_uid());
    assert_eq!(
        std::fs::symlink_metadata(&d.caminho).unwrap().uid(),
        ward_platform::effective_uid()
    );
    // Preparar de novo um 0700 nosso é aceito.
    preparar_diretorio(&d.caminho).unwrap();
}

#[test]
fn ataque_diretorio_existente_0755_nao_sobe_nem_conserta() {
    let d = DirTeste::novo();
    std::fs::create_dir(&d.caminho).unwrap();
    std::fs::set_permissions(&d.caminho, std::fs::Permissions::from_mode(0o755)).unwrap();
    let r = preparar_diretorio(&d.caminho);
    assert!(
        matches!(r, Err(ErroDiretorio::PermissaoErrada { modo: 0o755 })),
        "{r:?}"
    );
    // Não mexeu na permissão nem criou socket.
    assert_eq!(modo(&d.caminho), 0o755);
    assert_eq!(std::fs::read_dir(&d.caminho).unwrap().count(), 0);

    for m in [0o750, 0o701, 0o777, 0o500] {
        std::fs::set_permissions(&d.caminho, std::fs::Permissions::from_mode(m)).unwrap();
        assert!(
            matches!(
                preparar_diretorio(&d.caminho),
                Err(ErroDiretorio::PermissaoErrada { .. })
            ),
            "{m:o}"
        );
    }
    std::fs::set_permissions(&d.caminho, std::fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn ataque_diretorio_de_outro_dono_nao_sobe() {
    // `/run/user` é do root (0755): serve de "diretório de outro dono" sem precisar de
    // root para montar o cenário. Nada é criado nem alterado nele.
    let r = preparar_diretorio(Path::new("/run/user"));
    assert!(
        matches!(r, Err(ErroDiretorio::DonoErrado { uid: 0 })),
        "{r:?}"
    );
}

#[test]
fn ataque_link_simbolico_no_lugar_do_diretorio() {
    let alvo = DirTeste::novo();
    std::fs::create_dir(&alvo.caminho).unwrap();
    std::fs::set_permissions(&alvo.caminho, std::fs::Permissions::from_mode(0o700)).unwrap();
    let link = DirTeste::novo();
    std::os::unix::fs::symlink(&alvo.caminho, &link.caminho).unwrap();
    let r = preparar_diretorio(&link.caminho);
    assert!(matches!(r, Err(ErroDiretorio::NaoEDiretorio)), "{r:?}");
    // `remove_dir_all` do link apaga só o link.
    std::fs::remove_file(&link.caminho).unwrap();
}

#[test]
fn ataque_arquivo_no_lugar_do_diretorio() {
    let d = DirTeste::novo();
    std::fs::write(&d.caminho, b"").unwrap();
    let r = preparar_diretorio(&d.caminho);
    assert!(matches!(r, Err(ErroDiretorio::NaoEDiretorio)), "{r:?}");
    std::fs::remove_file(&d.caminho).unwrap();
}

#[test]
fn ataque_pasta_acima_gravavel_por_grupo() {
    let pai = DirTeste::novo();
    std::fs::create_dir(&pai.caminho).unwrap();
    std::fs::set_permissions(&pai.caminho, std::fs::Permissions::from_mode(0o770)).unwrap();
    let r = preparar_diretorio(&pai.caminho.join("ward"));
    assert!(matches!(r, Err(ErroDiretorio::PaiNaoConfiavel(_))), "{r:?}");
    assert!(!pai.caminho.join("ward").exists());
}

#[test]
fn ataque_arquivo_ou_link_no_lugar_do_socket() {
    let d = DirTeste::novo();
    let dir = preparar_diretorio(&d.caminho).unwrap();
    std::fs::write(d.caminho.join("s.sock"), b"isca").unwrap();
    assert!(matches!(
        dir.escutar("s.sock"),
        Err(ErroEscuta::CaminhoOcupado)
    ));
    // O arquivo de isca continua lá: nada é apagado sem ser um socket nosso.
    assert_eq!(std::fs::read(d.caminho.join("s.sock")).unwrap(), b"isca");

    std::os::unix::fs::symlink("/run/user", d.caminho.join("l.sock")).unwrap();
    assert!(matches!(
        dir.escutar("l.sock"),
        Err(ErroEscuta::CaminhoOcupado)
    ));

    for ruim in ["", ".", "..", "a/b", "../fora.sock"] {
        assert!(
            matches!(dir.escutar(ruim), Err(ErroEscuta::NomeInvalido)),
            "{ruim}"
        );
    }
}

#[test]
fn segundo_daemon_no_mesmo_socket_recusa_e_socket_velho_e_trocado() {
    let d = DirTeste::novo();
    let dir = preparar_diretorio(&d.caminho).unwrap();
    let primeiro = dir.escutar("wardd.sock").unwrap();
    assert!(matches!(
        dir.escutar("wardd.sock"),
        Err(ErroEscuta::JaEmUso)
    ));

    // Socket velho, sem ninguém escutando: o `std` não apaga no drop do listener; a
    // `Escuta` apaga. Simula um daemon que morreu sem apagar.
    let caminho = primeiro.caminho().to_path_buf();
    let velho = std::os::unix::net::UnixListener::bind(d.caminho.join("velho.sock")).unwrap();
    drop(velho);
    assert!(d.caminho.join("velho.sock").exists());
    let novo = dir.escutar("velho.sock").unwrap();
    assert!(UnixStream::connect(novo.caminho()).is_ok());

    drop(primeiro);
    assert!(!caminho.exists(), "a Escuta apaga o socket ao sair");
}

#[test]
fn diretorio_trocado_depois_de_preparar_recusa_escutar() {
    let d = DirTeste::novo();
    let dir = preparar_diretorio(&d.caminho).unwrap();
    // Troca o diretório por outro (também 0700 e nosso) com o mesmo nome.
    let outro = DirTeste::novo();
    std::fs::rename(&d.caminho, &outro.caminho).unwrap();
    std::fs::create_dir(&d.caminho).unwrap();
    std::fs::set_permissions(&d.caminho, std::fs::Permissions::from_mode(0o700)).unwrap();
    let r = dir.escutar("wardd.sock");
    assert!(
        matches!(r, Err(ErroEscuta::Diretorio(ErroDiretorio::Trocado))),
        "{r:?}"
    );
}

#[test]
fn aceitar_traz_pid_e_uid_do_par() {
    let d = DirTeste::novo();
    let dir = preparar_diretorio(&d.caminho).unwrap();
    let escuta = dir.escutar("wardd.sock").unwrap();
    let _cliente = UnixStream::connect(escuta.caminho()).unwrap();
    let conexao = escuta.aceitar().unwrap();
    let cred = conexao.par.credentials();
    assert_eq!(cred.pid, std::process::id());
    assert_eq!(cred.uid, ward_platform::effective_uid());
    // O próprio binário de teste (em target/, do usuário) não é binário instalado.
    let eu = std::env::current_exe().unwrap();
    assert!(conexao.par.identify(&comum::permitido(&eu)).is_err());
}
