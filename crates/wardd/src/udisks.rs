//! udisks2 no barramento do sistema (Fase 46): quais pendrives existem e quando um é
//! conectado ou montado.
//!
//! - **Consulta** ([`Udisks`]): `GetManagedObjects` de `org.freedesktop.UDisks2`, numa
//!   conexão nova a cada consulta, com o prazo das chamadas da Fase 24. De cada objeto
//!   com `Block` e `Filesystem` saem `IdUUID`, `IdLabel`, `DeviceNumber` e o primeiro
//!   de `MountPoints` ([`ler_objetos`]). Só entram sistemas de arquivos que a área de
//!   trabalho mostra: `HintIgnore` falso e `HintSystem` falso (pendrive, cartão), ou um
//!   dispositivo de loop (a imagem de teste do V7.4 montada por `udisksctl`).
//! - **Tudo que vem do barramento é conferido**: UUID no formato de D45, rótulo sem
//!   caractere de controle e cortado em [`MAX_ROTULO`], montagem absoluta, sem NUL, sem
//!   `.`/`..` e com até 4096 bytes (fora disso, conta como não montado), e no máximo
//!   [`MAX_OBJETOS`] objetos lidos. Quem garante que a montagem é a do UUID é o
//!   [`crate::externo`], pelo `st_dev` do fd aberto.
//! - **Vigia** ([`vigiar_udisks`]): uma thread assina os sinais de
//!   `org.freedesktop.UDisks2` (`InterfacesAdded`, `PropertiesChanged` do
//!   `MountPoints`…) e, a cada um, cutuca a thread da cópia, que espera um pouco (os
//!   sinais vêm em rajada), pede ao daemon a cópia ([`Daemon::sincronizar_externo`]) e
//!   registra o resultado com texto fixo. A cópia também é cutucada pelo próprio daemon
//!   (destravar, gravar, ligar o externo) e roda a cada [`INTERVALO_COPIA`], para um
//!   sinal perdido não deixar o pendrive sem cópia.
//! - Nenhum sinal ou resposta do barramento destrava, grava o cofre ou muda a
//!   configuração: o pior que um falso udisks2 consegue é apontar um diretório do
//!   próprio dispositivo que ele diz, e o que vai para lá é o backup cifrado.
//! - O endereço é o mesmo da Fase 24 ([`crate::trava::barramento_do_daemon`]): fixo no
//!   release; no daemon de teste, só `WARD_DEV_DBUS_SISTEMA`, e sem ela nem o logind
//!   nem o udisks2 são ligados.

use std::collections::HashMap;
use std::io;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::Duration;

use std::future::poll_fn;
use std::num::NonZeroUsize;
use std::pin::Pin;

use futures_core::Stream;
use zbus::message::Type as TipoMensagem;
use zbus::{Connection, MatchRule, MessageStream, OwnedObjectPath, OwnedVariant, Variant};

use ward_core::configuracao::BackupExterno;
use ward_core::protocolo::MAX_ROTULO;

use crate::externo::{ErroExterno, FontePendrives, Visto};
use crate::nucleo::Daemon;
use crate::trava::{ESPERA_MAXIMA_BARRAMENTO, conectar};
use crate::{aviso, depurar};

const UDISKS: &str = "org.freedesktop.UDisks2";
const CAMINHO_UDISKS: &str = "/org/freedesktop/UDisks2";
const IFACE_OBJETOS: &str = "org.freedesktop.DBus.ObjectManager";
const IFACE_BLOCO: &str = "org.freedesktop.UDisks2.Block";
const IFACE_FS: &str = "org.freedesktop.UDisks2.Filesystem";
const IFACE_LOOP: &str = "org.freedesktop.UDisks2.Loop";
/// Mais objetos lidos de uma resposta do udisks2.
pub const MAX_OBJETOS: usize = 4096;
/// Maior caminho de montagem aceito.
const MAX_MONTAGEM: usize = 4096;
/// Espera depois de um sinal, para juntar a rajada (o `MountPoints` chega depois do
/// `InterfacesAdded`).
pub const ESPERA_RAJADA: Duration = Duration::from_millis(500);
/// De quanto em quanto tempo a cópia roda mesmo sem sinal.
pub const INTERVALO_COPIA: Duration = Duration::from_secs(300);
const FILA: NonZeroUsize = NonZeroUsize::new(256).unwrap();

/// Objetos do `GetManagedObjects`: caminho → interface → propriedade → valor.
pub type Objetos = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedVariant>>>;

fn texto(p: &HashMap<String, OwnedVariant>, k: &str) -> Option<String> {
    match p.get(k).map(|v| &**v) {
        Some(Variant::Str(s)) => Some(s.as_str().to_owned()),
        _ => None,
    }
}

