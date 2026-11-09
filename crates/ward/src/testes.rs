//! Testes da CLI contra servidores falsos no próprio processo: um "daemon" honesto que
//! fala o protocolo e servidores maliciosos que gravam tudo o que recebem. Só valores
//! falsos; o socket fica numa pasta 0700 só do teste em `/run/user/<uid>/` (o caminho
//! não cabe em `target/tmp`), apagada no fim. Nunca o cofre real nem o daemon de teste.

use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use ward_core::limites::MAX_QUADRO_COMUM;
use ward_core::protocolo::quadro::{self, LIMITE_RESPOSTA};
use ward_core::protocolo::{self, Papel, Pedido};
use ward_plataforma::{ErroTerminal, ServidorEsperado};
use zeroize::Zeroizing;

use crate::cliente::{Conexao, ErroConexao};
use crate::comandos::{Entorno, SAIDA_ERRO, SAIDA_OK, SAIDA_USO};

const SENHA: &str = "falso-mestra-ward-01";

static N: AtomicU32 = AtomicU32::new(0);

/// Pasta 0700 só deste teste, apagada no `Drop`.
struct Pasta(PathBuf);

impl Pasta {
    fn nova() -> Pasta {
        let p = ward_plataforma::socket::diretorio_runtime().join(format!(
            "ward-cli-t-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir(&p).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        Pasta(p)
    }
    fn socket(&self) -> PathBuf {
        self.0.join("wardd.sock")
    }
}

impl Drop for Pasta {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// O que o servidor falso viu.
#[derive(Default)]
struct Visto {
    conexoes: u32,
    bytes: Vec<u8>,
    operacoes: Vec<&'static str>,
    senhas: Vec<Vec<u8>>,
}

type Responder = dyn Fn(&Pedido) -> Vec<u8> + Send + Sync;

/// Servidor falso no socket da pasta. `responder = None`: só grava os bytes (malicioso
/// que espera a senha). Com `responder`: lê quadros como o daemon e responde.
fn servidor(pasta: &Pasta, responder: Option<Arc<Responder>>) -> Arc<Mutex<Visto>> {
    let l = UnixListener::bind(pasta.socket()).unwrap();
    fs::set_permissions(pasta.socket(), fs::Permissions::from_mode(0o600)).unwrap();
    let visto = Arc::new(Mutex::new(Visto::default()));
    let v = Arc::clone(&visto);
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { return };
            v.lock().unwrap().conexoes += 1;
            match &responder {
                None => {
                    let mut buf = Vec::new();
                    let _ = s.read_to_end(&mut buf);
                    v.lock().unwrap().bytes.extend_from_slice(&buf);
                }
                Some(r) => loop {
                    let Ok(corpo) = quadro::ler_quadro(&mut s, MAX_QUADRO_COMUM) else {
                        break;
                    };
                    let p = protocolo::decodificar_pedido(Papel::Agente, &corpo).unwrap();
                    {
                        let mut v = v.lock().unwrap();
                        v.bytes.extend_from_slice(&corpo);
                        v.operacoes.push(p.operacao().nome());
                        if let Pedido::Destravar { senha } = &p {
                            v.senhas.push(senha.expor().to_vec());
                        }
                    }
                    let resposta = r(&p);
                    if quadro::escrever_quadro(&mut s, &resposta, LIMITE_RESPOSTA).is_err() {
                        break;
                    }
                },
            }
        }
    });
    visto
}

/// Entorno de teste: registra a ordem dos passos e a saída.
struct Teste {
    esperado: ServidorEsperado,
    /// Conferência completa (`Conexao::conectar`) ou sem a extra do `modo-teste`.
    completa: bool,
    eventos: Vec<&'static str>,
    antes_da_senha: Option<Box<dyn FnMut()>>,
    senha: Result<&'static str, ErroTerminal>,
    out: Vec<u8>,
    err: Vec<u8>,
}

