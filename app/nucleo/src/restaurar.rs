//! Restaurar backup (Fase 47, V7.7; D17, D39, S9).
//!
//! - [`VersaoVista`]: uma versão do backup local como a tela a mostra: o nome exato do
//!   arquivo (`ward-AAAAMMDD-HHMMSS[-N].ward`, o mesmo que volta no pedido) e o instante
//!   em segundos UTC, tirado do próprio nome (a tela mostra no fuso local). Nada do
//!   conteúdo: o protocolo (`backups`, Fase 45) só traz os nomes.
//! - [`Restauracao`]: o que a tela manda ao confirmar: a versão, a senha mestra daquela
//!   versão ([`SenhaMestra`], zera ao sair) e o PIN atual ([`Pin`], zera ao sair; o
//!   daemon exige o PIN do cofre de agora, porque a versão volta com outros níveis e
//!   outro PIN, S9). Campo a mais é recusado na leitura.
//! - [`Restaurado`]: a resposta: restaurado e quantas entradas a versão tem (lido de
//!   novo do daemon). Sem valor, sem caminho, sem geração.
//!
//! A versão vinda da interface não é confiável: o formato é conferido aqui
//! ([`ward_core::backup::nome_valido`]) e o serviço só pede o `restaurar_backup` se o
//! nome estiver na lista que o daemon acabou de dar.

use serde::{Deserialize, Serialize};

use crate::ErroApp;
use crate::destrave::SenhaMestra;
use crate::nivel::Pin;

/// Uma versão do backup local, sem conteúdo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VersaoVista {
    /// O nome exato do arquivo, como o daemon deu.
    pub nome: String,
    /// Segundos desde 1970 em UTC, do nome (data e hora; o sufixo `-N` não muda).
    pub instante: i64,
}

impl VersaoVista {
    /// Lê um nome que veio do daemon. Fora do formato estrito de D17 é `None` (a leitura
    /// estrita do protocolo já recusou; aqui é a segunda camada).
    pub fn de_nome(nome: &str) -> Option<VersaoVista> {
        if !nome_ok(nome) {
            return None;
        }
        let b = nome.as_bytes();
        let num = |de: usize, ate: usize| -> Option<i64> {
            std::str::from_utf8(&b[de..ate]).ok()?.parse::<i64>().ok()
        };
        // `ward-AAAAMMDD-HHMMSS…`: data em 5..13, hora em 14..20.
        let (a, m, d) = (num(5, 9)?, num(9, 11)?, num(11, 13)?);
        let (h, mi, s) = (num(14, 16)?, num(16, 18)?, num(18, 20)?);
        let instante = dias_de_civil(a, m, d) * 86_400 + h * 3600 + mi * 60 + s;
        Some(VersaoVista {
            nome: nome.to_owned(),
            instante,
        })
    }
}

/// Dias desde 1970-01-01 de uma data civil (algoritmo de Howard Hinnant, só inteiros).
fn dias_de_civil(a: i64, m: i64, d: i64) -> i64 {
    let a = if m <= 2 { a - 1 } else { a };
    let era = a.div_euclid(400);
    let ano_da_era = a - era * 400;
    let mp = (m + 9) % 12;
    let dia_do_ano = (153 * mp + 2) / 5 + d - 1;
    let dia_da_era = ano_da_era * 365 + ano_da_era / 4 - ano_da_era / 100 + dia_do_ano;
    era * 146_097 + dia_da_era - 719_468
}

#[cfg(unix)]
fn nome_ok(nome: &str) -> bool {
    ward_core::backup::nome_valido(nome)
}

#[cfg(not(unix))]
fn nome_ok(_nome: &str) -> bool {
    false
}

/// Os nomes da resposta `backups` viram versões; um nome fora do formato faz a resposta
/// inteira ser recusada (o daemon não manda isso).
pub fn versoes_de(nomes: &[String]) -> Result<Vec<VersaoVista>, ErroApp> {
    nomes
        .iter()
        .map(|n| VersaoVista::de_nome(n).ok_or(ErroApp::RespostaInvalida))
        .collect()
}

/// A versão escolhida na interface: o nome tem de estar no formato. Não confere a lista
/// (isso é do serviço, com o daemon).
pub fn versao_pedida(nome: &str) -> Result<&str, ErroApp> {
    if nome_ok(nome) {
        Ok(nome)
    } else {
        Err(ErroApp::PedidoInvalido)
    }
}

/// "Restaurar esta versão": a versão, a senha mestra dela e o PIN atual.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Restauracao {
    pub versao: String,
    pub senha: SenhaMestra,
    pub pin: Pin,
}

