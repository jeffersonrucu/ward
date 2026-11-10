//! Testes do corpo do cofre (D26, D30): ida e volta (V2.30), adulteração do corpo
//! decifrado (V2.31), a seção de configuração do corpo v2 (V2.39) e recusas da gravação. Os testes que precisam de alocador global (V2.32 com
//! contagem, V2.33 com procura de segredo em bloco liberado) estão em
//! `tests/corpo_memoria.rs`. Só valores falsos (D15, D28).

use std::panic::{AssertUnwindSafe, catch_unwind};

use super::*;
use crate as wc;
use crate::entrada::{Ambiente, CampoExtra, Login, RascunhoAnexo, RascunhoDados};
use crate::formato::ParametrosKdf as ParametrosKdfPin;

#[path = "cofre_teste.rs"]
mod cofre_teste;
use cofre_teste::*;

// ---- Ajudantes ----

/// A configuração do cofre de teste (hash do PIN de teste, 240 minutos, backup
/// desligado), calculada uma vez.
fn cfg() -> &'static Configuracao {
    configuracao_de_teste()
}

fn corpo_de_teste() -> Zeroizing<Vec<u8>> {
    serializar(cfg(), 1, &cofre_de_teste()).expect("o cofre de teste serializa")
}

/// Lê sem deixar o pânico passar: um pânico vira falha do teste com mensagem própria.
fn ler_tudo(corpo: &[u8]) -> Result<(Configuracao, Vec<Entrada>), ErroCorpo> {
    match catch_unwind(AssertUnwindSafe(|| desserializar(corpo))) {
        Ok(r) => r.map(|(c, _, e)| (c, e)),
        Err(_) => panic!("o decodificador entrou em pânico"),
    }
}

/// Só as entradas (a configuração é conferida nos testes de V2.39).
fn ler(corpo: &[u8]) -> Result<Vec<Entrada>, ErroCorpo> {
    ler_tudo(corpo).map(|(_, e)| e)
}

/// O corpo tem de ser recusado com o erro genérico, sem o valor forjado no erro.
fn recusado(corpo: &[u8], caso: &str, valor: &str) {
    match ler(corpo) {
        Ok(e) => panic!("{caso}: aceito com {} entradas", e.len()),
        Err(erro) => {
            assert_eq!(erro, ErroCorpo::Invalido, "{caso}");
            for texto in [format!("{erro}"), format!("{erro:?}"), format!("{erro:#?}")] {
                assert!(
                    !valor.is_empty() && !texto.contains(valor),
                    "{caso}: erro com valor"
                );
                assert!(!texto.contains("falso"), "{caso}: erro com segredo");
            }
        }
    }
}

fn mesmo_segredo(a: &Segredo, b: &Segredo) -> bool {
    a.expor() == b.expor()
}

fn mesmo_opcional(a: &Option<Segredo>, b: &Option<Segredo>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => mesmo_segredo(a, b),
        _ => false,
    }
}

fn mesma_autenticacao(a: &Autenticacao, b: &Autenticacao) -> bool {
    match (a, b) {
        (Autenticacao::Senha(a), Autenticacao::Senha(b)) => mesmo_segredo(a, b),
        (
            Autenticacao::Chave {
                chave_privada: ca,
                passphrase: pa,
            },
            Autenticacao::Chave {
                chave_privada: cb,
                passphrase: pb,
            },
        ) => mesmo_segredo(ca, cb) && mesmo_opcional(pa, pb),
        _ => false,
    }
}

fn mesmos_dados(a: &Dados, b: &Dados) -> bool {
    match (a, b) {
        (Dados::Login(a), Dados::Login(b)) => {
            let Login {
                url,
                usuario,
                senha,
                totp_semente,
            } = a;
            *url == b.url
                && *usuario == b.usuario
                && mesmo_segredo(senha, &b.senha)
                && mesmo_opcional(totp_semente, &b.totp_semente)
        }
        (Dados::Banco(a), Dados::Banco(b)) => {
            a.motor == b.motor
                && a.host == b.host
                && a.porta == b.porta
                && a.base == b.base
                && a.usuario == b.usuario
                && mesmo_segredo(&a.senha, &b.senha)
                && a.tunel == b.tunel
        }
        (Dados::Servidor(a), Dados::Servidor(b)) => {
            a.host == b.host
                && a.porta == b.porta
                && a.usuario == b.usuario
                && mesma_autenticacao(&a.autenticacao, &b.autenticacao)
                && a.salto == b.salto
        }
        (Dados::Api(a), Dados::Api(b)) => {
            a.url == b.url && mesmo_segredo(&a.token, &b.token) && a.variavel == b.variavel
        }
        (Dados::Aws(a), Dados::Aws(b)) => {
            a.access_key_id == b.access_key_id
                && mesmo_segredo(&a.secret_access_key, &b.secret_access_key)
                && a.regiao == b.regiao
                && a.role_arn == b.role_arn
                && mesmo_opcional(&a.session_token, &b.session_token)
        }
        (Dados::Smtp(a), Dados::Smtp(b)) => {
            a.host == b.host
                && a.porta == b.porta
                && a.usuario == b.usuario
                && mesmo_segredo(&a.senha, &b.senha)
                && a.seguranca == b.seguranca
        }
        (Dados::FtpSftp(a), Dados::FtpSftp(b)) => {
            a.protocolo == b.protocolo
                && a.host == b.host
                && a.porta == b.porta
                && a.usuario == b.usuario
                && mesma_autenticacao(&a.autenticacao, &b.autenticacao)
        }
        (Dados::NotaSegura(a), Dados::NotaSegura(b)) => mesmo_segredo(&a.texto, &b.texto),
        (Dados::Generico, Dados::Generico) => true,
        _ => false,
    }
}

fn mesmo_extra(a: &CampoExtra, b: &CampoExtra) -> bool {
    a.nome() == b.nome()
        && match (a.valor(), b.valor()) {
            (ValorExtra::Texto(x), ValorExtra::Texto(y)) => x == y,
            (ValorExtra::Segredo(x), ValorExtra::Segredo(y)) => mesmo_segredo(x, y),
            _ => false,
        }
}

/// Campo a campo; segredos comparados pelos bytes, sem nunca imprimi-los.
fn conferir_iguais(a: &Entrada, b: &Entrada) {
    let nome = a.nome();
    assert_eq!(a.nome(), b.nome());
    assert_eq!(a.tipo(), b.tipo(), "{nome}: tipo");
    assert_eq!(a.nivel(), b.nivel(), "{nome}: nível");
    assert_eq!(a.grupo(), b.grupo(), "{nome}: grupo");
    assert_eq!(a.ambiente(), b.ambiente(), "{nome}: ambiente");
    assert_eq!(a.projeto(), b.projeto(), "{nome}: projeto");
    assert_eq!(a.tags(), b.tags(), "{nome}: tags");
    assert_eq!(a.datas(), b.datas(), "{nome}: datas");
    assert!(mesmos_dados(a.dados(), b.dados()), "{nome}: dados");
    assert_eq!(a.extras().len(), b.extras().len(), "{nome}: extras");
    for (x, y) in a.extras().iter().zip(b.extras()) {
        assert!(mesmo_extra(x, y), "{nome}: extra {}", x.nome());
    }
    assert_eq!(a.anexos().len(), b.anexos().len(), "{nome}: anexos");
    for (x, y) in a.anexos().iter().zip(b.anexos()) {
        assert_eq!(x.nome(), y.nome(), "{nome}: anexo");
        assert!(
            mesmo_segredo(x.conteudo(), y.conteudo()),
            "{nome}: conteúdo do anexo {}",
            x.nome()
        );
    }
}

fn conferir_ida_e_volta(originais: &[Entrada], voltaram: &[Entrada]) {
    assert_eq!(originais.len(), voltaram.len());
    let mut ordem: Vec<&Entrada> = originais.iter().collect();
    ordem.sort_by(|a, b| a.nome().cmp(b.nome()));
    for (a, b) in ordem.iter().zip(voltaram) {
        conferir_iguais(a, b);
    }
}

/// O que o decodificador aceitou tem de ser um cofre válido: a configuração dentro das
/// regras, cada entrada confere contra todas as outras (a conferência completa,
/// quadrática, sem o índice do módulo) e serializar de novo dá os mesmos bytes.
fn conferir_aceito(corpo: &[u8], configuracao: &Configuracao, entradas: &[Entrada]) {
    // D39: a geração aceita é ≥ 1 e volta igual (ela está logo depois da versão).
    let geracao = desserializar(corpo).map(|(_, g, _)| g).unwrap();
    assert!(geracao >= 1, "aceitou geração 0");
    assert!(configuracao::minutos_validos(configuracao.minutos_trava()));
    assert!(configuracao::conferir_texto(
        configuracao.hash_pin().bytes()
    ));
    assert!(!configuracao.backup().ligado() || configuracao.backup().uuid().is_some());
    for (i, e) in entradas.iter().enumerate() {
        let outras = entradas
            .iter()
            .enumerate()
            .filter(move |(j, _)| *j != i)
            .map(|(_, o)| o);
        assert!(
            e.conferir_no_cofre(outras).is_ok(),
            "aceitou entrada que não confere no cofre"
        );
    }
    let de_novo = serializar(configuracao, geracao, entradas).expect("o aceito serializa");
    assert!(de_novo[..] == *corpo, "aceito não é canônico");
}

// ---- Corpo cru, para forjar ----

/// Uma entrada como bytes crus, para forjar campos que a validação nunca deixaria
/// existir numa [`Entrada`].
#[derive(Clone)]
struct Crua {
    tipo: Vec<u8>,
    nivel: Vec<u8>,
    nome: Vec<u8>,
    grupo: Vec<u8>,
    ambiente: Vec<u8>,
    projeto: Vec<u8>,
    tags: Vec<Vec<u8>>,
    dados: Vec<Vec<u8>>,
    extras: Vec<(Vec<u8>, u8, Vec<u8>)>,
    anexos: Vec<(Vec<u8>, Vec<u8>)>,
    /// D65 d: criação e alteração, 0 = sem.
    criada: u64,
    alterada: u64,
}

