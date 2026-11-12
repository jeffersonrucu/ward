//! Vault body under a global test allocator (V2.32, V2.33, S8), with the config section
//! of the v2 body (D30): the test PIN and its hash are marks too.
//!
//! A test binary of its own because of the `#[global_allocator]`. The [`Observer`]
//! allocator forwards everything to `System` and, only on the thread that asked, (a)
//! counts the requests and keeps the largest and (b) searches the marks of the fake
//! secrets in every block that is freed or reallocated. Without allocating anything
//! inside the allocator.
//!
//! `unsafe` only here, in the test allocator (implementing `GlobalAlloc` requires it): no
//! Ward code runs with it outside this test binary. It is the only accepted exception to
//! the rule "`unsafe` only in `ward-platform`" (D31, V2.40).
//!
//! Every block is born zeroed (`alloc` uses `alloc_zeroed`, D31) and the reallocation is
//! the default one of `GlobalAlloc` (new block through this `alloc`, copy and `dealloc`
//! of the old one). So the search never reads a byte that was not written: uninitialized
//! memory, in the Rust model, has no defined value.

#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

use ward_core as wc;
use ward_core::atomic_write::write_entries;
use ward_core::body::{BodyError, deserialize, open_entries, serialize};
use ward_core::config::{PinHash, VaultConfig};
use ward_core::entry::{AccessLevel, AttachmentDraft, DataDraft, Draft, Entry, ExtraDraft, Secret};
use ward_core::format::{MasterPassword, OpenVault};
use ward_core::limits::{
    MAX_ATTACHMENTS_TOTAL, MAX_BODY, MAX_ENTRIES, MAX_EXTRA_VALUE, MAX_EXTRAS, MAX_NOTE,
};

#[path = "../src/body/test_vault.rs"]
mod test_vault;
use test_vault::{
    MASTER_PASSWORD, SECRETS, TEST_PIN, new_config, test_config, test_vault_entries, validated,
};

// ---- The allocator ----

thread_local! {
    static COUNT: Cell<bool> = const { Cell::new(false) };
    static REQUESTS: Cell<usize> = const { Cell::new(0) };
    static LARGEST: Cell<usize> = const { Cell::new(0) };
    static SEARCH: Cell<bool> = const { Cell::new(false) };
    static FOUND: Cell<usize> = const { Cell::new(0) };
    /// Extra mark of this thread (the B64 output of the PIN hash, which has a random salt
    /// and is only known at run time), in a fixed buffer so as not to allocate.
    static EXTRA_MARK: Cell<([u8; 64], usize)> = const { Cell::new(([0; 64], 0)) };
}

/// Also searches `mark` (up to 64 bytes) on this thread.
fn set_extra_mark(mark: &[u8]) {
    let mut buf = [0u8; 64];
    buf[..mark.len()].copy_from_slice(mark);
    EXTRA_MARK.with(|m| m.set((buf, mark.len())));
}

/// First bytes of the marks, so the search goes over each block only once.
const FIRST_BYTES: [bool; 256] = {
    let mut t = [false; 256];
    let mut i = 0;
    while i < SECRETS.len() {
        t[SECRETS[i].as_bytes()[0] as usize] = true;
        i += 1;
    }
    t[TEST_PIN.as_bytes()[0] as usize] = true;
    t
};

/// Whether the mark starts at `i` in the block.
///
/// # Safety
/// `ptr` points to a live block of `size` bytes, all initialized.
unsafe fn matches_at(ptr: *const u8, size: usize, i: usize, m: &[u8]) -> bool {
    if m.is_empty() || i + m.len() > size {
        return false;
    }
    for (k, &expected) in m.iter().enumerate() {
        // SAFETY: `i + k < i + m.len() <= size`, inside the block (contract of the function).
        if unsafe { core::ptr::read_volatile(ptr.add(i + k)) } != expected {
            return false;
        }
    }
    true
}

fn is_on(key: &'static std::thread::LocalKey<Cell<bool>>) -> bool {
    key.try_with(Cell::get).unwrap_or(false)
}

fn count(size: usize) {
    if is_on(&COUNT) {
        let _ = REQUESTS.try_with(|p| p.set(p.get() + 1));
        let _ = LARGEST.try_with(|m| m.set(m.get().max(size)));
    }
}

