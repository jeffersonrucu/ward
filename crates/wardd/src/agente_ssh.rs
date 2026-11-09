//! Agente SSH temporário do `ward ssh` com chave (D12, Fase 27; S1, S2, S8, S13).
//!
//! - **Só aquela conexão:** socket em `<rt>ssh-<aleatório>/agent.sock`, numa pasta 0700
//!   criada para a sessão; o `ssh` filho o recebe por `IdentityAgent` (D13).
//! - **Só o `ssh` filho:** cada conexão passa pelo `SO_PEERCRED` (mesmo uid) e o pid do
//!   par tem de ser o do filho da sessão. No `sftp`, o agente fica **preso ao primeiro
//!   par aceito** cujo pai é o `sftp` filho (o `ssh` que o `sftp` cria, que conecta antes
//!   de qualquer comando: com `BatchMode=yes`, sem o agente não há sessão) e, daí em
//!   diante, só esse pid exato passa. Um `!cmd` do `sftp` (outro filho dele: `ssh-add -L`,
//!   `ssh` para outro host com a chave) é recusado (auditoria do M4). Qualquer outro
//!   processo (um `ssh-add` do mesmo usuário) tem a conexão fechada sem nenhum byte.
//! - **Só lista e assina:** `REQUEST_IDENTITIES` devolve só a chave da entrada e
//!   `SIGN_REQUEST` assina só com ela. Adicionar, remover, travar, destravar e as
//!   extensões respondem `FAILURE`.
//! - **A chave privada nunca sai do daemon:** é lida da entrada (OpenSSH, com
//!   passphrase pelo `bcrypt-pbkdf` do RustCrypto) e assina com `ed25519-dalek`. Só
//!   Ed25519 (RSA ficaria com RUSTSEC-2023-0071). Nenhum arquivo com a chave privada é
//!   gravado: a pasta tem só o socket e a chave **pública** (`id.pub`), que o `ssh` usa
//!   para escolher a chave do agente com `IdentitiesOnly=yes`.
//! - **Some:** socket, chave pública e pasta somem quando a sessão acaba (o `ssh`
//!   termina, falha ou é morto, ou a CLI cai) e quando o cofre trava (a contagem de
//!   travas do núcleo muda); a chave decifrada zera ao sair.
//! - O log tem só o tipo da mensagem e o pid (S13).

use std::fs;
use std::io::{self, Read, Write};
use std::os::fd::AsFd;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use ssh_key::encoding::Encode;
use ssh_key::{Algorithm, PrivateKey, SigningKey};
use zeroize::Zeroizing;

use ward_core::entrada::Segredo;
use ward_plataforma::{DiretorioSocket, ErroAceitar, Escuta};

use crate::{aviso, depurar};

/// Nome do socket dentro da pasta da sessão.
pub const NOME_SOCKET: &str = "agent.sock";
/// Nome da chave pública dentro da pasta da sessão.
pub const NOME_PUBLICA: &str = "id.pub";
/// Prefixo da pasta da sessão em `<rt>`.
pub const PREFIXO_PASTA: &str = "ssh-";
/// Maior mensagem aceita (a do `ssh-agent` do OpenSSH).
const MAX_MENSAGEM: usize = 256 * 1024;
/// Conexões do `ssh` atendidas ao mesmo tempo.
const MAX_CONEXOES: usize = 4;
/// De quanto em quanto tempo as threads olham se devem parar (trava, fim da sessão).
const PASSO_MS: i32 = 100;
/// Prazo para ler uma mensagem já começada.
const PRAZO_MENSAGEM: Duration = Duration::from_secs(10);

// Números do protocolo do agente (draft-miller-ssh-agent).
const FAILURE: u8 = 5;
const REQUEST_IDENTITIES: u8 = 11;
const IDENTITIES_ANSWER: u8 = 12;
const SIGN_REQUEST: u8 = 13;
const SIGN_RESPONSE: u8 = 14;

