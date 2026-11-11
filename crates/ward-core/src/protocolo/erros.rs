//! Respostas de erro do protocolo. Cada código tem uma mensagem fixa em português; nenhum
//! erro leva valor do usuário, contagem de tentativas nem geração do cofre (S13, D20,
//! D39). Na leitura, a mensagem que veio no quadro é ignorada e volta a fixa do código:
//! nem um servidor falso consegue pôr texto próprio na tela do cliente por aqui.

use core::fmt;

#[cfg(unix)]
use crate::atomic_write::WriteError;
use crate::corpo::ErroAbrir;
use crate::entrada::ErrosValidacao;

/// Um problema de validação da entrada (campo, posição na lista e motivo), copiado de
/// [`crate::entrada::ErroValidacao`], que só tem textos fixos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problema {
    pub campo: String,
    pub posicao: Option<u64>,
    pub motivo: String,
}

/// Erro devolvido ao cliente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroResposta {
    /// A operação não é do papel do cliente (D5), ou o pedido de outro papel.
    NaoAutorizado,
    /// O cofre está travado.
    Travado,
    /// O pedido esperou o destrave e o tempo acabou (D16).
    TravadoTempoEsgotado,
    /// O quadro não é um pedido válido (JSON, campo a mais, tamanho, formato).
    PedidoInvalido,
    /// A operação não existe.
    OperacaoDesconhecida,
    /// Versão do protocolo diferente da deste build.
    VersaoDesconhecida,
    /// O quadro passou do limite do papel ou veio vazio (V3.4).
    QuadroInvalido,
    /// O daemon está atendendo conexões demais.
    Ocupado,
    /// Senha errada ou arquivo adulterado (o formato não distingue, de propósito).
    SenhaIncorreta,
    /// Decifrou, mas o conteúdo não é um cofre válido.
    CofreInvalido,
    /// Ainda não existe cofre (primeiro uso).
    CofreInexistente,
    /// Já existe um cofre.
    CofreJaExiste,
    /// D39: o cofre no disco tem geração menor que a maior já vista.
    Retrocesso,
    /// D39: o arquivo do cofre mudou fora do Ward (dev, inode, `mtime` ou tamanho).
    CofreMudou,
    /// D39: a geração chegou a `u64::MAX`; nada foi gravado.
    GeracaoEsgotada,
    /// Falha ao gravar o cofre (o arquivo de antes continua).
    FalhaAoGravar,
    /// PIN errado.
    PinIncorreto,
    /// Entrada que não existe ou que o papel não pode ver: para o agente, uma
    /// bloqueada responde igual a uma inexistente (V3.17).
    EntradaInexistente,
    /// O campo pedido não existe na entrada.
    CampoInexistente,
    /// A aprovação foi negada (ou a janela fechou).
    Negado,
    /// A aprovação expirou em 2 minutos (S10).
    Expirado,
    /// Não há como pedir aprovação agora (D21).
    AprovacaoIndisponivel,
    /// A origem da aba não bate com a da entrada (D9).
    OrigemDiferente,
    /// `ward playwright` (Fase 44): a página da entrada não tem um formulário de login
    /// que dê para preencher com segurança, ou o site não aceitou o login.
    LoginRecusado,
    /// `ward playwright` (Fase 44): o Chrome não abriu, caiu ou não respondeu a tempo.
    NavegadorFalhou,
    /// `ward playwright` (Fase 44, S1): a sessão do site guardou a senha (ou uma
    /// variação dela) em cookie ou `localStorage`; nada foi gravado.
    SessaoComSegredo,
    /// A entrada não passou na validação.
    Validacao(Vec<Problema>),
    /// `ward run` de um tipo sem entrega (`nota segura`, D10): nada é executado.
    TipoSemEntrega,
    /// A entrega da credencial não pôde ser montada (arquivo temporário, código TOTP,
    /// túnel): nada é executado.
    EntregaFalhou,
    /// A versão de backup pedida não existe na pasta (Fase 45).
    BackupInexistente,
    /// A pasta de backups não está acessível ou não é um diretório 0700 do usuário, ou
    /// o cofre atual não pôde ser guardado antes de restaurar (Fase 45).
    BackupIndisponivel,
    /// O udisks2 não respondeu: não dá para ver os pendrives (Fase 46).
    PendrivesIndisponiveis,
    /// Erro interno do daemon (sem detalhe).
    Interno,
}

