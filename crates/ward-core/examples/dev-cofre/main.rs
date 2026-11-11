//! `make dev-vault` (Fase 19, V2.29): monta o cofre de teste a partir de
//! `dev/fixtures/entries.json` (D15 + `acme-totp-dev` de D28) pela API pública do
//! `ward-core` (validação da Fase 15, corpo v4 com a configuração de D30 e a geração de
//! D39, `write_entries` de D27/D44) e o reabre com a senha de teste para conferir. O
//! cofre recém-criado está sempre na geração 1, e as entradas nascem sem as datas de
//! D65 d (só o daemon carimba, pelo relógio dele; a tela não mostra o rodapé). O PIN de
//! teste da fixture só entra no cofre como hash Argon2id (D30); nunca em claro.
//!
//! Uso (da raiz; o `make` passa os argumentos):
//!
//! ```text
//! dev-cofre [--output .dev/<nome>.ward]              cria (ou recria) o cofre
//! dev-cofre [--output …] --conferir [--senha-stdin]  só abre e confere
//! dev-cofre [--output …] --cruzado < padroes.tsv     teste cruzado (V2.57)
//! ```
//!
//! `--cruzado` (Fase 19e, D54, V2.57): lê do stdin, uma por linha, `entrada<TAB>tipo<TAB>
//! padrão em hex` (gerado por `dev/tests/dev-vault.sh` a partir do `dev/variations.py`),
//! abre o cofre de teste e passa cada padrão, sozinho, pela máscara da entrada de onde
//! ele veio (com a entrega dela, montada com recursos falsos). `tipo` `inteiro`: a saída
//! tem de ser `••••`. `tipo` `url` (o `DATABASE_URL`, que depende do host e da porta de
//! quem roda): tem de ter `••••` e nenhum trecho de 6+ bytes de segredo da entrada. Só
//! imprime contagens, nomes de entrada e o número do padrão; nunca o padrão.
//!
//! Regras (S15, R19):
//! - A saída tem de ficar dentro de `<raiz>/.dev/`. A conferência é feita primeiro só
//!   no texto do caminho (nada fora do repositório é sequer consultado no disco) e
//!   depois no disco: nenhum componente pode ser link simbólico, e o destino, se
//!   existir, tem de ser arquivo comum.
//! - Entradas lidas só de dentro do repositório: a chave privada em
//!   `.dev/services/keys/` (gerada por `make dev-services`) e os anexos em
//!   `dev/fixtures/attachments/`, sem link simbólico.
//! - Nenhum segredo vai para argv, stdout, stderr ou mensagem de erro: só nomes de
//!   entrada, de campo e níveis, e da configuração só os parâmetros do hash do PIN (não
//!   o hash), o tempo de trava e o estado do backup.
//!
//! Exemplo do `ward-core` (não é binário instalado): só existe para o desenvolvimento.

use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{self, DirBuilder, File};
use std::io::{self, Read};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use ward_core::atomic_write::write_entries;
use ward_core::configuracao::{Configuracao, HashPin, KDF_PIN};
use ward_core::corpo::{abrir_entradas, serializar};
use ward_core::entrada::{
    Entrada, NivelAcesso, Rascunho, RascunhoAnexo, RascunhoDados, RascunhoExtra, Segredo,
    TipoEntrada,
};
use ward_core::entrega::{self, ErroEntrega, ErroRecurso, Recursos};
use ward_core::format::{MasterPassword, OpenVault};
use ward_core::limits::{DEFAULT_LOCK_MINUTES, MAX_ATTACHMENTS_TOTAL};
use ward_core::mascara::{MARCA, Mascara};
use zeroize::Zeroizing;

#[cfg(test)]
use ward_core as wc;

#[cfg(test)]
#[path = "../../src/corpo/cofre_teste.rs"]
mod cofre_teste;

const FIXTURES: &str = "dev/fixtures/entries.json";
/// Geração de um cofre recém-gravado (D39): a primeira gravação de um `OpenVault` novo.
const GERACAO_NOVA: u64 = 1;
const SAIDA_PADRAO: &str = ".dev/test-vault.ward";
const DIR_DEV: &str = ".dev";
const BASE_CHAVES: &str = ".dev/services/keys";
const BASE_ANEXOS: &str = "dev/fixtures/attachments";

const MAX_FIXTURES: u64 = 1024 * 1024;
const MAX_CHAVE: u64 = 64 * 1024;
/// Limite de leitura de um anexo: o de `ward_core::limits` (D29, D32; a validação
/// confere a soma).
const MAX_ANEXO: u64 = MAX_ATTACHMENTS_TOTAL as u64;
/// Corpo de 64 MiB (D26) cifrado, com folga para o cabeçalho e a tag.
const MAX_COFRE: u64 = 65 * 1024 * 1024;

fn main() -> ExitCode {
    match executar(env::args_os().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("dev-cofre: {e}");
            ExitCode::FAILURE
        }
    }
}

// --- Erros (só texto fixo, nomes de entrada/campo e caminhos do repositório) ---------

#[derive(Debug)]
enum Erro {
    Uso(&'static str),
    /// Caminho de saída recusado.
    Saida(&'static str),
    /// Arquivo de entrada (chave, anexo, fixtures) recusado ou ilegível.
    Arquivo {
        qual: String,
        motivo: &'static str,
    },
    Json {
        pos: usize,
        motivo: &'static str,
    },
    /// Fixture com forma errada: onde (entrada.campo) e por quê, sem o valor.
    Fixture {
        onde: String,
        motivo: String,
    },
    Cofre(String),
}

impl fmt::Display for Erro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Erro::Uso(m) => write!(
                f,
                "{m}\nuso: dev-cofre [--output .dev/<nome>.ward] [--conferir [--senha-stdin] | --cruzado]"
            ),
            Erro::Saida(m) => write!(f, "caminho de saída recusado: {m}"),
            Erro::Arquivo { qual, motivo } => write!(f, "{qual}: {motivo}"),
            Erro::Json { pos, motivo } => write!(f, "{FIXTURES}: byte {pos}: {motivo}"),
            Erro::Fixture { onde, motivo } => write!(f, "{FIXTURES}: {onde}: {motivo}"),
            Erro::Cofre(m) => f.write_str(m),
        }
    }
}