/// A seção de configuração como bytes crus, para forjar.
#[derive(Clone)]
struct CruaCfg {
    /// A geração (D39); `None` monta a seção sem ela, como no corpo v2.
    geracao: Option<u64>,
    hash_pin: Vec<u8>,
    minutos: u32,
    ligado: u8,
    uuid: Vec<u8>,
}

impl CruaCfg {
    /// A configuração do cofre de teste.
    fn padrao() -> Self {
        Self::de(cfg())
    }

    fn de(c: &Configuracao) -> Self {
        Self {
            geracao: Some(1),
            hash_pin: c.hash_pin().bytes().to_vec(),
            minutos: c.minutos_trava(),
            ligado: u8::from(c.backup().ligado()),
            uuid: c.backup().uuid().unwrap_or("").as_bytes().to_vec(),
        }
    }
}

/// Lê um corpo bem formado em entradas cruas (pela leitura do módulo).
fn cruas(corpo: &[u8]) -> Vec<Crua> {
    let mut l = Leitor { resto: corpo };
    let (_, n) = ler_cabecalho(&mut l).unwrap();
    (0..n)
        .map(|_| {
            let b = ler_bruta(&mut l).unwrap();
            let mut lt = Leitor {
                resto: b.tags.bytes,
            };
            let mut le = Leitor {
                resto: b.extras.bytes,
            };
            let mut la = Leitor {
                resto: b.anexos.bytes,
            };
            Crua {
                tipo: b.tipo.nome().as_bytes().to_vec(),
                nivel: b.nivel.nome().as_bytes().to_vec(),
                nome: b.nome.to_vec(),
                grupo: b.grupo.to_vec(),
                ambiente: b.ambiente.to_vec(),
                projeto: b.projeto.to_vec(),
                tags: (0..b.tags.n)
                    .map(|_| ler_tag(&mut lt).unwrap().to_vec())
                    .collect(),
                dados: b.dados[..esquema(b.tipo).len()]
                    .iter()
                    .map(|d| d.to_vec())
                    .collect(),
                extras: (0..b.extras.n)
                    .map(|_| {
                        let (n, s, v) = ler_extra(&mut le).unwrap();
                        (n.to_vec(), u8::from(s), v.to_vec())
                    })
                    .collect(),
                anexos: (0..b.anexos.n)
                    .map(|_| {
                        let (n, c) = ler_anexo(&mut la).unwrap();
                        (n.to_vec(), c.to_vec())
                    })
                    .collect(),
                criada: b.datas.criada_em().unwrap_or(0),
                alterada: b.datas.alterada_em().unwrap_or(0),
            }
        })
        .collect()
}

/// Escritor independente do módulo (o formato do topo de `corpo.rs`, à mão), com a
/// configuração do cofre de teste.
fn forjar_com(versao: u16, n: u32, entradas: &[Crua]) -> Vec<u8> {
    forjar_cfg(versao, Some(&CruaCfg::padrao()), n, entradas)
}

/// Escritor independente, com a seção de configuração dada (ou sem ela, `None`).
fn forjar_cfg(versao: u16, cfg: Option<&CruaCfg>, n: u32, entradas: &[Crua]) -> Vec<u8> {
    forjar_bruto(versao, cfg, n, entradas, true)
}

/// Escritor independente; `com_datas: false` monta as entradas como no corpo v3, sem
/// os 16 bytes das datas (D65 d).
fn forjar_bruto(
    versao: u16,
    cfg: Option<&CruaCfg>,
    n: u32,
    entradas: &[Crua],
    com_datas: bool,
) -> Vec<u8> {
    fn u32le(v: &mut Vec<u8>, n: usize) {
        v.extend_from_slice(&(n as u32).to_le_bytes());
    }
    fn campo(v: &mut Vec<u8>, b: &[u8]) {
        u32le(v, b.len());
        v.extend_from_slice(b);
    }
    let mut v = Vec::new();
    v.extend_from_slice(&versao.to_le_bytes());
    if let Some(c) = cfg {
        if let Some(g) = c.geracao {
            v.extend_from_slice(&g.to_le_bytes());
        }
        campo(&mut v, &c.hash_pin);
        v.extend_from_slice(&c.minutos.to_le_bytes());
        v.push(c.ligado);
        campo(&mut v, &c.uuid);
    }
    v.extend_from_slice(&n.to_le_bytes());
    for e in entradas {
        for c in [
            &e.tipo,
            &e.nivel,
            &e.nome,
            &e.grupo,
            &e.ambiente,
            &e.projeto,
        ] {
            campo(&mut v, c);
        }
        u32le(&mut v, e.tags.len());
        for tag in &e.tags {
            campo(&mut v, tag);
        }
        for d in &e.dados {
            campo(&mut v, d);
        }
        u32le(&mut v, e.extras.len());
        for (nome, s, valor) in &e.extras {
            campo(&mut v, nome);
            v.push(*s);
            campo(&mut v, valor);
        }
        u32le(&mut v, e.anexos.len());
        for (nome, conteudo) in &e.anexos {
            campo(&mut v, nome);
            campo(&mut v, conteudo);
        }
        if com_datas {
            v.extend_from_slice(&e.criada.to_le_bytes());
            v.extend_from_slice(&e.alterada.to_le_bytes());
        }
    }
    v
}

fn forjar(entradas: &[Crua]) -> Vec<u8> {
    forjar_com(VERSAO_CORPO, entradas.len() as u32, entradas)
}

fn indice(cruas: &[Crua], nome: &str) -> usize {
    cruas
        .iter()
        .position(|c| c.nome == nome.as_bytes())
        .unwrap()
}

/// Gerador pseudoaleatório só para as mutações (semente fixa, reprodutível). Não é
/// cripto e não sai dos testes.
struct SplitMix(u64);

