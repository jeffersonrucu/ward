//! `ward run` (Fase 26; D10, D11, D13, D16, D52, D60, D63; S1–S4, S8, S13).
//!
//! Uma sessão por conexão do papel agente:
//! 1. `nota segura` (visível) responde `tipo_sem_entrega` antes de qualquer pergunta;
//! 2. [`Daemon::autorizar_uso`]: espera do destrave (D16), nível, aprovação (Fase 23);
//! 3. com o estado travado, a entrega de D10 ([`ward_core::entrega::montar`]): anexos e
//!    chave em `<rt>tmp/<sessão>/`, 0600, numa pasta 0700 só da sessão (S2); e a
//!    máscara da entrada e da entrega (D11);
//! 4. o ambiente do filho vem **só** do pedido da CLI (D13, D60): sem `LD_*`, sem nada
//!    do ambiente do daemon (`LISTEN_*`); a entrega por cima, com o mesmo nome
//!    substituindo o da CLI; locale com `LC_CTYPE` UTF-8 (ver [`ajustar_locale`]);
//! 5. o filho ([`ward_plataforma::filho`]): grupo de processos próprio, nenhum fd do
//!    daemon, sem shell (S3), morre com a thread da conexão;
//! 6. stdout e stderr passam pela máscara, cada um no seu fluxo, e voltam em quadros
//!    `saida`; a entrada da CLI chega pelo `entrada_padrao` (vazio = fim);
//! 7. o filho termina: o grupo inteiro é morto, o temporário é zerado e apagado e a CLI
//!    recebe `fim` com o código (128 + sinal, como o shell);
//! 8. a CLI cai (fim da conexão) ou manda um quadro fora da sessão: o grupo do filho
//!    leva `SIGKILL` (D13) e a limpeza segue igual.
//!
//! O log tem só números e textos fixos (S13). Os temporários que um `wardd` morto deixou
//! são apagados na partida ([`limpar_temporarios`]).

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use zeroize::Zeroizing;

use ward_core::entrada::{Dados, Entrada, NivelAcesso, Segredo};
use ward_core::entrega::{self, Entrega, ErroEntrega, ErroRecurso, Recursos};
use ward_core::limites::{MAX_QUADRO_COMUM, MAX_TEXTO_EXIBIDO};
use ward_core::mascara::Mascara;
use ward_core::protocolo::{
    self, ErroResposta, Fluxo, Janela, Papel, Pedido, Resposta, Variavel, quadro,
};
use ward_core::totp::Totp;
use ward_plataforma::filho::{self, Comando, Grupo, Pty};

use crate::agente_ssh::{AgenteSsh, Permitido};

use crate::aprovacao::Cliente;
use crate::nucleo::Daemon;
use crate::{aviso, depurar};

/// Pasta dos temporários dentro de `<rt>`.
pub const PASTA_TEMPORARIOS: &str = "tmp";
/// Depois do fim do filho, quanto esperar as threads da saída entregarem o resto.
const PRAZO_RESTO_DA_SAIDA: Duration = Duration::from_secs(2);
/// De quanto em quanto tempo a leitura da saída olha se deve parar.
const PASSO_LEITURA_MS: i32 = 100;
/// Pedaço lido da saída do filho.
const PEDACO: usize = 16 * 1024;
/// Quadros de entrada guardados à espera do filho ler (pressão de volta na CLI).
const FILA_ENTRADA: usize = 8;
/// Saída interativa (D34): sem saída nova por este tempo, o fim segurado sai como marca.
pub const PAUSA_INTERATIVA: Duration = Duration::from_millis(300);
/// Locale que o filho recebe quando o da CLI não é UTF-8.
const LOCALE_UTF8: &[u8] = b"C.UTF-8";

static SESSOES: AtomicU64 = AtomicU64::new(0);

// --- Temporários (S2) ----------------------------------------------------------------

/// Um arquivo da sessão, para conferir na hora de zerar.
struct Arquivo {
    caminho: PathBuf,
    dev: u64,
    ino: u64,
    tamanho: u64,
}

/// A pasta 0700 de uma sessão em `<rt>tmp/`, com os anexos 0600. Ao sair (inclusive
/// em pânico), cada arquivo é zerado e a pasta é apagada.
pub struct Temporarios {
    pasta: PathBuf,
    arquivos: Vec<Arquivo>,
}

impl std::fmt::Debug for Temporarios {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Temporarios({} arquivo(s))", self.arquivos.len())
    }
}

