//! Gravação atômica do arquivo do cofre (S7): temporário no mesmo diretório → fsync do
//! temporário → rename por cima do destino → fsync do diretório.
//!
//! Uma gravação interrompida em qualquer ponto (processo morto, disco cheio, falha de
//! fsync) deixa no destino a versão anterior inteira ou a nova inteira, nunca uma
//! mistura. O temporário é criado com `O_CREAT|O_EXCL` (não segue link simbólico nem
//! reaproveita arquivo existente), nome aleatório e modo 0600.
//!
//! Só bytes que já são um cofre cifrado entram aqui (S2). As únicas gravações públicas
//! são [`gravar_entradas`] (o caminho do daemon) e [`trocar_senha`] (D27, D44): as duas
//! partem de um [`CofreAberto`] ou da senha atual, serializam ou validam o corpo (com o
//! hash do PIN, D30) e selam uma geração que nunca foi selada (D39, D53). Selar e gravar
//! bytes soltos ([`CofreAberto::selar`], `gravar_cofre`, `gravar`) é interno ao crate:
//! `gravar` só confere a estrutura do cabeçalho, não a autenticidade (não tem a chave),
//! e `gravar_cofre` selaria qualquer corpo, inclusive um sem hash de PIN. Nenhum caminho
//! deste módulo escreve conteúdo em claro.
//!
//! De fora do `ward-core`, gravar bytes soltos não compila (V2.34):
//!
//! ```compile_fail,E0603
//! let mut bytes = Vec::from(*b"WARDCOFR");
//! bytes.resize(160 + 16, 0);
//! let _ = ward_core::gravacao::gravar(std::path::Path::new("cofre.ward"), &bytes);
//! ```
//!
//! Nem selar e gravar um corpo qualquer pelo cofre aberto (V2.50, D44):
//!
//! ```compile_fail,E0603
//! use ward_core::configuracao::{Configuracao, HashPin};
//! use ward_core::formato::{CofreAberto, SenhaMestra};
//!
//! let senha = SenhaMestra::nova("falso-mestra-ward-01".to_owned());
//! let configuracao = Configuracao::nova(HashPin::criar(b"135790").unwrap());
//! let cofre = CofreAberto::novo(&senha, configuracao).unwrap();
//! // Corpo sem hash de PIN: só a versão e zero entradas.
//! let corpo = [3u8, 0, 0, 0, 0, 0];
//! let _ = ward_core::gravacao::gravar_cofre(std::path::Path::new("cofre.ward"), &cofre, &corpo);
//! ```
//!
//! O caminho público:
//!
//! ```no_run
//! use ward_core::configuracao::{Configuracao, HashPin};
//! use ward_core::formato::{CofreAberto, SenhaMestra};
//! use ward_core::gravacao::gravar_entradas;
//!
//! let senha = SenhaMestra::nova("falso-mestra-ward-01".to_owned());
//! let configuracao = Configuracao::nova(HashPin::criar(b"135790").unwrap());
//! let mut cofre = CofreAberto::novo(&senha, configuracao).unwrap();
//! gravar_entradas(std::path::Path::new("cofre.ward"), &mut cofre, &[]).unwrap();
//! assert_eq!(cofre.geracao(), 1);
//! ```
//!
//! Identidade (D62): [`gravar_entradas`] e [`trocar_senha`] devolvem a
//! [`IdentidadeArquivo`] lida por `fstat` do fd do temporário depois do fsync e antes do
//! rename (o rename não muda dev, inode, `mtime` nem tamanho). Quem chama nunca precisa
//! de um `stat` do caminho depois do rename, que pegaria um arquivo trocado por outro
//! processo nesse meio-tempo.

use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::corpo::{self, ErroCorpo};
use crate::cripto;
use crate::entrada::Entrada;
use crate::formato::{self, CofreAberto, ErroFormato, SenhaMestra};

/// Sufixo dos temporários: `.<nome do destino>.ward-tmp-<32 hex>`.
const MARCA_TEMPORARIO: &str = ".ward-tmp-";
/// Modo do cofre e do temporário.
const MODO: u32 = 0o600;
/// Tamanho de cada `write` (e de cada ponto de teste no meio da escrita).
const PEDACO: usize = 1024 * 1024;

