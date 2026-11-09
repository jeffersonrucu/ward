//! Memória travada para segredos (D14, S8).
//!
//! [`BufferTravado`] é uma região própria de páginas inteiras, fora do heap: `mlock`
//! (não vai para a swap), `MADV_DONTDUMP` (fora de qualquer core), `MADV_WIPEONFORK`
//! (um filho criado por `fork` a recebe zerada) e zerada com `zeroize` antes de
//! devolver ao kernel. Por ter páginas só suas, o `munlock` dela nunca destrava o
//! segredo de outro buffer que dividisse a página.
//!
//! [`travar_regiao`] serve para memória que já existe (por exemplo, a chave de dados
//! do `ward-core`, num `Box` do heap). O `mlock` vale para a página inteira e não é
//! contado por região: destravar uma região destrava a página para todas as que a
//! dividem. Por isso não há função de destravar aqui; as páginas ficam travadas até o
//! processo devolvê-las ao kernel, o que não é risco (só conta no `RLIMIT_MEMLOCK`).

use std::fmt;
use std::io;
use std::ptr::NonNull;

use zeroize::Zeroize;

use crate::sys;

/// Maior buffer travado aceito (1 MiB). O `RLIMIT_MEMLOCK` do usuário costuma ser de
/// 8 MiB; o daemon guarda poucos segredos travados ao mesmo tempo.
pub const MAX_BUFFER_TRAVADO: usize = 1024 * 1024;

/// Buffer de segredo em páginas travadas, zerado ao sair. Não implementa `Clone`, e o
/// `Debug` mostra só o tamanho.
pub struct BufferTravado {
    inicio: NonNull<u8>,
    /// Bytes úteis.
    len: usize,
    /// Bytes mapeados (múltiplo da página, ≥ `len`).
    mapeado: usize,
}

// SAFETY: o `BufferTravado` é o único dono da região (não há cópia do ponteiro fora
// dele), e só dá acesso de escrita por `&mut self`; mandá-lo para outra thread ou
// emprestá-lo `&` entre threads é como um `Box<[u8]>`.
#[allow(unsafe_code)]
unsafe impl Send for BufferTravado {}
// SAFETY: `&BufferTravado` só lê os bytes (sem mutabilidade interior).
#[allow(unsafe_code)]
unsafe impl Sync for BufferTravado {}

impl BufferTravado {
    /// Cria um buffer de `len` bytes zerados, travado na memória. Falha (sem criar
    /// nada) se `len` for 0 ou maior que [`MAX_BUFFER_TRAVADO`], ou se o `mlock` ou o
    /// `madvise` falharem: segredo não fica em buffer que só pareça travado.
    pub fn novo(len: usize) -> io::Result<Self> {
        if len == 0 || len > MAX_BUFFER_TRAVADO {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "tamanho do buffer travado fora do limite",
            ));
        }
        let pagina = sys::tamanho_pagina();
        let mapeado = len
            .div_ceil(pagina)
            .checked_mul(pagina)
            .ok_or(io::ErrorKind::InvalidInput)?;
        let inicio = sys::mapear_anonimo(mapeado)?;
        let buf = BufferTravado {
            inicio,
            len,
            mapeado,
        };
        // A partir daqui, um erro solta `buf`, e o `Drop` desmapeia a região.
        sys::aconselhar(inicio, mapeado, libc::MADV_DONTDUMP)?;
        sys::aconselhar(inicio, mapeado, libc::MADV_WIPEONFORK)?;
        sys::mlock(buf.regiao_inteira())?;
        Ok(buf)
    }

    /// Bytes úteis.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Sempre `false`: não existe buffer travado vazio.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[allow(unsafe_code)]
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: a região tem `mapeado ≥ len` bytes, mapeada e iniciada (páginas
        // anônimas vêm zeradas) até o `Drop`; o empréstimo de `self` impede o `Drop`
        // enquanto a fatia viver.
        unsafe { std::slice::from_raw_parts(self.inicio.as_ptr(), self.len) }
    }

    #[allow(unsafe_code)]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: como em `as_slice`; `&mut self` garante acesso exclusivo.
        unsafe { std::slice::from_raw_parts_mut(self.inicio.as_ptr(), self.len) }
    }

    #[allow(unsafe_code)]
    fn regiao_inteira(&self) -> &[u8] {
        // SAFETY: a região inteira (`mapeado` bytes) está mapeada até o `Drop`.
        unsafe { std::slice::from_raw_parts(self.inicio.as_ptr(), self.mapeado) }
    }

    #[allow(unsafe_code)]
    fn regiao_inteira_mut(&mut self) -> &mut [u8] {
        // SAFETY: como em `regiao_inteira`, com acesso exclusivo por `&mut self`.
        unsafe { std::slice::from_raw_parts_mut(self.inicio.as_ptr(), self.mapeado) }
    }

    /// Zera a região inteira (não só os `len` bytes úteis) com o `zeroize`, que o
    /// compilador não pode eliminar.
    pub(crate) fn zerar(&mut self) {
        self.regiao_inteira_mut().zeroize();
    }
}

