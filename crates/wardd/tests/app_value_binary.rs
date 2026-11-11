//! V3.39 and V3.40 (D4, D65 b and c; Phase 22c), through the test daemon binary with
//! the test vault (the 22 entries of `test_vault.rs`, only fake values):
//!
//! - a Python client in the agent role (which unlocks with the test password from
//!   stdin) and another that claims to be the extension host (`argv[0]` = `ward-nm`,
//!   outside Chrome) ask to reveal, copy and take the TOTP of `acme-pg-dev`, the
//!   blocked `gama` entry and `acme-totp-dev`: all of them get "not authorized", and
//!   no answer has a secret; the agent's list comes without level, summary or user;
//! - the test app opened by the open-app operation (app role through the launcher,
//!   D63) reveals the password of `acme-pg-dev` and that of the blocked entry
//!   without a PIN and
//!   without a window, copies, takes the TOTP, gets "field does not exist", the list
//!   with the summary and, after locking, "locked" (the app compares the values and
//!   only prints whether they are equal);
//! - the daemon log has the reveal, copy and TOTP lines (operation, entry and field)
//!   and no
//!   secret of the test vault (not even in base64), nor any pattern of
//!   `.dev/variations.txt` when the file exists (`make dev-variations`).

// m2:Phase 54a: without `test-mode` the binaries this test runs are the release ones,
// which only know the system socket (`/run/ward/<uid>/wardd.sock`, the installed
// daemon). The build fails, loudly, instead of a test reaching it.
#[cfg(not(feature = "test-mode"))]
compile_error!(
    "este teste exige a feature test-mode (cargo test --all-features ou --features test-mode): sem ela os binários do teste são os de release, que só conhecem o socket do sistema"
);

mod common;

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::{Dir, b64};
use ward_core::atomic_write::write_entries;
use ward_core::format::{MasterPassword, OpenVault};
use ward_core::protocol::{self, Response, ResponseError, Role};

// The test vault through the public API (the same as the test app's and the fake
// daemon's).
use ward_core as wc;
#[path = "../../ward-core/src/body/test_vault.rs"]
mod test_vault;

const WARDD: &str = env!("CARGO_BIN_EXE_wardd");
const TEST_APP: &str = env!("CARGO_BIN_EXE_wardd-test-app");
const PYTHON: &str = "/usr/bin/python3";

/// Raw Python client: reads the password (base64) from stdin, unlocks if asked and
/// prints one line per answer (`<op> <entry> <JSON body>`). Nothing of the vault in the
/// argv.
const CLIENT: &str = r#"
import json, socket, struct, sys
s = socket.socket(socket.AF_UNIX)
s.connect(sys.argv[1] + "/wardd.sock")
s.settimeout(60)
def ler(n):
    b = b""
    while len(b) < n:
        p = s.recv(n - len(b))
        if not p:
            raise SystemExit("conexão fechada")
        b += p
    return b
def pedir(op, dados):
    q = json.dumps({"v": 2, "op": op, "data": dados}).encode()
    s.sendall(struct.pack(">I", len(q)) + q)
    return ler(struct.unpack(">I", ler(4))[0]).decode()
print("estado -", pedir("status", {}))
if sys.argv[2] == "destravar":
    senha = sys.stdin.readline().strip()
    print("destravar -", pedir("unlock", {"password": senha}))
for op in ("reveal", "copy"):
    for e in ("acme-pg-dev", "gama-bloqueada"):
        print(op, e, pedir(op, {"entry": e, "field": "password"}))
    print(op, "gama-generico", pedir(op, {"entry": "gama-generico", "extra": "CHAVE_LICENCA"}))
for e in ("acme-totp-dev", "gama-bloqueada"):
    print("totp", e, pedir("totp", {"entry": e}))
print("lista -", pedir("list", {}))
"#;

struct SocketDir(PathBuf);

