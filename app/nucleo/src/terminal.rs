//! "Conectar" da entrada `servidor` (D3, V5.24, Fase 34).
//!
//! O app pede ao daemon um token de uso único (`emitir_token`: 30 s, uma conexão, preso
//! à entrada) e abre o terminal padrão rodando `ward ssh --token-fd <n> <entrada>`.
//!
//! O token pula o nível e a aprovação, então nenhum outro processo do mesmo uid pode
//! vê-lo (Fase 27a; achado alto da auditoria do M3). Ele **nunca** vai em argv nem em
//! variável de ambiente (`/proc/<pid>/cmdline` e `/proc/<pid>/environ` são legíveis pelo
//! mesmo uid), nem à interface, nem a log. Vai por um socket Unix
//! ([`ward_plataforma::conectar::CanalConectar`]): o app escreve o token e fecha a ponta
//! dele; a outra ponta é o único fd ≥ 3 que o terminal herda, e o terminal a repassa ao
//! `ward ssh`, que lê o token e fecha o fd antes de conectar. O número do fd vai em
//! argv (não é segredo). Um socket não reabre por `/proc/<pid>/fd/<n>`, ao contrário de
//! pipe ou arquivo.
//!
//! Terminais conferidos (o terminal tem de repassar o fd herdado ao comando do `-e`):
//! `cosmic-term` (o `x-terminal-emulator` do COSMIC) e `xterm`. Um terminal que entrega
//! o comando a um servidor já aberto (ex.: `gnome-terminal`) não repassa o fd: o
//! `ward ssh` sai com "o canal do token do Conectar não está aberto" e não conecta
//! (falha fechada).
//!
//! O [`Terminal`] real ([`TerminalDoSistema`]) chama `/usr/bin/x-terminal-emulator -e`
//! (o terminal padrão do Debian/Ubuntu; a política do Debian exige o `-e <comando>
//! <argumentos>`) com o `ward` instalado em `/usr/local/bin` e um ambiente montado do
//! zero com só o que o terminal precisa para abrir na sessão. O app já largou o grupo
//! `ward-app` antes do GTK (D47), então o terminal não herda o papel app.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ward_plataforma::conectar::CanalConectar;
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
    /// A resposta do daemon não é confiável (S1): só `[A-Za-z0-9_-]`, de 16 a 64 bytes,
    /// sem começar por `-` (o formato que o `ward ssh` aceita do canal).
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

/// O comando do terminal e o canal do token. O canal tem de viver até o `spawn`
/// voltar ([`Abertura::abrir`] cuida disso).
pub struct Abertura {
    pub comando: Command,
    canal: CanalConectar,
}

impl Abertura {
    /// Roda o terminal e solta a ponta do canal do app: o terminal fica com a única
    /// cópia, e o token, só na fila do socket.
    pub fn abrir(mut self) -> std::io::Result<std::process::Child> {
        let filho = self.comando.spawn();
        drop(self.canal);
        filho
    }
}

impl TerminalDoSistema {
    /// Para os testes: outro emulador e outro `ward`.
    pub fn com_caminhos(emulador: PathBuf, ward: PathBuf) -> TerminalDoSistema {
        TerminalDoSistema { emulador, ward }
    }

