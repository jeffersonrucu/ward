//! Daemon falso, só nos testes e no `modo-teste` (D7): responde pelo protocolo da Fase
//! 20 com as entradas falsas do cofre de teste (D15 + `acme-totp-dev`, as mesmas de
//! `dev/fixtures/entradas.json`, montadas por `crates/ward-core/src/corpo/cofre_teste.rs`).
//!
//! Serve para a interface ser conferida no Chrome pela ponte de teste antes do `wardd`
//! da Fase 22. Recebe o corpo do quadro, lê com `decodificar_pedido` no papel app, monta
//! a resposta com `ItemLista::de_entrada` (com o resumo do papel app, D65 c) /
//! `DetalheEntrada::de_entrada` (sem segredo por construção) e devolve o corpo de
//! `codificar_resposta`. O segredo do `revelar`/`copiar`/`totp` sai por
//! `Alvo::segredo_em`, o mesmo do `wardd` (D65 b). Atende `estado`, `lista`,
//! `detalhe`, `copiar`, `revelar`, `totp`, `emitir_token` (Fase 34) e `salvar_entrada` (Fase 33, com a mesma
//! regra do `wardd`: `Rascunho::validar`, `conferir_no_cofre`, nível "com aprovação" na
//! entrada nova e mantido na editada) e `mudar_nivel` (Fase 35, a mesma regra do
//! `wardd`: PIN conferido no hash Argon2id da configuração do cofre de teste, em tempo
//! constante pela biblioteca; o 5º erro seguido trava, e travado tudo menos `estado`
//! responde "travado"), `destravar` (Fase 31: senha mestra de teste conferida em tempo
//! constante, espera de D20 antes de olhar a senha) e `responder_aprovacao` (Fase 36,
//! só os pedidos postos por [`DaemonFalso::com_aprovacao`], com a mesma regra do
//! `wardd`); o resto responde `interno`. As
//! mudanças ficam só na memória do processo (travado, só reiniciando a ponte). No
//! [`DaemonFalso::sem_cofre`], atende `criar_cofre` (Fase 30): guarda só o hash do PIN
//! (D30), que o `mudar_nivel` passa a conferir no lugar do PIN de teste.
//! As datas de D65 d (Fase 22d) seguem a regra do `wardd`: entrada nova ganha as duas,
//! edição e `mudar_nivel` só a alteração, pelo relógio do processo; as do cofre de
//! teste nascem sem datas.
//!
//! Sem a feature `modo-teste` este módulo nem existe no binário (só nos testes).

use zeroize::Zeroizing;

use ward_core::configuracao::HashPin;
use ward_core::entrada::{
    Datas, Entrada, NivelAcesso, Rascunho, Segredo as SegredoCore, TipoEntrada,
};
use ward_core::protocolo::{
    self, Alvo, Decisao, DetalheEntrada, ErroResposta, ItemLista, Papel, Pedido, PedidoDeAprovacao,
    Resposta,
};
use ward_core::totp::Totp;

use crate::ErroApp;
use crate::daemon::Daemon;

// O arquivo do cofre de teste usa a API pública do `ward-core` pelo apelido `wc`.
use ward_core as wc;
#[path = "../../../crates/ward-core/src/corpo/cofre_teste.rs"]
mod cofre_teste;

/// Os valores secretos do cofre de teste (as agulhas dos testes de ataque).
pub use cofre_teste::SEGREDOS;

/// Daemon falso com as entradas em memória.
pub struct DaemonFalso {
    entradas: Vec<Entrada>,
    /// `false` só no [`DaemonFalso::sem_cofre`] até o `criar_cofre` (Fase 30).
    existe: bool,
    /// Hash do PIN do cofre criado pelo `criar_cofre`, como o `wardd` o guarda (D30).
    hash_pin: Option<HashPin>,
    /// Erros de PIN seguidos (S9). Zera no acerto.
    erros_pin: u32,
    travado: bool,
    /// Senhas mestras erradas seguidas do papel app (D20, D62). Zera no acerto.
    falhas_destravar: u32,
    /// Dorme a espera de D20 antes de olhar a senha, como o `wardd` (os testes de
    /// unidade desligam para não esperar).
    espera_d20: bool,
    /// Pedidos de aprovação pendentes (Fase 36): id e entradas.
    aprovacoes: Vec<(u64, Vec<String>)>,
}