fn fixture(onde: impl Into<String>, motivo: impl Into<String>) -> Erro {
    Erro::Fixture {
        onde: onde.into(),
        motivo: motivo.into(),
    }
}

// --- Linha de comando ----------------------------------------------------------------

#[derive(Debug, Default)]
struct Opcoes {
    saida: Option<OsString>,
    conferir: bool,
    senha_stdin: bool,
    cruzado: bool,
}

fn ler_opcoes(args: impl Iterator<Item = OsString>) -> Result<Opcoes, Erro> {
    let mut o = Opcoes::default();
    let mut args = args;
    while let Some(a) = args.next() {
        match a.to_str() {
            Some("--output") => {
                let v = args.next().ok_or(Erro::Uso("--output sem caminho"))?;
                if o.saida.replace(v).is_some() {
                    return Err(Erro::Uso("--output repetido"));
                }
            }
            Some("--conferir") => o.conferir = true,
            Some("--senha-stdin") => o.senha_stdin = true,
            Some("--cruzado") => o.cruzado = true,
            _ => return Err(Erro::Uso("argumento desconhecido")),
        }
    }
    if o.cruzado && (o.conferir || o.senha_stdin) {
        return Err(Erro::Uso(
            "--cruzado não vale com --conferir nem --senha-stdin",
        ));
    }
    if o.senha_stdin && !o.conferir {
        return Err(Erro::Uso("--senha-stdin só vale com --conferir"));
    }
    Ok(o)
}

fn executar(args: impl Iterator<Item = OsString>) -> Result<(), Erro> {
    let opcoes = ler_opcoes(args)?;
    let raiz = raiz()?;
    let pedido = opcoes
        .saida
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| OsString::from(SAIDA_PADRAO));
    let saida = validar_saida(&raiz, &pedido)?;
    let exibir = saida.strip_prefix(&raiz).unwrap_or(&saida).display();

    let fixtures = ler_fixtures(&raiz)?;
    let mut ler = |origem: Origem, caminho: &str| ler_do_repositorio(&raiz, origem, caminho);
    let (senha, pin, esperadas) = montar(&fixtures, &mut ler)?;
    drop(fixtures);

    if opcoes.cruzado {
        conferir(&saida, &senha, &pin, &esperadas)?;
        return cruzado(&saida, &senha);
    }

    if opcoes.conferir {
        let senha = if opcoes.senha_stdin {
            senha_do_stdin()?
        } else {
            senha
        };
        let (n, geracao) = conferir(&saida, &senha, &pin, &esperadas)?;
        println!("dev-cofre: {exibir} abre com a senha e tem as {n} entradas das fixtures");
        imprimir_configuracao(geracao);
        imprimir_lista(&esperadas);
        return Ok(());
    }

    // D30: o cofre nasce com a configuração (hash do PIN de teste, trava padrão, backup
    // desligado). O PIN só existe aqui, em memória que zera ao sair.
    let hash = HashPin::criar(pin.as_bytes())
        .map_err(|e| Erro::Cofre(format!("não deu para criar o hash do PIN: {e}")))?;
    let mut cofre = OpenVault::new(&senha, Configuracao::nova(hash))
        .map_err(|e| Erro::Cofre(format!("não deu para criar o cofre: {e}")))?;
    write_entries(&saida, &mut cofre, &esperadas)
        .map_err(|e| Erro::Cofre(format!("não deu para gravar {exibir}: {e}")))?;
    drop(cofre);
    let (n, geracao) = conferir(&saida, &senha, &pin, &esperadas)?;
    // D39: o cofre recém-criado está na geração 1.
    if geracao != GERACAO_NOVA {
        return Err(Erro::Cofre(
            "o cofre gravado não está na geração de um cofre novo".into(),
        ));
    }
    println!("dev-cofre: gravado {exibir} (0600) com {n} entradas; reaberto e conferido");
    imprimir_configuracao(geracao);
    imprimir_lista(&esperadas);
    Ok(())
}

/// O que a conferência garantiu da configuração (nada disso é segredo; o hash não sai).
fn imprimir_configuracao(geracao: u64) {
    println!(
        "dev-cofre: configuração: geração {geracao}, hash do PIN Argon2id (m={} KiB, t={}, p={}), trava em {} min, backup externo desligado",
        KDF_PIN.memory_kib, KDF_PIN.passes, KDF_PIN.parallelism, DEFAULT_LOCK_MINUTES
    );
}

/// Nome, tipo e nível de cada entrada (nada disso é segredo).
fn imprimir_lista(entradas: &[Entrada]) {
    let mut nomes: Vec<&Entrada> = entradas.iter().collect();
    nomes.sort_by(|a, b| a.nome().cmp(b.nome()));
    for e in nomes {
        println!("  {}\t{}\t{}", e.nome(), e.tipo().nome(), e.nivel().nome());
    }
}

/// Senha do stdin (só para o teste de senha errada), sem eco e sem argv.
fn senha_do_stdin() -> Result<MasterPassword, Erro> {
    let mut buf = Zeroizing::new(Vec::with_capacity(256));
    io::stdin()
        .take(256)
        .read_to_end(&mut buf)
        .map_err(|_| Erro::Uso("não deu para ler a senha do stdin"))?;
    while buf.last().is_some_and(|b| *b == b'\n' || *b == b'\r') {
        buf.pop();
    }
    let mut bytes = Zeroizing::new(Vec::with_capacity(buf.len()));
    bytes.extend_from_slice(&buf);
    Ok(MasterPassword::from_bytes(bytes))
}

