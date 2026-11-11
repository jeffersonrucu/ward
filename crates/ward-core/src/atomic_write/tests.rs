//! Atomic write tests (V2.9–V2.11, S7, S2).
//!
//! The death tests run the write in a child process: the test binary itself, called
//! again with only the ignored test [`child_write`] and the mode in the environment. The
//! parent kills the child with SIGKILL (`Child::kill`) and checks the destination.
//!
//! Files only in `target/<profile>/ward-test-atomic-write/` (inside the repository,
//! outside git). Fake password and content (D15); the "in clear" mark of each test is
//! random.

use std::io::{BufRead, BufReader, Read as _};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::*;
use crate::config::{PinHash, VaultConfig};
use crate::format::{MasterPassword, open};

const PASSWORD: &str = "falso-mestra-ward-01";
const CHILD_NAME: &str = "atomic_write::tests::child_write";
const ENV_MODE: &str = "WARD_TESTE_GRAVACAO_MODO";
const ENV_DIR: &str = "WARD_TESTE_GRAVACAO_DIR";
const ENV_MARK: &str = "WARD_TESTE_GRAVACAO_MARCA";
const ENV_CONTROL: &str = "WARD_TESTE_GRAVACAO_CONTROLE";
const STRACE: &str = "/usr/bin/strace";
/// Maximum lifetime of the child: if the parent dies without killing it (interrupted
/// test), the child exits on its own instead of writing or sleeping forever.
const CHILD_LIFETIME: Duration = Duration::from_secs(90);
/// Fake secret that goes in clear in the test content (D15).
const FAKE_SECRET: &str = "falso-PG:s3nha/01@#";
/// Larger than a write chunk, so there is a point in the middle of the write.
const LARGE_SIZE: usize = CHUNK * 5 / 2;

/// One real Argon2id (256 MiB) at a time in these tests.
static ARGON: Mutex<()> = Mutex::new(());

fn password() -> MasterPassword {
    MasterPassword::new(PASSWORD.to_owned())
}

/// Config of the test vault (hash of the fake PIN of D15).
fn config() -> VaultConfig {
    VaultConfig::new(PinHash::create(b"135790").unwrap())
}

/// Shared open vault (a single Argon2id for all the tests here).
fn vault() -> &'static OpenVault {
    static VAULT: OnceLock<OpenVault> = OnceLock::new();
    VAULT.get_or_init(|| {
        let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
        OpenVault::new(&password(), config()).unwrap()
    })
}

fn open_content(file: &[u8]) -> Vec<u8> {
    let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
    open(file, &password()).unwrap().1.to_vec()
}

fn random_hex() -> String {
    let mut b = [0u8; 12];
    crypto::random(&mut b).unwrap();
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn random_u64() -> u64 {
    let mut b = [0u8; 8];
    crypto::random(&mut b).unwrap();
    u64::from_le_bytes(b)
}

/// Fake clear content with the mark, of the asked size (rounded up).
fn content(mark: &str, version: usize, size: usize) -> Vec<u8> {
    let line = format!(
        "{{\"versao\":{version},\"nome\":\"acme-pg-dev\",\"senha\":\"{FAKE_SECRET}\",\"marca\":\"{mark}\"}}\n"
    );
    let mut c = Vec::with_capacity(size + line.len());
    while c.len() < size {
        c.extend_from_slice(line.as_bytes());
    }
    c
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|j| j == needle)
}