/// `<rt>tmp/`, conferida ou criada: `<rt>` e `tmp` 0700 do usuário, sem link
/// simbólico, com as pastas acima do root ou do usuário (as regras do diretório do
/// socket, V3.1).
pub fn pasta_temporarios(rt: &Path) -> io::Result<PathBuf> {
    let erro = |_| io::Error::from(io::ErrorKind::PermissionDenied);
    ward_plataforma::preparar_diretorio(rt).map_err(erro)?;
    let tmp = rt.join(PASTA_TEMPORARIOS);
    ward_plataforma::preparar_diretorio(&tmp).map_err(erro)?;
    Ok(tmp)
}

impl Temporarios {
    /// Pasta nova da sessão (nunca reaproveita uma que já existe).
    pub fn criar(rt: &Path) -> io::Result<Temporarios> {
        let base = pasta_temporarios(rt)?;
        let n = SESSOES.fetch_add(1, Ordering::Relaxed);
        let pasta = base.join(format!("s{}-{n}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&pasta)?;
        Ok(Temporarios {
            pasta,
            arquivos: Vec::new(),
        })
    }

    /// Grava `conteudo` em `<pasta>/<nome>`, 0600, arquivo novo (sem seguir link).
    fn gravar(&mut self, nome: &str, conteudo: &[u8]) -> io::Result<String> {
        if nome.is_empty() || nome.contains('/') || nome.starts_with('.') || nome.contains('\0') {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let caminho = self.pasta.join(nome);
        let texto = caminho
            .to_str()
            .ok_or(io::ErrorKind::InvalidInput)?
            .to_owned();
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&caminho)?;
        let m = f.metadata()?;
        self.arquivos.push(Arquivo {
            caminho,
            dev: m.dev(),
            ino: m.ino(),
            tamanho: conteudo.len() as u64,
        });
        f.write_all(conteudo)?;
        Ok(texto)
    }
}

/// Zera o arquivo da sessão, se ainda é o mesmo (mesmo inode, um link só, regular):
/// um arquivo trocado pelo filho (link para outro) não é tocado.
fn zerar(a: &Arquivo) {
    let Ok(mut f) = fs::OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&a.caminho)
    else {
        return;
    };
    let Ok(m) = f.metadata() else { return };
    if !m.file_type().is_file() || m.dev() != a.dev || m.ino() != a.ino || m.nlink() != 1 {
        return;
    }
    let zeros = [0u8; 4096];
    let mut falta = a.tamanho.min(m.len());
    while falta > 0 {
        let n = usize::try_from(falta.min(zeros.len() as u64)).unwrap_or(zeros.len());
        if f.write_all(&zeros[..n]).is_err() {
            return;
        }
        falta -= n as u64;
    }
}

impl Drop for Temporarios {
    fn drop(&mut self) {
        for a in &self.arquivos {
            zerar(a);
        }
        // `remove_dir_all` não segue link simbólico.
        if fs::remove_dir_all(&self.pasta).is_err() {
            aviso!("não deu para apagar a pasta temporária da sessão");
        }
    }
}

/// Partida do daemon (S2, V4.15): apaga o que um `wardd` morto deixou em `<rt>tmp/`,
/// zerando antes os arquivos regulares de cada pasta de sessão. Devolve quantas
/// entradas apagou.
pub fn limpar_temporarios(rt: &Path) -> io::Result<usize> {
    let base = pasta_temporarios(rt)?;
    let mut n = 0;
    for item in fs::read_dir(&base)? {
        let item = item?;
        let tipo = item.file_type()?;
        let caminho = item.path();
        if tipo.is_dir() {
            if let Ok(dentro) = fs::read_dir(&caminho) {
                for a in dentro.flatten() {
                    if let Ok(m) = fs::symlink_metadata(a.path()) {
                        zerar(&Arquivo {
                            caminho: a.path(),
                            dev: m.dev(),
                            ino: m.ino(),
                            tamanho: m.len(),
                        });
                    }
                }
            }
            fs::remove_dir_all(&caminho)?;
        } else {
            fs::remove_file(&caminho)?;
        }
        n += 1;
    }
    Ok(n)
}

// --- Entrega (D10) ---------------------------------------------------------------------

/// Os recursos do sistema da entrega: código TOTP, arquivos temporários e o túnel.
struct RecursosRun<'a> {
    rt: Option<&'a Path>,
    temporarios: Option<Temporarios>,
}

