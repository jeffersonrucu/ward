//! Fim de sessão sem sobra (Fase 55, S8; achado médio da reauditoria final).
//!
//! O `ward run`, o `ward ssh`, a ponte e o Chrome do `ward playwright` rodam num grupo de
//! processos próprio ([`crate::filho::Grupo`]). Matar só o grupo deixava para trás o
//! descendente que saiu dele: `setpgid(0, 0)` (outro grupo na mesma sessão) ou
//! `setsid -f` (sessão nova, e o pai intermediário sai). Esse processo seguia com o
//! segredo no ambiente depois da trava e do fim do comando. Aqui:
//!
//! - **subreaper** ([`virar_subreaper`]): o `wardd` liga `PR_SET_CHILD_SUBREAPER`; todo
//!   órfão de um descendente vira filho do `wardd`, em vez de sumir para o `init` ou o
//!   `systemd --user`;
//! - **quem é da sessão**: os descendentes do líder (pela cadeia de `ppid` em `/proc`)
//!   e, entre os filhos do `wardd` que não são líderes registrados ([`registro`]), os
//!   órfãos da sessão do líder (`sid` = líder) e os órfãos **sem dono** (sessão que não
//!   é de líder nenhum nem a do `wardd`: quem fez `setsid`), mais os descendentes
//!   deles. O kernel não reusa um pid enquanto ele é id de sessão de alguém vivo, então
//!   o `sid` não é forjável: ninguém de fora entra na sessão do líder nem na do
//!   `wardd`. O órfão sem dono não diz de qual sessão veio; ele morre no fim de
//!   **qualquer** sessão e na trava (segurança antes de conveniência);
//! - **sem corrida com o reuso de pid**: cada processo achado leva `SIGSTOP` (parado,
//!   não colhe os filhos nem cria outros) até nenhum novo aparecer; depois `SIGKILL`
//!   em todos. Todo sinal vai por `pidfd`, aberto e conferido pelo horário de início
//!   do `stat`: o sinal nunca acerta outro processo que herdou o pid;
//! - **o que sobrou** é contado ([`Relatorio::sobraram`]): a CLI só ouve "encerrado"
//!   quando nada da sessão continua vivo.
//!
//! O filho direto do `wardd` na sessão dele (o app, aberto sem `setsid`) e o que ele
//! deixar órfão não são de sessão nenhuma e não levam sinal.
//!
//! Sem o subreaper (os testes de unidade, num processo que não é o `wardd`), só os
//! descendentes do líder contam: nenhum filho do processo de teste é tocado.

use std::collections::{HashMap, HashSet};
use std::io;
use std::os::fd::AsFd;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::sys;

/// Pids dos líderes vivos (lançados e ainda não colhidos). O mesmo mutex ordena o
/// lançamento (`fork` + registro) com a varredura: um líder recém-nascido nunca é
/// tomado por órfão sem dono.
static LIDERES: Mutex<Vec<libc::pid_t>> = Mutex::new(Vec::new());
/// Este processo é subreaper (o `wardd`)?
static SUBREAPER: AtomicBool = AtomicBool::new(false);

/// Rodadas de `SIGSTOP` antes de desistir de esperar o fim dos `fork`.
const MAX_RODADAS: usize = 256;
/// Quanto esperar os processos mortos saírem antes de contar o que sobrou.
const PRAZO_MORTE: Duration = Duration::from_secs(2);

/// Liga o `PR_SET_CHILD_SUBREAPER` neste processo (o `wardd`, na partida). Sem ele, o
/// daemon não sobe (falha fechada).
pub fn virar_subreaper() -> io::Result<()> {
    sys::virar_subreaper()?;
    SUBREAPER.store(true, Ordering::SeqCst);
    Ok(())
}

/// Este processo virou subreaper por [`virar_subreaper`]?
pub fn e_subreaper() -> bool {
    SUBREAPER.load(Ordering::SeqCst)
}