fn logico(p: &HashMap<String, OwnedVariant>, k: &str) -> Option<bool> {
    match p.get(k).map(|v| &**v) {
        Some(Variant::Bool(b)) => Some(*b),
        _ => None,
    }
}

fn numero(p: &HashMap<String, OwnedVariant>, k: &str) -> Option<u64> {
    match p.get(k).map(|v| &**v) {
        Some(Variant::U64(n)) => Some(*n),
        _ => None,
    }
}

/// `aay` → bytes de cada item.
fn bytes_de_aay(v: &Variant<'_>) -> Option<Vec<Vec<u8>>> {
    let Variant::Array(lista) = v else {
        return None;
    };
    let mut saida = Vec::new();
    for item in lista.inner() {
        let Variant::Array(b) = item else {
            return None;
        };
        let mut bytes = Vec::with_capacity(b.len());
        for x in b.inner() {
            let Variant::U8(c) = x else {
                return None;
            };
            bytes.push(*c);
        }
        saida.push(bytes);
    }
    Some(saida)
}

/// Caminho de montagem aceito: o `ay` sem o NUL do fim, absoluto, sem NUL no meio, sem
/// `.`/`..` e até [`MAX_MONTAGEM`] bytes.
pub fn montagem_valida(mut b: Vec<u8>) -> Option<PathBuf> {
    if b.last() == Some(&0) {
        b.pop();
    }
    if b.is_empty() || b.len() > MAX_MONTAGEM || b[0] != b'/' || b.contains(&0) {
        return None;
    }
    // Cada parte depois da raiz: não vazia (sem `//` nem `/` no fim), nem `.`, nem `..`.
    let simples = b == b"/"
        || b[1..]
            .split(|c| *c == b'/')
            .all(|parte| !parte.is_empty() && parte != b"." && parte != b"..");
    simples.then(|| PathBuf::from(std::ffi::OsString::from_vec(b)))
}

/// Rótulo para mostrar: sem caractere de controle, cortado em [`MAX_ROTULO`] bytes.
pub fn rotulo_limpo(r: &str) -> String {
    let mut s = String::new();
    for c in r.chars().filter(|c| !c.is_control()) {
        if s.len() + c.len_utf8() > MAX_ROTULO {
            break;
        }
        s.push(c);
    }
    s
}

/// Os pendrives dos objetos do udisks2, ordenados pelo UUID. Objeto que não passa nas
/// conferências fica de fora.
pub fn ler_objetos(objetos: &Objetos) -> Vec<Visto> {
    let mut vistos = Vec::new();
    for ifaces in objetos.values().take(MAX_OBJETOS) {
        let (Some(bloco), Some(fs)) = (ifaces.get(IFACE_BLOCO), ifaces.get(IFACE_FS)) else {
            continue;
        };
        if texto(bloco, "IdUsage").as_deref() != Some("filesystem")
            || logico(bloco, "HintIgnore") != Some(false)
        {
            continue;
        }
        let de_loop = ifaces.contains_key(IFACE_LOOP);
        if logico(bloco, "HintSystem") != Some(false) && !de_loop {
            continue;
        }
        let Some(uuid) = texto(bloco, "IdUUID") else {
            continue;
        };
        if BackupExterno::novo(false, Some(&uuid)).is_err() {
            continue;
        }
        let Some(dispositivo) = numero(bloco, "DeviceNumber") else {
            continue;
        };
        let rotulo = rotulo_limpo(&texto(bloco, "IdLabel").unwrap_or_default());
        let montagem = fs
            .get("MountPoints")
            .and_then(|v| bytes_de_aay(v))
            .and_then(|l| l.into_iter().next())
            .and_then(montagem_valida);
        vistos.push(Visto {
            uuid,
            rotulo,
            montagem,
            dispositivo,
        });
    }
    vistos.sort_by(|a, b| a.uuid.cmp(&b.uuid).then(a.dispositivo.cmp(&b.dispositivo)));
    vistos
}

async fn consultar(conn: &Connection) -> Option<Vec<Visto>> {
    let r = conn
        .call_method(
            Some(UDISKS),
            CAMINHO_UDISKS,
            Some(IFACE_OBJETOS),
            "GetManagedObjects",
            &(),
        )
        .await
        .ok()?;
    let objetos: Objetos = r.body().deserialize().ok()?;
    Some(ler_objetos(&objetos))
}

/// O udisks2 no barramento em `socket` (conexão nova a cada consulta).
pub struct Udisks {
    socket: PathBuf,
}

impl Udisks {
    pub fn novo(socket: PathBuf) -> Self {
        Self { socket }
    }
}

