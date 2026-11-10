//! Pedidos do cliente ao daemon: os campos de cada operação, a leitura estrita e a
//! escrita (lado do cliente).
//!
//! Formato e limite são conferidos aqui; regra de negócio (a entrada existe? o PIN
//! confere? a origem bate?) fica no daemon. Toda entrada nova ou editada ainda passa por
//! [`Rascunho::validar`] no daemon.

use zeroize::Zeroizing;

use super::Operacao;
use super::campos::{Campos, R, bytes_de, segredo_de, texto_de, textos_de};
use super::erros::ErroResposta;
use super::json::{Escritor, JsonInvalido, Valor};
use crate::entrada::{
    Autenticacao, Dados, Entrada, NivelAcesso, Rascunho, RascunhoAnexo, RascunhoDados,
    RascunhoExtra, Segredo, TipoEntrada, ValorExtra,
};
use crate::limits::{
    MAX_ARGUMENT, MAX_ARGUMENTS, MAX_ATTACHMENT_NAME, MAX_ATTACHMENTS, MAX_ATTACHMENTS_TOTAL,
    MAX_BACKUP_UUID, MAX_ENV_VARIABLES, MAX_EXTRA_NAME, MAX_EXTRA_VALUE, MAX_EXTRAS,
    MAX_LOCK_MINUTES, MAX_NAME, MAX_NOTE, MAX_PIN_DIGITS, MAX_SECRET, MAX_TAG, MAX_TAGS, MAX_TEXT,
    MAX_URL, MIN_LOCK_MINUTES, MIN_PIN_DIGITS,
};

/// Bytes que zeram ao sair (argumento, diretório, nome de variável).
pub type Bytes = Zeroizing<Vec<u8>>;

/// Uma variável do ambiente da CLI (D13: o daemon tira `LD_*` e afins na Fase 26).
#[derive(Debug)]
pub struct Variavel {
    pub nome: Bytes,
    pub valor: Segredo,
}

/// `ward run`: entrada, comando, diretório e ambiente da CLI, em bytes (podem não ser
/// UTF-8). Sem NUL em nenhum, sem `=` no nome de variável.
#[derive(Debug)]
pub struct PedidoExecutar {
    pub entrada: String,
    pub argv: Vec<Bytes>,
    pub cwd: Bytes,
    pub ambiente: Vec<Variavel>,
    /// Fase 54b (D68): o que é a entrada padrão da CLI.
    pub tipo_entrada: TipoEntradaPadrao,
}

/// Fase 54b (D68): o que é a entrada padrão do `ward run`/`ward ssh`, pelo `fstat` da
/// CLI. Quem diz é o cliente: o daemon não tem como conferir `terminal` (um cliente que
/// mente, ou um pty do próprio agente, passa por terminal); `nula` ele faz valer (a
/// entrada do filho fecha e dado que chegar mata o comando). Pipe, arquivo e o resto
/// vão à janela com aviso e, no nível "com aprovação", só passam com a confirmação a
/// mais ([`Pedido::ResponderAprovacao`] com `entrada_confirmada`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoEntradaPadrao {
    /// Um terminal (`isatty`).
    Terminal,
    /// `/dev/null` ou fechada: nenhum dado vai ao comando.
    Nula,
    /// Pipe ou socket.
    Pipe,
    /// Arquivo comum.
    Arquivo,
    /// Outro dispositivo (`/dev/zero`, `/dev/urandom`…) ou o que não deu para ver.
    Outra,
}

impl TipoEntradaPadrao {
    /// Todos, na ordem do quadro.
    pub const TODOS: [TipoEntradaPadrao; 5] = [
        TipoEntradaPadrao::Terminal,
        TipoEntradaPadrao::Nula,
        TipoEntradaPadrao::Pipe,
        TipoEntradaPadrao::Arquivo,
        TipoEntradaPadrao::Outra,
    ];

    /// Nome no quadro.
    pub fn nome(self) -> &'static str {
        match self {
            TipoEntradaPadrao::Terminal => "terminal",
            TipoEntradaPadrao::Nula => "nula",
            TipoEntradaPadrao::Pipe => "pipe",
            TipoEntradaPadrao::Arquivo => "arquivo",
            TipoEntradaPadrao::Outra => "outra",
        }
    }

    /// Pelo nome do quadro.
    pub fn de_nome(n: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|t| t.nome() == n)
    }

    /// O comando recebe dados que a janela de aprovação não mostra (nem terminal, nem
    /// nula).
    pub fn oculta(self) -> bool {
        !matches!(self, TipoEntradaPadrao::Terminal | TipoEntradaPadrao::Nula)
    }
}

/// Tamanho do terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Janela {
    pub linhas: u16,
    pub colunas: u16,
}

/// Qual segredo da entrada revelar ou copiar: um campo do tipo (pelo identificador do
/// protocolo, ex.: `senha`, `chave_privada`) ou um extra secreto pelo nome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Alvo {
    Campo(String),
    Extra(String),
}

impl Alvo {
    /// O segredo da entrada que este alvo aponta (D65 b: `revelar`/`copiar` do app), ou
    /// `None` se o tipo não tem o campo, o campo não está preenchido ou o extra não
    /// existe ou não é secreto. Anexo não é alvo.
    pub fn segredo_em<'a>(&self, e: &'a Entrada) -> Option<&'a Segredo> {
        match self {
            Alvo::Extra(nome) => e.extras().iter().find_map(|x| match x.valor() {
                ValorExtra::Segredo(v) if x.nome() == nome => Some(v),
                _ => None,
            }),
            Alvo::Campo(id) => segredo_do_campo(e.dados(), id),
        }
    }
}

/// Campo secreto do tipo pelo identificador do protocolo ([`CAMPOS_SECRETOS`]).
fn segredo_do_campo<'a>(d: &'a Dados, id: &str) -> Option<&'a Segredo> {
    let auth = |a: &'a Autenticacao| match (a, id) {
        (Autenticacao::Senha(s), "senha") => Some(s),
        (Autenticacao::Chave { chave_privada, .. }, "chave_privada") => Some(chave_privada),
        (Autenticacao::Chave { passphrase, .. }, "passphrase") => passphrase.as_ref(),
        _ => None,
    };
    match (d, id) {
        (Dados::Login(l), "senha") => Some(&l.senha),
        (Dados::Login(l), "totp_semente") => l.totp_semente.as_ref(),
        (Dados::Banco(b), "senha") => Some(&b.senha),
        (Dados::Servidor(v), _) => auth(&v.autenticacao),
        (Dados::FtpSftp(f), _) => auth(&f.autenticacao),
        (Dados::Api(a), "token") => Some(&a.token),
        (Dados::Aws(a), "secret_access_key") => Some(&a.secret_access_key),
        (Dados::Aws(a), "session_token") => a.session_token.as_ref(),
        (Dados::Smtp(m), "senha") => Some(&m.senha),
        (Dados::NotaSegura(n), "texto") => Some(&n.texto),
        _ => None,
    }
}

