//! Formato do arquivo do cofre (S6).
//!
//! ```text
//! offset tam  campo
//!      0   8  magia "WARDCOFR"
//!      8   2  versão do formato (u16 LE) = 1
//!     10   1  KDF  = 1 (Argon2id, versão 0x13)
//!     11   1  AEAD = 1 (XChaCha20-Poly1305)
//!     12   4  memória do Argon2id em KiB (u32 LE)
//!     16   4  passes do Argon2id (u32 LE)
//!     20   4  paralelismo do Argon2id (u32 LE)
//!     24  32  sal
//!     56  24  nonce da chave embrulhada
//!     80  48  chave de dados embrulhada (32 + tag 16)
//!    128  24  nonce dos dados
//!    152   8  tamanho do corpo cifrado com a tag (u64 LE)
//!    160   …  corpo: conteúdo cifrado ‖ tag
//! ```
//!
//! - KEK = Argon2id(senha mestra, sal, parâmetros do cabeçalho).
//! - A chave de dados é aleatória (32 bytes do sistema) e vai embrulhada pela KEK com
//!   XChaCha20-Poly1305; o AAD do embrulho são os bytes 0..80 do cabeçalho.
//! - O corpo é cifrado pela chave de dados com XChaCha20-Poly1305; o AAD é o cabeçalho
//!   inteiro (0..160). Todo byte do arquivo é autenticado: os 80 primeiros pelas duas
//!   tags, os 80 seguintes pela tag do corpo, o corpo pela própria tag, e o tamanho
//!   total pelo campo de tamanho (que está no AAD).
//! - Parâmetros abaixo do mínimo (256 MiB, 3 passes) ou acima do máximo são recusados
//!   **antes** de derivar: um cabeçalho rebaixado não faz o Argon2 rodar barato, e um
//!   inflado não faz o processo alocar memória sem fim.
//! - Senha errada, arquivo adulterado, truncado ou acrescido dão o mesmo erro
//!   ([`ErroFormato::SenhaOuArquivo`]).
//!
//! Nada de cripto aqui: as primitivas estão em [`crate::cripto`], que só chama
//! bibliotecas auditadas.

use core::fmt;

use zeroize::{ZeroizeOnDrop, Zeroizing};

use crate::configuracao::Configuracao;
use crate::cripto::{self, Chave, TAM_CHAVE, TAM_NONCE, TAM_TAG};

/// Começo de todo arquivo de cofre do Ward.
pub const MAGIA: [u8; 8] = *b"WARDCOFR";
/// Versão atual do formato.
pub const VERSAO: u16 = 1;
/// Identificador do KDF no cabeçalho: Argon2id versão 0x13.
pub const KDF_ARGON2ID_13: u8 = 1;
/// Identificador do AEAD no cabeçalho: XChaCha20-Poly1305.
pub const AEAD_XCHACHA20POLY1305: u8 = 1;
/// Tamanho do sal, em bytes.
pub const TAM_SAL: usize = 32;
/// Tamanho do cabeçalho, em bytes.
pub const TAM_CABECALHO: usize = 160;
/// Maior conteúdo aceito (em claro), em bytes: 512 MiB.
pub const MAX_CONTEUDO: usize = 512 * 1024 * 1024;

const TAM_EMBRULHADA: usize = TAM_CHAVE + TAM_TAG;
// Posições dos campos (ver o desenho no topo do módulo).
const P_VERSAO: usize = 8;
const P_KDF: usize = 10;
const P_AEAD: usize = 11;
const P_MEMORIA: usize = 12;
const P_PASSES: usize = 16;
const P_PARALELISMO: usize = 20;
const P_SAL: usize = 24;
const P_NONCE_CHAVE: usize = P_SAL + TAM_SAL; // 56
const P_EMBRULHADA: usize = P_NONCE_CHAVE + TAM_NONCE; // 80
const P_NONCE_DADOS: usize = P_EMBRULHADA + TAM_EMBRULHADA; // 128
const P_TAMANHO: usize = P_NONCE_DADOS + TAM_NONCE; // 152
const _: () = assert!(P_TAMANHO + 8 == TAM_CABECALHO);

