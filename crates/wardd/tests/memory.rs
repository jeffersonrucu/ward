//! V3.8 and V3.13 (S8, S9) under a global test allocator: after unlocking, using and
//! locking, no block freed by the daemon holds the vault's data key, the master
//! password, the PIN nor the secrets of the entries. The key is taken from the test
//! file itself (Argon2id + XChaCha20-Poly1305, the same audited libraries as the core),
//! so the search is for the exact value that `OpenVault` kept.
//!
//! A test binary of its own because of the `#[global_allocator]`. `unsafe` only here,
//! in the test allocator (D31: the exception accepted for integration tests); no daemon
//! code runs with it outside this binary.

#![allow(unsafe_code)]

mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::{Cell, RefCell};

use chacha20poly1305::aead::AeadInOut;
use chacha20poly1305::{Key, KeyInit, Tag, XChaCha20Poly1305, XNonce};
use common::*;
use ward_core::protocol::Role;

const MAX_MARKS: usize = 8;
const MARK_SIZE: usize = 64;

thread_local! {
    static SEARCHING: Cell<bool> = const { Cell::new(false) };
    static FINDINGS: Cell<usize> = const { Cell::new(0) };
    // The marks live only here and are read by reference: copying them to the stack
    // would leave remains of them on the stack, and those remains can end up in a heap
    // block (padding bytes of a moved `enum`), giving a finding that is not the
    // daemon's.
    static MARKS: RefCell<[([u8; MARK_SIZE], usize); MAX_MARKS]> =
        const { RefCell::new([([0; MARK_SIZE], 0); MAX_MARKS]) };
}

fn mark(i: usize, m: &[u8]) {
    MARKS.with(|c| {
        let mut t = c.borrow_mut();
        t[i].0[..m.len()].copy_from_slice(m);
        t[i].1 = m.len();
    });
}

/// # Safety
/// `ptr` points to a live block of `size` bytes, all initialized.
unsafe fn search(ptr: *const u8, size: usize) {
    if !SEARCHING.try_with(Cell::get).unwrap_or(false) || ptr.is_null() {
        return;
    }
    let _ = MARKS.try_with(|c| {
        let Ok(marks) = c.try_borrow() else {
            return;
        };
        for i in 0..size {
            // SAFETY: `i < size`, inside the block (the function's contract); initialized
            // bytes (the block was born from `alloc_zeroed`). Volatile read so it is not
            // optimized away from a block about to be freed.
            let b = unsafe { core::ptr::read_volatile(ptr.add(i)) };
            for (m, n) in marks.iter() {
                if *n == 0 || m[0] != b || i + n > size {
                    continue;
                }
                // SAFETY: `i + k < i + n <= size`, inside the block.
                if (0..*n).all(|k| unsafe { core::ptr::read_volatile(ptr.add(i + k)) } == m[k]) {
                    let _ = FINDINGS.try_with(|a| a.set(a.get() + 1));
                }
            }
        }
    });
}

struct Observer;

// SAFETY: everything goes to `System` with the same `Layout`; the observer only reads the
// block before giving it back and only touches `Cell`s of its own thread, without
// allocating. Every block is born zeroed (`alloc` → `alloc_zeroed`), so the search only
// reads initialized bytes; the default `realloc` goes through `alloc` and `dealloc` here.
unsafe impl GlobalAlloc for Observer {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: same contract as `alloc`, passed on as `alloc_zeroed`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: same contract, passed on.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: live block of this allocator with `layout.size()` initialized bytes.
        unsafe { search(ptr, layout.size()) };
        // SAFETY: same contract as `dealloc`, passed on.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Observer = Observer;

fn searching(f: impl FnOnce()) -> usize {
    FINDINGS.with(|a| a.set(0));
    SEARCHING.with(|p| p.set(true));
    f();
    SEARCHING.with(|p| p.set(false));
    FINDINGS.with(Cell::get)
}

/// The data key of the vault file (Phase 13 format): KEK = Argon2id(password, salt,
/// header parameters); the key goes wrapped with XChaCha20-Poly1305 and the AAD is
/// bytes 0..80.
fn data_key(file: &[u8], password: &str) -> Box<[u8; 32]> {
    let u32_at = |p: usize| u32::from_le_bytes(file[p..p + 4].try_into().unwrap());
    let params = argon2::Params::new(u32_at(12), u32_at(16), u32_at(20), Some(32)).unwrap();
    let a = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let mut kek = Box::new([0u8; 32]);
    a.hash_password_into(password.as_bytes(), &file[24..56], &mut kek[..])
        .unwrap();
    let cipher = XChaCha20Poly1305::new(<&Key>::try_from(&kek[..]).unwrap());
    let nonce = <&XNonce>::try_from(&file[56..80]).unwrap();
    let mut key = Box::new([0u8; 32]);
    key.copy_from_slice(&file[80..112]);
    let tag = <&Tag>::try_from(&file[112..128]).unwrap();
    cipher
        .decrypt_inout_detached(nonce, &file[..80], (&mut key[..]).into(), tag)
        .unwrap();
    key
}

#[test]
fn v3_8_lock_zeroes_the_key_and_the_secrets_before_freeing() {
    let dir = Dir::new("memory");
    let (d, _) = test_vault(&dir);
    d.lock();
    let file = std::fs::read(dir.paths().vault).unwrap();
    let key = data_key(&file, PASSWORD);
    drop(file);
    assert_ne!(*key, [0u8; 32]);
    mark(0, &key[..]);
    mark(1, PASSWORD.as_bytes());
    mark(2, PIN.as_bytes());
    for (i, s) in SECRETS.iter().enumerate() {
        mark(3 + i, s.as_bytes());
    }

    let findings = searching(|| {
        assert!(ok(&unlock(&d, PASSWORD)));
        assert!(!d.is_locked());
        let _ = request(&d, Role::Agent, "list", "{}");
        let _ = request(&d, Role::App, "detail", r#"{"entry":"t-aprov"}"#);
        assert!(ok(&set_level(&d, Role::App, "t-aprov", "free", PIN)));
        assert!(ok(&request(
            &d,
            Role::App,
            "save_entry",
            &login("t-memoria", SECRETS[3])
        )));
        let _ = set_level(&d, Role::App, "t-aprov", "free", WRONG_PIN);
        // V3.8: locking releases the open vault and the entries.
        d.lock();
        assert!(d.is_locked());
        assert_eq!(
            error(&request(&d, Role::App, "list", "{}")),
            Some(ward_core::protocol::ResponseError::Locked)
        );
    });
    assert_eq!(
        findings, 0,
        "chave, senha, PIN ou segredo num bloco liberado"
    );

    // Control: the detector sees the key in a block freed without zeroing.
    let control = searching(|| drop(std::hint::black_box(key.to_vec())));
    assert_eq!(control, 1);
}
