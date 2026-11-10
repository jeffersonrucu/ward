//! Configuração do cofre (D30): a seção que o corpo v2 grava antes das entradas, dentro
//! da parte cifrada.
//!
//! - **Hash do PIN** ([`HashPin`]): Argon2id em texto PHC (`$argon2id$v=19$m=…`), com
//!   parâmetros e sal, calculado por `crypto::argon2id` (memória de trabalho zerada). O
//!   PIN nunca é gravado em claro, nem dentro do corpo. [`HashPin::conferir`] compara em
//!   tempo constante pela biblioteca; o daemon (Fase 22) só chama.
//! - **Tempo de trava** por inatividade, em minutos: padrão 240 (R23, D16), de
//!   [`MIN_LOCK_MINUTES`] a [`MAX_LOCK_MINUTES`].
//! - **Backup externo** ([`BackupExterno`]): desligado por padrão, com o UUID opcional do
//!   pendrive (D17). Ligado exige o UUID.
//!
//! Um cofre sem hash de PIN não existe: [`Configuracao::nova`] exige um, e a leitura do
//! corpo recusa a seção sem ele.

use core::fmt;

use zeroize::{ZeroizeOnDrop, Zeroizing};

use crate::crypto;
use crate::formato::ParametrosKdf;
use crate::limits::{
    DEFAULT_LOCK_MINUTES, MAX_BACKUP_UUID, MAX_LOCK_MINUTES, MAX_PIN_DIGITS, MAX_PIN_HASH,
    MIN_LOCK_MINUTES, MIN_PIN_DIGITS,
};

#[cfg(test)]
mod testes;

/// Argon2id do PIN para hash novo: 64 MiB, 3 passes, 1 faixa. O daemon confere a cada
/// aprovação, então o custo fica abaixo do da senha mestra (que é 256 MiB). O hash só
/// existe dentro da parte cifrada do cofre.
pub const KDF_PIN: ParametrosKdf = ParametrosKdf {
    memoria_kib: 64 * 1024,
    passes: 3,
    paralelismo: 1,
};
/// Menor custo aceito na leitura do hash do PIN (o mesmo de [`KDF_PIN`]).
pub const KDF_PIN_MINIMO: ParametrosKdf = KDF_PIN;
/// Maior custo aceito na leitura: um hash com custo inflado é recusado sem rodar o
/// Argon2 (nem alocar a memória dele).
pub const KDF_PIN_MAXIMO: ParametrosKdf = ParametrosKdf {
    memoria_kib: 256 * 1024,
    passes: 16,
    paralelismo: 4,
};

/// Erro da configuração. Nenhuma variante carrega o PIN, o hash ou outro valor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroConfiguracao {
    /// O PIN não tem de 4 a 6 dígitos.
    PinInvalido,
    /// Falha ao calcular o hash do PIN (aleatório do sistema ou memória).
    Hash,
    /// Tempo de trava fora da faixa aceita.
    TempoDeTrava,
    /// UUID do pendrive vazio, longo ou com caractere fora de `[0-9A-Fa-f-]`.
    UuidInvalido,
    /// Backup externo ligado sem o UUID do pendrive.
    BackupSemUuid,
}

impl fmt::Display for ErroConfiguracao {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PinInvalido => "o PIN tem de ter de 4 a 6 dígitos",
            Self::Hash => "falha ao calcular o hash do PIN",
            Self::TempoDeTrava => "o tempo de trava vai de 1 a 1440 minutos",
            Self::UuidInvalido => {
                "UUID do pendrive inválido (até 36 caracteres, só hexadecimal e hífen)"
            }
            Self::BackupSemUuid => "o backup externo ligado precisa do UUID do pendrive",
        })
    }
}

impl std::error::Error for ErroConfiguracao {}

