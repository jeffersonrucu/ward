//! Backup externo em pendrive (R27, D17, D30; Fase 46).
//!
//! - **Desligado por padrão.** Ligado/desligado e o UUID do pendrive ficam na
//!   configuração do cofre (D30), dentro da parte cifrada: o daemon só sabe qual é o
//!   pendrive com o cofre destravado. Travado ou desligado, nada é copiado.
//! - **Qual pendrive.** O udisks2 ([`crate::udisks`]) diz quais sistemas de arquivos
//!   existem, o UUID de cada um, o número do dispositivo e onde estão montados. O Ward
//!   nunca monta nada: copia quando o pendrive escolhido aparece montado (pelo
//!   ambiente da área de trabalho ou pelo usuário). O UUID tem de bater com um único
//!   sistema de arquivos (dois pendrives clonados com o mesmo UUID: nada é copiado).
//! - **Pendrive trocado.** A montagem é aberta com `O_NOFOLLOW | O_DIRECTORY` e o
//!   `st_dev` do fd tem de ser o dispositivo que o udisks2 deu para aquele UUID. Depois
//!   de aberta, o udisks2 é consultado de novo e tem de dizer o mesmo (UUID,
//!   dispositivo e montagem). Daí em diante tudo passa pelo fd: trocar o pendrive, ou
//!   montar outro no mesmo caminho, não desvia a cópia para ele.
//! - **Pasta `<montagem>/ward-backup/`.** Criada se faltar, aberta com `O_NOFOLLOW`
//!   (link simbólico no lugar, num pendrive ext4, é recusado), do mesmo dispositivo e
//!   do próprio usuário. A permissão não é conferida: o vfat não tem dono nem modo por
//!   arquivo (vêm das opções de montagem). Os arquivos são o backup cifrado (S2), e
//!   nada além deles vai para o pendrive.
//! - **O que vai.** As versões do backup local (Fase 45, já cifradas com a senha mestra
//!   de cada versão) que o pendrive ainda não tem, até [`VERSOES`]: só arquivos comuns
//!   do usuário, com o nome exato e o cabeçalho do formato. Nenhum byte é decifrado ou
//!   selado aqui.
//! - **Sem `link(2)`** (o vfat não tem): temporário `O_EXCL` 0600 → fsync →
//!   `renameat2(RENAME_NOREPLACE)` para o nome final ([`renomear_sem_sobrescrever`]) →
//!   fsync da pasta. Uma versão que já está no pendrive nunca é sobrescrita; uma
//!   cópia interrompida deixa só um temporário com prefixo do cifrado, apagado na
//!   próxima.
//! - Depois de copiar, o pendrive fica com as [`VERSOES`] mais novas (pela data do
//!   nome), nunca apagando o que acabou de copiar nem o que não tem nome de versão.
//!   **Datas futuras** (Fase 49a): só versões com data até agora contam e podem ser
//!   apagadas ([`ward_core::backup::para_apagar`]); as plantadas com data futura (no
//!   pendrive ou no backup local) não fazem as reais saírem. Do backup local só vão as
//!   versões até agora + [`ward_core::backup::MARGEM_FUTURO`].
//! - **O pendrive é reconhecido só pelo UUID do sistema de arquivos** (Fase 49a). O UUID
//!   não é segredo nem prova de nada: quem tem um pendrive pode formatar outro com o
//!   mesmo UUID (`mkfs.vfat -i`, `tune2fs -U`) e, com ele conectado e montado no lugar do
//!   escolhido, recebe as cópias. Isso não abre nada, porque cada cópia é o arquivo
//!   cifrado com a senha mestra da versão (S2, S6), mas é uma cópia a mais fora da sua
//!   mão. Dois pendrives com o mesmo UUID conectados ao mesmo tempo não recebem nada
//!   (`Ambiguo`). Não há marcador na pasta `ward-backup/`: um arquivo plantado no
//!   pendrive não seria mais difícil de copiar do que o UUID.
//!
//! O log só tem textos fixos ([`ErroExterno::codigo`]): nem UUID, nem rótulo, nem
//! caminho.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use ward_core::backup::{Versao, para_apagar};
use ward_core::formato;
use ward_plataforma::pasta::renomear_sem_sobrescrever;

