//! Destravar pelo app (Fase 31; D16, D20, D62, D65 a).
//!
//! - [`SenhaMestra`]: a senha que vem da interface (não confiável). Zera ao sair, não
//!   tem `Clone`, `Display` nem `Serialize`, e o `Debug` não mostra o valor. A leitura
//!   (`Deserialize`) recusa vazio e acima de `ward_core::limits::MAX_SECRET` com uma
//!   mensagem fixa, sem ecoar o que veio. A senha só sai daqui para o pedido
//!   `destravar` do protocolo; nenhuma resposta a devolve e nenhum log a escreve.
//! - [`PedidoPendente`]: o `pedido_de_destrave` que o daemon mandou (um uso do agente
//!   espera o destrave, D16). O aviso pode chegar antes de a interface existir (o daemon
//!   abre o app por causa dele e o entrega assim que a conexão assina), então o lado
//!   Rust guarda o prazo e a tela o lê no `estado_do_cofre`. Fase 39a: o aviso traz a
//!   entrada que o agente pediu, a operação, o comando exato e os segundos que faltam
//!   pelo relógio do daemon (no máximo [`PRAZO_PEDIDO`]). Entrada e comando vêm do
//!   agente: o daemon já os formata com escape visível (D51) e aqui passam de novo por
//!   `neutralize` antes de ir à tela, que os mostra como texto, nunca como HTML.
//! - [`vigiar`]: a thread que tira os `pedido_de_destrave` da fila de avisos da conexão
//!   (D65 a), marca o [`PedidoPendente`] e avisa a interface (no app, pelo canal do
//!   Tauri; na ponte de teste, a tela consulta o estado).

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde::de::{self, Deserialize, Deserializer, Visitor};
use ward_core::display::neutralize;
use ward_core::entry::Secret as SegredoCore;
use ward_core::limits::MAX_SECRET;
use ward_core::protocolo::PedidoDeDestrave;
use zeroize::Zeroizing;

use crate::AppError;
use crate::daemon::NoticeQueue;

/// Quanto o pedido do agente espera o destrave (D16; o `PRAZO_DESTRAVE` do `wardd`).
pub const PRAZO_PEDIDO: Duration = Duration::from_secs(120);

/// Mensagem fixa da leitura recusada (não ecoa o valor).
const FORA_DO_FORMATO: &str = "senha mestra fora do formato";

/// Senha mestra digitada na tela de destravar.
pub struct SenhaMestra(Zeroizing<String>);

impl SenhaMestra {
    /// Confere o tamanho e toma posse do texto (sem cópia que fique sem zerar).
    pub fn nova(texto: String) -> Result<SenhaMestra, AppError> {
        let texto = Zeroizing::new(texto);
        if !texto.is_empty() && texto.len() <= MAX_SECRET {
            Ok(SenhaMestra(texto))
        } else {
            Err(AppError::InvalidRequest)
        }
    }