impl SplitMix {
    fn proximo(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn ate(&mut self, n: usize) -> usize {
        (self.proximo() % n as u64) as usize
    }
}

// ---- V2.30: ida e volta ----

#[test]
fn v2_30_ida_e_volta_do_cofre_de_teste() {
    let mut originais = cofre_de_teste();
    assert_eq!(originais.len(), 22);
    // D65 d: datas nas três formas (as duas, só a alteração, nenhuma).
    originais[0].definir_datas(Datas::carimbo_novo(1_762_000_000).unwrap());
    originais[1].definir_datas(
        Datas::carimbo_novo(1_762_000_000)
            .unwrap()
            .alterado(1_762_500_000)
            .unwrap(),
    );
    originais[2].definir_datas(Datas::NENHUMA.alterado(MAX_DATE).unwrap());
    let corpo = serializar(cfg(), 1, &originais).unwrap();
    let voltaram = ler(&corpo).unwrap();
    conferir_ida_e_volta(&originais, &voltaram);

    // Os três níveis de D15 voltam como estavam.
    let nivel = |nome: &str| {
        voltaram
            .iter()
            .find(|e| e.nome() == nome)
            .map(Entrada::nivel)
            .unwrap()
    };
    assert_eq!(nivel("gama-bloqueada"), NivelAcesso::Bloqueado);
    assert_eq!(nivel("acme-pg-aprovacao"), NivelAcesso::ComAprovacao);
    assert_eq!(nivel("acme-pg-dev"), NivelAcesso::Livre);
    assert_eq!(nivel("acme-totp-dev"), NivelAcesso::Livre);

    // Canônico: de novo dá os mesmos bytes, e a ordem de entrada não importa.
    assert!(serializar(cfg(), 1, &voltaram).unwrap()[..] == corpo[..]);
    let mut invertidas = ler(&corpo).unwrap();
    invertidas.reverse();
    assert!(serializar(cfg(), 1, &invertidas).unwrap()[..] == corpo[..]);
    // Saem em ordem de nome.
    assert!(voltaram.windows(2).all(|w| w[0].nome() < w[1].nome()));
    // O escritor independente dos testes chega aos mesmos bytes.
    assert!(forjar(&cruas(&corpo)) == corpo[..]);
}

#[test]
fn v2_30_uma_de_cada_tipo_no_maximo_do_modelo() {
    let originais = um_de_cada_cheio();
    for tipo in TipoEntrada::TODOS {
        assert!(originais.iter().any(|e| e.tipo() == tipo), "falta {tipo}");
    }
    for e in &originais {
        assert_eq!(e.tags().len(), 16);
        assert_eq!(e.extras().len(), 64);
        assert!(e.projeto().is_some());
    }
    let corpo = serializar(cfg(), 1, &originais).unwrap();
    let voltaram = ler(&corpo).unwrap();
    conferir_ida_e_volta(&originais, &voltaram);
    assert!(serializar(cfg(), 1, &voltaram).unwrap()[..] == corpo[..]);
}

#[test]
fn v2_30_cofre_vazio() {
    // Só a configuração: versão 4, geração 1, hash do PIN, 240 minutos, backup desligado
    // sem UUID, zero entradas.
    let corpo = serializar(cfg(), 1, &[]).unwrap();
    let hash = cfg().hash_pin().bytes();
    let mut esperado = vec![4, 0];
    esperado.extend_from_slice(&1u64.to_le_bytes());
    esperado.extend_from_slice(&(hash.len() as u32).to_le_bytes());
    esperado.extend_from_slice(hash);
    esperado.extend_from_slice(&240u32.to_le_bytes());
    esperado.extend_from_slice(&[0, 0, 0, 0, 0]);
    esperado.extend_from_slice(&[0, 0, 0, 0]);
    assert!(corpo[..] == esperado[..]);
    let (c, entradas) = ler_tudo(&corpo).unwrap();
    assert!(entradas.is_empty());
    assert!(c.conferir_pin(PIN_TESTE.as_bytes()));
    assert!(serializar(&c, 1, &[]).unwrap()[..] == corpo[..]);
}

/// Diretório de teste em `target/<perfil>/ward-teste-corpo/`, apagado no fim.
#[cfg(unix)]
struct Dir(std::path::PathBuf);

#[cfg(unix)]
impl Dir {
    fn novo(nome: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let exe = std::env::current_exe().unwrap();
        let base = exe
            .parent()
            .and_then(std::path::Path::parent)
            .unwrap()
            .join("ward-teste-corpo");
        std::fs::create_dir_all(&base).unwrap();
        let mut aleatorio = [0u8; 8];
        crate::crypto::random(&mut aleatorio).unwrap();
        let d = base.join(format!(
            "{nome}-{}-{}",
            std::process::id(),
            u64::from_le_bytes(aleatorio)
        ));
        std::fs::create_dir(&d).unwrap();
        std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self(d)
    }
}

#[cfg(unix)]
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
#[test]
fn v2_30_ida_e_volta_pelo_disco_com_anexos_no_limite() {
    use crate::gravacao::gravar_entradas;

    let dir = Dir::novo("ida-e-volta");
    let destino = dir.0.join("cofre.ward");
    let senha = SenhaMestra::nova(SENHA_MESTRA.to_owned());
    let mut cofre = CofreAberto::novo(&senha, configuracao_nova()).unwrap();

    // 22 do cofre de teste + uma de cada tipo cheia, cada uma com anexos que somam o
    // limite de D29 (256 KiB).
    let mut originais = cofre_de_teste();
    originais.extend(um_de_cada_cheio());
    for e in originais
        .iter()
        .filter(|e| e.nome().starts_with("zz-cheio-") && e.nome() != "zz-cheio-api-token")
    {
        let soma: usize = e.anexos().iter().map(|a| a.conteudo().len()).sum();
        assert_eq!(soma, crate::limits::MAX_ATTACHMENTS_TOTAL);
    }
    let esperado = serializar(cofre.configuracao(), 1, &originais).unwrap();
    assert!(
        esperado.len() > 8 * crate::limits::MAX_ATTACHMENTS_TOTAL && esperado.len() <= MAX_BODY
    );

    gravar_entradas(&destino, &mut cofre, &originais).unwrap();
    let arquivo = std::fs::read(&destino).unwrap();
    let hash = cofre.configuracao().hash_pin().bytes().to_vec();
    let hash = &hash[..];
    let marcas = SEGREDOS
        .iter()
        .filter(|m| **m != SENHA_MESTRA)
        .map(|m| m.as_bytes())
        .chain([PIN_TESTE.as_bytes(), hash, &hash[..9]]);
    for marca in marcas {
        assert!(
            !arquivo.windows(marca.len()).any(|j| j == marca),
            "segredo em claro no arquivo"
        );
    }
    let (reaberto, voltaram) = abrir_entradas(&arquivo, &senha).unwrap();
    conferir_ida_e_volta(&originais, &voltaram);
    // A configuração volta igual (D30), e o PIN de teste confere no hash lido.
    assert!(serializar(reaberto.configuracao(), 1, &voltaram).unwrap()[..] == esperado[..]);
    assert!(reaberto.configuracao().hash_pin().bytes() == hash);
    assert!(reaberto.configuracao().conferir_pin(PIN_TESTE.as_bytes()));

    // Cofre vazio (só a configuração) também faz ida e volta pelo disco.
    gravar_entradas(&destino, &mut cofre, &[]).unwrap();
    let arquivo = std::fs::read(&destino).unwrap();
    let (reaberto, voltaram) = abrir_entradas(&arquivo, &senha).unwrap();
    assert!(voltaram.is_empty());
    assert!(reaberto.configuracao().hash_pin().bytes() == hash);

    // Senha errada: erro do formato, não do corpo.
    let errada = SenhaMestra::nova("falso-mestra-ward-02".to_owned());
    assert_eq!(
        abrir_entradas(&arquivo, &errada).unwrap_err(),
        ErroAbrir::Formato(ErroFormato::SenhaOuArquivo)
    );
}

#[cfg(unix)]
#[test]
fn troca_de_senha_sela_e_grava_pelo_mesmo_caminho() {
    use crate::gravacao::{ErroGravacao, gravar_entradas, trocar_senha};

    let dir = Dir::novo("troca");
    let destino = dir.0.join("cofre.ward");
    let atual = SenhaMestra::nova(SENHA_MESTRA.to_owned());
    let nova = SenhaMestra::nova("falso-mestra-ward-02".to_owned());
    let mut cofre = CofreAberto::novo(&atual, configuracao_nova()).unwrap();
    let hash = cofre.configuracao().hash_pin().bytes().to_vec();
    let originais = cofre_de_teste();
    gravar_entradas(&destino, &mut cofre, &originais).unwrap();
    let antes = std::fs::read(&destino).unwrap();

    // Senha atual errada: recusa e não toca no arquivo.
    assert_eq!(
        trocar_senha(&destino, &nova, &nova, 0, 0).unwrap_err(),
        ErroGravacao::Formato(ErroFormato::SenhaOuArquivo)
    );
    assert!(std::fs::read(&destino).unwrap() == antes);

    let (mut novo, _) = trocar_senha(&destino, &atual, &nova, 0, 0).unwrap();
    // A troca de senha mantém a configuração (o mesmo hash do PIN).
    assert!(novo.configuracao().hash_pin().bytes() == &hash[..]);
    let depois = std::fs::read(&destino).unwrap();
    assert!(abrir_entradas(&depois, &atual).is_err());
    let (_, voltaram) = abrir_entradas(&depois, &nova).unwrap();
    conferir_ida_e_volta(&originais, &voltaram);
    // O cofre devolvido grava com a senha nova.
    gravar_entradas(&destino, &mut novo, &voltaram).unwrap();
    let (_, de_novo) = abrir_entradas(&std::fs::read(&destino).unwrap(), &nova).unwrap();
    conferir_ida_e_volta(&originais, &de_novo);
}

#[test]
fn formato_conhecido_de_uma_entrada() {
    // Uma entrada genérica escrita à mão, byte a byte, como no desenho do módulo:
    // versão 4, configuração (geração 7, hash, 240 minutos, backup desligado, sem UUID),
    // 1 entrada, com as datas no fim (D65 d).
    let mut v: Vec<u8> = vec![4, 0];
    v.extend_from_slice(&7u64.to_le_bytes());
    let campo = |v: &mut Vec<u8>, b: &[u8]| {
        v.extend_from_slice(&(b.len() as u32).to_le_bytes());
        v.extend_from_slice(b);
    };
    campo(&mut v, cfg().hash_pin().bytes());
    v.extend_from_slice(&240u32.to_le_bytes());
    v.push(0);
    campo(&mut v, b"");
    v.extend_from_slice(&1u32.to_le_bytes());
    campo(&mut v, b"generico");
    campo(&mut v, b"livre");
    campo(&mut v, b"x-1");
    campo(&mut v, b"grupo");
    campo(&mut v, b"dev");
    campo(&mut v, b"");
    v.extend_from_slice(&1u32.to_le_bytes());
    campo(&mut v, b"tag");
    v.extend_from_slice(&1u32.to_le_bytes());
    campo(&mut v, b"CAMPO");
    v.push(0);
    campo(&mut v, b"valor");
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&1_762_000_000u64.to_le_bytes());
    v.extend_from_slice(&1_762_086_400u64.to_le_bytes());

    let entradas = ler(&v).unwrap();
    assert_eq!(entradas.len(), 1);
    let e = &entradas[0];
    assert_eq!(e.nome(), "x-1");
    assert_eq!(e.tipo(), TipoEntrada::Generico);
    assert_eq!(e.nivel(), NivelAcesso::Livre);
    assert_eq!(e.ambiente(), Ambiente::Dev);
    assert_eq!(e.projeto(), None);
    assert_eq!(e.tags(), ["tag".to_owned()]);
    assert!(matches!(e.extras()[0].valor(), ValorExtra::Texto(t) if t == "valor"));
    assert_eq!(e.datas().criada_em(), Some(1_762_000_000));
    assert_eq!(e.datas().alterada_em(), Some(1_762_086_400));
    assert_eq!(desserializar(&v).map(|(_, g, _)| g), Ok(7));
    assert!(serializar(cfg(), 7, &entradas).unwrap()[..] == v[..]);
}

// ---- Recusas da gravação ----

#[test]
fn serializar_recusa_cofre_que_nao_reabriria() {
    // Nome repetido.
    let mut entradas = cofre_de_teste();
    entradas.extend(cofre_de_teste().into_iter().take(1));
    assert_eq!(
        serializar(cfg(), 1, &entradas).unwrap_err(),
        ErroCorpo::Inconsistente
    );

    // Túnel para um servidor que não está no cofre.
    let entradas: Vec<Entrada> = cofre_de_teste()
        .into_iter()
        .filter(|e| e.nome() != "acme-ssh-senha-stage")
        .collect();
    assert_eq!(
        serializar(cfg(), 1, &entradas).unwrap_err(),
        ErroCorpo::Inconsistente
    );

    // Salto para um servidor que tem salto (cadeia).
    let mut entradas: Vec<Entrada> = cofre_de_teste()
        .into_iter()
        .filter(|e| e.nome() != "acme-ssh-salto")
        .collect();
    let mut r = Rascunho {
        nome: "acme-ssh-salto".to_owned(),
        grupo: "acme".to_owned(),
        ambiente: "prod".to_owned(),
        projeto: String::new(),
        tags: Vec::new(),
        dados: RascunhoDados::Servidor {
            host: "127.0.0.1".to_owned(),
            porta: "2224".to_owned(),
            usuario: "salto".to_owned(),
            senha: Some(Segredo::de_texto("falso-SALTO-01".to_owned())),
            chave_privada: None,
            passphrase: None,
            salto: "acme-ssh-senha-stage".to_owned(),
        },
        extras: Vec::new(),
        anexos: Vec::new(),
    };
    r.tags.push("salto".to_owned());
    entradas.push(validada(r, NivelAcesso::ComAprovacao));
    assert_eq!(
        serializar(cfg(), 1, &entradas).unwrap_err(),
        ErroCorpo::Inconsistente
    );
}

