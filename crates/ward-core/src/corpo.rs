//! Corpo do cofre (D26, D30, D39, D65 d): a configuração, a geração e as entradas em bytes,
//! para o [`crate::format`] cifrar.
//!
//! Codificação binária própria, versionada e canônica, sem dependência nova (S14).
//! Inteiros em little-endian; todo campo de bytes leva o comprimento antes (u32 LE).
//!
//! ```text
//! corpo   = versão (u16) = 4 ‖ configuração ‖ n de entradas (u32) ‖ entrada × n
//! configuração = geração (u64, ≥ 1)
//!           ‖ hash do PIN (campo: texto PHC do Argon2id, obrigatório)
//!           ‖ minutos de trava (u32) ‖ backup ligado (u8 0|1)
//!           ‖ UUID do pendrive (campo; vazio = sem)
//! entrada = tipo ‖ nível ‖ nome ‖ grupo ‖ ambiente ‖ projeto
//!           ‖ n de tags (u32) ‖ tag × n
//!           ‖ campos do tipo (ver `esquema`)
//!           ‖ n de extras (u32) ‖ (nome ‖ segredo: u8 0|1 ‖ valor) × n
//!           ‖ n de anexos (u32) ‖ (nome ‖ conteúdo) × n
//!           ‖ criada em (u64) ‖ alterada em (u64)
//! campo   = comprimento (u32) ‖ bytes
//! ```
//!
//! - **Versão 3** (D30, D39): a configuração ([`VaultConfig`]) vem antes das entradas e
//!   é obrigatória, e começa pela **geração**: o número da versão gravada, que toda
//!   gravação a partir de um [`OpenVault`] sobe em 1. Ela barra a troca do arquivo
//!   por uma versão autêntica mais antiga ([`abrir_entradas_desde`] e
//!   [`ErroAbrir::Retrocesso`]). As versões 1 (só entradas) e 2 (sem geração) são
//!   desconhecidas e recusadas como qualquer outra; geração 0, um corpo sem hash de
//!   PIN, com hash fora do formato ou com custo fora da faixa também. Não há migração:
//!   não existe cofre real nas versões antigas.
//! - **Versão 4** (D65 d, provisória): cada entrada termina com as datas de criação e
//!   de alteração ([`Dates`]), em segundos UTC; 0 é "sem data". A leitura confere a
//!   regra de [`Dates`] (faixa de 1 a [`MAX_DATE`], criação só com alteração e não
//!   depois dela). A versão 3 (sem datas) é recusada como versão desconhecida, sem
//!   migração: não há cofre instalado, e o `make dev-vault` regrava o de teste.
//! - Tipo, nível, ambiente, motor, segurança e protocolo vão pelos identificadores
//!   estáveis da Fase 15 (`login`, `com-aprovacao`…). A porta vai em 2 bytes (u16).
//!   Campo opcional ausente é um campo vazio.
//! - **Canônica**: as entradas saem em ordem de nome; as mesmas entradas dão sempre os
//!   mesmos bytes. A leitura exige a ordem estrita (o que também barra nome repetido),
//!   então todo corpo aceito, serializado de novo, dá os mesmos bytes.
//! - **Gravar** (S8): o tamanho final é calculado antes, e o buffer é um
//!   `Zeroizing<Vec<u8>>` com essa capacidade exata; nenhuma realocação deixa cópia em
//!   claro. Corpo acima de [`MAX_BODY`] ou com mais de [`MAX_ENTRIES`] é recusado
//!   antes de alocar. Entradas inconsistentes entre si (nome repetido, túnel ou salto
//!   inválido) também são recusadas: o que se grava sempre reabre.
//! - **Ler** (o disco é entrada não confiável, mesmo autenticado): em duas passadas.
//!   A primeira só percorre o buffer, sem alocar nada, e confere versão, a
//!   configuração inteira (o texto PHC é lido e escrito de novo numa pilha), contagens,
//!   cada comprimento contra o que resta e contra os limites de [`crate::limits`]
//!   (inclusive a soma dos anexos de cada entrada, D29), tipo, nível, ordem dos nomes e
//!   byte sobrando. A segunda monta a configuração e cada entrada por
//!   [`Draft::validate`], aplica o nível guardado por [`Entry::set_level`] e
//!   confere o cofre todo por [`Entry::verify_in_vault`]. Qualquer falha recusa o
//!   corpo inteiro com [`ErroCorpo::Invalido`], sem valor do usuário (S13); nunca se
//!   abre um cofre pela metade.

use core::fmt;

use zeroize::Zeroizing;

use crate::config::{self, ExternalBackup, PinHash, VaultConfig};
use crate::entry::{
    AccessLevel, AttachmentDraft, Auth, Data, DataDraft, Dates, Draft, Entry, EntryType,
    ExtraDraft, ExtraValue, Secret,
};
use crate::format::{self, FormatError, MasterPassword, OpenVault};

#[cfg(test)]
mod testes;

/// Versão atual do corpo (D30: com a seção de configuração; D39: com a geração; D65 d:
/// com as datas de cada entrada).
pub const VERSAO_CORPO: u16 = 4;

