//! Chamadas diretas ao kernel pela `libc`. Com `memoria`, é o único lugar do Ward com
//! `unsafe`: cada bloco leva `// SAFETY:` e cada função é coberta pelos testes do crate. O resto do
//! crate só usa estas funções, que já devolvem tipos seguros (`OwnedFd`, `io::Result`).

use std::ffi::CStr;
use std::io;
use std::mem::size_of;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::ptr::NonNull;

/// `SO_PEERPIDFD` (Linux 6.5): pidfd do processo do outro lado do socket. O valor é o
/// do `asm-generic/socket.h` (x86_64 e aarch64, os alvos do Ward); a `libc` 0.2.191
/// não o exporta para glibc.
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
const SO_PEERPIDFD: libc::c_int = 77;

fn erro_os() -> io::Error {
    io::Error::last_os_error()
}

/// `SO_PEERCRED` do socket: pid, uid e gid do processo do outro lado no momento do
/// `connect` (ou do `listen`, visto pelo cliente).
#[allow(unsafe_code)]
pub(crate) fn peercred(fd: BorrowedFd<'_>) -> io::Result<libc::ucred> {
    let mut cred = libc::ucred {
        pid: 0,
        uid: u32::MAX,
        gid: u32::MAX,
    };
    let tamanho = size_of::<libc::ucred>();
    let mut len = libc::socklen_t::try_from(tamanho).map_err(|_| io::ErrorKind::InvalidInput)?;
    // SAFETY: `fd` é um descritor aberto (o `BorrowedFd` garante enquanto durar o
    // empréstimo); `cred` e `len` são locais, `len` tem exatamente o tamanho de
    // `ucred`, e o kernel escreve no máximo `len` bytes em `cred`.
    let r = unsafe {
        libc::getsockopt(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&raw mut cred).cast(),
            &raw mut len,
        )
    };
    if r != 0 {
        return Err(erro_os());
    }
    if usize::try_from(len).ok() != Some(tamanho) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(cred)
}

/// `SO_PEERPIDFD`: um pidfd novo, do processo que fez o `connect`. Ao contrário do pid,
/// o pidfd não passa a apontar para outro processo se o número for reaproveitado.
#[allow(unsafe_code)]
pub(crate) fn peer_pidfd(fd: BorrowedFd<'_>) -> io::Result<OwnedFd> {
    let mut pidfd: libc::c_int = -1;
    let tamanho = size_of::<libc::c_int>();
    let mut len = libc::socklen_t::try_from(tamanho).map_err(|_| io::ErrorKind::InvalidInput)?;
    // SAFETY: `fd` está aberto pelo empréstimo; `pidfd` e `len` são locais e `len` tem
    // o tamanho de um `c_int`, que é o que o kernel escreve para `SO_PEERPIDFD`.
    let r = unsafe {
        libc::getsockopt(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            SO_PEERPIDFD,
            (&raw mut pidfd).cast(),
            &raw mut len,
        )
    };
    if r != 0 {
        return Err(erro_os());
    }
    if usize::try_from(len).ok() != Some(tamanho) || pidfd < 0 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    // SAFETY: o `getsockopt` acabou de instalar `pidfd` na tabela deste processo, e
    // nenhum outro código o conhece; o `OwnedFd` passa a ser o único dono e o fecha.
    Ok(unsafe { OwnedFd::from_raw_fd(pidfd) })
}

/// `pidfd_open(pid, 0)`.
#[allow(unsafe_code)]
pub(crate) fn pidfd_open(pid: libc::pid_t) -> io::Result<OwnedFd> {
    if pid <= 0 {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    // SAFETY: syscall sem ponteiros: só um inteiro (pid) e as flags 0.
    let r = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0 as libc::c_uint) };
    if r < 0 {
        return Err(erro_os());
    }
    let fd = libc::c_int::try_from(r).map_err(|_| io::ErrorKind::InvalidData)?;
    // SAFETY: o kernel acabou de criar `fd` para este processo e ninguém mais o conhece.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// Confere se o processo do pidfd ainda existe (sinal 0 por `pidfd_send_signal`, que