/// Parâmetros do Argon2id gravados no cabeçalho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParametrosKdf {
    /// Memória, em KiB.
    pub memoria_kib: u32,
    /// Passes (custo de tempo).
    pub passes: u32,
    /// Paralelismo (faixas).
    pub paralelismo: u32,
}

impl ParametrosKdf {
    /// Usados para criar cofre novo: 256 MiB, 3 passes, 4 faixas.
    pub const PADRAO: Self = Self {
        memoria_kib: 256 * 1024,
        passes: 3,
        paralelismo: 4,
    };
    /// Mínimo aceito na abertura e na criação (S6: ~256 MiB / 3 passes).
    pub const MINIMO: Self = Self {
        memoria_kib: 256 * 1024,
        passes: 3,
        paralelismo: 1,
    };
    /// Máximo aceito: 2 GiB, 16 passes, 16 faixas. Um cabeçalho com valor maior é
    /// recusado sem alocar nada.
    pub const MAXIMO: Self = Self {
        memoria_kib: 2 * 1024 * 1024,
        passes: 16,
        paralelismo: 16,
    };

    fn conferir(&self, minimo: &Self) -> Result<(), ErroFormato> {
        let max = Self::MAXIMO;
        if self.memoria_kib > max.memoria_kib
            || self.passes > max.passes
            || self.paralelismo > max.paralelismo
        {
            return Err(ErroFormato::ParametrosAcimaDoMaximo);
        }
        if self.memoria_kib < minimo.memoria_kib
            || self.passes < minimo.passes
            || self.paralelismo < minimo.paralelismo.max(1)
            // Exigência do Argon2: 8 blocos de 1 KiB por faixa.
            || u64::from(self.memoria_kib) < 8 * u64::from(self.paralelismo)
        {
            return Err(ErroFormato::ParametrosAbaixoDoMinimo);
        }
        Ok(())
    }
}

/// O que o cabeçalho declara. **Não autenticado**: serve para conferir os parâmetros;
/// só [`abrir`] prova que o arquivo é íntegro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cabecalho {
    /// Versão do formato.
    pub versao: u16,
    /// Parâmetros do Argon2id.
    pub kdf: ParametrosKdf,
    /// Tamanho do corpo cifrado (com a tag), em bytes.
    pub tamanho_corpo: u64,
}

/// Erro do formato. Nenhuma variante carrega dado do cofre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroFormato {
    /// Senha mestra errada, ou arquivo adulterado, truncado, acrescido ou que não é um
    /// cofre do Ward. Um erro só, de propósito: a resposta não diz qual dos casos foi.
    SenhaOuArquivo,
    /// Versão do formato que este Ward não conhece.
    VersaoNaoSuportada(u16),
    /// O cabeçalho declara Argon2id mais fraco que o mínimo.
    ParametrosAbaixoDoMinimo,
    /// O cabeçalho declara Argon2id acima do máximo aceito.
    ParametrosAcimaDoMaximo,
    /// Conteúdo maior que [`MAX_CONTEUDO`].
    ConteudoGrande,
    /// Senha mestra vazia.
    SenhaVazia,
    /// O sistema não entregou bytes aleatórios.
    Aleatorio,
    /// Falha interna ao cifrar (memória, por exemplo).
    Cifrar,
}

impl fmt::Display for ErroFormato {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SenhaOuArquivo => {
                f.write_str("senha mestra errada ou arquivo do cofre adulterado")
            }
            Self::VersaoNaoSuportada(v) => {
                write!(f, "versão {v} do formato do cofre não suportada")
            }
            Self::ParametrosAbaixoDoMinimo => f.write_str(
                "parâmetros do Argon2id abaixo do mínimo (256 MiB de memória e 3 passes)",
            ),
            Self::ParametrosAcimaDoMaximo => {
                f.write_str("parâmetros do Argon2id acima do máximo aceito")
            }
            Self::ConteudoGrande => f.write_str("conteúdo do cofre grande demais"),
            Self::SenhaVazia => f.write_str("a senha mestra não pode ser vazia"),
            Self::Aleatorio => f.write_str("o sistema não entregou bytes aleatórios"),
            Self::Cifrar => f.write_str("falha ao cifrar o cofre"),
        }
    }
}

impl std::error::Error for ErroFormato {}

/// Senha mestra em memória. Zera ao sair; sem `Clone`, e o `Debug` não mostra o valor.
pub struct SenhaMestra(Zeroizing<Vec<u8>>);

