//! Corpo do cofre sob um alocador global de teste (V2.32, V2.33, S8), com a seção de
//! configuração do corpo v2 (D30): o PIN de teste e o hash dele também são marcas.
//!
//! Um binário de teste próprio por causa do `#[global_allocator]`. O alocador
//! [`Observador`] repassa tudo ao `System` e, só na thread que pediu, (a) conta os
//! pedidos e guarda o maior e (b) procura as marcas dos segredos falsos em todo bloco
//! que é liberado ou realocado. Sem alocar nada dentro do alocador.
//!
//! `unsafe` só aqui, no alocador de teste (implementar `GlobalAlloc` exige): nenhum
//! código do Ward roda com ele fora deste binário de teste. É a única exceção aceita à
//! regra "`unsafe` só no `ward-plataforma`" (D31, V2.40).
//!
//! Todo bloco nasce zerado (`alloc` usa `alloc_zeroed`, D31) e a realocação é a padrão
//! do `GlobalAlloc` (bloco novo pelo `alloc` daqui, cópia e `dealloc` do antigo). Assim
//! a procura nunca lê byte que não foi escrito: memória não inicializada, no modelo do
//! Rust, não tem valor definido.

#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

use ward_core as wc;
use ward_core::configuracao::{Configuracao, HashPin};
use ward_core::corpo::{ErroCorpo, abrir_entradas, desserializar, serializar};
use ward_core::entrada::{
    Entrada, NivelAcesso, Rascunho, RascunhoAnexo, RascunhoDados, RascunhoExtra, Segredo,
};
use ward_core::formato::{CofreAberto, SenhaMestra};
use ward_core::gravacao::gravar_entradas;
use ward_core::limites::{
    MAX_ANEXOS_SOMADOS, MAX_CORPO, MAX_ENTRADAS, MAX_EXTRAS, MAX_NOTA, MAX_VALOR_EXTRA,
};

#[path = "../src/corpo/cofre_teste.rs"]
mod cofre_teste;
use cofre_teste::{
    PIN_TESTE, SEGREDOS, SENHA_MESTRA, cofre_de_teste, configuracao_de_teste, configuracao_nova,
    validada,
};

// ---- O alocador ----

thread_local! {
    static CONTAR: Cell<bool> = const { Cell::new(false) };
    static PEDIDOS: Cell<usize> = const { Cell::new(0) };
    static MAIOR: Cell<usize> = const { Cell::new(0) };
    static PROCURAR: Cell<bool> = const { Cell::new(false) };
    static ACHADOS: Cell<usize> = const { Cell::new(0) };
    /// Marca extra desta thread (a saída B64 do hash do PIN, que tem sal aleatório e
    /// só se conhece em tempo de execução), num buffer fixo para não alocar.
    static MARCA_EXTRA: Cell<([u8; 64], usize)> = const { Cell::new(([0; 64], 0)) };
}

/// Procura também `marca` (até 64 bytes) nesta thread.
fn marcar_extra(marca: &[u8]) {
    let mut buf = [0u8; 64];
    buf[..marca.len()].copy_from_slice(marca);
    MARCA_EXTRA.with(|m| m.set((buf, marca.len())));
}

/// Primeiros bytes das marcas, para a procura passar uma vez só por bloco.
const PRIMEIROS: [bool; 256] = {
    let mut t = [false; 256];
    let mut i = 0;
    while i < SEGREDOS.len() {
        t[SEGREDOS[i].as_bytes()[0] as usize] = true;
        i += 1;
    }
    t[PIN_TESTE.as_bytes()[0] as usize] = true;
    t
};

/// Se a marca começa em `i` no bloco.
///
/// # Safety
/// `ptr` aponta para um bloco vivo de `tamanho` bytes, todos inicializados.
unsafe fn casa(ptr: *const u8, tamanho: usize, i: usize, m: &[u8]) -> bool {
    if m.is_empty() || i + m.len() > tamanho {
        return false;
    }
    for (k, &esperado) in m.iter().enumerate() {
        // SAFETY: `i + k < i + m.len() <= tamanho`, dentro do bloco (contrato da função).
        if unsafe { core::ptr::read_volatile(ptr.add(i + k)) } != esperado {
            return false;
        }
    }
    true
}

