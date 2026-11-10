//! Canal do token do "Conectar" (Fase 27a; D3; achado alto da auditoria do M3).
//!
//! O token de uso único que o app recebe do daemon pula o nível e a aprovação, então
//! nenhum outro processo do mesmo uid pode lê-lo. Ele **nunca** vai em argv nem em
//! variável de ambiente: os dois ficam em `/proc/<pid>/cmdline` e `/proc/<pid>/environ`,
//! legíveis pelo mesmo uid (e o `execve` volta o `dumpable` do terminal e do `ward` a 1).
//!
//! O caminho é um par de sockets Unix (`socketpair`):
//! - o app ([`CanalConectar::novo`]) escreve o token na sua ponta e a fecha; os bytes
//!   ficam na fila de leitura da outra ponta;
//! - só a outra ponta passa pelo `exec` do terminal ([`CanalConectar::preparar`]: todo
//!   outro fd ≥ 3 do app fecha no `exec`), e o terminal a repassa ao `ward ssh`, que
//!   recebe o número em `--token-fd <n>`;
//! - o `ward ssh` ([`ler_token_herdado`]) lê o token, fecha o fd e só então conecta.
//!
//! Por que socket e não pipe, memfd ou arquivo: o mesmo uid abre de novo um pipe, um
//! memfd ou um arquivo de outro processo por `/proc/<pid>/fd/<n>` (basta o
//! `PTRACE_MODE_READ`, que o Yama não restringe). Um socket não reabre por ali
//! (`ENXIO`). Tirar o fd de outro processo exige `pidfd_getfd`, que pede
//! `PTRACE_MODE_ATTACH`: com `ptrace_scope` ≥ 1, que o Ward exige (D47), só um ancestral
//! do terminal conseguiria. Por isso a leitura recusa fd que não seja socket Unix de
//! fluxo: um pipe passado no lugar seria legível por outros.

use std::fmt;
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, AsRawFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::process::Command;
use std::time::Duration;

use zeroize::Zeroizing;

use crate::sys;

/// Maior token aceito no canal (o formato do daemon: 16 a 64 bytes).
pub const MAX_TOKEN: usize = 64;

/// Quanto o `ward ssh` espera pelo token no canal. O app escreve e fecha a ponta dele
/// antes de abrir o terminal, então o token já está lá; o prazo só evita ficar parado
/// num socket que nunca fecha.
pub const PRAZO_LEITURA: Duration = Duration::from_secs(2);

/// Maior número de fd aceito em `--token-fd`.
pub const MAX_NUMERO_FD: u32 = 1 << 20;

/// A ponta do canal que vai ao terminal. O app a mantém viva até o `spawn` voltar e
/// então a solta (`drop`): o terminal fica com a única cópia.
pub struct CanalConectar {
    filho: OwnedFd,
}

impl fmt::Debug for CanalConectar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CanalConectar(fd {})", self.filho.as_raw_fd())
    }
}

impl CanalConectar {
    /// Cria o par, escreve `token` na ponta do app e a fecha. As duas pontas nascem com
    /// `FD_CLOEXEC`: nenhum outro processo que o app criar ao mesmo tempo as herda.
    pub fn novo(token: &[u8]) -> io::Result<CanalConectar> {
        if token.is_empty() || token.len() > MAX_TOKEN {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let (mut app, filho) = UnixStream::pair()?;
        app.write_all(token)?;
        drop(app);
        Ok(CanalConectar {
            filho: filho.into(),
        })
    }

    /// O número que vai em `--token-fd`.
    pub fn numero(&self) -> RawFd {
        self.filho.as_raw_fd()
    }

    /// Faz o `exec` de `cmd` levar só este fd além de 0–2 (todo outro fd ≥ 3 fecha).
    /// O canal tem de continuar vivo até o `spawn` de `cmd` voltar.
    pub fn preparar(&self, cmd: &mut Command) {
        sys::herdar_so_no_exec(cmd, self.filho.as_raw_fd());
    }
}

/// Por que o token não foi lido. Só textos fixos: nunca o conteúdo do canal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErroCanal {
    /// Número fora de 3..=[`MAX_NUMERO_FD`], ou fd fechado.
    Numero,
    /// O fd não é socket Unix de fluxo (pipe, arquivo, memfd, socket em escuta…).
    NaoSocket,
    /// O outro lado do socket é de outro usuário.
    OutroUsuario,
    /// O canal fechou sem nenhum byte.
    Vazio,
    /// Mais que [`MAX_TOKEN`] bytes.
    Grande,
    /// Nada chegou dentro de [`PRAZO_LEITURA`].
    Prazo,
    /// Outro erro de E/S.
    Io(io::ErrorKind),
}