impl Teste {
    fn novo(esperado: ServidorEsperado, completa: bool) -> Teste {
        Teste {
            esperado,
            completa,
            eventos: Vec::new(),
            antes_da_senha: None,
            senha: Ok(SENHA),
            out: Vec::new(),
            err: Vec::new(),
        }
    }
    fn rodar(&mut self, args: &[&str]) -> u8 {
        crate::principal(args.iter().map(std::ffi::OsString::from), self)
    }
    fn out(&self) -> String {
        String::from_utf8(self.out.clone()).unwrap()
    }
    fn err(&self) -> String {
        String::from_utf8(self.err.clone()).unwrap()
    }
}

impl Entorno for Teste {
    fn conectar(&mut self) -> Result<Conexao, ErroConexao> {
        self.eventos.push("conectar");
        if self.completa {
            Conexao::conectar(&self.esperado)
        } else {
            #[cfg(feature = "modo-teste")]
            {
                Conexao::conectar_sem_extra(&self.esperado)
            }
            #[cfg(not(feature = "modo-teste"))]
            {
                Conexao::conectar(&self.esperado)
            }
        }
    }
    fn endurecer(&mut self) -> io::Result<()> {
        // O processo de teste continua dumpable: só registra o passo.
        self.eventos.push("endurecer");
        Ok(())
    }
    fn ler_senha(&mut self) -> Result<Zeroizing<Vec<u8>>, ErroTerminal> {
        self.eventos.push("ler_senha");
        if let Some(f) = self.antes_da_senha.as_mut() {
            f();
        }
        self.senha.map(|s| Zeroizing::new(s.as_bytes().to_vec()))
    }
    fn saida(&mut self) -> &mut dyn Write {
        &mut self.out
    }
    fn erro(&mut self) -> &mut dyn Write {
        &mut self.err
    }
}

