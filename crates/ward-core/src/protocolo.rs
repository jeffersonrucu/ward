//! Protocolo do socket do `wardd` (R1, D5, Fase 20): quadros com limite de tamanho,
//! mensagens versionadas em JSON estrito, papéis e a tabela operação × papel.
//!
//! # Quadro
//!
//! 4 bytes de tamanho (u32 big-endian) e o corpo, um objeto JSON em UTF-8
//! ([`quadro`]). O limite depende do papel, que o daemon tira da conexão
//! (`SO_PEERCRED`, Fase 21) **antes** de ler o quadro: 1 MiB para o cliente comum
//! (agente) e a extensão ([`limites::MAX_QUADRO_COMUM`], V3.4), e um quadro maior só para
//! o app identificado ([`limites::MAX_QUADRO_APP`]), que cabe a maior entrada que a
//! validação aceita, com os 256 KiB de anexos de D29.
//!
//! # Mensagens
//!
//! ```text
//! pedido   = {"v": 1, "op": "<operação>", "dados": {…}}
//! resposta = {"v": 1, "tipo": "<tipo>", "dados": {…}}
//! erro     = {"v": 1, "tipo": "erro", "dados": {"codigo": "…", "mensagem": "…"}}
//! ```
//!
//! - `v` é a versão ([`VERSAO`]); outra é recusada com
//!   [`ErroResposta::VersaoDesconhecida`]. `dados` pode faltar quando vazio.
//! - Leitura estrita: campo que o pedido não tem (`"revelar": true`, `"campo": "senha"`
//!   num `lista`, `"nivel"` numa entrada nova, `"geracao"` em qualquer lugar) recusa o
//!   pedido inteiro. Chave repetida também (`{"op": "lista", "op": "revelar"}`).
//! - **Todo segredo vai em base64** (senha mestra, PIN, segredos da entrada, anexos,
//!   bytes de comando e ambiente): sem escape de JSON, com tamanho previsível, e o
//!   texto lido zera ao sair. Texto comum vai como texto JSON.
//! - Nenhuma mensagem tem geração do cofre (D39): nem o daemon a manda, nem um campo
//!   `geracao` vindo do cliente é aceito.
//!
//! # Papéis (D5)
//!
//! O papel vem da conexão, nunca do quadro. Cada operação tem os papéis que podem
//! pedi-la ([`Operacao::papeis`]) e cada tipo de resposta os papéis que podem recebê-la
//! ([`TipoResposta::papeis`]). A ordem da leitura do pedido é: JSON → versão → operação
//! conhecida → **papel** → campos. Assim, uma operação de app ou de extensão pedida pelo
//! agente responde [`ErroResposta::NaoAutorizado`] seja qual for o resto do quadro.
//!
//! A `lista` do agente vai sem o nível de cada entrada (D50); a do app leva.
//!
//! **Nenhuma resposta que carrega credencial vai para o agente** (S1): as que carregam
//! ([`TipoResposta::carrega_credencial`]) são só de app ou de extensão, e
//! [`codificar_resposta`] recusa mandá-las a outro papel. A asserção de compilação no
//! fim deste arquivo e o teste da tabela prendem isso. Aprovar, responder aprovação e
//! mudar nível são só do app (S9, S10, V3.15); não há feature, variável nem argumento
//! que mude a tabela (D8).

use zeroize::Zeroizing;

use crate::limites;

mod campos;
mod erros;
mod json;
mod pedido;
pub mod quadro;
mod resposta;

#[cfg(test)]
mod testes;

pub use erros::{ErroResposta, MSG_COFRE_MUDOU, MSG_GERACAO_ESGOTADA, MSG_RETROCESSO, Problema};
pub use pedido::{Alvo, Decisao, Janela, Pedido, PedidoExecutar, Variavel};
pub use resposta::{
    AnexoDetalhe, DetalheEntrada, ExtraDetalhe, Fluxo, ItemLista, PedidoDeAprovacao, Resposta,
    Sugestao,
};