/// O registro dos líderes vivos, travado.
pub(crate) fn registro() -> MutexGuard<'static, Vec<libc::pid_t>> {
    LIDERES.lock().unwrap_or_else(|e| e.into_inner())
}

/// O que o fim de uma sessão fez.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Relatorio {
    /// Processos que levaram `SIGKILL` (o líder vivo inclusive).
    pub mortos: usize,
    /// Desses, quantos continuavam vivos no fim do prazo.
    pub sobraram: usize,
}

#[derive(Debug, Clone, Copy)]
struct Proc {
    ppid: libc::pid_t,
    sid: libc::pid_t,
    inicio: u64,
    zumbi: bool,
}

/// `(estado, ppid, sid, início)` de um `/proc/<pid>/stat`. Lido depois do último `)`
/// (o nome do programa pode ter espaço e parênteses).
fn ler_stat(stat: &str) -> Option<(char, libc::pid_t, libc::pid_t, u64)> {
    let resto = &stat[stat.rfind(')')? + 1..];
    let c: Vec<&str> = resto.split_whitespace().collect();
    let estado = c.first()?.chars().next()?;
    let ppid = c.get(1)?.parse().ok()?;
    let sid = c.get(3)?.parse().ok()?;
    // `starttime` é o 22.º campo do `stat`; aqui, o 20.º depois do nome.
    let inicio = c.get(19)?.parse().ok()?;
    Some((estado, ppid, sid, inicio))
}

fn ler_proc(proc: &Path, pid: libc::pid_t) -> Option<Proc> {
    let s = std::fs::read_to_string(proc.join(pid.to_string()).join("stat")).ok()?;
    let (estado, ppid, sid, inicio) = ler_stat(&s)?;
    Some(Proc {
        ppid,
        sid,
        inicio,
        zumbi: estado == 'Z' || estado == 'X',
    })
}

/// Todos os processos de `proc` agora.
fn tabela(proc: &Path) -> HashMap<libc::pid_t, Proc> {
    let mut t = HashMap::new();
    let Ok(dir) = std::fs::read_dir(proc) else {
        return t;
    };
    for e in dir.flatten() {
        let Some(pid) = e
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<libc::pid_t>().ok())
        else {
            continue;
        };
        if let Some(p) = ler_proc(proc, pid) {
            t.insert(pid, p);
        }
    }
    t
}

/// Quem é da sessão de `lider` (ou, com `None`, só os órfãos sem dono), vivo, na
/// tabela `t`. `eu`/`meu_sid`: o processo que varre; `orfaos`: ele é subreaper.
fn alvos(
    t: &HashMap<libc::pid_t, Proc>,
    lider: Option<libc::pid_t>,
    eu: libc::pid_t,
    meu_sid: libc::pid_t,
    lideres: &[libc::pid_t],
    orfaos: bool,
) -> Vec<libc::pid_t> {
    let mut filhos: HashMap<libc::pid_t, Vec<libc::pid_t>> = HashMap::new();
    for (pid, p) in t {
        filhos.entry(p.ppid).or_default().push(*pid);
    }
    let mut fila: Vec<libc::pid_t> = Vec::new();
    if let Some(l) = lider
        && t.contains_key(&l)
    {
        fila.push(l);
    }
    if orfaos {
        for c in filhos.get(&eu).map(Vec::as_slice).unwrap_or(&[]) {
            let p = &t[c];
            if lideres.contains(c) || p.sid == meu_sid {
                continue;
            }
            let da_sessao = lider == Some(p.sid);
            let sem_dono = !lideres.contains(&p.sid);
            if da_sessao || sem_dono {
                fila.push(*c);
            }
        }
    }
    let mut vistos: HashSet<libc::pid_t> = HashSet::new();
    while let Some(p) = fila.pop() {
        if p <= 1 || p == eu || !vistos.insert(p) {
            continue;
        }
        for c in filhos.get(&p).map(Vec::as_slice).unwrap_or(&[]) {
            if !lideres.contains(c) || Some(*c) == lider {
                fila.push(*c);
            }
        }
    }
    vistos
        .into_iter()
        .filter(|p| t.get(p).is_some_and(|x| !x.zumbi))
        .collect()
}