impl SenhaMestra {
    /// Toma posse do texto (sem cópia) e o zera ao sair.
    pub fn nova(texto: String) -> Self {
        Self(Zeroizing::new(texto.into_bytes()))
    }

    /// Toma posse dos bytes, que já zeram ao sair.
    pub fn de_bytes(bytes: Zeroizing<Vec<u8>>) -> Self {
        Self(bytes)
    }

    /// Senha mestra que veio de quem digitou (app, CLI, socket), já em NFC (Fase 39c).
    ///
    /// O mesmo texto digitado num método de entrada que compõe ("é" = U+00E9) e noutro
    /// que decompõe ("e" + U+0301) tem bytes diferentes; sem normalizar, a senha criada
    /// no app não abriria pelo `ward destravar`. Todo caminho do daemon passa por aqui,
    /// então o cliente não precisa normalizar. ASCII (o caso comum) já é NFC e não é
    /// copiado. Bytes que não são UTF-8 ficam como vieram (não há o que normalizar; a
    /// senha nova recusa, [`SenhaMestra::serve_de_nova`]).
    ///
    /// O texto normalizado vai para um buffer `Zeroizing` com a capacidade final
    /// reservada antes (o NFC cresce no máximo 3× em UTF-8), para não deixar cópia em
    /// realocação. Resíduo: o iterador do `unicode-normalization` guarda no heap, sem
    /// zerar, só sequências de mais de 4 marcas combinantes seguidas.
    pub fn digitada(bytes: Zeroizing<Vec<u8>>) -> Self {
        Self(nfc(bytes))
    }

    /// Serve de senha mestra nova? UTF-8 válido e [`MIN_CARACTERES_SENHA_MESTRA`]
    /// caracteres ou mais (pontos de código, depois do NFC). O tamanho máximo é o do
    /// protocolo (`MAX_SEGREDO`).
    ///
    /// [`MIN_CARACTERES_SENHA_MESTRA`]: crate::limites::MIN_CARACTERES_SENHA_MESTRA
    pub fn serve_de_nova(&self) -> bool {
        std::str::from_utf8(self.bytes())
            .is_ok_and(|t| t.chars().count() >= crate::limites::MIN_CARACTERES_SENHA_MESTRA)
    }

    fn bytes(&self) -> &[u8] {
        &self.0
    }
}

/// NFC dos bytes, se forem UTF-8 (ver [`SenhaMestra::digitada`]).
fn nfc(bytes: Zeroizing<Vec<u8>>) -> Zeroizing<Vec<u8>> {
    use unicode_normalization::{IsNormalized, UnicodeNormalization, is_nfc_quick};
    let Ok(texto) = std::str::from_utf8(&bytes) else {
        return bytes;
    };
    if texto.is_ascii() || is_nfc_quick(texto.chars()) == IsNormalized::Yes {
        return bytes;
    }
    let mut saida = Zeroizing::new(String::with_capacity(
        bytes.len().saturating_mul(3).saturating_add(4),
    ));
    for c in texto.nfc() {
        saida.push(c);
    }
    // `into_bytes` move o buffer (sem cópia); o `Zeroizing` de fora zera o novo.
    Zeroizing::new(std::mem::take(&mut *saida).into_bytes())
}

// O único campo é um `Zeroizing`: a senha zera ao sair (V2.28).
impl ZeroizeOnDrop for SenhaMestra {}

impl fmt::Debug for SenhaMestra {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SenhaMestra(••••)")
    }
}