// Os limites estão em `crate::limits` (D32): a leitura recusa acima deles antes de
// alocar, e a validação da entrada confere de novo, com o limite exato de cada campo.
use crate::limits::{
    MAX_ATTACHMENTS, MAX_ATTACHMENTS_TOTAL, MAX_BACKUP_UUID, MAX_DATE, MAX_EXTRA_VALUE, MAX_EXTRAS,
    MAX_NOTE, MAX_PIN_HASH, MAX_SECRET, MAX_TAGS, MAX_TEXT, MIB,
};
pub use crate::limits::{MAX_BODY, MAX_ENTRIES};
/// Mais campos de um tipo (`banco`, `servidor`, `ftp/sftp`). Estrutural, não é limite.
const MAX_DADOS: usize = 7;

/// Erro do corpo. Nenhuma variante carrega dado do cofre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroCorpo {
    /// Leitura: o corpo decifrado não é um corpo válido (truncado, com sobra, versão,
    /// tipo ou nível desconhecido, campo fora do limite, entrada inválida, nome
    /// repetido, referência quebrada…). Um erro só, de propósito.
    Invalido,
    /// Gravação: acima de [`MAX_BODY`] bytes ou de [`MAX_ENTRIES`] entradas.
    GrandeDemais,
    /// Gravação: nome repetido ou túnel/salto que não fecha no cofre (não reabriria).
    Inconsistente,
}

impl fmt::Display for ErroCorpo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalido => f.write_str("conteúdo do cofre inválido ou corrompido"),
            // Montada a partir de `crate::limits` (D45): a mensagem acompanha a
            // constante. O milhar vai com espaço, como no resto do texto.
            Self::GrandeDemais => write!(
                f,
                "cofre grande demais (limite de {} MiB e {} entradas)",
                MAX_BODY / MIB,
                Milhar(MAX_ENTRIES)
            ),
            Self::Inconsistente => f.write_str(
                "entradas inconsistentes: nome repetido ou túnel/salto que não aponta para um servidor válido",
            ),
        }
    }
}

impl std::error::Error for ErroCorpo {}

/// Número com espaço entre os milhares (`10 000`), para as mensagens.
struct Milhar(usize);

impl fmt::Display for Milhar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (alto, baixo) = (self.0 / 1000, self.0 % 1000);
        if alto == 0 {
            write!(f, "{baixo}")
        } else {
            write!(f, "{} {baixo:03}", Milhar(alto))
        }
    }
}

/// Erro ao abrir o cofre e ler as entradas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroAbrir {
    /// Senha errada, arquivo adulterado ou parâmetros recusados.
    Formato(FormatError),
    /// Decifrou, mas o conteúdo não é um corpo válido.
    Corpo(ErroCorpo),
    /// Decifrou e o corpo é válido, mas a geração dele é menor que a mínima pedida
    /// ([`abrir_entradas_desde`], D39): o arquivo foi trocado por uma versão autêntica
    /// mais antiga (um backup, por exemplo). Não leva valor nenhum, nem as gerações.
    Retrocesso,
}

impl fmt::Display for ErroAbrir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Formato(e) => fmt::Display::fmt(e, f),
            Self::Corpo(e) => fmt::Display::fmt(e, f),
            Self::Retrocesso => f.write_str("o cofre no disco é mais antigo que o último aberto"),
        }
    }
}

impl std::error::Error for ErroAbrir {}

/// Abre o arquivo do cofre e lê a configuração, a geração e as entradas (o caminho do
/// `make dev-vault`). O [`OpenVault`] devolvido leva a configuração e a geração lidas
/// (D30, D39). O conteúdo decifrado zera ao sair; as entradas voltam em ordem de nome.
///
/// Aceita qualquer geração. O daemon (Fase 22) abre por [`abrir_entradas_desde`], com
/// a maior geração que já viu e a maior que já queimou.
pub fn abrir_entradas(
    arquivo: &[u8],
    senha: &MasterPassword,
) -> Result<(OpenVault, Vec<Entry>), ErroAbrir> {
    abrir_entradas_desde(arquivo, senha, 0, 0)
}

/// Como [`abrir_entradas`], mas recusa com [`ErroAbrir::Retrocesso`] um corpo cuja
/// geração seja menor que `geracao_minima` (D39): a troca do arquivo por uma versão
/// autêntica mais antiga. A conferência vem depois de decifrar e de validar o corpo
/// inteiro, e nenhuma entrada é devolvida.
///
/// `geracao_queimada` (D53) é a maior geração já selada ou tentada que o chamador
/// conhece (uma gravação que falhou depois de selar). O [`OpenVault`] devolvido fica
/// com max(geração do corpo, `geracao_queimada`), e a próxima gravação usa um número
/// que nunca foi selado.
pub fn abrir_entradas_desde(
    arquivo: &[u8],
    senha: &MasterPassword,
    geracao_minima: u64,
    geracao_queimada: u64,
) -> Result<(OpenVault, Vec<Entry>), ErroAbrir> {
    abrir_entradas_lida(arquivo, senha, geracao_minima, geracao_queimada)
        .map(|(cofre, entradas, _)| (cofre, entradas))
}

