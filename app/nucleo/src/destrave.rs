//! Destravar pelo app (Fase 31; D16, D20, D62, D65 a).
//!
//! - [`SenhaMestra`]: a senha que vem da interface (não confiável). Zera ao sair, não
//!   tem `Clone`, `Display` nem `Serialize`, e o `Debug` não mostra o valor. A leitura
//!   (`Deserialize`) recusa vazio e acima de `ward_core::limites::MAX_SEGREDO` com uma
//!   mensagem fixa, sem ecoar o que veio. A senha só sai daqui para o pedido
//!   `destravar` do protocolo; nenhuma resposta a devolve e nenhum log a escreve.
//! - [`PedidoPendente`]: o `pedido_de_destrave` que o daemon mandou (um uso do agente
//!   espera o destrave, D16). O aviso pode chegar antes de a interface existir (o daemon
//!   abre o app por causa dele e o entrega assim que a conexão assina), então o lado
//!   Rust guarda o prazo e a tela o lê no `estado_do_cofre`. O aviso não tem campo (o
//!   protocolo não traz entrada nem comando): o que a tela mostra é a contagem de
//!   [`PRAZO_PEDIDO`], contada da chegada do aviso.
//! - [`vigiar`]: a thread que tira os `pedido_de_destrave` da fila de avisos da conexão
//!   (D65 a), marca o [`PedidoPendente`] e avisa a interface (no app, pelo canal do
//!   Tauri; na ponte de teste, a tela consulta o estado).

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde::de::{self, Deserialize, Deserializer, Visitor};
use ward_core::entrada::Segredo as SegredoCore;
use ward_core::limites::MAX_SEGREDO;
use zeroize::Zeroizing;

use crate::ErroApp;
use crate::daemon::FilaAvisos;

/// Quanto o pedido do agente espera o destrave (D16; o `PRAZO_DESTRAVE` do `wardd`).
pub const PRAZO_PEDIDO: Duration = Duration::from_secs(120);

/// Mensagem fixa da leitura recusada (não ecoa o valor).
const FORA_DO_FORMATO: &str = "senha mestra fora do formato";

/// Senha mestra digitada na tela de destravar.
pub struct SenhaMestra(Zeroizing<String>);

impl SenhaMestra {
    /// Confere o tamanho e toma posse do texto (sem cópia que fique sem zerar).
    pub fn nova(texto: String) -> Result<SenhaMestra, ErroApp> {
        let texto = Zeroizing::new(texto);
        if !texto.is_empty() && texto.len() <= MAX_SEGREDO {
            Ok(SenhaMestra(texto))
        } else {
            Err(ErroApp::PedidoInvalido)
        }
    }

    /// A senha no tipo do protocolo (também zera ao sair).
    pub(crate) fn para_o_protocolo(&self) -> SegredoCore {
        SegredoCore::de_bytes(self.0.as_bytes().to_vec())
    }
}

impl fmt::Debug for SenhaMestra {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SenhaMestra(••••)")
    }
}

impl<'de> Deserialize<'de> for SenhaMestra {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = SenhaMestra;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("senha mestra")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<SenhaMestra, E> {
                if v.len() > MAX_SEGREDO {
                    return Err(E::custom(FORA_DO_FORMATO));
                }
                self.visit_string(v.to_owned())
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<SenhaMestra, E> {
                SenhaMestra::nova(v).map_err(|_| E::custom(FORA_DO_FORMATO))
            }
            // Número, booleano ou nulo no lugar do texto: o erro padrão do serde
            // ecoaria o valor.
            fn visit_u64<E: de::Error>(self, _: u64) -> Result<SenhaMestra, E> {
                Err(E::custom(FORA_DO_FORMATO))
            }
            fn visit_i64<E: de::Error>(self, _: i64) -> Result<SenhaMestra, E> {
                Err(E::custom(FORA_DO_FORMATO))
            }
            fn visit_f64<E: de::Error>(self, _: f64) -> Result<SenhaMestra, E> {
                Err(E::custom(FORA_DO_FORMATO))
            }
            fn visit_bool<E: de::Error>(self, _: bool) -> Result<SenhaMestra, E> {
                Err(E::custom(FORA_DO_FORMATO))
            }
            fn visit_unit<E: de::Error>(self) -> Result<SenhaMestra, E> {
                Err(E::custom(FORA_DO_FORMATO))
            }
        }
        d.deserialize_any(V)
    }
}

/// O que a tela recebe sobre o pedido do agente: um número que muda a cada aviso (para
/// "Agora não" valer só para aquele) e os segundos que faltam. Sem entrada nem comando:
/// o aviso do protocolo não os traz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct InfoPedido {
    pub id: u64,
    pub restante: u64,
}

#[derive(Default)]
struct Estado {
    id: u64,
    prazo: Option<Instant>,
}

/// O pedido de destrave que está esperando, se há um.
#[derive(Default)]
pub struct PedidoPendente {
    estado: Mutex<Estado>,
}

impl PedidoPendente {
    fn travar(&self) -> MutexGuard<'_, Estado> {
        self.estado.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Chegou um `pedido_de_destrave` agora: o prazo recomeça (é o do pedido mais novo).
    pub fn marcar(&self) {
        self.marcar_em(Instant::now());
    }