/// Mensagens fixas que outras partes conferem (D39).
pub const MSG_RETROCESSO: &str = "o cofre no disco é mais antigo que o último aberto";
/// Mensagem fixa de [`ErroResposta::CofreMudou`] (D39).
pub const MSG_COFRE_MUDOU: &str = "o arquivo do cofre mudou fora do Ward";
/// Mensagem fixa de [`ErroResposta::GeracaoEsgotada`] (igual à de
/// [`crate::atomic_write::WriteError::GenerationExhausted`]).
pub const MSG_GERACAO_ESGOTADA: &str =
    "recusado gravar: o contador de versões do cofre chegou ao fim";

impl ErroResposta {
    /// Todos os códigos sem dado (o de validação fica de fora).
    pub const SIMPLES: [ErroResposta; 32] = [
        Self::NaoAutorizado,
        Self::Travado,
        Self::TravadoTempoEsgotado,
        Self::PedidoInvalido,
        Self::OperacaoDesconhecida,
        Self::VersaoDesconhecida,
        Self::QuadroInvalido,
        Self::Ocupado,
        Self::SenhaIncorreta,
        Self::CofreInvalido,
        Self::CofreInexistente,
        Self::CofreJaExiste,
        Self::Retrocesso,
        Self::CofreMudou,
        Self::GeracaoEsgotada,
        Self::FalhaAoGravar,
        Self::PinIncorreto,
        Self::EntradaInexistente,
        Self::CampoInexistente,
        Self::Negado,
        Self::Expirado,
        Self::AprovacaoIndisponivel,
        Self::OrigemDiferente,
        Self::LoginRecusado,
        Self::NavegadorFalhou,
        Self::SessaoComSegredo,
        Self::TipoSemEntrega,
        Self::EntregaFalhou,
        Self::BackupInexistente,
        Self::BackupIndisponivel,
        Self::PendrivesIndisponiveis,
        Self::Interno,
    ];

