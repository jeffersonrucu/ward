//! Arquivos do daemon no disco: o cofre (só leitura e identidade; quem grava é o
//! `ward_core::gravacao`) e o arquivo de estado com as gerações de D39/D53.
//!
//! - Tudo é aberto com `O_NOFOLLOW`: link simbólico no lugar do cofre ou do estado é
//!   recusado. Só arquivo comum do próprio usuário.
//! - A identidade do cofre (`dev`, inode, `mtime` com nanossegundos e tamanho) vem do
//!   `fstat` do mesmo fd que foi lido (D39); depois de gravar, do fd do temporário que
//!   o `ward_core::gravacao` devolve (D62), nunca de um `stat` depois do rename.
//! - O estado é texto pequeno, sem segredo, gravado de forma atômica (temporário com
//!   `O_EXCL` + fsync + rename + fsync do diretório), 0600. Guarda as gerações (D53) e
//!   as senhas erradas seguidas do papel agente (D20 revista pela D62).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
pub use ward_core::gravacao::IdentidadeArquivo as Identidade;

use ward_core::formato;

/// Maior arquivo de cofre que o formato aceita (cabeçalho + conteúdo + tag).
pub(crate) const MAX_ARQUIVO_COFRE: u64 =
    (formato::TAM_CABECALHO + formato::MAX_CONTEUDO + 16) as u64;
/// Maior arquivo de estado aceito.
const MAX_ESTADO: u64 = 256;
/// Primeira linha do arquivo de estado.
const CABECALHO_ESTADO: &str = "ward-estado 1";

fn abrir_leitura(caminho: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(caminho)
}