/// Identidade de um arquivo (D39, D62): dispositivo, inode, `mtime` com nanossegundos e
/// tamanho, lidos por `fstat`. Só números; nenhum conteúdo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentidadeArquivo {
    dev: u64,
    ino: u64,
    mtime: i64,
    mtime_nsec: i64,
    tamanho: u64,
}

impl From<&fs::Metadata> for IdentidadeArquivo {
    fn from(m: &fs::Metadata) -> Self {
        Self {
            dev: m.dev(),
            ino: m.ino(),
            mtime: m.mtime(),
            mtime_nsec: m.mtime_nsec(),
            tamanho: m.len(),
        }
    }
}

/// Etapa da gravação em que a falha aconteceu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Etapa {
    /// Ler o cofre atual (troca de senha).
    Ler,
    /// Criar o temporário no diretório do destino.
    CriarTemporario,
    /// Escrever o temporário.
    Escrever,
    /// fsync do temporário.
    FsyncTemporario,
    /// rename do temporário por cima do destino.
    Renomear,
    /// fsync do diretório (o rename já aconteceu; a versão nova está no lugar, mas
    /// ainda pode não ter chegado ao disco).
    FsyncDiretorio,
}

impl Etapa {
    fn nome(self) -> &'static str {
        match self {
            Self::Ler => "ler o arquivo do cofre",
            Self::CriarTemporario => "criar o arquivo temporário",
            Self::Escrever => "escrever o arquivo temporário",
            Self::FsyncTemporario => "sincronizar o arquivo temporário com o disco",
            Self::Renomear => "trocar o cofre pelo arquivo novo",
            Self::FsyncDiretorio => "sincronizar o diretório do cofre com o disco",
        }
    }
}

/// Erro da gravação. Não carrega conteúdo do cofre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroGravacao {
    /// O cofre já está na maior geração possível (`u64::MAX`, D39): nada foi escrito.
    GeracaoEsgotada,
    /// Troca de senha: o arquivo no disco tem geração menor que a mínima pedida
    /// ([`trocar_senha`], D39). Nada foi escrito.
    Retrocesso,
    /// Os bytes não são um cofre cifrado bem formado (nada foi escrito).
    NaoECofre,
    /// Falha ao selar o conteúdo, ou ao abrir o cofre na troca de senha (nada foi
    /// escrito).
    Formato(ErroFormato),
    /// As entradas não viram um corpo gravável (grande demais ou inconsistentes; nada
    /// foi escrito).
    Corpo(ErroCorpo),
    /// O destino não tem nome de arquivo (`/`, `..`, vazio).
    DestinoInvalido,
    /// Falha de E/S numa etapa. Antes de [`Etapa::FsyncDiretorio`], o destino continua
    /// com a versão anterior e o temporário foi apagado.
    Io {
        /// Etapa que falhou.
        etapa: Etapa,
        /// Tipo do erro do sistema.
        tipo: io::ErrorKind,
        /// `Some` só em [`Etapa::FsyncDiretorio`]: o rename aconteceu, e esta é a
        /// identidade do arquivo que o Ward pôs no lugar, lida do fd do temporário
        /// (D62). Nas outras etapas, `None`.
        identidade: Option<IdentidadeArquivo>,
    },
}

impl fmt::Display for ErroGravacao {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GeracaoEsgotada => {
                f.write_str("recusado gravar: o contador de versões do cofre chegou ao fim")
            }
            Self::Retrocesso => f.write_str(
                "recusado trocar a senha: o cofre no disco é mais antigo que o último aberto",
            ),
            Self::NaoECofre => f.write_str("recusado gravar: os dados não são um cofre cifrado"),
            Self::Formato(e) => write!(f, "falha ao cifrar o cofre para gravar: {e}"),
            Self::Corpo(e) => write!(f, "recusado gravar: {e}"),
            Self::DestinoInvalido => f.write_str("caminho do cofre inválido"),
            Self::Io { etapa, tipo, .. } => {
                write!(f, "falha ao {} ({tipo})", etapa.nome())
            }
        }
    }
}

impl std::error::Error for ErroGravacao {}