/// `sinal` para `pid` só se ele ainda é o processo que começou em `inicio` (pidfd
/// aberto antes da conferência: o sinal não acerta quem herdou o pid).
fn sinal_conferido(pid: libc::pid_t, inicio: u64, sinal: libc::c_int) -> bool {
    let Ok(fd) = sys::pidfd_open(pid) else {
        return false;
    };
    if ler_proc(Path::new("/proc"), pid).is_none_or(|p| p.inicio != inicio) {
        return false;
    }
    sys::pidfd_sinal(fd.as_fd(), sinal).unwrap_or(false)
}

/// Colhe os órfãos zumbis (filhos deste processo, fora do registro e da sessão dele).
fn colher_orfaos(
    t: &HashMap<libc::pid_t, Proc>,
    eu: libc::pid_t,
    meu_sid: libc::pid_t,
    lideres: &[libc::pid_t],
) {
    for (pid, p) in t {
        if p.ppid == eu && p.zumbi && p.sid != meu_sid && !lideres.contains(pid) {
            sys::colher_se_saiu(*pid);
        }
    }
}

/// Mata a sessão de `lider` inteira (ver o módulo) e, sendo subreaper, os órfãos sem
/// dono. Com `None`, só os órfãos sem dono (a trava sem sessão em curso).
pub(crate) fn matar_sessao(lider: Option<libc::pid_t>) -> Relatorio {
    matar_sessao_em(Path::new("/proc"), lider)
}