/// Cofre destravado (D30): as chaves do arquivo, a configuração do cofre (hash do PIN,
/// tempo de trava, backup externo) e a geração (D39). Toda gravação das entradas
/// ([`crate::gravacao::gravar_entradas`]) leva esta configuração, então nenhum caminho
/// grava um corpo sem o hash do PIN. Sem `Clone`; o `Debug` não mostra a chave nem o
/// hash.
///
/// A geração (D39, D53) é o número da última versão lida ou selada: toda gravação sela
/// a seguinte e o `CofreAberto` passa a tê-la logo ao selar, antes de escrever o
/// temporário, e fica com ela mesmo que a escrita, o fsync, o rename ou o fsync do
/// diretório falhem (geração queimada). Assim, nenhum par de arquivos selados tem a
/// mesma geração. Ela pode ser lida
/// ([`CofreAberto::geracao`]), mas não existe setter público, nem pela configuração:
///
/// ```compile_fail,E0609
/// # use ward_core::configuracao::{Configuracao, HashPin};
/// # use ward_core::formato::{CofreAberto, SenhaMestra};
/// # let senha = SenhaMestra::nova("falso-mestra-ward-01".to_owned());
/// # let cfg = Configuracao::nova(HashPin::criar(b"135790").unwrap());
/// let mut cofre = CofreAberto::novo(&senha, cfg).unwrap();
/// cofre.configuracao_mut().geracao = 0; // não existe
/// ```
///
/// ```compile_fail,E0599
/// # use ward_core::configuracao::{Configuracao, HashPin};
/// # use ward_core::formato::{CofreAberto, SenhaMestra};
/// # let senha = SenhaMestra::nova("falso-mestra-ward-01".to_owned());
/// # let cfg = Configuracao::nova(HashPin::criar(b"135790").unwrap());
/// let mut cofre = CofreAberto::novo(&senha, cfg).unwrap();
/// cofre.configuracao_mut().definir_geracao(0); // não existe
/// ```
///
/// ```compile_fail,E0616
/// # use ward_core::configuracao::{Configuracao, HashPin};
/// # use ward_core::formato::{CofreAberto, SenhaMestra};
/// # let senha = SenhaMestra::nova("falso-mestra-ward-01".to_owned());
/// # let cfg = Configuracao::nova(HashPin::criar(b"135790").unwrap());
/// let mut cofre = CofreAberto::novo(&senha, cfg).unwrap();
/// cofre.geracao = 0; // campo privado
/// ```
///
/// Selar bytes soltos também não é público (D44): de fora, o caminho é
/// `gravar_entradas`, que serializa o corpo (com o hash do PIN) antes.
///
/// ```compile_fail,E0624
/// # use ward_core::configuracao::{Configuracao, HashPin};
/// # use ward_core::formato::{CofreAberto, SenhaMestra};
/// # let senha = SenhaMestra::nova("falso-mestra-ward-01".to_owned());
/// # let cfg = Configuracao::nova(HashPin::criar(b"135790").unwrap());
/// let cofre = CofreAberto::novo(&senha, cfg).unwrap();
/// let _ = cofre.selar(b"corpo sem hash de PIN");
/// ```
pub struct CofreAberto {
    chaves: ChavesCofre,
    configuracao: Configuracao,
    geracao: u64,
}

// A chave de dados e o hash do PIN zeram ao sair (V2.28).
impl ZeroizeOnDrop for CofreAberto {}

impl fmt::Debug for CofreAberto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CofreAberto")
            .field("chaves", &self.chaves)
            .field("configuracao", &self.configuracao)
            .field("geracao", &self.geracao)
            .finish()
    }
}

impl CofreAberto {
    /// Cofre novo com [`ParametrosKdf::PADRAO`]: sal, nonce e chave de dados novos, e a
    /// configuração dada (que já traz o hash do PIN). Ainda não gravado: geração 0, e a
    /// primeira gravação grava a 1 (D39).
    pub fn novo(senha: &SenhaMestra, configuracao: Configuracao) -> Result<Self, ErroFormato> {
        Self::novo_com(senha, ParametrosKdf::PADRAO, configuracao)
    }

    /// Cofre novo com os parâmetros dados (recusa abaixo do mínimo e acima do máximo).
    pub fn novo_com(
        senha: &SenhaMestra,
        kdf: ParametrosKdf,
        configuracao: Configuracao,
    ) -> Result<Self, ErroFormato> {
        Ok(Self {
            chaves: ChavesCofre::novo_com(senha, kdf)?,
            configuracao,
            geracao: 0,
        })
    }

    /// Cofre novo que já conhece a geração `queimada` (D62): a primeira gravação sela
    /// `queimada + 1`. Para o daemon que cria um cofre onde um arquivo de estado velho
    /// já queimou gerações: uma cópia antiga de um cofre anterior, posta no lugar depois,
    /// fica abaixo da `vista` e é recusada como retrocesso. Nunca desce a geração.
    pub fn depois_da_geracao(mut self, queimada: u64) -> Self {
        self.geracao = self.geracao.max(queimada);
        self
    }