/// Como [`abrir_entradas_desde`], e devolve também a geração **lida do corpo** (Fase 22,
/// D53). O [`OpenVault::generation`] fica em max(lida, `geracao_queimada`); o daemon
/// precisa da lida para subir a maior geração confirmada no disco (`vista`) sem
/// confundi-la com uma queimada que nunca chegou ao disco.
pub fn abrir_entradas_lida(
    arquivo: &[u8],
    senha: &MasterPassword,
    geracao_minima: u64,
    geracao_queimada: u64,
) -> Result<(OpenVault, Vec<Entry>, u64), ErroAbrir> {
    let (chaves, conteudo) = format::open(arquivo, senha).map_err(ErroAbrir::Formato)?;
    let (configuracao, geracao, entradas) = desserializar(&conteudo).map_err(ErroAbrir::Corpo)?;
    drop(conteudo);
    if geracao < geracao_minima {
        // As entradas e a configuração lidas zeram aqui.
        return Err(ErroAbrir::Retrocesso);
    }
    Ok((
        OpenVault::from_parts(chaves, configuracao, geracao.max(geracao_queimada)),
        entradas,
        geracao,
    ))
}

/// Serializa a configuração, a geração (de 1 a `u64::MAX`, D39) e as entradas (em
/// qualquer ordem; saem em ordem de nome). O buffer zera ao sair e nasce com a
/// capacidade final. Geração 0 é recusada com [`ErroCorpo::Inconsistente`].
///
/// Serializar não grava: fora do crate, só [`crate::atomic_write::write_entries`] e
/// [`crate::atomic_write::change_password`] selam e gravam, e é a gravação que escolhe a
/// geração (a do cofre aberto + 1).
pub fn serializar(
    configuracao: &VaultConfig,
    geracao: u64,
    entradas: &[Entry],
) -> Result<Zeroizing<Vec<u8>>, ErroCorpo> {
    if geracao == 0 {
        return Err(ErroCorpo::Inconsistente);
    }
    if entradas.len() > MAX_ENTRIES {
        return Err(ErroCorpo::GrandeDemais);
    }
    let mut ordem: Vec<&Entry> = entradas.iter().collect();
    ordem.sort_unstable_by(|a, b| a.name().cmp(b.name()));
    if !conferir_ordenadas(&ordem) {
        return Err(ErroCorpo::Inconsistente);
    }

    let mut conta = Saida {
        total: 0,
        buf: None,
        erro: false,
    };
    escrever_corpo(&mut conta, configuracao, geracao, &ordem);
    if conta.erro {
        return Err(ErroCorpo::Inconsistente);
    }
    let total = conta.total;
    if total > MAX_BODY {
        return Err(ErroCorpo::GrandeDemais);
    }

    let mut buf: Zeroizing<Vec<u8>> = Zeroizing::new(Vec::new());
    buf.try_reserve_exact(total)
        .map_err(|_| ErroCorpo::GrandeDemais)?;
    let mut saida = Saida {
        total: 0,
        buf: Some(&mut buf),
        erro: false,
    };
    escrever_corpo(&mut saida, configuracao, geracao, &ordem);
    if saida.erro || buf.len() != total {
        return Err(ErroCorpo::Inconsistente);
    }
    Ok(buf)
}

/// Corpo lido: a configuração, a geração (≥ 1) e as entradas, em ordem de nome.
pub type CorpoLido = (VaultConfig, u64, Vec<Entry>);

/// Lê o corpo decifrado: a configuração, a geração e as entradas. Recusa o corpo
/// inteiro com [`ErroCorpo::Invalido`] em qualquer problema.
pub fn desserializar(corpo: &[u8]) -> Result<CorpoLido, ErroCorpo> {
    if corpo.len() > MAX_BODY {
        return Err(ErroCorpo::Invalido);
    }
    // 1ª passada: estrutura inteira, sem alocar.
    let n = conferir_estrutura(corpo)?;

    // 2ª passada: monta e valida cada entrada (já se sabe que as `n` existem no buffer).
    let mut entradas: Vec<Entry> = Vec::new();
    entradas
        .try_reserve_exact(n)
        .map_err(|_| ErroCorpo::Invalido)?;
    let mut l = Leitor { resto: corpo };
    let (bruta_cfg, n2) = ler_cabecalho(&mut l)?;
    if n2 != n {
        return Err(ErroCorpo::Invalido);
    }
    let configuracao = montar_configuracao(&bruta_cfg)?;
    for _ in 0..n {
        let bruta = ler_bruta(&mut l)?;
        entradas.push(montar(&bruta)?);
    }
    if !l.resto.is_empty() {
        return Err(ErroCorpo::Invalido);
    }

    let ordem: Vec<&Entry> = entradas.iter().collect();
    if !conferir_ordenadas(&ordem) {
        return Err(ErroCorpo::Invalido);
    }
    drop(ordem);
    Ok((configuracao, bruta_cfg.geracao, entradas))
}

/// Só a geração de um corpo decifrado (Fase 49a), depois de conferir a estrutura
/// inteira (sem montar entradas nem alocar segredo). `None` em qualquer problema.
pub(crate) fn geracao_do_corpo(corpo: &[u8]) -> Option<u64> {
    if corpo.len() > MAX_BODY {
        return None;
    }
    conferir_estrutura(corpo).ok()?;
    let mut l = Leitor { resto: corpo };
    let (cfg, _) = ler_cabecalho(&mut l).ok()?;
    Some(cfg.geracao)
}

