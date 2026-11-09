//! Backup local automático (R27, D17, D27; Fase 45).
//!
//! - A cada gravação do cofre que deu certo, a **mesma versão selada** (os bytes
//!   cifrados que acabaram de ir para o cofre) vai para a pasta de backups como
//!   `ward-AAAAMMDD-HHMMSS.ward`: mesmo formato, cifrado com a senha mestra daquela
//!   versão. Nenhum byte em claro passa por aqui (S2), e nada é selado de novo: o backup
//!   não queima nem repete geração (D53).
//! - Data e hora em **UTC** (o núcleo não tem fuso horário; a interface mostra no fuso
//!   local). Duas versões no mesmo segundo ganham sufixo: `ward-AAAAMMDD-HHMMSS-1.ward`
//!   até `-99`, nessa ordem.
//! - Gravação atômica pelo caminho interno de [`crate::gravacao`] (D27): temporário
//!   `O_EXCL` 0600 → fsync → `link(2)` para o nome final, que **nunca sobrescreve** →
//!   fsync do diretório. Uma gravação interrompida em qualquer ponto não mexe nos
//!   backups que já estavam lá (S7, V7.3); o temporário que sobrar é cifrado e é
//!   apagado no começo da gravação seguinte.
//! - Mantém [`VERSOES`] versões: depois de gravar, apaga as mais antigas (pela data do
//!   nome), nunca a que acabou de gravar, e só arquivos comuns com o nome exato.
//! - A pasta tem de ser um diretório do próprio usuário, sem nenhuma permissão para
//!   grupo e outros (0700). O daemon a abre com `O_NOFOLLOW | O_DIRECTORY` e passa o
//!   caminho pelo fd (`/proc/self/fd/<n>`): trocar a pasta por um link simbólico, ou por
//!   outro diretório no meio da operação, não desvia nada (V7.x, "diretório trocado por
//!   link").
//!
//! Restaurar é do daemon, só pelo papel app (D17, D39, V7.8): abre a versão com a
//! senha dela ([`crate::corpo::abrir_entradas_lida`] sem geração mínima), guarda o cofre
//! atual como backup ([`guardar_atual`]) e sela a versão de novo com geração = maior
//! queimada + 1 pela gravação normal.

use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::formato::CofreAberto;
use crate::gravacao::{self, ErroGravacao, Ponto};

/// Quantas versões ficam na pasta (D17).
pub const VERSOES: usize = 30;
/// Maior sufixo para versões no mesmo segundo.
pub const MAX_SUFIXO: u32 = 99;
/// Base do nome dos temporários na pasta (`.backup.ward-tmp-<hex>`).
const BASE_TEMPORARIO: &str = "backup";
/// Tamanho do maior nome válido: `ward-AAAAMMDD-HHMMSS-NN.ward`.
pub const MAX_NOME: usize = 28;

/// Uma versão de backup, identificada pelo nome do arquivo. A ordem é a do tempo:
/// data, hora e sufixo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Versao {
    /// AAAAMMDD.
    data: u32,
    /// HHMMSS.
    hora: u32,
    /// 0 = sem sufixo.
    sufixo: u32,
}

impl Versao {
    /// A versão de um instante (UTC), sem sufixo. `None` antes de 1970 ou depois de
    /// 9999.
    pub fn do_instante(t: SystemTime) -> Option<Self> {
        let s = t.duration_since(UNIX_EPOCH).ok()?.as_secs();
        let dias = i64::try_from(s / 86_400).ok()?;
        let resto = s % 86_400;
        let (a, m, d) = civil_de_dias(dias);
        if !(1970..=9999).contains(&a) {
            return None;
        }
        let data = u32::try_from(a * 10_000 + m * 100 + d).ok()?;
        let hora =
            u32::try_from((resto / 3600) * 10_000 + (resto % 3600 / 60) * 100 + resto % 60).ok()?;
        Some(Self {
            data,
            hora,
            sufixo: 0,
        })
    }

    /// Lê o nome do arquivo, estrito: `ward-AAAAMMDD-HHMMSS.ward` ou com `-N` (1 a 99,
    /// sem zero à esquerda) antes do `.ward`; data e hora possíveis. Qualquer outra coisa
    /// (barra, `..`, espaço, maiúscula) é `None`.
    pub fn de_nome(nome: &str) -> Option<Self> {
        if nome.len() > MAX_NOME || !nome.is_ascii() {
            return None;
        }
        let miolo = nome.strip_prefix("ward-")?.strip_suffix(".ward")?;
        let b = miolo.as_bytes();
        if b.len() < 15 || b[8] != b'-' {
            return None;
        }
        let data = digitos(&b[..8])?;
        let hora = digitos(&b[9..15])?;
        let sufixo = match &b[15..] {
            [] => 0,
            [b'-', resto @ ..] if !resto.is_empty() && resto.len() <= 2 && resto[0] != b'0' => {
                digitos(resto)?
            }
            _ => return None,
        };
        let v = Self { data, hora, sufixo };
        v.possivel().then_some(v)
    }

