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

/// `renameat2(dir, de, dir, para, RENAME_NOREPLACE)`: renomeia dentro de `dir` sem
/// nunca sobrescrever (`EEXIST` se `para` existe). Funciona em vfat, que não tem
/// `link(2)` (Fase 46).
#[allow(unsafe_code)]
pub(crate) fn renameat_sem_sobrescrever(
    dir: BorrowedFd<'_>,
    de: &CStr,
    para: &CStr,
) -> io::Result<()> {
    // SAFETY: `dir` está aberto pelo empréstimo; `de` e `para` são strings C válidas até
    // o fim da chamada, e o kernel só as lê.
    let r = unsafe {
        libc::renameat2(
            dir.as_raw_fd(),
            de.as_ptr(),
            dir.as_raw_fd(),
            para.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if r != 0 {
        return Err(erro_os());
    }
    Ok(())
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
pub(crate) fn dumpable() -> io::Result<bool> {
    Ok(valor_dumpable()? != 0)
}

/// `prctl(PR_GET_DUMPABLE)` como o kernel devolve: 0, 1 ou 2 (`SUID_DUMP_ROOT`).
#[allow(unsafe_code)]
pub(crate) fn valor_dumpable() -> io::Result<i32> {
    let zero: libc::c_ulong = 0;
    // SAFETY: `PR_GET_DUMPABLE` não recebe ponteiro; devolve o valor no retorno.
    let r = unsafe { libc::prctl(libc::PR_GET_DUMPABLE, zero, zero, zero, zero) };
    if r < 0 {
        return Err(erro_os());
    }
    Ok(r)
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

/// `getrlimit(RLIMIT_MEMLOCK)` como `(atual, máximo)`; `u64::MAX` é `RLIM_INFINITY`.
#[allow(unsafe_code)]
pub(crate) fn rlimit_memlock() -> io::Result<(u64, u64)> {
    let mut lim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: `lim` é local, do tamanho de `rlimit`, e o kernel só escreve nele.
    let r = unsafe { libc::getrlimit(libc::RLIMIT_MEMLOCK, &raw mut lim) };
    if r != 0 {
        return Err(erro_os());
    }
    Ok((lim.rlim_cur, lim.rlim_max))
}

/// `mlockall(MCL_CURRENT | MCL_FUTURE | MCL_ONFAULT)` (D62): todas as páginas do
/// processo, as de agora e as que vierem, ficam fora da swap assim que são tocadas. Não
/// passa por `fork` nem por `execve`.
#[allow(unsafe_code)]
pub(crate) fn travar_toda_memoria() -> io::Result<()> {
    // SAFETY: `mlockall` só recebe as flags; não lê nem escreve memória do processo.
    let r = unsafe { libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE | libc::MCL_ONFAULT) };
    if r != 0 {
        return Err(erro_os());
    }
    Ok(())
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

// --- Partida do app e lançador (Fase 29a; D47, D57) -----------------------------------

/// `getresgid`: gid real, efetivo e salvo.
#[allow(unsafe_code)]
pub(crate) fn getresgid() -> io::Result<(u32, u32, u32)> {
    let (mut r, mut e, mut s) = (u32::MAX, u32::MAX, u32::MAX);
    // SAFETY: os três ponteiros são de locais `gid_t` válidos; o kernel só escreve neles.
    let rc = unsafe { libc::getresgid(&raw mut r, &raw mut e, &raw mut s) };
    if rc != 0 {
        return Err(erro_os());
    }
    Ok((r, e, s))
}

/// Larga o grupo do setgid: `setresgid(real, real, real)`, conferido por `getresgid`
/// depois. Com os três iguais ao real, o processo não volta ao grupo do binário (o
/// salvo também mudou).
#[allow(unsafe_code)]
pub(crate) fn largar_grupo_efetivo() -> io::Result<()> {
    let (real, _, _) = getresgid()?;
    // SAFETY: só inteiros; o kernel deixa um processo sem privilégio pôr os três gids
    // em qualquer um dos que ele já tem (o real é um deles).
    let rc = unsafe { libc::setresgid(real, real, real) };
    if rc != 0 {
        return Err(erro_os());
    }
    if getresgid()? != (real, real, real) {
        return Err(io::Error::other(
            "setresgid não deixou os três gids no real",
        ));
    }
    Ok(())
}

#[allow(unsafe_code)]
pub(crate) fn getppid() -> u32 {
    // SAFETY: sem argumentos e sem efeito colateral; nunca falha.
    let p = unsafe { libc::getppid() };
    u32::try_from(p).unwrap_or(0)
}

/// O fd tem `FD_CLOEXEC`?
#[allow(unsafe_code)]
pub(crate) fn tem_cloexec(fd: BorrowedFd<'_>) -> io::Result<bool> {
    // SAFETY: `F_GETFD` só consulta a tabela de fds; `fd` está aberto pelo empréstimo.
    let r = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFD) };
    if r < 0 {
        return Err(erro_os());
    }
    Ok(r & libc::FD_CLOEXEC != 0)
}

/// `tcgetattr`: atributos do terminal em `fd` (Fase 25, senha do `ward destravar`).
/// `ENOTTY` quando `fd` não é terminal.
#[allow(unsafe_code)]
pub(crate) fn tcgetattr(fd: BorrowedFd<'_>) -> io::Result<libc::termios> {
    // SAFETY: `termios` é uma struct C só de inteiros e arrays de inteiros; zerada é um
    // valor válido, e o `tcgetattr` a preenche inteira.
    let mut t: libc::termios = unsafe { std::mem::zeroed() };
    // SAFETY: `t` é local e gravável; `fd` está aberto pelo empréstimo.
    if unsafe { libc::tcgetattr(fd.as_raw_fd(), &raw mut t) } != 0 {
        return Err(erro_os());
    }
    Ok(t)
}

/// `tcsetattr(fd, acao, t)` (`TCSANOW`, `TCSAFLUSH`…).
#[allow(unsafe_code)]
pub(crate) fn tcsetattr(
    fd: BorrowedFd<'_>,
    acao: libc::c_int,
    t: &libc::termios,
) -> io::Result<()> {
    // SAFETY: `t` aponta para um `termios` válido que só é lido; `fd` está aberto pelo
    // empréstimo.
    if unsafe { libc::tcsetattr(fd.as_raw_fd(), acao, t) } != 0 {
        return Err(erro_os());
    }
    Ok(())
}

/// Abre um pseudoterminal novo e devolve (mestre, escravo), os dois com `O_NOCTTY` e
/// `O_CLOEXEC`: o pty do `ward ssh` interativo (Fase 27) e, nos testes, a leitura da
/// senha sem um terminal de verdade.
#[allow(unsafe_code)]
pub(crate) fn abrir_pty() -> io::Result<(OwnedFd, OwnedFd)> {
    let flags = libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC;
    // SAFETY: `posix_openpt` só recebe as flags e devolve um fd novo ou -1.
    let m = unsafe { libc::posix_openpt(flags) };
    if m < 0 {
        return Err(erro_os());
    }
    // SAFETY: `m` acabou de ser aberto por `posix_openpt` e só este `OwnedFd` o fecha.
    let mestre = unsafe { OwnedFd::from_raw_fd(m) };
    // SAFETY: `grantpt`/`unlockpt` só recebem o fd do mestre, aberto acima.
    if unsafe { libc::grantpt(m) } != 0 || unsafe { libc::unlockpt(m) } != 0 {
        return Err(erro_os());
    }
    let mut nome = [0u8; 128];
    // SAFETY: `nome` é local, com `nome.len()` bytes graváveis; o `ptsname_r` escreve
    // uma string C terminada em zero dentro desse tamanho ou devolve erro.
    let rc = unsafe { libc::ptsname_r(m, nome.as_mut_ptr().cast(), nome.len()) };
    if rc != 0 {
        return Err(io::Error::from_raw_os_error(rc));
    }
    let nome = CStr::from_bytes_until_nul(&nome).map_err(|_| io::ErrorKind::InvalidData)?;
    // SAFETY: `nome` é uma string C válida; `open` devolve um fd novo ou -1.
    let e = unsafe { libc::open(nome.as_ptr(), flags) };
    if e < 0 {
        return Err(erro_os());
    }
    // SAFETY: `e` acabou de ser aberto e só este `OwnedFd` o fecha.
    let escravo = unsafe { OwnedFd::from_raw_fd(e) };
    Ok((mestre, escravo))
}

/// `TIOCSWINSZ`: tamanho da janela do terminal em `fd` (o mestre do pty do `ward ssh`;
/// o kernel manda `SIGWINCH` ao grupo da frente).
#[allow(unsafe_code)]
pub(crate) fn definir_janela(fd: BorrowedFd<'_>, linhas: u16, colunas: u16) -> io::Result<()> {
    let w = libc::winsize {
        ws_row: linhas,
        ws_col: colunas,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: `w` é um `winsize` local, só lido pelo kernel; `fd` está aberto pelo
    // empréstimo.
    if unsafe { libc::ioctl(fd.as_raw_fd(), libc::TIOCSWINSZ, &raw const w) } != 0 {
        return Err(erro_os());
    }
    Ok(())
}

/// `TIOCGWINSZ`: (linhas, colunas) do terminal em `fd`. `ENOTTY` se não é terminal.
#[allow(unsafe_code)]
pub(crate) fn ler_janela(fd: BorrowedFd<'_>) -> io::Result<(u16, u16)> {
    let mut w = libc::winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: `w` é um `winsize` local e gravável, do tamanho que o `TIOCGWINSZ`
    // escreve; `fd` está aberto pelo empréstimo.
    if unsafe { libc::ioctl(fd.as_raw_fd(), libc::TIOCGWINSZ, &raw mut w) } != 0 {
        return Err(erro_os());
    }
    Ok((w.ws_row, w.ws_col))
}

/// `cfmakeraw` em `t` (modo cru do terminal da CLI no `ward ssh` interativo).
#[allow(unsafe_code)]
pub(crate) fn modo_cru(t: &mut libc::termios) {
    // SAFETY: `t` é um `termios` válido e gravável; o `cfmakeraw` só mexe nos campos.
    unsafe { libc::cfmakeraw(t) };
}

/// Só nos testes: tira o `FD_CLOEXEC` de `fd`, para simular um fd que o `exec` herdaria.
#[cfg(test)]
#[allow(unsafe_code)]
pub(crate) fn tirar_cloexec(fd: BorrowedFd<'_>) -> io::Result<()> {
    // SAFETY: `F_SETFD` com 0 só muda a flag do fd, aberto pelo empréstimo.
    if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, 0) } < 0 {
        return Err(erro_os());
    }
    Ok(())
}

/// Espera até o outro lado do socket fechar (`POLLRDHUP`/`POLLHUP`/`POLLERR`), sem ler
/// nada: não consome os bytes de uma resposta do protocolo.
pub(crate) fn esperar_fechamento(fd: BorrowedFd<'_>) -> io::Result<()> {
    while !fechamento(fd, -1)? {}
    Ok(())
}

/// O outro lado do socket fechou? Sem ler nada; `espera_ms` é o prazo do `poll` (0 não
/// espera, -1 espera sem prazo).
#[allow(unsafe_code)]
pub(crate) fn fechamento(fd: BorrowedFd<'_>, espera_ms: libc::c_int) -> io::Result<bool> {
    loop {
        let mut pfd = libc::pollfd {
            fd: fd.as_raw_fd(),
            events: libc::POLLRDHUP,
            revents: 0,
        };
        // SAFETY: `pfd` é um `pollfd` local e válido, e o tamanho passado é 1; o fd
        // está aberto pelo empréstimo; o prazo é um inteiro.
        let r = unsafe { libc::poll(&raw mut pfd, 1, espera_ms) };
        if r < 0 {
            let e = erro_os();
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e);
        }
        let fim = libc::POLLRDHUP | libc::POLLHUP | libc::POLLERR | libc::POLLNVAL;
        return Ok(pfd.revents & fim != 0);
    }
}