// ---- Conferência do cofre ----

/// Nomes em ordem estrita (sem repetido) e cada entrada conferida por
/// [`Entry::verify_in_vault`] contra as que importam para ela: a que ela aponta e
/// as que apontam para ela. São exatamente as que `conferir_no_cofre` olha (nome igual,
/// alvo da referência, quem referencia esta), sem custo quadrático em 10 000 entradas.
fn conferir_ordenadas(ordem: &[&Entry]) -> bool {
    if ordem.windows(2).any(|w| w[0].name() >= w[1].name()) {
        return false;
    }
    let achar = |nome: &str| ordem.binary_search_by(|e| e.name().cmp(nome)).ok();
    // (alvo, quem aponta), em ordem de alvo.
    let mut apontam: Vec<(usize, usize)> = ordem
        .iter()
        .enumerate()
        .filter_map(|(j, e)| e.reference().and_then(achar).map(|alvo| (alvo, j)))
        .collect();
    apontam.sort_unstable();
    let mut vizinhas: Vec<usize> = Vec::new();
    for (i, e) in ordem.iter().enumerate() {
        vizinhas.clear();
        if let Some(alvo) = e.reference().and_then(achar)
            && alvo != i
        {
            vizinhas.push(alvo);
        }
        let inicio = apontam.partition_point(|&(alvo, _)| alvo < i);
        for &(alvo, j) in &apontam[inicio..] {
            if alvo != i {
                break;
            }
            if j != i && !vizinhas.contains(&j) {
                vizinhas.push(j);
            }
        }
        if e.verify_in_vault(vizinhas.iter().map(|&j| ordem[j]))
            .is_err()
        {
            return false;
        }
    }
    true
}

// ---- Esquema dos campos de cada tipo ----

/// Tipo de um campo do tipo, com o limite de leitura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Campo {
    /// Texto que não é segredo.
    Texto,
    /// Porta: exatamente 2 bytes (u16 LE).
    Porta,
    /// Segredo de texto (senha, token, chave, semente…).
    Segredo,
    /// Texto da nota segura.
    Nota,
}

impl Campo {
    fn max(self) -> usize {
        match self {
            Campo::Texto => MAX_TEXT,
            Campo::Porta => 2,
            Campo::Segredo => MAX_SECRET,
            Campo::Nota => MAX_NOTE,
        }
    }
}

/// Ordem dos campos de cada tipo no corpo. Leitura e escrita usam esta mesma tabela.
fn esquema(tipo: EntryType) -> &'static [Campo] {
    use Campo::{Nota, Porta, Segredo as S, Texto as T};
    match tipo {
        // url, usuário, senha, semente TOTP
        EntryType::Login => &[T, T, S, S],
        // motor, host, porta, base, usuário, senha, túnel
        EntryType::Database => &[T, T, Porta, T, T, S, T],
        // host, porta, usuário, senha, chave privada, passphrase, salto
        EntryType::Server => &[T, Porta, T, S, S, S, T],
        // url, token, variável
        EntryType::Api => &[T, S, T],
        // access key ID, secret, região, role ARN, session token
        EntryType::Aws => &[T, S, T, T, S],
        // host, porta, usuário, senha, segurança
        EntryType::Smtp => &[T, Porta, T, S, T],
        // protocolo, host, porta, usuário, senha, chave privada, passphrase
        EntryType::FtpSftp => &[T, T, Porta, T, S, S, S],
        // texto
        EntryType::SecureNote => &[Nota],
        EntryType::Generic => &[],
    }
}

// ---- Escrita ----

/// Destino da escrita: só conta (primeira passada) ou copia num buffer que já tem a
/// capacidade final (segunda). Nunca realoca: se faltasse espaço, marca erro e para
/// de copiar.
struct Saida<'b> {
    total: usize,
    buf: Option<&'b mut Vec<u8>>,
    erro: bool,
}

impl Saida<'_> {
    fn por(&mut self, bytes: &[u8]) {
        self.total = self.total.saturating_add(bytes.len());
        if let Some(buf) = self.buf.as_deref_mut() {
            if buf.capacity() - buf.len() >= bytes.len() {
                buf.extend_from_slice(bytes);
            } else {
                self.erro = true;
            }
        }
    }

    fn u32(&mut self, n: usize) {
        match u32::try_from(n) {
            Ok(n) => self.por(&n.to_le_bytes()),
            Err(_) => self.erro = true,
        }
    }

    /// Campo com comprimento prefixado, conferido contra o limite da leitura (o que
    /// se grava sempre se lê de volta).
    fn campo(&mut self, bytes: &[u8], max: usize) {
        if bytes.len() > max {
            self.erro = true;
        }
        self.u32(bytes.len());
        self.por(bytes);
    }
}

fn escrever_corpo(s: &mut Saida<'_>, configuracao: &VaultConfig, geracao: u64, ordem: &[&Entry]) {
    s.por(&VERSAO_CORPO.to_le_bytes());
    if geracao == 0 {
        s.erro = true;
    }
    s.por(&geracao.to_le_bytes());
    escrever_configuracao(s, configuracao);
    s.u32(ordem.len());
    for e in ordem {
        escrever_entrada(s, e);
    }
}

