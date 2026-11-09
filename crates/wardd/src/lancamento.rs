//! O daemon abre o app (D57) pelo lançador do `ward-plataforma` (Fase 29a): `fork` +
//! `execve` do `ward-app` instalado, ambiente montado do zero, uma instância por vez.
//!
//! O app tem 0–2 em `/dev/null`, então a mensagem dele de "não abri" não aparece em
//! lugar nenhum. Por isso uma vigia colhe o filho e registra no log do daemon como ele
//! saiu (código ou sinal; pendência da 29a). Nada do pedido entra no ambiente do app:
//! a operação `abrir_app` não tem campo.

use std::os::unix::process::ExitStatusExt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ward_plataforma::{BinarioPermitido, ErroLancador, Instalacao, Lancador};

use crate::aprovacao::{AbridorApp, ErroAbrirApp};
use crate::aviso;

/// Caminho fixo do app instalado (D18).
pub const APP_INSTALADO: &str = "/usr/local/bin/ward-app";

/// De quanto em quanto tempo a vigia olha se o app saiu.
const PASSO_VIGIA: Duration = Duration::from_millis(250);

/// Abre o `ward-app` instalado pelo [`Lancador`].
pub struct AbridorReal {
    lancador: Arc<Mutex<Lancador>>,
}

impl AbridorReal {
    /// Com o binário instalado e os grupos da instalação.
    pub fn instalado(inst: &Instalacao) -> Option<Self> {
        let bin = BinarioPermitido::novo(APP_INSTALADO).ok()?;
        Some(Self {
            lancador: Arc::new(Mutex::new(Lancador::novo(bin, inst))),
        })
    }
}

/// Quem o lançador abriu (D63): o papel app só vale para esse processo, vivo.
#[derive(Clone)]
pub struct AppLancado(Arc<Mutex<Lancador>>);

impl AppLancado {
    /// `pid` é o app que o lançador abriu e que continua vivo?
    pub fn e_o_app(&self, pid: u32) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .e_o_app_lancado(pid)
    }
}

impl AbridorReal {
    /// Para a classificação dos pares (D63).
    pub fn app_lancado(&self) -> AppLancado {
        AppLancado(self.lancador.clone())
    }
}

/// A linha do log de como o app saiu (sem nada além de números).
pub fn descrever_saida(pid: u32, st: std::process::ExitStatus) -> String {
    match (st.code(), st.signal()) {
        (Some(c), _) => format!("app (pid {pid}) saiu com código {c}"),
        (None, Some(s)) => format!("app (pid {pid}) encerrado pelo sinal {s}"),
        _ => format!("app (pid {pid}) saiu"),
    }
}

impl AbridorApp for AbridorReal {
    fn abrir(&self, ao_sair: Box<dyn FnOnce() + Send>) -> Result<(), ErroAbrirApp> {
        let mut l = self.lancador.lock().unwrap_or_else(|e| e.into_inner());
        let pid = match l.abrir() {
            Ok(pid) => pid,
            Err(ErroLancador::JaAberto) => return Err(ErroAbrirApp::JaAberto),
            Err(e) => {
                // `ErroLancador` não leva caminho nem valor (só nomes de variável).
                aviso!("app não aberto: {e}");
                return Err(ErroAbrirApp::Indisponivel);
            }
        };
        drop(l);
        aviso!("app aberto (pid {pid})");
        let lancador = self.lancador.clone();
        let vigia = std::thread::Builder::new()
            .name("wardd-vigia-app".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(PASSO_VIGIA);
                    let mut l = lancador.lock().unwrap_or_else(|e| e.into_inner());
                    // Outro pid: este app já foi colhido por um `abrir` seguinte.
                    if !l.aberto() || l.pid() != Some(pid) {
                        match l.tomar_saida() {
                            Some(st) => aviso!("{}", descrever_saida(pid, st)),
                            None => aviso!("app (pid {pid}) saiu"),
                        }
                        break;
                    }
                }
                ao_sair();
            });
        if vigia.is_err() {
            aviso!("não deu para criar a vigia do app");
        }
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::process::Command;

    #[test]
    fn saida_vai_ao_log_so_com_numeros() {
        let st = Command::new("/usr/bin/false").status().unwrap();
        assert_eq!(descrever_saida(7, st), "app (pid 7) saiu com código 1");
        let mut c = Command::new("/usr/bin/yes")
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        c.kill().unwrap();
        let st = c.wait().unwrap();
        assert_eq!(descrever_saida(8, st), "app (pid 8) encerrado pelo sinal 9");
    }
}
