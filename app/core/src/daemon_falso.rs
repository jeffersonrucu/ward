//! Daemon falso, só nos testes e no `test-mode` (D7): responde pelo protocolo da Fase
//! 20 com as entradas falsas do cofre de teste (D15 + `acme-totp-dev`, as mesmas de
//! `dev/fixtures/entries.json`, montadas por `crates/ward-core/src/corpo/cofre_teste.rs`).
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
//! (D30), que o `mudar_nivel` passa a conferir no lugar do PIN de teste. Na tela
//! Configurações (Fase 38): `configuracao`, `tempo_trava` (1 a 1440, D45), `trocar_pin`
//! (PIN atual conferido como no `mudar_nivel`, com a mesma contagem de erros) e
//! `trocar_senha` (senha atual conferida em tempo constante; a nova passa a destravar).
//! Backup externo (Fase 48): `backup_externo`, `definir_backup_externo` (a mesma
//! conferência do `wardd`: `ExternalBackup::new`) e `pendrives`, com os pendrives falsos
//! de [`pendrives_de_teste`] (nenhum dispositivo de verdade; rótulos com HTML, direção de
//! texto e largura zero para a QA da exibição).
//! Restaurar (Fase 47): `backups` e `restaurar_backup` com a mesma ordem de conferência
//! do `wardd` (`nucleo.rs` › `restaurar_backup`): nome no formato, PIN atual (conta para
//! os 5 que travam), versão na pasta, senha mestra da versão (em tempo constante); o
//! atual vira a versão mais nova e o cofre passa a ter as entradas, a senha e o PIN da
//! versão. As versões falsas ([`backups_de_teste`]) são cópias do cofre de teste com
//! menos entradas, todas com a senha mestra de teste (D15).
//! As datas de D65 d (Fase 22d) seguem a regra do `wardd`: entrada nova ganha as duas,
//! edição e `mudar_nivel` só a alteração, pelo relógio do processo; as do cofre de
//! teste nascem sem datas.
//!
//! Sem a feature `test-mode` este módulo nem existe no binário (só nos testes).

use zeroize::Zeroizing;

use ward_core::config::PinHash;
use ward_core::entry::{AccessLevel, Dates, Draft, Entry, EntryType, Secret as SegredoCore};
use ward_core::protocolo::{
    self, Alvo, Decisao, DetalheEntrada, ErroResposta, ItemLista, Papel, Pedido, PedidoDeAprovacao,
    Pendrive, Resposta,
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
    entradas: Vec<Entry>,
    /// `false` só no [`DaemonFalso::sem_cofre`] até o `criar_cofre` (Fase 30).
    existe: bool,
    /// Hash do PIN do cofre criado pelo `criar_cofre`, como o `wardd` o guarda (D30).
    hash_pin: Option<PinHash>,
    /// Erros de PIN seguidos (S9). Zera no acerto.
    erros_pin: u32,
    travado: bool,
    /// Senhas mestras erradas seguidas do papel app (D20, D62). Zera no acerto.
    falhas_destravar: u32,
    /// Dorme a espera de D20 antes de olhar a senha, como o `wardd` (os testes de
    /// unidade desligam para não esperar).
    espera_d20: bool,
    /// Tempo de trava da configuração (D30; padrão 240).
    minutos_trava: u32,
    /// Senha mestra trocada pelo `trocar_senha` (Fase 38); sem ela vale a de teste.
    senha: Option<Zeroizing<Vec<u8>>>,
    /// Pedidos de aprovação pendentes (Fase 36): id e entradas.
    /// Id, entradas e se a entrada padrão é oculta (Fase 54b).
    aprovacoes: Vec<(u64, Vec<String>, bool)>,
    /// Backup externo da configuração (Fase 48; D30: desligado e sem UUID por padrão).
    backup: wc::config::ExternalBackup,
    /// Pendrives que o "udisks2" vê; `None` é o udisks2 sem resposta.
    pendrives: Option<Vec<Pendrive>>,
    /// Versões do backup local (Fase 47), da mais nova para a mais antiga; `None` é a
    /// pasta de backups indisponível.
    backups: Option<Vec<BackupFalso>>,
    /// Geração do cofre atual (Fase 49a): a que o atual leva ao virar backup.
    geracao: u64,
    /// A pasta do backup local é segura (Fase 49a); `false` no modo
    /// `backup-local-inseguro`.
    backup_local: bool,
}