/// New test directory in `target/<profile>/ward-test-atomic-write/`, 0700, deleted at
/// the end of the test.
struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Self {
        let exe = std::env::current_exe().unwrap();
        // target/<profile>/deps/<binary>
        let base = exe
            .parent()
            .and_then(Path::parent)
            .unwrap()
            .join("ward-test-atomic-write");
        fs::create_dir_all(&base).unwrap();
        let d = base.join(format!("{name}-{}-{}", std::process::id(), random_hex()));
        fs::create_dir(&d).unwrap();
        fs::set_permissions(&d, fs::Permissions::from_mode(0o700)).unwrap();
        Self(d)
    }

    fn vault(&self) -> PathBuf {
        self.0.join("cofre.ward")
    }

    /// Names in the directory that are temporary files of the vault.
    fn temp_files(&self) -> Vec<PathBuf> {
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

    fn names(&self) -> Vec<String> {
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

fn mode(p: &Path) -> u32 {
    fs::symlink_metadata(p).unwrap().permissions().mode() & 0o7777
}

// --- Happy path and refusals ------------------------------------------------------------

#[test]
fn round_trip_mode_0600_without_temp_file() {
    let d = Dir::new("ida");
    let mark = random_hex();
    let c = content(&mark, 1, 4096);
    write_vault(&d.vault(), vault(), &c).unwrap();
    let bytes = fs::read(d.vault()).unwrap();
    assert!(
        !contains(&bytes, mark.as_bytes()),
        "conteúdo em claro no arquivo"
    );
    assert_eq!(open_content(&bytes), c);
    assert_eq!(mode(&d.vault()), 0o600);
    assert_eq!(d.names(), vec!["cofre.ward"]);
}

#[test]
fn replaces_existing_destination_and_sets_0600() {
    let d = Dir::new("substitui");
    let v1 = vault().seal(b"v1").unwrap();
    let v2 = vault().seal(b"v2").unwrap();
    fs::write(d.vault(), &v1).unwrap();
    fs::set_permissions(d.vault(), fs::Permissions::from_mode(0o644)).unwrap();
    write(&d.vault(), &v2).unwrap();
    assert_eq!(fs::read(d.vault()).unwrap(), v2);
    assert_eq!(mode(&d.vault()), 0o600);
    assert_eq!(d.names(), vec!["cofre.ward"]);
}

#[test]
fn relative_path_without_directory() {
    // `split` uses `.` as the directory; here only the decomposition, without changing
    // the cwd.
    let (dir, name) = split(Path::new("cofre.ward")).unwrap();
    assert_eq!(dir, PathBuf::from("."));
    assert_eq!(name, OsString::from("cofre.ward"));
}

/// Attack (S2): a caller tries to write the clear content (or a truncated vault, or
/// garbage) through the write module. Nothing is created and the destination stays
/// intact.
#[test]
fn refuses_to_write_bytes_that_are_not_a_vault() {
    let d = Dir::new("recusa");
    let previous = vault().seal(b"anterior").unwrap();
    fs::write(d.vault(), &previous).unwrap();
    let mark = random_hex();
    let clear = content(&mark, 1, 2048);
    let mut truncated = vault().seal(b"x").unwrap();
    truncated.pop();
    let mut no_magic = vault().seal(b"x").unwrap();
    no_magic[0] ^= 1;
    for bad in [
        &clear[..],
        &truncated[..],
        &no_magic[..],
        &b""[..],
        &[0u8; 4096][..],
    ] {
        assert_eq!(write(&d.vault(), bad), Err(WriteError::NotVault));
        assert_eq!(fs::read(d.vault()).unwrap(), previous);
        assert_eq!(d.names(), vec!["cofre.ward"], "nenhum temporário criado");
    }
}

#[test]
fn invalid_destination() {
    let v = vault().seal(b"x").unwrap();
    for p in ["/", "", "..", "/tmp/.."] {
        assert_eq!(
            write(Path::new(p), &v),
            Err(WriteError::InvalidDestination),
            "{p:?}"
        );
    }
}

#[test]
fn directory_destination_fails_without_leaving_temp_file() {
    let d = Dir::new("destino-dir");
    fs::create_dir(d.vault()).unwrap();
    fs::write(d.vault().join("dentro"), b"intacto").unwrap();
    let v = vault().seal(b"x").unwrap();
    let e = write(&d.vault(), &v).unwrap_err();
    assert!(
        matches!(
            e,
            WriteError::Io {
                step: Step::Rename,
                ..
            }
        ),
        "{e:?}"
    );
    assert_eq!(d.names(), vec!["cofre.ward"]);
    assert_eq!(fs::read(d.vault().join("dentro")).unwrap(), b"intacto");
}

/// Attack: the destination is a symbolic link to a file of the victim. The rename
/// replaces the link itself with the new vault (does not follow the link) and the victim
/// stays intact.
#[test]
fn symlink_destination_is_not_followed() {
    let d = Dir::new("destino-link");
    let victim = d.0.join("vitima");
    fs::write(&victim, b"original").unwrap();
    symlink(&victim, d.vault()).unwrap();
    let v = vault().seal(b"x").unwrap();
    write(&d.vault(), &v).unwrap();
    assert_eq!(fs::read(&victim).unwrap(), b"original");
    let meta = fs::symlink_metadata(d.vault()).unwrap();
    assert!(meta.file_type().is_file(), "o destino continuou link");
    assert_eq!(fs::read(d.vault()).unwrap(), v);
    assert_eq!(mode(&d.vault()), 0o600);
}

/// Attack: a symbolic link (or file) planted with the temporary file's name pointing to
/// a file of the victim. The creation refuses (`O_EXCL`) and the victim stays intact.
#[test]
fn temp_file_does_not_follow_symlink_nor_reuse_file() {
    let d = Dir::new("link");
    let victim = d.0.join("vitima");
    fs::write(&victim, b"original").unwrap();
    let planted = d.0.join(".cofre.ward.ward-tmp-plantado");
    symlink(&victim, &planted).unwrap();
    let e = create_temp(&planted).unwrap_err();
    assert_eq!(e.kind(), io::ErrorKind::AlreadyExists);
    let missing = d.0.join("nao-existe");
    let dangling = d.0.join(".cofre.ward.ward-tmp-pendente");
    symlink(&missing, &dangling).unwrap();
    assert_eq!(
        create_temp(&dangling).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert!(!missing.exists(), "seguiu o link pendente");
    assert_eq!(
        create_temp(&victim).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(fs::read(&victim).unwrap(), b"original");
}

#[test]
fn temp_file_is_born_0600_with_any_umask() {
    let d = Dir::new("modo");
    let p = d.0.join(".cofre.ward.ward-tmp-modo");
    drop(create_temp(&p).unwrap());
    assert_eq!(mode(&p), 0o600);
}

#[test]
fn temp_file_names_do_not_repeat() {
    // Random name: two temporary files alive at the same time have different names.
    let d = Dir::new("nomes");
    let v = vault().seal(b"x").unwrap();
    let mut seen = Vec::new();
    for _ in 0..20 {
        let r = write_with_points(&d.vault(), &v, &mut |p| {
            if p == Point::AfterCreate {
                seen.extend(d.temp_files());
                return Err(io::Error::other("parar"));
            }
            Ok(())
        });
        assert!(r.is_err());
    }
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), 20);
}

/// Attack: a symbolic link with the temporary files' prefix pointing to a file of the
/// victim. The cleanup deletes only the regular temporary files of the vault itself.
#[test]
fn clean_temp_files_deletes_only_the_vault_ones() {
    let d = Dir::new("limpar");
    let victim = d.0.join("vitima");
    fs::write(&victim, b"original").unwrap();
    fs::write(d.0.join(".cofre.ward.ward-tmp-aaaa"), b"lixo").unwrap();
    fs::write(d.0.join(".cofre.ward.ward-tmp-bbbb"), b"lixo").unwrap();
    symlink(&victim, d.0.join(".cofre.ward.ward-tmp-link")).unwrap();
    fs::create_dir(d.0.join(".cofre.ward.ward-tmp-dir")).unwrap();
    fs::write(d.0.join(".outro.ward.ward-tmp-cccc"), b"de outro").unwrap();
    fs::write(d.0.join("cofre.ward"), b"cofre").unwrap();
    assert_eq!(clean_temp_files(&d.vault()).unwrap(), 2);
    assert_eq!(
        d.names(),
        vec![
            ".cofre.ward.ward-tmp-dir",
            ".cofre.ward.ward-tmp-link",
            ".outro.ward.ward-tmp-cccc",
            "cofre.ward",
            "vitima"
        ]
    );
    assert_eq!(fs::read(&victim).unwrap(), b"original");
}

#[test]
fn error_carries_no_content() {
    let e = WriteError::Io {
        step: Step::FsyncTemp,
        kind: io::ErrorKind::StorageFull,
        identity: None,
    };
    let text = format!("{e} {e:?}");
    assert!(text.contains("sincronizar"));
    assert_eq!(
        WriteError::NotVault.to_string(),
        "recusado gravar: os dados não são um cofre cifrado"
    );
}

// --- Failure injected between write, fsync and rename (V2.9) ----------------------------

/// Points in the order of the write; the first four come before the rename.
const POINTS: [Point; 6] = [
    Point::AfterCreate,
    Point::Writing(CHUNK),
    Point::AfterWrite,
    Point::AfterFsync,
    Point::AfterRename,
    Point::AfterFsyncDir,
];
const BEFORE_RENAME: usize = 4;

fn expected_step(i: usize) -> Step {
    match POINTS[i] {
        Point::AfterCreate => Step::CreateTemp,
        Point::Writing(_) | Point::AfterWrite => Step::Write,
        Point::AfterFsync => Step::FsyncTemp,
        Point::AfterRename | Point::AfterFsyncDir => Step::FsyncDir,
    }
}

/// The I/O error injected at point `i`: right step and kind; after the rename, the
/// identity (D62) is that of the file that stayed in place; before, none.
fn verify_io_error(r: Result<(), WriteError>, i: usize, destination: &Path, target: &Point) {
    match r {
        Err(WriteError::Io {
            step,
            kind,
            identity,
        }) => {
            assert_eq!(step, expected_step(i), "{target:?}");
            assert_eq!(kind, io::ErrorKind::StorageFull, "{target:?}");
            if i < BEFORE_RENAME {
                assert_eq!(identity, None, "{target:?}");
            } else {
                let in_place = FileIdentity::from(&fs::metadata(destination).unwrap());
                assert_eq!(identity, Some(in_place), "{target:?}");
            }
        }
        other => panic!("{target:?}: {other:?}"),
    }
}

#[test]
fn v2_9_io_failure_injected_at_each_point() {
    let mark = random_hex();
    let old = vault().seal(&content(&mark, 0, LARGE_SIZE)).unwrap();
    let new = vault().seal(&content(&mark, 1, LARGE_SIZE)).unwrap();
    for (i, target) in POINTS.iter().enumerate() {
        let d = Dir::new("injetada");
        write(&d.vault(), &old).unwrap();
        let mut reached = false;
        let r = write_with_points(&d.vault(), &new, &mut |p| {
            if p == *target {
                reached = true;
                return Err(io::Error::from(io::ErrorKind::StorageFull));
            }
            Ok(())
        });
        assert!(reached, "ponto {target:?} não aconteceu");
        verify_io_error(r.map(|_| ()), i, &d.vault(), target);
        let left = fs::read(d.vault()).unwrap();
        if i < BEFORE_RENAME {
            assert_eq!(left, old, "{target:?}: a versão anterior tem de ficar");
        } else {
            assert_eq!(left, new, "{target:?}: a versão nova já está no lugar");
        }
        assert!(d.temp_files().is_empty(), "{target:?}: temporário ficou");
        assert_eq!(mode(&d.vault()), 0o600);
    }
}

// --- Child process and SIGKILL (V2.9) ---------------------------------------------------

/// Child process of the death tests and of strace. Does nothing without the mode in the
/// environment.
#[test]
#[ignore = "só roda como processo filho dos testes de gravação"]
fn child_write() {
    let Ok(mode) = std::env::var(ENV_MODE) else {
        return;
    };
    let dir = PathBuf::from(std::env::var_os(ENV_DIR).unwrap());
    let destination = dir.join("cofre.ward");
    if let Some(i) = mode.strip_prefix("ponto:") {
        let i: usize = i.parse().unwrap();
        let new = fs::read(dir.join("novo")).unwrap();
        let _ = write_with_points(&destination, &new, &mut |p| {
            if p == POINTS[i] {
                println!("\nWARD-PONTO");
                // Waits for the parent's SIGKILL; without it, exits without going on with
                // the write.
                std::thread::sleep(CHILD_LIFETIME);
                std::process::exit(3);
            }
            Ok(())
        });
        println!("\nWARD-FIM");
    } else if mode == "laco" {
        let versions: Vec<Vec<u8>> = (0..3)
            .map(|k| fs::read(dir.join(format!("v{k}"))).unwrap())
            .collect();
        println!("\nWARD-PRONTO");
        let start = Instant::now();
        for v in versions.iter().cycle().skip(1) {
            write(&destination, v).unwrap();
            if start.elapsed() > CHILD_LIFETIME {
                std::process::exit(3);
            }
        }
    } else if mode == "strace" {
        let mark = std::env::var(ENV_MARK).unwrap();
        let control = std::env::var(ENV_CONTROL).unwrap();
        let c = content(&mark, 1, 4096);
        let vault = OpenVault::new(&password(), config()).unwrap();
        // Control of the detector: a write that strace has to show in full.
        println!("{control}");
        write_vault(&destination, &vault, &c).unwrap();
    } else {
        panic!("modo desconhecido");
    }
}

fn child(mode: &str, dir: &Path) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            CHILD_NAME,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(ENV_MODE, mode)
        .env(ENV_DIR, dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

/// Waits for the `expected` line in the child's output (up to 60 s).
fn wait_line(f: &mut Child, expected: &'static str) {
    let output = f.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for l in BufReader::new(output).lines() {
            let Ok(l) = l else { break };
            // libtest writes `test … ... ` without a line break before the child's
            // output; the child starts the mark on a new line, but the comparison
            // accepts both forms.
            if l.trim_end() == expected || l.ends_with(expected) {
                let _ = tx.send(());
                // keeps reading so the child never blocks on the pipe
            }
        }
    });
    if rx.recv_timeout(Duration::from_secs(60)).is_err() {
        let _ = f.kill();
        let st = f.wait();
        panic!("o filho não chegou a {expected}: {st:?}");
    }
}

fn kill(mut f: Child) {
    f.kill().unwrap(); // SIGKILL
    let st = f.wait().unwrap();
    assert!(!st.success(), "o filho tinha de morrer pelo SIGKILL");
}

/// SIGKILL exactly at each point: before the rename the whole previous version stays,
/// after it, the whole new one. The temporary file left behind is encrypted (without the
/// clear mark).
#[test]
fn v2_9_sigkill_at_each_point() {
    let mark = random_hex();
    let old = vault().seal(&content(&mark, 0, LARGE_SIZE)).unwrap();
    let new = vault().seal(&content(&mark, 1, LARGE_SIZE)).unwrap();
    for (i, target) in POINTS.iter().enumerate() {
        let d = Dir::new("sigkill-ponto");
        write(&d.vault(), &old).unwrap();
        fs::write(d.0.join("novo"), &new).unwrap();
        let mut f = child(&format!("ponto:{i}"), &d.0);
        wait_line(&mut f, "WARD-PONTO");
        kill(f);

        let left = fs::read(d.vault()).unwrap();
        let temps = d.temp_files();
        if i < BEFORE_RENAME {
            assert_eq!(left, old, "ponto {target:?}");
            assert_eq!(temps.len(), 1, "ponto {target:?}");
        } else {
            assert_eq!(left, new, "ponto {target:?}");
            assert!(temps.is_empty(), "ponto {target:?}");
        }
        for t in &temps {
            let b = fs::read(t).unwrap();
            assert!(!contains(&b, mark.as_bytes()), "temporário em claro");
            assert!(new.starts_with(&b), "temporário é prefixo do cifrado novo");
            assert_eq!(mode(t), 0o600);
        }
        assert_eq!(clean_temp_files(&d.vault()).unwrap(), temps.len());
        assert!(d.temp_files().is_empty());
        // The vault that stayed can be written again.
        write(&d.vault(), &old).unwrap();
    }
    assert_eq!(open_content(&old), content(&mark, 0, LARGE_SIZE));
    assert_eq!(open_content(&new), content(&mark, 1, LARGE_SIZE));
}

/// SIGKILL at 200 random points of a child that writes nonstop: the destination is
/// always one of the whole versions, and each version seen opens with the password.
#[test]
fn v2_9_sigkill_at_200_random_points() {
    const DEATHS: usize = 200;
    let d = Dir::new("sigkill-aleatorio");
    let mark = random_hex();
    let versions: Vec<Vec<u8>> = (0..3)
        .map(|k| vault().seal(&content(&mark, k, LARGE_SIZE)).unwrap())
        .collect();
    for (k, v) in versions.iter().enumerate() {
        fs::write(d.0.join(format!("v{k}")), v).unwrap();
    }
    write(&d.vault(), &versions[0]).unwrap();

    // Duration of one write here, to spread the deaths over several writes.
    let t0 = Instant::now();
    for v in &versions {
        write(&d.vault(), v).unwrap();
    }
    let one = (t0.elapsed() / 3).max(Duration::from_millis(1));
    let window = (one * 3).as_micros() as u64;

    let mut seen = [0usize; 3];
    let mut with_temp = 0;
    for _ in 0..DEATHS {
        let mut f = child("laco", &d.0);
        wait_line(&mut f, "WARD-PRONTO");
        std::thread::sleep(Duration::from_micros(random_u64() % window));
        kill(f);

        let left = fs::read(d.vault()).unwrap();
        let k = versions
            .iter()
            .position(|v| *v == left)
            .expect("o cofre não é nenhuma versão inteira");
        seen[k] += 1;
        let temps = d.temp_files();
        assert!(temps.len() <= 1);
        for t in &temps {
            let b = fs::read(t).unwrap();
            assert!(!contains(&b, mark.as_bytes()), "temporário em claro");
        }
        if !temps.is_empty() {
            with_temp += 1;
        }
        clean_temp_files(&d.vault()).unwrap();
    }
    eprintln!(
        "v2_9: {DEATHS} mortes, {with_temp} no meio de uma gravação, versões vistas {seen:?}, gravação ~{one:?}"
    );
    // The deaths did fall inside the writes, and the writes moved forward.
    assert!(with_temp >= DEATHS / 4, "poucas mortes no meio da gravação");
    assert!(seen.iter().filter(|&&n| n > 0).count() >= 2);
    for (k, n) in seen.iter().enumerate() {
        if *n > 0 {
            assert_eq!(open_content(&versions[k]), content(&mark, k, LARGE_SIZE));
        }
    }
}

// --- strace: order of the calls and nothing in clear (V2.10, V2.11) ---------------------

/// A system call of the strace log (`-f`: each line starts with the pid).
struct Call {
    name: String,
    args: String,
    ret: Option<i64>,
}

fn read_log(log: &str) -> Vec<Call> {
    let mut v = Vec::new();
    for line in log.lines() {
        let rest = line
            .split_once(' ')
            .map(|(pid, r)| {
                if pid.chars().all(|c| c.is_ascii_digit()) {
                    r
                } else {
                    line
                }
            })
            .unwrap_or(line)
            .trim_start();
        let Some(paren) = rest.find('(') else {
            continue;
        };
        let name = &rest[..paren];
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue; // `<... resumed>`, `+++ exited`, signals
        }
        // strace aligns the return value with spaces: `fsync(3)        = 0`.
        let ret = rest
            .rsplit_once(" = ")
            .filter(|(call, _)| call.trim_end().ends_with(')'))
            .and_then(|(_, r)| r.split_whitespace().next())
            .and_then(|r| r.parse().ok());
        v.push(Call {
            name: name.to_owned(),
            args: rest[paren + 1..].to_owned(),
            ret,
        });
    }
    v
}

