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
//! - Mantém [`VERSOES`] versões: depois de gravar, apaga as de geração mais antiga,
//!   nunca a que acabou de gravar, e só arquivos comuns com o nome exato.
//! - **Só o que confere conta e sai** (Fase 54c): a rotação lê cada versão e confere a
//!   geração selada pela chave do cofre aberto ([`para_apagar_conferidas`]). Ficam as
//!   [`VERSOES`] gerações mais altas, cada uma contada uma vez; lixo com nome de versão,
//!   versão de outro cofre, cópia de uma versão real com nome novo e versão de antes de
//!   uma troca de senha mestra (outra chave) não ocupam lugar das reais. O que não
//!   confere nunca é apagado pela rotação: só o usuário apaga.
//! - **Data do nome não é confiável** (Fase 49a): qualquer processo do usuário pode pôr
//!   na pasta um arquivo com nome de data futura. A rotação só conta e só apaga versões
//!   com data **até o segundo da versão que acabou de gravar**: uma versão com
//!   data passada nunca é apagada para manter uma futura, e as futuras não são contadas
//!   nem apagadas. A lista para o restaurar ([`PastaBackup::listar_ate`]) ignora as
//!   versões com data depois de agora + [`MARGEM_FUTURO`], e o daemon confere a geração
//!   selada de cada versão ([`geracao_selada`], pela chave do cofre aberto) e marca as
//!   que estão fora de ordem ([`fora_de_ordem`]); a geração em si não sai do daemon.
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
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
/// Folga para o relógio (Fase 49a): uma versão com data até agora + esta margem ainda
/// aparece na lista do restaurar (um ajuste pequeno do relógio para trás não esconde a
/// gravação mais nova). Depois disso, a versão é ignorada até o relógio chegar lá.
pub const MARGEM_FUTURO: Duration = Duration::from_secs(5 * 60);

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

    /// A maior versão possível no mesmo segundo de `self` (sufixo [`MAX_SUFIXO`]): o teto
    /// das versões "até este instante".
    pub fn fim_do_segundo(self) -> Self {
        self.com_sufixo(MAX_SUFIXO)
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

/// Como a rotação lê uma versão da pasta para conferir a geração selada (Fase 54c):
/// `None` se não é um arquivo comum do usuário do tamanho do formato, ou não deu para
/// ler. O daemon passa um leitor com `O_NOFOLLOW | O_NONBLOCK` ([`PastaBackup::com_leitor`]).
pub type LerVersao = fn(&Path, u32) -> Option<Vec<u8>>;

/// A pasta de backups e o uid dono esperado.
#[derive(Debug, Clone)]
pub struct PastaBackup {
    dir: PathBuf,
    uid: u32,
    ler: LerVersao,
}

impl PastaBackup {
    /// A pasta em `dir`, de dono `uid`. O daemon passa o caminho pelo fd aberto com
    /// `O_NOFOLLOW` (`/proc/self/fd/<n>`), então `dir` é seguido aqui: quem garante que
    /// não é um link simbólico trocado é quem abriu. Mesmo assim, toda operação confere
    /// que é um diretório do `uid`, sem permissão para grupo e outros.
    pub fn nova(dir: PathBuf, uid: u32) -> Self {
        Self {
            dir,
            uid,
            ler: ler_versao_padrao,
        }
    }

    /// Troca o leitor das versões da rotação (Fase 54c). O núcleo não tem `libc`; o
    /// daemon passa um que abre com `O_NOFOLLOW | O_NONBLOCK` (um FIFO trocado no lugar
    /// de uma versão não prende a gravação).
    pub fn com_leitor(mut self, ler: LerVersao) -> Self {
        self.ler = ler;
        self
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

    /// As versões da pasta que não estão no futuro: data até `agora` +
    /// [`MARGEM_FUTURO`] (Fase 49a), da mais nova para a mais antiga. Uma versão plantada
    /// com data futura não aparece como a mais nova no restaurar nem vai para o pendrive.
    pub fn listar_ate(&self, agora: SystemTime) -> Result<Vec<Versao>, ErroBackup> {
        let teto = teto_de(agora)?;
        let mut v = self.listar()?;
        v.retain(|x| *x <= teto);
        Ok(v)
    }

    /// Grava `arquivo` (já selado por `cofre` e com a estrutura conferida) como a
    /// versão do `instante` e apaga as que passarem de [`VERSOES`] entre as que conferem
    /// pela chave de `cofre` (Fase 54c, [`para_apagar_conferidas`]).
    pub(crate) fn guardar(
        &self,
        arquivo: &[u8],
        instante: SystemTime,
        cofre: &CofreAberto,
    ) -> Result<Versao, ErroBackup> {
        self.guardar_com_pontos(
            arquivo,
            instante,
            &|b: &[u8]| geracao_selada(cofre, b),
            &mut |_| Ok(()),
        )
    }

    /// [`PastaBackup::guardar`] com a conferência da geração (`geracao`) e os pontos
    /// da gravação à parte (os testes do V7.3 os trocam).
    pub(crate) fn guardar_com_pontos(
        &self,
        arquivo: &[u8],
        instante: SystemTime,
        geracao: &dyn Fn(&[u8]) -> Option<u64>,
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
        self.rodar(nova, geracao)?;
        Ok(nova)
    }

    /// Apaga as versões de geração mais antiga até sobrarem [`VERSOES`] gerações com
    /// data até o segundo da `nova`, nunca a `nova`, nunca uma com data depois desse
    /// segundo e nunca uma que não confere pela chave de `cofre`
    /// ([`para_apagar_conferidas`], Fase 54c). O teto é o fim do segundo (sufixo 99),
    /// não a `nova`: a rotação pode ter liberado um nome de sufixo menor no mesmo
    /// segundo, que a `nova` reusa. Só as versões até o teto são lidas e decifradas (o
    /// conteúdo zera ao sair; só a geração fica).
    fn rodar(
        &self,
        nova: Versao,
        geracao: &dyn Fn(&[u8]) -> Option<u64>,
    ) -> Result<(), ErroBackup> {
        let teto = nova.fim_do_segundo();
        let lista: Vec<(Versao, Option<u64>)> = self
            .listar()?
            .into_iter()
            .filter(|v| *v <= teto)
            .map(|v| {
                let g = (self.ler)(&self.caminho(v), self.uid).and_then(|b| geracao(&b));
                (v, g)
            })
            .collect();
        for v in para_apagar_conferidas(&lista, teto, &[nova]) {
            let caminho = self.caminho(v);
            // Só arquivo comum: um link posto no lugar não é seguido nem apagado.
            match fs::symlink_metadata(&caminho) {
                Ok(m) if m.file_type().is_file() => {
                    fs::remove_file(&caminho).map_err(|e| ErroBackup::Io(e.kind()))?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// O teto das versões aceitas em `agora`: o fim do segundo de agora + [`MARGEM_FUTURO`].
fn teto_de(agora: SystemTime) -> Result<Versao, ErroBackup> {
    let t = agora
        .checked_add(MARGEM_FUTURO)
        .ok_or(ErroBackup::Relogio)?;
    Versao::do_instante(t)
        .map(Versao::fim_do_segundo)
        .ok_or(ErroBackup::Relogio)
}

/// A rotação desde a Fase 54c (local e no pendrive): das versões em `lista`, com a
/// geração selada de cada uma ([`geracao_selada`]; `None` = não confere pela chave do
/// cofre aberto), quais apagar. **A data do nome não decide nada além do teto**, e o
/// que não confere nunca conta nem sai:
/// - contam só as versões com data até o `teto` (Fase 49a) e geração conhecida;
/// - ficam as [`VERSOES`] **gerações** mais altas (a geração é autenticada pela AEAD e
///   nunca se repete, D53); cada geração conta uma vez, então cópias de uma versão real
///   com nomes novos não ocupam lugar;
/// - saem as versões (originais e cópias) das gerações mais baixas, menos as
///   `protegidas` (o que acabou de ser gravado ou copiado);
/// - lixo, versão de outro cofre e versão de antes de uma troca de senha mestra (outra
///   chave) não contam e não são apagados: só o usuário os apaga.
///
/// Um relógio que voltou não apaga nada a mais: a versão nova, com data mais antiga,
/// tem a geração mais alta. Devolve da geração mais baixa para a mais alta (e, na mesma
/// geração, da data mais antiga para a mais nova).
pub fn para_apagar_conferidas(
    lista: &[(Versao, Option<u64>)],
    teto: Versao,
    protegidas: &[Versao],
) -> Vec<Versao> {
    let mut conferidas: Vec<(u64, Versao)> = lista
        .iter()
        .filter(|(v, _)| *v <= teto)
        .filter_map(|(v, g)| g.map(|g| (g, *v)))
        .collect();
    conferidas.sort_unstable();
    conferidas.dedup();
    let mut geracoes: Vec<u64> = conferidas.iter().map(|(g, _)| *g).collect();
    geracoes.dedup();
    let sobra = geracoes.len().saturating_sub(VERSOES);
    let Some(&ultima_fora) = sobra.checked_sub(1).and_then(|i| geracoes.get(i)) else {
        return Vec::new();
    };
    conferidas
        .into_iter()
        .filter(|(g, v)| *g <= ultima_fora && !protegidas.contains(v))
        .map(|(_, v)| v)
        .collect()
}

/// Lê uma versão para a rotação só com a biblioteca padrão (o padrão de
/// [`PastaBackup::nova`]; o daemon troca por um com `O_NOFOLLOW | O_NONBLOCK`): arquivo
/// comum do `uid` (sem seguir link), até o tamanho do formato, e o arquivo aberto tem
/// de ser o mesmo do `lstat`.
fn ler_versao_padrao(caminho: &Path, uid: u32) -> Option<Vec<u8>> {
    use std::io::Read;
    let limite = (crate::formato::TAM_CABECALHO
        + crate::formato::MAX_CONTEUDO
        + crate::crypto::TAG_SIZE) as u64;
    let antes = fs::symlink_metadata(caminho).ok()?;
    if !antes.file_type().is_file() || antes.uid() != uid || antes.len() > limite {
        return None;
    }
    let f = fs::File::open(caminho).ok()?;
    let m = f.metadata().ok()?;
    if !m.file_type().is_file() || m.dev() != antes.dev() || m.ino() != antes.ino() {
        return None;
    }
    let mut bytes = Vec::new();
    f.take(limite + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= limite).then_some(bytes)
}

/// Quais versões estão fora de ordem (Fase 49a): pela data do nome, da mais antiga para
/// a mais nova, uma versão cuja geração selada é menor que a de alguma versão com data
/// anterior (uma cópia antiga com nome novo, ou o relógio que andou para trás). Versões
/// sem geração conhecida (`None`: outra chave) não contam nem saem marcadas. Devolve uma
/// marca por item, na ordem de `lista`. A geração fica no daemon: só a marca vai ao app
/// (nenhuma resposta leva a geração, D39).
pub fn fora_de_ordem(lista: &[(Versao, Option<u64>)]) -> Vec<bool> {
    let mut ordem: Vec<usize> = (0..lista.len()).collect();
    ordem.sort_by_key(|&i| lista[i].0);
    let mut marcas = vec![false; lista.len()];
    let mut maior: Option<u64> = None;
    for i in ordem {
        if let Some(g) = lista[i].1 {
            marcas[i] = maior.is_some_and(|m| g < m);
            maior = Some(maior.map_or(g, |m| m.max(g)));
        }
    }
    marcas
}

/// A geração selada no corpo de `arquivo` (Fase 49a), se ele foi selado pela chave do
/// `cofre` aberto: autenticada pela AEAD, então não dá para forjar sem a chave. `None`
/// quando é de outra chave (outra senha mestra, de antes de uma troca) ou não abre. O
/// conteúdo decifrado zera ao sair; só o número sai daqui.
pub fn geracao_selada(cofre: &CofreAberto, arquivo: &[u8]) -> Option<u64> {
    cofre.geracao_selada(arquivo)
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
    pasta.guardar(arquivo, instante, cofre)
}

/// O nome de uma versão é válido? (Para o protocolo conferir o pedido.)
pub fn nome_valido(nome: &str) -> bool {
    Versao::de_nome(nome).is_some()
}

#[cfg(test)]
mod testes;