/// Pedido de aprovação simulado e há quanto tempo "chegou" (Fase 36, ponte de teste).
pub type AprovacaoSimulada = (PedidoDeAprovacao, std::time::Duration);

/// Os pedidos de aprovação da ponte de teste (`--daemon-falso aprovacao…`), para a QA
/// dos estados do M0 no Chrome. Tudo falso; nenhum segredo (o pedido não tem campo
/// para isso).
/// - `aprovacao`: `acme-pg-aprovacao` (stage), como o `Aprovação / Normal`;
/// - `aprovacao-prod`: `acme-pg-prod`, para o `Aprovação / prod`;
/// - `aprovacao-expirando`: o normal chegado há 104 s (faltam 16 s);
/// - `aprovacao-ataque` (V5.29): `sh -c` com U+202E, U+2066, largura zero, `ESC[2J`,
///   `\n` imitando outro comando e HTML, crus (como se o daemon não tivesse
///   neutralizado), e um processo `claude\u{202E}` na cadeia.
pub fn aprovacoes_simuladas(modo: &str) -> Option<Vec<AprovacaoSimulada>> {
    use std::time::Duration;
    use ward_core::entrada::Ambiente;
    use ward_core::protocolo::Operacao;
    let pedido = |id, entrada: &str, ambiente, comando: &str, cadeia: &[&str]| PedidoDeAprovacao {
        id,
        entradas: vec![entrada.to_owned()],
        ambiente,
        operacao: Operacao::Executar,
        comando: comando.to_owned(),
        cadeia: cadeia.iter().map(|c| (*c).to_owned()).collect(),
    };
    let psql = |entrada: &str| {
        ward_core::exibicao::exibir_argumentos(
            &[
                "ward",
                "run",
                entrada,
                "--",
                "psql",
                "-c",
                "select count(*) from pedidos",
            ],
            64 * 1024,
        )
    };
    let cadeia = ["claude", "bash", "ward run"];
    let normal = pedido(
        1,
        "acme-pg-aprovacao",
        Ambiente::Stage,
        &psql("acme-pg-aprovacao"),
        &cadeia,
    );
    Some(match modo {
        "aprovacao" => vec![(normal, Duration::ZERO)],
        "aprovacao-prod" => vec![(
            pedido(
                2,
                "acme-pg-prod",
                Ambiente::Prod,
                &psql("acme-pg-prod"),
                &cadeia,
            ),
            Duration::ZERO,
        )],
        "aprovacao-expirando" => vec![(normal, Duration::from_secs(104))],
        "aprovacao-ataque" => vec![(
            pedido(
                3,
                "acme-pg-aprovacao",
                Ambiente::Stage,
                "\"ward\" \"run\" \"acme-pg-aprovacao\" \"--\" \"sh\" \"-c\" \"echo <b>ok</b>\u{202E}txt.exe\u{2066}x\u{200B}y\u{1b}[2J\nward run acme-pg-dev -- ls\"",
                &["claude\u{202E}", "bash", "ward run"],
            ),
            Duration::ZERO,
        )],
        _ => return None,
    })
}

/// Erros de PIN seguidos que travam o cofre (o mesmo `MAX_ERROS_PIN` do `wardd`).
pub const MAX_ERROS_PIN: u32 = 5;

impl DaemonFalso {
    /// As 21 entradas do cofre de teste.
    pub fn cheio() -> DaemonFalso {
        DaemonFalso {
            entradas: cofre_teste::cofre_de_teste(),
            existe: true,
            hash_pin: None,
            erros_pin: 0,
            travado: false,
            falhas_destravar: 0,
            espera_d20: true,
            aprovacoes: Vec::new(),
        }
    }