/// Botão da janela de aprovação. Os três "Permitir" pedem PIN; "Negar" e "Confirmar"
/// não.
///
/// "Confirmar" (Fase 44c, D9) só vale num pedido que o daemon marcou como
/// [`crate::protocolo::PedidoDeAprovacao::confirmacao`]: o preenchimento numa origem
/// `http` (`localhost`/`127.0.0.1`) de uma entrada que não pede PIN (nível livre, ou o
/// popup da extensão). Quem confere isso é o daemon; aqui é só o formato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decisao {
    Negar,
    PermitirUmaVez,
    PermitirUmaHora,
    PermitirSempre,
    Confirmar,
}

impl Decisao {
    const TODAS: [Decisao; 5] = [
        Decisao::Negar,
        Decisao::PermitirUmaVez,
        Decisao::PermitirUmaHora,
        Decisao::PermitirSempre,
        Decisao::Confirmar,
    ];

    /// Nome no quadro.
    pub fn nome(self) -> &'static str {
        match self {
            Decisao::Negar => "negar",
            Decisao::PermitirUmaVez => "uma_vez",
            Decisao::PermitirUmaHora => "uma_hora",
            Decisao::PermitirSempre => "sempre",
            Decisao::Confirmar => "confirmar",
        }
    }

    /// Pede PIN?
    pub fn pede_pin(self) -> bool {
        !matches!(self, Decisao::Negar | Decisao::Confirmar)
    }
}

/// `ward login` (Fase 43): o que a extensão fez com o pedido de login, dito ao daemon
/// no `resultado_login`. Nenhum valor, só o desfecho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultadoLogin {
    /// Preencheu, enviou o formulário e limpou os campos; a página seguinte não tem
    /// mais campo de senha visível.
    Enviado,
    /// Enviou, mas a página seguinte (da mesma origem) ainda pede a senha: o site não
    /// aceitou o login.
    Recusado,
    /// A aba não tem um formulário de login que dê para enviar com segurança (nenhum ou
    /// mais de um campo de senha visível, `GET`, destino ou `target` fora).
    SemFormulario,
    /// A aba navegou (ou o documento trocou) entre a sonda e o preenchimento.
    AbaMudou,
    /// Qualquer outra falha da extensão (injeção recusada, aba fechada).
    Falha,
}

impl ResultadoLogin {
    /// Todos, na ordem do quadro.
    pub const TODOS: [ResultadoLogin; 5] = [
        ResultadoLogin::Enviado,
        ResultadoLogin::Recusado,
        ResultadoLogin::SemFormulario,
        ResultadoLogin::AbaMudou,
        ResultadoLogin::Falha,
    ];

    /// Nome no quadro.
    pub fn nome(self) -> &'static str {
        match self {
            ResultadoLogin::Enviado => "enviado",
            ResultadoLogin::Recusado => "recusado",
            ResultadoLogin::SemFormulario => "sem_formulario",
            ResultadoLogin::AbaMudou => "aba_mudou",
            ResultadoLogin::Falha => "falha",
        }
    }

    /// Pelo nome do quadro.
    pub fn de_nome(n: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|r| r.nome() == n)
    }
}

