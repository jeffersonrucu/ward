//! Senha mestra lida do terminal (Fase 25, `ward destravar`; "Destravar e travar":
//! "pela CLI no terminal do usuário"; S5, S8).
//!
//! - Lê de `/dev/tty`, nunca de stdin nem de argv (S3): um pipe ou um arquivo não
//!   servem, e a senha não passa pela linha de comando.
//! - O terminal fica sem eco, sem modo canônico e sem sinais (`ISIG`) durante a
//!   leitura, e volta ao estado de antes ao sair, inclusive por erro. Ctrl+C chega como
//!   byte e cancela; assim o processo não morre com o eco desligado.
//! - A entrada pendente é descartada antes do prompt (`TCSAFLUSH`): o que foi digitado
//!   antes de a pergunta aparecer não vira senha.
//! - O buffer é um `Zeroizing<Vec<u8>>` com a capacidade final reservada antes do
//!   primeiro byte: nenhuma realocação deixa cópia da senha para trás. O byte apagado
//!   (Backspace, Ctrl+U) é zerado antes de sair do buffer.

use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsFd;
use std::os::unix::fs::OpenOptionsExt;

use zeroize::{Zeroize, Zeroizing};

use crate::sys;

/// Por que a senha não foi lida. Sem conteúdo: pode ir para a mensagem ao usuário.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErroTerminal {
    /// Sem terminal (`/dev/tty` não abre ou não é terminal).
    SemTerminal,
    /// O usuário cancelou (Ctrl+C, Ctrl+D com a linha vazia, fim da entrada).
    Cancelado,
    /// Mais bytes que o limite.
    Longa,
    /// O eco não desligou, ou outro erro de E/S do terminal.
    Io(io::ErrorKind),
}

impl fmt::Display for ErroTerminal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErroTerminal::SemTerminal => f.write_str("sem terminal para ler a senha"),
            ErroTerminal::Cancelado => f.write_str("cancelado"),
            ErroTerminal::Longa => f.write_str("senha longa demais"),
            ErroTerminal::Io(k) => write!(f, "erro no terminal ({k})"),
        }
    }
}

impl std::error::Error for ErroTerminal {}

/// Abre `/dev/tty` e lê uma senha de até `max` bytes, sem eco, depois de escrever
/// `prompt` no próprio terminal.
pub fn ler_senha(prompt: &str, max: usize) -> Result<Zeroizing<Vec<u8>>, ErroTerminal> {
    let tty = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOCTTY | libc::O_CLOEXEC)
        .open("/dev/tty")
        .map_err(|_| ErroTerminal::SemTerminal)?;
    ler_senha_em(&tty, prompt, max)
}

/// Volta o terminal ao estado de antes no `Drop`.
struct Restaurar<'a> {
    tty: &'a File,
    original: libc::termios,
}

impl Drop for Restaurar<'_> {
    fn drop(&mut self) {
        let _ = sys::tcsetattr(self.tty.as_fd(), libc::TCSANOW, &self.original);
    }
}

/// [`ler_senha`] num terminal já aberto.
pub(crate) fn ler_senha_em(
    tty: &File,
    prompt: &str,
    max: usize,
) -> Result<Zeroizing<Vec<u8>>, ErroTerminal> {
    let original = sys::tcgetattr(tty.as_fd()).map_err(|_| ErroTerminal::SemTerminal)?;
    let mut cru = original;
    cru.c_lflag &= !(libc::ECHO | libc::ECHONL | libc::ICANON | libc::ISIG | libc::IEXTEN);
    cru.c_cc[libc::VMIN] = 1;
    cru.c_cc[libc::VTIME] = 0;
    let volta = Restaurar { tty, original };
    sys::tcsetattr(tty.as_fd(), libc::TCSAFLUSH, &cru).map_err(|e| ErroTerminal::Io(e.kind()))?;
    // Confere de volta: um terminal que aceitou o pedido sem desligar o eco mostraria
    // a senha na tela.
    let agora = sys::tcgetattr(tty.as_fd()).map_err(|e| ErroTerminal::Io(e.kind()))?;
    if agora.c_lflag & (libc::ECHO | libc::ICANON) != 0 {
        return Err(ErroTerminal::Io(io::ErrorKind::Unsupported));
    }
    let mut escrita = tty;
    escrita
        .write_all(prompt.as_bytes())
        .and_then(|()| escrita.flush())
        .map_err(|e| ErroTerminal::Io(e.kind()))?;
    let mut leitura = tty;
    let r = ler_linha(&mut leitura, max);
    let _ = escrita.write_all(b"\n");
    drop(volta);
    r
}