#[test]
fn serializar_recusa_mais_de_10_000_entradas() {
    let entradas: Vec<Entrada> = (0..=MAX_ENTRIES)
        .map(|i| {
            let mut r = Rascunho {
                nome: format!("e{i:05}"),
                grupo: "g".to_owned(),
                ambiente: "dev".to_owned(),
                projeto: String::new(),
                tags: Vec::new(),
                dados: RascunhoDados::Generico,
                extras: Vec::new(),
                anexos: Vec::new(),
            };
            r.extras.push(crate::entrada::RascunhoExtra {
                nome: "A".to_owned(),
                valor: Segredo::de_texto("v".to_owned()),
                segredo: false,
            });
            validada(r, NivelAcesso::Livre)
        })
        .collect();
    assert_eq!(
        serializar(cfg(), 1, &entradas).unwrap_err(),
        ErroCorpo::GrandeDemais
    );
    // 10 000 cabem e voltam.
    let corpo = serializar(cfg(), 1, &entradas[..MAX_ENTRIES]).unwrap();
    assert_eq!(ler(&corpo).unwrap().len(), MAX_ENTRIES);
}

#[test]
fn v2_30_anexo_vazio_recusado_ao_salvar_e_no_corpo() {
    // D29: anexo vazio é recusado ao salvar (não chega a virar `Entrada`) e no corpo
    // forjado.
    let mut r = Rascunho {
        nome: "anexo-vazio".to_owned(),
        grupo: "g".to_owned(),
        ambiente: "dev".to_owned(),
        projeto: String::new(),
        tags: Vec::new(),
        dados: RascunhoDados::Generico,
        extras: Vec::new(),
        anexos: Vec::new(),
    };
    r.anexos.push(RascunhoAnexo {
        nome: "vazio.txt".to_owned(),
        conteudo: Segredo::de_bytes(Vec::new()),
    });
    assert!(r.validar().is_err());

    let corpo = corpo_de_teste();
    let mut c = cruas(&corpo);
    let k = indice(&c, "gama-generico");
    c[k].anexos[0].1.clear();
    recusado(&forjar(&c), "anexo vazio", "FALSO_ENV");
}

// ---- V2.31: corpo adulterado ----

#[test]
fn v2_31_truncado_em_cada_tamanho_e_byte_a_mais() {
    let corpo = corpo_de_teste();
    for n in 0..corpo.len() {
        recusado(&corpo[..n], "truncado", "falso");
    }
    for extra in [0x00u8, 0x01, 0xff] {
        let mut v = corpo.to_vec();
        v.push(extra);
        recusado(&v, "byte a mais", "falso");
    }
    assert!(ler(&corpo).is_ok());
}

#[test]
fn v2_31_campos_forjados() {
    let corpo = corpo_de_teste();
    let base = cruas(&corpo);
    assert!(forjar(&base) == corpo[..]);
    let n = base.len() as u32;

    // (caso, corpo forjado, valor que não pode aparecer no erro)
    let mut casos: Vec<(&str, Vec<u8>, &str)> = Vec::new();
    // A versão 1 (só entradas, Fase 15a) é desconhecida desde D30, e a 2 (sem geração)
    // desde D39, e a 3 (sem datas) desde D65 d.
    for versao in [0u16, 1, 2, 3, 5, 0xffff] {
        casos.push((
            "versão desconhecida",
            forjar_com(versao, n, &base),
            "versão",
        ));
    }
    casos.push((
        "contagem a mais",
        forjar_com(VERSAO_CORPO, n + 1, &base),
        "contagem",
    ));
    casos.push((
        "contagem a menos",
        forjar_com(VERSAO_CORPO, n - 1, &base),
        "contagem",
    ));

    let mut com = |caso: &'static str, valor: &'static str, f: &dyn Fn(&mut Vec<Crua>)| {
        let mut c = base.clone();
        f(&mut c);
        casos.push((caso, forjar(&c), valor));
    };
    com("tipo fora da lista", "customizado", &|c| {
        c[0].tipo = b"customizado".to_vec()
    });
    com("tipo trocado", "banco", &|c| {
        let k = indice(c, "beta-github-api");
        c[k].tipo = b"banco".to_vec();
    });
    for nivel in ["admin", "", "Livre", "livre "] {
        com("nível fora da lista", "admin", &move |c| {
            c[1].nivel = nivel.as_bytes().to_vec()
        });
    }
    com("nome repetido", "acme-pg-dev", &|c| {
        let k = indice(c, "acme-pg-prod");
        c[k].nome = b"acme-pg-dev".to_vec();
    });
    com("nome repetido seguido", "acme-pg-aprovacao", &|c| {
        let k = indice(c, "acme-pg-aprovacao");
        let copia = c[k].clone();
        c.insert(k + 1, copia);
    });
    com("fora de ordem", "acme", &|c| c.swap(0, 1));
    com("salto inexistente", "nao-existe", &|c| {
        let k = indice(c, "acme-ssh-interno");
        c[k].dados[6] = b"nao-existe".to_vec();
    });
    com(
        "salto para servidor com salto",
        "acme-ssh-interno-aprov",
        &|c| {
            let k = indice(c, "acme-ssh-interno");
            c[k].dados[6] = b"acme-ssh-interno-aprov".to_vec();
        },
    );
    com("salto para si mesmo", "acme-ssh-interno", &|c| {
        let k = indice(c, "acme-ssh-interno");
        c[k].dados[6] = b"acme-ssh-interno".to_vec();
    });
    com(
        "servidor usado como salto ganha salto",
        "acme-ssh-senha-stage",
        &|c| {
            let k = indice(c, "acme-ssh-salto");
            c[k].dados[6] = b"acme-ssh-senha-stage".to_vec();
        },
    );
    com(
        "túnel para entrada que não é servidor",
        "acme-pg-dev",
        &|c| {
            let k = indice(c, "acme-pg-tunel");
            c[k].dados[6] = b"acme-pg-dev".to_vec();
        },
    );
    com("-oProxyCommand no host", "ProxyCommand", &|c| {
        let k = indice(c, "acme-ssh-senha-stage");
        c[k].dados[0] = b"-oProxyCommand=sh -c id".to_vec();
    });
    com("-J no usuário", "-Jx", &|c| {
        let k = indice(c, "acme-ssh-senha-stage");
        c[k].dados[2] = b"-Jx".to_vec();
    });
    com("U+202E no grupo", "\u{202E}", &|c| {
        c[2].grupo = "ac\u{202E}me".as_bytes().to_vec()
    });
    com("nulo num segredo", "falso", &|c| {
        let k = indice(c, "acme-ssh-senha-stage");
        c[k].dados[3] = b"falso\0SSH".to_vec();
    });
    com("senha e chave juntas", "falso", &|c| {
        let k = indice(c, "acme-ssh-senha-stage");
        c[k].dados[4] = chave_falsa().into_bytes();
    });
    com("porta 0", "porta", &|c| {
        let k = indice(c, "acme-ssh-senha-stage");
        c[k].dados[1] = vec![0, 0];
    });
    com("porta com 3 bytes", "porta", &|c| {
        let k = indice(c, "acme-ssh-senha-stage");
        c[k].dados[1] = vec![0x22, 0x08, 0];
    });
    com("ambiente fora da lista", "PROD", &|c| {
        c[0].ambiente = b"PROD".to_vec()
    });
    com("nome não UTF-8", "\u{FFFD}", &|c| {
        c[0].nome = vec![b'a', 0xff]
    });
    com("marca de segredo do extra inválida", "falso", &|c| {
        let k = indice(c, "gama-generico");
        c[k].extras[1].1 = 2;
    });
    com("17 tags", "tag-17", &|c| {
        c[0].tags = (1..=17)
            .map(|n| format!("tag-{n:02}").into_bytes())
            .collect()
    });
    com("65 extras", "extra_65", &|c| {
        c[0].extras = (1..=65)
            .map(|n| (format!("extra_{n:02}").into_bytes(), 0, b"v".to_vec()))
            .collect()
    });
    com("33 anexos", "a33", &|c| {
        c[0].anexos = (1..=33)
            .map(|n| (format!("a{n:02}").into_bytes(), b"x".to_vec()))
            .collect()
    });
    com("grupo acima do limite", "gggg", &|c| {
        c[0].grupo = vec![b'g'; 2049]
    });
    com("segredo acima do limite", "falso", &|c| {
        let k = indice(c, "acme-ssh-senha-stage");
        c[k].dados[3] = vec![b's'; 64 * 1024 + 1];
    });
    com("anexo do servidor chamado chave", "chave", &|c| {
        let k = indice(c, "acme-ssh-senha-stage");
        c[k].anexos.push((b"chave".to_vec(), b"x".to_vec()));
    });

    assert!(casos.len() >= 30);
    for (caso, forjado, valor) in &casos {
        recusado(forjado, caso, valor);
    }
}

#[test]
fn v2_31_bit_trocado_em_cada_byte() {
    let corpo = corpo_de_teste();
    let mut aceitos = 0;
    for i in 0..corpo.len() {
        let mut v = corpo.to_vec();
        v[i] ^= 1 << (i % 8);
        if let Ok((c, entradas)) = ler_tudo(&v) {
            conferir_aceito(&v, &c, &entradas);
            aceitos += 1;
        }
    }
    // Trocar um bit de um texto comum às vezes ainda dá uma entrada válida (é o AEAD
    // que barra isso no arquivo); o que importa é que nada inválido passe.
    assert!(aceitos < corpo.len());
}

