//! Identificação do processo do outro lado do socket (D5, D47; V3.5, V3.24; S1, S5).
//!
//! O "binário instalado" dos testes é o python do sistema (`/usr/bin/python3.12`, do
//! root, sem escrita para grupo e outros): o binário de teste do Ward fica em `target/`
//! e é do usuário, então nunca passa — que é justamente o caso do ataque.
//!
//! Papel pelo grupo (D47) no host, sem root: o "grupo do app" dos testes é o próprio
//! egid do usuário (`Instalacao::de_teste`, só no `modo-teste`) e o "daemon" é este
//! processo de teste, pai do cliente. O setgid de verdade fica para o contêiner
//! (`make teste-docker-plataforma`).
//!
//! Ataques: binário copiado para fora da instalação, `LD_PRELOAD`/`LD_AUDIT`/
//! `LD_LIBRARY_PATH`, `strace` (TracerPid ≠ 0), biblioteca de usuário carregada sem
//! variável, processo que sai antes da conferência, pai que não é o Chrome, pai do app
//! que não é o daemon, grupo comum, namespace de usuário, `ptrace_scope` 0 simulado.

#![cfg(feature = "modo-teste")]

mod comum;

use std::path::{Path, PathBuf};

use comum::{DirTeste, Filho, cliente_python, comando_limpo, iniciar, permitido, python_real};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use ward_plataforma::socket::{Escuta, preparar_diretorio};
use ward_plataforma::{
    BinarioPermitido, ClassePar, Instalacao, PoliticaKernel, ProcessoPar, Recusa,
};
use ward_plataforma::{RecusaServidor, ServidorEsperado, conectar_servidor, conferir_servidor};

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

/// gid que nenhum processo do teste tem.
const GID_NENHUM: u32 = 999_123;

fn politica_ok() -> PoliticaKernel {
    PoliticaKernel::simulada(Some(1), Some(2))
}

/// "Grupo do app" = `gid`; o do host, um gid que ninguém tem.
fn inst_app(gid: u32) -> Instalacao {
    Instalacao::de_teste(gid, GID_NENHUM, inexistente())
}

/// "Grupo do host" = `gid`, com o "Chrome" `chrome`.
fn inst_host(gid: u32, chrome: BinarioPermitido) -> Instalacao {
    Instalacao::de_teste(GID_NENHUM, gid, chrome)
}

/// Cliente python com o bash como pai (o `; true` impede o exec direto do python).
fn cliente_com_bash_de_pai(socket: &Path) -> std::process::Command {
    let script = format!(
        "{} -I -c \"$CLIENTE\" \"$SOCKET\"; true",
        python_real().display()
    );
    let mut cmd = comando_limpo(Path::new("/usr/bin/bash"));
    cmd.arg("-c")
        .arg(script)
        .env("CLIENTE", comum::CLIENTE_PY)
        .env("SOCKET", socket);
    cmd
}

#[test]
fn binario_instalado_e_identificado() {
    let (_d, escuta) = escutar();
    let py = python_real();
    let (filho, par) = conectar(&escuta, cliente_python(&py, escuta.caminho(), None));
    assert_eq!(par.identificar(&permitido(&py)), Ok(()));
    assert_eq!(par.reconferir_sem_depurador(), Ok(()));
    // D47: o egid do cliente é o "grupo do app" e o pai é este processo (o "daemon").
    let egid = par.credenciais().gid;
    assert_eq!(
        par.classificar(&inst_app(egid), &politica_ok()),
        ClassePar::App
    );
    // Outro gid no grupo do app: comum (agente), mesmo com o exe instalado.
    assert_eq!(
        par.classificar(&inst_app(egid.wrapping_add(7)), &politica_ok()),
        ClassePar::Comum {
            motivo: Recusa::GrupoComum
        }
    );
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
    // D47: com o grupo do app, mas sob depurador, fica agente.
    assert_eq!(
        par.classificar(&inst_app(par.credenciais().gid), &politica_ok()),
        ClassePar::Comum {
            motivo: Recusa::Depurador
        }
    );
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
    let mut filho = iniciar(cliente_com_bash_de_pai(escuta.caminho()));
    filho.esperar("conectado");
    let par = escuta.aceitar().unwrap().par;
    let bash = permitido(Path::new("/usr/bin/bash"));
    let egid = par.credenciais().gid;
    assert_eq!(par.conferir_pai(&bash), Ok(()));
    assert_eq!(
        par.classificar(&inst_host(egid, bash.clone()), &politica_ok()),
        ClassePar::HostExtensao
    );
    // Pai que não é o "Chrome" permitido.
    let sleep = permitido(Path::new("/usr/bin/sleep"));
    assert_eq!(par.conferir_pai(&sleep), Err(Recusa::PaiNaoPermitido));
    assert_eq!(
        par.classificar(&inst_host(egid, sleep), &politica_ok()),
        ClassePar::Comum {
            motivo: Recusa::PaiNaoPermitido
        }
    );
    // Com o grupo do app e o bash de pai (não é o daemon nem o systemd --user).
    assert_eq!(
        par.classificar(&inst_app(egid), &politica_ok()),
        ClassePar::Comum {
            motivo: Recusa::PaiDoAppNaoPermitido
        }
    );
    filho.encerrar();
}