/// `n`-th argument between quotes (from 0).
fn path_arg(args: &str, n: usize) -> Option<String> {
    let parts: Vec<&str> = args.split('"').collect();
    parts.get(1 + 2 * n).map(|s| (*s).to_owned())
}

fn fd_arg(args: &str) -> Option<i64> {
    args.split([',', ')']).next()?.trim().parse().ok()
}

#[test]
fn v2_10_v2_11_strace_order_and_nothing_in_clear() {
    assert!(
        Path::new(STRACE).exists(),
        "V2.10/V2.11 precisam do strace em {STRACE} (apt install strace)"
    );
    let d = Dir::new("strace");
    let mark = format!("WARD-MARCA-CLARO-{}", random_hex());
    let control = format!("WARD-CONTROLE-{}", random_hex());
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
            CHILD_NAME,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(ENV_MODE, "strace")
        .env(ENV_DIR, &d.0)
        .env(ENV_MARK, &mark)
        .env(ENV_CONTROL, &control)
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
    let mut text = String::new();
    File::open(&log).unwrap().read_to_string(&mut text).unwrap();

    // V2.11: the detector sees a whole write (control), and the clear mark does not
    // appear in any written buffer, neither in the vault nor in a temporary file.
    assert!(
        text.contains(&control),
        "o strace não mostrou a escrita de controle"
    );
    assert!(!text.contains(&mark), "conteúdo em claro escrito: V2.11");
    assert!(
        !text.contains(FAKE_SECRET),
        "segredo do conteúdo escrito: V2.11"
    );
    assert!(!text.contains(PASSWORD), "senha mestra escrita: V2.11");
    let vault_bytes = fs::read(d.vault()).unwrap();
    assert!(!contains(&vault_bytes, mark.as_bytes()));
    assert!(d.temp_files().is_empty());
    for name in d.names() {
        if name != "strace.log" {
            let b = fs::read(d.0.join(&name)).unwrap();
            assert!(
                !contains(&b, mark.as_bytes()),
                "{name} tem conteúdo em claro"
            );
        }
    }

    // V2.10: temporary file in the same directory → writes → its fsync → rename → fsync
    // of the directory.
    let dir = d.0.to_string_lossy().into_owned();
    let destination = d.vault().to_string_lossy().into_owned();
    let calls = read_log(&text);
    let mut i_tmp = None;
    let mut fd_tmp = -1;
    let mut tmp_path = String::new();
    for (i, c) in calls.iter().enumerate() {
        if c.name == "openat"
            && path_arg(&c.args, 0).is_some_and(|p| p.contains("/.cofre.ward.ward-tmp-"))
        {
            assert!(i_tmp.is_none(), "dois temporários numa gravação");
            let p = path_arg(&c.args, 0).unwrap();
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
            tmp_path = p;
        }
    }
    let i_tmp = i_tmp.expect("o temporário não foi aberto");
    let after = |from: usize, f: &dyn Fn(&Call) -> bool| -> usize {
        from + 1
            + calls[from + 1..]
                .iter()
                .position(f)
                .expect("chamada esperada não aconteceu")
    };
    let i_fsync = after(i_tmp, &|c| {
        c.name == "fsync" && fd_arg(&c.args) == Some(fd_tmp)
    });
    let writes: Vec<usize> = (i_tmp + 1..i_fsync)
        .filter(|&i| calls[i].name == "write" && fd_arg(&calls[i].args) == Some(fd_tmp))
        .collect();
    assert!(!writes.is_empty(), "nenhuma escrita no temporário");
    let i_rename = after(i_fsync, &|c| c.name.starts_with("rename"));
    let r = &calls[i_rename];
    assert_eq!(r.ret, Some(0));
    assert!(r.args.contains(&format!("\"{tmp_path}\"")), "{}", r.args);
    assert!(r.args.contains(&format!("\"{destination}\"")), "{}", r.args);
    let i_dir = after(i_rename, &|c| {
        c.name == "openat" && path_arg(&c.args, 0).as_deref() == Some(dir.as_str())
    });
    let fd_dir = calls[i_dir].ret.unwrap();
    let i_fsync_dir = after(i_dir, &|c| {
        c.name == "fsync" && fd_arg(&c.args) == Some(fd_dir)
    });
    assert_eq!(calls[i_fsync_dir].ret, Some(0));

    // Nobody else opened a file for writing in the vault directory, and the destination
    // was never opened for writing (only replaced by the rename).
    for c in &calls {
        if !matches!(c.name.as_str(), "openat" | "open" | "creat") {
            continue;
        }
        let Some(p) = path_arg(&c.args, 0) else {
            continue;
        };
        let writes = c.args.contains("O_WRONLY") || c.args.contains("O_RDWR");
        if writes && Path::new(&p).parent() == Some(d.0.as_path()) {
            assert_eq!(p, tmp_path, "arquivo aberto para escrita: {p}");
        }
    }
    // No write to the temporary file after its fsync.
    assert!(
        !calls[i_fsync + 1..]
            .iter()
            .take_while(|c| !c.name.starts_with("rename"))
            .any(|c| c.name == "write" && fd_arg(&c.args) == Some(fd_tmp))
    );
    assert_eq!(open_content(&vault_bytes), content(&mark, 1, 4096));
}