/// Id de um pedido de `ward login` (Fase 43): 32 letras e dígitos ASCII, do RNG do
/// sistema (o mesmo gerador do token do "Conectar").
pub fn id_login_ok(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// Pedido do cliente. Segredos em [`Segredo`] (zera ao sair, `Debug` sem valor).
#[derive(Debug)]
pub enum Pedido {
    Estado,
    Destravar {
        senha: Segredo,
    },
    Travar,
    Lista,
    Executar(PedidoExecutar),
    Ssh {
        entrada: String,
        /// Comando remoto (vazio = shell interativo).
        comando: Vec<Bytes>,
        /// Token do "Conectar" (D3); sem token vale o nível.
        token: Option<Segredo>,
        janela: Option<Janela>,
        /// Fase 54b (D68): o que é a entrada padrão da CLI.
        tipo_entrada: TipoEntradaPadrao,
    },
    Login {
        entrada: String,
    },
    Playwright {
        entrada: String,
    },
    EntradaPadrao {
        dados: Segredo,
    },
    Janela(Janela),
    CriarCofre {
        senha: Segredo,
        pin: Segredo,
    },
    Detalhe {
        entrada: String,
    },
    Revelar {
        entrada: String,
        alvo: Alvo,
    },
    Copiar {
        entrada: String,
        alvo: Alvo,
    },
    Totp {
        entrada: String,
    },
    SalvarEntrada {
        /// Nome atual da entrada editada; `None` cria.
        anterior: Option<String>,
        rascunho: Rascunho,
        /// Fase 39b (V5.12): nomes dos anexos da entrada editada (`anterior`) que
        /// continuam nela. O daemon copia o conteúdo deles para o rascunho, à frente dos
        /// anexos novos; o conteúdo de um anexo guardado nunca sai do daemon. Anexo da
        /// entrada antiga fora desta lista é removido. Vazio ao criar.
        manter_anexos: Vec<String>,
    },
    RemoverEntrada {
        entrada: String,
    },
    MudarNivel {
        entrada: String,
        nivel: NivelAcesso,
        pin: Segredo,
    },
    ResponderAprovacao {
        pedido: u64,
        decisao: Decisao,
        /// Presente se e só se a decisão pede PIN.
        pin: Option<Segredo>,
        /// Fase 54b (D68): o usuário marcou, na janela, que permite a entrada padrão
        /// que ela não mostra. Só com um "Permitir"; o daemon exige `true` num pedido
        /// com `entrada_oculta` e recusa `true` nos outros.
        entrada_confirmada: bool,
    },
    EmitirToken {
        entrada: String,
    },
    TrocarPin {
        atual: Segredo,
        novo: Segredo,
    },
    TrocarSenha {
        atual: Segredo,
        nova: Segredo,
    },
    TempoTrava {
        minutos: u32,
    },
    Sugerir {
        origem: String,
    },
    Preencher {
        entrada: String,
        origem: String,
    },
    SalvarLogin {
        /// Sempre `RascunhoDados::Login`.
        rascunho: Rascunho,
    },
    /// `ward login` (Fase 43): a extensão espera o próximo pedido de login, sem campo.
    AguardarLogin,
    /// `ward login` (Fase 43): a extensão pede usuário, senha e TOTP do pedido `id` para
    /// a origem do documento da aba ativa; o daemon compara a origem (D9).
    PreencherLogin {
        id: String,
        origem: String,
    },
    /// `ward login` (Fase 43): o desfecho do pedido `id` na aba.
    ResultadoLogin {
        id: String,
        resultado: ResultadoLogin,
    },
    /// D57: abrir a instância verdadeira do app (filho do daemon), sem nenhum campo.
    AbrirApp,
    /// Versões do backup local (Fase 45), sem campo.
    Backups,
    /// Tempo de trava e backup externo ligado ou não (Fase 38), sem campo.
    Configuracao,
    /// Restaurar uma versão do backup local (D17, D39; Fase 45): o nome exato do arquivo
    /// (`ward-AAAAMMDD-HHMMSS[-N].ward`, nunca um caminho), a senha mestra daquela versão
    /// e o PIN do cofre atual (S9: restaurar volta níveis e PIN).
    RestaurarBackup {
        versao: String,
        senha: Segredo,
        pin: Segredo,
    },
    /// Configuração do backup externo (Fase 46), sem campo.
    BackupExterno,
    /// Liga ou desliga o backup externo (D17, D30; Fase 46). Ligado exige o UUID do
    /// sistema de arquivos do pendrive (`[0-9A-Fa-f-]`, até 36, sem `-` nas pontas);
    /// desligado pode guardar o UUID ou não.
    DefinirBackupExterno {
        ligado: bool,
        uuid: Option<String>,
    },
    /// Pendrives vistos pelo udisks2 (Fase 46), sem campo.
    Pendrives,
}

impl Pedido {
    /// A operação deste pedido.
    pub fn operacao(&self) -> Operacao {
        match self {
            Pedido::Estado => Operacao::Estado,
            Pedido::Destravar { .. } => Operacao::Destravar,
            Pedido::Travar => Operacao::Travar,
            Pedido::Lista => Operacao::Lista,
            Pedido::Executar(_) => Operacao::Executar,
            Pedido::Ssh { .. } => Operacao::Ssh,
            Pedido::Login { .. } => Operacao::Login,
            Pedido::Playwright { .. } => Operacao::Playwright,
            Pedido::EntradaPadrao { .. } => Operacao::EntradaPadrao,
            Pedido::Janela(_) => Operacao::Janela,
            Pedido::CriarCofre { .. } => Operacao::CriarCofre,
            Pedido::Detalhe { .. } => Operacao::Detalhe,
            Pedido::Revelar { .. } => Operacao::Revelar,
            Pedido::Copiar { .. } => Operacao::Copiar,
            Pedido::Totp { .. } => Operacao::Totp,
            Pedido::SalvarEntrada { .. } => Operacao::SalvarEntrada,
            Pedido::RemoverEntrada { .. } => Operacao::RemoverEntrada,
            Pedido::MudarNivel { .. } => Operacao::MudarNivel,
            Pedido::ResponderAprovacao { .. } => Operacao::ResponderAprovacao,
            Pedido::EmitirToken { .. } => Operacao::EmitirToken,
            Pedido::TrocarPin { .. } => Operacao::TrocarPin,
            Pedido::TrocarSenha { .. } => Operacao::TrocarSenha,
            Pedido::TempoTrava { .. } => Operacao::TempoTrava,
            Pedido::Sugerir { .. } => Operacao::Sugerir,
            Pedido::Preencher { .. } => Operacao::Preencher,
            Pedido::SalvarLogin { .. } => Operacao::SalvarLogin,
            Pedido::AguardarLogin => Operacao::AguardarLogin,
            Pedido::PreencherLogin { .. } => Operacao::PreencherLogin,
            Pedido::ResultadoLogin { .. } => Operacao::ResultadoLogin,
            Pedido::AbrirApp => Operacao::AbrirApp,
            Pedido::Backups => Operacao::Backups,
            Pedido::Configuracao => Operacao::Configuracao,
            Pedido::RestaurarBackup { .. } => Operacao::RestaurarBackup,
            Pedido::BackupExterno => Operacao::BackupExterno,
            Pedido::DefinirBackupExterno { .. } => Operacao::DefinirBackupExterno,
            Pedido::Pendrives => Operacao::Pendrives,
        }
    }

    /// Confere o que a leitura conferiria, antes de escrever (o cliente não manda o que
    /// o daemon recusaria por formato).
    pub(crate) fn conferir(&self) -> R<()> {
        let ok = |c: bool| if c { Ok(()) } else { Err(JsonInvalido) };
        match self {
            Pedido::Destravar { senha } => ok(senha_ok(senha)),
            Pedido::CriarCofre { senha, pin } => ok(senha_ok(senha) && pin_ok(pin)),
            Pedido::MudarNivel { entrada, pin, .. } => ok(nome_ok(entrada) && pin_ok(pin)),
            Pedido::TrocarPin { atual, novo } => ok(pin_ok(atual) && pin_ok(novo)),
            Pedido::TrocarSenha { atual, nova } => ok(senha_ok(atual) && senha_ok(nova)),
            Pedido::ResponderAprovacao {
                decisao,
                pin,
                entrada_confirmada,
                ..
            } => ok((match pin {
                Some(p) => decisao.pede_pin() && pin_ok(p),
                None => !decisao.pede_pin(),
            }) && (!entrada_confirmada || decisao.pede_pin())),
            Pedido::TempoTrava { minutos } => {
                ok((MIN_LOCK_MINUTES..=MAX_LOCK_MINUTES).contains(minutos))
            }
            Pedido::Executar(x) => ok(nome_ok(&x.entrada)
                && comando_ok(&x.argv, true)
                && !x.cwd.is_empty()
                && x.cwd.len() <= MAX_ARGUMENT
                && sem_nul(&x.cwd)
                && x.ambiente.len() <= MAX_ENV_VARIABLES
                && x.ambiente.iter().all(variavel_ok)),
            Pedido::Ssh {
                entrada,
                comando,
                token,
                janela,
                ..
            } => ok(nome_ok(entrada)
                && comando_ok(comando, false)
                && token
                    .as_ref()
                    .is_none_or(|t| !t.is_empty() && t.len() <= MAX_NAME)
                && janela.is_none_or(janela_ok)),
            Pedido::Janela(j) => ok(janela_ok(*j)),
            Pedido::EntradaPadrao { dados } => ok(dados.len() <= MAX_ARGUMENT),
            Pedido::Login { entrada }
            | Pedido::Playwright { entrada }
            | Pedido::Detalhe { entrada }
            | Pedido::Totp { entrada }
            | Pedido::RemoverEntrada { entrada }
            | Pedido::EmitirToken { entrada } => ok(nome_ok(entrada)),
            Pedido::Revelar { entrada, alvo } | Pedido::Copiar { entrada, alvo } => {
                ok(nome_ok(entrada) && alvo_ok(alvo))
            }
            Pedido::Sugerir { origem } => ok(origem.len() <= MAX_URL),
            Pedido::Preencher { entrada, origem } => {
                ok(nome_ok(entrada) && origem.len() <= MAX_URL)
            }
            Pedido::PreencherLogin { id, origem } => ok(id_login_ok(id) && origem.len() <= MAX_URL),
            Pedido::ResultadoLogin { id, .. } => ok(id_login_ok(id)),
            Pedido::SalvarEntrada {
                anterior,
                rascunho,
                manter_anexos,
            } => ok(anterior.as_deref().is_none_or(nome_ok)
                && rascunho_ok(rascunho)
                && manter_anexos_ok(anterior.is_some(), manter_anexos, rascunho)),
            Pedido::SalvarLogin { rascunho } => {
                ok(matches!(rascunho.dados, RascunhoDados::Login { .. }) && rascunho_ok(rascunho))
            }
            Pedido::RestaurarBackup { versao, senha, pin } => {
                ok(versao_ok(versao) && senha_ok(senha) && pin_ok(pin))
            }
            Pedido::DefinirBackupExterno { ligado, uuid } => ok(backup_externo_ok(*ligado, uuid)),
            Pedido::Estado
            | Pedido::Travar
            | Pedido::Lista
            | Pedido::AbrirApp
            | Pedido::Backups
            | Pedido::Configuracao
            | Pedido::BackupExterno
            | Pedido::AguardarLogin
            | Pedido::Pendrives => Ok(()),
        }
    }

    /// Escreve os `dados` do pedido.
    pub(crate) fn escrever(&self, e: &mut Escritor) {
        match self {
            Pedido::Estado
            | Pedido::Travar
            | Pedido::Lista
            | Pedido::AbrirApp
            | Pedido::Backups
            | Pedido::Configuracao
            | Pedido::BackupExterno
            | Pedido::AguardarLogin
            | Pedido::Pendrives => {}
            Pedido::DefinirBackupExterno { ligado, uuid } => {
                e.chave("ligado");
                e.logico(*ligado);
                if let Some(u) = uuid {
                    e.chave("uuid");
                    e.texto(u);
                }
            }
            Pedido::RestaurarBackup { versao, senha, pin } => {
                e.chave("versao");
                e.texto(versao);
                e.chave("senha");
                e.base64(senha.expor());
                e.chave("pin");
                e.base64(pin.expor());
            }
            Pedido::Destravar { senha } => {
                e.chave("senha");
                e.base64(senha.expor());
            }
            Pedido::Executar(x) => {
                e.chave("entrada");
                e.texto(&x.entrada);
                e.chave("argv");
                escrever_bytes(e, &x.argv);
                e.chave("cwd");
                e.base64(&x.cwd);
                e.chave("tipo_entrada");
                e.texto(x.tipo_entrada.nome());
                e.chave("ambiente");
                e.lista(|e| {
                    for v in &x.ambiente {
                        e.item();
                        e.objeto(|e| {
                            e.chave("nome");
                            e.base64(&v.nome);
                            e.chave("valor");
                            e.base64(v.valor.expor());
                        });
                    }
                });
            }
            Pedido::Ssh {
                entrada,
                comando,
                token,
                janela,
                tipo_entrada,
            } => {
                e.chave("entrada");
                e.texto(entrada);
                e.chave("comando");
                escrever_bytes(e, comando);
                e.chave("tipo_entrada");
                e.texto(tipo_entrada.nome());
                if let Some(t) = token {
                    e.chave("token");
                    e.base64(t.expor());
                }
                if let Some(j) = janela {
                    e.chave("janela");
                    e.objeto(|e| escrever_janela(e, *j));
                }
            }
            Pedido::Login { entrada }
            | Pedido::Playwright { entrada }
            | Pedido::Detalhe { entrada }
            | Pedido::Totp { entrada }
            | Pedido::RemoverEntrada { entrada }
            | Pedido::EmitirToken { entrada } => {
                e.chave("entrada");
                e.texto(entrada);
            }
            Pedido::EntradaPadrao { dados } => {
                e.chave("dados");
                e.base64(dados.expor());
            }
            Pedido::Janela(j) => escrever_janela(e, *j),
            Pedido::CriarCofre { senha, pin } => {
                e.chave("senha");
                e.base64(senha.expor());
                e.chave("pin");
                e.base64(pin.expor());
            }
            Pedido::Revelar { entrada, alvo } | Pedido::Copiar { entrada, alvo } => {
                e.chave("entrada");
                e.texto(entrada);
                match alvo {
                    Alvo::Campo(c) => {
                        e.chave("campo");
                        e.texto(c);
                    }
                    Alvo::Extra(x) => {
                        e.chave("extra");
                        e.texto(x);
                    }
                }
            }
            Pedido::SalvarEntrada {
                anterior,
                rascunho,
                manter_anexos,
            } => {
                if let Some(a) = anterior {
                    e.chave("anterior");
                    e.texto(a);
                }
                if !manter_anexos.is_empty() {
                    e.chave("manter_anexos");
                    e.lista(|e| {
                        for n in manter_anexos {
                            e.item();
                            e.texto(n);
                        }
                    });
                }
                e.chave("entrada");
                e.objeto(|e| escrever_rascunho(e, rascunho));
            }
            Pedido::SalvarLogin { rascunho } => {
                e.chave("entrada");
                e.objeto(|e| escrever_rascunho(e, rascunho));
            }
            Pedido::MudarNivel {
                entrada,
                nivel,
                pin,
            } => {
                e.chave("entrada");
                e.texto(entrada);
                e.chave("nivel");
                e.texto(nivel.nome());
                e.chave("pin");
                e.base64(pin.expor());
            }
            Pedido::ResponderAprovacao {
                pedido,
                decisao,
                pin,
                entrada_confirmada,
            } => {
                e.chave("pedido");
                e.inteiro(*pedido);
                e.chave("decisao");
                e.texto(decisao.nome());
                if let Some(p) = pin {
                    e.chave("pin");
                    e.base64(p.expor());
                }
                if *entrada_confirmada {
                    e.chave("entrada_confirmada");
                    e.logico(true);
                }
            }
            Pedido::TrocarPin { atual, novo } => {
                e.chave("atual");
                e.base64(atual.expor());
                e.chave("novo");
                e.base64(novo.expor());
            }
            Pedido::TrocarSenha { atual, nova } => {
                e.chave("atual");
                e.base64(atual.expor());
                e.chave("nova");
                e.base64(nova.expor());
            }
            Pedido::TempoTrava { minutos } => {
                e.chave("minutos");
                e.inteiro(u64::from(*minutos));
            }
            Pedido::Sugerir { origem } => {
                e.chave("origem");
                e.texto(origem);
            }
            Pedido::Preencher { entrada, origem } => {
                e.chave("entrada");
                e.texto(entrada);
                e.chave("origem");
                e.texto(origem);
            }
            Pedido::PreencherLogin { id, origem } => {
                e.chave("id");
                e.texto(id);
                e.chave("origem");
                e.texto(origem);
            }
            Pedido::ResultadoLogin { id, resultado } => {
                e.chave("id");
                e.texto(id);
                e.chave("resultado");
                e.texto(resultado.nome());
            }
        }
    }
}

// ---- Conferências de formato (as mesmas na leitura e na escrita) ----

fn nome_ok(n: &str) -> bool {
    !n.is_empty() && n.len() <= MAX_NAME
}

/// Nome exato de uma versão de backup (Fase 45): sem barra, sem `..`, sem caminho.
fn versao_ok(v: &str) -> bool {
    #[cfg(unix)]
    {
        crate::backup::nome_valido(v)
    }
    #[cfg(not(unix))]
    {
        let _ = v;
        false
    }
}

/// Backup externo no formato que a configuração aceita (D45): UUID `[0-9A-Fa-f-]`, de 1
/// a 36, sem `-` nas pontas; ligado exige o UUID.
fn backup_externo_ok(ligado: bool, uuid: &Option<String>) -> bool {
    crate::configuracao::BackupExterno::novo(ligado, uuid.as_deref()).is_ok()
}

fn senha_ok(s: &Segredo) -> bool {
    !s.is_empty() && s.len() <= MAX_SECRET
}

/// PIN: de 4 a 6 dígitos ASCII. Quem confere contra o hash é o daemon (S9).
fn pin_ok(p: &Segredo) -> bool {
    let b = p.expor();
    (MIN_PIN_DIGITS..=MAX_PIN_DIGITS).contains(&b.len()) && b.iter().all(u8::is_ascii_digit)
}

fn sem_nul(b: &[u8]) -> bool {
    !b.contains(&0)
}

fn comando_ok(argv: &[Bytes], exige: bool) -> bool {
    (!exige || !argv.is_empty())
        && argv.len() <= MAX_ARGUMENTS
        && argv.iter().all(|a| a.len() <= MAX_ARGUMENT && sem_nul(a))
}

fn variavel_ok(v: &Variavel) -> bool {
    !v.nome.is_empty()
        && v.nome.len() <= MAX_ARGUMENT
        && sem_nul(&v.nome)
        && !v.nome.contains(&b'=')
        && v.valor.len() <= MAX_ARGUMENT
        && sem_nul(v.valor.expor())
}

fn janela_ok(j: Janela) -> bool {
    j.linhas > 0 && j.colunas > 0
}

fn alvo_ok(a: &Alvo) -> bool {
    match a {
        Alvo::Campo(c) => CAMPOS_SECRETOS.contains(&c.as_str()),
        Alvo::Extra(x) => !x.is_empty() && x.len() <= MAX_EXTRA_NAME,
    }
}

/// Identificadores dos campos secretos de todos os tipos (os mesmos do rascunho).
pub(crate) const CAMPOS_SECRETOS: &[&str] = &[
    "senha",
    "totp_semente",
    "chave_privada",
    "passphrase",
    "token",
    "secret_access_key",
    "session_token",
    "texto",
];

fn rascunho_ok(r: &Rascunho) -> bool {
    let anexos: usize = r.anexos.iter().map(|a| a.conteudo.len()).sum();
    let textos = [&r.grupo, &r.ambiente, &r.projeto];
    nome_ok(&r.nome)
        && textos.iter().all(|t| t.len() <= MAX_TEXT)
        && r.tags.len() <= MAX_TAGS
        && r.tags.iter().all(|t| t.len() <= MAX_TAG)
        && r.extras.len() <= MAX_EXTRAS
        && r.extras
            .iter()
            .all(|x| x.nome.len() <= MAX_EXTRA_NAME && x.valor.len() <= MAX_EXTRA_VALUE)
        && r.anexos.len() <= MAX_ATTACHMENTS
        && r.anexos.iter().all(|a| a.nome.len() <= MAX_ATTACHMENT_NAME)
        && anexos <= MAX_ATTACHMENTS_TOTAL
        && campos_do_rascunho(&r.dados).iter().all(|c| match c {
            CampoRascunho::Texto(_, t) => t.len() <= MAX_TEXT,
            CampoRascunho::Segredo(k, s) => s.len() <= max_segredo(k),
        })
}

/// Fase 39b: "manter anexo" só na edição; nomes não vazios, dentro do limite, sem
/// repetição, e mantidos mais novos cabem em [`MAX_ATTACHMENTS`]. Se o nome existe na
/// entrada editada é regra do daemon ([`juntar_anexos_mantidos`]).
fn manter_anexos_ok(editando: bool, nomes: &[String], r: &Rascunho) -> bool {
    (editando || nomes.is_empty())
        && nomes.len().saturating_add(r.anexos.len()) <= MAX_ATTACHMENTS
        && nomes.iter().enumerate().all(|(i, n)| {
            !n.is_empty() && n.len() <= MAX_ATTACHMENT_NAME && !nomes[..i].contains(n)
        })
}

/// Fase 39b (V5.12): põe à frente dos anexos novos do rascunho uma cópia de cada anexo
/// de `antiga` pedido em `nomes`, na ordem pedida. Nome que a entrada editada não tem, ou
/// repetido, é [`ErroResposta::PedidoInvalido`] (nada muda no rascunho). Depois disto o
/// rascunho passa por [`Rascunho::validar`] inteiro: soma de D29, nome repetido com um
/// anexo novo, orçamento da máscara. O conteúdo é copiado dentro do daemon; não há
/// operação que o devolva (S1, S11).
pub fn juntar_anexos_mantidos(
    rascunho: &mut Rascunho,
    antiga: &Entrada,
    nomes: &[String],
) -> Result<(), ErroResposta> {
    let mut mantidos = Vec::with_capacity(nomes.len());
    for (i, nome) in nomes.iter().enumerate() {
        if nomes[..i].contains(nome) {
            return Err(ErroResposta::PedidoInvalido);
        }
        let a = antiga
            .anexos()
            .iter()
            .find(|a| a.nome() == nome)
            .ok_or(ErroResposta::PedidoInvalido)?;
        mantidos.push(RascunhoAnexo {
            nome: a.nome().to_owned(),
            conteudo: Segredo::de_bytes(a.conteudo().expor().to_vec()),
        });
    }
    if mantidos.len().saturating_add(rascunho.anexos.len()) > MAX_ATTACHMENTS {
        return Err(ErroResposta::PedidoInvalido);
    }
    mantidos.append(&mut rascunho.anexos);
    rascunho.anexos = mantidos;
    Ok(())
}

fn max_segredo(campo: &str) -> usize {
    if campo == "texto" {
        MAX_NOTE
    } else {
        MAX_SECRET
    }
}

// ---- Leitura ----

/// Lê os `dados` da operação (o papel já foi conferido).
pub(crate) fn ler(op: Operacao, mut c: Campos) -> R<Pedido> {
    let p = match op {
        Operacao::Estado => Pedido::Estado,
        Operacao::Travar => Pedido::Travar,
        Operacao::Lista => Pedido::Lista,
        Operacao::AbrirApp => Pedido::AbrirApp,
        Operacao::Backups => Pedido::Backups,
        Operacao::Configuracao => Pedido::Configuracao,
        Operacao::BackupExterno => Pedido::BackupExterno,
        Operacao::Pendrives => Pedido::Pendrives,
        Operacao::AguardarLogin => Pedido::AguardarLogin,
        Operacao::PreencherLogin => Pedido::PreencherLogin {
            id: c.texto("id", MAX_NAME)?,
            origem: c.texto("origem", MAX_URL)?,
        },
        Operacao::ResultadoLogin => Pedido::ResultadoLogin {
            id: c.texto("id", MAX_NAME)?,
            resultado: ResultadoLogin::de_nome(&c.texto("resultado", MAX_NAME)?)
                .ok_or(JsonInvalido)?,
        },
        Operacao::DefinirBackupExterno => Pedido::DefinirBackupExterno {
            ligado: c.logico("ligado")?,
            uuid: c.texto_opc("uuid", MAX_BACKUP_UUID)?,
        },
        Operacao::RestaurarBackup => Pedido::RestaurarBackup {
            versao: c.texto("versao", MAX_NAME)?,
            senha: c.segredo("senha", MAX_SECRET)?,
            pin: c.segredo("pin", MAX_PIN_DIGITS)?,
        },
        Operacao::Destravar => Pedido::Destravar {
            senha: c.segredo("senha", MAX_SECRET)?,
        },
        Operacao::Executar => {
            let entrada = c.texto("entrada", MAX_NAME)?;
            let argv = ler_bytes(c.lista("argv", MAX_ARGUMENTS)?)?;
            let cwd = c.bytes("cwd", MAX_ARGUMENT)?;
            let tipo_entrada = ler_tipo_entrada(&mut c)?;
            let mut ambiente = Vec::new();
            for v in c.lista("ambiente", MAX_ENV_VARIABLES)? {
                let mut o = Campos::de(v)?;
                let nome = o.bytes("nome", MAX_ARGUMENT)?;
                let valor = o.segredo("valor", MAX_ARGUMENT)?;
                o.fim()?;
                ambiente.push(Variavel { nome, valor });
            }
            Pedido::Executar(PedidoExecutar {
                entrada,
                argv,
                cwd,
                ambiente,
                tipo_entrada,
            })
        }
        Operacao::Ssh => {
            let entrada = c.texto("entrada", MAX_NAME)?;
            let comando = ler_bytes(c.lista_ou_vazia("comando", MAX_ARGUMENTS)?)?;
            let token = c.segredo_opc("token", MAX_NAME)?;
            let janela = match c.tirar("janela") {
                Some(v) => Some(ler_janela(Campos::de(v)?)?),
                None => None,
            };
            let tipo_entrada = ler_tipo_entrada(&mut c)?;
            Pedido::Ssh {
                entrada,
                comando,
                token,
                janela,
                tipo_entrada,
            }
        }
        Operacao::Login => Pedido::Login {
            entrada: c.texto("entrada", MAX_NAME)?,
        },
        Operacao::Playwright => Pedido::Playwright {
            entrada: c.texto("entrada", MAX_NAME)?,
        },
        Operacao::EntradaPadrao => Pedido::EntradaPadrao {
            dados: c.segredo("dados", MAX_ARGUMENT)?,
        },
        Operacao::Janela => {
            let j = ler_janela(c)?;
            return conferido(Pedido::Janela(j));
        }
        Operacao::CriarCofre => Pedido::CriarCofre {
            senha: c.segredo("senha", MAX_SECRET)?,
            pin: c.segredo("pin", MAX_PIN_DIGITS)?,
        },
        Operacao::Detalhe => Pedido::Detalhe {
            entrada: c.texto("entrada", MAX_NAME)?,
        },
        Operacao::Revelar | Operacao::Copiar => {
            let entrada = c.texto("entrada", MAX_NAME)?;
            let alvo = match (c.tirar("campo"), c.tirar("extra")) {
                (Some(v), None) => Alvo::Campo(texto_de(v, MAX_NAME)?),
                (None, Some(v)) => Alvo::Extra(texto_de(v, MAX_EXTRA_NAME)?),
                _ => return Err(JsonInvalido),
            };
            if op == Operacao::Revelar {
                Pedido::Revelar { entrada, alvo }
            } else {
                Pedido::Copiar { entrada, alvo }
            }
        }
        Operacao::Totp => Pedido::Totp {
            entrada: c.texto("entrada", MAX_NAME)?,
        },
        Operacao::SalvarEntrada => {
            let anterior = c.texto_opc("anterior", MAX_NAME)?;
            let manter_anexos = textos_de(
                c.lista_ou_vazia("manter_anexos", MAX_ATTACHMENTS)?,
                MAX_ATTACHMENT_NAME,
            )?;
            let rascunho = ler_rascunho(c.objeto("entrada")?)?;
            Pedido::SalvarEntrada {
                anterior,
                rascunho,
                manter_anexos,
            }
        }
        Operacao::RemoverEntrada => Pedido::RemoverEntrada {
            entrada: c.texto("entrada", MAX_NAME)?,
        },
        Operacao::MudarNivel => {
            let entrada = c.texto("entrada", MAX_NAME)?;
            let nivel = NivelAcesso::de_nome(&c.texto("nivel", MAX_NAME)?).ok_or(JsonInvalido)?;
            let pin = c.segredo("pin", MAX_PIN_DIGITS)?;
            Pedido::MudarNivel {
                entrada,
                nivel,
                pin,
            }
        }
        Operacao::ResponderAprovacao => {
            let pedido = c.inteiro("pedido")?;
            let nome = c.texto("decisao", MAX_NAME)?;
            let decisao = Decisao::TODAS
                .into_iter()
                .find(|d| d.nome() == nome)
                .ok_or(JsonInvalido)?;
            let pin = c.segredo_opc("pin", MAX_PIN_DIGITS)?;
            // Fase 54b: ausente = não marcada; presente só como `true`.
            let entrada_confirmada = match c.tirar("entrada_confirmada") {
                None => false,
                Some(Valor::Logico(true)) => true,
                Some(_) => return Err(JsonInvalido),
            };
            Pedido::ResponderAprovacao {
                pedido,
                decisao,
                pin,
                entrada_confirmada,
            }
        }
        Operacao::EmitirToken => Pedido::EmitirToken {
            entrada: c.texto("entrada", MAX_NAME)?,
        },
        Operacao::TrocarPin => Pedido::TrocarPin {
            atual: c.segredo("atual", MAX_PIN_DIGITS)?,
            novo: c.segredo("novo", MAX_PIN_DIGITS)?,
        },
        Operacao::TrocarSenha => Pedido::TrocarSenha {
            atual: c.segredo("atual", MAX_SECRET)?,
            nova: c.segredo("nova", MAX_SECRET)?,
        },
        Operacao::TempoTrava => Pedido::TempoTrava {
            minutos: u32::try_from(c.inteiro("minutos")?).map_err(|_| JsonInvalido)?,
        },
        Operacao::Sugerir => Pedido::Sugerir {
            origem: c.texto("origem", MAX_URL)?,
        },
        Operacao::Preencher => Pedido::Preencher {
            entrada: c.texto("entrada", MAX_NAME)?,
            origem: c.texto("origem", MAX_URL)?,
        },
        Operacao::SalvarLogin => Pedido::SalvarLogin {
            rascunho: ler_rascunho(c.objeto("entrada")?)?,
        },
    };
    c.fim()?;
    conferido(p)
}

fn conferido(p: Pedido) -> R<Pedido> {
    p.conferir()?;
    Ok(p)
}

/// Fase 54b (D68): o tipo da entrada padrão, obrigatório (sem padrão: o cliente diz).
fn ler_tipo_entrada(c: &mut Campos) -> R<TipoEntradaPadrao> {
    TipoEntradaPadrao::de_nome(&c.texto("tipo_entrada", MAX_NAME)?).ok_or(JsonInvalido)
}

fn ler_bytes(v: Vec<Valor>) -> R<Vec<Bytes>> {
    v.into_iter().map(|a| bytes_de(a, MAX_ARGUMENT)).collect()
}

fn escrever_bytes(e: &mut Escritor, l: &[Bytes]) {
    e.lista(|e| {
        for a in l {
            e.item();
            e.base64(a);
        }
    });
}

fn ler_janela(mut c: Campos) -> R<Janela> {
    let linhas = u16::try_from(c.inteiro("linhas")?).map_err(|_| JsonInvalido)?;
    let colunas = u16::try_from(c.inteiro("colunas")?).map_err(|_| JsonInvalido)?;
    c.fim()?;
    Ok(Janela { linhas, colunas })
}

fn escrever_janela(e: &mut Escritor, j: Janela) {
    e.chave("linhas");
    e.inteiro(u64::from(j.linhas));
    e.chave("colunas");
    e.inteiro(u64::from(j.colunas));
}

// ---- Rascunho (formulário da entrada) ----
//
// {"nome", "grupo", "ambiente", "projeto"?, "tags"?: [..], "tipo",
//  "campos"?: {<campo do tipo>: texto | base64}, "extras"?: [{"nome", "valor", "segredo"}],
//  "anexos"?: [{"nome", "conteudo"}]}
//
// Sem "nivel": entrada nova nasce "com aprovação" e editar mantém o nível (S9). Campo
// do tipo ausente é "não informado" (texto vazio ou segredo `None`).

/// Um campo do tipo como o protocolo o vê.
enum CampoRascunho<'a> {
    Texto(&'static str, &'a String),
    Segredo(&'static str, &'a Segredo),
}