#[test]
fn v2_31_100_000_mutacoes_aleatorias() {
    let corpo = corpo_de_teste();
    const TOTAL: usize = 100_000;
    const FIOS: usize = 4;
    let interessantes: [u32; 8] = [
        0,
        1,
        2,
        16,
        0x7fff_ffff,
        0xffff_ffff,
        corpo.len() as u32,
        (corpo.len() + 1) as u32,
    ];
    let aceitos: usize = std::thread::scope(|escopo| {
        let fios: Vec<_> = (0..FIOS)
            .map(|f| {
                let corpo = &corpo;
                escopo.spawn(move || {
                    let mut rng = SplitMix(0x5741_5244_0015_a000 + f as u64);
                    let mut aceitos = 0;
                    for _ in 0..TOTAL / FIOS {
                        let mut v = corpo.to_vec();
                        for _ in 0..1 + rng.ate(3) {
                            let pos = rng.ate(v.len().max(1));
                            match rng.ate(7) {
                                0 => v[pos] ^= 1 << rng.ate(8),
                                1 => v[pos] = rng.proximo() as u8,
                                2 if pos + 4 <= v.len() => {
                                    let x = interessantes[rng.ate(interessantes.len())];
                                    v[pos..pos + 4].copy_from_slice(&x.to_le_bytes());
                                }
                                3 => {
                                    let fim = (pos + 1 + rng.ate(64)).min(v.len());
                                    v.drain(pos..fim);
                                }
                                4 => {
                                    let fim = (pos + 1 + rng.ate(64)).min(v.len());
                                    let pedaco = v[pos..fim].to_vec();
                                    let onde = rng.ate(v.len() + 1);
                                    v.splice(onde..onde, pedaco);
                                }
                                5 => v.insert(pos, rng.proximo() as u8),
                                _ => {
                                    let outro = rng.ate(v.len().max(1));
                                    v.swap(pos, outro);
                                }
                            }
                            if v.is_empty() {
                                break;
                            }
                        }
                        if let Ok((c, entradas)) = ler_tudo(&v) {
                            conferir_aceito(&v, &c, &entradas);
                            aceitos += 1;
                        }
                    }
                    aceitos
                })
            })
            .collect();
        fios.into_iter().map(|f| f.join().unwrap()).sum()
    });
    assert!(aceitos < TOTAL);
}

#[test]
fn erros_sem_dado_do_cofre() {
    for e in [
        ErroCorpo::Invalido,
        ErroCorpo::GrandeDemais,
        ErroCorpo::Inconsistente,
    ] {
        let texto = format!("{e} {e:?}");
        assert!(!texto.contains("falso"));
        assert!(!format!("{e}").is_empty());
    }
    let e = ErroAbrir::Corpo(ErroCorpo::Invalido);
    assert_eq!(format!("{e}"), "conteúdo do cofre inválido ou corrompido");
}

#[test]
fn leitura_no_limite_aceita_e_acima_recusa() {
    // Corpo acima de 64 MiB é recusado antes de qualquer leitura.
    let mut v = vec![0u8; MAX_BODY + 1];
    v[0] = 2;
    recusado(&v, "acima de 64 MiB", "falso");
    // Os limites de leitura não são menores que os da validação: a entrada cheia
    // (URL de 2048, token de 64 KiB, nota de 1 MiB, anexos somando 256 KiB) volta.
    let cheias = um_de_cada_cheio();
    let corpo = serializar(cfg(), 1, &cheias).unwrap();
    conferir_ida_e_volta(&cheias, &ler(&corpo).unwrap());
    // Ambiente lido de volta pelo identificador.
    assert!(cheias.iter().all(|e| e.ambiente() == Ambiente::Stage));
}

// ---- V2.36: entrada grande demais no corpo forjado (D29) ----

#[test]
fn v2_36_corpo_forjado_com_entrada_grande_demais() {
    use crate::limits::{MAX_ATTACHMENTS_TOTAL, MAX_EXTRA_VALUE, MAX_EXTRAS};

    let corpo = corpo_de_teste();
    let base = cruas(&corpo);

    // Um anexo de 256 KiB + 1 byte (recusado já na 1ª passada).
    let mut c = base.clone();
    let k = indice(&c, "gama-generico");
    c[k].anexos = vec![(
        b"grande.bin".to_vec(),
        vec![0x5a; MAX_ATTACHMENTS_TOTAL + 1],
    )];
    recusado(&forjar(&c), "anexo de 256 KiB + 1", "FALSO_ENV");

    // Dois anexos de 200 KiB: cada um cabe, a soma não.
    let mut c = base.clone();
    c[k].anexos = vec![
        (b"a.bin".to_vec(), vec![0x5a; 200 * 1024]),
        (b"b.bin".to_vec(), vec![0x5a; 200 * 1024]),
    ];
    recusado(&forjar(&c), "dois anexos de 200 KiB", "FALSO_ENV");

    // `api` com 64 extras secretos de 64 KiB: cada campo cabe nos limites, mas a
    // entrada passa do orçamento da máscara (recusada na validação da 2ª passada).
    let mut c = base.clone();
    let k = indice(&c, "beta-github-api");
    c[k].extras = (0..MAX_EXTRAS)
        .map(|i| {
            (
                format!("e{i:02}").into_bytes(),
                1,
                vec![b'k'; MAX_EXTRA_VALUE],
            )
        })
        .collect();
    recusado(&forjar(&c), "api acima do orçamento", "kkkk");

    // Controle: no limite (um anexo de 256 KiB, poucos extras grandes) o corpo abre.
    let mut c = base.clone();
    let k = indice(&c, "gama-generico");
    c[k].anexos = vec![(b"grande.bin".to_vec(), vec![0x5a; MAX_ATTACHMENTS_TOTAL])];
    let k = indice(&c, "beta-github-api");
    c[k].extras = (0..2)
        .map(|i| {
            (
                format!("e{i:02}").into_bytes(),
                1,
                vec![b'k'; MAX_EXTRA_VALUE],
            )
        })
        .collect();
    let forjado = forjar(&c);
    let (lida, lidas) = ler_tudo(&forjado).expect("no limite o corpo abre");
    conferir_aceito(&forjado, &lida, &lidas);
}

// ---- V2.39 / V2.30: a seção de configuração do corpo v2 (D30) ----

/// Texto PHC com os campos dados (para forjar hashes que a criação nunca faria).
fn phc(alg: &str, versao: &str, params: &str, sal: &str, hash: &str) -> Vec<u8> {
    let mut t = format!("${alg}");
    if !versao.is_empty() {
        t.push_str(&format!("$v={versao}"));
    }
    t.push_str(&format!("${params}${sal}${hash}"));
    t.into_bytes()
}

/// As partes `sal` e `hash` (B64 do PHC) do hash do cofre de teste.
fn partes_do_hash() -> (String, String) {
    let texto = core::str::from_utf8(cfg().hash_pin().bytes()).unwrap();
    let campos: Vec<&str> = texto.split('$').collect();
    assert_eq!(campos.len(), 6, "hash PHC com 5 campos");
    (campos[4].to_owned(), campos[5].to_owned())
}

#[test]
fn v2_30_configuracao_volta_igual() {
    // Valores fora do padrão para provar que cada campo vai e volta.
    let mut c = configuracao_nova();
    c.definir_minutos_trava(30).unwrap();
    c.definir_backup(BackupExterno::novo(true, Some("1234-ABCD")).unwrap());
    let corpo = serializar(&c, 1, &cofre_de_teste()).unwrap();
    let (lida, entradas) = ler_tudo(&corpo).unwrap();
    assert_eq!(entradas.len(), 22);
    assert_eq!(lida.minutos_trava(), 30);
    assert_eq!(
        lida.backup(),
        &BackupExterno::novo(true, Some("1234-ABCD")).unwrap()
    );
    assert!(lida.hash_pin().bytes() == c.hash_pin().bytes());
    assert!(lida.conferir_pin(PIN_TESTE.as_bytes()));
    assert!(!lida.conferir_pin(b"135791"));
    assert!(serializar(&lida, 1, &entradas).unwrap()[..] == corpo[..]);

    // Backup desligado com UUID guardado também volta (o pendrive já escolhido).
    c.definir_backup(
        BackupExterno::novo(false, Some("0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0")).unwrap(),
    );
    let corpo = serializar(&c, 1, &[]).unwrap();
    let (lida, _) = ler_tudo(&corpo).unwrap();
    assert_eq!(lida.backup(), c.backup());

    // Padrão do cofre de teste: 240 minutos e backup desligado sem UUID.
    let (lida, _) = ler_tudo(&corpo_de_teste()).unwrap();
    assert_eq!(lida.minutos_trava(), 240);
    assert_eq!(lida.backup(), &BackupExterno::desligado());
}

/// V2.39: o PIN de teste não aparece em claro no corpo decifrado; só o hash PHC, com
/// parâmetros e sal.
#[test]
fn v2_39_pin_so_como_hash_no_corpo() {
    let corpo = corpo_de_teste();
    assert!(
        !corpo
            .windows(PIN_TESTE.len())
            .any(|j| j == PIN_TESTE.as_bytes())
    );
    let hash = cfg().hash_pin().bytes();
    assert!(hash.starts_with(b"$argon2id$v=19$m=65536,t=3,p=1$"));
    assert!(corpo.windows(hash.len()).any(|j| j == hash));
    let (sal, saida) = partes_do_hash();
    assert_eq!(sal.len(), 22, "sal de 16 bytes");
    assert_eq!(saida.len(), 43, "saída de 32 bytes");
}