impl Recursos for RecursosRun<'_> {
    fn codigo_totp(&mut self, semente: &Segredo) -> Result<Zeroizing<String>, ErroRecurso> {
        let c = Totp::de_semente(semente)
            .and_then(|t| t.codigo_agora())
            .map_err(|_| ErroRecurso("código TOTP"))?;
        let mut s = Zeroizing::new(String::with_capacity(c.codigo().len()));
        s.push_str(c.codigo());
        Ok(s)
    }

    fn arquivo(&mut self, nome: &str, conteudo: &Segredo) -> Result<String, ErroRecurso> {
        let rt = self.rt.ok_or(ErroRecurso("sem pasta de temporários"))?;
        if self.temporarios.is_none() {
            self.temporarios =
                Some(Temporarios::criar(rt).map_err(|_| ErroRecurso("pasta de temporários"))?);
        }
        let t = self
            .temporarios
            .as_mut()
            .ok_or(ErroRecurso("pasta de temporários"))?;
        t.gravar(nome, conteudo.expor())
            .map_err(|_| ErroRecurso("arquivo temporário"))
    }

    fn tunel(&mut self, _servidor: &str, _host: &str, _porta: u16) -> Result<u16, ErroRecurso> {
        // O túnel SSH do `banco` é da Fase 28.
        Err(ErroRecurso("túnel SSH ainda não disponível"))
    }
}

/// Tudo pronto para lançar: o comando (com o ambiente, que tem segredo), a máscara, os
/// temporários (apagados ao sair) e, no `ward ssh` (Fase 27), o pty e o agente SSH.
pub(crate) struct Preparo {
    pub(crate) comando: Comando,
    pub(crate) mascara: Mascara,
    pub(crate) temporarios: Option<Temporarios>,
    /// `ward ssh` interativo: o filho num pty deste tamanho (D13, D34).
    pub(crate) pty: Option<Janela>,
    /// `ward ssh` com chave: o agente de D12, que só atende o filho.
    pub(crate) agente: Option<AgentePreparado>,
}

/// O agente SSH da sessão, pronto para receber o pid do filho (D12).
pub(crate) struct AgentePreparado {
    pub(crate) agente: AgenteSsh,
    /// `sftp`: o `ssh` é filho do filho.
    pub(crate) filho_do_filho: bool,
    /// A contagem de travas do núcleo e o valor dela quando a sessão foi autorizada.
    pub(crate) travas: Arc<AtomicU64>,
    pub(crate) inicial: u64,
}

/// Nome de variável que o daemon tira do ambiente da CLI (D13): `LD_*` (carregador
/// dinâmico).
fn variavel_proibida(nome: &[u8]) -> bool {
    nome.starts_with(b"LD_")
}

fn e_utf8(valor: &[u8]) -> bool {
    let v = valor.to_ascii_lowercase();
    v.windows(5).any(|w| w == b"utf-8") || v.windows(4).any(|w| w == b"utf8")
}

/// Locale do filho (handoff 22a, RA3): em locale C, o bash escreve `${X@Q}` e
/// `declare -p` de valor fora do ASCII como `$'…\303\247…'`, que a máscara não cobre.
/// Se o `LC_CTYPE` efetivo da CLI (`LC_ALL`, senão `LC_CTYPE`, senão `LANG`) não é
/// UTF-8, o filho recebe `C.UTF-8` só nessa variável (mensagens e formatos da CLI ficam
/// como estão). Devolve o ambiente com o ajuste: (nome, valor).
pub fn ajustar_locale<'a>(amb: Vec<(&'a [u8], &'a [u8])>) -> Vec<(&'a [u8], &'a [u8])> {
    let achar =
        |amb: &[(&[u8], &[u8])], n: &[u8]| amb.iter().position(|(k, v)| *k == n && !v.is_empty());
    let mut amb = amb;
    if let Some(i) = achar(&amb, b"LC_ALL") {
        if !e_utf8(amb[i].1) {
            amb[i].1 = LOCALE_UTF8;
        }
        return amb;
    }
    if let Some(i) = achar(&amb, b"LC_CTYPE") {
        if !e_utf8(amb[i].1) {
            amb[i].1 = LOCALE_UTF8;
        }
        return amb;
    }
    let lang_utf8 = achar(&amb, b"LANG").is_some_and(|i| e_utf8(amb[i].1));
    if !lang_utf8 {
        // Um `LC_CTYPE=` vazio sai, para não ficar repetido.
        amb.retain(|(k, _)| *k != b"LC_CTYPE");
        amb.push((b"LC_CTYPE", LOCALE_UTF8));
    }
    amb
}

/// O ambiente do filho (D13, D60): o da CLI sem `LD_*`, sem nome repetido (fica o
/// primeiro) e sem os nomes que a entrega traz; o locale ajustado; e a entrega por
/// cima. Nada vem do ambiente do daemon.
fn montar_ambiente(
    cmd: &mut Comando,
    cli: &[Variavel],
    entrega: &Entrega,
) -> Result<usize, ErroResposta> {
    let da_entrega: HashSet<&[u8]> = entrega.nomes().map(str::as_bytes).collect();
    let mut vistos: HashSet<&[u8]> = HashSet::new();
    let mut tirados = 0;
    let mut amb: Vec<(&[u8], &[u8])> = Vec::with_capacity(cli.len() + 1);
    for v in cli {
        let nome: &[u8] = &v.nome;
        if variavel_proibida(nome) {
            tirados += 1;
            continue;
        }
        if da_entrega.contains(nome) || !vistos.insert(nome) {
            continue;
        }
        amb.push((nome, v.valor.expor()));
    }
    for (k, v) in ajustar_locale(amb) {
        cmd.variavel(k, v)
            .map_err(|_| ErroResposta::PedidoInvalido)?;
    }
    for v in entrega.variaveis() {
        cmd.variavel(v.nome().as_bytes(), v.expor().as_bytes())
            .map_err(|_| ErroResposta::EntregaFalhou)?;
    }
    Ok(tirados)
}

