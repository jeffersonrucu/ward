//! "Carregar de arquivo" do campo "Chave privada" (Fase 53a; S2, S11, S12).
//!
//! A chave privada vai do arquivo escolhido para o daemon sem passar pela área de
//! transferência (o `wl-copy` grava a entrada em `/tmp`, S2) nem pela webview (S11). A
//! interface só pede `carregar_chave { campo }`; quem escolhe o arquivo é o mesmo
//! [`Seletor`](crate::anexos::Seletor) dos anexos (no app, o diálogo do GTK; na ponte de
//! teste, a pasta fixa da linha de comando). O arquivo é lido aqui, conferido e guardado
//! num [`Segredo`] (zera ao sair) até salvar, cancelar ou sair do formulário; o salvar o
//! põe no rascunho (`"carregada"`). A resposta traz só o tamanho, ou um aviso fixo.
//!
//! Recusas (nenhuma guarda nada nem diz o caminho ou o erro do sistema, S13):
//! - link simbólico no último componente (`O_NOFOLLOW`): o usuário escolhe o arquivo da
//!   chave, não um atalho que pode apontar para outro lugar;
//! - o que não é arquivo comum (pasta, FIFO, dispositivo, socket); `O_NONBLOCK` faz uma
//!   FIFO não prender o app na abertura;
//! - acima de `MAX_SEGREDO` (64 KiB, o limite do `ward-core` para um segredo de texto),
//!   pelo `fstat` e de novo pelo que foi lido;
//! - vazio;
//! - o que não é chave privada OpenSSH ou PEM: só ASCII imprimível, uma linha
//!   `-----BEGIN <rótulo>-----` com um rótulo de chave privada, o miolo em base64 (e os
//!   cabeçalhos `Nome: valor` do PEM antigo cifrado) e a linha `-----END` do mesmo
//!   rótulo. Chave pública, `.ppk` do PuTTY, certificado e texto qualquer são recusados.

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use serde::Serialize;
use ward_core::limites;

use crate::segredo::Segredo;

/// Campos do formulário que aceitam "Carregar de arquivo".
pub const CAMPOS_CARREGAVEIS: &[&str] = &["chave_privada"];

/// Maior arquivo aceito: o limite de um segredo de texto do `ward-core`.
pub const MAX_CHAVE: usize = limites::MAX_SEGREDO;

/// Rótulos PEM/OpenSSH de chave privada.
const ROTULOS: &[&str] = &[
    "OPENSSH PRIVATE KEY",
    "RSA PRIVATE KEY",
    "EC PRIVATE KEY",
    "DSA PRIVATE KEY",
    "PRIVATE KEY",
    "ENCRYPTED PRIVATE KEY",
];

pub const AVISO_LINK: &str = "o arquivo é um link simbólico; escolha o próprio arquivo da chave";
pub const AVISO_NAO_COMUM: &str = "o item escolhido não é um arquivo comum";
pub const AVISO_VAZIO: &str = "arquivo vazio: não há chave para carregar";
pub const AVISO_LEITURA: &str = "não foi possível ler o arquivo";
pub const AVISO_NAO_E_CHAVE: &str = "o arquivo não é uma chave privada OpenSSH ou PEM";

pub fn aviso_grande() -> String {
    format!(
        "o arquivo passa de {} KiB e não é uma chave privada",
        MAX_CHAVE / limites::KIB
    )
}

/// O que a interface recebe do `carregar_chave`. Nunca o conteúdo nem o caminho.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "resultado", rename_all = "kebab-case")]
pub enum Carregamento {
    Carregada {
        tamanho: u64,
    },
    Cancelado,
    /// Nada foi guardado; o motivo é um texto fixo.
    Aviso {
        motivo: String,
    },
}

/// Lê e confere o arquivo da chave. `Err` traz o aviso fixo.
pub fn ler_chave(caminho: &Path) -> Result<Segredo, String> {
    let arquivo = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_NOCTTY)
        .open(caminho)
        .map_err(|e| match e.raw_os_error() {
            Some(libc::ELOOP) => AVISO_LINK.to_owned(),
            _ => AVISO_LEITURA.to_owned(),
        })?;
    let meta = arquivo.metadata().map_err(|_| AVISO_LEITURA.to_owned())?;
    if !meta.is_file() {
        return Err(AVISO_NAO_COMUM.to_owned());
    }
    if meta.len() > MAX_CHAVE as u64 {
        return Err(aviso_grande());
    }
    let mut conteudo = ler_ate(arquivo)?;
    if conteudo.is_empty() {
        return Err(AVISO_VAZIO.to_owned());
    }
    if !e_chave_privada(&conteudo) {
        return Err(AVISO_NAO_E_CHAVE.to_owned());
    }
    Ok(Segredo::novo(core::mem::take(&mut *conteudo)))
}