/// PIN no formato aceito: de [`MIN_PIN_DIGITS`] a [`MAX_PIN_DIGITS`] dígitos ASCII.
fn pin_valido(pin: &[u8]) -> bool {
    (MIN_PIN_DIGITS..=MAX_PIN_DIGITS).contains(&pin.len()) && pin.iter().all(u8::is_ascii_digit)
}

/// Hash Argon2id do PIN em texto PHC. Zera ao sair; sem `Clone`, sem `Display`, e o
/// `Debug` mostra `••••`. O texto é tratado como segredo: um PIN de 6 dígitos sai do
/// hash por força bruta, então ele só existe dentro da parte cifrada do cofre.
pub struct HashPin {
    phc: Zeroizing<String>,
}

// O único campo é um `Zeroizing` (V2.28).
impl ZeroizeOnDrop for HashPin {}

impl fmt::Debug for HashPin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HashPin(••••)")
    }
}

impl HashPin {
    /// Hash novo do PIN (4 a 6 dígitos), com sal aleatório e [`KDF_PIN`].
    pub fn criar(pin: &[u8]) -> Result<Self, ErroConfiguracao> {
        if !pin_valido(pin) {
            return Err(ErroConfiguracao::PinInvalido);
        }
        let phc = crypto::hash_pin(pin, &KDF_PIN).map_err(|_| ErroConfiguracao::Hash)?;
        Ok(Self { phc })
    }

    /// Confere o PIN contra o hash, em tempo constante pela biblioteca. PIN fora do
    /// formato dá `false` sem rodar o Argon2 (o formato não é segredo).
    pub fn conferir(&self, pin: &[u8]) -> bool {
        if !pin_valido(pin) {
            return false;
        }
        match ler(self.phc.as_bytes()) {
            Some(phc) => crypto::verify_pin(pin, &phc),
            None => false,
        }
    }

    /// Parâmetros do Argon2id deste hash (não são segredo).
    pub fn parametros(&self) -> ParametrosKdf {
        // O texto foi conferido ao nascer; `KDF_PIN` só se algo muito errado acontecer.
        ler(self.phc.as_bytes()).map_or(KDF_PIN, |p| p.cost)
    }

    /// Hash lido do corpo: confere o texto inteiro (ver [`conferir_texto`]) e copia para
    /// uma alocação do tamanho exato, que zera ao sair.
    pub(crate) fn de_phc(texto: &[u8]) -> Option<Self> {
        if !conferir_texto(texto) {
            return None;
        }
        let texto = core::str::from_utf8(texto).ok()?;
        let mut phc: Zeroizing<String> = Zeroizing::new(String::new());
        phc.try_reserve_exact(texto.len()).ok()?;
        phc.push_str(texto);
        Some(Self { phc })
    }

    /// Bytes do texto PHC, para o corpo gravar.
    pub(crate) fn bytes(&self) -> &[u8] {
        self.phc.as_bytes()
    }
}

fn ler(texto: &[u8]) -> Option<crypto::PinPhc> {
    crypto::read_pin_phc(texto, &KDF_PIN_MINIMO, &KDF_PIN_MAXIMO)
}

/// Confere o texto PHC do hash do PIN sem alocar (a 1ª passada do corpo usa isto).
pub(crate) fn conferir_texto(texto: &[u8]) -> bool {
    texto.len() <= MAX_PIN_HASH && ler(texto).is_some()
}

/// Tempo de trava dentro da faixa aceita.
pub(crate) fn minutos_validos(minutos: u32) -> bool {
    (MIN_LOCK_MINUTES..=MAX_LOCK_MINUTES).contains(&minutos)
}

/// UUID do pendrive: de 1 a [`MAX_BACKUP_UUID`] caracteres `[0-9A-Fa-f-]`, sem `-` no
/// começo nem no fim. Não é segredo, mas vem do disco e da interface.
pub(crate) fn uuid_valido(uuid: &[u8]) -> bool {
    (1..=MAX_BACKUP_UUID).contains(&uuid.len())
        && uuid.iter().all(|b| b.is_ascii_hexdigit() || *b == b'-')
        && uuid.first() != Some(&b'-')
        && uuid.last() != Some(&b'-')
}