/// Ponto da gravação em que o teste pode injetar falha ou parar o processo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ponto {
    DepoisDeCriar,
    /// Depois de cada pedaço escrito, com o total escrito até ali.
    Escrevendo(usize),
    DepoisDeEscrever,
    DepoisDoFsync,
    DepoisDoRename,
    DepoisDoFsyncDir,
}

/// Serializa a configuração do cofre aberto, a geração seguinte (D39) e as entradas
/// (D26, D30), sela com a chave do cofre aberto e grava de forma atômica. O corpo em
/// claro zera ao sair.
///
/// Geração queimada (D53): assim que o corpo é selado, o `cofre` passa à geração nova,
/// antes de escrever o temporário, e fica nela se qualquer passo seguinte falhar
/// (escrita, fsync, rename, fsync do diretório). A gravação seguinte sela outra, nunca a
/// mesma. Em `u64::MAX`, recusa com [`ErroGravacao::GeracaoEsgotada`] antes de selar,
/// sem tocar no disco e sem queimar; um corpo que não serializa também não queima.
///
/// Devolve a identidade do arquivo gravado, lida do fd do temporário (D62).
pub fn gravar_entradas(
    destino: &Path,
    cofre: &mut CofreAberto,
    entradas: &[Entrada],
) -> Result<IdentidadeArquivo, ErroGravacao> {
    gravar_entradas_com_pontos(destino, cofre, entradas, &mut |_| Ok(()))
}

fn gravar_entradas_com_pontos(
    destino: &Path,
    cofre: &mut CofreAberto,
    entradas: &[Entrada],
    ponto: &mut dyn FnMut(Ponto) -> io::Result<()>,
) -> Result<IdentidadeArquivo, ErroGravacao> {
    let geracao = cofre
        .proxima_geracao()
        .ok_or(ErroGravacao::GeracaoEsgotada)?;
    let conteudo =
        corpo::serializar(cofre.configuracao(), geracao, entradas).map_err(ErroGravacao::Corpo)?;
    let arquivo = cofre.selar(&conteudo).map_err(ErroGravacao::Formato)?;
    drop(conteudo);
    // D53: a geração está selada; daqui em diante ela não volta a ser usada.
    cofre.queimar(geracao);
    gravar_com_pontos(destino, &arquivo, ponto)
}

/// Troca a senha mestra do cofre em `destino` (V2.8): lê o arquivo, exige a senha
/// atual, confere que o corpo é válido e lê a configuração e as entradas dele (D30),
/// sela de novo com sal, chave de dados e nonces novos e grava de forma atômica.
/// Devolve o cofre aberto com a senha nova, a mesma configuração e a geração nova.
///
/// `geracao_minima` (D39): um arquivo com geração menor (trocado por uma versão
/// autêntica mais antiga) é recusado com [`ErroGravacao::Retrocesso`]. O daemon passa
/// a maior geração confirmada no disco; 0 aceita qualquer uma.
///
/// `geracao_queimada` (D53): a maior geração já selada ou tentada que o chamador
/// conhece. A troca sela max(geração do arquivo, `geracao_queimada`) + 1, então nunca
/// repete um número que uma gravação que falhou já selou. Corpo inválido, retrocesso
/// ou geração esgotada: nada é gravado. Um erro depois de selar perde o cofre aberto
/// devolvido; o chamador trava e reabre com a geração queimada que guardou.
///
/// Devolve também a identidade do arquivo gravado (D62, como em [`gravar_entradas`]).
pub fn trocar_senha(
    destino: &Path,
    atual: &SenhaMestra,
    nova: &SenhaMestra,
    geracao_minima: u64,
    geracao_queimada: u64,
) -> Result<(CofreAberto, IdentidadeArquivo), ErroGravacao> {
    let arquivo = ler_arquivo(destino)?;
    let (chaves, conteudo) =
        formato::trocar_senha(&arquivo, atual, nova).map_err(ErroGravacao::Formato)?;
    let (configuracao, geracao, entradas) =
        corpo::desserializar(&conteudo).map_err(ErroGravacao::Corpo)?;
    drop(conteudo);
    if geracao < geracao_minima {
        return Err(ErroGravacao::Retrocesso);
    }
    let mut cofre = CofreAberto::de_partes(chaves, configuracao, geracao.max(geracao_queimada));
    let identidade = gravar_entradas(destino, &mut cofre, &entradas)?;
    Ok((cofre, identidade))
}