/// A entrada visível (bloqueada = inexistente, V3.17).
fn visivel<'a>(entradas: &'a [Entrada], nome: &str) -> Option<&'a Entrada> {
    entradas
        .iter()
        .find(|e| e.nome() == nome && e.nivel() != NivelAcesso::Bloqueado)
}

impl Daemon {
    /// Passos 1 a 4: autoriza, monta a entrega, a máscara e o comando.
    fn preparar_execucao(
        &self,
        cliente: &dyn Cliente,
        pedido: Pedido,
    ) -> Result<Preparo, ErroResposta> {
        let Pedido::Executar(x) = &pedido else {
            return Err(ErroResposta::PedidoInvalido);
        };
        // `nota segura` visível: recusa antes de pedir aprovação (V4.11).
        if let Some(a) = self.estado().aberto.as_ref()
            && visivel(&a.entradas, &x.entrada)
                .is_some_and(|e| matches!(e.dados(), Dados::NotaSegura(_)))
        {
            return Err(ErroResposta::TipoSemEntrega);
        }
        self.autorizar_uso(cliente, &pedido)?;
        let Pedido::Executar(x) = pedido else {
            return Err(ErroResposta::PedidoInvalido);
        };
        self.registrar_uso();
        let (argv0, cwd) = match (x.argv.first(), x.cwd.is_empty()) {
            (Some(a), false) => (a, &x.cwd),
            _ => return Err(ErroResposta::PedidoInvalido),
        };
        let mut comando = Comando::novo(argv0, cwd).map_err(|_| ErroResposta::PedidoInvalido)?;
        for a in &x.argv[1..] {
            comando
                .argumento(a)
                .map_err(|_| ErroResposta::PedidoInvalido)?;
        }
        let est = self.estado();
        let a = est.aberto.as_ref().ok_or(ErroResposta::Travado)?;
        // A entrada pode ter mudado durante a aprovação: confere de novo.
        let e = visivel(&a.entradas, &x.entrada).ok_or(ErroResposta::EntradaInexistente)?;
        let mut recursos = RecursosRun {
            rt: self.temporarios(),
            temporarios: None,
        };
        let entrega = match entrega::montar(e, &mut recursos) {
            Ok(g) => g,
            Err(ErroEntrega::TipoSemEntrega) => return Err(ErroResposta::TipoSemEntrega),
            Err(erro) => {
                aviso!("executar: {erro}");
                return Err(ErroResposta::EntregaFalhou);
            }
        };
        let mascara = Mascara::da_entrada(e, Some(&entrega)).map_err(|erro| {
            aviso!("executar: {erro}");
            ErroResposta::EntregaFalhou
        })?;
        drop(est);
        let tirados = montar_ambiente(&mut comando, &x.ambiente, &entrega)?;
        if tirados > 0 {
            depurar!("executar: {tirados} variável(is) LD_* tirada(s) do ambiente (D13)");
        }
        Ok(Preparo {
            comando,
            mascara,
            temporarios: recursos.temporarios,
            pty: None,
            agente: None,
        })
    }
}

// --- Sessão --------------------------------------------------------------------------

/// O lado da CLI: quadros escritos por várias threads, um inteiro por vez.
#[derive(Clone)]
pub(crate) struct Cli(Arc<Mutex<UnixStream>>);

impl Cli {
    /// Escreve a resposta; `false` se a CLI não está mais lá.
    fn enviar(&self, r: &Resposta) -> bool {
        let Ok(q) = protocolo::codificar_resposta(Papel::Agente, r) else {
            aviso!("executar: resposta recusada para o agente");
            return false;
        };
        let mut s = self.0.lock().unwrap_or_else(|e| e.into_inner());
        quadro::escrever_quadro(&mut *s, &q, quadro::LIMITE_RESPOSTA).is_ok()
    }