/// Por que o agente não subiu. Sem valor (pode ir para o log).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroAgente {
    /// A chave da entrada não é uma chave OpenSSH legível.
    ChaveInvalida,
    /// A chave tem passphrase e ela não está na entrada, ou não confere.
    Passphrase,
    /// Algoritmo que o agente não assina (só Ed25519).
    AlgoritmoNaoSuportado,
    /// Pasta, socket ou chave pública não foram criados.
    Sistema,
}

impl std::fmt::Display for ErroAgente {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ErroAgente::ChaveInvalida => "a chave privada da entrada não é uma chave OpenSSH",
            ErroAgente::Passphrase => "a passphrase da chave falta ou não confere",
            ErroAgente::AlgoritmoNaoSuportado => "só chaves Ed25519 são aceitas pelo agente",
            ErroAgente::Sistema => "não deu para criar o socket do agente",
        })
    }
}

/// A chave decifrada e o que vai ao `ssh` (o blob público).
struct Chave {
    privada: PrivateKey,
    blob: Vec<u8>,
}

impl Chave {
    fn ler(texto: &Segredo, passphrase: Option<&Segredo>) -> Result<Chave, ErroAgente> {
        let texto = std::str::from_utf8(texto.expor()).map_err(|_| ErroAgente::ChaveInvalida)?;
        let lida = PrivateKey::from_openssh(texto).map_err(|_| ErroAgente::ChaveInvalida)?;
        let privada = if lida.is_encrypted() {
            let p = passphrase.ok_or(ErroAgente::Passphrase)?;
            lida.decrypt(p.expor())
                .map_err(|_| ErroAgente::Passphrase)?
        } else {
            lida
        };
        if privada.algorithm() != Algorithm::Ed25519 {
            return Err(ErroAgente::AlgoritmoNaoSuportado);
        }
        let blob = privada
            .public_key()
            .key_data()
            .encode_vec()
            .map_err(|_| ErroAgente::ChaveInvalida)?;
        Ok(Chave { privada, blob })
    }

    /// A chave pública em texto (`ssh-ed25519 AAAA… ward`), sem comentário da entrada.
    fn publica_em_texto(&self) -> Result<String, ErroAgente> {
        let mut p = self.privada.public_key().clone();
        p.set_comment("ward");
        p.to_openssh().map_err(|_| ErroAgente::ChaveInvalida)
    }

    fn assinar(&self, dados: &[u8]) -> Option<Vec<u8>> {
        assinar_com(&self.privada, dados)?.encode_vec().ok()
    }
}

/// Assina pela biblioteca (o `Signer` do `ssh-key`, Ed25519 do `ed25519-dalek`).
fn assinar_com<S: SigningKey>(chave: &S, dados: &[u8]) -> Option<ssh_key::Signature> {
    chave.try_sign(dados).ok()
}

/// Quem pode usar o agente: o pid do filho da sessão.
#[derive(Debug, Clone, Copy)]
pub struct Permitido {
    pub pid: u32,
    /// `sftp`: o `ssh` é neto do daemon; vale o processo cujo pai é o filho.
    pub filho_do_filho: bool,
}

/// Agente criado (pasta, socket e chave pública prontos), à espera do pid do filho.
/// Ao sair (sem ter sido iniciado, ou no fim da thread), a chave zera e a pasta some.
pub struct AgenteSsh {
    pasta: PathBuf,
    dir: Option<DiretorioSocket>,
    escuta: Option<Escuta>,
    chave: Option<Arc<Chave>>,
}

impl Drop for AgenteSsh {
    fn drop(&mut self) {
        // O socket some com a `Escuta`; a chave decifrada zera ao sair.
        self.escuta.take();
        self.chave.take();
        self.dir.take();
        apagar_pasta(&self.pasta);
    }
}

impl std::fmt::Debug for AgenteSsh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AgenteSsh(••••)")
    }
}

/// Apaga a pasta da sessão inteira (sem seguir link).
fn apagar_pasta(pasta: &Path) {
    if let Err(e) = fs::remove_dir_all(pasta)
        && e.kind() != io::ErrorKind::NotFound
    {
        aviso!(
            "agente SSH: não deu para apagar a pasta da sessão ({})",
            e.kind()
        );
    }
}

