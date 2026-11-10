//! Configurações › backup externo em pendrive (Fase 48, V7.9; D17, D30).
//!
//! O daemon (Fase 46) tem `backup_externo`, `definir_backup_externo { ligado, uuid? }` e
//! `pendrives`, só para o papel app (V7.8). Aqui fica o que a tela recebe e o que ela
//! pode pedir:
//!
//! - [`PendriveVisto`]: UUID, rótulo e se está montado. O rótulo vem do udisks2 (o nome
//!   que qualquer um grava no pendrive), então é **não confiável**: o daemon já tira o
//!   caractere de controle e corta em `MAX_ROTULO`; aqui o invisível (direção de texto,
//!   largura zero, tags) vira escape visível por [`ward_core::display::neutralize`]
//!   (D51), e a tela o mostra como texto. Nenhum caminho de montagem chega à interface.
//! - [`BackupExternoVisto`]: ligado ou não, o UUID escolhido e, se ele estiver
//!   conectado, o pendrive visto.
//! - [`EscolhaDoPendrive`]: o que a interface manda. Ligar exige um UUID no formato de D45 **e**
//!   que seja um dos pendrives que o daemon vê naquela hora (o daemon é consultado de
//!   novo; um UUID forjado pela interface nem chega ao `definir_backup_externo`).
//!   Desligar não leva UUID: o lado Rust mantém o que estava escolhido.

use serde::Serialize;
use ward_core::configuracao::BackupExterno;
use ward_core::limits::MAX_BACKUP_UUID;
use ward_core::protocolo::Pendrive;

use crate::ErroApp;

/// Um pendrive para a tela: o rótulo já neutralizado (D51).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PendriveVisto {
    pub uuid: String,
    pub rotulo: String,
    pub montado: bool,
}

impl PendriveVisto {
    pub(crate) fn de(p: &Pendrive) -> PendriveVisto {
        PendriveVisto {
            uuid: p.uuid.clone(),
            rotulo: ward_core::display::neutralize(p.rotulo.trim()),
            montado: p.montado,
        }
    }
}

/// O que o cartão do backup externo mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BackupExternoVisto {
    pub ligado: bool,
    /// UUID do pendrive escolhido (o daemon o guarda mesmo desligado).
    pub uuid: Option<String>,
    /// O pendrive escolhido, se ele está conectado agora.
    pub pendrive: Option<PendriveVisto>,
    /// `false` se o udisks2 não respondeu (não dá para dizer se está conectado).
    pub pendrives_visiveis: bool,
}

/// Pedido da interface (não confiável): ligar com o pendrive escolhido, ou desligar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscolhaDoPendrive {
    Ligar(String),
    Desligar,
}

impl EscolhaDoPendrive {
    /// `ligado` e `uuid` como vieram da interface. Ligar sem UUID, UUID fora do formato
    /// de D45 ou desligar com UUID: [`ErroApp::PedidoInvalido`], antes do daemon.
    pub fn nova(ligado: bool, uuid: Option<&str>) -> Result<EscolhaDoPendrive, ErroApp> {
        match (ligado, uuid) {
            (true, Some(u)) if uuid_valido(u) => Ok(EscolhaDoPendrive::Ligar(u.to_owned())),
            (false, None) => Ok(EscolhaDoPendrive::Desligar),
            _ => Err(ErroApp::PedidoInvalido),
        }
    }
}

/// UUID no formato de D45 (`[0-9A-Fa-f-]`, 1 a 36, sem `-` nas pontas).
pub fn uuid_valido(uuid: &str) -> bool {
    uuid.len() <= MAX_BACKUP_UUID && BackupExterno::novo(true, Some(uuid)).is_ok()
}