fn matar_sessao_em(proc: &Path, lider: Option<libc::pid_t>) -> Relatorio {
    let lideres = registro();
    let orfaos = e_subreaper();
    let (eu, meu_sid) = sys::pid_e_sessao();
    if let Some(l) = lider {
        let _ = sys::parar_grupo(l);
    }
    // 1. Parar: cada achado leva `SIGSTOP` até a varredura não achar ninguém novo.
    let mut parados: HashMap<libc::pid_t, u64> = HashMap::new();
    for _ in 0..MAX_RODADAS {
        let t = tabela(proc);
        let novos: Vec<libc::pid_t> = alvos(&t, lider, eu, meu_sid, &lideres, orfaos)
            .into_iter()
            .filter(|p| !parados.contains_key(p))
            .collect();
        if novos.is_empty() {
            break;
        }
        for p in novos {
            let inicio = t[&p].inicio;
            sinal_conferido(p, inicio, libc::SIGSTOP);
            parados.insert(p, inicio);
        }
    }
    // 2. Matar todos (e o grupo do líder, por garantia).
    for (p, inicio) in &parados {
        sinal_conferido(*p, *inicio, libc::SIGKILL);
    }
    if let Some(l) = lider {
        let _ = sys::matar_grupo(l);
    }
    // 3. Esperar saírem; quem aparecer agora (um `SIGCONT` de fora antes do `SIGKILL`)
    //    também morre. O líder fica zumbi para o `colher` do grupo.
    let fim = Instant::now() + PRAZO_MORTE;
    let sobraram = loop {
        let t = tabela(proc);
        if orfaos {
            colher_orfaos(&t, eu, meu_sid, &lideres);
        }
        for p in alvos(&t, lider, eu, meu_sid, &lideres, orfaos) {
            if let std::collections::hash_map::Entry::Vacant(v) = parados.entry(p) {
                let inicio = t[&p].inicio;
                sinal_conferido(p, inicio, libc::SIGKILL);
                v.insert(inicio);
            }
        }
        let vivos = parados
            .iter()
            .filter(|(p, inicio)| {
                ler_proc(proc, **p).is_some_and(|x| !x.zumbi && x.inicio == **inicio)
            })
            .count();
        if vivos == 0 || Instant::now() >= fim {
            break vivos;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    if orfaos {
        colher_orfaos(&tabela(proc), eu, meu_sid, &lideres);
    }
    Relatorio {
        mortos: parados.len(),
        sobraram,
    }
}

/// Só os órfãos sem dono (a trava, Fase 55): o que saiu de uma sessão com `setsid` e
/// ficou filho do `wardd`. Sem o subreaper, não faz nada.
pub fn matar_orfaos() -> Relatorio {
    if !e_subreaper() {
        return Relatorio::default();
    }
    matar_sessao(None)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn p(ppid: libc::pid_t, sid: libc::pid_t) -> Proc {
        Proc {
            ppid,
            sid,
            inicio: 1,
            zumbi: false,
        }
    }

    #[test]
    fn stat_com_parenteses_no_nome() {
        let s = "42 (a) b) (c) S 7 42 40 0 -1 4194560 1 0 0 0 0 0 0 0 20 0 1 0 12345 0 0";
        assert_eq!(ler_stat(s), Some(('S', 7, 40, 12345)));
        assert_eq!(ler_stat("42 sem-parenteses"), None);
        assert_eq!(ler_stat("42 (x) S 7 1 1"), None);
    }

    /// A tabela de um `wardd` (100, sessão 100) com o app (101, sessão do daemon), a
    /// sessão A (200) e a sessão B (300), cada uma com um fugitivo.
    fn cenario() -> HashMap<libc::pid_t, Proc> {
        HashMap::from([
            (100, p(1, 100)),
            (101, p(100, 100)), // app
            (102, p(101, 100)), // filho do app
            (103, p(100, 100)), // órfão do app (sem setsid): não é de sessão nenhuma
            (104, p(100, 104)), // órfão do app que fez setsid: sem dono
            (200, p(100, 200)), // líder A
            (201, p(200, 200)),
            (202, p(201, 202)), // neto de A com sessão própria: descendente
            (203, p(100, 200)), // órfão de A (setpgid): sid = A
            (204, p(203, 204)),
            (300, p(100, 300)), // líder B
            (301, p(300, 300)),
            (302, p(100, 300)), // órfão de B
            (303, p(100, 303)), // órfão sem dono (setsid -f)
            (999, p(1, 999)),   // estranho
        ])
    }

    fn ordenado(mut v: Vec<libc::pid_t>) -> Vec<libc::pid_t> {
        v.sort_unstable();
        v
    }

    #[test]
    fn alvos_da_sessao_com_subreaper() {
        let t = cenario();
        let lideres = [200, 300, 101];
        assert_eq!(
            ordenado(alvos(&t, Some(200), 100, 100, &lideres, true)),
            [104, 200, 201, 202, 203, 204, 303]
        );
        assert_eq!(
            ordenado(alvos(&t, Some(300), 100, 100, &lideres, true)),
            [104, 300, 301, 302, 303]
        );
        assert_eq!(
            ordenado(alvos(&t, None, 100, 100, &lideres, true)),
            [104, 303]
        );
    }

    #[test]
    fn sem_subreaper_so_a_arvore_do_lider() {
        let t = cenario();
        assert_eq!(
            ordenado(alvos(&t, Some(200), 100, 100, &[200, 300], false)),
            [200, 201, 202]
        );
        assert!(alvos(&t, None, 100, 100, &[200, 300], false).is_empty());
    }

    #[test]
    fn zumbi_nao_e_alvo() {
        let mut t = cenario();
        t.get_mut(&200).unwrap().zumbi = true;
        t.get_mut(&303).unwrap().zumbi = true;
        assert_eq!(
            ordenado(alvos(&t, Some(200), 100, 100, &[200, 300], true)),
            [104, 201, 202, 203, 204]
        );
    }
}