/// Reabre o arquivo e confere que as entradas voltam iguais às das fixtures (mesmos
/// bytes do corpo canônico, então mesmos campos, segredos e níveis) e que a
/// configuração é a do cofre de teste (D30): o PIN da fixture confere no hash, um PIN
/// errado não, o hash usa os parâmetros padrão, a trava é a padrão e o backup está
/// desligado.
fn conferir(
    saida: &Path,
    senha: &MasterPassword,
    pin: &str,
    esperadas: &[Entrada],
) -> Result<(usize, u64), Erro> {
    let meta = fs::symlink_metadata(saida)
        .map_err(|_| Erro::Cofre("o cofre não existe; rode make dev-vault".into()))?;
    if !meta.file_type().is_file() {
        return Err(Erro::Cofre("o cofre não é um arquivo comum".into()));
    }
    if meta.permissions().mode() & 0o077 != 0 {
        return Err(Erro::Cofre(
            "o cofre está aberto para grupo ou outros".into(),
        ));
    }
    if meta.len() > MAX_COFRE {
        return Err(Erro::Cofre("o cofre passa do tamanho máximo".into()));
    }
    let bytes = fs::read(saida).map_err(|_| Erro::Cofre("não deu para ler o cofre".into()))?;
    let (cofre, lidas) = abrir_entradas(&bytes, senha)
        .map_err(|e| Erro::Cofre(format!("o cofre não abriu: {e}")))?;
    // D39: a geração lida (≥ 1; a leitura recusa 0) entra nos dois lados da comparação.
    let geracao = cofre.generation();
    let cfg = cofre.config();
    let a = serializar(cfg, geracao, &lidas).map_err(|e| Erro::Cofre(format!("{e}")))?;
    let b = serializar(cfg, geracao, esperadas).map_err(|e| Erro::Cofre(format!("{e}")))?;
    if *a != *b {
        return Err(Erro::Cofre(
            "o cofre abre, mas as entradas diferem das fixtures; rode make dev-vault".into(),
        ));
    }
    conferir_configuracao(cfg, pin)?;
    Ok((lidas.len(), geracao))
}

/// A configuração lida é a do cofre de teste (sem dizer nada do PIN nem do hash).
fn conferir_configuracao(cfg: &Configuracao, pin: &str) -> Result<(), Erro> {
    let errado = Zeroizing::new(pin_vizinho(pin));
    if !cfg.conferir_pin(pin.as_bytes()) || cfg.conferir_pin(errado.as_bytes()) {
        return Err(Erro::Cofre(
            "o hash do PIN do cofre não é o do PIN das fixtures; rode make dev-vault".into(),
        ));
    }
    if cfg.hash_pin().parametros() != KDF_PIN
        || cfg.minutos_trava() != DEFAULT_LOCK_MINUTES
        || cfg.backup().ligado()
        || cfg.backup().uuid().is_some()
    {
        return Err(Erro::Cofre(
            "a configuração do cofre não é a padrão; rode make dev-vault".into(),
        ));
    }
    Ok(())
}

// --- Teste cruzado oráculo → máscara (V2.57) -----------------------------------------

/// Recursos falsos para montar a entrega de cada entrada (o código TOTP e o caminho não
/// entram na máscara; a porta do túnel é qualquer uma).
struct RecursosFalsos;

impl Recursos for RecursosFalsos {
    fn codigo_totp(&mut self, _: &Segredo) -> Result<Zeroizing<String>, ErroRecurso> {
        Ok(Zeroizing::new(String::from("000000")))
    }
    fn arquivo(&mut self, nome: &str, _: &Segredo) -> Result<String, ErroRecurso> {
        Ok(format!("/run/user/0/ward-teste/{nome}"))
    }
    fn tunel(&mut self, _: &str, _: &str, _: u16) -> Result<u16, ErroRecurso> {
        Ok(40123)
    }
}

/// Maior padrão aceito numa linha do stdin (o maior anexo de D29 em hex, com folga).
const MAX_LINHA_CRUZADO: usize = 4 * MAX_ATTACHMENTS_TOTAL + 1024;

fn hex_para_bytes(h: &[u8]) -> Option<Zeroizing<Vec<u8>>> {
    if !h.len().is_multiple_of(2) {
        return None;
    }
    let digito = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut v = Zeroizing::new(Vec::with_capacity(h.len() / 2));
    for par in h.chunks(2) {
        v.push((digito(par[0])? << 4) | digito(par[1])?);
    }
    Some(v)
}

/// Algum trecho de 6 bytes ou mais de algum segredo da entrada sobra na saída.
fn sobra_segredo(saida: &[u8], e: &Entrada) -> bool {
    e.segredos().iter().any(|s| {
        let v = s.valor.expor();
        v.len() >= 6 && v.windows(6).any(|j| saida.windows(6).any(|w| w == j))
    })
}

