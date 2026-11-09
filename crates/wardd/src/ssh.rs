//! `ward ssh` (Fase 27; D3, D12, D13, D34, D43, D56; S1–S4, S8, S13).
//!
//! - **Quem chamou** (D3): a CLI é sempre agente; vale o nível da entrada. A exceção é
//!   o token do "Conectar" do app ([`Daemon::emitir_token`]): aleatório, de uso único,
//!   30 s, preso à entrada. Um token válido para a entrada pula o nível (nunca o
//!   bloqueado: a entrada continua invisível); inválido, vencido ou de outra entrada é
//!   ignorado e vale o nível. Todo token apresentado que existe morre ali, mesmo usado
//!   na entrada errada.
//! - **Sem shell** (S3, D43): o daemon executa `ssh` (ou `sshpass` → `ssh`) por
//!   `execve`, com o argv montado aqui: o usuário só como argumento próprio de
//!   `-l <usuário>`, o host depois de `--`, nunca `usuário@host` nem `-o User=`. No
//!   `sftp`, onde `-l` é limite de banda, o usuário vai como `-o` + `User=<usuário>`
//!   (D56), com `-F /dev/null` e o host depois de `--`.
//! - **Nada do `~/.ssh`** (D13, R19): `-F /dev/null`, `IdentitiesOnly=yes`, sem
//!   identidades padrão (`IdentityFile`/`CertificateFile`), `known_hosts` próprio do Ward
//!   (`StrictHostKeyChecking=accept-new`: a primeira conexão grava a chave do host; uma
//!   chave diferente depois recusa) e o ambiente do filho montado do zero (sem
//!   `SSH_AUTH_SOCK` do usuário nem nada do daemon).
//! - **Senha**: `sshpass -e`, a senha só em `SSHPASS` no ambiente do filho (nunca em
//!   argv), com `PubkeyAuthentication=no` e `IdentityAgent=none`.
//! - **Chave**: o agente temporário de D12 ([`crate::agente_ssh`]), só para o `ssh`
//!   filho; a chave privada nunca sai do daemon.
//! - **Interativo** (sem comando e com janela): o filho roda num pty; a saída passa pela
//!   máscara com a pausa de D34 (300 ms sem saída nova solta o fim segurado como
//!   `••••`). Com comando (ou sem terminal), saída e erro em fluxos separados, como no
//!   `ward run`.
//! - O salto (`ProxyJump`) é da Fase 28: até lá, `servidor` com salto responde
//!   `entrega_falhou` sem executar nada.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ward_core::entrada::{Autenticacao, Dados, Entrada, NivelAcesso, Protocolo, Segredo};
use ward_core::mascara::Mascara;
use ward_core::protocolo::{ErroResposta, Papel, Pedido, Resposta};
use ward_plataforma::filho::Comando;
use zeroize::Zeroizing;

use crate::agente_ssh::{AgenteSsh, ErroAgente};
use crate::aprovacao::Cliente;
use crate::execucao::{AgentePreparado, Preparo};
use crate::nucleo::Daemon;
use crate::{aviso, depurar};

/// Validade do token do "Conectar" (D3).
pub const PRAZO_TOKEN: Duration = Duration::from_secs(30);
/// Tokens não usados guardados ao mesmo tempo (o mais antigo sai).
const MAX_TOKENS: usize = 16;
/// `PATH` do filho (os programas vão por caminho absoluto; isto é para o `ssh` achar o
/// que ele mesmo chama, nada da sessão).
const PATH_FILHO: &[u8] = b"/usr/bin:/bin";
/// `TERM` do pty (o protocolo não traz o terminal da CLI).
const TERM_PTY: &[u8] = b"xterm-256color";

/// Os programas do `ward ssh`, por caminho absoluto.
#[derive(Debug, Clone)]
pub struct ProgramasSsh {
    pub ssh: PathBuf,
    pub sshpass: PathBuf,
    pub sftp: PathBuf,
}