/// O pendrive com aquele UUID na lista do daemon (sem diferenciar maiúsculas, como a
/// cópia do `wardd`). Dois com o mesmo UUID: nenhum (o `wardd` também não copia).
pub(crate) fn achar<'a>(lista: &'a [Pendrive], uuid: &str) -> Option<&'a Pendrive> {
    let mut iguais = lista.iter().filter(|p| p.uuid.eq_ignore_ascii_case(uuid));
    let primeiro = iguais.next()?;
    iguais.next().is_none().then_some(primeiro)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn p(uuid: &str, rotulo: &str) -> Pendrive {
        Pendrive {
            uuid: uuid.to_owned(),
            rotulo: rotulo.to_owned(),
            montado: true,
        }
    }

    #[test]
    fn escolha_valida() {
        assert_eq!(
            EscolhaDoPendrive::nova(true, Some("7C1E-42A9")),
            Ok(EscolhaDoPendrive::Ligar("7C1E-42A9".into()))
        );
        assert_eq!(
            EscolhaDoPendrive::nova(false, None),
            Ok(EscolhaDoPendrive::Desligar)
        );
    }

    #[test]
    fn ataque_uuid_forjado_fora_do_formato_e_recusado() {
        for ruim in [
            "",
            "-7C1E",
            "7C1E-",
            "../etc",
            "/media/usuario/X",
            "7C1E 42A9",
            "7C1E\n42A9",
            "7C1E-42A9\u{202E}",
            "７C1E",
            "g000",
            "<img src=x>",
        ] {
            assert_eq!(
                EscolhaDoPendrive::nova(true, Some(ruim)),
                Err(ErroApp::PedidoInvalido),
                "{ruim:?}"
            );
        }
        let longo = "A".repeat(MAX_BACKUP_UUID + 1);
        assert_eq!(
            EscolhaDoPendrive::nova(true, Some(&longo)),
            Err(ErroApp::PedidoInvalido)
        );
        // Ligar sem pendrive, e desligar trocando o UUID guardado.
        assert_eq!(
            EscolhaDoPendrive::nova(true, None),
            Err(ErroApp::PedidoInvalido)
        );
        assert_eq!(
            EscolhaDoPendrive::nova(false, Some("7C1E-42A9")),
            Err(ErroApp::PedidoInvalido)
        );
    }

    #[test]
    fn ataque_rotulo_com_direcao_e_largura_zero_vira_escape_visivel() {
        let v = PendriveVisto::de(&p(
            "7C1E-42A9",
            "<img src=x onerror=alert(1)>\u{202E}gpj.exe\u{200B}\u{2066}",
        ));
        assert!(
            !v.rotulo
                .chars()
                .any(|c| ward_core::display::invisible(c) || c.is_control()),
            "{:?}",
            v.rotulo
        );
        assert!(v.rotulo.contains("\\u{202E}"), "{}", v.rotulo);
        // O HTML continua texto: quem escapa é a tela (Svelte), e aqui nada é perdido.
        assert!(v.rotulo.starts_with("<img src=x onerror=alert(1)>"));
    }

    #[test]
    fn achar_sem_diferenciar_maiusculas_e_repetido_nao_vale() {
        let lista = [p("7C1E-42A9", "A"), p("3F2A-9B10", "B")];
        assert_eq!(
            achar(&lista, "7c1e-42a9").map(|p| p.rotulo.as_str()),
            Some("A")
        );
        assert!(achar(&lista, "0000-0000").is_none());
        let repetido = [p("7C1E-42A9", "A"), p("7c1e-42a9", "clone")];
        assert!(achar(&repetido, "7C1E-42A9").is_none());
    }

    /// Resposta forjada pelo "daemon" (o servidor é o atacante): rótulo com controle,
    /// caminho de montagem a mais, UUID fora do formato, pendrives demais. A leitura
    /// estrita recusa tudo; nada chega à tela.
    #[test]
    fn ataque_resposta_pendrives_forjada_e_recusada() {
        use zeroize::Zeroizing;

        use crate::cliente;
        use crate::daemon::Daemon;

        struct Resposta1(Vec<u8>);
        impl Daemon for Resposta1 {
            fn trocar(&mut self, _pedido: &[u8]) -> Result<Zeroizing<Vec<u8>>, ErroApp> {
                Ok(Zeroizing::new(self.0.clone()))
            }
        }
        let item = |corpo: &str| {
            format!(r#"{{"v":1,"tipo":"pendrives","dados":{{"itens":[{corpo}]}}}}"#).into_bytes()
        };
        let ok = item(r#"{"uuid":"7C1E-42A9","rotulo":"WARD","montado":true}"#);
        assert_eq!(cliente::pendrives(&mut Resposta1(ok)).unwrap().len(), 1);
        let muitos = (0..=ward_core::protocolo::MAX_PENDRIVES)
            .map(|i| format!(r#"{{"uuid":"{i:04X}","rotulo":"x","montado":true}}"#))
            .collect::<Vec<_>>()
            .join(",");
        for mau in [
            item(r#"{"uuid":"7C1E-42A9","rotulo":"WARD\u001b[2J","montado":true}"#),
            item(r#"{"uuid":"7C1E-42A9","rotulo":"A\nB","montado":true}"#),
            item(
                r#"{"uuid":"7C1E-42A9","rotulo":"WARD","montado":true,"montagem":"/media/u/WARD"}"#,
            ),
            item(r#"{"uuid":"../etc","rotulo":"WARD","montado":true}"#),
            item(&format!(
                r#"{{"uuid":"7C1E-42A9","rotulo":"{}","montado":true}}"#,
                "a".repeat(ward_core::protocolo::MAX_ROTULO + 1)
            )),
            item(&muitos),
        ] {
            assert_eq!(
                cliente::pendrives(&mut Resposta1(mau.clone())).unwrap_err(),
                ErroApp::RespostaInvalida,
                "{}",
                String::from_utf8_lossy(&mau)
            );
        }
    }

    mod pelo_servico {
        use std::sync::{Arc, Mutex};

        use zeroize::Zeroizing;

        use super::super::*;
        use crate::area_transferencia::{AreaTransferencia, Copiador};
        use crate::daemon::Daemon;
        use crate::daemon_falso::DaemonFalso;
        use crate::nivel::Pin;
        use crate::servico::Servico;

        type Quadros = Arc<Mutex<Vec<Vec<u8>>>>;

        struct SemArea;
        impl AreaTransferencia for SemArea {
            fn escrever(&self, _: &[u8]) -> Result<(), ErroApp> {
                Ok(())
            }
            fn ler(&self) -> Result<Option<Zeroizing<Vec<u8>>>, ErroApp> {
                Ok(None)
            }
            fn limpar(&self) -> Result<(), ErroApp> {
                Ok(())
            }
        }

        /// Daemon falso que guarda cada quadro recebido e deixa ver a configuração.
        #[derive(Clone)]
        struct Espiao(Arc<Mutex<DaemonFalso>>, Quadros);
        impl Daemon for Espiao {
            fn trocar(&mut self, pedido: &[u8]) -> Result<Zeroizing<Vec<u8>>, ErroApp> {
                self.1.lock().unwrap().push(pedido.to_vec());
                self.0.lock().unwrap().trocar(pedido)
            }
        }

        fn servico(falso: DaemonFalso) -> (Servico<Espiao, SemArea>, Espiao) {
            let e = Espiao(
                Arc::new(Mutex::new(falso.sem_espera())),
                Arc::new(Mutex::new(Vec::new())),
            );
            (Servico::novo(e.clone(), Copiador::novo(SemArea)), e)
        }

        fn definicoes(e: &Espiao) -> usize {
            e.1.lock()
                .unwrap()
                .iter()
                .filter(|q| String::from_utf8_lossy(q).contains("\"definir_backup_externo\""))
                .count()
        }

        fn sem_caminho(json: &str) {
            for agulha in ["/media", "/run", "montagem", "/dev/"] {
                assert!(!json.contains(agulha), "{agulha} em {json}");
            }
        }

        #[test]
        fn v7_9_escolher_ligar_e_desligar() {
            let (s, e) = servico(DaemonFalso::cheio());
            // Desligado por padrão (D17), sem pendrive escolhido.
            let b = s.backup_externo().unwrap();
            assert!(!b.ligado && b.uuid.is_none() && b.pendrive.is_none());
            assert!(!s.configuracoes().unwrap().backup_externo);
            let lista = s.listar_pendrives().unwrap();
            assert_eq!(lista.len(), 4);
            sem_caminho(&serde_json::to_string(&lista).unwrap());
            // Ligar com um pendrive da lista (em minúsculas: vai o UUID do daemon).
            let b = s.definir_backup_externo(true, Some("3f2a-9b10")).unwrap();
            assert!(b.ligado && b.pendrives_visiveis);
            assert_eq!(b.uuid.as_deref(), Some("3F2A-9B10"));
            let p = b.pendrive.as_ref().unwrap();
            assert_eq!((p.rotulo.as_str(), p.montado), ("FOTOS 2024", false));
            assert!(s.configuracoes().unwrap().backup_externo);
            assert_eq!(
                e.0.lock().unwrap().backup(),
                (true, Some("3F2A-9B10".into()))
            );
            sem_caminho(&serde_json::to_string(&b).unwrap());
            // Trocar de pendrive.
            let b = s.definir_backup_externo(true, Some("7C1E-42A9")).unwrap();
            assert_eq!(b.pendrive.unwrap().rotulo, "WARD-BACKUP");
            // Desligar mantém o UUID guardado; nada é apagado do pendrive (Fase 46).
            let b = s.definir_backup_externo(false, None).unwrap();
            assert!(!b.ligado);
            assert_eq!(b.uuid.as_deref(), Some("7C1E-42A9"));
            assert_eq!(
                e.0.lock().unwrap().backup(),
                (false, Some("7C1E-42A9".into()))
            );
            assert!(!s.configuracoes().unwrap().backup_externo);
            assert_eq!(definicoes(&e), 3);
        }

        #[test]
        fn ataque_uuid_forjado_pela_interface_nao_chega_ao_daemon() {
            let (s, e) = servico(DaemonFalso::cheio());
            // Fora do formato: nem um quadro.
            for ruim in [
                "../etc",
                "/media/usuario/WARD-BACKUP",
                "7C1E-42A9\u{202E}",
                "",
            ] {
                assert_eq!(
                    s.definir_backup_externo(true, Some(ruim)),
                    Err(ErroApp::PedidoInvalido),
                    "{ruim:?}"
                );
            }
            assert_eq!(
                s.definir_backup_externo(true, None),
                Err(ErroApp::PedidoInvalido)
            );
            assert_eq!(
                s.definir_backup_externo(false, Some("7C1E-42A9")),
                Err(ErroApp::PedidoInvalido)
            );
            assert!(e.1.lock().unwrap().is_empty());
            // No formato, mas não é de um pendrive conectado: o daemon só é consultado
            // (`pendrives`); o `definir_backup_externo` não sai.
            for forjado in ["DEAD-BEEF", "7C1E-42AA", "7C1E42A9"] {
                assert_eq!(
                    s.definir_backup_externo(true, Some(forjado)),
                    Err(ErroApp::PendriveNaoConectado),
                    "{forjado}"
                );
            }
            assert_eq!(definicoes(&e), 0);
            assert_eq!(e.0.lock().unwrap().backup(), (false, None));
        }

        #[test]
        fn ataque_pendrive_clonado_com_uuid_repetido_nao_e_escolhido() {
            let mut lista = crate::daemon_falso::pendrives_de_teste();
            let mut clone = lista[0].clone();
            clone.uuid = clone.uuid.to_lowercase();
            lista.push(clone);
            let (s, e) = servico(DaemonFalso::cheio().com_pendrives(Some(lista)));
            assert_eq!(
                s.definir_backup_externo(true, Some("7C1E-42A9")),
                Err(ErroApp::PendriveNaoConectado)
            );
            assert_eq!(definicoes(&e), 0);
        }

        #[test]
        fn sem_udisks_nao_liga_e_o_cartao_diz_que_nao_viu() {
            let (s, e) = servico(DaemonFalso::backup_externo_ligado().com_pendrives(None));
            assert_eq!(s.listar_pendrives(), Err(ErroApp::PendrivesIndisponiveis));
            let b = s.backup_externo().unwrap();
            assert!(b.ligado && !b.pendrives_visiveis && b.pendrive.is_none());
            assert_eq!(
                s.definir_backup_externo(true, Some("3F2A-9B10")),
                Err(ErroApp::PendrivesIndisponiveis)
            );
            assert_eq!(definicoes(&e), 0);
            // Desligar não depende do udisks2.
            assert!(!s.definir_backup_externo(false, None).unwrap().ligado);
        }

        #[test]
        fn ataque_rotulo_do_pendrive_chega_neutralizado() {
            let (s, _) = servico(DaemonFalso::cheio());
            let lista = s.listar_pendrives().unwrap();
            let estranho = lista.iter().find(|p| p.uuid.starts_with("a1b2")).unwrap();
            assert!(
                !estranho
                    .rotulo
                    .chars()
                    .any(|c| ward_core::display::invisible(c) || c.is_control()),
                "{:?}",
                estranho.rotulo
            );
            assert!(
                estranho.rotulo.contains("\\u{202E}") && estranho.rotulo.contains("\\u{200B}"),
                "{}",
                estranho.rotulo
            );
            let b = s
                .definir_backup_externo(true, Some(&estranho.uuid))
                .unwrap();
            assert_eq!(b.pendrive.unwrap().rotulo, estranho.rotulo);
        }

        #[test]
        fn travado_nao_le_nem_muda() {
            let (s, e) = servico(DaemonFalso::cheio());
            // Cinco PINs errados travam o falso (S9).
            for _ in 0..5 {
                let _ = s.mudar_nivel("acme-pg-dev", "livre", Pin::novo("0000".into()).unwrap());
            }
            assert_eq!(s.backup_externo(), Err(ErroApp::Travado));
            assert_eq!(s.listar_pendrives(), Err(ErroApp::Travado));
            assert_eq!(
                s.definir_backup_externo(true, Some("7C1E-42A9")),
                Err(ErroApp::Travado)
            );
            assert_eq!(definicoes(&e), 0);
        }
    }
}