/// não entrega nada). `Ok(false)` quando ele já saiu.
#[allow(unsafe_code)]
pub(crate) fn pidfd_vivo(pidfd: BorrowedFd<'_>) -> io::Result<bool> {
    // SAFETY: `pidfd` está aberto pelo empréstimo; `info` nulo é permitido e as flags
    // são 0; o sinal 0 só testa a existência e a permissão.
    let r = unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            pidfd.as_raw_fd(),
            0 as libc::c_int,
            std::ptr::null::<libc::siginfo_t>(),
            0 as libc::c_uint,
        )
    };
    if r == 0 {
        return Ok(true);
    }
    let e = erro_os();
    if e.raw_os_error() == Some(libc::ESRCH) {
        Ok(false)
    } else {
        Err(e)
    }
}

/// `openat(dir, nome, flags | O_CLOEXEC)`. `nome` é relativo a `dir`.
#[allow(unsafe_code)]
pub(crate) fn openat(dir: BorrowedFd<'_>, nome: &CStr, flags: libc::c_int) -> io::Result<OwnedFd> {
    // SAFETY: `dir` está aberto pelo empréstimo e `nome` é uma string C válida até o
    // fim da chamada; o modo só vale com O_CREAT, que quem chama não passa.
    let fd = unsafe { libc::openat(dir.as_raw_fd(), nome.as_ptr(), flags | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(erro_os());
    }
    // SAFETY: descritor novo, criado agora pelo `openat` e sem outro dono.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// `readlinkat(dir, nome)`, com no máximo `PATH_MAX` bytes. Resultado truncado é erro.
#[allow(unsafe_code)]
pub(crate) fn readlinkat(dir: BorrowedFd<'_>, nome: &CStr) -> io::Result<Vec<u8>> {
    let capacidade = usize::try_from(libc::PATH_MAX).map_err(|_| io::ErrorKind::InvalidData)?;
    let mut buf = vec![0u8; capacidade + 1];
    // SAFETY: `dir` aberto, `nome` string C válida, e `buf` tem `buf.len()` bytes
    // graváveis; o kernel escreve no máximo esse tanto e não põe NUL.
    let r = unsafe {
        libc::readlinkat(
            dir.as_raw_fd(),
            nome.as_ptr(),
            buf.as_mut_ptr().cast(),
            buf.len(),
        )
    };
    if r < 0 {
        return Err(erro_os());
    }
    let n = usize::try_from(r).map_err(|_| io::ErrorKind::InvalidData)?;
    if n > capacidade {
        return Err(io::ErrorKind::InvalidData.into());
    }
    buf.truncate(n);
    Ok(buf)
}

#[allow(unsafe_code)]
pub(crate) fn geteuid() -> u32 {
    // SAFETY: sem argumentos e sem efeito colateral; nunca falha.
    unsafe { libc::geteuid() }
}

/// `prctl(PR_SET_DUMPABLE, 0)` e a leitura de volta por `PR_GET_DUMPABLE`.
#[allow(unsafe_code)]
pub(crate) fn desligar_dumpable() -> io::Result<()> {
    let zero: libc::c_ulong = 0;
    // SAFETY: `PR_SET_DUMPABLE` só lê os inteiros passados; os não usados vão em 0.
    let r = unsafe { libc::prctl(libc::PR_SET_DUMPABLE, zero, zero, zero, zero) };
    if r != 0 {
        return Err(erro_os());
    }
    if dumpable()? {
        return Err(io::Error::other("PR_SET_DUMPABLE não ficou em 0"));
    }
    Ok(())
}

/// `prctl(PR_GET_DUMPABLE)`: `true` quando o processo pode ser depurado ou dar core.
#[allow(unsafe_code)]
pub(crate) fn dumpable() -> io::Result<bool> {
    let zero: libc::c_ulong = 0;
    // SAFETY: `PR_GET_DUMPABLE` não recebe ponteiro; devolve o valor no retorno.
    let r = unsafe { libc::prctl(libc::PR_GET_DUMPABLE, zero, zero, zero, zero) };
    if r < 0 {
        return Err(erro_os());
    }
    Ok(r != 0)
}

/// `setrlimit(RLIMIT_CORE, 0, 0)`: o limite rígido também em 0, para o processo não
/// poder subir de novo.
#[allow(unsafe_code)]
pub(crate) fn zerar_rlimit_core() -> io::Result<()> {
    let lim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: `lim` é um `rlimit` local e válido, só lido pelo kernel.
    let r = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &raw const lim) };
    if r != 0 {
        return Err(erro_os());
    }
    Ok(())
}

