//! V3.39 e V3.40 (D4, D65 b e c; Fase 22c), pelo binário do daemon de teste com o
//! cofre de teste (as 22 entradas de `test_vault.rs`, só valores falsos):
//!
//! - um cliente Python no papel agente (que destrava com a senha de teste pelo stdin) e
//!   outro que se diz host da extensão (`argv[0]` = `ward-nm`, fora do Chrome) pedem
//!   `revelar`, `copiar` e `totp` de `acme-pg-dev`, `gama-bloqueada` e `acme-totp-dev`:
//!   todos recebem "não autorizado", e nenhuma resposta tem segredo; a `lista` do
//!   agente vem sem nível, sem resumo e sem usuário;
//! - o app de teste aberto pelo `abrir_app` (papel app pelo lançador, D63) revela a
//!   senha de `acme-pg-dev` e a de `gama-bloqueada` sem PIN e sem janela, copia, tira o
//!   TOTP, recebe "campo inexistente", a lista com resumo e, depois de travar,
//!   "travado" (o app compara os valores e só imprime `igual`);
//! - o log do daemon tem `revelar <entrada> <campo>`, `copiar …` e `totp …` e nenhum
//!   segredo do cofre de teste (nem em base64), nem padrão de `.dev/variations.txt`
//!   quando o arquivo existe (`make dev-variations`).

#![cfg(feature = "test-mode")]

mod comum;

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use comum::{Dir, b64};
use ward_core::atomic_write::write_entries;
use ward_core::format::{MasterPassword, OpenVault};
use ward_core::protocolo::{self, ErroResposta, Papel, Resposta};

// O cofre de teste pela API pública (o mesmo do app de teste e do daemon falso).
use ward_core as wc;
#[path = "../../ward-core/src/body/test_vault.rs"]
mod test_vault;

const WARDD: &str = env!("CARGO_BIN_EXE_wardd");
const APP_TESTE: &str = env!("CARGO_BIN_EXE_wardd-app-teste");
const PYTHON: &str = "/usr/bin/python3";

/// Cliente Python cru: lê a senha (base64) do stdin, destrava se pedirem e imprime uma
/// linha por resposta (`<op> <entrada> <corpo JSON>`). Nada do cofre no argv.
const CLIENTE: &str = r#"
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
    q = json.dumps({"v": 1, "op": op, "dados": dados}).encode()
    s.sendall(struct.pack(">I", len(q)) + q)
    return ler(struct.unpack(">I", ler(4))[0]).decode()
print("estado -", pedir("estado", {}))
if sys.argv[2] == "destravar":
    senha = sys.stdin.readline().strip()
    print("destravar -", pedir("destravar", {"senha": senha}))
for op in ("revelar", "copiar"):
    for e in ("acme-pg-dev", "gama-bloqueada"):
        print(op, e, pedir(op, {"entrada": e, "campo": "senha"}))
    print(op, "gama-generico", pedir(op, {"entrada": "gama-generico", "extra": "CHAVE_LICENCA"}))
for e in ("acme-totp-dev", "gama-bloqueada"):
    print("totp", e, pedir("totp", {"entrada": e}))
print("lista -", pedir("lista", {}))
"#;

struct DirSocket(PathBuf);