/// V2.39: corpo v1 (o da Fase 15a, só entradas), corpo v2 sem a seção e corpo sem hash
/// de PIN são recusados com o erro genérico.
#[test]
fn v2_39_corpo_v1_sem_secao_ou_sem_hash_recusado() {
    let corpo = corpo_de_teste();
    let base = cruas(&corpo);
    let n = base.len() as u32;
    recusado(&forjar_cfg(1, None, n, &base), "corpo v1", "falso");
    recusado(&forjar_cfg(1, None, 0, &[]), "corpo v1 vazio", "falso");
    recusado(
        &forjar_cfg(1, Some(&CruaCfg::padrao()), n, &base),
        "v1 com configuração",
        "falso",
    );
    recusado(
        &forjar_cfg(VERSAO_CORPO, None, n, &base),
        "sem a seção",
        "falso",
    );
    recusado(
        &forjar_cfg(VERSAO_CORPO, None, 0, &[]),
        "sem a seção, vazio",
        "falso",
    );
    let sem_hash = CruaCfg {
        hash_pin: Vec::new(),
        ..CruaCfg::padrao()
    };
    recusado(
        &forjar_cfg(VERSAO_CORPO, Some(&sem_hash), n, &base),
        "sem hash",
        "falso",
    );
    recusado(
        &forjar_cfg(VERSAO_CORPO, Some(&sem_hash), 0, &[]),
        "sem hash, vazio",
        "falso",
    );
    // Controle: a mesma montagem com a seção certa abre.
    assert!(
        ler(&forjar_cfg(
            VERSAO_CORPO,
            Some(&CruaCfg::padrao()),
            n,
            &base
        ))
        .is_ok()
    );
}

/// Ataque: hash do PIN forjado no corpo (rebaixado para rodar barato, inflado para
/// travar o daemon, de outro algoritmo, com parâmetro a mais, sal ou saída curtos, fora
/// da forma canônica, o PIN em claro no lugar do hash…), e os outros campos da seção
/// fora da regra. Todos recusados com o mesmo erro, sem o valor no erro.
#[test]
fn v2_39_secao_de_configuracao_forjada() {
    let (sal, saida) = partes_do_hash();
    let (s, h) = (sal.as_str(), saida.as_str());
    let p = "m=65536,t=3,p=1";
    let hashes: Vec<(&str, Vec<u8>)> = vec![
        ("PIN em claro", PIN_TESTE.as_bytes().to_vec()),
        ("argon2i", phc("argon2i", "19", p, s, h)),
        ("argon2d", phc("argon2d", "19", p, s, h)),
        ("scrypt", phc("scrypt", "19", p, s, h)),
        ("versão 16", phc("argon2id", "16", p, s, h)),
        ("sem versão", phc("argon2id", "", p, s, h)),
        (
            "memória rebaixada",
            phc("argon2id", "19", "m=8,t=3,p=1", s, h),
        ),
        (
            "passes rebaixados",
            phc("argon2id", "19", "m=65536,t=1,p=1", s, h),
        ),
        (
            "paralelismo zero",
            phc("argon2id", "19", "m=65536,t=3,p=0", s, h),
        ),
        (
            "memória inflada",
            phc("argon2id", "19", "m=4194304,t=3,p=1", s, h),
        ),
        (
            "passes inflados",
            phc("argon2id", "19", "m=65536,t=4294967295,p=1", s, h),
        ),
        (
            "paralelismo inflado",
            phc("argon2id", "19", "m=65536,t=3,p=255", s, h),
        ),
        (
            "parâmetro a mais",
            phc("argon2id", "19", "m=65536,t=3,p=1,keyid=AAAA", s, h),
        ),
        (
            "parâmetro faltando",
            phc("argon2id", "19", "m=65536,t=3", s, h),
        ),
        (
            "fora de ordem",
            phc("argon2id", "19", "t=3,m=65536,p=1", s, h),
        ),
        (
            "zero à esquerda",
            phc("argon2id", "19", "m=065536,t=3,p=1", s, h),
        ),
        ("sal curto", phc("argon2id", "19", p, "AAAAAAAAAAA", h)),
        ("saída curta", phc("argon2id", "19", p, s, &h[..22])),
        ("sem saída", format!("$argon2id$v=19${p}${s}").into_bytes()),
        (
            "sobra",
            [phc("argon2id", "19", p, s, h), b"$x".to_vec()].concat(),
        ),
        (
            "espaço no fim",
            [phc("argon2id", "19", p, s, h), b" ".to_vec()].concat(),
        ),
        (
            "não ASCII",
            [phc("argon2id", "19", p, s, h), "é".as_bytes().to_vec()].concat(),
        ),
        (
            "acima do limite",
            vec![b'$'; crate::limits::MAX_PIN_HASH + 1],
        ),
    ];
    let corpo = corpo_de_teste();
    let base = cruas(&corpo);
    let n = base.len() as u32;
    for (caso, hash) in &hashes {
        let c = CruaCfg {
            hash_pin: hash.clone(),
            ..CruaCfg::padrao()
        };
        let inicio = std::time::Instant::now();
        recusado(&forjar_cfg(VERSAO_CORPO, Some(&c), n, &base), caso, h);
        // Nenhum desses chega a rodar o Argon2 (o inflado não aloca 4 GiB).
        assert!(
            inicio.elapsed() < std::time::Duration::from_secs(5),
            "{caso}: demorou"
        );
    }
    // Controle: o hash do cofre de teste, remontado pelo `phc` do teste, abre.
    let c = CruaCfg {
        hash_pin: phc("argon2id", "19", p, s, h),
        ..CruaCfg::padrao()
    };
    assert!(ler(&forjar_cfg(VERSAO_CORPO, Some(&c), n, &base)).is_ok());

    let outros: Vec<(&str, CruaCfg)> = vec![
        (
            "trava 0",
            CruaCfg {
                minutos: 0,
                ..CruaCfg::padrao()
            },
        ),
        (
            "trava 1441",
            CruaCfg {
                minutos: 1441,
                ..CruaCfg::padrao()
            },
        ),
        (
            "trava u32::MAX",
            CruaCfg {
                minutos: u32::MAX,
                ..CruaCfg::padrao()
            },
        ),
        (
            "marca do backup 2",
            CruaCfg {
                ligado: 2,
                ..CruaCfg::padrao()
            },
        ),
        (
            "backup ligado sem UUID",
            CruaCfg {
                ligado: 1,
                ..CruaCfg::padrao()
            },
        ),
        (
            "UUID com ..",
            CruaCfg {
                uuid: b"../../x".to_vec(),
                ..CruaCfg::padrao()
            },
        ),
        (
            "UUID com - no começo",
            CruaCfg {
                uuid: b"-oX".to_vec(),
                ..CruaCfg::padrao()
            },
        ),
        (
            "UUID com espaço",
            CruaCfg {
                uuid: b"1234 ABCD".to_vec(),
                ..CruaCfg::padrao()
            },
        ),
        (
            "UUID de 37",
            CruaCfg {
                uuid: vec![b'a'; 37],
                ..CruaCfg::padrao()
            },
        ),
        (
            "UUID com U+202E",
            CruaCfg {
                uuid: "12\u{202E}34".as_bytes().to_vec(),
                ..CruaCfg::padrao()
            },
        ),
    ];
    for (caso, c) in &outros {
        recusado(&forjar_cfg(VERSAO_CORPO, Some(c), n, &base), caso, "falso");
    }
    // Controles: os limites aceitos abrem.
    for minutos in [1, 240, 1440] {
        let c = CruaCfg {
            minutos,
            ..CruaCfg::padrao()
        };
        assert!(
            ler(&forjar_cfg(VERSAO_CORPO, Some(&c), n, &base)).is_ok(),
            "{minutos}"
        );
    }
    let c = CruaCfg {
        ligado: 1,
        uuid: b"1234-ABCD".to_vec(),
        ..CruaCfg::padrao()
    };
    assert!(ler(&forjar_cfg(VERSAO_CORPO, Some(&c), n, &base)).is_ok());
}

/// Ataque: um hash rebaixado (custo baixo, feito para o PIN certo) não abre o cofre nem
/// passa a valer: a leitura recusa antes de qualquer conferência.
#[test]
fn v2_39_hash_rebaixado_do_pin_certo_recusado() {
    let fraco = ParametrosKdfPin {
        memoria_kib: 8,
        passes: 1,
        paralelismo: 1,
    };
    let rebaixado = crate::crypto::hash_pin(PIN_TESTE.as_bytes(), &fraco).unwrap();
    // O hash rebaixado é PHC válido de Argon2id (a biblioteca o confere)…
    {
        use argon2::password_hash::PasswordVerifier;
        assert!(
            argon2::Argon2::default()
                .verify_password(PIN_TESTE.as_bytes(), rebaixado.as_str())
                .is_ok()
        );
    }
    // …mas o corpo que o traz é recusado, e o `HashPin` não nasce dele.
    let corpo = corpo_de_teste();
    let base = cruas(&corpo);
    let c = CruaCfg {
        hash_pin: rebaixado.as_bytes().to_vec(),
        ..CruaCfg::padrao()
    };
    recusado(
        &forjar_cfg(VERSAO_CORPO, Some(&c), base.len() as u32, &base),
        "hash rebaixado",
        "falso",
    );
    assert!(HashPin::de_phc(rebaixado.as_bytes()).is_none());
}

/// O codificador também recusa uma configuração que não reabriria (tempo de trava fora
/// da faixa só existe se alguém mexer por dentro do crate; o corpo nunca o grava).
#[test]
fn serializar_recusa_configuracao_fora_da_regra() {
    let c = Configuracao::de_partes(
        HashPin::de_phc(cfg().hash_pin().bytes()).unwrap(),
        0,
        BackupExterno::desligado(),
    );
    assert_eq!(
        serializar(&c, 1, &[]).unwrap_err(),
        ErroCorpo::Inconsistente
    );
}

// ---- Fase 19c: D39 (geração, anti-rollback) ----