    /// A senha no tipo do protocolo (também zera ao sair).
    pub(crate) fn para_o_protocolo(&self) -> SegredoCore {
        SegredoCore::from_bytes(self.0.as_bytes().to_vec())
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
                if v.len() > MAX_SECRET {
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
/// "Agora não" valer só para aquele), os segundos que faltam e, com escape visível
/// (D51), a entrada pedida, a operação e o comando exato (Fase 39a). Sem tipo nem
/// ambiente: com o cofre travado o daemon não os sabe. Nenhum segredo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InfoPedido {
    pub id: u64,
    pub restante: u64,
    pub entrada: String,
    pub operacao: &'static str,
    pub comando: String,
}

/// O texto do pedido como a tela o mostra (D51).
#[derive(Default)]
struct Texto {
    entrada: String,
    operacao: &'static str,
    comando: String,
}

#[derive(Default)]
struct Estado {
    id: u64,
    prazo: Option<Instant>,
    texto: Texto,
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

    /// Chegou um `pedido_de_destrave` agora: o pedido mostrado passa a ser este (o mais
    /// novo), com o prazo que o daemon mandou (nunca acima de [`PRAZO_PEDIDO`]).
    pub fn marcar(&self, p: &PedidoDeDestrave) {
        self.marcar_em(p, Instant::now());
    }

    fn marcar_em(&self, p: &PedidoDeDestrave, agora: Instant) {
        let texto = Texto {
            entrada: neutralize(&p.entrada),
            operacao: p.operacao.nome(),
            comando: neutralize(&p.comando),
        };
        let falta = Duration::from_secs(p.restante).min(PRAZO_PEDIDO);
        let mut e = self.travar();
        e.id = e.id.wrapping_add(1);
        e.prazo = Some(agora + falta);
        e.texto = texto;
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
                entrada: e.texto.entrada.clone(),
                operacao: e.texto.operacao,
                comando: e.texto.comando.clone(),
            }),
            None => {
                e.prazo = None;
                e.texto = Texto::default();
                None
            }
        }
    }

    /// O cofre destravou: o pedido segue sozinho no daemon e a tela não o mostra mais.
    pub fn limpar(&self) {
        let mut e = self.travar();
        e.prazo = None;
        e.texto = Texto::default();
    }
}