/// Resposta do `restaurar_backup`: sem valor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Restaurado {
    pub restaurado: bool,
    /// Quantas entradas o cofre tem depois da troca (as da versão).
    pub entradas: usize,
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nome_vira_instante_utc() {
        let v = VersaoVista::de_nome("ward-19700101-000000.ward").unwrap();
        assert_eq!(v.instante, 0);
        // 2026-11-08 14:32:00 UTC.
        let v = VersaoVista::de_nome("ward-20261108-143200.ward").unwrap();
        assert_eq!(v.instante, 1_794_148_320);
        // O sufixo não muda o instante.
        let s = VersaoVista::de_nome("ward-20261108-143200-2.ward").unwrap();
        assert_eq!(s.instante, v.instante);
        assert_eq!(s.nome, "ward-20261108-143200-2.ward");
        // Bissexto e virada de ano.
        assert_eq!(
            VersaoVista::de_nome("ward-20240229-235959.ward")
                .unwrap()
                .instante,
            1_709_251_199
        );
        assert_eq!(
            VersaoVista::de_nome("ward-20251231-235959.ward")
                .unwrap()
                .instante
                + 1,
            VersaoVista::de_nome("ward-20260101-000000.ward")
                .unwrap()
                .instante
        );
    }

    #[test]
    fn ataque_versao_forjada_fora_do_formato_e_recusada() {
        for ruim in [
            "",
            "ward-20261108-143200",
            "../ward-20261108-143200.ward",
            "/home/x/ward-20261108-143200.ward",
            "backups/ward-20261108-143200.ward",
            "ward-20261108-143200.ward/",
            "ward-20261108-143200-01.ward",
            "ward-20261108-143200-100.ward",
            "ward-20261308-143200.ward",
            "ward-20261108-246000.ward",
            "WARD-20261108-143200.ward",
            "ward-20261108-143200.ward\n",
            " ward-20261108-143200.ward",
            "ward-2026１108-143200.ward",
            "ward-20261108-143200\u{202E}.ward",
            "cofre.ward",
        ] {
            assert_eq!(
                versao_pedida(ruim),
                Err(ErroApp::PedidoInvalido),
                "{ruim:?}"
            );
            assert_eq!(VersaoVista::de_nome(ruim), None, "{ruim:?}");
        }
        assert!(versao_pedida("ward-20261108-143200-99.ward").is_ok());
        assert_eq!(
            versoes_de(&["ward-20261108-143200.ward".into(), "../x".into()]),
            Err(ErroApp::RespostaInvalida)
        );
    }

    #[test]
    fn ataque_restauracao_com_campo_a_mais_ou_pin_ruim_e_recusada() {
        let ok = serde_json::json!({
            "versao": "ward-20261108-143200.ward",
            "senha": "falso-mestra-ward-01",
            "pin": "135790",
        });
        let r: Restauracao = serde_json::from_value(ok.clone()).unwrap();
        assert_eq!(r.versao, "ward-20261108-143200.ward");
        // O Debug não mostra nada digitado.
        let d = format!("{r:?}");
        assert!(!d.contains("falso-mestra") && !d.contains("135790"), "{d}");
        for (chave, valor) in [
            ("geracao", serde_json::json!(9)),
            ("caminho", serde_json::json!("/tmp/x.ward")),
        ] {
            let mut v = ok.clone();
            v[chave] = valor;
            assert!(serde_json::from_value::<Restauracao>(v).is_err(), "{chave}");
        }
        for (chave, valor) in [
            ("pin", serde_json::json!("12ab")),
            ("pin", serde_json::json!("123")),
            ("pin", serde_json::json!(135790)),
            ("senha", serde_json::json!("")),
            ("versao", serde_json::json!(1)),
        ] {
            let mut v = ok.clone();
            v[chave] = valor;
            assert!(serde_json::from_value::<Restauracao>(v).is_err(), "{chave}");
        }
        for falta in ["versao", "senha", "pin"] {
            let mut v = ok.clone();
            v.as_object_mut().unwrap().remove(falta);
            assert!(serde_json::from_value::<Restauracao>(v).is_err(), "{falta}");
        }
    }

    mod pelo_servico {
        use std::sync::{Arc, Mutex};

        use ward_core::protocolo::{self, ErroResposta, Papel, Resposta};
        use zeroize::Zeroizing;

        use super::super::*;
        use crate::area_transferencia::{AreaTransferencia, Copiador};
        use crate::daemon::Daemon;
        use crate::daemon_falso::{DaemonFalso, SEGREDOS};
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

        /// Daemon falso que guarda cada quadro recebido.
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

        fn restauracoes(e: &Espiao) -> usize {
            e.1.lock()
                .unwrap()
                .iter()
                .filter(|q| String::from_utf8_lossy(q).contains("\"restaurar_backup\""))
                .count()
        }

        fn pedido(versao: &str, senha: &str, pin: &str) -> Restauracao {
            serde_json::from_value(serde_json::json!({
                "versao": versao, "senha": senha, "pin": pin,
            }))
            .unwrap()
        }

        const SENHA: &str = "falso-mestra-ward-01";
        const PIN: &str = "135790";

        /// Nenhuma resposta ao app traz segredo, senha, PIN, caminho ou geração.
        fn sem_segredo(json: &str) {
            for s in SEGREDOS {
                assert!(!json.contains(s), "segredo em {json}");
            }
            for agulha in [PIN, "/home", ".local", "backups/", "geracao"] {
                assert!(!json.contains(agulha), "{agulha} em {json}");
            }
        }

        #[test]
        fn v7_7_escolhe_versao_confirma_e_as_entradas_da_versao_aparecem() {
            let (s, e) = servico(DaemonFalso::cheio());
            let antes = s.listar_credenciais().unwrap().len();
            let versoes = s.listar_backups().unwrap();
            assert_eq!(versoes.len(), 30);
            sem_segredo(&serde_json::to_string(&versoes).unwrap());
            // Da mais nova para a mais antiga.
            assert!(versoes.windows(2).all(|w| w[0].instante > w[1].instante));
            let escolhida = versoes[2].clone();
            let r = s
                .restaurar_backup(pedido(&escolhida.nome, SENHA, PIN))
                .unwrap();
            assert!(r.restaurado);
            // A versão 2 tem duas entradas a menos que o cofre de teste.
            assert_eq!(r.entradas, antes - 2);
            assert_eq!(s.listar_credenciais().unwrap().len(), antes - 2);
            sem_segredo(&serde_json::to_string(&r).unwrap());
            // O atual virou a versão mais nova; continuam 30.
            let depois = s.listar_backups().unwrap();
            assert_eq!(depois.len(), 30);
            assert_ne!(depois[0].nome, versoes[0].nome);
            assert_eq!(restauracoes(&e), 1);
            // O atual guardado volta com a senha de antes e as entradas de antes.
            let r = s
                .restaurar_backup(pedido(&depois[0].nome, SENHA, PIN))
                .unwrap();
            assert_eq!(r.entradas, antes);
        }

        #[test]
        fn ataque_pin_errado_nao_restaura_e_o_quinto_trava() {
            let (s, e) = servico(DaemonFalso::cheio());
            let v = s.listar_backups().unwrap()[3].nome.clone();
            let antes = e.0.lock().unwrap().quantas_entradas();
            for n in 1..=4 {
                assert_eq!(
                    s.restaurar_backup(pedido(&v, SENHA, "000000")),
                    Err(ErroApp::PinIncorreto)
                );
                assert_eq!(e.0.lock().unwrap().erros_de_pin(), n);
            }
            assert_eq!(e.0.lock().unwrap().quantas_entradas(), antes);
            assert_eq!(
                s.restaurar_backup(pedido(&v, SENHA, "000000")),
                Err(ErroApp::PinIncorreto)
            );
            // O 5.º trava: nada mais é listado nem restaurado, nem com o PIN certo.
            assert_eq!(s.listar_backups(), Err(ErroApp::Travado));
            assert_eq!(
                s.restaurar_backup(pedido(&v, SENHA, PIN)),
                Err(ErroApp::Travado)
            );
            assert_eq!(e.0.lock().unwrap().quantas_entradas(), antes);
        }

        #[test]
        fn ataque_senha_da_versao_errada_nao_muda_nada() {
            let (s, e) = servico(DaemonFalso::cheio());
            let nomes = e.0.lock().unwrap().nomes_dos_backups();
            let antes = e.0.lock().unwrap().quantas_entradas();
            for errada in ["falso-mestra-ward-02", "falso-mestra-ward-0", "x"] {
                assert_eq!(
                    s.restaurar_backup(pedido(&nomes[1], errada, PIN)),
                    Err(ErroApp::SenhaIncorreta)
                );
            }
            assert_eq!(e.0.lock().unwrap().quantas_entradas(), antes);
            assert_eq!(e.0.lock().unwrap().nomes_dos_backups(), nomes);
            // O PIN certo zerou a contagem (como no `wardd`).
            assert_eq!(e.0.lock().unwrap().erros_de_pin(), 0);
        }

        #[test]
        fn ataque_versao_inexistente_ou_forjada_nao_chega_ao_daemon() {
            let (s, e) = servico(DaemonFalso::cheio());
            // Fora do formato: recusado antes de qualquer quadro.
            for ruim in [
                "../cofre.ward",
                "/home/usuario/.local/share/ward/cofre.ward",
                "ward-20261108-143200.ward/../../x",
                "ward-20261108-143200\u{202E}.ward",
            ] {
                assert_eq!(
                    s.restaurar_backup(pedido(ruim, SENHA, PIN)),
                    Err(ErroApp::PedidoInvalido),
                    "{ruim:?}"
                );
            }
            assert!(e.1.lock().unwrap().is_empty());
            // No formato, mas fora da lista do daemon: nenhum `restaurar_backup`.
            assert_eq!(
                s.restaurar_backup(pedido("ward-19990101-000000.ward", SENHA, PIN)),
                Err(ErroApp::BackupInexistente)
            );
            assert_eq!(restauracoes(&e), 0);
            // Um cliente que pule o serviço e mande direto: o falso responde como o
            // `wardd` (`backup_inexistente`), e nada muda.
            let antes = e.0.lock().unwrap().quantas_entradas();
            let r = crate::cliente::restaurar_backup(
                &mut e.clone(),
                "ward-19990101-000000.ward",
                &crate::destrave::SenhaMestra::nova(SENHA.into()).unwrap(),
                &crate::nivel::Pin::novo(PIN.into()).unwrap(),
            );
            assert_eq!(r, Err(ErroApp::BackupInexistente));
            assert_eq!(e.0.lock().unwrap().quantas_entradas(), antes);
        }

        #[test]
        fn ataque_cofre_travado_no_meio_nao_restaura() {
            let (s, e) = servico(DaemonFalso::cheio());
            let v = s.listar_backups().unwrap()[1].nome.clone();
            let antes = e.0.lock().unwrap().quantas_entradas();
            // Travou (tempo, bandeja, outra janela) entre escolher e confirmar.
            s.travar().unwrap();
            assert_eq!(
                s.restaurar_backup(pedido(&v, SENHA, PIN)),
                Err(ErroApp::Travado)
            );
            assert_eq!(restauracoes(&e), 0);
            assert_eq!(e.0.lock().unwrap().quantas_entradas(), antes);
        }

        #[test]
        fn pasta_indisponivel_e_sem_versoes() {
            let (s, _) = servico(DaemonFalso::cheio().com_backups(None));
            assert_eq!(s.listar_backups(), Err(ErroApp::BackupIndisponivel));
            let (s, _) = servico(DaemonFalso::cheio().com_backups(Some(Vec::new())));
            assert_eq!(s.listar_backups(), Ok(Vec::new()));
        }

        /// O daemon que vê esta conexão como agente (o `wardd` com outro papel, D5).
        #[derive(Clone)]
        struct ComoAgente(Quadros);
        impl Daemon for ComoAgente {
            fn trocar(&mut self, pedido: &[u8]) -> Result<Zeroizing<Vec<u8>>, ErroApp> {
                self.0.lock().unwrap().push(pedido.to_vec());
                let r = match protocolo::decodificar_pedido(Papel::Agente, pedido) {
                    Ok(_) => Resposta::Erro(ErroResposta::Interno),
                    Err(e) => Resposta::Erro(e),
                };
                protocolo::codificar_resposta(Papel::Agente, &r)
                    .map_err(|_| ErroApp::RespostaInvalida)
            }
        }

        #[test]
        fn v7_8_papel_agente_recebe_nao_autorizado() {
            let q: Quadros = Arc::new(Mutex::new(Vec::new()));
            let mut d = ComoAgente(Arc::clone(&q));
            assert_eq!(crate::cliente::backups(&mut d), Err(ErroApp::NaoAutorizado));
            let r = crate::cliente::restaurar_backup(
                &mut d,
                "ward-20261108-143200.ward",
                &crate::destrave::SenhaMestra::nova(SENHA.into()).unwrap(),
                &crate::nivel::Pin::novo(PIN.into()).unwrap(),
            );
            assert_eq!(r, Err(ErroApp::NaoAutorizado));
            // Pelo serviço também: a lista já é recusada e o restaurar nem é pedido.
            let s = Servico::novo(ComoAgente(Arc::clone(&q)), Copiador::novo(SemArea));
            assert_eq!(s.listar_backups(), Err(ErroApp::NaoAutorizado));
            assert_eq!(
                s.restaurar_backup(pedido("ward-20261108-143200.ward", SENHA, PIN)),
                Err(ErroApp::NaoAutorizado)
            );
            assert_eq!(q.lock().unwrap().len(), 4);
        }
    }
}