    fn marcar_em(&self, agora: Instant) {
        let mut e = self.travar();
        e.id = e.id.wrapping_add(1);
        e.prazo = Some(agora + PRAZO_PEDIDO);
    }

    /// O pedido ainda dentro do prazo, ou `None`.
    pub fn info(&self) -> Option<InfoPedido> {
        self.info_em(Instant::now())
    }

    fn info_em(&self, agora: Instant) -> Option<InfoPedido> {
        let mut e = self.travar();
        let falta = e
            .prazo?
            .checked_duration_since(agora)
            .filter(|f| !f.is_zero());
        match falta {
            // Arredonda para cima: com 0,4 s ainda há pedido.
            Some(f) => Some(InfoPedido {
                id: e.id,
                restante: f.as_secs() + u64::from(f.subsec_nanos() > 0),
            }),
            None => {
                e.prazo = None;
                None
            }
        }
    }

    /// O cofre destravou: o pedido segue sozinho no daemon e a tela não o mostra mais.
    pub fn limpar(&self) {
        self.travar().prazo = None;
    }
}

/// Liga a thread que tira os `pedido_de_destrave` da `fila` (D65 a): cada um marca o
/// `pedido` e chama `ao_chegar` (que não recebe nada do aviso: ele não tem campo). Os
/// avisos de aprovação ficam na fila para a janela de aprovação (Fase 36).
pub fn vigiar(
    fila: Arc<FilaAvisos>,
    pedido: Arc<PedidoPendente>,
    ao_chegar: impl Fn() + Send + 'static,
) -> Result<(), ErroApp> {
    std::thread::Builder::new()
        .name("ward-app-destrave".into())
        .spawn(move || {
            loop {
                if fila.esperar_destrave(Duration::from_secs(3600)) {
                    pedido.marcar();
                    ao_chegar();
                }
            }
        })
        .map(|_| ())
        .map_err(|_| ErroApp::ConexaoPerdida)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::daemon::Aviso;

    #[test]
    fn ataque_senha_fora_do_formato_e_recusada_sem_eco() {
        let longa = format!("\"{}\"", "x".repeat(MAX_SEGREDO + 1));
        for corpo in [
            "\"\"",
            longa.as_str(),
            "135790",
            "-1",
            "1.5",
            "true",
            "null",
            "[\"falso-mestra-ward-01\"]",
            "{\"senha\":\"falso-mestra-ward-01\"}",
        ] {
            let e = serde_json::from_str::<SenhaMestra>(corpo).unwrap_err();
            let texto = e.to_string();
            assert!(!texto.contains("falso"), "{texto}");
            assert!(!texto.contains("135790"), "{texto}");
            assert!(!texto.contains("xxxx"), "{texto}");
        }
        let s: SenhaMestra = serde_json::from_str("\"falso-mestra-ward-01\"").unwrap();
        assert_eq!(s.para_o_protocolo().expor(), b"falso-mestra-ward-01");
        // Com acento e espaço: a senha é o que veio, sem normalizar.
        let s: SenhaMestra = serde_json::from_str("\" senha é\\u0000x \"").unwrap();
        assert_eq!(s.para_o_protocolo().expor(), " senha é\0x ".as_bytes());
    }

    #[test]
    fn ataque_debug_da_senha_nao_mostra_o_valor() {
        let s = SenhaMestra::nova("falso-mestra-ward-01".into()).unwrap();
        let d = format!("{s:?}");
        assert!(!d.contains("falso"), "{d}");
        assert_eq!(
            SenhaMestra::nova(String::new()).unwrap_err(),
            ErroApp::PedidoInvalido
        );
    }

    #[test]
    fn pedido_conta_de_120_s_recomeca_e_some() {
        let p = PedidoPendente::default();
        assert_eq!(p.info(), None);
        let t0 = Instant::now();
        p.marcar_em(t0);
        let i = p.info_em(t0).unwrap();
        assert_eq!(i.restante, 120);
        let i = p.info_em(t0 + Duration::from_millis(18_400)).unwrap();
        assert_eq!(i.restante, 102);
        assert_eq!(p.info_em(t0 + PRAZO_PEDIDO), None);
        // Expirado não volta.
        assert_eq!(p.info_em(t0), None);
        // Um aviso novo tem outro id e recomeça.
        p.marcar_em(t0 + Duration::from_secs(10));
        let j = p.info_em(t0 + Duration::from_secs(10)).unwrap();
        assert_ne!(j.id, i.id);
        assert_eq!(j.restante, 120);
        p.limpar();
        assert_eq!(p.info(), None);
    }

    #[test]
    fn vigia_tira_o_destrave_e_marca_o_pedido() {
        let fila = Arc::new(FilaAvisos::default());
        let pedido = Arc::new(PedidoPendente::default());
        let (tx, rx) = std::sync::mpsc::channel();
        vigiar(fila.clone(), pedido.clone(), move || {
            let _ = tx.send(());
        })
        .unwrap();
        fila.simular(Aviso::Destrave);
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(pedido.info().is_some());
        assert_eq!(fila.tamanho(), 0);
    }
}