impl ProgramasSsh {
    /// Os do sistema (D13: `sshpass` é dependência de sistema da instalação).
    pub fn do_sistema() -> Self {
        ProgramasSsh {
            ssh: PathBuf::from("/usr/bin/ssh"),
            sshpass: PathBuf::from("/usr/bin/sshpass"),
            sftp: PathBuf::from("/usr/bin/sftp"),
        }
    }
}

/// Um token do "Conectar" emitido e ainda não usado.
pub(crate) struct TokenConectar {
    valor: Segredo,
    entrada: String,
    ate: Duration,
}

// --- Argv (S3, D13, D43, D56) ------------------------------------------------------------

/// Onde conectar (valores já validados pelo núcleo; conferidos de novo aqui).
#[derive(Debug, Clone, Copy)]
pub struct Destino<'a> {
    pub host: &'a str,
    pub porta: u16,
    pub usuario: &'a str,
}

/// Como autenticar.
#[derive(Debug, Clone, Copy)]
pub enum Autent<'a> {
    /// `sshpass -e`; a senha vai em `SSHPASS`, nunca aqui.
    Senha,
    /// Agente de D12: o socket e a chave pública (para `IdentitiesOnly=yes`).
    Chave { agente: &'a Path, publica: &'a Path },
}

/// Qual programa.
#[derive(Debug, Clone, Copy)]
pub enum Programa<'a> {
    /// `ssh`, com o comando remoto (vazio: shell).
    Ssh { comando: &'a [Zeroizing<Vec<u8>>] },
    /// `sftp` (a entrada `ftp/sftp` com protocolo `sftp`).
    Sftp,
}

/// Usuário de `servidor`/`ftp-sftp` (D43): `[A-Za-z0-9._@-]`, sem `-` no começo.
fn usuario_ok(u: &str) -> bool {
    !u.is_empty()
        && !u.starts_with('-')
        && u.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'@' | b'-'))
}

/// Host (D43): o do núcleo, sem `@` e sem `-` no começo.
fn host_ok(h: &str) -> bool {
    !h.is_empty()
        && !h.starts_with('-')
        && h.bytes().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_' | b':' | b'[' | b']')
        })
}

fn texto(p: &Path) -> Option<&str> {
    p.to_str()
        .filter(|s| !s.contains(['\n', '\0', ',', ' ', '%', '"', '\'']))
}

/// O argv inteiro do filho (`argv[0]` é o programa). `None` se algum valor não passa
/// na conferência (nada é executado). Sem shell: cada item vai como um argumento.
pub fn montar_argv(
    progs: &ProgramasSsh,
    known_hosts: &Path,
    destino: Destino<'_>,
    autent: Autent<'_>,
    programa: Programa<'_>,
) -> Option<Vec<Vec<u8>>> {
    if !usuario_ok(destino.usuario) || !host_ok(destino.host) || destino.porta == 0 {
        return None;
    }
    let kh = texto(known_hosts)?;
    let mut a = Argv(Vec::new());
    let prog = match programa {
        Programa::Ssh { .. } => &progs.ssh,
        Programa::Sftp => &progs.sftp,
    };
    if let Autent::Senha = autent {
        a.s(texto(&progs.sshpass)?);
        a.s("-e");
    }
    a.s(texto(prog)?);
    // Nada da configuração do usuário nem do sistema.
    a.s("-F");
    a.s("/dev/null");
    a.o("IdentitiesOnly", "yes");
    a.o("CertificateFile", "/dev/null");
    a.o("UserKnownHostsFile", kh);
    a.o("GlobalKnownHostsFile", "/dev/null");
    a.o("StrictHostKeyChecking", "accept-new");
    a.o("UpdateHostKeys", "no");
    a.o("ControlMaster", "no");
    a.o("ControlPath", "none");
    a.o("ForwardAgent", "no");
    a.o("ForwardX11", "no");
    a.o("PermitLocalCommand", "no");
    a.o("ProxyCommand", "none");
    match autent {
        Autent::Senha => {
            a.o("PubkeyAuthentication", "no");
            a.o("IdentityAgent", "none");
            a.o("IdentityFile", "none");
        }
        Autent::Chave { agente, publica } => {
            a.o("PubkeyAuthentication", "yes");
            a.o("IdentityAgent", texto(agente)?);
            a.o("IdentityFile", texto(publica)?);
            a.o("PasswordAuthentication", "no");
            a.o("KbdInteractiveAuthentication", "no");
            a.o("BatchMode", "yes");
        }
    }
    let porta = destino.porta.to_string();
    match programa {
        Programa::Ssh { comando } => {
            a.s("-p");
            a.s(&porta);
            // D43: o usuário só como argumento próprio de `-l`.
            a.s("-l");
            a.s(destino.usuario);
            a.s("--");
            a.s(destino.host);
            for c in comando {
                a.0.push(c.to_vec());
            }
        }
        Programa::Sftp => {
            // D56: no `sftp`, `-l` é limite de banda; o usuário vai como `-o` +
            // `User=<usuário>` (o conjunto de D43 não tem `%`, espaço, aspas, `=`, `,`).
            a.o("User", destino.usuario);
            a.s("-P");
            a.s(&porta);
            a.s("--");
            a.s(destino.host);
        }
    }
    Some(a.0)
}