impl SocketDir {
    fn new(label: &str) -> SocketDir {
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| {
                PathBuf::from(format!("/run/user/{}", ward_platform::effective_uid()))
            });
        let p = base.join(format!("wardd-v-{}-{label}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        SocketDir(p)
    }
}

impl Drop for SocketDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Wardd(Child);

impl Drop for Wardd {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_for(mut f: impl FnMut() -> bool, what: &str) {
    let end = Instant::now() + Duration::from_secs(60);
    while !f() {
        assert!(Instant::now() < end, "esperando {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn read(p: &Path) -> String {
    fs::read_to_string(p).unwrap_or_default()
}

/// The secrets of the test vault and the base64 of each one.
fn needles() -> Vec<String> {
    let mut v = Vec::new();
    for s in test_vault::SECRETS {
        v.push((*s).to_owned());
        v.push(b64(s.as_bytes()));
    }
    v.push(test_vault::fake_key());
    // The variations list of `make dev-variations`, if it exists (the D54 oracle).
    let variations = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.dev/variations.txt");
    if let Ok(t) = fs::read_to_string(variations) {
        v.extend(t.lines().filter(|l| l.len() >= 6).map(str::to_owned));
    }
    v
}

fn no_secret(text: &str, place: &str) {
    for a in needles() {
        // Never prints the pattern: only where it was found.
        assert!(!text.contains(&a), "padrão de segredo em {place}");
    }
}

/// Runs the Python client as the agent (or with the extension host's `argv[0]`).
fn python_client(sock: &Path, unlock: bool, name: &str) -> String {
    let mut c = Command::new(PYTHON);
    c.arg0(name)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .args(["-I", "-c", CLIENT])
        .arg(sock)
        .arg(if unlock { "destravar" } else { "nao" })
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = c.spawn().unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        if unlock {
            stdin
                .write_all(format!("{}\n", b64(test_vault::MASTER_PASSWORD.as_bytes())).as_bytes())
                .unwrap();
        }
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "cliente Python: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn send(s: &mut UnixStream, json: &str) -> Response {
    s.write_all(&(json.len() as u32).to_be_bytes())
        .and_then(|()| s.write_all(json.as_bytes()))
        .unwrap();
    let mut n = [0u8; 4];
    s.read_exact(&mut n).unwrap();
    let mut b = vec![0u8; u32::from_be_bytes(n) as usize];
    s.read_exact(&mut b).unwrap();
    protocol::decode_response(Role::Agent, &b).unwrap()
}

#[test]
fn v3_39_v3_40_app_values_and_list_through_the_test_daemon() {
    let repo = Dir::new("repo-valor");
    let dev = repo.0.join(".dev");
    fs::DirBuilder::new().mode(0o700).create(&dev).unwrap();
    fs::DirBuilder::new()
        .mode(0o700)
        .create(repo.0.join("bin"))
        .unwrap();
    let app = repo.0.join("bin/wardd-test-app");
    fs::hard_link(TEST_APP, &app).unwrap();

    // The test vault and the state at generation 0 (as `dev/daemon.sh` does, D39).
    wardd::files::write_state(
        &dev.join("test-vault.state"),
        wardd::files::Generations { seen: 0, burned: 0 },
    )
    .unwrap();
    let mut entries = test_vault::test_vault_entries();
    entries.sort_by(|a, b| a.name().cmp(b.name()));
    let password = MasterPassword::new(test_vault::MASTER_PASSWORD.to_owned());
    let mut vault = OpenVault::new(&password, test_vault::new_config()).unwrap();
    write_entries(&dev.join("test-vault.ward"), &mut vault, &entries).unwrap();
    drop(vault);

    let sock = SocketDir::new("v339");
    let log = dev.join("wardd.log");
    let _w = Wardd(
        Command::new(WARDD)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C.UTF-8")
            .env("WARD_DEV_DIR", &dev)
            .env("WARD_DEV_SOCKET_DIR", &sock.0)
            .env("WARD_DEV_APP", &app)
            .env("RUST_LOG", "trace")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(fs::File::create(&log).unwrap())
            .spawn()
            .unwrap(),
    );
    let socket = sock.0.join("wardd.sock");
    wait_for(|| socket.exists(), "o socket do daemon");

    // Agent (unlocks with the test password) and "extension" outside Chrome.
    for (unlock, name) in [(true, "python3"), (false, "ward-nm")] {
        let output = python_client(&sock.0, unlock, name);
        let lines: Vec<&str> = output.lines().collect();
        assert!(
            lines[0].contains(r#""role":"agent""#),
            "{name}: {}",
            lines[0]
        );
        if unlock {
            assert!(lines[1].starts_with("destravar - "));
            assert!(lines[1].contains(r#""type":"ok""#), "{}", lines[1]);
        }
        let attacks: Vec<&&str> = lines
            .iter()
            .filter(|l| {
                ["reveal ", "copy ", "totp "]
                    .iter()
                    .any(|p| l.starts_with(p))
            })
            .collect();
        assert_eq!(attacks.len(), 8, "{name}");
        for l in attacks {
            let (request, answer) = l.split_at(l.find('{').unwrap());
            let r = protocol::decode_response(Role::Agent, answer.as_bytes());
            assert!(
                matches!(r, Ok(Response::Error(ResponseError::Unauthorized))),
                "{name} {request}"
            );
        }
        let list = lines.iter().find(|l| l.starts_with("lista - ")).unwrap();
        // The agent that unlocked gets the list; the other one, already unlocked too.
        assert!(list.contains(r#""type":"list""#), "{list}");
        for forbidden in [
            r#""summary""#,
            r#""level""#,
            r#""username""#,
            "gama-bloqueada",
        ] {
            assert!(
                !list.contains(forbidden),
                "{name}: {forbidden} na lista do agente"
            );
        }
        no_secret(&output, "resposta ao agente");
    }

    // The test app, opened by the daemon: app role, values without a PIN or approval.
    let mut c = UnixStream::connect(&socket).unwrap();
    c.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
    assert!(matches!(
        send(&mut c, r#"{"v":2,"op":"open_app","data":{}}"#),
        Response::Ok
    ));
    let app_output = dev.join("logs/wardd-test-app.log");
    wait_for(|| read(&app_output).contains("fim=1"), "a saída do app");
    let text = read(&app_output);
    for line in [
        "papel=app",
        "revelar=secret",
        "revelar_pg=igual",
        "revelar_bloqueada=igual",
        "copiar_pg=igual",
        "totp=igual",
        "campo_inexistente=field_not_found",
        "lista=resumo",
        "travar=ok",
        "revelar_travado=locked",
    ] {
        assert!(text.lines().any(|l| l == line), "falta {line:?}:\n{text}");
    }
    no_secret(&text, "saída do app de teste");

    // The log: the operation, the entry and the field, never the value.
    let daemon = read(&log);
    no_secret(&daemon, "log do daemon");
    for line in [
        "wardd: reveal acme-pg-dev password",
        "wardd: reveal gama-bloqueada password",
        "wardd: copy acme-pg-dev password",
        "wardd: totp acme-totp-dev",
    ] {
        assert!(daemon.lines().any(|l| l == line), "falta {line:?} no log");
    }
    // No value revealed through the agent or to the extension.
    assert!(!daemon.contains("reveal gama-generico"));
}
