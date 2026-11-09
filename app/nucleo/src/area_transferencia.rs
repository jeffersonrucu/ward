//! "Copiar" no lado Rust do app (D4, S11, S12).
//!
//! O valor vem do daemon como [`Segredo`], vai direto para a área de transferência e
//! nunca passa pela webview. Depois de [`PRAZO_LIMPEZA`], a limpeza só apaga se a área
//! ainda contém exatamente o valor copiado (comparação em tempo constante): se o
//! usuário copiou outra coisa nesse meio-tempo, ela fica lá (V5.5).
//!
//! A área de transferência do Wayland é usada pelo `wl-copy`/`wl-paste` do sistema
//! (`/usr/bin`, dono root, sem escrita para grupo e outros), com o valor pela entrada
//! padrão, nunca em argv (S3), e ambiente limpo (só `WAYLAND_DISPLAY` e
//! `XDG_RUNTIME_DIR`). Ver "Problemas" no handoff da Fase 29 sobre o COSMIC.

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use ward_core::limites;
use zeroize::Zeroizing;

use crate::ErroApp;
use crate::segredo::Segredo;

/// S12: a área de transferência é limpa 30 s depois do "copiar".
pub const PRAZO_LIMPEZA: Duration = Duration::from_secs(30);

/// Onde o valor copiado fica. Os testes usam uma área falsa.
pub trait AreaTransferencia: Send + Sync + 'static {
    fn escrever(&self, valor: &[u8]) -> Result<(), ErroApp>;
    /// Conteúdo de texto atual (`None` se vazia ou sem texto), limitado a
    /// `MAX_SEGREDO + 1` bytes: mais que isso não pode ser o valor copiado.
    fn ler(&self) -> Result<Option<Zeroizing<Vec<u8>>>, ErroApp>;
    fn limpar(&self) -> Result<(), ErroApp>;
}

struct Pendente {
    geracao: u64,
    valor: Option<Zeroizing<Vec<u8>>>,
}

/// Copia e agenda a limpeza condicional.
pub struct Copiador<A: AreaTransferencia> {
    area: Arc<A>,
    pendente: Arc<Mutex<Pendente>>,
    prazo: Duration,
}

impl<A: AreaTransferencia> Copiador<A> {
    pub fn novo(area: A) -> Self {
        Self::com_prazo(area, PRAZO_LIMPEZA)
    }

    /// Prazo diferente, só para os testes.
    pub fn com_prazo(area: A, prazo: Duration) -> Self {
        Copiador {
            area: Arc::new(area),
            pendente: Arc::new(Mutex::new(Pendente {
                geracao: 0,
                valor: None,
            })),
            prazo,
        }
    }

    /// Escreve o valor e agenda a limpeza. Uma cópia nova substitui a anterior: o
    /// prazo da anterior deixa de valer e só o da nova limpa.
    pub fn copiar(&self, segredo: Segredo) -> Result<(), ErroApp> {
        if segredo.is_empty() || segredo.len() > limites::MAX_SEGREDO {
            return Err(ErroApp::PedidoInvalido);
        }
        let geracao = {
            let mut p = self
                .pendente
                .lock()
                .map_err(|_| ErroApp::AreaTransferencia)?;
            self.area.escrever(segredo.expor())?;
            p.geracao = p.geracao.wrapping_add(1);
            p.valor = Some(Zeroizing::new(segredo.expor().to_vec()));
            p.geracao
        };
        drop(segredo);
        let area = Arc::clone(&self.area);
        let pendente = Arc::clone(&self.pendente);
        let prazo = self.prazo;
        thread::Builder::new()
            .name("ward-limpeza".into())
            .spawn(move || {
                thread::sleep(prazo);
                limpar_se_ainda_for(&*area, &pendente, Some(geracao));
            })
            .map_err(|_| ErroApp::AreaTransferencia)?;
        Ok(())
    }

    /// Limpa já (ao travar ou ao fechar o app), com a mesma condição do prazo.
    pub fn limpar_agora(&self) {
        limpar_se_ainda_for(&*self.area, &self.pendente, None);
    }
}

fn limpar_se_ainda_for<A: AreaTransferencia>(
    area: &A,
    pendente: &Mutex<Pendente>,
    geracao: Option<u64>,
) {
    let Ok(mut p) = pendente.lock() else { return };
    if geracao.is_some_and(|g| g != p.geracao) {
        return; // uma cópia mais nova tem o próprio prazo
    }
    let Some(valor) = p.valor.take() else { return };
    if let Ok(Some(atual)) = area.ler()
        && iguais(&atual, &valor)
    {
        let _ = area.limpar();
    }
}

fn iguais(a: &[u8], b: &[u8]) -> bool {
    use ctutils::CtEq;
    a.ct_eq(b).to_bool()
}

