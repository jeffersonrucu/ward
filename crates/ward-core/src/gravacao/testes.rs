//! Testes da gravação atômica (V2.9–V2.11, S7, S2).
//!
//! Os testes de morte rodam a gravação num processo filho: o próprio binário de teste,
//! chamado de novo só com o teste ignorado [`filho_gravacao`] e o modo no ambiente. O
//! pai mata o filho com SIGKILL (`Child::kill`) e confere o destino.
//!
//! Arquivos só em `target/<perfil>/ward-teste-gravacao/` (dentro do repositório, fora
//! do git). Senha e conteúdo falsos (D15); a marca "em claro" de cada teste é aleatória.

use std::io::{BufRead, BufReader, Read as _};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::*;
use crate::configuracao::{Configuracao, HashPin};
use crate::formato::{SenhaMestra, abrir};

const SENHA: &str = "falso-mestra-ward-01";
const NOME_FILHO: &str = "gravacao::testes::filho_gravacao";
const ENV_MODO: &str = "WARD_TESTE_GRAVACAO_MODO";
const ENV_DIR: &str = "WARD_TESTE_GRAVACAO_DIR";
const ENV_MARCA: &str = "WARD_TESTE_GRAVACAO_MARCA";
const ENV_CONTROLE: &str = "WARD_TESTE_GRAVACAO_CONTROLE";
const STRACE: &str = "/usr/bin/strace";
/// Tempo máximo de vida do filho: se o pai morrer sem matá-lo (teste interrompido), o
/// filho sai sozinho em vez de ficar gravando ou dormindo para sempre.
const VIDA_FILHO: Duration = Duration::from_secs(90);
/// Segredo falso que vai em claro no conteúdo de teste (D15).
const SEGREDO_FALSO: &str = "falso-PG:s3nha/01@#";
/// Maior que um pedaço de escrita, para haver ponto no meio da escrita.
const TAM_GRANDE: usize = PEDACO * 5 / 2;

/// Um Argon2id real (256 MiB) por vez nestes testes.
static ARGON: Mutex<()> = Mutex::new(());

fn senha() -> SenhaMestra {
    SenhaMestra::nova(SENHA.to_owned())
}

/// Configuração do cofre de teste (hash do PIN falso de D15).
fn configuracao() -> Configuracao {
    Configuracao::nova(HashPin::criar(b"135790").unwrap())
}

/// Cofre aberto compartilhado (um Argon2id só para todos os testes daqui).
fn cofre() -> &'static CofreAberto {
    static COFRE: OnceLock<CofreAberto> = OnceLock::new();
    COFRE.get_or_init(|| {
        let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
        CofreAberto::novo(&senha(), configuracao()).unwrap()
    })
}

fn abrir_conteudo(arquivo: &[u8]) -> Vec<u8> {
    let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
    abrir(arquivo, &senha()).unwrap().1.to_vec()
}

