//! Os pedidos do app ao daemon pelo protocolo da Fase 20 (`ward_core::protocolo`), com
//! o papel app (D5): `estado`, `lista`, `detalhe`, `copiar`, `revelar`, `totp`,
//! `salvar_entrada`, `mudar_nivel` (Fase 35), `destravar` (Fase 31) e, na tela
//! Configurações (Fase 38), `configuracao`, `trocar_pin`, `trocar_senha` e `tempo_trava`;
//! no backup externo (Fase 48), `backup_externo`, `pendrives` e `definir_backup_externo`;
//! no restaurar (Fase 47), `backups` e `restaurar_backup`.
//!
//! Aqui o pedido vira o corpo do quadro (`codificar_pedido`) e a resposta é lida com a
//! leitura estrita do cliente (`decodificar_resposta`), que recusa tipo de outro papel e
//! detalhe forjado (extra secreto com valor, campo secreto em `campos`). O transporte
//! é o [`Daemon`]: o socket do `wardd` ([`crate::daemon::DaemonSocket`]) ou, nos testes
//! e na ponte de teste, o daemon falso ([`crate::daemon_falso`]).
//!
//! Nenhuma função daqui devolve segredo à interface: [`copiar`] e [`revelar`] leem um
//! valor em [`Secret`] (que zera ao sair); quem decide o destino é o
//! [`crate::service`]: a área de transferência (D4), o "revelar" no clique (S11) ou o
//! rascunho que volta ao daemon ao editar (Fase 33).

use ward_core::entry::{AccessLevel, Draft, Secret as SegredoCore};
use ward_core::protocolo::{
    self, Alvo, DetalheEntrada, ErroResposta, ItemLista, Papel, Pedido, Pendrive, Problema,
    Resposta,
};
use zeroize::Zeroizing;

use crate::AppError;
use crate::daemon::{Daemon, EstadoCofre};
use crate::secret::Secret;
use crate::terminal::Token;

/// Um pedido e a resposta já lida. Um `erro` do daemon vira [`AppError`] com mensagem
/// fixa (o código do protocolo não vai à interface).
fn pedir<D: Daemon + ?Sized>(daemon: &mut D, pedido: &Pedido) -> Result<Resposta, AppError> {
    let quadro =
        protocolo::codificar_pedido(Papel::App, pedido).map_err(|_| AppError::InvalidRequest)?;
    let corpo = daemon.trocar(&quadro)?;
    match protocolo::decodificar_resposta(Papel::App, &corpo) {
        Ok(Resposta::Erro(e)) => Err(erro_do_daemon(&e)),
        Ok(r) => Ok(r),
        Err(_) => Err(AppError::InvalidResponse),
    }
}

/// O erro do protocolo que a tela distingue; o resto é "recusado".
fn erro_do_daemon(e: &ErroResposta) -> AppError {
    match e {
        ErroResposta::Travado | ErroResposta::TravadoTempoEsgotado => AppError::Locked,
        ErroResposta::EntradaInexistente => AppError::EntryNotFound,
        ErroResposta::CampoInexistente => AppError::FieldNotFound,
        ErroResposta::NaoAutorizado => AppError::Unauthorized,
        ErroResposta::CofreJaExiste => AppError::VaultAlreadyExists,
        ErroResposta::FalhaAoGravar => AppError::VaultNotWritten,
        ErroResposta::PinIncorreto => AppError::WrongPin,
        ErroResposta::SenhaIncorreta => AppError::WrongPassword,
        ErroResposta::Retrocesso => AppError::VaultRollback,
        ErroResposta::PendrivesIndisponiveis => AppError::UsbDrivesUnavailable,
        ErroResposta::BackupInexistente => AppError::BackupNotFound,
        ErroResposta::BackupIndisponivel => AppError::BackupUnavailable,
        ErroResposta::CofreMudou => AppError::VaultChanged,
        _ => AppError::Refused,
    }
}

/// `estado`.
pub fn estado<D: Daemon + ?Sized>(daemon: &mut D) -> Result<EstadoCofre, AppError> {
    match pedir(daemon, &Pedido::Estado)? {
        Resposta::Estado {
            cofre_existe: false,
            ..
        } => Ok(EstadoCofre::SemCofre),
        Resposta::Estado { travado: true, .. } => Ok(EstadoCofre::Travado),
        Resposta::Estado { travado: false, .. } => Ok(EstadoCofre::Destravado),
        _ => Err(AppError::InvalidResponse),
    }
}

