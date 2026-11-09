//! Conferência do servidor pelo cliente (S5, D46): a CLI e o app só mandam algum byte,
//! e em especial a senha mestra, depois de conferir que o outro lado do socket é o
//! socket do systemd do sistema:
//! - toda pasta do caminho (`/run/ward/<uid>/` e acima) é do root, sem escrita para
//!   grupo e outros, sem link simbólico;
//! - o socket é do próprio uid e 0600;
//! - o `SO_PEERCRED` do servidor tem uid 0. Visto pelo cliente, ele é a credencial de
//!   quem fez o `listen`, que foi o systemd do sistema; nenhum processo do usuário cria
//!   um socket que escuta com uid 0;
//! - o endereço do par é o caminho esperado.
//!
//! O daemon de teste (D7) escuta numa pasta 0700 do usuário, com o uid do próprio
//! usuário. Essa forma ([`ServidorEsperado::teste`]) só existe com a feature
//! `modo-teste`: sem ela, o binário não tem caminho nenhum que aceite um servidor que
//! não seja do root.

use std::fmt;
use std::io;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use crate::socket::{RecusaCaminho, caminho_socket_sistema, conferir_caminho_do_sistema};
use crate::sys;

/// Que servidor o cliente aceita.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServidorEsperado {
    caminho: PathBuf,
    forma: Forma,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Forma {
    /// Socket do systemd do sistema: pasta do root e par com uid 0.
    Sistema,
    /// Daemon de teste: pasta 0700 do usuário e par do mesmo uid.
    #[cfg(feature = "modo-teste")]
    Teste,
}

impl ServidorEsperado {
    /// O `wardd` instalado: `/run/ward/<euid>/wardd.sock`.
    pub fn sistema() -> ServidorEsperado {
        ServidorEsperado::sistema_em(caminho_socket_sistema())
    }

    /// Mesmas regras de [`sistema`](Self::sistema), noutro caminho (testes, contêiner).
    /// As conferências não mudam: pasta do root e par com uid 0.
    pub fn sistema_em(caminho: PathBuf) -> ServidorEsperado {
        ServidorEsperado {
            caminho,
            forma: Forma::Sistema,
        }
    }

    /// Daemon de teste (D7): `/run/user/<euid>/ward-teste/wardd.sock`.
    #[cfg(feature = "modo-teste")]
    pub fn teste() -> ServidorEsperado {
        ServidorEsperado::teste_em(
            crate::socket::diretorio_runtime()
                .join(crate::socket::NOME_DIRETORIO_TESTE)
                .join(crate::socket::NOME_SOCKET),
        )
    }

    /// Daemon de teste noutro caminho.
    #[cfg(feature = "modo-teste")]
    pub fn teste_em(caminho: PathBuf) -> ServidorEsperado {
        ServidorEsperado {
            caminho,
            forma: Forma::Teste,
        }
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    fn conferir_caminho(&self) -> Result<(), RecusaServidor> {
        if !crate::processo::caminho_absoluto_simples(&self.caminho) {
            return Err(RecusaServidor::CaminhoInvalido);
        }
        let r = match self.forma {
            Forma::Sistema => conferir_caminho_do_sistema(&self.caminho),
            #[cfg(feature = "modo-teste")]
            Forma::Teste => crate::socket::conferir_caminho_de_teste(&self.caminho),
        };
        r.map_err(|e| match e {
            RecusaCaminho::Ausente => RecusaServidor::Ausente,
            RecusaCaminho::Pasta(_) => RecusaServidor::PastaNaoConfiavel,
            RecusaCaminho::Socket => RecusaServidor::SocketNaoConfiavel,
        })
    }

    fn uid_do_par_aceito(&self, uid: u32) -> bool {
        match self.forma {
            Forma::Sistema => uid == 0,
            #[cfg(feature = "modo-teste")]
            Forma::Teste => uid == sys::geteuid(),
        }
    }
}

/// Por que o cliente não fala com o servidor. Sem caminho nem conteúdo: pode ir para o
/// log e para a mensagem ao usuário.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecusaServidor {
    /// Não há socket (ou ninguém escutando): daemon indisponível.
    Ausente,
    /// Caminho esperado não absoluto ou com `.`/`..`.
    CaminhoInvalido,
    /// Uma pasta do caminho não é do root (ou não é a pasta de teste), aceita escrita
    /// de grupo/outros ou é link.
    PastaNaoConfiavel,
    /// O arquivo do socket não é socket, não é do próprio uid ou não é 0600.
    SocketNaoConfiavel,
    /// O par do socket não tem o uid esperado (0 no sistema).
    ParNaoConfiavel { uid: u32 },
    /// O par não está escutando no caminho esperado.
    EnderecoDiferente,
    /// Outro erro ao conectar ou ler as credenciais.
    Io(io::ErrorKind),
}