/// Versão do protocolo.
pub const VERSAO: u64 = 1;

/// Quem está do outro lado do socket (D5). O daemon decide pela conexão.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Papel {
    /// Qualquer cliente que não seja o app nem o host da extensão instalados: a CLI, o
    /// Claude, um script (D3).
    Agente,
    /// O app instalado, conferido (exe de root, sem depurador, sem `LD_*`).
    App,
    /// O host de native messaging instalado, filho do Chrome instalado.
    Extensao,
}

impl Papel {
    /// Todos os papéis.
    pub const TODOS: [Papel; 3] = [Papel::Agente, Papel::App, Papel::Extensao];

    /// Nome para log.
    pub fn nome(self) -> &'static str {
        match self {
            Papel::Agente => "agente",
            Papel::App => "app",
            Papel::Extensao => "extensao",
        }
    }
}

/// Conjunto de papéis (constante, para a tabela).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Papeis {
    pub agente: bool,
    pub app: bool,
    pub extensao: bool,
}

impl Papeis {
    const fn de(agente: bool, app: bool, extensao: bool) -> Self {
        Self {
            agente,
            app,
            extensao,
        }
    }

    /// O papel está no conjunto?
    pub const fn tem(self, p: Papel) -> bool {
        match p {
            Papel::Agente => self.agente,
            Papel::App => self.app,
            Papel::Extensao => self.extensao,
        }
    }
}

const AGENTE_E_APP: Papeis = Papeis::de(true, true, false);
const TODOS: Papeis = Papeis::de(true, true, true);
const SO_AGENTE: Papeis = Papeis::de(true, false, false);
const SO_APP: Papeis = Papeis::de(false, true, false);
const SO_EXTENSAO: Papeis = Papeis::de(false, false, true);
const APP_E_EXTENSAO: Papeis = Papeis::de(false, true, true);

macro_rules! tabela {
    ($nome:ident, $doc:literal, $( $var:ident = $txt:literal, $papeis:expr, $vdoc:literal; )+) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $nome {
            $( #[doc = $vdoc] $var, )+
        }

        impl $nome {
            /// Todos, na ordem da tabela.
            pub const TODAS: &'static [$nome] = &[$( $nome::$var, )+];

            /// Nome no quadro.
            pub const fn nome(self) -> &'static str {
                match self { $( $nome::$var => $txt, )+ }
            }

            /// Papéis permitidos.
            pub const fn papeis(self) -> Papeis {
                match self { $( $nome::$var => $papeis, )+ }
            }

            /// Pelo nome do quadro.
            pub fn de_nome(n: &str) -> Option<Self> {
                Self::TODAS.iter().copied().find(|o| o.nome() == n)
            }

            /// Este papel pode?
            pub const fn permitida(self, p: Papel) -> bool {
                self.papeis().tem(p)
            }
        }
    };
}