/// `criar_cofre` (primeiro uso, Fase 30): a senha mestra e o PIN já conferidos por
/// [`crate::primeiro_uso`]. O daemon grava o hash do PIN na configuração do corpo
/// (D30) e deixa o cofre destravado. Os valores vão em `Secret` do núcleo (zera ao
/// sair) e o quadro é zerado depois da troca.
pub fn criar_cofre<D: Daemon + ?Sized>(
    daemon: &mut D,
    senha: &[u8],
    pin: &[u8],
) -> Result<(), AppError> {
    let pedido = Pedido::CriarCofre {
        senha: SegredoCore::from_bytes(senha.to_vec()),
        pin: SegredoCore::from_bytes(pin.to_vec()),
    };
    match pedir(daemon, &pedido)? {
        Resposta::Ok => Ok(()),
        _ => Err(AppError::InvalidResponse),
    }
}

/// O papel que o daemon deu a esta conexão, pela resposta de `estado` (D63). O app e a
/// ponte de teste pedem isto **primeiro** e, sem [`Papel::App`], não usam a conexão.
pub fn papel<D: Daemon + ?Sized>(daemon: &mut D) -> Result<Papel, AppError> {
    match pedir(daemon, &Pedido::Estado)? {
        Resposta::Estado { papel, .. } => Ok(papel),
        _ => Err(AppError::InvalidResponse),
    }
}

/// O papel desta conexão é o de app? (O mesmo de [`papel`], para quem não depende do
/// `ward-core`, como a ponte de teste.)
pub fn tem_papel_app<D: Daemon + ?Sized>(daemon: &mut D) -> Result<bool, AppError> {
    papel(daemon).map(|p| p == Papel::App)
}

/// `abrir_app` (D57): pede ao daemon que abra a instância verdadeira do app. Sem
/// campo: nada do pedido chega ao processo que o daemon abre.
pub fn abrir_app<D: Daemon + ?Sized>(daemon: &mut D) -> Result<(), AppError> {
    match pedir(daemon, &Pedido::AbrirApp)? {
        Resposta::Ok => Ok(()),
        _ => Err(AppError::InvalidResponse),
    }
}

/// `abrir_app` pela conexão de um cliente comum (o app aberto pelo menu, que largou o
/// grupo antes de conectar): o daemon escreve a resposta no papel que deu, o de agente
/// (D63), e é nele que ela é lida.
pub fn abrir_app_como_agente<D: Daemon + ?Sized>(daemon: &mut D) -> Result<(), AppError> {
    let quadro = protocolo::codificar_pedido(Papel::Agente, &Pedido::AbrirApp)
        .map_err(|_| AppError::InvalidRequest)?;
    let corpo = daemon.trocar(&quadro)?;
    match protocolo::decodificar_resposta(Papel::Agente, &corpo) {
        Ok(Resposta::Ok) => Ok(()),
        Ok(Resposta::Erro(e)) => Err(erro_do_daemon(&e)),
        _ => Err(AppError::InvalidResponse),
    }
}

/// `lista`: todas as entradas (o app vê também as bloqueadas), sem segredo.
pub fn lista<D: Daemon + ?Sized>(daemon: &mut D) -> Result<Vec<ItemLista>, AppError> {
    match pedir(daemon, &Pedido::Lista)? {
        Resposta::Lista(itens) => Ok(itens),
        _ => Err(AppError::InvalidResponse),
    }
}

/// `detalhe` de uma entrada: campos de texto, nomes dos segredos, extras sem o valor
/// secreto e anexos sem o conteúdo. A resposta tem de ser da entrada pedida.
pub fn detalhe<D: Daemon + ?Sized>(
    daemon: &mut D,
    entrada: &str,
) -> Result<DetalheEntrada, AppError> {
    let pedido = Pedido::Detalhe {
        entrada: entrada.to_owned(),
    };
    match pedir(daemon, &pedido)? {
        Resposta::Detalhe(d) if d.nome == entrada => Ok(d),
        _ => Err(AppError::InvalidResponse),
    }
}