/// `wl-copy`/`wl-paste` do sistema.
pub struct Wayland {
    _privado: (),
}

const WL_COPY: &str = "/usr/bin/wl-copy";
const WL_PASTE: &str = "/usr/bin/wl-paste";
const TIPO: &str = "text/plain;charset=utf-8";

impl Wayland {
    /// Confere os dois binários (dono root, arquivo comum, sem escrita para grupo e
    /// outros, sem link simbólico) antes de usar.
    pub fn novo() -> Result<Wayland, ErroApp> {
        for b in [WL_COPY, WL_PASTE] {
            binario_confiavel(Path::new(b))?;
        }
        Ok(Wayland { _privado: () })
    }

    fn comando(programa: &str) -> Command {
        let mut c = Command::new(programa);
        c.env_clear();
        for v in ["WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"] {
            if let Some(valor) = std::env::var_os(v) {
                c.env(v, valor);
            }
        }
        c.stderr(Stdio::null());
        c
    }
}

fn binario_confiavel(caminho: &Path) -> Result<(), ErroApp> {
    let md = fs::symlink_metadata(caminho).map_err(|_| ErroApp::AreaTransferencia)?;
    if !md.file_type().is_file() || md.uid() != 0 || md.permissions().mode() & 0o022 != 0 {
        return Err(ErroApp::AreaTransferencia);
    }
    Ok(())
}

impl AreaTransferencia for Wayland {
    fn escrever(&self, valor: &[u8]) -> Result<(), ErroApp> {
        let mut filho = Self::comando(WL_COPY)
            .args(["--type", TIPO])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .map_err(|_| ErroApp::AreaTransferencia)?;
        {
            let mut entrada = filho.stdin.take().ok_or(ErroApp::AreaTransferencia)?;
            entrada
                .write_all(valor)
                .map_err(|_| ErroApp::AreaTransferencia)?;
        }
        let estado = filho.wait().map_err(|_| ErroApp::AreaTransferencia)?;
        if estado.success() {
            Ok(())
        } else {
            Err(ErroApp::AreaTransferencia)
        }
    }