    /// Junta as chaves de um arquivo aberto à configuração e à geração lidas do corpo
    /// dele.
    pub(crate) fn de_partes(chaves: ChavesCofre, configuracao: Configuracao, geracao: u64) -> Self {
        Self {
            chaves,
            configuracao,
            geracao,
        }
    }

    /// Geração deste cofre (D39, D53): a última lida ou selada. Ao abrir, é a maior
    /// entre a do corpo e a geração queimada que o chamador conhece; depois de cada
    /// gravação, a que ela selou, tenha a escrita dado certo ou não. 0 num cofre novo
    /// ainda não gravado. Só leitura.
    pub fn geracao(&self) -> u64 {
        self.geracao
    }

    /// A geração que a próxima gravação grava. `None` em `u64::MAX`: a gravação é
    /// recusada antes de tocar no disco (D39).
    pub(crate) fn proxima_geracao(&self) -> Option<u64> {
        self.geracao.checked_add(1)
    }

    /// Queima a geração `geracao` (D53): chamada logo depois de selar o corpo com ela,
    /// antes de escrever o temporário. O contador nunca desce.
    pub(crate) fn queimar(&mut self, geracao: u64) {
        debug_assert!(geracao > self.geracao);
        self.geracao = self.geracao.max(geracao);
    }

    /// Parâmetros do Argon2id deste cofre.
    pub fn parametros(&self) -> ParametrosKdf {
        self.chaves.kdf
    }

    /// Configuração do cofre (D30).
    pub fn configuracao(&self) -> &Configuracao {
        &self.configuracao
    }

    /// Configuração do cofre, para mudar (PIN, tempo de trava, backup). Vai para o disco
    /// na próxima gravação das entradas. A geração não fica na configuração e não muda
    /// por aqui (D39).
    pub fn configuracao_mut(&mut self) -> &mut Configuracao {
        &mut self.configuracao
    }

    /// Cifra `conteudo` e devolve o arquivo inteiro. Cada chamada usa um nonce de dados
    /// novo; sal, embrulho e chave de dados ficam os mesmos. Só dentro do crate (D44):
    /// selaria qualquer corpo, inclusive um sem hash de PIN.
    pub(crate) fn selar(&self, conteudo: &[u8]) -> Result<Vec<u8>, ErroFormato> {
        self.chaves.selar(conteudo)
    }

    /// O arquivo foi selado por este cofre (mesmo sal, mesmo embrulho da chave de dados,
    /// e o corpo abre com a chave de dados)? Sem Argon2. O conteúdo decifrado zera ao
    /// sair. Para o backup do cofre atual (D17, Fase 45): só um arquivo autêntico vai
    /// para a pasta de backups.
    pub(crate) fn autentica(&self, arquivo: &[u8]) -> bool {
        self.chaves.abrir_proprio(arquivo).is_ok()
    }

    /// A geração selada no corpo de `arquivo`, se ele foi selado por este cofre (mesma
    /// chave de dados) e o corpo é válido inteiro. Sem Argon2; o conteúdo decifrado zera
    /// ao sair. Para a lista do restaurar (Fase 49a): a geração é autenticada, a data do
    /// nome do arquivo não.
    pub(crate) fn geracao_selada(&self, arquivo: &[u8]) -> Option<u64> {
        let conteudo = self.chaves.abrir_proprio(arquivo).ok()?;
        crate::corpo::geracao_do_corpo(&conteudo)
    }
}

/// Chaves de um arquivo de cofre: a chave de dados (que zera ao sair) e o embrulho dela,
/// para regravar sem rodar o Argon2 de novo. Só dentro do crate: de fora, o caminho é o
/// [`CofreAberto`], que também leva a configuração.
pub(crate) struct ChavesCofre {
    kdf: ParametrosKdf,
    sal: [u8; TAM_SAL],
    nonce_chave: [u8; TAM_NONCE],
    embrulhada: [u8; TAM_EMBRULHADA],
    chave_dados: Chave,
}

// A chave de dados é `Zeroizing` e zera ao sair; sal, nonce e embrulho não são segredo
// (estão no cabeçalho do arquivo) (V2.28).
impl ZeroizeOnDrop for ChavesCofre {}