impl AgenteSsh {
    /// Lê a chave e cria `<rt>ssh-<aleatório>/` (0700) com `agent.sock` (0600) e
    /// `id.pub`. A chave é conferida antes de qualquer arquivo existir.
    pub fn criar(
        rt: &Path,
        chave: &Segredo,
        passphrase: Option<&Segredo>,
    ) -> Result<AgenteSsh, ErroAgente> {
        let chave = Chave::ler(chave, passphrase)?;
        let publica = chave.publica_em_texto()?;
        ward_plataforma::preparar_diretorio(rt).map_err(|_| ErroAgente::Sistema)?;
        let sufixo = ward_core::gerador::token_conectar().map_err(|_| ErroAgente::Sistema)?;
        let sufixo = std::str::from_utf8(&sufixo.expor()[..16]).map_err(|_| ErroAgente::Sistema)?;
        let pasta = rt.join(format!("{PREFIXO_PASTA}{sufixo}"));
        // Pasta nova: nunca reaproveita uma que já existe.
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&pasta)
            .map_err(|_| ErroAgente::Sistema)?;
        let montar = || -> Result<(DiretorioSocket, Escuta), ErroAgente> {
            let dir =
                ward_plataforma::preparar_diretorio(&pasta).map_err(|_| ErroAgente::Sistema)?;
            let escuta = dir.escutar(NOME_SOCKET).map_err(|_| ErroAgente::Sistema)?;
            escuta
                .listener()
                .set_nonblocking(true)
                .map_err(|_| ErroAgente::Sistema)?;
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(pasta.join(NOME_PUBLICA))
                .map_err(|_| ErroAgente::Sistema)?;
            f.write_all(publica.as_bytes())
                .and_then(|()| f.write_all(b"\n"))
                .map_err(|_| ErroAgente::Sistema)?;
            Ok((dir, escuta))
        };
        match montar() {
            Ok((dir, escuta)) => Ok(AgenteSsh {
                pasta,
                dir: Some(dir),
                escuta: Some(escuta),
                chave: Some(Arc::new(chave)),
            }),
            Err(e) => {
                apagar_pasta(&pasta);
                Err(e)
            }
        }
    }

    /// Caminho do socket (vai ao `ssh` por `IdentityAgent`).
    pub fn socket(&self) -> PathBuf {
        self.pasta.join(NOME_SOCKET)
    }

    /// Caminho da chave pública (vai ao `ssh` por `IdentityFile`, com
    /// `IdentitiesOnly=yes`).
    pub fn publica(&self) -> PathBuf {
        self.pasta.join(NOME_PUBLICA)
    }

    /// Começa a atender só `permitido`, até o [`Rodando`] sair ou a contagem de travas
    /// do núcleo mudar (`travas` com o valor `trava_inicial` de quando a sessão foi
    /// autorizada).
    pub fn iniciar(
        self,
        permitido: Permitido,
        travas: Arc<AtomicU64>,
        trava_inicial: u64,
    ) -> Rodando {
        let parar = Arc::new(AtomicBool::new(false));
        let pasta = self.pasta.clone();
        let p = parar.clone();
        let thread = std::thread::Builder::new()
            .name("wardd-agente-ssh".into())
            .spawn(move || laco(self, permitido, &p, &travas, trava_inicial));
        if thread.is_err() {
            aviso!("agente SSH: a thread não foi criada");
            apagar_pasta(&pasta);
        }
        Rodando {
            parar,
            thread: thread.ok(),
            pasta,
        }
    }
}

/// O agente atendendo. Ao sair, para as threads e apaga a pasta da sessão.
pub struct Rodando {
    parar: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    pasta: PathBuf,
}

impl std::fmt::Debug for Rodando {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Rodando(agente SSH)")
    }
}

impl Drop for Rodando {
    fn drop(&mut self) {
        self.parar.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        apagar_pasta(&self.pasta);
    }
}

/// Deve parar: a sessão acabou ou o cofre travou desde o começo da sessão.
fn deve_parar(parar: &AtomicBool, travas: &AtomicU64, inicial: u64) -> bool {
    parar.load(Ordering::Acquire) || travas.load(Ordering::Acquire) != inicial
}