    fn ler(&self) -> Result<Option<Zeroizing<Vec<u8>>>, ErroApp> {
        let mut filho = Self::comando(WL_PASTE)
            .args(["--no-newline", "--type", "text/plain"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|_| ErroApp::AreaTransferencia)?;
        let mut saida = Zeroizing::new(Vec::new());
        let lido = filho
            .stdout
            .take()
            .ok_or(ErroApp::AreaTransferencia)?
            .take(limites::MAX_SEGREDO as u64 + 1)
            .read_to_end(&mut saida);
        // Mais que o limite: o resto fica no cano e o wl-paste é encerrado.
        let _ = filho.kill();
        let estado = filho.wait().map_err(|_| ErroApp::AreaTransferencia)?;
        if lido.is_err() || (!estado.success() && saida.is_empty()) {
            return Ok(None); // vazia ou sem texto
        }
        Ok(Some(saida))
    }

    fn limpar(&self) -> Result<(), ErroApp> {
        let estado = Self::comando(WL_COPY)
            .arg("--clear")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .map_err(|_| ErroApp::AreaTransferencia)?;
        if estado.success() {
            Ok(())
        } else {
            Err(ErroApp::AreaTransferencia)
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Área falsa: guarda o conteúdo e conta as limpezas.
    #[derive(Default)]
    pub(crate) struct AreaFalsa {
        pub conteudo: Mutex<Option<Vec<u8>>>,
        pub limpezas: Mutex<u32>,
    }

    impl AreaTransferencia for Arc<AreaFalsa> {
        fn escrever(&self, valor: &[u8]) -> Result<(), ErroApp> {
            *self.conteudo.lock().unwrap() = Some(valor.to_vec());
            Ok(())
        }
        fn ler(&self) -> Result<Option<Zeroizing<Vec<u8>>>, ErroApp> {
            Ok(self.conteudo.lock().unwrap().clone().map(Zeroizing::new))
        }
        fn limpar(&self) -> Result<(), ErroApp> {
            *self.conteudo.lock().unwrap() = None;
            *self.limpezas.lock().unwrap() += 1;
            Ok(())
        }
    }

    const CURTO: Duration = Duration::from_millis(40);
    const ESPERA: Duration = Duration::from_millis(600);

    #[test]
    fn prazo_e_30_segundos() {
        assert_eq!(PRAZO_LIMPEZA, Duration::from_secs(30));
    }

    #[test]
    fn copia_e_limpa_depois_do_prazo() {
        let area = Arc::new(AreaFalsa::default());
        let c = Copiador::com_prazo(Arc::clone(&area), CURTO);
        c.copiar(Segredo::novo(b"falso-valor-1".to_vec())).unwrap();
        assert_eq!(
            area.conteudo.lock().unwrap().as_deref(),
            Some(&b"falso-valor-1"[..])
        );
        thread::sleep(ESPERA);
        assert_eq!(*area.conteudo.lock().unwrap(), None);
        assert_eq!(*area.limpezas.lock().unwrap(), 1);
    }

    #[test]
    fn ataque_texto_do_usuario_copiado_depois_nao_e_apagado() {
        let area = Arc::new(AreaFalsa::default());
        let c = Copiador::com_prazo(Arc::clone(&area), CURTO);
        c.copiar(Segredo::novo(b"falso-valor-2".to_vec())).unwrap();
        *area.conteudo.lock().unwrap() = Some(b"texto do usuario".to_vec());
        thread::sleep(ESPERA);
        assert_eq!(
            area.conteudo.lock().unwrap().as_deref(),
            Some(&b"texto do usuario"[..])
        );
        assert_eq!(*area.limpezas.lock().unwrap(), 0);
    }

    #[test]
    fn ataque_prefixo_do_valor_nao_conta_como_igual() {
        let area = Arc::new(AreaFalsa::default());
        let c = Copiador::com_prazo(Arc::clone(&area), CURTO);
        c.copiar(Segredo::novo(b"falso-valor-3".to_vec())).unwrap();
        *area.conteudo.lock().unwrap() = Some(b"falso-valor".to_vec());
        thread::sleep(ESPERA);
        assert_eq!(*area.limpezas.lock().unwrap(), 0);
    }

    #[test]
    fn copia_nova_substitui_o_prazo_da_anterior() {
        let area = Arc::new(AreaFalsa::default());
        let c = Copiador::com_prazo(Arc::clone(&area), Duration::from_millis(400));
        c.copiar(Segredo::novo(b"falso-a".to_vec())).unwrap();
        thread::sleep(Duration::from_millis(200));
        c.copiar(Segredo::novo(b"falso-b".to_vec())).unwrap();
        // O prazo da primeira vence aqui e não pode apagar a segunda antes da hora.
        thread::sleep(Duration::from_millis(300));
        assert_eq!(
            area.conteudo.lock().unwrap().as_deref(),
            Some(&b"falso-b"[..])
        );
        thread::sleep(Duration::from_millis(600));
        assert_eq!(*area.conteudo.lock().unwrap(), None);
        assert_eq!(*area.limpezas.lock().unwrap(), 1);
    }

    #[test]
    fn limpar_agora_respeita_a_mesma_condicao() {
        let area = Arc::new(AreaFalsa::default());
        let c = Copiador::com_prazo(Arc::clone(&area), Duration::from_secs(60));
        c.copiar(Segredo::novo(b"falso-c".to_vec())).unwrap();
        c.limpar_agora();
        assert_eq!(*area.conteudo.lock().unwrap(), None);
        c.copiar(Segredo::novo(b"falso-d".to_vec())).unwrap();
        *area.conteudo.lock().unwrap() = Some(b"outro".to_vec());
        c.limpar_agora();
        assert_eq!(
            area.conteudo.lock().unwrap().as_deref(),
            Some(&b"outro"[..])
        );
    }

    #[test]
    fn ataque_valor_vazio_ou_grande_demais_e_recusado() {
        let area = Arc::new(AreaFalsa::default());
        let c = Copiador::com_prazo(Arc::clone(&area), CURTO);
        assert_eq!(
            c.copiar(Segredo::novo(Vec::new())),
            Err(ErroApp::PedidoInvalido)
        );
        assert_eq!(
            c.copiar(Segredo::novo(vec![b'a'; limites::MAX_SEGREDO + 1])),
            Err(ErroApp::PedidoInvalido)
        );
        assert_eq!(*area.conteudo.lock().unwrap(), None);
    }

    #[test]
    fn ataque_binario_fora_do_padrao_e_recusado() {
        let base = std::env::temp_dir().join(format!("ward-app-wl-{}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        let falso = base.join("wl-copy");
        fs::write(&falso, b"#!/bin/sh\n").unwrap();
        fs::set_permissions(&falso, fs::Permissions::from_mode(0o755)).unwrap();
        // Arquivo do usuário (não root) e link simbólico: os dois recusados.
        assert_eq!(binario_confiavel(&falso), Err(ErroApp::AreaTransferencia));
        let link = base.join("link");
        std::os::unix::fs::symlink("/usr/bin/env", &link).unwrap();
        assert_eq!(binario_confiavel(&link), Err(ErroApp::AreaTransferencia));
        fs::remove_dir_all(base).unwrap();
    }
}