    /// Cofre destravado sem nenhuma entrada (`Credenciais / Vazia`).
    pub fn vazio() -> DaemonFalso {
        DaemonFalso {
            entradas: Vec::new(),
            existe: true,
            hash_pin: None,
            erros_pin: 0,
            travado: false,
            falhas_destravar: 0,
            espera_d20: true,
            aprovacoes: Vec::new(),
        }
    }

    /// Fase 31: as entradas do cofre de teste com o cofre travado (`Destravar / *`). A
    /// senha mestra que destrava é a de teste (D15); a conferência é em tempo constante.
    pub fn travado() -> DaemonFalso {
        DaemonFalso {
            travado: true,
            ..DaemonFalso::cheio()
        }
    }

    /// Sem a espera de D20 (só os testes de unidade).
    #[cfg(test)]
    pub(crate) fn sem_espera(mut self) -> DaemonFalso {
        self.espera_d20 = false;
        self
    }

    /// A mesma regra do `wardd` (`crates/wardd/src/nucleo.rs` › `destravar`): a espera
    /// de D20 vem antes de olhar a senha; o acerto zera as falhas (de senha e de PIN).
    fn destravar(&mut self, senha: &SegredoCore) -> Result<(), ErroResposta> {
        if !self.travado {
            return Ok(());
        }
        if self.espera_d20 && self.falhas_destravar > 0 {
            let segundos = 1u64
                .checked_shl(self.falhas_destravar - 1)
                .unwrap_or(u64::MAX)
                .min(30);
            std::thread::sleep(std::time::Duration::from_secs(segundos));
        }
        let certa = {
            use ctutils::CtEq;
            senha
                .expor()
                .ct_eq(cofre_teste::SENHA_MESTRA.as_bytes())
                .to_bool()
        };
        if !certa {
            self.falhas_destravar = self.falhas_destravar.saturating_add(1);
            return Err(ErroResposta::SenhaIncorreta);
        }
        self.falhas_destravar = 0;
        self.erros_pin = 0;
        self.travado = false;
        Ok(())
    }

    /// Sem cofre ainda (`Primeiro uso / *`, Fase 30): `estado` responde
    /// `cofre_existe: false`, o resto responde "ainda não existe cofre" até o
    /// `criar_cofre`, que deixa o cofre vazio e destravado, como o `wardd`.
    pub fn sem_cofre() -> DaemonFalso {
        DaemonFalso {
            entradas: Vec::new(),
            existe: false,
            hash_pin: None,
            erros_pin: 0,
            travado: false,
            falhas_destravar: 0,
            espera_d20: true,
            aprovacoes: Vec::new(),
        }
    }

    /// Fase 36: um pedido de aprovação pendente, como se o `wardd` o tivesse mandado ao
    /// app (o aviso vai à fila do serviço por quem monta a ponte).
    pub fn com_aprovacao(mut self, p: &PedidoDeAprovacao) -> DaemonFalso {
        self.aprovacoes.push((p.id, p.entradas.clone()));
        self
    }

    /// Ids dos pedidos de aprovação que ainda esperam (só para os testes).
    pub fn aprovacoes_pendentes(&self) -> Vec<u64> {
        self.aprovacoes.iter().map(|(id, _)| *id).collect()
    }

    /// Nível de uma entrada (só para os testes).
    pub fn nivel_de(&self, nome: &str) -> Option<NivelAcesso> {
        self.entrada(nome).map(Entrada::nivel)
    }

    /// O PIN confere com o hash guardado pelo `criar_cofre` (só para os testes).
    pub fn pin_confere(&self, pin: &[u8]) -> bool {
        self.hash_pin.as_ref().is_some_and(|h| h.conferir(pin))
    }

    fn entrada(&self, nome: &str) -> Option<&Entrada> {
        self.entradas.iter().find(|e| e.nome() == nome)
    }