/// `copiar`: o valor de um segredo, só para a área de transferência (D4).
pub fn copiar<D: Daemon + ?Sized>(
    daemon: &mut D,
    entrada: &str,
    alvo: Alvo,
) -> Result<Secret, AppError> {
    let pedido = Pedido::Copiar {
        entrada: entrada.to_owned(),
        alvo,
    };
    match pedir(daemon, &pedido)? {
        Resposta::Segredo(s) => Ok(Secret::new(s.expose().to_vec())),
        _ => Err(AppError::InvalidResponse),
    }
}

/// `revelar`: o valor de um segredo para mostrar no clique em "revelar" (S11) ou para
/// manter o valor ao editar a entrada (o rascunho leva todos os segredos).
pub fn revelar<D: Daemon + ?Sized>(
    daemon: &mut D,
    entrada: &str,
    alvo: Alvo,
) -> Result<Secret, AppError> {
    let pedido = Pedido::Revelar {
        entrada: entrada.to_owned(),
        alvo,
    };
    match pedir(daemon, &pedido)? {
        Resposta::Segredo(s) => Ok(Secret::new(s.expose().to_vec())),
        _ => Err(AppError::InvalidResponse),
    }
}

/// Código TOTP atual de uma entrada e os segundos até o próximo.
pub struct CodigoTotp {
    pub codigo: Zeroizing<String>,
    pub restante: u64,
}

/// `totp`: o código atual (6 dígitos, D35) e quanto falta para trocar (1 a 30 s).
pub fn totp<D: Daemon + ?Sized>(daemon: &mut D, entrada: &str) -> Result<CodigoTotp, AppError> {
    let pedido = Pedido::Totp {
        entrada: entrada.to_owned(),
    };
    match pedir(daemon, &pedido)? {
        Resposta::CodigoTotp { codigo, restante }
            if codigo.len() == 6 && (1..=30).contains(&restante) =>
        {
            Ok(CodigoTotp { codigo, restante })
        }
        _ => Err(AppError::InvalidResponse),
    }
}

/// Resultado do `salvar_entrada`: salvo, ou os problemas de validação por campo (só
/// textos fixos do `ward-core`; nenhum valor do usuário).
/// `emitir_token`: o token de uso único do "Conectar" (D3), conferido antes de ir a um
/// argv ([`Token::de_resposta`]).
pub fn emitir_token<D: Daemon + ?Sized>(daemon: &mut D, entrada: &str) -> Result<Token, AppError> {
    let pedido = Pedido::EmitirToken {
        entrada: entrada.to_owned(),
    };
    match pedir(daemon, &pedido)? {
        Resposta::Token(t) => Token::de_resposta(t.expose()).ok_or(AppError::InvalidResponse),
        _ => Err(AppError::InvalidResponse),
    }
}

pub enum Salvamento {
    Salvo,
    Problemas(Vec<Problema>),
}

/// `salvar_entrada`: cria (`anterior` vazio) ou edita. O nível não vai no pedido (S9).
/// `manter_anexos` (Fase 39b): nomes dos anexos da entrada editada que continuam nela;
/// o daemon copia o conteúdo, que nunca passa pelo app.
pub fn salvar<D: Daemon + ?Sized>(
    daemon: &mut D,
    anterior: Option<String>,
    rascunho: Draft,
    manter_anexos: Vec<String>,
) -> Result<Salvamento, AppError> {
    let pedido = Pedido::SalvarEntrada {
        anterior,
        rascunho,
        manter_anexos,
    };
    let quadro =
        protocolo::codificar_pedido(Papel::App, &pedido).map_err(|_| AppError::InvalidRequest)?;
    drop(pedido);
    let corpo = daemon.trocar(&quadro)?;
    drop(quadro);
    match protocolo::decodificar_resposta(Papel::App, &corpo) {
        Ok(Resposta::Ok) => Ok(Salvamento::Salvo),
        Ok(Resposta::Erro(ErroResposta::Validacao(p))) => Ok(Salvamento::Problemas(p)),
        Ok(Resposta::Erro(e)) => Err(erro_do_daemon(&e)),
        _ => Err(AppError::InvalidResponse),
    }
}

