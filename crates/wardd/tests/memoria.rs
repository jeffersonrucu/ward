//! V3.8 e V3.13 (S8, S9) sob um alocador global de teste: depois de destravar, usar e
//! travar, nenhum bloco liberado pelo daemon tem a chave de dados do cofre, a senha
//! mestra, o PIN nem os segredos das entradas. A chave é tirada do próprio arquivo de
//! teste (Argon2id + XChaCha20-Poly1305, as mesmas bibliotecas auditadas do núcleo),
//! então a procura é pelo valor exato que o `OpenVault` guardou.
//!
//! Binário de teste próprio por causa do `#[global_allocator]`. `unsafe` só aqui, no
//! alocador de teste (D31: a exceção aceita para teste de integração); nenhum código do
//! daemon roda com ele fora deste binário.

#![allow(unsafe_code)]

mod comum;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::{Cell, RefCell};

use chacha20poly1305::aead::AeadInOut;
use chacha20poly1305::{Key, KeyInit, Tag, XChaCha20Poly1305, XNonce};
use comum::*;
use ward_core::protocol::Role;

const MAX_MARCAS: usize = 8;
const TAM_MARCA: usize = 64;

thread_local! {
    static PROCURAR: Cell<bool> = const { Cell::new(false) };
    static ACHADOS: Cell<usize> = const { Cell::new(0) };
    // As marcas ficam só aqui e são lidas por referência: copiá-las para a pilha
    // deixaria resto delas na pilha, e esse resto pode ir parar num bloco do heap (bytes
    // de enchimento de um `enum` movido), dando um achado que não é do daemon.
    static MARCAS: RefCell<[([u8; TAM_MARCA], usize); MAX_MARCAS]> =
        const { RefCell::new([([0; TAM_MARCA], 0); MAX_MARCAS]) };
}

fn marcar(i: usize, m: &[u8]) {
    MARCAS.with(|c| {
        let mut t = c.borrow_mut();
        t[i].0[..m.len()].copy_from_slice(m);
        t[i].1 = m.len();
    });
}

/// # Safety
/// `ptr` aponta para um bloco vivo de `tamanho` bytes, todos inicializados.
unsafe fn procurar(ptr: *const u8, tamanho: usize) {
    if !PROCURAR.try_with(Cell::get).unwrap_or(false) || ptr.is_null() {
        return;
    }
    let _ = MARCAS.try_with(|c| {
        let Ok(marcas) = c.try_borrow() else {
            return;
        };
        for i in 0..tamanho {
            // SAFETY: `i < tamanho`, dentro do bloco (contrato da função); bytes
            // inicializados (o bloco nasceu de `alloc_zeroed`). Leitura volátil para não
            // ser tirada de um bloco que vai ser liberado.
            let b = unsafe { core::ptr::read_volatile(ptr.add(i)) };
            for (m, n) in marcas.iter() {
                if *n == 0 || m[0] != b || i + n > tamanho {
                    continue;
                }
                // SAFETY: `i + k < i + n <= tamanho`, dentro do bloco.
                if (0..*n).all(|k| unsafe { core::ptr::read_volatile(ptr.add(i + k)) } == m[k]) {
                    let _ = ACHADOS.try_with(|a| a.set(a.get() + 1));
                }
            }
        }
    });
}

struct Observador;

// SAFETY: tudo vai ao `System` com o mesmo `Layout`; o observador só lê o bloco antes de
// devolvê-lo e só mexe em `Cell`s da própria thread, sem alocar. Todo bloco nasce zerado
// (`alloc` → `alloc_zeroed`), então a procura só lê bytes inicializados; o `realloc`
// padrão passa por `alloc` e `dealloc` daqui.
unsafe impl GlobalAlloc for Observador {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: mesmo contrato de `alloc`, repassado como `alloc_zeroed`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: mesmo contrato, repassado.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: bloco vivo deste alocador com `layout.size()` bytes inicializados.
        unsafe { procurar(ptr, layout.size()) };
        // SAFETY: mesmo contrato de `dealloc`, repassado.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALOCADOR: Observador = Observador;

fn procurando(f: impl FnOnce()) -> usize {
    ACHADOS.with(|a| a.set(0));
    PROCURAR.with(|p| p.set(true));
    f();
    PROCURAR.with(|p| p.set(false));
    ACHADOS.with(Cell::get)
}

/// A chave de dados do arquivo do cofre (formato da Fase 13): KEK = Argon2id(senha,
/// sal, parâmetros do cabeçalho); a chave vai embrulhada com XChaCha20-Poly1305 e o AAD
/// são os bytes 0..80.
fn chave_de_dados(arquivo: &[u8], senha: &str) -> Box<[u8; 32]> {
    let u32_em = |p: usize| u32::from_le_bytes(arquivo[p..p + 4].try_into().unwrap());
    let params = argon2::Params::new(u32_em(12), u32_em(16), u32_em(20), Some(32)).unwrap();
    let a = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let mut kek = Box::new([0u8; 32]);
    a.hash_password_into(senha.as_bytes(), &arquivo[24..56], &mut kek[..])
        .unwrap();
    let cifra = XChaCha20Poly1305::new(<&Key>::try_from(&kek[..]).unwrap());
    let nonce = <&XNonce>::try_from(&arquivo[56..80]).unwrap();
    let mut chave = Box::new([0u8; 32]);
    chave.copy_from_slice(&arquivo[80..112]);
    let tag = <&Tag>::try_from(&arquivo[112..128]).unwrap();
    cifra
        .decrypt_inout_detached(nonce, &arquivo[..80], (&mut chave[..]).into(), tag)
        .unwrap();
    chave
}

#[test]
fn v3_8_travar_zera_a_chave_e_os_segredos_antes_de_liberar() {
    let dir = Dir::novo("memoria");
    let (d, _) = cofre_de_teste(&dir);
    d.travar();
    let arquivo = std::fs::read(dir.caminhos().cofre).unwrap();
    let chave = chave_de_dados(&arquivo, SENHA);
    drop(arquivo);
    assert_ne!(*chave, [0u8; 32]);
    marcar(0, &chave[..]);
    marcar(1, SENHA.as_bytes());
    marcar(2, PIN.as_bytes());
    for (i, s) in SEGREDOS.iter().enumerate() {
        marcar(3 + i, s.as_bytes());
    }

    let achados = procurando(|| {
        assert!(ok(&destravar(&d, SENHA)));
        assert!(!d.travado());
        let _ = pedir(&d, Role::Agent, "lista", "{}");
        let _ = pedir(&d, Role::App, "detalhe", r#"{"entrada":"t-aprov"}"#);
        assert!(ok(&mudar_nivel(&d, Role::App, "t-aprov", "livre", PIN)));
        assert!(ok(&pedir(
            &d,
            Role::App,
            "salvar_entrada",
            &login("t-memoria", SEGREDOS[3])
        )));
        let _ = mudar_nivel(&d, Role::App, "t-aprov", "livre", PIN_ERRADO);
        // V3.8: travar solta o cofre aberto e as entradas.
        d.travar();
        assert!(d.travado());
        assert_eq!(
            erro(&pedir(&d, Role::App, "lista", "{}")),
            Some(ward_core::protocol::ResponseError::Locked)
        );
    });
    assert_eq!(
        achados, 0,
        "chave, senha, PIN ou segredo num bloco liberado"
    );

    // Controle: o detector vê a chave num bloco liberado sem zerar.
    let controle = procurando(|| drop(std::hint::black_box(chave.to_vec())));
    assert_eq!(controle, 1);
}