fn escrever_configuracao(s: &mut Saida<'_>, c: &VaultConfig) {
    s.campo(c.pin_hash().bytes(), MAX_PIN_HASH);
    let minutos = c.lock_minutes();
    if !config::valid_minutes(minutos) {
        s.erro = true;
    }
    s.por(&minutos.to_le_bytes());
    s.por(&[u8::from(c.backup().enabled())]);
    s.campo(c.backup().uuid().unwrap_or("").as_bytes(), MAX_BACKUP_UUID);
}

fn opcional(s: &Option<Secret>) -> &[u8] {
    s.as_ref().map_or(&[], Secret::expose)
}

fn texto_opcional(t: &Option<String>) -> &[u8] {
    t.as_deref().map_or(&[], str::as_bytes)
}

/// Senha, chave privada e passphrase (as ausentes vazias).
fn autenticacao(a: &Auth) -> [&[u8]; 3] {
    match a {
        Auth::Password(senha) => [senha.expose(), &[], &[]],
        Auth::Key {
            private_key: chave_privada,
            passphrase,
        } => [&[], chave_privada.expose(), opcional(passphrase)],
    }
}

/// Os campos do tipo na ordem de [`esquema`].
fn campos_dados<'e>(dados: &'e Data, porta: &'e mut [u8; 2]) -> [&'e [u8]; MAX_DADOS] {
    let mut c: [&[u8]; MAX_DADOS] = [&[]; MAX_DADOS];
    match dados {
        Data::Login(l) => {
            c[..4].copy_from_slice(&[
                l.url.as_bytes(),
                l.username.as_bytes(),
                l.password.expose(),
                opcional(&l.totp_seed),
            ]);
        }
        Data::Database(b) => {
            *porta = b.port.to_le_bytes();
            c.copy_from_slice(&[
                b.engine.name().as_bytes(),
                b.host.as_bytes(),
                &porta[..],
                texto_opcional(&b.db_name),
                b.username.as_bytes(),
                b.password.expose(),
                texto_opcional(&b.tunnel),
            ]);
        }
        Data::Server(sv) => {
            *porta = sv.port.to_le_bytes();
            let [senha, chave, passphrase] = autenticacao(&sv.auth);
            c.copy_from_slice(&[
                sv.host.as_bytes(),
                &porta[..],
                sv.username.as_bytes(),
                senha,
                chave,
                passphrase,
                texto_opcional(&sv.jump),
            ]);
        }
        Data::Api(a) => {
            c[..3].copy_from_slice(&[
                texto_opcional(&a.url),
                a.token.expose(),
                texto_opcional(&a.variable),
            ]);
        }
        Data::Aws(a) => {
            c[..5].copy_from_slice(&[
                a.access_key_id.as_bytes(),
                a.secret_access_key.expose(),
                a.region.as_bytes(),
                texto_opcional(&a.role_arn),
                opcional(&a.session_token),
            ]);
        }
        Data::Smtp(sm) => {
            *porta = sm.port.to_le_bytes();
            c[..5].copy_from_slice(&[
                sm.host.as_bytes(),
                &porta[..],
                sm.username.as_bytes(),
                sm.password.expose(),
                sm.security.name().as_bytes(),
            ]);
        }
        Data::FtpSftp(f) => {
            *porta = f.port.to_le_bytes();
            let [senha, chave, passphrase] = autenticacao(&f.auth);
            c.copy_from_slice(&[
                f.protocol.name().as_bytes(),
                f.host.as_bytes(),
                &porta[..],
                f.username.as_bytes(),
                senha,
                chave,
                passphrase,
            ]);
        }
        Data::SecureNote(n) => c[0] = n.text.expose(),
        Data::Generic => {}
    }
    c
}

fn escrever_entrada(s: &mut Saida<'_>, e: &Entry) {
    s.campo(e.kind().name().as_bytes(), MAX_TEXT);
    s.campo(e.level().name().as_bytes(), MAX_TEXT);
    s.campo(e.name().as_bytes(), MAX_TEXT);
    s.campo(e.group().as_bytes(), MAX_TEXT);
    s.campo(e.environment().name().as_bytes(), MAX_TEXT);
    s.campo(e.project().unwrap_or("").as_bytes(), MAX_TEXT);
    s.u32(e.tags().len());
    for tag in e.tags() {
        s.campo(tag.as_bytes(), MAX_TEXT);
    }
    let mut porta = [0u8; 2];
    let valores = campos_dados(e.data(), &mut porta);
    for (campo, valor) in esquema(e.kind()).iter().zip(valores) {
        s.campo(valor, campo.max());
    }
    s.u32(e.extras().len());
    for extra in e.extras() {
        s.campo(extra.name().as_bytes(), MAX_TEXT);
        match extra.value() {
            ExtraValue::Text(t) => {
                s.por(&[0]);
                s.campo(t.as_bytes(), MAX_EXTRA_VALUE);
            }
            ExtraValue::Secret(v) => {
                s.por(&[1]);
                s.campo(v.expose(), MAX_EXTRA_VALUE);
            }
        }
    }
    s.u32(e.attachments().len());
    for anexo in e.attachments() {
        s.campo(anexo.name().as_bytes(), MAX_TEXT);
        s.campo(anexo.content().expose(), MAX_ATTACHMENTS_TOTAL);
    }
    // D65 d: 0 = sem data. Uma `Datas` só existe dentro da regra; a faixa é conferida
    // de novo para o que se grava sempre reabrir.
    let datas = e.dates();
    for d in [datas.created_at(), datas.modified_at()] {
        let d = d.unwrap_or(0);
        if d > MAX_DATE {
            s.erro = true;
        }
        s.por(&d.to_le_bytes());
    }
}