/// Lê bytes até Enter, um por vez, com a edição mínima de um terminal sem modo
/// canônico: Backspace/DEL apaga o último byte (zerado), Ctrl+U apaga a linha,
/// Ctrl+C cancela, Ctrl+D cancela com a linha vazia. Outros bytes de controle são
/// ignorados.
pub(crate) fn ler_linha(r: &mut impl Read, max: usize) -> Result<Zeroizing<Vec<u8>>, ErroTerminal> {
    let mut senha = Zeroizing::new(Vec::with_capacity(max));
    let mut b = Zeroizing::new([0u8; 1]);
    loop {
        match r.read(&mut b[..]) {
            Ok(0) => return Err(ErroTerminal::Cancelado),
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(ErroTerminal::Io(e.kind())),
        }
        match b[0] {
            b'\r' | b'\n' => break,
            0x03 => return Err(ErroTerminal::Cancelado),
            0x04 if senha.is_empty() => return Err(ErroTerminal::Cancelado),
            0x7f | 0x08 => {
                if let Some(ultimo) = senha.last_mut() {
                    *ultimo = 0;
                    let n = senha.len() - 1;
                    senha.truncate(n);
                }
            }
            0x15 => {
                senha.zeroize();
                senha.clear();
            }
            c if c < 0x20 => {}
            c => {
                if senha.len() >= max {
                    return Err(ErroTerminal::Longa);
                }
                senha.push(c);
            }
        }
    }
    b.zeroize();
    Ok(senha)
}

// --- `ward ssh` interativo (Fase 27) ---------------------------------------------------

/// `fd` é um terminal?
pub fn e_terminal(fd: impl AsFd) -> bool {
    sys::tcgetattr(fd.as_fd()).is_ok()
}

/// (linhas, colunas) do terminal em `fd`; `None` se não é terminal ou não diz.
pub fn tamanho(fd: impl AsFd) -> Option<(u16, u16)> {
    sys::read_window_size(fd.as_fd())
        .ok()
        .filter(|&(l, c)| l > 0 && c > 0)
}

/// O terminal da CLI em modo cru enquanto durar (`cfmakeraw`: sem eco, sem modo
/// canônico, sem sinais; Ctrl+C vai como byte ao `ssh` remoto, como no `ssh`). Volta ao
/// estado de antes no `Drop`, inclusive por erro ou pânico.
pub struct ModoCru {
    fd: std::os::fd::OwnedFd,
    original: libc::termios,
}

impl fmt::Debug for ModoCru {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ModoCru")
    }
}

impl ModoCru {
    /// Liga o modo cru no terminal `fd` (uma cópia do fd fica guardada para voltar).
    pub fn ligar(fd: impl AsFd) -> io::Result<ModoCru> {
        let fd = fd.as_fd().try_clone_to_owned()?;
        let original = sys::tcgetattr(fd.as_fd())?;
        let mut cru = original;
        sys::make_raw(&mut cru);
        sys::tcsetattr(fd.as_fd(), libc::TCSANOW, &cru)?;
        Ok(ModoCru { fd, original })
    }
}

impl Drop for ModoCru {
    fn drop(&mut self) {
        let _ = sys::tcsetattr(self.fd.as_fd(), libc::TCSANOW, &self.original);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn modo_cru_liga_e_volta_e_tamanho_do_pty() {
        let (mestre, escravo) = sys::open_pty().unwrap();
        sys::set_window_size(mestre.as_fd(), 33, 101).unwrap();
        assert!(e_terminal(&escravo));
        assert!(!e_terminal(File::open("/dev/null").unwrap()));
        assert_eq!(tamanho(&escravo), Some((33, 101)));
        assert_eq!(tamanho(File::open("/dev/null").unwrap()), None);
        let antes = sys::tcgetattr(escravo.as_fd()).unwrap();
        assert_ne!(antes.c_lflag & libc::ECHO, 0);
        {
            let _cru = ModoCru::ligar(&escravo).unwrap();
            let agora = sys::tcgetattr(escravo.as_fd()).unwrap();
            assert_eq!(agora.c_lflag & (libc::ECHO | libc::ICANON | libc::ISIG), 0);
        }
        let depois = sys::tcgetattr(escravo.as_fd()).unwrap();
        assert_eq!(depois.c_lflag, antes.c_lflag);
        assert_eq!(depois.c_iflag, antes.c_iflag);
    }

    fn linha(bytes: &[u8], max: usize) -> Result<Vec<u8>, ErroTerminal> {
        ler_linha(&mut &bytes[..], max).map(|s| s.to_vec())
    }

    #[test]
    fn edicao_da_linha() {
        assert_eq!(linha(b"abc\n", 64).unwrap(), b"abc");
        assert_eq!(linha(b"abc\r", 64).unwrap(), b"abc");
        assert_eq!(linha(b"abx\x7fc\n", 64).unwrap(), b"abc");
        assert_eq!(linha(b"\x7f\x08ab\x08c\n", 64).unwrap(), b"ac");
        assert_eq!(linha(b"errada\x15certa\n", 64).unwrap(), b"certa");
        assert_eq!(linha(b"a\x1bb\x01c\n", 64).unwrap(), b"abc");
        assert_eq!(linha("ção\n".as_bytes(), 64).unwrap(), "ção".as_bytes());
        assert_eq!(linha(b"\n", 64).unwrap(), b"");
        assert_eq!(linha(b"ab\x04c\n", 64).unwrap(), b"abc");
    }

    #[test]
    fn cancelar_e_limite() {
        assert_eq!(linha(b"abc\x03def\n", 64), Err(ErroTerminal::Cancelado));
        assert_eq!(linha(b"\x04", 64), Err(ErroTerminal::Cancelado));
        assert_eq!(linha(b"abc", 64), Err(ErroTerminal::Cancelado));
        assert_eq!(linha(b"abcd\n", 4).unwrap(), b"abcd");
        assert_eq!(linha(b"abcde\n", 4), Err(ErroTerminal::Longa));
    }

    #[test]
    fn capacidade_reservada_sem_realocar() {
        let s = ler_linha(&mut &b"0123456789\n"[..], 16).unwrap();
        assert_eq!(s.capacity(), 16);
    }

    #[test]
    fn arquivo_comum_nao_e_terminal() {
        let f = File::open("/dev/null").unwrap();
        assert_eq!(
            ler_senha_em(&f, "senha: ", 64).unwrap_err(),
            ErroTerminal::SemTerminal
        );
    }

    /// Ataque: alguém olhando a tela (ou o que o terminal devolve) vê a senha? Num pty
    /// de verdade, o mestre só recebe o prompt e a quebra de linha; a senha digitada
    /// não ecoa, a entrada pendente antes do prompt é descartada e o terminal volta com
    /// o eco ligado.
    #[test]
    fn ataque_senha_nao_ecoa_no_pty() {
        let (mestre, escravo) = sys::open_pty().unwrap();
        let mestre = File::from(mestre);
        let escravo = File::from(escravo);
        let antes = sys::tcgetattr(escravo.as_fd()).unwrap();
        assert_ne!(antes.c_lflag & libc::ECHO, 0, "o pty novo começa com eco");

        // Digitado antes do prompt: é descartado pelo TCSAFLUSH e não vira senha.
        (&mestre).write_all(b"antes\n").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));

