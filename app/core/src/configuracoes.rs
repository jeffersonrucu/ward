//! Tela Configurações (Fase 38, V5.22; D30, D39, S9).
//!
//! - [`TrocaPin`]: PIN atual, PIN novo e a confirmação. O daemon confere o PIN atual
//!   (`trocar_pin`, a contagem de erros e a trava no 5.º são dele) e grava o hash
//!   Argon2id do novo na configuração do cofre, com geração nova (D30, D39).
//! - [`TrocaSenha`]: senha mestra atual, nova e a confirmação. A nova segue a mesma regra
//!   do primeiro uso ([`crate::primeiro_uso::erros_da_senha`]: 12 caracteres ou mais,
//!   até `MAX_SECRET` bytes); a atual só precisa existir e caber no limite (quem a
//!   confere é o daemon, `trocar_senha`).
//! - Tempo de trava: de `MIN_LOCK_MINUTES` a `MAX_LOCK_MINUTES` (D45), gravado na
//!   configuração do cofre pelo daemon (`tempo_trava`).
//! - [`Configuracoes`]: o que a tela mostra (tempo de trava e backup externo ligado ou
//!   não), lido pela operação `configuracao`; nunca o hash do PIN nem o UUID.
//!
//! Tudo o que é digitado chega como [`Digitado`] (zera ao sair, `Debug` sem valor) e é
//! conferido aqui antes de ir ao daemon. Erros por campo, com texto fixo, sem valor.

use serde::{Deserialize, Serialize};
use ward_core::limits::{MAX_LOCK_MINUTES, MAX_SECRET, MIN_LOCK_MINUTES};

use crate::AppError;
use crate::formulario::ErroCampo;
use crate::primeiro_uso::{Digitado, PIN_DIFERENTE, PIN_INVALIDO, erros_da_senha, pin_valido};

pub const PIN_ATUAL_INVALIDO: &str = "o PIN atual tem de 4 a 6 dígitos, só números";
pub const SENHA_ATUAL_VAZIA: &str = "digite a senha mestra atual";

/// O que a tela `Configurações` mostra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Configuracoes {
    pub minutos_trava: u32,
    pub backup_externo: bool,
    /// A pasta do backup local abre e é segura (Fase 49a); `false`: o cofre é gravado
    /// sem backup e a linha "Backup local" avisa.
    pub backup_local: bool,
}

/// Resposta das trocas de PIN e de senha: trocado, ou os erros por campo. Sem valor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResultadoTroca {
    pub trocado: bool,
    pub erros: Vec<ErroCampo>,
}

impl ResultadoTroca {
    pub(crate) fn com_erros(erros: Vec<ErroCampo>) -> Self {
        ResultadoTroca {
            trocado: false,
            erros,
        }
    }

    pub(crate) fn trocado() -> Self {
        ResultadoTroca {
            trocado: true,
            erros: Vec::new(),
        }
    }
}

fn erro(campo: &'static str, motivo: &str) -> ErroCampo {
    ErroCampo {
        campo,
        posicao: None,
        motivo: motivo.to_owned(),
    }
}

/// "Trocar PIN": o atual e o novo duas vezes. Campo a mais é recusado.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrocaPin {
    pub atual: Digitado,
    pub novo: Digitado,
    pub confirmacao: Digitado,
}

impl TrocaPin {
    /// Todos os erros por campo; vazio se pode ir ao daemon.
    pub fn erros(&self) -> Vec<ErroCampo> {
        let mut erros = Vec::new();
        if !pin_valido(&self.atual) {
            erros.push(erro("atual", PIN_ATUAL_INVALIDO));
        }
        if !pin_valido(&self.novo) {
            erros.push(erro("novo", PIN_INVALIDO));
        } else if !self.novo.igual(&self.confirmacao) {
            erros.push(erro("confirmacao", PIN_DIFERENTE));
        }
        erros
    }
}

/// "Trocar senha mestra": a atual e a nova duas vezes. Campo a mais é recusado.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrocaSenha {
    pub atual: Digitado,
    pub nova: Digitado,
    pub confirmacao: Digitado,
}