// --- Generation burned on sealing (D53, V2.44 rewritten, V2.53) -------------------------

/// The test vault of D15 (the same entries as the body tests). The body test module is
/// private to it; including the file again here avoids opening the visibility of
/// `corpo::testes` only for the write tests.
#[allow(clippy::duplicate_mod)]
#[path = "../corpo/cofre_teste.rs"]
mod cofre_teste;
use crate as wc;

/// Generation written in the file (opens with no minimum nor burned).
fn generation_on_disk(path: &Path) -> u64 {
    let bytes = fs::read(path).unwrap();
    let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
    crate::corpo::abrir_entradas(&bytes, &password())
        .unwrap()
        .0
        .generation()
}

fn new_vault() -> OpenVault {
    let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
    OpenVault::new(&password(), config()).unwrap()
}

/// The same point, without looking at how much was already written.
fn same_point(a: Point, b: Point) -> bool {
    std::mem::discriminant(&a) == std::mem::discriminant(&b)
}

/// Error and `Debug` of the error without a value of the test vault.
fn without_value(text: &str) {
    for s in cofre_teste::SEGREDOS {
        assert!(!text.contains(s), "erro com segredo de teste");
    }
    assert!(!text.contains("falso") && !text.contains("acme"), "{text}");
}

/// V2.44 (rewritten, D53): failure injected at each point of the write (V2.9). The open
/// vault keeps the generation it sealed (n+1) even if the write fails before the rename
/// (the file stays at n) or after it (the file already has n+1); the next write writes
/// n+2. No pair of sealed files has the same generation.
#[test]
fn v2_44_failure_at_each_point_burns_the_generation() {
    let entries = cofre_teste::cofre_de_teste();
    for (i, target) in POINTS.iter().enumerate() {
        let d = Dir::new("queimada");
        let mut c = new_vault();
        write_entries(&d.vault(), &mut c, &entries).unwrap();
        let n = c.generation();
        assert_eq!(n, 1);
        let mut reached = false;
        let r = write_entries_with_points(&d.vault(), &mut c, &entries, &mut |p| {
            if same_point(p, *target) && !reached {
                reached = true;
                return Err(io::Error::from(io::ErrorKind::StorageFull));
            }
            Ok(())
        });
        assert!(reached, "ponto {target:?} não aconteceu");
        verify_io_error(r.map(|_| ()), i, &d.vault(), target);
        assert_eq!(
            c.generation(),
            n + 1,
            "{target:?}: a geração selada tem de ficar"
        );
        let on_disk = generation_on_disk(&d.vault());
        if i < BEFORE_RENAME {
            assert_eq!(on_disk, n, "{target:?}: a versão anterior tem de ficar");
        } else {
            assert_eq!(on_disk, n + 1, "{target:?}: a versão nova já está no lugar");
        }
        assert!(d.temp_files().is_empty(), "{target:?}: temporário ficou");
        // The next write uses a number that was never sealed.
        write_entries(&d.vault(), &mut c, &entries).unwrap();
        assert_eq!(c.generation(), n + 2, "{target:?}");
        assert_eq!(generation_on_disk(&d.vault()), n + 2, "{target:?}");
    }
}