impl Drop for BufferTravado {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        self.zerar();
        // Erro no munlock não impede o munmap, que destrava de qualquer jeito.
        let _ = sys::munlock(self.regiao_inteira());
        // SAFETY: `inicio`/`mapeado` são os do `mapear_anonimo` deste buffer, ainda
        // mapeado; depois daqui nenhum método roda (estamos no `Drop`).
        unsafe { sys::desmapear(self.inicio, self.mapeado) };
    }
}

impl fmt::Debug for BufferTravado {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BufferTravado")
            .field("len", &self.len)
            .finish_non_exhaustive()
    }
}

/// Trava (`mlock`) as páginas que contêm `regiao`. Ver o aviso do módulo sobre páginas
/// divididas; para segredo novo, prefira [`BufferTravado`].
pub fn travar_regiao(regiao: &[u8]) -> io::Result<()> {
    sys::mlock(regiao)
}

#[cfg(test)]
mod testes {
    use super::*;

    /// `VmLck` de `/proc/self/status`, em KiB.
    fn vm_lck() -> u64 {
        let status = std::fs::read_to_string("/proc/self/status").unwrap();
        let linha = status.lines().find(|l| l.starts_with("VmLck:")).unwrap();
        linha
            .split_whitespace()
            .nth(1)
            .and_then(|v| v.parse().ok())
            .unwrap()
    }

    #[test]
    fn buffer_travado_le_escreve_e_trava() {
        let mut b = BufferTravado::novo(100).unwrap();
        assert_eq!(b.len(), 100);
        assert!(b.as_slice().iter().all(|&x| x == 0));
        b.as_mut_slice().copy_from_slice(&[0xAB; 100]);
        assert!(b.as_slice().iter().all(|&x| x == 0xAB));
        // A página está travada: o VmLck do processo conta pelo menos uma página.
        assert!(vm_lck() >= (sys::tamanho_pagina() / 1024) as u64);
    }

    #[test]
    fn zerar_apaga_a_regiao_inteira() {
        let mut b = BufferTravado::novo(10).unwrap();
        b.regiao_inteira_mut().fill(0x5A);
        b.zerar();
        assert!(b.regiao_inteira().iter().all(|&x| x == 0));
    }

    #[test]
    fn tamanho_fora_do_limite_recusa() {
        assert!(BufferTravado::novo(0).is_err());
        assert!(BufferTravado::novo(MAX_BUFFER_TRAVADO + 1).is_err());
        assert!(BufferTravado::novo(MAX_BUFFER_TRAVADO).is_ok());
    }

    #[test]
    fn debug_nao_mostra_conteudo() {
        let mut b = BufferTravado::novo(16).unwrap();
        b.as_mut_slice().copy_from_slice(b"falso-segredo-01");
        let d = format!("{b:?}");
        assert!(!d.contains("falso"));
        assert!(!d.contains("102")); // nem como bytes (`f` = 102)
        assert!(d.contains("len: 16"));
    }

    #[test]
    fn regiao_existente_e_travada() {
        let chave = Box::new([7u8; 32]);
        travar_regiao(&chave[..]).unwrap();
        assert!(vm_lck() > 0);
        travar_regiao(&[]).unwrap();
    }
}