#[cfg(feature = "modo-teste")]
fn esperar_conexoes(visto: &Arc<Mutex<Visto>>, n: u32) {
    for _ in 0..200 {
        if visto.lock().unwrap().conexoes >= n {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// Ataque (V4.2, S5, D46): um servidor falso do mesmo uid numa pasta do usuário. Com a
/// regra do release (pasta do root, par uid 0), a CLI recusa antes de conectar: não lê
/// a senha, não endurece nada e o falso não recebe nem uma conexão.
#[test]
fn v4_2_ataque_servidor_falso_recusado_antes_de_ler_a_senha() {
    let pasta = Pasta::nova();
    let visto = servidor(&pasta, None);
    for cmd in [
        &["destravar"][..],
        &["lista"],
        &["lista", "--json"],
        &["travar"],
    ] {
        let mut t = Teste::novo(ServidorEsperado::sistema_em(pasta.socket()), true);
        assert_eq!(t.rodar(cmd), SAIDA_ERRO, "{cmd:?}");
        assert!(
            t.err().contains("servidor não é o wardd"),
            "{cmd:?}: {}",
            t.err()
        );
        assert!(t.err().contains("nada foi enviado"));
        assert_eq!(t.eventos, ["conectar"], "{cmd:?}");
        assert!(t.out.is_empty());
    }
    std::thread::sleep(std::time::Duration::from_millis(50));
    let v = visto.lock().unwrap();
    assert_eq!(v.conexoes, 0);
    assert!(v.bytes.is_empty());
}

/// Ataque (V4.2 no `modo-teste`): o falso fica no caminho do socket de teste, que passa
/// na regra de teste (pasta 0700 do usuário, par do mesmo uid). A conferência extra
/// recusa porque o processo do falso é dumpable; o falso vê a conexão e zero bytes, e
/// a senha não é lida.
#[cfg(feature = "modo-teste")]
#[test]
fn v4_2_ataque_servidor_falso_no_socket_de_teste_recusado() {
    let pasta = Pasta::nova();
    let visto = servidor(&pasta, None);
    for cmd in [&["destravar"][..], &["lista", "--json"], &["travar"]] {
        let mut t = Teste::novo(ServidorEsperado::teste_em(pasta.socket()), true);
        assert_eq!(t.rodar(cmd), SAIDA_ERRO);
        assert!(t.err().contains("servidor não é o wardd"), "{}", t.err());
        assert!(t.err().contains("pode ser depurado"), "{}", t.err());
        assert_eq!(t.eventos, ["conectar"]);
    }
    esperar_conexoes(&visto, 3);
    std::thread::sleep(std::time::Duration::from_millis(50));
    let v = visto.lock().unwrap();
    assert_eq!(v.conexoes, 3);
    assert!(
        v.bytes.is_empty(),
        "o falso recebeu {} bytes",
        v.bytes.len()
    );
}

#[test]
fn daemon_ausente() {
    let pasta = Pasta::nova();
    let mut t = Teste::novo(ServidorEsperado::sistema_em(pasta.socket()), true);
    // A pasta é do usuário: recusa antes mesmo de ver que não há socket.
    assert_eq!(t.rodar(&["lista"]), SAIDA_ERRO);
    let mut t = Teste::novo(
        ServidorEsperado::sistema_em(PathBuf::from("/run/ward/4294967294/wardd.sock")),
        true,
    );
    assert_eq!(t.rodar(&["destravar"]), SAIDA_ERRO);
    assert_eq!(t.err(), "ward: o daemon não está rodando\n");
    assert_eq!(t.eventos, ["conectar"]);
}

#[test]
fn uso_e_ajuda() {
    let mut t = Teste::novo(ServidorEsperado::sistema(), true);
    assert_eq!(t.rodar(&["--help"]), SAIDA_OK);
    assert!(t.out().contains("ward lista [--json]"));
    assert!(t.out().contains("ward destravar"));
    assert!(t.out().contains("ward travar"));
    let mut t = Teste::novo(ServidorEsperado::sistema(), true);
    assert_eq!(t.rodar(&[]), SAIDA_USO);
    assert!(t.err().contains("Uso:"));
    let mut t = Teste::novo(ServidorEsperado::sistema(), true);
    assert_eq!(t.rodar(&["destravar", SENHA]), SAIDA_USO);
    assert!(!t.err().contains(SENHA));
    assert!(t.eventos.is_empty(), "uso errado não conecta");
    let mut t = Teste::novo(ServidorEsperado::sistema(), true);
    assert_eq!(t.rodar(&["--version"]), SAIDA_OK);
    assert!(t.out().starts_with("ward "));
}

#[cfg(feature = "modo-teste")]
mod com_daemon_falso {
    use super::*;
    use ward_core::entrada::{Ambiente, TipoEntrada};
    use ward_core::protocolo::{ErroResposta, ItemLista, Resposta};

    fn codificar(r: &Resposta) -> Vec<u8> {
        protocolo::codificar_resposta(Papel::Agente, r)
            .unwrap()
            .to_vec()
    }

    /// Resposta de `estado`. Único lugar que monta `Resposta::Estado`: a Fase 23
    /// acrescenta o papel (D63) e só este ponto muda.
    fn resposta_estado(travado: bool) -> Resposta {
        Resposta::Estado {
            travado,
            cofre_existe: true,
        }
    }

    fn item(nome: &str, tipo: TipoEntrada, endereco: Option<&str>) -> ItemLista {
        ItemLista {
            nome: nome.to_owned(),
            tipo,
            grupo: "acme".to_owned(),
            ambiente: Ambiente::Dev,
            projeto: Some("site".to_owned()),
            tags: vec![],
            nivel: None,
            endereco: endereco.map(str::to_owned),
        }
    }

    /// Daemon falso honesto: `travado` muda com destravar/travar.
    fn daemon(pasta: &Pasta, travado: bool) -> Arc<Mutex<Visto>> {
        let estado = Arc::new(Mutex::new(travado));
        servidor(
            pasta,
            Some(Arc::new(move |p: &Pedido| {
                let mut travado = estado.lock().unwrap();
                codificar(&match p {
                    Pedido::Estado => resposta_estado(*travado),
                    Pedido::Travar => {
                        *travado = true;
                        Resposta::Ok
                    }
                    Pedido::Destravar { senha } if senha.expor() == SENHA.as_bytes() => {
                        *travado = false;
                        Resposta::Ok
                    }
                    Pedido::Destravar { .. } => Resposta::Erro(ErroResposta::SenhaIncorreta),
                    Pedido::Lista if *travado => Resposta::Erro(ErroResposta::Travado),
                    Pedido::Lista => Resposta::Lista(vec![
                        item(
                            "acme-wp-dev",
                            TipoEntrada::Login,
                            Some("http://127.0.0.1:8081"),
                        ),
                        item("gama-nota", TipoEntrada::NotaSegura, None),
                    ]),
                    _ => Resposta::Erro(ErroResposta::NaoAutorizado),
                })
            })),
        )
    }

    fn teste(pasta: &Pasta) -> Teste {
        Teste::novo(ServidorEsperado::teste_em(pasta.socket()), false)
    }

    #[test]
    fn v4_1_lista_em_texto_e_json() {
        let pasta = Pasta::nova();
        let _v = daemon(&pasta, false);
        let mut t = teste(&pasta);
        assert_eq!(t.rodar(&["lista", "--json"]), SAIDA_OK, "{}", t.err());
        assert_eq!(
            t.out(),
            "[{\"nome\":\"acme-wp-dev\",\"tipo\":\"login\",\"grupo\":\"acme\",\"ambiente\":\"dev\",\
             \"projeto\":\"site\",\"endereco\":\"http://127.0.0.1:8081\"},\
             {\"nome\":\"gama-nota\",\"tipo\":\"nota-segura\",\"grupo\":\"acme\",\"ambiente\":\"dev\",\
             \"projeto\":\"site\",\"endereco\":null}]\n"
        );
        let mut t = teste(&pasta);
        assert_eq!(t.rodar(&["lista"]), SAIDA_OK);
        assert!(t.out().starts_with("NOME "));
        assert!(t.out().contains("acme-wp-dev"));
        assert_eq!(t.eventos, ["conectar"]);
    }

    /// V4.22: depois de `ward travar`, `ward lista` responde "travado".
    #[test]
    fn v4_22_travar_e_lista_responde_travado() {
        let pasta = Pasta::nova();
        let v = daemon(&pasta, false);
        let mut t = teste(&pasta);
        assert_eq!(t.rodar(&["travar"]), SAIDA_OK);
        assert_eq!(t.out(), "cofre travado\n");
        let mut t = teste(&pasta);
        assert_eq!(t.rodar(&["lista"]), SAIDA_ERRO);
        assert!(t.err().starts_with("ward: travado"), "{}", t.err());
        assert!(t.out.is_empty());
        assert_eq!(v.lock().unwrap().operacoes, ["travar", "lista"]);
    }

    /// Ordem do destravar: servidor conferido, estado, endurecer, senha, reconferir,
    /// destravar. A senha vai uma vez só, no pedido `destravar`.
    #[test]
    fn destravar_na_ordem() {
        let pasta = Pasta::nova();
        let v = daemon(&pasta, true);
        let mut t = teste(&pasta);
        assert_eq!(t.rodar(&["destravar"]), SAIDA_OK, "{}", t.err());
        assert_eq!(t.out(), "cofre destravado\n");
        assert_eq!(t.eventos, ["conectar", "endurecer", "ler_senha"]);
        let v = v.lock().unwrap();
        assert_eq!(v.operacoes, ["estado", "destravar"]);
        assert_eq!(v.senhas, [SENHA.as_bytes()]);
        drop(v);

        // Já destravado: não lê a senha.
        let mut t = teste(&pasta);
        assert_eq!(t.rodar(&["destravar"]), SAIDA_OK);
        assert_eq!(t.out(), "o cofre já está destravado\n");
        assert_eq!(t.eventos, ["conectar"]);
    }

    #[test]
    fn destravar_senha_errada_vazia_cancelada() {
        let pasta = Pasta::nova();
        let v = daemon(&pasta, true);
        let mut t = teste(&pasta);
        t.senha = Ok("falso-errada-01");
        assert_eq!(t.rodar(&["destravar"]), SAIDA_ERRO);
        assert!(t.err().contains("senha mestra incorreta"), "{}", t.err());
        assert!(!t.err().contains("falso-errada-01") && !t.out().contains("falso-errada-01"));

        for (senha, msg) in [
            (Ok(""), "senha vazia"),
            (Err(ErroTerminal::Cancelado), "cancelado"),
            (Err(ErroTerminal::SemTerminal), "precisa de um terminal"),
            (Err(ErroTerminal::Longa), "longa demais"),
        ] {
            let mut t = teste(&pasta);
            t.senha = senha;
            assert_eq!(t.rodar(&["destravar"]), SAIDA_ERRO);
            assert!(t.err().contains(msg), "{}", t.err());
        }
        // Só a senha errada chegou ao daemon.
        let v = v.lock().unwrap();
        assert_eq!(v.senhas, [b"falso-errada-01".to_vec()]);
    }

    /// Ataque: o socket é trocado (fica 0666) enquanto o usuário digita. A conferência
    /// de antes de mandar recusa e a senha não sai.
    #[test]
    fn ataque_socket_trocado_enquanto_digita() {
        let pasta = Pasta::nova();
        let v = daemon(&pasta, true);
        let mut t = teste(&pasta);
        let sock = pasta.socket();
        t.antes_da_senha = Some(Box::new(move || {
            fs::set_permissions(&sock, fs::Permissions::from_mode(0o666)).unwrap();
        }));
        assert_eq!(t.rodar(&["destravar"]), SAIDA_ERRO);
        assert!(t.err().contains("servidor não é o wardd"), "{}", t.err());
        let v = v.lock().unwrap();
        assert_eq!(v.operacoes, ["estado"]);
        assert!(v.senhas.is_empty());
        assert!(!String::from_utf8_lossy(&v.bytes).contains("ZmFsc28tbWVzdHJh"));
    }

    /// Ataque (S1): um servidor que manda resposta com credencial (`segredo`) ou uma
    /// lista com nível (D50) é recusado na leitura; nada disso chega à saída.
    #[test]
    fn ataque_resposta_com_credencial_ou_nivel_recusada() {
        for corpo in [
            r#"{"v":1,"tipo":"segredo","dados":{"valor":"ZmFsc28tUEc6czNuaGEvMDFAIw=="}}"#,
            r#"{"v":1,"tipo":"lista","dados":{"itens":[{"nome":"a","tipo":"login","grupo":"g","ambiente":"dev","tags":[],"nivel":"livre","endereco":"http://x"}]}}"#,
            r#"{"v":1,"tipo":"preenchimento","dados":{"usuario":"u","senha":"ZmFsc28="}}"#,
            r#"{"v":2,"tipo":"ok"}"#,
        ] {
            let pasta = Pasta::nova();
            let corpo = corpo.as_bytes().to_vec();
            let _v = servidor(&pasta, Some(Arc::new(move |_: &Pedido| corpo.clone())));
            let mut t = teste(&pasta);
            assert_eq!(t.rodar(&["lista", "--json"]), SAIDA_ERRO);
            assert_eq!(t.err(), "ward: resposta inválida do daemon\n");
            assert!(t.out.is_empty());
        }
    }
}