use crate::arquivos::{self, PastaAberta};

/// Pasta das cópias no pendrive (D17).
pub const PASTA: &str = "ward-backup";
/// Prefixo dos temporários na pasta do pendrive.
const PREFIXO_TEMPORARIO: &str = ".ward-tmp-";

/// Um sistema de arquivos que o udisks2 mostra (só o que o Ward usa).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Visto {
    /// UUID do sistema de arquivos (`IdUUID`), já conferido pelo formato de D45.
    pub uuid: String,
    /// Rótulo (`IdLabel`), sem caractere de controle.
    pub rotulo: String,
    /// Primeiro ponto de montagem (`MountPoints`), absoluto; `None` se não montado.
    pub montagem: Option<PathBuf>,
    /// Número do dispositivo (`DeviceNumber`), o mesmo `dev_t` do `st_dev`.
    pub dispositivo: u64,
}

/// De onde vêm os pendrives (o udisks2 no daemon; uma lista nos testes).
pub trait FontePendrives: Send + Sync {
    /// Os sistemas de arquivos de pendrive vistos agora; `None` se a fonte não responde.
    fn pendrives(&self) -> Option<Vec<Visto>>;
}

/// Por que a cópia não aconteceu. Sem UUID, rótulo, caminho nem conteúdo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroExterno {
    /// O udisks2 não respondeu.
    SemUdisks,
    /// O pendrive escolhido não está conectado.
    NaoConectado,
    /// Mais de um sistema de arquivos com o mesmo UUID.
    Ambiguo,
    /// Conectado, mas não montado.
    NaoMontado,
    /// A montagem aberta não é a do dispositivo do UUID (pendrive trocado, link, outro
    /// sistema de arquivos no caminho) ou o udisks2 mudou de ideia no meio.
    Trocado,
    /// `ward-backup` não é um diretório do usuário no pendrive (link, outro dono…).
    PastaInsegura,
    /// A pasta de backups local não abre (Fase 45).
    SemBackupLocal,
    /// Falha de E/S no pendrive (cheio, removido no meio…).
    Io(io::ErrorKind),
}

impl ErroExterno {
    /// Texto fixo para o log.
    pub fn codigo(self) -> &'static str {
        match self {
            Self::SemUdisks => "udisks2 indisponível",
            Self::NaoConectado => "pendrive não conectado",
            Self::Ambiguo => "mais de um pendrive com o mesmo UUID",
            Self::NaoMontado => "pendrive não montado",
            Self::Trocado => "a montagem não é a do pendrive escolhido",
            Self::PastaInsegura => "pasta ward-backup insegura no pendrive",
            Self::SemBackupLocal => "pasta de backups local indisponível",
            Self::Io(_) => "e/s no pendrive",
        }
    }
}

/// O que uma cópia fez.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Copia {
    /// Versões copiadas agora (as que o pendrive não tinha).
    pub copiadas: Vec<Versao>,
    /// Versões antigas apagadas do pendrive pela rotação.
    pub apagadas: usize,
}

/// O único sistema de arquivos com o `uuid` (sem diferença de maiúscula, como o
/// `IdUUID` do vfat vem).
pub fn escolher<'a>(vistos: &'a [Visto], uuid: &str) -> Result<&'a Visto, ErroExterno> {
    let mut achados = vistos.iter().filter(|v| v.uuid.eq_ignore_ascii_case(uuid));
    let v = achados.next().ok_or(ErroExterno::NaoConectado)?;
    if achados.next().is_some() {
        return Err(ErroExterno::Ambiguo);
    }
    Ok(v)
}

fn io(e: io::Error) -> ErroExterno {
    ErroExterno::Io(e.kind())
}

