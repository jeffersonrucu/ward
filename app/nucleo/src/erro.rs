//! Erros que a interface recebe. Só mensagens fixas em português: nenhum valor da
//! entrada, nenhum caminho e nenhum texto de erro do sistema (S13).

use std::fmt;

/// Erro de um comando do app. `Display` dá a mensagem que a tela mostra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErroApp {
    /// O socket do daemon não existe ou não aceitou a conexão.
    DaemonIndisponivel,
    /// O diretório ou o socket do daemon não passaram na conferência (dono, permissão,
    /// tipo). O app não conversa com ele.
    DaemonNaoConfiavel,
    /// O protocolo do daemon ainda não existe (Fase 20).
    SemProtocolo,
    /// A conexão aberta na partida (D47) não existe mais; o app se executa de novo.
    ConexaoPerdida,
    /// O pedido da interface não passou na validação (tamanho, formato).
    PedidoInvalido,
    /// O cofre está travado.
    Travado,
    /// A área de transferência não aceitou o valor.
    AreaTransferencia,
    /// A entrada não existe (ou o daemon não a mostra a este papel).
    EntradaInexistente,
    /// O campo pedido não existe na entrada.
    CampoInexistente,
    /// O daemon respondeu "não autorizado": a conexão não tem o papel app (D5, D47).
    NaoAutorizado,
    /// O daemon recusou o pedido por outro motivo (o código do protocolo não vem à
    /// interface).
    Recusado,
    /// A resposta do daemon não é a esperada para o pedido (tipo, formato, papel).
    RespostaInvalida,
    /// O RNG do sistema não respondeu; o gerador não usa fonte reserva.
    AleatorioIndisponivel,
    /// "Copiar" ou "revelar" do gerador avulso sem senha gerada (ou já descartada).
    SemSenhaGerada,
    /// Editar uma entrada com anexos: o protocolo não traz o conteúdo dos anexos para
    /// regravar, e salvar sem eles os apagaria (Fase 34).
    AnexosNaoMantidos,
}

impl ErroApp {
    /// Código estável, para a interface escolher a tela sem comparar texto.
    pub fn codigo(self) -> &'static str {
        match self {
            ErroApp::DaemonIndisponivel => "daemon-indisponivel",
            ErroApp::DaemonNaoConfiavel => "daemon-nao-confiavel",
            ErroApp::SemProtocolo => "sem-protocolo",
            ErroApp::ConexaoPerdida => "conexao-perdida",
            ErroApp::PedidoInvalido => "pedido-invalido",
            ErroApp::Travado => "travado",
            ErroApp::AreaTransferencia => "area-transferencia",
            ErroApp::EntradaInexistente => "entrada-inexistente",
            ErroApp::CampoInexistente => "campo-inexistente",
            ErroApp::NaoAutorizado => "nao-autorizado",
            ErroApp::Recusado => "recusado",
            ErroApp::RespostaInvalida => "resposta-invalida",
            ErroApp::AleatorioIndisponivel => "aleatorio-indisponivel",
            ErroApp::SemSenhaGerada => "sem-senha-gerada",
            ErroApp::AnexosNaoMantidos => "anexos-nao-mantidos",
        }
    }
}

impl fmt::Display for ErroApp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ErroApp::DaemonIndisponivel => "o daemon do Ward não está rodando",
            ErroApp::DaemonNaoConfiavel => {
                "o socket do daemon não é confiável; o app não se conecta"
            }
            ErroApp::SemProtocolo => "o protocolo do daemon ainda não está disponível",
            ErroApp::ConexaoPerdida => "a conexão com o daemon caiu; o app vai se reabrir",
            ErroApp::PedidoInvalido => "pedido inválido",
            ErroApp::Travado => "o cofre está travado",
            ErroApp::AreaTransferencia => "não foi possível usar a área de transferência",
            ErroApp::EntradaInexistente => "essa entrada não existe mais",
            ErroApp::CampoInexistente => "esse campo não existe na entrada",
            ErroApp::NaoAutorizado => "o daemon não reconheceu este app",
            ErroApp::Recusado => "o daemon recusou o pedido",
            ErroApp::RespostaInvalida => "resposta inválida do daemon",
            ErroApp::AleatorioIndisponivel => "o gerador aleatório do sistema não respondeu",
            ErroApp::SemSenhaGerada => "gere uma senha antes",
            ErroApp::AnexosNaoMantidos => {
                "esta entrada tem anexos; editá-la aqui ainda apagaria os anexos"
            }
        })
    }
}

impl std::error::Error for ErroApp {}