/// V2.57: cada padrão do oráculo, sozinho, sai `••••` pela máscara da entrada dele.
fn cruzado(saida: &Path, senha: &MasterPassword) -> Result<(), Erro> {
    let bytes = fs::read(saida).map_err(|_| Erro::Cofre("não deu para ler o cofre".into()))?;
    let (_, lidas) = abrir_entradas(&bytes, senha)
        .map_err(|e| Erro::Cofre(format!("o cofre não abriu: {e}")))?;
    let mut tsv = Zeroizing::new(Vec::new());
    io::stdin()
        .take(64 * 1024 * 1024)
        .read_to_end(&mut tsv)
        .map_err(|_| Erro::Uso("não deu para ler os padrões do stdin"))?;
    let mut mascaras: Vec<(String, Mascara)> = Vec::new();
    let (mut total, mut falhas) = (0usize, 0usize);
    let mut nomes = std::collections::BTreeSet::new();
    for (i, linha) in tsv.split(|b| *b == b'\n').enumerate() {
        if linha.is_empty() {
            continue;
        }
        let mut partes = linha.splitn(3, |b| *b == b'\t');
        let (Some(nome), Some(tipo), Some(hex)) = (partes.next(), partes.next(), partes.next())
        else {
            return Err(Erro::Uso("linha do stdin sem entrada, tipo e padrão"));
        };
        if hex.len() > MAX_LINHA_CRUZADO {
            return Err(Erro::Uso("padrão grande demais no stdin"));
        }
        let nome = String::from_utf8_lossy(nome).into_owned();
        let Some(e) = lidas.iter().find(|e| e.nome() == nome) else {
            return Err(Erro::Cofre(format!(
                "cruzado: entrada {nome} não está no cofre"
            )));
        };
        let Some(padrao) = hex_para_bytes(hex) else {
            return Err(Erro::Uso("padrão do stdin não está em hex"));
        };
        if !mascaras.iter().any(|(n, _)| *n == nome) {
            let g = match entrega::montar(e, &mut RecursosFalsos) {
                Ok(g) => Some(g),
                Err(ErroEntrega::TipoSemEntrega) => None,
                Err(_) => return Err(Erro::Cofre(format!("cruzado: entrega de {nome} falhou"))),
            };
            let m = Mascara::da_entrada(e, g.as_ref())
                .map_err(|_| Erro::Cofre(format!("cruzado: máscara de {nome} não montou")))?;
            mascaras.push((nome.clone(), m));
        }
        let m = &mascaras.iter().find(|(n, _)| *n == nome).unwrap().1;
        let fora = Zeroizing::new(m.mascarar_tudo(&padrao));
        let ok = match tipo {
            b"inteiro" => fora.as_slice() == MARCA.as_bytes(),
            b"url" => {
                fora.windows(MARCA.len()).any(|w| w == MARCA.as_bytes()) && !sobra_segredo(&fora, e)
            }
            _ => return Err(Erro::Uso("tipo de padrão desconhecido no stdin")),
        };
        total += 1;
        nomes.insert(nome.clone());
        if !ok {
            falhas += 1;
            eprintln!(
                "dev-cofre: cruzado: o padrão da linha {} ({}) de {nome} não saiu {MARCA}",
                i + 1,
                String::from_utf8_lossy(tipo)
            );
        }
    }
    if total == 0 {
        return Err(Erro::Uso("nenhum padrão no stdin"));
    }
    if falhas > 0 {
        return Err(Erro::Cofre(format!(
            "cruzado: {falhas} de {total} padrões não saíram {MARCA}"
        )));
    }
    println!(
        "dev-cofre: cruzado: os {total} padrões de {} entradas saem {MARCA} pela máscara da entrada",
        nomes.len()
    );
    Ok(())
}

/// O PIN com o último dígito trocado (para conferir que o hash recusa um PIN errado).
fn pin_vizinho(pin: &str) -> String {
    let mut s = String::with_capacity(pin.len());
    let n = pin.len();
    for (i, c) in pin.chars().enumerate() {
        if i + 1 == n {
            s.push(if c == '0' { '1' } else { '0' });
        } else {
            s.push(c);
        }
    }
    s
}

// --- Caminhos ---------------------------------------------------------------------------

/// Raiz do repositório (dois acima deste crate), já canônica.
fn raiz() -> Result<PathBuf, Erro> {
    let r = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|_| Erro::Cofre("raiz do repositório não encontrada".into()))?;
    if !r.join("Cargo.toml").is_file() || !r.join(FIXTURES).is_file() {
        return Err(Erro::Cofre("raiz do repositório sem as fixtures".into()));
    }
    Ok(r)
}

/// Caminho absoluto só pelo texto: relativo é relativo à raiz; `..` é recusado.
fn absoluto_lexical(raiz: &Path, pedido: &Path) -> Option<PathBuf> {
    if pedido
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return None;
    }
    let p = if pedido.is_absolute() {
        pedido.to_path_buf()
    } else {
        raiz.join(pedido)
    };
    // `components()` já tira os `.` do meio; refaz o caminho sem eles.
    Some(p.components().collect())
}

/// Confere o caminho de saída (V2.29: recusa fora de `.dev/`).
fn validar_saida(raiz: &Path, pedido: &OsStr) -> Result<PathBuf, Erro> {
    let pedido = Path::new(pedido);
    let abs = absoluto_lexical(raiz, pedido).ok_or(Erro::Saida("não pode ter '..'"))?;
    let dev = raiz.join(DIR_DEV);
    // 1) Só texto: nada fora do repositório é consultado no disco.
    let rel = abs
        .strip_prefix(&dev)
        .map_err(|_| Erro::Saida("tem de ficar dentro de .dev/ do repositório"))?;
    let nome = rel
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or(Erro::Saida("sem nome de arquivo"))?;
    if rel.as_os_str().is_empty() || nome.starts_with('.') || !nome.ends_with(".ward") {
        return Err(Erro::Saida(
            "o nome tem de terminar em .ward e não começar com '.'",
        ));
    }
    if nome.len() <= ".ward".len() {
        return Err(Erro::Saida("nome vazio"));
    }
    // 2) Disco: `.dev` é diretório de verdade (criado 0700 se faltar), filho direto da
    //    raiz, e nenhum componente até o destino é link simbólico.
    match fs::symlink_metadata(&dev) {
        Ok(m) if m.file_type().is_dir() => {}
        Ok(_) => return Err(Erro::Saida(".dev não é um diretório (link simbólico?)")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            DirBuilder::new()
                .mode(0o700)
                .create(&dev)
                .map_err(|_| Erro::Saida("não deu para criar .dev/"))?;
        }
        Err(_) => return Err(Erro::Saida("não deu para ler .dev/")),
    }
    if dev.canonicalize().ok().as_deref() != Some(dev.as_path()) {
        return Err(Erro::Saida(".dev não fica na raiz do repositório"));
    }
    let mut atual = dev.clone();
    let partes: Vec<Component<'_>> = rel.components().collect();
    let (ultimo, meio) = partes
        .split_last()
        .ok_or(Erro::Saida("sem nome de arquivo"))?;
    for c in meio {
        atual.push(c);
        match fs::symlink_metadata(&atual) {
            Ok(m) if m.file_type().is_dir() => {}
            Ok(_) => return Err(Erro::Saida("um diretório do caminho é link simbólico")),
            Err(_) => return Err(Erro::Saida("diretório do caminho não existe")),
        }
    }
    atual.push(ultimo);
    match fs::symlink_metadata(&atual) {
        Ok(m) if m.file_type().is_file() => {}
        Ok(_) => return Err(Erro::Saida("o destino existe e não é arquivo comum")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err(Erro::Saida("não deu para ler o destino")),
    }
    Ok(atual)
}

/// De onde vem um arquivo citado nas fixtures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origem {
    Chave,
    Anexo,
}