// ---- Leitura ----

/// Leitor sobre o corpo: toda leitura confere o que resta antes; nada aloca.
struct Leitor<'a> {
    resto: &'a [u8],
}

impl<'a> Leitor<'a> {
    fn bytes(&mut self, n: usize) -> Result<&'a [u8], ErroCorpo> {
        if n > self.resto.len() {
            return Err(ErroCorpo::Invalido);
        }
        let (pedaco, resto) = self.resto.split_at(n);
        self.resto = resto;
        Ok(pedaco)
    }

    fn u8(&mut self) -> Result<u8, ErroCorpo> {
        Ok(self.bytes(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ErroCorpo> {
        let b = self.bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u64(&mut self) -> Result<u64, ErroCorpo> {
        let b = self.bytes(8)?;
        let mut b8 = [0u8; 8];
        b8.copy_from_slice(b);
        Ok(u64::from_le_bytes(b8))
    }

    fn u32(&mut self) -> Result<usize, ErroCorpo> {
        let b = self.bytes(4)?;
        usize::try_from(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .map_err(|_| ErroCorpo::Invalido)
    }

    /// Contagem de uma lista, recusada acima do limite antes de qualquer coisa.
    fn contagem(&mut self, max: usize) -> Result<usize, ErroCorpo> {
        let n = self.u32()?;
        if n > max {
            return Err(ErroCorpo::Invalido);
        }
        Ok(n)
    }

    /// Campo com comprimento prefixado: o comprimento é conferido contra o limite e
    /// contra o que resta do buffer antes de qualquer coisa.
    fn campo(&mut self, max: usize) -> Result<&'a [u8], ErroCorpo> {
        let n = self.u32()?;
        if n > max {
            return Err(ErroCorpo::Invalido);
        }
        self.bytes(n)
    }
}

/// Uma lista (tags, extras, anexos): quantos itens e os bytes deles, já percorridos.
#[derive(Clone, Copy)]
struct Lista<'a> {
    n: usize,
    bytes: &'a [u8],
}

/// Uma entrada como está no buffer: só fatias, nada alocado. Tudo já conferido contra
/// os limites; o conteúdo ainda não foi validado.
struct Bruta<'a> {
    tipo: EntryType,
    nivel: AccessLevel,
    nome: &'a [u8],
    grupo: &'a [u8],
    ambiente: &'a [u8],
    projeto: &'a [u8],
    tags: Lista<'a>,
    dados: [&'a [u8]; MAX_DADOS],
    extras: Lista<'a>,
    anexos: Lista<'a>,
    /// Já conferidas pela regra de [`Dates`] (D65 d).
    datas: Dates,
}

/// A seção de configuração como está no buffer, já conferida inteira (sem alocar).
struct BrutaConfiguracao<'a> {
    geracao: u64,
    hash_pin: &'a [u8],
    minutos_trava: u32,
    ligado: bool,
    uuid: &'a [u8],
}

/// Versão, configuração e número de entradas.
fn ler_cabecalho<'a>(l: &mut Leitor<'a>) -> Result<(BrutaConfiguracao<'a>, usize), ErroCorpo> {
    if l.u16()? != VERSAO_CORPO {
        return Err(ErroCorpo::Invalido);
    }
    let cfg = ler_configuracao(l)?;
    Ok((cfg, l.contagem(MAX_ENTRIES)?))
}

/// A seção de configuração (D30, D39): geração ≥ 1, hash PHC conferido por inteiro (Argon2id, custo na
/// faixa, sal e saída do tamanho certo, texto canônico), tempo de trava na faixa, marca
/// 0|1 e UUID válido; ligado sem UUID é recusado. Nada aloca.
fn ler_configuracao<'a>(l: &mut Leitor<'a>) -> Result<BrutaConfiguracao<'a>, ErroCorpo> {
    // D39: a geração começa em 1; 0 é corpo inválido (o erro genérico).
    let geracao = l.u64()?;
    if geracao == 0 {
        return Err(ErroCorpo::Invalido);
    }
    let hash_pin = l.campo(MAX_PIN_HASH)?;
    if !config::verify_text(hash_pin) {
        return Err(ErroCorpo::Invalido);
    }
    let b = l.bytes(4)?;
    let minutos_trava = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    if !config::valid_minutes(minutos_trava) {
        return Err(ErroCorpo::Invalido);
    }
    let ligado = match l.u8()? {
        0 => false,
        1 => true,
        _ => return Err(ErroCorpo::Invalido),
    };
    let uuid = l.campo(MAX_BACKUP_UUID)?;
    if (!uuid.is_empty() && !config::valid_uuid(uuid)) || (ligado && uuid.is_empty()) {
        return Err(ErroCorpo::Invalido);
    }
    Ok(BrutaConfiguracao {
        geracao,
        hash_pin,
        minutos_trava,
        ligado,
        uuid,
    })
}