/// Searches the marks in the block that is about to be freed or reallocated.
///
/// # Safety
/// `ptr` points to a live block of at least `size` bytes, allocated by this
/// allocator.
unsafe fn search(ptr: *const u8, size: usize) {
    if !is_on(&SEARCH) || ptr.is_null() {
        return;
    }
    let mut i = 0;
    while i < size {
        // SAFETY: `i < size` and the block has `size` bytes (contract of the function), all
        // initialized: the block was born from `alloc_zeroed` (see `Observer`). The read is
        // volatile so the compiler does not drop it from a block that is about to be freed.
        let b = unsafe { core::ptr::read_volatile(ptr.add(i)) };
        if FIRST_BYTES[b as usize] {
            for mark in SECRETS.iter().chain([&TEST_PIN]) {
                let m = mark.as_bytes();
                // SAFETY: the same block, with the contract of this function.
                if m[0] == b && unsafe { matches_at(ptr, size, i, m) } {
                    let _ = FOUND.try_with(|a| a.set(a.get() + 1));
                }
            }
        }
        if let Ok((extra, n)) = EXTRA_MARK.try_with(Cell::get)
            && n > 0
            && extra[0] == b
            // SAFETY: the same block, with the contract of this function.
            && unsafe { matches_at(ptr, size, i, &extra[..n]) }
        {
            let _ = FOUND.try_with(|a| a.set(a.get() + 1));
        }
        i += 1;
    }
}

struct Observer;