tabela! {
    Operacao, "Operação do protocolo e os papéis que podem pedi-la (D5).",
    Estado = "estado", TODOS, "Travado ou não, e se o cofre existe.";
    Destravar = "destravar", AGENTE_E_APP, "Destravar com a senha mestra (CLI no terminal do usuário ou app).";
    Travar = "travar", AGENTE_E_APP, "Travar agora.";
    Lista = "lista", AGENTE_E_APP, "Entradas visíveis (para o agente, sem as bloqueadas).";
    Executar = "executar", SO_AGENTE, "`ward run`: o daemon roda o comando com a credencial; volta a saída mascarada.";
    Ssh = "ssh", SO_AGENTE, "`ward ssh`.";
    Login = "login", SO_AGENTE, "`ward login`, pela extensão.";
    Playwright = "playwright", SO_AGENTE, "`ward playwright`: volta só o caminho do storage state.";
    EntradaPadrao = "entrada_padrao", SO_AGENTE, "Bytes para a entrada padrão da sessão aberta (vazio = fim).";
    Janela = "janela", SO_AGENTE, "Tamanho do terminal da sessão aberta.";
    CriarCofre = "criar_cofre", SO_APP, "Primeiro uso: senha mestra e PIN.";
    Detalhe = "detalhe", SO_APP, "Campos de uma entrada para o formulário, sem segredo.";
    Revelar = "revelar", SO_APP, "Valor de um segredo para mostrar (D4).";
    Copiar = "copiar", SO_APP, "Valor de um segredo para a área de transferência, pelo lado Rust do app (D4).";
    Totp = "totp", SO_APP, "Código TOTP atual da entrada.";
    SalvarEntrada = "salvar_entrada", SO_APP, "Criar ou editar uma entrada (sem nível; S9).";
    RemoverEntrada = "remover_entrada", SO_APP, "Apagar uma entrada.";
    MudarNivel = "mudar_nivel", SO_APP, "Mudar o nível de acesso, com PIN (S9).";
    ResponderAprovacao = "responder_aprovacao", SO_APP, "Resposta da janela de aprovação, com PIN para permitir (S10).";
    EmitirToken = "emitir_token", SO_APP, "Token de uso único do \"Conectar\" (D3).";
    TrocarPin = "trocar_pin", SO_APP, "Trocar o PIN.";
    TrocarSenha = "trocar_senha", SO_APP, "Trocar a senha mestra.";
    TempoTrava = "tempo_trava", SO_APP, "Tempo de trava por inatividade, em minutos.";
    Sugerir = "sugerir", SO_EXTENSAO, "Entradas `login` da origem da aba.";
    Preencher = "preencher", SO_EXTENSAO, "Usuário, senha e TOTP para preencher, se a origem bater (D9).";
    SalvarLogin = "salvar_login", SO_EXTENSAO, "Salvar um login novo visto pela extensão.";
}

tabela! {
    TipoResposta, "Tipo de resposta do daemon e os papéis que podem recebê-la.",
    Ok = "ok", TODOS, "Feito, sem dados.";
    Estado = "estado", TODOS, "Travado ou não, e se o cofre existe.";
    Erro = "erro", TODOS, "Erro com código e mensagem fixa.";
    Lista = "lista", AGENTE_E_APP, "Itens da lista (sem segredo).";
    Saida = "saida", SO_AGENTE, "Pedaço da saída já mascarada.";
    Fim = "fim", SO_AGENTE, "Fim da execução, com o código de saída.";
    SessaoPlaywright = "sessao_playwright", SO_AGENTE, "Caminho do storage state.";
    Detalhe = "detalhe", SO_APP, "Campos de uma entrada sem segredo.";
    Segredo = "segredo", SO_APP, "Valor de um segredo (revelar/copiar).";
    Token = "token", SO_APP, "Token do \"Conectar\".";
    PedidoDeAprovacao = "pedido_de_aprovacao", SO_APP, "O daemon pede ao app a janela de aprovação.";
    PedidoDeDestrave = "pedido_de_destrave", SO_APP, "O daemon pede ao app a tela de destravar (D16).";
    CodigoTotp = "codigo_totp", APP_E_EXTENSAO, "Código TOTP atual.";
    Sugestoes = "sugestoes", SO_EXTENSAO, "Entradas sugeridas para a aba.";
    Preenchimento = "preenchimento", SO_EXTENSAO, "Usuário, senha e TOTP para preencher.";
}

impl TipoResposta {
    /// A resposta leva valor de credencial (segredo, token ou código TOTP)?
    pub const fn carrega_credencial(self) -> bool {
        matches!(
            self,
            TipoResposta::Segredo
                | TipoResposta::Token
                | TipoResposta::CodigoTotp
                | TipoResposta::Preenchimento
        )
    }
}