#[test]
fn host_da_extensao_sem_pai_permitido_quando_o_pai_e_o_teste() {
    // O pai do cliente é o binário de teste (em target/, do usuário).
    let (_d, escuta) = escutar();
    let py = python_real();
    let (filho, par) = conectar(&escuta, cliente_python(&py, escuta.caminho(), None));
    let eu = permitido(&std::env::current_exe().unwrap());
    assert_eq!(par.conferir_pai(&eu), Err(Recusa::PaiNaoPermitido));
    assert_eq!(
        par.classificar(&inst_host(par.credenciais().gid, eu), &politica_ok()),
        ClassePar::Comum {
            motivo: Recusa::PaiNaoPermitido
        }
    );
    filho.encerrar();
}

#[test]
fn ataque_ptrace_scope_0_ou_suid_dumpable_1_desliga_os_papeis() {
    // V3.24: com a política do kernel fraca, nem o grupo certo dá papel.
    let (_d, escuta) = escutar();
    let py = python_real();
    let (filho, par) = conectar(&escuta, cliente_python(&py, escuta.caminho(), None));
    let egid = par.credenciais().gid;
    for politica in [
        PoliticaKernel::simulada(Some(0), Some(0)),
        PoliticaKernel::simulada(Some(1), Some(1)),
        PoliticaKernel::simulada(None, Some(0)),
    ] {
        for inst in [
            inst_app(egid),
            inst_host(egid, permitido(Path::new("/usr/bin/bash"))),
        ] {
            assert_eq!(
                par.classificar(&inst, &politica),
                ClassePar::Comum {
                    motivo: Recusa::PoliticaKernel
                },
                "{politica:?}"
            );
        }
    }
    // A política deste kernel (Pop!_OS: ptrace_scope 1, suid_dumpable 2) passa.
    assert_eq!(PoliticaKernel::ler().conferir(), Ok(()));
    filho.encerrar();
}

#[test]
fn ataque_namespace_de_usuario_nao_forja_o_grupo() {
    // V3.24: num namespace de usuário o cliente é "root" com gid 0, mas o daemon vê o
    // gid de verdade (ou 65534), nunca um gid de sistema que ele não tem.
    let (_d, escuta) = escutar();
    let py = python_real();
    let mut cmd = comando_limpo(Path::new("/usr/bin/unshare"));
    cmd.args(["--user", "--map-root-user", "--"])
        .arg(&py)
        .arg("-I")
        .arg("-c")
        .arg(comum::CLIENTE_PY)
        .arg(escuta.caminho());
    let (filho, par) = conectar(&escuta, cmd);
    let meu_gid = std::fs::metadata("/proc/self").unwrap().gid();
    assert_eq!(par.credenciais().gid, meu_gid);
    assert_eq!(par.credenciais().uid, ward_plataforma::uid_efetivo());
    assert_eq!(
        par.classificar(&inst_app(GID_NENHUM - 1), &politica_ok()),
        ClassePar::Comum {
            motivo: Recusa::GrupoComum
        }
    );
    assert!(matches!(
        par.classificar(&inst_host(0, inexistente()), &politica_ok()),
        ClassePar::Comum { .. }
    ));
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
fn ataque_servidor_falso_do_mesmo_uid_e_recusado_antes_de_qualquer_byte() {
    // V3.23: servidor do mesmo uid, dumpable ou não, na pasta do usuário. Pela regra do
    // sistema (D46), a pasta não é do root e o par não é uid 0: recusa sem escrever.
    for nao_dumpable in [false, true] {
        let d = DirTeste::novo();
        let dir = preparar_diretorio(&d.caminho).unwrap();
        let socket = dir.caminho().join("wardd.sock");
        let srv = servidor(&socket, nao_dumpable);
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).unwrap();
        let esperado = ServidorEsperado::sistema_em(socket.clone());
        assert_eq!(
            conectar_servidor(&esperado).err(),
            Some(RecusaServidor::PastaNaoConfiavel)
        );
        // Mesmo conectando por fora, a conferência do socket conectado recusa.
        let s = std::os::unix::net::UnixStream::connect(&socket).unwrap();
        assert_eq!(
            conferir_servidor(&s, &esperado),
            Err(RecusaServidor::PastaNaoConfiavel)
        );
        // A forma de teste (D7) aceita o mesmo servidor: é o daemon de teste.
        assert_eq!(
            conferir_servidor(&s, &ServidorEsperado::teste_em(socket.clone())),
            Ok(())
        );
        drop(s);
        srv.encerrar();
    }
}

#[test]
fn ataque_servidor_de_teste_com_socket_aberto_ou_endereco_trocado() {
    let d = DirTeste::novo();
    let dir = preparar_diretorio(&d.caminho).unwrap();
    let socket = dir.caminho().join("wardd.sock");
    let srv = servidor(&socket, false);
    let teste = ServidorEsperado::teste_em(socket.clone());
    // O python cria o socket com o umask (0755 ou 0775): não é 0600.
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o666)).unwrap();
    assert_eq!(
        conectar_servidor(&teste).err(),
        Some(RecusaServidor::SocketNaoConfiavel)
    );
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).unwrap();
    // Esperado noutro caminho, com um link para o socket de verdade: o par não escuta
    // no caminho esperado (e o link não é socket).
    let link = dir.caminho().join("outro.sock");
    std::os::unix::fs::symlink(&socket, &link).unwrap();
    assert_eq!(
        conectar_servidor(&ServidorEsperado::teste_em(link.clone())).err(),
        Some(RecusaServidor::SocketNaoConfiavel)
    );
    let s = std::os::unix::net::UnixStream::connect(&link).unwrap();
    assert_eq!(
        conferir_servidor(
            &s,
            &ServidorEsperado::teste_em(dir.caminho().join("x.sock"))
        ),
        Err(RecusaServidor::Ausente)
    );
    drop(s);
    let s = conectar_servidor(&teste).unwrap();
    drop(s);
    srv.encerrar();
}