/// O argv em montagem.
struct Argv(Vec<Vec<u8>>);

impl Argv {
    fn s(&mut self, x: &str) {
        self.0.push(x.as_bytes().to_vec());
    }

    /// `-o` e `Opção=valor` em dois argumentos.
    fn o(&mut self, op: &str, v: &str) {
        self.s("-o");
        self.0.push(format!("{op}={v}").into_bytes());
    }
}

// --- Token do "Conectar" (D3) ------------------------------------------------------------

impl Daemon {
    /// `emitir_token` (só o app; D3): token aleatório de uso único, 30 s, preso a uma
    /// entrada `servidor`. Some ao travar. O valor só vai na resposta ao app.
    pub(crate) fn emitir_token(&self, papel: Papel, nome: &str) -> Result<Resposta, ErroResposta> {
        if papel != Papel::App {
            return Err(ErroResposta::NaoAutorizado);
        }
        let agora = self.aprovacao.agora();
        let mut est = self.estado();
        let a = est.aberto.as_mut().ok_or(ErroResposta::Travado)?;
        let e = a
            .entradas
            .iter()
            .find(|e| e.nome() == nome)
            .ok_or(ErroResposta::EntradaInexistente)?;
        if !matches!(e.dados(), Dados::Servidor(_)) {
            return Err(ErroResposta::PedidoInvalido);
        }
        let valor = ward_core::gerador::token_conectar().map_err(|_| ErroResposta::Interno)?;
        a.tokens.retain(|t| agora < t.ate);
        if a.tokens.len() >= MAX_TOKENS {
            a.tokens.remove(0);
        }
        a.tokens.push(TokenConectar {
            valor: Segredo::de_bytes(valor.expor().to_vec()),
            entrada: nome.to_owned(),
            ate: agora.saturating_add(PRAZO_TOKEN),
        });
        aviso!("token do Conectar emitido para uma entrada servidor (D3)");
        Ok(Resposta::Token(valor))
    }

    /// O token vale para `entrada`? Todo token que existe morre aqui (uso único), mesmo
    /// apresentado com outra entrada. Comparação em tempo constante.
    fn usar_token(&self, token: &Segredo, entrada: &str) -> bool {
        let agora = self.aprovacao.agora();
        let mut est = self.estado();
        let Some(a) = est.aberto.as_mut() else {
            return false;
        };
        a.tokens.retain(|t| agora < t.ate);
        let mut achado = None;
        for (i, t) in a.tokens.iter().enumerate() {
            if ward_core::gerador::tokens_iguais(t.valor.expor(), token.expor()) {
                achado = Some(i);
            }
        }
        match achado {
            Some(i) => a.tokens.remove(i).entrada == entrada,
            None => false,
        }
    }
}

// --- Preparo --------------------------------------------------------------------------