impl TrocaSenha {
    /// Todos os erros por campo; vazio se pode ir ao daemon. A nova segue a regra do
    /// primeiro uso (o mesmo texto de erro).
    pub fn erros(&self) -> Vec<ErroCampo> {
        let mut erros = Vec::new();
        let atual = self.atual.bytes();
        if atual.is_empty() || atual.len() > MAX_SECRET {
            erros.push(erro("atual", SENHA_ATUAL_VAZIA));
        }
        for mut e in erros_da_senha(&self.nova, &self.confirmacao) {
            if e.campo == "senha" {
                e.campo = "nova";
            }
            erros.push(e);
        }
        erros
    }
}

/// Minutos de trava vindos da interface (não confiável), na faixa de D45.
pub fn minutos_validos(minutos: u64) -> Result<u32, AppError> {
    u32::try_from(minutos)
        .ok()
        .filter(|m| (MIN_LOCK_MINUTES..=MAX_LOCK_MINUTES).contains(m))
        .ok_or(AppError::InvalidRequest)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn d(s: &str) -> Digitado {
        Digitado::novo(s.to_owned())
    }

    fn pin(atual: &str, novo: &str, conf: &str) -> TrocaPin {
        TrocaPin {
            atual: d(atual),
            novo: d(novo),
            confirmacao: d(conf),
        }
    }

    fn senha(atual: &str, nova: &str, conf: &str) -> TrocaSenha {
        TrocaSenha {
            atual: d(atual),
            nova: d(nova),
            confirmacao: d(conf),
        }
    }

    fn campos(e: Vec<ErroCampo>) -> Vec<&'static str> {
        e.iter().map(|e| e.campo).collect()
    }

    const SENHA: &str = "falso-mestra-ward-01";
    const NOVA: &str = "falso-nova-mestra-02";

    #[test]
    fn trocas_validas_passam() {
        assert!(pin("135790", "2468", "2468").erros().is_empty());
        assert!(senha(SENHA, NOVA, NOVA).erros().is_empty());
        // A atual não segue o mínimo de 12 (é conferida pelo daemon, não pela regra nova).
        assert!(senha("curta", NOVA, NOVA).erros().is_empty());
    }

    #[test]
    fn ataque_pin_atual_ou_novo_fora_do_formato_e_recusado() {
        for ruim in [
            "",
            "135",
            "1357901",
            "13579a",
            "１３５７",
            "13 57",
            "1357\n",
            "+1357",
        ] {
            assert_eq!(
                campos(pin(ruim, "2468", "2468").erros()),
                ["atual"],
                "{ruim:?}"
            );
            assert_eq!(
                campos(pin("135790", ruim, ruim).erros()),
                ["novo"],
                "{ruim:?}"
            );
        }
        assert_eq!(
            campos(pin("135790", "2468", "2469").erros()),
            ["confirmacao"]
        );
        assert_eq!(
            campos(pin("135790", "2468", "24680").erros()),
            ["confirmacao"]
        );
    }

    #[test]
    fn ataque_senha_nova_curta_ou_diferente_e_atual_vazia_recusadas() {
        assert_eq!(
            campos(senha(SENHA, "curta-11chr", "curta-11chr").erros()),
            ["nova"]
        );
        assert_eq!(campos(senha(SENHA, NOVA, SENHA).erros()), ["confirmacao"]);
        assert_eq!(campos(senha("", NOVA, NOVA).erros()), ["atual"]);
        let grande = "a".repeat(MAX_SECRET + 1);
        assert_eq!(campos(senha(&grande, NOVA, NOVA).erros()), ["atual"]);
        assert_eq!(campos(senha(SENHA, &grande, &grande).erros()), ["nova"]);
    }

    #[test]
    fn ataque_erros_e_debug_nao_mostram_valor() {
        let p = pin("135790", "13579a", "13579a");
        let s = senha(SENHA, NOVA, "falso-outra-mestra-03");
        let texto = format!("{p:?} {:?} {s:?} {:?}", p.erros(), s.erros());
        for agulha in ["falso", "mestra", "13579", "2468"] {
            assert!(!texto.contains(agulha), "{texto}");
        }
    }

    #[test]
    fn ataque_campo_a_mais_ou_tipo_errado_e_recusado() {
        let ok = r#"{"atual":"1","novo":"2","confirmacao":"2"}"#;
        assert!(serde_json::from_str::<TrocaPin>(ok).is_ok());
        for mau in [
            r#"{"atual":"1","novo":"2","confirmacao":"2","pin":"3"}"#,
            r#"{"atual":1357,"novo":"2","confirmacao":"2"}"#,
            r#"{"novo":"2","confirmacao":"2"}"#,
        ] {
            assert!(serde_json::from_str::<TrocaPin>(mau).is_err(), "{mau}");
        }
        let ok = r#"{"atual":"1","nova":"2","confirmacao":"2"}"#;
        assert!(serde_json::from_str::<TrocaSenha>(ok).is_ok());
        for mau in [
            r#"{"atual":"1","nova":"2","confirmacao":"2","geracao":0}"#,
            r#"{"atual":"1","nova":["2"],"confirmacao":"2"}"#,
            r#"{"atual":"1","confirmacao":"2"}"#,
        ] {
            assert!(serde_json::from_str::<TrocaSenha>(mau).is_err(), "{mau}");
        }
    }

    #[test]
    fn ataque_minutos_fora_da_faixa() {
        for m in [
            0,
            u64::from(MAX_LOCK_MINUTES) + 1,
            u64::from(u32::MAX) + 1,
            u64::MAX,
        ] {
            assert_eq!(minutos_validos(m), Err(AppError::InvalidRequest), "{m}");
        }
        assert_eq!(minutos_validos(1), Ok(1));
        assert_eq!(minutos_validos(240), Ok(240));
        assert_eq!(minutos_validos(1440), Ok(1440));
    }

    mod pelo_servico {
        use std::sync::{Arc, Mutex};

        use zeroize::Zeroizing;

        use super::*;
        use crate::area_transferencia::{AreaTransferencia, Copiador};
        use crate::daemon::{Daemon, VaultStatus};
        use crate::destrave::SenhaMestra;
        use crate::fake_daemon::{FakeDaemon, SECRETS};
        use crate::service::Service;

        type Quadros = Arc<Mutex<Vec<Vec<u8>>>>;

        struct SemArea;
        impl AreaTransferencia for SemArea {
            fn escrever(&self, _: &[u8]) -> Result<(), AppError> {
                Ok(())
            }
            fn ler(&self) -> Result<Option<Zeroizing<Vec<u8>>>, AppError> {
                Ok(None)
            }
            fn limpar(&self) -> Result<(), AppError> {
                Ok(())
            }
        }

        /// Daemon falso que guarda cada quadro recebido (o "daemon" é quem olha).
        struct Espiao(FakeDaemon, Quadros);
        impl Daemon for Espiao {
            fn exchange(&mut self, pedido: &[u8]) -> Result<Zeroizing<Vec<u8>>, AppError> {
                self.1.lock().unwrap().push(pedido.to_vec());
                self.0.exchange(pedido)
            }
        }

        fn servico() -> (Service<Espiao, SemArea>, Quadros) {
            let quadros = Arc::new(Mutex::new(Vec::new()));
            let d = Espiao(FakeDaemon::full().without_wait(), Arc::clone(&quadros));
            (Service::new(d, Copiador::novo(SemArea)), quadros)
        }

        fn sem_valor(json: &str) {
            for agulha in ["135790", "2468", "falso", "argon2", "$m="]
                .iter()
                .copied()
                .chain(SECRETS.iter().map(|s| &s[..s.len().min(12)]))
            {
                assert!(!json.contains(agulha), "{agulha} em {json}");
            }
        }

        fn senha_mestra(s: &str) -> SenhaMestra {
            SenhaMestra::nova(s.to_owned()).unwrap()
        }

        #[test]
        fn v5_22_padrao_4h_desligado_e_tempo_alteravel() {
            let (s, _) = servico();
            let c = s.settings().unwrap();
            assert_eq!((c.minutos_trava, c.backup_externo), (240, false));
            let c = s.set_lock_timeout(30).unwrap();
            assert_eq!(c.minutos_trava, 30);
            assert_eq!(s.settings().unwrap().minutos_trava, 30);
            sem_valor(&serde_json::to_string(&c).unwrap());
        }

        #[test]
        fn ataque_tempo_fora_da_faixa_nem_chega_ao_daemon() {
            let (s, quadros) = servico();
            for m in [0, 1441, u64::MAX] {
                assert_eq!(s.set_lock_timeout(m), Err(AppError::InvalidRequest));
            }
            assert!(quadros.lock().unwrap().is_empty());
        }

        #[test]
        fn v5_22_trocar_pin_pede_o_pin_atual() {
            let (s, quadros) = servico();
            assert_eq!(
                s.change_pin(pin("246801", "2468", "2468")),
                Err(AppError::WrongPin)
            );
            let r = s.change_pin(pin("135790", "2468", "2468")).unwrap();
            assert!(r.trocado && r.erros.is_empty());
            sem_valor(&serde_json::to_string(&r).unwrap());
            // Daí só o PIN novo aprova.
            let mudar = |p: &str| {
                s.set_level(
                    "acme-pg-dev",
                    "bloqueado",
                    crate::nivel::Pin::novo(p.into()).unwrap(),
                )
            };
            assert_eq!(mudar("135790"), Err(AppError::WrongPin));
            assert!(mudar("2468").is_ok());
            // O quadro leva só `atual` e `novo` (a confirmação fica no app).
            let q = quadros.lock().unwrap();
            let trocas: Vec<_> = q
                .iter()
                .map(|q| String::from_utf8_lossy(q).into_owned())
                .filter(|q| q.contains("\"trocar_pin\""))
                .collect();
            assert_eq!(trocas.len(), 2);
            assert!(trocas.iter().all(|t| !t.contains("confirmacao")));
        }

        #[test]
        fn ataque_pin_ou_senha_fora_da_regra_nao_chegam_ao_daemon() {
            let (s, quadros) = servico();
            for t in [
                pin("135", "2468", "2468"),
                pin("135790", "13579a", "13579a"),
                pin("135790", "2468", "2469"),
            ] {
                let r = s.change_pin(t).unwrap();
                assert!(!r.trocado && !r.erros.is_empty());
            }
            for t in [
                senha("", NOVA, NOVA),
                senha(SENHA, "curta", "curta"),
                senha(SENHA, NOVA, "falso-outra-mestra-03"),
            ] {
                let r = s.change_master_password(t).unwrap();
                assert!(!r.trocado && !r.erros.is_empty());
                sem_valor(&serde_json::to_string(&r).unwrap());
            }
            assert!(quadros.lock().unwrap().is_empty());
        }

        #[test]
        fn ataque_adivinhar_o_pin_pela_troca_trava_no_quinto_erro() {
            let (s, _) = servico();
            for i in 0..5 {
                assert_eq!(
                    s.change_pin(pin(&format!("00{i:02}"), "2468", "2468")),
                    Err(AppError::WrongPin)
                );
            }
            assert_eq!(s.vault_status(), Ok(VaultStatus::Locked));
            assert_eq!(
                s.change_pin(pin("135790", "2468", "2468")),
                Err(AppError::Locked)
            );
            assert_eq!(s.settings(), Err(AppError::Locked));
        }

        #[test]
        fn v5_22_trocar_senha_pede_a_atual_e_a_nova_destrava() {
            let (s, _) = servico();
            assert_eq!(
                s.change_master_password(senha("falso-errada-mestra-09", NOVA, NOVA)),
                Err(AppError::WrongPassword)
            );
            assert_eq!(s.vault_status(), Ok(VaultStatus::Unlocked));
            let r = s.change_master_password(senha(SENHA, NOVA, NOVA)).unwrap();
            assert!(r.trocado);
            // Trava (5 PINs errados) e confere qual senha destrava.
            for _ in 0..5 {
                let _ = s.change_pin(pin("000000", "2468", "2468"));
            }
            assert_eq!(s.vault_status(), Ok(VaultStatus::Locked));
            assert_eq!(s.unlock(senha_mestra(SENHA)), Err(AppError::WrongPassword));
            assert!(s.unlock(senha_mestra(NOVA)).is_ok());
        }
    }
}
