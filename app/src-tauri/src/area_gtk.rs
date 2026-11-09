//! "Copiar" pela área de transferência do GTK (Fase 39f; S2, S12, D4).
//!
//! O valor fica só na memória do app: o GTK o anuncia ao compositor e, quando alguém
//! cola, escreve no cano que o compositor entrega (`wl_data_source.send`). Nenhum
//! programa externo nem arquivo entra no caminho. O `wl-copy` que o app usava antes
//! guardava o valor em `/tmp/wl-copy-buffer-*/stdin`, no disco (auditoria do M5).
//!
//! - Tipos oferecidos: texto UTF-8 (`text/plain;charset=utf-8`, `UTF8_STRING`,
//!   `text/plain`) e `x-kde-passwordManagerHint` = `secret`, que pede aos gerenciadores
//!   de área de transferência para não guardar o valor no histórico.
//! - "Ainda é do Ward" é ser o dono da seleção: o GTK chama a limpeza do conteúdo
//!   (`clear_func`, aqui o `Drop` da [`Guarda`]) quando outro programa copia algo ou
//!   quando o app limpa. Enquanto o Ward for o dono, [`AreaGtk::ler`] devolve o valor
//!   que ele publicou; depois, `None`. [`AreaGtk::limpar`] só tira o conteúdo se o dono
//!   ainda for a mesma cópia: o texto que o usuário copiou depois nunca é apagado, sem
//!   a corrida entre ler e limpar que o `wl-paste`/`wl-copy --clear` tinha.
//! - GTK só na thread principal. `escrever` vem do comando (síncrono no Tauri, na
//!   thread principal); fora dela é recusado. `limpar` pode vir de outra thread (prazo
//!   de 30 s, trava vista pela bandeja, vigia da conexão): o pedido vai para a thread
//!   principal pelo contexto principal do GLib, sem esperar, e lá confere de novo se a
//!   cópia ainda é a mesma.
//! - O valor fica num `Zeroizing` compartilhado entre a área e o conteúdo do GTK, zerado
//!   quando os dois o soltam. O GTK copia o valor nos próprios buffers a cada colagem
//!   (fora do nosso controle; ver o handoff da Fase 39f).

use std::sync::{Arc, Mutex, MutexGuard};

use gtk::{gdk, glib};
use ward_app_nucleo::ErroApp;
use ward_app_nucleo::area_transferencia::{AreaTransferencia, Zeroizing};

/// Tipos de texto oferecidos (`info` 0).
const TIPOS_TEXTO: [&str; 3] = ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain"];
/// Aviso para gerenciadores de área de transferência (KDE Klipper e outros): não guardar.
const TIPO_DICA: &str = "x-kde-passwordManagerHint";
const DICA: &[u8] = b"secret";
const INFO_TEXTO: u32 = 0;
const INFO_DICA: u32 = 1;

type Valor = Arc<Zeroizing<Vec<u8>>>;

#[derive(Default)]
struct Dono {
    /// Contador de cópias (cada `escrever` ganha um número).
    ultima: u64,
    /// A cópia que o GTK está servindo agora.
    atual: Option<(u64, Valor)>,
}

/// A área do GTK. Só guarda estado do Ward; o GTK é tocado na thread principal.
#[derive(Default)]
pub struct AreaGtk {
    dono: Arc<Mutex<Dono>>,
}

impl AreaGtk {
    pub fn nova() -> AreaGtk {
        AreaGtk::default()
    }
}

fn travar(dono: &Mutex<Dono>) -> MutexGuard<'_, Dono> {
    dono.lock().unwrap_or_else(|e| e.into_inner())
}

/// Vai com o conteúdo entregue ao GTK. Quando o GTK solta o conteúdo (outro programa
/// copiou, o app limpou ou trocou a cópia), a cópia deixa de ser a atual.
struct Guarda {
    id: u64,
    dono: Arc<Mutex<Dono>>,
}

impl Drop for Guarda {
    fn drop(&mut self) {
        soltar(&self.dono, self.id);
    }
}

fn soltar(dono: &Mutex<Dono>, id: u64) {
    let mut d = travar(dono);
    if d.atual.as_ref().is_some_and(|(atual, _)| *atual == id) {
        d.atual = None;
    }
}