/// Uma versão falsa do backup local (Fase 47): o nome, a senha mestra que valia nela e
/// as entradas. Só em memória.
pub struct BackupFalso {
    nome: String,
    /// A geração selada (Fase 49a); `None` como a de outra senha mestra.
    geracao: Option<u64>,
    senha: Zeroizing<Vec<u8>>,
    entradas: EntradasDoBackup,
}

/// As entradas de uma versão falsa. `Entrada` não tem `Clone` (tem segredo): as versões
/// de teste são montadas de novo a cada restaurar; o cofre guardado no restaurar vai
/// inteiro (e sai da lista quando é restaurado, a única diferença para o `wardd`, que
/// deixa o arquivo na pasta).
enum EntradasDoBackup {
    /// O cofre de teste menos as `n` últimas entradas.
    Teste {
        menos: usize,
    },
    Guardadas(Vec<Entry>),
}

fn cofre_de_teste_menos(menos: usize) -> Vec<Entry> {
    let mut e = cofre_teste::cofre_de_teste();
    e.truncate(e.len().saturating_sub(menos));
    e
}

/// Quantas versões ficam (D17), como o `wardd`.
const VERSOES_FALSAS: usize = 30;

/// As 30 versões falsas da ponte de teste (Fase 47), contadas para trás a partir de
/// `agora`: há 2 min, há 5 h, ontem, há 3 e há 6 dias e depois uma a cada 9 h. A versão
/// `i` tem as entradas do cofre de teste menos as `min(i, 6)` últimas, para a troca
/// aparecer na lista. Todas com a senha mestra de teste (D15); nenhum arquivo.
pub fn backups_de_teste(agora: std::time::SystemTime) -> Vec<BackupFalso> {
    use std::time::Duration;
    let min = |m: u64| Duration::from_secs(m * 60);
    let mut idades = vec![
        min(2),
        min(5 * 60 + 17),
        min(25 * 60 + 45),
        min(3 * 24 * 60 + 3 * 60 + 30),
        min(6 * 24 * 60 + 22 * 60 + 12),
    ];
    while idades.len() < VERSOES_FALSAS {
        let ultima = idades[idades.len() - 1];
        idades.push(ultima + min(9 * 60));
    }
    idades
        .iter()
        .enumerate()
        .filter_map(|(i, idade)| {
            let nome = wc::backup::Versao::do_instante(agora.checked_sub(*idade)?)?.nome();
            Some(BackupFalso {
                nome,
                geracao: u64::try_from(VERSOES_FALSAS - i).ok(),
                senha: Zeroizing::new(cofre_teste::SENHA_MESTRA.as_bytes().to_vec()),
                entradas: EntradasDoBackup::Teste { menos: i.min(6) },
            })
        })
        .collect()
}

/// Os pendrives falsos da ponte de teste (Fase 48). Só dados: nenhum dispositivo, nenhum
/// caminho. O 3º tem rótulo com HTML, U+202E e largura zero (o udisks2 aceita qualquer
/// rótulo); o 4º não tem rótulo.
pub fn pendrives_de_teste() -> Vec<Pendrive> {
    let p = |uuid: &str, rotulo: &str, montado| Pendrive {
        uuid: uuid.to_owned(),
        rotulo: rotulo.to_owned(),
        montado,
    };
    vec![
        p("7C1E-42A9", "WARD-BACKUP", true),
        p("3F2A-9B10", "FOTOS 2024", false),
        p(
            "a1b2c3d4-0000-4e5f-9a8b-1234567890ab",
            "<img src=x onerror=alert(1)>\u{202E}gpj.exe\u{200B}",
            true,
        ),
        p("0E9D-55C1", "", true),
    ]
}

/// Pedido de aprovação simulado e há quanto tempo "chegou" (Fase 36, ponte de teste).
pub type AprovacaoSimulada = (PedidoDeAprovacao, std::time::Duration);