impl fmt::Debug for ChavesCofre {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChavesCofre")
            .field("kdf", &self.kdf)
            .field("chave_dados", &"••••")
            .finish_non_exhaustive()
    }
}

impl ChavesCofre {
    /// Chaves novas com os parâmetros dados (recusa abaixo do mínimo e acima do máximo).
    pub(crate) fn novo_com(senha: &SenhaMestra, kdf: ParametrosKdf) -> Result<Self, ErroFormato> {
        Self::novo_interno(senha, kdf, &ParametrosKdf::MINIMO)
    }

    fn novo_interno(
        senha: &SenhaMestra,
        kdf: ParametrosKdf,
        minimo: &ParametrosKdf,
    ) -> Result<Self, ErroFormato> {
        if senha.bytes().is_empty() {
            return Err(ErroFormato::SenhaVazia);
        }
        kdf.conferir(minimo)?;
        let mut sal = [0u8; TAM_SAL];
        let mut nonce_chave = [0u8; TAM_NONCE];
        cripto::aleatorio(&mut sal).map_err(|_| ErroFormato::Aleatorio)?;
        cripto::aleatorio(&mut nonce_chave).map_err(|_| ErroFormato::Aleatorio)?;
        let chave_dados = cripto::chave_aleatoria().map_err(|_| ErroFormato::Aleatorio)?;
        let kek =
            cripto::derivar_kek(senha.bytes(), &sal, &kdf).map_err(|_| ErroFormato::Cifrar)?;
        let prefixo = prefixo_cabecalho(&kdf, &sal, &nonce_chave);
        let embrulhada = cripto::selar(&kek, &nonce_chave, &prefixo, &chave_dados[..])
            .map_err(|_| ErroFormato::Cifrar)?;
        let embrulhada: [u8; TAM_EMBRULHADA] =
            embrulhada.try_into().map_err(|_| ErroFormato::Cifrar)?;
        Ok(Self {
            kdf,
            sal,
            nonce_chave,
            embrulhada,
            chave_dados,
        })
    }

    /// Parâmetros do Argon2id destas chaves.
    pub(crate) fn parametros(&self) -> ParametrosKdf {
        self.kdf
    }

    /// Cifra `conteudo` e devolve o arquivo inteiro (ver [`CofreAberto::selar`]).
    pub(crate) fn selar(&self, conteudo: &[u8]) -> Result<Vec<u8>, ErroFormato> {
        if conteudo.len() > MAX_CONTEUDO {
            return Err(ErroFormato::ConteudoGrande);
        }
        let mut nonce_dados = [0u8; TAM_NONCE];
        cripto::aleatorio(&mut nonce_dados).map_err(|_| ErroFormato::Aleatorio)?;
        let tamanho_corpo = (conteudo.len() + TAM_TAG) as u64;

        let mut cabecalho = [0u8; TAM_CABECALHO];
        cabecalho[..P_EMBRULHADA].copy_from_slice(&prefixo_cabecalho(
            &self.kdf,
            &self.sal,
            &self.nonce_chave,
        ));
        cabecalho[P_EMBRULHADA..P_NONCE_DADOS].copy_from_slice(&self.embrulhada);
        cabecalho[P_NONCE_DADOS..P_TAMANHO].copy_from_slice(&nonce_dados);
        cabecalho[P_TAMANHO..].copy_from_slice(&tamanho_corpo.to_le_bytes());

        let corpo = cripto::selar(&self.chave_dados, &nonce_dados, &cabecalho, conteudo)
            .map_err(|_| ErroFormato::Cifrar)?;
        let mut arquivo = Vec::new();
        arquivo
            .try_reserve_exact(TAM_CABECALHO + corpo.len())
            .map_err(|_| ErroFormato::Cifrar)?;
        arquivo.extend_from_slice(&cabecalho);
        arquivo.extend_from_slice(&corpo);
        Ok(arquivo)
    }