/// D53: a body that does not serialize never gets to seal, so it neither burns nor
/// touches the disk.
#[test]
fn d53_body_that_does_not_serialize_does_not_burn() {
    let d = Dir::new("nao-queima");
    let mut c = new_vault();
    write_entries(&d.vault(), &mut c, &[]).unwrap();
    let before = fs::read(d.vault()).unwrap();
    // Two entries with the same name do not become a body.
    let one = cofre_teste::cofre_de_teste().remove(0);
    let two = cofre_teste::cofre_de_teste().remove(0);
    let r = write_entries(&d.vault(), &mut c, &[one, two]);
    assert!(matches!(r, Err(WriteError::Body(_))), "{r:?}");
    assert_eq!(c.generation(), 1);
    assert!(fs::read(d.vault()).unwrap() == before);
    assert!(d.temp_files().is_empty());
}

/// V2.53 (D53, the auditor's generation repro): the attacker removes the read
/// permission of the directory (0300), so the directory fsync fails after the rename.
/// Write A (one entry less) returns an error with the open vault at n+1; B (the entry
/// back) writes n+2; the copy of A put in the place of B is refused with `Retrocesso` by
/// the minimum n+2, without entry and without value. Before D53, B wrote n+1 again and A
/// opened with the minimum.
#[test]
fn v2_53_directory_fsync_fails_and_the_generation_does_not_repeat() {
    let d = Dir::new("v2-53");
    let mut c = new_vault();
    let mut entries = cofre_teste::cofre_de_teste();
    write_entries(&d.vault(), &mut c, &entries).unwrap();
    let n = c.generation();
    let file_n = fs::read(d.vault()).unwrap();
    let last = entries.pop().unwrap();

    fs::set_permissions(&d.0, fs::Permissions::from_mode(0o300)).unwrap();
    // Root reads the directory even with 0300: then the attack does not reproduce, and
    // the case is covered by the injection of
    // `v2_44_failure_at_each_point_burns_the_generation`.
    if File::open(&d.0).is_ok() {
        fs::set_permissions(&d.0, fs::Permissions::from_mode(0o700)).unwrap();
        eprintln!("v2_53: o processo lê diretório 0300 (root?); ataque não reproduz aqui");
        return;
    }
    let r = write_entries(&d.vault(), &mut c, &entries);
    fs::set_permissions(&d.0, fs::Permissions::from_mode(0o700)).unwrap();
    let in_place = FileIdentity::from(&fs::metadata(d.vault()).unwrap());
    assert_eq!(
        r,
        Err(WriteError::Io {
            step: Step::FsyncDir,
            kind: io::ErrorKind::PermissionDenied,
            identity: Some(in_place),
        })
    );
    assert_eq!(c.generation(), n + 1);
    let file_a = fs::read(d.vault()).unwrap();
    assert_eq!(generation_on_disk(&d.vault()), n + 1);

    entries.push(last);
    write_entries(&d.vault(), &mut c, &entries).unwrap();
    assert_eq!(c.generation(), n + 2);
    let file_b = fs::read(d.vault()).unwrap();
    assert_eq!(generation_on_disk(&d.vault()), n + 2);

    // No pair of sealed files in this sequence has the same generation.
    let generations: Vec<u64> = [&file_n, &file_a, &file_b]
        .iter()
        .map(|b| {
            let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
            crate::corpo::abrir_entradas(b, &password())
                .unwrap()
                .0
                .generation()
        })
        .collect();
    assert_eq!(generations, [n, n + 1, n + 2]);

    // The attacker swaps B for A; the daemon opens with the largest generation seen
    // (n+2).
    {
        let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
        match crate::corpo::abrir_entradas_desde(&file_a, &password(), n + 2, n + 2) {
            Ok(_) => panic!("a cópia de A abriu com o mínimo de B"),
            Err(e) => {
                assert_eq!(e, crate::corpo::ErroAbrir::Retrocesso);
                without_value(&format!("{e} {e:?}"));
            }
        }
    }

    // Reopened with the burned generation the daemon kept, A (n+1 ≥ n) stays at n+2 and
    // the next write seals n+3, never n+2 again.
    fs::write(d.vault(), &file_a).unwrap();
    let (mut reopened, read) = {
        let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
        crate::corpo::abrir_entradas_desde(&file_a, &password(), n, n + 2).unwrap()
    };
    assert_eq!(reopened.generation(), n + 2);
    write_entries(&d.vault(), &mut reopened, &read).unwrap();
    assert_eq!(reopened.generation(), n + 3);
    assert_eq!(generation_on_disk(&d.vault()), n + 3);

    // The password change with burned n+5 over the file at n+3 seals n+6.
    let new = MasterPassword::new("falso-mestra-ward-02".to_owned());
    let changed = {
        let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
        change_password(&d.vault(), &password(), &new, n + 3, n + 5)
            .unwrap()
            .0
    };
    assert_eq!(changed.generation(), n + 6);
    let bytes = fs::read(d.vault()).unwrap();
    let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
    let (opened, _) = crate::corpo::abrir_entradas(&bytes, &new).unwrap();
    assert_eq!(opened.generation(), n + 6);
}

