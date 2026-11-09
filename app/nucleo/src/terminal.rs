//! "Conectar" da entrada `servidor` (D3, V5.24, Fase 34).
//!
//! O app pede ao daemon um token de uso único (`emitir_token`: 30 s, uma conexão, preso
//! à entrada) e abre o terminal padrão rodando `ward ssh --token <t> <entrada>`. O token
//! não é segredo de credencial (D3) e morre no primeiro uso; mesmo assim ele só vai ao
//! argv do terminal, nunca à interface nem a log.
//!
//! O [`Terminal`] real ([`TerminalDoSistema`]) chama `/usr/bin/x-terminal-emulator -e`
//! (o terminal padrão do Debian/Ubuntu; a política do Debian exige o `-e <comando>
//! <argumentos>`) com o `ward` instalado em `/usr/local/bin` e um ambiente montado do
//! zero com só o que o terminal precisa para abrir na sessão. O app já largou o grupo
//! `ward-app` antes do GTK (D47), então o terminal não herda o papel app.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use zeroize::Zeroizing;

use crate::ErroApp;

/// Token do "Conectar", já conferido ([`Token::de_resposta`]).
pub struct Token(Zeroizing<String>);

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(••••)")
    }
}

/// Menor e maior token aceitos da resposta do daemon.
const MIN_TOKEN: usize = 16;
const MAX_TOKEN: usize = 64;

impl Token {
    /// A resposta do daemon não é confiável (S1): o token vai para o argv de outro
    /// programa, então só `[A-Za-z0-9_-]`, de 16 a 64 bytes, sem começar por `-` (não
    /// vira opção).
    pub fn de_resposta(bytes: &[u8]) -> Option<Token> {
        let ok = (MIN_TOKEN..=MAX_TOKEN).contains(&bytes.len())
            && bytes[0] != b'-'
            && bytes
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'));
        ok.then(|| Token(Zeroizing::new(String::from_utf8_lossy(bytes).into_owned())))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn expor(&self) -> &str {
        &self.0
    }
}

/// Nome de entrada que pode ir ao argv: o formato do `ward-core` (começa por letra
/// minúscula ou dígito; depois `[a-z0-9._-]`), até `MAX_NOME`.
pub fn entrada_para_argv(nome: &str) -> bool {
    let mut b = nome.bytes();
    nome.len() <= ward_core::limites::MAX_NOME
        && b.next()
            .is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit())
        && b.all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
        })
}

/// Quem abre o terminal.
pub trait Terminal: Send + Sync {
    fn abrir(&self, entrada: &str, token: &Token) -> Result<(), ErroApp>;
}

/// Sem terminal (o padrão do serviço até alguém montar um).
pub struct SemTerminal;

impl Terminal for SemTerminal {
    fn abrir(&self, _entrada: &str, _token: &Token) -> Result<(), ErroApp> {
        Err(ErroApp::TerminalIndisponivel)
    }
}

/// Variáveis que o terminal recebe do ambiente do app (que já é o de D57).
const AMBIENTE_DO_TERMINAL: &[&str] = &[
    "HOME",
    "LANG",
    "XDG_RUNTIME_DIR",
    "WAYLAND_DISPLAY",
    "DBUS_SESSION_BUS_ADDRESS",
    "XDG_CURRENT_DESKTOP",
    "XDG_SESSION_TYPE",
];

/// O terminal padrão do sistema.
pub struct TerminalDoSistema {
    emulador: PathBuf,
    ward: PathBuf,
}

impl Default for TerminalDoSistema {
    fn default() -> Self {
        TerminalDoSistema {
            emulador: PathBuf::from("/usr/bin/x-terminal-emulator"),
            ward: PathBuf::from("/usr/local/bin/ward"),
        }
    }
}

impl TerminalDoSistema {
    /// O comando, sem rodar (os testes conferem argv e ambiente).
    pub fn comando(&self, entrada: &str, token: &Token) -> Result<Command, ErroApp> {
        if !entrada_para_argv(entrada) {
            return Err(ErroApp::PedidoInvalido);
        }
        let mut c = Command::new(&self.emulador);
        c.env_clear()
            .env("PATH", "/usr/local/bin:/usr/bin:/bin")
            .current_dir(Path::new("/"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .arg("-e")
            .arg(&self.ward)
            .args(["ssh", "--token", token.expor(), entrada]);
        for k in AMBIENTE_DO_TERMINAL {
            if let Some(v) = std::env::var_os(k) {
                c.env(k, v);
            }
        }
        Ok(c)
    }
}

impl Terminal for TerminalDoSistema {
    fn abrir(&self, entrada: &str, token: &Token) -> Result<(), ErroApp> {
        let mut c = self.comando(entrada, token)?;
        let mut filho = c.spawn().map_err(|_| ErroApp::TerminalIndisponivel)?;
        // Espera o terminal numa thread para não deixar processo zumbi.
        std::thread::spawn(move || {
            let _ = filho.wait();
        });
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ataque_token_forjado_pelo_daemon_e_recusado() {
        for t in [
            &b""[..],
            b"curto",
            b"--help-aaaaaaaaaaaaaaaa",
            b"-aaaaaaaaaaaaaaaaaaaaaaaa",
            b"aaaaaaaaaaaaaaaa bbbbbbbb",
            b"aaaaaaaaaaaaaaaa;rm -rf /",
            b"aaaaaaaaaaaaaaaa\nbbbbbbbb",
            b"aaaaaaaaaaaaaaaa\xff",
            &[b'a'; 65],
        ] {
            assert!(Token::de_resposta(t).is_none(), "{t:?}");
        }
        let ok = Token::de_resposta(b"Ab3_-xyzXYZ01234").unwrap();
        assert_eq!(ok.len(), 16);
        assert!(!format!("{ok:?}").contains("Ab3"));
    }

    #[test]
    fn ataque_entrada_que_viraria_opcao_nao_vai_ao_argv() {
        for e in ["", "-o", "--token", "Acme", "a b", "a;b", "a\nb", "á"] {
            assert!(!entrada_para_argv(e), "{e:?}");
        }
        assert!(entrada_para_argv("acme-ssh-salto"));
        let t = Token::de_resposta(b"aaaaaaaaaaaaaaaaaaaa").unwrap();
        assert_eq!(
            TerminalDoSistema::default()
                .comando("-oProxyCommand=x", &t)
                .err(),
            Some(ErroApp::PedidoInvalido)
        );
    }

    #[test]
    fn comando_do_terminal_com_ambiente_do_zero() {
        let t = Token::de_resposta(b"tokendeteste0123456789").unwrap();
        let c = TerminalDoSistema::default()
            .comando("acme-ssh-salto", &t)
            .unwrap();
        assert_eq!(c.get_program(), "/usr/bin/x-terminal-emulator");
        let args: Vec<_> = c.get_args().map(|a| a.to_str().unwrap()).collect();
        assert_eq!(
            args,
            [
                "-e",
                "/usr/local/bin/ward",
                "ssh",
                "--token",
                "tokendeteste0123456789",
                "acme-ssh-salto"
            ]
        );
        let envs: Vec<_> = c
            .get_envs()
            .filter_map(|(k, v)| v.map(|_| k.to_str().unwrap().to_owned()))
            .collect();
        for k in &envs {
            assert!(
                k == "PATH" || AMBIENTE_DO_TERMINAL.contains(&k.as_str()),
                "{k}"
            );
        }
        assert_eq!(c.get_current_dir(), Some(Path::new("/")));
    }
}