/// Lê um arquivo citado nas fixtures: relativo à raiz, dentro da base da origem, sem
/// `..` nem link simbólico em nenhum componente, arquivo comum, até o limite.
fn ler_do_repositorio(raiz: &Path, origem: Origem, caminho: &str) -> Result<Segredo, Erro> {
    let (base, max, falta) = match origem {
        Origem::Chave => (
            BASE_CHAVES,
            MAX_CHAVE,
            "não existe; rode make dev-services antes",
        ),
        Origem::Anexo => (BASE_ANEXOS, MAX_ANEXO, "não existe"),
    };
    let erro = |motivo| Erro::Arquivo {
        qual: caminho.to_owned(),
        motivo,
    };
    let pedido = Path::new(caminho);
    if pedido.is_absolute() {
        return Err(erro("tem de ser relativo à raiz do repositório"));
    }
    let abs = absoluto_lexical(raiz, pedido).ok_or_else(|| erro("não pode ter '..'"))?;
    let rel = abs
        .strip_prefix(raiz.join(base))
        .map_err(|_| erro("fora da pasta permitida"))?;
    if rel.as_os_str().is_empty() {
        return Err(erro("sem nome de arquivo"));
    }
    let mut atual = raiz.to_path_buf();
    let partes: Vec<Component<'_>> = abs
        .strip_prefix(raiz)
        .map_err(|_| erro("fora do repositório"))?
        .components()
        .collect();
    let (ultimo, meio) = partes.split_last().ok_or_else(|| erro("sem nome"))?;
    for c in meio {
        atual.push(c);
        match fs::symlink_metadata(&atual) {
            Ok(m) if m.file_type().is_dir() => {}
            Ok(_) => return Err(erro("um diretório do caminho é link simbólico")),
            Err(_) => return Err(erro(falta)),
        }
    }
    atual.push(ultimo);
    let antes = match fs::symlink_metadata(&atual) {
        Ok(m) if m.file_type().is_file() => m,
        Ok(_) => return Err(erro("não é arquivo comum (link simbólico?)")),
        Err(_) => return Err(erro(falta)),
    };
    if antes.len() > max {
        return Err(erro("grande demais"));
    }
    let f = File::open(&atual).map_err(|_| erro("não deu para abrir"))?;
    let depois = f.metadata().map_err(|_| erro("não deu para ler"))?;
    // O que abriu é o mesmo arquivo que foi conferido (troca no meio é recusada).
    if (depois.dev(), depois.ino()) != (antes.dev(), antes.ino()) {
        return Err(erro("o arquivo mudou durante a leitura"));
    }
    // Capacidade exata antes de ler: o segredo não passa por realocação.
    let tam = usize::try_from(depois.len()).map_err(|_| erro("grande demais"))?;
    let mut bytes = Vec::with_capacity(tam);
    f.take(max)
        .read_to_end(&mut bytes)
        .map_err(|_| erro("não deu para ler"))?;
    let segredo = Segredo::de_bytes(bytes);
    if segredo.len() != tam {
        return Err(erro("o arquivo mudou durante a leitura"));
    }
    Ok(segredo)
}

fn ler_fixtures(raiz: &Path) -> Result<Zeroizing<Vec<u8>>, Erro> {
    let caminho = raiz.join(FIXTURES);
    let erro = |motivo| Erro::Arquivo {
        qual: FIXTURES.to_owned(),
        motivo,
    };
    let meta = fs::symlink_metadata(&caminho).map_err(|_| erro("não existe"))?;
    if !meta.file_type().is_file() {
        return Err(erro("não é arquivo comum"));
    }
    if meta.len() > MAX_FIXTURES {
        return Err(erro("grande demais"));
    }
    let tam = usize::try_from(meta.len()).map_err(|_| erro("grande demais"))?;
    let mut buf = Zeroizing::new(Vec::with_capacity(tam + 1));
    File::open(&caminho)
        .and_then(|f| f.take(MAX_FIXTURES + 1).read_to_end(&mut buf))
        .map_err(|_| erro("não deu para ler"))?;
    if buf.len() as u64 > MAX_FIXTURES {
        return Err(erro("grande demais"));
    }
    Ok(buf)
}

// --- JSON (só o que as fixtures usam; estrito) -----------------------------------------
//
// Sem dependência nova (S14): objetos, listas, textos com escapes, inteiros, booleanos
// e null. Chave repetida, número com fração/expoente, nível acima de 32, caractere de
// controle cru e qualquer byte depois do valor são recusados. Os textos nascem com a
// capacidade final (sem realocação) e zeram ao sair.

#[derive(Debug)]
enum Valor {
    Nulo,
    Bool(bool),
    Inteiro(i64),
    Texto(Zeroizing<String>),
    Lista(Vec<Valor>),
    Objeto(Vec<(String, Valor)>),
}

const MAX_NIVEL_JSON: usize = 32;

struct Leitor<'a> {
    b: &'a [u8],
    i: usize,
}

fn json(b: &[u8]) -> Result<Valor, Erro> {
    let mut l = Leitor { b, i: 0 };
    let v = l.valor(0)?;
    l.espaco();
    if l.i != b.len() {
        return Err(l.erro("sobra depois do valor"));
    }
    Ok(v)
}