/// O que a sessão precisa da entrada, copiado sob a trava do estado.
enum Cred {
    Senha(Segredo),
    Chave {
        chave: Segredo,
        passphrase: Option<Segredo>,
    },
}

fn copiar(s: &Segredo) -> Segredo {
    Segredo::de_bytes(s.expor().to_vec())
}

fn cred_de(a: &Autenticacao) -> Cred {
    match a {
        Autenticacao::Senha(s) => Cred::Senha(copiar(s)),
        Autenticacao::Chave {
            chave_privada,
            passphrase,
        } => Cred::Chave {
            chave: copiar(chave_privada),
            passphrase: passphrase.as_ref().map(copiar),
        },
    }
}

/// A entrada visível (bloqueada = inexistente, V3.17).
fn visivel<'a>(entradas: &'a [Entrada], nome: &str) -> Option<&'a Entrada> {
    entradas
        .iter()
        .find(|e| e.nome() == nome && e.nivel() != NivelAcesso::Bloqueado)
}

/// O tipo aceita o `ward ssh`? `servidor`, ou `ftp/sftp` com `sftp` e sem comando (o
/// `sftp` lê os comandos da entrada). Os outros: `tipo_sem_entrega`.
fn conferir_tipo(e: &Entrada, com_comando: bool) -> Result<(), ErroResposta> {
    match e.dados() {
        Dados::Servidor(_) => Ok(()),
        Dados::FtpSftp(f) if f.protocolo == Protocolo::Sftp => {
            if com_comando {
                Err(ErroResposta::PedidoInvalido)
            } else {
                Ok(())
            }
        }
        _ => Err(ErroResposta::TipoSemEntrega),
    }
}