/// Diretório do usuário `uid` pelo `getpwuid_r` (passwd ou NSS do sistema), nunca pelo
/// `$HOME`. Sem entrada ou com home vazio é erro.
#[allow(unsafe_code)]
pub(crate) fn home_do_uid(uid: u32) -> io::Result<std::ffi::OsString> {
    use std::os::unix::ffi::OsStrExt;
    let mut tamanho = 4096usize;
    loop {
        let mut buf = vec![0u8; tamanho];
        // SAFETY: `passwd` é uma struct C só de ponteiros e inteiros; zerada é um valor
        // válido (ponteiros nulos), e o `getpwuid_r` a preenche.
        let mut pw: libc::passwd = unsafe { std::mem::zeroed() };
        let mut resultado: *mut libc::passwd = std::ptr::null_mut();
        // SAFETY: `pw` e `resultado` são locais; `buf` tem `buf.len()` bytes graváveis
        // e vive até o fim desta volta do laço, enquanto os ponteiros de `pw` apontam
        // para dentro dele.
        let rc = unsafe {
            libc::getpwuid_r(
                uid,
                &raw mut pw,
                buf.as_mut_ptr().cast(),
                buf.len(),
                &raw mut resultado,
            )
        };
        if rc == libc::ERANGE && tamanho < 1024 * 1024 {
            tamanho *= 4;
            continue;
        }
        if rc != 0 {
            return Err(io::Error::from_raw_os_error(rc));
        }
        if resultado.is_null() || pw.pw_dir.is_null() {
            return Err(io::ErrorKind::NotFound.into());
        }
        // SAFETY: `pw_dir` não é nulo e aponta para uma string C dentro de `buf`, que
        // ainda está vivo; a cópia sai antes de `buf` ser liberado.
        let dir = unsafe { CStr::from_ptr(pw.pw_dir) };
        if dir.to_bytes().is_empty() {
            return Err(io::ErrorKind::NotFound.into());
        }
        return Ok(std::ffi::OsStr::from_bytes(dir.to_bytes()).to_owned());
    }
}