    /// Abre um arquivo selado por estas chaves: estrutura, os 128 primeiros bytes do
    /// cabeçalho iguais aos destas chaves (parâmetros, sal, nonce e chave embrulhada; são
    /// públicos, a comparação não precisa de tempo constante) e o corpo pela AEAD com a
    /// chave de dados. Qualquer diferença dá [`ErroFormato::SenhaOuArquivo`].
    fn abrir_proprio(&self, arquivo: &[u8]) -> Result<Zeroizing<Vec<u8>>, ErroFormato> {
        ler_cabecalho(arquivo)?;
        let prefixo = prefixo_cabecalho(&self.kdf, &self.sal, &self.nonce_chave);
        if arquivo[..P_EMBRULHADA] != prefixo
            || arquivo[P_EMBRULHADA..P_NONCE_DADOS] != self.embrulhada
        {
            return Err(ErroFormato::SenhaOuArquivo);
        }
        let mut nonce_dados = [0u8; TAM_NONCE];
        nonce_dados.copy_from_slice(&arquivo[P_NONCE_DADOS..P_TAMANHO]);
        cripto::abrir(
            &self.chave_dados,
            &nonce_dados,
            &arquivo[..TAM_CABECALHO],
            &arquivo[TAM_CABECALHO..],
        )
        .map_err(|_| ErroFormato::SenhaOuArquivo)
    }
}

/// Bytes 0..80 do cabeçalho (o AAD do embrulho da chave).
fn prefixo_cabecalho(
    kdf: &ParametrosKdf,
    sal: &[u8; TAM_SAL],
    nonce_chave: &[u8; TAM_NONCE],
) -> [u8; P_EMBRULHADA] {
    let mut p = [0u8; P_EMBRULHADA];
    p[..P_VERSAO].copy_from_slice(&MAGIA);
    p[P_VERSAO..P_KDF].copy_from_slice(&VERSAO.to_le_bytes());
    p[P_KDF] = KDF_ARGON2ID_13;
    p[P_AEAD] = AEAD_XCHACHA20POLY1305;
    p[P_MEMORIA..P_PASSES].copy_from_slice(&kdf.memoria_kib.to_le_bytes());
    p[P_PASSES..P_PARALELISMO].copy_from_slice(&kdf.passes.to_le_bytes());
    p[P_PARALELISMO..P_SAL].copy_from_slice(&kdf.paralelismo.to_le_bytes());
    p[P_SAL..P_NONCE_CHAVE].copy_from_slice(sal);
    p[P_NONCE_CHAVE..].copy_from_slice(nonce_chave);
    p
}

fn u32_em(arquivo: &[u8], pos: usize) -> u32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&arquivo[pos..pos + 4]);
    u32::from_le_bytes(b)
}

/// Lê e confere a estrutura do cabeçalho (sem senha e sem autenticar): magia, versão,
/// algoritmos e tamanho exato do arquivo.
pub fn ler_cabecalho(arquivo: &[u8]) -> Result<Cabecalho, ErroFormato> {
    if arquivo.len() < TAM_CABECALHO || arquivo[..P_VERSAO] != MAGIA {
        return Err(ErroFormato::SenhaOuArquivo);
    }
    let versao = u16::from_le_bytes([arquivo[P_VERSAO], arquivo[P_VERSAO + 1]]);
    if versao != VERSAO {
        return Err(ErroFormato::VersaoNaoSuportada(versao));
    }
    if arquivo[P_KDF] != KDF_ARGON2ID_13 || arquivo[P_AEAD] != AEAD_XCHACHA20POLY1305 {
        return Err(ErroFormato::SenhaOuArquivo);
    }
    let mut t = [0u8; 8];
    t.copy_from_slice(&arquivo[P_TAMANHO..TAM_CABECALHO]);
    let tamanho_corpo = u64::from_le_bytes(t);
    let real = (arquivo.len() - TAM_CABECALHO) as u64;
    if tamanho_corpo != real
        || tamanho_corpo < TAM_TAG as u64
        || tamanho_corpo > (MAX_CONTEUDO + TAM_TAG) as u64
    {
        return Err(ErroFormato::SenhaOuArquivo);
    }
    Ok(Cabecalho {
        versao,
        kdf: ParametrosKdf {
            memoria_kib: u32_em(arquivo, P_MEMORIA),
            passes: u32_em(arquivo, P_PASSES),
            paralelismo: u32_em(arquivo, P_PARALELISMO),
        },
        tamanho_corpo,
    })
}

