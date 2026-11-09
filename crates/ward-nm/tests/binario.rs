//! O binário `ward-nm` executado como um processo qualquer do usuário (Fase 40, V6.1):
//! argumento fora da forma, mensagens acima do limite, malformadas e sem daemon. Nada
//! de segredo existe aqui; o que se confere é que nada sai no stdout além das respostas
//! de erro do protocolo.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const ORIGEM: &str = "chrome-extension://dkacdineofgfabdognjgeoakekaenaei/";

fn pasta_vazia() -> PathBuf {
    let p =
        PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("ward-nm-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&p);
    p
}

fn rodar(args: &[&str], entrada: &[u8]) -> (Option<i32>, Vec<u8>, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ward-nm"))
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("WARD_DEV_SOCKET_DIR", pasta_vazia())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut i = c.stdin.take().unwrap();
    let _ = i.write_all(entrada);
    drop(i);
    let o = c.wait_with_output().unwrap();
    (
        o.status.code(),
        o.stdout,
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn quadro(corpo: &[u8]) -> Vec<u8> {
    let mut v = (corpo.len() as u32).to_ne_bytes().to_vec();
    v.extend_from_slice(corpo);
    v
}

#[test]
fn ataque_argumentos_fora_da_forma_saem_sem_nada_no_stdout() {
    let m = quadro(br#"{"v":1,"op":"estado"}"#);
    for args in [
        &[][..],
        &["chrome-extension://dkacdineofgfabdognjgeoakekaenaei"],
        &[ORIGEM, "--parent-window=0"],
        &["https://localhost:8081/"],
        &["--help"],
    ] {
        let (codigo, saida, _) = rodar(args, &m);
        assert_eq!(codigo, Some(2), "{args:?}");
        assert!(saida.is_empty(), "{args:?}");
    }
}

#[test]
fn ataque_mensagem_grande_demais_e_malformada() {
    // Acima de 64 KiB: responde quadro inválido e sai ≠ 0 sem ler o resto.
    let mut m = (1_000_000u32).to_ne_bytes().to_vec();
    m.extend(vec![b'{'; 1000]);
    let (codigo, saida, err) = rodar(&[ORIGEM], &m);
    assert_eq!(codigo, Some(1));
    assert!(String::from_utf8_lossy(&saida).contains("quadro_invalido"));
    assert!(!err.contains('{'), "stderr com conteúdo: {err}");

    // JSON malformado e operação do app: erro do protocolo, e o laço continua.
    let mut m = quadro(b"{nao e json");
    m.extend(quadro(
        br#"{"v":1,"op":"revelar","dados":{"entrada":"acme-wp-dev","campo":"senha"}}"#,
    ));
    let (codigo, saida, err) = rodar(&[ORIGEM], &m);
    assert_eq!(codigo, Some(0));
    let s = String::from_utf8_lossy(&saida);
    assert!(
        s.contains("pedido_invalido") && s.contains("nao_autorizado"),
        "{s}"
    );
    assert!(!err.contains("acme-wp-dev"), "stderr com conteúdo: {err}");
}

#[test]
fn sem_daemon_responde_interno() {
    let m = quadro(br#"{"v":1,"op":"preencher","dados":{"entrada":"acme-wp-dev","origem":"http://localhost:8081"}}"#);
    let (codigo, saida, err) = rodar(&[ORIGEM], &m);
    assert_eq!(codigo, Some(0));
    assert!(String::from_utf8_lossy(&saida).contains(r#""codigo":"interno""#));
    assert!(err.contains("daemon indisponível"), "{err}");
}