impl ErroCanal {
    pub fn mensagem(self) -> &'static str {
        match self {
            ErroCanal::Numero => "o canal do token do Conectar não está aberto",
            ErroCanal::NaoSocket => "o canal do token do Conectar não é o do app",
            ErroCanal::OutroUsuario => "o canal do token do Conectar é de outro usuário",
            ErroCanal::Vazio => "o canal do token do Conectar veio vazio",
            ErroCanal::Grande => "o canal do token do Conectar passa do tamanho do token",
            ErroCanal::Prazo => "o token do Conectar não chegou a tempo",
            ErroCanal::Io(_) => "erro ao ler o canal do token do Conectar",
        }
    }
}

impl fmt::Display for ErroCanal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.mensagem())
    }
}

impl std::error::Error for ErroCanal {}

/// Lê o token do fd `numero` herdado do terminal e o fecha (sempre, inclusive em erro).
/// Confere antes de ler: socket Unix de fluxo, fora de escuta, com o outro lado do
/// mesmo uid. Lê até o fim do canal (o app fecha a ponta dele), no máximo
/// [`MAX_TOKEN`] bytes, num buffer que zera.
///
/// Chamada uma vez, no começo do processo (o `ward ssh`), com o número do argv.
pub fn ler_token_herdado(numero: u32) -> Result<Zeroizing<Vec<u8>>, ErroCanal> {
    if !(3..=MAX_NUMERO_FD).contains(&numero) {
        return Err(ErroCanal::Numero);
    }
    let n = RawFd::try_from(numero).map_err(|_| ErroCanal::Numero)?;
    let fd = sys::tomar_socket_herdado(n).map_err(|e| match e.raw_os_error() {
        Some(libc::ENOTSOCK) => ErroCanal::NaoSocket,
        _ => ErroCanal::Numero,
    })?;
    conferir_socket(&fd)?;
    let mut s = UnixStream::from(fd);
    s.set_read_timeout(Some(PRAZO_LEITURA))
        .map_err(|e| ErroCanal::Io(e.kind()))?;
    let r = ler_ate_o_fim(&mut s);
    drop(s);
    r
}

fn conferir_socket(fd: &OwnedFd) -> Result<(), ErroCanal> {
    let b = fd.as_fd();
    let dominio = sys::sockopt_int(b, libc::SO_DOMAIN).map_err(|_| ErroCanal::NaoSocket)?;
    let tipo = sys::sockopt_int(b, libc::SO_TYPE).map_err(|_| ErroCanal::NaoSocket)?;
    let escuta = sys::sockopt_int(b, libc::SO_ACCEPTCONN).map_err(|_| ErroCanal::NaoSocket)?;
    if dominio != libc::AF_UNIX || tipo != libc::SOCK_STREAM || escuta != 0 {
        return Err(ErroCanal::NaoSocket);
    }
    let cred = sys::peercred(b).map_err(|_| ErroCanal::NaoSocket)?;
    if cred.uid != sys::geteuid() {
        return Err(ErroCanal::OutroUsuario);
    }
    Ok(())
}