/// O par é o `ssh` da sessão? No `sftp`, `preso` guarda o pid do primeiro filho do
/// `sftp` aceito; depois dele, só esse pid passa (nenhum outro filho do `sftp`).
fn par_permitido(
    par: &ward_plataforma::ProcessoPar,
    p: Permitido,
    preso: &mut Option<u32>,
) -> bool {
    par_permitido_pid(par.credenciais().pid, || par.pid_do_pai(), p, preso)
}

/// [`par_permitido`] com o pid do par e o pai dele já lidos (testável sem processo).
fn par_permitido_pid(
    pid: u32,
    pai: impl FnOnce() -> Option<u32>,
    p: Permitido,
    preso: &mut Option<u32>,
) -> bool {
    if !p.filho_do_filho {
        return pid == p.pid;
    }
    match *preso {
        Some(fixo) => pid == fixo,
        None => {
            if pid != p.pid && pai() == Some(p.pid) {
                *preso = Some(pid);
                true
            } else {
                false
            }
        }
    }
}

fn laco(
    mut agente: AgenteSsh,
    permitido: Permitido,
    parar: &AtomicBool,
    travas: &AtomicU64,
    inicial: u64,
) {
    let (Some(escuta), Some(chave)) = (agente.escuta.take(), agente.chave.take()) else {
        return;
    };
    let abertas = Arc::new(AtomicUsize::new(0));
    let parar_conexoes = Arc::new(AtomicBool::new(false));
    let mut conexoes = Vec::new();
    let mut preso = None;
    while !deve_parar(parar, travas, inicial) {
        match ward_plataforma::filho::pronto_para_ler_fd(escuta.listener().as_fd(), PASSO_MS) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(_) => break,
        }
        let stream = match escuta.listener().accept() {
            Ok((s, _)) => s,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        let par = match ward_plataforma::conferir_conexao(&stream) {
            Ok(p) => p,
            Err(ErroAceitar::UidRecusado { uid, pid }) => {
                aviso!("agente SSH: conexão recusada (uid {uid}, pid {pid}) (D12)");
                continue;
            }
            Err(_) => {
                aviso!("agente SSH: conexão recusada (sem identidade) (D12)");
                continue;
            }
        };
        let pid = par.credenciais().pid;
        if !par_permitido(&par, permitido, &mut preso) {
            // Fecha sem nenhum byte: só o `ssh` da sessão usa o agente.
            aviso!("agente SSH: conexão do pid {pid} recusada; só o ssh da sessão (D12)");
            continue;
        }
        if abertas.load(Ordering::Acquire) >= MAX_CONEXOES {
            aviso!("agente SSH: conexões demais do pid {pid}; fechada");
            continue;
        }
        depurar!("agente SSH: conexão do pid {pid}");
        abertas.fetch_add(1, Ordering::AcqRel);
        let (c, a, pc) = (chave.clone(), abertas.clone(), parar_conexoes.clone());
        let t = std::thread::Builder::new()
            .name("wardd-agente-con".into())
            .spawn(move || {
                atender(stream, &c, &pc);
                a.fetch_sub(1, Ordering::AcqRel);
            });
        match t {
            Ok(t) => conexoes.push(t),
            Err(_) => {
                abertas.fetch_sub(1, Ordering::AcqRel);
            }
        }
    }
    if travas.load(Ordering::Acquire) != inicial {
        aviso!("agente SSH: o cofre travou; agente derrubado (D12)");
    }
    parar_conexoes.store(true, Ordering::Release);
    for t in conexoes {
        let _ = t.join();
    }
    // A chave decifrada zera ao sair; o socket some com a `Escuta`, e a pasta, no
    // `Drop` do agente.
    drop(chave);
    drop(escuta);
    drop(agente);
}

