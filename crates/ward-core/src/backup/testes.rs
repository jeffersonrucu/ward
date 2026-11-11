//! Testes do backup local (Fase 45): nomes e datas, 30 versões (V7.1), nada em claro e
//! adulteração (V7.2), SIGKILL na gravação do backup (V7.3), permissões 0600/0700,
//! pasta insegura, link plantado com o nome de uma versão e o cofre atual autêntico
//! (D17). Senhas e segredos falsos do cofre de teste (D15).
//!
//! Arquivos só em `target/<perfil>/ward-teste-backup/` (dentro do repositório, fora do
//! git), apagados no fim de cada teste.

use std::io::{BufRead, BufReader};
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use base64ct::{Base64, Encoding};

use super::*;
use crate as wc;
use crate::atomic_write::{self, change_password_with_backup, write_entries_with_backup};
use crate::corpo;
use crate::entry::Entry;
use crate::format::MasterPassword;

#[allow(clippy::duplicate_mod)]
#[path = "../corpo/cofre_teste.rs"]
mod cofre_teste;
use cofre_teste::{SEGREDOS, SENHA_MESTRA, cofre_de_teste, configuracao_nova};

const NOME_FILHO: &str = "backup::testes::filho_backup";
const ENV_MODO: &str = "WARD_TESTE_BACKUP_MODO";
const ENV_DIR: &str = "WARD_TESTE_BACKUP_DIR";
const ENV_BASE: &str = "WARD_TESTE_BACKUP_BASE";
/// Se o pai morrer sem matar o filho, ele sai sozinho.
const VIDA_FILHO: Duration = Duration::from_secs(90);
/// 2026-11-09 15:30:00 UTC.
const INSTANTE: u64 = 1_794_238_200;

/// Um Argon2id real (256 MiB) por vez nestes testes.
static ARGON: Mutex<()> = Mutex::new(());

fn senha(s: &str) -> MasterPassword {
    MasterPassword::new(s.to_owned())
}

fn em(segundos: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(segundos)
}

fn uid() -> u32 {
    // O dono do diretório de teste que acabamos de criar é o euid do teste.
    fs::metadata(std::env::current_exe().unwrap().parent().unwrap())
        .unwrap()
        .uid()
}

/// Um cofre aberto novo com a senha de teste (um Argon2id).
fn cofre_novo(s: &str) -> OpenVault {
    let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
    OpenVault::new(&senha(s), configuracao_nova()).unwrap()
}

/// As entradas do cofre de teste (com os segredos falsos), montadas uma vez.
fn entradas() -> &'static [Entry] {
    static E: OnceLock<Vec<Entry>> = OnceLock::new();
    E.get_or_init(cofre_de_teste)
}

fn abrir(arquivo: &[u8], s: &str) -> Result<(OpenVault, Vec<Entry>, u64), corpo::ErroAbrir> {
    let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
    corpo::abrir_entradas_lida(arquivo, &senha(s), 0, 0)
}