/// 2ª passada da configuração: copia o hash para um tipo que zera ao sair.
fn montar_configuracao(b: &BrutaConfiguracao<'_>) -> Result<VaultConfig, ErroCorpo> {
    let hash_pin = PinHash::from_phc(b.hash_pin).ok_or(ErroCorpo::Invalido)?;
    let uuid = if b.uuid.is_empty() {
        None
    } else {
        Some(core::str::from_utf8(b.uuid).map_err(|_| ErroCorpo::Invalido)?)
    };
    let backup = ExternalBackup::new(b.ligado, uuid).map_err(|_| ErroCorpo::Invalido)?;
    Ok(VaultConfig::from_parts(hash_pin, b.minutos_trava, backup))
}

fn ler_tag<'a>(l: &mut Leitor<'a>) -> Result<&'a [u8], ErroCorpo> {
    l.campo(MAX_TEXT)
}

fn ler_extra<'a>(l: &mut Leitor<'a>) -> Result<(&'a [u8], bool, &'a [u8]), ErroCorpo> {
    let nome = l.campo(MAX_TEXT)?;
    let segredo = match l.u8()? {
        0 => false,
        1 => true,
        _ => return Err(ErroCorpo::Invalido),
    };
    let valor = l.campo(MAX_EXTRA_VALUE)?;
    Ok((nome, segredo, valor))
}

fn ler_anexo<'a>(l: &mut Leitor<'a>) -> Result<(&'a [u8], &'a [u8]), ErroCorpo> {
    let nome = l.campo(MAX_TEXT)?;
    let conteudo = l.campo(MAX_ATTACHMENTS_TOTAL)?;
    Ok((nome, conteudo))
}

/// Os anexos de uma entrada: até [`MAX_ATTACHMENTS`] itens que somam até
/// [`MAX_ATTACHMENTS_TOTAL`] (D29), conferidos antes de alocar.
fn ler_anexos<'a>(l: &mut Leitor<'a>) -> Result<Lista<'a>, ErroCorpo> {
    let n = l.contagem(MAX_ATTACHMENTS)?;
    let inicio = l.resto;
    let mut soma: usize = 0;
    for _ in 0..n {
        let (_, conteudo) = ler_anexo(l)?;
        soma = soma
            .checked_add(conteudo.len())
            .filter(|s| *s <= MAX_ATTACHMENTS_TOTAL)
            .ok_or(ErroCorpo::Invalido)?;
    }
    let usados = inicio.len() - l.resto.len();
    Ok(Lista {
        n,
        bytes: &inicio[..usados],
    })
}

/// Percorre uma lista de até `max` itens com `item`, sem alocar.
fn ler_lista<'a, T>(
    l: &mut Leitor<'a>,
    max: usize,
    item: fn(&mut Leitor<'a>) -> Result<T, ErroCorpo>,
) -> Result<Lista<'a>, ErroCorpo> {
    let n = l.contagem(max)?;
    let inicio = l.resto;
    for _ in 0..n {
        item(l)?;
    }
    let usados = inicio.len() - l.resto.len();
    Ok(Lista {
        n,
        bytes: &inicio[..usados],
    })
}

fn ler_bruta<'a>(l: &mut Leitor<'a>) -> Result<Bruta<'a>, ErroCorpo> {
    let tipo = core::str::from_utf8(l.campo(MAX_TEXT)?)
        .ok()
        .and_then(EntryType::from_name)
        .ok_or(ErroCorpo::Invalido)?;
    let nivel = core::str::from_utf8(l.campo(MAX_TEXT)?)
        .ok()
        .and_then(AccessLevel::from_name)
        .ok_or(ErroCorpo::Invalido)?;
    let nome = l.campo(MAX_TEXT)?;
    let grupo = l.campo(MAX_TEXT)?;
    let ambiente = l.campo(MAX_TEXT)?;
    let projeto = l.campo(MAX_TEXT)?;
    let tags = ler_lista(l, MAX_TAGS, ler_tag)?;
    let mut dados: [&[u8]; MAX_DADOS] = [&[]; MAX_DADOS];
    for (i, campo) in esquema(tipo).iter().enumerate() {
        let valor = l.campo(campo.max())?;
        if *campo == Campo::Porta && valor.len() != 2 {
            return Err(ErroCorpo::Invalido);
        }
        dados[i] = valor;
    }
    let extras = ler_lista(l, MAX_EXTRAS, ler_extra)?;
    let anexos = ler_anexos(l)?;
    let datas = ler_datas(l)?;
    Ok(Bruta {
        tipo,
        nivel,
        nome,
        grupo,
        ambiente,
        projeto,
        tags,
        dados,
        extras,
        anexos,
        datas,
    })
}

/// As datas da entrada (D65 d): 0 = sem; o resto pela regra de [`Dates`].
fn ler_datas(l: &mut Leitor<'_>) -> Result<Dates, ErroCorpo> {
    let opcional = |d: u64| (d != 0).then_some(d);
    let criada = l.u64()?;
    let alterada = l.u64()?;
    Dates::from_parts(opcional(criada), opcional(alterada)).ok_or(ErroCorpo::Invalido)
}