/// Lê exatamente `buf.len()` bytes, olhando `parar` entre as esperas. `false`: fim,
/// erro, prazo ou parada.
fn ler_exato(s: &mut UnixStream, buf: &mut [u8], parar: &AtomicBool, prazo: bool) -> bool {
    let mut lido = 0;
    let limite = std::time::Instant::now() + PRAZO_MENSAGEM;
    while lido < buf.len() {
        if parar.load(Ordering::Acquire) || (prazo && std::time::Instant::now() > limite) {
            return false;
        }
        match ward_plataforma::filho::pronto_para_ler_fd(s.as_fd(), PASSO_MS) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(_) => return false,
        }
        match s.read(&mut buf[lido..]) {
            Ok(0) => return false,
            Ok(n) => lido += n,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
                ) => {}
            Err(_) => return false,
        }
    }
    true
}

/// Atende uma conexão do `ssh` da sessão até ela fechar ou o agente parar.
fn atender(mut s: UnixStream, chave: &Chave, parar: &AtomicBool) {
    let _ = s.set_nonblocking(false);
    let _ = s.set_write_timeout(Some(PRAZO_MENSAGEM));
    loop {
        let mut n = [0u8; 4];
        if !ler_exato(&mut s, &mut n, parar, false) {
            return;
        }
        let tam = u32::from_be_bytes(n) as usize;
        if tam == 0 || tam > MAX_MENSAGEM {
            aviso!("agente SSH: mensagem de tamanho inválido; conexão fechada");
            return;
        }
        let mut msg = Zeroizing::new(vec![0u8; tam]);
        if !ler_exato(&mut s, &mut msg, parar, true) {
            return;
        }
        let resposta = responder(&msg, chave);
        let mut quadro = Zeroizing::new(Vec::with_capacity(resposta.len() + 4));
        quadro.extend_from_slice(&u32::try_from(resposta.len()).unwrap_or(0).to_be_bytes());
        quadro.extend_from_slice(&resposta);
        if s.write_all(&quadro).is_err() {
            return;
        }
    }
}

/// `string` do protocolo SSH: u32 + bytes.
fn ler_string<'a>(b: &mut &'a [u8]) -> Option<&'a [u8]> {
    let (n, resto) = b.split_first_chunk::<4>()?;
    let n = u32::from_be_bytes(*n) as usize;
    if resto.len() < n {
        return None;
    }
    let (s, resto) = resto.split_at(n);
    *b = resto;
    Some(s)
}

fn por_string(v: &mut Vec<u8>, s: &[u8]) {
    v.extend_from_slice(&u32::try_from(s.len()).unwrap_or(0).to_be_bytes());
    v.extend_from_slice(s);
}

/// A resposta a uma mensagem do agente (sem o tamanho). Só lista e assina (D12).
pub(crate) fn responder_mensagem(
    msg: &[u8],
    blob: &[u8],
    assinar: &dyn Fn(&[u8]) -> Option<Vec<u8>>,
) -> Vec<u8> {
    let Some((&tipo, mut corpo)) = msg.split_first() else {
        return vec![FAILURE];
    };
    match tipo {
        REQUEST_IDENTITIES if corpo.is_empty() => {
            depurar!("agente SSH: lista de chaves");
            let mut r = vec![IDENTITIES_ANSWER];
            r.extend_from_slice(&1u32.to_be_bytes());
            por_string(&mut r, blob);
            por_string(&mut r, b"ward");
            r
        }
        SIGN_REQUEST => {
            let (Some(chave), Some(dados)) = (ler_string(&mut corpo), ler_string(&mut corpo))
            else {
                return vec![FAILURE];
            };
            // flags (u32): Ed25519 não usa.
            if corpo.len() != 4 || chave != blob {
                return vec![FAILURE];
            }
            match assinar(dados) {
                Some(assinatura) => {
                    depurar!("agente SSH: assinatura feita");
                    let mut r = vec![SIGN_RESPONSE];
                    por_string(&mut r, &assinatura);
                    r
                }
                None => vec![FAILURE],
            }
        }
        outro => {
            // Adicionar, remover, travar, destravar, extensões: recusado (D12).
            aviso!("agente SSH: mensagem {outro} recusada (só lista e assina, D12)");
            vec![FAILURE]
        }
    }
}