        let leitor = std::thread::spawn(move || {
            let r = ler_senha_em(&escravo, "senha mestra: ", 64).map(|s| s.to_vec());
            let depois = sys::tcgetattr(escravo.as_fd()).unwrap();
            (r, depois.c_lflag)
        });
        // Espera o prompt antes de digitar.
        let mut visto = Vec::new();
        let mut buf = [0u8; 256];
        while !visto.ends_with(b"senha mestra: ") {
            let n = (&mestre).read(&mut buf).unwrap();
            assert!(n > 0);
            visto.extend_from_slice(&buf[..n]);
        }
        (&mestre).write_all(b"falso-eco-01\r").unwrap();
        let (r, lflag) = leitor.join().unwrap();
        assert_eq!(r.unwrap(), b"falso-eco-01");
        assert_ne!(lflag & libc::ECHO, 0, "o eco volta ao sair");
        assert_ne!(lflag & libc::ICANON, 0);
        assert_ne!(lflag & libc::ISIG, 0);

        // O que apareceu na "tela": só o prompt e a quebra de linha.
        let mut resto = Vec::new();
        let _ = sys::tcsetattr(mestre.as_fd(), libc::TCSANOW, &{
            let mut t = sys::tcgetattr(mestre.as_fd()).unwrap();
            t.c_cc[libc::VMIN] = 0;
            t.c_cc[libc::VTIME] = 2;
            t
        });
        loop {
            match (&mestre).read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => resto.extend_from_slice(&buf[..n]),
            }
        }
        visto.extend_from_slice(&resto);
        let tela = String::from_utf8_lossy(&visto);
        assert!(!tela.contains("falso-eco-01"), "a senha ecoou: {tela:?}");
        // "antes" ecoou quando foi digitado, com o eco ainda ligado; o que importa é que
        // não virou senha (conferido acima).
        assert!(tela.contains("senha mestra: "));
    }

    #[test]
    fn ctrl_c_cancela_e_restaura() {
        let (mestre, escravo) = sys::open_pty().unwrap();
        let mestre = File::from(mestre);
        let escravo = File::from(escravo);
        let leitor = std::thread::spawn(move || {
            let r = ler_senha_em(&escravo, "senha: ", 64).map(|s| s.to_vec());
            (r, sys::tcgetattr(escravo.as_fd()).unwrap().c_lflag)
        });
        let mut buf = [0u8; 64];
        let mut visto = Vec::new();
        while !visto.ends_with(b"senha: ") {
            let n = (&mestre).read(&mut buf).unwrap();
            visto.extend_from_slice(&buf[..n]);
        }
        (&mestre).write_all(b"meia\x03").unwrap();
        let (r, lflag) = leitor.join().unwrap();
        assert_eq!(r, Err(ErroTerminal::Cancelado));
        assert_ne!(lflag & libc::ECHO, 0);
    }
}