fn na_thread_principal() -> bool {
    gtk::is_initialized_main_thread()
}

impl AreaTransferencia for AreaGtk {
    fn escrever(&self, valor: &[u8]) -> Result<(), ErroApp> {
        if !na_thread_principal() {
            return Err(ErroApp::AreaTransferencia);
        }
        let valor: Valor = Arc::new(Zeroizing::new(valor.to_vec()));
        let id = {
            let mut d = travar(&self.dono);
            d.ultima = d.ultima.wrapping_add(1);
            d.ultima
        };
        let mut alvos: Vec<gtk::TargetEntry> = TIPOS_TEXTO
            .iter()
            .map(|t| gtk::TargetEntry::new(t, gtk::TargetFlags::empty(), INFO_TEXTO))
            .collect();
        alvos.push(gtk::TargetEntry::new(
            TIPO_DICA,
            gtk::TargetFlags::empty(),
            INFO_DICA,
        ));
        let guarda = Guarda {
            id,
            dono: Arc::clone(&self.dono),
        };
        let servido = Arc::clone(&valor);
        let area = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD);
        // O GTK solta a cópia anterior aqui dentro (a guarda dela tira a anterior de
        // `atual`); a trava do `dono` não está presa.
        let ok = area.set_with_data(&alvos, move |_, dados, info| {
            let _ = &guarda;
            let conteudo: &[u8] = if info == INFO_DICA { DICA } else { &servido };
            dados.set(&dados.target(), 8, conteudo);
        });
        if !ok {
            return Err(ErroApp::AreaTransferencia);
        }
        travar(&self.dono).atual = Some((id, valor));
        Ok(())
    }

    fn ler(&self) -> Result<Option<Zeroizing<Vec<u8>>>, ErroApp> {
        Ok(travar(&self.dono)
            .atual
            .as_ref()
            .map(|(_, v)| Zeroizing::new(v.to_vec())))
    }

    fn limpar(&self) -> Result<(), ErroApp> {
        let Some(id) = travar(&self.dono).atual.as_ref().map(|(id, _)| *id) else {
            return Ok(());
        };
        let dono = Arc::clone(&self.dono);
        let limpar = move || {
            // Fora da thread principal (o GLib roda aqui mesmo quando o laço não está
            // de pé, ex.: na saída): não toca no GTK.
            if !na_thread_principal() {
                return;
            }
            let ainda = travar(&dono)
                .atual
                .as_ref()
                .is_some_and(|(atual, _)| *atual == id);
            if ainda {
                // O GTK chama a limpeza do conteúdo (a guarda) aqui dentro.
                gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD).clear();
            }
        };
        if na_thread_principal() {
            limpar();
        } else {
            glib::MainContext::default().invoke(limpar);
        }
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn com_copia(area: &AreaGtk, id: u64, v: &[u8]) -> Guarda {
        let mut d = travar(&area.dono);
        d.ultima = id;
        d.atual = Some((id, Arc::new(Zeroizing::new(v.to_vec()))));
        Guarda {
            id,
            dono: Arc::clone(&area.dono),
        }
    }

    /// S12/V5.5: enquanto o Ward é o dono, `ler` devolve o valor copiado; quando o GTK
    /// solta o conteúdo (outro programa copiou), `ler` dá `None` e o copiador não limpa.
    #[test]
    fn ataque_texto_copiado_por_outro_programa_deixa_de_ser_do_ward() {
        let area = AreaGtk::nova();
        let guarda = com_copia(&area, 1, b"falso-39f-a");
        assert_eq!(
            area.ler().unwrap().as_deref().map(Vec::as_slice),
            Some(&b"falso-39f-a"[..])
        );
        drop(guarda);
        assert_eq!(area.ler().unwrap(), None);
        // Sem cópia atual, limpar não faz nada (nem toca no GTK).
        assert_eq!(area.limpar(), Ok(()));
    }

    /// A guarda de uma cópia antiga, solta depois da nova, não apaga a nova.
    #[test]
    fn ataque_guarda_antiga_nao_solta_a_copia_nova() {
        let area = AreaGtk::nova();
        let antiga = com_copia(&area, 1, b"falso-39f-b");
        let nova = com_copia(&area, 2, b"falso-39f-c");
        drop(antiga);
        assert_eq!(
            area.ler().unwrap().as_deref().map(Vec::as_slice),
            Some(&b"falso-39f-c"[..])
        );
        drop(nova);
        assert_eq!(area.ler().unwrap(), None);
    }

    /// Fora da thread principal do GTK o "copiar" é recusado e nada fica guardado.
    #[test]
    fn ataque_copiar_fora_da_thread_principal_e_recusado() {
        let area = AreaGtk::nova();
        assert_eq!(
            area.escrever(b"falso-39f-d"),
            Err(ErroApp::AreaTransferencia)
        );
        assert_eq!(area.ler().unwrap(), None);
    }

    /// `limpar` fora da thread principal com uma cópia atual: o pedido vai para o laço
    /// do GLib e não toca no GTK daqui (o GTK nem foi iniciado neste teste).
    #[test]
    fn limpar_fora_da_thread_principal_nao_toca_no_gtk() {
        let area = AreaGtk::nova();
        let guarda = com_copia(&area, 7, b"falso-39f-e");
        assert_eq!(area.limpar(), Ok(()));
        // Ninguém soltou a cópia: continua do Ward até o GTK soltar.
        assert!(area.ler().unwrap().is_some());
        drop(guarda);
    }

    /// Teste na sessão gráfica de verdade (Fase 39f): `make teste-area-transferencia`
    /// roda este teste sob `strace -f`, com o GTK iniciado aqui, e o guia pela entrada
    /// padrão (`dev/testes/area_transferencia.py`). Só valores falsos. Linhas:
    /// `prazo <ms>`, `copiar <valor>`, `copiar-fora <valor>`, `limpar-fora`, `sair`.
    /// Cada resposta sai numa linha `ward39f: …`.
    #[test]
    #[ignore = "precisa da sessão gráfica; rode por `make teste-area-transferencia`"]
    fn sessao_real() {
        use std::io::BufRead;
        use std::sync::mpsc;
        use std::time::Duration;
        use ward_app_nucleo::area_transferencia::Copiador;
        use ward_app_nucleo::segredo::Segredo;

        gtk::init().expect("GTK");
        let (tx, rx) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            for linha in std::io::stdin().lock().lines() {
                let Ok(l) = linha else { break };
                if tx.send(l).is_err() {
                    break;
                }
            }
        });
        let responder = |r: &str| println!("ward39f: {r}");
        let mut copiador: Option<Arc<Copiador<AreaGtk>>> = None;
        responder("pronto");
        loop {
            while gtk::events_pending() {
                gtk::main_iteration_do(false);
            }
            let Ok(linha) = rx.recv_timeout(Duration::from_millis(10)) else {
                continue;
            };
            let (cmd, arg) = linha.split_once(' ').unwrap_or((linha.as_str(), ""));
            match cmd {
                "prazo" => {
                    let ms: u64 = arg.parse().expect("prazo");
                    copiador = Some(Arc::new(Copiador::com_prazo(
                        AreaGtk::nova(),
                        Duration::from_millis(ms),
                    )));
                    responder("ok");
                }
                "copiar" => {
                    let c = copiador.as_ref().expect("prazo antes");
                    let r = c.copiar(Segredo::novo(arg.as_bytes().to_vec()));
                    responder(if r.is_ok() { "ok" } else { "erro" });
                }
                "copiar-fora" => {
                    let c = Arc::clone(copiador.as_ref().expect("prazo antes"));
                    let v = arg.as_bytes().to_vec();
                    let r = std::thread::spawn(move || c.copiar(Segredo::novo(v)))
                        .join()
                        .unwrap();
                    responder(if r.is_ok() { "ok" } else { "erro" });
                }
                "limpar-fora" => {
                    let c = Arc::clone(copiador.as_ref().expect("prazo antes"));
                    std::thread::spawn(move || c.limpar_agora()).join().unwrap();
                    responder("ok");
                }
                "sair" => break,
                _ => responder("desconhecido"),
            }
        }
        responder("fim");
    }
}