    fn atender(&mut self, pedido: Pedido) -> Resposta {
        if !self.existe {
            return match pedido {
                Pedido::Estado => Resposta::Estado {
                    travado: true,
                    cofre_existe: false,
                    papel: Papel::App,
                },
                // Como o `wardd` (`nucleo.rs` › `criar_cofre`): só o hash do PIN fica.
                Pedido::CriarCofre { senha: _, pin } => match HashPin::criar(pin.expor()) {
                    Ok(h) => {
                        self.hash_pin = Some(h);
                        self.existe = true;
                        Resposta::Ok
                    }
                    Err(_) => Resposta::Erro(ErroResposta::PedidoInvalido),
                },
                _ => Resposta::Erro(ErroResposta::CofreInexistente),
            };
        }
        if self.travado
            && !matches!(
                pedido,
                Pedido::Estado | Pedido::Destravar { .. } | Pedido::Travar
            )
        {
            return Resposta::Erro(ErroResposta::Travado);
        }
        match pedido {
            Pedido::Estado => Resposta::Estado {
                travado: self.travado,
                cofre_existe: true,
                papel: Papel::App,
            },
            Pedido::Destravar { senha } => match self.destravar(&senha) {
                Ok(()) => Resposta::Ok,
                Err(e) => Resposta::Erro(e),
            },
            Pedido::Travar => {
                self.travado = true;
                Resposta::Ok
            }
            Pedido::CriarCofre { .. } => Resposta::Erro(ErroResposta::CofreJaExiste),
            Pedido::Lista => Resposta::Lista(
                self.entradas
                    .iter()
                    .map(|e| ItemLista::de_entrada(e, Papel::App))
                    .collect(),
            ),
            Pedido::Detalhe { entrada } => match self.entrada(&entrada) {
                Some(e) => Resposta::Detalhe(DetalheEntrada::de_entrada(e)),
                None => Resposta::Erro(ErroResposta::EntradaInexistente),
            },
            Pedido::Copiar { entrada, alvo } | Pedido::Revelar { entrada, alvo } => {
                match self.entrada(&entrada) {
                    Some(e) => match alvo.segredo_em(e) {
                        Some(v) => Resposta::Segredo(SegredoCore::de_bytes(v.expor().to_vec())),
                        None => Resposta::Erro(ErroResposta::CampoInexistente),
                    },
                    None => Resposta::Erro(ErroResposta::EntradaInexistente),
                }
            }
            Pedido::Totp { entrada } => match self.entrada(&entrada) {
                Some(e) => match Alvo::Campo("totp_semente".to_owned()).segredo_em(e) {
                    Some(semente) => {
                        match Totp::de_semente(semente).and_then(|t| t.codigo_agora()) {
                            Ok(c) => Resposta::CodigoTotp {
                                codigo: Zeroizing::new(c.codigo().to_owned()),
                                restante: c.restante(),
                            },
                            Err(_) => Resposta::Erro(ErroResposta::Interno),
                        }
                    }
                    None => Resposta::Erro(ErroResposta::CampoInexistente),
                },
                None => Resposta::Erro(ErroResposta::EntradaInexistente),
            },
            Pedido::SalvarEntrada { anterior, rascunho } => {
                match self.salvar(anterior.as_deref(), rascunho) {
                    Ok(()) => Resposta::Ok,
                    Err(e) => Resposta::Erro(e),
                }
            }
            Pedido::MudarNivel {
                entrada,
                nivel,
                pin,
            } => match self.mudar_nivel(&entrada, nivel, &pin) {
                Ok(()) => Resposta::Ok,
                Err(e) => Resposta::Erro(e),
            },
            // Fase 34: o token do "Conectar" (D3), só para `servidor`. O falso não abre
            // sessão nenhuma; o token é aleatório e não é guardado.
            // Fase 36: a mesma regra do `wardd` (`uso.rs` › `responder_aprovacao`).
            Pedido::ResponderAprovacao {
                pedido,
                decisao,
                pin,
            } => match self.responder_aprovacao(pedido, decisao, pin.as_ref()) {
                Ok(()) => Resposta::Ok,
                Err(e) => Resposta::Erro(e),
            },
            Pedido::EmitirToken { entrada } => match self.entrada(&entrada) {
                Some(e) if e.tipo() == TipoEntrada::Servidor => {
                    match ward_core::gerador::gerar(&ward_core::gerador::OpcoesGerador {
                        tamanho: 32,
                        minusculas: true,
                        maiusculas: true,
                        digitos: true,
                        simbolos: false,
                    }) {
                        Ok(t) => Resposta::Token(t),
                        Err(_) => Resposta::Erro(ErroResposta::Interno),
                    }
                }
                Some(_) => Resposta::Erro(ErroResposta::PedidoInvalido),
                None => Resposta::Erro(ErroResposta::EntradaInexistente),
            },
            _ => Resposta::Erro(ErroResposta::Interno),
        }
    }