    /// Manda a saída já mascarada em quadros de até `MAX_TEXTO_EXIBIDO` e esvazia `out`.
    fn enviar_saida(&self, fluxo: Fluxo, out: &mut Zeroizing<Vec<u8>>) -> bool {
        let mut ok = true;
        for pedaco in out.chunks(MAX_TEXTO_EXIBIDO) {
            let mut dados = Zeroizing::new(Vec::with_capacity(pedaco.len()));
            dados.extend_from_slice(pedaco);
            if !self.enviar(&Resposta::Saida { fluxo, dados }) {
                ok = false;
                break;
            }
        }
        zeroize::Zeroize::zeroize(&mut **out);
        out.clear();
        ok
    }
}

/// Lê a saída do filho, mascara e manda à CLI (D11; um fluxo por saída). Para no fim
/// do pipe ou quando `parar` liga (um neto fora do grupo segurando o pipe). Com
/// `interativo` (o pty do `ward ssh`, D34), um fim segurado que fica
/// [`PAUSA_INTERATIVA`] sem saída nova sai como marca, nunca em claro, e o estado do
/// casamento continua.
fn ler_saida(
    mut f: File,
    fluxo: Fluxo,
    mascara: Arc<Mascara>,
    cli: Cli,
    parar: Arc<AtomicBool>,
    pronto: mpsc::Sender<()>,
    interativo: bool,
) {
    let mut m = mascara.fluxo();
    let mut buf = Zeroizing::new(vec![0u8; PEDACO]);
    let mut out = Zeroizing::new(Vec::with_capacity(2 * PEDACO));
    let mut cli_ok = true;
    let mut ultimo = Instant::now();
    loop {
        match filho::pronto_para_ler(&f, PASSO_LEITURA_MS) {
            Ok(true) => {}
            Ok(false) if !parar.load(Ordering::Acquire) => {
                if interativo && m.segurando() && ultimo.elapsed() >= PAUSA_INTERATIVA {
                    m.soltar(&mut out);
                    if cli_ok {
                        cli_ok = cli.enviar_saida(fluxo, &mut out);
                    } else {
                        zeroize::Zeroize::zeroize(&mut *out);
                    }
                }
                continue;
            }
            _ => break,
        }
        let n = match f.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            // O mestre do pty dá `EIO` quando o último processo da sessão fecha o
            // escravo: é o fim da saída.
            Err(_) => break,
        };
        ultimo = Instant::now();
        m.mascarar(&buf[..n], &mut out);
        if cli_ok {
            cli_ok = cli.enviar_saida(fluxo, &mut out);
        } else {
            zeroize::Zeroize::zeroize(&mut *out);
        }
    }
    // O que ficou segurado sai mascarado (nunca em claro).
    m.terminar(&mut out);
    if cli_ok {
        cli.enviar_saida(fluxo, &mut out);
    }
    let _ = pronto.send(());
}

/// Passa a entrada da CLI ao filho; `None` (ou a fila fechada) fecha a entrada dele.
fn escrever_entrada(mut f: File, fila: Receiver<Option<Segredo>>) {
    while let Ok(Some(dados)) = fila.recv() {
        if f.write_all(dados.expor()).is_err() {
            break;
        }
    }
}

/// Lê os quadros da CLI durante a sessão: `entrada_padrao` vai para o filho; `janela`
/// muda o tamanho do pty (sem pty, como no `ward run`, é ignorada); qualquer outro
/// quadro, ou a conexão que cai, mata o grupo do filho (D13). Depois do fim normal, a
/// leitura acaba sem matar nada. No pty, a entrada vazia não fecha nada (o fim da
/// entrada é o Ctrl+D que o terminal manda como byte).
fn ler_da_cli(
    mut s: UnixStream,
    grupo: Arc<Grupo>,
    fila: SyncSender<Option<Segredo>>,
    fim: Arc<AtomicBool>,
    pty: Option<Arc<Pty>>,
) {
    let mut entrada_aberta = true;
    loop {
        let bytes = match quadro::ler_quadro(&mut s, MAX_QUADRO_COMUM) {
            Ok(b) => b,
            Err(_) => {
                if !fim.load(Ordering::Acquire) {
                    aviso!("executar: a CLI saiu; o grupo do comando foi morto (D13)");
                    grupo.matar();
                }
                return;
            }
        };
        match protocolo::decodificar_pedido(Papel::Agente, &bytes) {
            Ok(Pedido::EntradaPadrao { dados }) => {
                if !entrada_aberta || (dados.is_empty() && pty.is_some()) {
                    continue;
                }
                let item = if dados.is_empty() {
                    entrada_aberta = false;
                    None
                } else {
                    Some(dados)
                };
                if !por_na_fila(&fila, item, &s, &fim, &grupo) {
                    return;
                }
            }
            Ok(Pedido::Janela(j)) => {
                if let Some(p) = &pty {
                    let _ = p.definir_janela(j.linhas, j.colunas);
                }
            }
            _ => {
                if !fim.load(Ordering::Acquire) {
                    aviso!("executar: quadro fora da sessão; o grupo do comando foi morto");
                    grupo.matar();
                }
                return;
            }
        }
    }
}

