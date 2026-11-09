//! Identificação do processo do outro lado do socket (D5; V3.5; S1, S5).
//!
//! O "binário instalado" dos testes é o python do sistema (`/usr/bin/python3.12`, do
//! root, sem escrita para grupo e outros): o binário de teste do Ward fica em `target/`
//! e é do usuário, então nunca passa — que é justamente o caso do ataque.
//!
//! Ataques: binário copiado para fora da instalação, `LD_PRELOAD`/`LD_AUDIT`/
//! `LD_LIBRARY_PATH`, `strace` (TracerPid ≠ 0), biblioteca de usuário carregada sem
//! variável, processo que sai antes da conferência, pai que não é o Chrome, servidor
//! falso no lugar do `wardd`.

mod comum;

use std::path::{Path, PathBuf};

use comum::{DirTeste, Filho, cliente_python, comando_limpo, iniciar, permitido, python_real};
use ward_plataforma::socket::{Escuta, preparar_diretorio};
use ward_plataforma::{BinarioPermitido, ClassePar, Instalacao, ProcessoPar, Recusa};

fn escutar() -> (DirTeste, Escuta) {
    let d = DirTeste::novo();
    let escuta = preparar_diretorio(&d.caminho)
        .unwrap()
        .escutar("wardd.sock")
        .unwrap();
    (d, escuta)
}

/// Sobe o cliente, espera conectar e devolve a conexão aceita.
fn conectar(escuta: &Escuta, cmd: std::process::Command) -> (Filho, ProcessoPar) {
    let mut filho = iniciar(cmd);
    filho.esperar("conectado");
    let conexao = escuta.aceitar().unwrap();
    assert_eq!(conexao.par.credenciais().pid, filho.pid());
    (filho, conexao.par)
}

fn inexistente() -> BinarioPermitido {
    BinarioPermitido::novo("/usr/local/bin/ward-falso-nao-instalado").unwrap()
}

#[test]
fn binario_instalado_e_identificado() {
    let (_d, escuta) = escutar();
    let py = python_real();
    let (filho, par) = conectar(&escuta, cliente_python(&py, escuta.caminho(), None));
    assert_eq!(par.identificar(&permitido(&py)), Ok(()));
    assert_eq!(par.reconferir_sem_depurador(), Ok(()));
    let inst = Instalacao {
        app: permitido(&py),
        host_extensao: inexistente(),
        chrome: inexistente(),
    };
    assert_eq!(par.classificar(&inst), ClassePar::App);
    // Outro binário permitido: comum (agente).
    let inst = Instalacao {
        app: permitido(Path::new("/usr/bin/bash")),
        host_extensao: inexistente(),
        chrome: inexistente(),
    };
    assert!(matches!(
        par.classificar(&inst),
        ClassePar::Comum {
            app: Recusa::ExeNaoPermitido,
            ..
        }
    ));
    // Cadeia para exibir: termina no cliente, e o teste está antes dele.
    let cadeia = par.cadeia();
    assert_eq!(cadeia.last().unwrap().pid, filho.pid());
    assert!(cadeia.iter().any(|e| e.pid == std::process::id()));
    filho.encerrar();
}

#[test]
fn ataque_binario_copiado_para_fora_da_instalacao() {
    // V3.5: o mesmo binário, copiado para uma pasta do usuário.
    let copia = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ward-app-copiado");
    std::fs::copy(python_real(), &copia).unwrap();
    let (_d, escuta) = escutar();
    let (filho, par) = conectar(&escuta, cliente_python(&copia, escuta.caminho(), None));
    assert_eq!(
        par.identificar(&permitido(&python_real())),
        Err(Recusa::ExeNaoPermitido)
    );
    // Nem pondo a cópia como "permitida": ela não é do root.
    assert_eq!(
        par.identificar(&permitido(&copia)),
        Err(Recusa::InstalacaoNaoConfiavel)
    );
    filho.encerrar();
    let _ = std::fs::remove_file(&copia);
}