/// `mudar_nivel` (S9): o PIN vai só neste pedido. O quadro com o PIN é largado (e
/// zerado: `codificar_pedido` devolve `Zeroizing`) logo depois da troca. PIN errado dá
/// [`AppError::WrongPin`]; a contagem e a trava no 5º erro são do daemon.
pub fn mudar_nivel<D: Daemon + ?Sized>(
    daemon: &mut D,
    entrada: &str,
    nivel: AccessLevel,
    pin: &crate::nivel::Pin,
) -> Result<(), AppError> {
    let pedido = Pedido::MudarNivel {
        entrada: entrada.to_owned(),
        nivel,
        pin: pin.para_o_protocolo(),
    };
    match pedir(daemon, &pedido)? {
        Resposta::Ok => Ok(()),
        _ => Err(AppError::InvalidResponse),
    }
}

/// O que o daemon disse da resposta da janela de aprovação (Fase 36).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Respondida {
    Ok,
    PinIncorreto,
    /// O daemon já não tem o pedido (expirou, ou outra resposta chegou antes).
    Expirado,
}

/// `responder_aprovacao` (S10): o PIN vai só neste pedido, e só nos "Permitir" (o
/// protocolo recusa PIN no "Negar" e "Permitir" sem PIN). O quadro com o PIN é largado
/// (e zerado) logo depois da troca. A contagem de erros e a trava no 5º são do daemon.
pub fn responder_aprovacao<D: Daemon + ?Sized>(
    daemon: &mut D,
    id: u64,
    decisao: protocolo::Decisao,
    pin: Option<&crate::nivel::Pin>,
    entrada_confirmada: bool,
) -> Result<Respondida, AppError> {
    if decisao.pede_pin() != pin.is_some() || (entrada_confirmada && !decisao.pede_pin()) {
        return Err(AppError::InvalidRequest);
    }
    let pedido = Pedido::ResponderAprovacao {
        pedido: id,
        decisao,
        pin: pin.map(crate::nivel::Pin::para_o_protocolo),
        entrada_confirmada,
    };
    let quadro =
        protocolo::codificar_pedido(Papel::App, &pedido).map_err(|_| AppError::InvalidRequest)?;
    drop(pedido);
    let corpo = daemon.trocar(&quadro)?;
    drop(quadro);
    match protocolo::decodificar_resposta(Papel::App, &corpo) {
        Ok(Resposta::Ok) => Ok(Respondida::Ok),
        Ok(Resposta::Erro(ErroResposta::PinIncorreto)) => Ok(Respondida::PinIncorreto),
        Ok(Resposta::Erro(ErroResposta::Expirado)) => Ok(Respondida::Expirado),
        Ok(Resposta::Erro(e)) => Err(erro_do_daemon(&e)),
        Ok(_) => Err(AppError::InvalidResponse),
        Err(_) => Err(AppError::InvalidResponse),
    }
}

/// `destravar { senha }` (Fase 31): a senha mestra só vai ao pedido, num `Secret` do
/// núcleo (que zera). `ok` destrava; senha errada é [`AppError::WrongPassword`], sem
/// contagem (D20: a espera é do daemon, antes de olhar a senha).
pub fn destravar<D: Daemon + ?Sized>(
    daemon: &mut D,
    senha: &crate::destrave::SenhaMestra,
) -> Result<(), AppError> {
    let pedido = Pedido::Destravar {
        senha: senha.para_o_protocolo(),
    };
    match pedir(daemon, &pedido)? {
        Resposta::Ok => Ok(()),
        _ => Err(AppError::InvalidResponse),
    }
}

/// `configuracao` (Fase 38): tempo de trava e backup externo ligado ou não. A leitura
/// estrita já recusou minutos fora de D45 e campo a mais.
pub fn configuracao<D: Daemon + ?Sized>(
    daemon: &mut D,
) -> Result<crate::configuracoes::Configuracoes, AppError> {
    match pedir(daemon, &Pedido::Configuracao)? {
        Resposta::Configuracao {
            minutos_trava,
            backup_externo,
            backup_local,
        } => Ok(crate::configuracoes::Configuracoes {
            minutos_trava,
            backup_externo,
            backup_local,
        }),
        _ => Err(AppError::InvalidResponse),
    }
}