    /// Código estável do quadro.
    pub fn codigo(&self) -> &'static str {
        match self {
            Self::NaoAutorizado => "nao_autorizado",
            Self::Travado => "travado",
            Self::TravadoTempoEsgotado => "travado_tempo_esgotado",
            Self::PedidoInvalido => "pedido_invalido",
            Self::OperacaoDesconhecida => "operacao_desconhecida",
            Self::VersaoDesconhecida => "versao_desconhecida",
            Self::QuadroInvalido => "quadro_invalido",
            Self::Ocupado => "ocupado",
            Self::SenhaIncorreta => "senha_incorreta",
            Self::CofreInvalido => "cofre_invalido",
            Self::CofreInexistente => "cofre_inexistente",
            Self::CofreJaExiste => "cofre_ja_existe",
            Self::Retrocesso => "retrocesso",
            Self::CofreMudou => "cofre_mudou",
            Self::GeracaoEsgotada => "geracao_esgotada",
            Self::FalhaAoGravar => "falha_ao_gravar",
            Self::PinIncorreto => "pin_incorreto",
            Self::EntradaInexistente => "entrada_inexistente",
            Self::CampoInexistente => "campo_inexistente",
            Self::Negado => "negado",
            Self::Expirado => "expirado",
            Self::AprovacaoIndisponivel => "aprovacao_indisponivel",
            Self::OrigemDiferente => "origem_diferente",
            Self::LoginRecusado => "login_recusado",
            Self::NavegadorFalhou => "navegador_falhou",
            Self::SessaoComSegredo => "sessao_com_segredo",
            Self::Validacao(_) => "validacao",
            Self::TipoSemEntrega => "tipo_sem_entrega",
            Self::EntregaFalhou => "entrega_falhou",
            Self::BackupInexistente => "backup_inexistente",
            Self::BackupIndisponivel => "backup_indisponivel",
            Self::PendrivesIndisponiveis => "pendrives_indisponiveis",
            Self::Interno => "interno",
        }
    }

    /// Mensagem fixa, em português, sem valor nenhum.
    pub fn mensagem(&self) -> &'static str {
        match self {
            Self::NaoAutorizado => "não autorizado",
            Self::Travado => "travado",
            Self::TravadoTempoEsgotado => "travado: tempo esgotado",
            Self::PedidoInvalido => "pedido inválido",
            Self::OperacaoDesconhecida => "operação desconhecida",
            Self::VersaoDesconhecida => "versão do protocolo desconhecida",
            Self::QuadroInvalido => "quadro vazio ou grande demais",
            Self::Ocupado => "o Ward está ocupado; tente de novo",
            Self::SenhaIncorreta => "senha mestra incorreta ou arquivo do cofre adulterado",
            Self::CofreInvalido => "o conteúdo do cofre é inválido",
            Self::CofreInexistente => "ainda não existe cofre",
            Self::CofreJaExiste => "o cofre já existe",
            Self::Retrocesso => MSG_RETROCESSO,
            Self::CofreMudou => MSG_COFRE_MUDOU,
            Self::GeracaoEsgotada => MSG_GERACAO_ESGOTADA,
            Self::FalhaAoGravar => "falha ao gravar o cofre; a versão anterior foi mantida",
            Self::PinIncorreto => "PIN incorreto",
            Self::EntradaInexistente => "entrada inexistente",
            Self::CampoInexistente => "campo inexistente",
            Self::Negado => "pedido negado",
            Self::Expirado => "pedido expirado",
            Self::AprovacaoIndisponivel => "aprovação indisponível",
            Self::OrigemDiferente => "a origem da página não bate com a da entrada",
            Self::LoginRecusado => {
                "a página não tem um formulário de login seguro ou o site não aceitou o login"
            }
            Self::NavegadorFalhou => "o navegador não abriu ou não respondeu a tempo",
            Self::SessaoComSegredo => "a sessão do site guarda a senha; nada foi gravado",
            Self::Validacao(_) => "entrada inválida",
            Self::TipoSemEntrega => "tipo sem entrega",
            Self::EntregaFalhou => "a entrega da credencial falhou; nada foi executado",
            Self::BackupInexistente => "a versão do backup não existe",
            Self::BackupIndisponivel => "a pasta de backups não está disponível ou não é segura",
            Self::PendrivesIndisponiveis => {
                "não deu para ver os pendrives (o udisks não respondeu)"
            }
            Self::Interno => "erro interno do Ward",
        }
    }

    /// O erro de um código lido do quadro. A validação volta sem problemas; quem lê
    /// preenche.
    pub(crate) fn de_codigo(codigo: &str) -> Option<Self> {
        if codigo == "validacao" {
            return Some(Self::Validacao(Vec::new()));
        }
        Self::SIMPLES.into_iter().find(|e| e.codigo() == codigo)
    }

    /// Erro de validação a partir dos problemas da entrada (só textos fixos).
    pub fn de_validacao(erros: &ErrosValidacao) -> Self {
        Self::Validacao(
            erros
                .lista()
                .iter()
                .map(|e| Problema {
                    campo: e.campo.to_owned(),
                    posicao: e.posicao.map(|p| p as u64),
                    motivo: e.motivo.to_owned(),
                })
                .collect(),
        )
    }
}

impl fmt::Display for ErroResposta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.mensagem())
    }
}

impl std::error::Error for ErroResposta {}

/// Erro do destravar (D39): `Retrocesso` vira o aviso próprio, sem as gerações.
impl From<&ErroAbrir> for ErroResposta {
    fn from(e: &ErroAbrir) -> Self {
        match e {
            ErroAbrir::Formato(_) => Self::SenhaIncorreta,
            ErroAbrir::Corpo(_) => Self::CofreInvalido,
            ErroAbrir::Retrocesso => Self::Retrocesso,
        }
    }
}

/// Erro das gravações (D39): `GeracaoEsgotada` e `Retrocesso` (troca de senha) com as
/// mensagens próprias; o resto, sem detalhe. Na troca de senha, `Formato` também é a
/// senha atual errada: o daemon, que sabe qual operação fez, responde
/// [`ErroResposta::SenhaIncorreta`] nesse caso.
#[cfg(unix)]
impl From<&WriteError> for ErroResposta {
    fn from(e: &WriteError) -> Self {
        match e {
            WriteError::GenerationExhausted => Self::GeracaoEsgotada,
            WriteError::Rollback => Self::Retrocesso,
            WriteError::Format(_)
            | WriteError::NotVault
            | WriteError::Body(_)
            | WriteError::InvalidDestination
            | WriteError::Io { .. } => Self::FalhaAoGravar,
        }
    }
}