impl FontePendrives for Udisks {
    fn pendrives(&self) -> Option<Vec<Visto>> {
        zbus::block_on(async {
            let conn = conectar(&self.socket).await?;
            consultar(&conn).await
        })
    }
}

async fn proxima(sinais: &mut MessageStream) -> Option<zbus::Result<zbus::Message>> {
    poll_fn(|cx| Pin::new(&mut *sinais).poll_next(cx)).await
}

/// Uma sessão de sinais: cutuca a cópia a cada sinal do udisks2. `false` se não
/// conectou.
async fn sessao(daemon: &Daemon, socket: &Path, pronto: &dyn Fn()) -> bool {
    let Some(conn) = conectar(socket).await else {
        return false;
    };
    let Ok(regra) = MatchRule::builder()
        .msg_type(TipoMensagem::Signal)
        .sender(UDISKS)
        .path_namespace(CAMINHO_UDISKS)
        .build()
    else {
        return false;
    };
    let Ok(mut sinais) = MessageStream::for_match_rule(regra, &conn, Some(FILA)).await else {
        return false;
    };
    depurar!("udisks2: vigiando pendrives");
    pronto();
    // O pendrive pode já estar conectado.
    daemon.cutucar_externo();
    while let Some(Ok(_)) = proxima(&mut sinais).await {
        daemon.cutucar_externo();
    }
    true
}

/// Registra o resultado de uma cópia, com texto fixo.
pub fn registrar(r: &Result<Option<crate::externo::Copia>, ErroExterno>) {
    match r {
        Ok(None) => {}
        Ok(Some(c)) if c.copiadas.is_empty() => depurar!("backup externo: pendrive em dia"),
        Ok(Some(c)) => aviso!(
            "backup externo: {} versão(ões) copiada(s) para o pendrive",
            c.copiadas.len()
        ),
        // Pendrive ausente é o normal.
        Err(ErroExterno::NaoConectado | ErroExterno::NaoMontado) => {}
        Err(e) => aviso!("backup externo não copiado ({})", e.codigo()),
    }
}

/// Thread da cópia: espera um cutucão (ou [`INTERVALO_COPIA`]), junta a rajada e copia.
pub fn vigiar_copia(
    daemon: Arc<Daemon>,
    fonte: Arc<dyn FontePendrives>,
    cutucoes: Receiver<()>,
) -> io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("wardd-externo".into())
        .spawn(move || {
            loop {
                match cutucoes.recv_timeout(INTERVALO_COPIA) {
                    Ok(()) | Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
                std::thread::sleep(ESPERA_RAJADA);
                while cutucoes.try_recv().is_ok() {}
                registrar(&daemon.sincronizar_externo(&*fonte));
            }
        })
}

/// Thread que vigia os sinais do udisks2 em `socket` para sempre. `pronto` é chamado a
/// cada conexão aberta (os testes esperam por ele).
pub fn vigiar_udisks(
    daemon: Arc<Daemon>,
    socket: PathBuf,
    pronto: Box<dyn Fn() + Send>,
) -> io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("wardd-udisks".into())
        .spawn(move || {
            let mut espera = Duration::from_secs(1);
            let mut avisou = false;
            loop {
                if zbus::block_on(sessao(&daemon, &socket, &*pronto)) {
                    espera = Duration::from_secs(1);
                    avisou = false;
                } else if !avisou {
                    aviso!("udisks2: barramento indisponível; backup externo parado até conectar");
                    avisou = true;
                }
                std::thread::sleep(espera);
                espera = (espera * 2).min(ESPERA_MAXIMA_BARRAMENTO);
            }
        })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ataque_montagem_estranha_conta_como_nao_montado() {
        for ruim in [
            &b""[..],
            b"\0",
            b"relativo/x",
            b"/media/u/../../etc",
            b"/media/u/./x",
            b"/media/u/a\0b",
            b"/media/u/x/.",
            b"/media//u",
            b"/media/u/",
        ] {
            assert_eq!(montagem_valida(ruim.to_vec()), None, "{ruim:?}");
        }
        let longo = [b"/".as_slice(), &[b'a'; MAX_MONTAGEM]].concat();
        assert_eq!(montagem_valida(longo), None);
        assert_eq!(
            montagem_valida(b"/media/usuario/PEN\0".to_vec()),
            Some(PathBuf::from("/media/usuario/PEN"))
        );
    }

    #[test]
    fn rotulo_sem_controle_e_cortado() {
        assert_eq!(rotulo_limpo("PEN\u{1b}[2J\n"), "PEN[2J");
        let longo = "é".repeat(MAX_ROTULO);
        let r = rotulo_limpo(&longo);
        assert!(r.len() <= MAX_ROTULO && r.chars().all(|c| c == 'é'));
    }
}