fn abrir_dir(caminho: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_CLOEXEC)
        .open(caminho)
}

fn pelo_fd(f: &File) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", f.as_raw_fd()))
}

/// Abre a montagem do pendrive e confere que é a do dispositivo do UUID, inclusive
/// perguntando de novo ao udisks2 depois de aberta.
fn abrir_montagem(
    visto: &Visto,
    uuid: &str,
    fonte: &dyn FontePendrives,
) -> Result<File, ErroExterno> {
    let montagem = visto.montagem.as_deref().ok_or(ErroExterno::NaoMontado)?;
    let fd = abrir_dir(montagem).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => ErroExterno::NaoMontado,
        _ => ErroExterno::Trocado,
    })?;
    let m = fd.metadata().map_err(io)?;
    if !m.is_dir() || m.dev() != visto.dispositivo {
        return Err(ErroExterno::Trocado);
    }
    // De novo, com o fd já aberto: o que o udisks2 diz agora tem de ser o mesmo. Uma
    // troca antes da abertura aparece aqui; uma depois não muda para onde o fd aponta.
    let agora = fonte.pendrives().ok_or(ErroExterno::SemUdisks)?;
    let de_novo = escolher(&agora, uuid)?;
    if de_novo.dispositivo != visto.dispositivo || de_novo.montagem != visto.montagem {
        return Err(ErroExterno::Trocado);
    }
    Ok(fd)
}

/// `<montagem>/ward-backup`, criada se faltar, pelo fd da montagem.
fn abrir_destino(montagem: &File, dispositivo: u64) -> Result<File, ErroExterno> {
    let caminho = pelo_fd(montagem).join(PASTA);
    match fs::DirBuilder::new().mode(0o700).create(&caminho) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(io(e)),
    }
    let fd = abrir_dir(&caminho).map_err(|e| match e.kind() {
        // `ELOOP` (link simbólico) e `ENOTDIR` (arquivo no lugar).
        io::ErrorKind::NotADirectory => ErroExterno::PastaInsegura,
        _ if e.raw_os_error() == Some(libc::ELOOP) => ErroExterno::PastaInsegura,
        _ => io(e),
    })?;
    let m = fd.metadata().map_err(io)?;
    if !m.is_dir() || m.dev() != dispositivo || m.uid() != ward_plataforma::uid_efetivo() {
        return Err(ErroExterno::PastaInsegura);
    }
    Ok(fd)
}

/// Versões com nome exato que são arquivos comuns na pasta (sem seguir link).
fn versoes_em(dir: &Path) -> Result<Vec<Versao>, ErroExterno> {
    let mut v = Vec::new();
    for item in fs::read_dir(dir).map_err(io)? {
        let item = item.map_err(io)?;
        let Some(versao) = item.file_name().to_str().and_then(Versao::de_nome) else {
            continue;
        };
        if item.file_type().map_err(io)?.is_file() {
            v.push(versao);
        }
    }
    v.sort_unstable_by(|a, b| b.cmp(a));
    Ok(v)
}

/// Apaga os temporários de uma cópia interrompida (só arquivos comuns com o prefixo).
fn limpar_temporarios(dir: &Path) {
    let Ok(itens) = fs::read_dir(dir) else {
        return;
    };
    for item in itens.flatten() {
        let nome = item.file_name();
        let e_temporario = nome
            .to_str()
            .is_some_and(|n| n.starts_with(PREFIXO_TEMPORARIO));
        if e_temporario && item.file_type().is_ok_and(|t| t.is_file()) {
            let _ = fs::remove_file(dir.join(nome));
        }
    }
}

static CONTADOR: AtomicU64 = AtomicU64::new(0);

fn nome_temporario() -> OsString {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    format!(
        "{PREFIXO_TEMPORARIO}{}-{}-{nanos:08x}",
        std::process::id(),
        CONTADOR.fetch_add(1, Ordering::Relaxed)
    )
    .into()
}