/// D53: the burned generation never lowers the generation: opening with a burned one
/// lower than the disk's keeps the disk's, and the password change with a lower burned
/// one seals the disk's + 1.
#[test]
fn d53_burned_lower_than_the_disk_does_not_go_down() {
    let d = Dir::new("queimada-menor");
    let mut c = new_vault();
    write_entries(&d.vault(), &mut c, &[]).unwrap();
    write_entries(&d.vault(), &mut c, &[]).unwrap();
    write_entries(&d.vault(), &mut c, &[]).unwrap();
    assert_eq!(c.generation(), 3);
    let bytes = fs::read(d.vault()).unwrap();
    let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
    let (opened, _) = crate::corpo::abrir_entradas_desde(&bytes, &password(), 0, 1).unwrap();
    assert_eq!(opened.generation(), 3);
    let new = MasterPassword::new("falso-mestra-ward-02".to_owned());
    let (changed, _) = change_password(&d.vault(), &password(), &new, 3, 2).unwrap();
    assert_eq!(changed.generation(), 4);
}

/// D62 (S7): the returned identity is that of the written file (from the temporary
/// file's fd), and a swap right after the rename does not change what was returned: the
/// caller notices the swap by comparing with the disk, instead of accepting the swapped
/// file as Ward's.
#[test]
fn d62_identity_from_the_fd_and_swap_right_after_the_rename() {
    let d = Dir::new("d62-identidade");
    let mut c = new_vault();
    let id = write_entries(&d.vault(), &mut c, &[]).unwrap();
    assert_eq!(id, FileIdentity::from(&fs::metadata(d.vault()).unwrap()));

    // Hook right after the rename: another process puts another file in place.
    let other = d.0.join("outro");
    let mut swapped = false;
    let r = write_entries_with_points(&d.vault(), &mut c, &[], &mut |p| {
        if p == Point::AfterRename {
            let bytes = fs::read(d.vault()).unwrap();
            fs::write(&other, &bytes).unwrap();
            fs::rename(&other, d.vault()).unwrap();
            swapped = true;
        }
        Ok(())
    });
    assert!(swapped);
    let returned = r.unwrap();
    let on_disk = FileIdentity::from(&fs::metadata(d.vault()).unwrap());
    assert_ne!(
        returned, on_disk,
        "a troca depois do rename passaria como do Ward"
    );
    without_value(&format!("{returned:?}"));
}