type Lido = zeroize::Zeroizing<Vec<u8>>;

/// Lê no máximo `MAX_CHAVE` bytes (o arquivo pode crescer entre o `fstat` e a leitura)
/// num buffer fixo de `MAX_CHAVE + 1`, alocado uma vez e zerado ao sair (Fase 53d): a
/// leitura nunca realoca, então nenhum pedaço da chave fica num bloco antigo do heap sem
/// zerar. O `truncate` só muda o tamanho; a capacidade (e o bloco) continuam os mesmos e
/// o `Segredo` toma posse dele sem cópia.
fn ler_ate(mut arquivo: File) -> Result<Lido, String> {
    let mut buf = zeroize::Zeroizing::new(vec![0u8; MAX_CHAVE + 1]);
    let mut lidos = 0usize;
    while lidos < buf.len() {
        match arquivo.read(&mut buf[lidos..]) {
            Ok(0) => break,
            Ok(n) => lidos += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err(AVISO_LEITURA.to_owned()),
        }
    }
    if lidos > MAX_CHAVE {
        return Err(aviso_grande());
    }
    buf.truncate(lidos);
    Ok(buf)
}

fn base64(linha: &str) -> bool {
    !linha.is_empty()
        && linha
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'/' | b'='))
}

/// Cabeçalho do PEM antigo cifrado (`Proc-Type: 4,ENCRYPTED`, `DEK-Info: …`).
fn cabecalho_pem(linha: &str) -> bool {
    linha.split_once(": ").is_some_and(|(nome, valor)| {
        !nome.is_empty()
            && nome.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
            && !valor.is_empty()
    })
}