/// Backup externo (D17): ligado ou não, e o UUID do sistema de arquivos do pendrive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupExterno {
    ligado: bool,
    uuid: Option<String>,
}

impl BackupExterno {
    /// Desligado e sem pendrive (o padrão).
    pub fn desligado() -> Self {
        Self {
            ligado: false,
            uuid: None,
        }
    }

    /// Backup com o UUID dado (opcional se desligado; obrigatório se ligado).
    pub fn novo(ligado: bool, uuid: Option<&str>) -> Result<Self, ErroConfiguracao> {
        if let Some(u) = uuid
            && !uuid_valido(u.as_bytes())
        {
            return Err(ErroConfiguracao::UuidInvalido);
        }
        if ligado && uuid.is_none() {
            return Err(ErroConfiguracao::BackupSemUuid);
        }
        Ok(Self {
            ligado,
            uuid: uuid.map(str::to_owned),
        })
    }

    /// Se o backup externo está ligado.
    pub fn ligado(&self) -> bool {
        self.ligado
    }

    /// UUID do pendrive, se escolhido.
    pub fn uuid(&self) -> Option<&str> {
        self.uuid.as_deref()
    }
}

/// Configuração do cofre (D30). Sem `Clone`; o `Debug` não mostra o hash.
#[derive(Debug)]
pub struct Configuracao {
    hash_pin: HashPin,
    minutos_trava: u32,
    backup: BackupExterno,
}

// O hash do PIN zera ao sair; tempo de trava e backup não são segredo (V2.28).
impl ZeroizeOnDrop for Configuracao {}

impl Configuracao {
    /// Configuração de cofre novo: o hash do PIN dado, trava em
    /// [`DEFAULT_LOCK_MINUTES`] minutos e backup externo desligado.
    pub fn nova(hash_pin: HashPin) -> Self {
        Self {
            hash_pin,
            minutos_trava: DEFAULT_LOCK_MINUTES,
            backup: BackupExterno::desligado(),
        }
    }

    /// Montada pela leitura do corpo, com tudo já conferido.
    pub(crate) fn de_partes(hash_pin: HashPin, minutos_trava: u32, backup: BackupExterno) -> Self {
        Self {
            hash_pin,
            minutos_trava,
            backup,
        }
    }

    /// Confere o PIN contra o hash guardado, em tempo constante (ver [`HashPin::conferir`]).
    pub fn conferir_pin(&self, pin: &[u8]) -> bool {
        self.hash_pin.conferir(pin)
    }

    /// O hash do PIN.
    pub fn hash_pin(&self) -> &HashPin {
        &self.hash_pin
    }

    /// Troca o hash do PIN (quem chama já conferiu o PIN atual, V5.22).
    pub fn trocar_hash_pin(&mut self, hash_pin: HashPin) {
        self.hash_pin = hash_pin;
    }

    /// Tempo de trava por inatividade, em minutos.
    pub fn minutos_trava(&self) -> u32 {
        self.minutos_trava
    }

    /// Muda o tempo de trava (de [`MIN_LOCK_MINUTES`] a [`MAX_LOCK_MINUTES`]).
    pub fn definir_minutos_trava(&mut self, minutos: u32) -> Result<(), ErroConfiguracao> {
        if !minutos_validos(minutos) {
            return Err(ErroConfiguracao::TempoDeTrava);
        }
        self.minutos_trava = minutos;
        Ok(())
    }

    /// Backup externo.
    pub fn backup(&self) -> &BackupExterno {
        &self.backup
    }

    /// Muda o backup externo.
    pub fn definir_backup(&mut self, backup: BackupExterno) {
        self.backup = backup;
    }
}