/// Os campos preenchidos do rascunho, na ordem do tipo.
fn campos_do_rascunho(d: &RascunhoDados) -> Vec<CampoRascunho<'_>> {
    use CampoRascunho::{Segredo as S, Texto as T};
    let mut v = Vec::new();
    macro_rules! t {
        ($k:literal, $x:expr) => {
            if !$x.is_empty() {
                v.push(T($k, $x));
            }
        };
    }
    macro_rules! s {
        ($k:literal, $x:expr) => {
            if let Some(x) = $x {
                v.push(S($k, x));
            }
        };
    }
    match d {
        RascunhoDados::Login {
            url,
            usuario,
            senha,
            totp_semente,
        } => {
            t!("url", url);
            t!("usuario", usuario);
            s!("senha", senha);
            s!("totp_semente", totp_semente);
        }
        RascunhoDados::Banco {
            motor,
            host,
            porta,
            base,
            usuario,
            senha,
            tunel,
        } => {
            t!("motor", motor);
            t!("host", host);
            t!("porta", porta);
            t!("base", base);
            t!("usuario", usuario);
            s!("senha", senha);
            t!("tunel", tunel);
        }
        RascunhoDados::Servidor {
            host,
            porta,
            usuario,
            senha,
            chave_privada,
            passphrase,
            salto,
        } => {
            t!("host", host);
            t!("porta", porta);
            t!("usuario", usuario);
            s!("senha", senha);
            s!("chave_privada", chave_privada);
            s!("passphrase", passphrase);
            t!("salto", salto);
        }
        RascunhoDados::Api {
            url,
            token,
            variavel,
        } => {
            t!("url", url);
            s!("token", token);
            t!("variavel", variavel);
        }
        RascunhoDados::Aws {
            access_key_id,
            secret_access_key,
            regiao,
            role_arn,
            session_token,
        } => {
            t!("access_key_id", access_key_id);
            s!("secret_access_key", secret_access_key);
            t!("regiao", regiao);
            t!("role_arn", role_arn);
            s!("session_token", session_token);
        }
        RascunhoDados::Smtp {
            host,
            porta,
            usuario,
            senha,
            seguranca,
        } => {
            t!("host", host);
            t!("porta", porta);
            t!("usuario", usuario);
            s!("senha", senha);
            t!("seguranca", seguranca);
        }
        RascunhoDados::FtpSftp {
            protocolo,
            host,
            porta,
            usuario,
            senha,
            chave_privada,
            passphrase,
        } => {
            t!("protocolo", protocolo);
            t!("host", host);
            t!("porta", porta);
            t!("usuario", usuario);
            s!("senha", senha);
            s!("chave_privada", chave_privada);
            s!("passphrase", passphrase);
        }
        RascunhoDados::NotaSegura { texto } => {
            s!("texto", texto);
        }
        RascunhoDados::Generico => {}
    }
    v
}