impl DirSocket {
    fn novo(rotulo: &str) -> DirSocket {
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| {
                PathBuf::from(format!("/run/user/{}", ward_platform::effective_uid()))
            });
        let p = base.join(format!("wardd-v-{}-{rotulo}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        DirSocket(p)
    }
}

impl Drop for DirSocket {
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

fn esperar(mut f: impl FnMut() -> bool, o_que: &str) {
    let fim = Instant::now() + Duration::from_secs(60);
    while !f() {
        assert!(Instant::now() < fim, "esperando {o_que}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn ler(p: &Path) -> String {
    fs::read_to_string(p).unwrap_or_default()
}

/// Os segredos do cofre de teste e o base64 de cada um.
fn agulhas() -> Vec<String> {
    let mut v = Vec::new();
    for s in test_vault::SECRETS {
        v.push((*s).to_owned());
        v.push(b64(s.as_bytes()));
    }
    v.push(test_vault::fake_key());
    // A lista de variações de `make dev-variations`, se existe (o oráculo de D54).
    let variacoes = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.dev/variations.txt");
    if let Ok(t) = fs::read_to_string(variacoes) {
        v.extend(t.lines().filter(|l| l.len() >= 6).map(str::to_owned));
    }
    v
}

fn sem_segredo(texto: &str, onde: &str) {
    for a in agulhas() {
        // Nunca imprime o padrão: só onde achou.
        assert!(!texto.contains(&a), "padrão de segredo em {onde}");
    }
}

/// Roda o cliente Python como agente (ou com `argv[0]` de host da extensão).
fn cliente_python(sock: &Path, destravar: bool, nome: &str) -> String {
    let mut c = Command::new(PYTHON);
    c.arg0(nome)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .args(["-I", "-c", CLIENTE])
        .arg(sock)
        .arg(if destravar { "destravar" } else { "nao" })
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut filho = c.spawn().unwrap();
    {
        let mut entrada = filho.stdin.take().unwrap();
        if destravar {
            entrada
                .write_all(format!("{}\n", b64(test_vault::MASTER_PASSWORD.as_bytes())).as_bytes())
                .unwrap();
        }
    }
    let saida = filho.wait_with_output().unwrap();
    assert!(
        saida.status.success(),
        "cliente Python: {}",
        String::from_utf8_lossy(&saida.stderr)
    );
    String::from_utf8(saida.stdout).unwrap()
}

fn mandar(s: &mut UnixStream, json: &str) -> Resposta {
    s.write_all(&(json.len() as u32).to_be_bytes())
        .and_then(|()| s.write_all(json.as_bytes()))
        .unwrap();
    let mut n = [0u8; 4];
    s.read_exact(&mut n).unwrap();
    let mut b = vec![0u8; u32::from_be_bytes(n) as usize];
    s.read_exact(&mut b).unwrap();
    protocolo::decodificar_resposta(Papel::Agente, &b).unwrap()
}

#[test]
fn v3_39_v3_40_valores_e_lista_do_app_pelo_daemon_de_teste() {
    let repo = Dir::novo("repo-valor");
    let dev = repo.0.join(".dev");
    fs::DirBuilder::new().mode(0o700).create(&dev).unwrap();
    fs::DirBuilder::new()
        .mode(0o700)
        .create(repo.0.join("bin"))
        .unwrap();
    let app = repo.0.join("bin/wardd-app-teste");
    fs::hard_link(APP_TESTE, &app).unwrap();

    // O cofre de teste e o estado na geração 0 (como o `dev/daemon.sh` faz, D39).
    wardd::arquivos::gravar_estado(
        &dev.join("test-vault.state"),
        wardd::arquivos::Geracoes {
            vista: 0,
            queimada: 0,
        },
    )
    .unwrap();
    let mut entradas = test_vault::test_vault_entries();
    entradas.sort_by(|a, b| a.name().cmp(b.name()));
    let senha = MasterPassword::new(test_vault::MASTER_PASSWORD.to_owned());
    let mut cofre = OpenVault::new(&senha, test_vault::new_config()).unwrap();
    write_entries(&dev.join("test-vault.ward"), &mut cofre, &entradas).unwrap();
    drop(cofre);

    let sock = DirSocket::novo("v339");
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
    esperar(|| socket.exists(), "o socket do daemon");

    // Agente (destrava com a senha de teste) e "extensão" fora do Chrome.
    for (destravar, nome) in [(true, "python3"), (false, "ward-nm")] {
        let saida = cliente_python(&sock.0, destravar, nome);
        let linhas: Vec<&str> = saida.lines().collect();
        assert!(
            linhas[0].contains(r#""papel":"agente""#),
            "{nome}: {}",
            linhas[0]
        );
        if destravar {
            assert!(linhas[1].starts_with("destravar - "));
            assert!(linhas[1].contains(r#""tipo":"ok""#), "{}", linhas[1]);
        }
        let ataques: Vec<&&str> = linhas
            .iter()
            .filter(|l| {
                ["revelar ", "copiar ", "totp "]
                    .iter()
                    .any(|p| l.starts_with(p))
            })
            .collect();
        assert_eq!(ataques.len(), 8, "{nome}");
        for l in ataques {
            let (pedido, resposta) = l.split_at(l.find('{').unwrap());
            let r = protocolo::decodificar_resposta(Papel::Agente, resposta.as_bytes());
            assert!(
                matches!(r, Ok(Resposta::Erro(ErroResposta::NaoAutorizado))),
                "{nome} {pedido}"
            );
        }
        let lista = linhas.iter().find(|l| l.starts_with("lista - ")).unwrap();
        // O agente que destravou recebe a lista; o outro, já destravado também.
        assert!(lista.contains(r#""tipo":"lista""#), "{lista}");
        for proibido in [
            r#""resumo""#,
            r#""nivel""#,
            r#""usuario""#,
            "gama-bloqueada",
        ] {
            assert!(
                !lista.contains(proibido),
                "{nome}: {proibido} na lista do agente"
            );
        }
        sem_segredo(&saida, "resposta ao agente");
    }

    // O app de teste, aberto pelo daemon: papel app, valores sem PIN nem aprovação.
    let mut c = UnixStream::connect(&socket).unwrap();
    c.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
    assert!(matches!(
        mandar(&mut c, r#"{"v":1,"op":"abrir_app","dados":{}}"#),
        Resposta::Ok
    ));
    let saida_app = dev.join("logs/wardd-app-teste.log");
    esperar(|| ler(&saida_app).contains("fim=1"), "a saída do app");
    let texto = ler(&saida_app);
    for linha in [
        "papel=app",
        "revelar=segredo",
        "revelar_pg=igual",
        "revelar_bloqueada=igual",
        "copiar_pg=igual",
        "totp=igual",
        "campo_inexistente=campo_inexistente",
        "lista=resumo",
        "travar=ok",
        "revelar_travado=travado",
    ] {
        assert!(
            texto.lines().any(|l| l == linha),
            "falta {linha:?}:\n{texto}"
        );
    }
    sem_segredo(&texto, "saída do app de teste");

    // O log: a operação, a entrada e o campo, nunca o valor.
    let daemon = ler(&log);
    sem_segredo(&daemon, "log do daemon");
    for linha in [
        "wardd: revelar acme-pg-dev senha",
        "wardd: revelar gama-bloqueada senha",
        "wardd: copiar acme-pg-dev senha",
        "wardd: totp acme-totp-dev",
    ] {
        assert!(daemon.lines().any(|l| l == linha), "falta {linha:?} no log");
    }
    // Nenhum valor revelado pelo agente nem para a extensão.
    assert!(!daemon.contains("revelar gama-generico"));
}