/// Faz o `exec` de `cmd` fechar todo fd ≥ 3: depois do `fork`, marca todos com
/// `FD_CLOEXEC` (`close_range` com `CLOSE_RANGE_CLOEXEC`), e o `exec` os fecha; o
/// programa recebe só 0–2. Marcar em vez de fechar mantém o canal do `std` que avisa a
/// falha do `exec` ao pai. Kernel sem a opção (< 5.11) faz o `spawn`/`exec` falhar
/// (falha fechada).
#[allow(unsafe_code)]
pub(crate) fn fechar_fds_no_exec(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    // SAFETY: o fechamento roda no filho entre o `fork` e o `exec` (ou no próprio
    // processo, logo antes do `exec`) e só faz uma syscall (`close_range`), que é
    // async-signal-safe: não aloca nem toca em trava.
    unsafe {
        cmd.pre_exec(|| {
            let r = libc::syscall(
                libc::SYS_close_range,
                3 as libc::c_uint,
                libc::c_uint::MAX,
                libc::CLOSE_RANGE_CLOEXEC,
            );
            if r != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

/// `clock_gettime(CLOCK_BOOTTIME)`: relógio monotônico que **conta o tempo suspenso**
/// (o `Instant` do Rust usa `CLOCK_MONOTONIC`, que para na suspensão). Serve à trava por
/// inatividade (Fase 24): 4 h com a máquina suspensa contam como 4 h sem uso.
#[allow(unsafe_code)]
pub(crate) fn relogio_boot() -> io::Result<std::time::Duration> {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` é local e do tipo que o `clock_gettime` escreve; `CLOCK_BOOTTIME` é
    // um relógio válido no Linux (2.6.39+). A chamada só escreve em `ts`.
    let r = unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &raw mut ts) };
    if r != 0 {
        return Err(erro_os());
    }
    let seg = u64::try_from(ts.tv_sec).map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
    let nano =
        u32::try_from(ts.tv_nsec).map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
    if nano >= 1_000_000_000 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(std::time::Duration::new(seg, nano))
}

// --- Filho do `ward run` (Fase 26; S3, S8, D13, D60) ---------------------------------

/// `pipe2` com `O_CLOEXEC`: (leitura, escrita), os dois ≥ 3 (um fd 0–2 vira cópia ≥ 3),
/// para o `dup2` do filho nunca pisar num fd que ainda vai usar.
#[allow(unsafe_code)]
fn pipe_cloexec() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: `fds` é um array local de dois `c_int`, o que o `pipe2` escreve.
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        return Err(erro_os());
    }
    // SAFETY: os dois fds acabaram de ser criados pelo `pipe2` e são só nossos.
    let (r, w) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    Ok((acima_de_2(r)?, acima_de_2(w)?))
}

/// O mesmo fd, ou uma cópia `CLOEXEC` ≥ 3 se ele for 0, 1 ou 2.
#[allow(unsafe_code)]
fn acima_de_2(fd: OwnedFd) -> io::Result<OwnedFd> {
    if fd.as_raw_fd() > 2 {
        return Ok(fd);
    }
    // SAFETY: `F_DUPFD_CLOEXEC` só duplica o fd aberto (vivo pelo `fd`) para o menor
    // número livre ≥ 3.
    let novo = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
    if novo < 0 {
        return Err(erro_os());
    }
    // SAFETY: `novo` acabou de ser criado pelo `fcntl` e é só nosso.
    Ok(unsafe { OwnedFd::from_raw_fd(novo) })
}

/// Tudo que o filho usa, montado antes do `fork`: depois dele, o filho só faz chamadas
/// async-signal-safe, sem alocar nem tocar em trava (o daemon tem várias threads).
pub(crate) struct PreparoExec<'a> {
    /// Caminhos a tentar no `execve`, na ordem (a busca no `PATH` já feita).
    pub caminhos: &'a [*const libc::c_char],
    /// `argv`, terminado por nulo.
    pub argv: &'a [*const libc::c_char],
    /// `envp`, terminado por nulo.
    pub envp: &'a [*const libc::c_char],
    /// Diretório de trabalho.
    pub cwd: &'a CStr,
}

/// Como o filho recebe 0, 1 e 2.
pub(crate) enum Stdio {
    /// Três pipes novos (o `ward run`, o `ward ssh` com comando).
    Pipes,
    /// O escravo de um pty nos três, como terminal de controle de uma sessão nova
    /// (`setsid` + `TIOCSCTTY`; o `ward ssh` interativo). O pai fica com o mestre.
    Pty(OwnedFd),
}

/// As pontas do pai.
pub(crate) enum PontasPai {
    Pipes {
        entrada: OwnedFd,
        saida: OwnedFd,
        erro: OwnedFd,
    },
    /// O pai não guarda o escravo: o fim da sessão aparece no mestre (`EIO`).
    Pty,
}

/// O filho lançado: pid e as pontas do pai.
pub(crate) struct FilhoCru {
    pub pid: libc::pid_t,
    pub pontas: PontasPai,
}

/// Por que o filho não chegou ao `execve`.
#[derive(Debug)]
pub(crate) enum ErroExec {
    /// Falha no pai (pipe, fork).
    Sistema(io::Error),
    /// O filho avisou pelo pipe: etapa ([`ETAPA_DIRETORIO`]…) e `errno`.
    Filho { etapa: u8, errno: i32 },
}

pub(crate) const ETAPA_PREPARO: u8 = 1;
pub(crate) const ETAPA_DIRETORIO: u8 = 2;
pub(crate) const ETAPA_EXEC: u8 = 3;

/// `errno` da thread, sem alocar.
#[allow(unsafe_code)]
fn errno() -> i32 {
    // SAFETY: `__errno_location` devolve o ponteiro (válido) para o `errno` da thread.
    unsafe { *libc::__errno_location() }
}

/// `fork` + `execve` com o `PreparoExec` (Fase 26). No filho, nesta ordem: grupo de
/// processos próprio (`setpgid`), morte junto com a thread do daemon que o criou
/// (`PR_SET_PDEATHSIG`), sinais no padrão e máscara vazia, os pipes em 0–2, o `chdir`,
/// todo fd ≥ 3 marcado `CLOEXEC` (`close_range`) e o `execve` de cada caminho. Uma
/// falha antes do `execve` volta pelo pipe de aviso (5 bytes: etapa e `errno`); o
/// sucesso fecha o pipe no `exec`, sem bytes. Nada passa por shell (S3).
#[allow(unsafe_code)]
pub(crate) fn fork_exec(p: &PreparoExec<'_>, stdio: Stdio) -> Result<FilhoCru, ErroExec> {
    let nulo = |v: &[*const libc::c_char]| v.last().is_some_and(|x| x.is_null());
    if p.caminhos.is_empty() || !nulo(p.argv) || !nulo(p.envp) {
        return Err(ErroExec::Sistema(io::ErrorKind::InvalidInput.into()));
    }
    // (fds do filho, pontas do pai, se é sessão nova com terminal de controle)
    let (filho_fds, pontas, sessao) = match stdio {
        Stdio::Pipes => {
            let (in_r, in_w) = pipe_cloexec().map_err(ErroExec::Sistema)?;
            let (out_r, out_w) = pipe_cloexec().map_err(ErroExec::Sistema)?;
            let (err_r, err_w) = pipe_cloexec().map_err(ErroExec::Sistema)?;
            (
                [in_r, out_w, err_w],
                PontasPai::Pipes {
                    entrada: in_w,
                    saida: out_r,
                    erro: err_r,
                },
                false,
            )
        }
        Stdio::Pty(escravo) => {
            let escravo = acima_de_2(escravo).map_err(ErroExec::Sistema)?;
            let e2 = escravo.try_clone().map_err(ErroExec::Sistema)?;
            let e3 = escravo.try_clone().map_err(ErroExec::Sistema)?;
            let (e2, e3) = (
                acima_de_2(e2).map_err(ErroExec::Sistema)?,
                acima_de_2(e3).map_err(ErroExec::Sistema)?,
            );
            ([escravo, e2, e3], PontasPai::Pty, true)
        }
    };
    let (aviso_r, aviso_w) = pipe_cloexec().map_err(ErroExec::Sistema)?;
    // SAFETY: `getpid` não tem pré-condição.
    let pai = unsafe { libc::getpid() };
    // SAFETY: `sigset_t` e `sigaction` são structs C de inteiros; zeradas são valores
    // válidos, e o `sigemptyset` as deixa no formato certo.
    let (vazio, padrao) = unsafe {
        let mut vazio: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&raw mut vazio);
        let mut padrao: libc::sigaction = std::mem::zeroed();
        padrao.sa_sigaction = libc::SIG_DFL;
        libc::sigemptyset(&raw mut padrao.sa_mask);
        (vazio, padrao)
    };
    let fds = [
        (filho_fds[0].as_raw_fd(), 0),
        (filho_fds[1].as_raw_fd(), 1),
        (filho_fds[2].as_raw_fd(), 2),
    ];
    let aviso = aviso_w.as_raw_fd();
    // SAFETY: `fork` não tem pré-condição; no filho só seguem chamadas
    // async-signal-safe (`filho_depois_do_fork`), sem alocar.
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return Err(ErroExec::Sistema(erro_os()));
    }
    if pid == 0 {
        // SAFETY: estamos no filho logo depois do `fork`; os ponteiros do `PreparoExec`
        // apontam para memória do pai copiada no filho, viva até o `execve`/`_exit`.
        unsafe { filho_depois_do_fork(p, pai, &fds, sessao, aviso, &vazio, &padrao) }
    }
    drop((filho_fds, aviso_w));
    if !sessao {
        // Também no pai, para o grupo existir já na volta (sem corrida com o `kill`).
        // Com pty, o filho é líder de sessão (`setsid`): o grupo existe quando o pipe de
        // aviso fecha no `exec`, antes da volta.
        // SAFETY: `setpgid` só mexe no grupo do filho; o erro (já trocou de grupo, ou
        // já fez o `exec`) não importa.
        unsafe { libc::setpgid(pid, pid) };
    }
    let mut buf = [0u8; 5];
    let mut lido = 0;
    let mut f = std::fs::File::from(aviso_r);
    while lido < buf.len() {
        match io::Read::read(&mut f, &mut buf[lido..]) {
            Ok(0) => break,
            Ok(n) => lido += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    if lido == 0 {
        return Ok(FilhoCru { pid, pontas });
    }
    // O filho não chegou ao `exec` e já saiu (ou vai sair): colhe.
    let _ = colher(pid);
    let errno = if lido == buf.len() {
        i32::from_ne_bytes([buf[1], buf[2], buf[3], buf[4]])
    } else {
        libc::EIO
    };
    Err(ErroExec::Filho {
        etapa: buf[0],
        errno,
    })
}

/// Parte do filho, entre o `fork` e o `execve`. Nunca volta.
///
/// # Safety
/// Só no filho recém-criado pelo `fork`, com o `PreparoExec` válido (ponteiros para
/// strings C terminadas em nulo, listas terminadas em nulo).
#[allow(unsafe_code)]
unsafe fn filho_depois_do_fork(
    p: &PreparoExec<'_>,
    pai: libc::pid_t,
    fds: &[(libc::c_int, libc::c_int); 3],
    sessao: bool,
    aviso: libc::c_int,
    vazio: &libc::sigset_t,
    padrao: &libc::sigaction,
) -> ! {
    let avisar = |etapa: u8, e: i32| {
        let b = e.to_ne_bytes();
        let msg = [etapa, b[0], b[1], b[2], b[3]];
        // SAFETY: `msg` é local; `write` é async-signal-safe; o erro não importa (o pai
        // vê o pipe fechar sem os 5 bytes).
        unsafe { libc::write(aviso, msg.as_ptr().cast(), msg.len()) };
    };
    // SAFETY: (todo o bloco) no filho de um `fork`, só chamadas async-signal-safe
    // (`setpgid`, `setsid`, `ioctl(TIOCSCTTY)`, `prctl`, `getppid`, `sigaction`,
    // `sigprocmask`, `dup2`, `chdir`, `syscall(close_range)`, `execve`, `write`,
    // `_exit`), sobre fds e ponteiros válidos (os do `PreparoExec`, vivos na cópia da
    // memória do pai).
    unsafe {
        if sessao {
            // Sessão nova (o grupo tem o pid do filho) com o escravo como terminal de
            // controle: o `ssh` lê a janela e os sinais do terminal dele.
            if libc::setsid() < 0 || libc::ioctl(fds[0].0, libc::TIOCSCTTY, 0 as libc::c_int) != 0 {
                avisar(ETAPA_PREPARO, errno());
                libc::_exit(127);
            }
        } else {
            libc::setpgid(0, 0);
        }
        if libc::prctl(
            libc::PR_SET_PDEATHSIG,
            libc::SIGKILL as libc::c_ulong,
            0,
            0,
            0,
        ) != 0
        {
            avisar(ETAPA_PREPARO, errno());
            libc::_exit(127);
        }
        // A thread do daemon já morreu antes do `prctl`: não segue órfão.
        if libc::getppid() != pai {
            libc::_exit(127);
        }
        // O runtime do Rust ignora o SIGPIPE; o filho começa com tudo no padrão.
        for sinal in 1..65 {
            if sinal != libc::SIGKILL && sinal != libc::SIGSTOP {
                libc::sigaction(sinal, padrao, std::ptr::null_mut());
            }
        }
        if libc::sigprocmask(libc::SIG_SETMASK, vazio, std::ptr::null_mut()) != 0 {
            avisar(ETAPA_PREPARO, errno());
            libc::_exit(127);
        }
        for &(de, para) in fds {
            // `de` ≥ 3 (`pipe_cloexec`): nunca é um dos destinos 0–2.
            if libc::dup2(de, para) < 0 {
                avisar(ETAPA_PREPARO, errno());
                libc::_exit(127);
            }
        }
        if libc::chdir(p.cwd.as_ptr()) != 0 {
            avisar(ETAPA_DIRETORIO, errno());
            libc::_exit(127);
        }
        // Nenhum fd do daemon (socket, cofre, conexões) chega ao programa.
        if libc::syscall(
            libc::SYS_close_range,
            3 as libc::c_uint,
            libc::c_uint::MAX,
            libc::CLOSE_RANGE_CLOEXEC,
        ) != 0
        {
            avisar(ETAPA_PREPARO, errno());
            libc::_exit(127);
        }
        // A busca do `execvpe`: segue nos erros de "não está aqui"; com `EACCES` em
        // algum, esse é o erro final. Sem o recurso ao `/bin/sh` do `ENOEXEC`.
        let mut viu_eacces = false;
        let mut ultimo = libc::ENOENT;
        for &c in p.caminhos {
            libc::execve(c, p.argv.as_ptr(), p.envp.as_ptr());
            ultimo = errno();
            match ultimo {
                libc::EACCES => viu_eacces = true,
                libc::ENOENT | libc::ENOTDIR | libc::ESTALE | libc::ENODEV | libc::ETIMEDOUT => {}
                _ => break,
            }
        }
        avisar(ETAPA_EXEC, if viu_eacces { libc::EACCES } else { ultimo });
        libc::_exit(127)
    }
}

/// Espera o processo `pid` terminar **sem colher** (`waitid` com `WNOWAIT`): o zumbi
/// segura o pid (e o grupo), e o `kill` do grupo não acerta outro processo.
#[allow(unsafe_code)]
pub(crate) fn esperar_sem_colher(pid: libc::pid_t) -> io::Result<()> {
    let id = libc::id_t::try_from(pid).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    loop {
        // SAFETY: `siginfo_t` é uma struct C; zerada é válida, e o `waitid` a escreve.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // SAFETY: `info` é local; `P_PID` com um pid nosso; `WNOWAIT` não colhe.
        let r = unsafe {
            libc::waitid(
                libc::P_PID,
                id,
                &raw mut info,
                libc::WEXITED | libc::WNOWAIT,
            )
        };
        if r == 0 {
            return Ok(());
        }
        let e = erro_os();
        if e.kind() != io::ErrorKind::Interrupted {
            return Err(e);
        }
    }
}

/// Colhe o filho `pid` (`waitpid`) e devolve o status cru.
#[allow(unsafe_code)]
pub(crate) fn colher(pid: libc::pid_t) -> io::Result<libc::c_int> {
    loop {
        let mut status: libc::c_int = 0;
        // SAFETY: `status` é local; o pid é de um filho nosso.
        let r = unsafe { libc::waitpid(pid, &raw mut status, 0) };
        if r == pid {
            return Ok(status);
        }
        let e = erro_os();
        if e.kind() != io::ErrorKind::Interrupted {
            return Err(e);
        }
    }
}

/// Há o que ler em `fd` (dados, fim ou erro) dentro de `espera_ms`? Sem ler nada.
#[allow(unsafe_code)]
pub(crate) fn esperar_leitura(fd: BorrowedFd<'_>, espera_ms: libc::c_int) -> io::Result<bool> {
    loop {
        let mut pfd = libc::pollfd {
            fd: fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: `pfd` é um `pollfd` local e válido, e o tamanho passado é 1; o fd
        // está aberto pelo empréstimo.
        let r = unsafe { libc::poll(&raw mut pfd, 1, espera_ms) };
        if r < 0 {
            let e = erro_os();
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e);
        }
        return Ok(r > 0);
    }
}

/// `SIGKILL` para o grupo de processos `pgid` inteiro. Grupo que não existe mais não é
/// erro.
#[allow(unsafe_code)]
pub(crate) fn matar_grupo(pgid: libc::pid_t) -> io::Result<()> {
    if pgid <= 1 {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    // SAFETY: `kill` com `-pgid` (> 1) manda o sinal só ao grupo; sem ponteiros.
    if unsafe { libc::kill(-pgid, libc::SIGKILL) } != 0 {
        let e = erro_os();
        if e.raw_os_error() != Some(libc::ESRCH) {
            return Err(e);
        }
    }
    Ok(())
}