fn ler_ate_o_fim(s: &mut impl Read) -> Result<Zeroizing<Vec<u8>>, ErroCanal> {
    // Um byte a mais que o máximo: se ele chegar, o canal passou do tamanho.
    let mut buf = Zeroizing::new([0u8; MAX_TOKEN + 1]);
    let mut lidos = 0;
    while lidos < buf.len() {
        match s.read(&mut buf[lidos..]) {
            Ok(0) => break,
            Ok(n) => lidos += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                return Err(ErroCanal::Prazo);
            }
            Err(e) => return Err(ErroCanal::Io(e.kind())),
        }
    }
    if lidos == 0 {
        return Err(ErroCanal::Vazio);
    }
    if lidos > MAX_TOKEN {
        return Err(ErroCanal::Grande);
    }
    let mut v = Zeroizing::new(Vec::with_capacity(lidos));
    v.extend_from_slice(&buf[..lidos]);
    Ok(v)
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::os::fd::IntoRawFd;

    const TOKEN: &[u8] = b"AbCdEfGhIjKlMnOpQrStUvWxYz012345";

    /// O número de um fd cuja posse sai deste teste (vai para o `ler_token_herdado`,
    /// como no `ward ssh`, que recebe o fd do pai).
    fn entregar(fd: impl Into<OwnedFd>) -> u32 {
        u32::try_from(fd.into().into_raw_fd()).unwrap()
    }

    /// O que `/proc/self/fd/<n>` aponta agora (`socket:[inode]`, …). Os testes rodam em
    /// paralelo e o número pode ser reaproveitado, mas um socket fechado nunca volta com
    /// o mesmo inode.
    fn alvo(n: u32) -> Option<std::path::PathBuf> {
        std::fs::read_link(format!("/proc/self/fd/{n}")).ok()
    }

    #[test]
    fn token_vai_e_volta_pelo_canal_e_o_fd_fecha() {
        let c = CanalConectar::novo(TOKEN).unwrap();
        let n = entregar(c.filho);
        let antes = alvo(n);
        assert!(antes.is_some());
        let t = ler_token_herdado(n).unwrap();
        assert_eq!(t.as_slice(), TOKEN);
        assert_ne!(alvo(n), antes, "o fd do canal continua aberto");
    }

    #[test]
    fn as_pontas_nascem_com_cloexec() {
        let c = CanalConectar::novo(TOKEN).unwrap();
        assert!(sys::tem_cloexec(c.filho.as_fd()).unwrap());
        assert!(!format!("{c:?}").contains("AbCd"));
    }

    /// Ataque: canal com lixo grande, vazio, que não fecha, ou que não é socket (pipe e
    /// arquivo, que o mesmo uid reabre por `/proc/<pid>/fd`); número 0–2, fora da faixa
    /// e fechado. Tudo recusado, e a mensagem nunca traz o conteúdo.
    #[test]
    fn ataque_canal_forjado_e_recusado_sem_eco() {
        let lixo = b"falso-lixo-do-canal-".repeat(40);
        let (mut a, b) = UnixStream::pair().unwrap();
        a.write_all(&lixo).unwrap();
        drop(a);
        let e = ler_token_herdado(entregar(b)).unwrap_err();
        assert_eq!(e, ErroCanal::Grande);
        assert!(!e.mensagem().contains("falso"));

        // 65 bytes: um a mais que o máximo.
        let (mut a, b) = UnixStream::pair().unwrap();
        a.write_all(&[b'a'; MAX_TOKEN + 1]).unwrap();
        drop(a);
        assert_eq!(ler_token_herdado(entregar(b)), Err(ErroCanal::Grande));

        let (a, b) = UnixStream::pair().unwrap();
        drop(a);
        assert_eq!(ler_token_herdado(entregar(b)), Err(ErroCanal::Vazio));

        // O outro lado nunca fecha nem escreve: o prazo vence e o fd fecha.
        let (a, b) = UnixStream::pair().unwrap();
        let n = entregar(b);
        let antes = alvo(n);
        assert_eq!(ler_token_herdado(n), Err(ErroCanal::Prazo));
        assert_ne!(alvo(n), antes, "o fd do canal continua aberto");
        drop(a);

        let (r, mut w) = std::io::pipe().unwrap();
        w.write_all(TOKEN).unwrap();
        drop(w);
        assert_eq!(ler_token_herdado(entregar(r)), Err(ErroCanal::NaoSocket));

        let f = std::fs::File::open("/proc/self/status").unwrap();
        assert_eq!(ler_token_herdado(entregar(f)), Err(ErroCanal::NaoSocket));

        // Socket em escuta e socket de datagrama.
        let pasta =
            crate::socket::diretorio_runtime().join(format!("ward-canal-t-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        std::fs::create_dir(&pasta).unwrap();
        let l = std::os::unix::net::UnixListener::bind(pasta.join("s")).unwrap();
        assert_eq!(ler_token_herdado(entregar(l)), Err(ErroCanal::NaoSocket));
        let _ = std::fs::remove_dir_all(&pasta);
        let (d, _d2) = std::os::unix::net::UnixDatagram::pair().unwrap();
        assert_eq!(ler_token_herdado(entregar(d)), Err(ErroCanal::NaoSocket));

        for n in [0, 1, 2, MAX_NUMERO_FD + 1, u32::MAX] {
            assert_eq!(ler_token_herdado(n), Err(ErroCanal::Numero), "{n}");
        }
        // Fechado (um número alto que este processo não usa).
        assert_eq!(ler_token_herdado(MAX_NUMERO_FD - 7), Err(ErroCanal::Numero));
    }

    /// Ataque (o motivo do socket): outro processo do mesmo uid tenta reabrir o canal
    /// por `/proc/<pid>/fd/<n>` de quem o segura. Com pipe isso funcionaria; com o
    /// socket, o `open` falha (`ENXIO`) e o token continua só na fila do canal.
    #[test]
    fn ataque_reabrir_o_canal_por_proc_falha() {
        let c = CanalConectar::novo(TOKEN).unwrap();
        let caminho = format!("/proc/self/fd/{}", c.numero());
        let e = std::fs::File::open(&caminho).unwrap_err();
        assert_eq!(e.raw_os_error(), Some(libc::ENXIO), "{e}");
        assert!(
            std::fs::OpenOptions::new()
                .write(true)
                .open(&caminho)
                .is_err()
        );
        // O token continua inteiro para o dono do fd.
        let t = ler_token_herdado(entregar(c.filho)).unwrap();
        assert_eq!(t.as_slice(), TOKEN);
        // Contraprova: um pipe no mesmo lugar seria reaberto e lido.
        let (r, mut w) = std::io::pipe().unwrap();
        w.write_all(b"falso-pelo-pipe").unwrap();
        let mut outro = std::fs::File::open(format!("/proc/self/fd/{}", r.as_raw_fd())).unwrap();
        drop(w);
        let mut lido = Vec::new();
        outro.read_to_end(&mut lido).unwrap();
        assert_eq!(lido, b"falso-pelo-pipe");
    }

    /// O `exec` leva só o canal (além de 0–2): um fd do processo sem `FD_CLOEXEC` não
    /// passa, e o canal passa com o mesmo número. Confere pelo alvo do link (inode), não
    /// só pelo número, que o `ls` pode reaproveitar.
    #[test]
    fn exec_leva_so_o_canal() {
        let c = CanalConectar::novo(TOKEN).unwrap();
        let (solto, _w) = std::io::pipe().unwrap();
        sys::tirar_cloexec(solto.as_fd()).unwrap();
        let alvo_solto = alvo(u32::try_from(solto.as_raw_fd()).unwrap()).unwrap();
        let alvo_canal = alvo(u32::try_from(c.numero()).unwrap()).unwrap();
        let mut cmd = Command::new("/bin/ls");
        cmd.args(["-l", "/proc/self/fd"])
            .env_clear()
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        c.preparar(&mut cmd);
        let saida = String::from_utf8(cmd.output().unwrap().stdout).unwrap();
        let canal = format!("{} -> {}", c.numero(), alvo_canal.display());
        assert!(saida.contains(&canal), "{saida}");
        assert!(!saida.contains(&*alvo_solto.to_string_lossy()), "{saida}");
        // 0–2, o canal e o fd que o próprio `ls` abre para ler o diretório.
        assert!(
            saida.lines().filter(|l| l.contains(" -> ")).count() <= 5,
            "{saida}"
        );
        assert!(sys::tem_cloexec(c.filho.as_fd()).unwrap());
    }

    #[test]
    fn token_vazio_ou_grande_nem_vira_canal() {
        assert!(CanalConectar::novo(b"").is_err());
        assert!(CanalConectar::novo(&[b'a'; MAX_TOKEN + 1]).is_err());
    }
}