fn responder(msg: &[u8], chave: &Chave) -> Vec<u8> {
    responder_mensagem(msg, &chave.blob, &|d| chave.assinar(d))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn ident(blob: &[u8]) -> Vec<u8> {
        responder_mensagem(&[REQUEST_IDENTITIES], blob, &|_| Some(b"sig".to_vec()))
    }

    #[test]
    fn ataque_sftp_bang_outro_filho_do_sftp_e_recusado() {
        // Auditoria do M4: o `!cmd` do `sftp` é outro filho dele. Preso ao primeiro.
        let sftp = 500;
        let p = Permitido {
            pid: sftp,
            filho_do_filho: true,
        };
        let mut preso = None;
        // Nem o próprio `sftp` nem um estranho prendem o agente.
        assert!(!par_permitido_pid(sftp, || Some(1), p, &mut preso));
        assert!(!par_permitido_pid(77, || Some(1), p, &mut preso));
        assert_eq!(preso, None);
        // O `ssh` que o `sftp` criou conecta primeiro e prende.
        assert!(par_permitido_pid(501, || Some(sftp), p, &mut preso));
        assert_eq!(preso, Some(501));
        assert!(par_permitido_pid(501, || Some(sftp), p, &mut preso));
        // `!ssh-add -L`, `!exec ssh …`: outro filho do `sftp`, recusado.
        assert!(!par_permitido_pid(502, || Some(sftp), p, &mut preso));
        assert!(!par_permitido_pid(503, || Some(sftp), p, &mut preso));
        assert_eq!(preso, Some(501));
        // Sem `sftp`: só o pid exato do filho.
        let q = Permitido {
            pid: 600,
            filho_do_filho: false,
        };
        let mut nada = None;
        assert!(par_permitido_pid(600, || None, q, &mut nada));
        assert!(!par_permitido_pid(601, || Some(600), q, &mut nada));
        assert_eq!(nada, None);
    }

    #[test]
    fn so_lista_e_assina() {
        let blob = b"blob-publico";
        let r = ident(blob);
        assert_eq!(r[0], IDENTITIES_ANSWER);
        assert_eq!(&r[1..5], &1u32.to_be_bytes());
        // Assinatura só com a chave da entrada.
        let mut pedido = vec![SIGN_REQUEST];
        por_string(&mut pedido, blob);
        por_string(&mut pedido, b"dados");
        pedido.extend_from_slice(&0u32.to_be_bytes());
        let r = responder_mensagem(&pedido, blob, &|d| {
            assert_eq!(d, b"dados");
            Some(b"sig".to_vec())
        });
        assert_eq!(r[0], SIGN_RESPONSE);
        let mut outra = vec![SIGN_REQUEST];
        por_string(&mut outra, b"outra-chave");
        por_string(&mut outra, b"dados");
        outra.extend_from_slice(&0u32.to_be_bytes());
        assert_eq!(
            responder_mensagem(&outra, blob, &|_| panic!("não assina")),
            vec![FAILURE]
        );
    }

    #[test]
    fn ataque_adicionar_remover_travar_e_lixo_recusados() {
        let blob = b"blob";
        // 17 add, 18 remove, 19 remove all, 22 lock, 23 unlock, 25 add constrained,
        // 27 extension, 9 (v1), mensagem vazia e truncada.
        for m in [
            &[17u8, 0, 0, 0, 3, b'x', b'y', b'z'][..],
            &[18, 0, 0, 0, 4, b'b', b'l', b'o', b'b'],
            &[19],
            &[22, 0, 0, 0, 1, b'p'],
            &[23, 0, 0, 0, 1, b'p'],
            &[25],
            &[27, 0, 0, 0, 1, b'e'],
            &[9],
            &[],
            &[SIGN_REQUEST, 0, 0, 0, 9, b'b'],
            &[REQUEST_IDENTITIES, 1],
        ] {
            assert_eq!(
                responder_mensagem(m, blob, &|_| panic!("não assina")),
                vec![FAILURE],
                "{m:?}"
            );
        }
    }
}