impl Daemon {
    /// Autoriza (token ou nível), monta o argv, o ambiente, a máscara e o agente.
    pub(crate) fn preparar_ssh(
        &self,
        cliente: &dyn Cliente,
        pedido: Pedido,
    ) -> Result<Preparo, ErroResposta> {
        let Pedido::Ssh {
            entrada,
            comando,
            token,
            janela,
        } = &pedido
        else {
            return Err(ErroResposta::PedidoInvalido);
        };
        let com_comando = !comando.is_empty();
        // Pty só no interativo: sem comando e com o tamanho do terminal da CLI.
        let janela = if com_comando { None } else { *janela };
        let nome = entrada.clone();
        // O nível vem antes do tipo (V4.32: o token inventado numa entrada `banco` com
        // aprovação dá "aprovação indisponível"); o tipo é conferido depois, sob a trava.
        let pelo_token = token.as_ref().is_some_and(|t| self.usar_token(t, &nome));
        if pelo_token {
            aviso!("ssh: token do Conectar aceito; sem nível nem aprovação (D3)");
        } else {
            if token.is_some() {
                aviso!(
                    "ssh: token inválido, vencido, já usado ou de outra entrada; vale o nível (D3)"
                );
            }
            self.autorizar_uso(cliente, &pedido)?;
        }
        self.registrar_uso();
        let Pedido::Ssh { comando, .. } = pedido else {
            return Err(ErroResposta::PedidoInvalido);
        };
        // Com o estado travado: a entrada pode ter mudado durante a aprovação.
        let (host, porta, usuario, cred, sftp, mascara, travas, inicial) = {
            let est = self.estado();
            let a = est.aberto.as_ref().ok_or(ErroResposta::Travado)?;
            let e = visivel(&a.entradas, &nome).ok_or(ErroResposta::EntradaInexistente)?;
            conferir_tipo(e, com_comando)?;
            let (host, porta, usuario, auth, sftp) = match e.dados() {
                Dados::Servidor(s) => {
                    if s.salto.is_some() {
                        aviso!("ssh: servidor com salto (ProxyJump) ainda não disponível");
                        return Err(ErroResposta::EntregaFalhou);
                    }
                    (&s.host, s.porta, &s.usuario, &s.autenticacao, false)
                }
                Dados::FtpSftp(f) => (&f.host, f.porta, &f.usuario, &f.autenticacao, true),
                _ => return Err(ErroResposta::TipoSemEntrega),
            };
            let mascara = Mascara::da_entrada(e, None).map_err(|erro| {
                aviso!("ssh: {erro}");
                ErroResposta::EntregaFalhou
            })?;
            let (travas, inicial) = self.travas_agora(&est);
            (
                host.clone(),
                porta,
                usuario.clone(),
                cred_de(auth),
                sftp,
                mascara,
                travas,
                inicial,
            )
        };
        let (senha, agente) = match cred {
            Cred::Senha(s) => (Some(s), None),
            Cred::Chave { chave, passphrase } => {
                let rt = self.temporarios().ok_or(ErroResposta::EntregaFalhou)?;
                let ag = AgenteSsh::criar(rt, &chave, passphrase.as_ref()).map_err(
                    |e: ErroAgente| {
                        aviso!("ssh: {e}");
                        ErroResposta::EntregaFalhou
                    },
                )?;
                (None, Some(ag))
            }
        };
        let caminhos = agente.as_ref().map(|a| (a.socket(), a.publica()));
        let autent = match &caminhos {
            None => Autent::Senha,
            Some((sock, publica)) => Autent::Chave {
                agente: sock,
                publica,
            },
        };
        let programa = if sftp {
            Programa::Sftp
        } else {
            Programa::Ssh { comando: &comando }
        };
        let destino = Destino {
            host: &host,
            porta,
            usuario: &usuario,
        };
        let argv = montar_argv(
            self.programas_ssh(),
            &self.known_hosts(),
            destino,
            autent,
            programa,
        )
        .ok_or_else(|| {
            aviso!("ssh: host, usuário ou caminho fora do formato; nada foi executado");
            ErroResposta::EntregaFalhou
        })?;
        let cwd = {
            use std::os::unix::ffi::OsStringExt;
            ward_plataforma::pasta_pessoal()
                .map(OsStringExt::into_vec)
                .unwrap_or_else(|_| b"/".to_vec())
        };
        let invalido = |_| ErroResposta::EntregaFalhou;
        let mut cmd = Comando::novo(&argv[0], &cwd).map_err(invalido)?;
        for x in &argv[1..] {
            cmd.argumento(x).map_err(invalido)?;
        }
        // Ambiente do zero (D13, R19): nada do daemon nem da sessão do usuário.
        cmd.variavel(b"PATH", PATH_FILHO).map_err(invalido)?;
        cmd.variavel(b"LANG", b"C.UTF-8").map_err(invalido)?;
        if janela.is_some() {
            cmd.variavel(b"TERM", TERM_PTY).map_err(invalido)?;
        }
        if let Some(s) = &senha {
            // `sshpass -e`: a senha só no ambiente do filho, nunca em argv (S3).
            cmd.variavel(b"SSHPASS", s.expor()).map_err(invalido)?;
        }
        drop(senha);
        depurar!(
            "ssh: {} argumento(s), {}, {}",
            argv.len(),
            if sftp { "sftp" } else { "ssh" },
            if janela.is_some() { "pty" } else { "sem pty" }
        );
        Ok(Preparo {
            comando: cmd,
            mascara,
            temporarios: None,
            pty: janela,
            agente: agente.map(|a| AgentePreparado {
                agente: a,
                filho_do_filho: sftp,
                travas,
                inicial,
            }),
        })
    }
}

/// Atende um `ssh` do papel agente, como o [`crate::execucao::sessao`] do `ward run`.
pub fn sessao(
    daemon: &Daemon,
    cliente: &dyn Cliente,
    stream: &std::os::unix::net::UnixStream,
    pedido: Pedido,
) -> bool {
    crate::execucao::sessao_com(
        stream,
        |c| {
            let r = daemon.preparar_ssh(c, pedido);
            if let Err(e) = &r {
                depurar!("ssh (agente): {}", e.codigo());
            }
            r
        },
        cliente,
    )
}

// Fora de `src/ssh/`: o V3.15 lê só os arquivos de primeiro nível de `src/`.
#[cfg(test)]
#[path = "ssh_testes.rs"]
mod testes;