/// `trocar_pin { atual, novo }` (Fase 38, S9): os dois PINs já conferidos no formato por
/// [`crate::configuracoes`], em `Secret` do núcleo (zera). PIN atual errado é
/// [`AppError::WrongPin`]; a contagem e a trava no 5.º erro são do daemon.
pub fn trocar_pin<D: Daemon + ?Sized>(
    daemon: &mut D,
    atual: &[u8],
    novo: &[u8],
) -> Result<(), AppError> {
    let pedido = Pedido::TrocarPin {
        atual: SegredoCore::from_bytes(atual.to_vec()),
        novo: SegredoCore::from_bytes(novo.to_vec()),
    };
    match pedir(daemon, &pedido)? {
        Resposta::Ok => Ok(()),
        _ => Err(AppError::InvalidResponse),
    }
}

/// `trocar_senha { atual, nova }` (Fase 38): senha atual errada é
/// [`AppError::WrongPassword`] (D52); o daemon grava o cofre com a chave nova e geração
/// nova (D39, D53).
pub fn trocar_senha<D: Daemon + ?Sized>(
    daemon: &mut D,
    atual: &[u8],
    nova: &[u8],
) -> Result<(), AppError> {
    let pedido = Pedido::TrocarSenha {
        atual: SegredoCore::from_bytes(atual.to_vec()),
        nova: SegredoCore::from_bytes(nova.to_vec()),
    };
    match pedir(daemon, &pedido)? {
        Resposta::Ok => Ok(()),
        _ => Err(AppError::InvalidResponse),
    }
}

/// `tempo_trava { minutos }` (Fase 38, D30): o daemon grava na configuração do cofre.
pub fn tempo_trava<D: Daemon + ?Sized>(daemon: &mut D, minutos: u32) -> Result<(), AppError> {
    match pedir(daemon, &Pedido::TempoTrava { minutos })? {
        Resposta::Ok => Ok(()),
        _ => Err(AppError::InvalidResponse),
    }
}

/// `travar` ("Travar agora" da janela e da bandeja, Fase 39): sem campo. O daemon zera a
/// chave e responde `ok` (travado já, também `ok`).
pub fn travar<D: Daemon + ?Sized>(daemon: &mut D) -> Result<(), AppError> {
    match pedir(daemon, &Pedido::Travar)? {
        Resposta::Ok => Ok(()),
        _ => Err(AppError::InvalidResponse),
    }
}

/// `backup_externo` (Fase 46/48): ligado ou não e o UUID escolhido (a leitura estrita já
/// conferiu o formato de D45 e que ligado tem UUID).
pub fn backup_externo<D: Daemon + ?Sized>(
    daemon: &mut D,
) -> Result<(bool, Option<String>), AppError> {
    match pedir(daemon, &Pedido::BackupExterno)? {
        Resposta::BackupExterno { ligado, uuid } => Ok((ligado, uuid)),
        _ => Err(AppError::InvalidResponse),
    }
}

/// `pendrives` (Fase 46/48): UUID, rótulo (sem controle, até `MAX_ROTULO`) e montado;
/// nunca o caminho de montagem. O udisks2 sem resposta é
/// [`AppError::UsbDrivesUnavailable`].
pub fn pendrives<D: Daemon + ?Sized>(daemon: &mut D) -> Result<Vec<Pendrive>, AppError> {
    match pedir(daemon, &Pedido::Pendrives)? {
        Resposta::Pendrives(l) => Ok(l),
        _ => Err(AppError::InvalidResponse),
    }
}

/// `definir_backup_externo { ligado, uuid? }` (Fase 46/48): o daemon grava na
/// configuração do cofre (geração nova, D39).
pub fn definir_backup_externo<D: Daemon + ?Sized>(
    daemon: &mut D,
    ligado: bool,
    uuid: Option<String>,
) -> Result<(), AppError> {
    match pedir(daemon, &Pedido::DefinirBackupExterno { ligado, uuid })? {
        Resposta::Ok => Ok(()),
        _ => Err(AppError::InvalidResponse),
    }
}