    /// A mesma regra do `wardd` (`crates/wardd/src/nucleo.rs` › `conferir_pin`): hash
    /// Argon2id da biblioteca (tempo constante); o 5º erro seguido trava.
    fn conferir_pin(&mut self, pin: &SegredoCore) -> Result<(), ErroResposta> {
        let confere = match &self.hash_pin {
            Some(h) => h.conferir(pin.expor()),
            None => cofre_teste::configuracao_de_teste().conferir_pin(pin.expor()),
        };
        if !confere {
            self.erros_pin = self.erros_pin.saturating_add(1);
            if self.erros_pin >= MAX_ERROS_PIN {
                self.travado = true;
            }
            return Err(ErroResposta::PinIncorreto);
        }
        self.erros_pin = 0;
        Ok(())
    }

    /// A mesma regra do `wardd` (`uso.rs` › `responder_aprovacao`): pedido desconhecido
    /// é `expirado`; "Negar" sem PIN; PIN errado deixa pendente, e o 5º trava e nega;
    /// "sempre" deixa livre as entradas do pedido que estão "com aprovação".
    fn responder_aprovacao(
        &mut self,
        id: u64,
        decisao: Decisao,
        pin: Option<&SegredoCore>,
    ) -> Result<(), ErroResposta> {
        let i = self
            .aprovacoes
            .iter()
            .position(|(p, _)| *p == id)
            .ok_or(ErroResposta::Expirado)?;
        if decisao == Decisao::Negar {
            self.aprovacoes.remove(i);
            return Ok(());
        }
        let pin = pin.ok_or(ErroResposta::PedidoInvalido)?;
        if let Err(e) = self.conferir_pin(pin) {
            if self.travado {
                self.aprovacoes.remove(i);
            }
            return Err(e);
        }
        let (_, entradas) = self.aprovacoes.remove(i);
        if decisao == Decisao::PermitirSempre {
            for e in self.entradas.iter_mut() {
                if entradas.iter().any(|n| n == e.nome()) && e.nivel() == NivelAcesso::ComAprovacao
                {
                    e.definir_nivel(NivelAcesso::Livre);
                }
            }
        }
        Ok(())
    }

    /// A mesma regra do `wardd` (`crates/wardd/src/nucleo.rs` › `conferir_pin` e
    /// `mudar_nivel`): o PIN primeiro, e só depois a entrada.
    fn mudar_nivel(
        &mut self,
        nome: &str,
        nivel: NivelAcesso,
        pin: &SegredoCore,
    ) -> Result<(), ErroResposta> {
        self.conferir_pin(pin)?;
        let e = self
            .entradas
            .iter_mut()
            .find(|e| e.nome() == nome)
            .ok_or(ErroResposta::EntradaInexistente)?;
        e.definir_nivel(nivel);
        e.definir_datas(alterado(e.datas()));
        Ok(())
    }