fn hex_aleatorio() -> String {
    let mut b = [0u8; 12];
    cripto::aleatorio(&mut b).unwrap();
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn u64_aleatorio() -> u64 {
    let mut b = [0u8; 8];
    cripto::aleatorio(&mut b).unwrap();
    u64::from_le_bytes(b)
}

/// Conteúdo falso em claro com a marca, do tamanho pedido (aproximado para cima).
fn conteudo(marca: &str, versao: usize, tam: usize) -> Vec<u8> {
    let linha = format!(
        "{{\"versao\":{versao},\"nome\":\"acme-pg-dev\",\"senha\":\"{SEGREDO_FALSO}\",\"marca\":\"{marca}\"}}\n"
    );
    let mut c = Vec::with_capacity(tam + linha.len());
    while c.len() < tam {
        c.extend_from_slice(linha.as_bytes());
    }
    c
}

fn contem(palheiro: &[u8], agulha: &[u8]) -> bool {
    palheiro.windows(agulha.len()).any(|j| j == agulha)
}

/// Diretório de teste novo em `target/<perfil>/ward-teste-gravacao/`, 0700, apagado no
/// fim do teste.
struct Dir(PathBuf);

impl Dir {
    fn novo(nome: &str) -> Self {
        let exe = std::env::current_exe().unwrap();
        // target/<perfil>/deps/<binário>
        let base = exe
            .parent()
            .and_then(Path::parent)
            .unwrap()
            .join("ward-teste-gravacao");
        fs::create_dir_all(&base).unwrap();
        let d = base.join(format!("{nome}-{}-{}", std::process::id(), hex_aleatorio()));
        fs::create_dir(&d).unwrap();
        fs::set_permissions(&d, fs::Permissions::from_mode(0o700)).unwrap();
        Self(d)
    }

    fn cofre(&self) -> PathBuf {
        self.0.join("cofre.ward")
    }

    /// Nomes no diretório que são temporários do cofre.
    fn temporarios(&self) -> Vec<PathBuf> {
        let mut v: Vec<_> = fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".cofre.ward.ward-tmp-")
            })
            .collect();
        v.sort();
        v
    }

    fn nomes(&self) -> Vec<String> {
        let mut v: Vec<_> = fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn modo(p: &Path) -> u32 {
    fs::symlink_metadata(p).unwrap().permissions().mode() & 0o7777
}

// --- Caminho feliz e recusas ------------------------------------------------------------

#[test]
fn ida_e_volta_modo_0600_sem_temporario() {
    let d = Dir::novo("ida");
    let marca = hex_aleatorio();
    let c = conteudo(&marca, 1, 4096);
    gravar_cofre(&d.cofre(), cofre(), &c).unwrap();
    let bytes = fs::read(d.cofre()).unwrap();
    assert!(
        !contem(&bytes, marca.as_bytes()),
        "conteúdo em claro no arquivo"
    );
    assert_eq!(abrir_conteudo(&bytes), c);
    assert_eq!(modo(&d.cofre()), 0o600);
    assert_eq!(d.nomes(), vec!["cofre.ward"]);
}

#[test]
fn substitui_destino_existente_e_fixa_0600() {
    let d = Dir::novo("substitui");
    let v1 = cofre().selar(b"v1").unwrap();
    let v2 = cofre().selar(b"v2").unwrap();
    fs::write(d.cofre(), &v1).unwrap();
    fs::set_permissions(d.cofre(), fs::Permissions::from_mode(0o644)).unwrap();
    gravar(&d.cofre(), &v2).unwrap();
    assert_eq!(fs::read(d.cofre()).unwrap(), v2);
    assert_eq!(modo(&d.cofre()), 0o600);
    assert_eq!(d.nomes(), vec!["cofre.ward"]);
}

#[test]
fn caminho_relativo_sem_diretorio() {
    // `separar` usa `.` como diretório; aqui só a decomposição, sem mudar o cwd.
    let (dir, nome) = separar(Path::new("cofre.ward")).unwrap();
    assert_eq!(dir, PathBuf::from("."));
    assert_eq!(nome, OsString::from("cofre.ward"));
}

/// Ataque (S2): um chamador tenta gravar o conteúdo em claro (ou um cofre truncado, ou
/// lixo) pelo módulo de gravação. Nada é criado e o destino fica intacto.
#[test]
fn recusa_gravar_bytes_que_nao_sao_cofre() {
    let d = Dir::novo("recusa");
    let anterior = cofre().selar(b"anterior").unwrap();
    fs::write(d.cofre(), &anterior).unwrap();
    let marca = hex_aleatorio();
    let claro = conteudo(&marca, 1, 2048);
    let mut truncado = cofre().selar(b"x").unwrap();
    truncado.pop();
    let mut sem_magia = cofre().selar(b"x").unwrap();
    sem_magia[0] ^= 1;
    for ruim in [
        &claro[..],
        &truncado[..],
        &sem_magia[..],
        &b""[..],
        &[0u8; 4096][..],
    ] {
        assert_eq!(gravar(&d.cofre(), ruim), Err(ErroGravacao::NaoECofre));
        assert_eq!(fs::read(d.cofre()).unwrap(), anterior);
        assert_eq!(d.nomes(), vec!["cofre.ward"], "nenhum temporário criado");
    }
}

#[test]
fn destino_invalido() {
    let v = cofre().selar(b"x").unwrap();
    for p in ["/", "", "..", "/tmp/.."] {
        assert_eq!(
            gravar(Path::new(p), &v),
            Err(ErroGravacao::DestinoInvalido),
            "{p:?}"
        );
    }
}

#[test]
fn destino_diretorio_falha_sem_deixar_temporario() {
    let d = Dir::novo("destino-dir");
    fs::create_dir(d.cofre()).unwrap();
    fs::write(d.cofre().join("dentro"), b"intacto").unwrap();
    let v = cofre().selar(b"x").unwrap();
    let e = gravar(&d.cofre(), &v).unwrap_err();
    assert!(
        matches!(
            e,
            ErroGravacao::Io {
                etapa: Etapa::Renomear,
                ..
            }
        ),
        "{e:?}"
    );
    assert_eq!(d.nomes(), vec!["cofre.ward"]);
    assert_eq!(fs::read(d.cofre().join("dentro")).unwrap(), b"intacto");
}

/// Ataque: o destino é um link simbólico para um arquivo da vítima. O rename troca o
/// próprio link pelo cofre novo (não segue o link) e a vítima fica intacta.
#[test]
fn destino_link_simbolico_nao_e_seguido() {
    let d = Dir::novo("destino-link");
    let vitima = d.0.join("vitima");
    fs::write(&vitima, b"original").unwrap();
    symlink(&vitima, d.cofre()).unwrap();
    let v = cofre().selar(b"x").unwrap();
    gravar(&d.cofre(), &v).unwrap();
    assert_eq!(fs::read(&vitima).unwrap(), b"original");
    let meta = fs::symlink_metadata(d.cofre()).unwrap();
    assert!(meta.file_type().is_file(), "o destino continuou link");
    assert_eq!(fs::read(d.cofre()).unwrap(), v);
    assert_eq!(modo(&d.cofre()), 0o600);
}

/// Ataque: um link simbólico (ou arquivo) plantado com o nome do temporário apontando
/// para um arquivo da vítima. A criação recusa (`O_EXCL`) e a vítima fica intacta.
#[test]
fn temporario_nao_segue_link_simbolico_nem_reaproveita_arquivo() {
    let d = Dir::novo("link");
    let vitima = d.0.join("vitima");
    fs::write(&vitima, b"original").unwrap();
    let plantado = d.0.join(".cofre.ward.ward-tmp-plantado");
    symlink(&vitima, &plantado).unwrap();
    let e = criar_temporario(&plantado).unwrap_err();
    assert_eq!(e.kind(), io::ErrorKind::AlreadyExists);
    let ausente = d.0.join("nao-existe");
    let pendente = d.0.join(".cofre.ward.ward-tmp-pendente");
    symlink(&ausente, &pendente).unwrap();
    assert_eq!(
        criar_temporario(&pendente).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert!(!ausente.exists(), "seguiu o link pendente");
    assert_eq!(
        criar_temporario(&vitima).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(fs::read(&vitima).unwrap(), b"original");
}

#[test]
fn temporario_nasce_0600_com_umask_qualquer() {
    let d = Dir::novo("modo");
    let p = d.0.join(".cofre.ward.ward-tmp-modo");
    drop(criar_temporario(&p).unwrap());
    assert_eq!(modo(&p), 0o600);
}

#[test]
fn nomes_de_temporario_nao_repetem() {
    // Nome aleatório: dois temporários vivos ao mesmo tempo têm nomes diferentes.
    let d = Dir::novo("nomes");
    let v = cofre().selar(b"x").unwrap();
    let mut vistos = Vec::new();
    for _ in 0..20 {
        let r = gravar_com_pontos(&d.cofre(), &v, &mut |p| {
            if p == Ponto::DepoisDeCriar {
                vistos.extend(d.temporarios());
                return Err(io::Error::other("parar"));
            }
            Ok(())
        });
        assert!(r.is_err());
    }
    vistos.sort();
    vistos.dedup();
    assert_eq!(vistos.len(), 20);
}

/// Ataque: link simbólico com o prefixo dos temporários apontando para um arquivo da
/// vítima. A limpeza apaga só os temporários comuns do próprio cofre.
#[test]
fn limpar_temporarios_so_apaga_os_do_cofre() {
    let d = Dir::novo("limpar");
    let vitima = d.0.join("vitima");
    fs::write(&vitima, b"original").unwrap();
    fs::write(d.0.join(".cofre.ward.ward-tmp-aaaa"), b"lixo").unwrap();
    fs::write(d.0.join(".cofre.ward.ward-tmp-bbbb"), b"lixo").unwrap();
    symlink(&vitima, d.0.join(".cofre.ward.ward-tmp-link")).unwrap();
    fs::create_dir(d.0.join(".cofre.ward.ward-tmp-dir")).unwrap();
    fs::write(d.0.join(".outro.ward.ward-tmp-cccc"), b"de outro").unwrap();
    fs::write(d.0.join("cofre.ward"), b"cofre").unwrap();
    assert_eq!(limpar_temporarios(&d.cofre()).unwrap(), 2);
    assert_eq!(
        d.nomes(),
        vec![
            ".cofre.ward.ward-tmp-dir",
            ".cofre.ward.ward-tmp-link",
            ".outro.ward.ward-tmp-cccc",
            "cofre.ward",
            "vitima"
        ]
    );
    assert_eq!(fs::read(&vitima).unwrap(), b"original");
}

#[test]
fn erro_nao_carrega_conteudo() {
    let e = ErroGravacao::Io {
        etapa: Etapa::FsyncTemporario,
        tipo: io::ErrorKind::StorageFull,
        identidade: None,
    };
    let texto = format!("{e} {e:?}");
    assert!(texto.contains("sincronizar"));
    assert_eq!(
        ErroGravacao::NaoECofre.to_string(),
        "recusado gravar: os dados não são um cofre cifrado"
    );
}

// --- Falha injetada entre escrita, fsync e rename (V2.9) --------------------------------

/// Pontos na ordem da gravação; os quatro primeiros vêm antes do rename.
const PONTOS: [Ponto; 6] = [
    Ponto::DepoisDeCriar,
    Ponto::Escrevendo(PEDACO),
    Ponto::DepoisDeEscrever,
    Ponto::DepoisDoFsync,
    Ponto::DepoisDoRename,
    Ponto::DepoisDoFsyncDir,
];
const ANTES_DO_RENAME: usize = 4;

fn etapa_esperada(i: usize) -> Etapa {
    match PONTOS[i] {
        Ponto::DepoisDeCriar => Etapa::CriarTemporario,
        Ponto::Escrevendo(_) | Ponto::DepoisDeEscrever => Etapa::Escrever,
        Ponto::DepoisDoFsync => Etapa::FsyncTemporario,
        Ponto::DepoisDoRename | Ponto::DepoisDoFsyncDir => Etapa::FsyncDiretorio,
    }
}

/// O erro de E/S injetado no ponto `i`: etapa e tipo certos; depois do rename, a
/// identidade (D62) é a do arquivo que ficou no lugar; antes, nenhuma.
fn conferir_erro_io(r: Result<(), ErroGravacao>, i: usize, destino: &Path, alvo: &Ponto) {
    match r {
        Err(ErroGravacao::Io {
            etapa,
            tipo,
            identidade,
        }) => {
            assert_eq!(etapa, etapa_esperada(i), "{alvo:?}");
            assert_eq!(tipo, io::ErrorKind::StorageFull, "{alvo:?}");
            if i < ANTES_DO_RENAME {
                assert_eq!(identidade, None, "{alvo:?}");
            } else {
                let no_lugar = IdentidadeArquivo::from(&fs::metadata(destino).unwrap());
                assert_eq!(identidade, Some(no_lugar), "{alvo:?}");
            }
        }
        outro => panic!("{alvo:?}: {outro:?}"),
    }
}

#[test]
fn v2_9_falha_de_es_injetada_em_cada_ponto() {
    let marca = hex_aleatorio();
    let antigo = cofre().selar(&conteudo(&marca, 0, TAM_GRANDE)).unwrap();
    let novo = cofre().selar(&conteudo(&marca, 1, TAM_GRANDE)).unwrap();
    for (i, alvo) in PONTOS.iter().enumerate() {
        let d = Dir::novo("injetada");
        gravar(&d.cofre(), &antigo).unwrap();
        let mut passou = false;
        let r = gravar_com_pontos(&d.cofre(), &novo, &mut |p| {
            if p == *alvo {
                passou = true;
                return Err(io::Error::from(io::ErrorKind::StorageFull));
            }
            Ok(())
        });
        assert!(passou, "ponto {alvo:?} não aconteceu");
        conferir_erro_io(r.map(|_| ()), i, &d.cofre(), alvo);
        let ficou = fs::read(d.cofre()).unwrap();
        if i < ANTES_DO_RENAME {
            assert_eq!(ficou, antigo, "{alvo:?}: a versão anterior tem de ficar");
        } else {
            assert_eq!(ficou, novo, "{alvo:?}: a versão nova já está no lugar");
        }
        assert!(d.temporarios().is_empty(), "{alvo:?}: temporário ficou");
        assert_eq!(modo(&d.cofre()), 0o600);
    }
}

// --- Processo filho e SIGKILL (V2.9) ----------------------------------------------------

/// Processo filho dos testes de morte e do strace. Não faz nada sem o modo no ambiente.
#[test]
#[ignore = "só roda como processo filho dos testes de gravação"]
fn filho_gravacao() {
    let Ok(modo) = std::env::var(ENV_MODO) else {
        return;
    };
    let dir = PathBuf::from(std::env::var_os(ENV_DIR).unwrap());
    let destino = dir.join("cofre.ward");
    if let Some(i) = modo.strip_prefix("ponto:") {
        let i: usize = i.parse().unwrap();
        let novo = fs::read(dir.join("novo")).unwrap();
        let _ = gravar_com_pontos(&destino, &novo, &mut |p| {
            if p == PONTOS[i] {
                println!("\nWARD-PONTO");
                // Espera o SIGKILL do pai; sem ele, sai sem continuar a gravação.
                std::thread::sleep(VIDA_FILHO);
                std::process::exit(3);
            }
            Ok(())
        });
        println!("\nWARD-FIM");
    } else if modo == "laco" {
        let versoes: Vec<Vec<u8>> = (0..3)
            .map(|k| fs::read(dir.join(format!("v{k}"))).unwrap())
            .collect();
        println!("\nWARD-PRONTO");
        let inicio = Instant::now();
        for v in versoes.iter().cycle().skip(1) {
            gravar(&destino, v).unwrap();
            if inicio.elapsed() > VIDA_FILHO {
                std::process::exit(3);
            }
        }
    } else if modo == "strace" {
        let marca = std::env::var(ENV_MARCA).unwrap();
        let controle = std::env::var(ENV_CONTROLE).unwrap();
        let c = conteudo(&marca, 1, 4096);
        let cofre = CofreAberto::novo(&senha(), configuracao()).unwrap();
        // Controle do detector: uma escrita que o strace tem de mostrar inteira.
        println!("{controle}");
        gravar_cofre(&destino, &cofre, &c).unwrap();
    } else {
        panic!("modo desconhecido");
    }
}

fn filho(modo: &str, dir: &Path) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            NOME_FILHO,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(ENV_MODO, modo)
        .env(ENV_DIR, dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

/// Espera a linha `esperada` na saída do filho (até 60 s).
fn esperar_linha(f: &mut Child, esperada: &'static str) {
    let saida = f.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for l in BufReader::new(saida).lines() {
            let Ok(l) = l else { break };
            // O libtest escreve `test … ... ` sem quebra de linha antes da saída do
            // filho; o filho começa a marca numa linha nova, mas a comparação aceita
            // as duas formas.
            if l.trim_end() == esperada || l.ends_with(esperada) {
                let _ = tx.send(());
                // continua lendo para o filho nunca travar no pipe
            }
        }
    });
    if rx.recv_timeout(Duration::from_secs(60)).is_err() {
        let _ = f.kill();
        let st = f.wait();
        panic!("o filho não chegou a {esperada}: {st:?}");
    }
}

fn matar(mut f: Child) {
    f.kill().unwrap(); // SIGKILL
    let st = f.wait().unwrap();
    assert!(!st.success(), "o filho tinha de morrer pelo SIGKILL");
}

/// SIGKILL exatamente em cada ponto: antes do rename fica a versão anterior inteira,
/// depois, a nova inteira. O temporário deixado é cifrado (sem a marca em claro).
#[test]
fn v2_9_sigkill_em_cada_ponto() {
    let marca = hex_aleatorio();
    let antigo = cofre().selar(&conteudo(&marca, 0, TAM_GRANDE)).unwrap();
    let novo = cofre().selar(&conteudo(&marca, 1, TAM_GRANDE)).unwrap();
    for (i, alvo) in PONTOS.iter().enumerate() {
        let d = Dir::novo("sigkill-ponto");
        gravar(&d.cofre(), &antigo).unwrap();
        fs::write(d.0.join("novo"), &novo).unwrap();
        let mut f = filho(&format!("ponto:{i}"), &d.0);
        esperar_linha(&mut f, "WARD-PONTO");
        matar(f);

        let ficou = fs::read(d.cofre()).unwrap();
        let temps = d.temporarios();
        if i < ANTES_DO_RENAME {
            assert_eq!(ficou, antigo, "ponto {alvo:?}");
            assert_eq!(temps.len(), 1, "ponto {alvo:?}");
        } else {
            assert_eq!(ficou, novo, "ponto {alvo:?}");
            assert!(temps.is_empty(), "ponto {alvo:?}");
        }
        for t in &temps {
            let b = fs::read(t).unwrap();
            assert!(!contem(&b, marca.as_bytes()), "temporário em claro");
            assert!(novo.starts_with(&b), "temporário é prefixo do cifrado novo");
            assert_eq!(modo(t), 0o600);
        }
        assert_eq!(limpar_temporarios(&d.cofre()).unwrap(), temps.len());
        assert!(d.temporarios().is_empty());
        // O cofre que ficou é gravável de novo.
        gravar(&d.cofre(), &antigo).unwrap();
    }
    assert_eq!(abrir_conteudo(&antigo), conteudo(&marca, 0, TAM_GRANDE));
    assert_eq!(abrir_conteudo(&novo), conteudo(&marca, 1, TAM_GRANDE));
}

/// SIGKILL em 200 pontos aleatórios de um filho que grava sem parar: o destino é
/// sempre uma das versões inteiras, e cada versão vista abre com a senha.
#[test]
fn v2_9_sigkill_em_200_pontos_aleatorios() {
    const MORTES: usize = 200;
    let d = Dir::novo("sigkill-aleatorio");
    let marca = hex_aleatorio();
    let versoes: Vec<Vec<u8>> = (0..3)
        .map(|k| cofre().selar(&conteudo(&marca, k, TAM_GRANDE)).unwrap())
        .collect();
    for (k, v) in versoes.iter().enumerate() {
        fs::write(d.0.join(format!("v{k}")), v).unwrap();
    }
    gravar(&d.cofre(), &versoes[0]).unwrap();

    // Duração de uma gravação aqui, para espalhar as mortes por várias gravações.
    let t0 = Instant::now();
    for v in &versoes {
        gravar(&d.cofre(), v).unwrap();
    }
    let uma = (t0.elapsed() / 3).max(Duration::from_millis(1));
    let janela = (uma * 3).as_micros() as u64;

    let mut vistas = [0usize; 3];
    let mut com_temporario = 0;
    for _ in 0..MORTES {
        let mut f = filho("laco", &d.0);
        esperar_linha(&mut f, "WARD-PRONTO");
        std::thread::sleep(Duration::from_micros(u64_aleatorio() % janela));
        matar(f);

        let ficou = fs::read(d.cofre()).unwrap();
        let k = versoes
            .iter()
            .position(|v| *v == ficou)
            .expect("o cofre não é nenhuma versão inteira");
        vistas[k] += 1;
        let temps = d.temporarios();
        assert!(temps.len() <= 1);
        for t in &temps {
            let b = fs::read(t).unwrap();
            assert!(!contem(&b, marca.as_bytes()), "temporário em claro");
        }
        if !temps.is_empty() {
            com_temporario += 1;
        }
        limpar_temporarios(&d.cofre()).unwrap();
    }
    eprintln!(
        "v2_9: {MORTES} mortes, {com_temporario} no meio de uma gravação, versões vistas {vistas:?}, gravação ~{uma:?}"
    );
    // As mortes caíram de fato dentro das gravações, e as gravações avançaram.
    assert!(
        com_temporario >= MORTES / 4,
        "poucas mortes no meio da gravação"
    );
    assert!(vistas.iter().filter(|&&n| n > 0).count() >= 2);
    for (k, n) in vistas.iter().enumerate() {
        if *n > 0 {
            assert_eq!(abrir_conteudo(&versoes[k]), conteudo(&marca, k, TAM_GRANDE));
        }
    }
}

// --- strace: ordem das chamadas e nada em claro (V2.10, V2.11) --------------------------

/// Uma chamada de sistema do log do strace (`-f`: cada linha começa com o pid).
struct Chamada {
    nome: String,
    args: String,
    ret: Option<i64>,
}

fn ler_log(log: &str) -> Vec<Chamada> {
    let mut v = Vec::new();
    for linha in log.lines() {
        let resto = linha
            .split_once(' ')
            .map(|(pid, r)| {
                if pid.chars().all(|c| c.is_ascii_digit()) {
                    r
                } else {
                    linha
                }
            })
            .unwrap_or(linha)
            .trim_start();
        let Some(abre) = resto.find('(') else {
            continue;
        };
        let nome = &resto[..abre];
        if nome.is_empty() || !nome.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue; // `<... resumed>`, `+++ exited`, sinais
        }
        // O strace alinha o retorno com espaços: `fsync(3)        = 0`.
        let ret = resto
            .rsplit_once(" = ")
            .filter(|(chamada, _)| chamada.trim_end().ends_with(')'))
            .and_then(|(_, r)| r.split_whitespace().next())
            .and_then(|r| r.parse().ok());
        v.push(Chamada {
            nome: nome.to_owned(),
            args: resto[abre + 1..].to_owned(),
            ret,
        });
    }
    v
}

/// `n`-ésimo argumento entre aspas (a partir de 0).
fn caminho_arg(args: &str, n: usize) -> Option<String> {
    let partes: Vec<&str> = args.split('"').collect();
    partes.get(1 + 2 * n).map(|s| (*s).to_owned())
}

fn fd_arg(args: &str) -> Option<i64> {
    args.split([',', ')']).next()?.trim().parse().ok()
}

#[test]
fn v2_10_v2_11_strace_ordem_e_nada_em_claro() {
    assert!(
        Path::new(STRACE).exists(),
        "V2.10/V2.11 precisam do strace em {STRACE} (apt install strace)"
    );
    let d = Dir::novo("strace");
    let marca = format!("WARD-MARCA-CLARO-{}", hex_aleatorio());
    let controle = format!("WARD-CONTROLE-{}", hex_aleatorio());
    let log = d.0.join("strace.log");
    let st = Command::new(STRACE)
        .args(["-f", "-qq", "-s", "100000", "-o"])
        .arg(&log)
        .args([
            "-e",
            "trace=openat,open,creat,write,pwrite64,writev,fsync,fdatasync,rename,renameat,renameat2",
            "--",
        ])
        .arg(std::env::current_exe().unwrap())
        .args([
            "--exact",
            NOME_FILHO,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(ENV_MODO, "strace")
        .env(ENV_DIR, &d.0)
        .env(ENV_MARCA, &marca)
        .env(ENV_CONTROLE, &controle)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert!(
        st.status.success(),
        "strace/filho falhou: {}",
        String::from_utf8_lossy(&st.stderr)
    );
    let mut texto = String::new();
    File::open(&log)
        .unwrap()
        .read_to_string(&mut texto)
        .unwrap();

    // V2.11: o detector enxerga escrita inteira (controle), e a marca em claro não
    // aparece em nenhum buffer escrito, nem no cofre, nem em temporário.
    assert!(
        texto.contains(&controle),
        "o strace não mostrou a escrita de controle"
    );
    assert!(!texto.contains(&marca), "conteúdo em claro escrito: V2.11");
    assert!(
        !texto.contains(SEGREDO_FALSO),
        "segredo do conteúdo escrito: V2.11"
    );
    assert!(!texto.contains(SENHA), "senha mestra escrita: V2.11");
    let cofre_bytes = fs::read(d.cofre()).unwrap();
    assert!(!contem(&cofre_bytes, marca.as_bytes()));
    assert!(d.temporarios().is_empty());
    for nome in d.nomes() {
        if nome != "strace.log" {
            let b = fs::read(d.0.join(&nome)).unwrap();
            assert!(
                !contem(&b, marca.as_bytes()),
                "{nome} tem conteúdo em claro"
            );
        }
    }

    // V2.10: temporário no mesmo diretório → escritas → fsync dele → rename → fsync
    // do diretório.
    let dir = d.0.to_string_lossy().into_owned();
    let destino = d.cofre().to_string_lossy().into_owned();
    let chamadas = ler_log(&texto);
    let mut i_tmp = None;
    let mut fd_tmp = -1;
    let mut caminho_tmp = String::new();
    for (i, c) in chamadas.iter().enumerate() {
        if c.nome == "openat"
            && caminho_arg(&c.args, 0).is_some_and(|p| p.contains("/.cofre.ward.ward-tmp-"))
        {
            assert!(i_tmp.is_none(), "dois temporários numa gravação");
            let p = caminho_arg(&c.args, 0).unwrap();
            assert_eq!(
                Path::new(&p).parent().unwrap(),
                d.0.as_path(),
                "temporário fora do diretório do cofre"
            );
            assert!(c.args.contains("O_CREAT") && c.args.contains("O_EXCL"));
            assert!(c.args.contains("O_CLOEXEC"));
            assert!(
                c.args.contains("0600"),
                "temporário sem modo 0600: {}",
                c.args
            );
            i_tmp = Some(i);
            fd_tmp = c.ret.unwrap();
            caminho_tmp = p;
        }
    }
    let i_tmp = i_tmp.expect("o temporário não foi aberto");
    let depois = |de: usize, f: &dyn Fn(&Chamada) -> bool| -> usize {
        de + 1
            + chamadas[de + 1..]
                .iter()
                .position(f)
                .expect("chamada esperada não aconteceu")
    };
    let i_fsync = depois(i_tmp, &|c| {
        c.nome == "fsync" && fd_arg(&c.args) == Some(fd_tmp)
    });
    let escritas: Vec<usize> = (i_tmp + 1..i_fsync)
        .filter(|&i| chamadas[i].nome == "write" && fd_arg(&chamadas[i].args) == Some(fd_tmp))
        .collect();
    assert!(!escritas.is_empty(), "nenhuma escrita no temporário");
    let i_rename = depois(i_fsync, &|c| c.nome.starts_with("rename"));
    let r = &chamadas[i_rename];
    assert_eq!(r.ret, Some(0));
    assert!(r.args.contains(&format!("\"{caminho_tmp}\"")), "{}", r.args);
    assert!(r.args.contains(&format!("\"{destino}\"")), "{}", r.args);
    let i_dir = depois(i_rename, &|c| {
        c.nome == "openat" && caminho_arg(&c.args, 0).as_deref() == Some(dir.as_str())
    });
    let fd_dir = chamadas[i_dir].ret.unwrap();
    let i_fsync_dir = depois(i_dir, &|c| {
        c.nome == "fsync" && fd_arg(&c.args) == Some(fd_dir)
    });
    assert_eq!(chamadas[i_fsync_dir].ret, Some(0));

    // Ninguém mais abriu arquivo para escrever no diretório do cofre, e o destino nunca
    // foi aberto para escrita (só trocado pelo rename).
    for c in &chamadas {
        if !matches!(c.nome.as_str(), "openat" | "open" | "creat") {
            continue;
        }
        let Some(p) = caminho_arg(&c.args, 0) else {
            continue;
        };
        let escreve = c.args.contains("O_WRONLY") || c.args.contains("O_RDWR");
        if escreve && Path::new(&p).parent() == Some(d.0.as_path()) {
            assert_eq!(p, caminho_tmp, "arquivo aberto para escrita: {p}");
        }
    }
    // Nenhuma escrita no temporário depois do fsync dele.
    assert!(
        !chamadas[i_fsync + 1..]
            .iter()
            .take_while(|c| !c.nome.starts_with("rename"))
            .any(|c| c.nome == "write" && fd_arg(&c.args) == Some(fd_tmp))
    );
    assert_eq!(abrir_conteudo(&cofre_bytes), conteudo(&marca, 1, 4096));
}

// --- Geração queimada ao selar (D53, V2.44 reescrita, V2.53) ----------------------------

/// O cofre de teste de D15 (as mesmas entradas dos testes do corpo). O módulo de teste
/// do corpo é privado dele; incluir o arquivo de novo aqui evita abrir a visibilidade
/// do `corpo::testes` só para os testes da gravação.
#[allow(clippy::duplicate_mod)]
#[path = "../corpo/cofre_teste.rs"]
mod cofre_teste;
use crate as wc;

/// Geração gravada no arquivo (abre sem mínimo nem queimada).
fn geracao_no_disco(caminho: &Path) -> u64 {
    let bytes = fs::read(caminho).unwrap();
    let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
    crate::corpo::abrir_entradas(&bytes, &senha())
        .unwrap()
        .0
        .geracao()
}

fn cofre_novo() -> CofreAberto {
    let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
    CofreAberto::novo(&senha(), configuracao()).unwrap()
}

/// O mesmo ponto, sem olhar quanto já foi escrito.
fn mesmo_ponto(a: Ponto, b: Ponto) -> bool {
    std::mem::discriminant(&a) == std::mem::discriminant(&b)
}

/// Erro e `Debug` do erro sem valor do cofre de teste.
fn sem_valor(texto: &str) {
    for s in cofre_teste::SEGREDOS {
        assert!(!texto.contains(s), "erro com segredo de teste");
    }
    assert!(
        !texto.contains("falso") && !texto.contains("acme"),
        "{texto}"
    );
}

/// V2.44 (reescrita, D53): falha injetada em cada ponto da gravação (V2.9). O cofre
/// aberto fica com a geração que selou (n+1) mesmo que a gravação falhe antes do rename
/// (o arquivo segue com n) ou depois dele (o arquivo já tem n+1); a gravação seguinte
/// grava n+2. Nenhum par de arquivos selados tem a mesma geração.
#[test]
fn v2_44_falha_em_cada_ponto_queima_a_geracao() {
    let entradas = cofre_teste::cofre_de_teste();
    for (i, alvo) in PONTOS.iter().enumerate() {
        let d = Dir::novo("queimada");
        let mut c = cofre_novo();
        gravar_entradas(&d.cofre(), &mut c, &entradas).unwrap();
        let n = c.geracao();
        assert_eq!(n, 1);
        let mut passou = false;
        let r = gravar_entradas_com_pontos(&d.cofre(), &mut c, &entradas, &mut |p| {
            if mesmo_ponto(p, *alvo) && !passou {
                passou = true;
                return Err(io::Error::from(io::ErrorKind::StorageFull));
            }
            Ok(())
        });
        assert!(passou, "ponto {alvo:?} não aconteceu");
        conferir_erro_io(r.map(|_| ()), i, &d.cofre(), alvo);
        assert_eq!(
            c.geracao(),
            n + 1,
            "{alvo:?}: a geração selada tem de ficar"
        );
        let no_disco = geracao_no_disco(&d.cofre());
        if i < ANTES_DO_RENAME {
            assert_eq!(no_disco, n, "{alvo:?}: a versão anterior tem de ficar");
        } else {
            assert_eq!(no_disco, n + 1, "{alvo:?}: a versão nova já está no lugar");
        }
        assert!(d.temporarios().is_empty(), "{alvo:?}: temporário ficou");
        // A gravação seguinte usa um número que nunca foi selado.
        gravar_entradas(&d.cofre(), &mut c, &entradas).unwrap();
        assert_eq!(c.geracao(), n + 2, "{alvo:?}");
        assert_eq!(geracao_no_disco(&d.cofre()), n + 2, "{alvo:?}");
    }
}

/// D53: um corpo que não serializa não chega a selar, então não queima nem toca no
/// disco.
#[test]
fn d53_corpo_que_nao_serializa_nao_queima() {
    let d = Dir::novo("nao-queima");
    let mut c = cofre_novo();
    gravar_entradas(&d.cofre(), &mut c, &[]).unwrap();
    let antes = fs::read(d.cofre()).unwrap();
    // Duas entradas com o mesmo nome não viram corpo.
    let um = cofre_teste::cofre_de_teste().remove(0);
    let dois = cofre_teste::cofre_de_teste().remove(0);
    let r = gravar_entradas(&d.cofre(), &mut c, &[um, dois]);
    assert!(matches!(r, Err(ErroGravacao::Corpo(_))), "{r:?}");
    assert_eq!(c.geracao(), 1);
    assert!(fs::read(d.cofre()).unwrap() == antes);
    assert!(d.temporarios().is_empty());
}

/// V2.53 (D53, repro `geracao.rs` do auditor): o atacante tira a leitura do diretório
/// (0300), então o fsync do diretório falha depois do rename. A gravação A (uma entrada
/// a menos) devolve erro com o cofre aberto em n+1; a B (a entrada de volta) grava n+2;
/// a cópia de A posta no lugar de B é recusada com `Retrocesso` pelo mínimo n+2, sem
/// entrada e sem valor. Antes de D53, B gravava n+1 de novo e A abria com o mínimo.
#[test]
fn v2_53_fsync_do_diretorio_falha_e_a_geracao_nao_repete() {
    let d = Dir::novo("v2-53");
    let mut c = cofre_novo();
    let mut entradas = cofre_teste::cofre_de_teste();
    gravar_entradas(&d.cofre(), &mut c, &entradas).unwrap();
    let n = c.geracao();
    let arquivo_n = fs::read(d.cofre()).unwrap();
    let ultima = entradas.pop().unwrap();

    fs::set_permissions(&d.0, fs::Permissions::from_mode(0o300)).unwrap();
    // Root lê o diretório mesmo com 0300: aí o ataque não reproduz, e o caso fica
    // coberto pela injeção de `v2_44_falha_em_cada_ponto_queima_a_geracao`.
    if File::open(&d.0).is_ok() {
        fs::set_permissions(&d.0, fs::Permissions::from_mode(0o700)).unwrap();
        eprintln!("v2_53: o processo lê diretório 0300 (root?); ataque não reproduz aqui");
        return;
    }
    let r = gravar_entradas(&d.cofre(), &mut c, &entradas);
    fs::set_permissions(&d.0, fs::Permissions::from_mode(0o700)).unwrap();
    let no_lugar = IdentidadeArquivo::from(&fs::metadata(d.cofre()).unwrap());
    assert_eq!(
        r,
        Err(ErroGravacao::Io {
            etapa: Etapa::FsyncDiretorio,
            tipo: io::ErrorKind::PermissionDenied,
            identidade: Some(no_lugar),
        })
    );
    assert_eq!(c.geracao(), n + 1);
    let arquivo_a = fs::read(d.cofre()).unwrap();
    assert_eq!(geracao_no_disco(&d.cofre()), n + 1);

    entradas.push(ultima);
    gravar_entradas(&d.cofre(), &mut c, &entradas).unwrap();
    assert_eq!(c.geracao(), n + 2);
    let arquivo_b = fs::read(d.cofre()).unwrap();
    assert_eq!(geracao_no_disco(&d.cofre()), n + 2);

    // Nenhum par de arquivos selados nessa sequência tem a mesma geração.
    let geracoes: Vec<u64> = [&arquivo_n, &arquivo_a, &arquivo_b]
        .iter()
        .map(|b| {
            let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
            crate::corpo::abrir_entradas(b, &senha())
                .unwrap()
                .0
                .geracao()
        })
        .collect();
    assert_eq!(geracoes, [n, n + 1, n + 2]);

    // O atacante troca B por A; o daemon abre com a maior geração vista (n+2).
    {
        let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
        match crate::corpo::abrir_entradas_desde(&arquivo_a, &senha(), n + 2, n + 2) {
            Ok(_) => panic!("a cópia de A abriu com o mínimo de B"),
            Err(e) => {
                assert_eq!(e, crate::corpo::ErroAbrir::Retrocesso);
                sem_valor(&format!("{e} {e:?}"));
            }
        }
    }

    // Reaberto com a queimada que o daemon guardou, A (n+1 ≥ n) fica em n+2 e a
    // gravação seguinte sela n+3, nunca n+2 de novo.
    fs::write(d.cofre(), &arquivo_a).unwrap();
    let (mut reaberto, lidas) = {
        let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
        crate::corpo::abrir_entradas_desde(&arquivo_a, &senha(), n, n + 2).unwrap()
    };
    assert_eq!(reaberto.geracao(), n + 2);
    gravar_entradas(&d.cofre(), &mut reaberto, &lidas).unwrap();
    assert_eq!(reaberto.geracao(), n + 3);
    assert_eq!(geracao_no_disco(&d.cofre()), n + 3);

    // A troca de senha com queimada n+5 sobre o arquivo em n+3 sela n+6.
    let nova = SenhaMestra::nova("falso-mestra-ward-02".to_owned());
    let trocado = {
        let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
        trocar_senha(&d.cofre(), &senha(), &nova, n + 3, n + 5)
            .unwrap()
            .0
    };
    assert_eq!(trocado.geracao(), n + 6);
    let bytes = fs::read(d.cofre()).unwrap();
    let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
    let (lido, _) = crate::corpo::abrir_entradas(&bytes, &nova).unwrap();
    assert_eq!(lido.geracao(), n + 6);
}

/// D53: a queimada nunca desce a geração: abrir com queimada menor que a do disco fica
/// com a do disco, e a troca de senha com queimada menor sela a do disco + 1.
#[test]
fn d53_queimada_menor_que_o_disco_nao_desce() {
    let d = Dir::novo("queimada-menor");
    let mut c = cofre_novo();
    gravar_entradas(&d.cofre(), &mut c, &[]).unwrap();
    gravar_entradas(&d.cofre(), &mut c, &[]).unwrap();
    gravar_entradas(&d.cofre(), &mut c, &[]).unwrap();
    assert_eq!(c.geracao(), 3);
    let bytes = fs::read(d.cofre()).unwrap();
    let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
    let (aberto, _) = crate::corpo::abrir_entradas_desde(&bytes, &senha(), 0, 1).unwrap();
    assert_eq!(aberto.geracao(), 3);
    let nova = SenhaMestra::nova("falso-mestra-ward-02".to_owned());
    let (trocado, _) = trocar_senha(&d.cofre(), &senha(), &nova, 3, 2).unwrap();
    assert_eq!(trocado.geracao(), 4);
}

/// D62 (S7): a identidade devolvida é a do arquivo gravado (do fd do temporário), e uma
/// troca logo depois do rename não muda o que foi devolvido: quem chama nota a troca
/// comparando com o disco, em vez de aceitar o arquivo trocado como sendo do Ward.
#[test]
fn d62_identidade_do_fd_e_troca_logo_depois_do_rename() {
    let d = Dir::novo("d62-identidade");
    let mut c = cofre_novo();
    let id = gravar_entradas(&d.cofre(), &mut c, &[]).unwrap();
    assert_eq!(
        id,
        IdentidadeArquivo::from(&fs::metadata(d.cofre()).unwrap())
    );

    // Gancho logo depois do rename: outro processo põe outro arquivo no lugar.
    let outro = d.0.join("outro");
    let mut trocou = false;
    let r = gravar_entradas_com_pontos(&d.cofre(), &mut c, &[], &mut |p| {
        if p == Ponto::DepoisDoRename {
            let bytes = fs::read(d.cofre()).unwrap();
            fs::write(&outro, &bytes).unwrap();
            fs::rename(&outro, d.cofre()).unwrap();
            trocou = true;
        }
        Ok(())
    });
    assert!(trocou);
    let devolvida = r.unwrap();
    let no_disco = IdentidadeArquivo::from(&fs::metadata(d.cofre()).unwrap());
    assert_ne!(
        devolvida, no_disco,
        "a troca depois do rename passaria como do Ward"
    );
    sem_valor(&format!("{devolvida:?}"));
}

/// D62 (V3.35): um cofre novo criado onde o estado velho queimou a geração 7 sela a 8
/// na primeira gravação; a geração nunca desce; em `u64::MAX`, a gravação é recusada.
#[test]
fn d62_cofre_novo_depois_da_geracao_queimada() {
    let d = Dir::novo("d62-geracao");
    let mut c = cofre_novo().depois_da_geracao(7);
    assert_eq!(c.geracao(), 7);
    gravar_entradas(&d.cofre(), &mut c, &[]).unwrap();
    assert_eq!(c.geracao(), 8);
    assert_eq!(geracao_no_disco(&d.cofre()), 8);
    let c = c.depois_da_geracao(3);
    assert_eq!(c.geracao(), 8, "nunca desce");
    let mut cheio = cofre_novo().depois_da_geracao(u64::MAX);
    assert_eq!(
        gravar_entradas(&d.cofre(), &mut cheio, &[]),
        Err(ErroGravacao::GeracaoEsgotada)
    );
    assert_eq!(geracao_no_disco(&d.cofre()), 8);
}