/// Só arquivo comum do próprio usuário, sem escrita para grupo e outros.
fn conferir_arquivo(m: &fs::Metadata) -> io::Result<()> {
    if !m.file_type().is_file()
        || m.uid() != ward_plataforma::uid_efetivo()
        || m.mode() & 0o022 != 0
    {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(())
}

/// `known_hosts` do Ward (D13; auditoria do M4): a pasta tem de ser do usuário efetivo,
/// diretório de verdade e sem permissão de grupo/outros (0700); o arquivo, se existe,
/// regular, sem link, do usuário e sem permissão de grupo/outros (0600). Se não existe,
/// é criado vazio e 0600 (sem seguir link), para o `ssh` não criá-lo com a `umask`.
/// Qualquer outra coisa: `PermissionDenied`.
pub fn preparar_known_hosts(caminho: &Path) -> io::Result<()> {
    let pasta = caminho
        .parent()
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    let fd = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_CLOEXEC)
        .open(pasta)?;
    let m = fd.metadata()?;
    if !m.is_dir() || m.uid() != ward_plataforma::uid_efetivo() || m.mode() & 0o077 != 0 {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let f = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(caminho)
    {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(caminho)
            .map_err(|_| io::Error::from(io::ErrorKind::PermissionDenied))?,
        Err(e) => return Err(e),
    };
    let m = f.metadata()?;
    if !m.file_type().is_file()
        || m.uid() != ward_plataforma::uid_efetivo()
        || m.mode() & 0o077 != 0
        || m.nlink() != 1
    {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(())
}

/// O cofre existe (qualquer coisa no caminho, inclusive link)?
pub fn cofre_existe(caminho: &Path) -> bool {
    fs::symlink_metadata(caminho).is_ok()
}

/// Lê o arquivo do cofre (cifrado) e a identidade do mesmo fd. `Ok(None)` se não existe.
pub fn ler_cofre(caminho: &Path) -> io::Result<Option<(Vec<u8>, Identidade)>> {
    let f = match abrir_leitura(caminho) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let m = f.metadata()?;
    conferir_arquivo(&m)?;
    if m.len() > MAX_ARQUIVO_COFRE {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut bytes = Vec::new();
    (&f).take(MAX_ARQUIVO_COFRE + 1).read_to_end(&mut bytes)?;
    // O que foi lido tem de ser o arquivo do `fstat` (sem escrita no meio).
    let depois = f.metadata()?;
    if bytes.len() as u64 != depois.len() || Identidade::from(&m) != Identidade::from(&depois) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(Some((bytes, Identidade::from(&m))))
}

/// Identidade atual do cofre no caminho. `Ok(None)` se não existe.
pub fn identidade(caminho: &Path) -> io::Result<Option<Identidade>> {
    match abrir_leitura(caminho) {
        Ok(f) => {
            let m = f.metadata()?;
            conferir_arquivo(&m)?;
            Ok(Some(Identidade::from(&m)))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// As duas gerações do arquivo de estado (D53).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geracoes {
    /// Maior geração confirmada no disco: a `geracao_minima` da abertura.
    pub vista: u64,
    /// Maior geração selada ou tentada. Nunca menor que `vista`.
    pub queimada: u64,
}

/// O arquivo de estado inteiro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArquivoEstado {
    pub geracoes: Geracoes,
    /// Senhas erradas seguidas do papel agente (D20, D62). O formato antigo, sem a
    /// linha, é lido como 0; toda gravação regrava no formato novo.
    pub falhas_agente: u32,
}

/// O arquivo de estado faltou ou não está no formato. O daemon trata os dois como
/// retrocesso (D39): não destrava um cofre existente sem estado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroEstado {
    Ausente,
    Invalido,
}

fn numero(linha: &str, chave: &str) -> Option<u64> {
    let resto = linha.strip_prefix(chave)?.strip_prefix(' ')?;
    // Forma única: só dígitos, sem zero à esquerda.
    if resto.is_empty()
        || !resto.bytes().all(|b| b.is_ascii_digit())
        || (resto.len() > 1 && resto.starts_with('0'))
    {
        return None;
    }
    resto.parse().ok()
}

/// Lê as gerações do arquivo de estado (ver [`ler_estado_completo`]).
pub fn ler_estado(caminho: &Path) -> Result<Geracoes, ErroEstado> {
    ler_estado_completo(caminho).map(|e| e.geracoes)
}

/// Lê o arquivo de estado (estrito: as linhas exatas, `queimada ≥ vista`; a linha das
/// falhas do agente é opcional, para ler o formato antigo como 0).
pub fn ler_estado_completo(caminho: &Path) -> Result<ArquivoEstado, ErroEstado> {
    let f = match abrir_leitura(caminho) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(ErroEstado::Ausente),
        Err(_) => return Err(ErroEstado::Invalido),
    };
    let m = f.metadata().map_err(|_| ErroEstado::Invalido)?;
    if conferir_arquivo(&m).is_err() || m.mode() & 0o077 != 0 || m.len() > MAX_ESTADO {
        return Err(ErroEstado::Invalido);
    }
    let mut texto = String::new();
    (&f).take(MAX_ESTADO + 1)
        .read_to_string(&mut texto)
        .map_err(|_| ErroEstado::Invalido)?;
    analisar_estado(&texto).ok_or(ErroEstado::Invalido)
}

fn analisar_estado(texto: &str) -> Option<ArquivoEstado> {
    let corpo = texto.strip_suffix('\n')?;
    let mut linhas = corpo.split('\n');
    if linhas.next()? != CABECALHO_ESTADO {
        return None;
    }
    let vista = numero(linhas.next()?, "vista")?;
    let queimada = numero(linhas.next()?, "queimada")?;
    // Formato antigo (Fase 22): sem a linha, 0 (D62).
    let falhas_agente = match linhas.next() {
        None => 0,
        Some(l) => u32::try_from(numero(l, "falhas_agente")?).ok()?,
    };
    if linhas.next().is_some() || queimada < vista {
        return None;
    }
    Some(ArquivoEstado {
        geracoes: Geracoes { vista, queimada },
        falhas_agente,
    })
}

fn texto_estado(e: ArquivoEstado) -> String {
    format!(
        "{CABECALHO_ESTADO}\nvista {}\nqueimada {}\nfalhas_agente {}\n",
        e.geracoes.vista, e.geracoes.queimada, e.falhas_agente
    )
}

static CONTADOR: AtomicU64 = AtomicU64::new(0);

/// Grava o estado de forma atômica (temporário `O_EXCL` 0600 no mesmo diretório, fsync,
/// rename, fsync do diretório), com as falhas do agente em 0: é o que vale com o cofre
/// aberto ou abrindo (o destravar certo zera, D62). Em erro, o temporário é apagado e o
/// estado anterior fica.
pub fn gravar_estado(caminho: &Path, g: Geracoes) -> io::Result<()> {
    gravar_estado_completo(
        caminho,
        ArquivoEstado {
            geracoes: g,
            falhas_agente: 0,
        },
    )
}

/// Como [`gravar_estado`], com as falhas do agente dadas. Quando volta `Ok`, o arquivo
/// e o diretório já passaram pelo fsync (D62: antes da resposta de erro).
pub fn gravar_estado_completo(caminho: &Path, e: ArquivoEstado) -> io::Result<()> {
    let g = e.geracoes;
    if g.queimada < g.vista {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let dir = caminho
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .ok_or(io::ErrorKind::InvalidInput)?;
    let nome = caminho.file_name().ok_or(io::ErrorKind::InvalidInput)?;
    let n = CONTADOR.fetch_add(1, Ordering::Relaxed);
    let mut tmp_nome = std::ffi::OsString::from(".");
    tmp_nome.push(nome);
    tmp_nome.push(format!(".tmp-{}-{n}", std::process::id()));
    let tmp: PathBuf = dir.join(tmp_nome);
    let r = (|| {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&tmp)?;
        f.write_all(texto_estado(e).as_bytes())?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, caminho)?;
        File::open(dir)?.sync_all()
    })();
    if r.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    r
}

/// Apaga os temporários do arquivo de estado que uma gravação interrompida deixou
/// (Fase 49a): `.<nome>.tmp-<pid>-<n>` na pasta do estado, só arquivos comuns do euid
/// (`DirEntry::file_type` não segue link), com o nome exato (pid e contador só com
/// dígitos) e de **outro processo** (pid ≠ o nosso: um temporário nosso pode estar sendo
/// gravado agora). O conteúdo deles é só o texto do estado (gerações e contagem), nada
/// secreto, mas eles se acumulariam. Devolve quantos apagou; erro de E/S para no meio.
pub fn limpar_temporarios_estado(caminho: &Path) -> io::Result<usize> {
    let dir = caminho
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .ok_or(io::ErrorKind::InvalidInput)?;
    let nome = caminho
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or(io::ErrorKind::InvalidInput)?;
    let prefixo = format!(".{nome}.tmp-");
    let meu = std::process::id().to_string();
    let euid = ward_plataforma::uid_efetivo();
    let mut apagados = 0;
    for item in fs::read_dir(dir)? {
        let item = item?;
        let Some(n) = item.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some(resto) = n.strip_prefix(&prefixo) else {
            continue;
        };
        let Some((pid, cont)) = resto.split_once('-') else {
            continue;
        };
        let so_digitos =
            |t: &str| !t.is_empty() && t.len() <= 20 && t.bytes().all(|b| b.is_ascii_digit());
        if !so_digitos(pid) || !so_digitos(cont) || pid == meu {
            continue;
        }
        if !item.file_type()?.is_file() {
            continue;
        }
        if item.metadata()?.uid() != euid {
            continue;
        }
        fs::remove_file(item.path())?;
        apagados += 1;
    }
    Ok(apagados)
}

/// Pasta de backups aberta pelo fd (Fase 45): o diretório foi aberto com
/// `O_NOFOLLOW | O_DIRECTORY` e conferido pelo `fstat` (diretório do euid, 0700). Todo
/// acesso passa por `/proc/self/fd/<n>/…`, então trocar a pasta por um link simbólico
/// ou por outro diretório depois de aberta não desvia nada.
pub struct PastaAberta {
    fd: File,
}

impl PastaAberta {
    /// O caminho pelo fd, para o `ward_core::backup`.
    pub fn caminho(&self) -> PathBuf {
        use std::os::fd::AsRawFd;
        PathBuf::from(format!("/proc/self/fd/{}", self.fd.as_raw_fd()))
    }

    /// A pasta para o núcleo, com o uid esperado.
    pub fn pasta(&self) -> ward_core::backup::PastaBackup {
        ward_core::backup::PastaBackup::nova(self.caminho(), ward_plataforma::uid_efetivo())
    }
}

/// Pasta de backups ao lado do cofre: `<pasta do cofre>/backups` (D17:
/// `~/.local/share/ward/backups`; no `modo-teste`, `.dev/backups`).
pub fn dir_backups(cofre: &Path) -> Option<PathBuf> {
    cofre
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .map(|d| d.join("backups"))
}

/// Abre a pasta de backups (cria 0700 se faltar, sem criar os pais). Link simbólico no
/// lugar, outro dono ou qualquer permissão de grupo/outros: `PermissionDenied`.
pub fn abrir_pasta_backup(dir: &Path) -> io::Result<PastaAberta> {
    use std::os::unix::fs::DirBuilderExt;
    match fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    let fd = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_CLOEXEC)
        .open(dir)?;
    let m = fd.metadata()?;
    if !m.is_dir() || m.uid() != ward_plataforma::uid_efetivo() || m.mode() & 0o077 != 0 {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(PastaAberta { fd })
}

#[cfg(test)]
mod testes {
    use super::*;

    fn so_geracoes(texto: &str) -> Option<Geracoes> {
        analisar_estado(texto).map(|e| e.geracoes)
    }

    /// Auditoria do M4 (baixo): `known_hosts` frouxo ou trocado é recusado.
    #[test]
    fn ataque_known_hosts_frouxo_ou_link_e_recusado() {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
        let raiz = std::env::temp_dir().join(format!("ward-kh-{}", std::process::id()));
        let _ = fs::remove_dir_all(&raiz);
        fs::DirBuilder::new().mode(0o700).create(&raiz).unwrap();
        let kh = raiz.join("known_hosts");
        // Não existe: nasce vazio e 0600.
        preparar_known_hosts(&kh).unwrap();
        let m = fs::symlink_metadata(&kh).unwrap();
        assert!(m.is_file() && m.len() == 0);
        assert_eq!(m.mode() & 0o777, 0o600);
        // Existe e certo: aceito de novo, sem mexer.
        fs::write(&kh, "[127.0.0.1]:2222 ssh-ed25519 AAAAfalso\n").unwrap();
        preparar_known_hosts(&kh).unwrap();
        // Arquivo legível ou gravável por outros: recusa.
        for modo in [0o644, 0o640, 0o602, 0o660] {
            fs::set_permissions(&kh, fs::Permissions::from_mode(modo)).unwrap();
            assert!(preparar_known_hosts(&kh).is_err(), "{modo:o}");
        }
        fs::set_permissions(&kh, fs::Permissions::from_mode(0o600)).unwrap();
        // Pasta frouxa: recusa.
        for modo in [0o755, 0o750, 0o711] {
            fs::set_permissions(&raiz, fs::Permissions::from_mode(modo)).unwrap();
            assert!(preparar_known_hosts(&kh).is_err(), "pasta {modo:o}");
        }
        fs::set_permissions(&raiz, fs::Permissions::from_mode(0o700)).unwrap();
        // Link no lugar do arquivo (para um arquivo 0600 de outro lugar): recusa.
        let alvo = raiz.join("outro");
        fs::write(&alvo, "").unwrap();
        fs::set_permissions(&alvo, fs::Permissions::from_mode(0o600)).unwrap();
        fs::remove_file(&kh).unwrap();
        symlink(&alvo, &kh).unwrap();
        assert!(preparar_known_hosts(&kh).is_err());
        fs::remove_file(&kh).unwrap();
        // Link quebrado: não cria o arquivo do outro lado.
        symlink(raiz.join("nao-existe"), &kh).unwrap();
        assert!(preparar_known_hosts(&kh).is_err());
        assert!(!raiz.join("nao-existe").exists());
        fs::remove_file(&kh).unwrap();
        // Link duro (outro nome para o mesmo arquivo): recusa.
        fs::hard_link(&alvo, &kh).unwrap();
        assert!(preparar_known_hosts(&kh).is_err());
        fs::remove_file(&kh).unwrap();
        // Diretório no lugar do arquivo: recusa.
        fs::DirBuilder::new().mode(0o700).create(&kh).unwrap();
        assert!(preparar_known_hosts(&kh).is_err());
        fs::remove_dir(&kh).unwrap();
        // Pasta que é link: recusa.
        let link_pasta = std::env::temp_dir().join(format!("ward-kh-l-{}", std::process::id()));
        let _ = fs::remove_file(&link_pasta);
        symlink(&raiz, &link_pasta).unwrap();
        assert!(preparar_known_hosts(&link_pasta.join("known_hosts")).is_err());
        fs::remove_file(&link_pasta).unwrap();
        fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn estado_estrito() {
        let ok = so_geracoes("ward-estado 1\nvista 3\nqueimada 4\n").unwrap();
        assert_eq!(
            ok,
            Geracoes {
                vista: 3,
                queimada: 4
            }
        );
        let cheio = ArquivoEstado {
            geracoes: Geracoes {
                vista: 0,
                queimada: u64::MAX,
            },
            falhas_agente: u32::MAX,
        };
        assert_eq!(analisar_estado(&texto_estado(cheio)), Some(cheio));
        for ruim in [
            "",
            "ward-estado 1\nvista 3\nqueimada 4",
            "ward-estado 2\nvista 3\nqueimada 4\n",
            "ward-estado 1\nvista 03\nqueimada 4\n",
            "ward-estado 1\nvista 5\nqueimada 4\n",
            "ward-estado 1\nvista -1\nqueimada 4\n",
            "ward-estado 1\nvista 3\nqueimada 4\nextra 1\n",
            "ward-estado 1\nqueimada 4\nvista 3\n",
            "ward-estado 1\nvista  3\nqueimada 4\n",
            "ward-estado 1\nvista 3\nqueimada 18446744073709551616\n",
            "ward-estado 1\r\nvista 3\nqueimada 4\n",
            // A linha das falhas do agente (D62): estrita como as outras.
            "ward-estado 1\nvista 3\nqueimada 4\nfalhas_agente 01\n",
            "ward-estado 1\nvista 3\nqueimada 4\nfalhas_agente -1\n",
            "ward-estado 1\nvista 3\nqueimada 4\nfalhas_agente 4294967296\n",
            "ward-estado 1\nvista 3\nqueimada 4\nfalhas_agente\n",
            "ward-estado 1\nvista 3\nqueimada 4\nfalhas 2\n",
            "ward-estado 1\nvista 3\nqueimada 4\nfalhas_agente 2\nfalhas_agente 2\n",
            "ward-estado 1\nfalhas_agente 2\nvista 3\nqueimada 4\n",
        ] {
            assert_eq!(analisar_estado(ruim), None, "{ruim:?}");
        }
    }

    /// D62: o formato antigo (três linhas) é lido com 0 falhas do agente; o novo, com a
    /// linha; a gravação sempre regrava no formato novo.
    #[test]
    fn d62_formato_antigo_le_zero_e_novo_tem_a_linha() {
        let antigo = analisar_estado("ward-estado 1\nvista 3\nqueimada 4\n").unwrap();
        assert_eq!(antigo.falhas_agente, 0);
        let novo =
            analisar_estado("ward-estado 1\nvista 3\nqueimada 4\nfalhas_agente 10\n").unwrap();
        assert_eq!(novo.falhas_agente, 10);
        assert_eq!(novo.geracoes, antigo.geracoes);
        assert_eq!(
            texto_estado(antigo),
            "ward-estado 1\nvista 3\nqueimada 4\nfalhas_agente 0\n"
        );
    }
}