// S1, na compilação: nenhuma resposta com credencial pode ir para o agente, e nenhuma
// operação de aprovação ou de nível pode vir dele (S9, S10, V3.15).
const _: () = {
    let mut i = 0;
    while i < TipoResposta::TODAS.len() {
        let t = TipoResposta::TODAS[i];
        assert!(!(t.carrega_credencial() && t.papeis().agente));
        i += 1;
    }
    assert!(!Operacao::ResponderAprovacao.papeis().agente);
    assert!(!Operacao::MudarNivel.papeis().agente);
    assert!(!Operacao::Revelar.papeis().agente);
    assert!(!Operacao::Copiar.papeis().agente);
    assert!(!Operacao::EmitirToken.papeis().agente);
    assert!(!Operacao::Preencher.papeis().agente);
    // O quadro do app cabe a maior entrada (D29) em base64 e os textos escapados.
    assert!(
        json::tamanho_base64(limites::MAX_ENTRADA)
            + 2 * limites::MAX_TEXTOS_ENTRADA
            + 64 * limites::KIB
            <= limites::MAX_QUADRO_APP
    );
    assert!(limites::MAX_QUADRO_COMUM < 2 * limites::MIB);
};

/// Por que não foi possível codificar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroCodificar {
    /// A mensagem não é deste papel.
    NaoPermitidaAoPapel,
    /// Passaria do limite do quadro.
    GrandeDemais,
    /// Campo fora do formato (texto acima do limite, por exemplo): o outro lado
    /// recusaria.
    Invalida,
}

impl core::fmt::Display for ErroCodificar {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::NaoPermitidaAoPapel => "mensagem não permitida a este papel",
            Self::GrandeDemais => "mensagem grande demais para o quadro",
            Self::Invalida => "mensagem fora do formato",
        })
    }
}

impl std::error::Error for ErroCodificar {}

/// Resposta do daemon que o cliente recusou (formato, versão ou tipo de outro papel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RespostaInvalida;

impl core::fmt::Display for RespostaInvalida {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("resposta inválida do Ward")
    }
}

impl std::error::Error for RespostaInvalida {}

/// Envelope lido até o nome: o resto do topo e os `dados` ficam para depois do papel.
struct Envelope {
    nome: String,
    topo: campos::Campos,
}

enum ErroEnvelope {
    Json,
    Versao,
}

/// Lê `{"v", <chave>, …}`: JSON, versão e nome.
fn ler_envelope(bytes: &[u8], chave: &str, max_nos: usize) -> Result<Envelope, ErroEnvelope> {
    let v = json::ler(bytes, max_nos).map_err(|_| ErroEnvelope::Json)?;
    let mut topo = campos::Campos::sem_conferir(v).map_err(|_| ErroEnvelope::Json)?;
    match topo.inteiro("v") {
        Ok(VERSAO) => {}
        Ok(_) => return Err(ErroEnvelope::Versao),
        Err(_) => return Err(ErroEnvelope::Json),
    }
    let nome = topo.texto(chave, 64).map_err(|_| ErroEnvelope::Json)?;
    Ok(Envelope { nome, topo })
}

impl Envelope {
    /// Os `dados` (ausente = vazio). Qualquer outra chave no topo (`"revelar": true`)
    /// recusa.
    fn dados(mut self) -> Result<campos::Campos, json::JsonInvalido> {
        // `v` e o nome já saíram; uma segunda cópia deles ficou e cai aqui ou no `fim`.
        self.topo.sem_repetidas()?;
        let dados = self.topo.objeto_ou_vazio("dados")?;
        self.topo.fim()?;
        Ok(dados)
    }
}