impl Leitor<'_> {
    fn erro(&self, motivo: &'static str) -> Erro {
        Erro::Json {
            pos: self.i,
            motivo,
        }
    }

    fn espaco(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.b.get(self.i) {
            self.i += 1;
        }
    }

    fn literal(&mut self, s: &[u8]) -> bool {
        if self.b[self.i..].starts_with(s) {
            self.i += s.len();
            true
        } else {
            false
        }
    }

    fn valor(&mut self, nivel: usize) -> Result<Valor, Erro> {
        if nivel > MAX_NIVEL_JSON {
            return Err(self.erro("aninhamento demais"));
        }
        self.espaco();
        match self.b.get(self.i) {
            Some(b'{') => self.objeto(nivel),
            Some(b'[') => self.lista(nivel),
            Some(b'"') => Ok(Valor::Texto(self.texto()?)),
            Some(b'-' | b'0'..=b'9') => self.inteiro(),
            Some(b't') if self.literal(b"true") => Ok(Valor::Bool(true)),
            Some(b'f') if self.literal(b"false") => Ok(Valor::Bool(false)),
            Some(b'n') if self.literal(b"null") => Ok(Valor::Nulo),
            Some(_) => Err(self.erro("valor inválido")),
            None => Err(self.erro("fim inesperado")),
        }
    }

    fn objeto(&mut self, nivel: usize) -> Result<Valor, Erro> {
        self.i += 1;
        let mut campos: Vec<(String, Valor)> = Vec::new();
        self.espaco();
        if self.b.get(self.i) == Some(&b'}') {
            self.i += 1;
            return Ok(Valor::Objeto(campos));
        }
        loop {
            self.espaco();
            if self.b.get(self.i) != Some(&b'"') {
                return Err(self.erro("esperava o nome do campo"));
            }
            let chave = String::clone(&*self.texto()?);
            if campos.iter().any(|(k, _)| *k == chave) {
                return Err(self.erro("campo repetido"));
            }
            self.espaco();
            if self.b.get(self.i) != Some(&b':') {
                return Err(self.erro("esperava ':'"));
            }
            self.i += 1;
            let v = self.valor(nivel + 1)?;
            campos.push((chave, v));
            self.espaco();
            match self.b.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Valor::Objeto(campos));
                }
                _ => return Err(self.erro("esperava ',' ou '}'")),
            }
        }
    }

    fn lista(&mut self, nivel: usize) -> Result<Valor, Erro> {
        self.i += 1;
        let mut itens = Vec::new();
        self.espaco();
        if self.b.get(self.i) == Some(&b']') {
            self.i += 1;
            return Ok(Valor::Lista(itens));
        }
        loop {
            itens.push(self.valor(nivel + 1)?);
            self.espaco();
            match self.b.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Valor::Lista(itens));
                }
                _ => return Err(self.erro("esperava ',' ou ']'")),
            }
        }
    }

    fn inteiro(&mut self) -> Result<Valor, Erro> {
        let ini = self.i;
        if self.b[self.i] == b'-' {
            self.i += 1;
        }
        let dig = self.i;
        while let Some(b'0'..=b'9') = self.b.get(self.i) {
            self.i += 1;
        }
        if self.i == dig || (self.b[dig] == b'0' && self.i - dig > 1) {
            return Err(self.erro("número inválido"));
        }
        if let Some(b'.' | b'e' | b'E') = self.b.get(self.i) {
            return Err(self.erro("só números inteiros"));
        }
        std::str::from_utf8(&self.b[ini..self.i])
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .map(Valor::Inteiro)
            .ok_or_else(|| self.erro("número fora do limite"))
    }

    fn hex4(&mut self) -> Result<u32, Erro> {
        let h = self
            .b
            .get(self.i..self.i + 4)
            .ok_or_else(|| self.erro("escape \\u incompleto"))?;
        let s = std::str::from_utf8(h).map_err(|_| self.erro("escape \\u inválido"))?;
        if !s.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(self.erro("escape \\u inválido"));
        }
        self.i += 4;
        u32::from_str_radix(s, 16).map_err(|_| self.erro("escape \\u inválido"))
    }

    fn texto(&mut self) -> Result<Zeroizing<String>, Erro> {
        self.i += 1; // aspas
        // 1ª passada: acha o fim e o tamanho máximo, sem copiar.
        let mut j = self.i;
        loop {
            match self.b.get(j) {
                None => {
                    return Err(Erro::Json {
                        pos: j,
                        motivo: "texto sem fim",
                    });
                }
                Some(b'"') => break,
                Some(b'\\') => j += 2,
                Some(_) => j += 1,
            }
        }
        let mut s = Zeroizing::new(String::with_capacity(j - self.i));
        // 2ª passada: decodifica.
        let mut cru = self.i;
        while self.i < self.b.len() {
            let c = self.b[self.i];
            match c {
                b'"' => {
                    let pedaco = std::str::from_utf8(&self.b[cru..self.i])
                        .map_err(|_| self.erro("texto não é UTF-8"))?;
                    s.push_str(pedaco);
                    self.i += 1;
                    return Ok(s);
                }
                0x00..=0x1f => return Err(self.erro("caractere de controle no texto")),
                b'\\' => {
                    let pedaco = std::str::from_utf8(&self.b[cru..self.i])
                        .map_err(|_| self.erro("texto não é UTF-8"))?;
                    s.push_str(pedaco);
                    self.i += 1;
                    let e = *self
                        .b
                        .get(self.i)
                        .ok_or_else(|| self.erro("escape sem fim"))?;
                    self.i += 1;
                    let ch = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let alto = self.hex4()?;
                            let cp = if (0xd800..0xdc00).contains(&alto) {
                                if !self.literal(b"\\u") {
                                    return Err(self.erro("par substituto incompleto"));
                                }
                                let baixo = self.hex4()?;
                                if !(0xdc00..0xe000).contains(&baixo) {
                                    return Err(self.erro("par substituto inválido"));
                                }
                                0x10000 + ((alto - 0xd800) << 10) + (baixo - 0xdc00)
                            } else {
                                alto
                            };
                            char::from_u32(cp).ok_or_else(|| self.erro("escape \\u inválido"))?
                        }
                        _ => return Err(self.erro("escape inválido")),
                    };
                    s.push(ch);
                    cru = self.i;
                }
                _ => self.i += 1,
            }
        }
        Err(self.erro("texto sem fim"))
    }
}