/// V2.44 (D39, D53), achado médio 2 (o repro `gravar.rs` do auditor regravava o mesmo
/// cofre aberto sem nada que distinguisse as versões): cada gravação sobe a geração, e
/// uma gravação que falha depois de selar queima a dela; uma
/// versão autêntica mais antiga copiada por cima é recusada com `Retrocesso`, sem
/// entrada e sem valor no erro; a troca de senha também sobe e também recusa.
#[cfg(unix)]
#[test]
fn v2_44_geracao_sobe_e_retrocesso_e_recusado() {
    use crate::gravacao::{ErroGravacao, gravar_entradas, trocar_senha};

    let dir = Dir::novo("geracao");
    let destino = dir.0.join("cofre.ward");
    let senha = SenhaMestra::nova(SENHA_MESTRA.to_owned());
    let nova = SenhaMestra::nova("falso-mestra-ward-02".to_owned());
    let entradas = cofre_de_teste();
    let geracao_do_arquivo = |s: &SenhaMestra| {
        let arquivo = std::fs::read(&destino).unwrap();
        abrir_entradas(&arquivo, s).unwrap().0.geracao()
    };

    let mut cofre = CofreAberto::novo(&senha, configuracao_nova()).unwrap();
    assert_eq!(cofre.geracao(), 0);
    gravar_entradas(&destino, &mut cofre, &entradas).unwrap();
    let n = cofre.geracao();
    assert_eq!(n, 1);
    assert_eq!(geracao_do_arquivo(&senha), n);
    let antigo = std::fs::read(&destino).unwrap();

    // n+1 por `gravar_entradas`.
    gravar_entradas(&destino, &mut cofre, &entradas).unwrap();
    assert_eq!(cofre.geracao(), n + 1);
    assert_eq!(geracao_do_arquivo(&senha), n + 1);

    // Falha na gravação depois de selar (destino vira diretório, o rename falha): o
    // arquivo segue com n+1 e o cofre aberto fica com a geração que selou (D53).
    let diretorio = dir.0.join("sou-diretorio");
    std::fs::create_dir(&diretorio).unwrap();
    assert!(gravar_entradas(&diretorio, &mut cofre, &entradas).is_err());
    assert_eq!(cofre.geracao(), n + 2);
    assert_eq!(geracao_do_arquivo(&senha), n + 1);

    // A troca de senha (mínimo n+1, o que o daemon viu; queimada n+2) sela n+3.
    let (trocado, _) = trocar_senha(&destino, &senha, &nova, n + 1, cofre.geracao()).unwrap();
    assert_eq!(trocado.geracao(), n + 3);
    assert_eq!(geracao_do_arquivo(&nova), n + 3);
    let do_n3 = std::fs::read(&destino).unwrap();
    let (_, lidas) = abrir_entradas_desde(&do_n3, &nova, n + 3, n + 3).unwrap();
    conferir_ida_e_volta(&entradas, &lidas);

    // Rollback com versão autêntica: o arquivo da geração n copiado por cima.
    std::fs::write(&destino, &antigo).unwrap();
    let arquivo = std::fs::read(&destino).unwrap();
    match abrir_entradas_desde(&arquivo, &senha, n + 3, n + 3) {
        Ok(_) => panic!("abriu um cofre mais antigo que o último visto"),
        Err(e) => {
            assert_eq!(e, ErroAbrir::Retrocesso);
            for texto in [format!("{e}"), format!("{e:?}")] {
                assert!(
                    !texto.contains("falso") && !texto.contains("acme"),
                    "{texto}"
                );
            }
        }
    }
    // Sem mínimo (0, primeira abertura) abre; a mesma geração do mínimo também.
    assert!(abrir_entradas_desde(&arquivo, &senha, 0, 0).is_ok());
    assert!(abrir_entradas_desde(&arquivo, &senha, n, 0).is_ok());
    // A troca de senha sobre o arquivo antigo também recusa, e não toca no disco.
    assert_eq!(
        trocar_senha(&destino, &senha, &nova, n + 3, n + 3).unwrap_err(),
        ErroGravacao::Retrocesso
    );
    assert!(std::fs::read(&destino).unwrap() == antigo);
}

/// V2.44 (D39): o cofre aberto na geração `u64::MAX` recusa gravar sem tocar no disco.
#[cfg(unix)]
#[test]
fn v2_44_geracao_esgotada_nao_toca_no_disco() {
    use crate::gravacao::{ErroGravacao, gravar_entradas};

    let dir = Dir::novo("esgotada");
    let destino = dir.0.join("cofre.ward");
    let senha = SenhaMestra::nova(SENHA_MESTRA.to_owned());
    let mut cofre = CofreAberto::novo(&senha, configuracao_nova()).unwrap();
    gravar_entradas(&destino, &mut cofre, &[]).unwrap();
    let antes = std::fs::read(&destino).unwrap();
    cofre.queimar(u64::MAX);
    assert_eq!(
        gravar_entradas(&destino, &mut cofre, &cofre_de_teste()).unwrap_err(),
        ErroGravacao::GeracaoEsgotada
    );
    assert_eq!(cofre.geracao(), u64::MAX);
    assert!(std::fs::read(&destino).unwrap() == antes);
    let nomes: Vec<_> = std::fs::read_dir(&dir.0).unwrap().collect();
    assert_eq!(nomes.len(), 1, "sobrou temporário");
}

/// V2.44 (D39): `serializar` recusa a geração 0, e a leitura recusa corpo v2 (sem
/// geração) e geração 0 com o erro genérico.
#[test]
fn v2_44_geracao_zero_e_corpo_v2_recusados() {
    assert_eq!(
        serializar(cfg(), 0, &cofre_de_teste()).unwrap_err(),
        ErroCorpo::Inconsistente
    );
    let corpo = corpo_de_teste();
    // Corpo v2: a versão 2 seguida direto da configuração (sem os 8 bytes da geração).
    let mut v2 = vec![2u8, 0];
    v2.extend_from_slice(&corpo[10..]);
    recusado(&v2, "corpo v2", "falso-mestra");
    let mut zero = corpo.to_vec();
    zero[2..10].copy_from_slice(&0u64.to_le_bytes());
    recusado(&zero, "geração 0", "geração");
    // Qualquer outra geração ≥ 1 abre e volta igual.
    for g in [1u64, 2, 0x0102_0304_0506_0708, u64::MAX] {
        let mut v = corpo.to_vec();
        v[2..10].copy_from_slice(&g.to_le_bytes());
        let (_, lida, _) = desserializar(&v).unwrap();
        assert_eq!(lida, g);
    }
}

// ---- Fase 19c: D38, D42, D43 no corpo forjado ----

/// V2.45, V2.48 e V2.49: a leitura do corpo aplica as mesmas regras da validação. Um
/// corpo forjado (escrito por fora, cifrado com a chave certa) com texto fora da lista
/// branca, variável sem sufixo ou usuário de `ssh` perigoso é recusado com o erro
/// genérico, sem o valor.
#[test]
fn ataque_corpo_forjado_com_texto_variavel_e_usuario() {
    let corpo = corpo_de_teste();
    let base = cruas(&corpo);
    let mut casos: Vec<(String, Vec<u8>, String)> = Vec::new();
    let mut com = |caso: String, valor: String, f: &dyn Fn(&mut Vec<Crua>)| {
        let mut c = base.clone();
        f(&mut c);
        casos.push((caso, forjar(&c), valor));
    };
    for ch in [
        "\u{3164}",
        "\u{1D173}",
        "\u{2800}",
        "\u{FE0F}",
        "\u{00A0}",
        "a\u{0301}",
    ] {
        let falso = format!("acme{ch}prod");
        let b = falso.clone().into_bytes();
        let login = indice(&base, "acme-wp-dev");
        let b1 = b.clone();
        com(format!("{ch:?} no projeto"), falso.clone(), &move |c| {
            c[login].projeto = b1.clone()
        });
        let b2 = b.clone();
        com(format!("{ch:?} na tag"), falso.clone(), &move |c| {
            c[login].tags = vec![b2.clone()]
        });
        let b3 = b.clone();
        com(
            format!("{ch:?} no usuário do login"),
            falso.clone(),
            &move |c| c[login].dados[1] = b3.clone(),
        );
        let k = indice(&base, "gama-generico");
        let b4 = b.clone();
        com(
            format!("{ch:?} no nome do extra"),
            falso.clone(),
            &move |c| c[k].extras[0].0 = b4.clone(),
        );
        let b5 = b.clone();
        com(
            format!("{ch:?} no nome do anexo"),
            falso.clone(),
            &move |c| c[k].anexos[0].0 = b5.clone(),
        );
    }
    let api = indice(&base, "beta-github-api");
    for var in [
        "GIT_PROXY_COMMAND",
        "PAGER",
        "LD_PRELOAD",
        "HTTPS_PROXY",
        "GIT_ASKPASS",
    ] {
        com(format!("variável {var}"), var.to_owned(), &move |c| {
            c[api].dados[2] = var.as_bytes().to_vec()
        });
    }
    let ssh = indice(&base, "acme-ssh-senha-stage");
    let salto = indice(&base, "acme-ssh-salto");
    let ftp = indice(&base, "beta-sftp-dev");
    for usuario in ["u;id", "u'$(id)'", "u%h", "-deploy@x", "u v", "usuário"] {
        com(
            format!("usuário {usuario}"),
            usuario.to_owned(),
            &move |c| c[ssh].dados[2] = usuario.as_bytes().to_vec(),
        );
        com(
            format!("usuário do salto {usuario}"),
            usuario.to_owned(),
            &move |c| c[salto].dados[2] = usuario.as_bytes().to_vec(),
        );
        com(
            format!("usuário do ftp {usuario}"),
            usuario.to_owned(),
            &move |c| c[ftp].dados[3] = usuario.as_bytes().to_vec(),
        );
    }
    for (caso, forjado, valor) in &casos {
        recusado(forjado, caso, valor);
    }
    // Controle: `@` no usuário (D43) e acento Latin no projeto passam.
    let mut c = base.clone();
    c[ssh].dados[2] = b"deploy@cliente.com.br".to_vec();
    c[ftp].dados[3] = b"deploy@cliente.com.br".to_vec();
    let login = indice(&base, "acme-wp-dev");
    c[login].projeto = "São Paulo".as_bytes().to_vec();
    assert!(ler(&forjar(&c)).is_ok());
}