/// Lê o arquivo do cofre (já cifrado) sem passar do maior tamanho possível de um cofre.
fn ler_arquivo(destino: &Path) -> Result<Vec<u8>, ErroGravacao> {
    let limite = (formato::TAM_CABECALHO + formato::MAX_CONTEUDO + cripto::TAM_TAG) as u64;
    let f = File::open(destino).map_err(erro(Etapa::Ler))?;
    let mut arquivo = Vec::new();
    io::Read::read_to_end(&mut io::Read::take(f, limite + 1), &mut arquivo)
        .map_err(erro(Etapa::Ler))?;
    if arquivo.len() as u64 > limite {
        return Err(ErroGravacao::Formato(ErroFormato::SenhaOuArquivo));
    }
    Ok(arquivo)
}

/// Sela `conteudo` com a chave do cofre aberto e grava o arquivo de forma atômica. Só
/// dentro do crate (D44): selaria qualquer corpo, inclusive um sem hash de PIN, e não
/// queima a geração (D53). De fora, o caminho é [`gravar_entradas`]. Hoje só os testes do
/// crate chamam direto; o backup (Fase 45) vai usar.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn gravar_cofre(
    destino: &Path,
    cofre: &CofreAberto,
    conteudo: &[u8],
) -> Result<IdentidadeArquivo, ErroGravacao> {
    gravar_cofre_com_pontos(destino, cofre, conteudo, &mut |_| Ok(()))
}

fn gravar_cofre_com_pontos(
    destino: &Path,
    cofre: &CofreAberto,
    conteudo: &[u8],
    ponto: &mut dyn FnMut(Ponto) -> io::Result<()>,
) -> Result<IdentidadeArquivo, ErroGravacao> {
    let arquivo = cofre.selar(conteudo).map_err(ErroGravacao::Formato)?;
    gravar_com_pontos(destino, &arquivo, ponto)
}

/// Grava de forma atômica um arquivo de cofre já cifrado (de
/// [`CofreAberto::selar`] ou [`formato::trocar_senha`]). Recusa, sem tocar no disco,
/// bytes que não tenham a estrutura de um cofre.
///
/// Só dentro do crate (D27): confere a estrutura, não a autenticidade. O backup
/// (Fase 45) também usa esta função por dentro do `ward-core`; hoje, só os testes.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn gravar(destino: &Path, arquivo: &[u8]) -> Result<IdentidadeArquivo, ErroGravacao> {
    gravar_com_pontos(destino, arquivo, &mut |_| Ok(()))
}

/// Apaga os temporários que uma gravação interrompida deixou ao lado de `destino` (só
/// arquivos comuns com o prefixo do destino; link simbólico e diretório ficam). O
/// daemon chama ao abrir o cofre, antes da primeira gravação. Devolve quantos apagou.
pub fn limpar_temporarios(destino: &Path) -> io::Result<usize> {
    let (dir, nome) = separar(destino).ok_or(io::ErrorKind::InvalidInput)?;
    let mut prefixo = OsString::from(".");
    prefixo.push(nome);
    prefixo.push(MARCA_TEMPORARIO);
    let prefixo = prefixo.as_encoded_bytes().to_vec();
    let mut apagados = 0;
    for item in fs::read_dir(&dir)? {
        let item = item?;
        if !item.file_name().as_encoded_bytes().starts_with(&prefixo) {
            continue;
        }
        // `DirEntry::file_type` não segue link simbólico.
        if item.file_type()?.is_file() {
            fs::remove_file(item.path())?;
            apagados += 1;
        }
    }
    Ok(apagados)
}

/// Diretório (`.` quando o destino não tem um) e nome do arquivo.
fn separar(destino: &Path) -> Option<(PathBuf, OsString)> {
    let nome = destino.file_name()?.to_os_string();
    let dir = match destino.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    Some((dir, nome))
}

fn erro(etapa: Etapa) -> impl FnOnce(io::Error) -> ErroGravacao {
    move |e| ErroGravacao::Io {
        etapa,
        tipo: e.kind(),
        identidade: None,
    }
}