/// 1ª passada: o corpo inteiro, sem alocar. Devolve o número de entradas.
fn conferir_estrutura(corpo: &[u8]) -> Result<usize, ErroCorpo> {
    let mut l = Leitor { resto: corpo };
    let (_, n) = ler_cabecalho(&mut l)?;
    let mut anterior: Option<&[u8]> = None;
    for _ in 0..n {
        let b = ler_bruta(&mut l)?;
        // Ordem estrita dos nomes: canônico e sem nome repetido.
        if anterior.is_some_and(|a| a >= b.nome) {
            return Err(ErroCorpo::Invalido);
        }
        anterior = Some(b.nome);
    }
    if !l.resto.is_empty() {
        return Err(ErroCorpo::Invalido);
    }
    Ok(n)
}

fn texto(bytes: &[u8]) -> Result<String, ErroCorpo> {
    core::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| ErroCorpo::Invalido)
}

/// Segredo copiado do buffer para uma alocação do tamanho exato (vazio = ausente).
fn segredo(bytes: &[u8]) -> Option<Secret> {
    (!bytes.is_empty()).then(|| Secret::from_bytes(bytes.to_vec()))
}

fn porta(bytes: &[u8]) -> Result<String, ErroCorpo> {
    match bytes {
        [a, b] => Ok(u16::from_le_bytes([*a, *b]).to_string()),
        _ => Err(ErroCorpo::Invalido),
    }
}

fn montar_dados(tipo: EntryType, d: &[&[u8]; MAX_DADOS]) -> Result<DataDraft, ErroCorpo> {
    Ok(match tipo {
        EntryType::Login => DataDraft::Login {
            url: texto(d[0])?,
            username: texto(d[1])?,
            password: segredo(d[2]),
            totp_seed: segredo(d[3]),
        },
        EntryType::Database => DataDraft::Database {
            engine: texto(d[0])?,
            host: texto(d[1])?,
            port: porta(d[2])?,
            db_name: texto(d[3])?,
            username: texto(d[4])?,
            password: segredo(d[5]),
            tunnel: texto(d[6])?,
        },
        EntryType::Server => DataDraft::Server {
            host: texto(d[0])?,
            port: porta(d[1])?,
            username: texto(d[2])?,
            password: segredo(d[3]),
            private_key: segredo(d[4]),
            passphrase: segredo(d[5]),
            jump: texto(d[6])?,
        },
        EntryType::Api => DataDraft::Api {
            url: texto(d[0])?,
            token: segredo(d[1]),
            variable: texto(d[2])?,
        },
        EntryType::Aws => DataDraft::Aws {
            access_key_id: texto(d[0])?,
            secret_access_key: segredo(d[1]),
            region: texto(d[2])?,
            role_arn: texto(d[3])?,
            session_token: segredo(d[4]),
        },
        EntryType::Smtp => DataDraft::Smtp {
            host: texto(d[0])?,
            port: porta(d[1])?,
            username: texto(d[2])?,
            password: segredo(d[3]),
            security: texto(d[4])?,
        },
        EntryType::FtpSftp => DataDraft::FtpSftp {
            protocol: texto(d[0])?,
            host: texto(d[1])?,
            port: porta(d[2])?,
            username: texto(d[3])?,
            password: segredo(d[4]),
            private_key: segredo(d[5]),
            passphrase: segredo(d[6]),
        },
        EntryType::SecureNote => DataDraft::SecureNote {
            text: segredo(d[0]),
        },
        EntryType::Generic => DataDraft::Generic,
    })
}

/// 2ª passada de uma entrada: rascunho → [`Draft::validate`] → nível guardado.
fn montar(b: &Bruta<'_>) -> Result<Entry, ErroCorpo> {
    let mut tags = Vec::with_capacity(b.tags.n);
    let mut l = Leitor {
        resto: b.tags.bytes,
    };
    for _ in 0..b.tags.n {
        tags.push(texto(ler_tag(&mut l)?)?);
    }
    let mut extras = Vec::with_capacity(b.extras.n);
    let mut l = Leitor {
        resto: b.extras.bytes,
    };
    for _ in 0..b.extras.n {
        let (nome, segredo, valor) = ler_extra(&mut l)?;
        extras.push(ExtraDraft {
            name: texto(nome)?,
            value: Secret::from_bytes(valor.to_vec()),
            secret: segredo,
        });
    }
    let mut anexos = Vec::with_capacity(b.anexos.n);
    let mut l = Leitor {
        resto: b.anexos.bytes,
    };
    for _ in 0..b.anexos.n {
        let (nome, conteudo) = ler_anexo(&mut l)?;
        anexos.push(AttachmentDraft {
            name: texto(nome)?,
            content: Secret::from_bytes(conteudo.to_vec()),
        });
    }
    let rascunho = Draft {
        name: texto(b.nome)?,
        group: texto(b.grupo)?,
        environment: texto(b.ambiente)?,
        project: texto(b.projeto)?,
        tags,
        data: montar_dados(b.tipo, &b.dados)?,
        extras,
        attachments: anexos,
    };
    let mut entrada = rascunho.validate().map_err(|_| ErroCorpo::Invalido)?;
    if entrada.kind() != b.tipo {
        return Err(ErroCorpo::Invalido);
    }
    entrada.set_level(b.nivel);
    entrada.set_dates(b.datas);
    Ok(entrada)
}
