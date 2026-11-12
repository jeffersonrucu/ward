//! Locked memory for secrets (D14, S8).
//!
//! [`LockedBuffer`] is a region of whole pages of its own, outside the heap: `mlock` (it
//! does not go to swap), `MADV_DONTDUMP` (out of any core), `MADV_WIPEONFORK` (a child
//! created by `fork` gets it zeroed) and zeroed with `zeroize` before it goes back to the
//! kernel. Since its pages are its own, its `munlock` never unlocks the secret of another
//! buffer that shared the page.
//!
//! [`lock_region`] is for memory that already exists (for example, the data key of
//! `ward-core`, in a heap `Box`). `mlock` applies to the whole page and is not counted
//! per region: unlocking one region unlocks the page for every region that shares it.
//! That is why there is no unlock function here; the pages stay locked until the
//! process gives them back to the kernel, which is no risk (it only counts toward
//! `RLIMIT_MEMLOCK`).

use std::fmt;
use std::io;
use std::ptr::NonNull;

use zeroize::Zeroize;

use crate::sys;

/// Largest locked buffer accepted (1 MiB). The user's `RLIMIT_MEMLOCK` is usually 8 MiB;
/// the daemon keeps few locked secrets at the same time.
pub const MAX_LOCKED_BUFFER: usize = 1024 * 1024;

/// Secret buffer in locked pages, zeroed on drop. It does not implement `Clone`, and
/// `Debug` shows only the length.
pub struct LockedBuffer {
    start: NonNull<u8>,
    /// Useful bytes.
    len: usize,
    /// Mapped bytes (a multiple of the page, ≥ `len`).
    mapped: usize,
}

// SAFETY: the `LockedBuffer` is the only owner of the region (there is no copy of the
// pointer outside it), and it only gives write access through `&mut self`; sending it to
// another thread or lending it `&` across threads is like a `Box<[u8]>`.
#[allow(unsafe_code)]
unsafe impl Send for LockedBuffer {}
// SAFETY: `&LockedBuffer` only reads the bytes (no interior mutability).
#[allow(unsafe_code)]
unsafe impl Sync for LockedBuffer {}

impl LockedBuffer {
    /// Creates a buffer of `len` zeroed bytes, locked in memory. Fails (creating
    /// nothing) if `len` is 0 or larger than [`MAX_LOCKED_BUFFER`], or if `mlock` or
    /// `madvise` fail: a secret never stays in a buffer that only looks locked.
    pub fn new(len: usize) -> io::Result<Self> {
        if len == 0 || len > MAX_LOCKED_BUFFER {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "tamanho do buffer travado fora do limite",
            ));
        }
        let page = sys::page_size();
        let mapped = len
            .div_ceil(page)
            .checked_mul(page)
            .ok_or(io::ErrorKind::InvalidInput)?;
        let start = sys::map_anonymous(mapped)?;
        let buf = LockedBuffer { start, len, mapped };
        // From here on, an error drops `buf`, and `Drop` unmaps the region.
        sys::advise(start, mapped, libc::MADV_DONTDUMP)?;
        sys::advise(start, mapped, libc::MADV_WIPEONFORK)?;
        sys::mlock(buf.whole_region())?;
        Ok(buf)
    }

    /// Useful bytes.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Always `false`: there is no empty locked buffer.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[allow(unsafe_code)]
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: the region has `mapped ≥ len` bytes, mapped and initialized (anonymous
        // pages come zeroed) until `Drop`; the borrow of `self` prevents `Drop` while
        // the slice lives.
        unsafe { std::slice::from_raw_parts(self.start.as_ptr(), self.len) }
    }

    #[allow(unsafe_code)]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: as in `as_slice`; `&mut self` guarantees exclusive access.
        unsafe { std::slice::from_raw_parts_mut(self.start.as_ptr(), self.len) }
    }

    #[allow(unsafe_code)]
    fn whole_region(&self) -> &[u8] {
        // SAFETY: the whole region (`mapped` bytes) is mapped until `Drop`.
        unsafe { std::slice::from_raw_parts(self.start.as_ptr(), self.mapped) }
    }

    #[allow(unsafe_code)]
    fn whole_region_mut(&mut self) -> &mut [u8] {
        // SAFETY: as in `whole_region`, with exclusive access through `&mut self`.
        unsafe { std::slice::from_raw_parts_mut(self.start.as_ptr(), self.mapped) }
    }

    /// Zeroes the whole region (not only the `len` useful bytes) with `zeroize`, which
    /// the compiler cannot elide.
    pub(crate) fn zero(&mut self) {
        self.whole_region_mut().zeroize();
    }
}

impl Drop for LockedBuffer {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        self.zero();
        // An error in munlock does not prevent munmap, which unlocks anyway.
        let _ = sys::munlock(self.whole_region());
        // SAFETY: `start`/`mapped` are the ones from this buffer's `map_anonymous`,
        // still mapped; after this no method runs (we are in `Drop`).
        unsafe { sys::unmap(self.start, self.mapped) };
    }
}

impl fmt::Debug for LockedBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LockedBuffer")
            .field("len", &self.len)
            .finish_non_exhaustive()
    }
}

/// Locks (`mlock`) the pages that contain `region`. See the module note on shared
/// pages; for a new secret, prefer [`LockedBuffer`].
pub fn lock_region(region: &[u8]) -> io::Result<()> {
    sys::mlock(region)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `VmLck` of `/proc/self/status`, in KiB.
    fn vm_lck() -> u64 {
        let status = std::fs::read_to_string("/proc/self/status").unwrap();
        let line = status.lines().find(|l| l.starts_with("VmLck:")).unwrap();
        line.split_whitespace()
            .nth(1)
            .and_then(|v| v.parse().ok())
            .unwrap()
    }

    #[test]
    fn locked_buffer_reads_writes_and_locks() {
        let mut b = LockedBuffer::new(100).unwrap();
        assert_eq!(b.len(), 100);
        assert!(b.as_slice().iter().all(|&x| x == 0));
        b.as_mut_slice().copy_from_slice(&[0xAB; 100]);
        assert!(b.as_slice().iter().all(|&x| x == 0xAB));
        // The page is locked: the process VmLck counts at least one page.
        assert!(vm_lck() >= (sys::page_size() / 1024) as u64);
    }

    #[test]
    fn zero_wipes_the_whole_region() {
        let mut b = LockedBuffer::new(10).unwrap();
        b.whole_region_mut().fill(0x5A);
        b.zero();
        assert!(b.whole_region().iter().all(|&x| x == 0));
    }

    #[test]
    fn size_out_of_limit_refuses() {
        assert!(LockedBuffer::new(0).is_err());
        assert!(LockedBuffer::new(MAX_LOCKED_BUFFER + 1).is_err());
        assert!(LockedBuffer::new(MAX_LOCKED_BUFFER).is_ok());
    }

    #[test]
    fn debug_does_not_show_content() {
        let mut b = LockedBuffer::new(16).unwrap();
        b.as_mut_slice().copy_from_slice(b"falso-segredo-01");
        let d = format!("{b:?}");
        assert!(!d.contains("falso"));
        assert!(!d.contains("102")); // not even as bytes (`f` = 102)
        assert!(d.contains("len: 16"));
    }

    #[test]
    fn existing_region_is_locked() {
        let key = Box::new([7u8; 32]);
        lock_region(&key[..]).unwrap();
        assert!(vm_lck() > 0);
        lock_region(&[]).unwrap();
    }
}