/// Liga a thread que tira os `pedido_de_destrave` da `fila` (D65 a): o mais novo marca o
/// `pedido` e chama `ao_chegar` (que não recebe nada do aviso: a tela lê o pedido no
/// `estado_do_cofre`). Os avisos de aprovação ficam na fila para a janela de aprovação
/// (Fase 36).
pub fn vigiar(
    fila: Arc<NoticeQueue>,
    pedido: Arc<PedidoPendente>,
    ao_chegar: impl Fn() + Send + 'static,
) -> Result<(), AppError> {
    std::thread::Builder::new()
        .name("ward-app-destrave".into())
        .spawn(move || {
            loop {
                if let Some(p) = fila.wait_unlock(Duration::from_secs(3600)) {
                    pedido.marcar(&p);
                    ao_chegar();
                }
            }
        })
        .map(|_| ())
        .map_err(|_| AppError::ConnectionLost)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::daemon::Notice;

    #[test]
    fn ataque_senha_fora_do_formato_e_recusada_sem_eco() {
        let longa = format!("\"{}\"", "x".repeat(MAX_SECRET + 1));
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
        assert_eq!(s.para_o_protocolo().expose(), b"falso-mestra-ward-01");
        // Com acento e espaço: a senha é o que veio, sem normalizar.
        let s: SenhaMestra = serde_json::from_str("\" senha é\\u0000x \"").unwrap();
        assert_eq!(s.para_o_protocolo().expose(), " senha é\0x ".as_bytes());
    }

    #[test]
    fn ataque_debug_da_senha_nao_mostra_o_valor() {
        let s = SenhaMestra::nova("falso-mestra-ward-01".into()).unwrap();
        let d = format!("{s:?}");
        assert!(!d.contains("falso"), "{d}");
        assert_eq!(
            SenhaMestra::nova(String::new()).unwrap_err(),
            AppError::InvalidRequest
        );
    }

    fn aviso(entrada: &str, restante: u64) -> PedidoDeDestrave {
        match Notice::test_unlock(entrada) {
            Notice::Unlock(mut p) => {
                p.restante = restante;
                p
            }
            Notice::Approval(_) => unreachable!(),
        }
    }

    #[test]
    fn pedido_conta_de_120_s_recomeca_e_some() {
        let p = PedidoPendente::default();
        assert_eq!(p.info(), None);
        let t0 = Instant::now();
        p.marcar_em(&aviso("acme-pg-aprovacao", 120), t0);
        let i = p.info_em(t0).unwrap();
        assert_eq!(i.restante, 120);
        let i = p.info_em(t0 + Duration::from_millis(18_400)).unwrap();
        assert_eq!(i.restante, 102);
        assert_eq!(p.info_em(t0 + PRAZO_PEDIDO), None);
        // Expirado não volta.
        assert_eq!(p.info_em(t0), None);
        // Um aviso novo tem outro id e recomeça.
        p.marcar_em(
            &aviso("acme-pg-aprovacao", 120),
            t0 + Duration::from_secs(10),
        );
        let j = p.info_em(t0 + Duration::from_secs(10)).unwrap();
        assert_ne!(j.id, i.id);
        assert_eq!(j.restante, 120);
        p.limpar();
        assert_eq!(p.info(), None);
    }

    #[test]
    fn vigia_tira_o_destrave_e_marca_o_pedido() {
        let fila = Arc::new(NoticeQueue::default());
        let pedido = Arc::new(PedidoPendente::default());
        let (tx, rx) = std::sync::mpsc::channel();
        vigiar(fila.clone(), pedido.clone(), move || {
            let _ = tx.send(());
        })
        .unwrap();
        fila.simulate(Notice::test_unlock("acme-pg-aprovacao"));
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let i = pedido.info().unwrap();
        assert_eq!(i.entrada, "acme-pg-aprovacao");
        assert_eq!(i.operacao, "executar");
        assert_eq!(i.comando, r#""psql" "-c" "select count(*) from pedidos""#);
        assert_eq!(fila.size(), 0);
    }

    /// Fase 39a (V5.8): o prazo é o que o daemon mandou (o app que assinou depois não
    /// mostra 2 min cheios), nunca acima de 2 min; o pedido mais novo toma o lugar.
    #[test]
    fn prazo_vem_do_daemon_e_nunca_passa_de_2_min() {
        let p = PedidoPendente::default();
        let t0 = Instant::now();
        p.marcar_em(&aviso("acme-pg-aprovacao", 70), t0);
        assert_eq!(p.info_em(t0).unwrap().restante, 70);
        assert_eq!(p.info_em(t0 + Duration::from_secs(70)), None);
        p.marcar_em(&aviso("acme-pg-aprovacao", u64::MAX), t0);
        assert_eq!(p.info_em(t0).unwrap().restante, 120);
        p.marcar_em(&aviso("acme-ssh-prod", 30), t0);
        assert_eq!(p.info_em(t0).unwrap().entrada, "acme-ssh-prod");
        p.limpar();
        assert_eq!(p.info_em(t0), None);
    }

    /// Fase 39a (D51): ataque com entrada e comando que trazem controle, bidi, largura
    /// zero e tag Unicode crus (um daemon que não formatou, ou um quadro forjado): nada
    /// invisível chega à tela; o que já era escape passa igual.
    #[test]
    fn ataque_entrada_e_comando_crus_chegam_neutralizados() {
        let p = PedidoPendente::default();
        let mut a = aviso("acme\u{202E}gp-pg\u{1b}[2K\u{200B}\u{E0041}\n", 120);
        a.comando = "\"psql\" \"-c\" \"x\u{2066}\u{feff}\u{9b}\u{7f}\"".into();
        p.marcar(&a);
        let i = p.info().unwrap();
        for t in [&i.entrada, &i.comando] {
            assert!(!t.chars().any(ward_core::display::invisible), "{t:?}");
        }
        assert_eq!(i.entrada, r"acme\u{202E}gp-pg\x1b[2K\u{200B}\u{E0041}\n");
        assert_eq!(i.comando, r#""psql" "-c" "x\u{2066}\u{FEFF}\x9b\x7f""#);
        // A forma já neutralizada pelo daemon passa igual (sem escape em dobro).
        let mut b = aviso(r"acme\u{202E}", 120);
        b.comando = r#""a\x1b""#.into();
        p.marcar(&b);
        let j = p.info().unwrap();
        assert_eq!(
            (j.entrada.as_str(), j.comando.as_str()),
            (r"acme\u{202E}", r#""a\x1b""#)
        );
        // Serializado para a tela: só os campos conhecidos.
        let v = serde_json::to_value(&j).unwrap();
        let mut chaves: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
        chaves.sort();
        assert_eq!(chaves, ["comando", "entrada", "id", "operacao", "restante"]);
    }
}