    /// A mesma regra do `wardd` (`crates/wardd/src/nucleo.rs` › `salvar_entrada`).
    fn salvar(&mut self, anterior: Option<&str>, rascunho: Rascunho) -> Result<(), ErroResposta> {
        let mut nova = rascunho
            .validar()
            .map_err(|e| ErroResposta::de_validacao(&e))?;
        match anterior {
            None => {
                nova.definir_nivel(NivelAcesso::ComAprovacao);
                nova.definir_datas(
                    agora()
                        .and_then(Datas::carimbo_novo)
                        .unwrap_or(Datas::NENHUMA),
                );
                nova.conferir_no_cofre(self.entradas.iter())
                    .map_err(|e| ErroResposta::de_validacao(&e))?;
            }
            Some(nome) => {
                let i = self
                    .entradas
                    .iter()
                    .position(|e| e.nome() == nome)
                    .ok_or(ErroResposta::EntradaInexistente)?;
                nova.definir_nivel(self.entradas[i].nivel());
                nova.definir_datas(alterado(self.entradas[i].datas()));
                nova.conferir_no_cofre(self.entradas.iter().filter(|e| e.nome() != nome))
                    .map_err(|e| ErroResposta::de_validacao(&e))?;
                drop(self.entradas.remove(i));
            }
        }
        self.entradas.push(nova);
        self.entradas.sort_by(|a, b| a.nome().cmp(b.nome()));
        Ok(())
    }
}

impl Daemon for DaemonFalso {
    fn trocar(&mut self, pedido: &[u8]) -> Result<Zeroizing<Vec<u8>>, ErroApp> {
        let resposta = match protocolo::decodificar_pedido(Papel::App, pedido) {
            Ok(p) => self.atender(p),
            Err(e) => Resposta::Erro(e),
        };
        protocolo::codificar_resposta(Papel::App, &resposta).map_err(|_| ErroApp::RespostaInvalida)
    }
}

/// Segundos UTC do relógio do processo (o `RelogioReal` do `wardd`, D65 d).
fn agora() -> Option<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|d| Datas::instante(d.as_secs()))
}

/// As datas depois de uma alteração agora (as mesmas sem relógio), como no `wardd`.
fn alterado(datas: Datas) -> Datas {
    agora().and_then(|a| datas.alterado(a)).unwrap_or(datas)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn mesmas_entradas_das_fixtures() {
        // `dev/fixtures/entradas.json` é a fonte das entradas do cofre de teste; o
        // daemon falso tem exatamente os mesmos nomes, tipos, grupos, ambientes e níveis.
        let caminho = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dev/fixtures/entradas.json"
        );
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(caminho).unwrap()).unwrap();
        let mut esperado: Vec<(String, String, String, String, String)> = json["entradas"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                let t = |k: &str| e[k].as_str().unwrap().to_owned();
                (t("nome"), t("tipo"), t("grupo"), t("ambiente"), t("nivel"))
            })
            .collect();
        let mut falso: Vec<_> = DaemonFalso::cheio()
            .entradas
            .iter()
            .map(|e| {
                (
                    e.nome().to_owned(),
                    e.tipo().nome().to_owned(),
                    e.grupo().to_owned(),
                    e.ambiente().nome().to_owned(),
                    e.nivel().nome().to_owned(),
                )
            })
            .collect();
        esperado.sort();
        falso.sort();
        assert_eq!(falso, esperado);
    }

    #[test]
    fn so_o_papel_app_e_atendido() {
        // O falso lê o pedido como o daemon real leria do app; um quadro de agente com
        // operação só do app vira "não autorizado" do lado do agente, e um corpo
        // inválido vira erro, nunca pânico.
        let mut d = DaemonFalso::cheio();
        for lixo in [&b""[..], b"{}", b"{\"v\":1,\"op\":\"revelar\"}", b"\xff"] {
            let r = d.trocar(lixo).unwrap();
            let r = protocolo::decodificar_resposta(Papel::App, &r).unwrap();
            assert!(matches!(r, Resposta::Erro(_)));
        }
    }
}