/// `getrlimit(RLIMIT_CORE)` como `(atual, máximo)`.
#[allow(unsafe_code)]
pub(crate) fn rlimit_core() -> io::Result<(u64, u64)> {
    let mut lim = libc::rlimit {
        rlim_cur: u64::MAX,
        rlim_max: u64::MAX,
    };
    // SAFETY: `lim` é local, do tamanho de `rlimit`, e o kernel só escreve nele.
    let r = unsafe { libc::getrlimit(libc::RLIMIT_CORE, &raw mut lim) };
    if r != 0 {
        return Err(erro_os());
    }
    Ok((lim.rlim_cur, lim.rlim_max))
}

/// Tamanho da página.
#[allow(unsafe_code)]
pub(crate) fn tamanho_pagina() -> usize {
    // SAFETY: `sysconf` só lê uma constante do sistema.
    let r = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    usize::try_from(r).ok().filter(|&p| p > 0).unwrap_or(4096)
}

/// Região anônima privada, só deste processo, com `len` bytes (múltiplo da página).
#[allow(unsafe_code)]
pub(crate) fn mapear_anonimo(len: usize) -> io::Result<NonNull<u8>> {
    // SAFETY: endereço nulo (o kernel escolhe), sem arquivo (fd -1, offset 0); a região
    // nova não sobrepõe nada já mapeado.
    let p = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            len,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        )
    };
    if p == libc::MAP_FAILED {
        return Err(erro_os());
    }
    NonNull::new(p.cast::<u8>()).ok_or_else(|| io::Error::other("mmap devolveu nulo"))
}

/// `munmap` de uma região criada por [`mapear_anonimo`].
///
/// # Safety
/// `p` e `len` têm de ser exatamente os de uma região de [`mapear_anonimo`] ainda não
/// desmapeada, e nenhuma referência a ela pode sobreviver a esta chamada.
#[allow(unsafe_code)]
pub(crate) unsafe fn desmapear(p: NonNull<u8>, len: usize) {
    // SAFETY: garantido pelo contrato da função (região própria, sem referências vivas).
    unsafe {
        libc::munmap(p.as_ptr().cast(), len);
    }
}

/// `madvise` numa região própria. Não lê nem escreve a memória.
#[allow(unsafe_code)]
pub(crate) fn aconselhar(p: NonNull<u8>, len: usize, conselho: libc::c_int) -> io::Result<()> {
    // SAFETY: `MADV_DONTDUMP` e `MADV_WIPEONFORK` só mudam atributos das páginas da
    // região de `p`, que quem chama mapeou; não tocam no conteúdo.
    let r = unsafe { libc::madvise(p.as_ptr().cast(), len, conselho) };
    if r != 0 {
        return Err(erro_os());
    }
    Ok(())
}

/// `mlock` das páginas que contêm `regiao`. Não lê nem escreve a memória; com faixa
/// inválida o kernel só devolve erro.
#[allow(unsafe_code)]
pub(crate) fn mlock(regiao: &[u8]) -> io::Result<()> {
    if regiao.is_empty() {
        return Ok(());
    }
    // SAFETY: a fatia é memória válida deste processo pelo empréstimo; `mlock` só
    // marca as páginas como residentes.
    let r = unsafe { libc::mlock(regiao.as_ptr().cast(), regiao.len()) };
    if r != 0 {
        return Err(erro_os());
    }
    Ok(())
}

/// `munlock` das páginas que contêm `regiao`.
#[allow(unsafe_code)]
pub(crate) fn munlock(regiao: &[u8]) -> io::Result<()> {
    if regiao.is_empty() {
        return Ok(());
    }
    // SAFETY: como em [`mlock`]: só muda o atributo das páginas, sem acessar o conteúdo.
    let r = unsafe { libc::munlock(regiao.as_ptr().cast(), regiao.len()) };
    if r != 0 {
        return Err(erro_os());
    }
    Ok(())
}

#[allow(unsafe_code)]
pub(crate) fn getegid() -> u32 {
    // SAFETY: sem argumentos e sem efeito colateral; nunca falha.
    unsafe { libc::getegid() }
}

#[allow(unsafe_code)]
pub(crate) fn getgid() -> u32 {
    // SAFETY: sem argumentos e sem efeito colateral; nunca falha.
    unsafe { libc::getgid() }
}