// --- Fixtures → rascunhos → entradas -----------------------------------------------

/// Objeto JSON que se consome campo a campo; no fim, campo que sobrou é erro.
struct Obj {
    onde: String,
    campos: Vec<(String, Valor)>,
}

impl Obj {
    fn de(onde: impl Into<String>, v: Valor) -> Result<Self, Erro> {
        let onde = onde.into();
        match v {
            Valor::Objeto(campos) => Ok(Obj { onde, campos }),
            _ => Err(fixture(onde, "esperava um objeto")),
        }
    }

    fn tirar(&mut self, chave: &str) -> Option<Valor> {
        let i = self.campos.iter().position(|(k, _)| k == chave)?;
        Some(self.campos.remove(i).1)
    }

    fn texto(&mut self, chave: &str) -> Result<Option<Zeroizing<String>>, Erro> {
        match self.tirar(chave) {
            None | Some(Valor::Nulo) => Ok(None),
            Some(Valor::Texto(t)) => Ok(Some(t)),
            Some(_) => Err(fixture(format!("{}.{chave}", self.onde), "esperava texto")),
        }
    }

    /// Texto comum (não segredo); ausente = vazio.
    fn comum(&mut self, chave: &str) -> Result<String, Erro> {
        Ok(self
            .texto(chave)?
            .map(|t| String::clone(&t))
            .unwrap_or_default())
    }

    fn obrigatorio(&mut self, chave: &str) -> Result<String, Erro> {
        let v = self.comum(chave)?;
        if v.is_empty() {
            return Err(fixture(format!("{}.{chave}", self.onde), "obrigatório"));
        }
        Ok(v)
    }

    /// Porta: número ou texto; ausente = vazio (a validação usa a padrão).
    fn porta(&mut self, chave: &str) -> Result<String, Erro> {
        match self.tirar(chave) {
            None | Some(Valor::Nulo) => Ok(String::new()),
            Some(Valor::Inteiro(n)) => Ok(n.to_string()),
            Some(Valor::Texto(t)) => Ok(String::clone(&t)),
            Some(_) => Err(fixture(
                format!("{}.{chave}", self.onde),
                "esperava número ou texto",
            )),
        }
    }

    /// Segredo em texto; o `String` é movido para o `Segredo`, sem cópia.
    fn segredo(&mut self, chave: &str) -> Result<Option<Segredo>, Erro> {
        Ok(self
            .texto(chave)?
            .map(|mut t| Segredo::de_texto(std::mem::take(&mut *t))))
    }

    fn fim(self) -> Result<(), Erro> {
        match self.campos.first() {
            None => Ok(()),
            Some((k, _)) => Err(fixture(format!("{}.{k}", self.onde), "campo desconhecido")),
        }
    }
}

type Leitura<'a> = dyn FnMut(Origem, &str) -> Result<Segredo, Erro> + 'a;

/// O que as fixtures dão: senha mestra, PIN (em memória que zera ao sair) e entradas.
type Montado = (MasterPassword, Zeroizing<String>, Vec<Entrada>);

/// Monta a senha mestra, o PIN e as entradas validadas (com o nível da fixture) a
/// partir do JSON. `ler` lê os arquivos citados (chave privada, anexos).
fn montar(bytes: &[u8], ler: &mut Leitura<'_>) -> Result<Montado, Erro> {
    let mut topo = Obj::de("(raiz)", json(bytes)?)?;
    let _ = topo.texto("_sobre")?;
    match topo.tirar("versao") {
        Some(Valor::Inteiro(1)) => {}
        _ => return Err(fixture("versao", "tem de ser 1")),
    }
    let senha = topo
        .texto("senha_mestra")?
        .filter(|t| !t.is_empty())
        .ok_or_else(|| fixture("senha_mestra", "obrigatória"))?;
    let pin = topo
        .texto("pin")?
        .ok_or_else(|| fixture("pin", "obrigatório"))?;
    if pin.len() != 6 || !pin.bytes().all(|b| b.is_ascii_digit()) {
        return Err(fixture("pin", "tem de ter 6 dígitos"));
    }
    let lista = match topo.tirar("entradas") {
        Some(Valor::Lista(l)) => l,
        _ => return Err(fixture("entradas", "esperava uma lista")),
    };
    topo.fim()?;

    let mut entradas = Vec::with_capacity(lista.len());
    for (i, v) in lista.into_iter().enumerate() {
        entradas.push(entrada(i, v, ler)?);
    }
    let senha = MasterPassword::new(String::clone(&senha));
    Ok((senha, pin, entradas))
}