#[test]
fn ataque_variaveis_ld_no_ambiente() {
    let py = python_real();
    // Valores vazios ou inexistentes: o carregador ignora, mas o nome já recusa.
    for (nome, valor) in [
        ("LD_PRELOAD", ""),
        ("LD_AUDIT", ""),
        ("LD_LIBRARY_PATH", "/nao-existe-ward"),
    ] {
        let (_d, escuta) = escutar();
        let mut cmd = cliente_python(&py, escuta.caminho(), None);
        cmd.env(nome, valor);
        let (filho, par) = conectar(&escuta, cmd);
        assert_eq!(
            par.identificar(&permitido(&py)),
            Err(Recusa::VariavelLd(match nome {
                "LD_PRELOAD" => "LD_PRELOAD",
                "LD_AUDIT" => "LD_AUDIT",
                _ => "LD_LIBRARY_PATH",
            })),
            "{nome}"
        );
        filho.encerrar();
    }
}

#[test]
fn ataque_biblioteca_de_usuario_sem_variavel() {
    // Como um `LD_PRELOAD` que apagou a própria variável: a biblioteca aparece no
    // `maps`, fora do root.
    let lib = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("libfalsa-ward.so");
    std::fs::copy("/usr/lib/x86_64-linux-gnu/libz.so.1", &lib).unwrap();
    let (_d, escuta) = escutar();
    let py = python_real();
    let (filho, par) = conectar(&escuta, cliente_python(&py, escuta.caminho(), Some(&lib)));
    assert_eq!(
        par.identificar(&permitido(&py)),
        Err(Recusa::BibliotecaNaoConfiavel)
    );
    filho.encerrar();
    let _ = std::fs::remove_file(&lib);
}

#[test]
fn ataque_cliente_sob_strace() {
    let (_d, escuta) = escutar();
    let py = python_real();
    let mut cmd = comando_limpo(Path::new("/usr/bin/strace"));
    cmd.args(["-f", "-o", "/dev/null"])
        .arg(&py)
        .arg("-I")
        .arg("-c")
        .arg(comum::CLIENTE_PY)
        .arg(escuta.caminho());
    let mut filho = iniciar(cmd);
    filho.esperar("conectado");
    let par = escuta.aceitar().unwrap().par;
    assert_eq!(par.identificar(&permitido(&py)), Err(Recusa::Depurador));
    assert_eq!(par.reconferir_sem_depurador(), Err(Recusa::Depurador));
    filho.encerrar();
}

#[test]
fn ataque_processo_que_sai_antes_da_conferencia() {
    let (_d, escuta) = escutar();
    let py = python_real();
    let (mut filho, par) = conectar(&escuta, cliente_python(&py, escuta.caminho(), None));
    drop(filho.filho.stdin.take());
    filho.filho.wait().unwrap();
    // O pid pode até ser reaproveitado; o pidfd não.
    assert_eq!(par.identificar(&permitido(&py)), Err(Recusa::ProcessoSumiu));
    assert_eq!(par.reconferir_sem_depurador(), Err(Recusa::ProcessoSumiu));
}

#[test]
fn host_da_extensao_exige_o_pai_permitido() {
    let (_d, escuta) = escutar();
    let py = python_real();
    // O bash fica como pai (o `; true` impede o exec direto do python).
    let script = format!("{} -I -c \"$CLIENTE\" \"$SOCKET\"; true", py.display());
    let mut cmd = comando_limpo(Path::new("/usr/bin/bash"));
    cmd.arg("-c")
        .arg(script)
        .env("CLIENTE", comum::CLIENTE_PY)
        .env("SOCKET", escuta.caminho());
    let mut filho = iniciar(cmd);
    filho.esperar("conectado");
    let par = escuta.aceitar().unwrap().par;
    let bash = permitido(Path::new("/usr/bin/bash"));
    assert_eq!(
        par.identificar_host_extensao(&permitido(&py), &bash),
        Ok(())
    );
    let inst = Instalacao {
        app: inexistente(),
        host_extensao: permitido(&py),
        chrome: bash.clone(),
    };
    assert_eq!(par.classificar(&inst), ClassePar::HostExtensao);
    // Pai que não é o "Chrome" permitido.
    assert_eq!(
        par.identificar_host_extensao(&permitido(&py), &permitido(Path::new("/usr/bin/sleep"))),
        Err(Recusa::PaiNaoPermitido)
    );
    // Host certo, mas o binário do host não é o instalado.
    assert_eq!(
        par.identificar_host_extensao(&bash, &bash),
        Err(Recusa::ExeNaoPermitido)
    );
    filho.encerrar();
}