/// `backups` (Fase 45/47/49a): as versões do backup local (nome e geração selada), da
/// mais nova para a mais antiga. A leitura estrita já recusou nome fora do formato,
/// geração 0, campo a mais e mais de `MAX_BACKUPS`.
pub fn backups<D: Daemon + ?Sized>(
    daemon: &mut D,
) -> Result<Vec<ward_core::protocolo::VersaoBackup>, AppError> {
    match pedir(daemon, &Pedido::Backups)? {
        Resposta::Backups(l) => Ok(l),
        _ => Err(AppError::InvalidResponse),
    }
}

/// `restaurar_backup { versao, senha, pin }` (Fase 45/47; D17, D39, S9): a senha mestra
/// daquela versão e o PIN atual vão só neste pedido, em `Secret` do núcleo (zera), e o
/// quadro é largado logo depois da troca. PIN errado é [`AppError::WrongPin`] (o
/// daemon conta; o 5.º trava); a senha da versão errada, [`AppError::WrongPassword`].
pub fn restaurar_backup<D: Daemon + ?Sized>(
    daemon: &mut D,
    versao: &str,
    senha: &crate::destrave::SenhaMestra,
    pin: &crate::nivel::Pin,
) -> Result<(), AppError> {
    let pedido = Pedido::RestaurarBackup {
        versao: versao.to_owned(),
        senha: senha.para_o_protocolo(),
        pin: pin.para_o_protocolo(),
    };
    let quadro =
        protocolo::codificar_pedido(Papel::App, &pedido).map_err(|_| AppError::InvalidRequest)?;
    drop(pedido);
    let corpo = daemon.trocar(&quadro)?;
    drop(quadro);
    match protocolo::decodificar_resposta(Papel::App, &corpo) {
        Ok(Resposta::Ok) => Ok(()),
        Ok(Resposta::Erro(e)) => Err(erro_do_daemon(&e)),
        _ => Err(AppError::InvalidResponse),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::daemon_falso::DaemonFalso;

    /// Daemon que devolve sempre o mesmo corpo (o "servidor" é o atacante).
    struct Resposta1(Vec<u8>);

    impl Daemon for Resposta1 {
        fn trocar(&mut self, _pedido: &[u8]) -> Result<Zeroizing<Vec<u8>>, AppError> {
            Ok(Zeroizing::new(self.0.clone()))
        }
    }

    #[test]
    fn estado_lista_e_detalhe_pelo_protocolo() {
        let mut d = DaemonFalso::cheio();
        assert_eq!(estado(&mut d), Ok(EstadoCofre::Destravado));
        let itens = lista(&mut d).unwrap();
        assert_eq!(
            itens.len(),
            serde_json::from_str::<serde_json::Value>(include_str!(
                "../../../dev/fixtures/entries.json"
            ))
            .unwrap()["entradas"]
                .as_array()
                .unwrap()
                .len()
        );
        let det = detalhe(&mut d, "acme-pg-dev").unwrap();
        assert_eq!(det.segredos, vec!["senha".to_owned()]);
        assert_eq!(
            detalhe(&mut d, "nao-existe").unwrap_err(),
            AppError::EntryNotFound
        );
        assert_eq!(
            estado(&mut DaemonFalso::vazio()),
            Ok(EstadoCofre::Destravado)
        );
        assert!(lista(&mut DaemonFalso::vazio()).unwrap().is_empty());
    }

    #[test]
    fn ataque_resposta_de_outro_tipo_ou_forjada_e_recusada() {
        // Um "daemon" que responde credencial ao pedido de lista, um detalhe de outra
        // entrada, um detalhe com o valor do extra secreto e uma resposta de outra
        // versão: tudo vira `RespostaInvalida`, nada chega à interface.
        let segredo_no_lugar_da_lista =
            br#"{"v":1,"tipo":"segredo","dados":{"valor":"ZmFsc28tUEc="}}"#.to_vec();
        assert_eq!(
            lista(&mut Resposta1(segredo_no_lugar_da_lista)).unwrap_err(),
            AppError::InvalidResponse
        );
        let mut falso = DaemonFalso::cheio();
        let quadro = protocolo::codificar_pedido(
            Papel::App,
            &Pedido::Detalhe {
                entrada: "acme-pg-dev".into(),
            },
        )
        .unwrap();
        let outro = falso.trocar(&quadro).unwrap().to_vec();
        assert_eq!(
            detalhe(&mut Resposta1(outro), "acme-pg-prod").unwrap_err(),
            AppError::InvalidResponse
        );
        // O detalhe verdadeiro de `gama-generico`, com o valor do extra secreto
        // enfiado pelo "daemon".
        let quadro = protocolo::codificar_pedido(
            Papel::App,
            &Pedido::Detalhe {
                entrada: "gama-generico".into(),
            },
        )
        .unwrap();
        let verdadeiro = String::from_utf8(falso.trocar(&quadro).unwrap().to_vec()).unwrap();
        let alvo = r#""segredo":true"#;
        assert!(verdadeiro.contains(alvo), "{verdadeiro}");
        let forjado = verdadeiro
            .replacen(alvo, r#""segredo":true,"valor":"falso-licenca-01""#, 1)
            .into_bytes();
        assert_eq!(
            detalhe(&mut Resposta1(forjado), "gama-generico").unwrap_err(),
            AppError::InvalidResponse
        );
        let versao = br#"{"v":2,"tipo":"lista","dados":{"itens":[]}}"#.to_vec();
        assert_eq!(
            lista(&mut Resposta1(versao)).unwrap_err(),
            AppError::InvalidResponse
        );
    }

    #[test]
    fn ataque_d65_lista_sem_resumo_ou_com_segredo_no_resumo_e_recusada() {
        // A lista do papel app traz o resumo de cada item (D65 c), sem segredo.
        let mut falso = DaemonFalso::cheio();
        let itens = lista(&mut falso).unwrap();
        assert!(
            itens
                .iter()
                .all(|i| i.resumo.is_some() && i.nivel.is_some())
        );
        let quadro = protocolo::codificar_pedido(Papel::App, &Pedido::Lista).unwrap();
        let verdadeira = String::from_utf8(falso.trocar(&quadro).unwrap().to_vec()).unwrap();
        for s in crate::daemon_falso::SEGREDOS {
            assert!(!verdadeira.contains(s), "segredo na lista do app");
        }
        // A lista do agente (sem nível nem resumo) não serve ao app.
        let do_agente: Vec<ItemLista> = itens
            .iter()
            .map(|i| ItemLista {
                nivel: None,
                resumo: None,
                ..i.clone()
            })
            .collect();
        let q = protocolo::codificar_resposta(Papel::Agente, &Resposta::Lista(do_agente))
            .unwrap()
            .to_vec();
        assert_eq!(
            lista(&mut Resposta1(q)).unwrap_err(),
            AppError::InvalidResponse
        );
        // Um "daemon" que enfia um segredo no resumo (campo secreto em `campos`, ou
        // chave a mais): recusado inteiro, nada chega à interface.
        let alvo = r#""resumo":{"#;
        assert!(verdadeira.contains(alvo));
        for troca in [
            r#""resumo":{"campos":{"senha":"falso-PG:s3nha/01@#"},"segredos":[],"extras":0,"anexos":0},"x":{"#,
            r#""resumo":{"valor":"ZmFsc28tUEc=","#,
        ] {
            let f = verdadeira.replacen(alvo, troca, 1).into_bytes();
            assert_eq!(
                lista(&mut Resposta1(f)).unwrap_err(),
                AppError::InvalidResponse,
                "{troca}"
            );
        }
    }

    #[test]
    fn erros_do_daemon_tem_mensagem_fixa() {
        let nao = br#"{"v":1,"tipo":"erro","dados":{"codigo":"nao_autorizado","mensagem":"<img src=x> falso-PG:s3nha/01@#"}}"#.to_vec();
        let e = lista(&mut Resposta1(nao)).unwrap_err();
        assert_eq!(e, AppError::Unauthorized);
        assert!(!e.to_string().contains("img"));
        assert!(!e.to_string().contains("falso"));
        let travado =
            br#"{"v":1,"tipo":"erro","dados":{"codigo":"travado","mensagem":"x"}}"#.to_vec();
        assert_eq!(
            estado(&mut Resposta1(travado)).unwrap_err(),
            AppError::Locked
        );
    }
}