fn entrada(i: usize, v: Valor, ler: &mut Leitura<'_>) -> Result<Entrada, Erro> {
    let mut e = Obj::de(format!("entradas[{i}]"), v)?;
    let nome = e.obrigatorio("nome")?;
    e.onde = nome.clone();
    let tipo_txt = e.obrigatorio("tipo")?;
    let tipo = TipoEntrada::de_nome(&tipo_txt)
        .ok_or_else(|| fixture(nome.as_str(), "tipo desconhecido"))?;
    let nivel = NivelAcesso::de_nome(&e.obrigatorio("nivel")?)
        .ok_or_else(|| fixture(nome.as_str(), "nível desconhecido"))?;
    let grupo = e.comum("grupo")?;
    let ambiente = e.comum("ambiente")?;
    let projeto = e.comum("projeto")?;
    let tags = match e.tirar("tags") {
        None | Some(Valor::Nulo) => Vec::new(),
        Some(Valor::Lista(l)) => l
            .into_iter()
            .map(|t| match t {
                Valor::Texto(t) => Ok(String::clone(&t)),
                _ => Err(fixture(format!("{nome}.tags"), "esperava texto")),
            })
            .collect::<Result<_, _>>()?,
        Some(_) => return Err(fixture(format!("{nome}.tags"), "esperava lista")),
    };
    let campos = Obj::de(
        format!("{nome}.campos"),
        e.tirar("campos").unwrap_or(Valor::Objeto(Vec::new())),
    )?;
    let segredos = Obj::de(
        format!("{nome}.segredos"),
        e.tirar("segredos").unwrap_or(Valor::Objeto(Vec::new())),
    )?;
    let dados = dados(tipo, campos, segredos, ler)?;

    let mut extras = Vec::new();
    match e.tirar("extras") {
        None | Some(Valor::Nulo) => {}
        Some(Valor::Lista(l)) => {
            for (j, x) in l.into_iter().enumerate() {
                let mut x = Obj::de(format!("{nome}.extras[{j}]"), x)?;
                let nome_extra = x.obrigatorio("nome")?;
                let valor = x
                    .segredo("valor")?
                    .ok_or_else(|| fixture(format!("{nome}.extras[{j}].valor"), "obrigatório"))?;
                let segredo = match x.tirar("segredo") {
                    Some(Valor::Bool(b)) => b,
                    _ => {
                        return Err(fixture(
                            format!("{nome}.extras[{j}].segredo"),
                            "esperava true ou false",
                        ));
                    }
                };
                x.fim()?;
                extras.push(RascunhoExtra {
                    nome: nome_extra,
                    valor,
                    segredo,
                });
            }
        }
        Some(_) => return Err(fixture(format!("{nome}.extras"), "esperava lista")),
    }

    let mut anexos = Vec::new();
    match e.tirar("anexos") {
        None | Some(Valor::Nulo) => {}
        Some(Valor::Lista(l)) => {
            for (j, a) in l.into_iter().enumerate() {
                let mut a = Obj::de(format!("{nome}.anexos[{j}]"), a)?;
                let nome_anexo = a.obrigatorio("nome")?;
                let arquivo = a.obrigatorio("arquivo")?;
                a.fim()?;
                anexos.push(RascunhoAnexo {
                    nome: nome_anexo,
                    conteudo: ler(Origem::Anexo, &arquivo)?,
                });
            }
        }
        Some(_) => return Err(fixture(format!("{nome}.anexos"), "esperava lista")),
    }
    e.fim()?;

    let r = Rascunho {
        nome: nome.clone(),
        grupo,
        ambiente,
        projeto,
        tags,
        dados,
        extras,
        anexos,
    };
    // A validação da Fase 15 é a mesma do app; o erro só tem textos fixos.
    let mut validada = r.validar().map_err(|e| fixture(nome, e.to_string()))?;
    // Só o daemon troca nível depois do PIN (S9); aqui é a montagem do cofre de teste,
    // que nasce com os níveis da tabela do contrato.
    validada.definir_nivel(nivel);
    Ok(validada)
}

fn dados(
    tipo: TipoEntrada,
    mut c: Obj,
    mut s: Obj,
    ler: &mut Leitura<'_>,
) -> Result<RascunhoDados, Erro> {
    let mut chave = |s: &mut Obj| -> Result<Option<Segredo>, Erro> {
        match s.texto("chave_privada_arquivo")? {
            None => Ok(None),
            Some(caminho) => ler(Origem::Chave, &caminho).map(Some),
        }
    };
    let d = match tipo {
        TipoEntrada::Login => RascunhoDados::Login {
            url: c.comum("url")?,
            usuario: c.comum("usuario")?,
            senha: s.segredo("senha")?,
            totp_semente: s.segredo("totp_semente")?,
        },
        TipoEntrada::Banco => RascunhoDados::Banco {
            motor: c.comum("motor")?,
            host: c.comum("host")?,
            porta: c.porta("porta")?,
            base: c.comum("base")?,
            usuario: c.comum("usuario")?,
            senha: s.segredo("senha")?,
            tunel: c.comum("tunel")?,
        },
        TipoEntrada::Servidor => RascunhoDados::Servidor {
            host: c.comum("host")?,
            porta: c.porta("porta")?,
            usuario: c.comum("usuario")?,
            senha: s.segredo("senha")?,
            chave_privada: chave(&mut s)?,
            passphrase: s.segredo("passphrase")?,
            salto: c.comum("salto")?,
        },
        TipoEntrada::Api => RascunhoDados::Api {
            url: c.comum("url")?,
            token: s.segredo("token")?,
            variavel: c.comum("variavel")?,
        },
        TipoEntrada::Aws => RascunhoDados::Aws {
            access_key_id: c.comum("access_key_id")?,
            secret_access_key: s.segredo("secret_access_key")?,
            regiao: c.comum("regiao")?,
            role_arn: c.comum("role_arn")?,
            session_token: s.segredo("session_token")?,
        },
        TipoEntrada::Smtp => RascunhoDados::Smtp {
            host: c.comum("host")?,
            porta: c.porta("porta")?,
            usuario: c.comum("usuario")?,
            senha: s.segredo("senha")?,
            seguranca: c.comum("seguranca")?,
        },
        TipoEntrada::FtpSftp => RascunhoDados::FtpSftp {
            protocolo: c.comum("protocolo")?,
            host: c.comum("host")?,
            porta: c.porta("porta")?,
            usuario: c.comum("usuario")?,
            senha: s.segredo("senha")?,
            chave_privada: chave(&mut s)?,
            passphrase: s.segredo("passphrase")?,
        },
        TipoEntrada::NotaSegura => RascunhoDados::NotaSegura {
            texto: s.segredo("texto")?,
        },
        TipoEntrada::Generico => RascunhoDados::Generico,
    };
    c.fim()?;
    s.fim()?;
    Ok(d)
}

#[cfg(test)]
mod testes;