// SAFETY: everything is forwarded to `System` with the same `Layout`; the observer only
// reads the block before giving it back and only touches `Cell`s of its own thread,
// without allocating. Every block comes out zeroed (`alloc` → `alloc_zeroed`, D31), so
// the search only reads initialized bytes. `realloc` keeps the default implementation of
// the trait, which asks `alloc` for the new block (zeroed and counted), copies and frees
// the old one through `dealloc` (searched): the old copy left by the reallocation is
// searched too.
unsafe impl GlobalAlloc for Observer {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        // SAFETY: the contract of `GlobalAlloc::alloc` (non-zero size) is the same as
        // `alloc_zeroed`'s, forwarded to `System`. Zeroing the block changes nothing for the
        // caller (uninitialized bytes may hold any value, zero included).
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        // SAFETY: same contract as `GlobalAlloc::alloc_zeroed`, forwarded.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` is a live block of this allocator with `layout.size()` bytes, all
        // initialized (it was born from `alloc_zeroed`).
        unsafe { search(ptr, layout.size()) };
        // SAFETY: same contract as `GlobalAlloc::dealloc`, forwarded.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Observer = Observer;

/// Turns counting on for this thread, runs `f` and returns (result, requests, largest).
fn counting<T>(f: impl FnOnce() -> T) -> (T, usize, usize) {
    REQUESTS.with(|p| p.set(0));
    LARGEST.with(|m| m.set(0));
    COUNT.with(|c| c.set(true));
    let r = f();
    COUNT.with(|c| c.set(false));
    (r, REQUESTS.with(Cell::get), LARGEST.with(Cell::get))
}

/// Turns the search on for this thread, runs `f` and returns how many marks it found in
/// freed or reallocated blocks.
fn searching(f: impl FnOnce()) -> usize {
    FOUND.with(|a| a.set(0));
    SEARCH.with(|p| p.set(true));
    f();
    SEARCH.with(|p| p.set(false));
    FOUND.with(Cell::get)
}

// ---- Bodies forged by hand (the format at the top of `body.rs`) ----

fn field(v: &mut Vec<u8>, b: &[u8]) {
    v.extend_from_slice(&(b.len() as u32).to_le_bytes());
    v.extend_from_slice(b);
}

/// Version (5, D65 d and m2:DT4) and generation (D39) at the start of every body forged
/// here.
fn start(generation: u64) -> Vec<u8> {
    let mut v = vec![5, 0];
    v.extend_from_slice(&generation.to_le_bytes());
    v
}

/// Where the PIN hash field starts: after the version (2) and the generation (8).
const P_HASH: usize = 2 + 8;

/// The empty body of the test vault: version 5 and the config (D30, D39), plus the
/// count.
fn empty_body() -> Vec<u8> {
    serialize(test_config(), 1, &[]).unwrap().to_vec()
}

/// Version and config of the test vault (the empty body without the final count).
fn header() -> Vec<u8> {
    let mut v = empty_body();
    v.truncate(v.len() - 4);
    v
}

/// The PHC text of the PIN hash of the test vault (read from the empty body).
fn test_hash() -> Vec<u8> {
    let v = empty_body();
    let n = u32::from_le_bytes(v[P_HASH..P_HASH + 4].try_into().unwrap()) as usize;
    v[P_HASH + 4..P_HASH + 4 + n].to_vec()
}

/// Version and a config section with the given PIN hash (240 minutes, backup
/// disabled).
fn header_with_hash(hash: &[u8]) -> Vec<u8> {
    let mut v = start(1);
    field(&mut v, hash);
    v.extend_from_slice(&240u32.to_le_bytes());
    v.push(0);
    field(&mut v, b"");
    v
}

/// Start of a body with a generic entry, up to just before the tags.
fn generic_until_tags() -> Vec<u8> {
    let mut v = header();
    v.extend_from_slice(&1u32.to_le_bytes());
    for c in [&b"generic"[..], b"free", b"x", b"g", b"dev", b""] {
        field(&mut v, c);
    }
    v
}

fn with_count(mut v: Vec<u8>, n: u32) -> Vec<u8> {
    v.extend_from_slice(&n.to_le_bytes());
    v
}

/// Valid generic entry up to just before the attachments (0 tags, 1 extra).
fn generic_until_attachments() -> Vec<u8> {
    let mut v = with_count(generic_until_tags(), 0);
    v.extend_from_slice(&1u32.to_le_bytes());
    field(&mut v, b"A");
    v.push(0);
    field(&mut v, b"v");
    v
}

fn forged_bodies() -> Vec<(&'static str, Vec<u8>)> {
    let mut cases = Vec::new();
    // Config section (D30): PIN hash with length u32::MAX, empty, with inflated cost
    // (4 GiB), the PIN in clear, version 1 and the body without the section. Each one
    // with a valid entry after it, so the 2nd pass (which allocates) does not start.
    let entry = with_count(generic_until_attachments(), 0)[header().len()..].to_vec();
    let mut v = start(1);
    v.extend_from_slice(&u32::MAX.to_le_bytes());
    v.extend_from_slice(&entry);
    cases.push(("hash do PIN com u32::MAX", v));
    let inflated = String::from_utf8(test_hash())
        .unwrap()
        .replace("m=65536", "m=4194304");
    let downgraded = String::from_utf8(test_hash())
        .unwrap()
        .replace("t=3", "t=1");
    for (case, hash) in [
        ("sem hash do PIN", &b""[..]),
        ("hash do PIN inflado", inflated.as_bytes()),
        ("hash do PIN rebaixado", downgraded.as_bytes()),
        ("PIN em claro", TEST_PIN.as_bytes()),
    ] {
        let mut v = header_with_hash(hash);
        v.extend_from_slice(&entry);
        cases.push((case, v));
    }
    let mut v = with_count(generic_until_attachments(), 0);
    v[0] = 1;
    cases.push(("versão 1", v));
    // D39: version 2 (without generation), 3 (without dates, D65 d) and 5 with generation 0.
    let mut v = with_count(generic_until_attachments(), 0);
    v[0] = 2;
    cases.push(("versão 2", v));
    let mut v = with_count(generic_until_attachments(), 0);
    v[0] = 3;
    cases.push(("versão 3", v));
    let mut v = with_count(generic_until_attachments(), 0);
    v[2..P_HASH].copy_from_slice(&0u64.to_le_bytes());
    cases.push(("geração 0", v));
    let mut v = start(1);
    v.extend_from_slice(&entry);
    cases.push(("sem a seção", v));
    // Length u32::MAX in the first field and in a middle field.
    let mut v = header();
    v.extend_from_slice(&1u32.to_le_bytes());
    v.extend_from_slice(&u32::MAX.to_le_bytes());
    cases.push(("campo com u32::MAX", v));
    let mut v = header();
    v.extend_from_slice(&1u32.to_le_bytes());
    field(&mut v, b"generic");
    field(&mut v, b"free");
    field(&mut v, b"x");
    v.extend_from_slice(&u32::MAX.to_le_bytes());
    cases.push(("grupo com u32::MAX", v));
    // Attachment declaring 4 GiB (u32::MAX), the limit + 1 and the limit (D29), with only
    // 5 real bytes.
    let limit = MAX_ATTACHMENTS_TOTAL as u32;
    for size in [u32::MAX, limit + 1, limit] {
        let mut v = with_count(generic_until_attachments(), 1);
        field(&mut v, b"a.bin");
        v.extend_from_slice(&size.to_le_bytes());
        v.extend_from_slice(b"pouco");
        cases.push(("anexo de 4 GiB ou acima do que existe", v));
    }
    // Attachments adding up to 256 KiB + 1 (D29), with the real bytes in the buffer: one of
    // half the limit and another of half + 1.
    let mut v = with_count(generic_until_attachments(), 2);
    let half = MAX_ATTACHMENTS_TOTAL / 2;
    field(&mut v, b"a.bin");
    field(&mut v, &vec![0x5a; half]);
    field(&mut v, b"b.bin");
    field(&mut v, &vec![0x5a; half + 1]);
    cases.push(("anexos somam o limite + 1", v));
    // A single real attachment, with the limit + 1.
    let mut v = with_count(generic_until_attachments(), 1);
    field(&mut v, b"a.bin");
    field(&mut v, &vec![0x5a; MAX_ATTACHMENTS_TOTAL + 1]);
    cases.push(("anexo com o limite + 1", v));
    // 2^32 − 1 entries and 10 001.
    let start = header().len() + 4;
    for n in [u32::MAX, (MAX_ENTRIES + 1) as u32, 2] {
        let mut v = header();
        v.extend_from_slice(&n.to_le_bytes());
        v.extend_from_slice(&generic_until_attachments()[start..]);
        v.extend_from_slice(&0u32.to_le_bytes());
        cases.push(("entradas demais", v));
    }
    // 17 tags, 65 extras, 33 attachments (declared, with one item of each).
    let mut v = with_count(generic_until_tags(), 17);
    field(&mut v, b"t");
    cases.push(("17 tags", v));
    let mut v = with_count(with_count(generic_until_tags(), 0), 65);
    field(&mut v, b"A");
    v.push(0);
    field(&mut v, b"v");
    cases.push(("65 extras", v));
    for n in [33u32, u32::MAX] {
        let mut v = with_count(generic_until_attachments(), n);
        field(&mut v, b"a");
        field(&mut v, b"x");
        cases.push(("anexos demais", v));
    }
    cases
}

// ---- V2.32 ----

#[test]
fn v2_32_forged_body_refused_fast_without_allocating() {
    // Control: the well-formed forged body (one generic entry, without dates) is accepted.
    let mut ok = with_count(generic_until_attachments(), 0);
    ok.extend_from_slice(&[0; 16]);
    assert_eq!(deserialize(&ok).map(|(_, _, e)| e.len()), Ok(1));
    // Control of the section: the header rebuilt with the test hash is the module's.
    assert!(header_with_hash(&test_hash()) == header());

    for (case, forged) in forged_bodies() {
        let start = Instant::now();
        let (r, requests, largest) =
            counting(|| catch_unwind(AssertUnwindSafe(|| deserialize(&forged))));
        let elapsed = start.elapsed();
        let r = r.unwrap_or_else(|_| panic!("{case}: pânico"));
        assert_eq!(r.err(), Some(BodyError::Invalid), "{case}");
        assert!(elapsed < Duration::from_millis(100), "{case}: {elapsed:?}");
        // The structure is checked before allocating: no request, not even a small one.
        assert_eq!(requests, 0, "{case}: alocou");
        assert!(largest <= forged.len(), "{case}: pedido de {largest} bytes");
    }

    // Body above 64 MiB.
    let mut large = vec![0u8; MAX_BODY + 1];
    large[0] = 2;
    let start = Instant::now();
    let (r, requests, _) = counting(|| deserialize(&large));
    assert_eq!(r.err(), Some(BodyError::Invalid));
    assert!(start.elapsed() < Duration::from_millis(100));
    assert_eq!(requests, 0);
}

#[test]
fn v2_32_authentic_body_allocates_only_what_exists() {
    // The test vault read for real: the largest request is the body (copy of a field) or
    // the vector of the 22 entries that exist, never something of the declared size.
    let body = serialize(test_config(), 1, &test_vault_entries()).unwrap();
    let (r, requests, largest) = counting(|| deserialize(&body));
    assert_eq!(r.map(|(_, _, e)| e.len()), Ok(22));
    assert!(requests > 0);
    let ceiling = body.len().max(22 * std::mem::size_of::<Entry>());
    assert!(largest <= ceiling, "pedido de {largest} > {ceiling}");
}

/// A secure note at the maximum of the model: 1 MiB text, 64 extras of 64 KiB and one
/// 256 KiB attachment (~5.3 MiB). Without delivery, it stays out of the mask budget
/// (D29), so it is the way to build a large body with valid entries.
fn full_note(name: &str) -> Entry {
    let mut r = Draft {
        name: name.to_owned(),
        group: "g".to_owned(),
        environment: "dev".to_owned(),
        project: String::new(),
        tags: Vec::new(),
        data: DataDraft::SecureNote {
            text: Some(Secret::from_bytes(vec![b'n'; MAX_NOTE])),
        },
        extras: Vec::new(),
        attachments: Vec::new(),
    };
    for i in 0..MAX_EXTRAS {
        r.extras.push(ExtraDraft {
            name: format!("e{i:02}"),
            value: Secret::from_bytes(vec![b'v'; MAX_EXTRA_VALUE]),
            secret: true,
        });
    }
    r.attachments.push(AttachmentDraft {
        name: "a.bin".to_owned(),
        content: Secret::from_bytes(vec![0x5a; MAX_ATTACHMENTS_TOTAL]),
    });
    validated(r, AccessLevel::Free)
}

#[test]
fn v2_32_encoder_refuses_above_limit_before_allocating() {
    // Full notes until going over 64 MiB (with 256 KiB attachments, D29, a single entry
    // does not come close to the body limit).
    let mut entries: Vec<Entry> = Vec::new();
    let mut approx = 0usize;
    while approx <= MAX_BODY {
        entries.push(full_note(&format!("nota-{:03}", entries.len())));
        approx += MAX_NOTE + MAX_EXTRAS * MAX_EXTRA_VALUE + MAX_ATTACHMENTS_TOTAL;
    }
    let cfg = test_config();
    let (r, _, largest) =
        counting(|| catch_unwind(AssertUnwindSafe(|| serialize(cfg, 1, &entries))));
    let r = r.unwrap_or_else(|_| panic!("pânico"));
    assert_eq!(r.err(), Some(BodyError::TooLarge));
    // Only the order vector (one pointer per entry); the body buffer is never born.
    assert!(largest < 1024, "pedido de {largest} bytes");

    // One fewer fits and comes back.
    entries.pop();
    let body = serialize(cfg, 1, &entries).unwrap();
    assert!(body.len() <= MAX_BODY);
    assert_eq!(
        deserialize(&body).map(|(_, _, e)| e.len()),
        Ok(entries.len())
    );
}

// ---- V2.33 ----

/// Test directory in a fixed directory under `target/<profile>/`, 0700, deleted at the end.
struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let exe = std::env::current_exe().unwrap();
        let base = exe
            .parent()
            .and_then(std::path::Path::parent)
            .unwrap()
            .join("ward-test-memory");
        std::fs::create_dir_all(&base).unwrap();
        let d = base.join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir(&d).unwrap();
        std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self(d)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn v2_33_no_freed_block_has_secret() {
    // Control: the detector sees a plain `Vec` with the body (the mutation of swapping the
    // `Zeroizing` for a `Vec`).
    // The config of this test, with its hash as the extra mark (the B64 output, the last
    // field of the PHC text).
    let cfg = new_config();
    let cfg_body = serialize(&cfg, 1, &[]).unwrap();
    let n = u32::from_le_bytes(cfg_body[P_HASH..P_HASH + 4].try_into().unwrap()) as usize;
    let text = &cfg_body[P_HASH + 4..P_HASH + 4 + n];
    let output = &text[text.iter().rposition(|b| *b == b'$').unwrap() + 1..];
    assert_eq!(output.len(), 43);
    set_extra_mark(output);
    drop(cfg_body);

    let body = serialize(&cfg, 1, &test_vault_entries()).unwrap();
    let found = searching(|| {
        let plain: Vec<u8> = body.to_vec();
        drop(plain);
    });
    // All the marks but the master password and the PIN (which are not in the body), plus
    // the PIN hash.
    assert!(found >= SECRETS.len(), "o detector não viu o controle");
    drop(body);

    // Control 3: the PIN and its hash in a plain `Vec` are seen.
    let found = searching(|| {
        drop(TEST_PIN.as_bytes().to_vec());
        drop(output_vec(&cfg));
    });
    assert_eq!(found, 2, "o detector não viu o PIN ou o hash");

    // Control 2: a secret that grows by reallocation is seen.
    let found = searching(|| {
        let mut s = String::new();
        s.push_str(SECRETS[1]);
        s.push_str(&"x".repeat(64));
        drop(Secret::from_text(s));
    });
    assert!(found >= 1, "o detector não viu a realocação");

    // The whole path: build the entries, serialize, deserialize, write through the public
    // path (`write_entries`, D44: sealing is not public) to a temporary directory, read,
    // open, write again and drop everything.
    let dir = TempDir::new("v2-33");
    let destination = dir.0.join("cofre.ward");
    let found = searching(|| {
        let password = MasterPassword::new(MASTER_PASSWORD.to_owned());
        let entries = test_vault_entries();
        let body = serialize(&cfg, 1, &entries).unwrap();
        let (_, _, read_back) = deserialize(&body).unwrap();
        assert_eq!(read_back.len(), 22);
        drop(body);
        let mut vault = OpenVault::new(&password, cfg).unwrap();
        write_entries(&destination, &mut vault, &entries).unwrap();
        let file = std::fs::read(&destination).unwrap();
        let (mut reopened, returned) = open_entries(&file, &password).unwrap();
        assert_eq!(returned.len(), 22);
        assert_eq!(reopened.generation(), 1);
        // The PIN verifies against the hash read from the body (D30); a new hash lives and dies.
        assert!(reopened.config().verify_pin(TEST_PIN.as_bytes()));
        assert!(!reopened.config().verify_pin(b"135791"));
        drop(VaultConfig::new(
            PinHash::create(TEST_PIN.as_bytes()).unwrap(),
        ));
        write_entries(&destination, &mut reopened, &returned).unwrap();
        let file2 = std::fs::read(&destination).unwrap();
        drop((entries, read_back, returned, file, file2));
        drop((vault, reopened, password));
    });
    assert_eq!(found, 0, "segredo em bloco liberado");

    // Body refused in the middle of the 2nd pass (invalid entry after others already
    // built): what was already built is zeroed too.
    let found = searching(|| {
        let mut body = serialize(test_config(), 1, &test_vault_entries())
            .unwrap()
            .to_vec();
        // Breaks the host of the last server entry with a key (`-` at the start): it passes
        // the 1st pass and falls in the validation, after other entries already built.
        let target = b"ssh-interno";
        let pos = body
            .windows(target.len())
            .rposition(|j| j == target)
            .unwrap();
        body[pos] = b'-';
        assert_eq!(deserialize(&body).err(), Some(BodyError::Invalid));
        // The clear body of this test is a plain `Vec`: zero it before dropping.
        zeroize::Zeroize::zeroize(&mut body);
    });
    assert_eq!(found, 0, "segredo em bloco liberado na recusa");
    set_extra_mark(&[]);
}

/// The B64 output of the PIN hash of `cfg`, in a plain `Vec` (only for the control).
fn output_vec(cfg: &VaultConfig) -> Vec<u8> {
    let body = serialize(cfg, 1, &[]).unwrap();
    let n = u32::from_le_bytes(body[P_HASH..P_HASH + 4].try_into().unwrap()) as usize;
    let text = &body[P_HASH + 4..P_HASH + 4 + n];
    text[text.iter().rposition(|b| *b == b'$').unwrap() + 1..].to_vec()
}

/// S8 in the protocol (Phase 20): the path of the whole frame (encode, write, read the
/// frame, decode, validate the entry that arrived, answer a secret to the app and read
/// the response) and the refusals (wrong role, attachment above D29) leave no secret in
/// a freed block, neither raw nor in the base64 of the frame.
#[test]
fn s8_protocol_no_freed_block_has_secret() {
    use std::io::Cursor;
    use ward_core::protocol::frame::{frame_limit, read_frame, write_frame};
    use ward_core::protocol::{self, Request, Response, ResponseError, Role};
    use zeroize::Zeroizing;

    fn s(v: &str) -> Secret {
        Secret::from_text(v.to_owned())
    }
    fn draft(attachment: &str) -> Draft {
        Draft {
            name: "clientex-stage-wp".to_owned(),
            group: "clientex".to_owned(),
            environment: "stage".to_owned(),
            project: String::new(),
            tags: Vec::new(),
            data: DataDraft::Login {
                url: "https://clientex.invalid/login".to_owned(),
                username: "admin".to_owned(),
                password: Some(s("falso-WP-senha-01")),
                totp_seed: Some(s("JBSWY3DPEHPK3PXP")),
            },
            extras: vec![ExtraDraft {
                name: "PIN_BANCO".to_owned(),
                value: s("falso-nota-01"),
                secret: true,
            }],
            attachments: vec![AttachmentDraft {
                name: "chave.pem".to_owned(),
                content: s(attachment),
            }],
        }
    }
    /// Writes the frame into a buffer that zeroes and reads it back with the frame reader.
    fn through_socket(q: &[u8], limit: usize) -> Zeroizing<Vec<u8>> {
        let mut buf = Zeroizing::new(vec![0u8; q.len() + 4]);
        write_frame(&mut Cursor::new(&mut buf[..]), q, limit).unwrap();
        read_frame(&mut &buf[..], limit).unwrap()
    }

    // The extra mark is the base64 of the master password, as it goes in the frame.
    let unlock = Request::Unlock {
        password: s(MASTER_PASSWORD),
    };
    let q = protocol::encode_request(Role::Agent, &unlock).unwrap();
    let text = core::str::from_utf8(&q).unwrap();
    let key = r#""password":""#;
    let begin = text.find(key).unwrap() + key.len();
    let end = begin + text[begin..].find('"').unwrap();
    let b64 = text.as_bytes()[begin..end].to_vec();
    assert!(b64.len() > 20);
    set_extra_mark(&b64);
    // Control: the detector sees the frame in a plain `Vec`.
    let found = searching(|| drop(q.to_vec()));
    assert!(found >= 1, "o detector não viu o quadro");
    drop((q, unlock));

    let found = searching(|| {
        let common_limit = frame_limit(Role::Agent);
        let app_limit = frame_limit(Role::App);
        // Unlock through the CLI.
        let p = Request::Unlock {
            password: s(MASTER_PASSWORD),
        };
        let q = protocol::encode_request(Role::Agent, &p).unwrap();
        let received = through_socket(&q, common_limit);
        let Ok(Request::Unlock { password }) = protocol::decode_request(Role::Agent, &received)
        else {
            panic!("destravar não voltou")
        };
        assert_eq!(password.expose(), MASTER_PASSWORD.as_bytes());
        drop((p, q, received, password));

        // Save through the app, validate what arrived and answer a secret to the app.
        let p = Request::SaveEntry {
            previous: None,
            draft: draft("falso-anexo-01"),
            keep_attachments: Vec::new(),
        };
        let q = protocol::encode_request(Role::App, &p).unwrap();
        let received = through_socket(&q, app_limit);
        let Ok(Request::SaveEntry { draft: r, .. }) =
            protocol::decode_request(Role::App, &received)
        else {
            panic!("salvar não voltou")
        };
        let entry = r.validate().unwrap();
        let response = Response::Secret(s("falso-REDIR-01"));
        let qr = protocol::encode_response(Role::App, &response).unwrap();
        let decoded = protocol::decode_response(Role::App, &qr).unwrap();
        // The agent does not read this frame (S1).
        assert!(protocol::decode_response(Role::Agent, &qr).is_err());
        drop((p, q, received, entry, response, qr, decoded));

        // Refusals: the same operation by the agent and an attachment above the D29 total.
        let p = Request::SaveEntry {
            previous: None,
            draft: draft("falso-anexo-01"),
            keep_attachments: Vec::new(),
        };
        let q = protocol::encode_request(Role::App, &p).unwrap();
        assert_eq!(
            protocol::decode_request(Role::Agent, &q).unwrap_err(),
            ResponseError::Unauthorized
        );
        let mut large = Zeroizing::new(String::with_capacity(MAX_ATTACHMENTS_TOTAL + 1));
        large.push_str("falso-anexo-01");
        while large.len() <= MAX_ATTACHMENTS_TOTAL {
            large.push('x');
        }
        let above = Request::SaveEntry {
            previous: None,
            draft: draft(&large),
            keep_attachments: Vec::new(),
        };
        assert!(protocol::encode_request(Role::App, &above).is_err());
        drop((p, q, large, above));
    });
    assert_eq!(found, 0, "segredo do protocolo em bloco liberado");
    set_extra_mark(&[]);
}