/// D62 (V3.35): a new vault created where the old state burned generation 7 seals 8 on
/// the first write; the generation never goes down; at `u64::MAX`, the write is refused.
#[test]
fn d62_new_vault_after_the_burned_generation() {
    let d = Dir::new("d62-geracao");
    let mut c = new_vault().after_generation(7);
    assert_eq!(c.generation(), 7);
    write_entries(&d.vault(), &mut c, &[]).unwrap();
    assert_eq!(c.generation(), 8);
    assert_eq!(generation_on_disk(&d.vault()), 8);
    let c = c.after_generation(3);
    assert_eq!(c.generation(), 8, "nunca desce");
    let mut full = new_vault().after_generation(u64::MAX);
    assert_eq!(
        write_entries(&d.vault(), &mut full, &[]),
        Err(WriteError::GenerationExhausted)
    );
    assert_eq!(generation_on_disk(&d.vault()), 8);
}

// --- D65 d (V3.41): the V2.2 and V2.9 tests with the v4 body and the dates --------------

/// Three large secure notes (no delivery, outside the mask budget) with dates: the v4
/// body goes past `LARGE_SIZE`, so there is a point in the middle of the write. The three
/// entries have the same size.
fn v4_body_with_dates(version: u64) -> zeroize::Zeroizing<Vec<u8>> {
    use crate::corpo::serializar;
    use crate::entry::{DataDraft, Dates, Draft, Secret};
    let entries: Vec<_> = (0..3u64)
        .map(|i| {
            let mut e = Draft {
                name: format!("nota-{i}"),
                group: "g".to_owned(),
                environment: "dev".to_owned(),
                project: String::new(),
                tags: Vec::new(),
                data: DataDraft::SecureNote {
                    text: Some(Secret::from_bytes(vec![b'n'; crate::limits::MAX_NOTE])),
                },
                extras: Vec::new(),
                attachments: Vec::new(),
            }
            .validate()
            .unwrap();
            let created = 1_762_000_000 + i;
            e.set_dates(
                Dates::new_stamp(created)
                    .unwrap()
                    .modified(created + version)
                    .unwrap(),
            );
            e
        })
        .collect();
    serializar(vault().config(), 1 + version, &entries).unwrap()
}