fn hex_aleatorio() -> String {
    let mut b = [0u8; 12];
    crate::crypto::random(&mut b).unwrap();
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn u64_aleatorio() -> u64 {
    let mut b = [0u8; 8];
    crate::crypto::random(&mut b).unwrap();
    u64::from_le_bytes(b)
}

fn contem(palheiro: &[u8], agulha: &[u8]) -> bool {
    palheiro.windows(agulha.len()).any(|j| j == agulha)
}

/// Diretório de teste com o cofre e a pasta `backups/` (0700).
struct Dir(PathBuf);

impl Dir {
    fn novo(nome: &str) -> Self {
        let exe = std::env::current_exe().unwrap();
        let base = exe
            .parent()
            .and_then(Path::parent)
            .unwrap()
            .join("ward-teste-backup");
        fs::create_dir_all(&base).unwrap();
        let d = base.join(format!("{nome}-{}-{}", std::process::id(), hex_aleatorio()));
        fs::create_dir(&d).unwrap();
        fs::set_permissions(&d, fs::Permissions::from_mode(0o700)).unwrap();
        fs::create_dir(d.join("backups")).unwrap();
        fs::set_permissions(d.join("backups"), fs::Permissions::from_mode(0o700)).unwrap();
        Self(d)
    }

    fn cofre(&self) -> PathBuf {
        self.0.join("cofre.ward")
    }

    fn backups(&self) -> PathBuf {
        self.0.join("backups")
    }

    fn pasta(&self) -> PastaBackup {
        PastaBackup::nova(self.backups(), uid())
    }

    /// Todos os nomes na pasta de backups, em ordem.
    fn nomes(&self) -> Vec<String> {
        let mut v: Vec<_> = fs::read_dir(self.backups())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    fn temporarios(&self) -> Vec<PathBuf> {
        self.nomes()
            .into_iter()
            .filter(|n| n.starts_with(".backup.ward-tmp-"))
            .map(|n| self.backups().join(n))
            .collect()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn modo(p: &Path) -> u32 {
    fs::symlink_metadata(p).unwrap().permissions().mode() & 0o7777
}

// --- Nomes ------------------------------------------------------------------------------

#[test]
fn nome_do_instante_em_utc_e_ida_e_volta() {
    let casos = [
        (0, "ward-19700101-000000.ward"),
        (INSTANTE, "ward-20261109-153000.ward"),
        // 2000-02-29 23:59:59 (bissexto de 400 anos).
        (951_868_799, "ward-20000229-235959.ward"),
        // 2100-03-01 00:00:00 (2100 não é bissexto).
        (4_107_542_400, "ward-21000301-000000.ward"),
        // 2024-12-31 23:59:59.
        (1_735_689_599, "ward-20241231-235959.ward"),
    ];
    for (s, nome) in casos {
        let v = Versao::do_instante(em(s)).unwrap();
        assert_eq!(v.nome(), nome, "{s}");
        assert_eq!(Versao::de_nome(nome), Some(v));
    }
    let v = Versao::do_instante(em(INSTANTE)).unwrap();
    for s in [1, 9, 10, 99] {
        let n = v.com_sufixo(s).nome();
        assert_eq!(Versao::de_nome(&n), Some(v.com_sufixo(s)), "{n}");
        assert!(v.com_sufixo(s) > v);
    }
    // A ordem é a do tempo, não a do texto (`-` vem antes de `.` na tabela ASCII).
    assert!(v.com_sufixo(1) > v);
    assert!(v.com_sufixo(10) > v.com_sufixo(9));
    // Antes de 1970: sem nome.
    assert_eq!(
        Versao::do_instante(UNIX_EPOCH - Duration::from_secs(1)),
        None
    );
}

/// O nome do pedido de restaurar nunca vira caminho: só o formato exato.
#[test]
fn ataque_nome_fora_do_formato_recusado() {
    for ruim in [
        "",
        "ward-20261109-153000",
        "ward-20261109-153000.ward/",
        "../ward-20261109-153000.ward",
        "ward-20261109-153000.ward/../../cofre.ward",
        "/home/x/.local/share/ward/cofre.ward",
        "backups/ward-20261109-153000.ward",
        "ward-20261109-153000-0.ward",
        "ward-20261109-153000-01.ward",
        "ward-20261109-153000-100.ward",
        "ward-20261109-153000-.ward",
        "ward-20261109-153000.WARD",
        "WARD-20261109-153000.ward",
        "ward-20261109-1530000.ward",
        "ward-2026119-153000.ward",
        "ward-20261309-153000.ward",
        "ward-20261100-153000.ward",
        "ward-20261132-153000.ward",
        "ward-20261109-243000.ward",
        "ward-20261109-156000.ward",
        "ward-20261109-153060.ward",
        "ward-19691231-235959.ward",
        "ward-+0261109-153000.ward",
        "ward-20261109 153000.ward",
        "ward-20261109-153000.ward\0",
        "ward-２0261109-153000.ward",
        "ward-20261109-153000.ward.ward",
        " ward-20261109-153000.ward",
        ".ward-20261109-153000.ward",
    ] {
        assert!(!nome_valido(ruim), "{ruim:?}");
    }
    assert!(nome_valido("ward-20261109-153000.ward"));
    assert!(nome_valido("ward-20261109-153000-99.ward"));
}

// --- V7.1: 30 versões --------------------------------------------------------------------

/// V7.1: 31 gravações → 30 arquivos `ward-AAAAMMDD-HHMMSS.ward`, o mais antigo saiu; cada
/// backup é a versão gravada (mesmo formato, mesma geração), 0600 numa pasta 0700.
#[test]
fn v7_1_31_gravacoes_deixam_30_versoes() {
    let d = Dir::novo("v7-1");
    let mut cofre = cofre_novo(SENHA_MESTRA);
    let pasta = d.pasta();
    let mut versoes = Vec::new();
    for i in 0..31u64 {
        let g = write_entries_with_backup(
            &d.cofre(),
            &mut cofre,
            entradas(),
            &pasta,
            em(INSTANTE + i * 61),
        )
        .unwrap();
        let v = g.backup.unwrap();
        // O backup é o mesmo arquivo que acabou de ir para o cofre.
        assert_eq!(
            fs::read(pasta.caminho(v)).unwrap(),
            fs::read(d.cofre()).unwrap()
        );
        versoes.push(v);
    }
    let nomes = d.nomes();
    assert_eq!(nomes.len(), VERSOES, "{nomes:?}");
    assert!(!nomes.contains(&versoes[0].nome()), "a mais antiga ficou");
    assert!(!fs::exists(pasta.caminho(versoes[0])).unwrap());
    for v in &versoes[1..] {
        assert!(nomes.contains(&v.nome()));
        let p = pasta.caminho(*v);
        assert_eq!(modo(&p), 0o600, "{v}");
        assert!(cofre.authenticates(&fs::read(&p).unwrap()), "{v}");
    }
    assert_eq!(modo(&d.backups()), 0o700);
    // A lista vem da mais nova para a mais antiga.
    let lista = pasta.listar().unwrap();
    let mut esperada: Vec<_> = versoes[1..].to_vec();
    esperada.reverse();
    assert_eq!(lista, esperada);
    // A mais nova abre com a senha, na geração 31, com as 22 entradas da fixture.
    let (_, lidas, geracao) =
        abrir(&fs::read(pasta.caminho(versoes[30])).unwrap(), SENHA_MESTRA).unwrap();
    assert_eq!(geracao, 31);
    assert_eq!(lidas.len(), entradas().len());
    // A segunda mais antiga que ficou é a geração 2.
    let (_, _, g2) = abrir(&fs::read(pasta.caminho(versoes[1])).unwrap(), SENHA_MESTRA).unwrap();
    assert_eq!(g2, 2);
}

/// Várias versões no mesmo segundo ganham sufixo e nenhuma sobrescreve outra.
#[test]
fn mesmo_segundo_ganha_sufixo_sem_sobrescrever() {
    let d = Dir::novo("sufixo");
    let mut cofre = cofre_novo(SENHA_MESTRA);
    let pasta = d.pasta();
    let mut arquivos = Vec::new();
    for _ in 0..3 {
        let g =
            write_entries_with_backup(&d.cofre(), &mut cofre, &[], &pasta, em(INSTANTE)).unwrap();
        arquivos.push((g.backup.unwrap(), fs::read(d.cofre()).unwrap()));
    }
    let nomes: Vec<_> = arquivos.iter().map(|(v, _)| v.nome()).collect();
    assert_eq!(
        nomes,
        [
            "ward-20261109-153000.ward",
            "ward-20261109-153000-1.ward",
            "ward-20261109-153000-2.ward"
        ]
    );
    for (v, bytes) in &arquivos {
        assert_eq!(&fs::read(pasta.caminho(*v)).unwrap(), bytes);
    }
    assert!(d.temporarios().is_empty());
}

// --- V7.2: nada em claro, senha errada e adulteração -----------------------------------

/// V7.2 (S6): o backup não tem nenhum segredo em claro nem em base64; com senha errada
/// não abre; 1 byte alterado em cada posição não autentica (AEAD com a chave de dados,
/// sem Argon2 por posição; a abertura completa por senha de cada posição é o V2.2, no
/// mesmo formato) e, com a senha certa, um byte de cada campo do cabeçalho e do corpo
/// faz a abertura falhar.
#[test]
fn v7_2_backup_cifrado_senha_errada_e_byte_alterado() {
    let d = Dir::novo("v7-2");
    let mut cofre = cofre_novo(SENHA_MESTRA);
    let pasta = d.pasta();
    let g = write_entries_with_backup(&d.cofre(), &mut cofre, entradas(), &pasta, em(INSTANTE))
        .unwrap();
    let arquivo = fs::read(pasta.caminho(g.backup.unwrap())).unwrap();
    for s in SEGREDOS {
        assert!(
            !contem(&arquivo, s.as_bytes()),
            "segredo em claro no backup"
        );
        let b64 = Base64::encode_string(s.as_bytes());
        assert!(
            !contem(&arquivo, &b64.as_bytes()[..b64.len() - 4]),
            "segredo em base64"
        );
    }
    assert!(!contem(&arquivo, cofre_teste::PIN_TESTE.as_bytes()));

    assert!(matches!(
        abrir(&arquivo, "falso-mestra-ward-02"),
        Err(corpo::ErroAbrir::Formato(_))
    ));
    assert!(cofre.authenticates(&arquivo));
    for i in 0..arquivo.len() {
        let mut a = arquivo.clone();
        a[i] ^= 1 << (i % 8);
        assert!(
            !cofre.authenticates(&a),
            "autenticou com o byte {i} trocado"
        );
    }
    // Abertura por senha com um byte trocado em cada campo: magia, versão, memória do
    // Argon2, sal, nonce da chave, chave embrulhada, nonce dos dados, tamanho, corpo e
    // tag.
    for i in [0, 8, 12, 30, 60, 100, 140, 155, 200, arquivo.len() - 1] {
        let mut a = arquivo.clone();
        a[i] ^= 0x01;
        assert!(
            abrir(&a, SENHA_MESTRA).is_err(),
            "abriu com o byte {i} trocado"
        );
    }
    // Controle: o original abre.
    assert!(abrir(&arquivo, SENHA_MESTRA).is_ok());
}

/// Troca de senha com backup: a versão nova só abre com a senha nova; a anterior, que
/// ficou na pasta, só com a antiga (D17: restaurar pede a senha daquela versão).
#[test]
fn troca_de_senha_vai_para_o_backup_com_a_senha_nova() {
    let d = Dir::novo("troca");
    let mut cofre = cofre_novo(SENHA_MESTRA);
    let pasta = d.pasta();
    let antiga = write_entries_with_backup(&d.cofre(), &mut cofre, &[], &pasta, em(INSTANTE))
        .unwrap()
        .backup
        .unwrap();
    let nova_senha = "falso-mestra-ward-03";
    let (novo, g) = {
        let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
        change_password_with_backup(
            &d.cofre(),
            &senha(SENHA_MESTRA),
            &senha(nova_senha),
            1,
            1,
            &pasta,
            em(INSTANTE + 1),
        )
        .unwrap()
    };
    let v = g.backup.unwrap();
    let bytes = fs::read(pasta.caminho(v)).unwrap();
    assert!(novo.authenticates(&bytes));
    assert!(abrir(&bytes, SENHA_MESTRA).is_err());
    assert_eq!(abrir(&bytes, nova_senha).unwrap().2, 2);
    assert_eq!(
        abrir(&fs::read(pasta.caminho(antiga)).unwrap(), SENHA_MESTRA)
            .unwrap()
            .2,
        1
    );
}

// --- Pasta, links e o cofre atual -------------------------------------------------------

/// Pasta com permissão para grupo ou outros: nada é gravado nem listado, e o cofre é
/// gravado assim mesmo (o backup nunca impede a gravação).
#[test]
fn ataque_pasta_com_permissao_aberta_recusada() {
    let d = Dir::novo("pasta-aberta");
    let mut cofre = cofre_novo(SENHA_MESTRA);
    let pasta = d.pasta();
    for m in [0o750, 0o705, 0o755, 0o777, 0o701] {
        fs::set_permissions(d.backups(), fs::Permissions::from_mode(m)).unwrap();
        let g =
            write_entries_with_backup(&d.cofre(), &mut cofre, &[], &pasta, em(INSTANTE)).unwrap();
        assert_eq!(g.backup, Err(ErroBackup::PastaInsegura), "{m:o}");
        assert_eq!(pasta.listar(), Err(ErroBackup::PastaInsegura), "{m:o}");
        assert!(fs::exists(d.cofre()).unwrap());
    }
    fs::set_permissions(d.backups(), fs::Permissions::from_mode(0o700)).unwrap();
    assert!(d.nomes().is_empty(), "{:?}", d.nomes());
    // Pasta que não existe e caminho que é arquivo.
    let sem = PastaBackup::nova(d.0.join("nao-existe"), uid());
    assert_eq!(sem.listar(), Err(ErroBackup::PastaInsegura));
    let arq = PastaBackup::nova(d.cofre(), uid());
    assert_eq!(arq.listar(), Err(ErroBackup::PastaInsegura));
    // Outro dono: o uid esperado não bate.
    let outro = PastaBackup::nova(d.backups(), uid().wrapping_add(1));
    assert_eq!(outro.listar(), Err(ErroBackup::PastaInsegura));
}

/// Link simbólico plantado com o nome da versão (e com o de uma antiga): o backup não
/// segue nem sobrescreve o alvo, a lista ignora o link e a rotação não o apaga.
#[test]
fn ataque_link_com_nome_de_versao_nao_e_seguido() {
    let d = Dir::novo("link-versao");
    let mut cofre = cofre_novo(SENHA_MESTRA);
    let pasta = d.pasta();
    let alvo = d.0.join("alvo");
    fs::write(&alvo, b"conteudo do alvo").unwrap();
    let base = Versao::do_instante(em(INSTANTE)).unwrap();
    symlink(&alvo, pasta.caminho(base)).unwrap();
    let antigo = Versao::do_instante(em(1_000_000_000)).unwrap();
    symlink(&alvo, pasta.caminho(antigo)).unwrap();

    let g = write_entries_with_backup(&d.cofre(), &mut cofre, &[], &pasta, em(INSTANTE)).unwrap();
    assert_eq!(g.backup, Ok(base.com_sufixo(1)));
    assert_eq!(fs::read(&alvo).unwrap(), b"conteudo do alvo");
    assert_eq!(pasta.listar().unwrap(), [base.com_sufixo(1)]);
    // Enche além das 30: os links continuam lá, e o alvo, intacto.
    for i in 1..=35u64 {
        write_entries_with_backup(&d.cofre(), &mut cofre, &[], &pasta, em(INSTANTE + i))
            .unwrap()
            .backup
            .unwrap();
    }
    assert_eq!(pasta.listar().unwrap().len(), VERSOES);
    assert!(
        fs::symlink_metadata(pasta.caminho(antigo))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read(&alvo).unwrap(), b"conteudo do alvo");
}

/// D17: o cofre atual só vai para o backup se foi selado por este cofre aberto. Outro
/// cofre (outra chave), um arquivo adulterado ou bytes quaisquer são recusados.
#[test]
fn ataque_guardar_atual_so_aceita_o_arquivo_autentico() {
    let d = Dir::novo("atual");
    let mut cofre = cofre_novo(SENHA_MESTRA);
    let pasta = d.pasta();
    atomic_write::write_entries(&d.cofre(), &mut cofre, &[]).unwrap();
    let atual = fs::read(d.cofre()).unwrap();

    let mut outro = cofre_novo(SENHA_MESTRA);
    let d2 = Dir::novo("atual-outro");
    atomic_write::write_entries(&d2.cofre(), &mut outro, &[]).unwrap();
    let do_outro = fs::read(d2.cofre()).unwrap();
    let mut adulterado = atual.clone();
    let ultimo = adulterado.len() - 1;
    adulterado[ultimo] ^= 1;
    for ruim in [&do_outro, &adulterado, &b"WARDCOFR qualquer".to_vec()] {
        assert_eq!(
            guardar_atual(&pasta, &cofre, ruim, em(INSTANTE)),
            Err(ErroBackup::NaoAutentico)
        );
    }
    assert!(d.nomes().is_empty());
    let v = guardar_atual(&pasta, &cofre, &atual, em(INSTANTE)).unwrap();
    assert_eq!(fs::read(pasta.caminho(v)).unwrap(), atual);
    assert_eq!(modo(&pasta.caminho(v)), 0o600);
}

// --- Fase 49a: datas futuras no nome e geração selada ----------------------------------

/// Fase 49a (achado médio do M7): 30 arquivos com nome de data futura plantados na pasta
/// (qualquer processo do usuário pode) não fazem a rotação apagar as versões reais, nem
/// as futuras contam para as 30. A lista do restaurar ignora as que passam de agora +
/// [`MARGEM_FUTURO`]; as dentro da margem aparecem, mas a geração selada as denuncia.
#[test]
fn ataque_versoes_com_data_futura_nao_apagam_as_reais() {
    let d = Dir::novo("futuro");
    let mut cofre = cofre_novo(SENHA_MESTRA);
    let pasta = d.pasta();
    let mut reais = Vec::new();
    for i in 0..5u64 {
        let g =
            write_entries_with_backup(&d.cofre(), &mut cofre, &[], &pasta, em(INSTANTE + i * 60))
                .unwrap();
        reais.push(g.backup.unwrap());
    }
    // Uma cópia autêntica antiga (geração 1) com nomes do futuro: 30 depois da margem
    // (2099) e 30 dentro dela (alguns segundos depois da próxima gravação).
    let antiga = fs::read(pasta.caminho(reais[0])).unwrap();
    let mut longe = Vec::new();
    let mut perto = Vec::new();
    for i in 0..30u64 {
        let v = Versao::do_instante(em(4_070_908_800 + i * 3600)).unwrap();
        fs::write(pasta.caminho(v), &antiga).unwrap();
        longe.push(v);
        let v = Versao::do_instante(em(INSTANTE + 5 * 60 + 1 + i)).unwrap();
        fs::write(pasta.caminho(v), &antiga).unwrap();
        perto.push(v);
    }
    let agora = em(INSTANTE + 5 * 60);
    let g = write_entries_with_backup(&d.cofre(), &mut cofre, &[], &pasta, agora).unwrap();
    reais.push(g.backup.unwrap());
    // Nenhuma real apagada, nenhuma plantada apagada.
    for v in reais.iter().chain(&longe).chain(&perto) {
        assert!(fs::exists(pasta.caminho(*v)).unwrap(), "{v} sumiu");
    }
    // A lista do restaurar: sem as de 2099; as da margem aparecem por cima.
    let lista = pasta.listar_ate(agora).unwrap();
    assert!(longe.iter().all(|v| !lista.contains(v)));
    assert!(reais.iter().all(|v| lista.contains(v)));
    assert_eq!(lista.len(), reais.len() + perto.len());
    // A geração selada denuncia as plantadas: geração 1, mais velha que a real mais nova.
    let mais_nova = fs::read(pasta.caminho(*reais.last().unwrap())).unwrap();
    assert_eq!(geracao_selada(&cofre, &mais_nova), Some(6));
    for v in &perto {
        let b = fs::read(pasta.caminho(*v)).unwrap();
        assert_eq!(geracao_selada(&cofre, &b), Some(1), "{v}");
    }
    // Mais 40 gravações reais: ficam as 30 reais mais novas; as futuras não contam e
    // continuam lá (e as de 2099, nunca na lista).
    for i in 0..40u64 {
        let g = write_entries_with_backup(
            &d.cofre(),
            &mut cofre,
            &[],
            &pasta,
            em(INSTANTE + 3600 + i * 60),
        )
        .unwrap();
        reais.push(g.backup.unwrap());
    }
    let ate_agora: Vec<Versao> = pasta
        .listar()
        .unwrap()
        .into_iter()
        .filter(|v| *v <= *reais.last().unwrap())
        .collect();
    assert_eq!(ate_agora.len(), VERSOES);
    let mut esperadas: Vec<Versao> = reais[reais.len() - VERSOES..].to_vec();
    esperadas.reverse();
    assert_eq!(ate_agora, esperadas);
    for v in &longe {
        assert!(fs::exists(pasta.caminho(*v)).unwrap());
    }
}

/// Planta `bytes` na pasta com o nome da versão `v`, 0600 (qualquer processo do
/// usuário pode).
fn plantar(pasta: &PastaBackup, v: Versao, bytes: &[u8]) {
    let p = pasta.caminho(v);
    fs::write(&p, bytes).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
}

/// Fase 54c (achado da Fase 54), ataque: um processo do usuário planta na pasta, com
/// datas mais novas que as versões reais (mas passadas, até a próxima gravação), 36
/// arquivos com nome de versão: cabeçalho válido com o corpo trocado, versão de outro
/// cofre, cópias da versão real mais antiga e cópias da mais nova. Antes da correção, a
/// gravação seguinte apagava as reais (a rotação contava o lixo pela data). Agora
/// nenhuma real sai; o lixo e o outro cofre nunca saem; as cópias saem só com a geração
/// delas, quando ela fica abaixo das 30.
#[test]
fn f54c_ataque_lixo_com_data_mais_nova_nao_apaga_versoes_reais() {
    let d = Dir::novo("f54c-lixo");
    let mut cofre = cofre_novo(SENHA_MESTRA);
    let pasta = d.pasta();
    let gravar = |cofre: &mut OpenVault, t: u64| {
        write_entries_with_backup(&d.cofre(), cofre, &[], &pasta, em(t))
            .unwrap()
            .backup
            .unwrap()
    };
    let mut reais: Vec<Versao> = (0..5u64)
        .map(|i| gravar(&mut cofre, INSTANTE + i * 60))
        .collect();
    let antiga = fs::read(pasta.caminho(reais[0])).unwrap();
    let recente = fs::read(pasta.caminho(reais[4])).unwrap();
    let mut adulterada = recente.clone();
    let n = adulterada.len();
    adulterada[n - 3] ^= 0x5a;
    let mut outro = cofre_novo(SENHA_MESTRA);
    let d2 = Dir::novo("f54c-lixo-outro");
    atomic_write::write_entries(&d2.cofre(), &mut outro, &[]).unwrap();
    let de_outro = fs::read(d2.cofre()).unwrap();
    let (mut lixos, mut copias) = (Vec::new(), Vec::new());
    for i in 0..36u64 {
        // Entre a última real (INSTANTE + 240) e a próxima gravação (INSTANTE + 600).
        let v = Versao::do_instante(em(INSTANTE + 300 + i)).unwrap();
        match i % 4 {
            0 => {
                plantar(&pasta, v, &adulterada);
                lixos.push(v);
            }
            1 => {
                plantar(&pasta, v, &de_outro);
                lixos.push(v);
            }
            2 => {
                plantar(&pasta, v, &antiga);
                copias.push(v);
            }
            _ => {
                plantar(&pasta, v, &recente);
                copias.push(v);
            }
        }
    }
    assert!(crate::format::read_header(&adulterada).is_ok());
    // A próxima gravação real: nada sai (6 gerações).
    reais.push(gravar(&mut cofre, INSTANTE + 600));
    for v in reais.iter().chain(&lixos).chain(&copias) {
        assert!(fs::exists(pasta.caminho(*v)).unwrap(), "{v} sumiu");
    }
    // Mais 29 reais (35 gerações): ficam as 30 reais mais novas; o lixo e o outro
    // cofre continuam; as cópias das gerações 1 e 5 saem com elas.
    for i in 0..29u64 {
        reais.push(gravar(&mut cofre, INSTANTE + 3600 + i * 60));
    }
    for v in &reais[..5] {
        assert!(!fs::exists(pasta.caminho(*v)).unwrap(), "{v} ficou");
    }
    for v in &reais[5..] {
        assert!(fs::exists(pasta.caminho(*v)).unwrap(), "{v} real sumiu");
    }
    for v in &lixos {
        assert!(fs::exists(pasta.caminho(*v)).unwrap(), "{v}: lixo apagado");
    }
    for v in &copias {
        assert!(!fs::exists(pasta.caminho(*v)).unwrap(), "{v}: cópia ficou");
    }
    assert_eq!(pasta.listar().unwrap().len(), VERSOES + lixos.len());
}

/// Fase 54c: as versões de antes de uma troca de senha mestra (outra chave de dados)
/// não conferem pela chave nova: não contam para as 30 e a rotação nunca as apaga (o
/// aviso da tela "Trocar senha" diz para o usuário apagá-las, se a senha vazou).
#[test]
fn f54c_versoes_de_antes_da_troca_de_senha_nao_contam_nem_saem() {
    let d = Dir::novo("f54c-troca");
    let mut cofre = cofre_novo(SENHA_MESTRA);
    let pasta = d.pasta();
    let antigas: Vec<Versao> = (0..3u64)
        .map(|i| {
            write_entries_with_backup(&d.cofre(), &mut cofre, &[], &pasta, em(INSTANTE + i))
                .unwrap()
                .backup
                .unwrap()
        })
        .collect();
    let (mut novo, g) = {
        let _g = ARGON.lock().unwrap_or_else(|e| e.into_inner());
        change_password_with_backup(
            &d.cofre(),
            &senha(SENHA_MESTRA),
            &senha("falso-mestra-ward-03"),
            3,
            3,
            &pasta,
            em(INSTANTE + 60),
        )
        .unwrap()
    };
    let mut novas = vec![g.backup.unwrap()];
    for i in 0..35u64 {
        let g = write_entries_with_backup(
            &d.cofre(),
            &mut novo,
            &[],
            &pasta,
            em(INSTANTE + 120 + i * 60),
        )
        .unwrap();
        novas.push(g.backup.unwrap());
    }
    for v in &antigas {
        assert!(
            fs::exists(pasta.caminho(*v)).unwrap(),
            "{v}: antiga apagada"
        );
    }
    for v in &novas[..novas.len() - VERSOES] {
        assert!(!fs::exists(pasta.caminho(*v)).unwrap(), "{v} ficou");
    }
    for v in &novas[novas.len() - VERSOES..] {
        assert!(fs::exists(pasta.caminho(*v)).unwrap(), "{v} sumiu");
    }
}

/// Fase 54c: relógio que volta. A gravação com o relógio uma hora atrasado não apaga
/// nenhuma versão (as de data depois dela não contam: falha fechada) e fica; quando o
/// relógio volta ao certo, a rotação segue pela geração, e a atrasada (geração mais
/// alta que as de antes) fica entre as 30, mesmo com a data mais antiga da pasta.
#[test]
fn f54c_relogio_que_volta_nao_apaga_versao_real() {
    let d = Dir::novo("f54c-relogio");
    let mut cofre = cofre_novo(SENHA_MESTRA);
    let pasta = d.pasta();
    let mut gravar = |t: u64| {
        write_entries_with_backup(&d.cofre(), &mut cofre, &[], &pasta, em(t))
            .unwrap()
            .backup
            .unwrap()
    };
    let mut reais: Vec<Versao> = (0..30u64).map(|i| gravar(INSTANTE + i * 60)).collect();
    let atrasada = gravar(INSTANTE - 3600);
    reais.push(atrasada);
    for v in &reais {
        assert!(fs::exists(pasta.caminho(*v)).unwrap(), "{v} sumiu");
    }
    // O relógio certo de novo: uma gravação (32 gerações); saem as gerações 1 e 2.
    reais.push(gravar(INSTANTE + 7200));
    for v in &reais[..2] {
        assert!(!fs::exists(pasta.caminho(*v)).unwrap(), "{v} ficou");
    }
    for v in &reais[2..] {
        assert!(fs::exists(pasta.caminho(*v)).unwrap(), "{v} sumiu");
    }
    assert_eq!(pasta.listar().unwrap().len(), VERSOES);
}

/// A regra da rotação, sozinha: versões passadas nunca saem por causa das futuras, as
/// protegidas nunca saem, e sem futuras é a rotação de antes (as mais antigas). Aqui a
/// geração anda com a data (a versão `i` tem a geração `i + 1`).
#[test]
fn rotacao_conta_so_ate_o_teto() {
    let v = |s: u64| Versao::do_instante(em(INSTANTE + s)).unwrap();
    let com_g = |l: &[Versao], g0: u64| -> Vec<(Versao, Option<u64>)> {
        l.iter()
            .enumerate()
            .map(|(i, x)| (*x, Some(g0 + i as u64)))
            .collect()
    };
    let passadas: Vec<Versao> = (0..35).map(|i| v(i * 10)).collect();
    let teto = passadas[34];
    let futuras: Vec<Versao> = (1..=40).map(|i| v(340 + i)).collect();
    let mut lista = com_g(&passadas, 1);
    lista.extend(com_g(&futuras, 36));
    let fora = para_apagar_conferidas(&lista, teto, &[teto]);
    assert_eq!(fora, passadas[..5].to_vec());
    // Só futuras além do teto: nada sai.
    assert!(para_apagar_conferidas(&com_g(&futuras, 36), teto, &[]).is_empty());
    // 31 passadas e o teto na mais antiga: só ela conta, nada sai.
    assert!(
        para_apagar_conferidas(&com_g(&passadas[..31], 1), passadas[0], &[passadas[0]]).is_empty()
    );
    // Protegida entre as mais antigas: pula ela e não apaga outra no lugar (a rotação
    // não tira uma geração que ficaria entre as 30).
    let fora = para_apagar_conferidas(&com_g(&passadas[..31], 1), passadas[30], &[passadas[0]]);
    assert!(fora.is_empty(), "{fora:?}");
    // O teto do segundo cobre os sufixos do mesmo segundo.
    let base = v(0);
    assert!(base.com_sufixo(99) <= base.fim_do_segundo());
    assert!(v(1) > base.fim_do_segundo());
}

/// Fase 54c, a regra sozinha: o que não confere (`None`) nunca conta nem sai; cada
/// geração conta uma vez (cópias com nomes novos não ocupam lugar) e sai inteira
/// quando fica abaixo das 30; a data não decide a ordem (a geração decide), então um
/// relógio que voltou não tira a versão nova.
#[test]
fn f54c_rotacao_so_conta_e_so_apaga_o_que_confere() {
    let v = |s: u64| Versao::do_instante(em(INSTANTE + s)).unwrap();
    // 30 reais (gerações 1..=30), datas 0..290.
    let reais: Vec<(Versao, Option<u64>)> = (0..30u64).map(|i| (v(i * 10), Some(i + 1))).collect();
    let teto = v(10_000);
    // 40 lixos com datas mais novas: nada sai.
    let mut lista = reais.clone();
    lista.extend((0..40u64).map(|i| (v(300 + i), None)));
    assert!(para_apagar_conferidas(&lista, teto, &[]).is_empty());
    // 40 cópias da real mais nova (geração 30) com datas mais novas: uma geração só.
    let mut lista = reais.clone();
    lista.extend((0..40u64).map(|i| (v(400 + i), Some(30))));
    assert!(para_apagar_conferidas(&lista, teto, &[]).is_empty());
    // 40 cópias da real mais antiga com datas mais novas e uma real nova: sai a
    // geração 1 inteira (a original e as cópias), nenhuma outra real.
    let mut lista = reais.clone();
    let copias: Vec<Versao> = (0..40u64).map(|i| v(500 + i)).collect();
    lista.extend(copias.iter().map(|c| (*c, Some(1))));
    let nova = v(600);
    lista.push((nova, Some(31)));
    let mut esperado = vec![reais[0].0];
    esperado.extend(&copias);
    assert_eq!(para_apagar_conferidas(&lista, teto, &[nova]), esperado);
    // Relógio que voltou: a nova (geração 31) com a data mais antiga de todas fica; sai
    // a geração 1.
    let mut lista = reais.clone();
    let atrasada = Versao::do_instante(em(INSTANTE - 3600)).unwrap();
    lista.push((atrasada, Some(31)));
    assert_eq!(
        para_apagar_conferidas(&lista, teto, &[atrasada]),
        [reais[0].0]
    );
    // Sem nada que confere: nada sai.
    let lixo: Vec<(Versao, Option<u64>)> = (0..100u64).map(|i| (v(i), None)).collect();
    assert!(para_apagar_conferidas(&lixo, teto, &[]).is_empty());
    assert!(para_apagar_conferidas(&[], teto, &[]).is_empty());
}

/// A marca de fora de ordem: a cópia antiga (geração 3) com nome novo sai marcada; as
/// reais em ordem, não; sem geração (outra chave) não conta; o sufixo desempata o
/// mesmo segundo; a ordem da lista não importa.
#[test]
fn ataque_copia_antiga_com_nome_novo_sai_marcada() {
    let v = |s: u64| Versao::do_instante(em(INSTANTE + s)).unwrap();
    let lista = [
        (v(3600), Some(3)),
        (v(60).com_sufixo(1), Some(12)),
        (v(60), Some(11)),
        (v(30), None),
        (v(0), Some(10)),
        (v(1), Some(4)),
    ];
    assert_eq!(
        fora_de_ordem(&lista),
        [true, false, false, false, false, true]
    );
    let mesmo = [(v(0), Some(12)), (v(0).com_sufixo(1), Some(11))];
    assert_eq!(fora_de_ordem(&mesmo), [false, true]);
    assert!(fora_de_ordem(&[]).is_empty());
}

/// A geração selada só sai de um arquivo da chave do cofre aberto: outro cofre (outra
/// senha), um byte trocado ou bytes quaisquer dão `None`.
#[test]
fn ataque_geracao_selada_nao_se_forja() {
    let d = Dir::novo("geracao");
    let mut cofre = cofre_novo(SENHA_MESTRA);
    for _ in 0..3 {
        atomic_write::write_entries(&d.cofre(), &mut cofre, entradas()).unwrap();
    }
    let bom = fs::read(d.cofre()).unwrap();
    assert_eq!(geracao_selada(&cofre, &bom), Some(3));
    let mut outro = cofre_novo(SENHA_MESTRA);
    let d2 = Dir::novo("geracao-outro");
    atomic_write::write_entries(&d2.cofre(), &mut outro, &[]).unwrap();
    assert_eq!(geracao_selada(&cofre, &fs::read(d2.cofre()).unwrap()), None);
    for i in [0, 100, 159, 160, bom.len() - 1] {
        let mut ruim = bom.clone();
        ruim[i] ^= 1;
        assert_eq!(geracao_selada(&cofre, &ruim), None, "byte {i}");
    }
    assert_eq!(geracao_selada(&cofre, b"WARDCOFR qualquer"), None);
}

// --- V7.3: SIGKILL na gravação do backup -------------------------------------------------

/// Pontos da gravação do backup, na ordem; os quatro primeiros vêm antes do link.
const PONTOS: [Point; 5] = [
    Point::AfterCreate,
    Point::AfterWrite,
    Point::AfterFsync,
    Point::AfterRename,
    Point::AfterFsyncDir,
];
const ANTES_DO_LINK: usize = 3;

/// Processo filho dos testes de morte. Não faz nada sem o modo no ambiente. Sem o
/// cofre aberto (o Argon2 por filho seria lento demais), a geração de cada versão vem
/// da tabela `v0..`: a versão `k` tem a geração `k + 1` (o pai as selou em ordem, com o
/// mesmo cofre, [`versoes_seladas`]).
#[test]
#[ignore = "só roda como processo filho dos testes de backup"]
fn filho_backup() {
    let Ok(modo) = std::env::var(ENV_MODO) else {
        return;
    };
    let dir = PathBuf::from(std::env::var_os(ENV_DIR).unwrap());
    let base: u64 = std::env::var(ENV_BASE).unwrap().parse().unwrap();
    let pasta = PastaBackup::nova(dir.join("backups"), uid());
    let versoes: Vec<Vec<u8>> = (0..)
        .map_while(|k| fs::read(dir.join(format!("v{k}"))).ok())
        .collect();
    let geracao = |b: &[u8]| {
        versoes
            .iter()
            .position(|v| v.as_slice() == b)
            .map(|i| i as u64 + 1)
    };
    if let Some(i) = modo.strip_prefix("ponto:") {
        let i: usize = i.parse().unwrap();
        let _ = pasta.guardar_com_pontos(&versoes[1], em(base), &geracao, &mut |p| {
            if p == PONTOS[i] {
                println!("\nWARD-PONTO");
                std::thread::sleep(VIDA_FILHO);
                std::process::exit(3);
            }
            Ok(())
        });
        println!("\nWARD-FIM");
    } else if modo == "laco" {
        // `base` é a primeira versão a gravar; cada uma com a data do seu número, para a
        // data andar com a geração, como na gravação de verdade.
        println!("\nWARD-PRONTO");
        let inicio = Instant::now();
        for k in base as usize.. {
            let Some(v) = versoes.get(k) else {
                println!("\nWARD-ACABOU");
                std::process::exit(4);
            };
            pasta
                .guardar_com_pontos(v, em(INSTANTE + k as u64), &geracao, &mut |_| Ok(()))
                .unwrap();
            if inicio.elapsed() > VIDA_FILHO {
                std::process::exit(3);
            }
        }
    } else {
        panic!("modo desconhecido");
    }
}

fn filho(modo: &str, dir: &Path, base: u64) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            NOME_FILHO,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(ENV_MODO, modo)
        .env(ENV_DIR, dir)
        .env(ENV_BASE, base.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

fn esperar_linha(f: &mut Child, esperada: &'static str) {
    let saida = f.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for l in BufReader::new(saida).lines() {
            let Ok(l) = l else { break };
            if l.trim_end() == esperada || l.ends_with(esperada) {
                let _ = tx.send(());
            }
        }
    });
    if rx.recv_timeout(Duration::from_secs(60)).is_err() {
        let _ = f.kill();
        let st = f.wait();
        panic!("o filho não chegou a {esperada}: {st:?}");
    }
}

fn matar(mut f: Child) {
    f.kill().unwrap(); // SIGKILL
    let st = f.wait().unwrap();
    assert!(!st.success(), "o filho tinha de morrer pelo SIGKILL");
}

/// Três versões seladas (com os segredos falsos do cofre de teste) e o cofre que as
/// selou, gravadas em `v0..v2` no diretório de teste.
fn tres_versoes(d: &Dir) -> (OpenVault, Vec<Vec<u8>>) {
    versoes_seladas(d, 3)
}

/// `n` versões seladas em ordem por um cofre novo (a `k` com a geração `k + 1`), em
/// `v0..` no diretório de teste. Sela direto, sem gravar o cofre (sem fsync por versão).
fn versoes_seladas(d: &Dir, n: usize) -> (OpenVault, Vec<Vec<u8>>) {
    let mut cofre = cofre_novo(SENHA_MESTRA);
    let mut versoes = Vec::new();
    for k in 0..n {
        let b = selar_proxima(&mut cofre, entradas());
        fs::write(d.0.join(format!("v{k}")), &b).unwrap();
        versoes.push(b);
    }
    (cofre, versoes)
}

/// Sela a próxima geração do `cofre` com `entradas`, como a gravação faz, sem gravar.
fn selar_proxima(cofre: &mut OpenVault, entradas: &[Entry]) -> Vec<u8> {
    let g = cofre.next_generation().unwrap();
    let conteudo = corpo::serializar(cofre.config(), g, entradas).unwrap();
    let b = cofre.seal(&conteudo).unwrap();
    cofre.burn(g);
    b
}

/// Confere a pasta depois de uma morte: todo arquivo com nome de versão é uma das
/// versões inteiras (abre), 0600; o temporário que sobrou é prefixo do cifrado.
fn conferir_pasta(d: &Dir, versoes: &[Vec<u8>]) -> (usize, usize) {
    let pasta = d.pasta();
    let lista = pasta.listar().unwrap();
    for v in &lista {
        let p = pasta.caminho(*v);
        let b = fs::read(&p).unwrap();
        assert!(
            versoes.contains(&b),
            "{v}: o backup não é uma versão inteira"
        );
        assert_eq!(modo(&p), 0o600, "{v}");
    }
    let temps = d.temporarios();
    assert!(temps.len() <= 1, "{temps:?}");
    for t in &temps {
        let b = fs::read(t).unwrap();
        assert!(
            versoes.iter().any(|v| v.starts_with(&b)),
            "temporário não é prefixo"
        );
        for s in SEGREDOS {
            assert!(!contem(&b, s.as_bytes()), "temporário em claro");
        }
        assert_eq!(modo(t), 0o600);
    }
    (lista.len(), temps.len())
}

/// V7.3 (S7): SIGKILL em cada ponto da gravação do backup. Antes do link, nenhum nome
/// novo aparece e os backups de antes ficam iguais; depois, o novo está inteiro. A
/// próxima gravação apaga o temporário que sobrou.
#[test]
fn v7_3_sigkill_em_cada_ponto_do_backup() {
    for (i, alvo) in PONTOS.iter().enumerate() {
        let d = Dir::novo("v7-3-ponto");
        let (cofre, versoes) = tres_versoes(&d);
        let pasta = d.pasta();
        let antes = pasta.guardar(&versoes[0], em(INSTANTE), &cofre).unwrap();
        let mut f = filho(&format!("ponto:{i}"), &d.0, INSTANTE + 60);
        esperar_linha(&mut f, "WARD-PONTO");
        matar(f);
        let (n, temps) = conferir_pasta(&d, &versoes);
        assert_eq!(
            fs::read(pasta.caminho(antes)).unwrap(),
            versoes[0],
            "{alvo:?}"
        );
        if i < ANTES_DO_LINK {
            assert_eq!(n, 1, "{alvo:?}");
            assert_eq!(temps, 1, "{alvo:?}");
        } else {
            assert_eq!(n, 2, "{alvo:?}");
            assert_eq!(temps, 0, "{alvo:?}");
        }
        pasta
            .guardar(&versoes[2], em(INSTANTE + 120), &cofre)
            .unwrap();
        assert!(d.temporarios().is_empty(), "{alvo:?}: o temporário ficou");
    }
}

/// V7.3 (S7): SIGKILL em 100 pontos aleatórios de um filho que grava backups sem parar
/// (com a rotação das 30): todo backup que ficou é uma versão inteira e abre; nenhum
/// backup some além da rotação.
#[test]
fn v7_3_sigkill_em_100_pontos_aleatorios() {
    const MORTES: u64 = 100;
    /// Versões distintas para os filhos gravarem (cada filho grava poucas antes de
    /// morrer; a que sobrar não é usada).
    const TABELA: usize = 5000;
    let d = Dir::novo("v7-3-aleatorio");
    let (mut cofre, versoes) = versoes_seladas(&d, TABELA);
    let pasta = d.pasta();
    // Duração de um backup aqui (com a rotação lendo a pasta cheia), para espalhar as
    // mortes por várias gravações.
    for (k, v) in versoes.iter().enumerate().take(VERSOES) {
        pasta.guardar(v, em(INSTANTE + k as u64), &cofre).unwrap();
    }
    let t0 = Instant::now();
    for (k, v) in versoes.iter().enumerate().skip(VERSOES).take(3) {
        pasta.guardar(v, em(INSTANTE + k as u64), &cofre).unwrap();
    }
    let uma = (t0.elapsed() / 3).max(Duration::from_millis(1));
    let janela = (uma * 4).as_micros() as u64;

    let mut com_temporario = 0;
    let mut antes = pasta.listar().unwrap().len();
    // A próxima versão a gravar: depois da mais alta que está na pasta.
    let proxima = |p: &PastaBackup| {
        p.listar()
            .unwrap()
            .iter()
            .filter_map(|v| {
                let b = fs::read(p.caminho(*v)).ok()?;
                versoes.iter().position(|x| *x == b)
            })
            .max()
            .unwrap()
            + 1
    };
    for m in 0..MORTES {
        let inicio = proxima(&pasta);
        assert!(inicio < TABELA, "a tabela acabou na morte {m}");
        let mut f = filho("laco", &d.0, inicio as u64);
        esperar_linha(&mut f, "WARD-PRONTO");
        std::thread::sleep(Duration::from_micros(u64_aleatorio() % janela));
        matar(f);
        let (n, temps) = conferir_pasta(&d, &versoes);
        // Uma versão só sai depois que a nova entrou (link antes da rotação).
        assert!(n >= antes.min(VERSOES), "morte {m}: {n} < {antes}");
        // Morta entre o link e a rotação, a pasta fica com uma a mais, que a próxima
        // gravação inteira tira.
        assert!(n <= antes.max(VERSOES) + 1, "morte {m}: {n}");
        antes = n;
        com_temporario += temps;
    }
    eprintln!(
        "v7_3: {MORTES} mortes, {com_temporario} no meio de um backup, backup ~{uma:?}, {} versões",
        proxima(&pasta)
    );
    assert!(
        com_temporario >= (MORTES / 10) as usize,
        "poucas mortes no meio da gravação"
    );
    // Todos os que ficaram abrem: os bytes são de uma das versões da tabela, e cada uma
    // abre com a senha.
    for v in &versoes {
        assert!(cofre.authenticates(v));
    }
    let (_, lidas, _) = abrir(&versoes[2], SENHA_MESTRA).unwrap();
    assert_eq!(lidas.len(), entradas().len());
    // A próxima gravação (geração nova) limpa o temporário e volta às 30.
    let nova = selar_proxima(&mut cofre, entradas());
    pasta
        .guardar(&nova, em(INSTANTE + 999_999), &cofre)
        .unwrap();
    assert!(d.temporarios().is_empty());
    assert_eq!(pasta.listar().unwrap().len(), VERSOES);
}