fn ligado(chave: &'static std::thread::LocalKey<Cell<bool>>) -> bool {
    chave.try_with(Cell::get).unwrap_or(false)
}

fn contar(tamanho: usize) {
    if ligado(&CONTAR) {
        let _ = PEDIDOS.try_with(|p| p.set(p.get() + 1));
        let _ = MAIOR.try_with(|m| m.set(m.get().max(tamanho)));
    }
}

/// Procura as marcas no bloco que vai ser liberado ou realocado.
///
/// # Safety
/// `ptr` aponta para um bloco vivo de pelo menos `tamanho` bytes, alocado por este
/// alocador.
unsafe fn procurar(ptr: *const u8, tamanho: usize) {
    if !ligado(&PROCURAR) || ptr.is_null() {
        return;
    }
    let mut i = 0;
    while i < tamanho {
        // SAFETY: `i < tamanho` e o bloco tem `tamanho` bytes (contrato da função), todos
        // inicializados: o bloco nasceu de `alloc_zeroed` (ver `Observador`). A leitura
        // é volátil para o compilador não tirá-la de um bloco que vai ser liberado.
        let b = unsafe { core::ptr::read_volatile(ptr.add(i)) };
        if PRIMEIROS[b as usize] {
            for marca in SEGREDOS.iter().chain([&PIN_TESTE]) {
                let m = marca.as_bytes();
                // SAFETY: o mesmo bloco, com o contrato desta função.
                if m[0] == b && unsafe { casa(ptr, tamanho, i, m) } {
                    let _ = ACHADOS.try_with(|a| a.set(a.get() + 1));
                }
            }
        }
        if let Ok((extra, n)) = MARCA_EXTRA.try_with(Cell::get)
            && n > 0
            && extra[0] == b
            // SAFETY: o mesmo bloco, com o contrato desta função.
            && unsafe { casa(ptr, tamanho, i, &extra[..n]) }
        {
            let _ = ACHADOS.try_with(|a| a.set(a.get() + 1));
        }
        i += 1;
    }
}

struct Observador;