/// Chave privada OpenSSH ou PEM, pela forma (o conteúdo da chave não é interpretado).
pub fn e_chave_privada(b: &[u8]) -> bool {
    if !b
        .iter()
        .all(|c| matches!(c, b'\n' | b'\r') || (0x20..0x7f).contains(c))
    {
        return false;
    }
    let Ok(texto) = core::str::from_utf8(b) else {
        return false;
    };
    let mut linhas = texto.lines().map(str::trim_end).filter(|l| !l.is_empty());
    let Some(primeira) = linhas.next() else {
        return false;
    };
    let Some(rotulo) = primeira
        .strip_prefix("-----BEGIN ")
        .and_then(|r| r.strip_suffix("-----"))
        .filter(|r| ROTULOS.contains(r))
    else {
        return false;
    };
    let fim = format!("-----END {rotulo}-----");
    let mut miolo = 0usize;
    let mut cabecalhos = true;
    let mut fechou = false;
    for linha in linhas {
        if fechou {
            // Nada depois do END (outra chave, certificado, texto).
            return false;
        }
        if linha == fim {
            fechou = true;
        } else if base64(linha) {
            miolo += 1;
            cabecalhos = false;
        } else if cabecalhos && miolo == 0 && cabecalho_pem(linha) {
            // Só antes do base64.
        } else {
            return false;
        }
    }
    fechou && miolo > 0
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;

    fn pasta(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ward-chave-{}-{nome}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    /// Chave falsa na forma OpenSSH, montada em pedaços (o texto inteiro de uma chave
    /// não fica no repositório).
    pub(crate) fn chave_falsa(rotulo: &str, miolo: &str) -> String {
        let (a, b) = ("-----BEGIN ", "-----END ");
        format!("{a}{rotulo}-----\n{miolo}\n{b}{rotulo}-----\n")
    }

    fn escrever(dir: &Path, nome: &str, conteudo: &[u8]) -> PathBuf {
        let p = dir.join(nome);
        let mut f = File::create(&p).unwrap();
        f.write_all(conteudo).unwrap();
        p
    }

    #[test]
    fn aceita_openssh_e_pem() {
        for r in ROTULOS {
            let k = chave_falsa(r, "QUFBQUFBQUFBQUFB\nZmFsc28tbWlvbG8=");
            assert!(e_chave_privada(k.as_bytes()), "{r}");
        }
        // CRLF e linhas em branco nas pontas.
        let k = chave_falsa("OPENSSH PRIVATE KEY", "ZmFsc28=").replace('\n', "\r\n");
        assert!(e_chave_privada(format!("\n{k}\n\n").as_bytes()));
        // PEM antigo cifrado: cabeçalhos antes do base64.
        let k = chave_falsa(
            "RSA PRIVATE KEY",
            "Proc-Type: 4,ENCRYPTED\nDEK-Info: AES-128-CBC,00FF\n\nZmFsc28=",
        );
        assert!(e_chave_privada(k.as_bytes()));
    }

    #[test]
    fn ataque_o_que_nao_e_chave_privada_e_recusado() {
        let pub_ = [
            "ssh-ed25519 ",
            "AAAAC3NzaC1lZDI1NTE5AAAAIfalso falso@host\n",
        ]
        .concat();
        let casos: Vec<String> = vec![
            pub_,
            chave_falsa("PUBLIC KEY", "ZmFsc28="),
            chave_falsa("CERTIFICATE", "ZmFsc28="),
            chave_falsa("OPENSSH PRIVATE KEY", ""),
            "PuTTY-User-Key-File-3: ssh-ed25519\nEncryption: none\n".into(),
            "falso texto qualquer\n".into(),
            // END de outro rótulo.
            chave_falsa("OPENSSH PRIVATE KEY", "ZmFsc28=")
                .replace("END OPENSSH PRIVATE", "END RSA PRIVATE"),
            // Sem END.
            ["-----BEGIN ", "OPENSSH PRIVATE KEY-----\nZmFsc28=\n"].concat(),
            // Duas chaves, ou texto depois do END.
            chave_falsa("OPENSSH PRIVATE KEY", "ZmFsc28=").repeat(2),
            chave_falsa("OPENSSH PRIVATE KEY", "ZmFsc28=") + "falso\n",
            // Texto antes do BEGIN.
            format!("falso\n{}", chave_falsa("OPENSSH PRIVATE KEY", "ZmFsc28=")),
            // Miolo com caractere fora do base64, controle, não-ASCII, NUL.
            chave_falsa("OPENSSH PRIVATE KEY", "ZmFs c28="),
            chave_falsa("OPENSSH PRIVATE KEY", "ZmFs\u{1b}[2Jc28="),
            chave_falsa("OPENSSH PRIVATE KEY", "ZmFsc28\u{e9}"),
            chave_falsa("OPENSSH PRIVATE KEY", "ZmFs\0c28="),
            // Cabeçalho depois do base64.
            chave_falsa("RSA PRIVATE KEY", "ZmFsc28=\nProc-Type: 4,ENCRYPTED"),
        ];
        for c in casos {
            assert!(!e_chave_privada(c.as_bytes()), "{c:?}");
        }
        assert!(!e_chave_privada(b""));
        assert!(!e_chave_privada(&[0xff, 0xfe]));
    }

    #[test]
    fn le_a_chave_sem_devolver_o_conteudo() {
        let d = pasta("ok");
        let k = chave_falsa("OPENSSH PRIVATE KEY", "QUFBQUFBQUFBQUFB");
        let p = escrever(&d, "deploy_ed25519", k.as_bytes());
        let s = ler_chave(&p).unwrap();
        assert_eq!(s.len(), k.len());
        assert_eq!(s.expor(), k.as_bytes());
        assert!(!format!("{s:?}").contains("QUFB"));
        let resposta = serde_json::to_string(&Carregamento::Carregada {
            tamanho: s.len() as u64,
        })
        .unwrap();
        assert_eq!(
            resposta,
            format!("{{\"resultado\":\"carregada\",\"tamanho\":{}}}", k.len())
        );
        let _ = std::fs::remove_dir_all(d);
    }

    /// Fase 53d: a leitura usa um buffer fixo de `MAX_CHAVE + 1` e não realoca; o
    /// `Segredo` fica com o mesmo bloco (capacidade igual), pequeno, no limite ou acima.
    #[test]
    fn ataque_leitura_nao_realoca_o_buffer_da_chave() {
        let d = pasta("buffer");
        let k = chave_falsa("OPENSSH PRIVATE KEY", "QUFBQUFBQUFBQUFB");
        let p = escrever(&d, "pequena", k.as_bytes());
        let lido = ler_ate(File::open(&p).unwrap()).unwrap();
        assert_eq!(lido.len(), k.len());
        assert_eq!(lido.capacity(), MAX_CHAVE + 1);
        let ptr = lido.as_ptr();
        let s = Segredo::novo({
            let mut l = lido;
            core::mem::take(&mut *l)
        });
        assert_eq!(s.expor().as_ptr(), ptr, "o Segredo fica com o mesmo bloco");

        let no_limite = escrever(&d, "limite", &vec![b'Q'; MAX_CHAVE]);
        let lido = ler_ate(File::open(&no_limite).unwrap()).unwrap();
        assert_eq!((lido.len(), lido.capacity()), (MAX_CHAVE, MAX_CHAVE + 1));

        // Arquivo maior que o limite (o que cresce depois do `fstat` cai aqui): recusado.
        let acima = escrever(&d, "acima", &vec![b'Q'; MAX_CHAVE * 2]);
        assert_eq!(
            ler_ate(File::open(&acima).unwrap()).err(),
            Some(aviso_grande())
        );
        let _ = std::fs::remove_dir_all(d);
    }

    /// S2/S11: link, pasta, FIFO, grande, vazio, inexistente e o que não é chave dão um
    /// aviso fixo, sem caminho nem conteúdo, e nada é guardado.
    #[test]
    fn ataque_link_fifo_grande_vazio_e_texto_sao_recusados() {
        let d = pasta("ataques");
        let k = chave_falsa("OPENSSH PRIVATE KEY", "QUFBQUFBQUFBQUFB");
        let alvo = escrever(&d, "chave", k.as_bytes());
        let link = d.join("link");
        std::os::unix::fs::symlink(&alvo, &link).unwrap();
        assert_eq!(ler_chave(&link).err().as_deref(), Some(AVISO_LINK));
        // Link para fora (o atacante aponta para outro arquivo do usuário).
        let fora = d.join("fora");
        std::os::unix::fs::symlink("/etc/hostname", &fora).unwrap();
        assert_eq!(ler_chave(&fora).err().as_deref(), Some(AVISO_LINK));

        assert_eq!(ler_chave(&d).err().as_deref(), Some(AVISO_NAO_COMUM));
        let fifo = d.join("fifo");
        let ok = std::process::Command::new("/usr/bin/mkfifo")
            .arg(&fifo)
            .status()
            .is_ok_and(|s| s.success());
        if ok {
            // Sem escritor do outro lado: com O_NONBLOCK, a abertura não prende.
            assert_eq!(ler_chave(&fifo).err().as_deref(), Some(AVISO_NAO_COMUM));
        }
        assert_eq!(
            ler_chave(Path::new("/dev/zero")).err().as_deref(),
            Some(AVISO_NAO_COMUM)
        );

        let grande = chave_falsa("OPENSSH PRIVATE KEY", &"QUFB\n".repeat(MAX_CHAVE / 5));
        assert!(grande.len() > MAX_CHAVE);
        let g = escrever(&d, "grande", grande.as_bytes());
        assert_eq!(ler_chave(&g).err(), Some(aviso_grande()));
        let v = escrever(&d, "vazio", b"");
        assert_eq!(ler_chave(&v).err().as_deref(), Some(AVISO_VAZIO));
        let t = escrever(&d, "texto.txt", b"falso-senha-em-texto\n");
        assert_eq!(ler_chave(&t).err().as_deref(), Some(AVISO_NAO_E_CHAVE));
        let p = escrever(
            &d,
            "id.pub",
            ["ssh-ed25519 ", "AAAAC3Nzfalso falso@host\n"]
                .concat()
                .as_bytes(),
        );
        assert_eq!(ler_chave(&p).err().as_deref(), Some(AVISO_NAO_E_CHAVE));
        assert_eq!(
            ler_chave(&d.join("nao-existe")).err().as_deref(),
            Some(AVISO_LEITURA)
        );
        // Sem permissão de leitura.
        let fechado = escrever(&d, "fechado", k.as_bytes());
        std::fs::set_permissions(
            &fechado,
            std::os::unix::fs::PermissionsExt::from_mode(0o000),
        )
        .unwrap();
        if File::open(&fechado).is_err() {
            assert_eq!(ler_chave(&fechado).err().as_deref(), Some(AVISO_LEITURA));
        }
        // Exatamente no limite passa (se tiver a forma de chave).
        let miolo_max = MAX_CHAVE - chave_falsa("OPENSSH PRIVATE KEY", "").len();
        let no_limite = chave_falsa("OPENSSH PRIVATE KEY", &"Q".repeat(miolo_max));
        assert_eq!(no_limite.len(), MAX_CHAVE);
        let n = escrever(&d, "no-limite", no_limite.as_bytes());
        assert_eq!(ler_chave(&n).map(|s| s.len()).ok(), Some(MAX_CHAVE));
        // Nenhum aviso cita caminho.
        for a in [
            AVISO_LINK,
            AVISO_NAO_COMUM,
            AVISO_VAZIO,
            AVISO_LEITURA,
            AVISO_NAO_E_CHAVE,
        ] {
            assert!(!a.contains('/'));
        }
        let _ = std::fs::remove_dir_all(d);
    }
}