    /// O comando, sem rodar (os testes conferem argv e ambiente), com o canal do token
    /// já escrito. O token não vai em argv nem no ambiente: só o número do fd.
    pub fn comando(&self, entrada: &str, token: &Token) -> Result<Abertura, ErroApp> {
        if !entrada_para_argv(entrada) {
            return Err(ErroApp::PedidoInvalido);
        }
        let canal = CanalConectar::novo(token.expor().as_bytes())
            .map_err(|_| ErroApp::TerminalIndisponivel)?;
        let mut c = Command::new(&self.emulador);
        c.env_clear()
            .env("PATH", "/usr/local/bin:/usr/bin:/bin")
            .current_dir(Path::new("/"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .arg("-e")
            .arg(&self.ward)
            .args(["ssh", "--token-fd", &canal.numero().to_string(), entrada]);
        for k in AMBIENTE_DO_TERMINAL {
            if let Some(v) = std::env::var_os(k) {
                c.env(k, v);
            }
        }
        canal.preparar(&mut c);
        Ok(Abertura { comando: c, canal })
    }
}

impl Terminal for TerminalDoSistema {
    fn abrir(&self, entrada: &str, token: &Token) -> Result<(), ErroApp> {
        let a = self.comando(entrada, token)?;
        let mut filho = a.abrir().map_err(|_| ErroApp::TerminalIndisponivel)?;
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
        let a = TerminalDoSistema::default()
            .comando("acme-ssh-salto", &t)
            .unwrap();
        let c = &a.comando;
        assert_eq!(c.get_program(), "/usr/bin/x-terminal-emulator");
        let args: Vec<_> = c.get_args().map(|a| a.to_str().unwrap()).collect();
        let fd = a.canal.numero().to_string();
        assert_eq!(
            args,
            [
                "-e",
                "/usr/local/bin/ward",
                "ssh",
                "--token-fd",
                fd.as_str(),
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
        // Fase 27a: o token não está em nenhum argumento nem valor de ambiente.
        for v in c
            .get_args()
            .chain(c.get_envs().filter_map(|(_, v)| v))
            .chain([c.get_program()])
        {
            assert!(!v.to_string_lossy().contains("tokendeteste"), "{v:?}");
        }
        assert_eq!(c.get_current_dir(), Some(Path::new("/")));
    }

    /// Fase 27a (achado alto da auditoria do M3): um "Conectar" de verdade, com um
    /// terminal falso (tira o `-e` e roda o resto) e um `ward` falso (lê o fd do
    /// `--token-fd`, grava o que leu e a lista de fds, e espera). Enquanto os dois rodam,
    /// um leitor em laço varre `/proc/*/cmdline` e `/proc/*/environ` de todo processo e
    /// tenta reabrir `/proc/<pid>/fd/*` do terminal e do `ward`: o token não aparece em
    /// lugar nenhum, e o `ward` falso o recebe inteiro pelo canal.
    #[test]
    fn ataque_token_do_conectar_nao_aparece_em_proc() {
        use std::os::unix::fs::PermissionsExt;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

        let base = std::env::temp_dir().join(format!("ward-terminal-27a-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir(&base).unwrap();
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700)).unwrap();
        let emulador = base.join("terminal-falso");
        let ward = base.join("ward-falso");
        let saida = base.join("saida");
        let pronto = base.join("pronto");
        std::fs::write(&emulador, "#!/bin/sh\nshift\nexec \"$@\"\n").unwrap();
        // $1=ssh $2=--token-fd $3=<n> $4=<entrada>. Lê o canal, lista os fds e fica
        // vivo um pouco para o leitor varrer o processo com o fd ainda aberto.
        std::fs::write(
            &ward,
            format!(
                "#!/bin/sh\nls -l /proc/$$/fd > '{d}/fds' 2>&1\nsleep 0.4\n\
                 eval \"cat <&$3\" > '{s}'\n: > '{p}'\nsleep 0.4\n",
                d = base.display(),
                s = saida.display(),
                p = pronto.display()
            ),
        )
        .unwrap();
        for f in [&emulador, &ward] {
            std::fs::set_permissions(f, std::fs::Permissions::from_mode(0o700)).unwrap();
        }

        // Token novo a cada rodada, gerado aqui (nunca escrito no repositório).
        let gerado = ward_core::gerador::token_conectar().unwrap();
        let token = Token::de_resposta(gerado.expor()).unwrap();
        let agulha = gerado.expor().to_vec();

        let parar = Arc::new(AtomicBool::new(false));
        let achados = Arc::new(AtomicUsize::new(0));
        let varridos = Arc::new(AtomicUsize::new(0));
        let fds_tentados = Arc::new(AtomicUsize::new(0));
        let leitor = {
            let (parar, achados, varridos, fds_tentados) = (
                parar.clone(),
                achados.clone(),
                varridos.clone(),
                fds_tentados.clone(),
            );
            let agulha = agulha.clone();
            let ward = ward.clone();
            std::thread::spawn(move || {
                let contem = |b: &[u8]| b.windows(agulha.len()).any(|w| w == agulha);
                while !parar.load(Ordering::Relaxed) {
                    let Ok(dir) = std::fs::read_dir("/proc") else {
                        continue;
                    };
                    for p in dir.flatten() {
                        let nome = p.file_name();
                        if !nome.to_string_lossy().bytes().all(|b| b.is_ascii_digit()) {
                            continue;
                        }
                        for arq in ["cmdline", "environ"] {
                            if let Ok(b) = std::fs::read(p.path().join(arq)) {
                                varridos.fetch_add(1, Ordering::Relaxed);
                                if contem(&b) {
                                    achados.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        }
                        // Os processos do terminal e do `ward` falsos: reabrir cada fd.
                        let cmd = std::fs::read(p.path().join("cmdline")).unwrap_or_default();
                        if contem_caminho(&cmd, &ward)
                            && let Ok(fds) = std::fs::read_dir(p.path().join("fd"))
                        {
                            for f in fds.flatten() {
                                // 0–2 são os arquivos de saída do próprio teste (o
                                // `ward` falso grava neles o que leu); o canal é ≥ 3.
                                if matches!(f.file_name().to_str(), Some("0" | "1" | "2")) {
                                    continue;
                                }
                                fds_tentados.fetch_add(1, Ordering::Relaxed);
                                if let Ok(mut a) = std::fs::File::open(f.path()) {
                                    let mut buf = vec![0u8; 256];
                                    if let Ok(n) = std::io::Read::read(&mut a, &mut buf)
                                        && contem(&buf[..n])
                                    {
                                        achados.fetch_add(1, Ordering::Relaxed);
                                    }
                                }
                            }
                        }
                    }
                }
            })
        };

        let t = TerminalDoSistema::com_caminhos(emulador.clone(), ward.clone());
        let a = t.comando("acme-ssh-salto", &token).unwrap();
        let mut filho = a.abrir().unwrap();
        let st = filho.wait().unwrap();
        parar.store(true, Ordering::Relaxed);
        leitor.join().unwrap();

        assert!(st.success(), "{st:?}");
        assert!(pronto.exists());
        assert_eq!(
            std::fs::read(&saida).unwrap(),
            agulha,
            "o ward não recebeu o token"
        );
        let fds = std::fs::read_to_string(base.join("fds")).unwrap();
        assert!(fds.contains("socket:["), "{fds}");
        assert!(varridos.load(Ordering::Relaxed) > 0);
        assert!(
            fds_tentados.load(Ordering::Relaxed) > 0,
            "o leitor não viu o ward"
        );
        assert_eq!(
            achados.load(Ordering::Relaxed),
            0,
            "token visto por outro processo"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    fn contem_caminho(cmdline: &[u8], caminho: &Path) -> bool {
        let c = caminho.as_os_str().as_encoded_bytes();
        cmdline.split(|b| *b == 0).any(|a| a == c)
    }
}