/// Escreve `bytes` como a versão `v` na pasta, sem sobrescrever. `Ok(false)`: o nome
/// já existia (nada mudou).
fn gravar_versao(destino: &File, v: Versao, bytes: &[u8]) -> io::Result<bool> {
    let dir = pelo_fd(destino);
    let tmp = nome_temporario();
    let caminho_tmp = dir.join(&tmp);
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&caminho_tmp)?;
    let escrito = f.write_all(bytes).and_then(|()| f.sync_all());
    drop(f);
    let r =
        escrito.and_then(|()| renomear_sem_sobrescrever(destino.as_fd(), &tmp, v.nome().as_ref()));
    match r {
        Ok(()) => Ok(true),
        Err(e) => {
            let _ = fs::remove_file(&caminho_tmp);
            if e.kind() == io::ErrorKind::AlreadyExists {
                Ok(false)
            } else {
                Err(e)
            }
        }
    }
}

/// Apaga as versões mais antigas do pendrive até sobrarem
/// [`ward_core::backup::VERSOES`] com data até `teto` (agora), nunca uma das `novas`,
/// nunca uma com data depois do `teto` e só arquivos comuns (Fase 49a).
fn rodar(destino: &File, novas: &[Versao], teto: Versao) -> Result<usize, ErroExterno> {
    let dir = pelo_fd(destino);
    let lista = versoes_em(&dir)?;
    let mut apagadas = 0;
    for v in para_apagar(&lista, teto, novas) {
        let caminho = dir.join(v.nome());
        if fs::symlink_metadata(&caminho).is_ok_and(|m| m.file_type().is_file()) {
            fs::remove_file(&caminho).map_err(io)?;
            apagadas += 1;
        }
    }
    Ok(apagadas)
}

/// Copia para o pendrive `visto` (o do `uuid`) as versões do backup local que ele ainda
/// não tem, e gira para [`VERSOES`]. Nada é sobrescrito.
pub fn copiar(
    local: &PastaAberta,
    visto: &Visto,
    uuid: &str,
    fonte: &dyn FontePendrives,
) -> Result<Copia, ErroExterno> {
    let agora = SystemTime::now();
    let teto = Versao::do_instante(agora)
        .map(Versao::fim_do_segundo)
        .ok_or(ErroExterno::SemBackupLocal)?;
    let montagem = abrir_montagem(visto, uuid, fonte)?;
    let destino = abrir_destino(&montagem, visto.dispositivo)?;
    drop(montagem);
    let dir = pelo_fd(&destino);
    limpar_temporarios(&dir);
    let la: BTreeSet<Versao> = versoes_em(&dir)?.into_iter().collect();
    let pasta = local.pasta();
    // Sem as versões locais com data futura (Fase 49a).
    let locais = pasta
        .listar_ate(agora)
        .map_err(|_| ErroExterno::SemBackupLocal)?;
    let mut copia = Copia::default();
    // Da mais antiga para a mais nova, só as `VERSOES` mais novas.
    for v in locais.into_iter().take(ward_core::backup::VERSOES).rev() {
        if la.contains(&v) {
            continue;
        }
        // Arquivo comum do usuário, sem link, do tamanho do formato.
        let bytes = match arquivos::ler_cofre(&pasta.caminho(v)) {
            Ok(Some((b, _))) => b,
            // Sumiu pela rotação local no meio, ou não é um arquivo do usuário.
            Ok(None) | Err(_) => continue,
        };
        if formato::ler_cabecalho(&bytes).is_err() {
            continue;
        }
        if gravar_versao(&destino, v, &bytes).map_err(io)? {
            copia.copiadas.push(v);
        }
    }
    copia.apagadas = rodar(&destino, &copia.copiadas, teto)?;
    // O vfat aceita fsync da pasta; se outro sistema de arquivos não aceitar, os
    // arquivos já foram sincronizados um a um.
    let _ = destino.sync_all();
    Ok(copia)
}