/// Cria o temporário: `O_WRONLY|O_CREAT|O_EXCL|O_CLOEXEC`, modo 0600. Com `O_EXCL`, um
/// link simbólico ou arquivo já existente com o mesmo nome faz a criação falhar.
fn criar_temporario(caminho: &Path) -> io::Result<File> {
    let f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(MODO)
        .open(caminho)?;
    // O umask só tira bits; mesmo assim, fixa 0600 no próprio descritor.
    f.set_permissions(fs::Permissions::from_mode(MODO))?;
    Ok(f)
}

/// Apaga o temporário quando a gravação não chega ao rename.
struct Temporario<'a> {
    caminho: &'a Path,
    ativo: bool,
}

impl Drop for Temporario<'_> {
    fn drop(&mut self) {
        if self.ativo {
            let _ = fs::remove_file(self.caminho);
        }
    }
}

fn gravar_com_pontos(
    destino: &Path,
    arquivo: &[u8],
    ponto: &mut dyn FnMut(Ponto) -> io::Result<()>,
) -> Result<IdentidadeArquivo, ErroGravacao> {
    // S2: só um cofre cifrado bem formado vai para o disco.
    formato::ler_cabecalho(arquivo).map_err(|_| ErroGravacao::NaoECofre)?;
    let (dir, nome) = separar(destino).ok_or(ErroGravacao::DestinoInvalido)?;

    let mut aleatorio = [0u8; 16];
    cripto::aleatorio(&mut aleatorio).map_err(|_| ErroGravacao::Io {
        etapa: Etapa::CriarTemporario,
        tipo: io::ErrorKind::Other,
        identidade: None,
    })?;
    let mut nome_tmp = OsString::from(".");
    nome_tmp.push(&nome);
    nome_tmp.push(MARCA_TEMPORARIO);
    for b in aleatorio {
        nome_tmp.push(format!("{b:02x}"));
    }
    let caminho_tmp = dir.join(nome_tmp);

    let mut f = criar_temporario(&caminho_tmp).map_err(erro(Etapa::CriarTemporario))?;
    let mut tmp = Temporario {
        caminho: &caminho_tmp,
        ativo: true,
    };
    ponto(Ponto::DepoisDeCriar).map_err(erro(Etapa::CriarTemporario))?;

    let mut escritos = 0;
    for pedaco in arquivo.chunks(PEDACO) {
        f.write_all(pedaco).map_err(erro(Etapa::Escrever))?;
        escritos += pedaco.len();
        ponto(Ponto::Escrevendo(escritos)).map_err(erro(Etapa::Escrever))?;
    }
    ponto(Ponto::DepoisDeEscrever).map_err(erro(Etapa::Escrever))?;

    // `sync_all` é fsync(2): dados e metadados do temporário no disco antes do rename.
    f.sync_all().map_err(erro(Etapa::FsyncTemporario))?;
    ponto(Ponto::DepoisDoFsync).map_err(erro(Etapa::FsyncTemporario))?;
    // D62: a identidade sai do `fstat` do fd do temporário, já sincronizado. O rename
    // não muda dev, inode, `mtime` nem tamanho; depois dele, nada mais é lido pelo
    // caminho (outro processo pode trocar o arquivo logo depois do rename).
    let identidade = f
        .metadata()
        .map(|m| IdentidadeArquivo::from(&m))
        .map_err(erro(Etapa::FsyncTemporario))?;
    drop(f);

    fs::rename(&caminho_tmp, destino).map_err(erro(Etapa::Renomear))?;
    tmp.ativo = false;
    let depois_do_rename = move |e: io::Error| ErroGravacao::Io {
        etapa: Etapa::FsyncDiretorio,
        tipo: e.kind(),
        identidade: Some(identidade),
    };
    ponto(Ponto::DepoisDoRename).map_err(depois_do_rename)?;

    // fsync do diretório: a troca de nome também chega ao disco.
    File::open(&dir)
        .and_then(|d| d.sync_all())
        .map_err(depois_do_rename)?;
    ponto(Ponto::DepoisDoFsyncDir).map_err(depois_do_rename)?;
    Ok(identidade)
}

#[cfg(test)]
mod testes;