fn body_dates(content: &[u8]) -> Vec<(Option<u64>, Option<u64>)> {
    crate::corpo::desserializar(content)
        .unwrap()
        .2
        .iter()
        .map(|e| (e.dates().created_at(), e.dates().modified_at()))
        .collect()
}

/// V3.41/V2.9 with v4: SIGKILL at each point of the write of a body with dates leaves
/// the whole previous or new version, and the dates come back as they were written.
#[test]
fn d65_v4_sigkill_at_each_point_keeps_the_dates() {
    let old_body = v4_body_with_dates(0);
    let new_body = v4_body_with_dates(86_400);
    assert!(new_body.len() > LARGE_SIZE);
    let old = vault().seal(&old_body).unwrap();
    let new = vault().seal(&new_body).unwrap();
    for (i, target) in POINTS.iter().enumerate() {
        let d = Dir::new("d65-sigkill");
        write(&d.vault(), &old).unwrap();
        fs::write(d.0.join("novo"), &new).unwrap();
        let mut f = child(&format!("ponto:{i}"), &d.0);
        wait_line(&mut f, "WARD-PONTO");
        kill(f);
        let left = fs::read(d.vault()).unwrap();
        let expected = if i < BEFORE_RENAME { &old } else { &new };
        assert!(left == *expected, "ponto {target:?}");
        clean_temp_files(&d.vault()).unwrap();
    }
    let read = body_dates(&open_content(&new));
    assert_eq!(read[0], (Some(1_762_000_000), Some(1_762_086_400)));
    let read = body_dates(&open_content(&old));
    assert_eq!(read[2], (Some(1_762_000_002), Some(1_762_000_002)));
}

/// V3.41/V2.2 with v4: one bit flipped in each encrypted byte of the dates (the 16 that
/// close each of the 3 entries) makes the body tag refuse, and the file with one of
/// those bytes flipped does not open even with the right password (the same error as
/// V2.1).
#[test]
fn d65_v4_byte_changed_in_the_dates_fails_the_opening() {
    let body = v4_body_with_dates(7);
    let empty = crate::corpo::serializar(vault().config(), 8, &[])
        .unwrap()
        .len();
    let size = (body.len() - empty) / 3;
    assert_eq!(empty + 3 * size, body.len());
    // The cipher is a stream cipher: byte i of the body is at byte HEADER_SIZE + i of
    // the file.
    let mut positions = Vec::new();
    for k in 1..=3 {
        let end = body.len() - (3 - k) * size;
        assert_eq!(
            body[end - 8..end],
            (1_762_000_000 + k as u64 - 1 + 7).to_le_bytes(),
            "a alteração fecha a entrada"
        );
        positions.extend((end - 16..end).map(|p| crate::format::HEADER_SIZE + p));
    }
    assert_eq!(positions.len(), 48);
    let file = vault().seal(&body).unwrap();
    assert!(
        vault().authenticates(&file),
        "controle: o original autentica"
    );
    for p in &positions {
        let mut a = file.clone();
        a[*p] ^= 1 << (p % 8);
        assert!(
            !vault().authenticates(&a),
            "byte {p} das datas trocado e aceito"
        );
    }
    let mut a = file.clone();
    a[positions[5]] ^= 0x01;
    let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(
        open(&a, &password()).map(|_| ()),
        Err(crate::format::FormatError::PasswordOrFile)
    );
}