    /// Nome do arquivo.
    pub fn nome(&self) -> String {
        if self.sufixo == 0 {
            format!("ward-{:08}-{:06}.ward", self.data, self.hora)
        } else {
            format!(
                "ward-{:08}-{:06}-{}.ward",
                self.data, self.hora, self.sufixo
            )
        }
    }

    fn com_sufixo(self, sufixo: u32) -> Self {
        Self { sufixo, ..self }
    }

    fn possivel(&self) -> bool {
        let (a, m, d) = (self.data / 10_000, self.data / 100 % 100, self.data % 100);
        let (h, mi, s) = (self.hora / 10_000, self.hora / 100 % 100, self.hora % 100);
        (1970..=9999).contains(&a)
            && (1..=12).contains(&m)
            && (1..=31).contains(&d)
            && h < 24
            && mi < 60
            && s < 60
            && self.sufixo <= MAX_SUFIXO
    }
}

impl fmt::Display for Versao {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.nome())
    }
}

fn digitos(b: &[u8]) -> Option<u32> {
    if b.is_empty() || !b.iter().all(u8::is_ascii_digit) {
        return None;
    }
    b.iter().try_fold(0u32, |n, &c| {
        n.checked_mul(10)?.checked_add(u32::from(c - b'0'))
    })
}

/// Dias desde 1970-01-01 → (ano, mês, dia) no calendário gregoriano proléptico
/// (algoritmo `civil_from_days` de Howard Hinnant; só aritmética inteira).
fn civil_de_dias(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let a = yoe + era * 400 + i64::from(m <= 2);
    (a, m, d)
}

/// Erro do backup. Não carrega caminho nem conteúdo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroBackup {
    /// A pasta não é um diretório do próprio usuário com 0700 (ou foi trocada por link).
    PastaInsegura,
    /// O arquivo do cofre atual não foi selado por este cofre ([`guardar_atual`]).
    NaoAutentico,
    /// O relógio está antes de 1970 ou depois de 9999.
    Relogio,
    /// Os 100 nomes do mesmo segundo já existem.
    NomesEsgotados,
    /// Falha na gravação atômica (o que estava na pasta continua igual).
    Gravacao(ErroGravacao),
    /// Falha de E/S ao listar ou ao apagar versões antigas.
    Io(io::ErrorKind),
}

impl fmt::Display for ErroBackup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PastaInsegura => {
                f.write_str("a pasta de backups não é um diretório 0700 do usuário")
            }
            Self::NaoAutentico => f.write_str("o arquivo do cofre não é o que o Ward gravou"),
            Self::Relogio => f.write_str("o relógio do sistema está fora do intervalo"),
            Self::NomesEsgotados => f.write_str("backups demais no mesmo segundo"),
            Self::Gravacao(e) => write!(f, "falha ao gravar o backup: {e}"),
            Self::Io(k) => write!(f, "falha de E/S na pasta de backups ({k})"),
        }
    }
}

impl std::error::Error for ErroBackup {}

/// A pasta de backups e o uid dono esperado.
#[derive(Debug, Clone)]
pub struct PastaBackup {
    dir: PathBuf,
    uid: u32,
}

impl PastaBackup {
    /// A pasta em `dir`, de dono `uid`. O daemon passa o caminho pelo fd aberto com
    /// `O_NOFOLLOW` (`/proc/self/fd/<n>`), então `dir` é seguido aqui: quem garante que
    /// não é um link simbólico trocado é quem abriu. Mesmo assim, toda operação confere
    /// que é um diretório do `uid`, sem permissão para grupo e outros.
    pub fn nova(dir: PathBuf, uid: u32) -> Self {
        Self { dir, uid }
    }

    /// Caminho do arquivo de uma versão.
    pub fn caminho(&self, v: Versao) -> PathBuf {
        self.dir.join(v.nome())
    }