/// Abre o cofre: confere estrutura e parâmetros, deriva a KEK, desembrulha a chave de
/// dados e decifra o conteúdo. O conteúdo em claro zera ao sair. Só dentro do crate: de
/// fora, o caminho é [`crate::corpo::abrir_entradas`], que lê o corpo e a configuração.
pub(crate) fn abrir(
    arquivo: &[u8],
    senha: &SenhaMestra,
) -> Result<(ChavesCofre, Zeroizing<Vec<u8>>), ErroFormato> {
    abrir_com_minimo(arquivo, senha, &ParametrosKdf::MINIMO)
}

fn abrir_com_minimo(
    arquivo: &[u8],
    senha: &SenhaMestra,
    minimo: &ParametrosKdf,
) -> Result<(ChavesCofre, Zeroizing<Vec<u8>>), ErroFormato> {
    let cab = ler_cabecalho(arquivo)?;
    cab.kdf.conferir(minimo)?;

    let mut sal = [0u8; TAM_SAL];
    sal.copy_from_slice(&arquivo[P_SAL..P_NONCE_CHAVE]);
    let mut nonce_chave = [0u8; TAM_NONCE];
    nonce_chave.copy_from_slice(&arquivo[P_NONCE_CHAVE..P_EMBRULHADA]);
    let mut embrulhada = [0u8; TAM_EMBRULHADA];
    embrulhada.copy_from_slice(&arquivo[P_EMBRULHADA..P_NONCE_DADOS]);
    let mut nonce_dados = [0u8; TAM_NONCE];
    nonce_dados.copy_from_slice(&arquivo[P_NONCE_DADOS..P_TAMANHO]);

    let kek = cripto::derivar_kek(senha.bytes(), &sal, &cab.kdf)
        .map_err(|_| ErroFormato::SenhaOuArquivo)?;
    let chave = cripto::abrir(&kek, &nonce_chave, &arquivo[..P_EMBRULHADA], &embrulhada)
        .map_err(|_| ErroFormato::SenhaOuArquivo)?;
    drop(kek);
    // Direto do buffer decifrado (que zera) para o heap (D40).
    let chave_dados = Chave::de_bytes(&chave).map_err(|_| ErroFormato::SenhaOuArquivo)?;
    drop(chave);

    let conteudo = cripto::abrir(
        &chave_dados,
        &nonce_dados,
        &arquivo[..TAM_CABECALHO],
        &arquivo[TAM_CABECALHO..],
    )
    .map_err(|_| ErroFormato::SenhaOuArquivo)?;

    Ok((
        ChavesCofre {
            kdf: cab.kdf,
            sal,
            nonce_chave,
            embrulhada,
            chave_dados,
        },
        conteudo,
    ))
}

/// Resultado da troca de senha: as chaves novas e o conteúdo em claro (que zera ao
/// sair).
pub(crate) type SenhaTrocada = (ChavesCofre, Zeroizing<Vec<u8>>);

/// Troca a senha mestra. Exige a senha atual. Devolve chaves novas (sal, chave de dados
/// e nonce do embrulho novos: quem tem a senha antiga e uma cópia antiga não abre o
/// novo), com os mesmos parâmetros do cofre ou os [`ParametrosKdf::PADRAO`], o que for
/// maior, e o conteúdo atual. Não sela: quem chama ([`crate::gravacao::trocar_senha`])
/// lê o corpo, grava a geração seguinte (D39) e sela com as chaves novas.
pub(crate) fn trocar_senha(
    arquivo: &[u8],
    atual: &SenhaMestra,
    nova: &SenhaMestra,
) -> Result<SenhaTrocada, ErroFormato> {
    if nova.bytes().is_empty() {
        return Err(ErroFormato::SenhaVazia);
    }
    let (antigo, conteudo) = abrir(arquivo, atual)?;
    let p = antigo.parametros();
    drop(antigo);
    let padrao = ParametrosKdf::PADRAO;
    let kdf = ParametrosKdf {
        memoria_kib: p.memoria_kib.max(padrao.memoria_kib),
        passes: p.passes.max(padrao.passes),
        paralelismo: p.paralelismo.max(padrao.paralelismo),
    };
    let novo = ChavesCofre::novo_com(nova, kdf)?;
    Ok((novo, conteudo))
}

#[cfg(test)]
mod testes;