// SAFETY: tudo é repassado ao `System` com o mesmo `Layout`; o observador só lê o
// bloco antes de devolvê-lo e só mexe em `Cell`s da própria thread, sem alocar. Todo
// bloco sai zerado (`alloc` → `alloc_zeroed`, D31), então a procura só lê bytes
// inicializados. `realloc` fica com a implementação padrão do trait, que pede o bloco
// novo a `alloc` (zerado e contado), copia e libera o antigo por `dealloc` (procurado):
// a cópia antiga deixada pela realocação também é procurada.
unsafe impl GlobalAlloc for Observador {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        contar(layout.size());
        // SAFETY: o contrato de `GlobalAlloc::alloc` (tamanho não nulo) é o mesmo de
        // `alloc_zeroed`, repassado ao `System`. Zerar o bloco não muda nada para quem
        // pediu (bytes não inicializados podem ter qualquer valor, inclusive zero).
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        contar(layout.size());
        // SAFETY: mesmo contrato de `GlobalAlloc::alloc_zeroed`, repassado.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` é um bloco vivo deste alocador com `layout.size()` bytes, todos
        // inicializados (nasceu de `alloc_zeroed`).
        unsafe { procurar(ptr, layout.size()) };
        // SAFETY: mesmo contrato de `GlobalAlloc::dealloc`, repassado.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALOCADOR: Observador = Observador;

/// Liga a contagem nesta thread, roda `f` e devolve (resultado, pedidos, maior pedido).
fn contando<T>(f: impl FnOnce() -> T) -> (T, usize, usize) {
    PEDIDOS.with(|p| p.set(0));
    MAIOR.with(|m| m.set(0));
    CONTAR.with(|c| c.set(true));
    let r = f();
    CONTAR.with(|c| c.set(false));
    (r, PEDIDOS.with(Cell::get), MAIOR.with(Cell::get))
}

/// Liga a procura nesta thread, roda `f` e devolve quantas marcas achou em blocos
/// liberados ou realocados.
fn procurando(f: impl FnOnce()) -> usize {
    ACHADOS.with(|a| a.set(0));
    PROCURAR.with(|p| p.set(true));
    f();
    PROCURAR.with(|p| p.set(false));
    ACHADOS.with(Cell::get)
}

// ---- Corpos forjados à mão (o formato do topo de `corpo.rs`) ----

fn campo(v: &mut Vec<u8>, b: &[u8]) {
    v.extend_from_slice(&(b.len() as u32).to_le_bytes());
    v.extend_from_slice(b);
}

/// Versão (4, D65 d) e geração (D39) no começo de todo corpo forjado aqui.
fn inicio(geracao: u64) -> Vec<u8> {
    let mut v = vec![4, 0];
    v.extend_from_slice(&geracao.to_le_bytes());
    v
}

/// Onde começa o campo do hash do PIN: depois da versão (2) e da geração (8).
const P_HASH: usize = 2 + 8;

/// O corpo vazio do cofre de teste: versão 4 e a configuração (D30, D39), mais a
/// contagem.
fn corpo_vazio() -> Vec<u8> {
    serializar(configuracao_de_teste(), 1, &[])
        .unwrap()
        .to_vec()
}

/// Versão e configuração do cofre de teste (o corpo vazio sem a contagem final).
fn cabecalho() -> Vec<u8> {
    let mut v = corpo_vazio();
    v.truncate(v.len() - 4);
    v
}

/// O texto PHC do hash do PIN do cofre de teste (lido do corpo vazio).
fn hash_de_teste() -> Vec<u8> {
    let v = corpo_vazio();
    let n = u32::from_le_bytes(v[P_HASH..P_HASH + 4].try_into().unwrap()) as usize;
    v[P_HASH + 4..P_HASH + 4 + n].to_vec()
}

/// Versão e uma seção de configuração com o hash do PIN dado (240 minutos, backup
/// desligado).
fn cabecalho_com_hash(hash: &[u8]) -> Vec<u8> {
    let mut v = inicio(1);
    campo(&mut v, hash);
    v.extend_from_slice(&240u32.to_le_bytes());
    v.push(0);
    campo(&mut v, b"");
    v
}

/// Começo de um corpo com uma entrada genérica, até antes das tags.
fn generico_ate_tags() -> Vec<u8> {
    let mut v = cabecalho();
    v.extend_from_slice(&1u32.to_le_bytes());
    for c in [&b"generico"[..], b"livre", b"x", b"g", b"dev", b""] {
        campo(&mut v, c);
    }
    v
}

fn com_contagem(mut v: Vec<u8>, n: u32) -> Vec<u8> {
    v.extend_from_slice(&n.to_le_bytes());
    v
}

/// Entrada genérica válida até antes dos anexos (0 tags, 1 extra).
fn generico_ate_anexos() -> Vec<u8> {
    let mut v = com_contagem(generico_ate_tags(), 0);
    v.extend_from_slice(&1u32.to_le_bytes());
    campo(&mut v, b"A");
    v.push(0);
    campo(&mut v, b"v");
    v
}

fn forjados() -> Vec<(&'static str, Vec<u8>)> {
    let mut casos = Vec::new();
    // Seção de configuração (D30): hash do PIN com comprimento u32::MAX, vazio, com
    // custo inflado (4 GiB), o PIN em claro, a versão 1 e o corpo sem a seção. Cada um
    // com uma entrada válida depois, para a 2ª passada (que aloca) não começar.
    let entrada = com_contagem(generico_ate_anexos(), 0)[cabecalho().len()..].to_vec();
    let mut v = inicio(1);
    v.extend_from_slice(&u32::MAX.to_le_bytes());
    v.extend_from_slice(&entrada);
    casos.push(("hash do PIN com u32::MAX", v));
    let inflado = String::from_utf8(hash_de_teste())
        .unwrap()
        .replace("m=65536", "m=4194304");
    let rebaixado = String::from_utf8(hash_de_teste())
        .unwrap()
        .replace("t=3", "t=1");
    for (caso, hash) in [
        ("sem hash do PIN", &b""[..]),
        ("hash do PIN inflado", inflado.as_bytes()),
        ("hash do PIN rebaixado", rebaixado.as_bytes()),
        ("PIN em claro", PIN_TESTE.as_bytes()),
    ] {
        let mut v = cabecalho_com_hash(hash);
        v.extend_from_slice(&entrada);
        casos.push((caso, v));
    }
    let mut v = com_contagem(generico_ate_anexos(), 0);
    v[0] = 1;
    casos.push(("versão 1", v));
    // D39: a versão 2 (sem geração), a 3 (sem datas, D65 d) e a 4 com geração 0.
    let mut v = com_contagem(generico_ate_anexos(), 0);
    v[0] = 2;
    casos.push(("versão 2", v));
    let mut v = com_contagem(generico_ate_anexos(), 0);
    v[0] = 3;
    casos.push(("versão 3", v));
    let mut v = com_contagem(generico_ate_anexos(), 0);
    v[2..P_HASH].copy_from_slice(&0u64.to_le_bytes());
    casos.push(("geração 0", v));
    let mut v = inicio(1);
    v.extend_from_slice(&entrada);
    casos.push(("sem a seção", v));
    // Comprimento u32::MAX no primeiro campo e num campo do meio.
    let mut v = cabecalho();
    v.extend_from_slice(&1u32.to_le_bytes());
    v.extend_from_slice(&u32::MAX.to_le_bytes());
    casos.push(("campo com u32::MAX", v));
    let mut v = cabecalho();
    v.extend_from_slice(&1u32.to_le_bytes());
    campo(&mut v, b"generico");
    campo(&mut v, b"livre");
    campo(&mut v, b"x");
    v.extend_from_slice(&u32::MAX.to_le_bytes());
    casos.push(("grupo com u32::MAX", v));
    // Anexo declarando 4 GiB (u32::MAX), o limite + 1 e o limite (D29), com só 5 bytes
    // de verdade.
    let limite = MAX_ANEXOS_SOMADOS as u32;
    for tamanho in [u32::MAX, limite + 1, limite] {
        let mut v = com_contagem(generico_ate_anexos(), 1);
        campo(&mut v, b"a.bin");
        v.extend_from_slice(&tamanho.to_le_bytes());
        v.extend_from_slice(b"pouco");
        casos.push(("anexo de 4 GiB ou acima do que existe", v));
    }
    // Anexos que somam 256 KiB + 1 (D29), com os bytes de verdade no buffer: um de
    // metade do limite e outro de metade + 1.
    let mut v = com_contagem(generico_ate_anexos(), 2);
    let metade = MAX_ANEXOS_SOMADOS / 2;
    campo(&mut v, b"a.bin");
    campo(&mut v, &vec![0x5a; metade]);
    campo(&mut v, b"b.bin");
    campo(&mut v, &vec![0x5a; metade + 1]);
    casos.push(("anexos somam o limite + 1", v));
    // Um anexo só, de verdade, com o limite + 1.
    let mut v = com_contagem(generico_ate_anexos(), 1);
    campo(&mut v, b"a.bin");
    campo(&mut v, &vec![0x5a; MAX_ANEXOS_SOMADOS + 1]);
    casos.push(("anexo com o limite + 1", v));
    // 2^32 − 1 entradas e 10 001.
    let inicio = cabecalho().len() + 4;
    for n in [u32::MAX, (MAX_ENTRADAS + 1) as u32, 2] {
        let mut v = cabecalho();
        v.extend_from_slice(&n.to_le_bytes());
        v.extend_from_slice(&generico_ate_anexos()[inicio..]);
        v.extend_from_slice(&0u32.to_le_bytes());
        casos.push(("entradas demais", v));
    }
    // 17 tags, 65 extras, 33 anexos (declarados, com um item de cada).
    let mut v = com_contagem(generico_ate_tags(), 17);
    campo(&mut v, b"t");
    casos.push(("17 tags", v));
    let mut v = com_contagem(com_contagem(generico_ate_tags(), 0), 65);
    campo(&mut v, b"A");
    v.push(0);
    campo(&mut v, b"v");
    casos.push(("65 extras", v));
    for n in [33u32, u32::MAX] {
        let mut v = com_contagem(generico_ate_anexos(), n);
        campo(&mut v, b"a");
        campo(&mut v, b"x");
        casos.push(("anexos demais", v));
    }
    casos
}

// ---- V2.32 ----

#[test]
fn v2_32_corpo_forjado_recusado_rapido_sem_alocar() {
    // Controle: o corpo forjado bem formado (uma entrada genérica, sem datas) é aceito.
    let mut ok = com_contagem(generico_ate_anexos(), 0);
    ok.extend_from_slice(&[0; 16]);
    assert_eq!(desserializar(&ok).map(|(_, _, e)| e.len()), Ok(1));
    // Controle da seção: o cabeçalho remontado com o hash de teste é o do módulo.
    assert!(cabecalho_com_hash(&hash_de_teste()) == cabecalho());

    for (caso, forjado) in forjados() {
        let inicio = Instant::now();
        let (r, pedidos, maior) =
            contando(|| catch_unwind(AssertUnwindSafe(|| desserializar(&forjado))));
        let tempo = inicio.elapsed();
        let r = r.unwrap_or_else(|_| panic!("{caso}: pânico"));
        assert_eq!(r.err(), Some(ErroCorpo::Invalido), "{caso}");
        assert!(tempo < Duration::from_millis(100), "{caso}: {tempo:?}");
        // A estrutura é conferida antes de alocar: nenhum pedido, nem pequeno.
        assert_eq!(pedidos, 0, "{caso}: alocou");
        assert!(maior <= forjado.len(), "{caso}: pedido de {maior} bytes");
    }

    // Corpo acima de 64 MiB.
    let mut grande = vec![0u8; MAX_CORPO + 1];
    grande[0] = 2;
    let inicio = Instant::now();
    let (r, pedidos, _) = contando(|| desserializar(&grande));
    assert_eq!(r.err(), Some(ErroCorpo::Invalido));
    assert!(inicio.elapsed() < Duration::from_millis(100));
    assert_eq!(pedidos, 0);
}

#[test]
fn v2_32_corpo_autentico_aloca_so_o_que_existe() {
    // O cofre de teste lido de verdade: o maior pedido é o corpo (cópia de um campo) ou
    // o vetor das 22 entradas que existem, nunca algo do tamanho declarado.
    let corpo = serializar(configuracao_de_teste(), 1, &cofre_de_teste()).unwrap();
    let (r, pedidos, maior) = contando(|| desserializar(&corpo));
    assert_eq!(r.map(|(_, _, e)| e.len()), Ok(22));
    assert!(pedidos > 0);
    let teto = corpo.len().max(22 * std::mem::size_of::<Entrada>());
    assert!(maior <= teto, "pedido de {maior} > {teto}");
}

/// Uma `nota segura` no máximo do modelo: texto de 1 MiB, 64 extras de 64 KiB e um
/// anexo de 256 KiB (~5,3 MiB). Sem entrega, fica fora do orçamento da máscara (D29),
/// então é o jeito de montar um corpo grande com entradas válidas.
fn nota_cheia(nome: &str) -> Entrada {
    let mut r = Rascunho {
        nome: nome.to_owned(),
        grupo: "g".to_owned(),
        ambiente: "dev".to_owned(),
        projeto: String::new(),
        tags: Vec::new(),
        dados: RascunhoDados::NotaSegura {
            texto: Some(Segredo::de_bytes(vec![b'n'; MAX_NOTA])),
        },
        extras: Vec::new(),
        anexos: Vec::new(),
    };
    for i in 0..MAX_EXTRAS {
        r.extras.push(RascunhoExtra {
            nome: format!("e{i:02}"),
            valor: Segredo::de_bytes(vec![b'v'; MAX_VALOR_EXTRA]),
            segredo: true,
        });
    }
    r.anexos.push(RascunhoAnexo {
        nome: "a.bin".to_owned(),
        conteudo: Segredo::de_bytes(vec![0x5a; MAX_ANEXOS_SOMADOS]),
    });
    validada(r, NivelAcesso::Livre)
}

#[test]
fn v2_32_codificador_recusa_acima_do_limite_antes_de_alocar() {
    // Notas cheias até passar de 64 MiB (com anexos de 256 KiB, D29, uma entrada sozinha
    // não chega perto do limite do corpo).
    let mut entradas: Vec<Entrada> = Vec::new();
    let mut aproximado = 0usize;
    while aproximado <= MAX_CORPO {
        entradas.push(nota_cheia(&format!("nota-{:03}", entradas.len())));
        aproximado += MAX_NOTA + MAX_EXTRAS * MAX_VALOR_EXTRA + MAX_ANEXOS_SOMADOS;
    }
    let cfg = configuracao_de_teste();
    let (r, _, maior) =
        contando(|| catch_unwind(AssertUnwindSafe(|| serializar(cfg, 1, &entradas))));
    let r = r.unwrap_or_else(|_| panic!("pânico"));
    assert_eq!(r.err(), Some(ErroCorpo::GrandeDemais));
    // Só o vetor de ordem (um ponteiro por entrada); o buffer do corpo não nasce.
    assert!(maior < 1024, "pedido de {maior} bytes");

    // Uma a menos cabe e volta.
    entradas.pop();
    let corpo = serializar(cfg, 1, &entradas).unwrap();
    assert!(corpo.len() <= MAX_CORPO);
    assert_eq!(
        desserializar(&corpo).map(|(_, _, e)| e.len()),
        Ok(entradas.len())
    );
}

// ---- V2.33 ----

/// Diretório de teste em `target/<perfil>/ward-teste-memoria/`, 0700, apagado no fim.
struct DirTemporario(std::path::PathBuf);

impl DirTemporario {
    fn novo(nome: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let exe = std::env::current_exe().unwrap();
        let base = exe
            .parent()
            .and_then(std::path::Path::parent)
            .unwrap()
            .join("ward-teste-memoria");
        std::fs::create_dir_all(&base).unwrap();
        let d = base.join(format!("{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir(&d).unwrap();
        std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self(d)
    }
}

impl Drop for DirTemporario {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn v2_33_nenhum_bloco_liberado_tem_segredo() {
    // Controle: o detector vê um `Vec` comum com o corpo (a mutação de trocar o
    // `Zeroizing` por um `Vec`).
    // A configuração deste teste, com o hash dela como marca extra (a saída B64, o
    // último campo do texto PHC).
    let cfg = configuracao_nova();
    let corpo_cfg = serializar(&cfg, 1, &[]).unwrap();
    let n = u32::from_le_bytes(corpo_cfg[P_HASH..P_HASH + 4].try_into().unwrap()) as usize;
    let texto = &corpo_cfg[P_HASH + 4..P_HASH + 4 + n];
    let saida = &texto[texto.iter().rposition(|b| *b == b'$').unwrap() + 1..];
    assert_eq!(saida.len(), 43);
    marcar_extra(saida);
    drop(corpo_cfg);

    let corpo = serializar(&cfg, 1, &cofre_de_teste()).unwrap();
    let achados = procurando(|| {
        let comum: Vec<u8> = corpo.to_vec();
        drop(comum);
    });
    // Todas as marcas menos a senha mestra e o PIN (que não estão no corpo), mais o
    // hash do PIN.
    assert!(achados >= SEGREDOS.len(), "o detector não viu o controle");
    drop(corpo);

    // Controle 3: o PIN e o hash dele num `Vec` comum são vistos.
    let achados = procurando(|| {
        drop(PIN_TESTE.as_bytes().to_vec());
        drop(saida_vec(&cfg));
    });
    assert_eq!(achados, 2, "o detector não viu o PIN ou o hash");

    // Controle 2: um segredo que cresce por realocação é visto.
    let achados = procurando(|| {
        let mut s = String::new();
        s.push_str(SEGREDOS[1]);
        s.push_str(&"x".repeat(64));
        drop(Segredo::de_texto(s));
    });
    assert!(achados >= 1, "o detector não viu a realocação");

    // O caminho inteiro: montar as entradas, serializar, desserializar, gravar pelo
    // caminho público (`gravar_entradas`, D44: selar não é público) num diretório
    // temporário, ler, abrir, gravar de novo e soltar tudo.
    let dir = DirTemporario::novo("v2-33");
    let destino = dir.0.join("cofre.ward");
    let achados = procurando(|| {
        let senha = SenhaMestra::nova(SENHA_MESTRA.to_owned());
        let entradas = cofre_de_teste();
        let corpo = serializar(&cfg, 1, &entradas).unwrap();
        let (_, _, lidas) = desserializar(&corpo).unwrap();
        assert_eq!(lidas.len(), 22);
        drop(corpo);
        let mut cofre = CofreAberto::novo(&senha, cfg).unwrap();
        gravar_entradas(&destino, &mut cofre, &entradas).unwrap();
        let arquivo = std::fs::read(&destino).unwrap();
        let (mut reaberto, voltaram) = abrir_entradas(&arquivo, &senha).unwrap();
        assert_eq!(voltaram.len(), 22);
        assert_eq!(reaberto.geracao(), 1);
        // O PIN confere no hash lido do corpo (D30), e um hash novo nasce e morre.
        assert!(reaberto.configuracao().conferir_pin(PIN_TESTE.as_bytes()));
        assert!(!reaberto.configuracao().conferir_pin(b"135791"));
        drop(Configuracao::nova(
            HashPin::criar(PIN_TESTE.as_bytes()).unwrap(),
        ));
        gravar_entradas(&destino, &mut reaberto, &voltaram).unwrap();
        let arquivo2 = std::fs::read(&destino).unwrap();
        drop((entradas, lidas, voltaram, arquivo, arquivo2));
        drop((cofre, reaberto, senha));
    });
    assert_eq!(achados, 0, "segredo em bloco liberado");

    // Corpo recusado no meio da 2ª passada (entrada inválida depois de outras já
    // montadas): o que já foi montado também zera.
    let achados = procurando(|| {
        let mut corpo = serializar(configuracao_de_teste(), 1, &cofre_de_teste())
            .unwrap()
            .to_vec();
        // Estraga o host da última entrada `servidor` com chave (`-` no começo): passa
        // na 1ª passada e cai na validação, depois de outras entradas já montadas.
        let alvo = b"ssh-interno";
        let pos = corpo.windows(alvo.len()).rposition(|j| j == alvo).unwrap();
        corpo[pos] = b'-';
        assert_eq!(desserializar(&corpo).err(), Some(ErroCorpo::Invalido));
        // O corpo em claro deste teste é um `Vec` comum: zera antes de soltar.
        zeroize::Zeroize::zeroize(&mut corpo);
    });
    assert_eq!(achados, 0, "segredo em bloco liberado na recusa");
    marcar_extra(&[]);
}

/// A saída B64 do hash do PIN de `cfg`, num `Vec` comum (só para o controle).
fn saida_vec(cfg: &Configuracao) -> Vec<u8> {
    let corpo = serializar(cfg, 1, &[]).unwrap();
    let n = u32::from_le_bytes(corpo[P_HASH..P_HASH + 4].try_into().unwrap()) as usize;
    let texto = &corpo[P_HASH + 4..P_HASH + 4 + n];
    texto[texto.iter().rposition(|b| *b == b'$').unwrap() + 1..].to_vec()
}

/// S8 no protocolo (Fase 20): o caminho do quadro inteiro (codificar, escrever, ler o
/// quadro, decodificar, validar a entrada que chegou, responder um segredo ao app e
/// ler a resposta) e as recusas (papel errado, anexo acima de D29) não deixam segredo
/// em bloco liberado, nem cru nem no base64 do quadro.
#[test]
fn s8_protocolo_nenhum_bloco_liberado_tem_segredo() {
    use std::io::Cursor;
    use ward_core::protocolo::quadro::{escrever_quadro, ler_quadro, limite_do_quadro};
    use ward_core::protocolo::{self, ErroResposta, Papel, Pedido, Resposta};
    use zeroize::Zeroizing;

    fn s(v: &str) -> Segredo {
        Segredo::de_texto(v.to_owned())
    }
    fn rascunho(anexo: &str) -> Rascunho {
        Rascunho {
            nome: "clientex-stage-wp".to_owned(),
            grupo: "clientex".to_owned(),
            ambiente: "stage".to_owned(),
            projeto: String::new(),
            tags: Vec::new(),
            dados: RascunhoDados::Login {
                url: "https://clientex.invalid/login".to_owned(),
                usuario: "admin".to_owned(),
                senha: Some(s("falso-WP-senha-01")),
                totp_semente: Some(s("JBSWY3DPEHPK3PXP")),
            },
            extras: vec![RascunhoExtra {
                nome: "PIN_BANCO".to_owned(),
                valor: s("falso-nota-01"),
                segredo: true,
            }],
            anexos: vec![RascunhoAnexo {
                nome: "chave.pem".to_owned(),
                conteudo: s(anexo),
            }],
        }
    }
    /// Escreve o quadro num buffer que zera e lê de volta pelo leitor de quadros.
    fn pelo_socket(q: &[u8], limite: usize) -> Zeroizing<Vec<u8>> {
        let mut buf = Zeroizing::new(vec![0u8; q.len() + 4]);
        escrever_quadro(&mut Cursor::new(&mut buf[..]), q, limite).unwrap();
        ler_quadro(&mut &buf[..], limite).unwrap()
    }

    // A marca extra é o base64 da senha mestra, como vai no quadro.
    let destravar = Pedido::Destravar {
        senha: s(SENHA_MESTRA),
    };
    let q = protocolo::codificar_pedido(Papel::Agente, &destravar).unwrap();
    let texto = core::str::from_utf8(&q).unwrap();
    let ini = texto.find(r#""senha":""#).unwrap() + 9;
    let fim = ini + texto[ini..].find('"').unwrap();
    let b64 = texto.as_bytes()[ini..fim].to_vec();
    assert!(b64.len() > 20);
    marcar_extra(&b64);
    // Controle: o detector vê o quadro num `Vec` comum.
    let achados = procurando(|| drop(q.to_vec()));
    assert!(achados >= 1, "o detector não viu o quadro");
    drop((q, destravar));

    let achados = procurando(|| {
        let limite_comum = limite_do_quadro(Papel::Agente);
        let limite_app = limite_do_quadro(Papel::App);
        // Destravar pela CLI.
        let p = Pedido::Destravar {
            senha: s(SENHA_MESTRA),
        };
        let q = protocolo::codificar_pedido(Papel::Agente, &p).unwrap();
        let lido = pelo_socket(&q, limite_comum);
        let Ok(Pedido::Destravar { senha }) = protocolo::decodificar_pedido(Papel::Agente, &lido)
        else {
            panic!("destravar não voltou")
        };
        assert_eq!(senha.expor(), SENHA_MESTRA.as_bytes());
        drop((p, q, lido, senha));

        // Salvar pelo app, validar o que chegou e responder um segredo ao app.
        let p = Pedido::SalvarEntrada {
            anterior: None,
            rascunho: rascunho("falso-anexo-01"),
        };
        let q = protocolo::codificar_pedido(Papel::App, &p).unwrap();
        let lido = pelo_socket(&q, limite_app);
        let Ok(Pedido::SalvarEntrada { rascunho: r, .. }) =
            protocolo::decodificar_pedido(Papel::App, &lido)
        else {
            panic!("salvar não voltou")
        };
        let entrada = r.validar().unwrap();
        let resposta = Resposta::Segredo(s("falso-REDIR-01"));
        let qr = protocolo::codificar_resposta(Papel::App, &resposta).unwrap();
        let lida = protocolo::decodificar_resposta(Papel::App, &qr).unwrap();
        // O agente não lê esse quadro (S1).
        assert!(protocolo::decodificar_resposta(Papel::Agente, &qr).is_err());
        drop((p, q, lido, entrada, resposta, qr, lida));

        // Recusas: a mesma operação pelo agente e um anexo acima da soma de D29.
        let p = Pedido::SalvarEntrada {
            anterior: None,
            rascunho: rascunho("falso-anexo-01"),
        };
        let q = protocolo::codificar_pedido(Papel::App, &p).unwrap();
        assert_eq!(
            protocolo::decodificar_pedido(Papel::Agente, &q).unwrap_err(),
            ErroResposta::NaoAutorizado
        );
        let mut grande = Zeroizing::new(String::with_capacity(MAX_ANEXOS_SOMADOS + 1));
        grande.push_str("falso-anexo-01");
        while grande.len() <= MAX_ANEXOS_SOMADOS {
            grande.push('x');
        }
        let acima = Pedido::SalvarEntrada {
            anterior: None,
            rascunho: rascunho(&grande),
        };
        assert!(protocolo::codificar_pedido(Papel::App, &acima).is_err());
        drop((p, q, grande, acima));
    });
    assert_eq!(achados, 0, "segredo do protocolo em bloco liberado");
    marcar_extra(&[]);
}