impl fmt::Display for RecusaServidor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecusaServidor::Ausente => f.write_str("o daemon não está rodando"),
            RecusaServidor::CaminhoInvalido => f.write_str("caminho do socket inválido"),
            RecusaServidor::PastaNaoConfiavel => {
                f.write_str("a pasta do socket não é do sistema; o servidor não é confiável")
            }
            RecusaServidor::SocketNaoConfiavel => {
                f.write_str("o socket não é 0600 deste usuário; o servidor não é confiável")
            }
            RecusaServidor::ParNaoConfiavel { uid } => write!(
                f,
                "o socket foi criado pelo uid {uid}, não pelo sistema; o servidor não é confiável"
            ),
            RecusaServidor::EnderecoDiferente => {
                f.write_str("o servidor não escuta no caminho esperado")
            }
            RecusaServidor::Io(k) => write!(f, "erro ao falar com o daemon: {k}"),
        }
    }
}

impl std::error::Error for RecusaServidor {}

/// Confere o caminho, conecta e confere o par; só depois disso o socket volta para
/// quem chama. Nenhum byte é escrito aqui.
pub fn conectar_servidor(esperado: &ServidorEsperado) -> Result<UnixStream, RecusaServidor> {
    esperado.conferir_caminho()?;
    let stream = UnixStream::connect(&esperado.caminho).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused => RecusaServidor::Ausente,
        k => RecusaServidor::Io(k),
    })?;
    conferir_servidor(&stream, esperado)?;
    Ok(stream)
}

/// Confere um socket já conectado: caminho (pastas e socket), endereço do par e uid do
/// par (`SO_PEERCRED`). Erro: quem chama fecha o socket sem escrever nada.
pub fn conferir_servidor(
    stream: &UnixStream,
    esperado: &ServidorEsperado,
) -> Result<(), RecusaServidor> {
    esperado.conferir_caminho()?;
    let endereco = stream
        .peer_addr()
        .map_err(|e| RecusaServidor::Io(e.kind()))?;
    if endereco.as_pathname() != Some(esperado.caminho.as_path()) {
        return Err(RecusaServidor::EnderecoDiferente);
    }
    let cred = sys::peercred(stream.as_fd()).map_err(|e| RecusaServidor::Io(e.kind()))?;
    if !esperado.uid_do_par_aceito(cred.uid) {
        return Err(RecusaServidor::ParNaoConfiavel { uid: cred.uid });
    }
    // O caminho de novo: a conferência inteira vale para o mesmo estado do disco.
    esperado.conferir_caminho()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn sistema_so_aceita_uid_0() {
        let s = ServidorEsperado::sistema();
        assert!(s.uid_do_par_aceito(0));
        assert!(!s.uid_do_par_aceito(sys::geteuid().max(1)));
        assert!(!s.uid_do_par_aceito(1000));
        assert!(!s.uid_do_par_aceito(u32::MAX));
        assert_eq!(
            s.caminho(),
            Path::new(&format!("/run/ward/{}/wardd.sock", sys::geteuid()))
        );
    }

    #[cfg(feature = "modo-teste")]
    #[test]
    fn teste_aceita_o_proprio_uid_e_nao_o_root() {
        let t = ServidorEsperado::teste();
        assert!(t.uid_do_par_aceito(sys::geteuid()));
        if sys::geteuid() != 0 {
            assert!(!t.uid_do_par_aceito(0));
        }
        assert_eq!(
            t.caminho(),
            Path::new(&format!(
                "/run/user/{}/ward-teste/wardd.sock",
                sys::geteuid()
            ))
        );
    }

    #[test]
    fn caminho_invalido_recusa() {
        for ruim in ["", "relativo", "/run/ward/../x/wardd.sock", "/run//ward"] {
            assert!(
                matches!(
                    conectar_servidor(&ServidorEsperado::sistema_em(PathBuf::from(ruim))),
                    Err(RecusaServidor::CaminhoInvalido)
                ),
                "{ruim}"
            );
        }
    }

    #[test]
    fn mensagens_sem_caminho() {
        for r in [
            RecusaServidor::Ausente,
            RecusaServidor::PastaNaoConfiavel,
            RecusaServidor::SocketNaoConfiavel,
            RecusaServidor::ParNaoConfiavel { uid: 1000 },
            RecusaServidor::EnderecoDiferente,
        ] {
            assert!(!r.to_string().contains('/'), "{r}");
        }
    }
}