    fn conferir(&self) -> Result<(), ErroBackup> {
        let m = fs::metadata(&self.dir).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => ErroBackup::PastaInsegura,
            k => ErroBackup::Io(k),
        })?;
        if !m.is_dir() || m.uid() != self.uid || m.permissions().mode() & 0o077 != 0 {
            return Err(ErroBackup::PastaInsegura);
        }
        Ok(())
    }

    /// As versões na pasta, da mais nova para a mais antiga. Só arquivos comuns (sem
    /// seguir link simbólico) com o nome exato; o resto é ignorado.
    pub fn listar(&self) -> Result<Vec<Versao>, ErroBackup> {
        self.conferir()?;
        let mut v = Vec::new();
        for item in fs::read_dir(&self.dir).map_err(|e| ErroBackup::Io(e.kind()))? {
            let item = item.map_err(|e| ErroBackup::Io(e.kind()))?;
            let Some(versao) = item.file_name().to_str().and_then(Versao::de_nome) else {
                continue;
            };
            // `DirEntry::file_type` não segue link simbólico.
            if item
                .file_type()
                .map_err(|e| ErroBackup::Io(e.kind()))?
                .is_file()
            {
                v.push(versao);
            }
        }
        v.sort_unstable_by(|a, b| b.cmp(a));
        Ok(v)
    }

    /// Grava `arquivo` (já selado e com a estrutura conferida) como a versão do
    /// `instante` e apaga as que passarem de [`VERSOES`].
    pub(crate) fn guardar(
        &self,
        arquivo: &[u8],
        instante: SystemTime,
    ) -> Result<Versao, ErroBackup> {
        self.guardar_com_pontos(arquivo, instante, &mut |_| Ok(()))
    }

    pub(crate) fn guardar_com_pontos(
        &self,
        arquivo: &[u8],
        instante: SystemTime,
        ponto: &mut dyn FnMut(Ponto) -> io::Result<()>,
    ) -> Result<Versao, ErroBackup> {
        self.conferir()?;
        let base = Versao::do_instante(instante).ok_or(ErroBackup::Relogio)?;
        // Temporários de uma gravação interrompida (cifrados), só os da pasta. Antes de
        // escrever: nunca sobra mais de um. Quem grava é só o daemon, uma vez por vez.
        let _ = gravacao::limpar_temporarios(&self.dir.join(BASE_TEMPORARIO));
        let versoes: Vec<Versao> = (0..=MAX_SUFIXO).map(|s| base.com_sufixo(s)).collect();
        let nomes: Vec<OsString> = versoes.iter().map(|v| v.nome().into()).collect();
        let (i, _) = gravacao::gravar_novo_com_pontos(
            &self.dir,
            BASE_TEMPORARIO.as_ref(),
            &nomes,
            arquivo,
            ponto,
        )
        .map_err(|e| match e {
            ErroGravacao::Io {
                tipo: io::ErrorKind::AlreadyExists,
                ..
            } => ErroBackup::NomesEsgotados,
            e => ErroBackup::Gravacao(e),
        })?;
        let nova = versoes[i];
        self.rodar(nova)?;
        Ok(nova)
    }

    /// Apaga as versões mais antigas até sobrarem [`VERSOES`], nunca a `nova`.
    fn rodar(&self, nova: Versao) -> Result<(), ErroBackup> {
        let lista = self.listar()?;
        let mut sobra = lista.len().saturating_sub(VERSOES);
        // Da mais antiga para a mais nova.
        for v in lista.iter().rev() {
            if sobra == 0 {
                break;
            }
            if *v == nova {
                continue;
            }
            let caminho = self.caminho(*v);
            // Só arquivo comum: um link posto no lugar não é seguido nem apagado.
            match fs::symlink_metadata(&caminho) {
                Ok(m) if m.file_type().is_file() => {
                    fs::remove_file(&caminho).map_err(|e| ErroBackup::Io(e.kind()))?;
                    sobra -= 1;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// Guarda o arquivo do cofre atual como backup antes de restaurar outra versão (D17).
/// Só se `arquivo` foi selado por `cofre` (mesma chave de dados, corpo autêntico): o
/// daemon passa o que leu do caminho do cofre, com a identidade conferida, e o cofre
/// aberto que tem na memória.
pub fn guardar_atual(
    pasta: &PastaBackup,
    cofre: &CofreAberto,
    arquivo: &[u8],
    instante: SystemTime,
) -> Result<Versao, ErroBackup> {
    if !cofre.autentica(arquivo) {
        return Err(ErroBackup::NaoAutentico);
    }
    pasta.guardar(arquivo, instante)
}

/// O nome de uma versão é válido? (Para o protocolo conferir o pedido.)
pub fn nome_valido(nome: &str) -> bool {
    Versao::de_nome(nome).is_some()
}

#[cfg(test)]
mod testes;