fn ler_dados(tipo: TipoEntrada, mut c: Campos) -> R<RascunhoDados> {
    let t = |c: &mut Campos, k: &str| c.texto_ou_vazio(k, MAX_TEXT);
    let s = |c: &mut Campos, k: &str| c.segredo_opc(k, max_segredo(k));
    let d = match tipo {
        TipoEntrada::Login => RascunhoDados::Login {
            url: t(&mut c, "url")?,
            usuario: t(&mut c, "usuario")?,
            senha: s(&mut c, "senha")?,
            totp_semente: s(&mut c, "totp_semente")?,
        },
        TipoEntrada::Banco => RascunhoDados::Banco {
            motor: t(&mut c, "motor")?,
            host: t(&mut c, "host")?,
            porta: t(&mut c, "porta")?,
            base: t(&mut c, "base")?,
            usuario: t(&mut c, "usuario")?,
            senha: s(&mut c, "senha")?,
            tunel: t(&mut c, "tunel")?,
        },
        TipoEntrada::Servidor => RascunhoDados::Servidor {
            host: t(&mut c, "host")?,
            porta: t(&mut c, "porta")?,
            usuario: t(&mut c, "usuario")?,
            senha: s(&mut c, "senha")?,
            chave_privada: s(&mut c, "chave_privada")?,
            passphrase: s(&mut c, "passphrase")?,
            salto: t(&mut c, "salto")?,
        },
        TipoEntrada::Api => RascunhoDados::Api {
            url: t(&mut c, "url")?,
            token: s(&mut c, "token")?,
            variavel: t(&mut c, "variavel")?,
        },
        TipoEntrada::Aws => RascunhoDados::Aws {
            access_key_id: t(&mut c, "access_key_id")?,
            secret_access_key: s(&mut c, "secret_access_key")?,
            regiao: t(&mut c, "regiao")?,
            role_arn: t(&mut c, "role_arn")?,
            session_token: s(&mut c, "session_token")?,
        },
        TipoEntrada::Smtp => RascunhoDados::Smtp {
            host: t(&mut c, "host")?,
            porta: t(&mut c, "porta")?,
            usuario: t(&mut c, "usuario")?,
            senha: s(&mut c, "senha")?,
            seguranca: t(&mut c, "seguranca")?,
        },
        TipoEntrada::FtpSftp => RascunhoDados::FtpSftp {
            protocolo: t(&mut c, "protocolo")?,
            host: t(&mut c, "host")?,
            porta: t(&mut c, "porta")?,
            usuario: t(&mut c, "usuario")?,
            senha: s(&mut c, "senha")?,
            chave_privada: s(&mut c, "chave_privada")?,
            passphrase: s(&mut c, "passphrase")?,
        },
        TipoEntrada::NotaSegura => RascunhoDados::NotaSegura {
            texto: s(&mut c, "texto")?,
        },
        TipoEntrada::Generico => RascunhoDados::Generico,
    };
    // Campo de outro tipo (ou inventado, como "nivel") recusa.
    c.fim()?;
    Ok(d)
}