/// V2.51 (D45): a mensagem de `GrandeDemais` sai das constantes de `limites` (mudar
/// `MAX_BODY` ou `MAX_ENTRIES` muda a mensagem, e este teste confere contra elas).
#[test]
fn v2_51_mensagem_de_grande_demais_acompanha_os_limites() {
    use crate::limits::{MAX_BODY, MAX_ENTRIES, MIB};
    let msg = format!("{}", ErroCorpo::GrandeDemais);
    let milhar = |n: usize| {
        let s = n.to_string();
        let mut v = String::new();
        for (i, c) in s.chars().enumerate() {
            if i > 0 && (s.len() - i).is_multiple_of(3) {
                v.push(' ');
            }
            v.push(c);
        }
        v
    };
    assert_eq!(
        msg,
        format!(
            "cofre grande demais (limite de {} MiB e {} entradas)",
            MAX_BODY / MIB,
            milhar(MAX_ENTRIES)
        )
    );
    const {
        assert!(
            MAX_BODY.is_multiple_of(MIB),
            "o limite do corpo é um número inteiro de MiB"
        )
    };
}

/// V2.58 (D55): a leitura do corpo aplica a regra do último rótulo. Um corpo forjado
/// com a URL do `login` ou do `api` lida como IPv4 pelo navegador é recusado com o erro
/// genérico, sem o valor; um domínio com dígito só no começo de um rótulo do meio abre.
#[test]
fn v2_58_corpo_forjado_com_ultimo_rotulo_numerico() {
    let corpo = corpo_de_teste();
    let base = cruas(&corpo);
    let login = indice(&base, "acme-wp-dev");
    let api = indice(&base, "beta-github-api");
    for url in [
        "https://0x7f000001/",
        "https://0x7f.0.0.1/",
        "https://1.2.3.0x4/",
        "https://x.123/",
        "https://x.0x10/",
        "https://x.example.9z/",
    ] {
        for (i, nome) in [(login, "login"), (api, "api")] {
            let mut c = base.clone();
            c[i].dados[0] = url.as_bytes().to_vec();
            recusado(&forjar(&c), &format!("url {url} no {nome}"), url);
        }
    }
    let mut c = base.clone();
    c[login].dados[0] = b"https://1password.com/".to_vec();
    c[api].dados[0] = b"https://x1.example2/".to_vec();
    assert!(ler(&forjar(&c)).is_ok());
}

// ---- Fase 22d: D65 d (datas de criação e alteração, corpo v4; V3.41) ----

/// A regra das datas: faixa de 1 a `MAX_DATE`, criação só com alteração e não depois
/// dela; a alteração nunca desce, nem com o relógio voltando.
#[test]
fn d65_regra_das_datas() {
    assert_eq!(Datas::carimbo_novo(0), None);
    assert_eq!(Datas::carimbo_novo(MAX_DATE + 1), None);
    assert_eq!(Datas::carimbo_novo(u64::MAX), None);
    let d = Datas::carimbo_novo(1_000).unwrap();
    assert_eq!((d.criada_em(), d.alterada_em()), (Some(1_000), Some(1_000)));
    let depois = d.alterado(5_000).unwrap();
    assert_eq!(
        (depois.criada_em(), depois.alterada_em()),
        (Some(1_000), Some(5_000))
    );
    // Relógio que volta: a alteração fica onde estava, a criação não muda.
    let voltou = depois.alterado(10).unwrap();
    assert_eq!(
        (voltou.criada_em(), voltou.alterada_em()),
        (Some(1_000), Some(5_000))
    );
    assert_eq!(depois.alterado(0), None);
    assert_eq!(depois.alterado(MAX_DATE + 1), None);
    // Sem datas: a alteração ganha data, a criação segue desconhecida.
    let so_alterada = Datas::NENHUMA.alterado(77).unwrap();
    assert_eq!(
        (so_alterada.criada_em(), so_alterada.alterada_em()),
        (None, Some(77))
    );
    assert_eq!(Datas::de_partes(Some(5), Some(4)), None);
    assert_eq!(Datas::de_partes(Some(5), None), None);
    assert_eq!(Datas::de_partes(None, Some(0)), None);
    assert_eq!(Datas::de_partes(Some(0), Some(1)), None);
}

/// V3.41: o corpo v3 (sem datas) é recusado como versão desconhecida, com o erro
/// genérico; o v4 sem os bytes das datas, ou com a versão trocada para 3, também.
#[test]
fn d65_corpo_v3_recusado() {
    let corpo = corpo_de_teste();
    let base = cruas(&corpo);
    let n = base.len() as u32;
    let cfg = CruaCfg::padrao();
    recusado(
        &forjar_bruto(3, Some(&cfg), n, &base, false),
        "corpo v3",
        "falso",
    );
    recusado(
        &forjar_bruto(3, Some(&cfg), n, &base, true),
        "v4 rotulado 3",
        "falso",
    );
    recusado(
        &forjar_bruto(4, Some(&cfg), n, &base, false),
        "v4 sem datas",
        "falso",
    );
    recusado(
        &forjar_bruto(5, Some(&cfg), n, &base, true),
        "versão 5",
        "falso",
    );
    // O cofre v3 vazio também: a versão é conferida antes de tudo.
    recusado(
        &forjar_bruto(3, Some(&cfg), 0, &[], false),
        "v3 vazio",
        "falso",
    );
    // Controle: a mesma montagem com a versão 4 e as datas abre.
    assert!(ler(&forjar_bruto(4, Some(&cfg), n, &base, true)).is_ok());
    assert_eq!(corpo[..2], [4, 0]);
}

/// V3.41: datas forjadas no corpo (fora da faixa, criação depois da alteração,
/// criação sem alteração) recusam o corpo inteiro com o erro genérico, sem o valor; as
/// formas válidas abrem e voltam iguais.
#[test]
fn d65_datas_forjadas_no_corpo() {
    let corpo = corpo_de_teste();
    let base = cruas(&corpo);
    let i = indice(&base, "acme-pg-dev");
    let com = |criada: u64, alterada: u64| {
        let mut c = base.clone();
        c[i].criada = criada;
        c[i].alterada = alterada;
        forjar(&c)
    };
    for (caso, criada, alterada) in [
        ("criação depois da alteração", 1_762_000_001, 1_762_000_000),
        ("criação sem alteração", 1_762_000_000, 0),
        ("criação acima do máximo", MAX_DATE + 1, MAX_DATE + 1),
        ("alteração acima do máximo", 0, MAX_DATE + 1),
        ("as duas em u64::MAX", u64::MAX, u64::MAX),
        ("alteração em u64::MAX", 1, u64::MAX),
    ] {
        recusado(&com(criada, alterada), caso, &alterada.to_string());
    }
    for (criada, alterada) in [(0, 0), (0, 7), (7, 7), (1, MAX_DATE), (MAX_DATE, MAX_DATE)] {
        let v = com(criada, alterada);
        let (c, entradas) = ler_tudo(&v).unwrap();
        conferir_aceito(&v, &c, &entradas);
        let e = entradas.iter().find(|e| e.nome() == "acme-pg-dev").unwrap();
        let opc = |d: u64| (d != 0).then_some(d);
        assert_eq!(e.datas().criada_em(), opc(criada));
        assert_eq!(e.datas().alterada_em(), opc(alterada));
    }
}

/// V3.41: as datas não guardam nem vazam segredo: o corpo com datas tem os mesmos
/// bytes do corpo sem elas fora dos 16 bytes de cada entrada, e mexer só nas datas não
/// muda nenhum outro campo.
#[test]
fn d65_datas_so_mudam_os_16_bytes_da_entrada() {
    let sem = corpo_de_teste();
    let mut entradas = cofre_de_teste();
    for e in &mut entradas {
        e.definir_datas(Datas::carimbo_novo(1_762_000_000).unwrap());
    }
    let com = serializar(cfg(), 1, &entradas).unwrap();
    assert_eq!(sem.len(), com.len());
    let diferentes = sem.iter().zip(com.iter()).filter(|(a, b)| a != b).count();
    // 1_762_000_000 = 0x6905_C680: 4 bytes não nulos em cada uma das 2 datas.
    assert_eq!(diferentes, entradas.len() * 2 * 4);
}

/// V3.41: um cofre v3 autêntico no disco (selado com a chave certa, corpo v3 sem datas)
/// é recusado pela abertura do daemon (`abrir_entradas_lida`) como corpo inválido, não
/// como senha errada, e a abertura não toca no arquivo nem deixa temporário.
#[cfg(unix)]
#[test]
fn d65_cofre_v3_no_disco_recusado_sem_tocar_no_arquivo() {
    let dir = Dir::novo("v3");
    let destino = dir.0.join("cofre.ward");
    let senha = SenhaMestra::nova(SENHA_MESTRA.to_owned());
    let cofre = CofreAberto::novo(&senha, configuracao_nova()).unwrap();
    let mut c = CruaCfg::de(cofre.configuracao());
    c.geracao = Some(1);
    let base = cruas(&corpo_de_teste());
    let v3 = forjar_bruto(3, Some(&c), base.len() as u32, &base, false);
    let arquivo = cofre.selar(&v3).unwrap();
    crate::gravacao::gravar(&destino, &arquivo).unwrap();
    let antes = std::fs::read(&destino).unwrap();
    let meta = std::fs::metadata(&destino).unwrap();
    for (minima, queimada) in [(0, 0), (1, 1), (5, 9)] {
        let lido = std::fs::read(&destino).unwrap();
        assert_eq!(
            abrir_entradas_lida(&lido, &senha, minima, queimada).map(|_| ()),
            Err(ErroAbrir::Corpo(ErroCorpo::Invalido))
        );
    }
    let depois = std::fs::metadata(&destino).unwrap();
    assert!(std::fs::read(&destino).unwrap() == antes, "o arquivo mudou");
    assert_eq!(meta.modified().unwrap(), depois.modified().unwrap());
    let nomes: Vec<_> = std::fs::read_dir(&dir.0).unwrap().collect();
    assert_eq!(nomes.len(), 1, "sobrou arquivo além do cofre");
    // Controle: o mesmo cofre com o corpo v4 abre.
    let v4 = forjar_bruto(4, Some(&c), base.len() as u32, &base, true);
    assert!(abrir_entradas(&cofre.selar(&v4).unwrap(), &senha).is_ok());
}