/// `getsockopt(SOL_SOCKET, opcao)` de uma opção inteira (`SO_ACCEPTCONN`, `SO_DOMAIN`,
/// `SO_TYPE`).
#[allow(unsafe_code)]
pub(crate) fn sockopt_int(fd: BorrowedFd<'_>, opcao: libc::c_int) -> io::Result<libc::c_int> {
    let mut valor: libc::c_int = -1;
    let tamanho = size_of::<libc::c_int>();
    let mut len = libc::socklen_t::try_from(tamanho).map_err(|_| io::ErrorKind::InvalidInput)?;
    // SAFETY: `fd` está aberto pelo empréstimo; `valor` e `len` são locais e `len` tem
    // o tamanho de um `c_int`, o que o kernel escreve para essas opções.
    let r = unsafe {
        libc::getsockopt(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            opcao,
            (&raw mut valor).cast(),
            &raw mut len,
        )
    };
    if r != 0 {
        return Err(erro_os());
    }
    if usize::try_from(len).ok() != Some(tamanho) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(valor)
}

/// Primeiro fd passado pelo systemd (`SD_LISTEN_FDS_START`).
pub(crate) const FD_ATIVACAO: libc::c_int = 3;

/// Já houve uma tentativa de pegar o fd 3 neste processo.
static FD_ATIVACAO_TOMADO: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Duplica o fd 3 (com `O_CLOEXEC`, para nenhum filho herdar o socket que escuta) sem
/// tomar posse dele. Só uma vez por processo. Quem chama confere a cópia e, se ela for
/// o socket esperado, fecha o original por [`fechar_fd_ativacao`].
#[allow(unsafe_code)]
pub(crate) fn duplicar_fd_ativacao() -> io::Result<OwnedFd> {
    if FD_ATIVACAO_TOMADO.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "fd de ativação já usado",
        ));
    }
    // SAFETY: `F_GETFD` só consulta a tabela de fds; não recebe ponteiro.
    if unsafe { libc::fcntl(FD_ATIVACAO, libc::F_GETFD) } < 0 {
        return Err(erro_os());
    }
    // SAFETY: o fd 3 acabou de ser conferido como aberto, e o empréstimo só dura até o
    // `try_clone_to_owned` abaixo (um `F_DUPFD_CLOEXEC`), dentro desta função. O fd 3
    // é do systemd (`LISTEN_FDS`): o daemon chama isto no começo do `main`, antes de
    // abrir qualquer outro arquivo, então nenhum outro código o fecha nesse intervalo.
    let emprestado = unsafe { BorrowedFd::borrow_raw(FD_ATIVACAO) };
    emprestado.try_clone_to_owned()
}

/// Fecha o fd 3 depois que a cópia de [`duplicar_fd_ativacao`] foi conferida.
#[allow(unsafe_code)]
pub(crate) fn fechar_fd_ativacao() {
    // SAFETY: o fd 3 é o socket do systemd, conferido pela cópia; ninguém mais neste
    // processo o usa (o daemon segue só com a cópia), então o `OwnedFd` passa a ser o
    // único dono e o fecha ao sair do escopo.
    drop(unsafe { OwnedFd::from_raw_fd(FD_ATIVACAO) });
}

/// Só no `modo-teste`: põe `fd` no número 3 sem `O_CLOEXEC`, como o systemd faz, para
/// o `exec` seguinte receber o socket.
///
/// # Safety
/// Fecha o que estivesse no fd 3 deste processo. Quem chama tem de fazer `exec` (ou
/// sair) logo em seguida, sem que nenhum outro código use o fd 3.
#[cfg(feature = "modo-teste")]
#[allow(unsafe_code)]
pub(crate) unsafe fn por_no_fd_ativacao(fd: BorrowedFd<'_>) -> io::Result<()> {
    if fd.as_raw_fd() == FD_ATIVACAO {
        // SAFETY: `F_SETFD` com 0 só tira o `FD_CLOEXEC` do fd 3, que é `fd`.
        if unsafe { libc::fcntl(FD_ATIVACAO, libc::F_SETFD, 0) } < 0 {
            return Err(erro_os());
        }
        return Ok(());
    }
    // SAFETY: `fd` está aberto pelo empréstimo; o `dup2` troca o fd 3 (o contrato da
    // função diz que ninguém mais o usa) e o novo não tem `FD_CLOEXEC`.
    if unsafe { libc::dup2(fd.as_raw_fd(), FD_ATIVACAO) } < 0 {
        return Err(erro_os());
    }
    Ok(())
}