/// Os pedidos de aprovação da ponte de teste (`--daemon-falso aprovacao…`), para a QA
/// dos estados do M0 no Chrome. Tudo falso; nenhum segredo (o pedido não tem campo
/// para isso).
/// - `aprovacao`: `acme-pg-aprovacao` (stage), como o `Aprovação / Normal`;
/// - `aprovacao-prod`: `acme-pg-prod`, para o `Aprovação / prod`;
/// - `aprovacao-expirando`: o normal chegado há 104 s (faltam 16 s);
/// - `aprovacao-entrada` (Fase 54b, D68): `printf <script> | ward run acme-pg-aprovacao
///   -- python3` da auditoria final: a entrada padrão é pipe, a janela avisa e os
///   "Permitir" esperam a caixa;
/// - `aprovacao-ataque` (V5.29): `sh -c` com U+202E, U+2066, largura zero, `ESC[2J`,
///   `\n` imitando outro comando e HTML, crus (como se o daemon não tivesse
///   neutralizado), e um processo `claude\u{202E}` na cadeia.
pub fn aprovacoes_simuladas(modo: &str) -> Option<Vec<AprovacaoSimulada>> {
    use std::time::Duration;
    use ward_core::entry::Environment;
    use ward_core::protocolo::Operacao;
    let pedido = |id, entrada: &str, ambiente, comando: &str, cadeia: &[&str]| PedidoDeAprovacao {
        id,
        entradas: vec![entrada.to_owned()],
        ambiente,
        operacao: Operacao::Executar,
        comando: comando.to_owned(),
        diretorio: "\"/home/dev/projetos/acme\"".to_owned(),
        cadeia: cadeia.iter().map(|c| (*c).to_owned()).collect(),
        origem: String::new(),
        confirmacao: false,
        entrada_oculta: false,
    };
    // Fase 44c (D9): preenchimento numa origem `http`.
    // Fase 44e: ambiente e origem da entrada de verdade do cofre de teste
    // (`acme-wp-dev` é dev, `acme-wp-prod` é prod), como o `wardd` montaria.
    let na_origem = |id, entrada: &str, operacao, confirmacao, cadeia: &[&str]| {
        let entradas = cofre_teste::cofre_de_teste();
        let e = entradas.iter().find(|e| e.name() == entrada)?;
        let ward_core::entry::Data::Login(l) = e.data() else {
            return None;
        };
        Some(PedidoDeAprovacao {
            id,
            entradas: vec![entrada.to_owned()],
            ambiente: e.environment(),
            operacao,
            comando: String::new(),
            diretorio: String::new(),
            cadeia: cadeia.iter().map(|c| (*c).to_owned()).collect(),
            origem: ward_core::entry::url_origin(&l.url)?,
            confirmacao,
            entrada_oculta: false,
        })
    };
    let psql = |entrada: &str| {
        ward_core::display::display_args(
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
        Environment::Stage,
        &psql("acme-pg-aprovacao"),
        &cadeia,
    );
    Some(match modo {
        "aprovacao" => vec![(normal, Duration::ZERO)],
        "aprovacao-prod" => vec![(
            pedido(
                2,
                "acme-pg-prod",
                Environment::Prod,
                &psql("acme-pg-prod"),
                &cadeia,
            ),
            Duration::ZERO,
        )],
        "aprovacao-expirando" => vec![(normal, Duration::from_secs(104))],
        "aprovacao-entrada" => vec![(
            PedidoDeAprovacao {
                entrada_oculta: true,
                ..pedido(
                    7,
                    "acme-pg-aprovacao",
                    Environment::Stage,
                    &ward_core::display::display_args(&["python3"], 64 * 1024),
                    &cadeia,
                )
            },
            Duration::ZERO,
        )],
        // Fase 44c: `ward playwright acme-wp-prod` (livre) numa origem `http`: só a
        // confirmação, sem PIN.
        "aprovacao-origem" => vec![(
            na_origem(
                4,
                "acme-wp-prod",
                Operacao::Playwright,
                true,
                &["claude", "bash", "ward playwright"],
            )?,
            Duration::ZERO,
        )],
        // Fase 44c: o popup da extensão numa origem `http`.
        "aprovacao-origem-popup" => vec![(
            na_origem(
                5,
                "acme-wp-prod",
                Operacao::Preencher,
                true,
                &["chrome", "ward-nm"],
            )?,
            Duration::ZERO,
        )],
        // Fase 44c: `ward login acme-wp-dev` (com aprovação) numa origem `http`: PIN e
        // origem.
        "aprovacao-origem-pin" => vec![(
            na_origem(
                6,
                "acme-wp-dev",
                Operacao::Login,
                false,
                &["claude", "bash", "ward login"],
            )?,
            Duration::ZERO,
        )],
        "aprovacao-ataque" => vec![(
            pedido(
                3,
                "acme-pg-aprovacao",
                Environment::Stage,
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
            backups: Some(backups_de_teste(std::time::SystemTime::now())),
            geracao: VERSOES_FALSAS as u64 + 1,
            ..DaemonFalso::vazio()
        }
    }

    /// Fase 49a: o cheio com uma cópia antiga (geração 3) plantada com o nome de daqui
    /// a 1 min, dentro da margem do relógio: aparece no topo, marcada pela geração.
    pub fn backup_plantado() -> DaemonFalso {
        let mut d = DaemonFalso::cheio();
        let agora = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
        if let (Some(lista), Some(v)) = (d.backups.as_mut(), wc::backup::Versao::do_instante(agora))
        {
            lista.insert(
                0,
                BackupFalso {
                    nome: v.nome(),
                    geracao: Some(3),
                    senha: Zeroizing::new(cofre_teste::SENHA_MESTRA.as_bytes().to_vec()),
                    entradas: EntradasDoBackup::Teste { menos: 6 },
                },
            );
        }
        d
    }

    /// Fase 49a: o cheio com a pasta do backup local insegura (a linha "Backup local"
    /// de Configurações avisa).
    pub fn backup_local_inseguro() -> DaemonFalso {
        DaemonFalso {
            backup_local: false,
            ..DaemonFalso::cheio()
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
            minutos_trava: wc::limits::DEFAULT_LOCK_MINUTES,
            senha: None,
            aprovacoes: Vec::new(),
            backup: wc::config::ExternalBackup::disabled(),
            pendrives: Some(pendrives_de_teste()),
            backups: Some(Vec::new()),
            geracao: 1,
            backup_local: true,
        }
    }

    /// Fase 48: o cofre de teste com o backup externo ligado no `WARD-BACKUP`
    /// (`Configurações / Backup externo ligado`).
    pub fn backup_externo_ligado() -> DaemonFalso {
        let mut d = DaemonFalso::cheio();
        d.backup = wc::config::ExternalBackup::new(true, Some("7C1E-42A9"))
            .unwrap_or_else(|_| wc::config::ExternalBackup::disabled());
        d
    }

    /// Fase 48: os pendrives que o falso mostra; `None` é o udisks2 sem resposta.
    pub fn com_pendrives(mut self, pendrives: Option<Vec<Pendrive>>) -> DaemonFalso {
        self.pendrives = pendrives;
        self
    }

    /// Fase 47: as versões do backup local; `None` é a pasta indisponível.
    pub fn com_backups(mut self, backups: Option<Vec<BackupFalso>>) -> DaemonFalso {
        self.backups = backups;
        self
    }

    /// Nomes das versões do backup local (só para os testes).
    pub fn nomes_dos_backups(&self) -> Vec<String> {
        self.backups
            .iter()
            .flatten()
            .map(|b| b.nome.clone())
            .collect()
    }

    /// Quantas entradas o cofre tem (só para os testes).
    pub fn quantas_entradas(&self) -> usize {
        self.entradas.len()
    }

    /// Erros de PIN seguidos (só para os testes).
    pub fn erros_de_pin(&self) -> u32 {
        self.erros_pin
    }

    /// Backup externo da configuração (só para os testes).
    pub fn backup(&self) -> (bool, Option<String>) {
        (self.backup.enabled(), self.backup.uuid().map(str::to_owned))
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
        if !self.senha_confere(senha) {
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
            existe: false,
            ..DaemonFalso::vazio()
        }
    }

    /// A senha mestra confere, em tempo constante: a trocada pelo `trocar_senha` ou, sem
    /// troca, a de teste (D15).
    fn senha_confere(&self, senha: &SegredoCore) -> bool {
        use ctutils::CtEq;
        let certa: &[u8] = match &self.senha {
            Some(s) => s,
            None => cofre_teste::SENHA_MESTRA.as_bytes(),
        };
        senha.expose().len() == certa.len() && senha.expose().ct_eq(certa).to_bool()
    }

    /// Fase 36: um pedido de aprovação pendente, como se o `wardd` o tivesse mandado ao
    /// app (o aviso vai à fila do serviço por quem monta a ponte).
    pub fn com_aprovacao(mut self, p: &PedidoDeAprovacao) -> DaemonFalso {
        self.aprovacoes
            .push((p.id, p.entradas.clone(), p.entrada_oculta));
        self
    }

    /// Ids dos pedidos de aprovação que ainda esperam (só para os testes).
    pub fn aprovacoes_pendentes(&self) -> Vec<u64> {
        self.aprovacoes.iter().map(|(id, _, _)| *id).collect()
    }

    /// Nível de uma entrada (só para os testes).
    pub fn nivel_de(&self, nome: &str) -> Option<AccessLevel> {
        self.entrada(nome).map(Entry::level)
    }

    /// O PIN confere com o hash guardado pelo `criar_cofre` (só para os testes).
    pub fn pin_confere(&self, pin: &[u8]) -> bool {
        self.hash_pin.as_ref().is_some_and(|h| h.verify(pin))
    }

    fn entrada(&self, nome: &str) -> Option<&Entry> {
        self.entradas.iter().find(|e| e.name() == nome)
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
                Pedido::CriarCofre { senha: _, pin } => match PinHash::create(pin.expose()) {
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
                        Some(v) => Resposta::Segredo(SegredoCore::from_bytes(v.expose().to_vec())),
                        None => Resposta::Erro(ErroResposta::CampoInexistente),
                    },
                    None => Resposta::Erro(ErroResposta::EntradaInexistente),
                }
            }
            Pedido::Totp { entrada } => match self.entrada(&entrada) {
                Some(e) => match Alvo::Campo("totp_semente".to_owned()).segredo_em(e) {
                    Some(semente) => match Totp::from_seed(semente).and_then(|t| t.code_now()) {
                        Ok(c) => Resposta::CodigoTotp {
                            codigo: Zeroizing::new(c.code().to_owned()),
                            restante: c.remaining(),
                        },
                        Err(_) => Resposta::Erro(ErroResposta::Interno),
                    },
                    None => Resposta::Erro(ErroResposta::CampoInexistente),
                },
                None => Resposta::Erro(ErroResposta::EntradaInexistente),
            },
            Pedido::SalvarEntrada {
                anterior,
                rascunho,
                manter_anexos,
            } => match self.salvar(anterior.as_deref(), rascunho, &manter_anexos) {
                Ok(()) => Resposta::Ok,
                Err(e) => Resposta::Erro(e),
            },
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
                entrada_confirmada,
            } => {
                match self.responder_aprovacao(pedido, decisao, pin.as_ref(), entrada_confirmada) {
                    Ok(()) => Resposta::Ok,
                    Err(e) => Resposta::Erro(e),
                }
            }
            Pedido::EmitirToken { entrada } => match self.entrada(&entrada) {
                Some(e) if e.kind() == EntryType::Server => {
                    match ward_core::generator::generate(&ward_core::generator::GeneratorOptions {
                        size: 32,
                        lowercase: true,
                        uppercase: true,
                        digits: true,
                        symbols: false,
                    }) {
                        Ok(t) => Resposta::Token(t),
                        Err(_) => Resposta::Erro(ErroResposta::Interno),
                    }
                }
                Some(_) => Resposta::Erro(ErroResposta::PedidoInvalido),
                None => Resposta::Erro(ErroResposta::EntradaInexistente),
            },
            // Fase 38: a configuração e as trocas (sem hash nem UUID na resposta).
            Pedido::Configuracao => Resposta::Configuracao {
                minutos_trava: self.minutos_trava,
                backup_externo: self.backup.enabled(),
                backup_local: self.backup_local,
            },
            // Fase 48: a mesma regra do `wardd` (Fase 46): só o formato de D45; ligado
            // exige UUID. Nada é copiado (não há pendrive de verdade).
            Pedido::BackupExterno => Resposta::BackupExterno {
                ligado: self.backup.enabled(),
                uuid: self.backup.uuid().map(str::to_owned),
            },
            Pedido::DefinirBackupExterno { ligado, uuid } => {
                match wc::config::ExternalBackup::new(ligado, uuid.as_deref()) {
                    Ok(b) => {
                        self.backup = b;
                        Resposta::Ok
                    }
                    Err(_) => Resposta::Erro(ErroResposta::PedidoInvalido),
                }
            }
            Pedido::Pendrives => match &self.pendrives {
                Some(l) => Resposta::Pendrives(l.clone()),
                None => Resposta::Erro(ErroResposta::PendrivesIndisponiveis),
            },
            // Fase 47: a mesma regra do `wardd` (Fase 45).
            Pedido::Backups => match &self.backups {
                // A mesma regra do `wardd` (Fase 49a): a geração fica aqui; só as marcas.
                Some(l) => {
                    let com: Vec<_> = l
                        .iter()
                        .filter_map(|b| Some((wc::backup::Versao::de_nome(&b.nome)?, b.geracao)))
                        .collect();
                    let marcas = wc::backup::fora_de_ordem(&com);
                    Resposta::Backups(
                        com.iter()
                            .zip(marcas)
                            .map(|((v, g), fora)| wc::protocolo::VersaoBackup {
                                nome: v.nome(),
                                conferida: g.is_some(),
                                fora_de_ordem: fora,
                            })
                            .collect(),
                    )
                }
                None => Resposta::Erro(ErroResposta::BackupIndisponivel),
            },
            Pedido::RestaurarBackup { versao, senha, pin } => {
                match self.restaurar(&versao, &senha, &pin) {
                    Ok(()) => Resposta::Ok,
                    Err(e) => Resposta::Erro(e),
                }
            }
            Pedido::TempoTrava { minutos } => {
                if (wc::limits::MIN_LOCK_MINUTES..=wc::limits::MAX_LOCK_MINUTES).contains(&minutos)
                {
                    self.minutos_trava = minutos;
                    Resposta::Ok
                } else {
                    Resposta::Erro(ErroResposta::PedidoInvalido)
                }
            }
            Pedido::TrocarPin { atual, novo } => match self.trocar_pin(&atual, &novo) {
                Ok(()) => Resposta::Ok,
                Err(e) => Resposta::Erro(e),
            },
            Pedido::TrocarSenha { atual, nova } => match self.trocar_senha(&atual, &nova) {
                Ok(()) => Resposta::Ok,
                Err(e) => Resposta::Erro(e),
            },
            _ => Resposta::Erro(ErroResposta::Interno),
        }
    }

    /// A mesma regra do `wardd` (`crates/wardd/src/nucleo.rs` › `conferir_pin`): hash
    /// Argon2id da biblioteca (tempo constante); o 5º erro seguido trava.
    fn conferir_pin(&mut self, pin: &SegredoCore) -> Result<(), ErroResposta> {
        let confere = match &self.hash_pin {
            Some(h) => h.verify(pin.expose()),
            None => cofre_teste::configuracao_de_teste().verify_pin(pin.expose()),
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

    /// A mesma regra do `wardd` (`nucleo.rs` › `trocar_pin`): o PIN atual primeiro; o
    /// novo vira só o hash (D30).
    fn trocar_pin(&mut self, atual: &SegredoCore, novo: &SegredoCore) -> Result<(), ErroResposta> {
        self.conferir_pin(atual)?;
        self.hash_pin =
            Some(PinHash::create(novo.expose()).map_err(|_| ErroResposta::PedidoInvalido)?);
        Ok(())
    }

    /// A mesma ordem do `wardd` (`nucleo.rs` › `restaurar_backup`): o nome no formato, o
    /// PIN atual (conta para os 5 que travam), a versão na pasta e a senha dela (tempo
    /// constante). O atual vira a versão mais nova (com a senha de agora); o cofre passa a
    /// ter as entradas, a senha e o PIN da versão (o de teste), sem liberações pendentes.
    fn restaurar(
        &mut self,
        versao: &str,
        senha: &SegredoCore,
        pin: &SegredoCore,
    ) -> Result<(), ErroResposta> {
        use ctutils::CtEq;
        if !wc::backup::nome_valido(versao) {
            return Err(ErroResposta::PedidoInvalido);
        }
        self.conferir_pin(pin)?;
        let Some(lista) = self.backups.as_mut() else {
            return Err(ErroResposta::BackupIndisponivel);
        };
        let Some(i) = lista.iter().position(|b| b.nome == versao) else {
            return Err(ErroResposta::BackupInexistente);
        };
        let s = senha.expose();
        let certa = &lista[i].senha;
        if s.len() != certa.len() || !s.ct_eq(&certa[..]).to_bool() {
            return Err(ErroResposta::SenhaIncorreta);
        }
        let senha_da_versao = certa.clone();
        let entradas = match &lista[i].entradas {
            EntradasDoBackup::Teste { menos } => cofre_de_teste_menos(*menos),
            EntradasDoBackup::Guardadas(_) => match lista.remove(i).entradas {
                EntradasDoBackup::Guardadas(e) => e,
                EntradasDoBackup::Teste { menos } => cofre_de_teste_menos(menos),
            },
        };
        let agora = wc::backup::Versao::do_instante(std::time::SystemTime::now())
            .ok_or(ErroResposta::Interno)?
            .nome();
        let senha_atual = match &self.senha {
            Some(s) => s.clone(),
            None => Zeroizing::new(cofre_teste::SENHA_MESTRA.as_bytes().to_vec()),
        };
        let atual = BackupFalso {
            nome: agora,
            geracao: Some(self.geracao),
            senha: senha_atual,
            entradas: EntradasDoBackup::Guardadas(std::mem::replace(&mut self.entradas, entradas)),
        };
        let lista = self.backups.get_or_insert_with(Vec::new);
        lista.retain(|b| b.nome != atual.nome);
        lista.insert(0, atual);
        lista.truncate(VERSOES_FALSAS);
        self.geracao = self.geracao.saturating_add(1);
        self.senha = Some(senha_da_versao);
        self.hash_pin = None;
        self.erros_pin = 0;
        self.aprovacoes.clear();
        Ok(())
    }

    /// A mesma resposta do `wardd` (`nucleo.rs` › `trocar_senha`, D52): a atual errada é
    /// `senha_incorreta` e não trava.
    fn trocar_senha(
        &mut self,
        atual: &SegredoCore,
        nova: &SegredoCore,
    ) -> Result<(), ErroResposta> {
        if !self.senha_confere(atual) {
            return Err(ErroResposta::SenhaIncorreta);
        }
        self.senha = Some(Zeroizing::new(nova.expose().to_vec()));
        Ok(())
    }

    /// A mesma regra do `wardd` (`uso.rs` › `responder_aprovacao`): pedido desconhecido
    /// é `expirado`; "Negar" sem PIN; PIN errado deixa pendente, e o 5º trava e nega;
    /// "sempre" deixa livre as entradas do pedido que estão "com aprovação". Fase 54b:
    /// um "Permitir" de pedido com a entrada padrão oculta só com `entrada_confirmada`
    /// (antes do PIN), e `entrada_confirmada` só nesse pedido.
    fn responder_aprovacao(
        &mut self,
        id: u64,
        decisao: Decisao,
        pin: Option<&SegredoCore>,
        entrada_confirmada: bool,
    ) -> Result<(), ErroResposta> {
        let i = self
            .aprovacoes
            .iter()
            .position(|(p, _, _)| *p == id)
            .ok_or(ErroResposta::Expirado)?;
        if decisao == Decisao::Negar {
            self.aprovacoes.remove(i);
            return Ok(());
        }
        // Fase 44c: a confirmação da origem não tem PIN (o falso não sabe que pedido é;
        // quem confere é o `wardd` e o serviço do app).
        if decisao == Decisao::Confirmar {
            self.aprovacoes.remove(i);
            return Ok(());
        }
        if self.aprovacoes[i].2 != entrada_confirmada {
            return Err(ErroResposta::PedidoInvalido);
        }
        let pin = pin.ok_or(ErroResposta::PedidoInvalido)?;
        if let Err(e) = self.conferir_pin(pin) {
            if self.travado {
                self.aprovacoes.remove(i);
            }
            return Err(e);
        }
        let (_, entradas, _) = self.aprovacoes.remove(i);
        if decisao == Decisao::PermitirSempre {
            for e in self.entradas.iter_mut() {
                if entradas.iter().any(|n| n == e.name()) && e.level() == AccessLevel::Approval {
                    e.set_level(AccessLevel::Free);
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
        nivel: AccessLevel,
        pin: &SegredoCore,
    ) -> Result<(), ErroResposta> {
        self.conferir_pin(pin)?;
        let e = self
            .entradas
            .iter_mut()
            .find(|e| e.name() == nome)
            .ok_or(ErroResposta::EntradaInexistente)?;
        e.set_level(nivel);
        e.set_dates(alterado(e.dates()));
        Ok(())
    }

    /// A mesma regra do `wardd` (`crates/wardd/src/nucleo.rs` › `salvar_entrada`).
    fn salvar(
        &mut self,
        anterior: Option<&str>,
        mut rascunho: Draft,
        manter_anexos: &[String],
    ) -> Result<(), ErroResposta> {
        // Fase 39b: a mesma regra do `wardd` (`nucleo.rs` › `salvar_entrada`).
        if !manter_anexos.is_empty() {
            let nome = anterior.ok_or(ErroResposta::PedidoInvalido)?;
            let antiga = self.entrada(nome).ok_or(ErroResposta::EntradaInexistente)?;
            protocolo::juntar_anexos_mantidos(&mut rascunho, antiga, manter_anexos)?;
        }
        let mut nova = rascunho
            .validate()
            .map_err(|e| ErroResposta::de_validacao(&e))?;
        match anterior {
            None => {
                nova.set_level(AccessLevel::Approval);
                nova.set_dates(agora().and_then(Dates::new_stamp).unwrap_or(Dates::NONE));
                nova.verify_in_vault(self.entradas.iter())
                    .map_err(|e| ErroResposta::de_validacao(&e))?;
            }
            Some(nome) => {
                let i = self
                    .entradas
                    .iter()
                    .position(|e| e.name() == nome)
                    .ok_or(ErroResposta::EntradaInexistente)?;
                nova.set_level(self.entradas[i].level());
                nova.set_dates(alterado(self.entradas[i].dates()));
                nova.verify_in_vault(self.entradas.iter().filter(|e| e.name() != nome))
                    .map_err(|e| ErroResposta::de_validacao(&e))?;
                drop(self.entradas.remove(i));
            }
        }
        self.entradas.push(nova);
        self.entradas.sort_by(|a, b| a.name().cmp(b.name()));
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
        .and_then(|d| Dates::instant(d.as_secs()))
}

/// As datas depois de uma alteração agora (as mesmas sem relógio), como no `wardd`.
fn alterado(datas: Dates) -> Dates {
    agora().and_then(|a| datas.modified(a)).unwrap_or(datas)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn mesmas_entradas_das_fixtures() {
        // `dev/fixtures/entries.json` é a fonte das entradas do cofre de teste; o
        // daemon falso tem exatamente os mesmos nomes, tipos, grupos, ambientes e níveis.
        let caminho = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dev/fixtures/entries.json"
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
                    e.name().to_owned(),
                    e.kind().name().to_owned(),
                    e.group().to_owned(),
                    e.environment().name().to_owned(),
                    e.level().name().to_owned(),
                )
            })
            .collect();
        esperado.sort();
        falso.sort();
        assert_eq!(falso, esperado);
    }

    #[test]
    fn pedidos_com_origem_usam_a_entrada_de_verdade() {
        // Fase 44e: o ambiente e a origem dos modos `aprovacao-origem*` vêm da entrada
        // do cofre de teste (`acme-wp-dev` é dev; `acme-wp-prod`, prod), como no `wardd`.
        use ward_core::entry::Environment;
        for (modo, ambiente, confirmacao) in [
            ("aprovacao-origem", Environment::Prod, true),
            ("aprovacao-origem-popup", Environment::Prod, true),
            ("aprovacao-origem-pin", Environment::Dev, false),
        ] {
            let v = aprovacoes_simuladas(modo).unwrap();
            let p = &v[0].0;
            assert_eq!(p.ambiente, ambiente, "{modo}");
            assert_eq!(p.confirmacao, confirmacao, "{modo}");
            assert_eq!(p.origem, "http://localhost:8081", "{modo}");
            assert!(p.comando.is_empty() && p.diretorio.is_empty(), "{modo}");
        }
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