/// Lê um pedido do cliente com este papel. O erro já é a resposta a mandar.
///
/// Ordem: JSON → versão → operação conhecida → **papel** → resto do topo e campos. O
/// papel é conferido antes de olhar o resto, então nenhum campo a mais, a menos ou
/// malformado muda o "não autorizado" de uma operação de outro papel (V3.3).
pub fn decodificar_pedido(papel: Papel, bytes: &[u8]) -> Result<Pedido, ErroResposta> {
    let env = ler_envelope(bytes, "op", json::MAX_NOS).map_err(|e| match e {
        ErroEnvelope::Json => ErroResposta::PedidoInvalido,
        ErroEnvelope::Versao => ErroResposta::VersaoDesconhecida,
    })?;
    let op = Operacao::de_nome(&env.nome).ok_or(ErroResposta::OperacaoDesconhecida)?;
    if !op.permitida(papel) {
        return Err(ErroResposta::NaoAutorizado);
    }
    let dados = env.dados().map_err(|_| ErroResposta::PedidoInvalido)?;
    pedido::ler(op, dados).map_err(|_| ErroResposta::PedidoInvalido)
}

/// Codifica um pedido (lado do cliente). Recusa operação de outro papel e quadro acima
/// do limite do papel.
pub fn codificar_pedido(papel: Papel, p: &Pedido) -> Result<Zeroizing<Vec<u8>>, ErroCodificar> {
    let op = p.operacao();
    if !op.permitida(papel) {
        return Err(ErroCodificar::NaoPermitidaAoPapel);
    }
    p.conferir().map_err(|_| ErroCodificar::Invalida)?;
    json::codificar(quadro::limite_do_quadro(papel), |e| {
        e.abre_objeto();
        e.chave("v");
        e.inteiro(VERSAO);
        e.chave("op");
        e.texto(op.nome());
        e.chave("dados");
        e.objeto(|e| p.escrever(e));
        e.fecha_objeto();
    })
    .map_err(|_| ErroCodificar::GrandeDemais)
}

/// Codifica uma resposta do daemon para o cliente com este papel. Recusa o tipo que
/// não é do papel: uma resposta com credencial nunca sai para o agente (S1).
pub fn codificar_resposta(papel: Papel, r: &Resposta) -> Result<Zeroizing<Vec<u8>>, ErroCodificar> {
    let tipo = r.tipo();
    if !tipo.permitida(papel) {
        return Err(ErroCodificar::NaoPermitidaAoPapel);
    }
    // D50: o nível de acesso nunca vai ao agente na lista.
    if let Resposta::Lista(itens) = r
        && papel != Papel::App
        && itens.iter().any(|i| i.nivel.is_some())
    {
        return Err(ErroCodificar::NaoPermitidaAoPapel);
    }
    r.conferir(papel).map_err(|_| ErroCodificar::Invalida)?;
    json::codificar(quadro::LIMITE_RESPOSTA, |e| {
        e.abre_objeto();
        e.chave("v");
        e.inteiro(VERSAO);
        e.chave("tipo");
        e.texto(tipo.nome());
        e.chave("dados");
        e.objeto(|e| r.escrever(e));
        e.fecha_objeto();
    })
    .map_err(|_| ErroCodificar::GrandeDemais)
}

/// Lê uma resposta do daemon (lado do cliente com este papel). Recusa o tipo que não é
/// do papel: a CLI nunca aceita uma resposta com credencial.
pub fn decodificar_resposta(papel: Papel, bytes: &[u8]) -> Result<Resposta, RespostaInvalida> {
    let env = ler_envelope(bytes, "tipo", json::MAX_NOS_RESPOSTA).map_err(|_| RespostaInvalida)?;
    let tipo = TipoResposta::de_nome(&env.nome).ok_or(RespostaInvalida)?;
    if !tipo.permitida(papel) {
        return Err(RespostaInvalida);
    }
    let dados = env.dados().map_err(|_| RespostaInvalida)?;
    resposta::ler(papel, tipo, dados).map_err(|_| RespostaInvalida)
}

/// A resposta de erro para um quadro recusado pela leitura (V3.4). O daemon manda esta
/// e fecha a conexão.
pub fn erro_do_quadro(_e: quadro::ErroQuadro) -> ErroResposta {
    ErroResposta::QuadroInvalido
}