/// Põe na fila da entrada sem perder a CLI de vista: com a fila cheia (o filho não
/// lê), olha se a conexão caiu enquanto espera. `false`: a sessão acabou.
fn por_na_fila(
    fila: &SyncSender<Option<Segredo>>,
    mut item: Option<Segredo>,
    s: &UnixStream,
    fim: &AtomicBool,
    grupo: &Grupo,
) -> bool {
    loop {
        match fila.try_send(item) {
            Ok(()) => return true,
            Err(TrySendError::Disconnected(_)) => return true,
            Err(TrySendError::Full(de_volta)) => {
                item = de_volta;
                if fim.load(Ordering::Acquire) {
                    return false;
                }
                if ward_plataforma::par_fechou(s) {
                    aviso!("executar: a CLI saiu; o grupo do comando foi morto (D13)");
                    grupo.matar();
                    return false;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        }
    }
}

fn thread(nome: &str, f: impl FnOnce() + Send + 'static) -> bool {
    std::thread::Builder::new()
        .name(nome.into())
        .spawn(f)
        .is_ok()
}

/// Atende um `executar` do papel agente na conexão `stream`, até o fim do comando. Toda
/// resposta (erro, saída, fim) é escrita aqui. Devolve `true` se o pedido foi recusado
/// antes de lançar qualquer processo (a conexão segue, como nas outras operações);
/// `false` quando a sessão aconteceu e a conexão acabou.
pub fn sessao(daemon: &Daemon, cliente: &dyn Cliente, stream: &UnixStream, pedido: Pedido) -> bool {
    sessao_com(
        stream,
        |c| {
            let r = daemon.preparar_execucao(c, pedido);
            if let Err(e) = &r {
                depurar!("executar (agente): {}", e.codigo());
            }
            r
        },
        cliente,
    )
}

/// O lançamento: o filho com pipes ou num pty.
enum Lancado {
    Pipes(filho::Pipes),
    Pty(Arc<Pty>),
}

/// Prepara (com `preparar`) e roda a sessão de `executar` ou `ssh` na conexão: mesmo
/// retorno de [`sessao`].
pub(crate) fn sessao_com(
    stream: &UnixStream,
    preparar: impl FnOnce(&dyn Cliente) -> Result<Preparo, ErroResposta>,
    cliente: &dyn Cliente,
) -> bool {
    let Ok(escrita) = stream.try_clone() else {
        return false;
    };
    let cli = Cli(Arc::new(Mutex::new(escrita)));
    let preparo = match preparar(cliente) {
        Ok(p) => p,
        Err(e) => return cli.enviar(&Resposta::Erro(e)),
    };
    // A sessão dura o que o comando durar; a CLI pode ficar parada (pipe cheio).
    let _ = stream.set_read_timeout(None);
    let _ = stream.set_write_timeout(None);
    let Preparo {
        comando,
        mascara,
        temporarios,
        pty,
        agente,
    } = preparo;
    let lancado = match pty {
        Some(j) => comando
            .lancar_em_pty(j.linhas, j.colunas)
            .map(|(g, p)| (g, Lancado::Pty(Arc::new(p)))),
        None => comando.lancar().map(|(g, p)| (g, Lancado::Pipes(p))),
    };
    // O ambiente com segredo zera agora, não no fim do comando.
    drop(comando);
    let (grupo, lancado) = match lancado {
        Ok(g) => g,
        Err(e) => {
            depurar!("executar: o filho não foi criado ({e:?})");
            drop(temporarios);
            drop(agente);
            let mut msg = Zeroizing::new(Vec::new());
            msg.extend_from_slice(b"ward: ");
            msg.extend_from_slice(e.mensagem().as_bytes());
            msg.extend_from_slice(b"\n");
            cli.enviar(&Resposta::Saida {
                fluxo: Fluxo::Erro,
                dados: msg,
            });
            cli.enviar(&Resposta::Fim {
                codigo: e.codigo_de_saida(),
            });
            let _ = stream.shutdown(std::net::Shutdown::Both);
            return false;
        }
    };
    let pid = grupo.pid();
    depurar!("executar: filho pid {pid}");
    // D12: o agente só atende o filho que acabou de nascer (o pid não é reusado: o
    // daemon só colhe o filho no fim).
    let agente = agente.map(|a| {
        a.agente.iniciar(
            Permitido {
                pid,
                filho_do_filho: a.filho_do_filho,
            },
            a.travas,
            a.inicial,
        )
    });
    let grupo = Arc::new(grupo);
    let mascara = Arc::new(mascara);
    let parar = Arc::new(AtomicBool::new(false));
    let fim = Arc::new(AtomicBool::new(false));
    let (pronto_tx, pronto_rx) = mpsc::channel();
    let mut leitores = 0;
    let (entrada, saidas, pty) = match lancado {
        Lancado::Pipes(filho::Pipes {
            entrada,
            saida,
            erro,
        }) => (
            entrada,
            vec![
                (saida, Fluxo::Saida, "wardd-run-saida"),
                (erro, Fluxo::Erro, "wardd-run-erro"),
            ],
            None,
        ),
        Lancado::Pty(p) => {
            let leitura = p.mestre.try_clone().ok();
            let escrita = p.mestre.try_clone().ok();
            if leitura.is_none() || escrita.is_none() {
                grupo.matar();
            }
            (
                escrita,
                vec![(leitura, Fluxo::Saida, "wardd-ssh-pty")],
                Some(p),
            )
        }
    };
    let interativo = pty.is_some();
    for (f, fluxo, nome) in saidas {
        let Some(f) = f else { continue };
        let (m, c, p, t) = (
            mascara.clone(),
            cli.clone(),
            parar.clone(),
            pronto_tx.clone(),
        );
        if thread(nome, move || ler_saida(f, fluxo, m, c, p, t, interativo)) {
            leitores += 1;
        }
    }
    drop(pronto_tx);
    let (fila_tx, fila_rx) = mpsc::sync_channel(FILA_ENTRADA);
    if let Some(f) = entrada {
        thread("wardd-run-entrada", move || escrever_entrada(f, fila_rx));
    }
    let leitor_ok = match stream.try_clone() {
        Ok(s) => {
            let (g, f, p) = (grupo.clone(), fim.clone(), pty.clone());
            thread("wardd-run-cli", move || ler_da_cli(s, g, fila_tx, f, p))
        }
        Err(_) => false,
    };
    if !leitor_ok {
        // Sem ver a CLI, o comando não roda (D13: a CLI que cai mata o grupo).
        grupo.matar();
    }

    // Fim do filho: o grupo inteiro morre, o temporário e o agente somem (V4.15, D12)
    // e a saída que falta é entregue.
    let _ = grupo.esperar_fim();
    grupo.matar();
    let resultado = grupo.colher();
    drop(agente);
    drop(temporarios);
    let limite = Instant::now() + PRAZO_RESTO_DA_SAIDA;
    for _ in 0..leitores {
        let falta = limite.saturating_duration_since(Instant::now());
        if pronto_rx.recv_timeout(falta).is_err() {
            break;
        }
    }
    parar.store(true, Ordering::Release);
    drop(pty);
    let codigo = match resultado {
        Ok(f) => {
            match f {
                filho::Fim::Codigo(c) => depurar!("executar: pid {pid} saiu com código {c}"),
                filho::Fim::Sinal(s) => depurar!("executar: pid {pid} encerrado pelo sinal {s}"),
            }
            f.codigo_de_saida()
        }
        Err(_) => {
            aviso!("executar: não deu para colher o pid {pid}");
            1
        }
    };
    fim.store(true, Ordering::Release);
    cli.enviar(&Resposta::Fim { codigo });
    let _ = stream.shutdown(std::net::Shutdown::Both);
    false
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Pasta de teste em `/run/user/<uid>/` (as pastas acima de `<rt>` têm de ser do
    /// root ou do usuário, sem escrita de grupo).
    fn rt_de_teste(rotulo: &str) -> PathBuf {
        ward_plataforma::socket::diretorio_runtime()
            .join(format!("wardd-exec-{}-{rotulo}", std::process::id()))
    }

    fn amb<'a>(v: &[(&'a str, &'a str)]) -> Vec<(&'a [u8], &'a [u8])> {
        v.iter()
            .map(|(k, x)| (k.as_bytes(), x.as_bytes()))
            .collect()
    }

    fn texto(v: Vec<(&[u8], &[u8])>) -> Vec<(String, String)> {
        v.into_iter()
            .map(|(k, x)| {
                (
                    String::from_utf8(k.to_vec()).unwrap(),
                    String::from_utf8(x.to_vec()).unwrap(),
                )
            })
            .collect()
    }

    #[test]
    fn locale_utf8_fica_e_c_vira_c_utf8() {
        let s = |v: &[(&str, &str)]| texto(ajustar_locale(amb(v)));
        let par = |k: &str, v: &str| (k.to_owned(), v.to_owned());
        assert_eq!(
            s(&[("LANG", "pt_BR.UTF-8")]),
            vec![par("LANG", "pt_BR.UTF-8")]
        );
        assert_eq!(
            s(&[("LANG", "en_US.utf8")]),
            vec![par("LANG", "en_US.utf8")]
        );
        assert_eq!(
            s(&[("LANG", "C")]),
            vec![par("LANG", "C"), par("LC_CTYPE", "C.UTF-8")]
        );
        assert_eq!(s(&[]), vec![par("LC_CTYPE", "C.UTF-8")]);
        assert_eq!(
            s(&[("LC_ALL", "C"), ("LANG", "pt_BR.UTF-8")]),
            vec![par("LC_ALL", "C.UTF-8"), par("LANG", "pt_BR.UTF-8")]
        );
        assert_eq!(
            s(&[("LC_CTYPE", "POSIX"), ("LANG", "pt_BR.UTF-8")]),
            vec![par("LC_CTYPE", "C.UTF-8"), par("LANG", "pt_BR.UTF-8")]
        );
        assert_eq!(
            s(&[("LC_CTYPE", ""), ("LANG", "C")]),
            vec![par("LANG", "C"), par("LC_CTYPE", "C.UTF-8")]
        );
        assert_eq!(
            s(&[("LC_ALL", "pt_BR.UTF-8")]),
            vec![par("LC_ALL", "pt_BR.UTF-8")]
        );
    }

    #[test]
    fn ld_e_proibida() {
        assert!(variavel_proibida(b"LD_PRELOAD"));
        assert!(variavel_proibida(b"LD_LIBRARY_PATH"));
        assert!(variavel_proibida(b"LD_AUDIT"));
        assert!(!variavel_proibida(b"OLD_PWD"));
        assert!(!variavel_proibida(b"PATH"));
    }

    /// S2: o arquivo é 0600 numa pasta 0700 da sessão; ao sair, zerado e apagado; e
    /// um link trocado pelo filho não leva o daemon a zerar outro arquivo.
    #[test]
    fn temporarios_0600_zerados_e_apagados() {
        use std::os::unix::fs::PermissionsExt;
        let rt = rt_de_teste("rt");
        let _ = fs::remove_dir_all(&rt);
        fs::create_dir_all(&rt).unwrap();
        fs::set_permissions(&rt, fs::Permissions::from_mode(0o700)).unwrap();
        let alvo = rt.join("alvo-de-fora");
        fs::write(&alvo, b"nao-tocar").unwrap();
        let mut t = Temporarios::criar(&rt).unwrap();
        let p = t.gravar("app.env", b"FALSO_ENV=falso-anexo-01").unwrap();
        let p2 = t.gravar("chave", b"falso-chave").unwrap();
        assert!(t.gravar("app.env", b"x").is_err(), "nome repetido");
        assert!(t.gravar("../fora", b"x").is_err());
        assert!(t.gravar(".oculto", b"x").is_err());
        let m = fs::metadata(&p).unwrap();
        assert_eq!(m.permissions().mode() & 0o777, 0o600);
        let pasta = std::path::Path::new(&p).parent().unwrap().to_path_buf();
        assert_eq!(
            fs::metadata(&pasta).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(rt.join("tmp")).unwrap().permissions().mode() & 0o777,
            0o700
        );
        // O "filho" troca a chave por um link para um arquivo de fora.
        fs::remove_file(&p2).unwrap();
        std::os::unix::fs::symlink(&alvo, &p2).unwrap();
        drop(t);
        assert!(!pasta.exists());
        assert_eq!(fs::read(&alvo).unwrap(), b"nao-tocar");
        fs::remove_dir_all(&rt).unwrap();
    }

    /// V4.15: o que um daemon morto deixou some na partida.
    #[test]
    fn partida_limpa_o_que_ficou() {
        use std::os::unix::fs::PermissionsExt;
        let rt = rt_de_teste("rt2");
        let _ = fs::remove_dir_all(&rt);
        fs::create_dir_all(&rt).unwrap();
        fs::set_permissions(&rt, fs::Permissions::from_mode(0o700)).unwrap();
        let mut t = Temporarios::criar(&rt).unwrap();
        t.gravar("app.env", b"FALSO_ENV=falso-anexo-01").unwrap();
        std::mem::forget(t);
        assert_eq!(limpar_temporarios(&rt).unwrap(), 1);
        assert_eq!(fs::read_dir(rt.join("tmp")).unwrap().count(), 0);
        fs::remove_dir_all(&rt).unwrap();
    }

    /// `<rt>` com permissão aberta ou link simbólico é recusado (nada é gravado).
    #[test]
    fn ataque_rt_aberto_ou_link_recusado() {
        use std::os::unix::fs::PermissionsExt;
        let base = rt_de_teste("rt3");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        let rt = base.join("rt");
        fs::create_dir(&rt).unwrap();
        fs::set_permissions(&rt, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(Temporarios::criar(&rt).is_err());
        let link = base.join("link");
        std::os::unix::fs::symlink(&rt, &link).unwrap();
        fs::set_permissions(&rt, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(Temporarios::criar(&link).is_err());
        fs::remove_dir_all(&base).unwrap();
    }
}