fn ler_rascunho(mut c: Campos) -> R<Rascunho> {
    let nome = c.texto("nome", MAX_NAME)?;
    let grupo = c.texto_ou_vazio("grupo", MAX_TEXT)?;
    let ambiente = c.texto_ou_vazio("ambiente", MAX_TEXT)?;
    let projeto = c.texto_ou_vazio("projeto", MAX_TEXT)?;
    let tags = textos_de(c.lista_ou_vazia("tags", MAX_TAGS)?, MAX_TAG)?;
    let tipo = TipoEntrada::de_nome(&c.texto("tipo", MAX_NAME)?).ok_or(JsonInvalido)?;
    let dados = ler_dados(tipo, c.objeto_ou_vazio("campos")?)?;
    let mut extras = Vec::new();
    for v in c.lista_ou_vazia("extras", MAX_EXTRAS)? {
        let mut o = Campos::de(v)?;
        let nome = o.texto("nome", MAX_EXTRA_NAME)?;
        let valor = o.segredo("valor", MAX_EXTRA_VALUE)?;
        let segredo = o.logico("segredo")?;
        o.fim()?;
        extras.push(RascunhoExtra {
            nome,
            valor,
            segredo,
        });
    }
    let mut anexos = Vec::new();
    let mut soma = 0usize;
    for v in c.lista_ou_vazia("anexos", MAX_ATTACHMENTS)? {
        let mut o = Campos::de(v)?;
        let nome = o.texto("nome", MAX_ATTACHMENT_NAME)?;
        // O limite de cada um é o que ainda cabe na soma (D29).
        let conteudo = segredo_de(
            o.tirar("conteudo").ok_or(JsonInvalido)?,
            MAX_ATTACHMENTS_TOTAL - soma,
        )?;
        o.fim()?;
        soma += conteudo.len();
        anexos.push(RascunhoAnexo { nome, conteudo });
    }
    c.fim()?;
    Ok(Rascunho {
        nome,
        grupo,
        ambiente,
        projeto,
        tags,
        dados,
        extras,
        anexos,
    })
}