#[test]
fn host_da_extensao_sem_pai_permitido_quando_o_pai_e_o_teste() {
    // O pai do cliente é o binário de teste (em target/, do usuário).
    let (_d, escuta) = escutar();
    let py = python_real();
    let (filho, par) = conectar(&escuta, cliente_python(&py, escuta.caminho(), None));
    let eu = std::env::current_exe().unwrap();
    assert_eq!(
        par.identificar_host_extensao(&permitido(&py), &permitido(&eu)),
        Err(Recusa::PaiNaoPermitido)
    );
    filho.encerrar();
}

/// Servidor python que escuta em `socket`; com `nao_dumpable`, faz
/// `prctl(PR_SET_DUMPABLE, 0)` antes, como o `wardd` endurecido.
const SERVIDOR_PY: &str = r#"
import socket, sys, ctypes
s = socket.socket(socket.AF_UNIX)
s.bind(sys.argv[1]); s.listen(1)
if sys.argv[2] == "1":
    ctypes.CDLL(None).prctl(4, 0, 0, 0, 0)
sys.stdout.write("conectado\n"); sys.stdout.flush()
c, _ = s.accept()
sys.stdin.read()
"#;

fn servidor(socket: &Path, nao_dumpable: bool) -> Filho {
    let mut cmd = comando_limpo(&python_real());
    cmd.arg("-I")
        .arg("-c")
        .arg(SERVIDOR_PY)
        .arg(socket)
        .arg(if nao_dumpable { "1" } else { "0" });
    let mut f = iniciar(cmd);
    f.esperar("conectado");
    f
}

#[test]
fn cliente_confere_o_servidor_antes_da_senha() {
    let d = DirTeste::novo();
    let dir = preparar_diretorio(&d.caminho).unwrap();
    let socket = dir.caminho().join("wardd.sock");
    let py = python_real();
    let srv = servidor(&socket, false);
    let s = std::os::unix::net::UnixStream::connect(&socket).unwrap();
    assert_eq!(
        ward_plataforma::conferir_servidor(&s, &permitido(&py)),
        Ok(())
    );
    // Servidor falso (qualquer outro binário) no lugar do wardd.
    assert_eq!(
        ward_plataforma::conferir_servidor(&s, &permitido(Path::new("/usr/bin/bash"))),
        Err(Recusa::ExeNaoPermitido)
    );
    drop(s);
    srv.encerrar();
}

#[test]
fn servidor_nao_dumpable_nao_pode_ser_conferido_pelo_proc() {
    // Registro do conflito S5 × S8 (handoff da Fase 21): com PR_SET_DUMPABLE=0 o
    // /proc do servidor fica ilegível para o cliente, e a conferência falha fechada.
    let d = DirTeste::novo();
    let dir = preparar_diretorio(&d.caminho).unwrap();
    let socket = dir.caminho().join("wardd.sock");
    let py = python_real();
    let srv = servidor(&socket, true);
    let s = std::os::unix::net::UnixStream::connect(&socket).unwrap();
    assert_eq!(
        ward_plataforma::conferir_servidor(&s, &permitido(&py)),
        Err(Recusa::SemAcesso)
    );
    drop(s);
    srv.encerrar();
}