fn escrever_rascunho(e: &mut Escritor, r: &Rascunho) {
    e.chave("nome");
    e.texto(&r.nome);
    e.chave("grupo");
    e.texto(&r.grupo);
    e.chave("ambiente");
    e.texto(&r.ambiente);
    if !r.projeto.is_empty() {
        e.chave("projeto");
        e.texto(&r.projeto);
    }
    e.chave("tags");
    e.lista(|e| {
        for t in &r.tags {
            e.item();
            e.texto(t);
        }
    });
    e.chave("tipo");
    e.texto(r.dados.tipo().nome());
    e.chave("campos");
    e.objeto(|e| {
        for c in campos_do_rascunho(&r.dados) {
            match c {
                CampoRascunho::Texto(k, t) => {
                    e.chave(k);
                    e.texto(t);
                }
                CampoRascunho::Segredo(k, s) => {
                    e.chave(k);
                    e.base64(s.expor());
                }
            }
        }
    });
    e.chave("extras");
    e.lista(|e| {
        for x in &r.extras {
            e.item();
            e.objeto(|e| {
                e.chave("nome");
                e.texto(&x.nome);
                e.chave("valor");
                e.base64(x.valor.expor());
                e.chave("segredo");
                e.logico(x.segredo);
            });
        }
    });
    e.chave("anexos");
    e.lista(|e| {
        for a in &r.anexos {
            e.item();
            e.objeto(|e| {
                e.chave("nome");
                e.texto(&a.nome);
                e.chave("conteudo");
                e.base64(a.conteudo.expor());
            });
        }
    });
}
